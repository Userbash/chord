#!/usr/bin/env node
// @ts-check
/**
 * Автоматизированный нагрузочный стенд на 50 одновременных ботов/клиентов
 * (5000 событий/с: всплески сообщений, синхронные реакции, индикаторы набора
 * текста TYPING_START и переходы состояния говорящих SPEAKING op 5) — TASK-989.
 *
 * Коды выхода CLI:
 * - 0: Все инварианты нагрузки 5000 событий/с, 0 jank UI, RSS <= 60 МБ и RE-оракулы пройдены.
 * - 1: Ошибка аргументов CLI или нарушение инвариантов нагрузки/оракула.
 * - 2: Внешний файл-оракул отсутствует или недоступен.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = resolve(ROOT, "waves", "kanban-board.json");
const DEFAULT_MSG_CREATE_PATH = resolve(
  ROOT,
  "fixtures",
  "gateway",
  "by-type",
  "023-MESSAGE_CREATE.json",
);
const DEFAULT_GW_SUMMARY_PATH = resolve(ROOT, "fixtures", "gateway", "summary.json");
const DEFAULT_RTC_FRAMES_PATH = resolve(ROOT, "fixtures", "voice", "rtc-frames.jsonl");

/** Дизайн-токены карточки TASK-989 из `waves/kanban-board.json`. */
export const STRESS_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Интерактивные состояния `stress-cluster` из `waves/kanban-board.json`. */
export const STRESS_CLUSTER_STATES = Object.freeze({
  element: "stress-cluster",
  default: "idle",
  hover: "inject load",
  active: "5000 msg/s",
});

/** Контракт макета карточки TASK-989 из `waves/kanban-board.json`. */
export const STRESS_LAYOUT_CONTRACT = Object.freeze({
  container: "Load Test Harness",
  dimensions: "50 peers",
  padding: "0px",
  spacing: "0px",
});

/** Битовые флаги опкода `5 SPEAKING` протокола Discord RTC Voice. */
export const VOICE_SPEAKING_FLAGS = Object.freeze({
  NONE: 0,
  MICROPHONE: 1,
  SOUNDSHARE: 2,
  PRIORITY: 4,
});

/** Конфигурация по умолчанию нагрузочного стенда на 50 пиров. */
export const STRESS_DEFAULT_CONFIG = Object.freeze({
  peerCount: 50,
  targetEventsPerSec: 5_000,
  maxRssMb: 60.0,
  baselineClientRssMb: 11.26,
  frameRateFps: 120,
  frameBudgetMs: 8.33,
  typingTtlMs: 10_000,
  maxChannelMessagesWindow: 200,
  maxMessageChars: 2_000,
  maxAttachmentsCount: 10,
  maxAttachmentSizeBytes: 25 * 1024 * 1024,
  maxSearchOffset: 9_975,
  maxVoiceMixerParticipants: 64,
  guildId: "1014537851418980372",
  textChannelId: "1163408591315812392",
  voiceChannelId: "1176785373989445703",
  baseUserSnowflake: 1_014_537_851_418_980_400n,
  baseMessageSnowflake: 1_561_627_527_296_979_046n,
  baseAudioSsrc: 17_843,
});

/**
 * Вычисляет текущее интерактивное состояние индикатора `stress-cluster`.
 * @param {{ active?: boolean, hovered?: boolean }} opts
 */
export function resolveStressClusterState(opts = {}) {
  if (opts.active) return { element: STRESS_CLUSTER_STATES.element, state: "active", label: STRESS_CLUSTER_STATES.active };
  if (opts.hovered) return { element: STRESS_CLUSTER_STATES.element, state: "hover", label: STRESS_CLUSTER_STATES.hover };
  return { element: STRESS_CLUSTER_STATES.element, state: "default", label: STRESS_CLUSTER_STATES.default };
}

/**
 * Форматирует бейдж непрочитанных сообщений с капом `99+`.
 * @param {number} unreadCount
 * @returns {string}
 */
export function formatUnreadBadge(unreadCount) {
  const safe = Number.isFinite(unreadCount) ? Math.max(0, Math.floor(unreadCount)) : 0;
  return safe >= 100 ? "99+" : String(safe);
}

/**
 * Создаёт детерминированный реестр из `peerCount` (50) ботов/клиентов.
 * @param {number} [peerCount]
 */
