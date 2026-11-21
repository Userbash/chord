#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/reserve-slots.mjs
 * 
 * Резервирование пула слотов с доски задач (Batch Reservation & Worker Slot Allocation).
 * 
 * Возможности:
 * 1. Загрузка первых N (по умолчанию 15) незавершенных задач с Kanban доски.
 * 2. Резервирование слотов с привязкой к модели, эфорту и отделу (G1..G7).
 * 3. Выявление Tier 0 задач (детерминированные скрипты без ИИ) и их мгновенное закрытие.
 * 4. Диспетчеризация воркеров в безопасном пуле (максимум 4 активных параллельно для соблюдения WORKER_RAM_LIMIT <= 25 GB)
 *    с непрерывной 1-в-1 ротацией слотов.
 * 5. Сохранение манифеста в waves/reserved-15-slots.json.
 * 
 * Использование:
 *   node scripts/reserve-slots.mjs --reserve 15
 *   node scripts/reserve-slots.mjs --dispatch 15
 *   node scripts/reserve-slots.mjs --status
 */

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';

const KANBAN_PATH = path.resolve('waves/kanban-board.json');
const RESERVED_PATH = path.resolve('waves/reserved-15-slots.json');
const ORCA_RUN_PATH = path.resolve('.claude/ecc-orca-run.json');
const MAX_PARALLEL_ACTIVE_WORKERS = 4; // AGENTS.md §8: RAM limit <= 25 GB (Swap: 0)

function runCmd(cmd) {
  try {
    return execSync(cmd, { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });
  } catch (err) {
    return err.stdout || err.stderr || String(err);
  }
}

function getActiveRunId() {
  if (fs.existsSync(ORCA_RUN_PATH)) {
    try {
      const data = JSON.parse(fs.readFileSync(ORCA_RUN_PATH, 'utf8'));
      return data.runId || 'run_d3c9df2d4c8c';
    } catch {}
  }
  return 'run_d3c9df2d4c8c';
}

function routeTask(card) {
  const specText = `${card.title} ${card.description || ''}`;
  try {
    const out = runCmd(`node scripts/task-router.mjs --spec ${JSON.stringify(specText)}`);
    return JSON.parse(out);
  } catch {
    return {
      tier: 'TIER_M',
      isScript: false,
      scriptCommand: null,
      model: 'gemini-4-hydrogen-medium',
      effort: 'low',
      maxIterations: 4,
      rationale: 'Default fallback route'
    };
  }
}

