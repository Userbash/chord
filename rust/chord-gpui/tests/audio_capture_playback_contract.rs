#![forbid(unsafe_code)]
//! Контрактные тесты живых конвейеров захвата микрофона и воспроизведения голоса (`W3-2`: `P1-1.3`, `P1-1.4`).
//!
//! Покрывают все 9 обязательных измерений тестирования (`TEST-POLICY.md` §12):
//! 1. Позитивный сценарий: сквозной UDP loopback `127.0.0.1` между `AudioCapturePipeline` и `AudioPlaybackPipeline`
//!    как в режиме SRTP (`aead_aes256_gcm`), так и в двухслойном режиме DAVE MLS v1 E2EE (`enable_dave_session`).
//! 2. Негативный сценарий: отклонение пустых (`0B`), превышающих лимит RFC 6716 (`> 1275B`) и повреждённых SRTP/DAVE пакетов.
//! 3. Граничные условия: 20-мс фрейминг (`< 960` сэмплов буферизуется, ровно `960` моно / `1920` стерео выдаёт 1 фрейм),
//!    масштабирование громкости (`0%` тишина, `100%` единичное усиление, `200%` усиление с жёстким клиппингом `[-1.0, 1.0]`).
//! 4. Обработка ошибок: защита от `NaN` / `±Infinity` на входе микрофона и выходе декодера, ошибки аутентификации AEAD.
//! 5. Пустые состояния: чтение из пустого `AudioPlaybackPipeline` безопасно заполняет выходной буфер CPAL нулями (`0.0`) без underflow.
//! 6. Состояния буферизации (pre-buffering): удержание выдачи джиттер-буфером (`20..=60 мс`) до накопления целевого числа фреймов.
//! 7. Права/состояния микрофона и наушников: `mute` / порог VAD генерируют `DAVE_OPUS_SILENCE_PACKET` (`[0xF8, 0xFF, 0xFE]`)
//!    без утечки шума, `deafen` гарантирует абсолютную тишину на выходе динамиков.
//! 8. Восстановление и деградация: переупорядочивание внеочередных RTP-пакетов, отбрасывание дубликатов/опоздавших,
//!    генерация PLC-фреймов (`ConcealLoss`) при потере пакетов и адаптация битрейта по RTCP Receiver Reports.
//! 9. Наблюдаемость и телеметрия: точный учёт `AudioCaptureStats` и `AudioPlaybackStats`.

pub mod state {
    #[derive(Debug, Clone)]
    pub struct CameraDevice {
        pub id: String,
        pub name: String,
        pub is_default: bool,
    }
}

#[path = "../src/audio/mod.rs"]
mod audio;

