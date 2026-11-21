#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/orchestration-auto-driver.mjs
 *
 * Автоматический драйвер оркестрации Orca и непрерывной ротации слотов воркеров.
 * Полностью исключает затраты токенов ИИ на рутинную проверку сообщений воркеров,
 * закрытие завершённых задач, освобождение терминалов, запуск следующих слотов
 * и синхронизацию Git репозиториев.
 *
 * Возможности:
 * 1. Проверяет входящие сообщения оркестрации (orca orchestration check).
 * 2. При получении worker_done немедленно освобождает терминал (worker-release).
 * 3. Автоматически переводит задачу в 'done' на канбан-доске (waves/kanban-board.json).
 * 4. Фиксирует статус COMPLETED_VERIFIED в реестре слотов (waves/reserved-all-slots.json).
 * 5. Контролирует лимит оперативной памяти (WORKER_RAM_LIMIT < 25 GB).
 * 6. Берёт следующий зарезервированный слот из пула и запускает воркера в Orca.
 * 7. При завершении задач автоматически выполняет git-sync в origin и public.
 * 8. Поддерживает фоновый режим непрерывного цикла (--watch / --daemon).
 *
 * Использование:
 *   node scripts/orchestration-auto-driver.mjs --once              # Однократная проверка и обработка
 *   node scripts/orchestration-auto-driver.mjs --status            # Показать статус слотов и памяти
 *   node scripts/orchestration-auto-driver.mjs --watch [seconds]   # Фоновый цикл (по умолчанию каждые 15с)
 */

import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { cmdCommit } from './git-sync.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const KANBAN_PATH = path.join(PROJECT_ROOT, 'waves/kanban-board.json');
const RESERVED_PATH = path.join(PROJECT_ROOT, 'waves/reserved-all-slots.json');
const RUN_ID = 'run_d3c9df2d4c8c';
const TERMINAL_HANDLE = 'term_f93a22fc-a3b7-4a72-b617-92c8df5bcaf6';
const MAX_CONCURRENT_WORKERS = 4;
const MAX_RAM_GIB = 25.0;

function execQuiet(cmd) {
  try {
    return execSync(cmd, { cwd: PROJECT_ROOT, encoding: 'utf8', stdio: 'pipe' }).trim();
  } catch (err) {
    return '';
  }
}

export function getWorkerMemoryGiB() {
  const out = execQuiet("ps -eo rss,comm | awk '/opencode|agy|cargo|rustc|node/ {s+=$1} END {printf \"%.2f\", s/1024/1024}'");
  return parseFloat(out) || 0.0;
}

export function checkOrchestrationMessages() {
  const out = execQuiet(`orca orchestration check --run ${RUN_ID} --terminal ${TERMINAL_HANDLE}`);
  if (!out || out.includes('No messages.')) {
    return [];
  }

  const messages = [];
  const lines = out.split('\n');
  let currentMsg = null;

  for (const line of lines) {
    const headerMatch = line.match(/^([a-zA-Z0-9_]+)\s+([a-zA-Z0-9_]+)\s+from\s+([a-zA-Z0-9_-]+)/);
    if (headerMatch) {
      if (currentMsg) messages.push(currentMsg);
      currentMsg = {
        msgId: headerMatch[1],
        type: headerMatch[2],
        from: headerMatch[3],
        lines: []
      };
      continue;
    }

    if (currentMsg) {
      currentMsg.lines.push(line);
      const payloadMatch = line.match(/dispatchId=([a-zA-Z0-9_]+)/);
      if (payloadMatch) currentMsg.dispatchId = payloadMatch[1];
      const taskMatch = line.match(/taskId=([a-zA-Z0-9_]+)/);
      if (taskMatch) currentMsg.taskId = taskMatch[1];
    }
  }

  if (currentMsg) messages.push(currentMsg);
  return messages;
}

export function releaseWorkerTerminal(dispatchId) {
  if (!dispatchId) return false;
  console.log(`🧹 [AUTO-DRIVER] Освобождение терминала воркера ${dispatchId}...`);
  try {
    execSync(`orca orchestration worker-release --dispatch ${dispatchId}`, { cwd: PROJECT_ROOT, stdio: 'inherit' });
    return true;
  } catch (err) {
    console.warn(`⚠️ [AUTO-DRIVER] Ошибка при освобождении ${dispatchId}: ${err.message}`);
    return false;
  }
}

export function markTaskCompleted(cardId) {
  if (!cardId) return;
  console.log(`✅ [AUTO-DRIVER] Фиксация завершения задачи [${cardId}]...`);

  // 1. Канбан-доска
  if (fs.existsSync(KANBAN_PATH)) {
    const board = JSON.parse(fs.readFileSync(KANBAN_PATH, 'utf8'));
    const card = board.cards.find(c => c.id === cardId);
    if (card) {
      card.status = 'done';
      card.column = 'done';
      card.completedAt = new Date().toISOString();
      fs.writeFileSync(KANBAN_PATH, JSON.stringify(board, null, 2));
    }
  }

  // 2. Реестр слотов
  if (fs.existsSync(RESERVED_PATH)) {
    const res = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
    const slot = res.slots.find(s => s.id === cardId);
    if (slot) {
      slot.status = 'COMPLETED_VERIFIED';
      slot.completedAt = new Date().toISOString();
      fs.writeFileSync(RESERVED_PATH, JSON.stringify(res, null, 2));
    }
  }
}

export function getActiveWorkerCount() {
  const out = execQuiet(`orca orchestration worker-list --run ${RUN_ID}`);
  const lines = out.split('\n').filter(Boolean);
  return lines.filter(l => l.includes('state=ready') || (l.includes('terminalState=active') && !l.includes('terminalState=released'))).length;
}

