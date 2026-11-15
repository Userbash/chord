//! Contract tests for `RW51-EPIC3-T1`:
//! Integration of `twilight-model` standard deserializer `Event` into gateway dispatch ingestion.

use chord_client::data;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("chord-twilight-test-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("create gateway dirs");
    fs::write(root.join("gateway/002-READY.json"), r#"{"d":{"user":{"id":"777000","username":"pilot"},"guilds":[],"private_channels":[],"relationships":[],"users":[]}}"#).expect("write ready");
    root
}

#[test]
fn test_rw51_epic3_t1_message_and_guild_events_via_twilight() {
    let root = temp_root("events");
    data::set_live_session(Some("live-token".to_string()), None);

    let initial_ready = json!({
        "user": {
            "id": "777000",
            "username": "live_user",
            "global_name": "Live User",
            "premium_type": 0,
            "mfa_enabled": true
        },
        "guilds": [],
        "private_channels": [],
        "relationships": [],
        "users": []
    });
    data::ingest_live_dispatch("READY", &initial_ready);

    // 1. GUILD_CREATE event via twilight
    let guild_payload = json!({
        "id": "888000111222333444",
        "name": "Twilight Guild Alpha",
        "member_count": 10,
        "channels": [
            { "id": "999000111222333444", "type": 0, "name": "general", "position": 0 }
        ],
        "members": [],
        "roles": []
    });
    data::ingest_live_dispatch("GUILD_CREATE", &guild_payload);

    let r_guild = data::ready(&root).expect("ready after GUILD_CREATE");
    assert_eq!(r_guild.guild_count, 1);
    assert_eq!(r_guild.guilds[0].id, "888000111222333444");
    assert_eq!(r_guild.guilds[0].name, "Twilight Guild Alpha");

    // 1b. VOICE_STATE_UPDATE event via twilight
    let voice_payload = json!({
        "user_id": "777000",
        "channel_id": "999000111222333444",
        "guild_id": "888000111222333444",
        "session_id": "session_twilight_123",
        "deaf": false,
        "mute": false,
        "self_deaf": false,
        "self_mute": true,
        "self_video": false,
        "suppress": false
    });
    data::ingest_live_dispatch("VOICE_STATE_UPDATE", &voice_payload);

    let r_voice = data::ready(&root).expect("ready after VOICE_STATE_UPDATE");
    let vs = &r_voice.guilds[0].raw["voice_states"];
    assert!(vs.is_array());
    assert_eq!(vs.as_array().unwrap().len(), 1);
    assert_eq!(vs.as_array().unwrap()[0]["user_id"], "777000");
    assert_eq!(vs.as_array().unwrap()[0]["channel_id"], "999000111222333444");

    // 2. MESSAGE_CREATE event via twilight
    let chan_id = "999000111222333444";
    let msg_id = "111000111222333444";
    let msg_payload = json!({
        "id": msg_id,
        "channel_id": chan_id,
        "guild_id": "888000111222333444",
        "content": "Hello from twilight event!",
        "timestamp": "2026-11-16T00:00:00.000000+00:00",
        "edited_timestamp": null,
        "tts": false,
        "mention_everyone": false,
        "mentions": [],
        "mention_roles": [],
        "attachments": [],
        "embeds": [],
        "pinned": false,
        "type": 0,
        "author": {
            "id": "777000",
            "username": "live_user",
            "discriminator": "0001",
            "avatar": null
        }
    });
    data::ingest_live_dispatch("MESSAGE_CREATE", &msg_payload);

    let page = data::messages_page(&root, chan_id, None, Some(50)).expect("messages page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, msg_id);
    assert_eq!(page.items[0].content, "Hello from twilight event!");

    // 3. MESSAGE_UPDATE event via twilight
    let update_payload = json!({
        "id": msg_id,
        "channel_id": chan_id,
        "guild_id": "888000111222333444",
        "content": "Updated content from twilight event!"
    });
    data::ingest_live_dispatch("MESSAGE_UPDATE", &update_payload);

    let page_updated = data::messages_page(&root, chan_id, None, Some(50)).expect("messages page updated");
    assert_eq!(page_updated.items.len(), 1);
    assert_eq!(page_updated.items[0].content, "Updated content from twilight event!");

    // 4. MESSAGE_DELETE event via twilight
    let delete_payload = json!({
        "id": msg_id,
        "channel_id": chan_id,
        "guild_id": "888000111222333444"
    });
    data::ingest_live_dispatch("MESSAGE_DELETE", &delete_payload);

    let page_deleted = data::messages_page(&root, chan_id, None, Some(50)).expect("messages page deleted");
    assert_eq!(page_deleted.items.len(), 0);

    // 5. GUILD_DELETE event via twilight
    let guild_del = json!({
        "id": "888000111222333444",
        "unavailable": false
    });
    data::ingest_live_dispatch("GUILD_DELETE", &guild_del);

    let r_after_del = data::ready(&root).expect("ready after GUILD_DELETE");
    assert_eq!(r_after_del.guild_count, 0);

    data::set_live_session(None, None);
    let _ = fs::remove_dir_all(&root);
}
