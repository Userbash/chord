#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Интерактивный круглый радар участников голосового канала `SpatialRadar` (`TASK-1017B`).
//! Оракулы: `waves/kanban-board.json` (`TASK-1017B`, `TASK-1017A`, `TASK-986`),
//! `/home/sanya/re-deep/voice/ui/01-candidates.json` (`voiceUser__07f91 clickable__07f91 userSmall__07f91`,
//! `voiceChannelEffectsContainer__2f4f7`, `rtcConnectionStatusWrapper__06d62`),
//! `/home/sanya/re-deep/voice/ui/02-requiremodule-voice.json` (`isSpatialAudioEnabled`, `setOutputVolume`),
//! `/home/sanya/re-deep/frontend-d03.json` (`avatar_fdd9da`, `tooltip_c36707`).

use std::collections::BTreeMap;
use std::fmt;

pub const RADAR_CONTAINER: &str = "Spatial Radar Canvas";
pub const RADAR_DIMENSIONS: &str = "240px circle";
pub const RADAR_DIAMETER_PX: u32 = 240;
pub const RADAR_PADDING_PX: u32 = 8;
pub const RADAR_SPACING_PX: u32 = 0;
pub const RADAR_USABLE_RADIUS_PX: f32 = 112.0;
pub const RADAR_CENTER_X_PX: f32 = 120.0;
pub const RADAR_CENTER_Y_PX: f32 = 120.0;
pub const RADAR_AVATAR_SIZE_PX: u32 = 32;
pub const RADAR_AVATAR_HIT_RADIUS_PX: f32 = 16.0;

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const RADAR_AVATAR_ELEMENT: &str = "radar-avatar";

pub const RE_VOICE_USER_CLASS: &str = "voiceUser__07f91 clickable__07f91 userSmall__07f91";
pub const RE_VOICE_EFFECTS_CONTAINER_CLASS: &str = "voiceChannelEffectsContainer__2f4f7";
pub const RE_RTC_STATUS_WRAPPER_CLASS: &str = "rtcConnectionStatusWrapper__06d62";
pub const RE_AVATAR_CLASS: &str = "avatar_fdd9da";
pub const RE_TOOLTIP_CLASS: &str = "tooltip_c36707";
pub const NAPI_IS_SPATIAL_AUDIO_ENABLED: &str = "isSpatialAudioEnabled";

pub const MIN_AZIMUTH_DEG: f32 = -180.0;
pub const MAX_AZIMUTH_DEG: f32 = 180.0;
pub const MIN_DISTANCE_M: f32 = 1.0;
pub const MAX_DISTANCE_M: f32 = 50.0;
pub const DEFAULT_SNAP_STEP_DEG: f32 = 5.0;
pub const MAX_RADAR_PARTICIPANTS: usize = 64;

pub const SQLITE_PLAIN_MAGIC: &[u8; 16] = b"SQLite format 3\0";
pub const SQLITE_SCHEMA_VERSION: u32 = 1;
pub const SQL_UPSERT_RADAR_POSITION: &str = "INSERT INTO spatial_radar_positions (channel_id, user_id, azimuth_deg, distance_m, pinned_azimuth) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(channel_id, user_id) DO UPDATE SET azimuth_deg = excluded.azimuth_deg, distance_m = excluded.distance_m, pinned_azimuth = excluded.pinned_azimuth;";

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarAvatarInteractiveState {
    Positioned,
    DragIcon,
    Dragging,
}

