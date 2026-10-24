// @ts-check
/**
 * DP-01 (RW-40/PARITY): адаптив shell по SPEC.md §5.1.
 *
 * Оракул — не текущий CSS, а правила SPEC:
 *   ≥1600px    tree 311, member 256 (`[L]` wide-токены);
 *   1200–1599  tree 240, member 240 (базовые токены);
 *   960–1199   member panel скрыт, tree 240, chat ≥ 520;
 *   720–959    tree 200 (`--layout-tree-min-w`), member скрыт;
 *   <720       tree скрыт, навигация только рейлом.
 *
 * Тест статический (без окна/live): читает `ui/tokens.css` и `ui/style.css`,
 * разбирает media-query и прогоняет каскад по ширинам. Дополнительно
 * фиксирует, что скрытие панелей делается только CSS (`display:none` в
 * media-query) и не вмешивается в `aria-hidden`/переключение экранов:
 * JS ставит inline-display только ""/none (не `flex`), а `aria-hidden`
 * выставляется исключительно на `.screen`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));

/** @param {string} name */
const readUi = (name) => readFileSync(`${uiDir}${name}`, "utf8");

/** @param {string} css */
function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Тело блока, открывающая скобка которого на индексе `open`.
 * @param {string} css @param {number} open @returns {string}
 */
function blockBody(css, open) {
  let depth = 0;
  for (let i = open; i < css.length; i += 1) {
    const ch = css[i];
    if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) return css.slice(open + 1, i);
    }
  }
  throw new Error("незакрытый блок CSS");
}

/**
 * Все `@media (...) { ... }` в порядке следования.
 * @param {string} css
 * @returns {{ query: string, body: string, index: number }[]}
 */
function mediaBlocks(css) {
  /** @type {{ query: string, body: string, index: number }[]} */
  const out = [];
  const re = /@media\s*\(([^)]*)\)\s*\{/g;
  for (const match of css.matchAll(re)) {
    const open = (match.index ?? 0) + match[0].length - 1;
    out.push({ query: (match[1] ?? "").trim(), body: blockBody(css, open), index: match.index ?? 0 });
  }
  return out;
}

/**
 * Правила внутри фрагмента CSS (одноуровневые: selector { body }).
 * @param {string} block
 * @returns {{ selector: string, body: string }[]}
 */
function cssRules(block) {
  /** @type {{ selector: string, body: string }[]} */
  const out = [];
  for (const match of block.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    out.push({
      selector: (match[1] ?? "").trim().replace(/\s+/g, " "),
      body: match[2] ?? "",
    });
  }
  return out;
}

/**
 * Значение custom property внутри `body` объявления.
 * @param {string} body @param {string} name @returns {string | null}
 */
function declaredValue(body, name) {
  const re = new RegExp(`(?:^|[;{])\\s*${name}\\s*:\\s*([^;]+);`);
  const match = re.exec(body);
  return match ? (match[1] ?? "").trim() : null;
}

/**
 * Применяется ли query к ширине. Поддерживаются только width-запросы —
 * этого достаточно для tokens.css/style.css; иное считаем ошибкой теста,
 * чтобы новые условия не проходили незамеченными.
 * @param {string} query @param {number} width @returns {boolean}
 */
function mediaMatches(query, width) {
  const parts = query.split(/\s+and\s+/).map((part) => part.trim());
  let matches = true;
  for (const part of parts) {
    const m = /^(min|max)-width\s*:\s*(\d+)px$/.exec(part);
    if (!m) {
      if (part.startsWith("prefers-")) continue;
      throw new Error(`неизвестное media-условие «${part}» (ожидалось min/max-width)`);
    }
    const limit = Number(m[2]);
    if (m[1] === "min") matches &&= width >= limit;
    else matches &&= width <= limit;
  }
  return matches;
}

/**
 * Каскад токенов: базовый `:root` + `:root` в media-блоках (в порядке файла).
 * @param {string} css
 * @returns {(width: number) => Map<string, string>}
 */
