use crate::state::{AppState, MessageDensity, SettingsTab, ThemeKind};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutostartToggleKind {
    OpenOnStartup,
    StartMinimized,
    MinimizeToTray,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartRowViewModel {
    pub kind: AutostartToggleKind,
    pub title: &'static str,
    pub description: &'static str,
    pub is_on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartRowsSnapshot {
    pub rows: Vec<AutostartRowViewModel>,
    pub last_error: Option<String>,
    pub is_open_on_startup: bool,
    pub is_start_minimized: bool,
    pub is_minimize_to_tray: bool,
    pub desktop_path: PathBuf,
    pub is_installed: bool,
}

pub struct SettingsDialog;

impl SettingsDialog {
    pub const AUTOSTART_SECTION_TITLE: &'static str = "НАСТРОЙКИ ЗАПУСКА (LINUX XDG AUTOSTART)";
    pub const TITLE_OPEN_ON_STARTUP: &'static str =
        "Запускать Discord при старте системы (XDG Autostart)";
    pub const TITLE_START_MINIMIZED: &'static str =
        "Запускать свёрнутым в системный трей (--start-minimized)";
    pub const TITLE_MINIMIZE_TO_TRAY: &'static str = "Сворачивать в трей при закрытии окна";

    pub fn autostart_rows_snapshot(
        controller: &crate::platform::autostart::AutostartSettingsController,
    ) -> AutostartRowsSnapshot {
        let snap = controller.snapshot();
        let rows = vec![
            AutostartRowViewModel {
                kind: AutostartToggleKind::OpenOnStartup,
                title: Self::TITLE_OPEN_ON_STARTUP,
                description: "Создаёт .desktop ярлык в ~/.config/autostart для автоматического запуска при входе в сессию.",
                is_on: snap.open_on_startup,
            },
            AutostartRowViewModel {
                kind: AutostartToggleKind::StartMinimized,
                title: Self::TITLE_START_MINIMIZED,
                description: "Запускает приложение без отображения главного окна с иконкой в системном трее.",
                is_on: snap.start_minimized,
            },
            AutostartRowViewModel {
                kind: AutostartToggleKind::MinimizeToTray,
                title: Self::TITLE_MINIMIZE_TO_TRAY,
                description: "При нажатии на крестик окно скрывается в трей вместо полного завершения процесса.",
                is_on: snap.minimize_to_tray,
            },
        ];
        AutostartRowsSnapshot {
            rows,
            last_error: snap.last_error,
            is_open_on_startup: snap.open_on_startup,
            is_start_minimized: snap.start_minimized,
            is_minimize_to_tray: snap.minimize_to_tray,
            desktop_path: snap.desktop_path,
            is_installed: snap.desktop_installed,
        }
    }

    pub fn apply_autostart_toggle(
        controller: &mut crate::platform::autostart::AutostartSettingsController,
        kind: AutostartToggleKind,
        explicit_value: Option<bool>,
    ) -> Result<
        crate::platform::autostart::AutostartSettingsSnapshot,
        crate::platform::autostart::AutostartError,
    > {
        match kind {
            AutostartToggleKind::OpenOnStartup => {
                let target = explicit_value.unwrap_or(!controller.open_on_startup());
                controller.set_open_on_startup(target)?;
            }
            AutostartToggleKind::StartMinimized => {
                let target = explicit_value.unwrap_or(!controller.start_minimized());
                controller.set_start_minimized(target)?;
            }
            AutostartToggleKind::MinimizeToTray => {
                let target = explicit_value.unwrap_or(!controller.minimize_to_tray());
                controller.set_minimize_to_tray(target)?;
            }
        }
        Ok(controller.snapshot())
    }

    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_settings_open {
            return div().into_any_element();
        }

        div()
            .absolute()
            .inset_0()
            .bg(DISCORD_THEME.bg_primary)
            .flex()
            .flex_row()
            // 1. Сайдбар категорий настроек (240px)
            .child(Self::render_sidebar(state, cx))
            // 2. Центральная область с содержимым выбранной вкладки
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .h_full()
                    .p(px(40.))
                    .gap(px(24.))
                    .overflow_hidden()
                    .child(match state.settings_tab {
                        SettingsTab::VoiceAndVideo => {
                            Self::render_voice_video_tab(state, cx).into_any_element()
                        }
                        SettingsTab::Appearance => {
                            Self::render_appearance_tab(state, cx).into_any_element()
                        }
                        SettingsTab::MyAccount => {
                            Self::render_my_account_tab(state, cx).into_any_element()
                        }
                        SettingsTab::Profile => {
                            Self::render_profile_tab(state, cx).into_any_element()
                        }
                        SettingsTab::Keybinds => {
                            Self::render_keybinds_tab(state, cx).into_any_element()
                        }
                        SettingsTab::Notifications => {
                            Self::render_notifications_tab(state, cx).into_any_element()
                        }
                        SettingsTab::TextAndImages => {
                            Self::render_text_images_tab(state, cx).into_any_element()
                        }
                        SettingsTab::Language => {
                            Self::render_language_tab(state, cx).into_any_element()
                        }
                    }),
            )
            // 3. Правая кнопка закрытия (ESC)
            .child(Self::render_close_button(cx))
            .into_any_element()
    }

    /// Сайдбар навигации по категориям настроек
    fn render_sidebar(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .v_flex()
            .w(px(240.))
            .h_full()
            .bg(DISCORD_THEME.bg_secondary)
            .border_r_1()
            .border_color(DISCORD_THEME.border_subtle)
            .p(px(20.))
            .gap(px(4.))
            .overflow_hidden()
            // Категория: Настройки пользователя
            .child(
                div()
                    .pb(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_muted)
                    .child("НАСТРОЙКИ ПОЛЬЗОВАТЕЛЯ"),
            )
            .child(Self::render_sidebar_tab(
                "Моя учётная запись",
                SettingsTab::MyAccount,
                state.settings_tab == SettingsTab::MyAccount,
                cx,
            ))
            .child(Self::render_sidebar_tab(
                "Профиль пользователя",
                SettingsTab::Profile,
                state.settings_tab == SettingsTab::Profile,
                cx,
            ))
            // Категория: Настройки приложения
            .child(
                div()
                    .pt(px(16.))
                    .pb(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_muted)
                    .child("НАСТРОЙКИ ПРИЛОЖЕНИЯ"),
            )
            .child(Self::render_sidebar_tab(
                "Голос и видео (CPAL)",
                SettingsTab::VoiceAndVideo,
                state.settings_tab == SettingsTab::VoiceAndVideo,
                cx,
            ))
            .child(Self::render_sidebar_tab(
                "Внешний вид",
                SettingsTab::Appearance,
                state.settings_tab == SettingsTab::Appearance,
                cx,
            ))
            .child(Self::render_sidebar_tab(
                "Горячие клавиши",
                SettingsTab::Keybinds,
                state.settings_tab == SettingsTab::Keybinds,
                cx,
            ))
            .child(Self::render_sidebar_tab(
                "Уведомления",
                SettingsTab::Notifications,
                state.settings_tab == SettingsTab::Notifications,
                cx,
            ))
            .child(Self::render_sidebar_tab(
                "Текст и изображения",
                SettingsTab::TextAndImages,
                state.settings_tab == SettingsTab::TextAndImages,
                cx,
            ))
            .child(Self::render_sidebar_tab(
                "Язык интерфейса",
                SettingsTab::Language,
                state.settings_tab == SettingsTab::Language,
                cx,
            ))
            // Разделитель и выход
            .child(div().my(px(12.)).h(px(1.)).bg(DISCORD_THEME.border_subtle))
            .child(
                div()
                    .h(px(32.))
                    .px(px(10.))
                    .rounded(px(4.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .flex()
                    .items_center()
                    .text_size(px(14.))
                    .text_color(DISCORD_THEME.text_danger)
                    .cursor_pointer()
                    .child("🚪 Выйти из аккаунта")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.logout();
                            cx.notify();
                        }),
                    ),
            )
            // Версия сборки
            .child(
                div()
                    .pt(px(20.))
                    .text_size(px(11.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child("discord-gpui v0.1.0 (CPAL Audio)"),
            )
    }

    /// Элемент вкладки сайдбара
    fn render_sidebar_tab(
        label: &'static str,
        tab: SettingsTab,
        is_active: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .h(px(32.))
            .px(px(10.))
            .rounded(px(4.))
            .cursor_pointer()
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
            .flex()
            .items_center()
            .text_size(px(14.))
            .font_weight(if is_active {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            })
            .text_color(if is_active {
                gpui::white()
            } else {
                DISCORD_THEME.text_normal
            })
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.state.select_settings_tab(tab);
                    cx.notify();
                }),
            )
    }

    /// Вкладка «Голос и видео (CPAL Audio Stack)»
    fn render_voice_video_tab(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        let input_devices = state.available_audio_inputs.clone();
        let output_devices = state.available_audio_outputs.clone();
        let current_input = state
            .selected_audio_input
            .clone()
            .or_else(|| state.available_audio_inputs.first().cloned())
            .unwrap_or_else(|| "Системный микрофон по умолчанию (CPAL)".into());
        let current_output = state
            .selected_audio_output
            .clone()
            .or_else(|| state.available_audio_outputs.first().cloned())
            .unwrap_or_else(|| "Системные динамики по умолчанию (CPAL)".into());

        div()
            .v_flex()
            .gap(px(24.))
            // Заголовок и кнопка обновления устройств
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("НАСТРОЙКИ ГОЛОСА (CPAL AUDIO STACK)"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("Прямое взаимодействие с аудиосистемой ALSA / PulseAudio / PipeWire через CPAL 0.15"),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-refresh-audio")
                            .cursor_pointer()
                            .px(px(12.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("🔄 Обновить аудиоустройства")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.refresh_audio_devices();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // Блок Устройств ввода и вывода CPAL
            .child(
                div()
                    .flex()
                    .gap(px(20.))
                    // Устройство ввода (Микрофон)
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("УСТРОЙСТВО ВВОДА (МИКРОФОН CPAL)"),
                            )
                            .child(
                                div()
                                    .id("dropdown-audio-input")
                                    .h(px(40.))
                                    .px(px(12.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.border_subtle)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(DISCORD_THEME.text_header)
                                            .child(current_input),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(if state.is_input_dropdown_open { "▲" } else { "▼" }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.toggle_input_dropdown();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .children(if state.is_input_dropdown_open {
                                Some(
                                    div()
                                        .v_flex()
                                        .bg(DISCORD_THEME.bg_secondary)
                                        .border_1()
                                        .border_color(DISCORD_THEME.border_subtle)
                                        .rounded(px(4.))
                                        .p(px(4.))
                                        .gap(px(2.))
                                        .children(input_devices.iter().map(|dev| {
                                            let dev_name = dev.clone();
                                            let is_sel = state.selected_audio_input.as_deref() == Some(dev.as_str());
                                            div()
                                                .cursor_pointer()
                                                .px(px(8.))
                                                .py(px(6.))
                                                .rounded(px(4.))
                                                .bg(if is_sel { DISCORD_THEME.bg_active } else { gpui::transparent_black() })
                                                .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                                .text_size(px(13.))
                                                .text_color(if is_sel { gpui::white() } else { DISCORD_THEME.text_normal })
                                                .child(format!("🎤 {}", dev_name))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.state.select_audio_input(dev_name.clone());
                                                        cx.notify();
                                                    }),
                                                )
                                        })),
                                )
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("ГРОМКОСТЬ МИКРОФОНА")
                                    .child(format!("{}%", (state.input_volume * 100.0).round() as u32)),
                            )
                            .child(
                                div()
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .child(
                                        div()
                                            .w(px(220. * state.input_volume.clamp(0.0, 1.0)))
                                            .h_full()
                                            .rounded_full()
                                            .bg(DISCORD_THEME.blurple),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .child(Self::render_vol_button("0%", 0.0, true, cx))
                                    .child(Self::render_vol_button("25%", 0.25, true, cx))
                                    .child(Self::render_vol_button("50%", 0.50, true, cx))
                                    .child(Self::render_vol_button("75%", 0.75, true, cx))
                                    .child(Self::render_vol_button("100%", 1.0, true, cx)),
                            ),
                    )
                    // Устройство вывода (Динамики/Наушники)
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("УСТРОЙСТВО ВЫВОДА (ДИНАМИКИ CPAL)"),
                            )
                            .child(
                                div()
                                    .id("dropdown-audio-output")
                                    .h(px(40.))
                                    .px(px(12.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.border_subtle)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(DISCORD_THEME.text_header)
                                            .child(current_output),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(if state.is_output_dropdown_open { "▲" } else { "▼" }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.toggle_output_dropdown();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .children(if state.is_output_dropdown_open {
                                Some(
                                    div()
                                        .v_flex()
                                        .bg(DISCORD_THEME.bg_secondary)
                                        .border_1()
                                        .border_color(DISCORD_THEME.border_subtle)
                                        .rounded(px(4.))
                                        .p(px(4.))
                                        .gap(px(2.))
                                        .children(output_devices.iter().map(|dev| {
                                            let dev_name = dev.clone();
                                            let is_sel = state.selected_audio_output.as_deref() == Some(dev.as_str());
                                            div()
                                                .cursor_pointer()
                                                .px(px(8.))
                                                .py(px(6.))
                                                .rounded(px(4.))
                                                .bg(if is_sel { DISCORD_THEME.bg_active } else { gpui::transparent_black() })
                                                .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                                .text_size(px(13.))
                                                .text_color(if is_sel { gpui::white() } else { DISCORD_THEME.text_normal })
                                                .child(format!("🎧 {}", dev_name))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.state.select_audio_output(dev_name.clone());
                                                        cx.notify();
                                                    }),
                                                )
                                        })),
                                )
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("ГРОМКОСТЬ ЗВУКА (ДИАПАЗОН 0–200%)")
                                    .child(format!("{}%", (state.output_volume * 100.0).round() as u32)),
                            )
                            .child(
                                div()
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .child(
                                        div()
                                            .w(px(220. * (state.output_volume / 2.0).clamp(0.0, 1.0)))
                                            .h_full()
                                            .rounded_full()
                                            .bg(DISCORD_THEME.status_online),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .child(Self::render_vol_button("50%", 0.5, false, cx))
                                    .child(Self::render_vol_button("100%", 1.0, false, cx))
                                    .child(Self::render_vol_button("150%", 1.5, false, cx))
                                    .child(Self::render_vol_button("200%", 2.0, false, cx)),
                            ),
                    ),
            )
            // Блок Анимированного визуализатора проверки микрофона (Animated Mic Test Visualizer)
            .child(Self::render_animated_mic_visualizer(state, cx))
            // Блок режима ввода (Voice Activity vs Push-to-Talk)
            .child(
                div()
                    .v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("РЕЖИМ ВВОДА"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(16.))
                            .child(
                                div()
                                    .flex_1()
                                    .p(px(12.))
                                    .rounded(px(6.))
                                    .bg(if !state.is_ptt_enabled {
                                        DISCORD_THEME.bg_active
                                    } else {
                                        DISCORD_THEME.bg_secondary
                                    })
                                    .border_1()
                                    .border_color(if !state.is_ptt_enabled {
                                        DISCORD_THEME.blurple
                                    } else {
                                        DISCORD_THEME.border_subtle
                                    })
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().text_size(px(14.)).child(if !state.is_ptt_enabled { "🔘" } else { "⚪" }))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_header)
                                            .child("Активация по голосу (VAD)"),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.set_input_mode(false);
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p(px(12.))
                                    .rounded(px(6.))
                                    .bg(if state.is_ptt_enabled {
                                        DISCORD_THEME.bg_active
                                    } else {
                                        DISCORD_THEME.bg_secondary
                                    })
                                    .border_1()
                                    .border_color(if state.is_ptt_enabled {
                                        DISCORD_THEME.blurple
                                    } else {
                                        DISCORD_THEME.border_subtle
                                    })
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().text_size(px(14.)).child(if state.is_ptt_enabled { "🔘" } else { "⚪" }))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_header)
                                            .child("Режим рации (Push-to-Talk)"),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.set_input_mode(true);
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            // Блок расширенной обработки звука (Noise Suppression, AEC, AGC)
            .child(
                div()
                    .v_flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ОБРАБОТКА И УЛУЧШЕНИЕ ГОЛОСА"),
                    )
                    // Krisp / RNNoise
                    .child(Self::render_toggle_row(
                        "Нейросетевое шумоподавление (Krisp / RNNoise)",
                        "Удаляет фоновый шум клавиатуры, клики мыши и вентиляторы при сохранении чёткости голоса.",
                        state.is_krisp_enabled,
                        |this, cx| {
                            this.state.toggle_krisp();
                            cx.notify();
                        },
                        cx,
                    ))
                    // Эхоподавление (AEC)
                    .child(Self::render_toggle_row(
                        "Эхоподавление (AEC)",
                        "Предотвращает передачу звуков из динамиков обратно в микрофон.",
                        state.echo_cancellation,
                        |this, cx| {
                            this.state.toggle_echo_cancellation();
                            cx.notify();
                        },
                        cx,
                    ))
                    // Автоматическая регулировка усиления (AGC)
                    .child(Self::render_toggle_row(
                        "Автоматическая регулировка усиления (AGC)",
                        "Выравнивает громкость тихого и громкого шёпота/речи.",
                        state.auto_gain_control,
                        |this, cx| {
                            this.state.toggle_auto_gain_control();
                            cx.notify();
                        },
                        cx,
                    ))
                    // Шумоподавление (Noise Reduction)
                    .child(Self::render_toggle_row(
                        "Фильтрация статических шумов",
                        "Отсекает постоянный белый шум и гул сети питания 50/60 Гц.",
                        state.noise_reduction,
                        |this, cx| {
                            this.state.toggle_noise_reduction();
                            cx.notify();
                        },
                        cx,
                    )),
            )
    }

    /// Анимированный визуализатор проверки микрофона с 16-полосным эквалайзером
    fn render_animated_mic_visualizer(
        state: &AppState,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let bars = state.get_mic_test_bars();
        let is_active = state.is_mic_test_active;

        div()
            .p(px(16.))
            .rounded(px(8.))
            .bg(DISCORD_THEME.bg_secondary)
            .border_1()
            .border_color(if is_active {
                DISCORD_THEME.blurple
            } else {
                DISCORD_THEME.border_subtle
            })
            .v_flex()
            .gap(px(14.))
            // Верхняя строка с кнопкой
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("ПРОВЕРКА МИКРОФОНА (ANIMATED VISUALIZER)"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(if is_active {
                                        "🟢 Микрофон слушает: говорите в микрофон..."
                                    } else {
                                        "Нажмите «Проверить микрофон», чтобы увидеть динамический спектр."
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-mic-test")
                            .cursor_pointer()
                            .px(px(16.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(if is_active {
                                DISCORD_THEME.status_dnd
                            } else {
                                DISCORD_THEME.blurple
                            })
                            .hover(|s| {
                                if is_active {
                                    s.bg(DISCORD_THEME.status_dnd_hover)
                                } else {
                                    s.bg(DISCORD_THEME.blurple_hover)
                                }
                            })
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child(if is_active {
                                "⏹ Остановить проверку"
                            } else {
                                "▶ Проверить микрофон"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_mic_test();
                                    this.state.step_mic_test_animation();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // 16-полосный динамический эквалайзер / VU-meter
            .child(
                div()
                    .h(px(54.))
                    .px(px(12.))
                    .rounded(px(6.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .flex()
                    .items_end()
                    .justify_between()
                    .gap(px(4.))
                    .children(bars.iter().enumerate().map(|(idx, &val)| {
                        let bar_height = if is_active {
                            (val * 46.0).max(6.0)
                        } else {
                            4.0
                        };
                        // Цветовой градиент от зеленого к желтому и красному при перегрузке
                        let bar_color = if !is_active {
                            DISCORD_THEME.text_muted
                        } else if idx >= 13 {
                            DISCORD_THEME.status_dnd
                        } else if idx >= 9 {
                            DISCORD_THEME.status_idle
                        } else {
                            DISCORD_THEME.status_online
                        };

                        div()
                            .flex_1()
                            .h(px(bar_height))
                            .rounded_t(px(2.))
                            .bg(bar_color)
                    })),
            )
            // Шкала децибел и порог чувствительности
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(10.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child("-60 dB")
                    .child("-40 dB")
                    .child("-20 dB")
                    .child("-10 dB")
                    .child("0 dB (PEAK)"),
            )
            // Ползунок чувствительности микрофона
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_normal)
                            .child("Автоматически определять чувствительность"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .w(px(38.))
                            .h(px(20.))
                            .rounded_full()
                            .bg(if state.mic_sensitivity_auto {
                                DISCORD_THEME.status_online
                            } else {
                                DISCORD_THEME.bg_tertiary
                            })
                            .p(px(2.))
                            .flex()
                            .justify_end()
                            .child(div().size(px(16.)).rounded_full().bg(gpui::white()))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_mic_sensitivity_auto();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    /// Вкладка «Внешний вид (Theme & Density Switcher)»
    fn render_appearance_tab(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(24.))
            // Заголовок
            .child(
                div()
                    .v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child("ВНЕШНИЙ ВИД (THEME & DENSITY SWITCHER)"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Настройка темы оформления, масштаба и плотности отображения сообщений"),
                    ),
            )
            // 1. Выбор темы оформления (Theme Switcher)
            .child(
                div()
                    .v_flex()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ТЕМА ОФОРМЛЕНИЯ (THEME)"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(Self::render_theme_card(
                                ThemeKind::Dark,
                                "Тёмная",
                                "Классика Discord",
                                state.selected_theme == ThemeKind::Dark,
                                cx,
                            ))
                            .child(Self::render_theme_card(
                                ThemeKind::Light,
                                "Светлая",
                                "Контрастный белый",
                                state.selected_theme == ThemeKind::Light,
                                cx,
                            ))
                            .child(Self::render_theme_card(
                                ThemeKind::Midnight,
                                "Полночь",
                                "OLED / AMOLED (#000)",
                                state.selected_theme == ThemeKind::Midnight,
                                cx,
                            ))
                            .child(Self::render_theme_card(
                                ThemeKind::System,
                                "Система",
                                "Синхронизация с ОС",
                                state.selected_theme == ThemeKind::System,
                                cx,
                            )),
                    ),
            )
            // 2. Интерактивный предпросмотр сообщений (Live Preview)
            .child(
                div()
                    .v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ПРЕДПРОСМОТР ОТОБРАЖЕНИЯ (LIVE PREVIEW)"),
                    )
                    .child(Self::render_live_chat_preview(state)),
            )
            // 3. Выбор плотности сообщений (Density Switcher)
            .child(
                div()
                    .v_flex()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ПЛОТНОСТЬ ОТОБРАЖЕНИЯ СООБЩЕНИЙ (MESSAGE DENSITY)"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(16.))
                            .child(Self::render_density_card(
                                MessageDensity::Cozy,
                                "Уютный (Cozy)",
                                "Отображает аватарки 40x40px, удобный комфортный межстрочный интервал.",
                                "👤",
                                state.message_density == MessageDensity::Cozy,
                                cx,
                            ))
                            .child(Self::render_density_card(
                                MessageDensity::Compact,
                                "Компактный (Compact)",
                                "Сообщения в одну строку без аватарок (IRC-стиль) для максимальной вместимости.",
                                "≡",
                                state.message_density == MessageDensity::Compact,
                                cx,
                            )),
                    ),
            )
            // 4. Масштаб шрифта и интерфейса
            .child(
                div()
                    .v_flex()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("МАСШТАБ ШРИФТА ЧАТА"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.blurple)
                                    .child(format!("{}%", (state.font_scale * 100.0).round() as u32)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(Self::render_font_scale_button("85% (12px)", 0.85, (state.font_scale - 0.85).abs() < 1e-4, cx))
                            .child(Self::render_font_scale_button("90% (14px)", 0.90, (state.font_scale - 0.90).abs() < 1e-4, cx))
                            .child(Self::render_font_scale_button("100% (16px Стандарт)", 1.0, (state.font_scale - 1.0).abs() < 1e-4, cx))
                            .child(Self::render_font_scale_button("115% (18px)", 1.15, (state.font_scale - 1.15).abs() < 1e-4, cx))
                            .child(Self::render_font_scale_button("125% (20px)", 1.25, (state.font_scale - 1.25).abs() < 1e-4, cx)),
                    ),
            )
            // 5. Специальные возможности (Accessibility)
            .child(
                div()
                    .v_flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("СПЕЦИАЛЬНЫЕ ВОЗМОЖНОСТИ И ДОСТУПНОСТЬ"),
                    )
                    .child(Self::render_toggle_row(
                        "Уменьшение движения (Reduced Motion)",
                        "Отключает динамические анимации интерфейса и визуализаторов.",
                        state.reduced_motion,
                        |this, cx| {
                            this.state.toggle_reduced_motion();
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(Self::render_toggle_row(
                        "Повышенная контрастность (WCAG AAA)",
                        "Увеличивает контрастность текста и границ элементов до соотношения 7.0:1.",
                        state.high_contrast,
                        |this, cx| {
                            this.state.toggle_high_contrast();
                            cx.notify();
                        },
                        cx,
                    )),
            )
            // 6. Настройки запуска (Linux XDG Autostart & System Tray)
            .child(Self::render_autostart_section(cx))
    }

    /// Секция настроек автозапуска и системного трея Linux XDG
    fn render_autostart_section(cx: &mut Context<AppLayout>) -> impl IntoElement {
        let (open_on_startup, start_minimized, minimize_to_tray, last_error) = {
            let ctrl_arc = crate::platform::autostart::global_autostart_controller();
            let ctrl = ctrl_arc.lock().expect("autostart controller lock");
            (
                ctrl.open_on_startup(),
                ctrl.start_minimized(),
                ctrl.minimize_to_tray(),
                ctrl.last_error().map(str::to_string),
            )
        };

        div()
            .v_flex()
            .gap(px(12.))
            .child(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child(Self::AUTOSTART_SECTION_TITLE),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Интеграция с рабочим столом Linux: автозапуск через XDG Desktop Entry и поведение трея"),
                    ),
            )
            .children(last_error.map(|err| {
                div()
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .border_1()
                    .border_color(DISCORD_THEME.text_danger)
                    .text_size(px(12.))
                    .text_color(DISCORD_THEME.text_danger)
                    .child(format!("Ошибка автозапуска: {err}"))
            }))
            // 1. Запускать Discord при старте системы (XDG Autostart)
            .child(Self::render_toggle_row(
                Self::TITLE_OPEN_ON_STARTUP,
                "Создаёт .desktop ярлык в ~/.config/autostart для автоматического запуска при входе в сессию.",
                open_on_startup,
                |_this, cx| {
                    let arc = crate::platform::autostart::global_autostart_controller();
                    if let Ok(mut ctrl) = arc.lock() {
                        let _ = ctrl.toggle_open_on_startup();
                    }
                    cx.notify();
                },
                cx,
            ))
            // 2. Запускать свёрнутым в системный трей (--start-minimized)
            .child(Self::render_toggle_row(
                Self::TITLE_START_MINIMIZED,
                "Запускает приложение без отображения главного окна с иконкой в системном трее.",
                start_minimized,
                |_this, cx| {
                    let arc = crate::platform::autostart::global_autostart_controller();
                    if let Ok(mut ctrl) = arc.lock() {
                        let _ = ctrl.toggle_start_minimized();
                    }
                    cx.notify();
                },
                cx,
            ))
            // 3. Сворачивать в трей при закрытии окна
            .child(Self::render_toggle_row(
                Self::TITLE_MINIMIZE_TO_TRAY,
                "При нажатии на крестик окно скрывается в трей вместо полного завершения процесса.",
                minimize_to_tray,
                |_this, cx| {
                    let arc = crate::platform::autostart::global_autostart_controller();
                    if let Ok(mut ctrl) = arc.lock() {
                        let _ = ctrl.toggle_minimize_to_tray();
                    }
                    cx.notify();
                },
                cx,
            ))
    }

    /// Карточка выбора темы оформления
    fn render_theme_card(
        theme: ThemeKind,
        name: &'static str,
        desc: &'static str,
        is_active: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .p(px(14.))
            .rounded(px(8.))
            .cursor_pointer()
            .bg(DISCORD_THEME.bg_secondary)
            .border_2()
            .border_color(if is_active {
                DISCORD_THEME.blurple
            } else {
                DISCORD_THEME.border_subtle
            })
            .hover(|s| {
                if !is_active {
                    s.border_color(DISCORD_THEME.text_muted)
                } else {
                    s
                }
            })
            .v_flex()
            .gap(px(8.))
            // Цветовой образец темы
            .child(
                div()
                    .h(px(40.))
                    .rounded(px(4.))
                    .bg(match theme {
                        ThemeKind::Dark => rgb(0x313338),
                        ThemeKind::Light => rgb(0xf2f3f5),
                        ThemeKind::Midnight => rgb(0x000000),
                        ThemeKind::System => rgb(0x2b2d31),
                    })
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(16.))
                            .child(if is_active { "✓" } else { "" }),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_active {
                        DISCORD_THEME.blurple
                    } else {
                        DISCORD_THEME.text_header
                    })
                    .child(name),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child(desc),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.state.set_theme(theme);
                    cx.notify();
                }),
            )
    }

    /// Карточка выбора плотности сообщений
    fn render_density_card(
        density: MessageDensity,
        name: &'static str,
        desc: &'static str,
        icon: &'static str,
        is_active: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .p(px(16.))
            .rounded(px(8.))
            .cursor_pointer()
            .bg(DISCORD_THEME.bg_secondary)
            .border_2()
            .border_color(if is_active {
                DISCORD_THEME.blurple
            } else {
                DISCORD_THEME.border_subtle
            })
            .v_flex()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(format!("{} {}", icon, name)),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.blurple)
                            .child(if is_active { "🔘" } else { "⚪" }),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child(desc),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.state.set_density(density);
                    cx.notify();
                }),
            )
    }

    /// Живой интерактивный предпросмотр сообщений в зависимости от темы и плотности
    fn render_live_chat_preview(state: &AppState) -> impl IntoElement {
        let is_compact = state.message_density == MessageDensity::Compact;
        let preview_bg = match state.selected_theme {
            ThemeKind::Dark => rgb(0x313338),
            ThemeKind::Light => rgb(0xffffff),
            ThemeKind::Midnight => rgb(0x000000),
            ThemeKind::System => rgb(0x2b2d31),
        };
        let preview_text = match state.selected_theme {
            ThemeKind::Light => rgb(0x2e3338),
            _ => rgb(0xdbdee1),
        };
        let preview_header = match state.selected_theme {
            ThemeKind::Light => rgb(0x060607),
            _ => rgb(0xf2f3f5),
        };

        div()
            .p(px(16.))
            .rounded(px(8.))
            .bg(preview_bg)
            .border_1()
            .border_color(DISCORD_THEME.border_subtle)
            .v_flex()
            .gap(if is_compact { px(6.) } else { px(12.) })
            // Сообщение 1
            .child(if is_compact {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("12:45"),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(preview_header)
                            .child("Rustery:"),
                    )
                    .child(
                        div().text_color(preview_text).child(
                            "Привет всем! Новые настройки в discord-gpui работают отлично! 🦀",
                        ),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex()
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
                            .child("R"),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(preview_header)
                                            .child("Rustery"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("Сегодня в 12:45"),
                                    ),
                            )
                            .child(div().text_size(px(14.)).text_color(preview_text).child(
                                "Привет всем! Новые настройки в discord-gpui работают отлично! 🦀",
                            )),
                    )
                    .into_any_element()
            })
            // Сообщение 2
            .child(if is_compact {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("12:46"),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.blurple)
                            .child("DeepSeek:"),
                    )
                    .child(
                        div().text_color(preview_text).child(
                            "CPAL аудио-движок и анимированный визуализатор готовы к релизу.",
                        ),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .size(px(40.))
                            .rounded_full()
                            .bg(DISCORD_THEME.status_online)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui::white())
                            .font_weight(FontWeight::BOLD)
                            .child("D"),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(preview_header)
                                            .child("DeepSeek Reviewer"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("Сегодня в 12:46"),
                                    ),
                            )
                            .child(div().text_size(px(14.)).text_color(preview_text).child(
                                "CPAL аудио-движок и анимированный визуализатор готовы к релизу.",
                            )),
                    )
                    .into_any_element()
            })
    }

    /// Вкладка «Моя учётная запись»
    fn render_my_account_tab(state: &AppState, _cx: &mut Context<AppLayout>) -> impl IntoElement {
        let user = &state.current_user;
        let initial = user.username.chars().next().unwrap_or('U').to_string();
        let username_str = user.username.clone();

        div()
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child("МОЯ УЧЁТНАЯ ЗАПИСЬ"),
            )
            // Карточка профиля
            .child(
                div()
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .overflow_hidden()
                    .child(div().h(px(80.)).bg(DISCORD_THEME.blurple))
                    .child(
                        div()
                            .p(px(16.))
                            .v_flex()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_end()
                                    .child(
                                        div()
                                            .size(px(64.))
                                            .rounded_full()
                                            .bg(DISCORD_THEME.bg_active)
                                            .border_4()
                                            .border_color(DISCORD_THEME.bg_secondary)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(28.))
                                            .text_color(gpui::white())
                                            .child(initial),
                                    )
                                    .child(
                                        div()
                                            .px(px(12.))
                                            .py(px(6.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.blurple)
                                            .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::white())
                                            .child("Редактировать профиль"),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_header)
                                            .child(username_str),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(format!("ID: {}", user.id)),
                                    ),
                            ),
                    ),
            )
            // Безопасность и данные аккаунта
            .child(
                div()
                    .p(px(16.))
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .v_flex()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .v_flex()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("ЭЛЕКТРОННАЯ ПОЧТА"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("rust***@discord.local"),
                                    ),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_link)
                                    .child("Показать"),
                            ),
                    )
                    .child(div().h(px(1.)).bg(DISCORD_THEME.border_subtle))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .v_flex()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("НОМЕР ТЕЛЕФОНА"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("+7 ••• ••• 42-13"),
                                    ),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_link)
                                    .child("Изменить"),
                            ),
                    )
                    .child(div().h(px(1.)).bg(DISCORD_THEME.border_subtle))
                    .child(
                        div().flex().justify_between().items_center().child(
                            div()
                                .v_flex()
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child("ДВУХФАКТОРНАЯ АУТЕНТИФИКАЦИЯ (2FA)"),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(DISCORD_THEME.status_online)
                                        .child("✓ Включена (Google Authenticator / TOTP)"),
                                ),
                        ),
                    ),
            )
    }

    /// Вкладка «Профиль пользователя»
    fn render_profile_tab(state: &AppState, _cx: &mut Context<AppLayout>) -> impl IntoElement {
        let username_str = state.current_user.username.clone();

        div()
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child("ПРОФИЛЬ ПОЛЬЗОВАТЕЛЯ"),
            )
            .child(
                div()
                    .p(px(16.))
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .v_flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ОТОБРАЖАЕМОЕ ИМЯ"),
                    )
                    .child(
                        div()
                            .h(px(36.))
                            .px(px(12.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .flex()
                            .items_center()
                            .text_color(DISCORD_THEME.text_header)
                            .child(username_str),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("О СЕБЕ (BIO)"),
                    )
                    .child(
                        div()
                            .p(px(12.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .text_size(px(13.))
                            .text_color(DISCORD_THEME.text_normal)
                            .child("Rust Developer & GPUI Contributor 🦀 Разработка нативного клиента Discord."),
                    ),
            )
    }

    /// Вкладка «Горячие клавиши»
    fn render_keybinds_tab(_state: &AppState, _cx: &mut Context<AppLayout>) -> impl IntoElement {
        let keybinds = [
            ("Режим рации (Push-to-Talk)", "Caps Lock"),
            ("Включить/выключить микрофон (Mute)", "Ctrl + Shift + M"),
            ("Включить/выключить звук (Deafen)", "Ctrl + Shift + D"),
            ("Быстрый переход (Quick Switcher)", "Ctrl + K"),
            ("Палитра эмодзи", "Ctrl + E"),
            ("Поиск по чату", "Ctrl + F"),
            ("Настройки пользователя", "Ctrl + ,"),
            ("Закрыть модальное окно", "Escape"),
        ];

        div()
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child("ГОРЯЧИЕ КЛАВИШИ"),
            )
            .child(
                div()
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .p(px(16.))
                    .v_flex()
                    .gap(px(8.))
                    .children(keybinds.iter().map(|(action, key)| {
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .py(px(6.))
                            .border_b_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(DISCORD_THEME.text_normal)
                                    .child(*action),
                            )
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(3.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.border_subtle)
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(gpui::white())
                                    .child(*key),
                            )
                    })),
            )
    }

    /// Вкладка «Уведомления»
    fn render_notifications_tab(
        _state: &AppState,
        _cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child("УВЕДОМЛЕНИЯ"),
            )
            .child(
                div()
                    .p(px(16.))
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .v_flex()
                    .gap(px(12.))
                    .child(Self::render_static_toggle(
                        "Системные уведомления рабочего стола (Freedesktop / DBus)",
                        true,
                    ))
                    .child(Self::render_static_toggle("Звук входящего сообщения", true))
                    .child(Self::render_static_toggle(
                        "Звук подключения к голосовому каналу",
                        true,
                    ))
                    .child(Self::render_static_toggle(
                        "Звуковой сигнал активации Push-to-Talk",
                        false,
                    )),
            )
    }

    /// Вкладка «Текст и изображения»
    fn render_text_images_tab(_state: &AppState, _cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child("ТЕКСТ И ИЗОБРАЖЕНИЯ"),
            )
            .child(
                div()
                    .p(px(16.))
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .v_flex()
                    .gap(px(12.))
                    .child(Self::render_static_toggle(
                        "Автоматически показывать изображения и видео в чате",
                        true,
                    ))
                    .child(Self::render_static_toggle(
                        "Предпросмотр ссылок (OpenGraph embeds)",
                        true,
                    ))
                    .child(Self::render_static_toggle(
                        "Преобразовывать текстовые смайлики в эмодзи (:) -> 😊)",
                        true,
                    ))
                    .child(Self::render_static_toggle(
                        "Рендеринг разметки Markdown в реальном времени",
                        true,
                    )),
            )
    }

    /// Вкладка «Язык интерфейса»
    fn render_language_tab(_state: &AppState, _cx: &mut Context<AppLayout>) -> impl IntoElement {
        let languages = [
            ("Русский (Russian)", "✓ Выбран"),
            ("English, US (American English)", ""),
            ("Deutsch (German)", ""),
            ("Français (French)", ""),
            ("日本語 (Japanese)", ""),
        ];

        div()
            .v_flex()
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_header)
                    .child("ЯЗЫК ИНТЕРФЕЙСА (I18N)"),
            )
            .child(
                div()
                    .p(px(16.))
                    .rounded(px(8.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .v_flex()
                    .gap(px(6.))
                    .children(languages.iter().map(|(lang, status)| {
                        div()
                            .p(px(10.))
                            .rounded(px(4.))
                            .bg(if !status.is_empty() {
                                DISCORD_THEME.bg_active
                            } else {
                                gpui::transparent_black()
                            })
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(gpui::white())
                                    .child(*lang),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.status_online)
                                    .child(*status),
                            )
                    })),
            )
    }

    /// Кнопка закрытия окна настроек (ESC)
    fn render_close_button(cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .pt(px(40.))
            .pr(px(40.))
            .v_flex()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .id("btn-settings-close")
                    .size(px(36.))
                    .rounded_full()
                    .border_2()
                    .border_color(DISCORD_THEME.text_muted)
                    .hover(|s| s.border_color(DISCORD_THEME.text_header))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(18.))
                    .text_color(DISCORD_THEME.text_muted)
                    .hover(|s| s.text_color(DISCORD_THEME.text_header))
                    .child("✕")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.is_settings_open = false;
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_muted)
                    .child("ESC"),
            )
    }

    /// Вспомогательная кнопка выбора уровня громкости
    fn render_vol_button(
        label: &'static str,
        vol: f32,
        is_input: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .cursor_pointer()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(DISCORD_THEME.bg_secondary)
            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
            .text_size(px(11.))
            .font_weight(FontWeight::BOLD)
            .text_color(DISCORD_THEME.text_muted)
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if is_input {
                        this.state.set_input_volume(vol);
                    } else {
                        this.state.set_output_volume(vol);
                    }
                    cx.notify();
                }),
            )
    }

    /// Вспомогательная кнопка выбора масштаба шрифта
    fn render_font_scale_button(
        label: &'static str,
        scale: f32,
        is_selected: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .cursor_pointer()
            .px(px(12.))
            .py(px(6.))
            .rounded(px(4.))
            .bg(if is_selected {
                DISCORD_THEME.blurple
            } else {
                DISCORD_THEME.bg_secondary
            })
            .hover(|s| {
                if !is_selected {
                    s.bg(DISCORD_THEME.bg_hover)
                } else {
                    s
                }
            })
            .text_size(px(12.))
            .font_weight(FontWeight::BOLD)
            .text_color(if is_selected {
                gpui::white()
            } else {
                DISCORD_THEME.text_normal
            })
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.state.set_font_scale(scale);
                    cx.notify();
                }),
            )
    }

    /// Строка интерактивного переключателя (Toggle Switch)
    fn render_toggle_row(
        title: &'static str,
        description: &'static str,
        is_on: bool,
        on_toggle: impl Fn(&mut AppLayout, &mut Context<AppLayout>) + 'static + Send + Sync,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .p(px(14.))
            .rounded(px(6.))
            .bg(DISCORD_THEME.bg_secondary)
            .border_1()
            .border_color(DISCORD_THEME.border_subtle)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child(description),
                    ),
            )
            .child(
                div()
                    .cursor_pointer()
                    .w(px(42.))
                    .h(px(24.))
                    .rounded_full()
                    .bg(if is_on {
                        DISCORD_THEME.status_online
                    } else {
                        DISCORD_THEME.bg_tertiary
                    })
                    .p(px(2.))
                    .flex()
                    .justify_end()
                    .child(div().size(px(20.)).rounded_full().bg(gpui::white()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            on_toggle(this, cx);
                        }),
                    ),
            )
    }

    /// Статическая строка переключателя (для информационных вкладок)
    fn render_static_toggle(title: &'static str, is_on: bool) -> impl IntoElement {
        div()
            .py(px(8.))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(DISCORD_THEME.border_subtle)
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(DISCORD_THEME.text_normal)
                    .child(title),
            )
            .child(
                div()
                    .w(px(38.))
                    .h(px(20.))
                    .rounded_full()
                    .bg(if is_on {
                        DISCORD_THEME.status_online
                    } else {
                        DISCORD_THEME.bg_tertiary
                    })
                    .p(px(2.))
                    .flex()
                    .justify_end()
                    .child(div().size(px(16.)).rounded_full().bg(gpui::white())),
            )
    }
}
