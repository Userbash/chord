// Проверка геометрии: значения берутся из ui/tokens.css (source of truth),
// а не из CSS-файлов «на глаз». Использование: node geometry-check.mjs
import { readFileSync } from "node:fs";

const root = new URL("../../../../rust/gui-spike/ui/", import.meta.url);
const tokens = readFileSync(new URL("tokens.css", root), "utf8");

/** @type {Map<string,string>} */
const decl = new Map();
// Только базовый блок :root: медиа/атрибутные переопределения (широкий shell,
// density=compact) идут после и не должны подменять базовую геометрию.
for (const m of tokens.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
  if (!decl.has(m[1])) decl.set(m[1], m[2].trim());
}

// Значения широкого shell (>=1600px) — отдельные токены, проверяются явно.
for (const [name, value] of [
  ["--layout-tree-w-wide", "311px"],
  ["--layout-member-panel-w-wide", "256px"],
]) {
  const got = resolve(name);
  if (got !== value) throw new Error(`${name}: ожидалось ${value}, получено ${got}`);
}
console.log(
  `ok   | широкий shell (>=1600px) | --layout-tree-w-wide/${"--layout-member-panel-w-wide"} = 311px/256px`,
);


/** @param {string} name */
function resolve(name, depth = 0) {
  const v = decl.get(name);
  if (v === undefined) throw new Error(`нет токена ${name}`);
  if (depth > 8) throw new Error(`цикл ${name}`);
  const ref = /^var\((--[a-z0-9-]+)\)$/.exec(v);
  return ref ? resolve(ref[1], depth + 1) : v;
}

const expect = [
  ["titlebar", "--layout-titlebar-h", /^\d+px$/],
  ["rail", "--layout-rail-w", /^(6[4-9]|7[0-2])px$/],
  ["tree", "--layout-tree-w", /^2[0-9]{2}px$/],
  ["chat header", "--layout-header-h", /^(4[6-9]|5[0-2])px$/],
  ["composer", "--layout-composer-h", /^(5[4-8]|56)px$/],
  ["member panel", "--layout-member-panel-w", /^2[0-9]{2}px$/],
  ["user bar", "--layout-userbar-h", /^56px$/],
  ["tree header", "--space-11", /^48px$/],
  ["chat row track", "--row-message-track", /^44px$/],
  ["chat line-height", "--line-height-message", /^22px$/],
  ["chat group start (RE)", "--row-message-start", /^48px$/],
  ["chat cozy (RE, справочно)", "--row-message-cozy", /^26px$/],
  ["member row track", "--row-member-track", /^40px$/],
  ["member row (panel)", "--row-member", /^42px$/],
  ["channel row", "--row-channel", /^28px$/],
  ["notice", "--layout-group-title-h", /^24px$/],
  ["emoji card w", "--emoji-picker-w", /^498px$/],
  ["emoji card h", "--emoji-picker-h", /^440px$/],
  ["emoji rail", "--emoji-rail-w", /^48px$/],
  ["emoji cell", "--emoji-cell", /^44px$/],
  ["emoji gap", "--emoji-gap", /^4px$/],
  ["emoji cols", "--emoji-cols", /^9$/],
  ["avatar sm", "--avatar-sm", /^20px$/],
  ["avatar md", "--avatar-md", /^32px$/],
  ["avatar lg", "--avatar-lg", /^40px$/],
  ["switch w", "--switch-w", /^40px$/],
  ["switch h", "--switch-h", /^24px$/],
  ["switch thumb", "--switch-thumb", /^18px$/],
  ["settings nav", "--settings-nav-w", /^232px$/],
  ["settings nav item", "--settings-nav-item-h", /^34px$/],
  ["content max w", "--layout-content-max-w", /^720px$/],
];

let bad = 0;
for (const [block, token, re] of expect) {
  const value = resolve(token);
  const ok = re.test(value);
  if (!ok) bad++;
  console.log(`${ok ? "ok  " : "FAIL"} | ${block} | ${token} = ${value}`);
}
if (bad > 0) {
  console.error(`\n${bad} расхождений геометрии`);
  process.exit(1);
}
console.log(`\nвсе ${expect.length} геометрических значений в допуске`);
