//! macOS-specific platform integration (Task R6-04).
//!
//! Covers:
//! - Vibrancy material window (`NSVisualEffectView` materials, blending modes, active state).
//! - Native traffic lights controls (close/minimize/zoom insets, fullscreen visibility).
//! - `open-url` deep links (`discord://`, `chord://` custom protocol handling).
//! - Notarization, entitlements, and `Info.plist` bundle validation for `.dmg` distribution.

use std::collections::BTreeMap;
use std::fmt;

/// Errors produced by macOS platform operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacosPlatformError {
    BackendFailure(String),
    InvalidConfiguration(String),
}

impl fmt::Display for MacosPlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendFailure(msg) => write!(f, "macOS platform backend failure: {msg}"),
            Self::InvalidConfiguration(msg) => write!(f, "invalid macOS configuration: {msg}"),
        }
    }
}

impl std::error::Error for MacosPlatformError {}

/// `NSVisualEffectView.Material` presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VibrancyMaterial {
    HudWindow,
    Sidebar,
    UnderWindowBackground,
    Popover,
    Titlebar,
    Menu,
}

/// `NSVisualEffectView.BlendingMode` presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VibrancyBlendingMode {
    BehindWindow,
    WithinWindow,
}

/// `NSVisualEffectView.State` presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VibrancyState {
    FollowsWindowActiveState,
    Active,
    Inactive,
}

/// Backend abstraction for applying macOS window vibrancy.
pub trait VibrancyBackend {
    fn apply_vibrancy(
        &mut self,
        material: VibrancyMaterial,
        blending_mode: VibrancyBlendingMode,
        state: VibrancyState,
    ) -> Result<(), MacosPlatformError>;
}

/// Deterministic in-memory backend for testing macOS vibrancy.
#[derive(Debug, Default, Clone)]
pub struct InMemoryVibrancyBackend {
    apply_calls: usize,
    last_material: Option<VibrancyMaterial>,
    last_blending_mode: Option<VibrancyBlendingMode>,
    last_state: Option<VibrancyState>,
    fail_next: bool,
}

impl InMemoryVibrancyBackend {
    pub fn apply_calls(&self) -> usize {
        self.apply_calls
    }

    pub fn last_material(&self) -> Option<VibrancyMaterial> {
        self.last_material
    }

    pub fn last_blending_mode(&self) -> Option<VibrancyBlendingMode> {
        self.last_blending_mode
    }

    pub fn last_state(&self) -> Option<VibrancyState> {
        self.last_state
    }

    pub fn set_fail_next(&mut self, fail: bool) {
        self.fail_next = fail;
    }
}

impl VibrancyBackend for InMemoryVibrancyBackend {
    fn apply_vibrancy(
        &mut self,
        material: VibrancyMaterial,
        blending_mode: VibrancyBlendingMode,
        state: VibrancyState,
    ) -> Result<(), MacosPlatformError> {
        if self.fail_next {
            self.fail_next = false;
            return Err(MacosPlatformError::BackendFailure(
                "simulated NSVisualEffectView failure".to_string(),
            ));
        }
        self.apply_calls += 1;
        self.last_material = Some(material);
        self.last_blending_mode = Some(blending_mode);
        self.last_state = Some(state);
        Ok(())
    }
}

/// Coordinates macOS window vibrancy material, blending mode, and active state.
#[derive(Debug)]
pub struct MacosVibrancyController<B: VibrancyBackend> {
    backend: B,
    material: VibrancyMaterial,
    blending_mode: VibrancyBlendingMode,
    state: VibrancyState,
}

