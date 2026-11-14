// @ts-check
/**
 * TASK-801 [P0] G5 Voice / Hardware:
 * Integrate NVENC/CUDA/Vulkan Video encoder in discord_voice.node on NVIDIA RTX 5070 Ti
 * instead of CPU libopenh264 fallback in rust/crates/chord-core/src/ws.rs.
 * Enable true 60 FPS hardware encoding without WebRTC VideoSource (30 FPS) or
 * QualityScalerResource (20 FPS) framerate degradation.
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

describe("TASK-801: NVENC/CUDA/Vulkan 60 FPS Hardware Video Encoder in discord_voice.node", () => {
  const wsRs = fs.readFileSync(WS_RS_PATH, "utf8");

  /**
   * Executes the real `session_description` handler from `DV_BRIDGE_JS` in `ws.rs`
   * against a mocked `conn` and environment to inspect `setTransportOptions`.
   *
   * @param {{
   *   msg?: Record<string, any>,
   *   env?: Record<string, string | undefined>,
   *   hasNvidiaDev?: boolean,
   *   vaapiEnabled?: boolean,
   *   isStreamWorker?: boolean,
   *   isSelfBroadcaster?: boolean
   * }} [options]
   * @returns {Array<Record<string, any>>}
   */
  function runRealSessionDescription(options = {}) {
    const startMarker = 'const bridgeStartTime = Date.now();';
    const ratesStart = wsRs.indexOf("function computeAdaptiveRates(");
    const ratesEnd = wsRs.indexOf("function bindVideoSinkId(");
    assert.ok(ratesStart > 0 && ratesEnd > ratesStart, "computeAdaptiveRates must exist in ws.rs");
    const ratesSnippet = wsRs.slice(ratesStart, ratesEnd);

    const sdStart = wsRs.indexOf('cmd === "session_description" && conn');
    const sdEnd = wsRs.indexOf('setTransportOptions err: " + (e && e.message) });\n    }', sdStart);
    assert.ok(sdStart > 0 && sdEnd > sdStart, "session_description block must exist in ws.rs");
    const blockSnippet = wsRs.slice(
      sdStart,
      sdEnd + 'setTransportOptions err: " + (e && e.message) });\n    }'.length,
    );

    void startMarker;
    /** @type {Array<Record<string, any>>} */
    const transportCalls = [];

    const hasNvidiaDev = options.hasNvidiaDev ?? true;
    const vaapiEnabled = options.vaapiEnabled ?? false;

    const sandbox = {
      global: {},
      process: {
        env: {
          ...(options.env || {}),
        },
      },
      fs: {
        existsSync: (/** @type {string} */ p) => (p === "/dev/nvidiactl" ? hasNvidiaDev : false),
      },
      dv: {
        isVaapiEnabled: () => vaapiEnabled,
        DegradationPreference: {
          MAINTAIN_RESOLUTION: 0,
          MAINTAIN_FRAMERATE: 1,
          BALANCED: 2,
          DISABLED: 3,
        },
      },
      cmd: "session_description",
      msg: options.msg || {
        mode: "aead_aes256_gcm_rtpsize",
        secretKey: new Array(32).fill(1),
      },
      isStreamWorker: options.isStreamWorker ?? true,
      isSelfBroadcaster: options.isSelfBroadcaster ?? true,
      connConnected: false,
      currentSelfMute: false,
      currentSelfDeafen: false,
      currentChannelId: "700482297027231769",
      currentLocalSsrc: 9208,
      currentVideoSsrc: 9209,
      currentRtxSsrc: 9210,
      smoothedPingMs: 25,
      activeVideoSinks: new Map(),
      attachConnCallbacks: () => {},
      bindVideoSinkId: () => {},
      flushMergedUsersToConn: () => {},
      applyScreenBroadcast: () => {},
      applyStreamAudioSettings: () => {},
      applyVoiceMuteAndVolumeState: () => {},
      sendEv: () => {},
      conn: {
        setTransportOptions(/** @type {Record<string, any>} */ opts) {
          transportCalls.push(opts);
        },
      },
    };

    const scriptSource = `
      ${ratesSnippet}
      if (${blockSnippet}
      }
    `;
    vm.runInNewContext(scriptSource, sandbox);
    return transportCalls;
  }

  it("T801-1: On NVIDIA RTX 5070 Ti, H264 and AV1X videoEncoder use NVENC/CUDA/Vulkan HW encoder and disable CPU libopenh264 fallback", () => {
    const h264Calls = runRealSessionDescription({
      msg: {
        mode: "aead_aes256_gcm_rtpsize",
        secretKey: new Array(32).fill(1),
        videoCodec: "H264",
      },
      env: {
        DISCORD_GPU_VENDOR: "NVIDIA",
        DV_GPU_VENDOR: "NVIDIA",
        DV_PREFERRED_CODEC: "H264",
      },
      hasNvidiaDev: true,
    });

    assert.equal(h264Calls.length, 1, "setTransportOptions must be called once");
    assert.ok(h264Calls[0]);
    const h264Enc = h264Calls[0].videoEncoder;
    assert.ok(h264Enc, "videoEncoder must be defined");
    assert.equal(h264Enc.name, "H264");
    assert.equal(h264Enc.type, 105);
    assert.equal(h264Enc.rtxType, 106);
    assert.equal(h264Enc.maxFrameRate, 60);
    assert.equal(h264Enc.params["hardware-h264"], "1", "hardware-h264 must be '1' on NVIDIA GPU");
    assert.equal(h264Enc.params["hardware-nvenc"], "1", "hardware-nvenc must be '1' on NVIDIA GPU");
    assert.equal(h264Enc.params["use-nvenc"], "1", "use-nvenc must be '1' on NVIDIA GPU");
    assert.equal(h264Enc.params["nvenc-cuda"], "1", "nvenc-cuda must be '1' on NVIDIA GPU");
    assert.equal(h264Enc.params["nvenc-vulkan"], "1", "nvenc-vulkan must be '1' on NVIDIA GPU");
    assert.equal(
      h264Enc.params["discord-latency-profile"],
      "ultra_low",
      "discord-latency-profile must be 'ultra_low' for NVENC low-latency preset",
    );
    assert.match(
      String(h264Enc.params["video-encoder-experiments"] || ""),
      /nvNewPresets/,
      "video-encoder-experiments must include nvNewPresets",
    );
    assert.match(
      String(h264Enc.params["video-encoder-experiments"] || ""),
      /linux-vulkan/,
      "video-encoder-experiments must include linux-vulkan",
    );
    assert.notEqual(
      h264Enc.params["libopenh264"],
      "1",
      "videoEncoder.params must NOT enable CPU libopenh264 ('1') when NVIDIA NVENC HW encoding is active",
    );
    assert.equal(
      h264Enc.params["libopenh264-path"],
      undefined,
      "videoEncoder.params must NOT pass libopenh264-path when NVIDIA NVENC HW encoding is active",
    );

    const av1Calls = runRealSessionDescription({
      msg: {
        mode: "aead_aes256_gcm_rtpsize",
        secretKey: new Array(32).fill(1),
        videoCodec: "AV1",
      },
      env: {
        DISCORD_GPU_VENDOR: "NVIDIA",
        DV_GPU_VENDOR: "NVIDIA",
        DV_PREFERRED_CODEC: "AV1",
      },
      hasNvidiaDev: true,
    });
    assert.equal(av1Calls.length, 1);
    assert.ok(av1Calls[0]);
    const av1Enc = av1Calls[0].videoEncoder;
    assert.equal(av1Enc.name, "AV1X");
    assert.equal(av1Enc.codec, "AV1");
    assert.equal(av1Enc.params["hardware-av1"], "1", "AV1X encoder on RTX 5070 Ti must enable hardware-av1");
    assert.equal(av1Enc.params["hardware-nvenc"], "1", "AV1X encoder on RTX 5070 Ti must enable hardware-nvenc");
    assert.equal(av1Enc.params["use-nvenc"], "1", "AV1X encoder on RTX 5070 Ti must enable use-nvenc");
  });

  it("T801-2: When neither NVIDIA GPU nor VAAPI is available, H264 videoEncoder gracefully falls back to CPU libopenh264", () => {
    const swCalls = runRealSessionDescription({
      msg: {
        mode: "aead_aes256_gcm_rtpsize",
        secretKey: new Array(32).fill(1),
        videoCodec: "H264",
      },
      env: {
        DISCORD_GPU_VENDOR: "CPU",
        DV_GPU_VENDOR: "CPU",
        DV_PREFERRED_CODEC: "H264",
      },
      hasNvidiaDev: false,
      vaapiEnabled: false,
    });

    assert.equal(swCalls.length, 1);
    assert.ok(swCalls[0]);
    const swEnc = swCalls[0].videoEncoder;
    assert.equal(swEnc.name, "H264");
    assert.equal(swEnc.params["hardware-h264"], "0");
    assert.equal(swEnc.params["libopenh264"], "1");
    assert.match(String(swEnc.params["libopenh264-path"] || ""), /libopenh264/);
    assert.equal(swEnc.params["hardware-nvenc"], undefined);
  });

  it("T801-3: Enforces true 60 FPS encoding options (captureVideoFrameRate: 60, encodingVideoDegradationPreference: MAINTAIN_FRAMERATE=1, ultra_low latency) across session_description, applyScreenBroadcast, and set_stream_source", () => {
    const calls = runRealSessionDescription({
      msg: {
        mode: "aead_aes256_gcm_rtpsize",
        secretKey: new Array(32).fill(1),
        videoCodec: "H264",
      },
      env: {
        DISCORD_GPU_VENDOR: "NVIDIA",
        DV_GPU_VENDOR: "NVIDIA",
      },
      hasNvidiaDev: true,
    });

    assert.equal(calls.length, 1);
    const opts = calls[0];
    assert.ok(opts);
    assert.equal(opts.encodingVideoFrameRate, 60, "encodingVideoFrameRate must be 60");
    assert.equal(
      opts.captureVideoFrameRate,
      60,
      "captureVideoFrameRate must be 60 so WebRTC VideoSource does not cap source aligned adapter at 30 FPS",
    );
    assert.equal(opts.remoteSinkWantsMaxFramerate, 60, "remoteSinkWantsMaxFramerate must be 60");
    assert.equal(
      opts.encodingVideoDegradationPreference,
      1,
      "encodingVideoDegradationPreference must be 1 (MAINTAIN_FRAMERATE) so QualityScalerResource never drops FPS to 20",
    );
    assert.equal(
      opts.videoEncoderLatencyProfile,
      "ultra_low",
      "videoEncoderLatencyProfile must be 'ultra_low'",
    );
    assert.match(
      String(opts.videoEncoderExperiments || ""),
      /nvNewPresets/,
      "videoEncoderExperiments must include nvNewPresets",
    );

    // Check applyScreenBroadcast in ws.rs
    const bcastIdx = wsRs.indexOf("function applyScreenBroadcast(");
    const bcastEndIdx = wsRs.indexOf("function applyStreamAudioSettings(", bcastIdx);
    const bcastBody = wsRs.slice(bcastIdx, bcastEndIdx);
    assert.match(
      bcastBody,
      /captureVideoFrameRate:\s*60/,
      "applyScreenBroadcast must set captureVideoFrameRate: 60",
    );
    assert.match(
      bcastBody,
      /encodingVideoDegradationPreference:/,
      "applyScreenBroadcast must set encodingVideoDegradationPreference (MAINTAIN_FRAMERATE)",
    );

    // Check set_stream_source in ws.rs
    const switchIdx = wsRs.indexOf('else if (cmd === "set_stream_source")');
    const switchEndIdx = wsRs.indexOf('else if (cmd === "stop")', switchIdx);
    const switchBody = wsRs.slice(switchIdx, switchEndIdx);
    assert.match(
      switchBody,
      /captureVideoFrameRate:\s*streamFrameRate/,
      "set_stream_source must set captureVideoFrameRate: streamFrameRate",
    );
    assert.match(
      switchBody,
      /encodingVideoDegradationPreference:/,
      "set_stream_source must set encodingVideoDegradationPreference",
    );
  });

  it("T801-4: DV_SCOPE_FIX_C preloads libcuda.so.1, libnvidia-encode.so.1, and libvulkan.so.1 with RTLD_GLOBAL, and build_voice_v8_video_op12 advertises 60 FPS", () => {
    const scopeFixIdx = wsRs.indexOf("const DV_SCOPE_FIX_C: &str");
    const scopeFixEnd = wsRs.indexOf("const DV_BRIDGE_JS: &str", scopeFixIdx);
    assert.ok(scopeFixIdx > 0 && scopeFixEnd > scopeFixIdx, "DV_SCOPE_FIX_C must exist in ws.rs");
    const scopeFixBody = wsRs.slice(scopeFixIdx, scopeFixEnd);

    assert.match(
      scopeFixBody,
      /"libcuda\.so\.1"/,
      "DV_SCOPE_FIX_C preload_libs must include libcuda.so.1 for NVIDIA CUDA context initialization",
    );
    assert.match(
      scopeFixBody,
      /"libnvidia-encode\.so\.1"/,
      "DV_SCOPE_FIX_C preload_libs must include libnvidia-encode.so.1 for NVENC hardware video encoding",
    );
    assert.match(
      scopeFixBody,
      /"libvulkan\.so\.1"/,
      "DV_SCOPE_FIX_C preload_libs must include libvulkan.so.1 for Vulkan video interop",
    );

    const op12Idx = wsRs.indexOf("pub fn build_voice_v8_video_op12(");
    assert.ok(op12Idx > 0, "build_voice_v8_video_op12 must exist in ws.rs");
    const op12Body = wsRs.slice(op12Idx, op12Idx + 900);
    assert.match(
      op12Body,
      /\\"max_framerate\\":60/,
      "build_voice_v8_video_op12 must advertise max_framerate: 60 for 60 FPS HW encoding",
    );
  });
});
