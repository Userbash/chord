//! W7 Contract Tests (`TASK-822` & `TASK-823`):
//! 1. `guilds::resolve_invite_info`: HTTP/2 connection pooling with 90s keep-alive,
//!    retry-with-exponential-backoff (150ms base), and elimination of the 2.6s freeze.
//! 2. Defeating Discord AVError / stream error codes `2004`, `2005`, `2009`, `2010`, `2016`, `2017`
//!    (`stream-send-high-packet-loss`, `stream-send-low-encode-fps`, `video-encode-error`,
//!    `stream-full`, `video-background-unavailable`, `stream-transport-codec-recovery`).
//! 3. RTC Quality Status Bar DTO (`build_rtc_quality_status_bar_dto`) with RTT/loss color grades,
//!    codecs (`Opus`, `AV1`/`H.264`), and `DAVE MLS v1 Verified` status.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::guilds::{
    resolve_invite_info, resolve_invite_info_with_pool, InviteConnectionPool, InvitePoolConfig,
    INVITE_ATTEMPT_TIMEOUT_MS, INVITE_INITIAL_BACKOFF_MS, INVITE_MAX_RETRY_ATTEMPTS,
    INVITE_POOL_KEEP_ALIVE_SECS, LEGACY_FREEZE_THRESHOLD_MS,
};
use discord_gui_spike::invites::{InviteError, InviteTransport};
use discord_gui_spike::voice_diagnostics::{
    build_rtc_quality_status_bar_dto, defeat_av_error_code, discord_min_encode_fps_threshold,
    evaluate_and_defeat_av_errors, AvStreamTelemetryInput, ConnectionQuality, DiscordAvErrorCode,
    DISCORD_DEFEATED_AV_ERROR_CODES,
};

const SAMPLE_INVITE_JSON: &[u8] = br#"{
    "code": "eSrxp2YGUC",
    "type": 0,
    "guild": {
        "id": "274093052673392641",
        "name": "ITSKIDON PC BOOST",
        "icon": "a_123456"
    },
    "channel": {
        "id": "274093052673392642",
        "name": "welcome",
        "type": 0
    },
    "approximate_member_count": 14520,
    "approximate_presence_count": 1890,
    "expires_at": null
}"#;

/// Скриптованный транспорт для проверки пула соединений и ретраев с экспоненциальным backoff.
#[derive(Clone, Default)]
struct ScriptedInviteTransport {
    outcomes: Arc<Mutex<Vec<Result<HttpResponse, InviteError>>>>,
    gets: Arc<Mutex<Vec<String>>>,
}

impl ScriptedInviteTransport {
    fn new(outcomes: Vec<Result<HttpResponse, InviteError>>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes)),
            gets: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.gets.lock().unwrap().clone()
    }
}

impl InviteTransport for ScriptedInviteTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.gets.lock().unwrap().push(target.to_string());
        let mut queue = self.outcomes.lock().unwrap();
        let next = if queue.is_empty() {
            Ok(HttpResponse {
                status: 200,
                headers: vec![("keep-alive".to_string(), "timeout=90, max=100".to_string())],
                body: SAMPLE_INVITE_JSON.to_vec(),
            })
        } else {
            queue.remove(0)
        };
        Box::pin(async move { next })
    }

    fn post<'a>(
        &'a self,
        _target: &'a str,
        _body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        Box::pin(async { Err(InviteError::Network) })
    }
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn task_822_pool_config_constants_enforce_http2_keepalive_90s_and_prevent_2600ms_freeze() {
    assert_eq!(INVITE_POOL_KEEP_ALIVE_SECS, 90);
    assert_eq!(INVITE_INITIAL_BACKOFF_MS, 150);
    assert_eq!(INVITE_MAX_RETRY_ATTEMPTS, 3);
    assert!(INVITE_ATTEMPT_TIMEOUT_MS <= 500);

    let cfg = InvitePoolConfig::default();
    assert!(
        cfg.http2_multiplexing,
        "HTTP/2 multiplexing must be enabled"
    );
    assert_eq!(cfg.tcp_keepalive_secs, 90);
    assert_eq!(cfg.pool_idle_timeout_secs, 90);
    assert_eq!(cfg.initial_backoff_ms, 150);
    assert_eq!(cfg.backoff_delay_ms(0), 150);
    assert_eq!(cfg.backoff_delay_ms(1), 300);
    assert_eq!(cfg.backoff_delay_ms(2), 600);
    assert!(
        cfg.max_worst_case_latency_ms() < LEGACY_FREEZE_THRESHOLD_MS,
        "Worst-case retry + timeout latency ({}) must be strictly below the 2600ms freeze threshold",
        cfg.max_worst_case_latency_ms()
    );
}

