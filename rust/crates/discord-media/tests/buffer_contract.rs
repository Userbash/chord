//! TASK-614: Контрактные тесты атомарного 60 FPS кольцевого слота последнего кадра (`discord_media::buffer`).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/voice-deep.md` и `docs/re/voice-deep.md` (§3: видеопоток 1280x720@30/60,
//!   кодеки AV1/H264, `updateVideoQuality`);
//! - `/home/sanya/re-deep/native-re.md` (`VoiceEngine.getNextVideoOutputFrame`, `WumpusVideoSource`,
//!   `FrameBuffer/Pool`, `BaseCapturerPipeWire`, `ScreenCapturerX11`, `WindowCapturerX11`, `XServerPixelBuffer`);
//! - `Serein/apps/desktop/src/watch.rs` (`store_frame` L21-38, `Watch::poll` L297-304, тесты L480-544);
//! - `Serein/crates/discord-voice/src/screen.rs` (`retain_screen_frame` L442-454).
//!
//! Покрывает 9 обязательных измерений `TEST-POLICY.md` §5:
//! 1. Позитивные сценарии (запись/чтение `LatestFrameSlot`, переиспользование буфера, zero-copy чтение).
//! 2. Негативные сценарии (отклонение невалидных кадров без порчи последнего валидного кадра).
//! 3. Граничные условия (сброс буфера > 1 МБ при падении разрешения в >4 раза, точный `reserve_exact` без удвоения, бюджет 16 мс / 2 мс).
//! 4. Ошибки захвата (`InvalidDimensions`, `InvalidStride`, `BufferTooSmall`, `CropOutOfBounds` сохраняют прежний кадр).
//! 5. Пустые состояния (новый слот, `clear()`, повторное чтение без нового кадра).
//! 6. Промежуточные / частотные состояния (60 FPS гейтинг `16 мс`, `force_keyframe` повтор кадра для энкодера при статичном экране).
//! 7. Изоляция и безопасность памяти (`#![forbid(unsafe_code)]`, отсутствие накопления памяти).
//! 8. Интеграция с `PipeWireFrameBuffer` (`capture::portal`) и `X11ShmPixelBuffer` (`capture::x11`) через двойную буферизацию (`slot` + `scratch`).
//! 9. Многопоточная стабильность и наблюдаемость (`LatestFrameMetrics`, задержка `< 2 мс`, отсутствие блокировки UI-потока).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use discord_media::buffer::{
    BorrowedFrameView, LARGE_BUFFER_SHRINK_THRESHOLD, LatestFrameMetrics, LatestFrameSlot,
    MAX_SLOT_LATENCY, SHRINK_RATIO_MULTIPLIER,
};
use discord_media::capture::CaptureError;
use discord_media::capture::portal::{CropRect, PipeWireFrameBuffer, PipeWirePixelFormat};
use discord_media::capture::x11::{X11MonitorGeometry, X11ShmPixelBuffer, X11WindowMetadata};
use discord_media::{
    ColorSpace, DesktopBackend, FRAME_BUDGET_60FPS, PixelLayout, ScreenCapturer, VideoFrame,
};

fn make_i420_frame(width: u32, height: u32, fill: u8, timestamp_us: u64) -> VideoFrame {
    let len = PixelLayout::I420
        .byte_len(width, height)
        .expect("valid I420 byte_len");
    VideoFrame::new(width, height, PixelLayout::I420, vec![fill; len])
        .with_timestamp_us(timestamp_us)
}

// ---------------------------------------------------------------------------
// 1. Пустые состояния, константы и базовые метрики LatestFrameSlot
// ---------------------------------------------------------------------------

