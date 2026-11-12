//! Модуль взаимодействия с XDG Desktop Portal `org.freedesktop.portal.ScreenCast` и PipeWire (`TASK-971`).
//! Передача FD через `SCM_RIGHTS`, согласование `SPA_DATA_DmaBuf` и авто-восстановление при рестарте `pipewire.service`.

#![allow(dead_code, unsafe_code)]

use std::fmt;
use std::io::ErrorKind;
use std::mem::size_of;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};

pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
pub const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
pub const SOURCE_TYPE_MONITOR: u32 = 1;
pub const SOURCE_TYPE_WINDOW: u32 = 2;
pub const SOURCE_TYPE_VIRTUAL: u32 = 4;
pub const SOURCE_TYPE_ALL_MASK: u32 = 7;

pub const SOL_SOCKET: i32 = 1;
pub const SCM_RIGHTS: i32 = 1;
pub const MSG_CTRUNC: i32 = 0x08;
pub const MSG_NOSIGNAL: i32 = 0x4000;
pub const F_DUPFD_CLOEXEC: i32 = 1030;
pub const MAX_FDS_PER_MESSAGE: usize = 16;

pub const DRM_FORMAT_MOD_LINEAR: u64 = 0;
pub const DRM_FORMAT_MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;
pub const DRM_FOURCC_BGRA8888: u32 = 0x3432_5241;
pub const DRM_FOURCC_RGBA8888: u32 = 0x3432_4241;
pub const DRM_FOURCC_NV12: u32 = 0x3231_564E;
pub const DRM_FOURCC_YUV420: u32 = 0x3231_5559;

pub const MIN_CAPTURE_DIMENSION: u32 = 64;
pub const MAX_CAPTURE_WIDTH: u32 = 7680;
pub const MAX_CAPTURE_HEIGHT: u32 = 4320;
pub const MAX_CAPTURE_FPS: u32 = 120;

const INT_BYTES: usize = size_of::<i32>();
const CMSG_HEADER_BYTES: usize = size_of::<Cmsghdr>();
const CONTROL_BUFFER_BYTES: usize = CMSG_HEADER_BYTES + MAX_FDS_PER_MESSAGE * INT_BYTES;

static TOTAL_FDS_OPENED: AtomicU64 = AtomicU64::new(0);
static TOTAL_FDS_CLOSED: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
struct Iovec {
    iov_base: *mut u8,
    iov_len: usize,
}

#[repr(C)]
struct Msghdr {
    msg_name: *mut u8,
    msg_namelen: u32,
    msg_iov: *mut Iovec,
    msg_iovlen: usize,
    msg_control: *mut u8,
    msg_controllen: usize,
    msg_flags: i32,
}

impl Msghdr {
    fn new(iov: &mut Iovec, msg_control: *mut u8, msg_controllen: usize) -> Self {
        Self {
            msg_name: std::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: iov,
            msg_iovlen: 1,
            msg_control,
            msg_controllen,
            msg_flags: 0,
        }
    }
}

#[repr(C)]
struct Cmsghdr {
    cmsg_len: usize,
    cmsg_level: i32,
    cmsg_type: i32,
}

#[repr(align(8))]
struct AlignedControlBuf([u8; CONTROL_BUFFER_BYTES]);

