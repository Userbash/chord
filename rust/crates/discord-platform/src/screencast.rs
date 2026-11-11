//! Платформенный D-Bus клиент `org.freedesktop.portal.ScreenCast` (PipeWire fd) и зонд X11 Display/SHM.
//!
//! Оракул: `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml` (v6),
//! `org.freedesktop.portal.Request.xml`, `org.freedesktop.portal.Session.xml` и
//! `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> `capture_linux`).

#![allow(unsafe_code)]

use std::fmt;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::dbus::{BusAddress, Connection, DbusError, Value, fd};

/// Имя D-Bus сервиса XDG Desktop Portal.
pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
/// Путь корневого объекта XDG Desktop Portal.
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
/// Интерфейс захвата экрана и окон.
pub const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
/// Интерфейс асинхронного запроса портала.
pub const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
/// Интерфейс сессии портала.
pub const SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";

/// Таймаут ожидания ответа пользователя в диалоге портала по умолчанию.
pub const DEFAULT_PORTAL_DIALOG_TIMEOUT: Duration = Duration::from_secs(30);

/// Битовая маска типов источников `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalSourceTypes(u32);

impl PortalSourceTypes {
    /// Захват монитора (`1`).
    pub const MONITOR: Self = Self(1);
    /// Захват окна (`2`).
    pub const WINDOW: Self = Self(2);
    /// Виртуальный дисплей (`4`).
    pub const VIRTUAL: Self = Self(4);
    /// Захват мониторов и окон (`1 | 2`).
    pub const MONITOR_AND_WINDOW: Self = Self(1 | 2);

    /// Создаёт маску с усечением неизвестных битов.
    #[must_use]
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & 0b111)
    }

    /// Возвращает числовое представление битовой маски.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Проверяет, что маска пуста.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Режим курсора `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PortalCursorMode {
    /// Курсор скрыт (`1`).
    Hidden = 1,
    /// Курсор врисован в кадр (`2`).
    Embedded = 2,
    /// Курсор передаётся в метаданных PipeWire (`4`).
    Metadata = 4,
}

/// Режим сохранения разрешения `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PortalPersistMode {
    /// Не сохранять (`0`).
    DoNotPersist = 0,
    /// Сохранять на время работы приложения (`1`).
    ApplicationRunning = 1,
    /// Сохранять до явного отзыва (`2`).
    ExplicitlyRevoked = 2,
}

/// Ошибки платформенного клиента `ScreenCastPortal` и зонда X11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalRequestError {
    /// Пользователь отклонил запрос в диалоге (`Response == 1`).
    ConsentDenied,
    /// Диалог портала был прерван (`Response == 2`).
    ConsentAborted,
    /// Истекло время ожидания сигнала `Request.Response`.
    Timeout,
    /// Недопустимый токен запроса/сессии (`handle_token` / `session_handle_token`).
    InvalidToken(String),
    /// Недопустимый путь объекта D-Bus.
    InvalidObjectPath(String),
    /// Недопустимая маска типов источников.
    InvalidSourceTypes(u32),
    /// Невалидный файловый дескриптор PipeWire (`fd < 0`).
    InvalidFd(RawFd),
    /// Портал не вернул потоков PipeWire в `Start`.
    EmptyStreams,
    /// Некорректная строка `DISPLAY` X11.
    InvalidX11Display(String),
    /// Ошибка протокола или транспорта D-Bus.
    Dbus(String),
}

impl fmt::Display for PortalRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsentDenied => write!(f, "пользователь отклонил захват экрана в портале"),
            Self::ConsentAborted => write!(f, "диалог портала захвата экрана был прерван"),
            Self::Timeout => write!(f, "истекло время ожидания ответа от портала ScreenCast"),
            Self::InvalidToken(t) => write!(f, "недопустимый токен портала: {t:?}"),
            Self::InvalidObjectPath(p) => write!(f, "недопустимый D-Bus путь: {p:?}"),
            Self::InvalidSourceTypes(m) => write!(f, "недопустимая маска типов источников: {m}"),
            Self::InvalidFd(raw) => write!(f, "недопустимый файловый дескриптор PipeWire: {raw}"),
            Self::EmptyStreams => write!(f, "портал не вернул ни одного потока PipeWire"),
            Self::InvalidX11Display(d) => write!(f, "некорректная спецификация X11 DISPLAY: {d:?}"),
            Self::Dbus(msg) => write!(f, "ошибка D-Bus: {msg}"),
        }
    }
}

impl std::error::Error for PortalRequestError {}

impl From<DbusError> for PortalRequestError {
    fn from(err: DbusError) -> Self {
        match err {
            DbusError::Timeout => Self::Timeout,
            DbusError::Remote { ref name, .. }
                if name == "org.freedesktop.portal.Error.Cancelled"
                    || name == "org.freedesktop.portal.Error.NotAllowed" =>
            {
                Self::ConsentDenied
            }
            other => Self::Dbus(format!("{other:?}")),
        }
    }
}

/// Опции запуска сессии `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCastOptions {
    /// Токен запроса (`handle_token`).
    pub handle_token: String,
    /// Токен сессии (`session_handle_token`).
    pub session_handle_token: String,
    /// Типы запрашиваемых источников.
    pub source_types: PortalSourceTypes,
    /// Разрешить множественный выбор.
    pub multiple: bool,
    /// Режим курсора.
    pub cursor_mode: PortalCursorMode,
    /// Режим сохранения токена.
    pub persist_mode: PortalPersistMode,
    /// Предыдущий токен восстановления.
    pub restore_token: Option<String>,
    /// Идентификатор родительского окна.
    pub parent_window: String,
}

impl ScreenCastOptions {
    /// Проверяет корректность токенов и маски типов источников.
    pub fn validate(&self) -> Result<(), PortalRequestError> {
        validate_token(&self.handle_token)?;
        validate_token(&self.session_handle_token)?;
        if self.source_types.is_empty() {
            return Err(PortalRequestError::InvalidSourceTypes(
                self.source_types.bits(),
            ));
        }
        Ok(())
    }
}

/// Описание потока из ответа `Start` (`streams: a(ua{sv})`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalStreamInfo {
    /// Идентификатор узла PipeWire (`node_id`).
    pub node_id: u32,
    /// Строковый идентификатор источника (`id`).
    pub id: Option<String>,
    /// Код типа источника (`1` = монитор, `2` = окно, `4` = виртуальный).
    pub source_type: u32,
    /// Координаты `(x, y)`.
    pub position: Option<(i32, i32)>,
    /// Размеры `(width, height)`.
    pub size: Option<(u32, u32)>,
    /// Идентификатор сопоставления (`mapping_id`).
    pub mapping_id: Option<String>,
}

/// Создаёт дубликат файлового дескриптора с атомарным флагом `FD_CLOEXEC` (`fcntl(F_DUPFD_CLOEXEC)`).
fn dup_raw_fd(raw: RawFd) -> Result<RawFd, PortalRequestError> {
    if raw < 0 {
        return Err(PortalRequestError::InvalidFd(raw));
    }
    // SAFETY: F_DUPFD_CLOEXEC атомарно выделяет новый дескриптор с O_CLOEXEC, не трогая исходный.
    let new_fd = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 0) };
    if new_fd < 0 {
        let err = std::io::Error::last_os_error();
        Err(PortalRequestError::Dbus(format!(
            "не удалось продублировать дескриптор {raw}: {err}"
        )))
    } else {
        Ok(new_fd)
    }
}

/// RAII-обёртка над файловым дескриптором сокета PipeWire remote (`OpenPipeWireRemote`).
///
/// Гарантированно закрывает дескриптор через [`fd::close_fd`] при выходе из области видимости.
#[derive(Debug)]
pub struct OwnedPipeWireFd {
    raw: RawFd,
}

