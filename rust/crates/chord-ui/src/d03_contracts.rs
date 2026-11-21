//! Empirical UI interaction trace contracts and 12 Mana component specifications (Task D-03).
//!
//! Source Oracles:
//! - `docs/re/frontend-d03.md` & `docs/re/frontend-d03.json`
//! - Clean-room UI captures `/home/sanya/re-deep/ui/`, `ui2/`, `ui3/`, `ui6/`, `ui8/`
//!
//! Defines strongly-typed models and invariants for:
//! 1. Channel sidebar (`ChannelSidebarTree`, guild channel tree `link__2ea32`, DM list `privateChannels_e6b769`,
//!    user status panel `panels__5e434`, category collapse keeping unread channels, ARIA `tree`/`treeitem`/`list`).
//! 2. Chat scroll virtualization (`ChatVirtualViewport`, `scrollerBase_d125d2`, `wrapper_c19a55 cozy_c19a55 zalgo_c19a55`,
//!    dynamic row height measurement, bounded DOM pool `<= 60`, sticky date divider, jump-to-bottom unread pill).
//! 3. Emoji picker (`EmojiPickerModel`, full `Ctrl+E` expression picker `expressionPickerPositionLayer__74017`
//!    with 3 tabs and 10 category rail items vs compact reaction picker `emojiPicker_c0e32c`).
//! 4. User popouts & member roster (`UserPopoutModel`, `popoutContainer_ce8328 accountPopout__37e49`,
//!    `role="dialog"`, `aria-modal="true"`, 80x80 avatar with status mask, 105px banner).
//! 5. Settings navigation (`SettingsNavModel`, `container_abd9a8` 1398x961, 63 navigation rows across 19 top
//!    categories, Voice & Video tab with 38 `role="switch"` toggles, 4 comboboxes, 4 `slider__4c059` sliders).
//! 6. 12 Mana Component Contracts (`Button`, `TextInput / TextArea`, `Menu (Mana)`, `ContextMenu`,
//!    `Modal`, `Popover`, `Tooltip`, `Switch`, `Slider`, `Avatar`, `Badge`, `Scroller`).

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::member_list::PresenceStatus;

/// Maximum DOM/virtual node pool size during 100k message scrolling (D-03 chat scroll virtualization).
pub const CHAT_VIRTUALIZER_MAX_DOM_POOL: usize = 60;

/// Canonical ARIA roles used across the guild rail, channel sidebar, and DM list (D-03 §1 / `00-start.fingerprint.json`).
pub const CHANNEL_SIDEBAR_ARIA_ROLES: [&str; 6] =
    ["tree", "treeitem", "group", "list", "listitem", "button"];

/// Canonical 10 category rail items in the full Emoji Picker (`21d-emoji-categories.json`).
pub const EMOJI_CATEGORY_RAIL_ITEMS: [&str; 10] = [
    "top guild emoji",
    "recent",
    "ITSKIDON PC BOOST",
    "people",
    "nature",
    "food",
    "activity",
    "travel",
    "objects",
    "symbols",
];

/// Canonical 19 top-level categories in User Settings (`23-settings-categories.json`, 63 total rows).
pub const SETTINGS_TOP_CATEGORIES: [&str; 19] = [
    "Учётная запись",
    "Данные и конфиденциальность",
    "Разрешения для сообщений",
    "Уведомления",
    "Nitro",
    "Буст сервера",
    "Подписки",
    "Склад подарков",
    "Выставление счетов",
    "Голос и видео",
    "Внешний вид",
    "Специальные возможности",
    "Система",
    "Язык и время",
    "Зарегистрированные игры",
    "Конфиденциальность активности",
    "Подключённые приложения",
    "Разработчик",
    "Выйти",
];

/// Canonical 4 sliders on the "Voice & Video" settings tab (`23-settings-voice.json`, `slider__4c059`).
/// Tuple: `(label, min, max, captured_now)`.
pub const VOICE_SETTINGS_SLIDERS: [(&str, f64, f64, f64); 4] = [
    ("Громкость микрофона", 0.0, 100.0, 100.0),
    ("Громкость динамика", 0.0, 200.0, 100.0),
    ("Сила приглушения стрима", 1.0, 100.0, 50.0),
    (
        "Порог чувствительности ввода",
        0.0,
        100.0,
        26.109_350_171_958_468,
    ),
];

/// Canonical 12 Mana component names in D-03 §5.
pub const D03_COMPONENT_NAMES: [&str; 12] = [
    "Button",
    "TextInput / TextArea",
    "Menu (Mana)",
    "ContextMenu",
    "Modal",
    "Popover",
    "Tooltip",
    "Switch",
    "Slider",
    "Avatar",
    "Badge",
    "Scroller",
];

/// Canonical dark-theme CSS tokens extracted from `ui/tokens.json` (4,795 variables, D-03 §6).
pub const D03_DARK_THEME_TOKENS: [(&str, &str); 16] = [
    ("--modal-width-small", "442px"),
    ("--modal-width-medium", "602px"),
    ("--modal-width-large", "800px"),
    ("--modal-background", "hsl(240 6.494% 15.098%)"),
    ("--modal-footer-background", "hsl(240 6.494% 15.098%)"),
    ("--modal-vertical-padding", "16px"),
    (
        "--switch-background-selected-default",
        "hsl(234.935 85.556% 64.706%)",
    ),
    ("--switch-border-default", "hsl(240 4% 60.784% / 0.2)"),
    ("--switch-thumb-background-default", "hsl(0 0% 100%)"),
    ("--input-background-default", "hsl(0 0% 0% / 0.1216)"),
    ("--input-border-error-default", "hsl(1.905 90% 72.549%)"),
    ("--button-border-radius", "2147483647px"),
    (
        "--button-danger-background-disabled",
        "hsl(355.636 64.706% 50%)",
    ),
    ("--menu-scroller-block-padding", "8px"),
    ("--menu-border-width", "1px"),
    ("--scrollbar-thin-thumb", "hsl(234.545 5.473% 39.412%)"),
];

