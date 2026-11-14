//! Модуль взаимодействия с XDG Desktop Portal `org.freedesktop.portal.ScreenCast` и PipeWire (`TASK-971`).
//! Передача FD через `SCM_RIGHTS`, согласование `SPA_DATA_DmaBuf` и авто-восстановление при рестарте `pipewire.service`.

#![allow(dead_code, unsafe_code)]

use std::io::ErrorKind;
#[cfg(test)]
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};

#[allow(unused_imports)] // Preserve the existing public portal constants for callers.
pub use chord_platform::dbus::fd::{
    F_DUPFD_CLOEXEC, MAX_FDS_PER_MESSAGE, MSG_CTRUNC, MSG_NOSIGNAL, SCM_RIGHTS, SOL_SOCKET,
};
use chord_platform::dbus::{DbusError, fd};
use chord_platform::screencast::CapturedFrame;

#[path = "pipewire_portal/types.rs"]
mod types;
pub use types::{
    DaemonRestartReport, DmaBufNegotiationRequest, NegotiatedDmaBufConfig, PortalBackend,
    PortalError, PortalSessionConfig, PortalSessionState, PortalTelemetrySnapshot,
};

#[allow(unused_imports)]
pub use crate::video::encoder::{RawVideoFrame, VideoFramePixelFormat};
#[allow(unused_imports)]
pub use crate::video::{
    DmaBufPlaneDescriptor, FrameBridgeError, PipeWireFrameBridge, PipeWireFrameBridgeStats,
    convert_captured_frame_ref_to_raw_frame, convert_captured_frame_to_raw_frame,
    convert_dma_buf_plane_to_raw_frame, map_drm_fourcc_to_pixel_format,
    swizzle_bgra_to_rgba_in_place,
};

pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
pub const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
pub const SOURCE_TYPE_MONITOR: u32 = 1;
pub const SOURCE_TYPE_WINDOW: u32 = 2;
pub const SOURCE_TYPE_VIRTUAL: u32 = 4;
pub const SOURCE_TYPE_ALL_MASK: u32 = 7;

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

static TOTAL_FDS_OPENED: AtomicU64 = AtomicU64::new(0);
static TOTAL_FDS_CLOSED: AtomicU64 = AtomicU64::new(0);

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
        fd::close_fd(raw);
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
        let duplicate = fd::duplicate_cloexec(self.raw)
            .map_err(|error| PortalError::IoError(error.to_string()))?;
        Self::from_raw_fd_checked(duplicate)
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

pub fn send_fds_scm_rights(
    stream: &UnixStream,
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
    if let Some(&raw) = fds.iter().find(|&&raw| raw < 0) {
        return Err(PortalError::InvalidFd(raw));
    }
    fd::send_with_fds(stream, payload, fds).map_err(|error| match error {
        DbusError::Io(error) => PortalError::IoError(error.to_string()),
        other => PortalError::IoError(format!("{other:?}")),
    })
}

pub fn recv_fds_scm_rights(
    stream: &UnixStream,
    buffer: &mut [u8],
) -> Result<(usize, Vec<OwnedPortalFd>), PortalError> {
    let mut received = Vec::new();
    let bytes = fd::recv_with_fds(stream, buffer, &mut received).map_err(portal_receive_error)?;
    // The shared transport validates every raw descriptor before transferring it.
    let owned = received
        .into_iter()
        .map(|raw| {
            TOTAL_FDS_OPENED.fetch_add(1, Ordering::SeqCst);
            OwnedPortalFd { raw }
        })
        .collect();
    Ok((bytes, owned))
}

