//! Атомарный неблокирующий (lock-free) 60 FPS кольцевой буфер последнего видеокадра
//! (`LatestFrameRingBuffer`, `AtomicFrameSlot`, `LatestFrameSlot`) с жёстким бюджетом памяти `< 50 МБ`.
//!
//! Оракулы:
//! - `/home/sanya/re-deep/voice-deep.md` и `docs/re/voice-deep.md` (§3: видеопоток 1280x720@30/60);
//! - `/home/sanya/re-deep/native-re.md` (`VoiceEngine.getNextVideoOutputFrame`, `WumpusVideoSource`,
//!   `FrameBuffer/Pool`, `BaseCapturerPipeWire`, `ScreenCapturerX11`, `WindowCapturerX11`, `XServerPixelBuffer`);
//! - `docs/specs/STREAM-WAYLAND-GUIDE.md` (§1.3: тройной кольцевой буфер `g_cap_slots[3]` с атомарным
//!   индексом активного слота и защитой от разрывов кадра);
//! - `Serein/apps/desktop/src/watch.rs` (`store_frame` L21-38, `Watch::poll` L297-304);
//! - `Serein/crates/discord-voice/src/screen.rs` (`retain_screen_frame` L442-454).

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use crate::capture::CaptureError;
use crate::capture::portal::{CropRect, PipeWireFrameBuffer, PipeWirePixelFormat};
use crate::capture::x11::{X11MonitorGeometry, X11ShmPixelBuffer};
use crate::{ColorSpace, FRAME_BUDGET_60FPS, PixelLayout, VideoFrame};

/// Максимально допустимая задержка операции записи/чтения слота (< 2 мс при бюджете кадра 16 мс).
pub const MAX_SLOT_LATENCY: Duration = Duration::from_millis(2);

/// Порог ёмкости буфера (1 МБ), выше которого старый буфер освобождается при сильном падении разрешения
/// (`Serein/apps/desktop/src/watch.rs::store_frame`).
pub const LARGE_BUFFER_SHRINK_THRESHOLD: usize = 1024 * 1024;

/// Кратность уменьшения кадра (`capacity > len * 4`), при которой сбрасывается буфер высокого разрешения.
pub const SHRINK_RATIO_MULTIPLIER: usize = 4;

/// Число атомарных слотов в кольцевом буфере по умолчанию (тройная буферизация: `Writing` + `Ready` + `Locked`).
pub const DEFAULT_RING_BUFFER_SLOTS: usize = 3;

/// Минимально допустимое число атомарных слотов в кольцевом буфере.
pub const MIN_RING_BUFFER_SLOTS: usize = 2;

/// Максимально допустимое число атомарных слотов в кольцевом буфере.
pub const MAX_RING_BUFFER_SLOTS: usize = 8;

/// Жёсткий верхний предел суммарной памяти кольцевого буфера кадров (`50 МиБ` = `52_428_800` байт).
pub const MAX_RING_BUFFER_MEMORY_BYTES: usize = 50 * 1024 * 1024;

/// Максимально допустимый размер одного несжатого видеокадра в слоте (`16 МиБ`, чтобы 3 слота гарантированно укладывались в `< 50 МБ`).
pub const MAX_SINGLE_FRAME_BYTES: usize = 16 * 1024 * 1024;

const NO_LATEST_SLOT: usize = usize::MAX;

/// Применяет правило точного переиспользования и сброса избыточной ёмкости буфера кадра
/// (`Serein/apps/desktop/src/watch.rs::store_frame`).
pub(crate) fn prepare_reused_vec(data: &mut Vec<u8>, required_len: usize) {
    if data.capacity() > LARGE_BUFFER_SHRINK_THRESHOLD
        && data.capacity() > required_len.saturating_mul(SHRINK_RATIO_MULTIPLIER)
    {
        *data = Vec::new();
    }
    data.clear();
    data.reserve_exact(required_len);
}

/// Заимствованное zero-copy представление видеокадра внутри [`LatestFrameSlot`] и [`LatestFrameRingBuffer`]
/// для кодировщика, GPU-шейдера и Tauri IPC без клонирования `Vec<u8>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorrowedFrameView<'a> {
    /// Ширина кадра в пикселях.
    pub width: u32,
    /// Высота кадра в пикселях.
    pub height: u32,
    /// Отображаемый размер с учётом поворота `(display_width, display_height)`.
    pub display_size: (u32, u32),
    /// Формат размещения плоскостей (`Rgba`, `Nv12`, `I420`).
    pub layout: PixelLayout,
    /// Цветовое пространство и диапазон (`BT.601` / `BT.709`).
    pub color: ColorSpace,
    /// Поворот кадра в градусах (`0`, `90`, `180`, `270`).
    pub rotation: u16,
    /// Временная метка захвата кадра в микросекундах.
    pub timestamp_us: u64,
    /// Монотонный порядковый номер записанного в слот кадра (`1..`).
    pub sequence: u64,
    /// Признак того, что этот кадр был новым (свежим) в момент чтения.
    pub is_fresh: bool,
    /// Заимствованный срез всех байтов кадра без копирования.
    pub data: &'a [u8],
    /// Необязательный аппаратный дескриптор DMA-BUF для zero-copy (TASK-804).
    pub dma_buf_fd: Option<i32>,
    /// DRM формат-модификатор (DRM modifier).
    pub drm_modifier: u64,
}

impl<'a> BorrowedFrameView<'a> {
    /// Создаёт zero-copy представление из ссылки на [`VideoFrame`].
    #[must_use]
    pub fn from_frame(frame: &'a VideoFrame, sequence: u64, is_fresh: bool) -> Self {
        Self {
            width: frame.width,
            height: frame.height,
            display_size: frame.display_size(),
            layout: frame.layout,
            color: frame.color,
            rotation: frame.rotation,
            timestamp_us: frame.timestamp_us,
            sequence,
            is_fresh,
            data: &frame.data,
            dma_buf_fd: frame.dma_buf_fd,
            drm_modifier: frame.drm_modifier,
        }
    }

    /// Проверяет, является ли кадр аппаратным буфером DMA-BUF (TASK-804).
    #[must_use]
    pub const fn is_dma_buf(&self) -> bool {
        self.dma_buf_fd.is_some()
    }

    /// Возвращает файловый дескриптор DMA-BUF (если передан аппаратный буфер).
    #[must_use]
    pub const fn dma_buf_fd(&self) -> Option<i32> {
        self.dma_buf_fd
    }

    /// Возвращает DRM модификатор кадра.
    #[must_use]
    pub const fn drm_modifier(&self) -> u64 {
        self.drm_modifier
    }

    /// Длина буфера кадра в байтах.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.data.len()
    }

    /// Указатель на начало буфера кадра (для верификации zero-copy и переиспользования памяти).
    #[must_use]
    pub const fn data_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    /// Возвращает заимствованный срез плоскости яркости `Y` для `I420` или `Nv12` (`None` для `Rgba`).
    #[must_use]
    pub fn luma_plane(&self) -> Option<&'a [u8]> {
        match self.layout {
            PixelLayout::I420 | PixelLayout::Nv12 => {
                let y_len = (self.width as usize).checked_mul(self.height as usize)?;
                self.data.get(..y_len)
            }
            PixelLayout::Rgba => None,
        }
    }

    /// Возвращает кортеж заимствованных плоскостей `(Y, U, V)` для формата `I420` без копирования.
    #[must_use]
    pub fn i420_planes(&self) -> Option<(&'a [u8], &'a [u8], &'a [u8])> {
        if self.layout != PixelLayout::I420 {
            return None;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let y_len = w.checked_mul(h)?;
        let c_len = w.div_ceil(2).checked_mul(h.div_ceil(2))?;
        if self.data.len() != y_len.checked_add(c_len.checked_mul(2)?)? {
            return None;
        }
        let (y_plane, chroma) = self.data.split_at(y_len);
        let (u_plane, v_plane) = chroma.split_at(c_len);
        Some((y_plane, u_plane, v_plane))
    }

    /// Возвращает кортеж заимствованных плоскостей `(Y, UV)` для формата `Nv12` без копирования.
    #[must_use]
    pub fn nv12_planes(&self) -> Option<(&'a [u8], &'a [u8])> {
        if self.layout != PixelLayout::Nv12 {
            return None;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let y_len = w.checked_mul(h)?;
        let uv_len = w.div_ceil(2).checked_mul(h.div_ceil(2))?.checked_mul(2)?;
        if self.data.len() != y_len.checked_add(uv_len)? {
            return None;
        }
        Some(self.data.split_at(y_len))
    }

    /// Возвращает заимствованный срез упакованных пикселей `RGBA8` (`None` для `I420`/`Nv12`).
    #[must_use]
    pub fn rgba_bytes(&self) -> Option<&'a [u8]> {
        if self.layout == PixelLayout::Rgba {
            Some(self.data)
        } else {
            None
        }
    }
}

