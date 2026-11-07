#![forbid(unsafe_code)]
//! # discord-media
//!
//! Медиа-конвейер: аудио (Opus) и видео (AV1/H264) кодеки, захват экрана/камеры, стриминг.
//!
//! Спека: `docs/re/voice-deep.md` §4 (движок кодеков из логов `[Connection]`).
//! План: P3-06…P3-09 (медиа), P8 (скриншот-стрим); тесты: T-MEDIA-*.

use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::Duration;

/// Аудио-кодек медиа-канала.
pub const AUDIO_CODEC: &str = "opus";

/// Видео-кодек медиа-канала (Session Description).
pub const VIDEO_CODEC: &str = "AV1";

/// Доступные движку кодеки из лога клиента (`Available engine codecs`).
pub const ENGINE_CODECS: [&str; 4] = ["AV1", "H264", "H265", "VP8"];

/// Целевая частота обновления рендера видеопотока (60 FPS).
pub const TARGET_RENDER_FPS: u32 = 60;

/// Бюджет одного кадра при 60 FPS (16 мс).
pub const FRAME_BUDGET_60FPS: Duration = Duration::from_millis(16);

/// Ёмкость ограниченного канала между потоком захвата и энкодером.
pub const CAPTURE_QUEUE_CAPACITY: usize = 2;

/// Размер кольцевого буфера истории отправленных RTP-пакетов для RTX.
pub const MAX_RTX_HISTORY_PACKETS: usize = 512;

/// Максимальный размер одиночного NAL-юнита в RTP-пакете (без FU-A фрагментации).
pub const MAX_SINGLE_NAL_SIZE: usize = 1180;

/// Размер стандартного заголовка RTP-пакета (RFC 3550).
pub const RTP_HEADER_LEN: usize = 12;

/// Payload Type для H.264 видео в Discord Voice/Stream.
pub const RTP_VIDEO_PAYLOAD_TYPE: u8 = 101;

/// Payload Type для RFC 4588 RTX ретрансляции в Discord Voice/Stream.
pub const RTP_RTX_PAYLOAD_TYPE: u8 = 102;

/// Максимальное число RTP-фрагментов на один H.264 кадр.
pub const MAX_H264_FRAGMENTS: usize = 2048;

/// Максимальный размер входного H.264 Annex-B кадра (2 МБ + 64 КБ DAVE-оверхед).
pub const MAX_H264_FRAME_BYTES: usize = 2 * 1024 * 1024 + 64 * 1024;

/// Встроенный WGSL-шейдер прямой загрузки YUV420 (NV12 / I420) плоскостей в GPU
/// (по образцу `Serein/apps/desktop/src/gpu_video.rs`).
pub const WGSL_YUV_SHADER: &str = r"
struct Params {
	rows: array<vec4<f32>, 3>,
	rotation: vec4<u32>,
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var planes: sampler;
@group(0) @binding(2) var luma: texture_2d<f32>;
@group(0) @binding(3) var chroma_a: texture_2d<f32>;
@group(0) @binding(4) var chroma_b: texture_2d<f32>;

struct Varyings {
	@builtin(position) position: vec4<f32>,
	@location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Varyings {
	let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	var uv = corner;
	switch params.rotation.x {
		case 90u: { uv = vec2<f32>(corner.y, 1.0 - corner.x); }
		case 180u: { uv = vec2<f32>(1.0) - corner; }
		case 270u: { uv = vec2<f32>(1.0 - corner.y, corner.x); }
		default: {}
	}
	var out: Varyings;
	out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
	out.uv = uv;
	return out;
}

fn to_rgb(yuv: vec3<f32>) -> vec4<f32> {
	let rgb = vec3<f32>(
		dot(params.rows[0].xyz, yuv) + params.rows[0].w,
		dot(params.rows[1].xyz, yuv) + params.rows[1].w,
		dot(params.rows[2].xyz, yuv) + params.rows[2].w,
	);
	return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

@fragment
fn fs_nv12(in: Varyings) -> @location(0) vec4<f32> {
	let y = textureSampleLevel(luma, planes, in.uv, 0.0).r;
	let cbcr = textureSampleLevel(chroma_a, planes, in.uv, 0.0).rg;
	return to_rgb(vec3<f32>(y, cbcr));
}

@fragment
fn fs_i420(in: Varyings) -> @location(0) vec4<f32> {
	let y = textureSampleLevel(luma, planes, in.uv, 0.0).r;
	let cb = textureSampleLevel(chroma_a, planes, in.uv, 0.0).r;
	let cr = textureSampleLevel(chroma_b, planes, in.uv, 0.0).r;
	return to_rgb(vec3<f32>(y, cb, cr));
}
";

/// Формат размещения пиксельных плоскостей кадра.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelLayout {
    /// Упакованный RGBA8 (4 байта на пиксель).
    Rgba,
    /// Полупланарный YUV 4:2:0: плоскость Y (`W*H`), затем чередующиеся пары Cb/Cr (`(W/2)*(H/2)*2`).
    Nv12,
    /// Планарный YUV 4:2:0: плоскость Y (`W*H`), затем Cb (`(W/2)*(H/2)`), затем Cr (`(W/2)*(H/2)`).
    I420,
}

impl PixelLayout {
    /// Точный размер буфера кадра в байтах для заданных размеров или `None` при переполнении/нуле.
    pub fn byte_len(self, width: u32, height: u32) -> Option<usize> {
        if width == 0 || height == 0 {
            return None;
        }
        let (w, h) = (width as usize, height as usize);
        let luma = w.checked_mul(h)?;
        let chroma = w.div_ceil(2).checked_mul(h.div_ceil(2))?;
        match self {
            Self::Rgba => luma.checked_mul(4),
            Self::Nv12 | Self::I420 => luma.checked_add(chroma.checked_mul(2)?),
        }
    }
}

/// Матрица коэффициентов цветового пространства YCbCr -> RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMatrix {
    /// ITU-R BT.601 (SD / WebRTC default).
    Bt601,
    /// ITU-R BT.709 (HD >= 720p).
    Bt709,
}

/// Описание цветового пространства и диапазона яркости/цветности видеокадра.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorSpace {
    pub matrix: ColorMatrix,
    pub full_range: bool,
}

impl ColorSpace {
    pub const BT601_LIMITED: Self = Self {
        matrix: ColorMatrix::Bt601,
        full_range: false,
    };

    pub const BT601_FULL: Self = Self {
        matrix: ColorMatrix::Bt601,
        full_range: true,
    };

    pub const BT709_LIMITED: Self = Self {
        matrix: ColorMatrix::Bt709,
        full_range: false,
    };

    pub const BT709_FULL: Self = Self {
        matrix: ColorMatrix::Bt709,
        full_range: true,
    };

    pub fn untagged(height: u32, full_range: bool) -> Self {
        Self {
            matrix: if height >= 720 {
                ColorMatrix::Bt709
            } else {
                ColorMatrix::Bt601
            },
            full_range,
        }
    }

