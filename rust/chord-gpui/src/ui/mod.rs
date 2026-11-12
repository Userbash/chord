//! GPUI user interface components, panels, modals, and root workspace layout.

pub mod account_switcher;
pub mod call_overlay;
pub mod captcha_modal;
pub mod channel_sidebar;
pub mod chat_view;
pub mod context_menu;
pub mod create_channel_modal;
pub mod create_guild_modal;
pub mod emoji_picker;
pub mod explore_view;
pub mod friends_view;
pub mod layout;
pub mod login_modal;
pub mod member_sidebar;
pub mod quick_switcher;
pub mod screen_share_modal;
pub mod search_drawer;
pub mod server_rail;
pub mod settings_dialog;
pub mod status_picker;
pub mod user_profile_card;
pub mod user_profile_sidebar;
pub mod voice_panel;

pub use layout::AppLayout;