impl<B: VibrancyBackend> MacosVibrancyController<B> {
    pub fn new(
        backend: B,
        material: VibrancyMaterial,
        blending_mode: VibrancyBlendingMode,
    ) -> Self {
        Self {
            backend,
            material,
            blending_mode,
            state: VibrancyState::FollowsWindowActiveState,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn material(&self) -> VibrancyMaterial {
        self.material
    }

    pub fn blending_mode(&self) -> VibrancyBlendingMode {
        self.blending_mode
    }

    pub fn state(&self) -> VibrancyState {
        self.state
    }

    pub fn apply(&mut self) -> Result<(), MacosPlatformError> {
        self.backend
            .apply_vibrancy(self.material, self.blending_mode, self.state)
    }

    pub fn set_material(&mut self, material: VibrancyMaterial) -> Result<(), MacosPlatformError> {
        self.material = material;
        self.apply()
    }

    pub fn set_state(&mut self, state: VibrancyState) -> Result<(), MacosPlatformError> {
        self.state = state;
        self.apply()
    }
}

/// Native macOS traffic light window buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficLightButton {
    Close,
    Minimize,
    Zoom,
}

/// Positioning insets for custom titlebar traffic light controls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrafficLightInsets {
    pub x: f32,
    pub y: f32,
    pub spacing: f32,
}

/// Backend abstraction for native macOS traffic light controls.
pub trait TrafficLightsBackend {
    fn set_insets(&mut self, insets: TrafficLightInsets) -> Result<(), MacosPlatformError>;
    fn set_visible(&mut self, visible: bool) -> Result<(), MacosPlatformError>;
}

/// Deterministic in-memory backend for testing traffic light controls.
#[derive(Debug, Default, Clone)]
pub struct InMemoryTrafficLightsBackend {
    layout_calls: usize,
    visibility_calls: usize,
    last_insets: Option<TrafficLightInsets>,
    last_visible: Option<bool>,
}

impl InMemoryTrafficLightsBackend {
    pub fn layout_calls(&self) -> usize {
        self.layout_calls
    }

    pub fn visibility_calls(&self) -> usize {
        self.visibility_calls
    }

    pub fn last_insets(&self) -> Option<TrafficLightInsets> {
        self.last_insets
    }

    pub fn last_visible(&self) -> Option<bool> {
        self.last_visible
    }
}

impl TrafficLightsBackend for InMemoryTrafficLightsBackend {
    fn set_insets(&mut self, insets: TrafficLightInsets) -> Result<(), MacosPlatformError> {
        self.layout_calls += 1;
        self.last_insets = Some(insets);
        Ok(())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), MacosPlatformError> {
        self.visibility_calls += 1;
        self.last_visible = Some(visible);
        Ok(())
    }
}

/// Controls native traffic light visibility and titlebar insets.
#[derive(Debug)]
pub struct MacosTrafficLightsController<B: TrafficLightsBackend> {
    backend: B,
    insets: TrafficLightInsets,
    visible: bool,
    fullscreen: bool,
}

impl<B: TrafficLightsBackend> MacosTrafficLightsController<B> {
    pub fn new(backend: B, insets: TrafficLightInsets) -> Self {
        Self {
            backend,
            insets,
            visible: true,
            fullscreen: false,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn sync_layout(&mut self) -> Result<(), MacosPlatformError> {
        self.backend.set_insets(self.insets)
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), MacosPlatformError> {
        self.fullscreen = fullscreen;
        self.visible = !fullscreen;
        self.backend.set_visible(self.visible)
    }

    pub fn is_button_enabled(&self, button: TrafficLightButton) -> bool {
        match button {
            TrafficLightButton::Close => true,
            TrafficLightButton::Minimize | TrafficLightButton::Zoom => !self.fullscreen,
        }
    }
}

/// Errors from handling macOS `open-url` Apple Events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLinkOpenUrlError {
    EmptyUrl,
    MissingScheme,
    UnsupportedScheme(String),
}

/// Parsed deep link from macOS `application:openURLs:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMacosDeepLink {
    pub scheme: String,
    pub path: String,
}

/// Handles macOS `open-url` events for registered custom URL schemes.
#[derive(Debug, Clone)]
pub struct MacosDeepLinkHandler {
    allowed_schemes: Vec<String>,
}

impl Default for MacosDeepLinkHandler {
    fn default() -> Self {
        Self {
            allowed_schemes: vec!["discord".to_string(), "chord".to_string()],
        }
    }
}

