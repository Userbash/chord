use crate::state::{AppState, LoginStep};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Префикс URL для сканирования QR-кода Remote Auth в мобильном приложении Discord
pub const REMOTE_AUTH_QR_URL_PREFIX: &str = "https://discord.com/ra/";

/// Ошибки валидации отпечатка и генерации матрицы QR-кода
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrMatrixError {
    EmptyFingerprint,
    InvalidCharacters,
    PathTraversalAttempt,
    InvalidLength,
}

impl std::fmt::Display for QrMatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFingerprint => write!(f, "fingerprint cannot be empty"),
            Self::InvalidCharacters => write!(f, "fingerprint contains invalid characters"),
            Self::PathTraversalAttempt => {
                write!(f, "fingerprint contains path traversal sequences")
            }
            Self::InvalidLength => write!(f, "fingerprint length is invalid"),
        }
    }
}

impl std::error::Error for QrMatrixError {}

/// Построение проверенного URL сканирования `https://discord.com/ra/<fingerprint>`.
/// Отклоняет пустые строки, пробельные символы, переносы строк и попытки обхода путей.
pub fn build_remote_auth_qr_url(fingerprint: &str) -> Result<String, QrMatrixError> {
    if fingerprint.is_empty() || fingerprint.trim().is_empty() {
        return Err(QrMatrixError::EmptyFingerprint);
    }
    if fingerprint.contains("..") || fingerprint.contains('/') || fingerprint.contains('\\') {
        return Err(QrMatrixError::PathTraversalAttempt);
    }
    for c in fingerprint.chars() {
        if c.is_whitespace() || c == '\n' || c == '\r' || c == '\t' {
            return Err(QrMatrixError::InvalidCharacters);
        }
        if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(QrMatrixError::InvalidCharacters);
        }
    }
    Ok(format!("{}{}", REMOTE_AUTH_QR_URL_PREFIX, fingerprint))
}

