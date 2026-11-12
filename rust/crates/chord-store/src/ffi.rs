//! Тонкая обёртка над системным `libsqlite3` — единственное место крейта, где есть `unsafe`.
//!
//! Согласовано с владельцем (вариант A, `waves/RW-31/SQLITE-DECISION.md`): новых крейтов нет,
//! линкуется системная библиотека (`#[link(name = "sqlite3")]`, 3.54), весь `unsafe` изолирован
//! здесь, а наружу торчит безопасный API `Db`/`Stmt`. Каждый `unsafe`-блок снабжён `// SAFETY:`.
//!
//! Инварианты, на которых держится безопасность:
//!
//! - `Db::raw` — живой handle `sqlite3*`, закрывается ровно один раз в `Drop`.
//! - `Stmt<'a>` держит `&'a Db`, поэтому не может пережить соединение, которым владеет.
//! - `Stmt::raw` — подготовленный statement, финализируется ровно один раз в `Drop`; методы
//!   чтения колонок доступны только после `Step::Row` (это проверяет вызывающий код).
//! - Все строки, уходящие в C, — `CString` без внутренних NUL; значения биндятся
//!   (`sqlite3_bind_*`), SQL со значениями не конкатенируется.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::{self, NonNull};
use std::slice;

/// Непрозрачный тип `sqlite3` (его раскладку знает только библиотека).
#[repr(C)]
pub(crate) struct Sqlite3 {
    _private: [u8; 0],
}

/// Непрозрачный тип `sqlite3_stmt`.
#[repr(C)]
pub(crate) struct Sqlite3Stmt {
    _private: [u8; 0],
}

pub(crate) const SQLITE_OK: c_int = 0;
pub(crate) const SQLITE_CORRUPT: i32 = 11;
pub(crate) const SQLITE_CONSTRAINT: i32 = 19;
pub(crate) const SQLITE_MISUSE: i32 = 21;
pub(crate) const SQLITE_RANGE: i32 = 25;

const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_NULL: c_int = 5;

const SQLITE_OPEN_READWRITE: c_int = 0x0000_0002;
const SQLITE_OPEN_CREATE: c_int = 0x0000_0004;

/// `SQLITE_TRANSIENT`: SQLite копирует строку в момент `bind`, указатель дальше не нужен.
const SQLITE_TRANSIENT: *mut c_void = usize::MAX as *mut c_void;

type ExecCallback =
    unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int;

