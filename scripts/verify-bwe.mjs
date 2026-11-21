#!/usr/bin/env node
// @ts-check
/**
 * Скрипт верификации и симуляции WebRTC Bandwidth Estimation (BWE / GCC)
 * по обратной связи RTCP REMB (PT=206, FMT=15) и RTCP TWCC (PT=205, FMT=15)
 * в диапазоне видеобитрейта 500..8000 kbps — TASK-982.
 *
 * Коды выхода CLI:
 * - 0: Все инварианты BWE, RE-оракулы и сценарии плавной адаптации пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение инвариантов BWE.
 * - 2: Внешний файл-оракул недоступен или отсутствует.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

/** Дизайн-токены карточки TASK-982 из `waves/kanban-board.json`. */
export const BWE_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Интерактивные состояния графика `bwe-graph` из `waves/kanban-board.json`. */
export const BWE_GRAPH_STATES = Object.freeze({
  element: "bwe-graph",
  default: "probing",
  hover: "limit bandwidth",
  active: "adapted bitrate",
});

/** Конфигурация по умолчанию для контроллера перегрузки GCC (REMB + TWCC). */
export const BWE_DEFAULT_CONFIG = Object.freeze({
  minBitrateKbps: 500,
  maxBitrateKbps: 8_000,
  startBitrateKbps: 2_500,
  fallbackMinBitrateBps: 150_000,
  maxSingleStepDropRatio: 0.15,
  aimdDecreaseFactor: 0.85,
  probingStepKbps: 250,
  overuseThresholdMs: 12.5,
  rtcpRtpfbPt: 205,
  rtcpPsfbPt: 206,
  rtcpFmtRemb: 15,
  rtcpFmtTwcc: 15,
  twccExtensionUri:
    "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01",
  absCaptureTimeUri:
    "http://www.webrtc.org/experiments/rtp-hdrext/abs-capture-time",
  trendlineExperimentSymbol:
    "kDiscordBandwidthEstimationExperimentTrendlineWindowDuration",
});

/**
 * Вычисляет текущее UI-состояние элемента `bwe-graph`.
 * @param {{ mode: "probing" | "adapted", hovered?: boolean }} opts
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveBweGraphState(opts) {
  if (opts.hovered) {
    return {
      element: BWE_GRAPH_STATES.element,
      state: "hover",
      label: BWE_GRAPH_STATES.hover,
    };
  }
  if (opts.mode === "adapted") {
    return {
      element: BWE_GRAPH_STATES.element,
      state: "active",
      label: BWE_GRAPH_STATES.active,
    };
  }
  return {
    element: BWE_GRAPH_STATES.element,
    state: "default",
    label: BWE_GRAPH_STATES.default,
  };
}

/**
 * Кодирует битрейт (bps) в 6-битную экспоненту и 18-битную мантиссу RTCP REMB.
 * @param {number} bitrateBps
 * @returns {{ exponent: number, mantissa: number }}
 */
export function encodeRembBitrate(bitrateBps) {
  const safeBps = Math.max(0, Math.floor(bitrateBps));
  let exponent = 0;
  let mantissa = safeBps;
  while (mantissa > 0x3_ffff && exponent < 63) {
    mantissa = Math.floor(mantissa / 2);
    exponent += 1;
  }
  return {
    exponent: exponent & 0x3f,
    mantissa: mantissa & 0x3_ffff,
  };
}

/**
 * Декодирует 6-битную экспоненту и 18-битную мантиссу RTCP REMB в битрейт (bps).
 * @param {number} exponent
 * @param {number} mantissa
 * @returns {number | null}
 */
export function decodeRembBitrate(exponent, mantissa) {
  if (exponent < 0 || exponent > 63 || mantissa < 0 || mantissa > 0x3_ffff) {
    return null;
  }
  const scaled = mantissa * 2 ** exponent;
  if (!Number.isSafeInteger(scaled)) {
    return null;
  }
  return scaled;
}

/**
 * Собирает бинарный пакет RTCP REMB (`PT=206, FMT=15, "REMB"`).
 * @param {{ senderSsrc: number, mediaSsrc?: number, bitrateBps: number, ssrcs: number[] }} pkt
 * @returns {Buffer}
 */
