//! RW-40 / task-269 (`BE-INT-12`) + RW-41 / TASK-814 (`G1 Backend`):
//! контрактные тесты единого типизированного роутера команд BFF (`bff_domains_router`).
//!
//! Проверяет:
//! 1. `SUPPORTED_BFF_DOMAINS`: точный список из 7 доменов (`slash_commands`, `onboarding`, `soundboard`, `stage_instances`, `automod`, `expressions`, `webhooks`).
//! 2. `is_supported_bff_domain(domain: &str) -> bool`: позитивные и негативные проверки (регистр, пробелы, пустая строка, неизвестные домены).
//! 3. `BffDomainAction` и `UpdateGuildOnboardingRequest`: serde round-trip всех вариантов (включая `ClickMessageComponent`).
//! 4. Успешная маршрутизация `dispatch_bff_domain_action` для всех вариантов, включая `ClickMessageComponent` (`POST /api/v9/interactions`, `type = 3`) и граничные Snowflake (`"1"` и 20 девяток).
//! 5. Проброс ошибок валидации `dispatch_bff_domain_action` из всех доменов (невалидные Snowflake `"0"`, пустые, 21 цифра, невалидные имена/лимиты/privacy_level/`custom_id`/`component_type`).
//! 6. `resolve_bff_domain_action`: HTTP-метод (`POST`/`PATCH`) вместе с маршрутом и телом в [`BffResolvedAction`].
//! 7. Wire-формат `BffDomainRequest`: буквальный JSON с внешней serde-теггировкой по имени варианта.

use chord_client::automod::{
    AUTOMOD_ACTION_BLOCK_MESSAGE, AUTOMOD_ACTION_TIMEOUT, AUTOMOD_EVENT_MESSAGE_SEND,
    AUTOMOD_TRIGGER_KEYWORD, AUTOMOD_TRIGGER_SPAM, MAX_TIMEOUT_DURATION_SECS,
};
use chord_client::bff_domains_router::{
    dispatch_bff_domain_action, is_supported_bff_domain, resolve_bff_domain_action,
    BffDomainAction, BffDomainRequest, BffResolvedAction, ClickMessageComponentRequest,
    CreateGuildEmojiRequest, CreateWebhookRequest, InteractionType, UpdateGuildOnboardingRequest,
    UpsertAutoModRuleRequest, SUPPORTED_BFF_DOMAINS,
};
use chord_client::expressions::MAX_EMOJI_BYTES;
use chord_client::onboarding::WelcomeScreenChannelEntry;
use chord_client::slash_commands::{
    InvokeSlashCommandRequest, COMMAND_TYPE_CHAT_INPUT, COMPONENT_TYPE_BUTTON,
    INTERACTION_TYPE_APPLICATION_COMMAND, INTERACTION_TYPE_MESSAGE_COMPONENT,
    MAX_COMMAND_NAME_CHARS, MAX_COMMAND_OPTIONS, MAX_COMPONENT_CUSTOM_ID_CHARS,
};
use chord_client::soundboard::SendSoundboardSoundRequest;
use chord_client::stage_instances::{CreateStageInstanceRequest, STAGE_PRIVACY_GUILD_ONLY};
use serde_json::json;

const VALID_APP_ID: &str = "1014537851418980372";
const VALID_GUILD_ID: &str = "2024537851418980373";
const VALID_CHANNEL_ID: &str = "3034537851418980374";
const VALID_CMD_ID: &str = "4044537851418980375";
const VALID_CMD_VER: &str = "5054537851418980376";
const VALID_NONCE: &str = "6064537851418980377";
const VALID_SOUND_ID: &str = "7074537851418980378";
const VALID_EMOJI_ID: &str = "8084537851418980379";
const VALID_RULE_ID: &str = "9094537851418980380";
const VALID_NEWS_CHANNEL_ID: &str = "1114537851418980381";
const VALID_MESSAGE_ID: &str = "1224537851418980382";

