#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/git-sync.mjs
 *
 * Полная zero-token автоматизация работы с Git и мульти-репозиториями (origin + public):
 * 1. Безопасное стягивание (PULL / FETCH): авто-stash, ff-only слияние.
 * 2. Предполётные проверки (CHECK): валидация типов, синтаксиса, TDD-плана и безопасности.
 * 3. Безопасная индексация (STAGE): фильтрация запрещенных путей (rust/chord-gpui/ на tauri).
 * 4. Умный коммит (COMMIT / AUTO-COMMIT): авто-генерация Conventional Commits при отсутствии явного текста.
 * 5. Надежная параллельная отправка (PUSH): повторные попытки, таймауты, пуш в origin и public.
 * 6. Полный сквозной цикл (SYNC): pull -> check -> stage & commit -> push.
 * 7. Фоновый режим слежения (WATCH): автоматическая синхронизация при сохранении файлов.
 *
 * Использование:
 *   node scripts/git-sync.mjs status               # Статус синхронизации и измененные файлы
 *   node scripts/git-sync.mjs pull                 # Стянуть свежие коммиты со всех remotes
 *   node scripts/git-sync.mjs check                # Предкоммитные zero-token проверки
 *   node scripts/git-sync.mjs push                 # Запушить готовые коммиты в origin и public
 *   node scripts/git-sync.mjs commit -m "msg"      # Проверить, закоммитить и запушить
 *   node scripts/git-sync.mjs auto-commit          # Автоматически определить тип и закоммитить
 *   node scripts/git-sync.mjs sync                 # Pull + Check + Auto-Commit + Push
 *   node scripts/git-sync.mjs watch [seconds]      # Фоновый демон авто-синхронизации
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { syncTddPlan } from './sync-tdd-plan.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const REMOTES = ['origin', 'public'];
const PUSH_TIMEOUT_MS = 60000;
const MAX_PUSH_RETRIES = 3;

function execQuiet(cmd, opts = {}) {
  try {
    return execSync(cmd, { cwd: PROJECT_ROOT, encoding: 'utf8', stdio: 'pipe', ...opts }).trim();
  } catch (err) {
    return null;
  }
}

function runCommand(cmd, args = [], opts = {}) {
  const res = spawnSync(cmd, args, { cwd: PROJECT_ROOT, stdio: 'inherit', ...opts });
  return res.status === 0;
}

export function getCurrentBranch() {
  return execQuiet('git branch --show-current') || 'tauri';
}

export function getConfiguredRemotes() {
  const configured = [];
  for (const r of REMOTES) {
    if (execQuiet(`git remote | grep -w "^${r}$"`)) {
      configured.push(r);
    }
  }
  return configured.length > 0 ? configured : ['origin'];
}

export function getChangedFiles() {
  const statusOut = execQuiet('git status --porcelain=v1') || '';
  if (!statusOut) return [];
  return statusOut
    .split('\n')
    .map(line => {
      const match = line.match(/^..\s+(.*)$/);
      return match ? match[1].trim() : null;
    })
    .filter(Boolean);
}

export function filterSafeFiles(files) {
  const branch = getCurrentBranch();
  // На ветке tauri pre-commit хук строго запрещает коммитить rust/chord-gpui/
  if (branch === 'tauri') {
    return files.filter(f => !f.startsWith('rust/chord-gpui/'));
  }
  return files;
}

export function cmdStatus() {
  const branch = getCurrentBranch();
  const remotes = getConfiguredRemotes();
  const allFiles = getChangedFiles();
  const safeFiles = filterSafeFiles(allFiles);
  const skippedFiles = allFiles.filter(f => !safeFiles.includes(f));

  console.log('================================================================================');
  console.log(`🔄 [GIT-SYNC STATUS] Ветка: ${branch} | Remotes: ${remotes.join(', ')}`);
  console.log('================================================================================');

  for (const r of remotes) {
    const upstream = `${r}/${branch}`;
    const remoteRef = execQuiet(`git rev-parse --verify refs/remotes/${upstream} 2>/dev/null`);
    if (!remoteRef) {
      console.log(`ℹ️ [${r}] Ветка ${upstream} еще не отслеживается.`);
      continue;
    }
    const ahead = execQuiet(`git rev-list --count ${upstream}..HEAD`) || '0';
    const behind = execQuiet(`git rev-list --count HEAD..${upstream}`) || '0';
    console.log(`📡 [${r}] Опережает remote на: ${ahead} коммит(ов) | Отстает от remote на: ${behind} коммит(ов)`);
  }

  console.log(`\n📂 Изменения: ${allFiles.length} всего | ${safeFiles.length} готовы к коммиту`);
  if (skippedFiles.length > 0) {
    console.log(`🛡️  Защита ветки (игнорируются): ${skippedFiles.length} файлов в rust/chord-gpui/`);
  }
  if (safeFiles.length > 0) {
    console.log('Готовы к синхронизации:');
    safeFiles.slice(0, 10).forEach(f => console.log(`   + ${f}`));
    if (safeFiles.length > 10) console.log(`   ... и еще ${safeFiles.length - 10} файлов`);
  }
}

