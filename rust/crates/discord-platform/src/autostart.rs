//! P6-03: XDG autostart-файл и модель бейджа Unity Launcher API.
//!
//! Оракул — XDG Desktop Entry 1.5: `[Desktop Entry]`, `Type=Application`, `Name=`,
//! `Exec=`, `Terminal=false`; экранирование аргументов Exec; выключение автозапуска
//! через `X-GNOME-Autostart-enabled=false` (ключ наблюдался в RE клиента).
//!
//! Файлы не создаются: [`desktop_entry`] возвращает текст, [`autostart_path`] — путь.
//! Бейдж — только модель свойств Unity Launcher API; D-Bus/wire-совместимость
//! не реализуются и не заявляются.

use std::path::{Path, PathBuf};

/// Запись автозапуска: имя, программа, аргументы и флаг «скрыть из автозапуска».
#[derive(Debug, PartialEq, Eq)]
pub struct AutostartEntry<'a> {
    pub name: &'a str,
    pub exec: &'a str,
    pub args: &'a [&'a str],
    pub hidden: bool,
}

/// Ошибки генерации `.desktop`-файла.
#[derive(Debug, PartialEq, Eq)]
pub enum AutostartError {
    /// Пустое `Name=`.
    EmptyName,
    /// Пустой `Exec=`.
    EmptyExec,
    /// `\n`, `\r` или `\0` в любом строковом поле.
    ControlCharacter,
}

/// Генерирует текст XDG autostart-файла (с завершающим `\n`).
///
/// `hidden=true` выключает автозапуск через `X-GNOME-Autostart-enabled=false`
/// (один механизм, зафиксирован контрактным тестом; `Hidden=true` не дублируется).
pub fn desktop_entry(entry: &AutostartEntry<'_>) -> Result<String, AutostartError> {
    if entry.name.is_empty() {
        return Err(AutostartError::EmptyName);
    }
    if entry.exec.is_empty() {
        return Err(AutostartError::EmptyExec);
    }
    if has_forbidden_control(entry.name) || has_forbidden_control(entry.exec) {
        return Err(AutostartError::ControlCharacter);
    }
    for arg in entry.args {
        if has_forbidden_control(arg) {
            return Err(AutostartError::ControlCharacter);
        }
    }

    let mut text = String::new();
    text.push_str("[Desktop Entry]\nType=Application\nName=");
    text.push_str(entry.name);
    text.push_str("\nExec=");
    text.push_str(&escape_exec_arg(entry.exec));
    for arg in entry.args {
        text.push(' ');
        text.push_str(&escape_exec_arg(arg));
    }
    text.push_str("\nX-GNOME-Autostart-enabled=");
    text.push_str(if entry.hidden { "false" } else { "true" });
    text.push_str("\nTerminal=false\n");
    Ok(text)
}

/// Путь XDG autostart-файла: `<config_home>/autostart/discord-lab.desktop`.
pub fn autostart_path(config_home: &Path) -> PathBuf {
    config_home.join("autostart").join("discord-lab.desktop")
}

/// Значение свойства Unity Launcher API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeValue {
    Count(u64),
    Visible(bool),
    Urgent(bool),
}

/// Состояние бейджа приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Badge {
    pub count: u64,
    pub urgent: bool,
    pub visible: bool,
}

/// Пары свойств бейджа в стабильном порядке: `count`, `count-visible`, `urgent`.
pub fn badge_pairs(badge: &Badge) -> Vec<(&'static str, BadgeValue)> {
    vec![
        ("count", BadgeValue::Count(badge.count)),
        ("count-visible", BadgeValue::Visible(badge.visible)),
        ("urgent", BadgeValue::Urgent(badge.urgent)),
    ]
}

/// Unity-идентификатор приложения для `.desktop`-файла.
pub fn app_uri(desktop_file: &str) -> String {
    format!("application://{desktop_file}")
}

fn has_forbidden_control(value: &str) -> bool {
    value.bytes().any(|byte| matches!(byte, b'\n' | b'\r' | 0))
}

fn escape_exec_arg(arg: &str) -> String {
    // XDG Desktop Entry 1.5, "The Exec key": аргумент с reserved-символом
    // заключается в двойные кавычки, иначе парсер лаунчера разобьёт его.
    let needs_quotes = arg.is_empty()
        || arg.chars().any(|c| {
            matches!(
                c,
                ' ' | '\t'
                    | '"'
                    | '\''
                    | '\\'
                    | '>'
                    | '<'
                    | '~'
                    | '|'
                    | '&'
                    | ';'
                    | '$'
                    | '*'
                    | '?'
                    | '#'
                    | '('
                    | ')'
                    | '`'
                    | '%'
            )
        });
    if !needs_quotes {
        return arg.to_owned();
    }

    let mut escaped = String::with_capacity(arg.len() + 2);
    escaped.push('"');
    for c in arg.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '$' => escaped.push_str("\\$"),
            '`' => escaped.push_str("\\`"),
            '%' => escaped.push_str("%%"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}
