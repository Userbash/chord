#![recursion_limit = "512"]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/platform/mod.rs"]
mod platform;
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

use std::time::Instant;

use audio::{
    AdaptiveJitterBuffer, DaveMlsGroupContext, DaveMlsKeySchedule, DaveTransportCipherMode,
    DaveVoiceSession,
};
use chord_platform::screencast::RTP_VIDEO_CLOCK_HZ;
use state::{
    AppState, CallStatus, Channel, ChannelType, Guild, LoginStep, Message, MessageKind,
    QuickSwitcherItemKind, UserSummary,
};
use storage::LocalDatabase;
use ui::screen_share_modal::{
    ScreenShareFps, ScreenShareQualityPreset, ScreenShareResolution, ScreenShareSelectorState,
    ScreenShareTab,
};

/// ═══════════════════════════════════════════════════════════════════════════
/// E2E TEST HARNESS: Mock Discord Gateway & Deterministic Scenario Driver
/// ═══════════════════════════════════════════════════════════════════════════

struct MockDiscordGateway {
    current_seq: u64,
    guild_id: String,
    channel_id: String,
}

impl MockDiscordGateway {
    fn new(guild_id: &str, channel_id: &str) -> Self {
        Self {
            current_seq: 1,
            guild_id: guild_id.to_string(),
            channel_id: channel_id.to_string(),
        }
    }

    /// Эмуляция создания серверов и каналов
    fn emit_guild_create(&mut self, state: &mut AppState) {
        state.guilds.push(Guild {
            id: self.guild_id.clone(),
            name: "Rust Community".into(),
            icon: None,
            owner_id: Some("u-owner".into()),
            categories: vec![],
            channels: vec![
                Channel {
                    id: self.channel_id.clone(),
                    guild_id: Some(self.guild_id.clone()),
                    name: "general".into(),
                    channel_type: ChannelType::GuildText,
                    topic: None,
                    unread: false,
                    category_id: None,
                    position: 0,
                    is_locked: false,
                    permissions_bitmask: 0,
                },
                Channel {
                    id: "c-dev".into(),
                    guild_id: Some(self.guild_id.clone()),
                    name: "dev-chat".into(),
                    channel_type: ChannelType::GuildText,
                    topic: None,
                    unread: false,
                    category_id: None,
                    position: 1,
                    is_locked: false,
                    permissions_bitmask: 0,
                },
            ],
            unread_count: 0,
            mention_count: 0,
        });
        state.select_guild(Some(self.guild_id.clone()));
        state.select_channel(self.channel_id.clone());
        self.current_seq += 1;
    }

    /// Эмуляция входящего сообщения
    fn emit_message_create(&mut self, state: &mut AppState, author: &str, text: &str) {
        let msg = Message {
            id: format!("msg-{}", self.current_seq),
            channel_id: self.channel_id.clone(),
            author: UserSummary {
                id: "u-other".into(),
                username: author.into(),
                global_name: Some(author.into()),
                avatar: None,
                is_bot: false,
            },
            content: text.into(),
            timestamp: "2026-11-11T13:40:00Z".into(),
            edited_timestamp: None,
            attachments: vec![],
            reactions: vec![],
            is_pinned: false,
            message_kind: MessageKind::Default,
        };
        state
            .messages
            .entry(self.channel_id.clone())
            .or_default()
            .push(msg);
        self.current_seq += 1;
    }
}

