use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chord_client::auth::AuthUser;
use chord_client::data;
use chord_client::guilds::{self, GuildJoinTransport, GuildTransportResponse};
use chord_client::permissions::{
    AllowedChannelAccess, ChannelOverwrite, PermissionCalculator, RolePermission, PERM_CONNECT,
    PERM_SEND_MESSAGES, PERM_SPEAK, PERM_VIEW_CHANNEL,
};
use serde_json::{json, Value};

fn repo_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Base64 of "999888777666555444" is "OTk5ODg4Nzc3NjY2NTU1NDQ0"
const TEST_USER_ID: &str = "999888777666555444";
const TEST_LIVE_TOKEN: &str = "test_live_token_mock_val_0123456789";

/// Base64 of "111222333444555666" is "MTExMjIyMzMzNDQ0NTU1NjY2"
const FOREIGN_USER_ID: &str = "111222333444555666";

static TEST_SERIAL_LOCK: Mutex<()> = Mutex::new(());

fn setup_clean_live_session() -> (std::sync::MutexGuard<'static, ()>, AuthUser) {
    let guard = TEST_SERIAL_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    data::set_live_session(None, None);
    data::clear_outbound_voice_op4_frames();
    data::clear_live_rest_mock_handler();
    let user = AuthUser {
        id: TEST_USER_ID.to_string(),
        username: "live_verifier".to_string(),
        global_name: Some("Live Verifier".to_string()),
        discriminator: "0".to_string(),
        avatar: None,
        bot: false,
    };
    data::set_live_session(Some(TEST_LIVE_TOKEN.to_string()), Some(&user));
    (guard, user)
}