#[test]
fn buffer_module_constants_and_initial_empty_slot_state() {
    assert_eq!(FRAME_BUDGET_60FPS, Duration::from_millis(16));
    assert_eq!(MAX_SLOT_LATENCY, Duration::from_millis(2));
    assert_eq!(LARGE_BUFFER_SHRINK_THRESHOLD, 1024 * 1024);
    assert_eq!(SHRINK_RATIO_MULTIPLIER, 4);

    let slot = LatestFrameSlot::new();
    assert!(!slot.has_frame());
    assert!(!slot.is_fresh());
    assert_eq!(slot.sequence(), 0);
    assert_eq!(slot.stored_frames(), 0);
    assert_eq!(slot.overwritten_frames(), 0);
    assert_eq!(slot.rejected_frames(), 0);
    assert_eq!(slot.polled_frames(), 0);
    assert_eq!(slot.slot_capacity_bytes(), 0);
    assert_eq!(slot.allocated_capacity_bytes(), 0);
    assert_eq!(slot.last_store_duration(), Duration::ZERO);
    assert_eq!(slot.max_store_duration(), Duration::ZERO);

    assert_eq!(slot.metrics(), LatestFrameMetrics::default());

    assert!(slot.poll_latest().is_none());
    assert!(slot.poll_latest_60fps(FRAME_BUDGET_60FPS).is_none());
    assert!(slot.with_latest_fresh(|_| 1).is_none());
    assert!(
        slot.with_latest_fresh_60fps(FRAME_BUDGET_60FPS, |_| 1)
            .is_none()
    );
    assert!(slot.with_ipc_frame(|_| 1).is_none());
    assert!(slot.peek_ipc_frame(|_| 1).is_none());
    assert!(slot.peek_latest(|_| 1).is_none());
    assert!(slot.read_for_encoder(false, |_| 1).is_none());
    assert!(slot.read_for_encoder(true, |_| 1).is_none());
    assert!(slot.take_latest().is_none());
}

// ---------------------------------------------------------------------------
// 2. Переиспользование аллокации на месте и авто-замена устаревших кадров
// ---------------------------------------------------------------------------

#[test]
fn store_frame_reuses_exact_heap_allocation_and_overwrites_stale_frames() {
    let slot = LatestFrameSlot::new();

    assert!(slot.store_frame(make_i420_frame(4, 4, 1, 16_666)));
    assert!(slot.has_frame());
    assert!(slot.is_fresh());
    assert_eq!(slot.sequence(), 1);
    assert_eq!(slot.stored_frames(), 1);
    assert_eq!(slot.overwritten_frames(), 0);
    assert_eq!(slot.slot_capacity_bytes(), 24);

    let initial_ptr = slot
        .peek_ipc_frame(|view| view.data_ptr() as usize)
        .expect("slot must contain frame");

    // Записываем 20 кадров без чтения UI-потоком: устаревшие кадры заменяются на месте
    // в том же буфере памяти без роста аллокаций.
    for idx in 1..=20u8 {
        let ts = u64::from(idx + 1) * 16_666;
        assert!(slot.store_frame(make_i420_frame(4, 4, idx + 10, ts)));
        let (ptr, seq, first_byte, cap) = slot
            .peek_ipc_frame(|view| {
                (
                    view.data_ptr() as usize,
                    view.sequence,
                    view.data[0],
                    view.byte_len(),
                )
            })
            .expect("peek_ipc_frame must succeed");
        assert_eq!(
            ptr, initial_ptr,
            "frame {idx} must reuse the same heap buffer"
        );
        assert_eq!(seq, u64::from(idx) + 1);
        assert_eq!(first_byte, idx + 10);
        assert_eq!(cap, 24);
        assert_eq!(slot.slot_capacity_bytes(), 24);
    }

    assert_eq!(slot.stored_frames(), 21);
    assert_eq!(slot.overwritten_frames(), 20);
    assert!(slot.is_fresh());

    // Проверка границы бюджета 60 FPS (16 мс): при < 16 мс кадр не выдаётся и флаг fresh сохраняется.
    assert!(
        slot.with_latest_fresh_60fps(Duration::from_millis(8), |_| ())
            .is_none()
    );
    assert!(
        slot.with_latest_fresh_60fps(Duration::from_millis(15), |_| ())
            .is_none()
    );
    assert!(slot.is_fresh());

    // На границе >= 16 мс чтение через zero-copy замыкание потребляет флаг свежести, но оставляет буфер внутри слота.
    let read_ts = slot
        .with_latest_fresh_60fps(FRAME_BUDGET_60FPS, |frame| {
            assert_eq!(frame.data.as_ptr() as usize, initial_ptr);
            assert_eq!(frame.data[0], 30);
            frame.timestamp_us
        })
        .expect("fresh frame must be yielded at 16ms budget");
    assert_eq!(read_ts, 21 * 16_666);
    assert!(!slot.is_fresh());
    assert_eq!(slot.polled_frames(), 1);
    assert!(slot.with_latest_fresh(|_| ()).is_none());
    assert!(
        slot.with_latest_fresh_60fps(Duration::from_millis(20), |_| ())
            .is_none()
    );

    // Кадр меньшего размера (2x2 = 6 байт) также переиспользует существующую аллокацию 24 байта.
    assert!(slot.store_frame(make_i420_frame(2, 2, 99, 500_000)));
    let (smaller_ptr, dims, len) = slot
        .with_ipc_frame(|view| {
            (
                view.data_ptr() as usize,
                (view.width, view.height),
                view.byte_len(),
            )
        })
        .expect("smaller frame must be readable");
    assert_eq!(smaller_ptr, initial_ptr);
    assert_eq!(dims, (2, 2));
    assert_eq!(len, 6);
    assert_eq!(slot.slot_capacity_bytes(), 24);
}

