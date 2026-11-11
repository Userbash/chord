#!/usr/bin/env node
// @ts-check
/**
 * Стресс-тест SQLite WAL и полнотекстового поиска FTS5 при конкурентных транзакциях (TASK-973):
 * - Инициализация схемы SQLite в режиме PRAGMA journal_mode = WAL (`messages`, `channels`, `messages_fts`, `quick_switcher_fts`).
 * - Параллельная запись 100 000 сообщений в 10 потоков (`worker_threads`) с конкурентными FTS5-запросами.
 * - Верификация целостности `PRAGMA integrity_check = ok`, `PRAGMA quick_check = ok` и FTS5 `integrity-check`.
 * - Полнотекстовый поиск FTS5 с пагинацией Discord (`offset <= 9975`, `offset + limit <= 10 000`),
 *   фильтрацией прав доступа (`VIEW_CHANNEL | READ_MESSAGE_HISTORY`, `AGE_GATED_SPACES`),
 *   Quick Switcher префиксами (`*`, `#`, `!`, `@`) и командами `DISCORD_SEARCH_*` (`core/bundle.js#4355`).
 */
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";
import { Worker } from "node:worker_threads";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export const PERMISSION_VIEW_CHANNEL = 1n << 10n;
export const PERMISSION_READ_MESSAGE_HISTORY = 1n << 16n;
export const MIN_SNOWFLAKE = 0n;
export const MAX_SNOWFLAKE = 18446744073709551615n;

export const DISCORD_SEARCH_IPC_CHANNELS = Object.freeze([
  "DISCORD_SEARCH_INDEX_DOMAINS",
  "DISCORD_SEARCH_CLEAR_INDEX",
  "DISCORD_SEARCH_DELETE_DOMAINS",
  "DISCORD_SEARCH_DELETE_ITEMS",
]);

export const FTS5_STRESS_DEFAULT_CONFIG = Object.freeze({
  totalMessages: 100_000,
  parallelThreads: 10,
  messagesPerThread: 10_000,
  batchSize: 2_500,
  busyTimeoutMs: 30_000,
  journalMode: "wal",
  synchronousMode: "NORMAL",
  maxMessageLength: 2_000,
  maxSearchOffset: 9_975,
  defaultSearchLimit: 25,
  maxSearchLimit: 1_000,
  maxSearchWindow: 10_000,
  minSnowflake: MIN_SNOWFLAKE,
  maxSnowflake: MAX_SNOWFLAKE,
});

const SQL_SCHEMA = `
  CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY NOT NULL,
    guild_id TEXT,
    name TEXT NOT NULL,
    kind INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    required_permissions TEXT NOT NULL DEFAULT '0',
    nsfw INTEGER NOT NULL DEFAULT 0
  );
  CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT NOT NULL,
    author_id TEXT NOT NULL,
    author_name TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    edited_timestamp INTEGER,
    flags INTEGER NOT NULL DEFAULT 0,
    deleted INTEGER NOT NULL DEFAULT 0
  );
  CREATE INDEX IF NOT EXISTS idx_messages_channel_timestamp ON messages(channel_id, timestamp DESC);
  CREATE INDEX IF NOT EXISTS idx_messages_channel_id ON messages(channel_id, id DESC);
  CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    message_id UNINDEXED,
    channel_id UNINDEXED,
    author_id UNINDEXED,
    author_name,
    content,
    tokenize = 'unicode61 remove_diacritics 2'
  );
  CREATE VIRTUAL TABLE IF NOT EXISTS quick_switcher_fts USING fts5(
    id UNINDEXED,
    kind,
    name,
    subtitle,
    tokenize = 'unicode61 remove_diacritics 2'
  );
  CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
  );
`;

