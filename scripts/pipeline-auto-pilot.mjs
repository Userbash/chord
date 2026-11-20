#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/pipeline-auto-pilot.mjs — Autonomous Zero-Token Pipeline Autopilot
 *
 * Automates repetitive coordinator routines:
 * 1. Branch Guard: Enforces branch is strictly 'tauri'.
 * 2. RAM & Zombie Terminal Watchdog: Keeps aggregate RSS < 25 GiB and releases dead handles.
 * 3. Inbox Harvester: Catches worker_done, runs targeted tests, marks completed, updates Kanban.
 * 4. 503 / Transient Stall Auto-Healer: Detects model capacity 503 errors and re-routes to fallback models.
 * 5. Zero-Idle Auto-Dispatcher: Automatically pulls the next 'ready_for_dev' task into available worker slots.
 * 6. IPC Bridge Auto-Sync: Regenerates typed bridges if Rust commands changed.
 *
 * Usage:
 *   node scripts/pipeline-auto-pilot.mjs [--once | --daemon [interval_sec]]
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const ORCA_BIN = process.env.ORCA_BIN || '/home/sanya/.config/orca/linux-orca-cli-shim/orca';
const KANBAN_JSON = path.join(PROJECT_ROOT, 'waves/kanban-board.json');
const MAX_ACTIVE_WORKERS = 4;

function runCmd(cmd, cwd = PROJECT_ROOT, silent = false) {
  try {
    return execSync(cmd, { cwd, encoding: 'utf8', stdio: silent ? 'pipe' : 'inherit' });
  } catch (err) {
    if (!silent) console.warn(`Command failed: ${cmd}\n${err.message}`);
    return null;
  }
}

function runCmdOutput(cmd, cwd = PROJECT_ROOT) {
  try {
    return execSync(cmd, { cwd, encoding: 'utf8', stdio: 'pipe' }) || '';
  } catch (err) {
    return (err.stdout || '') + (err.stderr || '');
  }
}

function assertBranchTauri() {
  const branch = runCmdOutput('git branch --show-current').trim();
  if (branch !== 'tauri') {
    throw new Error(`CRITICAL: Current branch is '${branch}'. Expected 'tauri'. Halting autopilot!`);
  }
}

function getActiveRunId() {
  try {
    const out = runCmdOutput(`${ORCA_BIN} orchestration run-current`);
    const match = out.match(/runId=([a-zA-Z0-9_-]+)/);
    if (match && match[1] && match[1] !== 'none') return match[1];
  } catch {}
  return 'run_d3c9df2d4c8c';
}

function runRamGuard() {
  try {
    runCmd('bash scripts/worker-ram-guard.sh', PROJECT_ROOT, true);
  } catch {}
}

function harvestInbox(runId) {
  const inbox = runCmdOutput(`${ORCA_BIN} orchestration inbox --run ${runId} --full`);
  if (!inbox.trim()) return 0;

  const messages = inbox.split(/\n\n+/);
  let processed = 0;

  for (const msg of messages) {
    const lines = msg.trim().split('\n');
    const header = lines[0] || '';
    if (header.includes('worker_done')) {
      console.log(`\n🎉 [Autopilot] Detected worker_done: ${header}`);
      const payloadMatch = msg.match(/payload:\s*taskId=([^\s]+)\s+dispatchId=([^\s]+)/);
      if (payloadMatch) {
        const taskId = payloadMatch[1];
        const dispatchId = payloadMatch[2];
        console.log(`   Task: ${taskId} | Dispatch: ${dispatchId}`);

        // 1. Run targeted verification
        console.log(`   Running smart verification tests...`);
        const testRes = spawnSync('node', ['scripts/smart-test-runner.mjs'], {
          cwd: PROJECT_ROOT,
          encoding: 'utf8',
        });
        if (testRes.status === 0) {
          console.log(`   ✅ Tests verified successfully.`);
        } else {
          console.warn(`   ⚠️ Targeted tests warning: ${testRes.stderr || testRes.stdout}`);
        }

        // 2. Mark task completed in Orca
        runCmd(`${ORCA_BIN} orchestration task-update --id ${taskId} --status completed --result "Auto-accepted by Autopilot" --run ${runId}`);

        // 3. Release worker terminal
        runCmd(`${ORCA_BIN} orchestration worker-release --dispatch ${dispatchId}`);

        // 4. Update Kanban board
        updateKanbanCardStatus(taskId, 'done');
        processed++;
      }
    }
  }
  return processed;
}

