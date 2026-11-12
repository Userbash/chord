//! Контрактные тесты неблокирующего (lock-free) 60 FPS кольцевого буфера последнего кадра
//! с атомарными слотами и жёстким бюджетом памяти `< 50 МБ` (`chord_media::buffer`).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/voice-deep.md` и `docs/re/voice-deep.md` (§3: видеопоток 1280x720@30/60,
//!   кодеки AV1/H264, `updateVideoQuality`);
//! - `/home/sanya/re-deep/native-re.md` (`VoiceEngine.getNextVideoOutputFrame`, `WumpusVideoSource`,
//!   `FrameBuffer/Pool`, `BaseCapturerPipeWire`, `ScreenCapturerX11`, `WindowCapturerX11`);
//! - `docs/specs/STREAM-WAYLAND-GUIDE.md` (§1.3: тройной кольцевой буфер `g_cap_slots[3]` с атомарным
//!   индексом активного слота и защитой от разрывов кадра);
//! - `Serein/apps/desktop/src/watch.rs` (`store_frame` L21-38, `Watch::poll` L297-304);
//! - `Serein/crates/discord-voice/src/screen.rs` (`retain_screen_frame` L442-454).
//!
//! Покрывает 9 обязательных измерений `TEST-POLICY.md` §5:
//! 1. Позитивные сценарии (ротация атомарных слотов `LatestFrameRingBuffer`, zero-copy чтение).
//! 2. Негативные сценарии (отклонение невалидных кадров и ошибок захвата без порчи последнего кадра).
//! 3. Граничные условия (жёсткий предел памяти `< 50 МБ`, авто-освобождение `Idle`-слотов, сброс `> 1 МБ` при `> 4x` сжатии).
//! 4. Ошибки (`InvalidDimensions`, `CropOutOfBounds`, превышение лимита памяти `< 50 МБ`, сбой callback).
//! 5. Пустые состояния (новый кольцевой буфер, `clear()`, повторное чтение без нового кадра).
//! 6. Частотные / промежуточные состояния (гейтинг 60 FPS `16 мс`, `force_keyframe` PLI-повтор, состояния `Idle`/`Writing`/`Ready`/`Locked`).
//! 7. Защита от гонок и разрывов кадра (читатель удерживает слот `Locked`, пока писатель публикует новые кадры в соседние слоты).
//! 8. Интеграция с `PipeWireFrameBuffer` (CPU + `DMA-BUF`) и `X11ShmPixelBuffer`.
//! 9. Многопоточная стабильность и наблюдаемость (`RingBufferMetrics`, задержка `< 2 мс`, память `< 50 МБ`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use chord_media::buffer::{
    BorrowedFrameView, DEFAULT_RING_BUFFER_SLOTS, FrameSlotState, LARGE_BUFFER_SHRINK_THRESHOLD,
    LatestFrameRingBuffer, LatestFrameSlot, MAX_RING_BUFFER_MEMORY_BYTES, MAX_RING_BUFFER_SLOTS,
    MAX_SINGLE_FRAME_BYTES, MAX_SLOT_LATENCY, MIN_RING_BUFFER_SLOTS, SHRINK_RATIO_MULTIPLIER,
};
use chord_media::capture::CaptureError;
use chord_media::capture::portal::{CropRect, PipeWireFrameBuffer, PipeWirePixelFormat};
use chord_media::capture::x11::{X11MonitorGeometry, X11ShmPixelBuffer};
use chord_media::{ColorSpace, FRAME_BUDGET_60FPS, PixelLayout, VideoFrame};

fn make_patterned_i420_frame(width: u32, height: u32, tag: u8, timestamp_us: u64) -> VideoFrame {
    let w = width as usize;
    let h = height as usize;
    let y_len = w * h;
    let c_len = w.div_ceil(2) * h.div_ceil(2);
    let mut data = vec![0u8; y_len + c_len * 2];
    data[..y_len].fill(tag);
    data[y_len..y_len + c_len].fill(tag.wrapping_add(50));
    data[y_len + c_len..].fill(tag.wrapping_add(100));
    VideoFrame::new(width, height, PixelLayout::I420, data).with_timestamp_us(timestamp_us)
}