function tokenCascade(css) {
  const clean = stripComments(css);
  const baseOpen = clean.indexOf(":root {");
  assert.notEqual(baseOpen, -1, "в tokens.css нет базового :root");
  /** @type {Map<string, string>} */
  const base = new Map();
  for (const match of blockBody(clean, baseOpen + ":root ".length).matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    base.set(match[1] ?? "", (match[2] ?? "").trim());
  }
  /** @type {{ query: string | null, vars: Map<string, string>, index: number }[]} */
  const layers = [{ query: null, vars: base, index: baseOpen }];
  for (const media of mediaBlocks(clean)) {
    /** @type {Map<string, string>} */
    const vars = new Map();
    for (const rule of cssRules(media.body)) {
      if (rule.selector !== ":root") continue;
      for (const match of rule.body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
        vars.set(match[1] ?? "", (match[2] ?? "").trim());
      }
    }
    if (vars.size > 0) layers.push({ query: `(${media.query})`, vars, index: media.index });
  }
  return (width) => {
    /** @type {Map<string, string>} */
    const merged = new Map();
    for (const layer of layers) {
      if (layer.query !== null && !mediaMatches(layer.query.slice(1, -1), width)) continue;
      for (const [name, value] of layer.vars) merged.set(name, value);
    }
    return merged;
  };
}

/**
 * @param {Map<string, string>} tokens @param {string} name @param {number} [depth]
 * @returns {string}
 */
function tokenValue(tokens, name, depth = 0) {
  const value = tokens.get(name);
  if (value === undefined) throw new Error(`нет токена ${name}`);
  if (depth > 8) throw new Error(`цикл токена ${name}`);
  const ref = /^var\((--[a-z0-9-]+)\)$/.exec(value);
  return ref ? tokenValue(tokens, ref[1] ?? "", depth + 1) : value;
}

/**
 * Убирает `@media (...) { ... }` из CSS, оставляя правила верхнего уровня.
 * @param {string} css @returns {string}
 */
function withoutMediaBlocks(css) {
  const re = /@media\s*\([^)]*\)\s*\{/g;
  /** @type {{ start: number, end: number }[]} */
  const spans = [];
  for (const match of css.matchAll(re)) {
    const open = (match.index ?? 0) + match[0].length - 1;
    let depth = 0;
    for (let i = open; i < css.length; i += 1) {
      if (css[i] === "{") depth += 1;
      else if (css[i] === "}") {
        depth -= 1;
        if (depth === 0) {
          spans.push({ start: match.index ?? 0, end: i + 1 });
          break;
        }
      }
    }
  }
  let out = css;
  for (const span of spans.reverse()) out = out.slice(0, span.start) + out.slice(span.end);
  return out;
}

/**
 * Эффективный display для селектора с учётом media и порядка правил.
 * Сравниваются только правила с одинаковой специфичностью (id+attr) —
 * это гарантирует отдельная проверка селектора.
 * @param {string} css @param {string} id @param {number} width
 * @returns {string | null}
 */
function effectiveDisplay(css, id, width) {
  const clean = stripComments(css);
  /** @type {{ display: string, query: string | null }[]} */
  const layers = [];
  const needle = `#${id}`;
  // Проверяем чат: правила видимости для экрана настроек (data-active-screen=
  // "settings"/:has(#screen-settings)) к раскладке чата не относятся.
  /** @param {string} selector @returns {boolean} */
  const runtimeSpecific = (selector) =>
    /data-active-screen="(?!chat")/.test(selector) || selector.includes(":has(");
  // Правила верхнего уровня.
  for (const rule of cssRules(withoutMediaBlocks(clean))) {
    if (runtimeSpecific(rule.selector)) continue;
    if (!rule.selector.includes(needle)) continue;
    const display = declaredValue(rule.body, "display");
    if (display !== null) layers.push({ display, query: null });
  }
  // Правила в media-блоках (порядок — сверху вниз).
  for (const media of mediaBlocks(clean)) {
    for (const rule of cssRules(media.body)) {
      if (!rule.selector.includes(needle)) continue;
      const display = declaredValue(rule.body, "display");
      if (display !== null) layers.push({ display, query: media.query });
    }
  }
  let result = null;
  for (const layer of layers) {
    if (layer.query !== null && !mediaMatches(layer.query, width)) continue;
    result = layer.display;
  }
  return result;
}

