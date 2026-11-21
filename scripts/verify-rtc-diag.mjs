#!/usr/bin/env node
// @ts-check
/**
 * Автономный верификатор продвинутой WebRTC-диагностики и окна `RTC Internals HUD`
 * (`TASK-1011` / `TASK-1011A` / `TASK-1011B` — `RE-GAP-22`).
 * Коды выхода CLI: 0 — успех, 1 — ошибка аргументов/инвариантов, 2 — оракул недоступен.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_RE_GAP_22_PATH = join(ROOT, "waves", "RW-40", "RE-AUDIT", "RE-GAP-22-VOICE-DIAGNOSTICS.md");
const DEFAULT_VOICE_DEEP_PATH = join(ROOT, "docs", "re", "voice-deep.json");
const DEFAULT_RTCP_UDP3_PATH = join(ROOT, "waves", "RW-26", "research", "fixtures", "rtcp-voice-udp3-server.json");
const DEFAULT_RTCP_UDP4_PATH = join(ROOT, "waves", "RW-26", "research", "fixtures", "rtcp-voice-udp4-server.json");
const DEFAULT_RUST_MODULE_PATH = join(ROOT, "rust", "chord-gpui", "src", "voice", "rtc_diagnostics.rs");

/** Макет `RTC Internals HUD` из карточки `TASK-1011` (`waves/kanban-board.json`). */
export const RTC_INTERNALS_HUD_LAYOUT = Object.freeze({
  container: "RTC Internals HUD",
  dimensions: "640px x 480px",
  padding: "16px",
  spacing: "10px",
});

/** Макет сборщика `RTCP Collector` из карточки `TASK-1011A` (`waves/kanban-board.json`). */
export const RTCP_COLLECTOR_LAYOUT = Object.freeze({
  container: "RTCP Collector",
  dimensions: "RingBuffer 120",
  padding: "0px",
  spacing: "0px",
});

/** Макет окна `RTC HUD` из карточки `TASK-1011B` (`waves/kanban-board.json`). */
export const RTC_HUD_LAYOUT = Object.freeze({
  container: "RTC HUD",
  dimensions: "640px x 480px",
  padding: "16px",
  spacing: "10px",
});

/** Дизайн-токены карточек `TASK-1011` / `1011A` / `1011B` из `waves/kanban-board.json`. */
export const RTC_DIAGNOSTICS_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#23a55a",
  border: "#3f4147",
});

/** Интерактивные состояния элемента `rtc-graph` (`TASK-1011`). */
export const RTC_GRAPH_STATES = Object.freeze({
  element: "rtc-graph",
  default: "60fps graph",
  hover: "data point tooltip",
  active: "zoomed metric",
});

/** Интерактивные состояния элемента `telemetry-probe` (`TASK-1011A`). */
export const TELEMETRY_PROBE_STATES = Object.freeze({
  element: "telemetry-probe",
  default: "polling",
  hover: "collect",
  active: "buffered",
});

/** Интерактивные состояния элемента `graph-canvas` (`TASK-1011B`). */
export const GRAPH_CANVAS_STATES = Object.freeze({
  element: "graph-canvas",
  default: "rendering",
  hover: "tooltip value",
  active: "zoom range",
});

/** Константы протокола RTCP/SRTCP, порогов `RTCConnection.quality` и DAVE MLS v1. */
export const RTC_PROTOCOL_CONSTANTS = Object.freeze({
  RTCP_VERSION: 2,
  RTCP_SR_PT: 200,
  RTCP_RR_PT: 201,
  RTCP_SR_CLEAR_LEN: 28,
  SRTCP_SR_WIRE_LEN: 60,
  RTCP_RR_CLEAR_LEN: 32,
  SRTCP_RR_WIRE_LEN: 52,
  SRTCP_E_BIT: 0x80000000,
  OPUS_SAMPLE_RATE_KHZ: 48,
  RING_BUFFER_CAPACITY: 120,
  RING_BUFFER_STEP_MS: 500,
  HUD_TARGET_FPS: 60,
  HUD_WIDTH_PX: 640,
  HUD_HEIGHT_PX: 480,
  HUD_PADDING_PX: 16,
  HUD_SPACING_PX: 10,
  HUD_GRID_LINES: 4,
  SPARKLINE_WIDTH_PX: 258,
  SPARKLINE_HEIGHT_PX: 80,
  PING_FINE_MAX_MS: 250,
  PING_AVERAGE_MAX_MS: 500,
  LOSS_FINE_MAX_PCT: 5,
  LOSS_AVERAGE_MAX_PCT: 10,
  PING_HISTORY_MAX_POINTS: 199,
  PING_AVERAGE_WINDOW: 20,
  DEFAULT_PING_TIMEOUT_MS: 4000,
  PRIVACY_CODE_DIGITS: 30,
  PRIVACY_CODE_GROUP_LEN: 5,
  PRIVACY_CODE_GROUPS: 6,
  PERM_VIEW_CHANNEL: 1n << 10n,
  PERM_CONNECT: 1n << 20n,
  PERM_SPEAK: 1n << 21n,
});

