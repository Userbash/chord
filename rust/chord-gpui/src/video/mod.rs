//! Мост для аппаратного ускорения видеопотоков и рендеринга на GPU.
//! Интегрирует структуры дескрипторов видеокадров (NV12, I420, RGBA, BGRA) для 120 FPS рендеринга
//! и аппаратный коннектор PipeWire DMA-BUF (`SPA_DATA_DmaBuf`) для захвата экрана 60 FPS.

#![allow(dead_code)]

pub mod encoder;
pub mod sender;

use std::fmt;
use std::sync::{Arc, Mutex};

use chord_media::buffer::{LatestFrameRingBuffer, RingBufferMetrics};
use chord_media::capture::portal::{CropRect, PipeWireFrameBuffer, PipeWirePixelFormat};
use chord_media::{PixelLayout, VideoFrame};
use chord_platform::screencast::{
    CaptureResolutionPreset, CaptureTargetKind, CapturedFrame, LockFreeFrameSlot,
    NegotiatedStreamParams, PortalCursorMode, PortalPersistMode, PortalRequestError,
    PortalSourceTypes, PortalStreamInfo, RTP_VIDEO_CLOCK_HZ, ScreenCastCapturePipeline,
    ScreenCastNegotiationRequest, ScreenCastOptions, ScreenCastPipelineState, ScreenCastPortal,
    negotiate_stream_params,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use self::encoder::{
    MAX_ENCODER_DIMENSION, MIN_ENCODER_DIMENSION, RawVideoFrame, VideoEncoder, VideoEncoderConfig,
    VideoEncoderError, VideoFramePixelFormat,
};
use self::sender::{VideoRtpSender, VideoSenderError};

/// Целевая частота захвата экрана по контракту PipeWire DMA-BUF (60 FPS).
pub const CAPTURE_TARGET_FPS_60: u32 = 60;
/// Бюджет интервала кадра для 60 FPS (16 666 мкс ≈ 16.67 мс).
pub const FRAME_INTERVAL_60FPS_US: u64 = 1_000_000 / CAPTURE_TARGET_FPS_60 as u64;
/// Минимальный интервал кадра для 60 FPS с учётом 20% допуска джиттера (13 332 мкс).
pub const MIN_FRAME_INTERVAL_60FPS_US: u64 = (FRAME_INTERVAL_60FPS_US * 80) / 100;
/// Шаг временной метки RTP (90 кГц) на один кадр при 60 FPS (`90_000 / 60 = 1_500`).
pub const RTP_TIMESTAMP_STEP_60FPS: u32 = RTP_VIDEO_CLOCK_HZ / CAPTURE_TARGET_FPS_60;
/// Минимальный допустимый размер стороны кадра (в пикселях).
pub const MIN_CAPTURE_DIMENSION: u32 = 64;
/// Максимальная допустимая ширина 8K UHD.
pub const MAX_CAPTURE_WIDTH: u32 = 7680;
/// Максимальная допустимая высота 8K UHD.
pub const MAX_CAPTURE_HEIGHT: u32 = 4320;
/// DRM модификатор линейной раскладки (`DRM_FORMAT_MOD_LINEAR`).
pub const DRM_FORMAT_MOD_LINEAR: u64 = 0;
/// Невалидный DRM модификатор (`DRM_FORMAT_MOD_INVALID`), требующий fallback на Shm/MemFd.
pub const DRM_FORMAT_MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;
/// Код DRM FourCC для `BGRA8888` (`AR24`).
pub const DRM_FOURCC_BGRA8888: u32 = 0x3432_5241;
/// Код DRM FourCC для `RGBA8888` (`AB24`).
pub const DRM_FOURCC_RGBA8888: u32 = 0x3432_4241;
/// Код DRM FourCC для `NV12`.
pub const DRM_FOURCC_NV12: u32 = 0x3231_564E;
/// Код DRM FourCC для `YUV420` (`I420`).
pub const DRM_FOURCC_YUV420: u32 = 0x3231_5559;

/// Формат пикселей видеопотока
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoPixelFormat {
    Rgba8,
    Nv12,
    I420,
    Bgra8,
}

impl VideoPixelFormat {
    /// Конвертация в `PixelLayout` крейта `discord-media`.
    pub fn to_media_layout(self) -> PixelLayout {
        match self {
            Self::Rgba8 | Self::Bgra8 => PixelLayout::Rgba,
            Self::Nv12 => PixelLayout::Nv12,
            Self::I420 => PixelLayout::I420,
        }
    }

    /// Конвертация в `PipeWirePixelFormat` портала PipeWire (для RGB/BGR форматов).
    pub fn to_pipewire_format(self) -> PipeWirePixelFormat {
        match self {
            Self::Rgba8 | Self::Nv12 | Self::I420 => PipeWirePixelFormat::Rgba8,
            Self::Bgra8 => PipeWirePixelFormat::Bgra8,
        }
    }

    /// Конвертация из `PipeWirePixelFormat`.
    pub fn from_pipewire_format(fmt: PipeWirePixelFormat) -> Self {
        match fmt {
            PipeWirePixelFormat::Rgba8 | PipeWirePixelFormat::Rgbx8 => Self::Rgba8,
            PipeWirePixelFormat::Bgra8 | PipeWirePixelFormat::Bgrx8 => Self::Bgra8,
        }
    }

    /// Код DRM FourCC для экспорта DMA-BUF текстуры в WGPU/Vulkan.
    pub fn drm_fourcc(self) -> u32 {
        match self {
            Self::Bgra8 => DRM_FOURCC_BGRA8888,
            Self::Rgba8 => DRM_FOURCC_RGBA8888,
            Self::Nv12 => DRM_FOURCC_NV12,
            Self::I420 => DRM_FOURCC_YUV420,
        }
    }

    /// Минимальный шаг строки (stride в байтах) для заданной ширины.
    pub fn min_stride_bytes(self, width: u32) -> u32 {
        match self {
            Self::Rgba8 | Self::Bgra8 => width.saturating_mul(4),
            Self::Nv12 | Self::I420 => width,
        }
    }

    /// Требуется ли чётная размерность по ширине и высоте (для субдискретизации 4:2:0).
    pub fn requires_even_dimensions(self) -> bool {
        matches!(self, Self::Nv12 | Self::I420)
    }
}

/// Дескриптор плоскости аппаратного буфера DMA-BUF (`SPA_DATA_DmaBuf`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaBufPlaneDescriptor {
    pub dma_buf_fd: i32,
    pub offset_bytes: u32,
    pub stride_bytes: u32,
    pub size_bytes: usize,
    pub drm_fourcc: u32,
    pub drm_modifier: u64,
}

/// Конверт кадра конвейера PipeWire DMA-BUF (60 FPS) с метаданными синхронизации RTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaBufFrameEnvelope {
    pub seq: u64,
    pub node_id: u32,
    pub width: u32,
    pub height: u32,
    pub format: VideoPixelFormat,
    pub plane: DmaBufPlaneDescriptor,
    pub crop: Option<CropRect>,
    pub pts_us: u64,
    pub rtp_timestamp: u32,
    pub is_zero_copy_dma_buf: bool,
}

/// Снимок метрик наблюдаемости коннектора PipeWire DMA-BUF 60 FPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaBufConnectorMetrics {
    pub accepted_frames: u64,
    pub paced_drops: u64,
    pub ring_overwrites: u64,
    pub dma_buf_zero_copy_frames: u64,
    pub cpu_fallback_frames: u64,
    pub pli_retransmits: u64,
    pub last_rtp_timestamp: u32,
    pub last_pts_us: u64,
    pub target_fps: u32,
    pub frame_interval_us: u64,
    pub active_node_id: Option<u32>,
    pub is_zero_copy_active: bool,
    pub pipeline_state: ScreenCastPipelineState,
}

/// Дескриптор GPU-текстуры для отображения видеотрансляции / экрана
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuTextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub format: VideoPixelFormat,
    pub stride_bytes: u32,
    pub buffer_size_bytes: usize,
    pub is_hardware_accelerated: bool,
    #[serde(default)]
    pub dma_buf_fd: Option<i32>,
    #[serde(default)]
    pub drm_modifier: u64,
}

impl GpuTextureDescriptor {
    pub fn new(width: u32, height: u32, format: VideoPixelFormat) -> Self {
        let (stride, buffer_size) = match format {
            VideoPixelFormat::Rgba8 | VideoPixelFormat::Bgra8 => {
                let stride = width * 4;
                let size = (stride * height) as usize;
                (stride, size)
            }
            VideoPixelFormat::Nv12 | VideoPixelFormat::I420 => {
                let stride = width;
                // Y-плоскость (W * H) + UV-плоскость (W * H / 2) = W * H * 1.5
                let size = ((width * height * 3) / 2) as usize;
                (stride, size)
            }
        };

        Self {
            width,
            height,
            format,
            stride_bytes: stride,
            buffer_size_bytes: buffer_size,
            is_hardware_accelerated: true,
            dma_buf_fd: None,
            drm_modifier: DRM_FORMAT_MOD_LINEAR,
        }
    }

    /// Создание zero-copy дескриптора GPU-текстуры напрямую из дескриптора DMA-BUF.
    pub fn from_dma_buf(
        width: u32,
        height: u32,
        format: VideoPixelFormat,
        dma_buf_fd: i32,
        stride_bytes: u32,
        drm_modifier: u64,
    ) -> Result<Self, String> {
        if dma_buf_fd < 0 {
            return Err(format!(
                "Недопустимый файловый дескриптор DMA-BUF: {dma_buf_fd}"
            ));
        }
        if drm_modifier == DRM_FORMAT_MOD_INVALID {
            return Err("Недопустимый модификатор DRM_FORMAT_MOD_INVALID для DMA-BUF".into());
        }
        validate_capture_geometry(width, height, format, None)?;
        let min_stride = format.min_stride_bytes(width);
        if stride_bytes < min_stride {
            return Err(format!(
                "Недостаточный stride DMA-BUF: получено {stride_bytes}, минимум {min_stride}"
            ));
        }
        let buffer_size_bytes = compute_plane_size_bytes(stride_bytes, height, format);
        Ok(Self {
            width,
            height,
            format,
            stride_bytes,
            buffer_size_bytes,
            is_hardware_accelerated: true,
            dma_buf_fd: Some(dma_buf_fd),
            drm_modifier,
        })
    }

    /// Признак активного аппаратного импорта zero-copy через файловый дескриптор DMA-BUF.
    pub fn is_zero_copy_dma_buf(&self) -> bool {
        self.is_hardware_accelerated
            && self.dma_buf_fd.is_some_and(|fd| fd >= 0)
            && self.drm_modifier != DRM_FORMAT_MOD_INVALID
    }

    /// Проверка соответствия размера буфера кадра
    pub fn validate_buffer(&self, buffer: &[u8]) -> bool {
        buffer.len() >= self.buffer_size_bytes
    }
}

fn compute_plane_size_bytes(stride_bytes: u32, height: u32, format: VideoPixelFormat) -> usize {
    match format {
        VideoPixelFormat::Rgba8 | VideoPixelFormat::Bgra8 => {
            (stride_bytes as usize).saturating_mul(height as usize)
        }
        VideoPixelFormat::Nv12 | VideoPixelFormat::I420 => {
            ((stride_bytes as usize).saturating_mul(height as usize) * 3) / 2
        }
    }
}

fn validate_capture_geometry(
    width: u32,
    height: u32,
    format: VideoPixelFormat,
    crop: Option<CropRect>,
) -> Result<(u32, u32), String> {
    if !(MIN_CAPTURE_DIMENSION..=MAX_CAPTURE_WIDTH).contains(&width)
        || !(MIN_CAPTURE_DIMENSION..=MAX_CAPTURE_HEIGHT).contains(&height)
    {
        return Err(format!(
            "Разрешение кадра {width}x{height} вне допустимых границ ({MIN_CAPTURE_DIMENSION}..={MAX_CAPTURE_WIDTH}x{MAX_CAPTURE_HEIGHT})"
        ));
    }
    if format.requires_even_dimensions() && (width % 2 != 0 || height % 2 != 0) {
        return Err(format!(
            "Формат {format:?} (4:2:0) требует чётных размеров кадра, получено {width}x{height}"
        ));
    }

    if let Some(c) = crop {
        if c.width == 0 || c.height == 0 {
            return Err("Область обрезки CropRect не может иметь нулевые размеры".into());
        }
        let end_x =
            c.x.checked_add(c.width)
                .ok_or("Переполнение координаты X в CropRect")?;
        let end_y =
            c.y.checked_add(c.height)
                .ok_or("Переполнение координаты Y в CropRect")?;
        if end_x > width || end_y > height {
            return Err(format!(
                "Область обрезки CropRect ({}+{}x{}+{}) выходит за пределы кадра {width}x{height}",
                c.x, c.width, c.y, c.height
            ));
        }
        if format.requires_even_dimensions() && (c.width % 2 != 0 || c.height % 2 != 0) {
            return Err(format!(
                "Область обрезки для {format:?} должна иметь чётные размеры, получено {}x{}",
                c.width, c.height
            ));
        }
        return Ok((c.width, c.height));
    }

    Ok((width, height))
}