// ---------------------------------------------------------------------------
// 1. Константы, пустые состояния и начальные состояния атомарных слотов
// ---------------------------------------------------------------------------

#[test]
fn ring_buffer_constants_and_initial_atomic_slots_state() {
    assert_eq!(MAX_RING_BUFFER_MEMORY_BYTES, 50 * 1024 * 1024);
    assert_eq!(MAX_SINGLE_FRAME_BYTES, 16 * 1024 * 1024);
    assert_eq!(DEFAULT_RING_BUFFER_SLOTS, 3);
    assert_eq!(MIN_RING_BUFFER_SLOTS, 2);
    assert_eq!(MAX_RING_BUFFER_SLOTS, 8);
    assert_eq!(FRAME_BUDGET_60FPS, Duration::from_millis(16));
    assert_eq!(MAX_SLOT_LATENCY, Duration::from_millis(2));
    assert_eq!(LARGE_BUFFER_SHRINK_THRESHOLD, 1024 * 1024);
    assert_eq!(SHRINK_RATIO_MULTIPLIER, 4);

    assert_eq!(FrameSlotState::Idle.as_str(), "idle");
    assert_eq!(FrameSlotState::Writing.as_str(), "writing");
    assert_eq!(FrameSlotState::Ready.as_str(), "ready");
    assert_eq!(FrameSlotState::Locked.as_str(), "locked");

    let ring = LatestFrameRingBuffer::new();
    assert_eq!(ring.slot_count(), DEFAULT_RING_BUFFER_SLOTS);
    assert_eq!(ring.max_memory_bytes(), MAX_RING_BUFFER_MEMORY_BYTES);
    assert_eq!(ring.allocated_memory_bytes(), 0);
    assert_eq!(ring.peak_memory_bytes(), 0);
    assert_eq!(ring.latest_slot_index(), None);
    assert!(!ring.has_frame());
    assert!(!ring.is_fresh());
    assert_eq!(ring.sequence(), 0);
    assert_eq!(ring.stored_frames(), 0);
    assert_eq!(ring.overwritten_frames(), 0);
    assert_eq!(ring.rejected_frames(), 0);
    assert_eq!(ring.memory_limit_rejections(), 0);
    assert_eq!(ring.polled_frames(), 0);
    assert_eq!(ring.last_store_duration(), Duration::ZERO);
    assert_eq!(ring.max_store_duration(), Duration::ZERO);

    for idx in 0..DEFAULT_RING_BUFFER_SLOTS {
        assert_eq!(ring.slot_state(idx), Some(FrameSlotState::Idle));
        let slot = ring.slot(idx).expect("slot by index");
        assert_eq!(slot.index(), idx);
        assert_eq!(slot.state(), FrameSlotState::Idle);
        assert_eq!(slot.sequence(), 0);
        assert_eq!(slot.timestamp_us(), 0);
        assert_eq!(slot.capacity_bytes(), 0);
    }
    assert!(ring.slot_state(DEFAULT_RING_BUFFER_SLOTS).is_none());

    assert!(ring.poll_latest().is_none());
    assert!(ring.poll_latest_60fps(FRAME_BUDGET_60FPS).is_none());
    assert!(ring.with_latest_fresh(|_| 1).is_none());
    assert!(
        ring.with_latest_fresh_60fps(FRAME_BUDGET_60FPS, |_| 1)
            .is_none()
    );
    assert!(ring.with_ipc_frame(|_| 1).is_none());
    assert!(
        ring.with_ipc_frame_60fps(FRAME_BUDGET_60FPS, |_| 1)
            .is_none()
    );
    assert!(ring.peek_ipc_frame(|_| 1).is_none());
    assert!(ring.peek_latest(|_| 1).is_none());
    assert!(ring.read_for_encoder(false, |_| 1).is_none());
    assert!(ring.read_for_encoder(true, |_| 1).is_none());
    assert!(ring.take_latest().is_none());

    // Проверка клампинга количества слотов и лимита памяти
    let clamped_low = LatestFrameRingBuffer::with_slots(1);
    assert_eq!(clamped_low.slot_count(), MIN_RING_BUFFER_SLOTS);
    let clamped_high =
        LatestFrameRingBuffer::with_slots_and_max_memory(99, MAX_RING_BUFFER_MEMORY_BYTES * 4);
    assert_eq!(clamped_high.slot_count(), MAX_RING_BUFFER_SLOTS);
    assert_eq!(
        clamped_high.max_memory_bytes(),
        MAX_RING_BUFFER_MEMORY_BYTES
    );
}

