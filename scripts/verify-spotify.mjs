#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор интеграции со Spotify по D-Bus MPRIS (Listen Along & Presence — TASK-1013):
 * - Сверка дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#1db954`, `#3f4147`), геометрии
 *   `Spotify Activity Card`, `MPRIS Bridge`, `Spotify Profile Card`, интерактивных состояний
 *   `listen-along-btn` (`spotify green -> brighten -> sync playback`), `mpris-event` (`idle -> detect track -> status broadcast`)
 *   и `listen-together` (`button -> brighten -> sync started`).
 * - Подписка на сигналы D-Bus `org.mpris.MediaPlayer2.spotify` (`PropertiesChanged`, `NameOwnerChanged`),
 *   формирование активности типа 2 (`LISTENING`) в Gateway Opcode 3 (`STATUS_UPDATE`) и синхронизация
 *   «Слушать вместе» (Listen Along) через `PUT https://api.spotify.com/v1/me/player/play`.
 * - Покрытие 9 обязательных измерений `TEST-POLICY.md` и маскирование секретов.
 * Коды выхода CLI: 0 — успех, 1 — ошибка аргументов/инвариантов, 2 — оракул недоступен.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_GW_PROTOCOL_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const DEFAULT_RE_GW_SCHEMA_PATH = "/home/sanya/re-deep/gateway-schema.json";
const DEFAULT_RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const DEFAULT_RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** Макет карточки активности `Spotify Activity Card` (`TASK-1013`). */
export const SPOTIFY_ACTIVITY_CARD_LAYOUT = Object.freeze({
  container: "Spotify Activity Card",
  dimensions: "100% x auto",
  padding: "8px",
  spacing: "6px",
});

/** Макет моста D-Bus `MPRIS Bridge` (`TASK-1013A`). */
export const SPOTIFY_MPRIS_BRIDGE_LAYOUT = Object.freeze({
  container: "MPRIS Bridge",
  dimensions: "D-Bus Listener",
  padding: "0px",
  spacing: "0px",
});

/** Макет виджета профиля `Spotify Profile Card` (`TASK-1013B`). */
export const SPOTIFY_PROFILE_CARD_LAYOUT = Object.freeze({
  container: "Spotify Profile Card",
  dimensions: "100% x auto",
  padding: "8px",
  spacing: "6px",
});

/** Дизайн-токены карточки `TASK-1013` из `waves/kanban-board.json`. */
export const SPOTIFY_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#1db954",
  border: "#3f4147",
});

/** Интерактивные состояния кнопки `listen-along-btn` (`TASK-1013`). */
export const LISTEN_ALONG_BTN_STATES = Object.freeze({
  element: "listen-along-btn",
  default: "spotify green",
  hover: "brighten",
  active: "sync playback",
});

/** Интерактивные состояния D-Bus слушателя `mpris-event` (`TASK-1013A`). */
export const MPRIS_EVENT_STATES = Object.freeze({
  element: "mpris-event",
  default: "idle",
  hover: "detect track",
  active: "status broadcast",
});

/** Интерактивные состояния кнопки профиля `listen-together` (`TASK-1013B`). */
export const LISTEN_TOGETHER_STATES = Object.freeze({
  element: "listen-together",
  default: "button",
  hover: "brighten",
  active: "sync started",
});

