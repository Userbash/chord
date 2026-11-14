#![allow(dead_code)]

use crate::state::{AppState, CallStatus, ScreenSource};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use crate::video::{ScreenCastManager, ScreenCastState};
use chord_platform::screencast::{
    CaptureResolutionPreset, CaptureTargetKind, NegotiatedStreamParams, PortalRequestError,
    PortalSourceTypes, PortalStreamInfo, RTP_VIDEO_CLOCK_HZ, ScreenCastCapturePipeline,
    ScreenCastNegotiationRequest, ScreenCastOptions, ScreenCastPortal, negotiate_stream_params,
};
use gpui::*;
use gpui_kit::component::StyledExt;
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};

/// Вкладки нативного селектора демонстрации экрана («Экраны» и «Окна приложений»)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreenShareTab {
    #[default]
    Monitors,
    Windows,
}

impl ScreenShareTab {
    pub const ALL: [Self; 2] = [Self::Monitors, Self::Windows];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Monitors => "Экраны",
            Self::Windows => "Окна приложений",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Monitors => "🖥️",
            Self::Windows => "🪟",
        }
    }

    pub const fn empty_message(self) -> &'static str {
        match self {
            Self::Monitors => "Нет доступных экранов для трансляции",
            Self::Windows => "Нет открытых окон приложений для трансляции",
        }
    }

    pub const fn portal_source_types(self) -> PortalSourceTypes {
        match self {
            Self::Monitors => PortalSourceTypes::MONITOR,
            Self::Windows => PortalSourceTypes::WINDOW,
        }
    }

    pub const fn capture_target_kind(self) -> CaptureTargetKind {
        match self {
            Self::Monitors => CaptureTargetKind::Monitor,
            Self::Windows => CaptureTargetKind::Window,
        }
    }

    pub const fn matches_source(self, source: &ScreenSource) -> bool {
        match self {
            Self::Monitors => source.is_screen,
            Self::Windows => !source.is_screen,
        }
    }
}

/// Пресеты разрешения демонстрации экрана (720p / 1080p)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreenShareResolution {
    P720,
    #[default]
    P1080,
}

impl ScreenShareResolution {
    pub const ALL: [Self; 2] = [Self::P720, Self::P1080];

    pub const fn label(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
        }
    }

    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::P720 => "1280×720 HD",
            Self::P1080 => "1920×1080 FHD",
        }
    }

    pub const fn dimensions(self) -> (u32, u32) {
        self.capture_preset().dimensions()
    }

    pub const fn capture_preset(self) -> CaptureResolutionPreset {
        match self {
            Self::P720 => CaptureResolutionPreset::Hd720p,
            Self::P1080 => CaptureResolutionPreset::Fhd1080p,
        }
    }

    pub const fn from_height(height: u32) -> Option<Self> {
        match height {
            720 => Some(Self::P720),
            1080 => Some(Self::P1080),
            _ => None,
        }
    }
}

/// Пресеты частоты кадров демонстрации экрана (30 FPS / 60 FPS)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreenShareFps {
    Fps30,
    #[default]
    Fps60,
}

impl ScreenShareFps {
    pub const ALL: [Self; 2] = [Self::Fps30, Self::Fps60];

    pub const fn value(self) -> u32 {
        match self {
            Self::Fps30 => 30,
            Self::Fps60 => 60,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Fps30 => "30 FPS",
            Self::Fps60 => "60 FPS",
        }
    }

    pub const fn frame_interval_us(self) -> u64 {
        1_000_000 / (self.value() as u64)
    }

    pub const fn rtp_timestamp_step(self) -> u32 {
        RTP_VIDEO_CLOCK_HZ / self.value()
    }

    pub const fn from_value(fps: u32) -> Option<Self> {
        match fps {
            30 => Some(Self::Fps30),
            60 => Some(Self::Fps60),
            _ => None,
        }
    }
}

/// Комбинированные пресеты качества трансляции (720p/1080p × 30/60 FPS)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreenShareQualityPreset {
    P720Fps30,
    P720Fps60,
    P1080Fps30,
    #[default]
    P1080Fps60,
}

impl ScreenShareQualityPreset {
    pub const ALL: [Self; 4] = [
        Self::P720Fps30,
        Self::P720Fps60,
        Self::P1080Fps30,
        Self::P1080Fps60,
    ];

    pub const fn from_parts(resolution: ScreenShareResolution, fps: ScreenShareFps) -> Self {
        match (resolution, fps) {
            (ScreenShareResolution::P720, ScreenShareFps::Fps30) => Self::P720Fps30,
            (ScreenShareResolution::P720, ScreenShareFps::Fps60) => Self::P720Fps60,
            (ScreenShareResolution::P1080, ScreenShareFps::Fps30) => Self::P1080Fps30,
            (ScreenShareResolution::P1080, ScreenShareFps::Fps60) => Self::P1080Fps60,
        }
    }

    pub fn from_dimensions_and_fps(width: u32, height: u32, fps: u32) -> Result<Self, String> {
        let res = match (width, height) {
            (1280, 720) => ScreenShareResolution::P720,
            (1920, 1080) => ScreenShareResolution::P1080,
            _ => {
                return Err(format!(
                    "Неподдерживаемое разрешение пресета: {width}x{height} (допустимы 1280x720 и 1920x1080)"
                ));
            }
        };
        let fps_preset = ScreenShareFps::from_value(fps).ok_or_else(|| {
            format!("Неподдерживаемая частота кадров пресета: {fps} FPS (допустимы 30 и 60)")
        })?;
        Ok(Self::from_parts(res, fps_preset))
    }