// ---------------------------------------------------------------------------
// 3. Сброс крупного буфера при падении разрешения и точный рост без удвоения
// ---------------------------------------------------------------------------

#[test]
fn resolution_drop_releases_large_buffer_and_growth_uses_exact_capacity() {
    let slot = LatestFrameSlot::new();

    // 1080p I420 = 1920 * 1080 * 3 / 2 = 3_110_400 байт (> 1 МБ).
    let hd_bytes = 1920 * 1080 * 3 / 2;
    assert!(slot.store_frame(make_i420_frame(1920, 1080, 16, 16_666)));
    assert_eq!(slot.slot_capacity_bytes(), hd_bytes);

    // Падение разрешения до 16x16 (384 байта; 3_110_400 > 384 * 4) освобождает старый буфер 3.11 МБ.
    let small_bytes = 16 * 16 * 3 / 2;
    assert!(slot.store_frame(make_i420_frame(16, 16, 80, 33_333)));
    assert_eq!(slot.slot_capacity_bytes(), small_bytes);
    assert!(slot.allocated_capacity_bytes() <= small_bytes * SHRINK_RATIO_MULTIPLIER);

    // Рост разрешения с 1600x900 (2_160_000 Б) до 1920x1080 (3_110_400 Б) выделяет ровно
    // 3_110_400 Б через reserve_exact, не удваивая ёмкость до 4_320_000 Б.
    assert!(slot.store_frame(make_i420_frame(1600, 900, 32, 50_000)));
    assert_eq!(slot.slot_capacity_bytes(), 1600 * 900 * 3 / 2);

    assert!(slot.store_frame(make_i420_frame(1920, 1080, 64, 66_666)));
    assert_eq!(slot.slot_capacity_bytes(), hd_bytes);
}

// ---------------------------------------------------------------------------
// 4. Отклонение невалидных кадров с сохранением последнего валидного кадра
// ---------------------------------------------------------------------------

#[test]
fn invalid_frames_are_rejected_and_preserve_previous_valid_frame_and_freshness() {
    let slot = LatestFrameSlot::new();
    let valid = VideoFrame::new(2, 2, PixelLayout::Nv12, vec![16, 16, 16, 16, 128, 128])
        .with_timestamp_us(12_345);
    assert!(slot.store_frame(valid));
    let valid_ptr = slot.peek_ipc_frame(|v| v.data_ptr() as usize).unwrap();

    let invalid_cases = [
        VideoFrame::new(0, 2, PixelLayout::Nv12, vec![]),
        VideoFrame::new(2, 0, PixelLayout::Nv12, vec![]),
        VideoFrame::new(2, 2, PixelLayout::Nv12, vec![1, 2, 3]),
        VideoFrame::new(2, 2, PixelLayout::Nv12, vec![16, 16, 16, 16, 128, 128]).with_rotation(45),
        VideoFrame::new(u32::MAX, u32::MAX, PixelLayout::I420, vec![]),
    ];

    for bad in invalid_cases {
        assert!(!slot.store_frame(bad));
    }

    assert_eq!(slot.rejected_frames(), 5);
    assert_eq!(slot.stored_frames(), 1);
    assert_eq!(slot.overwritten_frames(), 0);
    assert_eq!(slot.sequence(), 1);
    assert!(slot.is_fresh());

    let preserved = slot
        .peek_ipc_frame(|view| {
            (
                view.data_ptr() as usize,
                view.width,
                view.height,
                view.timestamp_us,
                view.data.to_vec(),
            )
        })
        .expect("previous valid frame must remain intact");
    assert_eq!(preserved.0, valid_ptr);
    assert_eq!(preserved.1, 2);
    assert_eq!(preserved.2, 2);
    assert_eq!(preserved.3, 12_345);
    assert_eq!(preserved.4, vec![16, 16, 16, 16, 128, 128]);
}

