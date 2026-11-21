//! Контрактные и сквозные тесты `TASK-901`:
//! - SUB-901-1: Изолированный композитор Wayland в ОЗУ (`wayland-99`, `llvmpipe`, `< 300 мс`)
//! - SUB-901-2: Локальный Tokio Mock Discord Gateway Daemon (HTTP/2 + HTTP/1.1 REST v9 + WS v9 + `capture-ready.jsonl`)
//! - SUB-901-3: E2E LifeCycle Runner & Process Orchestrator (graceful shutdown, перехват паник/бэктрейсов, 0 зомби и утечек FD)

use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chord_gpui_e2e_harness::{
    CaptureReadyTrace, CompositorConfig, CompositorSession, E2eRunner, E2eRunnerConfig,
    MockGatewayConfig, MockGatewayDaemon,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("Корень репозитория должен существовать")
}

#[test]
fn sub_901_1_compositor_session_allocates_wayland_99_in_ram_under_300ms_and_cleans_up() {
    let mut session = CompositorSession::start(CompositorConfig::default())
        .expect("CompositorSession должен стартовать без физического монитора");

    assert!(
        session.startup_elapsed() < Duration::from_millis(300),
        "Старт композитора занял {:?} (превышен DoD бюджет < 300 мс)",
        session.startup_elapsed()
    );
    assert_eq!(session.wayland_display(), "wayland-99");
    assert!(
        session.is_socket_alive(),
        "Сокет wayland-99 должен быть активным AF_UNIX сокетом"
    );

    let sock_path = session.socket_path();
    let meta = std::fs::symlink_metadata(&sock_path).expect("Метаданные сокета wayland-99");
    assert!(
        meta.file_type().is_socket(),
        "Путь {} обязан быть UNIX-сокетом (S_IFSOCK)",
        sock_path.display()
    );

    let env_map: std::collections::HashMap<String, String> =
        session.env_vars().into_iter().collect();
    assert_eq!(
        env_map.get("WAYLAND_DISPLAY").map(String::as_str),
        Some("wayland-99")
    );
    assert_eq!(
        env_map.get("LIBGL_ALWAYS_SOFTWARE").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        env_map.get("GALLIUM_DRIVER").map(String::as_str),
        Some("llvmpipe")
    );
    assert_eq!(
        env_map.get("WLR_BACKENDS").map(String::as_str),
        Some("headless")
    );
    assert_eq!(
        env_map.get("WLR_LIBINPUT_NO_DEVICES").map(String::as_str),
        Some("1")
    );

    let runtime_dir = session.runtime_dir().to_path_buf();
    session.shutdown().expect("Graceful shutdown композитора");

    assert!(
        !sock_path.exists(),
        "Сокет wayland-99 должен быть удалён после shutdown"
    );
    assert!(
        !runtime_dir.exists(),
        "Временный XDG_RUNTIME_DIR в ОЗУ должен быть удалён"
    );
}

#[test]
fn sub_901_2_3_capture_ready_trace_loader_hydrates_oracle_from_re_deep() {
    let trace = CaptureReadyTrace::load_default()
        .expect("Загрузчик трасс должен прочитать /home/sanya/re-deep/capture-ready.jsonl");

    // Независимый оракул из /home/sanya/re-deep/capture-ready.jsonl и gw-proto/msgs-run1-57992_5112.jsonl
    assert_eq!(trace.gateway_request_id, "57992.5112");
    assert!(
        trace
            .gateway_url
            .contains("encoding=etf&v=9&compress=zstd-stream")
    );
    assert_eq!(trace.attached_guild_id, "1014537851418980372");
    assert_eq!(trace.attached_channel_id, "1163408591315812392");
    assert_eq!(trace.heartbeat_interval_ms, 41250);

    assert_eq!(trace.current_user.id, "384299970632876032");
    assert_eq!(trace.current_user.username, "pharos12510");
    assert_eq!(
        trace.current_user.global_name.as_deref(),
        Some("ShadowByte")
    );
    assert_eq!(trace.current_user.discriminator, "0");

    assert_eq!(
        trace.guilds.len(),
        7,
        "В READY из capture-ready.jsonl ровно 7 гильдий"
    );
    let first_guild = &trace.guilds[0];
    assert_eq!(first_guild.id, "274093052673392641");
    assert_eq!(first_guild.name, "Игровое сообщество ⚛MONOLIT⚛");
    assert_eq!(first_guild.channels.len(), 55);

    let itskidon = trace
        .guilds
        .iter()
        .find(|g| g.id == "1014537851418980372")
        .expect("Гильдия ITSKIDON (1014537851418980372) должна присутствовать");
    assert!(itskidon.name.contains("ITSKIDON"));

    assert!(!trace.raw_wsrecv_frames.is_empty());
    assert_eq!(trace.hello_etf_frame.first(), Some(&0x83));
    assert_eq!(trace.ready_etf_frame.first(), Some(&0x83));
}

