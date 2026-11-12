//! Модуль захвата экрана и окон (`org.freedesktop.portal.ScreenCast` / PipeWire и X11 SHM).
//!
//! Оракул: `/home/sanya/re-deep/native-re.md` (`chord_voice.node` -> `capture_linux`,
//! `webrtc::BaseCapturerPipeWire`, `webrtc::ScreenCapturerX11`, `webrtc::WindowCapturerX11`,
//! `webrtc::XServerPixelBuffer`) и `/home/sanya/re-deep/backend-ipc.md`
//! (`DISCORD_DESKTOP_CAPTURER_GET_SOURCES`).

use std::fmt;

use crate::{PixelLayout, VideoFrame};

pub mod portal;
pub mod sources;
pub mod x11;

use self::portal::{PipeWireFrameBuffer, PortalSessionState, PortalStream};
use self::sources::{SourceFilter, default_sources_for_backend, detect_backend_live};
use self::x11::{
    X11MonitorGeometry, X11ShmPixelBuffer, X11WindowMetadata, filter_capturable_x11_windows,
};

/// Псевдоним формата пикселей видеокадра для совместимости контрактов захвата.
pub type PixelFormat = PixelLayout;

/// Максимально допустимый линейный размер захватываемого кадра (8K UHD = 8192 пикселя).
pub const MAX_CAPTURE_DIMENSION: u32 = 8192;

/// Ошибки подсистемы захвата экрана и окон (PipeWire Portal и X11 SHM).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// Пользователь отклонил запрос на захват экрана в диалоге портала (`Response == 1`).
    ConsentDenied,
    /// Диалог выбора источника портала был прерван (`Response == 2`).
    ConsentAborted,
    /// Истекло время ожидания ответа от `org.freedesktop.portal.Request`.
    ConsentTimeout,
    /// Захватываемое окно было уничтожено (`org.freedesktop.portal.Error.WindowDestroyed`).
    WindowDestroyed,
    /// Некорректный токен запроса или сессии портала (`handle_token` / `session_handle_token`).
    InvalidToken(String),
    /// Некорректный путь объекта D-Bus (`ObjectPath`).
    InvalidObjectPath(String),
    /// Некорректная битовая маска типов источников захвата.
    InvalidSourceTypes(u32),
    /// Портал вернул невалидный файловый дескриптор PipeWire (`fd < 0`).
    InvalidPipeWireFd(i32),
    /// Портал завершил `Start` со статусом успеха, но не вернул ни одного потока `streams`.
    NoStreamsReturned,
    /// Нарушен порядок вызовов жизненного цикла сессии портала.
    InvalidSessionState {
        /// Ожидаемое состояние.
        expected: &'static str,
        /// Фактическое состояние.
        actual: PortalSessionState,
    },
    /// Недопустимые размеры кадра или окна (`0` или `> MAX_CAPTURE_DIMENSION`).
    InvalidDimensions {
        /// Ширина.
        width: u32,
        /// Высота.
        height: u32,
    },
    /// Шаг строки (`stride` / `bytes_per_line`) меньше минимально необходимого `width * 4`.
    InvalidStride {
        /// Переданный шаг строки.
        stride: usize,
        /// Минимально допустимый шаг строки.
        min_stride: usize,
    },
    /// Размер буфера пикселей меньше требуемого для заданных `height` и `stride`.
    BufferTooSmall {
        /// Фактический размер буфера в байтах.
        actual: usize,
        /// Требуемый минимальный размер в байтах.
        required: usize,
    },
    /// Область обрезки (`CropRect` или геометрия монитора) выходит за пределы буфера.
    CropOutOfBounds {
        /// Смещение X области.
        crop_x: u32,
        /// Смещение Y области.
        crop_y: u32,
        /// Ширина области.
        crop_width: u32,
        /// Высота области.
        crop_height: u32,
        /// Ширина исходного буфера.
        frame_width: u32,
        /// Высота исходного буфера.
        frame_height: u32,
    },
    /// Запрошенный источник захвата не найден.
    SourceNotFound(String),
    /// Дисплей X11 недоступен.
    X11Unavailable(String),
    /// Ошибка сегмента разделяемой памяти X11 MIT-SHM.
    ShmError(String),
    /// Ошибка транспорта D-Bus при обращении к порталу.
    DbusError(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsentDenied => {
                write!(f, "пользователь отклонил доступ к захвату экрана в портале")
            }
            Self::ConsentAborted => write!(f, "сессия выбора источника портала была прервана"),
            Self::ConsentTimeout => {
                write!(f, "истекло время ожидания ответа от портала ScreenCast")
            }
            Self::WindowDestroyed => write!(f, "захватываемое окно было закрыто или уничтожено"),
            Self::InvalidToken(tok) => write!(f, "недопустимый токен портала: {tok:?}"),
            Self::InvalidObjectPath(path) => write!(f, "недопустимый путь объекта D-Bus: {path:?}"),
            Self::InvalidSourceTypes(mask) => {
                write!(f, "недопустимая битовая маска типов источников: 0x{mask:x}")
            }
            Self::InvalidPipeWireFd(fd) => {
                write!(f, "портал вернул невалидный дескриптор PipeWire: {fd}")
            }
            Self::NoStreamsReturned => {
                write!(f, "портал ScreenCast не вернул ни одного потока PipeWire")
            }
            Self::InvalidSessionState { expected, actual } => write!(
                f,
                "недопустимое состояние сессии портала: ожидалось {expected}, фактически {actual:?}"
            ),
            Self::InvalidDimensions { width, height } => {
                write!(f, "недопустимый размер кадра захвата: {width}x{height}")
            }
            Self::InvalidStride { stride, min_stride } => write!(
                f,
                "шаг строки буфера ({stride} Б) меньше минимального ({min_stride} Б)"
            ),
            Self::BufferTooSmall { actual, required } => write!(
                f,
                "буфер кадра слишком мал: получено {actual} Б, требуется минимум {required} Б"
            ),
            Self::CropOutOfBounds {
                crop_x,
                crop_y,
                crop_width,
                crop_height,
                frame_width,
                frame_height,
            } => write!(
                f,
                "область ({crop_x},{crop_y} {crop_width}x{crop_height}) выходит за границы кадра {frame_width}x{frame_height}"
            ),
            Self::SourceNotFound(id) => write!(f, "источник захвата {id:?} не найден"),
            Self::X11Unavailable(msg) => write!(f, "дисплей X11 недоступен: {msg}"),
            Self::ShmError(msg) => write!(f, "ошибка X11 MIT-SHM: {msg}"),
            Self::DbusError(msg) => write!(f, "ошибка D-Bus портала ScreenCast: {msg}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// Тип захватываемого источника рабочего стола (`DISCORD_DESKTOP_CAPTURER_GET_SOURCES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopSourceType {
    /// Весь экран (монитор).
    Screen,
    /// Отдельное окно приложения.
    Window,
}

impl DesktopSourceType {
    /// Каноническое строковое имя типа источника (`"screen"` или `"window"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Window => "window",
        }
    }
}