/// Аппаратный коннектор PipeWire DMA-BUF для захвата экрана 60 FPS.
///
/// Связывает согласование параметров PipeWire (`NegotiatedStreamParams`), `LockFreeFrameSlot`
/// (`discord-platform`) и `LatestFrameRingBuffer` (`discord-media`) в единый тракт 60 FPS.
#[derive(Clone)]
pub struct PipeWireDmaBufConnector {
    state: ScreenCastPipelineState,
    negotiated_params: Option<NegotiatedStreamParams>,
    frame_slot: Arc<LockFreeFrameSlot>,
    media_ring: LatestFrameRingBuffer,
    latest_envelope: Arc<Mutex<Option<DmaBufFrameEnvelope>>>,
    cached_encoder_frame: Arc<Mutex<Option<CapturedFrame>>>,
    pipewire_fd: Option<i32>,
    active_format: VideoPixelFormat,
    drm_modifier: u64,
    is_zero_copy_active: bool,
    accepted_frames: u64,
    paced_drops: u64,
    dma_buf_zero_copy_frames: u64,
    cpu_fallback_frames: u64,
    pli_retransmits: u64,
    last_rtp_timestamp: u32,
    last_pts_us: Option<u64>,
}

impl fmt::Debug for PipeWireDmaBufConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeWireDmaBufConnector")
            .field("state", &self.state)
            .field("negotiated_params", &self.negotiated_params)
            .field("pipewire_fd", &self.pipewire_fd)
            .field("active_format", &self.active_format)
            .field("drm_modifier", &self.drm_modifier)
            .field("is_zero_copy_active", &self.is_zero_copy_active)
            .field("accepted_frames", &self.accepted_frames)
            .field("paced_drops", &self.paced_drops)
            .field("dma_buf_zero_copy_frames", &self.dma_buf_zero_copy_frames)
            .field("cpu_fallback_frames", &self.cpu_fallback_frames)
            .field("pli_retransmits", &self.pli_retransmits)
            .field("last_rtp_timestamp", &self.last_rtp_timestamp)
            .field("last_pts_us", &self.last_pts_us)
            .field("ring_metrics", &self.media_ring.metrics())
            .finish()
    }
}

impl Default for PipeWireDmaBufConnector {
    fn default() -> Self {
        Self::new_60fps(3)
    }
}

impl PipeWireDmaBufConnector {
    /// Создаёт коннектор с целевой частотой 60 FPS и кольцевым буфером `LatestFrameRingBuffer`.
    pub fn new_60fps(ring_capacity: usize) -> Self {
        Self {
            state: ScreenCastPipelineState::Idle,
            negotiated_params: None,
            frame_slot: Arc::new(LockFreeFrameSlot::new()),
            media_ring: LatestFrameRingBuffer::with_slots(ring_capacity.max(2)),
            latest_envelope: Arc::new(Mutex::new(None)),
            cached_encoder_frame: Arc::new(Mutex::new(None)),
            pipewire_fd: None,
            active_format: VideoPixelFormat::Bgra8,
            drm_modifier: DRM_FORMAT_MOD_LINEAR,
            is_zero_copy_active: false,
            accepted_frames: 0,
            paced_drops: 0,
            dma_buf_zero_copy_frames: 0,
            cpu_fallback_frames: 0,
            pli_retransmits: 0,
            last_rtp_timestamp: 0,
            last_pts_us: None,
        }
    }

    /// Подключение потока PipeWire (`fd` из `OpenPipeWireRemote` и `node_id` из `Start`).
    pub fn connect_stream(
        &mut self,
        pipewire_fd: i32,
        node_id: u32,
        width: u32,
        height: u32,
        format: VideoPixelFormat,
        drm_modifier: u64,
    ) -> Result<NegotiatedStreamParams, String> {
        if pipewire_fd < 0 {
            return Err(format!(
                "Недопустимый файловый дескриптор сокета PipeWire: {pipewire_fd}"
            ));
        }
        if node_id == 0 {
            return Err("Недопустимый идентификатор узла PipeWire: node_id == 0".into());
        }
        validate_capture_geometry(width, height, format, None)?;

        let stream = PortalStreamInfo {
            node_id,
            id: Some(format!("pw-node-{node_id}")),
            source_type: PortalSourceTypes::MONITOR.bits(),
            position: Some((0, 0)),
            size: Some((width, height)),
            mapping_id: None,
        };
        let req = ScreenCastNegotiationRequest {
            max_width: width,
            max_height: height,
            target_fps: CAPTURE_TARGET_FPS_60,
            target_kind: CaptureTargetKind::Monitor,
            custom_bitrate_bps: None,
        };
        let mut params = negotiate_stream_params(&stream, &req).map_err(|e| e.to_string())?;
        params.width = width;
        params.height = height;
        params.stride_bytes = format.min_stride_bytes(width);

        self.pipewire_fd = Some(pipewire_fd);
        self.active_format = format;
        self.drm_modifier = drm_modifier;
        self.is_zero_copy_active = drm_modifier != DRM_FORMAT_MOD_INVALID;
        self.negotiated_params = Some(params.clone());
        self.last_pts_us = None;
        self.state = ScreenCastPipelineState::Streaming {
            node_id,
            fps: params.fps,
        };
        Ok(params)
    }

    /// Приём аппаратного кадра `SPA_DATA_DmaBuf` с контролем тайминга 60 FPS (16.67 мс) и области обрезки.
    #[allow(clippy::too_many_arguments)]
    pub fn ingest_dma_buf_frame(
        &mut self,
        dma_buf_fd: i32,
        offset_bytes: u32,
        stride_bytes: u32,
        drm_modifier: u64,
        crop: Option<CropRect>,
        pts_us: u64,
    ) -> Result<Option<DmaBufFrameEnvelope>, String> {
        let node_id = match self.state {
            ScreenCastPipelineState::Paused { .. } => return Ok(None),
            ScreenCastPipelineState::Streaming { node_id, .. } => node_id,
            _ => {
                return Err(
                    "Коннектор PipeWire DMA-BUF не находится в активном состоянии Streaming".into(),
                );
            }
        };

        if dma_buf_fd < 0 {
            return Err(format!(
                "Недопустимый файловый дескриптор кадра DMA-BUF: {dma_buf_fd}"
            ));
        }
        if drm_modifier == DRM_FORMAT_MOD_INVALID {
            return Err(
                "Модификатор DRM_FORMAT_MOD_INVALID не допускается для аппаратного DMA-BUF кадра"
                    .into(),
            );
        }

        let params = self
            .negotiated_params
            .as_ref()
            .ok_or("Параметры потока PipeWire не согласованы")?;
        let width = params.width;
        let height = params.height;
        let frame_interval_us = params.frame_interval_us;
        let rtp_step = params.rtp_timestamp_step;

        let min_stride = self.active_format.min_stride_bytes(width);
        if stride_bytes < min_stride {
            return Err(format!(
                "Шаг строки DMA-BUF ({stride_bytes} байт) меньше минимального ({min_stride} байт)"
            ));
        }

        let (out_w, out_h) = validate_capture_geometry(width, height, self.active_format, crop)?;

        // Пейсинг 60 FPS (интервал 16 666 мкс, допуск джиттера 20% = 13 332 мкс)
        let min_interval_us = (frame_interval_us * 80) / 100;
        if let Some(prev_pts) = self.last_pts_us
            && pts_us >= prev_pts
            && (pts_us - prev_pts) < min_interval_us
        {
            self.paced_drops += 1;
            return Ok(None);
        }

        self.last_pts_us = Some(pts_us);
        self.accepted_frames += 1;
        self.dma_buf_zero_copy_frames += 1;
        self.is_zero_copy_active = true;
        self.drm_modifier = drm_modifier;
        self.last_rtp_timestamp = self.last_rtp_timestamp.wrapping_add(rtp_step);
        let seq = self.accepted_frames;

        let size_bytes = compute_plane_size_bytes(stride_bytes, height, self.active_format);
        let fourcc = self.active_format.drm_fourcc();

        // Публикация zero-copy кадра в кольцевой буфер discord-media (< 50 МБ)
        let media_frame = VideoFrame::new_dma_buf(
            out_w,
            out_h,
            self.active_format.to_media_layout(),
            dma_buf_fd,
            drm_modifier,
        )
        .with_timestamp_us(pts_us);
        let _ = self.media_ring.store_frame(media_frame);

        // Компактный дескриптор DMA-BUF плоскости для неблокирующего слота аппаратного кодера
        let mut handle_tag = Vec::with_capacity(16);
        handle_tag.extend_from_slice(&dma_buf_fd.to_le_bytes());
        handle_tag.extend_from_slice(&offset_bytes.to_le_bytes());
        handle_tag.extend_from_slice(&drm_modifier.to_le_bytes());
        let mut cap_frame = CapturedFrame::new(out_w, out_h, stride_bytes, fourcc)
            .with_data(handle_tag)
            .with_timestamp_us(pts_us);
        cap_frame.drm_modifier = drm_modifier;
        cap_frame.sequence = seq;
        let _ = self.frame_slot.publish_frame(cap_frame.clone());
        if let Ok(mut guard) = self.cached_encoder_frame.lock() {
            *guard = Some(cap_frame);
        }

        let envelope = DmaBufFrameEnvelope {
            seq,
            node_id,
            width: out_w,
            height: out_h,
            format: self.active_format,
            plane: DmaBufPlaneDescriptor {
                dma_buf_fd,
                offset_bytes,
                stride_bytes,
                size_bytes,
                drm_fourcc: fourcc,
                drm_modifier,
            },
            crop,
            pts_us,
            rtp_timestamp: self.last_rtp_timestamp,
            is_zero_copy_dma_buf: true,
        };
        if let Ok(mut guard) = self.latest_envelope.lock() {
            *guard = Some(envelope.clone());
        }

        Ok(Some(envelope))
    }

    /// Приём кадра в режиме программного отката (`SPA_DATA_MemFd` / `MemPtr`), когда DMA-BUF недоступен.
    pub fn ingest_cpu_fallback_frame(
        &mut self,
        frame_bytes: &[u8],
        stride_bytes: u32,
        crop: Option<CropRect>,
        pts_us: u64,
    ) -> Result<Option<DmaBufFrameEnvelope>, String> {
        let node_id = match self.state {
            ScreenCastPipelineState::Paused { .. } => return Ok(None),
            ScreenCastPipelineState::Streaming { node_id, .. } => node_id,
            _ => {
                return Err(
                    "Коннектор PipeWire не находится в активном состоянии Streaming".into(),
                );
            }
        };

        let params = self
            .negotiated_params
            .as_ref()
            .ok_or("Параметры потока PipeWire не согласованы")?;
        let width = params.width;
        let height = params.height;
        let frame_interval_us = params.frame_interval_us;
        let rtp_step = params.rtp_timestamp_step;

        let min_stride = self.active_format.min_stride_bytes(width);
        if stride_bytes < min_stride {
            return Err(format!(
                "Шаг строки CPU буфера ({stride_bytes} байт) меньше минимального ({min_stride} байт)"
            ));
        }

        let (out_w, out_h) = validate_capture_geometry(width, height, self.active_format, crop)?;
        let required_bytes = compute_plane_size_bytes(stride_bytes, height, self.active_format);
        if frame_bytes.len() < required_bytes {
            return Err(format!(
                "Недостаточный размер CPU буфера PipeWire: ожидалось {required_bytes} байт, получено {}",
                frame_bytes.len()
            ));
        }

        let min_interval_us = (frame_interval_us * 80) / 100;
        if let Some(prev_pts) = self.last_pts_us
            && pts_us >= prev_pts
            && (pts_us - prev_pts) < min_interval_us
        {
            self.paced_drops += 1;
            return Ok(None);
        }

        self.last_pts_us = Some(pts_us);
        self.accepted_frames += 1;
        self.cpu_fallback_frames += 1;
        self.is_zero_copy_active = false;
        self.last_rtp_timestamp = self.last_rtp_timestamp.wrapping_add(rtp_step);
        let seq = self.accepted_frames;

        let layout = self.active_format.to_media_layout();
        if let Some(expected_media_len) = layout.byte_len(out_w, out_h) {
            let mut media_payload = Vec::with_capacity(expected_media_len);
            if frame_bytes.len() >= expected_media_len {
                media_payload.extend_from_slice(&frame_bytes[..expected_media_len]);
            } else {
                media_payload.resize(expected_media_len, 0);
            }
            let media_frame =
                VideoFrame::new(out_w, out_h, layout, media_payload).with_timestamp_us(pts_us);
            let _ = self.media_ring.store_frame(media_frame);
        }

        let fourcc = self.active_format.drm_fourcc();
        let mut cap_frame = CapturedFrame::new(out_w, out_h, stride_bytes, fourcc)
            .with_data(frame_bytes[..required_bytes].to_vec())
            .with_timestamp_us(pts_us);
        cap_frame.sequence = seq;
        let _ = self.frame_slot.publish_frame(cap_frame.clone());
        if let Ok(mut guard) = self.cached_encoder_frame.lock() {
            *guard = Some(cap_frame);
        }

        let envelope = DmaBufFrameEnvelope {
            seq,
            node_id,
            width: out_w,
            height: out_h,
            format: self.active_format,
            plane: DmaBufPlaneDescriptor {
                dma_buf_fd: -1,
                offset_bytes: 0,
                stride_bytes,
                size_bytes: required_bytes,
                drm_fourcc: fourcc,
                drm_modifier: DRM_FORMAT_MOD_INVALID,
            },
            crop,
            pts_us,
            rtp_timestamp: self.last_rtp_timestamp,
            is_zero_copy_dma_buf: false,
        };
        if let Ok(mut guard) = self.latest_envelope.lock() {
            *guard = Some(envelope.clone());
        }

        Ok(Some(envelope))
    }