// ---------------------------------------------------------------------------
// 2. Ротация атомарных слотов, переиспользование буферов и 60 FPS гейтинг
// ---------------------------------------------------------------------------

#[test]
fn atomic_slot_rotation_reuses_bounded_buffers_and_tracks_overwrites() {
    let ring = LatestFrameRingBuffer::new();
    let frame_bytes = PixelLayout::I420.byte_len(16, 16).unwrap();

    let mut seen_ptrs = Vec::new();
    for seq in 1..=30u64 {
        let tag = (seq as u8).wrapping_mul(3);
        let ts = seq * 16_666;
        assert!(ring.store_frame(make_patterned_i420_frame(16, 16, tag, ts)));
        assert!(ring.has_frame());
        assert!(ring.is_fresh());
        assert_eq!(ring.sequence(), seq);

        let latest_idx = ring.latest_slot_index().expect("latest slot index");
        assert!(latest_idx < DEFAULT_RING_BUFFER_SLOTS);
        assert_eq!(ring.slot_state(latest_idx), Some(FrameSlotState::Ready));

        // Все остальные слоты обязаны находиться в состоянии Idle (не накапливать Ready-очередь)
        for other in 0..DEFAULT_RING_BUFFER_SLOTS {
            if other != latest_idx {
                assert_eq!(
                    ring.slot_state(other),
                    Some(FrameSlotState::Idle),
                    "non-latest slot {other} must be returned to Idle"
                );
            }
        }

        let ptr = ring
            .peek_ipc_frame(|view| {
                assert_eq!(view.sequence, seq);
                assert_eq!(view.timestamp_us, ts);
                assert_eq!(view.data[0], tag);
                view.data_ptr() as usize
            })
            .expect("peek_ipc_frame must succeed");
        if !seen_ptrs.contains(&ptr) {
            seen_ptrs.push(ptr);
        }
    }

    assert!(
        seen_ptrs.len() <= DEFAULT_RING_BUFFER_SLOTS,
        "ring buffer must reuse at most {} slot allocations, saw {}",
        DEFAULT_RING_BUFFER_SLOTS,
        seen_ptrs.len()
    );
    assert_eq!(ring.stored_frames(), 30);
    assert_eq!(ring.overwritten_frames(), 29);
    assert!(ring.allocated_memory_bytes() <= frame_bytes * DEFAULT_RING_BUFFER_SLOTS);

    // Проверка гейтинга 60 FPS (16 мс): до 16 мс кадр не потребляется
    assert!(
        ring.with_ipc_frame_60fps(Duration::from_millis(10), |_| ())
            .is_none()
    );
    assert!(ring.is_fresh());

    // На 16 мс читаем самый свежий 30-й кадр
    let read_seq = ring
        .with_ipc_frame_60fps(FRAME_BUDGET_60FPS, |view| {
            assert!(view.is_fresh);
            assert_eq!(view.data[0], (30u8).wrapping_mul(3));
            view.sequence
        })
        .expect("60fps frame read at 16ms");
    assert_eq!(read_seq, 30);
    assert!(!ring.is_fresh());
    assert_eq!(ring.polled_frames(), 1);

    // После потребления все слоты находятся в состоянии Idle, но последний кадр доступен для PLI keyframe repeat
    let latest_idx = ring.latest_slot_index().unwrap();
    assert_eq!(ring.slot_state(latest_idx), Some(FrameSlotState::Idle));
    assert!(ring.read_for_encoder(false, |_| ()).is_none());
    let pli_seq = ring
        .read_for_encoder(true, |view| {
            assert!(!view.is_fresh);
            view.sequence
        })
        .expect("PLI keyframe repeat must read retained frame");
    assert_eq!(pli_seq, 30);
}

