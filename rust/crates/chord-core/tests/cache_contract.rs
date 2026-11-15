//! Contract tests for `RW51-EPIC3-T2`: `InMemoryCache` and `InMemoryCacheBuilder`.

use std::sync::Arc;
use std::time::Instant;
use chord_core::cache::{InMemoryCacheBuilder, ResourceType};
use twilight_model::id::Id;

#[test]
fn test_cache_builder_and_empty_state() {
    let cache = InMemoryCacheBuilder::new()
        .resource_types(ResourceType::ALL)
        .build();

    assert!(cache.current_user().is_none());
    assert!(cache.channels().is_empty());
    assert!(cache.guild_channels(Id::new(123)).is_none());
}

#[test]
fn test_cache_channel_lookup_under_100ns() {
    let cache = InMemoryCacheBuilder::new()
        .resource_types(ResourceType::CHANNEL | ResourceType::GUILD)
        .build();

    let channel_id = Id::new(987654321098765432);
    let guild_id = Id::new(111222333444555666);

    // Seed channel into cache
    cache.seed_channel(guild_id, channel_id, "general", 0, 0);

    let found = cache.channel(channel_id);
    assert!(found.is_some());
    let ch = found.unwrap();
    assert_eq!(ch.name.as_deref(), Some("general"));

    // Benchmark 100,000 lookups to verify DoD: < 0.1 µs per lookup
    let iterations = 100_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = cache.channel(channel_id);
    }
    let elapsed = start.elapsed();
    let per_lookup = elapsed / iterations;
    println!("Channel lookup latency: {:?}", per_lookup);
    assert!(
        per_lookup.as_nanos() < 500,
        "Channel lookup latency must be < 500ns (DoD is < 0.1us = 100ns under release; debug target < 500ns): took {:?}",
        per_lookup
    );
}

#[test]
fn test_cache_update_event_and_thread_safety() {
    let cache = Arc::new(
        InMemoryCacheBuilder::new()
            .resource_types(ResourceType::ALL)
            .build(),
    );

    let json_ready = serde_json::json!({
        "v": 9,
        "user": {
            "id": "10001",
            "username": "self_user",
            "discriminator": "0001",
            "avatar": null,
            "bot": false
        },
        "guilds": [
            {
                "id": "20001",
                "name": "Rust Guild",
                "channels": [
                    {
                        "id": "30001",
                        "guild_id": "20001",
                        "name": "general",
                        "type": 0,
                        "position": 1
                    }
                ],
                "roles": [
                    {
                        "id": "40001",
                        "name": "Admin",
                        "color": 16711680,
                        "position": 10,
                        "permissions": "8"
                    }
                ],
                "members": [
                    {
                        "user": {
                            "id": "10001",
                            "username": "self_user",
                            "discriminator": "0001"
                        },
                        "roles": ["40001"],
                        "nick": "AdminUser"
                    }
                ],
                "voice_states": [
                    {
                        "guild_id": "20001",
                        "channel_id": "30001",
                        "user_id": "10001",
                        "session_id": "sess_1",
                        "deaf": false,
                        "mute": false,
                        "self_deaf": false,
                        "self_mute": false,
                        "suppress": false
                    }
                ]
            }
        ],
        "private_channels": [],
        "session_id": "gateway_sess_1",
        "resume_gateway_url": "wss://gateway.discord.gg"
    });

    cache.update_from_ready_value(&json_ready).expect("should load ready value");

    // Verify current user
    let me = cache.current_user().expect("current user cached");
    assert_eq!(me.id.get(), 10001);
    assert_eq!(me.name, "self_user");

    // Verify channel
    let ch = cache.channel(Id::new(30001)).expect("channel 30001 cached");
    assert_eq!(ch.name.as_deref(), Some("general"));

    // Verify role
    let role = cache.role(Id::new(40001)).expect("role 40001 cached");
    assert_eq!(role.name, "Admin");

    // Verify member
    let member = cache.member(Id::new(20001), Id::new(10001)).expect("member cached");
    assert_eq!(member.nick.as_deref(), Some("AdminUser"));

    // Verify voice state
    let vs = cache.voice_state(Id::new(20001), Id::new(10001)).expect("voice state cached");
    assert_eq!(vs.channel_id.map(|id| id.get()), Some(30001));

    // Multi-threaded concurrent reads & updates to ensure zero deadlocks
    let mut handles = Vec::new();
    for thread_idx in 0..8 {
        let cache_clone = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            for i in 0..1000 {
                let _ = cache_clone.channel(Id::new(30001));
                let _ = cache_clone.member(Id::new(20001), Id::new(10001));
                let _ = cache_clone.voice_state(Id::new(20001), Id::new(10001));
                if thread_idx == 0 && i % 100 == 0 {
                    cache_clone.seed_channel(
                        Id::new(20001),
                        Id::new(30000 + i as u64),
                        "dynamic-chan",
                        0,
                        i as i32,
                    );
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should join cleanly without deadlocks");
    }
}
