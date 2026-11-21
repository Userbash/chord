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

use std::sync::mpsc;

use dispatch::BackgroundEvent;
use state::{AppState, ChannelType, PresenceStatus};
use ui::AppLayout;

#[test]
fn attach_and_poll_background_events_respects_256_batch_cap_and_channel_states() {
    let mut state = AppState::empty();
    assert!(state.background_rx.is_none());
    assert_eq!(
        state.poll_background_events(),
        0,
        "polling without an attached receiver must return 0"
    );

    let (tx, rx) = mpsc::channel();
    state.attach_background_receiver(rx);
    assert!(state.background_rx.is_some());

    // Empty channel returns 0
    assert_eq!(state.poll_background_events(), 0);

    // Enqueue 300 events: first poll drains exact cap 256, second drains remaining 44
    for seq in 1..=300u64 {
        tx.send(BackgroundEvent::GatewayResumed { seq })
            .expect("send BackgroundEvent");
    }

    let first_batch = state.poll_background_events();
    assert_eq!(first_batch, 256);
    assert_eq!(state.last_gateway_seq, Some(256));

    let second_batch = state.poll_background_events();
    assert_eq!(second_batch, 44);
    assert_eq!(state.last_gateway_seq, Some(300));

    // Empty again after draining
    assert_eq!(state.poll_background_events(), 0);

    // Buffered events sent right before sender disconnect are drained, then returns 0
    tx.send(BackgroundEvent::GatewayResumed { seq: 301 })
        .expect("send final event");
    tx.send(BackgroundEvent::GatewayResumed { seq: 302 })
        .expect("send final event 2");
    drop(tx);

    assert_eq!(state.poll_background_events(), 2);
    assert_eq!(state.last_gateway_seq, Some(302));
    assert_eq!(
        state.poll_background_events(),
        0,
        "disconnected receiver must return 0 without panicking"
    );
}

#[test]
fn gateway_ready_and_resumed_hydrate_session_user_login_modal_and_guild_count() {
    let mut state = AppState::empty();
    state.init_clean_state();
    assert!(state.is_login_modal_open);
    assert!(state.guilds.is_empty());
    assert_eq!(state.gateway_session_id, None);
    assert_eq!(state.resume_gateway_url, None);
    assert_eq!(state.last_gateway_seq, None);

    let (tx, rx) = mpsc::channel();
    state.attach_background_receiver(rx);

    tx.send(BackgroundEvent::GatewayReady {
        session_id: "sess_live_abc123".into(),
        resume_gateway_url: Some("wss://gateway-us-east1-c.discord.gg".into()),
        user_id: "384299970632876032".into(),
        username: "pharos12510".into(),
        guild_count: 3,
    })
    .unwrap();
    tx.send(BackgroundEvent::GatewayResumed { seq: 332 })
        .unwrap();

    assert_eq!(state.poll_background_events(), 2);
    assert_eq!(
        state.gateway_session_id.as_deref(),
        Some("sess_live_abc123")
    );
    assert_eq!(
        state.resume_gateway_url.as_deref(),
        Some("wss://gateway-us-east1-c.discord.gg")
    );
    assert_eq!(state.current_user.id, "384299970632876032");
    assert_eq!(state.current_user.username, "pharos12510");
    assert_eq!(state.current_user.status, PresenceStatus::Online);
    assert!(
        !state.is_login_modal_open,
        "GatewayReady must close the login modal"
    );
    assert!(
        state.guilds.len() >= 3,
        "GatewayReady must ensure guilds.len() >= guild_count"
    );
    assert_eq!(state.last_gateway_seq, Some(332));

    // Pre-populated guilds larger than guild_count must not be truncated, and u64::MAX seq works
    let mut preloaded = AppState::new();
    let existing_len = preloaded.guilds.len();
    assert_eq!(existing_len, 2);
    preloaded.apply_background_event(BackgroundEvent::GatewayReady {
        session_id: "sess_2".into(),
        resume_gateway_url: None,
        user_id: "0".into(),
        username: "ZeroSnowflakeUser".into(),
        guild_count: 1,
    });
    preloaded.apply_background_event(BackgroundEvent::GatewayResumed { seq: u64::MAX });
    assert_eq!(preloaded.guilds.len(), existing_len);
    assert!(preloaded.guilds.len() >= 1);
    assert_eq!(preloaded.resume_gateway_url, None);
    assert_eq!(preloaded.last_gateway_seq, Some(u64::MAX));
}

