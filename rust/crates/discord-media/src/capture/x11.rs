//! Захват окон и мониторов через X11 MIT-SHM (`XShmGetImage` / `XServerPixelBuffer`) для fallback.
//!
//! Оракул: `/home/sanya/re-deep/native-re.md` (`discord_voice.node` ->
//! `webrtc::ScreenCapturerX11`, `webrtc::WindowCapturerX11`, `webrtc::XServerPixelBuffer`,
//! `webrtc::XAtomCache`, `"Failed to open Wayland display, falling back to X11"`).

use crate::capture::portal::{CropRect, PipeWireFrameBuffer, PipeWirePixelFormat};
use crate::capture::{CaptureError, DesktopSource, DesktopSourceType, MAX_CAPTURE_DIMENSION};
use crate::{PixelLayout, VideoFrame};

/// Режим работы захвата X11 и цепочка деградации (fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X11CaptureMode {
    /// Высокопроизводительный захват через общую память `MIT-SHM` (`XShmGetImage`).
    MitShm,
    /// Резервный захват через сокет X11 (`XGetImage`), когда `MIT-SHM` недоступен.
    XGetImageFallback,
    /// Синтетический fallback при полной недоступности дисплея X11.
    SyntheticFallback,
}

/// Геометрия и метаданные монитора XRandR / Xinerama (`webrtc::ScreenCapturerX11`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11MonitorGeometry {
    /// Уникальный идентификатор источника (например, `"screen:0"`).
    pub id: String,
    /// Человекочитаемое имя выхода XRandR (например, `"DP-1"`, `"HDMI-1"`).
    pub name: String,
    /// Смещение по горизонтали относительно root-окна X11.
    pub x: i32,
    /// Смещение по вертикали относительно root-окна X11.
    pub y: i32,
    /// Ширина монитора в пикселях.
    pub width: u32,
    /// Высота монитора в пикселях.
    pub height: u32,
    /// Признак основного монитора (`XRRGetOutputPrimary`).
    pub is_primary: bool,
}

impl X11MonitorGeometry {
    /// Проверяет корректность геометрии монитора.
    pub fn validate(&self) -> Result<(), CaptureError> {
        if self.id.trim().is_empty() {
            return Err(CaptureError::SourceNotFound(self.id.clone()));
        }
        if self.width == 0
            || self.height == 0
            || self.width > MAX_CAPTURE_DIMENSION
            || self.height > MAX_CAPTURE_DIMENSION
        {
            return Err(CaptureError::InvalidDimensions {
                width: self.width,
                height: self.height,
            });
        }
        Ok(())
    }

    /// Преобразует монитор X11 в унифицированный [`DesktopSource`].
    #[must_use]
    pub fn to_desktop_source(&self) -> DesktopSource {
        DesktopSource {
            id: self.id.clone(),
            name: self.name.clone(),
            source_type: DesktopSourceType::Screen,
            is_primary: self.is_primary,
        }
    }
}

/// Метаданные окна верхнего уровня X11 (`_NET_CLIENT_LIST` / `WM_STATE` / `_NET_WM_NAME`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11WindowMetadata {
    /// Числовой идентификатор окна X11 (`Window` XID).
    pub xid: u64,
    /// Заголовок окна (`_NET_WM_NAME` или `WM_NAME`).
    pub title: String,
    /// Класс окна (`WM_CLASS`).
    pub class_name: Option<String>,
    /// Координата X.
    pub x: i32,
    /// Координата Y.
    pub y: i32,
    /// Ширина клиентской области в пикселях.
    pub width: u32,
    /// Высота клиентской области в пикселях.
    pub height: u32,
    /// Отображено ли окно (`XWindowAttributes.map_state == IsViewable`).
    pub is_mapped: bool,
    /// Свёрнуто ли окно (`WM_STATE == IconicState` / `_NET_WM_STATE_HIDDEN`).
    pub is_minimized: bool,
    /// Флаг `override_redirect` (всплывающие меню, тултипы, splash-экраны).
    pub is_override_redirect: bool,
}

