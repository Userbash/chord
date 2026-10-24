#!/usr/bin/env node
// @ts-check
/**
 * RW-39 / W39a — проверка `waves/RW-39/DESIGN/layout.json`.
 *
 * Проверяет:
 *  1. JSON парсится; schema = rw39/layout-v1;
 *  2. у каждого блока (объект с полем `id`) есть `source` с непустым списком кодов
 *     из {M,T,L,A,F} и строкой method/ref;
 *  3. диапазоны ТЗ: rail 64–72, tree ~240, header ~48, composer 56, members ~240,
 *     строки сообщений 22–26/48, строка участника 42;
 *  4. имена токенов из layout существуют в tokens.json, а числовые размеры,
 *     объявленные токеном, совпадают с его px-значением;
 *  5. арифметика: сумма колонок референса = 2048 (±0.5), фиксированные высоты
 *     не превышают окно 1106.4.
 *
 * Запуск: node waves/RW-39/PM/w39a/verify-layout.mjs
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../../../..');
const LAYOUT = resolve(ROOT, 'waves/RW-39/DESIGN/layout.json');
const TOKENS = resolve(ROOT, 'waves/RW-39/DESIGN/tokens.json');

/** @type {string[]} */
const errors = [];
/** @param {boolean} ok @param {string} msg */
function must(ok, msg) { if (!ok) errors.push(msg); }

const doc = JSON.parse(readFileSync(LAYOUT, 'utf8'));
const tokens = JSON.parse(readFileSync(TOKENS, 'utf8')).tokens;

must(doc.schema === 'rw39/layout-v1', `schema: ${doc.schema}`);

let blocks = 0;
/**
 * @param {any} node
 * @param {string} path
 */
function walk(node, path) {
  if (Array.isArray(node)) { node.forEach((v, i) => walk(v, `${path}[${i}]`)); return; }
  if (!node || typeof node !== 'object') return;
  if ('id' in node) {
    blocks++;
    const s = node.source;
    must(!!s && typeof s === 'object', `${path} (${node.id}): нет source`);
    if (s) {
      const codes = s.codes ?? (s.code ? [s.code] : []);
      must(Array.isArray(codes) && codes.length > 0, `${path} (${node.id}): пустые коды источника`);
      for (const c of codes) must(['M', 'T', 'L', 'A', 'F'].includes(c), `${path} (${node.id}): код ${c}`);
      must(typeof s.method === 'string' && s.method.length > 3, `${path} (${node.id}): нет method`);
      must(typeof s.ref === 'string' && s.ref.length > 3, `${path} (${node.id}): нет ref`);
    }
  }
  for (const [k, v] of Object.entries(node)) {
    // Аудит-массивы: у записей есть id, но нет source — это не блоки раскладки.
    // Их проверяем отдельно ниже (severity/evidence/action).
    if (k === 'discrepancies' || k === 'assumptions') continue;
    walk(v, `${path}.${k}`);
  }
}
walk(doc, '$');

let audits = 0;
for (const d of doc.discrepancies ?? []) {
  audits++;
  must(typeof d.id === 'string' && d.id.length > 2, `discrepancy: нет id`);
  must(['P0', 'P1', 'P2', 'P3'].includes(d.severity), `discrepancy ${d.id}: severity ${d.severity}`);
  must(typeof d.evidence === 'string' && d.evidence.length > 3, `discrepancy ${d.id}: нет evidence`);
  must(typeof d.action === 'string' && d.action.length > 3, `discrepancy ${d.id}: нет action`);
}

// --- диапазоны ТЗ / SPEC ---
const S = doc.shell;
function inRange(v, [lo, hi]) { return typeof v === 'number' && v >= lo && v <= hi; }
must(inRange(S.columns.rail.w, [64, 72]), `rail=${S.columns.rail.w} вне 64–72`);
must(inRange(S.columns.rail.w, S.columns.rail.range), 'rail.w вне собственного range');
must(inRange(S.columns.tree.w, [230, 250]), `tree=${S.columns.tree.w} вне ~240`);
must(S.columns.tree.wide_w === 311, `tree.wide_w=${S.columns.tree.wide_w}`);
must(inRange(S.columns.members.w, [230, 250]), `members=${S.columns.members.w} вне ~240`);
must(S.columns.members.wide_w === 256, `members.wide_w=${S.columns.members.wide_w}`);
must(inRange(S.rows.header.h, [48, 50]), `header=${S.rows.header.h} вне ~48`);
must(inRange(S.rows.composer.h, [55, 57]), `composer=${S.rows.composer.h} вне ~56`);

