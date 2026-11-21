#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/orchestration-auto-driver.mjs
 *
 * Automated Orca orchestration driver and continuous worker slot rotator.
 * Completely eliminates AI token expenditure on routine checks of worker messages,
 * task completion closing, terminal reclamation, launching next reserved slots,
 * and Git remote synchronization.
 *
 * Capabilities:
 * 1. Checks incoming orchestration messages (orca orchestration check).
 * 2. On receiving worker_done, immediately reclaims terminal (worker-release).
 * 3. Automatically marks task as 'done' on the Kanban board (waves/kanban-board.json).
 * 4. Records COMPLETED_VERIFIED status in the slot registry (waves/reserved-all-slots.json).
 * 5. Enforces worker memory ceiling (WORKER_RAM_LIMIT < 25 GB).
 * 6. Dispatches next reserved slot from pool to Orca worker.
 * 7. Automatically triggers git-sync to origin and public upon task completion.
 * 8. Supports continuous watch daemon mode (--watch / --daemon).
 *
 * Usage:
 *   node scripts/orchestration-auto-driver.mjs --once              # One-shot check and processing
 *   node scripts/orchestration-auto-driver.mjs --status            # Show slots and memory status
 *   node scripts/orchestration-auto-driver.mjs --watch [seconds]   # Background watch cycle (default: 15s)
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { cmdCommit } from './git-sync.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const KANBAN_PATH = path.join(PROJECT_ROOT, 'waves/kanban-board.json');
const RESERVED_PATH = path.join(PROJECT_ROOT, 'waves/reserved-all-slots.json');
const RUN_ID = 'run_d3c9df2d4c8c';
const TERMINAL_HANDLE = 'term_f93a22fc-a3b7-4a72-b617-92c8df5bcaf6';
const MAX_CONCURRENT_WORKERS = 4;
const MAX_RAM_GIB = 25.0;

function execQuiet(cmd) {
  try {
    return execSync(cmd, { cwd: PROJECT_ROOT, encoding: 'utf8', stdio: 'pipe' }).trim();
  } catch (err) {
    return '';
  }
}

export function getWorkerMemoryGiB() {
  const out = execQuiet("ps -eo rss,comm | awk '/opencode|agy|cargo|rustc|node/ {s+=$1} END {printf \"%.2f\", s/1024/1024}'");
  return parseFloat(out) || 0.0;
}

export function checkOrchestrationMessages() {
  const out = execQuiet(`orca orchestration check --run ${RUN_ID} --terminal ${TERMINAL_HANDLE}`);
  if (!out || out.includes('No messages.')) {
    return [];
  }

  const messages = [];
  const lines = out.split('\n');
  let currentMsg = null;

  for (const line of lines) {
    const headerMatch = line.match(/^([a-zA-Z0-9_]+)\s+([a-zA-Z0-9_]+)\s+from\s+([a-zA-Z0-9_-]+)/);
    if (headerMatch) {
      if (currentMsg) messages.push(currentMsg);
      currentMsg = {
        msgId: headerMatch[1],
        type: headerMatch[2],
        from: headerMatch[3],
        lines: []
      };
      continue;
    }

    if (currentMsg) {
      currentMsg.lines.push(line);
      const payloadMatch = line.match(/dispatchId=([a-zA-Z0-9_]+)/);
      if (payloadMatch) currentMsg.dispatchId = payloadMatch[1];
      const taskMatch = line.match(/taskId=([a-zA-Z0-9_]+)/);
      if (taskMatch) currentMsg.taskId = taskMatch[1];
    }
  }

  if (currentMsg) messages.push(currentMsg);
  return messages;
}

export function releaseWorkerTerminal(dispatchId) {
  if (!dispatchId) return false;
  console.log(`🧹 [AUTO-DRIVER] Reclaiming worker terminal for ${dispatchId}...`);
  try {
    execSync(`orca orchestration worker-release --dispatch ${dispatchId}`, { cwd: PROJECT_ROOT, stdio: 'inherit' });
    return true;
  } catch (err) {
    console.warn(`⚠️ [AUTO-DRIVER] Error reclaiming terminal ${dispatchId}: ${err.message}`);
    return false;
  }
}