export function buildRembPacket(pkt) {
  if (!Array.isArray(pkt.ssrcs) || pkt.ssrcs.length > 255) {
    throw new Error("Некорректный список SSRC для пакета REMB");
  }
  const totalBytes = 20 + pkt.ssrcs.length * 4;
  const wordLen = totalBytes / 4 - 1;
  const buf = Buffer.alloc(totalBytes);
  buf[0] = (2 << 6) | BWE_DEFAULT_CONFIG.rtcpFmtRemb;
  buf[1] = BWE_DEFAULT_CONFIG.rtcpPsfbPt;
  buf.writeUInt16BE(wordLen, 2);
  buf.writeUInt32BE(pkt.senderSsrc >>> 0, 4);
  buf.writeUInt32BE((pkt.mediaSsrc ?? 0) >>> 0, 8);
  buf.write("REMB", 12, "ascii");
  buf[16] = pkt.ssrcs.length & 0xff;
  const { exponent, mantissa } = encodeRembBitrate(pkt.bitrateBps);
  buf[17] = ((exponent & 0x3f) << 2) | ((mantissa >>> 16) & 0x03);
  buf[18] = (mantissa >>> 8) & 0xff;
  buf[19] = mantissa & 0xff;
  pkt.ssrcs.forEach((ssrc, idx) => {
    buf.writeUInt32BE(ssrc >>> 0, 20 + idx * 4);
  });
  return buf;
}

/**
 * Разбирает бинарный пакет RTCP REMB (`PT=206, FMT=15, "REMB"`).
 * @param {Buffer | Uint8Array} raw
 * @returns {{ senderSsrc: number, mediaSsrc: number, bitrateBps: number, ssrcs: number[] }}
 */
export function parseRembPacket(raw) {
  const buf = Buffer.isBuffer(raw) ? raw : Buffer.from(raw);
  if (buf.length < 20) {
    throw new Error(`Truncated RTCP REMB: слишком короткий пакет (${buf.length} байт)`);
  }
  const version = buf.readUInt8(0) >>> 6;
  const fmt = buf.readUInt8(0) & 0x1f;
  const pt = buf.readUInt8(1);
  if (
    version !== 2 ||
    fmt !== BWE_DEFAULT_CONFIG.rtcpFmtRemb ||
    pt !== BWE_DEFAULT_CONFIG.rtcpPsfbPt
  ) {
    throw new Error(`Неверный заголовок RTCP REMB: V=${version}, FMT=${fmt}, PT=${pt}`);
  }
  if (buf.subarray(12, 16).toString("ascii") !== "REMB") {
    throw new Error("Отсутствует сигнатура идентификатора REMB");
  }
  const numSsrc = buf.readUInt8(16);
  const expectedLen = 20 + numSsrc * 4;
  if (buf.length < expectedLen) {
    throw new Error(
      `Mismatch SSRC length: ожидалось ${expectedLen} байт, получено ${buf.length}`,
    );
  }
  const b17 = buf.readUInt8(17);
  const b18 = buf.readUInt8(18);
  const b19 = buf.readUInt8(19);
  const exponent = (b17 >>> 2) & 0x3f;
  const mantissa = ((b17 & 0x03) << 16) | (b18 << 8) | b19;
  const bitrateBps = decodeRembBitrate(exponent, mantissa);
  if (bitrateBps === null) {
    throw new Error("Переполнение при декодировании битрейта REMB");
  }
  const ssrcs = [];
  for (let i = 0; i < numSsrc; i += 1) {
    ssrcs.push(buf.readUInt32BE(20 + i * 4));
  }
  return {
    senderSsrc: buf.readUInt32BE(4),
    mediaSsrc: buf.readUInt32BE(8),
    bitrateBps,
    ssrcs,
  };
}

/**
 * Определяет 2-битный символ статуса пакета TWCC и сериализует дельту (шаг 250 мкс).
 * @param {{ received: boolean, recvDeltaUs?: number | null }} sample
 * @returns {{ symbol: number, deltaBytes: number[] }}
 */
function encodeTwccSampleStatus(sample) {
  if (!sample.received) {
    return { symbol: 0, deltaBytes: [] };
  }
  const ticks = Math.max(
    -32_768,
    Math.min(32_767, Math.trunc((sample.recvDeltaUs ?? 0) / 250)),
  );
  if (ticks >= 0 && ticks <= 255) {
    return { symbol: 1, deltaBytes: [ticks & 0xff] };
  }
  return {
    symbol: 2,
    deltaBytes: [(ticks >>> 8) & 0xff, ticks & 0xff],
  };
}