#[test]
fn task_822_resolve_invite_info_reuses_warm_http2_connection_and_eliminates_freeze() {
    let pool = InviteConnectionPool::new();
    let cfg = InvitePoolConfig::default();
    assert_eq!(pool.warm_connections("discord.com"), 0);

    // Pre-warm connection to discord.com
    pool.prewarm("discord.com", 1000);
    assert_eq!(pool.warm_connections("discord.com"), 1);

    let transport = ScriptedInviteTransport::new(vec![
        Ok(HttpResponse {
            status: 200,
            headers: vec![("keep-alive".to_string(), "timeout=90, max=100".to_string())],
            body: SAMPLE_INVITE_JSON.to_vec(),
        }),
        Ok(HttpResponse {
            status: 200,
            headers: vec![("keep-alive".to_string(), "timeout=90, max=100".to_string())],
            body: SAMPLE_INVITE_JSON.to_vec(),
        }),
    ]);

    let started = Instant::now();
    let first = run(resolve_invite_info_with_pool(
        &transport,
        &pool,
        &cfg,
        "https://discord.gg/eSrxp2YGUC",
    ))
    .expect("first resolve_invite_info_with_pool must succeed");
    let elapsed_ms = started.elapsed().as_millis() as u64;

    assert!(
        elapsed_ms < 500,
        "pooled resolve_invite_info must complete well under 500ms (actual: {elapsed_ms}ms)"
    );
    assert_eq!(first.preview.code, "eSrxp2YGUC");
    assert_eq!(first.preview.guild_id, "274093052673392641");
    assert_eq!(first.preview.guild_name, "ITSKIDON PC BOOST");
    assert_eq!(first.metrics.attempts, 1);
    assert!(
        first.metrics.reused_connection,
        "prewarmed connection must be reused on first request"
    );
    assert_eq!(first.metrics.http_version, "HTTP/2");
    assert_eq!(first.metrics.keep_alive_header, "timeout=90, max=100");
    assert_eq!(first.metrics.total_backoff_ms, 0);

    // Second request reuses the pooled connection and increments reuse counter
    let second = run(resolve_invite_info(&transport, "eSrxp2YGUC"))
        .expect("resolve_invite_info free function must succeed");
    assert_eq!(second.guild_id, "274093052673392641");
    assert!(pool.total_reuses() >= 1);
}

#[test]
fn task_822_resolve_invite_info_retries_transient_network_and_5xx_with_150ms_backoff() {
    let pool = InviteConnectionPool::new();
    let cfg = InvitePoolConfig::default();

    // First attempt fails with transient Network (simulating stale socket),
    // second attempt fails with HTTP 502 Bad Gateway,
    // third attempt succeeds with 200 OK.
    let transport = ScriptedInviteTransport::new(vec![
        Err(InviteError::Network),
        Ok(HttpResponse {
            status: 502,
            headers: Vec::new(),
            body: b"Bad Gateway".to_vec(),
        }),
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: SAMPLE_INVITE_JSON.to_vec(),
        }),
    ]);

    let resolved = run(resolve_invite_info_with_pool(
        &transport,
        &pool,
        &cfg,
        "eSrxp2YGUC",
    ))
    .expect("resolve_invite_info_with_pool must recover after transient Network + 502 errors");

    assert_eq!(resolved.preview.code, "eSrxp2YGUC");
    assert_eq!(resolved.metrics.attempts, 3);
    assert_eq!(
        resolved.metrics.total_backoff_ms,
        150 + 300,
        "total backoff across 2 retries must be 150ms + 300ms = 450ms"
    );
    assert_eq!(transport.calls().len(), 3);
}

