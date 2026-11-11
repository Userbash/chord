//! RW-40 / task-253 (`BE-P1-29`) + RW-41 / TASK-814 (`G1 Backend`):
//! контрактные тесты BFF-модуля slash-команд и взаимодействий с компонентами сообщений
//! (`slash_commands`: `APPLICATION_COMMAND` type 2 и `MESSAGE_COMPONENT` type 3).
//!
//! Проверяет:
//! 1. Константы и перечисление `InteractionType`:
//!    - `INTERACTION_TYPE_APPLICATION_COMMAND: u8 = 2` (`InteractionType::ApplicationCommand`)
//!    - `INTERACTION_TYPE_MESSAGE_COMPONENT: u8 = 3` (`InteractionType::MessageComponent`)
//!    - `INTERACTION_TYPE_AUTOCOMPLETE: u8 = 4` (`InteractionType::Autocomplete`)
//!    - `INTERACTION_TYPE_MODAL_SUBMIT: u8 = 5` (`InteractionType::ModalSubmit`)
//!    - `COMMAND_TYPE_CHAT_INPUT: u8 = 1`, `COMMAND_TYPE_USER: u8 = 2`, `COMMAND_TYPE_MESSAGE: u8 = 3`
//!    - `COMPONENT_TYPE_ACTION_ROW: u8 = 1`, `COMPONENT_TYPE_BUTTON: u8 = 2`,
//!      `COMPONENT_TYPE_STRING_SELECT: u8 = 3`, `COMPONENT_TYPE_TEXT_INPUT: u8 = 4`,
//!      `COMPONENT_TYPE_USER_SELECT: u8 = 5`, `COMPONENT_TYPE_ROLE_SELECT: u8 = 6`,
//!      `COMPONENT_TYPE_MENTIONABLE_SELECT: u8 = 7`, `COMPONENT_TYPE_CHANNEL_SELECT: u8 = 8`
//!    - `MAX_COMMAND_NAME_CHARS: usize = 32`, `MAX_COMMAND_OPTIONS: usize = 25`,
//!      `MAX_COMPONENT_CUSTOM_ID_CHARS: usize = 100`
//! 2. Структуру `InvokeSlashCommandRequest` и её serde round-trip (`Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`, `Eq`).
//! 3. `validate_slash_command_name(name: &str) -> Result<(), String>`.
//! 4. `validate_invoke_slash_command(req: &InvokeSlashCommandRequest) -> Result<(), String>`.
//! 5. `build_interactions_route_and_body(req: &InvokeSlashCommandRequest) -> Result<(String, serde_json::Value), String>`.
//! 6. TASK-814: структуру `ClickMessageComponentRequest`, `validate_component_custom_id`,
//!    `validate_message_component_type`, `validate_click_message_component` и
//!    `build_message_component_interactions_route_and_body` для `POST /api/v9/interactions`
//!    (`type = 3`, `component_type = 2`, `custom_id`, `message_id`, `channel_id`, `guild_id`, `session_id`, `nonce`).

use discord_gui_spike::slash_commands::{
    build_interactions_route_and_body, build_message_component_interactions_route_and_body,
    validate_click_message_component, validate_component_custom_id, validate_invoke_slash_command,
    validate_message_component_type, validate_slash_command_name, ClickMessageComponentRequest,
    InteractionType, InvokeSlashCommandRequest, COMMAND_TYPE_CHAT_INPUT, COMMAND_TYPE_MESSAGE,
    COMMAND_TYPE_USER, COMPONENT_TYPE_ACTION_ROW, COMPONENT_TYPE_BUTTON,
    COMPONENT_TYPE_CHANNEL_SELECT, COMPONENT_TYPE_MENTIONABLE_SELECT, COMPONENT_TYPE_ROLE_SELECT,
    COMPONENT_TYPE_STRING_SELECT, COMPONENT_TYPE_TEXT_INPUT, COMPONENT_TYPE_USER_SELECT,
    INTERACTION_TYPE_APPLICATION_COMMAND, INTERACTION_TYPE_AUTOCOMPLETE,
    INTERACTION_TYPE_MESSAGE_COMPONENT, INTERACTION_TYPE_MODAL_SUBMIT, MAX_COMMAND_NAME_CHARS,
    MAX_COMMAND_OPTIONS, MAX_COMPONENT_CUSTOM_ID_CHARS,
};
use serde_json::json;

