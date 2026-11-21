#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/git-sync.mjs
 *
 * Full zero-token Git and multi-remote automation harness (origin + public):
 * 1. Safe fetch and pull (PULL / FETCH): auto-stash, ff-only merge.
 * 2. Pre-flight checks (CHECK): typecheck, syntax, TDD plan sync, and security shield.
 * 3. Safe staging (STAGE): branch protection against forbidden paths (rust/chord-gpui/ on tauri).
 * 4. Smart commit (COMMIT / AUTO-COMMIT): auto-deduces Conventional Commits when no message provided.
 * 5. Reliable parallel push (PUSH): retries, timeouts, multi-remote push to origin and public.
 * 6. Full end-to-end cycle (SYNC): pull -> check -> stage & commit -> push.
 * 7. Background watch daemon (WATCH): continuous auto-sync on file change.
 *
 * Usage:
 *   node scripts/git-sync.mjs status               # Sync status and modified files
 *   node scripts/git-sync.mjs pull                 # Pull updates from all remotes
 *   node scripts/git-sync.mjs check                # Pre-commit zero-token validation checks
 *   node scripts/git-sync.mjs push                 # Push commits to origin and public
 *   node scripts/git-sync.mjs commit -m "msg"      # Check, stage, commit, and push
 *   node scripts/git-sync.mjs auto-commit          # Auto-deduce Conventional Commit and commit
 *   node scripts/git-sync.mjs sync                 # Pull + Check + Auto-Commit + Push
 *   node scripts/git-sync.mjs watch [seconds]      # Background continuous sync daemon
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
  // On the tauri branch, pre-commit hook strictly forbids committing rust/chord-gpui/
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
  console.log(`🔄 [GIT-SYNC STATUS] Branch: ${branch} | Remotes: ${remotes.join(', ')}`);
  console.log('================================================================================');

  for (const r of remotes) {
    const upstream = `${r}/${branch}`;
    const remoteRef = execQuiet(`git rev-parse --verify refs/remotes/${upstream} 2>/dev/null`);
    if (!remoteRef) {
      console.log(`ℹ️ [${r}] Branch ${upstream} is not tracked yet.`);
      continue;
    }
    const ahead = execQuiet(`git rev-list --count ${upstream}..HEAD`) || '0';
    const behind = execQuiet(`git rev-list --count HEAD..${upstream}`) || '0';
    console.log(`📡 [${r}] Ahead of remote: ${ahead} commit(s) | Behind remote: ${behind} commit(s)`);
  }

  console.log(`\n📂 Changes: ${allFiles.length} total | ${safeFiles.length} staged/safe for commit`);
  if (skippedFiles.length > 0) {
    console.log(`🛡️  Branch protection (ignored): ${skippedFiles.length} files in rust/chord-gpui/`);
  }
  if (safeFiles.length > 0) {
    console.log('Ready for synchronization:');
    safeFiles.slice(0, 10).forEach(f => console.log(`   + ${f}`));
    if (safeFiles.length > 10) console.log(`   ... and ${safeFiles.length - 10} more files`);
  }
}

export function cmdPull() {
  const branch = getCurrentBranch();
  const remotes = getConfiguredRemotes();
  console.log('================================================================================');
  console.log(`📥 [GIT-SYNC PULL] Fetching updates from all remotes (${remotes.join(', ')})...`);
  console.log('================================================================================');

  execQuiet('git fetch --all --prune');

  let hasStash = false;
  const statusBefore = execQuiet('git status --porcelain');
  if (statusBefore) {
    console.log('📦 Local changes detected. Stashing changes temporarily...');
    execQuiet('git stash push -m "git-sync-auto-stash"');
    hasStash = true;
  }

  const primary = remotes.includes('origin') ? 'origin' : remotes[0];
  const upstream = `${primary}/${branch}`;

  if (execQuiet(`git rev-parse --verify refs/remotes/${upstream} 2>/dev/null`)) {
    console.log(`🔄 Merging changes from ${upstream} (--ff-only)...`);
    const mergeRes = execQuiet(`git merge --ff-only ${upstream}`);
    if (mergeRes) {
      console.log(`✅ [GIT-SYNC] Successfully fast-forwarded from ${primary}!`);
    } else {
      console.warn(`⚠️ [GIT-SYNC] Fast-forward merge not possible. Branches may have diverged.`);
    }
  }

  if (hasStash) {
    console.log('📦 Restoring local changes from stash...');
    execQuiet('git stash pop');
  }

  console.log(`🏁 Current commit: ${execQuiet('git log -n 1 --oneline HEAD')}`);
  return true;
}