    /// Приём универсального буфера `PipeWireFrameBuffer` из `discord-media`.
    pub fn ingest_pipewire_buffer(
        &mut self,
        buffer: &PipeWireFrameBuffer,
        pts_us: u64,
    ) -> Result<Option<DmaBufFrameEnvelope>, String> {
        let stride_u32 = u32::try_from(buffer.stride)
            .map_err(|_| "Переполнение значения stride в PipeWireFrameBuffer".to_string())?;
        if let Some(fd) = buffer.dma_buf_fd {
            self.ingest_dma_buf_frame(fd, 0, stride_u32, buffer.drm_modifier, buffer.crop, pts_us)
        } else {
            self.ingest_cpu_fallback_frame(buffer.data, stride_u32, buffer.crop, pts_us)
        }
    }

    /// Возвращает последний принятый конверт кадра DMA-BUF для рендера в GPUI.
    pub fn poll_latest_dma_buf_frame(&self) -> Option<DmaBufFrameEnvelope> {
        self.latest_envelope.lock().ok()?.clone()
    }

    /// Чтение кадра для аппаратного видеокодера с поддержкой повтора ключевого кадра по RTCP PLI/FIR.
    pub fn read_frame_for_encoder(&mut self, force_keyframe_pli: bool) -> Option<CapturedFrame> {
        if let Some(fresh) = self.frame_slot.with_latest_fresh(|frame| frame.clone()) {
            if let Ok(mut guard) = self.cached_encoder_frame.lock() {
                *guard = Some(fresh.clone());
            }
            return Some(fresh);
        }
        if force_keyframe_pli
            && let Ok(guard) = self.cached_encoder_frame.lock()
            && let Some(cached) = guard.clone()
        {
            self.pli_retransmits += 1;
            return Some(cached);
        }
        None
    }

    /// Zero-copy чтение последнего кадра из `LatestFrameRingBuffer` в рамках бюджета 60 FPS (16.6 мс).
    pub fn with_latest_media_frame<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame, u64) -> R,
    {
        let seq = self.accepted_frames;
        self.media_ring.with_latest_fresh(|frame| f(frame, seq))
    }

    /// Бесшовное переключение источника захвата (другой монитор или окно) без пересоздания портала.
    pub fn switch_source(
        &mut self,
        new_node_id: u32,
        width: u32,
        height: u32,
        format: VideoPixelFormat,
    ) -> Result<(), String> {
        let fd = self
            .pipewire_fd
            .ok_or("Невозможно переключить источник: сокет PipeWire не подключён")?;
        let was_paused = matches!(self.state, ScreenCastPipelineState::Paused { .. });
        let params =
            self.connect_stream(fd, new_node_id, width, height, format, self.drm_modifier)?;
        if was_paused {
            self.state = ScreenCastPipelineState::Paused {
                node_id: new_node_id,
                fps: params.fps,
            };
        }
        Ok(())
    }

    /// Пауза или возобновление приёма кадров PipeWire.
    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
        match self.state {
            ScreenCastPipelineState::Streaming { node_id, fps } if paused => {
                self.state = ScreenCastPipelineState::Paused { node_id, fps };
                Ok(())
            }
            ScreenCastPipelineState::Paused { node_id, fps } if !paused => {
                self.state = ScreenCastPipelineState::Streaming { node_id, fps };
                self.last_pts_us = None;
                Ok(())
            }
            ScreenCastPipelineState::Streaming { .. } | ScreenCastPipelineState::Paused { .. } => {
                Ok(())
            }
            _ => Err("Невозможно изменить статус паузы для неактивного коннектора PipeWire".into()),
        }
    }

    /// Остановка конвейера захвата и сброс активного дескриптора.
    pub fn stop(&mut self) {
        self.pipewire_fd = None;
        self.is_zero_copy_active = false;
        self.last_pts_us = None;
        self.state = ScreenCastPipelineState::Stopped;
    }

    /// Снимок метрик коннектора PipeWire DMA-BUF.
    pub fn metrics(&self) -> DmaBufConnectorMetrics {
        let ring_metrics: RingBufferMetrics = self.media_ring.metrics();
        let (target_fps, frame_interval_us, active_node_id) = match &self.negotiated_params {
            Some(p) => (p.fps, p.frame_interval_us, Some(p.node_id)),
            None => (CAPTURE_TARGET_FPS_60, FRAME_INTERVAL_60FPS_US, None),
        };
        DmaBufConnectorMetrics {
            accepted_frames: self.accepted_frames,
            paced_drops: self.paced_drops,
            ring_overwrites: ring_metrics.overwritten_frames,
            dma_buf_zero_copy_frames: self.dma_buf_zero_copy_frames,
            cpu_fallback_frames: self.cpu_fallback_frames,
            pli_retransmits: self.pli_retransmits,
            last_rtp_timestamp: self.last_rtp_timestamp,
            last_pts_us: self.last_pts_us.unwrap_or(0),
            target_fps,
            frame_interval_us,
            active_node_id,
            is_zero_copy_active: self.is_zero_copy_active,
            pipeline_state: self.state,
        }
    }
}

/// Конвейер GPU видеопотока
#[derive(Debug, Clone)]
pub struct GpuVideoPipeline {
    pub current_descriptor: Option<GpuTextureDescriptor>,
    pub frame_counter: u64,
    pub target_fps: u32,
    pub pipewire_fd: Option<i32>,
    pub pipewire_node_id: Option<u32>,
    pub dma_buf_connector: PipeWireDmaBufConnector,
}

impl Default for GpuVideoPipeline {
    fn default() -> Self {
        Self::new(120)
    }
}

impl GpuVideoPipeline {
    pub fn new(target_fps: u32) -> Self {
        Self {
            current_descriptor: None,
            frame_counter: 0,
            target_fps,
            pipewire_fd: None,
            pipewire_node_id: None,
            dma_buf_connector: PipeWireDmaBufConnector::new_60fps(3),
        }
    }

    /// Создаёт конвейер GPU, настроенный специально под захват экрана 60 FPS через PipeWire DMA-BUF.
    pub fn new_screencast_60fps() -> Self {
        Self::new(CAPTURE_TARGET_FPS_60)
    }

    /// Привязка PipeWire потока захвата экрана из XDG Desktop Portal
    pub fn connect_pipewire_stream(&mut self, fd: i32, node_id: u32) {
        self.pipewire_fd = Some(fd);
        self.pipewire_node_id = Some(node_id);
    }

    /// Полная привязка и согласование аппаратного PipeWire DMA-BUF потока 60 FPS.
    pub fn connect_pipewire_dma_buf_stream(
        &mut self,
        fd: i32,
        node_id: u32,
        width: u32,
        height: u32,
        format: VideoPixelFormat,
        drm_modifier: u64,
    ) -> Result<NegotiatedStreamParams, String> {
        let negotiated = self.dma_buf_connector.connect_stream(
            fd,
            node_id,
            width,
            height,
            format,
            drm_modifier,
        )?;
        self.pipewire_fd = Some(fd);
        self.pipewire_node_id = Some(node_id);

        if drm_modifier != DRM_FORMAT_MOD_INVALID {
            let stride = format.min_stride_bytes(width);
            self.current_descriptor = Some(GpuTextureDescriptor::from_dma_buf(
                width,
                height,
                format,
                fd,
                stride,
                drm_modifier,
            )?);
        } else {
            let mut desc = GpuTextureDescriptor::new(width, height, format);
            desc.is_hardware_accelerated = false;
            desc.drm_modifier = DRM_FORMAT_MOD_INVALID;
            self.current_descriptor = Some(desc);
        }

        Ok(negotiated)
    }

    /// Инициализация GPU-контекста под разрешение экрана или камеры
    pub fn configure(&mut self, width: u32, height: u32, format: VideoPixelFormat) {
        self.current_descriptor = Some(GpuTextureDescriptor::new(width, height, format));
    }

    /// Загрузка кадра на GPU (симуляция быстрой передачи текстуры в WGPU без копирования)
    pub fn push_frame(&mut self, frame_bytes: &[u8]) -> Result<u64, String> {
        let desc = self
            .current_descriptor
            .as_ref()
            .ok_or("Конвейер GPU не сконфигурирован")?;
        if !desc.validate_buffer(frame_bytes) {
            return Err(format!(
                "Недостаточный размер кадра: ожидалось {} байт, получено {}",
                desc.buffer_size_bytes,
                frame_bytes.len()
            ));
        }
        self.frame_counter += 1;
        Ok(self.frame_counter)
    }

    /// Прямая подача аппаратного кадра DMA-BUF в конвейер 60 FPS без копирования через CPU.
    pub fn push_dma_buf_frame(
        &mut self,
        dma_buf_fd: i32,
        offset_bytes: u32,
        stride_bytes: u32,
        drm_modifier: u64,
        crop: Option<CropRect>,
        pts_us: u64,
    ) -> Result<Option<DmaBufFrameEnvelope>, String> {
        let maybe_env = self.dma_buf_connector.ingest_dma_buf_frame(
            dma_buf_fd,
            offset_bytes,
            stride_bytes,
            drm_modifier,
            crop,
            pts_us,
        )?;
        if let Some(ref env) = maybe_env {
            self.frame_counter += 1;
            self.current_descriptor = Some(GpuTextureDescriptor::from_dma_buf(
                env.width,
                env.height,
                env.format,
                env.plane.dma_buf_fd,
                env.plane.stride_bytes,
                env.plane.drm_modifier,
            )?);
        }
        Ok(maybe_env)
    }

    /// Подача кадра в режиме fallback (`SPA_DATA_MemFd` / CPU) через коннектор 60 FPS.
    pub fn push_cpu_fallback_frame(
        &mut self,
        frame_bytes: &[u8],
        stride_bytes: u32,
        crop: Option<CropRect>,
        pts_us: u64,
    ) -> Result<Option<DmaBufFrameEnvelope>, String> {
        let maybe_env = self.dma_buf_connector.ingest_cpu_fallback_frame(
            frame_bytes,
            stride_bytes,
            crop,
            pts_us,
        )?;
        if let Some(ref env) = maybe_env {
            self.frame_counter += 1;
            let mut desc = GpuTextureDescriptor::new(env.width, env.height, env.format);
            desc.is_hardware_accelerated = false;
            desc.drm_modifier = DRM_FORMAT_MOD_INVALID;
            self.current_descriptor = Some(desc);
        }
        Ok(maybe_env)
    }

    /// Получение последнего готового DMA-BUF кадра для отрисовки в GPUI.
    pub fn poll_latest_dma_buf_frame(&self) -> Option<DmaBufFrameEnvelope> {
        self.dma_buf_connector.poll_latest_dma_buf_frame()
    }

