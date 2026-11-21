//! Contract tests for Task D-01: Gateway Initial State (`READY`).
//!
//! Verifies the full `READY` payload schema against the clean-room oracle documented
//! in `docs/re/gateway-protocol.md` and `docs/re/gateway-protocol.json`:
//! 1. Complete 40-key top-level `READY` payload schema parsed into strongly-typed structs.
//! 2. User accounts (`user` with 32 wire keys + 146 `users[]`, 17 `private_channels[]`, 60 `relationships[]`, 1 `sessions[]`).
//! 3. Guild lists (`7` hydrated guilds in `data_mode: full`, zero separate `GUILD_CREATE` needed).
//! 4. Channel structures (`272` channels across text `0`, voice `2`, category `4`, announcement `5`, forum `15` + `permission_overwrites`).
//! 5. Member roles (`132` roles across 7 guilds, `merged_members` hydration, permission bitmasks, position hierarchy).
//! 6. 9 mandatory testing dimensions (positive, negative, edge boundaries `0`/`u64::MAX`, error handling, empty states, recovery/fallback, diagnostics telemetry, secret redaction).

mod support;

use std::path::PathBuf;

use chord_codec::etf::EtfValue;
use chord_core::ready::{
    Channel, Guild, GuildMember, PermissionOverwrite, PrivateChannel, ReadState, Ready,
    ReadyDiagnostics, ReadyError, ReadySummary, Relationship, Role, Session, TOP_LEVEL_KEYS, User,
    UserGuildSettings, value,
};

fn load_ready_dispatch() -> EtfValue {
    support::ready_fixture::load_ready().expect("002-READY.json fixture must load")
}

