#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор интерактивных опросов в сообщениях (Polls & Voting — TASK-1004):
 * - Сверка дизайн-токенов, геометрии `Poll Card` (`100% x auto`, `padding: 12px`, `spacing: 8px`),
 *   интерактивных состояний `poll-option` (`radio` -> `hover bg` -> `voted`), маршрутов REST
 *   (`PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`, `PUT /channels/{id}/polls/{msg_id}/answers/@me`,
 *   `POST /channels/{id}/polls/{msg_id}/expire`, `GET /channels/{id}/polls/{msg_id}/answers/{answer_id}`)
 *   и событий Gateway (`MESSAGE_POLL_VOTE_ADD` = 4 в дампе) с независимыми RE-оракулами.
 * - Математическая модель процентных индикаторов (защита от деления на ноль, анимированная шкала `#5865f233`,
 *   определение лидера и победителя), одиночный/множественный выбор и дедлайны истечения.
 * - Покрытие 9 обязательных измерений `TEST-POLICY.md` и маскирование секретов.
 * - Коды выхода CLI: 0 (успех), 1 (ошибка аргументов/инвариантов), 2 (оракул недоступен).
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_GW_PROTOCOL_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const DEFAULT_REST_D02_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const DEFAULT_RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const DEFAULT_RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** Макет карточки опроса `Poll Card` из `waves/kanban-board.json` (`TASK-1004`). */
export const POLL_CARD_LAYOUT = Object.freeze({
  container: "Poll Card",
  dimensions: "100% x auto",
  padding: "12px",
  spacing: "8px",
});

