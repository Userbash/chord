#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/precheck.mjs — Zero-Token Ultra-Fast Pre-Commit & Syntax Verifier
 * Runs TypeScript, JS syntax check, and meta tests in parallel.
 */

import { execSync, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

async function runCheck(name, cmd, args) {
  const start = Date.now();
  return new Promise((resolve) => {
    const proc = spawn(cmd, args, { cwd: PROJECT_ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    let stderr = '';
    proc.stderr.on('data', (d) => { stderr += d.toString(); });
    proc.on('close', (code) => {
      resolve({
        name,
        ok: code === 0,
        duration: Date.now() - start,
        error: code !== 0 ? stderr.trim() : null,
      });
    });
  });
}

async function main() {
  const totalStart = Date.now();
  console.log('⚡ [PRECHECK] Running parallel sanity verification...');

  // 0. Verify GPUI status
  try {
    const gitDiff = execSync('git diff --name-only HEAD 2>/dev/null || true', { encoding: 'utf8' });
    if (gitDiff.split('\n').some(f => f.startsWith('rust/chord-gpui/src/app_layout.rs'))) {
      console.warn('⚠️ [PRECHECK] Note: Changes in core GPUI layout detected.');
    }
  } catch {}

  const checks = await Promise.all([
    runCheck('TSC', 'npx', ['tsc', '-p', 'tsconfig.json']),
    runCheck('Syntax', 'node', ['--check', 'rust/gui-spike/ui/app.js']),
    runCheck('Meta', 'node', ['--test', 'tests/meta/plan-sync.test.mjs', 'tests/meta/design-breakpoints.test.mjs']),
  ]);

  let allOk = true;
  for (const c of checks) {
    if (!c.ok) {
      allOk = false;
      console.error(`❌ [PRECHECK] ${c.name} FAILED (${c.duration}ms):`);
      if (c.error) console.error(c.error.split('\n').slice(0, 10).join('\n'));
    }
  }

  const durationSec = ((Date.now() - totalStart) / 1000).toFixed(1);
  if (allOk) {
    const details = checks.map(c => `${c.name}: ${c.duration}ms`).join(' | ');
    console.log(`✅ [PRECHECK] Passed in ${durationSec}s (${details})`);
    process.exit(0);
  } else {
    console.error(`💥 [PRECHECK] Failed in ${durationSec}s`);
    process.exit(1);
  }
}

main();
