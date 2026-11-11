//! Контрактные тесты стенда Chaos Engineering (`TASK-905`):
//! - `SUB-905-1`: Сетевой хаос-инжектор (обрывы TCP каждые N сек с проверкой `RESUME Opcode 6`,
//!   инжекция HTTP 429 `retry_after_ms` и экспоненциального бэкоффа, 5%/15%/30% потерь UDP
//!   и джиттер 20–60 мс для проверки адаптивного битрейта Opus и PLC).
//! - `SUB-905-2`: Аппаратный и системный хаос (внезапное удаление активного аудиоустройства
//!   с переключением на Default, аварийный перезапуск демона PipeWire во время скриншаринга,
//!   инжекция OOM со сбросом `MediaCache`).
//! - `SUB-905-3`: Целостность состояния и живучесть (отсутствие дубликатов сообщений после серии
//!   реконнектов, сохранение черновика `draft_text` до 2000 символов при закрытии модалок,
//!   0 утечек файловых дескрипторов и сокетов при 100 последовательных циклах сбоя).

#![recursion_limit = "512"]
#![allow(dead_code, unused_imports)]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/chaos/mod.rs"]
mod chaos;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;

use chaos::{
    AudioDeviceChaosController, ChaosError, ChaosResilienceRunner, DraftIntegrityGuard,
    GatewayChaosInjector, Http429FaultInjector, OomChaosInjector, PipeWireChaosSupervisor,
    ReconnectMessageDeduplicator, ResourceLeakTracker, ResumeOpcode6Payload, TcpDisconnectKind,
    TcpDisconnectSchedule, UdpVoiceChaosInjector,
    hardware_faults::{
        DEFAULT_FALLBACK_AUDIO_INPUT, DEFAULT_FALLBACK_AUDIO_OUTPUT, MEMORY_BUDGET_LIMIT_BYTES,
    },
};
use media::MediaCache;
use state::{AppState, CallStatus, Message, MessageKind, UserSummary};
use video::{
    DRM_FORMAT_MOD_INVALID, DRM_FORMAT_MOD_LINEAR, PipeWireDmaBufConnector, VideoPixelFormat,
};

