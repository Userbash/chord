#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Пространственное 3D-позиционирование звука участников (HRTF) — `TASK-986`.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Instant;

pub const OPUS_SAMPLE_RATE_HZ: u32 = 48_000;
pub const PCM_FRAME_SAMPLES: usize = 960;
pub const FRAME_DURATION_MS: f32 = 20.0;
pub const FRAME_DURATION_US: f64 = 20_000.0;
pub const KEMAR_HEAD_RADIUS_M: f32 = 0.0875;
pub const SPEED_OF_SOUND_M_PER_S: f32 = 343.0;
pub const MAX_ITD_MS: f32 = 0.7;
pub const MAX_ITD_DELAY_SAMPLES: usize = 36;
pub const MAX_ILD_DB: f32 = 20.0;
pub const MIN_AZIMUTH_DEG: f32 = -180.0;
pub const MAX_AZIMUTH_DEG: f32 = 180.0;
pub const MIN_ELEVATION_DEG: f32 = -90.0;
pub const MAX_ELEVATION_DEG: f32 = 90.0;
pub const REFERENCE_DISTANCE_M: f32 = 1.0;
pub const MAX_DISTANCE_M: f32 = 50.0;
pub const AIR_ABSORPTION_PER_METER: f32 = 0.008;
pub const HRIR_TAP_COUNT: usize = 16;
pub const HISTORY_BUFFER_LEN: usize = MAX_ITD_DELAY_SAMPLES + HRIR_TAP_COUNT;
pub const BENCHMARK_PARTICIPANTS_COUNT: usize = 10;
pub const MAX_SPATIAL_PARTICIPANTS: usize = 64;
pub const MAX_SPATIAL_CPU_LOAD_PCT: f32 = 2.0;
pub const MICRO_TARGET_CPU_LOAD_PCT: f32 = 1.5;
pub const MIN_PARTICIPANT_VOLUME_PCT: u16 = 0;
pub const DEFAULT_PARTICIPANT_VOLUME_PCT: u16 = 100;
pub const MAX_PARTICIPANT_VOLUME_PCT: u16 = 200;
pub const OPUS_DTX_SILENCE_FRAME: [u8; 3] = [0xF8, 0xFF, 0xFE];
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;

pub const DISCORD_VOICE_NODE_SIZE: u64 = 126_136_024;
pub const DISCORD_VOICE_NODE_MD5: &str = "5fe80ce0f5d89fd53d30b2fb25a6cdac";
pub const DISCORD_VOICE_NODE_SHA256: &str =
    "96721305d59a4fdcecfa4eec48daabc50dba96c9213e5d3b514e9690d3b208ca";
pub const STEAM_AUDIO_SOFA_PATH: &str = "/home/discord/discord/discord_common/native/third_party/steam_audio/build/linux-x64-Release/install/mysofa/share/libmysofa/default.sofa";
pub const STEAM_AUDIO_API_BINAURAL_PATH: &str = "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/api_binaural_effect.cpp";
pub const STEAM_AUDIO_API_HRTF_PATH: &str = "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/api_hrtf.cpp";
pub const STEAM_AUDIO_DISTANCE_ATTENUATION_PATH: &str = "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/distance_attenuation.cpp";
pub const STEAM_AUDIO_OVERLAP_SAVE_PATH: &str = "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/overlap_save_convolution_effect.cpp";
pub const NAPI_IS_SPATIAL_AUDIO_ENABLED: &str = "isSpatialAudioEnabled";
pub const NAPI_SET_SPATIAL_STATUS_CB: &str = "setSpatialAudioStatusCallback";
pub const EVENT_SET_SPATIAL_STATUS_CB: &str = "SetSpatialAudioStatusCallback";

pub const DESIGN_TOKEN_BACKGROUND: &str = "#1e1f22";
pub const DESIGN_TOKEN_SURFACE: &str = "#2b2d31";
pub const DESIGN_TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const DESIGN_TOKEN_ACCENT: &str = "#5865f2";
pub const DESIGN_TOKEN_BORDER: &str = "#3f4147";
pub const LAYOUT_CONTAINER: &str = "Spatial Audio Canvas";
pub const LAYOUT_DIMENSIONS: &str = "360 deg";
pub const INTERACTIVE_ELEMENT: &str = "peer-avatar-3d";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerAvatar3dState {
    Center,
    DragPosition,
    Panned,
}