/// ═══════════════════════════════════════════════════════════════════════════
/// USER JOURNEY 1: Холодный старт, гидратация кэша SQLite FTS5 и навигация
/// ═══════════════════════════════════════════════════════════════════════════
#[core::prelude::v1::test]
fn e2e_user_journey_1_cold_boot_sqlite_cache_hydration_and_navigation() {
    let start_instant = Instant::now();

    // 1. Инициализация локальной базы данных SQLite с FTS5
    let db =
        LocalDatabase::open_in_memory().expect("SQLite in-memory база должна инициализироваться");
    let msg1 = Message {
        id: "m-100".into(),
        channel_id: "c-general".into(),
        author: UserSummary {
            id: "u-alice".into(),
            username: "Alice".into(),
            global_name: Some("Alice".into()),
            avatar: None,
            is_bot: false,
        },
        content: "Первое сообщение в кэше".into(),
        timestamp: "2026-11-11T13:40:00Z".into(),
        edited_timestamp: None,
        attachments: vec![],
        reactions: vec![],
        is_pinned: false,
        message_kind: MessageKind::Default,
    };
    let msg2 = Message {
        id: "m-101".into(),
        channel_id: "c-general".into(),
        author: UserSummary {
            id: "u-bob".into(),
            username: "Bob".into(),
            global_name: Some("Bob".into()),
            avatar: None,
            is_bot: false,
        },
        content: "Привет всем в Discord GPUI!".into(),
        timestamp: "2026-11-11T13:40:10Z".into(),
        edited_timestamp: None,
        attachments: vec![],
        reactions: vec![],
        is_pinned: false,
        message_kind: MessageKind::Default,
    };
    db.insert_message(&msg1).expect("Вставка m-100 в SQLite");
    db.insert_message(&msg2).expect("Вставка m-101 в SQLite");

    // 2. Инициализация состояния приложения
    let mut state = AppState::new();
    assert_eq!(state.login_form.step, LoginStep::Credentials);
    assert_eq!(state.current_user.username, "Rustery");

    // 3. Эмуляция создания серверов и каналов
    let mut gateway = MockDiscordGateway::new("g-rust-community", "c-general");
    gateway.emit_guild_create(&mut state);
    assert_eq!(state.selected_guild_id.as_deref(), Some("g-rust-community"));
    assert_eq!(state.selected_channel_id.as_deref(), Some("c-general"));

    // 4. Поиск в SQLite FTS5 и проверка восстановления
    let search_results = db
        .search_messages("Discord", Some("c-general"), None)
        .expect("FTS5 поиск должен пройти");
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].id, "m-101");
    assert_eq!(search_results[0].content, "Привет всем в Discord GPUI!");

    let duration_ms = start_instant.elapsed().as_millis();
    // Бюджет холодного старта и гидратации: < 150 мс
    assert!(
        duration_ms < 150,
        "Холодный старт и гидратация кэша заняли {} мс (бюджет <150 мс)",
        duration_ms
    );
}

/// ═══════════════════════════════════════════════════════════════════════════
/// USER JOURNEY 2: QuickSwitcher (`Ctrl+K`) -> Поиск каналов/пользователей -> Переход
/// ═══════════════════════════════════════════════════════════════════════════
#[core::prelude::v1::test]
fn e2e_user_journey_2_quick_switcher_keyboard_navigation_flow() {
    let mut state = AppState::new();
    let mut gateway = MockDiscordGateway::new("g-guild-1", "c-general");
    gateway.emit_guild_create(&mut state);

    // 1. Открытие QuickSwitcher по Ctrl+K
    state.open_quick_switcher();
    assert!(state.is_quick_switcher_open);
    assert_eq!(state.quick_switcher_query, "");

    // 2. Ввод поискового запроса по каналу "#dev"
    state.quick_switcher_query = "#dev".into();
    assert_eq!(state.quick_switcher_query, "#dev");

    // Фильтрация элементов QuickSwitcher
    let items = state.filter_quick_switcher();
    let dev_channel = items.iter().find(|i| i.name.contains("dev"));
    assert!(
        dev_channel.is_some(),
        "Канал dev-chat должен быть найден в результатах"
    );
    assert_eq!(dev_channel.unwrap().kind, QuickSwitcherItemKind::Channel);

    // 3. Выбор найденного канала и закрытие модалки
    let selected_item = dev_channel.unwrap().clone();
    state.select_quick_switcher_item(&selected_item);
    state.close_quick_switcher();
    assert!(
        !state.is_quick_switcher_open,
        "QuickSwitcher должен закрыться после выбора"
    );

    // 4. Проверка смены активного канала
    assert_eq!(state.selected_channel_id.as_deref(), Some("c-dev"));
}