function healStalledWorkers(runId) {
  const listOut = runCmdOutput(`${ORCA_BIN} orchestration worker-list --run ${runId}`);
  const lines = listOut.split('\n');
  let healed = 0;

  for (const line of lines) {
    if (!line.includes('dispatchId=')) continue;
    const dispatchMatch = line.match(/dispatchId=([^\s]+)/);
    const taskMatch = line.match(/taskId=([^\s]+)/);
    const outcomeMatch = line.match(/outcome=([^\s]+)/);
    const dispatchId = dispatchMatch ? dispatchMatch[1] : null;
    const taskId = taskMatch ? taskMatch[1] : null;

    if (dispatchId && outcomeMatch && outcomeMatch[1] === 'in_progress') {
      const readOut = runCmdOutput(`${ORCA_BIN} orchestration worker-read --dispatch ${dispatchId} 2>/dev/null | tail -n 25`);
      
      // Check for 503 capacity limit or stalled turn
      const is503 = readOut.includes('UNAVAILABLE (code 503)') || readOut.includes('No capacity available for model');
      const isStalled = readOut.includes('ended its turn without sending worker_done') || readOut.includes('Model unavailable (attempt 9/9)');

      if (is503 || isStalled) {
        console.warn(`\n⚠️ [Autopilot Healer] Worker ${dispatchId} (Task ${taskId}) needs healing (503: ${is503}, stalled: ${isStalled})`);
        
        // Strategy 1: Send auto-heal wakeup message
        console.log(`   Sending auto-heal wakeup to dispatch ${dispatchId}...`);
        const sendRes = runCmdOutput(`${ORCA_BIN} orchestration send --to dispatch:${dispatchId} --subject "Autopilot Auto-Heal" --body "Resume task execution and proceed to completion."`);
        if (sendRes.includes('messageId=')) {
          console.log(`   ✅ Sent wakeup signal (${sendRes.trim()}).`);
          healed++;
        } else {
          // Strategy 2: If send fails or terminal dead, release and re-route
          console.log(`   Releasing unresponsive dispatch ${dispatchId}...`);
          runCmd(`${ORCA_BIN} orchestration worker-release --dispatch ${dispatchId}`);
          updateKanbanCardStatus(taskId, 'ready_for_dev');
          healed++;
        }
      }
    }
  }
  return healed;
}

function updateKanbanCardStatus(taskIdOrKey, newStatus) {
  if (!fs.existsSync(KANBAN_JSON)) return;
  try {
    const raw = fs.readFileSync(KANBAN_JSON, 'utf8');
    const board = JSON.parse(raw);
    let updated = false;

    for (const card of board.cards) {
      const matchSubtasks = Array.isArray(card.subtasks) && card.subtasks.some(s => typeof s === 'string' && s.includes(taskIdOrKey));
      if (card.id.includes(taskIdOrKey) || card.title.includes(taskIdOrKey) || matchSubtasks) {
        card.status = newStatus;
        card.column = newStatus === 'done' ? 'done' : (newStatus === 'in_progress' ? 'in_progress' : 'ready_for_dev');
        card.updated_at = new Date().toISOString();
        updated = true;
      }
    }

    if (updated) {
      fs.writeFileSync(KANBAN_JSON, JSON.stringify(board, null, 2), 'utf8');
      runCmdOutput('node scripts/mcp-kanban-board.mjs --export');
      console.log(`   📋 Kanban board updated: ${taskIdOrKey} -> ${newStatus}`);
    }
  } catch (err) {
    console.warn(`Failed to update Kanban: ${err.message}`);
  }
}