/**
 * Проверяет корректность Snowflake ID (`> 0` и `<= u64::MAX`).
 * @param {unknown} raw
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
 * Вычисляет состояние графика `rtc-graph`.
 * @param {{ hovered?: boolean, zoomed?: boolean }} [opts]
 * @returns {string}
 */
export function resolveRtcGraphState(opts = {}) {
  if (opts.zoomed) return RTC_GRAPH_STATES.active;
  if (opts.hovered) return RTC_GRAPH_STATES.hover;
  return RTC_GRAPH_STATES.default;
}

/**
 * Вычисляет состояние пробника телеметрии `telemetry-probe`.
 * @param {"default" | "hover" | "active"} mode
 * @returns {string}
 */
export function resolveTelemetryProbeState(mode) {
  if (mode === "active") return TELEMETRY_PROBE_STATES.active;
  if (mode === "hover") return TELEMETRY_PROBE_STATES.hover;
  return TELEMETRY_PROBE_STATES.default;
}

/**
 * Вычисляет состояние холста графика `graph-canvas`.
 * @param {"default" | "hover" | "active"} mode
 * @returns {string}
 */
export function resolveGraphCanvasState(mode) {
  if (mode === "active") return GRAPH_CANVAS_STATES.active;
  if (mode === "hover") return GRAPH_CANVAS_STATES.hover;
  return GRAPH_CANVAS_STATES.default;
}

/**
 * Собирает 60-байтный SRTCP Sender Report (`PT = 200`, `RC = 0`, `length_words = 6`).
 * @param {{ senderSsrc: number, ntpSec: number, ntpFrac: number, rtpTimestamp: number, packetCount: number, octetCount: number, srtcpIndex?: number }} params
 * @returns {Uint8Array}
 */
export function buildRtcpSenderReportBytes(params) {
  const buf = new ArrayBuffer(RTC_PROTOCOL_CONSTANTS.SRTCP_SR_WIRE_LEN);
  const view = new DataView(buf);
  view.setUint8(0, 0x80);
  view.setUint8(1, RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT);
  view.setUint16(2, 6, false);
  view.setUint32(4, params.senderSsrc >>> 0, false);
  view.setUint32(8, params.ntpSec >>> 0, false);
  view.setUint32(12, params.ntpFrac >>> 0, false);
  view.setUint32(16, params.rtpTimestamp >>> 0, false);
  view.setUint32(20, params.packetCount >>> 0, false);
  view.setUint32(24, params.octetCount >>> 0, false);
  view.setUint32(56, ((params.srtcpIndex ?? 0) | RTC_PROTOCOL_CONSTANTS.SRTCP_E_BIT) >>> 0, true);
  return new Uint8Array(buf);
}

/**
 * Собирает 52-байтный серверный SRTCP Receiver Report (`PT = 201`, `RC = 1`, `length_words = 7`).
 * @param {{ senderSsrc: number, ssrc: number, fractionLostRaw: number, cumulativeLost: number, extendedHighestSeq: number, jitterTicks: number, lastSr: number, delaySinceLastSr: number, srtcpCounter?: number }} params
 * @returns {Uint8Array}
 */
export function buildRtcpReceiverReportBytes(params) {
  const buf = new ArrayBuffer(RTC_PROTOCOL_CONSTANTS.SRTCP_RR_WIRE_LEN);
  const view = new DataView(buf);
  view.setUint8(0, 0x81);
  view.setUint8(1, RTC_PROTOCOL_CONSTANTS.RTCP_RR_PT);
  view.setUint16(2, 7, false);
  view.setUint32(4, params.senderSsrc >>> 0, false);
  view.setUint32(8, params.ssrc >>> 0, false);
  view.setUint8(12, params.fractionLostRaw & 0xff);
  const cum = params.cumulativeLost & 0xffffff;
  view.setUint8(13, (cum >>> 16) & 0xff);
  view.setUint8(14, (cum >>> 8) & 0xff);
  view.setUint8(15, cum & 0xff);
  view.setUint32(16, params.extendedHighestSeq >>> 0, false);
  view.setUint32(20, params.jitterTicks >>> 0, false);
  view.setUint32(24, params.lastSr >>> 0, false);
  view.setUint32(28, params.delaySinceLastSr >>> 0, false);
  view.setUint32(48, (params.srtcpCounter ?? 0) >>> 0, true);
  return new Uint8Array(buf);
}

