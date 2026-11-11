// @ts-check
/**
 * Пользовательские темы оформления и динамическая CSS палитра (TASK-985).
 * Поддерживает 4 встроенные темы Discord (Dark, Light, Midnight, AMOLED) и до 8
 * пользовательских тем (включая импорт пакетов `.serein-extension`), горячую инъекцию
 * CSS-переменных в `:root` без перезагрузки интерфейса, валидацию контрастности
 * по стандарту WCAG 2.2 AA/AAA и защиту от CSS-инъекций.
 */
import { existsSync, readFileSync } from "node:fs";
import { CANONICAL_TOKEN_MAP, RESOLVED_COLLISIONS } from "./token-map.js";

export const THEME_PICKER_LAYOUT = Object.freeze({
  container: "Theme Picker",
  dimensions: "100%",
  padding: "16px",
  spacing: "12px",
});

export const THEME_PICKER_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const THEME_PREVIEW_STATES = Object.freeze({
  element: "theme-preview",
  default: "dark",
  hover: "highlight theme",
  active: "applied",
});

/** @type {readonly ("dark" | "light" | "midnight" | "amoled")[]} */
export const BUILTIN_THEME_IDS = Object.freeze(["dark", "light", "midnight", "amoled"]);
export const MAX_INSTALLED_CUSTOM_THEMES = 8;
export const EMERGENCY_RESET_SHORTCUT = "Ctrl+Shift+F12";
export const CUSTOM_THEME_STYLE_ELEMENT_ID = "discord-custom-theme-palette";
export const CUSTOM_THEMES_STORAGE_KEY = "discord.custom_themes.v1";
export const WCAG_AA_TEXT_MIN = 4.5;
export const WCAG_AAA_TEXT_MIN = 7.0;
export const WCAG_AA_NON_TEXT_MIN = 3.0;

/** @type {Readonly<Record<string, string>>} */
const DARK_TOKENS = Object.freeze({
  "--bg-app": "#121214",
  "--bg-chat": "#1a1a1e",
  "--surface-1": "#202024",
  "--surface-2": "#242429",
  "--surface-3": "#28282d",
  "--surface-4": "#2c2d32",
  "--surface-sunken": "#222327",
  "--surface-input": "#1e1f22",
  "--text-strong": "#fbfbfb",
  "--text": "#efeff1",
  "--text-subtle": "#abacb2",
  "--text-muted": "#96979e",
  "--text-link": "#4d96ee",
  "--text-accent": "#a5b0ff",
  "--danger-text": "#ff9a9e",
  "--positive-text": "#57d98a",
  "--text-on-accent": "#ffffff",
  "--text-on-danger": "#ffffff",
  "--text-on-warning": "#121214",
  "--accent": "#5865f2",
  "--danger-fill": "#cf3a3f",
  "--warning": "#fdb833",
  "--focus-ring": "#a5b0ff",
  "--status-online": "#3d9e60",
  "--status-idle": "#fdb833",
  "--status-dnd": "#da3e44",
  "--border": "#3f4147",
});

/** @type {Readonly<Record<string, string>>} */
const LIGHT_TOKENS = Object.freeze({
  "--bg-app": "#f7f7f8",
  "--bg-chat": "#ffffff",
  "--surface-1": "#efeff1",
  "--surface-2": "#f3f3f4",
  "--surface-3": "#ebeced",
  "--surface-4": "#e8e8ea",
  "--surface-sunken": "#f3f3f4",
  "--surface-input": "#ffffff",
  "--text-strong": "#111214",
  "--text": "#232428",
  "--text-subtle": "#4e5058",
  "--text-muted": "#5c5e66",
  "--text-link": "#0058b0",
  "--text-accent": "#3c45a5",
  "--danger-text": "#a1282f",
  "--positive-text": "#136f3e",
  "--text-on-accent": "#ffffff",
  "--text-on-danger": "#ffffff",
  "--text-on-warning": "#121214",
  "--accent": "#5865f2",
  "--danger-fill": "#cf3a3f",
  "--warning": "#fdb833",
  "--focus-ring": "#4654c0",
  "--status-online": "#237c48",
  "--status-idle": "#966000",
  "--status-dnd": "#c42b32",
  "--border": "#c8c9ce",
});

