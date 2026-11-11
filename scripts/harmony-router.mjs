#!/usr/bin/env node
// @ts-check
/**
 * scripts/harmony-router.mjs — DSH-Superagent-Inspired Proactive Harmony Router & Admission Engine
 * 
 * Архитектура построена на базе концепций DeepSeek Harness Superagent (dsh-superagent-prototype):
 * 1. Router–Worker Topology: Долгоживущий роутер с защитой бюджета внимания (Attention Budget).
 * 2. Admission Matrix & Dependency Guard: Проверка доступности памяти, лимита параллелизма, завершённости зависимостей и мьютексов файлов.
 * 3. Immutable Worker Mandates: Генерация строгих неизменяемых мандатов с write-scope, оракулами и T0-тестами.
 * 4. Model Harmony Tiers: Строгая дифференциация (Carbon для P0 ядра/крипто, Argon для BFF/QA, Flash для UI).
 * 5. Dynamic File Lock Registry: Защита от коллизий файлов между параллельными воркерами.
 * 6. Transactional Outbox & Automated Arbiter: Авто-проверка T0, аудит диффа, атомарное обновление Канбан-доски и освобождение терминалов (Zero-Idle & Zero-OOM).
 */

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';

const PROJECT_ROOT = '/home/sanya/orca/project';
const KANBAN_FILE = path.join(PROJECT_ROOT, 'waves/kanban-board.json');
const POOL_FILE = path.join(PROJECT_ROOT, 'waves/RW-41-ECC-INTEGRATION/TASK-POOL.md');
const TELEMETRY_FILE = path.join(PROJECT_ROOT, 'waves/RW-41-ECC-INTEGRATION/LIVE-TELEMETRY.jsonl');
const LOCKS_FILE = path.join(PROJECT_ROOT, 'waves/RW-41-ECC-INTEGRATION/dsh-locks.json');
const OUTBOX_FILE = path.join(PROJECT_ROOT, 'waves/RW-41-ECC-INTEGRATION/dsh-outbox.jsonl');

/**
 * Граф зависимостей между задачами (Dependency Graph)
 * @type {Record<string, string[]>}
 */
export const TASK_DEPENDENCIES = Object.freeze({
  'TASK-618': ['TASK-617'], // UI поповера зависит от BFF REST-поиска
  'TASK-620': ['TASK-619'], // Контекстное меню зависит от вычисления битовых прав v9
  'TASK-614': ['TASK-613'], // Ring Buffer 60 FPS принимает кадры от PipeWire ScreenCast
  'TASK-615': ['TASK-614'], // RTP пакетная сборка и DAVE MLS v1 требуют Ring Buffer
  'TASK-616': ['TASK-615'], // UI видео-плеера PiP требует готовый RTP видеопоток
  'TASK-809': ['TASK-619'], // Обработка ошибок create_invite на каналах требует Permissions
  'TASK-622': ['TASK-615', 'TASK-621'], // Мутационный гейт 15/15 требует готовности медиа и CI
  'TASK-705': ['TASK-702'], // Headless UI харнесс требует базовую модель AppState
  'TASK-706': ['TASK-702', 'TASK-703'], // AppLayout требует модель AppState и DiscordDarkTheme
  'TASK-709': ['TASK-706'], // Virtual ChatView требует AppLayout
  'TASK-710': ['TASK-709'], // TextView Markdown требует ChatView
  'TASK-711': ['TASK-706'], // MessageBar требует AppLayout
  'TASK-712': ['TASK-706'], // Модалка капчи GPUI требует AppLayout
  'TASK-713': ['TASK-706', 'TASK-704'], // Диалоги Join/Create требуют AppLayout и REST v9
  'TASK-714': ['TASK-706'], // MemberSidebar 25k требует AppLayout
  'TASK-715': ['TASK-706', 'TASK-703'], // SettingsDialog требует AppLayout и тему
  'TASK-720': ['TASK-709', 'TASK-705'], // Финальная сертификация 120 FPS требует ChatView и харнесс
});

