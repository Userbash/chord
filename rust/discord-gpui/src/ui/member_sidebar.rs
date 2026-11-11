use crate::state::{AppState, GuildMember, PresenceStatus};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::tooltip::Tooltip;
use gpui_kit::component::StyledExt;

/// Конфигурация роли для группировки, раскраски бейджей и порядка в списке
#[derive(Debug, Clone, PartialEq)]
pub struct RoleConfig {
    pub name: String,
    pub color: Hsla,
    pub badge_bg: Hsla,
    pub badge_border: Hsla,
    pub priority: i32,
    pub emoji: Option<&'static str>,
}

/// Группа участников по роли в боковой панели
#[derive(Debug, Clone)]
pub struct MemberGroup {
    pub role_name: String,
    pub role_color: Hsla,
    pub badge_bg: Hsla,
    pub badge_border: Hsla,
    pub priority: i32,
    pub emoji: Option<&'static str>,
    pub members: Vec<GuildMember>,
    pub is_offline: bool,
}

pub struct MemberSidebar;

impl MemberSidebar {
    /// Получение конфигурации роли (цвет, приоритет, бейдж, эмодзи)
    pub fn role_config_for(role_name: &str) -> RoleConfig {
        match role_name {
            "Admin" | "Администратор" | "Owner" | "Владелец" => RoleConfig {
                name: role_name.to_string(),
                color: gpui::rgb(0xed4245).into(), // Discord Red #ED4245
                badge_bg: gpui::rgba(0xed424526).into(),
                badge_border: gpui::rgba(0xed42454d).into(),
                priority: 100,
                emoji: Some("👑"),
            },
            "Server Booster" | "Бустер" => RoleConfig {
                name: role_name.to_string(),
                color: gpui::rgb(0xf47fff).into(), // Discord Nitro Pink #F47FFF
                badge_bg: gpui::rgba(0xf47fff26).into(),
                badge_border: gpui::rgba(0xf47fff4d).into(),
                priority: 90,
                emoji: Some("🚀"),
            },
            "Developer" | "Разработчик" => RoleConfig {
                name: role_name.to_string(),
                color: gpui::rgb(0x00a8fc).into(), // Discord Cyan/Blue #00A8FC
                badge_bg: gpui::rgba(0x00a8fc26).into(),
                badge_border: gpui::rgba(0x00a8fc4d).into(),
                priority: 80,
                emoji: Some("💻"),
            },
            "Moderator" | "Модератор" => RoleConfig {
                name: role_name.to_string(),
                color: gpui::rgb(0x2ecc71).into(), // Discord Green #2ECC71
                badge_bg: gpui::rgba(0x2ecc7126).into(),
                badge_border: gpui::rgba(0x2ecc714d).into(),
                priority: 70,
                emoji: Some("🛡️"),
            },
            "QA" | "Тестировщик" => RoleConfig {
                name: role_name.to_string(),
                color: gpui::rgb(0x57f287).into(), // Discord Mint #57F287
                badge_bg: gpui::rgba(0x57f28726).into(),
                badge_border: gpui::rgba(0x57f2874d).into(),
                priority: 60,
                emoji: Some("🧪"),
            },
            "VIP" | "Премиум" => RoleConfig {
                name: role_name.to_string(),
                color: gpui::rgb(0xfaa81a).into(), // Discord Gold #FAA81A
                badge_bg: gpui::rgba(0xfaa81a26).into(),
                badge_border: gpui::rgba(0xfaa81a4d).into(),
                priority: 50,
                emoji: Some("⭐"),
            },
            other => {
                // Стабильный алгоритм генерации гармоничного Discord-цвета по хэшу имени
                let hash = other.bytes().fold(5381u32, |acc, b| {
                    acc.wrapping_mul(33).wrapping_add(b as u32)
                });
                let hue = (hash % 360) as f32 / 360.0;
                let color = gpui::hsla(hue, 0.75, 0.65, 1.0);
                let badge_bg = gpui::hsla(hue, 0.75, 0.65, 0.15);
                let badge_border = gpui::hsla(hue, 0.75, 0.65, 0.35);
                RoleConfig {
                    name: other.to_string(),
                    color,
                    badge_bg,
                    badge_border,
                    priority: 30,
                    emoji: None,
                }
            }
        }
    }

