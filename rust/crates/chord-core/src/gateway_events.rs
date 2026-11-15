//! Discord gateway event deserialization and strongly typed routing
//! using `twilight-model::gateway::event::Event`.

use serde::de::DeserializeSeed;
use serde_json::Value;
use twilight_model::gateway::event::{DispatchEventWithTypeDeserializer, Event};

/// Deserializes a raw JSON value of a gateway dispatch into a strongly-typed [`Event`].
///
/// Returns `Ok(Event)` if the dispatch kind is supported by `twilight-model`
/// and the payload schema matches.
pub fn parse_dispatch_event(kind: &str, raw_data: &Value) -> Result<Event, serde_json::Error> {
    let deserializer = DispatchEventWithTypeDeserializer::new(kind);
    deserializer.deserialize(raw_data).map(Event::from)
}

/// Deserializes a raw JSON string of a gateway dispatch into a strongly-typed [`Event`].
pub fn parse_dispatch_event_str(kind: &str, json_str: &str) -> Result<Event, serde_json::Error> {
    let mut json_deserializer = serde_json::Deserializer::from_str(json_str);
    let deserializer = DispatchEventWithTypeDeserializer::new(kind);
    deserializer
        .deserialize(&mut json_deserializer)
        .map(Event::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_message_create() {
        let payload = json!({
            "id": "123456789012345678",
            "channel_id": "987654321098765432",
            "guild_id": "111222333444555666",
            "content": "Hello via twilight!",
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
                "id": "123456789012345678",
                "username": "tester",
                "discriminator": "0001",
                "avatar": null
            }
        });

        let event = parse_dispatch_event("MESSAGE_CREATE", &payload).expect("should parse message create");
        match event {
            Event::MessageCreate(msg) => {
                assert_eq!(msg.content, "Hello via twilight!");
                assert_eq!(msg.channel_id.get(), 987654321098765432);
                assert_eq!(msg.author.name, "tester");
            }
            other => panic!("expected MessageCreate, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_voice_state_update() {
        let payload = json!({
            "user_id": "123456789012345678",
            "channel_id": "987654321098765432",
            "guild_id": "111222333444555666",
            "session_id": "abcdef123456",
            "deaf": false,
            "mute": false,
            "self_deaf": false,
            "self_mute": false,
            "self_video": false,
            "suppress": false
        });

        let event = parse_dispatch_event("VOICE_STATE_UPDATE", &payload).expect("should parse voice state update");
        match event {
            Event::VoiceStateUpdate(vs) => {
                assert_eq!(vs.0.user_id.get(), 123456789012345678);
                assert_eq!(vs.0.channel_id.map(|c| c.get()), Some(987654321098765432));
                assert_eq!(vs.0.session_id, "abcdef123456");
            }
            other => panic!("expected VoiceStateUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_message_delete_and_bulk() {
        let single_payload = json!({
            "id": "111222333",
            "channel_id": "444555666",
            "guild_id": "777888999"
        });
        let event = parse_dispatch_event("MESSAGE_DELETE", &single_payload)
            .expect("should parse single message delete");
        match event {
            Event::MessageDelete(md) => {
                assert_eq!(md.id.get(), 111222333);
                assert_eq!(md.channel_id.get(), 444555666);
                assert_eq!(md.guild_id.map(|g| g.get()), Some(777888999));
            }
            other => panic!("expected MessageDelete, got {other:?}"),
        }

        let bulk_payload = json!({
            "ids": ["111222333", "222333444"],
            "channel_id": "444555666",
            "guild_id": "777888999"
        });
        let bulk_event = parse_dispatch_event("MESSAGE_DELETE_BULK", &bulk_payload)
            .expect("should parse message delete bulk");
        match bulk_event {
            Event::MessageDeleteBulk(mdb) => {
                assert_eq!(mdb.ids.len(), 2);
                assert_eq!(mdb.ids[0].get(), 111222333);
                assert_eq!(mdb.ids[1].get(), 222333444);
                assert_eq!(mdb.channel_id.get(), 444555666);
            }
            other => panic!("expected MessageDeleteBulk, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_guild_delete() {
        let payload = json!({
            "id": "1234567890",
            "unavailable": false
        });
        let event = parse_dispatch_event("GUILD_DELETE", &payload)
            .expect("should parse guild delete");
        match event {
            Event::GuildDelete(gd) => {
                assert_eq!(gd.id.get(), 1234567890);
                assert!(!gd.unavailable);
            }
            other => panic!("expected GuildDelete, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_dispatch_event_str_roundtrip() {
        let json_str = r#"{"id":"999000111","channel_id":"888000222","guild_id":null}"#;
        let event = parse_dispatch_event_str("MESSAGE_DELETE", json_str)
            .expect("should parse from JSON string");
        match event {
            Event::MessageDelete(md) => {
                assert_eq!(md.id.get(), 999000111);
                assert_eq!(md.channel_id.get(), 888000222);
                assert!(md.guild_id.is_none());
            }
            other => panic!("expected MessageDelete, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_unknown_or_malformed_event() {
        let payload = json!({ "invalid": true });
        let res = parse_dispatch_event("UNKNOWN_CUSTOM_EVENT", &payload);
        assert!(res.is_err(), "unknown event kind should return Err");
    }
}
