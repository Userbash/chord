//! RW-40 / task-196: контрактные тесты расширенного IPC-диспетчера (`ipc_ext`).
//!
//! Проверяет:
//! 1. Десериализацию/сериализацию `ExtendedIpcCommand` (`#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]`)
//!    по всем 5 командам (`get_user_profile`, `ack_messages_bulk`, `add_reaction_empty_body`,
//!    `vote_poll`, `search_guild_messages`).
//! 2. `validate_and_build_plan(cmd: &ExtendedIpcCommand) -> Result<ExtendedIpcPlan, String>`:
//!    - точные методы (`GET`, `POST`, `PUT`), пути Discord v9 REST и JSON-тела (`Option<serde_json::Value>`);
//!    - валидацию Snowflake ID (только ASCII-цифры, непустые, не `"0"`, в пределах `1..=u64::MAX`);
//!    - валидацию границ `AckMessagesBulk` (`read_states`: `1..=100`, плюс валидация `channel_id`/`message_id` каждого элемента);
//!    - валидацию `AddReactionEmptyBody` (`emoji` не пустой, тело `None`);
//!    - валидацию `VotePoll` (`answer_ids`: `1..=10`, тело `{"answer_ids": [...]}`);
//!    - валидацию `SearchGuildMessages` (`offset <= 9975`, `limit` в `1..=25`, `content` опциональный).

use discord_gui_spike::ipc_ext::{
    validate_and_build_plan, BulkReadStateEntry, ExtendedIpcCommand, ExtendedIpcPlan,
};
use serde_json::json;

const VALID_USER_ID: &str = "1168176063113875531";
const VALID_GUILD_ID: &str = "1014537851418980372";
const VALID_CHANNEL_ID: &str = "1014537851418980375";
const VALID_MESSAGE_ID: &str = "1293192721237938187";

// ---------------------------------------------------------------------------
// 1. Serde-контракт ExtendedIpcCommand (tag = "cmd", content = "args", snake_case)
// ---------------------------------------------------------------------------

