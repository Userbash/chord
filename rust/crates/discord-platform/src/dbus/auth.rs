//! Аутентификация D-Bus: `AUTH EXTERNAL <hex>` по unix-сокету.
//!
//! UID берётся без `unsafe` и без внешних крейтов — из `/proc/self/status` (`Uid:`).
//! Оракул — спецификация D-Bus и reference-реализация (libdbus): клиент отправляет `\0`,
//! затем `AUTH EXTERNAL <hex>\r\n`, ожидает `OK <guid>`, отправляет `BEGIN\r\n`.
//! Идентичность EXTERNAL — **десятичная запись uid, закодированная в hex**: для uid 1000
//! аргумент — `31303030` («1000» в ASCII-hex), а не `3e8`.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::DbusError;

/// Таймаут рукопожатия аутентификации.
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
/// Предел длины строки ответа шины (защита от мусора в сокете).
const MAX_LINE: usize = 1024;

/// Строка `AUTH EXTERNAL` для uid: десятичная запись uid, закодированная в hex.
///
/// Пример: uid 1000 → `AUTH EXTERNAL 31303030` (hex от ASCII-строки «1000»).
pub fn external_auth_line(uid: u32) -> String {
    let decimal = uid.to_string();
    let mut hex = String::with_capacity(decimal.len() * 2);
    for byte in decimal.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("AUTH EXTERNAL {hex}")
}

/// Текущий effective uid процесса.
pub fn current_uid() -> Result<u32, DbusError> {
    let status = std::fs::read_to_string("/proc/self/status").map_err(DbusError::Io)?;
    parse_uid(&status).ok_or(DbusError::Auth("no Uid line in /proc/self/status"))
}

/// Разбирает первую строку `Uid:` из `/proc/self/status` (effective uid — второй столбец).
pub fn parse_uid(status: &str) -> Option<u32> {
    let line = status.lines().find(|line| line.starts_with("Uid:"))?;
    let mut fields = line["Uid:".len()..].split_whitespace();
    let real = fields.next()?.parse().ok()?;
    let effective = fields.next().and_then(|value| value.parse().ok());
    Some(effective.unwrap_or(real))
}

/// Проходит рукопожатие на уже подключённом сокете.
///
/// Сокет должен быть в блокирующем режиме (таймаут выставляет вызывающая сторона).
pub fn authenticate(stream: &UnixStream) -> Result<(), DbusError> {
    let uid = current_uid()?;
    let mut request = Vec::with_capacity(32);
    request.push(0u8); // credentials byte: без него шина не читает команды
    request.extend_from_slice(external_auth_line(uid).as_bytes());
    request.extend_from_slice(b"\r\n");

    let mut writer = stream;
    writer.write_all(&request).map_err(DbusError::Io)?;

    let reply = read_line(stream)?;
    if reply.starts_with("OK ") {
        writer.write_all(b"BEGIN\r\n").map_err(DbusError::Io)?;
        return Ok(());
    }
    if reply.starts_with("REJECTED") {
        return Err(DbusError::Auth("EXTERNAL auth rejected"));
    }
    if reply.starts_with("ERROR") {
        return Err(DbusError::Auth("auth error"));
    }
    Err(DbusError::Protocol("unexpected auth reply"))
}

fn read_line(stream: &UnixStream) -> Result<String, DbusError> {
    let mut reader = stream;
    let mut bytes = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => return Err(DbusError::Protocol("auth connection closed")),
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                if bytes.len() >= MAX_LINE {
                    return Err(DbusError::Protocol("auth reply too long"));
                }
                bytes.push(byte[0]);
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Err(DbusError::Timeout);
            }
            Err(error) => return Err(DbusError::Io(error)),
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes).map_err(|_| DbusError::Protocol("auth reply is not utf-8"))
}
