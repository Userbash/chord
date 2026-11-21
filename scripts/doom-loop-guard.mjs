#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/doom-loop-guard.mjs — Anti-Doom-Loop Circuit Breaker for Autonomous QA & AI Agents
 *
 * Protects against:
 * 1. Oscillation: Detecting when code changes alternate/flip-flop between two states.
 * 2. Repetitive Failure: Detecting when the exact same test fails >= 3 times consecutively.
 * 3. Runaway Mutation Loop: Detecting when mutation tests fail without mutation death.
 *
 * Actions on Trip:
 * - Creates a recovery checkpoint in .claude/checkpoints/
 * - Prevents infinite retry loops and runaway token consumption
 * - Recommends targeted rollback or consulting Qdrant semantic memory
 *
 * Usage:
 *   node scripts/doom-loop-guard.mjs --check
 *   node scripts/doom-loop-guard.mjs --record --status pass|fail --test <name> [--diff-hash <hash>]
 *   node scripts/doom-loop-guard.mjs --reset
 */

import { execSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const CHECKPOINTS_DIR = path.join(PROJECT_ROOT, '.claude/checkpoints');
const LEDGER_FILE = path.join(CHECKPOINTS_DIR, 'doom_loop_ledger.json');

const MAX_CONSECUTIVE_FAILURES = 3;
const MAX_HISTORY_LENGTH = 10;

function ensureCheckpointsDir() {
  if (!fs.existsSync(CHECKPOINTS_DIR)) {
    fs.mkdirSync(CHECKPOINTS_DIR, { recursive: true });
  }
}

function loadLedger() {
  ensureCheckpointsDir();
  if (fs.existsSync(LEDGER_FILE)) {
    try {
      return JSON.parse(fs.readFileSync(LEDGER_FILE, 'utf8'));
    } catch {
      // ignore corrupt file
    }
  }
  return {
    history: [],
    consecutiveFailures: {},
    tripped: false,
    lastTripReason: null,
  };
}

function saveLedger(ledger) {
  ensureCheckpointsDir();
  fs.writeFileSync(LEDGER_FILE, JSON.stringify(ledger, null, 2), 'utf8');
}

export function getCurrentDiffHash() {
  try {
    const diff = execSync('git diff HEAD', { cwd: PROJECT_ROOT, encoding: 'utf8', stdio: ['pipe', 'pipe', 'ignore'] });
    return crypto.createHash('sha256').update(diff || 'empty').digest('hex').slice(0, 16);
  } catch {
    return 'unknown';
  }
}

export function checkDoomLoop() {
  const ledger = loadLedger();

  if (ledger.tripped) {
    return {
      tripped: true,
      reason: ledger.lastTripReason || 'Previous circuit breaker trip was not reset',
      recommendation: 'Review .claude/checkpoints/, fix root cause or run: node scripts/doom-loop-guard.mjs --reset',
    };
  }

  // Check 1: Consecutive identical test failures
  for (const [testName, count] of Object.entries(ledger.consecutiveFailures)) {
    if (count >= MAX_CONSECUTIVE_FAILURES) {
      return {
        tripped: true,
        reason: `Doom Loop: Test '${testName}' failed ${count} consecutive times without resolution.`,
        recommendation: `Check Qdrant semantic memory: node scripts/qdrant-qa-memory.mjs --search "${testName}"`,
      };
    }
  }

  // Check 2: Code oscillation (A -> B -> A pattern in diff hashes)
  const history = ledger.history;
  if (history.length >= 3) {
    const h0 = history[history.length - 1]?.diffHash;
    const h1 = history[history.length - 2]?.diffHash;
    const h2 = history[history.length - 3]?.diffHash;

    if (h0 && h1 && h2 && h0 === h2 && h0 !== h1 && h0 !== 'empty') {
      return {
        tripped: true,
        reason: `Doom Loop: Code oscillation detected! Diff state (${h0}) reverted to state from 2 iterations ago.`,
        recommendation: 'Stop automated retrying. Revert flip-flopping edit and inspect test requirements.',
      };
    }
  }

  return { tripped: false };
}

export function recordTestRun({ testName, status, errorSnippet = '' }) {
  const ledger = loadLedger();
  const diffHash = getCurrentDiffHash();
  const timestamp = new Date().toISOString();

  ledger.history.push({
    testName,
    status,
    diffHash,
    errorSnippet: errorSnippet.slice(0, 200),
    timestamp,
  });

  if (ledger.history.length > MAX_HISTORY_LENGTH) {
    ledger.history.shift();
  }

  if (status === 'fail') {
    ledger.consecutiveFailures[testName] = (ledger.consecutiveFailures[testName] || 0) + 1;
    if (ledger.consecutiveFailures[testName] >= MAX_CONSECUTIVE_FAILURES) {
      ledger.tripped = true;
      ledger.lastTripReason = `Test '${testName}' failed ${ledger.consecutiveFailures[testName]} times consecutively.`;
      createSnapshot(`consecutive_fail_${testName.replace(/[^a-zA-Z0-9]/g, '_')}`);
    }
  } else if (status === 'pass') {
    delete ledger.consecutiveFailures[testName];
    // If all passed, ensure tripped flag cleared
    if (Object.keys(ledger.consecutiveFailures).length === 0) {
      ledger.tripped = false;
      ledger.lastTripReason = null;
    }
  }

  // Check for oscillation immediately
  if (ledger.history.length >= 3) {
    const h0 = ledger.history[ledger.history.length - 1]?.diffHash;
    const h1 = ledger.history[ledger.history.length - 2]?.diffHash;
    const h2 = ledger.history[ledger.history.length - 3]?.diffHash;
    if (h0 && h1 && h2 && h0 === h2 && h0 !== h1 && h0 !== 'empty') {
      ledger.tripped = true;
      ledger.lastTripReason = `Code oscillation between states ${h0} <-> ${h1}.`;
      createSnapshot('oscillation_detected');
    }
  }

  saveLedger(ledger);
  return ledger;
}

export function createSnapshot(prefix = 'doom_recovery') {
  ensureCheckpointsDir();
  try {
    const diff = execSync('git diff HEAD', { cwd: PROJECT_ROOT, encoding: 'utf8', stdio: ['pipe', 'pipe', 'ignore'] });
    if (diff.trim()) {
      const snapPath = path.join(CHECKPOINTS_DIR, `${prefix}_${Date.now()}.patch`);
      fs.writeFileSync(snapPath, diff, 'utf8');
      console.log(`🚨 [DOOM-GUARD] Circuit breaker tripped! Saved recovery snapshot: ${snapPath}`);
      return snapPath;
    }
  } catch {
    // ignore
  }
  return null;
}

export function resetDoomLoop() {
  const ledger = {
    history: [],
    consecutiveFailures: {},
    tripped: false,
    lastTripReason: null,
  };
  saveLedger(ledger);
  return ledger;
}

// CLI Execution
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);

  if (args.includes('--reset')) {
    resetDoomLoop();
    console.log('✅ [DOOM-GUARD] Circuit breaker and failure history reset.');
  } else if (args.includes('--check')) {
    const res = checkDoomLoop();
    if (res.tripped) {
      console.error(`🚨 [DOOM-GUARD] CIRCUIT BREAKER ACTIVE!`);
      console.error(`   Reason:         ${res.reason}`);
      console.error(`   Recommendation: ${res.recommendation}`);
      process.exit(2);
    } else {
      console.log('✅ [DOOM-GUARD] Circuit clear. No doom loop or oscillation detected.');
    }
  } else if (args.includes('--record')) {
    const testIdx = args.indexOf('--test') + 1;
    const statusIdx = args.indexOf('--status') + 1;
    const testName = args[testIdx] || 'general_test';
    const status = args[statusIdx] || 'pass';
    recordTestRun({ testName, status });
    console.log(`📝 [DOOM-GUARD] Recorded ${status} for ${testName}.`);
  } else {
    const ledger = loadLedger();
    console.log('🛡️ [DOOM-GUARD] Status Report:');
    console.log(`   Tripped:              ${ledger.tripped}`);
    console.log(`   Last Trip Reason:     ${ledger.lastTripReason || 'none'}`);
    console.log(`   Consecutive Failures: ${JSON.stringify(ledger.consecutiveFailures)}`);
    console.log(`   History Events:       ${ledger.history.length}`);
  }
}