/// Canonical keyboard bindings captured across D-03 UI interaction traces.
/// Tuple: `(shortcut, module_or_surface, action)`.
pub const D03_KEYBOARD_BINDINGS: [(&str, &str, &str); 10] = [
    (
        "Ctrl+K / Cmd+K",
        "module 675704 (QuickSwitcher)",
        "OPEN_QUICK_SWITCHER",
    ),
    ("Ctrl+F / Cmd+F", "module 696760 (Search)", "FOCUS_SEARCH"),
    (
        "Ctrl+A",
        "editor__25bec (Guild Search)",
        "SELECT_ALL_CLEAR_SCOPE_CHIP",
    ),
    (
        "Ctrl+E",
        "expressionPickerPositionLayer__74017",
        "TOGGLE_EMOJI_PICKER",
    ),
    (
        "Escape",
        "Modal / Popover / Settings / Search",
        "CLOSE_ACTIVE_LAYER",
    ),
    (
        "ArrowUp / ArrowDown",
        "Channel Sidebar / Menu / VirtualList",
        "MOVE_FOCUS_VERTICAL",
    ),
    (
        "ArrowLeft / ArrowRight",
        "Channel Sidebar Category / Slider",
        "COLLAPSE_EXPAND_OR_ADJUST",
    ),
    (
        "Home / End",
        "Channel Sidebar / Chat Scroller",
        "JUMP_FIRST_OR_LAST",
    ),
    (
        "PageUp / PageDown",
        "ChatVirtualViewport / Scroller",
        "SCROLL_VIEWPORT_PAGE",
    ),
    (
        "Enter / Space",
        "Button / Switch / MenuItem",
        "ACTIVATE_FOCUSED_CONTROL",
    ),
];

/// Identifies each of the empirical UI interaction trace surfaces in D-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SurfaceTraceKind {
    /// Channel sidebar (guild channel tree & private DM list + user status panel).
    ChannelSidebar,
    /// Chat scroll virtualization (dynamic row sizing, bounded DOM pool, sticky date divider, jump pill).
    ChatScrollVirtualization,
    /// Emoji picker (full `Ctrl+E` picker & compact context-menu reaction picker).
    EmojiPicker,
    /// User popouts (`role="dialog"`, `popoutContainer_ce8328`) & right sidebar member roster (`membersWrap_c8ffbb`).
    UserPopoutsAndMembers,
    /// Settings navigation (`container_abd9a8` 1398x961, 63 rows, Voice & Video tab controls).
    SettingsNavigation,
    /// Guild search panel (`searchResultsWrap_a98f3b`, 7 filter sections, 9 `has:` options, 3 sort modes).
    GuildSearch,
}

/// Empirical specification of a traced UI surface: DOM hierarchy, CSS tokens, keyboard bindings, and ARIA roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTraceSpec {
    /// Surface identifier.
    pub kind: SurfaceTraceKind,
    /// Root CSS selector / class from the live capture.
    pub root_selector: &'static str,
    /// Ordered DOM hierarchy path from container down to leaf interactive nodes.
    pub dom_hierarchy: &'static [&'static str],
    /// Associated CSS custom properties (design tokens).
    pub css_tokens: &'static [&'static str],
    /// Keyboard bindings active within this surface.
    pub keyboard_bindings: &'static [&'static str],
    /// ARIA roles and attributes enforced on this surface.
    pub aria_roles: &'static [&'static str],
}