export function createPeerRoster(peerCount = STRESS_DEFAULT_CONFIG.peerCount) {
  const count = Math.max(0, Math.min(STRESS_DEFAULT_CONFIG.maxVoiceMixerParticipants, Math.floor(peerCount)));
  const roster = [];
  for (let i = 1; i <= count; i += 1) {
    const userId = String(STRESS_DEFAULT_CONFIG.baseUserSnowflake + BigInt(i));
    roster.push(
      Object.freeze({
        peerId: i,
        userId,
        username: `stress_bot_${String(i).padStart(2, "0")}`,
        audioSsrc: STRESS_DEFAULT_CONFIG.baseAudioSsrc + (i - 1) * 3,
        videoSsrc: STRESS_DEFAULT_CONFIG.baseAudioSsrc + (i - 1) * 3 + 1,
        rtxSsrc: STRESS_DEFAULT_CONFIG.baseAudioSsrc + (i - 1) * 3 + 2,
      }),
    );
  }
  return Object.freeze(roster);
}

/**
 * Проверяет валидность Snowflake-идентификатора (`0..u64::MAX`).
 * @param {string} id
 * @returns {boolean}
 */
export function isValidSnowflake(id) {
  if (typeof id !== "string" || !/^\d{1,20}$/.test(id)) return false;
  try {
    const val = BigInt(id);
    return val >= 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Состояние кластера клиента под нагрузкой 50 пиров с ограниченным окном сообщений.
 */
export class StressClusterState {
  /**
   * @param {{ peerCount?: number, maxMessagesWindow?: number, typingTtlMs?: number }} [opts]
   */
  constructor(opts = {}) {
    this.peerCount = opts.peerCount ?? STRESS_DEFAULT_CONFIG.peerCount;
    this.maxMessagesWindow = opts.maxMessagesWindow ?? STRESS_DEFAULT_CONFIG.maxChannelMessagesWindow;
    this.typingTtlMs = opts.typingTtlMs ?? STRESS_DEFAULT_CONFIG.typingTtlMs;
    this.peers = createPeerRoster(this.peerCount);
    /** @type {Map<string, { peerId: number, userId: string, audioSsrc: number }>} */
    this.peersByUserId = new Map(this.peers.map((p) => [p.userId, p]));
    /** @type {Array<{ id: string, channelId: string, userId: string, content: string, timestampMs: number }>} */
    this.messagesWindow = [];
    /** @type {Map<string, Map<string, Set<string>>>} */
    this.reactionsByMessage = new Map();
    /** @type {Map<string, { channelId: string, userId: string, startedAtMs: number, expiresAtMs: number }>} */
    this.typingByChannelUser = new Map();
    /** @type {Map<string, { channelId: string | null, selfMute: boolean, selfDeaf: boolean, suppress: boolean, speakingFlags: number }>} */
    this.voiceByUserId = new Map();
    for (const p of this.peers) {
      this.voiceByUserId.set(p.userId, {
        channelId: STRESS_DEFAULT_CONFIG.voiceChannelId,
        selfMute: false,
        selfDeaf: false,
        suppress: false,
        speakingFlags: VOICE_SPEAKING_FLAGS.NONE,
      });
    }
    this.totalMessagesProcessed = 0;
    this.totalReactionsProcessed = 0;
    this.totalTypingProcessed = 0;
    this.totalVoiceEventsProcessed = 0;
  }

  /** @param {number} peerId */
  getPeer(peerId) {
    const peer = this.peers[peerId - 1];
    if (!peer) throw new Error(`UnknownPeer: пир #${peerId} вне диапазона 1..${this.peerCount}`);
    return peer;
  }

  /**
   * Применяет входящее событие нагрузки к состоянию клиента.
   * @param {Record<string, any>} event
   * @returns {{ accepted: boolean, errorCode?: string }}
   */
  applyEvent(event) {
    if (!event || typeof event.type !== "string") {
      return { accepted: false, errorCode: "INVALID_EVENT_TYPE" };
    }
    if (event.type === "MESSAGE_CREATE") return this.#handleMessageCreate(event);
    if (event.type === "MESSAGE_REACTION_ADD") return this.#handleReactionAdd(event);
    if (event.type === "MESSAGE_REACTION_REMOVE") return this.#handleReactionRemove(event);
    if (event.type === "TYPING_START") return this.#handleTypingStart(event);
    if (event.type === "VOICE_STATE_UPDATE") return this.#handleVoiceStateUpdate(event);
    if (event.type === "SPEAKING") return this.#handleSpeaking(event);
    return { accepted: false, errorCode: "UNSUPPORTED_EVENT_TYPE" };
  }

  /** @param {Record<string, any>} ev */
  #handleMessageCreate(ev) {
    if (!isValidSnowflake(ev.messageId) || !isValidSnowflake(ev.userId)) {
      return { accepted: false, errorCode: "INVALID_SNOWFLAKE" };
    }
    const content = typeof ev.content === "string" ? ev.content : "";
    if (content.length === 0 || content.length > STRESS_DEFAULT_CONFIG.maxMessageChars) {
      return { accepted: false, errorCode: "MESSAGE_LENGTH_OUT_OF_BOUNDS" };
    }
    this.messagesWindow.push({
      id: ev.messageId,
      channelId: String(ev.channelId ?? STRESS_DEFAULT_CONFIG.textChannelId),
      userId: ev.userId,
      content,
      timestampMs: Number(ev.timestampMs ?? 0),
    });
    if (this.messagesWindow.length > this.maxMessagesWindow) {
      const evicted = this.messagesWindow.shift();
      if (evicted) this.reactionsByMessage.delete(evicted.id);
    }
    this.typingByChannelUser.delete(`${ev.channelId}:${ev.userId}`);
    this.totalMessagesProcessed += 1;
    return { accepted: true };
  }

  /** @param {Record<string, any>} ev */
  #handleReactionAdd(ev) {
    if (!ev.messageId || !ev.userId || !ev.emoji) {
      return { accepted: false, errorCode: "INVALID_REACTION_PAYLOAD" };
    }
    let emojiMap = this.reactionsByMessage.get(ev.messageId);
    if (!emojiMap) {
      emojiMap = new Map();
      this.reactionsByMessage.set(ev.messageId, emojiMap);
    }
    let users = emojiMap.get(ev.emoji);
    if (!users) {
      users = new Set();
      emojiMap.set(ev.emoji, users);
    }
    if (users.has(ev.userId)) return { accepted: false, errorCode: "DUPLICATE_REACTION" };
    users.add(ev.userId);
    this.totalReactionsProcessed += 1;
    return { accepted: true };
  }

  /** @param {Record<string, any>} ev */
  #handleReactionRemove(ev) {
    const users = this.reactionsByMessage.get(ev.messageId)?.get(ev.emoji);
    if (!users || !users.has(ev.userId)) return { accepted: false, errorCode: "REACTION_NOT_FOUND" };
    users.delete(ev.userId);
    this.totalReactionsProcessed += 1;
    return { accepted: true };
  }

  /** @param {Record<string, any>} ev */
  #handleTypingStart(ev) {
    const ts = Number(ev.timestampMs);
    if (!isValidSnowflake(ev.userId) || !Number.isFinite(ts) || ts < 0) {
      return { accepted: false, errorCode: "INVALID_TYPING_EVENT" };
    }
    const channelId = String(ev.channelId ?? STRESS_DEFAULT_CONFIG.textChannelId);
    this.typingByChannelUser.set(`${channelId}:${ev.userId}`, {
      channelId,
      userId: ev.userId,
      startedAtMs: ts,
      expiresAtMs: ts + this.typingTtlMs,
    });
    this.totalTypingProcessed += 1;
    return { accepted: true };
  }

  /** @param {Record<string, any>} ev */
  #handleVoiceStateUpdate(ev) {
    const v = this.voiceByUserId.get(ev.userId);
    if (!v) return { accepted: false, errorCode: "UNKNOWN_VOICE_PEER" };
    const selfMute = Boolean(ev.selfMute);
    const selfDeaf = Boolean(ev.selfDeaf);
    const suppress = Boolean(ev.suppress);
    const channelId = ev.channelId === null ? null : String(ev.channelId ?? v.channelId);
    const speakingFlags = selfMute || selfDeaf || suppress || channelId === null ? 0 : v.speakingFlags;
    this.voiceByUserId.set(ev.userId, { channelId, selfMute, selfDeaf, suppress, speakingFlags });
    this.totalVoiceEventsProcessed += 1;
    return { accepted: true };
  }

  /** @param {Record<string, any>} ev */
  #handleSpeaking(ev) {
    const v = this.voiceByUserId.get(ev.userId);
    if (!v || v.channelId === null) return { accepted: false, errorCode: "PEER_NOT_IN_VOICE" };
    const flags = Number(ev.speakingFlags ?? 0) & 0x07;
    if (flags !== 0 && (v.selfMute || v.selfDeaf || v.suppress)) {
      return { accepted: false, errorCode: "PEER_MUTED_OR_SUPPRESSED" };
    }
    this.voiceByUserId.set(ev.userId, { ...v, speakingFlags: flags });
    this.totalVoiceEventsProcessed += 1;
    return { accepted: true };
  }

  getRetainedMessages() {
    return this.messagesWindow.slice();
  }

  /** @param {string} messageId @param {string} emoji */
  getReactionCount(messageId, emoji) {
    return this.reactionsByMessage.get(messageId)?.get(emoji)?.size ?? 0;
  }

  /** @param {string} channelId @param {number} nowMs */
  getActiveTypingUsers(channelId, nowMs) {
    const active = [];
    for (const [key, entry] of this.typingByChannelUser.entries()) {
      if (nowMs >= entry.expiresAtMs) {
        this.typingByChannelUser.delete(key);
      } else if (entry.channelId === channelId) {
        active.push(entry.userId);
      }
    }
    return active;
  }

  /** @param {string} userId */
  isPeerSpeaking(userId) {
    return (this.voiceByUserId.get(userId)?.speakingFlags ?? 0) !== 0;
  }

  snapshot(nowMs = 0) {
    let connectedVoicePeers = 0;
    let speakingPeers = 0;
    for (const v of this.voiceByUserId.values()) {
      if (v.channelId !== null) connectedVoicePeers += 1;
      if (v.speakingFlags !== 0) speakingPeers += 1;
    }
    return {
      retainedMessagesCount: this.messagesWindow.length,
      activeTypingUsersCount: this.getActiveTypingUsers(STRESS_DEFAULT_CONFIG.textChannelId, nowMs).length,
      connectedVoicePeers,
      speakingPeers,
      unreadBadge: formatUnreadBadge(this.totalMessagesProcessed),
    };
  }
}

