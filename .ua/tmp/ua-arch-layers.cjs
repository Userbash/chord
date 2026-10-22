#!/usr/bin/env node
// Builds .ua/intermediate/layers.json from the arch input, with exhaustive
// validation. CommonJS because the repo package.json is "type": "module".
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..', '..');
const INPUT = path.join(ROOT, '.ua', 'tmp', 'ua-arch-input.json');
const OUTPUT = path.join(ROOT, '.ua', 'intermediate', 'layers.json');

const input = JSON.parse(fs.readFileSync(INPUT, 'utf8'));
const nodes = input.fileNodes;
const byId = new Map(nodes.map((n) => [n.id, n]));

const p = (n) => n.filePath;
const inCrate = (n, crate) => p(n).startsWith(`rust/crates/${crate}/`);

const rules = [
  {
    id: 'layer:gateway',
    name: 'Шлюз и реальное время',
    description:
      'WebSocket-транспорт, стейт-машина шлюза Discord, IDENTIFY/RESUME и маршрутизация dispatch-событий; здесь же корень крейта discord-core.',
    match: (n) =>
      inCrate(n, 'discord-core') &&
      /^rust\/crates\/discord-core\/src\/(lib|dispatch|gateway|identify|resume|ws)\.rs$/.test(p(n)),
  },
  {
    id: 'layer:ready',
    name: 'READY и модель состояния',
    description:
      'Разбор READY/READY_SUPPLEMENTAL и модель состояния ядра: гильдии, read_state, social-граф, снапшоты и общие value-утилиты.',
    match: (n) =>
      inCrate(n, 'discord-core') &&
      /^rust\/crates\/discord-core\/src\/(ready|ready_data)\.rs$|^rust\/crates\/discord-core\/src\/ready\//.test(p(n)),
  },
  {
    id: 'layer:rest',
    name: 'REST-клиент',
    description:
      'Ручной HTTP/1.1 поверх tokio и rustls: каталог маршрутов /api/v9, заголовки, identity и super-properties, rate limit, retry, телеметрия и ack.',
    match: (n) => inCrate(n, 'discord-core') && /^rust\/crates\/discord-core\/src\/rest\//.test(p(n)),
  },
  {
    id: 'layer:codec',
    name: 'Бинарный протокол',
    description:
      'Крейт discord-codec: ETF-декодер (11 тегов, big-int) и потоковый zstd-декодер без I/O, на которых держится приём кадров шлюза.',
    match: (n) => inCrate(n, 'discord-codec') && /^rust\/crates\/discord-codec\/src\//.test(p(n)),
  },
  {
    id: 'layer:testing',
    name: 'Тесты и конформанс',
    description:
      'Интеграционные тесты discord-core и discord-codec с оракулами и фикстурами, а также dev-крейт discord-conformance для доступа к эталонным данным.',
    match: (n) => /\/tests\//.test(p(n)) || /^rust\/crates\/discord-conformance\/src\//.test(p(n)),
  },
  {
    id: 'layer:product-crates',
    name: 'Каркасы продуктовых крейтов',
    description:
      'Каркасы продуктовых крейтов Rust-клиента — UI на Tauri, голос, медиа, хранилище, платформенные интеграции, i18n и бинарь discord-app.',
    match: (n) =>
      /^rust\/crates\/discord-(app|ui|voice|media|store|platform|i18n)\/src\//.test(p(n)),
  },
  {
    id: 'layer:documentation',
    name: 'Документация и верификация',
    description:
      'Нормативная документация проекта и пакета rust-client (концепт, архитектура, план P0–P8, тест-план, трассировка) и оракул верификации verify-re.mjs с отчётом.',
    match: (n) => n.type === 'document' || p(n).startsWith('docs/rust-client/verify/'),
  },
  {
    id: 'layer:build-ci',
    name: 'Сборка и CI',
    description:
      'Конфигурация cargo-воркспейса и десяти крейтов плюс CI-пайплайн GitHub Actions с гейтами качества.',
    match: (n) =>
      (n.type === 'config' && !p(n).startsWith('docs/rust-client/verify/')) || n.type === 'pipeline',
  },
];

const layers = rules.map((r) => ({ id: r.id, name: r.name, description: r.description, nodeIds: [] }));
const assigned = new Map();

for (const n of nodes) {
  const matches = rules.filter((r) => r.match(n));
  if (matches.length !== 1) {
    throw new Error(`node ${n.id} matched ${matches.length} rules: ${matches.map((m) => m.id).join(',')}`);
  }
  const layer = layers.find((l) => l.id === matches[0].id);
  layer.nodeIds.push(n.id);
  assigned.set(n.id, layer.id);
}

// ---- validation ----
const errors = [];
const seen = new Set();
for (const l of layers) {
  if (!new RegExp('^layer:[a-z0-9]+(-[a-z0-9]+)*$').test(l.id)) errors.push(`bad id ${l.id}`);
  if (!l.name) errors.push(`${l.id}: empty name`);
  if (!l.description) errors.push(`${l.id}: empty description`);
  if (l.nodeIds.length === 0) errors.push(`${l.id}: empty nodeIds`);
  for (const id of l.nodeIds) {
    if (!byId.has(id)) errors.push(`${l.id}: unknown nodeId ${id}`);
    if (seen.has(id)) errors.push(`duplicate nodeId ${id}`);
    seen.add(id);
  }
}
if (layers.length < 5 || layers.length > 10) errors.push(`layer count ${layers.length} outside 5..10`);
const missing = nodes.filter((n) => !seen.has(n.id)).map((n) => n.id);
if (missing.length) errors.push(`unassigned nodes: ${missing.join(', ')}`);
if (seen.size !== nodes.length) errors.push(`assigned ${seen.size} of ${nodes.length}`);

if (errors.length) {
  process.stderr.write(errors.join('\n') + '\n');
  process.exit(1);
}

fs.mkdirSync(path.dirname(OUTPUT), { recursive: true });
fs.writeFileSync(OUTPUT, JSON.stringify(layers, null, 2) + '\n');

process.stdout.write(
  `wrote ${OUTPUT}\n` + layers.map((l) => `${l.id}\t${l.nodeIds.length}`).join('\n') + `\ntotal ${seen.size}\n`
);