// ---------------------------------------------------------------------------
// 5. Zero-copy чтение для кодировщика и Tauri IPC (BorrowedFrameView, PLI repeat)
// ---------------------------------------------------------------------------

#[test]
fn zero_copy_read_for_encoder_and_tauri_ipc_borrows_planes_without_cloning() {
    let slot = LatestFrameSlot::new();

    // 1. I420 кадр 4x2 (Y = 8 байт, U = 2 байта, V = 2 байта) с поворотом 90°
    let i420_data = vec![10, 11, 12, 13, 14, 15, 16, 17, 100, 101, 200, 201];
    let frame_i420 = VideoFrame::new(4, 2, PixelLayout::I420, i420_data)
        .with_color(ColorSpace::BT709_LIMITED)
        .with_rotation(90)
        .with_timestamp_us(77_000);
    assert!(slot.store_frame(frame_i420));

    let raw_slot_ptr = slot.inner().lock().unwrap().as_ref().unwrap().data.as_ptr() as usize;

    // Чтение для Tauri IPC заимствует срез напрямую из слота без копирования Vec<u8>
    let ipc_summary = slot
        .with_ipc_frame(|view: BorrowedFrameView<'_>| {
            assert_eq!(view.data_ptr() as usize, raw_slot_ptr);
            assert_eq!(view.width, 4);
            assert_eq!(view.height, 2);
            assert_eq!(view.display_size, (2, 4));
            assert_eq!(view.rotation, 90);
            assert_eq!(view.color, ColorSpace::BT709_LIMITED);
            assert_eq!(view.timestamp_us, 77_000);
            assert_eq!(view.sequence, 1);
            assert!(view.is_fresh);
            assert_eq!(
                view.luma_plane(),
                Some(&[10, 11, 12, 13, 14, 15, 16, 17][..])
            );
            let (y, u, v) = view.i420_planes().expect("I420 planes");
            assert_eq!(y, &[10, 11, 12, 13, 14, 15, 16, 17]);
            assert_eq!(u, &[100, 101]);
            assert_eq!(v, &[200, 201]);
            assert!(view.nv12_planes().is_none());
            assert!(view.rgba_bytes().is_none());
            view.byte_len()
        })
        .expect("with_ipc_frame must succeed on fresh frame");
    assert_eq!(ipc_summary, 12);
    assert!(!slot.is_fresh());

    // Повторный вызов with_ipc_frame без нового кадра возвращает None
    assert!(slot.with_ipc_frame(|_| ()).is_none());

    // Кодировщик: без нового кадра и без force_keyframe возвращает None
    assert!(slot.read_for_encoder(false, |_| ()).is_none());

    // Кодировщик: при запросе RTCP PLI / FIR (force_keyframe = true) на статичном экране
    // возвращает удержанный кадр без клонирования Vec<u8> и с признаком is_fresh == false!
    let pli_repeat = slot
        .read_for_encoder(true, |view| {
            assert_eq!(view.data_ptr() as usize, raw_slot_ptr);
            assert!(!view.is_fresh);
            assert_eq!(view.sequence, 1);
            view.timestamp_us
        })
        .expect("encoder PLI repeat must read retained frame");
    assert_eq!(pli_repeat, 77_000);

    // 2. NV12 и RGBA плоскости через BorrowedFrameView
    let nv12 = VideoFrame::new(2, 2, PixelLayout::Nv12, vec![1, 2, 3, 4, 50, 60]);
    assert!(slot.store_frame(nv12));
    slot.with_ipc_frame(|view| {
        let (y, uv) = view.nv12_planes().expect("NV12 planes");
        assert_eq!(y, &[1, 2, 3, 4]);
        assert_eq!(uv, &[50, 60]);
    })
    .unwrap();

    let rgba = VideoFrame::new(1, 1, PixelLayout::Rgba, vec![255, 128, 64, 255]);
    assert!(slot.store_frame(rgba));
    slot.with_ipc_frame(|view| {
        assert_eq!(view.rgba_bytes(), Some(&[255, 128, 64, 255][..]));
        assert!(view.luma_plane().is_none());
    })
    .unwrap();
}

