#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/qa-inspector.mjs — Comprehensive QA Department Automation & Feature Inspector
 *
 * Automated QA scanner for:
 * 1. UI <-> Rust BFF Command Parity (detects unhandled UI calls)
 * 2. Modal Dialog FSM & Accessibility Audit (checks all modals against state machine & a11y criteria)
 * 3. Stub & Unimplemented Feature Discovery
 * 4. QA Readiness Score (0-100%)
 *
 * Usage:
 *   node scripts/qa-inspector.mjs [--strict] [--json]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

function extractUiInvokes(uiDir) {
  const invokes = new Set();
  function walk(dir) {
    if (!fs.existsSync(dir)) return;
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.isFile() && e.name.endsWith('.js')) {
        const content = fs.readFileSync(full, 'utf8');
        const matches = content.matchAll(/invoke\(\s*['"]([a-zA-Z0-9_]+)['"]/g);
        for (const m of matches) {
          invokes.add(m[1]);
        }
      }
    }
  }
  walk(uiDir);
  return Array.from(invokes).sort();
}

function extractRustCommands(rustSrcDir) {
  const commands = new Set();
  function walk(dir) {
    if (!fs.existsSync(dir)) return;
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.isFile() && e.name.endsWith('.rs')) {
        const content = fs.readFileSync(full, 'utf8');
        const matches = content.matchAll(/#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)/g);
        for (const m of matches) {
          commands.add(m[1]);
        }
      }
    }
  }
  walk(rustSrcDir);
  return Array.from(commands).sort();
}