fn sample_slash_command_request() -> InvokeSlashCommandRequest {
    InvokeSlashCommandRequest {
        application_id: VALID_APP_ID.to_string(),
        guild_id: Some(VALID_GUILD_ID.to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        session_id: "  session-abc-123  ".to_string(),
        command_id: VALID_CMD_ID.to_string(),
        command_version: VALID_CMD_VER.to_string(),
        command_name: "ping_bot-1".to_string(),
        options: vec![json!({"name": "target", "type": 3, "value": "world"})],
        nonce: VALID_NONCE.to_string(),
    }
}

fn sample_click_component_request() -> ClickMessageComponentRequest {
    ClickMessageComponentRequest {
        application_id: VALID_APP_ID.to_string(),
        guild_id: Some(VALID_GUILD_ID.to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        message_id: VALID_MESSAGE_ID.to_string(),
        session_id: "  session-abc-123  ".to_string(),
        custom_id: "verify_captcha_button".to_string(),
        component_type: COMPONENT_TYPE_BUTTON,
        nonce: VALID_NONCE.to_string(),
    }
}

fn sample_onboarding_request() -> UpdateGuildOnboardingRequest {
    UpdateGuildOnboardingRequest {
        enabled: true,
        description: Some("Добро пожаловать на наш сервер!".to_string()),
        welcome_channels: vec![WelcomeScreenChannelEntry {
            channel_id: VALID_CHANNEL_ID.to_string(),
            description: "Правила и объявления сервера".to_string(),
            emoji_id: Some(VALID_EMOJI_ID.to_string()),
            emoji_name: Some("wave".to_string()),
        }],
    }
}

fn sample_soundboard_request() -> SendSoundboardSoundRequest {
    SendSoundboardSoundRequest {
        sound_id: VALID_SOUND_ID.to_string(),
        source_guild_id: Some(VALID_GUILD_ID.to_string()),
    }
}

fn sample_stage_instance_request() -> CreateStageInstanceRequest {
    CreateStageInstanceRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        topic: "  Открытая архитектура Rust BFF  ".to_string(),
        privacy_level: STAGE_PRIVACY_GUILD_ONLY,
        send_start_notification: Some(true),
    }
}

fn sample_automod_rule_request() -> UpsertAutoModRuleRequest {
    UpsertAutoModRuleRequest {
        name: "Блокировка спама".to_string(),
        event_type: AUTOMOD_EVENT_MESSAGE_SEND,
        trigger_type: AUTOMOD_TRIGGER_SPAM,
        keyword_filter: vec![],
        mention_total_limit: None,
        action_type: AUTOMOD_ACTION_BLOCK_MESSAGE,
        timeout_duration_secs: None,
        enabled: true,
    }
}

fn sample_emoji_request() -> CreateGuildEmojiRequest {
    CreateGuildEmojiRequest {
        name: "party_wave".to_string(),
        image_data_uri: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg".to_string(),
        roles: vec![VALID_GUILD_ID.to_string()],
    }
}

fn sample_webhook_request() -> CreateWebhookRequest {
    CreateWebhookRequest {
        name: "  Build Notifications  ".to_string(),
        avatar: None,
    }
}

// ---------------------------------------------------------------------------
// 1. SUPPORTED_BFF_DOMAINS и is_supported_bff_domain
// ---------------------------------------------------------------------------

#[test]
fn supported_bff_domains_constant_matches_exact_contract() {
    assert_eq!(
        SUPPORTED_BFF_DOMAINS,
        &[
            "slash_commands",
            "onboarding",
            "soundboard",
            "stage_instances",
            "automod",
            "expressions",
            "webhooks"
        ]
    );
    assert_eq!(SUPPORTED_BFF_DOMAINS.len(), 7);
}

#[test]
fn is_supported_bff_domain_accepts_only_registered_domains() {
    for domain in SUPPORTED_BFF_DOMAINS {
        assert!(
            is_supported_bff_domain(domain),
            "domain {domain:?} must be supported"
        );
    }

    for unsupported in [
        "",
        " ",
        "slash-commands",
        "SLASH_COMMANDS",
        "slash_commands ",
        " slash_commands",
        "Onboarding",
        "sound_board",
        "stage-instances",
        "polls",
        "forum",
        "unknown",
        "automod_rules",
        "auto_moderation",
        "emoji",
        "webhook",
    ] {
        assert!(
            !is_supported_bff_domain(unsupported),
            "domain {unsupported:?} must not be supported"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Serde round-trip базовых вариантов BffDomainAction (включая ClickMessageComponent)
// ---------------------------------------------------------------------------

#[test]
fn bff_domain_action_serde_roundtrip_all_variants() {
    let actions = vec![
        BffDomainAction::InvokeSlashCommand(sample_slash_command_request()),
        BffDomainAction::ClickMessageComponent(sample_click_component_request()),
        BffDomainAction::UpdateGuildOnboarding {
            guild_id: VALID_GUILD_ID.to_string(),
            request: sample_onboarding_request(),
        },
        BffDomainAction::SendSoundboardSound {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: sample_soundboard_request(),
        },
        BffDomainAction::CreateStageInstance(sample_stage_instance_request()),
    ];

    for action in actions {
        let serialized = serde_json::to_value(&action).expect("serialize BffDomainAction");
        let deserialized: BffDomainAction =
            serde_json::from_value(serialized).expect("deserialize BffDomainAction");
        assert_eq!(deserialized, action);
    }
}

// ---------------------------------------------------------------------------
// 3. Успешная маршрутизация вариантов BffDomainAction
// ---------------------------------------------------------------------------

#[test]
fn dispatch_invoke_slash_command_builds_interactions_route_and_body() {
    let req = sample_slash_command_request();
    let action = BffDomainAction::InvokeSlashCommand(req);
    let (route, body) =
        dispatch_bff_domain_action(&action).expect("valid InvokeSlashCommand must succeed");

    assert_eq!(route, "/api/v9/interactions");
    assert_eq!(
        body,
        json!({
            "type": INTERACTION_TYPE_APPLICATION_COMMAND,
            "application_id": VALID_APP_ID,
            "guild_id": VALID_GUILD_ID,
            "channel_id": VALID_CHANNEL_ID,
            "session_id": "session-abc-123",
            "nonce": VALID_NONCE,
            "data": {
                "version": VALID_CMD_VER,
                "id": VALID_CMD_ID,
                "name": "ping_bot-1",
                "type": COMMAND_TYPE_CHAT_INPUT,
                "options": [{"name": "target", "type": 3, "value": "world"}],
            }
        })
    );

    // Граничные Snowflake ("1" и 20 девяток) и guild_id = None
    let max_20 = "9".repeat(20);
    let boundary_action = BffDomainAction::InvokeSlashCommand(InvokeSlashCommandRequest {
        application_id: "1".to_string(),
        guild_id: None,
        channel_id: max_20.clone(),
        session_id: "s".to_string(),
        command_id: "1".to_string(),
        command_version: max_20.clone(),
        command_name: "a".repeat(MAX_COMMAND_NAME_CHARS),
        options: vec![],
        nonce: "1".to_string(),
    });
    let (route_b, body_b) =
        dispatch_bff_domain_action(&boundary_action).expect("boundary InvokeSlashCommand");
    assert_eq!(route_b, "/api/v9/interactions");
    assert_eq!(body_b["guild_id"], json!(null));
    assert_eq!(body_b["channel_id"], json!(max_20));
}

#[test]
fn dispatch_click_message_component_builds_type_3_interactions_route_and_body() {
    let req = sample_click_component_request();
    let action = BffDomainAction::ClickMessageComponent(req);
    let (route, body) =
        dispatch_bff_domain_action(&action).expect("valid ClickMessageComponent must succeed");

    assert_eq!(route, "/api/v9/interactions");
    assert_eq!(
        body,
        json!({
            "type": InteractionType::MessageComponent.as_u8(),
            "application_id": VALID_APP_ID,
            "guild_id": VALID_GUILD_ID,
            "channel_id": VALID_CHANNEL_ID,
            "message_id": VALID_MESSAGE_ID,
            "session_id": "session-abc-123",
            "nonce": VALID_NONCE,
            "data": {
                "component_type": COMPONENT_TYPE_BUTTON,
                "custom_id": "verify_captcha_button",
            }
        })
    );
    assert_eq!(body["type"], json!(INTERACTION_TYPE_MESSAGE_COMPONENT));

    let resolved = resolve_bff_domain_action(action).expect("resolve ClickMessageComponent");
    assert_eq!(resolved.method, "POST");
    assert_eq!(resolved.route, "/api/v9/interactions");
    assert_eq!(resolved.body, body);

    // Граничные Snowflake ("1" и 20 девяток), guild_id = None, custom_id = 100 символов
    let max_20 = "9".repeat(20);
    let boundary_action = BffDomainAction::ClickMessageComponent(ClickMessageComponentRequest {
        application_id: "1".to_string(),
        guild_id: None,
        channel_id: max_20.clone(),
        message_id: "1".to_string(),
        session_id: "s".to_string(),
        custom_id: "b".repeat(MAX_COMPONENT_CUSTOM_ID_CHARS),
        component_type: COMPONENT_TYPE_BUTTON,
        nonce: max_20.clone(),
    });
    let (route_b, body_b) =
        dispatch_bff_domain_action(&boundary_action).expect("boundary ClickMessageComponent");
    assert_eq!(route_b, "/api/v9/interactions");
    assert_eq!(body_b["type"], json!(3));
    assert_eq!(body_b["guild_id"], json!(null));
    assert_eq!(body_b["channel_id"], json!(max_20));
    assert_eq!(body_b["message_id"], json!("1"));
    assert_eq!(body_b["data"]["component_type"], json!(2));

    // Проверка буквального wire-формата и serde-алиаса "MessageComponent"
    let wire = json!({
        "ClickMessageComponent": {
            "application_id": VALID_APP_ID,
            "guild_id": VALID_GUILD_ID,
            "channel_id": VALID_CHANNEL_ID,
            "message_id": VALID_MESSAGE_ID,
            "session_id": "  session-abc-123  ",
            "custom_id": "verify_captcha_button",
            "component_type": 2,
            "nonce": VALID_NONCE
        }
    });
    let decoded: BffDomainRequest =
        serde_json::from_value(wire.clone()).expect("deserialize ClickMessageComponent wire JSON");
    assert_eq!(
        decoded,
        BffDomainAction::ClickMessageComponent(sample_click_component_request())
    );
    assert_eq!(
        serde_json::to_value(&decoded).expect("serialize ClickMessageComponent"),
        wire
    );

    let alias_wire = json!({
        "MessageComponent": {
            "application_id": VALID_APP_ID,
            "guild_id": VALID_GUILD_ID,
            "channel_id": VALID_CHANNEL_ID,
            "message_id": VALID_MESSAGE_ID,
            "session_id": "  session-abc-123  ",
            "custom_id": "verify_captcha_button",
            "nonce": VALID_NONCE
        }
    });
    let decoded_alias: BffDomainRequest = serde_json::from_value(alias_wire)
        .expect("deserialize MessageComponent alias with default component_type=2");
    assert_eq!(
        decoded_alias,
        BffDomainAction::ClickMessageComponent(sample_click_component_request())
    );
}

#[test]
fn dispatch_update_guild_onboarding_builds_welcome_screen_route_and_body() {
    let action = BffDomainAction::UpdateGuildOnboarding {
        guild_id: VALID_GUILD_ID.to_string(),
        request: sample_onboarding_request(),
    };
    let (route, body) =
        dispatch_bff_domain_action(&action).expect("valid UpdateGuildOnboarding must succeed");

    assert_eq!(
        route,
        format!("/api/v9/guilds/{VALID_GUILD_ID}/welcome-screen")
    );
    assert_eq!(
        body,
        json!({
            "enabled": true,
            "description": "Добро пожаловать на наш сервер!",
            "welcome_channels": [
                {
                    "channel_id": VALID_CHANNEL_ID,
                    "description": "Правила и объявления сервера",
                    "emoji_id": VALID_EMOJI_ID,
                    "emoji_name": "wave"
                }
            ]
        })
    );
}

#[test]
fn dispatch_send_soundboard_sound_builds_channel_route_and_body() {
    let action = BffDomainAction::SendSoundboardSound {
        channel_id: VALID_CHANNEL_ID.to_string(),
        request: sample_soundboard_request(),
    };
    let (route, body) =
        dispatch_bff_domain_action(&action).expect("valid SendSoundboardSound must succeed");

    assert_eq!(
        route,
        format!("/api/v9/channels/{VALID_CHANNEL_ID}/send-soundboard-sound")
    );
    assert_eq!(
        body,
        json!({
            "sound_id": VALID_SOUND_ID,
            "source_guild_id": VALID_GUILD_ID,
        })
    );

    // С source_guild_id = None
    let local_action = BffDomainAction::SendSoundboardSound {
        channel_id: "1".to_string(),
        request: SendSoundboardSoundRequest {
            sound_id: "9".repeat(20),
            source_guild_id: None,
        },
    };
    let (route_local, body_local) =
        dispatch_bff_domain_action(&local_action).expect("local SendSoundboardSound");
    assert_eq!(route_local, "/api/v9/channels/1/send-soundboard-sound");
    assert_eq!(body_local["source_guild_id"], json!(null));
}

#[test]
fn dispatch_create_stage_instance_builds_stage_instances_route_and_body() {
    let action = BffDomainAction::CreateStageInstance(sample_stage_instance_request());
    let (route, body) =
        dispatch_bff_domain_action(&action).expect("valid CreateStageInstance must succeed");

    assert_eq!(route, "/api/v9/stage-instances");
    assert_eq!(
        body,
        json!({
            "channel_id": VALID_CHANNEL_ID,
            "topic": "Открытая архитектура Rust BFF",
            "privacy_level": STAGE_PRIVACY_GUILD_ONLY,
            "send_start_notification": true,
        })
    );
}

// ---------------------------------------------------------------------------
// 4. Проброс ошибок валидации во всех вариантах BffDomainAction
// ---------------------------------------------------------------------------

#[test]
fn dispatch_bff_domain_action_propagates_validation_errors_across_all_domains() {
    let twenty_one = "1".repeat(21);

    // 4.1. InvokeSlashCommand: невалидный application_id, пустой session_id, верхний регистр в command_name, > 25 options
    let mut bad_slash = sample_slash_command_request();
    bad_slash.application_id = "0".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::InvokeSlashCommand(bad_slash)).is_err(),
        "InvokeSlashCommand with application_id='0' must fail"
    );

    let mut bad_slash_name = sample_slash_command_request();
    bad_slash_name.command_name = "BadName".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::InvokeSlashCommand(bad_slash_name)).is_err(),
        "InvokeSlashCommand with uppercase command_name must fail"
    );

    let mut bad_slash_opts = sample_slash_command_request();
    bad_slash_opts.options = vec![json!({}); MAX_COMMAND_OPTIONS + 1];
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::InvokeSlashCommand(bad_slash_opts)).is_err(),
        "InvokeSlashCommand with 26 options must fail"
    );

    // 4.1b. ClickMessageComponent: невалидный message_id, пустой custom_id, неподдерживаемый component_type
    let mut bad_comp_msg = sample_click_component_request();
    bad_comp_msg.message_id = "0".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::ClickMessageComponent(bad_comp_msg)).is_err(),
        "ClickMessageComponent with message_id='0' must fail"
    );

    let mut bad_comp_custom = sample_click_component_request();
    bad_comp_custom.custom_id = "".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::ClickMessageComponent(bad_comp_custom))
            .is_err(),
        "ClickMessageComponent with empty custom_id must fail"
    );

    let mut bad_comp_type = sample_click_component_request();
    bad_comp_type.component_type = 1;
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::ClickMessageComponent(bad_comp_type)).is_err(),
        "ClickMessageComponent with component_type=1 must fail"
    );

    // 4.2. UpdateGuildOnboarding: невалидный guild_id ("0", 21 цифра) и невалидный request (пустое description, невалидный channel_id)
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::UpdateGuildOnboarding {
            guild_id: "0".to_string(),
            request: sample_onboarding_request(),
        })
        .is_err(),
        "UpdateGuildOnboarding with guild_id='0' must fail"
    );
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::UpdateGuildOnboarding {
            guild_id: twenty_one.clone(),
            request: sample_onboarding_request(),
        })
        .is_err(),
        "UpdateGuildOnboarding with 21-digit guild_id must fail"
    );

    let mut bad_onboarding = sample_onboarding_request();
    bad_onboarding.description = Some("   ".to_string());
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::UpdateGuildOnboarding {
            guild_id: VALID_GUILD_ID.to_string(),
            request: bad_onboarding,
        })
        .is_err(),
        "UpdateGuildOnboarding with blank description must fail"
    );

    // 4.3. SendSoundboardSound: невалидный channel_id, sound_id или source_guild_id
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::SendSoundboardSound {
            channel_id: "0".to_string(),
            request: sample_soundboard_request(),
        })
        .is_err(),
        "SendSoundboardSound with channel_id='0' must fail"
    );
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::SendSoundboardSound {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: SendSoundboardSoundRequest {
                sound_id: "0123".to_string(),
                source_guild_id: None,
            },
        })
        .is_err(),
        "SendSoundboardSound with leading-zero sound_id must fail"
    );
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::SendSoundboardSound {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: SendSoundboardSoundRequest {
                sound_id: VALID_SOUND_ID.to_string(),
                source_guild_id: Some(twenty_one),
            },
        })
        .is_err(),
        "SendSoundboardSound with 21-digit source_guild_id must fail"
    );

    // 4.4. CreateStageInstance: невалидный channel_id, пустой topic, privacy_level != 2
    let mut bad_stage_channel = sample_stage_instance_request();
    bad_stage_channel.channel_id = "".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateStageInstance(bad_stage_channel))
            .is_err(),
        "CreateStageInstance with empty channel_id must fail"
    );

    let mut bad_stage_topic = sample_stage_instance_request();
    bad_stage_topic.topic = "   ".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateStageInstance(bad_stage_topic)).is_err(),
        "CreateStageInstance with blank topic must fail"
    );

    let mut bad_stage_privacy = sample_stage_instance_request();
    bad_stage_privacy.privacy_level = 1;
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateStageInstance(bad_stage_privacy))
            .is_err(),
        "CreateStageInstance with privacy_level=1 must fail"
    );
}