/// Состояние атомарного слота кольцевого буфера (`AtomicFrameSlot`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameSlotState {
    /// Слот свободен для записи нового кадра (`idle`).
    #[default]
    Idle = 0,
    /// Слот в данный момент заполняется потоком захвата (`writing`).
    Writing = 1,
    /// Слот содержит опубликованный свежий кадр, готовый к чтению (`ready`).
    Ready = 2,
    /// Слот удерживается читателем (UI / GPU / кодировщик) для zero-copy чтения без разрывов (`locked`).
    Locked = 3,
}

impl FrameSlotState {
    /// Декодирует состояние слота из атомарного `u8`.
    #[must_use]
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Writing,
            2 => Self::Ready,
            3 => Self::Locked,
            _ => Self::Idle,
        }
    }

    /// Строковое представление состояния слота (`idle`, `writing`, `ready`, `locked`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Writing => "writing",
            Self::Ready => "ready",
            Self::Locked => "locked",
        }
    }
}

/// Атомарный слот видеокадра в составе неблокирующего кольцевого буфера [`LatestFrameRingBuffer`].
///
/// Координация между потоком захвата (Producer) и потоком чтения (Consumer) осуществляется
/// через неблокирующие CAS-переходы (`compare_exchange`) атомарного состояния `state`:
/// - писатель захватывает только `Idle` (или устаревший `Ready`) слот в `Writing`;
/// - читатель захватывает опубликованный слот в `Locked`, гарантируя отсутствие разрывов кадра (zero tearing).
#[derive(Debug)]
pub struct AtomicFrameSlot {
    index: usize,
    state: AtomicU8,
    sequence: AtomicU64,
    timestamp_us: AtomicU64,
    capacity_bytes: AtomicUsize,
    frame: Mutex<Option<VideoFrame>>,
}

impl AtomicFrameSlot {
    /// Создаёт новый пустой атомарный слот с заданным индексом.
    #[must_use]
    pub fn new(index: usize) -> Self {
        Self {
            index,
            state: AtomicU8::new(FrameSlotState::Idle as u8),
            sequence: AtomicU64::new(0),
            timestamp_us: AtomicU64::new(0),
            capacity_bytes: AtomicUsize::new(0),
            frame: Mutex::new(None),
        }
    }

    /// Индекс слота в кольцевом буфере (`0..slot_count`).
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Текущее атомарное состояние слота ([`FrameSlotState`]).
    #[must_use]
    pub fn state(&self) -> FrameSlotState {
        FrameSlotState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Порядковый номер последнего записанного в этот слот кадра (`0`, если слот пуст).
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }

    /// Временная метка кадра в слоте в микросекундах (`0`, если слот пуст).
    #[must_use]
    pub fn timestamp_us(&self) -> u64 {
        self.timestamp_us.load(Ordering::Acquire)
    }

