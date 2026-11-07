// @ts-check
/**
 * Реактивный стор состояния UI (`@preact/signals-core`-совместимое ядро без внешних зависимостей).
 *
 * Обеспечивает атомарные сигналы (`createSignal`), производные вычисления (`createComputed`),
 * автоматические подписки (`createEffect`), пакетные транзакции (`batch`) и единый типизированный
 * стор живой сессии Discord (`createAppReactiveStore`) для синхронизации `voiceStates`,
 * `activeVoiceConnection`, `dmCallStage` и дерева серверов по пуш-событиям из Rust (`discord://live-state-push`).
 */

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

/** @type {(() => void) | null} */
let currentObserver = null;
let batchDepth = 0;
/** @type {Set<() => void>} */
const pendingEffects = new Set();

/**
 * Выполняет серию изменений сигналов атомарно, вызывая зависимые эффекты ровно один раз в конце.
 * @template R
 * @param {() => R} fn
 * @returns {R}
 */
export function batch(fn) {
  batchDepth += 1;
  try {
    return fn();
  } finally {
    batchDepth -= 1;
    if (batchDepth === 0 && pendingEffects.size > 0) {
      const toRun = Array.from(pendingEffects);
      pendingEffects.clear();
      for (const effect of toRun) {
        effect();
      }
    }
  }
}

/**
 * Создаёт реактивный сигнал со значением `initialValue`.
 * @template T
 * @param {T} initialValue
 * @returns {Signal<T>}
 */
export function createSignal(initialValue) {
  let currentValue = initialValue;
  /** @type {Set<() => void>} */
  const subscribers = new Set();

  return {
    get value() {
      if (currentObserver) {
        subscribers.add(currentObserver);
      }
      return currentValue;
    },
    set value(nextValue) {
      if (Object.is(currentValue, nextValue)) {
        return;
      }
      currentValue = nextValue;
      const snapshot = Array.from(subscribers);
      if (batchDepth > 0) {
        for (const sub of snapshot) {
          pendingEffects.add(sub);
        }
      } else {
        for (const sub of snapshot) {
          sub();
        }
      }
    },
    peek() {
      return currentValue;
    },
    subscribe(listener) {
      return createEffect(() => {
        listener(this.value);
      });
    },
  };
}

/**
 * Создаёт реактивный эффект, автоматически отслеживающий прочитанные внутри сигналы.
 * @param {() => void} fn
 * @returns {() => void} функция отписки (`dispose`)
 */
export function createEffect(fn) {
  let disposed = false;
  const execute = () => {
    if (disposed) {
      return;
    }
    const prevObserver = currentObserver;
    currentObserver = execute;
    try {
      fn();
    } finally {
      currentObserver = prevObserver;
    }
  };
  execute();
  return () => {
    disposed = true;
    pendingEffects.delete(execute);
  };
}

/**
 * Создаёт вычисляемый (`computed`) сигнал, автоматически пересчитывающийся при изменении зависимостей.
 * @template T
 * @param {() => T} computeFn
 * @returns {ReadonlySignal<T>}
 */
export function createComputed(computeFn) {
  const internal = createSignal(/** @type {T} */ (undefined));
  createEffect(() => {
    internal.value = computeFn();
  });
  return {
    get value() {
      return internal.value;
    },
    peek() {
      return internal.peek();
    },
    subscribe(listener) {
      return internal.subscribe(listener);
    },
  };
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
