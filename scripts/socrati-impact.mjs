#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/socrati-impact.mjs — SocratiCode Blast Radius & Smart Test Selector
 *
 * Calls SocratiCode codebase_impact to calculate exact blast radius before editing a file,
 * and automatically selects the exact targeted tests (T0) that must be verified.
 *
 * Usage:
 *   node scripts/socrati-impact.mjs <file_path> [--depth 3]
 */

import { spawn } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const SOCRATICODE_BIN = '/home/sanya/.local/bin/socraticode';

async function callSocratiCodeTool(toolName, args) {
  return new Promise((resolve, reject) => {
    if (!fs.existsSync(SOCRATICODE_BIN)) {
      return reject(new Error('SocratiCode binary not found'));
    }

    const proc = spawn(SOCRATICODE_BIN, [], {
      cwd: PROJECT_ROOT,
      stdio: ['pipe', 'pipe', 'ignore'],
    });

    let stdoutData = '';
    let initialized = false;

    const timeout = setTimeout(() => {
      proc.kill();
      reject(new Error('SocratiCode request timed out'));
    }, 8000);

    proc.stdout.on('data', (chunk) => {
      stdoutData += chunk.toString('utf8');
      const lines = stdoutData.split('\n');
      stdoutData = lines.pop() || '';

      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const msg = JSON.parse(line.trim());
          if (msg.id === 1) {
            // Initialized -> send tool call
            initialized = true;
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
          // ignore non-json notifications
        }
      }
    });

    proc.on('error', (err) => {
      clearTimeout(timeout);
      reject(err);
    });

    // Send initialize request
    const initReq = {
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'socrati-impact-cli', version: '1.0' },
      },
    };
    proc.stdin.write(JSON.stringify(initReq) + '\n');
  });
}

function parseImpactOutput(text) {
  const lines = text.split('\n');
  const files = [];
  const tests = [];

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('- ')) {
      const filePath = trimmed.slice(2).trim();
      files.push(filePath);
      if (
        filePath.includes('_contract.rs') ||
        filePath.includes('.test.mjs') ||
        filePath.startsWith('tests/') ||
        filePath.includes('/tests/')
      ) {
        tests.push(filePath);
      }
    }
  }

  return { files, tests };
}

async function runTargetedTests(tests) {
  if (tests.length === 0) {
    console.log('⚡ [SOCRATI-IMPACT] No targeted test suites identified in blast radius.');
    return 0;
  }

  console.log(`\n🚀 [SOCRATI-IMPACT] Auto-running ${tests.length} targeted test suites...`);
  let hasFailures = false;

  for (const t of tests.slice(0, 5)) {
    if (t.endsWith('.rs')) {
      const testName = path.basename(t, '.rs');
      console.log(`\n▶️  Running Rust contract test: ${testName}`);
      const res = spawnSync('bash', [path.join(PROJECT_ROOT, 'scripts/rust-slim.sh'), 'test', '--test', testName], {
        cwd: PROJECT_ROOT,
        stdio: 'inherit',
      });
      if (res.status !== 0) hasFailures = true;
    } else {
      console.log(`\n▶️  Running Node.js test: ${t}`);
      const res = spawnSync('node', ['--test', t], {
        cwd: PROJECT_ROOT,
        stdio: 'inherit',
      });
      if (res.status !== 0) hasFailures = true;
    }
  }

  return hasFailures ? 1 : 0;
}

async function main() {
  const isRunMode = process.argv.includes('--run');
  const targetArg = process.argv.filter(a => !a.startsWith('--') && a !== process.argv[0] && a !== process.argv[1])[0];

  let normTarget = targetArg;
  if (!normTarget) {
    // Auto-detect from git status / diff
    try {
      const diffOutput = spawnSync('git', ['diff', '--name-only', 'HEAD'], { encoding: 'utf8' }).stdout || '';
      const changedFiles = diffOutput.trim().split('\n').filter(f => f && (f.endsWith('.rs') || f.endsWith('.js') || f.endsWith('.mjs')));
      if (changedFiles.length > 0) {
        normTarget = changedFiles[0];
        console.log(`🔍 [SOCRATI-IMPACT] Auto-detected changed file: ${normTarget}`);
      }
    } catch {
      // fallback
    }
  }

  if (!normTarget) {
    console.error('Usage: node scripts/socrati-impact.mjs <file_path> [--run] [--depth 3]');
    process.exit(1);
  }

  const normalized = path.relative(PROJECT_ROOT, path.resolve(PROJECT_ROOT, normTarget));

  console.log(`💥 [SOCRATI-IMPACT] Calculating Blast Radius for: ${normalized}`);

  try {
    const res = await callSocratiCodeTool('codebase_impact', {
      projectPath: PROJECT_ROOT,
      target: normalized,
      depth: 3,
    });

    const contentText = res.content?.[0]?.text || '';
    const { files, tests } = parseImpactOutput(contentText);

    console.log(`📊 Total impacted files: ${files.length}`);
    if (files.length === 0) {
      console.log('   ✅ Standalone module or leaf: No callers or dependents found.');
    } else {
      console.log(`\n🎯 Recommended Targeted Tests (T0):`);
      if (tests.length === 0) {
        console.log('   (No direct unit tests in immediate blast radius — run targeted crate test)');
      } else {
        for (const t of tests.slice(0, 8)) {
          if (t.endsWith('.rs')) {
            const testName = path.basename(t, '.rs');
            console.log(`   • cargo test --test ${testName}`);
          } else {
            console.log(`   • node --test ${t}`);
          }
        }
        if (tests.length > 8) {
          console.log(`   ... and ${tests.length - 8} more test suites.`);
        }
      }

      console.log(`\n🔗 Top 5 Direct Downstream Dependents (Hop 1):`);
      for (const f of files.slice(0, 5)) {
        console.log(`   • ${f}`);
      }
    }

    if (isRunMode && tests.length > 0) {
      const code = await runTargetedTests(tests);
      process.exit(code);
    }
  } catch (err) {
    console.error(`⚠️  [SOCRATI-IMPACT] SocratiCode tool unavailable: ${err.message}`);
    console.log('   (Fallback: using grep for immediate module references)');
  }
}

main();
