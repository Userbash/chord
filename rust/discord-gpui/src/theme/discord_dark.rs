use gpui::{Hsla, rgb, rgba};

/// Шкала типографики Discord (соответствует эталону Web/Desktop клиента Discord).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyScale {
    // Размеры шрифтов (px)
    pub xxs: f32,  // 10px - бейджи, компактные таймстемпы
    pub xs: f32,   // 12px - метаданные, категории каналов, теги ролей
    pub sm: f32,   // 14px - имена каналов, список участников, поля ввода
    pub base: f32, // 16px - основной текст сообщений чата, стандартный body
    pub lg: f32,   // 18px - подзаголовки, заголовки секций
    pub xl: f32,   // 20px - заголовок канала, карточки
    pub xxl: f32,  // 24px - заголовки модальных окон, имя сервера
    pub hero: f32, // 32px - приветственные экраны, баннеры Discovery

    // Интерлиньяж (line-height multiplier)
    pub line_height_tight: f32,   // 1.2 - для крупных заголовков
    pub line_height_snug: f32,    // 1.25 - для компактных списков
    pub line_height_normal: f32,  // 1.375 - стандарт разметки сообщений Discord (22px на 16px base)
    pub line_height_relaxed: f32, // 1.5 - для описаний и длинных текстов

    // Насыщенность шрифта (font-weight)
    pub weight_light: u16,     // 300
    pub weight_regular: u16,   // 400
    pub weight_medium: u16,    // 500
    pub weight_semibold: u16,  // 600
    pub weight_bold: u16,      // 700
    pub weight_extrabold: u16, // 800

    // Семейства шрифтов
    pub font_primary: &'static str, // "gg sans", "Noto Sans", "Helvetica Neue", sans-serif
    pub font_display: &'static str, // "ABC Ginto Discord", "gg sans", sans-serif
    pub font_code: &'static str,    // "gg mono", "Source Code Pro", Consolas, monospace
}

impl Default for TypographyScale {
    fn default() -> Self {
        Self {
            xxs: 10.0,
            xs: 12.0,
            sm: 14.0,
            base: 16.0,
            lg: 18.0,
            xl: 20.0,
            xxl: 24.0,
            hero: 32.0,

            line_height_tight: 1.2,
            line_height_snug: 1.25,
            line_height_normal: 1.375,
            line_height_relaxed: 1.5,

            weight_light: 300,
            weight_regular: 400,
            weight_medium: 500,
            weight_semibold: 600,
            weight_bold: 700,
            weight_extrabold: 800,

            font_primary: "gg sans, Noto Sans, Helvetica Neue, Helvetica, Arial, sans-serif",
            font_display: "ABC Ginto Discord, gg sans, Noto Sans, Helvetica Neue, sans-serif",
            font_code: "gg mono, Source Code Pro, Consolas, Andale Mono, Lucida Console, monospace",
        }
    }
}

impl TypographyScale {
    /// Проверка монотонного возрастания шкалы размеров шрифтов
    #[inline]
    #[allow(dead_code)]
    pub fn font_size_scale_ordered(&self) -> bool {
        self.xxs < self.xs
            && self.xs < self.sm
            && self.sm < self.base
            && self.base < self.lg
            && self.lg < self.xl
            && self.xl < self.xxl
            && self.xxl < self.hero
    }
}

/// Набор параметров стиля текста для элементов UI
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub font_weight: u16,
    pub font_family: &'static str,
}

/// Отчёт аудита контрастности по стандарту WCAG 2.1 / 2.2 AA
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct WcagComplianceReport {
    pub passed: bool,
    pub audited_pairs: usize,
    pub failed_pairs: Vec<String>,
}

/// Конвертация GPUI Hsla в sRGB (0.0..1.0)
#[allow(dead_code)]
pub fn hsla_to_rgba(hsla: Hsla) -> (f32, f32, f32, f32) {
    let h = hsla.h.rem_euclid(1.0);
    let s = hsla.s.clamp(0.0, 1.0);
    let l = hsla.l.clamp(0.0, 1.0);

    if s == 0.0 {
        return (l, l, l, hsla.a);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - (l * s)
    };
    let p = 2.0 * l - q;

    let hue_to_rgb = |p: f32, q: f32, mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };

    (
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
        hsla.a,
    )
}

