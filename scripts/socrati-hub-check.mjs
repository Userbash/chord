#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/socrati-hub-check.mjs — Architectural Hub Impact Detector
 *
 * Scans modified/staged files against SocratiCode dependency graph hubs.
 * If a core hub (>30 dependents) is modified, forces targeted test reminders.
 *
 * Usage:
 *   node scripts/socrati-hub-check.mjs [--staged]
 */

import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

// Known architectural hubs in Chord/Discord-Lab based on AST analysis
const CORE_HUBS = {
  'rust/gui-spike/src/data.rs': 47,
  'rust/crates/chord-core/src/state/mod.rs': 273,
  'rust/crates/chord-core/src/lib.rs': 256,
  'rust/chord-gpui/src/state/mod.rs': 198,
  'rust/crates/chord-core/src/rest/client.rs': 189,
  'rust/crates/chord-voice/src/lib.rs': 150,
  'rust/crates/chord-platform/src/audio_engine.rs': 42,
  'rust/gui-spike/ui/model.js': 38,
};

function getModifiedFiles(isStagedOnly) {
  try {
    const cmd = isStagedOnly
      ? 'git diff --cached --name-only --diff-filter=ACM'
      : 'git status --porcelain';
    const output = execSync(cmd, { cwd: PROJECT_ROOT, encoding: 'utf8' });
    if (isStagedOnly) {
      return output.split('\n').filter(Boolean);
    } else {
      return output
        .split('\n')
        .map((l) => l.trim().slice(3))
        .filter(Boolean);
    }
  } catch {
    return [];
  }
}

function main() {
  const isStaged = process.argv.includes('--staged');
  const files = getModifiedFiles(isStaged);

  console.log('🏛️  [SOCRATI-HUB] Checking modified files for high-impact architectural hubs...');

  const touchedHubs = [];
  for (const f of files) {
    const norm = f.replace(/\\/g, '/');
    if (CORE_HUBS[norm]) {
      touchedHubs.push({ file: norm, dependents: CORE_HUBS[norm] });
    }
  }

  if (touchedHubs.length === 0) {
    console.log('✅ [SOCRATI-HUB] Safe: No high-impact core hubs modified.');
    process.exit(0);
  }

  console.log(`\n⚠️  [SOCRATI-HUB] NOTICE: ${touchedHubs.length} core architectural hub(s) modified:`);
  for (const h of touchedHubs) {
    console.log(`   🚨 ${h.file} -> ${h.dependents} downstream dependents!`);
  }
  console.log('💡 Recommendation: Run `npm run socrati:impact <file>` to verify targeted tests (T0).\n');
  process.exit(0);
}

main();
