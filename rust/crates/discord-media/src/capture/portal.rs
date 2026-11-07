//! Протокол `org.freedesktop.portal.ScreenCast` (D-Bus / PipeWire fd) и приём кадров PipeWire.
//!
//! Оракул: `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> `capture_linux`,
//! `webrtc::BaseCapturerPipeWire`, `CcPipewireConfig`) и спецификация
//! `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml` (v6).

use crate::capture::{CaptureError, DesktopSource, DesktopSourceType, MAX_CAPTURE_DIMENSION};
use crate::{PixelLayout, VideoFrame};

/// Имя D-Bus сервиса XDG Desktop Portal.
pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
/// Путь объекта XDG Desktop Portal.
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
/// Интерфейс захвата экрана и окон.
pub const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
/// Интерфейс асинхронного запроса портала.
pub const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
/// Интерфейс сессии портала.
pub const SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";

/// Битовая маска типа источника: монитор (экран целиком).
pub const SOURCE_TYPE_MONITOR: u32 = 1;
/// Битовая маска типа источника: отдельное окно.
pub const SOURCE_TYPE_WINDOW: u32 = 2;
/// Битовая маска типа источника: виртуальный дисплей.
pub const SOURCE_TYPE_VIRTUAL: u32 = 4;
/// Допустимая маска всех типов источников (`MONITOR | WINDOW | VIRTUAL`).
pub const SOURCE_TYPE_ALL_MASK: u32 =
    SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW | SOURCE_TYPE_VIRTUAL;

/// Режим отображения курсора в потоке `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalCursorMode {
    /// Курсор скрыт (`1`).
    Hidden = 1,
    /// Курсор врисован в буфер кадра (`2`).
    Embedded = 2,
    /// Курсор передаётся отдельными метаданными PipeWire (`4`).
    Metadata = 4,
}

impl PortalCursorMode {
    /// Числовое значение битовой маски для D-Bus словаря `SelectSources`.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Режим сохранения токена разрешения (`persist_mode` в `SelectSources`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalPersistMode {
    /// Не сохранять разрешение (`0`).
    DoNotPersist = 0,
    /// Сохранять разрешение пока запущено приложение (`1`).
    ApplicationRunning = 1,
    /// Сохранять разрешение до явного отзыва пользователем (`2`).
    ExplicitlyRevoked = 2,
}

impl PortalPersistMode {
    /// Числовое значение для D-Bus словаря `SelectSources`.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Код ответа сигнала `org.freedesktop.portal.Request::Response(u, a{sv})`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalResponseCode {
    /// Пользователь подтвердил действие (`0`).
    Success = 0,
    /// Пользователь нажал «Отмена» в системном диалоге (`1`).
    CancelledByUser = 1,
    /// Взаимодействие прервано иным образом (`2`).
    EndedOtherwise = 2,
}

impl PortalResponseCode {
    /// Преобразует код `u32` из сигнала `Request.Response` в типизированный статус.
    #[must_use]
    pub const fn from_u32(code: u32) -> Self {
        match code {
            0 => Self::Success,
            1 => Self::CancelledByUser,
            _ => Self::EndedOtherwise,
        }
    }
}

/// Состояние конечного автомата сессии `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalSessionState {
    /// Сессия ещё не создана.
    Unconnected,
    /// Вызов `CreateSession` успешно завершён, получен `session_handle`.
    SessionCreated,
    /// Вызов `SelectSources` успешно завершён.
    SourcesSelected,
    /// Вызовы `Start` и `OpenPipeWireRemote` завершены, получен PipeWire fd и `node_id`.
    Streaming,
    /// Сессия закрыта (штатно или после ошибки/отмены).
    Closed,
}

/// Параметры конфигурации сессии `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalSessionConfig {
    /// Токен запроса (`handle_token`, ASCII `[A-Za-z0-9_]+`).
    pub handle_token: String,
    /// Токен сессии (`session_handle_token`, ASCII `[A-Za-z0-9_]+`).
    pub session_handle_token: String,
    /// Битовая маска типов источников (`SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW | ...`).
    pub source_types: u32,
    /// Разрешить выбор нескольких источников.
    pub multiple: bool,
    /// Режим отображения курсора.
    pub cursor_mode: PortalCursorMode,
    /// Режим сохранения токена `restore_token`.
    pub persist_mode: PortalPersistMode,
    /// Токен восстановления предыдущей сессии (если сохранён).
    pub restore_token: Option<String>,
    /// Идентификатор родительского окна (`""`, `"wayland:..."`, `"x11:0x..."`).
    pub parent_window: String,
}