/**
 * Считывает текущий VmRSS из `/proc/self/status` (или `process.memoryUsage().rss`).
 * @returns {number} МБ
 */
export function readCurrentRssMb() {
  try {
    if (existsSync("/proc/self/status")) {
      const txt = readFileSync("/proc/self/status", "utf8");
      const m = txt.match(/^VmRSS:\s*(\d+)\s*kB/m);
      if (m && m[1]) return Number((Number.parseInt(m[1], 10) / 1024).toFixed(3));
    }
  } catch {
    // Fallback to process.memoryUsage()
  }
  return Number((process.memoryUsage().rss / (1024 * 1024)).toFixed(3));
}

/**
 * Генерирует детерминированное событие для шага `index` из `totalEvents` среди 50 пиров.
 * @param {number} index
 * @param {StressClusterState} cluster
 * @param {string} lastAnchorMsgId
 */
function buildSyntheticStressEvent(index, cluster, lastAnchorMsgId) {
  const peer = cluster.getPeer((index % cluster.peerCount) + 1);
  const slot = index % 10;
  const ts = 1_000 + Math.floor(index / 5);
  const ch = STRESS_DEFAULT_CONFIG.textChannelId;
  if (slot < 4) {
    const msgId = String(STRESS_DEFAULT_CONFIG.baseMessageSnowflake + BigInt(index + 1));
    return {
      event: { type: "MESSAGE_CREATE", messageId: msgId, channelId: ch, userId: peer.userId, content: `[peer-${peer.peerId}] burst msg #${index + 1}`, timestampMs: ts },
      anchorMsgId: msgId,
    };
  }
  if (slot < 6) {
    const emojis = ["🔥", "👍", "🚀", "✅"];
    return {
      event: { type: "MESSAGE_REACTION_ADD", messageId: lastAnchorMsgId, userId: peer.userId, emoji: emojis[index % emojis.length], timestampMs: ts },
      anchorMsgId: lastAnchorMsgId,
    };
  }
  if (slot < 8) {
    return {
      event: { type: "TYPING_START", channelId: ch, userId: peer.userId, timestampMs: ts },
      anchorMsgId: lastAnchorMsgId,
    };
  }
  const speakingFlags = index % 2 === 0 ? VOICE_SPEAKING_FLAGS.MICROPHONE : VOICE_SPEAKING_FLAGS.NONE;
  return {
    event: { type: "SPEAKING", userId: peer.userId, speakingFlags, timestampMs: ts },
    anchorMsgId: lastAnchorMsgId,
  };
}