#[test]
fn task_822_resolve_invite_info_does_not_retry_non_transient_404_or_invalid_arg() {
    let pool = InviteConnectionPool::new();
    let cfg = InvitePoolConfig::default();

    // Invalid invite syntax -> rejected before touching transport (0 attempts)
    let transport = ScriptedInviteTransport::new(vec![]);
    let invalid_err = run(resolve_invite_info_with_pool(
        &transport,
        &pool,
        &cfg,
        "   bad invite with spaces   ",
    ))
    .expect_err("invalid code must return InvalidArg");
    assert_eq!(invalid_err, InviteError::InvalidArg);
    assert_eq!(
        transport.calls().len(),
        0,
        "invalid invite must not hit network"
    );

    // HTTP 404 NotFound -> must fail immediately on attempt 1 without retry
    let transport_404 = ScriptedInviteTransport::new(vec![Ok(HttpResponse {
        status: 404,
        headers: Vec::new(),
        body: br#"{"message": "Unknown Invite", "code": 10006}"#.to_vec(),
    })]);
    let not_found_err = run(resolve_invite_info_with_pool(
        &transport_404,
        &pool,
        &cfg,
        "deadcode",
    ))
    .expect_err("HTTP 404 must return NotFound");
    assert_eq!(not_found_err, InviteError::NotFound);
    assert_eq!(
        transport_404.calls().len(),
        1,
        "HTTP 404 NotFound is non-transient and must not be retried"
    );
}