#[test]
fn test_sub_905_1_tcp_reset_and_eof_periodic_resume_opcode_6_with_seq_and_redacted_secrets() {
    // 1. Позитивный сценарий: обрывы TCP (SocketReset / UnexpectedEof) каждые 5 секунд на протяжении 25 секунд
    let schedule = TcpDisconnectSchedule::new(5, 25, TcpDisconnectKind::SocketReset)
        .expect("Валидное расписание обрывов TCP каждые 5 секунд");
    assert_eq!(schedule.expected_disconnect_count(), 5);

    let secret_token = "mfa.secret_live_token_do_not_leak_905";
    let secret_session = "sess_905_cafe_babe_dead_beef";
    let mut injector = GatewayChaosInjector::new(
        secret_session,
        secret_token,
        "wss://gateway-us-east1-b.discord.gg/?v=9&encoding=etf&compress=zstd-stream",
        100,
    );

    let cycles = injector
        .run_schedule(&schedule, 12)
        .expect("Плавный переход в RESUME (Opcode 6) при периодических обрывах TCP");
    assert_eq!(cycles.len(), 5);

    for (idx, cycle) in cycles.iter().enumerate() {
        assert_eq!(cycle.cycle_index, (idx + 1) as u32);
        assert_eq!(cycle.elapsed_secs, ((idx + 1) * 5) as u64);
        assert_eq!(cycle.resume_opcode, ResumeOpcode6Payload::OPCODE_RESUME);
        assert_eq!(cycle.resume_opcode, 6);
        assert!(cycle.session_id_preserved);
        assert!(cycle.used_resume_gateway_url);
        assert!(cycle.recovered);
        assert_eq!(
            cycle.post_resume_seq, cycle.pre_disconnect_seq,
            "После RESUME (Opcode 6) номер последовательности seq не должен теряться"
        );
    }
    assert_eq!(injector.current_seq(), 100 + 5 * 12);

    // 2. Проверка проводного формата Opcode 6 и маскирования секретов в логах/диагностике
    let payload = injector.build_resume_payload();
    let wire = payload.to_wire_json();
    assert_eq!(wire["op"], 6);
    assert_eq!(wire["d"]["token"], secret_token);
    assert_eq!(wire["d"]["session_id"], secret_session);
    assert_eq!(wire["d"]["seq"], 160);

    let redacted = payload.to_redacted_json();
    let redacted_str = redacted.to_string();
    assert_eq!(redacted["op"], 6);
    assert_eq!(redacted["d"]["token"], "[REDACTED]");
    assert_eq!(redacted["d"]["session_id"], "[REDACTED]");
    assert_eq!(redacted["d"]["seq"], 160);
    assert!(
        !redacted_str.contains(secret_token) && !redacted_str.contains(secret_session),
        "Утечка токена или session_id в отладочном JSON строго запрещена"
    );

    // 3. Граничные значения seq: 0 и u64::MAX
    let mut boundary_injector =
        GatewayChaosInjector::new(secret_session, secret_token, "wss://gateway.discord.gg", 0);
    let eof_schedule =
        TcpDisconnectSchedule::new(10, 10, TcpDisconnectKind::UnexpectedEof).unwrap();
    let eof_cycles = boundary_injector.run_schedule(&eof_schedule, 0).unwrap();
    assert_eq!(eof_cycles[0].pre_disconnect_seq, 0);
    assert_eq!(eof_cycles[0].post_resume_seq, 0);

    let mut max_seq_injector = GatewayChaosInjector::new(
        secret_session,
        secret_token,
        "wss://gateway.discord.gg",
        u64::MAX - 2,
    );
    let max_cycles = max_seq_injector.run_schedule(&eof_schedule, 5).unwrap();
    assert_eq!(max_cycles[0].pre_disconnect_seq, u64::MAX);
    assert_eq!(max_cycles[0].post_resume_seq, u64::MAX);

    // 4. Негативные сценарии: нулевой интервал и фатальный код закрытия 4004 Authentication Failed
    assert_eq!(
        TcpDisconnectSchedule::new(0, 30, TcpDisconnectKind::SocketReset),
        Err(ChaosError::InvalidIntervalSecs(0))
    );
    assert_eq!(
        TcpDisconnectSchedule::new(10, 5, TcpDisconnectKind::SocketReset),
        Err(ChaosError::InvalidIntervalSecs(10))
    );

    let fatal_schedule =
        TcpDisconnectSchedule::new(5, 5, TcpDisconnectKind::CloseCode(4004)).unwrap();
    assert_eq!(
        injector.run_schedule(&fatal_schedule, 1),
        Err(ChaosError::NonResumableCloseCode(4004))
    );

    // 5. InvalidSession(resumable=false): fallback на свежий IDENTIFY (Opcode 2) со сбросом seq в 0
    let inv_schedule =
        TcpDisconnectSchedule::new(5, 5, TcpDisconnectKind::InvalidSession { resumable: false })
            .unwrap();
    let inv_cycles = injector.run_schedule(&inv_schedule, 3).unwrap();
    assert_eq!(inv_cycles[0].resume_opcode, 2);
    assert!(!inv_cycles[0].session_id_preserved);
    assert_eq!(inv_cycles[0].post_resume_seq, 0);
    assert!(inv_cycles[0].recovered);
}

