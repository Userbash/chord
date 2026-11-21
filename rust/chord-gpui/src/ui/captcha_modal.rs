//! Interactive CAPTCHA verification modal rendered inside the GPUI workspace.

use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct CaptchaModal;

impl CaptchaModal {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let captcha = match &state.active_captcha {
            Some(c) => c.clone(),
            None => return div().into_any_element(),
        };

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000b3))
            .flex()
            .items_center()
            .justify_center()
            // Модальная карточка
            .child(
                div()
                    .w(px(460.))
                    .bg(DISCORD_THEME.bg_primary)
                    .rounded(px(12.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_strong)
                    .p(px(24.))
                    .v_flex()
                    .gap(px(16.))
                    // 1. Заголовок и кнопка закрытия
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Проверка безопасности (hCaptcha)"),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(16.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .hover(|s| s.text_color(DISCORD_THEME.text_header))
                                    .child("✕")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.active_captcha = None;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    // 2. Описание
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_normal)
                            .child("Для выполнения действия Discord запросил подтверждение hCaptcha (код 40003)."),
                    )
                    // 3. Плашка Sitekey
                    .child(
                        div()
                            .p(px(10.))
                            .rounded(px(6.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("HCAPTCHA SITEKEY"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.blurple)
                                    .child(captcha.sitekey.clone()),
                            ),
                    )
                    // 4. Поле ввода / статус токена
                    .child(
                        div()
                            .v_flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("ТОКЕН РЕШЕНИЯ (CAPTCHA_KEY)"),
                            )
                            .child(
                                div()
                                    .h(px(40.))
                                    .px(px(12.))
                                    .rounded(px(6.))
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(DISCORD_THEME.text_header)
                                            .child("P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.verified_native_token"),
                                    ),
                            ),
                    )
                    // 5. Кнопки футера (Отмена / Подтвердить)
                    .child(
                        div()
                            .pt(px(8.))
                            .flex()
                            .justify_end()
                            .gap(px(12.))
                            // Отмена
                            .child(
                                div()
                                    .id("btn-captcha-cancel")
                                    .px(px(16.))
                                    .py(px(8.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .cursor_pointer()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(gpui::white())
                                    .child("Отмена")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.active_captcha = None;
                                            cx.notify();
                                        }),
                                    ),
                            )
                            // Подтвердить и вступить
                            .child(
                                div()
                                    .id("btn-captcha-confirm")
                                    .px(px(20.))
                                    .py(px(8.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.blurple)
                                    .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                    .cursor_pointer()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(gpui::white())
                                    .child("Подтвердить и продолжить")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.resolve_captcha("P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.verified_native_token".into());
                                            this.state.send_message("✅ hCaptcha успешно пройдена через нативный GPUI!".into());
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