unsafe extern "C" {
    fn close(fd: i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn sendmsg(fd: i32, msg: *const Msghdr, flags: i32) -> isize;
    fn recvmsg(fd: i32, msg: *mut Msghdr, flags: i32) -> isize;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalError {
    InvalidToken(String),
    InvalidObjectPath(String),
    InvalidSourceTypes(u32),
    InvalidSessionState(&'static str, PortalSessionState),
    NoStreamsReturned,
    InvalidFd(RawFd),
    TooManyFds(usize),
    EmptyPayloadWithFds,
    ControlDataTruncated,
    MalformedAncillaryData,
    InvalidDimensions(u32, u32),
    InvalidStride(u32, u32),
    NoCompatibleDmaBufModifier,
    PipeWireDaemonDisconnected(String),
    IoError(String),
}

impl fmt::Display for PortalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PortalError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalSessionState {
    Unconnected,
    SessionCreated,
    SourcesSelected,
    Streaming,
    ReconnectingDaemon,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalSessionConfig {
    pub handle_token: String,
    pub session_handle_token: String,
    pub source_types: u32,
    pub restore_token: Option<String>,
}

impl PortalSessionConfig {
    #[must_use]
    pub fn default_monitor_and_window() -> Self {
        Self {
            handle_token: "discord_gpui_req_971".to_string(),
            session_handle_token: "discord_gpui_sess_971".to_string(),
            source_types: SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW,
            restore_token: None,
        }
    }

    pub fn validate(&self) -> Result<(), PortalError> {
        validate_portal_token(&self.handle_token)?;
        validate_portal_token(&self.session_handle_token)?;
        if self.source_types == 0 || (self.source_types & !SOURCE_TYPE_ALL_MASK) != 0 {
            return Err(PortalError::InvalidSourceTypes(self.source_types));
        }
        Ok(())
    }
}

pub fn validate_portal_token(token: &str) -> Result<(), PortalError> {
    let ok = !token.is_empty()
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(PortalError::InvalidToken(token.to_string()))
    }
}

pub fn validate_dbus_object_path(path: &str) -> Result<(), PortalError> {
    let ok = path == "/"
        || (path.starts_with('/')
            && !path.ends_with('/')
            && path[1..].split('/').all(|s| {
                !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }));
    if ok {
        Ok(())
    } else {
        Err(PortalError::InvalidObjectPath(path.to_string()))
    }
}

#[must_use]
pub fn redact_restore_token(token: &str) -> String {
    if token.trim().is_empty() {
        "<empty>".to_string()
    } else {
        "[REDACTED]".to_string()
    }
}

pub fn close_raw_fd(raw: RawFd) {
    if raw >= 0 {
        unsafe {
            let _ = close(raw);
        }
        TOTAL_FDS_CLOSED.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct OwnedPortalFd {
    raw: RawFd,
}

impl OwnedPortalFd {
    pub fn from_raw_fd_checked(raw: RawFd) -> Result<Self, PortalError> {
        if raw < 0 {
            return Err(PortalError::InvalidFd(raw));
        }
        TOTAL_FDS_OPENED.fetch_add(1, Ordering::SeqCst);
        Ok(Self { raw })
    }

    #[must_use]
    pub const fn as_raw_fd(&self) -> RawFd {
        self.raw
    }

    pub fn try_clone(&self) -> Result<Self, PortalError> {
        let dup = unsafe { fcntl(self.raw, F_DUPFD_CLOEXEC, 0) };
        if dup < 0 {
            return Err(PortalError::IoError(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        Self::from_raw_fd_checked(dup)
    }
}

impl Drop for OwnedPortalFd {
    fn drop(&mut self) {
        if self.raw >= 0 {
            close_raw_fd(self.raw);
            self.raw = -1;
        }
    }
}

const fn cmsg_align(len: usize) -> usize {
    (len + size_of::<usize>() - 1) & !(size_of::<usize>() - 1)
}

pub fn send_fds_scm_rights(
    s: &UnixStream,
    payload: &[u8],
    fds: &[RawFd],
) -> Result<(), PortalError> {
    if fds.len() > MAX_FDS_PER_MESSAGE {
        return Err(PortalError::TooManyFds(fds.len()));
    }
    if payload.is_empty() {
        return if fds.is_empty() {
            Ok(())
        } else {
            Err(PortalError::EmptyPayloadWithFds)
        };
    }
    if let Some(&bad) = fds.iter().find(|&&fd| fd < 0) {
        return Err(PortalError::InvalidFd(bad));
    }
    let mut ctrl = AlignedControlBuf([0u8; CONTROL_BUFFER_BYTES]);
    let mut iov = Iovec {
        iov_base: payload.as_ptr() as *mut u8,
        iov_len: payload.len(),
    };
    let (c_ptr, c_len) = encode_cmsg(&mut ctrl, fds);
    let hdr = Msghdr::new(&mut iov, c_ptr, c_len);
    if unsafe { sendmsg(s.as_raw_fd(), &hdr, MSG_NOSIGNAL) } < 0 {
        return Err(PortalError::IoError(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok(())
}

fn encode_cmsg(ctrl: &mut AlignedControlBuf, fds: &[RawFd]) -> (*mut u8, usize) {
    if fds.is_empty() {
        return (std::ptr::null_mut(), 0);
    }
    let data_bytes = fds.len() * INT_BYTES;
    let cmsg_len = CMSG_HEADER_BYTES + data_bytes;
    unsafe {
        let hdr = ctrl.0.as_mut_ptr() as *mut Cmsghdr;
        (*hdr).cmsg_len = cmsg_len;
        (*hdr).cmsg_level = SOL_SOCKET;
        (*hdr).cmsg_type = SCM_RIGHTS;
        let dst = ctrl.0.as_mut_ptr().add(CMSG_HEADER_BYTES);
        std::ptr::copy_nonoverlapping(fds.as_ptr() as *const u8, dst, data_bytes);
    }
    (ctrl.0.as_mut_ptr(), cmsg_align(cmsg_len))
}

pub fn recv_fds_scm_rights(
    s: &UnixStream,
    buf: &mut [u8],
) -> Result<(usize, Vec<OwnedPortalFd>), PortalError> {
    let mut ctrl = AlignedControlBuf([0u8; CONTROL_BUFFER_BYTES]);
    let mut iov = Iovec {
        iov_base: buf.as_mut_ptr(),
        iov_len: buf.len(),
    };
    let mut message = Msghdr::new(&mut iov, ctrl.0.as_mut_ptr(), CONTROL_BUFFER_BYTES);
    let n = unsafe { recvmsg(s.as_raw_fd(), &mut message, 0) };
    if n < 0 {
        let err = std::io::Error::last_os_error();
        if matches!(
            err.kind(),
            ErrorKind::ConnectionReset | ErrorKind::BrokenPipe
        ) {
            return Err(PortalError::PipeWireDaemonDisconnected(err.to_string()));
        }
        return Err(PortalError::IoError(err.to_string()));
    }
    finalize_scm_recv(n as usize, &message, &ctrl.0[..message.msg_controllen])
}

fn finalize_scm_recv(
    bytes: usize,
    message: &Msghdr,
    ctrl_slice: &[u8],
) -> Result<(usize, Vec<OwnedPortalFd>), PortalError> {
    let received_fds = parse_scm_rights_control(ctrl_slice)?;
    if (message.msg_flags & MSG_CTRUNC) != 0 {
        drop(received_fds);
        return Err(PortalError::ControlDataTruncated);
    }
    Ok((bytes, received_fds))
}

fn parse_scm_rights_control(ctrl: &[u8]) -> Result<Vec<OwnedPortalFd>, PortalError> {
    if ctrl.is_empty() {
        return Ok(Vec::new());
    }
    if ctrl.len() < CMSG_HEADER_BYTES {
        return Err(PortalError::MalformedAncillaryData);
    }
    let mut fds = Vec::new();
    let mut off = 0usize;
    while off + CMSG_HEADER_BYTES <= ctrl.len() {
        let hdr = unsafe { std::ptr::read_unaligned(ctrl[off..].as_ptr() as *const Cmsghdr) };
        let plen = hdr.cmsg_len.checked_sub(CMSG_HEADER_BYTES).unwrap_or(1);
        if hdr.cmsg_len < CMSG_HEADER_BYTES
            || off + hdr.cmsg_len > ctrl.len()
            || !plen.is_multiple_of(INT_BYTES)
            || hdr.cmsg_level != SOL_SOCKET
            || hdr.cmsg_type != SCM_RIGHTS
        {
            return Err(PortalError::MalformedAncillaryData);
        }
        for c in ctrl[off + CMSG_HEADER_BYTES..off + hdr.cmsg_len].chunks_exact(INT_BYTES) {
            fds.push(OwnedPortalFd::from_raw_fd_checked(i32::from_ne_bytes([
                c[0], c[1], c[2], c[3],
            ]))?);
        }
        off += cmsg_align(hdr.cmsg_len);
    }
    Ok(fds)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaBufNegotiationRequest {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub drm_fourcc: u32,
    pub candidate_stride: u32,
    pub advertised_modifiers: Vec<u64>,
    pub allow_shm_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedDmaBufConfig {
    pub drm_fourcc: u32,
    pub drm_modifier: u64,
    pub stride_bytes: u32,
    pub is_zero_copy: bool,
}

pub fn negotiate_dma_buf_stream(
    req: &DmaBufNegotiationRequest,
) -> Result<NegotiatedDmaBufConfig, PortalError> {
    let is_planar = matches!(req.drm_fourcc, DRM_FOURCC_NV12 | DRM_FOURCC_YUV420);
    let dims_ok = (MIN_CAPTURE_DIMENSION..=MAX_CAPTURE_WIDTH).contains(&req.width)
        && (MIN_CAPTURE_DIMENSION..=MAX_CAPTURE_HEIGHT).contains(&req.height)
        && (1..=MAX_CAPTURE_FPS).contains(&req.fps)
        && (!is_planar || (req.width % 2 == 0 && req.height % 2 == 0));
    if !dims_ok {
        return Err(PortalError::InvalidDimensions(req.width, req.height));
    }
    let min_stride = if is_planar {
        req.width
    } else {
        req.width.saturating_mul(4)
    };
    let candidate_stride = req.candidate_stride;
    if candidate_stride < min_stride {
        return Err(PortalError::InvalidStride(candidate_stride, min_stride));
    }
    let valid_mod = req
        .advertised_modifiers
        .iter()
        .copied()
        .find(|m| *m != DRM_FORMAT_MOD_INVALID);
    let (drm_modifier, is_zero_copy) = match (valid_mod, req.allow_shm_fallback) {
        (Some(m), _) => (m, true),
        (None, true) => (DRM_FORMAT_MOD_LINEAR, false),
        (None, false) => return Err(PortalError::NoCompatibleDmaBufModifier),
    };
    Ok(NegotiatedDmaBufConfig {
        drm_fourcc: req.drm_fourcc,
        drm_modifier,
        stride_bytes: candidate_stride,
        is_zero_copy,
    })
}

pub trait PortalBackend {
    fn create_session(&mut self, req: &str, sess: &str) -> Result<String, PortalError>;
    fn select_sources(&mut self, path: &str, cfg: &PortalSessionConfig) -> Result<(), PortalError>;
    fn start(&mut self, path: &str, req: &str) -> Result<(u32, Option<String>), PortalError>;
    fn open_pipewire_remote(&mut self, path: &str) -> Result<Vec<OwnedPortalFd>, PortalError>;
    fn close_session(&mut self, path: &str) -> Result<(), PortalError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRestartReport {
    pub closed_old_fd: Option<RawFd>,
    pub new_fd: RawFd,
    pub node_id: u32,
    pub session_recreated: bool,
    pub used_restore_token: bool,
    pub negotiated: NegotiatedDmaBufConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalTelemetrySnapshot {
    pub state: PortalSessionState,
    pub active_node_id: Option<u32>,
    pub active_fd: Option<RawFd>,
    pub redacted_restore_token: String,
    pub total_fds_opened: u64,
    pub total_fds_closed: u64,
}

impl PortalTelemetrySnapshot {
    #[must_use]
    pub const fn active_fd_balance(&self) -> u64 {
        self.total_fds_opened.saturating_sub(self.total_fds_closed)
    }
}

pub struct PipeWirePortalManager<B: PortalBackend> {
    backend: B,
    state: PortalSessionState,
    config: Option<PortalSessionConfig>,
    session_handle: Option<String>,
    active_fd: Option<OwnedPortalFd>,
    active_node_id: Option<u32>,
    saved_restore_token: Option<String>,
}

impl<B: PortalBackend> PipeWirePortalManager<B> {
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self {
            backend,
            state: PortalSessionState::Unconnected,
            config: None,
            session_handle: None,
            active_fd: None,
            active_node_id: None,
            saved_restore_token: None,
        }
    }

    fn fail<T>(&mut self, err: PortalError) -> Result<T, PortalError> {
        let _ = self.close();
        Err(err)
    }

    pub fn open_session(
        &mut self,
        cfg: &PortalSessionConfig,
        dma_req: &DmaBufNegotiationRequest,
    ) -> Result<NegotiatedDmaBufConfig, PortalError> {
        if !matches!(
            self.state,
            PortalSessionState::Unconnected | PortalSessionState::ReconnectingDaemon
        ) {
            return Err(PortalError::InvalidSessionState("Unconnected", self.state));
        }
        cfg.validate()?;
        let (path, node_id, r_tok, fd) = self.acquire_remote_session(cfg)?;
        let negotiated = match negotiate_dma_buf_stream(dma_req) {
            Ok(neg) => neg,
            Err(err) => {
                drop(fd);
                return self.fail(err);
            }
        };
        if let Some(tok) = r_tok.or_else(|| cfg.restore_token.clone()) {
            self.saved_restore_token = Some(tok);
        }
        self.config = Some(cfg.clone());
        self.session_handle = Some(path);
        self.active_fd = Some(fd);
        self.active_node_id = Some(node_id);
        self.state = PortalSessionState::Streaming;
        Ok(negotiated)
    }

    fn acquire_remote_session(
        &mut self,
        cfg: &PortalSessionConfig,
    ) -> Result<(String, u32, Option<String>, OwnedPortalFd), PortalError> {
        let path = self
            .backend
            .create_session(&cfg.handle_token, &cfg.session_handle_token)?;
        validate_dbus_object_path(&path)?;
        self.session_handle = Some(path.clone());
        self.state = PortalSessionState::SessionCreated;
        let mut eff = cfg.clone();
        eff.restore_token = eff
            .restore_token
            .or_else(|| self.saved_restore_token.clone());
        self.backend
            .select_sources(&path, &eff)
            .or_else(|e| self.fail(e))?;
        self.state = PortalSessionState::SourcesSelected;
        let (node_id, r_tok) = self
            .backend
            .start(&path, &cfg.handle_token)
            .or_else(|e| self.fail(e))?;
        if node_id == 0 {
            return self.fail(PortalError::NoStreamsReturned);
        }
        let mut fds = self
            .backend
            .open_pipewire_remote(&path)
            .or_else(|e| self.fail(e))?;
        if fds.is_empty() {
            return self.fail(PortalError::InvalidFd(-1));
        }
        Ok((path, node_id, r_tok, fds.remove(0)))
    }

    fn release_active_fd(&mut self) -> Option<RawFd> {
        let old = self.active_fd.take()?;
        let raw = old.as_raw_fd();
        drop(old);
        Some(raw)
    }

    pub fn recover_after_pipewire_restart(
        &mut self,
        dma_req: &DmaBufNegotiationRequest,
    ) -> Result<DaemonRestartReport, PortalError> {
        if self.state != PortalSessionState::Streaming {
            return Err(PortalError::InvalidSessionState("Streaming", self.state));
        }
        self.state = PortalSessionState::ReconnectingDaemon;
        let closed_old_fd = self.release_active_fd();
        if let Some(path) = self.session_handle.clone()
            && let Ok(mut fds) = self.backend.open_pipewire_remote(&path)
            && !fds.is_empty()
        {
            let new_owned = fds.remove(0);
            let negotiated = negotiate_dma_buf_stream(dma_req)?;
            let new_fd = new_owned.as_raw_fd();
            self.active_fd = Some(new_owned);
            self.state = PortalSessionState::Streaming;
            return Ok(DaemonRestartReport {
                closed_old_fd,
                new_fd,
                node_id: self.active_node_id.unwrap_or(1),
                session_recreated: false,
                used_restore_token: self.saved_restore_token.is_some(),
                negotiated,
            });
        }
        if let Some(old_h) = self.session_handle.take() {
            let _ = self.backend.close_session(&old_h);
        }
        let mut cfg = self
            .config
            .clone()
            .unwrap_or_else(PortalSessionConfig::default_monitor_and_window);
        cfg.restore_token = self.saved_restore_token.clone().or(cfg.restore_token);
        let negotiated = self.open_session(&cfg, dma_req)?;
        Ok(DaemonRestartReport {
            closed_old_fd,
            new_fd: self.active_fd.as_ref().map_or(-1, OwnedPortalFd::as_raw_fd),
            node_id: self.active_node_id.unwrap_or(1),
            session_recreated: true,
            used_restore_token: self.saved_restore_token.is_some(),
            negotiated,
        })
    }

    pub fn close(&mut self) -> Result<(), PortalError> {
        let _ = self.release_active_fd();
        let res = if let Some(h) = self.session_handle.take() {
            self.backend.close_session(&h)
        } else {
            Ok(())
        };
        self.state = PortalSessionState::Closed;
        res
    }

    #[must_use]
    pub fn telemetry_snapshot(&self) -> PortalTelemetrySnapshot {
        let raw_tok = self.saved_restore_token.as_deref().unwrap_or("");
        PortalTelemetrySnapshot {
            state: self.state,
            active_node_id: self.active_node_id,
            active_fd: self.active_fd.as_ref().map(OwnedPortalFd::as_raw_fd),
            redacted_restore_token: redact_restore_token(raw_tok),
            total_fds_opened: TOTAL_FDS_OPENED.load(Ordering::SeqCst),
            total_fds_closed: TOTAL_FDS_CLOSED.load(Ordering::SeqCst),
        }
    }
}

impl<B: PortalBackend> Drop for PipeWirePortalManager<B> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[derive(Default)]
    struct FakeBackend {
        fail_next_remote: bool,
        held_peers: Vec<UnixStream>,
    }

    impl PortalBackend for FakeBackend {
        fn create_session(&mut self, _r: &str, _s: &str) -> Result<String, PortalError> {
            Ok("/org/freedesktop/portal/desktop/session/1_971/s1".to_string())
        }
        fn select_sources(
            &mut self,
            _p: &str,
            _c: &PortalSessionConfig,
        ) -> Result<(), PortalError> {
            Ok(())
        }
        fn start(&mut self, _p: &str, _r: &str) -> Result<(u32, Option<String>), PortalError> {
            Ok((42, Some("secret_portal_restore_token_971".to_string())))
        }
        fn open_pipewire_remote(&mut self, _p: &str) -> Result<Vec<OwnedPortalFd>, PortalError> {
            if self.fail_next_remote {
                self.fail_next_remote = false;
                return Err(PortalError::PipeWireDaemonDisconnected("stale".to_string()));
            }
            let (tx, rx) = UnixStream::pair().map_err(|e| PortalError::IoError(e.to_string()))?;
            let (pw, peer) = UnixStream::pair().map_err(|e| PortalError::IoError(e.to_string()))?;
            self.held_peers.push(peer);
            send_fds_scm_rights(&tx, b"PW", &[pw.as_raw_fd()])?;
            drop(pw);
            let mut buf = [0u8; 8];
            Ok(recv_fds_scm_rights(&rx, &mut buf)?.1)
        }
        fn close_session(&mut self, _p: &str) -> Result<(), PortalError> {
            Ok(())
        }
    }

    fn default_dma_req() -> DmaBufNegotiationRequest {
        DmaBufNegotiationRequest {
            width: 1920,
            height: 1080,
            fps: 60,
            drm_fourcc: DRM_FOURCC_BGRA8888,
            candidate_stride: 1920 * 4,
            advertised_modifiers: vec![0x0100_0000_0000_0001, DRM_FORMAT_MOD_LINEAR],
            allow_shm_fallback: true,
        }
    }

    #[test]
    fn test_sub_971_1_portal_session_lifecycle_and_token_redaction() {
        assert_eq!(PORTAL_BUS_NAME, "org.freedesktop.portal.Desktop");
        assert_eq!(PORTAL_OBJECT_PATH, "/org/freedesktop/portal/desktop");
        assert_eq!(SCREENCAST_INTERFACE, "org.freedesktop.portal.ScreenCast");
        assert!(validate_portal_token("bad-token").is_err());
        assert!(validate_dbus_object_path("relative/path").is_err());
        assert_eq!(redact_restore_token(""), "<empty>");
        assert_eq!(redact_restore_token("tok_secret_971"), "[REDACTED]");

        let mut mgr = PipeWirePortalManager::new(FakeBackend::default());
        let cfg = PortalSessionConfig::default_monitor_and_window();
        let neg = mgr.open_session(&cfg, &default_dma_req()).expect("open ok");
        assert!(neg.is_zero_copy);
        let snap = mgr.telemetry_snapshot();
        assert_eq!(snap.state, PortalSessionState::Streaming);
        assert_eq!(snap.active_node_id, Some(42));
        assert_eq!(snap.redacted_restore_token, "[REDACTED]");
        mgr.close().expect("close ok");
        assert_eq!(mgr.telemetry_snapshot().active_fd_balance(), 0);
    }

    #[test]
    fn test_sub_971_2_scm_rights_fd_transfer_and_zero_leaks() {
        assert!(OwnedPortalFd::from_raw_fd_checked(-1).is_err());
        let (tx, rx) = UnixStream::pair().expect("pair");
        let empty_err = send_fds_scm_rights(&tx, b"", &[tx.as_raw_fd()]);
        assert_eq!(empty_err, Err(PortalError::EmptyPayloadWithFds));
        let many = vec![tx.as_raw_fd(); MAX_FDS_PER_MESSAGE + 1];
        assert!(send_fds_scm_rights(&tx, b"x", &many).is_err());

        let (mut p_a, p_b) = UnixStream::pair().expect("pipe");
        send_fds_scm_rights(&tx, b"FD", &[p_b.as_raw_fd()]).expect("send");
        drop(p_b);
        let mut buf = [0u8; 8];
        let (n, fds) = recv_fds_scm_rights(&rx, &mut buf).expect("recv");
        assert_eq!(&buf[..n], b"FD");
        let cloned = fds[0].try_clone().expect("clone");
        p_a.write_all(b"ping").expect("write");
        drop(p_a);
        let mut s =
            unsafe { <UnixStream as std::os::fd::FromRawFd>::from_raw_fd(cloned.as_raw_fd()) };
        let mut out = [0u8; 4];
        s.read_exact(&mut out).expect("read");
        let _ = std::os::fd::IntoRawFd::into_raw_fd(s);
        assert_eq!(&out, b"ping");
        drop(cloned);
        drop(fds);

        let mut iov = Iovec {
            iov_base: std::ptr::null_mut(),
            iov_len: 0,
        };
        let mut msg = Msghdr::new(&mut iov, std::ptr::null_mut(), 0);
        msg.msg_flags = MSG_CTRUNC;
        let trunc = finalize_scm_recv(1, &msg, &[]);
        assert_eq!(trunc, Err(PortalError::ControlDataTruncated));
        let bad_cmsg = parse_scm_rights_control(&[1, 2]);
        assert_eq!(bad_cmsg, Err(PortalError::MalformedAncillaryData));
    }

    #[test]
    fn test_sub_971_3_dma_buf_negotiation_and_validation() {
        let mut req = default_dma_req();
        let neg = negotiate_dma_buf_stream(&req).expect("zero copy");
        assert!(neg.is_zero_copy);
        assert_eq!(neg.drm_modifier, 0x0100_0000_0000_0001);

        req.advertised_modifiers = vec![DRM_FORMAT_MOD_INVALID];
        let shm = negotiate_dma_buf_stream(&req).expect("shm fallback");
        assert!(!shm.is_zero_copy);

        req.allow_shm_fallback = false;
        let no_mod = negotiate_dma_buf_stream(&req);
        assert_eq!(no_mod, Err(PortalError::NoCompatibleDmaBufModifier));

        let mut bad = default_dma_req();
        bad.candidate_stride = 100;
        let bad_res = negotiate_dma_buf_stream(&bad);
        assert_eq!(bad_res, Err(PortalError::InvalidStride(100, 1920 * 4)));
    }

    #[test]
    fn test_sub_971_4_daemon_restart_recovery_and_fd_cleanup() {
        let mut mgr = PipeWirePortalManager::new(FakeBackend::default());
        let cfg = PortalSessionConfig::default_monitor_and_window();
        let dma = default_dma_req();
        mgr.open_session(&cfg, &dma).expect("open");
        let old_fd = mgr.telemetry_snapshot().active_fd.expect("fd");

        let rep1 = mgr.recover_after_pipewire_restart(&dma).expect("reopen");
        assert_eq!(rep1.closed_old_fd, Some(old_fd));
        assert!(!rep1.session_recreated);

        let old_fd2 = mgr.telemetry_snapshot().active_fd.expect("fd2");
        mgr.backend.fail_next_remote = true;
        let rep2 = mgr.recover_after_pipewire_restart(&dma).expect("recreate");
        assert_eq!(rep2.closed_old_fd, Some(old_fd2));
        assert!(rep2.session_recreated);
        assert!(rep2.used_restore_token);
        mgr.close().expect("close");
        assert_eq!(mgr.telemetry_snapshot().active_fd_balance(), 0);
    }
}