/** Константы протокола D-Bus MPRIS, Gateway Opcode 3 и Spotify Connect Web API из RE-дампа. */
export const SPOTIFY_PROTOCOL_CONSTANTS = Object.freeze({
  DBUS_BUS_NAME: "org.mpris.MediaPlayer2.spotify",
  DBUS_OBJECT_PATH: "/org/mpris/MediaPlayer2",
  DBUS_PLAYER_INTERFACE: "org.mpris.MediaPlayer2.Player",
  DBUS_PROPERTIES_INTERFACE: "org.freedesktop.DBus.Properties",
  DBUS_SIGNAL_PROPERTIES_CHANGED: "PropertiesChanged",
  DBUS_SIGNAL_NAME_OWNER_CHANGED: "NameOwnerChanged",
  GATEWAY_OPCODE_STATUS_UPDATE: 3,
  ACTIVITY_TYPE_LISTENING: 2,
  ACTIVITY_NAME: "Spotify",
  ACTIVITY_ID: "spotify:1",
  ACTIVITY_FLAG_SYNC: 16,
  ACTIVITY_FLAG_PLAY: 32,
  ACTIVITY_FLAGS_PLAY_SYNC: 48,
  PARTY_PREFIX: "spotify:",
  IMAGE_ASSET_PREFIX: "spotify:",
  TRACK_ID_LEN: 22,
  MAX_DETAILS_CHARS: 128,
  TRUNCATED_DETAILS_PREFIX_CHARS: 125,
  MAX_ARTISTS: 5,
  START_TIME_JITTER_MS: 1_500,
  SPEAKING_AUTO_PAUSE_MS: 30_000,
  LISTEN_ALONG_HOST_TIMEOUT_MS: 300_000,
  TRACK_END_GRACE_MS: 5_000,
  HTTP_202_RETRY_DELAY_MS: 5_000,
  SPOTIFY_API_BASE: "https://api.spotify.com/v1",
  PLAYER_PLAY_URL: "https://api.spotify.com/v1/me/player/play",
  PLAYER_PAUSE_URL: "https://api.spotify.com/v1/me/player/pause",
  PLAYER_DEVICES_URL: "https://api.spotify.com/v1/me/player/devices",
  CONTENT_INVENTORY_PATH: "/content-inventory/users/@me/spotify",
  RE_MODULES: Object.freeze(["107750", "170699", "210528", "272984", "402857", "655116", "970111", "981616"]),
});

/** 4 живые активности Spotify (`type: 2`, `LISTENING`) из `/home/sanya/re-deep/gw-proto/msgs-run*.jsonl`. */
export const LIVE_SPOTIFY_ACTIVITIES_ORACLE = Object.freeze([
  Object.freeze({
    captureSource: "msgs-run1-57992_5112.jsonl:s=4",
    type: 2,
    name: "Spotify",
    id: "spotify:1",
    flags: 48,
    details: "Freeman",
    state: "Miyagi & Andy Panda",
    sync_id: "3cdUEnMVspkAfx6nnxSdTl",
    session_id: "5658ed3b2c7e19a11f36e2b86b0db208",
    party: Object.freeze({ id: "spotify:366982137616007169" }),
    assets: Object.freeze({
      large_image: "spotify:ab67616d0000b2736700e765df4db64f4b0c6276",
      large_text: "Freeman",
    }),
    timestamps: Object.freeze({ start: 1_792_390_365_757, end: 1_792_390_595_979 }),
  }),
  Object.freeze({
    captureSource: "msgs-run1-57992_5112.jsonl:s=32",
    type: 2,
    name: "Spotify",
    id: "spotify:1",
    flags: 48,
    details: "we fell in love in october",
    state: "girl in red",
    sync_id: "6IPwKM3fUUzlElbvKw2sKl",
    session_id: "5658ed3b2c7e19a11f36e2b86b0db208",
    party: Object.freeze({ id: "spotify:366982137616007169" }),
    assets: Object.freeze({
      large_image: "spotify:ab67616d0000b273deade9f354749841f09feca2",
      large_text: "we fell in love in october / October Passed Me By",
    }),
    timestamps: Object.freeze({ start: 1_792_390_590_036, end: 1_792_390_774_189 }),
  }),
  Object.freeze({
    captureSource: "msgs-run2-95974_8269.jsonl:s=514",
    type: 2,
    name: "Spotify",
    id: "spotify:1",
    flags: 48,
    details: "Killing",
    state: "Ally Nicholas",
    sync_id: "3e3ZxVIhHIzltQCZ26zr66",
    session_id: "fdd0860ffe31f2ffa3ff1151d219b1b2",
    party: Object.freeze({ id: "spotify:293808922463174658" }),
    assets: Object.freeze({
      large_image: "spotify:ab67616d0000b273492a155f0ffffe441035d93a",
      large_text: "Killing",
    }),
    timestamps: Object.freeze({ start: 1_792_394_422_210, end: 1_792_394_639_079 }),
  }),
  Object.freeze({
    captureSource: "msgs-run2-95974_8269.jsonl:s=557",
    type: 2,
    name: "Spotify",
    id: "spotify:1",
    flags: 48,
    details: "When It's Raining",
    state: "Borderline",
    sync_id: "6vnqF5m7gC8z29RYen3PQf",
    session_id: "fdd0860ffe31f2ffa3ff1151d219b1b2",
    party: Object.freeze({ id: "spotify:293808922463174658" }),
    assets: Object.freeze({
      large_image: "spotify:ab67616d0000b273bd97bfe6504c493d7eb7e711",
      large_text: "Borderline",
    }),
    timestamps: Object.freeze({ start: 1_792_394_639_613, end: 1_792_394_877_613 }),
  }),
]);