const R = doc.rows;
must(inRange(R.message_line.h, [22, 26]), `message_line=${R.message_line.h}`);
must(inRange(R.message_continuation.h, [22, 26]), `continuation=${R.message_continuation.h}`);
must(R.message_group_start.h === 48, `group_start=${R.message_group_start.h}`);
must(R.message_track.h === 44, `track=${R.message_track.h}`);
must(R.member_row_panel.h === 42, `member panel=${R.member_row_panel.h}`);
must(R.member_track.h === 40, `member track=${R.member_track.h}`);

// --- токены ---
const lenTokens = new Map();
for (const [n, t] of Object.entries(tokens)) {
  const m = String(t.value).match(/^(-?[\d.]+)px$/);
  if (m) lenTokens.set(n, Number(m[1]));
}
let tokenRefs = 0;
/** @param {any} node @param {string} path */
function checkTokens(node, path) {
  if (Array.isArray(node)) { node.forEach((v, i) => checkTokens(v, `${path}[${i}]`)); return; }
  if (!node || typeof node !== 'object') return;
  if (Array.isArray(node.tokens)) {
    for (const n of node.tokens) {
      tokenRefs++;
      must(n in tokens, `${path}: токена ${n} нет в tokens.json`);
    }
    // числовые размеры блока сверяем с px-токенами, если они объявлены
    for (const key of ['w', 'h', 'h_px', 'value']) {
      const v = node[key];
      if (typeof v !== 'number') continue;
      const same = node.tokens
        .map((n) => [n, lenTokens.get(n)])
        .filter(([, tv]) => typeof tv === 'number' && Math.abs(tv - v) < 0.01);
      const declared = node.tokens.filter((n) => lenTokens.has(n));
      if (declared.length && !same.length) {
        errors.push(`${path}: ${key}=${v}, но токены ${declared} дают ${declared.map((n) => lenTokens.get(n))}`);
      }
    }
  }
  for (const [k, v] of Object.entries(node)) checkTokens(v, `${path}.${k}`);
}
checkTokens(doc, '$');

// --- арифметика референса ---
const cols = S.columns;
must(Number.isInteger(cols.chat.ref_w) || typeof cols.chat.ref_w === 'number', 'нет ref_w чата');
const sum = cols.rail.w + cols.tree.wide_w + cols.chat.ref_w + cols.members.wide_w;
must(Math.abs(sum - 2048) <= 0.5, `сумма колонок ${sum} ≠ 2048`);
must(Math.abs(doc.shell.check.ref_columns_sum - sum) < 0.01,
  `check.ref_columns_sum=${doc.shell.check.ref_columns_sum} ≠ фактической ${sum}`);
const fixedH = S.rows.titlebar.h + S.rows.header.h + S.rows.chat_notice.h + S.rows.composer.h;
must(fixedH < doc.reference_viewport.window_css.h, `фиксированные высоты ${fixedH} ≥ окна`);

const screenIds = Object.keys(doc.screens);
must(screenIds.length === 4, `экранов ${screenIds.length}, ожидалось 4`);
for (const id of ['chat', 'emoji', 'members', 'settings']) must(screenIds.includes(id), `нет экрана ${id}`);

if (errors.length) {
  console.error(`FAIL: ${errors.length} ошибок`);
  for (const e of errors.slice(0, 40)) console.error(' - ' + e);
  process.exit(1);
}
console.log(`ok: layout.json — ${blocks} блоков с source, ${tokenRefs} ссылок на токены, `
  + `экранов ${screenIds.length}; rail ${cols.rail.w}, tree ${cols.tree.w}/${cols.tree.wide_w}, `
  + `header ${S.rows.header.h}, composer ${S.rows.composer.h}, members ${cols.members.w}/${cols.members.wide_w}, `
  + `rows 26/48 + member 42; колонки ${sum} ≈ 2048`);