    /// Снимок метрик аппаратного коннектора PipeWire DMA-BUF.
    pub fn connector_metrics(&self) -> DmaBufConnectorMetrics {
        self.dma_buf_connector.metrics()
    }
}

/// Статус и конфигурация активного захвата экрана
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenCastState {
    pub is_active: bool,
    pub source_type: String, // "Monitor" или "Window"
    pub source_name: String,
    pub fps: u32, // 30 или 60
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub pipewire_node_id: Option<u32>,
}

impl Default for ScreenCastState {
    fn default() -> Self {
        Self {
            is_active: false,
            source_type: "Monitor".into(),
            source_name: "Полный экран 1 (2560x1440)".into(),
            fps: 60,
            resolution_width: 1920,
            resolution_height: 1080,
            pipewire_node_id: None,
        }
    }
}

/// Источник видеозахвата для окна демонстрации экрана
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSourceOption {
    pub id: String,
    pub name: String,
    pub is_window: bool,
    pub thumbnail_color: String,
}

/// Менеджер демонстрации экрана через Linux XDG Desktop Portal и PipeWire
pub struct ScreenCastManager;

impl ScreenCastManager {
    /// Проверка доступности D-Bus сессии XDG Desktop Portal
    pub fn is_portal_available() -> bool {
        ScreenCastPortal::connect_session().is_ok()
    }

    /// Сборка параметров захвата экрана по эталону Serein portal_linux.rs
    pub fn build_portal_options(
        source_types: PortalSourceTypes,
        cursor_embedded: bool,
    ) -> ScreenCastOptions {
        ScreenCastOptions {
            handle_token: "discord_screencast_req".into(),
            session_handle_token: "discord_screencast_session".into(),
            source_types,
            multiple: false,
            cursor_mode: if cursor_embedded {
                PortalCursorMode::Embedded
            } else {
                PortalCursorMode::Metadata
            },
            persist_mode: PortalPersistMode::ApplicationRunning,
            restore_token: None,
            parent_window: String::new(),
        }
    }

    /// Согласование параметров потока PipeWire 60 FPS с приоритетом аппаратного DMA-BUF.
    pub fn negotiate_60fps_stream(
        node_id: u32,
        source_size: Option<(u32, u32)>,
        preset: CaptureResolutionPreset,
        is_window: bool,
    ) -> Result<NegotiatedStreamParams, String> {
        let target_kind = if is_window {
            CaptureTargetKind::Window
        } else {
            CaptureTargetKind::Monitor
        };
        let stream = PortalStreamInfo {
            node_id,
            id: Some(format!("pw-node-{node_id}")),
            source_type: target_kind.as_source_types().bits(),
            position: Some((0, 0)),
            size: source_size,
            mapping_id: None,
        };
        let req =
            ScreenCastNegotiationRequest::from_preset(preset, CAPTURE_TARGET_FPS_60, target_kind);
        negotiate_stream_params(&stream, &req).map_err(|e| e.to_string())
    }

    /// Инициализация сконфигурированного конвейера `GpuVideoPipeline` 60 FPS для выбранного узла PipeWire.
    pub fn create_dma_buf_pipeline_60fps(
        pipewire_fd: i32,
        stream_info: &PortalStreamInfo,
        format: VideoPixelFormat,
        drm_modifier: u64,
    ) -> Result<(GpuVideoPipeline, ScreenCastState), String> {
        let (width, height) = stream_info.size.unwrap_or((1920, 1080));
        let mut pipeline = GpuVideoPipeline::new_screencast_60fps();
        let negotiated = pipeline.connect_pipewire_dma_buf_stream(
            pipewire_fd,
            stream_info.node_id,
            width,
            height,
            format,
            drm_modifier,
        )?;

        let is_window = stream_info.source_type == PortalSourceTypes::WINDOW.bits();
        let source_type = if is_window { "Window" } else { "Monitor" }.to_string();
        let source_name = stream_info.id.clone().unwrap_or_else(|| {
            format!("{source_type} #{} ({width}x{height})", stream_info.node_id)
        });

        let state = ScreenCastState {
            is_active: true,
            source_type,
            source_name,
            fps: negotiated.fps,
            resolution_width: negotiated.width,
            resolution_height: negotiated.height,
            pipewire_node_id: Some(stream_info.node_id),
        };
        Ok((pipeline, state))
    }

    /// Получение списка доступных источников (экраны и окна)
    pub fn list_available_sources() -> Vec<ScreenSourceOption> {
        vec![
            ScreenSourceOption {
                id: "screen-1".into(),
                name: "Экран 1: 2560x1440 (Основной)".into(),
                is_window: false,
                thumbnail_color: "#2b2d31".into(),
            },
            ScreenSourceOption {
                id: "screen-2".into(),
                name: "Экран 2: 1920x1080 (Второй)".into(),
                is_window: false,
                thumbnail_color: "#1e1f22".into(),
            },
            ScreenSourceOption {
                id: "win-browser".into(),
                name: "Firefox / Chromium".into(),
                is_window: true,
                thumbnail_color: "#313338".into(),
            },
            ScreenSourceOption {
                id: "win-terminal".into(),
                name: "Terminal / Alacritty".into(),
                is_window: true,
                thumbnail_color: "#111214".into(),
            },
        ]
    }

    /// Запуск сессии захвата экрана через D-Bus портал
    pub fn start_screencast(
        source_types: PortalSourceTypes,
    ) -> Result<(String, Vec<PortalStreamInfo>), String> {
        let mut portal = ScreenCastPortal::connect_session()
            .map_err(|e| format!("Не удалось подключиться к сессионной шине D-Bus: {:?}", e))?;
        let options = Self::build_portal_options(source_types, true);
        let session_handle = portal
            .create_session(&options)
            .map_err(|e| format!("CreateSession error: {:?}", e))?;
        portal
            .select_sources(&session_handle, &options)
            .map_err(|e| format!("SelectSources error: {:?}", e))?;
        let (streams, _) = portal
            .start(&session_handle, &options)
            .map_err(|e| format!("Start error: {:?}", e))?;
        Ok((session_handle, streams))
    }
}

// ============================================================================
// P1-2.2: PipeWire / DMA-BUF -> `RawVideoFrame` Bridge
// ============================================================================

/// Ошибки моста конвертации `CapturedFrame` / `DmaBufPlaneDescriptor` в [`RawVideoFrame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameBridgeError {
    /// Неподдерживаемый код формата DRM FourCC.
    UnsupportedFourCc(u32),
    /// Разрешение кадра вне допустимых границ кодера (`64..=3840`).
    InvalidDimensions { width: u32, height: u32 },
    /// Нечётная ширина или высота для формата с субдискретизацией 4:2:0 (`NV12` / `I420`).
    OddSubsampledDimensions {
        width: u32,
        height: u32,
        format: VideoFramePixelFormat,
    },
    /// Шаг строки (`stride_bytes`) меньше минимально допустимого для ширины кадра.
    InvalidStride {
        stride_bytes: u32,
        minimum_stride_bytes: u32,
    },
    /// Отрицательный файловый дескриптор DMA-BUF (`fd < 0`).
    InvalidFd(i32),
    /// Невалидный DRM формат-модификатор (`DRM_FORMAT_MOD_INVALID`) для DMA-BUF плоскости.
    InvalidDrmModifier(u64),
    /// Пустой буфер пиксельной плоскости.
    EmptyPlaneBuffer,
    /// Усечённый буфер пиксельной плоскости (`actual_bytes < expected_bytes`).
    TruncatedPlaneBuffer {
        actual_bytes: usize,
        expected_bytes: usize,
    },
    /// Ошибка валидации упакованного [`RawVideoFrame`].
    EncoderFrameValidation(VideoEncoderError),
}

impl fmt::Display for FrameBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFourCc(fourcc) => {
                write!(f, "неподдерживаемый формат DRM FourCC: 0x{fourcc:08X}")
            }
            Self::InvalidDimensions { width, height } => {
                write!(
                    f,
                    "недопустимое разрешение кадра {width}x{height} (допустимо {MIN_ENCODER_DIMENSION}..={MAX_ENCODER_DIMENSION})"
                )
            }
            Self::OddSubsampledDimensions {
                width,
                height,
                format,
            } => write!(
                f,
                "формат {format:?} (4:2:0) требует чётных размеров кадра, получено {width}x{height}"
            ),
            Self::InvalidStride {
                stride_bytes,
                minimum_stride_bytes,
            } => write!(
                f,
                "недостаточный stride плоскости: получено {stride_bytes} байт, минимум {minimum_stride_bytes}"
            ),
            Self::InvalidFd(fd) => write!(f, "недопустимый файловый дескриптор DMA-BUF: {fd}"),
            Self::InvalidDrmModifier(modifier) => {
                write!(f, "недопустимый DRM модификатор: 0x{modifier:016X}")
            }
            Self::EmptyPlaneBuffer => write!(f, "пустой буфер пиксельной плоскости"),
            Self::TruncatedPlaneBuffer {
                actual_bytes,
                expected_bytes,
            } => write!(
                f,
                "усечённый буфер плоскости: получено {actual_bytes} байт, ожидалось не менее {expected_bytes}"
            ),
            Self::EncoderFrameValidation(err) => write!(f, "ошибка валидации кадра кодера: {err}"),
        }
    }
}

impl std::error::Error for FrameBridgeError {}

/// Сопоставляет код DRM FourCC с [`VideoFramePixelFormat`] кодера и флагом необходимости
/// перестановки каналов `BGRA -> RGBA` (`true` для `DRM_FOURCC_BGRA8888`).
pub fn map_drm_fourcc_to_pixel_format(
    drm_fourcc: u32,
) -> Result<(VideoFramePixelFormat, bool), FrameBridgeError> {
    match drm_fourcc {
        DRM_FOURCC_NV12 => Ok((VideoFramePixelFormat::Nv12, false)),
        DRM_FOURCC_YUV420 => Ok((VideoFramePixelFormat::I420, false)),
        DRM_FOURCC_RGBA8888 => Ok((VideoFramePixelFormat::Rgba8, false)),
        DRM_FOURCC_BGRA8888 => Ok((VideoFramePixelFormat::Rgba8, true)),
        other => Err(FrameBridgeError::UnsupportedFourCc(other)),
    }
}

/// Выполняет перестановку каналов `BGRA -> RGBA` на месте с учётом выравнивания шага строки (`stride_bytes`),
/// не затрагивая хвостовые байты отступа (`padding`) в конце каждой строки.
pub fn swizzle_bgra_to_rgba_in_place(
    buf: &mut [u8],
    width: u32,
    height: u32,
    stride_bytes: u32,
) -> Result<(), FrameBridgeError> {
    if !(MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&width)
        || !(MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&height)
    {
        return Err(FrameBridgeError::InvalidDimensions { width, height });
    }
    let min_stride = width.saturating_mul(4);
    if stride_bytes < min_stride {
        return Err(FrameBridgeError::InvalidStride {
            stride_bytes,
            minimum_stride_bytes: min_stride,
        });
    }
    if buf.is_empty() {
        return Err(FrameBridgeError::EmptyPlaneBuffer);
    }
    let expected_bytes = (stride_bytes as usize).saturating_mul(height as usize);
    if buf.len() < expected_bytes {
        return Err(FrameBridgeError::TruncatedPlaneBuffer {
            actual_bytes: buf.len(),
            expected_bytes,
        });
    }

    let row_active_bytes = min_stride as usize;
    let row_stride = stride_bytes as usize;
    for row in 0..(height as usize) {
        let row_start = row * row_stride;
        let active_row = &mut buf[row_start..row_start + row_active_bytes];
        for px in active_row.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }
    Ok(())
}

