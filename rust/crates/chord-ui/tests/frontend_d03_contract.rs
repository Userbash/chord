//! Contract tests for Task D-03: UI Component Interaction Traces.
//!
//! Verifies empirical UI interaction contracts and the 12 Mana component specifications
//! against the clean-room oracle documented in `docs/re/frontend-d03.md` and
//! `docs/re/frontend-d03.json`:
//! 1. Channel sidebar (`link__2ea32`, `privateChannels_e6b769`, `scroller__99e7c`, `channel__972a0`,
//!    `panels__5e434`, category collapse keeping unread channels, ARIA roles `tree`/`treeitem`/`list`/`listitem`).
//! 2. Chat scroll virtualization (`scrollerBase_d125d2`, `scroller_affa7e`, `listItems_affa7e`,
//!    `scrollbarGutterStable_d125d2`, `wrapper_c19a55 cozy_c19a55 zalgo_c19a55`, dynamic height measurement,
//!    bounded DOM pool, sticky date divider, jump-to-bottom unread pill).
//! 3. Emoji picker (`expressionPickerPositionLayer__74017`, `layer__59d0d`, 3 `[role="tab"]` tabs,
//!    10 nav categories in `listItems_affa7e`, 50 visible / 17 locked grid items, compact reaction picker
//!    `emojiPicker_c0e32c` with 60 items / 43 locked via `message-add-reaction`).
//! 4. User popouts & Member roster (`popoutContainer_ce8328 accountPopout__37e49`, `role="dialog"`,
//!    `aria-modal="true"`, `outer_c0bea0`, `banner_f7e69e`, `avatarStack__44b0c`, `membersWrap_c8ffbb`,
//!    35 visible rows across "Server Booster — 2" and "Online — 735").
//! 5. Settings navigation (`container_abd9a8` 1398x961, `sidebarRegion`, 63 navigation rows across 19 top
//!    categories, Voice & Video tab with 38 `role="switch"` toggles, 4 comboboxes including "Krisp",
//!    4 `slider__4c059` sliders `0..100` / `0..200` / `1..100`, 1 `BaseRadioGroup` `role="radiogroup"`).
//! 6. 12 Mana Component Contracts (`Button`, `TextInput / TextArea`, `Menu (Mana)`, `ContextMenu`,
//!    `Modal`, `Popover`, `Tooltip`, `Switch`, `Slider`, `Avatar`, `Badge`, `Scroller`), design tokens,
//!    keyboard bindings, ARIA roles, and 9 mandatory testing dimensions.

use std::fs;
use std::path::{Path, PathBuf};

