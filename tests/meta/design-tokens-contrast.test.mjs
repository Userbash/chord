// @ts-check
/**
 * TASK-631: Design Tokens V2, Dark/Light/Midnight Theme WCAG 2.2 Contrast Audit,
 * Breakpoints Spec & Meta Test Sync.
 *
 * Contract tokens: `WCAG 2.2`, `design-tokens-v2.md`, `DESIGN-TOKENS-V2.md`,
 * `contrastRatio`, `relativeLuminance`, `Midnight`, `4.5`, `3.0`.
 *
 * Проверяет:
 *  1. Валидность синтаксиса всех 204 базовых токенов и всех цветов (hex/rgb/hsl/var)
 *     в `rust/gui-spike/ui/tokens.css`, отсутствие циклов алиасов, внешних URL и @font-face.
 *  2. Строгий расчёт относительной яркости sRGB и контрастности по WCAG 2.2 AA/AAA:
 *     - обычный текст >= 4.5:1 (AAA >= 7.0:1 для основного текста) на всех поверхностях;
 *     - крупные элементы, нетекстовые UI-индикаторы, статус-точки и кольцо фокуса >= 3.0:1;
 *     - пары текст/заливка кнопок и бейджей (`--text-on-accent`, `--text-on-danger`, `--text-on-warning`, `--avatar-fg`) >= 4.5:1;
 *     - детерминированные фоны аватаров `hsl(0..359, 45%, 32%)` с `#ffffff` >= 4.5:1 на всём круге оттенков.
 *  3. Контрастность и полноту трёх тем (Dark, Light, Midnight/OLED) и сверку с
 *     эталонным RE-оракулом `/home/sanya/re-deep/frontend-d03.md` и `waves/RW-40/theme-spec.json`.
 *  4. Синхронизацию спецификации `docs/specs/design-tokens-v2.md`, отчёта аудита
 *     `waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md`, брейкпоинтов `1600/1200/960/720/<720`
 *     (`tests/meta/design-breakpoints.test.mjs`) и карты токенов `token-map.js`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  CANONICAL_TOKEN_MAP,
  RESOLVED_COLLISIONS,
} from "../../rust/gui-spike/ui/features/token-map.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const TOKENS_CSS_PATH = join(ROOT, "rust/gui-spike/ui/tokens.css");
const STYLE_CSS_PATH = join(ROOT, "rust/gui-spike/ui/style.css");
const THEME_SPEC_JSON_PATH = join(ROOT, "waves/RW-40/theme-spec.json");
const SPEC_V2_PATH = join(ROOT, "docs/specs/design-tokens-v2.md");
const REPORT_V2_PATH = join(ROOT, "waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md");
const BREAKPOINTS_TEST_PATH = join(ROOT, "tests/meta/design-breakpoints.test.mjs");
const RE_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.md";

/**
 * @typedef {{ r: number, g: number, b: number, a: number }} RgbaColor
 */

/**
 * Удаляет CSS-комментарии из строки.
 * @param {string} css
 * @returns {string}
 */
export function stripCssComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Извлекает базовый блок `:root { ... }` из `tokens.css`.
 * @param {string} css
 * @returns {Map<string, string>}
 */
export function parseRootTokens(css) {
  const clean = stripCssComments(css);
  const start = clean.indexOf(":root {");
  assert.notEqual(start, -1, "В tokens.css отсутствует базовый блок :root {");
  let depth = 0;
  let body = "";
  for (let i = start + ":root ".length; i < clean.length; i += 1) {
    const ch = clean[i];
    if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) {
        body = clean.slice(start + ":root {".length, i);
        break;
      }
    }
  }
  /** @type {Map<string, string>} */
  const map = new Map();
  for (const match of body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    const name = (match[1] ?? "").trim();
    const val = (match[2] ?? "").replace(/\s+/g, " ").trim();
    assert.equal(map.has(name), false, `Дублирование токена ${name} в базовом :root`);
    map.set(name, val);
  }
  return map;
}