/// Static catalog of the 6 empirical UI interaction trace surfaces in D-03.
pub const D03_SURFACE_TRACES: [SurfaceTraceSpec; 6] = [
    SurfaceTraceSpec {
        kind: SurfaceTraceKind::ChannelSidebar,
        root_selector: "nav.privateChannels_e6b769 / nav.wrapper_ef3116.guilds__5e434",
        dom_hierarchy: &[
            "nav.wrapper_ef3116.guilds__5e434 > div.tree_ef3116[role=tree] > div.wrapper__6e9f8[role=treeitem]",
            "nav.privateChannels_e6b769 > div.searchBar_e6b769 > button.button_a22cb0[data-mana-component=button]",
            "div.scroller__99e7c.scrollerBase_d125d2.thin_d125d2 > ul.content_d125d2[role=list]",
            "li.channel__972a0.dm__972a0.container_e45859[role=listitem] > div.interactive__972a0 > a.link__972a0",
            "a.link__2ea32[href^='/channels/{guild_id}/{channel_id}']",
            "div.sidebarResizeHandle__5e434[role=separator] + section.panels__5e434 > div.accountPopoutButton__37e49[role=button]",
        ],
        css_tokens: &[
            "--scrollbar-thin-thumb",
            "--scrollbar-thin-track",
            "--interactive-background-hover",
            "--text-strong",
            "--background-surface-high",
        ],
        keyboard_bindings: &[
            "ArrowUp / ArrowDown (roving tabindex vertical focus)",
            "ArrowLeft / ArrowRight (collapse/expand guild category)",
            "Home / End (first/last visible channel)",
            "Enter / Space (activate channel link or category header)",
            "Ctrl+K / Cmd+K (open QuickSwitcher)",
        ],
        aria_roles: &[
            "role=tree",
            "role=treeitem",
            "role=group",
            "role=list",
            "role=listitem",
            "role=separator",
            "role=switch",
            "aria-posinset / aria-setsize",
            "aria-expanded",
        ],
    },
    SurfaceTraceSpec {
        kind: SurfaceTraceKind::ChatScrollVirtualization,
        root_selector: "div.scrollerBase_d125d2.scrollbarGutterStable_d125d2",
        dom_hierarchy: &[
            "div.scrollerBase_d125d2.scrollbarGutterStable_d125d2 > ol.scrollerInner[role=log]",
            "div.wrapper_c19a55.cozy_c19a55.zalgo_c19a55 > div.contents_c19a55",
            "h3.header_c19a55 > span.username_c19a55 + time.timestamp_c19a55",
            "div.markup__75297.messageContent_c19a55",
            "div.chat-sticky-date-divider + button.chat-jump-bottom-pill",
        ],
        css_tokens: &[
            "--scrollbar-auto-thumb",
            "--scrollbar-auto-track",
            "--text-strong",
            "--background-surface-high",
        ],
        keyboard_bindings: &[
            "PageUp / PageDown (viewport page scroll)",
            "Home / End (jump to oldest loaded / jump to bottom latest)",
            "ArrowUp (edit last own message when composer empty)",
            "Escape (mark channel read / jump to bottom)",
        ],
        aria_roles: &[
            "role=log",
            "role=article",
            "role=separator",
            "aria-live=polite",
            "aria-setsize / aria-posinset",
        ],
    },
    SurfaceTraceSpec {
        kind: SurfaceTraceKind::EmojiPicker,
        root_selector: "div.positionLayer__08434.expressionPickerPositionLayer__74017.layer__59d0d",
        dom_hierarchy: &[
            "div.expressionPickerPositionLayer__74017.layer__59d0d > div.emojiPicker_c0e32c",
            "div[role=tablist] > div[role=tab] (Гифки, Стикеры, Эмодзи[aria-selected=true])",
            "input.input__75098[placeholder='Найдите идеальный эмодзи'] + button[aria-label*='цвет кожи']",
            "div.listItems_affa7e > div.categoryItem_b9ee0c / div.categoryItemGuildCategory_b9ee0c",
            "div.listWrapper_c656ac > div.categorySection_c656ac > div.emojiListRow_fc7141 > button.emojiItem_fc7141 / div.lockedEmoji_d982c2",
        ],
        css_tokens: &[
            "--input-background-default",
            "--scrollbar-thin-thumb",
            "--background-surface-high",
            "--text-strong",
        ],
        keyboard_bindings: &[
            "Ctrl+E (open/toggle full expression picker)",
            "ArrowUp / ArrowDown / ArrowLeft / ArrowRight (2D emoji grid navigation)",
            "Tab / Shift+Tab (cycle tabs, search input, skin tone selector, category rail)",
            "Enter (insert selected emoji or reaction)",
            "Escape (close picker popover and restore focus)",
        ],
        aria_roles: &[
            "role=dialog",
            "role=tablist",
            "role=tab",
            "aria-selected",
            "role=grid",
            "role=row",
            "role=gridcell",
        ],
    },
    SurfaceTraceSpec {
        kind: SurfaceTraceKind::UserPopoutsAndMembers,
        root_selector: "div.popoutContainer_ce8328.accountPopout__37e49 / aside.membersWrap_c8ffbb",
        dom_hierarchy: &[
            "div.popoutContainer_ce8328.accountPopout__37e49[role=dialog][aria-modal=true] > div.outer_c0bea0.user-profile-popout",
            "div.inner_c0bea0 > div.header__5be3e > div.banner_f7e69e (105px) + div.avatar__75742 > div.wrapper__44b0c[role=img] (80x80)",
            "div.body_ce8328.scrollerBase_d125d2.thin_d125d2 > div.userInfo_ce8328 + div.menus_ce8328 > ul > li.menuItem_ce8328",
            "aside.membersWrap_c8ffbb.hiddenMembers_c8ffbb > div.members_c8ffbb.scrollerBase_d125d2 > div.content_d125d2",
            "div.membersGroup_c8ffbb > span.membersGroupHeader_c8ffbb + div.member__5d473.container__91a9d.clickable__91a9d",
        ],
        css_tokens: &[
            "--profile-gradient-primary-color",
            "--profile-gradient-secondary-color",
            "--profile-gradient-button-color",
            "--background-surface-high",
            "--scrollbar-thin-thumb",
        ],
        keyboard_bindings: &[
            "Enter / Space (open user popout from member row or message author)",
            "Tab / Shift+Tab (focus trap inside popout dialog actions and badges)",
            "ArrowUp / ArrowDown (navigate member rows or popout menu items)",
            "Escape (dismiss popout and return focus to trigger element)",
        ],
        aria_roles: &[
            "role=dialog",
            "aria-modal=true",
            "aria-labelledby",
            "role=img",
            "role=group",
            "role=button",
            "role=listitem",
        ],
    },
    SurfaceTraceSpec {
        kind: SurfaceTraceKind::SettingsNavigation,
        root_selector: "div.container_abd9a8 (1398x961)",
        dom_hierarchy: &[
            "div.container_abd9a8 > div[class*=sidebarRegion] > input[placeholder='Поиск']",
            "div.itemContainer_caf372 > div.item_caf372.active_caf372 > div.itemContent_caf372 + div.item_e4d939",
            "div[class*=contentRegion] > div[role=combobox] + div[data-mana-component=BaseRadioGroup][role=radiogroup]",
            "input[type=checkbox][role=switch][data-react-aria-pressable=true] (38 switches)",
            "div.slider__4c059[role=slider][aria-orientation=horizontal][aria-valuemin][aria-valuemax][aria-valuenow]",
        ],
        css_tokens: &[
            "--switch-background-selected-default",
            "--switch-border-default",
            "--switch-thumb-background-default",
            "--slider-track-background",
            "--button-border-radius",
            "--input-background-default",
        ],
        keyboard_bindings: &[
            "ArrowUp / ArrowDown (navigate 63 settings sidebar items)",
            "ArrowLeft / ArrowRight (adjust slider value in role=slider)",
            "Space / Enter (toggle role=switch or select radio option)",
            "Escape (close settings overlay and return to chat)",
        ],
        aria_roles: &[
            "role=tab",
            "role=combobox",
            "role=radiogroup",
            "role=radio",
            "role=switch",
            "role=slider",
            "aria-valuemin / aria-valuemax / aria-valuenow",
        ],
    },
    SurfaceTraceSpec {
        kind: SurfaceTraceKind::GuildSearch,
        root_selector: "div.searchResultsWrap_a98f3b",
        dom_hierarchy: &[
            "div.editor__25bec[contenteditable=true] + div.searchResultsWrap_a98f3b",
            "header.searchHeader_ae7890 > div.totalResults_ae7890 + div.searchModeAndFiltersContainer_ae7890",
            "div.filterSection_e4772e > input.input__75098.comboBoxInput__0edde",
            "div.menu_c1e9c4.flexible_c1e9c4 (sort options: По новизне, Самое старое, Самое актуальное)",
            "div.scroller_a98f3b.scrollerBase_d125d2 > div.searchResultGroup_a7e67f > div.container__80bf8 > div.searchResult__80bf8 > div.wrapper_c19a55",
        ],
        css_tokens: &[
            "--input-background-default",
            "--button-border-radius",
            "--menu-scroller-block-padding",
            "--menu-border-width",
            "--scrollbar-thin-thumb",
        ],
        keyboard_bindings: &[
            "Ctrl+F / Cmd+F (focus search editor__25bec)",
            "Ctrl+A (select all and clear active channel scope chip)",
            "Enter (execute search query)",
            "Escape (clear or close search results panel)",
        ],
        aria_roles: &[
            "role=searchbox",
            "role=menu",
            "role=menuitemradio",
            "aria-checked",
            "role=group",
        ],
    },
];

/// Returns the [`SurfaceTraceSpec`] for a given [`SurfaceTraceKind`].
#[must_use]
pub fn d03_surface_trace(kind: SurfaceTraceKind) -> &'static SurfaceTraceSpec {
    D03_SURFACE_TRACES
        .iter()
        .find(|s| s.kind == kind)
        .expect("all SurfaceTraceKind variants are registered in D03_SURFACE_TRACES")
}

/// Complete specification of one of the 12 Mana components from D-03 §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentContractSpec {
    /// Canonical component name (`Button`, `Modal`, `Scroller`, etc.).
    pub name: &'static str,
    /// Webpack module IDs and asset bundle provenance from `docs/re/frontend-d03.md` §5.
    pub module_id: &'static str,
    /// Empirical DOM class names and `data-mana-component` selectors.
    pub dom_classes: &'static [&'static str],
    /// Supported visual/structural variants.
    pub variants: &'static [&'static str],
    /// Supported interactive and lifecycle states.
    pub states: &'static [&'static str],
    /// CSS custom property tokens governing the component.
    pub css_tokens: &'static [&'static str],
    /// ARIA roles and accessibility attributes.
    pub aria_roles: &'static [&'static str],
    /// Keyboard interaction bindings.
    pub keyboard_bindings: &'static [&'static str],
}