fn validate_and_pack_plane(
    width: u32,
    height: u32,
    stride_bytes: u32,
    drm_fourcc: u32,
    plane_bytes: &[u8],
    pts_us: u64,
    scratch: Option<&mut Vec<u8>>,
) -> Result<(RawVideoFrame, bool), FrameBridgeError> {
    let (pixel_format, needs_bgra_swizzle) = map_drm_fourcc_to_pixel_format(drm_fourcc)?;

    if !(MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&width)
        || !(MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&height)
    {
        return Err(FrameBridgeError::InvalidDimensions { width, height });
    }

    if pixel_format.requires_even_dimensions() && (width % 2 != 0 || height % 2 != 0) {
        return Err(FrameBridgeError::OddSubsampledDimensions {
            width,
            height,
            format: pixel_format,
        });
    }

    let min_stride = match pixel_format {
        VideoFramePixelFormat::Rgba8 => width.saturating_mul(4),
        VideoFramePixelFormat::Nv12 | VideoFramePixelFormat::I420 => width,
    };
    if stride_bytes < min_stride {
        return Err(FrameBridgeError::InvalidStride {
            stride_bytes,
            minimum_stride_bytes: min_stride,
        });
    }

    if plane_bytes.is_empty() {
        return Err(FrameBridgeError::EmptyPlaneBuffer);
    }

    let expected_strided_bytes = match pixel_format {
        VideoFramePixelFormat::Rgba8 => (stride_bytes as usize).saturating_mul(height as usize),
        VideoFramePixelFormat::Nv12 | VideoFramePixelFormat::I420 => {
            ((stride_bytes as usize).saturating_mul(height as usize) * 3) / 2
        }
    };
    if plane_bytes.len() < expected_strided_bytes {
        return Err(FrameBridgeError::TruncatedPlaneBuffer {
            actual_bytes: plane_bytes.len(),
            expected_bytes: expected_strided_bytes,
        });
    }

    let compact_len = pixel_format.expected_buffer_bytes(min_stride, height);
    let mut out = if let Some(buf) = scratch {
        buf.clear();
        if buf.capacity() < compact_len {
            buf.reserve(compact_len - buf.capacity());
        }
        std::mem::take(buf)
    } else {
        Vec::with_capacity(compact_len)
    };

    if stride_bytes == min_stride {
        out.extend_from_slice(&plane_bytes[..compact_len]);
    } else {
        let stride_usize = stride_bytes as usize;
        let min_stride_usize = min_stride as usize;
        match pixel_format {
            VideoFramePixelFormat::Rgba8 => {
                for row in 0..(height as usize) {
                    let start = row * stride_usize;
                    out.extend_from_slice(&plane_bytes[start..start + min_stride_usize]);
                }
            }
            VideoFramePixelFormat::Nv12 => {
                for row in 0..(height as usize) {
                    let start = row * stride_usize;
                    out.extend_from_slice(&plane_bytes[start..start + min_stride_usize]);
                }
                let uv_base = (height as usize) * stride_usize;
                for row in 0..((height as usize) / 2) {
                    let start = uv_base + row * stride_usize;
                    out.extend_from_slice(&plane_bytes[start..start + min_stride_usize]);
                }
            }
            VideoFramePixelFormat::I420 => {
                for row in 0..(height as usize) {
                    let start = row * stride_usize;
                    out.extend_from_slice(&plane_bytes[start..start + min_stride_usize]);
                }
                let uv_base = (height as usize) * stride_usize;
                let half_stride = stride_usize / 2;
                let half_width = min_stride_usize / 2;
                for row in 0..(height as usize) {
                    let start = uv_base + row * half_stride;
                    out.extend_from_slice(&plane_bytes[start..start + half_width]);
                }
            }
        }
    }

    if needs_bgra_swizzle {
        for px in out.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }

    let raw = RawVideoFrame::new_tight(width, height, pixel_format, pts_us, out);
    raw.validate()
        .map_err(FrameBridgeError::EncoderFrameValidation)?;
    Ok((raw, needs_bgra_swizzle))
}

/// Конвертирует захваченный кадр [`CapturedFrame`] (DMA-BUF или SHM/CPU) по ссылке в [`RawVideoFrame`].
pub fn convert_captured_frame_ref_to_raw_frame(
    frame: &CapturedFrame,
) -> Result<RawVideoFrame, FrameBridgeError> {
    if frame.is_dma_buf() {
        if let Some(fd) = frame.dma_buf_raw_fd()
            && fd < 0
        {
            return Err(FrameBridgeError::InvalidFd(fd));
        }
        if frame.drm_modifier == DRM_FORMAT_MOD_INVALID {
            return Err(FrameBridgeError::InvalidDrmModifier(frame.drm_modifier));
        }
    }
    let (raw, _) = validate_and_pack_plane(
        frame.width,
        frame.height,
        frame.stride,
        frame.format,
        &frame.data,
        frame.timestamp_us,
        None,
    )?;
    Ok(raw)
}

/// Конвертирует захваченный кадр [`CapturedFrame`] в [`RawVideoFrame`].
pub fn convert_captured_frame_to_raw_frame(
    frame: CapturedFrame,
) -> Result<RawVideoFrame, FrameBridgeError> {
    convert_captured_frame_ref_to_raw_frame(&frame)
}

/// Конвертирует плоскость аппаратного буфера [`DmaBufPlaneDescriptor`] и отображённый байтовый срез
/// в [`RawVideoFrame`] для подачи в [`VideoEncoder`].
pub fn convert_dma_buf_plane_to_raw_frame(
    width: u32,
    height: u32,
    plane: &DmaBufPlaneDescriptor,
    mapped_bytes: &[u8],
    pts_us: u64,
) -> Result<RawVideoFrame, FrameBridgeError> {
    if plane.dma_buf_fd < 0 {
        return Err(FrameBridgeError::InvalidFd(plane.dma_buf_fd));
    }
    if plane.drm_modifier == DRM_FORMAT_MOD_INVALID {
        return Err(FrameBridgeError::InvalidDrmModifier(plane.drm_modifier));
    }
    if mapped_bytes.is_empty() {
        return Err(FrameBridgeError::EmptyPlaneBuffer);
    }
    let offset = plane.offset_bytes as usize;
    if offset >= mapped_bytes.len() {
        return Err(FrameBridgeError::TruncatedPlaneBuffer {
            actual_bytes: 0,
            expected_bytes: plane.size_bytes.max(1),
        });
    }
    let slice = &mapped_bytes[offset..];
    let (raw, _) = validate_and_pack_plane(
        width,
        height,
        plane.stride_bytes,
        plane.drm_fourcc,
        slice,
        pts_us,
        None,
    )?;
    Ok(raw)
}

/// Счётчики телеметрии моста [`PipeWireFrameBridge`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipeWireFrameBridgeStats {
    /// Общее число успешно сконвертированных кадров.
    pub frames_converted: u64,
    /// Число кадров с аппаратным дескриптором DMA-BUF.
    pub dma_buf_zero_copy_frames: u64,
    /// Число кадров в режиме CPU/SHM fallback.
    pub shm_fallback_frames: u64,
    /// Число кадров, прошедших перестановку каналов `BGRA -> RGBA`.
    pub bgra_swizzled_frames: u64,
    /// Число повторных использований внутреннего буфера при конвертации.
    pub buffer_reuses: u64,
    /// Число отклонённых невалидных кадров.
    pub conversion_errors: u64,
}

/// Буферизующий мост конвертации кадров PipeWire DMA-BUF / SHM в [`RawVideoFrame`].
#[derive(Debug, Clone, Default)]
pub struct PipeWireFrameBridge {
    scratch: Vec<u8>,
    stats: PipeWireFrameBridgeStats,
}

impl PipeWireFrameBridge {
    /// Создаёт новый мост конвертации кадров с пулом переиспользования памяти.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scratch: Vec::new(),
            stats: PipeWireFrameBridgeStats {
                frames_converted: 0,
                dma_buf_zero_copy_frames: 0,
                shm_fallback_frames: 0,
                bgra_swizzled_frames: 0,
                buffer_reuses: 0,
                conversion_errors: 0,
            },
        }
    }

    /// Возвращает текущую статистику конвертации кадров.
    #[must_use]
    pub const fn stats(&self) -> &PipeWireFrameBridgeStats {
        &self.stats
    }

    /// Возвращает буфер кадра обратно в пул моста для последующего переиспользования без аллокаций.
    pub fn recycle_frame_buffer(&mut self, mut frame: RawVideoFrame) {
        frame.data.clear();
        if frame.data.capacity() > self.scratch.capacity() {
            self.scratch = frame.data;
        }
    }

    /// Конвертирует [`CapturedFrame`] в [`RawVideoFrame`] с переиспользованием буфера и учётом телеметрии.
    pub fn convert_captured_frame_reusing_buffer(
        &mut self,
        frame: &CapturedFrame,
    ) -> Result<RawVideoFrame, FrameBridgeError> {
        if frame.is_dma_buf() {
            if let Some(fd) = frame.dma_buf_raw_fd()
                && fd < 0
            {
                self.stats.conversion_errors += 1;
                return Err(FrameBridgeError::InvalidFd(fd));
            }
            if frame.drm_modifier == DRM_FORMAT_MOD_INVALID {
                self.stats.conversion_errors += 1;
                return Err(FrameBridgeError::InvalidDrmModifier(frame.drm_modifier));
            }
        }

        if self.scratch.capacity() == 0 {
            self.scratch.reserve(4096);
        }

        match validate_and_pack_plane(
            frame.width,
            frame.height,
            frame.stride,
            frame.format,
            &frame.data,
            frame.timestamp_us,
            Some(&mut self.scratch),
        ) {
            Ok((raw, swizzled)) => {
                self.stats.frames_converted += 1;
                self.stats.buffer_reuses += 1;
                if frame.is_dma_buf() {
                    self.stats.dma_buf_zero_copy_frames += 1;
                } else {
                    self.stats.shm_fallback_frames += 1;
                }
                if swizzled {
                    self.stats.bgra_swizzled_frames += 1;
                }
                self.scratch = Vec::with_capacity(raw.data.len());
                Ok(raw)
            }
            Err(err) => {
                self.stats.conversion_errors += 1;
                Err(err)
            }
        }
    }
}

// ============================================================================
// P1-2.5: End-to-End `ScreenSharePipelineController` & Gateway Opcodes 18/19/22
// ============================================================================

/// Discord Gateway Opcode 18: `STREAM_CREATE` (`docs/re/gateway-schema.md`, `docs/specs/STREAM-WAYLAND-GUIDE.md`).
pub const GATEWAY_OP_STREAM_CREATE: u8 = 18;
/// Discord Gateway Opcode 19: `STREAM_DELETE`.
pub const GATEWAY_OP_STREAM_DELETE: u8 = 19;
/// Discord Gateway Opcode 22: `STREAM_SET_PAUSED`.
pub const GATEWAY_OP_STREAM_SET_PAUSED: u8 = 22;

/// Адресат сигнализации демонстрации экрана Discord Go Live (серверный голосовой канал или звонок DM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSignalingTarget {
    /// Snowflake ID гильдии (`Some(guild_id)` для `"guild"`, `None` для `"call"`).
    pub guild_id: Option<String>,
    /// Snowflake ID голосового канала.
    pub channel_id: String,
    /// Snowflake ID локального пользователя.
    pub user_id: String,
    /// Предпочитаемый голосовой регион RTC (опционально).
    pub preferred_region: Option<String>,
}

impl StreamSignalingTarget {
    /// Создаёт цель демонстрации экрана в голосовом канале сервера (`type = "guild"`).
    #[must_use]
    pub fn guild(
        guild_id: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            guild_id: Some(guild_id.into()),
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            preferred_region: None,
        }
    }

    /// Создаёт цель демонстрации экрана в личном или групповом звонке (`type = "call"`).
    #[must_use]
    pub fn call(channel_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            guild_id: None,
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            preferred_region: None,
        }
    }

    /// Задаёт предпочитаемый голосовой регион (`preferred_region`).
    #[must_use]
    pub fn with_preferred_region(mut self, region: impl Into<String>) -> Self {
        self.preferred_region = Some(region.into());
        self
    }

    /// Проверяет непустоту идентификаторов `channel_id`, `user_id` и (если задан) `guild_id`.
    pub fn validate(&self) -> Result<(), String> {
        if self.channel_id.trim().is_empty() {
            return Err("channel_id не может быть пустым для Go Live стрима".to_string());
        }
        if self.user_id.trim().is_empty() {
            return Err("user_id не может быть пустым для Go Live стрима".to_string());
        }
        if let Some(ref gid) = self.guild_id
            && gid.trim().is_empty()
        {
            return Err("guild_id не может быть пустой строкой".to_string());
        }
        Ok(())
    }

    /// Формирует канонический ключ стрима (`"guild:<gid>:<cid>:<uid>"` или `"call:<cid>:<uid>"`).
    pub fn stream_key(&self) -> Result<String, String> {
        self.validate()?;
        match &self.guild_id {
            Some(gid) => Ok(format!("guild:{gid}:{}:{}", self.channel_id, self.user_id)),
            None => Ok(format!("call:{}:{}", self.channel_id, self.user_id)),
        }
    }
}