#[test]
fn serde_roundtrip_all_five_commands_matches_tagged_snake_case_wire_format() {
    let profile_json = json!({
        "cmd": "get_user_profile",
        "args": {
            "user_id": VALID_USER_ID,
            "with_mutual_guilds": true,
            "with_mutual_friends_count": false,
            "guild_id": VALID_GUILD_ID
        }
    });
    let cmd: ExtendedIpcCommand =
        serde_json::from_value(profile_json.clone()).expect("deserialize get_user_profile");
    assert_eq!(
        cmd,
        ExtendedIpcCommand::GetUserProfile {
            user_id: VALID_USER_ID.to_string(),
            with_mutual_guilds: true,
            with_mutual_friends_count: false,
            guild_id: Some(VALID_GUILD_ID.to_string()),
        }
    );
    assert_eq!(serde_json::to_value(&cmd).expect("serialize"), profile_json);

    let ack_json = json!({
        "cmd": "ack_messages_bulk",
        "args": {
            "read_states": [
                {
                    "channel_id": VALID_CHANNEL_ID,
                    "message_id": VALID_MESSAGE_ID
                },
                {
                    "channel_id": "1014537851418980376",
                    "message_id": "1293192721237938188",
                    "read_state_type": 2
                }
            ]
        }
    });
    let cmd_ack: ExtendedIpcCommand =
        serde_json::from_value(ack_json.clone()).expect("deserialize ack_messages_bulk");
    assert_eq!(
        cmd_ack,
        ExtendedIpcCommand::AckMessagesBulk {
            read_states: vec![
                BulkReadStateEntry {
                    channel_id: VALID_CHANNEL_ID.to_string(),
                    message_id: VALID_MESSAGE_ID.to_string(),
                    read_state_type: None,
                },
                BulkReadStateEntry {
                    channel_id: "1014537851418980376".to_string(),
                    message_id: "1293192721237938188".to_string(),
                    read_state_type: Some(2),
                },
            ]
        }
    );
    assert_eq!(serde_json::to_value(&cmd_ack).expect("serialize"), ack_json);

    let reaction_json = json!({
        "cmd": "add_reaction_empty_body",
        "args": {
            "channel_id": VALID_CHANNEL_ID,
            "message_id": VALID_MESSAGE_ID,
            "emoji": "%F0%9F%91%8D"
        }
    });
    let cmd_react: ExtendedIpcCommand =
        serde_json::from_value(reaction_json.clone()).expect("deserialize add_reaction_empty_body");
    assert_eq!(
        cmd_react,
        ExtendedIpcCommand::AddReactionEmptyBody {
            channel_id: VALID_CHANNEL_ID.to_string(),
            message_id: VALID_MESSAGE_ID.to_string(),
            emoji: "%F0%9F%91%8D".to_string(),
        }
    );
    assert_eq!(
        serde_json::to_value(&cmd_react).expect("serialize"),
        reaction_json
    );

    let poll_json = json!({
        "cmd": "vote_poll",
        "args": {
            "channel_id": VALID_CHANNEL_ID,
            "message_id": VALID_MESSAGE_ID,
            "answer_ids": [1, 3]
        }
    });
    let cmd_poll: ExtendedIpcCommand =
        serde_json::from_value(poll_json.clone()).expect("deserialize vote_poll");
    assert_eq!(
        cmd_poll,
        ExtendedIpcCommand::VotePoll {
            channel_id: VALID_CHANNEL_ID.to_string(),
            message_id: VALID_MESSAGE_ID.to_string(),
            answer_ids: vec![1, 3],
        }
    );
    assert_eq!(
        serde_json::to_value(&cmd_poll).expect("serialize"),
        poll_json
    );

    let search_json = json!({
        "cmd": "search_guild_messages",
        "args": {
            "guild_id": VALID_GUILD_ID,
            "content": "привет",
            "offset": 50,
            "limit": 25
        }
    });
    let cmd_search: ExtendedIpcCommand =
        serde_json::from_value(search_json.clone()).expect("deserialize search_guild_messages");
    assert_eq!(
        cmd_search,
        ExtendedIpcCommand::SearchGuildMessages {
            guild_id: VALID_GUILD_ID.to_string(),
            content: Some("привет".to_string()),
            offset: 50,
            limit: 25,
        }
    );
    assert_eq!(
        serde_json::to_value(&cmd_search).expect("serialize"),
        search_json
    );
}

#[test]
fn serde_rejects_unknown_command_tag() {
    let invalid = json!({
        "cmd": "unknown_command",
        "args": {}
    });
    assert!(serde_json::from_value::<ExtendedIpcCommand>(invalid).is_err());
}

// ---------------------------------------------------------------------------
// 2. GetUserProfile: позитивные, граничные и негативные сценарии
// ---------------------------------------------------------------------------

#[test]
fn get_user_profile_builds_exact_route_with_and_without_guild_id() {
    let without_guild = ExtendedIpcCommand::GetUserProfile {
        user_id: VALID_USER_ID.to_string(),
        with_mutual_guilds: true,
        with_mutual_friends_count: false,
        guild_id: None,
    };
    let plan =
        validate_and_build_plan(&without_guild).expect("valid profile plan without guild_id");
    assert_eq!(
        plan,
        ExtendedIpcPlan {
            method: "GET",
            path: format!(
                "/users/{VALID_USER_ID}/profile?with_mutual_guilds=true&with_mutual_friends_count=false"
            ),
            body: None,
        }
    );

    let with_guild = ExtendedIpcCommand::GetUserProfile {
        user_id: VALID_USER_ID.to_string(),
        with_mutual_guilds: false,
        with_mutual_friends_count: true,
        guild_id: Some(VALID_GUILD_ID.to_string()),
    };
    let plan_with_guild =
        validate_and_build_plan(&with_guild).expect("valid profile plan with guild_id");
    assert_eq!(
        plan_with_guild,
        ExtendedIpcPlan {
            method: "GET",
            path: format!(
                "/users/{VALID_USER_ID}/profile?with_mutual_guilds=false&with_mutual_friends_count=true&guild_id={VALID_GUILD_ID}"
            ),
            body: None,
        }
    );
}