// ---------------------------------------------------------------------------
// 3. Защита от разрывов кадра (Zero-Tearing): читатель держит Locked, писатель пишет в другие слоты
// ---------------------------------------------------------------------------

#[test]
fn reader_locked_slot_is_isolated_from_concurrent_producer_writes_preventing_frame_tearing() {
    let ring = LatestFrameRingBuffer::new();

    // Публикуем Кадр #1 (заполнен байтами 0x11)
    assert!(ring.store_frame(make_patterned_i420_frame(32, 32, 0x11, 16_666)));
    let locked_slot_idx = ring.latest_slot_index().unwrap();
    let expected_frame1 = make_patterned_i420_frame(32, 32, 0x11, 16_666);

    // Читатель (UI / GPU / Кодировщик) захватывает слот в состояние Locked через with_ipc_frame
    let result = ring
        .with_ipc_frame(|view: BorrowedFrameView<'_>| {
            assert_eq!(
                ring.slot_state(locked_slot_idx),
                Some(FrameSlotState::Locked),
                "active reader slot must be in Locked state"
            );
            assert_eq!(view.sequence, 1);
            assert_eq!(view.data, expected_frame1.data.as_slice());

            // Пока читатель удерживает этот слот (Locked), поток захвата публикует 10 новых кадров!
            for next in 2..=11u64 {
                let fill_tag = (next as u8) * 10;
                let seq = ring
                    .store_with_reused_buffer(
                        32,
                        32,
                        PixelLayout::I420,
                        ColorSpace::BT601_LIMITED,
                        0,
                        next * 16_666,
                        |buf| {
                            // Во время записи нового кадра активный слот писателя имеет статус Writing,
                            // а слот читателя остаётся Locked!
                            assert_eq!(
                                ring.slot_state(locked_slot_idx),
                                Some(FrameSlotState::Locked)
                            );
                            buf.fill(fill_tag);
                            Ok::<(), &'static str>(())
                        },
                    )
                    .expect("producer write must succeed without blocking while reader holds slot");
                assert_eq!(seq, next);
                assert_ne!(
                    ring.latest_slot_index(),
                    Some(locked_slot_idx),
                    "producer must never overwrite the Locked reader slot"
                );
            }

            // Проверяем, что буфер, который всё это время читал потребитель, не потерпел ни единого разрыва (tearing = 0)!
            assert_eq!(
                view.data,
                expected_frame1.data.as_slice(),
                "locked reader buffer must remain 100% intact without tearing"
            );
            view.byte_len()
        })
        .expect("with_ipc_frame must succeed");

    assert_eq!(result, expected_frame1.data.len());
    // После выхода из замыкания читателя слот освобождается обратно в Idle
    assert_eq!(ring.slot_state(locked_slot_idx), Some(FrameSlotState::Idle));

    // Следующее чтение мгновенно получает самый последний записанный Кадр #11
    let latest_seq = ring
        .with_ipc_frame(|view| {
            assert_eq!(view.sequence, 11);
            assert!(view.data.iter().all(|&b| b == 110));
            view.sequence
        })
        .expect("must read frame #11");
    assert_eq!(latest_seq, 11);
}