    pub const fn resolution(self) -> ScreenShareResolution {
        match self {
            Self::P720Fps30 | Self::P720Fps60 => ScreenShareResolution::P720,
            Self::P1080Fps30 | Self::P1080Fps60 => ScreenShareResolution::P1080,
        }
    }

    pub const fn fps(self) -> ScreenShareFps {
        match self {
            Self::P720Fps30 | Self::P1080Fps30 => ScreenShareFps::Fps30,
            Self::P720Fps60 | Self::P1080Fps60 => ScreenShareFps::Fps60,
        }
    }

    pub const fn dimensions(self) -> (u32, u32) {
        self.resolution().dimensions()
    }

    pub const fn fps_value(self) -> u32 {
        self.fps().value()
    }

    pub const fn recommended_bitrate_bps(self) -> u32 {
        match self {
            Self::P720Fps30 => 4_000_000,
            Self::P720Fps60 => 8_000_000,
            Self::P1080Fps30 => 8_000_000,
            Self::P1080Fps60 => 16_000_000,
        }
    }

    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::P720Fps30 => "720p • 30 FPS",
            Self::P720Fps60 => "720p • 60 FPS",
            Self::P1080Fps30 => "1080p • 30 FPS",
            Self::P1080Fps60 => "1080p • 60 FPS",
        }
    }

    pub const fn summary_label(self) -> &'static str {
        match self {
            Self::P720Fps30 => "Качество: 720p @ 30 FPS",
            Self::P720Fps60 => "Качество: 720p @ 60 FPS",
            Self::P1080Fps30 => "Качество: 1080p @ 30 FPS",
            Self::P1080Fps60 => "Качество: 1080p @ 60 FPS",
        }
    }

    pub const fn to_negotiation_request(
        self,
        target_kind: CaptureTargetKind,
    ) -> ScreenCastNegotiationRequest {
        ScreenCastNegotiationRequest::from_preset(
            self.resolution().capture_preset(),
            self.fps_value(),
            target_kind,
        )
    }

    pub fn negotiate_stream(
        self,
        node_id: u32,
        source_size: Option<(u32, u32)>,
        tab: ScreenShareTab,
    ) -> Result<NegotiatedStreamParams, String> {
        let target_kind = tab.capture_target_kind();
        let stream = PortalStreamInfo {
            node_id,
            id: Some(format!("pw-node-{node_id}")),
            source_type: tab.portal_source_types().bits(),
            position: Some((0, 0)),
            size: source_size,
            mapping_id: None,
        };
        let req = self.to_negotiation_request(target_kind);
        negotiate_stream_params(&stream, &req).map_err(|e| e.to_string())
    }
}

/// Профиль кодирования контента (плавность движений или чёткость текста)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreenShareContentHint {
    #[default]
    Motion,
    Detail,
}

impl ScreenShareContentHint {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Motion => "Плавность видео",
            Self::Detail => "Чёткость текста",
        }
    }
}

/// Абстракция запуска и остановки сессии портала `ScreenCast` (для мокирования в оффлайн-тестах
/// и работы с реальным [`ScreenCastPortal`] / [`ScreenCastCapturePipeline`]).
pub trait ScreenCastPortalRunner {
    fn start_portal_session(
        &mut self,
        options: &ScreenCastOptions,
        request: &ScreenCastNegotiationRequest,
    ) -> Result<(Option<String>, NegotiatedStreamParams), PortalRequestError>;

    fn stop_portal_session(
        &mut self,
        session_handle: Option<&str>,
    ) -> Result<(), PortalRequestError>;
}

/// Оффлайн-раннер портала `ScreenCast` для контрактного тестирования отказов, отмены и успеха.
#[derive(Debug, Clone)]
pub struct MockScreenCastPortalRunner {
    start_result: Result<(Option<String>, NegotiatedStreamParams), PortalRequestError>,
    pub stop_calls: usize,
}

impl MockScreenCastPortalRunner {
    #[must_use]
    pub fn succeeding(params: NegotiatedStreamParams) -> Self {
        Self {
            start_result: Ok((
                Some("/org/freedesktop/portal/desktop/session/mock/s1".to_string()),
                params,
            )),
            stop_calls: 0,
        }
    }

    #[must_use]
    pub fn failing(err: PortalRequestError) -> Self {
        Self {
            start_result: Err(err),
            stop_calls: 0,
        }
    }
}

impl ScreenCastPortalRunner for MockScreenCastPortalRunner {
    fn start_portal_session(
        &mut self,
        _options: &ScreenCastOptions,
        _request: &ScreenCastNegotiationRequest,
    ) -> Result<(Option<String>, NegotiatedStreamParams), PortalRequestError> {
        self.start_result.clone()
    }

    fn stop_portal_session(
        &mut self,
        _session_handle: Option<&str>,
    ) -> Result<(), PortalRequestError> {
        self.stop_calls += 1;
        Ok(())
    }
}

/// Состояние нативного селектора экранов и окон
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenShareSelectorState {
    pub active_tab: ScreenShareTab,
    pub resolution: ScreenShareResolution,
    pub fps: ScreenShareFps,
    pub share_audio: bool,
    pub embed_cursor: bool,
    pub content_hint: ScreenShareContentHint,
    pub selected_source_id: Option<String>,
    pub last_started_stream: Option<ScreenCastState>,
    pub last_error: Option<String>,
    #[serde(skip)]
    pub active_session_handle: Option<String>,
    #[serde(skip)]
    pub negotiated_stream: Option<NegotiatedStreamParams>,
}

impl Default for ScreenShareSelectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenShareSelectorState {
    pub fn new() -> Self {
        Self {
            active_tab: ScreenShareTab::Monitors,
            resolution: ScreenShareResolution::P1080,
            fps: ScreenShareFps::Fps60,
            share_audio: true,
            embed_cursor: true,
            content_hint: ScreenShareContentHint::Motion,
            selected_source_id: None,
            last_started_stream: None,
            last_error: None,
            active_session_handle: None,
            negotiated_stream: None,
        }
    }