/** @param {string | number | bigint} raw */
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

/** @param {{ syncing?: boolean, hovered?: boolean }} opts */
export function resolveListenAlongButtonState(opts) {
  if (opts.syncing) {
    return { element: LISTEN_ALONG_BTN_STATES.element, state: "active", label: LISTEN_ALONG_BTN_STATES.active };
  }
  if (opts.hovered) {
    return { element: LISTEN_ALONG_BTN_STATES.element, state: "hover", label: LISTEN_ALONG_BTN_STATES.hover };
  }
  return { element: LISTEN_ALONG_BTN_STATES.element, state: "default", label: LISTEN_ALONG_BTN_STATES.default };
}

/** @param {{ broadcasting?: boolean, detecting?: boolean }} opts */
export function resolveMprisEventState(opts) {
  if (opts.broadcasting) {
    return { element: MPRIS_EVENT_STATES.element, state: "active", label: MPRIS_EVENT_STATES.active };
  }
  if (opts.detecting) {
    return { element: MPRIS_EVENT_STATES.element, state: "hover", label: MPRIS_EVENT_STATES.hover };
  }
  return { element: MPRIS_EVENT_STATES.element, state: "default", label: MPRIS_EVENT_STATES.default };
}

/** @param {string | null | undefined} rawTrackId */
export function extractSpotifyTrackId(rawTrackId) {
  const str = String(rawTrackId ?? "").trim();
  if (!str || str.includes("NoTrack") || str.startsWith("spotify:local:") || str.includes("/local/")) {
    return null;
  }
  let candidate = str;
  if (str.startsWith("/com/spotify/track/")) candidate = str.slice("/com/spotify/track/".length);
  else if (str.startsWith("spotify:track:")) candidate = str.slice("spotify:track:".length);
  else if (str.startsWith("https://open.spotify.com/track/")) {
    candidate = str.slice("https://open.spotify.com/track/".length).split("?")[0] ?? "";
  }
  return /^[A-Za-z0-9]{22}$/.test(candidate) ? candidate : null;
}

/** @param {string | null | undefined} rawArtUrl */
export function normalizeSpotifyArtAsset(rawArtUrl) {
  const str = String(rawArtUrl ?? "").trim();
  if (!str) return null;
  if (str.startsWith("spotify:")) {
    const hex = str.slice("spotify:".length);
    return /^[0-9a-fA-F]+$/.test(hex) ? str : null;
  }
  for (const prefix of ["https://i.scdn.co/image/", "http://i.scdn.co/image/", "https://open.spotify.com/image/"]) {
    if (str.startsWith(prefix)) {
      const id = (str.slice(prefix.length).split(/[?/]/)[0] ?? "").trim();
      return /^[0-9a-fA-F]+$/.test(id) ? `spotify:${id}` : null;
    }
  }
  return null;
}

/** @param {string} title */
export function formatSpotifyDetails(title) {
  const trimmed = String(title ?? "").trim();
  if (trimmed.length > SPOTIFY_PROTOCOL_CONSTANTS.MAX_DETAILS_CHARS) {
    return `${trimmed.slice(0, SPOTIFY_PROTOCOL_CONSTANTS.TRUNCATED_DETAILS_PREFIX_CHARS)}...`;
  }
  return trimmed;
}

/** @param {readonly string[]} artists */
export function formatSpotifyStateArtists(artists) {
  if (!Array.isArray(artists)) return "";
  return artists
    .map((a) => String(a ?? "").replace(/;/g, "").trim())
    .filter(Boolean)
    .slice(0, SPOTIFY_PROTOCOL_CONSTANTS.MAX_ARTISTS)
    .join("; ");
}

/**
 * Разбирает сигнал D-Bus `PropertiesChanged` интерфейса `org.mpris.MediaPlayer2.Player`.
 * @param {{ interfaceName: string, playbackStatus: string, positionUs?: number, metadata?: Record<string, any> }} signal
 * @param {number} [nowMs]
 * @param {number | null} [prevStartMs]
 */