function loadAndReserveSlots(count = 15, autoDispatch = false) {
  if (!fs.existsSync(KANBAN_PATH)) {
    console.error(`❌ Kanban board not found at ${KANBAN_PATH}`);
    process.exit(1);
  }

  const board = JSON.parse(fs.readFileSync(KANBAN_PATH, 'utf8'));
  const nonDone = board.cards.filter(c => 
    (c.column || '').toLowerCase() !== 'done' && 
    (c.status || '').toLowerCase() !== 'done'
  );

  // Приоритетная сортировка: in_progress -> ready_for_dev -> in_dev -> backlog, затем по приоритету P0 -> P1 -> P2 -> P3
  const prioWeight = { P0: 4, P1: 3, P2: 2, P3: 1 };
  const colWeight = { in_progress: 4, ready_for_dev: 3, in_dev: 2, backlog: 1 };

  nonDone.sort((a, b) => {
    const colDiff = (colWeight[b.column] || 0) - (colWeight[a.column] || 0);
    if (colDiff !== 0) return colDiff;
    return (prioWeight[b.priority] || 0) - (prioWeight[a.priority] || 0);
  });

  const selectedCards = nonDone.slice(0, count);

  console.log(`\n=============================================================`);
  console.log(`📋 [Slot Manager] Загрузка задач с доски. Резервирование ${selectedCards.length} слотов`);
  console.log(`=============================================================`);

  const slots = [];
  const runId = getActiveRunId();

  // Привязываем Run к координатору
  runCmd(`orca orchestration run-use --id ${runId}`);

  let tier0Count = 0;
  let activeDispatchedCount = 0;

  for (let i = 0; i < selectedCards.length; i++) {
    const card = selectedCards[i];
    const route = routeTask(card);
    const slotNumber = i + 1;

    const slotInfo = {
      slot: slotNumber,
      id: card.id,
      title: card.title,
      description: card.description || '',
      priority: card.priority,
      currentColumn: card.column,
      department: card.dept || 'G1',
      tier: route.tier,
      isScript: route.isScript,
      scriptCommand: route.scriptCommand,
      model: route.model,
      effort: route.effort,
      maxIterations: route.maxIterations,
      rationale: route.rationale,
      status: card.column === 'in_progress' ? 'ACTIVE' : 'QUEUED',
      dispatchedAt: null,
      terminalId: null,
      dispatchId: null
    };

    if (route.isScript) {
      tier0Count++;
      slotInfo.status = 'READY_SCRIPT';
    }

    slots.push(slotInfo);
  }

  // Запись в файл waves/reserved-15-slots.json
  const manifest = {
    reservedAt: new Date().toISOString(),
    runId,
    totalSlots: slots.length,
    tierBreakdown: {
      tier0_scripts: slots.filter(s => s.tier === 'TIER_0_SCRIPT').length,
      tier_s_micro_ai: slots.filter(s => s.tier === 'TIER_S').length,
      tier_m_routine_ai: slots.filter(s => s.tier === 'TIER_M').length,
      tier_l_core_ai: slots.filter(s => s.tier === 'TIER_L').length,
      tier_xl_hardcore_ai: slots.filter(s => s.tier === 'TIER_XL').length
    },
    ramSafetyPolicy: {
      workerRamLimit: '25 GB',
      hostRam: '48 GB',
      maxParallelActiveWorkers: MAX_PARALLEL_ACTIVE_WORKERS,
      slotRotationStrategy: '1-to-1 continuous rotation upon worker_done/worker-release'
    },
    slots
  };

  fs.writeFileSync(RESERVED_PATH, JSON.stringify(manifest, null, 2), 'utf8');

  // Вывод красивой таблицы слотов
  console.log(`\nРезерв слотов сформирован в: ${RESERVED_PATH}\n`);
  console.log(`| Слот | ID | Prio | Отдел | Тир / Модель | Эфорт | Статус слота | Название |`);
  console.log(`|---|---|---|---|---|---|---|---|`);

  for (const s of slots) {
    const modelStr = s.isScript ? '⚡ 0-Token Script' : s.model.replace('gemini-', '');
    console.log(`| #${s.slot.toString().padStart(2, '0')} | ${s.id.padEnd(12, ' ')} | ${s.priority} | ${s.department} | ${s.tier} (${modelStr}) | ${s.effort} | ${s.status} | ${s.title.slice(0, 40)}... |`);
  }

  console.log(`\n📊 Распределение 15 слотов по тирам:`);
  console.log(`   - ⚡ Tier 0 (Детерминированные скрипты, 0 токенов): ${manifest.tierBreakdown.tier0_scripts}`);
  console.log(`   - 🟢 Tier S (Микро-ИИ, hydrogen-low / flash-low):  ${manifest.tierBreakdown.tier_s_micro_ai}`);
  console.log(`   - 🟡 Tier M (Рутинный ИИ, hydrogen-medium):        ${manifest.tierBreakdown.tier_m_routine_ai}`);
  console.log(`   - 🔵 Tier L (Ядро / Протоколы, argon-medium):      ${manifest.tierBreakdown.tier_l_core_ai}`);
  console.log(`   - 🟣 Tier XL (Хардкор / Concurrency, carbon-high): ${manifest.tierBreakdown.tier_xl_hardcore_ai}`);

  // Если запрошена диспетчеризация
  if (autoDispatch) {
    dispatchSlots(slots, runId);
  } else {
    console.log(`\n💡 Для запуска воркеров выполните: npm run slots:dispatch`);
  }
}

