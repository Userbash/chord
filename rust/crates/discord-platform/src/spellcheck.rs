//! P6-07: проверка орфографии через системный `libhunspell` (headless).
//!
//! Безопасный API поверх `ffi_hunspell` (единственное место с `unsafe`):
//! [`SpellChecker`] загружает пару `.aff`/`.dic` и делегирует `check`/`suggest`/`add`/`remove`
//! в hunspell. [`tokenize`] — чистый Rust-токенайзер для предварительного разбиения текста
//! сообщения на слова с байтовыми смещениями.
//!
//! Особенности контракта (TZ W32a §1):
//!
//! - `SpellChecker` — `!Send + !Sync`: handle hunspell не потокобезопасен, один экземпляр —
//!   один поток. `Drop` вызывает `hunspell_destroy` ровно один раз.
//! - `new` отклоняет путь с NUL ([`SpellError::Nul`]) и недоступный/пустой словарь до вызова
//!   C: `hunspell_create` 1.7.5 на отсутствующих файлах возвращает не-NULL handle, поэтому
//!   контрактное «файлы/битый словарь → ошибка» обеспечивается проверкой в безопасном слое.
//!   NULL от `hunspell_create` тоже даёт ошибку ([`SpellError::Create`]).
//! - `suggest` возвращает не более `MAX_SUGGESTIONS` вариантов в порядке hunspell.
//! - `tokenize`: слово — Unicode-буквы, внутри допустимы `'`/`-` (не в начале/конце); цифры
//!   и `_` разрывают слово. Пропускаются URL-токены (`://`), e-mail-токены (`@`), содержимое
//!   обратных кавычек, упоминания/каналы `<@…>`/`<#…>` и `:emoji:`. Смещения — байтовые.
//!
//! Ограничения: библиотека и словарь — системные/синтетические (фикстуры), CLI-оракула нет;
//! кодировка словаря не конвертируется (ожидается UTF-8), GUI-интеграция не входит в карточку.

use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use crate::ffi_hunspell::Handle;

/// Верхняя граница числа вариантов `suggest` (TZ W32a §1).
const MAX_SUGGESTIONS: usize = 10;

/// Слово текста с байтовыми смещениями в исходной строке.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Текст слова (совпадает с `&source[start..end]`).
    pub text: String,
    /// Начало в байтах (включительно).
    pub start: usize,
    /// Конец в байтах (исключительно), `start < end`.
    pub end: usize,
}

/// Ошибки загрузки словаря. Ни один путь не паникует: всё возвращается значением.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellError {
    /// Путь содержит NUL-байт и не представим C-строкой.
    Nul { path: String },
    /// Путь не является UTF-8 (hunspell принимает пути как C-строки).
    InvalidPath { path: String },
    /// Файл словаря отсутствует или недоступен.
    Missing { path: String, reason: String },
    /// Файл есть, но структурно непригоден (пустой, битый заголовок `.dic`).
    InvalidDictionary { path: String, reason: String },
    /// `hunspell_create` вернул NULL.
    Create {
        aff: String,
        dic: String,
        reason: String,
    },
}

impl fmt::Display for SpellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nul { path } => write!(formatter, "путь содержит NUL-байт: {path}"),
            Self::InvalidPath { path } => write!(formatter, "путь не является UTF-8: {path}"),
            Self::Missing { path, reason } => {
                write!(formatter, "файл словаря недоступен: {path} ({reason})")
            }
            Self::InvalidDictionary { path, reason } => {
                write!(formatter, "битый словарь: {path} ({reason})")
            }
            Self::Create { aff, dic, reason } => {
                write!(formatter, "hunspell_create({aff}, {dic}): {reason}")
            }
        }
    }
}

impl std::error::Error for SpellError {}

/// Проверка орфографии поверх hunspell. Однопоточный: `!Send + !Sync`.
pub struct SpellChecker {
    handle: Handle,
    aff: PathBuf,
    dic: PathBuf,
    /// Явная фиксация `!Send + !Sync` независимо от состава полей.
    _not_send_sync: PhantomData<*mut ()>,
}

impl fmt::Debug for SpellChecker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpellChecker")
            .field("aff", &self.aff)
            .field("dic", &self.dic)
            .finish_non_exhaustive()
    }
}