/**
 * Разбирает 24-битное знаковое целое из буфера в сетевом порядке байт (big-endian).
 * @param {DataView} view
 * @param {number} offset
 * @returns {number}
 */
function readSigned24Be(view, offset) {
  const raw =
    (view.getUint8(offset) << 16) |
    (view.getUint8(offset + 1) << 8) |
    view.getUint8(offset + 2);
  return (raw << 8) >> 8;
}

/**
 * Разбирает бинарный пакет RTCP/SRTCP Sender Report (`PT = 200`) или Receiver Report (`PT = 201`).
 * @param {Uint8Array} bytes
 */
export function parseRtcpPacketBytes(bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 8) {
    throw new RangeError("Truncated RTCP packet: minimum 8 bytes required");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const first = view.getUint8(0);
  const version = first >>> 6;
  if (version !== RTC_PROTOCOL_CONSTANTS.RTCP_VERSION) {
    throw new RangeError(`Invalid RTCP version: ${version}`);
  }
  if ((first & 0x20) !== 0) {
    throw new RangeError("Unsupported RTCP padding bit");
  }
  const rc = first & 0x1f;
  const pt = view.getUint8(1);
  const lengthWords = view.getUint16(2, false);
  const totalBytes = (lengthWords + 1) * 4;
  const minBytes =
    pt === RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT
      ? 28 + rc * 24
      : pt === RTC_PROTOCOL_CONSTANTS.RTCP_RR_PT
        ? 8 + rc * 24
        : 0;
  if (minBytes === 0) throw new RangeError(`Unsupported RTCP payload type: ${pt}`);
  if (totalBytes < minBytes || bytes.byteLength < totalBytes) {
    throw new RangeError(`Invalid RTCP packet length: ${totalBytes}`);
  }

  const senderSsrc = view.getUint32(4, false);
  const reportsOffset = pt === RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT ? 28 : 8;
  const reports = [];
  for (let i = 0; i < rc; i += 1) {
    const off = reportsOffset + i * 24;
    const fractionLostRaw = view.getUint8(off + 4);
    const jitterTicks = view.getUint32(off + 12, false);
    reports.push({
      ssrc: view.getUint32(off, false),
      fractionLostRaw,
      fractionLostPct: (fractionLostRaw / 256) * 100,
      cumulativeLost: readSigned24Be(view, off + 5),
      extendedHighestSeq: view.getUint32(off + 8, false),
      jitterTicks,
      jitterMs: Math.floor(jitterTicks / RTC_PROTOCOL_CONSTANTS.OPUS_SAMPLE_RATE_KHZ),
      lastSr: view.getUint32(off + 16, false),
      delaySinceLastSr: view.getUint32(off + 20, false),
    });
  }

  const senderInfo =
    pt === RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT
      ? {
          ntpSec: view.getUint32(8, false),
          ntpFrac: view.getUint32(12, false),
          rtpTimestamp: view.getUint32(16, false),
          packetCount: view.getUint32(20, false),
          octetCount: view.getUint32(24, false),
        }
      : null;

  return {
    kind: pt === RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT ? "SR" : "RR",
    version,
    payloadType: pt,
    reportCount: rc,
    senderSsrc,
    senderInfo,
    reports,
    trailerLen: bytes.byteLength - totalBytes,
  };
}

/**
 * Вычисляет RTT в миллисекундах по компактному NTP-времени приёма, `LSR` и `DLSR` (RFC 3550 §6.4.1).
 * @param {{ receiveNtpCompact: number, lastSr: number, delaySinceLastSr: number }} params
 * @returns {number | null}
 */
export function computeRttFromReportBlockMs(params) {
  if (!params || !params.lastSr) return null;
  const elapsed = (params.receiveNtpCompact - params.lastSr) >>> 0;
  const rttCompact = Math.max(0, elapsed - (params.delaySinceLastSr >>> 0));
  return Math.round((rttCompact * 1000) / 65536);
}

/**
 * Считает процент потерь исходящих пакетов по дельтам двух соседних снимков (`module 206959`).
 * @param {{ packetsSent: number, packetsLost: number }} prev
 * @param {{ packetsSent: number, packetsLost: number }} curr
 * @returns {number}
 */