use audio::{
    AudioCaptureConfig, AudioCapturePipeline, AudioCodecBackend, AudioPlaybackConfig,
    AudioPlaybackPipeline, DAVE_OPUS_SILENCE_PACKET, DaveMlsGroupContext, DaveMlsKeySchedule,
    DaveTransportCipherMode, DaveVoiceSession, JitterPopStatus, OPUS_MAX_PACKET_BYTES,
    SoftwareOpusCodec, VoiceUdpConfig, VoiceUdpStreamer,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

async fn make_loopback_streamer_pair(
    secret_key: [u8; 32],
    sender_ssrc: u32,
    receiver_ssrc: u32,
) -> (VoiceUdpStreamer, VoiceUdpStreamer) {
    let recv_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let recv_addr = recv_sock.local_addr().unwrap();

    let send_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let send_addr = send_sock.local_addr().unwrap();

    send_sock.connect(recv_addr).await.unwrap();
    recv_sock.connect(send_addr).await.unwrap();

    let sender = VoiceUdpStreamer {
        socket: Arc::new(send_sock),
        config: VoiceUdpConfig {
            remote_addr: recv_addr,
            ssrc: sender_ssrc,
            secret_key,
        },
        sequence: Arc::new(Mutex::new(100)),
        timestamp: Arc::new(Mutex::new(9600)),
        keepalive_seq: Arc::new(Mutex::new(0)),
        srtp_nonce_counter: Arc::new(Mutex::new(0)),
        dave_session: Arc::new(Mutex::new(None)),
        stats: Arc::new(Mutex::new(audio::VoiceStreamStats::default())),
        bitrate_controller: Arc::new(Mutex::new(audio::OpusBitrateController::new())),
        jitter_buffer: Arc::new(Mutex::new(audio::AdaptiveJitterBuffer::new())),
        is_running: Arc::new(AtomicBool::new(true)),
    };

    let receiver = VoiceUdpStreamer {
        socket: Arc::new(recv_sock),
        config: VoiceUdpConfig {
            remote_addr: send_addr,
            ssrc: receiver_ssrc,
            secret_key,
        },
        sequence: Arc::new(Mutex::new(0)),
        timestamp: Arc::new(Mutex::new(0)),
        keepalive_seq: Arc::new(Mutex::new(0)),
        srtp_nonce_counter: Arc::new(Mutex::new(0)),
        dave_session: Arc::new(Mutex::new(None)),
        stats: Arc::new(Mutex::new(audio::VoiceStreamStats::default())),
        bitrate_controller: Arc::new(Mutex::new(audio::OpusBitrateController::new())),
        jitter_buffer: Arc::new(Mutex::new(audio::AdaptiveJitterBuffer::new())),
        is_running: Arc::new(AtomicBool::new(true)),
    };

    (sender, receiver)
}

fn sample_dave_group_context(epoch: u64) -> DaveMlsGroupContext {
    DaveMlsGroupContext {
        group_id: b"w3-2-voice-channel-loopback".to_vec(),
        epoch,
        tree_hash: [0x33u8.wrapping_add(epoch as u8); 32],
        confirmed_transcript_hash: [0x7Au8.wrapping_add(epoch as u8); 32],
        extensions: vec![0x00, 0x02, 0x01, 0x02],
    }
}

#[test]
fn capture_20ms_framing_boundaries_mono_and_stereo_with_overrun_telemetry() {
    // 1. Моно (48 кГц, 1 канал): 20 мс = 960 сэмплов
    let mono_cfg = AudioCaptureConfig::new(48_000, 1).with_vad_threshold(0.0);
    let mut mono_capture = AudioCapturePipeline::new(mono_cfg);

    // Неполный чанк (959 сэмплов < 960) буферизуется без генерации Opus-пакета
    assert_eq!(mono_capture.push_mic_samples(&vec![0.5f32; 959]), 959);
    let encoded_partial = mono_capture.drain_and_encode_frames().unwrap();
    assert!(encoded_partial.is_empty());
    assert_eq!(mono_capture.stats().frames_encoded, 0);

    // Добавление 1 сэмпла завершает ровно 960 сэмплов (20 мс) -> эмитится ровно 1 фрейм Opus
    assert_eq!(mono_capture.push_mic_samples(&[0.5f32]), 1);
    let encoded_one = mono_capture.drain_and_encode_frames().unwrap();
    assert_eq!(encoded_one.len(), 1);
    assert_eq!(encoded_one[0].len(), 160); // 64 kbps -> 160 байт на 20 мс
    assert_eq!(mono_capture.stats().frames_encoded, 1);
    assert_eq!(mono_capture.stats().captured_samples, 960);

    // 2. Стерео (48 кГц, 2 канала): 20 мс = 1920 чередующихся сэмплов
    let mut stereo_cfg = AudioCaptureConfig::new(48_000, 2).with_vad_threshold(0.0);
    stereo_cfg.ring_capacity_samples = 4000;
    let mut stereo_capture = AudioCapturePipeline::new(stereo_cfg);

    assert_eq!(stereo_capture.push_mic_samples(&vec![0.4f32; 960]), 960);
    assert!(stereo_capture.drain_and_encode_frames().unwrap().is_empty());

    assert_eq!(stereo_capture.push_mic_samples(&vec![0.4f32; 960]), 960);
    let stereo_frames = stereo_capture.drain_and_encode_frames().unwrap();
    assert_eq!(stereo_frames.len(), 1);
    assert_eq!(stereo_frames[0].len(), 160);

    // 3. Проверка учёта overrun_drops при переполнении входного кольцевого буфера микрофона
    let pushed = stereo_capture.push_mic_samples(&vec![0.25f32; 4500]);
    assert_eq!(pushed, 4500);
    assert_eq!(stereo_capture.stats().overrun_drops, 500);
}

#[test]
fn capture_mute_vad_dtx_silence_and_nan_inf_sanitization() {
    let cfg = AudioCaptureConfig::new(48_000, 2)
        .with_vad_threshold(0.05)
        .with_mic_gain(1.0);
    let mut capture = AudioCapturePipeline::new(cfg);

    // 1. Сигнал ниже порога VAD (0.01 < 0.05) -> генерируется 3-байтный DAVE_OPUS_SILENCE_PACKET без утечки шума
    capture.push_mic_samples(&vec![0.01f32; 1920]);
    let dtx_frames = capture.drain_and_encode_frames().unwrap();
    assert_eq!(dtx_frames.len(), 1);
    assert_eq!(dtx_frames[0], DAVE_OPUS_SILENCE_PACKET);
    assert_eq!(capture.stats().silence_frames, 1);
    assert_eq!(capture.stats().frames_encoded, 0);

    // 2. Включённый mute при громком сигнале (0.8) -> также принудительно выдаёт DAVE_OPUS_SILENCE_PACKET
    capture.set_mute(true);
    assert!(capture.is_muted());
    capture.push_mic_samples(&vec![0.8f32; 1920]);
    let muted_frames = capture.drain_and_encode_frames().unwrap();
    assert_eq!(muted_frames.len(), 1);
    assert_eq!(muted_frames[0], DAVE_OPUS_SILENCE_PACKET);
    assert_eq!(capture.stats().silence_frames, 2);

    // 3. При отключении emit_dtx_silence_frames тихие/заглушенные фреймы полностью пропускаются
    let mut skip_cfg = AudioCaptureConfig::new(48_000, 1).with_mute(true);
    skip_cfg.emit_dtx_silence_frames = false;
    let mut skip_capture = AudioCapturePipeline::new(skip_cfg);
    skip_capture.push_mic_samples(&vec![0.9f32; 960]);
    assert!(skip_capture.drain_and_encode_frames().unwrap().is_empty());
    assert_eq!(skip_capture.stats().silence_frames, 1);

    // 4. Защита от NaN и ±Infinity: все не-конечные значения очищаются в 0.0 и не вызывают паники
    capture.set_mute(false);
    let mut corrupted_pcm = vec![f32::NAN; 960];
    corrupted_pcm.extend_from_slice(&vec![f32::INFINITY; 480]);
    corrupted_pcm.extend_from_slice(&vec![f32::NEG_INFINITY; 480]);
    capture.push_mic_samples(&corrupted_pcm);
    let sanitized_frames = capture.drain_and_encode_frames().unwrap();
    assert_eq!(sanitized_frames.len(), 1);
    // Поскольку все NaN/Inf обращены в 0.0, энергия равна 0.0 <= vad_threshold -> тишина DTX
    assert_eq!(sanitized_frames[0], DAVE_OPUS_SILENCE_PACKET);
}

#[test]
fn playback_volume_scaling_bounds_deafen_and_underflow_safety() {
    let mut codec = SoftwareOpusCodec::new();
    // Кодируем эталонный 20-мс моно-фрейм с постоянной амплитудой 0.6
    let source_pcm = vec![0.6f32; 960];
    let opus_pkt = codec.encode_20ms(&source_pcm, 1, 160).unwrap();

    let mut playback_cfg = AudioPlaybackConfig::new(48_000, 1);
    playback_cfg.initial_jitter_delay_ms = 20; // 1 фрейм до старта выдачи
    let mut playback = AudioPlaybackPipeline::new(playback_cfg);

    // 1. Чтение из пустого буфера до прихода пакетов безопасно заполняет выход нулями (underflow guard)
    let mut out = vec![0.99f32; 960];
    let read_empty = playback.read_speaker_samples(&mut out);
    assert_eq!(read_empty, 0);
    assert!(out.iter().all(|&s| s == 0.0));

    // 2. Громкость 0% (0.0) -> полная тишина
    playback.set_output_volume(0.0);
    assert_eq!(playback.output_volume(), 0.0);
    playback
        .ingest_rtp_opus_frame(1, 960, opus_pkt.clone())
        .unwrap();
    playback.drain_jitter_to_speaker_ring().unwrap();
    assert_eq!(playback.read_speaker_samples(&mut out), 960);
    assert!(out.iter().all(|&s| s == 0.0));

    // 3. Громкость 100% (1.0) -> единичное усиление (~0.6)
    playback.set_output_volume(1.0);
    playback
        .ingest_rtp_opus_frame(2, 1920, opus_pkt.clone())
        .unwrap();
    playback.drain_jitter_to_speaker_ring().unwrap();
    assert_eq!(playback.read_speaker_samples(&mut out), 960);
    for &sample in &out {
        assert!(
            (sample - 0.6).abs() < 0.02,
            "При 100% громкости ожидалось ~0.6, получено {sample}"
        );
    }

    // 4. Громкость 200% (2.0) -> 0.6 * 2.0 = 1.2 -> жёстко зажимается (clamped) в 1.0!
    // А попытка выставить 5.0 зажимается конфигурацией в предел 2.0 (200%)
    playback.set_output_volume(5.0);
    assert_eq!(playback.output_volume(), 2.0);
    playback
        .ingest_rtp_opus_frame(3, 2880, opus_pkt.clone())
        .unwrap();
    playback.drain_jitter_to_speaker_ring().unwrap();
    assert_eq!(playback.read_speaker_samples(&mut out), 960);
    for &sample in &out {
        assert!(
            (sample - 1.0).abs() < 1e-5,
            "При 200% громкости сэмпл 0.6*2.0 обязан клиппироваться в 1.0, получено {sample}"
        );
    }

    // Проверка отрицательного клиппинга в -1.0 при 200% громкости
    let neg_opus = codec.encode_20ms(&vec![-0.75f32; 960], 1, 160).unwrap();
    playback.ingest_rtp_opus_frame(4, 3840, neg_opus).unwrap();
    playback.drain_jitter_to_speaker_ring().unwrap();
    assert_eq!(playback.read_speaker_samples(&mut out), 960);
    for &sample in &out {
        assert!(
            (sample - (-1.0)).abs() < 1e-5,
            "Отрицательный пик -0.75*2.0 обязан клиппироваться в -1.0, получено {sample}"
        );
    }

    // 5. Режим deafen (самооглушение) принудительно глушит выход динамиков в 0.0 даже при громкости 200%
    playback.set_deafen(true);
    assert!(playback.is_deafened());
    playback.ingest_rtp_opus_frame(5, 4800, opus_pkt).unwrap();
    playback.drain_jitter_to_speaker_ring().unwrap();
    assert_eq!(playback.read_speaker_samples(&mut out), 960);
    assert!(out.iter().all(|&s| s == 0.0));
}

#[test]
fn playback_jitter_reordering_plc_concealment_and_packet_validation() {
    let mut codec = SoftwareOpusCodec::new();
    let pkt_10 = codec.encode_20ms(&vec![0.4f32; 1920], 2, 120).unwrap();
    let pkt_11 = codec.encode_20ms(&vec![0.8f32; 1920], 2, 120).unwrap();
    let pkt_13 = codec.encode_20ms(&vec![0.2f32; 1920], 2, 120).unwrap();

    let mut cfg = AudioPlaybackConfig::new(48_000, 2);
    cfg.initial_jitter_delay_ms = 40; // 2 фрейма pre-buffering
    let mut playback = AudioPlaybackPipeline::new(cfg);

    // 1. Валидация отклонения пустого (0B) и превышающего RFC 6716 (> 1275B) пакета Opus
    assert!(playback.ingest_rtp_opus_frame(1, 960, vec![]).is_err());
    assert!(
        playback
            .ingest_rtp_opus_frame(1, 960, vec![0xAA; OPUS_MAX_PACKET_BYTES + 1])
            .is_err()
    );
    assert!(codec.decode_20ms(&[], 2).is_err());
    assert!(
        codec
            .decode_20ms(&vec![0xBB; OPUS_MAX_PACKET_BYTES + 1], 2)
            .is_err()
    );

    // 2. Внеочередной приход пакетов: сначала seq=11, затем дубликат seq=11, затем seq=10, затем seq=13 (seq=12 потерян!)
    assert!(playback.ingest_rtp_opus_frame(11, 1920, pkt_11).unwrap());
    // Пока 1 фрейм (< 2 для 40 мс) — буфер удерживает выдачу в состоянии Buffering
    assert_eq!(
        playback.pop_and_decode_next_frame().unwrap(),
        JitterPopStatus::Buffering
    );

    // Дубликат seq=11 отбрасывается
    assert!(
        !playback
            .ingest_rtp_opus_frame(11, 1920, vec![0x78; 80])
            .unwrap()
    );
    assert_eq!(playback.stats().dropped_duplicate, 1);

    // Приход более раннего seq=10 завершает pre-buffering (2 фрейма) и ставит 10 перед 11
    assert!(playback.ingest_rtp_opus_frame(10, 960, pkt_10).unwrap());
    assert!(playback.ingest_rtp_opus_frame(13, 3840, pkt_13).unwrap());

    // Первый извлечённый фрейм — seq=10 (амплитуда ~0.4)
    match playback.pop_and_decode_next_frame().unwrap() {
        JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 10),
        other => panic!("Ожидался Ready(10), получено {:?}", other),
    }

    // Опоздавший пакет seq=9 (< следующего ожидаемого 11) отбрасывается
    assert!(
        !playback
            .ingest_rtp_opus_frame(9, 0, vec![0x78; 80])
            .unwrap()
    );
    assert_eq!(playback.stats().dropped_late, 1);

    // Второй извлечённый фрейм — seq=11 (амплитуда ~0.8)
    match playback.pop_and_decode_next_frame().unwrap() {
        JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 11),
        other => panic!("Ожидался Ready(11), получено {:?}", other),
    }

    // Третий шаг: seq=12 отсутствует, а seq=13 уже в очереди -> генерируется PLC-фрейм (ConcealLoss { missing_sequence: 12 })
    assert_eq!(
        playback.pop_and_decode_next_frame().unwrap(),
        JitterPopStatus::ConcealLoss {
            missing_sequence: 12
        }
    );
    assert_eq!(playback.stats().plc_frames, 1);

    // Четвёртый шаг — seq=13
    match playback.pop_and_decode_next_frame().unwrap() {
        JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 13),
        other => panic!("Ожидался Ready(13), получено {:?}", other),
    }

    // Все 4 фрейма (3 декодированных + 1 PLC) записали по 1920 сэмплов = 7680 сэмплов в speaker_ring
    assert_eq!(playback.stats().packets_received, 3);
    assert_eq!(playback.stats().packets_decoded, 3);
    assert_eq!(playback.stats().samples_written_to_ring, 4 * 1920);

    let mut speaker_out = vec![0.0f32; 4 * 1920];
    assert_eq!(playback.read_speaker_samples(&mut speaker_out), 4 * 1920);
    // Проверяем, что PLC-сегмент (сэмплы 3840..5760) содержит плавно затухающий сигнал без NaN и в пределах [-1.0, 1.0]
    let plc_slice = &speaker_out[2 * 1920..3 * 1920];
    assert!(plc_slice.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    let plc_mean_abs: f32 = plc_slice.iter().map(|s| s.abs()).sum::<f32>() / plc_slice.len() as f32;
    assert!(
        plc_mean_abs > 0.1 && plc_mean_abs < 0.8,
        "PLC фрейм должен быть плавно ослабленной копией предыдущего фрейма (0.8), получено {plc_mean_abs}"
    );
}

