// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");
const WS_RS_PATH = path.join(REPO_ROOT, "rust/crates/discord-core/src/ws.rs");

/**
 * Извлекает тело JS-моста `DV_BRIDGE_JS` из `rust/crates/discord-core/src/ws.rs`.
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
 * Запускает обработку session_description в изолированном контексте DV_BRIDGE_JS.
 * @param {string} bridgeJs
 * @param {Record<string, any>} msg
 * @param {Record<string, string>} [envOverrides]
 * @returns {Record<string, any>} captured transport options
 */
function runSessionDescription(bridgeJs, msg, envOverrides = {}) {
  const ratesIdx = bridgeJs.indexOf("function computeAdaptiveRates(");
  assert.ok(ratesIdx > 0, "computeAdaptiveRates must be defined in bridgeJs");

  const sessionDescIdx = bridgeJs.indexOf('cmd === "session_description" && conn');
  assert.ok(sessionDescIdx > 0, 'cmd === "session_description" must exist in bridgeJs');

  let capturedOptions = null;
  const dummyConn = {
    setTransportOptions: (/** @type {any} */ opts) => {
      capturedOptions = opts;
    },
    mergeUsers: () => {},
    setSelfMute: () => {},
    setSelfDeafen: () => {},
    setOnSpeakingCallback: () => {},
    setOnSpeakingWhileMutedCallback: () => {},
    setPingInterval: () => {},
    setPingCallback: () => {},
    setPingTimeoutCallback: () => {},
    prepareSecureFramesTransition: () => {},
    prepareSecureFramesEpoch: () => {},
    executeSecureFramesTransition: () => {},
  };

  const sandbox = {
    conn: dummyConn,
    connConnected: false,
    currentLocalSsrc: 1000,
    currentVideoSsrc: 1001,
    currentRtxSsrc: 1002,
    currentSelfMute: false,
    currentSelfDeafen: false,
    currentChannelId: "999888777",
    isStreamWorker: false,
    isSelfBroadcaster: true,
    hasSoundshareMonitor: false,
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
        ...envOverrides,
      },
    },
    fs: { existsSync: () => false },
    cmd: "session_description",
    msg,
  };

  const ratesEnd = bridgeJs.indexOf("function bindVideoSinkId(", ratesIdx);
  assert.ok(ratesEnd > ratesIdx, "ratesEnd must exist");
  const ratesSnippet = bridgeJs.slice(ratesIdx, ratesEnd);

  const blockStart = sessionDescIdx;
  const catchMarker = "setTransportOptions err: \" + (e && e.message) });\n    }";
  const catchIdx = bridgeJs.indexOf(catchMarker, blockStart);
  assert.ok(catchIdx > blockStart, "catch block must exist");
  const blockEnd = catchIdx + catchMarker.length;
  const blockSnippet = bridgeJs.slice(blockStart, blockEnd);

  const scriptCode = `
    let smoothedPingMs = 60;
    ${ratesSnippet}
    if (${blockSnippet}
    }
  `;

  vm.createContext(sandbox);
  vm.runInContext(scriptCode, sandbox);

  assert.ok(capturedOptions, "conn.setTransportOptions must be called in session_description");
  return capturedOptions;
}