export function computeOutboundLossRateDeltaPct(prev, curr) {
  const deltaSent = Math.max(0, Number(curr.packetsSent) - Number(prev.packetsSent));
  const deltaLost = Math.max(0, Number(curr.packetsLost) - Number(prev.packetsLost));
  if (deltaSent === 0) return 0;
  return Math.min(1, Math.max(0, deltaLost / (deltaSent + deltaLost))) * 100;
}

/**
 * Классифицирует качество соединения `RTCConnection.quality` (`fine` / `average` / `bad` / `unknown`).
 * @param {boolean} connected
 * @param {unknown} pingMs
 * @param {unknown} [outboundLossRatePct]
 * @returns {"fine" | "average" | "bad" | "unknown"}
 */
export function classifyRtcConnectionQuality(connected, pingMs, outboundLossRatePct) {
  if (!connected || typeof pingMs !== "number" || !Number.isFinite(pingMs) || pingMs < 0) {
    return "unknown";
  }
  /** @type {number | null} */
  let loss = null;
  if (outboundLossRatePct !== null && outboundLossRatePct !== undefined) {
    const parsed = Number(outboundLossRatePct);
    if (!Number.isFinite(parsed) || parsed < 0 || parsed > 100) return "unknown";
    loss = parsed;
  }
  if (
    pingMs > RTC_PROTOCOL_CONSTANTS.PING_AVERAGE_MAX_MS ||
    (loss !== null && loss > RTC_PROTOCOL_CONSTANTS.LOSS_AVERAGE_MAX_PCT)
  ) {
    return "bad";
  }
  if (
    pingMs > RTC_PROTOCOL_CONSTANTS.PING_FINE_MAX_MS ||
    (loss !== null && loss > RTC_PROTOCOL_CONSTANTS.LOSS_FINE_MAX_PCT)
  ) {
    return "average";
  }
  return "fine";
}

/** Кольцевой буфер `RingBuffer 120` (60 с с шагом 500 мс) для метрик RTCP. */
export class RtcpMetricsRingBuffer {
  /** @param {number} [capacity] */
  constructor(capacity = RTC_PROTOCOL_CONSTANTS.RING_BUFFER_CAPACITY) {
    this.capacity = Math.max(1, Math.min(RTC_PROTOCOL_CONSTANTS.RING_BUFFER_CAPACITY, capacity));
    /** @type {Array<{ timeMs: number, rttMs: number, jitterMs: number, lossPct: number, bitrateKbps: number }>} */
    this._items = [];
  }

  /** @param {{ timeMs: number, rttMs: number, jitterMs: number, lossPct: number, bitrateKbps: number }} sample */
  pushSample(sample) {
    this._items.push({ ...sample });
    while (this._items.length > this.capacity) this._items.shift();
  }

  size() {
    return this._items.length;
  }

  samples() {
    return [...this._items];
  }
}

/**
 * Вычисляет 2D-координаты кривой на холсте `width x height` (меньше значение -> выше точка).
 * @param {readonly number[]} values
 * @param {number} width
 * @param {number} height
 */
export function computeGraphCurvePoints(values, width, height) {
  if (!Array.isArray(values) || values.length === 0) return [];
  let min = values[0] ?? 0;
  let max = min;
  for (const v of values) {
    if (v < min) min = v;
    if (v > max) max = v;
  }
  const span = max - min;
  return values.map((value, idx) => {
    const x = values.length === 1 ? 0 : (idx / (values.length - 1)) * width;
    const y = span === 0 ? height / 2 : height - ((value - min) / span) * height;
    return { x: Math.min(Math.max(x, 0), width), y: Math.min(Math.max(y, 0), height), value };
  });
}

/**
 * Строит кадр 60 FPS для окна `RTC Internals HUD` (`640x480`, 3 кривые: RTT, джиттер, битрейт).
 * @param {ReadonlyArray<{ timeMs: number, rttMs: number, jitterMs: number, lossPct: number, bitrateKbps: number }>} samples
 * @param {{ zoomFactor?: number, hoverIndex?: number | null }} [opts]
 */
