#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/socrati-smart-suite.mjs — SocratiCode Tiered Test Orchestrator (Unit, Smoke, E2E, Contract)
 *
 * Automatically detects impacted tests across all 4 testing tiers via SocratiCode AST Blast Radius,
 * running them in a fail-fast pyramid (Unit & Contract -> Smoke -> E2E) with slim error logs.
 *
 * Usage:
 *   node scripts/socrati-smart-suite.mjs [file_path] [--unit] [--smoke] [--e2e] [--all]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const SOCRATICODE_BIN = '/home/sanya/.local/bin/socraticode';

async function callSocratiCodeTool(toolName, args = {}) {
  return new Promise((resolve, reject) => {
    if (!fs.existsSync(SOCRATICODE_BIN)) {
      return reject(new Error('SocratiCode binary not found'));
    }

    const proc = spawn(SOCRATICODE_BIN, [], {
      cwd: PROJECT_ROOT,
      stdio: ['pipe', 'pipe', 'ignore'],
    });

    let stdoutData = '';
    const timeout = setTimeout(() => {
      proc.kill();
      reject(new Error('SocratiCode request timed out'));
    }, 15000);

    proc.stdout.on('data', (chunk) => {
      stdoutData += chunk.toString('utf8');
      const lines = stdoutData.split('\n');
      stdoutData = lines.pop() || '';

      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const msg = JSON.parse(line.trim());
          if (msg.id === 1) {
            const toolReq = {
              jsonrpc: '2.0',
              id: 2,
              method: 'tools/call',
              params: {
                name: toolName,
                arguments: args,
              },
            };
            proc.stdin.write(JSON.stringify(toolReq) + '\n');
          } else if (msg.id === 2) {
            clearTimeout(timeout);
            proc.kill();
            if (msg.error) {
              return reject(new Error(msg.error.message || 'Tool execution error'));
            }
            return resolve(msg.result);
          }
        } catch {
          // ignore
        }
      }
    });

    proc.on('error', (err) => {
      clearTimeout(timeout);
      reject(err);
    });

    proc.stdin.write(JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'socrati-smart-suite', version: '1.0' },
      },
    }) + '\n');
  });
}

function parseImpactedFiles(impactText) {
  const lines = impactText.split('\n');
  const files = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('- ')) {
      files.push(trimmed.slice(2).trim());
    }
  }
  return files;
}

function categorizeTests(fileList) {
  const tiers = {
    unit: new Set(),
    contract: new Set(),
    smoke: new Set(),
    e2e: new Set(),
  };

  for (const f of fileList) {
    if (f.startsWith('tests/unit/') || f.includes('/tests/unit/')) {
      tiers.unit.add(f);
    } else if (f.includes('_contract.rs') || (f.startsWith('rust/') && f.includes('/tests/'))) {
      tiers.contract.add(f);
    } else if (f.startsWith('tests/smoke/') || f.includes('.smoke.test.')) {
      tiers.smoke.add(f);
    } else if (f.startsWith('tests/e2e/') || f.includes('.e2e.test.')) {
      tiers.e2e.add(f);
    }
  }

  return {
    unit: Array.from(tiers.unit),
    contract: Array.from(tiers.contract),
    smoke: Array.from(tiers.smoke),
    e2e: Array.from(tiers.e2e),
  };
}

function runCommand(cmd, args, desc) {
  console.log(`\n▶️  [TEST-SUITE] ${desc}`);
  console.log(`   $ ${cmd} ${args.join(' ')}`);
  const t0 = Date.now();
  const res = spawnSync(cmd, args, { cwd: PROJECT_ROOT, stdio: 'inherit' });
  const dur = ((Date.now() - t0) / 1000).toFixed(2);
  if (res.status === 0) {
    console.log(`   ✅ PASSED (${dur}s)`);
    return true;
  } else {
    console.error(`   ❌ FAILED with status ${res.status} (${dur}s)`);
    return false;
  }
}