    /// Текущая выделенная ёмкость буфера памяти в этом слоте (в байтах).
    #[must_use]
    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes.load(Ordering::Acquire)
    }

    /// Пытается атомарно перевести свободный слот из `Idle` в `Writing` (`CAS`).
    pub fn try_acquire_write(&self) -> bool {
        self.state
            .compare_exchange(
                FrameSlotState::Idle as u8,
                FrameSlotState::Writing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Пытается атомарно перехватить непрочитанный `Ready`-слот для перезаписи более свежим кадром (`CAS`).
    pub fn try_steal_ready_for_write(&self) -> bool {
        self.state
            .compare_exchange(
                FrameSlotState::Ready as u8,
                FrameSlotState::Writing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Пытается атомарно перевести слот в состояние чтения `Locked` из `Ready` или `Idle` (если в слоте есть кадр).
    pub fn try_acquire_read(&self, allow_idle_retained: bool) -> Option<FrameSlotState> {
        if self
            .state
            .compare_exchange(
                FrameSlotState::Ready as u8,
                FrameSlotState::Locked as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            return Some(FrameSlotState::Ready);
        }
        if allow_idle_retained
            && self.sequence.load(Ordering::Acquire) > 0
            && self
                .state
                .compare_exchange(
                    FrameSlotState::Idle as u8,
                    FrameSlotState::Locked as u8,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
        {
            return Some(FrameSlotState::Idle);
        }
        None
    }
}

/// RAII-страж состояния `Locked` для чтения из [`AtomicFrameSlot`], гарантирующий возврат слота
/// в `Idle` (или `Ready` при `peek`) даже при раннем выходе из замыкания.
struct LockedSlotReadGuard<'a> {
    ring: &'a RingBufferInner,
    slot_idx: usize,
    restore_ready_if_latest: bool,
}

impl Drop for LockedSlotReadGuard<'_> {
    fn drop(&mut self) {
        let slot = &self.ring.slots[self.slot_idx];
        let restore_state = if self.restore_ready_if_latest
            && self.ring.latest_slot_index.load(Ordering::Acquire) == self.slot_idx
            && self.ring.fresh.load(Ordering::Acquire)
        {
            FrameSlotState::Ready as u8
        } else {
            FrameSlotState::Idle as u8
        };
        slot.state.store(restore_state, Ordering::Release);
    }
}

/// Сводный снимок телеметрии и состояния [`LatestFrameRingBuffer`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RingBufferMetrics {
    /// Количество атомарных слотов в кольцевом буфере.
    pub slot_count: usize,
    /// Верхний предел допустимой памяти кольцевого буфера в байтах (`<= 50 МиБ`).
    pub max_memory_bytes: usize,
    /// Текущая суммарная выделенная ёмкость памяти во всех слотах (в байтах).
    pub allocated_memory_bytes: usize,
    /// Пиковая зафиксированная выделенная память (в байтах).
    pub peak_memory_bytes: usize,
    /// Общее число успешно опубликованных кадров.
    pub stored_frames: u64,
    /// Число устаревших кадров, вытесненных до чтения потребителем.
    pub overwritten_frames: u64,
    /// Общее число отклонённых кадров (невалидные параметры или превышение лимита памяти).
    pub rejected_frames: u64,
    /// Число кадров, отклонённых специально из-за защиты бюджета памяти `< 50 МБ`.
    pub memory_limit_rejections: u64,
    /// Число успешных чтений кадра потребителем.
    pub polled_frames: u64,
    /// Монотонный порядковый номер последнего опубликованного кадра.
    pub sequence: u64,
    /// Индекс активного опубликованного слота (`None`, если кадров нет).
    pub latest_slot_index: Option<usize>,
    /// Ожидает ли в кольцевом буфере непрочитанный свежий кадр.
    pub fresh: bool,
    /// Длительность последней операции публикации в наносекундах.
    pub last_store_duration_ns: u64,
    /// Максимальная зафиксированная длительность публикации в наносекундах.
    pub max_store_duration_ns: u64,
    /// Состояния всех слотов кольцевого буфера.
    pub slot_states: Vec<FrameSlotState>,
    /// Выделенная ёмкость каждого слота в байтах.
    pub slot_capacities: Vec<usize>,
}

#[derive(Debug)]
struct RingBufferInner {
    slots: Vec<AtomicFrameSlot>,
    max_memory_bytes: usize,
    allocated_memory_bytes: AtomicUsize,
    peak_memory_bytes: AtomicUsize,
    latest_slot_index: AtomicUsize,
    write_cursor: AtomicUsize,
    fresh: AtomicBool,
    stored_frames: AtomicU64,
    overwritten_frames: AtomicU64,
    rejected_frames: AtomicU64,
    memory_limit_rejections: AtomicU64,
    polled_frames: AtomicU64,
    sequence: AtomicU64,
    last_store_duration_ns: AtomicU64,
    max_store_duration_ns: AtomicU64,
}

/// Неблокирующий (lock-free по координации слотов) 60 FPS кольцевой буфер последнего видеокадра
/// с атомарными слотами [`AtomicFrameSlot`] и строгим ограничением памяти `< 50 МБ`.
///
/// Исключает разрывы кадра (tearing) и блокировки между потоком захвата PipeWire/X11 и
/// потребителями (UI / GPU-шейдер / аппаратный видеокодировщик): пока читатель удерживает
/// опубликованный слот в состоянии [`FrameSlotState::Locked`], писатель атомарно выбирает
/// свободный соседний слот в состоянии [`FrameSlotState::Idle`] и публикует новый кадр без ожидания.
#[derive(Clone, Debug)]
pub struct LatestFrameRingBuffer {
    inner: Arc<RingBufferInner>,
}

impl Default for LatestFrameRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl LatestFrameRingBuffer {
    /// Создаёт кольцевой буфер с 3 атомарными слотами (`DEFAULT_RING_BUFFER_SLOTS`) и бюджетом `< 50 МБ`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_slots_and_max_memory(DEFAULT_RING_BUFFER_SLOTS, MAX_RING_BUFFER_MEMORY_BYTES)
    }

    /// Создаёт кольцевой буфер с заданным числом слотов (`2..=8`) и бюджетом `< 50 МБ`.
    #[must_use]
    pub fn with_slots(slot_count: usize) -> Self {
        Self::with_slots_and_max_memory(slot_count, MAX_RING_BUFFER_MEMORY_BYTES)
    }

    /// Создаёт кольцевой буфер с заданным числом слотов (`2..=8`) и верхним пределом памяти
    /// (ограниченным сверху константой [`MAX_RING_BUFFER_MEMORY_BYTES`] = `50 МиБ`).
    #[must_use]
    pub fn with_slots_and_max_memory(slot_count: usize, max_memory_bytes: usize) -> Self {
        let clamped_slots = slot_count.clamp(MIN_RING_BUFFER_SLOTS, MAX_RING_BUFFER_SLOTS);
        let clamped_memory = max_memory_bytes.clamp(1, MAX_RING_BUFFER_MEMORY_BYTES);
        let slots = (0..clamped_slots).map(AtomicFrameSlot::new).collect();
        Self {
            inner: Arc::new(RingBufferInner {
                slots,
                max_memory_bytes: clamped_memory,
                allocated_memory_bytes: AtomicUsize::new(0),
                peak_memory_bytes: AtomicUsize::new(0),
                latest_slot_index: AtomicUsize::new(NO_LATEST_SLOT),
                write_cursor: AtomicUsize::new(0),
                fresh: AtomicBool::new(false),
                stored_frames: AtomicU64::new(0),
                overwritten_frames: AtomicU64::new(0),
                rejected_frames: AtomicU64::new(0),
                memory_limit_rejections: AtomicU64::new(0),
                polled_frames: AtomicU64::new(0),
                sequence: AtomicU64::new(0),
                last_store_duration_ns: AtomicU64::new(0),
                max_store_duration_ns: AtomicU64::new(0),
            }),
        }
    }

    /// Количество атомарных слотов в кольцевом буфере.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.inner.slots.len()
    }

    /// Верхний предел памяти кольцевого буфера в байтах (`<= 50 МиБ`).
    #[must_use]
    pub fn max_memory_bytes(&self) -> usize {
        self.inner.max_memory_bytes
    }

    /// Текущая суммарная выделенная ёмкость памяти во всех слотах (в байтах).
    #[must_use]
    pub fn allocated_memory_bytes(&self) -> usize {
        self.inner.allocated_memory_bytes.load(Ordering::Acquire)
    }

    /// Пиковая зафиксированная выделенная память (в байтах).
    #[must_use]
    pub fn peak_memory_bytes(&self) -> usize {
        self.inner.peak_memory_bytes.load(Ordering::Acquire)
    }

    /// Возвращает ссылку на срез всех атомарных слотов [`AtomicFrameSlot`].
    #[must_use]
    pub fn slots(&self) -> &[AtomicFrameSlot] {
        &self.inner.slots
    }

    /// Возвращает ссылку на атомарный слот по индексу.
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<&AtomicFrameSlot> {
        self.inner.slots.get(index)
    }

    /// Возвращает текущее состояние слота по индексу.
    #[must_use]
    pub fn slot_state(&self, index: usize) -> Option<FrameSlotState> {
        self.inner.slots.get(index).map(AtomicFrameSlot::state)
    }

    /// Индекс слота, содержащего последний опубликованный кадр (`None`, если буфер пуст).
    #[must_use]
    pub fn latest_slot_index(&self) -> Option<usize> {
        let idx = self.inner.latest_slot_index.load(Ordering::Acquire);
        if idx < self.inner.slots.len() {
            Some(idx)
        } else {
            None
        }
    }

    fn record_rejected(&self) {
        self.inner.rejected_frames.fetch_add(1, Ordering::Relaxed);
    }

    fn record_memory_limit_rejection(&self) {
        self.inner.rejected_frames.fetch_add(1, Ordering::Relaxed);
        self.inner
            .memory_limit_rejections
            .fetch_add(1, Ordering::Relaxed);
    }

    fn record_store_latency(&self, elapsed: Duration) {
        let ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.inner
            .last_store_duration_ns
            .store(ns, Ordering::Release);
        self.inner
            .max_store_duration_ns
            .fetch_max(ns, Ordering::AcqRel);
    }

    fn sync_allocated_bytes(&self) -> usize {
        let total: usize = self
            .inner
            .slots
            .iter()
            .map(AtomicFrameSlot::capacity_bytes)
            .sum();
        self.inner
            .allocated_memory_bytes
            .store(total, Ordering::Release);
        self.inner
            .peak_memory_bytes
            .fetch_max(total, Ordering::AcqRel);
        total
    }

    /// Атомарно выбирает слот для записи без блокировки слота читателя (`Locked`).
    /// Возвращает `(slot_index, stole_ready_slot)`.
    fn acquire_writable_slot(&self) -> Option<(usize, bool)> {
        let n = self.inner.slots.len();
        let latest = self.inner.latest_slot_index.load(Ordering::Acquire);
        let start = self.inner.write_cursor.fetch_add(1, Ordering::Relaxed) % n;

        // Проход 1: ищем свободный Idle-слот, отличный от последнего опубликованного слота,
        // чтобы текущий кадр оставался доступным читателям во время заполнения нового слота.
        for offset in 0..n {
            let idx = (start + offset) % n;
            if idx == latest {
                continue;
            }
            if self.inner.slots[idx].try_acquire_write() {
                return Some((idx, false));
            }
        }

        // Проход 2: если последний опубликованный слот уже прочитан (Idle), переиспользуем его.
        if latest < n && self.inner.slots[latest].try_acquire_write() {
            return Some((latest, false));
        }

        // Проход 3: перехватываем непрочитанный Ready-слот (авто-вытеснение устаревшего кадра).
        for offset in 0..n {
            let idx = (start + offset) % n;
            if self.inner.slots[idx].try_steal_ready_for_write() {
                return Some((idx, true));
            }
        }

        None
    }

    /// Освобождает память неактивных `Idle`-слотов (кроме `exclude_idx`), когда приближается предел `< 50 МБ`.
    fn reclaim_idle_slots_except(&self, exclude_idx: usize) {
        let latest = self.inner.latest_slot_index.load(Ordering::Acquire);
        for (idx, slot) in self.inner.slots.iter().enumerate() {
            if idx == exclude_idx || slot.capacity_bytes() == 0 {
                continue;
            }
            // В первую очередь освобождаем не-latest слоты, но при нехватке бюджета можем освободить любой Idle-слот
            if idx != latest && slot.try_acquire_write() {
                if let Ok(mut guard) = slot.frame.lock() {
                    *guard = None;
                }
                slot.capacity_bytes.store(0, Ordering::Release);
                slot.state
                    .store(FrameSlotState::Idle as u8, Ordering::Release);
            }
        }
        self.sync_allocated_bytes();
    }

    /// Проверяет и гарантирует, что выделение `required_len` байт в слоте `slot_idx`
    /// сохранит суммарную память кольцевого буфера строго `< max_memory_bytes` (`< 50 МБ`).
    /// Возвращает `true`, если память укладывается в бюджет (при необходимости сбрасывая старый буфер слота).
    fn ensure_memory_budget_for_slot(
        &self,
        slot_idx: usize,
        slot_data: &mut Vec<u8>,
        required_len: usize,
    ) -> bool {
        if required_len > MAX_SINGLE_FRAME_BYTES || required_len >= self.inner.max_memory_bytes {
            return false;
        }

        let other_bytes = |ring: &Self| -> usize {
            ring.inner
                .slots
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != slot_idx)
                .map(|(_, s)| s.capacity_bytes())
                .sum()
        };

        let projected_cap = if slot_data.capacity() > LARGE_BUFFER_SHRINK_THRESHOLD
            && slot_data.capacity() > required_len.saturating_mul(SHRINK_RATIO_MULTIPLIER)
        {
            required_len
        } else {
            slot_data.capacity().max(required_len)
        };

        if other_bytes(self).saturating_add(projected_cap) >= self.inner.max_memory_bytes {
            self.reclaim_idle_slots_except(slot_idx);
        }

        let others_after_reclaim = other_bytes(self);
        if others_after_reclaim.saturating_add(required_len) >= self.inner.max_memory_bytes {
            return false;
        }

        // Если старая ёмкость текущего слота выше required_len и вместе с другими слотами превышает лимит,
        // сбрасываем буфер текущего слота, чтобы выделить ровно required_len.
        if others_after_reclaim.saturating_add(slot_data.capacity()) >= self.inner.max_memory_bytes
            && slot_data.capacity() > required_len
        {
            *slot_data = Vec::new();
        }

        true
    }

    fn publish_written_slot(
        &self,
        slot_idx: usize,
        new_frame: VideoFrame,
        stole_ready: bool,
    ) -> u64 {
        let slot = &self.inner.slots[slot_idx];
        let new_cap = new_frame.data.capacity();
        let ts = new_frame.timestamp_us;

        if let Ok(mut guard) = slot.frame.lock() {
            *guard = Some(new_frame);
        }
        slot.capacity_bytes.store(new_cap, Ordering::Release);
        self.sync_allocated_bytes();

        let seq = self.inner.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        slot.sequence.store(seq, Ordering::Release);
        slot.timestamp_us.store(ts, Ordering::Release);
        slot.state
            .store(FrameSlotState::Ready as u8, Ordering::Release);

        let prev_latest = self
            .inner
            .latest_slot_index
            .swap(slot_idx, Ordering::AcqRel);
        let mut prev_was_ready = false;
        if prev_latest < self.inner.slots.len() && prev_latest != slot_idx {
            prev_was_ready = self.inner.slots[prev_latest]
                .state
                .compare_exchange(
                    FrameSlotState::Ready as u8,
                    FrameSlotState::Idle as u8,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok();
        }

        let was_fresh = self.inner.fresh.swap(true, Ordering::AcqRel);
        if was_fresh || stole_ready || prev_was_ready {
            self.inner
                .overwritten_frames
                .fetch_add(1, Ordering::Relaxed);
        }
        self.inner.stored_frames.fetch_add(1, Ordering::Relaxed);
        seq
    }

    fn abort_writable_slot(&self, slot_idx: usize, return_buf: Vec<u8>) {
        let slot = &self.inner.slots[slot_idx];
        if !return_buf.is_empty() || return_buf.capacity() > 0 {
            if let Ok(mut guard) = slot.frame.lock()
                && guard.is_none()
            {
                let cap = return_buf.capacity();
                slot.capacity_bytes.store(cap, Ordering::Release);
                *guard = Some(VideoFrame::new(1, 1, PixelLayout::Rgba, return_buf));
            }
            self.sync_allocated_bytes();
        }
        slot.state
            .store(FrameSlotState::Idle as u8, Ordering::Release);
    }

    /// Публикует валидный [`VideoFrame`] в свободный атомарный слот кольцевого буфера
    /// с переиспользованием памяти и контролем бюджета `< 50 МБ`.
    pub fn store_frame(&self, frame: VideoFrame) -> bool {
        let t0 = Instant::now();
        if !frame.is_valid() {
            self.record_rejected();
            return false;
        }
        let required_len = if frame.is_dma_buf() {
            0
        } else {
            frame.data.len()
        };
        if required_len > MAX_SINGLE_FRAME_BYTES || required_len >= self.inner.max_memory_bytes {
            self.record_memory_limit_rejection();
            return false;
        }

        let Some((slot_idx, stole_ready)) = self.acquire_writable_slot() else {
            self.record_rejected();
            return false;
        };

        let slot = &self.inner.slots[slot_idx];
        let mut reused_data = slot
            .frame
            .lock()
            .ok()
            .and_then(|mut g| g.take().map(|old| old.data))
            .unwrap_or_default();

        if !self.ensure_memory_budget_for_slot(slot_idx, &mut reused_data, required_len) {
            slot.capacity_bytes
                .store(reused_data.capacity(), Ordering::Release);
            self.abort_writable_slot(slot_idx, reused_data);
            self.record_memory_limit_rejection();
            return false;
        }

        if frame.is_dma_buf() {
            reused_data = Vec::new();
        } else {
            prepare_reused_vec(&mut reused_data, required_len);
            reused_data.extend_from_slice(&frame.data);
        }

        let new_frame = VideoFrame {
            data: reused_data,
            ..frame
        };
        let _ = self.publish_written_slot(slot_idx, new_frame, stole_ready);
        self.record_store_latency(t0.elapsed());
        true
    }

    /// Заполняет переиспользуемый буфер выбранного атомарного слота (`Writing`) на месте
    /// и публикует готовый кадр (`Ready`) без блокировки слота, удерживаемого читателем (`Locked`).
    #[allow(clippy::too_many_arguments)]
    pub fn store_with_reused_buffer<F, E>(
        &self,
        width: u32,
        height: u32,
        layout: PixelLayout,
        color: ColorSpace,
        rotation: u16,
        timestamp_us: u64,
        fill: F,
    ) -> Result<u64, E>
    where
        F: FnOnce(&mut [u8]) -> Result<(), E>,
        E: From<&'static str>,
    {
        let t0 = Instant::now();
        if width == 0 || height == 0 || !matches!(rotation, 0 | 90 | 180 | 270) {
            self.record_rejected();
            return Err(E::from(
                "Некорректные размеры или угол поворота кадра для LatestFrameRingBuffer",
            ));
        }
        let Some(required_len) = layout.byte_len(width, height) else {
            self.record_rejected();
            return Err(E::from("Переполнение размера буфера кадра"));
        };
        if required_len > MAX_SINGLE_FRAME_BYTES || required_len >= self.inner.max_memory_bytes {
            self.record_memory_limit_rejection();
            return Err(E::from("Превышен лимит памяти кольцевого буфера (< 50 МБ)"));
        }

        let Some((slot_idx, stole_ready)) = self.acquire_writable_slot() else {
            self.record_rejected();
            return Err(E::from("Нет доступных атомарных слотов для записи кадра"));
        };

        let slot = &self.inner.slots[slot_idx];
        let mut data = slot
            .frame
            .lock()
            .ok()
            .and_then(|mut g| g.take().map(|old| old.data))
            .unwrap_or_default();

        if !self.ensure_memory_budget_for_slot(slot_idx, &mut data, required_len) {
            slot.capacity_bytes
                .store(data.capacity(), Ordering::Release);
            self.abort_writable_slot(slot_idx, data);
            self.record_memory_limit_rejection();
            return Err(E::from("Превышен лимит памяти кольцевого буфера (< 50 МБ)"));
        }

        prepare_reused_vec(&mut data, required_len);
        data.resize(required_len, 0);

        if let Err(err) = fill(&mut data) {
            data.clear();
            slot.capacity_bytes
                .store(data.capacity(), Ordering::Release);
            self.abort_writable_slot(slot_idx, data);
            self.record_rejected();
            return Err(err);
        }

        let new_frame = VideoFrame {
            width,
            height,
            layout,
            color,
            rotation,
            timestamp_us,
            data,
            dma_buf_fd: None,
            drm_modifier: 0,
        };
        let seq = self.publish_written_slot(slot_idx, new_frame, stole_ready);
        self.record_store_latency(t0.elapsed());
        Ok(seq)
    }

    /// Конвертирует буфер кадра PipeWire (`SpaBuffer`) напрямую в свободный атомарный слот
    /// кольцевого буфера (поддерживает zero-copy `DMA-BUF` и CPU-конвертацию с бюджетом `< 50 МБ`).
    pub fn store_portal_frame(
        &self,
        frame: &PipeWireFrameBuffer<'_>,
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<u64, CaptureError> {
        let t0 = Instant::now();
        if frame.is_dma_buf() {
            let new_frame = match frame.to_video_frame(output_format, timestamp_us) {
                Ok(f) => f,
                Err(err) => {
                    self.record_rejected();
                    return Err(err);
                }
            };
            let Some((slot_idx, stole_ready)) = self.acquire_writable_slot() else {
                self.record_rejected();
                return Err(CaptureError::DbusError(
                    "Нет свободных атомарных слотов в LatestFrameRingBuffer".to_string(),
                ));
            };
            let seq = self.publish_written_slot(slot_idx, new_frame, stole_ready);
            self.record_store_latency(t0.elapsed());
            return Ok(seq);
        }

        let (out_w, out_h) = frame
            .crop
            .map_or((frame.width, frame.height), |c| (c.width, c.height));
        if out_w == 0 || out_h == 0 {
            self.record_rejected();
            return Err(CaptureError::InvalidDimensions {
                width: out_w,
                height: out_h,
            });
        }
        let Some(required_len) = output_format.byte_len(out_w, out_h) else {
            self.record_rejected();
            return Err(CaptureError::InvalidDimensions {
                width: out_w,
                height: out_h,
            });
        };
        if required_len > MAX_SINGLE_FRAME_BYTES || required_len >= self.inner.max_memory_bytes {
            self.record_memory_limit_rejection();
            return Err(CaptureError::InvalidDimensions {
                width: out_w,
                height: out_h,
            });
        }

        let Some((slot_idx, stole_ready)) = self.acquire_writable_slot() else {
            self.record_rejected();
            return Err(CaptureError::DbusError(
                "Нет свободных атомарных слотов в LatestFrameRingBuffer".to_string(),
            ));
        };

        let slot = &self.inner.slots[slot_idx];
        let mut data = slot
            .frame
            .lock()
            .ok()
            .and_then(|mut g| g.take().map(|old| old.data))
            .unwrap_or_default();

        if !self.ensure_memory_budget_for_slot(slot_idx, &mut data, required_len) {
            slot.capacity_bytes
                .store(data.capacity(), Ordering::Release);
            self.abort_writable_slot(slot_idx, data);
            self.record_memory_limit_rejection();
            return Err(CaptureError::InvalidDimensions {
                width: out_w,
                height: out_h,
            });
        }

        if let Err(err) = frame.write_into_vec(output_format, &mut data) {
            data.clear();
            slot.capacity_bytes
                .store(data.capacity(), Ordering::Release);
            self.abort_writable_slot(slot_idx, data);
            self.record_rejected();
            return Err(err);
        }

        let new_frame =
            VideoFrame::new(out_w, out_h, output_format, data).with_timestamp_us(timestamp_us);
        let seq = self.publish_written_slot(slot_idx, new_frame, stole_ready);
        self.record_store_latency(t0.elapsed());
        Ok(seq)
    }

    /// Захватывает окно X11 из сегмента разделяемой памяти MIT-SHM напрямую в атомарный слот кольцевого буфера.
    pub fn store_x11_window(
        &self,
        shm: &X11ShmPixelBuffer,
        shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<u64, CaptureError> {
        let pw_format = if shm.depth() == 32 {
            PipeWirePixelFormat::Bgra8
        } else {
            PipeWirePixelFormat::Bgrx8
        };
        let buf = PipeWireFrameBuffer {
            width: shm.width(),
            height: shm.height(),
            stride: shm.bytes_per_line(),
            format: pw_format,
            data: shm_data,
            crop: None,
            dma_buf_fd: None,
            drm_modifier: 0,
        };
        self.store_portal_frame(&buf, output_format, timestamp_us)
    }

    /// Вырезает прямоугольник монитора X11 из буфера root-окна MIT-SHM и записывает напрямую в кольцевой буфер.
    pub fn store_x11_monitor(
        &self,
        shm: &X11ShmPixelBuffer,
        monitor: &X11MonitorGeometry,
        root_shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<u64, CaptureError> {
        if let Err(err) = monitor.validate() {
            self.record_rejected();
            return Err(err);
        }
        let Ok(crop_x) = u32::try_from(monitor.x) else {
            self.record_rejected();
            return Err(CaptureError::CropOutOfBounds {
                crop_x: 0,
                crop_y: 0,
                crop_width: monitor.width,
                crop_height: monitor.height,
                frame_width: shm.width(),
                frame_height: shm.height(),
            });
        };
        let Ok(crop_y) = u32::try_from(monitor.y) else {
            self.record_rejected();
            return Err(CaptureError::CropOutOfBounds {
                crop_x,
                crop_y: 0,
                crop_width: monitor.width,
                crop_height: monitor.height,
                frame_width: shm.width(),
                frame_height: shm.height(),
            });
        };
        let pw_format = if shm.depth() == 32 {
            PipeWirePixelFormat::Bgra8
        } else {
            PipeWirePixelFormat::Bgrx8
        };
        let buf = PipeWireFrameBuffer {
            width: shm.width(),
            height: shm.height(),
            stride: shm.bytes_per_line(),
            format: pw_format,
            data: root_shm_data,
            crop: Some(CropRect {
                x: crop_x,
                y: crop_y,
                width: monitor.width,
                height: monitor.height,
            }),
            dma_buf_fd: None,
            drm_modifier: 0,
        };
        self.store_portal_frame(&buf, output_format, timestamp_us)
    }

    /// Захватывает последний опубликованный слот в состояние `Locked` для чтения без разрывов.
    fn lock_latest_slot_for_read(
        &self,
        allow_idle_retained: bool,
        consume_fresh: bool,
    ) -> Option<(LockedSlotReadGuard<'_>, u64, bool)> {
        let n = self.inner.slots.len();
        for _ in 0..n {
            let idx = self.inner.latest_slot_index.load(Ordering::Acquire);
            if idx >= n {
                return None;
            }
            if !allow_idle_retained && !self.inner.fresh.load(Ordering::Acquire) {
                return None;
            }
            let slot = &self.inner.slots[idx];
            let Some(prev_state) = slot.try_acquire_read(allow_idle_retained) else {
                continue;
            };
            let guard = LockedSlotReadGuard {
                ring: &self.inner,
                slot_idx: idx,
                restore_ready_if_latest: !consume_fresh && prev_state == FrameSlotState::Ready,
            };
            // Убеждаемся, что пока мы делали CAS, указатель latest_slot_index не сместился на более новый слот
            if self.inner.latest_slot_index.load(Ordering::Acquire) != idx {
                drop(guard);
                continue;
            }
            let was_fresh = if consume_fresh {
                self.inner.fresh.swap(false, Ordering::AcqRel)
                    || prev_state == FrameSlotState::Ready
            } else {
                self.inner.fresh.load(Ordering::Acquire) || prev_state == FrameSlotState::Ready
            };
            if consume_fresh && !was_fresh && !allow_idle_retained {
                return None;
            }
            let seq = slot.sequence.load(Ordering::Acquire);
            if seq == 0 {
                return None;
            }
            return Some((guard, seq, was_fresh));
        }
        None
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра по ссылке `&VideoFrame`.
    ///
    /// Во время выполнения замыкания `f` слот удерживается в атомарном состоянии [`FrameSlotState::Locked`],
    /// поэтому поток захвата при записи новых кадров выбирает соседние слоты без блокировки и без разрывов.
    pub fn with_latest_fresh<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame) -> R,
    {
        let (guard, _seq, _was_fresh) = self.lock_latest_slot_for_read(false, true)?;
        let slot = &self.inner.slots[guard.slot_idx];
        let frame_guard = slot.frame.try_lock().ok()?;
        let frame = frame_guard.as_ref()?;
        self.inner.polled_frames.fetch_add(1, Ordering::Relaxed);
        Some(f(frame))
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра с проверкой бюджета 60 FPS (`16 мс`).
    pub fn with_latest_fresh_60fps<R, F>(&self, elapsed: Duration, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame) -> R,
    {
        if elapsed < FRAME_BUDGET_60FPS {
            return None;
        }
        self.with_latest_fresh(f)
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра через [`BorrowedFrameView`].
    ///
    /// Слот удерживается в состоянии [`FrameSlotState::Locked`] на время вызова `f`,
    /// полностью исключая разрывы кадра (frame tearing) при 60 FPS записи.
    pub fn with_ipc_frame<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        let (guard, seq, _was_fresh) = self.lock_latest_slot_for_read(false, true)?;
        let slot = &self.inner.slots[guard.slot_idx];
        let frame_guard = slot.frame.try_lock().ok()?;
        let frame = frame_guard.as_ref()?;
        self.inner.polled_frames.fetch_add(1, Ordering::Relaxed);
        Some(f(BorrowedFrameView::from_frame(frame, seq, true)))
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра через [`BorrowedFrameView`]
    /// с проверкой бюджета 60 FPS (`16 мс`).
    pub fn with_ipc_frame_60fps<R, F>(&self, elapsed: Duration, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        if elapsed < FRAME_BUDGET_60FPS {
            return None;
        }
        self.with_ipc_frame(f)
    }

    /// Неблокирующий zero-copy осмотр текущего кадра в кольцевом буфере через [`BorrowedFrameView`]
    /// без сброса флага свежести `fresh`.
    pub fn peek_ipc_frame<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        let (guard, seq, is_fresh) = self.lock_latest_slot_for_read(true, false)?;
        let slot = &self.inner.slots[guard.slot_idx];
        let frame_guard = slot.frame.try_lock().ok()?;
        let frame = frame_guard.as_ref()?;
        Some(f(BorrowedFrameView::from_frame(frame, seq, is_fresh)))
    }

    /// Неблокирующий zero-copy осмотр ссылки `&VideoFrame` без сброса флага свежести `fresh`.
    pub fn peek_latest<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame) -> R,
    {
        let (guard, _seq, _is_fresh) = self.lock_latest_slot_for_read(true, false)?;
        let slot = &self.inner.slots[guard.slot_idx];
        let frame_guard = slot.frame.try_lock().ok()?;
        let frame = frame_guard.as_ref()?;
        Some(f(frame))
    }

    /// Zero-copy чтение последнего кадра для видео-энкодера (`retain_screen_frame`).
    ///
    /// Вызывает `f` без копирования буфера, если:
    /// - в буфере появился новый кадр (`fresh == true`), ИЛИ
    /// - запрошен ключевой кадр `force_keyframe == true` (RTCP PLI/FIR на статичном экране —
    ///   энкодер повторно читает удержанный в последнем слоте кадр с `view.is_fresh == false`).
    pub fn read_for_encoder<R, F>(&self, force_keyframe: bool, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        let (guard, seq, was_fresh) = self.lock_latest_slot_for_read(force_keyframe, true)?;
        let slot = &self.inner.slots[guard.slot_idx];
        let frame_guard = slot.frame.try_lock().ok()?;
        let frame = frame_guard.as_ref()?;
        self.inner.polled_frames.fetch_add(1, Ordering::Relaxed);
        Some(f(BorrowedFrameView::from_frame(frame, seq, was_fresh)))
    }

    /// Неблокирующее извлечение владения `VideoFrame` из последнего опубликованного слота.
    pub fn take_latest(&self) -> Option<VideoFrame> {
        let (mut guard, _seq, _was_fresh) = self.lock_latest_slot_for_read(true, true)?;
        guard.restore_ready_if_latest = false;
        let slot = &self.inner.slots[guard.slot_idx];
        let mut frame_guard = slot.frame.try_lock().ok()?;
        let frame = frame_guard.take()?;
        slot.sequence.store(0, Ordering::Release);
        slot.timestamp_us.store(0, Ordering::Release);
        slot.capacity_bytes.store(0, Ordering::Release);
        drop(frame_guard);
        self.inner
            .latest_slot_index
            .store(NO_LATEST_SLOT, Ordering::Release);
        self.sync_allocated_bytes();
        self.inner.polled_frames.fetch_add(1, Ordering::Relaxed);
        Some(frame)
    }

    /// Неблокирующий опрос последнего свежего кадра с клонированием.
    pub fn poll_latest(&self) -> Option<VideoFrame> {
        self.with_latest_fresh(Clone::clone)
    }

    /// Неблокирующий опрос последнего свежего кадра с проверкой бюджета 60 FPS (`16 мс`).
    pub fn poll_latest_60fps(&self, elapsed: Duration) -> Option<VideoFrame> {
        self.with_latest_fresh_60fps(elapsed, Clone::clone)
    }

    /// Очищает все незаблокированные слоты кольцевого буфера и освобождает выделенную память.
    pub fn clear(&self) {
        self.inner.fresh.store(false, Ordering::Release);
        self.inner
            .latest_slot_index
            .store(NO_LATEST_SLOT, Ordering::Release);
        for slot in &self.inner.slots {
            if slot.try_acquire_write() || slot.try_steal_ready_for_write() {
                if let Ok(mut guard) = slot.frame.lock() {
                    *guard = None;
                }
                slot.sequence.store(0, Ordering::Release);
                slot.timestamp_us.store(0, Ordering::Release);
                slot.capacity_bytes.store(0, Ordering::Release);
                slot.state
                    .store(FrameSlotState::Idle as u8, Ordering::Release);
            }
        }
        self.sync_allocated_bytes();
    }

    /// Проверяет наличие опубликованного валидного кадра в кольцевом буфере.
    #[must_use]
    pub fn has_frame(&self) -> bool {
        let idx = self.inner.latest_slot_index.load(Ordering::Acquire);
        idx < self.inner.slots.len() && self.inner.slots[idx].sequence() > 0
    }

    /// Проверяет, ожидает ли в кольцевом буфере непрочитанный свежий кадр.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.inner.fresh.load(Ordering::Acquire)
    }

    /// Возвращает монотонный порядковый номер последнего записанного кадра (`0`, если кадров не было).
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.inner.sequence.load(Ordering::Acquire)
    }

    /// Количество успешно опубликованных кадров.
    #[must_use]
    pub fn stored_frames(&self) -> u64 {
        self.inner.stored_frames.load(Ordering::Acquire)
    }

    /// Количество устаревших кадров, перезаписанных до чтения потребителем.
    #[must_use]
    pub fn overwritten_frames(&self) -> u64 {
        self.inner.overwritten_frames.load(Ordering::Acquire)
    }

    /// Общее количество отклонённых кадров.
    #[must_use]
    pub fn rejected_frames(&self) -> u64 {
        self.inner.rejected_frames.load(Ordering::Acquire)
    }

    /// Количество кадров, отклонённых из-за превышения лимита памяти `< 50 МБ`.
    #[must_use]
    pub fn memory_limit_rejections(&self) -> u64 {
        self.inner.memory_limit_rejections.load(Ordering::Acquire)
    }

    /// Количество успешных чтений кадра потребителем.
    #[must_use]
    pub fn polled_frames(&self) -> u64 {
        self.inner.polled_frames.load(Ordering::Acquire)
    }

    /// Длительность последней операции записи в кольцевой буфер.
    #[must_use]
    pub fn last_store_duration(&self) -> Duration {
        Duration::from_nanos(self.inner.last_store_duration_ns.load(Ordering::Acquire))
    }

    /// Максимальная зафиксированная длительность записи в кольцевой буфер.
    #[must_use]
    pub fn max_store_duration(&self) -> Duration {
        Duration::from_nanos(self.inner.max_store_duration_ns.load(Ordering::Acquire))
    }

    /// Возвращает сводный снимок телеметрии [`RingBufferMetrics`].
    #[must_use]
    pub fn metrics(&self) -> RingBufferMetrics {
        let slot_states = self
            .inner
            .slots
            .iter()
            .map(AtomicFrameSlot::state)
            .collect();
        let slot_capacities = self
            .inner
            .slots
            .iter()
            .map(AtomicFrameSlot::capacity_bytes)
            .collect();
        RingBufferMetrics {
            slot_count: self.slot_count(),
            max_memory_bytes: self.max_memory_bytes(),
            allocated_memory_bytes: self.allocated_memory_bytes(),
            peak_memory_bytes: self.peak_memory_bytes(),
            stored_frames: self.stored_frames(),
            overwritten_frames: self.overwritten_frames(),
            rejected_frames: self.rejected_frames(),
            memory_limit_rejections: self.memory_limit_rejections(),
            polled_frames: self.polled_frames(),
            sequence: self.sequence(),
            latest_slot_index: self.latest_slot_index(),
            fresh: self.is_fresh(),
            last_store_duration_ns: self.inner.last_store_duration_ns.load(Ordering::Acquire),
            max_store_duration_ns: self.inner.max_store_duration_ns.load(Ordering::Acquire),
            slot_states,
            slot_capacities,
        }
    }
}

/// Снимок телеметрии и состояния [`LatestFrameSlot`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatestFrameMetrics {
    /// Общее число успешно записанных кадров.
    pub stored_frames: u64,
    /// Число кадров, перезаписанных до того, как их прочитал потребитель (авто-замена устаревших).
    pub overwritten_frames: u64,
    /// Число отклонённых невалидных кадров.
    pub rejected_frames: u64,
    /// Число успешных чтений свежего кадра потребителем.
    pub polled_frames: u64,
    /// Текущий монотонный номер кадра в слоте.
    pub sequence: u64,
    /// Есть ли в слоте непрочитанный свежий кадр.
    pub fresh: bool,
    /// Текущая выделенная ёмкость активного буфера в слоте (в байтах).
    pub slot_capacity_bytes: usize,
    /// Текущая выделенная ёмкость резервного буфера `scratch` (в байтах).
    pub scratch_capacity_bytes: usize,
    /// Длительность последней операции записи в наносекундах.
    pub last_store_duration_ns: u64,
    /// Максимальная зафиксированная длительность записи в наносекундах.
    pub max_store_duration_ns: u64,
}