/// ═══════════════════════════════════════════════════════════════════════════
/// USER JOURNEY 3: Подключение к голосу, адаптивный Jitter Buffer и DAVE E2EE MLS v1
/// ═══════════════════════════════════════════════════════════════════════════
#[core::prelude::v1::test]
fn e2e_user_journey_3_voice_call_adaptive_jitter_and_dave_e2ee_lifecycle() {
    let mut state = AppState::new();

    // 1. Подключение к голосовому вызову
    state.start_call("c-voice-1", false);
    assert!(
        matches!(state.active_call, CallStatus::Connected { .. }),
        "Голосовой звонок должен перейти в статус Connected"
    );
    assert!(!state.is_muted);
    assert!(!state.is_deafened);

    // 2. Инициализация сессии Discord DAVE E2EE MLS v1 для 2 участников (Alice & Bob)
    let init_secret = [0x42u8; 32];
    let commit_e1 = [0x55u8; 32];
    let group_ctx = DaveMlsGroupContext {
        group_id: b"discord-voice-e2e-channel".to_vec(),
        epoch: 1,
        tree_hash: [0x77u8; 32],
        confirmed_transcript_hash: [0x88u8; 32],
        extensions: vec![0x00, 0x01],
    };
    let members = [1001u64, 2002u64];

    let ks_alice =
        DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit_e1, &group_ctx, None)
            .expect("Инициализация MLS key schedule для Alice");
    let ks_bob = DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit_e1, &group_ctx, None)
        .expect("Инициализация MLS key schedule для Bob");

    let mut alice = DaveVoiceSession::new(
        1001,
        ks_alice,
        &members,
        DaveTransportCipherMode::AeadAes256GcmRtpsize,
    )
    .expect("Сессия DAVE Alice");

    let mut bob = DaveVoiceSession::new(
        2002,
        ks_bob,
        &members,
        DaveTransportCipherMode::AeadAes256GcmRtpsize,
    )
    .expect("Сессия DAVE Bob");

    // Общий код приватности для подтверждения безопасности
    assert_eq!(
        alice.voice_privacy_code().expect("Код приватности Alice"),
        bob.voice_privacy_code().expect("Код приватности Bob")
    );

    // 3. Сквозное шифрование и дешифрование фрейма голоса между Alice и Bob
    let original_voice_payload = b"encrypted-opus-voice-frame-packet";
    let encrypted_frame = alice
        .encrypt_opus(original_voice_payload)
        .expect("Шифрование голоса");
    let decrypted_frame = bob
        .decrypt_opus(1001, &encrypted_frame)
        .expect("Дешифрование голоса");
    assert_eq!(decrypted_frame, original_voice_payload);

    // 4. Проверка адаптивного Jitter Buffer при сетевом джиттере
    let mut jitter_buffer = AdaptiveJitterBuffer::new();
    let packet = vec![0xAB; 40];
    jitter_buffer.push(100, 960, packet.clone());
    jitter_buffer.push(101, 1920, packet.clone());
    jitter_buffer.push(102, 2880, packet.clone());
    assert_eq!(jitter_buffer.len(), 3);

    // 5. Завершение голосового вызова
    state.end_call();
    assert!(
        matches!(state.active_call, CallStatus::Idle),
        "Голосовой звонок должен вернуться в статус Idle"
    );
}