impl X11WindowMetadata {
    /// Проверяет, подходит ли окно для захвата согласно `webrtc::WindowCapturerX11::GetWindowList`.
    #[must_use]
    pub fn is_capturable(&self) -> bool {
        self.xid != 0
            && self.is_mapped
            && !self.is_minimized
            && !self.is_override_redirect
            && self.width > 0
            && self.height > 0
            && self.width <= MAX_CAPTURE_DIMENSION
            && self.height <= MAX_CAPTURE_DIMENSION
            && !self.title.trim().is_empty()
    }

    /// Преобразует окно X11 в унифицированный [`DesktopSource`].
    #[must_use]
    pub fn to_desktop_source(&self) -> DesktopSource {
        DesktopSource {
            id: format!("window:x11:{}", self.xid),
            name: self.title.trim().to_string(),
            source_type: DesktopSourceType::Window,
            is_primary: false,
        }
    }
}

/// Фильтрует список окон X11, оставляя только видимые пользовательские окна
/// (исключает свёрнутые, неотображённые, `override_redirect`, нулевые и без заголовка).
#[must_use]
pub fn filter_capturable_x11_windows(windows: &[X11WindowMetadata]) -> Vec<X11WindowMetadata> {
    windows
        .iter()
        .filter(|win| win.is_capturable())
        .cloned()
        .collect()
}

/// Пиксельный буфер X11 MIT-SHM (`webrtc::XServerPixelBuffer`).
///
/// Поддерживает 24-битную глубину (`BGRx` в 32-битном ZPixmap) и 32-битную глубину (`BGRA` с альфа-каналом),
/// выравнивание строк `bytes_per_line >= width * 4`, вырезку под-области монитора из root-окна
/// и контролируемую деградацию `MitShm -> XGetImageFallback -> SyntheticFallback`.
#[derive(Debug, Clone)]
pub struct X11ShmPixelBuffer {
    width: u32,
    height: u32,
    bytes_per_line: usize,
    depth: u8,
    mode: X11CaptureMode,
    fallback_count: u32,
    last_fallback_reason: Option<String>,
}

impl X11ShmPixelBuffer {
    /// Создаёт и валидирует дескриптор пиксельного буфера X11 SHM.
    pub fn new(
        width: u32,
        height: u32,
        bytes_per_line: usize,
        depth: u8,
    ) -> Result<Self, CaptureError> {
        if width == 0
            || height == 0
            || width > MAX_CAPTURE_DIMENSION
            || height > MAX_CAPTURE_DIMENSION
        {
            return Err(CaptureError::InvalidDimensions { width, height });
        }
        if depth != 24 && depth != 32 {
            return Err(CaptureError::ShmError(format!(
                "неподдерживаемая глубина цвета X11 ZPixmap: {depth} (ожидалось 24 или 32)"
            )));
        }
        let min_stride = (width as usize).saturating_mul(4);
        if bytes_per_line < min_stride {
            return Err(CaptureError::InvalidStride {
                stride: bytes_per_line,
                min_stride,
            });
        }

        Ok(Self {
            width,
            height,
            bytes_per_line,
            depth,
            mode: X11CaptureMode::MitShm,
            fallback_count: 0,
            last_fallback_reason: None,
        })
    }

    /// Возвращает ширину буфера в пикселях.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Возвращает высоту буфера в пикселях.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Возвращает шаг строки (`bytes_per_line`) в байтах.
    #[must_use]
    pub const fn bytes_per_line(&self) -> usize {
        self.bytes_per_line
    }

    /// Возвращает глубину цвета (`24` или `32`).
    #[must_use]
    pub const fn depth(&self) -> u8 {
        self.depth
    }

    /// Возвращает текущий режим захвата X11.
    #[must_use]
    pub const fn mode(&self) -> X11CaptureMode {
        self.mode
    }