// ---------------------------------------------------------------------------
// 6. Интеграция с PipeWireFrameBuffer (discord_media::capture::portal)
// ---------------------------------------------------------------------------

#[test]
fn portal_pipewire_frame_buffer_integration_writes_into_slot_with_buffer_reuse() {
    let slot = LatestFrameSlot::new();
    let capturer = ScreenCapturer::with_backend(DesktopBackend::PipeWirePortal);

    // Создаём буфер 4x4 BGRA со stride = 24 байта и областью обрезки 2x2 со смещением (1, 1)
    let stride = 24usize;
    let mut raw = vec![0u8; stride * 4];
    for row in 0..4usize {
        for col in 0..4usize {
            let off = row * stride + col * 4;
            raw[off] = (col * 40) as u8; // B
            raw[off + 1] = (row * 50) as u8; // G
            raw[off + 2] = 200; // R
            raw[off + 3] = 128; // A
        }
    }

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
    };

    // Сравниваем прямую запись в LatestFrameSlot с эталонным to_video_frame для Rgba, I420 и Nv12
    for (idx, layout) in [PixelLayout::Rgba, PixelLayout::I420, PixelLayout::Nv12]
        .into_iter()
        .enumerate()
    {
        let ts = (idx as u64 + 1) * 16_666;
        let expected = pw_frame
            .to_video_frame(layout, ts)
            .expect("oracle to_video_frame");
        let seq = capturer
            .capture_portal_frame_into_slot(&pw_frame, layout, ts, &slot)
            .expect("capture_portal_frame_into_slot");
        assert_eq!(seq, idx as u64 + 1);

        slot.with_latest_fresh(|actual| {
            assert_eq!(actual, &expected);
        })
        .expect("frame in slot must match oracle");
    }

    // Проверяем, что в стационарном цикле 60 FPS пинг-понг буферы (slot + scratch)
    // переиспользуют ровно 2 аллокации без накопления памяти.
    let mut seen_ptrs = Vec::new();
    for step in 0..20u64 {
        let seq = pw_frame
            .capture_into_slot(&slot, PixelLayout::I420, 100_000 + step * 16_666)
            .expect("capture_into_slot in loop");
        assert_eq!(seq, 4 + step);
        let ptr = slot.peek_ipc_frame(|v| v.data_ptr() as usize).unwrap();
        if !seen_ptrs.contains(&ptr) {
            seen_ptrs.push(ptr);
        }
    }
    assert_eq!(
        seen_ptrs.len(),
        2,
        "ping-pong ring must reuse exactly 2 buffers, saw {} pointers",
        seen_ptrs.len()
    );
    // Буфер A сохранил ёмкость 16 байт от первого прохода Rgba (2x2x4), буфер B имеет ровно 6 байт (2x2 I420).
    assert_eq!(slot.allocated_capacity_bytes(), 16 + 6);

    // Ошибка валидации PipeWireFrameBuffer (CropOutOfBounds) не должна затирать последний кадр в слоте
    let bad_pw = PipeWireFrameBuffer {
        crop: Some(CropRect {
            x: 3,
            y: 3,
            width: 2,
            height: 2,
        }),
        ..pw_frame
    };
    let err = bad_pw
        .capture_into_slot(&slot, PixelLayout::I420, 999_999)
        .expect_err("out-of-bounds crop must fail");
    assert!(matches!(err, CaptureError::CropOutOfBounds { .. }));
    assert_eq!(slot.rejected_frames(), 1);
    assert_eq!(
        slot.peek_ipc_frame(|v| v.timestamp_us),
        Some(100_000 + 19 * 16_666)
    );
}

