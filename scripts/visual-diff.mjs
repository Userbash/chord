#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/visual-diff.mjs — Headless Visual Regression & Snapshot Diffing
 *
 * Compares UI screenshots against baseline snapshots to detect layout shifts,
 * broken components, and color/spacing regressions for PM acceptance.
 *
 * Usage:
 *   node scripts/visual-diff.mjs status
 *   node scripts/visual-diff.mjs compare <baseline_img> <candidate_img> [--threshold 0.002]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const BASELINE_DIR = path.join(PROJECT_ROOT, 'waves/BASELINE/screenshots');
const DIFF_DIR = path.join(PROJECT_ROOT, '.gate/visual-diff');

function ensureDirs() {
  if (!fs.existsSync(BASELINE_DIR)) fs.mkdirSync(BASELINE_DIR, { recursive: true });
  if (!fs.existsSync(DIFF_DIR)) fs.mkdirSync(DIFF_DIR, { recursive: true });
}

function compareImageBuffers(bufA, bufB) {
  // Fast binary and dimension hash comparison
  if (bufA.length === bufB.length && bufA.equals(bufB)) {
    return { diffRatio: 0, identical: true };
  }

  // Calculate byte difference ratio as first-pass visual proxy
  const minLen = Math.min(bufA.length, bufB.length);
  const maxLen = Math.max(bufA.length, bufB.length);
  let diffBytes = maxLen - minLen;

  for (let i = 0; i < minLen; i += 4) {
    if (bufA[i] !== bufB[i] || bufA[i + 1] !== bufB[i + 1] || bufA[i + 2] !== bufB[i + 2]) {
      diffBytes++;
    }
  }

  const diffRatio = diffBytes / (maxLen / 4);
  return {
    diffRatio: Math.min(1.0, diffRatio),
    identical: diffRatio === 0,
    sizeA: bufA.length,
    sizeB: bufB.length,
  };
}

function main() {
  ensureDirs();
  const args = process.argv.slice(2);
  const cmd = args[0] || 'status';

  if (cmd === 'status') {
    const baselines = fs.readdirSync(BASELINE_DIR).filter((f) => f.endsWith('.png'));
    console.log('\n🖼️  [VISUAL-DIFF] Headless Visual Regression Engine');
    console.log(`📁 Baseline Directory : ${BASELINE_DIR}`);
    console.log(`📊 Total Baselines    : ${baselines.length} snapshot(s)`);
    for (const b of baselines) {
      console.log(`   • ${b}`);
    }
    console.log(`📁 Diff Output Dir    : ${DIFF_DIR}\n`);
    process.exit(0);
  }

  if (cmd === 'compare') {
    const imgA = args[1];
    const imgB = args[2];
    const threshold = parseFloat(args.find((a, i) => args[i - 1] === '--threshold') || '0.002');

    if (!imgA || !imgB) {
      console.error('Usage: visual-diff.mjs compare <baseline_img> <candidate_img> [--threshold 0.002]');
      process.exit(1);
    }

    if (!fs.existsSync(imgA) || !fs.existsSync(imgB)) {
      console.error(`❌ One of the image files does not exist: ${imgA} or ${imgB}`);
      process.exit(1);
    }

    const bufA = fs.readFileSync(imgA);
    const bufB = fs.readFileSync(imgB);

    const result = compareImageBuffers(bufA, bufB);
    const pct = (result.diffRatio * 100).toFixed(3);

    console.log(`\n🔍 [VISUAL-DIFF] Comparing screenshots:`);
    console.log(`   Baseline  : ${path.basename(imgA)} (${result.sizeA} bytes)`);
    console.log(`   Candidate : ${path.basename(imgB)} (${result.sizeB} bytes)`);
    console.log(`   Diff Ratio: ${pct}% (Max tolerance: ${(threshold * 100).toFixed(3)}%)`);

    if (result.diffRatio <= threshold) {
      console.log(`✅ [VISUAL-DIFF] PASSED: Visual shift is within acceptable threshold (${pct}% <= ${(threshold * 100).toFixed(3)}%)\n`);
      process.exit(0);
    } else {
      console.error(`❌ [VISUAL-DIFF] FAILED: Visual regression detected! Difference ${pct}% exceeds tolerance.\n`);
      process.exit(1);
    }
  }

  console.log('Commands: status, compare');
}

main();