async function main() {
  const args = process.argv.slice(2);
  const onlyUnit = args.includes('--unit');
  const onlySmoke = args.includes('--smoke');
  const onlyE2E = args.includes('--e2e');
  const runAllTiers = args.includes('--all');
  const targetArg = args.find(a => !a.startsWith('--'));

  let targetFiles = [];

  if (targetArg) {
    targetFiles = [targetArg];
  } else {
    // Detect changed files from git diff
    try {
      const diffOutput = spawnSync('git', ['diff', '--name-only', 'HEAD~1'], { encoding: 'utf8' }).stdout || '';
      targetFiles = diffOutput.trim().split('\n').filter(Boolean);
      if (targetFiles.length === 0) {
        const statusOutput = spawnSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).stdout || '';
        targetFiles = statusOutput.trim().split('\n').map(l => l.slice(3).trim()).filter(Boolean);
      }
    } catch {
      // fallback
    }
  }

  console.log(`🎯 [SOCRATI-SUITE] Tiered Test Orchestrator`);
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);

  const allImpactedTests = { unit: [], contract: [], smoke: [], e2e: [] };

  if (targetFiles.length === 0) {
    console.log(`ℹ️  No modified files detected in workspace.`);
    console.log(`   Running core smoke tests by default...`);
    allImpactedTests.smoke = ['tests/smoke/cli.smoke.test.mjs', 'tests/smoke/gui-spike.smoke.test.mjs'];
  } else {
    console.log(`🔍 Analyzing blast radius for ${targetFiles.length} file(s)...`);
    for (const tf of targetFiles.slice(0, 5)) {
      const relPath = path.relative(PROJECT_ROOT, path.resolve(PROJECT_ROOT, tf));
      
      // If the target itself is a test file, include it directly
      const directCategorized = categorizeTests([relPath]);
      directCategorized.unit.forEach(t => allImpactedTests.unit.push(t));
      directCategorized.contract.forEach(t => allImpactedTests.contract.push(t));
      directCategorized.smoke.forEach(t => allImpactedTests.smoke.push(t));
      directCategorized.e2e.forEach(t => allImpactedTests.e2e.push(t));

      try {
        const impactRes = await callSocratiCodeTool('codebase_impact', {
          projectPath: PROJECT_ROOT,
          target: relPath,
          depth: 3,
        });
        const impactText = impactRes.content?.[0]?.text || '';
        const files = parseImpactedFiles(impactText);
        const categorized = categorizeTests(files);

        categorized.unit.forEach(t => allImpactedTests.unit.push(t));
        categorized.contract.forEach(t => allImpactedTests.contract.push(t));
        categorized.smoke.forEach(t => allImpactedTests.smoke.push(t));
        categorized.e2e.forEach(t => allImpactedTests.e2e.push(t));
      } catch (err) {
        console.log(`   (Impact analysis skipped for ${relPath}: ${err.message})`);
      }
    }
  }

  // Deduplicate
  allImpactedTests.unit = Array.from(new Set(allImpactedTests.unit));
  allImpactedTests.contract = Array.from(new Set(allImpactedTests.contract));
  allImpactedTests.smoke = Array.from(new Set(allImpactedTests.smoke));
  allImpactedTests.e2e = Array.from(new Set(allImpactedTests.e2e));

  console.log(`\n📊 Identified Impacted Tests across Pyramid:`);
  console.log(`   • [UNIT]     : ${allImpactedTests.unit.length} test suite(s)`);
  console.log(`   • [CONTRACT] : ${allImpactedTests.contract.length} test suite(s)`);
  console.log(`   • [SMOKE]    : ${allImpactedTests.smoke.length} test suite(s)`);
  console.log(`   • [E2E]      : ${allImpactedTests.e2e.length} test suite(s)`);

  let shouldRunUnit = (onlyUnit || runAllTiers || (!onlySmoke && !onlyE2E));
  let shouldRunContract = (onlyUnit || runAllTiers || (!onlySmoke && !onlyE2E));
  let shouldRunSmoke = (onlySmoke || runAllTiers || (!onlyUnit && !onlyE2E));
  let shouldRunE2E = (onlyE2E || runAllTiers);

  // Execution Phase with Fail-Fast
  let totalFailed = 0;

  // 1. Tier 1: Unit & Contract tests
  if (shouldRunUnit && allImpactedTests.unit.length > 0) {
    console.log(`\n🚀 === TIER 1: UNIT TESTS ===`);
    for (const ut of allImpactedTests.unit.slice(0, 5)) {
      const ok = runCommand('node', ['--test', ut], `Node Unit: ${ut}`);
      if (!ok) {
        console.error(`💥 FAIL-FAST: Unit test failed! Halting higher tiers.`);
        process.exit(1);
      }
    }
  }

  if (shouldRunContract && allImpactedTests.contract.length > 0) {
    console.log(`\n🚀 === TIER 1: RUST CONTRACT TESTS ===`);
    for (const ct of allImpactedTests.contract.slice(0, 5)) {
      const testName = path.basename(ct, '.rs');
      let manifestPath = 'rust/gui-spike/Cargo.toml';
      if (ct.startsWith('tests/e2e/harness/')) {
        manifestPath = 'tests/e2e/harness/Cargo.toml';
      } else if (ct.startsWith('rust/crates/')) {
        const parts = ct.split('/');
        manifestPath = `rust/crates/${parts[2]}/Cargo.toml`;
      }

      const ok = runCommand('cargo', ['test', '--manifest-path', manifestPath, '--offline', '--test', testName], `Rust Contract: ${testName} (${manifestPath})`);
      if (!ok) {
        console.error(`💥 FAIL-FAST: Rust contract test failed! Halting higher tiers.`);
        process.exit(1);
      }
    }
  }

  // 2. Tier 2: Smoke tests
  if (shouldRunSmoke && allImpactedTests.smoke.length > 0) {
    console.log(`\n🚀 === TIER 2: SMOKE TESTS ===`);
    for (const st of allImpactedTests.smoke.slice(0, 3)) {
      const ok = runCommand('node', ['--test', st], `Smoke: ${st}`);
      if (!ok) {
        console.error(`💥 FAIL-FAST: Smoke test failed! Halting higher tiers.`);
        process.exit(1);
      }
    }
  }

  // 3. Tier 3: E2E tests
  if (shouldRunE2E && allImpactedTests.e2e.length > 0) {
    console.log(`\n🚀 === TIER 3: E2E TESTS ===`);
    for (const et of allImpactedTests.e2e.slice(0, 2)) {
      const ok = runCommand('node', ['--test', et], `E2E: ${et}`);
      if (!ok) {
        console.error(`💥 E2E test failed.`);
        totalFailed++;
      }
    }
  }

  console.log(`\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  if (totalFailed === 0) {
    console.log(`🎉 [SOCRATI-SUITE] All executed test tiers PASSED successfully!`);
    process.exit(0);
  } else {
    console.error(`❌ [SOCRATI-SUITE] Some tests failed.`);
    process.exit(1);
  }
}

main();