// ---------------------------------------------------------------------------
// 5. resolve_bff_domain_action: HTTP-метод + маршрут + тело одним результатом
// ---------------------------------------------------------------------------

#[test]
fn resolve_bff_domain_action_returns_http_method_route_and_body() {
    let cases: [(BffDomainRequest, &'static str, String); 5] = [
        (
            BffDomainAction::InvokeSlashCommand(sample_slash_command_request()),
            "POST",
            "/api/v9/interactions".to_string(),
        ),
        (
            BffDomainAction::ClickMessageComponent(sample_click_component_request()),
            "POST",
            "/api/v9/interactions".to_string(),
        ),
        (
            BffDomainAction::UpdateGuildOnboarding {
                guild_id: VALID_GUILD_ID.to_string(),
                request: sample_onboarding_request(),
            },
            "PATCH",
            format!("/api/v9/guilds/{VALID_GUILD_ID}/welcome-screen"),
        ),
        (
            BffDomainAction::SendSoundboardSound {
                channel_id: VALID_CHANNEL_ID.to_string(),
                request: sample_soundboard_request(),
            },
            "POST",
            format!("/api/v9/channels/{VALID_CHANNEL_ID}/send-soundboard-sound"),
        ),
        (
            BffDomainAction::CreateStageInstance(sample_stage_instance_request()),
            "POST",
            "/api/v9/stage-instances".to_string(),
        ),
    ];

    for (request, expected_method, expected_route) in cases {
        let (dispatch_route, dispatch_body) =
            dispatch_bff_domain_action(&request).expect("dispatch valid BffDomainRequest");
        assert_eq!(
            dispatch_route, expected_route,
            "resolve and dispatch must not diverge"
        );

        let resolved: BffResolvedAction =
            resolve_bff_domain_action(request).expect("resolve valid BffDomainRequest");
        let method: &'static str = resolved.method;
        assert_eq!(method, expected_method, "route {}", resolved.route);
        assert_eq!(resolved.route, expected_route);
        assert_eq!(resolved.body, dispatch_body);
    }

    // Негативный срез: ошибка валидации не превращается в план запроса.
    let invalid = BffDomainAction::SendSoundboardSound {
        channel_id: "0".to_string(),
        request: sample_soundboard_request(),
    };
    assert!(
        resolve_bff_domain_action(invalid).is_err(),
        "resolve_bff_domain_action must propagate validation errors"
    );
}

// ---------------------------------------------------------------------------
// 6. Wire-формат BffDomainRequest закреплён буквальным JSON
// ---------------------------------------------------------------------------

#[test]
fn bff_domain_request_wire_format_matches_tagged_domain_action() {
    // `BffDomainRequest` — externally tagged enum: тег варианта снаружи, поля внутри.
    // Литерал ниже фиксирует контракт буквально: `#[serde(rename_all)]` или
    // `#[serde(tag = ...)]` сломают его, а не только симметричный round-trip.
    let wire = json!({
        "SendSoundboardSound": {
            "channel_id": VALID_CHANNEL_ID,
            "request": {
                "sound_id": VALID_SOUND_ID,
                "source_guild_id": VALID_GUILD_ID,
            }
        }
    });

    let decoded: BffDomainRequest =
        serde_json::from_value(wire.clone()).expect("literal wire JSON must deserialize");
    assert_eq!(
        decoded,
        BffDomainAction::SendSoundboardSound {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: SendSoundboardSoundRequest {
                sound_id: VALID_SOUND_ID.to_string(),
                source_guild_id: Some(VALID_GUILD_ID.to_string()),
            },
        }
    );
    assert_eq!(
        serde_json::to_value(&decoded).expect("serialize BffDomainRequest"),
        wire,
        "wire format must stay externally tagged by variant name"
    );

    // Плоский конверт `{domain, action}` НЕ является текущим контрактом `BffDomainRequest`.
    assert!(
        serde_json::from_value::<BffDomainRequest>(json!({
            "domain": "soundboard",
            "action": {"guild_id": "111", "sound_id": "222"},
        }))
        .is_err(),
        "flat {{domain, action}} envelope must not silently deserialize"
    );
}

// ---------------------------------------------------------------------------
// 7. Семь доменов: resolve_bff_domain_action покрывает каждый домен и его метод
// ---------------------------------------------------------------------------

#[test]
fn resolve_bff_domain_action_covers_all_seven_domains() {
    let cases: [(BffDomainRequest, &'static str, String); 7] = [
        (
            BffDomainAction::InvokeSlashCommand(sample_slash_command_request()),
            "POST",
            "/api/v9/interactions".to_string(),
        ),
        (
            BffDomainAction::UpdateGuildOnboarding {
                guild_id: VALID_GUILD_ID.to_string(),
                request: sample_onboarding_request(),
            },
            "PATCH",
            format!("/api/v9/guilds/{VALID_GUILD_ID}/welcome-screen"),
        ),
        (
            BffDomainAction::SendSoundboardSound {
                channel_id: VALID_CHANNEL_ID.to_string(),
                request: sample_soundboard_request(),
            },
            "POST",
            format!("/api/v9/channels/{VALID_CHANNEL_ID}/send-soundboard-sound"),
        ),
        (
            BffDomainAction::CreateStageInstance(sample_stage_instance_request()),
            "POST",
            "/api/v9/stage-instances".to_string(),
        ),
        (
            BffDomainAction::UpsertAutoModRule {
                guild_id: VALID_GUILD_ID.to_string(),
                rule_id: None,
                request: sample_automod_rule_request(),
            },
            "POST",
            format!("/api/v9/guilds/{VALID_GUILD_ID}/auto-moderation/rules"),
        ),
        (
            BffDomainAction::CreateGuildEmoji {
                guild_id: VALID_GUILD_ID.to_string(),
                byte_len: MAX_EMOJI_BYTES,
                request: sample_emoji_request(),
            },
            "POST",
            format!("/api/v9/guilds/{VALID_GUILD_ID}/emojis"),
        ),
        (
            BffDomainAction::CreateWebhook {
                channel_id: VALID_CHANNEL_ID.to_string(),
                request: sample_webhook_request(),
            },
            "POST",
            format!("/api/v9/channels/{VALID_CHANNEL_ID}/webhooks"),
        ),
    ];

    assert_eq!(
        cases.len(),
        SUPPORTED_BFF_DOMAINS.len(),
        "каждый домен обязан иметь хотя бы одно действие в таблице"
    );

    for (request, expected_method, expected_route) in cases {
        let resolved = resolve_bff_domain_action(request)
            .unwrap_or_else(|err| panic!("resolve домена {expected_route}: {err}"));
        assert_eq!(resolved.method, expected_method, "route {}", resolved.route);
        assert_eq!(resolved.route, expected_route);
    }

    // Новые домены зарегистрированы ровно под своими именами модулей.
    for domain in ["automod", "expressions", "webhooks"] {
        assert!(
            is_supported_bff_domain(domain),
            "domain {domain:?} must be supported"
        );
    }
    for unsupported in ["automod_rules", "emoji", "webhook"] {
        assert!(
            !is_supported_bff_domain(unsupported),
            "domain {unsupported:?} must not be supported"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Serde round-trip новых вариантов BffDomainAction
// ---------------------------------------------------------------------------

#[test]
fn bff_domain_action_serde_roundtrip_extended_variants() {
    let actions = vec![
        BffDomainAction::UpsertAutoModRule {
            guild_id: VALID_GUILD_ID.to_string(),
            rule_id: Some(VALID_RULE_ID.to_string()),
            request: sample_automod_rule_request(),
        },
        BffDomainAction::CreateGuildEmoji {
            guild_id: VALID_GUILD_ID.to_string(),
            byte_len: 1024,
            request: sample_emoji_request(),
        },
        BffDomainAction::CreateWebhook {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: sample_webhook_request(),
        },
        BffDomainAction::FollowNewsChannel {
            news_channel_id: VALID_NEWS_CHANNEL_ID.to_string(),
            target_channel_id: VALID_CHANNEL_ID.to_string(),
        },
    ];

    for action in actions {
        let serialized = serde_json::to_value(&action).expect("serialize BffDomainAction");
        let deserialized: BffDomainAction =
            serde_json::from_value(serialized.clone()).expect("deserialize BffDomainAction");
        assert_eq!(deserialized, action);
    }

    // Буквальный wire-формат нового варианта: внешний тег варианта, поля внутри.
    let wire = json!({
        "FollowNewsChannel": {
            "news_channel_id": VALID_NEWS_CHANNEL_ID,
            "target_channel_id": VALID_CHANNEL_ID,
        }
    });
    let decoded: BffDomainAction =
        serde_json::from_value(wire.clone()).expect("literal FollowNewsChannel wire JSON");
    assert_eq!(
        decoded,
        BffDomainAction::FollowNewsChannel {
            news_channel_id: VALID_NEWS_CHANNEL_ID.to_string(),
            target_channel_id: VALID_CHANNEL_ID.to_string(),
        }
    );
    assert_eq!(
        serde_json::to_value(&decoded).expect("serialize FollowNewsChannel"),
        wire
    );
}

// ---------------------------------------------------------------------------
// 9. automod: POST без rule_id, PATCH с rule_id, проброс ошибок
// ---------------------------------------------------------------------------

#[test]
fn dispatch_upsert_automod_rule_builds_post_and_patch_routes_and_bodies() {
    let expected_body = json!({
        "name": "Блокировка спама",
        "event_type": AUTOMOD_EVENT_MESSAGE_SEND,
        "trigger_type": AUTOMOD_TRIGGER_SPAM,
        "keyword_filter": [],
        "mention_total_limit": null,
        "action_type": AUTOMOD_ACTION_BLOCK_MESSAGE,
        "timeout_duration_secs": null,
        "enabled": true,
    });

    // Создание правила: rule_id = None -> POST на коллекцию.
    let create = BffDomainAction::UpsertAutoModRule {
        guild_id: VALID_GUILD_ID.to_string(),
        rule_id: None,
        request: sample_automod_rule_request(),
    };
    let (create_route, create_body) =
        dispatch_bff_domain_action(&create).expect("valid UpsertAutoModRule (create) must succeed");
    assert_eq!(
        create_route,
        format!("/api/v9/guilds/{VALID_GUILD_ID}/auto-moderation/rules")
    );
    assert_eq!(create_body, expected_body);

    let resolved_create =
        resolve_bff_domain_action(create).expect("resolve UpsertAutoModRule (create)");
    assert_eq!(resolved_create.method, "POST");
    assert_eq!(resolved_create.route, create_route);
    assert_eq!(resolved_create.body, expected_body);

    // Обновление правила: rule_id = Some(_) -> PATCH на конкретное правило.
    let update = BffDomainAction::UpsertAutoModRule {
        guild_id: VALID_GUILD_ID.to_string(),
        rule_id: Some(VALID_RULE_ID.to_string()),
        request: sample_automod_rule_request(),
    };
    let resolved_update =
        resolve_bff_domain_action(update).expect("resolve UpsertAutoModRule (update)");
    assert_eq!(resolved_update.method, "PATCH");
    assert_eq!(
        resolved_update.route,
        format!("/api/v9/guilds/{VALID_GUILD_ID}/auto-moderation/rules/{VALID_RULE_ID}")
    );
    assert_eq!(resolved_update.body, expected_body);

    // ERROR: невалидные Snowflake и запрос.
    for (label, action) in [
        (
            "guild_id='0'",
            BffDomainAction::UpsertAutoModRule {
                guild_id: "0".to_string(),
                rule_id: None,
                request: sample_automod_rule_request(),
            },
        ),
        (
            "rule_id='0123'",
            BffDomainAction::UpsertAutoModRule {
                guild_id: VALID_GUILD_ID.to_string(),
                rule_id: Some("0123".to_string()),
                request: sample_automod_rule_request(),
            },
        ),
    ] {
        assert!(
            dispatch_bff_domain_action(&action).is_err(),
            "UpsertAutoModRule with {label} must fail"
        );
    }

    let mut bad_name = sample_automod_rule_request();
    bad_name.name = "   ".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::UpsertAutoModRule {
            guild_id: VALID_GUILD_ID.to_string(),
            rule_id: None,
            request: bad_name,
        })
        .is_err(),
        "UpsertAutoModRule with blank name must fail"
    );

    let mut bad_keywords = sample_automod_rule_request();
    bad_keywords.trigger_type = AUTOMOD_TRIGGER_KEYWORD;
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::UpsertAutoModRule {
            guild_id: VALID_GUILD_ID.to_string(),
            rule_id: None,
            request: bad_keywords,
        })
        .is_err(),
        "UpsertAutoModRule KEYWORD without keyword_filter must fail"
    );

    let mut bad_timeout = sample_automod_rule_request();
    bad_timeout.action_type = AUTOMOD_ACTION_TIMEOUT;
    bad_timeout.timeout_duration_secs = Some(MAX_TIMEOUT_DURATION_SECS + 1);
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::UpsertAutoModRule {
            guild_id: VALID_GUILD_ID.to_string(),
            rule_id: None,
            request: bad_timeout,
        })
        .is_err(),
        "UpsertAutoModRule TIMEOUT above 28 days must fail"
    );
}

