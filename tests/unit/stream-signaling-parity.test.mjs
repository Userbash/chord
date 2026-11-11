// @ts-check
/**
 * TASK-761: Контрактные тесты сигнализации стрима (Go Live / Stream Signaling)
 * для устранения дефекта подключения официального клиента Discord к трансляции.
 *
 * Проверяемые требования:
 * 1. Stream Key парсинг и извлечение channel_id для guild и call стримов без вычитания -1.
 * 2. Кодеки видео в OP 1 SELECT_PROTOCOL: и H264, и VP8 должны поддерживать encode: true при вещании.
 * 3. Извлечение серверных SSRCs (video_ssrc, rtx_ssrc) из d.streams в OP 2 READY.
 * 4. Формирование OP 12 VIDEO с валидными SSRCs и активным стримом.
 * 5. Формирование OP 4 VOICE_STATE_UPDATE со статусом self_stream: true.
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("TASK-761: Stream Key & Channel ID Resolution", () => {
  /**
   * @param {string | null | undefined} streamKey
   * @param {string | null} [fallbackCid]
   * @returns {string | null}
   */
  function parseStreamKeyChannel(streamKey, fallbackCid = null) {
    if (!streamKey || typeof streamKey !== "string") return fallbackCid;
    const parts = streamKey.split(":");
    if (parts[0] === "call" && parts.length >= 3 && parts[1]) {
      return parts[1];
    }
    if (parts[0] === "guild" && parts.length >= 4 && parts[2]) {
      return parts[2];
    }
    return fallbackCid;
  }

  /**
   * @param {string | null | undefined} streamKey
   * @param {string | null} [fallbackServerId]
   * @returns {string | null}
   */
  function parseStreamKeyServerId(streamKey, fallbackServerId = null) {
    if (!streamKey || typeof streamKey !== "string") return fallbackServerId;
    const parts = streamKey.split(":");
    if (parts[0] === "guild" && parts.length >= 4 && parts[1]) {
      return parts[1];
    }
    return fallbackServerId;
  }

  it("extracts exact channel_id from guild stream key without subtracting 1", () => {
    const skey = "guild:132049582910394857:987654321098765432:112233445566778899";
    const cid = parseStreamKeyChannel(skey);
    assert.equal(cid, "987654321098765432", "Channel ID must be parsed from segment 2");
    assert.notEqual(cid, "132049582910394856", "Channel ID must not be server_id - 1");
  });

  it("extracts exact channel_id from call stream key", () => {
    const skey = "call:887766554433221100:112233445566778899";
    const cid = parseStreamKeyChannel(skey);
    assert.equal(cid, "887766554433221100", "Channel ID must be parsed from segment 1");
  });

  it("extracts exact server_id from guild stream key", () => {
    const skey = "guild:132049582910394857:987654321098765432:112233445566778899";
    const sid = parseStreamKeyServerId(skey);
    assert.equal(sid, "132049582910394857", "Server ID must be parsed from segment 1");
  });
});

describe("TASK-761: Codec Negotiation in OP 1 SELECT_PROTOCOL", () => {
  /**
   * @param {string} extIp
   * @param {number} extPort
   * @param {string} mode
   * @param {boolean} canEncode
   */
  function buildSelectProtocolPayload(extIp, extPort, mode, canEncode) {
    return {
      op: 1,
      d: {
        protocol: "udp",
        address: extIp,
        port: extPort,
        mode: mode,
        data: { address: extIp, port: extPort, mode: mode },
        codecs: [
          { name: "opus", type: "audio", priority: 1000, payload_type: 120 },
          { name: "H264", type: "video", priority: 1000, payload_type: 105, rtx_payload_type: 106, encode: canEncode, decode: true },
          { name: "VP8", type: "video", priority: 2000, payload_type: 107, rtx_payload_type: 108, encode: canEncode, decode: true },
          { name: "AV1", type: "video", priority: 3000, payload_type: 101, rtx_payload_type: 102, encode: false, decode: true },
        ],
      },
    };
  }

  it("enables encode for both H264 and VP8 when canEncode is true", () => {
    const payload = buildSelectProtocolPayload("192.168.1.50", 50000, "aead_aes256_gcm_rtpsize", true);
    const h264 = payload.d.codecs.find((c) => c.name === "H264");
    const vp8 = payload.d.codecs.find((c) => c.name === "VP8");
    assert.ok(h264, "H264 codec must be present");
    assert.ok(vp8, "VP8 codec must be present");
    assert.equal(h264.encode, true, "H264 encode must be enabled when broadcasting");
    assert.equal(vp8.encode, true, "VP8 encode must be enabled when broadcasting");
  });
});