impl OwnedPipeWireFd {
    /// Проверяет, что `raw >= 0`, и оборачивает дескриптор в RAII-структуру.
    pub fn from_raw_fd_checked(raw: RawFd) -> Result<Self, PortalRequestError> {
        if raw < 0 {
            return Err(PortalRequestError::InvalidFd(raw));
        }
        Ok(Self { raw })
    }

    /// Возвращает числовое значение файлового дескриптора.
    #[must_use]
    pub const fn as_raw_fd(&self) -> RawFd {
        self.raw
    }

    /// Создаёт независимый дубликат дескриптора с флагом `O_CLOEXEC` без повторного запроса к порталу.
    pub fn try_clone(&self) -> Result<Self, PortalRequestError> {
        let cloned = dup_raw_fd(self.raw)?;
        Ok(Self { raw: cloned })
    }
}

impl Drop for OwnedPipeWireFd {
    fn drop(&mut self) {
        if self.raw >= 0 {
            fd::close_fd(self.raw);
        }
    }
}

/// RAII-обёртка над дескриптором аппаратного буфера DMA-BUF из потока PipeWire.
#[derive(Debug)]
pub struct OwnedDmaBuf {
    raw: RawFd,
}

impl OwnedDmaBuf {
    /// Оборачивает проверенный дескриптор DMA-BUF (`raw >= 0`).
    pub fn from_raw_fd_checked(raw: RawFd) -> Result<Self, PortalRequestError> {
        if raw < 0 {
            return Err(PortalRequestError::InvalidFd(raw));
        }
        Ok(Self { raw })
    }

    /// Возвращает числовое значение дескриптора.
    #[must_use]
    pub const fn as_raw_fd(&self) -> RawFd {
        self.raw
    }

    /// Создаёт дубликат дескриптора DMA-BUF.
    pub fn try_clone(&self) -> Result<Self, PortalRequestError> {
        let cloned = dup_raw_fd(self.raw)?;
        Ok(Self { raw: cloned })
    }
}

impl Drop for OwnedDmaBuf {
    fn drop(&mut self) {
        if self.raw >= 0 {
            fd::close_fd(self.raw);
        }
    }
}

impl PartialEq for OwnedDmaBuf {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for OwnedDmaBuf {}

/// Метаданные и буфер захваченного кадра (PipeWire DMA-BUF / MemFd или X11 SHM).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedFrame {
    /// Ширина кадра в пикселях.
    pub width: u32,
    /// Высота кадра в пикселях.
    pub height: u32,
    /// Шаг строки в байтах (`stride`).
    pub stride: u32,
    /// Код формата пикселей (FourCC или PipeWire SPA).
    pub format: u32,
    /// Разделяемый RAII-дескриптор DMA-BUF для аппаратного zero-copy импорта (EGL/VA-API).
    pub dma_buf: Option<Arc<OwnedDmaBuf>>,
    /// DRM формат-модификатор (DRM modifier).
    pub drm_modifier: u64,
    /// Временная метка захвата в микросекундах.
    pub timestamp_us: u64,
    /// Порядковый номер кадра в потоке.
    pub sequence: u64,
    /// Пиксельные данные для CPU/MemFd буферов.
    pub data: Vec<u8>,
}

impl CapturedFrame {
    /// Создаёт новый пустой кадр заданных размеров.
    #[must_use]
    pub fn new(width: u32, height: u32, stride: u32, format: u32) -> Self {
        Self {
            width,
            height,
            stride,
            format,
            dma_buf: None,
            drm_modifier: 0,
            timestamp_us: 0,
            sequence: 0,
            data: Vec::new(),
        }
    }

    /// Создаёт кадр на базе аппаратного буфера DMA-BUF (TASK-804 zero-copy).
    #[must_use]
    pub fn new_dma_buf(
        width: u32,
        height: u32,
        stride: u32,
        format: u32,
        dma_buf: OwnedDmaBuf,
        drm_modifier: u64,
    ) -> Self {
        Self {
            width,
            height,
            stride,
            format,
            dma_buf: Some(Arc::new(dma_buf)),
            drm_modifier,
            timestamp_us: 0,
            sequence: 0,
            data: Vec::new(),
        }
    }

    /// Устанавливает пиксельные данные кадра.
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Устанавливает временную метку захвата.
    pub fn with_timestamp_us(mut self, timestamp_us: u64) -> Self {
        self.timestamp_us = timestamp_us;
        self
    }

    /// Проверяет, пуст ли кадр (нулевые размеры или отсутствие данных).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || (self.data.is_empty() && self.dma_buf.is_none())
    }

    /// Возвращает дескриптор DMA-BUF (если передан аппаратный буфер).
    #[must_use]
    pub fn dma_buf_raw_fd(&self) -> Option<RawFd> {
        self.dma_buf.as_ref().map(|b| b.as_raw_fd())
    }

    /// Проверяет, является ли кадр аппаратно ускоренным буфером DMA-BUF.
    #[must_use]
    pub fn is_dma_buf(&self) -> bool {
        self.dma_buf.is_some()
    }

    /// Возвращает размер буфера кадра в байтах.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        if !self.data.is_empty() {
            self.data.len()
        } else {
            (self.stride as usize).saturating_mul(self.height as usize)
        }
    }
}

/// Телеметрия атомарного слота кадра.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameSlotMetrics {
    /// Текущий монотонный номер кадра.
    pub sequence: u64,
    /// Всего опубликованных кадров.
    pub published_frames: u64,
    /// Всего прочитанных потребителем кадров.
    pub read_frames: u64,
    /// Число кадров, перезаписанных до чтения потребителем.
    pub dropped_frames: u64,
    /// Доступен ли новый свежий кадр.
    pub has_new_frame: bool,
}

/// Атомарный неблокирующий слот кадра с тройной буферизацией (Lock-Free Triple Buffer).
///
/// Устраняет взаимные блокировки потока захвата PipeWire и потока кодирования/рендеринга.
/// Запись и чтение выполняются атомарным обменом индексов (O(1), без мьютексов и ожидания).
pub struct LockFreeFrameSlot {
    slots: [std::cell::UnsafeCell<CapturedFrame>; 3],
    shared_index: AtomicUsize,
    writer_index: AtomicUsize,
    reader_index: AtomicUsize,
    has_new_frame: AtomicBool,
    writer_busy: AtomicBool,
    reader_busy: AtomicBool,
    sequence: AtomicU64,
    published_frames: AtomicU64,
    dropped_frames: AtomicU64,
    read_frames: AtomicU64,
}

unsafe impl Sync for LockFreeFrameSlot {}
unsafe impl Send for LockFreeFrameSlot {}