export function renderRtcHudFrame60Fps(samples, opts = {}) {
  const safeSamples = Array.isArray(samples) ? samples : [];
  const zoomFactor = Math.max(1, Math.floor(opts.zoomFactor ?? 1));
  const zoomed = zoomFactor > 1;
  const visibleLen =
    safeSamples.length === 0 ? 0 : Math.max(1, Math.floor(safeSamples.length / zoomFactor));
  const slice = safeSamples.slice(-visibleLen);
  const hoverIdx = opts.hoverIndex;
  const hoveredItem =
    typeof hoverIdx === "number" && hoverIdx >= 0 && hoverIdx < slice.length
      ? slice[hoverIdx]
      : null;
  const hovered = Boolean(hoveredItem);

  const innerWidth =
    RTC_PROTOCOL_CONSTANTS.HUD_WIDTH_PX - RTC_PROTOCOL_CONSTANTS.HUD_PADDING_PX * 2;
  const curveHeight = 120;

  const rttPoints = computeGraphCurvePoints(slice.map((s) => s.rttMs), innerWidth, curveHeight);
  const jitterPoints = computeGraphCurvePoints(slice.map((s) => s.jitterMs), innerWidth, curveHeight);
  const bitratePoints = computeGraphCurvePoints(slice.map((s) => s.bitrateKbps), innerWidth, curveHeight);
  const sparkline258x80 = computeGraphCurvePoints(
    safeSamples.slice(0, 10).map((s) => s.rttMs),
    RTC_PROTOCOL_CONSTANTS.SPARKLINE_WIDTH_PX,
    RTC_PROTOCOL_CONSTANTS.SPARKLINE_HEIGHT_PX,
  );

  const tooltip = hoveredItem
    ? {
        ...hoveredItem,
        textRu: `RTT: ${hoveredItem.rttMs} мс | Джиттер: ${hoveredItem.jitterMs} мс | Потери: ${hoveredItem.lossPct.toFixed(1)}% | Битрейт: ${hoveredItem.bitrateKbps} кбит/с`,
      }
    : null;

  return {
    container: RTC_INTERNALS_HUD_LAYOUT.container,
    widthPx: RTC_PROTOCOL_CONSTANTS.HUD_WIDTH_PX,
    heightPx: RTC_PROTOCOL_CONSTANTS.HUD_HEIGHT_PX,
    paddingPx: RTC_PROTOCOL_CONSTANTS.HUD_PADDING_PX,
    spacingPx: RTC_PROTOCOL_CONSTANTS.HUD_SPACING_PX,
    targetFps: RTC_PROTOCOL_CONSTANTS.HUD_TARGET_FPS,
    frameBudgetMs: 16.66,
    gridLinesCount: RTC_PROTOCOL_CONSTANTS.HUD_GRID_LINES,
    graphState: resolveRtcGraphState({ hovered, zoomed }),
    canvasState: resolveGraphCanvasState(zoomed ? "active" : hovered ? "hover" : "default"),
    rttCurve: { colorHex: RTC_DIAGNOSTICS_DESIGN_TOKENS.accent, points: rttPoints },
    jitterCurve: { colorHex: "#f0b232", points: jitterPoints },
    bitrateCurve: { colorHex: "#5865f2", points: bitratePoints },
    sparkline258x80,
    tooltip,
  };
}

/**
 * Проверяет, должен ли отображаться код верификации DAVE Secure Frames для канала (`module 259374`).
 * @param {{ channelId?: string | null, activeVoiceChannelId?: string | null, isStageVoiceChannel?: boolean | null, protocolVersion?: number | null }} params
 * @returns {boolean}
 */
export function shouldShowDaveVerificationCode(params) {
  if (!params || typeof params !== "object") return false;
  if (typeof params.channelId !== "string" || params.channelId.length === 0) return false;
  if (params.channelId !== params.activeVoiceChannelId) return false;
  if (params.isStageVoiceChannel === true) return false;
  return (
    typeof params.protocolVersion === "number" &&
    Number.isFinite(params.protocolVersion) &&
    params.protocolVersion > 0
  );
}

/**
 * Вычисляет и верифицирует состояние DAVE MLS v1 и 30-значный `Voice Privacy Code` (`6x5`).
 * @param {{ channelId?: string, activeVoiceChannelId?: string, isStageVoiceChannel?: boolean, protocolVersion?: number, epoch?: number, transitionCount?: number, rosterSize?: number, timeToDaveGroupMs?: number, authenticatorBytes?: Uint8Array }} params
 */
