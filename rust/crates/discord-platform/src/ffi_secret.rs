//! Тонкая обёртка над системным `libsecret 0.21` — единственное место крейта, где есть `unsafe`.
//!
//! Линкуются системные `libsecret-1`, `glib-2.0`, `gobject-2.0`, `gio-2.0`
//! (TZ W33a §1). Используются не-variadic вызовы `secret_service_*_sync` и
//! `secret_schema_newv`, чтобы не зависеть от соглашения о variadic-хвостах.
//!
//! Инварианты безопасности:
//!
//! - [`Service`], [`Schema`], [`Table`] и [`Value`] владеют ровно одним C-объектом и
//!   освобождают его ровно один раз в [`Drop`]; ни один тип не `Clone`, второго владельца нет.
//! - `GError*` читается (код + сообщение) и освобождается `g_error_free` на каждом пути,
//!   включая успешный: `take_error` вызывается до проверки результата.
//! - Значение из `secret_value_get` копируется `g_strndup` (освобождается `g_free`) до
//!   `secret_value_unref`, поэтому наружу не уходит указатель на чужую память.
//! - Секреты не попадают в сообщения об ошибках и не логируются: только код и текст glib.
//! - Каждый `unsafe`-блок снабжён `// SAFETY:`; наружу торчит безопасный API.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr::{self, NonNull};
use std::slice;

/// `SECRET_SERVICE_NONE`.
const SECRET_SERVICE_NONE: c_int = 0;
/// MIME-тип текстового секрета (`secret_value_new`).
const CONTENT_TYPE: &[u8] = b"text/plain\0";

/// Непрозрачный `SecretService` (раскладку знает только библиотека).
#[repr(C)]
pub(crate) struct SecretService {
    _private: [u8; 0],
}

/// Непрозрачный `SecretValue`.
#[repr(C)]
pub(crate) struct SecretValue {
    _private: [u8; 0],
}

/// Непрозрачный `SecretSchema`.
#[repr(C)]
pub(crate) struct SecretSchema {
    _private: [u8; 0],
}

/// Непрозрачный `GHashTable`.
#[repr(C)]
pub(crate) struct GHashTable {
    _private: [u8; 0],
}

/// `GError` из glib: `{ GQuark domain; gint code; gchar *message; }`.
#[repr(C)]
struct GError {
    domain: c_uint,
    code: c_int,
    message: *mut c_char,
}

type GHashFunc = Option<unsafe extern "C" fn(*const c_void) -> c_uint>;
type GEqualFunc = Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>;
type GDestroyNotify = Option<unsafe extern "C" fn(*mut c_void)>;

// SAFETY: имена и сигнатуры сверены с `/usr/include/libsecret-1/libsecret/*.h` (0.21.8) и
// `/usr/include/glib-2.0/glib/*.h`; линковка — системные библиотеки.
#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_free(memory: *mut c_void);
    fn g_strdup(text: *const c_char) -> *mut c_char;
    fn g_str_hash(value: *const c_void) -> c_uint;
    fn g_str_equal(left: *const c_void, right: *const c_void) -> c_int;
    fn g_hash_table_new_full(
        hash_func: GHashFunc,
        key_equal_func: GEqualFunc,
        key_destroy_func: GDestroyNotify,
        value_destroy_func: GDestroyNotify,
    ) -> *mut GHashTable;
    fn g_hash_table_insert(table: *mut GHashTable, key: *mut c_void, value: *mut c_void);
    fn g_hash_table_unref(table: *mut GHashTable);
    fn g_error_free(error: *mut GError);
}

// SAFETY: сигнатура сверена с `/usr/include/glib-2.0/gobject/gobject.h`.
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_object_unref(object: *mut c_void);
}

// SAFETY: сигнатура сверена с `/usr/include/glib-2.0/gio/gioerror.h`; линковка `gio-2.0`
// зафиксирована контрактом TZ §1. Прямой вызов не нужен: ошибки D-Bus приходят в `GError`
// от libsecret. Декларация оставлена как явная фиксация зависимости.
#[link(name = "gio-2.0")]
unsafe extern "C" {
    #[allow(dead_code)]
    fn g_dbus_error_is_remote_error(error: *mut GError) -> c_int;
}

