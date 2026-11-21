// @ts-check
/**
 * Реактивный стор состояния UI на базе проверенной ESM-библиотеки `@preact/signals-core`.
 *
 * Обеспечивает атомарные сигналы (`createSignal` / `signal`), производные вычисления (`createComputed` / `computed`),
 * автоматические подписки (`createEffect` / `effect`), пакетные транзакции (`batch`) и единый типизированный
 * стор живой сессии Discord (`createAppReactiveStore`) для синхронизации `voiceStates`,
 * `activeVoiceConnection`, `dmCallStage` и дерева серверов по пуш-событиям из Rust (`discord://live-state-push`).
 */

import {
  signal as coreSignal,
  computed as coreComputed,
  effect as coreEffect,
  batch as coreBatch,
} from "@preact/signals-core";

export const isSignalsCorePowered = true;

/**
 * @template T
 * @typedef {Object} ReadonlySignal
 * @property {T} value
 * @property {() => T} peek
 * @property {(listener: (val: T) => void) => () => void} subscribe
 */

/**
 * @template T
 * @typedef {Object} Signal
 * @property {T} value
 * @property {() => T} peek
 * @property {(listener: (val: T) => void) => () => void} subscribe
 */

/**
 * Выполняет серию изменений сигналов атомарно, вызывая зависимые эффекты ровно один раз в конце.
 * @template R
 * @param {() => R} fn
 * @returns {R}
 */
export const batch = coreBatch;

/**
 * Создаёт реактивный сигнал со значением `initialValue`.
 * @template T
 * @param {T} initialValue
 * @returns {Signal<T>}
 */
export function createSignal(initialValue) {
  const s = coreSignal(initialValue);
  return {
    get value() {
      return s.value;
    },
    set value(nextValue) {
      if (Object.is(s.peek(), nextValue)) {
        return;
      }
      s.value = nextValue;
    },
    peek() {
      return s.peek();
    },
    subscribe(listener) {
      return s.subscribe(listener);
    },
  };
}

/**
 * Создаёт реактивный эффект, автоматически отслеживающий прочитанные внутри сигналы.
 * @param {() => void} fn
 * @returns {() => void} функция отписки (`dispose`)
 */
export function createEffect(fn) {
  return coreEffect(fn);
}

/**
 * Создаёт вычисляемый (`computed`) сигнал, автоматически пересчитывающийся при изменении зависимостей.
 * @template T
 * @param {() => T} computeFn
 * @returns {ReadonlySignal<T>}
 */
export function createComputed(computeFn) {
  return /** @type {ReadonlySignal<T>} */ (coreComputed(computeFn));
}

/**
 * @typedef {Object} VoiceConnectionSnapshot
 * @property {string} guildId
 * @property {string} channelId
 * @property {string} channelName
 * @property {boolean} selfMute
 * @property {boolean} selfDeaf
 * @property {boolean} selfVideo
 * @property {boolean} selfStream
 */

/**
 * @typedef {Object} DmCallStageSummary
 * @property {boolean} inCall
 * @property {string | null} channelId
 * @property {number} participantCount
 * @property {boolean} anyStreaming
 */

/**
 * Создаёт типизированный реактивный стор для голосовых состояний, звонков в ЛС и гильдий.
 */
export function createAppReactiveStore() {
  /** @type {Signal<VoiceConnectionSnapshot | null>} */
  const activeVoiceConnection = createSignal(/** @type {VoiceConnectionSnapshot | null} */ (null));
  /** @type {Signal<Record<string, Array<Record<string, any>>>>} */
  const voiceStatesByGuild = createSignal(/** @type {Record<string, Array<Record<string, any>>>} */ ({}));
  /** @type {Signal<Array<Record<string, any>>>} */
  const guilds = createSignal(/** @type {Array<Record<string, any>>} */ ([]));
  /** @type {Signal<number>} */
  const pushRevision = createSignal(0);

  /** @type {ReadonlySignal<DmCallStageSummary>} */
  const dmCallStageSummary = createComputed(() => {
    const conn = activeVoiceConnection.value;
    const allVs = voiceStatesByGuild.value;
    const homeVs = Array.isArray(allVs["@me"]) ? allVs["@me"] : [];
    const isDmConn = Boolean(conn && (!conn.guildId || conn.guildId === "@me") && conn.channelId);
    const activeCid = isDmConn && conn ? String(conn.channelId) : null;
    const filtered = activeCid
      ? homeVs.filter((vs) => String(vs?.channel_id ?? vs?.channelId ?? "") === activeCid)
      : homeVs;
    const anyStreaming =
      Boolean(conn?.selfStream) ||
      filtered.some((vs) => Boolean(vs?.self_stream ?? vs?.selfStream));
    return {
      inCall: isDmConn || filtered.length > 0,
      channelId: activeCid ?? (filtered[0] ? String(filtered[0].channel_id ?? filtered[0].channelId ?? "") || null : null),
      participantCount: filtered.length,
      anyStreaming,
    };
  });

  /**
   * Применяет пуш-снимок состояния из Rust BFF (`discord://live-state-push` или `voice_states_sync`).
   * @param {{
   *   voiceConnection?: VoiceConnectionSnapshot | null,
   *   voiceStates?: Record<string, Array<Record<string, any>>>,
   *   guilds?: Array<Record<string, any>>,
   * }} payload
   */
  function applyLivePushPayload(payload) {
    if (!payload || typeof payload !== "object") {
      return;
    }
    batch(() => {
      if ("voiceConnection" in payload) {
        activeVoiceConnection.value = payload.voiceConnection ?? null;
      }
      if (payload.voiceStates && typeof payload.voiceStates === "object") {
        voiceStatesByGuild.value = { ...payload.voiceStates };
      }
      if (Array.isArray(payload.guilds)) {
        guilds.value = payload.guilds.slice();
      }
      pushRevision.value = pushRevision.peek() + 1;
    });
  }

  return {
    activeVoiceConnection,
    voiceStatesByGuild,
    guilds,
    pushRevision,
    dmCallStageSummary,
    applyLivePushPayload,
  };
}