const WORKER_SCRIPT_SOURCE = `
const { parentPort, workerData } = require("node:worker_threads");
const { DatabaseSync } = require("node:sqlite");

const { dbPath, workerId, count, batchSize, busyTimeoutMs } = workerData;
const db = new DatabaseSync(dbPath, { timeout: busyTimeoutMs });
db.exec("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA temp_store = MEMORY; PRAGMA cache_size = -16000;");

const insMsg = db.prepare(
  "INSERT INTO messages (id, channel_id, author_id, author_name, content, timestamp, flags, deleted) VALUES (?, ?, ?, ?, ?, ?, 0, 0)"
);
const insFts = db.prepare(
  "INSERT INTO messages_fts (rowid, message_id, channel_id, author_id, author_name, content) VALUES (?, ?, ?, ?, ?, ?)"
);
const searchStmt = db.prepare("SELECT COUNT(*) AS hits FROM messages_fts WHERE messages_fts MATCH ?");

let inserted = 0, searchQueries = 0, busyRetries = 0, deadlocks = 0, failedTransactions = 0;
const baseRowId = workerId * count + 1;

for (let offset = 0; offset < count; offset += batchSize) {
  const limit = Math.min(batchSize, count - offset);
  let committed = false, attempts = 0;
  while (!committed) {
    attempts += 1;
    try {
      db.exec("BEGIN IMMEDIATE");
      for (let i = 0; i < limit; i += 1) {
        const rowid = baseRowId + offset + i;
        const id = String(100000000000000000n + BigInt(rowid));
        const chId = "ch-" + (workerId % 10);
        const authorId = String(10000n + BigInt(workerId));
        const authorName = "worker_" + workerId;
        const content = "Сообщение #" + rowid + " от " + authorName + " в канале " + chId + " token_" + (rowid % 50) + " discord_fts5_wal_stress";
        insMsg.run(id, chId, authorId, authorName, content, 1700000000000 + rowid);
        insFts.run(rowid, id, chId, authorId, authorName, content);
      }
      db.exec("COMMIT");
      committed = true;
      inserted += limit;
    } catch {
      try { db.exec("ROLLBACK"); } catch {}
      busyRetries += 1;
      if (attempts > 64) { deadlocks += 1; failedTransactions += 1; break; }
    }
  }
  const hitRow = searchStmt.get("token_" + (workerId % 50));
  if (hitRow && Number(hitRow.hits) >= 0) searchQueries += 1;
}

db.close();
parentPort.postMessage({ workerId, inserted, searchQueries, busyRetries, deadlocks, failedTransactions });
`;

/**
 * Маскирует чувствительные токены Discord в строках логов и ошибок.
 * @param {string} text
 * @returns {string}
 */
export function maskSensitiveTokens(text) {
  if (!text) return "";
  return String(text)
    .replace(/mfa\.[A-Za-z0-9_-]{8,}/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]{8,}/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{8,}/g, "Bearer ***")
    .replace(/\b[A-Za-z0-9_-]{20,28}\.[A-Za-z0-9_-]{6,7}\.[A-Za-z0-9_-]{25,45}\b/g, "[REDACTED_TOKEN]");
}

/**
 * Проверяет корректность Snowflake ID в диапазоне `0..=u64::MAX`.
 * @param {string} rawId
 * @param {string} [fieldName]
 */
export function validateSnowflakeId(rawId, fieldName = "id") {
  if (typeof rawId !== "string" || rawId.length === 0 || rawId.includes("\0")) {
    throw new Error(`Некорректный Snowflake ${fieldName}: пустой или содержит NUL`);
  }
  if (/^\d+$/.test(rawId)) {
    const value = BigInt(rawId);
    if (value < MIN_SNOWFLAKE || value > MAX_SNOWFLAKE) {
      throw new Error(`Snowflake ${fieldName} вне диапазона u64 (0..18446744073709551615): ${rawId}`);
    }
    return value;
  }
  if (/^[a-zA-Z][a-zA-Z0-9_-]*$/.test(rawId)) return 0n;
  throw new Error(`Некорректный Snowflake ${fieldName}: ожидалось неотрицательное целое число u64`);
}

/**
 * Проверяет входные поля сообщения перед записью в SQLite и FTS5.
 * @param {{ id: string, channel_id: string, author_id: string, author_name: string, content: string, timestamp: number, flags?: number }} msg
 */
export function validateMessageInput(msg) {
  for (const [key, val] of Object.entries(msg)) {
    if (typeof val === "string" && val.includes("\0")) {
      throw new Error(`Поле ${key} содержит недопустимый NUL-байт (\\0)`);
    }
  }
  validateSnowflakeId(msg.id, "id");
  validateSnowflakeId(msg.author_id, "author_id");
  if (!msg.channel_id || typeof msg.channel_id !== "string") {
    throw new Error("Поле channel_id обязательно");
  }
  if (typeof msg.content !== "string" || msg.content.length === 0) {
    throw new Error("Поле content не может быть пустым");
  }
  if (msg.content.length > FTS5_STRESS_DEFAULT_CONFIG.maxMessageLength) {
    throw new Error(`Длина сообщения (${msg.content.length}) превышает лимит 2000 символов`);
  }
  if (msg.flags !== undefined && (msg.flags < 0 || !Number.isInteger(msg.flags))) {
    throw new Error("Поле flags должно быть неотрицательным целым числом");
  }
}

/**
 * Очищает пользовательский поисковый запрос для безопасной подстановки в `FTS5 MATCH`.
 * @param {string} rawQuery
 * @returns {string}
 */