// ---------------------------------------------------------------------------
// 10. expressions: создание кастомного эмодзи, граничный byte_len
// ---------------------------------------------------------------------------

#[test]
fn dispatch_create_guild_emoji_builds_guild_emojis_route_and_body() {
    let expected_body = json!({
        "name": "party_wave",
        "image_data_uri": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg",
        "roles": [VALID_GUILD_ID],
    });

    let action = BffDomainAction::CreateGuildEmoji {
        guild_id: VALID_GUILD_ID.to_string(),
        byte_len: MAX_EMOJI_BYTES,
        request: sample_emoji_request(),
    };
    let (route, body) =
        dispatch_bff_domain_action(&action).expect("valid CreateGuildEmoji must succeed");
    assert_eq!(route, format!("/api/v9/guilds/{VALID_GUILD_ID}/emojis"));
    assert_eq!(body, expected_body);

    let resolved = resolve_bff_domain_action(action).expect("resolve CreateGuildEmoji");
    assert_eq!(resolved.method, "POST");
    assert_eq!(
        resolved.route,
        format!("/api/v9/guilds/{VALID_GUILD_ID}/emojis")
    );

    // Граница снизу: 1 байт допустим.
    let one_byte = BffDomainAction::CreateGuildEmoji {
        guild_id: "1".to_string(),
        byte_len: 1,
        request: CreateGuildEmojiRequest {
            roles: vec![],
            ..sample_emoji_request()
        },
    };
    let (route_b, body_b) =
        dispatch_bff_domain_action(&one_byte).expect("1-byte CreateGuildEmoji must succeed");
    assert_eq!(route_b, "/api/v9/guilds/1/emojis");
    assert_eq!(body_b["roles"], json!([]));

    // ERROR: пустой/слишком большой файл, невалидное имя, чужой Data URI, невалидная роль.
    for (label, byte_len) in [("byte_len=0", 0), ("byte_len=MAX+1", MAX_EMOJI_BYTES + 1)] {
        assert!(
            dispatch_bff_domain_action(&BffDomainAction::CreateGuildEmoji {
                guild_id: VALID_GUILD_ID.to_string(),
                byte_len,
                request: sample_emoji_request(),
            })
            .is_err(),
            "CreateGuildEmoji with {label} must fail"
        );
    }

    let mut bad_emoji_name = sample_emoji_request();
    bad_emoji_name.name = "ab-cd".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateGuildEmoji {
            guild_id: VALID_GUILD_ID.to_string(),
            byte_len: 1024,
            request: bad_emoji_name,
        })
        .is_err(),
        "CreateGuildEmoji with non-alphanumeric name must fail"
    );

    let mut bad_emoji_uri = sample_emoji_request();
    bad_emoji_uri.image_data_uri = "https://cdn.example/emoji.png".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateGuildEmoji {
            guild_id: VALID_GUILD_ID.to_string(),
            byte_len: 1024,
            request: bad_emoji_uri,
        })
        .is_err(),
        "CreateGuildEmoji with non-Data-URI image must fail"
    );

    let mut bad_emoji_role = sample_emoji_request();
    bad_emoji_role.roles = vec!["0".to_string()];
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateGuildEmoji {
            guild_id: VALID_GUILD_ID.to_string(),
            byte_len: 1024,
            request: bad_emoji_role,
        })
        .is_err(),
        "CreateGuildEmoji with role='0' must fail"
    );

    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateGuildEmoji {
            guild_id: "0".to_string(),
            byte_len: 1024,
            request: sample_emoji_request(),
        })
        .is_err(),
        "CreateGuildEmoji with guild_id='0' must fail"
    );
}