#[test]
fn gateway_message_create_deduplicates_clears_typing_and_tracks_unread_state() {
    let mut state = AppState::new();
    state.select_guild(Some("guild-1".into()));
    state.select_channel("ch-general".into());

    // Clear ch-announcements initial unread flag so we verify GatewayMessageCreate sets it
    state.select_channel("ch-announcements".into());
    state.select_channel("ch-general".into());
    assert_eq!(state.selected_channel_id.as_deref(), Some("ch-general"));

    let ch_ann_before = state
        .guilds
        .iter()
        .find(|g| g.id == "guild-1")
        .and_then(|g| g.channels.iter().find(|c| c.id == "ch-announcements"))
        .unwrap();
    assert!(!ch_ann_before.unread);

    // Typing start on both selected and non-selected channels (with duplicate check)
    state.apply_background_event(BackgroundEvent::GatewayTypingStart {
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        user_id: "384299970632876032".into(),
        timestamp: 1_729_318_688,
    });
    state.apply_background_event(BackgroundEvent::GatewayTypingStart {
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        user_id: "384299970632876032".into(),
        timestamp: 1_729_318_689,
    });
    state.apply_background_event(BackgroundEvent::GatewayTypingStart {
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        user_id: "18446744073709551615".into(),
        timestamp: 1_729_318_690,
    });
    assert_eq!(
        state.typing_users.get("ch-general").map(Vec::as_slice),
        Some(
            [
                "384299970632876032".to_string(),
                "18446744073709551615".to_string()
            ]
            .as_slice()
        )
    );

    state.apply_background_event(BackgroundEvent::GatewayTypingStart {
        channel_id: "ch-announcements".into(),
        guild_id: Some("guild-1".into()),
        user_id: "384299970632876032".into(),
        timestamp: 1_729_318_691,
    });

    // 2000-character message boundary on currently selected channel
    let content_2000 = "Я".repeat(2000);
    let initial_general_len = state.messages.get("ch-general").map_or(0, Vec::len);
    let msg_ev = BackgroundEvent::GatewayMessageCreate {
        id: "1561632596763353163".into(),
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        author_id: "384299970632876032".into(),
        author_name: "ProBot".into(),
        content: content_2000.clone(),
        timestamp: Some("2026-10-19T06:18:08.463000+00:00".into()),
    };

    state.apply_background_event(msg_ev.clone());
    // Duplicate delivery of the same message ID must be ignored
    state.apply_background_event(msg_ev);

    let general_msgs = state.messages.get("ch-general").unwrap();
    assert_eq!(general_msgs.len(), initial_general_len + 1);
    let inserted = general_msgs.last().unwrap();
    assert_eq!(inserted.id, "1561632596763353163");
    assert_eq!(inserted.author.id, "384299970632876032");
    assert_eq!(inserted.author.username, "ProBot");
    assert_eq!(inserted.content.chars().count(), 2000);
    assert_eq!(inserted.content, content_2000);

    // Author removed from typing_users while other typing user remains
    assert_eq!(
        state.typing_users.get("ch-general").map(Vec::as_slice),
        Some(["18446744073709551615".to_string()].as_slice())
    );

    // Currently selected channel stays read (unread == false)
    let ch_general = state
        .guilds
        .iter()
        .find(|g| g.id == "guild-1")
        .and_then(|g| g.channels.iter().find(|c| c.id == "ch-general"))
        .unwrap();
    assert!(!ch_general.unread);

    // Message in non-selected channel marks it unread = true and clears author typing
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "1561632596763353999".into(),
        channel_id: "ch-announcements".into(),
        guild_id: Some("guild-1".into()),
        author_id: "384299970632876032".into(),
        author_name: "ProBot".into(),
        content: "New release notes!".into(),
        timestamp: None,
    });

    let ch_ann_after = state
        .guilds
        .iter()
        .find(|g| g.id == "guild-1")
        .and_then(|g| g.channels.iter().find(|c| c.id == "ch-announcements"))
        .unwrap();
    assert!(
        ch_ann_after.unread,
        "non-selected channel must be marked unread = true on GatewayMessageCreate"
    );
    assert!(
        state
            .typing_users
            .get("ch-announcements")
            .is_none_or(Vec::is_empty)
    );
}

