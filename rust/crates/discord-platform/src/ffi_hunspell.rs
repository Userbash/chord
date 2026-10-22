//! Тонкая обёртка над системным `libhunspell` — единственное место крейта, где есть `unsafe`.
//!
//! Линкуется системная библиотека `libhunspell-1.7` (`#[link(name = "hunspell-1.7")]`,
//! 1.7.5). C-API hunspell экспортирует символы с заглавной буквы (`Hunspell_create`,
//! `Hunspell_spell`, …), поэтому Rust-имена модуля — по контракту TZ W32a §1
//! (`hunspell_create`, …), а `#[link_name]` указывает реальный символ.
//!
//! Инварианты безопасности:
//!
//! - [`Handle`] владеет живым `Hunhandle*`; создаётся только [`Handle::create`] и
//!   уничтожается ровно один раз в [`Drop`]. `Handle` не `Clone`, поэтому второго владельца нет.
//! - Все строки, уходящие в C, — [`CString`]; внутренний NUL не обрезается, а даёт ошибку
//!   или `false`/пустой список.
//! - Список вариантов `Hunspell_suggest` освобождается `Hunspell_free_list` целиком
//!   (с исходным числом вариантов) после копирования строк в Rust.
//! - Каждый `unsafe`-блок снабжён `// SAFETY:`; наружу торчит безопасный API.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr::{self, NonNull};
use std::slice;

/// Непрозрачный тип `Hunhandle` (раскладку знает только библиотека).
#[repr(C)]
pub(crate) struct Hunhandle {
    _private: [u8; 0],
}

// SAFETY: имена и сигнатуры совпадают с `/usr/include/hunspell/hunspell.h` 1.7.5;
// линковка — системная `libhunspell-1.7`.
#[link(name = "hunspell-1.7")]
unsafe extern "C" {
    #[link_name = "Hunspell_create"]
    fn hunspell_create(affpath: *const c_char, dpath: *const c_char) -> *mut Hunhandle;
    #[link_name = "Hunspell_destroy"]
    fn hunspell_destroy(handle: *mut Hunhandle);
    #[link_name = "Hunspell_spell"]
    fn hunspell_spell(handle: *mut Hunhandle, word: *const c_char) -> c_int;
    #[link_name = "Hunspell_suggest"]
    fn hunspell_suggest(
        handle: *mut Hunhandle,
        list: *mut *mut *mut c_char,
        word: *const c_char,
    ) -> c_int;
    #[link_name = "Hunspell_add"]
    fn hunspell_add(handle: *mut Hunhandle, word: *const c_char) -> c_int;
    #[link_name = "Hunspell_remove"]
    fn hunspell_remove(handle: *mut Hunhandle, word: *const c_char) -> c_int;
    #[link_name = "Hunspell_free_list"]
    fn hunspell_free_list(handle: *mut Hunhandle, list: *mut *mut *mut c_char, count: c_int);
}

/// Ошибка уровня FFI: только создание handle (проверки путей — в безопасном слое).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FfiError {
    pub(crate) message: String,
}

impl FfiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// Живой handle словаря. Уничтожается в [`Drop`], ровно один раз.
pub(crate) struct Handle {
    raw: NonNull<Hunhandle>,
}

impl Handle {
    /// `hunspell_create(aff, dic)`; NULL (в том числе после отказа библиотеки) — ошибка.
    pub(crate) fn create(aff: &str, dic: &str) -> Result<Self, FfiError> {
        let c_aff =
            CString::new(aff).map_err(|_| FfiError::new("путь aff-файла содержит NUL-байт"))?;
        let c_dic =
            CString::new(dic).map_err(|_| FfiError::new("путь dic-файла содержит NUL-байт"))?;
        // SAFETY: обе строки — валидные NUL-терминированные C-строки и живут до конца вызова;
        // функция не сохраняет указатели после возврата (копирует пути при загрузке словаря).
        let raw = unsafe { hunspell_create(c_aff.as_ptr(), c_dic.as_ptr()) };
        // SAFETY-инвариант типа: `Handle` создаётся только здесь и только из non-null.
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| FfiError::new("hunspell_create вернул NULL (словарь не загрузился)"))
    }

    /// `hunspell_spell` != 0; слово с NUL не может быть C-строкой → `false`.
    pub(crate) fn check(&mut self, word: &str) -> bool {
        let Some(c_word) = c_string(word) else {
            return false;
        };
        // SAFETY: `raw` — живой handle, принадлежащий `self`; `c_word` валиден на время вызова;
        // функция только читает слово и состояние словаря.
        unsafe { hunspell_spell(self.raw.as_ptr(), c_word.as_ptr()) != 0 }
    }

    /// Варианты исправления (не больше `limit`), в порядке hunspell. Список освобождается.
    pub(crate) fn suggest(&mut self, word: &str, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let Some(c_word) = c_string(word) else {
            return Vec::new();
        };
        let mut list: *mut *mut c_char = ptr::null_mut();
        // SAFETY: `raw` — живой handle; `&mut list` — корректный out-параметр (массив строк);
        // `c_word` валиден на время вызова. Возвращается число строк и владение массивом.
        let count = unsafe { hunspell_suggest(self.raw.as_ptr(), &mut list, c_word.as_ptr()) };
        if count <= 0 || list.is_null() {
            return Vec::new();
        }
        let count_usize = usize::try_from(count).unwrap_or(0);
        let mut suggestions = Vec::new();
        // SAFETY: hunspell вернул массив из `count` валидных указателей на C-строки; массив
        // принадлежит нам до `hunspell_free_list` и не меняется во время чтения.
        let entries = unsafe { slice::from_raw_parts(list, count_usize) };
        for entry in entries.iter().take(limit) {
            if entry.is_null() {
                continue;
            }
            // SAFETY: каждый элемент массива — NUL-терминированная строка, выделенная hunspell;
            // копируем её в `String` до освобождения списка.
            let text = unsafe { CStr::from_ptr(*entry) }
                .to_string_lossy()
                .into_owned();
            suggestions.push(text);
        }
        // SAFETY: `list` — тот же массив, что вернул hunspell_suggest, и ещё не освобождён;
        // `count` — исходное число элементов (а не длина усечённого `suggestions`).
        unsafe { hunspell_free_list(self.raw.as_ptr(), &mut list, count) };
        suggestions
    }

    /// `hunspell_add`: 0 — успех.
    pub(crate) fn add(&mut self, word: &str) -> bool {
        let Some(c_word) = c_string(word) else {
            return false;
        };
        // SAFETY: `raw` — живой handle; `c_word` валиден на время вызова; функция копирует
        // слово в рантайм-словарь.
        unsafe { hunspell_add(self.raw.as_ptr(), c_word.as_ptr()) == 0 }
    }

    /// `hunspell_remove`: 0 — успех.
    pub(crate) fn remove(&mut self, word: &str) -> bool {
        let Some(c_word) = c_string(word) else {
            return false;
        };
        // SAFETY: `raw` — живой handle; `c_word` валиден на время вызова; функция помечает
        // слово удалённым в рантайм-словаре.
        unsafe { hunspell_remove(self.raw.as_ptr(), c_word.as_ptr()) == 0 }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `raw` создан `hunspell_create` и ещё не уничтожен; `Handle` не `Clone`,
        // владение единственное, поэтому `hunspell_destroy` вызывается ровно один раз.
        unsafe { hunspell_destroy(self.raw.as_ptr()) };
    }
}

/// C-строка без внутренних NUL; `None` — слово не представимо в C (не обрезаем молча).
fn c_string(text: &str) -> Option<CString> {
    CString::new(text).ok()
}