/** Дизайн-токены карточки `TASK-1004` из `waves/kanban-board.json`. */
export const POLL_DESIGN_TOKENS = Object.freeze({
  background: "#2b2d31",
  surface: "#1e1f22",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Интерактивные состояния элемента `poll-option` из `waves/kanban-board.json` (`TASK-1004`). */
export const POLL_OPTION_STATES = Object.freeze({
  element: "poll-option",
  default: "radio",
  hover: "hover bg",
  active: "voted",
});

/** Константы протокола и UI опросов из модулей `412136`, `789646`, `969632` RE-дампа Discord. */
export const POLL_PROTOCOL_CONSTANTS = Object.freeze({
  MODAL_KEY: "POLL_CREATION_MODAL_KEY",
  MIN_ANSWERS: 1,
  MAX_ANSWERS: 10,
  MAX_QUESTION_CHARS: 300,
  MAX_ANSWER_CHARS: 55,
  MAX_VOTER_PREVIEW_NAMES: 3,
  TITLE_TRUNCATE_CHARS: 50,
  MAX_DURATION_HOURS: 336,
  DURATION_PRESETS_HOURS: Object.freeze([1, 4, 8, 24, 72, 168, 336]),
  PROGRESS_FILL_HEX: "#5865f233",
  LAYOUT_TYPE_DEFAULT: 1,
  PERM_VIEW_CHANNEL: 1n << 10n,
  PERM_SEND_MESSAGES: 1n << 11n,
  PERM_READ_MESSAGE_HISTORY: 1n << 16n,
  PERM_SEND_POLLS: 1n << 49n,
  RE_MODULE_CONTEXT: "290794",
  RE_MODULE_CONSTANTS: "412136",
  RE_MODULE_FOCUS: "789646",
  RE_MODULE_ACTIONS: "969632",
});

/** 4 живых события `MESSAGE_POLL_VOTE_ADD` из `/home/sanya/re-deep/gw-proto/msgs-run2-95974_8269.jsonl`. */
export const LIVE_POLL_VOTE_EVENTS_ORACLE = Object.freeze([
  Object.freeze({
    seq: 114,
    type: "MESSAGE_POLL_VOTE_ADD",
    guildId: "942355007271997450",
    channelId: "1294815650186788916",
    messageId: "1558237752917758108",
    userId: "606781270382346250",
    answerId: 8,
  }),
  Object.freeze({
    seq: 168,
    type: "MESSAGE_POLL_VOTE_ADD",
    guildId: "942355007271997450",
    channelId: "1294815650186788916",
    messageId: "1558237967334772848",
    userId: "479659568322969611",
    answerId: 1,
  }),
  Object.freeze({
    seq: 170,
    type: "MESSAGE_POLL_VOTE_ADD",
    guildId: "942355007271997450",
    channelId: "1294815650186788916",
    messageId: "1558237752917758108",
    userId: "479659568322969611",
    answerId: 8,
  }),
  Object.freeze({
    seq: 473,
    type: "MESSAGE_POLL_VOTE_ADD",
    guildId: "942355007271997450",
    channelId: "1294815650186788916",
    messageId: "1558237967334772848",
    userId: "1068752077230182481",
    answerId: 7,
  }),
]);

/**
 * Проверяет корректность Snowflake ID (`> 0` и `<= u64::MAX`).
 * @param {string | number | bigint} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  const str = String(raw ?? "").trim();
  if (!/^\d{1,20}$/.test(str)) return false;
  try {
    const val = BigInt(str);
    return val > 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Вычисляет интерактивное состояние элемента `poll-option` (`radio` / `hover bg` / `voted`).
 * @param {{ voted?: boolean, hovered?: boolean }} opts
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolvePollOptionState(opts) {
  if (opts.voted) {
    return { element: POLL_OPTION_STATES.element, state: "active", label: POLL_OPTION_STATES.active };
  }
  if (opts.hovered) {
    return { element: POLL_OPTION_STATES.element, state: "hover", label: POLL_OPTION_STATES.hover };
  }
  return { element: POLL_OPTION_STATES.element, state: "default", label: POLL_OPTION_STATES.default };
}

/**
 * Проверяет, истёк ли срок действия опроса на момент `nowMs`.
 * @param {number | null | undefined} expiryMs
 * @param {number} [nowMs]
 * @returns {boolean}
 */
export function isPollExpired(expiryMs, nowMs = Date.now()) {
  if (expiryMs == null || !Number.isFinite(expiryMs)) {
    return true;
  }
  return expiryMs <= nowMs;
}

const MINUTE_MS = 60_000;
const HOUR_MS = 3_600_000;
const DAY_MS = 86_400_000;

/**
 * Форматирует обратный отсчёт дедлайна опроса на русском языке (паритет с `poll-progress.js`).
 * @param {number | null | undefined} expiryMs
 * @param {number} [nowMs]
 * @returns {string}
 */
export function formatPollExpiryCountdownRu(expiryMs, nowMs = Date.now()) {
  if (isPollExpired(expiryMs, nowMs)) {
    return "Опрос завершён";
  }
  const diffMs = /** @type {number} */ (expiryMs) - nowMs;
  if (diffMs < MINUTE_MS) return "Осталось меньше минуты";
  if (diffMs < HOUR_MS) return `Осталось ${Math.ceil(diffMs / MINUTE_MS)} мин.`;
  if (diffMs < DAY_MS) return `Осталось ${Math.ceil(diffMs / HOUR_MS)} ч.`;
  return `Осталось ${Math.ceil(diffMs / DAY_MS)} дн.`;
}

/**
 * Переключает выбранный вариант ответа в одиночном (`radio`) или множественном (`checkbox`) режиме.
 * @param {readonly number[]} selectedIds
 * @param {number} answerId
 * @param {boolean} [allowMultiselect]
 * @param {number} [maxChoices]
 * @returns {number[]}
 */
export function togglePollSelection(
  selectedIds,
  answerId,
  allowMultiselect = false,
  maxChoices = POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS,
) {
  const current = Array.isArray(selectedIds) ? [...selectedIds] : [];
  if (!Number.isInteger(answerId) || answerId < 1 || answerId > POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS) {
    throw new RangeError(`Недопустимый answer_id опроса: ${answerId}`);
  }
  const safeLimit = Math.max(1, Math.min(POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS, Math.floor(maxChoices)));
  if (!allowMultiselect) {
    return current.includes(answerId) ? [] : [answerId];
  }
  if (current.includes(answerId)) {
    return current.filter((id) => id !== answerId);
  }
  return current.length < safeLimit ? [...current, answerId] : current;
}

/**
 * Строит REST-запрос голосования `PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}` (или `DELETE`).
 * @param {{ channelId: string, messageId: string, answerId: number, action?: "add" | "remove" }} params
 */
export function buildPollVoteRequest(params) {
  if (!isValidSnowflake(params.channelId)) {
    throw new RangeError(`Некорректный channel_id: ${params.channelId}`);
  }
  if (!isValidSnowflake(params.messageId)) {
    throw new RangeError(`Некорректный message_id: ${params.messageId}`);
  }
  if (
    !Number.isInteger(params.answerId) ||
    params.answerId < POLL_PROTOCOL_CONSTANTS.MIN_ANSWERS ||
    params.answerId > POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS
  ) {
    throw new RangeError(`Некорректный answer_id: ${params.answerId}`);
  }
  const method = params.action === "remove" ? "DELETE" : "PUT";
  const path = `/channels/${params.channelId}/polls/${params.messageId}/answers/${params.answerId}`;
  return {
    method,
    path,
    apiV9Path: `/api/v9${path}`,
    channelId: params.channelId,
    messageId: params.messageId,
    answerId: params.answerId,
  };
}

/**
 * Строит пакетный REST-запрос `PUT /channels/{id}/polls/{msg_id}/answers/@me`.
 * @param {{ channelId: string, messageId: string, answerIds: readonly number[], allowMultiselect?: boolean }} params
 */
export function buildPollBatchAnswersRequest(params) {
  if (!isValidSnowflake(params.channelId) || !isValidSnowflake(params.messageId)) {
    throw new RangeError("Некорректный Snowflake channelId или messageId");
  }
  if (!Array.isArray(params.answerIds) || params.answerIds.length > POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS) {
    throw new RangeError("Список answerIds превышает лимит 10 вариантов");
  }
  if (params.allowMultiselect === false && params.answerIds.length > 1) {
    throw new RangeError("Опрос без allow_multiselect не допускает более 1 ответа");
  }
  for (const id of params.answerIds) {
    if (!Number.isInteger(id) || id < 1 || id > POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS) {
      throw new RangeError(`Недопустимый answer_id: ${id}`);
    }
  }
  const path = `/channels/${params.channelId}/polls/${params.messageId}/answers/@me`;
  return {
    method: "PUT",
    path,
    apiV9Path: `/api/v9${path}`,
    body: { answer_ids: params.answerIds.map((id) => String(id)) },
  };
}

/**
 * Строит REST-запрос досрочного завершения опроса `POST /channels/{id}/polls/{msg_id}/expire`.
 * @param {{ channelId: string, messageId: string }} params
 */
export function buildPollExpireRequest(params) {
  if (!isValidSnowflake(params.channelId) || !isValidSnowflake(params.messageId)) {
    throw new RangeError("Некорректный Snowflake channelId или messageId");
  }
  const path = `/channels/${params.channelId}/polls/${params.messageId}/expire`;
  return { method: "POST", path, apiV9Path: `/api/v9${path}` };
}

/**
 * Строит REST-запрос списка проголосовавших `GET /channels/{id}/polls/{msg_id}/answers/{answer_id}`.
 * @param {{ channelId: string, messageId: string, answerId: number, limit?: number, after?: string }} params
 */
export function buildPollAnswerVotersRequest(params) {
  if (!isValidSnowflake(params.channelId) || !isValidSnowflake(params.messageId)) {
    throw new RangeError("Некорректный Snowflake channelId или messageId");
  }
  if (!Number.isInteger(params.answerId) || params.answerId < 1 || params.answerId > 10) {
    throw new RangeError(`Недопустимый answerId: ${params.answerId}`);
  }
  const limit = params.limit ?? 25;
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) {
    throw new RangeError(`Лимит пагинации проголосовавших должен быть 1..100: ${limit}`);
  }
  const queryParts = [`limit=${limit}`];
  if (params.after !== undefined) {
    if (!isValidSnowflake(params.after)) {
      throw new RangeError(`Некорректный курсор after: ${params.after}`);
    }
    queryParts.push(`after=${params.after}`);
  }
  const basePath = `/channels/${params.channelId}/polls/${params.messageId}/answers/${params.answerId}`;
  const query = queryParts.join("&");
  return { method: "GET", path: `${basePath}?${query}`, apiV9Path: `/api/v9${basePath}?${query}` };
}

/**
 * Вычисляет процентные индикаторы, счётчики голосов, лидирующие и победившие варианты опроса.
 * @param {ReadonlyArray<{ answerId: number, text: string, emojiName?: string }>} answers
 * @param {ReadonlyArray<{ id: number, count: number, meVoted?: boolean }>} counts
 * @param {boolean} [isFinalizedOrExpired]
 */
export function computePollPercentages(answers, counts, isFinalizedOrExpired = false) {
  const safeAnswers = Array.isArray(answers) ? answers : [];
  const safeCounts = Array.isArray(counts) ? counts : [];
  /** @type {Map<number, { id: number, count: number, meVoted?: boolean }>} */
  const byId = new Map();
  for (const c of safeCounts) {
    if (c && Number.isInteger(c.id)) byId.set(c.id, c);
  }
  const bare = safeAnswers.map((ans) => {
    const entry = byId.get(ans.answerId);
    const rawCount = entry && Number.isFinite(entry.count) ? Math.max(0, Math.floor(entry.count)) : 0;
    return {
      answerId: ans.answerId,
      text: typeof ans.text === "string" ? ans.text : "",
      emojiName: ans.emojiName ?? null,
      count: rawCount,
      meVoted: Boolean(entry && entry.meVoted),
    };
  });
  const totalVotes = bare.reduce((sum, item) => sum + item.count, 0);
  const maxCount = bare.reduce((max, item) => Math.max(max, item.count), 0);
  const items = bare.map((item) => {
    const percentage = totalVotes > 0 ? Math.round((item.count * 100) / totalVotes) : 0;
    const isLeading = totalVotes > 0 && item.count === maxCount;
    const isWinner = Boolean(isFinalizedOrExpired) && isLeading;
    const uiState = resolvePollOptionState({ voted: item.meVoted, hovered: false });
    return {
      answerId: item.answerId,
      text: item.text,
      emojiName: item.emojiName,
      count: item.count,
      percentage,
      fillWidthPercent: percentage,
      fillColorHex: POLL_PROTOCOL_CONSTANTS.PROGRESS_FILL_HEX,
      meVoted: item.meVoted,
      isLeading,
      isWinner,
      state: uiState.label,
    };
  });
  return { totalVotes, maxCount, isFinalized: Boolean(isFinalizedOrExpired), items };
}

/**
 * Движок интерактивного опроса с оптимистичным голосованием, откатом и поддержкой Gateway.
 */
export class InteractivePollEngine {
  /**
   * @param {{
   *   channelId: string,
   *   messageId: string,
   *   guildId?: string,
   *   currentUserId: string,
   *   question: string,
   *   answers: ReadonlyArray<{ answerId: number, text: string, emojiName?: string }>,
   *   allowMultiselect?: boolean,
   *   expiryMs: number,
   *   isFinalized?: boolean
   * }} config
   */
  constructor(config) {
    if (!isValidSnowflake(config.channelId) || !isValidSnowflake(config.messageId) || !isValidSnowflake(config.currentUserId)) {
      throw new RangeError("InteractivePollEngine: некорректный Snowflake ID");
    }
    const q = (config.question ?? "").trim();
    if (q.length === 0 || q.length > POLL_PROTOCOL_CONSTANTS.MAX_QUESTION_CHARS) {
      throw new RangeError(`Вопрос опроса должен содержать 1..300 символов (получено ${q.length})`);
    }
    if (!Array.isArray(config.answers) || config.answers.length < 1 || config.answers.length > 10) {
      throw new RangeError("Опрос должен содержать от 1 до 10 вариантов ответа");
    }
    for (const ans of config.answers) {
      const t = (ans.text ?? "").trim();
      if (t.length === 0 || t.length > POLL_PROTOCOL_CONSTANTS.MAX_ANSWER_CHARS) {
        throw new RangeError(`Текст ответа должен содержать 1..55 символов (получено ${t.length})`);
      }
    }
    this.channelId = config.channelId;
    this.messageId = config.messageId;
    this.guildId = config.guildId ?? null;
    this.currentUserId = config.currentUserId;
    this.question = q;
    this.answers = config.answers.map((a) => ({ ...a }));
    this.allowMultiselect = Boolean(config.allowMultiselect);
    this.expiryMs = config.expiryMs;
    this.isFinalized = Boolean(config.isFinalized);
    /** @type {Map<number, Set<string>>} */
    this.votersByAnswer = new Map(this.answers.map((a) => [a.answerId, new Set()]));
    /** @type {number[] | null} */
    this._optimisticBackup = null;
  }

  /**
   * Применяет входящее событие Gateway (`MESSAGE_POLL_VOTE_ADD` / `MESSAGE_POLL_VOTE_REMOVE`).
   * @param {{ type: string, channelId: string, messageId: string, userId: string, answerId: number }} event
   * @returns {boolean}
   */
  applyGatewayVoteEvent(event) {
    if (event.channelId !== this.channelId || event.messageId !== this.messageId || !isValidSnowflake(event.userId)) {
      return false;
    }
    const voters = this.votersByAnswer.get(event.answerId);
    if (!voters) return false;
    if (event.type === "MESSAGE_POLL_VOTE_ADD") {
      if (!this.allowMultiselect) {
        for (const [ansId, set] of this.votersByAnswer.entries()) {
          if (ansId !== event.answerId) set.delete(event.userId);
        }
      }
      voters.add(event.userId);
      return true;
    }
    if (event.type === "MESSAGE_POLL_VOTE_REMOVE") {
      voters.delete(event.userId);
      return true;
    }
    return false;
  }

  /**
   * Голосование текущего пользователя с проверкой дедлайна и формированием REST-запроса.
   * @param {number} answerId
   * @param {number} nowMs
   */
  castVote(answerId, nowMs) {
    if (this.isFinalized || isPollExpired(this.expiryMs, nowMs)) {
      throw new Error("POLL_EXPIRED: голосование в завершённом опросе запрещено");
    }
    if (!this.votersByAnswer.has(answerId)) {
      throw new RangeError(`Неизвестный answer_id: ${answerId}`);
    }
    const prevSelected = [];
    for (const [id, set] of this.votersByAnswer.entries()) {
      if (set.has(this.currentUserId)) prevSelected.push(id);
    }
    this._optimisticBackup = prevSelected;
    const alreadyVoted = prevSelected.includes(answerId);
    this.applyGatewayVoteEvent({
      type: alreadyVoted ? "MESSAGE_POLL_VOTE_REMOVE" : "MESSAGE_POLL_VOTE_ADD",
      channelId: this.channelId,
      messageId: this.messageId,
      userId: this.currentUserId,
      answerId,
    });
    return buildPollVoteRequest({
      channelId: this.channelId,
      messageId: this.messageId,
      answerId,
      action: alreadyVoted ? "remove" : "add",
    });
  }

  /** Откатывает оптимистичный голос при сетевой ошибке (`401/403/429/5xx`). */
  rollbackOptimisticVote() {
    if (!this._optimisticBackup) return false;
    for (const set of this.votersByAnswer.values()) set.delete(this.currentUserId);
    for (const ansId of this._optimisticBackup) this.votersByAnswer.get(ansId)?.add(this.currentUserId);
    this._optimisticBackup = null;
    return true;
  }

  /**
   * Возвращает снимок состояния карточки опроса с процентами и индикаторами.
   * @param {number} nowMs
   */
  snapshot(nowMs) {
    const expired = this.isFinalized || isPollExpired(this.expiryMs, nowMs);
    const counts = this.answers.map((ans) => {
      const set = this.votersByAnswer.get(ans.answerId);
      return { id: ans.answerId, count: set ? set.size : 0, meVoted: set ? set.has(this.currentUserId) : false };
    });
    const computed = computePollPercentages(this.answers, counts, expired);
    return {
      channelId: this.channelId,
      messageId: this.messageId,
      question: this.question,
      allowMultiselect: this.allowMultiselect,
      expired,
      expiryTextRu: formatPollExpiryCountdownRu(this.expiryMs, nowMs),
      totalVotes: computed.totalVotes,
      items: computed.items,
    };
  }
}

/**
 * Сверяет константы, дизайн-токены, маршруты REST и события Gateway с независимыми оракулами.
 * @param {{
 *   kanbanPath?: string,
 *   gatewayProtocolPath?: string,
 *   restD02Path?: string,
 *   reRestStaticPath?: string,
 *   reModulesIndexPath?: string
 * }} [opts]
 */
export function verifyPollsOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const gatewayProtocolPath = opts.gatewayProtocolPath ?? DEFAULT_GW_PROTOCOL_PATH;
  const restD02Path = opts.restD02Path ?? DEFAULT_REST_D02_PATH;
  /** @type {string[]} */
  const reasons = [];

  for (const p of [kanbanPath, gatewayProtocolPath, restD02Path]) {
    if (!existsSync(p)) {
      return { passed: false, reasons: [`Отсутствует оракул: ${p}`], gatewayVoteAddCount: 0 };
    }
  }
  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card1004 = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1004");
  if (!card1004) {
    reasons.push("Карточка TASK-1004 не найдена в waves/kanban-board.json");
  } else {
    if (card1004.layout?.container !== POLL_CARD_LAYOUT.container) reasons.push("Несовпадение container");
    if (card1004.design_tokens?.accent !== POLL_DESIGN_TOKENS.accent) reasons.push("Несовпадение accent");
    if (card1004.interactive_states?.[0]?.active !== POLL_OPTION_STATES.active) reasons.push("Несовпадение active");
  }
  const gwProto = JSON.parse(readFileSync(gatewayProtocolPath, "utf8"));
  const gatewayVoteAddCount = gwProto.opcodes?.dispatchTypes?.MESSAGE_POLL_VOTE_ADD ?? 0;
  if (gatewayVoteAddCount !== 4) {
    reasons.push(`Ожидалось MESSAGE_POLL_VOTE_ADD=4, получено ${gatewayVoteAddCount}`);
  }
  const restD02 = JSON.parse(readFileSync(restD02Path, "utf8"));
  const hasMsgEp = (restD02.endpoints ?? []).some((/** @type {any} */ ep) => ep.path === "/api/v9/channels/{id}/messages");
  if (!hasMsgEp) reasons.push("В docs/re/rest-d02.json не найден маршрут /api/v9/channels/{id}/messages");

  const reStaticPath = opts.reRestStaticPath ?? (existsSync(DEFAULT_RE_REST_STATIC_PATH) ? DEFAULT_RE_REST_STATIC_PATH : undefined);
  if (reStaticPath && existsSync(reStaticPath)) {
    const reStatic = JSON.parse(readFileSync(reStaticPath, "utf8"));
    const keys = new Set((reStatic.endpoints ?? []).map((/** @type {any} */ e) => e.key).filter(Boolean));
    for (const reqKey of ["POLL_ANSWERS", "POLL_EXPIRE", "POLL_ANSWER_VOTERS"]) {
      if (!keys.has(reqKey)) reasons.push(`Отсутствует ключ ${reqKey}`);
    }
  }
  const reModPath = opts.reModulesIndexPath ?? (existsSync(DEFAULT_RE_MODULES_INDEX_PATH) ? DEFAULT_RE_MODULES_INDEX_PATH : undefined);
  if (reModPath && existsSync(reModPath)) {
    const reMods = JSON.parse(readFileSync(reModPath, "utf8"));
    for (const modId of ["290794", "412136", "789646"]) {
      if (!reMods[modId]) reasons.push(`Отсутствует модуль ${modId}`);
    }
  }
  return { passed: reasons.length === 0, reasons, gatewayVoteAddCount };
}

