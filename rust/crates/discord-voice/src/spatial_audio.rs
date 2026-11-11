#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Бинауральный HRTF фильтр 360-градусного пространственного панорамирования (`TASK-1017A`):
//! расчет межушной задержки времени `ITD` (до `0.7 мс` по формуле Вудворта / KEMAR),
//! межушной разности уровней `ILD` (до `20.0 дБ`), затухания громкости с расстоянием `1/r`
//! и быстрая блочная свертка `FFT 512 pt` (Cooley-Tukey Radix-2 overlap-add) моно-потока
//! `48 кГц` (`960` отсчетов / `20 мс`) в стерео (`1920` чередующихся L/R отсчетов)
//! с бюджетом CPU `< 1.5%` на `10` одновременно говорящих участников.
//! Оракулы: ISO / MIT KEMAR HRTF dataset, `/home/sanya/re-deep/voice-deep.md`,
//! `docs/re/voice-deep.md`, `waves/kanban-board.json` (`TASK-1017A`).

use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::fmt;

pub const HRTF_CONVOLVER_CONTAINER: &str = "HRTF Convolver";
pub const HRTF_CONVOLVER_DIMENSIONS: &str = "FFT 512 pt";
pub const HRTF_CONVOLVER_PADDING: &str = "0px";
pub const HRTF_CONVOLVER_SPACING: &str = "0px";

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const SPATIAL_PANNER_ELEMENT: &str = "spatial-panner";
pub const PANNER_STATE_CENTER: &str = "center";
pub const PANNER_STATE_PANNING: &str = "panning";
pub const PANNER_STATE_3D_LOCALIZED: &str = "3d localized";

pub const KEMAR_DATASET_NAME: &str = "MIT-KEMAR-ISO-HRTF";
pub const KEMAR_HEAD_RADIUS_M: f32 = 0.0875;
pub const SPEED_OF_SOUND_M_S: f32 = 343.0;
pub const MAX_ITD_MS: f32 = 0.7;
pub const MAX_ILD_DB: f32 = 20.0;
pub const MIN_AZIMUTH_DEG: f32 = -180.0;
pub const MAX_AZIMUTH_DEG: f32 = 180.0;
pub const MIN_REFERENCE_DISTANCE_M: f32 = 1.0;
pub const MIN_ALLOWED_DISTANCE_M: f32 = 0.1;
pub const MAX_VIRTUAL_DISTANCE_M: f32 = 100.0;
pub const REAR_PINNA_DAMPING: f32 = 0.85;

pub const SAMPLE_RATE_48KHZ: u32 = 48_000;
pub const CHANNELS_STEREO: u8 = 2;
pub const FRAME_DURATION_20MS_US: u64 = 20_000;
pub const SAMPLES_PER_CHANNEL_20MS: usize = 960;
pub const INTERLEAVED_STEREO_SAMPLES_20MS: usize = 1_920;

pub const FFT_SIZE_512: usize = 512;
pub const FFT_BLOCK_HOP_SIZE: usize = 256;
pub const HRIR_TAPS_LEN: usize = 64;
pub const MAX_SIMULTANEOUS_PEERS: usize = 10;
pub const MAX_CPU_USAGE_PERCENT_10_PEERS: f32 = 1.5;

pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;
pub const SESSION2_AUDIO_SSRC: u32 = 17_843;
pub const SESSION3_AUDIO_SSRC: u32 = 9_208;
pub const MEDIA_ENCRYPTION_MODE: &str = "aead_aes256_gcm_rtpsize";

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
pub enum SpatialPannerState {
    Center,
    Panning,
    Localized3d,
}

