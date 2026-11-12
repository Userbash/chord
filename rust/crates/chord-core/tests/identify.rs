//! Единый источник свойств IDENTIFY и валидация токена (RW-39 / NET-C1, P1-1, P1-4).
//!
//! **Провенанс**: `docs/re/rest-d02.md` §6.2 (22 поля `X-Super-Properties`, живой снимок
//! `95974.2323`), `tests/identify_ready.rs` (структура op2 из бандла веб-клиента),
//! `waves/RW-39/NET/CORE-AUDIT.md` P1-1 (расхождение launch_id/UA/версии) и P1-4
//! (пустой токен), `src/resume.rs` (`ResumeError::EmptyToken` — симметрия).

use chord_core::identify::{
    CAPABILITIES_ACTIVE, IdentifyError, IdentifyParams, IdentifyProperties, build_identify,
    build_identify_frame, qos_token_client_provided,
};
use chord_core::rest::super_properties::SuperProperties;

/// Супер-свойства с различимыми значениями во всех полях, которые видит IDENTIFY.
fn super_properties() -> SuperProperties {
    SuperProperties {
        os: "Linux".to_string(),
        browser: "Discord Client".to_string(),
        release_channel: "stable".to_string(),
        client_version: "0.1.0".to_string(),
        os_version: "6.17.0-lab".to_string(),
        os_arch: "x64".to_string(),
        app_arch: "x64".to_string(),
        system_locale: "ru".to_string(),
        has_client_mods: false,
        client_launch_id: "11111111-2222-4333-8444-555555555555".to_string(),
        browser_user_agent: "discord-lab/0.1.0 (Linux; x64)".to_string(),
        browser_version: "42.11.12".to_string(),
        window_manager: "unknown".to_string(),
        distro: "Linux".to_string(),
        runtime_environment: "unknown".to_string(),
        display_server: "wayland".to_string(),
        client_build_number: 641_004,
        native_build_number: 0,
        client_event_source: None,
        launch_signature: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".to_string(),
        client_heartbeat_session_id: "ffffffff-0000-4111-8222-333333333333".to_string(),
        client_app_state: "focused".to_string(),
    }
}

fn params(token: &str) -> IdentifyParams {
    IdentifyParams {
        token: token.to_string(),
        capabilities: CAPABILITIES_ACTIVE,
        properties: IdentifyProperties::from_super_properties(&super_properties()),
        presence: None,
        compress: false,
        client_state_guild_versions: Vec::new(),
        qos_token: qos_token_client_provided(false),
    }
}

#[test]
fn identify_properties_follow_super_properties() {
    let sp = super_properties();
    let properties = IdentifyProperties::from_super_properties(&sp);

    assert_eq!(properties.os, sp.os);
    assert_eq!(properties.browser, sp.browser);
    assert_eq!(properties.release_channel, sp.release_channel);
    assert_eq!(properties.client_version, sp.client_version);
    assert_eq!(properties.os_version, sp.os_version);
    assert_eq!(properties.os_arch, sp.os_arch);
    assert_eq!(properties.app_arch, sp.app_arch);
    assert_eq!(properties.system_locale, sp.system_locale);
    assert_eq!(properties.has_client_mods, sp.has_client_mods);
    assert_eq!(
        properties.client_launch_id, sp.client_launch_id,
        "launch_id обязан совпадать: один запуск — одна идентичность"
    );
    assert_eq!(properties.browser_user_agent, sp.browser_user_agent);
    assert_eq!(properties.browser_version, sp.browser_version);
    assert_eq!(
        properties.client_build_number,
        u64::from(sp.client_build_number)
    );
    assert_eq!(properties.client_app_state, sp.client_app_state);

    // Поля IDENTIFY, которых нет в супер-свойствах, — константы нашего клиента.
    assert!(!properties.is_fast_connect);
    assert_eq!(properties.gateway_connect_reasons, "COLD_START");
    assert_eq!(properties.installation_id, None);
}

#[test]
fn identify_properties_keep_launch_id_current() {
    let mut sp = super_properties();
    sp.client_launch_id = "99999999-8888-4777-8666-555555555555".to_string();
    sp.browser_user_agent = "discord-lab/9.9.9".to_string();
    sp.client_version = "9.9.9".to_string();

    let properties = IdentifyProperties::from_super_properties(&sp);
    assert_eq!(properties.client_launch_id, sp.client_launch_id);
    assert_eq!(properties.browser_user_agent, sp.browser_user_agent);
    assert_eq!(properties.client_version, sp.client_version);
    assert_eq!(
        properties.browser_version, sp.browser_version,
        "версия движка берётся из своего поля, а не подменяется версией клиента"
    );
}

#[test]
fn build_identify_rejects_empty_token() {
    assert_eq!(
        build_identify(&params("")).expect_err("пустой токен"),
        IdentifyError::EmptyToken
    );
    assert_eq!(
        build_identify_frame(&params("")).expect_err("пустой токен"),
        IdentifyError::EmptyToken
    );
}

#[test]
fn build_identify_rejects_whitespace_token() {
    assert_eq!(
        build_identify(&params("   ")).expect_err("пробельный токен"),
        IdentifyError::EmptyToken
    );
}

#[test]
fn build_identify_accepts_non_empty_token() {
    let bytes = build_identify(&params("user-token")).expect("валидный токен");
    assert_eq!(bytes.first(), Some(&0x83), "ведущий байт ETF");
}