    /// Определение цвета индикатора статуса присутствия с учётом стриминга
    pub fn status_color(status: PresenceStatus, is_streaming: bool) -> Hsla {
        if is_streaming {
            DISCORD_THEME.status_streaming
        } else {
            match status {
                PresenceStatus::Online => DISCORD_THEME.status_online,
                PresenceStatus::Idle => DISCORD_THEME.status_idle,
                PresenceStatus::Dnd => DISCORD_THEME.status_dnd,
                PresenceStatus::Offline => DISCORD_THEME.status_offline,
            }
        }
    }

    /// Определение высшей роли участника
    pub fn top_role_for(member: &GuildMember) -> Option<RoleConfig> {
        member
            .roles
            .iter()
            .map(|r| Self::role_config_for(r))
            .max_by_key(|cfg| cfg.priority)
    }

    /// Группировка участников гильдии по ролям с учётом онлайн-статуса и приоритета
    pub fn group_members(members: &[GuildMember]) -> Vec<MemberGroup> {
        let mut role_groups: std::collections::HashMap<String, (RoleConfig, Vec<GuildMember>)> =
            std::collections::HashMap::new();
        let mut online_no_role: Vec<GuildMember> = Vec::new();
        let mut offline_members: Vec<GuildMember> = Vec::new();

        for member in members {
            if member.status == PresenceStatus::Offline {
                offline_members.push(member.clone());
            } else if let Some(top_role) = Self::top_role_for(member) {
                role_groups
                    .entry(top_role.name.clone())
                    .or_insert_with(|| (top_role.clone(), Vec::new()))
                    .1
                    .push(member.clone());
            } else {
                online_no_role.push(member.clone());
            }
        }

        let mut groups: Vec<MemberGroup> = Vec::new();

        // 1. Добавляем ролевые группы
        for (_, (cfg, mut mems)) in role_groups {
            mems.sort_by(|a, b| {
                let name_a = a.global_name.as_deref().unwrap_or(&a.username);
                let name_b = b.global_name.as_deref().unwrap_or(&b.username);
                name_a.cmp(name_b)
            });
            groups.push(MemberGroup {
                role_name: cfg.name,
                role_color: cfg.color,
                badge_bg: cfg.badge_bg,
                badge_border: cfg.badge_border,
                priority: cfg.priority,
                emoji: cfg.emoji,
                members: mems,
                is_offline: false,
            });
        }

        // Сортировка ролевых групп по приоритету (высокий приоритет сверху)
        groups.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.role_name.cmp(&b.role_name))
        });

        // 2. Добавляем онлайн участников без специальных ролей (В СЕТИ)
        if !online_no_role.is_empty() {
            online_no_role.sort_by(|a, b| {
                let name_a = a.global_name.as_deref().unwrap_or(&a.username);
                let name_b = b.global_name.as_deref().unwrap_or(&b.username);
                name_a.cmp(name_b)
            });
            groups.push(MemberGroup {
                role_name: "В СЕТИ".to_string(),
                role_color: DISCORD_THEME.text_muted,
                badge_bg: gpui::rgba(0xffffff10).into(),
                badge_border: DISCORD_THEME.border_subtle,
                priority: 10,
                emoji: None,
                members: online_no_role,
                is_offline: false,
            });
        }

        // 3. Добавляем офлайн участников (НЕ В СЕТИ)
        if !offline_members.is_empty() {
            offline_members.sort_by(|a, b| {
                let name_a = a.global_name.as_deref().unwrap_or(&a.username);
                let name_b = b.global_name.as_deref().unwrap_or(&b.username);
                name_a.cmp(name_b)
            });
            groups.push(MemberGroup {
                role_name: "НЕ В СЕТИ".to_string(),
                role_color: DISCORD_THEME.status_offline,
                badge_bg: gpui::rgba(0x80848e1a).into(),
                badge_border: DISCORD_THEME.border_subtle,
                priority: -1,
                emoji: None,
                members: offline_members,
                is_offline: true,
            });
        }

        groups
    }

    /// Рендеринг всплывающей карточки профиля при наведении (Profile Hover Tooltip)
    pub fn render_profile_tooltip(
        member: &GuildMember,
        top_role: Option<&RoleConfig>,
    ) -> AnyElement {
        let username = member.username.clone();
        let display_name = member
            .global_name
            .clone()
            .unwrap_or_else(|| username.clone());
        let role_color = top_role.map(|r| r.color).unwrap_or(DISCORD_THEME.blurple);
        let status = member.status;
        let is_streaming = member.activity.as_deref().map_or(false, |a| {
            let lower = a.to_lowercase();
            lower.contains("стрим") || lower.contains("stream") || lower.contains("эфир")
        });
        let st_color = Self::status_color(status, is_streaming);

        div()
            .v_flex()
            .w(px(260.))
            .bg(DISCORD_THEME.bg_secondary)
            .rounded(px(8.))
            .border_1()
            .border_color(DISCORD_THEME.border_subtle)
            .shadow_lg()
            .overflow_hidden()
            // 1. Верхний акцентный баннер с цветом роли
            .child(div().h(px(44.)).w_full().bg(role_color).opacity(0.85))
            // 2. Тело профиля (аватар с наложением на баннер, имя, статус, активность, роли)
            .child(
                div()
                    .v_flex()
                    .p(px(12.))
                    .gap(px(8.))
                    // Аватарка поверх баннера + бейдж статуса
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .justify_between()
                            .mt(px(-28.))
                            .child(
                                div()
                                    .relative()
                                    .size(px(48.))
                                    .rounded_full()
                                    .border(px(3.))
                                    .border_color(DISCORD_THEME.bg_secondary)
                                    .bg(role_color)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(18.))
                                    .child(
                                        username
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .right_0()
                                            .size(px(14.))
                                            .rounded_full()
                                            .border_2()
                                            .border_color(DISCORD_THEME.bg_secondary)
                                            .bg(st_color),
                                    ),
                            )
                            // Текстовый бейдж статуса
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded_full()
                                    .bg(DISCORD_THEME.bg_primary)
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(div().size(px(6.)).rounded_full().bg(st_color))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(status.label()),
                                    ),
                            ),
                    )
                    // Имя пользователя и тег
                    .child(
                        div()
                            .v_flex()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child(display_name),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(format!("@{}", username)),
                            ),
                    )
                    // Блок активности (если есть)
                    .when_some(member.activity.as_ref(), |this, act| {
                        this.child(
                            div()
                                .v_flex()
                                .p(px(8.))
                                .rounded(px(6.))
                                .bg(DISCORD_THEME.bg_primary)
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child(if is_streaming {
                                            "В ЭФИРЕ"
                                        } else {
                                            "АКТИВНОСТЬ"
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(DISCORD_THEME.text_header)
                                        .child(act.clone()),
                                ),
                        )
                    })
                    // Секция цветных бейджей ролей
                    .when(!member.roles.is_empty(), |this| {
                        this.child(
                            div()
                                .v_flex()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child("РОЛИ"),
                                )
                                .child(div().flex().flex_wrap().gap(px(4.)).children(
                                    member.roles.iter().map(|r| {
                                        let cfg = Self::role_config_for(r);
                                        div()
                                            .h(px(20.))
                                            .px(px(6.))
                                            .rounded(px(4.))
                                            .bg(cfg.badge_bg)
                                            .border_1()
                                            .border_color(cfg.badge_border)
                                            .flex()
                                            .items_center()
                                            .gap(px(4.))
                                            .child(div().size(px(6.)).rounded_full().bg(cfg.color))
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cfg.color)
                                                    .child(r.clone()),
                                            )
                                    }),
                                )),
                        )
                    })
                    // Подвал-подсказка
                    .child(
                        div()
                            .pt(px(4.))
                            .border_t_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .text_size(px(10.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Нажмите, чтобы открыть полный профиль"),
                    ),
            )
            .into_any_element()
    }

    /// Отрисовка строки участника с цветными бейджами, индикатором статуса и всплывающим тултипом
    fn render_member_row(
        member: &GuildMember,
        is_offline: bool,
        cx: &mut Context<AppLayout>,
    ) -> AnyElement {
        let uid = member.id.clone();
        let username = member.username.clone();
        let display_name = member
            .global_name
            .clone()
            .unwrap_or_else(|| username.clone());
        let activity = member.activity.clone();
        let status = member.status;
        let top_role = Self::top_role_for(member);

        // Цвет имени: если участник офлайн — приглушённый, иначе цвет высшей роли
        let name_color = if is_offline {
            DISCORD_THEME.text_muted
        } else if let Some(ref r) = top_role {
            r.color
        } else {
            DISCORD_THEME.text_header
        };

        // Цвет фона аватара
        let avatar_bg = if is_offline {
            DISCORD_THEME.bg_active
        } else if let Some(ref r) = top_role {
            r.color
        } else {
            DISCORD_THEME.blurple
        };

        let is_streaming = activity.as_deref().map_or(false, |a| {
            let lower = a.to_lowercase();
            lower.contains("стрим") || lower.contains("stream") || lower.contains("эфир")
        });
        let st_color = Self::status_color(status, is_streaming);

        let member_for_tooltip = member.clone();
        let top_role_for_tooltip = top_role.clone();

        div()
            .id(SharedString::from(format!("member-{}", uid)))
            .h(px(44.))
            .px(px(8.))
            .rounded(px(6.))
            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(10.))
            .when(is_offline, |this| this.opacity(0.6))
            // 1. Аватарка с индикатором статуса
            .child(
                div()
                    .relative()
                    .size(px(32.))
                    .rounded_full()
                    .bg(avatar_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::white())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(14.))
                    .child(
                        username
                            .chars()
                            .next()
                            .map(|c| c.to_string())
                            .unwrap_or_default(),
                    )
                    // Индикатор статуса (зелёный/жёлтый/красный/серый/фиолетовый)
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .right_0()
                            .size(px(10.))
                            .rounded_full()
                            .border_2()
                            .border_color(DISCORD_THEME.bg_secondary)
                            .bg(st_color)
                            // Если DND — горизонтальная полоска внутри
                            .when(status == PresenceStatus::Dnd, |this| {
                                this.flex().items_center().justify_center().child(
                                    div().w(px(6.)).h(px(2.)).bg(gpui::white()).rounded(px(1.)),
                                )
                            }),
                    ),
            )
            // 2. Информация: имя + бейдж роли + активность
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .overflow_hidden()
                    // Первая строка: имя + бейджи
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(name_color)
                                    .child(display_name),
                            )
                            // Цветной бейдж роли (Role Badge Pill)
                            .when_some(top_role.as_ref(), |this, role| {
                                this.child(
                                    div()
                                        .h(px(16.))
                                        .px(px(5.))
                                        .rounded(px(4.))
                                        .bg(role.badge_bg)
                                        .border_1()
                                        .border_color(role.badge_border)
                                        .flex()
                                        .items_center()
                                        .gap(px(3.))
                                        .when_some(role.emoji, |this, em| {
                                            this.child(div().text_size(px(9.)).child(em))
                                        })
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(role.color)
                                                .child(role.name.clone()),
                                        ),
                                )
                            }),
                    )
                    // Вторая строка: активность или статус присутствия
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child(activity.unwrap_or_else(|| status.label().into())),
                    ),
            )
            // 3. Всплывающий тултип профиля при наведении (Profile Hover Tooltip)
            .tooltip(move |window, cx| {
                let mem = member_for_tooltip.clone();
                let role = top_role_for_tooltip.clone();
                Tooltip::element(move |_win, _app| {
                    Self::render_profile_tooltip(&mem, role.as_ref())
                })
                .build(window, cx)
            })
            // 4. Клик левой кнопкой мыши для открытия полной карточки профиля
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let uid = uid.clone();
                    move |this, _, _window, cx| {
                        this.state.open_member_profile(&uid);
                        cx.notify();
                    }
                }),
            )
            .into_any_element()
    }

    /// Главный метод рендеринга боковой панели участников
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_member_sidebar_open {
            return div().into_any_element();
        }

        // Группировка участников по гильдийным ролям
        let groups = Self::group_members(&state.members);

        let mut group_elements: Vec<AnyElement> = Vec::new();

        for group in groups {
            let is_offline = group.is_offline;
            let header_title = group.role_name.to_uppercase();
            let count_str = group.members.len().to_string();
            let role_color = group.role_color;
            let badge_bg = group.badge_bg;
            let badge_border = group.badge_border;
            let emoji = group.emoji;

            let header = div()
                .pt(px(12.))
                .pb(px(4.))
                .px(px(8.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .when_some(emoji, |this, em| {
                            this.child(div().text_size(px(11.)).child(em))
                        })
                        .when(emoji.is_none(), |this| {
                            this.child(div().size(px(6.)).rounded_full().bg(role_color))
                        })
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(DISCORD_THEME.text_muted)
                                .child(header_title),
                        ),
                )
                // Бейдж с количеством участников в группе роли
                .child(
                    div()
                        .px(px(6.))
                        .py(px(1.))
                        .rounded(px(10.))
                        .bg(badge_bg)
                        .border_1()
                        .border_color(badge_border)
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(role_color)
                        .child(count_str),
                );

            let mut member_elements: Vec<AnyElement> = Vec::new();
            for member in &group.members {
                member_elements.push(Self::render_member_row(member, is_offline, cx));
            }

            group_elements.push(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .child(header)
                    .children(member_elements)
                    .into_any_element(),
            );
        }

        div()
            .v_flex()
            .w(px(240.))
            .h_full()
            .bg(DISCORD_THEME.bg_secondary)
            .border_l_1()
            .border_color(DISCORD_THEME.border_subtle)
            .p(px(8.))
            .gap(px(4.))
            .overflow_hidden()
            .children(group_elements)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GuildMember;

    #[core::prelude::v1::test]
    fn test_member_sidebar_grouping_by_guild_roles() {
        let members = vec![
            GuildMember {
                id: "u-pharos".into(),
                username: "ShadowByte".into(),
                global_name: Some("ShadowByte".into()),
                avatar: None,
                status: PresenceStatus::Online,
                activity: Some("В эфире: Hacking GPUI ⚡".into()),
                roles: vec!["Server Booster".into()],
            },
            GuildMember {
                id: "u-architect".into(),
                username: "ChiefArchitect".into(),
                global_name: Some("Chief Architect".into()),
                avatar: None,
                status: PresenceStatus::Online,
                activity: None,
                roles: vec!["Admin".into()],
            },
            GuildMember {
                id: "u-rustery".into(),
                username: "Rustery".into(),
                global_name: Some("Rustery Developer".into()),
                avatar: None,
                status: PresenceStatus::Idle,
                activity: None,
                roles: vec!["Developer".into()],
            },
            GuildMember {
                id: "u-tester".into(),
                username: "QALead".into(),
                global_name: Some("QA Lead".into()),
                avatar: None,
                status: PresenceStatus::Offline,
                activity: None,
                roles: vec!["QA".into()],
            },
        ];

        let groups = MemberSidebar::group_members(&members);
        assert_eq!(
            groups.len(),
            4,
            "Должно быть 4 группы: Admin, Server Booster, Developer, НЕ В СЕТИ"
        );

        assert_eq!(groups[0].role_name, "Admin");
        assert_eq!(groups[0].priority, 100);
        assert_eq!(groups[0].members.len(), 1);
        assert_eq!(groups[0].members[0].username, "ChiefArchitect");

        assert_eq!(groups[1].role_name, "Server Booster");
        assert_eq!(groups[1].priority, 90);
        assert_eq!(groups[1].members.len(), 1);
        assert_eq!(groups[1].members[0].username, "ShadowByte");

        assert_eq!(groups[2].role_name, "Developer");
        assert_eq!(groups[2].priority, 80);
        assert_eq!(groups[2].members.len(), 1);
        assert_eq!(groups[2].members[0].username, "Rustery");

        assert_eq!(groups[3].role_name, "НЕ В СЕТИ");
        assert_eq!(groups[3].is_offline, true);
        assert_eq!(groups[3].members.len(), 1);
        assert_eq!(groups[3].members[0].username, "QALead");
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_colored_badges_and_configs() {
        let admin_cfg = MemberSidebar::role_config_for("Admin");
        assert_eq!(admin_cfg.priority, 100);
        assert_eq!(admin_cfg.emoji, Some("👑"));

        let booster_cfg = MemberSidebar::role_config_for("Server Booster");
        assert_eq!(booster_cfg.priority, 90);
        assert_eq!(booster_cfg.emoji, Some("🚀"));

        let dev_cfg = MemberSidebar::role_config_for("Developer");
        assert_eq!(dev_cfg.priority, 80);
        assert_eq!(dev_cfg.emoji, Some("💻"));

        let qa_cfg = MemberSidebar::role_config_for("QA");
        assert_eq!(qa_cfg.priority, 60);
        assert_eq!(qa_cfg.emoji, Some("🧪"));

        // Кастомная роль имеет стабильный сгенерированный цвет и базовый приоритет 30
        let custom_cfg = MemberSidebar::role_config_for("VIP Contributor");
        assert_eq!(custom_cfg.priority, 30);
        assert_eq!(custom_cfg.emoji, None);
        assert_eq!(custom_cfg.name, "VIP Contributor");
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_online_status_indicators() {
        assert_eq!(
            MemberSidebar::status_color(PresenceStatus::Online, false),
            DISCORD_THEME.status_online
        );
        assert_eq!(
            MemberSidebar::status_color(PresenceStatus::Idle, false),
            DISCORD_THEME.status_idle
        );
        assert_eq!(
            MemberSidebar::status_color(PresenceStatus::Dnd, false),
            DISCORD_THEME.status_dnd
        );
        assert_eq!(
            MemberSidebar::status_color(PresenceStatus::Offline, false),
            DISCORD_THEME.status_offline
        );
        // Стриминг переопределяет статус на фиолетовый
        assert_eq!(
            MemberSidebar::status_color(PresenceStatus::Online, true),
            DISCORD_THEME.status_streaming
        );
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_multiple_roles_highest_priority() {
        let member = GuildMember {
            id: "u-multi".into(),
            username: "PowerUser".into(),
            global_name: Some("Power User".into()),
            avatar: None,
            status: PresenceStatus::Online,
            activity: None,
            roles: vec!["QA".into(), "Admin".into(), "Developer".into()],
        };

        let top = MemberSidebar::top_role_for(&member).unwrap();
        assert_eq!(
            top.name, "Admin",
            "Admin должен иметь наивысший приоритет (100 > 80 > 60)"
        );
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_online_users_without_roles_grouped_in_online() {
        let member = GuildMember {
            id: "u-regular".into(),
            username: "RegularUser".into(),
            global_name: None,
            avatar: None,
            status: PresenceStatus::Online,
            activity: None,
            roles: Vec::new(),
        };

        let groups = MemberSidebar::group_members(&[member]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].role_name, "В СЕТИ");
        assert_eq!(groups[0].members.len(), 1);
        assert_eq!(groups[0].members[0].username, "RegularUser");
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_empty_list_returns_zero_groups() {
        let groups = MemberSidebar::group_members(&[]);
        assert!(
            groups.is_empty(),
            "Пустой список участников возвращает 0 групп"
        );
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_all_offline_users_grouped_in_offline() {
        let members = vec![
            GuildMember {
                id: "u-1".into(),
                username: "User1".into(),
                global_name: None,
                avatar: None,
                status: PresenceStatus::Offline,
                activity: None,
                roles: vec!["Admin".into()],
            },
            GuildMember {
                id: "u-2".into(),
                username: "User2".into(),
                global_name: None,
                avatar: None,
                status: PresenceStatus::Offline,
                activity: None,
                roles: vec!["Developer".into()],
            },
        ];

        let groups = MemberSidebar::group_members(&members);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].role_name, "НЕ В СЕТИ");
        assert_eq!(groups[0].members.len(), 2);
        assert!(groups[0].is_offline);
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_sorting_within_group() {
        let members = vec![
            GuildMember {
                id: "u-z".into(),
                username: "Zeta".into(),
                global_name: Some("Zeta".into()),
                avatar: None,
                status: PresenceStatus::Online,
                activity: None,
                roles: vec!["Developer".into()],
            },
            GuildMember {
                id: "u-a".into(),
                username: "Alpha".into(),
                global_name: Some("Alpha".into()),
                avatar: None,
                status: PresenceStatus::Online,
                activity: None,
                roles: vec!["Developer".into()],
            },
        ];

        let groups = MemberSidebar::group_members(&members);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members[0].username, "Alpha");
        assert_eq!(groups[0].members[1].username, "Zeta");
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_profile_tooltip_element_generation() {
        let member = GuildMember {
            id: "u-tooltip".into(),
            username: "TooltipUser".into(),
            global_name: Some("Tooltip Display Name".into()),
            avatar: None,
            status: PresenceStatus::Online,
            activity: Some("Кодит на Rust 🦀".into()),
            roles: vec!["Admin".into(), "Developer".into()],
        };

        let role_cfg = MemberSidebar::role_config_for("Admin");
        // Генерация тултипа не должна паниковать
        let _tooltip_el = MemberSidebar::render_profile_tooltip(&member, Some(&role_cfg));
    }

    #[core::prelude::v1::test]
    fn test_member_sidebar_custom_role_hash_stability() {
        let cfg1 = MemberSidebar::role_config_for("Moderator Helper");
        let cfg2 = MemberSidebar::role_config_for("Moderator Helper");
        assert_eq!(cfg1.color, cfg2.color);
        assert_eq!(cfg1.badge_bg, cfg2.badge_bg);
        assert_eq!(cfg1.badge_border, cfg2.badge_border);
        assert_eq!(cfg1.priority, 30);
    }
}