/// Валидирует формат `stream_key` (`"guild:<gid>:<cid>:<uid>"` или `"call:<cid>:<uid>"`).
pub fn validate_stream_key(stream_key: &str) -> Result<(), String> {
    let trimmed = stream_key.trim();
    if trimmed.is_empty() {
        return Err("stream_key не может быть пустым".to_string());
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.first().copied() {
        Some("guild") => {
            if parts.len() != 4 || parts.iter().any(|p| p.trim().is_empty()) {
                return Err(format!(
                    "некорректный формат guild stream_key '{stream_key}' (ожидалось 'guild:<gid>:<cid>:<uid>')"
                ));
            }
            Ok(())
        }
        Some("call") => {
            if parts.len() != 3 || parts.iter().any(|p| p.trim().is_empty()) {
                return Err(format!(
                    "некорректный формат call stream_key '{stream_key}' (ожидалось 'call:<cid>:<uid>')"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "неизвестный префикс stream_key '{stream_key}' (допустимы 'guild:' или 'call:')"
        )),
    }
}

/// Строит JSON-пакет Discord Gateway Opcode 18 (`STREAM_CREATE`).
pub fn build_gateway_stream_create_op18(target: &StreamSignalingTarget) -> Result<Value, String> {
    target.validate()?;
    let stream_type = if target.guild_id.is_some() {
        "guild"
    } else {
        "call"
    };
    Ok(json!({
        "op": GATEWAY_OP_STREAM_CREATE,
        "d": {
            "type": stream_type,
            "guild_id": target.guild_id,
            "channel_id": target.channel_id,
            "preferred_region": target.preferred_region,
        }
    }))
}

/// Строит JSON-пакет Discord Gateway Opcode 22 (`STREAM_SET_PAUSED`).
pub fn build_gateway_stream_set_paused_op22(
    stream_key: &str,
    paused: bool,
) -> Result<Value, String> {
    validate_stream_key(stream_key)?;
    Ok(json!({
        "op": GATEWAY_OP_STREAM_SET_PAUSED,
        "d": {
            "stream_key": stream_key,
            "paused": paused,
        }
    }))
}

/// Строит JSON-пакет Discord Gateway Opcode 19 (`STREAM_DELETE`).
pub fn build_gateway_stream_delete_op19(stream_key: &str) -> Result<Value, String> {
    validate_stream_key(stream_key)?;
    Ok(json!({
        "op": GATEWAY_OP_STREAM_DELETE,
        "d": {
            "stream_key": stream_key,
        }
    }))
}

/// Ошибки сквозного контроллера демонстрации экрана [`ScreenSharePipelineController`].
#[derive(Debug, Clone)]
pub enum ScreenSharePipelineError {
    /// Конвейер не запущен или уже остановлен.
    NotStreaming,
    /// Ошибка взаимодействия с D-Bus порталом `ScreenCast`.
    Portal(PortalRequestError),
    /// Ошибка моста конвертации кадра PipeWire / DMA-BUF.
    Bridge(FrameBridgeError),
    /// Ошибка видеокодера VP8/VP9.
    Encoder(VideoEncoderError),
    /// Ошибка RTP пакетизатора.
    Sender(VideoSenderError),
    /// Ошибка формирования Gateway сигнализации (Opcode 18/19/22).
    Signaling(String),
}

impl PartialEq for ScreenSharePipelineError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NotStreaming, Self::NotStreaming) => true,
            (Self::Portal(a), Self::Portal(b)) => a.to_string() == b.to_string(),
            (Self::Bridge(a), Self::Bridge(b)) => a == b,
            (Self::Encoder(a), Self::Encoder(b)) => a == b,
            (Self::Sender(a), Self::Sender(b)) => a == b,
            (Self::Signaling(a), Self::Signaling(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ScreenSharePipelineError {}

impl fmt::Display for ScreenSharePipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotStreaming => write!(f, "конвейер демонстрации экрана не активен"),
            Self::Portal(err) => write!(f, "ошибка портала ScreenCast: {err}"),
            Self::Bridge(err) => write!(f, "ошибка моста кадра PipeWire: {err}"),
            Self::Encoder(err) => write!(f, "ошибка видеокодера: {err}"),
            Self::Sender(err) => write!(f, "ошибка RTP отправителя: {err}"),
            Self::Signaling(msg) => write!(f, "ошибка сигнализации Gateway: {msg}"),
        }
    }
}

impl std::error::Error for ScreenSharePipelineError {}

impl From<PortalRequestError> for ScreenSharePipelineError {
    fn from(value: PortalRequestError) -> Self {
        Self::Portal(value)
    }
}

impl From<FrameBridgeError> for ScreenSharePipelineError {
    fn from(value: FrameBridgeError) -> Self {
        Self::Bridge(value)
    }
}

impl From<VideoEncoderError> for ScreenSharePipelineError {
    fn from(value: VideoEncoderError) -> Self {
        Self::Encoder(value)
    }
}

impl From<VideoSenderError> for ScreenSharePipelineError {
    fn from(value: VideoSenderError) -> Self {
        Self::Sender(value)
    }
}

/// Снимок наблюдаемости сквозного конвейера [`ScreenSharePipelineController`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenSharePipelineTelemetry {
    /// Текущее состояние конечного автомата конвейера захвата.
    pub state: ScreenCastPipelineState,
    /// Активный `stream_key` (если сессия активна).
    pub active_stream_key: Option<String>,
    /// Число захваченных и принятых в обработку кадров.
    pub frames_captured: u64,
    /// Число закодированных кадров.
    pub frames_encoded: u64,
    /// Число закодированных ключевых кадров (keyframes).
    pub keyframes_encoded: u64,
    /// Число закодированных межкадровых дельт (inter-frames).
    pub inter_frames_encoded: u64,
    /// Число принудительных ключевых кадров (по RTCP PLI или после снятия с паузы).
    pub forced_keyframes: u64,
    /// Число отправленных RTP-пакетов.
    pub rtp_packets_sent: u64,
    /// Суммарный объём отправленных RTP-пакетов в байтах.
    pub rtp_bytes_sent: u64,
    /// Число кадров, пропущенных во время паузы (`STREAM_SET_PAUSED`).
    pub paused_frames_skipped: u64,
    /// Число кадров с аппаратным дескриптором DMA-BUF.
    pub dma_buf_zero_copy_frames: u64,
    /// Число кадров в режиме CPU/SHM fallback.
    pub shm_fallback_frames: u64,
    /// Число кадров с перестановкой `BGRA -> RGBA`.
    pub bgra_swizzled_frames: u64,
    /// Число открытых дескрипторов сессии PipeWire.
    pub total_fds_opened: u64,
    /// Число закрытых дескрипторов сессии PipeWire.
    pub total_fds_closed: u64,
}

impl ScreenSharePipelineTelemetry {
    /// Возвращает текущий баланс открытых файловых дескрипторов (`0` после `stop()`).
    #[must_use]
    pub const fn active_fd_balance(&self) -> u64 {
        self.total_fds_opened.saturating_sub(self.total_fds_closed)
    }
}

/// Сквозной контроллер конвейера демонстрации экрана (`P1-2.5`):
/// `ScreenCastCapturePipeline` -> [`PipeWireFrameBridge`] -> [`VideoEncoder`] -> [`VideoRtpSender`]
/// + сигнализация Discord Gateway Opcodes `18` / `22` / `19`.
pub struct ScreenSharePipelineController {
    pipeline: ScreenCastCapturePipeline,
    bridge: PipeWireFrameBridge,
    encoder: VideoEncoder,
    sender: VideoRtpSender,
    state: ScreenCastPipelineState,
    negotiated_params: Option<NegotiatedStreamParams>,
    active_stream_key: Option<String>,
    is_paused: bool,
    frames_captured: u64,
    paused_frames_skipped: u64,
    total_fds_opened: u64,
    total_fds_closed: u64,
}

impl ScreenSharePipelineController {
    /// Создаёт новый сквозной контроллер демонстрации экрана с заданной конфигурацией кодера и RTP SSRC.
    pub fn new(
        encoder_config: VideoEncoderConfig,
        ssrc: u32,
        payload_type: u8,
    ) -> Result<Self, ScreenSharePipelineError> {
        let encoder = VideoEncoder::new(encoder_config)?;
        let sender = VideoRtpSender::new(ssrc, payload_type)?;
        Ok(Self {
            pipeline: ScreenCastCapturePipeline::new(),
            bridge: PipeWireFrameBridge::new(),
            encoder,
            sender,
            state: ScreenCastPipelineState::Idle,
            negotiated_params: None,
            active_stream_key: None,
            is_paused: false,
            frames_captured: 0,
            paused_frames_skipped: 0,
            total_fds_opened: 0,
            total_fds_closed: 0,
        })
    }

    /// Запускает сессию захвата через `ScreenCastPortal`, настраивает кодер под согласованное разрешение
    /// и возвращает `(NegotiatedStreamParams, Gateway Opcode 18 JSON)`.
    pub fn start(
        &mut self,
        portal: &mut ScreenCastPortal,
        options: &ScreenCastOptions,
        request: &ScreenCastNegotiationRequest,
        signaling: &StreamSignalingTarget,
    ) -> Result<(NegotiatedStreamParams, Value), ScreenSharePipelineError> {
        let stream_key = signaling
            .stream_key()
            .map_err(ScreenSharePipelineError::Signaling)?;
        let op18 = build_gateway_stream_create_op18(signaling)
            .map_err(ScreenSharePipelineError::Signaling)?;

        let params = self.pipeline.start_session(portal, options, request)?;
        self.sync_encoder_to_negotiated(&params)?;
        self.state = self.pipeline.state();
        self.negotiated_params = Some(params.clone());
        self.active_stream_key = Some(stream_key);
        self.is_paused = false;
        self.total_fds_opened += 1;

        Ok((params, op18))
    }

    /// Подключает уже запущенный экземпляр [`ScreenCastCapturePipeline`] (например, после UI `Go Live`)
    /// и формирует пакет сигнализации Gateway Opcode 18 (`STREAM_CREATE`).
    pub fn attach_started_pipeline(
        &mut self,
        pipeline: ScreenCastCapturePipeline,
        signaling: &StreamSignalingTarget,
    ) -> Result<Value, ScreenSharePipelineError> {
        let stream_key = signaling
            .stream_key()
            .map_err(ScreenSharePipelineError::Signaling)?;
        let op18 = build_gateway_stream_create_op18(signaling)
            .map_err(ScreenSharePipelineError::Signaling)?;

        if !matches!(
            pipeline.state(),
            ScreenCastPipelineState::Streaming { .. } | ScreenCastPipelineState::Paused { .. }
        ) {
            return Err(ScreenSharePipelineError::NotStreaming);
        }

        if let Some(params) = pipeline.negotiated_params() {
            self.sync_encoder_to_negotiated(params)?;
            self.negotiated_params = Some(params.clone());
        }
        self.state = pipeline.state();
        self.is_paused = matches!(self.state, ScreenCastPipelineState::Paused { .. });
        self.pipeline = pipeline;
        self.active_stream_key = Some(stream_key);
        self.total_fds_opened += 1;

        Ok(op18)
    }

    /// Запускает контроллер с заранее согласованными параметрами потока (для детерминированных контрактных тестов без D-Bus)
    /// и возвращает пакет Gateway Opcode 18 (`STREAM_CREATE`).
    pub fn start_with_negotiated(
        &mut self,
        params: NegotiatedStreamParams,
        signaling: &StreamSignalingTarget,
    ) -> Result<Value, ScreenSharePipelineError> {
        let stream_key = signaling
            .stream_key()
            .map_err(ScreenSharePipelineError::Signaling)?;
        let op18 = build_gateway_stream_create_op18(signaling)
            .map_err(ScreenSharePipelineError::Signaling)?;

        self.sync_encoder_to_negotiated(&params)?;
        self.state = ScreenCastPipelineState::Streaming {
            node_id: params.node_id,
            fps: params.fps,
        };
        self.negotiated_params = Some(params);
        self.active_stream_key = Some(stream_key);
        self.is_paused = false;
        self.total_fds_opened += 1;

        Ok(op18)
    }