fn parse_synthetic(json: &str) -> Ready {
    let etf = support::ready_fixture::parse_json(json).expect("synthetic JSON must parse");
    Ready::parse(&etf).expect("synthetic READY payload must parse")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// 1. Positive & Oracle Alignment: All 7 guilds in `002-READY.json` match the exact
/// clean-room inventory in `docs/re/gateway-protocol.json` (`ready.guildList`).
#[test]
fn d01_all_seven_guilds_match_gateway_protocol_oracle() {
    let frame = load_ready_dispatch();
    let ready = Ready::from_dispatch(&frame).expect("READY dispatch must parse");
    let summary = ReadySummary::from_dispatch(&frame).expect("ReadySummary must parse");

    assert_eq!(ready.top_level_keys, 40);
    assert_eq!(summary.top_keys, 40);
    assert_eq!(ready.v, Some(9));
    assert_eq!(ready.session_type.as_deref(), Some("normal"));
    assert_eq!(ready.resume_gateway_url.as_deref(), Some("REDACTED"));
    assert_eq!(ready.country_code.as_deref(), Some("DE"));
    assert_eq!(ready.subdivision_code.as_deref(), Some("HE"));
    assert_eq!(ready.api_code_version, Some(1));
    assert_eq!(ready.explicit_content_scan_version, Some(4));
    assert_eq!(ready.friend_suggestion_count, Some(0));
    assert!(!ready.ad_personalization_toggles_disabled);
    assert_eq!(ready.geo_ordered_rtc_regions.len(), 5);
    assert_eq!(ready.geo_ordered_rtc_regions[0], "frankfurt");

    // Expected 7 guilds from `docs/re/gateway-protocol.json` -> `ready.guildList`:
    // (id, name, channels, roles, emojis, stickers, member_count)
    let expected_guilds: &[(u64, &str, usize, usize, usize, usize, i64)] = &[
        (
            274_093_052_673_392_641,
            "Игровое сообщество ⚛MONOLIT⚛",
            55,
            31,
            71,
            29,
            157,
        ),
        (
            294_064_825_540_280_335,
            "Generations of Players",
            24,
            16,
            150,
            0,
            112,
        ),
        (
            338_001_249_918_255_106,
            "True Job Project",
            45,
            16,
            25,
            0,
            59,
        ),
        (700_482_297_027_231_765, "SISI 18+ PROJECT", 24, 6, 0, 0, 73),
        (942_355_007_271_997_450, "XOS", 35, 28, 71, 16, 44_749),
        (
            1_014_537_851_418_980_372,
            "ITSKIDON PC BOOST",
            83,
            34,
            46,
            6,
            24_228,
        ),
        (1_340_572_775_974_305_803, "Game Room", 6, 1, 0, 0, 3),
    ];

    assert_eq!(ready.guilds.len(), expected_guilds.len());
    assert_eq!(summary.guilds, expected_guilds.len());

    for (idx, &(id, name, channels, roles, emojis, stickers, member_count)) in
        expected_guilds.iter().enumerate()
    {
        let g: &Guild = &ready.guilds[idx];
        assert_eq!(g.id, id, "guild[{idx}] id mismatch");
        assert_eq!(g.name.as_deref(), Some(name), "guild[{idx}] name mismatch");
        assert_eq!(
            g.data_mode.as_deref(),
            Some("full"),
            "guild[{idx}] must be hydrated in data_mode=full"
        );
        assert!(g.lazy, "guild[{idx}] lazy flag must be true");
        assert!(!g.unavailable, "guild[{idx}] must be available");
        assert_eq!(g.channels.len(), channels, "guild[{idx}] channels count");
        assert_eq!(g.roles.len(), roles, "guild[{idx}] roles count");
        assert_eq!(g.emojis.len(), emojis, "guild[{idx}] emojis count");
        assert_eq!(g.stickers.len(), stickers, "guild[{idx}] stickers count");
        assert_eq!(
            g.member_count,
            Some(member_count),
            "guild[{idx}] member_count"
        );
        assert_eq!(
            g.members.len(),
            1,
            "guild[{idx}] must have 1 hydrated self-member from merged_members[{idx}]"
        );
        assert_eq!(g.members[0].user_id, ready.user.id);
    }
}

/// 2. Channel Structures & Member Roles Deep Verification:
/// Verifies channel types (`0`, `2`, `4`, `5`, `15`), parent hierarchy, permission overwrites,
/// role bitmasks, role positions, and member role assignments across the capture.
#[test]
fn d01_channel_structures_and_member_roles_schema_completeness() {
    let frame = load_ready_dispatch();
    let ready = Ready::from_dispatch(&frame).expect("READY dispatch must parse");

    let all_channels: Vec<&Channel> = ready.guilds.iter().flat_map(|g| &g.channels).collect();
    assert_eq!(all_channels.len(), 272);

    // Channel type distribution across all 7 guilds:
    // 0 (GUILD_TEXT): 137, 2 (GUILD_VOICE): 82, 4 (GUILD_CATEGORY): 43, 5 (GUILD_ANNOUNCEMENT): 1, 15 (GUILD_FORUM): 9
    let count_type = |t: i64| all_channels.iter().filter(|c| c.channel_type == t).count();
    assert_eq!(count_type(0), 137, "GUILD_TEXT count across 7 guilds");
    assert_eq!(count_type(2), 82, "GUILD_VOICE count across 7 guilds");
    assert_eq!(count_type(4), 43, "GUILD_CATEGORY count across 7 guilds");
    assert_eq!(count_type(5), 1, "GUILD_ANNOUNCEMENT count across 7 guilds");
    assert_eq!(count_type(15), 9, "GUILD_FORUM count across 7 guilds");

    // Verify permission overwrites exist and distinguish role (0) vs member (1) targets
    let all_overwrites: Vec<&PermissionOverwrite> = all_channels
        .iter()
        .flat_map(|c| &c.permission_overwrites)
        .collect();
    assert!(
        !all_overwrites.is_empty(),
        "permission_overwrites must be populated"
    );
    assert!(
        all_overwrites.iter().any(|ow| ow.target_type == 0),
        "must contain role permission overwrites (type 0)"
    );
    assert!(
        all_overwrites.iter().any(|ow| ow.target_type == 1),
        "must contain member permission overwrites (type 1)"
    );

    // Verify specific channels and roles from `docs/re/gateway-protocol.json` -> `guildCreate.guild`
    let itskidon = ready
        .guilds
        .iter()
        .find(|g| g.id == 1_014_537_851_418_980_372)
        .expect("ITSKIDON guild must exist");

    let roblox_chan = itskidon
        .channels
        .iter()
        .find(|c| c.id == 1_275_727_867_497_680_918)
        .expect("channel 1275727867497680918 must exist");
    assert_eq!(roblox_chan.name.as_deref(), Some("🎮┃roblox"));
    assert_eq!(roblox_chan.channel_type, 0);

    let mod_role: &Role = itskidon
        .roles
        .iter()
        .find(|r| r.id == 1_057_598_992_449_552_425)
        .expect("role 1057598992449552425 must exist");
    assert_eq!(mod_role.name.as_deref(), Some("Модератор"));
    assert_eq!(mod_role.position, 30);

    let bit32_role: &Role = itskidon
        .roles
        .iter()
        .find(|r| r.id == 1_088_036_342_321_123_398)
        .expect("role 1088036342321123398 must exist");
    assert_eq!(bit32_role.name.as_deref(), Some("32-BIT"));
    assert_eq!(bit32_role.position, 22);

    // Verify every guild has an @everyone role whose ID equals the guild ID (position 0)
    for g in &ready.guilds {
        let everyone = g.roles.iter().find(|r| r.id == g.id).unwrap_or_else(|| {
            panic!(
                "guild {} must have @everyone role with id == guild.id",
                g.id
            )
        });
        assert_eq!(everyone.position, 0);
        assert!(
            everyone.permissions > 0,
            "@everyone role must have base permissions"
        );
    }

    // Verify member roles reference valid guild role IDs in ITSKIDON
    let member: &GuildMember = &itskidon.members[0];
    assert!(
        !member.roles.is_empty(),
        "ITSKIDON member must have assigned roles"
    );
    for &role_id in &member.roles {
        assert!(
            itskidon.roles.iter().any(|r| r.id == role_id),
            "member role {role_id} must exist in guild.roles"
        );
    }
}

/// 3. User Accounts, Private Channels, Relationships, Sessions & Read States:
/// Verifies consistency across `user`, `users[]`, `private_channels[]`, `relationships[]`,
/// `sessions[]`, `read_state`, and `user_guild_settings`.
#[test]
fn d01_user_accounts_social_graph_and_read_state_consistency() {
    let frame = load_ready_dispatch();
    let ready = Ready::from_dispatch(&frame).expect("READY dispatch must parse");
    let summary = ReadySummary::from_dispatch(&frame).expect("ReadySummary must parse");

    let self_user: &User = &ready.user;
    assert_eq!(self_user.id, 384_299_970_632_876_032);
    assert_eq!(summary.user_id.as_deref(), Some("384299970632876032"));
    assert_eq!(summary.user_keys, Some(32));

    // All 17 private channels are DM channels (type 1) and reference users present in `ready.users`
    assert_eq!(ready.private_channels.len(), 17);
    assert_eq!(summary.private_channels, 17);
    for dm in &ready.private_channels {
        let dm: &PrivateChannel = dm;
        assert_eq!(dm.channel_type, 1);
        assert!(!dm.recipient_ids.is_empty());
        for &recipient_id in &dm.recipient_ids {
            assert!(
                ready.users.iter().any(|u| u.id == recipient_id),
                "DM recipient {recipient_id} must be present in ready.users"
            );
        }
    }

    // All 60 relationships have valid user_ids and types (18 friends type=1, 42 blocked type=2)
    assert_eq!(ready.relationships.len(), 60);
    assert_eq!(summary.relationships, 60);
    for rel in &ready.relationships {
        let rel: &Relationship = rel;
        assert_eq!(rel.id, rel.user_id);
        assert!(matches!(rel.relation_type, 1 | 2));
    }

    // Sessions, ReadState, and UserGuildSettings
    let sessions: &[Session] = &ready.sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(summary.sessions, 1);

    let rs: &ReadState = &ready.read_state;
    assert_eq!(rs.entries.len(), 157);
    assert_eq!(summary.read_state_entries, 157);
    assert_eq!(summary.read_state_partial, Some(false));
    assert_eq!(summary.read_state_version, Some(128_949));

    let ugs: &UserGuildSettings = &ready.user_guild_settings;
    assert_eq!(ugs.entries.len(), 58);
    assert_eq!(summary.user_guild_settings_entries, 58);
}

/// 4. Boundary & Edge Dimensions (Snowflake `0`, `u64::MAX`, 2000-char strings, bitmasks `u64::MAX`,
/// empty states, error handling, recovery/fallback, telemetry diagnostics, secret redaction).
#[test]
fn d01_nine_dimensions_boundaries_recovery_and_telemetry() {
    let topic_2000 = "a".repeat(2000);
    let json = format!(
        r#"{{
            "v": 9,
            "user": {{"id": "0", "username": "zero_boundary", "bot": "false"}},
            "session_id": "super_secret_session_token_value",
            "user_settings_proto": "super_secret_proto_base64_value",
            "experimental_future_key": "telemetry_tracked",
            "guilds": [
                {{
                    "id": "18446744073709551615",
                    "name": "Max U64 Guild",
                    "data_mode": "full",
                    "channels": [
                        {{
                            "id": "18446744073709551615",
                            "type": 0,
                            "name": "boundary-channel",
                            "topic": "{topic_2000}",
                            "permission_overwrites": [
                                {{
                                    "id": "0",
                                    "type": 0,
                                    "allow": "18446744073709551615",
                                    "deny": "0"
                                }}
                            ]
                        }},
                        {{
                            "id": "-1",
                            "type": 0
                        }}
                    ],
                    "roles": [
                        {{
                            "id": "18446744073709551615",
                            "name": "Max Permissions Role",
                            "permissions": "18446744073709551615",
                            "position": 999
                        }}
                    ]
                }}
            ]
        }}"#
    );

    let ready = parse_synthetic(&json);

    // Snowflake boundaries: 0 and u64::MAX
    assert_eq!(ready.user.id, 0);
    assert_eq!(ready.guilds.len(), 1);
    let g = &ready.guilds[0];
    assert_eq!(g.id, u64::MAX);
    assert_eq!(
        g.channels.len(),
        1,
        "negative channel id must be skipped via recovery"
    );
    assert_eq!(g.channels[0].id, u64::MAX);
    assert_eq!(g.channels[0].topic.as_deref().map(str::len), Some(2000));
    assert_eq!(g.channels[0].permission_overwrites[0].id, 0);
    assert_eq!(g.channels[0].permission_overwrites[0].allow, u64::MAX);
    assert_eq!(g.channels[0].permission_overwrites[0].deny, 0);
    assert_eq!(g.roles[0].id, u64::MAX);
    assert_eq!(g.roles[0].permissions, u64::MAX);

    // Telemetry diagnostics: unknown top-level key and malformed negative channel recorded
    let diag: &ReadyDiagnostics = &ready.diagnostics;
    assert_eq!(
        diag.unknown_top_level,
        vec!["experimental_future_key".to_string()]
    );
    assert_eq!(diag.malformed, vec!["guilds[0].channels[1]".to_string()]);

    // Security dimension: Debug output redacts `session_id` and `user_settings_proto`
    let debug_repr = format!("{ready:?}");
    assert!(!debug_repr.contains("super_secret_session_token_value"));
    assert!(!debug_repr.contains("super_secret_proto_base64_value"));
    assert!(debug_repr.contains("<redacted>"));

    // Negative / Error handling: overflow u64::MAX + 1 on anchor user.id returns InvalidId
    let overflow_etf = support::ready_fixture::parse_json(
        r#"{"user":{"id":"18446744073709551616"},"guilds":[],"session_id":"s"}"#,
    )
    .expect("JSON parses");
    assert_eq!(
        Ready::parse(&overflow_etf),
        Err(ReadyError::InvalidId {
            path: "d.user.id".to_string(),
            value: "18446744073709551616".to_string(),
        })
    );
}

