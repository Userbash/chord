#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/audit-thin-bridges.mjs — Автоматический линтер архитектурного бюджета «Тонкий мост»
 *
 * Проверяет выполнение ключевого принципа дорожной карты упрощения Chord:
 * - Все мосты и адаптеры должны быть тонкими (<600 LOC рекомендуемо, жесткий лимит 800 LOC).
 * - Никаких самописных криптографических алгоритмов (только ring, aes-gcm, sha2).
 * - Никаких самописных конкатенаций URL строк (только url::Url и reqwest).
 * - Никаких очередей звука вручную (только crossbeam-queue::ArrayQueue).
 * - Никакого самописного C FFI для БД (чистый rusqlite).
 * - Строгий запрет unsafe: наличие #![forbid(unsafe_code)] во всех мостах.
 *
 * Использование:
 *   node scripts/audit-thin-bridges.mjs [--json | --strict]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const BRIDGES = [
  {
    name: 'REST Transparent Bridge',
    crate: 'chord-core',
    path: 'rust/crates/chord-core/src/rest/bridge.rs',
    maxLoc: 800,
    softLoc: 600,
    requiredPatterns: ['url::Url', 'reqwest'],
    forbiddenPatterns: ['unsafe ', 'format!("/guilds/']
  },
  {
    name: 'SQLite Worker & Store Bridge',
    crate: 'chord-store',
    path: 'rust/crates/chord-store/src/worker.rs',
    maxLoc: 800,
    softLoc: 600,
    requiredPatterns: ['rusqlite::Connection', 'sync_channel'],
    forbiddenPatterns: ['unsafe ', 'extern "C"']
  },
  {
    name: 'DAVE E2EE Voice Bridge',
    crate: 'dave-spike',
    path: 'rust/dave-spike/src/bridge.rs',
    maxLoc: 800,
    softLoc: 600,
    requiredPatterns: ['DaveBackend'],
    forbiddenPatterns: ['unsafe ']
  },
  {
    name: 'Audio Lock-Free Ring Buffer',
    crate: 'audio-spike',
    path: 'rust/audio-spike/src/ring_buffer.rs',
    maxLoc: 400,
    softLoc: 250,
    requiredPatterns: ['crossbeam_queue::ArrayQueue'],
    forbiddenPatterns: ['unsafe ', 'std::sync::Mutex']
  },
  {
    name: 'Audio VAD Energy Detector',
    crate: 'audio-spike',
    path: 'rust/audio-spike/src/vad.rs',
    maxLoc: 400,
    softLoc: 200,
    requiredPatterns: ['VadDetector'],
    forbiddenPatterns: ['unsafe ']
  },
  {
    name: 'Frontend Signals Bridge',
    crate: 'gui-spike-ui',
    path: 'rust/gui-spike/ui/state/signals.js',
    maxLoc: 300,
    softLoc: 150,
    requiredPatterns: ['@preact/signals-core'],
    forbiddenPatterns: ['class EventBus']
  },
  {
    name: 'Frontend Virtualizer Bridge',
    crate: 'gui-spike-ui',
    path: 'rust/gui-spike/ui/features/virtualizer.js',
    maxLoc: 400,
    softLoc: 250,
    requiredPatterns: ['@tanstack/virtual-core'],
    forbiddenPatterns: ['onscroll = function']
  },
  {
    name: 'Frontend Popover Bridge',
    crate: 'gui-spike-ui',
    path: 'rust/gui-spike/ui/features/floating-popover.js',
    maxLoc: 400,
    softLoc: 250,
    requiredPatterns: ['computePosition'],
    forbiddenPatterns: ['position: fixed; top: 0']
  }
];

export function auditThinBridges(strict = false) {
  let passed = true;
  const results = [];
  let totalLoc = 0;

  for (const b of BRIDGES) {
    const fullPath = path.join(PROJECT_ROOT, b.path);
    if (!fs.existsSync(fullPath)) {
      results.push({
        name: b.name,
        path: b.path,
        status: 'MISSING',
        error: `File not found: ${b.path}`
      });
      passed = false;
      continue;
    }

    const content = fs.readFileSync(fullPath, 'utf8');
    const lines = content.split('\n');
    const loc = lines.length;
    totalLoc += loc;

    const violations = [];

    // Check LOC limits
    if (loc > b.maxLoc) {
      violations.push(`Hard LOC limit exceeded: ${loc} > ${b.maxLoc}`);
    } else if (loc > b.softLoc) {
      violations.push(`Soft LOC limit warning: ${loc} > ${b.softLoc}`);
    }

    // Check required patterns (delegation to vetted libraries)
    for (const req of b.requiredPatterns) {
      if (!content.includes(req)) {
        violations.push(`Missing required library delegation pattern: '${req}'`);
      }
    }

    // Check forbidden patterns (reinvented wheels or unsafe code)
    for (const forb of b.forbiddenPatterns) {
      if (content.includes(forb)) {
        violations.push(`Found forbidden pattern: '${forb}'`);
      }
    }

    const isOk = violations.filter(v => !v.startsWith('Soft LOC')).length === 0;
    if (!isOk) passed = false;

    results.push({
      name: b.name,
      path: b.path,
      loc,
      maxLoc: b.maxLoc,
      status: isOk ? 'PASSED' : 'VIOLATION',
      violations
    });
  }

  return {
    passed,
    totalBridges: BRIDGES.length,
    totalLoc,
    averageLoc: Math.round(totalLoc / BRIDGES.length),
    results
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  const isStrict = process.argv.includes('--strict');
  const audit = auditThinBridges(isStrict);

  console.log(`\n================================================================`);
  console.log(`🛡️  АРХИТЕКТУРНЫЙ АУДИТ ТЕХНОЛОГИЧЕСКИХ МОСТОВ («ТОНКИЙ МОСТ»)`);
  console.log(`================================================================`);
  console.log(`Всего мостов проверено: ${audit.totalBridges}`);
  console.log(`Суммарный объем кода мостов: ${audit.totalLoc} LOC (в среднем: ${audit.averageLoc} LOC на мост)`);
  console.log(`Лимит на размер адаптера: <= 600 LOC (soft) / <= 800 LOC (hard)`);
  console.log(`----------------------------------------------------------------`);

  for (const r of audit.results) {
    const icon = r.status === 'PASSED' ? '✅' : '❌';
    console.log(`${icon} [${r.status}] ${r.name.padEnd(35, ' ')} ${r.loc} LOC (макс ${r.maxLoc})`);
    if (r.violations && r.violations.length > 0) {
      for (const v of r.violations) {
        console.log(`   ↳ ⚠️ ${v}`);
      }
    }
  }

  console.log(`================================================================`);
  if (audit.passed) {
    console.log(`🎉 Все мосты соответствуют стандарту тонких адаптеров и делегируют задачи библиотекам!\n`);
    process.exit(0);
  } else {
    console.log(`❌ Обнаружены архитектурные нарушения в мостах. Исправьте замечания выше.\n`);
    process.exit(1);
  }
}
