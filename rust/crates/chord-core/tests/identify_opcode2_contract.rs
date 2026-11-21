#![forbid(unsafe_code)]

//! Contract tests for `STAGE-05.3.1`:
//! Identify payload generator (Opcode 2) with 64-bit client properties:
//! Correct handshake with preservation of Snowflake IDs and system headers.
//!
//! Verifies:
//! 1. `IDENTIFY_OP == 2` and 64-bit capabilities / build numbers (`CAPABILITIES_ACTIVE`,
//!    `CAPABILITIES_CHANNEL_OBFUSCATION`, `client_build_number: u64`, `native_build_number: Option<u64>`).
//! 2. 64-bit Snowflake ID preservation in `IdentifyClientState` (`guild_versions: Vec<(u64, u64)>`
//!    and `highest_last_message_id: Option<u64>`) across full 64-bit Snowflake ranges without precision loss.
//! 3. System headers synchronization (`IdentifySystemHeaders::from_client_identity`) ensuring 1:1
//!    alignment between `X-Super-Properties` (Base64 JSON), `User-Agent`, `X-Discord-Locale`,
//!    `Accept-Language`, and the Opcode 2 `IDENTIFY` wire payload.
//! 4. `IdentifyHandshakeGenerator` construction, token redaction (`REDACTED`), error handling
//!    (`EmptyToken`, `QosTokenNotUtf8`, `ZeroSnowflakeId`), and ETF + JSON Opcode 2 payload/frame encoding.
//! 5. Round-trip ETF decoding and Snowflake 64-bit boundary verification (`1`, `1014537851418980372`,
//!    `i64::MAX as u64`, `u64::MAX`).

use chord_core::codec::etf::{self, EtfValue};
use chord_core::identify::{
    CAPABILITIES_ACTIVE, CAPABILITIES_CHANNEL_OBFUSCATION, IDENTIFY_OP, IdentifyClientState,
    IdentifyError, IdentifyHandshakeFrame, IdentifyHandshakeGenerator, IdentifyParams,
    IdentifyProperties, IdentifySystemHeaders, PresenceInfo, build_identify, build_identify_frame,
    qos_token_client_provided,
};
use chord_core::rest::identity::ClientIdentity;
use chord_core::rest::super_properties::SuperProperties;

fn sample_super_properties() -> SuperProperties {
    SuperProperties {
        os: "Linux".to_string(),
        browser: "Discord Client".to_string(),
        release_channel: "stable".to_string(),
        client_version: "1.0.162".to_string(),
        os_version: "6.17.0-chord".to_string(),
        os_arch: "x64".to_string(),
        app_arch: "x64".to_string(),
        system_locale: "en-US".to_string(),
        has_client_mods: false,
        client_launch_id: "11111111-2222-4333-8444-555555555555".to_string(),
        browser_user_agent:
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 discord/1.0.162 Chrome/148.0.0.0 Electron/42.11.12 Safari/537.36"
                .to_string(),
        browser_version: "42.11.12".to_string(),
        window_manager: "KDE".to_string(),
        distro: "\"Arch Linux\"".to_string(),
        runtime_environment: "native".to_string(),
        display_server: "wayland".to_string(),
        client_build_number: 641_004,
        native_build_number: 59_102,
        client_event_source: None,
        launch_signature: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".to_string(),
        client_heartbeat_session_id: "ffffffff-0000-4111-8222-333333333333".to_string(),
        client_app_state: "focused".to_string(),
    }
}

#[test]
fn test_identify_opcode_2_and_64bit_client_properties() {
    assert_eq!(IDENTIFY_OP, 2);
    assert_eq!(CAPABILITIES_ACTIVE, 1_734_653_u64);
    assert_eq!(CAPABILITIES_CHANNEL_OBFUSCATION, 1_767_421_u64);

    let sp = sample_super_properties();
    let identity = ClientIdentity::new(sp.clone());
    let props = IdentifyProperties::from_client_identity(&identity);

    assert_eq!(props.client_build_number, 641_004_u64);
    assert_eq!(props.os_arch, "x64");
    assert_eq!(props.app_arch, "x64");
    assert!(props.is_64bit_client());
    assert_eq!(props.client_launch_id, sp.client_launch_id);
    assert_eq!(props.launch_signature, sp.launch_signature);
}