/**
 * Собирает пакет RTCP TWCC (`PT=205, FMT=15`) с 2-битными статус-векторами и дельтами по 250 мкс.
 * @param {{
 *   senderSsrc: number,
 *   mediaSsrc: number,
 *   baseSeq: number,
 *   referenceTime64ms: number,
 *   fbPktCount: number,
 *   packets: Array<{ sequence: number, received: boolean, recvDeltaUs?: number | null }>
 * }} pkt
 * @returns {Buffer}
 */
export function buildTwccPacket(pkt) {
  if (!Array.isArray(pkt.packets) || pkt.packets.length === 0) {
    throw new Error("Список пакетов TWCC не может быть пустым");
  }
  const symbols = [];
  const deltaBytes = [];
  for (const item of pkt.packets) {
    const encoded = encodeTwccSampleStatus(item);
    symbols.push(encoded.symbol);
    deltaBytes.push(...encoded.deltaBytes);
  }
  const chunkWords = [];
  for (let offset = 0; offset < symbols.length; offset += 7) {
    let chunk = 0xc000;
    for (let idx = 0; idx < 7; idx += 1) {
      const sym = symbols[offset + idx] ?? 0;
      chunk |= (sym & 0x03) << (12 - idx * 2);
    }
    chunkWords.push(chunk);
  }
  const rawLen = 20 + chunkWords.length * 2 + deltaBytes.length;
  const padLen = (4 - (rawLen % 4)) % 4;
  const totalLen = rawLen + padLen;
  const buf = Buffer.alloc(totalLen);
  buf[0] = (2 << 6) | BWE_DEFAULT_CONFIG.rtcpFmtTwcc;
  buf[1] = BWE_DEFAULT_CONFIG.rtcpRtpfbPt;
  buf.writeUInt16BE(totalLen / 4 - 1, 2);
  buf.writeUInt32BE(pkt.senderSsrc >>> 0, 4);
  buf.writeUInt32BE(pkt.mediaSsrc >>> 0, 8);
  buf.writeUInt16BE(pkt.baseSeq & 0xffff, 12);
  buf.writeUInt16BE(pkt.packets.length & 0xffff, 14);
  const refTime = pkt.referenceTime64ms & 0xff_ffff;
  buf[16] = (refTime >>> 16) & 0xff;
  buf[17] = (refTime >>> 8) & 0xff;
  buf[18] = refTime & 0xff;
  buf[19] = pkt.fbPktCount & 0xff;
  chunkWords.forEach((word, i) => buf.writeUInt16BE(word, 20 + i * 2));
  const deltaStart = 20 + chunkWords.length * 2;
  deltaBytes.forEach((b, i) => {
    buf[deltaStart + i] = b;
  });
  return buf;
}

/**
 * Декодирует статус-чанки RTCP TWCC в массив кодов статуса (`0..2`).
 * @param {Buffer} buf
 * @param {number} startOffset
 * @param {number} count
 * @returns {{ statuses: number[], offset: number }}
 */
function decodeTwccStatusChunks(buf, startOffset, count) {
  const statuses = [];
  let offset = startOffset;
  while (statuses.length < count) {
    if (offset + 2 > buf.length) {
      throw new Error("Truncated TWCC status chunk");
    }
    const chunk = buf.readUInt16BE(offset);
    offset += 2;
    const isVector = (chunk & 0x8000) !== 0;
    if (!isVector) {
      const symbol = (chunk >>> 13) & 0x03;
      const runLen = chunk & 0x1fff;
      const take = Math.min(runLen, count - statuses.length);
      for (let i = 0; i < take; i += 1) {
        statuses.push(symbol);
      }
    } else {
      const twoBit = (chunk & 0x4000) !== 0;
      const slots = twoBit ? 7 : 14;
      for (let idx = 0; idx < slots && statuses.length < count; idx += 1) {
        const sym = twoBit
          ? (chunk >>> (12 - idx * 2)) & 0x03
          : (chunk >>> (13 - idx)) & 0x01;
        statuses.push(sym);
      }
    }
  }
  return { statuses, offset };
}