/// Атомарный 60 FPS слот последнего видеокадра с двойной буферизацией (`slot` + `scratch`),
/// неблокирующим чтением для UI / Tauri IPC / кодировщика и переиспользованием памяти без накопления.
#[derive(Clone, Default, Debug)]
pub struct LatestFrameSlot {
    slot: Arc<Mutex<Option<VideoFrame>>>,
    scratch: Arc<Mutex<Vec<u8>>>,
    fresh: Arc<AtomicBool>,
    stored_frames: Arc<AtomicU64>,
    overwritten_frames: Arc<AtomicU64>,
    rejected_frames: Arc<AtomicU64>,
    polled_frames: Arc<AtomicU64>,
    sequence: Arc<AtomicU64>,
    last_store_duration_ns: Arc<AtomicU64>,
    max_store_duration_ns: Arc<AtomicU64>,
}

impl LatestFrameSlot {
    /// Создаёт новый пустой слот последнего кадра.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Возвращает ссылку на внутренний слот `Arc<Mutex<Option<VideoFrame>>>`.
    #[must_use]
    pub fn inner(&self) -> &Arc<Mutex<Option<VideoFrame>>> {
        &self.slot
    }

    /// Фиксирует отклонённый кадр в счётчике `rejected_frames`.
    pub(crate) fn record_rejected_frame(&self) {
        self.rejected_frames.fetch_add(1, Ordering::Relaxed);
    }

