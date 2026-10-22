//! SCM_RIGHTS: приём и отправка unix-fd через `sendmsg`/`recvmsg`.
//!
//! D-Bus передаёт fd не в теле сообщения, а ancillary-данными (`SCM_RIGHTS`), приложенными
//! к байтам сообщения; в теле (`h`) лежит `u32`-индекс в массив fd этого сообщения.
//! Вся работа с `libc` (и единственный `unsafe` крейта вне существующих ffi-модулей)
//! изолирована здесь: [`super::message`] и [`super::Connection`] видят только безопасные
//! функции и [`std::os::fd::RawFd`].
//!
//! Ограничения (осознанные):
//! - Linux/Unix: `SCM_RIGHTS`, `MSG_CTRUNC`, `sendmsg`/`recvmsg`;
//! - принимается не больше [`MAX_FDS_PER_MESSAGE`] fd на сообщение (столько же по умолчанию
//!   допускает `dbus-daemon`); лишние → `MSG_CTRUNC` → [`super::DbusError::Protocol`],
//!   уже доставленные fd закрываются;
//! - отправка — не больше `SCM_MAX_FD` (253) fd одним сообщением;
//! - fd не наследуются через `fork`/`exec`: владение остаётся у процесса-получателя.

#![allow(unsafe_code)]

use std::io::ErrorKind;
use std::mem::size_of;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;

use super::DbusError;

/// Максимум fd, принимаемых в одном сообщении.
///
/// Совпадает с дефолтным лимитом `dbus-daemon` (`max_message_unix_fds`); сообщение с
/// большим числом fd приходит с `MSG_CTRUNC` и отвергается как `Protocol`.
pub const MAX_FDS_PER_MESSAGE: usize = 16;

/// Предел ядра Linux на число fd в одном `sendmsg` (`SCM_MAX_FD`).
const SCM_MAX_FD: usize = 253;

const INT_BYTES: usize = size_of::<libc::c_int>();

/// Буфер ancillary для приёма: `cmsghdr` + ровно [`MAX_FDS_PER_MESSAGE`] fd.
///
/// Ровно по лимиту (а не с запасом), чтобы сообщение с бо́льшим числом fd гарантированно
/// приходило с `MSG_CTRUNC` и отвергалось, а не «влезало наполовину».
const CONTROL_BYTES: usize = 16 + MAX_FDS_PER_MESSAGE * INT_BYTES;
/// Буфер ancillary для отправки: до [`SCM_MAX_FD`] fd.
const SEND_CONTROL_BYTES: usize = 16 + SCM_MAX_FD * INT_BYTES + 8;

/// Выровненный под `cmsghdr` буфер: `cmsghdr` требует выравнивания `size_t`.
#[repr(align(8))]
struct ControlBuffer([u8; CONTROL_BYTES]);

#[repr(align(8))]
struct SendControlBuffer([u8; SEND_CONTROL_BYTES]);

/// Закрывает fd ровно один раз; отрицательные значения игнорируются.
///
/// Повторный `close` уже закрытого fd — ошибка `EBADF`, которую здесь некому обработать
/// (владелец вызывает функцию ровно один раз); поэтому значение `fd < 0` не трогается.
pub fn close_fd(fd: RawFd) {
    if fd < 0 {
        return;
    }
    // SAFETY: `close(2)` принимает любой целочисленный fd; недействительный fd даёт EBADF,
    // памяти не касается. Владение fd передано вызывающему (см. контракт `UnixFd`).
    unsafe {
        libc::close(fd);
    }
}

/// Закрывает все fd из среза (порядок сохраняется, повторы закрываются как отдельные).
pub fn close_all(fds: &[RawFd]) {
    for fd in fds {
        close_fd(*fd);
    }
}

