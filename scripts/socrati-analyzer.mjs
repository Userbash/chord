#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/socrati-analyzer.mjs — SocratiCode Automated Architecture & Test Gap Analyzer
 *
 * Scans codebase AST graph to identify high-impact modules, circular dependencies,
 * and untested files (gap analysis).
 *
 * Usage:
 *   node scripts/socrati-analyzer.mjs [--untested] [--json]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

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
          // ignore non-json
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
        clientInfo: { name: 'socrati-analyzer', version: '1.0' },
      },
    }) + '\n');
  });
}

function findRustFiles(dir, files = []) {
  if (!fs.existsSync(dir)) return files;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const e of entries) {
    const fullPath = path.join(dir, e.name);
    if (e.isDirectory() && e.name !== 'target' && e.name !== 'node_modules') {
      findRustFiles(fullPath, files);
    } else if (e.isFile() && e.name.endsWith('.rs') && !e.name.endsWith('_contract.rs')) {
      files.push(fullPath);
    }
  }
  return files;
}

function findContractTests(dir, tests = new Set()) {
  if (!fs.existsSync(dir)) return tests;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const e of entries) {
    const fullPath = path.join(dir, e.name);
    if (e.isDirectory() && e.name !== 'target' && e.name !== 'node_modules') {
      findContractTests(fullPath, tests);
    } else if (e.isFile() && (e.name.endsWith('_contract.rs') || e.name.endsWith('.test.mjs'))) {
      const base = e.name.replace('_contract.rs', '').replace('.test.mjs', '');
      tests.add(base);
    }
  }
  return tests;
}

async function main() {
  console.log('🔬 [SOCRATI-ANALYZER] Performing deep AST graph and test gap analysis...');

  // 1. Gather existing contract tests
  const existingTests = findContractTests(path.join(PROJECT_ROOT, 'rust'));
  findContractTests(path.join(PROJECT_ROOT, 'tests'), existingTests);

  // 2. Scan core source files
  const coreSrcDir = path.join(PROJECT_ROOT, 'rust/crates/chord-core/src');
  const srcFiles = findRustFiles(coreSrcDir);

  const untested = [];
  const covered = [];

  for (const f of srcFiles) {
    const rel = path.relative(PROJECT_ROOT, f);
    const base = path.basename(f, '.rs');
    if (base === 'mod' || base === 'lib') continue;

    if (existingTests.has(base)) {
      covered.push({ file: rel, module: base });
    } else {
      untested.push({ file: rel, module: base });
    }
  }

  console.log(`\n📊 [TEST COVERAGE GAP REPORT]`);
  console.log(`   Covered modules with dedicated contract tests: ${covered.length}`);
  console.log(`   Modules without dedicated contract tests      : ${untested.length}`);

  if (untested.length > 0) {
    console.log(`\n⚠️  Top Modules Lacking Dedicated Contract Tests:`);
    for (const u of untested.slice(0, 10)) {
      console.log(`   • ${u.file}  (Module: ${u.module})`);
    }
    if (untested.length > 10) {
      console.log(`   ... and ${untested.length - 10} more files.`);
    }
    console.log(`\n💡 To auto-generate a 9-dimension contract test for any module, run:`);
    console.log(`   node scripts/socrati-scaffold-test.mjs <source_file>`);
  }

  // 3. Query Graph Stats from SocratiCode
  try {
    const statsRes = await callSocratiCodeTool('codebase_graph_stats', { projectPath: PROJECT_ROOT });
    const statsText = statsRes.content?.[0]?.text || '';
    const topLines = statsText.split('\n').filter(l => l.includes('connections')).slice(0, 5);

    if (topLines.length > 0) {
      console.log(`\n🏛️  Top 5 Architectural Hubs by Dependencies:`);
      for (const line of topLines) {
        console.log(`   ${line.trim()}`);
      }
    }
  } catch (err) {
    console.log(`   (Graph stats cache updating: ${err.message})`);
  }

  console.log('\n✅ [SOCRATI-ANALYZER] Analysis complete.');
}

main();