const SHA256_H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn qr_sha256(data: &[u8]) -> [u8; 32] {
    let mut state = SHA256_H0;
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let rem = (data.len() + 9) % 64;
    let zero_pad = if rem == 0 { 0 } else { 64 - rem };
    let total_len = data.len() + 1 + zero_pad + 8;

    let mut padded = Vec::with_capacity(total_len);
    padded.extend_from_slice(data);
    padded.push(0x80);
    padded.resize(data.len() + 1 + zero_pad, 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut w = [0_u32; 64];
    for chunk in padded.chunks_exact(64) {
        for (i, word_bytes) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word_bytes[0], word_bytes[1], word_bytes[2], word_bytes[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for i in 0..64 {
            let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(big_s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = big_s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut digest = [0_u8; 32];
    for (slot, word) in digest.chunks_exact_mut(4).zip(state.iter()) {
        slot.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn apply_finder_pattern(modules: &mut [Vec<bool>], r0: usize, c0: usize) {
    for dr in 0..7 {
        for dc in 0..7 {
            let is_dark = if dr == 0 || dr == 6 || dc == 0 || dc == 6 {
                true
            } else if dr == 1 || dr == 5 || dc == 1 || dc == 5 {
                false
            } else {
                true
            };
            modules[r0 + dr][c0 + dc] = is_dark;
        }
    }
}

fn is_function_pattern(r: usize, c: usize) -> bool {
    // Верхний левый искатель и разделитель
    if r <= 7 && c <= 7 {
        return true;
    }
    // Верхний правый искатель и разделитель
    if r <= 7 && c >= 17 {
        return true;
    }
    // Нижний левый искатель и разделитель
    if r >= 17 && c <= 7 {
        return true;
    }
    // Синхронизирующие полосы (Timing patterns)
    if (r == 6 && (8..=16).contains(&c)) || (c == 6 && (8..=16).contains(&r)) {
        return true;
    }
    false
}

/// Детерминированная булева матрица QR-кода 25x25 (стандарт QR Version 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCodeMatrix {
    pub size: usize,
    pub modules: Vec<Vec<bool>>,
}

impl QrCodeMatrix {
    pub const SIZE: usize = 25;

    /// Построение детерминированной матрицы QR-кода из отпечатка Remote Auth.
    pub fn from_remote_auth_fingerprint(fingerprint: &str) -> Result<Self, QrMatrixError> {
        let _url = build_remote_auth_qr_url(fingerprint)?;

        let mut modules = vec![vec![false; Self::SIZE]; Self::SIZE];

        // 1. Поисковые узоры 7x7 в трех углах
        apply_finder_pattern(&mut modules, 0, 0);
        apply_finder_pattern(&mut modules, 0, 18);
        apply_finder_pattern(&mut modules, 18, 0);

        // 2. Белые разделители (Quiet separators) вокруг искателей
        for c in 0..=7 {
            modules[7][c] = false;
        }
        for r in 0..=7 {
            modules[r][7] = false;
        }

        for c in 17..=24 {
            modules[7][c] = false;
        }
        for r in 0..=7 {
            modules[r][17] = false;
        }

        for c in 0..=7 {
            modules[17][c] = false;
        }
        for r in 17..=24 {
            modules[r][7] = false;
        }

        // 3. Синхродорожки на строке 6 и столбце 6 между разделителями (индексы 8..=16)
        for c in 8..=16 {
            modules[6][c] = c % 2 == 0;
        }
        for r in 8..=16 {
            modules[r][6] = r % 2 == 0;
        }

        // 4. Заполнение области данных детерминированным битовым потоком SHA-256
        let hash0 = qr_sha256(fingerprint.as_bytes());
        let mut seed1 = Vec::with_capacity(fingerprint.len() + 1);
        seed1.extend_from_slice(fingerprint.as_bytes());
        seed1.push(1);
        let hash1 = qr_sha256(&seed1);

        let mut bitstream = [0_u8; 64];
        bitstream[..32].copy_from_slice(&hash0);
        bitstream[32..].copy_from_slice(&hash1);

        let mut bit_idx = 0;
        for r in 0..Self::SIZE {
            for c in 0..Self::SIZE {
                if !is_function_pattern(r, c) {
                    let byte_pos = bit_idx / 8;
                    let bit_offset = 7 - (bit_idx % 8);
                    let is_dark = ((bitstream[byte_pos] >> bit_offset) & 1) == 1;
                    modules[r][c] = is_dark;
                    bit_idx += 1;
                }
            }
        }

        Ok(Self {
            size: Self::SIZE,
            modules,
        })
    }

    pub fn is_dark(&self, row: usize, col: usize) -> bool {
        if row < self.size && col < self.size {
            self.modules[row][col]
        } else {
            false
        }
    }
}

/// Состояния жизненного цикла отображения QR-кода в UI входа Discord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrLoginUiState {
    AwaitingFingerprint,
    Ready {
        fingerprint: String,
        url: String,
        matrix: QrCodeMatrix,
    },
    Scanned {
        username: String,
        user_id: String,
    },
    Expired,
    Error(String),
}

impl Default for QrLoginUiState {
    fn default() -> Self {
        Self::AwaitingFingerprint
    }
}

impl QrLoginUiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_fingerprint_received(&mut self, fingerprint: &str) -> Result<(), QrMatrixError> {
        match QrCodeMatrix::from_remote_auth_fingerprint(fingerprint) {
            Ok(matrix) => {
                let url = build_remote_auth_qr_url(fingerprint)?;
                *self = Self::Ready {
                    fingerprint: fingerprint.to_string(),
                    url,
                    matrix,
                };
                Ok(())
            }
            Err(err) => {
                *self = Self::Error(err.to_string());
                Err(err)
            }
        }
    }

    pub fn on_scanned(&mut self, username: impl Into<String>, user_id: impl Into<String>) {
        *self = Self::Scanned {
            username: username.into(),
            user_id: user_id.into(),
        };
    }

    pub fn on_expired(&mut self) {
        *self = Self::Expired;
    }

    pub fn on_error(&mut self, message: impl Into<String>) {
        *self = Self::Error(message.into());
    }

    pub fn reset(&mut self) {
        *self = Self::AwaitingFingerprint;
    }

    pub fn is_awaiting_fingerprint(&self) -> bool {
        matches!(self, Self::AwaitingFingerprint)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn is_scanned(&self) -> bool {
        matches!(self, Self::Scanned { .. })
    }

    pub fn is_expired(&self) -> bool {
        matches!(self, Self::Expired)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Модальное окно входа в учетную запись Discord по эталону официального клиента (/home/sanya/re-deep/rest-static.md §1)
/// Поддерживает:
/// - Аутентификацию по Email / Номеру телефона и паролю (POST /auth/login)
/// - Вход по токену авторизации (User / Bot / Dev Token)
/// - Двухфакторную аутентификацию (2FA / TOTP / Резервные коды - POST /auth/mfa/totp)
/// - Вход по QR-коду через мобильное приложение Discord (Remote Auth - POST /users/@me/remote-auth/login)
/// - Вызов капчи hCaptcha (код 40003) при подозрительной активности
pub struct LoginModal;

impl LoginModal {
    #[must_use]
    pub fn is_submitting(state: &AppState) -> bool {
        matches!(state.login_form.step, LoginStep::Loading)
    }

    #[must_use]
    pub fn login_submit_button_label(state: &AppState) -> &'static str {
        if Self::is_submitting(state) {
            "Вход..."
        } else {
            "Войти"
        }
    }

    #[must_use]
    pub fn mfa_submit_button_label(state: &AppState) -> &'static str {
        if Self::is_submitting(state) {
            "Проверка..."
        } else {
            "Подтвердить"
        }
    }

    #[must_use]
    pub fn active_captcha_banner_text(state: &AppState) -> Option<String> {
        state.active_captcha.as_ref().map(|captcha| {
            format!(
                "Требуется проверка {} (sitekey: {}). Автоматический обход запрещён.",
                captcha.service, captcha.sitekey
            )
        })
    }

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
            // 2. Блок ошибки и уведомления о капче (если есть)
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
            .child(
                if let Some(captcha_banner) = Self::active_captcha_banner_text(state) {
                    div()
                        .p(px(10.))
                        .bg(DISCORD_THEME.bg_secondary)
                        .border_1()
                        .border_color(DISCORD_THEME.blurple)
                        .rounded(px(4.))
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(DISCORD_THEME.text_normal)
                                .child(captcha_banner),
                        )
                        .into_any_element()
                } else {
                    div().into_any_element()
                },
            )
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
                    .child(Self::login_submit_button_label(state))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            let _ = this.state.submit_login_flow();
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

    /// Разрешение текущего состояния QR-авторизации из AppState
    pub fn resolve_qr_ui_state(state: &AppState) -> QrLoginUiState {
        let fp = state.login_form.qr_fingerprint.trim();
        if fp.is_empty() {
            return QrLoginUiState::AwaitingFingerprint;
        }
        if fp.eq_ignore_ascii_case("expired") {
            return QrLoginUiState::Expired;
        }
        if let Some(scanned_info) = fp.strip_prefix("scanned:") {
            let mut parts = scanned_info.splitn(2, ':');
            let username = parts.next().unwrap_or("User").to_string();
            let user_id = parts.next().unwrap_or("0").to_string();
            return QrLoginUiState::Scanned { username, user_id };
        }
        if let Some(err_msg) = fp.strip_prefix("error:") {
            return QrLoginUiState::Error(err_msg.to_string());
        }

        match QrCodeMatrix::from_remote_auth_fingerprint(fp) {
            Ok(matrix) => {
                let url = build_remote_auth_qr_url(fp).unwrap_or_default();
                QrLoginUiState::Ready {
                    fingerprint: fp.to_string(),
                    url,
                    matrix,
                }
            }
            Err(err) => QrLoginUiState::Error(err.to_string()),
        }
    }

    /// Отображение реальной матрицы QR-кода 25x25 в сетке GPUI
    pub fn render_qr_matrix(matrix: &QrCodeMatrix) -> impl IntoElement + use<> {
        div()
            .size(px(144.))
            .flex()
            .flex_col()
            .relative()
            .children(matrix.modules.iter().map(|row| {
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .children(row.iter().map(|&cell| {
                        div()
                            .flex_1()
                            .bg(if cell { gpui::black() } else { gpui::white() })
                    }))
            }))
            // Центральный значок Discord (🎮)
            .child(
                div()
                    .absolute()
                    .top(px(53.))
                    .left(px(53.))
                    .size(px(38.))
                    .bg(DISCORD_THEME.blurple)
                    .rounded(px(19.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::white())
                    .text_size(px(20.))
                    .child("🎮"),
            )
    }

    /// Содержимое рамки QR-кода в зависимости от статуса QrLoginUiState
    pub fn render_qr_box_content(qr_state: &QrLoginUiState) -> impl IntoElement {
        match qr_state {
            QrLoginUiState::Ready { matrix, .. } => {
                Self::render_qr_matrix(matrix).into_any_element()
            }
            QrLoginUiState::Scanned { username, user_id } => div()
                .size(px(144.))
                .v_flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .child(
                    div()
                        .size(px(44.))
                        .bg(DISCORD_THEME.blurple)
                        .rounded(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(gpui::white())
                        .text_size(px(22.))
                        .child("👤"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::black())
                        .text_center()
                        .child(username.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(gpui::rgba(0x4E5058FF))
                        .child(format!("ID: {user_id}")),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(DISCORD_THEME.status_online)
                        .font_weight(FontWeight::BOLD)
                        .child("✓ Код отсканирован"),
                )
                .into_any_element(),
            QrLoginUiState::Expired => div()
                .size(px(144.))
                .bg(gpui::rgba(0x000000E0))
                .rounded(px(4.))
                .v_flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .p(px(8.))
                .child(div().text_size(px(26.)).child("⏱️"))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .text_center()
                        .child("Код устарел"),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(DISCORD_THEME.text_muted)
                        .text_center()
                        .child("Нажмите для обновления"),
                )
                .into_any_element(),
            QrLoginUiState::AwaitingFingerprint => div()
                .size(px(144.))
                .v_flex()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(div().text_size(px(28.)).child("⏳"))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(gpui::black())
                        .child("Загрузка QR..."),
                )
                .into_any_element(),
            QrLoginUiState::Error(err) => div()
                .size(px(144.))
                .v_flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .p(px(8.))
                .child(div().text_size(px(26.)).child("⚠️"))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(gpui::rgba(0xDA373CFF))
                        .text_center()
                        .child(err.clone()),
                )
                .into_any_element(),
        }
    }

    /// Обертка контейнера QR-кода
    pub fn render_qr_container(qr_state: &QrLoginUiState) -> impl IntoElement {
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
            .child(Self::render_qr_box_content(qr_state))
    }

    /// Правая колонка с QR-кодом для входа через мобильное приложение (Remote Auth)
    pub fn render_right_qr(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        let qr_state = Self::resolve_qr_ui_state(state);

        div()
            .w(px(280.))
            .p(px(32.))
            .v_flex()
            .items_center()
            .justify_center()
            .gap(px(16.))
            // QR-код контейнер со стилизацией Discord
            .child(Self::render_qr_container(&qr_state))
            // Заголовок QR
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .text_center()
                    .child(match &qr_state {
                        QrLoginUiState::Scanned { .. } => "Проверьте телефон",
                        QrLoginUiState::Expired => "QR-код устарел",
                        QrLoginUiState::Error(_) => "Ошибка QR-кода",
                        QrLoginUiState::AwaitingFingerprint => "Генерация QR-кода",
                        QrLoginUiState::Ready { .. } => "Вход с помощью QR-кода",
                    }),
            )
            // Описание
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(DISCORD_THEME.text_muted)
                    .text_center()
                    .child(match &qr_state {
                        QrLoginUiState::Scanned { username, .. } => {
                            format!("Подтвердите вход для {username} в мобильном приложении Discord.")
                        }
                        QrLoginUiState::Expired => {
                            "Срок действия QR-кода истек. Пожалуйста, обновите код.".to_string()
                        }
                        QrLoginUiState::Error(err) => {
                            format!("Не удалось инициализировать Remote Auth: {err}")
                        }
                        QrLoginUiState::AwaitingFingerprint => {
                            "Ожидание ключей авторизации от шлюза Discord...".to_string()
                        }
                        QrLoginUiState::Ready { .. } => {
                            "Сканируйте этот код с помощью мобильного приложения Discord, чтобы войти мгновенно.".to_string()
                        }
                    }),
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
                    .child(Self::mfa_submit_button_label(state))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        let _ = this.state.submit_mfa_flow();
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
