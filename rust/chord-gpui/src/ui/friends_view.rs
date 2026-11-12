use crate::state::{AppState, RelationshipType};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Вкладка списка друзей и личных сообщений (Friends List View)
/// Отображается когда не выбран ни один сервер (`selected_guild_id == None`)
pub struct FriendsView;

impl FriendsView {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let friends: Vec<_> = state
            .relationships
            .iter()
            .filter(|r| r.relationship_type == RelationshipType::Friend)
            .collect();

        let pending: Vec<_> = state
            .relationships
            .iter()
            .filter(|r| {
                r.relationship_type == RelationshipType::IncomingRequest
                    || r.relationship_type == RelationshipType::OutgoingRequest
            })
            .collect();

        div()
            .v_flex()
            .flex_1()
            .h_full()
            .bg(DISCORD_THEME.bg_primary)
            // 1. Верхний бар вкладок друзей
            .child(
                div()
                    .h(px(48.))
                    .px(px(16.))
                    .border_b_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(18.)).child("👋"))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Друзья"),
                            ),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_active)
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child(format!("Все — {}", friends.len())),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .hover(|s| s.text_color(DISCORD_THEME.text_normal))
                            .child(format!("Ожидание — {}", pending.len())),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.status_online)
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .cursor_pointer()
                            .child("Добавить в друзья"),
                    ),
            )
            // 2. Список друзей
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .p(px(20.))
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child(format!("ВСЕ ДРУЗЬЯ — {}", friends.len())),
                    )
                    .children(friends.into_iter().map(|rel| {
                        let uid = rel.user.id.clone();
                        let uname = rel.user.username.clone();
                        let dname = rel
                            .nickname
                            .clone()
                            .or_else(|| rel.user.global_name.clone())
                            .unwrap_or_else(|| uname.clone());
                        let activity = rel.activity.clone();

                        div()
                            .h(px(60.))
                            .px(px(12.))
                            .rounded(px(8.))
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .border_t_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .size(px(40.))
                                            .rounded_full()
                                            .bg(DISCORD_THEME.blurple)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(gpui::white())
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(16.))
                                            .child(
                                                uname
                                                    .chars()
                                                    .next()
                                                    .map(|c| c.to_string())
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .v_flex()
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(DISCORD_THEME.text_header)
                                                    .child(dname),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(DISCORD_THEME.text_muted)
                                                    .child(
                                                        activity.unwrap_or_else(|| {
                                                            format!("@{}", uname)
                                                        }),
                                                    ),
                                            ),
                                    ),
                            )
                            // Кнопки действия (ЛС, Звонок)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .size(px(36.))
                                            .rounded_full()
                                            .bg(DISCORD_THEME.bg_secondary)
                                            .hover(|s| s.bg(DISCORD_THEME.bg_active))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("💬")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener({
                                                    let uid = uid.clone();
                                                    move |this, _, _window, cx| {
                                                        this.state
                                                            .select_channel(format!("dm-{}", uid));
                                                        cx.notify();
                                                    }
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .size(px(36.))
                                            .rounded_full()
                                            .bg(DISCORD_THEME.bg_secondary)
                                            .hover(|s| s.bg(DISCORD_THEME.bg_active))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("📞")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener({
                                                    let uname = uname.clone();
                                                    move |this, _, _window, cx| {
                                                        this.state.connect_voice(
                                                            "call-direct".into(),
                                                            uname.clone(),
                                                            "Личный звонок".into(),
                                                        );
                                                        cx.notify();
                                                    }
                                                }),
                                            ),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }
}