impl PortalSessionConfig {
    /// Создаёт конфигурацию по умолчанию для заданной битовой маски типов источников.
    #[must_use]
    pub fn default_for_types(source_types: u32) -> Self {
        Self {
            handle_token: "discord_capture_req".to_string(),
            session_handle_token: "discord_capture_sess".to_string(),
            source_types,
            multiple: false,
            cursor_mode: PortalCursorMode::Embedded,
            persist_mode: PortalPersistMode::ApplicationRunning,
            restore_token: None,
            parent_window: String::new(),
        }
    }

    /// Проверяет корректность токенов и битовой маски типов источников.
    pub fn validate(&self) -> Result<(), CaptureError> {
        validate_portal_token(&self.handle_token)?;
        validate_portal_token(&self.session_handle_token)?;
        if self.source_types == 0 || (self.source_types & !SOURCE_TYPE_ALL_MASK) != 0 {
            return Err(CaptureError::InvalidSourceTypes(self.source_types));
        }
        Ok(())
    }
}

/// Дескриптор одного потока PipeWire из ответа `Start` (`streams: a(ua{sv})`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalStream {
    /// Идентификатор узла PipeWire (`node_id`).
    pub node_id: u32,
    /// Необязательный строковый идентификатор потока из свойств портала.
    pub id: Option<String>,
    /// Тип захваченного источника (экран или окно).
    pub source_type: DesktopSourceType,
    /// Позиция источника в виртуальном пространстве `(x, y)`.
    pub position: Option<(i32, i32)>,
    /// Размер источника `(width, height)`.
    pub size: Option<(u32, u32)>,
    /// Идентификатор сопоставления монитора/окна (`mapping_id`).
    pub mapping_id: Option<String>,
}

impl PortalStream {
    /// Преобразует поток портала в унифицированный [`DesktopSource`].
    #[must_use]
    pub fn to_desktop_source(&self, index: usize) -> DesktopSource {
        let prefix = match self.source_type {
            DesktopSourceType::Screen => "screen",
            DesktopSourceType::Window => "window",
        };
        let id = self
            .id
            .clone()
            .unwrap_or_else(|| format!("{prefix}:pw:{}", self.node_id));
        let name = match (&self.mapping_id, self.source_type) {
            (Some(mapping), DesktopSourceType::Screen) if !mapping.trim().is_empty() => {
                format!("Экран {} ({mapping})", index + 1)
            }
            (Some(mapping), DesktopSourceType::Window) if !mapping.trim().is_empty() => {
                mapping.clone()
            }
            (_, DesktopSourceType::Screen) => format!("Экран PipeWire #{}", self.node_id),
            (_, DesktopSourceType::Window) => format!("Окно PipeWire #{}", self.node_id),
        };
        DesktopSource {
            id,
            name,
            source_type: self.source_type,
            is_primary: self.source_type == DesktopSourceType::Screen && index == 0,
        }
    }
}

/// Результат вызова `org.freedesktop.portal.ScreenCast.Start`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalResponse {
    /// Код завершения диалога.
    pub code: PortalResponseCode,
    /// Список согласованных потоков PipeWire.
    pub streams: Vec<PortalStream>,
    /// Новый токен восстановления сессии (если портал его выдал).
    pub restore_token: Option<String>,
}

/// Итоговая конфигурация PipeWire после успешного рукопожатия портала
/// (соответствует `CcPipewireConfig` в `discord_voice.node`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeWireConfig {
    /// Дескриптор сокета PipeWire remote (`fd >= 0`).
    pub fd: i32,
    /// Идентификатор основного узла PipeWire (`node_id > 0`).
    pub node_id: u32,
    /// D-Bus путь активной сессии портала.
    pub session_handle: String,
    /// Токен восстановления для последующих запусков без диалога.
    pub restore_token: Option<String>,
    /// Все потоки, возвращённые порталом.
    pub streams: Vec<PortalStream>,
}