/// 5. Documentation & Specification Contract:
/// Verifies that `docs/re/gateway-protocol.md` documents the full `READY` schema,
/// all 40 top-level keys, strongly-typed struct mapping (`chord_core::ready`),
/// user accounts, guild lists, channel structures, and member roles.
#[test]
fn d01_gateway_protocol_doc_specifies_full_ready_schema_and_typed_structs() {
    let doc_path = repo_root().join("docs/re/gateway-protocol.md");
    let doc = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", doc_path.display()));

    // Every one of the 40 top-level READY keys must be documented in `docs/re/gateway-protocol.md`
    for key in TOP_LEVEL_KEYS {
        assert!(
            doc.contains(key),
            "docs/re/gateway-protocol.md must document top-level READY key `{key}`"
        );
    }

    // Strongly-typed Rust structs and sub-schemas must be documented in `docs/re/gateway-protocol.md`
    for required_symbol in [
        "Ready",
        "ReadySummary",
        "ReadyDiagnostics",
        "ReadyError",
        "User",
        "Guild",
        "Channel",
        "PermissionOverwrite",
        "Role",
        "Emoji",
        "Sticker",
        "GuildMember",
        "PrivateChannel",
        "Relationship",
        "Session",
        "ClientInfo",
        "ReadState",
        "ReadStateEntry",
        "UserGuildSettings",
        "UserGuildSettingsEntry",
        "ChannelOverride",
        "MuteConfig",
        "TOP_LEVEL_KEYS",
    ] {
        assert!(
            doc.contains(required_symbol),
            "docs/re/gateway-protocol.md must document strongly-typed struct/symbol `{required_symbol}`"
        );
    }

    // Verify `value::kind` and helpers cover all 8 EtfValue variants
    assert_eq!(value::kind(&EtfValue::Nil), "null");
    assert_eq!(value::kind(&EtfValue::Atom("a".into())), "atom");
    assert_eq!(value::kind(&EtfValue::Int(1)), "int");
    assert_eq!(value::kind(&EtfValue::Float(1.0)), "float");
    assert_eq!(value::kind(&EtfValue::Str("s".into())), "str");
    assert_eq!(value::kind(&EtfValue::Tuple(vec![])), "tuple");
    assert_eq!(value::kind(&EtfValue::List(vec![])), "list");
    assert_eq!(value::kind(&EtfValue::Map(vec![])), "map");
}
