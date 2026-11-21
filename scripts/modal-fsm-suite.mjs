#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/modal-fsm-suite.mjs — Dedicated Modal & Dialog FSM Test Runner
 *
 * Runs universal modal router state machine tests and all individual modal unit tests,
 * verifying focus management, Escape dismiss, backdrop dismiss, and WCAG A11y compliance.
 *
 * Usage:
 *   node scripts/modal-fsm-suite.mjs
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const MODAL_TESTS = [
  'tests/unit/modals-fsm-contract.test.mjs',
  'tests/unit/modal-router.test.mjs',
  'tests/unit/settings-modal.test.mjs',
  'tests/unit/profile-modal.test.mjs',
  'tests/unit/invite-modal.test.mjs',
  'tests/unit/guild-modals.test.mjs',
  'tests/unit/events-modal.test.mjs',
  'tests/unit/captcha-modal.test.mjs',
  'tests/unit/captcha-modal-interactive.test.mjs',
  'tests/unit/gift-modal.test.mjs',
  'tests/unit/qr-auth-modal.test.mjs',
  'tests/unit/rules-screening-banner-modal.test.mjs',
];

async function main() {
  console.log(`🪟 [MODAL-FSM-SUITE] Executing Universal Modal & Dialog Quality Test Suite`);
  console.log(`================================================================================`);

  let passedSuites = 0;
  let failedSuites = 0;

  for (const testFile of MODAL_TESTS) {
    const fullPath = path.join(PROJECT_ROOT, testFile);
    if (!fs.existsSync(fullPath)) {
      console.log(`⚠️  Skipping missing test: ${testFile}`);
      continue;
    }

    const t0 = Date.now();
    const res = spawnSync('node', ['--test', testFile], {
      cwd: PROJECT_ROOT,
      stdio: 'pipe',
      encoding: 'utf8',
    });
    const dur = ((Date.now() - t0) / 1000).toFixed(2);

    if (res.status === 0) {
      console.log(`   ✅ PASS (${dur}s): ${testFile}`);
      passedSuites++;
    } else {
      console.error(`   ❌ FAIL (${dur}s): ${testFile}`);
      console.error(res.stderr || res.stdout);
      failedSuites++;
    }
  }

  console.log(`────────────────────────────────────────────────────────────────────────────────`);
  console.log(`📊 Total Suites Run: ${passedSuites + failedSuites} | Passed: ${passedSuites} | Failed: ${failedSuites}`);

  if (failedSuites === 0) {
    console.log(`🎉 [MODAL-FSM-SUITE] All Modal Dialogs PASSED FSM & A11y Quality Standards!`);
    process.exit(0);
  } else {
    console.error(`💥 [MODAL-FSM-SUITE] Some modal tests failed.`);
    process.exit(1);
  }
}

main();
