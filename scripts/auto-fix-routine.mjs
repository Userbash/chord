// @ts-check
/**
 * scripts/auto-fix-routine.mjs
 *
 * Единый скрипт автоматизации всей рутины разработки (Zero-Token Dev Autopilot).
 *
 * Заменяет многократные ручные команды и вызовы ИИ на одну быструю команду (0.8с):
 * 1. Автоматическая починка реестра TDD-PLAN.md §8 (plan-autofix).
 * 2. Аудит и очистка битых импортов в тестах (test-auditor).
 * 3. Проверка согласованности вызовов UI и команд Rust BFF (gui-parity-audit).
 * 4. Мгновенная параллельная валидация TypeScript и мета-тестов (precheck).
 *
 * Запуск: npm run fix:routine
 */

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../', import.meta.url));

/**
 * @param {string} title
 * @param {string} cmd
 * @param {string[]} args
 * @returns {boolean}
 */
function step(title, cmd, args) {
  console.log(`▶️  ${title}...`);
  const t0 = Date.now();
  const res = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8' });
  const dur = ((Date.now() - t0) / 1000).toFixed(2);
  if (res.status === 0) {
    console.log(`   ✅ OK (${dur}s)`);
    return true;
  }
  console.error(`   ❌ FAILED (${dur}s):`);
  console.error(res.stdout || res.stderr);
  return false;
}

export function runAutoFixRoutine() {
  console.log('🤖 [AUTO-FIX-ROUTINE] Zero-Token Autonomous Development Autopilot');
  console.log('================================================================================');
  const tStart = Date.now();

  // 1. Починка реестра планов
  if (!step('1. Auto-fixing TDD-PLAN.md §8 registry', 'node', ['scripts/plan-autofix.mjs'])) {
    process.exit(1);
  }

  // 2. Аудит гигиены тестов
  if (!step('2. Auditing test suite imports & assertions', 'node', ['scripts/test-auditor.mjs'])) {
    process.exit(1);
  }

  // 3. Паритет UI <-> Rust BFF
  if (!step('3. Verifying GUI <-> Rust BFF feature parity', 'node', ['scripts/gui-parity-audit.mjs'])) {
    process.exit(1);
  }

  // 4. Субсекундный предполётный чек (TSC, Syntax, Meta)
  if (!step('4. Running sub-second precheck', 'node', ['scripts/precheck.mjs'])) {
    process.exit(1);
  }

  const total = ((Date.now() - tStart) / 1000).toFixed(2);
  console.log('================================================================================');
  console.log(`🎉 [AUTO-FIX-ROUTINE] All routine tasks successfully automated in ${total}s!`);
  console.log('   0 AI tokens consumed. Codebase is clean, tested, and ready for commit.');
  console.log('================================================================================\n');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runAutoFixRoutine();
}
