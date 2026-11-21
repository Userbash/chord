use crate::state::{AppState, PresenceStatus};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use chord_platform::rpc_ipc::RpcActivity;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Модель отображения блока Rich Presence (`SET_ACTIVITY`, `P2-2.3`) в `UserProfileCard`.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichPresenceBlockViewModel {
    pub header_label: String,
    pub name: String,
    pub details: Option<String>,
    pub state_line: Option<String>,
    pub party_text: Option<String>,
    pub timestamps_text: Option<String>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
    pub buttons: Vec<(String, String)>,
}

impl RichPresenceBlockViewModel {
    #[must_use]
    pub fn from_rpc_activity(activity: &RpcActivity) -> Self {
        let header_label = match activity.activity_type {
            0 => "ИГРАЕТ В ИГРУ",
            1 => "СТРИМИТ",
            2 => "СЛУШАЕТ",
            3 => "СМОТРИТ",
            4 => "ПОЛЬЗОВАТЕЛЬСКИЙ СТАТУС",
            5 => "СОРЕВНУЕТСЯ В",
            _ => "АКТИВНОСТЬ",
        }
        .to_string();

        let name = activity
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Rich Presence")
            .to_string();

        let details = activity
            .details
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);

        let party_text = activity
            .party
            .as_ref()
            .and_then(|p| p.size)
            .map(|[cur, max]| format!("({cur} из {max})"));

        let raw_state = activity
            .state
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);

        let state_line = match (&raw_state, &party_text) {
            (Some(st), Some(pt)) => Some(format!("{st} {pt}")),
            (Some(st), None) => Some(st.clone()),
            (None, Some(pt)) => Some(pt.clone()),
            (None, None) => None,
        };

        let timestamps_text = activity.timestamps.as_ref().and_then(|ts| {
            let norm = ts.normalized_millis();
            match (norm.start, norm.end) {
                (Some(start), Some(end)) => {
                    let total_secs = end.saturating_sub(start) / 1000;
                    let mins = total_secs / 60;
                    let secs = total_secs % 60;
                    Some(format!("{mins:02}:{secs:02} осталось"))
                }
                (Some(start), None) => Some(format!("В процессе (с {start} мс)")),
                (None, Some(end)) => Some(format!("До {end} мс")),
                (None, None) => None,
            }
        });

        let (large_image, large_text, small_image, small_text) = match &activity.assets {
            Some(assets) => (
                assets
                    .large_image
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string),
                assets
                    .large_text
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string),
                assets
                    .small_image
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string),
                assets
                    .small_text
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string),
            ),
            None => (None, None, None, None),
        };

        let buttons = activity
            .buttons
            .iter()
            .take(chord_platform::rpc_ipc::MAX_RPC_BUTTONS)
            .map(|b| (b.label.clone(), b.url.clone()))
            .collect();

        Self {
            header_label,
            name,
            details,
            state_line,
            party_text,
            timestamps_text,
            large_image,
            large_text,
            small_image,
            small_text,
            buttons,
        }
    }

    #[must_use]
    pub fn from_member_activity_text(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(Self {
            header_label: "АКТИВНОСТЬ".to_string(),
            name: trimmed.to_string(),
            details: None,
            state_line: None,
            party_text: None,
            timestamps_text: None,
            large_image: None,
            large_text: None,
            small_image: None,
            small_text: None,
            buttons: Vec::new(),
        })
    }
}

/// Карточка пользователя (User Profile Popover)
/// Реализует окно профиля по эталону /home/sanya/re-deep/ui/06-profile-popout.html
pub struct UserProfileCard;

