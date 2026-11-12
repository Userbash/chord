#!/usr/bin/env node
// @ts-check
/**
 * RW-39 / W39a — генератор `waves/RW-39/DESIGN/tokens.json` из `tokens.css`.
 *
 * Источник истины значений — `waves/RW-39/DESIGN/tokens.css` (владелец W39b).
 * Скрипт НЕ правит CSS: только читает, парсит и выгружает машиночитаемую выжимку
 * (имя → значение → статус → ссылки на дамп Discord 1.0.162).
 *
 * Запуск:
 *   node waves/RW-39/PM/w39a/gen-tokens.mjs
 * Проверка (не пишет):
 *   node waves/RW-39/PM/w39a/gen-tokens.mjs --check
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../../../..');
const CSS = resolve(ROOT, 'waves/RW-39/DESIGN/tokens.css');
const OUT = resolve(ROOT, 'waves/RW-39/DESIGN/tokens.json');
const DUMP = '/home/user/clean-room-oracle/ui/tokens.json';

const STATUS_CODES = {
  '[M]': 'измерено на референсе (/home/user/clean-room-oracle/ui/00-app.png, ui8/*.jpg)',
  '[T]': 'токен из /home/user/clean-room-oracle/ui/tokens.json (dark, сборка 1.0.162)',
  '[L]': 'RE-контракт waves/RW-37/RE/layout.json (rw37/re-layout-v1)',
  '[A]': 'assumption, обоснование в SPEC.md',
  '[F]': 'frozen тестами RW-38 (tests/unit/gui-spike-dom.test.mjs)',
};

/**
 * Заменяет /*...*\/ на пробелы, сохраняя длину и переводы строк.
 * @param {string} text
 */
function blankComments(text) {
  let out = '';
  let i = 0;
  while (i < text.length) {
    if (text.startsWith('/*', i)) {
      const end = text.indexOf('*/', i + 2);
      const stop = end === -1 ? text.length : end + 2;
      out += text.slice(i, stop).replace(/[^\n]/g, ' ');
      i = stop;
    } else {
      out += text[i];
      i += 1;
    }
  }
  return out;
}

/** hsl(...) → {hex, alpha}; понимает calc(1*X%) и /alpha. */
function hslToRgb(v) {
  const s = String(v).replace(/calc\(\s*1\s*\*\s*([^)]*)\)/g, '$1');
  const m = s.match(/hsl\(\s*(-?[\d.]+)(?:deg)?[\s,]+([\d.]+)%[\s,]+([\d.]+)%(?:\s*\/\s*([\d.]+%?))?/i);
  if (!m) return null;
  const h = Number(m[1]) % 360, sat = Number(m[2]) / 100, l = Number(m[3]) / 100;
  const c = (1 - Math.abs(2 * l - 1)) * sat;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m0 = l - c / 2;
  const rgb = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x]
    : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return { hex: '#' + rgb.map((n) => Math.round((n + m0) * 255).toString(16).padStart(2, '0')).join(''), alpha: parseAlpha(m[4]) };
}

/** @param {string|undefined} a */
function parseAlpha(a) {
  if (!a) return 1;
  return a.endsWith('%') ? Number(a.slice(0, -1)) / 100 : Number(a);
}

/**
 * Нормализация значения для сверки: цвет → #rrggbb(+alpha), длина → Npx,
 * calc с одним уровнем — посчитанное значение. null = сверка вручную.
 * @param {string} v
 */