impl LockFreeFrameSlot {
    /// Создаёт новый неблокирующий слот кадра.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [
                std::cell::UnsafeCell::new(CapturedFrame::default()),
                std::cell::UnsafeCell::new(CapturedFrame::default()),
                std::cell::UnsafeCell::new(CapturedFrame::default()),
            ],
            shared_index: AtomicUsize::new(1),
            writer_index: AtomicUsize::new(0),
            reader_index: AtomicUsize::new(2),
            has_new_frame: AtomicBool::new(false),
            writer_busy: AtomicBool::new(false),
            reader_busy: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            published_frames: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            read_frames: AtomicU64::new(0),
        }
    }

    /// Публикует готовый кадр в слот без блокировки потока чтения.
    pub fn publish_frame(&self, mut frame: CapturedFrame) -> bool {
        if self.writer_busy.swap(true, Ordering::Acquire) {
            return false;
        }

        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        frame.sequence = seq;

        let w_idx = self.writer_index.load(Ordering::Relaxed);
        // SAFETY: w_idx принадлежит писателю, пока writer_busy == true.
        unsafe {
            *self.slots[w_idx].get() = frame;
        }

        let next_w = self.shared_index.swap(w_idx, Ordering::AcqRel);
        self.writer_index.store(next_w, Ordering::Relaxed);

        if self.has_new_frame.swap(true, Ordering::AcqRel) {
            self.dropped_frames.fetch_add(1, Ordering::Relaxed);
        }
        self.published_frames.fetch_add(1, Ordering::Relaxed);

        self.writer_busy.store(false, Ordering::Release);
        true
    }

    /// Выполняет запись в буфер писателя на месте (in-place) без промежуточных аллокаций.
    pub fn publish_with<F>(&self, f: F) -> bool
    where
        F: FnOnce(&mut CapturedFrame),
    {
        if self.writer_busy.swap(true, Ordering::Acquire) {
            return false;
        }

        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let w_idx = self.writer_index.load(Ordering::Relaxed);

        // SAFETY: w_idx принадлежит писателю, пока writer_busy == true.
        unsafe {
            let slot = &mut *self.slots[w_idx].get();
            f(slot);
            slot.sequence = seq;
        }

        let next_w = self.shared_index.swap(w_idx, Ordering::AcqRel);
        self.writer_index.store(next_w, Ordering::Relaxed);

        if self.has_new_frame.swap(true, Ordering::AcqRel) {
            self.dropped_frames.fetch_add(1, Ordering::Relaxed);
        }
        self.published_frames.fetch_add(1, Ordering::Relaxed);

        self.writer_busy.store(false, Ordering::Release);
        true
    }

    /// Неблокирующее zero-copy чтение последнего доступного свежего кадра.
    ///
    /// Если нового кадра не поступало, возвращает `None` без блокировки вызывающего потока.
    pub fn with_latest_fresh<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&CapturedFrame) -> R,
    {
        if self.reader_busy.swap(true, Ordering::Acquire) {
            return None;
        }

        if !self.has_new_frame.swap(false, Ordering::AcqRel) {
            self.reader_busy.store(false, Ordering::Release);
            return None;
        }

        let r_idx = self.reader_index.load(Ordering::Relaxed);
        let next_r = self.shared_index.swap(r_idx, Ordering::AcqRel);
        self.reader_index.store(next_r, Ordering::Relaxed);

        // SAFETY: next_r принадлежит читателю, пока reader_busy == true.
        let result = unsafe {
            let frame = &*self.slots[next_r].get();
            f(frame)
        };

        self.read_frames.fetch_add(1, Ordering::Relaxed);
        self.reader_busy.store(false, Ordering::Release);
        Some(result)
    }

    /// Осматривает текущий кадр читателя без сброса признака свежести.
    pub fn peek_current<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&CapturedFrame) -> R,
    {
        if self.reader_busy.swap(true, Ordering::Acquire) {
            return None;
        }

        let r_idx = self.reader_index.load(Ordering::Relaxed);
        // SAFETY: r_idx принадлежит читателю.
        let result = unsafe {
            let frame = &*self.slots[r_idx].get();
            if frame.is_empty() {
                None
            } else {
                Some(f(frame))
            }
        };

        self.reader_busy.store(false, Ordering::Release);
        result
    }

    /// Проверяет наличие нового свежего кадра.
    #[must_use]
    pub fn has_new_frame(&self) -> bool {
        self.has_new_frame.load(Ordering::Acquire)
    }

    /// Текущий монотонный номер кадра.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }

    /// Возвращает метрики работы слота кадра.
    #[must_use]
    pub fn metrics(&self) -> FrameSlotMetrics {
        FrameSlotMetrics {
            sequence: self.sequence.load(Ordering::Acquire),
            published_frames: self.published_frames.load(Ordering::Relaxed),
            read_frames: self.read_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            has_new_frame: self.has_new_frame.load(Ordering::Acquire),
        }
    }
}

impl Default for LockFreeFrameSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Атомарный двухбуферный обменник кадра (Double-Buffering) с неблокирующим чтением.
pub struct DoubleBufferedFrame {
    slots: [std::sync::RwLock<CapturedFrame>; 2],
    active_index: AtomicUsize,
    has_fresh: AtomicBool,
    sequence: AtomicU64,
    published_frames: AtomicU64,
    read_frames: AtomicU64,
}

impl DoubleBufferedFrame {
    /// Создаёт новый двухбуферный обменник.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [
                std::sync::RwLock::new(CapturedFrame::default()),
                std::sync::RwLock::new(CapturedFrame::default()),
            ],
            active_index: AtomicUsize::new(0),
            has_fresh: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            published_frames: AtomicU64::new(0),
            read_frames: AtomicU64::new(0),
        }
    }

    /// Записывает кадр в фоновый буфер и атомарно переключает активный индекс.
    pub fn store(&self, mut frame: CapturedFrame) -> bool {
        let current_active = self.active_index.load(Ordering::Relaxed);
        let back_index = 1 - current_active;

        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        frame.sequence = seq;

        if let Ok(mut guard) = self.slots[back_index].try_write() {
            *guard = frame;
            drop(guard);
            self.active_index.store(back_index, Ordering::Release);
            self.has_fresh.store(true, Ordering::Release);
            self.published_frames.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Читает последний активный кадр без блокировки писателя.
    pub fn with_active<R, F: FnOnce(&CapturedFrame) -> R>(&self, f: F) -> Option<R> {
        let active = self.active_index.load(Ordering::Acquire);
        let guard = self.slots[active].try_read().ok()?;
        if guard.is_empty() {
            return None;
        }
        self.has_fresh.store(false, Ordering::Release);
        self.read_frames.fetch_add(1, Ordering::Relaxed);
        Some(f(&guard))
    }

    /// Проверяет наличие свежего кадра.
    #[must_use]
    pub fn has_fresh(&self) -> bool {
        self.has_fresh.load(Ordering::Acquire)
    }

    /// Текущий монотонный номер кадра.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }
}

impl Default for DoubleBufferedFrame {
    fn default() -> Self {
        Self::new()
    }
}

/// Разобранная спецификация дисплея X11 (переменная `DISPLAY`, например `:0`, `:1.2`, `unix:0.0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11DisplaySpec {
    /// Номер дисплея X11.
    pub display_number: u32,
    /// Номер экрана X11.
    pub screen_number: u32,
    /// Путь к локальному unix-сокету `/tmp/.X11-unix/X<N>`.
    pub unix_socket_path: PathBuf,
}

impl X11DisplaySpec {
    /// Проверяет наличие файла сокета X11 в файловой системе.
    #[must_use]
    pub fn is_socket_present(&self) -> bool {
        self.unix_socket_path.exists()
    }
}

/// Разбирает строку переменной `DISPLAY` (например, `":0"`, `":1.2"`, `"unix:0"`)
/// и возвращает путь к локальному сокету `/tmp/.X11-unix/X<display>`.
pub fn parse_x11_display_spec(display: &str) -> Result<X11DisplaySpec, PortalRequestError> {
    let trimmed = display.trim();
    let rest = if let Some(after_unix) = trimmed.strip_prefix("unix:") {
        after_unix
    } else if let Some(after_colon) = trimmed.strip_prefix(':') {
        after_colon
    } else {
        return Err(PortalRequestError::InvalidX11Display(display.to_string()));
    };

    if rest.is_empty() {
        return Err(PortalRequestError::InvalidX11Display(display.to_string()));
    }

    let (disp_str, screen_str) = match rest.split_once('.') {
        Some((d, s)) => (d, Some(s)),
        None => (rest, None),
    };

    if disp_str.is_empty() || !disp_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PortalRequestError::InvalidX11Display(display.to_string()));
    }
    let display_number = disp_str
        .parse::<u32>()
        .map_err(|_| PortalRequestError::InvalidX11Display(display.to_string()))?;

    let screen_number = match screen_str {
        Some(s) => {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(PortalRequestError::InvalidX11Display(display.to_string()));
            }
            s.parse::<u32>()
                .map_err(|_| PortalRequestError::InvalidX11Display(display.to_string()))?
        }
        None => 0,
    };

    let unix_socket_path = PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    Ok(X11DisplaySpec {
        display_number,
        screen_number,
        unix_socket_path,
    })
}

