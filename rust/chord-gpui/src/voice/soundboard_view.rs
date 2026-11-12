//! Сетка кнопок саундборда `SoundboardGridView`, анимация нажатия плитки,
//! привязка хоткеев и слайдер громкости SFX (`TASK-1012C`).
//! Оракулы: `waves/kanban-board.json`, `/home/sanya/re-deep/voice/ui7/`,
//! `/home/sanya/re-deep/voice/ui/01-candidates.json`, `docs/re/frontend-d03.json`
//! и чанки `209932` (`SoundboardStore`), `528772` (`SoundboardOverlayStore`),
//! `125831`, `813564`, `665252`, `533978` (`ActionBarSoundboardButton`), `904054`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const SOUNDBOARD_GRID_CONTAINER: &str = "Soundboard Grid";
pub const SOUNDBOARD_GRID_DIMENSIONS: &str = "320px x 360px";
pub const SOUNDBOARD_GRID_WIDTH_PX: u32 = 320;
pub const SOUNDBOARD_GRID_HEIGHT_PX: u32 = 360;
pub const SOUNDBOARD_GRID_PADDING_PX: u32 = 8;
pub const SOUNDBOARD_GRID_SPACING_PX: u32 = 6;
pub const SOUNDBOARD_GRID_COLUMNS: usize = 3;
pub const SOUND_TILE_WIDTH_PX: u32 = 97;
pub const SOUND_TILE_HEIGHT_PX: u32 = 76;
pub const SOUNDBOARD_TARGET_FPS: u32 = 120;

pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const SOUND_TILE_ELEMENT: &str = "sound-tile";
pub const SOUND_BUTTON_ELEMENT: &str = "sound-button";

pub const UI7_GUILD_ID: &str = "1014537851418980372";
pub const UI7_EFFECTS_CONTAINER_CLASS: &str = "voiceChannelEffectsContainer__2f4f7";
pub const UI7_DISCONNECT_BUTTON_CLASS: &str = "button__67645";
pub const UI7_SEARCH_BAR_CLASS: &str = "search__49676";
pub const UI7_SEARCH_RESULTS_WRAP_CLASS: &str = "searchResultsWrap_a98f3b";
pub const D03_SLIDER_CLASS: &str = "slider__4c059";
pub const D03_BUTTON_CLASS: &str = "button_a22cb0";
pub const D03_SCROLLER_BASE_CLASS: &str = "scrollerBase_d125d2";
pub const RE_ACTION_BAR_BUTTON_NAME: &str = "ActionBarSoundboardButton";
pub const RE_SOUNDBOARD_STORE_NAME: &str = "SoundboardStore";
pub const RE_SOUNDBOARD_OVERLAY_STORE_NAME: &str = "SoundboardOverlayStore";

pub const SOUNDBOARD_DIALOG_ARIA_RU: &str = "Звуковая панель";
pub const SOUNDBOARD_SEARCH_PLACEHOLDER_RU: &str = "Поиск звуков";
pub const SOUNDBOARD_VOLUME_ARIA_RU: &str = "Громкость звуковой панели";
pub const SOUNDBOARD_EMPTY_RESULTS_RU: &str = "Нет результатов";

pub const MIN_SOUNDBOARD_NAME_CHARS: usize = 2;
pub const MAX_SOUNDBOARD_NAME_CHARS: usize = 32;
pub const MAX_SOUNDBOARD_BYTES: usize = 524_288;
pub const MAX_FAVORITE_SOUNDS: usize = 250;

pub const MOD_CTRL: u8 = 1 << 0;
pub const MOD_SHIFT: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_SUPER: u8 = 1 << 3;
pub const MOD_ALL_MASK: u8 = MOD_CTRL | MOD_SHIFT | MOD_ALT | MOD_SUPER;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;
pub const PERM_USE_SOUNDBOARD: u64 = 1 << 42;
pub const PERM_USE_EXTERNAL_SOUNDS: u64 = 1 << 45;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundTileInteractiveState {
    Tile,
    Scale105,
    PlayingSound,
}