#[test]
fn test_sub_905_1_http_429_retry_after_and_exponential_backoff_contract() {
    // 1. Серия искусственных HTTP 429 со случайными/заданными retry_after_ms (включая потолок 60_000 мс)
    let series_ms = [250, 1_200, 4_500, 15_000, 60_000, 120_000];
    let outcomes = Http429FaultInjector::run_429_burst_series(&series_ms)
        .expect("Обработка серии HTTP 429 Too Many Requests");
    assert_eq!(outcomes.len(), series_ms.len());

    assert_eq!(outcomes[0].computed_backoff_ms, Some(250));
    assert_eq!(outcomes[1].computed_backoff_ms, Some(1_200));
    assert_eq!(outcomes[2].computed_backoff_ms, Some(4_500));
    assert_eq!(outcomes[3].computed_backoff_ms, Some(15_000));
    assert_eq!(outcomes[4].computed_backoff_ms, Some(60_000));
    // Потолок оракула /home/sanya/re-deep/ (new a.A(1e3, 6e4)): 120_000 мс зажимается до 60_000 мс
    assert_eq!(outcomes[5].computed_backoff_ms, Some(60_000));
    assert!(outcomes.iter().all(|o| o.will_retry && o.status == 429));

    // 2. HTTP 429 без заголовка Retry-After использует экспоненциальный бэкофф: 150 -> 300 -> 600 мс
    let fallback_1 = Http429FaultInjector::evaluate_status_backoff(429, None, 1).unwrap();
    let fallback_2 = Http429FaultInjector::evaluate_status_backoff(429, None, 2).unwrap();
    let fallback_3 = Http429FaultInjector::evaluate_status_backoff(429, None, 3).unwrap();
    assert_eq!(fallback_1.computed_backoff_ms, Some(150));
    assert_eq!(fallback_2.computed_backoff_ms, Some(300));
    assert_eq!(fallback_3.computed_backoff_ms, Some(600));

    // 3. Ошибки 5xx (500, 502, 503, 504) используют экспоненциальный бэкофф, а 401/403/404/413 не ретраятся
    let err_502 = Http429FaultInjector::evaluate_status_backoff(502, None, 2).unwrap();
    assert!(err_502.will_retry);
    assert_eq!(err_502.computed_backoff_ms, Some(300));

    for fatal_status in [400, 401, 403, 404, 413] {
        let out = Http429FaultInjector::evaluate_status_backoff(fatal_status, None, 1).unwrap();
        assert!(
            !out.will_retry && out.computed_backoff_ms.is_none(),
            "HTTP статус {fatal_status} не должен вызывать повторных попыток"
        );
    }

    // 4. Детерминированный генератор джиттера retry_after_ms в диапазоне
    let d1 = Http429FaultInjector::deterministic_jitter_retry_after_ms(42, 200, 5_000);
    let d2 = Http429FaultInjector::deterministic_jitter_retry_after_ms(42, 200, 5_000);
    assert_eq!(d1, d2);
    assert!(d1.as_millis() >= 200 && d1.as_millis() <= 5_000);
}