// SAFETY: сигнатуры сверены с `/usr/include/libsecret-1/libsecret/secret-service.h`,
// `secret-value.h`, `secret-schema.h`, `secret-types.h` (0.21.8).
#[link(name = "secret-1")]
unsafe extern "C" {
    fn secret_service_get_sync(
        flags: c_int,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut SecretService;
    fn secret_service_store_sync(
        service: *mut SecretService,
        schema: *const SecretSchema,
        attributes: *mut GHashTable,
        collection: *const c_char,
        label: *const c_char,
        value: *mut SecretValue,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> c_int;
    fn secret_service_lookup_sync(
        service: *mut SecretService,
        schema: *const SecretSchema,
        attributes: *mut GHashTable,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut SecretValue;
    fn secret_service_clear_sync(
        service: *mut SecretService,
        schema: *const SecretSchema,
        attributes: *mut GHashTable,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> c_int;
    fn secret_value_new(
        secret: *const c_char,
        length: isize,
        content_type: *const c_char,
    ) -> *mut SecretValue;
    fn secret_value_get(value: *mut SecretValue, length: *mut usize) -> *const c_char;
    fn secret_value_unref(value: *mut c_void);
    fn secret_schema_newv(
        name: *const c_char,
        flags: c_int,
        attribute_names_and_types: *mut GHashTable,
    ) -> *mut SecretSchema;
    fn secret_schema_unref(schema: *mut SecretSchema);
}

/// Ошибка FFI-слоя: код `GError` (0 — ошибка без `GError`) и текст.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FfiError {
    pub(crate) code: i32,
    pub(crate) message: String,
}

impl FfiError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// Забирает `GError*` (если он есть): копирует код и сообщение, освобождает структуру.
fn take_error(error: *mut GError) -> Option<FfiError> {
    if error.is_null() {
        return None;
    }
    // SAFETY: `error` — ненулевой указатель на `GError`, заполненный glib/libsecret; поля
    // валидны до `g_error_free`, сообщение — NUL-терминированная строка (может быть NULL).
    let (code, message) = unsafe {
        let code = (*error).code;
        let message = if (*error).message.is_null() {
            String::new()
        } else {
            CStr::from_ptr((*error).message)
                .to_string_lossy()
                .into_owned()
        };
        (code, message)
    };
    // SAFETY: `error` получен от glib и ещё не освобождён; освобождаем ровно один раз.
    unsafe { g_error_free(error) };
    Some(FfiError::new(code, message))
}

/// Копия C-строки (`g_strdup`), владение — у вызывающего (дальше передаётся в `GHashTable`).
fn duplicate(text: &str) -> Result<NonNull<c_char>, FfiError> {
    let c_text = CString::new(text).map_err(|_| FfiError::new(0, "строка содержит NUL-байт"))?;
    // SAFETY: `c_text` — валидная NUL-терминированная строка, живёт до конца вызова;
    // g_strdup копирует её в память glib.
    NonNull::new(unsafe { g_strdup(c_text.as_ptr()) })
        .ok_or_else(|| FfiError::new(0, "g_strdup вернул NULL"))
}

/// Владелец `GHashTable`: ключи C-строки, значения — C-строки либо `GINT_TO_POINTER`.
pub(crate) struct Table {
    raw: NonNull<GHashTable>,
}

impl Table {
    fn new(key_destroy: GDestroyNotify, value_destroy: GDestroyNotify) -> Result<Self, FfiError> {
        // SAFETY: g_str_hash/g_str_equal — штатные функции сравнения C-строк; деструкторы
        // вызываются glib при удалении элементов и уничтожении таблицы.
        let raw = unsafe {
            g_hash_table_new_full(
                Some(g_str_hash),
                Some(g_str_equal),
                key_destroy,
                value_destroy,
            )
        };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| FfiError::new(0, "g_hash_table_new_full вернул NULL"))
    }

    /// Таблица атрибутов `{ name: value }` — обе строки освобождаются `g_free`.
    fn strings() -> Result<Self, FfiError> {
        Self::new(Some(g_free), Some(g_free))
    }

    /// Таблица имён схемы `{ name: GINT_TO_POINTER(type) }` — освобождаются только ключи.
    fn names() -> Result<Self, FfiError> {
        Self::new(Some(g_free), None)
    }

    fn put_string(&mut self, name: &str, value: &str) -> Result<(), FfiError> {
        let key = duplicate(name)?;
        let value = match duplicate(value) {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: `key` — свежая копия glib, ещё не переданная таблице;
                // освобождаем её, чтобы не течь на пути ошибки второго выделения.
                unsafe { g_free(key.as_ptr().cast::<c_void>()) };
                return Err(error);
            }
        };
        // SAFETY: таблица жива и принадлежит `self`; ключ и значение — свежие копии glib,
        // владение передаётся таблице (освободятся её деструкторами).
        unsafe {
            g_hash_table_insert(
                self.raw.as_ptr(),
                key.as_ptr().cast::<c_void>(),
                value.as_ptr().cast::<c_void>(),
            );
        }
        Ok(())
    }

    fn put_name(&mut self, name: &str) -> Result<(), FfiError> {
        let key = duplicate(name)?;
        // SAFETY: таблица жива; ключ — свежая копия glib, значение — GINT_TO_POINTER(0)
        // (`SECRET_SCHEMA_ATTRIBUTE_STRING`), освобождать его не нужно.
        unsafe {
            g_hash_table_insert(
                self.raw.as_ptr(),
                key.as_ptr().cast::<c_void>(),
                ptr::null_mut(),
            );
        }
        Ok(())
    }

    fn as_ptr(&self) -> *mut GHashTable {
        self.raw.as_ptr()
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        // SAFETY: таблица создана `g_hash_table_new_full` и ещё не освобождена; владение
        // единственное (`Table` не `Clone`), поэтому `g_hash_table_unref` — ровно один раз.
        unsafe { g_hash_table_unref(self.raw.as_ptr()) };
    }
}

/// Живой `SecretService*` (default-подключение). `!Send + !Sync` обеспечивает вызывающий слой.
pub(crate) struct Service {
    raw: NonNull<SecretService>,
}

impl Service {
    /// `secret_service_get_sync(SECRET_SERVICE_NONE)` — default-сервис сессионной шины.
    pub(crate) fn connect() -> Result<Self, FfiError> {
        let mut error: *mut GError = ptr::null_mut();
        // SAFETY: out-параметр `&mut error` валиден; cancellable = NULL; функция возвращает
        // новую ссылку на сервис или NULL с заполненным GError.
        let raw =
            unsafe { secret_service_get_sync(SECRET_SERVICE_NONE, ptr::null_mut(), &mut error) };
        if let Some(error) = take_error(error) {
            return Err(error);
        }
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| FfiError::new(0, "secret_service_get_sync вернул NULL без GError"))
    }

    /// `secret_service_store_sync` в default-коллекцию (`collection = NULL`).
    ///
    /// `attribute` — имя атрибута схемы (`key`), `value` — значение атрибута (сам ключ).
    pub(crate) fn store(
        &self,
        schema: &Schema,
        attribute: &str,
        value: &str,
        label: &str,
        secret: &str,
    ) -> Result<(), FfiError> {
        let mut attributes = Table::strings()?;
        attributes.put_string(attribute, value)?;
        let c_label =
            CString::new(label).map_err(|_| FfiError::new(0, "метка содержит NUL-байт"))?;
        let c_secret =
            CString::new(secret).map_err(|_| FfiError::new(0, "секрет содержит NUL-байт"))?;
        let length = isize::try_from(c_secret.as_bytes().len())
            .map_err(|_| FfiError::new(0, "секрет длиннее isize::MAX"))?;
        // SAFETY: строка и content_type — валидные NUL-терминированные строки; length —
        // точная длина без терминатора; функция копирует секрет в новый SecretValue.
        let value = unsafe {
            secret_value_new(
                c_secret.as_ptr(),
                length,
                CONTENT_TYPE.as_ptr().cast::<c_char>(),
            )
        };
        let Some(value) = NonNull::new(value) else {
            return Err(FfiError::new(0, "secret_value_new вернул NULL"));
        };
        let mut error: *mut GError = ptr::null_mut();
        // SAFETY: сервис и схема живы; таблица атрибутов — валидный GHashTable на время
        // вызова (libsecret читает её синхронно); collection = NULL — default-коллекция;
        // value принадлежит нам, метка валидна; out-параметр `&mut error` корректен.
        let stored = unsafe {
            secret_service_store_sync(
                self.raw.as_ptr(),
                schema.raw.as_ptr(),
                attributes.as_ptr(),
                ptr::null(),
                c_label.as_ptr(),
                value.as_ptr(),
                ptr::null_mut(),
                &mut error,
            )
        };
        // SAFETY: `value` создан `secret_value_new`, ещё не освобождён; владение единственное,
        // libsecret скопировал секрет при записи. Освобождаем ровно один раз.
        unsafe { secret_value_unref(value.as_ptr().cast::<c_void>()) };
        if let Some(error) = take_error(error) {
            return Err(error);
        }
        if stored == 0 {
            return Err(FfiError::new(0, "secret_service_store_sync вернул FALSE"));
        }
        Ok(())
    }

    /// `secret_service_lookup_sync`: `None` — записи нет; байты копируются из `SecretValue`.
    pub(crate) fn lookup(
        &self,
        schema: &Schema,
        attribute: &str,
        value: &str,
    ) -> Result<Option<Vec<u8>>, FfiError> {
        let mut attributes = Table::strings()?;
        attributes.put_string(attribute, value)?;
        let mut error: *mut GError = ptr::null_mut();
        // SAFETY: сервис/схема живы, таблица атрибутов валидна на время вызова;
        // cancellable = NULL; функция возвращает новую ссылку SecretValue или NULL.
        let value = unsafe {
            secret_service_lookup_sync(
                self.raw.as_ptr(),
                schema.raw.as_ptr(),
                attributes.as_ptr(),
                ptr::null_mut(),
                &mut error,
            )
        };
        if let Some(error) = take_error(error) {
            return Err(error);
        }
        let Some(value) = NonNull::new(value) else {
            return Ok(None);
        };
        let mut length: usize = 0;
        // SAFETY: `value` — живой SecretValue, принадлежащий нам; `&mut length` — корректный
        // out-параметр; возвращённый указатель валиден до `secret_value_unref` ниже.
        let data = unsafe { secret_value_get(value.as_ptr(), &mut length) };
        if data.is_null() {
            // SAFETY: `value` ещё не освобождён; владение единственное.
            unsafe { secret_value_unref(value.as_ptr().cast::<c_void>()) };
            return Err(FfiError::new(0, "secret_value_get вернул NULL"));
        }
        // SAFETY: `data` валиден ровно на `length` байт (контракт `secret_value_get`);
        // копируем байты как есть, не полагаясь на NUL-терминатор: значение может содержать
        // встроенный NUL (бинарные записи), и `g_strndup` обрезал бы его.
        let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), length) }.to_vec();
        // SAFETY: `value` ещё не освобождён; владение единственное.
        unsafe { secret_value_unref(value.as_ptr().cast::<c_void>()) };
        Ok(Some(bytes))
    }

    /// `secret_service_clear_sync`: `false` — записи не было (GError пуст).
    pub(crate) fn clear(
        &self,
        schema: &Schema,
        attribute: &str,
        value: &str,
    ) -> Result<bool, FfiError> {
        let mut attributes = Table::strings()?;
        attributes.put_string(attribute, value)?;
        let mut error: *mut GError = ptr::null_mut();
        // SAFETY: сервис/схема живы, таблица атрибутов валидна на время вызова;
        // cancellable = NULL; out-параметр `&mut error` корректен.
        let cleared = unsafe {
            secret_service_clear_sync(
                self.raw.as_ptr(),
                schema.raw.as_ptr(),
                attributes.as_ptr(),
                ptr::null_mut(),
                &mut error,
            )
        };
        if let Some(error) = take_error(error) {
            return Err(error);
        }
        Ok(cleared != 0)
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        // SAFETY: сервис получен `secret_service_get_sync` и ещё не освобождён; владение
        // единственное (`Service` не `Clone`), `g_object_unref` — ровно один раз.
        unsafe { g_object_unref(self.raw.as_ptr().cast::<c_void>()) };
    }
}