    /// Строки матрицы `rgb = row.xyz · [y, cb, cr] + row.w` для нормализованных (0..1) сэмплов
    /// (по образцу `Serein/crates/platform/src/video/picture.rs::Color::coefficients`).
    pub fn coefficients(self) -> [[f32; 4]; 3] {
        let (kr, kb) = match self.matrix {
            ColorMatrix::Bt601 => (0.299, 0.114),
            ColorMatrix::Bt709 => (0.2126, 0.0722),
        };
        let kg = 1.0 - kr - kb;
        let (luma_scale, luma_offset, chroma_scale, chroma_offset) = if self.full_range {
            (1.0, 0.0, 1.0, -128.0 / 255.0)
        } else {
            (255.0 / 219.0, -16.0 / 219.0, 255.0 / 224.0, -128.0 / 224.0)
        };
        let cr_r = 2.0 * (1.0 - kr);
        let cb_b = 2.0 * (1.0 - kb);
        let cb_g = 2.0 * kb * (1.0 - kb) / kg;
        let cr_g = 2.0 * kr * (1.0 - kr) / kg;
        [
            [
                luma_scale,
                0.0,
                cr_r * chroma_scale,
                luma_offset + cr_r * chroma_offset,
            ],
            [
                luma_scale,
                -cb_g * chroma_scale,
                -cr_g * chroma_scale,
                luma_offset - (cb_g + cr_g) * chroma_offset,
            ],
            [
                luma_scale,
                cb_b * chroma_scale,
                0.0,
                luma_offset + cb_b * chroma_offset,
            ],
        ]
    }
}

/// Видеокадр для доставки между декодером/захватом и рендером/энкодером.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub layout: PixelLayout,
    pub color: ColorSpace,
    pub rotation: u16,
    pub timestamp_us: u64,
    pub data: Vec<u8>,
}