#[test]
fn test_sub_905_1_udp_packet_loss_5_15_30_percent_opus_adaptive_bitrate_and_plc_jitter_20_60ms() {
    // 1. Профиль 5% потерь UDP и джиттер 20 мс (умеренные потери: битрейт снижается с 64 до 56 кбит/с)
    let rep_5 = UdpVoiceChaosInjector::simulate_udp_loss_and_jitter(5, 20, 100, 1_000)
        .expect("Симуляция 5% потерь UDP и джиттера 20 мс");
    assert_eq!(rep_5.loss_percent, 5);
    assert_eq!(rep_5.rtcp_fraction_lost, 13); // round(5% * 256) = 13
    assert_eq!(rep_5.total_packets_sent, 100);
    assert_eq!(rep_5.packets_dropped, 5);
    assert_eq!(rep_5.packets_delivered, 95);
    assert_eq!(rep_5.initial_bitrate_bps, 64_000);
    assert!(
        rep_5.adapted_bitrate_bps <= 56_000 && rep_5.adapted_bitrate_bps >= 32_000,
        "При 5% потерь битрейт Opus должен снизиться с 64 кбит/с (получено {})",
        rep_5.adapted_bitrate_bps
    );
    assert_eq!(rep_5.jitter_buffer_target_ms, 40);
    assert_eq!(rep_5.plc_frames_generated, 5);
    assert_eq!(rep_5.recovered_bitrate_bps, 64_000);

    // 2. Профиль 15% потерь UDP и джиттер 40 мс (высокие потери: быстрое снижение битрейта и глубина буфера 60 мс)
    let rep_15 = UdpVoiceChaosInjector::simulate_udp_loss_and_jitter(15, 40, 100, 2_000)
        .expect("Симуляция 15% потерь UDP и джиттера 40 мс");
    assert_eq!(rep_15.loss_percent, 15);
    assert!(rep_15.rtcp_fraction_lost >= 26); // 38 >= RTCP_LOSS_HIGH_THRESHOLD (26)
    assert_eq!(rep_15.packets_dropped, 15);
    assert_eq!(rep_15.packets_delivered, 85);
    assert!(rep_15.adapted_bitrate_bps <= 48_000);
    assert_eq!(rep_15.jitter_buffer_target_ms, 60);
    assert_eq!(rep_15.plc_frames_generated, 15);
    assert_eq!(rep_15.recovered_bitrate_bps, 64_000);

    // 3. Профиль 30% потерь UDP на границе переполнения u16::MAX (65_500.. -> wrap-around через 65_535 -> 0)
    let rep_30 = UdpVoiceChaosInjector::simulate_udp_loss_and_jitter(30, 60, 100, 65_500)
        .expect("Симуляция 30% потерь UDP на границе u16::MAX");
    assert_eq!(rep_30.loss_percent, 30);
    assert_eq!(rep_30.packets_dropped, 30);
    assert_eq!(rep_30.packets_delivered, 70);
    assert_eq!(
        rep_30.adapted_bitrate_bps, 32_000,
        "При затяжных 30% потерях битрейт Opus опускается до минимального порога 32 кбит/с"
    );
    assert_eq!(rep_30.jitter_buffer_target_ms, 60);
    assert_eq!(rep_30.plc_frames_generated, 30);
    assert_eq!(rep_30.recovered_bitrate_bps, 64_000);

    // 4. Негативные граничные значения потерь (> 100%) и джиттера (< 20 мс или > 60 мс)
    assert_eq!(
        UdpVoiceChaosInjector::loss_percent_to_fraction_lost(101),
        Err(ChaosError::InvalidLossPercent(101))
    );
    assert_eq!(
        UdpVoiceChaosInjector::simulate_udp_loss_and_jitter(15, 10, 50, 0),
        Err(ChaosError::InvalidJitterMs(10))
    );
    assert_eq!(
        UdpVoiceChaosInjector::simulate_udp_loss_and_jitter(15, 80, 50, 0),
        Err(ChaosError::InvalidJitterMs(80))
    );
}