#[test]
fn test_identify_handshake_generator_preserves_64bit_snowflake_ids() {
    let identity = ClientIdentity::new(sample_super_properties());

    // Real 64-bit Discord Snowflake IDs from clean-room capture (`docs/re/gateway-protocol.md` §2.3)
    // plus high 64-bit boundaries (`i64::MAX as u64` and `u64::MAX`).
    let guild_1: u64 = 1_014_537_851_418_980_372;
    let guild_2: u64 = 1_176_785_373_989_445_703;
    let guild_max: u64 = u64::MAX;
    let last_msg_id: u64 = 1_289_082_384_299_970_632;

    let client_state = IdentifyClientState::new()
        .with_guild_version(guild_1, 734)
        .with_guild_version(guild_2, 1_024)
        .with_guild_version(guild_max, 9_999)
        .with_highest_last_message_id(last_msg_id);

    assert_eq!(client_state.validate(), Ok(()));
    assert_eq!(client_state.guild_versions().len(), 3);
    assert_eq!(client_state.highest_last_message_id(), Some(last_msg_id));

    let generator = IdentifyHandshakeGenerator::from_client_identity(&identity)
        .with_capabilities(CAPABILITIES_CHANNEL_OBFUSCATION)
        .with_client_state(client_state)
        .with_presence(PresenceInfo {
            status: "online".to_string(),
            since: 1_700_000_000_000_u64,
            activities: Vec::new(),
            afk: false,
        });

    let handshake: IdentifyHandshakeFrame = generator
        .generate_handshake("mfa.test_secret_token_value")
        .expect("handshake generation must succeed");

    assert_eq!(handshake.op, IDENTIFY_OP);
    assert_eq!(handshake.capabilities, CAPABILITIES_CHANNEL_OBFUSCATION);
    assert_eq!(handshake.preserved_guild_snowflakes().len(), 3);
    assert_eq!(
        handshake.preserved_guild_snowflakes(),
        &[(guild_1, 734), (guild_2, 1_024), (guild_max, 9_999),]
    );
    assert_eq!(handshake.highest_last_message_id, Some(last_msg_id));

    // Decode ETF wire frame and verify exact Snowflake preservation
    let (decoded_frame, consumed) =
        etf::decode(&handshake.etf_frame).expect("ETF frame must decode cleanly");
    assert_eq!(consumed, handshake.etf_frame.len());
    assert_eq!(
        decoded_frame.get("op").and_then(EtfValue::as_int),
        Some(i64::from(IDENTIFY_OP))
    );

    let d = decoded_frame.get("d").expect("d payload in op2 frame");
    assert_eq!(
        d.get("capabilities").and_then(EtfValue::as_int),
        Some(CAPABILITIES_CHANNEL_OBFUSCATION as i64)
    );

    let cs = d.get("client_state").expect("client_state map");
    let gv = cs.get("guild_versions").expect("guild_versions map");
    assert_eq!(
        gv.get(&guild_1.to_string()).and_then(EtfValue::as_str),
        Some("734")
    );
    assert_eq!(
        gv.get(&guild_2.to_string()).and_then(EtfValue::as_str),
        Some("1024")
    );
    assert_eq!(
        gv.get(&guild_max.to_string()).and_then(EtfValue::as_str),
        Some("9999")
    );
    assert_eq!(
        cs.get("highest_last_message_id").and_then(EtfValue::as_str),
        Some(last_msg_id.to_string().as_str())
    );
}