#[test]
fn s1_forbids_002_ready_fixture_when_live_token_present_and_purges_foreign_state_cache() {
    let _user = setup_clean_live_session();

    // 1. Both data::resolve_live_token() and guilds::resolve_live_token() must detect the live token.
    assert_eq!(
        data::resolve_live_token().as_deref(),
        Some(TEST_LIVE_TOKEN),
        "data::resolve_live_token must return the active live token"
    );
    assert_eq!(
        guilds::resolve_live_token().as_deref(),
        Some(TEST_LIVE_TOKEN),
        "guilds::resolve_live_token must return the active live token"
    );

    // 2. Even when pointed at the real fixtures directory (which contains 002-READY.json with guild 384299970632876032),
    // ready(), read_state(), channels(), and guilds::is_fixture_ready_forbidden() must NEVER read 002-READY.json.
    let fixtures_root = repo_fixtures_dir();
    assert!(
        guilds::is_fixture_ready_forbidden(&fixtures_root.join("gateway/by-type/002-READY.json")),
        "002-READY.json must be strictly forbidden when resolve_live_token().is_some()"
    );

    let ready_payload = data::ready(&fixtures_root)
        .expect("ready() must succeed in live mode without reading 002-READY.json");
    assert_eq!(ready_payload.user.id, TEST_USER_ID);
    assert!(
        ready_payload.guilds.is_empty(),
        "ready() must not leak fixture guild 384299970632876032 from 002-READY.json when live token is present"
    );

    let read_state_entries = data::read_state(&fixtures_root)
        .expect("read_state() must succeed in live mode without reading 002-READY.json");
    assert!(
        read_state_entries.is_empty(),
        "read_state() must not leak fixture entries from 002-READY.json"
    );

    let all_channels = data::channels(&fixtures_root, None)
        .expect("channels(None) must succeed in live mode without reading 002-READY.json");
    assert!(
        all_channels.is_empty(),
        "channels(None) must not read fixture channels from 002-READY.json when live token is present"
    );
    assert!(
        data::channels(&fixtures_root, Some("384299970632876032")).is_err(),
        "channels(Some(fixture_guild)) must not find fixture guild from 002-READY.json when live token is present"
    );

    // Also verify ready() works even when root is a completely non-existent directory
    // (proving 002-READY.json is never opened from disk when resolve_live_token().is_some()).
    let missing_root = PathBuf::from("/tmp/non-existent-discord-fixtures-s1-test");
    let ready_without_disk = data::ready(&missing_root)
        .expect("ready() with live token must not attempt to read 002-READY.json from disk");
    assert_eq!(ready_without_disk.user.id, TEST_USER_ID);

    // 3. state_cache.json verification: mismatching state.user.id vs active token must be rejected.
    let tmp_dir =
        std::env::temp_dir().join(format!("discord_s1_cache_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let cache_path = tmp_dir.join("state_cache.json");

    let foreign_cache = json!({
        "user": {
            "id": FOREIGN_USER_ID,
            "username": "foreign_account"
        },
        "guilds": [
            {
                "id": "384299970632876032",
                "name": "Foreign Cached Guild",
                "isMember": true,
                "lurker": false
            }
        ]
    });
    std::fs::write(&cache_path, serde_json::to_string(&foreign_cache).unwrap()).unwrap();

    let rejected =
        data::load_verified_state_cache(&cache_path, TEST_LIVE_TOKEN, Some(TEST_USER_ID));
    assert!(
        rejected.is_none(),
        "state_cache.json belonging to another user.id must be rejected"
    );
    assert!(
        !cache_path.exists(),
        "foreign state_cache.json must be removed from disk upon user.id mismatch"
    );

    // 4. state_cache.json with matching user.id must purge foreign / unconfirmed / lurker guilds.
    let mixed_cache = json!({
        "user": {
            "id": TEST_USER_ID,
            "username": "live_verifier"
        },
        "guilds": [
            {
                "id": "700000000000000001",
                "name": "Confirmed Guild",
                "isMember": true,
                "lurker": false,
                "ownerUserId": TEST_USER_ID
            },
            {
                "id": "700000000000000002",
                "name": "Lurker Preview Guild",
                "isMember": false,
                "lurker": true
            },
            {
                "id": "700000000000000003",
                "name": "Unconfirmed Foreign Guild",
                "isMember": true,
                "lurker": false,
                "ownerUserId": FOREIGN_USER_ID
            },
            {
                "id": "384299970632876032",
                "name": "Fixture Demo Guild",
                "isMember": true,
                "lurker": false
            }
        ]
    });
    std::fs::write(&cache_path, serde_json::to_string(&mixed_cache).unwrap()).unwrap();

    let loaded = data::load_verified_state_cache(&cache_path, TEST_LIVE_TOKEN, Some(TEST_USER_ID))
        .expect("state_cache.json with matching user.id must load");
    let guilds_arr = loaded
        .get("guilds")
        .and_then(|v| v.as_array())
        .expect("guilds array must exist");
    let remaining_ids: Vec<&str> = guilds_arr
        .iter()
        .filter_map(|g| g.get("id").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(
        remaining_ids,
        vec!["700000000000000001"],
        "Only confirmed guilds for the active user may remain after state_cache.json sanitization"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
    data::set_live_session(None, None);
}

#[derive(Clone, Default)]
struct RecordedHttpCall {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Option<Value>,
}

struct MockVerificationTransport {
    calls: Arc<Mutex<Vec<RecordedHttpCall>>>,
    guild_id: String,
    verified_role_id: String,
    trigger_403_gate: bool,
}

impl GuildJoinTransport for MockVerificationTransport {
    fn request(
        &self,
        method: &str,
        path: &str,
        headers: &HashMap<String, String>,
        body: Option<&Value>,
    ) -> Result<GuildTransportResponse, String> {
        self.calls.lock().unwrap().push(RecordedHttpCall {
            method: method.to_string(),
            path: path.to_string(),
            headers: headers.clone(),
            body: body.cloned(),
        });

        let gid = &self.guild_id;
        let role_id = &self.verified_role_id;

        if method == "POST" && path.contains("/invites/") {
            if self.trigger_403_gate {
                return Ok(GuildTransportResponse {
                    status: 403,
                    body: json!({
                        "code": 40007,
                        "message": "Verification gate required",
                        "guild": { "id": gid, "name": "Protected Guild" }
                    }),
                });
            }
            return Ok(GuildTransportResponse {
                status: 200,
                body: json!({
                    "code": "verify-me",
                    "guild": {
                        "id": gid,
                        "name": "Protected Guild",
                        "features": ["MEMBER_VERIFICATION_GATE_ENABLED"]
                    },
                    "channel": {
                        "id": "880000000000000010",
                        "name": "general",
                        "type": 0
                    },
                    "member": {
                        "user": { "id": TEST_USER_ID },
                        "roles": [],
                        "pending": true
                    }
                }),
            });
        }

        if method == "GET" && path == format!("/api/v9/guilds/{gid}/member-verification") {
            return Ok(GuildTransportResponse {
                status: 200,
                body: json!({
                    "version": "2026-04-01T12:00:00.000000+00:00",
                    "description": "Server rules verification",
                    "form_fields": [
                        {
                            "field_type": "TERMS",
                            "label": "Принимаю правила сервера",
                            "values": ["Rule 1: Be respectful", "Rule 2: No spam"],
                            "required": true
                        }
                    ]
                }),
            });
        }

        if method == "PUT" && path == format!("/api/v9/guilds/{gid}/requests/@me") {
            return Ok(GuildTransportResponse {
                status: 200,
                body: json!({
                    "application_status": "APPROVED",
                    "guild_id": gid,
                    "user_id": TEST_USER_ID
                }),
            });
        }

        if method == "GET" && path == format!("/api/v9/guilds/{gid}/members/@me") {
            let accepted_rules = self.calls.lock().unwrap().iter().any(|c| {
                c.method == "PUT" && c.path == format!("/api/v9/guilds/{gid}/requests/@me")
            });
            return Ok(GuildTransportResponse {
                status: 200,
                body: if accepted_rules {
                    json!({
                        "user": { "id": TEST_USER_ID },
                        "roles": [role_id],
                        "pending": false,
                        "is_pending": false
                    })
                } else {
                    json!({
                        "user": { "id": TEST_USER_ID },
                        "roles": [],
                        "pending": true,
                        "is_pending": true
                    })
                },
            });
        }

        if method == "GET" && path == format!("/api/v9/guilds/{gid}") {
            return Ok(GuildTransportResponse {
                status: 200,
                body: json!({
                    "id": gid,
                    "name": "Protected Guild",
                    "owner_id": "100000000000000001",
                    "roles": [
                        {
                            "id": gid,
                            "name": "@everyone",
                            "permissions": PERM_VIEW_CHANNEL.to_string(),
                            "position": 0
                        },
                        {
                            "id": role_id,
                            "name": "Verified Member",
                            "permissions": (PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_CONNECT | PERM_SPEAK).to_string(),
                            "position": 1
                        }
                    ]
                }),
            });
        }

        if method == "GET" && path == format!("/api/v9/guilds/{gid}/channels") {
            return Ok(GuildTransportResponse {
                status: 200,
                body: json!([
                    {
                        "id": "880000000000000010",
                        "guild_id": gid,
                        "name": "general",
                        "type": 0,
                        "permission_overwrites": []
                    },
                    {
                        "id": "880000000000000020",
                        "guild_id": gid,
                        "name": "Voice Lounge",
                        "type": 2,
                        "permission_overwrites": []
                    }
                ]),
            });
        }

        Ok(GuildTransportResponse {
            status: 200,
            body: json!({}),
        })
    }
}

#[test]
fn s3_accept_invite_and_hydrate_live_guild_auto_completes_member_verification_and_recalculates_permissions(
) {
    let _user = setup_clean_live_session();
    let guild_id = "880000000000000001".to_string();
    let verified_role_id = "880000000000000099".to_string();

    // Part A: Test guilds::accept_invite_and_join_guild when pending == true
    let calls = Arc::new(Mutex::new(Vec::new()));
    let transport = MockVerificationTransport {
        calls: calls.clone(),
        guild_id: guild_id.clone(),
        verified_role_id: verified_role_id.clone(),
        trigger_403_gate: false,
    };

    let join_outcome =
        guilds::accept_invite_and_join_guild(&transport, "verify-me", &guild_id, TEST_USER_ID)
            .expect(
                "accept_invite_and_join_guild must succeed and auto-complete verification gate",
            );

    assert!(
        !join_outcome.pending,
        "pending must be false after rules confirmation"
    );
    assert_eq!(join_outcome.roles, vec![verified_role_id.clone()]);

    let recorded = calls.lock().unwrap().clone();
    let paths: Vec<(String, String)> = recorded
        .iter()
        .map(|c| (c.method.clone(), c.path.clone()))
        .collect();
    assert!(
        paths.contains(&(
            "GET".to_string(),
            format!("/api/v9/guilds/{guild_id}/member-verification")
        )),
        "Must request GET /api/v9/guilds/{{id}}/member-verification when pending == true, got: {paths:?}"
    );
    let put_req = recorded
        .iter()
        .find(|c| c.method == "PUT" && c.path == format!("/api/v9/guilds/{guild_id}/requests/@me"))
        .expect("Must send PUT /api/v9/guilds/{id}/requests/@me to confirm rules");
    let super_props = put_req
        .headers
        .get("X-Super-Properties")
        .or_else(|| put_req.headers.get("x-super-properties"))
        .expect("PUT /requests/@me must include X-Super-Properties header");
    assert!(
        !super_props.trim().is_empty(),
        "X-Super-Properties header must not be empty"
    );
    let put_body = put_req
        .body
        .as_ref()
        .expect("PUT /requests/@me must include JSON body");
    assert_eq!(
        put_body
            .get("form_fields")
            .and_then(|f| f.get(0))
            .and_then(|f| f.get("response"))
            .and_then(|v| v.as_bool()),
        Some(true),
        "form_fields[0].response must be set to true when accepting TERMS"
    );
    assert!(
        paths.contains(&(
            "GET".to_string(),
            format!("/api/v9/guilds/{guild_id}/members/@me")
        )),
        "Must reload member roles via GET /api/v9/guilds/{{id}}/members/@me after accepting rules"
    );

    // Verify recalculated channelPermissions via PermissionCalculator
    let text_access = join_outcome
        .channel_permissions
        .get("880000000000000010")
        .expect("channelPermissions must contain text channel 880000000000000010");
    assert!(text_access.can_view);
    assert!(
        text_access.can_send,
        "Verified role must grant can_send after recalculating channelPermissions"
    );
    let voice_access = join_outcome
        .channel_permissions
        .get("880000000000000020")
        .expect("channelPermissions must contain voice channel 880000000000000020");
    assert!(voice_access.can_view);
    assert!(
        voice_access.can_connect,
        "Verified role must grant can_connect after recalculating channelPermissions"
    );

    // Part B: Test data::hydrate_live_guild_blocking with 403 verification gate
    let hydrate_calls = Arc::new(Mutex::new(Vec::new()));
    let hydrate_transport = Arc::new(MockVerificationTransport {
        calls: hydrate_calls.clone(),
        guild_id: guild_id.clone(),
        verified_role_id: verified_role_id.clone(),
        trigger_403_gate: true,
    });
    data::set_live_rest_mock_handler(Some(hydrate_transport));

    // Seed guild in live session as pending/unverified
    data::ingest_live_dispatch(
        "READY",
        &json!({
            "user": { "id": TEST_USER_ID, "username": "live_verifier" },
            "guilds": [
                {
                    "id": guild_id,
                    "name": "Protected Guild",
                    "pending": true,
                    "lurker": false,
                    "isMember": true,
                    "my_roles": [],
                    "roles": [],
                    "channels": []
                }
            ]
        }),
    );

    let hydrated = data::hydrate_live_guild_blocking(&guild_id)
        .expect("hydrate_live_guild_blocking must succeed and auto-complete verification");
    assert_eq!(
        hydrated.get("pending").and_then(|v| v.as_bool()),
        Some(false),
        "Hydrated guild must have pending == false after auto-verification"
    );
    let ch_perms = hydrated
        .get("channelPermissions")
        .and_then(|v| v.as_object())
        .expect("Hydrated guild must contain recalculated channelPermissions map");
    assert_eq!(
        ch_perms
            .get("880000000000000020")
            .and_then(|v| v.get("canConnect"))
            .and_then(|v| v.as_bool()),
        Some(true),
        "Hydrated channelPermissions must reflect recalculated canConnect == true"
    );

    // Part C: Regression check for non-reentrant live_session().lock() during invite materialization
    let invite_payload = serde_json::to_vec(&json!({
        "code": "wVc2y3ET",
        "guild": {
            "id": "1340572775974305803",
            "name": "Game Room",
            "icon": null
        },
        "channel": {
            "id": "1340572775974305810",
            "name": "general",
            "type": 0
        },
        "approximate_member_count": 42,
        "approximate_presence_count": 12
    }))
    .unwrap();
    data::cache_resolved_invite_payload("/invites/wVc2y3ET", &invite_payload);
    let materialized = data::materialized_accept_body_for_invite_target("/invites/wVc2y3ET")
        .expect(
        "materialized_accept_body_for_invite_target must not deadlock when live token is active",
    );
    data::ingest_accepted_invite_payload(&materialized);
    data::ensure_local_guild_present("1340572775974305803");
    assert!(
        data::has_live_guild_or_channels("1340572775974305803"),
        "Joined guild 1340572775974305803 must be materialized in live session without mutex deadlock"
    );

    data::clear_live_rest_mock_handler();
    data::set_live_session(None, None);
}

#[test]
fn s4_blocks_gateway_op4_when_cannot_connect_with_exact_russian_reasons() {
    let _user = setup_clean_live_session();
    let guild_id = "910000000000000001";
    let voice_channel_id = "910000000000000022";

    let roles = vec![RolePermission {
        id: guild_id.to_string(),
        permissions: PERM_VIEW_CHANNEL, // no PERM_CONNECT
        position: 0,
    }];
    let overwrites: Vec<ChannelOverwrite> = vec![];

    // 1. Direct PermissionCalculator::evaluate_allowed_channel_access checks:
    let lurker_access: AllowedChannelAccess = PermissionCalculator::evaluate_allowed_channel_access(
        guild_id,
        "100000000000000001",
        TEST_USER_ID,
        &[],
        &roles,
        &overwrites,
        true,  // is_lurker
        false, // is_pending
    );
    assert!(!lurker_access.can_connect);
    assert!(!lurker_access.can_send);
    assert_eq!(
        lurker_access.denied_reason.as_deref(),
        Some("Сервер в режиме просмотра")
    );

    let pending_access: AllowedChannelAccess =
        PermissionCalculator::evaluate_allowed_channel_access(
            guild_id,
            "100000000000000001",
            TEST_USER_ID,
            &[],
            &roles,
            &overwrites,
            false, // is_lurker
            true,  // is_pending
        );
    assert!(!pending_access.can_connect);
    assert!(!pending_access.can_send);
    assert_eq!(
        pending_access.denied_reason.as_deref(),
        Some("Требуется принять правила сервера")
    );

    let role_denied_access: AllowedChannelAccess =
        PermissionCalculator::evaluate_allowed_channel_access(
            guild_id,
            "100000000000000001",
            TEST_USER_ID,
            &[],
            &roles,
            &overwrites,
            false,
            false,
        );
    assert!(role_denied_access.can_view);
    assert!(!role_denied_access.can_connect);
    assert_eq!(
        role_denied_access.denied_reason.as_deref(),
        Some("Роль не даёт права Подключаться (CONNECT)")
    );

    // 2. Verify data::connect_or_update_live_voice_ext blocks Gateway op:4 for all 3 cases:
    // Case A: Lurker mode -> 'Сервер в режиме просмотра'
    data::clear_outbound_voice_op4_frames();
    data::ingest_live_dispatch(
        "READY",
        &json!({
            "user": { "id": TEST_USER_ID, "username": "live_verifier" },
            "guilds": [{
                "id": guild_id,
                "name": "Preview Guild",
                "owner_id": "100000000000000001",
                "lurker": true,
                "isMember": false,
                "pending": false,
                "my_roles": [],
                "roles": [{ "id": guild_id, "permissions": (PERM_VIEW_CHANNEL | PERM_CONNECT).to_string(), "position": 0 }],
                "channels": [{ "id": voice_channel_id, "type": 2, "name": "Voice", "permission_overwrites": [] }]
            }]
        }),
    );
    let err_lurker = data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        false,
        false,
    )
    .expect_err("Must block op:4 in lurker/preview mode");
    assert_eq!(err_lurker, "Сервер в режиме просмотра");
    assert!(
        data::outbound_voice_op4_frames().is_empty(),
        "Gateway op:4 must not be sent when blocked by lurker mode"
    );

    // Case B: Pending verification -> 'Требуется принять правила сервера'
    data::ingest_live_dispatch(
        "READY",
        &json!({
            "user": { "id": TEST_USER_ID, "username": "live_verifier" },
            "guilds": [{
                "id": guild_id,
                "name": "Pending Guild",
                "owner_id": "100000000000000001",
                "lurker": false,
                "isMember": true,
                "pending": true,
                "my_roles": [],
                "roles": [{ "id": guild_id, "permissions": (PERM_VIEW_CHANNEL | PERM_CONNECT).to_string(), "position": 0 }],
                "channels": [{ "id": voice_channel_id, "type": 2, "name": "Voice", "permission_overwrites": [] }]
            }]
        }),
    );
    let err_pending = data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        false,
        false,
    )
    .expect_err("Must block op:4 when pending == true");
    assert_eq!(err_pending, "Требуется принять правила сервера");
    assert!(
        data::outbound_voice_op4_frames().is_empty(),
        "Gateway op:4 must not be sent when blocked by pending rules"
    );

    // Case C: Role lacks PERM_CONNECT -> 'Роль не даёт права Подключаться (CONNECT)'
    data::ingest_live_dispatch(
        "READY",
        &json!({
            "user": { "id": TEST_USER_ID, "username": "live_verifier" },
            "guilds": [{
                "id": guild_id,
                "name": "No Connect Role Guild",
                "owner_id": "100000000000000001",
                "lurker": false,
                "isMember": true,
                "pending": false,
                "my_roles": [],
                "roles": [{ "id": guild_id, "permissions": PERM_VIEW_CHANNEL.to_string(), "position": 0 }],
                "channels": [{ "id": voice_channel_id, "type": 2, "name": "Locked Voice", "permission_overwrites": [] }]
            }]
        }),
    );
    let err_no_connect = data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        false,
        false,
    )
    .expect_err("Must block op:4 when role lacks CONNECT");
    assert_eq!(err_no_connect, "Роль не даёт права Подключаться (CONNECT)");
    assert!(
        data::outbound_voice_op4_frames().is_empty(),
        "Gateway op:4 must not be sent when role lacks CONNECT"
    );

    data::set_live_session(None, None);
}

#[test]
fn v1_voice_phases_signaling_negotiating_connected_120ms_coalescing_and_15s_phantom_timeout() {
    let _user = setup_clean_live_session();
    let guild_id = "950000000000000001";
    let voice_channel_id = "950000000000000022";

    data::ingest_live_dispatch(
        "READY",
        &json!({
            "user": { "id": TEST_USER_ID, "username": "live_verifier" },
            "guilds": [{
                "id": guild_id,
                "name": "Voice Ready Guild",
                "owner_id": "100000000000000001",
                "lurker": false,
                "isMember": true,
                "pending": false,
                "my_roles": [],
                "roles": [{
                    "id": guild_id,
                    "permissions": (PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK).to_string(),
                    "position": 0
                }],
                "channels": [{
                    "id": voice_channel_id,
                    "type": 2,
                    "name": "General Voice",
                    "permission_overwrites": []
                }]
            }]
        }),
    );

    data::clear_outbound_voice_op4_frames();

    // 1. Click to connect -> phase must be 'signaling' (NOT 'connected')
    data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        false,
        false,
    )
    .expect("Voice connect with PERM_CONNECT must succeed");

    let status_after_click = data::live_voice_rtc_status_json();
    assert_eq!(
        status_after_click.get("state").and_then(|v| v.as_str()),
        Some("signaling"),
        "Phase immediately after click must be 'signaling', not optimistic 'connected'"
    );
    assert_eq!(
        status_after_click
            .get("connected")
            .and_then(|v| v.as_bool()),
        Some(false),
        "connected boolean must be false during 'signaling'"
    );
    assert_eq!(
        data::outbound_voice_op4_frames().len(),
        1,
        "Initial channel connect must send 1 Gateway op:4 frame"
    );

    // 2. Receive VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE -> phase transitions to 'negotiating'
    data::ingest_raw_gateway_dispatch(
        "VOICE_STATE_UPDATE",
        &json!({
            "guild_id": guild_id,
            "channel_id": voice_channel_id,
            "user_id": TEST_USER_ID,
            "session_id": "rtc_session_abc123",
            "self_mute": false,
            "self_deaf": false
        }),
    );
    data::ingest_raw_gateway_dispatch(
        "VOICE_SERVER_UPDATE",
        &json!({
            "guild_id": guild_id,
            "endpoint": "finland1234.discord.media:443",
            "token": "voice_token_xyz"
        }),
    );

    let status_negotiating = data::live_voice_rtc_status_json();
    assert_eq!(
        status_negotiating.get("state").and_then(|v| v.as_str()),
        Some("negotiating"),
        "Phase after VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE (before RTC op:4) must be 'negotiating'"
    );
    assert_eq!(
        status_negotiating
            .get("connected")
            .and_then(|v| v.as_bool()),
        Some(false),
        "connected must still be false during 'negotiating'"
    );

    // 3. Confirm RTC op:4 -> phase transitions to 'connected'
    data::confirm_live_rtc_op4_session_description(Some(&json!({
        "mode": "xsalsa20_poly1305",
        "media_session_id": "media_sess_01"
    })));

    let status_connected = data::live_voice_rtc_status_json();
    assert_eq!(
        status_connected.get("state").and_then(|v| v.as_str()),
        Some("connected"),
        "Phase must transition to 'connected' after VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE + RTC op:4"
    );
    assert_eq!(
        status_connected.get("connected").and_then(|v| v.as_bool()),
        Some(true),
        "connected must be true once phase is 'connected'"
    );

    // 4. Test 120ms coalescing for rapid mute/deaf toggles on the same channel:
    data::clear_outbound_voice_op4_frames();
    data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        true,
        false,
    )
    .unwrap();
    data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        false,
        false,
    )
    .unwrap();
    data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(voice_channel_id.to_string()),
        true,
        true,
    )
    .unwrap();

    // Immediately after rapid toggles (< 120ms), intermediate op:4 frames must be coalesced (0 sent yet)
    assert_eq!(
        data::outbound_voice_op4_frames().len(),
        0,
        "Rapid mute/deaf toggles within 120ms must be coalesced and not sent immediately"
    );

    // Wait for the 120ms coalescing window to elapse
    std::thread::sleep(Duration::from_millis(160));
    let coalesced_frames = data::outbound_voice_op4_frames();
    assert_eq!(
        coalesced_frames.len(),
        1,
        "Exactly 1 coalesced op:4 frame must be sent after 120ms window"
    );
    assert_eq!(
        coalesced_frames[0]
            .get("self_mute")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        coalesced_frames[0]
            .get("self_deaf")
            .and_then(|v| v.as_bool()),
        Some(true)
    );

    // 5. Test 15s phantom session auto-cleanup sending op:4 (channel_id: null):
    // Start a new voice connection that gets stuck in 'signaling' / 'negotiating' (never receives RTC op:4)
    let stuck_channel_id = "950000000000000033";
    data::ingest_live_dispatch(
        "CHANNEL_CREATE",
        &json!({
            "id": stuck_channel_id,
            "guild_id": guild_id,
            "type": 2,
            "name": "Stuck Voice",
            "permission_overwrites": []
        }),
    );
    data::clear_outbound_voice_op4_frames();
    data::connect_or_update_live_voice(
        Some(guild_id.to_string()),
        Some(stuck_channel_id.to_string()),
        false,
        false,
    )
    .expect("Connect to Stuck Voice must start signaling");
    assert_eq!(
        data::live_voice_rtc_status_json()
            .get("state")
            .and_then(|v| v.as_str()),
        Some("signaling")
    );
    data::clear_outbound_voice_op4_frames();

    // Advance / trigger handshake timeout of 15s (15_000ms)
    let timed_out = data::check_voice_handshake_timeout_with_elapsed(Duration::from_secs(15));
    assert!(
        timed_out,
        "Phantom voice session in 'signaling' for 15s must trigger timeout auto-cleanup"
    );

    let cleanup_frames = data::outbound_voice_op4_frames();
    assert_eq!(
        cleanup_frames.len(),
        1,
        "15s phantom session cleanup must send op:4 frame"
    );
    assert!(
        cleanup_frames[0]
            .get("channel_id")
            .map(|v| v.is_null())
            .unwrap_or(false),
        "15s phantom session cleanup op:4 must have channel_id: null, got: {:?}",
        cleanup_frames[0]
    );
    let status_after_timeout = data::live_voice_rtc_status_json();
    assert_eq!(
        status_after_timeout.get("state").and_then(|v| v.as_str()),
        Some("disconnected"),
        "Voice state must be reset to 'disconnected' after 15s phantom session auto-cleanup"
    );
    assert!(
        status_after_timeout
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("не подтвердил вход в голосовой канал"))
            .unwrap_or(false),
        "live_voice_rtc_status_json must preserve the timeout error message instead of overwriting it with null, got: {:?}",
        status_after_timeout.get("error")
    );

    data::set_live_session(None, None);
}