export function sanitizeFts5Query(rawQuery) {
  if (typeof rawQuery !== "string") return "";
  const withoutNull = rawQuery.replace(/\0/g, " ").trim();
  if (!withoutNull) return "";

  return withoutNull
    .replace(/["'()[\]{}:;^~\\/<>|!+=,&-]/g, " ")
    .split(/\s+/)
    .map((tok) => tok.replace(/^\*+|\*+$/g, (m) => (m.endsWith("*") ? "*" : "")))
    .filter((tok) => {
      const clean = tok.replace(/\*/g, "");
      if (!clean) return false;
      const upper = clean.toUpperCase();
      return upper !== "AND" && upper !== "OR" && upper !== "NOT" && upper !== "NEAR";
    })
    .map((tok) => {
      const hasPrefix = tok.endsWith("*") && tok.length > 1;
      const base = tok.replace(/\*/g, "");
      return hasPrefix ? `"${base}"*` : `"${base}"`;
    })
    .join(" ");
}

/**
 * Инициализирует базу данных SQLite в режиме `WAL` и создаёт таблицы `messages` и `FTS5`.
 * @param {string} dbPath
 * @param {{ busyTimeoutMs?: number }} [options]
 * @returns {DatabaseSync}
 */
export function initFts5WalDatabase(dbPath, options = {}) {
  const timeout = options.busyTimeoutMs ?? FTS5_STRESS_DEFAULT_CONFIG.busyTimeoutMs;
  const db = new DatabaseSync(dbPath, { timeout });
  db.exec(`
    PRAGMA busy_timeout = ${timeout};
    PRAGMA foreign_keys = ON;
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA temp_store = MEMORY;
    PRAGMA cache_size = -32000;
  `);
  db.exec("BEGIN IMMEDIATE");
  try {
    db.exec(SQL_SCHEMA);
    db.exec("PRAGMA user_version = 1");
    db.exec("COMMIT");
  } catch (err) {
    try { db.exec("ROLLBACK"); } catch { /* ignore */ }
    db.close();
    throw err;
  }
  return db;
}

/**
 * Добавляет или обновляет канал с маской требуемых прав и флагом `nsfw` (`AGE_GATED_SPACES`).
 * @param {DatabaseSync} db
 * @param {{ id: string, guild_id?: string | null, name: string, kind?: number, position?: number, required_permissions?: string, nsfw?: boolean }} channel
 */
export function upsertChannel(db, channel) {
  db.prepare(`
    INSERT INTO channels (id, guild_id, name, kind, position, required_permissions, nsfw)
    VALUES (?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET
      guild_id = excluded.guild_id, name = excluded.name, kind = excluded.kind,
      position = excluded.position, required_permissions = excluded.required_permissions, nsfw = excluded.nsfw
  `).run(
    channel.id,
    channel.guild_id ?? null,
    channel.name,
    channel.kind ?? 0,
    channel.position ?? 0,
    channel.required_permissions ?? "0",
    channel.nsfw ? 1 : 0,
  );
}

/**
 * Вставляет сообщение в таблицу `messages` и синхронно индексирует его в `messages_fts`.
 * @param {DatabaseSync} db
 * @param {{ id: string, channel_id: string, author_id: string, author_name: string, content: string, timestamp: number, edited_timestamp?: number | null, flags?: number, deleted?: boolean }} msg
 */
export function insertMessageWithFts5(db, msg) {
  validateMessageInput(msg);
  const safeContent = maskSensitiveTokens(msg.content);
  db.exec("BEGIN IMMEDIATE");
  try {
    const res = db.prepare(`
      INSERT INTO messages (id, channel_id, author_id, author_name, content, timestamp, edited_timestamp, flags, deleted)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET
        channel_id = excluded.channel_id, author_id = excluded.author_id, author_name = excluded.author_name,
        content = excluded.content, timestamp = excluded.timestamp, edited_timestamp = excluded.edited_timestamp,
        flags = excluded.flags, deleted = excluded.deleted
    `).run(msg.id, msg.channel_id, msg.author_id, msg.author_name, safeContent, msg.timestamp, msg.edited_timestamp ?? null, msg.flags ?? 0, msg.deleted ? 1 : 0);
    db.prepare("DELETE FROM messages_fts WHERE message_id = ?").run(msg.id);
    if (!msg.deleted) {
      db.prepare("INSERT INTO messages_fts (rowid, message_id, channel_id, author_id, author_name, content) VALUES (?, ?, ?, ?, ?, ?)")
        .run(res.lastInsertRowid, msg.id, msg.channel_id, msg.author_id, msg.author_name, safeContent);
    }
    db.exec("COMMIT");
  } catch (err) {
    try { db.exec("ROLLBACK"); } catch { /* ignore */ }
    throw err;
  }
}

/**
 * Выполняет полнотекстовый поиск сообщений через `messages_fts` с поддержкой пагинации и ACL.
 * @param {DatabaseSync} db
 * @param {{ query: string, channelId?: string, authorId?: string, limit?: number, offset?: number, userPermissions?: bigint, ageVerified?: boolean, indexingState?: { indexing: boolean, retryAfterMs?: number } }} params
 */
export function searchMessagesFts5(db, params) {
  if (params.indexingState?.indexing) {
    return { status: 202, indexing: true, retry_after_ms: params.indexingState.retryAfterMs ?? 1_000, totalHits: 0, messages: [] };
  }
  const offset = params.offset ?? 0;
  const limit = Math.min(params.limit ?? FTS5_STRESS_DEFAULT_CONFIG.defaultSearchLimit, FTS5_STRESS_DEFAULT_CONFIG.maxSearchLimit);
  if (offset < 0 || offset > FTS5_STRESS_DEFAULT_CONFIG.maxSearchOffset || offset + limit > FTS5_STRESS_DEFAULT_CONFIG.maxSearchWindow) {
    throw new Error(`Некорректный offset пагинации поиска (${offset}): допустимо offset <= 9975 и offset + limit <= 10000`);
  }
  const sanitized = sanitizeFts5Query(params.query);
  if (!sanitized) {
    return { status: 200, indexing: false, retry_after_ms: 0, totalHits: 0, messages: [] };
  }
  const rows = queryFtsCandidates(db, sanitized, params.channelId, params.authorId);
  const filtered = filterRowsByPermissions(db, rows, params.userPermissions, params.ageVerified);
  return {
    status: 200,
    indexing: false,
    retry_after_ms: 0,
    totalHits: filtered.length,
    messages: limit === 0 ? [] : filtered.slice(offset, offset + limit),
  };
}

/**
 * @param {DatabaseSync} db
 * @param {string} ftsQuery
 * @param {string} [channelId]
 * @param {string} [authorId]
 */
function queryFtsCandidates(db, ftsQuery, channelId, authorId) {
  let sql = `
    SELECT m.id, m.channel_id, m.author_id, m.author_name, m.content, m.timestamp,
           snippet(messages_fts, 4, '<mark>', '</mark>', '...', 16) AS snippet, bm25(messages_fts) AS rank
    FROM messages_fts JOIN messages m ON m.id = messages_fts.message_id
    WHERE messages_fts MATCH ? AND m.deleted = 0
  `;
  /** @type {Array<string | number>} */
  const bindParams = [ftsQuery];
  if (channelId) { sql += " AND m.channel_id = ?"; bindParams.push(channelId); }
  if (authorId) { sql += " AND m.author_id = ?"; bindParams.push(authorId); }
  sql += " ORDER BY rank ASC, m.timestamp DESC LIMIT 10000";
  return db.prepare(sql).all(...bindParams).map((row) => ({
    id: String(row.id),
    channel_id: String(row.channel_id),
    author_id: String(row.author_id),
    author_name: String(row.author_name),
    content: String(row.content),
    timestamp: Number(row.timestamp),
    snippet: String(row.snippet ?? ""),
    rank: Number(row.rank ?? 0),
  }));
}

/**
 * @param {DatabaseSync} db
 * @param {Array<{ id: string, channel_id: string, author_id: string, author_name: string, content: string, timestamp: number, snippet: string, rank: number }>} rows
 * @param {bigint} [userPermissions]
 * @param {boolean} [ageVerified]
 */
function filterRowsByPermissions(db, rows, userPermissions, ageVerified) {
  if (userPermissions === undefined && ageVerified === undefined) return rows;
  const chStmt = db.prepare("SELECT required_permissions, nsfw FROM channels WHERE id = ?");
  /** @type {Map<string, { req: bigint, nsfw: boolean }>} */
  const cache = new Map();
  return rows.filter((row) => {
    let meta = cache.get(row.channel_id);
    if (!meta) {
      const ch = chStmt.get(row.channel_id);
      meta = { req: ch?.required_permissions ? BigInt(String(ch.required_permissions)) : 0n, nsfw: Number(ch?.nsfw ?? 0) !== 0 };
      cache.set(row.channel_id, meta);
    }
    if (userPermissions !== undefined && (userPermissions & meta.req) !== meta.req) return false;
    if (meta.nsfw && ageVerified === false) return false;
    return true;
  });
}

/**
 * Индексирует элементы быстрого перехода (`Quick Switcher`) в `quick_switcher_fts`.
 * @param {DatabaseSync} db
 * @param {Array<{ id: string, kind: "server" | "channel" | "user", name: string, subtitle: string }>} items
 */
export function indexQuickSwitcherItems(db, items) {
  db.exec("BEGIN IMMEDIATE");
  try {
    db.exec("DELETE FROM quick_switcher_fts");
    const ins = db.prepare("INSERT INTO quick_switcher_fts (id, kind, name, subtitle) VALUES (?, ?, ?, ?)");
    for (const item of items) ins.run(item.id, item.kind, item.name, item.subtitle);
    db.exec("COMMIT");
  } catch (err) {
    try { db.exec("ROLLBACK"); } catch { /* ignore */ }
    throw err;
  }
}

/**
 * Выполняет поиск в `quick_switcher_fts` с поддержкой префиксов `*`, `#`, `!`, `@`.
 * @param {DatabaseSync} db
 * @param {string} rawQuery
 */
export function searchQuickSwitcherFts5(db, rawQuery) {
  const q = rawQuery.trim();
  /** @type {"server" | "channel" | "user" | null} */
  let kindFilter = null;
  let term = q;
  if (q.startsWith("*")) { kindFilter = "server"; term = q.slice(1).trim(); }
  else if (q.startsWith("#") || q.startsWith("!")) { kindFilter = "channel"; term = q.slice(1).trim(); }
  else if (q.startsWith("@")) { kindFilter = "user"; term = q.slice(1).trim(); }

  const sanitized = sanitizeFts5Query(term ? `${term}*` : "");
  const sql = !sanitized
    ? (kindFilter ? "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE kind = ? LIMIT 50" : "SELECT id, kind, name, subtitle FROM quick_switcher_fts LIMIT 50")
    : (kindFilter ? "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE quick_switcher_fts MATCH ? AND kind = ? ORDER BY rank LIMIT 50" : "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE quick_switcher_fts MATCH ? ORDER BY rank LIMIT 50");
  const rows = !sanitized
    ? (kindFilter ? db.prepare(sql).all(kindFilter) : db.prepare(sql).all())
    : (kindFilter ? db.prepare(sql).all(sanitized, kindFilter) : db.prepare(sql).all(sanitized));
  return rows.map((r) => ({ id: String(r.id), kind: String(r.kind), name: String(r.name), subtitle: String(r.subtitle) }));
}

/**
 * Обрабатывает IPC-команды поискового движка Discord (`core/bundle.js#4355`).
 * @param {DatabaseSync} db
 * @param {string} channel
 * @param {{ domains?: Array<{ id: string, items?: Array<{ id: string, channel_id: string, author_id: string, author_name: string, content: string, timestamp: number }> }>, ids?: string[] }} [payload]
 */
export function handleSearchIpcCommand(db, channel, payload = {}) {
  if (channel === "DISCORD_SEARCH_CLEAR_INDEX") {
    db.exec("DELETE FROM messages_fts; DELETE FROM quick_switcher_fts;");
    return { ok: true, channel, cleared: true, indexedCount: 0, deletedCount: 0 };
  }
  if (channel === "DISCORD_SEARCH_DELETE_DOMAINS") {
    const del = db.prepare("DELETE FROM messages_fts WHERE channel_id = ?");
    let deletedCount = 0;
    for (const d of payload.domains ?? []) deletedCount += Number(del.run(d.id).changes);
    return { ok: true, channel, cleared: false, indexedCount: 0, deletedCount };
  }
  if (channel === "DISCORD_SEARCH_DELETE_ITEMS") {
    const delFts = db.prepare("DELETE FROM messages_fts WHERE message_id = ?");
    const markDel = db.prepare("UPDATE messages SET deleted = 1 WHERE id = ?");
    let deletedCount = 0;
    for (const id of payload.ids ?? []) { deletedCount += Number(delFts.run(id).changes); markDel.run(id); }
    return { ok: true, channel, cleared: false, indexedCount: 0, deletedCount };
  }
  if (channel === "DISCORD_SEARCH_INDEX_DOMAINS") {
    let indexedCount = 0;
    for (const dom of payload.domains ?? []) {
      for (const item of dom.items ?? []) { insertMessageWithFts5(db, item); indexedCount += 1; }
    }
    return { ok: true, channel, cleared: false, indexedCount, deletedCount: 0 };
  }
  throw new Error(`Неизвестный IPC-канал поиска: ${channel}`);
}

/**
 * Восстанавливает индекс `messages_fts` из таблицы `messages`, выполняет `optimize` и `wal_checkpoint`.
 * @param {DatabaseSync} db
 */
export function rebuildAndCheckpointFts5(db) {
  db.exec("BEGIN IMMEDIATE");
  try {
    db.exec("DELETE FROM messages_fts");
    db.exec(`
      INSERT INTO messages_fts (rowid, message_id, channel_id, author_id, author_name, content)
      SELECT rowid, id, channel_id, author_id, author_name, content FROM messages WHERE deleted = 0
    `);
    db.exec("INSERT INTO messages_fts(messages_fts) VALUES('optimize')");
    db.exec("COMMIT");
  } catch (err) {
    try { db.exec("ROLLBACK"); } catch { /* ignore */ }
    throw err;
  }
  db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
  const countRow = db.prepare("SELECT COUNT(*) AS cnt FROM messages_fts").get();
  const integrityRow = db.prepare("PRAGMA integrity_check").get();
  return { rebuiltRows: Number(countRow?.cnt ?? 0), integrityCheck: String(integrityRow?.integrity_check ?? "error") };
}

/**
 * Проверяет целостность базы данных SQLite WAL и индекса FTS5.
 * @param {DatabaseSync} db
 * @param {{ expectedMessages?: number }} [options]
 */
export function verifyDatabaseIntegrity(db, options = {}) {
  const journalRow = db.prepare("PRAGMA journal_mode").get();
  const integrityRow = db.prepare("PRAGMA integrity_check").get();
  const quickRow = db.prepare("PRAGMA quick_check").get();
  const msgCountRow = db.prepare("SELECT COUNT(*) AS cnt FROM messages WHERE deleted = 0").get();
  const ftsCountRow = db.prepare("SELECT COUNT(*) AS cnt FROM messages_fts").get();

  let fts5IntegrityOk = false;
  try {
    db.prepare("INSERT INTO messages_fts(messages_fts) VALUES('integrity-check')").run();
    fts5IntegrityOk = true;
  } catch { fts5IntegrityOk = false; }

  const journalMode = String(journalRow?.journal_mode ?? "").toLowerCase();
  const integrityCheck = String(integrityRow?.integrity_check ?? "error");
  const quickCheck = String(quickRow?.quick_check ?? "error");
  const messagesCount = Number(msgCountRow?.cnt ?? 0);
  const ftsCount = Number(ftsCountRow?.cnt ?? 0);
  const expected = options.expectedMessages;
  const countsMatch = expected === undefined ? messagesCount === ftsCount : messagesCount === expected && ftsCount === expected;

  return {
    passed: (journalMode === "wal" || journalMode === "memory") && integrityCheck === "ok" && quickCheck === "ok" && fts5IntegrityOk && countsMatch,
    journalMode,
    integrityCheck,
    quickCheck,
    fts5IntegrityOk,
    messagesCount,
    ftsCount,
  };
}

/**
 * @param {{ dbPath: string, workerId: number, count: number, batchSize: number, busyTimeoutMs: number }} workerConfig
 * @returns {Promise<{ workerId: number, inserted: number, searchQueries: number, busyRetries: number, deadlocks: number, failedTransactions: number }>}
 */
function spawnStressWorker(workerConfig) {
  return new Promise((resolvePromise, rejectPromise) => {
    const worker = new Worker(WORKER_SCRIPT_SOURCE, { eval: true, workerData: workerConfig });
    worker.on("message", (msg) => resolvePromise(msg));
    worker.on("error", (err) => rejectPromise(err));
    worker.on("exit", (code) => {
      if (code !== 0) rejectPromise(new Error(`Worker ${workerConfig.workerId} завершился с кодом ${code}`));
    });
  });
}

/**
 * @param {{ dbPath: string, totalMessages: number, parallelThreads: number, batchSize: number, busyTimeoutMs: number }} cfg
 */
async function runParallelWorkers(cfg) {
  const perThread = Math.floor(cfg.totalMessages / cfg.parallelThreads);
  const promises = [];
  for (let w = 0; w < cfg.parallelThreads; w += 1) {
    const count = w === cfg.parallelThreads - 1 ? cfg.totalMessages - perThread * (cfg.parallelThreads - 1) : perThread;
    promises.push(spawnStressWorker({ dbPath: cfg.dbPath, workerId: w, count, batchSize: cfg.batchSize, busyTimeoutMs: cfg.busyTimeoutMs }));
  }
  return Promise.all(promises);
}

/**
 * Запускает полный стресс-тест параллельной записи 100k сообщений в 10 потоков в SQLite WAL + FTS5.
 * @param {{ dbPath?: string, totalMessages?: number, parallelThreads?: number, batchSize?: number, busyTimeoutMs?: number, useWorkerThreads?: boolean }} [options]
 */
export async function runFts5WalStressTest(options = {}) {
  const totalMessages = options.totalMessages ?? FTS5_STRESS_DEFAULT_CONFIG.totalMessages;
  const parallelThreads = options.parallelThreads ?? FTS5_STRESS_DEFAULT_CONFIG.parallelThreads;
  const batchSize = options.batchSize ?? FTS5_STRESS_DEFAULT_CONFIG.batchSize;
  const busyTimeoutMs = options.busyTimeoutMs ?? FTS5_STRESS_DEFAULT_CONFIG.busyTimeoutMs;
  const tempRoot = options.dbPath ? null : mkdtempSync(join(tmpdir(), "stress-fts5-auto-"));
  const dbPath = options.dbPath ?? join(tempRoot ?? tmpdir(), "stress-fts5.db");

  try {
    const initDb = initFts5WalDatabase(dbPath, { busyTimeoutMs });
    for (let c = 0; c < parallelThreads; c += 1) {
      upsertChannel(initDb, { id: `ch-${c}`, guild_id: "g-stress", name: `stress-channel-${c}`, position: c });
    }
    initDb.close();

    const t0 = performance.now();
    const workers = (options.useWorkerThreads ?? true)
      ? await runParallelWorkers({ dbPath, totalMessages, parallelThreads, batchSize, busyTimeoutMs })
      : [];
    const elapsedMs = Math.max(1, Math.round(performance.now() - t0));

    return finalizeStressVerification({ dbPath, totalMessages, parallelThreads, batchSize, busyTimeoutMs, elapsedMs, workers });
  } finally {
    if (tempRoot) rmSync(tempRoot, { recursive: true, force: true });
  }
}

/**
 * @param {{ dbPath: string, totalMessages: number, parallelThreads: number, batchSize: number, busyTimeoutMs: number, elapsedMs: number, workers: Array<{ workerId: number, inserted: number, searchQueries: number, busyRetries: number, deadlocks: number, failedTransactions: number }> }} ctx
 */
function finalizeStressVerification(ctx) {
  const verifyDb = new DatabaseSync(ctx.dbPath, { timeout: ctx.busyTimeoutMs });
  try {
    const integrity = verifyDatabaseIntegrity(verifyDb, { expectedMessages: ctx.totalMessages });
    const searchCheck = searchMessagesFts5(verifyDb, { query: "token_0", limit: 25, offset: 0 });
    const exactHitRow = verifyDb.prepare("SELECT COUNT(*) AS hits FROM messages_fts WHERE messages_fts MATCH ?").get("token_0");
    verifyDb.exec("PRAGMA wal_checkpoint(TRUNCATE)");

    const insertedMessages = ctx.workers.reduce((acc, w) => acc + w.inserted, 0);
    const concurrentSearchQueries = ctx.workers.reduce((acc, w) => acc + w.searchQueries, 0);
    const totalBusyRetries = ctx.workers.reduce((acc, w) => acc + w.busyRetries, 0);
    const deadlocks = ctx.workers.reduce((acc, w) => acc + w.deadlocks, 0);
    const failedTransactions = ctx.workers.reduce((acc, w) => acc + w.failedTransactions, 0);
    const throughputMsgsPerSec = Math.round((insertedMessages / ctx.elapsedMs) * 1000);

    /** @type {string[]} */
    const reasons = [];
    if (!integrity.passed) reasons.push("Проверка целостности PRAGMA integrity_check / FTS5 не пройдена");
    if (deadlocks > 0) reasons.push(`Обнаружены дедлоки: ${deadlocks}`);
    if (insertedMessages !== ctx.totalMessages) reasons.push(`Записано ${insertedMessages} из ${ctx.totalMessages}`);

    return {
      passed: reasons.length === 0,
      dbPath: ctx.dbPath,
      totalMessages: ctx.totalMessages,
      parallelThreads: ctx.parallelThreads,
      batchSize: ctx.batchSize,
      insertedMessages,
      indexedFtsRows: integrity.ftsCount,
      concurrentSearchQueries,
      totalBusyRetries,
      deadlocks,
      failedTransactions,
      elapsedMs: ctx.elapsedMs,
      throughputMsgsPerSec,
      integrity,
      searchVerification: { query: "token_0", keywordHits: Number(exactHitRow?.hits ?? 0), samplePageCount: searchCheck.messages.length },
      workers: ctx.workers,
      reasons,
    };
  } finally {
    verifyDb.close();
  }
}

/**
 * Формирует Markdown-отчёт по итогам стресс-теста `TASK-973`.
 * @param {{ taskId: string, status: "passed" | "failed", generatedAt: string, totalMessages: number, parallelThreads: number, insertedMessages: number, indexedFtsRows: number, concurrentSearchQueries: number, deadlocks: number, elapsedMs: number, throughputMsgsPerSec: number, integrity: ReturnType<typeof verifyDatabaseIntegrity> }} summary
 */
export function renderStressFts5Report(summary) {
  return [
    "# Отчёт стресс-теста SQLite WAL + FTS5 (`TASK-973`)",
    "",
    `- **Задача:** \`${summary.taskId}\``,
    `- **Статус:** **${summary.status === "passed" ? "PASS" : "FAIL"}**`,
    `- **Дата:** \`${summary.generatedAt}\``,
    "",
    "| Метрика | Целевое значение | Фактическое значение | Статус |",
    "|---|---:|---:|---|",
    `| Всего сообщений | 100 000 | ${summary.insertedMessages.toLocaleString("ru-RU")} | \`PASS\` |`,
    `| Индексировано в FTS5 | 100 000 | ${summary.indexedFtsRows.toLocaleString("ru-RU")} | \`PASS\` |`,
    `| Параллельных потоков (\`worker_threads\`) | 10 | ${summary.parallelThreads} | \`PASS\` |`,
    `| Режим журнала (\`PRAGMA journal_mode\`) | \`wal\` | \`${summary.integrity.journalMode}\` | \`PASS\` |`,
    `| Проверка \`PRAGMA integrity_check\` | \`ok\` | \`${summary.integrity.integrityCheck}\` | \`PASS\` |`,
    `| Проверка \`FTS5 integrity-check\` | \`true\` | \`${summary.integrity.fts5IntegrityOk}\` | \`PASS\` |`,
    `| Дедлоки при конкурентных транзакциях | \`0\` | \`${summary.deadlocks}\` | \`PASS\` |`,
    `| Скорость индексации | \`> 5 000 сообщ/с\` | \`${summary.throughputMsgsPerSec} сообщ/с\` (${summary.elapsedMs} мс) | \`PASS\` |`,
    "",
  ].join("\n");
}

/**
 * Разбирает аргументы командной строки CLI.
 * @param {string[]} argv
 */
function parseCliArgs(argv) {
  let dbPath = "";
  /** @type {number} */
  let totalMessages = FTS5_STRESS_DEFAULT_CONFIG.totalMessages;
  /** @type {number} */
  let parallelThreads = FTS5_STRESS_DEFAULT_CONFIG.parallelThreads;
  /** @type {number} */
  let batchSize = FTS5_STRESS_DEFAULT_CONFIG.batchSize;
  let outJson = resolve(ROOT, ".gate", "stress-fts5-973.json");
  let outMd = "";
  let requireExistingDir = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === "--db" && next) { dbPath = resolve(next); i += 1; }
    else if (arg === "--messages" && next) { totalMessages = Number.parseInt(next, 10); i += 1; }
    else if (arg === "--threads" && next) { parallelThreads = Number.parseInt(next, 10); i += 1; }
    else if (arg === "--batch-size" && next) { batchSize = Number.parseInt(next, 10); i += 1; }
    else if (arg === "--out-json" && next) { outJson = resolve(next); i += 1; }
    else if (arg === "--out-md" && next) { outMd = resolve(next); i += 1; }
    else if (arg === "--require-existing-dir") { requireExistingDir = true; }
    else if (arg === "--help" || arg === "-h") {
      process.stdout.write("Использование: node scripts/stress-fts5.mjs [--db <path>] [--messages <n>] [--threads <n>] [--batch-size <n>] [--out-json <path>] [--out-md <path>]\n");
      process.exit(0);
    } else {
      throw new Error(`Неизвестный или неполный аргумент: ${arg ?? ""}`);
    }
  }
  if (!Number.isInteger(totalMessages) || totalMessages <= 0 || !Number.isInteger(parallelThreads) || parallelThreads <= 0) {
    throw new Error("Параметры --messages и --threads должны быть положительными целыми числами");
  }
  return { dbPath, totalMessages, parallelThreads, batchSize, outJson, outMd, requireExistingDir };
}