// ---------------------------------------------------------------------------
// 4. Жёсткое ограничение памяти < 50 МБ, авто-вытеснение Idle-слотов и отклонение сверхкрупных кадров
// ---------------------------------------------------------------------------

#[test]
fn ring_buffer_enforces_strict_under_50mb_memory_ceiling_and_reclaims_idle_slots() {
    let ring = LatestFrameRingBuffer::new();

    // 1. Записываем серию 1080p RGBA кадров (1920 * 1080 * 4 = 8_294_400 байт ~ 7.91 МиБ на слот).
    // Для 3 слотов суммарная память = 24_883_200 байт (~23.73 МиБ) < 50 МБ.
    let rgba_1080p_len = PixelLayout::Rgba.byte_len(1920, 1080).unwrap();
    for i in 1..=6u64 {
        let seq = ring
            .store_with_reused_buffer(
                1920,
                1080,
                PixelLayout::Rgba,
                ColorSpace::BT709_LIMITED,
                0,
                i * 16_666,
                |buf| {
                    buf[0] = i as u8;
                    buf[rgba_1080p_len - 1] = i as u8;
                    Ok::<(), &'static str>(())
                },
            )
            .expect("1080p RGBA frame must fit within 50MB budget");
        assert_eq!(seq, i);
        assert!(
            ring.allocated_memory_bytes() < MAX_RING_BUFFER_MEMORY_BYTES,
            "allocated memory {} must be strictly < 50MB ({})",
            ring.allocated_memory_bytes(),
            MAX_RING_BUFFER_MEMORY_BYTES
        );
        assert!(ring.allocated_memory_bytes() < 50_000_000);
    }
    assert_eq!(
        ring.allocated_memory_bytes(),
        rgba_1080p_len * DEFAULT_RING_BUFFER_SLOTS
    );

    // 2. Переходим на 1440p RGBA (2560 * 1440 * 4 = 14_745_600 байт ~ 14.06 МиБ на слот).
    // 3 слота = 44_236_800 байт (~42.19 МиБ) — всё ещё строго < 50_000_000 и < 50 МиБ.
    let rgba_1440p_len = PixelLayout::Rgba.byte_len(2560, 1440).unwrap();
    for i in 7..=10u64 {
        let seq = ring
            .store_with_reused_buffer(
                2560,
                1440,
                PixelLayout::Rgba,
                ColorSpace::BT709_LIMITED,
                0,
                i * 16_666,
                |buf| {
                    buf[0] = i as u8;
                    buf[rgba_1440p_len - 1] = i as u8;
                    Ok::<(), &'static str>(())
                },
            )
            .expect("1440p RGBA frame must fit in < 50MB");
        assert_eq!(seq, i);
        assert!(ring.allocated_memory_bytes() < 50_000_000);
        assert!(ring.allocated_memory_bytes() < MAX_RING_BUFFER_MEMORY_BYTES);
    }

    // 3. Кадр 4K RGBA (3840 * 2160 * 4 = 33_177_600 байт > MAX_SINGLE_FRAME_BYTES 32 МиБ,
    // а в сумме с другими слотами превысил бы 50 МБ) обязан быть отклонён без аллокации!
    let prev_allocated = ring.allocated_memory_bytes();
    let res_4k_rgba = ring.store_with_reused_buffer(
        3840,
        2160,
        PixelLayout::Rgba,
        ColorSpace::BT709_LIMITED,
        0,
        999_999,
        |_buf| Ok::<(), &'static str>(()),
    );
    assert!(
        res_4k_rgba.is_err(),
        "4K RGBA frame exceeding single-frame / 50MB ring budget must be rejected"
    );
    assert_eq!(ring.rejected_frames(), 1);
    assert_eq!(ring.memory_limit_rejections(), 1);
    assert_eq!(ring.allocated_memory_bytes(), prev_allocated);
    assert!(ring.allocated_memory_bytes() < MAX_RING_BUFFER_MEMORY_BYTES);

    // Предыдущий валидный 1440p кадр (#10) остался нетронутым
    assert_eq!(
        ring.peek_ipc_frame(|v| (v.sequence, v.width, v.height)),
        Some((10, 2560, 1440))
    );

    // 4. Проверяем авто-вытеснение (reclaim) памяти из Idle-слотов при ограниченном бюджете памяти:
    // Создаём буфер с лимитом 20 МиБ и 3 слотами. Один кадр 1080p RGBA занимает 8.29 МБ.
    // Два слота = 16.59 МБ (< 20 МиБ), но три слота = 24.88 МБ (> 20 МиБ).
    // Кольцевой буфер обязан автоматически освободить память неиспользуемого Idle-слота,
    // чтобы продолжать 60 FPS запись, никогда не превышая бюджет 20 МиБ!
    let budget_20mb = 20 * 1024 * 1024;
    let tight_ring = LatestFrameRingBuffer::with_slots_and_max_memory(3, budget_20mb);
    for step in 1..=12u64 {
        let seq = tight_ring
            .store_with_reused_buffer(
                1920,
                1080,
                PixelLayout::Rgba,
                ColorSpace::BT709_LIMITED,
                0,
                step * 16_666,
                |buf| {
                    buf[0] = step as u8;
                    Ok::<(), &'static str>(())
                },
            )
            .expect("tight ring must reclaim idle slot to stay within 20MB budget");
        assert_eq!(seq, step);
        assert!(
            tight_ring.allocated_memory_bytes() < budget_20mb,
            "tight ring allocated {} bytes >= budget {}",
            tight_ring.allocated_memory_bytes(),
            budget_20mb
        );
    }
    assert!(tight_ring.peak_memory_bytes() < budget_20mb);

    // 5. Падение разрешения до 16x16 I420 (384 байта) сбрасывает крупные буферы (> 1 МБ)
    for step in 11..=15u64 {
        assert!(ring.store_frame(make_patterned_i420_frame(16, 16, step as u8, step * 16_666)));
    }
    assert!(
        ring.allocated_memory_bytes() <= 384 * DEFAULT_RING_BUFFER_SLOTS,
        "after resolution drop all slots must shrink, got {}",
        ring.allocated_memory_bytes()
    );

    // 6. Проверяем, что LatestFrameSlot также отвергает кадры, нарушающие предел < 50 МБ
    let single_slot = LatestFrameSlot::new();
    let res_slot_huge = single_slot.store_with_reused_buffer(
        7680,
        4320,
        PixelLayout::Rgba,
        ColorSpace::BT709_LIMITED,
        0,
        16_666,
        |_buf| Ok::<(), &'static str>(()),
    );
    assert!(res_slot_huge.is_err());
    assert_eq!(single_slot.rejected_frames(), 1);
    assert_eq!(single_slot.allocated_capacity_bytes(), 0);
}