// ---------------------------------------------------------------------------
// 7. Интеграция с X11 MIT-SHM (discord_media::capture::x11)
// ---------------------------------------------------------------------------

#[test]
fn x11_shm_window_and_monitor_integration_writes_into_slot_and_handles_errors() {
    let slot = LatestFrameSlot::new();
    let capturer = ScreenCapturer::with_backend(DesktopBackend::X11Shm);

    let win_meta = X11WindowMetadata {
        xid: 0x400001,
        title: "Discord Lab Window".to_string(),
        class_name: Some("discord-lab".to_string()),
        x: 0,
        y: 0,
        width: 2,
        height: 2,
        is_mapped: true,
        is_minimized: false,
        is_override_redirect: false,
    };

    // BGRx 2x2 со stride = 8
    let shm_bytes = vec![
        255, 0, 0, 0, 0, 255, 0, 0, // строка 0: синий, зелёный
        0, 0, 255, 0, 255, 255, 255, 0, // строка 1: красный, белый
    ];

    let seq = capturer
        .capture_x11_shm_window_into_slot(
            &win_meta,
            &shm_bytes,
            8,
            PixelLayout::Rgba,
            42_000,
            &slot,
        )
        .expect("capture X11 window into slot");
    assert_eq!(seq, 1);

    slot.with_ipc_frame(|view| {
        assert_eq!(view.width, 2);
        assert_eq!(view.height, 2);
        assert_eq!(view.timestamp_us, 42_000);
        // Пиксель 0 (B=255, G=0, R=0) -> RGBA [0, 0, 255, 255]
        assert_eq!(&view.data[0..4], &[0, 0, 255, 255]);
        // Пиксель 2 (B=0, G=0, R=255) -> RGBA [255, 0, 0, 255]
        assert_eq!(&view.data[8..12], &[255, 0, 0, 255]);
    })
    .expect("X11 window frame in slot");

    // Захват монитора из root-окна 4x2 через X11ShmPixelBuffer::capture_monitor_rect_into_slot
    let root_shm = vec![128u8; 4 * 2 * 4];
    let shm_buf = X11ShmPixelBuffer::new(4, 2, 16, 24).expect("valid root SHM buffer");
    let monitor = X11MonitorGeometry {
        id: "screen:0".to_string(),
        name: "DP-1".to_string(),
        x: 2,
        y: 0,
        width: 2,
        height: 2,
        is_primary: true,
    };

    let seq2 = shm_buf
        .capture_monitor_rect_into_slot(&monitor, &root_shm, PixelLayout::Nv12, 84_000, &slot)
        .expect("capture X11 monitor into slot");
    assert_eq!(seq2, 2);
    assert_eq!(
        slot.peek_ipc_frame(|v| (v.width, v.height, v.layout, v.timestamp_us)),
        Some((2, 2, PixelLayout::Nv12, 84_000))
    );

    // Свёрнутое окно отклоняется и сохраняет предыдущий кадр монитора в слоте
    let minimized_win = X11WindowMetadata {
        is_minimized: true,
        ..win_meta
    };
    let res = capturer.capture_x11_shm_window_into_slot(
        &minimized_win,
        &shm_bytes,
        8,
        PixelLayout::Rgba,
        99_000,
        &slot,
    );
    assert!(matches!(res, Err(CaptureError::InvalidDimensions { .. })));
    assert_eq!(slot.rejected_frames(), 1);
    assert_eq!(slot.peek_ipc_frame(|v| v.timestamp_us), Some(84_000));

    // Проверим также capture_x11_shm_monitor_into_slot и capture_frame_into_slot на ScreenCapturer
    let seq3 = capturer
        .capture_x11_shm_monitor_into_slot(
            &monitor,
            4,
            2,
            &root_shm,
            16,
            PixelLayout::I420,
            100_000,
            &slot,
        )
        .expect("capture_x11_shm_monitor_into_slot");
    assert_eq!(seq3, 3);

    let seq4 = capturer
        .capture_frame_into_slot("screen:0", 8, 8, 116_666, &slot)
        .expect("capture_frame_into_slot");
    assert_eq!(seq4, 4);
    let expected_synth = capturer
        .capture_frame("screen:0", 8, 8, 116_666)
        .expect("capture_frame oracle");
    slot.with_latest_fresh(|actual| {
        assert_eq!(actual, &expected_synth);
        assert_eq!(
            actual
                .as_borrowed_view(4, true)
                .luma_plane()
                .map(<[u8]>::len),
            Some(64)
        );
    })
    .unwrap();
}