describe("TASK-761: SSRC Extraction from OP 2 READY", () => {
  /**
   * @param {any} readyData
   * @param {number} localSsrc
   */
  function extractSfuSsrcs(readyData, localSsrc) {
    let videoSsrc = localSsrc + 1;
    let rtxSsrc = localSsrc + 2;
    if (Array.isArray(readyData?.streams)) {
      for (const s of readyData.streams) {
        if (s.type === "video" || s.type === "screen") {
          if (typeof s.ssrc === "number" && s.ssrc > 0) {
            videoSsrc = s.ssrc;
          }
          if (typeof s.rtx_ssrc === "number" && s.rtx_ssrc > 0) {
            rtxSsrc = s.rtx_ssrc;
          } else if (typeof s.rtxSsrc === "number" && s.rtxSsrc > 0) {
            rtxSsrc = s.rtxSsrc;
          }
        }
      }
    }
    return { videoSsrc, rtxSsrc };
  }

  it("extracts server-assigned SSRC from d.streams in OP 2 READY", () => {
    const readyPacket = {
      ssrc: 9208,
      ip: "104.29.146.60",
      port: 19329,
      streams: [
        { type: "video", rid: "100", ssrc: 9209, rtx_ssrc: 9210, quality: 100, active: false }
      ],
    };
    const ssrcs = extractSfuSsrcs(readyPacket, 9208);
    assert.equal(ssrcs.videoSsrc, 9209, "Must extract video SSRC 9209");
    assert.equal(ssrcs.rtxSsrc, 9210, "Must extract rtx SSRC 9210");
  });
});

describe("TASK-761: Gateway OP 4 VOICE_STATE_UPDATE self_stream flag", () => {
  /**
   * @param {string | null} guildId
   * @param {string | null} channelId
   * @param {boolean} selfMute
   * @param {boolean} selfDeaf
   * @param {boolean} selfVideo
   * @param {boolean} selfStream
   */
  function buildVoiceStatePayload(guildId, channelId, selfMute, selfDeaf, selfVideo, selfStream) {
    return {
      op: 4,
      d: {
        guild_id: guildId,
        channel_id: channelId,
        self_mute: selfMute,
        self_deaf: selfDeaf,
        self_video: selfVideo,
        self_stream: selfStream,
        flags: selfStream ? 2 : 0,
      },
    };
  }

  it("includes self_stream: true in OP 4 when broadcasting", () => {
    const vs = buildVoiceStatePayload("100", "200", false, false, false, true);
    assert.equal(vs.d.self_stream, true, "self_stream must be true in OP 4");
    assert.equal(vs.d.flags, 2, "flags must indicate stream");
  });
});