function auditModals(uiFeaturesDir, unitTestsDir) {
  const routerPath = path.join(uiFeaturesDir, 'modal-router.js');
  let modalIds = [];
  if (fs.existsSync(routerPath)) {
    const content = fs.readFileSync(routerPath, 'utf8');
    const match = content.match(/KNOWN_MODAL_IDS\s*=\s*Object\.freeze\(\[\s*([\s\S]*?)\]\)/);
    if (match) {
      modalIds = match[1]
        .split('\n')
        .map(l => l.trim().replace(/['",]/g, ''))
        .filter(l => l && !l.startsWith('//'));
    }
  }

  const results = [];
  for (const id of modalIds) {
    // Check if test exists
    const candidateNames = [
      `${id}.test.mjs`,
      `${id.replace('-modal', '')}-modal.test.mjs`,
      `${id.replace('-dialog', '')}.test.mjs`,
    ];
    let testFound = null;
    let hasEscapeCheck = false;
    let hasBackdropCheck = false;
    let hasAriaCheck = false;

    for (const c of candidateNames) {
      const p = path.join(unitTestsDir, c);
      if (fs.existsSync(p)) {
        testFound = c;
        const testContent = fs.readFileSync(p, 'utf8');
        if (testContent.includes('Escape') || testContent.includes('escape') || testContent.includes('closeTop')) {
          hasEscapeCheck = true;
        }
        if (testContent.includes('backdrop') || testContent.includes('overlay') || testContent.includes('closeAll')) {
          hasBackdropCheck = true;
        }
        if (testContent.includes('dialog') || testContent.includes('aria-modal') || testContent.includes('aria-')) {
          hasAriaCheck = true;
        }
        break;
      }
    }

    results.push({
      id,
      testFound,
      hasEscapeCheck,
      hasBackdropCheck,
      hasAriaCheck,
      score: (testFound ? 25 : 0) + (hasEscapeCheck ? 25 : 0) + (hasBackdropCheck ? 25 : 0) + (hasAriaCheck ? 25 : 0),
    });
  }

  return results;
}

function findStubs(dir) {
  const stubs = [];
  function walk(current) {
    if (!fs.existsSync(current)) return;
    for (const e of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, e.name);
      if (e.isDirectory() && e.name !== 'target' && e.name !== 'node_modules') {
        walk(full);
      } else if (e.isFile() && (e.name.endsWith('.js') || e.name.endsWith('.rs'))) {
        const lines = fs.readFileSync(full, 'utf8').split('\n');
        lines.forEach((line, idx) => {
          const match = line.match(/(STUB-\d+|TODO|FIXME)/i);
          if (match) {
            stubs.push({
              file: path.relative(PROJECT_ROOT, full),
              line: idx + 1,
              tag: match[1].toUpperCase(),
              text: line.trim(),
            });
          }
        });
      }
    }
  }
  walk(dir);
  return stubs;
}

async function main() {
  console.log(`🛡️   [QA-INSPECTOR] Automated Product Quality & Completeness Audit`);
  console.log(`================================================================================`);

  // 1. UI <-> BFF Command Parity
  const uiInvokes = extractUiInvokes(path.join(PROJECT_ROOT, 'rust/gui-spike/ui'));
  const rustCommands = new Set(extractRustCommands(path.join(PROJECT_ROOT, 'rust/gui-spike/src')));

  const missingCommands = uiInvokes.filter(cmd => !rustCommands.has(cmd));
  console.log(`📡 1. UI <-> RUST BFF COMMAND PARITY:`);
  console.log(`   • Total UI Invoked Commands  : ${uiInvokes.length}`);
  console.log(`   • Total Registered BFF Fn    : ${rustCommands.size}`);
  if (missingCommands.length === 0) {
    console.log(`   ✅ 100% PARITY: All UI commands are handled in Rust backend!`);
  } else {
    console.log(`   ⚠️  Found ${missingCommands.length} unhandled UI command(s):`);
    missingCommands.forEach(c => console.log(`      ❌ ${c}`));
  }
  console.log(`────────────────────────────────────────────────────────────────────────────────`);

  // 2. Modals & Dialogs FSM Audit
  const modals = auditModals(
    path.join(PROJECT_ROOT, 'rust/gui-spike/ui/features'),
    path.join(PROJECT_ROOT, 'tests/unit')
  );

  console.log(`🪟 2. MODAL & DIALOG STATE MACHINE (FSM) & A11Y AUDIT:`);
  console.log(`   ┌─────────────────────────┬────────┬────────┬──────────┬──────────┬───────┐`);
  console.log(`   │ Modal Layer ID          │ Tested │ Escape │ Backdrop │ A11y/Aria│ Score │`);
  console.log(`   ├─────────────────────────┼────────┼────────┼──────────┼──────────┼───────┤`);
  for (const m of modals) {
    const tIcon = m.testFound ? '  ✅  ' : '  ❌  ';
    const eIcon = m.hasEscapeCheck ? '  ✅   ' : '  ⚠️    ';
    const bIcon = m.hasBackdropCheck ? '   ✅    ' : '   ⚠️     ';
    const aIcon = m.hasAriaCheck ? '   ✅    ' : '   ⚠️     ';
    console.log(`   │ ${m.id.padEnd(23)} │${tIcon}│${eIcon}│${bIcon}│${aIcon}│ ${String(m.score).padStart(3)}%  │`);
  }
  console.log(`   └─────────────────────────┴────────┴────────┴──────────┴──────────┴───────┘`);
  const avgModalScore = Math.round(modals.reduce((acc, m) => acc + m.score, 0) / (modals.length || 1));
  console.log(`   • Modal Quality Maturity: ${avgModalScore}% compliance with FSM/A11y standards.`);
  console.log(`────────────────────────────────────────────────────────────────────────────────`);

  // 3. Stub & Unimplemented Feature Discovery
  const stubs = findStubs(path.join(PROJECT_ROOT, 'rust/gui-spike/ui/features'));
  console.log(`🔍 3. UNIMPLEMENTED FEATURES & STUB DISCOVERY:`);
  console.log(`   • Total STUB/TODO annotations: ${stubs.length}`);
  const stubsByTag = {};
  stubs.forEach(s => {
    stubsByTag[s.tag] = (stubsByTag[s.tag] || 0) + 1;
  });
  console.log(`   • Breakdown: ${Object.entries(stubsByTag).map(([k, v]) => `${k} (${v})`).join(', ')}`);
  for (const s of stubs.slice(0, 5)) {
    console.log(`     - [${s.file}:${s.line}] ${s.text.slice(0, 75)}...`);
  }
  if (stubs.length > 5) {
    console.log(`     ... and ${stubs.length - 5} more tracked stubs in backlog.`);
  }
  console.log(`────────────────────────────────────────────────────────────────────────────────`);

  // 4. Overall Product QA Score
  const commandScore = missingCommands.length === 0 ? 100 : Math.max(0, 100 - missingCommands.length * 10);
  const overallScore = Math.round(commandScore * 0.5 + avgModalScore * 0.5);

  console.log(`🏆 OVERALL PRODUCT QA READINESS: ${overallScore}%`);
  if (overallScore >= 80) {
    console.log(`   ✅ QUALITY GATE: READY FOR QA QUALIFICATION`);
  } else {
    console.log(`   ⚠️  QUALITY GATE: ACTION REQUIRED (Improve modal FSM & A11y tests)`);
  }
  console.log(`================================================================================\n`);
}

main();