/// D-Bus клиент портала `org.freedesktop.portal.ScreenCast`.
pub struct ScreenCastPortal {
    conn: Connection,
    dialog_timeout: Duration,
    cached_pipewire_remote: Option<(String, OwnedPipeWireFd)>,
}

impl ScreenCastPortal {
    /// Подключается к сессионной шине D-Bus с согласованием `NEGOTIATE_UNIX_FD`.
    pub fn connect_session() -> Result<Self, PortalRequestError> {
        let addr = BusAddress::from_env().ok_or_else(|| {
            PortalRequestError::Dbus("DBUS_SESSION_BUS_ADDRESS не задан".to_string())
        })?;
        Self::connect(&addr)
    }

    /// Подключается к указанному адресу шины D-Bus.
    pub fn connect(address: &BusAddress) -> Result<Self, PortalRequestError> {
        let conn = Connection::connect(address)?;
        Ok(Self {
            conn,
            dialog_timeout: DEFAULT_PORTAL_DIALOG_TIMEOUT,
            cached_pipewire_remote: None,
        })
    }

    /// Устанавливает таймаут ожидания сигнала `Request.Response`.
    pub fn set_dialog_timeout(&mut self, timeout: Duration) {
        self.dialog_timeout = timeout;
    }

    /// Вызывает `CreateSession` и ожидает `session_handle` из сигнала `Request.Response`.
    pub fn create_session(
        &mut self,
        options: &ScreenCastOptions,
    ) -> Result<String, PortalRequestError> {
        options.validate()?;
        let dict = Value::Dict(vec![
            (
                Value::String("handle_token".to_string()),
                Value::Variant(Box::new(Value::String(options.handle_token.clone()))),
            ),
            (
                Value::String("session_handle_token".to_string()),
                Value::Variant(Box::new(Value::String(
                    options.session_handle_token.clone(),
                ))),
            ),
        ]);

        let reply = self.conn.call_typed(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "CreateSession",
            "a{sv}",
            &[dict],
        )?;
        let request_path = extract_object_path(&reply)?;
        let results = self.await_request_response(&request_path)?;

        if let Some(session_handle) = lookup_dict_string(&results, "session_handle") {
            validate_object_path(&session_handle)?;
            Ok(session_handle)
        } else {
            Err(PortalRequestError::Dbus(
                "в ответе CreateSession отсутствует поле session_handle".to_string(),
            ))
        }
    }

    /// Вызывает `SelectSources` для активной сессии портала.
    pub fn select_sources(
        &mut self,
        session_handle: &str,
        options: &ScreenCastOptions,
    ) -> Result<(), PortalRequestError> {
        validate_object_path(session_handle)?;
        options.validate()?;

        let mut entries = vec![
            (
                Value::String("handle_token".to_string()),
                Value::Variant(Box::new(Value::String(options.handle_token.clone()))),
            ),
            (
                Value::String("types".to_string()),
                Value::Variant(Box::new(Value::U32(options.source_types.bits()))),
            ),
            (
                Value::String("multiple".to_string()),
                Value::Variant(Box::new(Value::Bool(options.multiple))),
            ),
            (
                Value::String("cursor_mode".to_string()),
                Value::Variant(Box::new(Value::U32(options.cursor_mode as u32))),
            ),
            (
                Value::String("persist_mode".to_string()),
                Value::Variant(Box::new(Value::U32(options.persist_mode as u32))),
            ),
        ];
        if let Some(ref restore) = options.restore_token
            && !restore.is_empty()
        {
            entries.push((
                Value::String("restore_token".to_string()),
                Value::Variant(Box::new(Value::String(restore.clone()))),
            ));
        }

        let reply = self.conn.call_typed(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "SelectSources",
            "oa{sv}",
            &[
                Value::String(session_handle.to_string()),
                Value::Dict(entries),
            ],
        )?;
        let request_path = extract_object_path(&reply)?;
        let _ = self.await_request_response(&request_path)?;
        Ok(())
    }

    /// Вызывает `Start` и разбирает согласованные потоки `streams: a(ua{sv})` и `restore_token`.
    pub fn start(
        &mut self,
        session_handle: &str,
        options: &ScreenCastOptions,
    ) -> Result<(Vec<PortalStreamInfo>, Option<String>), PortalRequestError> {
        validate_object_path(session_handle)?;
        options.validate()?;

        let dict = Value::Dict(vec![(
            Value::String("handle_token".to_string()),
            Value::Variant(Box::new(Value::String(options.handle_token.clone()))),
        )]);

        let reply = self.conn.call_typed(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "Start",
            "osa{sv}",
            &[
                Value::String(session_handle.to_string()),
                Value::String(options.parent_window.clone()),
                dict,
            ],
        )?;
        let request_path = extract_object_path(&reply)?;
        let results = self.await_request_response(&request_path)?;
        Self::parse_start_results(&results)
    }

    /// Вызывает `OpenPipeWireRemote` и возвращает RAII-дескриптор сокета PipeWire (`SCM_RIGHTS`).
    ///
    /// Кэширует дескриптор для текущей сессии: повторные вызовы дублируют дескриптор
    /// через `fcntl(F_DUPFD_CLOEXEC)` без избыточных D-Bus roundtrip циклов.
    pub fn open_pipewire_remote(
        &mut self,
        session_handle: &str,
    ) -> Result<OwnedPipeWireFd, PortalRequestError> {
        validate_object_path(session_handle)?;

        if let Some((ref cached_handle, ref cached_fd)) = self.cached_pipewire_remote
            && cached_handle == session_handle
        {
            return cached_fd.try_clone();
        }

        let (values, fds) = self.conn.call_typed_with_fds(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "OpenPipeWireRemote",
            "oa{sv}",
            &[
                Value::String(session_handle.to_string()),
                Value::Dict(Vec::new()),
            ],
        )?;

        let raw_fd = match values.first() {
            Some(Value::UnixFd(fd_val)) => *fd_val,
            _ => {
                fd::close_all(&fds);
                return Err(PortalRequestError::Dbus(
                    "ответ OpenPipeWireRemote не содержит дескриптора h".to_string(),
                ));
            }
        };

        // Закрываем все побочные fd, если сервер прислал больше одного
        for extra in fds.into_iter().filter(|f| *f != raw_fd) {
            fd::close_fd(extra);
        }

        let owned = OwnedPipeWireFd::from_raw_fd_checked(raw_fd)?;
        let client_copy = owned.try_clone()?;
        self.cached_pipewire_remote = Some((session_handle.to_string(), owned));
        Ok(client_copy)
    }

    /// Возвращает числовой дескриптор кэшированного сокета PipeWire remote (если доступен).
    #[must_use]
    pub fn cached_pipewire_fd(&self) -> Option<RawFd> {
        self.cached_pipewire_remote
            .as_ref()
            .map(|(_, fd)| fd.as_raw_fd())
    }

    /// Инвалидирует кэшированный дескриптор PipeWire remote для принудительного переподключения.
    pub fn invalidate_pipewire_cache(&mut self) {
        self.cached_pipewire_remote = None;
    }