/// Абстракция транспорта для взаимодействия с `org.freedesktop.portal.ScreenCast`.
pub trait PortalTransport {
    /// Вызывает `CreateSession` и ожидает `Request.Response`.
    fn create_session(
        &mut self,
        handle_token: &str,
        session_handle_token: &str,
    ) -> Result<(PortalResponseCode, String), CaptureError>;

    /// Вызывает `SelectSources` для созданной сессии и ожидает `Request.Response`.
    fn select_sources(
        &mut self,
        session_handle: &str,
        config: &PortalSessionConfig,
    ) -> Result<PortalResponseCode, CaptureError>;

    /// Вызывает `Start` и возвращает список потоков PipeWire и `restore_token`.
    fn start(
        &mut self,
        session_handle: &str,
        parent_window: &str,
        handle_token: &str,
    ) -> Result<PortalResponse, CaptureError>;

    /// Вызывает `OpenPipeWireRemote` и возвращает полученный через `SCM_RIGHTS` файловый дескриптор.
    fn open_pipewire_remote(&mut self, session_handle: &str) -> Result<i32, CaptureError>;

    /// Закрывает сессию портала (`org.freedesktop.portal.Session.Close`).
    fn close_session(&mut self, session_handle: &str) -> Result<(), CaptureError>;

    /// Отменяет активный запрос (`org.freedesktop.portal.Request.Close`).
    fn cancel_request(&mut self, request_handle: &str) -> Result<(), CaptureError>;

    /// Закрывает файловый дескриптор PipeWire при завершении сессии.
    fn close_pipewire_fd(&mut self, fd: i32);
}

/// Клиент управления сессией `org.freedesktop.portal.ScreenCast`.
#[derive(Debug)]
pub struct PortalScreenCastClient<T: PortalTransport> {
    transport: T,
    state: PortalSessionState,
    session_handle: Option<String>,
    pipewire_fd: Option<i32>,
}

impl<T: PortalTransport> PortalScreenCastClient<T> {
    /// Создаёт новый клиент поверх заданного транспорта.
    #[must_use]
    pub const fn new(transport: T) -> Self {
        Self {
            transport,
            state: PortalSessionState::Unconnected,
            session_handle: None,
            pipewire_fd: None,
        }
    }

    /// Возвращает текущее состояние сессии.
    #[must_use]
    pub const fn state(&self) -> PortalSessionState {
        self.state
    }

    /// Возвращает путь активной сессии портала (если создана).
    #[must_use]
    pub fn session_handle(&self) -> Option<&str> {
        self.session_handle.as_deref()
    }

    /// Шаг 1: Создание сессии `CreateSession`.
    pub fn create_session(&mut self, config: &PortalSessionConfig) -> Result<String, CaptureError> {
        if self.state != PortalSessionState::Unconnected {
            return Err(CaptureError::InvalidSessionState {
                expected: "Unconnected",
                actual: self.state,
            });
        }
        config.validate()?;

        let (code, session_path) = self
            .transport
            .create_session(&config.handle_token, &config.session_handle_token)?;

        match code {
            PortalResponseCode::Success => {
                validate_dbus_object_path(&session_path)?;
                self.session_handle = Some(session_path.clone());
                self.state = PortalSessionState::SessionCreated;
                Ok(session_path)
            }
            PortalResponseCode::CancelledByUser => {
                self.state = PortalSessionState::Closed;
                Err(CaptureError::ConsentDenied)
            }
            PortalResponseCode::EndedOtherwise => {
                self.state = PortalSessionState::Closed;
                Err(CaptureError::ConsentAborted)
            }
        }
    }

