#!/usr/bin/env node
// @ts-check
/**
 * RW-39 / W39a — проверка `tokens.json` ↔ `tokens.css` (1:1) и ссылок на дамп.
 *
 * Проверяет:
 *  1. JSON парсится, schema = rw39/tokens-v1;
 *  2. каждое имя из tokens.json есть в `:root` tokens.css и значение совпадает
 *     посимвольно; каждое имя из `:root` tokens.css присутствует в tokens.json;
 *  3. md5 tokens.css в файле совпадает с фактическим;
 *  4. оверрайды (media/queries) совпадают с CSS;
 *  5. у каждого токена непустой список кодов статуса из {M,T,L,A,F};
 *  6. каждая ссылка dump_refs либо отсутствует с missing:true, либо имеет
 *     match:true/false (не null) — иначе сверка не выполнена.
 *
 * Запуск: node waves/RW-39/PM/w39a/verify-tokens.mjs
 */
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../../../..');
const CSS = resolve(ROOT, 'waves/RW-39/DESIGN/tokens.css');
const JSONF = resolve(ROOT, 'waves/RW-39/DESIGN/tokens.json');

/** @type {string[]} */
const errors = [];
/** @param {boolean} ok @param {string} msg */
function must(ok, msg) { if (!ok) errors.push(msg); }

function blankComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '));
}

const cssRaw = readFileSync(CSS, 'utf8');
const cssCode = blankComments(cssRaw);
const doc = JSON.parse(readFileSync(JSONF, 'utf8'));

must(doc.schema === 'rw39/tokens-v1', `schema: ${doc.schema}`);
must(typeof doc.tokens === 'object' && doc.tokens !== null, 'нет tokens{}');

// --- CSS: :root верхнего уровня ---
/** @type {Map<string,string>} */
const cssRoot = new Map();
// :root { ... } до первого @media/:root[...] — базовый блок.
const rootBlock = cssCode.slice(cssCode.indexOf(':root'), cssCode.indexOf('@media'));
for (const m of rootBlock.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;}]+);/g)) {
  const name = m[1];
  const value = m[2].replace(/\s+/g, ' ').trim();
  if (cssRoot.has(name)) errors.push(`tokens.css: дубль ${name}`);
  cssRoot.set(name, value);
}

const jsonNames = Object.keys(doc.tokens);
must(jsonNames.length === cssRoot.size,
  `разное число токенов: json=${jsonNames.length} css=${cssRoot.size}`);

for (const [name, value] of cssRoot) {
  const t = doc.tokens[name];
  if (!t) { errors.push(`в tokens.json нет ${name}`); continue; }
  if (t.value !== value) errors.push(`${name}: json="${t.value}" ≠ css="${value}"`);
  if (!Array.isArray(t.status) || t.status.length === 0) errors.push(`${name}: пустой status`);
  for (const c of t.status) must(['M', 'T', 'L', 'A', 'F'].includes(c), `${name}: неизвестный код ${c}`);
  for (const ref of t.dump_refs ?? []) {
    must(ref.missing === true || ref.match === true || ref.match === false,
      `${name}: dump_ref ${ref.name} без результата сверки (match=${ref.match})`);
  }
}
for (const name of jsonNames) if (!cssRoot.has(name)) errors.push(`в tokens.css нет ${name}`);

// --- md5 ---
const md5 = createHash('md5').update(cssRaw).digest('hex');
must(doc.source_css?.md5 === md5, `md5: json=${doc.source_css?.md5} факт=${md5}`);

// --- оверрайды ---
/** @type {Record<string, Record<string,string>>} */
const cssOverrides = {};
for (const bm of cssCode.matchAll(/(@media\s*\(([^)]+)\)|:root\[([^\]]+)\])\s*\{([\s\S]*?)\n\s*\}/g)) {
  const key = bm[2]
    ? (bm[2].includes('min-width') ? 'min-width>=1600px'
      : bm[2].includes('reduced-motion') ? 'prefers-reduced-motion:reduce' : `@media(${bm[2]})`)
    : bm[3].replace(/"/g, '');
  cssOverrides[key] ??= {};
  for (const d of bm[4].matchAll(/(--[a-z0-9-]+)\s*:\s*([^;}]+);/g)) {
    cssOverrides[key][d[1]] = d[2].replace(/\s+/g, ' ').trim();
  }
}
const jsonOver = doc.overrides ?? {};
must(JSON.stringify(Object.keys(cssOverrides).sort()) === JSON.stringify(Object.keys(jsonOver).sort()),
  `набор оверрайдов: css=${Object.keys(cssOverrides)} json=${Object.keys(jsonOver)}`);
for (const [scope, map] of Object.entries(cssOverrides)) {
  for (const [n, v] of Object.entries(map)) {
    must(jsonOver[scope]?.[n] === v, `override ${scope} ${n}: json=${jsonOver[scope]?.[n]} css=${v}`);
  }
}

const matched = jsonNames.flatMap((n) => doc.tokens[n].dump_refs ?? []);
const okRefs = matched.filter((r) => r.match === true).length;
const badRefs = matched.filter((r) => r.match === false).length;
const missingRefs = matched.filter((r) => r.missing === true).length;

if (errors.length) {
  console.error(`FAIL: ${errors.length} ошибок`);
  for (const e of errors.slice(0, 40)) console.error(' - ' + e);
  process.exit(1);
}
console.log(`ok: ${jsonNames.length} токенов 1:1 с tokens.css (md5 ${md5}); `
  + `dump_refs: ${okRefs} match, ${badRefs} mismatch, ${missingRefs} missing (расхождения — в REPORT.md, CSS не правится)`);