#[test]
fn gateway_message_update_and_delete_handle_edits_bulk_deletes_and_missing_targets() {
    let mut state = AppState::empty();
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "100".into(),
        channel_id: "ch-live".into(),
        guild_id: Some("g-1".into()),
        author_id: "u-1".into(),
        author_name: "Alice".into(),
        content: "initial".into(),
        timestamp: Some("2026-11-14T10:00:00Z".into()),
    });
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "101".into(),
        channel_id: "ch-live".into(),
        guild_id: Some("g-1".into()),
        author_id: "u-2".into(),
        author_name: "Bob".into(),
        content: "second".into(),
        timestamp: Some("2026-11-14T10:01:00Z".into()),
    });
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "102".into(),
        channel_id: "ch-live".into(),
        guild_id: Some("g-1".into()),
        author_id: "u-3".into(),
        author_name: "Carol".into(),
        content: "third".into(),
        timestamp: Some("2026-11-14T10:02:00Z".into()),
    });

    // Update content + edited_timestamp on msg 100
    state.apply_background_event(BackgroundEvent::GatewayMessageUpdate {
        id: "100".into(),
        channel_id: "ch-live".into(),
        guild_id: Some("g-1".into()),
        content: Some("updated content".into()),
        edited_timestamp: Some("2026-11-14T10:05:00Z".into()),
    });

    let msg_100 = state
        .messages
        .get("ch-live")
        .unwrap()
        .iter()
        .find(|m| m.id == "100")
        .unwrap();
    assert_eq!(msg_100.content, "updated content");
    assert_eq!(
        msg_100.edited_timestamp.as_deref(),
        Some("2026-11-14T10:05:00Z")
    );

    // Partial update (content: None) must preserve existing content
    state.apply_background_event(BackgroundEvent::GatewayMessageUpdate {
        id: "100".into(),
        channel_id: "ch-live".into(),
        guild_id: Some("g-1".into()),
        content: None,
        edited_timestamp: Some("2026-11-14T10:06:00Z".into()),
    });
    let msg_100_after = state
        .messages
        .get("ch-live")
        .unwrap()
        .iter()
        .find(|m| m.id == "100")
        .unwrap();
    assert_eq!(msg_100_after.content, "updated content");
    assert_eq!(
        msg_100_after.edited_timestamp.as_deref(),
        Some("2026-11-14T10:06:00Z")
    );

    // Missing message ID and missing channel ID on update/delete are safe no-ops
    state.apply_background_event(BackgroundEvent::GatewayMessageUpdate {
        id: "999999".into(),
        channel_id: "ch-live".into(),
        guild_id: None,
        content: Some("ghost".into()),
        edited_timestamp: None,
    });
    state.apply_background_event(BackgroundEvent::GatewayMessageUpdate {
        id: "100".into(),
        channel_id: "ch-missing".into(),
        guild_id: None,
        content: Some("ghost".into()),
        edited_timestamp: None,
    });
    state.apply_background_event(BackgroundEvent::GatewayMessageDelete {
        ids: vec!["999999".into()],
        channel_id: "ch-missing".into(),
        guild_id: None,
    });
    assert_eq!(state.messages.get("ch-live").unwrap().len(), 3);

    // Bulk delete removes all matching IDs (100 and 102) and ignores non-existent IDs
    state.open_context_menu("100");
    state.apply_background_event(BackgroundEvent::GatewayMessageDelete {
        ids: vec!["100".into(), "102".into(), "404".into()],
        channel_id: "ch-live".into(),
        guild_id: Some("g-1".into()),
    });

    let remaining = state.messages.get("ch-live").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "101");
    assert!(state.active_context_menu.is_none());
}