    pub const fn quality_preset(&self) -> ScreenShareQualityPreset {
        ScreenShareQualityPreset::from_parts(self.resolution, self.fps)
    }

    pub fn set_tab(&mut self, tab: ScreenShareTab, state: &mut AppState) {
        self.active_tab = tab;
        self.last_error = None;
        let current_matches = self
            .selected_source_id
            .as_deref()
            .or(state.selected_screen_source_id.as_deref())
            .and_then(|id| state.available_screen_sources.iter().find(|s| s.id == id))
            .is_some_and(|s| tab.matches_source(s));

        if !current_matches {
            let first_in_tab = state
                .available_screen_sources
                .iter()
                .find(|s| tab.matches_source(s))
                .map(|s| s.id.clone());
            self.selected_source_id = first_in_tab.clone();
            state.selected_screen_source_id = first_in_tab;
        }
    }

    pub fn set_resolution(&mut self, resolution: ScreenShareResolution) {
        self.resolution = resolution;
        self.last_error = None;
    }

    pub fn set_fps(&mut self, fps: ScreenShareFps) {
        self.fps = fps;
        self.last_error = None;
    }

    pub fn apply_quality_preset(&mut self, preset: ScreenShareQualityPreset) {
        self.resolution = preset.resolution();
        self.fps = preset.fps();
        self.last_error = None;
    }

    pub fn toggle_share_audio(&mut self) {
        self.share_audio = !self.share_audio;
    }

    pub fn toggle_embed_cursor(&mut self) {
        self.embed_cursor = !self.embed_cursor;
    }

    pub fn toggle_content_hint(&mut self) {
        self.content_hint = match self.content_hint {
            ScreenShareContentHint::Motion => ScreenShareContentHint::Detail,
            ScreenShareContentHint::Detail => ScreenShareContentHint::Motion,
        };
    }