    /// Шаг 2: Выбор источников `SelectSources`.
    pub fn select_sources(&mut self, config: &PortalSessionConfig) -> Result<(), CaptureError> {
        if self.state != PortalSessionState::SessionCreated {
            return Err(CaptureError::InvalidSessionState {
                expected: "SessionCreated",
                actual: self.state,
            });
        }
        config.validate()?;
        let session_handle =
            self.session_handle
                .clone()
                .ok_or(CaptureError::InvalidSessionState {
                    expected: "SessionCreated",
                    actual: self.state,
                })?;

        match self.transport.select_sources(&session_handle, config) {
            Ok(PortalResponseCode::Success) => {
                self.state = PortalSessionState::SourcesSelected;
                Ok(())
            }
            Ok(PortalResponseCode::CancelledByUser) => {
                self.abort_and_close();
                Err(CaptureError::ConsentDenied)
            }
            Ok(PortalResponseCode::EndedOtherwise) => {
                self.abort_and_close();
                Err(CaptureError::ConsentAborted)
            }
            Err(err) => {
                self.abort_and_close();
                Err(err)
            }
        }
    }

    /// Шаг 3 и 4: Запуск потока `Start` и получение файлового дескриптора `OpenPipeWireRemote`.
    pub fn start_and_open_remote(
        &mut self,
        config: &PortalSessionConfig,
    ) -> Result<PipeWireConfig, CaptureError> {
        if self.state != PortalSessionState::SourcesSelected {
            return Err(CaptureError::InvalidSessionState {
                expected: "SourcesSelected",
                actual: self.state,
            });
        }
        config.validate()?;
        let session_handle =
            self.session_handle
                .clone()
                .ok_or(CaptureError::InvalidSessionState {
                    expected: "SourcesSelected",
                    actual: self.state,
                })?;

        let response =
            match self
                .transport
                .start(&session_handle, &config.parent_window, &config.handle_token)
            {
                Ok(resp) => resp,
                Err(err) => {
                    self.abort_and_close();
                    return Err(err);
                }
            };

        match response.code {
            PortalResponseCode::Success => {}
            PortalResponseCode::CancelledByUser => {
                self.abort_and_close();
                return Err(CaptureError::ConsentDenied);
            }
            PortalResponseCode::EndedOtherwise => {
                self.abort_and_close();
                return Err(CaptureError::ConsentAborted);
            }
        }

        let Some(first_stream) = response.streams.first() else {
            self.abort_and_close();
            return Err(CaptureError::NoStreamsReturned);
        };
        if first_stream.node_id == 0 {
            self.abort_and_close();
            return Err(CaptureError::NoStreamsReturned);
        }

        let fd = match self.transport.open_pipewire_remote(&session_handle) {
            Ok(fd) => fd,
            Err(err) => {
                self.abort_and_close();
                return Err(err);
            }
        };
        if fd < 0 {
            self.abort_and_close();
            return Err(CaptureError::InvalidPipeWireFd(fd));
        }

        self.pipewire_fd = Some(fd);
        self.state = PortalSessionState::Streaming;

        Ok(PipeWireConfig {
            fd,
            node_id: first_stream.node_id,
            session_handle,
            restore_token: response.restore_token,
            streams: response.streams,
        })
    }

    /// Штатно закрывает сессию портала и освобождает дескриптор PipeWire.
    pub fn close(&mut self) -> Result<(), CaptureError> {
        if self.state == PortalSessionState::Closed {
            return Ok(());
        }
        if let Some(fd) = self.pipewire_fd.take() {
            self.transport.close_pipewire_fd(fd);
        }
        let result = if let Some(handle) = self.session_handle.take() {
            self.transport.close_session(&handle)
        } else {
            Ok(())
        };
        self.state = PortalSessionState::Closed;
        result
    }

    fn abort_and_close(&mut self) {
        if let Some(fd) = self.pipewire_fd.take() {
            self.transport.close_pipewire_fd(fd);
        }
        if let Some(handle) = self.session_handle.take() {
            // Игнорируем вторичную ошибку при аварийном закрытии сессии.
            let _ = self.transport.close_session(&handle);
        }
        self.state = PortalSessionState::Closed;
    }
}

/// Проверяет, что `handle_token` / `session_handle_token` состоит из непустых ASCII `[A-Za-z0-9_]`.
pub fn validate_portal_token(token: &str) -> Result<(), CaptureError> {
    if token.is_empty()
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(CaptureError::InvalidToken(token.to_string()));
    }
    Ok(())
}

