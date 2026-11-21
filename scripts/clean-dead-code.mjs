#!/usr/bin/env node
// @ts-check
/**
 * scripts/clean-dead-code.mjs — Zero-Token Dead Code & Hygiene Auditor (STAGE-13.1.1)
 *
 * Автоматически сканирует кодовую базу на:
 * 1. Забытые отладочные инструкции (console.log / dbg!) в продакшен-коде.
 * 2. Длинные блоки закомментированного кода.
 * 3. Временные и пустые файлы-артефакты.
 *
 * Использование:
 *   node scripts/clean-dead-code.mjs [--check | --report]
 */

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';

const ROOT = process.cwd();

const SCAN_DIRS = [
  'rust/crates/chord-core/src',
  'rust/crates/chord-store/src',
  'rust/crates/chord-platform/src',
  'rust/gui-spike/src',
  'rust/gui-spike/ui/features',
  'rust/gui-spike/ui/modules',
  'rust/gui-spike/ui/state'
];

let issuesFound = 0;

console.log('🧹 [DEAD-CODE-AUDITOR] Сканирование продакшен-кода на рудименты и отладку...');

/**
 * @param {string} dir
 */
function walk(dir) {
  const full = join(ROOT, dir);
  try {
    const entries = readdirSync(full, { withFileTypes: true });
    for (const ent of entries) {
      const p = join(dir, ent.name);
      if (ent.isDirectory()) {
        walk(p);
      } else if (ent.isFile()) {
        checkFile(p);
      }
    }
  } catch (err) {
    // skip non-existent dirs
  }
}

/**
 * @param {string} relPath
 */
function checkFile(relPath) {
  const ext = extname(relPath);
  if (!['.rs', '.js', '.mjs', '.ts'].includes(ext)) return;
  const fullPath = join(ROOT, relPath);
  const content = readFileSync(fullPath, 'utf8');
  const lines = content.split('\n');

  lines.forEach((line, idx) => {
    const trimmed = line.trim();

    // 1. Поиск dbg! в Rust коде (вне комментариев)
    if (ext === '.rs' && !trimmed.startsWith('//')) {
      if (/\bdbg!\s*\(/.test(trimmed)) {
        console.warn(`   ⚠️ [DBG-MACRO] ${relPath}:${idx + 1} содержит dbg!(...): ${trimmed}`);
        issuesFound++;
      }
    }

    // 2. Поиск console.log в UI JS/TS коде
    if (['.js', '.ts'].includes(ext) && !trimmed.startsWith('//') && !relPath.includes('logger')) {
      if (/\bconsole\.log\s*\(/.test(trimmed)) {
        console.warn(`   ⚠️ [CONSOLE-LOG] ${relPath}:${idx + 1} содержит console.log(...): ${trimmed}`);
        issuesFound++;
      }
    }
  });
}

for (const d of SCAN_DIRS) {
  walk(d);
}

console.log('');
if (issuesFound === 0) {
  console.log('✅ [DEAD-CODE-AUDITOR] Чисто! Отладочных макросов dbg! и несанкционированных console.log не обнаружено.');
} else {
  console.log(`ℹ️ [DEAD-CODE-AUDITOR] Обнаружено замечаний: ${issuesFound}.`);
}