#[test]
fn task_823_defeats_all_six_discord_av_error_codes_2004_2005_2009_2010_2016_2017() {
    assert_eq!(
        DISCORD_DEFEATED_AV_ERROR_CODES,
        [2004, 2005, 2009, 2010, 2016, 2017]
    );

    // Verify Discord RE oracle FPS threshold function B(e):
    // e <= 5 -> 3, e <= 15 -> 8, e <= 30 -> 15, e > 30 -> 30
    assert_eq!(discord_min_encode_fps_threshold(5), 3);
    assert_eq!(discord_min_encode_fps_threshold(15), 8);
    assert_eq!(discord_min_encode_fps_threshold(30), 15);
    assert_eq!(discord_min_encode_fps_threshold(60), 30);

    // Every single target code 2004, 2005, 2009, 2010, 2016, 2017 must return a concrete mitigation plan
    for code in DISCORD_DEFEATED_AV_ERROR_CODES {
        let plan = defeat_av_error_code(code)
            .unwrap_or_else(|| panic!("AVError code {code} must have a defeat mitigation plan"));
        assert_eq!(plan.code, code);
        assert!(
            plan.suppressed,
            "AVError {code} must be defeated/suppressed"
        );
        assert!(!plan.error_slug.is_empty());
        assert!(!plan.recovery_action.is_empty());
    }
    assert_eq!(
        defeat_av_error_code(2004).unwrap().error_slug,
        "stream-send-high-packet-loss"
    );
    assert_eq!(
        defeat_av_error_code(2005).unwrap().error_slug,
        "stream-send-low-encode-fps"
    );
    assert_eq!(
        defeat_av_error_code(2009).unwrap().error_slug,
        "video-encode-error"
    );
    assert_eq!(
        defeat_av_error_code(2010).unwrap().error_slug,
        "stream-full"
    );
    assert_eq!(
        defeat_av_error_code(2016).unwrap().error_slug,
        "video-background-unavailable"
    );
    assert_eq!(
        defeat_av_error_code(2017).unwrap().error_slug,
        "stream-transport-codec-recovery"
    );

    // Simulate simultaneous adverse telemetry triggering all 6 conditions:
    // - 2004: short_packet_loss_rate = 0.18 (18% > 10%)
    // - 2005: short_encode_fps = 12.0 (< 30 FPS threshold for 60 FPS target)
    // - 2009: video_encode_error = true on "nvenc_h264"
    // - 2010: stream_delete_reason = Some("stream_full")
    // - 2016: video_background_filter_error = true
    // - 2017: transport_codec_desync = true
    let telemetry = AvStreamTelemetryInput {
        short_packet_loss_rate: 0.18,
        long_packet_loss_rate: 0.14,
        short_encode_fps: 12.0,
        long_encode_fps: 18.0,
        target_max_fps: 60,
        current_bitrate_bps: 6_000_000,
        video_encode_error: true,
        active_encoder: "nvenc_h264".to_string(),
        stream_delete_reason: Some("stream_full".to_string()),
        active_viewer_count: 10,
        max_viewer_capacity: 10,
        video_background_filter_error: true,
        transport_codec_desync: true,
    };

    let report = evaluate_and_defeat_av_errors(&telemetry);
    assert_eq!(
        report.defeated_codes,
        vec![2004, 2005, 2009, 2010, 2016, 2017]
    );
    // 2004 defeated: FEC enabled, bitrate stepped down, effective loss <= 10.0%
    assert!(report.fec_enabled);
    assert!(report.adjusted_bitrate_bps < telemetry.current_bitrate_bps);
    assert!(report.effective_packet_loss_pct <= 10.0);
    // 2005 defeated: duplicate frame keepalive maintains >= B(60) = 30 FPS
    assert!(report.duplicate_frame_keepalive_injected);
    assert!(report.effective_encode_fps >= 30);
    // 2009 defeated: hardware encoder automatically fell back to software_openh264
    assert_eq!(report.active_encoder, "software_openh264");
    // 2010 defeated: stale isStreamMarkedFull cleared so reconnect/watch is not permanently blocked
    assert!(!report.is_stream_marked_full);
    // 2016 defeated: background filter fell back to passthrough_raw on LIVE target
    assert_eq!(report.background_filter_mode, "passthrough_raw");
    // 2017 defeated: transport SSRC/codec reconciled without dropping voice session
    assert!(report.transport_reconciled);
    assert!(report.stream_alive);

    // Ensure enum variants map 1-to-1 with numeric codes
    assert_eq!(DiscordAvErrorCode::StreamSendHighPacketLoss.code(), 2004);
    assert_eq!(DiscordAvErrorCode::StreamSendLowEncodeFps.code(), 2005);
    assert_eq!(DiscordAvErrorCode::VideoEncodeError.code(), 2009);
    assert_eq!(DiscordAvErrorCode::StreamFull.code(), 2010);
    assert_eq!(DiscordAvErrorCode::VideoBackgroundUnavailable.code(), 2016);
    assert_eq!(
        DiscordAvErrorCode::StreamTransportCodecRecovery.code(),
        2017
    );
}

#[test]
fn task_823_rtc_quality_status_bar_dto_evaluates_colors_codecs_and_dave_mls_v1() {
    let fine_bar = build_rtc_quality_status_bar_dto(42, 0.8, "Opus", Some("AV1"), true);
    assert_eq!(fine_bar.quality, ConnectionQuality::Fine.as_str());
    assert_eq!(fine_bar.indicator_color_hex, "#23a55a");
    assert_eq!(fine_bar.ping_ms, 42);
    assert_eq!(fine_bar.audio_codec, "Opus");
    assert_eq!(fine_bar.video_codec.as_deref(), Some("AV1"));
    assert!(fine_bar.dave_mls_v1_verified);
    assert_eq!(fine_bar.dave_status_label, "DAVE MLS v1 Verified");
    assert_eq!(
        fine_bar.defeated_av_codes,
        vec![2004, 2005, 2009, 2010, 2016, 2017]
    );

    let avg_bar = build_rtc_quality_status_bar_dto(320, 4.0, "Opus", Some("H.264"), true);
    assert_eq!(avg_bar.quality, ConnectionQuality::Average.as_str());
    assert_eq!(avg_bar.indicator_color_hex, "#f0b232");

    let bad_bar = build_rtc_quality_status_bar_dto(610, 12.5, "Opus", None, false);
    assert_eq!(bad_bar.quality, ConnectionQuality::Bad.as_str());
    assert_eq!(bad_bar.indicator_color_hex, "#f23f43");
    assert!(!bad_bar.dave_mls_v1_verified);
}