export function cmdPull() {
  const branch = getCurrentBranch();
  const remotes = getConfiguredRemotes();
  console.log('================================================================================');
  console.log(`📥 [GIT-SYNC PULL] Извлечение обновлений со всех remotes (${remotes.join(', ')})...`);
  console.log('================================================================================');

  execQuiet('git fetch --all --prune');

  let hasStash = false;
  const statusBefore = execQuiet('git status --porcelain');
  if (statusBefore) {
    console.log('📦 Обнаружены локальные изменения. Создание временного stash...');
    execQuiet('git stash push -m "git-sync-auto-stash"');
    hasStash = true;
  }

  const primary = remotes.includes('origin') ? 'origin' : remotes[0];
  const upstream = `${primary}/${branch}`;

  if (execQuiet(`git rev-parse --verify refs/remotes/${upstream} 2>/dev/null`)) {
    console.log(`🔄 Слияние изменений из ${upstream} (--ff-only)...`);
    const mergeRes = execQuiet(`git merge --ff-only ${upstream}`);
    if (mergeRes) {
      console.log(`✅ [GIT-SYNC] Успешно обновлено из ${primary}!`);
    } else {
      console.warn(`⚠️ [GIT-SYNC] Fast-forward слияние невозможно. Ветки разошлись.`);
    }
  }

  if (hasStash) {
    console.log('📦 Восстановление локальных изменений из stash...');
    execQuiet('git stash pop');
  }

  console.log(`🏁 Текущий коммит: ${execQuiet('git log -n 1 --oneline HEAD')}`);
  return true;
}

export function cmdCheck() {
  console.log('================================================================================');
  console.log('🔍 [GIT-SYNC CHECK] Zero-Token Pre-Commit Sanity & Security Check');
  console.log('================================================================================');

  // 1. Авто-синхронизация TDD-PLAN.md §8
  console.log('🔍 1. Проверка и синхронизация TDD-PLAN.md §8...');
  syncTddPlan({ fix: true });

  // 2. Precheck (TSC + Syntax + Meta)
  console.log('🔍 2. Запуск npm run precheck (TSC + Syntax + Meta)...');
  const precheckOk = runCommand('npm', ['run', 'precheck']);
  if (!precheckOk) {
    console.error('❌ [GIT-SYNC] Precheck завершился с ошибкой!');
    return false;
  }

  // 3. Security Shield
  console.log('🔍 3. Проверка безопасности (Security Shield)...');
  const shieldOk = runCommand('node', ['scripts/security-shield.mjs']);
  if (!shieldOk) {
    console.error('❌ [GIT-SYNC] Security Shield обнаружил уязвимости или секреты!');
    return false;
  }

  console.log('✅ [GIT-SYNC] Все проверки пройдены за доли секунды без затрат токенов!');
  return true;
}

export function pushToRemote(remote, branch) {
  console.log(`🚀 [GIT-SYNC] Отправка изменений в ${remote}/${branch}...`);
  for (let attempt = 1; attempt <= MAX_PUSH_RETRIES; attempt++) {
    try {
      execSync(`git push ${remote} ${branch}`, {
        cwd: PROJECT_ROOT,
        timeout: PUSH_TIMEOUT_MS,
        stdio: 'inherit'
      });
      console.log(`✅ [GIT-SYNC] Успешно отправлено в ${remote}/${branch}!`);
      return true;
    } catch (err) {
      console.warn(`⚠️ [GIT-SYNC] Попытка ${attempt}/${MAX_PUSH_RETRIES} отправки в ${remote} не удалась: ${err.message}`);
      if (attempt < MAX_PUSH_RETRIES) {
        console.log('⏳ Ожидание 3 секунды перед повторной попыткой...');
        execSync('sleep 3');
      }
    }
  }
  console.error(`❌ [GIT-SYNC] Не удалось отправить изменения в ${remote}.`);
  return false;
}

export function cmdPush() {
  const branch = getCurrentBranch();
  const remotes = getConfiguredRemotes();
  console.log(`🔄 [GIT-SYNC] Синхронизация ветки '${branch}' с remotes: ${remotes.join(', ')}...`);

  let allSuccess = true;
  for (const remote of remotes) {
    const success = pushToRemote(remote, branch);
    if (!success) allSuccess = false;
  }
  return allSuccess;
}

