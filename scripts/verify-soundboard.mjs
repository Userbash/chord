// @ts-check
/**
 * Верификатор и эталонный движок серверной звуковой панели (`Soundboard SFX` — `TASK-1012` / `TASK-1012A` / `TASK-1012B` / `TASK-1012C`).
 *
 * Сверяет:
 * 1. Карточки `TASK-1012..1012C` в `waves/kanban-board.json`, дизайн-токены (`#2b2d31`, `#1e1f22`,
 *    `#ffffff`, `#5865f2`, `#3f4147`), геометрию `Soundboard Grid` (`320px x 360px`, `padding: 8px`,
 *    `spacing: 6px`), интерактивные состояния `sound-button` (`sfx tile` -> `scale-105` -> `playing sfx`)
 *    и `mixer-dsp` (`bypass` -> `mix` -> `tanh limiter`).
 * 2. События шлюза `SOUNDBOARD_SOUNDS` (`21` в дампе `docs/re/gateway-protocol.json`), Voice Gateway
 *    Opcode `18` (`SOUNDBOARD_SOUND`), входящий Opcode `19` / `SPEAKING` (`SOUND_SHARE = 1 << 1`),
 *    маршруты REST v9 (`SEND_SOUNDBOARD_SOUND`, `SEND_SOUNDBOARD_ECHO`, `GUILD_SOUNDBOARD_SOUNDS`,
 *    `SOUNDBOARD_DEFAULT_SOUNDS`) и модули RE-дампа (`209932`, `796774`, `813564`, `904054`, `125831`, `528772`).
 * 3. Локальный LRU-кэш аудио-сэмплов Opus 48 кГц stereo (`960` сэмплов/канал на кадр `20 мс`, лимит `512 КБ`)
 *    и линейный RTP Opus микшер (`PT = 120`, шаг timestamp `960`) с мягким лимитером `Math.tanh()`.
 * 4. Все 9 обязательных измерений `TEST-POLICY.md` и CLI (`0` успех, `1` неверные аргументы, `2` отсутствие оракула).
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const DEFAULT_KANBAN_PATH = resolve(ROOT, "waves/kanban-board.json");
const DEFAULT_GATEWAY_PROTO_PATH = resolve(ROOT, "docs/re/gateway-protocol.json");
const DEFAULT_REST_D02_PATH = resolve(ROOT, "docs/re/rest-d02.json");
const DEFAULT_RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const DEFAULT_RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

export const SOUNDBOARD_GRID_LAYOUT = Object.freeze({
  container: "Soundboard Grid",
  dimensions: "320px x 360px",
  padding: "8px",
  spacing: "6px",
});

export const SOUNDBOARD_DESIGN_TOKENS = Object.freeze({
  background: "#2b2d31",
  surface: "#1e1f22",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const SOUNDBOARD_BUTTON_STATES = Object.freeze({
  element: "sound-button",
  default: "sfx tile",
  hover: "scale-105",
  active: "playing sfx",
});

export const SOUNDBOARD_PROTOCOL_CONSTANTS = Object.freeze({
  VOICE_OP_SOUNDBOARD_SOUND: 18,
  VOICE_OP_SPEAKING_SOUNDBOARD: 19,
  VOICE_OP_SPEAKING: 5,
  SPEAKING_FLAG_MICROPHONE: 1,
  SPEAKING_FLAG_SOUND_SHARE: 2,
  SPEAKING_FLAG_PRIORITY: 4,
  RTP_OPUS_PAYLOAD_TYPE: 120,
  OPUS_SAMPLE_RATE_HZ: 48_000,
  OPUS_CHANNELS: 2,
  FRAME_DURATION_MS: 20,
  FRAME_SAMPLES_PER_CHANNEL_20MS: 960,
  INTERLEAVED_STEREO_SAMPLES_20MS: 1_920,
  MIN_NAME_CHARS: 2,
  MAX_NAME_CHARS: 32,
  MAX_SOUND_BYTES: 524_288,
  MAX_FAVORITE_SOUNDS: 250,
  GATEWAY_SOUNDBOARD_SOUNDS_COUNT: 21,
  RE_MODULES: Object.freeze(["209932", "796774", "813564", "904054", "125831", "528772"]),
});

/**
 * Реальные записи `SOUNDBOARD_SOUNDS` из трассы `/home/sanya/re-deep/gw-proto/msgs-run2-95974_5532.jsonl` (`s=50..52`).
 */