// SAFETY: имена/сигнатуры совпадают с `sqlite3.h` 3.54; линковка — системная `libsqlite3`.
#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open_v2(
        filename: *const c_char,
        db: *mut *mut Sqlite3,
        flags: c_int,
        vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut Sqlite3) -> c_int;
    fn sqlite3_errmsg(db: *mut Sqlite3) -> *const c_char;
    fn sqlite3_extended_errcode(db: *mut Sqlite3) -> c_int;
    fn sqlite3_exec(
        db: *mut Sqlite3,
        sql: *const c_char,
        callback: Option<ExecCallback>,
        argument: *mut c_void,
        error_message: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_free(pointer: *mut c_void);
    fn sqlite3_busy_timeout(db: *mut Sqlite3, milliseconds: c_int) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut Sqlite3,
        sql: *const c_char,
        bytes: c_int,
        statement: *mut *mut Sqlite3Stmt,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_bind_text(
        statement: *mut Sqlite3Stmt,
        index: c_int,
        text: *const c_char,
        bytes: c_int,
        destructor: *mut c_void,
    ) -> c_int;
    fn sqlite3_bind_int64(statement: *mut Sqlite3Stmt, index: c_int, value: i64) -> c_int;
    fn sqlite3_bind_null(statement: *mut Sqlite3Stmt, index: c_int) -> c_int;
    fn sqlite3_step(statement: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_reset(statement: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_clear_bindings(statement: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_column_count(statement: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_column_type(statement: *mut Sqlite3Stmt, column: c_int) -> c_int;
    fn sqlite3_column_text(statement: *mut Sqlite3Stmt, column: c_int) -> *const u8;
    fn sqlite3_column_bytes(statement: *mut Sqlite3Stmt, column: c_int) -> c_int;
    fn sqlite3_column_int64(statement: *mut Sqlite3Stmt, column: c_int) -> i64;
    fn sqlite3_finalize(statement: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_changes(db: *mut Sqlite3) -> c_int;
}

/// Ошибка уровня FFI: `code` — расширенный код SQLite, `message` — текст для человека.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FfiError {
    pub(crate) code: i32,
    pub(crate) message: String,
}

impl FfiError {
    pub(crate) fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Основной код (младший байт расширенного): `19` — `SQLITE_CONSTRAINT`.
    pub(crate) fn primary_code(&self) -> i32 {
        self.code & 0xff
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "SQLite {}: {}", self.code, self.message)
    }
}

/// Итог `sqlite3_step`: есть строка или работа закончена.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Row,
    Done,
}

/// Соединение с БД. Закрывается в `Drop`.
pub(crate) struct Db {
    raw: NonNull<Sqlite3>,
}

impl Db {
    /// Открывает (или создаёт) БД по пути; `:memory:` — БД в памяти.
    pub(crate) fn open(path: &str) -> Result<Self, FfiError> {
        let c_path = CString::new(path)
            .map_err(|_| FfiError::new(SQLITE_MISUSE, "путь содержит NUL-байт"))?;
        let mut raw: *mut Sqlite3 = ptr::null_mut();
        // SAFETY: `c_path` — валидная NUL-терминированная строка на всё время вызова;
        // `&mut raw` — корректный out-параметр, `vfs` = NULL (VFS по умолчанию).
        let code = unsafe {
            sqlite3_open_v2(
                c_path.as_ptr(),
                &mut raw,
                SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
                ptr::null(),
            )
        };
        let Some(db) = NonNull::new(raw).map(|raw| Self { raw }) else {
            return Err(FfiError::new(
                if code == SQLITE_OK {
                    SQLITE_MISUSE
                } else {
                    code
                },
                "sqlite3_open_v2 вернул NULL-handle",
            ));
        };
        if code != SQLITE_OK {
            // Handle, даже неудачный, нужно закрыть — это делает Drop у `db`.
            return Err(db.failure(code));
        }
        Ok(db)
    }

    /// Текст последней ошибки соединения (копия — указатель принадлежит SQLite).
    pub(crate) fn errmsg(&self) -> String {
        // SAFETY: `raw` — живой handle; `sqlite3_errmsg` возвращает NUL-терминированную
        // строку, принадлежащую соединению. Копируем её сразу, до следующего вызова SQLite.
        unsafe {
            let pointer = sqlite3_errmsg(self.raw.as_ptr());
            if pointer.is_null() {
                return "<нет сообщения>".to_owned();
            }
            CStr::from_ptr(pointer).to_string_lossy().into_owned()
        }
    }

    fn extended_errcode(&self) -> i32 {
        // SAFETY: `raw` — живой handle соединения, функция только читает его состояние.
        unsafe { sqlite3_extended_errcode(self.raw.as_ptr()) }
    }

    fn failure(&self, fallback: c_int) -> FfiError {
        let code = self.extended_errcode();
        FfiError::new(
            if code == SQLITE_OK { fallback } else { code },
            self.errmsg(),
        )
    }

    /// `sqlite3_exec`: один или несколько SQL-операторов без параметров.
    pub(crate) fn exec(&self, sql: &str) -> Result<(), FfiError> {
        let c_sql =
            CString::new(sql).map_err(|_| FfiError::new(SQLITE_MISUSE, "SQL содержит NUL-байт"))?;
        let mut error_message: *mut c_char = ptr::null_mut();
        // SAFETY: handle жив; `c_sql` — валидная C-строка; callback = None (результат не нужен),
        // `error_message` — out-параметр, который SQLite заполнит через sqlite3_malloc.
        let code = unsafe {
            sqlite3_exec(
                self.raw.as_ptr(),
                c_sql.as_ptr(),
                None,
                ptr::null_mut(),
                &mut error_message,
            )
        };
        let text = if error_message.is_null() {
            None
        } else {
            // SAFETY: SQLite выделила строку через sqlite3_malloc; читаем её до освобождения.
            let text = unsafe { CStr::from_ptr(error_message) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: тот же указатель, выделенный SQLite, ещё не освобождён и больше не нужен.
            unsafe { sqlite3_free(error_message.cast()) };
            Some(text)
        };
        if code == SQLITE_OK {
            return Ok(());
        }
        Err(match text {
            Some(message) => FfiError::new(self.extended_errcode().max(code), message),
            None => self.failure(code),
        })
    }

    /// `PRAGMA busy_timeout` (мс): ждать блокировку записи, а не падать сразу.
    pub(crate) fn busy_timeout(&self, milliseconds: c_int) -> Result<(), FfiError> {
        // SAFETY: `raw` — живой handle; функция только настраивает соединение.
        let code = unsafe { sqlite3_busy_timeout(self.raw.as_ptr(), milliseconds) };
        if code == SQLITE_OK {
            Ok(())
        } else {
            Err(self.failure(code))
        }
    }

    /// Готовит один SQL-оператор с параметрами (`?1`, `?2`, …).
    pub(crate) fn prepare(&self, sql: &str) -> Result<Stmt<'_>, FfiError> {
        let c_sql =
            CString::new(sql).map_err(|_| FfiError::new(SQLITE_MISUSE, "SQL содержит NUL-байт"))?;
        let mut raw: *mut Sqlite3Stmt = ptr::null_mut();
        // SAFETY: handle жив; `c_sql` валиден; `bytes = -1` — читать до NUL; `tail` = NULL
        // (хвост после первого оператора не используется); `&mut raw` — out-параметр.
        let code = unsafe {
            sqlite3_prepare_v2(
                self.raw.as_ptr(),
                c_sql.as_ptr(),
                -1,
                &mut raw,
                ptr::null_mut(),
            )
        };
        match NonNull::new(raw) {
            Some(raw) if code == SQLITE_OK => Ok(Stmt { raw, db: self }),
            Some(raw) => {
                // SAFETY: SQLite вернула подготовленный statement — финализируем его ровно раз.
                unsafe { sqlite3_finalize(raw.as_ptr()) };
                Err(self.failure(code))
            }
            None => Err(self.failure(code)),
        }
    }

    /// Число строк, изменённых последним `INSERT`/`UPDATE`/`DELETE`.
    pub(crate) fn changes(&self) -> u64 {
        // SAFETY: `raw` — живой handle соединения.
        let count = unsafe { sqlite3_changes(self.raw.as_ptr()) };
        u64::try_from(count).unwrap_or(0)
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        // SAFETY: handle открыт в `open` и ещё не закрыт; statement'ы не могут пережить `Db`
        // (их держит заимствование `&Db`), поэтому close не вернёт SQLITE_BUSY. Код закрытия
        // не проверяем: освобождать handle всё равно больше нечем.
        unsafe {
            sqlite3_close(self.raw.as_ptr());
        }
    }
}

/// Подготовленный statement. Финализируется в `Drop`; живёт не дольше своего `Db`.
pub(crate) struct Stmt<'a> {
    raw: NonNull<Sqlite3Stmt>,
    db: &'a Db,
}

impl Stmt<'_> {
    fn check(&self, code: c_int) -> Result<(), FfiError> {
        if code == SQLITE_OK {
            Ok(())
        } else {
            Err(self.db.failure(code))
        }
    }

    fn index(column: usize) -> Result<c_int, FfiError> {
        c_int::try_from(column)
            .map_err(|_| FfiError::new(SQLITE_RANGE, "индекс параметра/колонки вне c_int"))
    }

    /// Биндит текст. Внутренний NUL недопустим (C-строка) — ошибка, а не обрезание.
    pub(crate) fn bind_text(&mut self, index: usize, value: &str) -> Result<(), FfiError> {
        let c_value = CString::new(value)
            .map_err(|_| FfiError::new(SQLITE_MISUSE, "значение содержит NUL-байт"))?;
        let bytes = c_int::try_from(c_value.as_bytes().len())
            .map_err(|_| FfiError::new(SQLITE_RANGE, "значение длиннее c_int"))?;
        let index = Self::index(index)?;
        // SAFETY: statement жив; `c_value` валиден на время вызова; SQLITE_TRANSIENT просит
        // SQLite скопировать данные сразу, поэтому указатель после вызова не хранится.
        let code = unsafe {
            sqlite3_bind_text(
                self.raw.as_ptr(),
                index,
                c_value.as_ptr(),
                bytes,
                SQLITE_TRANSIENT,
            )
        };
        self.check(code)
    }

    pub(crate) fn bind_i64(&mut self, index: usize, value: i64) -> Result<(), FfiError> {
        let index = Self::index(index)?;
        // SAFETY: statement жив; биндинг целого не владеет памятью.
        let code = unsafe { sqlite3_bind_int64(self.raw.as_ptr(), index, value) };
        self.check(code)
    }

    pub(crate) fn bind_null(&mut self, index: usize) -> Result<(), FfiError> {
        let index = Self::index(index)?;
        // SAFETY: statement жив; биндинг NULL не владеет памятью.
        let code = unsafe { sqlite3_bind_null(self.raw.as_ptr(), index) };
        self.check(code)
    }

    /// Шаг выполнения: `Row` — есть строка (колонки можно читать), `Done` — конец.
    pub(crate) fn step(&mut self) -> Result<Step, FfiError> {
        // SAFETY: statement жив и подготовлен; вызов можно повторять до `Done`.
        let code = unsafe { sqlite3_step(self.raw.as_ptr()) };
        match code {
            SQLITE_ROW => Ok(Step::Row),
            SQLITE_DONE => Ok(Step::Done),
            _ => Err(self.db.failure(code)),
        }
    }

    /// Сбрасывает statement для повторного использования (параметры сохраняются).
    #[allow(dead_code)] // часть согласованного FFI-слота (TZ §1): переиспользование prepare
    pub(crate) fn reset(&mut self) -> Result<(), FfiError> {
        // SAFETY: statement жив и не выполняется в этот момент (владение `&mut self`).
        let code = unsafe { sqlite3_reset(self.raw.as_ptr()) };
        self.check(code)
    }

    /// Снимает все биндинги (после `reset` statement готов к новым значениям).
    #[allow(dead_code)] // часть согласованного FFI-слота (TZ §1)
    pub(crate) fn clear_bindings(&mut self) -> Result<(), FfiError> {
        // SAFETY: statement жив; функция только очищает биндинги.
        let code = unsafe { sqlite3_clear_bindings(self.raw.as_ptr()) };
        self.check(code)
    }

    /// Число колонок в результате.
    #[allow(dead_code)] // диагностический слот FFI; прикладной код читает колонки по индексу
    pub(crate) fn column_count(&self) -> c_int {
        // SAFETY: statement жив и подготовлен.
        unsafe { sqlite3_column_count(self.raw.as_ptr()) }
    }

    /// `true`, если колонка текущей строки — SQL NULL.
    pub(crate) fn column_is_null(&self, column: usize) -> Result<bool, FfiError> {
        let column = Self::index(column)?;
        // SAFETY: statement жив; вызывающий читает колонки только после `Step::Row`.
        let kind = unsafe { sqlite3_column_type(self.raw.as_ptr(), column) };
        Ok(kind == SQLITE_NULL)
    }

    /// Значение TEXT текущей строки; `None` для SQL NULL.
    pub(crate) fn column_text(&self, column: usize) -> Result<Option<String>, FfiError> {
        let column = Self::index(column)?;
        // SAFETY: statement жив; вызывающий читает колонки только после `Step::Row`.
        // `sqlite3_column_type` различает SQL NULL от пустой строки — без этого NULL
        // превратился бы в `Some("")`.
        let kind = unsafe { sqlite3_column_type(self.raw.as_ptr(), column) };
        if kind == SQLITE_NULL {
            return Ok(None);
        }
        // SAFETY: statement жив; `sqlite3_column_text` конвертирует значение и возвращает
        // указатель на буфер, принадлежащий statement; `sqlite3_column_bytes` даёт длину.
        let (pointer, bytes) = unsafe {
            (
                sqlite3_column_text(self.raw.as_ptr(), column),
                sqlite3_column_bytes(self.raw.as_ptr(), column),
            )
        };
        if pointer.is_null() {
            // Нулевая длина может вернуть NULL-указатель — это пустая строка, не NULL.
            return if bytes == 0 {
                Ok(Some(String::new()))
            } else {
                Err(FfiError::new(
                    SQLITE_CORRUPT,
                    "колонка TEXT вернула NULL-указатель",
                ))
            };
        }
        let length = usize::try_from(bytes)
            .map_err(|_| FfiError::new(SQLITE_CORRUPT, "длина колонки отрицательна"))?;
        // SAFETY: `pointer` валиден для `length` байт, пока statement не сдвинут на новую
        // строку; копируем данные в Vec до возврата управления вызывающему.
        let data = unsafe { slice::from_raw_parts(pointer, length) }.to_vec();
        String::from_utf8(data)
            .map(Some)
            .map_err(|_| FfiError::new(SQLITE_CORRUPT, "колонка TEXT не является UTF-8"))
    }

    /// Значение INTEGER текущей строки (для NULL вернёт 0 — проверяйте `column_is_null`).
    pub(crate) fn column_i64(&self, column: usize) -> Result<i64, FfiError> {
        let column = Self::index(column)?;
        // SAFETY: statement жив; чтение INTEGER не владеет памятью.
        Ok(unsafe { sqlite3_column_int64(self.raw.as_ptr(), column) })
    }
}

impl Drop for Stmt<'_> {
    fn drop(&mut self) {
        // SAFETY: statement подготовлен в `prepare`/`Db::prepare` и ещё не финализирован;
        // код возврата не нужен — это освобождение ресурса, ошибок вызывающему не вернуть.
        unsafe {
            sqlite3_finalize(self.raw.as_ptr());
        }
    }
}
