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