/// Описание доступного источника для демонстрации экрана (`ScreenCapturer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSource {
    /// Уникальный идентификатор источника (например, `"screen:0"`, `"window:x11:101"`).
    pub id: String,
    /// Человекочитаемое название экрана или заголовок окна.
    pub name: String,
    /// Тип источника (экран или окно).
    pub source_type: DesktopSourceType,
    /// Признак основного монитора.
    pub is_primary: bool,
}

/// Бэкенд захвата экрана в текущем окружении.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopBackend {
    /// Захват через `org.freedesktop.portal.ScreenCast` + PipeWire (Wayland / современная сессия Linux).
    PipeWirePortal,
    /// Захват через X11 MIT-SHM (Xorg / XWayland fallback).
    X11Shm,
    /// Синтетический генератор тестовой таблицы (fallback в headless-окружении).
    SyntheticFallback,
}

impl DesktopBackend {
    /// Строковое имя бэкенда для телеметрии и логов.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PipeWirePortal => "pipewire_portal",
            Self::X11Shm => "x11_shm",
            Self::SyntheticFallback => "synthetic_fallback",
        }
    }
}

/// Менеджер захвата экрана и окон (`ScreenCapturer`).
///
/// Объединяет:
/// - перечисление доступных источников захвата (мониторы, окна) с фильтрацией по типам;
/// - приём кадров из потока `org.freedesktop.portal.ScreenCast` / PipeWire;
/// - захват окон и мониторов через X11 MIT-SHM;
/// - генерацию синтетического кадра в headless-режиме.
#[derive(Debug, Clone)]
pub struct ScreenCapturer {
    backend: DesktopBackend,
    custom_sources: Option<Vec<DesktopSource>>,
}