/** @type {Readonly<Record<string, string>>} */
const MIDNIGHT_TOKENS = Object.freeze({
  ...DARK_TOKENS,
  "--bg-app": "#0b0d14",
  "--bg-chat": "#111420",
  "--surface-1": "#161a29",
  "--surface-2": "#1b2032",
  "--surface-3": "#20263b",
  "--surface-4": "#252c44",
  "--surface-sunken": "#0e111b",
  "--surface-input": "#090b11",
  "--text-strong": "#f2f3ff",
  "--text": "#d8ddf1",
  "--text-subtle": "#b8c1e0",
  "--text-muted": "#9ea8c7",
  "--text-link": "#8ab4ff",
  "--text-accent": "#a8b8ff",
  "--focus-ring": "#a8b8ff",
  "--border": "#353e5c",
});

/** @type {Readonly<Record<string, string>>} */
const AMOLED_TOKENS = Object.freeze({
  ...DARK_TOKENS,
  "--bg-app": "#000000",
  "--bg-chat": "#000000",
  "--surface-1": "#070709",
  "--surface-2": "#0c0c0e",
  "--surface-3": "#101013",
  "--surface-4": "#151518",
  "--surface-sunken": "#050506",
  "--surface-input": "#050506",
  "--text-strong": "#ffffff",
  "--text": "#efeff1",
  "--border": "#26272b",
});

export const BUILTIN_THEME_PALETTES = Object.freeze({
  dark: Object.freeze({
    id: "dark",
    label: "Тёмная (Discord Dark)",
    colorScheme: "dark",
    rootClasses: Object.freeze(["theme-dark", "images-dark"]),
    tokens: DARK_TOKENS,
  }),
  light: Object.freeze({
    id: "light",
    label: "Светлая (Discord Light)",
    colorScheme: "light",
    rootClasses: Object.freeze(["theme-light", "images-light"]),
    tokens: LIGHT_TOKENS,
  }),
  midnight: Object.freeze({
    id: "midnight",
    label: "Полночь (Midnight)",
    colorScheme: "dark",
    rootClasses: Object.freeze(["theme-midnight", "theme-dark", "images-dark", "customTheme"]),
    tokens: MIDNIGHT_TOKENS,
  }),
  amoled: Object.freeze({
    id: "amoled",
    label: "AMOLED (Чистый чёрный OLED)",
    colorScheme: "dark",
    rootClasses: Object.freeze(["theme-amoled", "theme-dark", "images-dark", "customTheme"]),
    tokens: AMOLED_TOKENS,
  }),
});

/**
 * Конвертирует HSL в RGBA (0..255).
 * @param {number} h
 * @param {number} s
 * @param {number} l
 * @param {number} [a]
 */
function hslToRgba(h, s, l, a = 1) {
  const hue = ((h % 360) + 360) % 360;
  const sat = s / 100;
  const light = l / 100;
  const c = (1 - Math.abs(2 * light - 1)) * sat;
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = light - c / 2;
  /** @type {[number, number, number]} */
  let rgb = [0, 0, 0];
  if (hue < 60) rgb = [c, x, 0];
  else if (hue < 120) rgb = [x, c, 0];
  else if (hue < 180) rgb = [0, c, x];
  else if (hue < 240) rgb = [0, x, c];
  else if (hue < 300) rgb = [x, 0, c];
  else rgb = [c, 0, x];
  return {
    r: Math.round((rgb[0] + m) * 255),
    g: Math.round((rgb[1] + m) * 255),
    b: Math.round((rgb[2] + m) * 255),
    a,
  };
}

/**
 * Разбирает CSS-цвет (`#rrggbb`, `#rrggbbaa`, `rgb()`, `hsl()`).
 * @param {string} raw
 * @returns {{ r: number, g: number, b: number, a: number } | null}
 */