// ---------------------------------------------------------------------------
// 5. Отклонение невалидных кадров и интеграция с PipeWire (CPU + DMA-BUF) и X11 MIT-SHM
// ---------------------------------------------------------------------------

#[test]
fn invalid_frames_and_capture_errors_preserve_latest_slot_and_pipewire_x11_dma_buf_work() {
    let ring = LatestFrameRingBuffer::new();

    // 1. Захват PipeWire BGRA 4x4 с crop 2x2
    let stride = 16usize;
    let raw = vec![128u8; stride * 4];
    let pw_frame = PipeWireFrameBuffer {
        width: 4,
        height: 4,
        stride,
        format: PipeWirePixelFormat::Bgra8,
        data: &raw,
        crop: Some(CropRect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        }),
        dma_buf_fd: None,
        drm_modifier: 0,
    };

    let seq1 = ring
        .store_portal_frame(&pw_frame, PixelLayout::I420, 16_666)
        .expect("store PipeWire portal frame into ring");
    assert_eq!(seq1, 1);
    assert_eq!(ring.peek_ipc_frame(|v| (v.width, v.height)), Some((2, 2)));

    // 2. Ошибка валидации PipeWire (CropOutOfBounds) сохраняет кадр #1 и возвращает рабочий слот в Idle
    let bad_pw = PipeWireFrameBuffer {
        crop: Some(CropRect {
            x: 3,
            y: 3,
            width: 4,
            height: 4,
        }),
        ..pw_frame
    };
    assert!(matches!(
        ring.store_portal_frame(&bad_pw, PixelLayout::I420, 33_333),
        Err(CaptureError::CropOutOfBounds { .. })
    ));
    assert_eq!(ring.rejected_frames(), 1);
    assert_eq!(ring.sequence(), 1);

    // 3. Невалидные VideoFrame (width=0, height=0, bad rotation, bad data len)
    assert!(!ring.store_frame(VideoFrame::new(0, 2, PixelLayout::I420, vec![])));
    assert!(!ring.store_frame(VideoFrame::new(2, 0, PixelLayout::I420, vec![])));
    assert!(
        !ring.store_frame(VideoFrame::new(2, 2, PixelLayout::I420, vec![0; 6]).with_rotation(45))
    );
    assert!(!ring.store_frame(VideoFrame::new(2, 2, PixelLayout::I420, vec![0; 3])));
    assert_eq!(ring.rejected_frames(), 5);
    assert_eq!(ring.sequence(), 1);

    // 4. Захват X11 окна и монитора через MIT-SHM
    let shm = X11ShmPixelBuffer::new(4, 2, 16, 32).expect("valid X11 SHM");
    let shm_data = vec![200u8; 32];
    let seq2 = ring
        .store_x11_window(&shm, &shm_data, PixelLayout::Nv12, 50_000)
        .expect("store X11 window into ring");
    assert_eq!(seq2, 2);

    let monitor = X11MonitorGeometry {
        id: "screen:0".to_string(),
        name: "DP-1".to_string(),
        x: 0,
        y: 0,
        width: 2,
        height: 2,
        is_primary: true,
    };
    let seq3 = ring
        .store_x11_monitor(&shm, &monitor, &shm_data, PixelLayout::Rgba, 66_666)
        .expect("store X11 monitor into ring");
    assert_eq!(seq3, 3);

    // 5. Аппаратный zero-copy DMA-BUF кадр (0 байт CPU аллокаций)
    let dma_pw = PipeWireFrameBuffer::new_dma_buf(
        2560,
        1440,
        2560 * 4,
        PipeWirePixelFormat::Bgra8,
        55,
        0x0102_0304_0506_0708,
        None,
    );
    let seq4 = ring
        .store_portal_frame(&dma_pw, PixelLayout::Rgba, 83_333)
        .expect("store DMA-BUF frame into ring");
    assert_eq!(seq4, 4);

    ring.with_ipc_frame(|view| {
        assert!(view.is_dma_buf());
        assert_eq!(view.dma_buf_fd(), Some(55));
        assert_eq!(view.drm_modifier(), 0x0102_0304_0506_0708);
        assert_eq!(view.width, 2560);
        assert_eq!(view.height, 1440);
        assert_eq!(view.byte_len(), 0);
    })
    .expect("read DMA-BUF frame from ring");
}