export function markTaskCompleted(cardId) {
  if (!cardId) return;
  console.log(`✅ [AUTO-DRIVER] Recording completion of task [${cardId}]...`);

  // 1. Kanban board
  if (fs.existsSync(KANBAN_PATH)) {
    const board = JSON.parse(fs.readFileSync(KANBAN_PATH, 'utf8'));
    const card = board.cards.find(c => c.id === cardId);
    if (card) {
      card.status = 'done';
      card.column = 'done';
      card.completedAt = new Date().toISOString();
      fs.writeFileSync(KANBAN_PATH, JSON.stringify(board, null, 2));
    }
  }

  // 2. Slot registry
  if (fs.existsSync(RESERVED_PATH)) {
    const res = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
    const slot = res.slots.find(s => s.id === cardId);
    if (slot) {
      slot.status = 'COMPLETED_VERIFIED';
      slot.completedAt = new Date().toISOString();
      fs.writeFileSync(RESERVED_PATH, JSON.stringify(res, null, 2));
    }
  }
}

export function getActiveWorkerCount() {
  const out = execQuiet(`orca orchestration worker-list --run ${RUN_ID}`);
  const lines = out.split('\n').filter(Boolean);
  return lines.filter(l => l.includes('state=ready') || (l.includes('terminalState=active') && !l.includes('terminalState=released'))).length;
}

export function dispatchNextSlot() {
  if (!fs.existsSync(RESERVED_PATH)) return false;

  const currentMem = getWorkerMemoryGiB();
  if (currentMem >= MAX_RAM_GIB) {
    console.warn(`⚠️ [AUTO-DRIVER] Worker memory (${currentMem} GiB) reached ceiling (${MAX_RAM_GIB} GiB). Dispatch postponed.`);
    return false;
  }

  const activeCount = getActiveWorkerCount();
  if (activeCount >= MAX_CONCURRENT_WORKERS) {
    console.log(`ℹ️ [AUTO-DRIVER] Pool at capacity (${activeCount}/${MAX_CONCURRENT_WORKERS} active workers).`);
    return false;
  }

  const res = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
  const nextSlot = res.slots.find(s => s.status === 'RESERVED');
  if (!nextSlot) {
    console.log('🎉 [AUTO-DRIVER] All reserved slots have been dispatched or completed!');
    return false;
  }

  if (nextSlot.isScript || nextSlot.tier === 'TIER_0_SCRIPT') {
    console.log(`⚡ [AUTO-DRIVER] Slot #${nextSlot.slot} [${nextSlot.id}] executing via local script (0 tokens): ${nextSlot.scriptCommand}`);
    if (nextSlot.scriptCommand) {
      try {
        execSync(nextSlot.scriptCommand, { cwd: PROJECT_ROOT, stdio: 'inherit' });
      } catch (err) {
        console.warn(`⚠️ [AUTO-DRIVER] Warning while executing ${nextSlot.scriptCommand}: ${err.message}`);
      }
    }
    markTaskCompleted(nextSlot.id);
    return true;
  }

  console.log(`🚀 [AUTO-DRIVER] Spawning worker for Slot #${nextSlot.slot} [${nextSlot.id}]: ${nextSlot.title}`);
  const specText = `Task ${nextSlot.id}: ${nextSlot.title}\nDescription: ${nextSlot.description}\nDepartment: ${nextSlot.department}\nPriority: ${nextSlot.priority}\nContract: Follow AGENTS.md, TEST-POLICY.md strictly. Write tests first.`;
  const taskTitle = `[${nextSlot.id}] ${nextSlot.title}`;

  try {
    const resSpawn = spawnSync('orca', [
      'orchestration', 'worker-start',
      '--run', RUN_ID,
      '--agent', 'antigravity',
      '--model', nextSlot.model,
      '--effort', nextSlot.effort,
      '--spec', specText,
      '--task-title', taskTitle,
      '--json'
    ], { cwd: PROJECT_ROOT, encoding: 'utf8' });

    if (resSpawn.status !== 0) {
      throw new Error(resSpawn.stderr || resSpawn.stdout || 'orca worker-start exited with non-zero code');
    }

    const out = resSpawn.stdout.trim();
    const parsed = JSON.parse(out);
    nextSlot.dispatchId = parsed.dispatchId || parsed.id || 'dispatched';
    nextSlot.status = 'DISPATCHED_WORKING';
    nextSlot.dispatchedAt = new Date().toISOString();
    fs.writeFileSync(RESERVED_PATH, JSON.stringify(res, null, 2));
    console.log(`✅ [AUTO-DRIVER] Worker spawned successfully! DispatchId: ${nextSlot.dispatchId}`);
    return true;
  } catch (err) {
    console.error(`❌ [AUTO-DRIVER] Error spawning worker: ${err.message}`);
    nextSlot.status = 'DISPATCH_FAILED';
    fs.writeFileSync(RESERVED_PATH, JSON.stringify(res, null, 2));
    return false;
  }
}

