// @ts-check
/**
 * TASK-800 [P0] G5 Voice: Контрактные тесты согласования и конфигурации видеокодека AV1
 * в WebRTC RTCControlSocket (`rust/crates/chord-core/src/ws.rs:3675-3707`).
 *
 * Проверяемые требования:
 * 1. При получении `video_codec: "AV1"` или `videoCodec: "AV1"` / `"AV1X"` в Opcode 4 Session Description
 *    `videoEncoderCfg` настраивается на `AV1X` (`codec: "AV1"`) с `type: 101`, `rtxType: 102`.
 * 2. Отсутствует слепой откат в `H264` при передаче `video_codec: "AV1"` (snake_case из OP 4),
 *    а также при `videoCodec: "unknown"` с аппаратным профилем `DV_PREFERRED_CODEC="AV1"` / `"AV1 (av1_nvenc)"`.
 * 3. В `videoDecoders` зарегистрирован декодер `AV1X` (`type: 101`, `rtxType: 102`, `"hardware-av1-decode": "1"`).
 * 4. В `ws.rs` обработчик `OP 4 SESSION_DESCRIPTION` и `PureRustVoiceV8State` сохраняют согласованный кодек `AV1`.
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");
const WS_RS_PATH = path.join(REPO_ROOT, "rust/crates/chord-core/src/ws.rs");

/**
 * Извлекает тело JS-моста `DV_BRIDGE_JS` из `rust/crates/chord-core/src/ws.rs`.
 * @param {string} wsRs
 * @returns {string}
 */
function extractDvBridgeJs(wsRs) {
  const marker = 'const DV_BRIDGE_JS: &str = r#"';
  const startIdx = wsRs.indexOf(marker);
  assert.ok(startIdx >= 0, "DV_BRIDGE_JS constant must exist in ws.rs");
  const bodyStart = startIdx + marker.length;
  const endIdx = wsRs.indexOf('"#;', bodyStart);
  assert.ok(endIdx > bodyStart, "DV_BRIDGE_JS closing delimiter must exist in ws.rs");
  return wsRs.slice(bodyStart, endIdx);
}

/**
 * Запускает обработку `session_description` в изолированном контексте `DV_BRIDGE_JS`.
 * @param {string} bridgeJs
 * @param {Record<string, any>} msg
 * @param {Record<string, string>} [envOverrides]
 * @returns {Record<string, any>}
 */
function runSessionDescription(bridgeJs, msg, envOverrides = {}) {
  const ratesIdx = bridgeJs.indexOf("function computeAdaptiveRates(");
  assert.ok(ratesIdx > 0, "computeAdaptiveRates must be defined in bridgeJs");
  const ratesEnd = bridgeJs.indexOf("function bindVideoSinkId(", ratesIdx);
  assert.ok(ratesEnd > ratesIdx, "bindVideoSinkId must follow computeAdaptiveRates");
  const ratesSnippet = bridgeJs.slice(ratesIdx, ratesEnd);

  const sessionDescIdx = bridgeJs.indexOf('cmd === "session_description" && conn');
  assert.ok(sessionDescIdx > 0, 'cmd === "session_description" && conn must exist in bridgeJs');

  const catchMarker = 'setTransportOptions err: " + (e && e.message) });\n    }';
  const catchIdx = bridgeJs.indexOf(catchMarker, sessionDescIdx);
  assert.ok(catchIdx > sessionDescIdx, "setTransportOptions catch block must exist");
  const blockEnd = catchIdx + catchMarker.length;
  const blockSnippet = bridgeJs.slice(sessionDescIdx, blockEnd);

  /** @type {Record<string, any> | null} */
  let capturedOptions = null;
  const dummyConn = {
    setTransportOptions: (/** @type {Record<string, any>} */ opts) => {
      capturedOptions = opts;
    },
    mergeUsers: () => {},
    setSelfMute: () => {},
    setSelfDeafen: () => {},
  };

  const sandbox = {
    conn: dummyConn,
    connConnected: false,
    currentLocalSsrc: 9208,
    currentVideoSsrc: 9209,
    currentRtxSsrc: 9210,
    currentSelfMute: false,
    currentSelfDeafen: false,
    currentChannelId: "700482297027231769",
    isStreamWorker: true,
    isSelfBroadcaster: true,
    smoothedPingMs: 45,
    activeVideoSinks: new Map(),
    attachConnCallbacks: () => {},
    flushMergedUsersToConn: () => {},
    bindVideoSinkId: () => {},
    applyScreenBroadcast: () => {},
    applyStreamAudioSettings: () => {},
    applyVoiceMuteAndVolumeState: () => {},
    sendEv: () => {},
    global: {},
    dv: { isVaapiEnabled: () => false },
    process: {
      env: {
        DV_HAS_SOUNDSHARE_MONITOR: "0",
        DV_PREFERRED_CODEC: "H264",
        ...envOverrides,
      },
    },
    fs: { existsSync: () => false },
    cmd: "session_description",
    msg,
  };

  const scriptCode = `
    ${ratesSnippet}
    if (${blockSnippet}
    }
  `;

  vm.createContext(sandbox);
  vm.runInContext(scriptCode, sandbox);

  assert.ok(capturedOptions, "conn.setTransportOptions must be called in session_description");
  return capturedOptions;
}

