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
    ConsentDenied,
    NoStreamsReturned,
    InvalidFd(RawFd),
    TooManyFds(usize),
    EmptyPayloadWithFds,
    ControlDataTruncated,
    UnexpectedAncillaryHeader(i32, i32),
    MalformedAncillaryData,
    InvalidDimensions(u32, u32),
    OddChromaDimensions(u32, u32),
    InvalidFps(u32),
    InvalidStride(u32, u32),
    NoCompatibleDmaBufModifier,
    PipeWireDaemonDisconnected(String),
    MaxReconnectAttemptsExceeded(u32),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalResponseCode {
    Success = 0,
    CancelledByUser = 1,
}

impl PortalResponseCode {
    pub const fn into_result(self) -> Result<(), PortalError> {
        match self {
            Self::Success => Ok(()),
            Self::CancelledByUser => Err(PortalError::ConsentDenied),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalSessionConfig {
    pub handle_token: String,
    pub session_handle_token: String,
    pub source_types: u32,
    pub multiple: bool,
    pub cursor_mode: u32,
    pub persist_mode: u32,
    pub restore_token: Option<String>,
    pub parent_window: String,
}

impl PortalSessionConfig {
    #[must_use]
    pub fn default_monitor_and_window() -> Self {
        Self {
            handle_token: "discord_gpui_req_971".to_string(),
            session_handle_token: "discord_gpui_sess_971".to_string(),
            source_types: SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW,
            multiple: false,
            cursor_mode: 4,
            persist_mode: 2,
            restore_token: None,
            parent_window: String::new(),
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
    if path == "/" {
        return Ok(());
    }
    let valid_segs = path.starts_with('/')
        && !path.ends_with('/')
        && path[1..]
            .split('/')
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    if valid_segs {
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

#[derive(Debug)]
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
        {
            return Err(PortalError::MalformedAncillaryData);
        }
        for chunk in ctrl[off + CMSG_HEADER_BYTES..off + hdr.cmsg_len].chunks_exact(INT_BYTES) {
            fds.push(OwnedPortalFd::from_raw_fd_checked(i32::from_ne_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3],
            ]))?);
        }
        if hdr.cmsg_level != SOL_SOCKET || hdr.cmsg_type != SCM_RIGHTS {
            fds.clear();
            return Err(PortalError::UnexpectedAncillaryHeader(
                hdr.cmsg_level,
                hdr.cmsg_type,
            ));
        }
        off += cmsg_align(hdr.cmsg_len);
    }
    Ok(fds)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaBufPixelFormat {
    Bgra8,
    Rgba8,
    Nv12,
    I420,
}

impl DmaBufPixelFormat {
    #[must_use]
    pub const fn drm_fourcc(self) -> u32 {
        match self {
            Self::Bgra8 => DRM_FOURCC_BGRA8888,
            Self::Rgba8 => DRM_FOURCC_RGBA8888,
            Self::Nv12 => DRM_FOURCC_NV12,
            Self::I420 => DRM_FOURCC_YUV420,
        }
    }

    #[must_use]
    pub const fn min_stride_bytes(self, width: u32) -> u32 {
        match self {
            Self::Bgra8 | Self::Rgba8 => width.saturating_mul(4),
            Self::Nv12 | Self::I420 => width,
        }
    }

    #[must_use]
    pub const fn requires_even_dimensions(self) -> bool {
        matches!(self, Self::Nv12 | Self::I420)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaBufBufferKind {
    ZeroCopyDmaBuf,
    SharedMemoryMemFd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaBufNegotiationRequest {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: DmaBufPixelFormat,
    pub candidate_stride: u32,
    pub advertised_modifiers: Vec<u64>,
    pub allow_shm_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedDmaBufConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: DmaBufPixelFormat,
    pub drm_fourcc: u32,
    pub drm_modifier: u64,
    pub stride_bytes: u32,
    pub buffer_kind: DmaBufBufferKind,
    pub is_zero_copy: bool,
}

pub fn negotiate_dma_buf_stream(
    req: &DmaBufNegotiationRequest,
) -> Result<NegotiatedDmaBufConfig, PortalError> {
    if !(MIN_CAPTURE_DIMENSION..=MAX_CAPTURE_WIDTH).contains(&req.width)
        || !(MIN_CAPTURE_DIMENSION..=MAX_CAPTURE_HEIGHT).contains(&req.height)
    {
        return Err(PortalError::InvalidDimensions(req.width, req.height));
    }
    if req.format.requires_even_dimensions() && (req.width % 2 != 0 || req.height % 2 != 0) {
        return Err(PortalError::OddChromaDimensions(req.width, req.height));
    }
    if req.fps == 0 || req.fps > MAX_CAPTURE_FPS {
        return Err(PortalError::InvalidFps(req.fps));
    }
    let min_stride = req.format.min_stride_bytes(req.width);
    let candidate_stride = req.candidate_stride;
    if candidate_stride < min_stride {
        return Err(PortalError::InvalidStride(candidate_stride, min_stride));
    }
    let valid_mod = req
        .advertised_modifiers
        .iter()
        .copied()
        .find(|m| *m != DRM_FORMAT_MOD_INVALID);
    let (drm_modifier, buffer_kind, is_zero_copy) = match (valid_mod, req.allow_shm_fallback) {
        (Some(m), _) => (m, DmaBufBufferKind::ZeroCopyDmaBuf, true),
        (None, true) => (
            DRM_FORMAT_MOD_LINEAR,
            DmaBufBufferKind::SharedMemoryMemFd,
            false,
        ),
        (None, false) => return Err(PortalError::NoCompatibleDmaBufModifier),
    };
    Ok(NegotiatedDmaBufConfig {
        width: req.width,
        height: req.height,
        fps: req.fps,
        format: req.format,
        drm_fourcc: req.format.drm_fourcc(),
        drm_modifier,
        stride_bytes: candidate_stride,
        buffer_kind,
        is_zero_copy,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalStreamDescriptor {
    pub node_id: u32,
    pub source_type: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalStartReply {
    pub code: PortalResponseCode,
    pub streams: Vec<PortalStreamDescriptor>,
    pub restore_token: Option<String>,
}

pub trait PortalBackend {
    fn create_session(
        &mut self,
        req: &str,
        sess: &str,
    ) -> Result<(PortalResponseCode, String), PortalError>;
    fn select_sources(
        &mut self,
        path: &str,
        cfg: &PortalSessionConfig,
    ) -> Result<PortalResponseCode, PortalError>;
    fn start(&mut self, path: &str, win: &str, req: &str) -> Result<PortalStartReply, PortalError>;
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
    pub daemon_restarts: u64,
    pub successful_recoveries: u64,
    pub dma_buf_sessions: u64,
    pub shm_fallback_sessions: u64,
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
    counters: (u64, u64, u64, u64),
    max_reconnect_attempts: u32,
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
            counters: (0, 0, 0, 0),
            max_reconnect_attempts: 3,
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
        let path = self.step_create_and_select(cfg)?;
        let (node_id, r_tok, fd) = self.step_start_and_open(&path, cfg)?;
        let negotiated = match negotiate_dma_buf_stream(dma_req) {
            Ok(neg) => neg,
            Err(err) => {
                drop(fd);
                return self.fail(err);
            }
        };
        if negotiated.is_zero_copy {
            self.counters.2 += 1
        } else {
            self.counters.3 += 1
        }
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

    fn step_create_and_select(&mut self, cfg: &PortalSessionConfig) -> Result<String, PortalError> {
        let (code, path) = self
            .backend
            .create_session(&cfg.handle_token, &cfg.session_handle_token)?;
        if let Err(err) = code.into_result() {
            self.state = PortalSessionState::Closed;
            return Err(err);
        }
        validate_dbus_object_path(&path)?;
        self.session_handle = Some(path.clone());
        self.state = PortalSessionState::SessionCreated;
        let mut eff = cfg.clone();
        if eff.restore_token.is_none() {
            eff.restore_token = self.saved_restore_token.clone();
        }
        if let Err(err) = self
            .backend
            .select_sources(&path, &eff)
            .and_then(|c| c.into_result())
        {
            return self.fail(err);
        }
        self.state = PortalSessionState::SourcesSelected;
        Ok(path)
    }

    fn step_start_and_open(
        &mut self,
        path: &str,
        cfg: &PortalSessionConfig,
    ) -> Result<(u32, Option<String>, OwnedPortalFd), PortalError> {
        let reply = match self
            .backend
            .start(path, &cfg.parent_window, &cfg.handle_token)
        {
            Ok(r) => r,
            Err(err) => return self.fail(err),
        };
        if let Err(err) = reply.code.into_result() {
            return self.fail(err);
        }
        let Some(first) = reply.streams.first().filter(|s| s.node_id > 0) else {
            return self.fail(PortalError::NoStreamsReturned);
        };
        let mut fds = match self.backend.open_pipewire_remote(path) {
            Ok(list) if !list.is_empty() => list,
            Ok(_) => return self.fail(PortalError::InvalidFd(-1)),
            Err(err) => return self.fail(err),
        };
        let primary = fds.remove(0);
        drop(fds);
        Ok((first.node_id, reply.restore_token, primary))
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
        self.counters.0 += 1;
        self.state = PortalSessionState::ReconnectingDaemon;
        let closed_old_fd = self.release_active_fd();
        if let Some(rep) = self.try_reopen_existing(closed_old_fd, dma_req)? {
            self.counters.1 += 1;
            return Ok(rep);
        }
        let rep = self.recreate_with_retries(closed_old_fd, dma_req)?;
        self.counters.1 += 1;
        Ok(rep)
    }

    fn try_reopen_existing(
        &mut self,
        closed_old_fd: Option<RawFd>,
        dma_req: &DmaBufNegotiationRequest,
    ) -> Result<Option<DaemonRestartReport>, PortalError> {
        let Some(path) = self.session_handle.clone() else {
            return Ok(None);
        };
        let Ok(mut fds) = self.backend.open_pipewire_remote(&path) else {
            return Ok(None);
        };
        if fds.is_empty() {
            return Ok(None);
        }
        let new_owned = fds.remove(0);
        drop(fds);
        let negotiated = negotiate_dma_buf_stream(dma_req)?;
        if negotiated.is_zero_copy {
            self.counters.2 += 1
        } else {
            self.counters.3 += 1
        }
        let new_fd = new_owned.as_raw_fd();
        self.active_fd = Some(new_owned);
        self.state = PortalSessionState::Streaming;
        Ok(Some(DaemonRestartReport {
            closed_old_fd,
            new_fd,
            node_id: self.active_node_id.unwrap_or(1),
            session_recreated: false,
            used_restore_token: self.saved_restore_token.is_some(),
            negotiated,
        }))
    }

    fn recreate_with_retries(
        &mut self,
        closed_old_fd: Option<RawFd>,
        dma_req: &DmaBufNegotiationRequest,
    ) -> Result<DaemonRestartReport, PortalError> {
        if let Some(old_h) = self.session_handle.take() {
            let _ = self.backend.close_session(&old_h);
        }
        let mut cfg = self
            .config
            .clone()
            .unwrap_or_else(PortalSessionConfig::default_monitor_and_window);
        let used_restore_token = self.saved_restore_token.is_some();
        if used_restore_token {
            cfg.restore_token = self.saved_restore_token.clone();
        }
        for _ in 0..self.max_reconnect_attempts {
            self.state = PortalSessionState::ReconnectingDaemon;
            if let Ok(negotiated) = self.open_session(&cfg, dma_req) {
                return Ok(DaemonRestartReport {
                    closed_old_fd,
                    new_fd: self.active_fd.as_ref().map_or(-1, OwnedPortalFd::as_raw_fd),
                    node_id: self.active_node_id.unwrap_or(1),
                    session_recreated: true,
                    used_restore_token,
                    negotiated,
                });
            }
        }
        self.state = PortalSessionState::Closed;
        Err(PortalError::MaxReconnectAttemptsExceeded(
            self.max_reconnect_attempts,
        ))
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
            daemon_restarts: self.counters.0,
            successful_recoveries: self.counters.1,
            dma_buf_sessions: self.counters.2,
            shm_fallback_sessions: self.counters.3,
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

    struct FakeBackend {
        fail_next_remote: bool,
        held_peers: Vec<UnixStream>,
    }

    impl FakeBackend {
        fn new() -> Self {
            Self {
                fail_next_remote: false,
                held_peers: Vec::new(),
            }
        }
    }

    impl PortalBackend for FakeBackend {
        fn create_session(
            &mut self,
            _r: &str,
            _s: &str,
        ) -> Result<(PortalResponseCode, String), PortalError> {
            Ok((
                PortalResponseCode::Success,
                "/org/freedesktop/portal/desktop/session/1_971/s1".to_string(),
            ))
        }

        fn select_sources(
            &mut self,
            _p: &str,
            _c: &PortalSessionConfig,
        ) -> Result<PortalResponseCode, PortalError> {
            Ok(PortalResponseCode::Success)
        }

        fn start(&mut self, _p: &str, _w: &str, _r: &str) -> Result<PortalStartReply, PortalError> {
            Ok(PortalStartReply {
                code: PortalResponseCode::Success,
                streams: vec![PortalStreamDescriptor {
                    node_id: 42,
                    source_type: 1,
                    width: 1920,
                    height: 1080,
                }],
                restore_token: Some("secret_portal_restore_token_971".to_string()),
            })
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
            format: DmaBufPixelFormat::Bgra8,
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

        let mut mgr = PipeWirePortalManager::new(FakeBackend::new());
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
        assert_eq!(
            send_fds_scm_rights(&tx, b"", &[tx.as_raw_fd()]),
            Err(PortalError::EmptyPayloadWithFds)
        );
        let many = vec![tx.as_raw_fd(); MAX_FDS_PER_MESSAGE + 1];
        assert_eq!(
            send_fds_scm_rights(&tx, b"x", &many),
            Err(PortalError::TooManyFds(MAX_FDS_PER_MESSAGE + 1))
        );

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

        let mut dummy_iov = Iovec {
            iov_base: std::ptr::null_mut(),
            iov_len: 0,
        };
        let mut trunc_msg = Msghdr::new(&mut dummy_iov, std::ptr::null_mut(), 0);
        trunc_msg.msg_flags = MSG_CTRUNC;
        assert_eq!(
            finalize_scm_recv(1, &trunc_msg, &[]).unwrap_err(),
            PortalError::ControlDataTruncated
        );
        assert_eq!(
            parse_scm_rights_control(&[1, 2, 3]).unwrap_err(),
            PortalError::MalformedAncillaryData
        );
    }

    #[test]
    fn test_sub_971_3_dma_buf_negotiation_and_validation() {
        let mut req = default_dma_req();
        let neg = negotiate_dma_buf_stream(&req).expect("zero copy");
        assert_eq!(neg.buffer_kind, DmaBufBufferKind::ZeroCopyDmaBuf);
        assert_eq!(neg.drm_modifier, 0x0100_0000_0000_0001);

        req.advertised_modifiers = vec![DRM_FORMAT_MOD_INVALID];
        let shm = negotiate_dma_buf_stream(&req).expect("shm fallback");
        assert_eq!(shm.buffer_kind, DmaBufBufferKind::SharedMemoryMemFd);
        assert!(!shm.is_zero_copy);

        req.allow_shm_fallback = false;
        assert_eq!(
            negotiate_dma_buf_stream(&req),
            Err(PortalError::NoCompatibleDmaBufModifier)
        );

        let mut bad_stride = default_dma_req();
        bad_stride.candidate_stride = 100;
        assert_eq!(
            negotiate_dma_buf_stream(&bad_stride),
            Err(PortalError::InvalidStride(100, 1920 * 4))
        );
    }

    #[test]
    fn test_sub_971_4_daemon_restart_recovery_and_fd_cleanup() {
        let mut mgr = PipeWirePortalManager::new(FakeBackend::new());
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