#[test]
fn test_sub_905_2_audio_device_hot_unplug_during_call_switches_to_default() {
    let mut state = AppState::new();
    state.connect_voice(
        "vc-general".into(),
        "General Voice".into(),
        "Rust Guild".into(),
    );
    state.start_call("u-peer".into(), true);
    state.connect_active_call();
    assert!(matches!(state.active_call, CallStatus::Connected { .. }));

    // Выбираем внешнюю USB-гарнитуру как активное устройство ввода и вывода
    state.available_audio_inputs = vec![
        "Default ALSA Microphone".into(),
        "USB Studio Condenser Mic".into(),
    ];
    state.available_audio_outputs = vec![
        "Default PulseAudio Speakers".into(),
        "USB Gaming Headset Output".into(),
    ];
    state.select_audio_input("USB Studio Condenser Mic".into());
    state.select_audio_output("USB Gaming Headset Output".into());

    // 1. Внезапно отключаем USB-микрофон во время активного звонка
    let in_outcome = AudioDeviceChaosController::unplug_active_input_device(&mut state)
        .expect("Автоматическое переключение микрофона на Default");
    assert_eq!(in_outcome.removed_device, "USB Studio Condenser Mic");
    assert_eq!(in_outcome.fallback_device, "Default ALSA Microphone");
    assert!(in_outcome.is_input);
    assert!(in_outcome.call_remained_active);
    assert!(in_outcome.voice_session_preserved);
    assert_eq!(
        state.selected_audio_input.as_deref(),
        Some("Default ALSA Microphone")
    );
    assert!(matches!(state.active_call, CallStatus::Connected { .. }));

    // 2. Внезапно отключаем и последнее физическое устройство ввода (пустой список -> системный Fallback)
    let empty_in_outcome = AudioDeviceChaosController::unplug_active_input_device(&mut state)
        .expect("Переключение на виртуальный системный микрофон при пустом списке");
    assert_eq!(
        empty_in_outcome.fallback_device,
        DEFAULT_FALLBACK_AUDIO_INPUT
    );
    assert_eq!(
        state.selected_audio_input.as_deref(),
        Some(DEFAULT_FALLBACK_AUDIO_INPUT)
    );

    // 3. Внезапно отключаем наушники вывода во время звонка
    let out_outcome = AudioDeviceChaosController::unplug_active_output_device(&mut state)
        .expect("Автоматическое переключение вывода на Default");
    assert_eq!(out_outcome.removed_device, "USB Gaming Headset Output");
    assert_eq!(out_outcome.fallback_device, "Default PulseAudio Speakers");
    assert!(!out_outcome.is_input);
    assert!(out_outcome.call_remained_active);
    assert!(out_outcome.voice_session_preserved);

    // 4. Отключаем последнее устройство вывода -> виртуальный системный вывод
    let empty_out_outcome = AudioDeviceChaosController::unplug_active_output_device(&mut state)
        .expect("Переключение на виртуальный системный вывод при пустом списке");
    assert_eq!(
        empty_out_outcome.fallback_device,
        DEFAULT_FALLBACK_AUDIO_OUTPUT
    );
}

#[test]
fn test_sub_905_2_pipewire_daemon_crash_and_restart_during_active_screen_share() {
    let mut state = AppState::new();
    state.start_screen_sharing("screen:0".into());
    assert!(state.is_screen_sharing);

    let mut connector = PipeWireDmaBufConnector::new_60fps(3);

    // 1. Перезапуск демона PipeWire с сохранением аппаратного DMA-BUF (DRM_FORMAT_MOD_LINEAR)
    let recovery_hw = PipeWireChaosSupervisor::simulate_daemon_restart(
        &mut state,
        &mut connector,
        41,
        101,
        42,
        202,
        1920,
        1080,
        VideoPixelFormat::Bgra8,
        DRM_FORMAT_MOD_LINEAR,
    )
    .expect("Восстановление трансляции экрана после перезапуска PipeWire (DMA-BUF)");

    assert_eq!(recovery_hw.previous_fd, 41);
    assert_eq!(recovery_hw.new_fd, 42);
    assert_eq!(recovery_hw.previous_node_id, 101);
    assert_eq!(recovery_hw.new_node_id, 202);
    assert!(recovery_hw.screen_share_remained_active);
    assert!(recovery_hw.post_recovery_frames_accepted >= 2);
    assert!(!recovery_hw.used_cpu_shm_fallback);
    assert!(state.is_screen_sharing);
    assert_eq!(connector.metrics().active_node_id, Some(202));

    // 2. Перезапуск демона PipeWire при отказе GPU-модификатора (DRM_FORMAT_MOD_INVALID -> Shm/CPU fallback)
    let recovery_shm = PipeWireChaosSupervisor::simulate_daemon_restart(
        &mut state,
        &mut connector,
        42,
        202,
        43,
        303,
        1280,
        720,
        VideoPixelFormat::Rgba8,
        DRM_FORMAT_MOD_INVALID,
    )
    .expect("Восстановление трансляции экрана через CPU/Shm fallback после рестарта PipeWire");

    assert!(recovery_shm.screen_share_remained_active);
    assert!(recovery_shm.used_cpu_shm_fallback);
    assert!(recovery_shm.post_recovery_frames_accepted >= 2);
    assert_eq!(connector.metrics().active_node_id, Some(303));

    // 3. Негативные проверки: невалидный файловый дескриптор (< 0) или нулевой node_id
    assert_eq!(
        PipeWireChaosSupervisor::simulate_daemon_restart(
            &mut state,
            &mut connector,
            43,
            303,
            -1,
            404,
            1280,
            720,
            VideoPixelFormat::Bgra8,
            DRM_FORMAT_MOD_LINEAR,
        ),
        Err(ChaosError::InvalidPipeWireFd(-1))
    );
    assert_eq!(
        PipeWireChaosSupervisor::simulate_daemon_restart(
            &mut state,
            &mut connector,
            43,
            303,
            44,
            0,
            1280,
            720,
            VideoPixelFormat::Bgra8,
            DRM_FORMAT_MOD_LINEAR,
        ),
        Err(ChaosError::InvalidPipeWireNodeId(0))
    );
}