    fn record_store_latency(&self, elapsed: Duration) {
        let ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.last_store_duration_ns.store(ns, Ordering::Release);
        self.max_store_duration_ns.fetch_max(ns, Ordering::AcqRel);
    }

    fn commit_frame_locked(
        &self,
        guard: &mut Option<VideoFrame>,
        new_frame: VideoFrame,
    ) -> (u64, Option<Vec<u8>>) {
        let old_data = guard.take().map(|old| old.data);
        *guard = Some(new_frame);
        if self.fresh.swap(true, Ordering::AcqRel) {
            self.overwritten_frames.fetch_add(1, Ordering::Relaxed);
        }
        self.stored_frames.fetch_add(1, Ordering::Relaxed);
        let seq = self.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        (seq, old_data)
    }

    fn recycle_into_scratch(&self, mut old_data: Vec<u8>, current_len: usize) {
        if old_data.capacity() > LARGE_BUFFER_SHRINK_THRESHOLD
            && old_data.capacity() > current_len.saturating_mul(SHRINK_RATIO_MULTIPLIER)
        {
            old_data = Vec::new();
        } else {
            old_data.clear();
        }
        if let Ok(mut scratch_guard) = self.scratch.lock()
            && scratch_guard.capacity() < old_data.capacity()
        {
            *scratch_guard = old_data;
        }
    }

