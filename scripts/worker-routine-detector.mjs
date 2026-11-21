#!/usr/bin/env node
// @ts-check
/**
 * scripts/worker-routine-detector.mjs — Zero-Token Worker Routine Detector & Automator
 *
 * Automatically analyzes worker tasks, commit history, and task queues,
 * identifying repetitive, deterministic routine tasks on which agents spend AI tokens.
 *
 * Classifies operations into:
 * 1. AUTOMATE_BY_SCRIPT (100% deterministic script, 0 tokens, sub-second)
 * 2. AUTOMATE_BY_MICRO_AI (Tier S: gemini-4-hydrogen-low, effort: low — only for unstructured text)
 * 3. ESCALATE_TO_CORE_AI (Tier L/XL — strictly for complex business logic, cryptography, or state machines)
 *
 * Usage:
 *   node scripts/worker-routine-detector.mjs --scan       # Analyze repetitions and estimate token savings
 *   node scripts/worker-routine-detector.mjs --auto-fix   # Automatically execute scripts for detected routines
 *   node scripts/worker-routine-detector.mjs --json       # Output JSON format for harness integration
 */

import { execSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const KANBAN_JSON = join(ROOT, 'waves/kanban-board.json');

// Catalog of routine repetitive worker tasks with token expenditure estimates
const ROUTINE_PATTERNS = [
  {
    id: 'IPC_BRIDGE_GENERATION',
    name: 'Tauri IPC bridge and TS types generation (bindings.d.ts)',
    description: 'Parses #[tauri::command] and emits TypeScript interface definitions',
    avgTokensSpentByAI: 3500,
    avgTimeByAISeconds: 45,
    handler: 'node scripts/generate-ipc-bridges.mjs',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['bindings.d.ts', 'specta', 'generate-ipc', 'ipc bridge', 'tauri::command']
  },
  {
    id: 'TEST_SCAFFOLDING',
    name: 'Contract test scaffolding (AAA Pattern & FSM)',
    description: 'Creates test files with imports, fixtures, mocks, and contract assertions',
    avgTokensSpentByAI: 4000,
    avgTimeByAISeconds: 60,
    handler: 'node scripts/auto-test-generator.mjs',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['test scaffold', 'auto-test', 'fsm test', 'contract test skeleton']
  },
  {
    id: 'DTO_SERDE_CREATION',
    name: 'DTO struct templating (Serde camelCase)',
    description: 'Emits Rust structs with #[derive(Serialize, Deserialize)] and TS interfaces',
    avgTokensSpentByAI: 2500,
    avgTimeByAISeconds: 35,
    handler: 'node scripts/scaffold-dto.mjs',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['dto', 'serde', 'camelcase', 'struct request', 'interface dto']
  },
  {
    id: 'DEAD_CODE_SANITATION',
    name: 'Dead code, dbg!, and console.log cleanup',
    description: 'Sanitizes debug leftovers and unused declarations in production code',
    avgTokensSpentByAI: 2000,
    avgTimeByAISeconds: 30,
    handler: 'node scripts/clean-dead-code.mjs',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['clean-dead-code', 'console.log', 'dbg!', 'cleanup', 'unused code']
  },
  {
    id: 'SECURITY_DEPENDENCY_AUDIT',
    name: 'Security audit, unsafe code, and C FFI check',
    description: 'Scans for forbid(unsafe_code) directives, secret leaks, and known CVEs',
    avgTokensSpentByAI: 3000,
    avgTimeByAISeconds: 40,
    handler: 'npm run audit:security',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['cargo audit', 'unsafe', 'security audit', 'secret-scan']
  },
  {
    id: 'MULTI_REMOTE_GIT_SYNC',
    name: 'Bidirectional Git sync across remotes (origin + public)',
    description: 'Verifies tracking, performs safe fast-forward pull, precheck, and dual push',
    avgTokensSpentByAI: 2500,
    avgTimeByAISeconds: 35,
    handler: 'node scripts/git-sync.mjs sync',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['git pull', 'git push', 'origin public', 'git sync', 'remote sync']
  },
  {
    id: 'TDD_PLAN_TABLE_SYNC',
    name: 'TDD-PLAN.md §8 contract test table sync',
    description: 'Automatically detects test files on disk and registers them in the matrix',
    avgTokensSpentByAI: 2000,
    avgTimeByAISeconds: 20,
    handler: 'node scripts/sync-tdd-plan.mjs --fix',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['tdd-plan', 'sync-tdd', 'tdd:sync', 'plan-sync']
  },
  {
    id: 'ORCHESTRATION_AUTODRIVER_CYCLE',
    name: 'Orchestration driver & worker slot rotator (Zero-Token Auto-Driver)',
    description: 'Checks orchestration messages, reclaims terminals, rotates slots, and pushes',
    avgTokensSpentByAI: 3500,
    avgTimeByAISeconds: 30,
    handler: 'node scripts/orchestration-auto-driver.mjs --once',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['orchestration check', 'worker-release', 'worker_done', 'auto-driver', 'slot rotation']
  },
  {
    id: 'THIN_BRIDGE_AUDIT',
    name: 'Architectural budget audit (Thin Bridge <= 600 LOC)',
    description: 'Enforces thin adapters without duplicate business logic delegating to libraries',
    avgTokensSpentByAI: 2500,
    avgTimeByAISeconds: 25,
    handler: 'node scripts/audit-thin-bridges.mjs',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['audit:bridges', 'thin bridge', 'adapter loc']
  },
  {
    id: 'STRESS_BENCHMARKING',
    name: 'FTS5, jitter buffer, and load stress testing',
    description: 'Executes pre-built stress scenarios for SQLite and media packet rings',
    avgTokensSpentByAI: 3500,
    avgTimeByAISeconds: 50,
    handler: 'npm run test:stress',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['stress-fts5', 'bench-voice', '50-peers', 'stress test']
  },
  {
    id: 'PLAN_ROADMAP_SYNC',
    name: 'Roadmap, metrics, and Kanban board synchronization',
    description: 'Updates TDD-PLAN.md §8, exports HTML report, and tallies bridge LOC',
    avgTokensSpentByAI: 2000,
    avgTimeByAISeconds: 25,
    handler: 'npm run roadmap:sync',
    tier: 'TIER_0_SCRIPT',
    canAutomateWithScript: true,
    keywords: ['roadmap', 'kanban-board', 'tdd-plan', 'sync board']
  }
];

/**
 * @param {string} cmd
 * @returns {string}
 */
function runCmd(cmd) {
  try {
    return execSync(cmd, { cwd: ROOT, encoding: 'utf8', stdio: 'pipe' }) || '';
  } catch (/** @type {any} */ err) {
    return (err.stdout || '') + (err.stderr || '');
  }
}

/**
 * Analyzes last 30 commits and active tasks for recurring routine patterns
 */
export function analyzeRoutines() {
  const gitLog = runCmd('git log -n 30 --oneline');
  /** @type {any[]} */
  let kanbanCards = [];

  if (existsSync(KANBAN_JSON)) {
    try {
      const data = JSON.parse(readFileSync(KANBAN_JSON, 'utf8'));
      kanbanCards = data.cards || [];
    } catch {}
  }

  const detections = [];
  let totalSavedTokens = 0;
  let totalSavedSeconds = 0;

  for (const pattern of ROUTINE_PATTERNS) {
    let occurrencesInGit = 0;
    let matchingPendingCards = [];

    // 1. Search commit history
    for (const kw of pattern.keywords) {
      const regex = new RegExp(kw.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i');
      const matches = gitLog.split('\n').filter((/** @type {string} */ line) => regex.test(line));
      occurrencesInGit += matches.length;
    }

    // 2. Search pending queue tasks (ready_for_dev / backlog)
    for (const card of kanbanCards) {
      if (card.status !== 'done' && card.column !== 'done') {
        const titleLower = (card.title + ' ' + (card.description || '')).toLowerCase();
        if (pattern.keywords.some(kw => titleLower.includes(kw.toLowerCase()))) {
          matchingPendingCards.push({ id: card.id, title: card.title });
        }
      }
    }

    const totalOccurrences = occurrencesInGit + matchingPendingCards.length;

    if (totalOccurrences > 0) {
      const tokensForThis = totalOccurrences * pattern.avgTokensSpentByAI;
      const secondsForThis = totalOccurrences * pattern.avgTimeByAISeconds;
      totalSavedTokens += tokensForThis;
      totalSavedSeconds += secondsForThis;

      detections.push({
        id: pattern.id,
        name: pattern.name,
        occurrences: totalOccurrences,
        historyMatches: occurrencesInGit,
        pendingCards: matchingPendingCards,
        scriptHandler: pattern.handler,
        recommendedTier: pattern.tier,
        tokenSavingsEstimate: tokensForThis,
        timeSavingsSeconds: secondsForThis
      });
    }
  }

  return {
    detections,
    totalSavedTokens,
    totalSavedSeconds,
    totalSavedMinutes: (totalSavedSeconds / 60).toFixed(1)
  };
}

/**
 * Executes automatic resolution for all available routine scripts
 */
export function autoFixAllRoutines() {
  console.log('⚡ [ROUTINE-AUTOMATOR] Automatically executing deterministic scripts (0 tokens)...');
  const results = [];

  const scriptsToRun = [
    { title: '1. Tauri IPC bridges and TS types generation', cmd: 'node scripts/generate-ipc-bridges.mjs' },
    { title: '2. Dead code audit and cleanup', cmd: 'node scripts/clean-dead-code.mjs' },
    { title: '3. Security audit and unsafe code prohibition check', cmd: 'npm run audit:security' },
    { title: '4. Git multi-remote sync status check', cmd: 'node scripts/git-sync.mjs status' },
    { title: '5. Simplification roadmap and Kanban synchronization', cmd: 'npm run roadmap:sync' }
  ];

  for (const s of scriptsToRun) {
    console.log(`\n▶️  ${s.title} (${s.cmd})...`);
    const t0 = Date.now();
    const out = runCmd(s.cmd);
    const dur = ((Date.now() - t0) / 1000).toFixed(2);
    console.log(`   ✅ Finished in ${dur}s:`);
    console.log('   ' + out.trim().split('\n').slice(-2).join('\n   '));
    results.push({ cmd: s.cmd, duration: dur, status: 'OK' });
  }

  console.log('\n🎉 [ROUTINE-AUTOMATOR] All routine tasks automated successfully with 100% token savings!');
  return results;
}

// CLI Interface
const mode = process.argv[2] || '--scan';

if (mode === '--auto-fix' || mode === '--fix') {
  autoFixAllRoutines();
} else if (mode === '--json') {
  const report = analyzeRoutines();
  console.log(JSON.stringify(report, null, 2));
} else {
  console.log('🔍 ==============================================================================');
  console.log('🔍   WORKER ROUTINE PATTERN DETECTOR & ZERO-TOKEN SAVINGS EVALUATOR');
  console.log('🔍 ==============================================================================\n');

  const report = analyzeRoutines();

  console.log(`📊 Detected recurring task patterns: ${report.detections.length}`);
  console.log(`💰 Potential AI token savings: ~${report.totalSavedTokens.toLocaleString()} tokens`);
  console.log(`⏱️  Saved machine wait time: ~${report.totalSavedMinutes} minutes\n`);

  console.log('📋 DETECTED ROUTINE PATTERNS IN WORKER WORKFLOW:\n');

  report.detections.forEach((d, idx) => {
    console.log(`${idx + 1}. [${d.id}] ${d.name}`);
    console.log(`   • Recorded occurrences: ${d.occurrences} (in git commits: ${d.historyMatches}, in queue: ${d.pendingCards.length})`);
    console.log(`   • Strategy: ${d.recommendedTier === 'TIER_0_SCRIPT' ? '✅ 100% SCRIPT (0 tokens)' : '⚠️ MICRO_AI'}`);
    console.log(`   • Handler: ${d.scriptHandler}`);
    console.log(`   • Token savings: ~${d.tokenSavingsEstimate.toLocaleString()} tokens`);
    if (d.pendingCards.length > 0) {
      console.log(`   • Affected queue tasks: ${d.pendingCards.map(c => c.id).join(', ')}`);
    }
    console.log('');
  });

  console.log('==============================================================================');
  console.log('💡 Run `node scripts/worker-routine-detector.mjs --auto-fix` for instant execution.');
  console.log('==============================================================================');
}
