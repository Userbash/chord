#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/kanban-automate.mjs
 *
 * Автоматизированный менеджер канбан-доски дорожной карты упрощения Chord.
 * Устраняет рутинную ручную работу ИИ и экономит тысячи токенов контекста.
 * Добавляет структурированные задачи, проверяет целостность и экспортирует HTML.
 */

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const KANBAN_JSON = path.join(PROJECT_ROOT, 'waves/kanban-board.json');

/**
 * @typedef {{
 *   id: string,
 *   title: string,
 *   description: string,
 *   status: string,
 *   column: string,
 *   priority: "P0" | "P1" | "P2" | "P3",
 *   tags: string[],
 *   assignee?: string,
 *   epic?: string,
 *   subtasks?: string[],
 *   transparent_bridge?: string,
 *   library?: string,
 *   what_to_keep?: string
 * }} KanbanCard
 */

/** @type {KanbanCard[]} */
const NEW_SIMPLIFICATION_CARDS = [
  {
    id: "LIB-SIMPLIFY-01",
    title: "HTTP & REST: Перенос на reqwest + url + serde через прозрачный мост",
    description: "Устранение ручной сборки URL и query-строк в ipc_ext.rs и chord-core. Замена на reqwest, url::Url и типизированные serde DTO с сохранением правил Discord rate limits.",
    status: "done",
    column: "done",
    priority: "P0",
    tags: ["Backend", "HTTP", "reqwest", "Library-Bridge", "P0", "Done"],
    assignee: "G1 Backend",
    epic: "EPIC-LIB-01: HTTP & REST API Transparent Bridge",
    transparent_bridge: "DiscordRestBridge: Tauri Commands -> RestBridge -> reqwest -> Discord API v9",
    library: "reqwest = { version = '0.12', default-features = false, features = ['rustls-tls', 'json'] }, url = '2.5'",
    what_to_keep: "Специфичные заголовки Discord, обработку 429 Retry-After, snowflake-валидацию",
    subtasks: [
      "101.1: Создание структуры DiscordHttpClient с переиспользуемым connection pool (ВЫПОЛНЕНО: bridge.rs)",
      "101.2: Безопасное процентное кодирование параметров через percent_encoding / url (ВЫПОЛНЕНО: 33 теста)",
      "102.1: Типизированные Serde DTO для запросов и ответов вместо нетипизированного json! (ВЫПОЛНЕНО)",
      "103.1: Управление rate limits через tokio::sync::Semaphore и учет X-RateLimit-Bucket (ВЫПОЛНЕНО)"
    ]
  },
  {
    id: "LIB-SIMPLIFY-02",
    title: "Storage: Перенос chord-store на rusqlite + выделенный DB-Worker",
    description: "Замена опасного C FFI chord-store на чистый rusqlite (#![forbid(unsafe_code)]) с выделенным DB-Worker (mpsc 1024), полнотекстовым поиском FTS5 unicode61 и миграциями схемы v2.",
    status: "done",
    column: "done",
    priority: "P0",
    tags: ["Backend", "SQLite", "rusqlite", "Storage", "FTS5", "P0", "Done"],
    assignee: "G1 Backend",
    epic: "EPIC-LIB-02: Local SQLite Storage & FTS5 Transparent Bridge",
    transparent_bridge: "StoreBridge: Async Tokio -> bounded mpsc(1024) -> Dedicated std::thread DB Worker -> rusqlite",
    library: "rusqlite = { version = '0.31', features = ['bundled', 'backup'] }",
    what_to_keep: "Существующую схему FTS5 виртуального поиска, разделение кэша по аккаунтам",
    subtasks: [
      "201.1: Подключение rusqlite в chord-store/Cargo.toml и удаление ffi.rs (ВЫПОЛНЕНО)",
      "201.2: Создание схемы таблиц: messages, channels, guilds, read_states, messages_fts (ВЫПОЛНЕНО)",
      "202.1: Выделенный поток DB Worker с очередью команд mpsc и oneshot-каналами ответов (ВЫПОЛНЕНО: 22 теста)",
      "203.1: Механизм транзакционных миграций схемы с защитой от прерывания питания (ВЫПОЛНЕНО)"
    ]
  },
  {
    id: "LIB-SIMPLIFY-03",
    title: "System & Secrets: Интеграция OS Keyring + Tauri Plugins + ashpd",
    description: "Интеграция keyring для хранения Discord-токенов в Secret Service/Keychain/Credential Manager, зануление секретов zeroize и порталы ashpd.",
    status: "done",
    column: "done",
    priority: "P1",
    tags: ["Platform", "keyring", "Security", "Tauri-Plugins", "ashpd", "P1", "Done"],
    assignee: "G5 Voice / Platform",
    epic: "EPIC-LIB-03: System Platform & Credential Bridge",
    transparent_bridge: "PlatformBridge: Auth Manager -> keyring (Secret Service / Keychain / Credential Manager)",
    library: "keyring = '2.1', zeroize = '1.8', ashpd = '0.13'",
    what_to_keep: "Интеграцию со скринкастом Wayland PipeWire через ashpd, zeroize для очистки токенов в RAM",
    subtasks: [
      "301.1: Безопасное сохранение Discord Token в OS Keyring без plaintext-записи на диск (ВЫПОЛНЕНО: 31 тест)",
      "301.2: Очистка временных секретов в памяти через zeroize (ВЫПОЛНЕНО)",
      "302.1: Использование ashpd screencast для PipeWire (ВЫПОЛНЕНО)"
    ]
  },
  {
    id: "LIB-SIMPLIFY-04",
    title: "Audio Engine: Изолированный спайк cpal + rtrb + opus2 в rust/audio-spike",
    description: "Построение изолированного спайка в rust/audio-spike (по образцу dave-spike): беблокировочный колбэк микрофона через rtrb, cpal и чистый opus2 без системного C FFI.",
    status: "done",
    column: "done",
    priority: "P0",
    tags: ["Voice", "Audio", "cpal", "rtrb", "opus2", "AudioSpike", "P0", "Done"],
    assignee: "G5 Voice / Platform",
    epic: "EPIC-LIB-04: Audio Real-Time Engine & Codecs",
    transparent_bridge: "AudioPipelineBridge: cpal mic callback -> lock-free rtrb -> dedicated Audio Worker -> opus2 -> UDP",
    library: "cpal = '0.15', rtrb = '0.4', opus2 = { version = '0.4', features = ['bundled', 'static'] }",
    what_to_keep: "Tauri IPC команды переключения устройств, UI-сцену звонка (voice-call-stage.js)",
    subtasks: [
      "401.1: Создание отдельного крейта rust/audio-spike с cpal + rtrb + opus2 (ВЫПОЛНЕНО)",
      "401.2: Беблокировочный rtrb ring buffer между cpal StreamCallback и воркером (ВЫПОЛНЕНО)",
      "402.1: Интеграция opus2 (кодирование/декодирование 48kHz голоса с bundled libopus) (ВЫПОЛНЕНО)",
      "402.2: Контрактные тесты сквозного кодирования и декодирования аудиопотока (ВЫПОЛНЕНО: 7/7 тестов)"
    ]
  },
  {
    id: "LIB-SIMPLIFY-05",
    title: "DAVE E2EE: Двойной спайк-прототип libdave vs davey + openmls",
    description: "Сравнение двух подходов к E2EE голоса Discord в rust/dave-spike. Принято решение ADR-DAVE-05 в пользу чистого Rust Candidate B (davey + ring + RustCrypto, 0 строк кастомной криптографии).",
    status: "done",
    column: "done",
    priority: "P1",
    tags: ["Voice", "Crypto", "DAVE", "MLS", "libdave", "davey", "P1", "Done"],
    assignee: "G5 Voice / Platform",
    epic: "EPIC-LIB-05: DAVE E2EE Crypto Prototype & Modernization",
    transparent_bridge: "DaveVoiceBridge: Voice Gateway (Op 18..22) -> DaveSessionManager -> DaveBackend (DaveySession)",
    library: "davey = '0.1.4', ring = '0.17', aes-gcm = '0.11', hkdf = '0.12', zeroize = '1.9'",
    what_to_keep: "Спецификацию сигнализации Voice Gateway (opcodes 18..22), смену эпох и участников",
    subtasks: [
      "501.1: Сборка и тестирование минимального Rust FFI прототипа с libdave (ВЫПОЛНЕНО)",
      "501.2: Сборка и тестирование прототипа с davey/RustCrypto из Serein (ВЫПОЛНЕНО)",
      "501.3: Сравнительный аудит производительности, безопасности и зависимостей (ВЫПОЛНЕНО: docs/specs/dave-e2ee-spike-comparison.md)",
      "502.1: Реализация единого прозрачного адаптера DaveVoiceBridge (ВЫПОЛНЕНО)"
    ]
  },
  {
    id: "LIB-SIMPLIFY-06",
    title: "Concurrency & Automation: x86_64 Tokio/Rayon Runtime + Скрипты экономии токенов",
    description: "Разделение потоков исполнения: Tokio для асинхронной сети, Rayon для тяжелых CPU-задач (картинки/видео), rtrb для звука. Разработка автоматических скриптов для рутинных задач ИИ.",
    status: "in_dev",
    column: "in_dev",
    priority: "P0",
    tags: ["Architecture", "x86_64", "Tokio", "Rayon", "Automation", "Scripts", "P0"],
    assignee: "Orchestrator",
    epic: "EPIC-LIB-06: x86_64 Multi-Core Concurrency & Automation",
    transparent_bridge: "TaskRouterBridge: UI Command -> Async Tokio Dispatcher -> Bounded Channels -> Specialized Workers",
    library: "tokio = { version = '1', features = ['full'] }, rayon = '1.10', tracing = '0.1', metrics = '0.23'",
    what_to_keep: "Оболочку приложения Tauri 2, структуру неблокирующего UI на сигналах",
    subtasks: [
      "601.1: Единый Tokio runtime с CancellationToken и JoinSet для управляемого завершения",
      "601.2: Ограниченный Rayon пул потоков для параллельной обработки медиа без просадок UI",
      "602.1: Сквозная трассировка tracing со скрытием токенов и паролей (Secret Redaction Filter)",
      "603.1: Написание скрипта kanban-automate.mjs для беззатратного управления доской",
      "603.2: Написание скрипта smart-typecheck.mjs и ram-guard для воркеров"
    ]
  },
  {
    id: "LIB-SIMPLIFY-07",
    title: "Frontend Libraries: Завершение интеграции ESM-библиотек (W01–W10)",
    description: "Фронтенд-библиотеки полностью вытеснили монолитный самописный код: @preact/signals-core, @tanstack/virtual-core, @floating-ui/dom, tinykeys. 57 из 57 контрактных тестов успешно проходят.",
    status: "done",
    column: "done",
    priority: "P0",
    tags: ["Frontend", "ESM", "PreactSignals", "TanStackVirtual", "FloatingUI", "Tinykeys", "Done"],
    assignee: "G2 Frontend",
    epic: "EPIC-LIB-07: Frontend Transparent Library Integration",
    transparent_bridge: "AutoIpcBridge: window.__TAURI__.core.invoke -> auto-ipc-bridge.js -> reactive UI signals",
    library: "@preact/signals-core = 1.8.0, @tanstack/virtual-core = 3.10.8, @floating-ui/dom = 1.6.12, tinykeys = 3.0.0",
    what_to_keep: "Cleanroom CSS дизайн-токены (tokens-cleanroom.css), верстку и стили компонентов Discord",
    subtasks: [
      "W01 Voice Call Stage & Video Demo Grid (тесты 4/4 зеленые)",
      "W02 Virtualized Chat History & Scroller (TanStack Virtual 100k сообщений)",
      "W03 Discord Markdown & Codeblock Engine (AST парсер без eval)",
      "W04 Guild Member List (виртуализация 25k+ участников)",
      "W05 Chat Composer & Autocomplete (автокомплит на Floating UI и шорткаты tinykeys)",
      "W06 Expression Picker & Hover Reactions (чистый пикер эмодзи)",
      "W07 Cleanroom User Profile Popout & Modal (18/18 тестов зеленые)"
    ]
  }
];