export function deduceCommitMessage(files) {
  if (files.some(f => f.startsWith('rust/crates/chord-media/'))) {
    return 'feat(media): update media pipeline and stream sources';
  }
  if (files.some(f => f.startsWith('rust/crates/chord-platform/'))) {
    return 'feat(platform): update system integrations and platform adapters';
  }
  if (files.some(f => f.startsWith('rust/crates/chord-voice/') || f.startsWith('rust/crates/chord-codec/'))) {
    return 'feat(voice): enhance voice signaling and codecs';
  }
  if (files.some(f => f.startsWith('waves/kanban-board.') || f.startsWith('waves/reserved-'))) {
    return 'docs(kanban): synchronize kanban tasks and worker slots';
  }
  if (files.some(f => f.startsWith('tests/'))) {
    return 'test(contract): expand contract and unit test suites';
  }
  if (files.some(f => f.startsWith('scripts/'))) {
    return 'chore(scripts): optimize zero-token developer automation';
  }
  return 'chore(auto): update verified changes';
}

export function cmdCommit(customMessage = null) {
  const allFiles = getChangedFiles();
  const safeFiles = filterSafeFiles(allFiles);

  if (safeFiles.length === 0) {
    console.log('ℹ️ [GIT-SYNC] Нет изменений для коммита.');
    return true;
  }

  const commitMsg = customMessage || deduceCommitMessage(safeFiles);

  console.log(`📦 [GIT-SYNC] Индексация ${safeFiles.length} безопасных файлов...`);
  for (const file of safeFiles) {
    execQuiet(`git add "${file}"`);
  }

  if (!cmdCheck()) {
    console.error('❌ [GIT-SYNC] Коммит отменен из-за ошибок валидации.');
    return false;
  }

  console.log(`📝 [GIT-SYNC] Создание коммита: "${commitMsg}"...`);
  const ok = runCommand('git', ['commit', '-m', commitMsg]);
  if (!ok) {
    console.error('❌ [GIT-SYNC] Ошибка git commit.');
    return false;
  }

  return cmdPush();
}

export function cmdSync(customMessage = null) {
  console.log('🔄 [GIT-SYNC] Полный сквозной цикл синхронизации (SYNC)...');
  cmdPull();
  const ok = cmdCommit(customMessage);
  return ok;
}

export function cmdWatch(intervalSeconds = 15) {
  console.log(`👀 [GIT-SYNC WATCH] Запущен фоновый демон синхронизации (интервал ${intervalSeconds}с)...`);
  console.log('Нажмите Ctrl+C для остановки.');

  setInterval(() => {
    const allFiles = getChangedFiles();
    const safeFiles = filterSafeFiles(allFiles);
    if (safeFiles.length > 0) {
      console.log(`\n🔔 [GIT-SYNC WATCH] Обнаружены изменения (${safeFiles.length} файлов). Запуск авто-синхронизации...`);
      cmdCommit();
    }
  }, intervalSeconds * 1000);
}

function main() {
  const args = process.argv.slice(2);
  const command = args[0] || 'status';

  if (command === 'status' || command === '--status') {
    cmdStatus();
  } else if (command === 'pull' || command === '--pull') {
    const ok = cmdPull();
    process.exit(ok ? 0 : 1);
  } else if (command === 'check' || command === '--check') {
    const ok = cmdCheck();
    process.exit(ok ? 0 : 1);
  } else if (command === 'push' || command === '--push' || command === '--push-only') {
    const ok = cmdPush();
    process.exit(ok ? 0 : 1);
  } else if (command === 'commit' || command === '--commit') {
    const msg = args[1] === '-m' ? args[2] : (args[1] || null);
    const ok = cmdCommit(msg);
    process.exit(ok ? 0 : 1);
  } else if (command === 'auto-commit' || command === '--auto-commit') {
    const ok = cmdCommit(null);
    process.exit(ok ? 0 : 1);
  } else if (command === 'sync' || command === '--sync') {
    const msg = args[1] === '-m' ? args[2] : (args[1] || null);
    const ok = cmdSync(msg);
    process.exit(ok ? 0 : 1);
  } else if (command === 'watch' || command === '--watch') {
    const sec = parseInt(args[1], 10) || 15;
    cmdWatch(sec);
  } else {
    console.log(`
Использование: node scripts/git-sync.mjs <команда>
  status              - Показать статус расхождений с remotes и измененные файлы
  pull                - Стянуть обновления с origin и public
  check               - Проверить типы, синтаксис, TDD-план и безопасность
  push                - Отправить коммиты в origin и public
  commit -m "msg"     - Проиндексировать безопасные файлы, проверить и запушить
  auto-commit         - Автоматически определить тип и создать коммит
  sync                - Полный цикл: pull -> check -> commit -> push
  watch [сек]         - Фоновый демон непрерывной синхронизации (по умолчанию 15с)
`);
  }
}

if (process.argv[1] && process.argv[1].endsWith('git-sync.mjs')) {
  main();
}