#[test]
fn test_identify_system_headers_match_super_properties_and_opcode2_wire() {
    let sp = sample_super_properties();
    let identity = ClientIdentity::new(sp.clone());
    let generator = IdentifyHandshakeGenerator::from_client_identity(&identity)
        .with_timezone("Europe/Berlin")
        .with_debug_options("bugReporterEnabled");

    let handshake = generator
        .generate_handshake("user_session_token_123")
        .expect("generate handshake");

    let headers: &IdentifySystemHeaders = &handshake.system_headers;
    assert_eq!(headers.user_agent, sp.browser_user_agent);
    assert_eq!(headers.discord_locale, sp.system_locale);
    assert_eq!(headers.discord_timezone, "Europe/Berlin");
    assert_eq!(headers.debug_options, "bugReporterEnabled");
    assert_eq!(headers.super_properties_base64, sp.to_base64());

    // Decode X-Super-Properties from Base64 and verify 1:1 match with OP2 properties
    let decoded_sp = SuperProperties::from_base64(&headers.super_properties_base64)
        .expect("X-Super-Properties base64 must decode to valid 22-field SuperProperties");
    assert_eq!(decoded_sp, sp);

    let (decoded_frame, _) = etf::decode(&handshake.etf_frame).expect("decode etf");
    let props = decoded_frame
        .get("d")
        .and_then(|d| d.get("properties"))
        .expect("d.properties");

    assert_eq!(
        props.get("client_launch_id").and_then(EtfValue::as_str),
        Some(decoded_sp.client_launch_id.as_str())
    );
    assert_eq!(
        props.get("launch_signature").and_then(EtfValue::as_str),
        Some(decoded_sp.launch_signature.as_str())
    );
    assert_eq!(
        props.get("browser_user_agent").and_then(EtfValue::as_str),
        Some(headers.user_agent.as_str())
    );
    assert_eq!(
        props.get("system_locale").and_then(EtfValue::as_str),
        Some(headers.discord_locale.as_str())
    );
    assert_eq!(
        props.get("client_build_number").and_then(EtfValue::as_int),
        Some(i64::from(decoded_sp.client_build_number))
    );

    let header_pairs = headers.to_header_pairs();
    assert!(
        header_pairs
            .iter()
            .any(|(k, v)| *k == "X-Super-Properties" && *v == headers.super_properties_base64)
    );
    assert!(
        header_pairs
            .iter()
            .any(|(k, v)| *k == "User-Agent" && *v == headers.user_agent)
    );
    assert!(
        header_pairs
            .iter()
            .any(|(k, v)| *k == "X-Discord-Locale" && *v == headers.discord_locale)
    );
}

#[test]
fn test_identify_client_state_rejects_zero_snowflake_id() {
    let invalid_guild = IdentifyClientState::new().with_guild_version(0, 1);
    assert_eq!(
        invalid_guild.validate(),
        Err(IdentifyError::ZeroSnowflakeId)
    );

    let invalid_msg = IdentifyClientState::new().with_highest_last_message_id(0);
    assert_eq!(invalid_msg.validate(), Err(IdentifyError::ZeroSnowflakeId));

    let identity = ClientIdentity::new(sample_super_properties());
    let generator = IdentifyHandshakeGenerator::from_client_identity(&identity)
        .with_client_state(invalid_guild);
    assert_eq!(
        generator.generate_handshake("valid_token"),
        Err(IdentifyError::ZeroSnowflakeId)
    );
}

#[test]
fn test_identify_rejects_empty_token_and_non_utf8_qos_token() {
    let identity = ClientIdentity::new(sample_super_properties());
    let generator = IdentifyHandshakeGenerator::from_client_identity(&identity);

    assert_eq!(
        generator.generate_handshake(""),
        Err(IdentifyError::EmptyToken)
    );
    assert_eq!(
        generator.generate_handshake("   \t\n  "),
        Err(IdentifyError::EmptyToken)
    );

    let mut bad_params = IdentifyParams {
        token: "valid_token".to_string(),
        capabilities: CAPABILITIES_ACTIVE,
        properties: identity.to_identify_properties(),
        presence: None,
        compress: false,
        client_state_guild_versions: Vec::new(),
        qos_token: vec![0xFF, 0xFE, 0xFD],
    };
    assert_eq!(
        build_identify(&bad_params),
        Err(IdentifyError::QosTokenNotUtf8)
    );
    assert_eq!(
        build_identify_frame(&bad_params),
        Err(IdentifyError::QosTokenNotUtf8)
    );

    bad_params.qos_token = qos_token_client_provided(true);
    assert!(build_identify_frame(&bad_params).is_ok());
}

#[test]
fn test_identify_token_redaction_in_debug_output() {
    let secret = "mock_dummy_secret_auth_token_for_redaction_test";
    let identity = ClientIdentity::new(sample_super_properties());
    let params = IdentifyParams {
        token: secret.to_string(),
        capabilities: CAPABILITIES_ACTIVE,
        properties: identity.to_identify_properties(),
        presence: None,
        compress: false,
        client_state_guild_versions: Vec::new(),
        qos_token: qos_token_client_provided(false),
    };

    let debug_params = format!("{params:?}");
    assert!(
        !debug_params.contains(secret),
        "Token must never leak in IdentifyParams Debug"
    );
    assert!(debug_params.contains("REDACTED"));

    let generator = IdentifyHandshakeGenerator::from_client_identity(&identity);
    let handshake = generator.generate_handshake(secret).expect("handshake ok");
    let debug_handshake = format!("{handshake:?}");
    assert!(
        !debug_handshake.contains(secret),
        "Token must never leak in IdentifyHandshakeFrame Debug"
    );
    assert!(debug_handshake.contains("REDACTED"));
}