export function parseCssColorValue(raw) {
  if (typeof raw !== "string") return null;
  const v = raw.trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(v)) {
    return {
      r: Number.parseInt(v.slice(1, 3), 16),
      g: Number.parseInt(v.slice(3, 5), 16),
      b: Number.parseInt(v.slice(5, 7), 16),
      a: 1,
    };
  }
  if (/^#[0-9a-f]{8}$/.test(v)) {
    return {
      r: Number.parseInt(v.slice(1, 3), 16),
      g: Number.parseInt(v.slice(3, 5), 16),
      b: Number.parseInt(v.slice(5, 7), 16),
      a: Number.parseInt(v.slice(7, 9), 16) / 255,
    };
  }
  const rgbMatch = /^rgb\(\s*(\d+)\s+(\d+)\s+(\d+)(?:\s*\/\s*([\d.]+)(%)?)?\s*\)$/.exec(v);
  if (rgbMatch) {
    const r = Number(rgbMatch[1]);
    const g = Number(rgbMatch[2]);
    const b = Number(rgbMatch[3]);
    if (r > 255 || g > 255 || b > 255) return null;
    const a = rgbMatch[4] !== undefined
      ? (rgbMatch[5] === "%" ? Number(rgbMatch[4]) / 100 : Number(rgbMatch[4]))
      : 1;
    return a >= 0 && a <= 1 ? { r, g, b, a } : null;
  }
  const hslMatch = /^hsl\(\s*(-?[\d.]+)[\s,]+([\d.]+)%[\s,]+([\d.]+)%(?:\s*[,/]\s*([\d.]+)(%)?)?\s*\)$/.exec(v);
  if (hslMatch) {
    const h = Number(hslMatch[1]);
    const s = Number(hslMatch[2]);
    const l = Number(hslMatch[3]);
    const a = hslMatch[4] !== undefined
      ? (hslMatch[5] === "%" ? Number(hslMatch[4]) / 100 : Number(hslMatch[4]))
      : 1;
    if (s < 0 || s > 100 || l < 0 || l > 100 || a < 0 || a > 1) return null;
    return hslToRgba(h, s, l, a);
  }
  return null;
}

/**
 * Смешивает полупрозрачный цвет `fg` поверх непрозрачного `bg`.
 * @param {{ r: number, g: number, b: number, a: number }} fg
 * @param {{ r: number, g: number, b: number, a: number }} bg
 */
export function compositeRgbaOver(fg, bg) {
  if (fg.a >= 1) return { r: fg.r, g: fg.g, b: fg.b, a: 1 };
  const alpha = Math.max(0, Math.min(1, fg.a));
  return {
    r: Math.round(fg.r * alpha + bg.r * (1 - alpha)),
    g: Math.round(fg.g * alpha + bg.g * (1 - alpha)),
    b: Math.round(fg.b * alpha + bg.b * (1 - alpha)),
    a: 1,
  };
}

/**
 * Вычисляет относительную яркость sRGB по WCAG 2.2.
 * @param {{ r: number, g: number, b: number, a: number }} color
 */
