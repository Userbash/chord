use crate::state::AppState;
use crate::ui::account_switcher::AccountSwitcher;
use crate::ui::captcha_modal::CaptchaModal;
use crate::ui::channel_sidebar::ChannelSidebar;
use crate::ui::chat_view::ChatView;
use crate::ui::context_menu::ContextMenu;
use crate::ui::create_channel_modal::CreateChannelModal;
use crate::ui::create_guild_modal::CreateGuildModal;
use crate::ui::emoji_picker::EmojiPicker;
use crate::ui::explore_view::ExploreView;
use crate::ui::friends_view::FriendsView;
use crate::ui::login_modal::LoginModal;
use crate::ui::member_sidebar::MemberSidebar;
use crate::ui::quick_switcher::QuickSwitcher;
use crate::ui::screen_share_modal::ScreenShareModal;
use crate::ui::search_drawer::SearchDrawer;
use crate::ui::server_rail::ServerRail;
use crate::ui::settings_dialog::SettingsDialog;
use crate::ui::status_picker::StatusPicker;
use crate::ui::user_profile_card::UserProfileCard;
use gpui::*;

pub struct AppLayout {
    pub state: AppState,
    pub focus_handle: FocusHandle,
}

impl AppLayout {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: AppState::new_live(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for AppLayout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("app-root")
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    if this.state.handle_escape_key() {
                        cx.notify();
                    }
                } else if event.keystroke.modifiers.control && event.keystroke.key == "k" {
                    this.state.toggle_quick_switcher();
                    cx.notify();
                } else if event.keystroke.modifiers.control && event.keystroke.key == "e" {
                    this.state.toggle_emoji_picker();
                    cx.notify();
                } else if event.keystroke.modifiers.control && event.keystroke.key == "f" {
                    this.state.toggle_search();
                    cx.notify();
                } else if event.keystroke.modifiers.control
                    && (event.keystroke.key == "v"
                        || event.keystroke.key == "V"
                        || event.keystroke.key == "м"
                        || event.keystroke.key == "М")
                {
                    if let Some(item) = cx.read_from_clipboard() {
                        if let Some(text) = item.text() {
                            if this.state.paste_text(&text) {
                                cx.notify();
                            }
                        }
                    }
                } else {
                    let key = event.keystroke.key.as_str();
                    let is_ctrl = event.keystroke.modifiers.control;
                    if this.state.handle_key_input(key, is_ctrl) {
                        cx.notify();
                    }
                }
            }))
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            // 1. Колонка серверов (ServerRail - 72px)
            .child(
                div()
                    .flex_none()
                    .w(px(72.))
                    .h_full()
                    .child(ServerRail::render(&self.state, window, cx)),
            )
            // 2. Список каналов (ChannelSidebar - 240px)
            .child(
                div()
                    .flex_none()
                    .w(px(240.))
                    .h_full()
                    .child(ChannelSidebar::render(&self.state, window, cx)),
            )
            // 3. Основная область (FriendsView если выбран Личные сообщения, иначе ChatView)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_hidden()
                    .child(
                        if self.state.selected_guild_id.is_none()
                            && self.state.selected_channel_id.as_deref() == Some("explore-servers")
                        {
                            ExploreView::render(&self.state, window, cx).into_any_element()
                        } else if self.state.selected_guild_id.is_none()
                            && (self.state.selected_channel_id.as_deref() == Some("dm-friends")
                                || self.state.selected_channel_id.is_none()
                                || !self
                                    .state
                                    .selected_channel_id
                                    .as_deref()
                                    .unwrap_or("")
                                    .starts_with("dm-"))
                        {
                            FriendsView::render(&self.state, window, cx).into_any_element()
                        } else {
                            ChatView::render(&self.state, window, cx).into_any_element()
                        },
                    ),
            )
            // 4. Правый сайдбар: MemberSidebar на сервере или UserProfileSidebar в ЛС
            .child(if self.state.selected_guild_id.is_some() {
                if self.state.is_member_sidebar_open {
                    div()
                        .flex_none()
                        .w(px(240.))
                        .h_full()
                        .child(MemberSidebar::render(&self.state, window, cx))
                        .into_any_element()
                } else {
                    div().into_any_element()
                }
            } else if self.state.selected_channel_id.as_deref() != Some("dm-friends")
                && self.state.selected_channel_id.is_some()
                && self.state.is_dm_profile_open
            {
                div()
                    .flex_none()
                    .w(px(240.))
                    .h_full()
                    .child(crate::ui::user_profile_sidebar::UserProfileSidebar::render(
                        &self.state,
                        window,
                        cx,
                    ))
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            // 5. Оверлей модального окна hCaptcha (при наличии вызова 40003)
            .children(if self.state.active_captcha.is_some() {
                Some(CaptchaModal::render(&self.state, window, cx))
            } else {
                None
            })
            // 6. Окно создания/вступления на сервер
            .children(if self.state.is_create_server_open {
                Some(CreateGuildModal::render(&self.state, window, cx))
            } else {
                None
            })
            // 7. Окно настроек «Голос и видео»
            .children(if self.state.is_settings_open {
                Some(SettingsDialog::render(&self.state, window, cx))
            } else {
                None
            })
            // 8. Контекстное меню сообщения
            .children(if self.state.active_context_menu.is_some() {
                Some(ContextMenu::render(&self.state, window, cx))
            } else {
                None
            })
            // 9. Quick Switcher (Ctrl+K)
            .children(if self.state.is_quick_switcher_open {
                Some(QuickSwitcher::render(&self.state, window, cx))
            } else {
                None
            })
            // 10. Поповер выбора статуса пользователя
            .children(if self.state.is_status_picker_open {
                Some(StatusPicker::render(&self.state, window, cx))
            } else {
                None
            })
            // 11. Нативный пикер эмодзи (Ctrl+E)
            .children(if self.state.is_emoji_picker_open {
                Some(EmojiPicker::render(&self.state, window, cx))
            } else {
                None
            })
            // 12. Панель поиска сообщений (Ctrl+F)
            .children(if self.state.is_search_open {
                Some(SearchDrawer::render(&self.state, window, cx))
            } else {
                None
            })
            // 13. Карточка профиля участника
            .children(if self.state.selected_member_profile.is_some() {
                Some(UserProfileCard::render(&self.state, window, cx))
            } else {
                None
            })
            // 14. Модальное окно создания канала
            .children(if self.state.is_create_channel_open {
                Some(CreateChannelModal::render(&self.state, window, cx))
            } else {
                None
            })
            // 15. Меню переключения учетных записей
            .children(if self.state.is_account_switcher_open {
                Some(AccountSwitcher::render(&self.state, window, cx))
            } else {
                None
            })
            // 16. Окно входа в Discord
            .children(if self.state.is_login_modal_open {
                Some(LoginModal::render(&self.state, window, cx))
            } else {
                None
            })
            // 17. Окно выбора демонстрации экрана
            .children(if self.state.is_screen_share_modal_open {
                Some(ScreenShareModal::render(&self.state, window, cx))
            } else {
                None
            })
    }
}