#[test]
fn test_sub_905_2_oom_injection_purges_media_cache_within_60mb_budget() {
    let mut cache = MediaCache::new();
    cache.put("att-1", vec![0xAA; 128 * 1024]);
    cache.put("att-2", vec![0xBB; 256 * 1024]);
    cache.put("att-3", vec![0xCC; 512 * 1024]);
    assert_eq!(cache.len(), 3);

    // Симулируем скачок памяти до 72 МБ (> 60 МБ лимита) и инжектируем OOM
    let simulated_bytes = 72 * 1024 * 1024;
    let outcome = OomChaosInjector::inject_oom_and_purge_cache(&mut cache, simulated_bytes)
        .expect("OOM-сброс кэша изображений MediaCache");

    assert_eq!(outcome.cached_items_before, 3);
    assert_eq!(outcome.cached_bytes_before, simulated_bytes);
    assert_eq!(outcome.cached_items_after, 0);
    assert_eq!(outcome.cached_bytes_after, 0);
    assert!(outcome.within_60mb_budget);
    assert_eq!(cache.len(), 0);
    assert!(outcome.cached_bytes_after < MEMORY_BUDGET_LIMIT_BYTES);
}

#[test]
fn test_sub_905_3_message_deduplication_and_draft_preservation_across_reconnects_and_modal_storm() {
    let mut state = AppState::new();
    let channel_id = "c-chaos-general";

    let make_msg = |id: &str, text: &str| Message {
        id: id.to_string(),
        channel_id: channel_id.to_string(),
        author: UserSummary {
            id: "u-1".into(),
            username: "ChaosBot".into(),
            global_name: Some("ChaosBot".into()),
            avatar: None,
            is_bot: true,
        },
        content: text.to_string(),
        timestamp: "2026-11-11T12:00:00Z".into(),
        edited_timestamp: None,
        attachments: vec![],
        reactions: vec![],
        is_pinned: false,
        message_kind: MessageKind::Default,
    };

    // Включаем граничные Snowflake ID ("0" и u64::MAX = "18446744073709551615")
    let batch_1 = vec![
        make_msg("0", "Genesis boundary snowflake"),
        make_msg("1001", "First message before TCP drop"),
        make_msg("1002", "Second message before TCP drop"),
    ];
    let batch_2_replay = vec![
        make_msg("1001", "First message before TCP drop"),
        make_msg("1002", "Second message before TCP drop"),
        make_msg("18446744073709551615", "Max u64 boundary snowflake"),
    ];

    let mut dedup = ReconnectMessageDeduplicator::new();
    let inserted_1 = dedup
        .ingest_gateway_batch(&mut state, channel_id, &batch_1)
        .unwrap();
    let inserted_2 = dedup
        .ingest_gateway_batch(&mut state, channel_id, &batch_2_replay)
        .unwrap();

    assert_eq!(inserted_1, 3);
    assert_eq!(inserted_2, 1);
    assert_eq!(dedup.duplicates_suppressed(), 2);

    let unique_count =
        ReconnectMessageDeduplicator::verify_no_duplicates(&state, channel_id).unwrap();
    assert_eq!(unique_count, 4);

    // Проверка сохранения черновика сообщения (включая граничную длину 2000 символов) при шторме модалок
    let draft_2000 = "Д".repeat(2000);
    assert_eq!(draft_2000.chars().count(), 2000);
    let preserved = DraftIntegrityGuard::verify_draft_survives_modal_storm(&mut state, &draft_2000)
        .expect("Черновик 2000 символов обязан сохраниться после закрытия всех модалок");
    assert_eq!(preserved, draft_2000);
    assert_eq!(state.draft_text, draft_2000);
}