impl UserProfileCard {
    /// Возвращает актуальную модель блока Rich Presence для открытой карточки пользователя
    /// (или активной сессии `SET_ACTIVITY`), либо `None`, если активность очищена (`activity == None`).
    #[must_use]
    pub fn rich_presence_block(state: &AppState) -> Option<RichPresenceBlockViewModel> {
        let is_current_user_profile = match &state.selected_member_profile {
            Some(member) => {
                (!state.current_user.id.is_empty() && member.id == state.current_user.id)
                    || (!state.current_user.username.is_empty()
                        && member.username == state.current_user.username)
            }
            None => true,
        };

        if is_current_user_profile {
            if let Some(ref rpc) = state.active_rpc_activity {
                return Some(RichPresenceBlockViewModel::from_rpc_activity(rpc));
            }
            if let Some(ref member) = state.selected_member_profile
                && let Some(ref act_text) = member.activity
            {
                return RichPresenceBlockViewModel::from_member_activity_text(act_text);
            }
            return None;
        }

        if let Some(ref member) = state.selected_member_profile {
            if let Some(ref act_text) = member.activity {
                return RichPresenceBlockViewModel::from_member_activity_text(act_text);
            }
            if let Some(ref rpc) = state.active_rpc_activity {
                return Some(RichPresenceBlockViewModel::from_rpc_activity(rpc));
            }
        }

        None
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn has_active_rich_presence(state: &AppState) -> bool {
        Self::rich_presence_block(state).is_some()
    }

    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let member = match &state.selected_member_profile {
            Some(m) => m,
            None => return div().into_any_element(),
        };

        let username = member.username.clone();
        let display_name = member
            .global_name
            .clone()
            .unwrap_or_else(|| username.clone());
        let status = member.status;
        let rich_presence = Self::rich_presence_block(state);
        let roles = member.roles.clone();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.close_member_profile();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .v_flex()
                    .w(px(320.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .rounded(px(10.))
                    .shadow_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .on_mouse_down(MouseButton::Left, |_e, _window, cx| {
                        // Остановка всплытия клика внутри модалки
                        cx.stop_propagation();
                    })
                    // 1. Верхний баннер профиля (акцентный цвет Discord)
                    .child(
                        div()
                            .relative()
                            .h(px(70.))
                            .w_full()
                            .bg(gpui::rgb(0x30231c))
                            // Круглая аватарка с абсолютным позиционированием
                            .child(
                                div()
                                    .absolute()
                                    .top(px(24.))
                                    .left(px(16.))
                                    .size(px(72.))
                                    .rounded_full()
                                    .border_4()
                                    .border_color(DISCORD_THEME.bg_secondary)
                                    .bg(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(28.))
                                    .child(
                                        username
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    )
                                    // Индикатор статуса присутствия
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .right_0()
                                            .size(px(16.))
                                            .rounded_full()
                                            .border_2()
                                            .border_color(DISCORD_THEME.bg_secondary)
                                            .bg(match status {
                                                PresenceStatus::Online => {
                                                    DISCORD_THEME.status_online
                                                }
                                                PresenceStatus::Idle => DISCORD_THEME.status_idle,
                                                PresenceStatus::Dnd => DISCORD_THEME.status_dnd,
                                                PresenceStatus::Offline => {
                                                    DISCORD_THEME.status_offline
                                                }
                                            }),
                                    ),
                            ),
                    )
                    // 2. Тело карточки с отступом для накладываемой аватарки
                    .child(
                        div()
                            .v_flex()
                            .pt(px(36.))
                            .px(px(16.))
                            .pb(px(16.))
                            .gap(px(10.))
                            // Имя и тег
                            .child(
                                div()
                                    .v_flex()
                                    .child(
                                        div()
                                            .text_size(px(18.))
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
                            // Значки пользователя (Badges)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_active)
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("🏆 Rust GPUI Pioneer"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_active)
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("⚡ Early Adopter"),
                                    ),
                            )
                            // Разделитель
                            .child(div().h(px(1.)).w_full().bg(DISCORD_THEME.border_subtle))
                            // Живой блок Rich Presence (P2-2.3), скрывается при activity == None
                            .children(rich_presence.map(|block| {
                                div()
                                    .v_flex()
                                    .gap(px(4.))
                                    .p(px(8.))
                                    .rounded(px(6.))
                                    .bg(DISCORD_THEME.bg_primary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.border_subtle)
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(block.header_label),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_header)
                                            .child(block.name),
                                    )
                                    .children(block.details.map(|details| {
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child(details)
                                    }))
                                    .children(block.state_line.map(|st| {
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(st)
                                    }))
                                    .children(block.timestamps_text.map(|ts| {
                                        div()
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(ts)
                                    }))
                                    .children(block.large_text.or(block.large_image).map(
                                        |asset_caption| {
                                            div()
                                                .text_size(px(11.))
                                                .text_color(DISCORD_THEME.text_muted)
                                                .child(format!("🖼 {asset_caption}"))
                                        },
                                    ))
                                    .children(if block.buttons.is_empty() {
                                        None
                                    } else {
                                        Some(div().v_flex().gap(px(4.)).pt(px(4.)).children(
                                            block.buttons.into_iter().map(|(label, _url)| {
                                                div()
                                                    .h(px(28.))
                                                    .w_full()
                                                    .rounded(px(4.))
                                                    .bg(DISCORD_THEME.bg_active)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(DISCORD_THEME.text_header)
                                                    .child(label)
                                            }),
                                        ))
                                    })
                            }))
                            // Роли на сервере
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(format!("РОЛИ — {}", roles.len())),
                                    )
                                    .child(div().flex().flex_wrap().gap(px(4.)).children(
                                        roles.into_iter().map(|role| {
                                            div()
                                                .px(px(6.))
                                                .py(px(2.))
                                                .rounded(px(4.))
                                                .bg(DISCORD_THEME.bg_primary)
                                                .text_size(px(11.))
                                                .text_color(DISCORD_THEME.text_normal)
                                                .border_1()
                                                .border_color(DISCORD_THEME.border_subtle)
                                                .child(format!("• {}", role))
                                        }),
                                    )),
                            )
                            // Кнопка быстрого действия: Отправить сообщение
                            .child(
                                div()
                                    .h(px(34.))
                                    .w_full()
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.blurple)
                                    .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("💬 Написать сообщение")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            this.state.close_member_profile();
                                            this.state.select_guild(None); // Переход в ЛС
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
