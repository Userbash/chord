// @ts-check
/**
 * scripts/plan-autofix.mjs
 *
 * Автоматический Zero-Token синхронизатор реестра TDD-PLAN.md §8.
 * Полностью исключает рутину ручного редактирования таблицы планов и экономит токены ИИ.
 *
 * При обнаружении новых тестов или расхождений токенов автоматически:
 * 1. Находит целевой модуль, импортируемый в тесте.
 * 2. Анализирует его экспорты.
 * 3. Формирует правильную строку в Markdown-таблице §8.
 * 4. Валидирует результат мета-тестом plan-sync.
 *
 * Запуск: npm run plan:fix
 */

import { readdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, basename, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const ROOT = fileURLToPath(new URL('../', import.meta.url));
const PLAN_FILE = join(ROOT, 'TDD-PLAN.md');
const TESTS_DIR = join(ROOT, 'tests');

/**
 * @param {string} dir
 * @returns {string[]}
 */
function walkTests(dir) {
  /** @type {string[]} */
  const results = [];
  if (!existsSync(dir)) return results;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...walkTests(full));
    } else if (entry.name.endsWith('.test.mjs') || entry.name.endsWith('.test.js')) {
      results.push(full);
    }
  }
  return results;
}

export function autoFixPlan() {
  console.log('⚡ [PLAN-AUTOFIX] Zero-Token TDD-PLAN.md §8 Synchronizer');
  console.log('================================================================================');

  const initialCheck = spawnSync('node', ['--test', 'tests/meta/plan-sync.test.mjs'], {
    cwd: ROOT,
    encoding: 'utf8',
  });

  if (initialCheck.status === 0) {
    console.log('✅ [PLAN-AUTOFIX] TDD-PLAN.md §8 is already 100% synchronized!');
    console.log('================================================================================\n');
    return;
  }

  console.log('🔄 Desynchronization detected. Inspecting test files and modules...');
  const allTests = walkTests(TESTS_DIR);
  let planContent = readFileSync(PLAN_FILE, 'utf8');

  let fixedCount = 0;
  for (const testPath of allTests) {
    const relTest = relative(ROOT, testPath);
    if (planContent.includes(`\`${relTest}\``)) {
      continue;
    }

    // Читаем тест и ищем целевой импортируемый модуль
    const testCode = readFileSync(testPath, 'utf8');
    const importMatch = testCode.match(/from\s+["'](\.\.?\/[^"']+)["']/);

    let targetModule = '—';
    let targetTokens = basename(relTest).replace('.test.mjs', '').replace('.test.js', '');

    if (importMatch && importMatch[1]) {
      const rawImport = importMatch[1];
      const resolved = join(join(testPath, '..'), rawImport);
      const relMod = relative(ROOT, resolved);
      targetModule = relMod;

      // Пробуем извлечь экспорты модуля
      try {
        const modCode = readFileSync(resolved, 'utf8');
        const exportMatches = [...modCode.matchAll(/export\s+(?:function|const|class)\s+([a-zA-Z0-9_$]+)/g)].map(
          (m) => `\`${m[1]}\``
        );
        if (exportMatches.length > 0) {
          targetTokens = exportMatches.slice(0, 3).join(', ');
        }
      } catch {
        // fallback
      }
    }

    const newRow = `| \`${relTest}\` | \`${targetModule}\` | ${targetTokens} |\n`;

    // Вставляем строку перед концом таблицы §8
    const marker = '| `tests/unit/static-fixture-server.test.mjs`';
    if (planContent.includes(marker)) {
      planContent = planContent.replace(marker, `${newRow}${marker}`);
      fixedCount++;
    }
  }

  writeFileSync(PLAN_FILE, planContent, 'utf8');
  console.log(`📝 Automatically synchronized ${fixedCount} test entry(ies) in TDD-PLAN.md §8.`);

  // Финальная валидация
  const finalCheck = spawnSync('node', ['--test', 'tests/meta/plan-sync.test.mjs'], {
    cwd: ROOT,
    encoding: 'utf8',
  });

  if (finalCheck.status === 0) {
    console.log('🎉 [PLAN-AUTOFIX] TDD-PLAN.md §8 successfully restored to 100% green state!\n');
  } else {
    console.log('⚠️  Partial sync complete. Verification output:');
    console.log(finalCheck.stdout || finalCheck.stderr);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  autoFixPlan();
}
