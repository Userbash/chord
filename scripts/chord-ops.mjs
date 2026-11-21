#!/usr/bin/env node
// @ts-nocheck
/**
 * chord-ops.mjs — Universal Operations & Intent Dispatcher
 *
 * Intercepts common repetitive user requests and routes them to zero-token scripts.
 * Supports natural language prompts in Russian and English.
 *
 * Examples:
 *   node scripts/chord-ops.mjs "обнови доску"
 *   node scripts/chord-ops.mjs "запусти воркеров"
 *   node scripts/chord-ops.mjs "статус"
 *   node scripts/chord-ops.mjs "очисти память"
 *   node scripts/chord-ops.mjs "проверь тесты"
 *   node scripts/chord-ops.mjs "проанализируй монолит"
 *   node scripts/chord-ops.mjs "сделай шаг конвейера"
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { showStatus, step, harvestSettledWorkers, dispatchNextTask, watch } from './auto-pipeline.mjs';
import { runGC } from './worker-gc.mjs';
import { inspectMonolith } from './monolith-inspector.mjs';
import { runTests, getChangedFiles } from './test-runner-smart.mjs';
import { parseKanban, findKanbanPath, getStats } from './kanban-tool.mjs';
import { fetchPrDetails, auditPr, generateMarkdownReport } from './pr-ops.mjs';

const INTENT_PATTERNS = [
  // 0. Pull Request Automation & Audit
  {
    regex: /(пиар|pull\s*request|\bpr\b)/i,
    action: (rawInput) => {
      const m = rawInput.match(/\d+/);
      const prNum = m ? parseInt(m[0], 10) : 16;
      console.log(`\n🔍 Analyzing PR #${prNum} against collaborative development rules...`);
      const details = fetchPrDetails(prNum);
      const res = auditPr(details);
      console.log('\n' + generateMarkdownReport(res));
    },
  },
  // 1. Status & Dashboard
  {
    regex: /(статус|проверь|как дела|дашборд|dashboard|status|check|state)/i,
    action: () => showStatus(),
  },
  // 2. Kanban Board
  {
    regex: /(доск|канбан|kanban|board)/i,
    action: () => {
      const p = findKanbanPath();
      const { tasks } = parseKanban(p);
      const s = getStats(tasks);
      console.log(`\n📋 KANBAN SUMMARY: ${s.done}/${s.total} (${s.percent}%) done | ${s.inProgress} active | ${s.queued} queued`);
    },
  },
  // 3. Spawning / Dispatching Workers
  {
    regex: /(запусти|заспам|воркер|загрузи|spawn|dispatch|worker|next|start)/i,
    action: () => dispatchNextTask(),
  },
  // 4. Memory / GC / Process cleanup
  {
    regex: /(памят|очист|убей|reclaim|gc|cleanup|kill|memory|ram)/i,
    action: () => runGC(),
  },
  // 5. Testing
  {
    regex: /(тест|проверь код|прогон|test|verify|check-code)/i,
    action: () => {
      const files = getChangedFiles();
      runTests(files);
    },
  },
  // 6. Monolith Inspection
  {
    regex: /(монолит|функци|инвентар|разбор|monolith|functions|inspect)/i,
    action: () => {
      const rep = inspectMonolith();
      console.log(`\n🏛️  Monolith Status:`);
      console.log(`   data.rs: ${rep.rust.lines} lines, ${rep.rust.totalFunctions} functions`);
      console.log(`   app.js:  ${rep.js.lines} lines, ${rep.js.totalFunctions} functions`);
    },
  },
  // 7. Full step / Pipeline cycle
  {
    regex: /(шаг|цикл|авто|конвейер|step|cycle|harvest|pipeline)/i,
    action: () => step(),
  },
];

async function main() {
  const rawInput = process.argv.slice(2).join(' ').trim();
  if (!rawInput) {
    console.log(`\n🤖 CHORD-OPS — Zero-Token Smart Operator`);
    console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
    console.log(`Supported requests (in Russian or English):`);
    console.log(`  • "статус" / "status"              -> Show active workers, RAM, Kanban`);
    console.log(`  • "обнови доску" / "kanban"         -> Show/update Kanban board`);
    console.log(`  • "запусти воркеров" / "dispatch"  -> Spawn next queued task`);
    console.log(`  • "очисти память" / "gc"           -> Reclaim terminals & kill stale build processes`);
    console.log(`  • "проверь тесты" / "test"         -> Run smart targeted tests on changed files`);
    console.log(`  • "монолит" / "inspect"            -> Show function extraction stats from data.rs/app.js`);
    console.log(`  • "шаг" / "step"                   -> Harvest settled -> Test -> Commit -> Dispatch`);
    console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n`);
    return;
  }

  for (const { regex, action } of INTENT_PATTERNS) {
    if (regex.test(rawInput)) {
      await action(rawInput);
      return;
    }
  }

  console.log(`⚠️  Intent not matched automatically for "${rawInput}". Running default status check:`);
  showStatus();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