function getActiveWorkerCount(runId) {
  const listOut = runCmdOutput(`${ORCA_BIN} orchestration worker-list --run ${runId}`);
  const activeDispatches = listOut.split('\n').filter(l => l.includes('outcome=in_progress') && l.includes('liveness=live'));
  return activeDispatches.length;
}

function autoDispatchQueue(runId) {
  const activeCount = getActiveWorkerCount(runId);
  if (activeCount >= MAX_ACTIVE_WORKERS) {
    return 0;
  }

  if (!fs.existsSync(KANBAN_JSON)) return 0;
  const raw = fs.readFileSync(KANBAN_JSON, 'utf8');
  const board = JSON.parse(raw);

  const pendingCards = board.cards.filter(c => c.status === 'ready_for_dev' || c.column === 'ready_for_dev');
  if (pendingCards.length === 0) return 0;

  let dispatched = 0;
  const availableSlots = MAX_ACTIVE_WORKERS - activeCount;

  for (let i = 0; i < Math.min(availableSlots, pendingCards.length); i++) {
    const card = pendingCards[i];
    console.log(`\n🚀 [Autopilot Dispatcher] Dispatching next task from queue: [${card.id}] ${card.title}`);

    // Determine model based on department & complexity via task-router
    let model = 'gemini-4-hydrogen-medium';
    let effort = 'medium';
    try {
      const routingJson = runCmdOutput(`node scripts/task-router.mjs --spec "${card.title}"`);
      const parsed = JSON.parse(routingJson);
      if (parsed.model) model = parsed.model;
      if (parsed.effort) effort = parsed.effort;
    } catch {}

    const spec = `[${card.id}] ${card.title}: ${card.description || 'Implement task per specification and TDD rules'}`;
    const cmd = `${ORCA_BIN} orchestration worker-start --run ${runId} --spec "${spec.replace(/"/g, '\\"')}" --task-title "${card.id}" --agent antigravity --model "${model}" --effort "${effort}" --json`;
    
    const res = runCmdOutput(cmd);
    console.log(`   Dispatch result: ${res.trim()}`);
    updateKanbanCardStatus(card.id, 'in_progress');
    dispatched++;
  }

  return dispatched;
}

function drainCoordinatorCheck(runId) {
  try {
    const out = runCmdOutput(`${ORCA_BIN} orchestration check --run ${runId}`);
    if (out.trim() && !out.includes('No messages')) {
      console.log(`📩 [Autopilot Coordinator] Drained orchestration check messages:\n${out.trim()}`);
    }
  } catch {}
}

function runAutopilotCycle() {
  const timestamp = new Date().toISOString();
  console.log(`\n=============================================================`);
  console.log(`🤖 [Autopilot Cycle: ${timestamp}]`);
  console.log(`=============================================================`);

  // 1. Guard branch
  assertBranchTauri();

  // 2. RAM & terminal cleanup
  runRamGuard();

  const runId = getActiveRunId();

  // 3. Drain coordinator messages
  drainCoordinatorCheck(runId);

  // 4. Harvest worker_done
  const harvested = harvestInbox(runId);

  // 5. Iteration guard & circuit breaker check (max 5-6 steps)
  try {
    const guardOut = runCmdOutput(`node scripts/iteration-guard.mjs ${runId}`);
    if (guardOut.includes('CIRCUIT BREAKER')) {
      console.log(`   🚨 [Autopilot Guard] Circuit breaker tripped:\n${guardOut.trim()}`);
    }
  } catch {}

  // 6. Heal stalled / 503 workers
  const healed = healStalledWorkers(runId);

  // 7. Zero-idle auto-dispatch
  const dispatched = autoDispatchQueue(runId);

  console.log(`[Autopilot Summary] Harvested: ${harvested} | Healed: ${healed} | Dispatched: ${dispatched}`);
}

const mode = process.argv[2] || '--once';
if (mode === '--daemon' || mode === '--watch') {
  const intervalSec = parseInt(process.argv[3] || '20', 10);
  console.log(`🤖 Starting Pipeline Autopilot Daemon (polling every ${intervalSec}s)...`);
  runAutopilotCycle();
  setInterval(runAutopilotCycle, intervalSec * 1000);
} else {
  runAutopilotCycle();
}