    pub fn filtered_sources<'a>(&self, state: &'a AppState) -> Vec<&'a ScreenSource> {
        state
            .available_screen_sources
            .iter()
            .filter(|s| self.active_tab.matches_source(s))
            .collect()
    }

    pub fn tab_counts(state: &AppState) -> (usize, usize) {
        let monitors = state
            .available_screen_sources
            .iter()
            .filter(|s| s.is_screen)
            .count();
        let windows = state
            .available_screen_sources
            .iter()
            .filter(|s| !s.is_screen)
            .count();
        (monitors, windows)
    }

    pub fn effective_selected_source<'a>(&self, state: &'a AppState) -> Option<&'a ScreenSource> {
        let candidate_id = self
            .selected_source_id
            .as_deref()
            .or(state.selected_screen_source_id.as_deref());

        if let Some(id) = candidate_id
            && let Some(found) = state
                .available_screen_sources
                .iter()
                .find(|s| s.id == id && self.active_tab.matches_source(s))
        {
            return Some(found);
        }

        state
            .available_screen_sources
            .iter()
            .find(|s| self.active_tab.matches_source(s))
            .or_else(|| state.available_screen_sources.first())
    }

    pub fn select_source(
        &mut self,
        source_id: &str,
        state: &mut AppState,
    ) -> Result<ScreenSource, String> {
        let Some(src) = state
            .available_screen_sources
            .iter()
            .find(|s| s.id == source_id)
            .cloned()
        else {
            let err = format!("Источник захвата '{source_id}' не найден в списке доступных");
            self.last_error = Some(err.clone());
            return Err(err);
        };

        self.active_tab = if src.is_screen {
            ScreenShareTab::Monitors
        } else {
            ScreenShareTab::Windows
        };
        self.selected_source_id = Some(src.id.clone());
        state.selected_screen_source_id = Some(src.id.clone());
        self.last_error = None;
        Ok(src)
    }

    pub fn build_portal_options(&self) -> ScreenCastOptions {
        ScreenCastManager::build_portal_options(
            self.active_tab.portal_source_types(),
            self.embed_cursor,
        )
    }

    pub const fn build_negotiation_request(&self) -> ScreenCastNegotiationRequest {
        self.quality_preset()
            .to_negotiation_request(self.active_tab.capture_target_kind())
    }

    fn format_portal_error(err: &PortalRequestError) -> String {
        match err {
            PortalRequestError::ConsentDenied => format!("Cancelled: {err}"),
            PortalRequestError::ConsentAborted => format!("Cancelled (aborted): {err}"),
            other => other.to_string(),
        }
    }

    fn apply_started_stream_to_state(
        &mut self,
        state: &mut AppState,
        src: ScreenSource,
        params: &NegotiatedStreamParams,
    ) {
        let source_type = if src.is_screen { "Monitor" } else { "Window" }.to_string();
        state.start_screen_sharing(&src.id);
        if let CallStatus::Connected {
            ref mut is_screen_sharing,
            ..
        } = state.active_call
        {
            *is_screen_sharing = true;
        }

        let cast_state = ScreenCastState {
            is_active: true,
            source_type,
            source_name: src.name,
            fps: params.fps,
            resolution_width: params.width,
            resolution_height: params.height,
            pipewire_node_id: Some(params.node_id),
        };

        self.selected_source_id = Some(src.id);
        self.last_started_stream = Some(cast_state);
        self.negotiated_stream = Some(params.clone());
        self.last_error = None;
    }

    pub fn confirm_go_live_with_pipeline(
        &mut self,
        state: &mut AppState,
        portal: &mut ScreenCastPortal,
        pipeline: &mut ScreenCastCapturePipeline,
    ) -> Result<NegotiatedStreamParams, String> {
        let Some(src) = self.effective_selected_source(state).cloned() else {
            let err = "Не выбран источник для демонстрации экрана".to_string();
            self.last_error = Some(err.clone());
            return Err(err);
        };

        let options = self.build_portal_options();
        let request = self.build_negotiation_request();
        let params = match pipeline.start_session(portal, &options, &request) {
            Ok(params) => params,
            Err(portal_err) => {
                let msg = Self::format_portal_error(&portal_err);
                self.last_error = Some(msg.clone());
                return Err(msg);
            }
        };

        self.active_session_handle = pipeline.session_handle().map(str::to_string);
        self.apply_started_stream_to_state(state, src, &params);
        Ok(params)
    }

    pub fn confirm_go_live_with_portal(
        &mut self,
        state: &mut AppState,
        portal: &mut ScreenCastPortal,
    ) -> Result<NegotiatedStreamParams, String> {
        let mut pipeline = ScreenCastCapturePipeline::new();
        self.confirm_go_live_with_pipeline(state, portal, &mut pipeline)
    }

    pub fn confirm_go_live_with_runner<R: ScreenCastPortalRunner>(
        &mut self,
        state: &mut AppState,
        runner: &mut R,
    ) -> Result<NegotiatedStreamParams, String> {
        let Some(src) = self.effective_selected_source(state).cloned() else {
            let err = "Не выбран источник для демонстрации экрана".to_string();
            self.last_error = Some(err.clone());
            return Err(err);
        };

        let options = self.build_portal_options();
        let request = self.build_negotiation_request();
        let (session_handle, params) = match runner.start_portal_session(&options, &request) {
            Ok(res) => res,
            Err(portal_err) => {
                let msg = Self::format_portal_error(&portal_err);
                self.last_error = Some(msg.clone());
                return Err(msg);
            }
        };

        self.active_session_handle = session_handle;
        self.apply_started_stream_to_state(state, src, &params);
        Ok(params)
    }

    pub fn switch_live_source_with_pipeline(
        &mut self,
        source_id: &str,
        state: &mut AppState,
        pipeline: &mut ScreenCastCapturePipeline,
    ) -> Result<NegotiatedStreamParams, String> {
        let src = self.select_source(source_id, state)?;
        let target_kind = if src.is_screen {
            CaptureTargetKind::Monitor
        } else {
            CaptureTargetKind::Window
        };
        let preset = self.resolution.capture_preset();
        let target_fps = self.fps.value();

        let params = match pipeline.switch_stream_target(target_kind, preset, target_fps) {
            Ok(params) => params,
            Err(portal_err) => {
                let msg = Self::format_portal_error(&portal_err);
                self.last_error = Some(msg.clone());
                return Err(msg);
            }
        };

        self.apply_started_stream_to_state(state, src, &params);
        Ok(params)
    }

    pub fn confirm_go_live(&mut self, state: &mut AppState) -> Result<ScreenCastState, String> {
        let Some(src) = self.effective_selected_source(state).cloned() else {
            let err = "Не выбран источник для демонстрации экрана".to_string();
            self.last_error = Some(err.clone());
            return Err(err);
        };

        let (width, height) = self.resolution.dimensions();
        let fps = self.fps.value();
        let source_type = if src.is_screen { "Monitor" } else { "Window" }.to_string();

        state.start_screen_sharing(&src.id);
        if let CallStatus::Connected {
            ref mut is_screen_sharing,
            ..
        } = state.active_call
        {
            *is_screen_sharing = true;
        }

        let cast_state = ScreenCastState {
            is_active: true,
            source_type,
            source_name: src.name,
            fps,
            resolution_width: width,
            resolution_height: height,
            pipewire_node_id: Some(if src.is_screen { 101 } else { 202 }),
        };

        self.selected_source_id = Some(src.id);
        self.last_started_stream = Some(cast_state.clone());
        self.last_error = None;
        Ok(cast_state)
    }

    pub fn stop_go_live(&mut self, state: &mut AppState) {
        state.stop_screen_sharing();
        if let CallStatus::Connected {
            ref mut is_screen_sharing,
            ..
        } = state.active_call
        {
            *is_screen_sharing = false;
        }
        if let Some(ref mut last) = self.last_started_stream {
            last.is_active = false;
        }
        self.active_session_handle = None;
        self.negotiated_stream = None;
    }

    pub fn stop_go_live_with_pipeline(
        &mut self,
        state: &mut AppState,
        portal: &mut ScreenCastPortal,
        pipeline: &mut ScreenCastCapturePipeline,
    ) -> Result<(), String> {
        let res = pipeline
            .stop(portal)
            .map_err(|e| Self::format_portal_error(&e));
        portal.invalidate_pipewire_cache();
        self.stop_go_live(state);
        if let Err(ref err) = res {
            self.last_error = Some(err.clone());
        }
        res
    }

    pub fn stop_go_live_with_portal(
        &mut self,
        state: &mut AppState,
        portal: &mut ScreenCastPortal,
    ) -> Result<(), String> {
        let res = if let Some(handle) = self.active_session_handle.take() {
            portal
                .close_session(&handle)
                .map_err(|e| Self::format_portal_error(&e))
        } else {
            Ok(())
        };
        portal.invalidate_pipewire_cache();
        self.stop_go_live(state);
        if let Err(ref err) = res {
            self.last_error = Some(err.clone());
        }
        res
    }

    pub fn status_summary(&self) -> String {
        let preset = self.quality_preset();
        let mbps = (preset.recommended_bitrate_bps() as f32) / 1_000_000.0;
        let audio_tag = if self.share_audio {
            "Звук вкл."
        } else {
            "Без звука"
        };
        format!(
            "{} • {:.1} Мбит/с • {} • {}",
            preset.summary_label(),
            mbps,
            audio_tag,
            self.content_hint.label()
        )
    }
}

