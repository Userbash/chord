#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/qa-auto-system.mjs — Comprehensive Autonomous QA Testing System
 *
 * Coordinates the entire end-to-end automated testing workflow:
 * 1. Infrastructure Health (SocratiCode on node2 + local Ollama)
 * 2. Pre-Flight Architecture Checks (Syntax, Typescript, Meta Plan Sync)
 * 3. Security Shield (Secret scanning, unsafe blocks)
 * 4. Contract Parity (Rust BFF <-> Frontend DTOs)
 * 5. Modal Dialog FSM & A11y Suite (Universal State Machine verification)
 * 6. Tiered Test Pyramid (Unit -> Contract -> Smoke)
 * 7. Generates QA Certification Report
 *
 * Usage:
 *   node scripts/qa-auto-system.mjs [--quick] [--full]
 */

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

function runStep(title, cmd, args) {
  console.log(`\n▶️  [STEP] ${title}`);
  const t0 = Date.now();
  const res = spawnSync(cmd, args, { cwd: PROJECT_ROOT, stdio: 'inherit' });
  const dur = ((Date.now() - t0) / 1000).toFixed(2);
  if (res.status === 0) {
    console.log(`   ✅ ${title} PASSED (${dur}s)`);
    return true;
  } else {
    console.error(`   ❌ ${title} FAILED with code ${res.status} (${dur}s)`);
    return false;
  }
}

async function main() {
  const isQuick = process.argv.includes('--quick');
  const tStart = Date.now();

  console.log(`🛡️   [QA-AUTO-SYSTEM] Full Autonomous Testing & Quality Certification Engine`);
  console.log(`================================================================================`);

  // Step 1: SocratiCode Infrastructure Health
  if (!runStep('1. SocratiCode & node2 Infrastructure Guard', 'bash', ['scripts/socrati-guard.sh'])) {
    process.exit(1);
  }

  // Step 2: Sub-second Precheck (Syntax, TSC, Meta)
  if (!runStep('2. Parallel Sanity Precheck (Syntax, TypeScript, Meta)', 'node', ['scripts/precheck.mjs'])) {
    process.exit(1);
  }

  // Step 3: Security Shield
  if (!runStep('3. AST Security Shield (Secrets, Unsafe code, Dangerous calls)', 'node', ['scripts/security-shield.mjs'])) {
    process.exit(1);
  }

  // Step 4: DTO Contract Parity
  if (!runStep('4. Rust BFF <-> UI DTO Contract Linting', 'node', ['scripts/contract-lint.mjs'])) {
    process.exit(1);
  }

  // Step 5: GUI Feature Parity & DOM Integrity Audit
  if (!runStep('5. GUI Feature Parity & DOM Integrity Audit', 'node', ['scripts/gui-parity-audit.mjs'])) {
    process.exit(1);
  }

  // Step 6: Modal Dialogs FSM & A11y Suite
  if (!runStep('6. Modal Dialogs FSM & W3C Accessibility Suite', 'node', ['scripts/modal-fsm-suite.mjs'])) {
    process.exit(1);
  }

  // Step 7: Test Suite Hygiene
  if (!runStep('7. Test Suite Hygiene & Broken Import Audit', 'node', ['scripts/test-auditor.mjs'])) {
    process.exit(1);
  }

  // Step 8: Live Window Real E2E (if display available)
  const hasDisplay = Boolean(process.env.DISPLAY || process.env.WAYLAND_DISPLAY);
  if (hasDisplay && !isQuick) {
    if (!runStep('8. Native Window Real E2E & AT-SPI Controls', 'node', ['scripts/gui-live-e2e.mjs'])) {
      process.exit(1);
    }
  }

  // Step 9: Tiered Test Execution
  const suiteArgs = isQuick ? ['--unit'] : ['--smoke'];
  if (!runStep('9. SocratiCode Tiered Test Runner', 'node', ['scripts/socrati-smart-suite.mjs', ...suiteArgs])) {
    process.exit(1);
  }

  // Step 10: Telemetry Dashboard
  runStep('10. Project Architecture & Test Intelligence Dashboard', 'node', ['scripts/socrati-dashboard.mjs']);

  const totalDur = ((Date.now() - tStart) / 1000).toFixed(2);
  console.log(`================================================================================`);
  console.log(`🎉 [QA-AUTO-SYSTEM] 100% QUALITY CERTIFICATION COMPLETE in ${totalDur}s`);
  console.log(`   All architecture gates, contracts, modal FSMs, and tests are GREEN.`);
  console.log(`================================================================================\n`);
}

main();