    /// Заменяет последний кадр на месте с переиспользованием буфера памяти в `slot`
    /// (`Serein/apps/desktop/src/watch.rs::store_frame`).
    ///
    /// При невалидном кадре или превышении лимита памяти `< 50 МБ` увеличивает `rejected_frames`,
    /// сохраняет предыдущий валидный кадр и возвращает `false`.
    pub fn store_frame(&self, frame: VideoFrame) -> bool {
        let t0 = Instant::now();
        if !frame.is_valid()
            || (!frame.is_dma_buf()
                && (frame.data.len() > MAX_SINGLE_FRAME_BYTES
                    || frame.data.len() >= MAX_RING_BUFFER_MEMORY_BYTES))
        {
            self.record_rejected_frame();
            return false;
        }
        let Ok(mut guard) = self.slot.lock() else {
            self.record_rejected_frame();
            return false;
        };
        let len = frame.data.len();
        let mut data = guard.take().map(|old| old.data).unwrap_or_default();
        prepare_reused_vec(&mut data, len);
        data.extend_from_slice(&frame.data);
        let (_, _) = self.commit_frame_locked(&mut guard, VideoFrame { data, ..frame });
        drop(guard);

        // При падении разрешения сбрасываем также резервный scratch-буфер, если он удерживал старый размер.
        if let Ok(mut scratch_guard) = self.scratch.lock()
            && scratch_guard.capacity() > LARGE_BUFFER_SHRINK_THRESHOLD
            && scratch_guard.capacity() > len.saturating_mul(SHRINK_RATIO_MULTIPLIER)
        {
            *scratch_guard = Vec::new();
        }

        self.record_store_latency(t0.elapsed());
        true
    }