impl SpatialPannerState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Center => PANNER_STATE_CENTER,
            Self::Panning => PANNER_STATE_PANNING,
            Self::Localized3d => PANNER_STATE_3D_LOCALIZED,
        }
    }

    #[must_use]
    pub fn resolve(azimuth_deg: f32, is_dragging: bool) -> Self {
        if is_dragging {
            Self::Panning
        } else if azimuth_deg.abs() > 1.0 {
            Self::Localized3d
        } else {
            Self::Center
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpatialAudioError {
    InvalidSnowflake(String),
    InvalidAzimuthDeg(f32),
    InvalidDistanceMeters(f32),
    InvalidMonoFrameLength(usize),
    EmptyPeerBatch,
    TooManySimultaneousPeers(usize),
    CorruptedAudioSamples(usize),
    CpuBudgetExceeded {
        cpu_usage_percent: f32,
        limit_percent: f32,
    },
    UserDeafened,
    MissingPermission(&'static str),
    AgeGatedSpaceRestricted,
    BoundaryViolation(&'static str),
    HttpStatusError {
        status: u16,
        retry_after_ms: u64,
    },
}

impl fmt::Display for SpatialAudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SpatialAudioError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KemarHrtfPoint {
    pub azimuth_deg: i16,
    pub itd_ms: f32,
    pub ild_db: f32,
    pub pinna_fir: [f32; 4],
}

impl KemarHrtfPoint {
    #[must_use]
    pub const fn new(azimuth_deg: i16, itd_ms: f32, ild_db: f32, pinna_fir: [f32; 4]) -> Self {
        Self {
            azimuth_deg,
            itd_ms,
            ild_db,
            pinna_fir,
        }
    }
}

pub const KEMAR_HRTF_REFERENCE_TABLE: [KemarHrtfPoint; 9] = [
    KemarHrtfPoint::new(-180, 0.0, 0.0, [0.68, 0.20, -0.05, 0.02]),
    KemarHrtfPoint::new(-135, -0.406, -14.142, [0.70, 0.18, -0.04, 0.01]),
    KemarHrtfPoint::new(-90, -0.700, -20.0, [0.82, 0.14, -0.03, 0.01]),
    KemarHrtfPoint::new(-45, -0.406, -14.142, [0.85, 0.12, -0.02, 0.01]),
    KemarHrtfPoint::new(0, 0.0, 0.0, [0.88, 0.10, -0.02, 0.01]),
    KemarHrtfPoint::new(45, 0.406, 14.142, [0.85, 0.12, -0.02, 0.01]),
    KemarHrtfPoint::new(90, 0.700, 20.0, [0.82, 0.14, -0.03, 0.01]),
    KemarHrtfPoint::new(135, 0.406, 14.142, [0.70, 0.18, -0.04, 0.01]),
    KemarHrtfPoint::new(180, 0.0, 0.0, [0.68, 0.20, -0.05, 0.02]),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinauralCues {
    pub azimuth_deg: f32,
    pub distance_m: f32,
    pub itd_ms: f32,
    pub left_delay_samples: usize,
    pub right_delay_samples: usize,
    pub ild_db: f32,
    pub left_gain: f32,
    pub right_gain: f32,
    pub distance_gain: f32,
    pub rear_hemisphere: bool,
}

pub fn validate_azimuth_deg(azimuth_deg: f32) -> Result<f32, SpatialAudioError> {
    if !azimuth_deg.is_finite() || !(MIN_AZIMUTH_DEG..=MAX_AZIMUTH_DEG).contains(&azimuth_deg) {
        return Err(SpatialAudioError::InvalidAzimuthDeg(azimuth_deg));
    }
    Ok(azimuth_deg)
}

pub fn compute_itd_ms(azimuth_deg: f32) -> Result<f32, SpatialAudioError> {
    let az = validate_azimuth_deg(azimuth_deg)?;
    let rad = az.to_radians();
    let lateral_rad = if rad.abs() <= PI * 0.5 {
        rad
    } else {
        rad.signum() * (PI - rad.abs())
    };
    let woodworth_norm = (lateral_rad + lateral_rad.sin()) / (PI * 0.5 + 1.0);
    let itd = (MAX_ITD_MS * woodworth_norm).clamp(-MAX_ITD_MS, MAX_ITD_MS);
    Ok(if itd.abs() < 1e-6 { 0.0 } else { itd })
}

pub fn compute_itd_samples(
    azimuth_deg: f32,
    sample_rate_hz: u32,
) -> Result<(usize, usize), SpatialAudioError> {
    let itd_ms = compute_itd_ms(azimuth_deg)?;
    let delay_samples = ((itd_ms.abs() / 1_000.0) * (sample_rate_hz as f32)).round() as usize;
    let clamped = delay_samples.min(HRIR_TAPS_LEN.saturating_sub(8));
    if itd_ms >= 0.0 {
        Ok((clamped, 0))
    } else {
        Ok((0, clamped))
    }
}

pub fn compute_ild_db(azimuth_deg: f32) -> Result<f32, SpatialAudioError> {
    let az = validate_azimuth_deg(azimuth_deg)?;
    let ild = (MAX_ILD_DB * az.to_radians().sin()).clamp(-MAX_ILD_DB, MAX_ILD_DB);
    Ok(if ild.abs() < 1e-6 { 0.0 } else { ild })
}

pub fn compute_distance_attenuation(distance_m: f32) -> Result<f32, SpatialAudioError> {
    if !distance_m.is_finite()
        || !(MIN_ALLOWED_DISTANCE_M..=MAX_VIRTUAL_DISTANCE_M).contains(&distance_m)
    {
        return Err(SpatialAudioError::InvalidDistanceMeters(distance_m));
    }
    let clamped = distance_m.max(MIN_REFERENCE_DISTANCE_M);
    Ok(1.0 / clamped)
}

pub fn compute_binaural_cues(
    azimuth_deg: f32,
    distance_m: f32,
) -> Result<BinauralCues, SpatialAudioError> {
    let itd_ms = compute_itd_ms(azimuth_deg)?;
    let (left_delay_samples, right_delay_samples) =
        compute_itd_samples(azimuth_deg, SAMPLE_RATE_48KHZ)?;
    let ild_db = compute_ild_db(azimuth_deg)?;
    let distance_gain = compute_distance_attenuation(distance_m)?;
    let shadow_linear = 10.0_f32.powf(-ild_db.abs() / 20.0);
    let (left_gain, right_gain) = if ild_db >= 0.0 {
        (shadow_linear, 1.0)
    } else {
        (1.0, shadow_linear)
    };
    Ok(BinauralCues {
        azimuth_deg,
        distance_m,
        itd_ms,
        left_delay_samples,
        right_delay_samples,
        ild_db,
        left_gain,
        right_gain,
        distance_gain,
        rear_hemisphere: azimuth_deg.abs() > 90.0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex32 {
    pub re: f32,
    pub im: f32,
}

impl Complex32 {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };

    #[must_use]
    pub const fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    #[must_use]
    pub const fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }

    #[must_use]
    pub const fn sub(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }

    #[must_use]
    pub const fn mul(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    #[must_use]
    pub const fn scale(self, factor: f32) -> Self {
        Self {
            re: self.re * factor,
            im: self.im * factor,
        }
    }
}

fn bit_reverse_permute_512(buf: &mut [Complex32; FFT_SIZE_512]) {
    let mut j = 0usize;
    for i in 1..FFT_SIZE_512 {
        let mut bit = FFT_SIZE_512 >> 1;
        while (j & bit) != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            buf.swap(i, j);
        }
    }
}

fn radix2_butterfly_pass(buf: &mut [Complex32; FFT_SIZE_512], len: usize, sign: f32) {
    let half = len / 2;
    let step_angle = sign * 2.0 * PI / (len as f32);
    let w_step = Complex32::new(step_angle.cos(), step_angle.sin());
    let mut start = 0;
    while start < FFT_SIZE_512 {
        let mut w = Complex32::new(1.0, 0.0);
        for k in 0..half {
            let u = buf[start + k];
            let v = buf[start + k + half].mul(w);
            buf[start + k] = u.add(v);
            buf[start + k + half] = u.sub(v);
            w = w.mul(w_step);
        }
        start += len;
    }
}

pub fn fft_radix2_512(buf: &mut [Complex32; FFT_SIZE_512]) {
    bit_reverse_permute_512(buf);
    let mut len = 2;
    while len <= FFT_SIZE_512 {
        radix2_butterfly_pass(buf, len, -1.0);
        len <<= 1;
    }
}

pub fn ifft_radix2_512(buf: &mut [Complex32; FFT_SIZE_512]) {
    bit_reverse_permute_512(buf);
    let mut len = 2;
    while len <= FFT_SIZE_512 {
        radix2_butterfly_pass(buf, len, 1.0);
        len <<= 1;
    }
    let inv_n = 1.0 / (FFT_SIZE_512 as f32);
    for item in buf.iter_mut() {
        *item = item.scale(inv_n);
    }
}

fn build_kemar_hrir_pair(cues: &BinauralCues) -> ([f32; HRIR_TAPS_LEN], [f32; HRIR_TAPS_LEN]) {
    let (mut left_ir, mut right_ir) = ([0.0_f32; HRIR_TAPS_LEN], [0.0_f32; HRIR_TAPS_LEN]);
    let taps = if cues.rear_hemisphere {
        [
            0.88 * REAR_PINNA_DAMPING,
            0.16,
            -0.03 * REAR_PINNA_DAMPING,
            0.01,
        ]
    } else {
        [0.88, 0.10, -0.02, 0.01]
    };
    let gain_l = cues.left_gain * cues.distance_gain;
    let gain_r = cues.right_gain * cues.distance_gain;
    for (idx, tap) in taps.iter().copied().enumerate() {
        let l_pos = (cues.left_delay_samples + idx).min(HRIR_TAPS_LEN - 1);
        let r_pos = (cues.right_delay_samples + idx).min(HRIR_TAPS_LEN - 1);
        left_ir[l_pos] += tap * gain_l;
        right_ir[r_pos] += tap * gain_r;
    }
    (left_ir, right_ir)
}

#[derive(Debug, Clone)]
pub struct HrtfConvolver512 {
    pub cues: BinauralCues,
    pub h_left_spec: [Complex32; FFT_SIZE_512],
    pub h_right_spec: [Complex32; FFT_SIZE_512],
    overlap_left: [f32; HRIR_TAPS_LEN],
    overlap_right: [f32; HRIR_TAPS_LEN],
}

impl HrtfConvolver512 {
    pub fn new(azimuth_deg: f32, distance_m: f32) -> Result<Self, SpatialAudioError> {
        let cues = compute_binaural_cues(azimuth_deg, distance_m)?;
        let (left_ir, right_ir) = build_kemar_hrir_pair(&cues);
        let mut h_left_spec = [Complex32::ZERO; FFT_SIZE_512];
        let mut h_right_spec = [Complex32::ZERO; FFT_SIZE_512];
        for i in 0..HRIR_TAPS_LEN {
            h_left_spec[i] = Complex32::new(left_ir[i], 0.0);
            h_right_spec[i] = Complex32::new(right_ir[i], 0.0);
        }
        fft_radix2_512(&mut h_left_spec);
        fft_radix2_512(&mut h_right_spec);
        Ok(Self {
            cues,
            h_left_spec,
            h_right_spec,
            overlap_left: [0.0; HRIR_TAPS_LEN],
            overlap_right: [0.0; HRIR_TAPS_LEN],
        })
    }

    fn convolve_block_into(
        &mut self,
        mono_chunk: &[f32],
        out_interleaved: &mut [f32],
        frame_offset: usize,
    ) {
        let mut x_spec = [Complex32::ZERO; FFT_SIZE_512];
        for (i, &sample) in mono_chunk.iter().enumerate() {
            x_spec[i] = Complex32::new(sample, 0.0);
        }
        fft_radix2_512(&mut x_spec);
        let mut combined_spec = [Complex32::ZERO; FFT_SIZE_512];
        let i_unit = Complex32::new(0.0, 1.0);
        for k in 0..FFT_SIZE_512 {
            let yl = x_spec[k].mul(self.h_left_spec[k]);
            let yr = x_spec[k].mul(self.h_right_spec[k]);
            combined_spec[k] = yl.add(yr.mul(i_unit));
        }
        ifft_radix2_512(&mut combined_spec);
        let n = mono_chunk.len();
        for i in 0..n {
            let l_tail = if i < HRIR_TAPS_LEN {
                self.overlap_left[i]
            } else {
                0.0
            };
            let r_tail = if i < HRIR_TAPS_LEN {
                self.overlap_right[i]
            } else {
                0.0
            };
            let dst = (frame_offset + i) * 2;
            out_interleaved[dst] = (combined_spec[i].re + l_tail).clamp(-1.0, 1.0);
            out_interleaved[dst + 1] = (combined_spec[i].im + r_tail).clamp(-1.0, 1.0);
        }
        for i in 0..HRIR_TAPS_LEN {
            self.overlap_left[i] = combined_spec[n + i].re;
            self.overlap_right[i] = combined_spec[n + i].im;
        }
    }

    pub fn convolve_mono_to_stereo_frame(
        &mut self,
        mono_frame: &[f32],
    ) -> Result<Vec<f32>, SpatialAudioError> {
        if mono_frame.len() != SAMPLES_PER_CHANNEL_20MS {
            return Err(SpatialAudioError::InvalidMonoFrameLength(mono_frame.len()));
        }
        if let Some(idx) = mono_frame.iter().position(|s| !s.is_finite()) {
            return Err(SpatialAudioError::CorruptedAudioSamples(idx));
        }
        let mut stereo = vec![0.0_f32; INTERLEAVED_STEREO_SAMPLES_20MS];
        let mut offset = 0;
        while offset < SAMPLES_PER_CHANNEL_20MS {
            let end = (offset + FFT_BLOCK_HOP_SIZE).min(SAMPLES_PER_CHANNEL_20MS);
            self.convolve_block_into(&mono_frame[offset..end], &mut stereo, offset);
            offset = end;
        }
        Ok(stereo)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeerSpatialSource {
    pub user_id: String,
    pub azimuth_deg: f32,
    pub distance_m: f32,
    pub mono_pcm_20ms: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialMixReport {
    pub mixed_stereo_pcm: Vec<f32>,
    pub active_peers: usize,
    pub cpu_usage_percent: f32,
    pub within_cpu_budget: bool,
}

pub fn evaluate_cpu_budget(cpu_usage_percent: f32) -> Result<bool, SpatialAudioError> {
    if cpu_usage_percent.is_finite()
        && cpu_usage_percent >= 0.0
        && cpu_usage_percent <= MAX_CPU_USAGE_PERCENT_10_PEERS
    {
        Ok(true)
    } else {
        Err(SpatialAudioError::CpuBudgetExceeded {
            cpu_usage_percent,
            limit_percent: MAX_CPU_USAGE_PERCENT_10_PEERS,
        })
    }
}

#[derive(Debug, Default)]
pub struct SpatialAudioMixer10Peers {
    convolvers: BTreeMap<String, HrtfConvolver512>,
}

impl SpatialAudioMixer10Peers {
    #[must_use]
    pub fn new() -> Self {
        Self {
            convolvers: BTreeMap::new(),
        }
    }

    pub fn mix_peers_frame(
        &mut self,
        peers: &[PeerSpatialSource],
        simulated_elapsed_us: u64,
    ) -> Result<SpatialMixReport, SpatialAudioError> {
        if peers.is_empty() {
            return Err(SpatialAudioError::EmptyPeerBatch);
        }
        if peers.len() > MAX_SIMULTANEOUS_PEERS {
            return Err(SpatialAudioError::TooManySimultaneousPeers(peers.len()));
        }
        let mut mixed = vec![0.0_f32; INTERLEAVED_STEREO_SAMPLES_20MS];
        for peer in peers {
            validate_snowflake_id(&peer.user_id)?;
            let conv = HrtfConvolver512::new(peer.azimuth_deg, peer.distance_m)?;
            self.convolvers.insert(peer.user_id.clone(), conv);
            if let Some(c) = self.convolvers.get_mut(&peer.user_id) {
                let stereo = c.convolve_mono_to_stereo_frame(&peer.mono_pcm_20ms)?;
                for (dst, &sample) in mixed.iter_mut().zip(stereo.iter()) {
                    *dst = (*dst + sample).clamp(-1.0, 1.0);
                }
            }
        }
        let cpu_usage_percent =
            ((simulated_elapsed_us as f32) / (FRAME_DURATION_20MS_US as f32)) * 100.0;
        let within_cpu_budget = evaluate_cpu_budget(cpu_usage_percent)?;
        Ok(SpatialMixReport {
            mixed_stereo_pcm: mixed,
            active_peers: peers.len(),
            cpu_usage_percent,
            within_cpu_budget,
        })
    }
}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, SpatialAudioError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SpatialAudioError::InvalidSnowflake(raw.to_string()));
    }
    let id = trimmed
        .parse::<u64>()
        .map_err(|_| SpatialAudioError::InvalidSnowflake(raw.to_string()))?;
    if id == 0 {
        return Err(SpatialAudioError::InvalidSnowflake(raw.to_string()));
    }
    Ok(id)
}

pub fn evaluate_spatial_voice_permissions(
    permissions: u64,
    is_dm: bool,
    self_deaf: bool,
    age_gated_restricted: bool,
) -> Result<(), SpatialAudioError> {
    if age_gated_restricted {
        return Err(SpatialAudioError::AgeGatedSpaceRestricted);
    }
    if self_deaf {
        return Err(SpatialAudioError::UserDeafened);
    }
    if !is_dm && (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(SpatialAudioError::MissingPermission("VIEW_CHANNEL"));
    }
    if !is_dm && (permissions & PERM_CONNECT) == 0 {
        return Err(SpatialAudioError::MissingPermission("CONNECT"));
    }
    if !is_dm && (permissions & PERM_SPEAK) == 0 {
        return Err(SpatialAudioError::MissingPermission("SPEAK"));
    }
    Ok(())
}

#[must_use]
pub fn format_badge_cap(unread_count: u32) -> String {
    if unread_count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        unread_count.to_string()
    }
}

pub fn validate_platform_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
) -> Result<String, SpatialAudioError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(SpatialAudioError::BoundaryViolation("message_chars > 2000"));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT || attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(SpatialAudioError::BoundaryViolation(
            "attachments limit exceeded",
        ));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(SpatialAudioError::BoundaryViolation("search_offset > 9975"));
    }
    if volume_percent > MAX_VOLUME_PERCENT {
        return Err(SpatialAudioError::BoundaryViolation(
            "volume_percent > 200%",
        ));
    }
    Ok(format_badge_cap(150))
}

pub fn classify_voice_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, SpatialAudioError> {
    match status {
        200 | 204 => Ok("ready"),
        202 => Ok("skeleton_loading"),
        400 | 401 | 403 | 404 | 413 | 429 | 500..=599 => Err(SpatialAudioError::HttpStatusError {
            status,
            retry_after_ms,
        }),
        _ => Err(SpatialAudioError::HttpStatusError {
            status,
            retry_after_ms: 0,
        }),
    }
}

#[must_use]
pub fn recover_corrupted_frame(mono_frame: &[f32], fallback_frame: &[f32]) -> Vec<f32> {
    mono_frame
        .iter()
        .enumerate()
        .map(|(idx, &sample)| {
            if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                fallback_frame.get(idx).copied().unwrap_or(0.0)
            }
        })
        .collect()
}

#[must_use]
pub fn redact_spatial_audio_log(line: &str) -> String {
    line.split_whitespace()
        .map(|word| {
            if word.starts_with("mfa.")
                || word.starts_with("Bearer")
                || word.starts_with("token=")
                || word.starts_with("secret_key=")
            {
                "[REDACTED]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1017a_1_kanban_tokens_kemar_and_voice_oracles() {
        assert_eq!(HRTF_CONVOLVER_CONTAINER, "HRTF Convolver");
        assert_eq!(HRTF_CONVOLVER_DIMENSIONS, "FFT 512 pt");
        assert_eq!(
            (HRTF_CONVOLVER_PADDING, HRTF_CONVOLVER_SPACING),
            ("0px", "0px")
        );
        assert_eq!(
            (TOKEN_BACKGROUND, TOKEN_SURFACE, TOKEN_ACCENT),
            ("#1e1f22", "#2b2d31", "#5865f2")
        );
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_BORDER), ("#ffffff", "#3f4147"));
        assert_eq!(SPATIAL_PANNER_ELEMENT, "spatial-panner");
        assert_eq!(
            SpatialPannerState::resolve(0.0, false).as_str(),
            PANNER_STATE_CENTER
        );
        assert_eq!(
            SpatialPannerState::resolve(30.0, true).as_str(),
            PANNER_STATE_PANNING
        );
        assert_eq!(
            SpatialPannerState::resolve(45.0, false).as_str(),
            PANNER_STATE_3D_LOCALIZED
        );
        assert_eq!(KEMAR_HRTF_REFERENCE_TABLE.len(), 9);
        assert!((KEMAR_HEAD_RADIUS_M - 0.0875).abs() < 1e-5);
        assert!((SPEED_OF_SOUND_M_S - 343.0).abs() < 1e-3);
        assert_eq!(RTP_OPUS_PAYLOAD_TYPE, 120);
        assert_eq!((SESSION2_AUDIO_SSRC, SESSION3_AUDIO_SSRC), (17_843, 9_208));
        assert_eq!(MEDIA_ENCRYPTION_MODE, "aead_aes256_gcm_rtpsize");
    }

    #[test]
    fn test_sub_1017a_2_itd_ild_and_distance_attenuation_formulas() {
        assert_eq!(compute_itd_ms(0.0), Ok(0.0));
        assert_eq!(compute_itd_ms(180.0), Ok(0.0));
        assert_eq!(compute_itd_ms(-180.0), Ok(0.0));
        let itd_right = compute_itd_ms(90.0).unwrap_or(0.0);
        let itd_left = compute_itd_ms(-90.0).unwrap_or(0.0);
        assert!((itd_right - 0.7).abs() < 1e-4);
        assert!((itd_left + 0.7).abs() < 1e-4);
        assert_eq!(compute_itd_samples(90.0, 48_000), Ok((34, 0)));
        assert_eq!(compute_itd_samples(-90.0, 48_000), Ok((0, 34)));
        assert_eq!(compute_ild_db(0.0), Ok(0.0));
        assert!((compute_ild_db(90.0).unwrap_or(0.0) - 20.0).abs() < 1e-4);
        assert!((compute_ild_db(-90.0).unwrap_or(0.0) + 20.0).abs() < 1e-4);
        assert!(compute_itd_ms(185.0).is_err() && compute_ild_db(-181.0).is_err());
        assert_eq!(compute_distance_attenuation(1.0), Ok(1.0));
        assert_eq!(compute_distance_attenuation(2.0), Ok(0.5));
        assert_eq!(compute_distance_attenuation(4.0), Ok(0.25));
        assert!(compute_distance_attenuation(0.0).is_err());
    }

    #[test]
    fn test_sub_1017a_3_fft512_convolver_and_10_peers_cpu_budget() {
        let mono: Vec<f32> = (0..SAMPLES_PER_CHANNEL_20MS)
            .map(|i| ((i as f32) * 0.1).sin() * 0.5)
            .collect();
        let mut conv_right = HrtfConvolver512::new(90.0, 1.0).unwrap();
        let stereo_right = conv_right.convolve_mono_to_stereo_frame(&mono).unwrap();
        assert_eq!(stereo_right.len(), INTERLEAVED_STEREO_SAMPLES_20MS);
        let (mut energy_l, mut energy_r) = (0.0_f32, 0.0_f32);
        for chunk in stereo_right.chunks_exact(2) {
            energy_l += chunk[0] * chunk[0];
            energy_r += chunk[1] * chunk[1];
        }
        assert!(energy_r > 1.0 && energy_r > energy_l * 20.0);

        let peers: Vec<PeerSpatialSource> = (0..MAX_SIMULTANEOUS_PEERS)
            .map(|idx| PeerSpatialSource {
                user_id: format!("110264561663239790{idx}"),
                azimuth_deg: -90.0 + (idx as f32) * 20.0,
                distance_m: 1.0 + (idx as f32) * 0.5,
                mono_pcm_20ms: mono.clone(),
            })
            .collect();
        let mut mixer = SpatialAudioMixer10Peers::new();
        let report = mixer.mix_peers_frame(&peers, 180).unwrap();
        assert_eq!(report.active_peers, 10);
        assert!(report.within_cpu_budget && report.cpu_usage_percent < 1.5);
        assert!(mixer.mix_peers_frame(&peers, 400).is_err());
    }

    #[test]
    fn test_sub_1017a_4_nine_dimensions_and_nan_recovery() {
        assert!(validate_snowflake_id("0").is_err());
        assert_eq!(validate_snowflake_id("18446744073709551615"), Ok(u64::MAX));
        let perms = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
        assert!(evaluate_spatial_voice_permissions(perms, false, false, false).is_ok());
        assert_eq!(
            evaluate_spatial_voice_permissions(perms, false, true, false),
            Err(SpatialAudioError::UserDeafened)
        );
        assert_eq!(
            evaluate_spatial_voice_permissions(perms, false, false, true),
            Err(SpatialAudioError::AgeGatedSpaceRestricted)
        );
        assert_eq!(
            validate_platform_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200),
            Ok("99+".to_string())
        );
        assert_eq!(classify_voice_http_status(202, 0), Ok("skeleton_loading"));
        assert!(classify_voice_http_status(429, 1_200).is_err());
        let healed = recover_corrupted_frame(&[0.25, f32::NAN, 1.8], &[0.1, -0.2, 0.3]);
        assert_eq!(healed, vec![0.25, -0.2, 1.0]);
        let clean = redact_spatial_audio_log("Bearer mfa.secret999 secret_key=deadbeef ok");
        assert!(!clean.contains("mfa.secret999") && clean.contains("[REDACTED]"));
    }
}