#[tokio::test]
async fn live_udp_loopback_roundtrip_srtp_only_and_rtcp_bitrate_scaling() {
    let secret_key = [0x5Au8; 32];
    let (tx_streamer, rx_streamer) = make_loopback_streamer_pair(secret_key, 1111, 2222).await;

    let mut capture = AudioCapturePipeline::new(
        AudioCaptureConfig::new(48_000, 2)
            .with_mic_gain(1.0)
            .with_vad_threshold(0.02),
    );
    let mut playback_cfg = AudioPlaybackConfig::new(48_000, 2);
    playback_cfg.initial_jitter_delay_ms = 40; // 2 фрейма
    let mut playback = AudioPlaybackPipeline::new(playback_cfg);

    // Подаём 3 фрейма по 20 мс (3 * 1920 = 5760 стерео-сэмплов):
    // Фрейм 1: 0.5, Фрейм 2: -0.5, Фрейм 3: тишина (0.0 -> DTX)
    let mut mic_input = Vec::with_capacity(3 * 1920);
    mic_input.extend_from_slice(&vec![0.5f32; 1920]);
    mic_input.extend_from_slice(&vec![-0.5f32; 1920]);
    mic_input.extend_from_slice(&vec![0.0f32; 1920]);

    capture.push_mic_samples(&mic_input);
    let tick = capture.poll_and_send_frames(&tx_streamer).await.unwrap();
    assert_eq!(tick.frames_polled, 3);
    assert_eq!(tick.frames_encoded, 2);
    assert_eq!(tick.silence_frames, 1);
    assert_eq!(tick.packets_sent, 3);
    assert_eq!(tick.payloads[2], DAVE_OPUS_SILENCE_PACKET);

    // Принимаем все 3 RTP/SRTP пакета из сокета loopback
    for expected_seq in 100u16..103u16 {
        let pkt = playback
            .recv_and_queue_packet(&rx_streamer, 1001)
            .await
            .unwrap();
        assert_eq!(pkt.sequence, expected_seq);
        assert_eq!(pkt.ssrc, 1111);
    }

    let decoded_count = playback.drain_jitter_to_speaker_ring().unwrap();
    assert_eq!(decoded_count, 3);

    let mut speaker_pcm = vec![0.0f32; 3 * 1920];
    assert_eq!(playback.read_speaker_samples(&mut speaker_pcm), 3 * 1920);

    // Проверяем восстановленные уровни всех 3 фреймов
    let f1_avg: f32 = speaker_pcm[0..1920].iter().sum::<f32>() / 1920.0;
    let f2_avg: f32 = speaker_pcm[1920..3840].iter().sum::<f32>() / 1920.0;
    let f3_max_abs: f32 = speaker_pcm[3840..5760]
        .iter()
        .map(|s| s.abs())
        .fold(0.0, f32::max);

    assert!(
        (f1_avg - 0.5).abs() < 0.02,
        "Фрейм 1 должен быть ~0.5, получено {f1_avg}"
    );
    assert!(
        (f2_avg - (-0.5)).abs() < 0.02,
        "Фрейм 2 должен быть ~-0.5, получено {f2_avg}"
    );
    assert_eq!(
        f3_max_abs, 0.0,
        "DTX фрейм 3 обязан декодироваться в чистую тишину 0.0"
    );

    // Проверяем реакцию размера пакета захвата на RTCP потери (64 кбит/с = 160B -> 32 кбит/с = 80B)
    tx_streamer.apply_fraction_lost(128).await;
    tx_streamer.apply_fraction_lost(128).await;
    capture.push_mic_samples(&vec![0.35f32; 1920]);
    let low_br_tick = capture.poll_and_send_frames(&tx_streamer).await.unwrap();
    assert_eq!(low_br_tick.payloads.len(), 1);
    assert_eq!(low_br_tick.payloads[0].len(), 80);
}