// ---------------------------------------------------------------------------
// 11. webhooks: создание вебхука и подписка на новостной канал
// ---------------------------------------------------------------------------

#[test]
fn dispatch_create_webhook_and_follow_news_channel_build_routes_and_bodies() {
    // Создание вебхука: имя тримится доменной валидацией.
    let create = BffDomainAction::CreateWebhook {
        channel_id: VALID_CHANNEL_ID.to_string(),
        request: sample_webhook_request(),
    };
    let (create_route, create_body) =
        dispatch_bff_domain_action(&create).expect("valid CreateWebhook must succeed");
    assert_eq!(
        create_route,
        format!("/api/v9/channels/{VALID_CHANNEL_ID}/webhooks")
    );
    assert_eq!(
        create_body,
        json!({"name": "Build Notifications", "avatar": null})
    );

    let resolved_create = resolve_bff_domain_action(create).expect("resolve CreateWebhook");
    assert_eq!(resolved_create.method, "POST");
    assert_eq!(resolved_create.route, create_route);
    assert_eq!(resolved_create.body, create_body);

    // Подписка на новостной канал.
    let follow = BffDomainAction::FollowNewsChannel {
        news_channel_id: VALID_NEWS_CHANNEL_ID.to_string(),
        target_channel_id: VALID_CHANNEL_ID.to_string(),
    };
    let (follow_route, follow_body) =
        dispatch_bff_domain_action(&follow).expect("valid FollowNewsChannel must succeed");
    assert_eq!(
        follow_route,
        format!("/api/v9/channels/{VALID_NEWS_CHANNEL_ID}/followers")
    );
    assert_eq!(follow_body, json!({"webhook_channel_id": VALID_CHANNEL_ID}));

    let resolved_follow = resolve_bff_domain_action(follow).expect("resolve FollowNewsChannel");
    assert_eq!(resolved_follow.method, "POST");
    assert_eq!(resolved_follow.route, follow_route);
    assert_eq!(resolved_follow.body, follow_body);

    // ERROR: запрещённые имена, невалидные Snowflake, совпадение каналов.
    let mut clyde = sample_webhook_request();
    clyde.name = "Clyde".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateWebhook {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: clyde,
        })
        .is_err(),
        "CreateWebhook named 'Clyde' must fail"
    );

    let mut discord_name = sample_webhook_request();
    discord_name.name = "Discord Build Bot".to_string();
    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateWebhook {
            channel_id: VALID_CHANNEL_ID.to_string(),
            request: discord_name,
        })
        .is_err(),
        "CreateWebhook containing 'discord' must fail"
    );

    assert!(
        dispatch_bff_domain_action(&BffDomainAction::CreateWebhook {
            channel_id: "0".to_string(),
            request: sample_webhook_request(),
        })
        .is_err(),
        "CreateWebhook with channel_id='0' must fail"
    );

    assert!(
        dispatch_bff_domain_action(&BffDomainAction::FollowNewsChannel {
            news_channel_id: VALID_CHANNEL_ID.to_string(),
            target_channel_id: VALID_CHANNEL_ID.to_string(),
        })
        .is_err(),
        "FollowNewsChannel with identical channels must fail"
    );
}