#[test]
fn get_user_profile_accepts_min_positive_and_u64_max_snowflakes_and_rejects_invalid() {
    let max_id = u64::MAX.to_string();
    let min_positive = ExtendedIpcCommand::GetUserProfile {
        user_id: "1".to_string(),
        with_mutual_guilds: true,
        with_mutual_friends_count: true,
        guild_id: Some(max_id.clone()),
    };
    let plan =
        validate_and_build_plan(&min_positive).expect("1 and u64::MAX must be valid snowflakes");
    assert_eq!(
        plan.path,
        format!("/users/1/profile?with_mutual_guilds=true&with_mutual_friends_count=true&guild_id={max_id}")
    );

    for bad_id in [
        "",
        "0",
        "0000",
        "abc",
        "123a",
        "-1",
        " 123",
        "123 ",
        "18446744073709551616",
    ] {
        let bad_user = ExtendedIpcCommand::GetUserProfile {
            user_id: bad_id.to_string(),
            with_mutual_guilds: true,
            with_mutual_friends_count: false,
            guild_id: None,
        };
        assert!(
            validate_and_build_plan(&bad_user).is_err(),
            "user_id={bad_id:?} must be rejected"
        );

        let bad_guild = ExtendedIpcCommand::GetUserProfile {
            user_id: VALID_USER_ID.to_string(),
            with_mutual_guilds: true,
            with_mutual_friends_count: false,
            guild_id: Some(bad_id.to_string()),
        };
        assert!(
            validate_and_build_plan(&bad_guild).is_err(),
            "guild_id={bad_id:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. AckMessagesBulk: позитивные, граничные (1..=100) и негативные сценарии
// ---------------------------------------------------------------------------

#[test]
fn ack_messages_bulk_builds_post_read_states_ack_bulk_plan() {
    let cmd = ExtendedIpcCommand::AckMessagesBulk {
        read_states: vec![
            BulkReadStateEntry {
                channel_id: VALID_CHANNEL_ID.to_string(),
                message_id: VALID_MESSAGE_ID.to_string(),
                read_state_type: None,
            },
            BulkReadStateEntry {
                channel_id: "1014537851418980376".to_string(),
                message_id: "1293192721237938188".to_string(),
                read_state_type: Some(2),
            },
        ],
    };
    let plan = validate_and_build_plan(&cmd).expect("valid bulk ack plan");
    assert_eq!(plan.method, "POST");
    assert_eq!(plan.path, "/read-states/ack-bulk");
    assert_eq!(
        plan.body,
        Some(json!({
            "read_states": [
                {
                    "channel_id": VALID_CHANNEL_ID,
                    "message_id": VALID_MESSAGE_ID
                },
                {
                    "channel_id": "1014537851418980376",
                    "message_id": "1293192721237938188",
                    "read_state_type": 2
                }
            ]
        }))
    );
}

#[test]
fn ack_messages_bulk_enforces_1_to_100_bounds_and_snowflake_ids() {
    // 0 entries -> Err
    let empty_cmd = ExtendedIpcCommand::AckMessagesBulk {
        read_states: Vec::new(),
    };
    assert!(validate_and_build_plan(&empty_cmd).is_err());

    // 100 entries -> Ok
    let hundred_entries: Vec<BulkReadStateEntry> = (1..=100)
        .map(|i| BulkReadStateEntry {
            channel_id: (1_000_000_000_000_000_000u64 + i).to_string(),
            message_id: (2_000_000_000_000_000_000u64 + i).to_string(),
            read_state_type: None,
        })
        .collect();
    let max_cmd = ExtendedIpcCommand::AckMessagesBulk {
        read_states: hundred_entries.clone(),
    };
    let max_plan = validate_and_build_plan(&max_cmd).expect("100 entries must be accepted");
    assert_eq!(max_plan.method, "POST");
    assert_eq!(max_plan.path, "/read-states/ack-bulk");
    assert_eq!(
        max_plan
            .body
            .as_ref()
            .and_then(|b| b["read_states"].as_array())
            .map(Vec::len),
        Some(100)
    );

    // 101 entries -> Err
    let mut hundred_one = hundred_entries;
    hundred_one.push(BulkReadStateEntry {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        read_state_type: None,
    });
    let too_many_cmd = ExtendedIpcCommand::AckMessagesBulk {
        read_states: hundred_one,
    };
    assert!(validate_and_build_plan(&too_many_cmd).is_err());

    // Invalid channel_id or message_id in entry -> Err
    for (bad_ch, bad_msg) in [
        ("0", VALID_MESSAGE_ID),
        ("", VALID_MESSAGE_ID),
        ("not-digits", VALID_MESSAGE_ID),
        (VALID_CHANNEL_ID, "0"),
        (VALID_CHANNEL_ID, ""),
        (VALID_CHANNEL_ID, "123/456"),
    ] {
        let bad_entry_cmd = ExtendedIpcCommand::AckMessagesBulk {
            read_states: vec![BulkReadStateEntry {
                channel_id: bad_ch.to_string(),
                message_id: bad_msg.to_string(),
                read_state_type: None,
            }],
        };
        assert!(
            validate_and_build_plan(&bad_entry_cmd).is_err(),
            "entry ({bad_ch:?}, {bad_msg:?}) must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. AddReactionEmptyBody: PUT .../reactions/{emoji}/@me с пустым телом (None)
// ---------------------------------------------------------------------------

#[test]
fn add_reaction_empty_body_builds_put_plan_with_none_body() {
    let cmd = ExtendedIpcCommand::AddReactionEmptyBody {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        emoji: "%F0%9F%91%8B".to_string(),
    };
    let plan = validate_and_build_plan(&cmd).expect("valid add_reaction_empty_body plan");
    assert_eq!(
        plan,
        ExtendedIpcPlan {
            method: "PUT",
            path: format!(
                "/channels/{VALID_CHANNEL_ID}/messages/{VALID_MESSAGE_ID}/reactions/%F0%9F%91%8B/@me"
            ),
            body: None,
        }
    );
}

#[test]
fn add_reaction_empty_body_rejects_invalid_snowflakes_and_empty_emoji() {
    for (ch, msg, emoji) in [
        ("", VALID_MESSAGE_ID, "👍"),
        ("0", VALID_MESSAGE_ID, "👍"),
        ("abc", VALID_MESSAGE_ID, "👍"),
        (VALID_CHANNEL_ID, "", "👍"),
        (VALID_CHANNEL_ID, "0", "👍"),
        (VALID_CHANNEL_ID, "xyz", "👍"),
        (VALID_CHANNEL_ID, VALID_MESSAGE_ID, ""),
        (VALID_CHANNEL_ID, VALID_MESSAGE_ID, "   "),
    ] {
        let cmd = ExtendedIpcCommand::AddReactionEmptyBody {
            channel_id: ch.to_string(),
            message_id: msg.to_string(),
            emoji: emoji.to_string(),
        };
        assert!(
            validate_and_build_plan(&cmd).is_err(),
            "add_reaction_empty_body ({ch:?}, {msg:?}, {emoji:?}) must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. VotePoll: PUT /channels/{id}/polls/{id}/answers/@me, answer_ids 1..=10
// ---------------------------------------------------------------------------

#[test]
fn vote_poll_builds_put_answers_me_with_answer_ids_body() {
    let cmd = ExtendedIpcCommand::VotePoll {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1, 4],
    };
    let plan = validate_and_build_plan(&cmd).expect("valid vote_poll plan");
    assert_eq!(
        plan,
        ExtendedIpcPlan {
            method: "PUT",
            path: format!("/channels/{VALID_CHANNEL_ID}/polls/{VALID_MESSAGE_ID}/answers/@me"),
            body: Some(json!({ "answer_ids": [1, 4] })),
        }
    );
}

#[test]
fn vote_poll_enforces_answer_ids_1_to_10_and_snowflake_ids() {
    // 1 answer_id -> Ok
    let one_cmd = ExtendedIpcCommand::VotePoll {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: vec![1],
    };
    assert!(validate_and_build_plan(&one_cmd).is_ok());

    // 10 answer_ids -> Ok
    let ten_cmd = ExtendedIpcCommand::VotePoll {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: (1..=10).collect(),
    };
    let ten_plan = validate_and_build_plan(&ten_cmd).expect("10 answer_ids must be valid");
    assert_eq!(
        ten_plan.body,
        Some(json!({ "answer_ids": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] }))
    );

    // 0 answer_ids -> Err
    let empty_cmd = ExtendedIpcCommand::VotePoll {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: Vec::new(),
    };
    assert!(validate_and_build_plan(&empty_cmd).is_err());

    // 11 answer_ids -> Err
    let eleven_cmd = ExtendedIpcCommand::VotePoll {
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        answer_ids: (1..=11).collect(),
    };
    assert!(validate_and_build_plan(&eleven_cmd).is_err());

    // Invalid channel_id or message_id -> Err
    for (ch, msg) in [
        ("0", VALID_MESSAGE_ID),
        ("", VALID_MESSAGE_ID),
        ("chan", VALID_MESSAGE_ID),
        (VALID_CHANNEL_ID, "0"),
        (VALID_CHANNEL_ID, ""),
        (VALID_CHANNEL_ID, "msg"),
    ] {
        let bad_cmd = ExtendedIpcCommand::VotePoll {
            channel_id: ch.to_string(),
            message_id: msg.to_string(),
            answer_ids: vec![1],
        };
        assert!(
            validate_and_build_plan(&bad_cmd).is_err(),
            "vote_poll ({ch:?}, {msg:?}) must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. SearchGuildMessages: GET /guilds/{id}/messages/search, offset <= 9975, limit 1..=25
// ---------------------------------------------------------------------------

#[test]
fn search_guild_messages_builds_get_plan_with_and_without_content() {
    let with_content = ExtendedIpcCommand::SearchGuildMessages {
        guild_id: VALID_GUILD_ID.to_string(),
        content: Some("release".to_string()),
        offset: 0,
        limit: 25,
    };
    let plan = validate_and_build_plan(&with_content).expect("valid search plan with content");
    assert_eq!(
        plan,
        ExtendedIpcPlan {
            method: "GET",
            path: format!(
                "/guilds/{VALID_GUILD_ID}/messages/search?content=release&offset=0&limit=25"
            ),
            body: None,
        }
    );

    let without_content = ExtendedIpcCommand::SearchGuildMessages {
        guild_id: VALID_GUILD_ID.to_string(),
        content: None,
        offset: 9975,
        limit: 1,
    };
    let plan_no_content =
        validate_and_build_plan(&without_content).expect("valid search plan without content");
    assert_eq!(
        plan_no_content,
        ExtendedIpcPlan {
            method: "GET",
            path: format!("/guilds/{VALID_GUILD_ID}/messages/search?offset=9975&limit=1"),
            body: None,
        }
    );
}

#[test]
fn search_guild_messages_enforces_offset_limit_and_guild_id_bounds() {
    // offset = 9976 (> 9975) -> Err
    let bad_offset = ExtendedIpcCommand::SearchGuildMessages {
        guild_id: VALID_GUILD_ID.to_string(),
        content: Some("test".to_string()),
        offset: 9976,
        limit: 25,
    };
    assert!(validate_and_build_plan(&bad_offset).is_err());

    // limit = 0 -> Err
    let zero_limit = ExtendedIpcCommand::SearchGuildMessages {
        guild_id: VALID_GUILD_ID.to_string(),
        content: Some("test".to_string()),
        offset: 0,
        limit: 0,
    };
    assert!(validate_and_build_plan(&zero_limit).is_err());

    // limit = 26 (> 25) -> Err
    let high_limit = ExtendedIpcCommand::SearchGuildMessages {
        guild_id: VALID_GUILD_ID.to_string(),
        content: Some("test".to_string()),
        offset: 0,
        limit: 26,
    };
    assert!(validate_and_build_plan(&high_limit).is_err());

    // Invalid guild_id -> Err
    for bad_guild in ["", "0", "guild", "18446744073709551616"] {
        let bad_cmd = ExtendedIpcCommand::SearchGuildMessages {
            guild_id: bad_guild.to_string(),
            content: None,
            offset: 0,
            limit: 10,
        };
        assert!(
            validate_and_build_plan(&bad_cmd).is_err(),
            "guild_id={bad_guild:?} must be rejected"
        );
    }
}