impl SoundTileInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tile => "tile",
            Self::Scale105 => "scale-105",
            Self::PlayingSound => "playing sound",
        }
    }

    #[must_use]
    pub const fn as_button_str(self) -> &'static str {
        match self {
            Self::Tile => "sfx tile",
            Self::Scale105 => "scale-105",
            Self::PlayingSound => "playing sfx",
        }
    }

    #[must_use]
    pub const fn resolve(hovered: bool, playing: bool) -> Self {
        match (playing, hovered) {
            (true, _) => Self::PlayingSound,
            (false, true) => Self::Scale105,
            (false, false) => Self::Tile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundboardViewError {
    InvalidSnowflake(String),
    InvalidSoundName(String),
    HotkeyMissingModifier,
    HotkeyInvalidKey(String),
    HotkeyConflict {
        chord: String,
        existing_sound_id: String,
    },
    SoundUnavailable(String),
    UserMutedOrDeafened,
    MissingPermission(&'static str),
    ExternalSoundsNotAllowed,
    AgeGatedRestricted,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    FavoritesLimitReached(usize),
    SoundBytesTooLarge(usize),
    HttpFailure {
        status: u16,
        retry_after_ms: u64,
    },
}

impl fmt::Display for SoundboardViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SoundboardViewError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, SoundboardViewError> {
    let t = raw.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SoundboardViewError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = t
        .parse::<u64>()
        .map_err(|_| SoundboardViewError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(SoundboardViewError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub fn is_valid_sound_name(name: &str) -> bool {
    let len = name.trim().chars().count();
    (MIN_SOUNDBOARD_NAME_CHARS..=MAX_SOUNDBOARD_NAME_CHARS).contains(&len)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundboardSoundEntry {
    pub sound_id: String,
    pub name: String,
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
    pub source_guild_id: Option<String>,
    pub volume: f32,
    pub available: bool,
    pub favorite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TilePressAnimationFrame {
    pub scale: f32,
    pub ring_opacity: f32,
    pub border_hex: &'static str,
    pub state: SoundTileInteractiveState,
}

#[must_use]
pub fn interpolate_tile_press_animation(
    hovered: bool,
    playing: bool,
    elapsed_ms: u32,
    duration_ms: u32,
) -> TilePressAnimationFrame {
    let state = SoundTileInteractiveState::resolve(hovered, playing);
    if !playing {
        return TilePressAnimationFrame {
            scale: if hovered { 1.05 } else { 1.0 },
            ring_opacity: 0.0,
            border_hex: TOKEN_BORDER,
            state,
        };
    }
    let progress = ((elapsed_ms as f32) / (duration_ms.max(1) as f32)).clamp(0.0, 1.0);
    let dip = if progress < 0.25 {
        1.0 - 0.04 * (progress / 0.25)
    } else {
        0.96 + 0.04 * ((progress - 0.25) / 0.75).min(1.0)
    };
    TilePressAnimationFrame {
        scale: (dip * 1000.0).round() / 1000.0,
        ring_opacity: ((1.0 - progress * 0.5) * 100.0).round() / 100.0,
        border_hex: TOKEN_ACCENT,
        state,
    }
}

#[must_use]
pub fn filter_and_sort_soundboard_entries(
    entries: &[SoundboardSoundEntry],
    query: &str,
    favorites_first: bool,
) -> Vec<SoundboardSoundEntry> {
    let needle = query.trim().to_lowercase();
    let mut out: Vec<SoundboardSoundEntry> = entries
        .iter()
        .filter(|e| e.available && is_valid_sound_name(&e.name))
        .filter(|e| needle.is_empty() || e.name.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        let fav_cmp = if favorites_first {
            b.favorite.cmp(&a.favorite)
        } else {
            std::cmp::Ordering::Equal
        };
        fav_cmp
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.sound_id.cmp(&b.sound_id))
    });
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundTileViewSpec {
    pub sound_id: String,
    pub index: usize,
    pub row: usize,
    pub col: usize,
    pub x_px: u32,
    pub y_px: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub display_emoji: String,
    pub title: String,
    pub hotkey_badge: Option<String>,
    pub favorite: bool,
    pub state: SoundTileInteractiveState,
    pub animation: TilePressAnimationFrame,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundboardGridViewSpec {
    pub container: &'static str,
    pub dimensions: &'static str,
    pub width_px: u32,
    pub height_px: u32,
    pub padding_px: u32,
    pub spacing_px: u32,
    pub columns: usize,
    pub query: String,
    pub skeleton_loading: bool,
    pub empty_placeholder_ru: Option<&'static str>,
    pub tiles: Vec<SoundTileViewSpec>,
    pub volume_slider: SfxVolumeSliderSpec,
}

fn build_tile_view(
    idx: usize,
    item: &SoundboardSoundEntry,
    is_hov: bool,
    is_play: bool,
    elapsed_anim_ms: u32,
    hotkeys: &SoundboardHotkeyRegistry,
) -> SoundTileViewSpec {
    let (row, col) = (idx / SOUNDBOARD_GRID_COLUMNS, idx % SOUNDBOARD_GRID_COLUMNS);
    let anim = interpolate_tile_press_animation(is_hov, is_play, elapsed_anim_ms, 240);
    let emoji = item
        .emoji_name
        .clone()
        .or_else(|| item.emoji_id.as_ref().map(|id| format!(":{id}:")))
        .unwrap_or_else(|| "🔊".to_string());
    SoundTileViewSpec {
        sound_id: item.sound_id.clone(),
        index: idx,
        row,
        col,
        x_px: SOUNDBOARD_GRID_PADDING_PX
            + (col as u32) * (SOUND_TILE_WIDTH_PX + SOUNDBOARD_GRID_SPACING_PX),
        y_px: SOUNDBOARD_GRID_PADDING_PX
            + (row as u32) * (SOUND_TILE_HEIGHT_PX + SOUNDBOARD_GRID_SPACING_PX),
        width_px: SOUND_TILE_WIDTH_PX,
        height_px: SOUND_TILE_HEIGHT_PX,
        display_emoji: emoji,
        title: item.name.trim().to_string(),
        hotkey_badge: hotkeys.hotkey_for_sound(&item.sound_id),
        favorite: item.favorite,
        state: anim.state,
        animation: anim,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_soundboard_grid_view(
    entries: &[SoundboardSoundEntry],
    query: &str,
    hovered_sound_id: Option<&str>,
    playing_sound_id: Option<&str>,
    elapsed_anim_ms: u32,
    hotkeys: &SoundboardHotkeyRegistry,
    volume_slider: SfxVolumeSliderSpec,
    skeleton_loading: bool,
) -> SoundboardGridViewSpec {
    let filtered = filter_and_sort_soundboard_entries(entries, query, true);
    let tiles: Vec<SoundTileViewSpec> = filtered
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_hov = hovered_sound_id == Some(item.sound_id.as_str());
            let is_play = playing_sound_id == Some(item.sound_id.as_str());
            build_tile_view(idx, item, is_hov, is_play, elapsed_anim_ms, hotkeys)
        })
        .collect();
    let empty_placeholder_ru =
        (!skeleton_loading && tiles.is_empty()).then_some(SOUNDBOARD_EMPTY_RESULTS_RU);
    SoundboardGridViewSpec {
        container: SOUNDBOARD_GRID_CONTAINER,
        dimensions: SOUNDBOARD_GRID_DIMENSIONS,
        width_px: SOUNDBOARD_GRID_WIDTH_PX,
        height_px: SOUNDBOARD_GRID_HEIGHT_PX,
        padding_px: SOUNDBOARD_GRID_PADDING_PX,
        spacing_px: SOUNDBOARD_GRID_SPACING_PX,
        columns: SOUNDBOARD_GRID_COLUMNS,
        query: query.to_string(),
        skeleton_loading,
        empty_placeholder_ru,
        tiles,
        volume_slider,
    }
}

pub fn format_hotkey_chord(modifiers: u8, key: &str) -> Result<String, SoundboardViewError> {
    if modifiers == 0 {
        return Err(SoundboardViewError::HotkeyMissingModifier);
    }
    if (modifiers & !MOD_ALL_MASK) != 0 {
        return Err(SoundboardViewError::HotkeyInvalidKey(
            "unknown_modifier".to_string(),
        ));
    }
    let normalized_key = normalize_hotkey_key(key)?;
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    for (flag, name) in [
        (MOD_CTRL, "Ctrl"),
        (MOD_SHIFT, "Shift"),
        (MOD_ALT, "Alt"),
        (MOD_SUPER, "Super"),
    ] {
        if (modifiers & flag) != 0 {
            parts.push(name);
        }
    }
    Ok(format!("{}+{normalized_key}", parts.join("+")))
}

fn normalize_hotkey_key(raw_key: &str) -> Result<String, SoundboardViewError> {
    let trimmed = raw_key.trim().to_ascii_uppercase();
    if trimmed.len() == 1 && trimmed.as_bytes()[0].is_ascii_alphanumeric() {
        return Ok(trimmed);
    }
    if let Some(num_str) = trimmed.strip_prefix('F')
        && let Ok(n) = num_str.parse::<u8>()
        && (1..=12).contains(&n)
    {
        return Ok(format!("F{n}"));
    }
    if let Some(num_str) = trimmed.strip_prefix("NUMPAD")
        && num_str.len() == 1
        && num_str.as_bytes()[0].is_ascii_digit()
    {
        return Ok(format!("Numpad{num_str}"));
    }
    Err(SoundboardViewError::HotkeyInvalidKey(raw_key.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundboardHotkeyBinding {
    pub sound_id: String,
    pub modifiers: u8,
    pub key: String,
    pub chord_label: String,
}

#[derive(Debug, Clone, Default)]
pub struct SoundboardHotkeyRegistry {
    by_sound_id: BTreeMap<String, SoundboardHotkeyBinding>,
    sound_by_chord: BTreeMap<String, String>,
}

impl SoundboardHotkeyRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind_hotkey(
        &mut self,
        sound_id: &str,
        modifiers: u8,
        key: &str,
    ) -> Result<SoundboardHotkeyBinding, SoundboardViewError> {
        validate_snowflake(sound_id)?;
        let chord_label = format_hotkey_chord(modifiers, key)?;
        let sid = sound_id.trim().to_string();
        if let Some(existing_sid) = self.sound_by_chord.get(&chord_label)
            && existing_sid != &sid
        {
            return Err(SoundboardViewError::HotkeyConflict {
                chord: chord_label,
                existing_sound_id: existing_sid.clone(),
            });
        }
        if let Some(prev) = self.by_sound_id.remove(&sid) {
            self.sound_by_chord.remove(&prev.chord_label);
        }
        let binding = SoundboardHotkeyBinding {
            sound_id: sid.clone(),
            modifiers,
            key: normalize_hotkey_key(key)?,
            chord_label: chord_label.clone(),
        };
        self.sound_by_chord.insert(chord_label, sid.clone());
        self.by_sound_id.insert(sid, binding.clone());
        Ok(binding)
    }

    pub fn unbind_sound(&mut self, sound_id: &str) -> bool {
        if let Some(prev) = self.by_sound_id.remove(sound_id.trim()) {
            self.sound_by_chord.remove(&prev.chord_label);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn hotkey_for_sound(&self, sound_id: &str) -> Option<String> {
        self.by_sound_id
            .get(sound_id.trim())
            .map(|b| b.chord_label.clone())
    }

    pub fn resolve_triggered_sound(
        &self,
        modifiers: u8,
        key: &str,
        entries: &[SoundboardSoundEntry],
    ) -> Result<Option<String>, SoundboardViewError> {
        let chord = format_hotkey_chord(modifiers, key)?;
        let Some(sid) = self.sound_by_chord.get(&chord) else {
            return Ok(None);
        };
        match entries.iter().find(|e| e.sound_id == *sid) {
            Some(entry) if entry.available => Ok(Some(sid.clone())),
            Some(_) => Err(SoundboardViewError::SoundUnavailable(sid.clone())),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SfxVolumeSliderSpec {
    pub role: &'static str,
    pub css_class: &'static str,
    pub aria_label_ru: &'static str,
    pub min_percent: u16,
    pub max_percent: u16,
    pub value_percent: u16,
    pub track_fill_ratio: f32,
    pub local_soundboard_muted: bool,
    pub effective_gain: f32,
}

#[must_use]
pub fn compute_effective_sfx_gain(
    sound_vol: f32,
    slider_percent: f32,
    output_percent: f32,
    local_soundboard_muted: bool,
) -> f32 {
    if local_soundboard_muted {
        return 0.0;
    }
    let s = sound_vol
        .is_finite()
        .then(|| sound_vol.clamp(0.0, 1.0))
        .unwrap_or(1.0);
    let set_v = slider_percent
        .is_finite()
        .then(|| slider_percent.clamp(0.0, 200.0))
        .unwrap_or(100.0);
    let out_v = output_percent
        .is_finite()
        .then(|| output_percent.clamp(0.0, 200.0))
        .unwrap_or(100.0);
    (s * (set_v / 100.0) * (out_v / 100.0).min(1.0)).min(1.0)
}

#[must_use]
pub fn build_sfx_volume_slider_spec(
    raw_percent: u16,
    sound_vol: f32,
    output_percent: f32,
    local_soundboard_muted: bool,
) -> SfxVolumeSliderSpec {
    let clamped_pct = raw_percent.min(MAX_VOLUME_PERCENT);
    let fill_ratio = (f32::from(clamped_pct) / f32::from(MAX_VOLUME_PERCENT)).clamp(0.0, 1.0);
    let effective_gain = compute_effective_sfx_gain(
        sound_vol,
        f32::from(clamped_pct),
        output_percent,
        local_soundboard_muted,
    );
    SfxVolumeSliderSpec {
        role: "slider",
        css_class: D03_SLIDER_CLASS,
        aria_label_ru: SOUNDBOARD_VOLUME_ARIA_RU,
        min_percent: 0,
        max_percent: MAX_VOLUME_PERCENT,
        value_percent: clamped_pct,
        track_fill_ratio: (fill_ratio * 1000.0).round() / 1000.0,
        local_soundboard_muted,
        effective_gain,
    }
}

pub fn evaluate_soundboard_view_permissions(
    permissions: u64,
    is_dm: bool,
    is_external_sound: bool,
    muted_or_deafened: bool,
    age_gated_restricted: bool,
) -> Result<(), SoundboardViewError> {
    if age_gated_restricted {
        return Err(SoundboardViewError::AgeGatedRestricted);
    }
    if muted_or_deafened {
        return Err(SoundboardViewError::UserMutedOrDeafened);
    }
    if !is_dm && (permissions & PERM_CONNECT) == 0 {
        return Err(SoundboardViewError::MissingPermission("CONNECT"));
    }
    if !is_dm && (permissions & PERM_SPEAK) == 0 {
        return Err(SoundboardViewError::MissingPermission("SPEAK"));
    }
    if !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {
        return Err(SoundboardViewError::MissingPermission("USE_SOUNDBOARD"));
    }
    if !is_dm && is_external_sound && (permissions & PERM_USE_EXTERNAL_SOUNDS) == 0 {
        return Err(SoundboardViewError::ExternalSoundsNotAllowed);
    }
    Ok(())
}

pub fn validate_soundboard_view_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
    favorites_count: usize,
    sound_bytes: usize,
) -> Result<(), SoundboardViewError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(SoundboardViewError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(SoundboardViewError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(SoundboardViewError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(SoundboardViewError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_percent > MAX_VOLUME_PERCENT {
        return Err(SoundboardViewError::VolumeOutOfBounds(volume_percent));
    }
    if favorites_count > MAX_FAVORITE_SOUNDS {
        return Err(SoundboardViewError::FavoritesLimitReached(favorites_count));
    }
    if sound_bytes > MAX_SOUNDBOARD_BYTES {
        return Err(SoundboardViewError::SoundBytesTooLarge(sound_bytes));
    }
    Ok(())
}

pub fn classify_soundboard_view_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, SoundboardViewError> {
    match status {
        200 | 201 | 204 => Ok("ready"),
        202 => Ok("skeleton"),
        _ => Err(SoundboardViewError::HttpFailure {
            status,
            retry_after_ms,
        }),
    }
}

#[must_use]
pub fn format_soundboard_badge_cap(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

#[must_use]
pub fn redact_soundboard_view_log(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["Bearer ", "token="] {
        while let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let tail = &out[start..];
            if tail.starts_with("[REDACTED]") {
                break;
            }
            let end_rel = tail
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '&')
                .unwrap_or(tail.len());
            out.replace_range(start..start + end_rel, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_entry(id: &str, name: &str, avail: bool, fav: bool) -> SoundboardSoundEntry {
        SoundboardSoundEntry {
            sound_id: id.to_string(),
            name: name.to_string(),
            emoji_id: Some("1024454092543758356".to_string()),
            emoji_name: Some("🔥".to_string()),
            source_guild_id: Some("274093052673392641".to_string()),
            volume: 1.0,
            available: avail,
            favorite: fav,
        }
    }

    fn sample_entries() -> Vec<SoundboardSoundEntry> {
        vec![
            mk_entry("1102650064771485746", "Оу мааай", true, false),
            mk_entry("1327231186044583947", "Позвонишь", false, true),
            mk_entry("1102645616632397904", "Хабар принёс", true, true),
        ]
    }

    #[test]
    fn test_sub_1012c_1_tokens_layout_and_interactive_states() {
        assert_eq!(SOUNDBOARD_GRID_CONTAINER, "Soundboard Grid");
        assert_eq!(SOUNDBOARD_GRID_DIMENSIONS, "320px x 360px");
        assert_eq!(SOUNDBOARD_GRID_WIDTH_PX, 320);
        assert_eq!(SOUNDBOARD_GRID_HEIGHT_PX, 360);
        assert_eq!(
            (SOUNDBOARD_GRID_PADDING_PX, SOUNDBOARD_GRID_SPACING_PX),
            (8, 6)
        );
        assert_eq!(
            (TOKEN_BACKGROUND, TOKEN_SURFACE, TOKEN_ACCENT),
            ("#2b2d31", "#1e1f22", "#5865f2")
        );
        let s0 = SoundTileInteractiveState::resolve(false, false);
        let s1 = SoundTileInteractiveState::resolve(true, false);
        let s2 = SoundTileInteractiveState::resolve(false, true);
        assert_eq!(
            (s0.as_str(), s1.as_str(), s2.as_str()),
            ("tile", "scale-105", "playing sound")
        );
        assert_eq!(
            (s0.as_button_str(), s2.as_button_str()),
            ("sfx tile", "playing sfx")
        );
        assert_eq!(UI7_GUILD_ID, "1014537851418980372");
    }

    #[test]
    fn test_sub_1012c_2_grid_layout_filtering_and_press_animation() {
        let entries = sample_entries();
        let (sid1, sid2) = ("1102645616632397904", "1102650064771485746");
        let mut hotkeys = SoundboardHotkeyRegistry::new();
        let _ = hotkeys.bind_hotkey(sid1, MOD_CTRL | MOD_SHIFT, "1");
        let slider = build_sfx_volume_slider_spec(100, 1.0, 100.0, false);
        let grid = build_soundboard_grid_view(
            &entries,
            "",
            Some(sid2),
            Some(sid1),
            60,
            &hotkeys,
            slider,
            false,
        );
        assert_eq!((grid.width_px, grid.height_px, grid.columns), (320, 360, 3));
        assert_eq!(grid.tiles.len(), 2);
        assert_eq!(grid.tiles[0].sound_id, sid1);
        assert_eq!(grid.tiles[0].hotkey_badge.as_deref(), Some("Ctrl+Shift+1"));
        assert_eq!(grid.tiles[0].state.as_str(), "playing sound");
        assert!(grid.tiles[0].animation.scale < 1.0);
        assert_eq!(grid.tiles[0].animation.border_hex, "#5865f2");
        assert_eq!(grid.tiles[1].state.as_str(), "scale-105");
        assert!((grid.tiles[1].animation.scale - 1.05).abs() < 1e-4);
    }

    #[test]
    fn test_sub_1012c_3_hotkey_registry_and_volume_slider() {
        let entries = sample_entries();
        let (sid1, sid2) = ("1102645616632397904", "1102650064771485746");
        let mut reg = SoundboardHotkeyRegistry::new();
        assert_eq!(
            reg.bind_hotkey(sid1, 0, "1"),
            Err(SoundboardViewError::HotkeyMissingModifier)
        );
        assert!(reg.bind_hotkey(sid1, MOD_CTRL | MOD_ALT, "f5").is_ok());
        assert_eq!(reg.hotkey_for_sound(sid1).as_deref(), Some("Ctrl+Alt+F5"));
        assert!(matches!(
            reg.bind_hotkey(sid2, MOD_CTRL | MOD_ALT, "F5"),
            Err(SoundboardViewError::HotkeyConflict { .. })
        ));
        let trig = reg.resolve_triggered_sound(MOD_CTRL | MOD_ALT, "F5", &entries);
        assert_eq!(trig, Ok(Some(sid1.to_string())));
        assert!(reg.unbind_sound(sid1) && reg.hotkey_for_sound(sid1).is_none());

        let s_active = build_sfx_volume_slider_spec(80, 0.5, 100.0, false);
        assert!((s_active.effective_gain - 0.4).abs() < 1e-5);
        assert!((s_active.track_fill_ratio - 0.4).abs() < 1e-4);
        let s_muted = build_sfx_volume_slider_spec(150, 1.0, 100.0, true);
        assert_eq!(s_muted.effective_gain, 0.0);
    }

    #[test]
    fn test_sub_1012c_4_nine_dimensions_permissions_and_boundaries() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(validate_snowflake("18446744073709551615"), Ok(u64::MAX));
        assert!(is_valid_sound_name("Оу") && !is_valid_sound_name("А"));
        let base_p = PERM_CONNECT | PERM_SPEAK;
        let p_sb = base_p | PERM_USE_SOUNDBOARD;
        let p_full = p_sb | PERM_USE_EXTERNAL_SOUNDS;
        let err_sb = evaluate_soundboard_view_permissions(base_p, false, false, false, false);
        assert_eq!(
            err_sb,
            Err(SoundboardViewError::MissingPermission("USE_SOUNDBOARD"))
        );
        let err_ext = evaluate_soundboard_view_permissions(p_sb, false, true, false, false);
        assert_eq!(err_ext, Err(SoundboardViewError::ExternalSoundsNotAllowed));
        let err_age = evaluate_soundboard_view_permissions(p_sb, false, false, false, true);
        assert_eq!(err_age, Err(SoundboardViewError::AgeGatedRestricted));
        assert!(evaluate_soundboard_view_permissions(p_full, false, true, false, false).is_ok());
        let ok_b = validate_soundboard_view_boundaries(
            2_000,
            10,
            25 * 1024 * 1024,
            9_975,
            200,
            250,
            524_288,
        );
        assert!(ok_b.is_ok());
        assert!(validate_soundboard_view_boundaries(2_001, 10, 1024, 0, 100, 10, 1024).is_err());
        assert_eq!(format_soundboard_badge_cap(150), "99+");
        assert_eq!(classify_soundboard_view_http_status(202, 0), Ok("skeleton"));
        assert!(classify_soundboard_view_http_status(429, 1_200).is_err());
        let red = redact_soundboard_view_log("Authorization: Bearer mfa.secret99 token=xyz123");
        assert!(!red.contains("mfa.secret99") && red.contains("[REDACTED]"));
    }
}