#[test]
fn u1_unified_workspace_crates_bridge_and_hallucinated_channel_purge() {
    let snap = chord_client::core_bridge::unified_engine_snapshot()
        .expect("unified_engine_snapshot must succeed across all linked workspace crates");
    assert_eq!(snap.etf_tag_count, 11);
    assert_eq!(snap.store_schema_version, 1);
    assert!(
        snap.store_in_memory_ok,
        "discord-store SQLite must initialize in memory"
    );
    assert_eq!(snap.i18n_total_locales, 36);
    assert_eq!(snap.i18n_resolved_ru, "ru");
    assert_eq!(snap.voice_opus_payload_type, 120);
    assert_eq!(snap.media_video_payload_type, 101);
    assert_eq!(snap.media_rtx_payload_type, 102);
    assert_eq!(snap.ui_component_contracts, 12);

    // Verify sanitize_state_cache_for_token strips hallucinated synthetic channels while keeping real channels
    let mut cached = json!({
        "user": { "id": TEST_USER_ID, "username": "live_verifier" },
        "guilds": [{
            "id": "1340572775974305803",
            "name": "Game Room",
            "isMember": true,
            "lurker": false,
            "ownerUserId": TEST_USER_ID,
            "channels": [
                { "id": "1340572775974305805", "name": "общее", "type": 0 },
                { "id": "1340572775974305925", "name": "медиа-и-мемы", "type": 0 },
                { "id": "1340572775974305944", "name": "модераторская", "type": 0 },
                { "id": "1340572775974305935", "name": "Игровая комната", "type": 2 },
                { "id": "invalid_overflow_channel_id", "name": "broken", "type": 0 }
            ]
        }]
    });
    assert!(data::sanitize_state_cache_for_token(
        &mut cached,
        TEST_LIVE_TOKEN,
        Some(TEST_USER_ID)
    ));
    let remaining_channels: Vec<&str> = cached["guilds"][0]["channels"]
        .as_array()
        .expect("channels array")
        .iter()
        .filter_map(|c| c.get("name").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(
        remaining_channels,
        vec!["общее"],
        "Hallucinated synthetic channels (#медиа-и-мемы, #модераторская, Игровая комната) and non-u64 IDs must be purged"
    );
}

#[test]
fn u2_no_fake_channels_on_invite_and_voice_op8_hello_60ms_heartbeat_rtt() {
    let (_guard, _user) = setup_clean_live_session();
    let root = repo_fixtures_dir();

    // 1. Accepting an invite stub (even with guild id 384299970632876032 or a 19-digit guild id)
    // must NEVER inject 002-READY.json fixture channels or synthetic channels (#правила, #общение, #модераторская).
    data::ingest_accepted_invite_payload(
        json!({
            "code": "clean-invite",
            "guild": { "id": "384299970632876032", "name": "Live Server" }
        })
        .to_string()
        .as_bytes(),
    );
    let stub_channels = data::channels(&root, Some("384299970632876032")).unwrap_or_default();
    assert!(
        stub_channels.is_empty(),
        "Invite stub must NOT generate fake channels (#правила, #общение, #модераторская) or read 002-READY.json before GUILD_CREATE arrives, got: {:?}",
        stub_channels
    );

    // 2. Voice WS v=8 OP 8 HELLO must schedule the initial OP 3 HEARTBEAT at 60ms (never waiting 13_750ms)
    // and OP 6 HEARTBEAT_ACK must compute real RTT.
    let mut v8 = chord_core::ws::PureRustVoiceV8State::new(
        "1340572775974305803",
        "1340572775974305806",
        TEST_USER_ID,
        "rtc_sess_live",
        "rtc_token_live",
    );
    let out_hello = v8
        .on_text_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750},"seq":4}"#)
        .expect("OP 8 HELLO must be handled");
    assert_eq!(v8.heartbeat_interval_ms(), 13_750);
    assert_eq!(
        v8.initial_heartbeat_delay_ms(),
        Some(chord_core::ws::VOICE_V8_INITIAL_HEARTBEAT_DELAY_MS),
        "OP 8 HELLO must set initial heartbeat delay to 60ms instead of waiting 13.75s"
    );
    assert_eq!(v8.seq_ack(), Some(4));
    assert_eq!(
        out_hello.len(),
        1,
        "OP 8 HELLO must immediately produce the initial OP 3 HEARTBEAT frame"
    );

    std::thread::sleep(Duration::from_millis(12));
    let out_ack = v8
        .on_text_frame(r#"{"op":6,"d":{"t":1700000000001}}"#)
        .expect("OP 6 HEARTBEAT_ACK must be handled");
    assert!(out_ack.is_empty());
    let rtt = v8
        .last_rtt_ms()
        .expect("OP 6 HEARTBEAT_ACK must compute real RTT");
    assert!(
        (8..500).contains(&rtt),
        "Measured RTT must reflect real elapsed time (~12ms), got {rtt}ms"
    );

    data::set_live_session(None, None);
}