/**
 * Разрешает значение токена с учётом ссылок `var(--other)`, защищая от циклов.
 * @param {ReadonlyMap<string, string>} tokens
 * @param {string} name
 * @param {number} [depth]
 * @returns {string}
 */
export function resolveTokenValue(tokens, name, depth = 0) {
  if (depth > 8) {
    throw new Error(`Циклическая ссылка токенов при разрешении ${name}`);
  }
  const raw = tokens.get(name);
  if (raw === undefined) {
    throw new Error(`Токен ${name} не найден в карте токенов`);
  }
  const refMatch = /^var\(\s*(--[a-z0-9-]+)\s*\)$/.exec(raw);
  if (refMatch) {
    return resolveTokenValue(tokens, refMatch[1] ?? "", depth + 1);
  }
  return raw;
}

/**
 * Конвертирует HSL (`h` 0..360, `s` 0..100, `l` 0..100) в RGB `0..255`.
 * @param {number} h
 * @param {number} s
 * @param {number} l
 * @param {number} [a]
 * @returns {RgbaColor}
 */
export function hslToRgba(h, s, l, a = 1) {
  if (!Number.isFinite(h) || !Number.isFinite(s) || !Number.isFinite(l) || !Number.isFinite(a)) {
    throw new Error("Некорректные числовые параметры HSL");
  }
  if (s < 0 || s > 100 || l < 0 || l > 100 || a < 0 || a > 1) {
    throw new Error(`Выход за границы диапазона HSL: s=${s}, l=${l}, a=${a}`);
  }
  const hue = ((h % 360) + 360) % 360;
  const sat = s / 100;
  const light = l / 100;
  const chroma = (1 - Math.abs(2 * light - 1)) * sat;
  const x = chroma * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = light - chroma / 2;
  /** @type {[number, number, number]} */
  let rgb = [0, 0, 0];
  if (hue < 60) rgb = [chroma, x, 0];
  else if (hue < 120) rgb = [x, chroma, 0];
  else if (hue < 180) rgb = [0, chroma, x];
  else if (hue < 240) rgb = [0, x, chroma];
  else if (hue < 300) rgb = [x, 0, chroma];
  else rgb = [chroma, 0, x];
  return {
    r: Math.round((rgb[0] + m) * 255),
    g: Math.round((rgb[1] + m) * 255),
    b: Math.round((rgb[2] + m) * 255),
    a,
  };
}

/**
 * Парсит CSS-цвет (`#rrggbb`, `rgb(r g b / a%)`, `hsl(h s% l% / a)`).
 * Возвращает `null`, если строка не является валидным цветом.
 * @param {string} raw
 * @returns {RgbaColor | null}
 */
export function parseCssColor(raw) {
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
  const rgbMatch = /^rgb\(\s*(\d+)\s+(\d+)\s+(\d+)(?:\s*\/\s*([\d.]+)(%)?)?\s*\)$/.exec(v);
  if (rgbMatch) {
    const r = Number(rgbMatch[1]);
    const g = Number(rgbMatch[2]);
    const b = Number(rgbMatch[3]);
    if (r < 0 || r > 255 || g < 0 || g > 255 || b < 0 || b > 255) return null;
    let a = 1;
    if (rgbMatch[4] !== undefined) {
      const rawA = Number(rgbMatch[4]);
      a = rgbMatch[5] === "%" ? rawA / 100 : rawA;
      if (!Number.isFinite(a) || a < 0 || a > 1) return null;
    }
    return { r, g, b, a };
  }
  const cleanedHsl = v.replace(/calc\(\s*1\s*\*\s*([\d.]+)%\s*\)/g, "$1%");
  const hslMatch =
    /^hsl\(\s*(-?[\d.]+)(?:deg)?[\s,]+([\d.]+)%[\s,]+([\d.]+)%(?:\s*[,/]\s*([\d.]+)(%)?)?\s*\)$/.exec(
      cleanedHsl,
    );
  if (hslMatch) {
    const h = Number(hslMatch[1]);
    const s = Number(hslMatch[2]);
    const l = Number(hslMatch[3]);
    let a = 1;
    if (hslMatch[4] !== undefined) {
      const rawA = Number(hslMatch[4]);
      a = hslMatch[5] === "%" ? rawA / 100 : rawA;
    }
    if (s < 0 || s > 100 || l < 0 || l > 100 || a < 0 || a > 1) return null;
    return hslToRgba(h, s, l, a);
  }
  return null;
}

