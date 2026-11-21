#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/report-diff.mjs — Zero-Token Command diff comparing two inventories/reports
 *
 * Сравнивает два отчета, JSONL-телеметрии, инвентаря пакетов или дампов тестов
 * без затрат токенов ИИ. Выводит разницу добавленных, удаленных и измененных ключей/строк.
 *
 * Использование:
 *   node scripts/report-diff.mjs <fileA> <fileB> [--json]
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

function parseContent(filePath) {
  if (!existsSync(filePath)) {
    return null;
  }
  const raw = readFileSync(filePath, 'utf8').trim();
  try {
    return JSON.parse(raw);
  } catch {
    // Treat as lines or jsonl
    const lines = raw.split('\n').filter(Boolean);
    const jsonl = [];
    for (const line of lines) {
      try {
        jsonl.push(JSON.parse(line));
      } catch {
        // Just text lines
        return { type: 'lines', data: lines };
      }
    }
    return { type: 'jsonl', data: jsonl };
  }
}

export function diffReports(fileA, fileB) {
  const dataA = parseContent(fileA);
  const dataB = parseContent(fileB);

  if (!dataA || !dataB) {
    return {
      error: `One or both files do not exist: ${fileA}, ${fileB}`,
      diffs: []
    };
  }

  const keysA = new Set(Object.keys(dataA));
  const keysB = new Set(Object.keys(dataB));

  const added = [...keysB].filter(k => !keysA.has(k));
  const removed = [...keysA].filter(k => !keysB.has(k));
  const common = [...keysA].filter(k => keysB.has(k));
  const modified = [];

  for (const k of common) {
    if (JSON.stringify(dataA[k]) !== JSON.stringify(dataB[k])) {
      modified.push({ key: k, before: dataA[k], after: dataB[k] });
    }
  }

  return { added, removed, modified };
}

function main() {
  const args = process.argv.slice(2).filter(a => !a.startsWith('--'));
  if (args.length < 2) {
    console.log('✅ [REPORT-DIFF] Готов к работе. Использование: node scripts/report-diff.mjs <fileA> <fileB>');
    process.exit(0);
  }

  const [fileA, fileB] = args.map(f => resolve(process.cwd(), f));
  const result = diffReports(fileA, fileB);

  if (process.argv.includes('--json')) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  console.log(`=== [REPORT-DIFF] ${fileA} vs ${fileB} ===`);
  if (result.error) {
    console.error(`❌ ${result.error}`);
    process.exit(1);
  }

  console.log(`➕ Добавлено: ${result.added.length} элементов (${result.added.slice(0, 5).join(', ')}...)`);
  console.log(`➖ Удалено:   ${result.removed.length} элементов (${result.removed.slice(0, 5).join(', ')}...)`);
  console.log(`🔄 Изменено:  ${result.modified.length} элементов`);
}

if (process.argv[1] && process.argv[1].endsWith('report-diff.mjs')) {
  main();
}
