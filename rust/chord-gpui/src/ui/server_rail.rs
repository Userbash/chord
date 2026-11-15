//! Leftmost 72px guild navigation rail with DM shortcut, guild icons, and discovery trigger.

use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct ServerRail;

impl ServerRail {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let selected_guild_id = state.selected_guild_id.clone();
        let guilds = state.guilds.clone();

        div()
            .v_flex()
            .flex_none()
            .w(px(72.))
            .h_full()
            .bg(DISCORD_THEME.bg_tertiary)
            .py(px(12.))
            .items_center()
            .gap(px(8.))
            .overflow_hidden()
            // 1. Direct Messages / Discord Home Icon
            .child({
                let is_selected = selected_guild_id.is_none();
                div()
                    .relative()
                    .flex_none()
                    .w(px(72.))
                    .h(px(48.))
                    .flex()
                    .items_center()
                    .justify_center()
                    // Pill Indicator
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .w(px(4.))
                            .rounded_r(px(4.))
                            .bg(gpui::white())
                            .h(if is_selected { px(40.) } else { px(0.) }),
                    )
                    // Icon Button
                    .child(
                        div()
                            .id("btn-home")
                            .size(px(48.))
                            .rounded(if is_selected { px(16.) } else { px(24.) })
                            .bg(if is_selected {
                                DISCORD_THEME.blurple
                            } else {
                                DISCORD_THEME.bg_primary
                            })
                            .hover(|s| s.bg(DISCORD_THEME.blurple).rounded(px(16.)))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui::white())
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(20.))
                            .child("🎮")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.select_guild(None);
                                    cx.notify();
                                }),
                            ),
                    )
            })
            // 2. Разделитель
            .child(
                div()
                    .flex_none()
                    .w(px(32.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(DISCORD_THEME.border_subtle),
            )
            // 3. Список серверов (с поддержкой группировки guild_folders по референсу dissent)
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_hidden()
                    .items_center()
                    .gap(px(8.))
                    .children(if state.guild_folders.is_empty() {
                        // Обычный плоский список, если папки не заданы
                        guilds
                            .into_iter()
                            .map(|guild| {
                                let is_selected = selected_guild_id.as_deref() == Some(&guild.id);
                                Self::render_guild_icon(&guild, is_selected, cx).into_any_element()
                            })
                            .collect::<Vec<AnyElement>>()
                    } else {
                        // Иерархический список с группировкой по guild_folders
                        let mut folder_elements: Vec<AnyElement> = Vec::new();
                        for folder in &state.guild_folders {
                            if let Some(folder_id) = folder.id {
                                let is_expanded = state.is_folder_expanded(folder_id);
                                let fid = folder_id;

                                // Иконка папки серверов
                                let folder_btn = div()
                                    .id(SharedString::from(format!("folder-icon-{}", fid)))
                                    .relative()
                                    .flex_none()
                                    .w(px(72.))
                                    .h(px(48.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .size(px(48.))
                                            .rounded(px(16.))
                                            .bg(if is_expanded {
                                                DISCORD_THEME.bg_secondary
                                            } else {
                                                DISCORD_THEME.bg_primary
                                            })
                                            .hover(|s| s.bg(DISCORD_THEME.bg_secondary))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(20.))
                                            .child(if is_expanded { "📂" } else { "📁" })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.state.toggle_folder_expanded(fid);
                                                    cx.notify();
                                                }),
                                            ),
                                    );
                                folder_elements.push(folder_btn.into_any_element());

                                // Если папка раскрыта — отображаем серверы внутри папки
                                if is_expanded {
                                    for gid in &folder.guild_ids {
                                        if let Some(guild) = guilds.iter().find(|g| &g.id == gid) {
                                            let is_selected =
                                                selected_guild_id.as_deref() == Some(&guild.id);
                                            folder_elements.push(
                                                Self::render_guild_icon(guild, is_selected, cx)
                                                    .into_any_element(),
                                            );
                                        }
                                    }
                                }
                            } else {
                                // Серверы верхнего уровня без папки
                                for gid in &folder.guild_ids {
                                    if let Some(guild) = guilds.iter().find(|g| &g.id == gid) {
                                        let is_selected =
                                            selected_guild_id.as_deref() == Some(&guild.id);
                                        folder_elements.push(
                                            Self::render_guild_icon(guild, is_selected, cx)
                                                .into_any_element(),
                                        );
                                    }
                                }
                            }
                        }
                        folder_elements
                    }),
            )
            // 4. Кнопка «Добавить сервер» (+)
            .child(
                div()
                    .id("btn-add-server")
                    .flex_none()
                    .size(px(48.))
                    .rounded_full()
                    .bg(DISCORD_THEME.bg_primary)
                    .hover(|s| {
                        s.bg(DISCORD_THEME.status_online)
                            .rounded(px(16.))
                            .text_color(gpui::white())
                    })
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(DISCORD_THEME.status_online)
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .child("+")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.open_create_server();
                            cx.notify();
                        }),
                    ),
            )
            // 5. Кнопка «Публичные серверы» (Explore Public Servers 🧭)
            .child(
                div()
                    .id("btn-explore-servers")
                    .flex_none()
                    .size(px(48.))
                    .rounded_full()
                    .bg(DISCORD_THEME.bg_primary)
                    .hover(|s| {
                        s.bg(DISCORD_THEME.status_online)
                            .rounded(px(16.))
                            .text_color(gpui::white())
                    })
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(DISCORD_THEME.status_online)
                    .text_size(px(20.))
                    .child("🧭")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.selected_guild_id = None;
                            this.state.selected_channel_id = Some("explore-servers".into());
                            cx.notify();
                        }),
                    ),
            )
    }

    /// Отрисовка отдельной иконки сервера в боковой панели
    fn render_guild_icon(
        guild: &crate::state::Guild,
        is_selected: bool,
        cx: &mut Context<AppLayout>,
    ) -> Div {
        let gid = guild.id.clone();
        let has_unread = guild.unread_count > 0;

        div()
            .relative()
            .flex_none()
            .w(px(72.))
            .h(px(48.))
            .flex()
            .items_center()
            .justify_center()
            // Pill indicator
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w(px(4.))
                    .rounded_r(px(4.))
                    .bg(gpui::white())
                    .h(if is_selected {
                        px(40.)
                    } else if has_unread {
                        px(8.)
                    } else {
                        px(0.)
                    }),
            )
            // Guild Icon
            .child(
                div()
                    .id(SharedString::from(format!("guild-icon-{}", gid)))
                    .size(px(48.))
                    .rounded(if is_selected { px(16.) } else { px(24.) })
                    .bg(if is_selected {
                        DISCORD_THEME.blurple
                    } else {
                        DISCORD_THEME.bg_primary
                    })
                    .hover(|s| s.bg(DISCORD_THEME.blurple).rounded(px(16.)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::white())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(18.))
                    .child(guild.icon.clone().unwrap_or_else(|| {
                        guild
                            .name
                            .chars()
                            .next()
                            .map(|c| c.to_string())
                            .unwrap_or_default()
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.state.select_guild(Some(gid.clone()));
                            cx.notify();
                        }),
                    ),
            )
            // Mention Badge (red bubble with numeric count or 99+)
            .children(if guild.mention_count > 0 {
                let badge_text = if guild.mention_count > 99 {
                    "99+".to_string()
                } else {
                    guild.mention_count.to_string()
                };
                Some(
                    div()
                        .absolute()
                        .bottom_0()
                        .right(px(8.))
                        .min_w(px(18.))
                        .h(px(18.))
                        .px(px(4.))
                        .rounded_full()
                        .bg(DISCORD_THEME.status_dnd)
                        .border_2()
                        .border_color(DISCORD_THEME.bg_tertiary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .child(badge_text),
                )
            } else {
                None
            })
    }
}