/// Проверяет корректность D-Bus Object Path согласно спецификации D-Bus.
pub fn validate_dbus_object_path(path: &str) -> Result<(), CaptureError> {
    if path == "/" {
        return Ok(());
    }
    if !path.starts_with('/') || path.ends_with('/') {
        return Err(CaptureError::InvalidObjectPath(path.to_string()));
    }
    for segment in path[1..].split('/') {
        if segment.is_empty()
            || !segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(CaptureError::InvalidObjectPath(path.to_string()));
        }
    }
    Ok(())
}

/// Формат пикселей буфера `SpaBuffer` из потока PipeWire (`capture_linux::cpu_processing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWirePixelFormat {
    /// `SPA_VIDEO_FORMAT_BGRA` (32 бит `[B, G, R, A]`).
    Bgra8,
    /// `SPA_VIDEO_FORMAT_BGRx` (32 бит `[B, G, R, X]`, альфа принудительно `255`).
    Bgrx8,
    /// `SPA_VIDEO_FORMAT_RGBA` (32 бит `[R, G, B, A]`).
    Rgba8,
    /// `SPA_VIDEO_FORMAT_RGBx` (32 бит `[R, G, B, X]`, альфа принудительно `255`).
    Rgbx8,
}

/// Прямоугольник обрезки кадра (`SpaMetaVideoCrop`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    /// Смещение по горизонтали.
    pub x: u32,
    /// Смещение по вертикали.
    pub y: u32,
    /// Ширина области обрезки.
    pub width: u32,
    /// Высота области обрезки.
    pub height: u32,
}

/// Буфер кадра PipeWire (MemFd / DMA-BUF mmap) с поддержкой произвольного `stride` и `SpaMetaVideoCrop`.
#[derive(Debug, Clone, Copy)]
pub struct PipeWireFrameBuffer<'a> {
    /// Полная ширина буфера в пикселях.
    pub width: u32,
    /// Полная высота буфера в пикселях.
    pub height: u32,
    /// Шаг строки в байтах (`stride >= width * 4`).
    pub stride: usize,
    /// Формат пикселей PipeWire.
    pub format: PipeWirePixelFormat,
    /// Сырые байты кадра.
    pub data: &'a [u8],
    /// Необязательная область обрезки `SpaMetaVideoCrop`.
    pub crop: Option<CropRect>,
}

