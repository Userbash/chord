#!/usr/bin/env node
// @ts-nocheck
/**
 * protocol-daemon.mjs — Zero-Token Unified Protocol & PR Collaboration Daemon
 *
 * Automates repetitive cycles without LLM intervention:
 * 1. Checks Orca messages, harvest completed workers, runs smart targeted tests.
 * 2. Reclaims settled worker terminals (worker-release).
 * 3. Commits passing code with Conventional Commits and pushes to remotes.
 * 4. Scans open PRs against AGENTS.md compliance, posts rework plans, or auto-approves.
 * 5. Salvages verified assets from external PRs.
 * 6. Enforces memory ceiling (RSS < 25.6 GiB).
 *
 * Usage:
 *   node scripts/protocol-daemon.mjs --status
 *   node scripts/protocol-daemon.mjs --once
 *   node scripts/protocol-daemon.mjs --watch [interval_sec]
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const AUTO_PIPELINE = path.join(__dirname, 'auto-pipeline.mjs');
const PR_COLLABORATOR = path.join(__dirname, 'pr-collaborator.mjs');
const KANBAN_TOOL = path.join(__dirname, 'kanban-tool.mjs');

function runCommand(cmd, opts = {}) {
  try {
    return execSync(cmd, { encoding: 'utf8', cwd: PROJECT_ROOT, ...opts });
  } catch (err) {
    return null;
  }
}

function getRssMemoryMb() {
  try {
    const psOut = execSync('ps -eo rss,comm', { encoding: 'utf8' });
    let total = 0;
    for (const l of psOut.split('\n')) {
      if (/antigravity|agy|orca|cargo|rustc|node/.test(l)) {
        const parts = l.trim().split(/\s+/);
        if (parts[0]) total += parseInt(parts[0], 10);
      }
    }
    return Math.round(total / 1024);
  } catch {
    return 0;
  }
}

/**
 * Run a single synchronization step
 */
function syncCycle() {
  console.log(`\n================================================================`);
  console.log(`⚡ PROTOCOL AUTONOMOUS SYNC CYCLE [${new Date().toLocaleTimeString()}]`);
  console.log(`================================================================`);

  // 1. Memory check
  const rss = getRssMemoryMb();
  const rssLimit = 25600;
  console.log(`💾 Memory Guard: RSS is ${rss}MB / ${rssLimit}MB (${Math.round((rss / rssLimit) * 100)}% of ceiling)`);
  if (rss > rssLimit) {
    console.error(`⚠️ Memory exceeds limit! Reclaiming reclaimable terminals...`);
    runCommand('orca orchestration worker-release --all-settled');
  }

  // 2. Run Pipeline step (Harvest, Test, Commit, Push, Kanban)
  console.log(`\n🌾 Step 1: Checking worker dispatches and harvesting...`);
  try {
    execSync(`node "${AUTO_PIPELINE}" --step`, { stdio: 'inherit', cwd: PROJECT_ROOT });
  } catch (err) {
    console.warn(`  ⚠️ Pipeline step noted failure or active tests:`, err.message);
  }

  // 3. Scan open PRs
  console.log(`\n🤝 Step 2: Auditing open Pull Requests against AGENTS.md...`);
  try {
    execSync(`node "${PR_COLLABORATOR}" scan`, { stdio: 'inherit', cwd: PROJECT_ROOT });
  } catch (err) {
    console.warn(`  ⚠️ PR scan failed:`, err.message);
  }

  // 4. Output Kanban status summary
  console.log(`\n📋 Step 3: Current Kanban Status:`);
  try {
    execSync(`node "${KANBAN_TOOL}" status`, { stdio: 'inherit', cwd: PROJECT_ROOT });
  } catch {}

  console.log(`================================================================\n`);
}

/**
 * Watch mode: continuously run sync cycles every interval
 */
async function watchLoop(intervalSec = 15) {
  console.log(`\n🚀 Starting Protocol Autonomous Watcher (interval: ${intervalSec}s, zero tokens)...`);
  console.log(`Press Ctrl+C to terminate.\n`);

  while (true) {
    syncCycle();

    // Check if 100% Kanban tasks are DONE
    try {
      const statusOut = execSync(`node "${KANBAN_TOOL}" status`, { encoding: 'utf8', cwd: PROJECT_ROOT });
      if (statusOut.includes('Done:           14 (100%)')) {
        console.log(`\n🎉🎉🎉 ALL WAVE RW-51 TASKS ARE COMPLETE (14/14 DONE)! 🎉🎉🎉`);
        console.log(`Protocol reached 100% completion. Exiting watch loop.`);
        break;
      }
    } catch {}

    await new Promise((resolve) => setTimeout(resolve, intervalSec * 1000));
  }
}

// CLI router
const args = process.argv.slice(2);
const mode = args[0] || '--once';

if (mode === '--watch') {
  const sec = parseInt(args[1] || '15', 10);
  watchLoop(sec);
} else if (mode === '--status') {
  const rss = getRssMemoryMb();
  console.log(`💾 Worker RSS: ${rss}MB / 25600MB`);
  execSync(`node "${KANBAN_TOOL}" status`, { stdio: 'inherit', cwd: PROJECT_ROOT });
  execSync(`node "${PR_COLLABORATOR}" scan`, { stdio: 'inherit', cwd: PROJECT_ROOT });
} else {
  syncCycle();
}
