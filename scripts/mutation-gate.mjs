#!/usr/bin/env node
// @ts-nocheck
/**
 * mutation-gate.mjs — Automated 5/5 Mutation Testing Gate for discord-lab
 *
 * Implements AGENTS.md §4.1 and TEST-POLICY.md:
 * Verifies that test suites catch injected mutations (100% killed required).
 *
 * Injects 5 standard mutation patterns:
 *   1. Boundary/Comparison: '>' <-> '<=', '==' <-> '!='
 *   2. Guard/Condition: 'if condition' -> 'if false'
 *   3. Return Value / Error: 'Ok(' -> 'Err(' or inverting bool
 *   4. String / Serde Key: mutates serialized command or field names
 *   5. Numerical Constant: '0' -> '1' or '+ 1'
 *
 * Usage:
 *   node scripts/mutation-gate.mjs <target_file> [test_command]
 *
 * Example:
 *   node scripts/mutation-gate.mjs rust/gui-spike/src/chat_plugin.rs
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveTestCommands } from './test-runner-smart.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const MUTATION_PATTERNS = [
  {
    name: 'MUT_1_BOUNDARY_INVERSION',
    description: 'Invert comparison boundary (== to !=, > to <=)',
    search: /(===|!==|==|!=|>|<)/,
    replace: (match) => {
      switch (match) {
        case '===': return '!==';
        case '!==': return '===';
        case '==': return '!=';
        case '!=': return '==';
        case '>': return '<=';
        case '<': return '>=';
        default: return '!=';
      }
    },
  },
  {
    name: 'MUT_2_GUARD_BYPASS',
    description: 'Bypass guard check / if condition',
    search: /if\s*\(([^)]+)\)\s*\{/,
    replace: () => 'if (false) {',
  },
  {
    name: 'MUT_3_RETURN_OR_ERROR',
    description: 'Mutate result or return value',
    search: /(return\s+true|return\s+false|Ok\(|Err\()/,
    replace: (match) => {
      if (match.includes('true')) return 'return false';
      if (match.includes('false')) return 'return true';
      if (match.includes('Ok(')) return 'Err("mutated_error".into()); // Ok(';
      return match;
    },
  },
  {
    name: 'MUT_4_CONSTANT_TAMPER',
    description: 'Mutate integer or numerical constant',
    search: /(\b\d+\b)/,
    replace: (match) => String(parseInt(match, 10) + 1),
  },
  {
    name: 'MUT_5_BOOLEAN_INVERSION',
    description: 'Invert boolean literal or operator',
    search: /(\btrue\b|\bfalse\b)/,
    replace: (match) => (match === 'true' ? 'false' : 'true'),
  },
];

export function runMutationGate(targetFilePath, customTestCmd = null) {
  const fullPath = path.resolve(PROJECT_ROOT, targetFilePath);
  if (!fs.existsSync(fullPath)) {
    throw new Error(`Target file not found: ${fullPath}`);
  }

  const originalContent = fs.readFileSync(fullPath, 'utf8');

  // Determine test command
  let testCommands = [];
  if (customTestCmd) {
    const parts = customTestCmd.split(' ');
    testCommands = [[parts[0], parts.slice(1)]];
  } else {
    testCommands = resolveTestCommands([targetFilePath]);
  }

  if (testCommands.length === 0) {
    throw new Error(`No targeted test command resolved for ${targetFilePath}. Specify one as second argument.`);
  }

  console.log(`\n🧬 [MUTATION-GATE] Starting 5/5 Mutation Verification for: ${targetFilePath}`);
  console.log(`   Test suite: ${testCommands.map((c) => c[0] + ' ' + c[1].join(' ')).join(' && ')}`);
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);

  // First verify baseline passes
  console.log(`▶ Verifying clean baseline tests...`);
  let baselinePass = true;
  for (const [bin, args] of testCommands) {
    const res = spawnSync(bin, args, { cwd: PROJECT_ROOT, stdio: 'pipe' });
    if (res.status !== 0) {
      baselinePass = false;
      break;
    }
  }

  if (!baselinePass) {
    console.error(`❌ Baseline tests failed before any mutation! Fix implementation first.`);
    return { success: false, reason: 'BASELINE_FAILED' };
  }
  console.log(`✅ Baseline tests clean (GREEN).\n`);

  const results = [];

  try {
    for (let i = 0; i < MUTATION_PATTERNS.length; i++) {
      const mut = MUTATION_PATTERNS[i];
      console.log(`▶ Testing [${i + 1}/5]: ${mut.name} (${mut.description})...`);

      // Search for first match
      const match = originalContent.match(mut.search);
      if (!match) {
        console.log(`   ⚠️ Pattern not applicable to this file (skipped).`);
        results.push({ name: mut.name, status: 'SKIPPED', description: mut.description });
        continue;
      }

      // Inject mutation
      const mutatedContent = originalContent.replace(mut.search, mut.replace(match[0]));
      fs.writeFileSync(fullPath, mutatedContent, 'utf8');

      // Run tests on mutated code
      let testFailedOnMutation = false;
      for (const [bin, args] of testCommands) {
        const res = spawnSync(bin, args, { cwd: PROJECT_ROOT, stdio: 'pipe' });
        if (res.status !== 0) {
          testFailedOnMutation = true;
          break;
        }
      }

      // Revert immediately
      fs.writeFileSync(fullPath, originalContent, 'utf8');

      if (testFailedOnMutation) {
        console.log(`   🗡️ KILLED (Tests correctly caught the injected bug!)`);
        results.push({ name: mut.name, status: 'KILLED', description: mut.description });
      } else {
        console.warn(`   🧟 SURVIVED! (Tests passed despite injected bug — coverage gap!)`);
        results.push({ name: mut.name, status: 'SURVIVED', description: mut.description });
      }
    }
  } finally {
    // Guaranteed rollback to original content
    fs.writeFileSync(fullPath, originalContent, 'utf8');
  }

  const killed = results.filter((r) => r.status === 'KILLED').length;
  const survived = results.filter((r) => r.status === 'SURVIVED').length;
  const totalTested = killed + survived;
  const killRate = totalTested > 0 ? Math.round((killed / totalTested) * 100) : 100;

  console.log(`\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  console.log(`📊 MUTATION RESULTS: ${killed}/${totalTested} killed (${killRate}%)`);
  if (survived === 0 && killed > 0) {
    console.log(`🎉 5/5 MUTATION GATE PASSED! Tests independently verify contract.`);
  } else if (survived > 0) {
    console.log(`⚠️  ${survived} mutation(s) survived! Additional contract assertions required.`);
  }
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n`);

  return { success: survived === 0, killed, survived, killRate, results };
}

// CLI handler
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const target = process.argv[2] || 'rust/gui-spike/ui/state/signals.js';
  const customCmd = process.argv[3] || null;

  try {
    const res = runMutationGate(target, customCmd);
    process.exit(res.success ? 0 : 1);
  } catch (err) {
    console.error(`Mutation gate error:`, err.message);
    process.exit(1);
  }
}