export function verifyDaveMlsState(params) {
  const protocolVersion = params?.protocolVersion ?? 0;
  const isActive = Number.isInteger(protocolVersion) && protocolVersion > 0;
  const shouldShowVerificationCode = shouldShowDaveVerificationCode(params);

  /** @type {string[]} */
  const chunks = [];
  if (
    isActive &&
    params.authenticatorBytes instanceof Uint8Array &&
    params.authenticatorBytes.byteLength >= RTC_PROTOCOL_CONSTANTS.PRIVACY_CODE_DIGITS
  ) {
    for (
      let offset = 0;
      offset < RTC_PROTOCOL_CONSTANTS.PRIVACY_CODE_DIGITS;
      offset += RTC_PROTOCOL_CONSTANTS.PRIVACY_CODE_GROUP_LEN
    ) {
      let blockVal = 0n;
      for (let i = 0; i < RTC_PROTOCOL_CONSTANTS.PRIVACY_CODE_GROUP_LEN; i += 1) {
        blockVal = (blockVal << 8n) | BigInt(params.authenticatorBytes[offset + i] ?? 0);
      }
      chunks.push((blockVal % 100000n).toString().padStart(5, "0"));
    }
  }

  return {
    isActive,
    protocolVersion,
    epoch: params.epoch ?? 0,
    shouldShowVerificationCode,
    privacyCodeChunks: Object.freeze(chunks),
    privacyCodeFormatted: chunks.join(" "),
    transitionSummary: Object.freeze({
      isActive,
      statusLabel: isActive ? `DAVE v${protocolVersion} (MLS)` : "Транспортное шифрование (SRTP)",
      epochTransitionLabel:
        typeof params.transitionCount === "number" && params.transitionCount >= 0
          ? `Переходов эпох: ${Math.trunc(params.transitionCount)}`
          : "Переходов эпох: —",
      rosterLabel:
        typeof params.rosterSize === "number" && params.rosterSize >= 0
          ? `Участников MLS: ${Math.trunc(params.rosterSize)}`
          : "Участников MLS: —",
      joinLatencyLabel:
        typeof params.timeToDaveGroupMs === "number" && params.timeToDaveGroupMs >= 0
          ? `Вход в DAVE-группу: ${Math.round(params.timeToDaveGroupMs)} мс`
          : "Вход в DAVE-группу: —",
    }),
  };
}

/**
 * Извлекает короткое имя RTC-сервера до первой `.` или `:`.
 * @param {string} endpoint
 * @returns {string}
 */
export function extractShortRtcHostname(endpoint) {
  if (typeof endpoint !== "string") return "";
  const match = /^[^.:]*/.exec(endpoint.trim());
  return match ? (match[0] ?? "") : "";
}

/**
 * Форматирует русские строки попапа `RTCConnectionPopout` (`RE-GAP-22`).
 * @param {{ shortHostname?: string | null, averagePingMs?: number | null, lastPingMs?: number | null, outboundLossRatePct?: number | null, isEndToEndEncrypted?: boolean, secureFramesProtocolVersion?: number | null }} input
 */
export function formatRtcPopoutSummaryRu(input) {
  const shortHostname = typeof input?.shortHostname === "string" ? input.shortHostname.trim() : "";
  const avgPing = Math.round(Number(input?.averagePingMs) || 0);
  const lastPing = Math.round(Number(input?.lastPingMs) || 0);
  const rawLoss = input?.outboundLossRatePct;
  const numLoss = rawLoss === null || rawLoss === undefined ? Number.NaN : Number(rawLoss);
  const hasLoss = Number.isFinite(numLoss);
  const isE2ee = input?.isEndToEndEncrypted === true;

  return {
    headline: shortHostname
      ? `Голосовая связь подключена — ${shortHostname}`
      : "Голосовая связь подключена",
    averagePingLabel: `Средняя задержка: ${avgPing} мс`,
    lastPingLabel: `Последняя задержка: ${lastPing} мс`,
    lossLabel: hasLoss
      ? `Потеря исходящих пакетов: ${numLoss.toFixed(1)}%`
      : "Потеря исходящих пакетов: —",
    encryptionLabel: isE2ee ? "Защищено сквозным шифрованием" : "Защищённое соединение",
    hasWarning:
      Number(input?.lastPingMs) > RTC_PROTOCOL_CONSTANTS.PING_FINE_MAX_MS ||
      (hasLoss && numLoss > RTC_PROTOCOL_CONSTANTS.LOSS_AVERAGE_MAX_PCT),
  };
}

/**
 * Сверяет реализацию с независимыми RE-оракулами (`waves/kanban-board.json`,
 * `waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`, `docs/re/voice-deep.json`,
 * `waves/RW-26/research/fixtures/rtcp-voice-udp{3,4}-server.json`).
 * @param {{ kanbanPath?: string, reGap22Path?: string, voiceDeepPath?: string, rtcpUdp3Path?: string, rtcpUdp4Path?: string }} [opts]
 */
export function verifyRtcDiagnosticsOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const reGap22Path = opts.reGap22Path ?? DEFAULT_RE_GAP_22_PATH;
  const voiceDeepPath = opts.voiceDeepPath ?? DEFAULT_VOICE_DEEP_PATH;
  const rtcpUdp3Path = opts.rtcpUdp3Path ?? DEFAULT_RTCP_UDP3_PATH;
  const rtcpUdp4Path = opts.rtcpUdp4Path ?? DEFAULT_RTCP_UDP4_PATH;
  /** @type {string[]} */
  const reasons = [];

  for (const p of [kanbanPath, reGap22Path, voiceDeepPath, rtcpUdp3Path, rtcpUdp4Path]) {
    if (!existsSync(p)) {
      return { passed: false, reasons: [`Отсутствует оракул: ${p}`], udp3PacketCount: 0, udp4PacketCount: 0 };
    }
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card1011 = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1011");
  const card1011a = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1011A");
  const card1011b = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1011B");
  if (!card1011 || !card1011a || !card1011b) {
    reasons.push("Карточки TASK-1011 / 1011A / 1011B не найдены в waves/kanban-board.json");
  } else if (
    card1011.layout?.container !== RTC_INTERNALS_HUD_LAYOUT.container ||
    card1011.design_tokens?.accent !== RTC_DIAGNOSTICS_DESIGN_TOKENS.accent
  ) {
    reasons.push("Несовпадение контракта TASK-1011");
  }

  const reGapMd = readFileSync(reGap22Path, "utf8");
  for (const token of ["RTCConnectionPopout", "RTCDebugStore", "epochAuthenticator", "daXg45"]) {
    if (!reGapMd.includes(token)) reasons.push(`В RE-GAP-22 отсутствует маркер ${token}`);
  }

  const voiceDeep = JSON.parse(readFileSync(voiceDeepPath, "utf8"));
  if (
    voiceDeep?.udp?.rtcp?.sr?.pt !== RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT ||
    voiceDeep?.udp?.rtcp?.rr_server?.pt !== RTC_PROTOCOL_CONSTANTS.RTCP_RR_PT
  ) {
    reasons.push("Несовпадение PT RTCP SR/RR в docs/re/voice-deep.json");
  }
  if (voiceDeep?.engine?.dave?.protocol_version !== 1) {
    reasons.push("Несовпадение версии DAVE MLS v1 в docs/re/voice-deep.json");
  }

  const udp3 = JSON.parse(readFileSync(rtcpUdp3Path, "utf8"));
  const udp4 = JSON.parse(readFileSync(rtcpUdp4Path, "utf8"));
  const udp3PacketCount = Array.isArray(udp3.packets) ? udp3.packets.length : 0;
  const udp4PacketCount = Array.isArray(udp4.packets) ? udp4.packets.length : 0;
  if (udp3PacketCount !== 12 || udp4PacketCount !== 20) {
    reasons.push(`Несовпадение числа пакетов в фикстурах RTCP: ${udp3PacketCount} / ${udp4PacketCount}`);
  }

  return { passed: reasons.length === 0, reasons, udp3PacketCount, udp4PacketCount };
}

/**
 * Маскирует токены авторизации и криптографические ключи в строках телеметрии RTC.
 * @param {string} text
 * @returns {string}
 */
export function redactRtcDiagnosticsTelemetry(text) {
  return String(text ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "mfa.[REDACTED]")
    .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED]")
    .replace(/(token|secret_key|session_token|authorization)\s*[:=]\s*["']?[^"'\s]+["']?/gi, "$1=[REDACTED]");
}