impl PipeWireFrameBuffer<'_> {
    /// Валидирует геометрию буфера и конвертирует его в [`VideoFrame`] (`Rgba`, `I420` или `Nv12`).
    pub fn to_video_frame(
        &self,
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
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

        let min_stride = (self.width as usize).saturating_mul(4);
        if self.stride < min_stride {
            return Err(CaptureError::InvalidStride {
                stride: self.stride,
                min_stride,
            });
        }

        let required_bytes = (self.height as usize - 1)
            .saturating_mul(self.stride)
            .saturating_add(min_stride);
        if self.data.len() < required_bytes {
            return Err(CaptureError::BufferTooSmall {
                actual: self.data.len(),
                required: required_bytes,
            });
        }

        let (start_x, start_y, out_w, out_h) = match self.crop {
            Some(crop) => {
                if crop.width == 0
                    || crop.height == 0
                    || crop.x.saturating_add(crop.width) > self.width
                    || crop.y.saturating_add(crop.height) > self.height
                {
                    return Err(CaptureError::CropOutOfBounds {
                        crop_x: crop.x,
                        crop_y: crop.y,
                        crop_width: crop.width,
                        crop_height: crop.height,
                        frame_width: self.width,
                        frame_height: self.height,
                    });
                }
                (crop.x, crop.y, crop.width, crop.height)
            }
            None => (0, 0, self.width, self.height),
        };

        match output_format {
            PixelLayout::Rgba => {
                let mut rgba = Vec::with_capacity((out_w as usize) * (out_h as usize) * 4);
                for row in start_y..(start_y + out_h) {
                    let row_offset = (row as usize) * self.stride;
                    for col in start_x..(start_x + out_w) {
                        let px_offset = row_offset + (col as usize) * 4;
                        let b0 = self.data[px_offset];
                        let b1 = self.data[px_offset + 1];
                        let b2 = self.data[px_offset + 2];
                        let b3 = self.data[px_offset + 3];
                        let (r, g, b, a) = match self.format {
                            PipeWirePixelFormat::Bgra8 => (b2, b1, b0, b3),
                            PipeWirePixelFormat::Bgrx8 => (b2, b1, b0, 255),
                            PipeWirePixelFormat::Rgba8 => (b0, b1, b2, b3),
                            PipeWirePixelFormat::Rgbx8 => (b0, b1, b2, 255),
                        };
                        rgba.extend_from_slice(&[r, g, b, a]);
                    }
                }
                Ok(VideoFrame::new(out_w, out_h, PixelLayout::Rgba, rgba)
                    .with_timestamp_us(timestamp_us))
            }
            PixelLayout::I420 | PixelLayout::Nv12 => {
                let frame = convert_strided_to_i420(
                    self.data,
                    self.stride,
                    self.format,
                    start_x,
                    start_y,
                    out_w,
                    out_h,
                    timestamp_us,
                );
                if output_format == PixelLayout::I420 {
                    Ok(frame)
                } else {
                    // Преобразуем плоский U/V из I420 в интерливированный UV для NV12
                    let y_len = (out_w as usize) * (out_h as usize);
                    let uv_w = (out_w as usize).div_ceil(2);
                    let uv_h = (out_h as usize).div_ceil(2);
                    let plane_len = uv_w * uv_h;
                    let mut nv12 = Vec::with_capacity(y_len + plane_len * 2);
                    nv12.extend_from_slice(&frame.data[..y_len]);
                    let u_plane = &frame.data[y_len..y_len + plane_len];
                    let v_plane = &frame.data[y_len + plane_len..y_len + plane_len * 2];
                    for idx in 0..plane_len {
                        nv12.push(u_plane[idx]);
                        nv12.push(v_plane[idx]);
                    }
                    Ok(VideoFrame::new(out_w, out_h, PixelLayout::Nv12, nv12)
                        .with_timestamp_us(timestamp_us))
                }
            }
        }
    }
}

/// Конвертирует RGB8 в BT.601 limited range `(Y, U, V)`.
#[must_use]
pub fn rgb_to_yuv_bt601(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r_i = r as i32;
    let g_i = g as i32;
    let b_i = b as i32;
    let y = ((66 * r_i + 129 * g_i + 25 * b_i + 128) >> 8) + 16;
    let u = ((-38 * r_i - 74 * g_i + 112 * b_i + 128) >> 8) + 128;
    let v = ((112 * r_i - 94 * g_i - 18 * b_i + 128) >> 8) + 128;
    (
        y.clamp(16, 235) as u8,
        u.clamp(16, 240) as u8,
        v.clamp(16, 240) as u8,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn convert_strided_to_i420(
    data: &[u8],
    stride: usize,
    format: PipeWirePixelFormat,
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
    timestamp_us: u64,
) -> VideoFrame {
    let w = width as usize;
    let h = height as usize;
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    let y_size = w * h;
    let uv_size = uv_w * uv_h;

    let mut out = vec![0u8; y_size + uv_size * 2];

    for row in 0..h {
        let src_row = (start_y as usize + row) * stride;
        for col in 0..w {
            let px = src_row + (start_x as usize + col) * 4;
            let (r, g, b) = match format {
                PipeWirePixelFormat::Bgra8 | PipeWirePixelFormat::Bgrx8 => {
                    (data[px + 2], data[px + 1], data[px])
                }
                PipeWirePixelFormat::Rgba8 | PipeWirePixelFormat::Rgbx8 => {
                    (data[px], data[px + 1], data[px + 2])
                }
            };
            let (y_val, u_val, v_val) = rgb_to_yuv_bt601(r, g, b);
            out[row * w + col] = y_val;

            if row % 2 == 0 && col % 2 == 0 {
                let uv_idx = (row / 2) * uv_w + (col / 2);
                out[y_size + uv_idx] = u_val;
                out[y_size + uv_size + uv_idx] = v_val;
            }
        }
    }

    VideoFrame::new(width, height, PixelLayout::I420, out).with_timestamp_us(timestamp_us)
}