impl RadarAvatarInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Positioned => "positioned",
            Self::DragIcon => "drag icon",
            Self::Dragging => "dragging",
        }
    }

    #[must_use]
    pub const fn resolve(dragging: bool, hovered: bool) -> Self {
        if dragging {
            Self::Dragging
        } else if hovered {
            Self::DragIcon
        } else {
            Self::Positioned
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpatialRadarError {
    InvalidSnowflake(String),
    InvalidCoordinate(&'static str),
    InvalidAzimuth(f32),
    InvalidDistance(f32),
    InvalidSnapStep(f32),
    ParticipantNotFound(u64),
    ParticipantLimitExceeded(usize),
    NoActiveDragSession,
    SqliteNulByteRejected,
    SqliteCorruptedImage(&'static str),
    SqliteChecksumMismatch { expected: u64, actual: u64 },
    MissingPermission(&'static str),
    AgeGatedSpaceRestricted,
    BoundaryViolation(&'static str),
    WarmupPending { retry_after_ms: u64 },
    HttpError { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for SpatialRadarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SpatialRadarError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, SpatialRadarError> {
    let s = raw.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SpatialRadarError::InvalidSnowflake(raw.to_string()));
    }
    let val = s
        .parse::<u64>()
        .map_err(|_| SpatialRadarError::InvalidSnowflake(raw.to_string()))?;
    if val == 0 {
        return Err(SpatialRadarError::InvalidSnowflake(raw.to_string()));
    }
    Ok(val)
}

#[must_use]
pub fn normalize_azimuth_deg(azimuth_deg: f32) -> f32 {
    let mut norm = ((azimuth_deg + 180.0) % 360.0 + 360.0) % 360.0 - 180.0;
    if (norm - (-180.0)).abs() < 1e-5 && azimuth_deg > 0.0 {
        norm = 180.0;
    }
    norm
}

pub fn snap_azimuth_deg(
    azimuth_deg: f32,
    snap_step_deg: Option<f32>,
) -> Result<f32, SpatialRadarError> {
    if !azimuth_deg.is_finite() {
        return Err(SpatialRadarError::InvalidAzimuth(azimuth_deg));
    }
    if let Some(step) = snap_step_deg {
        if !step.is_finite() || step <= 0.0 || step > 180.0 {
            return Err(SpatialRadarError::InvalidSnapStep(step));
        }
        let snapped = (azimuth_deg / step).round() * step;
        return Ok(normalize_azimuth_deg(snapped));
    }
    Ok(normalize_azimuth_deg(azimuth_deg))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarPolarPoint {
    pub azimuth_deg: f32,
    pub distance_m: f32,
    pub clamped_radius_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarCanvasPoint {
    pub x_px: f32,
    pub y_px: f32,
    pub radius_px: f32,
}

pub fn polar_to_radar_xy(
    azimuth_deg: f32,
    distance_m: f32,
) -> Result<RadarCanvasPoint, SpatialRadarError> {
    if !azimuth_deg.is_finite() {
        return Err(SpatialRadarError::InvalidAzimuth(azimuth_deg));
    }
    if !distance_m.is_finite() || !(0.0..=MAX_DISTANCE_M * 4.0).contains(&distance_m) {
        return Err(SpatialRadarError::InvalidDistance(distance_m));
    }
    let norm_az = normalize_azimuth_deg(azimuth_deg);
    let clamped_dist = distance_m.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M);
    let norm_r = (clamped_dist - MIN_DISTANCE_M) / (MAX_DISTANCE_M - MIN_DISTANCE_M);
    let r_px = norm_r * RADAR_USABLE_RADIUS_PX;
    let az_rad = norm_az.to_radians();
    let x_px = RADAR_CENTER_X_PX + r_px * az_rad.sin();
    let y_px = RADAR_CENTER_Y_PX - r_px * az_rad.cos();
    Ok(RadarCanvasPoint {
        x_px,
        y_px,
        radius_px: r_px,
    })
}

pub fn radar_xy_to_polar(
    x_px: f32,
    y_px: f32,
    snap_step_deg: Option<f32>,
) -> Result<RadarPolarPoint, SpatialRadarError> {
    if !x_px.is_finite() || !y_px.is_finite() {
        return Err(SpatialRadarError::InvalidCoordinate("non_finite_xy"));
    }
    let (dx, dy) = (x_px - RADAR_CENTER_X_PX, RADAR_CENTER_Y_PX - y_px);
    let raw_r = dx.hypot(dy);
    let clamped_radius_px = raw_r.min(RADAR_USABLE_RADIUS_PX);
    if raw_r < 1e-4 {
        let _ = snap_azimuth_deg(0.0, snap_step_deg)?;
        return Ok(RadarPolarPoint {
            azimuth_deg: 0.0,
            distance_m: MIN_DISTANCE_M,
            clamped_radius_px: 0.0,
        });
    }
    let mut azimuth_deg = snap_azimuth_deg(dx.atan2(dy).to_degrees(), snap_step_deg)?;
    if (azimuth_deg - (-180.0)).abs() < 1e-5 && dx >= 0.0 {
        azimuth_deg = 180.0;
    }
    let ratio = clamped_radius_px / RADAR_USABLE_RADIUS_PX;
    let distance_m = MIN_DISTANCE_M + ratio * (MAX_DISTANCE_M - MIN_DISTANCE_M);
    Ok(RadarPolarPoint {
        azimuth_deg,
        distance_m,
        clamped_radius_px,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqliteRadarRow {
    pub channel_id: u64,
    pub user_id: u64,
    pub azimuth_deg: f32,
    pub distance_m: f32,
    pub pinned_azimuth: bool,
}

impl SqliteRadarRow {
    pub fn validate(&self) -> Result<(), SpatialRadarError> {
        if self.channel_id == 0 || self.user_id == 0 {
            return Err(SpatialRadarError::InvalidSnowflake("0".to_string()));
        }
        if !(MIN_AZIMUTH_DEG..=MAX_AZIMUTH_DEG).contains(&self.azimuth_deg) {
            return Err(SpatialRadarError::InvalidAzimuth(self.azimuth_deg));
        }
        if !(MIN_DISTANCE_M..=MAX_DISTANCE_M).contains(&self.distance_m) {
            return Err(SpatialRadarError::InvalidDistance(self.distance_m));
        }
        Ok(())
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[rustfmt::skip]
fn read_u64_be(b: &[u8], i: usize) -> u64 {
    u64::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3], b[i + 4], b[i + 5], b[i + 6], b[i + 7]])
}

#[rustfmt::skip]
fn read_f32_be(b: &[u8], i: usize) -> f32 {
    f32::from_bits(u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]))
}

fn encode_row_bytes(row: &SqliteRadarRow, out: &mut Vec<u8>) {
    out.extend_from_slice(&row.channel_id.to_be_bytes());
    out.extend_from_slice(&row.user_id.to_be_bytes());
    out.extend_from_slice(&row.azimuth_deg.to_bits().to_be_bytes());
    out.extend_from_slice(&row.distance_m.to_bits().to_be_bytes());
    out.push(u8::from(row.pinned_azimuth));
}

fn decode_row_bytes(chunk: &[u8]) -> Result<SqliteRadarRow, SpatialRadarError> {
    if chunk.len() != 25 {
        return Err(SpatialRadarError::SqliteCorruptedImage(
            "invalid_row_stride",
        ));
    }
    let row = SqliteRadarRow {
        channel_id: read_u64_be(chunk, 0),
        user_id: read_u64_be(chunk, 8),
        azimuth_deg: read_f32_be(chunk, 16),
        distance_m: read_f32_be(chunk, 20),
        pinned_azimuth: chunk[24] != 0,
    };
    row.validate()?;
    Ok(row)
}

#[derive(Debug, Clone, Default)]
pub struct SpatialRadarSqliteStore {
    rows: BTreeMap<(u64, u64), SqliteRadarRow>,
    pub schema_version: u32,
}

impl SpatialRadarSqliteStore {
    #[must_use]
    pub fn new_in_memory() -> Self {
        Self {
            rows: BTreeMap::new(),
            schema_version: SQLITE_SCHEMA_VERSION,
        }
    }

    pub fn validate_sql_param_text(text: &str) -> Result<(), SpatialRadarError> {
        if text.contains('\0') {
            return Err(SpatialRadarError::SqliteNulByteRejected);
        }
        Ok(())
    }

    pub fn upsert_position(&mut self, row: &SqliteRadarRow) -> Result<(), SpatialRadarError> {
        row.validate()?;
        self.rows.insert((row.channel_id, row.user_id), row.clone());
        Ok(())
    }

    #[must_use]
    pub fn get_position(&self, channel_id: u64, user_id: u64) -> Option<&SqliteRadarRow> {
        self.rows.get(&(channel_id, user_id))
    }

    #[must_use]
    pub fn list_channel_positions(&self, channel_id: u64) -> Vec<SqliteRadarRow> {
        self.rows
            .range((channel_id, 0)..=(channel_id, u64::MAX))
            .map(|(_, row)| row.clone())
            .collect()
    }

    #[must_use]
    pub fn export_sqlite_image(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(self.rows.len() * 25);
        for row in self.rows.values() {
            encode_row_bytes(row, &mut payload);
        }
        let checksum = fnv1a64(&payload);
        let count = self.rows.len() as u32;
        let mut image = Vec::with_capacity(32 + payload.len());
        image.extend_from_slice(SQLITE_PLAIN_MAGIC);
        image.extend_from_slice(&self.schema_version.to_be_bytes());
        image.extend_from_slice(&count.to_be_bytes());
        image.extend_from_slice(&checksum.to_be_bytes());
        image.extend_from_slice(&payload);
        image
    }

    pub fn import_sqlite_image(bytes: &[u8]) -> Result<Self, SpatialRadarError> {
        if bytes.len() < 32 || &bytes[0..16] != SQLITE_PLAIN_MAGIC {
            return Err(SpatialRadarError::SqliteCorruptedImage("bad_sqlite_magic"));
        }
        let version = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        if version != SQLITE_SCHEMA_VERSION {
            return Err(SpatialRadarError::SqliteCorruptedImage(
                "unsupported_version",
            ));
        }
        let count = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
        let expected_crc = read_u64_be(bytes, 24);
        let body = &bytes[32..];
        if body.len() != count * 25 {
            return Err(SpatialRadarError::SqliteCorruptedImage("truncated_rows"));
        }
        let actual_crc = fnv1a64(body);
        if actual_crc != expected_crc {
            return Err(SpatialRadarError::SqliteChecksumMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }
        let mut store = Self::new_in_memory();
        for chunk in body.chunks_exact(25) {
            let row = decode_row_bytes(chunk)?;
            store.rows.insert((row.channel_id, row.user_id), row);
        }
        Ok(store)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarParticipantNode {
    pub user_id: u64,
    pub display_name: String,
    pub azimuth_deg: f32,
    pub distance_m: f32,
    pub hovered: bool,
    pub dragging: bool,
    pub pinned_azimuth: bool,
}

impl RadarParticipantNode {
    pub fn new(
        user_id: u64,
        display_name: &str,
        azimuth_deg: f32,
        distance_m: f32,
    ) -> Result<Self, SpatialRadarError> {
        if user_id == 0 {
            return Err(SpatialRadarError::InvalidSnowflake("0".to_string()));
        }
        SpatialRadarSqliteStore::validate_sql_param_text(display_name)?;
        if display_name.trim().is_empty() {
            return Err(SpatialRadarError::BoundaryViolation("empty_display_name"));
        }
        let _ = polar_to_radar_xy(azimuth_deg, distance_m)?;
        Ok(Self {
            user_id,
            display_name: display_name.trim().to_string(),
            azimuth_deg: normalize_azimuth_deg(azimuth_deg),
            distance_m: distance_m.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M),
            hovered: false,
            dragging: false,
            pinned_azimuth: false,
        })
    }

    #[must_use]
    pub const fn interactive_state(&self) -> RadarAvatarInteractiveState {
        RadarAvatarInteractiveState::resolve(self.dragging, self.hovered)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WgpuRadarAvatarQuad {
    pub user_id: u64,
    pub center_x_px: f32,
    pub center_y_px: f32,
    pub interactive_state: RadarAvatarInteractiveState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WgpuRadarSceneSpec {
    pub container: &'static str,
    pub dimensions: &'static str,
    pub avatars: Vec<WgpuRadarAvatarQuad>,
}

#[derive(Debug, Clone)]
pub struct SpatialRadarCanvas {
    pub channel_id: u64,
    pub participants: BTreeMap<u64, RadarParticipantNode>,
    pub active_drag_user_id: Option<u64>,
    pub snap_step_deg: Option<f32>,
}

impl SpatialRadarCanvas {
    pub fn new(channel_id: u64) -> Result<Self, SpatialRadarError> {
        if channel_id == 0 {
            return Err(SpatialRadarError::InvalidSnowflake("0".to_string()));
        }
        Ok(Self {
            channel_id,
            participants: BTreeMap::new(),
            active_drag_user_id: None,
            snap_step_deg: Some(DEFAULT_SNAP_STEP_DEG),
        })
    }

    pub fn upsert_participant(
        &mut self,
        node: RadarParticipantNode,
    ) -> Result<(), SpatialRadarError> {
        if !self.participants.contains_key(&node.user_id)
            && self.participants.len() >= MAX_RADAR_PARTICIPANTS
        {
            return Err(SpatialRadarError::ParticipantLimitExceeded(
                MAX_RADAR_PARTICIPANTS,
            ));
        }
        self.participants.insert(node.user_id, node);
        Ok(())
    }

    pub fn hydrate_from_sqlite(
        &mut self,
        store: &SpatialRadarSqliteStore,
    ) -> Result<usize, SpatialRadarError> {
        let mut restored = 0usize;
        for row in store.list_channel_positions(self.channel_id) {
            if let Some(peer) = self.participants.get_mut(&row.user_id) {
                peer.azimuth_deg = row.azimuth_deg;
                peer.distance_m = row.distance_m;
                peer.pinned_azimuth = row.pinned_azimuth;
                restored += 1;
            }
        }
        Ok(restored)
    }

    #[must_use]
    pub fn hit_test_avatar(&self, x_px: f32, y_px: f32) -> Option<u64> {
        let mut best: Option<(u64, f32)> = None;
        for peer in self.participants.values() {
            if let Ok(pt) = polar_to_radar_xy(peer.azimuth_deg, peer.distance_m) {
                let dist = (x_px - pt.x_px).hypot(y_px - pt.y_px);
                if dist <= RADAR_AVATAR_HIT_RADIUS_PX
                    && best.is_none_or(|(_, best_d)| dist < best_d)
                {
                    best = Some((peer.user_id, dist));
                }
            }
        }
        best.map(|(uid, _)| uid)
    }

    pub fn update_hover(&mut self, x_px: f32, y_px: f32) -> Option<u64> {
        let hit = self.hit_test_avatar(x_px, y_px);
        for peer in self.participants.values_mut() {
            peer.hovered = Some(peer.user_id) == hit;
        }
        hit
    }

    pub fn begin_drag_at(&mut self, x_px: f32, y_px: f32) -> Result<u64, SpatialRadarError> {
        let uid = self
            .hit_test_avatar(x_px, y_px)
            .ok_or(SpatialRadarError::NoActiveDragSession)?;
        for peer in self.participants.values_mut() {
            peer.dragging = peer.user_id == uid;
            peer.hovered = peer.user_id == uid;
        }
        self.active_drag_user_id = Some(uid);
        Ok(uid)
    }

    pub fn drag_move_to(
        &mut self,
        x_px: f32,
        y_px: f32,
    ) -> Result<RadarPolarPoint, SpatialRadarError> {
        let uid = self
            .active_drag_user_id
            .ok_or(SpatialRadarError::NoActiveDragSession)?;
        let polar = radar_xy_to_polar(x_px, y_px, self.snap_step_deg)?;
        let peer = self
            .participants
            .get_mut(&uid)
            .ok_or(SpatialRadarError::ParticipantNotFound(uid))?;
        if !peer.pinned_azimuth {
            peer.azimuth_deg = polar.azimuth_deg;
        }
        peer.distance_m = polar.distance_m;
        Ok(RadarPolarPoint {
            azimuth_deg: peer.azimuth_deg,
            distance_m: peer.distance_m,
            clamped_radius_px: polar.clamped_radius_px,
        })
    }

    pub fn end_drag_and_persist(
        &mut self,
        store: &mut SpatialRadarSqliteStore,
        lock_azimuth: bool,
    ) -> Result<SqliteRadarRow, SpatialRadarError> {
        let uid = self
            .active_drag_user_id
            .take()
            .ok_or(SpatialRadarError::NoActiveDragSession)?;
        let peer = self
            .participants
            .get_mut(&uid)
            .ok_or(SpatialRadarError::ParticipantNotFound(uid))?;
        peer.dragging = false;
        peer.pinned_azimuth = lock_azimuth;
        let row = SqliteRadarRow {
            channel_id: self.channel_id,
            user_id: peer.user_id,
            azimuth_deg: peer.azimuth_deg,
            distance_m: peer.distance_m,
            pinned_azimuth: peer.pinned_azimuth,
        };
        store.upsert_position(&row)?;
        Ok(row)
    }
}

#[must_use]
pub fn format_badge_cap(count: usize) -> String {
    if count as u32 > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

pub fn build_wgpu_radar_scene(
    canvas: &SpatialRadarCanvas,
) -> Result<WgpuRadarSceneSpec, SpatialRadarError> {
    let mut avatars = Vec::with_capacity(canvas.participants.len());
    for peer in canvas.participants.values() {
        let pt = polar_to_radar_xy(peer.azimuth_deg, peer.distance_m)?;
        avatars.push(WgpuRadarAvatarQuad {
            user_id: peer.user_id,
            center_x_px: pt.x_px,
            center_y_px: pt.y_px,
            interactive_state: peer.interactive_state(),
        });
    }
    Ok(WgpuRadarSceneSpec {
        container: RADAR_CONTAINER,
        dimensions: RADAR_DIMENSIONS,
        avatars,
    })
}

pub fn validate_voice_radar_access(
    permissions: u64,
    age_gated: bool,
    age_verified: bool,
) -> Result<(), SpatialRadarError> {
    let req = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
    if (permissions & req) != req {
        return Err(SpatialRadarError::MissingPermission("VOICE_RADAR"));
    }
    if age_gated && !age_verified {
        return Err(SpatialRadarError::AgeGatedSpaceRestricted);
    }
    Ok(())
}

pub fn validate_policy_boundaries(
    msg_len: usize,
    attachments: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), SpatialRadarError> {
    let ok = (1..=MAX_MESSAGE_CHARS).contains(&msg_len)
        && attachments <= MAX_ATTACHMENTS_COUNT
        && attachment_bytes <= MAX_ATTACHMENT_BYTES
        && search_offset <= MAX_SEARCH_OFFSET
        && volume_pct <= MAX_VOLUME_PERCENT;
    if !ok {
        return Err(SpatialRadarError::BoundaryViolation("policy_limit"));
    }
    Ok(())
}

pub fn classify_http_status(status: u16, retry_after_ms: u64) -> Result<(), SpatialRadarError> {
    match status {
        200 | 204 => Ok(()),
        202 => Err(SpatialRadarError::WarmupPending {
            retry_after_ms: retry_after_ms.max(100),
        }),
        _ => Err(SpatialRadarError::HttpError {
            status,
            retry_after_ms,
        }),
    }
}

#[must_use]
pub fn redact_sensitive_log(input: &str) -> String {
    let mut out = input.to_string();
    for prefix in ["mfa.", "dQw4w9WgXcQ:", "Bearer "] {
        if let Some(idx) = out.find(prefix) {
            let tail = &out[idx + prefix.len()..];
            let end = tail
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .unwrap_or(tail.len());
            out.replace_range(idx..idx + prefix.len() + end, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1017b_1_tokens_states_and_re_oracles() {
        assert_eq!(RADAR_CONTAINER, "Spatial Radar Canvas");
        assert_eq!(RADAR_DIMENSIONS, "240px circle");
        assert_eq!(
            (RADAR_DIAMETER_PX, RADAR_PADDING_PX, RADAR_SPACING_PX),
            (240, 8, 0)
        );
        assert_eq!(RADAR_USABLE_RADIUS_PX, 112.0);
        assert_eq!((TOKEN_BACKGROUND, TOKEN_SURFACE), ("#1e1f22", "#2b2d31"));
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_ACCENT), ("#ffffff", "#5865f2"));
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(
            RadarAvatarInteractiveState::resolve(false, false).as_str(),
            "positioned"
        );
        assert_eq!(
            RadarAvatarInteractiveState::resolve(false, true).as_str(),
            "drag icon"
        );
        assert_eq!(
            RadarAvatarInteractiveState::resolve(true, true).as_str(),
            "dragging"
        );
        assert!(RE_VOICE_USER_CLASS.contains("voiceUser__07f91"));
        assert_eq!(NAPI_IS_SPATIAL_AUDIO_ENABLED, "isSpatialAudioEnabled");
    }

    #[test]
    fn test_sub_1017b_2_wgpu_radar_polar_math_and_mouse_drag() {
        let top = polar_to_radar_xy(0.0, 50.0).unwrap();
        assert!((top.x_px - 120.0).abs() < 1e-4 && (top.y_px - 8.0).abs() < 1e-4);
        let right = polar_to_radar_xy(90.0, 50.0).unwrap();
        assert!((right.x_px - 232.0).abs() < 1e-4 && (right.y_px - 120.0).abs() < 1e-4);

        let mut store = SpatialRadarSqliteStore::new_in_memory();
        let mut canvas = SpatialRadarCanvas::new(1058348665342808104).unwrap();
        canvas.snap_step_deg = Some(15.0);
        canvas
            .upsert_participant(RadarParticipantNode::new(42, "ShadowByte", 0.0, 50.0).unwrap())
            .unwrap();
        assert_eq!(canvas.update_hover(120.0, 8.0), Some(42));
        assert_eq!(canvas.begin_drag_at(120.0, 8.0).unwrap(), 42);
        assert_eq!(
            canvas.participants[&42].interactive_state().as_str(),
            "dragging"
        );

        let moved = canvas.drag_move_to(200.0, 42.0).unwrap();
        assert!((moved.azimuth_deg - 45.0).abs() < 1e-4);
        let saved = canvas.end_drag_and_persist(&mut store, true).unwrap();
        assert!(saved.pinned_azimuth && (saved.azimuth_deg - 45.0).abs() < 1e-4);
        assert_eq!(build_wgpu_radar_scene(&canvas).unwrap().avatars.len(), 1);
    }

    #[test]
    fn test_sub_1017b_3_sqlite_upsert_and_image_persistence() {
        let mut store = SpatialRadarSqliteStore::new_in_memory();
        let mut canvas = SpatialRadarCanvas::new(777).unwrap();
        canvas
            .upsert_participant(RadarParticipantNode::new(999, "PeerOne", -30.0, 25.5).unwrap())
            .unwrap();
        let pt = polar_to_radar_xy(-30.0, 25.5).unwrap();
        canvas.begin_drag_at(pt.x_px, pt.y_px).unwrap();
        canvas.drag_move_to(64.0, 120.0).unwrap();
        canvas.end_drag_and_persist(&mut store, true).unwrap();

        let stored = store.get_position(777, 999).unwrap().clone();
        assert!(
            (stored.azimuth_deg - (-90.0)).abs() < 1e-4 && (stored.distance_m - 25.5).abs() < 1e-3
        );
        let img = store.export_sqlite_image();
        assert_eq!(&img[0..16], SQLITE_PLAIN_MAGIC);
        let decoded = SpatialRadarSqliteStore::import_sqlite_image(&img).unwrap();
        assert_eq!(decoded.get_position(777, 999), Some(&stored));

        let mut fresh = SpatialRadarCanvas::new(777).unwrap();
        fresh
            .upsert_participant(RadarParticipantNode::new(999, "PeerOne", 0.0, 1.0).unwrap())
            .unwrap();
        assert_eq!(fresh.hydrate_from_sqlite(&decoded).unwrap(), 1);
        assert!((fresh.participants[&999].azimuth_deg - (-90.0)).abs() < 1e-4);
    }

    #[test]
    fn test_sub_1017b_4_nine_dimensions_and_security() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(
            validate_snowflake("18446744073709551615").unwrap(),
            u64::MAX
        );
        assert!(validate_policy_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_policy_boundaries(2_001, 10, 100, 0, 100).is_err());
        assert!(validate_policy_boundaries(10, 11, 100, 0, 100).is_err());
        assert!(validate_policy_boundaries(10, 1, 25 * 1024 * 1024 + 1, 0, 100).is_err());
        assert!(validate_policy_boundaries(10, 1, 100, 9_976, 100).is_err());
        assert!(validate_policy_boundaries(10, 1, 100, 0, 201).is_err());
        assert_eq!(format_badge_cap(99), "99");
        assert_eq!(format_badge_cap(100), "99+");
        let full_perms = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
        assert!(validate_voice_radar_access(full_perms, true, true).is_ok());
        assert!(validate_voice_radar_access(full_perms, true, false).is_err());
        assert!(validate_voice_radar_access(PERM_VIEW_CHANNEL, false, false).is_err());
        assert!(classify_http_status(429, 250).is_err());
        assert!(classify_http_status(202, 300).is_err());

        let mut bad_img = SpatialRadarSqliteStore::new_in_memory().export_sqlite_image();
        bad_img[25] ^= 0xff;
        assert!(SpatialRadarSqliteStore::import_sqlite_image(&bad_img).is_err());
        assert!(SpatialRadarSqliteStore::validate_sql_param_text("bad\0name").is_err());
        let masked = redact_sensitive_log("tok=mfa.secret123 env=dQw4w9WgXcQ:abc");
        assert!(!masked.contains("secret123") && !masked.contains("dQw4w9WgXcQ:abc"));
    }
}
