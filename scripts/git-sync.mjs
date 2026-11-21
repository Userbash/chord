#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/git-sync.mjs
 *
 * Автоматизированный скрипт безопасной синхронизации с Git и мульти-репозиториями.
 * Экономит тысячи токенов ИИ на рутинных командах git add, commit, push, retry и precheck.
 *
 * Возможности:
 * 1. Проверяет текущую ветку (защита ветки tauri).
 * 2. Исключает запрещенные правилами директории (rust/chord-gpui/ на ветке tauri).
 * 3. Автоматически запускает npm run precheck перед коммитом.
 * 4. Коммитит изменения по правилам Conventional Commits.
 * 5. Пушит изменения в origin и public с таймаутом и повторными попытками.
 *
 * Использование:
 *   node scripts/git-sync.mjs --commit "feat: сообщение коммита"
 *   node scripts/git-sync.mjs --push-only
 *   node scripts/git-sync.mjs --status
 */

import { execSync, spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const REMOTES = ['origin', 'public'];
const TARGET_BRANCH = 'tauri';
const PUSH_TIMEOUT_MS = 60000;
const MAX_PUSH_RETRIES = 2;

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
  return execQuiet('git branch --show-current') || '';
}

export function getChangedFiles() {
  const statusOut = execQuiet('git status --porcelain=v1') || '';
  if (!statusOut) return [];
  return statusOut
    .split('\n')
    .map(line => line.trim())
    .filter(Boolean)
    .map(line => line.slice(3).trim());
}

export function filterSafeFiles(files) {
  const branch = getCurrentBranch();
  // На ветке tauri pre-commit хук строго запрещает коммитить rust/chord-gpui/
  if (branch === 'tauri') {
    return files.filter(f => !f.startsWith('rust/chord-gpui/'));
  }
  return files;
}

export function runPrecheck() {
  console.log('🔍 [GIT-SYNC] Запуск быстрой предварительной проверки precheck...');
  const ok = runCommand('npm', ['run', 'precheck']);
  if (!ok) {
    console.error('❌ [GIT-SYNC] Precheck завершился ошибкой. Коммит отменен.');
    return false;
  }
  console.log('✅ [GIT-SYNC] Precheck пройден успешно.');
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
      console.warn(`⚠️ [GIT-SYNC] Попытка ${attempt}/${MAX_PUSH_RETRIES} отправки в ${remote} завершилась неудачей: ${err.message}`);
      if (attempt < MAX_PUSH_RETRIES) {
        console.log('⏳ Ожидание 3 секунды перед повтором...');
        execSync('sleep 3');
      }
    }
  }
  console.error(`❌ [GIT-SYNC] Не удалось отправить изменения в ${remote}.`);
  return false;
}

export function syncMultiRemotes() {
  const branch = getCurrentBranch();
  console.log(`🔄 [GIT-SYNC] Синхронизация удаленных репозиториев для ветки '${branch}'...`);
  let allSuccess = true;
  for (const remote of REMOTES) {
    const success = pushToRemote(remote, branch);
    if (!success) allSuccess = false;
  }
  return allSuccess;
}

export function stageAndCommit(commitMessage) {
  if (!commitMessage || typeof commitMessage !== 'string' || !commitMessage.trim()) {
    console.error('❌ [GIT-SYNC] Ошибка: требуется указать непустое сообщение коммита (--commit "msg").');
    return false;
  }

  const allFiles = getChangedFiles();
  const safeFiles = filterSafeFiles(allFiles);

  if (safeFiles.length === 0) {
    console.log('ℹ️ [GIT-SYNC] Нет изменений для коммита (все изменения либо отсутствуют, либо в игнорируемых путях).');
    return true;
  }

  console.log(`📦 [GIT-SYNC] Добавление ${safeFiles.length} безопасных файлов в индекс:`);
  safeFiles.forEach(f => console.log(`   + ${f}`));

  for (const file of safeFiles) {
    execQuiet(`git add "${file}"`);
  }

  if (!runPrecheck()) {
    return false;
  }

  console.log(`📝 [GIT-SYNC] Создание коммита: "${commitMessage}"...`);
  const commitOk = runCommand('git', ['commit', '-m', commitMessage]);
  if (!commitOk) {
    console.error('❌ [GIT-SYNC] Ошибка при создании коммита.');
    return false;
  }

  return syncMultiRemotes();
}

function printStatus() {
  const branch = getCurrentBranch();
  const allFiles = getChangedFiles();
  const safeFiles = filterSafeFiles(allFiles);
  const skippedFiles = allFiles.filter(f => !safeFiles.includes(f));

  console.log('=== [GIT-SYNC STATUS] ===');
  console.log(`Текущая ветка: ${branch}`);
  console.log(`Всего измененных файлов: ${allFiles.length}`);
  console.log(`Готовы к коммиту (Safe): ${safeFiles.length}`);
  if (skippedFiles.length > 0) {
    console.log(`Исключены (Branch Protection): ${skippedFiles.length} (например, rust/chord-gpui/*)`);
  }
  console.log('=========================');
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes('--status')) {
    printStatus();
    return;
  }

  if (args.includes('--push-only')) {
    syncMultiRemotes();
    return;
  }

  const commitIdx = args.indexOf('--commit');
  if (commitIdx !== -1 && args[commitIdx + 1]) {
    const msg = args[commitIdx + 1];
    const success = stageAndCommit(msg);
    process.exit(success ? 0 : 1);
  }

  console.log(`Использование git-sync.mjs:
  node scripts/git-sync.mjs --commit "feat: сообщение"  # Проверить, закоммитить и запушить в remotes
  node scripts/git-sync.mjs --push-only                 # Запушить текущую ветку в origin и public
  node scripts/git-sync.mjs --status                    # Показать статус и фильтрацию файлов
`);
}

if (process.argv[1] && process.argv[1].endsWith('git-sync.mjs')) {
  main();
}