fn sample_valid_request() -> InvokeSlashCommandRequest {
    InvokeSlashCommandRequest {
        application_id: "900000000000000001".to_string(),
        guild_id: Some("100000000000000001".to_string()),
        channel_id: "200000000000000002".to_string(),
        session_id: "  a1b2c3d4e5f60718293a4b5c6d7e8f90  ".to_string(),
        command_id: "300000000000000003".to_string(),
        command_version: "300000000000000004".to_string(),
        command_name: "ping_test-1".to_string(),
        options: vec![json!({
            "type": 3,
            "name": "query",
            "value": "status"
        })],
        nonce: "1234567890123456789".to_string(),
    }
}

fn sample_valid_component_request() -> ClickMessageComponentRequest {
    ClickMessageComponentRequest {
        application_id: "900000000000000001".to_string(),
        guild_id: Some("100000000000000001".to_string()),
        channel_id: "200000000000000002".to_string(),
        message_id: "400000000000000004".to_string(),
        session_id: "  a1b2c3d4e5f60718293a4b5c6d7e8f90  ".to_string(),
        custom_id: "verify_captcha_button".to_string(),
        component_type: COMPONENT_TYPE_BUTTON,
        nonce: "1234567890123456789".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip InvokeSlashCommandRequest
// ---------------------------------------------------------------------------

#[test]
fn slash_commands_constants_match_discord_contract() {
    assert_eq!(INTERACTION_TYPE_APPLICATION_COMMAND, 2_u8);
    assert_eq!(INTERACTION_TYPE_MESSAGE_COMPONENT, 3_u8);
    assert_eq!(INTERACTION_TYPE_AUTOCOMPLETE, 4_u8);
    assert_eq!(INTERACTION_TYPE_MODAL_SUBMIT, 5_u8);
    assert_eq!(COMMAND_TYPE_CHAT_INPUT, 1_u8);
    assert_eq!(COMMAND_TYPE_USER, 2_u8);
    assert_eq!(COMMAND_TYPE_MESSAGE, 3_u8);
    assert_eq!(COMPONENT_TYPE_ACTION_ROW, 1_u8);
    assert_eq!(COMPONENT_TYPE_BUTTON, 2_u8);
    assert_eq!(COMPONENT_TYPE_STRING_SELECT, 3_u8);
    assert_eq!(COMPONENT_TYPE_TEXT_INPUT, 4_u8);
    assert_eq!(COMPONENT_TYPE_USER_SELECT, 5_u8);
    assert_eq!(COMPONENT_TYPE_ROLE_SELECT, 6_u8);
    assert_eq!(COMPONENT_TYPE_MENTIONABLE_SELECT, 7_u8);
    assert_eq!(COMPONENT_TYPE_CHANNEL_SELECT, 8_u8);
    assert_eq!(MAX_COMMAND_NAME_CHARS, 32_usize);
    assert_eq!(MAX_COMMAND_OPTIONS, 25_usize);
    assert_eq!(MAX_COMPONENT_CUSTOM_ID_CHARS, 100_usize);

    assert_eq!(InteractionType::ApplicationCommand.as_u8(), 2_u8);
    assert_eq!(InteractionType::MessageComponent.as_u8(), 3_u8);
    assert_eq!(InteractionType::Autocomplete.as_u8(), 4_u8);
    assert_eq!(InteractionType::ModalSubmit.as_u8(), 5_u8);
    assert_eq!(
        InteractionType::from_u8(2),
        Some(InteractionType::ApplicationCommand)
    );
    assert_eq!(
        InteractionType::from_u8(3),
        Some(InteractionType::MessageComponent)
    );
    assert_eq!(
        InteractionType::from_u8(4),
        Some(InteractionType::Autocomplete)
    );
    assert_eq!(
        InteractionType::from_u8(5),
        Some(InteractionType::ModalSubmit)
    );
    assert_eq!(InteractionType::from_u8(0), None);
    assert_eq!(InteractionType::from_u8(1), None);
    assert_eq!(InteractionType::from_u8(6), None);
}

#[test]
fn interaction_type_serde_supports_numeric_and_named_representations() {
    let serialized =
        serde_json::to_value(InteractionType::MessageComponent).expect("serialize InteractionType");
    assert_eq!(serialized, json!(3));

    let from_num: InteractionType =
        serde_json::from_value(json!(3)).expect("deserialize InteractionType from 3");
    assert_eq!(from_num, InteractionType::MessageComponent);

    let from_name: InteractionType = serde_json::from_value(json!("MessageComponent"))
        .expect("deserialize InteractionType from variant name");
    assert_eq!(from_name, InteractionType::MessageComponent);

    let from_screaming: InteractionType = serde_json::from_value(json!("MESSAGE_COMPONENT"))
        .expect("deserialize InteractionType from SCREAMING_SNAKE_CASE");
    assert_eq!(from_screaming, InteractionType::MessageComponent);

    assert!(serde_json::from_value::<InteractionType>(json!(1)).is_err());
    assert!(serde_json::from_value::<InteractionType>(json!(99)).is_err());
    assert!(serde_json::from_value::<InteractionType>(json!("UNKNOWN")).is_err());
}

#[test]
fn invoke_slash_command_request_serde_roundtrip() {
    let req = sample_valid_request();
    let encoded = serde_json::to_value(&req).expect("serialize InvokeSlashCommandRequest");
    assert_eq!(
        encoded,
        json!({
            "application_id": "900000000000000001",
            "guild_id": "100000000000000001",
            "channel_id": "200000000000000002",
            "session_id": "  a1b2c3d4e5f60718293a4b5c6d7e8f90  ",
            "command_id": "300000000000000003",
            "command_version": "300000000000000004",
            "command_name": "ping_test-1",
            "options": [
                {
                    "type": 3,
                    "name": "query",
                    "value": "status"
                }
            ],
            "nonce": "1234567890123456789"
        })
    );
    let decoded: InvokeSlashCommandRequest =
        serde_json::from_value(encoded).expect("deserialize InvokeSlashCommandRequest");
    assert_eq!(decoded, req);

    // DM-контекст (guild_id = None)
    let mut dm_req = req.clone();
    dm_req.guild_id = None;
    let dm_json = serde_json::to_value(&dm_req).expect("serialize DM InvokeSlashCommandRequest");
    assert_eq!(dm_json["guild_id"], json!(null));
    let dm_back: InvokeSlashCommandRequest =
        serde_json::from_value(dm_json).expect("deserialize DM InvokeSlashCommandRequest");
    assert_eq!(dm_back, dm_req);
}

// ---------------------------------------------------------------------------
// 2. validate_slash_command_name: 1..=32 символов, [a-z0-9_-]
// ---------------------------------------------------------------------------

#[test]
fn validate_slash_command_name_accepts_valid_names_and_boundaries() {
    for valid in [
        "a",
        "z",
        "0",
        "9",
        "-",
        "_",
        "ping",
        "mod-ban_user2",
        "a_b-c_0123456789",
    ] {
        assert!(
            validate_slash_command_name(valid).is_ok(),
            "command name={valid:?} should be valid"
        );
    }

    let max_32 = "a".repeat(MAX_COMMAND_NAME_CHARS);
    assert_eq!(max_32.len(), 32);
    assert!(validate_slash_command_name(&max_32).is_ok());
}

#[test]
fn validate_slash_command_name_rejects_empty_too_long_uppercase_or_invalid_chars() {
    assert!(validate_slash_command_name("").is_err());

    let too_long_33 = "a".repeat(MAX_COMMAND_NAME_CHARS + 1);
    assert!(validate_slash_command_name(&too_long_33).is_err());

    for invalid in [
        "Ping",
        "PING",
        "pingTest",
        "ping test",
        " ping",
        "ping ",
        "/ping",
        "ping!",
        "ping.cmd",
        "команда",
    ] {
        assert!(
            validate_slash_command_name(invalid).is_err(),
            "command name={invalid:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. validate_invoke_slash_command: снежинки, session_id, command_name, options <= 25
// ---------------------------------------------------------------------------

#[test]
fn validate_invoke_slash_command_accepts_guild_and_dm_requests_and_option_bounds() {
    let mut req = sample_valid_request();
    assert!(validate_invoke_slash_command(&req).is_ok());

    // DM: guild_id = None
    req.guild_id = None;
    assert!(validate_invoke_slash_command(&req).is_ok());

    // Границы Snowflake: 1 цифра и 20 цифр
    req.application_id = "1".to_string();
    req.guild_id = Some("9".repeat(20));
    req.channel_id = "1".to_string();
    req.command_id = "9".repeat(20);
    req.command_version = "1".to_string();
    req.nonce = "9".repeat(20);
    assert!(validate_invoke_slash_command(&req).is_ok());

    // Границы options: 0 и 25 элементов
    req.options.clear();
    assert!(validate_invoke_slash_command(&req).is_ok());

    req.options = (0..MAX_COMMAND_OPTIONS)
        .map(|i| json!({ "name": format!("opt_{i}"), "type": 3, "value": "v" }))
        .collect();
    assert_eq!(req.options.len(), 25);
    assert!(validate_invoke_slash_command(&req).is_ok());
}

#[test]
fn validate_invoke_slash_command_rejects_invalid_snowflakes_session_name_or_options() {
    let twenty_one_digits = "1".repeat(21);
    for bad_id in [
        "",
        "0",
        "00",
        "012345",
        "-1",
        "+1",
        "abc",
        "123a456",
        " 100000000000000001",
        "100000000000000001 ",
        &twenty_one_digits,
    ] {
        let mut bad_app = sample_valid_request();
        bad_app.application_id = bad_id.to_string();
        assert!(
            validate_invoke_slash_command(&bad_app).is_err(),
            "application_id={bad_id:?} must be rejected"
        );

        let mut bad_guild = sample_valid_request();
        bad_guild.guild_id = Some(bad_id.to_string());
        assert!(
            validate_invoke_slash_command(&bad_guild).is_err(),
            "guild_id=Some({bad_id:?}) must be rejected"
        );

        let mut bad_channel = sample_valid_request();
        bad_channel.channel_id = bad_id.to_string();
        assert!(
            validate_invoke_slash_command(&bad_channel).is_err(),
            "channel_id={bad_id:?} must be rejected"
        );

        let mut bad_cmd_id = sample_valid_request();
        bad_cmd_id.command_id = bad_id.to_string();
        assert!(
            validate_invoke_slash_command(&bad_cmd_id).is_err(),
            "command_id={bad_id:?} must be rejected"
        );

        let mut bad_cmd_ver = sample_valid_request();
        bad_cmd_ver.command_version = bad_id.to_string();
        assert!(
            validate_invoke_slash_command(&bad_cmd_ver).is_err(),
            "command_version={bad_id:?} must be rejected"
        );

        let mut bad_nonce = sample_valid_request();
        bad_nonce.nonce = bad_id.to_string();
        assert!(
            validate_invoke_slash_command(&bad_nonce).is_err(),
            "nonce={bad_id:?} must be rejected"
        );
    }

    // Пустой / пробельный session_id
    for empty_session in ["", " ", "   ", "\t\n\r"] {
        let mut bad_session = sample_valid_request();
        bad_session.session_id = empty_session.to_string();
        assert!(
            validate_invoke_slash_command(&bad_session).is_err(),
            "session_id={empty_session:?} must be rejected"
        );
    }

    // Некорректное имя команды
    let mut bad_name = sample_valid_request();
    bad_name.command_name = "InvalidName".to_string();
    assert!(validate_invoke_slash_command(&bad_name).is_err());

    // Превышение MAX_COMMAND_OPTIONS (26 > 25)
    let mut too_many_opts = sample_valid_request();
    too_many_opts.options = (0..=MAX_COMMAND_OPTIONS)
        .map(|i| json!({ "name": format!("opt_{i}"), "type": 3, "value": "v" }))
        .collect();
    assert_eq!(too_many_opts.options.len(), 26);
    assert!(validate_invoke_slash_command(&too_many_opts).is_err());
}

// ---------------------------------------------------------------------------
// 4. build_interactions_route_and_body: канонический маршрут и JSON-тело
// ---------------------------------------------------------------------------

#[test]
fn build_interactions_route_and_body_returns_canonical_route_and_payload() {
    let req = sample_valid_request();
    let (route, body) =
        build_interactions_route_and_body(&req).expect("valid interactions route and body");
    assert_eq!(route, "/api/v9/interactions");
    assert_eq!(
        body,
        json!({
            "type": 2,
            "application_id": "900000000000000001",
            "guild_id": "100000000000000001",
            "channel_id": "200000000000000002",
            "session_id": "a1b2c3d4e5f60718293a4b5c6d7e8f90",
            "nonce": "1234567890123456789",
            "data": {
                "version": "300000000000000004",
                "id": "300000000000000003",
                "name": "ping_test-1",
                "type": 1,
                "options": [
                    {
                        "type": 3,
                        "name": "query",
                        "value": "status"
                    }
                ]
            }
        })
    );

    // Проверка с guild_id = None и пустыми options
    let mut dm_req = sample_valid_request();
    dm_req.guild_id = None;
    dm_req.options = vec![];
    let (dm_route, dm_body) =
        build_interactions_route_and_body(&dm_req).expect("valid DM interactions route and body");
    assert_eq!(dm_route, "/api/v9/interactions");
    assert_eq!(dm_body["guild_id"], json!(null));
    assert_eq!(dm_body["data"]["options"], json!([]));
}

#[test]
fn build_interactions_route_and_body_propagates_validation_errors() {
    let mut invalid = sample_valid_request();
    invalid.command_name = "BAD_NAME".to_string();
    assert!(build_interactions_route_and_body(&invalid).is_err());

    let mut invalid_session = sample_valid_request();
    invalid_session.session_id = "   ".to_string();
    assert!(build_interactions_route_and_body(&invalid_session).is_err());
}

// ---------------------------------------------------------------------------
// 5. TASK-814: ClickMessageComponentRequest (MESSAGE_COMPONENT type = 3)
// ---------------------------------------------------------------------------

#[test]
fn click_message_component_request_serde_roundtrip_and_default_button_type() {
    let req = sample_valid_component_request();
    let encoded = serde_json::to_value(&req).expect("serialize ClickMessageComponentRequest");
    assert_eq!(
        encoded,
        json!({
            "application_id": "900000000000000001",
            "guild_id": "100000000000000001",
            "channel_id": "200000000000000002",
            "message_id": "400000000000000004",
            "session_id": "  a1b2c3d4e5f60718293a4b5c6d7e8f90  ",
            "custom_id": "verify_captcha_button",
            "component_type": 2,
            "nonce": "1234567890123456789"
        })
    );
    let decoded: ClickMessageComponentRequest =
        serde_json::from_value(encoded).expect("deserialize ClickMessageComponentRequest");
    assert_eq!(decoded, req);

    // Если component_type опущен в JSON, по умолчанию подставляется COMPONENT_TYPE_BUTTON (2)
    let without_component_type = json!({
        "application_id": "900000000000000001",
        "guild_id": null,
        "channel_id": "200000000000000002",
        "message_id": "400000000000000004",
        "session_id": "session-xyz",
        "custom_id": "wick:verify:1",
        "nonce": "1234567890123456789"
    });
    let defaulted: ClickMessageComponentRequest = serde_json::from_value(without_component_type)
        .expect("deserialize ClickMessageComponentRequest with default component_type");
    assert_eq!(defaulted.component_type, COMPONENT_TYPE_BUTTON);
    assert_eq!(defaulted.guild_id, None);
}

#[test]
fn validate_component_custom_id_and_type_enforce_discord_bounds() {
    for valid_custom_id in [
        "v",
        "verify_captcha_button",
        "wick:verify:987654321",
        "carl-bot/role/accept_rules",
        "button:✅:confirm",
    ] {
        assert!(
            validate_component_custom_id(valid_custom_id).is_ok(),
            "custom_id={valid_custom_id:?} should be valid"
        );
    }

    let max_100 = "x".repeat(MAX_COMPONENT_CUSTOM_ID_CHARS);
    assert_eq!(max_100.chars().count(), 100);
    assert!(validate_component_custom_id(&max_100).is_ok());

    // Негативные сценарии custom_id: пустой, пробельный, > 100 символов, управляющие символы
    for bad_custom_id in ["", " ", "   ", "\t\n", "bad\0id", "bad\nid"] {
        assert!(
            validate_component_custom_id(bad_custom_id).is_err(),
            "custom_id={bad_custom_id:?} must be rejected"
        );
    }
    let too_long_101 = "x".repeat(MAX_COMPONENT_CUSTOM_ID_CHARS + 1);
    assert!(validate_component_custom_id(&too_long_101).is_err());

    // Валидация типа компонента кнопки (component_type == 2)
    assert!(validate_message_component_type(COMPONENT_TYPE_BUTTON).is_ok());
    for bad_type in [0_u8, COMPONENT_TYPE_ACTION_ROW, 3, 4, 5, 8, 255] {
        assert!(
            validate_message_component_type(bad_type).is_err(),
            "component_type={bad_type} must be rejected for button click"
        );
    }
}

#[test]
fn validate_click_message_component_checks_snowflakes_session_custom_id_and_type() {
    let mut req = sample_valid_component_request();
    assert!(validate_click_message_component(&req).is_ok());

    // DM: guild_id = None
    req.guild_id = None;
    assert!(validate_click_message_component(&req).is_ok());

    // Граничные Snowflake ("1" и 20 девяток) и custom_id длины 1 и 100
    req.application_id = "1".to_string();
    req.guild_id = Some("9".repeat(20));
    req.channel_id = "1".to_string();
    req.message_id = "9".repeat(20);
    req.nonce = "1".to_string();
    req.custom_id = "a".repeat(MAX_COMPONENT_CUSTOM_ID_CHARS);
    assert!(validate_click_message_component(&req).is_ok());

    let twenty_one_digits = "1".repeat(21);
    for bad_id in [
        "",
        "0",
        "00",
        "012345",
        "-1",
        "+1",
        "abc",
        "123a456",
        " 100000000000000001",
        "100000000000000001 ",
        &twenty_one_digits,
    ] {
        let mut bad_app = sample_valid_component_request();
        bad_app.application_id = bad_id.to_string();
        assert!(
            validate_click_message_component(&bad_app).is_err(),
            "application_id={bad_id:?} must be rejected"
        );

        let mut bad_guild = sample_valid_component_request();
        bad_guild.guild_id = Some(bad_id.to_string());
        assert!(
            validate_click_message_component(&bad_guild).is_err(),
            "guild_id=Some({bad_id:?}) must be rejected"
        );

        let mut bad_channel = sample_valid_component_request();
        bad_channel.channel_id = bad_id.to_string();
        assert!(
            validate_click_message_component(&bad_channel).is_err(),
            "channel_id={bad_id:?} must be rejected"
        );

        let mut bad_message = sample_valid_component_request();
        bad_message.message_id = bad_id.to_string();
        assert!(
            validate_click_message_component(&bad_message).is_err(),
            "message_id={bad_id:?} must be rejected"
        );

        let mut bad_nonce = sample_valid_component_request();
        bad_nonce.nonce = bad_id.to_string();
        assert!(
            validate_click_message_component(&bad_nonce).is_err(),
            "nonce={bad_id:?} must be rejected"
        );
    }

    for empty_session in ["", " ", "   ", "\t\n\r"] {
        let mut bad_session = sample_valid_component_request();
        bad_session.session_id = empty_session.to_string();
        assert!(
            validate_click_message_component(&bad_session).is_err(),
            "session_id={empty_session:?} must be rejected"
        );
    }

    let mut bad_custom = sample_valid_component_request();
    bad_custom.custom_id = "   ".to_string();
    assert!(validate_click_message_component(&bad_custom).is_err());

    let mut bad_type = sample_valid_component_request();
    bad_type.component_type = COMPONENT_TYPE_ACTION_ROW;
    assert!(validate_click_message_component(&bad_type).is_err());
}

#[test]
fn build_message_component_interactions_route_and_body_returns_type_3_payload() {
    let req = sample_valid_component_request();
    let (route, body) = build_message_component_interactions_route_and_body(&req)
        .expect("valid message component interaction route and body");
    assert_eq!(route, "/api/v9/interactions");
    assert_eq!(
        body,
        json!({
            "type": 3,
            "application_id": "900000000000000001",
            "guild_id": "100000000000000001",
            "channel_id": "200000000000000002",
            "message_id": "400000000000000004",
            "session_id": "a1b2c3d4e5f60718293a4b5c6d7e8f90",
            "nonce": "1234567890123456789",
            "data": {
                "component_type": 2,
                "custom_id": "verify_captcha_button"
            }
        })
    );

    // DM-контекст (guild_id = None)
    let mut dm_req = sample_valid_component_request();
    dm_req.guild_id = None;
    let (dm_route, dm_body) = build_message_component_interactions_route_and_body(&dm_req)
        .expect("valid DM message component interaction route and body");
    assert_eq!(dm_route, "/api/v9/interactions");
    assert_eq!(dm_body["type"], json!(INTERACTION_TYPE_MESSAGE_COMPONENT));
    assert_eq!(dm_body["guild_id"], json!(null));
    assert_eq!(dm_body["message_id"], json!("400000000000000004"));
    assert_eq!(dm_body["data"]["component_type"], json!(2));
    assert_eq!(dm_body["data"]["custom_id"], json!("verify_captcha_button"));

    // Ошибки валидации пробрасываются наружу
    let mut invalid = sample_valid_component_request();
    invalid.custom_id = "".to_string();
    assert!(build_message_component_interactions_route_and_body(&invalid).is_err());

    let mut invalid_msg = sample_valid_component_request();
    invalid_msg.message_id = "0".to_string();
    assert!(build_message_component_interactions_route_and_body(&invalid_msg).is_err());
}
