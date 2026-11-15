#!/usr/bin/env node
// @ts-nocheck
/**
 * worker-gc.mjs — Worker Terminal & Memory Garbage Collector
 *
 * Automatically frees memory, kills orphaned processes, and reclaims Orca terminals.
 * Eliminates repetitive manual commands:
 *   "for ctx in $(orca worker-list | awk ...); do orca worker-release; done"
 *   "ps aux | grep cargo | kill ..."
 *
 * Usage:
 *   node scripts/worker-gc.mjs
 *   node scripts/worker-gc.mjs --dry-run
 */

import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const RUN_STATE_FILE = path.join(PROJECT_ROOT, '.claude/ecc-orca-run.json');
const ORCA_BIN = process.env.ORCA_BIN || '/home/sanya/.config/orca/linux-orca-cli-shim/orca';

export function runGC(dryRun = false) {
  console.log(`\n🧹 [WORKER-GC] Starting garbage collection & memory optimization...`);

  // 1. Determine active runId
  let runId = 'run_31999544da31';
  try {
    if (fs.existsSync(RUN_STATE_FILE)) {
      const data = JSON.parse(fs.readFileSync(RUN_STATE_FILE, 'utf8'));
      if (data.runId) runId = data.runId;
    }
  } catch {}

  // 2. Scan and release reclaimable Orca terminals
  let releasedCount = 0;
  try {
    const listOut = execSync(`${ORCA_BIN} orchestration worker-list --run ${runId}`, {
      cwd: PROJECT_ROOT,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'],
    });

    const lines = listOut.split('\n');
    for (const l of lines) {
      if (l.includes('terminal=reclaimable') || (l.includes('terminalState=retained') && (l.includes('state=succeeded') || l.includes('state=stopped')))) {
        const m = l.match(/dispatchId=([^\s]+)/);
        if (m && m[1]) {
          const dispatchId = m[1];
          console.log(`   Found reclaimable dispatch: ${dispatchId}`);
          if (!dryRun) {
            try {
              execSync(`${ORCA_BIN} orchestration worker-release --dispatch ${dispatchId}`, {
                cwd: PROJECT_ROOT,
                stdio: 'ignore',
              });
              releasedCount++;
              console.log(`   ✅ Released: ${dispatchId}`);
            } catch (err) {
              console.warn(`   ⚠️  Could not release ${dispatchId}: ${err.message}`);
            }
          } else {
            releasedCount++;
            console.log(`   [DRY-RUN] Would release: ${dispatchId}`);
          }
        }
      }
    }
  } catch (err) {
    console.warn(`   ⚠️  Orca worker-list failed:`, err.message);
  }

  // 3. Scan for zombie/orphaned build processes
  let killedZombies = 0;
  try {
    const psOut = execSync('ps -eo pid,ppid,etimes,comm,args', { encoding: 'utf8' });
    const pLines = psOut.split('\n');
    for (const pl of pLines) {
      // Find cargo or rustc running for > 3600 seconds (1 hour) or orphaned (ppid 1)
      if (/(cargo|rustc)/.test(pl) && !/worker-gc/.test(pl)) {
        const parts = pl.trim().split(/\s+/);
        const pid = parseInt(parts[0], 10);
        const ppid = parseInt(parts[1], 10);
        const etimes = parseInt(parts[2], 10);

        if (etimes > 3600 || ppid === 1) {
          console.log(`   Found orphaned/stale compiler process PID ${pid} (${parts[3]}, uptime: ${etimes}s)`);
          if (!dryRun) {
            try {
              process.kill(pid, 'SIGTERM');
              killedZombies++;
              console.log(`   ✅ Terminated PID ${pid}`);
            } catch {}
          } else {
            killedZombies++;
            console.log(`   [DRY-RUN] Would terminate PID ${pid}`);
          }
        }
      }
    }
  } catch (err) {
    console.warn(`   ⚠️  Process scan failed:`, err.message);
  }

  // 4. Memory summary after GC
  let memAvailableMb = 0;
  try {
    const meminfo = fs.readFileSync('/proc/meminfo', 'utf8');
    const m = meminfo.match(/MemAvailable:\s+(\d+)\s+kB/);
    if (m && m[1]) memAvailableMb = Math.round(parseInt(m[1], 10) / 1024);
  } catch {}

  let agentRssMb = 0;
  try {
    const rssOut = execSync('ps -eo rss,comm', { encoding: 'utf8' });
    for (const l of rssOut.split('\n')) {
      if (/antigravity|agy|orca|cargo|rustc|node/.test(l)) {
        const parts = l.trim().split(/\s+/);
        if (parts[0]) agentRssMb += parseInt(parts[0], 10);
      }
    }
    agentRssMb = Math.round(agentRssMb / 1024);
  } catch {}

  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  console.log(`🎉 GC Complete: ${releasedCount} terminals reclaimed, ${killedZombies} stale processes cleaned.`);
  console.log(`💾 Memory: Agent RSS: ${agentRssMb}MB | System MemAvailable: ${memAvailableMb}MB`);
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n`);

  return { releasedCount, killedZombies, agentRssMb, memAvailableMb };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const dryRun = process.argv.includes('--dry-run');
  runGC(dryRun);
}