    /// Закрывает сессию портала (`org.freedesktop.portal.Session.Close`).
    pub fn close_session(&mut self, session_handle: &str) -> Result<(), PortalRequestError> {
        validate_object_path(session_handle)?;
        if let Some((ref cached_handle, _)) = self.cached_pipewire_remote
            && cached_handle == session_handle
        {
            self.cached_pipewire_remote = None;
        }
        let _ = self.conn.call_typed(
            PORTAL_BUS_NAME,
            session_handle,
            SESSION_INTERFACE,
            "Close",
            "",
            &[],
        )?;
        Ok(())
    }

    /// Неблокирующий опрос сигнала `Request.Response` без синхронного ожидания потока.
    pub fn poll_request_response(
        &mut self,
        request_path: &str,
    ) -> Result<Option<Value>, PortalRequestError> {
        let signal = match self.conn.next_signal(Duration::ZERO) {
            Ok(sig) => sig,
            Err(DbusError::Timeout) => return Ok(None),
            Err(err) => return Err(PortalRequestError::from(err)),
        };

        if signal.interface == REQUEST_INTERFACE
            && signal.member == "Response"
            && signal.path == request_path
        {
            let code = match signal.body.first() {
                Some(Value::U32(c)) => *c,
                _ => {
                    return Err(PortalRequestError::Dbus(
                        "сигнал Request.Response не содержит u32 кода".to_string(),
                    ));
                }
            };
            match code {
                0 => {
                    let results = signal
                        .body
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| Value::Dict(Vec::new()));
                    Ok(Some(results))
                }
                1 => Err(PortalRequestError::ConsentDenied),
                _ => Err(PortalRequestError::ConsentAborted),
            }
        } else {
            Ok(None)
        }
    }

    /// Отменяет активный запрос (`org.freedesktop.portal.Request.Close`).
    pub fn cancel_request(&mut self, request_handle: &str) -> Result<(), PortalRequestError> {
        validate_object_path(request_handle)?;
        let _ = self.conn.call_typed(
            PORTAL_BUS_NAME,
            request_handle,
            REQUEST_INTERFACE,
            "Close",
            "",
            &[],
        )?;
        Ok(())
    }

    /// Разбирает словарь `results` (`a{sv}`) из ответа `Start`, извлекая `streams: a(ua{sv})` и `restore_token`.
    pub fn parse_start_results(
        results: &Value,
    ) -> Result<(Vec<PortalStreamInfo>, Option<String>), PortalRequestError> {
        let restore_token = lookup_dict_string(results, "restore_token");
        let Some(streams_val) = lookup_dict_value(results, "streams") else {
            return Err(PortalRequestError::EmptyStreams);
        };
        let Value::Array(items) = streams_val else {
            return Err(PortalRequestError::Dbus(
                "поле streams имеет неверный тип (ожидался массив a(ua{sv}))".to_string(),
            ));
        };

        let mut streams = Vec::with_capacity(items.len());
        for item in items {
            let Value::Struct(fields) = item else {
                return Err(PortalRequestError::Dbus(
                    "элемент streams не является структурой (ua{sv})".to_string(),
                ));
            };
            let Some(Value::U32(node_id)) = fields.first() else {
                return Err(PortalRequestError::Dbus(
                    "первое поле структуры потока не u32 node_id".to_string(),
                ));
            };
            let props = fields.get(1).ok_or_else(|| {
                PortalRequestError::Dbus("отсутствует словарь свойств потока".to_string())
            })?;

            let id = lookup_dict_string(props, "id");
            let source_type = lookup_dict_u32(props, "source_type").unwrap_or(1);
            let position = lookup_dict_i32_pair(props, "position");
            let size = lookup_dict_i32_pair(props, "size").and_then(|(w, h)| {
                let w_u = u32::try_from(w).ok()?;
                let h_u = u32::try_from(h).ok()?;
                Some((w_u, h_u))
            });
            let mapping_id = lookup_dict_string(props, "mapping_id");

            streams.push(PortalStreamInfo {
                node_id: *node_id,
                id,
                source_type,
                position,
                size,
                mapping_id,
            });
        }

        if streams.is_empty() {
            return Err(PortalRequestError::EmptyStreams);
        }

        Ok((streams, restore_token))
    }

    fn await_request_response(&mut self, request_path: &str) -> Result<Value, PortalRequestError> {
        let match_rule = format!(
            "type='signal',interface='{REQUEST_INTERFACE}',member='Response',path='{request_path}'"
        );
        self.conn.add_match(&match_rule)?;

        let deadline = Instant::now() + self.dialog_timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                let _ = self.cancel_request(request_path);
                return Err(PortalRequestError::Timeout);
            }
            let remaining = deadline.saturating_duration_since(now);
            let signal = match self.conn.next_signal(remaining) {
                Ok(sig) => sig,
                Err(DbusError::Timeout) => {
                    let _ = self.cancel_request(request_path);
                    return Err(PortalRequestError::Timeout);
                }
                Err(err) => return Err(PortalRequestError::from(err)),
            };

            if signal.interface == REQUEST_INTERFACE
                && signal.member == "Response"
                && signal.path == request_path
            {
                let code = match signal.body.first() {
                    Some(Value::U32(c)) => *c,
                    _ => {
                        return Err(PortalRequestError::Dbus(
                            "сигнал Request.Response не содержит u32 кода".to_string(),
                        ));
                    }
                };
                match code {
                    0 => {
                        let results = signal
                            .body
                            .get(1)
                            .cloned()
                            .unwrap_or_else(|| Value::Dict(Vec::new()));
                        return Ok(results);
                    }
                    1 => return Err(PortalRequestError::ConsentDenied),
                    _ => return Err(PortalRequestError::ConsentAborted),
                }
            }
        }
    }
}

fn validate_token(token: &str) -> Result<(), PortalRequestError> {
    if token.is_empty()
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(PortalRequestError::InvalidToken(token.to_string()));
    }
    Ok(())
}

fn validate_object_path(path: &str) -> Result<(), PortalRequestError> {
    if path == "/" {
        return Ok(());
    }
    if !path.starts_with('/') || path.ends_with('/') {
        return Err(PortalRequestError::InvalidObjectPath(path.to_string()));
    }
    for seg in path[1..].split('/') {
        if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(PortalRequestError::InvalidObjectPath(path.to_string()));
        }
    }
    Ok(())
}

fn extract_object_path(reply: &[Value]) -> Result<String, PortalRequestError> {
    match reply.first() {
        Some(Value::String(path)) => {
            validate_object_path(path)?;
            Ok(path.clone())
        }
        _ => Err(PortalRequestError::Dbus(
            "ответ метода портала не содержит ObjectPath".to_string(),
        )),
    }
}

fn unwrap_variant(val: &Value) -> &Value {
    match val {
        Value::Variant(inner) => inner.as_ref(),
        other => other,
    }
}

fn lookup_dict_value<'a>(dict: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Dict(entries) = dict else {
        return None;
    };
    for (k, v) in entries {
        if let Value::String(k_str) = k
            && k_str == key
        {
            return Some(unwrap_variant(v));
        }
    }
    None
}