/// Живая `SecretSchema*` (`secret_schema_newv`); освобождается `secret_schema_unref`.
pub(crate) struct Schema {
    raw: NonNull<SecretSchema>,
    /// libsecret копирует имена атрибутов, но таблица держится живой до конца схемы, чтобы
    /// не зависеть от этого: освобождение ключей произойдёт после `secret_schema_unref`.
    _attributes: Table,
}

impl Schema {
    /// Схема `name` с одним строковым атрибутом `attribute` (не-variadic `secret_schema_newv`).
    pub(crate) fn new(name: &str, attribute: &str) -> Result<Self, FfiError> {
        let c_name = CString::new(name).map_err(|_| FfiError::new(0, "имя схемы содержит NUL"))?;
        let mut attributes = Table::names()?;
        attributes.put_name(attribute)?;
        // SAFETY: имя — валидная C-строка; таблица `{ name: GINT_TO_POINTER(STRING) }` жива
        // на время вызова и остаётся живой в `Schema`; функция копирует имена в новую схему.
        let raw = unsafe { secret_schema_newv(c_name.as_ptr(), 0, attributes.as_ptr()) };
        let raw =
            NonNull::new(raw).ok_or_else(|| FfiError::new(0, "secret_schema_newv вернул NULL"))?;
        Ok(Self {
            raw,
            _attributes: attributes,
        })
    }
}

impl Drop for Schema {
    fn drop(&mut self) {
        // SAFETY: схема создана `secret_schema_newv` и ещё не освобождена; владение
        // единственное, `secret_schema_unref` — ровно один раз.
        unsafe { secret_schema_unref(self.raw.as_ptr()) };
    }
}