describe("TASK-802: WebRTC VP8 and VP9 Codec Negotiation and Transport Options", () => {
  const wsRs = fs.readFileSync(WS_RS_PATH, "utf8");
  const bridgeJs = extractDvBridgeJs(wsRs);

  it("T802-1: Negotiates VP8 encoder when server specifies videoCodec='VP8'", () => {
    const opts = runSessionDescription(bridgeJs, {
      videoCodec: "VP8",
      mode: "aead_aes256_gcm_rtpsize",
    });

    assert.ok(opts.videoEncoder, "videoEncoder must be configured");
    assert.equal(opts.videoEncoder.name, "VP8", "Encoder name must be VP8");
    assert.equal(opts.videoEncoder.type, 107, "VP8 payload type must be 107");
    assert.equal(opts.videoEncoder.rtxType, 108, "VP8 RTX payload type must be 108");
    assert.equal(opts.videoEncoder.params["keyframe-interval"], "3000", "VP8 keyframe interval must be 3000ms");
    assert.equal(opts.videoEncoder.params["temporal-layers"], "3", "VP8 temporal layers must be configured to 3");
    assert.equal(opts.videoEncoder.params["profile-id"], "0", "VP8 profile-id must be 0");
    assert.ok(opts.videoEncoder.maxBitrate > 0, "Adaptive maxBitrate must be populated");
    assert.ok(opts.videoEncoder.targetBitrate > 0, "Adaptive targetBitrate must be populated");
    assert.equal(opts.videoEncoder.minBitrate, 350000, "VP8 minBitrate must be 350000");
  });

  it("T802-2: Negotiates VP8 encoder when client explicitly requests VP8 via DV_PREFERRED_CODEC='VP8'", () => {
    const opts = runSessionDescription(
      bridgeJs,
      { mode: "aead_aes256_gcm_rtpsize" },
      { DV_PREFERRED_CODEC: "VP8" },
    );

    assert.ok(opts.videoEncoder, "videoEncoder must be configured");
    assert.equal(opts.videoEncoder.name, "VP8", "Encoder name must be VP8 when requested via DV_PREFERRED_CODEC");
    assert.equal(opts.videoEncoder.type, 107, "VP8 payload type must be 107");
    assert.equal(opts.videoEncoder.rtxType, 108, "VP8 RTX payload type must be 108");
    assert.equal(opts.videoEncoder.params["temporal-layers"], "3", "VP8 temporal layers must be 3");
    assert.equal(opts.videoEncoder.params["keyframe-interval"], "3000", "VP8 keyframe interval must be 3000ms");
  });

  it("T802-3: Negotiates VP9 encoder with payload 109 and rtx 110 when server specifies videoCodec='VP9'", () => {
    const opts = runSessionDescription(bridgeJs, {
      videoCodec: "VP9",
      mode: "aead_aes256_gcm_rtpsize",
    });

    assert.ok(opts.videoEncoder, "videoEncoder must be configured for VP9");
    assert.equal(opts.videoEncoder.name, "VP9", "Encoder name must be VP9");
    assert.equal(opts.videoEncoder.type, 109, "VP9 payload type must be 109");
    assert.equal(opts.videoEncoder.rtxType, 110, "VP9 RTX payload type must be 110");
    assert.equal(opts.videoEncoder.params["keyframe-interval"], "3000", "VP9 keyframe interval must be 3000ms");
    assert.equal(opts.videoEncoder.params["temporal-layers"], "3", "VP9 temporal layers must be configured to 3");
    assert.equal(opts.videoEncoder.params["profile-id"], "0", "VP9 profile-id must be 0");
    assert.ok(opts.videoEncoder.maxBitrate > 0, "Adaptive maxBitrate must be populated for VP9");
    assert.ok(opts.videoEncoder.targetBitrate > 0, "Adaptive targetBitrate must be populated for VP9");
    assert.equal(opts.videoEncoder.minBitrate, 350000, "VP9 minBitrate must be 350000");
  });

  it("T802-4: Negotiates VP9 encoder when client explicitly requests VP9 via DV_PREFERRED_CODEC='VP9'", () => {
    const opts = runSessionDescription(
      bridgeJs,
      { mode: "aead_aes256_gcm_rtpsize" },
      { DV_PREFERRED_CODEC: "VP9" },
    );

    assert.ok(opts.videoEncoder, "videoEncoder must be configured for VP9");
    assert.equal(opts.videoEncoder.name, "VP9", "Encoder name must be VP9");
    assert.equal(opts.videoEncoder.type, 109, "VP9 payload type must be 109");
    assert.equal(opts.videoEncoder.rtxType, 110, "VP9 RTX payload type must be 110");
  });

  it("T802-5: Initializes VP8 and VP9 in videoDecoders with valid parameters without empty params: {}", () => {
    const opts = runSessionDescription(bridgeJs, {
      videoCodec: "H264",
      mode: "aead_aes256_gcm_rtpsize",
    });

    assert.ok(Array.isArray(opts.videoDecoders), "videoDecoders must be an array");
    const vp8Decoder = opts.videoDecoders.find((d) => d.name === "VP8");
    const vp9Decoder = opts.videoDecoders.find((d) => d.name === "VP9");

    assert.ok(vp8Decoder, "VP8 decoder must be registered in videoDecoders");
    assert.equal(vp8Decoder.type, 107, "VP8 decoder payload type must be 107");
    assert.equal(vp8Decoder.rtxType, 108, "VP8 decoder rtxType must be 108");
    assert.notDeepEqual(vp8Decoder.params, {}, "VP8 decoder params must not be empty {}");
    assert.equal(vp8Decoder.params["keyframe-interval"], "3000", "VP8 decoder keyframe interval must be 3000ms");
    assert.equal(vp8Decoder.params["temporal-layers"], "3", "VP8 decoder temporal layers must be 3");

    assert.ok(vp9Decoder, "VP9 decoder must be registered in videoDecoders");
    assert.equal(vp9Decoder.type, 109, "VP9 decoder payload type must be 109");
    assert.equal(vp9Decoder.rtxType, 110, "VP9 decoder rtxType must be 110");
    assert.notDeepEqual(vp9Decoder.params, {}, "VP9 decoder params must not be empty {}");
    assert.equal(vp9Decoder.params["keyframe-interval"], "3000", "VP9 decoder keyframe interval must be 3000ms");
    assert.equal(vp9Decoder.params["temporal-layers"], "3", "VP9 decoder temporal layers must be 3");
  });

  it("T802-6: Transport options enforces global keyframeInterval: 3000 and adaptive encodingVideoBitRate", () => {
    const opts = runSessionDescription(bridgeJs, {
      videoCodec: "VP8",
      mode: "aead_aes256_gcm_rtpsize",
    });

    assert.equal(opts.keyframeInterval, 3000, "setTransportOptions must pass keyframeInterval: 3000");
    assert.ok(opts.encodingVideoBitRate > 0, "encodingVideoBitRate must be adaptive");
    assert.equal(opts.encodingVideoMinBitRate, 350000, "encodingVideoMinBitRate must be 350000");
    assert.ok(opts.encodingVideoMaxBitRate >= opts.encodingVideoBitRate, "Max bit rate must be >= target bit rate");
  });
});