impl MacosDeepLinkHandler {
    pub fn handle_open_url(&self, raw_url: &str) -> Result<ParsedMacosDeepLink, DeepLinkOpenUrlError> {
        let trimmed = raw_url.trim();
        if trimmed.is_empty() {
            return Err(DeepLinkOpenUrlError::EmptyUrl);
        }

        let Some((scheme_raw, rest)) = trimmed.split_once("://") else {
            return Err(DeepLinkOpenUrlError::MissingScheme);
        };

        let scheme = scheme_raw.to_ascii_lowercase();
        if !self.allowed_schemes.iter().any(|s| s == &scheme) {
            return Err(DeepLinkOpenUrlError::UnsupportedScheme(scheme));
        }

        let normalized_path = if let Some(after_dash) = rest.strip_prefix("-/") {
            format!("/{after_dash}")
        } else if rest.starts_with('/') {
            rest.to_string()
        } else {
            format!("/{rest}")
        };

        Ok(ParsedMacosDeepLink {
            scheme,
            path: normalized_path,
        })
    }
}

/// macOS code signing entitlements manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementsManifest {
    entries: BTreeMap<String, bool>,
}

impl EntitlementsManifest {
    pub fn default_macos_client() -> Self {
        let mut entries = BTreeMap::new();
        entries.insert("com.apple.security.app-sandbox".to_string(), false);
        entries.insert("com.apple.security.cs.allow-jit".to_string(), true);
        entries.insert("com.apple.security.device.audio-input".to_string(), true);
        entries.insert("com.apple.security.device.camera".to_string(), true);
        entries.insert("com.apple.security.network.client".to_string(), true);
        Self { entries }
    }

    pub fn has_entitlement(&self, key: &str) -> bool {
        self.entries.get(key).copied().unwrap_or(false)
    }

    pub fn to_plist_xml(&self) -> String {
        let mut out = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n",
        );
        for (key, value) in &self.entries {
            let bool_tag = if *value { "<true/>" } else { "<false/>" };
            out.push_str(&format!("  <key>{key}</key>\n  {bool_tag}\n"));
        }
        out.push_str("</dict>\n</plist>\n");
        out
    }
}

/// `Info.plist` bundle specification for `DiscordGPUI.app`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoPlistBundleSpec {
    pub bundle_identifier: String,
    pub bundle_name: String,
    pub bundle_version: String,
    pub url_schemes: Vec<String>,
    pub minimum_system_version: String,
}

impl InfoPlistBundleSpec {
    pub fn validate(&self) -> Result<(), MacosPlatformError> {
        if self.bundle_identifier.trim().is_empty() {
            return Err(MacosPlatformError::InvalidConfiguration(
                "bundle_identifier must not be empty".to_string(),
            ));
        }
        if self.bundle_name.trim().is_empty() {
            return Err(MacosPlatformError::InvalidConfiguration(
                "bundle_name must not be empty".to_string(),
            ));
        }
        if self.url_schemes.is_empty() {
            return Err(MacosPlatformError::InvalidConfiguration(
                "at least one CFBundleURLSchemes entry is required".to_string(),
            ));
        }
        Ok(())
    }
}

/// Notarization and `.dmg` signing configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotarizationConfig {
    pub signing_identity: String,
    pub apple_id: String,
    pub team_id: String,
    pub dmg_name: String,
    pub hardened_runtime: bool,
    pub staple_ticket: bool,
}

impl NotarizationConfig {
    pub fn validate(&self) -> Result<(), MacosPlatformError> {
        if self.signing_identity.trim().is_empty()
            || self.apple_id.trim().is_empty()
            || self.team_id.trim().is_empty()
        {
            return Err(MacosPlatformError::InvalidConfiguration(
                "signing_identity, apple_id, and team_id must be non-empty".to_string(),
            ));
        }
        if !self.dmg_name.ends_with(".dmg") {
            return Err(MacosPlatformError::InvalidConfiguration(
                "dmg_name must end with .dmg".to_string(),
            ));
        }
        if !self.hardened_runtime {
            return Err(MacosPlatformError::InvalidConfiguration(
                "hardened_runtime must be enabled for Apple notarization".to_string(),
            ));
        }
        Ok(())
    }

    pub fn codesign_args(&self, app_bundle_path: &str) -> Vec<String> {
        let mut args = vec![
            "--force".to_string(),
            "--deep".to_string(),
            "--sign".to_string(),
            self.signing_identity.clone(),
        ];
        if self.hardened_runtime {
            args.push("--options".to_string());
            args.push("runtime".to_string());
        }
        args.push(app_bundle_path.to_string());
        args
    }
}