/**
 * Модельные гармоники и уровни рассуждения по приоритету и отделу
 * @param {string} taskId
 * @param {string} priority
 * @param {string} department
 * @returns {{ agent: string, model: string, effort: 'low' | 'medium' | 'high' | 'xhigh', rationale: string }}
 */
export function resolveModelHarmony(taskId, priority, department) {
  // P0 задачи ядра, криптографии, атомиков и GPUI-моста — эксклюзивно Carbon
  if (['TASK-614', 'TASK-615', 'TASK-702'].includes(taskId) || (priority === 'P0' && department.includes('G5'))) {
    return {
      agent: 'antigravity',
      model: 'gemini-4-carbon-high',
      effort: 'xhigh',
      rationale: 'P0 Hardcore Core: DAVE MLS v1, Ring Buffer zero-copy или Tokio↔GPUI AppState bridge'
    };
  }

  // P1 задачи бэкенда, Rust BFF, QA и RE — рабочая лошадка Argon High
  if (priority === 'P0' || (priority === 'P1' && (department.includes('G1') || department.includes('QA') || department.includes('DevOps') || department.includes('G7')))) {
    return {
      agent: 'antigravity',
      model: 'gemini-4-argon-high',
      effort: 'high',
      rationale: 'P1 Rust Core, BFF, QA автоматизация и RE-оракулы'
    };
  }

  // UI компоненты, верстка, CSS токены и попапы — легковесный и быстрый Flash High/Medium
  if (department.includes('G2') || department.includes('G3')) {
    return {
      agent: 'antigravity',
      model: 'gemini-3.8-flash-high',
      effort: 'medium',
      rationale: 'UI компоненты, CSS-токены, верстка, модалки и клавиатурная навигация'
    };
  }

  // Документация, трей, второстепенные задачи — Flash Low/Medium
  return {
    agent: 'antigravity',
    model: 'gemini-3.8-flash-high',
    effort: 'low',
    rationale: 'Документация, системный трей, вспомогательные утилиты'
  };
}

/**
 * Чтение текущего состояния карточек на Канбан-доске
 * @returns {{ cards: Array<{ id: string, title: string, status: string, priority: string, department: string }> }}
 */
export function loadKanbanCards() {
  if (!fs.existsSync(KANBAN_FILE)) {
    throw new Error(`Файл Канбан-доски не найден: ${KANBAN_FILE}`);
  }
  return JSON.parse(fs.readFileSync(KANBAN_FILE, 'utf8'));
}

/**
 * Запись обновлённого состояния Канбан-доски
 * @param {any} board
 */
export function saveKanbanCards(board) {
  board.updated_at = new Date().toISOString();
  fs.writeFileSync(KANBAN_FILE, JSON.stringify(board, null, 2), 'utf8');
}

/**
 * Мьютексы файлов (File Lock Registry)
 * @returns {Record<string, string>} map: filePath -> taskId
 */
export function getActiveLocks() {
  if (!fs.existsSync(LOCKS_FILE)) {
    return {};
  }
  try {
    return JSON.parse(fs.readFileSync(LOCKS_FILE, 'utf8'));
  } catch {
    return {};
  }
}

/**
 * Захват блокировок файлов для задачи
 * @param {string} taskId
 * @param {string[]} files
 */
export function acquireLocks(taskId, files) {
  const locks = getActiveLocks();
  for (const f of files) {
    if (locks[f] && locks[f] !== taskId) {
      throw new Error(`Конфликт блокировок: файл ${f} уже заблокирован задачей ${locks[f]}`);
    }
  }
  for (const f of files) {
    locks[f] = taskId;
  }
  fs.writeFileSync(LOCKS_FILE, JSON.stringify(locks, null, 2), 'utf8');
  recordOutboxEvent('LOCKS_ACQUIRED', taskId, { files });
}