#[test]
fn task_810_main_accept_invite_wires_accept_invite_and_join_guild_and_auto_confirms_rules_screening(
) {
    let (_guard, _user) = setup_clean_live_session();

    // 1. Source wiring check: main.rs::accept_invite must call guilds::accept_invite_and_join_guild
    // and guilds::apply_guild_join_outcome after accepting the invite.
    let main_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    let main_src = std::fs::read_to_string(&main_rs_path).expect("read src/main.rs");
    let accept_fn_start = main_src
        .find("async fn accept_invite(")
        .expect("async fn accept_invite must exist in src/main.rs");
    let accept_fn_slice = &main_src[accept_fn_start..];
    let accept_fn_end = accept_fn_slice
        .find("\n#[tauri::command]")
        .unwrap_or(accept_fn_slice.len());
    let accept_fn_body = &accept_fn_slice[..accept_fn_end];
    assert!(
        accept_fn_body.contains("guilds::accept_invite_and_join_guild"),
        "main.rs::accept_invite must wire guilds::accept_invite_and_join_guild"
    );
    assert!(
        accept_fn_body.contains("guilds::apply_guild_join_outcome"),
        "main.rs::accept_invite must wire guilds::apply_guild_join_outcome"
    );

    // 2. Rules Screening detection helper contract
    let invite_with_gate = json!({
        "code": "rules-gate",
        "guild": {
            "id": "890000000000000001",
            "name": "Rules Gate Server",
            "features": ["COMMUNITY", "MEMBER_VERIFICATION_GATE_ENABLED"]
        }
    });
    assert!(
        guilds::guild_requires_rules_screening(&invite_with_gate),
        "guild_requires_rules_screening must detect MEMBER_VERIFICATION_GATE_ENABLED in invite guild.features"
    );

    let guild_already_verified = json!({
        "id": "890000000000000001",
        "pending": false,
        "features": ["COMMUNITY", "MEMBER_VERIFICATION_GATE_ENABLED"]
    });
    assert!(
        !guilds::guild_requires_rules_screening(&guild_already_verified),
        "guild_requires_rules_screening must return false if pending is explicitly false"
    );

    // 3. Full join pipeline with MEMBER_VERIFICATION_GATE_ENABLED and stale pending on replica:
    // PUT /guilds/{id}/requests/@me must be sent with X-Super-Properties and response: true,
    // and the member must immediately transition to pending: false with unlocked channelPermissions.
    struct RulesGateTransport {
        calls: Arc<Mutex<Vec<(String, String, HashMap<String, String>, Option<Value>)>>>,
        guild_id: String,
        verified_role_id: String,
    }

    impl GuildJoinTransport for RulesGateTransport {
        fn request(
            &self,
            method: &str,
            path: &str,
            headers: &HashMap<String, String>,
            body: Option<&Value>,
        ) -> Result<GuildTransportResponse, String> {
            self.calls.lock().unwrap().push((
                method.to_string(),
                path.to_string(),
                headers.clone(),
                body.cloned(),
            ));
            let gid = &self.guild_id;
            match (method, path) {
                ("POST", p) if p.starts_with("/api/v9/invites/") => Ok(GuildTransportResponse {
                    status: 200,
                    body: json!({
                        "code": "rules-gate",
                        "guild": {
                            "id": gid,
                            "name": "Rules Gate Server",
                            "features": ["COMMUNITY", "MEMBER_VERIFICATION_GATE_ENABLED"]
                        }
                    }),
                }),
                ("GET", p) if p == format!("/api/v9/guilds/{gid}/member-verification") => {
                    Ok(GuildTransportResponse {
                        status: 200,
                        body: json!({
                            "version": "2026-04-19T12:00:00.000000+00:00",
                            "description": "Accept server rules",
                            "form_fields": [
                                {
                                    "field_type": "TERMS",
                                    "label": "Read and agree to server rules",
                                    "values": ["Be respectful", "No spam"],
                                    "required": true
                                }
                            ]
                        }),
                    })
                }
                ("PUT", p) if p == format!("/api/v9/guilds/{gid}/requests/@me") => {
                    Ok(GuildTransportResponse {
                        status: 201,
                        body: json!({
                            "application_status": "APPROVED",
                            "guild_id": gid
                        }),
                    })
                }
                ("GET", p) if p == format!("/api/v9/guilds/{gid}/members/@me") => {
                    // Simulate read replica still returning pending: true briefly after PUT /requests/@me
                    Ok(GuildTransportResponse {
                        status: 200,
                        body: json!({
                            "user": { "id": TEST_USER_ID },
                            "roles": [self.verified_role_id],
                            "pending": true,
                            "is_pending": true
                        }),
                    })
                }
                ("GET", p) if p == format!("/api/v9/guilds/{gid}") => Ok(GuildTransportResponse {
                    status: 200,
                    body: json!({
                        "id": gid,
                        "name": "Rules Gate Server",
                        "owner_id": "890000000000000099",
                        "features": ["COMMUNITY", "MEMBER_VERIFICATION_GATE_ENABLED"],
                        "roles": [
                            {
                                "id": gid,
                                "name": "@everyone",
                                "permissions": PERM_VIEW_CHANNEL.to_string(),
                                "position": 0
                            },
                            {
                                "id": self.verified_role_id,
                                "name": "Verified",
                                "permissions": (PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_CONNECT).to_string(),
                                "position": 1
                            }
                        ]
                    }),
                }),
                ("GET", p) if p == format!("/api/v9/guilds/{gid}/channels") => {
                    Ok(GuildTransportResponse {
                        status: 200,
                        body: json!([
                            {
                                "id": "890000000000000010",
                                "guild_id": gid,
                                "name": "general",
                                "type": 0,
                                "permission_overwrites": []
                            },
                            {
                                "id": "890000000000000020",
                                "guild_id": gid,
                                "name": "Voice Lobby",
                                "type": 2,
                                "permission_overwrites": []
                            }
                        ]),
                    })
                }
                _ => Err(format!("Unexpected request: {method} {path}")),
            }
        }
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    let guild_id = "890000000000000001".to_string();
    let verified_role_id = "890000000000000005".to_string();
    let transport = RulesGateTransport {
        calls: calls.clone(),
        guild_id: guild_id.clone(),
        verified_role_id: verified_role_id.clone(),
    };

    let outcome =
        guilds::accept_invite_and_join_guild(&transport, "rules-gate", &guild_id, TEST_USER_ID)
            .expect(
            "accept_invite_and_join_guild must succeed for MEMBER_VERIFICATION_GATE_ENABLED guild",
        );

    let recorded = calls.lock().unwrap();
    let put_req = recorded
        .iter()
        .find(|(m, p, _, _)| m == "PUT" && p == &format!("/api/v9/guilds/{guild_id}/requests/@me"))
        .expect(
            "PUT /api/v9/guilds/{id}/requests/@me must be called automatically for Rules Screening",
        );
    assert!(
        put_req
            .2
            .get("X-Super-Properties")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
        "PUT /requests/@me must include non-empty X-Super-Properties header"
    );
    let put_body = put_req
        .3
        .as_ref()
        .expect("PUT /requests/@me must include JSON body");
    assert_eq!(
        put_body["form_fields"][0]["response"],
        json!(true),
        "PUT /requests/@me must set response: true on TERMS field"
    );

    assert!(
        !outcome.pending,
        "Member must immediately transition to pending == false after PUT /requests/@me succeeds"
    );
    assert_eq!(outcome.roles, vec![verified_role_id.clone()]);
    let text_perms = outcome
        .channel_permissions
        .get("890000000000000010")
        .expect("text channel permissions must be computed");
    assert!(
        text_perms.can_send,
        "Text channel can_send must be unlocked immediately after auto-confirming Rules Screening"
    );
    let voice_perms = outcome
        .channel_permissions
        .get("890000000000000020")
        .expect("voice channel permissions must be computed");
    assert!(
        voice_perms.can_connect,
        "Voice channel can_connect must be unlocked immediately after auto-confirming Rules Screening"
    );

    // 4. Applying outcome to live session state must materialize guild with channels and pending == false
    guilds::apply_guild_join_outcome(&outcome);
    let root = repo_fixtures_dir();
    assert!(
        data::has_live_guild_or_channels(&guild_id),
        "Joined guild must be present in live session after apply_guild_join_outcome"
    );
    let live_channels = data::channels(&root, Some(&guild_id))
        .expect("data::channels must return materialized channels for joined guild");
    assert_eq!(
        live_channels.len(),
        2,
        "Both channels of the joined guild must be materialized in live session"
    );

    data::set_live_session(None, None);
}