export function dispatchNextSlot() {
  if (!fs.existsSync(RESERVED_PATH)) return false;

  const currentMem = getWorkerMemoryGiB();
  if (currentMem >= MAX_RAM_GIB) {
    console.warn(`⚠️ [AUTO-DRIVER] Память воркеров (${currentMem} GiB) достигла лимита (${MAX_RAM_GIB} GiB). Запуск отложен.`);
    return false;
  }

  const activeCount = getActiveWorkerCount();
  if (activeCount >= MAX_CONCURRENT_WORKERS) {
    console.log(`ℹ️ [AUTO-DRIVER] Пул заполнен (${activeCount}/${MAX_CONCURRENT_WORKERS} активных воркеров).`);
    return false;
  }

  const res = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
  const nextSlot = res.slots.find(s => s.status === 'RESERVED');
  if (!nextSlot) {
    console.log('🎉 [AUTO-DRIVER] Все зарезервированные слоты уже запущены или выполнены!');
    return false;
  }

  console.log(`🚀 [AUTO-DRIVER] Запуск воркера для Слота #${nextSlot.slot} [${nextSlot.id}]: ${nextSlot.title}`);
  const specText = `Task ${nextSlot.id}: ${nextSlot.title}\nDescription: ${nextSlot.description}\nDepartment: ${nextSlot.department}\nPriority: ${nextSlot.priority}\nContract: Follow AGENTS.md, TEST-POLICY.md strictly. Write tests first.`;
  const taskTitle = `[${nextSlot.id}] ${nextSlot.title}`;

  try {
    const resSpawn = spawnSync('orca', [
      'orchestration', 'worker-start',
      '--run', RUN_ID,
      '--agent', 'antigravity',
      '--model', nextSlot.model,
      '--effort', nextSlot.effort,
      '--spec', specText,
      '--task-title', taskTitle,
      '--json'
    ], { cwd: PROJECT_ROOT, encoding: 'utf8' });

    if (resSpawn.status !== 0) {
      throw new Error(resSpawn.stderr || resSpawn.stdout || 'orca worker-start exited with non-zero code');
    }

    const out = resSpawn.stdout.trim();
    const parsed = JSON.parse(out);
    nextSlot.dispatchId = parsed.dispatchId || parsed.id || 'dispatched';
    nextSlot.status = 'DISPATCHED_WORKING';
    nextSlot.dispatchedAt = new Date().toISOString();
    fs.writeFileSync(RESERVED_PATH, JSON.stringify(res, null, 2));
    console.log(`✅ [AUTO-DRIVER] Воркер успешно запущен! DispatchId: ${nextSlot.dispatchId}`);
    return true;
  } catch (err) {
    console.error(`❌ [AUTO-DRIVER] Ошибка при запуске воркера: ${err.message}`);
    nextSlot.status = 'DISPATCH_FAILED';
    fs.writeFileSync(RESERVED_PATH, JSON.stringify(res, null, 2));
    return false;
  }
}

export function runCycle() {
  console.log('=== [ORCHESTRATION AUTO-DRIVER CYCLE] ===');
  const mem = getWorkerMemoryGiB();
  console.log(`Память воркеров: ${mem} GiB / ${MAX_RAM_GIB} GiB`);

  const messages = checkOrchestrationMessages();
  console.log(`Входящих сообщений: ${messages.length}`);

  let completedTasks = [];

  for (const msg of messages) {
    if (msg.type === 'worker_done') {
      console.log(`📩 Получен worker_done для dispatchId: ${msg.dispatchId}`);
      releaseWorkerTerminal(msg.dispatchId);

      // Ищем карточку по dispatchId в реестре
      if (fs.existsSync(RESERVED_PATH)) {
        const res = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
        const slot = res.slots.find(s => s.dispatchId === msg.dispatchId);
        if (slot) {
          markTaskCompleted(slot.id);
          completedTasks.push(slot.id);
        }
      }
    }
  }

  // Непрерывная ротация: пока есть свободные слоты до лимита
  let spawned = false;
  while (getActiveWorkerCount() < MAX_CONCURRENT_WORKERS) {
    const ok = dispatchNextSlot();
    if (ok) spawned = true;
    else break;
  }

  // Если задачи завершились, выполняем авто-коммит и пуш
  if (completedTasks.length > 0) {
    console.log(`🚀 [AUTO-DRIVER] Синхронизация Git после завершения ${completedTasks.join(', ')}...`);
    cmdCommit(`docs(kanban): auto-complete tasks ${completedTasks.join(', ')}`);
  }

  console.log('=========================================');
  return { messagesCount: messages.length, completedTasks, spawned };
}

export function runWatchMode(intervalSeconds = 15) {
  console.log(`👀 [AUTO-DRIVER WATCH] Запущен фоновый драйвер (интервал ${intervalSeconds}с)...`);
  console.log('Нажмите Ctrl+C для завершения.');

  runCycle();
  setInterval(() => {
    runCycle();
  }, intervalSeconds * 1000);
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes('--status')) {
    const mem = getWorkerMemoryGiB();
    const active = getActiveWorkerCount();
    console.log(`Status: Active workers = ${active}/${MAX_CONCURRENT_WORKERS}, RAM = ${mem} GiB`);
    return;
  }

  if (args.includes('--watch') || args.includes('--daemon')) {
    const watchIdx = args.indexOf('--watch') !== -1 ? args.indexOf('--watch') : args.indexOf('--daemon');
    const sec = parseInt(args[watchIdx + 1], 10) || 15;
    runWatchMode(sec);
    return;
  }

  runCycle();
}

if (process.argv[1] && process.argv[1].endsWith('orchestration-auto-driver.mjs')) {
  main();
}
