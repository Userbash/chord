#!/usr/bin/env node
// @ts-nocheck
/**
 * auto-pipeline.mjs — Zero-Token Autonomous Orchestration Pipeline
 *
 * Automates repetitive cycles previously executed by LLMs:
 * 1. Checks Orca messages & active worker statuses
 * 2. Identifies settled/completed workers
 * 3. Runs targeted automated tests (test-runner-smart.mjs)
 * 4. Reclaims worker terminals (worker-release)
 * 5. Commits changes with Conventional Commits & pushes to remotes
 * 6. Updates Kanban board (kanban-tool.mjs)
 * 7. Dispatches next queued tasks within RAM & concurrency limits
 *
 * Usage:
 *   node scripts/auto-pipeline.mjs --status
 *   node scripts/auto-pipeline.mjs --step
 *   node scripts/auto-pipeline.mjs --harvest
 *   node scripts/auto-pipeline.mjs --dispatch
 *   node scripts/auto-pipeline.mjs --watch [interval_sec]
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { findKanbanPath, parseKanban, updateTaskStatus, getStats } from './kanban-tool.mjs';
import { runTests, getChangedFiles } from './test-runner-smart.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const RUN_STATE_FILE = path.join(PROJECT_ROOT, '.claude/ecc-orca-run.json');

const MAX_ACTIVE_WORKERS = parseInt(process.env.MAX_ACTIVE_WORKERS || '4', 10);
const WORKER_RAM_LIMIT_MB = parseInt(process.env.WORKER_RAM_LIMIT_MB || '25600', 10);
const ORCA_BIN = process.env.ORCA_BIN || '/home/sanya/.config/orca/linux-orca-cli-shim/orca';

function getActiveRunId() {
  try {
    if (fs.existsSync(RUN_STATE_FILE)) {
      const data = JSON.parse(fs.readFileSync(RUN_STATE_FILE, 'utf8'));
      if (data.runId) return data.runId;
    }
  } catch {}
  return 'run_31999544da31'; // fallback to current wave run
}

function getMemoryMetrics() {
  let memAvailableMb = 0;
  try {
    const meminfo = fs.readFileSync('/proc/meminfo', 'utf8');
    const m = meminfo.match(/MemAvailable:\s+(\d+)\s+kB/);
    if (m && m[1]) memAvailableMb = Math.round(parseInt(m[1], 10) / 1024);
  } catch {}

  let agentRssMb = 0;
  try {
    const psOut = execSync('ps -eo rss,comm', { encoding: 'utf8' });
    const lines = psOut.split('\n');
    for (const l of lines) {
      if (/antigravity|agy|orca|cargo|rustc|node/.test(l)) {
        const parts = l.trim().split(/\s+/);
        if (parts[0]) agentRssMb += parseInt(parts[0], 10);
      }
    }
    agentRssMb = Math.round(agentRssMb / 1024);
  } catch {}

  return { memAvailableMb, agentRssMb };
}

function getOrcaWorkers(runId) {
  try {
    const out = execSync(`${ORCA_BIN} orchestration worker-list --run ${runId}`, {
      cwd: PROJECT_ROOT,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'],
    });

    const lines = out.split('\n').filter((l) => l.startsWith('dispatchId='));
    const workers = [];

    for (const l of lines) {
      const parts = l.split(/\s+/);
      const w = {};
      for (const p of parts) {
        const [k, v] = p.split('=');
        if (k && v) w[k] = v;
      }
      workers.push(w);
    }
    return workers;
  } catch (err) {
    return [];
  }
}

function checkOrcaMessages(runId) {
  try {
    const out = execSync(`${ORCA_BIN} orchestration check --run ${runId}`, {
      cwd: PROJECT_ROOT,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'],
    });
    return out.trim();
  } catch {
    return '';
  }
}

export function showStatus() {
  const runId = getActiveRunId();
  const { memAvailableMb, agentRssMb } = getMemoryMetrics();
  const kanbanPath = findKanbanPath();
  const { tasks } = parseKanban(kanbanPath);
  const stats = getStats(tasks);
  const workers = getOrcaWorkers(runId);
  const liveWorkers = workers.filter((w) => w.liveness === 'live');
  const reclaimable = workers.filter((w) => w.terminalState === 'retained' && (w.state === 'succeeded' || w.state === 'stopped'));
  const changedFiles = getChangedFiles();

  console.log(`\n================================================================`);
  console.log(`🚀 AUTONOMOUS PIPELINE DASHBOARD (Run: ${runId})`);
  console.log(`================================================================`);
  console.log(`📊 Kanban Progress:  ${stats.done}/${stats.total} (${stats.percent}%) done | ${stats.inProgress} in progress | ${stats.queued} queued`);
  console.log(`🖥️  Workers:          ${liveWorkers.length} live / ${MAX_ACTIVE_WORKERS} max allowed (${reclaimable.length} reclaimable)`);
  console.log(`💾 RAM Status:       RSS: ${agentRssMb}MB / Limit: ${WORKER_RAM_LIMIT_MB}MB | MemAvailable: ${memAvailableMb}MB`);
  console.log(`📁 Git Working Tree: ${changedFiles.length} changed files`);

  if (liveWorkers.length > 0) {
    console.log(`\nActive Worker Dispatches:`);
    for (const w of liveWorkers) {
      console.log(`  • ${w.dispatchId} | Task: ${w.taskId} | State: ${w.state} | Term: ${w.terminal}`);
    }
  }

  if (changedFiles.length > 0) {
    console.log(`\nModified Files:`);
    for (const f of changedFiles) {
      console.log(`  • ${f}`);
    }
  }
  console.log(`================================================================\n`);
}

export function harvestSettledWorkers() {
  console.log(`\n🌾 [HARVEST] Scanning for settled workers and changes...`);
  const runId = getActiveRunId();
  const workers = getOrcaWorkers(runId);
  const kanbanPath = findKanbanPath();
  const { tasks } = parseKanban(kanbanPath);

  // Check reclaimable terminals
  for (const w of workers) {
    if (w.terminalState === 'retained' && (w.state === 'succeeded' || w.state === 'stopped' || w.state === 'failed')) {
      console.log(`♻️  Releasing settled terminal for dispatch ${w.dispatchId}...`);
      try {
        execSync(`${ORCA_BIN} orchestration worker-release --dispatch ${w.dispatchId}`, {
          cwd: PROJECT_ROOT,
          stdio: 'ignore',
        });
        console.log(`   Released dispatch ${w.dispatchId}`);
      } catch (e) {
        console.error(`   Failed to release ${w.dispatchId}:`, e.message);
      }
    }
  }

  // Check if there are changed files
  const changedFiles = getChangedFiles();
  if (changedFiles.length === 0) {
    console.log(`   No uncommitted file changes found.`);
    return { changesProcessed: 0 };
  }

  console.log(`📝 Detected changes in ${changedFiles.length} files:`, changedFiles);

  // Run targeted tests
  console.log(`🧪 Running targeted verification suite...`);
  const testRes = runTests(changedFiles);
  if (!testRes.success) {
    console.error(`❌ HARVEST BLOCKED: Targeted tests failed! Human or agent intervention required.`);
    return { error: 'TESTS_FAILED', failedCommand: testRes.failedCommand };
  }

  // Determine which task this commit belongs to based on active/changed file scope
  let matchedTask = null;
  for (const t of tasks) {
    if (t.status === 'IN_PROGRESS' || t.status === 'QUEUED') {
      const scopeBase = path.basename(t.scope);
      if (changedFiles.some((f) => f.includes(scopeBase) || (t.scope && f.includes(t.scope)))) {
        matchedTask = t;
        break;
      }
    }
  }

  // If no direct in-progress match, pick the first in-progress task
  if (!matchedTask) {
    matchedTask = tasks.find((t) => t.status === 'IN_PROGRESS');
  }

  const taskId = matchedTask ? matchedTask.taskId : 'RW51-AUTO';
  const workerName = matchedTask ? matchedTask.worker : 'auto';
  const scopeDesc = matchedTask ? matchedTask.scope.split('/').pop().replace(/\.[^.]+$/, '') : 'core';

  const commitMsg = `feat(${scopeDesc}): modular implementation for ${taskId}`;
  console.log(`📦 Committing changes with message: "${commitMsg}"...`);

  try {
    execSync('git add -A', { cwd: PROJECT_ROOT, stdio: 'inherit' });
    execSync(`git commit -m "${commitMsg}"`, { cwd: PROJECT_ROOT, stdio: 'inherit' });
    const commitHash = execSync('git rev-parse --short HEAD', { cwd: PROJECT_ROOT, encoding: 'utf8' }).trim();
    console.log(`✅ Commit created: ${commitHash}`);

    // Push to remotes if available
    try {
      console.log(`🚀 Pushing to remotes...`);
      execSync('git push origin HEAD && git push public HEAD', { cwd: PROJECT_ROOT, stdio: 'inherit' });
      console.log(`✅ Pushed to origin and public`);
    } catch (pushErr) {
      console.warn(`⚠️  Push failed (offline or remote unreachable):`, pushErr.message);
    }

    // Update kanban board
    if (matchedTask) {
      console.log(`📋 Updating Kanban board: ${matchedTask.taskId} -> DONE (${commitHash})`);
      updateTaskStatus(kanbanPath, matchedTask.taskId, 'done', commitHash);
    }

    return { changesProcessed: changedFiles.length, commitHash, taskId };
  } catch (err) {
    console.error(`❌ Commit failed:`, err.message);
    return { error: err.message };
  }
}

export function dispatchNextTask() {
  console.log(`\n🚀 [DISPATCH] Checking queue and slot capacity...`);
  const runId = getActiveRunId();
  const { memAvailableMb, agentRssMb } = getMemoryMetrics();

  // Guard RAM limit
  if (memAvailableMb < 8192) {
    console.warn(`⚠️  MemAvailable (${memAvailableMb}MB) < 8192MB. Skipping dispatch to prevent OOM.`);
    return { dispatched: false, reason: 'MEM_AVAILABLE_LOW' };
  }

  if (agentRssMb >= WORKER_RAM_LIMIT_MB) {
    console.warn(`⚠️  Agent RSS (${agentRssMb}MB) >= Limit (${WORKER_RAM_LIMIT_MB}MB). Skipping dispatch.`);
    return { dispatched: false, reason: 'AGENT_RSS_LIMIT' };
  }

  const workers = getOrcaWorkers(runId);
  const liveWorkers = workers.filter((w) => w.liveness === 'live');
  if (liveWorkers.length >= MAX_ACTIVE_WORKERS) {
    console.log(`⏳ Concurrency ceiling reached: ${liveWorkers.length}/${MAX_ACTIVE_WORKERS} workers active.`);
    return { dispatched: false, reason: 'CONCURRENCY_CEILING' };
  }

  const kanbanPath = findKanbanPath();
  const { tasks } = parseKanban(kanbanPath);
  const nextTask = tasks.find((t) => t.status === 'QUEUED');

  if (!nextTask) {
    console.log(`🎉 No queued tasks remaining in backlog!`);
    return { dispatched: false, reason: 'QUEUE_EMPTY' };
  }

  console.log(`🎯 Next task found: ${nextTask.taskId} (${nextTask.worker}) - ${nextTask.title || nextTask.scope}`);

  const sanitizedTaskText = (nextTask.annotation || nextTask.title || nextTask.scope).replace(/[`"'\\]/g, '');
  const bridgeCmd = [
    'bash',
    'scripts/ecc-orca-bridge.sh',
    '--session-name',
    `"${nextTask.worker}"`,
    '--model',
    `"${nextTask.modelName || 'gemini-4-argon-medium'}"`,
    '--effort',
    `"${nextTask.effort || 'medium'}"`,
    '--task',
    `"Implement ${nextTask.taskId}: ${sanitizedTaskText}. Strictly follow AGENTS.md conventions with English code and documentation."`,
  ].join(' ');

  console.log(`▶ Executing: ${bridgeCmd}`);
  try {
    execSync(bridgeCmd, { cwd: PROJECT_ROOT, stdio: 'inherit' });
    updateTaskStatus(kanbanPath, nextTask.taskId, 'in_progress', nextTask.worker);
    console.log(`🚀 Task ${nextTask.taskId} dispatched successfully!`);
    return { dispatched: true, task: nextTask };
  } catch (err) {
    console.error(`❌ Dispatch failed:`, err.message);
    return { dispatched: false, error: err.message };
  }
}

export function step() {
  checkOrcaMessages(getActiveRunId());
  harvestSettledWorkers();
  dispatchNextTask();
  showStatus();
}

export async function watch(intervalSec = 15) {
  console.log(`\n👀 Starting Autonomous Pipeline Watcher (interval: ${intervalSec}s)`);
  console.log(`   Press Ctrl+C to stop.`);

  let running = true;
  process.on('SIGINT', () => {
    console.log('\n🛑 Watcher stopped.');
    running = false;
    process.exit(0);
  });

  while (running) {
    try {
      step();
    } catch (err) {
      console.error(`[WATCHER ERROR]:`, err.message);
    }
    await new Promise((resolve) => setTimeout(resolve, intervalSec * 1000));
  }
}

// CLI handler
async function main() {
  const args = process.argv.slice(2);
  const flag = args[0] || '--status';

  switch (flag) {
    case '--status':
      showStatus();
      break;
    case '--harvest':
      harvestSettledWorkers();
      break;
    case '--dispatch':
      dispatchNextTask();
      break;
    case '--step':
      step();
      break;
    case '--watch': {
      const interval = parseInt(args[1] || '15', 10);
      await watch(interval);
      break;
    }
    default:
      console.log(`Usage: node scripts/auto-pipeline.mjs [--status|--step|--harvest|--dispatch|--watch [sec]]`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