impl PeerAvatar3dState {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::DragPosition => "drag position",
            Self::Panned => "panned",
        }
    }

    #[must_use]
    pub fn resolve(azimuth_deg: f32, dragging: bool) -> Self {
        if dragging {
            Self::DragPosition
        } else if azimuth_deg.abs() > 1.0 {
            Self::Panned
        } else {
            Self::Center
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialBackendKind {
    SofaHrtf,
    KemarBinaural,
    StereoPanning,
    MonoBypass,
}

impl SpatialBackendKind {
    #[must_use]
    pub const fn select_best(sofa_loaded: bool, hrtf_enabled: bool, stereo_output: bool) -> Self {
        if !stereo_output {
            Self::MonoBypass
        } else if sofa_loaded && hrtf_enabled {
            Self::SofaHrtf
        } else if hrtf_enabled {
            Self::KemarBinaural
        } else {
            Self::StereoPanning
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpatialAudioError {
    InvalidAzimuth(f32),
    InvalidElevation(f32),
    InvalidDistance(f32),
    InvalidFrameLength { expected: usize, actual: usize },
    NonFiniteSample { index: usize },
    CpuBudgetExceeded { actual_pct: f32, max_pct: f32 },
    ParticipantLimitExceeded(usize),
    InvalidSnowflakeId,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    InvalidVolumePercent(u16),
    PermissionDenied(&'static str),
    AgeGatedSpaceRestricted,
    EngineWarmupPending { retry_after_ms: u32 },
}

impl fmt::Display for SpatialAudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SpatialAudioError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPosition3d {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub distance_m: f32,
}

impl SpatialPosition3d {
    pub fn new(az: f32, el: f32, dist: f32) -> Result<Self, SpatialAudioError> {
        if !az.is_finite() {
            return Err(SpatialAudioError::InvalidAzimuth(az));
        }
        if !el.is_finite() || !(MIN_ELEVATION_DEG..=MAX_ELEVATION_DEG).contains(&el) {
            return Err(SpatialAudioError::InvalidElevation(el));
        }
        if !dist.is_finite() || !(0.0..=MAX_DISTANCE_M).contains(&dist) {
            return Err(SpatialAudioError::InvalidDistance(dist));
        }
        Ok(Self {
            azimuth_deg: normalize_azimuth_deg(az),
            elevation_deg: el,
            distance_m: dist,
        })
    }
}

#[must_use]
pub fn normalize_azimuth_deg(azimuth_deg: f32) -> f32 {
    let mut norm = ((azimuth_deg + 180.0) % 360.0 + 360.0) % 360.0 - 180.0;
    if (norm - (-180.0)).abs() < 1e-6 && azimuth_deg > 0.0 {
        norm = 180.0;
    }
    norm
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinauralCues {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub distance_m: f32,
    pub itd_ms: f32,
    pub itd_samples: f32,
    pub ild_db: f32,
    pub distance_gain: f32,
    pub left_gain: f32,
    pub right_gain: f32,
    pub left_delay_samples: f32,
    pub right_delay_samples: f32,
    pub rear_hemisphere: bool,
}

#[must_use]
pub fn compute_binaural_cues(az: f32, el: f32, dist: f32) -> BinauralCues {
    let azimuth_deg = normalize_azimuth_deg(az);
    let elevation_deg = el.clamp(MIN_ELEVATION_DEG, MAX_ELEVATION_DEG);
    let clamped_dist = dist.clamp(REFERENCE_DISTANCE_M, MAX_DISTANCE_M);
    let sin_theta =
        (azimuth_deg.to_radians().sin() * elevation_deg.to_radians().cos()).clamp(-1.0, 1.0);
    let abs_theta = sin_theta.abs().asin();
    let sign = if sin_theta >= 0.0 { 1.0_f32 } else { -1.0_f32 };
    let head_delay = KEMAR_HEAD_RADIUS_M / SPEED_OF_SOUND_M_PER_S;
    let itd_sec = sign * head_delay * (abs_theta + sin_theta.abs());
    let itd_ms = itd_sec * 1_000.0;
    let itd_samples = itd_sec * (OPUS_SAMPLE_RATE_HZ as f32);
    let ild_db = MAX_ILD_DB * sin_theta;
    let distance_gain = REFERENCE_DISTANCE_M / clamped_dist;
    let shadow_gain = 10.0_f32.powf(-ild_db.abs() / 20.0);
    let left_gain = (if ild_db > 0.0 { shadow_gain } else { 1.0 }) * distance_gain;
    let right_gain = (if ild_db < 0.0 { shadow_gain } else { 1.0 }) * distance_gain;
    let left_delay_samples = itd_samples.max(0.0);
    let right_delay_samples = (-itd_samples).max(0.0);
    BinauralCues {
        azimuth_deg,
        elevation_deg,
        distance_m: clamped_dist,
        itd_ms,
        itd_samples,
        ild_db,
        distance_gain,
        left_gain,
        right_gain,
        left_delay_samples,
        right_delay_samples,
        rear_hemisphere: azimuth_deg.abs() > 90.0,
    }
}

#[must_use]
pub fn synthesize_kemar_hrir(
    cues: &BinauralCues,
) -> ([f32; HRIR_TAP_COUNT], [f32; HRIR_TAP_COUNT]) {
    let air = (1.0 - (cues.distance_m - REFERENCE_DISTANCE_M) * AIR_ABSORPTION_PER_METER)
        .clamp(0.55, 1.0);
    let damp = air * if cues.rear_hemisphere { 0.78 } else { 1.0 };
    (
        build_ear_hrir(cues.left_gain, cues.ild_db > 0.0, damp),
        build_ear_hrir(cues.right_gain, cues.ild_db < 0.0, damp),
    )
}

fn build_ear_hrir(target_gain: f32, is_shadowed: bool, damp: f32) -> [f32; HRIR_TAP_COUNT] {
    let alpha = if is_shadowed {
        0.42_f32
    } else {
        0.16_f32 * (2.0 - damp)
    };
    let mut raw = [0.0_f32; HRIR_TAP_COUNT];
    let (mut sum, mut p) = (0.0_f32, 1.0_f32);
    for slot in &mut raw {
        *slot = p;
        sum += p;
        p *= alpha;
    }
    let scale = if sum > 1e-9 {
        target_gain / sum
    } else {
        target_gain
    };
    raw.map(|v| v * scale)
}

#[derive(Debug, Clone)]
pub struct StereoFrame {
    pub left: [f32; PCM_FRAME_SAMPLES],
    pub right: [f32; PCM_FRAME_SAMPLES],
}

impl Default for StereoFrame {
    fn default() -> Self {
        Self {
            left: [0.0_f32; PCM_FRAME_SAMPLES],
            right: [0.0_f32; PCM_FRAME_SAMPLES],
        }
    }
}

#[derive(Debug, Clone)]
pub struct HrtfBinauralConvolver {
    position: SpatialPosition3d,
    cues: BinauralCues,
    left_hrir: [f32; HRIR_TAP_COUNT],
    right_hrir: [f32; HRIR_TAP_COUNT],
    history: [f32; HISTORY_BUFFER_LEN],
    volume_multiplier: f32,
}

impl HrtfBinauralConvolver {
    pub fn new(pos: SpatialPosition3d) -> Self {
        let cues = compute_binaural_cues(pos.azimuth_deg, pos.elevation_deg, pos.distance_m);
        let (left_hrir, right_hrir) = synthesize_kemar_hrir(&cues);
        Self {
            position: pos,
            cues,
            left_hrir,
            right_hrir,
            history: [0.0_f32; HISTORY_BUFFER_LEN],
            volume_multiplier: 1.0,
        }
    }

    pub fn set_position(&mut self, pos: SpatialPosition3d) {
        self.position = pos;
        self.cues = compute_binaural_cues(pos.azimuth_deg, pos.elevation_deg, pos.distance_m);
        (self.left_hrir, self.right_hrir) = synthesize_kemar_hrir(&self.cues);
    }

    pub fn set_volume_percent(&mut self, percent: u16) -> Result<f32, SpatialAudioError> {
        if percent > MAX_PARTICIPANT_VOLUME_PCT {
            return Err(SpatialAudioError::InvalidVolumePercent(percent));
        }
        self.volume_multiplier = f32::from(percent) / 100.0;
        Ok(self.volume_multiplier)
    }

    #[must_use]
    pub const fn cues(&self) -> BinauralCues {
        self.cues
    }

    pub fn convolve_mono_frame(&mut self, mono: &[f32]) -> Result<StereoFrame, SpatialAudioError> {
        let mut out = StereoFrame::default();
        self.convolve_accumulate_into(mono, &mut out.left, &mut out.right)?;
        Ok(out)
    }

    pub fn convolve_accumulate_into(
        &mut self,
        mono: &[f32],
        out_l: &mut [f32; PCM_FRAME_SAMPLES],
        out_r: &mut [f32; PCM_FRAME_SAMPLES],
    ) -> Result<(), SpatialAudioError> {
        if mono.len() != PCM_FRAME_SAMPLES {
            return Err(SpatialAudioError::InvalidFrameLength {
                expected: PCM_FRAME_SAMPLES,
                actual: mono.len(),
            });
        }
        if let Some(idx) = mono.iter().position(|s| !s.is_finite()) {
            return Err(SpatialAudioError::NonFiniteSample { index: idx });
        }
        let mut ext = [0.0_f32; HISTORY_BUFFER_LEN + PCM_FRAME_SAMPLES];
        ext[..HISTORY_BUFFER_LEN].copy_from_slice(&self.history);
        ext[HISTORY_BUFFER_LEN..].copy_from_slice(mono);
        self.history
            .copy_from_slice(&mono[PCM_FRAME_SAMPLES - HISTORY_BUFFER_LEN..]);
        if self.volume_multiplier > 1e-7 {
            let v = self.volume_multiplier;
            render_ear_channel(
                &ext,
                &self.left_hrir,
                self.cues.left_delay_samples,
                v,
                out_l,
            );
            render_ear_channel(
                &ext,
                &self.right_hrir,
                self.cues.right_delay_samples,
                v,
                out_r,
            );
        }
        Ok(())
    }
}

fn render_ear_channel(
    ext: &[f32; HISTORY_BUFFER_LEN + PCM_FRAME_SAMPLES],
    hrir: &[f32; HRIR_TAP_COUNT],
    delay_samples: f32,
    vol: f32,
    out: &mut [f32; PCM_FRAME_SAMPLES],
) {
    let clamped_delay = delay_samples.clamp(0.0, MAX_ITD_DELAY_SAMPLES as f32 - 1.001);
    let delay_int = clamped_delay.floor() as usize;
    let frac = clamped_delay - (delay_int as f32);
    let inv_frac = 1.0 - frac;
    for (n, out_sample) in out.iter_mut().enumerate() {
        let base_idx = HISTORY_BUFFER_LEN + n - delay_int;
        let mut acc = 0.0_f32;
        for (k, &h) in hrir.iter().enumerate() {
            let idx0 = base_idx - k;
            let delayed = ext[idx0] * inv_frac + ext[idx0 - 1] * frac;
            acc += delayed * h;
        }
        *out_sample += acc * vol;
    }
}

#[derive(Debug, Clone)]
pub struct SpatialAudioMixer {
    sources: BTreeMap<u64, HrtfBinauralConvolver>,
    backend: SpatialBackendKind,
}

impl SpatialAudioMixer {
    #[must_use]
    pub fn new(backend: SpatialBackendKind) -> Self {
        Self {
            sources: BTreeMap::new(),
            backend,
        }
    }

    #[must_use]
    pub fn participant_count(&self) -> usize {
        self.sources.len()
    }

    pub fn upsert_participant(
        &mut self,
        user_id: u64,
        pos: SpatialPosition3d,
        vol_pct: u16,
    ) -> Result<BinauralCues, SpatialAudioError> {
        if user_id == 0 || user_id == u64::MAX {
            return Err(SpatialAudioError::InvalidSnowflakeId);
        }
        if !self.sources.contains_key(&user_id) && self.sources.len() >= MAX_SPATIAL_PARTICIPANTS {
            return Err(SpatialAudioError::ParticipantLimitExceeded(
                MAX_SPATIAL_PARTICIPANTS,
            ));
        }
        let conv = self
            .sources
            .entry(user_id)
            .or_insert_with(|| HrtfBinauralConvolver::new(pos));
        conv.set_position(pos);
        conv.set_volume_percent(vol_pct)?;
        Ok(conv.cues())
    }

    pub fn remove_participant(&mut self, user_id: u64) -> bool {
        self.sources.remove(&user_id).is_some()
    }

    pub fn mix_frame(
        &mut self,
        inputs: &[(u64, &[f32])],
    ) -> Result<StereoFrame, SpatialAudioError> {
        let mut mixed = StereoFrame::default();
        for &(uid, pcm) in inputs {
            if let Some(conv) = self.sources.get_mut(&uid) {
                conv.convolve_accumulate_into(pcm, &mut mixed.left, &mut mixed.right)?;
            }
        }
        let mut i = 0;
        while i < PCM_FRAME_SAMPLES {
            mixed.left[i] = mixed.left[i].clamp(-1.0, 1.0);
            mixed.right[i] = mixed.right[i].clamp(-1.0, 1.0);
            i += 1;
        }
        Ok(mixed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialCpuBenchmarkReport {
    pub participants: usize,
    pub frames_mixed: usize,
    pub avg_frame_time_us: f64,
    pub cpu_load_pct: f32,
}

pub fn benchmark_ten_participants_cpu(
    iterations: usize,
) -> Result<SpatialCpuBenchmarkReport, SpatialAudioError> {
    let mut mixer = SpatialAudioMixer::new(SpatialBackendKind::SofaHrtf);
    let azs = [
        -150.0_f32, -110.0, -75.0, -40.0, -15.0, 15.0, 40.0, 75.0, 110.0, 150.0,
    ];
    let mut frames = [[0.0_f32; PCM_FRAME_SAMPLES]; BENCHMARK_PARTICIPANTS_COUNT];
    for (idx, &az) in azs.iter().enumerate() {
        let pos = SpatialPosition3d::new(az, 0.0, 1.5 + (idx as f32) * 0.4)?;
        mixer.upsert_participant(10_001 + (idx as u64), pos, DEFAULT_PARTICIPANT_VOLUME_PCT)?;
        for (s_idx, s) in frames[idx].iter_mut().enumerate() {
            *s = 0.25 * ((s_idx as f32) * 0.045 + (idx as f32) * 0.3).sin();
        }
    }
    let rounds = iterations.max(1);
    let mut min_total_us = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..rounds {
            let refs: [(u64, &[f32]); BENCHMARK_PARTICIPANTS_COUNT] =
                std::array::from_fn(|idx| (10_001 + (idx as u64), &frames[idx][..]));
            let mixed = mixer.mix_frame(&refs)?;
            std::hint::black_box(mixed.left[0] + mixed.right[PCM_FRAME_SAMPLES - 1]);
        }
        min_total_us = min_total_us.min(start.elapsed().as_secs_f64() * 1_000_000.0);
    }
    let avg_frame_time_us = (min_total_us / (rounds as f64)).max(1.0);
    let cpu_load_pct = ((avg_frame_time_us / FRAME_DURATION_US) * 100.0) as f32;
    validate_spatial_cpu_budget(cpu_load_pct)?;
    Ok(SpatialCpuBenchmarkReport {
        participants: BENCHMARK_PARTICIPANTS_COUNT,
        frames_mixed: rounds,
        avg_frame_time_us,
        cpu_load_pct,
    })
}

pub fn validate_spatial_cpu_budget(cpu_load_pct: f32) -> Result<(), SpatialAudioError> {
    if !cpu_load_pct.is_finite() {
        return Err(SpatialAudioError::CpuBudgetExceeded {
            actual_pct: cpu_load_pct,
            max_pct: MAX_SPATIAL_CPU_LOAD_PCT,
        });
    }
    if cpu_load_pct <= 0.0 || cpu_load_pct >= MAX_SPATIAL_CPU_LOAD_PCT {
        return Err(SpatialAudioError::CpuBudgetExceeded {
            actual_pct: cpu_load_pct,
            max_pct: MAX_SPATIAL_CPU_LOAD_PCT,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialWorkloadContext {
    pub user_id: u64,
    pub message_len: usize,
    pub attachment_count: usize,
    pub attachment_bytes: u64,
    pub search_offset: u32,
    pub volume_pct: u16,
    pub unread_count: u32,
    pub permissions: u64,
    pub age_gated_space: bool,
    pub age_verified: bool,
    pub http_status: u16,
}

pub fn validate_spatial_workload_boundaries(
    ctx: &SpatialWorkloadContext,
) -> Result<&'static str, SpatialAudioError> {
    if ctx.user_id == 0 || ctx.user_id == u64::MAX {
        return Err(SpatialAudioError::InvalidSnowflakeId);
    }
    if ctx.message_len > 2_000 {
        return Err(SpatialAudioError::MessageTooLong(ctx.message_len));
    }
    if ctx.attachment_count > 10 {
        return Err(SpatialAudioError::TooManyAttachments(ctx.attachment_count));
    }
    if ctx.attachment_bytes > 25 * 1024 * 1024 {
        return Err(SpatialAudioError::AttachmentTooLarge(ctx.attachment_bytes));
    }
    if ctx.search_offset > 9_975 {
        return Err(SpatialAudioError::SearchOffsetOutOfBounds(
            ctx.search_offset,
        ));
    }
    if ctx.volume_pct > MAX_PARTICIPANT_VOLUME_PCT {
        return Err(SpatialAudioError::InvalidVolumePercent(ctx.volume_pct));
    }
    if ctx.permissions & (PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK)
        != (PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK)
    {
        return Err(SpatialAudioError::PermissionDenied("VOICE_SPATIAL"));
    }
    if ctx.age_gated_space && !ctx.age_verified {
        return Err(SpatialAudioError::AgeGatedSpaceRestricted);
    }
    if ctx.http_status == 202 {
        return Err(SpatialAudioError::EngineWarmupPending {
            retry_after_ms: 250,
        });
    }
    Ok(if ctx.unread_count > 99 {
        "99+"
    } else {
        "exact"
    })
}

#[must_use]
pub const fn compute_spatial_rest_backoff_ms(
    status: u16,
    retry_after_ms: Option<u32>,
    attempt: u32,
) -> Option<u32> {
    let custom = match retry_after_ms {
        Some(ms) if ms > 0 => ms,
        _ => 0,
    };
    match status {
        200 | 204 => Some(0),
        202 => Some(if custom > 0 { custom } else { 250 }),
        429 => Some(if custom > 0 { custom } else { 1_000 }),
        500..=599 => Some(500_u32.saturating_mul(1_u32 << if attempt > 4 { 4 } else { attempt })),
        _ => None,
    }
}

#[must_use]
pub const fn is_opus_dtx_silence_packet(packet: &[u8]) -> bool {
    packet.is_empty()
        || matches!(packet, [0xF8, 0xFF, 0xFE])
        || (packet.len() <= 3 && packet[0] == 0)
}

#[must_use]
pub fn redact_spatial_secrets(input: &str) -> String {
    input
        .split_whitespace()
        .map(|tok| {
            if tok.contains("mfa.") {
                "mfa.***"
            } else if tok.contains("Bearer") || tok.contains("secret_key=") {
                "[REDACTED]"
            } else {
                tok
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn export_spatial_telemetry_json(
    cues: &BinauralCues,
    cpu_load_pct: f32,
    state: PeerAvatar3dState,
    raw_note: &str,
) -> String {
    format!(
        "{{\"element\":\"{}\",\"state\":\"{}\",\"azimuth_deg\":{:.2},\"itd_ms\":{:.4},\"ild_db\":{:.2},\"distance_gain\":{:.4},\"cpu_load_pct\":{:.4},\"note\":\"{}\"}}",
        INTERACTIVE_ELEMENT,
        state.as_label(),
        cues.azimuth_deg,
        cues.itd_ms,
        cues.ild_db,
        cues.distance_gain,
        cpu_load_pct,
        redact_spatial_secrets(raw_note)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_986_1_oracle_constants_and_avatar_states() {
        assert_eq!((OPUS_SAMPLE_RATE_HZ, PCM_FRAME_SAMPLES), (48_000, 960));
        assert_eq!(DISCORD_VOICE_NODE_SIZE, 126_136_024);
        assert_eq!(DISCORD_VOICE_NODE_MD5, "5fe80ce0f5d89fd53d30b2fb25a6cdac");
        assert!(STEAM_AUDIO_SOFA_PATH.ends_with("default.sofa"));
        assert!(STEAM_AUDIO_API_HRTF_PATH.ends_with("api_hrtf.cpp"));
        assert!(STEAM_AUDIO_API_BINAURAL_PATH.ends_with("api_binaural_effect.cpp"));
        assert_eq!(LAYOUT_CONTAINER, "Spatial Audio Canvas");
        assert_eq!(LAYOUT_DIMENSIONS, "360 deg");
        assert_eq!(DESIGN_TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(DESIGN_TOKEN_ACCENT, "#5865f2");
        assert_eq!(PeerAvatar3dState::resolve(0.0, false).as_label(), "center");
        assert_eq!(
            PeerAvatar3dState::resolve(15.0, true).as_label(),
            "drag position"
        );
        assert_eq!(
            PeerAvatar3dState::resolve(-45.0, false).as_label(),
            "panned"
        );
    }

    #[test]
    fn test_sub_986_2_woodworth_itd_kemar_ild_and_distance_vectors() {
        let cases = [
            (-180.0_f32, 0.0_f32, 1.0_f32),
            (-90.0, 0.0, 1.0),
            (-45.0, 0.0, 2.0),
            (0.0, 0.0, 1.0),
            (45.0, 15.0, 4.0),
            (90.0, 0.0, 1.0),
            (135.0, -20.0, 10.0),
            (180.0, 0.0, 50.0),
        ];
        let mut items = Vec::new();
        for &(az, el, dist) in &cases {
            let c = compute_binaural_cues(az, el, dist);
            assert!(c.itd_ms.abs() <= MAX_ITD_MS && c.ild_db.abs() <= MAX_ILD_DB + 1e-5);
            items.push(format!(
                "{{\"azimuth_deg\":{},\"elevation_deg\":{},\"distance_m\":{},\"itd_ms\":{:.6},\"itd_samples\":{:.6},\"ild_db\":{:.6},\"distance_gain\":{:.6},\"left_gain\":{:.6},\"right_gain\":{:.6}}}",
                az, el, dist, c.itd_ms, c.itd_samples, c.ild_db, c.distance_gain, c.left_gain, c.right_gain
            ));
        }
        println!("SPATIAL_CUES_VECTOR_JSON:[{}]", items.join(","));
        let r90 = compute_binaural_cues(90.0, 0.0, 1.0);
        let l90 = compute_binaural_cues(-90.0, 0.0, 1.0);
        assert!(r90.itd_ms > 0.64 && r90.itd_ms < 0.67 && (r90.ild_db - 20.0).abs() < 1e-4);
        assert!(r90.right_gain > r90.left_gain * 9.5 && l90.left_gain > l90.right_gain * 9.5);
        assert!((compute_binaural_cues(0.0, 0.0, 2.0).distance_gain - 0.5).abs() < 1e-6);
        assert!((compute_binaural_cues(0.0, 0.0, 10.0).distance_gain - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_sub_986_3_hrtf_convolver_directional_rms_and_delay() {
        let mut mono = [0.0_f32; PCM_FRAME_SAMPLES];
        for (i, s) in mono.iter_mut().enumerate().skip(40) {
            *s = 0.6 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 48_000.0).sin();
        }
        let pos_c = SpatialPosition3d::new(0.0, 0.0, 1.0).unwrap();
        let out_c = HrtfBinauralConvolver::new(pos_c)
            .convolve_mono_frame(&mono)
            .unwrap();
        assert!((out_c.left[100] - out_c.right[100]).abs() < 1e-6);
        let pos_r = SpatialPosition3d::new(90.0, 0.0, 1.0).unwrap();
        let out_r = HrtfBinauralConvolver::new(pos_r)
            .convolve_mono_frame(&mono)
            .unwrap();
        let rms_l: f32 = out_r.left.iter().map(|x| x * x).sum::<f32>().sqrt();
        let rms_r: f32 = out_r.right.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(rms_r > rms_l * 8.0);
        let first_r = out_r.right.iter().position(|&x| x.abs() > 0.01).unwrap();
        let first_l = out_r.left.iter().position(|&x| x.abs() > 0.005).unwrap();
        assert!(first_l >= first_r + 28);
    }

    #[test]
    fn test_sub_986_3_overlap_save_continuity_and_distance_scaling() {
        let mono: [f32; PCM_FRAME_SAMPLES] = std::array::from_fn(|i| {
            0.4 * (2.0 * std::f32::consts::PI * 4.0 * (i as f32) / 960.0).cos()
        });
        let mut near = HrtfBinauralConvolver::new(SpatialPosition3d::new(30.0, 0.0, 1.0).unwrap());
        let mut far = HrtfBinauralConvolver::new(SpatialPosition3d::new(30.0, 0.0, 5.0).unwrap());
        let f1 = near.convolve_mono_frame(&mono).unwrap();
        let f2 = near.convolve_mono_frame(&mono).unwrap();
        let ff = far.convolve_mono_frame(&mono).unwrap();
        assert!((f2.right[0] - f1.right[PCM_FRAME_SAMPLES - 1]).abs() < 0.05);
        let e_near: f32 = f1.right.iter().map(|x| x * x).sum();
        let e_far: f32 = ff.right.iter().map(|x| x * x).sum();
        assert!(e_near > e_far * 20.0);
    }

    #[test]
    fn test_sub_986_4_ten_participants_cpu_benchmark_and_budget_boundaries() {
        let r = benchmark_ten_participants_cpu(25).unwrap();
        assert!(r.participants == 10 && r.cpu_load_pct > 0.0 && r.cpu_load_pct < 2.0);
        println!(
            "SPATIAL_BENCH_10_PEERS_JSON:{{\"participants\":{},\"frames_mixed\":{},\"avg_frame_time_us\":{:.3},\"cpu_load_pct\":{:.4}}}",
            r.participants, r.frames_mixed, r.avg_frame_time_us, r.cpu_load_pct
        );
        assert!(validate_spatial_cpu_budget(1.99).is_ok());
        for bad in [2.00_f32, 0.0, f32::NAN] {
            assert!(validate_spatial_cpu_budget(bad).is_err());
        }
    }

    #[test]
    fn test_sub_986_4_nine_dimensions_policy_and_telemetry() {
        let mut v = SpatialWorkloadContext {
            user_id: 424_242,
            message_len: 2_000,
            attachment_count: 10,
            attachment_bytes: 25 * 1024 * 1024,
            search_offset: 9_975,
            volume_pct: 200,
            unread_count: 150,
            permissions: PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK,
            age_gated_space: true,
            age_verified: true,
            http_status: 200,
        };
        assert_eq!(validate_spatial_workload_boundaries(&v).unwrap(), "99+");
        v.user_id = 0;
        assert!(validate_spatial_workload_boundaries(&v).is_err());
        v.user_id = u64::MAX;
        assert!(validate_spatial_workload_boundaries(&v).is_err());
        v.user_id = 424_242;
        v.volume_pct = 201;
        assert!(validate_spatial_workload_boundaries(&v).is_err());
        v.volume_pct = 200;
        v.http_status = 202;
        assert!(validate_spatial_workload_boundaries(&v).is_err());
        assert_eq!(
            compute_spatial_rest_backoff_ms(429, Some(777), 0),
            Some(777)
        );
        assert_eq!(compute_spatial_rest_backoff_ms(503, None, 2), Some(2_000));
        assert_eq!(compute_spatial_rest_backoff_ms(401, None, 0), None);
        assert!(is_opus_dtx_silence_packet(&OPUS_DTX_SILENCE_FRAME));
        let kind = SpatialBackendKind::select_best(false, true, true);
        assert_eq!(kind, SpatialBackendKind::KemarBinaural);
        let cues = compute_binaural_cues(45.0, 0.0, 2.0);
        let note = "mfa.secret986 secret_key=deadbeef";
        let st = PeerAvatar3dState::Panned;
        let j = export_spatial_telemetry_json(&cues, 0.35, st, note);
        assert!(j.contains("mfa.***") && j.contains("[REDACTED]") && !j.contains("secret986"));
    }
}