function normalizeValue(v) {
  const s0 = String(v).trim().replace(/calc\(\s*1\s*\*\s*([^)]*)\)/g, '$1');
  const cm = s0.match(/color-mix\([^,]+,\s*(hsl\([^)]*\)|rgb\([^)]*\)|#[0-9a-f]{3,8})/i);
  const src = cm ? cm[1] : s0;
  const h = hslToRgb(src);
  if (h) return h.alpha === 1 ? h.hex : `${h.hex}@${Math.round(h.alpha * 100)}%`;
  const r = src.match(/^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/i);
  if (r) {
    const hex = '#' + [r[1], r[2], r[3]].map((n) => Math.round(Number(n)).toString(16).padStart(2, '0')).join('');
    const a = parseAlpha(r[4]);
    return a === 1 ? hex : `${hex}@${Math.round(a * 100)}%`;
  }
  if (/^#[0-9a-f]{6}$/i.test(src)) return src.toLowerCase();
  const c = src.match(/^calc\((.+)\)$/);
  if (c) {
    const expr = c[1].replace(/(\d)px/g, '$1');
    if (/^[\d.\s+*-]+$/.test(expr)) {
      try { return `${Number(Function(`return (${expr})`)()).toString()}px`; } catch { /* ниже — null */ }
    }
  }
  if (/^-?[\d.]+px$/.test(src)) return `${Number.parseFloat(src)}px`;
  if (/^-?[\d.]+$/.test(src)) return String(Number(src));
  if (/^-?[\d.]+(rem|em|ms|s)$/.test(src)) return src;
  return null;
}

/** Человекочитаемая разница цветов (Δ по каналам), null если не цвета. */
function colorDelta(a, b) {
  if (!a || !b || !a.startsWith('#') || !b.startsWith('#') || a.length !== 7 || b.length !== 7) return null;
  const pa = [1, 3, 5].map((i) => parseInt(a.slice(i, i + 2), 16));
  const pb = [1, 3, 5].map((i) => parseInt(b.slice(i, i + 2), 16));
  return Math.max(...pa.map((n, i) => Math.abs(n - pb[i])));
}

/** Значение после подстановки var(--x) по карте base-токенов. */
function resolveValue(value, map, seen = new Set()) {
  const m = value.match(/^var\((--[a-z0-9-]+)\)$/);
  if (!m) return value;
  if (seen.has(m[1])) return value;
  const next = map.get(m[1]);
  if (!next) return value;
  seen.add(m[1]);
  return resolveValue(next, map, seen);
}

const raw = readFileSync(CSS, 'utf8');
const code = blankComments(raw);

// Группы: заголовки " * 9. Раскладка shell (SPEC.md §5)".
/** @type {{at:number, title:string}[]} */
const groups = [...raw.matchAll(/^\s*\*\s*(\d+\.\s*[^\n]+?)\s*$/gm)]
  .map((m) => ({ at: m.index ?? 0, title: m[1].trim() }));

/**
 * Интервалы блоков, меняющих scope: `@media (...)` и `:root[...]`.
 * Возвращает [{start, end, scope}] по позиции открывающей `{`.
 * @param {string} text очищенный от комментариев CSS
 */
function blockIntervals(text) {
  /** @type {{start:number,end:number,scope:string}[]} */
  const out = [];
  for (const m of text.matchAll(/(@media\s*\([^)]+\)|:root\[[^\]]+\])\s*\{/g)) {
    const open = (m.index ?? 0) + m[0].length - 1;
    let depth = 0, end = text.length;
    for (let i = open; i < text.length; i++) {
      if (text[i] === '{') depth++;
      else if (text[i] === '}') { depth--; if (depth === 0) { end = i; break; } }
    }
    const sel = m[1];
    let scope = 'root';
    if (sel.startsWith('@media')) {
      const q = sel.slice(6).trim().replace(/^\(|\)$/g, '');
      scope = q.includes('min-width') ? 'min-width>=1600px'
        : q.includes('reduced-motion') ? 'prefers-reduced-motion:reduce'
        : `@media(${q})`;
    } else scope = sel.replace(/^:root\[|\]$/g, '').replace(/"/g, '').replace('=', '=');
    out.push({ start: open, end, scope });
  }
  return out;
}
const BLOCKS = blockIntervals(code);

/** scope по позиции: самый внутренний блок, содержащий декларацию. */
function scopeAt(pos) {
  const hits = BLOCKS.filter((b) => b.start < pos && pos < b.end);
  return hits.length ? hits[hits.length - 1].scope : 'root';
}

const declRe = /(--[a-z0-9-]+)\s*:\s*([^;}]+);/g;
/** @type {{name:string,value:string,comment:string,group:string,scope:string}[]} */
const decls = [];
for (const m of code.matchAll(declRe)) {
  const at = m.index ?? 0;
  const group = [...groups].filter((g) => g.at < at).pop()?.title ?? '';
  // Провенанс: комментарий в конце той же строки, где закрывается `;`.
  const tailStart = at + m[0].length;
  const eol = raw.indexOf('\n', tailStart);
  const window = raw.slice(at, eol === -1 ? raw.length : eol);
  const comments = [...window.matchAll(/\/\*([\s\S]*?)\*\//g)];
  const comment = comments.length ? comments[0][1].replace(/\s+/g, ' ').trim() : '';
  decls.push({ name: m[1], value: m[2].replace(/\s+/g, ' ').trim(), comment, group, scope: scopeAt(at) });
}

const base = decls.filter((d) => d.scope === 'root');
const overrides = decls.filter((d) => d.scope !== 'root');
const names = base.map((d) => d.name);
const dupes = names.filter((n, i) => names.indexOf(n) !== i);
if (dupes.length) throw new Error(`дубли имён в :root: ${[...new Set(dupes)].join(', ')}`);

const map = new Map(base.map((d) => [d.name, d.value]));
const dump = JSON.parse(readFileSync(DUMP, 'utf8'));

/** @type {Record<string, any>} */
const tokens = {};
/** @type {Record<string, any>} */
const overridesOut = {};
let dumpChecked = 0, dumpMismatch = 0, dumpMissing = 0, aliasCount = 0;
/** @type {string[]} */
const warnings = [];

for (const d of base) {
  const codes = [...new Set([...d.comment.matchAll(/\[([MTLAF])\]/g)].map((m) => m[1]))];
  const isAlias = /^var\(--[a-z0-9-]+\)$/.test(d.value);
  if (isAlias) aliasCount++;
  // Ссылки на дамп: только имена, указанные после маркера [T] (имена своих же
  // токенов, упомянутые в примечании, ссылками на дамп не считаются).
  /** @type {string[]} */
  const refsAll = [];
  for (const tm of d.comment.matchAll(/\[T\]([\s\S]*?)(?=\[[MTLAF]\]|$)/g)) {
    for (const n of tm[1].match(/--[a-z0-9-]+/g) ?? []) {
      // Имя своего же токена (кроме одноимённого) — внутренняя ссылка, не дамп.
      if (n !== d.name && map.has(n)) continue;
      if (!refsAll.includes(n)) refsAll.push(n);
    }
  }
  const dumpRefs = [];
  for (const n of refsAll) {
    if (!(n in dump)) {
      dumpMissing++;
      warnings.push(`${d.name}: токена ${n} нет в дампе tokens.json (цитата [T] не подтверждается)`);
      dumpRefs.push({ name: n, missing: true });
      continue;
    }
    const dv = String(dump[n]);
    const cmpRaw = isAlias ? resolveValue(d.value, map) : d.value;
    const a = normalizeValue(cmpRaw);
    const b = normalizeValue(dv);
    dumpChecked++;
    let match = null;
    if (a !== null && b !== null) {
      match = a === b;
      if (!match) {
        dumpMismatch++;
        const delta = colorDelta(a, b);
        warnings.push(`${d.name}: css=${cmpRaw} ≠ dump ${n}=${dv} (${b}${delta === null ? '' : `, Δ=${delta}`})`);
      }
    } else {
      warnings.push(`${d.name}: dump ${n}=${dv} — сравнение вручную (css=${cmpRaw})`);
    }
    dumpRefs.push({ name: n, value: dv, ...(b ? { resolved: b } : {}), match });
  }
  tokens[d.name] = {
    value: d.value,
    group: d.group,
    status: codes.length ? codes : ['A'],
    ...(isAlias ? { kind: 'alias', resolves_to: resolveValue(d.value, map) } : {}),
    ...(dumpRefs.length ? { dump_refs: dumpRefs } : {}),
    ...(d.comment ? { note: d.comment } : {}),
  };
}

for (const d of overrides) {
  overridesOut[d.scope] ??= {};
  overridesOut[d.scope][d.name] = d.value;
}

const md5 = createHash('md5').update(raw).digest('hex');
const out = {
  schema: 'rw39/tokens-v1',
  generated: '2026-10-23',
  card: 'RW-39 / Phase 1a — RE-артефакты (W39a)',
  scope: 'Используемые токены дизайн-системы DL, 1:1 с waves/RW-39/DESIGN/tokens.css. Полные 4795 переменных дампа сюда НЕ переносятся.',
  source_css: {
    path: 'waves/RW-39/DESIGN/tokens.css',
    md5,
    declarations: decls.length,
    root_names: base.length,
    aliases: aliasCount,
    owner: 'W39b (значения менять только там)',
  },
  dump: { path: '/home/user/clean-room-oracle/ui/tokens.json', variables: Object.keys(dump).length, build: '1.0.162', theme: 'dark', read_only: true },
  status_codes: STATUS_CODES,
  verification: {
    method: 'node waves/RW-39/PM/w39a/verify-tokens.mjs — имена и значения сверяются с tokens.css программно',
    dump_refs_checked: dumpChecked,
    dump_refs_mismatch: dumpMismatch,
    dump_refs_missing: dumpMissing,
  },
  tokens,
  overrides: overridesOut,
  warnings,
};

const serialized = JSON.stringify(out, null, 1) + '\n';
if (process.argv.includes('--check')) {
  const cur = readFileSync(OUT, 'utf8');
  if (cur !== serialized) { console.error('tokens.json рассинхронизирован с tokens.css — перегенерируй'); process.exit(1); }
  console.log(`ok: tokens.json актуален (${base.length} токенов, md5 css ${md5})`);
} else {
  writeFileSync(OUT, serialized);
  console.log(`written tokens.json: ${base.length} токенов (${aliasCount} alias), ${overrides.length} оверрайдов, md5 css ${md5}`);
  console.log(`dump refs: checked=${dumpChecked} mismatch=${dumpMismatch} missing=${dumpMissing} warnings=${warnings.length}`);
}