export function cmdCheck() {
  console.log('================================================================================');
  console.log('🔍 [GIT-SYNC CHECK] Zero-Token Pre-Commit Sanity & Security Check');
  console.log('================================================================================');

  // 1. Auto-sync TDD-PLAN.md §8
  console.log('🔍 1. Validating and synchronizing TDD-PLAN.md §8...');
  syncTddPlan({ fix: true });

  // 2. Precheck (TSC + Syntax + Meta)
  console.log('🔍 2. Running npm run precheck (TSC + Syntax + Meta)...');
  const precheckOk = runCommand('npm', ['run', 'precheck']);
  if (!precheckOk) {
    console.error('❌ [GIT-SYNC] Precheck failed!');
    return false;
  }

  // 3. Security Shield
  console.log('🔍 3. Security verification (Security Shield)...');
  const shieldOk = runCommand('node', ['scripts/security-shield.mjs']);
  if (!shieldOk) {
    console.error('❌ [GIT-SYNC] Security Shield detected vulnerabilities or secrets!');
    return false;
  }

  console.log('✅ [GIT-SYNC] All pre-commit checks passed in sub-seconds with zero tokens!');
  return true;
}

export function pushToRemote(remote, branch) {
  console.log(`🚀 [GIT-SYNC] Pushing changes to ${remote}/${branch}...`);
  for (let attempt = 1; attempt <= MAX_PUSH_RETRIES; attempt++) {
    try {
      execSync(`git push ${remote} ${branch}`, {
        cwd: PROJECT_ROOT,
        timeout: PUSH_TIMEOUT_MS,
        stdio: 'inherit'
      });
      console.log(`✅ [GIT-SYNC] Successfully pushed to ${remote}/${branch}!`);
      return true;
    } catch (err) {
      console.warn(`⚠️ [GIT-SYNC] Push attempt ${attempt}/${MAX_PUSH_RETRIES} to ${remote} failed: ${err.message}`);
      if (attempt < MAX_PUSH_RETRIES) {
        console.log('⏳ Waiting 3 seconds before retry...');
        execSync('sleep 3');
      }
    }
  }
  console.error(`❌ [GIT-SYNC] Failed to push changes to ${remote}.`);
  return false;
}

export function cmdPush() {
  const branch = getCurrentBranch();
  const remotes = getConfiguredRemotes();
  console.log(`🔄 [GIT-SYNC] Synchronizing branch '${branch}' across remotes: ${remotes.join(', ')}...`);

  let allSuccess = true;
  for (const remote of remotes) {
    const success = pushToRemote(remote, branch);
    if (!success) allSuccess = false;
  }
  return allSuccess;
}

export function deduceCommitMessage(files) {
  if (files.some(f => f.startsWith('README.md') || f.startsWith('docs/map.md'))) {
    return 'docs(readme): update project documentation, map, and architectural comparison';
  }
  if (files.some(f => f.startsWith('rust/crates/chord-media/'))) {
    return 'feat(media): update media pipeline and stream sources';
  }
  if (files.some(f => f.startsWith('rust/crates/chord-platform/'))) {
    return 'feat(platform): update system integrations and platform adapters';
  }
  if (files.some(f => f.startsWith('rust/crates/chord-voice/') || f.startsWith('rust/crates/chord-codec/'))) {
    return 'feat(voice): enhance voice signaling and codecs';
  }
  if (files.some(f => f.startsWith('waves/kanban-board.') || f.startsWith('waves/reserved-') || f.startsWith('waves/ROADMAP-STATUS.'))) {
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
    console.log('ℹ️ [GIT-SYNC] No changes to commit.');
    return true;
  }

  const commitMsg = customMessage || deduceCommitMessage(safeFiles);

  console.log(`📦 [GIT-SYNC] Staging ${safeFiles.length} safe files...`);
  for (const file of safeFiles) {
    execQuiet(`git add "${file}"`);
  }

  if (!cmdCheck()) {
    console.error('❌ [GIT-SYNC] Commit aborted due to check failure.');
    return false;
  }

  console.log(`📝 [GIT-SYNC] Creating commit: "${commitMsg}"...`);
  const ok = runCommand('git', ['commit', '-m', commitMsg]);
  if (!ok) {
    console.error('❌ [GIT-SYNC] git commit failed.');
    return false;
  }

  return cmdPush();
}

export function cmdSync(customMessage = null) {
  console.log('🔄 [GIT-SYNC] Full end-to-end sync cycle (SYNC)...');
  cmdPull();
  const ok = cmdCommit(customMessage);
  return ok;
}

export function cmdWatch(intervalSeconds = 15) {
  console.log(`👀 [GIT-SYNC WATCH] Background sync daemon started (interval ${intervalSeconds}s)...`);
  console.log('Press Ctrl+C to exit.');

  setInterval(() => {
    const allFiles = getChangedFiles();
    const safeFiles = filterSafeFiles(allFiles);
    if (safeFiles.length > 0) {
      console.log(`\n🔔 [GIT-SYNC WATCH] Changes detected (${safeFiles.length} files). Triggering auto-sync...`);
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
Usage: node scripts/git-sync.mjs <command>
  status              - Show sync status with remotes and changed files
  pull                - Pull updates from origin and public
  check               - Validate types, syntax, TDD plan, and security
  push                - Push commits to origin and public
  commit -m "msg"     - Stage safe files, validate, commit, and push
  auto-commit         - Auto-deduce commit type and create commit
  sync                - Full cycle: pull -> check -> commit -> push
  watch [seconds]     - Background continuous sync daemon (default: 15s)
`);
  }
}

if (process.argv[1] && process.argv[1].endsWith('git-sync.mjs')) {
  main();
}