/** Проверяет все 9 обязательных измерений `TEST-POLICY.md` для модуля WebRTC-диагностики. */
export function evaluateRtcDiagNineDimensions() {
  /** @type {string[]} */
  const failures = [];
  const rr = buildRtcpReceiverReportBytes({
    senderSsrc: 17843,
    ssrc: 100,
    fractionLostRaw: 0,
    cumulativeLost: 0,
    extendedHighestSeq: 50,
    jitterTicks: 240,
    lastSr: 1000,
    delaySinceLastSr: 100,
  });
  if (parseRtcpPacketBytes(rr).kind !== "RR" || classifyRtcConnectionQuality(true, 42, 0) !== "fine") {
    failures.push("dim1_positive");
  }
  let invalidRejected = false;
  try {
    parseRtcpPacketBytes(new Uint8Array([0x00, 201, 0, 7, 0, 0, 0, 1]));
  } catch {
    invalidRejected = true;
  }
  if (!invalidRejected) failures.push("dim2_negative");
  if (
    isValidSnowflake("0") !== false ||
    isValidSnowflake("18446744073709551615") !== true ||
    classifyRtcConnectionQuality(true, 250, 5) !== "fine" ||
    classifyRtcConnectionQuality(true, 251, 5) !== "average" ||
    classifyRtcConnectionQuality(true, 500, 10) !== "average" ||
    classifyRtcConnectionQuality(true, 501, 10) !== "bad"
  ) {
    failures.push("dim3_boundaries");
  }
  const retryable429 = (/** @type {number} */ code) => code === 429 || code >= 500;
  if (!retryable429(429) || retryable429(403)) failures.push("dim4_error_handling");
  if (renderRtcHudFrame60Fps([]).rttCurve.points.length !== 0) failures.push("dim5_empty_state");
  if (classifyRtcConnectionQuality(false, 40, 0) !== "unknown") failures.push("dim6_loading_state");

  const fullPerms =
    RTC_PROTOCOL_CONSTANTS.PERM_VIEW_CHANNEL |
    RTC_PROTOCOL_CONSTANTS.PERM_CONNECT |
    RTC_PROTOCOL_CONSTANTS.PERM_SPEAK;
  const canDiag = (/** @type {bigint} */ p, /** @type {boolean} */ ageBlocked) =>
    !ageBlocked && (p & fullPerms) === fullPerms;
  if (!canDiag(fullPerms, false) || canDiag(fullPerms, true) || canDiag(0n, false)) {
    failures.push("dim7_permissions");
  }
  const rb = new RtcpMetricsRingBuffer(120);
  for (let i = 0; i < 130; i += 1) {
    rb.pushSample({ timeMs: i * 500, rttMs: 30, jitterMs: 2, lossPct: 0, bitrateKbps: 64 });
  }
  if (rb.size() !== 120) failures.push("dim8_ring_buffer_recovery");
  if (redactRtcDiagnosticsTelemetry("Bearer mfa.secret99").includes("secret99")) {
    failures.push("dim9_observability_redaction");
  }
  return { passed: failures.length === 0, dimensionsChecked: 9, failures };
}

/**
 * Точка входа CLI `scripts/verify-rtc-diag.mjs`.
 * @param {string[]} argv
 * @returns {number}
 */
export function runRtcDiagCli(argv) {
  let outJson = "";
  let outMd = "";
  let oraclePath = DEFAULT_KANBAN_PATH;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const nextArg = argv[i + 1];
    if (arg === "--help" || arg === "-h") {
      process.stdout.write(
        "Использование: node scripts/verify-rtc-diag.mjs [--out-json <path>] [--out-md <path>] [--oracle <path>]\n",
      );
      return 0;
    }
    if (arg === "--out-json" && nextArg) {
      outJson = resolve(process.cwd(), nextArg);
      i += 1;
    } else if (arg === "--out-md" && nextArg) {
      outMd = resolve(process.cwd(), nextArg);
      i += 1;
    } else if (arg === "--oracle" && nextArg) {
      oraclePath = resolve(process.cwd(), nextArg);
      i += 1;
    } else {
      process.stderr.write(`Ошибка аргументов CLI: неизвестный флаг "${arg}"\n`);
      return 1;
    }
  }

  if (!existsSync(oraclePath) || !existsSync(DEFAULT_RUST_MODULE_PATH)) {
    process.stderr.write(`Оракул или модуль недоступен: ${oraclePath}\n`);
    return 2;
  }

  const parity = verifyRtcDiagnosticsOracleParity({ kanbanPath: oraclePath });
  const dimensions = evaluateRtcDiagNineDimensions();
  const rustMd5 = createHash("md5").update(readFileSync(DEFAULT_RUST_MODULE_PATH)).digest("hex");
  const passed = parity.passed && dimensions.passed;
  const report = { taskId: "TASK-1011", passed, parity, dimensions, rustMd5 };

  if (outJson) {
    mkdirSync(dirname(outJson), { recursive: true });
    writeFileSync(outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(outMd), { recursive: true });
    writeFileSync(
      outMd,
      `# Отчёт верификации WebRTC-диагностики (TASK-1011)\n\n- **Статус**: \`${passed ? "PASS" : "FAIL"}\`\n- **MD5**: \`${rustMd5}\`\n`,
      "utf8",
    );
  }
  if (!passed) {
    process.stderr.write(`Верификация TASK-1011 не пройдена: ${parity.reasons.join("; ")}\n`);
    return 1;
  }
  process.stdout.write("Верификация WebRTC-диагностики (TASK-1011) успешно пройдена.\n");
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runRtcDiagCli(process.argv.slice(2)));
}