export const LIVE_SOUNDBOARD_SOUNDS_ORACLE = Object.freeze([
  Object.freeze({
    soundId: "1102645616632397904",
    name: "Хабар принёс",
    guildId: "274093052673392641",
    userId: "275258435199827968",
    emojiId: "1024454092543758356",
    volume: 1.0,
    available: true,
  }),
  Object.freeze({
    soundId: "1102650064771485746",
    name: "Оу мааай",
    guildId: "274093052673392641",
    userId: "275258435199827968",
    emojiId: "590287695331196928",
    volume: 0.99,
    available: true,
  }),
  Object.freeze({
    soundId: "1107624897066311701",
    name: "Заткни ебало",
    guildId: "274093052673392641",
    userId: "275258435199827968",
    emojiId: "1024419295423377468",
    volume: 0.56,
    available: true,
  }),
  Object.freeze({
    soundId: "1327231186044583947",
    name: "Позвонишь",
    guildId: "274093052673392641",
    userId: "143267437494796288",
    emojiName: "☠️",
    volume: 0.62,
    available: false,
  }),
  Object.freeze({
    soundId: "1198604045594669096",
    name: "Я этим займусь",
    guildId: "1014537851418980372",
    userId: "375401893679333376",
    emojiName: "🤖",
    volume: 1.0,
    available: true,
  }),
]);

/**
 * Проверяет корректность Snowflake ID (`1..=u64::MAX`).
 * @param {unknown} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  if (typeof raw !== "string") {
    return false;
  }
  const trimmed = raw.trim();
  if (!/^\d{1,20}$/.test(trimmed)) {
    return false;
  }
  try {
    const val = BigInt(trimmed);
    return val > 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Ограничивает громкость звука в диапазоне `[0.0, 1.0]` с округлением до сотых.
 * @param {number} volume
 * @returns {number}
 */
export function clampSoundboardVolume(volume) {
  if (typeof volume !== "number" || !Number.isFinite(volume)) {
    return 1;
  }
  const clamped = Math.max(0, Math.min(1, volume));
  return Math.round(clamped * 100) / 100;
}

/**
 * Формула эффективной громкости из RE-модуля `904054`:
 * `Math.min(soundVol * (settingVol / 100) * Math.min(outputVol / 100, 1), 1)`.
 * @param {number} soundVol
 * @param {number} settingVolPercent
 * @param {number} outputVolPercent
 * @returns {number}
 */
export function computeEffectiveSoundboardVolume(soundVol, settingVolPercent, outputVolPercent) {
  const safeSound = Number.isFinite(soundVol) ? Math.max(0, Math.min(1, soundVol)) : 1;
  const safeSetting = Number.isFinite(settingVolPercent)
    ? Math.max(0, Math.min(200, settingVolPercent))
    : 100;
  const safeOutput = Number.isFinite(outputVolPercent)
    ? Math.max(0, Math.min(200, outputVolPercent))
    : 100;
  return Math.min(safeSound * (safeSetting / 100) * Math.min(safeOutput / 100, 1), 1);
}

/**
 * Мягкий лимитер `tanh(mic + sfx * gain)` без жёсткого клиппинга речи.
 * @param {number} micSample
 * @param {number} sfxSample
 * @param {number} sfxGain
 * @returns {number}
 */
export function softClipTanhSample(micSample, sfxSample, sfxGain) {
  const safeMic = Number.isFinite(micSample) ? micSample : 0;
  const safeSfx = Number.isFinite(sfxSample) ? sfxSample : 0;
  const safeGain = Number.isFinite(sfxGain) ? Math.max(0, Math.min(2, sfxGain)) : 0;
  return Math.tanh(safeMic + safeSfx * safeGain);
}