    /// Возвращает количество срабатываний деградации (fallback).
    #[must_use]
    pub const fn fallback_count(&self) -> u32 {
        self.fallback_count
    }

    /// Возвращает причину последнего перехода в fallback-режим.
    #[must_use]
    pub fn last_fallback_reason(&self) -> Option<&str> {
        self.last_fallback_reason.as_deref()
    }

    /// Фиксирует ошибку `MIT-SHM` (`XShmAttach` / `XShmGetImage`) и переводит захват
    /// по цепочке `MitShm -> XGetImageFallback -> SyntheticFallback`.
    pub fn record_shm_failure(&mut self, reason: impl Into<String>) {
        self.fallback_count = self.fallback_count.saturating_add(1);
        self.last_fallback_reason = Some(reason.into());
        self.mode = match self.mode {
            X11CaptureMode::MitShm => X11CaptureMode::XGetImageFallback,
            X11CaptureMode::XGetImageFallback | X11CaptureMode::SyntheticFallback => {
                X11CaptureMode::SyntheticFallback
            }
        };
    }

    /// Захватывает полный буфер окна X11 из сегмента разделяемой памяти `shm_data` напрямую в [`crate::LatestFrameSlot`].
    pub fn capture_window_into_slot(
        &self,
        shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
        slot: &crate::LatestFrameSlot,
    ) -> Result<u64, CaptureError> {
        slot.store_x11_window(self, shm_data, output_format, timestamp_us)
    }

    /// Захватывает полный буфер окна X11 из сегмента разделяемой памяти `shm_data`.
    pub fn capture_window(
        &self,
        shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
        let pw_format = if self.depth == 32 {
            PipeWirePixelFormat::Bgra8
        } else {
            PipeWirePixelFormat::Bgrx8
        };
        let buf = PipeWireFrameBuffer {
            width: self.width,
            height: self.height,
            stride: self.bytes_per_line,
            format: pw_format,
            data: shm_data,
            crop: None,
        };
        buf.to_video_frame(output_format, timestamp_us)
    }

    /// Вырезает и конвертирует прямоугольник монитора `monitor` из буфера root-окна X11 напрямую в [`crate::LatestFrameSlot`].
    pub fn capture_monitor_rect_into_slot(
        &self,
        monitor: &X11MonitorGeometry,
        root_shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
        slot: &crate::LatestFrameSlot,
    ) -> Result<u64, CaptureError> {
        slot.store_x11_monitor(self, monitor, root_shm_data, output_format, timestamp_us)
    }

    /// Вырезает и конвертирует прямоугольник монитора `monitor` из буфера root-окна X11.
    pub fn capture_monitor_rect(
        &self,
        monitor: &X11MonitorGeometry,
        root_shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
        monitor.validate()?;
        let Ok(crop_x) = u32::try_from(monitor.x) else {
            return Err(CaptureError::CropOutOfBounds {
                crop_x: 0,
                crop_y: 0,
                crop_width: monitor.width,
                crop_height: monitor.height,
                frame_width: self.width,
                frame_height: self.height,
            });
        };
        let Ok(crop_y) = u32::try_from(monitor.y) else {
            return Err(CaptureError::CropOutOfBounds {
                crop_x,
                crop_y: 0,
                crop_width: monitor.width,
                crop_height: monitor.height,
                frame_width: self.width,
                frame_height: self.height,
            });
        };

        let pw_format = if self.depth == 32 {
            PipeWirePixelFormat::Bgra8
        } else {
            PipeWirePixelFormat::Bgrx8
        };
        let buf = PipeWireFrameBuffer {
            width: self.width,
            height: self.height,
            stride: self.bytes_per_line,
            format: pw_format,
            data: root_shm_data,
            crop: Some(CropRect {
                x: crop_x,
                y: crop_y,
                width: monitor.width,
                height: monitor.height,
            }),
        };
        buf.to_video_frame(output_format, timestamp_us)
    }
}