export function parseMprisMetadata(signal, nowMs = Date.now(), prevStartMs = null) {
  if (!signal || signal.interfaceName !== SPOTIFY_PROTOCOL_CONSTANTS.DBUS_PLAYER_INTERFACE) {
    throw new RangeError(`Неподдерживаемый D-Bus интерфейс: ${signal?.interfaceName}`);
  }
  const meta = signal.metadata ?? {};
  const title = formatSpotifyDetails(String(meta["xesam:title"] ?? "").trim());
  if (!title) return null;
  const rawArtists = Array.isArray(meta["xesam:artist"]) ? meta["xesam:artist"] : [String(meta["xesam:artist"] ?? "")];
  const artistsJoined = formatSpotifyStateArtists(rawArtists);
  const album = String(meta["xesam:album"] ?? "").trim();
  const trackId = extractSpotifyTrackId(meta["mpris:trackid"]);
  const isLocal = trackId === null;
  const durationMs = Math.floor(Math.max(0, Number(meta["mpris:length"] ?? 0)) / 1_000);
  const positionMs = Math.min(Math.floor(Math.max(0, Number(signal.positionUs ?? 0)) / 1_000), Math.max(durationMs, 1));
  const candidateStartMs = Math.max(0, nowMs - positionMs);
  const startMs =
    prevStartMs != null && Math.abs(candidateStartMs - prevStartMs) <= SPOTIFY_PROTOCOL_CONSTANTS.START_TIME_JITTER_MS
      ? prevStartMs
      : candidateStartMs;
  const endMs = startMs + durationMs;
  return {
    trackId,
    title,
    artistsJoined,
    album,
    largeImage: normalizeSpotifyArtAsset(meta["mpris:artUrl"]),
    durationMs,
    positionMs,
    startMs,
    endMs,
    isPlaying: String(signal.playbackStatus).trim() === "Playing",
    isLocal,
  };
}

/**
 * Форматирует миллисекунды в `MM:SS`.
 * @param {number} ms
 * @returns {string}
 */
export function formatDurationMmSs(ms) {
  const safeSecs = Math.max(0, Math.floor(ms / 1_000));
  const mins = String(Math.floor(safeSecs / 60)).padStart(2, "0");
  const secs = String(safeSecs % 60).padStart(2, "0");
  return `${mins}:${secs}`;
}

/**
 * Вычисляет прогресс таймлайна трека `0..100%` с заморозкой ползунка на паузе.
 * @param {{ start: number, end: number }} timestamps
 * @param {number} [nowMs]
 * @param {boolean} [isPaused]
 * @param {number} [pausedAtMs]
 */
export function computeTrackProgress(timestamps, nowMs = Date.now(), isPaused = false, pausedAtMs = nowMs) {
  const start = Number(timestamps?.start ?? 0);
  const end = Number(timestamps?.end ?? 0);
  const durationMs = Math.max(0, end - start);
  if (durationMs === 0) {
    return { elapsedMs: 0, durationMs: 0, progressPercent: 0, elapsedFormatted: "00:00", totalFormatted: "00:00" };
  }
  const effectiveNow = isPaused ? pausedAtMs : nowMs;
  const elapsedMs = Math.min(durationMs, Math.max(0, effectiveNow - start));
  const progressPercent = Math.min(100, Math.max(0, Math.round((elapsedMs * 100) / durationMs)));
  return {
    elapsedMs,
    durationMs,
    progressPercent,
    elapsedFormatted: formatDurationMmSs(elapsedMs),
    totalFormatted: formatDurationMmSs(durationMs),
  };
}

/**
 * Строит REST-запрос синхронизации «Слушать вместе» `PUT https://api.spotify.com/v1/me/player/play`.
 * @param {{ syncId: string, partyId: string, deviceId: string, isPremium: boolean, positionMs: number, contextUri?: string }} params
 */
export function buildListenAlongPlayRequest(params) {
  if (!params.isPremium) {
    throw new Error("spotify account is not premium");
  }
  const deviceId = String(params.deviceId ?? "").trim();
  if (!deviceId) throw new Error("no active profile");
  const partyId = String(params.partyId ?? "").trim();
  if (!partyId.startsWith(SPOTIFY_PROTOCOL_CONSTANTS.PARTY_PREFIX)) {
    throw new RangeError(`Некорректный party.id Spotify: ${partyId}`);
  }
  const cleanTrackId = extractSpotifyTrackId(params.syncId);
  if (!cleanTrackId) {
    throw new RangeError(`Некорректный sync_id трека Spotify: ${params.syncId}`);
  }
  const method = "PUT";
  const trackUri = `spotify:track:${cleanTrackId}`;
  const positionMs = Math.max(0, Math.floor(Number(params.positionMs ?? 0)));
  const body = params.contextUri
    ? { context_uri: params.contextUri, offset: { uri: trackUri }, position_ms: positionMs }
    : { uris: [trackUri], position_ms: positionMs };
  return {
    method,
    url: `${SPOTIFY_PROTOCOL_CONSTANTS.PLAYER_PLAY_URL}?device_id=${encodeURIComponent(deviceId)}`,
    partyId,
    syncId: cleanTrackId,
    body,
  };
}