impl SpellChecker {
    /// Загружает словарь `.aff` + `.dic`. Ошибки — значением, без паники.
    pub fn new(aff: &Path, dic: &Path) -> Result<Self, SpellError> {
        // Порядок важен: NUL проверяется до существования файла (h12).
        let aff_text = path_text(aff)?;
        let dic_text = path_text(dic)?;
        validate_aff(aff)?;
        validate_dic(dic)?;
        let handle = Handle::create(aff_text, dic_text).map_err(|error| SpellError::Create {
            aff: aff.display().to_string(),
            dic: dic.display().to_string(),
            reason: error.to_string(),
        })?;
        Ok(Self {
            handle,
            aff: aff.to_path_buf(),
            dic: dic.to_path_buf(),
            _not_send_sync: PhantomData,
        })
    }

    /// Слово есть в словаре (с учётом рантайм-добавлений/удалений).
    pub fn check(&mut self, word: &str) -> bool {
        self.handle.check(word)
    }

    /// Варианты исправления, не более `MAX_SUGGESTIONS`, порядок hunspell.
    pub fn suggest(&mut self, word: &str) -> Vec<String> {
        self.handle.suggest(word, MAX_SUGGESTIONS)
    }

    /// Добавляет слово в рантайм-словарь; `true` — hunspell сообщил успех.
    pub fn add(&mut self, word: &str) -> bool {
        self.handle.add(word)
    }

    /// Удаляет слово из рантайм-словаря; `true` — hunspell сообщил успех.
    pub fn remove(&mut self, word: &str) -> bool {
        self.handle.remove(word)
    }
}

/// UTF-8 путь без NUL. NUL — отдельная ошибка, а не молчаливое обрезание.
fn path_text(path: &Path) -> Result<&str, SpellError> {
    let text = path.to_str().ok_or_else(|| SpellError::InvalidPath {
        path: path.display().to_string(),
    })?;
    if text.contains('\0') {
        return Err(SpellError::Nul {
            path: path.display().to_string(),
        });
    }
    Ok(text)
}

/// `.aff` должен существовать, быть обычным непустым файлом.
fn validate_aff(path: &Path) -> Result<(), SpellError> {
    let metadata = std::fs::metadata(path).map_err(|error| missing(path, &error))?;
    if !metadata.is_file() {
        return Err(SpellError::Missing {
            path: path.display().to_string(),
            reason: "не обычный файл".to_owned(),
        });
    }
    if metadata.len() == 0 {
        return Err(SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: "aff-файл пуст".to_owned(),
        });
    }
    let text = std::fs::read_to_string(path).map_err(|error| SpellError::InvalidDictionary {
        path: path.display().to_string(),
        reason: format!("aff не читается как UTF-8: {error}"),
    })?;
    let has_set = text.lines().any(|line| {
        let line = line.trim_start_matches('\u{feff}').trim_start();
        line.starts_with("SET ")
    });
    if !has_set {
        return Err(SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: "aff без директивы SET".to_owned(),
        });
    }
    Ok(())
}

/// `.dic` должен существовать, быть обычным файлом с положительным числом в заголовке.
fn validate_dic(path: &Path) -> Result<(), SpellError> {
    let metadata = std::fs::metadata(path).map_err(|error| missing(path, &error))?;
    if !metadata.is_file() {
        return Err(SpellError::Missing {
            path: path.display().to_string(),
            reason: "не обычный файл".to_owned(),
        });
    }
    if metadata.len() == 0 {
        return Err(SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: "dic-файл пуст".to_owned(),
        });
    }
    let file = File::open(path).map_err(|error| missing(path, &error))?;
    let mut header = String::new();
    let read = BufReader::new(file)
        .read_line(&mut header)
        .map_err(|error| SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: format!("заголовок не читается как UTF-8/ASCII: {error}"),
        })?;
    if read == 0 {
        return Err(SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: "dic-файл пуст".to_owned(),
        });
    }
    let header = header.trim_start_matches('\u{feff}').trim();
    let count = header
        .parse::<usize>()
        .map_err(|_| SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: format!("заголовок dic не число: {header:?}"),
        })?;
    if count == 0 {
        return Err(SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: "заголовок dic: 0 слов".to_owned(),
        });
    }
    let bytes = std::fs::read(path).map_err(|error| missing(path, &error))?;
    let text = String::from_utf8_lossy(&bytes);
    let entries: Vec<&str> = text
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if entries.len() != count {
        return Err(SpellError::InvalidDictionary {
            path: path.display().to_string(),
            reason: format!("dic: заголовок {count}, записей {}", entries.len()),
        });
    }
    for entry in entries {
        let word = entry.split(['/', ' ', '\t']).next().unwrap_or_default();
        if word.is_empty()
            || !word
                .chars()
                .all(|ch| ch.is_alphabetic() || ch == '\'' || ch == '-')
        {
            return Err(SpellError::InvalidDictionary {
                path: path.display().to_string(),
                reason: format!("dic: недопустимая запись {entry:?}"),
            });
        }
    }
    Ok(())
}