#[test]
fn test_sub_905_3_hundred_consecutive_fault_cycles_zero_fd_and_socket_leaks() {
    let report = ChaosResilienceRunner::run_consecutive_fault_cycles(100)
        .expect("100 последовательных циклов Chaos Engineering должны завершиться без ошибок");

    assert_eq!(report.cycles_completed, 100);
    assert!(report.passed);
    assert_eq!(
        report.leaked_fds, 0,
        "Утечки файловых дескрипторов недопустимы"
    );
    assert_eq!(report.leaked_sockets, 0, "Утечки сокетов недопустимы");
    assert!(report.redacted_credentials_verified);

    assert_eq!(report.network.tcp_disconnects_injected, 100);
    assert_eq!(report.network.resume_opcode6_verified, 100);
    assert_eq!(report.network.http_429_injected, 100);
    assert_eq!(report.network.udp_loss_profiles_verified, vec![5, 15, 30]);
    assert_eq!(report.network.min_opus_bitrate_bps, 32_000);
    assert_eq!(report.network.max_opus_bitrate_bps, 64_000);
    assert!(report.network.total_plc_frames > 0);

    assert_eq!(report.hardware.audio_input_fallbacks, 100);
    assert_eq!(report.hardware.audio_output_fallbacks, 100);
    assert_eq!(report.hardware.pipewire_restarts_recovered, 100);
    assert_eq!(report.hardware.oom_cache_evictions, 100);
    assert_eq!(report.hardware.max_observed_cache_bytes_after_oom, 0);

    assert_eq!(
        report.integrity.fds_allocated_total,
        report.integrity.fds_released_total
    );
    assert_eq!(
        report.integrity.sockets_allocated_total,
        report.integrity.sockets_released_total
    );
    assert_eq!(report.integrity.draft_preserved_chars, 2000);

    let json = report
        .to_json_pretty()
        .expect("Сериализация ChaosAuditReport в JSON");
    assert!(json.contains("\"cycles_completed\": 100"));
    assert!(json.contains("\"leaked_fds\": 0"));
    assert!(json.contains("\"leaked_sockets\": 0"));

    // Негативная проверка детектора утечек ресурсов
    let mut leak_tracker = ResourceLeakTracker::new();
    leak_tracker.allocate_fd(900);
    leak_tracker.allocate_socket(5001);
    assert_eq!(
        leak_tracker.verify_zero_leaks(),
        Err(ChaosError::ResourceLeakDetected {
            leaked_fds: 1,
            leaked_sockets: 1,
        })
    );
    assert!(leak_tracker.release_fd(900));
    assert!(leak_tracker.release_socket(5001));
    assert!(leak_tracker.verify_zero_leaks().is_ok());
}