// ---------------------------------------------------------------------------
// 8. Многопоточный 60 FPS стресс-тест: задержка < 2 мс, без блокировки UI-потока
// ---------------------------------------------------------------------------

#[test]
fn concurrent_60fps_producer_and_non_blocking_ui_consumer_latency_under_2ms() {
    let slot = LatestFrameSlot::new();
    let writer_slot = slot.clone();
    let done = Arc::new(AtomicBool::new(false));
    let reader_done = done.clone();

    // Поток чтения (симуляция UI / Tauri IPC / Encoder) непрерывно опрашивает слот без блокировки
    let reader_handle = thread::spawn(move || {
        let mut last_seq = 0u64;
        let mut last_ts = 0u64;
        let mut reads = 0u64;
        let mut max_read_elapsed = Duration::ZERO;

        while !reader_done.load(Ordering::Acquire) {
            let t0 = Instant::now();
            if let Some((seq, ts, len)) =
                slot.with_ipc_frame(|view| (view.sequence, view.timestamp_us, view.byte_len()))
            {
                let elapsed = t0.elapsed();
                if elapsed > max_read_elapsed {
                    max_read_elapsed = elapsed;
                }
                assert!(
                    seq > last_seq,
                    "sequence must strictly increase: {seq} <= {last_seq}"
                );
                assert!(
                    ts > last_ts,
                    "timestamp must strictly increase: {ts} <= {last_ts}"
                );
                assert_eq!(len, 640 * 360 * 3 / 2);
                last_seq = seq;
                last_ts = ts;
                reads += 1;
            }
            thread::yield_now();
        }
        (reads, max_read_elapsed)
    });

    // Поток захвата записывает 120 кадров 640x360 I420 (345_600 байт) через пинг-понг буфер
    let frame_bytes = 640 * 360 * 3 / 2;
    let source_plane = vec![42u8; frame_bytes];
    for i in 1..=120u64 {
        let ts = i * 16_666;
        let t0 = Instant::now();
        let seq = writer_slot
            .store_with_reused_buffer(
                640,
                360,
                PixelLayout::I420,
                ColorSpace::BT601_LIMITED,
                0,
                ts,
                |buf| {
                    buf.copy_from_slice(&source_plane);
                    Ok::<(), &'static str>(())
                },
            )
            .expect("store_with_reused_buffer must succeed");
        let write_elapsed = t0.elapsed();
        assert_eq!(seq, i);
        assert!(
            write_elapsed < MAX_SLOT_LATENCY * 5,
            "write took {write_elapsed:?}"
        );
        if i % 4 == 0 {
            thread::yield_now();
        }
    }

    done.store(true, Ordering::Release);
    let (reads, max_read_elapsed) = reader_handle.join().expect("reader thread join");

    assert_eq!(writer_slot.stored_frames(), 120);
    assert_eq!(writer_slot.sequence(), 120);
    assert_eq!(writer_slot.rejected_frames(), 0);
    assert!(
        writer_slot.last_store_duration() < MAX_SLOT_LATENCY,
        "last store duration {:?} must be < 2ms",
        writer_slot.last_store_duration()
    );
    assert!(
        max_read_elapsed < MAX_SLOT_LATENCY,
        "UI zero-copy read duration {max_read_elapsed:?} must be < 2ms (reads={reads})"
    );
    // Суммарная память обоих буферов (slot + scratch) строго ограничена <= 2 кадрами
    assert!(
        writer_slot.allocated_capacity_bytes() <= frame_bytes * 2,
        "allocated capacity {} exceeds 2 frames ({})",
        writer_slot.allocated_capacity_bytes(),
        frame_bytes * 2
    );

    // Очистка слота освобождает всю память буферов
    writer_slot.clear();
    assert!(!writer_slot.has_frame());
    assert!(!writer_slot.is_fresh());
    assert_eq!(writer_slot.allocated_capacity_bytes(), 0);
}