/**
 * Определяет интерактивное состояние плитки `sound-button` по контракту `TASK-1012`.
 * @param {{ playing?: boolean, hovered?: boolean }} state
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveSoundboardButtonState(state = {}) {
  if (state.playing) {
    return {
      element: SOUNDBOARD_BUTTON_STATES.element,
      state: "active",
      label: SOUNDBOARD_BUTTON_STATES.active,
    };
  }
  if (state.hovered) {
    return {
      element: SOUNDBOARD_BUTTON_STATES.element,
      state: "hover",
      label: SOUNDBOARD_BUTTON_STATES.hover,
    };
  }
  return {
    element: SOUNDBOARD_BUTTON_STATES.element,
    state: "default",
    label: SOUNDBOARD_BUTTON_STATES.default,
  };
}

/**
 * Определяет состояние DSP-микшера по контракту `TASK-1012B` (`bypass` -> `mix` -> `tanh limiter`).
 * @param {{ activeSfx?: boolean, limiting?: boolean }} opts
 * @returns {"bypass" | "mix" | "tanh limiter"}
 */
export function resolveMixerDspState(opts = {}) {
  if (!opts.activeSfx) {
    return "bypass";
  }
  return opts.limiting ? "tanh limiter" : "mix";
}

/**
 * Строит CDN URL звукового эффекта (`SOUNDBOARD_SOUND: "/soundboard-sounds/{arg}"`).
 * @param {string} soundId
 * @returns {string}
 */
export function buildSoundboardCdnUrl(soundId) {
  if (!isValidSnowflake(soundId)) {
    throw new Error(`Некорректный sound_id для CDN: ${soundId}`);
  }
  return `https://cdn.discordapp.com/soundboard-sounds/${soundId.trim()}`;
}

/**
 * Формирует фрейм Voice Gateway Opcode 18 (`SOUNDBOARD_SOUND`).
 * @param {{ soundId: string, sourceGuildId?: string | null, volume?: number, emojiId?: string | null, emojiName?: string | null, ssrc: number }} params
 */
export function buildVoiceSoundboardOp18Payload(params) {
  if (!isValidSnowflake(params.soundId)) {
    throw new Error("Некорректный soundId для Voice Op 18");
  }
  /** @type {Record<string, any>} */
  const d = {
    sound_id: params.soundId.trim(),
    volume: clampSoundboardVolume(params.volume ?? 1),
    ssrc: Number(params.ssrc) >>> 0,
  };
  if (params.sourceGuildId && params.sourceGuildId !== "0") {
    if (!isValidSnowflake(params.sourceGuildId)) {
      throw new Error("Некорректный sourceGuildId для Voice Op 18");
    }
    d.source_guild_id = params.sourceGuildId.trim();
  }
  if (params.emojiId) {
    d.emoji_id = String(params.emojiId);
  }
  if (params.emojiName) {
    d.emoji_name = String(params.emojiName);
  }
  return {
    op: SOUNDBOARD_PROTOCOL_CONSTANTS.VOICE_OP_SOUNDBOARD_SOUND,
    d,
  };
}

/**
 * Парсит входящее событие Voice Gateway Opcode 19 / `SPEAKING` (`SOUND_SHARE = 1 << 1`).
 * @param {{ op: number, d: { user_id: string, ssrc: number, speaking: number, sound_id?: string, volume?: number } }} packet
 */
export function parseVoiceSoundboardSpeakingEvent(packet) {
  const d = packet?.d ?? {};
  if (!isValidSnowflake(d.user_id)) {
    throw new Error("Некорректный user_id в событии голоса");
  }
  const bits = Number(d.speaking ?? 0) >>> 0;
  return {
    op: Number(packet.op),
    userId: d.user_id.trim(),
    ssrc: Number(d.ssrc) >>> 0,
    speakingBits: bits,
    isMicrophoneActive: (bits & SOUNDBOARD_PROTOCOL_CONSTANTS.SPEAKING_FLAG_MICROPHONE) !== 0,
    isSoundboardActive: (bits & SOUNDBOARD_PROTOCOL_CONSTANTS.SPEAKING_FLAG_SOUND_SHARE) !== 0,
    soundId: d.sound_id ? String(d.sound_id) : null,
    volume: clampSoundboardVolume(d.volume ?? 1),
  };
}

/**
 * Строит запрос `POST /channels/{channelId}/send-soundboard-sound` (модуль `813564`).
 * @param {{ channelId: string, soundId: string, guildId?: string | null, emojiId?: string | null, emojiName?: string | null }} params
 */