static GLOBAL_SELECTOR_STATE: LazyLock<Mutex<ScreenShareSelectorState>> =
    LazyLock::new(|| Mutex::new(ScreenShareSelectorState::new()));

/// Модальное окно выбора экрана или окна для стриминга (Screen Share Picker)
/// Реализует показ экрана по эталону Discord UI с разделением на «Экраны» и «Окна приложений»
/// и пресетами качества 720p/1080p × 30/60 FPS.
pub struct ScreenShareModal;

impl ScreenShareModal {
    pub fn snapshot_selector_state() -> ScreenShareSelectorState {
        GLOBAL_SELECTOR_STATE
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn update_selector_state<F>(f: F)
    where
        F: FnOnce(&mut ScreenShareSelectorState),
    {
        if let Ok(mut guard) = GLOBAL_SELECTOR_STATE.lock() {
            f(&mut guard);
        }
    }

    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_screen_share_modal_open {
            return div().into_any_element();
        }

        let selector = Self::snapshot_selector_state();
        let (monitors_cnt, windows_cnt) = ScreenShareSelectorState::tab_counts(state);
        let filtered = selector.filtered_sources(state);
        let selected_id = selector
            .effective_selected_source(state)
            .map(|s| s.id.clone());

        div()
            .id("screen-share-modal-backdrop")
            .absolute()
            .inset_0()
            .bg(DISCORD_THEME.bg_backdrop)
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.close_screen_share_modal();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("screen-share-modal-dialog")
                    .v_flex()
                    .w(px(620.))
                    .bg(DISCORD_THEME.bg_modal)
                    .rounded(px(8.))
                    .shadow_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .on_mouse_down(MouseButton::Left, |_e, _window, cx| {
                        cx.stop_propagation();
                    })
                    .child(Self::render_header(&selector, cx))
                    .child(Self::render_tabs_bar(
                        &selector,
                        monitors_cnt,
                        windows_cnt,
                        cx,
                    ))
                    .child(Self::render_sources_grid(
                        &selector,
                        &filtered,
                        selected_id.as_deref(),
                        cx,
                    ))
                    .child(Self::render_quality_controls(&selector, cx))
                    .child(Self::render_footer(&selector, cx)),
            )
            .into_any_element()
    }

    fn render_header(
        selector: &ScreenShareSelectorState,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let badge = selector.quality_preset().badge_label();
        div()
            .px(px(20.))
            .py(px(16.))
            .flex()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(DISCORD_THEME.border_subtle)
            .child(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Демонстрация экрана"),
                            )
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.blurple)
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.btn_primary_text)
                                    .child(badge),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Выберите экран или окно приложения и настройте параметры трансляции"),
                    ),
            )
            .child(
                div()
                    .id("screen-share-close-btn")
                    .cursor_pointer()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(4.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover).text_color(gpui::white()))
                    .text_color(DISCORD_THEME.text_muted)
                    .child("✕")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.close_screen_share_modal();
                            cx.notify();
                        }),
                    ),
            )
    }

    fn render_tabs_bar(
        selector: &ScreenShareSelectorState,
        monitors_cnt: usize,
        windows_cnt: usize,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let is_monitors = selector.active_tab == ScreenShareTab::Monitors;
        let is_windows = selector.active_tab == ScreenShareTab::Windows;

        div()
            .px(px(20.))
            .pt(px(12.))
            .pb(px(8.))
            .flex()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(DISCORD_THEME.border_subtle)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Self::render_tab_button(
                        "screen-share-tab-monitors",
                        ScreenShareTab::Monitors,
                        monitors_cnt,
                        is_monitors,
                        cx,
                    ))
                    .child(Self::render_tab_button(
                        "screen-share-tab-windows",
                        ScreenShareTab::Windows,
                        windows_cnt,
                        is_windows,
                        cx,
                    )),
            )
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(4.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .text_size(px(11.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child("PipeWire • XDG Portal"),
            )
    }

    fn render_tab_button(
        id: &'static str,
        tab: ScreenShareTab,
        count: usize,
        is_active: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let label = format!("{} {} ({})", tab.icon(), tab.label(), count);
        div()
            .id(id)
            .px(px(12.))
            .py(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .bg(if is_active {
                DISCORD_THEME.blurple
            } else {
                DISCORD_THEME.bg_secondary
            })
            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
            .text_size(px(13.))
            .font_weight(FontWeight::BOLD)
            .text_color(if is_active {
                DISCORD_THEME.interactive_active
            } else {
                DISCORD_THEME.text_normal
            })
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    Self::update_selector_state(|sel| {
                        sel.set_tab(tab, &mut this.state);
                    });
                    cx.notify();
                }),
            )
    }

    fn render_sources_grid(
        selector: &ScreenShareSelectorState,
        sources: &[&ScreenSource],
        selected_id: Option<&str>,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let mut container = div().px(px(20.)).py(px(14.)).v_flex().gap(px(10.));

        if sources.is_empty() {
            return container.child(
                div()
                    .h(px(120.))
                    .w_full()
                    .rounded(px(6.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child(selector.active_tab.empty_message()),
            );
        }

        let badge = selector.quality_preset().badge_label();
        let mut cards = Vec::with_capacity(sources.len());
        for src in sources {
            cards.push(Self::render_source_card(src, selected_id, badge, cx));
        }
        container = container.child(div().flex().flex_wrap().gap(px(12.)).children(cards));
        container
    }

    fn render_source_card(
        src: &ScreenSource,
        selected_id: Option<&str>,
        quality_badge: &'static str,
        cx: &mut Context<AppLayout>,
    ) -> Stateful<Div> {
        let sid = src.id.clone();
        let name = src.name.clone();
        let icon = src.preview_icon.clone();
        let kind_tag = if src.is_screen {
            "ЭКРАН"
        } else {
            "ОКНО"
        };
        let is_selected = selected_id == Some(sid.as_str());

        div()
            .id(SharedString::from(format!("screen-share-source-{sid}")))
            .w(px(280.))
            .p(px(10.))
            .rounded(px(6.))
            .bg(if is_selected {
                DISCORD_THEME.bg_active
            } else {
                DISCORD_THEME.bg_secondary
            })
            .border_2()
            .border_color(if is_selected {
                DISCORD_THEME.blurple
            } else {
                DISCORD_THEME.border_subtle
            })
            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
            .cursor_pointer()
            .v_flex()
            .gap(px(8.))
            .child(
                div()
                    .h(px(84.))
                    .w_full()
                    .rounded(px(4.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .p(px(6.))
                    .v_flex()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(3.))
                                    .bg(DISCORD_THEME.bg_secondary)
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(kind_tag),
                            )
                            .child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(3.))
                                    .bg(DISCORD_THEME.blurple)
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.btn_primary_text)
                                    .child(quality_badge),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(28.))
                            .child(icon),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child(name),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    let sid_clone = sid.clone();
                    Self::update_selector_state(|sel| {
                        let _ = sel.select_source(&sid_clone, &mut this.state);
                    });
                    cx.notify();
                }),
            )
    }

    fn render_quality_controls(
        selector: &ScreenShareSelectorState,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let current_preset = selector.quality_preset();

        div()
            .px(px(20.))
            .py(px(12.))
            .bg(DISCORD_THEME.bg_secondary_alt)
            .border_t_1()
            .border_color(DISCORD_THEME.border_subtle)
            .v_flex()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_muted)
                    .child("КАЧЕСТВО ТРАНСЛЯЦИИ (720P / 1080P • 30 / 60 FPS):"),
            )
            .child(div().flex().flex_wrap().gap(px(8.)).children(
                ScreenShareQualityPreset::ALL.iter().copied().map(|preset| {
                    let is_active = preset == current_preset;
                    div()
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .bg(if is_active {
                            DISCORD_THEME.blurple
                        } else {
                            DISCORD_THEME.bg_secondary
                        })
                        .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                        .text_size(px(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_active {
                            DISCORD_THEME.btn_primary_text
                        } else {
                            DISCORD_THEME.text_normal
                        })
                        .child(preset.badge_label())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_this, _, _window, cx| {
                                Self::update_selector_state(|sel| {
                                    sel.apply_quality_preset(preset);
                                });
                                cx.notify();
                            }),
                        )
                }),
            ))
            .child(Self::render_segmented_row(selector, cx))
    }

    fn render_segmented_row(
        selector: &ScreenShareSelectorState,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let current_res = selector.resolution;
        let current_fps = selector.fps;
        let share_audio = selector.share_audio;
        let embed_cursor = selector.embed_cursor;

        div()
            .flex()
            .justify_between()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .children(ScreenShareResolution::ALL.iter().copied().map(|res| {
                        let active = res == current_res;
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .bg(if active {
                                DISCORD_THEME.bg_active
                            } else {
                                DISCORD_THEME.bg_secondary
                            })
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(res.subtitle())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _window, cx| {
                                    Self::update_selector_state(|sel| sel.set_resolution(res));
                                    cx.notify();
                                }),
                            )
                    }))
                    .children(ScreenShareFps::ALL.iter().copied().map(|fps_opt| {
                        let active = fps_opt == current_fps;
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .bg(if active {
                                DISCORD_THEME.bg_active
                            } else {
                                DISCORD_THEME.bg_secondary
                            })
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(fps_opt.label())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _window, cx| {
                                    Self::update_selector_state(|sel| sel.set_fps(fps_opt));
                                    cx.notify();
                                }),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .id("screen-share-toggle-audio")
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .bg(if share_audio {
                                DISCORD_THEME.btn_success_bg
                            } else {
                                DISCORD_THEME.bg_secondary
                            })
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(if share_audio {
                                "🔊 Звук вкл."
                            } else {
                                "🔇 Без звука"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, cx| {
                                    Self::update_selector_state(|sel| sel.toggle_share_audio());
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("screen-share-toggle-cursor")
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .bg(if embed_cursor {
                                DISCORD_THEME.bg_active
                            } else {
                                DISCORD_THEME.bg_secondary
                            })
                            .text_size(px(11.))
                            .text_color(DISCORD_THEME.text_normal)
                            .child(if embed_cursor {
                                "🖱️ Курсор вкл."
                            } else {
                                "🖱️ Курсор скрыт"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, cx| {
                                    Self::update_selector_state(|sel| sel.toggle_embed_cursor());
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    fn render_footer(
        selector: &ScreenShareSelectorState,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let summary = selector.status_summary();

        div()
            .px(px(20.))
            .py(px(14.))
            .bg(DISCORD_THEME.bg_modal_footer)
            .border_t_1()
            .border_color(DISCORD_THEME.border_subtle)
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child(summary),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .id("screen-share-cancel-btn")
                            .px(px(14.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.btn_secondary_bg)
                            .hover(|s| s.bg(DISCORD_THEME.btn_secondary_hover))
                            .cursor_pointer()
                            .text_color(DISCORD_THEME.btn_secondary_text)
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .child("Отмена")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.close_screen_share_modal();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("screen-share-go-live-btn")
                            .px(px(16.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.blurple)
                            .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                            .cursor_pointer()
                            .text_color(DISCORD_THEME.btn_primary_text)
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .child("Выйти в прямой эфир")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    Self::update_selector_state(|sel| {
                                        let _ = sel.confirm_go_live(&mut this.state);
                                    });
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_platform::screencast::PortalCursorMode;

    #[core::prelude::v1::test]
    fn screen_share_tabs_and_source_filtering_contract() {
        let mut app_state = AppState::new();
        let mut selector = ScreenShareSelectorState::new();

        // По умолчанию открыта вкладка "Экраны" (Monitors), пресет 1080p @ 60 FPS
        assert_eq!(selector.active_tab, ScreenShareTab::Monitors);
        assert_eq!(ScreenShareTab::Monitors.label(), "Экраны");
        assert_eq!(ScreenShareTab::Windows.label(), "Окна приложений");
        assert_eq!(
            ScreenShareTab::Monitors.portal_source_types(),
            PortalSourceTypes::MONITOR
        );
        assert_eq!(
            ScreenShareTab::Windows.portal_source_types(),
            PortalSourceTypes::WINDOW
        );
        assert_eq!(
            ScreenShareTab::Monitors.capture_target_kind(),
            CaptureTargetKind::Monitor
        );
        assert_eq!(
            ScreenShareTab::Windows.capture_target_kind(),
            CaptureTargetKind::Window
        );

        let (monitors_cnt, windows_cnt) = ScreenShareSelectorState::tab_counts(&app_state);
        assert_eq!(monitors_cnt, 1);
        assert_eq!(windows_cnt, 2);

        // Во вкладке Monitors виден только экран (is_screen == true)
        let monitors = selector.filtered_sources(&app_state);
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].id, "screen-full-1");
        assert!(monitors[0].is_screen);

        // Переключение на вкладку Windows показывает окна приложений (is_screen == false)
        selector.set_tab(ScreenShareTab::Windows, &mut app_state);
        assert_eq!(selector.active_tab, ScreenShareTab::Windows);
        let windows = selector.filtered_sources(&app_state);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].id, "window-discord");
        assert_eq!(windows[1].id, "window-code");
        assert!(!windows[0].is_screen);
        assert_eq!(
            app_state.selected_screen_source_id.as_deref(),
            Some("window-discord")
        );
    }

    #[core::prelude::v1::test]
    fn screen_share_quality_presets_720p_1080p_30_60fps_contract() {
        // Все 4 обязательных пресета: 720p/1080p × 30/60 FPS
        let p720_30 = ScreenShareQualityPreset::from_parts(
            ScreenShareResolution::P720,
            ScreenShareFps::Fps30,
        );
        assert_eq!(p720_30, ScreenShareQualityPreset::P720Fps30);
        assert_eq!(p720_30.dimensions(), (1280, 720));
        assert_eq!(p720_30.fps_value(), 30);
        assert_eq!(p720_30.fps().frame_interval_us(), 33_333);
        assert_eq!(p720_30.fps().rtp_timestamp_step(), 3_000);
        assert_eq!(p720_30.recommended_bitrate_bps(), 4_000_000);
        assert_eq!(p720_30.badge_label(), "720p • 30 FPS");

        let p720_60 = ScreenShareQualityPreset::from_parts(
            ScreenShareResolution::P720,
            ScreenShareFps::Fps60,
        );
        assert_eq!(p720_60, ScreenShareQualityPreset::P720Fps60);
        assert_eq!(p720_60.dimensions(), (1280, 720));
        assert_eq!(p720_60.fps_value(), 60);
        assert_eq!(p720_60.fps().frame_interval_us(), 16_666);
        assert_eq!(p720_60.fps().rtp_timestamp_step(), 1_500);
        assert_eq!(p720_60.recommended_bitrate_bps(), 8_000_000);
        assert_eq!(p720_60.badge_label(), "720p • 60 FPS");

        let p1080_30 = ScreenShareQualityPreset::from_parts(
            ScreenShareResolution::P1080,
            ScreenShareFps::Fps30,
        );
        assert_eq!(p1080_30, ScreenShareQualityPreset::P1080Fps30);
        assert_eq!(p1080_30.dimensions(), (1920, 1080));
        assert_eq!(p1080_30.fps_value(), 30);
        assert_eq!(p1080_30.recommended_bitrate_bps(), 8_000_000);
        assert_eq!(p1080_30.badge_label(), "1080p • 30 FPS");

        let p1080_60 = ScreenShareQualityPreset::from_parts(
            ScreenShareResolution::P1080,
            ScreenShareFps::Fps60,
        );
        assert_eq!(p1080_60, ScreenShareQualityPreset::P1080Fps60);
        assert_eq!(p1080_60.dimensions(), (1920, 1080));
        assert_eq!(p1080_60.fps_value(), 60);
        assert_eq!(p1080_60.recommended_bitrate_bps(), 16_000_000);
        assert_eq!(p1080_60.badge_label(), "1080p • 60 FPS");

        // Согласование параметров PipeWire для всех 4 пресетов
        for preset in ScreenShareQualityPreset::ALL {
            let negotiated = preset
                .negotiate_stream(101, Some((2560, 1440)), ScreenShareTab::Monitors)
                .expect("Должно успешно согласовать поток");
            assert_eq!((negotiated.width, negotiated.height), preset.dimensions());
            assert_eq!(negotiated.fps, preset.fps_value());
            assert_eq!(negotiated.rtp_clock_hz, RTP_VIDEO_CLOCK_HZ);
        }
    }

    #[core::prelude::v1::test]
    fn screen_share_negative_and_boundary_validation_contract() {
        // Негативные сценарии: неподдерживаемые разрешения и FPS
        assert!(ScreenShareResolution::from_height(0).is_none());
        assert!(ScreenShareResolution::from_height(480).is_none());
        assert!(ScreenShareResolution::from_height(1440).is_none());
        assert_eq!(
            ScreenShareResolution::from_height(720),
            Some(ScreenShareResolution::P720)
        );
        assert_eq!(
            ScreenShareResolution::from_height(1080),
            Some(ScreenShareResolution::P1080)
        );

        assert!(ScreenShareFps::from_value(0).is_none());
        assert!(ScreenShareFps::from_value(15).is_none());
        assert!(ScreenShareFps::from_value(120).is_none());
        assert_eq!(ScreenShareFps::from_value(30), Some(ScreenShareFps::Fps30));
        assert_eq!(ScreenShareFps::from_value(60), Some(ScreenShareFps::Fps60));

        assert!(ScreenShareQualityPreset::from_dimensions_and_fps(1280, 720, 24).is_err());
        assert!(ScreenShareQualityPreset::from_dimensions_and_fps(800, 600, 60).is_err());
        assert_eq!(
            ScreenShareQualityPreset::from_dimensions_and_fps(1280, 720, 30).unwrap(),
            ScreenShareQualityPreset::P720Fps30
        );
        assert_eq!(
            ScreenShareQualityPreset::from_dimensions_and_fps(1920, 1080, 60).unwrap(),
            ScreenShareQualityPreset::P1080Fps60
        );

        // Выбор несуществующего источника возвращает ошибку и сохраняет last_error
        let mut app_state = AppState::new();
        let mut selector = ScreenShareSelectorState::new();
        let err = selector
            .select_source("non-existent-source", &mut app_state)
            .expect_err("Ожидалась ошибка для неизвестного ID источника");
        assert!(err.contains("non-existent-source"));
        assert!(selector.last_error.is_some());

        // Пустой список источников: безопасная фильтрация и ошибка при попытке Go Live
        let mut empty_state = AppState::empty();
        assert_eq!(ScreenShareSelectorState::tab_counts(&empty_state), (0, 0));
        assert!(selector.filtered_sources(&empty_state).is_empty());
        assert!(selector.effective_selected_source(&empty_state).is_none());
        assert!(selector.confirm_go_live(&mut empty_state).is_err());
    }

    #[core::prelude::v1::test]
    fn screen_share_go_live_lifecycle_and_portal_options_contract() {
        let mut app_state = AppState::new();
        app_state.start_call("u-demo-1", false);
        app_state.open_screen_share_modal();
        assert!(app_state.is_screen_share_modal_open);

        let mut selector = ScreenShareSelectorState::new();

        // Переключаем пресет на 720p @ 30 FPS и выбираем окно "window-code"
        selector.set_resolution(ScreenShareResolution::P720);
        selector.set_fps(ScreenShareFps::Fps30);
        assert_eq!(
            selector.quality_preset(),
            ScreenShareQualityPreset::P720Fps30
        );

        let chosen = selector
            .select_source("window-code", &mut app_state)
            .expect("Окно window-code должно выбраться");
        assert_eq!(chosen.id, "window-code");
        assert_eq!(selector.active_tab, ScreenShareTab::Windows);
        assert!(selector.last_error.is_none());

        // Проверка опций XDG Desktop Portal для вкладки Windows и переключения курсора
        let opts_embedded = selector.build_portal_options();
        assert_eq!(opts_embedded.source_types, PortalSourceTypes::WINDOW);
        assert_eq!(opts_embedded.cursor_mode, PortalCursorMode::Embedded);

        selector.toggle_embed_cursor();
        assert!(!selector.embed_cursor);
        let opts_meta = selector.build_portal_options();
        assert_eq!(opts_meta.cursor_mode, PortalCursorMode::Metadata);

        selector.toggle_share_audio();
        assert!(!selector.share_audio);
        selector.toggle_content_hint();
        assert_eq!(selector.content_hint, ScreenShareContentHint::Detail);

        // Запуск прямого эфира (Go Live)
        let stream_state = selector
            .confirm_go_live(&mut app_state)
            .expect("Запуск трансляции должен пройти успешно");
        assert!(stream_state.is_active);
        assert_eq!(stream_state.source_type, "Window");
        assert_eq!(stream_state.source_name, "VS Code / Alacritty Terminal");
        assert_eq!(stream_state.resolution_width, 1280);
        assert_eq!(stream_state.resolution_height, 720);
        assert_eq!(stream_state.fps, 30);

        assert!(app_state.is_screen_sharing);
        assert!(!app_state.is_screen_share_modal_open);
        assert_eq!(
            app_state.selected_screen_source_id.as_deref(),
            Some("window-code")
        );
        if let CallStatus::Connected {
            is_screen_sharing, ..
        } = app_state.active_call
        {
            assert!(is_screen_sharing);
        } else {
            panic!("Ожидался активный звонок с включённым стримом");
        }

        // Остановка трансляции и повторный запуск на мониторе в 1080p @ 60 FPS
        selector.stop_go_live(&mut app_state);
        assert!(!app_state.is_screen_sharing);
        assert!(!selector.last_started_stream.as_ref().unwrap().is_active);

        selector.set_tab(ScreenShareTab::Monitors, &mut app_state);
        selector.apply_quality_preset(ScreenShareQualityPreset::P1080Fps60);
        let monitor_stream = selector
            .confirm_go_live(&mut app_state)
            .expect("Повторный запуск на мониторе должен пройти успешно");
        assert_eq!(monitor_stream.source_type, "Monitor");
        assert_eq!(monitor_stream.resolution_width, 1920);
        assert_eq!(monitor_stream.resolution_height, 1080);
        assert_eq!(monitor_stream.fps, 60);
        assert!(selector.status_summary().contains("1080p"));
        assert!(selector.status_summary().contains("60 FPS"));
    }
}