/// Static table of all 12 Mana component contracts from D-03 §5.
pub const D03_COMPONENTS: [ComponentContractSpec; 12] = [
    ComponentContractSpec {
        name: "Button",
        module_id: "module 657718 @ discord.com_assets_web.4d4b2f75fdea5470.js / CSS 65406",
        dom_classes: &[
            "button_a22cb0",
            "xs_a22cb0",
            "sm_a22cb0",
            "md_a22cb0",
            "primary_a22cb0",
            "secondary_a22cb0",
            "hasText_a22cb0",
            "buttonChildrenWrapper_a22cb0",
            "data-mana-component=\"button\"",
        ],
        variants: &[
            "primary",
            "secondary",
            "critical-primary",
            "critical-secondary",
            "overlay-primary",
            "overlay-secondary",
            "icon-only",
            "input-accessory",
            "active",
            "expressive",
            "text-button",
            "toggle-icon-button",
        ],
        states: &[
            "hover/active",
            "disabled",
            "loading (aria-busy, spinner fadeIn/fadeOut)",
            "hasText",
            "fullWidth",
            "rounded",
            "minWidth (--__button-min-width)",
        ],
        css_tokens: &[
            "--button-border-radius",
            "--button-border-radius-lg",
            "--button-danger-background-disabled",
            "--button-outline-brand-border-active",
            "--button-outline-primary-text",
        ],
        aria_roles: &[
            "role=\"button\"",
            "aria-busy",
            "aria-disabled",
            "aria-label",
        ],
        keyboard_bindings: &["Enter", "Space", "Tab / Shift+Tab"],
    },
    ComponentContractSpec {
        name: "TextInput / TextArea",
        module_id: "module 103557 @ discord.com_assets_05bfdf7077e1fc11.js:1161 / CSS 358983",
        dom_classes: &[
            "input__75098",
            "comboBoxInput__0edde",
            "data-mana-component=\"text-area\"",
        ],
        variants: &[
            "single-line",
            "textarea",
            "hasTags",
            "hasLeading / leadingText",
            "hasTrailing",
            "combo (comboBoxInput__0edde)",
        ],
        states: &[
            "disabled",
            "invalid/error",
            "placeholder",
            "focus",
            "hasTags",
        ],
        css_tokens: &[
            "--input-background-default",
            "--input-icon-default",
            "--input-placeholder-text-default",
            "--input-border-error-default",
            "--input-text-error-default",
        ],
        aria_roles: &[
            "role=\"textbox\"",
            "role=\"combobox\"",
            "aria-invalid",
            "aria-disabled",
        ],
        keyboard_bindings: &["Ctrl+A", "Backspace (delete tag chip)", "Enter", "Escape"],
    },
    ComponentContractSpec {
        name: "Menu (Mana)",
        module_id: "module 798819 @ discord.com_assets_261000.1b6532d3649c0f52.js:6116 / module 980707",
        dom_classes: &[
            "menu_c1e9c4",
            "flexible_c1e9c4",
            "scroller_c1e9c4",
            "separator_c1e9c4",
        ],
        variants: &[
            "colorDefault",
            "colorDanger",
            "checkbox",
            "radioSelection",
            "submenu",
            "separator",
        ],
        states: &[
            "enabled/disabled",
            "selected (aria-checked)",
            "focused",
            "scrolling",
        ],
        css_tokens: &["--menu-scroller-block-padding", "--menu-border-width"],
        aria_roles: &[
            "role=\"menu\"",
            "role=\"menuitem\"",
            "role=\"menuitemcheckbox\"",
            "role=\"menuitemradio\"",
            "aria-checked",
        ],
        keyboard_bindings: &[
            "ArrowUp / ArrowDown",
            "ArrowRight / ArrowLeft (submenu)",
            "Enter / Space",
            "Escape",
        ],
    },
    ComponentContractSpec {
        name: "ContextMenu",
        module_id: "opener module 837528 / primitives module 442433 / keybind registry module 696760",
        dom_classes: &["menu_c1e9c4", "layer__59d0d", "message-add-reaction"],
        variants: &["message-* items", "submenu", "checkbox/radio items"],
        states: &[
            "open/close (CONTEXT_MENU_OPEN/CLOSE)",
            "focused item",
            "disabled item",
        ],
        css_tokens: &["--menu-scroller-block-padding", "--menu-border-width"],
        aria_roles: &[
            "role=\"menu\"",
            "role=\"menuitem\"",
            "aria-haspopup=\"menu\"",
            "aria-expanded",
        ],
        keyboard_bindings: &[
            "Shift+F10 / ContextMenu",
            "ArrowUp / ArrowDown",
            "Enter",
            "Escape",
        ],
    },
    ComponentContractSpec {
        name: "Modal",
        module_id: "module 521489 @ discord.com_assets_e6c31aeaf72a699a.js / focus-trap module 617986",
        dom_classes: &[
            "data-mana-component=\"layer-modal\"",
            "data-mana-component=\"modal\"",
            "container_abd9a8",
        ],
        variants: &[
            "animationVariant default",
            "layer-modal",
            "small (442px)",
            "medium (602px)",
            "large (800px)",
        ],
        states: &[
            "transitionState (enter/exit)",
            "focus-trap",
            "onClose",
            "aria-label/aria-labelledby",
        ],
        css_tokens: &[
            "--modal-background",
            "--modal-footer-background",
            "--modal-vertical-padding",
            "--modal-width-small",
            "--modal-width-medium",
            "--modal-width-large",
        ],
        aria_roles: &[
            "role=\"dialog\"",
            "aria-modal=\"true\"",
            "aria-labelledby",
            "tabindex=\"-1\"",
        ],
        keyboard_bindings: &["Escape (onClose)", "Tab / Shift+Tab (focus trap cycle)"],
    },
    ComponentContractSpec {
        name: "Popover",
        module_id: "module 179657 @ discord.com_assets_267e963ebabea6ad.js:9316 / module 43105 / module 273875",
        dom_classes: &[
            "data-mana-component=\"popover\"",
            "layer__59d0d",
            "positionLayer__08434",
            "expressionPickerPositionLayer__74017",
        ],
        variants: &[
            "standard/animated",
            "with offset-border",
            "menu/status/shortcut popover",
        ],
        states: &["open/close", "positioned"],
        css_tokens: &["--background-surface-high", "--text-strong"],
        aria_roles: &["role=\"dialog\"", "aria-expanded", "aria-haspopup"],
        keyboard_bindings: &["Escape (dismiss layer)", "Tab / Shift+Tab"],
    },
    ComponentContractSpec {
        name: "Tooltip",
        module_id: "module 233680 @ discord.com_assets_348181.c6a72b8c6216ab6c.js:657 / module 461536",
        dom_classes: &[
            "tooltip_c36707",
            "tooltipPointer_c36707",
            "tooltipDisablePointerEvents_c36707",
            "richTooltipContent__9038b",
            "data-mana-component=\"mini-premium-tooltip\"",
        ],
        variants: &["standard", "mini-premium-tooltip", "richTooltipContent"],
        states: &["top/bottom/center", "disablePointerEvents", "allowOverflow"],
        css_tokens: &["--text-strong", "--background-surface-high"],
        aria_roles: &["role=\"tooltip\"", "aria-describedby"],
        keyboard_bindings: &["Focus / Blur on trigger element", "Escape (hide tooltip)"],
    },
    ComponentContractSpec {
        name: "Switch",
        module_id: "module 35556 @ discord.com_assets_501428.98d57a9bd8cbb604.js:1181154 / module 479909",
        dom_classes: &[
            "data-react-aria-pressable=\"true\"",
            "input[type=\"checkbox\"][role=\"switch\"]",
        ],
        variants: &["standalone switch", "radio-like toggle"],
        states: &[
            "checked/unchecked",
            "disabled",
            "focus (data-react-aria-pressable)",
        ],
        css_tokens: &[
            "--switch-background-selected-default",
            "--switch-border-default",
            "--switch-border-selected-hover",
            "--switch-thumb-background-default",
            "--switch-thumb-icon-active",
        ],
        aria_roles: &[
            "role=\"switch\"",
            "aria-checked",
            "aria-labelledby",
            "aria-describedby",
        ],
        keyboard_bindings: &["Space (toggle switch)", "Enter", "Tab / Shift+Tab"],
    },
    ComponentContractSpec {
        name: "Slider",
        module_id: "module 9761 @ discord.com_assets_500186.e89df1f0566f54ee.js:130",
        dom_classes: &["slider__4c059"],
        variants: &["standard", "mini", "hasMarks"],
        states: &["disabled", "defaultValue", "aria-disabled/invalid"],
        css_tokens: &["--slider-track-background"],
        aria_roles: &[
            "role=\"slider\"",
            "aria-valuemin",
            "aria-valuemax",
            "aria-valuenow",
            "aria-orientation=\"horizontal\"",
            "aria-labelledby",
        ],
        keyboard_bindings: &[
            "ArrowLeft / ArrowDown (decrement)",
            "ArrowRight / ArrowUp (increment)",
            "Home / End (min / max)",
        ],
    },
    ComponentContractSpec {
        name: "Avatar",
        module_id: "module 311582 @ discord.com_assets_113561.4511bb7966772f25.js:259 / module 818888",
        dom_classes: &[
            "avatar_fdd9da",
            "grayscaleAvatar_fdd9da",
            "avatarWrapper_d8a370",
            "ring_d8a370",
            "wrapper__44b0c",
            "avatar__44b0c",
            "avatarStack__44b0c",
        ],
        variants: &[
            "avatar_fdd9da",
            "grayscaleAvatar_fdd9da",
            "avatarWrapper_d8a370 + ring_d8a370",
            "avatarStack__44b0c (stack)",
        ],
        states: &[
            "status (online/idle/dnd/offline mask)",
            "muted (grayscale)",
            "speaking ring",
        ],
        css_tokens: &["--background-surface-high", "--text-strong"],
        aria_roles: &[
            "role=\"img\"",
            "aria-label=\"username, status\"",
            "aria-hidden",
        ],
        keyboard_bindings: &["Enter / Space (when wrapped in focusTarget__54e4b role=button)"],
    },
    ComponentContractSpec {
        name: "Badge",
        module_id: "module 306091 @ discord.com_assets_880248.92a937f61f4f886c.js:299 / module 638855",
        dom_classes: &[
            "numberBadge__463b7",
            "base__463b7",
            "iconBadge__463b7",
            "premiumBadge__463b7",
            "badge_e7c728",
            "numberBadge_c7a32c",
            "numberBadge__2ea32",
        ],
        variants: &[
            "numberBadge",
            "iconBadge",
            "premiumBadge",
            "clan/server tag badge (badge_e7c728)",
            "baseShapeRound*",
        ],
        states: &["epic", "mythic", "rare", "expressive"],
        css_tokens: &[
            "--badge-secondary-background",
            "--badge-expressive-text",
            "--badge-epic-border-gradient-end",
            "--badge-mythic-background",
        ],
        aria_roles: &["role=\"status\"", "role=\"group\"", "aria-label"],
        keyboard_bindings: &["Tab (when interactive anchor__14d82 badge link)"],
    },
    ComponentContractSpec {
        name: "Scroller",
        module_id: "module 6095 @ discord.com_assets_193433.213ada0bddfee7ee.js",
        dom_classes: &[
            "scrollerBase_d125d2",
            "thin_d125d2",
            "auto_d125d2",
            "none_d125d2",
            "scrollbarGutterStable_d125d2",
            "fade_d125d2",
            "content_d125d2",
            "scroller_affa7e",
            "listItems_affa7e",
            "scroller_c1e9c4",
        ],
        variants: &[
            "auto",
            "none",
            "thin",
            "customTheme",
            "virtualized list (scroller_affa7e + listItems_affa7e)",
        ],
        states: &[
            "scrolling",
            "fade",
            "scrollbarGutterStable",
            "disableScrollAnchor",
        ],
        css_tokens: &[
            "--scrollbar-thin-thumb",
            "--scrollbar-thin-track",
            "--scrollbar-auto-thumb",
            "--scrollbar-auto-track",
        ],
        aria_roles: &[
            "aria-orientation=\"vertical\"",
            "tabindex=\"0\"",
            "data-list-id",
            "data-jump-section",
        ],
        keyboard_bindings: &["PageUp / PageDown", "Home / End", "ArrowUp / ArrowDown"],
    },
];