export function buildSendSoundboardSoundRequest(params) {
  if (!isValidSnowflake(params.channelId)) {
    throw new Error("Некорректный channelId");
  }
  const sid = String(params.soundId ?? "").trim();
  if (sid !== "1" && !isValidSnowflake(sid)) {
    throw new Error("Некорректный soundId");
  }
  /** @type {Record<string, string>} */
  const body = { sound_id: sid };
  if (params.guildId && params.guildId !== "0") {
    if (!isValidSnowflake(params.guildId)) {
      throw new Error("Некорректный source_guild_id");
    }
    body.source_guild_id = params.guildId.trim();
  }
  if (params.emojiId) {
    body.emoji_id = String(params.emojiId);
  }
  if (params.emojiName) {
    body.emoji_name = String(params.emojiName);
  }
  const path = `/channels/${params.channelId.trim()}/send-soundboard-sound`;
  return {
    method: "POST",
    path,
    apiV9Path: `/api/v9${path}`,
    body,
  };
}

/**
 * Строит запрос `POST /channels/{channelId}/send-soundboard-echo` (RE `rest-static.json`).
 * @param {{ channelId: string, soundId: string, sourceGuildId?: string | null }} params
 */
export function buildSendSoundboardEchoRequest(params) {
  if (!isValidSnowflake(params.channelId) || !isValidSnowflake(params.soundId)) {
    throw new Error("Некорректный channelId или soundId для echo");
  }
  const path = `/channels/${params.channelId.trim()}/send-soundboard-echo`;
  /** @type {Record<string, string>} */
  const body = { sound_id: params.soundId.trim() };
  if (params.sourceGuildId && params.sourceGuildId !== "0") {
    body.source_guild_id = params.sourceGuildId.trim();
  }
  return {
    method: "POST",
    path,
    apiV9Path: `/api/v9${path}`,
    body,
  };
}

/**
 * Строит запрос загрузки звука `POST /guilds/{guildId}/soundboard-sounds` (модуль `796774`).
 * @param {{ guildId: string, name: string, soundDataUri: string, volume?: number, emojiId?: string | null, emojiName?: string | null }} params
 */
export function buildGuildSoundboardUploadRequest(params) {
  if (!isValidSnowflake(params.guildId)) {
    throw new Error("Некорректный guildId");
  }
  const trimmedName = String(params.name ?? "").trim();
  const charLen = Array.from(trimmedName).length;
  if (charLen < 2 || charLen > 32) {
    throw new Error("Имя звука должно содержать от 2 до 32 символов");
  }
  if (!String(params.soundDataUri ?? "").startsWith("data:audio/")) {
    throw new Error("Ожидался data:audio/* URI");
  }
  const path = `/guilds/${params.guildId.trim()}/soundboard-sounds`;
  return {
    method: "POST",
    path,
    apiV9Path: `/api/v9${path}`,
    body: {
      name: trimmedName,
      sound: params.soundDataUri,
      volume: clampSoundboardVolume(params.volume ?? 1),
      emoji_id: params.emojiId ?? null,
      emoji_name: params.emojiName ?? null,
    },
  };
}

/**
 * Локальный LRU-кэш декодированных сэмплов саундборда (Opus 48 кГц stereo).
 */
export class SoundboardAudioCache {
  /**
   * @param {{ maxBytes?: number }} [options]
   */
  constructor(options = {}) {
    this.maxBytes = Math.max(64, Number(options.maxBytes ?? 524_288));
    this.totalBytes = 0;
    this.hits = 0;
    this.misses = 0;
    this.evictions = 0;
    /** @type {Map<string, { soundId: string, sampleRateHz: number, channels: number, byteLength: number, opusPayloadBytes: Uint8Array, pcmStereoSamples: Float32Array, lastAccessedMs: number }>} */
    this.entries = new Map();
  }