fn lookup_dict_string(dict: &Value, key: &str) -> Option<String> {
    match lookup_dict_value(dict, key)? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn lookup_dict_u32(dict: &Value, key: &str) -> Option<u32> {
    match lookup_dict_value(dict, key)? {
        Value::U32(n) => Some(*n),
        _ => None,
    }
}

fn lookup_dict_i32_pair(dict: &Value, key: &str) -> Option<(i32, i32)> {
    let Value::Struct(items) = lookup_dict_value(dict, key)? else {
        return None;
    };
    let (Some(Value::I32(a)), Some(Value::I32(b))) = (items.first(), items.get(1)) else {
        return None;
    };
    Some((*a, *b))
}

/// Минимально допустимый размер кадра захвата (по ширине или высоте) в пикселях.
pub const MIN_CAPTURE_DIMENSION: u32 = 64;
/// Максимально допустимая ширина кадра захвата (8K UHD).
pub const MAX_CAPTURE_WIDTH: u32 = 7680;
/// Максимально допустимая высота кадра захвата (8K UHD).
pub const MAX_CAPTURE_HEIGHT: u32 = 4320;
/// Тактовая частота видео-таймстемпов RTP (RFC 3551: 90 кГц).
pub const RTP_VIDEO_CLOCK_HZ: u32 = 90_000;
/// Минимальный целевой битрейт видеопотока (250 Кбит/с).
pub const MIN_BITRATE_BPS: u32 = 250_000;
/// Максимальный целевой битрейт видеопотока (16 Мбит/с).
pub const MAX_BITRATE_BPS: u32 = 16_000_000;

/// Тип цели захвата в конвейере `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTargetKind {
    /// Захват монитора (экрана целиком, `source_type == 1`).
    Monitor,
    /// Захват отдельного окна приложения (`source_type == 2`).
    Window,
    /// Виртуальный дисплей (`source_type == 4`).
    Virtual,
}

impl CaptureTargetKind {
    /// Преобразует числовой `source_type` из ответа портала в [`CaptureTargetKind`].
    #[must_use]
    pub const fn from_source_type(source_type: u32) -> Option<Self> {
        match source_type {
            1 => Some(Self::Monitor),
            2 => Some(Self::Window),
            4 => Some(Self::Virtual),
            _ => None,
        }
    }

    /// Возвращает битовую маску [`PortalSourceTypes`] для данного типа цели.
    #[must_use]
    pub const fn as_source_types(self) -> PortalSourceTypes {
        match self {
            Self::Monitor => PortalSourceTypes::MONITOR,
            Self::Window => PortalSourceTypes::WINDOW,
            Self::Virtual => PortalSourceTypes::VIRTUAL,
        }
    }
}

/// Стандартные пресеты разрешения демонстрации экрана Discord Go Live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureResolutionPreset {
    /// 480p SD (`854 x 480`).
    Sd480p,
    /// 720p HD (`1280 x 720`).
    Hd720p,
    /// 1080p Full HD (`1920 x 1080`).
    Fhd1080p,
    /// 1440p QHD (`2560 x 1440`).
    Qhd1440p,
    /// 2160p 4K UHD (`3840 x 2160`).
    Uhd4k,
}

impl CaptureResolutionPreset {
    /// Возвращает пару `(width, height)` для пресета разрешения.
    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Sd480p => (854, 480),
            Self::Hd720p => (1280, 720),
            Self::Fhd1080p => (1920, 1080),
            Self::Qhd1440p => (2560, 1440),
            Self::Uhd4k => (3840, 2160),
        }
    }
}

/// Возможности (capabilities) сервиса `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalCapabilities {
    /// Версия интерфейса `org.freedesktop.portal.ScreenCast` (ожидается `>= 4`, эталон `6`).
    pub version: u32,
    /// Поддерживаемые типы источников (`AvailableSourceTypes`).
    pub available_source_types: PortalSourceTypes,
    /// Битовая маска поддерживаемых режимов курсора (`AvailableCursorModes`).
    pub available_cursor_modes: u32,
}

impl PortalCapabilities {
    /// Проверяет, поддерживает ли портал все запрошенные биты типов источников.
    #[must_use]
    pub const fn supports_source_types(&self, requested: PortalSourceTypes) -> bool {
        !requested.is_empty()
            && (self.available_source_types.bits() & requested.bits()) == requested.bits()
    }

    /// Выбирает поддерживаемый порталом режим курсора с фоллбэком `preferred -> Embedded -> Hidden`.
    #[must_use]
    pub const fn select_supported_cursor_mode(
        &self,
        preferred: PortalCursorMode,
    ) -> Option<PortalCursorMode> {
        let pref_bit = preferred as u32;
        if (self.available_cursor_modes & pref_bit) != 0 {
            return Some(preferred);
        }
        if (self.available_cursor_modes & (PortalCursorMode::Embedded as u32)) != 0 {
            return Some(PortalCursorMode::Embedded);
        }
        if (self.available_cursor_modes & (PortalCursorMode::Hidden as u32)) != 0 {
            return Some(PortalCursorMode::Hidden);
        }
        if (self.available_cursor_modes & (PortalCursorMode::Metadata as u32)) != 0 {
            return Some(PortalCursorMode::Metadata);
        }
        None
    }
}

/// Запрос на согласование разрешения и частоты кадров (15 / 30 / 60 FPS) для потока портала.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCastNegotiationRequest {
    /// Максимальная ширина кадра (граница вписывания).
    pub max_width: u32,
    /// Максимальная высота кадра (граница вписывания).
    pub max_height: u32,
    /// Целевая частота кадров (`15`, `30` или `60` FPS).
    pub target_fps: u32,
    /// Целевой тип захватываемого источника (монитор или окно).
    pub target_kind: CaptureTargetKind,
    /// Пользовательский целевой битрейт в бит/с (если `None`, вычисляется автоматически).
    pub custom_bitrate_bps: Option<u32>,
}

impl ScreenCastNegotiationRequest {
    /// Создаёт запрос согласования из стандартного пресета разрешения и целевого FPS.
    #[must_use]
    pub const fn from_preset(
        preset: CaptureResolutionPreset,
        target_fps: u32,
        target_kind: CaptureTargetKind,
    ) -> Self {
        let (max_width, max_height) = preset.dimensions();
        Self {
            max_width,
            max_height,
            target_fps,
            target_kind,
            custom_bitrate_bps: None,
        }
    }

    /// Проверяет допустимость границ разрешения (`64..=7680x4320`) и частоты кадров (`15 | 30 | 60`).
    pub fn validate(&self) -> Result<(), PortalRequestError> {
        if !matches!(self.target_fps, 15 | 30 | 60) {
            return Err(PortalRequestError::Dbus(format!(
                "неподдерживаемая частота кадров ScreenCast: {} FPS (допустимы 15, 30, 60)",
                self.target_fps
            )));
        }
        if self.max_width < MIN_CAPTURE_DIMENSION
            || self.max_height < MIN_CAPTURE_DIMENSION
            || self.max_width > MAX_CAPTURE_WIDTH
            || self.max_height > MAX_CAPTURE_HEIGHT
        {
            return Err(PortalRequestError::Dbus(format!(
                "недопустимые границы разрешения ScreenCast: {}x{}",
                self.max_width, self.max_height
            )));
        }
        Ok(())
    }
}

/// Согласованные параметры видеопотока после рукопожатия `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedStreamParams {
    /// Идентификатор узла PipeWire (`node_id`).
    pub node_id: u32,
    /// Строковый идентификатор потока портала (`id`).
    pub stream_id: Option<String>,
    /// Идентификатор сопоставления монитора или окна (`mapping_id`).
    pub mapping_id: Option<String>,
    /// Согласованный тип источника.
    pub target_kind: CaptureTargetKind,
    /// Чётная ширина кадра в пикселях (выровнена по 2 для YUV420 / H.264 / AV1).
    pub width: u32,
    /// Чётная высота кадра в пикселях (выровнена по 2 для YUV420 / H.264 / AV1).
    pub height: u32,
    /// Шаг строки BGRA/RGBA буфера в байтах (`width * 4`).
    pub stride_bytes: u32,
    /// Согласованная частота кадров (`15`, `30` или `60` FPS).
    pub fps: u32,
    /// Интервал между кадрами в микросекундах (`16_666` мкс для 60 FPS).
    pub frame_interval_us: u64,
    /// Частота часов RTP видео (`90_000` Гц).
    pub rtp_clock_hz: u32,
    /// Шаг приращения RTP timestamp на один кадр (`1_500` тиков при 60 FPS).
    pub rtp_timestamp_step: u32,
    /// Рекомендуемый битрейт видеоэнкодера в бит/с.
    pub target_bitrate_bps: u32,
}