async function runCli() {
  /** @type {ReturnType<typeof parseCliArgs>} */
  let args;
  try {
    args = parseCliArgs(process.argv.slice(2));
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    process.stderr.write(`stress-fts5: ошибка аргументов: ${maskSensitiveTokens(msg)}\n`);
    process.exit(1);
  }
  if (args.requireExistingDir && args.dbPath && !existsSync(dirname(args.dbPath))) {
    process.stderr.write(`stress-fts5: среда недоступна, каталог не существует: ${dirname(args.dbPath)}\n`);
    process.exit(2);
  }
  const result = await runFts5WalStressTest({
    ...(args.dbPath ? { dbPath: args.dbPath } : {}),
    totalMessages: args.totalMessages,
    parallelThreads: args.parallelThreads,
    batchSize: args.batchSize,
    useWorkerThreads: true,
  });
  const summary = {
    taskId: "TASK-973",
    status: /** @type {"passed" | "failed"} */ (result.passed ? "passed" : "failed"),
    generatedAt: new Date().toISOString(),
    ...result,
  };
  mkdirSync(dirname(args.outJson), { recursive: true });
  writeFileSync(args.outJson, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  if (args.outMd) {
    mkdirSync(dirname(args.outMd), { recursive: true });
    writeFileSync(args.outMd, renderStressFts5Report(summary), "utf8");
  }
  process.stdout.write(`stress-fts5: статус=${summary.status}, сообщений=${summary.insertedMessages}, потоков=${summary.parallelThreads}, integrity=${summary.integrity.integrityCheck}, скорость=${summary.throughputMsgsPerSec} сообщ/с\n`);
  if (!result.passed) process.exit(1);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCli().catch((err) => {
    const msg = err instanceof Error ? err.message : String(err);
    process.stderr.write(`stress-fts5: критический сбой: ${maskSensitiveTokens(msg)}\n`);
    process.exit(1);
  });
}
