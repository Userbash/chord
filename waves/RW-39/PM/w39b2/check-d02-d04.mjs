#!/usr/bin/env node
// @ts-check
/**
 * RW-39 / W39b2 — независимая проверка применения D-02 (значения дампа) и D-04
 * (переменные высоты строк) в DESIGN/tokens.css и DESIGN/SPEC.md.
 *
 * Оракул — DECISIONS.md D-02/D-04 и waves/RW-39/PM/w39a/REPORT.md §4:
 *   значения дампа ui/tokens.json (build 1.0.162), а не текущая реализация.
 *
 * Запуск: node waves/RW-39/PM/w39b2/check-d02-d04.mjs
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../../../..');
const css = readFileSync(resolve(ROOT, 'waves/RW-39/DESIGN/tokens.css'), 'utf8');
const spec = readFileSync(resolve(ROOT, 'waves/RW-39/DESIGN/SPEC.md'), 'utf8');
const tokens = JSON.parse(readFileSync(resolve(ROOT, 'waves/RW-39/DESIGN/tokens.json'), 'utf8')).tokens;

/** @type {string[]} */
const bad = [];
/** @param {boolean} ok @param {string} msg */
const must = (ok, msg) => { if (!ok) bad.push(msg); };

/** Значение токена в :root (до первого @media). */
const rootCss = css.slice(0, css.indexOf('@media'));
/** @param {string} name */
function val(name) {
  const m = rootCss.match(new RegExp(`${name}\\s*:\\s*([^;}]+);`));
  return m ? m[1].replace(/\s+/g, ' ').trim() : null;
}
/** @param {string} name @param {string} expected @param {string} why */
function eq(name, expected, why) {
  const v = val(name);
  must(v === expected, `${name}: css="${v}" ≠ ожидаемому "${expected}" (${why})`);
  must(tokens[name]?.value === expected, `${name}: tokens.json="${tokens[name]?.value}" ≠ "${expected}"`);
}

console.log('=== D-02: значения дампа (16 расхождений W39a §4) ===');
eq('--danger', '#da3e44', 'dump --status-danger [T]');
eq('--status-dnd', '#da3e44', 'dump --status-danger [T]');
eq('--warning', '#fdb833', 'dump --status-warning/--yellow-300 [T]');
eq('--status-idle', '#fdb833', 'dump --status-warning [T]');
eq('--positive', '#3d9e60', 'dump --status-positive/--green-360 [T]');
eq('--status-online', '#3d9e60', 'dump --status-online [T]');
eq('--text-inverse', '#2f3035', 'dump --text-invert (Δ23) [T]');
eq('--accent-hover', '#4654c0', 'dump --brand-560 (Δ4) [T]');
eq('--accent-soft', 'rgb(88 101 242 / 16%)', 'dump --message-highlight-background-default 16%');
eq('--border-subtle', 'rgb(151 151 159 / 12%)', 'dump --border-subtle #97979f@12% (Δ9)');
eq('--border', 'rgb(151 151 159 / 20%)', 'dump --border-normal #97979f@20%');
eq('--border-strong', 'rgb(151 151 159 / 44%)', 'dump --border-strong #97979f@44%');
eq('--badge-secondary-bg', 'rgb(151 151 159 / 16%)', 'dump --badge-secondary-background #97979f@16%');
eq('--radius-round', '2147483647px', 'dump --radius-round (эквивалент, D-02)');

console.log('=== D-02: метка --status-streaming → [A] ===');
const streamingLine = rootCss.split('\n').find((l) => l.includes('--status-streaming:')) ?? '';
must(/\[A\]/.test(streamingLine), `--status-streaming без [A]: ${streamingLine.trim()}`);
must(!/\[T\]/.test(streamingLine), `--status-streaming всё ещё [T]: ${streamingLine.trim()}`);
must(/\| `--status-streaming` \| `#593695` \| `\[A\]`/.test(spec), 'SPEC: строка --status-streaming не [A]');

console.log('=== D-02: шаг строки канала 30 (высота 28) ===');
eq('--row-channel', '28px', 'высота строки (RE)');
eq('--row-channel-pitch', '30px', 'шаг 28 + 2 зазор [M]');
const channelLine = rootCss.split('\n').find((l) => l.includes('--row-channel-pitch:')) ?? '';
must(/28[^0-9].*зазор|28 высота/.test(channelLine), `--row-channel-pitch: в комментарии нет «28 высота»: ${channelLine.trim()}`);
must(/\*\*30px\*\*/.test(spec), 'SPEC §5.1: нет шага 30px у строки канала');

console.log('=== D-04: переменные высоты строк ===');
eq('--row-h-continuation', '26px', 'RE message_cozy_continuation');
eq('--row-h-group-start', '48px', 'RE message_cozy_group_start');
eq('--row-line-extra', '22px', 'RE line_extra (+22)');
eq('--row-h-member', '42px', 'RE member_row');

console.log('=== D-04: старые константы не подаются как дизайн-модель ===');
for (const legacy of ['--row-message-track', '--row-member-track']) {
  must(tokens[legacy]?.status?.includes('F'), `${legacy}: статус должен быть [F] (сейчас ${tokens[legacy]?.status})`);
}
must(/\[F\].*FROZEN RW-38/.test(css), 'tokens.css: треки не помечены [F] FROZEN RW-38');
must(!/\| Шаг строки сообщения \| \*\*44px\*\*/.test(spec), 'SPEC §5.1: константа 44px осталась');
must(!/строки 40 `\[F\]`/.test(spec), 'SPEC §9.3: константа «строки 40» осталась');
must(!/окно — 44px-треки/.test(spec), 'SPEC §9.1: «окно — 44px-треки» осталось');
must(/--row-h-continuation/.test(spec) && /--row-h-group-start/.test(spec), 'SPEC: нет ссылок на --row-h-*');

console.log('=== Ложные числа не оставлены ===');
const staleColors = [['#ed4245', 'старый --danger'], ['#f0b232', 'старый --warning'],
  ['#23a55a', 'старый --positive'], ['#4752c4', 'старый --accent-hover']];
for (const [c, why] of staleColors) {
  must(!spec.includes(c), `SPEC: осталось ${c} (${why})`);
  must(!rootCss.includes(c), `tokens.css: осталось ${c} (${why})`);
}
must(!/'#1a1a1e' \| текст на светлой|`--text-inverse` \| `#1a1a1e`/.test(spec), 'SPEC: --text-inverse всё ещё #1a1a1e');
must(!/7\.5–9\.1/.test(rootCss), 'tokens.css: устаревший диапазон 7.5–9.1');
must(!/3\.4–4\.1/.test(spec), 'SPEC: устаревший диапазон 3.4–4.1');

if (bad.length) {
  console.error(`FAIL: ${bad.length} ошибок`);
  for (const b of bad) console.error(' - ' + b);
  process.exit(1);
}
console.log('ok: D-02 (значения + метка + шаг 30) и D-04 (4 токена + снятые константы) применены; '
  + 'устаревших значений/диапазонов в tokens.css и SPEC.md нет');