/// Выполняет согласование разрешения (с сохранением пропорций и чётным выравниванием YUV420),
/// частоты 60 FPS, шага RTP-таймстемпа и целевого битрейта для потока экрана или окна.
pub fn negotiate_stream_params(
    stream: &PortalStreamInfo,
    request: &ScreenCastNegotiationRequest,
) -> Result<NegotiatedStreamParams, PortalRequestError> {
    request.validate()?;
    if stream.node_id == 0 {
        return Err(PortalRequestError::EmptyStreams);
    }

    let (width, height) =
        negotiate_even_dimensions(stream.size, request.max_width, request.max_height)?;
    let stride_bytes = width.saturating_mul(4);
    let frame_interval_us = 1_000_000 / u64::from(request.target_fps);
    let rtp_timestamp_step = RTP_VIDEO_CLOCK_HZ / request.target_fps;
    let target_bitrate_bps = match request.custom_bitrate_bps {
        Some(custom) => custom.clamp(MIN_BITRATE_BPS, MAX_BITRATE_BPS),
        None => compute_recommended_bitrate(width, height, request.target_fps),
    };
    let target_kind =
        CaptureTargetKind::from_source_type(stream.source_type).unwrap_or(request.target_kind);

    Ok(NegotiatedStreamParams {
        node_id: stream.node_id,
        stream_id: stream.id.clone(),
        mapping_id: stream.mapping_id.clone(),
        target_kind,
        width,
        height,
        stride_bytes,
        fps: request.target_fps,
        frame_interval_us,
        rtp_clock_hz: RTP_VIDEO_CLOCK_HZ,
        rtp_timestamp_step,
        target_bitrate_bps,
    })
}

fn negotiate_even_dimensions(
    source_size: Option<(u32, u32)>,
    max_width: u32,
    max_height: u32,
) -> Result<(u32, u32), PortalRequestError> {
    let even_max_w = (max_width & !1).max(MIN_CAPTURE_DIMENSION);
    let even_max_h = (max_height & !1).max(MIN_CAPTURE_DIMENSION);

    let Some((src_w, src_h)) = source_size else {
        return Ok((even_max_w, even_max_h));
    };

    if src_w < MIN_CAPTURE_DIMENSION
        || src_h < MIN_CAPTURE_DIMENSION
        || src_w > MAX_CAPTURE_WIDTH
        || src_h > MAX_CAPTURE_HEIGHT
    {
        return Err(PortalRequestError::Dbus(format!(
            "исходный размер потока портала вне допустимых границ: {src_w}x{src_h}"
        )));
    }

    if src_w <= even_max_w && src_h <= even_max_h {
        let out_w = (src_w & !1).max(MIN_CAPTURE_DIMENSION);
        let out_h = (src_h & !1).max(MIN_CAPTURE_DIMENSION);
        return Ok((out_w, out_h));
    }

    let lhs = u64::from(src_w) * u64::from(even_max_h);
    let rhs = u64::from(src_h) * u64::from(even_max_w);
    let (scaled_w, scaled_h) = if lhs >= rhs {
        let h =
            (u64::from(src_h) * u64::from(even_max_w) + u64::from(src_w) / 2) / u64::from(src_w);
        (even_max_w, h as u32)
    } else {
        let w =
            (u64::from(src_w) * u64::from(even_max_h) + u64::from(src_h) / 2) / u64::from(src_h);
        (w as u32, even_max_h)
    };

    let aligned_w = ((scaled_w + 1) & !1).clamp(MIN_CAPTURE_DIMENSION, even_max_w);
    let aligned_h = ((scaled_h + 1) & !1).clamp(MIN_CAPTURE_DIMENSION, even_max_h);
    Ok((aligned_w, aligned_h))
}

fn compute_recommended_bitrate(width: u32, height: u32, fps: u32) -> u32 {
    let pixels = u64::from(width) * u64::from(height);
    let base_bps: u32 = if pixels <= 854 * 480 {
        2_000_000
    } else if pixels <= 1280 * 720 {
        4_000_000
    } else if pixels <= 1920 * 1080 {
        8_000_000
    } else {
        12_000_000
    };
    let scaled = if fps >= 60 {
        base_bps.saturating_mul(2)
    } else {
        base_bps
    };
    scaled.clamp(MIN_BITRATE_BPS, MAX_BITRATE_BPS)
}

/// Состояние конечного автомата конвейера захвата `ScreenCastCapturePipeline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenCastPipelineState {
    /// Конвейер создан, но сессия портала ещё не открыта.
    Idle,
    /// Сессия `CreateSession` создана.
    SessionCreated,
    /// Источники `SelectSources` выбраны.
    SourcesSelected,
    /// Активная передача кадров с согласованным узлом PipeWire и частотой FPS.
    Streaming {
        /// Идентификатор активного узла PipeWire.
        node_id: u32,
        /// Согласованная частота кадров.
        fps: u32,
    },
    /// Трансляция поставлена на паузу (`Gateway OP 22 STREAM_SET_PAUSED`).
    Paused {
        /// Идентификатор узла PipeWire.
        node_id: u32,
        /// Согласованная частота кадров.
        fps: u32,
    },
    /// Конвейер остановлен, сессия портала и дескриптор PipeWire закрыты.
    Stopped,
}

/// Полный конвейер захвата экрана и окон `org.freedesktop.portal.ScreenCast`
/// с согласованием разрешения 60 FPS, контролем темпа кадров и тройной буферизацией.
pub struct ScreenCastCapturePipeline {
    state: ScreenCastPipelineState,
    session_handle: Option<String>,
    restore_token: Option<String>,
    streams: Vec<PortalStreamInfo>,
    negotiated: Option<NegotiatedStreamParams>,
    pipewire_fd: Option<OwnedPipeWireFd>,
    frame_slot: Arc<LockFreeFrameSlot>,
    last_frame_timestamp_us: Option<u64>,
    accepted_frames: u64,
    paced_drops: u64,
    rtp_timestamp: u32,
}