#[test]
fn gateway_channel_lifecycle_creates_updates_all_channel_types_and_deletes_selected() {
    let mut state = AppState::new();
    let initial_g2_channels = state
        .guilds
        .iter()
        .find(|g| g.id == "guild-2")
        .unwrap()
        .channels
        .len();

    let channel_cases = [
        ("ch-new-text", 0i64, "rust-live", ChannelType::GuildText),
        ("ch-new-voice", 2i64, "Stage Voice", ChannelType::GuildVoice),
        (
            "ch-new-news",
            5i64,
            "release- radar",
            ChannelType::GuildAnnouncement,
        ),
        ("ch-new-dm", 1i64, "direct-peer", ChannelType::Dm),
        ("ch-new-gdm", 3i64, "squad-group", ChannelType::GroupDm),
    ];

    for (cid, kind, name, expected_type) in channel_cases {
        state.apply_background_event(BackgroundEvent::GatewayChannelLifecycle {
            event_name: "CHANNEL_CREATE".into(),
            id: cid.into(),
            guild_id: Some("guild-1".into()),
            kind: Some(kind),
            name: Some(name.into()),
        });

        let g1 = state.guilds.iter().find(|g| g.id == "guild-1").unwrap();
        let created = g1
            .channels
            .iter()
            .find(|c| c.id == cid)
            .expect("channel created in guild-1");
        assert_eq!(created.channel_type, expected_type);
        assert_eq!(created.name, name);
    }

    // Other guilds must remain untouched
    assert_eq!(
        state
            .guilds
            .iter()
            .find(|g| g.id == "guild-2")
            .unwrap()
            .channels
            .len(),
        initial_g2_channels
    );

    // CHANNEL_UPDATE mutates existing channel without duplicating it
    let g1_count_before_update = state
        .guilds
        .iter()
        .find(|g| g.id == "guild-1")
        .unwrap()
        .channels
        .len();
    state.apply_background_event(BackgroundEvent::GatewayChannelLifecycle {
        event_name: "CHANNEL_UPDATE".into(),
        id: "ch-new-text".into(),
        guild_id: Some("guild-1".into()),
        kind: Some(5),
        name: Some("rust-announcements-renamed".into()),
    });
    let g1_after_update = state.guilds.iter().find(|g| g.id == "guild-1").unwrap();
    assert_eq!(g1_after_update.channels.len(), g1_count_before_update);
    let updated = g1_after_update
        .channels
        .iter()
        .find(|c| c.id == "ch-new-text")
        .unwrap();
    assert_eq!(updated.name, "rust-announcements-renamed");
    assert_eq!(updated.channel_type, ChannelType::GuildAnnouncement);

    // Select ch-new-text, then CHANNEL_DELETE removes it and clears selected_channel_id
    state.select_channel("ch-new-text".into());
    assert_eq!(state.selected_channel_id.as_deref(), Some("ch-new-text"));

    state.apply_background_event(BackgroundEvent::GatewayChannelLifecycle {
        event_name: "CHANNEL_DELETE".into(),
        id: "ch-new-text".into(),
        guild_id: Some("guild-1".into()),
        kind: None,
        name: None,
    });

    let g1_after_delete = state.guilds.iter().find(|g| g.id == "guild-1").unwrap();
    assert!(
        g1_after_delete
            .channels
            .iter()
            .all(|c| c.id != "ch-new-text")
    );
    assert_eq!(
        state.selected_channel_id, None,
        "deleting currently selected channel must clear selected_channel_id"
    );

    // Deleting a non-selected channel keeps selected_channel_id intact
    state.select_channel("ch-general".into());
    state.apply_background_event(BackgroundEvent::GatewayChannelLifecycle {
        event_name: "CHANNEL_DELETE".into(),
        id: "ch-new-voice".into(),
        guild_id: Some("guild-1".into()),
        kind: None,
        name: None,
    });
    assert_eq!(state.selected_channel_id.as_deref(), Some("ch-general"));
}