impl VideoFrame {
    pub fn new(width: u32, height: u32, layout: PixelLayout, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            layout,
            color: ColorSpace::untagged(height, false),
            rotation: 0,
            timestamp_us: 0,
            data,
        }
    }

    pub fn with_color(mut self, color: ColorSpace) -> Self {
        self.color = color;
        self
    }

    pub fn with_rotation(mut self, rotation: u16) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_timestamp_us(mut self, timestamp_us: u64) -> Self {
        self.timestamp_us = timestamp_us;
        self
    }

    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && matches!(self.rotation, 0 | 90 | 180 | 270)
            && self.layout.byte_len(self.width, self.height) == Some(self.data.len())
    }

    pub const fn format(&self) -> PixelLayout {
        self.layout
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.is_valid() {
            Ok(())
        } else {
            Err("Некорректные размеры, поворот или длина буфера VideoFrame")
        }
    }

    pub fn display_size(&self) -> (u32, u32) {
        if matches!(self.rotation, 90 | 270) {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    pub fn chroma_size(&self) -> (u32, u32) {
        (self.width.div_ceil(2), self.height.div_ceil(2))
    }

    pub fn to_rgba(&self) -> Option<(u32, u32, Vec<u8>)> {
        if !self.is_valid() {
            return None;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let rgba = match self.layout {
            PixelLayout::Rgba => self.data.clone(),
            PixelLayout::Nv12 => {
                let (y_plane, uv_plane) = self.data.split_at(w * h);
                nv12_to_rgba(self.width, self.height, y_plane, uv_plane, self.color).ok()?
            }
            PixelLayout::I420 => {
                let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
                let (y_plane, chroma) = self.data.split_at(w * h);
                let (u_plane, v_plane) = chroma.split_at(cw * ch);
                i420_to_rgba(
                    self.width,
                    self.height,
                    y_plane,
                    u_plane,
                    v_plane,
                    self.color,
                )
                .ok()?
            }
        };
        Some(rotate_rgba(
            &rgba,
            self.width,
            self.height,
            u32::from(self.rotation),
        ))
    }
}

/// Атомарный слот последнего кадра `Arc<Mutex<Option<VideoFrame>>>` без накопления очереди и задержки
/// (по образцу `Serein/apps/desktop/src/watch.rs`).
#[derive(Clone, Default)]
pub struct LatestFrameSlot {
    slot: Arc<Mutex<Option<VideoFrame>>>,
    fresh: Arc<AtomicBool>,
    stored_frames: Arc<AtomicU64>,
    overwritten_frames: Arc<AtomicU64>,
    rejected_frames: Arc<AtomicU64>,
}

impl LatestFrameSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Возвращает ссылку на внутренний атомарный слот `Arc<Mutex<Option<VideoFrame>>>`.
    pub fn inner(&self) -> &Arc<Mutex<Option<VideoFrame>>> {
        &self.slot
    }

    /// Заменяет последний кадр на месте с переиспользованием буфера памяти (как `watch.rs::store_frame`).
    pub fn store_frame(&self, frame: VideoFrame) -> bool {
        if !frame.is_valid() {
            self.rejected_frames.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let Ok(mut guard) = self.slot.lock() else {
            self.rejected_frames.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        let len = frame.data.len();
        let mut data = guard.take().map(|old| old.data).unwrap_or_default();
        if data.capacity() > 1024 * 1024 && data.capacity() > len.saturating_mul(4) {
            // Освобождаем буфер от прежнего высокого разрешения, сохраняя переиспользование при обычном ресайзе.
            data = Vec::new();
        }
        data.clear();
        data.reserve_exact(len);
        data.extend_from_slice(&frame.data);
        *guard = Some(VideoFrame { data, ..frame });
        if self.fresh.swap(true, Ordering::AcqRel) {
            self.overwritten_frames.fetch_add(1, Ordering::Relaxed);
        }
        self.stored_frames.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Неблокирующий опрос последнего свежего кадра для рендера (возвращает `Some` только один раз на каждый новый кадр).
    pub fn poll_latest(&self) -> Option<VideoFrame> {
        let guard = self.slot.try_lock().ok()?;
        let frame = guard.as_ref()?;
        if !self.fresh.swap(false, Ordering::AcqRel) {
            return None;
        }
        Some(frame.clone())
    }

    /// Опрос последнего кадра с проверкой бюджета 60 FPS / 16 мс (`FRAME_BUDGET_60FPS`).
    pub fn poll_latest_60fps(&self, elapsed: Duration) -> Option<VideoFrame> {
        if elapsed < FRAME_BUDGET_60FPS {
            return None;
        }
        self.poll_latest()
    }

    pub fn stored_frames(&self) -> u64 {
        self.stored_frames.load(Ordering::Acquire)
    }

    pub fn overwritten_frames(&self) -> u64 {
        self.overwritten_frames.load(Ordering::Acquire)
    }

    pub fn rejected_frames(&self) -> u64 {
        self.rejected_frames.load(Ordering::Acquire)
    }
}

/// Очередь захвата -> кодирования на базе ограниченного `sync_channel(2)` с неблокирующим `try_send`
/// (по образцу `Serein/crates/discord-voice/src/screen.rs`).
pub struct CaptureEncodeQueue {
    tx: SyncSender<VideoFrame>,
    rx: Arc<Mutex<Receiver<VideoFrame>>>,
    sent_frames: Arc<AtomicU64>,
    dropped_frames: Arc<AtomicU64>,
    force_keyframe: Arc<AtomicBool>,
}

impl Default for CaptureEncodeQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureEncodeQueue {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(CAPTURE_QUEUE_CAPACITY);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
            sent_frames: Arc::new(AtomicU64::new(0)),
            dropped_frames: Arc::new(AtomicU64::new(0)),
            force_keyframe: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Неблокирующая отправка кадра: если канал полон (энкодер занят), отбрасывает самый старый кадр
    /// из очереди без блокировки потока захвата, инкрементирует `dropped_frames` и выставляет `force_keyframe`.
    pub fn try_send(&self, frame: VideoFrame) -> bool {
        if !frame.is_valid() {
            self.dropped_frames.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        match self.tx.try_send(frame) {
            Ok(()) => {
                self.sent_frames.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(mpsc::TrySendError::Full(newest_frame)) => {
                if let Ok(rx_guard) = self.rx.lock() {
                    let _ = rx_guard.try_recv();
                }
                let _ = self.tx.try_send(newest_frame);
                self.dropped_frames.fetch_add(1, Ordering::Relaxed);
                self.force_keyframe.store(true, Ordering::Release);
                false
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.dropped_frames.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Неблокирующее извлечение следующего кадра для потока энкодера.
    pub fn try_recv(&self) -> Option<VideoFrame> {
        let guard = self.rx.lock().ok()?;
        guard.try_recv().ok()
    }

    /// Выставляет атомарный флаг принудительного ключевого кадра (по запросу RTCP PLI / FIR).
    pub fn request_keyframe(&self) {
        self.force_keyframe.store(true, Ordering::Release);
    }

    /// Атомарно считывает и сбрасывает флаг `force_keyframe` перед вызовом энкодера (`swap(false, AcqRel)`).
    pub fn take_force_keyframe(&self) -> bool {
        self.force_keyframe.swap(false, Ordering::AcqRel)
    }

    /// Проверяет текущее состояние флага `force_keyframe` без сброса.
    pub fn force_keyframe(&self) -> bool {
        self.force_keyframe.load(Ordering::Acquire)
    }

    pub fn sent_frames(&self) -> u64 {
        self.sent_frames.load(Ordering::Acquire)
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames.load(Ordering::Acquire)
    }
}

/// RTP-пакет видеопотока или RTX-ретрансляции.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtpPacket {
    pub header: [u8; RTP_HEADER_LEN],
    pub payload: Vec<u8>,
}

impl RtpPacket {
    pub fn new(
        payload_type: u8,
        marker: bool,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
        payload: Vec<u8>,
    ) -> Self {
        let mut header = [0u8; RTP_HEADER_LEN];
        header[0] = 0x80;
        header[1] = (payload_type & 0x7f) | (u8::from(marker) << 7);
        header[2..4].copy_from_slice(&sequence_number.to_be_bytes());
        header[4..8].copy_from_slice(&timestamp.to_be_bytes());
        header[8..12].copy_from_slice(&ssrc.to_be_bytes());
        Self { header, payload }
    }

    pub fn sequence_number(&self) -> u16 {
        u16::from_be_bytes([self.header[2], self.header[3]])
    }

    pub fn timestamp(&self) -> u32 {
        u32::from_be_bytes([
            self.header[4],
            self.header[5],
            self.header[6],
            self.header[7],
        ])
    }

    pub fn ssrc(&self) -> u32 {
        u32::from_be_bytes([
            self.header[8],
            self.header[9],
            self.header[10],
            self.header[11],
        ])
    }

    pub fn payload_type(&self) -> u8 {
        self.header[1] & 0x7f
    }

    pub fn marker(&self) -> bool {
        (self.header[1] & 0x80) != 0
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(RTP_HEADER_LEN + self.payload.len());
        out.extend_from_slice(&self.header);
        out.extend_from_slice(&self.payload);
        out
    }
}

/// Кольцевой буфер на 512 отправленных RTP-пакетов для обслуживания NACK и сборки RFC 4588 RTX
/// (по образцу `Serein/crates/discord-voice/src/video.rs::History`).
#[derive(Default)]
pub struct RtxHistory {
    packets: VecDeque<RtpPacket>,
}

impl RtxHistory {
    pub fn new() -> Self {
        Self {
            packets: VecDeque::with_capacity(MAX_RTX_HISTORY_PACKETS),
        }
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn clear(&mut self) {
        self.packets.clear();
    }

    /// Сохраняет отправленный RTP-пакет в кольцевой буфер (максимум 512 пакетов).
    pub fn record(&mut self, packet: RtpPacket) {
        let seq = packet.sequence_number();
        if let Some(pos) = self
            .packets
            .iter()
            .position(|existing| existing.sequence_number() == seq)
        {
            self.packets.remove(pos);
        }
        while self.packets.len() >= MAX_RTX_HISTORY_PACKETS {
            self.packets.pop_front();
        }
        self.packets.push_back(packet);
    }

    /// Поиск сохранённого RTP-пакета по 16-битному `sequence_number`.
    pub fn find_by_sequence(&self, sequence_number: u16) -> Option<&RtpPacket> {
        let target = sequence_number.to_be_bytes();
        self.packets.iter().find(|pkt| pkt.header[2..4] == target)
    }

    /// Собирает RFC 4588 RTX-пакет (`PT = 102`) с 2-байтным Original Sequence Number (OSN) в начале payload.
    pub fn build_rtx_packet(
        &self,
        original_sequence: u16,
        rtx_ssrc: u32,
        rtx_sequence: &mut u16,
    ) -> Option<RtpPacket> {
        if rtx_ssrc == 0 {
            return None;
        }
        let sent = self.find_by_sequence(original_sequence)?;
        let mut header = sent.header;
        header[1] = (header[1] & 0x80) | RTP_RTX_PAYLOAD_TYPE;
        header[2..4].copy_from_slice(&rtx_sequence.to_be_bytes());
        header[8..12].copy_from_slice(&rtx_ssrc.to_be_bytes());
        *rtx_sequence = rtx_sequence.wrapping_add(1);

        let mut payload = Vec::with_capacity(sent.payload.len() + 2);
        payload.extend_from_slice(&original_sequence.to_be_bytes());
        payload.extend_from_slice(&sent.payload);
        Some(RtpPacket { header, payload })
    }

    /// Восстанавливает оригинальный RTP-пакет из входящего RFC 4588 RTX-пакета (`PT = 102`).
    pub fn restore_rtx_packet(rtx_packet: &RtpPacket, video_ssrc: u32) -> Option<RtpPacket> {
        if rtx_packet.payload_type() != RTP_RTX_PAYLOAD_TYPE || rtx_packet.payload.len() < 2 {
            return None;
        }
        let osn = [rtx_packet.payload[0], rtx_packet.payload[1]];
        let mut header = rtx_packet.header;
        header[1] = (header[1] & 0x80) | RTP_VIDEO_PAYLOAD_TYPE;
        header[2..4].copy_from_slice(&osn);
        header[8..12].copy_from_slice(&video_ssrc.to_be_bytes());
        let payload = rtx_packet.payload[2..].to_vec();
        Some(RtpPacket { header, payload })
    }
}

/// Адаптивный регулятор битрейта по входящим RTCP REMB и потерям с плавным восстановлением
/// (по образцу `Serein/crates/discord-voice/src/video.rs::Rate`).
#[derive(Clone, Debug)]
pub struct AdaptiveBitrateController {
    min_bitrate: u32,
    max_bitrate: u32,
    target_bitrate: u32,
    remb_ceiling: u32,
}

impl AdaptiveBitrateController {
    pub fn new(min_bitrate: u32, max_bitrate: u32) -> Self {
        let min_b = min_bitrate.min(max_bitrate).max(1);
        let max_b = max_bitrate.max(min_bitrate).max(min_b);
        Self {
            min_bitrate: min_b,
            max_bitrate: max_b,
            target_bitrate: max_b,
            remb_ceiling: max_b,
        }
    }

    pub fn min_bitrate(&self) -> u32 {
        self.min_bitrate
    }

    pub fn max_bitrate(&self) -> u32 {
        self.max_bitrate
    }

    pub fn target_bitrate(&self) -> u32 {
        self.target_bitrate
    }

    /// Обновляет оценку по входящему RTCP REMB: при просадке снижает битрейт сразу,
    /// при росте — поднимает потолок и делает плавный шаг восстановления в границах `min_bitrate..=max_bitrate`.
    pub fn on_remb(&mut self, remb_bps: u32) -> u32 {
        let clamped_remb = remb_bps.clamp(self.min_bitrate, self.max_bitrate);
        self.remb_ceiling = clamped_remb;
        if clamped_remb <= self.target_bitrate {
            self.target_bitrate = clamped_remb;
        } else {
            self.recover_step();
        }
        self.target_bitrate
    }

    /// Реакция на RTCP Receiver Report `fraction_lost` (в долях `/256`):
    /// при потерях `>= 5%` (`>= 13/256`) снижает битрейт на 20%, при потерях `<= 2%` (`<= 5/256`) делает плавный шаг вверх.
    pub fn on_packet_loss(&mut self, fraction_lost: u8) -> u32 {
        if fraction_lost >= 13 {
            let reduced = (u64::from(self.target_bitrate) * 4 / 5) as u32;
            self.target_bitrate = reduced.clamp(self.min_bitrate, self.max_bitrate);
        } else if fraction_lost <= 5 {
            self.recover_step();
        }
        self.target_bitrate
    }

    /// Выполняет один шаг плавного восстановления (`+8%`, но не менее `+25 Кбит/с`) до `remb_ceiling.min(max_bitrate)`.
    pub fn recover_step(&mut self) -> u32 {
        let ceiling = self.remb_ceiling.clamp(self.min_bitrate, self.max_bitrate);
        if self.target_bitrate < ceiling {
            let delta = (self.target_bitrate / 12).max(25_000);
            self.target_bitrate = self
                .target_bitrate
                .saturating_add(delta)
                .min(ceiling)
                .clamp(self.min_bitrate, self.max_bitrate);
        } else {
            self.target_bitrate = ceiling;
        }
        self.target_bitrate
    }
}

/// Пакетизатор H.264 Annex-B в RTP (Single NAL <= 1180 байт и FU-A фрагментация с маркером M=1 на последнем пакете,
/// по образцу `Serein/crates/discord-voice/src/video.rs::packetize`).
pub fn packetize_h264_annex_b(
    frame: &[u8],
    sequence: &mut u16,
    timestamp: u32,
    ssrc: u32,
) -> Result<Vec<RtpPacket>, &'static str> {
    if frame.len() > MAX_H264_FRAME_BYTES {
        return Err("H264 frame exceeds the sharing limit");
    }
    let nalus = parse_annex_b_nalus(frame)?;
    let mut packets = Vec::new();
    for (index, nalu) in nalus.iter().enumerate() {
        if nalu.is_empty() {
            continue;
        }
        let last_nalu = index + 1 == nalus.len();
        if nalu.len() <= MAX_SINGLE_NAL_SIZE {
            push_rtp_packet(
                &mut packets,
                sequence,
                timestamp,
                ssrc,
                nalu.to_vec(),
                last_nalu,
            )?;
            continue;
        }
        if nalu.len() < 2 {
            return Err("Invalid H264 NAL unit");
        }
        let indicator = (nalu[0] & 0xe0) | 28;
        let kind = nalu[0] & 0x1f;
        let mut offset = 1;
        while offset < nalu.len() {
            let take = (nalu.len() - offset).min(MAX_SINGLE_NAL_SIZE - 2);
            let mut payload = Vec::with_capacity(take + 2);
            payload.push(indicator);
            payload.push(
                kind | if offset == 1 { 0x80 } else { 0 }
                    | if offset + take == nalu.len() { 0x40 } else { 0 },
            );
            payload.extend_from_slice(&nalu[offset..offset + take]);
            let last = last_nalu && offset + take == nalu.len();
            push_rtp_packet(&mut packets, sequence, timestamp, ssrc, payload, last)?;
            offset += take;
        }
    }
    if packets.is_empty() {
        return Err("H264 frame has no NAL units");
    }
    Ok(packets)
}

fn push_rtp_packet(
    packets: &mut Vec<RtpPacket>,
    sequence: &mut u16,
    timestamp: u32,
    ssrc: u32,
    payload: Vec<u8>,
    marker: bool,
) -> Result<(), &'static str> {
    if packets.len() == MAX_H264_FRAGMENTS {
        return Err("H264 frame exceeds fragment limit");
    }
    let pkt = RtpPacket::new(
        RTP_VIDEO_PAYLOAD_TYPE,
        marker,
        *sequence,
        timestamp,
        ssrc,
        payload,
    );
    *sequence = sequence.wrapping_add(1);
    packets.push(pkt);
    Ok(())
}

fn parse_annex_b_nalus(frame: &[u8]) -> Result<Vec<&[u8]>, &'static str> {
    let mut starts = Vec::new();
    let mut at = 0;
    while at + 3 <= frame.len() {
        let size = if frame[at..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if frame[at..].starts_with(&[0, 0, 1]) {
            3
        } else {
            at += 1;
            continue;
        };
        if starts.len() == MAX_H264_FRAGMENTS {
            return Err("H264 frame exceeds fragment limit");
        }
        starts.push((at, size));
        at += size;
    }
    if starts.is_empty() || starts[0].0 != 0 {
        return Err("H264 frame is not Annex-B");
    }
    let mut out = Vec::with_capacity(starts.len());
    for (index, (start, size)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(frame.len(), |next| next.0);
        if start + size >= end {
            return Err("H264 frame contains an empty NAL unit");
        }
        out.push(&frame[start + size..end]);
    }
    Ok(out)
}

/// Конвертер полупланарного YUV420 NV12 (`Y` + чередующийся `UV`) в RGBA8
/// (по образцу `Serein/crates/platform/src/video/picture.rs` и `gpu_video.rs::fs_nv12`).
pub fn nv12_to_rgba(
    width: u32,
    height: u32,
    y_plane: &[u8],
    uv_plane: &[u8],
    color: ColorSpace,
) -> Result<Vec<u8>, &'static str> {
    if width == 0 || height == 0 {
        return Err("Frame dimensions must be non-zero");
    }
    let (w, h) = (width as usize, height as usize);
    let luma_len = w.checked_mul(h).ok_or("Luma dimension overflow")?;
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let uv_len = cw
        .checked_mul(ch)
        .and_then(|c| c.checked_mul(2))
        .ok_or("Chroma dimension overflow")?;
    if y_plane.len() != luma_len || uv_plane.len() != uv_len {
        return Err("NV12 plane sizes do not match frame dimensions");
    }
    Ok(convert_yuv_planes_to_rgba(
        w,
        h,
        cw,
        ch,
        y_plane,
        color,
        |cx, cy| {
            let at = (cy * cw + cx) * 2;
            [f32::from(uv_plane[at]), f32::from(uv_plane[at + 1])]
        },
    ))
}

/// Конвертер планарного YUV420 I420 (`Y`, `U`, `V`) в RGBA8
/// (по образцу `Serein/crates/platform/src/video/picture.rs` и `gpu_video.rs::fs_i420`).
pub fn i420_to_rgba(
    width: u32,
    height: u32,
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    color: ColorSpace,
) -> Result<Vec<u8>, &'static str> {
    if width == 0 || height == 0 {
        return Err("Frame dimensions must be non-zero");
    }
    let (w, h) = (width as usize, height as usize);
    let luma_len = w.checked_mul(h).ok_or("Luma dimension overflow")?;
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let c_len = cw.checked_mul(ch).ok_or("Chroma dimension overflow")?;
    if y_plane.len() != luma_len || u_plane.len() != c_len || v_plane.len() != c_len {
        return Err("I420 plane sizes do not match frame dimensions");
    }
    Ok(convert_yuv_planes_to_rgba(
        w,
        h,
        cw,
        ch,
        y_plane,
        color,
        |cx, cy| {
            let at = cy * cw + cx;
            [f32::from(u_plane[at]), f32::from(v_plane[at])]
        },
    ))
}

fn convert_yuv_planes_to_rgba(
    width: usize,
    height: usize,
    chroma_width: usize,
    chroma_height: usize,
    y_plane: &[u8],
    color: ColorSpace,
    fetch_chroma: impl Fn(usize, usize) -> [f32; 2],
) -> Vec<u8> {
    let rows = color.coefficients();
    let columns = chroma_taps(width, chroma_width);
    let lines = chroma_taps(height, chroma_height);
    let mut rgba = vec![0u8; width * height * 4];
    for (y, row) in rgba.chunks_exact_mut(width * 4).enumerate() {
        let (top, bottom, down) = lines[y];
        for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (left, right, across) = columns[x];
            let blend = |a: [f32; 2], b: [f32; 2], t: f32| {
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            };
            let [cb, cr] = blend(
                blend(fetch_chroma(left, top), fetch_chroma(right, top), across),
                blend(
                    fetch_chroma(left, bottom),
                    fetch_chroma(right, bottom),
                    across,
                ),
                down,
            );
            let sample = [
                f32::from(y_plane[y * width + x]) / 255.0,
                cb / 255.0,
                cr / 255.0,
            ];
            let channel = |r: [f32; 4]| {
                let value = r[0] * sample[0] + r[1] * sample[1] + r[2] * sample[2] + r[3];
                (value * 255.0).round().clamp(0.0, 255.0) as u8
            };
            *pixel = [channel(rows[0]), channel(rows[1]), channel(rows[2]), 255];
        }
    }
    rgba
}

fn chroma_taps(output: usize, source: usize) -> Vec<(usize, usize, f32)> {
    (0..output)
        .map(|index| {
            let position = (index as f32 + 0.5) * source as f32 / output as f32 - 0.5;
            let base = position.floor();
            let clamp = |value: f32| (value.max(0.0) as usize).min(source - 1);
            (clamp(base), clamp(base + 1.0), position - base)
        })
        .collect()
}

fn rotate_rgba(rgba: &[u8], width: u32, height: u32, rotation: u32) -> (u32, u32, Vec<u8>) {
    let (w, h) = (width as usize, height as usize);
    if rotation == 0 || rgba.len() != w * h * 4 {
        return (width, height, rgba.to_vec());
    }
    let (out_w, out_h) = if rotation == 90 || rotation == 270 {
        (h, w)
    } else {
        (w, h)
    };
    let mut out = vec![0u8; rgba.len()];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match rotation {
                90 => (h - 1 - y, x),
                180 => (w - 1 - x, h - 1 - y),
                270 => (y, w - 1 - x),
                _ => (x, y),
            };
            let source = (y * w + x) * 4;
            let target = (dy * out_w + dx) * 4;
            out[target..target + 4].copy_from_slice(&rgba[source..source + 4]);
        }
    }
    (out_w as u32, out_h as u32, out)
}

/// Описание одной текстурной плоскости для прямой загрузки в GPU (`queue.write_texture`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuPlaneUploadDescriptor {
    pub label: &'static str,
    pub binding: u32,
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub channels: u8,
    pub byte_offset: usize,
    pub byte_len: usize,
}

/// Дескриптор WGSL-шейдера и текстурных привязок для прямой загрузки YUV420 в GPU без кодирования в JPEG
/// (по образцу `Serein/apps/desktop/src/gpu_video.rs`).
#[derive(Clone, Debug, PartialEq)]
pub struct WgslVideoShaderDescriptor {
    pub label: &'static str,
    pub shader_source: &'static str,
    pub vertex_entry: &'static str,
    pub fragment_entry: &'static str,
    pub display_size: (u32, u32),
    pub uniform_bytes: [u8; 64],
    pub planes: Vec<GpuPlaneUploadDescriptor>,
    pub bytes_per_pixel_uploaded: f32,
}

impl WgslVideoShaderDescriptor {
    pub fn for_frame(frame: &VideoFrame) -> Result<Self, &'static str> {
        if !frame.is_valid() {
            return Err("Invalid video frame for GPU upload");
        }
        let (cw, ch) = frame.chroma_size();
        let luma_len = (frame.width as usize) * (frame.height as usize);
        let chroma_plane_len = (cw as usize) * (ch as usize);

        let (fragment_entry, planes, bytes_per_pixel_uploaded) = match frame.layout {
            PixelLayout::Nv12 => (
                "fs_nv12",
                vec![
                    GpuPlaneUploadDescriptor {
                        label: "video-luma",
                        binding: 2,
                        width: frame.width,
                        height: frame.height,
                        bytes_per_row: frame.width,
                        channels: 1,
                        byte_offset: 0,
                        byte_len: luma_len,
                    },
                    GpuPlaneUploadDescriptor {
                        label: "video-chroma",
                        binding: 3,
                        width: cw,
                        height: ch,
                        bytes_per_row: cw * 2,
                        channels: 2,
                        byte_offset: luma_len,
                        byte_len: chroma_plane_len * 2,
                    },
                ],
                1.5,
            ),
            PixelLayout::I420 => (
                "fs_i420",
                vec![
                    GpuPlaneUploadDescriptor {
                        label: "video-luma",
                        binding: 2,
                        width: frame.width,
                        height: frame.height,
                        bytes_per_row: frame.width,
                        channels: 1,
                        byte_offset: 0,
                        byte_len: luma_len,
                    },
                    GpuPlaneUploadDescriptor {
                        label: "video-cb",
                        binding: 3,
                        width: cw,
                        height: ch,
                        bytes_per_row: cw,
                        channels: 1,
                        byte_offset: luma_len,
                        byte_len: chroma_plane_len,
                    },
                    GpuPlaneUploadDescriptor {
                        label: "video-cr",
                        binding: 4,
                        width: cw,
                        height: ch,
                        bytes_per_row: cw,
                        channels: 1,
                        byte_offset: luma_len + chroma_plane_len,
                        byte_len: chroma_plane_len,
                    },
                ],
                1.5,
            ),
            PixelLayout::Rgba => {
                return Err("WGSL YUV shader descriptor requires planar NV12 or I420 frame");
            }
        };

        let mut uniform_bytes = [0u8; 64];
        for (idx, value) in frame.color.coefficients().into_iter().flatten().enumerate() {
            let start = idx * 4;
            uniform_bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        uniform_bytes[48..52].copy_from_slice(&u32::from(frame.rotation).to_le_bytes());

        Ok(Self {
            label: "video-yuv",
            shader_source: WGSL_YUV_SHADER,
            vertex_entry: "vs_main",
            fragment_entry,
            display_size: frame.display_size(),
            uniform_bytes,
            planes,
            bytes_per_pixel_uploaded,
        })
    }
}

pub mod capture;
pub use capture::{DesktopBackend, DesktopSource, DesktopSourceType, PixelFormat, ScreenCapturer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_codecs_match_client_log() {
        assert_eq!(ENGINE_CODECS.len(), 4);
        assert!(ENGINE_CODECS.contains(&VIDEO_CODEC));
    }

    #[test]
    fn latest_frame_slot_overwrites_stale_frame_and_reuses_buffer_for_60fps_poll() {
        let slot = LatestFrameSlot::new();
        assert!(slot.poll_latest().is_none());

        // Невалидный кадр (неверная длина данных) отклоняется и не затирает слот.
        let invalid = VideoFrame::new(4, 4, PixelLayout::Nv12, vec![0u8; 5]);
        assert!(!slot.store_frame(invalid));
        assert_eq!(slot.rejected_frames(), 1);
        assert_eq!(slot.stored_frames(), 0);

        // Записываем 3 кадра подряд без чтения UI-потоком: старые перезаписываются на месте без очереди.
        let f1 = VideoFrame::new(2, 2, PixelLayout::Nv12, vec![16, 16, 16, 16, 128, 128])
            .with_timestamp_us(16_666);
        let f2 = VideoFrame::new(2, 2, PixelLayout::Nv12, vec![80, 80, 80, 80, 128, 128])
            .with_timestamp_us(33_333);
        let f3 = VideoFrame::new(2, 2, PixelLayout::Nv12, vec![235, 235, 235, 235, 128, 128])
            .with_timestamp_us(50_000);

        assert!(slot.store_frame(f1));
        assert!(slot.store_frame(f2));
        assert!(slot.store_frame(f3.clone()));

        assert_eq!(slot.stored_frames(), 3);
        assert_eq!(slot.overwritten_frames(), 2);

        // При опросе на 60 FPS (16 мс) получаем сразу самый свежий 3-й кадр, без задержки очереди.
        assert!(slot.poll_latest_60fps(Duration::from_millis(8)).is_none());
        let polled = slot
            .poll_latest_60fps(FRAME_BUDGET_60FPS)
            .expect("latest frame must be returned at 16ms tick");
        assert_eq!(polled.timestamp_us, 50_000);
        assert_eq!(polled.data, f3.data);

        // Повторный опрос без нового кадра возвращает None.
        assert!(slot.poll_latest().is_none());
    }

    #[test]
    fn capture_encode_queue_drops_oldest_on_busy_encoder_and_handles_pli_fir_keyframe() {
        let queue = CaptureEncodeQueue::new();
        let make_frame = |ts: u64, luma: u8| {
            VideoFrame::new(
                2,
                2,
                PixelLayout::Nv12,
                vec![luma, luma, luma, luma, 128, 128],
            )
            .with_timestamp_us(ts)
        };

        // Канал sync_channel(2) вмещает 2 кадра без блокировки.
        assert!(queue.try_send(make_frame(1, 10)));
        assert!(queue.try_send(make_frame(2, 20)));
        assert_eq!(queue.sent_frames(), 2);
        assert_eq!(queue.dropped_frames(), 0);
        assert!(!queue.force_keyframe());

        // При занятости энкодера 3-й кадр вытесняет устаревший 1-й кадр без блокировки потока захвата,
        // увеличивает dropped_frames и взводит force_keyframe.
        assert!(!queue.try_send(make_frame(3, 30)));
        assert_eq!(queue.sent_frames(), 2);
        assert_eq!(queue.dropped_frames(), 1);
        assert!(queue.force_keyframe());
        assert!(queue.take_force_keyframe());
        assert!(!queue.force_keyframe());

        // В очереди остались кадры 2 и 3 (самый старый кадр 1 был отброшен).
        let first = queue.try_recv().expect("frame 2");
        let second = queue.try_recv().expect("frame 3");
        assert_eq!(first.timestamp_us, 2);
        assert_eq!(second.timestamp_us, 3);
        assert!(queue.try_recv().is_none());

        // Запрос RTCP PLI/FIR атомарно выставляет флаг принудительного ключевого кадра.
        queue.request_keyframe();
        assert!(queue.force_keyframe());
        assert!(queue.take_force_keyframe());
        assert!(!queue.take_force_keyframe());
    }

    #[test]
    fn rtx_history_ring_buffer_512_and_rfc4588_osn_packet_construction() {
        let mut history = RtxHistory::new();
        for seq in 0..600u16 {
            let pkt = RtpPacket::new(
                RTP_VIDEO_PAYLOAD_TYPE,
                seq % 5 == 0,
                seq,
                90_000 + u32::from(seq) * 3000,
                0x1122_3344,
                vec![0xaa, (seq & 0xff) as u8],
            );
            history.record(pkt);
        }

        // Кольцевой буфер строго ограничен 512 последними пакетами (88..=599).
        assert_eq!(history.len(), MAX_RTX_HISTORY_PACKETS);
        assert!(history.find_by_sequence(0).is_none());
        assert!(history.find_by_sequence(87).is_none());
        let found = history.find_by_sequence(88).expect("seq 88 must remain");
        assert_eq!(found.sequence_number(), 88);

        // Сборка RFC 4588 RTX-пакета с 2-байтным Original Sequence Number (OSN).
        let mut rtx_seq = 7000u16;
        let rtx_pkt = history
            .build_rtx_packet(595, 0x9988_7766, &mut rtx_seq)
            .expect("build RTX packet for seq 595");
        assert_eq!(rtx_seq, 7001);
        assert_eq!(rtx_pkt.payload_type(), RTP_RTX_PAYLOAD_TYPE);
        assert!(rtx_pkt.marker()); // 595 % 5 == 0 -> marker сохранён
        assert_eq!(rtx_pkt.sequence_number(), 7000);
        assert_eq!(rtx_pkt.ssrc(), 0x9988_7766);
        assert_eq!(&rtx_pkt.payload[0..2], &595u16.to_be_bytes());
        assert_eq!(&rtx_pkt.payload[2..], &[0xaa, (595u16 & 0xff) as u8]);

        // Обратная распаковка RTX возвращает оригинальный пакет видеопотока.
        let restored = RtxHistory::restore_rtx_packet(&rtx_pkt, 0x1122_3344).expect("restore RTX");
        assert_eq!(restored.payload_type(), RTP_VIDEO_PAYLOAD_TYPE);
        assert_eq!(restored.sequence_number(), 595);
        assert_eq!(restored.ssrc(), 0x1122_3344);
        assert_eq!(restored.payload, vec![0xaa, (595u16 & 0xff) as u8]);
    }

    #[test]
    fn adaptive_bitrate_controller_remb_bounds_and_smooth_recovery() {
        let mut abr = AdaptiveBitrateController::new(250_000, 4_000_000);
        assert_eq!(abr.target_bitrate(), 4_000_000);

        // При поступлении низкого RTCP REMB битрейт немедленно снижается до оценки.
        assert_eq!(abr.on_remb(1_000_000), 1_000_000);

        // Нижняя граница min_bitrate соблюдается при экстремальной просадке REMB.
        assert_eq!(abr.on_remb(50_000), 250_000);

        // При росте REMB битрейт восстанавливается плавно, а не скачком сразу до 4 Мбит/с.
        let step1 = abr.on_remb(4_000_000);
        assert!(step1 > 250_000 && step1 < 4_000_000, "step1={step1}");
        let step2 = abr.recover_step();
        assert!(step2 > step1 && step2 <= 4_000_000, "step2={step2}");

        // За серию шагов восстановления битрейт доходит до max_bitrate и не превышает его.
        for _ in 0..64 {
            abr.recover_step();
        }
        assert_eq!(abr.target_bitrate(), 4_000_000);
        assert_eq!(abr.on_remb(10_000_000), 4_000_000);

        // Потери пакетов (>= 5%, т.е. fraction_lost >= 13/256) снижают целевой битрейт на 20%.
        let after_loss = abr.on_packet_loss(26);
        assert_eq!(after_loss, 3_200_000);
    }

    #[test]
    fn packetize_h264_annex_b_single_nal_1180_and_fu_a_fragmentation_with_marker() {
        // Одиночный NAL ровно 1180 байт помещается в 1 Single NAL RTP-пакет с маркером M=1.
        let mut single_frame = vec![0, 0, 0, 1];
        single_frame.resize(4 + MAX_SINGLE_NAL_SIZE, 0x65);
        let mut seq = 100u16;
        let single_pkts =
            packetize_h264_annex_b(&single_frame, &mut seq, 90_000, 0xABCD).expect("single NAL");
        assert_eq!(single_pkts.len(), 1);
        assert_eq!(single_pkts[0].payload.len(), MAX_SINGLE_NAL_SIZE);
        assert!(single_pkts[0].marker());
        assert_eq!(single_pkts[0].sequence_number(), 100);
        assert_eq!(seq, 101);

        // Кадр из SPS (мал. NAL) + крупного IDR (2500 байт > 1180) фрагментируется через FU-A (type 28).
        let mut multi_frame = vec![0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1f];
        multi_frame.extend_from_slice(&[0, 0, 1, 0x65]);
        multi_frame.resize(multi_frame.len() + 2500, 0x5a);
        let mut seq = 500u16;
        let pkts =
            packetize_h264_annex_b(&multi_frame, &mut seq, 180_000, 0x1234).expect("FU-A packets");
        assert_eq!(pkts.len(), 4); // 1 SPS + 3 FU-A фрагмента

        // Маркер M=1 установлен ТОЛЬКО на самом последнем пакете кадра.
        for pkt in &pkts[..pkts.len() - 1] {
            assert!(!pkt.marker());
            assert!(pkt.payload.len() <= MAX_SINGLE_NAL_SIZE);
        }
        let last = pkts.last().unwrap();
        assert!(last.marker());
        assert!(last.payload.len() <= MAX_SINGLE_NAL_SIZE);

        // Проверяем заголовки FU-A: первый фрагмент S=1 (0x80), средний S=0/E=0, последний E=1 (0x40).
        assert_eq!(pkts[1].payload[0], (0x65 & 0xe0) | 28);
        assert_eq!(pkts[1].payload[1], (0x65 & 0x1f) | 0x80);
        assert_eq!(pkts[2].payload[0], (0x65 & 0xe0) | 28);
        assert_eq!(pkts[2].payload[1], 0x65 & 0x1f);
        assert_eq!(pkts[3].payload[0], (0x65 & 0xe0) | 28);
        assert_eq!(pkts[3].payload[1], (0x65 & 0x1f) | 0x40);
    }

    #[test]
    fn yuv_converters_nv12_and_i420_match_bt601_bt709_and_wgsl_shader_descriptor() {
        // Проверка чёрного (Y=16) и белого (Y=235) в limited-range BT.601 и красного в BT.709.
        let black_rgba = nv12_to_rgba(
            2,
            2,
            &[16, 16, 16, 16],
            &[128, 128],
            ColorSpace::BT601_LIMITED,
        )
        .expect("black nv12");
        assert_eq!(&black_rgba[0..4], &[0, 0, 0, 255]);

        let white_rgba = i420_to_rgba(
            2,
            2,
            &[235, 235, 235, 235],
            &[128],
            &[128],
            ColorSpace::BT709_LIMITED,
        )
        .expect("white i420");
        assert_eq!(&white_rgba[0..4], &[255, 255, 255, 255]);

        // BT.709 limited-range красный (Y=63, Cb=102, Cr=240): NV12 и I420 дают побайтово идентичный результат.
        let red_nv12 = nv12_to_rgba(
            2,
            2,
            &[63, 63, 63, 63],
            &[102, 240],
            ColorSpace::BT709_LIMITED,
        )
        .expect("red nv12");
        let red_i420 = i420_to_rgba(
            2,
            2,
            &[63, 63, 63, 63],
            &[102],
            &[240],
            ColorSpace::BT709_LIMITED,
        )
        .expect("red i420");
        assert_eq!(red_nv12, red_i420);
        assert!(red_nv12[0] >= 253 && red_nv12[1] <= 2 && red_nv12[2] <= 2);
        assert_eq!(red_nv12[3], 255);

        // Дескриптор WGSL-шейдера для прямой загрузки YUV420 на GPU без JPEG-перекодирования (1.5 байт/пиксель).
        let frame = VideoFrame::new(
            1920,
            1080,
            PixelLayout::Nv12,
            vec![0u8; 1920 * 1080 * 3 / 2],
        )
        .with_color(ColorSpace::BT709_LIMITED)
        .with_rotation(90);
        let desc = WgslVideoShaderDescriptor::for_frame(&frame).expect("wgsl descriptor");
        assert_eq!(desc.label, "video-yuv");
        assert_eq!(desc.vertex_entry, "vs_main");
        assert_eq!(desc.fragment_entry, "fs_nv12");
        assert!(desc.shader_source.contains("fn fs_nv12"));
        assert!(desc.shader_source.contains("fn fs_i420"));
        assert_eq!(desc.display_size, (1080, 1920));
        assert_eq!(desc.planes.len(), 2);
        assert_eq!(desc.planes[0].width, 1920);
        assert_eq!(desc.planes[0].height, 1080);
        assert_eq!(desc.planes[1].width, 960);
        assert_eq!(desc.planes[1].height, 540);
        assert_eq!(desc.planes[1].channels, 2);
        assert_eq!(
            u32::from_le_bytes(desc.uniform_bytes[48..52].try_into().unwrap()),
            90
        );
        assert!((desc.bytes_per_pixel_uploaded - 1.5).abs() < f32::EPSILON);

        // Проверка I420 WGSL-дескриптора (3 плоскости: Y, Cb, Cr).
        let i420_frame =
            VideoFrame::new(1280, 720, PixelLayout::I420, vec![0u8; 1280 * 720 * 3 / 2]);
        let i420_desc = WgslVideoShaderDescriptor::for_frame(&i420_frame).expect("i420 descriptor");
        assert_eq!(i420_desc.fragment_entry, "fs_i420");
        assert_eq!(i420_desc.planes.len(), 3);
        assert_eq!(i420_desc.planes[0].binding, 2);
        assert_eq!(i420_desc.planes[1].binding, 3);
        assert_eq!(i420_desc.planes[2].binding, 4);
        assert_eq!(i420_desc.planes[2].byte_offset, 1280 * 720 + 640 * 360);

        // Отказ WGSL YUV дескриптора для RGBA и невалидных кадров.
        let rgba_frame = VideoFrame::new(2, 2, PixelLayout::Rgba, vec![0u8; 16]);
        assert!(WgslVideoShaderDescriptor::for_frame(&rgba_frame).is_err());
    }

    #[test]
    fn h264_boundary_1180_vs_1181_depacketize_oracle_and_error_cases() {
        fn depacketize_oracle(packets: &[RtpPacket]) -> Vec<u8> {
            let mut out = Vec::new();
            let mut in_fu = false;
            for pkt in packets {
                let nal_type = pkt.payload[0] & 0x1f;
                if nal_type == 28 {
                    let s = (pkt.payload[1] & 0x80) != 0;
                    let e = (pkt.payload[1] & 0x40) != 0;
                    if s {
                        assert!(!in_fu);
                        out.extend_from_slice(&[0, 0, 0, 1]);
                        out.push((pkt.payload[0] & 0xe0) | (pkt.payload[1] & 0x1f));
                        in_fu = true;
                    } else {
                        assert!(in_fu);
                    }
                    out.extend_from_slice(&pkt.payload[2..]);
                    if e {
                        in_fu = false;
                    }
                } else {
                    assert!(!in_fu);
                    out.extend_from_slice(&[0, 0, 0, 1]);
                    out.extend_from_slice(&pkt.payload);
                }
            }
            assert!(!in_fu);
            out
        }

        // Граница 1181 байт -> ровно 2 FU-A пакета (1180 байт и 4 байта), с переходом sequence через u16::MAX.
        let mut boundary_frame = vec![0, 0, 0, 1, 0x65];
        for i in 0..MAX_SINGLE_NAL_SIZE {
            boundary_frame.push((i % 251) as u8);
        }
        assert_eq!(boundary_frame.len() - 4, MAX_SINGLE_NAL_SIZE + 1);

        let mut seq = u16::MAX;
        let pkts =
            packetize_h264_annex_b(&boundary_frame, &mut seq, 270_000, 77).expect("1181 NAL");
        assert_eq!(pkts.len(), 2);
        assert_eq!(pkts[0].sequence_number(), u16::MAX);
        assert_eq!(pkts[1].sequence_number(), 0);
        assert_eq!(seq, 1);
        assert_eq!(pkts[0].payload.len(), MAX_SINGLE_NAL_SIZE);
        assert_eq!(pkts[1].payload.len(), 4);
        assert!(!pkts[0].marker());
        assert!(pkts[1].marker());
        assert_eq!(depacketize_oracle(&pkts), boundary_frame);

        // Ошибки: не Annex-B, пустой NAL в конце, пустой буфер.
        assert!(packetize_h264_annex_b(&[], &mut seq, 0, 1).is_err());
        assert!(packetize_h264_annex_b(&[1, 2, 3, 4, 5], &mut seq, 0, 1).is_err());
        assert!(packetize_h264_annex_b(&[0, 0, 0, 1, 0x65, 1, 0, 0, 1], &mut seq, 0, 1).is_err());
    }

    #[test]
    fn bilinear_chroma_rotation_and_edge_cases_across_all_components() {
        // Нечётный размер 3x1 с поворотом 90 градусов: NV12 и I420 обязаны совпасть побайтово.
        let nv12 = VideoFrame::new(
            3,
            1,
            PixelLayout::Nv12,
            vec![16, 126, 235, 128, 128, 90, 240],
        )
        .with_color(ColorSpace::BT601_LIMITED)
        .with_rotation(90);
        let i420 = VideoFrame::new(
            3,
            1,
            PixelLayout::I420,
            vec![16, 126, 235, 128, 90, 128, 240],
        )
        .with_color(ColorSpace::BT601_LIMITED)
        .with_rotation(90);

        let (w1, h1, rgba1) = nv12.to_rgba().expect("nv12 3x1 rot 90");
        let (w2, h2, rgba2) = i420.to_rgba().expect("i420 3x1 rot 90");
        assert_eq!((w1, h1), (1, 3));
        assert_eq!((w2, h2), (1, 3));
        assert_eq!(rgba1, rgba2);
        assert_eq!(&rgba1[0..4], &[0, 0, 0, 255]);

        // Проверка ошибок размерностей YUV-конвертеров.
        assert!(nv12_to_rgba(0, 2, &[], &[], ColorSpace::BT601_LIMITED).is_err());
        assert!(nv12_to_rgba(2, 2, &[0; 3], &[0; 2], ColorSpace::BT601_LIMITED).is_err());
        assert!(i420_to_rgba(2, 2, &[0; 4], &[0; 2], &[0; 1], ColorSpace::BT709_LIMITED).is_err());

        // Границы потерь AdaptiveBitrateController: 12 (<5%) не снижает, 13 (>=5%) снижает на 20%.
        let mut abr = AdaptiveBitrateController::new(250_000, 1_000_000);
        assert_eq!(abr.on_packet_loss(12), 1_000_000);
        assert_eq!(abr.on_packet_loss(13), 800_000);

        // RtxHistory: ssrc=0 и не-RTX пакет отклоняются.
        let mut hist = RtxHistory::new();
        let pkt = RtpPacket::new(RTP_VIDEO_PAYLOAD_TYPE, false, 42, 90_000, 10, vec![1, 2, 3]);
        hist.record(pkt.clone());
        let mut rtx_seq = 1u16;
        assert!(hist.build_rtx_packet(42, 0, &mut rtx_seq).is_none());
        assert!(RtxHistory::restore_rtx_packet(&pkt, 10).is_none());
    }

    #[test]
    fn screen_capturer_backend_detection_and_frame_pipeline() {
        let capturer = ScreenCapturer::new();
        let sources = capturer.enumerate_sources();
        assert!(
            !sources.is_empty(),
            "Capturer must report at least 1 screen source"
        );
        let primary = sources
            .iter()
            .find(|s| s.is_primary)
            .expect("Primary source");
        assert_eq!(primary.source_type, DesktopSourceType::Screen);

        let frame = capturer
            .capture_frame(&primary.id, 1280, 720, 16_666)
            .expect("Capture 720p frame");
        assert!(frame.is_valid());
        assert_eq!(frame.width, 1280);
        assert_eq!(frame.height, 720);
        assert_eq!(frame.timestamp_us, 16_666);

        // Интеграция с LatestFrameSlot (60 FPS atomic slot)
        let slot = LatestFrameSlot::new();
        assert!(slot.store_frame(frame));
        assert_eq!(slot.stored_frames(), 1);
        let polled = slot.poll_latest().expect("Poll latest frame");
        assert!(polled.is_valid());
    }
}