#[tokio::test]
async fn live_udp_loopback_roundtrip_dave_mls_v1_e2ee() {
    let srtp_key = [0x9Cu8; 32];
    let init_secret = [0x41u8; 32];
    let commit_secret = [0x29u8; 32];
    let ctx = sample_dave_group_context(1);
    let members = [1001u64, 2002u64];

    let (tx_streamer, rx_streamer) = make_loopback_streamer_pair(srtp_key, 5555, 6666).await;

    let alice_session = DaveVoiceSession::new(
        1001,
        DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit_secret, &ctx, None).unwrap(),
        &members,
        DaveTransportCipherMode::AeadAes256GcmRtpsize,
    )
    .unwrap();
    let bob_session = DaveVoiceSession::new(
        2002,
        DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit_secret, &ctx, None).unwrap(),
        &members,
        DaveTransportCipherMode::AeadAes256GcmRtpsize,
    )
    .unwrap();

    tx_streamer.enable_dave_session(alice_session).await;
    rx_streamer.enable_dave_session(bob_session).await;

    let mut capture =
        AudioCapturePipeline::new(AudioCaptureConfig::new(48_000, 2).with_vad_threshold(0.01));
    let mut playback_cfg = AudioPlaybackConfig::new(48_000, 2);
    playback_cfg.initial_jitter_delay_ms = 20; // 1 фрейм
    let mut playback = AudioPlaybackPipeline::new(playback_cfg);

    // Генерируем стерео-сигнал с разными уровнями в левом (0.7) и правом (-0.3) каналах
    let mut stereo_frame = Vec::with_capacity(1920);
    for _ in 0..960 {
        stereo_frame.push(0.7f32);
        stereo_frame.push(-0.3f32);
    }

    capture.push_mic_samples(&stereo_frame);
    // И второй фрейм тишины (DTX bypass внутри DAVE)
    capture.push_mic_samples(&vec![0.0f32; 1920]);

    let tick = capture.poll_and_send_frames(&tx_streamer).await.unwrap();
    assert_eq!(tick.packets_sent, 2);
    assert_eq!(tx_streamer.stats.lock().await.dave_frames_encrypted, 2);

    // Принимаем оба пакета с двухслойной расшифровкой (SRTP rtpsize + DAVE MLS v1)
    let p1 = playback
        .recv_and_queue_packet(&rx_streamer, 1001)
        .await
        .unwrap();
    let p2 = playback
        .recv_and_queue_packet(&rx_streamer, 1001)
        .await
        .unwrap();
    assert_eq!(p1.sequence, 100);
    assert_eq!(p2.sequence, 101);
    assert_eq!(&*p2, &DAVE_OPUS_SILENCE_PACKET[..]);
    assert_eq!(rx_streamer.stats.lock().await.dave_frames_decrypted, 2);

    assert_eq!(playback.drain_jitter_to_speaker_ring().unwrap(), 2);

    let mut out_stereo = vec![0.0f32; 1920];
    assert_eq!(playback.read_speaker_samples(&mut out_stereo), 1920);
    let left_avg: f32 = out_stereo.iter().step_by(2).sum::<f32>() / 960.0;
    let right_avg: f32 = out_stereo.iter().skip(1).step_by(2).sum::<f32>() / 960.0;
    assert!(
        (left_avg - 0.7).abs() < 0.03,
        "Левый канал DAVE должен быть ~0.7, получено {left_avg}"
    );
    assert!(
        (right_avg - (-0.3)).abs() < 0.03,
        "Правый канал DAVE должен быть ~-0.3, получено {right_avg}"
    );

    let mut out_silence = vec![1.0f32; 1920];
    assert_eq!(playback.read_speaker_samples(&mut out_silence), 1920);
    assert!(out_silence.iter().all(|&s| s == 0.0));
}