    /// Заполняет переиспользуемый резервный буфер `scratch` вне блокировки `slot` и атомарно
    /// публикует готовый кадр за микросекунды без блокировки UI-потока.
    #[allow(clippy::too_many_arguments)]
    pub fn store_with_reused_buffer<F, E>(
        &self,
        width: u32,
        height: u32,
        layout: PixelLayout,
        color: ColorSpace,
        rotation: u16,
        timestamp_us: u64,
        fill: F,
    ) -> Result<u64, E>
    where
        F: FnOnce(&mut [u8]) -> Result<(), E>,
        E: From<&'static str>,
    {
        let t0 = Instant::now();
        if width == 0 || height == 0 || !matches!(rotation, 0 | 90 | 180 | 270) {
            self.record_rejected_frame();
            return Err(E::from(
                "Некорректные размеры или угол поворота кадра для LatestFrameSlot",
            ));
        }
        let Some(required_len) = layout.byte_len(width, height) else {
            self.record_rejected_frame();
            return Err(E::from("Переполнение размера буфера кадра"));
        };
        if required_len > MAX_SINGLE_FRAME_BYTES || required_len >= MAX_RING_BUFFER_MEMORY_BYTES {
            self.record_rejected_frame();
            return Err(E::from("Превышен лимит памяти кольцевого буфера (< 50 МБ)"));
        }

        let mut data = self
            .scratch
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();
        prepare_reused_vec(&mut data, required_len);
        data.resize(required_len, 0);

        if let Err(err) = fill(&mut data) {
            self.recycle_into_scratch(data, required_len);
            self.record_rejected_frame();
            return Err(err);
        }

        let new_frame = VideoFrame {
            width,
            height,
            layout,
            color,
            rotation,
            timestamp_us,
            data,
            dma_buf_fd: None,
            drm_modifier: 0,
        };

        let swap_start = Instant::now();
        let Ok(mut guard) = self.slot.lock() else {
            self.record_rejected_frame();
            return Err(E::from("Слот последнего кадра отравлен (mutex poisoned)"));
        };
        let (seq, old_data) = self.commit_frame_locked(&mut guard, new_frame);
        drop(guard);
        let swap_elapsed = swap_start.elapsed();

        if let Some(old) = old_data {
            self.recycle_into_scratch(old, required_len);
        }
        let _ = t0;
        self.record_store_latency(swap_elapsed);
        Ok(seq)
    }

    /// Конвертирует буфер кадра PipeWire (`SpaBuffer`) во внеблочный `scratch`-буфер и
    /// атомарно публикует его в слот без промежуточных аллокаций.
    pub fn store_portal_frame(
        &self,
        frame: &PipeWireFrameBuffer<'_>,
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<u64, CaptureError> {
        // Zero-copy оптимизация для PipeWire DMA-BUF кадра (TASK-804):
        // исключает CPU копирование и преобразование 8-15 МБ на кадр, устраняя 86 мс задержки.
        if frame.is_dma_buf() {
            let new_frame = match frame.to_video_frame(output_format, timestamp_us) {
                Ok(f) => f,
                Err(err) => {
                    self.record_rejected_frame();
                    return Err(err);
                }
            };
            let swap_start = Instant::now();
            let Ok(mut guard) = self.slot.lock() else {
                self.record_rejected_frame();
                return Err(CaptureError::DbusError(
                    "Слот последнего кадра недоступен (mutex poisoned)".to_string(),
                ));
            };
            let (seq, old_data) = self.commit_frame_locked(&mut guard, new_frame);
            drop(guard);
            self.record_store_latency(swap_start.elapsed());
            if let Some(old) = old_data {
                self.recycle_into_scratch(old, 0);
            }
            return Ok(seq);
        }

        let mut data = self
            .scratch
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();

        let (out_w, out_h) = match frame.write_into_vec(output_format, &mut data) {
            Ok(dims) => dims,
            Err(err) => {
                if let Ok(mut scratch_guard) = self.scratch.lock()
                    && scratch_guard.capacity() < data.capacity()
                {
                    *scratch_guard = data;
                }
                self.record_rejected_frame();
                return Err(err);
            }
        };

        let len = data.len();
        if len > MAX_SINGLE_FRAME_BYTES || len >= MAX_RING_BUFFER_MEMORY_BYTES {
            self.record_rejected_frame();
            return Err(CaptureError::InvalidDimensions {
                width: out_w,
                height: out_h,
            });
        }
        let new_frame =
            VideoFrame::new(out_w, out_h, output_format, data).with_timestamp_us(timestamp_us);

        let swap_start = Instant::now();
        let Ok(mut guard) = self.slot.lock() else {
            self.record_rejected_frame();
            return Err(CaptureError::DbusError(
                "Слот последнего кадра недоступен (mutex poisoned)".to_string(),
            ));
        };
        let (seq, old_data) = self.commit_frame_locked(&mut guard, new_frame);
        drop(guard);
        self.record_store_latency(swap_start.elapsed());

        if let Some(old) = old_data {
            self.recycle_into_scratch(old, len);
        }
        Ok(seq)
    }

    /// Захватывает окно X11 из сегмента разделяемой памяти MIT-SHM напрямую в слот.
    pub fn store_x11_window(
        &self,
        shm: &X11ShmPixelBuffer,
        shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<u64, CaptureError> {
        let pw_format = if shm.depth() == 32 {
            PipeWirePixelFormat::Bgra8
        } else {
            PipeWirePixelFormat::Bgrx8
        };
        let buf = PipeWireFrameBuffer {
            width: shm.width(),
            height: shm.height(),
            stride: shm.bytes_per_line(),
            format: pw_format,
            data: shm_data,
            crop: None,
            dma_buf_fd: None,
            drm_modifier: 0,
        };
        self.store_portal_frame(&buf, output_format, timestamp_us)
    }

    /// Вырезает прямоугольник монитора X11 из буфера root-окна MIT-SHM и записывает напрямую в слот.
    pub fn store_x11_monitor(
        &self,
        shm: &X11ShmPixelBuffer,
        monitor: &X11MonitorGeometry,
        root_shm_data: &[u8],
        output_format: PixelLayout,
        timestamp_us: u64,
    ) -> Result<u64, CaptureError> {
        if let Err(err) = monitor.validate() {
            self.record_rejected_frame();
            return Err(err);
        }
        let Ok(crop_x) = u32::try_from(monitor.x) else {
            self.record_rejected_frame();
            return Err(CaptureError::CropOutOfBounds {
                crop_x: 0,
                crop_y: 0,
                crop_width: monitor.width,
                crop_height: monitor.height,
                frame_width: shm.width(),
                frame_height: shm.height(),
            });
        };
        let Ok(crop_y) = u32::try_from(monitor.y) else {
            self.record_rejected_frame();
            return Err(CaptureError::CropOutOfBounds {
                crop_x,
                crop_y: 0,
                crop_width: monitor.width,
                crop_height: monitor.height,
                frame_width: shm.width(),
                frame_height: shm.height(),
            });
        };
        let pw_format = if shm.depth() == 32 {
            PipeWirePixelFormat::Bgra8
        } else {
            PipeWirePixelFormat::Bgrx8
        };
        let buf = PipeWireFrameBuffer {
            width: shm.width(),
            height: shm.height(),
            stride: shm.bytes_per_line(),
            format: pw_format,
            data: root_shm_data,
            crop: Some(CropRect {
                x: crop_x,
                y: crop_y,
                width: monitor.width,
                height: monitor.height,
            }),
            dma_buf_fd: None,
            drm_modifier: 0,
        };
        self.store_portal_frame(&buf, output_format, timestamp_us)
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра по ссылке `&VideoFrame`.
    ///
    /// Возвращает `Some(R)` только один раз на каждый новый кадр, оставляя буфер памяти внутри слота
    /// для переиспользования потоком захвата.
    pub fn with_latest_fresh<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame) -> R,
    {
        let guard = self.slot.try_lock().ok()?;
        let frame = guard.as_ref()?;
        if !self.fresh.swap(false, Ordering::AcqRel) {
            return None;
        }
        self.polled_frames.fetch_add(1, Ordering::Relaxed);
        Some(f(frame))
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра с проверкой бюджета 60 FPS (`16 мс`).
    pub fn with_latest_fresh_60fps<R, F>(&self, elapsed: Duration, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame) -> R,
    {
        if elapsed < FRAME_BUDGET_60FPS {
            return None;
        }
        self.with_latest_fresh(f)
    }

    /// Неблокирующее zero-copy чтение последнего свежего кадра для Tauri IPC и GPU-шейдера
    /// через [`BorrowedFrameView`] без копирования пиксельных плоскостей.
    pub fn with_ipc_frame<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        let guard = self.slot.try_lock().ok()?;
        let frame = guard.as_ref()?;
        if !self.fresh.swap(false, Ordering::AcqRel) {
            return None;
        }
        self.polled_frames.fetch_add(1, Ordering::Relaxed);
        let seq = self.sequence.load(Ordering::Acquire);
        Some(f(BorrowedFrameView::from_frame(frame, seq, true)))
    }

    /// Неблокирующий zero-copy осмотр текущего кадра в слоте через [`BorrowedFrameView`]
    /// без сброса флага свежести `fresh`.
    pub fn peek_ipc_frame<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        let guard = self.slot.try_lock().ok()?;
        let frame = guard.as_ref()?;
        let seq = self.sequence.load(Ordering::Acquire);
        let is_fresh = self.fresh.load(Ordering::Acquire);
        Some(f(BorrowedFrameView::from_frame(frame, seq, is_fresh)))
    }

    /// Неблокирующий zero-copy осмотр ссылки `&VideoFrame` в слоте без сброса флага свежести `fresh`.
    pub fn peek_latest<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&VideoFrame) -> R,
    {
        let guard = self.slot.try_lock().ok()?;
        let frame = guard.as_ref()?;
        Some(f(frame))
    }

