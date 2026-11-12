use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Экран поиска и обзора публичных серверов Discord (Explore Public Servers / Discovery)
/// Соответствует оригинальному экрану официального клиента Discord (Video_2026-11-10_00-10-28.mp4, кадр 14)
pub struct ExploreView;

struct DiscoverableCommunity {
    name: &'static str,
    icon: &'static str,
    description: &'static str,
    online_count: &'static str,
    total_count: &'static str,
    banner_color: Hsla,
}

impl ExploreView {
    pub fn render(
        _state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let communities = vec![
            DiscoverableCommunity {
                name: "Marvel Rivals Official",
                icon: "🦸",
                description: "Официальное русскоязычное и глобальное сообщество супергеройского шутера Marvel Rivals!",
                online_count: "42 910 онлайн",
                total_count: "256 120 участников",
                banner_color: gpui::rgba(0x8B0000ff).into(),
            },
            DiscoverableCommunity {
                name: "Midjourney",
                icon: "⛵",
                description: "Официальный сервер генерации изображений нейросетью Midjourney.",
                online_count: "189 450 онлайн",
                total_count: "1 840 200 участников",
                banner_color: gpui::rgba(0x1E3A8Aff).into(),
            },
            DiscoverableCommunity {
                name: "Rust & GPUI Developers",
                icon: "🦀",
                description: "Сообщество разработчиков нативного клиента Discord на Rust и высокопроизводительном GPUI.",
                online_count: "6 420 онлайн",
                total_count: "48 900 участников",
                banner_color: gpui::rgba(0xB45309ff).into(),
            },
            DiscoverableCommunity {
                name: "Genshin Impact Official",
                icon: "✨",
                description: "Официальный сервер Teyvat: новости, совместные рейды, гайды и промокоды.",
                online_count: "120 500 онлайн",
                total_count: "980 400 участников",
                banner_color: gpui::rgba(0x065F46ff).into(),
            },
            DiscoverableCommunity {
                name: "Minecraft Community",
                icon: "⛏️",
                description: "Крупнейшее сообщество игроков и строителей Minecraft: серверы, моды и текстуры.",
                online_count: "210 000 онлайн",
                total_count: "1 150 000 участников",
                banner_color: gpui::rgba(0x166534ff).into(),
            },
            DiscoverableCommunity {
                name: "Honkai: Star Rail",
                icon: "🚂",
                description: "Космическая станция Первопроходцев: стратегии, арты и обсуждения сюжета.",
                online_count: "85 000 онлайн",
                total_count: "640 000 участников",
                banner_color: gpui::rgba(0x4C1D95ff).into(),
            },
        ];

        div()
            .v_flex()
            .flex_1()
            .h_full()
            .bg(DISCORD_THEME.bg_primary)
            .overflow_hidden()
            // 1. Верхний баннер Hero (по эталону Discord Linux)
            .child(
                div()
                    .h(px(200.))
                    .flex_none()
                    .w_full()
                    .bg(gpui::rgba(0x23272aff))
                    .px(px(32.))
                    .v_flex()
                    .justify_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("НАЙДИТЕ СООБЩЕСТВО СЕБЕ ПО ДУШЕ В DISCORD"),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child(
                                "От игр до музыки и обучения — здесь найдется место для каждого.",
                            ),
                    )
                    // Поле поиска сообществ
                    .child(
                        div()
                            .mt(px(8.))
                            .w(px(520.))
                            .h(px(40.))
                            .px(px(12.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .rounded(px(8.))
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("Исследуйте сообщества"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("🔍"),
                            ),
                    ),
            )
            // 2. Панель категорий
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .w_full()
                    .px(px(32.))
                    .border_b_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(Self::render_category_tab("Главная", true))
                    .child(Self::render_category_tab("Игры", false))
                    .child(Self::render_category_tab("Музыка", false))
                    .child(Self::render_category_tab("Образование", false))
                    .child(Self::render_category_tab("Наука и технологии", false))
                    .child(Self::render_category_tab("Развлечения", false)),
            )
            // 3. Сетка карточек публичных серверов
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(32.))
                    .gap(px(16.))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child("Популярные сообщества"),
                    )
                    .child(div().flex().flex_wrap().gap(px(16.)).children(
                        communities.into_iter().map(|comm| {
                            let c_name = comm.name.to_string();
                            div()
                                .w(px(280.))
                                .bg(DISCORD_THEME.bg_secondary)
                                .rounded(px(12.))
                                .border_1()
                                .border_color(DISCORD_THEME.border_subtle)
                                .overflow_hidden()
                                .v_flex()
                                .hover(|s| s.bg(DISCORD_THEME.bg_secondary_alt))
                                // Шапка карточки с цветом баннера
                                .child(
                                    div()
                                        .h(px(64.))
                                        .bg(comm.banner_color)
                                        .p(px(8.))
                                        .flex()
                                        .items_end()
                                        .child(
                                            div()
                                                .size(px(40.))
                                                .rounded(px(12.))
                                                .bg(DISCORD_THEME.bg_primary)
                                                .border_2()
                                                .border_color(DISCORD_THEME.bg_secondary)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(20.))
                                                .child(comm.icon),
                                        ),
                                )
                                // Описание карточки
                                .child(
                                    div()
                                        .p(px(12.))
                                        .v_flex()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_size(px(15.))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(DISCORD_THEME.text_header)
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(comm.name),
                                        )
                                        .child(
                                            div()
                                                .h(px(36.))
                                                .text_size(px(12.))
                                                .text_color(DISCORD_THEME.text_muted)
                                                .child(comm.description),
                                        )
                                        // Счётчики участников
                                        .child(
                                            div()
                                                .pt(px(4.))
                                                .flex()
                                                .items_center()
                                                .gap(px(12.))
                                                .text_size(px(11.))
                                                .text_color(DISCORD_THEME.text_muted)
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.))
                                                        .child(
                                                            div()
                                                                .size(px(8.))
                                                                .rounded_full()
                                                                .bg(DISCORD_THEME.status_online),
                                                        )
                                                        .child(comm.online_count),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.))
                                                        .child(
                                                            div()
                                                                .size(px(8.))
                                                                .rounded_full()
                                                                .bg(DISCORD_THEME.text_muted),
                                                        )
                                                        .child(comm.total_count),
                                                ),
                                        )
                                        // Кнопка вступления
                                        .child(
                                            div()
                                                .mt(px(8.))
                                                .w_full()
                                                .h(px(32.))
                                                .rounded(px(4.))
                                                .bg(DISCORD_THEME.blurple)
                                                .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(13.))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gpui::white())
                                                .child("Присоединиться")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.state
                                                            .join_guild_by_invite(c_name.clone());
                                                        cx.notify();
                                                    }),
                                                ),
                                        ),
                                )
                        }),
                    )),
            )
    }

    fn render_category_tab(label: &'static str, is_active: bool) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(6.))
            .rounded(px(4.))
            .bg(if is_active {
                DISCORD_THEME.bg_active
            } else {
                gpui::transparent_black()
            })
            .hover(|s| {
                if !is_active {
                    s.bg(DISCORD_THEME.bg_hover)
                } else {
                    s
                }
            })
            .cursor_pointer()
            .text_size(px(14.))
            .font_weight(FontWeight::BOLD)
            .text_color(if is_active {
                DISCORD_THEME.text_header
            } else {
                DISCORD_THEME.text_muted
            })
            .child(label)
    }
}
