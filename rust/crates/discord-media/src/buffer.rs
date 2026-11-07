//! Атомарный 60 FPS кольцевой буфер последнего видеокадра (`LatestFrameSlot`).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/voice-deep.md` и `docs/re/voice-deep.md` (§3: видеопоток 1280x720@30/60);
//! - `/home/sanya/re-deep/native-re.md` (`VoiceEngine.getNextVideoOutputFrame`, `WumpusVideoSource`,
//!   `FrameBuffer/Pool`, `BaseCapturerPipeWire`, `ScreenCapturerX11`, `WindowCapturerX11`, `XServerPixelBuffer`);
//! - `Serein/apps/desktop/src/watch.rs` (`store_frame` L21-38, `Watch::poll` L297-304);
//! - `Serein/crates/discord-voice/src/screen.rs` (`retain_screen_frame` L442-454).

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
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

/// Заимствованное zero-copy представление видеокадра внутри [`LatestFrameSlot`]
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
        }
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
    /// При невалидном кадре увеличивает `rejected_frames`, сохраняет предыдущий валидный кадр
    /// и возвращает `false`.
    pub fn store_frame(&self, frame: VideoFrame) -> bool {
        let t0 = Instant::now();
        if !frame.is_valid() {
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