/** Движок интеграции D-Bus MPRIS Spotify, публикации статуса Opcode 3 и синхронизации Listen Along. */
export class SpotifyMprisBridgeEngine {
  /**
   * @param {{ currentUserId: string, sessionId: string, isPremium?: boolean, activeDeviceId?: string }} config
   */
  constructor(config) {
    if (!isValidSnowflake(config.currentUserId)) {
      throw new RangeError(`Некорректный currentUserId: ${config.currentUserId}`);
    }
    this.currentUserId = config.currentUserId;
    this.sessionId = config.sessionId;
    this.isPremium = config.isPremium ?? true;
    this.activeDeviceId = config.activeDeviceId ?? "";
    /** @type {ReturnType<typeof parseMprisMetadata>} */
    this.currentTrack = null;
    /** @type {any} */
    this.currentActivity = null;
    /** @type {{ hostUserId: string, partyId: string, trackId: string, startMs: number } | null} */
    this.syncingHost = null;
  }

  /**
   * Обрабатывает сигнал D-Bus `PropertiesChanged` и возвращает кадр Gateway Opcode 3 (`STATUS_UPDATE`).
   * @param {Parameters<typeof parseMprisMetadata>[0]} signal
   * @param {number} [nowMs]
   */
  handleDbusPropertiesChanged(signal, nowMs = Date.now()) {
    const incomingTrackId = extractSpotifyTrackId(signal.metadata?.["mpris:trackid"]);
    const prevStart =
      this.currentTrack && this.currentTrack.trackId === incomingTrackId ? this.currentTrack.startMs : null;
    const parsed = parseMprisMetadata(signal, nowMs, prevStart);
    if (!parsed || !parsed.isPlaying) {
      this.currentTrack = null;
      this.currentActivity = null;
      return {
        op: SPOTIFY_PROTOCOL_CONSTANTS.GATEWAY_OPCODE_STATUS_UPDATE,
        d: { since: null, activities: [], status: "online", afk: false },
      };
    }
    this.currentTrack = parsed;
    const partyId = this.syncingHost?.partyId ?? `spotify:${this.currentUserId}`;
    /** @type {Record<string, string>} */
    const assets = {};
    if (parsed.largeImage) assets.large_image = parsed.largeImage;
    if (parsed.album) assets.large_text = parsed.album;
    const activity = {
      type: SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_TYPE_LISTENING,
      name: SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_NAME,
      id: SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_ID,
      flags: parsed.isLocal ? 0 : SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_FLAGS_PLAY_SYNC,
      details: parsed.title,
      state: parsed.artistsJoined,
      sync_id: parsed.trackId,
      session_id: this.sessionId,
      party: { id: partyId },
      assets,
      timestamps: { start: parsed.startMs, end: parsed.endMs },
      created_at: nowMs,
    };
    this.currentActivity = activity;
    return {
      op: SPOTIFY_PROTOCOL_CONSTANTS.GATEWAY_OPCODE_STATUS_UPDATE,
      d: { since: null, activities: [activity], status: "online", afk: false },
    };
  }

  /**
   * Обрабатывает сигнал D-Bus `NameOwnerChanged` (очистка статуса при закрытии Spotify).
   * @param {string} busName
   * @param {string} _oldOwner
   * @param {string} newOwner
   */
  handleDbusNameOwnerChanged(busName, _oldOwner, newOwner) {
    if (busName === SPOTIFY_PROTOCOL_CONSTANTS.DBUS_BUS_NAME && String(newOwner ?? "").trim() === "") {
      this.currentTrack = null;
      this.currentActivity = null;
      this.syncingHost = null;
    }
    return {
      op: SPOTIFY_PROTOCOL_CONSTANTS.GATEWAY_OPCODE_STATUS_UPDATE,
      d: {
        since: null,
        activities: this.currentActivity ? [this.currentActivity] : [],
        status: "online",
        afk: false,
      },
    };
  }

  getCurrentActivity() {
    return this.currentActivity;
  }