export const signal = createSignal;
export const computed = createComputed;
export const effect = createEffect;

/**
 * Создаёт полный реактивный стор состояния Discord (серверы, каналы, сообщения, звонки в ЛС).
 */
export function createDiscordReactiveStore() {
  /** @type {Signal<any>} */
  const selfUser = createSignal(null);
  /** @type {Signal<string>} */
  const selectedGuildId = createSignal("@me");
  /** @type {Signal<string>} */
  const selectedChannelId = createSignal("");
  /** @type {Signal<Map<string, any>>} */
  const guildsById = createSignal(new Map());
  /** @type {Signal<Map<string, any[]>>} */
  const channelsByGuildId = createSignal(new Map());
  /** @type {Signal<Map<string, any[]>>} */
  const messagesByChannelId = createSignal(new Map());
  /** @type {Signal<Map<string, any[]>>} */
  const voiceStatesByGuild = createSignal(new Map());

  const isDmMode = createComputed(() => selectedGuildId.value === "@me");

  const activeGuildChannels = createComputed(() => {
    const gid = selectedGuildId.value;
    return channelsByGuildId.value.get(gid) ?? [];
  });

  const activeChannelMessages = createComputed(() => {
    const cid = selectedChannelId.value;
    return messagesByChannelId.value.get(cid) ?? [];
  });

  const activeDmCallParticipants = createComputed(() => {
    if (!isDmMode.value) return [];
    const cid = selectedChannelId.value;
    if (!cid) return [];
    const list = voiceStatesByGuild.value.get("@me") ?? [];
    return list.filter((v) => String(v?.channel_id ?? v?.channelId ?? "") === cid);
  });

  /**
   * @param {any} readyPayload
   */
  function hydrateReady(readyPayload) {
    batch(() => {
      if (readyPayload?.user) {
        selfUser.value = readyPayload.user;
      }
      if (Array.isArray(readyPayload?.guilds)) {
        const nextGuilds = new Map(guildsById.peek());
        for (const g of readyPayload.guilds) {
          if (g && g.id) {
            nextGuilds.set(String(g.id), g);
          }
        }
        guildsById.value = nextGuilds;
      }
      if (Array.isArray(readyPayload?.channels)) {
        const nextCh = new Map(channelsByGuildId.peek());
        for (const ch of readyPayload.channels) {
          const gid = String(ch?.guildId ?? ch?.guild_id ?? "@me");
          const arr = nextCh.get(gid) ? [...(nextCh.get(gid) ?? [])] : [];
          const idx = arr.findIndex((x) => String(x?.id) === String(ch?.id));
          if (idx >= 0) arr[idx] = ch;
          else arr.push(ch);
          nextCh.set(gid, arr);
        }
        channelsByGuildId.value = nextCh;
      }
      const rawVs = readyPayload?.voiceStates ?? readyPayload?.voice_states;
      if (rawVs && typeof rawVs === "object") {
        const nextVs = new Map();
        for (const [k, arr] of Object.entries(rawVs)) {
          nextVs.set(String(k), Array.isArray(arr) ? arr : []);
        }
        voiceStatesByGuild.value = nextVs;
      }
    });
  }

  /**
   * @param {string} guildId
   * @param {string} [channelId]
   */
  function selectGuild(guildId, channelId = "") {
    batch(() => {
      selectedGuildId.value = String(guildId || "@me");
      if (channelId !== undefined) {
        selectedChannelId.value = String(channelId || "");
      }
    });
  }

  /**
   * @param {string} channelId
   * @param {any} message
   */
  function AppendOrUpdateMessage(channelId, message) {
    const cid = String(channelId || "");
    if (!cid || !message) return;
    const nextMap = new Map(messagesByChannelId.peek());
    const list = nextMap.get(cid) ? [...(nextMap.get(cid) ?? [])] : [];
    const idx = list.findIndex((m) => String(m?.id) === String(message.id));
    if (idx >= 0) list[idx] = { ...list[idx], ...message };
    else list.push(message);
    nextMap.set(cid, list);
    messagesByChannelId.value = nextMap;
  }

  return {
    selfUser,
    selectedGuildId,
    selectedChannelId,
    guildsById,
    channelsByGuildId,
    messagesByChannelId,
    voiceStatesByGuild,
    isDmMode,
    activeGuildChannels,
    activeChannelMessages,
    activeDmCallParticipants,
    hydrateReady,
    selectGuild,
    AppendOrUpdateMessage,
  };
}

export const globalAppReactiveStore = createAppReactiveStore();
