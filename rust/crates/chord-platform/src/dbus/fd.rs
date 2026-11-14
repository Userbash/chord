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
//! - received fd have `FD_CLOEXEC`: Linux/Android sets the flag atomically with
//!   `MSG_CMSG_CLOEXEC`, so the descriptors are closed on `exec`;
//! - other Unix targets use `fcntl` before returning fd. That fallback is not atomic:
//!   callers must coordinate receipt with concurrent `fork`/`exec`.

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

/// Native socket constants, also re-exported by the legacy GPUI portal facade.
pub const SOL_SOCKET: i32 = libc::SOL_SOCKET;
pub const SCM_RIGHTS: i32 = libc::SCM_RIGHTS;
pub const MSG_CTRUNC: i32 = libc::MSG_CTRUNC;
pub const MSG_NOSIGNAL: i32 = libc::MSG_NOSIGNAL;
pub const F_DUPFD_CLOEXEC: i32 = libc::F_DUPFD_CLOEXEC;

/// Предел ядра Linux на число fd в одном `sendmsg` (`SCM_MAX_FD`).
const SCM_MAX_FD: usize = 253;

const INT_BYTES: usize = size_of::<libc::c_int>();

#[cfg(any(target_os = "linux", target_os = "android"))]
const RECEIVE_FLAGS: libc::c_int = libc::MSG_CMSG_CLOEXEC;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const RECEIVE_FLAGS: libc::c_int = 0;

/// Буфер ancillary для приёма: `cmsghdr` + ровно [`MAX_FDS_PER_MESSAGE`] fd.
///
/// Ровно по лимиту (а не с запасом), чтобы сообщение с бо́льшим числом fd гарантированно
/// приходило с `MSG_CTRUNC` и отвергалось, а не «влезало наполовину».
// SAFETY: the bounded payload is at most 16 c_int values; CMSG_SPACE only
// computes the native header size and alignment and cannot overflow here.
const CONTROL_BYTES: usize =
    unsafe { libc::CMSG_SPACE((MAX_FDS_PER_MESSAGE * INT_BYTES) as u32) } as usize;
/// Буфер ancillary для отправки: до [`SCM_MAX_FD`] fd.
// SAFETY: the bounded payload is at most 253 c_int values, below c_uint::MAX.
const SEND_CONTROL_BYTES: usize =
    unsafe { libc::CMSG_SPACE((SCM_MAX_FD * INT_BYTES) as u32) } as usize;

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

