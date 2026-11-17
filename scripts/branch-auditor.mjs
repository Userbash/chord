#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/branch-auditor.mjs — Zero-Token Branch Hygiene & Audit Engine
 *
 * Automatically inspects, categorizes, validates, and prunes stale, duplicate,
 * and quarantined branches without manual token expenditure.
 *
 * Usage:
 *   node scripts/branch-auditor.mjs                    # Full audit report
 *   node scripts/branch-auditor.mjs --prune-duplicates # Delete redundant branches pointing to same SHA
 *   node scripts/branch-auditor.mjs --prune-merged     # Delete branches already applied/merged
 *   node scripts/branch-auditor.mjs --prune-stale      # Prune duplicates + merged + quarantined
 *   node scripts/branch-auditor.mjs --dry-run          # Preview pruning operations safely
 */

import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const PROTECTED_BRANCHES = new Set([
  'main',
  'tauri',
  'release/developer-alpha-preview',
  'unstable',
  'testing',
  'beta',
]);

function run(cmd, silent = false) {
  try {
    return execSync(cmd, { cwd: PROJECT_ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', silent ? 'ignore' : 'pipe'] }).trim();
  } catch (e) {
    return '';
  }
}

function getActiveBranch() {
  return run('git branch --show-current') || 'tauri';
}

function getAllBranches() {
  const raw = run('git branch --format="%(refname:short)|%(objectname)|%(committerdate:iso8601)|%(subject)"');
  if (!raw) return [];
  return raw.split('\n').filter(Boolean).map(line => {
    const [name, sha, date, subject] = line.split('|');
    return { name, sha, date, subject };
  });
}

function checkCherry(base, target) {
  const raw = run(`git cherry ${base} ${target}`);
  if (!raw) return { total: 0, applied: 0, unmerged: 0 };
  const lines = raw.split('\n').filter(Boolean);
  let applied = 0;
  let unmerged = 0;
  for (const l of lines) {
    if (l.startsWith('-')) applied++;
    else if (l.startsWith('+')) unmerged++;
  }
  return { total: lines.length, applied, unmerged };
}

function checkTouchedDirs(target) {
  const diff = run(`git diff --name-only release/developer-alpha-preview...${target}`);
  const files = diff.split('\n').filter(Boolean);
  const gpuiOnly = files.length > 0 && files.every(f => f.startsWith('rust/chord-gpui/'));
  return { filesCount: files.length, gpuiOnly };
}

function getWorktrees() {
  const raw = run('git worktree list --porcelain');
  if (!raw) return new Map();
  const map = new Map();
  let currentPath = '';
  for (const line of raw.split('\n')) {
    if (line.startsWith('worktree ')) currentPath = line.split(' ')[1];
    else if (line.startsWith('branch refs/heads/')) {
      const b = line.replace('branch refs/heads/', '');
      map.set(b, currentPath);
    }
  }
  return map;
}

function main() {
  const args = process.argv.slice(2);
  const isDryRun = args.includes('--dry-run');
  const doPruneDuplicates = args.includes('--prune-duplicates') || args.includes('--prune-stale');
  const doPruneMerged = args.includes('--prune-merged') || args.includes('--prune-stale');
  const doPruneQuarantine = args.includes('--prune-quarantine') || args.includes('--prune-stale');
  const doPruneGpui = args.includes('--prune-gpui') || args.includes('--prune-stale');

  const baseBranch = getActiveBranch();
  const allBranches = getAllBranches();

  console.log(`================================================================================`);
  console.log(`🌿 [BRANCH-AUDITOR] Automated Branch Hygiene & Validation Engine`);
  console.log(`   Base Branch: ${baseBranch} | Total Local Branches: ${allBranches.length}`);
  console.log(`================================================================================\n`);

  // 1. Group by Commit SHA to identify exact duplicates
  const shaMap = new Map();
  for (const b of allBranches) {
    if (!shaMap.has(b.sha)) shaMap.set(b.sha, []);
    shaMap.get(b.sha).push(b);
  }

  const duplicatesToDelete = [];
  const mergedToDelete = [];
  const gpuiToDelete = [];
  const quarantineToDelete = [];
  const unmergedBranches = [];

  for (const b of allBranches) {
    if (PROTECTED_BRANCHES.has(b.name)) continue;

    // Check if duplicate
    const siblings = shaMap.get(b.sha);
    if (siblings && siblings.length > 1) {
      // Pick first as canonical, mark rest as duplicate
      const canonical = siblings[0];
      if (b.name !== canonical.name) {
        duplicatesToDelete.push({ branch: b, canonicalName: canonical.name });
        continue;
      }
    }

    // Check quarantine
    if (b.name.startsWith('pr-16-')) {
      quarantineToDelete.push(b);
      continue;
    }

    // Check merged / cherry status against release/developer-alpha-preview and base
    const cherry = checkCherry('release/developer-alpha-preview', b.name);
    const aheadCount = parseInt(run(`git rev-list --count ${baseBranch}..${b.name}`) || '0', 10);

    if (aheadCount === 0 || (cherry.total > 0 && cherry.unmerged === 0)) {
      mergedToDelete.push(b);
      continue;
    }

    // Check if GPUI frozen branch
    const { gpuiOnly } = checkTouchedDirs(b.name);
    if (gpuiOnly || b.name.startsWith('Userbash/w3-') || b.name.startsWith('Userbash/w4-1')) {
      gpuiToDelete.push(b);
      continue;
    }

    unmergedBranches.push({ branch: b, ahead: aheadCount, cherry });
  }

  // Print Summary
  console.log(`📊 Audit Findings:`);
  console.log(`   • Protected Branches    : ${Array.from(PROTECTED_BRANCHES).join(', ')}`);
  console.log(`   • Redundant Duplicates  : ${duplicatesToDelete.length} branch(es)`);
  console.log(`   • Fully Integrated/Merged: ${mergedToDelete.length} branch(es)`);
  console.log(`   • Quarantined (PR #16)  : ${quarantineToDelete.length} branch(es)`);
  console.log(`   • Obsolete Frozen GPUI  : ${gpuiToDelete.length} branch(es)`);
  console.log(`   • Active Unmerged Work  : ${unmergedBranches.length} branch(es)\n`);

  if (duplicatesToDelete.length > 0) {
    console.log(`🔁 Group 1: Exact Duplicates (Pointing to same commit as another branch):`);
    for (const d of duplicatesToDelete) {
      console.log(`   - ${d.branch.name} -> redundant alias of [${d.canonicalName}] (${d.branch.sha.slice(0, 8)})`);
    }
    console.log('');
  }

  if (mergedToDelete.length > 0) {
    console.log(`✅ Group 2: Fully Integrated / Safe to Delete:`);
    for (const m of mergedToDelete) {
      console.log(`   - ${m.name} (${m.sha.slice(0, 8)}: ${m.subject.slice(0, 60)})`);
    }
    console.log('');
  }

  if (quarantineToDelete.length > 0) {
    console.log(`⚠️ Group 3: Quarantined PR #16 Artifacts:`);
    for (const q of quarantineToDelete) {
      console.log(`   - ${q.name} (${q.sha.slice(0, 8)}: ${q.subject.slice(0, 60)})`);
    }
    console.log('');
  }

  if (gpuiToDelete.length > 0) {
    console.log(`❄️ Group 4: Obsolete Frozen GPUI Branches:`);
    for (const g of gpuiToDelete) {
      console.log(`   - ${g.name} (${g.sha.slice(0, 8)}: ${g.subject.slice(0, 60)})`);
    }
    console.log('');
  }

  // Handle Pruning Actions
  const toDelete = [];
  if (doPruneDuplicates) toDelete.push(...duplicatesToDelete.map(d => d.branch.name));
  if (doPruneMerged) toDelete.push(...mergedToDelete.map(m => m.name));
  if (doPruneQuarantine) toDelete.push(...quarantineToDelete.map(q => q.name));
  if (doPruneGpui) toDelete.push(...gpuiToDelete.map(g => g.name));

  if (toDelete.length > 0) {
    const uniqueDelete = Array.from(new Set(toDelete));
    console.log(`================================================================================`);
    if (isDryRun) {
      console.log(`🔍 [DRY-RUN] The following ${uniqueDelete.length} branch(es) WOULD be deleted:`);
      uniqueDelete.forEach(b => console.log(`   git branch -D ${b}`));
    } else {
      console.log(`🧹 [PRUNE] Deleting ${uniqueDelete.length} branch(es)...`);
      const worktreeMap = getWorktrees();
      for (const b of uniqueDelete) {
        if (worktreeMap.has(b)) {
          const wtPath = worktreeMap.get(b);
          if (wtPath !== PROJECT_ROOT) {
            run(`git worktree remove --force "${wtPath}"`, true);
          }
        }
        const out = run(`git branch -D "${b}"`);
        console.log(`   🗑️  ${out || b}`);
      }
      run('git worktree prune', true);
      console.log(`\n🎉 Successfully cleaned up ${uniqueDelete.length} stale branch(es)!`);
    }
    console.log(`================================================================================`);
  } else {
    console.log(`💡 Usage Hint:`);
    console.log(`   • To preview cleanup: node scripts/branch-auditor.mjs --dry-run --prune-stale`);
    console.log(`   • To execute full cleanup: node scripts/branch-auditor.mjs --prune-stale`);
  }
}

main();