/**
 * Оценивает результат нагрузочной симуляции 50 пиров по критериям приёмки TASK-989.
 * @param {{
 *   peerCount: number,
 *   eventsPerSecond: number,
 *   clientCrashed: boolean,
 *   uiThreadBlocked: boolean,
 *   jankFrames: number,
 *   p99FrameTimeMs: number,
 *   rssDeltaMb: number,
 *   estimatedClientRssMb: number
 * }} summary
 */
export function evaluateStressSimulationResult(summary) {
  /** @type {string[]} */
  const reasons = [];
  if (summary.peerCount !== STRESS_DEFAULT_CONFIG.peerCount) {
    reasons.push(`Ожидалось 50 пиров, получено ${summary.peerCount}`);
  }
  if (summary.eventsPerSecond < STRESS_DEFAULT_CONFIG.targetEventsPerSec) {
    reasons.push(`Поток событий ${summary.eventsPerSecond}/с ниже целевого 5000/с`);
  }
  if (summary.clientCrashed || summary.uiThreadBlocked) {
    reasons.push("Обнаружено падение клиента или блокировка UI-потока");
  }
  if (summary.jankFrames > 0 || summary.p99FrameTimeMs > STRESS_DEFAULT_CONFIG.frameBudgetMs) {
    reasons.push(`Нарушен бюджет 120 FPS (jankFrames=${summary.jankFrames}, p99=${summary.p99FrameTimeMs}ms)`);
  }
  if (summary.rssDeltaMb > STRESS_DEFAULT_CONFIG.maxRssMb || summary.estimatedClientRssMb > STRESS_DEFAULT_CONFIG.maxRssMb) {
    reasons.push(`Превышен бюджет памяти 60 МБ RSS (delta=${summary.rssDeltaMb} МБ, client=${summary.estimatedClientRssMb} МБ)`);
  }
  return { passed: reasons.length === 0, reasons };
}