const tokensCss = readUi("tokens.css");
const styleCss = readUi("style.css");
const primitivesCss = readUi("components/primitives.css");
const tokensAt = tokenCascade(tokensCss);

describe("DP-01: breakpoints SPEC §5.1", () => {
  it("токены по ширинам: 311/256 → 240/240 → 240 → 200", () => {
    /** @type {{ width: number, tree: string, member: string }[]} */
    const matrix = [
      { width: 2048, tree: "311px", member: "256px" },
      { width: 1600, tree: "311px", member: "256px" },
      { width: 1599, tree: "240px", member: "240px" },
      { width: 1440, tree: "240px", member: "240px" },
      { width: 1280, tree: "240px", member: "240px" },
      { width: 1200, tree: "240px", member: "240px" },
      { width: 1199, tree: "240px", member: "240px" },
      { width: 960, tree: "240px", member: "240px" },
      { width: 959, tree: "200px", member: "240px" },
      { width: 720, tree: "200px", member: "240px" },
      { width: 719, tree: "200px", member: "240px" },
      { width: 600, tree: "200px", member: "240px" },
    ];
    for (const row of matrix) {
      const tokens = tokensAt(row.width);
      assert.equal(tokenValue(tokens, "--layout-tree-w"), row.tree, `tree при ${row.width}px`);
      assert.equal(tokenValue(tokens, "--layout-member-panel-w"), row.member, `member при ${row.width}px`);
    }
  });

  it("дефолт 1200–1599 — базовые токены, без media-переопределения", () => {
    const base = tokensAt(1280);
    assert.equal(tokenValue(base, "--layout-tree-w"), "240px");
    assert.equal(tokenValue(base, "--layout-member-panel-w"), "240px");
    // Широкие значения при этом остаются доступны как отдельные токены.
    assert.equal(tokenValue(base, "--layout-tree-w-wide"), "311px");
    assert.equal(tokenValue(base, "--layout-member-panel-w-wide"), "256px");
  });

  it("style.css: member panel скрыт на 960–1199, tree — на <720", () => {
    // 1200–1599: панель видима на чате.
    const wide = effectiveDisplay(styleCss, "member-panel", 1280);
    assert.notEqual(wide, "none", "member-panel скрыт на 1280px (SPEC: 1200–1599 — 240)");
    // 960–1199 и ниже: display:none.
    for (const width of [1199, 1024, 960, 959, 720, 719]) {
      assert.equal(
        effectiveDisplay(styleCss, "member-panel", width),
        "none",
        `member-panel не скрыт при ${width}px (SPEC 960–1199/720–959/<720)`,
      );
    }
    // Дерево: 720–959 видно, <720 скрыто.
    for (const width of [1599, 1280, 1199, 960, 959, 720]) {
      assert.notEqual(effectiveDisplay(styleCss, "channel-tree", width), "none", `дерево скрыто при ${width}px`);
    }
    assert.equal(effectiveDisplay(styleCss, "channel-tree", 719), "none", "дерево не скрыто при 719px");
    assert.equal(effectiveDisplay(styleCss, "channel-tree", 600), "none", "дерево не скрыто при 600px");
  });

  it("скрытие панели перекрывает body[data-active-screen=chat] (специфичность)", () => {
    const blocks = mediaBlocks(stripComments(styleCss));
    const hideBlock = blocks.find((media) => mediaMatches(media.query, 960));
    assert.ok(hideBlock, "нет media-query, покрывающего 960px");
    const hideRule = cssRules(hideBlock.body).find(
      (rule) => rule.selector.includes("#member-panel") && /display:\s*none/.test(rule.body),
    );
    assert.ok(hideRule, "в media ≤1199 нет правила #member-panel { display: none }");
    // Базовое правило показа — body[data-active-screen="chat"] #member-panel (id+attr).
    // Скрывающее должно иметь ту же специфичность и идти позже.
    assert.match(
      hideRule.selector,
      /body\[data-active-screen="chat"\]\s+#member-panel/,
      "скрывающий селектор слабее body[data-active-screen=chat] #member-panel и проиграет каскад",
    );
    const flexIndex = styleCss.indexOf('body[data-active-screen="chat"] #member-panel');
    const hideIndex = styleCss.indexOf('body[data-active-screen="chat"] #member-panel', flexIndex + 1);
    assert.ok(flexIndex !== -1, "нет базового правила показа member-panel");
    assert.ok(hideIndex > flexIndex, "media-скрытие должно идти после базового display:flex");
  });

  it("chat ≥ 520 на 960–1199 и отсутствие горизонтального скролла shell", () => {
    const tokens = tokensAt(960);
    const rail = parseFloat(tokenValue(tokens, "--layout-rail-w"));
    const tree = parseFloat(tokenValue(tokens, "--layout-tree-w"));
    const chatAt960 = 960 - rail - tree;
    assert.ok(chatAt960 >= 520, `на 960px чат ${chatAt960}px < 520 (member должен быть скрыт)`);
    // Чат — flex-элемент с min-width:0: он сжимается, а не выносит shell за viewport.
    assert.match(
      stripComments(primitivesCss),
      /html,\s*body\s*\{[^}]*overflow:\s*hidden/,
      "components/primitives.css: нет html, body { overflow: hidden }",
    );
    for (const selector of [".workspace", ".content", ".chat-main"]) {
      const rule = cssRules(stripComments(styleCss)).find((item) => item.selector === selector);
      assert.ok(rule, `style.css: нет правила ${selector}`);
      assert.match(rule.body, /min-width:\s*0/, `${selector}: нет min-width: 0`);
    }
    // Горизонтальный скролл в shell не появляется.
    for (const media of mediaBlocks(stripComments(styleCss))) {
      assert.doesNotMatch(media.body, /overflow-x:\s*(?:auto|scroll)/, "горизонтальный скролл в media shell");
    }
  });

  it("aria/screen-switch: скрытие только CSS, aria-hidden — только на .screen", () => {
    const html = readUi("index.html");
    const panel = /<aside id="member-panel"[^>]*>/.exec(html)?.[0] ?? "";
    assert.match(panel, /role="complementary"/, "#member-panel потерял role=complementary");
    assert.match(panel, /aria-label="[^"]+"/, "#member-panel потерял aria-label");
    assert.doesNotMatch(panel, /\shidden(?:\s|=|>)/, "#member-panel получил hidden вместо CSS-адаптива");
    const panelList = /<div id="member-panel-list"[^>]*>/.exec(html)?.[0] ?? "";
    assert.match(panelList, /role="list"/, "#member-panel-list потерял role=list");
    assert.doesNotMatch(panelList, /\shidden(?:\s|=|>)/, "#member-panel-list получил hidden");

    // Переключение экранов: aria-hidden ставится на .screen, не на панели.
    const screensJs = readUi("features/screens.js");
    assert.match(screensJs, /setAriaHidden\(node, !active\)/, "screens.js: пропал aria-hidden на неактивных экранах");
    assert.doesNotMatch(screensJs, /member-panel|channel-tree/, "screens.js не должен управлять скрытием панелей");
    // actions.js: inline-display только ""/none — media-query остаётся главной.
    const actionsJs = readUi("features/actions.js");
    assert.match(actionsJs, /panel\.style\.display\s*=\s*expanded\s*\?\s*"none"\s*:\s*""/);
    assert.doesNotMatch(actionsJs, /style\.display\s*=\s*"(?:flex|block|grid)"/, "inline flex перекроет media-query скрытия");
  });
});