#[tokio::test]
async fn sub_901_2_1_mock_gateway_http2_rest_v9_routes_and_error_dimensions() {
    let trace = CaptureReadyTrace::load_default().expect("load trace");
    let config = MockGatewayConfig {
        require_auth: true,
        expected_token: Some("test-e2e-token".to_string()),
    };
    let mut daemon = MockGatewayDaemon::start_with_config(trace, config)
        .await
        .expect("Старт MockGatewayDaemon");

    let auth = [("authorization", "test-e2e-token")];

    // 1. Позитивный HTTP/2 запрос: GET /api/v9/users/@me
    let me_resp = daemon
        .send_http2_request("GET", "/api/v9/users/@me", &auth, None)
        .await
        .expect("HTTP/2 GET /users/@me");
    assert_eq!(me_resp.status, 200);
    assert!(
        me_resp.settings_acked,
        "Сервер обязан подтвердить HTTP/2 SETTINGS ACK"
    );
    let me_json: serde_json::Value =
        serde_json::from_slice(&me_resp.body).expect("JSON /users/@me");
    assert_eq!(me_json["id"], "384299970632876032");
    assert_eq!(me_json["username"], "pharos12510");

    // 2. Позитивный HTTP/2 запрос: GET /api/v9/guilds (и /users/@me/guilds)
    let guilds_resp = daemon
        .send_http2_request("GET", "/api/v9/guilds", &auth, None)
        .await
        .expect("HTTP/2 GET /guilds");
    assert_eq!(guilds_resp.status, 200);
    let guilds_json: serde_json::Value =
        serde_json::from_slice(&guilds_resp.body).expect("JSON /guilds");
    assert_eq!(guilds_json.as_array().map(Vec::len), Some(7));

    // 3. Позитивный HTTP/2 запрос: GET /api/v9/channels/1163408591315812392/messages
    let msgs_resp = daemon
        .send_http2_request(
            "GET",
            "/api/v9/channels/1163408591315812392/messages?limit=12",
            &auth,
            None,
        )
        .await
        .expect("HTTP/2 GET /channels/{id}/messages");
    assert_eq!(msgs_resp.status, 200);
    let msgs_json: serde_json::Value =
        serde_json::from_slice(&msgs_resp.body).expect("JSON messages");
    assert!(!msgs_json.as_array().unwrap().is_empty());

    // 4. Граничный сценарий: POST /channels/{id}/messages с 2000 символами (OK) и 2001 символом (400)
    let max_body = serde_json::json!({ "content": "А".repeat(2000) }).to_string();
    let post_ok = daemon
        .send_http2_request(
            "POST",
            "/api/v9/channels/1163408591315812392/messages",
            &auth,
            Some(max_body.as_bytes()),
        )
        .await
        .expect("POST 2000 символов");
    assert_eq!(post_ok.status, 200);

    let oversized_body = serde_json::json!({ "content": "Б".repeat(2001) }).to_string();
    let post_bad = daemon
        .send_http2_request(
            "POST",
            "/api/v9/channels/1163408591315812392/messages",
            &auth,
            Some(oversized_body.as_bytes()),
        )
        .await
        .expect("POST 2001 символ");
    assert_eq!(post_bad.status, 400);

    // 5. Права доступа / Негативный сценарий: запрос без токена возвращает 401 Unauthorized
    let unauth_resp = daemon
        .send_http2_request("GET", "/api/v9/users/@me", &[], None)
        .await
        .expect("HTTP/2 без токена");
    assert_eq!(unauth_resp.status, 401);

    // 6. Инжекция 429 Too Many Requests с retry_after_ms
    daemon.inject_http_status("/api/v9/guilds", 429, Some(250));
    let rate_limited = daemon
        .send_http2_request("GET", "/api/v9/guilds", &auth, None)
        .await
        .expect("HTTP/2 429");
    assert_eq!(rate_limited.status, 429);
    let rl_json: serde_json::Value = serde_json::from_slice(&rate_limited.body).expect("429 JSON");
    assert!(rl_json["retry_after"].as_f64().unwrap_or(0.0) > 0.0);

    assert!(daemon.metrics().http2_requests >= 6);
    daemon.shutdown().await.expect("shutdown daemon");
}