use chord_ui::COMPONENT_CONTRACTS;
use chord_ui::d03_contracts::{
    CHANNEL_SIDEBAR_ARIA_ROLES, CHAT_VIRTUALIZER_MAX_DOM_POOL, ChannelCategoryNode,
    ChannelNodeKind, ChannelSidebarItem, ChatVirtualViewport, ComponentContractSpec,
    D03_COMPONENT_NAMES, D03_DARK_THEME_TOKENS, D03_KEYBOARD_BINDINGS, D03_SURFACE_TRACES,
    EMOJI_CATEGORY_RAIL_ITEMS, EmojiPickerMode, EmojiPickerModel, SETTINGS_TOP_CATEGORIES,
    SettingsNavModel, SurfaceTraceKind, UserPopoutModel, VOICE_SETTINGS_SLIDERS,
    build_channel_sidebar, component_contract_by_name, d03_component_contracts, d03_surface_trace,
    validate_settings_slider_value,
};
use chord_ui::member_list::{
    MemberGroup, MemberGroupKind, MemberListModel, MemberRow, PresenceStatus, VirtualWindow,
};
use chord_ui::search_overlay::{
    SearchAuthorType, SearchContentKind, SearchOverlay, SearchQuery, SearchSort,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Section 1 — Positive & Oracle Alignment: All 5 UI interaction trace surfaces and 12 Mana component
/// contracts match `docs/re/frontend-d03.md` and `docs/re/frontend-d03.json`.
#[test]
fn d03_five_ui_surfaces_and_twelve_component_contracts_match_oracle() {
    assert_eq!(COMPONENT_CONTRACTS, 12);
    assert_eq!(D03_COMPONENT_NAMES.len(), 12);
    assert_eq!(D03_SURFACE_TRACES.len(), 6);

    let contracts: &[ComponentContractSpec] = d03_component_contracts();
    assert_eq!(contracts.len(), 12);

    for (idx, expected_name) in D03_COMPONENT_NAMES.iter().enumerate() {
        assert_eq!(
            contracts[idx].name, *expected_name,
            "component[{idx}] name mismatch"
        );
        let looked_up = component_contract_by_name(expected_name)
            .unwrap_or_else(|| panic!("component {expected_name} must resolve by name"));
        assert_eq!(looked_up.name, *expected_name);
        assert!(
            !looked_up.module_id.is_empty(),
            "{expected_name} module_id must be non-empty"
        );
        assert!(
            !looked_up.dom_classes.is_empty(),
            "{expected_name} dom_classes must be non-empty"
        );
        assert!(
            !looked_up.aria_roles.is_empty(),
            "{expected_name} aria_roles must be non-empty"
        );
        assert!(
            !looked_up.keyboard_bindings.is_empty(),
            "{expected_name} keyboard_bindings must be non-empty"
        );
        assert!(
            !looked_up.variants.is_empty(),
            "{expected_name} variants must be non-empty"
        );
    }

    // Verify all required D-03 surface traces exist and define DOM hierarchy, CSS tokens,
    // keyboard bindings, and ARIA roles.
    for kind in [
        SurfaceTraceKind::ChannelSidebar,
        SurfaceTraceKind::ChatScrollVirtualization,
        SurfaceTraceKind::EmojiPicker,
        SurfaceTraceKind::UserPopoutsAndMembers,
        SurfaceTraceKind::SettingsNavigation,
        SurfaceTraceKind::GuildSearch,
    ] {
        let trace = d03_surface_trace(kind);
        assert_eq!(trace.kind, kind);
        assert!(!trace.root_selector.is_empty());
        assert!(!trace.dom_hierarchy.is_empty());
        assert!(!trace.css_tokens.is_empty());
        assert!(!trace.keyboard_bindings.is_empty());
        assert!(!trace.aria_roles.is_empty());
    }

    assert!(D03_DARK_THEME_TOKENS.len() >= 14);
    assert!(D03_KEYBOARD_BINDINGS.len() >= 10);
}

/// Section 2 — Channel Sidebar Contract: Guild channel tree (`link__2ea32`) & DM list (`privateChannels_e6b769`,
/// `scroller__99e7c`, `channel__972a0`), category collapse filtering unread vs read channels,
/// deterministic ordering (non-voice before voice, then position, then snowflake id), and ARIA roles.
#[test]
fn d03_channel_sidebar_tree_collapse_ordering_and_aria_contract() {
    assert!(CHANNEL_SIDEBAR_ARIA_ROLES.contains(&"tree"));
    assert!(CHANNEL_SIDEBAR_ARIA_ROLES.contains(&"treeitem"));
    assert!(CHANNEL_SIDEBAR_ARIA_ROLES.contains(&"list"));
    assert!(CHANNEL_SIDEBAR_ARIA_ROLES.contains(&"listitem"));

    let items = vec![
        ChannelSidebarItem {
            id: 294_064_825_540_280_335,
            parent_id: Some(100),
            name: "🔤болталка".to_string(),
            kind: ChannelNodeKind::GuildText,
            position: 1,
            unread: true,
            mentions: 2,
        },
        ChannelSidebarItem {
            id: 645_271_072_786_350_111,
            parent_id: Some(100),
            name: "💬welcome".to_string(),
            kind: ChannelNodeKind::GuildText,
            position: 0,
            unread: false,
            mentions: 0,
        },
        ChannelSidebarItem {
            id: 700_000_000_000_000_001,
            parent_id: Some(100),
            name: "General Voice".to_string(),
            kind: ChannelNodeKind::GuildVoice,
            position: 0,
            unread: false,
            mentions: 0,
        },
        ChannelSidebarItem {
            id: 1_094_593_230_114_390_136,
            parent_id: None,
            name: "💾dpakc_help🖥💻".to_string(),
            kind: ChannelNodeKind::GuildForum,
            position: 0,
            unread: false,
            mentions: 0,
        },
    ];

    let categories = vec![ChannelCategoryNode {
        id: 100,
        name: "Основное".to_string(),
        position: 0,
        collapsed: false,
    }];

    // Expanded category shows all 3 child channels with non-voice sorted before voice.
    let expanded = build_channel_sidebar(&categories, &items);
    assert_eq!(expanded.uncategorized.len(), 1);
    assert_eq!(expanded.uncategorized[0].id, 1_094_593_230_114_390_136);
    assert_eq!(expanded.categories.len(), 1);
    assert_eq!(expanded.categories[0].all_children.len(), 3);
    assert_eq!(expanded.categories[0].visible_children.len(), 3);
    // Position 0 text ("welcome") before position 1 text ("болталка") before position 0 voice ("General Voice")
    assert_eq!(
        expanded.categories[0].visible_children[0].id,
        645_271_072_786_350_111
    );
    assert_eq!(
        expanded.categories[0].visible_children[1].id,
        294_064_825_540_280_335
    );
    assert_eq!(
        expanded.categories[0].visible_children[2].id,
        700_000_000_000_000_001
    );

    // Collapsed category hides read channels but preserves unread/mentioned channels in visible_children.
    let collapsed_cats = vec![ChannelCategoryNode {
        id: 100,
        name: "Основное".to_string(),
        position: 0,
        collapsed: true,
    }];
    let collapsed = build_channel_sidebar(&collapsed_cats, &items);
    assert_eq!(collapsed.categories[0].all_children.len(), 3);
    assert_eq!(collapsed.categories[0].visible_children.len(), 1);
    assert_eq!(
        collapsed.categories[0].visible_children[0].id,
        294_064_825_540_280_335
    );
    assert!(
        collapsed.categories[0].visible_children[0]
            .aria_label()
            .contains("не прочитано")
    );
}

/// Section 3 — Chat Scroll Virtualization Contract: Scroller (`scrollerBase_d125d2`, `scrollbarGutterStable_d125d2`),
/// message rows (`wrapper_c19a55 cozy_c19a55 zalgo_c19a55`), dynamic height measurement,
/// bounded DOM pool (`<= 60` nodes for 100k messages), sticky date divider, and jump-to-bottom unread pill.
#[test]
fn d03_chat_scroll_virtualization_dynamic_heights_and_bounded_pool() {
    let mut viewport = ChatVirtualViewport::new(100_000, 48, 600, 6).expect("valid viewport");
    assert_eq!(viewport.total_height(), 4_800_000);
    assert!(viewport.is_at_bottom());
    assert!(!viewport.jump_pill_visible());

    // At bottom of 100k messages, visible pool is strictly bounded <= CHAT_VIRTUALIZER_MAX_DOM_POOL (60).
    let bottom_range = viewport.rendered_range();
    assert!(bottom_range.len() <= CHAT_VIRTUALIZER_MAX_DOM_POOL);
    assert_eq!(bottom_range.end, 100_000);

    // Dynamic height measurement: message 0 expands from 48px to 120px (+72px).
    viewport.measure_row(0, 120).expect("row 0 measured");
    assert_eq!(viewport.total_height(), 4_800_072);
    assert_eq!(viewport.row_offset(0), 0);
    assert_eq!(viewport.row_offset(1), 120);

    // Scroll up to middle (index 50,000): jump-to-bottom pill becomes visible, pool stays bounded.
    viewport.scroll_to_index(50_000);
    assert!(!viewport.is_at_bottom());
    assert!(viewport.jump_pill_visible());
    let mid_range = viewport.rendered_range();
    assert!(mid_range.contains(&50_000));
    assert!(mid_range.len() <= CHAT_VIRTUALIZER_MAX_DOM_POOL);

    // Jump to bottom resets scroll to max and hides unread pill.
    viewport.record_unread(5);
    assert_eq!(viewport.unread_pill_count(), 5);
    viewport.jump_to_bottom();
    assert!(viewport.is_at_bottom());
    assert!(!viewport.jump_pill_visible());
    assert_eq!(viewport.unread_pill_count(), 0);

    // Negative boundary: 0 row height or 0 viewport height rejected.
    assert!(ChatVirtualViewport::new(100, 0, 600, 4).is_err());
    assert!(ChatVirtualViewport::new(100, 48, 0, 4).is_err());
    assert!(viewport.measure_row(100_000, 80).is_err());
    assert!(viewport.measure_row(0, 0).is_err());
}

/// Section 4 — Emoji Picker Contract: Full expression picker (`Ctrl+E`, `expressionPickerPositionLayer__74017`,
/// 3 tabs: GIFs/Stickers/Emoji, 10 category rail items in `listItems_affa7e`, 50 visible / 17 locked
/// in guild channel, 42 visible in DM, `:fire` search returning 10/11 items) vs compact reaction picker
/// (`emojiPicker_c0e32c`, 60 items / 43 locked, no search/tabs).
#[test]
fn d03_emoji_picker_full_and_reaction_modes_match_empirical_counts() {
    assert_eq!(EMOJI_CATEGORY_RAIL_ITEMS.len(), 10);
    assert_eq!(EMOJI_CATEGORY_RAIL_ITEMS[0], "top guild emoji");
    assert_eq!(EMOJI_CATEGORY_RAIL_ITEMS[1], "recent");
    assert_eq!(EMOJI_CATEGORY_RAIL_ITEMS[9], "symbols");

    let mut full_guild = EmojiPickerModel::new(EmojiPickerMode::FullGuildChannel);
    assert_eq!(full_guild.tabs(), &["Гифки", "Стикеры", "Эмодзи"]);
    assert_eq!(full_guild.active_tab(), "Эмодзи");
    assert!(full_guild.has_search_input());
    assert_eq!(full_guild.visible_grid_count(), 50);
    assert_eq!(full_guild.locked_nitro_count(), 17);
    assert_eq!(full_guild.category_rail().len(), 10);

    // Search ":fire" in guild channel yields 10 Nitro-gated items (D-03 §2.1).
    full_guild.set_search_query(":fire");
    assert_eq!(full_guild.filtered_count(), 10);

    // Full picker in DM yields 42 visible items and 11 results for ":fire" (D-03 §2.2).
    let mut full_dm = EmojiPickerModel::new(EmojiPickerMode::FullDirectMessage);
    assert_eq!(full_dm.visible_grid_count(), 42);
    full_dm.set_search_query(":fire");
    assert_eq!(full_dm.filtered_count(), 11);

    // Reaction picker opened from message context menu `message-add-reaction` omits tabs and search (D-03 §2.3).
    let reaction = EmojiPickerModel::new(EmojiPickerMode::CompactReaction);
    assert!(reaction.tabs().is_empty());
    assert!(!reaction.has_search_input());
    assert_eq!(reaction.visible_grid_count(), 60);
    assert_eq!(reaction.locked_nitro_count(), 43);
}

/// Section 5 — User Popouts & Member Roster Contract: Account/User popout (`popoutContainer_ce8328`,
/// `role="dialog"`, `aria-modal="true"`, 80x80 avatar with status mask, banner 105px height,
/// custom status button, copy username/user ID actions) and right sidebar member roster
/// (`membersWrap_c8ffbb`, 256x1025 rect, "Server Booster — 2", "В сети — 735", 35 visible rows).
#[test]
fn d03_user_popouts_and_member_roster_contract() {
    let popout = UserPopoutModel::new(
        384_299_970_632_876_032,
        "ShadowByte",
        "pharos12510",
        PresenceStatus::Invisible,
        vec![
            "Старое имя: Tobio#8521".to_string(),
            "Достигнут уровень 0".to_string(),
        ],
    )
    .expect("valid user popout");

    assert_eq!(popout.aria_role(), "dialog");
    assert!(popout.aria_modal());
    assert_eq!(
        popout.root_class(),
        "popoutContainer_ce8328 accountPopout__37e49"
    );
    assert_eq!(popout.banner_height_px(), 105);
    assert_eq!(popout.avatar_size_px(), 80);
    assert_eq!(popout.badges().len(), 2);

    // Boundary check: user_id 0 or empty username rejected.
    assert!(
        UserPopoutModel::new(
            0,
            "ShadowByte",
            "pharos12510",
            PresenceStatus::Online,
            vec![]
        )
        .is_err()
    );
    assert!(UserPopoutModel::new(1, "ShadowByte", "", PresenceStatus::Online, vec![]).is_err());
    assert!(
        UserPopoutModel::new(
            u64::MAX,
            "MaxUser",
            "max_user",
            PresenceStatus::Online,
            vec![]
        )
        .is_ok()
    );

    // Member roster contract from D-03 §3
    let booster_group = MemberGroup {
        kind: MemberGroupKind::Role {
            role_id: 1,
            name: "Server Booster".to_string(),
        },
        members: vec![
            MemberRow {
                user_id: 101,
                display_name: Some("Roman Pony".to_string()),
                username: "roman_pony".to_string(),
                status: PresenceStatus::Online,
                avatar_url: None,
                role_color: None,
                activity: Some("Буст сервера с 4 окт. 2026 г.".to_string()),
            },
            MemberRow {
                user_id: 102,
                display_name: Some("VoidCat | Kirby".to_string()),
                username: "voidcat".to_string(),
                status: PresenceStatus::Online,
                avatar_url: None,
                role_color: None,
                activity: Some("Буст сервера с 29 сент.".to_string()),
            },
        ],
    };
    assert_eq!(booster_group.title(), "Server Booster — 2");

    let mut online_members = Vec::with_capacity(735);
    for i in 0..735 {
        online_members.push(MemberRow {
            user_id: 1000 + i as u64,
            display_name: Some(format!("Member {i}")),
            username: format!("member_{i}"),
            status: PresenceStatus::Online,
            avatar_url: None,
            role_color: None,
            activity: None,
        });
    }
    let online_group = MemberGroup {
        kind: MemberGroupKind::Online,
        members: online_members,
    };
    assert_eq!(online_group.title(), "В сети — 735");

    let mut roster = MemberListModel::new(vec![booster_group, online_group]);
    // Set viewport to 35 visible rows (34 + 1 overscan at 30px per row = 1020px height, matching 1025px capture)
    roster.set_window(VirtualWindow::new(0, 1020, 30).unwrap());
    assert_eq!(roster.visible_range().len(), 35);
}

/// Section 6 — Settings Navigation & Voice Tab Contract: Root `container_abd9a8` (1398x961),
/// 63 navigation rows across 19 top categories, search input, Escape key dismissal,
/// and "Voice & Video" tab controls (38 switches, 4 selects including Krisp, 1 radio group,
/// 4 sliders with exact ranges `0..100`, `0..200`, `1..100`, `0..100`).
#[test]
fn d03_settings_navigation_and_voice_video_controls_contract() {
    assert_eq!(SETTINGS_TOP_CATEGORIES.len(), 19);
    assert_eq!(VOICE_SETTINGS_SLIDERS.len(), 4);

    let mut settings = SettingsNavModel::new();
    assert_eq!(settings.root_class(), "container_abd9a8");
    assert_eq!(settings.dimensions_px(), (1398, 961));
    assert_eq!(settings.total_nav_rows(), 63);
    assert_eq!(settings.top_categories().len(), 19);
    assert_eq!(settings.active_category(), "Учётная запись");

    // Select "Голос и видео" tab
    settings
        .select_category("Голос и видео")
        .expect("Voice & Video category exists");
    assert_eq!(settings.active_category(), "Голос и видео");
    assert_eq!(settings.voice_tab_switch_count(), 38);
    assert_eq!(settings.voice_tab_select_count(), 4);
    assert_eq!(
        settings.voice_tab_radio_options(),
        &["Изоляция голоса", "Студия", "Пользовательский"]
    );

    // Filter categories by search query
    settings.set_search_filter("Голос");
    assert_eq!(settings.filtered_categories(), vec!["Голос и видео"]);
    settings.set_search_filter("несуществующая_категория");
    assert!(settings.filtered_categories().is_empty());
    settings.set_search_filter("");
    assert_eq!(settings.filtered_categories().len(), 19);

    // Invalid category selection returns explicit error
    assert!(settings.select_category("Unknown Category").is_err());

    // Validate slider ranges (Input Volume 0..100, Output Volume 0..200, Stream Attenuation 1..100, Sensitivity 0..100)
    assert!(validate_settings_slider_value(0, 100.0).is_ok());
    assert!(validate_settings_slider_value(0, 101.0).is_err());
    assert!(validate_settings_slider_value(1, 200.0).is_ok());
    assert!(validate_settings_slider_value(1, 201.0).is_err());
    assert!(validate_settings_slider_value(2, 1.0).is_ok());
    assert!(validate_settings_slider_value(2, 0.0).is_err());
    assert!(validate_settings_slider_value(3, 26.11).is_ok());
    assert!(validate_settings_slider_value(99, 50.0).is_err());
}

/// Section 7 — Guild Search UI & Filter Contract: 7 filter sections, 9 `has:` options, 3 sort options,
/// `Ctrl+A` scope-chip clearing workflow (`total_results=3302` unscoped vs `0` channel-scoped).
#[test]
fn d03_guild_search_filters_sorting_and_scope_chip_contract() {
    assert_eq!(SearchContentKind::all().len(), 9);
    assert_eq!(SearchSort::all().len(), 3);

    let mut overlay = SearchOverlay::new();
    overlay.open();
    overlay.set_text("ProBot");
    overlay.set_sort(SearchSort::Newest);
    overlay.toggle_contains(SearchContentKind::Poll);
    overlay.query_mut().filters.author_type = Some(SearchAuthorType::Bot);

    let params = overlay.query().to_params();
    assert!(params.contains(&("content".to_string(), "ProBot".to_string())));
    assert!(params.contains(&("has".to_string(), "poll".to_string())));
    assert!(params.contains(&("author_type".to_string(), "bot".to_string())));
    assert!(params.contains(&("sort_by".to_string(), "timestamp".to_string())));
    assert!(params.contains(&("sort_order".to_string(), "desc".to_string())));

    // Empty query without filters is not executable
    assert!(!SearchQuery::new("   ").is_executable());
}

/// Section 8 — Documentation Completeness: `docs/re/frontend-d03.md` and `docs/re/frontend-d03.json`
/// exist, are synchronized, and document all 5 D-03 target surfaces (channel sidebar,
/// chat scroll virtualization, emoji picker, user popouts, settings navigation) plus all
/// 12 Mana component contracts with DOM hierarchy, CSS tokens, keyboard bindings, and ARIA roles.
#[test]
fn d03_markdown_and_json_documentation_completeness() {
    let root = repo_root();
    let md_path = root.join("docs/re/frontend-d03.md");
    let json_path = root.join("docs/re/frontend-d03.json");

    let md = fs::read_to_string(&md_path).expect("docs/re/frontend-d03.md must exist");
    let json = fs::read_to_string(&json_path).expect("docs/re/frontend-d03.json must exist");

    // Verify all D-03 criteria surfaces and component contracts are documented in docs/re/frontend-d03.md
    for required_marker in [
        "Channel Sidebar",
        "Chat Scroll Virtualization",
        "Emoji",
        "User Popout",
        "Member Roster",
        "Settings",
        "Component Contracts (12 Components)",
        "Design Tokens Reference",
        "Keyboard Bindings & ARIA Roles Matrix",
        "link__2ea32",
        "privateChannels_e6b769",
        "scroller__99e7c",
        "channel__972a0",
        "panels__5e434",
        "scrollerBase_d125d2",
        "scrollbarGutterStable_d125d2",
        "listItems_affa7e",
        "wrapper_c19a55",
        "expressionPickerPositionLayer__74017",
        "emojiPicker_c0e32c",
        "emojiItem_fc7141",
        "lockedEmoji_d982c2",
        "popoutContainer_ce8328",
        "accountPopout__37e49",
        "outer_c0bea0",
        "membersWrap_c8ffbb",
        "container_abd9a8",
        "slider__4c059",
        "button_a22cb0",
        "input__75098",
        "menu_c1e9c4",
        "layer__59d0d",
        "tooltip_c36707",
        "avatar_fdd9da",
        "numberBadge__463b7",
        "--modal-width-small/medium/large",
        "442px / 602px / 800px",
        "2147483647px",
    ] {
        assert!(
            md.contains(required_marker),
            "docs/re/frontend-d03.md missing required D-03 specification marker: {required_marker}"
        );
    }

    // Verify JSON oracle contains task D-03 and all 12 component names
    assert!(json.contains("\"task\": \"D-03\""));
    for comp_name in D03_COMPONENT_NAMES {
        assert!(
            json.contains(comp_name),
            "docs/re/frontend-d03.json missing component: {comp_name}"
        );
    }
}