#[test]
fn gateway_voice_state_server_update_and_ping_sync_current_user_only() {
    let mut state = AppState::new();
    state.current_user.id = "384299970632876032".into();
    state.connect_voice(
        "ch-voice-1".into(),
        "Голосовой 1 (CPAL Audio)".into(),
        "Rust Developers Community".into(),
    );
    assert!(!state.is_muted);
    assert!(!state.is_deafened);
    assert!(!state.is_camera_enabled);
    assert!(state.voice_session.is_some());

    // Another user's VOICE_STATE_UPDATE must NOT mutate self.is_muted or clear self.voice_session
    state.apply_background_event(BackgroundEvent::GatewayVoiceStateUpdate {
        guild_id: Some("guild-1".into()),
        channel_id: None,
        user_id: "999999999999999999".into(),
        session_id: "other_sess".into(),
        self_mute: true,
        self_deaf: true,
        self_video: true,
    });
    assert!(!state.is_muted);
    assert!(!state.is_deafened);
    assert!(!state.is_camera_enabled);
    assert!(state.voice_session.is_some());

    // Current user's VOICE_STATE_UPDATE updates mute/deaf/camera flags
    state.apply_background_event(BackgroundEvent::GatewayVoiceStateUpdate {
        guild_id: Some("guild-1".into()),
        channel_id: Some("ch-voice-1".into()),
        user_id: "384299970632876032".into(),
        session_id: "self_sess".into(),
        self_mute: true,
        self_deaf: true,
        self_video: true,
    });
    assert!(state.is_muted);
    assert!(state.is_deafened);
    assert!(state.is_camera_enabled);
    assert!(state.voice_session.is_some());

    // VOICE_SERVER_UPDATE (P1-1.1) stores pending_voice_server_update and updates voice_session.endpoint
    state.apply_background_event(BackgroundEvent::GatewayVoiceServerUpdate {
        guild_id: Some("guild-1".into()),
        endpoint: Some("c-arn02-5229f1fc.discord.media:2096".into()),
        token_present: true,
    });
    assert_eq!(
        state.pending_voice_server_update,
        Some((
            Some("guild-1".into()),
            Some("c-arn02-5229f1fc.discord.media:2096".into()),
            true
        ))
    );
    assert_eq!(
        state.voice_session.as_ref().unwrap().endpoint,
        "c-arn02-5229f1fc.discord.media:2096"
    );

    // VoicePing (P1-1.5) updates voice_session.ping_ms
    state.apply_background_event(BackgroundEvent::VoicePing(29));
    assert_eq!(state.voice_session.as_ref().unwrap().ping_ms, 29);

    // Current user leaving voice (channel_id == None) clears voice_session
    state.apply_background_event(BackgroundEvent::GatewayVoiceStateUpdate {
        guild_id: Some("guild-1".into()),
        channel_id: None,
        user_id: "384299970632876032".into(),
        session_id: "self_sess".into(),
        self_mute: false,
        self_deaf: false,
        self_video: false,
    });
    assert!(!state.is_muted);
    assert!(!state.is_deafened);
    assert!(!state.is_camera_enabled);
    assert!(state.voice_session.is_none());

    // VoicePing with no active voice_session is a safe no-op
    state.apply_background_event(BackgroundEvent::VoicePing(45));
    assert!(state.voice_session.is_none());
}