/**
 * Смешивает полупрозрачный цвет переднего плана `fg` с непрозрачным фоном `bg` в sRGB.
 * @param {RgbaColor} fg
 * @param {RgbaColor} bg
 * @returns {RgbaColor}
 */
export function compositeOver(fg, bg) {
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
 * Вычисляет относительную яркость sRGB по формуле WCAG 2.2:
 * L = 0.2126 * R + 0.7152 * G + 0.0722 * B.
 * @param {RgbaColor} color
 * @returns {number}
 */
export function relativeLuminance(color) {
  /** @param {number} channel */
  const linearize = (channel) => {
    const c = channel / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return (
    0.2126 * linearize(color.r) +
    0.7152 * linearize(color.g) +
    0.0722 * linearize(color.b)
  );
}

/**
 * Вычисляет коэффициент контрастности WCAG 2.2 между `fg` и `bg` (1.0 .. 21.0).
 * Если `fg` имеет альфа-канал (< 1), он предварительно композируется поверх `bg`.
 * @param {RgbaColor} fg
 * @param {RgbaColor} bg
 * @returns {number}
 */
export function contrastRatio(fg, bg) {
  const effectiveFg = fg.a < 1 ? compositeOver(fg, bg) : fg;
  const l1 = relativeLuminance(effectiveFg);
  const l2 = relativeLuminance(bg);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

/** Поверхности тёмной темы (`rust/gui-spike/ui/tokens.css`). */
const DARK_SURFACE_TOKENS = [
  "--bg-app",
  "--bg-chat",
  "--surface-1",
  "--surface-2",
  "--surface-3",
  "--surface-4",
  "--surface-sunken",
  "--surface-input",
];

/** Текстовые токены, обязанные проходить WCAG 2.2 AA (>= 4.5:1) на всех 8 поверхностях. */
const UNIVERSAL_TEXT_TOKENS = [
  "--text-strong",
  "--text",
  "--text-subtle",
  "--text-muted",
  "--text-link",
  "--text-accent",
  "--danger-text",
  "--positive-text",
  "--info",
];

/** Палитра Midnight (OLED / Onyx) на базе нейтральной шкалы `/home/sanya/re-deep/ui/tokens.json`. */
const MIDNIGHT_SURFACES = Object.freeze({
  "--bg-app": "#000000", // --neutral-100 / --black-500
  "--bg-chat": "#070709", // --neutral-97
  "--surface-1": "#0c0c0e", // --neutral-95 / --primary-830
  "--surface-2": "#101013", // --neutral-93
  "--surface-3": "#151518", // --neutral-90
  "--surface-4": "#17181b", // --neutral-88
  "--surface-sunken": "#0a0a0c", // --neutral-96
  "--surface-input": "#050506", // --neutral-98 / --primary-860
});

describe("TASK-631: 1. Валидация синтаксиса и цветовых форматов rust/gui-spike/ui/tokens.css", () => {
  const rawCss = readFileSync(TOKENS_CSS_PATH, "utf8");
  const rootTokens = parseRootTokens(rawCss);

  it("базовый :root содержит все 204 канонических токена без внешних ссылок, @font-face и !important", () => {
    assert.equal(rootTokens.size, 204, `Ожидалось 204 токена в :root, найдено ${rootTokens.size}`);
    assert.equal(/url\s*\(/i.test(rawCss), false, "В tokens.css запрещены внешние ресурсы url()");
    assert.equal(/@font-face/i.test(rawCss), false, "В tokens.css запрещены @font-face");
    assert.equal(/!important/i.test(rawCss), false, "В tokens.css запрещён !important");
    assert.equal(/gg sans|ABC Ginto/i.test(stripCssComments(rawCss)), false, "Запрещены проприетарные шрифты");
  });

  it("все алиасы var(--*) разрешаются за <= 8 шагов, а циклы и неизвестные ссылки отвергаются", () => {
    for (const name of rootTokens.keys()) {
      const resolved = resolveTokenValue(rootTokens, name);
      assert.ok(resolved.length > 0, `Пустое разрешённое значение у ${name}`);
      assert.equal(
        /^var\(--[a-z0-9-]+\)$/.test(resolved),
        false,
        `Алиас ${name} не разрешился до конечного значения: ${resolved}`,
      );
    }

    const cyclicMap = new Map([
      ["--a", "var(--b)"],
      ["--b", "var(--a)"],
    ]);
    assert.throws(() => resolveTokenValue(cyclicMap, "--a"), /Циклическая ссылка/);
    assert.throws(() => resolveTokenValue(cyclicMap, "--missing"), /не найден/);
  });

  it("все цветовые токены имеют строго валидный hex (#rrggbb) или rgb(r g b / a%), а битые форматы отвергаются", () => {
    const colorTokenNames = [
      ...DARK_SURFACE_TOKENS,
      ...UNIVERSAL_TEXT_TOKENS,
      "--text-faint",
      "--text-on-accent",
      "--text-on-danger",
      "--text-on-warning",
      "--text-inverse",
      "--accent",
      "--accent-hover",
      "--accent-active",
      "--accent-soft",
      "--danger",
      "--danger-fill",
      "--danger-fill-hover",
      "--warning",
      "--positive",
      "--status-online",
      "--status-idle",
      "--status-dnd",
      "--status-offline",
      "--status-streaming",
      "--border-subtle",
      "--border",
      "--border-strong",
      "--divider",
      "--state-hover",
      "--state-active",
      "--state-selected",
      "--state-selected-strong",
      "--focus-ring",
      "--scrollbar-thumb",
      "--avatar-fg",
      "--badge-secondary-bg",
    ];

    for (const tokenName of colorTokenNames) {
      const val = resolveTokenValue(rootTokens, tokenName);
      const parsed = parseCssColor(val);
      assert.notEqual(parsed, null, `Токен ${tokenName} имеет невалидный цветовой формат: ${val}`);
    }

    // Негативные и граничные проверки парсера цветов
    for (const invalid of [
      "",
      "#12345",
      "#1234567",
      "#gggggg",
      "rgb(256 0 0 / 50%)",
      "rgb(0 0 0 / 120%)",
      "hsl(200 120% 50%)",
      "hsl(200 50% 105%)",
      "not-a-color",
    ]) {
      assert.equal(parseCssColor(invalid), null, `Невалидный цвет ${invalid} должен отвергаться`);
    }
  });
});

describe("TASK-631: 2. Аудит контрастности WCAG 2.2 AA/AAA — Dark Theme (tokens.css)", () => {
  const rawCss = readFileSync(TOKENS_CSS_PATH, "utf8");
  const rootTokens = parseRootTokens(rawCss);

  /** @param {string} name @returns {RgbaColor} */
  const colorOf = (name) => {
    const c = parseCssColor(resolveTokenValue(rootTokens, name));
    assert.ok(c !== null, `Не удалось разобрать цвет ${name}`);
    return c;
  };

  it("все основные текстовые токены проходят WCAG 2.2 AA (>= 4.5:1) на всех 8 поверхностях Dark Theme", () => {
    for (const textToken of UNIVERSAL_TEXT_TOKENS) {
      const fg = colorOf(textToken);
      for (const surfaceToken of DARK_SURFACE_TOKENS) {
        const bg = colorOf(surfaceToken);
        const ratio = contrastRatio(fg, bg);
        assert.ok(
          ratio >= 4.5,
          `Dark Theme: контраст ${textToken} на ${surfaceToken} = ${ratio.toFixed(2)}:1 < 4.5:1 (WCAG 2.2 AA)`,
        );
      }
    }
  });

  it("--text-strong и --text проходят повышенный порог WCAG 2.2 AAA (>= 7.0:1) на всех 8 поверхностях", () => {
    for (const primaryText of ["--text-strong", "--text"]) {
      const fg = colorOf(primaryText);
      for (const surfaceToken of DARK_SURFACE_TOKENS) {
        const bg = colorOf(surfaceToken);
        const ratio = contrastRatio(fg, bg);
        assert.ok(
          ratio >= 7.0,
          `Dark Theme AAA: ${primaryText} на ${surfaceToken} = ${ratio.toFixed(2)}:1 < 7.0:1`,
        );
      }
    }
  });

  it("--text-faint проходит >= 4.5:1 на --bg-app (4.89) и --bg-chat (4.54) и запрещён на --surface-2 (4.04 < 4.5)", () => {
    const faint = colorOf("--text-faint");
    const onApp = contrastRatio(faint, colorOf("--bg-app"));
    const onChat = contrastRatio(faint, colorOf("--bg-chat"));
    const onSurface2 = contrastRatio(faint, colorOf("--surface-2"));
    assert.ok(onApp >= 4.5, `--text-faint на --bg-app = ${onApp.toFixed(2)} < 4.5`);
    assert.ok(onChat >= 4.5, `--text-faint на --bg-chat = ${onChat.toFixed(2)} < 4.5`);
    assert.ok(onSurface2 < 4.5, `--text-faint на --surface-2 должен быть < 4.5 (факт ${onSurface2.toFixed(2)})`);
  });

  it("пары текст-на-заливке кнопок и бейджей проходят >= 4.5:1, а --danger-fill исправляет дефицит --danger", () => {
    const onAccent = colorOf("--text-on-accent");
    assert.ok(contrastRatio(onAccent, colorOf("--accent")) >= 4.5);
    assert.ok(contrastRatio(onAccent, colorOf("--accent-hover")) >= 6.0);
    assert.ok(contrastRatio(onAccent, colorOf("--accent-active")) >= 7.0);

    const onDanger = colorOf("--text-on-danger");
    const fillRatio = contrastRatio(onDanger, colorOf("--danger-fill"));
    const rawDangerRatio = contrastRatio(onDanger, colorOf("--danger"));
    assert.ok(fillRatio >= 4.5, `--text-on-danger на --danger-fill = ${fillRatio.toFixed(2)} < 4.5`);
    assert.ok(
      rawDangerRatio < 4.5,
      `Сырой --danger (#da3e44) с белым текстом даёт ${rawDangerRatio.toFixed(2)} < 4.5, поэтому обязателен --danger-fill`,
    );
    assert.ok(contrastRatio(onDanger, colorOf("--danger-fill-hover")) >= 6.0);

    const onWarning = colorOf("--text-on-warning");
    assert.ok(contrastRatio(onWarning, colorOf("--warning")) >= 10.0);
    assert.ok(contrastRatio(colorOf("--text-inverse"), colorOf("--warning")) >= 7.0);
  });

  it("нетекстовые UI-компоненты, статус-индикаторы и кольцо фокуса проходят WCAG 2.2 AA (>= 3.0:1)", () => {
    const nonTextTokens = [
      "--focus-ring",
      "--danger",
      "--warning",
      "--positive",
      "--status-online",
      "--status-idle",
      "--status-dnd",
      "--status-offline",
    ];
    for (const token of nonTextTokens) {
      const fg = colorOf(token);
      for (const surface of ["--bg-app", "--bg-chat", "--surface-1", "--surface-2", "--surface-3", "--surface-4"]) {
        const ratio = contrastRatio(fg, colorOf(surface));
        assert.ok(
          ratio >= 3.0,
          `Non-text ${token} на ${surface} = ${ratio.toFixed(2)}:1 < 3.0:1 (WCAG 2.2 SC 1.4.11)`,
        );
      }
    }
    // Кольцо фокуса проходит даже текстовый порог >= 6.5:1 на всех поверхностях
    for (const surface of DARK_SURFACE_TOKENS) {
      assert.ok(contrastRatio(colorOf("--focus-ring"), colorOf(surface)) >= 6.5);
    }
  });

  it("аватары hsl(0..359, 45%, 32%) с белыми инициалами (--avatar-fg) гарантируют >= 4.5:1 для любого оттенка", () => {
    const fg = colorOf("--avatar-fg");
    const sat = Number.parseFloat(resolveTokenValue(rootTokens, "--avatar-s"));
    const light = Number.parseFloat(resolveTokenValue(rootTokens, "--avatar-l"));
    assert.equal(sat, 45);
    assert.equal(light, 32);

    let minRatio = Infinity;
    for (let hue = 0; hue < 360; hue += 1) {
      const bg = hslToRgba(hue, sat, light);
      const ratio = contrastRatio(fg, bg);
      if (ratio < minRatio) minRatio = ratio;
      assert.ok(ratio >= 4.5, `Аватар при hue=${hue} дал контраст ${ratio.toFixed(2)} < 4.5`);
    }
    assert.ok(minRatio >= 4.7, `Минимальный контраст аватара ${minRatio.toFixed(3)} должен быть >= 4.7`);
  });

  it("полупрозрачные состояния hover/active/selected не опускают контраст основного и вторичного текста ниже 4.5:1", () => {
    const text = colorOf("--text");
    const subtle = colorOf("--text-subtle");
    const muted = colorOf("--text-muted");
    for (const stateToken of ["--state-hover", "--state-active", "--state-selected", "--state-selected-strong"]) {
      const overlay = colorOf(stateToken);
      for (const baseSurface of ["--bg-app", "--bg-chat", "--surface-1", "--surface-2"]) {
        const blendedBg = compositeOver(overlay, colorOf(baseSurface));
        assert.ok(contrastRatio(text, blendedBg) >= 7.0);
        if (stateToken !== "--state-selected-strong") {
          assert.ok(contrastRatio(subtle, blendedBg) >= 4.5);
        }
      }
    }
    // --text-muted сохраняет >= 4.5:1 при наведении (--state-hover) на базовых поверхностях
    const hoverOverlay = colorOf("--state-hover");
    for (const baseSurface of ["--bg-app", "--bg-chat", "--surface-1"]) {
      const blendedHover = compositeOver(hoverOverlay, colorOf(baseSurface));
      assert.ok(contrastRatio(muted, blendedHover) >= 4.5);
    }
  });
});

describe("TASK-631: 3. Аудит контрастности WCAG 2.2 — Light Theme и Midnight (OLED) Theme", () => {
  const rawCss = readFileSync(TOKENS_CSS_PATH, "utf8");
  const rootTokens = parseRootTokens(rawCss);
  const themeSpec = JSON.parse(readFileSync(THEME_SPEC_JSON_PATH, "utf8"));

  it("Light Theme (57 переопределений): все текстовые, парные и нетекстовые токены проходят матрицу WCAG 2.2 AA", () => {
    const lightTokens = themeSpec.light.tokens;
    assert.equal(Object.keys(lightTokens).length, 57, "Light-тема должна содержать 57 токенов");

    /** @type {Map<string, RgbaColor>} */
    const lightColors = new Map();
    for (const [name, entry] of Object.entries(lightTokens)) {
      const parsed = parseCssColor(entry.value);
      if (parsed) lightColors.set(name, parsed);
    }

    let checkedPairs = 0;
    for (const [name, entry] of Object.entries(lightTokens)) {
      if (!entry.aa) continue;
      const fg = lightColors.get(name);
      assert.ok(fg, `Light-токен ${name} не является валидным цветом`);
      for (const targetName of entry.aa.against) {
        const bg = lightColors.get(targetName);
        assert.ok(bg, `Целевая поверхность ${targetName} не найдена в Light-теме`);
        const ratio = contrastRatio(fg, bg);
        assert.ok(
          ratio + 1e-9 >= entry.aa.min,
          `Light Theme: ${name} на ${targetName} = ${ratio.toFixed(3)} < ${entry.aa.min}`,
        );
        checkedPairs += 1;
      }
    }
    assert.equal(checkedPairs, 160, "Должно быть проверено ровно 160 пар контраста Light-темы");
  });

  it("Midnight (OLED) Theme: все 10 текстовых токенов (включая --text-faint) проходят >= 4.5:1 на всех 8 поверхностях", () => {
    const allMidnightTextTokens = [...UNIVERSAL_TEXT_TOKENS, "--text-faint"];
    for (const textToken of allMidnightTextTokens) {
      const fg = parseCssColor(resolveTokenValue(rootTokens, textToken));
      assert.ok(fg !== null);
      for (const [surfaceName, hex] of Object.entries(MIDNIGHT_SURFACES)) {
        const bg = parseCssColor(hex);
        assert.ok(bg !== null);
        const ratio = contrastRatio(fg, bg);
        assert.ok(
          ratio >= 4.5,
          `Midnight Theme: ${textToken} на ${surfaceName} (${hex}) = ${ratio.toFixed(2)}:1 < 4.5:1`,
        );
      }
    }

    // В Midnight теме основной и вторичный текст проходят AAA (>= 7.0:1) на всех поверхностях
    for (const aaaToken of ["--text-strong", "--text", "--text-subtle", "--text-accent", "--danger-text", "--positive-text"]) {
      const fg = parseCssColor(resolveTokenValue(rootTokens, aaaToken));
      assert.ok(fg !== null);
      for (const [surfaceName, hex] of Object.entries(MIDNIGHT_SURFACES)) {
        const bg = parseCssColor(hex);
        assert.ok(bg !== null);
        const ratio = contrastRatio(fg, bg);
        assert.ok(
          ratio >= 7.0,
          `Midnight Theme AAA: ${aaaToken} на ${surfaceName} = ${ratio.toFixed(2)}:1 < 7.0:1`,
        );
      }
    }
  });
});

describe("TASK-631: 4. Сверка с RE-оракулом frontend-d03.md, спекой design-tokens-v2.md и брейкпоинтами", () => {
  const rawCss = readFileSync(TOKENS_CSS_PATH, "utf8");
  const rootTokens = parseRootTokens(rawCss);

  it("эталон /home/sanya/re-deep/frontend-d03.md согласован с размерами и цветами 12 Mana-компонентов в tokens.css", () => {
    if (existsSync(RE_FRONTEND_D03_PATH)) {
      const d03 = readFileSync(RE_FRONTEND_D03_PATH, "utf8");
      assert.match(d03, /`--modal-width-small\/medium\/large`\s*\|\s*442px\s*\/\s*602px\s*\/\s*800px/);
      assert.match(d03, /`--modal-vertical-padding`\s*\|\s*16px/);
      assert.match(d03, /`--menu-scroller-block-padding`.*8px\s*\/\s*1px/);
      assert.match(d03, /2147483647px/);
    }
    assert.equal(resolveTokenValue(rootTokens, "--modal-w-sm"), "442px");
    assert.equal(resolveTokenValue(rootTokens, "--modal-w-md"), "602px");
    assert.equal(resolveTokenValue(rootTokens, "--modal-w-lg"), "800px");
    assert.equal(resolveTokenValue(rootTokens, "--modal-pad-y"), "16px");
    assert.equal(resolveTokenValue(rootTokens, "--menu-pad"), "8px");
    assert.equal(resolveTokenValue(rootTokens, "--menu-border"), "1px");
    assert.equal(resolveTokenValue(rootTokens, "--radius-round"), "2147483647px");
  });

  it("docs/specs/design-tokens-v2.md и waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md существуют и содержат полный контракт V2", () => {
    assert.equal(existsSync(SPEC_V2_PATH), true, "Отсутствует спецификация docs/specs/design-tokens-v2.md");
    assert.equal(existsSync(REPORT_V2_PATH), true, "Отсутствует отчёт waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md");

    const specText = readFileSync(SPEC_V2_PATH, "utf8");
    const reportText = readFileSync(REPORT_V2_PATH, "utf8");

    /** @type {Array<[string, string]>} */
    const docsToVerify = [
      ["design-tokens-v2.md", specText],
      ["DESIGN-TOKENS-V2.md", reportText],
    ];
    for (const [label, doc] of docsToVerify) {
      assert.equal(/url\s*\(/i.test(doc), false, `${label} не должен содержать url()`);
      assert.equal(/@font-face/i.test(doc), false, `${label} не должен содержать @font-face`);
      assert.match(doc, /WCAG 2\.2/, `${label}: нет раздела WCAG 2.2`);
      assert.match(doc, /Dark/i, `${label}: нет описания Dark theme`);
      assert.match(doc, /Light/i, `${label}: нет описания Light theme`);
      assert.match(doc, /Midnight/i, `${label}: нет описания Midnight theme`);
      // Все 5 брейкпоинтов shell
      for (const bp of ["1600", "1200", "960", "720", "311px", "256px", "240px", "200px"]) {
        assert.ok(doc.includes(bp), `${label}: отсутствует значение брейкпоинта ${bp}`);
      }
      // Все 7 разрешённых коллизий из token-map.js задокументированы
      for (const [localTok, discordTok] of Object.entries(RESOLVED_COLLISIONS)) {
        assert.ok(doc.includes(localTok), `${label}: не задокументирована коллизия ${localTok}`);
        assert.ok(doc.includes(discordTok), `${label}: не задокументирован целевой токен ${discordTok}`);
      }
    }

    for (const requiredSpecToken of [
      "#121214",
      "#efeff1",
      "#f7f7f8",
      "#3f4048",
      "#000000",
      "#070709",
      "#17181b",
      "442px",
      "602px",
      "800px",
      "2147483647px",
      "160",
    ]) {
      assert.ok(
        specText.includes(requiredSpecToken),
        `design-tokens-v2.md: отсутствует каноническое значение ${requiredSpecToken}`,
      );
    }

    assert.equal(Object.keys(CANONICAL_TOKEN_MAP).length, 25);
  });

  it("брейкпоинты в tokens.css, style.css, design-breakpoints.test.mjs и design-tokens-v2.md полностью синхронны", () => {
    const styleCss = stripCssComments(readFileSync(STYLE_CSS_PATH, "utf8"));
    const bpTest = readFileSync(BREAKPOINTS_TEST_PATH, "utf8");
    const specText = readFileSync(SPEC_V2_PATH, "utf8");

    assert.equal(resolveTokenValue(rootTokens, "--layout-rail-w"), "64px");
    assert.equal(resolveTokenValue(rootTokens, "--layout-tree-w"), "240px");
    assert.equal(resolveTokenValue(rootTokens, "--layout-tree-w-wide"), "311px");
    assert.equal(resolveTokenValue(rootTokens, "--layout-tree-min-w"), "200px");
    assert.equal(resolveTokenValue(rootTokens, "--layout-member-panel-w"), "240px");
    assert.equal(resolveTokenValue(rootTokens, "--layout-member-panel-w-wide"), "256px");

    assert.match(rawCss, /@media\s*\(min-width:\s*1600px\)/);
    assert.match(rawCss, /@media\s*\(max-width:\s*959px\)/);
    assert.match(styleCss, /@media\s*\(max-width:\s*1199px\)/);
    assert.match(styleCss, /@media\s*\(max-width:\s*719px\)/);

    for (const width of [2048, 1600, 1599, 1280, 1200, 1199, 960, 959, 720, 719, 600]) {
      assert.ok(bpTest.includes(String(width)), `design-breakpoints.test.mjs не проверяет ширину ${width}px`);
      assert.ok(specText.includes(String(width)), `design-tokens-v2.md не содержит ширину матрицы ${width}px`);
    }
  });
});