impl Default for ScreenCapturer {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenCapturer {
    /// Создаёт захватчик экрана с автоматическим определением бэкенда окружения.
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend: Self::detect_backend(),
            custom_sources: None,
        }
    }

    /// Создаёт захватчик с явно заданным бэкендом.
    #[must_use]
    pub const fn with_backend(backend: DesktopBackend) -> Self {
        Self {
            backend,
            custom_sources: None,
        }
    }

    /// Создаёт захватчик с явно обнаруженными мониторами и окнами X11 (отфильтровывая невидимые окна).
    #[must_use]
    pub fn with_sources(
        backend: DesktopBackend,
        monitors: Vec<X11MonitorGeometry>,
        windows: Vec<X11WindowMetadata>,
    ) -> Self {
        let mut sources = Vec::with_capacity(monitors.len() + windows.len());
        for mon in &monitors {
            if mon.validate().is_ok() {
                sources.push(mon.to_desktop_source());
            }
        }
        for win in filter_capturable_x11_windows(&windows) {
            sources.push(win.to_desktop_source());
        }
        Self {
            backend,
            custom_sources: Some(sources),
        }
    }

    /// Создаёт захватчик на базе согласованных потоков `org.freedesktop.portal.ScreenCast`.
    #[must_use]
    pub fn from_portal_streams(streams: &[PortalStream]) -> Self {
        let sources = streams
            .iter()
            .enumerate()
            .map(|(idx, stream)| stream.to_desktop_source(idx))
            .collect();
        Self {
            backend: DesktopBackend::PipeWirePortal,
            custom_sources: Some(sources),
        }
    }

    /// Определяет доступный бэкенд захвата экрана по переменным окружения (`WAYLAND_DISPLAY`, `DISPLAY`, `XDG_SESSION_TYPE`).
    #[must_use]
    pub fn detect_backend() -> DesktopBackend {
        detect_backend_live()
    }

    /// Возвращает активный бэкенд захвата.
    #[must_use]
    pub const fn backend(&self) -> DesktopBackend {
        self.backend
    }

    /// Перечисляет все доступные источники захвата (экраны и окна).
    #[must_use]
    pub fn enumerate_sources(&self) -> Vec<DesktopSource> {
        self.custom_sources
            .clone()
            .unwrap_or_else(|| default_sources_for_backend(self.backend))
    }

    /// Перечисляет доступные источники захвата с применением фильтра [`SourceFilter`].
    #[must_use]
    pub fn enumerate_sources_filtered(&self, filter: SourceFilter) -> Vec<DesktopSource> {
        self.enumerate_sources()
            .into_iter()
            .filter(|src| filter.matches(src))
            .collect()
    }

    /// Перечисляет только доступные мониторы (`DesktopSourceType::Screen`).
    #[must_use]
    pub fn enumerate_screens(&self) -> Vec<DesktopSource> {
        self.enumerate_sources_filtered(SourceFilter::screens_only())
    }

    /// Перечисляет только доступные окна (`DesktopSourceType::Window`).
    #[must_use]
    pub fn enumerate_windows(&self) -> Vec<DesktopSource> {
        self.enumerate_sources_filtered(SourceFilter::windows_only())
    }

    /// Ищет источник захвата по его идентификатору `source_id`.
    #[must_use]
    pub fn find_source(&self, source_id: &str) -> Option<DesktopSource> {
        self.enumerate_sources()
            .into_iter()
            .find(|src| src.id == source_id)
    }

    /// Конвертирует буфер кадра PipeWire (`SpaBuffer`) в [`VideoFrame`].
    pub fn capture_portal_frame(
        &self,
        frame: &PipeWireFrameBuffer<'_>,
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
        frame.to_video_frame(output_format, timestamp_us)
    }

    /// Конвертирует буфер кадра PipeWire (`SpaBuffer`) и записывает напрямую в [`crate::LatestFrameSlot`].
    pub fn capture_portal_frame_into_slot(
        &self,
        frame: &PipeWireFrameBuffer<'_>,
        output_format: PixelLayout,
        timestamp_us: u64,
        slot: &crate::LatestFrameSlot,
    ) -> Result<u64, CaptureError> {
        frame.capture_into_slot(slot, output_format, timestamp_us)
    }

    /// Захватывает окно X11 из сегмента разделяемой памяти MIT-SHM.
    pub fn capture_x11_shm_window(
        &self,
        window: &X11WindowMetadata,
        shm_data: &[u8],
        bytes_per_line: usize,
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
        if !window.is_capturable() {
            return Err(CaptureError::InvalidDimensions {
                width: window.width,
                height: window.height,
            });
        }
        let shm_buf = X11ShmPixelBuffer::new(window.width, window.height, bytes_per_line, 24)?;
        shm_buf.capture_window(shm_data, output_format, timestamp_us)
    }

    /// Захватывает окно X11 из сегмента разделяемой памяти MIT-SHM напрямую в [`crate::LatestFrameSlot`].
    pub fn capture_x11_shm_window_into_slot(
        &self,
        window: &X11WindowMetadata,
        shm_data: &[u8],
        bytes_per_line: usize,
        output_format: PixelLayout,
        timestamp_us: u64,
        slot: &crate::LatestFrameSlot,
    ) -> Result<u64, CaptureError> {
        if !window.is_capturable() {
            slot.record_rejected_frame();
            return Err(CaptureError::InvalidDimensions {
                width: window.width,
                height: window.height,
            });
        }
        let shm_buf = match X11ShmPixelBuffer::new(window.width, window.height, bytes_per_line, 24)
        {
            Ok(buf) => buf,
            Err(err) => {
                slot.record_rejected_frame();
                return Err(err);
            }
        };
        shm_buf.capture_window_into_slot(shm_data, output_format, timestamp_us, slot)
    }

    /// Захватывает прямоугольник монитора X11 из сегмента разделяемой памяти root-окна.
    #[allow(clippy::too_many_arguments)]
    pub fn capture_x11_shm_monitor(
        &self,
        monitor: &X11MonitorGeometry,
        root_width: u32,
        root_height: u32,
        root_shm_data: &[u8],
        bytes_per_line: usize,
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
        let shm_buf = X11ShmPixelBuffer::new(root_width, root_height, bytes_per_line, 24)?;
        shm_buf.capture_monitor_rect(monitor, root_shm_data, output_format, timestamp_us)
    }

    /// Захватывает прямоугольник монитора X11 из сегмента разделяемой памяти root-окна напрямую в [`crate::LatestFrameSlot`].
    #[allow(clippy::too_many_arguments)]
    pub fn capture_x11_shm_monitor_into_slot(
        &self,
        monitor: &X11MonitorGeometry,
        root_width: u32,
        root_height: u32,
        root_shm_data: &[u8],
        bytes_per_line: usize,
        output_format: PixelLayout,
        timestamp_us: u64,
        slot: &crate::LatestFrameSlot,
    ) -> Result<u64, CaptureError> {
        let shm_buf = match X11ShmPixelBuffer::new(root_width, root_height, bytes_per_line, 24) {
            Ok(buf) => buf,
            Err(err) => {
                slot.record_rejected_frame();
                return Err(err);
            }
        };
        shm_buf.capture_monitor_rect_into_slot(
            monitor,
            root_shm_data,
            output_format,
            timestamp_us,
            slot,
        )
    }

    /// Захватывает одиночный видеокадр для заданного источника и разрешения.
    /// Формирует валидный буфер `VideoFrame` (`I420` планарный) с таймстемпом.
    pub fn capture_frame(
        &self,
        source_id: &str,
        width: u32,
        height: u32,
        timestamp_us: u64,
    ) -> Result<VideoFrame, &'static str> {
        if width == 0
            || height == 0
            || width > MAX_CAPTURE_DIMENSION
            || height > MAX_CAPTURE_DIMENSION
        {
            return Err("Разрешение кадра должно быть в пределах 1..=8192");
        }
        if source_id.trim().is_empty() {
            return Err("source_id не может быть пустым");
        }
        let w = width as usize;
        let h = height as usize;
        let luma_len = w.checked_mul(h).ok_or("Переполнение размера luma")?;
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);
        let chroma_len = cw.checked_mul(ch).ok_or("Переполнение размера chroma")?;
        let total_len = luma_len
            .checked_add(chroma_len.checked_mul(2).ok_or("Переполнение chroma")?)
            .ok_or("Переполнение total_len")?;

        let mut data = vec![128u8; total_len];
        let shift = ((timestamp_us / 16_666) % 256) as u8;
        for (i, byte) in data[..luma_len].iter_mut().enumerate() {
            *byte = ((i % 256) as u8).wrapping_add(shift);
        }

        Ok(VideoFrame::new(width, height, PixelLayout::I420, data).with_timestamp_us(timestamp_us))
    }

    /// Захватывает одиночный видеокадр (`I420`) напрямую в [`crate::LatestFrameSlot`] с переиспользованием буфера.
    pub fn capture_frame_into_slot(
        &self,
        source_id: &str,
        width: u32,
        height: u32,
        timestamp_us: u64,
        slot: &crate::LatestFrameSlot,
    ) -> Result<u64, &'static str> {
        if width == 0
            || height == 0
            || width > MAX_CAPTURE_DIMENSION
            || height > MAX_CAPTURE_DIMENSION
        {
            slot.record_rejected_frame();
            return Err("Разрешение кадра должно быть в пределах 1..=8192");
        }
        if source_id.trim().is_empty() {
            slot.record_rejected_frame();
            return Err("source_id не может быть пустым");
        }
        let luma_len = (width as usize)
            .checked_mul(height as usize)
            .ok_or("Переполнение размера luma")?;
        let shift = ((timestamp_us / 16_666) % 256) as u8;
        let color = crate::ColorSpace::untagged(height, false);
        slot.store_with_reused_buffer(
            width,
            height,
            PixelLayout::I420,
            color,
            0,
            timestamp_us,
            |buf| {
                for (i, byte) in buf[..luma_len].iter_mut().enumerate() {
                    *byte = ((i % 256) as u8).wrapping_add(shift);
                }
                buf[luma_len..].fill(128u8);
                Ok(())
            },
        )
    }
}