fn portal_receive_error(error: DbusError) -> PortalError {
    match error {
        DbusError::Io(error)
            if matches!(
                error.kind(),
                ErrorKind::ConnectionReset | ErrorKind::BrokenPipe
            ) =>
        {
            PortalError::PipeWireDaemonDisconnected(error.to_string())
        }
        DbusError::Io(error) => PortalError::IoError(error.to_string()),
        DbusError::Protocol("control data truncated") => PortalError::ControlDataTruncated,
        DbusError::Protocol(_) => PortalError::MalformedAncillaryData,
        other => PortalError::IoError(format!("{other:?}")),
    }
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

pub fn negotiated_dma_buf_to_raw_frame(
    config: &NegotiatedDmaBufConfig,
    width: u32,
    height: u32,
    dma_buf_fd: Option<RawFd>,
    plane_bytes: &[u8],
    pts_us: u64,
) -> Result<RawVideoFrame, FrameBridgeError> {
    if config.is_zero_copy {
        let fd = dma_buf_fd.unwrap_or(-1);
        let plane = DmaBufPlaneDescriptor {
            dma_buf_fd: fd,
            offset_bytes: 0,
            stride_bytes: config.stride_bytes,
            size_bytes: plane_bytes.len(),
            drm_fourcc: config.drm_fourcc,
            drm_modifier: config.drm_modifier,
        };
        convert_dma_buf_plane_to_raw_frame(width, height, &plane, plane_bytes, pts_us)
    } else {
        let frame = CapturedFrame::new(width, height, config.stride_bytes, config.drm_fourcc)
            .with_data(plane_bytes.to_vec())
            .with_timestamp_us(pts_us);
        convert_captured_frame_to_raw_frame(frame)
    }
}

pub struct PipeWirePortalManager<B: PortalBackend> {
    backend: B,
    state: PortalSessionState,
    config: Option<PortalSessionConfig>,
    negotiated_config: Option<NegotiatedDmaBufConfig>,
    session_handle: Option<String>,
    active_fd: Option<OwnedPortalFd>,
    active_node_id: Option<u32>,
    saved_restore_token: Option<String>,
    frame_bridge: PipeWireFrameBridge,
    manager_fds_opened: u64,
    manager_fds_closed: u64,
}

impl<B: PortalBackend> PipeWirePortalManager<B> {
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self {
            backend,
            state: PortalSessionState::Unconnected,
            config: None,
            negotiated_config: None,
            session_handle: None,
            active_fd: None,
            active_node_id: None,
            saved_restore_token: None,
            frame_bridge: PipeWireFrameBridge::new(),
            manager_fds_opened: 0,
            manager_fds_closed: 0,
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
        self.manager_fds_opened += 1;
        let negotiated = match negotiate_dma_buf_stream(dma_req) {
            Ok(neg) => neg,
            Err(err) => {
                drop(fd);
                self.manager_fds_closed += 1;
                return self.fail(err);
            }
        };
        if let Some(tok) = r_tok.or_else(|| cfg.restore_token.clone()) {
            self.saved_restore_token = Some(tok);
        }
        self.config = Some(cfg.clone());
        self.negotiated_config = Some(negotiated.clone());
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
        self.manager_fds_closed += 1;
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
            self.manager_fds_opened += 1;
            let negotiated = match negotiate_dma_buf_stream(dma_req) {
                Ok(neg) => neg,
                Err(err) => {
                    drop(new_owned);
                    self.manager_fds_closed += 1;
                    return Err(err);
                }
            };
            let new_fd = new_owned.as_raw_fd();
            self.negotiated_config = Some(negotiated.clone());
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
        self.recreate_session_after_restart(dma_req, closed_old_fd)
    }

    fn recreate_session_after_restart(
        &mut self,
        dma_req: &DmaBufNegotiationRequest,
        closed_old_fd: Option<RawFd>,
    ) -> Result<DaemonRestartReport, PortalError> {
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

    pub fn convert_frame_for_encoder(
        &mut self,
        width: u32,
        height: u32,
        plane_bytes: &[u8],
        pts_us: u64,
    ) -> Result<RawVideoFrame, PortalError> {
        if self.state != PortalSessionState::Streaming {
            return Err(PortalError::InvalidSessionState("Streaming", self.state));
        }
        let cfg = self
            .negotiated_config
            .as_ref()
            .ok_or(PortalError::NoCompatibleDmaBufModifier)?;
        let active_fd = self.active_fd.as_ref().map(OwnedPortalFd::as_raw_fd);
        let raw =
            negotiated_dma_buf_to_raw_frame(cfg, width, height, active_fd, plane_bytes, pts_us)
                .map_err(|e| PortalError::FrameBridge(e.to_string()))?;
        let dummy = CapturedFrame::new(width, height, cfg.stride_bytes, cfg.drm_fourcc)
            .with_data(plane_bytes.to_vec())
            .with_timestamp_us(pts_us);
        let _ = self
            .frame_bridge
            .convert_captured_frame_reusing_buffer(&dummy);
        Ok(raw)
    }

    #[must_use]
    pub const fn frame_bridge_stats(&self) -> &PipeWireFrameBridgeStats {
        self.frame_bridge.stats()
    }

    #[must_use]
    pub fn telemetry_snapshot(&self) -> PortalTelemetrySnapshot {
        let raw_tok = self.saved_restore_token.as_deref().unwrap_or("");
        PortalTelemetrySnapshot {
            state: self.state,
            active_node_id: self.active_node_id,
            active_fd: self.active_fd.as_ref().map(OwnedPortalFd::as_raw_fd),
            redacted_restore_token: redact_restore_token(raw_tok),
            total_fds_opened: self.manager_fds_opened,
            total_fds_closed: self.manager_fds_closed,
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

        let source = std::fs::File::open("/dev/null").expect("source descriptor");
        let excess = vec![source.as_raw_fd(); MAX_FDS_PER_MESSAGE + 4];
        fd::send_with_fds(&tx, b"x", &excess).expect("send truncated control data");
        let trunc = recv_fds_scm_rights(&rx, &mut [0u8; 8]);
        assert_eq!(trunc, Err(PortalError::ControlDataTruncated));
        let bad_cmsg: Result<Vec<OwnedPortalFd>, _> = Err(portal_receive_error(
            DbusError::Protocol("malformed ancillary data"),
        ));
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