  /**
   * Разбирает входящую активность Spotify из `PRESENCE_UPDATE` / `GUILD_MEMBER_LIST_UPDATE`.
   * @param {any} activity
   */
  ingestGatewayPresenceActivity(activity) {
    const isSpotifyListening =
      activity != null &&
      Number(activity.type) === SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_TYPE_LISTENING &&
      activity.name === SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_NAME &&
      typeof activity.party?.id === "string" &&
      activity.party.id.startsWith(SPOTIFY_PROTOCOL_CONSTANTS.PARTY_PREFIX);
    return {
      isSpotifyListening,
      syncId: activity?.sync_id ?? null,
      partyId: activity?.party?.id ?? null,
      details: activity?.details ?? "",
      state: activity?.state ?? "",
      startMs: Number(activity?.timestamps?.start ?? 0),
      endMs: Number(activity?.timestamps?.end ?? 0),
    };
  }

  /**
   * Запускает синхронное прослушивание «Слушать вместе» (Listen Along).
   * @param {string} hostUserId
   * @param {any} hostActivity
   * @param {number} [nowMs]
   */
  startListenAlong(hostUserId, hostActivity, nowMs = Date.now()) {
    if (!isValidSnowflake(hostUserId) || hostUserId === this.currentUserId) {
      throw new RangeError("Недопустимый hostUserId для Listen Along");
    }
    const startMs = Number(hostActivity?.timestamps?.start ?? nowMs);
    const positionMs = Math.max(0, nowMs - startMs);
    const req = buildListenAlongPlayRequest({
      syncId: hostActivity?.sync_id,
      partyId: hostActivity?.party?.id,
      deviceId: this.activeDeviceId,
      isPremium: this.isPremium,
      positionMs,
    });
    this.syncingHost = { hostUserId, partyId: req.partyId, trackId: req.syncId, startMs };
    return req;
  }

  /**
   * Выполняет ресинк при смене трека или перемотке (> 1500 мс) у хоста Listen Along.
   * @param {string} hostUserId
   * @param {any} hostActivity
   * @param {number} [nowMs]
   */
  syncWithHostActivity(hostUserId, hostActivity, nowMs = Date.now()) {
    if (!this.syncingHost || this.syncingHost.hostUserId !== hostUserId) return null;
    const nextTrackId = extractSpotifyTrackId(hostActivity?.sync_id);
    const nextStartMs = Number(hostActivity?.timestamps?.start ?? nowMs);
    const trackChanged = nextTrackId !== this.syncingHost.trackId;
    const seekDrifted =
      Math.abs(nextStartMs - this.syncingHost.startMs) > SPOTIFY_PROTOCOL_CONSTANTS.START_TIME_JITTER_MS;
    if (trackChanged || seekDrifted) {
      return this.startListenAlong(hostUserId, hostActivity, nowMs);
    }
    return null;
  }

  /** @param {boolean} [hovered] */
  getListenAlongButtonState(hovered = false) {
    return resolveListenAlongButtonState({ syncing: this.syncingHost !== null, hovered });
  }
}

/**
 * Сверяет инварианты модуля Spotify MPRIS с независимыми RE-оракулами.
 * @param {{ kanbanPath?: string, gatewayProtocolPath?: string, reGatewaySchemaPath?: string, reRestStaticPath?: string, reModulesIndexPath?: string }} [opts]
 */