/// Отправляет `bytes` одним сообщением, приложив `fds` через `SCM_RIGHTS`.
///
/// `bytes` обязаны быть непустыми: ancillary-данные прикрепляются к первому байту.
/// Частичная отправка повторяется; fd прикладываются только к первому `sendmsg`
/// (иначе ядро продублировало бы их). Ошибка отправки не закрывает `fds` — ими
/// по-прежнему владеет вызывающий.
pub fn send_with_fds(stream: &UnixStream, bytes: &[u8], fds: &[RawFd]) -> Result<(), DbusError> {
    if fds.len() > SCM_MAX_FD {
        return Err(DbusError::Protocol("too many unix fds to send"));
    }
    if bytes.is_empty() {
        return if fds.is_empty() {
            Ok(())
        } else {
            Err(DbusError::Protocol("empty message with unix fds"))
        };
    }

    let mut control = SendControlBuffer([0u8; SEND_CONTROL_BYTES]);
    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr() as *mut libc::c_void,
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = {
        // SAFETY: `msghdr` — POD; нулевая инициализация корректна, поля заполняются ниже.
        unsafe { std::mem::zeroed() }
    };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;

    if !fds.is_empty() {
        let payload = fds.len() * INT_BYTES;
        let payload_u32 =
            u32::try_from(payload).map_err(|_| DbusError::Protocol("unix fd payload too large"))?;
        // SAFETY: буфер выровнен и вмещает `CMSG_SPACE(payload)`; cmsg_len/CMSG_DATA
        // вычисляются для этого же буфера, копирование не выходит за payload.
        let control_len = unsafe {
            let header = control.0.as_mut_ptr() as *mut libc::cmsghdr;
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(payload_u32) as usize;
            std::ptr::copy_nonoverlapping(
                fds.as_ptr() as *const u8,
                libc::CMSG_DATA(header),
                payload,
            );
            // SAFETY: `CMSG_SPACE` — чистая функция от длины; значение влезает в буфер.
            libc::CMSG_SPACE(payload_u32) as usize
        };
        message.msg_control = control.0.as_mut_ptr() as *mut libc::c_void;
        message.msg_controllen = control_len;
    }

    let mut sent_total = 0usize;
    while sent_total < bytes.len() {
        // SAFETY: `message` указывает на живой `iov` и (опционально) control-буфер;
        // сокет валиден; MSG_NOSIGNAL исключает SIGPIPE.
        let sent = unsafe { libc::sendmsg(stream.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
        if sent < 0 {
            let error = std::io::Error::last_os_error();
            match error.kind() {
                ErrorKind::Interrupted => continue,
                ErrorKind::WouldBlock | ErrorKind::TimedOut => return Err(DbusError::Timeout),
                _ => return Err(DbusError::Io(error)),
            }
        }
        if sent == 0 {
            return Err(DbusError::Protocol("short send"));
        }
        let sent = sent as usize;
        sent_total += sent;
        // fd уже отправлены с первым успешным sendmsg: очищаем ancillary.
        message.msg_control = std::ptr::null_mut();
        message.msg_controllen = 0;
        // SAFETY: `sent <= iov.iov_len` (sendmsg не отправляет больше запрошенного),
        // сдвиг остаётся внутри исходного среза.
        iov.iov_base = unsafe { (iov.iov_base as *mut u8).add(sent) } as *mut libc::c_void;
        iov.iov_len -= sent;
    }
    Ok(())
}

/// Читает ровно `buffer.len()` байт, собирая пришедшие `SCM_RIGHTS` fd в `fds`.
///
/// При `MSG_CTRUNC`, неожиданном/битом ancillary или обрыве соединения возвращает
/// `DbusError::Protocol`/`Timeout`/`Io`, **закрывая** уже собранные fd и очищая `fds`:
/// наружу не отдаётся ни одного «осиротевшего» дескриптора.
pub fn recv_exact_with_fds(
    stream: &UnixStream,
    buffer: &mut [u8],
    fds: &mut Vec<RawFd>,
) -> Result<(), DbusError> {
    let mut filled = 0usize;
    while filled < buffer.len() {
        let mut control = ControlBuffer([0u8; CONTROL_BYTES]);
        let mut iov = libc::iovec {
            iov_base: buffer[filled..].as_mut_ptr() as *mut libc::c_void,
            iov_len: buffer.len() - filled,
        };
        let mut message: libc::msghdr = {
            // SAFETY: `msghdr` — POD; нулевая инициализация корректна, поля заполняются ниже.
            unsafe { std::mem::zeroed() }
        };
        message.msg_iov = &mut iov;
        message.msg_iovlen = 1;
        message.msg_control = control.0.as_mut_ptr() as *mut libc::c_void;
        message.msg_controllen = CONTROL_BYTES as _;

        // SAFETY: `message` описывает живой буфер `buffer[filled..]` и control-буфер
        // ровно заявленной длины; сокет валиден.
        let received = unsafe { libc::recvmsg(stream.as_raw_fd(), &mut message, 0) };
        if received < 0 {
            let error = std::io::Error::last_os_error();
            match error.kind() {
                ErrorKind::Interrupted => continue,
                ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                    return Err(discard(fds, DbusError::Timeout));
                }
                _ => return Err(discard(fds, DbusError::Io(error))),
            }
        }
        if received == 0 {
            return Err(discard(fds, DbusError::Protocol("truncated message")));
        }
        // Ancillary мог быть обрезан (`MSG_CTRUNC`), но доставленные cmsg всё равно
        // разбираем: их fd обязаны быть закрыты, иначе утечка.
        // SAFETY: `msg_control`/`msg_controllen` заполнены ядром; обход cmsg ограничен
        // этими полями (CMSG_FIRSTHDR/CMSG_NXTHDR).
        unsafe {
            let mut header = libc::CMSG_FIRSTHDR(&message);
            while !header.is_null() {
                if (*header).cmsg_level != libc::SOL_SOCKET
                    || (*header).cmsg_type != libc::SCM_RIGHTS
                {
                    return Err(discard(
                        fds,
                        DbusError::Protocol("unexpected ancillary data"),
                    ));
                }
                let length = (*header).cmsg_len as usize;
                let header_len = libc::CMSG_LEN(0) as usize;
                if length < header_len || !(length - header_len).is_multiple_of(INT_BYTES) {
                    return Err(discard(
                        fds,
                        DbusError::Protocol("malformed ancillary data"),
                    ));
                }
                let count = (length - header_len) / INT_BYTES;
                let control_start = message.msg_control as usize;
                let control_end = control_start.saturating_add(message.msg_controllen);
                let header_end = (header as usize).saturating_add(length);
                let data = libc::CMSG_DATA(header);
                let data_end = (data as usize).saturating_add(count * INT_BYTES);
                if header_end > control_end || data_end > control_end {
                    return Err(discard(
                        fds,
                        DbusError::Protocol("ancillary data exceeds control buffer"),
                    ));
                }
                for index in 0..count {
                    let mut raw: libc::c_int = 0;
                    std::ptr::copy_nonoverlapping(
                        data.add(index * INT_BYTES),
                        &mut raw as *mut libc::c_int as *mut u8,
                        INT_BYTES,
                    );
                    if raw < 0 {
                        return Err(discard(fds, DbusError::Protocol("invalid unix fd")));
                    }
                    fds.push(raw);
                }
                header = libc::CMSG_NXTHDR(&message, header);
            }
        }
        if message.msg_flags & libc::MSG_CTRUNC != 0 {
            return Err(discard(fds, DbusError::Protocol("control data truncated")));
        }
        if fds.len() > MAX_FDS_PER_MESSAGE {
            return Err(discard(
                fds,
                DbusError::Protocol("too many unix fds in message"),
            ));
        }

        filled += received as usize;
    }
    Ok(())
}

/// Закрывает собранные fd и очищает вектор, чтобы вызывающий не увидел «висящих» fd.
fn discard(fds: &mut Vec<RawFd>, error: DbusError) -> DbusError {
    close_all(fds);
    fds.clear();
    error
}