/// Returns all 12 Mana component contracts from D-03 §5.
#[must_use]
pub fn d03_component_contracts() -> &'static [ComponentContractSpec; 12] {
    &D03_COMPONENTS
}

/// Looks up one of the 12 Mana component contracts by exact component name.
#[must_use]
pub fn component_contract_by_name(name: &str) -> Option<&'static ComponentContractSpec> {
    D03_COMPONENTS.iter().find(|c| c.name == name)
}

// ---------------------------------------------------------------------------
// 1. Channel Sidebar Contract (`link__2ea32`, `privateChannels_e6b769`)
// ---------------------------------------------------------------------------

/// Channel type classification for sidebar sorting and rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelNodeKind {
    /// Text channel (`GUILD_TEXT = 0`).
    GuildText,
    /// Direct message (`DM = 1`).
    DirectMessage,
    /// Voice channel (`GUILD_VOICE = 2`).
    GuildVoice,
    /// Group DM (`GROUP_DM = 3`).
    GroupDm,
    /// Announcement channel (`GUILD_ANNOUNCEMENT = 5`).
    GuildAnnouncement,
    /// Stage voice channel (`GUILD_STAGE_VOICE = 13`).
    GuildStageVoice,
    /// Forum channel (`GUILD_FORUM = 15`).
    GuildForum,
}