impl ScreenCastCapturePipeline {
    /// Создаёт новый конвейер захвата экрана и окон в состоянии [`ScreenCastPipelineState::Idle`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ScreenCastPipelineState::Idle,
            session_handle: None,
            restore_token: None,
            streams: Vec::new(),
            negotiated: None,
            pipewire_fd: None,
            frame_slot: Arc::new(LockFreeFrameSlot::new()),
            last_frame_timestamp_us: None,
            accepted_frames: 0,
            paced_drops: 0,
            rtp_timestamp: 0,
        }
    }

    /// Возвращает текущее состояние конвейера.
    #[must_use]
    pub const fn state(&self) -> ScreenCastPipelineState {
        self.state
    }

    /// Возвращает D-Bus путь активной сессии портала (если открыта).
    #[must_use]
    pub fn session_handle(&self) -> Option<&str> {
        self.session_handle.as_deref()
    }

    /// Возвращает токен восстановления сессии (`restore_token`), полученный от портала.
    #[must_use]
    pub fn restore_token(&self) -> Option<&str> {
        self.restore_token.as_deref()
    }

    /// Возвращает согласованные параметры текущего потока.
    #[must_use]
    pub const fn negotiated_params(&self) -> Option<&NegotiatedStreamParams> {
        self.negotiated.as_ref()
    }

    /// Возвращает числовой дескриптор открытого сокета PipeWire remote.
    #[must_use]
    pub fn pipewire_raw_fd(&self) -> Option<RawFd> {
        self.pipewire_fd.as_ref().map(OwnedPipeWireFd::as_raw_fd)
    }

    /// Возвращает ссылку на неблокирующий слот кадра с тройной буферизацией.
    #[must_use]
    pub const fn frame_slot(&self) -> &Arc<LockFreeFrameSlot> {
        &self.frame_slot
    }

    /// Выполняет полное рукопожатие `CreateSession -> SelectSources -> Start -> OpenPipeWireRemote`,
    /// согласует разрешение и 60 FPS и переводит конвейер в состояние `Streaming`.
    /// При любой ошибке на промежуточном шаге автоматически закрывает сессию портала.
    pub fn start_session(
        &mut self,
        portal: &mut ScreenCastPortal,
        options: &ScreenCastOptions,
        request: &ScreenCastNegotiationRequest,
    ) -> Result<NegotiatedStreamParams, PortalRequestError> {
        options.validate()?;
        request.validate()?;

        let session_handle = portal.create_session(options)?;
        self.session_handle = Some(session_handle.clone());
        self.state = ScreenCastPipelineState::SessionCreated;

        if let Err(err) = portal.select_sources(&session_handle, options) {
            self.abort_session(portal);
            return Err(err);
        }
        self.state = ScreenCastPipelineState::SourcesSelected;

        let (streams, restore_token) = match portal.start(&session_handle, options) {
            Ok(res) => res,
            Err(err) => {
                self.abort_session(portal);
                return Err(err);
            }
        };

        let target_type_bit = request.target_kind.as_source_types().bits();
        let selected_stream = streams
            .iter()
            .find(|s| s.source_type == target_type_bit)
            .or_else(|| streams.first())
            .ok_or(PortalRequestError::EmptyStreams)?;

        let params = match negotiate_stream_params(selected_stream, request) {
            Ok(p) => p,
            Err(err) => {
                self.abort_session(portal);
                return Err(err);
            }
        };

        let pw_fd = match portal.open_pipewire_remote(&session_handle) {
            Ok(fd) => fd,
            Err(err) => {
                self.abort_session(portal);
                return Err(err);
            }
        };

        self.streams = streams;
        self.restore_token = restore_token;
        self.pipewire_fd = Some(pw_fd);
        self.negotiated = Some(params.clone());
        self.last_frame_timestamp_us = None;
        self.accepted_frames = 0;
        self.paced_drops = 0;
        self.rtp_timestamp = 0;
        self.state = ScreenCastPipelineState::Streaming {
            node_id: params.node_id,
            fps: params.fps,
        };

        Ok(params)
    }

    /// Переключает активный поток внутри открытой сессии между монитором и окном
    /// с пересогласованием разрешения и частоты кадров без повторного открытия сессии D-Bus.
    pub fn switch_stream_target(
        &mut self,
        target_kind: CaptureTargetKind,
        preset: CaptureResolutionPreset,
        target_fps: u32,
    ) -> Result<NegotiatedStreamParams, PortalRequestError> {
        let target_bit = target_kind.as_source_types().bits();
        let stream = self
            .streams
            .iter()
            .find(|s| s.source_type == target_bit)
            .ok_or(PortalRequestError::EmptyStreams)?;

        let req = ScreenCastNegotiationRequest::from_preset(preset, target_fps, target_kind);
        let params = negotiate_stream_params(stream, &req)?;

        self.state = match self.state {
            ScreenCastPipelineState::Paused { .. } => ScreenCastPipelineState::Paused {
                node_id: params.node_id,
                fps: params.fps,
            },
            _ => ScreenCastPipelineState::Streaming {
                node_id: params.node_id,
                fps: params.fps,
            },
        };
        self.negotiated = Some(params.clone());
        self.last_frame_timestamp_us = None;
        Ok(params)
    }

    /// Ставит конвейер на паузу или снимает с паузы (`Gateway OP 22 STREAM_SET_PAUSED`).
    pub fn set_paused(&mut self, paused: bool) -> Result<(), PortalRequestError> {
        match self.state {
            ScreenCastPipelineState::Streaming { node_id, fps } if paused => {
                self.state = ScreenCastPipelineState::Paused { node_id, fps };
                Ok(())
            }
            ScreenCastPipelineState::Paused { node_id, fps } if !paused => {
                self.state = ScreenCastPipelineState::Streaming { node_id, fps };
                self.last_frame_timestamp_us = None;
                Ok(())
            }
            ScreenCastPipelineState::Streaming { .. } | ScreenCastPipelineState::Paused { .. } => {
                Ok(())
            }
            _ => Err(PortalRequestError::Dbus(
                "невозможно изменить статус паузы для неактивного конвейера ScreenCast".to_string(),
            )),
        }
    }

    /// Принимает захваченный кадр (DMA-BUF или CPU), применяет пейсер 60 FPS,
    /// начисляет шаг часов RTP 90 кГц и публикует кадр в `LockFreeFrameSlot`.
    pub fn ingest_frame(&mut self, frame: CapturedFrame) -> Result<bool, PortalRequestError> {
        match self.state {
            ScreenCastPipelineState::Paused { .. } => return Ok(false),
            ScreenCastPipelineState::Streaming { .. } => {}
            _ => {
                return Err(PortalRequestError::Dbus(
                    "конвейер ScreenCast не находится в состоянии Streaming".to_string(),
                ));
            }
        }

        if frame.is_empty() {
            return Err(PortalRequestError::Dbus(
                "попытка публикации пустого кадра в ScreenCastCapturePipeline".to_string(),
            ));
        }

        let params = self.negotiated.as_ref().ok_or_else(|| {
            PortalRequestError::Dbus("отсутствуют согласованные параметры потока".to_string())
        })?;

        // Пейсинг 60 FPS с допуском 20% на джиттер планировщика композитора
        let min_interval_us = (params.frame_interval_us * 4) / 5;
        if let Some(prev_ts) = self.last_frame_timestamp_us
            && frame.timestamp_us >= prev_ts
            && (frame.timestamp_us - prev_ts) < min_interval_us
        {
            self.paced_drops += 1;
            return Ok(false);
        }

        self.last_frame_timestamp_us = Some(frame.timestamp_us);
        if self.accepted_frames > 0 {
            self.rtp_timestamp = self.rtp_timestamp.wrapping_add(params.rtp_timestamp_step);
        }
        self.accepted_frames += 1;

        Ok(self.frame_slot.publish_frame(frame))
    }

    /// Неблокирующее чтение последнего свежего кадра вместе с текущим RTP timestamp (90 кГц).
    pub fn poll_latest_frame<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&CapturedFrame, u32) -> R,
    {
        let rtp_ts = self.rtp_timestamp;
        self.frame_slot.with_latest_fresh(|frame| f(frame, rtp_ts))
    }

    /// Останавливает конвейер, закрывает дескриптор PipeWire и сессию D-Bus портала.
    pub fn stop(&mut self, portal: &mut ScreenCastPortal) -> Result<(), PortalRequestError> {
        self.pipewire_fd = None;
        let res = if let Some(handle) = self.session_handle.take() {
            portal.close_session(&handle)
        } else {
            Ok(())
        };
        self.state = ScreenCastPipelineState::Stopped;
        res
    }

    fn abort_session(&mut self, portal: &mut ScreenCastPortal) {
        self.pipewire_fd = None;
        if let Some(handle) = self.session_handle.take() {
            let _ = portal.close_session(&handle);
        }
        self.state = ScreenCastPipelineState::Stopped;
    }
}

impl Default for ScreenCastCapturePipeline {
    fn default() -> Self {
        Self::new()
    }
}