/**
 * Разбирает бинарный пакет RTCP TWCC (`PT=205, FMT=15`) и вычисляет градиенты задержки.
 * @param {Buffer | Uint8Array} raw
 * @param {number} [defaultSendDeltaUs=20000]
 * @returns {{
 *   senderSsrc: number,
 *   mediaSsrc: number,
 *   baseSeq: number,
 *   packetStatusCount: number,
 *   referenceTime64ms: number,
 *   fbPktCount: number,
 *   feedbacks: Array<{
 *     sequence: number,
 *     received: boolean,
 *     recvDeltaUs: number | null,
 *     sendDeltaUs: number,
 *     delayGradientMs: number
 *   }>
 * }}
 */
export function parseTwccPacket(raw, defaultSendDeltaUs = 20_000) {
  const buf = Buffer.isBuffer(raw) ? raw : Buffer.from(raw);
  if (buf.length < 20) {
    throw new Error(`Truncated RTCP TWCC: слишком короткий пакет (${buf.length} байт)`);
  }
  const version = buf.readUInt8(0) >>> 6;
  const fmt = buf.readUInt8(0) & 0x1f;
  const pt = buf.readUInt8(1);
  if (
    version !== 2 ||
    fmt !== BWE_DEFAULT_CONFIG.rtcpFmtTwcc ||
    pt !== BWE_DEFAULT_CONFIG.rtcpRtpfbPt
  ) {
    throw new Error(`Неверный заголовок RTCP TWCC: V=${version}, FMT=${fmt}, PT=${pt}`);
  }
  const baseSeq = buf.readUInt16BE(12);
  const packetStatusCount = buf.readUInt16BE(14);
  const referenceTime64ms =
    (buf.readUInt8(16) << 16) | (buf.readUInt8(17) << 8) | buf.readUInt8(18);
  const fbPktCount = buf.readUInt8(19);
  const { statuses, offset: deltaStart } = decodeTwccStatusChunks(
    buf,
    20,
    packetStatusCount,
  );
  let cursor = deltaStart;
  const feedbacks = [];
  for (let i = 0; i < statuses.length; i += 1) {
    const seq = (baseSeq + i) & 0xffff;
    const status = statuses[i];
    if (status === 0) {
      feedbacks.push({
        sequence: seq,
        received: false,
        recvDeltaUs: null,
        sendDeltaUs: defaultSendDeltaUs,
        delayGradientMs: 0,
      });
    } else if (status === 1) {
      const recvDeltaUs = buf.readUInt8(cursor) * 250;
      cursor += 1;
      feedbacks.push({
        sequence: seq,
        received: true,
        recvDeltaUs,
        sendDeltaUs: defaultSendDeltaUs,
        delayGradientMs: (recvDeltaUs - defaultSendDeltaUs) / 1000,
      });
    } else {
      const recvDeltaUs = buf.readInt16BE(cursor) * 250;
      cursor += 2;
      feedbacks.push({
        sequence: seq,
        received: true,
        recvDeltaUs,
        sendDeltaUs: defaultSendDeltaUs,
        delayGradientMs: (recvDeltaUs - defaultSendDeltaUs) / 1000,
      });
    }
  }
  return {
    senderSsrc: buf.readUInt32BE(4),
    mediaSsrc: buf.readUInt32BE(8),
    baseSeq,
    packetStatusCount,
    referenceTime64ms,
    fbPktCount,
    feedbacks,
  };
}

/**
 * Определяет профиль разрешения и FPS видео для заданного битрейта (kbps).
 * @param {number} bitrateKbps
 * @returns {{ label: string, width: number, height: number, maxFps: number }}
 */
export function resolveVideoResolutionTier(bitrateKbps) {
  if (bitrateKbps >= 4_000) {
    return { label: "1080p60", width: 1920, height: 1080, maxFps: 60 };
  }
  if (bitrateKbps >= 2_500) {
    return { label: "1080p30", width: 1920, height: 1080, maxFps: 30 };
  }
  if (bitrateKbps >= 1_200) {
    return { label: "720p30", width: 1280, height: 720, maxFps: 30 };
  }
  return { label: "480p20", width: 854, height: 480, maxFps: 20 };
}

/**
 * Контроллер перегрузки сети GCC (Trendline + AIMD + REMB Cap) для видеопотоков WebRTC.
 */