    /// Zero-copy чтение последнего кадра для видео-энкодера (`Serein/crates/discord-voice/src/screen.rs::retain_screen_frame`).
    ///
    /// Вызывает `f` без копирования буфера, если:
    /// - в слоте появился новый кадр (`fresh == true`, флаг атомарно сбрасывается), ИЛИ
    /// - запрошен ключевой кадр `force_keyframe == true` (RTCP PLI/FIR на статичном экране —
    ///   энкодер повторно кодирует удержанный в слоте кадр с `view.is_fresh == false`).
    pub fn read_for_encoder<R, F>(&self, force_keyframe: bool, f: F) -> Option<R>
    where
        F: FnOnce(BorrowedFrameView<'_>) -> R,
    {
        let guard = self.slot.try_lock().ok()?;
        let frame = guard.as_ref()?;
        let was_fresh = self.fresh.swap(false, Ordering::AcqRel);
        if !was_fresh && !force_keyframe {
            return None;
        }
        self.polled_frames.fetch_add(1, Ordering::Relaxed);
        let seq = self.sequence.load(Ordering::Acquire);
        Some(f(BorrowedFrameView::from_frame(frame, seq, was_fresh)))
    }

    /// Неблокирующее извлечение владения `VideoFrame` из слота (`guard.take()`) без клонирования `Vec<u8>`.
    pub fn take_latest(&self) -> Option<VideoFrame> {
        let mut guard = self.slot.try_lock().ok()?;
        let frame = guard.take()?;
        self.fresh.store(false, Ordering::Release);
        self.polled_frames.fetch_add(1, Ordering::Relaxed);
        Some(frame)
    }

    /// Неблокирующий опрос последнего свежего кадра с клонированием (обратная совместимость).
    pub fn poll_latest(&self) -> Option<VideoFrame> {
        self.with_latest_fresh(Clone::clone)
    }

    /// Опрос последнего кадра с проверкой бюджета 60 FPS / 16 мс (`FRAME_BUDGET_60FPS`).
    pub fn poll_latest_60fps(&self, elapsed: Duration) -> Option<VideoFrame> {
        self.with_latest_fresh_60fps(elapsed, Clone::clone)
    }

    /// Полностью очищает активный слот и резервный буфер `scratch`, освобождая память.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.slot.lock() {
            *guard = None;
        }
        if let Ok(mut scratch_guard) = self.scratch.lock() {
            *scratch_guard = Vec::new();
        }
        self.fresh.store(false, Ordering::Release);
    }

    /// Проверяет наличие валидного кадра в слоте.
    #[must_use]
    pub fn has_frame(&self) -> bool {
        self.slot
            .try_lock()
            .ok()
            .is_some_and(|guard| guard.is_some())
    }

    /// Проверяет, ожидает ли в слоте непрочитанный свежий кадр.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.fresh.load(Ordering::Acquire)
    }

    /// Возвращает монотонный порядковый номер последнего записанного кадра (`0`, если кадров не было).
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }

    /// Количество успешно записанных в слот кадров.
    #[must_use]
    pub fn stored_frames(&self) -> u64 {
        self.stored_frames.load(Ordering::Acquire)
    }

    /// Количество устаревших кадров, перезаписанных до чтения потребителем.
    #[must_use]
    pub fn overwritten_frames(&self) -> u64 {
        self.overwritten_frames.load(Ordering::Acquire)
    }

    /// Количество отклонённых невалидных кадров.
    #[must_use]
    pub fn rejected_frames(&self) -> u64 {
        self.rejected_frames.load(Ordering::Acquire)
    }

    /// Количество успешных чтений кадра потребителем.
    #[must_use]
    pub fn polled_frames(&self) -> u64 {
        self.polled_frames.load(Ordering::Acquire)
    }

    /// Текущая выделенная ёмкость буфера в активном слоте (`slot`, в байтах).
    #[must_use]
    pub fn slot_capacity_bytes(&self) -> usize {
        self.slot
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|f| f.data.capacity()))
            .unwrap_or(0)
    }

    /// Суммарная выделенная ёмкость памяти в кольцевом слоте (`slot` + `scratch`, в байтах).
    #[must_use]
    pub fn allocated_capacity_bytes(&self) -> usize {
        let slot_cap = self.slot_capacity_bytes();
        let scratch_cap = self.scratch.lock().ok().map(|g| g.capacity()).unwrap_or(0);
        slot_cap.saturating_add(scratch_cap)
    }

    /// Длительность последней операции записи в слот.
    #[must_use]
    pub fn last_store_duration(&self) -> Duration {
        Duration::from_nanos(self.last_store_duration_ns.load(Ordering::Acquire))
    }

    /// Максимальная зафиксированная длительность записи в слот.
    #[must_use]
    pub fn max_store_duration(&self) -> Duration {
        Duration::from_nanos(self.max_store_duration_ns.load(Ordering::Acquire))
    }

    /// Возвращает сводный снимок метрик [`LatestFrameMetrics`].
    #[must_use]
    pub fn metrics(&self) -> LatestFrameMetrics {
        let slot_capacity_bytes = self.slot_capacity_bytes();
        let scratch_capacity_bytes = self.scratch.lock().ok().map(|g| g.capacity()).unwrap_or(0);
        LatestFrameMetrics {
            stored_frames: self.stored_frames(),
            overwritten_frames: self.overwritten_frames(),
            rejected_frames: self.rejected_frames(),
            polled_frames: self.polled_frames(),
            sequence: self.sequence(),
            fresh: self.is_fresh(),
            slot_capacity_bytes,
            scratch_capacity_bytes,
            last_store_duration_ns: self.last_store_duration_ns.load(Ordering::Acquire),
            max_store_duration_ns: self.max_store_duration_ns.load(Ordering::Acquire),
        }
    }
}