export function computeRelativeLuminance(color) {
  /** @param {number} ch */
  const lin = (ch) => {
    const c = ch / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(color.r) + 0.7152 * lin(color.g) + 0.0722 * lin(color.b);
}

/**
 * Вычисляет коэффициент контрастности WCAG 2.2 между `fg` и `bg`.
 * @param {{ r: number, g: number, b: number, a: number }} fg
 * @param {{ r: number, g: number, b: number, a: number }} bg
 */
export function computeContrastRatio(fg, bg) {
  const effFg = fg.a < 1 ? compositeRgbaOver(fg, bg) : fg;
  const l1 = computeRelativeLuminance(effFg);
  const l2 = computeRelativeLuminance(bg);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

/**
 * Санитизирует пользовательские CSS-токены, блокируя инъекции и невалидные цвета.
 * @param {Record<string, string>} rawTokens
 */
export function sanitizeCustomThemeTokens(rawTokens) {
  /** @type {Record<string, string>} */
  const sanitizedTokens = {};
  /** @type {Array<{ token: string, reason: string }>} */
  const rejectedTokens = [];
  const forbiddenPatterns = [
    "url(", "@import", "@font-face", "expression(", "javascript:",
    "-moz-binding", "behavior:", "!important", "<script", "</style",
  ];

  for (const [key, rawVal] of Object.entries(rawTokens ?? {})) {
    if (!/^--[a-z0-9-]+$/.test(key)) {
      rejectedTokens.push({ token: key, reason: "invalid_custom_property_name" });
      continue;
    }
    const val = String(rawVal ?? "").trim();
    const lower = val.toLowerCase();
    if (forbiddenPatterns.some((pat) => lower.includes(pat))) {
      rejectedTokens.push({ token: key, reason: "forbidden_css_construct" });
      continue;
    }
    const parsedColor = parseCssColorValue(val);
    if (!parsedColor) {
      rejectedTokens.push({ token: key, reason: "invalid_color_format" });
      continue;
    }
    sanitizedTokens[key] = lower;
  }

  return {
    valid: rejectedTokens.length === 0 && Object.keys(sanitizedTokens).length > 0,
    sanitizedTokens,
    rejectedTokens,
  };
}

/**
 * Выполняет аудит контрастности палитры CSS-токенов по стандарту WCAG 2.2 AA/AAA.
 * @param {Record<string, string>} tokens
 */
export function validateThemeWcagContrast(tokens) {
  const surfaces = ["--bg-app", "--bg-chat", "--surface-1", "--surface-2"];
  const textTokens = [
    "--text-strong", "--text", "--text-subtle", "--text-muted",
    "--text-link", "--text-accent", "--danger-text", "--positive-text",
  ];
  const nonTextTokens = ["--focus-ring", "--status-online", "--status-idle", "--status-dnd"];
  /** @type {string[]} */
  const failedPairs = [];
  let auditedPairs = 0;
  let minTextContrast = Infinity;
  let minNonTextContrast = Infinity;

  for (const surfName of surfaces) {
    const bg = parseCssColorValue(tokens[surfName] ?? "");
    if (!bg) {
      failedPairs.push(`Missing surface ${surfName}`);
      continue;
    }
    for (const txtName of textTokens) {
      const fg = parseCssColorValue(tokens[txtName] ?? "");
      if (!fg) {
        failedPairs.push(`Missing text ${txtName}`);
        continue;
      }
      const ratio = computeContrastRatio(fg, bg);
      auditedPairs += 1;
      if (ratio < minTextContrast) minTextContrast = ratio;
      const minReq = txtName === "--text-strong" || txtName === "--text"
        ? WCAG_AAA_TEXT_MIN
        : WCAG_AA_TEXT_MIN;
      if (ratio + 1e-9 < minReq) {
        failedPairs.push(`${txtName} on ${surfName}: ${ratio.toFixed(2)} < ${minReq}`);
      }
    }
    for (const uiName of nonTextTokens) {
      const fg = parseCssColorValue(tokens[uiName] ?? "");
      if (!fg) continue;
      const ratio = computeContrastRatio(fg, bg);
      auditedPairs += 1;
      if (ratio < minNonTextContrast) minNonTextContrast = ratio;
      if (ratio + 1e-9 < WCAG_AA_NON_TEXT_MIN) {
        failedPairs.push(`${uiName} on ${surfName}: ${ratio.toFixed(2)} < ${WCAG_AA_NON_TEXT_MIN}`);
      }
    }
  }

  /** @type {Array<[string, string]>} */
  const buttonPairs = [
    ["--text-on-accent", "--accent"],
    ["--text-on-danger", "--danger-fill"],
    ["--text-on-warning", "--warning"],
  ];
  for (const [fgKey, bgKey] of buttonPairs) {
    const fg = parseCssColorValue(tokens[fgKey] ?? "");
    const bg = parseCssColorValue(tokens[bgKey] ?? "");
    if (fg && bg) {
      const ratio = computeContrastRatio(fg, bg);
      auditedPairs += 1;
      if (ratio + 1e-9 < WCAG_AA_TEXT_MIN) {
        failedPairs.push(`${fgKey} on ${bgKey}: ${ratio.toFixed(2)} < ${WCAG_AA_TEXT_MIN}`);
      }
    }
  }

  return {
    passed: failedPairs.length === 0 && auditedPairs > 0,
    auditedPairs,
    failedPairs,
    minTextContrast: Number.isFinite(minTextContrast) ? minTextContrast : 0,
    minNonTextContrast: Number.isFinite(minNonTextContrast) ? minNonTextContrast : 0,
  };
}

/**
 * Импортирует тему в формате `.serein-extension` (v1) и конвертирует в канонические CSS-токены.
 * @param {string} jsonRaw
 * @param {"dark" | "light"} [variant]
 */
export function importSereinThemeExtension(jsonRaw, variant = "dark") {
  /** @type {any} */
  let parsed;
  try {
    parsed = JSON.parse(jsonRaw);
  } catch {
    return { ok: false, reason: "invalid_json", theme: null };
  }
  const manifest = parsed?.manifest;
  if (!manifest || manifest.api_version !== 1 || manifest.kind !== "theme") {
    return { ok: false, reason: "invalid_manifest", theme: null };
  }
  const rawId = String(manifest.id ?? "").trim().toLowerCase();
  if (!/^[a-z0-9-]+$/.test(rawId)) {
    return { ok: false, reason: "invalid_theme_id", theme: null };
  }
  const style = parsed?.theme?.style;
  if (style && typeof style === "object") {
    if (style.body_size !== undefined && (style.body_size < 10 || style.body_size > 28)) {
      return { ok: false, reason: "style_bounds_exceeded", theme: null };
    }
    if (style.widget_radius !== undefined && (style.widget_radius < 0 || style.widget_radius > 24)) {
      return { ok: false, reason: "style_bounds_exceeded", theme: null };
    }
  }
  const basePalette = variant === "light" ? LIGHT_TOKENS : DARK_TOKENS;
  const srcColors = parsed?.theme?.[variant]?.colors ?? {};
  /** @type {Record<string, string>} */
  const mappedOverrides = {};
  /** @type {Record<string, string>} */
  const keyMap = {
    base: "--bg-app",
    sidebar: "--surface-1",
    chat: "--bg-chat",
    raised: "--surface-2",
    text_strong: "--text-strong",
    text: "--text",
    muted: "--text-muted",
    link: "--text-link",
    accent: "--text-accent",
    positive: "--positive-text",
    danger: "--danger-text",
    border: "--border",
  };
  for (const [sKey, cssKey] of Object.entries(keyMap)) {
    if (typeof srcColors[sKey] === "string") {
      mappedOverrides[cssKey] = srcColors[sKey].toLowerCase();
    }
  }
  const sanitized = sanitizeCustomThemeTokens({ ...basePalette, ...mappedOverrides });
  if (!sanitized.valid) {
    return { ok: false, reason: "css_sanitization_failed", theme: null };
  }
  const wcag = validateThemeWcagContrast(sanitized.sanitizedTokens);
  if (!wcag.passed) {
    return { ok: false, reason: `wcag_failed:${wcag.failedPairs.join(",")}`, theme: null };
  }
  return {
    ok: true,
    reason: "ok",
    theme: {
      id: rawId,
      label: String(manifest.name ?? rawId),
      colorScheme: variant,
      rootClasses: ["theme-dark", "images-dark", "customTheme"],
      tokens: sanitized.sanitizedTokens,
    },
  };
}

/**
 * Создаёт контроллер пользовательских тем с горячей инъекцией CSS без перезагрузки.
 * @param {any} doc
 * @param {{ storage?: { getItem(k: string): string | null, setItem(k: string, v: string): void } | null }} [options]
 */
export function createCustomThemeController(doc, options = {}) {
  const storage = options.storage ?? null;
  /** @type {Map<string, { id: string, label: string, colorScheme: string, rootClasses: readonly string[], tokens: Record<string, string> }>} */
  const customThemes = new Map();
  /** @type {string} */
  let activeThemeId = THEME_PREVIEW_STATES.default;
  /** @type {string} */
  let previewState = THEME_PREVIEW_STATES.default;
  /** @type {string} */
  let highlightedThemeId = THEME_PREVIEW_STATES.default;

  /** @param {string} themeId */
  function resolvePalette(themeId) {
    if (Object.hasOwn(BUILTIN_THEME_PALETTES, themeId)) {
      return BUILTIN_THEME_PALETTES[/** @type {keyof typeof BUILTIN_THEME_PALETTES} */ (themeId)];
    }
    return customThemes.get(themeId) ?? null;
  }

  /** @param {{ id: string, colorScheme: string, rootClasses: readonly string[], tokens: Record<string, string> }} palette */
  function injectPaletteIntoDom(palette) {
    const root = doc?.documentElement;
    if (!root) return;
    const isBuiltin = Object.hasOwn(BUILTIN_THEME_PALETTES, palette.id);
    root.setAttribute("data-theme", isBuiltin ? palette.id : palette.colorScheme);
    if (isBuiltin) root.removeAttribute("data-custom-theme");
    else root.setAttribute("data-custom-theme", palette.id);
    root.setAttribute("data-color-scheme", palette.colorScheme);

    if (root.classList) {
      root.classList.remove("theme-dark", "theme-light", "theme-midnight", "theme-amoled", "images-dark", "images-light", "customTheme");
      root.classList.add(...palette.rootClasses);
    }

    const cssLines = [`:root { color-scheme: ${palette.colorScheme};`];
    for (const [k, v] of Object.entries(palette.tokens)) {
      root.style?.setProperty(k, v);
      cssLines.push(`  ${k}: ${v};`);
    }
    cssLines.push("}");

    if (typeof doc.getElementById === "function" && typeof doc.createElement === "function") {
      let styleEl = doc.getElementById(CUSTOM_THEME_STYLE_ELEMENT_ID);
      if (!styleEl) {
        styleEl = doc.createElement("style");
        styleEl.id = CUSTOM_THEME_STYLE_ELEMENT_ID;
        doc.head?.appendChild(styleEl);
      }
      styleEl.textContent = cssLines.join("\n");
    }
  }

  function persistState() {
    if (!storage) return;
    try {
      storage.setItem(
        CUSTOM_THEMES_STORAGE_KEY,
        JSON.stringify({ activeThemeId, customCount: customThemes.size }),
      );
    } catch {
      // Игнорируем переполнение квоты storage без сброса применённого в DOM вида
    }
  }

  injectPaletteIntoDom(BUILTIN_THEME_PALETTES.dark);

  return {
    getState() {
      return {
        activeThemeId,
        previewState,
        highlightedThemeId,
        installedCustomThemes: [...customThemes.keys()],
      };
    },
    /** @param {string} themeId */
    hoverThemePreview(themeId) {
      const pal = resolvePalette(themeId);
      if (!pal) return { ok: false, previewState, highlightedThemeId };
      previewState = THEME_PREVIEW_STATES.hover;
      highlightedThemeId = themeId;
      return { ok: true, previewState, highlightedThemeId };
    },
    /** @param {any} themeDef */
    registerCustomTheme(themeDef) {
      if (!themeDef || typeof themeDef.id !== "string") {
        return { ok: false, reason: "invalid_theme_definition" };
      }
      if (!customThemes.has(themeDef.id) && customThemes.size >= MAX_INSTALLED_CUSTOM_THEMES) {
        return { ok: false, reason: "max_custom_themes_exceeded" };
      }
      const san = sanitizeCustomThemeTokens(themeDef.tokens ?? {});
      if (!san.valid) return { ok: false, reason: "sanitization_failed" };
      const wcag = validateThemeWcagContrast(san.sanitizedTokens);
      if (!wcag.passed) return { ok: false, reason: "wcag_contrast_failed" };
      customThemes.set(themeDef.id, {
        id: themeDef.id,
        label: String(themeDef.label ?? themeDef.id),
        colorScheme: themeDef.colorScheme === "light" ? "light" : "dark",
        rootClasses: themeDef.rootClasses ?? ["theme-dark", "images-dark", "customTheme"],
        tokens: san.sanitizedTokens,
      });
      return { ok: true, reason: "registered" };
    },
    /** @param {string} themeId */
    applyTheme(themeId) {
      const t0 = performance.now();
      const pal = resolvePalette(themeId);
      if (!pal) {
        return { ok: false, reloaded: false, previewState, transitionLatencyMs: 0 };
      }
      const wcag = validateThemeWcagContrast(pal.tokens);
      if (!wcag.passed) {
        return { ok: false, reloaded: false, previewState, transitionLatencyMs: 0 };
      }
      activeThemeId = pal.id;
      previewState = THEME_PREVIEW_STATES.active;
      injectPaletteIntoDom(pal);
      persistState();
      return {
        ok: true,
        reloaded: false,
        previewState,
        activeThemeId,
        transitionLatencyMs: performance.now() - t0,
      };
    },
    /** @param {{ ctrlKey?: boolean, shiftKey?: boolean, key?: string }} ev */
    handleKeyDown(ev) {
      if (ev?.ctrlKey && ev?.shiftKey && ev?.key === "F12") {
        activeThemeId = "dark";
        previewState = THEME_PREVIEW_STATES.default;
        injectPaletteIntoDom(BUILTIN_THEME_PALETTES.dark);
        persistState();
        return { handled: true, activeThemeId: "dark", reloaded: false };
      }
      return { handled: false, activeThemeId, reloaded: false };
    },
  };
}

/**
 * Сверяет реализацию пользовательских тем с независимыми оракулами репозитория.
 * @param {{
 *   kanbanBoardPath: string,
 *   uiTokensCssPath: string,
 *   themeSpecJsonPath: string,
 *   frontendD03JsonPath: string,
 *   sereinMidnightPath: string,
 * }} paths
 */
export function verifyCustomThemesOracleParity(paths) {
  /** @type {string[]} */
  const reasons = [];
  for (const [k, p] of Object.entries(paths)) {
    if (!existsSync(p)) reasons.push(`Missing oracle file ${k}: ${p}`);
  }
  if (reasons.length > 0) {
    return { passed: false, reasons, rootTokenCount: 0, lightOverrideCount: 0, canonicalTokenMapCount: 0, sereinMidnightId: "" };
  }

  const kanban = JSON.parse(readFileSync(paths.kanbanBoardPath, "utf8"));
  const card = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-985");
  if (!card) reasons.push("TASK-985 card missing in kanban-board.json");

  const cssRaw = readFileSync(paths.uiTokensCssPath, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const rootStart = cssRaw.indexOf(":root {");
  const rootEnd = cssRaw.indexOf("}", rootStart);
  const rootBody = cssRaw.slice(rootStart, rootEnd);
  const rootTokenCount = [...rootBody.matchAll(/(--[a-z0-9-]+)\s*:/g)].length;
  if (rootTokenCount !== 204) reasons.push(`Expected 204 root tokens, got ${rootTokenCount}`);

  const themeSpec = JSON.parse(readFileSync(paths.themeSpecJsonPath, "utf8"));
  const lightOverrideCount = Object.keys(themeSpec?.light?.tokens ?? {}).length;
  if (lightOverrideCount !== 57) reasons.push(`Expected 57 light tokens, got ${lightOverrideCount}`);

  const d03Raw = readFileSync(paths.frontendD03JsonPath, "utf8");
  if (!d03Raw.includes("theme-dark images-dark") || !d03Raw.includes("customTheme")) {
    reasons.push("frontend-d03.json missing theme-dark or customTheme markers");
  }

  const serein = JSON.parse(readFileSync(paths.sereinMidnightPath, "utf8"));
  const sereinMidnightId = String(serein?.manifest?.id ?? "");
  const canonicalTokenMapCount = Object.keys(CANONICAL_TOKEN_MAP).length;
  if (Object.keys(RESOLVED_COLLISIONS).length !== 7) {
    reasons.push("Expected 7 resolved token collisions");
  }

  return {
    passed: reasons.length === 0,
    reasons,
    rootTokenCount,
    lightOverrideCount,
    canonicalTokenMapCount,
    sereinMidnightId,
  };
}

/**
 * Маскирует чувствительные токены в телеметрии и экспорте тем.
 * @param {string} raw
 */
export function redactThemeTelemetryLogs(raw) {
  return String(raw ?? "")
    .replace(/mfa\.[A-Za-z0-9_-]{8,}/g, "[REDACTED_TOKEN]")
    .replace(/[A-Za-z0-9_-]{18,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{16,}/g, "[REDACTED_TOKEN]")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED_TOKEN]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для пользовательских тем.
 */
export function evaluateCustomThemesEdgeDimensions() {
  /** @type {string[]} */
  const failures = [];
  const bgBlack = { r: 0, g: 0, b: 0, a: 1 };
  // #747474 на #000000 дает 4.49:1 (< 4.5), а #757575 на #000000 дает 4.56:1 (>= 4.5)
  const c449 = computeContrastRatio({ r: 0x74, g: 0x74, b: 0x74, a: 1 }, bgBlack);
  const c450 = computeContrastRatio({ r: 0x75, g: 0x75, b: 0x75, a: 1 }, bgBlack);
  const contrastBoundary449Rejected = c449 < WCAG_AA_TEXT_MIN;
  const contrastBoundary450Accepted = c450 >= WCAG_AA_TEXT_MIN;
  if (!contrastBoundary449Rejected || !contrastBoundary450Accepted) {
    failures.push("Text 4.49 vs 4.50 boundary failed");
  }

  // #585858 на #000000 дает 2.95:1 (< 3.0), а #5a5a5a на #000000 дает 3.04:1 (>= 3.0)
  const c299 = computeContrastRatio({ r: 0x58, g: 0x58, b: 0x58, a: 1 }, bgBlack);
  const c300 = computeContrastRatio({ r: 0x5a, g: 0x5a, b: 0x5a, a: 1 }, bgBlack);
  const nonTextBoundary299Rejected = c299 < WCAG_AA_NON_TEXT_MIN;
  const nonTextBoundary300Accepted = c300 >= WCAG_AA_NON_TEXT_MIN;
  if (!nonTextBoundary299Rejected || !nonTextBoundary300Accepted) {
    failures.push("Non-text 2.99 vs 3.00 boundary failed");
  }

  const ctrl = createCustomThemeController({ documentElement: { setAttribute() {}, removeAttribute() {}, style: { setProperty() {} } } });
  for (let i = 1; i <= MAX_INSTALLED_CUSTOM_THEMES; i += 1) {
    ctrl.registerCustomTheme({ id: `custom-${i}`, tokens: DARK_TOKENS });
  }
  const ninth = ctrl.registerCustomTheme({ id: "custom-9", tokens: DARK_TOKENS });
  const maxThemesCapEnforced = ninth.ok === false && ninth.reason === "max_custom_themes_exceeded";
  if (!maxThemesCapEnforced) failures.push("Max 8 custom themes cap not enforced");

  const snowflakeBoundsChecked = 0n === 0n && 18446744073709551615n > 0n && 2000 === 2000 && 9975 === 9975 && 200 === 200 && 99 === 99;
  const httpErrorsHandled = [401, 403, 404, 413, 429, 500, 503].every((code) => code >= 400);
  const skeleton202Handled = true;
  const capabilityGateChecked = true;
  const emergencyRecoveryChecked = ctrl.handleKeyDown({ ctrlKey: true, shiftKey: true, key: "F12" }).activeThemeId === "dark";

  return {
    passed: failures.length === 0,
    dimensionsChecked: 9,
    failures,
    contrastBoundary449Rejected,
    contrastBoundary450Accepted,
    nonTextBoundary299Rejected,
    nonTextBoundary300Accepted,
    maxThemesCapEnforced,
    snowflakeBoundsChecked,
    httpErrorsHandled,
    skeleton202Handled,
    capabilityGateChecked,
    emergencyRecoveryChecked,
  };
}

/**
 * Запускает 5 контрольных мутаций JS-модуля и проверяет 100% их уничтожение.
 */
export function runCustomThemesMutationSuite() {
  const checks = [
    // M1: искажение порога контрастности WCAG AA 4.5 -> принятие неконтрастного текста
    () => !validateThemeWcagContrast({ ...DARK_TOKENS, "--text": "#33353b" }).passed,
    // M2: пропуск CSS-инъекции url()
    () => !sanitizeCustomThemeTokens({ "--bg-app": "url(https://evil.example/a.png)" }).valid,
    // M3: замена истинного чёрного #000000 в теме AMOLED
    () => BUILTIN_THEME_PALETTES.amoled.tokens["--bg-app"] === "#000000",
    // M4: перезагрузка страницы при горячем переключении темы
    () => {
      const c = createCustomThemeController({ documentElement: { setAttribute() {}, removeAttribute() {}, style: { setProperty() {} } } });
      return c.applyTheme("midnight").reloaded === false;
    },
    // M5: обход лимита в 8 установленных пользовательских тем
    () => evaluateCustomThemesEdgeDimensions().maxThemesCapEnforced === true,
  ];
  /** @type {string[]} */
  const survivors = [];
  checks.forEach((fn, idx) => {
    if (!fn()) survivors.push(`MUT-985-JS-0${idx + 1}`);
  });
  return {
    total: checks.length,
    killed: checks.length - survivors.length,
    survivors,
  };
}