fn missing(path: &Path, error: &std::io::Error) -> SpellError {
    SpellError::Missing {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}

/// Разбивает текст на слова с байтовыми смещениями. Не паникует на любом Unicode.
///
/// URL/e-mail определяются по непустому токену между пробелами: если он содержит `://`
/// или `@`, токен пропускается целиком. Обратные кавычки, `<@…>`/`<#…>` и `:emoji:`
/// пропускаются посимвольно, поэтому слова рядом с ними сохраняются.
pub fn tokenize(text: &str) -> Vec<Token> {
    let positions: Vec<(usize, char)> = text.char_indices().collect();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < positions.len() {
        let (start, character) = positions[index];

        if character.is_whitespace() {
            index += 1;
            continue;
        }

        if character == '`' {
            index += 1;
            while index < positions.len() && positions[index].1 != '`' {
                index += 1;
            }
            // Закрывающая кавычка (если есть) тоже пропускается.
            if index < positions.len() {
                index += 1;
            }
            continue;
        }

        if character == '<' && is_mention_start(&positions, index) {
            index += 2;
            while index < positions.len()
                && positions[index].1 != '>'
                && !positions[index].1.is_whitespace()
            {
                index += 1;
            }
            if index < positions.len() && positions[index].1 == '>' {
                index += 1;
            }
            continue;
        }

        if character == ':'
            && let Some(after) = emoji_end(&positions, index)
        {
            index = after;
            continue;
        }

        if !character.is_alphabetic() {
            index += 1;
            continue;
        }

        let chunk_end = chunk_end(&positions, index);
        let chunk_end_byte = byte_at(&positions, chunk_end, text);
        let chunk = &text[start..chunk_end_byte];
        if chunk.contains("://") || chunk.contains('@') {
            index = chunk_end;
            continue;
        }

        let mut cursor = index + 1;
        while cursor < positions.len() {
            let current = positions[cursor].1;
            let next_is_letter = positions
                .get(cursor + 1)
                .is_some_and(|(_, next)| next.is_alphabetic());
            let word_char =
                current.is_alphabetic() || (current == '\'' || current == '-') && next_is_letter;
            if word_char {
                cursor += 1;
            } else {
                break;
            }
        }
        let end = byte_at(&positions, cursor, text);
        tokens.push(Token {
            text: text[start..end].to_owned(),
            start,
            end,
        });
        index = cursor;
    }

    tokens
}

/// `<` + `@`/`#` — начало упоминания или канала.
fn is_mention_start(positions: &[(usize, char)], index: usize) -> bool {
    positions
        .get(index + 1)
        .is_some_and(|(_, next)| *next == '@' || *next == '#')
}

/// Конец whitespace-токена (индекс в `positions`, не байтовое смещение).
fn chunk_end(positions: &[(usize, char)], start: usize) -> usize {
    let mut end = start;
    while end < positions.len() && !positions[end].1.is_whitespace() {
        end += 1;
    }
    end
}

/// Байтовое смещение для индекса `positions`; за концом — длина текста.
fn byte_at(positions: &[(usize, char)], index: usize, text: &str) -> usize {
    positions.get(index).map_or(text.len(), |(byte, _)| *byte)
}

/// `:shortcode:` → индекс сразу после закрывающего `:`; `None`, если это не emoji.
fn emoji_end(positions: &[(usize, char)], start: usize) -> Option<usize> {
    let mut index = start + 1;
    let mut length = 0;
    while index < positions.len() {
        let character = positions[index].1;
        if character == ':' {
            return (length >= 2).then_some(index + 1);
        }
        let valid = character.is_ascii_alphanumeric()
            || character == '_'
            || character == '+'
            || character == '-';
        if !valid {
            return None;
        }
        length += 1;
        index += 1;
    }
    None
}