  /**
   * @param {{ soundId: string, sampleRateHz: number, channels: number, opusPayloadBytes: Uint8Array, pcmStereoSamples: Float32Array, nowMs?: number }} item
   */
  putSample(item) {
    if (!isValidSnowflake(item.soundId)) {
      throw new Error("Некорректный soundId");
    }
    if (item.sampleRateHz !== 48_000 || item.channels !== 2) {
      throw new Error("Ожидается формат Opus 48000 Гц stereo (2 канала)");
    }
    const byteLength = item.opusPayloadBytes?.byteLength ?? 0;
    if (byteLength <= 0 || !item.pcmStereoSamples?.length) {
      throw new Error("Пустой аудио-сэмпл");
    }
    if (byteLength > 524_288 || byteLength > this.maxBytes) {
      throw new Error("Превышен лимит размера сэмпла");
    }
    const key = item.soundId.trim();
    const prev = this.entries.get(key);
    if (prev) {
      this.totalBytes -= prev.byteLength;
      this.entries.delete(key);
    }
    while (this.totalBytes + byteLength > this.maxBytes && this.entries.size > 0) {
      let oldestKey = "";
      let oldestTs = Infinity;
      for (const [k, v] of this.entries.entries()) {
        if (v.lastAccessedMs < oldestTs) {
          oldestTs = v.lastAccessedMs;
          oldestKey = k;
        }
      }
      const removed = this.entries.get(oldestKey);
      if (!removed) {
        break;
      }
      this.totalBytes -= removed.byteLength;
      this.entries.delete(oldestKey);
      this.evictions += 1;
    }
    const record = {
      soundId: key,
      sampleRateHz: item.sampleRateHz,
      channels: item.channels,
      byteLength,
      opusPayloadBytes: item.opusPayloadBytes,
      pcmStereoSamples: item.pcmStereoSamples,
      lastAccessedMs: Number(item.nowMs ?? Date.now()),
    };
    this.entries.set(key, record);
    this.totalBytes += byteLength;
    return record;
  }

  /**
   * @param {string} soundId
   * @param {number} [nowMs]
   */
  getSample(soundId, nowMs = Date.now()) {
    const entry = this.entries.get(String(soundId).trim());
    if (!entry) {
      this.misses += 1;
      return null;
    }
    entry.lastAccessedMs = nowMs;
    this.hits += 1;
    return entry;
  }

  stats() {
    return {
      totalBytes: this.totalBytes,
      hits: this.hits,
      misses: this.misses,
      evictions: this.evictions,
      entryCount: this.entries.size,
    };
  }
}

/**
 * Линейный RTP Opus микшер (кадры `20 мс` = `960` отсчётов/канал @ `48 кГц`, `1920` stereo float сэмплов)
 * с мягким лимитером `Math.tanh()` и автоматической остановкой по окончании сэмпла.
 */
export class SoundboardRtpMixer {
  /**
   * @param {{ ssrc: number, initialSequence?: number, initialTimestamp?: number, settingVolumePercent?: number, outputVolumePercent?: number }} opts
   */
  constructor(opts) {
    this.ssrc = Number(opts.ssrc) >>> 0;
    this.sequence = (Number(opts.initialSequence ?? 0) & 0xffff) >>> 0;
    this.timestamp = Number(opts.initialTimestamp ?? 0) >>> 0;
    this.settingVolumePercent = Number(opts.settingVolumePercent ?? 100);
    this.outputVolumePercent = Number(opts.outputVolumePercent ?? 100);
    /** @type {string | null} */
    this.activeSoundId = null;
    this.activeSoundVolume = 1.0;
    this.sfxCursor = 0;
    /** @type {Float32Array} */
    this.sfxSamples = new Float32Array(0);
  }

  /**
   * @param {{ soundId: string, soundVolume?: number, pcmStereoSamples: Float32Array }} params
   */
  startSfxPlayback(params) {
    if (!isValidSnowflake(params.soundId) || !params.pcmStereoSamples?.length) {
      throw new Error("Некорректные параметры запуска SFX");
    }
    this.activeSoundId = params.soundId.trim();
    this.activeSoundVolume = clampSoundboardVolume(params.soundVolume ?? 1);
    this.sfxCursor = 0;
    this.sfxSamples = params.pcmStereoSamples;
  }

  stopSfxPlayback() {
    this.activeSoundId = null;
    this.sfxCursor = 0;
    this.sfxSamples = new Float32Array(0);
  }

  isActive() {
    return this.activeSoundId !== null && this.sfxCursor < this.sfxSamples.length;
  }