    /// Переводит трансляцию на паузу или возобновляет её, возвращая пакет Gateway Opcode 22 (`STREAM_SET_PAUSED`).
    /// При возобновлении после паузы принудительно запрашивает ключевой кадр у [`VideoEncoder`].
    pub fn set_paused(&mut self, paused: bool) -> Result<Value, ScreenSharePipelineError> {
        let stream_key = self
            .active_stream_key
            .as_deref()
            .ok_or(ScreenSharePipelineError::NotStreaming)?;

        self.state = match self.state {
            ScreenCastPipelineState::Streaming { node_id, fps } if paused => {
                ScreenCastPipelineState::Paused { node_id, fps }
            }
            ScreenCastPipelineState::Paused { node_id, fps } if !paused => {
                ScreenCastPipelineState::Streaming { node_id, fps }
            }
            ScreenCastPipelineState::Streaming { .. } | ScreenCastPipelineState::Paused { .. } => {
                self.state
            }
            _ => return Err(ScreenSharePipelineError::NotStreaming),
        };

        if matches!(
            self.pipeline.state(),
            ScreenCastPipelineState::Streaming { .. } | ScreenCastPipelineState::Paused { .. }
        ) {
            let _ = self.pipeline.set_paused(paused);
        }

        if self.is_paused && !paused {
            self.encoder.request_keyframe();
        }
        self.is_paused = paused;

        build_gateway_stream_set_paused_op22(stream_key, paused)
            .map_err(ScreenSharePipelineError::Signaling)
    }

    /// Обрабатывает захваченный кадр [`CapturedFrame`]:
    /// - если поток на паузе, пропускает кодирование и возвращает пустой вектор пакетов;
    /// - иначе конвертирует кадр через [`PipeWireFrameBridge`], кодирует в [`VideoEncoder`]
    ///   и фрагментирует в RTP-пакеты через [`VideoRtpSender`].
    pub fn process_captured_frame(
        &mut self,
        frame: CapturedFrame,
    ) -> Result<Vec<Vec<u8>>, ScreenSharePipelineError> {
        if self.active_stream_key.is_none()
            || matches!(
                self.state,
                ScreenCastPipelineState::Idle | ScreenCastPipelineState::Stopped
            )
        {
            return Err(ScreenSharePipelineError::NotStreaming);
        }

        if self.is_paused || matches!(self.state, ScreenCastPipelineState::Paused { .. }) {
            self.paused_frames_skipped += 1;
            return Ok(Vec::new());
        }

        let raw_frame = self.bridge.convert_captured_frame_reusing_buffer(&frame)?;
        self.frames_captured += 1;

        if raw_frame.width != self.encoder.config().width
            || raw_frame.height != self.encoder.config().height
            || raw_frame.pixel_format != self.encoder.config().pixel_format
        {
            let mut updated = *self.encoder.config();
            updated.width = raw_frame.width;
            updated.height = raw_frame.height;
            updated.pixel_format = raw_frame.pixel_format;
            self.encoder.reconfigure(updated)?;
        }

        let encoded = self.encoder.encode_frame(&raw_frame)?;
        self.bridge.recycle_frame_buffer(raw_frame);
        let packets = self
            .sender
            .packetize_frame(&encoded.payload, encoded.timestamp_90khz)?;
        Ok(packets)
    }

    /// Опрашивает последний свежий кадр из `ScreenCastCapturePipeline::poll_latest_frame`
    /// и при наличии кодирует и пакетизирует его в RTP.
    pub fn poll_and_encode_latest_frame(
        &mut self,
    ) -> Result<Option<Vec<Vec<u8>>>, ScreenSharePipelineError> {
        let maybe_frame = self
            .pipeline
            .poll_latest_frame(|frame, _rtp_ts| frame.clone());
        match maybe_frame {
            Some(frame) => self.process_captured_frame(frame).map(Some),
            None => Ok(None),
        }
    }

    /// Перенаправляет запрос ключевого кадра (RTCP PLI / FIR) в [`VideoEncoder`].
    pub fn request_keyframe(&mut self) {
        self.encoder.request_keyframe();
    }

    /// Передает оценку пропускной способности (RTCP REMB / TWCC) в [`VideoEncoder`].
    pub fn on_bandwidth_estimate(&mut self, estimated_bps: u32) -> u32 {
        self.encoder.on_bandwidth_estimate(estimated_bps)
    }

    /// Останавливает демонстрацию экрана, закрывает сессию портала и файловый дескриптор PipeWire
    /// (`active_fd_balance() == 0`) и возвращает пакет Gateway Opcode 19 (`STREAM_DELETE`).
    pub fn stop(
        &mut self,
        portal: &mut ScreenCastPortal,
    ) -> Result<Value, ScreenSharePipelineError> {
        let stream_key = self
            .active_stream_key
            .take()
            .ok_or(ScreenSharePipelineError::NotStreaming)?;
        let op19 = build_gateway_stream_delete_op19(&stream_key)
            .map_err(ScreenSharePipelineError::Signaling)?;

        let stop_res = self.pipeline.stop(portal);
        portal.invalidate_pipewire_cache();
        self.state = ScreenCastPipelineState::Stopped;
        self.is_paused = false;
        self.total_fds_closed = self.total_fds_opened;

        stop_res?;
        Ok(op19)
    }

    /// Возвращает активный `stream_key` (если сессия запущена).
    #[must_use]
    pub fn active_stream_key(&self) -> Option<&str> {
        self.active_stream_key.as_deref()
    }

    /// Возвращает текущий баланс открытых файловых дескрипторов (`0` после `stop()`).
    #[must_use]
    pub const fn active_fd_balance(&self) -> u64 {
        self.total_fds_opened.saturating_sub(self.total_fds_closed)
    }

    /// Возвращает ссылку на внутренний [`VideoEncoder`].
    #[must_use]
    pub const fn encoder(&self) -> &VideoEncoder {
        &self.encoder
    }

    /// Возвращает ссылку на внутренний [`VideoRtpSender`].
    #[must_use]
    pub const fn sender(&self) -> &VideoRtpSender {
        &self.sender
    }

    /// Возвращает полный снимок телеметрии конвейера.
    #[must_use]
    pub fn telemetry_snapshot(&self) -> ScreenSharePipelineTelemetry {
        let enc_stats = self.encoder.stats();
        let snd_stats = self.sender.stats();
        let br_stats = self.bridge.stats();
        ScreenSharePipelineTelemetry {
            state: self.state,
            active_stream_key: self.active_stream_key.clone(),
            frames_captured: self.frames_captured,
            frames_encoded: enc_stats.frames_encoded,
            keyframes_encoded: enc_stats.keyframes_encoded,
            inter_frames_encoded: enc_stats.inter_frames_encoded,
            forced_keyframes: enc_stats.forced_keyframes,
            rtp_packets_sent: snd_stats.packets_sent,
            rtp_bytes_sent: snd_stats.bytes_sent,
            paused_frames_skipped: self.paused_frames_skipped,
            dma_buf_zero_copy_frames: br_stats.dma_buf_zero_copy_frames,
            shm_fallback_frames: br_stats.shm_fallback_frames,
            bgra_swizzled_frames: br_stats.bgra_swizzled_frames,
            total_fds_opened: self.total_fds_opened,
            total_fds_closed: self.total_fds_closed,
        }
    }