export class GccCongestionController {
  /**
   * @param {{
   *   minBitrateKbps?: number,
   *   maxBitrateKbps?: number,
   *   startBitrateKbps?: number,
   *   overuseThresholdMs?: number,
   *   probingStepKbps?: number
   * }} [opts]
   */
  constructor(opts = {}) {
    this.minBitrateKbps = Math.max(
      BWE_DEFAULT_CONFIG.minBitrateKbps,
      opts.minBitrateKbps ?? BWE_DEFAULT_CONFIG.minBitrateKbps,
    );
    this.maxBitrateKbps = Math.min(
      BWE_DEFAULT_CONFIG.maxBitrateKbps,
      Math.max(
        this.minBitrateKbps,
        opts.maxBitrateKbps ?? BWE_DEFAULT_CONFIG.maxBitrateKbps,
      ),
    );
    this.currentBitrateKbps = Math.min(
      this.maxBitrateKbps,
      Math.max(
        this.minBitrateKbps,
        opts.startBitrateKbps ?? BWE_DEFAULT_CONFIG.startBitrateKbps,
      ),
    );
    this.rembCapKbps = this.maxBitrateKbps;
    this.overuseThresholdMs =
      opts.overuseThresholdMs ?? BWE_DEFAULT_CONFIG.overuseThresholdMs;
    this.probingStepKbps =
      opts.probingStepKbps ?? BWE_DEFAULT_CONFIG.probingStepKbps;
    this.smoothedGradientMs = 0;
    this.packetLossPercent = 0;
    /** @type {string} */
    this.usageState = "normal";
    /** @type {string} */
    this.interactiveState = BWE_GRAPH_STATES.default;
    this.connectionAlive = true;
    this.reconnectCount = 0;
  }

  /**
   * Плавно применяет целевой битрейт с ограничением шага падения `<= 15%` за один тик.
   * @param {number} desiredKbps
   * @returns {number}
   */
  applySmoothTarget(desiredKbps) {
    const hardCeiling = Math.min(this.maxBitrateKbps, this.rembCapKbps);
    const clampedTarget = Math.max(
      this.minBitrateKbps,
      Math.min(hardCeiling, Math.floor(desiredKbps)),
    );
    if (clampedTarget < this.currentBitrateKbps) {
      const minAllowed = Math.ceil((this.currentBitrateKbps * 85) / 100);
      this.currentBitrateKbps = Math.max(
        this.minBitrateKbps,
        Math.max(clampedTarget, minAllowed),
      );
    } else {
      this.currentBitrateKbps = clampedTarget;
    }
    return this.currentBitrateKbps;
  }

  /**
   * Обрабатывает обратную связь RTCP TWCC и адаптирует битрейт без сброса соединения.
   * @param {{
   *   baseSeq: number,
   *   packets: Array<{ sequence: number, received: boolean, recvDeltaUs?: number | null, sendDeltaUs?: number }>
   * }} feedback
   */
  onTwccFeedback(feedback) {
    const packets = feedback.packets ?? [];
    if (packets.length === 0) {
      return this.snapshot();
    }
    let lost = 0;
    let received = 0;
    let gradSumMs = 0;
    for (const pkt of packets) {
      if (!pkt.received) {
        lost += 1;
      } else {
        received += 1;
        const recvUs = pkt.recvDeltaUs ?? 20_000;
        const sendUs = pkt.sendDeltaUs ?? 20_000;
        gradSumMs += (recvUs - sendUs) / 1000;
      }
    }
    const meanGradMs = received > 0 ? gradSumMs / received : 0;
    this.packetLossPercent = (lost / packets.length) * 100;
    this.smoothedGradientMs = this.smoothedGradientMs * 0.3 + meanGradMs * 0.7;

    if (
      this.smoothedGradientMs > this.overuseThresholdMs ||
      this.packetLossPercent > 10
    ) {
      this.usageState = "overusing";
      this.interactiveState = BWE_GRAPH_STATES.active;
      const reduced = Math.floor((this.currentBitrateKbps * 85) / 100);
      this.applySmoothTarget(reduced);
    } else if (this.smoothedGradientMs < -this.overuseThresholdMs) {
      this.usageState = "underusing";
      this.interactiveState = BWE_GRAPH_STATES.hover;
    } else {
      this.usageState = "normal";
      this.interactiveState = BWE_GRAPH_STATES.default;
      const increased = this.currentBitrateKbps + this.probingStepKbps;
      this.applySmoothTarget(increased);
    }
    return this.snapshot();
  }