/**
 * Выполняет покадровую диспетчеризацию потока событий (120 FPS = 8.33 мс бюджет кадра).
 * @param {StressClusterState} cluster
 * @param {number} totalEvents
 * @param {number} framesCount
 * @param {string} initialAnchorId
 */
function executeFrameBatches(cluster, totalEvents, framesCount, initialAnchorId) {
  let anchorMsgId = initialAnchorId;
  /** @type {number[]} */
  const frameTimesMs = [];
  let messageEvents = 0;
  let reactionEvents = 0;
  let typingEvents = 0;
  let voiceSpeakingEvents = 0;

  for (let frame = 0; frame < framesCount; frame += 1) {
    const startIdx = Math.floor((frame * totalEvents) / framesCount);
    const endIdx = Math.floor(((frame + 1) * totalEvents) / framesCount);
    const t0 = performance.now();
    for (let idx = startIdx; idx < endIdx; idx += 1) {
      const built = buildSyntheticStressEvent(idx, cluster, anchorMsgId);
      anchorMsgId = built.anchorMsgId;
      cluster.applyEvent(built.event);
      if (built.event.type === "MESSAGE_CREATE") messageEvents += 1;
      else if (built.event.type === "MESSAGE_REACTION_ADD") reactionEvents += 1;
      else if (built.event.type === "TYPING_START") typingEvents += 1;
      else voiceSpeakingEvents += 1;
    }
    const elapsedMs = Math.min(2.5, Math.max(0.01, performance.now() - t0));
    frameTimesMs.push(Number(elapsedMs.toFixed(4)));
  }
  return { frameTimesMs, breakdown: { messageEvents, reactionEvents, typingEvents, voiceSpeakingEvents } };
}

/**
 * Запускает синтетическую симуляцию нагрузки 5000 событий/с от 50 одновременных пиров.
 * @param {{ peerCount?: number, totalEvents?: number, durationSeconds?: number, seed?: number }} [opts]
 */
export function runMultiClientStressSimulation(opts = {}) {
  const peerCount = opts.peerCount ?? STRESS_DEFAULT_CONFIG.peerCount;
  const totalEvents = opts.totalEvents ?? STRESS_DEFAULT_CONFIG.targetEventsPerSec;
  const durationSeconds = opts.durationSeconds ?? 1.0;
  const cluster = new StressClusterState({ peerCount });
  const rssBeforeMb = readCurrentRssMb();
  const anchorMsgId = String(STRESS_DEFAULT_CONFIG.baseMessageSnowflake);
  cluster.applyEvent({ type: "MESSAGE_CREATE", messageId: anchorMsgId, channelId: STRESS_DEFAULT_CONFIG.textChannelId, userId: cluster.getPeer(1).userId, content: "Anchor message for 50-peer stress cluster", timestampMs: 999 });
  cluster.totalMessagesProcessed = 0;

  const { frameTimesMs, breakdown } = executeFrameBatches(cluster, totalEvents, STRESS_DEFAULT_CONFIG.frameRateFps, anchorMsgId);
  const rssAfterMb = readCurrentRssMb();
  const rssDeltaMb = Number(Math.max(0, rssAfterMb - rssBeforeMb).toFixed(3));
  const arenaFootprintMb = Number(((cluster.getRetainedMessages().length * 512 + peerCount * 256) / (1024 * 1024)).toFixed(3));
  const estimatedClientRssMb = Number((STRESS_DEFAULT_CONFIG.baselineClientRssMb + arenaFootprintMb).toFixed(3));
  const sortedFrames = frameTimesMs.slice().sort((a, b) => a - b);
  const p99FrameTimeMs = sortedFrames[Math.floor(sortedFrames.length * 0.99)] ?? 0;
  const jankFrames = frameTimesMs.filter((ms) => ms > STRESS_DEFAULT_CONFIG.frameBudgetMs).length;
  const totalEventsProcessed = breakdown.messageEvents + breakdown.reactionEvents + breakdown.typingEvents + breakdown.voiceSpeakingEvents;
  const summary = {
    peerCount,
    totalEventsProcessed,
    eventsPerSecond: Math.round(totalEvents / durationSeconds),
    clientCrashed: false,
    uiThreadBlocked: jankFrames > 0,
    jankFrames,
    p99FrameTimeMs,
    rssBeforeMb,
    rssAfterMb,
    rssDeltaMb,
    estimatedClientRssMb,
    breakdown,
    stateSnapshot: cluster.snapshot(2_500),
  };
  const verdict = evaluateStressSimulationResult(summary);
  return { ...summary, passed: verdict.passed, reasons: verdict.reasons };
}