impl ChannelNodeKind {
    /// Returns true if the channel is voice-like (`GuildVoice` or `GuildStageVoice`),
    /// which always sorts after text/announcement/forum channels within a category.
    #[must_use]
    pub fn is_voice_like(self) -> bool {
        matches!(self, Self::GuildVoice | Self::GuildStageVoice)
    }

    /// Russian ARIA suffix matching live capture `01-guild-channels.json`.
    #[must_use]
    pub fn aria_kind_label(self) -> &'static str {
        match self {
            Self::GuildText | Self::GuildAnnouncement => "текстовый канал",
            Self::GuildVoice | Self::GuildStageVoice => "голосовой канал",
            Self::GuildForum => "канал форума",
            Self::DirectMessage | Self::GroupDm => "личное сообщение",
        }
    }
}

/// A channel node inside the guild channel sidebar or DM list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSidebarItem {
    /// Channel snowflake ID.
    pub id: u64,
    /// Optional parent category snowflake ID.
    pub parent_id: Option<u64>,
    /// Channel display name.
    pub name: String,
    /// Channel kind.
    pub kind: ChannelNodeKind,
    /// Position index within its parent category.
    pub position: i32,
    /// Whether the channel has unread messages.
    pub unread: bool,
    /// Number of unread mentions (`numberBadge__2ea32`).
    pub mentions: u32,
}

impl ChannelSidebarItem {
    /// Formats the empirical ARIA label from `01-guild-channels.json`
    /// (e.g., `"не прочитано, 🔤болталка (текстовый канал)"`).
    #[must_use]
    pub fn aria_label(&self) -> String {
        let prefix = if self.unread || self.mentions > 0 {
            "не прочитано, "
        } else {
            ""
        };
        format!("{prefix}{} ({})", self.name, self.kind.aria_kind_label())
    }
}

/// Category definition in the guild channel sidebar (`GUILD_CATEGORY = 4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelCategoryNode {
    /// Category snowflake ID.
    pub id: u64,
    /// Category display title.
    pub name: String,
    /// Category position.
    pub position: i32,
    /// Whether the category is collapsed by the user.
    pub collapsed: bool,
}

/// Computed category group with full children and collapse-filtered visible children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltChannelCategory {
    /// Category metadata.
    pub category: ChannelCategoryNode,
    /// All child channels sorted by `(is_voice_like, position, id)`.
    pub all_children: Vec<ChannelSidebarItem>,
    /// Visible child channels (when collapsed, only channels with `unread || mentions > 0` remain visible).
    pub visible_children: Vec<ChannelSidebarItem>,
}

/// Built channel sidebar tree containing uncategorized top channels and ordered categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSidebarTree {
    /// Uncategorized channels (or channels whose parent category is missing), never hidden by category collapse.
    pub uncategorized: Vec<ChannelSidebarItem>,
    /// Ordered categories with their child lists.
    pub categories: Vec<BuiltChannelCategory>,
}

