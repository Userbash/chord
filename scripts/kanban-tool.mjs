#!/usr/bin/env node
// @ts-nocheck
/**
 * kanban-tool.mjs — Automated Kanban Board Manager for discord-lab
 *
 * Eliminates repetitive manual python regex replacements and token waste.
 * Parses and updates Markdown tables, extracts task metadata and instructions.
 *
 * Usage:
 *   node scripts/kanban-tool.mjs status
 *   node scripts/kanban-tool.mjs list
 *   node scripts/kanban-tool.mjs next
 *   node scripts/kanban-tool.mjs get <TASK_ID>
 *   node scripts/kanban-tool.mjs update <TASK_ID> <done|in_progress|queued|failed> [extra_info]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

// Default path to kanban board in brain directory or project waves
const DEFAULT_KANBAN_PATHS = [
  path.join('/home/sanya/.gemini/antigravity-cli/brain/44033f2e-f1dd-4c11-807d-19cc2fcbf29f/kanban-board.md'),
  path.join(PROJECT_ROOT, 'waves/RW-51/KANBAN.md'),
  path.join(PROJECT_ROOT, 'KANBAN.md'),
];

export function findKanbanPath(customPath) {
  if (customPath && fs.existsSync(customPath)) return customPath;
  for (const p of DEFAULT_KANBAN_PATHS) {
    if (fs.existsSync(p)) return p;
  }
  return DEFAULT_KANBAN_PATHS[0];
}

export function parseKanban(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Kanban file not found: ${filePath}`);
  }

  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');

  let inTable = false;
  let headerFound = false;
  const tasks = [];
  const tableLineIndices = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (line.includes('| Slot # | Worker Name | Dept | Model & Effort | Assigned Task | Scope | Status |')) {
      inTable = true;
      headerFound = true;
      continue;
    }

    if (inTable) {
      if (line.startsWith('|---') || line.startsWith('|:---')) {
        continue;
      }
      if (!line.startsWith('|')) {
        inTable = false;
        continue;
      }

      // Parse row: | **1** | `g1_backend_specta` | G1 Backend | `gemini-4-argon-medium` (`medium`) | `RW51-EPIC1-T1` | `rust/gui-spike/src/bindings.rs` | 🚀 **IN PROGRESS** |
      const parts = line.split('|').map((s) => s.trim()).filter(Boolean);
      if (parts.length >= 7) {
        const slot = parts[0].replace(/\*/g, '');
        const worker = parts[1].replace(/`/g, '');
        const dept = parts[2];
        const modelEffort = parts[3];
        const taskId = parts[4].replace(/`/g, '');
        const scope = parts[5].replace(/`/g, '');
        const rawStatus = parts[6];

        let status = 'UNKNOWN';
        let commitOrCtx = '';

        if (rawStatus.includes('DONE')) {
          status = 'DONE';
          const m = rawStatus.match(/\(([^)]+)\)/);
          if (m) commitOrCtx = m[1].replace(/`/g, '');
        } else if (rawStatus.includes('IN PROGRESS')) {
          status = 'IN_PROGRESS';
          const m = rawStatus.match(/\(([^)]+)\)/);
          if (m) commitOrCtx = m[1].replace(/`/g, '');
        } else if (rawStatus.includes('QUEUED')) {
          status = 'QUEUED';
        } else if (rawStatus.includes('FAILED')) {
          status = 'FAILED';
        }

        tasks.push({
          slot,
          worker,
          dept,
          modelEffort,
          taskId,
          scope,
          rawStatus,
          status,
          commitOrCtx,
          lineIndex: i,
        });
        tableLineIndices.push(i);
      }
    }
  }

  // Also parse detailed instructions/specs for each task
  for (const t of tasks) {
    const taskHeaderRegex = new RegExp(`####\\s+\`${t.taskId}\`:(.*)`);
    const taskHeaderIdx = lines.findIndex((l) => taskHeaderRegex.test(l));
    if (taskHeaderIdx !== -1) {
      const titleMatch = lines[taskHeaderIdx].match(taskHeaderRegex);
      t.title = titleMatch ? titleMatch[1].trim() : '';

      let descLines = [];
      for (let j = taskHeaderIdx + 1; j < lines.length; j++) {
        if (lines[j].startsWith('#### ') || lines[j].startsWith('### ') || lines[j].startsWith('---')) break;
        descLines.push(lines[j]);
      }
      t.instructions = descLines.join('\n').trim();

      // Extract model, effort, annotation, step-by-step
      const modelMatch = t.instructions.match(/- \*\*Model \/ Effort:\*\*\s+`?([^`\n]+)`?/);
      if (modelMatch) {
        const full = modelMatch[1];
        const parts = full.split('(');
        t.modelName = parts[0].trim().replace(/`/g, '');
        t.effort = parts[1] ? parts[1].replace(/[()`]/g, '').trim() : 'medium';
      }

      const annotMatch = t.instructions.match(/- \*\*Annotation:\*\*\s+(.*)/);
      if (annotMatch) t.annotation = annotMatch[1].trim();
    }
  }

  return { tasks, content, lines, filePath };
}

export function updateTaskStatus(filePath, taskId, newStatus, extra = '') {
  const { tasks, lines } = parseKanban(filePath);
  const target = tasks.find((t) => t.taskId.toLowerCase() === taskId.toLowerCase());
  if (!target) {
    throw new Error(`Task ${taskId} not found in kanban board.`);
  }

  let formattedStatus = '';
  switch (newStatus.toLowerCase()) {
    case 'done':
      formattedStatus = extra ? `✅ **DONE** (\`${extra}\`)` : `✅ **DONE**`;
      break;
    case 'in_progress':
      formattedStatus = extra ? `🚀 **IN PROGRESS** (\`${extra}\`)` : `🚀 **IN PROGRESS**`;
      break;
    case 'queued':
      formattedStatus = `⏳ **QUEUED**`;
      break;
    case 'failed':
      formattedStatus = extra ? `❌ **FAILED** (${extra})` : `❌ **FAILED**`;
      break;
    default:
      formattedStatus = newStatus;
  }

  const oldLine = lines[target.lineIndex];
  const parts = oldLine.split('|');
  // parts[0] is empty, parts[1] is slot, ..., parts[7] is status, parts[8] is empty
  if (parts.length >= 8) {
    parts[7] = ` ${formattedStatus} `;
    lines[target.lineIndex] = parts.join('|');
    fs.writeFileSync(filePath, lines.join('\n'), 'utf8');
    return { success: true, taskId, oldStatus: target.status, newStatus, line: lines[target.lineIndex] };
  } else {
    throw new Error(`Failed to parse table line: ${oldLine}`);
  }
}

export function getStats(tasks) {
  const total = tasks.length;
  const done = tasks.filter((t) => t.status === 'DONE').length;
  const inProgress = tasks.filter((t) => t.status === 'IN_PROGRESS').length;
  const queued = tasks.filter((t) => t.status === 'QUEUED').length;
  const failed = tasks.filter((t) => t.status === 'FAILED').length;
  const percent = total > 0 ? Math.round((done / total) * 100) : 0;

  return { total, done, inProgress, queued, failed, percent };
}

// CLI handler
async function main() {
  const args = process.argv.slice(2);
  const cmd = args[0] || 'status';
  const kanbanPath = findKanbanPath(process.env.KANBAN_PATH);

  try {
    const { tasks, filePath } = parseKanban(kanbanPath);

    switch (cmd) {
      case 'status':
      case 'stats': {
        const stats = getStats(tasks);
        console.log(`\n📋 KANBAN BOARD STATUS (${path.basename(filePath)})`);
        console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
        console.log(`  Total Tasks:       ${stats.total}`);
        console.log(`  ✅ Done:           ${stats.done} (${stats.percent}%)`);
        console.log(`  🚀 In Progress:    ${stats.inProgress}`);
        console.log(`  ⏳ Queued:         ${stats.queued}`);
        console.log(`  ❌ Failed:         ${stats.failed}`);
        console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n`);
        break;
      }

      case 'list': {
        console.log(`\n📋 KANBAN TASKS LIST:`);
        for (const t of tasks) {
          const badge =
            t.status === 'DONE'
              ? '✅ DONE'
              : t.status === 'IN_PROGRESS'
              ? '🚀 RUNNING'
              : t.status === 'QUEUED'
              ? '⏳ QUEUED'
              : '❓ ' + t.status;
          const extra = t.commitOrCtx ? ` [${t.commitOrCtx}]` : '';
          console.log(`  ${t.slot.padStart(2)}. ${t.taskId.padEnd(16)} | ${t.worker.padEnd(20)} | ${badge.padEnd(12)}${extra} | ${t.scope}`);
        }
        console.log();
        break;
      }

      case 'next': {
        const nextTask = tasks.find((t) => t.status === 'QUEUED');
        if (!nextTask) {
          console.log(JSON.stringify({ found: false, message: 'All tasks completed or in progress!' }, null, 2));
        } else {
          console.log(JSON.stringify({ found: true, task: nextTask }, null, 2));
        }
        break;
      }

      case 'get': {
        const taskId = args[1];
        if (!taskId) {
          console.error('Error: specify TASK_ID (e.g. RW51-EPIC1-T3)');
          process.exit(1);
        }
        const task = tasks.find((t) => t.taskId.toLowerCase() === taskId.toLowerCase());
        if (!task) {
          console.error(`Task ${taskId} not found.`);
          process.exit(1);
        }
        console.log(JSON.stringify(task, null, 2));
        break;
      }

      case 'update': {
        const taskId = args[1];
        const status = args[2];
        const extra = args[3] || '';
        if (!taskId || !status) {
          console.error('Usage: kanban-tool.mjs update <TASK_ID> <done|in_progress|queued|failed> [extra]');
          process.exit(1);
        }
        const res = updateTaskStatus(filePath, taskId, status, extra);
        console.log(`Updated ${res.taskId} -> ${res.newStatus} (extra: ${extra || 'none'})`);
        break;
      }

      default:
        console.log('Available commands: status, list, next, get <TASK_ID>, update <TASK_ID> <status> [extra]');
    }
  } catch (err) {
    console.error('Error:', err.message);
    process.exit(1);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