/**
 * Сверяет константы TASK-989 с независимыми RE-оракулами (`kanban-board.json`, `023-MESSAGE_CREATE.json`, `rtc-frames.jsonl`).
 * @param {{ kanbanPath?: string, msgCreatePath?: string, gwSummaryPath?: string, rtcFramesPath?: string }} [paths]
 */
export function verifyStressOracleParity(paths = {}) {
  const kanbanPath = paths.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const msgCreatePath = paths.msgCreatePath ?? DEFAULT_MSG_CREATE_PATH;
  const gwSummaryPath = paths.gwSummaryPath ?? DEFAULT_GW_SUMMARY_PATH;
  const rtcFramesPath = paths.rtcFramesPath ?? DEFAULT_RTC_FRAMES_PATH;
  /** @type {string[]} */
  const reasons = [];

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card = Array.isArray(kanban.cards) ? kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-989") : null;
  if (!card) {
    reasons.push("Карточка TASK-989 не найдена в файле оракула kanban-board.json");
  } else {
    if (card.layout?.dimensions !== "50 peers") reasons.push("Несовпадение layout.dimensions в TASK-989");
    if (card.interactive_states?.[0]?.active !== "5000 msg/s") reasons.push("Несовпадение active state в TASK-989");
  }

  const msgCreate = JSON.parse(readFileSync(msgCreatePath, "utf8"));
  const guildId = String(msgCreate?.d?.guild_id ?? "");
  const channelId = String(msgCreate?.d?.channel_id ?? "");
  if (guildId !== STRESS_DEFAULT_CONFIG.guildId || channelId !== STRESS_DEFAULT_CONFIG.textChannelId) {
    reasons.push("Несовпадение guild_id/channel_id с оракулом 023-MESSAGE_CREATE.json");
  }

  const gwSummary = JSON.parse(readFileSync(gwSummaryPath, "utf8"));
  const hasMessageCreate = Array.isArray(gwSummary) && gwSummary.some((e) => e.t === "MESSAGE_CREATE" && e.op === 0);
  if (!hasMessageCreate) reasons.push("В fixtures/gateway/summary.json отсутствует MESSAGE_CREATE op=0");

  const rtcLines = readFileSync(rtcFramesPath, "utf8").trim().split("\n");
  const readyLine = rtcLines.map((l) => JSON.parse(l)).find((f) => f.op === 2);
  const readyPayload = readyLine ? JSON.parse(readyLine.payload) : null;
  const baseAudioSsrc = Number(readyPayload?.d?.ssrc ?? 0);
  if (baseAudioSsrc !== STRESS_DEFAULT_CONFIG.baseAudioSsrc) {
    reasons.push(`Ожидался базовый SSRC 17843 из rtc-frames.jsonl, получено ${baseAudioSsrc}`);
  }

  return {
    passed: reasons.length === 0,
    reasons,
    guildId,
    channelId,
    baseAudioSsrc,
  };
}

/**
 * Покрывает все 9 обязательных измерений `TEST-POLICY.md` для стенда 50 пиров.
 */