#[test]
fn gateway_disconnected_4004_logs_out_while_resumable_preserves_session() {
    let mut state = AppState::new();
    state.gateway_session_id = Some("sess_resume_keep".into());
    state.resume_gateway_url = Some("wss://gateway-us-east1-c.discord.gg".into());
    state.last_gateway_seq = Some(142);
    assert!(state.active_token.is_some());
    assert!(!state.is_login_modal_open);

    // Resumable close code records last_gateway_disconnect without logging out
    state.apply_background_event(BackgroundEvent::GatewayDisconnected {
        close_code: Some(4000),
        resumable: true,
        reason: "zstd stream reset".into(),
    });
    assert_eq!(
        state.last_gateway_disconnect,
        Some((Some(4000), true, "zstd stream reset".into()))
    );
    assert!(state.active_token.is_some());
    assert!(!state.is_login_modal_open);
    assert_eq!(
        state.gateway_session_id.as_deref(),
        Some("sess_resume_keep")
    );
    assert_eq!(state.last_gateway_seq, Some(142));

    // Fatal 4004 Authentication Failed logs out and opens login modal
    state.connect_voice("ch-voice-1".into(), "Voice".into(), "Guild".into());
    state.apply_background_event(BackgroundEvent::GatewayDisconnected {
        close_code: Some(4004),
        resumable: false,
        reason: "Authentication failed.".into(),
    });
    assert_eq!(
        state.last_gateway_disconnect,
        Some((Some(4004), false, "Authentication failed.".into()))
    );
    assert!(state.active_token.is_none());
    assert!(state.is_login_modal_open);
    assert!(state.voice_session.is_none());
    assert!(state.gateway_session_id.is_none());
    assert!(state.resume_gateway_url.is_none());
}

#[test]
fn join_and_leave_voice_channel_emit_opcode_4_frames_and_sync_voice_session_and_layout_api() {
    let mut state = AppState::new();
    assert!(state.outbound_gateway_frames.is_empty());
    assert!(state.voice_session.is_none());

    state.is_muted = true;
    state.is_deafened = false;
    state.is_camera_enabled = true;

    state.join_voice_channel(Some("guild-1".into()), "ch-voice-1".into());

    let session = state
        .voice_session
        .as_ref()
        .expect("join_voice_channel must create voice_session");
    assert_eq!(session.channel_id, "ch-voice-1");
    assert_eq!(session.channel_name, "Голосовой 1 (CPAL Audio)");
    assert_eq!(session.guild_name, "Rust Developers Community");

    assert_eq!(state.outbound_gateway_frames.len(), 1);
    let join_raw = &state.outbound_gateway_frames[0];
    assert!(
        join_raw.contains("\"op\":4"),
        "serialized Opcode 4 frame must contain \"op\":4, got: {join_raw}"
    );
    let join_json: serde_json::Value =
        serde_json::from_str(join_raw).expect("valid Opcode 4 join JSON");
    assert_eq!(join_json["op"], 4);
    assert_eq!(join_json["d"]["guild_id"], "guild-1");
    assert_eq!(join_json["d"]["channel_id"], "ch-voice-1");
    assert_eq!(join_json["d"]["self_mute"], true);
    assert_eq!(join_json["d"]["self_deaf"], false);
    assert_eq!(join_json["d"]["self_video"], true);

    state.leave_voice_channel();
    assert!(
        state.voice_session.is_none(),
        "leave_voice_channel must clear voice_session"
    );
    assert_eq!(state.outbound_gateway_frames.len(), 2);

    let leave_raw = &state.outbound_gateway_frames[1];
    assert!(
        leave_raw.contains("\"op\":4"),
        "serialized Opcode 4 leave frame must contain \"op\":4, got: {leave_raw}"
    );
    let leave_json: serde_json::Value =
        serde_json::from_str(leave_raw).expect("valid Opcode 4 leave JSON");
    assert_eq!(leave_json["op"], 4);
    assert_eq!(leave_json["d"]["guild_id"], "guild-1");
    assert!(leave_json["d"]["channel_id"].is_null());

    // Compile-time / runtime signature verification for AppLayout live bridge constructors
    let _new_fn: fn(&mut gpui::Context<AppLayout>) -> AppLayout = AppLayout::new;
    let _live_fn: fn(mpsc::Receiver<BackgroundEvent>, &mut gpui::Context<AppLayout>) -> AppLayout =
        AppLayout::with_live_receiver;
    let _loop_fn: fn(&mut gpui::Context<AppLayout>) = AppLayout::start_background_event_loop;
}