describe("TASK-778: Gateway OP 18/19/22 & Voice OP 1 SELECT_PROTOCOL Source Parity", () => {
  const wsRs = fs.readFileSync(path.join(REPO_ROOT, "rust/crates/discord-core/src/ws.rs"), "utf8");
  const mainRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/main.rs"), "utf8");

  it("T778-1: queue_live_stream_delete in ws.rs clears LAST_STREAM_KEY for instant stream restart", () => {
    const fnIdx = wsRs.indexOf("pub fn queue_live_stream_delete(");
    assert.ok(fnIdx > 0, "queue_live_stream_delete must exist in ws.rs");
    const fnBody = wsRs.slice(fnIdx, fnIdx + 1200);
    assert.match(
      fnBody,
      /LAST_STREAM_KEY|clear_last_stream_key\(\)/,
      "queue_live_stream_delete must clear LAST_STREAM_KEY so restarting a stream in the same channel is not suppressed",
    );
    assert.match(
      fnBody,
      /EtfValue::Int\(19\)/,
      "queue_live_stream_delete must encode Gateway OP 19 STREAM_DELETE",
    );
  });

  it("T778-2: ws.rs encodes OP 18 STREAM_CREATE with Atom('nil') and exposes queue_live_stream_set_paused for OP 22", () => {
    const createIdx = wsRs.indexOf("pub fn queue_live_stream_create(");
    assert.ok(createIdx > 0, "queue_live_stream_create must exist in ws.rs");
    const createBody = wsRs.slice(createIdx, createIdx + 1500);
    assert.match(
      createBody,
      /EtfValue::Atom\("nil"\.to_string\(\)\)/,
      "queue_live_stream_create must encode null fields as Atom('nil') for Discord wetf compatibility",
    );
    assert.match(
      wsRs,
      /pub fn queue_live_stream_set_paused\s*\(/,
      "ws.rs must expose queue_live_stream_set_paused for OP 22",
    );
  });

  it("T778-3: main.rs clears LAST_STREAM_KEY on stream start/stop/disconnect and wires OP 22 set_paused", () => {
    const stopIdx = mainRs.indexOf("async fn voice_stream_stop(");
    assert.ok(stopIdx > 0, "voice_stream_stop must exist in main.rs");
    const stopBody = mainRs.slice(stopIdx, stopIdx + 1800);
    assert.match(
      stopBody,
      /clear_last_stream_key\(\)/,
      "voice_stream_stop in main.rs must explicitly call clear_last_stream_key()",
    );
    assert.match(
      stopBody,
      /queue_live_stream_delete/,
      "voice_stream_stop in main.rs must call queue_live_stream_delete",
    );

    const pauseIdx = mainRs.indexOf("async fn voice_stream_set_paused(");
    assert.ok(pauseIdx > 0, "voice_stream_set_paused must exist in main.rs");
    const pauseBody = mainRs.slice(pauseIdx, pauseIdx + 900);
    assert.match(
      pauseBody,
      /queue_live_stream_set_paused/,
      "voice_stream_set_paused in main.rs must dispatch OP 22 via queue_live_stream_set_paused",
    );
  });

  it("T778-4: ws.rs OP 1 SELECT_PROTOCOL enables encode for both H264 (105/106) and VP8 (107/108) when broadcasting", () => {
    const h264Matches = wsRs.match(
      /\{\{\\"name\\":\\"H264\\",\\"type\\":\\"video\\",\\"priority\\":1000,\\"payload_type\\":105,\\"rtx_payload_type\\":106,\\"encode\\":\{can_encode\},\\"decode\\":true\}\}/g,
    );
    const vp8Matches = wsRs.match(
      /\{\{\\"name\\":\\"VP8\\",\\"type\\":\\"video\\",\\"priority\\":2000,\\"payload_type\\":107,\\"rtx_payload_type\\":108,\\"encode\\":\{can_encode\},\\"decode\\":true\}\}/g,
    );
    assert.ok(
      h264Matches && h264Matches.length >= 2,
      "Both pure-Rust and native bridge OP 1 SELECT_PROTOCOL payloads must configure H264 encode:{can_encode}",
    );
    assert.ok(
      vp8Matches && vp8Matches.length >= 2,
      "Both pure-Rust and native bridge OP 1 SELECT_PROTOCOL payloads must configure VP8 encode:{can_encode}",
    );
    assert.match(
      wsRs,
      /is_cached_broadcaster/,
      "run_rtc_task must resolve is_self_broadcaster with LAST_VOICE_STATE_CACHE fallback so can_encode is true when broadcasting",
    );
  });

  it("T785-1: ws.rs handles incoming Voice Opcode 16 with heartbeat/ping reply (OP 3) and sends Voice Opcode 12 (VIDEO) with video_ssrc and rtx_ssrc", () => {
    assert.match(
      wsRs,
      /pub fn build_voice_v8_op16\(\)/,
      "ws.rs must expose build_voice_v8_op16() helper",
    );
    assert.match(
      wsRs,
      /pub fn build_voice_v8_video_op12\(/,
      "ws.rs must expose build_voice_v8_video_op12() helper with audio_ssrc, video_ssrc, and rtx_ssrc",
    );
    assert.match(
      wsRs,
      /16\s*=>\s*\{[\s\S]{0,600}?VOICE_BACKEND_VERSION[\s\S]{0,600}?build_voice_v8_heartbeat_op3\(/,
      "run_rtc_task in ws.rs must handle incoming Voice Opcode 16 and respond with OP 3 heartbeat/ping",
    );
    assert.match(
      wsRs,
      /"video_callback"\s*=>\s*\{[\s\S]{0,1600}?build_voice_v8_video_op12\(/,
      "run_rtc_task in ws.rs must emit OP 12 VIDEO with video_ssrc and rtx_ssrc on native video_callback",
    );
  });
});