    fn sync_encoder_to_negotiated(
        &mut self,
        params: &NegotiatedStreamParams,
    ) -> Result<(), VideoEncoderError> {
        if (MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&params.width)
            && (MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&params.height)
        {
            let mut cfg = *self.encoder.config();
            cfg.width = params.width;
            cfg.height = params.height;
            cfg.fps = params.fps.clamp(1, 120);
            let target_bps = params
                .target_bitrate_bps
                .clamp(cfg.min_bitrate_bps, cfg.max_bitrate_bps);
            cfg.target_bitrate_bps = target_bps;
            self.encoder.reconfigure(cfg)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1. Позитивный сценарий: подключение PipeWire DMA-BUF потока 60 FPS и zero-copy подача кадров
    #[test]
    fn dma_buf_connector_60fps_positive_zero_copy_flow() {
        let mut pipeline = GpuVideoPipeline::new_screencast_60fps();
        assert_eq!(pipeline.target_fps, 60);

        let negotiated = pipeline
            .connect_pipewire_dma_buf_stream(
                42,
                108,
                1920,
                1080,
                VideoPixelFormat::Bgra8,
                DRM_FORMAT_MOD_LINEAR,
            )
            .expect("Поток PipeWire DMA-BUF должен успешно подключиться");

        assert_eq!(negotiated.fps, 60);
        assert_eq!(negotiated.frame_interval_us, 16_666);
        assert_eq!(negotiated.rtp_timestamp_step, 1_500);
        assert!(pipeline.connector_metrics().is_zero_copy_active);
        assert_eq!(pipeline.pipewire_fd, Some(42));
        assert_eq!(pipeline.pipewire_node_id, Some(108));

        let desc = pipeline
            .current_descriptor
            .as_ref()
            .expect("Дескриптор текстуры должен быть создан");
        assert!(desc.is_hardware_accelerated);
        assert!(desc.is_zero_copy_dma_buf());
        assert_eq!(desc.dma_buf_fd, Some(42));
        assert_eq!(desc.stride_bytes, 1920 * 4);
        assert_eq!(desc.buffer_size_bytes, 1920 * 1080 * 4);

        // Первый кадр (t = 100_000 мкс)
        let env1 = pipeline
            .push_dma_buf_frame(77, 0, 1920 * 4, DRM_FORMAT_MOD_LINEAR, None, 100_000)
            .expect("Валидный кадр DMA-BUF не должен возвращать ошибку")
            .expect("Первый кадр должен быть принят");

        assert_eq!(env1.seq, 1);
        assert_eq!(env1.node_id, 108);
        assert_eq!(env1.width, 1920);
        assert_eq!(env1.height, 1080);
        assert_eq!(env1.plane.dma_buf_fd, 77);
        assert_eq!(env1.plane.drm_fourcc, DRM_FOURCC_BGRA8888);
        assert_eq!(env1.rtp_timestamp, 1_500);
        assert!(env1.is_zero_copy_dma_buf);
        assert_eq!(pipeline.frame_counter, 1);

        // Второй кадр ровно через 16 666 мкс (60 FPS)
        let env2 = pipeline
            .push_dma_buf_frame(78, 0, 1920 * 4, DRM_FORMAT_MOD_LINEAR, None, 116_666)
            .expect("Валидный кадр DMA-BUF")
            .expect("Второй кадр через 16.67 мс должен быть принят");

        assert_eq!(env2.seq, 2);
        assert_eq!(env2.rtp_timestamp, 3_000);
        assert_eq!(pipeline.frame_counter, 2);

        let latest = pipeline
            .poll_latest_dma_buf_frame()
            .expect("Последний кадр должен быть доступен для GPUI");
        assert_eq!(latest.seq, 2);
        assert_eq!(latest.plane.dma_buf_fd, 78);

        // Чтение из LatestFrameRingBuffer в рамках бюджета 60 FPS
        let media_dims = pipeline
            .dma_buf_connector
            .with_latest_media_frame(|frame, seq| (frame.width, frame.height, frame.layout, seq))
            .expect("Кадр должен присутствовать в кольцевом буфере discord-media");
        assert_eq!(media_dims, (1920, 1080, PixelLayout::Rgba, 2));
    }

    // 2. Негативные сценарии и ошибки: невалидные FD, node_id, нечётные размеры NV12/I420, малый stride
    #[test]
    fn dma_buf_connector_rejects_invalid_fds_dimensions_and_strides() {
        let mut connector = PipeWireDmaBufConnector::new_60fps(3);

        // Невалидный fd < 0 при подключении потока
        assert!(
            connector
                .connect_stream(-1, 108, 1920, 1080, VideoPixelFormat::Bgra8, 0)
                .is_err()
        );

        // Невалидный node_id == 0
        assert!(
            connector
                .connect_stream(10, 0, 1920, 1080, VideoPixelFormat::Bgra8, 0)
                .is_err()
        );

        // Нечётные размеры для NV12/I420 (субдискретизация 4:2:0)
        assert!(
            connector
                .connect_stream(10, 108, 1921, 1080, VideoPixelFormat::Nv12, 0)
                .is_err()
        );
        assert!(
            connector
                .connect_stream(10, 108, 1920, 1081, VideoPixelFormat::I420, 0)
                .is_err()
        );

        // Подача кадра до подключения потока
        assert!(
            connector
                .ingest_dma_buf_frame(15, 0, 1920 * 4, 0, None, 10_000)
                .is_err()
        );

        // Подключаем валидный поток 1920x1080 BGRA8
        connector
            .connect_stream(10, 108, 1920, 1080, VideoPixelFormat::Bgra8, 0)
            .unwrap();

        // Отрицательный dma_buf_fd кадра
        assert!(
            connector
                .ingest_dma_buf_frame(-5, 0, 1920 * 4, 0, None, 20_000)
                .is_err()
        );

        // Невалидный DRM модификатор (DRM_FORMAT_MOD_INVALID) для DMA-BUF кадра
        assert!(
            connector
                .ingest_dma_buf_frame(15, 0, 1920 * 4, DRM_FORMAT_MOD_INVALID, None, 20_000)
                .is_err()
        );

        // Слишком маленький stride (< width * 4 для BGRA8)
        assert!(
            connector
                .ingest_dma_buf_frame(15, 0, 1920 * 4 - 4, 0, None, 20_000)
                .is_err()
        );
    }

    // 3. Граничные условия (Boundary): 64x64, 7680x4320 (8K), выход за границы и CropRect
    #[test]
    fn dma_buf_connector_boundary_dimensions_and_crop_validation() {
        let mut connector = PipeWireDmaBufConnector::new_60fps(3);

        // Ниже минимума 64x64
        assert!(
            connector
                .connect_stream(10, 42, 63, 64, VideoPixelFormat::Rgba8, 0)
                .is_err()
        );
        assert!(
            connector
                .connect_stream(10, 42, 64, 63, VideoPixelFormat::Rgba8, 0)
                .is_err()
        );

        // Выше максимума 7680x4320
        assert!(
            connector
                .connect_stream(10, 42, 7681, 4320, VideoPixelFormat::Rgba8, 0)
                .is_err()
        );
        assert!(
            connector
                .connect_stream(10, 42, 7680, 4321, VideoPixelFormat::Rgba8, 0)
                .is_err()
        );

        // Точная нижняя граница 64x64
        connector
            .connect_stream(10, 42, 64, 64, VideoPixelFormat::Nv12, 0)
            .expect("Граница 64x64 должна поддерживаться");

        // Точная верхняя граница 7680x4320 (8K)
        connector
            .connect_stream(10, 42, 7680, 4320, VideoPixelFormat::Nv12, 0)
            .expect("Граница 7680x4320 (8K) должна поддерживаться");

        // Проверка области обрезки окна (SPA_META_VideoCrop) на 1920x1080
        connector
            .connect_stream(10, 42, 1920, 1080, VideoPixelFormat::Bgra8, 0)
            .unwrap();

        let valid_crop = CropRect {
            x: 100,
            y: 80,
            width: 1820,
            height: 1000,
        };
        let env = connector
            .ingest_dma_buf_frame(21, 0, 1920 * 4, 0, Some(valid_crop), 50_000)
            .expect("Корректный CropRect должен приниматься")
            .expect("Кадр с валидным CropRect должен быть опубликован");
        assert_eq!(env.crop, Some(valid_crop));

        // Выход CropRect за правую/нижнюю границу кадра (100 + 1821 > 1920)
        let out_of_bounds_crop = CropRect {
            x: 100,
            y: 80,
            width: 1821,
            height: 1000,
        };
        assert!(
            connector
                .ingest_dma_buf_frame(21, 0, 1920 * 4, 0, Some(out_of_bounds_crop), 70_000)
                .is_err()
        );
    }

    // 4. Контроль частоты 60 FPS (Frame Pacing), пропуск избыточных кадров (144 Гц монитор -> 60 FPS) и пауза
    #[test]
    fn dma_buf_connector_60fps_frame_pacing_and_pause_lifecycle() {
        let mut connector = PipeWireDmaBufConnector::new_60fps(3);
        connector
            .connect_stream(12, 77, 2560, 1440, VideoPixelFormat::Nv12, 0)
            .unwrap();

        // Кадр #1 в t = 1_000_000 мкс -> Принят
        let f1 = connector
            .ingest_dma_buf_frame(30, 0, 2560, 0, None, 1_000_000)
            .unwrap();
        assert!(f1.is_some());

        // Слишком ранний кадр через 6 944 мкс (144 Гц монитор < 13 332 мкс порога) -> Отброшен пейсером
        let f_early = connector
            .ingest_dma_buf_frame(31, 0, 2560, 0, None, 1_006_944)
            .unwrap();
        assert!(f_early.is_none());

        // Кадр на границе 20% допуска джиттера (1_000_000 + 13_332 = 1_013_332 мкс) -> Принят
        let f2 = connector
            .ingest_dma_buf_frame(32, 0, 2560, 0, None, 1_013_332)
            .unwrap();
        assert!(f2.is_some());

        // Перевод на паузу: кадры игнорируются без ошибок
        connector.set_paused(true).unwrap();
        assert_eq!(
            connector.metrics().pipeline_state,
            ScreenCastPipelineState::Paused {
                node_id: 77,
                fps: 60
            }
        );
        let f_paused = connector
            .ingest_dma_buf_frame(33, 0, 2560, 0, None, 1_050_000)
            .unwrap();
        assert!(f_paused.is_none());

        // Возобновление потока
        connector.set_paused(false).unwrap();
        assert_eq!(
            connector.metrics().pipeline_state,
            ScreenCastPipelineState::Streaming {
                node_id: 77,
                fps: 60
            }
        );
        let f3 = connector
            .ingest_dma_buf_frame(34, 0, 2560, 0, None, 1_070_000)
            .unwrap();
        assert!(f3.is_some());

        let metrics = connector.metrics();
        assert_eq!(metrics.accepted_frames, 3);
        assert_eq!(metrics.paced_drops, 1);
        assert_eq!(metrics.dma_buf_zero_copy_frames, 3);
        assert_eq!(metrics.last_rtp_timestamp, 4_500);
    }

    // 5. Восстановление и Fallback: деградация DMA-BUF -> CPU MemFd, повтор кадра по RTCP PLI и смена источника
    #[test]
    fn dma_buf_connector_cpu_fallback_pli_keyframe_and_source_switch() {
        let mut connector = PipeWireDmaBufConnector::new_60fps(3);

        // Подключение с DRM_FORMAT_MOD_INVALID инициирует fallback на CPU/MemFd
        let _params = connector
            .connect_stream(
                14,
                200,
                1280,
                720,
                VideoPixelFormat::Rgba8,
                DRM_FORMAT_MOD_INVALID,
            )
            .expect("При DRM_FORMAT_MOD_INVALID коннектор должен переключиться в Shm fallback");
        assert!(!connector.metrics().is_zero_copy_active);

        let raw_bytes = vec![0xAAu8; 1280 * 720 * 4];
        let env = connector
            .ingest_cpu_fallback_frame(&raw_bytes, 1280 * 4, None, 500_000)
            .expect("Fallback кадр должен успешно обрабатываться")
            .expect("Fallback кадр должен быть принят");
        assert!(!env.is_zero_copy_dma_buf);
        assert_eq!(connector.metrics().cpu_fallback_frames, 1);

        // Чтение для аппаратного кодера: первый раз забирает свежий кадр, второй раз по PLI возвращает кэшированный
        let enc_frame_1 = connector
            .read_frame_for_encoder(false)
            .expect("Кодер должен получить свежий кадр");
        assert_eq!(enc_frame_1.sequence, 1);
        assert!(connector.read_frame_for_encoder(false).is_none());

        let pli_frame = connector
            .read_frame_for_encoder(true)
            .expect("При запросе RTCP PLI кодер должен получить последний кадр повторно");
        assert_eq!(pli_frame.sequence, 1);
        assert_eq!(connector.metrics().pli_retransmits, 1);

        // Горячее переключение источника (Monitor -> Window 2560x1440 NV12)
        connector
            .switch_source(305, 2560, 1440, VideoPixelFormat::Nv12)
            .expect("Переключение источника должно пройти без ошибок");
        assert_eq!(connector.metrics().active_node_id, Some(305));

        // Проверка фабрики ScreenCastManager::create_dma_buf_pipeline_60fps
        let stream_info = PortalStreamInfo {
            node_id: 512,
            id: Some("screen-main".into()),
            source_type: PortalSourceTypes::MONITOR.bits(),
            position: Some((0, 0)),
            size: Some((1920, 1080)),
            mapping_id: None,
        };
        let (mut gpu_pipe, state) = ScreenCastManager::create_dma_buf_pipeline_60fps(
            55,
            &stream_info,
            VideoPixelFormat::Bgra8,
            DRM_FORMAT_MOD_LINEAR,
        )
        .expect("Фабрика 60 FPS конвейера должна создать сконфигурированный пайплайн");
        assert!(state.is_active);
        assert_eq!(state.fps, 60);
        assert_eq!(state.pipewire_node_id, Some(512));
        assert_eq!(gpu_pipe.pipewire_node_id, Some(512));

        // Подача CPU fallback кадра через GpuVideoPipeline::push_cpu_fallback_frame
        let fallback_buf = vec![0x55u8; 1920 * 1080 * 4];
        let fb_env = gpu_pipe
            .push_cpu_fallback_frame(&fallback_buf, 1920 * 4, None, 2_000_000)
            .expect("push_cpu_fallback_frame должен работать")
            .expect("CPU fallback кадр должен быть принят");
        assert!(!fb_env.is_zero_copy_dma_buf);
        assert!(
            !gpu_pipe
                .current_descriptor
                .as_ref()
                .unwrap()
                .is_hardware_accelerated
        );

        // Приём PipeWireFrameBuffer (DMA-BUF и CPU) и остановка коннектора
        let pw_dma_buf = PipeWireFrameBuffer::new_dma_buf(
            2560,
            1440,
            2560,
            VideoPixelFormat::Nv12.to_pipewire_format(),
            88,
            DRM_FORMAT_MOD_LINEAR,
            None,
        );
        assert_eq!(
            VideoPixelFormat::from_pipewire_format(pw_dma_buf.format),
            VideoPixelFormat::Rgba8
        );
        let pw_env = connector
            .ingest_pipewire_buffer(&pw_dma_buf, 900_000)
            .expect("ingest_pipewire_buffer DMA-BUF")
            .expect("DMA-BUF из PipeWireFrameBuffer должен быть принят");
        assert!(pw_env.is_zero_copy_dma_buf);
        assert_eq!(pw_env.plane.dma_buf_fd, 88);

        let negotiated_60 = ScreenCastManager::negotiate_60fps_stream(
            700,
            Some((2560, 1440)),
            CaptureResolutionPreset::Qhd1440p,
            false,
        )
        .expect("Согласование 1440p60 должно пройти успешно");
        assert_eq!(negotiated_60.fps, 60);
        assert_eq!(negotiated_60.width, 2560);
        assert_eq!(negotiated_60.height, 1440);

        connector.stop();
        assert_eq!(
            connector.metrics().pipeline_state,
            ScreenCastPipelineState::Stopped
        );
        assert!(!connector.metrics().is_zero_copy_active);
    }

    // 6. Регрессионная совместимость с существующими контрактами GpuVideoPipeline (120 FPS рендер) и ScreenCastManager
    #[test]
    fn legacy_gpu_video_pipeline_and_screencast_contract_compat() {
        let mut pipeline = GpuVideoPipeline::default();
        assert_eq!(pipeline.target_fps, 120);
        assert_eq!(pipeline.frame_counter, 0);
        assert!(pipeline.current_descriptor.is_none());

        assert!(pipeline.push_frame(&[0u8; 64]).is_err());

        pipeline.connect_pipewire_stream(42, 108);
        assert_eq!(pipeline.pipewire_fd, Some(42));
        assert_eq!(pipeline.pipewire_node_id, Some(108));

        pipeline.configure(2560, 1440, VideoPixelFormat::Rgba8);
        let desc = pipeline.current_descriptor.as_ref().unwrap();
        assert_eq!(desc.width, 2560);
        assert_eq!(desc.height, 1440);
        assert_eq!(desc.stride_bytes, 2560 * 4);
        assert_eq!(desc.buffer_size_bytes, 2560 * 1440 * 4);
        assert!(desc.is_hardware_accelerated);

        let valid_frame = vec![128u8; 2560 * 1440 * 4];
        assert_eq!(pipeline.push_frame(&valid_frame).unwrap(), 1);
        assert!(pipeline.push_frame(&vec![0u8; 1024]).is_err());

        let nv12 = GpuTextureDescriptor::new(1920, 1080, VideoPixelFormat::Nv12);
        assert_eq!(nv12.buffer_size_bytes, (1920 * 1080 * 3) / 2);
        let i420 = GpuTextureDescriptor::new(1280, 720, VideoPixelFormat::I420);
        assert_eq!(i420.buffer_size_bytes, (1280 * 720 * 3) / 2);

        let sources = ScreenCastManager::list_available_sources();
        assert_eq!(sources.len(), 4);
        let opts = ScreenCastManager::build_portal_options(PortalSourceTypes::MONITOR, false);
        assert_eq!(opts.cursor_mode, PortalCursorMode::Metadata);
    }
}