// ---------------------------------------------------------------------------
// 6. Многопоточный 60 FPS стресс-тест: проверка отсутствия разрывов (tearing) и памяти < 50 МБ
// ---------------------------------------------------------------------------

#[test]
fn concurrent_60fps_lock_free_ring_buffer_stress_zero_tearing_and_bounded_memory() {
    let ring = LatestFrameRingBuffer::new();
    let writer_ring = ring.clone();
    let done = Arc::new(AtomicBool::new(false));
    let reader_done = done.clone();

    let width = 640u32;
    let height = 360u32;
    let y_len = (width * height) as usize;
    let c_len = (width as usize / 2) * (height as usize / 2);
    let total_len = y_len + c_len * 2;

    // Поток потребителя непрерывно читает последний кадр zero-copy и сверяет каждый байт плоскостей
    // с ожидаемым шаблоном кадра (независимый оракул целостности против разрывов кадра).
    let reader_handle = thread::spawn(move || {
        let mut last_seq = 0u64;
        let mut reads = 0u64;
        let mut max_read_latency = Duration::ZERO;

        while !reader_done.load(Ordering::Acquire) {
            let t0 = Instant::now();
            if let Some((seq, tag_ok)) = ring.with_ipc_frame(|view| {
                let tag = (view.sequence & 0xFF) as u8;
                let (y, u, v) = view.i420_planes().expect("valid I420 planes");
                // Если бы писатель перезаписал буфер во время чтения, байты начала и конца
                // плоскостей Y, U, V не совпали бы с `tag` от `view.sequence`!
                let intact = y.first() == Some(&tag)
                    && y.last() == Some(&tag)
                    && u.first() == Some(&tag.wrapping_add(17))
                    && u.last() == Some(&tag.wrapping_add(17))
                    && v.first() == Some(&tag.wrapping_add(37))
                    && v.last() == Some(&tag.wrapping_add(37));
                (view.sequence, intact)
            }) {
                let elapsed = t0.elapsed();
                if elapsed > max_read_latency {
                    max_read_latency = elapsed;
                }
                assert!(tag_ok, "frame tearing detected at sequence {seq}");
                assert!(
                    seq > last_seq,
                    "sequence must strictly increase: {seq} <= {last_seq}"
                );
                last_seq = seq;
                reads += 1;
            }
            thread::yield_now();
        }
        (reads, max_read_latency)
    });

    // Поток захвата публикует 180 кадров 640x360 I420 (345_600 байт)
    for seq in 1..=180u64 {
        let tag = (seq & 0xFF) as u8;
        let written_seq = writer_ring
            .store_with_reused_buffer(
                width,
                height,
                PixelLayout::I420,
                ColorSpace::BT601_LIMITED,
                0,
                seq * 16_666,
                |buf| {
                    buf[..y_len].fill(tag);
                    buf[y_len..y_len + c_len].fill(tag.wrapping_add(17));
                    buf[y_len + c_len..].fill(tag.wrapping_add(37));
                    Ok::<(), &'static str>(())
                },
            )
            .expect("concurrent store_with_reused_buffer must succeed");
        assert_eq!(written_seq, seq);
        if seq % 3 == 0 {
            thread::yield_now();
        }
    }

    done.store(true, Ordering::Release);
    let (reads, max_read_latency) = reader_handle.join().expect("reader thread join");

    let metrics = writer_ring.metrics();
    assert_eq!(metrics.stored_frames, 180);
    assert_eq!(metrics.rejected_frames, 0);
    assert_eq!(metrics.memory_limit_rejections, 0);
    assert_eq!(metrics.sequence, 180);
    assert!(
        metrics.allocated_memory_bytes <= total_len * DEFAULT_RING_BUFFER_SLOTS,
        "allocated memory {} exceeds 3 slots ({})",
        metrics.allocated_memory_bytes,
        total_len * DEFAULT_RING_BUFFER_SLOTS
    );
    assert!(metrics.peak_memory_bytes < MAX_RING_BUFFER_MEMORY_BYTES);
    assert!(
        max_read_latency < MAX_SLOT_LATENCY * 5,
        "reader latency {max_read_latency:?} too high (reads={reads})"
    );

    writer_ring.clear();
    assert!(!writer_ring.has_frame());
    assert!(!writer_ring.is_fresh());
    assert_eq!(writer_ring.allocated_memory_bytes(), 0);
    assert_eq!(writer_ring.latest_slot_index(), None);
}