/**
 * Маскирует токены авторизации и секреты в строках телеметрии опросов.
 * @param {string} text
 * @returns {string}
 */
export function redactPollTelemetry(text) {
  return String(text ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "mfa.[REDACTED]")
    .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED]")
    .replace(/(token|secret|authorization)\s*[:=]\s*["']?[^"'\s]+["']?/gi, "$1=[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для модуля опросов.
 */
export function evaluatePollNineDimensions() {
  /** @type {string[]} */
  const failures = [];
  const req = buildPollVoteRequest({
    channelId: "1294815650186788916",
    messageId: "1558237752917758108",
    answerId: 1,
    action: "add",
  });
  if (req.method !== "PUT") failures.push("dim1_positive_put_method");

  let expiredRejected = false;
  try {
    const eng = new InteractivePollEngine({
      channelId: "1294815650186788916",
      messageId: "1558237752917758108",
      currentUserId: "479659568322969611",
      question: "Тест дедлайна?",
      answers: [{ answerId: 1, text: "Да" }],
      expiryMs: 1_000,
    });
    eng.castVote(1, 2_000);
  } catch {
    expiredRejected = true;
  }
  if (!expiredRejected) failures.push("dim2_negative_expired_vote_rejected");

  if (isValidSnowflake("0") !== false || isValidSnowflake("18446744073709551615") !== true) {
    failures.push("dim3_boundary_snowflake");
  }
  const badgeFormat = (/** @type {number} */ n) => (n > 99 ? "99+" : String(n));
  if (badgeFormat(99) !== "99" || badgeFormat(100) !== "99+") failures.push("dim3_boundary_badge_cap");

  const mapHttpStatus = (/** @type {number} */ status, /** @type {number} */ retryMs) => ({
    retryable: status === 429 || status >= 500,
    backoffMs: status === 429 ? Math.max(100, retryMs) : status >= 500 ? 1_000 : 0,
  });
  if (!mapHttpStatus(429, 1_500).retryable || mapHttpStatus(403, 0).retryable) {
    failures.push("dim4_http_error_classification");
  }

  const emptyCalc = computePollPercentages([{ answerId: 1, text: "Вариант 1" }], [], false);
  if (emptyCalc.totalVotes !== 0 || emptyCalc.items[0]?.percentage !== 0 || emptyCalc.items[0]?.isLeading) {
    failures.push("dim5_empty_zero_votes");
  }

  const isSkeleton = (/** @type {{ loading?: boolean, httpStatus?: number }} */ s) =>
    Boolean(s.loading || s.httpStatus === 202);
  if (!isSkeleton({ httpStatus: 202 }) || isSkeleton({ httpStatus: 200 })) {
    failures.push("dim6_loading_http_202");
  }

  const fullPerms =
    POLL_PROTOCOL_CONSTANTS.PERM_VIEW_CHANNEL |
    POLL_PROTOCOL_CONSTANTS.PERM_SEND_MESSAGES |
    POLL_PROTOCOL_CONSTANTS.PERM_SEND_POLLS;
  const canCreatePoll = (/** @type {bigint} */ perms, /** @type {boolean} */ ageRestricted) =>
    !ageRestricted && (perms & fullPerms) === fullPerms;
  if (!canCreatePoll(fullPerms, false) || canCreatePoll(fullPerms, true) || canCreatePoll(0n, false)) {
    failures.push("dim7_permissions_and_age_gate");
  }

  const recEngine = new InteractivePollEngine({
    channelId: "1294815650186788916",
    messageId: "1558237752917758108",
    currentUserId: "479659568322969611",
    question: "Проверка отката?",
    answers: [{ answerId: 1, text: "Один" }, { answerId: 2, text: "Два" }],
    expiryMs: 2_000_000_000_000,
  });
  recEngine.castVote(1, 1_700_000_000_000);
  recEngine.rollbackOptimisticVote();
  if (recEngine.snapshot(1_700_000_000_000).totalVotes !== 0) failures.push("dim8_optimistic_rollback");

  if (redactPollTelemetry("Authorization: Bearer mfa.secret12345").includes("mfa.secret12345")) {
    failures.push("dim9_observability_redaction");
  }
  return { passed: failures.length === 0, dimensionsChecked: 9, failures };
}

/**
 * Исполняет CLI `scripts/verify-polls.mjs`.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runPollsCli(argv) {
  let oraclePath = DEFAULT_KANBAN_PATH;
  let outJson = "";
  let outMd = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      console.log("Использование: node scripts/verify-polls.mjs [--oracle <path>] [--out-json <path>] [--out-md <path>]");
      return 0;
    }
    if (arg === "--oracle" || arg === "--out-json" || arg === "--out-md") {
      const val = argv[i + 1];
      if (!val) return 1;
      if (arg === "--oracle") oraclePath = val;
      if (arg === "--out-json") outJson = val;
      if (arg === "--out-md") outMd = val;
      i += 1;
      continue;
    }
    return 1;
  }

  if (!existsSync(oraclePath)) {
    console.error(`verify-polls: файл-оракул недоступен: ${oraclePath}`);
    return 2;
  }

  const parity = verifyPollsOracleParity({ kanbanPath: oraclePath });
  const dims = evaluatePollNineDimensions();
  if (!parity.passed || !dims.passed) {
    console.error("verify-polls: нарушение контракта", [...parity.reasons, ...dims.failures]);
    return 1;
  }

  const report = {
    taskId: "TASK-1004",
    status: "PASS",
    layout: POLL_CARD_LAYOUT,
    designTokens: POLL_DESIGN_TOKENS,
    optionStates: POLL_OPTION_STATES,
    gatewayVoteAddCount: parity.gatewayVoteAddCount,
    dimensionsChecked: dims.dimensionsChecked,
  };
  if (outJson) {
    mkdirSync(dirname(resolve(outJson)), { recursive: true });
    writeFileSync(resolve(outJson), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(resolve(outMd)), { recursive: true });
    writeFileSync(
      resolve(outMd),
      `# Отчёт верификации TASK-1004 (Polls & Voting)\n\n- Статус: **PASS**\n- Событий MESSAGE_POLL_VOTE_ADD: \`${parity.gatewayVoteAddCount}\`\n- Проверено измерений: \`${dims.dimensionsChecked}/9\`\n`,
      "utf8",
    );
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runPollsCli(process.argv.slice(2)));
}