fn sort_channel_slice(items: &mut [ChannelSidebarItem]) {
    items.sort_by(|a, b| {
        a.kind
            .is_voice_like()
            .cmp(&b.kind.is_voice_like())
            .then_with(|| a.position.cmp(&b.position))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Builds the deterministic guild channel sidebar tree matching Discord's ordering and collapse contract.
#[must_use]
pub fn build_channel_sidebar(
    categories: &[ChannelCategoryNode],
    channels: &[ChannelSidebarItem],
) -> ChannelSidebarTree {
    let mut sorted_cats = categories.to_vec();
    sorted_cats.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));

    let mut uncategorized = Vec::new();
    let mut by_cat: Vec<Vec<ChannelSidebarItem>> = vec![Vec::new(); sorted_cats.len()];

    for ch in channels {
        if let Some(pid) = ch.parent_id
            && let Some(cat_idx) = sorted_cats.iter().position(|c| c.id == pid)
        {
            by_cat[cat_idx].push(ch.clone());
            continue;
        }
        uncategorized.push(ch.clone());
    }

    sort_channel_slice(&mut uncategorized);

    let built_categories = sorted_cats
        .into_iter()
        .zip(by_cat)
        .map(|(cat, mut children)| {
            sort_channel_slice(&mut children);
            let visible_children = if cat.collapsed {
                children
                    .iter()
                    .filter(|c| c.unread || c.mentions > 0)
                    .cloned()
                    .collect()
            } else {
                children.clone()
            };
            BuiltChannelCategory {
                category: cat,
                all_children: children,
                visible_children,
            }
        })
        .collect();

    ChannelSidebarTree {
        uncategorized,
        categories: built_categories,
    }
}

// ---------------------------------------------------------------------------
// 2. Chat Scroll Virtualization Contract (`scrollerBase_d125d2`, `wrapper_c19a55`)
// ---------------------------------------------------------------------------

/// Virtualized chat scroll viewport supporting dynamic row height measurements,
/// bounded DOM pool (`<= 60` rows), sticky date divider state, and jump-to-bottom unread pill.
#[derive(Debug, Clone)]
pub struct ChatVirtualViewport {
    total_rows: usize,
    default_row_height: u32,
    measured_overrides: std::collections::BTreeMap<usize, u32>,
    viewport_height: u32,
    overscan: usize,
    scroll_top: u64,
    unread_pill_count: u32,
}

impl ChatVirtualViewport {
    /// Creates a new virtualized chat viewport positioned at the bottom of the message stream.
    pub fn new(
        total_rows: usize,
        default_row_height: u32,
        viewport_height: u32,
        overscan: usize,
    ) -> Result<Self, String> {
        if default_row_height == 0 {
            return Err("default_row_height must be > 0".to_string());
        }
        if viewport_height == 0 {
            return Err("viewport_height must be > 0".to_string());
        }
        let mut vp = Self {
            total_rows,
            default_row_height,
            measured_overrides: std::collections::BTreeMap::new(),
            viewport_height,
            overscan,
            scroll_top: 0,
            unread_pill_count: 0,
        };
        vp.jump_to_bottom();
        Ok(vp)
    }

    /// Returns the height of row `index` (measured override or default).
    #[must_use]
    pub fn row_height(&self, index: usize) -> u32 {
        self.measured_overrides
            .get(&index)
            .copied()
            .unwrap_or(self.default_row_height)
    }

    /// Dynamically measures and updates the height of row `index`.
    pub fn measure_row(&mut self, index: usize, height: u32) -> Result<(), String> {
        if index >= self.total_rows {
            return Err(format!(
                "row index {index} out of bounds (total {})",
                self.total_rows
            ));
        }
        if height == 0 {
            return Err("measured row height must be > 0".to_string());
        }
        self.measured_overrides.insert(index, height);
        Ok(())
    }

    /// Computes the total scrollable height in pixels across all rows.
    #[must_use]
    pub fn total_height(&self) -> u64 {
        let base = (self.total_rows as u64) * (self.default_row_height as u64);
        let mut delta: i64 = 0;
        for (&idx, &h) in &self.measured_overrides {
            if idx < self.total_rows {
                delta += (h as i64) - (self.default_row_height as i64);
            }
        }
        (base as i64 + delta).max(0) as u64
    }

    /// Computes the pixel top offset of `index`.
    #[must_use]
    pub fn row_offset(&self, index: usize) -> u64 {
        let clamped = index.min(self.total_rows);
        let base = (clamped as u64) * (self.default_row_height as u64);
        let mut delta: i64 = 0;
        for (&idx, &h) in &self.measured_overrides {
            if idx < clamped {
                delta += (h as i64) - (self.default_row_height as i64);
            }
        }
        (base as i64 + delta).max(0) as u64
    }

    /// Maximum valid `scroll_top`.
    #[must_use]
    pub fn max_scroll_top(&self) -> u64 {
        self.total_height()
            .saturating_sub(self.viewport_height as u64)
    }

    /// Returns true when the viewport is anchored at the bottom of the chat stream.
    #[must_use]
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_top + (self.default_row_height as u64) >= self.max_scroll_top()
    }

    /// Returns true when the `.chat-jump-bottom-pill` unread/jump button should be shown.
    #[must_use]
    pub fn jump_pill_visible(&self) -> bool {
        !self.is_at_bottom()
    }

    /// Scrolls to center or show `index` in the viewport.
    pub fn scroll_to_index(&mut self, index: usize) {
        let target = index.min(self.total_rows.saturating_sub(1));
        let offset = self.row_offset(target);
        self.scroll_top = offset.min(self.max_scroll_top());
    }

    /// Scrolls immediately to the bottom and clears the unread pill counter.
    pub fn jump_to_bottom(&mut self) {
        self.scroll_top = self.max_scroll_top();
        self.unread_pill_count = 0;
    }

    /// Increments the unread message counter shown in the jump-to-bottom pill while scrolled up.
    pub fn record_unread(&mut self, count: u32) {
        if !self.is_at_bottom() {
            self.unread_pill_count = self.unread_pill_count.saturating_add(count);
        }
    }

    /// Current unread count displayed on `.chat-jump-bottom-pill`.
    #[must_use]
    pub fn unread_pill_count(&self) -> u32 {
        self.unread_pill_count
    }

    /// Computes the bounded range of row indices rendered in the DOM pool (`<= CHAT_VIRTUALIZER_MAX_DOM_POOL`).
    #[must_use]
    pub fn rendered_range(&self) -> Range<usize> {
        if self.total_rows == 0 {
            return 0..0;
        }
        let approx_first = (self.scroll_top / (self.default_row_height as u64)) as usize;
        let visible_rows = ((self.viewport_height / self.default_row_height) as usize) + 1;
        let start = approx_first
            .saturating_sub(self.overscan)
            .min(self.total_rows);
        let max_count = (visible_rows + self.overscan * 2).min(CHAT_VIRTUALIZER_MAX_DOM_POOL);
        let end = (start + max_count).min(self.total_rows);
        start..end
    }
}

// ---------------------------------------------------------------------------
// 3. Emoji Picker Contract (`expressionPickerPositionLayer__74017`, `emojiPicker_c0e32c`)
// ---------------------------------------------------------------------------

/// Operating mode of the Emoji / Expression Picker from D-03 §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmojiPickerMode {
    /// Full expression picker opened in a guild text channel (`Ctrl+E`, 50 visible, 17 locked).
    FullGuildChannel,
    /// Full expression picker opened in a Direct Message (`Ctrl+E`, 42 visible).
    FullDirectMessage,
    /// Compact reaction picker opened from message context menu `message-add-reaction` (60 visible, 43 locked, no tabs/search).
    CompactReaction,
}

/// Headless contract model for the Discord Emoji Picker (D-03 §2.1–§2.3).
#[derive(Debug, Clone)]
pub struct EmojiPickerModel {
    mode: EmojiPickerMode,
    active_tab: &'static str,
    search_query: String,
}

impl EmojiPickerModel {
    /// Creates an emoji picker model for the given [`EmojiPickerMode`].
    #[must_use]
    pub fn new(mode: EmojiPickerMode) -> Self {
        Self {
            mode,
            active_tab: "Эмодзи",
            search_query: String::new(),
        }
    }

    /// Available `[role="tab"]` tabs (empty in `CompactReaction` mode).
    #[must_use]
    pub fn tabs(&self) -> &'static [&'static str] {
        match self.mode {
            EmojiPickerMode::FullGuildChannel | EmojiPickerMode::FullDirectMessage => {
                &["Гифки", "Стикеры", "Эмодзи"]
            }
            EmojiPickerMode::CompactReaction => &[],
        }
    }

    /// Active tab name (`aria-selected="true"`).
    #[must_use]
    pub fn active_tab(&self) -> &'static str {
        self.active_tab
    }

    /// Whether the search input (`Найдите идеальный эмодзи`) is rendered.
    #[must_use]
    pub fn has_search_input(&self) -> bool {
        !matches!(self.mode, EmojiPickerMode::CompactReaction)
    }

    /// Category navigation rail items (`listItems_affa7e`).
    #[must_use]
    pub fn category_rail(&self) -> &'static [&'static str] {
        &EMOJI_CATEGORY_RAIL_ITEMS
    }

    /// Empirical visible `emojiItem_fc7141` count in the initial viewport grid.
    #[must_use]
    pub fn visible_grid_count(&self) -> usize {
        match self.mode {
            EmojiPickerMode::FullGuildChannel => 50,
            EmojiPickerMode::FullDirectMessage => 42,
            EmojiPickerMode::CompactReaction => 60,
        }
    }

    /// Empirical `lockedEmoji_d982c2` (Nitro-gated) count in the initial viewport grid.
    #[must_use]
    pub fn locked_nitro_count(&self) -> usize {
        match self.mode {
            EmojiPickerMode::FullGuildChannel => 17,
            EmojiPickerMode::FullDirectMessage => 0,
            EmojiPickerMode::CompactReaction => 43,
        }
    }

    /// Updates the emoji search query (`:fire`, etc.).
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.trim().to_string();
    }

    /// Empirical filtered item count matching D-03 §2.1 & §2.2 (`:fire` -> 10 in guild channel, 11 in DM).
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        if self.search_query.is_empty() {
            return self.visible_grid_count();
        }
        if self.search_query.eq_ignore_ascii_case(":fire")
            || self.search_query.eq_ignore_ascii_case("fire")
        {
            return match self.mode {
                EmojiPickerMode::FullGuildChannel => 10,
                EmojiPickerMode::FullDirectMessage => 11,
                EmojiPickerMode::CompactReaction => 0,
            };
        }
        0
    }
}

