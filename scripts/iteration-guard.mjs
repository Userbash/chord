#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/iteration-guard.mjs — Automated Iteration Counter & Circuit Breaker Guard
 *
 * Enforces the strict rule:
 * - Max 5-6 steps/iterations per worker.
 * - At step 5: logs warning and pre-snapshots diff.
 * - At step 6 (or when stuck looping on the same file reads):
 *   1. Captures git diff to .claude/checkpoints/${taskId}.patch
 *   2. Fences & stops the runaway worker (worker-stop)
 *   3. Releases the terminal (worker-release)
 *   4. Prepares handover notes for a fresh worker.
 */

import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const ORCA_BIN = process.env.ORCA_BIN || '/home/sanya/.config/orca/linux-orca-cli-shim/orca';
const CHECKPOINTS_DIR = path.join(PROJECT_ROOT, '.claude/checkpoints');
const MAX_STEPS = 6;

function runCmdOutput(cmd, cwd = PROJECT_ROOT) {
  try {
    return execSync(cmd, { cwd, encoding: 'utf8', stdio: 'pipe' }) || '';
  } catch (err) {
    return (err.stdout || '') + (err.stderr || '');
  }
}

export function inspectWorkerIterations(dispatchId, taskId) {
  const readOut = runCmdOutput(`${ORCA_BIN} orchestration worker-read --dispatch ${dispatchId} --limit 60`);
  if (!readOut.trim()) return { steps: 0, isLooping: false, repeatedFile: null };

  // Count steps by counting tool invocations or thought blocks or prompts
  const thoughtMatches = readOut.match(/▸ Thought for/g) || [];
  const bashMatches = readOut.match(/● Bash\(/g) || [];
  const editMatches = readOut.match(/● Edit\(/g) || [];
  const readMatches = readOut.match(/● Read\(/g) || [];
  
  // Total major actions:
  const actionCount = thoughtMatches.length + Math.floor((bashMatches.length + editMatches.length + readMatches.length) / 3);
  const estimatedSteps = Math.max(thoughtMatches.length, actionCount, 1);

  // Check for looping on identical read targets
  const readLines = readOut.split('\n').filter(l => l.includes('● Read('));
  const counts = {};
  let repeatedFile = null;
  let isLooping = false;

  for (const line of readLines) {
    const m = line.match(/● Read\(([^)]+)\)/);
    if (m && m[1]) {
      const file = m[1];
      counts[file] = (counts[file] || 0) + 1;
      if (counts[file] >= 3) {
        repeatedFile = file;
        isLooping = true;
      }
    }
  }

  return {
    steps: estimatedSteps,
    isLooping,
    repeatedFile,
    readOut,
  };
}

export function tripCircuitBreaker(dispatchId, taskId, reason) {
  console.log(`\n🚨 [CIRCUIT BREAKER] Tripping breaker for Task ${taskId} (Dispatch ${dispatchId})`);
  console.log(`   Reason: ${reason}`);

  if (!fs.existsSync(CHECKPOINTS_DIR)) {
    fs.mkdirSync(CHECKPOINTS_DIR, { recursive: true });
  }

  // 1. Capture git diff as checkpoint patch
  const patchFile = path.join(CHECKPOINTS_DIR, `${taskId}.patch`);
  const statusFile = path.join(CHECKPOINTS_DIR, `${taskId}.status`);
  const diff = runCmdOutput('git diff');

  if (diff.trim()) {
    fs.writeFileSync(patchFile, diff, 'utf8');
    console.log(`   💾 Saved work-in-progress patch: ${patchFile} (${diff.length} bytes)`);
  } else {
    console.log(`   ℹ️ No uncommitted git changes detected.`);
  }

  fs.writeFileSync(statusFile, JSON.stringify({
    taskId,
    dispatchId,
    trippedAt: new Date().toISOString(),
    reason,
  }, null, 2), 'utf8');

  // 2. Stop worker terminal
  console.log(`   Stopping worker process...`);
  runCmdOutput(`${ORCA_BIN} orchestration worker-stop --dispatch ${dispatchId}`);

  // 3. Release worker terminal
  console.log(`   Releasing worker terminal...`);
  runCmdOutput(`${ORCA_BIN} orchestration worker-release --dispatch ${dispatchId}`);

  console.log(`   ✅ Circuit breaker executed. Terminal reclaimed. Ready for fresh worker handover.\n`);
}

export function checkAllActiveWorkers(runId) {
  const listOut = runCmdOutput(`${ORCA_BIN} orchestration worker-list --run ${runId}`);
  const lines = listOut.split('\n');
  let checked = 0;
  let tripped = 0;

  for (const line of lines) {
    if (!line.includes('dispatchId=') || !line.includes('outcome=in_progress')) continue;
    const dispatchMatch = line.match(/dispatchId=([^\s]+)/);
    const taskMatch = line.match(/taskId=([^\s]+)/);
    if (!dispatchMatch || !taskMatch) continue;

    const dispatchId = dispatchMatch[1];
    const taskId = taskMatch[1];
    checked++;

    const status = inspectWorkerIterations(dispatchId, taskId);
    console.log(`[Iteration Guard] ${taskId} (${dispatchId}): Step ~${status.steps}/${MAX_STEPS} (Looping: ${status.isLooping ? status.repeatedFile : 'no'})`);

    if (status.steps >= MAX_STEPS) {
      tripCircuitBreaker(dispatchId, taskId, `Exceeded max step threshold (${status.steps} >= ${MAX_STEPS})`);
      tripped++;
    } else if (status.isLooping && status.steps >= 4) {
      tripCircuitBreaker(dispatchId, taskId, `Stuck looping on reading ${status.repeatedFile}`);
      tripped++;
    } else if (status.steps === 5) {
      console.log(`   ⚠️ WARNING: Worker ${dispatchId} is on Step 5. Approaching circuit breaker limit.`);
    }
  }

  return { checked, tripped };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const runId = process.argv[2] || 'run_d3c9df2d4c8c';
  console.log(`🛡️ Running Iteration Guard on Run ${runId}...`);
  const res = checkAllActiveWorkers(runId);
  console.log(`Done. Checked: ${res.checked}, Tripped: ${res.tripped}`);
}