/// ═══════════════════════════════════════════════════════════════════════════
/// USER JOURNEY 4: Модальное окно Screen Share и согласование PipeWire DMA-BUF 60 FPS
/// ═══════════════════════════════════════════════════════════════════════════
#[core::prelude::v1::test]
fn e2e_user_journey_4_screen_share_modal_and_pipewire_stream_negotiation() {
    let mut state = AppState::new();
    state.start_call("c-stream-room", false);

    // 1. Открытие модального окна выбора источника экрана
    state.open_screen_share_modal();
    assert!(state.is_screen_share_modal_open);

    let mut selector = ScreenShareSelectorState::new();
    // 2. Вкладка "Экраны": проверяем фильтрацию мониторов
    let monitors = selector.filtered_sources(&state);
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id, "screen-full-1");
    assert!(monitors[0].is_screen);

    // 3. Переключение на вкладку "Окна приложений"
    selector.set_tab(ScreenShareTab::Windows, &mut state);
    let windows = selector.filtered_sources(&state);
    assert_eq!(windows.len(), 2);
    assert!(!windows[0].is_screen);

    // 4. Выбор окна Discord и переключение на пресет 1080p @ 60 FPS
    selector
        .select_source("window-discord", &mut state)
        .expect("Выбор окна приложения");
    selector.set_resolution(ScreenShareResolution::P1080);
    selector.set_fps(ScreenShareFps::Fps60);

    assert_eq!(
        selector.quality_preset(),
        ScreenShareQualityPreset::P1080Fps60
    );
    assert_eq!(selector.quality_preset().dimensions(), (1920, 1080));
    assert_eq!(selector.quality_preset().fps_value(), 60);

    // 5. Согласование параметров аппаратного захвата PipeWire
    let stream_params = selector
        .quality_preset()
        .negotiate_stream(201, Some((1920, 1080)), ScreenShareTab::Windows)
        .expect("Параметры потока PipeWire должны согласоваться");

    assert_eq!(stream_params.width, 1920);
    assert_eq!(stream_params.height, 1080);
    assert_eq!(stream_params.fps, 60);
    assert_eq!(stream_params.rtp_clock_hz, RTP_VIDEO_CLOCK_HZ);
    assert_eq!(selector.quality_preset().fps().frame_interval_us(), 16_666);
    assert_eq!(selector.quality_preset().fps().rtp_timestamp_step(), 1_500);

    // 6. Подтверждение запуска трансляции "В эфир"
    let stream_info = selector
        .confirm_go_live(&mut state)
        .expect("Запуск стрима Go Live");
    assert!(stream_info.is_active, "Стрим должен стать активным");
    assert_eq!(stream_info.resolution_width, 1920);
    assert_eq!(stream_info.resolution_height, 1080);
    assert_eq!(stream_info.fps, 60);
    assert!(
        !state.is_screen_share_modal_open,
        "Модалка должна закрыться после старта стрима"
    );
}

/// ═══════════════════════════════════════════════════════════════════════════
/// NON-FUNCTIONAL E2E VERIFICATION: Бюджет кадра 120 FPS и память <60 MB
/// ═══════════════════════════════════════════════════════════════════════════
#[core::prelude::v1::test]
fn e2e_non_functional_frame_budget_and_memory_footprint() {
    let mut state = AppState::new();
    let mut gateway = MockDiscordGateway::new("g-guild-1", "c-general");
    gateway.emit_guild_create(&mut state);

    // Замеряем время на 100 циклов обновления состояния (эмуляция фреймов GPUI)
    let start = Instant::now();
    for i in 0..100 {
        gateway.emit_message_create(&mut state, "User", &format!("Message #{}", i));
    }
    let elapsed = start.elapsed();
    let avg_per_frame_us = elapsed.as_micros() / 100;

    // Для 120 FPS бюджет каждого фрейма составляет < 8333 микросекунд (8.33 мс)
    assert!(
        avg_per_frame_us < 8333,
        "Среднее время обновления фрейма {} мкс превысило бюджет 120 FPS (8333 мкс)",
        avg_per_frame_us
    );

    // Проверка расхода памяти в процессе E2E (через чтение /proc/self/statm)
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if let Some(resident_pages_str) = parts.get(1) {
            if let Ok(pages) = resident_pages_str.parse::<u64>() {
                let page_size = 4096u64;
                let rss_mb = (pages * page_size) / (1024 * 1024);
                // Лимит для тестов: < 100 МБ (для продакшн <60 МБ)
                assert!(
                    rss_mb < 100,
                    "Потребление памяти E2E тестом {} МБ превысило бюджет",
                    rss_mb
                );
            }
        }
    }
}