#[tokio::test]
async fn sub_901_2_2_mock_gateway_websocket_v9_identify_heartbeat_voice_and_resume() {
    let trace = CaptureReadyTrace::load_default().expect("load trace");
    let mut daemon = MockGatewayDaemon::start(trace)
        .await
        .expect("Старт MockGatewayDaemon");

    let mut ws = daemon
        .connect_ws_client()
        .await
        .expect("Подключение WebSocket клиента к MockGatewayDaemon");

    // 1. При подключении сервер немедленно присылает Opcode 10 HELLO
    let hello = ws.recv_json().await.expect("Получение Opcode 10 HELLO");
    assert_eq!(hello["op"], 10);
    assert_eq!(hello["d"]["heartbeat_interval"], 41250);

    // 2. Отправка Opcode 2 IDENTIFY -> получение Opcode 0 READY + Opcode 0 READY_SUPPLEMENTAL
    ws.send_json(&serde_json::json!({
        "op": 2,
        "d": {
            "token": "test-e2e-token",
            "capabilities": 1734653,
            "properties": { "os": "Linux", "browser": "Discord Client" }
        }
    }))
    .await
    .expect("Отправка Opcode 2 IDENTIFY");

    let ready = ws.recv_json().await.expect("Получение Opcode 0 READY");
    assert_eq!(ready["op"], 0);
    assert_eq!(ready["t"], "READY");
    assert_eq!(ready["s"], 1);
    assert_eq!(ready["d"]["user"]["id"], "384299970632876032");
    assert_eq!(ready["d"]["guilds"].as_array().map(Vec::len), Some(7));

    let supplemental = ws
        .recv_json()
        .await
        .expect("Получение Opcode 0 READY_SUPPLEMENTAL");
    assert_eq!(supplemental["op"], 0);
    assert_eq!(supplemental["t"], "READY_SUPPLEMENTAL");
    assert_eq!(supplemental["s"], 2);

    // 3. Отправка Opcode 1 Heartbeat -> получение Opcode 11 Heartbeat ACK
    ws.send_json(&serde_json::json!({ "op": 1, "d": 2 }))
        .await
        .expect("Отправка Opcode 1 Heartbeat");
    let hb_ack = ws
        .recv_json()
        .await
        .expect("Получение Opcode 11 Heartbeat ACK");
    assert_eq!(hb_ack["op"], 11);

    // 4. Отправка Opcode 4 VOICE_STATE_UPDATE -> получение VOICE_STATE_UPDATE и VOICE_SERVER_UPDATE
    ws.send_json(&serde_json::json!({
        "op": 4,
        "d": {
            "guild_id": "1014537851418980372",
            "channel_id": "1163408591315812392",
            "self_mute": false,
            "self_deaf": false,
            "self_video": true
        }
    }))
    .await
    .expect("Отправка Opcode 4 VOICE_STATE_UPDATE");

    let voice_state = ws.recv_json().await.expect("VOICE_STATE_UPDATE dispatch");
    assert_eq!(voice_state["op"], 0);
    assert_eq!(voice_state["t"], "VOICE_STATE_UPDATE");
    assert_eq!(voice_state["d"]["channel_id"], "1163408591315812392");
    assert_eq!(voice_state["d"]["self_video"], true);

    let voice_server = ws.recv_json().await.expect("VOICE_SERVER_UPDATE dispatch");
    assert_eq!(voice_server["op"], 0);
    assert_eq!(voice_server["t"], "VOICE_SERVER_UPDATE");
    assert_eq!(voice_server["d"]["guild_id"], "1014537851418980372");

    // 5. Recovery: Opcode 6 RESUME -> получение Opcode 0 RESUMED
    ws.send_json(&serde_json::json!({
        "op": 6,
        "d": {
            "token": "test-e2e-token",
            "session_id": ready["d"]["session_id"],
            "seq": 4
        }
    }))
    .await
    .expect("Отправка Opcode 6 RESUME");

    let resumed = ws.recv_json().await.expect("RESUMED dispatch");
    assert_eq!(resumed["op"], 0);
    assert_eq!(resumed["t"], "RESUMED");

    ws.close().await.expect("Закрытие WS клиента");

    let metrics = daemon.metrics();
    assert_eq!(metrics.identifies, 1);
    assert_eq!(metrics.heartbeats_acked, 1);
    assert_eq!(metrics.voice_state_updates, 1);
    assert_eq!(metrics.resumes, 1);

    daemon.shutdown().await.expect("shutdown daemon");
}

