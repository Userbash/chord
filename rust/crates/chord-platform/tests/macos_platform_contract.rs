use chord_platform::macos::{
    DeepLinkOpenUrlError, EntitlementsManifest, InMemoryTrafficLightsBackend,
    InMemoryVibrancyBackend, InfoPlistBundleSpec, MacosDeepLinkHandler, MacosPlatformError,
    MacosTrafficLightsController, MacosVibrancyController, NotarizationConfig,
    TrafficLightButton, TrafficLightInsets, VibrancyBlendingMode, VibrancyMaterial,
    VibrancyState,
};

#[test]
fn r6_04_vibrancy_material_window_applies_and_updates_state() {
    let backend = InMemoryVibrancyBackend::default();
    let mut controller = MacosVibrancyController::new(
        backend,
        VibrancyMaterial::HudWindow,
        VibrancyBlendingMode::BehindWindow,
    );

    assert_eq!(controller.material(), VibrancyMaterial::HudWindow);
    assert_eq!(controller.blending_mode(), VibrancyBlendingMode::BehindWindow);
    assert_eq!(controller.state(), VibrancyState::FollowsWindowActiveState);

    controller.apply().expect("initial vibrancy apply");
    assert_eq!(controller.backend().apply_calls(), 1);
    assert_eq!(
        controller.backend().last_material(),
        Some(VibrancyMaterial::HudWindow)
    );

    controller
        .set_material(VibrancyMaterial::Sidebar)
        .expect("update material");
    controller
        .set_state(VibrancyState::Active)
        .expect("update state");
    assert_eq!(controller.material(), VibrancyMaterial::Sidebar);
    assert_eq!(controller.state(), VibrancyState::Active);
    assert_eq!(controller.backend().apply_calls(), 3);
}

#[test]
fn r6_04_vibrancy_backend_error_propagates_without_silent_fallback() {
    let mut backend = InMemoryVibrancyBackend::default();
    backend.set_fail_next(true);
    let mut controller = MacosVibrancyController::new(
        backend,
        VibrancyMaterial::UnderWindowBackground,
        VibrancyBlendingMode::BehindWindow,
    );

    let err = controller.apply().unwrap_err();
    assert!(matches!(err, MacosPlatformError::BackendFailure(_)));
}

#[test]
fn r6_04_native_traffic_lights_controls_and_insets() {
    let backend = InMemoryTrafficLightsBackend::default();
    let mut controller = MacosTrafficLightsController::new(
        backend,
        TrafficLightInsets {
            x: 12.0,
            y: 14.0,
            spacing: 20.0,
        },
    );

    controller.sync_layout().expect("sync traffic lights");
    assert_eq!(controller.backend().layout_calls(), 1);
    assert!(controller.is_visible());

    // Entering fullscreen hides custom-positioned traffic lights
    controller.set_fullscreen(true).expect("enter fullscreen");
    assert!(!controller.is_visible());
    assert_eq!(controller.backend().visibility_calls(), 1);

    // Exiting fullscreen restores traffic lights
    controller.set_fullscreen(false).expect("exit fullscreen");
    assert!(controller.is_visible());
    assert_eq!(controller.backend().visibility_calls(), 2);

    // Button hit-testing / state query
    assert!(controller.is_button_enabled(TrafficLightButton::Close));
    assert!(controller.is_button_enabled(TrafficLightButton::Minimize));
    assert!(controller.is_button_enabled(TrafficLightButton::Zoom));
}

#[test]
fn r6_04_open_url_deep_links_accepts_custom_protocols_and_rejects_others() {
    let handler = MacosDeepLinkHandler::default();

    let discord_link = handler
        .handle_open_url("discord://-/channels/123456789/987654321")
        .expect("discord:// should be accepted");
    assert_eq!(discord_link.scheme, "discord");
    assert_eq!(discord_link.path, "/channels/123456789/987654321");

    let chord_link = handler
        .handle_open_url("chord://-/invite/abc123")
        .expect("chord:// should be accepted");
    assert_eq!(chord_link.scheme, "chord");
    assert_eq!(chord_link.path, "/invite/abc123");

    let invalid = handler.handle_open_url("https://example.com/evil");
    assert!(matches!(
        invalid,
        Err(DeepLinkOpenUrlError::UnsupportedScheme(_))
    ));

    let empty = handler.handle_open_url("   ");
    assert!(matches!(empty, Err(DeepLinkOpenUrlError::EmptyUrl)));
}

#[test]
fn r6_04_notarization_entitlements_and_bundle_validation() {
    let entitlements = EntitlementsManifest::default_macos_client();
    assert!(entitlements.has_entitlement("com.apple.security.network.client"));
    assert!(entitlements.has_entitlement("com.apple.security.device.audio-input"));
    assert!(entitlements.has_entitlement("com.apple.security.device.camera"));
    let plist_xml = entitlements.to_plist_xml();
    assert!(plist_xml.contains("<key>com.apple.security.network.client</key>"));
    assert!(plist_xml.contains("<true/>"));

    let bundle = InfoPlistBundleSpec {
        bundle_identifier: "com.chord.desktop".to_string(),
        bundle_name: "DiscordGPUI".to_string(),
        bundle_version: "1.0.0".to_string(),
        url_schemes: vec!["discord".to_string(), "chord".to_string()],
        minimum_system_version: "13.0".to_string(),
    };
    assert!(bundle.validate().is_ok());

    let notarization = NotarizationConfig {
        signing_identity: "Developer ID Application: Chord Labs (TEAM12345)".to_string(),
        apple_id: "release@chord.example".to_string(),
        team_id: "TEAM12345".to_string(),
        dmg_name: "DiscordGPUI-1.0.0-universal.dmg".to_string(),
        hardened_runtime: true,
        staple_ticket: true,
    };
    assert!(notarization.validate().is_ok());
    let cmd_args = notarization.codesign_args("DiscordGPUI.app");
    assert!(cmd_args.contains(&"--options".to_string()));
    assert!(cmd_args.contains(&"runtime".to_string()));
}