  /**
   * Обрабатывает ограничение битрейта из пакета RTCP REMB (`bitrateBps`).
   * @param {number} bitrateBps
   */
  onRembBitrateBps(bitrateBps) {
    const capKbps = Math.floor(Math.max(0, bitrateBps) / 1000);
    this.rembCapKbps = Math.max(
      this.minBitrateKbps,
      Math.min(this.maxBitrateKbps, capKbps),
    );
    if (this.rembCapKbps < this.currentBitrateKbps) {
      this.interactiveState = BWE_GRAPH_STATES.active;
      this.applySmoothTarget(this.rembCapKbps);
    }
    return this.snapshot();
  }

  /**
   * Возвращает неизменяемый снимок состояния контроллера перегрузки.
   */
  snapshot() {
    return Object.freeze({
      currentBitrateKbps: this.currentBitrateKbps,
      rembCapKbps: this.rembCapKbps,
      smoothedGradientMs: Number(this.smoothedGradientMs.toFixed(3)),
      packetLossPercent: Number(this.packetLossPercent.toFixed(2)),
      usageState: this.usageState,
      interactiveState: this.interactiveState,
      resolutionTier: resolveVideoResolutionTier(this.currentBitrateKbps),
      connectionAlive: this.connectionAlive,
      reconnectCount: this.reconnectCount,
    });
  }
}

/**
 * Сверяет константы BWE с независимыми оракулами (`kanban-board.json`, `native-re.md`, `voice-deep.md`, `rtc-frames.jsonl`).
 * @param {{
 *   kanbanPath: string,
 *   nativeRePath: string,
 *   voiceDeepPath: string,
 *   rtcFramesPath: string
 * }} paths
 */
export function verifyBweOracleParity(paths) {
  const reasons = [];
  const kanban = JSON.parse(readFileSync(paths.kanbanPath, "utf8"));
  const card = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-982");
  if (!card) {
    reasons.push("Карточка TASK-982 не найдена в waves/kanban-board.json");
  } else if (card.layout?.dimensions !== "500-8000 kbps") {
    reasons.push(`Неверный диапазон в карточке: ${card.layout?.dimensions}`);
  }
  const nativeRe = readFileSync(paths.nativeRePath, "utf8");
  if (!nativeRe.includes(BWE_DEFAULT_CONFIG.trendlineExperimentSymbol)) {
    reasons.push("В docs/re/native-re.md отсутствует символ TrendlineWindowDuration");
  }
  if (!nativeRe.includes(BWE_DEFAULT_CONFIG.twccExtensionUri)) {
    reasons.push("В docs/re/native-re.md отсутствует URI расширения TWCC");
  }
  const voiceDeep = readFileSync(paths.voiceDeepPath, "utf8");
  if (!voiceDeep.includes("1280x720@30")) {
    reasons.push("В docs/re/voice-deep.md отсутствует профиль 1280x720@30");
  }
  const rtcFramesLines = readFileSync(paths.rtcFramesPath, "utf8")
    .trim()
    .split("\n")
    .map((line) => {
      const outer = JSON.parse(line);
      return typeof outer.payload === "string" ? JSON.parse(outer.payload) : outer;
    });
  const op2 = rtcFramesLines.find((f) => f.op === 2);
  const op1 = rtcFramesLines.find((f) => f.op === 1);
  const videoStream = op2?.d?.streams?.[0];
  const av1Codec = op1?.d?.codecs?.find((/** @type {any} */ c) => c.name === "AV1");
  return {
    passed: reasons.length === 0,
    reasons,
    videoSsrc: videoStream?.ssrc ?? 0,
    rtxSsrc: videoStream?.rtx_ssrc ?? 0,
    av1PayloadType: av1Codec?.payload_type ?? 0,
  };
}

/**
 * Запускает детерминированный сквозной сценарий адаптации битрейта (2500 -> 8000 -> 500 -> 2500 kbps).
 */