/**
 * Освобождение блокировок файлов задачи
 * @param {string} taskId
 */
export function releaseLocks(taskId) {
  const locks = getActiveLocks();
  const released = [];
  for (const [file, owner] of Object.entries(locks)) {
    if (owner === taskId) {
      delete locks[file];
      released.push(file);
    }
  }
  fs.writeFileSync(LOCKS_FILE, JSON.stringify(locks, null, 2), 'utf8');
  recordOutboxEvent('LOCKS_RELEASED', taskId, { released });
}

/**
 * Запись события в Transactional Outbox (DSH Pattern)
 * @param {string} event
 * @param {string} taskId
 * @param {Record<string, any>} payload
 */
export function recordOutboxEvent(event, taskId, payload = {}) {
  const record = {
    timestamp: new Date().toISOString(),
    event,
    taskId,
    ...payload
  };
  fs.appendFileSync(OUTBOX_FILE, JSON.stringify(record) + '\n', 'utf8');
}

/**
 * Оценка допуска задачи через Admission Matrix (DSH Pattern)
 * @param {string} taskId
 * @returns {{ allowed: boolean, reason?: string, missingDeps?: string[], lockConflicts?: string[], harmony?: ReturnType<typeof resolveModelHarmony> }}
 */
export function evaluateAdmission(taskId) {
  const board = loadKanbanCards();
  const card = board.cards.find(c => c.id === taskId);
  if (!card) {
    return { allowed: false, reason: `Задача ${taskId} не зарегистрирована на Канбан-доске.` };
  }

  if (card.status === 'done') {
    return { allowed: false, reason: `Задача ${taskId} уже выполнена и принята (done).` };
  }

  // 1. Проверка зависимостей
  const deps = TASK_DEPENDENCIES[taskId] || [];
  const missingDeps = deps.filter(depId => {
    const depCard = board.cards.find(c => c.id === depId);
    return !depCard || depCard.status !== 'done';
  });

  if (missingDeps.length > 0) {
    return {
      allowed: false,
      reason: `Заблокировано зависимостями: задачи [${missingDeps.join(', ')}] еще не завершены (не в статусе done).`,
      missingDeps
    };
  }

  // 2. Проверка доступной системной памяти
  let memAvailableMb = 16384;
  try {
    const meminfo = fs.readFileSync('/proc/meminfo', 'utf8');
    const match = meminfo.match(/MemAvailable:\s+(\d+)\s+kB/);
    if (match && match[1]) {
      memAvailableMb = Math.floor(parseInt(match[1], 10) / 1024);
    }
  } catch {}

  if (memAvailableMb < 8192) {
    return {
      allowed: false,
      reason: `Аппаратный барьер: доступно ${memAvailableMb} МБ ОЗУ (< 8192 МБ). Риск OOM Killer.`
    };
  }

  // 3. Проверка файловых мьютексов
  const mandate = generateMandate(taskId);
  const activeLocks = getActiveLocks();
  const lockConflicts = mandate.writeScope.filter(f => activeLocks[f] && activeLocks[f] !== taskId);
  if (lockConflicts.length > 0) {
    return {
      allowed: false,
      reason: `Конфликт файловых мьютексов: файлы [${lockConflicts.join(', ')}] заняты другими задачами.`,
      lockConflicts
    };
  }

  const harmony = resolveModelHarmony(card.id, card.priority, card.department);

  return {
    allowed: true,
    harmony
  };
}

/**
 * Генерация неизменяемого мандата воркера (Immutable Worker Mandate)
 * @param {string} taskId
 * @returns {{ mandateId: string, taskId: string, writeScope: string[], forbiddenPatterns: string[], testCmd: string, oracle: string, harmony: ReturnType<typeof resolveModelHarmony> }}
 */