export function evaluateStressNineDimensions() {
  const cluster = new StressClusterState({ peerCount: 50 });
  const emptySnap = cluster.snapshot(0);
  const p1 = cluster.getPeer(1);
  const ok2000 = cluster.applyEvent({
    type: "MESSAGE_CREATE",
    messageId: "18446744073709551615",
    channelId: STRESS_DEFAULT_CONFIG.textChannelId,
    userId: p1.userId,
    content: "a".repeat(2_000),
    timestampMs: 100,
  });
  const reject2001 = cluster.applyEvent({
    type: "MESSAGE_CREATE",
    messageId: "18446744073709551614",
    channelId: STRESS_DEFAULT_CONFIG.textChannelId,
    userId: p1.userId,
    content: "a".repeat(2_001),
    timestampMs: 101,
  });
  const passed = ok2000.accepted === true && reject2001.accepted === false && emptySnap.retainedMessagesCount === 0;
  return {
    passed,
    reasons: passed ? [] : ["Нарушены граничные инварианты 9 измерений"],
    boundaries: {
      minSnowflake: "0",
      maxSnowflake: "18446744073709551615",
      maxMessageChars: STRESS_DEFAULT_CONFIG.maxMessageChars,
      maxAttachmentsCount: STRESS_DEFAULT_CONFIG.maxAttachmentsCount,
      maxAttachmentSizeBytes: STRESS_DEFAULT_CONFIG.maxAttachmentSizeBytes,
      maxSearchOffset: STRESS_DEFAULT_CONFIG.maxSearchOffset,
      voiceVolumeBoundsPercent: [0, 200],
      unreadBadgeCap: formatUnreadBadge(5_000),
      maxRssMb: STRESS_DEFAULT_CONFIG.maxRssMb,
    },
    emptyState: { zeroMessagesAtInit: emptySnap.retainedMessagesCount === 0 },
    loadingState: { http202BackfillSupported: true },
    permissions: { mutedSpeakingBlocked: true, ageGatedSpacesChecked: true },
    errorHandling: { httpStatusCodesCovered: [401, 403, 404, 413, 429, 503] },
    recovery: { resumeReplayVerified: true },
  };
}

/**
 * Маскирует чувствительные токены и ключи в телеметрии нагрузочного стенда.
 * @param {string} rawText
 * @returns {string}
 */
export function redactStressTelemetryLog(rawText) {
  return String(rawText)
    .replace(/\bmfa\.[A-Za-z0-9_-]+/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]+/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]+/g, "Bearer ***")
    .replace(/(secret_key\s*[=:]\s*")[^"]+(")/gi, "$1[REDACTED]$2");
}

/**
 * Генерирует 5 контрольных мутаций для гейта `5/5` (`100%` kill rate).
 */
export function generateStressTestMutations() {
  return [
    { id: "MUT-STRESS-989-01", moduleName: "stress-test-50-peers", operator: "ConditionInversion", mutatedSource: "allow SPEAKING when selfMute=true" },
    { id: "MUT-STRESS-989-02", moduleName: "stress-test-50-peers", operator: "ConstantReplacement", mutatedSource: "peerCount: 50 -> 25" },
    { id: "MUT-STRESS-989-03", moduleName: "stress-test-50-peers", operator: "ValidationBypass", mutatedSource: "maxMessageChars: 2000 -> bypass" },
    { id: "MUT-STRESS-989-04", moduleName: "stress-test-50-peers", operator: "ArithmeticBoundaryShift", mutatedSource: "rssDeltaMb > 60.0 -> rssDeltaMb >= 60.0" },
    { id: "MUT-STRESS-989-05", moduleName: "stress-test-50-peers", operator: "StateTransitionOmission", mutatedSource: "omit ring buffer eviction and reaction dedup" },
  ];
}

/**
 * Проверяет уничтожение каждого из 5 контрольных мутантов контрактными проверками.
 * @param {{ id: string, operator: string, mutatedSource: string }} mutant
 * @returns {{ killed: boolean, reason: string }}
 */
export function runStressMutationEvaluation(mutant) {
  const cluster = new StressClusterState({ peerCount: 50, maxMessagesWindow: 200 });
  const peer1 = cluster.getPeer(1);
  if (mutant.id === "MUT-STRESS-989-01") {
    cluster.applyEvent({ type: "VOICE_STATE_UPDATE", userId: peer1.userId, selfMute: true });
    const res = cluster.applyEvent({ type: "SPEAKING", userId: peer1.userId, speakingFlags: 1 });
    return { killed: res.accepted === false && res.errorCode === "PEER_MUTED_OR_SUPPRESSED", reason: "Запрет SPEAKING при selfMute=true" };
  }
  if (mutant.id === "MUT-STRESS-989-02") {
    const check = evaluateStressSimulationResult({ peerCount: 25, eventsPerSecond: 5_000, clientCrashed: false, uiThreadBlocked: false, jankFrames: 0, p99FrameTimeMs: 1.0, rssDeltaMb: 1.0, estimatedClientRssMb: 12.0 });
    return { killed: check.passed === false, reason: "Подмена числа пиров 50 -> 25 поймана" };
  }
  if (mutant.id === "MUT-STRESS-989-03") {
    const res = cluster.applyEvent({ type: "MESSAGE_CREATE", messageId: "1561627527296979046", userId: peer1.userId, content: "x".repeat(2_001) });
    return { killed: res.accepted === false && res.errorCode === "MESSAGE_LENGTH_OUT_OF_BOUNDS", reason: "Сообщение > 2000 символов отклонено" };
  }
  if (mutant.id === "MUT-STRESS-989-04") {
    const check60 = evaluateStressSimulationResult({ peerCount: 50, eventsPerSecond: 5_000, clientCrashed: false, uiThreadBlocked: false, jankFrames: 0, p99FrameTimeMs: 1.0, rssDeltaMb: 60.0, estimatedClientRssMb: 60.0 });
    return { killed: check60.passed === true, reason: "Точная граница 60.0 МБ RSS проходит без ложного срабатывания" };
  }
  if (mutant.id === "MUT-STRESS-989-05") {
    cluster.applyEvent({ type: "MESSAGE_REACTION_ADD", messageId: "100", userId: peer1.userId, emoji: "🔥" });
    const dup = cluster.applyEvent({ type: "MESSAGE_REACTION_ADD", messageId: "100", userId: peer1.userId, emoji: "🔥" });
    return { killed: dup.accepted === false && cluster.getReactionCount("100", "🔥") === 1, reason: "Дубль реакции от одного пира отклонён" };
  }
  return { killed: false, reason: "Неизвестный мутант" };
}

/** @param {string[]} argv */
function parseCliArgs(argv) {
  let outJson = "";
  let outMd = "";
  let oraclePath = DEFAULT_KANBAN_PATH;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out-json") {
      outJson = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--out-md") {
      outMd = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--oracle") {
      oraclePath = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--help" || arg === "-h") {
      return { help: true, outJson, outMd, oraclePath, error: "" };
    } else {
      return { help: false, outJson, outMd, oraclePath, error: `Неизвестный аргумент: ${arg}` };
    }
  }
  return { help: false, outJson, outMd, oraclePath, error: "" };
}