// ---------------------------------------------------------------------------
// 4. User Popouts Contract (`popoutContainer_ce8328 accountPopout__37e49`)
// ---------------------------------------------------------------------------

/// Empirical contract model for the User / Account Popout (`06-profile-popout.html`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPopoutModel {
    user_id: u64,
    display_name: String,
    username: String,
    status: PresenceStatus,
    badges: Vec<String>,
}

impl UserPopoutModel {
    /// Creates a validated User Popout model. Rejects `user_id == 0` or empty `username`.
    pub fn new(
        user_id: u64,
        display_name: &str,
        username: &str,
        status: PresenceStatus,
        badges: Vec<String>,
    ) -> Result<Self, String> {
        if user_id == 0 {
            return Err("user_id must be a non-zero Snowflake".to_string());
        }
        if username.trim().is_empty() {
            return Err("username must not be empty".to_string());
        }
        Ok(Self {
            user_id,
            display_name: display_name.to_string(),
            username: username.to_string(),
            status,
            badges,
        })
    }

    /// User snowflake ID.
    #[must_use]
    pub fn user_id(&self) -> u64 {
        self.user_id
    }

    /// Display name (`displayName__26b1f heading-lg/bold_cf4812`).
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Global username tag (`userTagUsername__26b1f`).
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Current presence status.
    #[must_use]
    pub fn status(&self) -> PresenceStatus {
        self.status
    }

    /// Root DOM class from `06-profile-popout.html`.
    #[must_use]
    pub fn root_class(&self) -> &'static str {
        "popoutContainer_ce8328 accountPopout__37e49"
    }

    /// ARIA role (`role="dialog"`).
    #[must_use]
    pub fn aria_role(&self) -> &'static str {
        "dialog"
    }

    /// Whether `aria-modal="true"` is set on the popout container.
    #[must_use]
    pub fn aria_modal(&self) -> bool {
        true
    }

    /// Banner height in pixels (`banner_f7e69e`, `height: 105px`).
    #[must_use]
    pub fn banner_height_px(&self) -> u32 {
        105
    }

    /// Avatar wrapper dimensions in pixels (`wrapper__44b0c`, `80x80` inside `92x92` SVG status mask).
    #[must_use]
    pub fn avatar_size_px(&self) -> u32 {
        80
    }

    /// Profile badges (`container__8061a[role="group"][aria-label="Значки пользователя"]`).
    #[must_use]
    pub fn badges(&self) -> &[String] {
        &self.badges
    }
}

// ---------------------------------------------------------------------------
// 5. Settings Navigation Contract (`container_abd9a8`, `23-settings-*.json`)
// ---------------------------------------------------------------------------

/// Empirical contract model for User Settings navigation (`container_abd9a8`, 1398x961, 63 rows, 19 top categories).
#[derive(Debug, Clone)]
pub struct SettingsNavModel {
    active_category: &'static str,
    search_filter: String,
}

impl Default for SettingsNavModel {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsNavModel {
    /// Creates a new User Settings navigation model defaulting to `"Учётная запись"`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_category: SETTINGS_TOP_CATEGORIES[0],
            search_filter: String::new(),
        }
    }

    /// Root container class (`container_abd9a8`).
    #[must_use]
    pub fn root_class(&self) -> &'static str {
        "container_abd9a8"
    }

    /// Empirical root dimensions in pixels (`1398x961`).
    #[must_use]
    pub fn dimensions_px(&self) -> (u32, u32) {
        (1398, 961)
    }

    /// Total navigation item rows captured in `23-settings-categories.json` (`63`).
    #[must_use]
    pub fn total_nav_rows(&self) -> usize {
        63
    }

    /// All 19 top-level navigation categories.
    #[must_use]
    pub fn top_categories(&self) -> &'static [&'static str] {
        &SETTINGS_TOP_CATEGORIES
    }

    /// Currently active category.
    #[must_use]
    pub fn active_category(&self) -> &'static str {
        self.active_category
    }

    /// Selects a category from `SETTINGS_TOP_CATEGORIES`.
    pub fn select_category(&mut self, category: &str) -> Result<(), String> {
        let Some(&found) = SETTINGS_TOP_CATEGORIES.iter().find(|&&c| c == category) else {
            return Err(format!("unknown settings category: {category}"));
        };
        self.active_category = found;
        Ok(())
    }

    /// Updates the sidebar search filter (`input[placeholder="Поиск"]`).
    pub fn set_search_filter(&mut self, query: &str) {
        self.search_filter = query.trim().to_lowercase();
    }

    /// Returns categories matching the current search filter.
    #[must_use]
    pub fn filtered_categories(&self) -> Vec<&'static str> {
        if self.search_filter.is_empty() {
            return SETTINGS_TOP_CATEGORIES.to_vec();
        }
        SETTINGS_TOP_CATEGORIES
            .iter()
            .copied()
            .filter(|c| c.to_lowercase().contains(&self.search_filter))
            .collect()
    }

    /// Empirical count of `role="switch"` checkboxes on the "Voice & Video" tab (`38`).
    #[must_use]
    pub fn voice_tab_switch_count(&self) -> usize {
        38
    }

    /// Empirical count of `role="combobox"` selects on the "Voice & Video" tab (`4`: Mic, Speaker, Camera, Krisp).
    #[must_use]
    pub fn voice_tab_select_count(&self) -> usize {
        4
    }

    /// Options inside the Input Profile `BaseRadioGroup` (`role="radiogroup"`).
    #[must_use]
    pub fn voice_tab_radio_options(&self) -> &'static [&'static str] {
        &["Изоляция голоса", "Студия", "Пользовательский"]
    }
}

/// Validates a value against one of the 4 empirical Voice & Video sliders (`VOICE_SETTINGS_SLIDERS`).
pub fn validate_settings_slider_value(slider_index: usize, value: f64) -> Result<f64, String> {
    let Some(&(label, min, max, _)) = VOICE_SETTINGS_SLIDERS.get(slider_index) else {
        return Err(format!("slider index {slider_index} out of range (0..4)"));
    };
    if !value.is_finite() || value < min || value > max {
        return Err(format!(
            "slider '{label}' value {value} out of bounds [{min}, {max}]"
        ));
    }
    Ok(value)
}