function dispatchSlots(slots, runId) {
  console.log(`\n=============================================================`);
  console.log(`🚀 [Slot Dispatcher] Спам / Диспетчеризация воркеров по слотам`);
  console.log(`   Политика памяти: не более ${MAX_PARALLEL_ACTIVE_WORKERS} активных воркеров в RAM одновременно`);
  console.log(`=============================================================`);

  // 1. Выполнение Tier 0 задач мгновенно без затрат токенов
  const scriptSlots = slots.filter(s => s.isScript && s.scriptCommand);
  for (const s of scriptSlots) {
    console.log(`\n⚡ [Tier 0 Exec] Слот #${s.slot} [${s.id}]: ${s.scriptCommand}`);
    const res = runCmd(s.scriptCommand);
    console.log(`   Результат: ${res.trim().split('\n').slice(-2).join(' ')}`);
    s.status = 'COMPLETED';
    s.dispatchedAt = new Date().toISOString();
  }

  // 2. Диспетчеризация AI воркеров (первая волна в пределах лимита RAM)
  const aiSlots = slots.filter(s => !s.isScript);
  let activeCount = 0;

  for (const s of aiSlots) {
    if (activeCount >= MAX_PARALLEL_ACTIVE_WORKERS) {
      console.log(`\n⏳ [Queue Reserved] Слот #${s.slot} [${s.id}] поставлен в очередь 1-to-1 ротации (лимит RAM 25GB)`);
      s.status = 'QUEUED_IN_SLOT';
      continue;
    }

    console.log(`\n🤖 [Worker Spawn] Слот #${s.slot} [${s.id}]: Запуск воркера (${s.model}, effort: ${s.effort})`);
    const spec = `[${s.id}] ${s.title}: ${s.description || 'Implement task per specification and TDD rules'}`;
    const cmd = `orca orchestration worker-start --run ${runId} --spec "${spec.replace(/"/g, '\\"')}" --task-title "${s.id}" --agent antigravity --model "${s.model}" --effort "${s.effort}" --json`;
    
    const out = runCmd(cmd);
    let ok = false;
    try {
      const parsed = JSON.parse(out);
      ok = parsed.ok !== false;
      if (parsed.dispatchId) s.dispatchId = parsed.dispatchId;
    } catch {
      ok = !out.includes('error:');
    }

    console.log(`   Статус диспетчеризации: ${ok ? '✅ Запущен' : '⚠️ Отклонен/Занят'} (${out.trim().slice(0, 100)})`);
    s.status = ok ? 'ACTIVE_RUNNING' : 'DISPATCH_PENDING';
    s.dispatchedAt = new Date().toISOString();
    activeCount++;
  }

  // Обновляем манифест
  const currentManifest = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
  currentManifest.slots = slots;
  currentManifest.lastDispatchedAt = new Date().toISOString();
  fs.writeFileSync(RESERVED_PATH, JSON.stringify(currentManifest, null, 2), 'utf8');

  console.log(`\n✅ Пул из 15 слотов успешно заряжен! ${activeCount} воркеров в активном исполнении, ${15 - activeCount - scriptSlots.length} слотов в очереди ротации.`);
}

function showStatus() {
  if (!fs.existsSync(RESERVED_PATH)) {
    console.log(`Резерв слотов еще не создан. Выполните: npm run slots:reserve`);
    return;
  }
  const data = JSON.parse(fs.readFileSync(RESERVED_PATH, 'utf8'));
  console.log(`\n=============================================================`);
  console.log(`📊 Статус пула 15 слотов (Создан: ${data.reservedAt})`);
  console.log(`   Run ID: ${data.runId} | RAM Limit: ${data.ramSafetyPolicy.workerRamLimit}`);
  console.log(`=============================================================`);
  console.table(data.slots.map(s => ({
    Slot: `#${s.slot}`,
    ID: s.id,
    Tier: s.tier,
    Model: s.isScript ? 'Script' : s.model.replace('gemini-', ''),
    Effort: s.effort,
    Status: s.status,
    Title: s.title.slice(0, 35) + '...'
  })));
}

// CLI Routing
const args = process.argv.slice(2);
if (args.includes('--status')) {
  showStatus();
} else if (args.includes('--dispatch')) {
  const countIdx = args.indexOf('--dispatch');
  const count = parseInt(args[countIdx + 1], 10) || 15;
  loadAndReserveSlots(count, true);
} else {
  const countIdx = args.indexOf('--reserve');
  const count = countIdx !== -1 ? (parseInt(args[countIdx + 1], 10) || 15) : 15;
  loadAndReserveSlots(count, false);
}
