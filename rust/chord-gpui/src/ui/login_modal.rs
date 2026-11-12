use crate::state::{AppState, LoginStep};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Модальное окно входа в учетную запись Discord по эталону официального клиента (/home/sanya/re-deep/rest-static.md §1)
/// Поддерживает:
/// - Аутентификацию по Email / Номеру телефона и паролю (POST /auth/login)
/// - Вход по токену авторизации (User / Bot / Dev Token)
/// - Двухфакторную аутентификацию (2FA / TOTP / Резервные коды - POST /auth/mfa/totp)
/// - Вход по QR-коду через мобильное приложение Discord (Remote Auth - POST /users/@me/remote-auth/login)
/// - Вызов капчи hCaptcha (код 40003) при подозрительной активности
pub struct LoginModal;

impl LoginModal {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_login_modal_open {
            return div().into_any_element();
        }

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000B2)) // 70% dark overlay
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.close_login_modal();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .bg(DISCORD_THEME.bg_primary)
                    .rounded(px(8.))
                    .shadow_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .on_mouse_down(MouseButton::Left, |_e, _window, cx| {
                        cx.stop_propagation();
                    })
                    .child(if matches!(state.login_form.step, LoginStep::Mfa { .. }) {
                        Self::render_mfa_step(state, cx).into_any_element()
                    } else {
                        Self::render_standard_login(state, cx).into_any_element()
                    }),
            )
            .into_any_element()
    }

    /// Стандартное двухколоночное окно входа Discord:
    /// Слева - форма входа (Email/Pass или Token), Справа - QR-код для мобильного приложения
    fn render_standard_login(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .w(px(784.))
            .flex()
            .flex_row()
            .child(Self::render_left_form(state, cx))
            .child(
                // Разделитель между колонками
                div().w(px(1.)).bg(DISCORD_THEME.border_subtle).my(px(32.)),
            )
            .child(Self::render_right_qr(state, cx))
    }

    /// Левая колонка формы входа (Email / Пароль / Токен)
    fn render_left_form(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .flex_1()
            .p(px(32.))
            .v_flex()
            .gap(px(16.))
            // 1. Заголовки
            .child(
                div()
                    .v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child("С возвращением!"),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Мы так рады видеть вас снова!"),
                    ),
            )
            // 2. Блок ошибки (если есть)
            .child(if let Some(err) = &state.login_form.error_message {
                div()
                    .p(px(10.))
                    .bg(gpui::rgba(0xDA373C22))
                    .border_1()
                    .border_color(DISCORD_THEME.status_dnd)
                    .rounded(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_danger)
                            .child(format!("ОШИБКА: {}", err)),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            // 3. Переключатель метода входа (Email/Пароль или Токен)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if !state.login_form.is_token_mode {
                                DISCORD_THEME.blurple
                            } else {
                                DISCORD_THEME.text_muted
                            })
                            .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                            .child("Логин и пароль")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.login_form.is_token_mode = false;
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("•"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if state.login_form.is_token_mode {
                                DISCORD_THEME.blurple
                            } else {
                                DISCORD_THEME.text_muted
                            })
                            .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                            .child("Токен авторизации (Token Auth)")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.login_form.is_token_mode = true;
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // 4. Поля формы
            .child(if state.login_form.is_token_mode {
                Self::render_token_inputs(state, cx).into_any_element()
            } else {
                Self::render_credentials_inputs(state, cx).into_any_element()
            })
            // 5. Кнопка отправки формы
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .rounded(px(4.))
                    .bg(DISCORD_THEME.blurple)
                    .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::white())
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child("Вход")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            let _ = this.state.submit_login();
                            cx.notify();
                        }),
                    ),
            )
            // 6. Подвал формы (Регистрация + Триггер капчи)
            .child(
                div()
                    .v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(4.))
                            .text_size(px(13.))
                            .child(
                                div()
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("Нужна учётная запись?"),
                            )
                            .child(
                                div()
                                    .text_color(DISCORD_THEME.blurple)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                                    .child("Зарегистрироваться"),
                            ),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .hover(|s| s.text_color(DISCORD_THEME.blurple))
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("🛡️ Проверка на бота (hCaptcha код 40003)")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.trigger_captcha(
                                        "a9b5fb07-92ff-493f-86fe-352a2843b3df".into(),
                                    );
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    /// Поля ввода Email и Пароля
    fn render_credentials_inputs(
        state: &AppState,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(16.))
            // Поле: Email или номер телефона
            .child(
                div()
                    .v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("АДРЕС ЭЛЕКТРОННОЙ ПОЧТЫ ИЛИ НОМЕР ТЕЛЕФОНА *"),
                    )
                    .child(
                        div()
                            .h(px(40.))
                            .w_full()
                            .px(px(12.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .rounded(px(4.))
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(if state.login_form.email_or_phone.is_empty() {
                                        DISCORD_THEME.text_muted
                                    } else {
                                        DISCORD_THEME.text_normal
                                    })
                                    .child(if state.login_form.email_or_phone.is_empty() {
                                        "name@domain.com".to_string()
                                    } else {
                                        state.login_form.email_or_phone.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(11.))
                                    .text_color(DISCORD_THEME.blurple)
                                    .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                                    .child("Пример")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            this.state
                                                .set_login_email("alexander.dev@discord.gg".into());
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            // Поле: Пароль
            .child(
                div()
                    .v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("ПАРОЛЬ *"),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(11.))
                                    .text_color(DISCORD_THEME.blurple)
                                    .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                                    .child("Забыли пароль?"),
                            ),
                    )
                    .child(
                        div()
                            .h(px(40.))
                            .w_full()
                            .px(px(12.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .rounded(px(4.))
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(if state.login_form.password.is_empty() {
                                        DISCORD_THEME.text_muted
                                    } else {
                                        DISCORD_THEME.text_normal
                                    })
                                    .child(if state.login_form.password.is_empty() {
                                        "••••••••••••".to_string()
                                    } else if state.login_form.is_password_visible {
                                        state.login_form.password.clone()
                                    } else {
                                        "•".repeat(state.login_form.password.len().min(16))
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .child(if state.login_form.is_password_visible {
                                                "🙈"
                                            } else {
                                                "👁️"
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _window, cx| {
                                                    this.state.toggle_password_visibility();
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.blurple)
                                            .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                                            .child("Тест пароль")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _window, cx| {
                                                    this.state.set_login_password(
                                                        "SuperSecret2026!mfa".into(),
                                                    );
                                                    cx.notify();
                                                }),
                                            ),
                                    ),
                            ),
                    ),
            )
    }

    /// Поле ввода токена учётной записи (Token Auth)
    fn render_token_inputs(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_muted)
                    .child("ТОКЕН УЧЁТНОЙ ЗАПИСИ (USER / BOT / DEV TOKEN) *"),
            )
            .child(
                div()
                    .h(px(40.))
                    .w_full()
                    .px(px(12.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .rounded(px(4.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(if state.login_form.token.is_empty() {
                                DISCORD_THEME.text_muted
                            } else {
                                DISCORD_THEME.text_normal
                            })
                            .child(if state.login_form.token.is_empty() {
                                "OT... / mfa.ab12cd34ef56...".to_string()
                            } else {
                                state.login_form.token.clone()
                            }),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(11.))
                            .text_color(DISCORD_THEME.blurple)
                            .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                            .child("Вставить токен")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_login_token(
                                        "mfa.998877665544332211.token_sample_auth".into(),
                                    );
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    /// Правая колонка с QR-кодом для входа через мобильное приложение (Remote Auth)
    fn render_right_qr(_state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .w(px(280.))
            .p(px(32.))
            .v_flex()
            .items_center()
            .justify_center()
            .gap(px(16.))
            // QR-код контейнер со стилизацией Discord
            .child(
                div()
                    .size(px(168.))
                    .bg(gpui::white())
                    .rounded(px(8.))
                    .p(px(12.))
                    .shadow_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .relative()
                    // Стилизованный QR-паттерн
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(Self::qr_corner_box())
                                    .child(Self::qr_corner_box()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(Self::qr_corner_box())
                                    .child(
                                        div()
                                            .size(px(32.))
                                            .bg(gpui::black())
                                            .rounded(px(4.)),
                                    ),
                            ),
                    )
                    // Центральный значок Discord
                    .child(
                        div()
                            .absolute()
                            .size(px(38.))
                            .bg(DISCORD_THEME.blurple)
                            .rounded(px(19.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui::white())
                            .text_size(px(20.))
                            .child("🎮"),
                    ),
            )
            // Заголовок QR
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .text_center()
                    .child("Вход с помощью QR-кода"),
            )
            // Описание
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(DISCORD_THEME.text_muted)
                    .text_center()
                    .child("Сканируйте этот код с помощью мобильного приложения Discord, чтобы войти мгновенно."),
            )
            // Кнопка быстрого тестирования Remote Auth
            .child(
                div()
                    .mt(px(4.))
                    .px(px(12.))
                    .py(px(6.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .hover(|s| s.bg(DISCORD_THEME.border_subtle))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(DISCORD_THEME.text_normal)
                    .child("📲 Сканировать с телефона (Тест)")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        this.state.complete_qr_remote_auth("776655443322110099", "MobileScannedUser");
                        cx.notify();
                    })),
            )
    }

    /// Угловой маркер позиционирования QR-кода
    fn qr_corner_box() -> impl IntoElement {
        div()
            .size(px(40.))
            .border_4()
            .border_color(gpui::black())
            .rounded(px(4.))
            .p(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .child(div().size(px(18.)).bg(gpui::black()).rounded(px(2.)))
    }

    /// Шаг двухфакторной аутентификации (MFA / 2FA / TOTP)
    fn render_mfa_step(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .w(px(460.))
            .p(px(32.))
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .v_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(36.))
                            .child("🛡️"),
                    )
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child("Введите код подтверждения"),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_muted)
                            .text_center()
                            .child("Введите 6-значный код из вашего приложения для аутентификации (Google Authenticator, Authy) или 8-значный резервный код."),
                    ),
            )
            // Ошибка MFA если есть
            .child(if let Some(err) = &state.login_form.error_message {
                div()
                    .p(px(8.))
                    .bg(gpui::rgba(0xDA373C22))
                    .border_1()
                    .border_color(DISCORD_THEME.status_dnd)
                    .rounded(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_danger)
                    .child(err.clone())
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            // Поле ввода кода MFA
            .child(
                div()
                    .v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("КОД АУТЕНТИФИКАЦИИ *"),
                    )
                    .child(
                        div()
                            .h(px(44.))
                            .w_full()
                            .px(px(12.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .rounded(px(4.))
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if state.login_form.mfa_code.is_empty() {
                                        DISCORD_THEME.text_muted
                                    } else {
                                        DISCORD_THEME.text_header
                                    })
                                    .child(if state.login_form.mfa_code.is_empty() {
                                        "123 456".to_string()
                                    } else {
                                        state.login_form.mfa_code.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.blurple)
                                    .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                                    .child("Вставить 6 цифр")
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.set_login_mfa_code("849102".into());
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Кнопка подтверждения
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .rounded(px(4.))
                    .bg(DISCORD_THEME.blurple)
                    .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::white())
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child("Подтвердить")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        let _ = this.state.submit_mfa();
                        cx.notify();
                    })),
            )
            // Ссылка отмены и возврата
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(13.))
                            .text_color(DISCORD_THEME.blurple)
                            .hover(|s| s.text_color(DISCORD_THEME.blurple_hover))
                            .child("Отправить SMS"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(13.))
                            .text_color(DISCORD_THEME.text_muted)
                            .hover(|s| s.text_color(DISCORD_THEME.text_normal))
                            .child("Отмена")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                this.state.cancel_mfa();
                                cx.notify();
                            })),
                    ),
            )
    }
}