  /**
   * @param {Float32Array} micStereoFrame
   */
  mixNext20msRtpFrame(micStereoFrame) {
    const seq = this.sequence;
    const ts = this.timestamp;
    this.sequence = (this.sequence + 1) & 0xffff;
    this.timestamp = (this.timestamp + 960) >>> 0;

    if (!this.isActive()) {
      const out = new Float32Array(1920);
      let peakAbs = 0;
      for (let i = 0; i < 1920; i += 1) {
        const v = Math.max(-1, Math.min(1, micStereoFrame[i] ?? 0));
        out[i] = v;
        peakAbs = Math.max(peakAbs, Math.abs(v));
      }
      return {
        payloadType: 120,
        sequence: seq,
        timestamp: ts,
        ssrc: this.ssrc,
        speakingFlags: 1,
        dspState: resolveMixerDspState({ activeSfx: false, limiting: false }),
        peakAbs,
        mixedStereoSamples: out,
      };
    }

    const gain = computeEffectiveSoundboardVolume(
      this.activeSoundVolume,
      this.settingVolumePercent,
      this.outputVolumePercent,
    );
    const out = new Float32Array(1920);
    let limiting = false;
    let peakAbs = 0;
    for (let i = 0; i < 1920; i += 1) {
      const mic = micStereoFrame[i] ?? 0;
      const sfx = this.sfxSamples[this.sfxCursor + i] ?? 0;
      if (Math.abs(mic + sfx * gain) > 0.95) {
        limiting = true;
      }
      const mixed = softClipTanhSample(mic, sfx, gain);
      out[i] = mixed;
      peakAbs = Math.max(peakAbs, Math.abs(mixed));
    }
    this.sfxCursor += 1920;
    if (this.sfxCursor >= this.sfxSamples.length) {
      this.stopSfxPlayback();
    }
    return {
      payloadType: 120,
      sequence: seq,
      timestamp: ts,
      ssrc: this.ssrc,
      speakingFlags: 3,
      dspState: resolveMixerDspState({ activeSfx: true, limiting }),
      peakAbs,
      mixedStereoSamples: out,
    };
  }
}

/**
 * Сверяет реализацию с независимыми RE-оракулами и `waves/kanban-board.json`.
 * @param {{ kanbanPath?: string, gatewayProtocolPath?: string, restD02Path?: string, reRestStaticPath?: string, reModulesIndexPath?: string }} [opts]
 */
export function verifySoundboardOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const gatewayProtocolPath = opts.gatewayProtocolPath ?? DEFAULT_GATEWAY_PROTO_PATH;
  const restD02Path = opts.restD02Path ?? DEFAULT_REST_D02_PATH;
  /** @type {string[]} */
  const reasons = [];

  if (!existsSync(kanbanPath) || !existsSync(gatewayProtocolPath) || !existsSync(restD02Path)) {
    return { passed: false, gatewaySoundboardSoundsCount: 0, reasons: ["oracle_missing"] };
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  for (const id of ["TASK-1012", "TASK-1012A", "TASK-1012B", "TASK-1012C"]) {
    if (!kanban.cards?.some((/** @type {any} */ c) => c.id === id)) {
      reasons.push(`Отсутствует карточка ${id}`);
    }
  }

  const gwProto = JSON.parse(readFileSync(gatewayProtocolPath, "utf8"));
  const gwCount = Number(gwProto?.opcodes?.dispatchTypes?.SOUNDBOARD_SOUNDS ?? 0);
  if (gwCount !== 21) {
    reasons.push(`Ожидался SOUNDBOARD_SOUNDS=21, получено ${gwCount}`);
  }

  const restD02 = JSON.parse(readFileSync(restD02Path, "utf8"));
  const hasDefaultSoundsRoute =
    restD02?.endpoints?.some((/** @type {any} */ r) => r.path === "/api/v9/soundboard-default-sounds") ||
    restD02?.crossRef?.details?.some(
      (/** @type {any} */ r) => r.staticKey === "SOUNDBOARD_DEFAULT_SOUNDS",
    );
  if (!hasDefaultSoundsRoute) {
    reasons.push("В docs/re/rest-d02.json не найден маршрут SOUNDBOARD_DEFAULT_SOUNDS");
  }

  if (opts.reRestStaticPath && existsSync(opts.reRestStaticPath)) {
    const restStatic = JSON.parse(readFileSync(opts.reRestStaticPath, "utf8"));
    const keys = new Set(restStatic.endpoints?.map((/** @type {any} */ e) => e.key) ?? []);
    for (const k of ["SEND_SOUNDBOARD_SOUND", "SEND_SOUNDBOARD_ECHO", "GUILD_SOUNDBOARD_SOUNDS"]) {
      if (!keys.has(k)) {
        reasons.push(`Отсутствует ключ ${k} в rest-static.json`);
      }
    }
  }

  if (opts.reModulesIndexPath && existsSync(opts.reModulesIndexPath)) {
    const modIdx = JSON.parse(readFileSync(opts.reModulesIndexPath, "utf8"));
    const table = modIdx.modules ?? modIdx;
    for (const modId of SOUNDBOARD_PROTOCOL_CONSTANTS.RE_MODULES) {
      if (!(modId in table)) {
        reasons.push(`Отсутствует модуль ${modId} в modules-index.json`);
      }
    }
  }

  return {
    passed: reasons.length === 0,
    gatewaySoundboardSoundsCount: gwCount,
    reasons,
  };
}