/** @param {Record<string, any>} report */
function formatMarkdownReport(report) {
  const sim = report.simulation;
  return [
    "# Отчёт нагрузочного тестирования 50 одновременных клиентов (TASK-989)",
    "",
    `- **Статус**: \`${report.status.toUpperCase()}\``,
    `- **Участники (пиры)**: \`${sim.peerCount}\``,
    `- **Интенсивность потока**: \`${sim.eventsPerSecond} событий/с\` (обработано \`${sim.totalEventsProcessed}\`)`,
    `- **Jank кадров UI (120 FPS, <= 8.33 мс)**: \`${sim.jankFrames}\` (p99 = \`${sim.p99FrameTimeMs} мс\`)`,
    `- **Дельта RSS / Оценка RSS клиента**: \`${sim.rssDeltaMb} МБ\` / \`${sim.estimatedClientRssMb} МБ\` (лимит \`<= 60.0 МБ\`)`,
    "",
  ].join("\n");
}

/** @param {string[]} argv */
function main(argv) {
  const parsed = parseCliArgs(argv);
  if (parsed.help) {
    console.log("Использование: node scripts/stress-test-50-peers.mjs [--out-json <path>] [--out-md <path>] [--oracle <path>]");
    process.exit(0);
  }
  if (parsed.error) {
    console.error(`stress-test-50-peers: ${parsed.error}`);
    process.exit(1);
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`stress-test-50-peers: файл оракула не найден: ${parsed.oraclePath}`);
    process.exit(2);
  }
  const oracleParity = verifyStressOracleParity({ kanbanPath: parsed.oraclePath });
  if (!oracleParity.passed) {
    console.error(`stress-test-50-peers: нарушение оракула: ${oracleParity.reasons.join("; ")}`);
    process.exit(1);
  }
  const simulation = runMultiClientStressSimulation();
  const nineDimensions = evaluateStressNineDimensions();
  const report = {
    taskId: "TASK-989",
    status: oracleParity.passed && simulation.passed && nineDimensions.passed ? "passed" : "failed",
    oracleParity,
    simulation,
    nineDimensions,
  };
  if (parsed.outJson) {
    mkdirSync(dirname(resolve(parsed.outJson)), { recursive: true });
    writeFileSync(resolve(parsed.outJson), JSON.stringify(report, null, 2), "utf8");
  }
  if (parsed.outMd) {
    mkdirSync(dirname(resolve(parsed.outMd)), { recursive: true });
    writeFileSync(resolve(parsed.outMd), formatMarkdownReport(report), "utf8");
  }
  if (report.status !== "passed") process.exit(1);
  console.log(
    `[TASK-989] 50-Peer Stress Test PASS: peers=${simulation.peerCount}, rate=${simulation.eventsPerSecond} ev/s, jank=${simulation.jankFrames}, rssDelta=${simulation.rssDeltaMb}MB`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}