function main() {
  console.log('[Kanban Automation] Loading waves/kanban-board.json...');
  if (!fs.existsSync(KANBAN_JSON)) {
    console.error(`Error: ${KANBAN_JSON} not found`);
    process.exit(1);
  }

  const raw = fs.readFileSync(KANBAN_JSON, 'utf-8');
  const board = JSON.parse(raw);

  // Проверяем наличие существующих карточек LIB-SIMPLIFY
  const existingIds = new Set(board.cards.map((/** @type {any} */ c) => c.id));
  let addedCount = 0;
  let updatedCount = 0;

  for (const card of NEW_SIMPLIFICATION_CARDS) {
    if (existingIds.has(card.id)) {
      const idx = board.cards.findIndex((/** @type {any} */ c) => c.id === card.id);
      board.cards[idx] = { ...board.cards[idx], ...card };
      updatedCount++;
    } else {
      board.cards.push(card);
      addedCount++;
    }
  }

  board.updated_at = new Date().toISOString();
  board.wave = "RW-53: Library-Driven Refactoring & Cleanroom Architecture Migration";

  // Сохраняем обновленный JSON
  fs.writeFileSync(KANBAN_JSON, JSON.stringify(board, null, 2), 'utf-8');
  console.log(`[Kanban Automation] Saved: ${addedCount} new cards added, ${updatedCount} updated.`);
  console.log(`[Kanban Automation] Total board cards: ${board.cards.length}`);

  // Экспортируем интерактивный HTML дашборд
  try {
    execSync('node scripts/mcp-kanban-board.mjs --export', { stdio: 'inherit' });
    console.log('[Kanban Automation] Interactive HTML dashboard updated at waves/kanban-board.html');
  } catch (err) {
    console.warn('[Kanban Automation] HTML export warning:', err.message);
  }
}

main();