#[tokio::test]
async fn sub_901_3_e2e_lifecycle_runner_executes_headless_gpui_under_300ms_and_captures_panics_and_hygiene()
 {
    let start_total = Instant::now();
    let mut runner = E2eRunner::boot(E2eRunnerConfig::default())
        .await
        .expect("Запуск E2eRunner (Wayland + MockGateway)");

    let boot_elapsed = runner.boot_elapsed();
    assert!(
        boot_elapsed < Duration::from_millis(300),
        "Развёртывание виртуального окружения E2E заняло {:?} (бюджет DoD < 300 мс)",
        boot_elapsed
    );

    // 1. Запуск нативного бинарника discord-gpui в изолированном Wayland окружении без монитора
    let gpui_bin = repo_root().join("rust/dist/discord-gpui");
    assert!(
        gpui_bin.exists(),
        "Бинарник rust/dist/discord-gpui должен существовать"
    );

    let smoke_res = runner
        .run_binary(
            &gpui_bin,
            // FIX-12: Используем --smoke=summary (однострочный вывод) вместо JSON --headless-smoke
            &["--smoke=summary"],
            &[("AUTHORIZATION", "Bearer secret-live-token-999")],
        )
        .await
        .expect("Выполнение chord-gpui --smoke=summary через E2eRunner");

    let total_e2e_elapsed = start_total.elapsed();
    assert!(
        total_e2e_elapsed < Duration::from_millis(300),
        "Полный цикл чистого headless запуска E2E занял {:?} (DoD < 300 мс)",
        total_e2e_elapsed
    );

    assert_eq!(smoke_res.exit_code, Some(0));
    assert!(
        smoke_res.panic_report.is_none(),
        "При штатном запуске паник быть не должно"
    );
    // FIX-12: --smoke=summary возвращает однострочный формат, не JSON
    assert!(
        smoke_res.stdout.contains("SMOKE ok ") || smoke_res.stdout.contains("GPUI_SMOKE_OK "),
        "Бинарник должен вернуть SMOKE ok ... или GPUI_SMOKE_OK ...: {}",
        smoke_res.stdout
    );
    assert!(
        smoke_res.stdout.contains("backend=wayland") || smoke_res.stdout.contains("\"display_backend\":\"wayland\""),
        "Бинарник должен видеть чистый изолированный бэкенд wayland (без X11 DISPLAY): {}",
        smoke_res.stdout
    );

    // 2. Проверка перехвата паники, дампа бэктрейса и маскирования секретных токенов в логах
    let bash_bin = Path::new("/bin/bash");
    let panic_script = concat!(
        "echo 'Auth token: secret-live-token-999'; ",
        "echo \"thread 'main' panicked at src/ui/layout.rs:42:9:\nindex out of bounds: the len is 0 but the index is 1\" >&2; ",
        "echo \"stack backtrace:\n   0: rust_begin_unwind\n   1: core::panicking::panic_fmt\n   2: discord_gpui::ui::layout::render\" >&2; ",
        "exit 101"
    );
    let panic_res = runner
        .run_binary(
            bash_bin,
            &["-c", panic_script],
            &[("DISCORD_TOKEN", "secret-live-token-999")],
        )
        .await
        .expect("Запуск процесса с эмуляцией паники");

    assert_eq!(panic_res.exit_code, Some(101));
    assert!(
        !panic_res.stdout.contains("secret-live-token-999"),
        "Секретный токен обязан маскироваться в перехваченном stdout: {}",
        panic_res.stdout
    );
    let panic_report = panic_res
        .panic_report
        .expect("При панике бинарника обязан формироваться PanicReport");
    assert!(panic_report.message.contains("index out of bounds"));
    assert_eq!(
        panic_report.location.as_deref(),
        Some("src/ui/layout.rs:42:9")
    );
    assert!(
        panic_report
            .backtrace
            .contains("discord_gpui::ui::layout::render")
    );

    // 3. Graceful shutdown и проверка гигиены ресурсов (0 зомби-процессов, 0 утечек FD)
    let hygiene = runner
        .shutdown()
        .await
        .expect("Graceful shutdown E2eRunner");

    assert_eq!(
        hygiene.zombie_processes, 0,
        "После завершения сценария не должно быть зомби-процессов"
    );
    assert_eq!(
        hygiene.lingering_children, 0,
        "Все дочерние процессы должны быть завершены"
    );
    assert_eq!(
        hygiene.fd_leaked, 0,
        "Не должно быть утечек файловых дескрипторов"
    );
    assert!(
        hygiene.socket_cleaned,
        "Виртуальный сокет Wayland должен быть удалён"
    );
    assert!(
        hygiene.runtime_dir_cleaned,
        "Временный каталог XDG_RUNTIME_DIR должен быть удалён"
    );
}