describe("TASK-800: Согласование и конфигурация видеокодека AV1 в WebRTC RTCControlSocket", () => {
  const wsRs = fs.readFileSync(WS_RS_PATH, "utf8");
  const bridgeJs = extractDvBridgeJs(wsRs);

  it("T800-1: Opcode 4 Session Description с video_codec: 'AV1' (snake_case) и videoCodec: 'AV1' настраивает videoEncoderCfg на AV1/AV1X (payload 101, rtxType 102) без отката в H264", () => {
    for (const payload of [
      { video_codec: "AV1", mode: "aead_aes256_gcm_rtpsize" },
      { videoCodec: "AV1", mode: "aead_aes256_gcm_rtpsize" },
      { video_codec: "AV1X", mode: "aead_aes256_gcm_rtpsize" },
      { videoCodec: "av1", mode: "aead_aes256_gcm_rtpsize" },
    ]) {
      const opts = runSessionDescription(bridgeJs, payload, { DV_PREFERRED_CODEC: "H264" });
      assert.ok(opts.videoEncoder, "videoEncoder must be present in transport options");
      assert.notEqual(
        opts.videoEncoder.name,
        "H264",
        `Must not fallback blindly to H264 when payload is ${JSON.stringify(payload)}`,
      );
      assert.ok(
        opts.videoEncoder.name === "AV1X" || opts.videoEncoder.name === "AV1",
        "videoEncoder.name must be AV1X or AV1",
      );
      assert.equal(opts.videoEncoder.codec, "AV1", "videoEncoder.codec must be 'AV1'");
      assert.equal(opts.videoEncoder.type, 101, "AV1 payload type must be 101");
      assert.equal(opts.videoEncoder.rtxType, 102, "AV1 RTX payload type must be 102");
      assert.equal(opts.videoEncoder.maxFrameRate, 60, "AV1 maxFrameRate must be 60");
      assert.equal(opts.videoEncoder.width, 1280, "AV1 width must be 1280");
      assert.equal(opts.videoEncoder.height, 720, "AV1 height must be 720");
      assert.equal(opts.videoEncoder.minBitrate, 350000, "AV1 minBitrate must be 350000");
      assert.ok(opts.videoEncoder.maxBitrate > 0, "AV1 maxBitrate must be > 0");
      assert.ok(opts.videoEncoder.targetBitrate > 0, "AV1 targetBitrate must be > 0");
    }
  });

  it("T800-2: При videoCodec='unknown' (или пустом) и DV_PREFERRED_CODEC='AV1' / 'AV1 (av1_nvenc)' не откатывается слепо в H264, а выбирает AV1X (101/102)", () => {
    for (const pref of ["AV1", "AV1X", "AV1 (av1_nvenc)", "AV1 (av1_vaapi)"]) {
      const opts = runSessionDescription(
        bridgeJs,
        { videoCodec: "unknown", mode: "aead_aes256_gcm_rtpsize" },
        { DV_PREFERRED_CODEC: pref },
      );
      assert.notEqual(
        opts.videoEncoder.name,
        "H264",
        `Must not blindly fallback to H264 when videoCodec='unknown' and DV_PREFERRED_CODEC='${pref}'`,
      );
      assert.equal(opts.videoEncoder.name, "AV1X");
      assert.equal(opts.videoEncoder.type, 101);
      assert.equal(opts.videoEncoder.rtxType, 102);
    }
  });

  it("T800-3: Декодер AV1X в videoDecoders сконфигурирован с type: 101, rtxType: 102 и hardware-av1-decode: '1'", () => {
    const opts = runSessionDescription(bridgeJs, {
      video_codec: "AV1",
      mode: "aead_aes256_gcm_rtpsize",
    });
    assert.ok(Array.isArray(opts.videoDecoders), "videoDecoders must be an array");
    const av1Dec = opts.videoDecoders.find((d) => d.name === "AV1X" || d.name === "AV1");
    assert.ok(av1Dec, "AV1X decoder must be registered in videoDecoders");
    assert.equal(av1Dec.type, 101, "AV1 decoder payload type must be 101");
    assert.equal(av1Dec.rtxType, 102, "AV1 decoder rtxType must be 102");
    assert.equal(
      av1Dec.params["hardware-av1-decode"],
      "1",
      "AV1 decoder must signal hardware-av1-decode: '1'",
    );
  });

  it("T800-4: ws.rs обновляет active_video_codec и PureRustVoiceV8State.video_codec при получении OP 4 SESSION_DESCRIPTION с video_codec: 'AV1'", () => {
    assert.match(
      wsRs,
      /pub\s+fn\s+video_codec\(&self\)\s*->\s*Option<&str>/,
      "PureRustVoiceV8State in ws.rs must expose video_codec(&self) -> Option<&str>",
    );
    assert.match(
      wsRs,
      /g\.active_video_codec\s*=/,
      "OP 4 SESSION_DESCRIPTION handler in ws.rs must update g.active_video_codec when video_codec is negotiated",
    );
  });
});