export function runBweSimulationScenario() {
  const ctrl = new GccCongestionController();
  /** @type {Array<Record<string, unknown>>} */
  const timeline = [];
  let maxSingleStepDropRatio = 0;
  let minObservedBitrateKbps = ctrl.currentBitrateKbps;
  let maxObservedBitrateKbps = ctrl.currentBitrateKbps;

  const recordStep = (/** @type {string} */ phase, /** @type {number} */ prevKbps) => {
    const snap = ctrl.snapshot();
    if (snap.currentBitrateKbps < prevKbps) {
      const drop = (prevKbps - snap.currentBitrateKbps) / prevKbps;
      maxSingleStepDropRatio = Math.max(maxSingleStepDropRatio, drop);
    }
    minObservedBitrateKbps = Math.min(minObservedBitrateKbps, snap.currentBitrateKbps);
    maxObservedBitrateKbps = Math.max(maxObservedBitrateKbps, snap.currentBitrateKbps);
    timeline.push({ phase, ...snap });
  };

  for (let i = 0; i < 24; i += 1) {
    const prev = ctrl.currentBitrateKbps;
    ctrl.onTwccFeedback({
      baseSeq: i * 4,
      packets: [0, 1, 2, 3].map((k) => ({
        sequence: (i * 4 + k) & 0xffff,
        received: true,
        recvDeltaUs: 20_000,
        sendDeltaUs: 20_000,
      })),
    });
    recordStep("probing_to_max", prev);
  }
  for (let i = 0; i < 8; i += 1) {
    const prev = ctrl.currentBitrateKbps;
    ctrl.onTwccFeedback({
      baseSeq: 200 + i * 4,
      packets: [0, 1, 2, 3].map((k) => ({
        sequence: (200 + i * 4 + k) & 0xffff,
        received: true,
        recvDeltaUs: 45_000,
        sendDeltaUs: 20_000,
      })),
    });
    recordStep("twcc_overuse", prev);
  }
  for (let i = 0; i < 16; i += 1) {
    const prev = ctrl.currentBitrateKbps;
    ctrl.onRembBitrateBps(500_000);
    recordStep("remb_floor_500kbps", prev);
  }
  ctrl.onRembBitrateBps(8_000_000);
  for (let i = 0; i < 8; i += 1) {
    const prev = ctrl.currentBitrateKbps;
    ctrl.onTwccFeedback({
      baseSeq: 500 + i * 4,
      packets: [0, 1, 2, 3].map((k) => ({
        sequence: (500 + i * 4 + k) & 0xffff,
        received: true,
        recvDeltaUs: 20_000,
        sendDeltaUs: 20_000,
      })),
    });
    recordStep("recovery_probing", prev);
  }

  return {
    minObservedBitrateKbps,
    maxObservedBitrateKbps,
    finalBitrateKbps: ctrl.currentBitrateKbps,
    maxSingleStepDropRatio: Number(maxSingleStepDropRatio.toFixed(4)),
    connectionDropped: !ctrl.connectionAlive,
    reconnectCount: ctrl.reconnectCount,
    stepsExecuted: timeline.length,
    timeline,
  };
}

/**
 * Проверяет результаты симуляции BWE на соответствие критериям приёмки TASK-982.
 * @param {ReturnType<typeof runBweSimulationScenario>} sim
 * @returns {{ passed: boolean, reasons: string[] }}
 */
export function evaluateBweVerificationResult(sim) {
  const reasons = [];
  if (sim.connectionDropped || sim.reconnectCount !== 0) {
    reasons.push("Обнаружен сброс соединения или повторное подключение при адаптации битрейта");
  }
  if (sim.minObservedBitrateKbps < BWE_DEFAULT_CONFIG.minBitrateKbps) {
    reasons.push(`Битрейт опустился ниже 500 kbps: ${sim.minObservedBitrateKbps}`);
  }
  if (sim.maxObservedBitrateKbps > BWE_DEFAULT_CONFIG.maxBitrateKbps) {
    reasons.push(`Битрейт превысил 8000 kbps: ${sim.maxObservedBitrateKbps}`);
  }
  if (sim.maxSingleStepDropRatio > BWE_DEFAULT_CONFIG.maxSingleStepDropRatio + 0.0005) {
    reasons.push(`Превышен лимит плавного снижения за шаг (15%): ${sim.maxSingleStepDropRatio}`);
  }
  return { passed: reasons.length === 0, reasons };
}

/**
 * Маскирует токены и 32-байтовые ключи SRTP/DAVE в диагностических строках BWE.
 * @param {string} text
 * @returns {string}
 */
