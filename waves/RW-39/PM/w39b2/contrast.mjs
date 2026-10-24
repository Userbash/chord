#!/usr/bin/env node
// @ts-check
/**
 * RW-39 / W39b2 — пересчёт контрастных чисел SPEC.md §3.4/§3.3 под значения D-02.
 * Независимый оракул: WCAG 2.2, relative luminance sRGB.
 *
 * Запуск: node waves/RW-39/PM/w39b2/contrast.mjs
 */
const hex = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const chan = (c) => {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};
const lum = (h) => {
  const [r, g, b] = hex(h).map(chan);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
/** @param {string} a @param {string} b */
const cr = (a, b) => {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};
const f2 = (n) => n.toFixed(2);

const surfaces = ['#121214', '#1a1a1e', '#202024', '#242429', '#2c2d32'];
const texts = [
  '--text #efeff1', '#efeff1',
  '--text-strong #fbfbfb', '#fbfbfb',
  '--text-muted #96979e', '#96979e',
  '--text-subtle #abacb2', '#abacb2',
  '--text-faint #81828a', '#81828a',
  '--text-link #4d96ee', '#4d96ee',
  '--text-accent #a5b0ff', '#a5b0ff',
  '--danger-text #ff9a9e', '#ff9a9e',
];

console.log('=== SPEC §3.4 таблица (текст \\ фон) ===');
for (let i = 0; i < texts.length; i += 2) {
  const row = surfaces.map((s) => f2(cr(texts[i + 1], s)));
  console.log(`${texts[i].padEnd(24)} ${row.join('  ')}`);
}

console.log('\n=== Пары контролов (inline §3.4) ===');
console.log(`#ffffff на --accent #5865f2          = ${f2(cr('#ffffff', '#5865f2'))}`);
console.log(`#ffffff на --danger-fill #cf3a3f    = ${f2(cr('#ffffff', '#cf3a3f'))}`);
console.log(`#ffffff на --danger-fill-hover #b92d33 = ${f2(cr('#ffffff', '#b92d33'))}`);
console.log(`#121214 на --warning #fdb833        = ${f2(cr('#121214', '#fdb833'))}`);
console.log(`--focus-ring #a5b0ff: ${surfaces.map((s) => f2(cr('#a5b0ff', s))).join('  ')}`);
console.log(`--scrollbar-thumb #5f606a на #1a1a1e = ${f2(cr('#5f606a', '#1a1a1e'))}, на #121214 = ${f2(cr('#5f606a', '#121214'))}`);
console.log(`--accent #5865f2 как non-text: ${surfaces.map((s) => f2(cr('#5865f2', s))).join('  ')}`);

console.log('\n=== Проверка комментариев tokens.css ===');
console.log(`--positive #3d9e60 на --bg-chat #1a1a1e = ${f2(cr('#3d9e60', '#1a1a1e'))}`);
console.log(`--positive-text #57d98a на #242429 = ${f2(cr('#57d98a', '#242429'))}, на #2c2d32 = ${f2(cr('#57d98a', '#2c2d32'))}`);
console.log(`--text-inverse #2f3035 на --warning #fdb833 = ${f2(cr('#2f3035', '#fdb833'))} (Δ23; было #1a1a1e = ${f2(cr('#1a1a1e', '#fdb833'))})`);
console.log(`--danger #da3e44 на --bg-chat (non-text ≥3) = ${f2(cr('#da3e44', '#1a1a1e'))}`);
console.log(`--warning #fdb833 на --bg-chat (non-text ≥3) = ${f2(cr('#fdb833', '#1a1a1e'))}`);
console.log(`--text-on-accent #ffffff на --accent = ${f2(cr('#ffffff', '#5865f2'))}`);
console.log(`--text-strong #fbfbfb на --bg-chat = ${f2(cr('#fbfbfb', '#1a1a1e'))}`);