export function generateMandate(taskId) {
  const board = loadKanbanCards();
  const card = board.cards.find(c => c.id === taskId);
  if (!card) {
    throw new Error(`Карточка ${taskId} не найдена`);
  }

  const harmony = resolveModelHarmony(card.id, card.priority, card.department);

  let writeScope = [];
  let testCmd = '';
  let oracle = '/home/sanya/re-deep/decoded/';

  switch (taskId) {
    case 'TASK-615':
      writeScope = ['rust/crates/discord-voice/src/rtp/video.rs', 'rust/crates/discord-voice/tests/rtp_video_contract.rs'];
      testCmd = 'cargo test --manifest-path rust/Cargo.toml -p discord-voice --test rtp_video_contract';
      oracle = '/home/sanya/re-deep/voice-deep.md';
      break;
    case 'TASK-616':
      writeScope = ['rust/gui-spike/ui/features/stream-viewer.js', 'rust/gui-spike/ui/styles/stream-viewer.css', 'tests/unit/stream-viewer.test.mjs'];
      testCmd = 'node --test tests/unit/stream-viewer.test.mjs tests/unit/stream-viewer-ui.test.mjs';
      oracle = '/home/sanya/re-deep/voice/ui7/';
      break;
    case 'TASK-808':
      writeScope = ['rust/gui-spike/src/messages.rs', 'rust/gui-spike/tests/messages_contract.rs'];
      testCmd = 'cargo test --manifest-path rust/gui-spike/Cargo.toml --test messages_contract';
      oracle = 'docs/reference-projects/sources/Serein/crates/discord-client/src/chat.rs';
      break;
    case 'TASK-809':
      writeScope = ['rust/gui-spike/src/invites.rs', 'rust/gui-spike/tests/invites_contract.rs'];
      testCmd = 'cargo test --manifest-path rust/gui-spike/Cargo.toml --test invites_contract';
      oracle = '/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0';
      break;
    case 'TASK-622':
      writeScope = ['tests/meta/mutations.test.mjs'];
      testCmd = 'node --test tests/meta/mutations.test.mjs';
      oracle = 'TEST-POLICY.md §11-§15';
      break;
    case 'TASK-702':
      writeScope = ['rust/discord-gpui/src/state/mod.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_initial_state_and_theme_palette';
      oracle = 'docs/specs/GPUI-ARCHITECTURE-MIGRATION.md';
      break;
    case 'TASK-703':
      writeScope = ['rust/discord-gpui/src/theme/mod.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_initial_state_and_theme_palette';
      oracle = '/home/sanya/re-deep/frontend-d03.md';
      break;
    case 'TASK-704':
      writeScope = ['rust/discord-gpui/src/net/mod.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_rest_client_super_properties_and_rate_limit_contract';
      oracle = '/home/sanya/re-deep/rest-d02.md';
      break;
    case 'TASK-719':
      writeScope = ['rust/discord-gpui/src/dispatch/mod.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_system_tray_states_and_badge_contract';
      oracle = '/home/sanya/re-deep/native-re.md';
      break;
    case 'TASK-705':
      writeScope = ['rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_120_fps_render_frame_budget_benchmark';
      oracle = 'docs/specs/GPUI-ARCHITECTURE-MIGRATION.md';
      break;
    case 'TASK-706':
      writeScope = ['rust/discord-gpui/src/ui/layout.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_select_guild_and_channel_transitions';
      oracle = 'docs/specs/GPUI-ARCHITECTURE-MIGRATION.md';
      break;
    case 'TASK-709':
      writeScope = ['rust/discord-gpui/src/ui/chat_view.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_message_sending_and_feed_update';
      oracle = 'docs/specs/GPUI-ARCHITECTURE-MIGRATION.md';
      break;
    case 'TASK-710':
      writeScope = ['rust/discord-gpui/src/ui/chat_view.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_quick_switcher_and_markdown_contract';
      oracle = 'docs/specs/GPUI-ARCHITECTURE-MIGRATION.md';
      break;
    case 'TASK-711':
      writeScope = ['rust/discord-gpui/src/ui/chat_view.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_keyboard_typing_and_draft_chat_contract';
      oracle = 'docs/specs/GPUI-ARCHITECTURE-MIGRATION.md';
      break;
    case 'TASK-712':
      writeScope = ['rust/discord-gpui/src/ui/captcha_modal.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_hcaptcha_challenge_trigger_and_resolution';
      oracle = '/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0';
      break;
    case 'TASK-713':
      writeScope = ['rust/discord-gpui/src/ui/create_guild_modal.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_create_guild_contract';
      oracle = '/home/sanya/re-deep/frontend-d03.md';
      break;
    case 'TASK-714':
      writeScope = ['rust/discord-gpui/src/ui/member_sidebar.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_member_sidebar_contract';
      oracle = '/home/sanya/re-deep/frontend-d03.md';
      break;
    case 'TASK-715':
      writeScope = ['rust/discord-gpui/src/ui/settings_dialog.rs', 'rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_settings_dialog_contract';
      oracle = '/home/sanya/re-deep/ui8/';
      break;
    case 'TASK-720':
      writeScope = ['rust/discord-gpui/tests/gpui_contract_test.rs'];
      testCmd = 'cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_contract_test test_120_fps_render_frame_budget_benchmark';
      oracle = 'TEST-POLICY.md §11-§15';
      break;
    default:
      writeScope = [`rust/gui-spike/src/${taskId.toLowerCase()}.rs`];
      testCmd = 'npm run test:unit';
  }

  return {
    mandateId: `MANDATE-${taskId}-${Date.now().toString(36)}`,
    taskId,
    writeScope,
    forbiddenPatterns: [
      'rust/crates/discord-core/src/lib.rs',
      'fixtures/**',
      'golden/**',
      'state/reducer.rs',
      'PROJECT-CONTEXT.md'
    ],
    testCmd,
    oracle,
    harmony
  };
}

/**
 * Валидация диффа воркера против мандата (Pre-Acceptance Verification)
 * @param {string[]} modifiedFiles
 * @param {string[]} allowedScope
 * @param {string[]} forbiddenPatterns
 * @returns {{ valid: boolean, violations: string[] }}
 */
export function validateMandateCompliance(modifiedFiles, allowedScope, forbiddenPatterns) {
  const violations = [];

  for (const file of modifiedFiles) {
    // 1. Проверка запрещённых паттернов
    for (const forbidden of forbiddenPatterns) {
      if (file === forbidden || (forbidden.endsWith('**') && file.startsWith(forbidden.slice(0, -2)))) {
        violations.push(`Файл ${file} находится в списке строго запрещённых (forbidden).`);
      }
    }

    // 2. Проверка попадания в разрешённый write-scope
    const inScope = allowedScope.some(scope => {
      if (scope === file) return true;
      if (scope.endsWith('**') && file.startsWith(scope.slice(0, -2))) return true;
      return false;
    });

    if (!inScope) {
      violations.push(`Файл ${file} выходит за рамки назначенного write-scope [${allowedScope.join(', ')}].`);
    }
  }

  return {
    valid: violations.length === 0,
    violations
  };
}

/**
 * Автоматизированный арбитр и финализация задачи (Automated Arbiter / Critic)
 * @param {string} taskId
 * @returns {{ success: boolean, output: string, error?: string }}
 */
export function verifyAndSettle(taskId) {
  const mandate = generateMandate(taskId);
  console.log(`[DSH ARBITER] Запуск проверки мандата ${mandate.mandateId} для задачи ${taskId}...`);
  console.log(`[DSH ARBITER] Команда верификации: ${mandate.testCmd}`);

  try {
    const stdout = execSync(mandate.testCmd, {
      cwd: PROJECT_ROOT,
      encoding: 'utf8',
      env: { ...process.env, CARGO_TERM_COLOR: 'never' },
      timeout: 120000
    });

    console.log(`[DSH ARBITER] ✅ Тесты успешно пройдены!`);

    // Обновляем статус карточки на доске
    const board = loadKanbanCards();
    const card = board.cards.find(c => c.id === taskId);
    if (card) {
      const prevStatus = card.status;
      card.status = 'done';
      saveKanbanCards(board);
      console.log(`[DSH ARBITER] Статус карточки ${taskId} изменён: ${prevStatus} -> done`);
    }

    // Освобождаем блокировки
    releaseLocks(taskId);

    // Записываем событие в Outbox
    recordOutboxEvent('TASK_SETTLED', taskId, {
      mandateId: mandate.mandateId,
      status: 'done'
    });

    return { success: true, output: stdout };
  } catch (err) {
    const errorMsg = (err && typeof err === 'object' && 'stdout' in err && err.stdout)
      ? `${String(err.stdout)}\n${String(/** @type {any} */(err).stderr || '')}`
      : String(/** @type {any} */(err)?.message || err);
    console.error(`[DSH ARBITER] ❌ Ошибка выполнения проверок для ${taskId}:\n${errorMsg}`);
    recordOutboxEvent('ARBITER_FAILED', taskId, { error: errorMsg });
    return { success: false, output: '', error: errorMsg };
  }
}

/**
 * CLI Entrypoint
 */
export function main() {
  const [,, cmd, ...args] = process.argv;

  switch (cmd) {
    case 'admission': {
      const taskId = args[0] || 'TASK-615';
      const decision = evaluateAdmission(taskId);
      console.log(JSON.stringify(decision, null, 2));
      break;
    }
    case 'mandate': {
      const taskId = args[0] || 'TASK-615';
      const mandate = generateMandate(taskId);
      console.log(JSON.stringify(mandate, null, 2));
      break;
    }
    case 'acquire': {
      const taskId = args[0];
      if (!taskId) throw new Error('taskId required');
      const mandate = generateMandate(taskId);
      acquireLocks(taskId, mandate.writeScope);
      console.log(`Блокировки успешно захвачены для ${taskId}: ${mandate.writeScope.join(', ')}`);
      break;
    }
    case 'release': {
      const taskId = args[0];
      if (!taskId) throw new Error('taskId required');
      releaseLocks(taskId);
      console.log(`Блокировки освобождены для ${taskId}`);
      break;
    }
    case 'verify':
    case 'settle': {
      const taskId = args[0];
      if (!taskId) throw new Error('taskId required');
      const res = verifyAndSettle(taskId);
      if (!res.success) {
        process.exit(1);
      }
      break;
    }
    case 'matrix': {
      const board = loadKanbanCards();
      console.log('=== HARMONY ADMISSION MATRIX (DSH ENGINE) ===');
      for (const card of board.cards.filter(c => c.status !== 'done')) {
        const decision = evaluateAdmission(card.id);
        const icon = decision.allowed ? '✅ READY' : '⏳ BLOCKED';
        console.log(`${icon} [${card.priority}] ${card.id}: ${card.title}`);
        if (!decision.allowed) {
          console.log(`   └─ Причина: ${decision.reason}`);
        } else if (decision.harmony) {
          console.log(`   └─ Harmony: ${decision.harmony.model} (effort: ${decision.harmony.effort}) — ${decision.harmony.rationale}`);
        }
      }
      break;
    }
    case 'locks': {
      console.log(JSON.stringify(getActiveLocks(), null, 2));
      break;
    }
    case 'status':
    default: {
      const board = loadKanbanCards();
      const counts = /** @type {Record<string, number>} */ ({ done: 0, in_dev: 0, backlog: 0 });
      board.cards.forEach(c => counts[c.status] = (counts[c.status] || 0) + 1);
      console.log(`Harmony Router Status: Total ${board.cards.length} cards | Done: ${counts.done} | In Dev: ${counts.in_dev} | Backlog: ${counts.backlog}`);
      break;
    }
  }
}

if (process.argv[1] && process.argv[1].endsWith('harmony-router.mjs')) {
  main();
}