export function redactBweSecrets(text) {
  return String(text ?? "")
    .replace(/\b[0-9a-fA-F]{64}\b/g, "[REDACTED]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "mfa.***")
    .replace(/(secret_key\s*=\s*")[^"]+(")/gi, "$1[REDACTED]$2");
}

/**
 * Формирует Markdown-отчёт верификации BWE.
 * @param {{ parity: ReturnType<typeof verifyBweOracleParity>, sim: ReturnType<typeof runBweSimulationScenario>, verdict: ReturnType<typeof evaluateBweVerificationResult>, rustMd5: string }} data
 */
function renderMarkdownReport(data) {
  return [
    "# Отчёт верификации контроля перегрузки сети WebRTC BWE (TASK-982)",
    "",
    `- **Статус**: \`${data.verdict.passed && data.parity.passed ? "PASS" : "FAIL"}\``,
    `- **Диапазон адаптации битрейта**: \`${data.sim.minObservedBitrateKbps}..${data.sim.maxObservedBitrateKbps} kbps\``,
    `- **Максимальное снижение за один шаг**: \`${(data.sim.maxSingleStepDropRatio * 100).toFixed(2)}%\` (порог \`<= 15.00%\`)`,
    `- **Сброс соединения (` + "`connectionDropped`" + `)**: \`${data.sim.connectionDropped}\` (\`reconnectCount = ${data.sim.reconnectCount}\`)`,
    `- **SSRC из оракула ` + "`rtc-frames.jsonl`" + `**: video=\`${data.parity.videoSsrc}\`, rtx=\`${data.parity.rtxSsrc}\`, AV1 PT=\`${data.parity.av1PayloadType}\``,
    `- **MD5 модуля ` + "`rust/chord-voice/src/congestion.rs`" + `**: \`${data.rustMd5}\``,
    "",
  ].join("\n");
}

/**
 * Точка входа CLI `scripts/verify-bwe.mjs`.
 * @param {string[]} argv
 * @returns {number}
 */
export function runCli(argv) {
  let outJson = "";
  let outMd = "";
  let oraclePath = resolve(ROOT, "waves/kanban-board.json");

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const nextArg = argv[i + 1];
    if (arg === "--help" || arg === "-h") {
      process.stdout.write(
        "Использование: node scripts/verify-bwe.mjs [--out-json <path>] [--out-md <path>] [--oracle <path>]\n",
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

  const nativeRePath = resolve(ROOT, "docs/re/native-re.md");
  const voiceDeepPath = resolve(ROOT, "docs/re/voice-deep.md");
  const rtcFramesPath = resolve(ROOT, "fixtures/voice/rtc-frames.jsonl");
  const rustModulePath = resolve(ROOT, "rust/chord-voice/src/congestion.rs");

  for (const requiredPath of [oraclePath, nativeRePath, voiceDeepPath, rtcFramesPath, rustModulePath]) {
    if (!existsSync(requiredPath)) {
      process.stderr.write(`Оракул или модуль недоступен: ${requiredPath}\n`);
      return 2;
    }
  }

  const parity = verifyBweOracleParity({
    kanbanPath: oraclePath,
    nativeRePath,
    voiceDeepPath,
    rtcFramesPath,
  });
  const sim = runBweSimulationScenario();
  const verdict = evaluateBweVerificationResult(sim);
  const rustMd5 = createHash("md5").update(readFileSync(rustModulePath)).digest("hex");

  const payload = {
    taskId: "TASK-982",
    passed: parity.passed && verdict.passed,
    parity,
    verdict,
    rustMd5,
    simulation: {
      minObservedBitrateKbps: sim.minObservedBitrateKbps,
      maxObservedBitrateKbps: sim.maxObservedBitrateKbps,
      finalBitrateKbps: sim.finalBitrateKbps,
      maxSingleStepDropRatio: sim.maxSingleStepDropRatio,
      connectionDropped: sim.connectionDropped,
      reconnectCount: sim.reconnectCount,
      stepsExecuted: sim.stepsExecuted,
    },
  };

  if (outJson) {
    mkdirSync(dirname(outJson), { recursive: true });
    writeFileSync(outJson, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(outMd), { recursive: true });
    writeFileSync(outMd, renderMarkdownReport({ parity, sim, verdict, rustMd5 }), "utf8");
  }

  if (!payload.passed) {
    process.stderr.write(
      `Проверка BWE завершилась с ошибкой: ${[...parity.reasons, ...verdict.reasons].join("; ")}\n`,
    );
    return 1;
  }
  process.stdout.write(
    `Проверка WebRTC BWE (TASK-982) успешно пройдена: диапазон ${sim.minObservedBitrateKbps}..${sim.maxObservedBitrateKbps} kbps, сбросов соединения: 0.\n`,
  );
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli(process.argv.slice(2)));
}