/// Преобразование sRGB канала в линейный фотометрический канал
fn srgb_channel_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Вычисление относительной яркости (Relative Luminance) по W3C WCAG 2.1
#[allow(dead_code)]
pub fn calculate_relative_luminance(color: Hsla) -> f32 {
    let (r, g, b, _) = hsla_to_rgba(color);
    let r_lin = srgb_channel_to_linear(r);
    let g_lin = srgb_channel_to_linear(g);
    let b_lin = srgb_channel_to_linear(b);
    0.2126 * r_lin + 0.7152 * g_lin + 0.0722 * b_lin
}

/// Вычисление коэффициента контрастности (Contrast Ratio) по W3C WCAG 2.1
#[allow(dead_code)]
pub fn calculate_contrast_ratio(fg: Hsla, bg: Hsla) -> f32 {
    let l1 = calculate_relative_luminance(fg);
    let l2 = calculate_relative_luminance(bg);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

/// Цветовая палитра Discord Dark Theme (эталон из Web/Desktop клиента Discord).
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct DiscordThemeColors {
    // Фоны поверхностей
    pub bg_tertiary: Hsla,      // #1e1f22 - Серверная колонка (Server Rail)
    pub bg_secondary: Hsla,     // #2b2d31 - Список каналов / сайдбар
    pub bg_secondary_alt: Hsla, // #232428 - Нижняя панель профиля / заголовки
    pub bg_primary: Hsla,       // #313338 - Основное окно чата
    pub bg_accent: Hsla,        // #383a40 - Поле ввода сообщений / карточки
    pub bg_hover: Hsla,         // #35373c - Состояние при наведении на элемент
    pub bg_active: Hsla,        // #404249 - Активный элемент списка
    pub bg_floating: Hsla,      // #2b2d31 - Всплывающие меню, поповеры, тултипы
    pub bg_backdrop: Hsla,      // rgba(0, 0, 0, 0.7) - Затемнение модальных окон
    pub bg_modal: Hsla,         // #313338 - Тело модального окна
    pub bg_modal_footer: Hsla,  // #2b2d31 - Подвал модального окна

    // Интерактивные полупрозрачные модификаторы наведения
    pub bg_modifier_hover: Hsla,  // rgba(255, 255, 255, 0.07) - Наведение
    pub bg_modifier_active: Hsla, // rgba(255, 255, 255, 0.11) - Нажатие
    pub bg_modifier_selected: Hsla, // rgba(255, 255, 255, 0.14) - Выделение
    pub bg_modifier_accent: Hsla, // rgba(255, 255, 255, 0.06) - Акцент

    // Интерактивные состояния текста и иконок
    pub interactive_normal: Hsla, // #949ba4 - Стандартный цвет элементов управления
    pub interactive_hover: Hsla,  // #dbdee1 - Наведение на элемент
    pub interactive_active: Hsla, // #ffffff - Активный элемент
    pub interactive_muted: Hsla,  // #4e5058 - Отключённый/неактивный элемент

    // Акценты и фирменный Blurple
    pub blurple: Hsla,          // #5865f2 - Фирменный Discord Blurple
    pub blurple_hover: Hsla,    // #4752c4 - Blurple при наведении
    pub blurple_active: Hsla,   // #3c45a5 - Blurple при нажатии
    pub blurple_light: Hsla,    // #7983f5 - Осветлённый Blurple для мелких акцентов
    pub blurple_alpha_15: Hsla, // rgba(88, 101, 242, 0.15) - Фон упоминаний
    pub blurple_alpha_30: Hsla, // rgba(88, 101, 242, 0.30) - Интенсивная подсветка

    // Кнопки (Button Variants)
    pub btn_primary_bg: Hsla,
    pub btn_primary_hover: Hsla,
    pub btn_primary_active: Hsla,
    pub btn_primary_text: Hsla,

    pub btn_secondary_bg: Hsla,
    pub btn_secondary_hover: Hsla,
    pub btn_secondary_active: Hsla,
    pub btn_secondary_text: Hsla,

    pub btn_danger_bg: Hsla,
    pub btn_danger_hover: Hsla,
    pub btn_danger_active: Hsla,
    pub btn_danger_text: Hsla,

    pub btn_success_bg: Hsla,
    pub btn_success_hover: Hsla,
    pub btn_success_active: Hsla,
    pub btn_success_text: Hsla,

    pub btn_ghost_bg: Hsla,
    pub btn_ghost_hover: Hsla,
    pub btn_ghost_active: Hsla,
    pub btn_ghost_text: Hsla,

    // Статусы
    pub status_online: Hsla,       // #23a55a - В сети (зелёный)
    pub status_online_hover: Hsla, // #1e8749
    pub status_idle: Hsla,         // #f0b232 - Не активен (жёлтый)
    pub status_idle_hover: Hsla,   // #d89b1c
    pub status_dnd: Hsla,          // #f23f43 - Не беспокоить (красный)
    pub status_dnd_hover: Hsla,    // #da373c
    pub status_offline: Hsla,      // #80848e - Не в сети (серый)
    pub status_streaming: Hsla,    // #593695 - Стриминг (фиолетовый)

    // Текстовая палитра (WCAG AA)
    pub text_header: Hsla,     // #f2f3f5 - Белый заголовков (WCAG AAA >= 7:1)
    pub text_normal: Hsla,     // #dbdee1 - Обычный читаемый текст (WCAG AAA >= 7:1)
    pub text_muted: Hsla,      // #949ba4 - Второстепенный текст (WCAG AA >= 4.5:1 на primary)
    pub text_muted_aa: Hsla, // #9fa6b2 - Высококонтрастный текст (WCAG AA >= 4.5:1 на всех поверхностях)
    pub text_link: Hsla,     // #00a8fc - Гиперссылки (WCAG AA >= 4.5:1)
    pub text_link_hover: Hsla, // #1ebbff - Ссылки при наведении
    pub text_danger: Hsla,   // #fa777c - Текст ошибок и предупреждений (WCAG AA >= 4.5:1)
    pub text_positive: Hsla, // #57f287 - Текст успеха (WCAG AA >= 4.5:1)
    pub text_warning: Hsla,  // #fee75c - Текст предупреждений (WCAG AA >= 4.5:1)
    pub text_code: Hsla,     // #e3e5e8 - Моноширинный текст кода

    // Разделители и границы
    pub border_subtle: Hsla, // #3f4147 - Тонкие границы между панелями
    pub border_strong: Hsla, // #4e5058 - Контрастные границы
    pub border_interactive_hover: Hsla, // #6d6f78 - Граница при наведении
    pub border_interactive_focus: Hsla, // #00a8fc - Синяя рамка фокуса
    pub mention_bg: Hsla,    // rgba(88, 101, 242, 0.15) - Подсветка упоминаний
    pub mention_border: Hsla, // #5865f2 - Вертикальная полоса упоминания
    pub mention_hover_bg: Hsla, // rgba(88, 101, 242, 0.25)

    // Шкала типографики
    pub typography: TypographyScale,
}

impl Default for DiscordThemeColors {
    fn default() -> Self {
        Self {
            bg_tertiary: rgb(0x1e1f22).into(),
            bg_secondary: rgb(0x2b2d31).into(),
            bg_secondary_alt: rgb(0x232428).into(),
            bg_primary: rgb(0x313338).into(),
            bg_accent: rgb(0x383a40).into(),
            bg_hover: rgb(0x35373c).into(),
            bg_active: rgb(0x404249).into(),
            bg_floating: rgb(0x2b2d31).into(),
            bg_backdrop: rgba(0x000000b2).into(),
            bg_modal: rgb(0x313338).into(),
            bg_modal_footer: rgb(0x2b2d31).into(),

            bg_modifier_hover: rgba(0xffffff12).into(),
            bg_modifier_active: rgba(0xffffff1c).into(),
            bg_modifier_selected: rgba(0xffffff24).into(),
            bg_modifier_accent: rgba(0xffffff0f).into(),

            interactive_normal: rgb(0x949ba4).into(),
            interactive_hover: rgb(0xdbdee1).into(),
            interactive_active: rgb(0xffffff).into(),
            interactive_muted: rgb(0x4e5058).into(),

            blurple: rgb(0x5865f2).into(),
            blurple_hover: rgb(0x4752c4).into(),
            blurple_active: rgb(0x3c45a5).into(),
            blurple_light: rgb(0x7983f5).into(),
            blurple_alpha_15: rgba(0x5865f226).into(),
            blurple_alpha_30: rgba(0x5865f24d).into(),

            btn_primary_bg: rgb(0x5865f2).into(),
            btn_primary_hover: rgb(0x4752c4).into(),
            btn_primary_active: rgb(0x3c45a5).into(),
            btn_primary_text: rgb(0xffffff).into(),

            btn_secondary_bg: rgb(0x4e5058).into(),
            btn_secondary_hover: rgb(0x6d6f78).into(),
            btn_secondary_active: rgb(0x80848e).into(),
            btn_secondary_text: rgb(0xffffff).into(),

            btn_danger_bg: rgb(0xda373c).into(),
            btn_danger_hover: rgb(0xa12828).into(),
            btn_danger_active: rgb(0x8f2022).into(),
            btn_danger_text: rgb(0xffffff).into(),

            btn_success_bg: rgb(0x248046).into(),
            btn_success_hover: rgb(0x1a6334).into(),
            btn_success_active: rgb(0x144d28).into(),
            btn_success_text: rgb(0xffffff).into(),

            btn_ghost_bg: rgba(0x00000000).into(),
            btn_ghost_hover: rgba(0xffffff12).into(),
            btn_ghost_active: rgba(0xffffff1c).into(),
            btn_ghost_text: rgb(0xdbdee1).into(),

            status_online: rgb(0x23a55a).into(),
            status_online_hover: rgb(0x1e8749).into(),
            status_idle: rgb(0xf0b232).into(),
            status_idle_hover: rgb(0xd89b1c).into(),
            status_dnd: rgb(0xf23f43).into(),
            status_dnd_hover: rgb(0xda373c).into(),
            status_offline: rgb(0x80848e).into(),
            status_streaming: rgb(0x593695).into(),

            text_header: rgb(0xf2f3f5).into(),
            text_normal: rgb(0xdbdee1).into(),
            text_muted: rgb(0x949ba4).into(),
            text_muted_aa: rgb(0x9fa6b2).into(),
            text_link: rgb(0x00a8fc).into(),
            text_link_hover: rgb(0x1ebbff).into(),
            text_danger: rgb(0xfa777c).into(),
            text_positive: rgb(0x57f287).into(),
            text_warning: rgb(0xfee75c).into(),
            text_code: rgb(0xe3e5e8).into(),

            border_subtle: rgb(0x3f4147).into(),
            border_strong: rgb(0x4e5058).into(),
            border_interactive_hover: rgb(0x6d6f78).into(),
            border_interactive_focus: rgb(0x00a8fc).into(),
            mention_bg: rgba(0x5865f226).into(),
            mention_border: rgb(0x5865f2).into(),
            mention_hover_bg: rgba(0x5865f240).into(),

            typography: TypographyScale::default(),
        }
    }
}

impl DiscordThemeColors {
    /// Вычисление относительной яркости цвета темы
    #[inline]
    #[allow(dead_code)]
    pub fn relative_luminance(&self, color: Hsla) -> f32 {
        calculate_relative_luminance(color)
    }

    /// Вычисление коэффициента контрастности между двумя цветами
    #[inline]
    pub fn contrast_ratio(&self, fg: Hsla, bg: Hsla) -> f32 {
        calculate_contrast_ratio(fg, bg)
    }

    /// Проверка соответствия стандарту WCAG AA для обычного текста (коэффициент >= 4.5:1)
    #[inline]
    #[allow(dead_code)]
    pub fn passes_wcag_aa_normal(&self, fg: Hsla, bg: Hsla) -> bool {
        self.contrast_ratio(fg, bg) >= 4.5
    }

    /// Проверка соответствия стандарту WCAG AA для крупного текста и элементов UI (коэффициент >= 3.0:1)
    #[inline]
    #[allow(dead_code)]
    pub fn passes_wcag_aa_large(&self, fg: Hsla, bg: Hsla) -> bool {
        self.contrast_ratio(fg, bg) >= 3.0
    }

    /// Проверка соответствия наивысшему стандарту WCAG AAA (коэффициент >= 7.0:1)
    #[inline]
    #[allow(dead_code)]
    pub fn passes_wcag_aaa(&self, fg: Hsla, bg: Hsla) -> bool {
        self.contrast_ratio(fg, bg) >= 7.0
    }

    /// Запуск полного аудита контрастности цветовых пар темы на соответствие стандарту WCAG AA
    #[allow(dead_code)]
    pub fn check_wcag_aa_compliance(&self) -> WcagComplianceReport {
        let pairs: &[(&str, Hsla, Hsla, f32)] = &[
            (
                "text_header on bg_primary",
                self.text_header,
                self.bg_primary,
                4.5,
            ),
            (
                "text_header on bg_secondary",
                self.text_header,
                self.bg_secondary,
                4.5,
            ),
            (
                "text_header on bg_tertiary",
                self.text_header,
                self.bg_tertiary,
                4.5,
            ),
            (
                "text_normal on bg_primary",
                self.text_normal,
                self.bg_primary,
                4.5,
            ),
            (
                "text_normal on bg_secondary",
                self.text_normal,
                self.bg_secondary,
                4.5,
            ),
            (
                "text_normal on bg_tertiary",
                self.text_normal,
                self.bg_tertiary,
                4.5,
            ),
            (
                "text_muted on bg_primary",
                self.text_muted,
                self.bg_primary,
                4.5,
            ),
            (
                "text_muted on bg_secondary",
                self.text_muted,
                self.bg_secondary,
                4.5,
            ),
            (
                "text_muted on bg_tertiary",
                self.text_muted,
                self.bg_tertiary,
                4.5,
            ),
            (
                "text_muted_aa on bg_accent",
                self.text_muted_aa,
                self.bg_accent,
                4.5,
            ),
            (
                "text_link on bg_primary",
                self.text_link,
                self.bg_primary,
                4.5,
            ),
            (
                "text_link on bg_secondary",
                self.text_link,
                self.bg_secondary,
                4.5,
            ),
            (
                "text_danger on bg_primary",
                self.text_danger,
                self.bg_primary,
                4.5,
            ),
            (
                "btn_primary_text on btn_primary_bg",
                self.btn_primary_text,
                self.btn_primary_bg,
                4.5,
            ),
            (
                "btn_danger_text on btn_danger_bg",
                self.btn_danger_text,
                self.btn_danger_bg,
                4.5,
            ),
            (
                "btn_success_text on btn_success_bg",
                self.btn_success_text,
                self.btn_success_bg,
                4.5,
            ),
        ];

        let mut failed = Vec::new();
        for (name, fg, bg, threshold) in pairs {
            let ratio = self.contrast_ratio(*fg, *bg);
            if ratio < *threshold {
                failed.push(format!(
                    "{name}: ratio {ratio:.2}:1 < threshold {threshold:.1}:1"
                ));
            }
        }

        WcagComplianceReport {
            passed: failed.is_empty(),
            audited_pairs: pairs.len(),
            failed_pairs: failed,
        }
    }

    /// Пресет стиля заголовка (semibold/bold)
    #[allow(dead_code)]
    pub fn header_primary(&self, size: f32) -> TypographyStyle {
        TypographyStyle {
            font_size: size,
            line_height: self.typography.line_height_tight,
            font_weight: self.typography.weight_bold,
            font_family: self.typography.font_primary,
        }
    }

    /// Пресет основного текста сообщений (16px, 1.375 line-height, regular)
    #[allow(dead_code)]
    pub fn body_normal(&self) -> TypographyStyle {
        TypographyStyle {
            font_size: self.typography.base,
            line_height: self.typography.line_height_normal,
            font_weight: self.typography.weight_regular,
            font_family: self.typography.font_primary,
        }
    }

    /// Пресет второстепенного текста (14px, medium)
    #[allow(dead_code)]
    pub fn body_muted(&self) -> TypographyStyle {
        TypographyStyle {
            font_size: self.typography.sm,
            line_height: self.typography.line_height_normal,
            font_weight: self.typography.weight_medium,
            font_family: self.typography.font_primary,
        }
    }

    /// Пресет метаданных, дат и меток (12px, regular)
    #[allow(dead_code)]
    pub fn metadata(&self) -> TypographyStyle {
        TypographyStyle {
            font_size: self.typography.xs,
            line_height: self.typography.line_height_snug,
            font_weight: self.typography.weight_regular,
            font_family: self.typography.font_primary,
        }
    }

    /// Пресет встроенного фрагмента кода (моноширинный)
    #[allow(dead_code)]
    pub fn code_inline(&self) -> TypographyStyle {
        TypographyStyle {
            font_size: self.typography.sm,
            line_height: self.typography.line_height_normal,
            font_weight: self.typography.weight_regular,
            font_family: self.typography.font_code,
        }
    }
}

pub static DISCORD_THEME: std::sync::LazyLock<DiscordThemeColors> =
    std::sync::LazyLock::new(DiscordThemeColors::default);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_tokens_expansion_and_hover_effects() {
        let theme = &*DISCORD_THEME;

        // 1. Проверка интерактивных состояний и hover-модификаторов
        assert_ne!(theme.interactive_normal, theme.interactive_hover);
        assert_ne!(theme.interactive_hover, theme.interactive_active);
        assert_ne!(theme.bg_modifier_hover, theme.bg_modifier_active);

        // 2. Проверка палитры кнопок (Blurple, Secondary, Danger, Success)
        assert_ne!(theme.btn_primary_bg, theme.btn_primary_hover);
        assert_ne!(theme.btn_primary_hover, theme.btn_primary_active);
        assert_ne!(theme.btn_secondary_bg, theme.btn_secondary_hover);
        assert_ne!(theme.btn_danger_bg, theme.btn_danger_hover);
        assert_ne!(theme.btn_success_bg, theme.btn_success_hover);

        // 3. Плавающие панели и модальные фоны
        assert_ne!(theme.bg_floating, theme.bg_backdrop);
        assert_ne!(theme.bg_modal, theme.bg_modal_footer);
    }

    #[test]
    fn test_typography_scale_hierarchy() {
        let typo = &DISCORD_THEME.typography;

        // Монотонный рост шкалы шрифтов: xxs < xs < sm < base < lg < xl < xxl < hero
        assert!(typo.xxs < typo.xs);
        assert!(typo.xs < typo.sm);
        assert!(typo.sm < typo.base);
        assert!(typo.base < typo.lg);
        assert!(typo.lg < typo.xl);
        assert!(typo.xl < typo.xxl);
        assert!(typo.xxl < typo.hero);

        // Стандарт интерлиньяжа чата Discord (1.375 / 22px на 16px шрифт)
        assert!((typo.line_height_normal - 1.375).abs() < 1e-4);
        assert!(typo.line_height_tight < typo.line_height_normal);
        assert!(typo.line_height_normal < typo.line_height_relaxed);

        // Проверка весов шрифтов
        assert_eq!(typo.weight_regular, 400);
        assert_eq!(typo.weight_medium, 500);
        assert_eq!(typo.weight_semibold, 600);
        assert_eq!(typo.weight_bold, 700);

        // Пресеты типографики
        let body = DISCORD_THEME.body_normal();
        assert_eq!(body.font_size, typo.base);
        assert_eq!(body.font_weight, typo.weight_regular);

        let header = DISCORD_THEME.header_primary(typo.xl);
        assert_eq!(header.font_size, typo.xl);
        assert_eq!(header.font_weight, typo.weight_bold);
    }

    #[test]
    fn test_wcag_aa_contrast_ratios_compliance() {
        let theme = &*DISCORD_THEME;

        // 1. Проверка формулы контрастности для заведомо контрастной пары (черный/белый = 21:1)
        let white: Hsla = rgb(0xffffff).into();
        let black: Hsla = rgb(0x000000).into();
        let ratio_bw = theme.contrast_ratio(white, black);
        assert!(
            (ratio_bw - 21.0).abs() < 0.2,
            "White on black must be ~21:1, got {ratio_bw}"
        );

        // Симметричность вычисления контраста
        assert!(
            (theme.contrast_ratio(white, black) - theme.contrast_ratio(black, white)).abs() < 1e-4
        );

        // Контраст одинаковых цветов равен 1.0:1
        assert!((theme.contrast_ratio(white, white) - 1.0).abs() < 1e-4);
        assert!((theme.contrast_ratio(black, black) - 1.0).abs() < 1e-4);

        // Проверка уровней относительной яркости (luminance)
        assert!((theme.relative_luminance(black) - 0.0).abs() < 1e-3);
        assert!((theme.relative_luminance(white) - 1.0).abs() < 1e-3);

        // 2. Текст заголовков и обычный текст должны соответствовать стандарту WCAG AAA (>= 7.0:1)
        assert!(theme.contrast_ratio(theme.text_header, theme.bg_primary) >= 7.0);
        assert!(theme.contrast_ratio(theme.text_header, theme.bg_secondary) >= 7.0);
        assert!(theme.contrast_ratio(theme.text_header, theme.bg_tertiary) >= 7.0);

        assert!(theme.contrast_ratio(theme.text_normal, theme.bg_primary) >= 7.0);
        assert!(theme.contrast_ratio(theme.text_normal, theme.bg_secondary) >= 7.0);
        assert!(theme.contrast_ratio(theme.text_normal, theme.bg_tertiary) >= 7.0);

        assert!(theme.passes_wcag_aaa(theme.text_header, theme.bg_primary));
        assert!(theme.passes_wcag_aaa(theme.text_normal, theme.bg_primary));

        // 3. Второстепенный текст text_muted должен соответствовать WCAG AA (>= 4.5:1)
        assert!(
            theme.passes_wcag_aa_normal(theme.text_muted, theme.bg_primary),
            "text_muted on bg_primary must pass WCAG AA (>= 4.5:1)"
        );
        assert!(
            theme.passes_wcag_aa_normal(theme.text_muted_aa, theme.bg_accent),
            "text_muted_aa on bg_accent must pass WCAG AA (>= 4.5:1)"
        );

        // 4. Текст ссылок должен проходить WCAG AA на основных фонах
        assert!(theme.passes_wcag_aa_normal(theme.text_link, theme.bg_primary));
        assert!(theme.passes_wcag_aa_normal(theme.text_link, theme.bg_secondary));

        // 5. Белый текст на кнопках Blurple, Danger, Success должен проходить WCAG AA
        assert!(theme.passes_wcag_aa_normal(theme.btn_primary_text, theme.btn_primary_bg));
        assert!(theme.passes_wcag_aa_normal(theme.btn_danger_text, theme.btn_danger_bg));
        assert!(theme.passes_wcag_aa_normal(theme.btn_success_text, theme.btn_success_bg));

        // 6. Полный аудит соответствия
        let report = theme.check_wcag_aa_compliance();
        assert!(
            report.passed,
            "WCAG AA compliance audit failed: {:?}",
            report.failed_pairs
        );
        assert!(report.audited_pairs >= 12);
    }

    #[test]
    fn test_all_9_dimensions_theme_contract() {
        let theme = &*DISCORD_THEME;
        let typo = &theme.typography;

        // Измерение 1: Позитивные проверки (Positive)
        assert_eq!(typo.font_size_scale_ordered(), true);
        assert_ne!(theme.bg_primary, theme.bg_secondary);
        assert_ne!(theme.bg_secondary, theme.bg_tertiary);
        assert_ne!(theme.text_header, theme.text_muted);

        // Измерение 2: Негативные проверки (Negative)
        // Близкие по тону цвета фона не должны проходить проверку контрастности как текст
        assert!(!theme.passes_wcag_aa_normal(theme.bg_primary, theme.bg_secondary));
        assert!(!theme.passes_wcag_aa_large(theme.bg_primary, theme.bg_hover));

        // Измерение 3: Граничные значения (Boundary)
        // Границы шрифтов: от 10px до 32px
        assert!(typo.xxs >= 10.0 && typo.xxs <= 10.5);
        assert!(typo.hero >= 32.0 && typo.hero <= 36.0);
        // Границы весов: 300..=800
        assert_eq!(typo.weight_light, 300);
        assert_eq!(typo.weight_extrabold, 800);

        // Измерение 4: Обработка ошибок и отчёты (Errors & Audits)
        let audit = theme.check_wcag_aa_compliance();
        assert!(audit.passed);
        assert!(audit.failed_pairs.is_empty());
        assert_eq!(audit.audited_pairs, 16);

        // Измерение 5: Пустые / исходные состояния (Defaults & Empty)
        let default_theme = DiscordThemeColors::default();
        assert_eq!(default_theme.typography.font_primary.is_empty(), false);
        assert_eq!(default_theme.typography.font_display.is_empty(), false);
        assert_eq!(default_theme.typography.font_code.is_empty(), false);
        assert!(!default_theme.typography.base.is_nan());
        assert!(!default_theme.typography.line_height_normal.is_nan());

        // Измерение 6: Модификаторы прозрачности и альфа-каналы (Modifiers & Alpha)
        assert!(theme.bg_modifier_hover.a > 0.0 && theme.bg_modifier_hover.a < 0.2);
        assert!(theme.bg_modifier_active.a > theme.bg_modifier_hover.a);
        assert!(theme.bg_modifier_selected.a > theme.bg_modifier_active.a);
        assert_eq!(theme.btn_ghost_bg.a, 0.0);

        // Измерение 7: Интерактивные состояния hover/active/idle (Interactive & States)
        // Primary
        assert_ne!(theme.btn_primary_bg, theme.btn_primary_hover);
        assert_ne!(theme.btn_primary_hover, theme.btn_primary_active);
        // Secondary
        assert_ne!(theme.btn_secondary_bg, theme.btn_secondary_hover);
        assert_ne!(theme.btn_secondary_hover, theme.btn_secondary_active);
        // Danger
        assert_ne!(theme.btn_danger_bg, theme.btn_danger_hover);
        assert_ne!(theme.btn_danger_hover, theme.btn_danger_active);
        // Success
        assert_ne!(theme.btn_success_bg, theme.btn_success_hover);
        assert_ne!(theme.btn_success_hover, theme.btn_success_active);

        // Измерение 8: Пресеты типографики (Typography Presets)
        let body_muted = theme.body_muted();
        assert_eq!(body_muted.font_size, typo.sm);
        assert_eq!(body_muted.font_weight, typo.weight_medium);

        let metadata = theme.metadata();
        assert_eq!(metadata.font_size, typo.xs);
        assert_eq!(metadata.line_height, typo.line_height_snug);

        let code = theme.code_inline();
        assert_eq!(code.font_size, typo.sm);
        assert_eq!(code.font_family, typo.font_code);

        // Измерение 9: Совместимость и наблюдаемость (Compatibility & Observability)
        let sample_report = WcagComplianceReport {
            passed: false,
            audited_pairs: 1,
            failed_pairs: vec!["sample_fg on sample_bg: ratio 2.1 < 4.5".to_string()],
        };
        assert!(!sample_report.passed);
        assert_eq!(sample_report.failed_pairs.len(), 1);
    }
}