/// Duplicate a descriptor atomically with FD_CLOEXEC; the caller owns the result.
/// An invalid descriptor or native failure returns the original OS error.
pub fn duplicate_cloexec(raw: RawFd) -> Result<RawFd, std::io::Error> {
    loop {
        // SAFETY: fcntl only queries the integer descriptor and allocates a new one;
        // invalid descriptors return EBADF and no pointers are dereferenced.
        let duplicate = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 0) };
        if duplicate >= 0 {
            return Ok(duplicate);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error);
        }
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
            (*header).cmsg_len = checked_control_length(libc::CMSG_LEN(payload_u32))?;
            std::ptr::copy_nonoverlapping(
                fds.as_ptr() as *const u8,
                libc::CMSG_DATA(header),
                payload,
            );
            // SAFETY: `CMSG_SPACE` — чистая функция от длины; значение влезает в буфер.
            libc::CMSG_SPACE(payload_u32) as usize
        };
        message.msg_control = control.0.as_mut_ptr() as *mut libc::c_void;
        message.msg_controllen = checked_control_length(control_len)?;
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
    let mut filled = 0;
    while filled < buffer.len() {
        match recv_with_fds(stream, &mut buffer[filled..], fds) {
            Ok(0) => return Err(discard(fds, DbusError::Protocol("truncated message"))),
            Ok(received) => filled += received,
            Err(DbusError::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                return Err(discard(fds, DbusError::Timeout));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Receive one available chunk without waiting to fill the buffer; EOF returns 0.
/// Received descriptors have FD_CLOEXEC. Every descriptor in fds is closed and
/// removed on failure. OS errors retain their original kind; EINTR is retried.
/// An empty buffer returns 0 without consuming bytes or changing existing fds.
pub fn recv_with_fds(
    stream: &UnixStream,
    buffer: &mut [u8],
    fds: &mut Vec<RawFd>,
) -> Result<usize, DbusError> {
    if buffer.is_empty() {
        return Ok(0);
    }
    let mut control = ControlBuffer([0u8; CONTROL_BYTES]);
    let mut iov = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast(),
        iov_len: buffer.len(),
    };
    // SAFETY: msghdr is a C value with nullable pointers; its fields are filled
    // with live buffers below before it is passed to recvmsg.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.0.as_mut_ptr().cast();
    message.msg_controllen =
        checked_control_length(CONTROL_BYTES).map_err(|error| discard(fds, error))?;
    let received = receive_message(stream, &mut message)
        .map_err(|error| discard(fds, DbusError::Io(error)))?;
    collect_received_fds(&message, fds).map_err(|error| discard(fds, error))?;
    if message.msg_flags & libc::MSG_CTRUNC != 0 {
        return Err(discard(fds, DbusError::Protocol("control data truncated")));
    }
    if fds.len() > MAX_FDS_PER_MESSAGE {
        return Err(discard(
            fds,
            DbusError::Protocol("too many unix fds in message"),
        ));
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    ensure_close_on_exec(fds).map_err(|error| discard(fds, DbusError::Io(error)))?;
    Ok(received)
}

fn receive_message(
    stream: &UnixStream,
    message: &mut libc::msghdr,
) -> Result<usize, std::io::Error> {
    let capacity = message.msg_controllen;
    loop {
        message.msg_controllen = capacity;
        message.msg_flags = 0;
        // SAFETY: recv_with_fds supplies live data/control buffers and native libc
        // layouts. Linux/Android sets close-on-exec atomically for delivered fds.
        let received = unsafe { libc::recvmsg(stream.as_raw_fd(), message, RECEIVE_FLAGS) };
        if received >= 0 {
            return Ok(received as usize);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn collect_received_fds(message: &libc::msghdr, fds: &mut Vec<RawFd>) -> Result<(), DbusError> {
    let mut first_error = None;
    // SAFETY: recvmsg populated this control buffer. Native CMSG helpers bound
    // traversal; each payload is range-checked before its descriptor bytes are read.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(message);
        while !header.is_null() {
            match ancillary_fd_count(message, header) {
                Ok(count) => {
                    let data = libc::CMSG_DATA(header);
                    for index in 0..count {
                        let raw = std::ptr::read_unaligned(
                            data.add(index * INT_BYTES).cast::<libc::c_int>(),
                        );
                        if raw < 0 {
                            first_error.get_or_insert(DbusError::Protocol("invalid unix fd"));
                        } else {
                            fds.push(raw);
                        }
                    }
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
            // An unexpected header can precede installed SCM_RIGHTS descriptors.
            // Drain later rights before reporting the error so the caller closes them.
            header = libc::CMSG_NXTHDR(message, header);
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn ancillary_fd_count(
    message: &libc::msghdr,
    header: *mut libc::cmsghdr,
) -> Result<usize, DbusError> {
    // SAFETY: collect_received_fds obtains header from the native CMSG iterator;
    // it points into the live kernel-populated control buffer owned by the caller.
    let (level, kind, length, header_len, data) = unsafe {
        (
            (*header).cmsg_level,
            (*header).cmsg_type,
            (*header).cmsg_len,
            libc::CMSG_LEN(0) as usize,
            libc::CMSG_DATA(header),
        )
    };
    let length: usize = checked_control_length(length)?;
    if level != libc::SOL_SOCKET || kind != libc::SCM_RIGHTS {
        return Err(DbusError::Protocol("unexpected ancillary data"));
    }
    if length < header_len || !(length - header_len).is_multiple_of(INT_BYTES) {
        return Err(DbusError::Protocol("malformed ancillary data"));
    }
    let count = (length - header_len) / INT_BYTES;
    let control_length: usize = checked_control_length(message.msg_controllen)?;
    let control_end = (message.msg_control as usize).saturating_add(control_length);
    let header_end = (header as usize).saturating_add(length);
    let data_end = (data as usize).saturating_add(count * INT_BYTES);
    if header_end > control_end || data_end > control_end {
        return Err(DbusError::Protocol("ancillary data exceeds control buffer"));
    }
    Ok(count)
}

/// libc uses size_t for these fields on Linux and socklen_t on Darwin.
/// Check conversions in both directions rather than truncating a native field.
fn checked_control_length<T, U>(length: T) -> Result<U, DbusError>
where
    U: TryFrom<T>,
{
    U::try_from(length).map_err(|_| DbusError::Protocol("unix fd control length out of range"))
}

/// POSIX fallback: set close-on-exec before handing descriptors to the caller.
/// The caller closes the complete received set if any fcntl call fails.
#[cfg(any(test, not(any(target_os = "linux", target_os = "android"))))]
fn ensure_close_on_exec(fds: &[RawFd]) -> Result<(), std::io::Error> {
    for &raw in fds {
        // SAFETY: the receive path owns each descriptor until it is returned;
        // F_GETFD reads descriptor flags without taking ownership or accessing memory.
        let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
        if flags < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if flags & libc::FD_CLOEXEC != 0 {
            continue;
        }
        // SAFETY: raw is still owned by the receive path. F_SETFD only changes its
        // descriptor flags, preserving existing flags and leaving ownership unchanged.
        if unsafe { libc::fcntl(raw, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Закрывает собранные fd и очищает вектор, чтобы вызывающий не увидел «висящих» fd.
fn discard(fds: &mut Vec<RawFd>, error: DbusError) -> DbusError {
    close_all(fds);
    fds.clear();
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor_flags(raw: RawFd) -> libc::c_int {
        // SAFETY: tests keep the descriptor's owning UnixStream alive; F_GETFD
        // reads flags without changing ownership or accessing memory.
        let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
        assert!(flags >= 0, "F_GETFD: {}", std::io::Error::last_os_error());
        flags
    }

    #[test]
    fn close_on_exec_fallback_marks_every_descriptor_and_preserves_ownership() {
        let (first, second) = UnixStream::pair().expect("socketpair");
        let descriptors = [first.as_raw_fd(), second.as_raw_fd()];
        for raw in descriptors {
            let flags = descriptor_flags(raw);
            // SAFETY: every fd is owned by a live UnixStream. Clearing its descriptor
            // flag creates the state produced by recvmsg without MSG_CMSG_CLOEXEC.
            let result = unsafe { libc::fcntl(raw, libc::F_SETFD, flags & !libc::FD_CLOEXEC) };
            assert_eq!(result, 0, "clear FD_CLOEXEC");
        }

        ensure_close_on_exec(&descriptors).expect("set FD_CLOEXEC");

        for raw in descriptors {
            assert_ne!(descriptor_flags(raw) & libc::FD_CLOEXEC, 0);
        }
        ensure_close_on_exec(&descriptors).expect("already set is idempotent");
    }

    #[test]
    fn close_on_exec_fallback_returns_the_os_error() {
        let error = ensure_close_on_exec(&[-1]).expect_err("invalid descriptor");
        assert_eq!(error.raw_os_error(), Some(libc::EBADF));
    }

    #[test]
    fn ancillary_lengths_preserve_native_width_and_reject_overflow() {
        let native: u32 = checked_control_length(80usize).expect("Darwin control field");
        let address_length: usize = checked_control_length(native).expect("buffer offset");
        assert_eq!(native, 80);
        assert_eq!(address_length, 80);
        let overflow = u64::from(u32::MAX) + 1;
        let result: Result<u32, DbusError> = checked_control_length(overflow);
        assert!(matches!(result, Err(DbusError::Protocol(_))));
    }
}