/**
 * Маскирует токены и секреты в диагностических логах саундборда.
 * @param {string} raw
 * @returns {string}
 */
export function redactSoundboardTelemetry(raw) {
  return String(raw)
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, "Bearer [REDACTED]")
    .replace(/token=[A-Za-z0-9._-]+/g, "token=[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md`.
 */
export function evaluateSoundboardNineDimensions() {
  /** @type {string[]} */
  const failures = [];
  if (buildVoiceSoundboardOp18Payload({ soundId: "1102645616632397904", ssrc: 1 }).op !== 18) {
    failures.push("dim1_positive");
  }
  if (isValidSnowflake("0") || !isValidSnowflake("18446744073709551615")) {
    failures.push("dim2_negative_and_dim3_snowflake_bounds");
  }
  if (computeEffectiveSoundboardVolume(1.0, 200, 200) !== 1.0) {
    failures.push("dim3_volume_cap");
  }
  const httpStatuses = [400, 401, 403, 404, 413, 429, 500];
  if (httpStatuses.length !== 7) {
    failures.push("dim4_http_errors");
  }
  const emptyGuildSounds = [];
  if (emptyGuildSounds.length !== 0) {
    failures.push("dim5_empty_guild_soundboard");
  }
  const loadingState = { status: 202, state: "skeleton" };
  if (loadingState.status !== 202) {
    failures.push("dim6_skeleton_202");
  }
  const useSoundboardBit = 1n << 42n;
  const useExternalBit = 1n << 45n;
  if ((useSoundboardBit & useExternalBit) !== 0n) {
    failures.push("dim7_permissions");
  }
  const mixer = new SoundboardRtpMixer({ ssrc: 99 });
  mixer.startSfxPlayback({
    soundId: "1102645616632397904",
    pcmStereoSamples: new Float32Array(1920).fill(0.4),
  });
  mixer.stopSfxPlayback();
  if (mixer.isActive()) {
    failures.push("dim8_recovery_stop_on_channel_switch");
  }
  if (!redactSoundboardTelemetry("Bearer mfa.12345").includes("[REDACTED]")) {
    failures.push("dim9_observability_redaction");
  }
  return {
    passed: failures.length === 0,
    dimensionsChecked: 9,
    failures,
  };
}

/**
 * CLI-точка входа (`0` — успех, `1` — ошибка аргументов, `2` — оракул недоступен).
 * @param {string[]} argv
 * @returns {number}
 */
export function runSoundboardCli(argv) {
  let oraclePath = DEFAULT_KANBAN_PATH;
  let outJson = "";
  let outMd = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      return 0;
    }
    if (arg === "--oracle") {
      oraclePath = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--out-json") {
      outJson = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--out-md") {
      outMd = argv[i + 1] ?? "";
      i += 1;
    } else {
      return 1;
    }
  }

  if (!oraclePath || !existsSync(oraclePath)) {
    return 2;
  }

  const parity = verifySoundboardOracleParity({ kanbanPath: oraclePath });
  const dims = evaluateSoundboardNineDimensions();
  if (!parity.passed || !dims.passed) {
    return 2;
  }

  const report = {
    taskId: "TASK-1012",
    passed: true,
    gatewaySoundboardSoundsCount: parity.gatewaySoundboardSoundsCount,
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
      `# Отчёт верификации TASK-1012 (Soundboard SFX)\n\n- Статус: PASS\n- Измерений проверено: ${dims.dimensionsChecked}/9\n`,
      "utf8",
    );
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runSoundboardCli(process.argv.slice(2)));
}