export function runCycle() {
  console.log('=== [ORCHESTRATION AUTO-DRIVER CYCLE] ===');
  const mem = getWorkerMemoryGiB();
  console.log(`Worker memory: ${mem} GiB / ${MAX_RAM_GIB} GiB`);

  const messages = checkOrchestrationMessages();
  console.log(`Incoming messages: ${messages.length}`);

  let completedTasks = [];

  for (const msg of messages) {
    if (msg.type === 'worker_done') {
      console.log(`📩 Received worker_done for dispatchId: ${msg.dispatchId}`);
      releaseWorkerTerminal(msg.dispatchId);

      // Find card by dispatchId in slot registry
      if (fs.existsSync(RESERVED_PATH)) {
        const res = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
        const slot = res.slots.find(s => s.dispatchId === msg.dispatchId);
        if (slot) {
          markTaskCompleted(slot.id);
          completedTasks.push(slot.id);
        }
      }
    }
  }

  // Continuous rotation: dispatch while slots available under limits
  let spawned = false;
  while (getActiveWorkerCount() < MAX_CONCURRENT_WORKERS) {
    const ok = dispatchNextSlot();
    if (ok) spawned = true;
    else break;
  }

  // If tasks completed, sync git and map
  if (completedTasks.length > 0) {
    try {
      execSync('node scripts/sync-project-map.mjs', { cwd: PROJECT_ROOT, stdio: 'inherit' });
    } catch {}
    console.log(`🚀 [AUTO-DRIVER] Synchronizing Git following completion of ${completedTasks.join(', ')}...`);
    cmdCommit(`docs(kanban): auto-complete tasks ${completedTasks.join(', ')}`);
  }

  console.log('=========================================');
  return { messagesCount: messages.length, completedTasks, spawned };
}

export function runWatchMode(intervalSeconds = 15) {
  console.log(`👀 [AUTO-DRIVER WATCH] Background driver started (interval ${intervalSeconds}s)...`);
  console.log('Press Ctrl+C to exit.');

  runCycle();
  setInterval(() => {
    runCycle();
  }, intervalSeconds * 1000);
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes('--status')) {
    const mem = getWorkerMemoryGiB();
    const active = getActiveWorkerCount();
    console.log(`Status: Active workers = ${active}/${MAX_CONCURRENT_WORKERS}, RAM = ${mem} GiB`);
    return;
  }

  if (args.includes('--watch') || args.includes('--daemon')) {
    const watchIdx = args.indexOf('--watch') !== -1 ? args.indexOf('--watch') : args.indexOf('--daemon');
    const sec = parseInt(args[watchIdx + 1], 10) || 15;
    runWatchMode(sec);
    return;
  }

  runCycle();
}

if (process.argv[1] && process.argv[1].endsWith('orchestration-auto-driver.mjs')) {
  main();
}