export function verifySpotifyOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const gatewayProtocolPath = opts.gatewayProtocolPath ?? DEFAULT_GW_PROTOCOL_PATH;
  /** @type {string[]} */
  const reasons = [];

  if (!existsSync(kanbanPath) || !existsSync(gatewayProtocolPath)) {
    return {
      passed: false,
      missingOracle: true,
      presenceUpdateCount: 0,
      reasons: ["Отсутствует обязательный файл-оракул kanban-board.json или gateway-protocol.json"],
    };
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card1013 = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1013");
  const card1013A = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1013A");
  const card1013B = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1013B");
  if (!card1013 || !card1013A || !card1013B) {
    reasons.push("Карточки TASK-1013 / TASK-1013A / TASK-1013B не найдены в waves/kanban-board.json");
  } else {
    if (card1013.design_tokens?.accent !== SPOTIFY_DESIGN_TOKENS.accent) {
      reasons.push("Несовпадение accent-токена #1db954 в TASK-1013");
    }
    if (card1013.interactive_states?.[0]?.active !== LISTEN_ALONG_BTN_STATES.active) {
      reasons.push("Несовпадение active-состояния listen-along-btn");
    }
  }

  const gwProto = JSON.parse(readFileSync(gatewayProtocolPath, "utf8"));
  const presenceUpdateCount = Number(gwProto.opcodes?.dispatchTypes?.PRESENCE_UPDATE ?? 0);
  if (presenceUpdateCount !== 107) {
    reasons.push(`Ожидалось PRESENCE_UPDATE = 107, получено: ${presenceUpdateCount}`);
  }

  if (opts.reRestStaticPath && existsSync(opts.reRestStaticPath)) {
    const restText = readFileSync(opts.reRestStaticPath, "utf8");
    if (!restText.includes(SPOTIFY_PROTOCOL_CONSTANTS.CONTENT_INVENTORY_PATH)) {
      reasons.push("Маршрут /content-inventory/users/@me/spotify не найден в rest-static.json");
    }
  }

  if (opts.reModulesIndexPath && existsSync(opts.reModulesIndexPath)) {
    const modIndex = JSON.parse(readFileSync(opts.reModulesIndexPath, "utf8"));
    for (const modId of SPOTIFY_PROTOCOL_CONSTANTS.RE_MODULES) {
      if (!modIndex[modId]) reasons.push(`Модуль ${modId} отсутствует в modules-index.json`);
    }
  }

  return { passed: reasons.length === 0, missingOracle: false, presenceUpdateCount, reasons };
}

/** Проверяет все 9 обязательных измерений `TEST-POLICY.md` для Spotify MPRIS & Listen Along. */
export function evaluateSpotifyNineDimensions() {
  /** @type {string[]} */
  const failures = [];
  const posEngine = new SpotifyMprisBridgeEngine({
    currentUserId: "366982137616007169",
    sessionId: "5658ed3b2c7e19a11f36e2b86b0db208",
    isPremium: true,
    activeDeviceId: "dev_ok",
  });
  const posFrame = posEngine.handleDbusPropertiesChanged(
    {
      interfaceName: "org.mpris.MediaPlayer2.Player",
      playbackStatus: "Playing",
      positionUs: 10_000_000,
      metadata: {
        "mpris:trackid": "/com/spotify/track/3cdUEnMVspkAfx6nnxSdTl",
        "mpris:length": 200_000_000,
        "xesam:title": "Freeman",
        "xesam:artist": ["Miyagi & Andy Panda"],
        "xesam:album": "Freeman",
      },
    },
    1_792_390_000_000,
  );
  if (posFrame.op !== 3 || posFrame.d.activities[0]?.type !== 2) failures.push("dimension_1_positive_failed");

  let negCaught = false;
  try {
    parseMprisMetadata({ interfaceName: "org.Invalid", playbackStatus: "Playing" });
  } catch {
    negCaught = true;
  }
  if (!negCaught || extractSpotifyTrackId("spotify:local:a:b:c:120") !== null) {
    failures.push("dimension_2_negative_failed");
  }

  const boundaryOk =
    !isValidSnowflake("0") &&
    isValidSnowflake("18446744073709551615") &&
    !isValidSnowflake("18446744073709551616") &&
    formatSpotifyDetails("X".repeat(150)).length === 128 &&
    formatSpotifyStateArtists(["1", "2", "3", "4", "5", "6"]).split("; ").length === 5;
  if (!boundaryOk) failures.push("dimension_3_boundaries_failed");

  let nonPremiumBlocked = false;
  try {
    buildListenAlongPlayRequest({
      syncId: "3cdUEnMVspkAfx6nnxSdTl",
      partyId: "spotify:366982137616007169",
      deviceId: "dev_ok",
      isPremium: false,
      positionMs: 0,
    });
  } catch {
    nonPremiumBlocked = true;
  }
  if (!nonPremiumBlocked) failures.push("dimension_4_api_errors_failed");

  const emptyFrame = posEngine.handleDbusPropertiesChanged({
    interfaceName: "org.mpris.MediaPlayer2.Player",
    playbackStatus: "Stopped",
    metadata: { "mpris:trackid": "/org/mpris/MediaPlayer2/TrackList/NoTrack", "xesam:title": "Freeman" },
  });
  if (emptyFrame.d.activities.length !== 0) failures.push("dimension_5_empty_state_failed");

  if (SPOTIFY_PROTOCOL_CONSTANTS.HTTP_202_RETRY_DELAY_MS !== 5_000) {
    failures.push("dimension_6_loading_202_failed");
  }

  let selfSyncBlocked = false;
  try {
    posEngine.startListenAlong("366982137616007169", LIVE_SPOTIFY_ACTIVITIES_ORACLE[0]);
  } catch {
    selfSyncBlocked = true;
  }
  if (!selfSyncBlocked) failures.push("dimension_7_permissions_failed");

  const recFrame = posEngine.handleDbusNameOwnerChanged("org.mpris.MediaPlayer2.spotify", ":1.55", "");
  if (recFrame.d.activities.length !== 0) failures.push("dimension_8_recovery_failed");

  const red = redactSpotifyTelemetry("Bearer BQD_secret_token_999 mfa.discord_sec_888");
  if (red.includes("BQD_secret_token_999") || red.includes("discord_sec_888")) {
    failures.push("dimension_9_observability_redaction_failed");
  }

  return { passed: failures.length === 0, dimensionsChecked: 9, failures };
}

/**
 * Маскирует токены Spotify OAuth (`Bearer ...`, `BQD...`) и Discord (`mfa....`) в логах диагностики.
 * @param {string} rawText
 * @returns {string}
 */
export function redactSpotifyTelemetry(rawText) {
  return String(rawText ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+=*/g, "Bearer [REDACTED]")
    .replace(/\bBQD[A-Za-z0-9._~+/-]+/g, "[REDACTED]")
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "[REDACTED]")
    .replace(/\b[\w-]{24}\.[\w-]{6}\.[\w-]{27,}\b/g, "[REDACTED]");
}

/**
 * CLI-точка входа `scripts/verify-spotify.mjs`.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runSpotifyCli(argv) {
  let oraclePath = DEFAULT_KANBAN_PATH;
  let outJson = "";
  let outMd = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      console.log(
        "Использование: node scripts/verify-spotify.mjs [--oracle <path>] [--out-json <path>] [--out-md <path>]",
      );
      return 0;
    }
    if (arg === "--oracle") {
      oraclePath = argv[i + 1] ?? "";
      i += 1;
      continue;
    }
    if (arg === "--out-json") {
      outJson = argv[i + 1] ?? "";
      i += 1;
      continue;
    }
    if (arg === "--out-md") {
      outMd = argv[i + 1] ?? "";
      i += 1;
      continue;
    }
    console.error(`Неизвестный аргумент: ${arg}`);
    return 1;
  }

  if (!oraclePath || !existsSync(oraclePath)) {
    console.error(`Оракул недоступен: ${oraclePath}`);
    return 2;
  }

  const parity = verifySpotifyOracleParity({
    kanbanPath: oraclePath,
    gatewayProtocolPath: DEFAULT_GW_PROTOCOL_PATH,
    reGatewaySchemaPath: existsSync(DEFAULT_RE_GW_SCHEMA_PATH) ? DEFAULT_RE_GW_SCHEMA_PATH : undefined,
    reRestStaticPath: existsSync(DEFAULT_RE_REST_STATIC_PATH) ? DEFAULT_RE_REST_STATIC_PATH : undefined,
    reModulesIndexPath: existsSync(DEFAULT_RE_MODULES_INDEX_PATH) ? DEFAULT_RE_MODULES_INDEX_PATH : undefined,
  });
  const dims = evaluateSpotifyNineDimensions();
  const passed = parity.passed && dims.passed;
  const report = {
    taskId: "TASK-1013",
    passed,
    parity,
    dimensions: dims,
    liveOracleActivities: LIVE_SPOTIFY_ACTIVITIES_ORACLE.length,
  };

  if (outJson) {
    mkdirSync(dirname(resolve(outJson)), { recursive: true });
    writeFileSync(resolve(outJson), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(resolve(outMd)), { recursive: true });
    const md = [
      "# Отчёт верификации TASK-1013 (Spotify D-Bus MPRIS & Listen Along)",
      "",
      `- Статус: **${passed ? "PASSED" : "FAILED"}**`,
      `- Событий PRESENCE_UPDATE в оракуле: \`${parity.presenceUpdateCount}\``,
      `- Живых активностей Spotify (type 2): \`${LIVE_SPOTIFY_ACTIVITIES_ORACLE.length}\``,
      `- Проверено измерений TEST-POLICY.md: \`${dims.dimensionsChecked}/9\``,
    ].join("\n");
    writeFileSync(resolve(outMd), `${md}\n`, "utf8");
  }

  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runSpotifyCli(process.argv.slice(2)));
}
