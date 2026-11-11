// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  createVoiceDevicesSettings,
  createVoiceDevicesSettingsController,
} from "../../rust/gui-spike/ui/features/voice-devices-settings.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");
const WS_RS_PATH = path.join(REPO_ROOT, "rust/crates/discord-core/src/ws.rs");
const DATA_RS_PATH = path.join(REPO_ROOT, "rust/gui-spike/src/data.rs");
const LIVE_SETTINGS_PATH = path.join(
  REPO_ROOT,
  "rust/gui-spike/ui/features/live-settings-enhancer.js",
);

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
 * Загружает вспомогательные функции стабилизации Krisp NC / VAD из `DV_BRIDGE_JS` в изолированный VM-контекст.
 * @param {string} bridgeJs
 */
function loadKrispStabilityHelpers(bridgeJs) {
  const dspIdx = bridgeJs.indexOf("function buildVoiceDspTransportOptions(");
  const attachIdx = bridgeJs.indexOf("function attachConnCallbacks(");
  const flushIdx = bridgeJs.indexOf("function flushMergedUsersToConn(");
  assert.ok(dspIdx > 0, "buildVoiceDspTransportOptions must be defined in DV_BRIDGE_JS");
  assert.ok(attachIdx > dspIdx, "attachConnCallbacks must follow DSP helpers in DV_BRIDGE_JS");
  assert.ok(flushIdx > attachIdx, "flushMergedUsersToConn must follow attachConnCallbacks in DV_BRIDGE_JS");

  const snippet = bridgeJs.slice(dspIdx, flushIdx);
  /** @type {Array<Record<string, unknown>>} */
  const events = [];
  /** @type {Record<string, any>} */
  const sandbox = {
    currentUserId: "111222333",
    sendEv: (/** @type {Record<string, unknown>} */ ev) => events.push(ev),
    subscribeRemoteVideoSink: () => {},
  };
  vm.createContext(sandbox);
  vm.runInContext(snippet, sandbox);
  return {
    /** @type {(isStreamConn: boolean, krispEnabled: boolean, autoGain: boolean) => Record<string, unknown>} */
    buildVoiceDspTransportOptions: sandbox.buildVoiceDspTransportOptions,
    /** @type {(isStreamConn: boolean, vadThreshold: number, autoSensitivity: boolean) => Record<string, unknown>} */
    buildSafeVadInputModeOptions: sandbox.buildSafeVadInputModeOptions,
    /** @type {(opts: any) => any} */
    enforceSafeVadTransportOptions: sandbox.enforceSafeVadTransportOptions,
    /** @type {(c: any) => void} */
    attachConnCallbacks: sandbox.attachConnCallbacks,
    events,
  };
}

describe("TASK-763: Krisp NC (noiseCancellation: true) & WebRTC VAD (vadUseKrisp: false) Stability Contract", () => {
  const wsRs = fs.readFileSync(WS_RS_PATH, "utf8");
  const bridgeJs = extractDvBridgeJs(wsRs);

  it("KVS-01 [Positive]: initializes setKrispPath with neural models and defaults currentKrispEnabled = true", () => {
    assert.match(
      bridgeJs,
      /if\s*\(typeof\s+dv\.setKrispPath\s*===\s*"function"\)\s*\{\s*dv\.setKrispPath\(/,
      "DV_BRIDGE_JS must initialize dv.setKrispPath when available",
    );
    assert.match(
      bridgeJs,
      /let\s+currentKrispEnabled\s*=\s*true;/,
      "currentKrispEnabled must default to true for neural noise cancellation",
    );
  });

  it("KVS-02 [Positive]: separates Krisp NC (noiseCancellation: true) from WebRTC VAD (vadUseKrisp: false)", () => {
    const { buildVoiceDspTransportOptions, buildSafeVadInputModeOptions } =
      loadKrispStabilityHelpers(bridgeJs);

    const dspOpts = buildVoiceDspTransportOptions(false, true, true);
    assert.equal(dspOpts.echoCancellation, true);
    assert.equal(dspOpts.noiseCancellation, true, "Krisp neural noiseCancellation must be enabled");
    assert.equal(dspOpts.noiseSuppression, false, "Legacy WebRTC noiseSuppression must be disabled when Krisp NC is on");
    assert.equal(dspOpts.automaticGainControl, true);

    const vadOpts = buildSafeVadInputModeOptions(false, -42, true);
    assert.equal(
      vadOpts.vadUseKrisp,
      false,
      "vadUseKrisp must strictly be false to prevent SIGSEGV in krispCreateVadInt16",
    );
    assert.equal(vadOpts.vadThreshold, -42);
    assert.equal(vadOpts.vadAutoThreshold, 3);
    assert.equal(vadOpts.vadLeading, 5);
    assert.equal(vadOpts.vadTrailing, 25);
  });

  it("KVS-03 [Negative]: DV_BRIDGE_JS never sets vadUseKrisp to true or binds it to currentKrispEnabled", () => {
    assert.doesNotMatch(
      bridgeJs,
      /vadUseKrisp\s*:\s*true/,
      "vadUseKrisp must never be set to true",
    );
    assert.doesNotMatch(
      bridgeJs,
      /vadUseKrisp\s*:[^\n,}]*currentKrispEnabled/,
      "vadUseKrisp must never depend on currentKrispEnabled",
    );
    const vadMatches = [...bridgeJs.matchAll(/vadUseKrisp\s*:\s*([a-zA-Z0-9_!()]+)/g)];
    assert.ok(vadMatches.length >= 3, "vadUseKrisp: false must be explicitly present across transport option blocks");
    for (const m of vadMatches) {
      assert.equal(m[1], "false", `Expected vadUseKrisp: false, found vadUseKrisp: ${m[1]}`);
    }
  });

  it("KVS-04 [Negative]: disabling Krisp switches to standard WebRTC noiseSuppression while keeping vadUseKrisp: false", () => {
    const { buildVoiceDspTransportOptions, buildSafeVadInputModeOptions } =
      loadKrispStabilityHelpers(bridgeJs);

    const dspOff = buildVoiceDspTransportOptions(false, false, false);
    assert.equal(dspOff.noiseCancellation, false);
    assert.equal(dspOff.noiseSuppression, true);
    assert.equal(dspOff.automaticGainControl, false);

    const vadManual = buildSafeVadInputModeOptions(false, -55, false);
    assert.equal(vadManual.vadUseKrisp, false);
    assert.equal(vadManual.vadAutoThreshold, -1);
    assert.equal(vadManual.vadThreshold, -55);
  });

  it("KVS-05 [Boundary]: clamps vadThreshold to [-85, -5] dBFS and volumes to valid bounds", () => {
    assert.match(
      bridgeJs,
      /Math\.max\(-85,\s*Math\.min\(-5,\s*Math\.round\(msg\.vadThreshold\)\)\)/,
      "vadThreshold must be clamped to [-85, -5] dBFS",
    );
    assert.match(
      bridgeJs,
      /Math\.max\(0,\s*Math\.min\(100,\s*msg\.inputVolume\)\)/,
      "inputVolume must be clamped to [0, 100]",
    );
    assert.match(
      bridgeJs,
      /Math\.max\(0,\s*Math\.min\(200,\s*msg\.outputVolume\)\)/,
      "outputVolume must be clamped to [0, 200]",
    );
  });

  it("KVS-06 [Error / SEGV Guard]: attachConnCallbacks wraps conn.setTransportOptions to force vadUseKrisp: false", () => {
    const { attachConnCallbacks } = loadKrispStabilityHelpers(bridgeJs);
    /** @type {Array<any>} */
    const recordedCalls = [];
    /** @type {Record<string, any>} */
    const mockConn = {
      setTransportOptions(/** @type {any} */ opts) {
        recordedCalls.push(opts);
      },
    };

    attachConnCallbacks(mockConn);
    assert.equal(mockConn.__krispVadGuarded, true, "conn must be marked with __krispVadGuarded");

    // Attempt to pass vadUseKrisp: true through conn.setTransportOptions
    mockConn.setTransportOptions({
      inputMode: 1,
      inputModeOptions: {
        vadThreshold: -40,
        vadAutoThreshold: 3,
        vadUseKrisp: true,
        vadLeading: 5,
        vadTrailing: 25,
      },
      fec: true,
    });

    assert.equal(recordedCalls.length, 1);
    assert.equal(
      recordedCalls[0].inputModeOptions.vadUseKrisp,
      false,
      "Guarded setTransportOptions must override vadUseKrisp: true with false to prevent Signal 11 SEGV",
    );
    assert.equal(recordedCalls[0].inputModeOptions.vadThreshold, -40);
    assert.equal(recordedCalls[0].fec, true);
  });

  it("KVS-07 [Empty / Null]: enforceSafeVadTransportOptions safely handles null, undefined, and non-VAD payloads", () => {
    const { enforceSafeVadTransportOptions } = loadKrispStabilityHelpers(bridgeJs);
    assert.equal(enforceSafeVadTransportOptions(null), null);
    assert.equal(enforceSafeVadTransportOptions(undefined), undefined);

    const videoOnly = { keyframeInterval: 1000, alwaysSendVideo: true };
    const out = enforceSafeVadTransportOptions(videoOnly);
    assert.deepEqual(out, { keyframeInterval: 1000, alwaysSendVideo: true });
  });

  it("KVS-08 [Loading / Ready]: cmd === 'ready' accepts optional msg.krisp and applies separated NC/VAD options", () => {
    assert.match(
      bridgeJs,
      /if\s*\(typeof\s+msg\.krisp\s*===\s*"boolean"\)\s*currentKrispEnabled\s*=\s*msg\.krisp;/,
      "ready handler must accept msg.krisp boolean state",
    );
  });

  it("KVS-09 [Stream Isolation]: stream worker disables Krisp NC, echoCancellation, AGC, and Krisp VAD", () => {
    const { buildVoiceDspTransportOptions, buildSafeVadInputModeOptions } =
      loadKrispStabilityHelpers(bridgeJs);

    const streamDsp = buildVoiceDspTransportOptions(true, true, true);
    assert.equal(streamDsp.echoCancellation, false);
    assert.equal(streamDsp.noiseCancellation, false);
    assert.equal(streamDsp.noiseSuppression, false);
    assert.equal(streamDsp.automaticGainControl, false);

    const streamVad = buildSafeVadInputModeOptions(true, -42, true);
    assert.equal(streamVad.vadUseKrisp, false);
    assert.equal(streamVad.vadThreshold, -60);
    assert.equal(streamVad.vadAutoThreshold, -1);
  });

  it("KVS-10 [Recovery / Loopback & State Toggle]: loopback and set_state preserve Krisp NC separation and vadUseKrisp: false", () => {
    const loopbackIdx = bridgeJs.indexOf('else if (cmd === "loopback")');
    assert.ok(loopbackIdx > 0, "loopback command handler must exist in DV_BRIDGE_JS");
    const loopbackBlock = bridgeJs.slice(loopbackIdx, loopbackIdx + 600);
    assert.match(
      loopbackBlock,
      /noiseCancellation:\s*Boolean\(currentKrispEnabled\)/,
      "loopback must pass noiseCancellation: Boolean(currentKrispEnabled)",
    );
    assert.match(
      loopbackBlock,
      /noiseSuppression:\s*!currentKrispEnabled/,
      "loopback must pass noiseSuppression: !currentKrispEnabled",
    );
    assert.match(
      loopbackBlock,
      /vadUseKrisp:\s*false/,
      "loopback must explicitly set vadUseKrisp: false",
    );
  });

  it("KVS-11 [Input Mode Switching: Voice Activity <-> Push-to-Talk]: switches between voice-activity and push-to-talk modes with threshold and delay clamping", () => {
    const doc = new ShimDocument();
    const host = doc.createElement("section");
    host.id = "voice-devices";
    doc.body.append(host);

    /** @type {Array<{ op: string, arg: any }>} */
    const recordedOps = [];
    const controller = createVoiceDevicesSettingsController({
      host: /** @type {any} */ (host),
      devices: [
        { id: "mic-usb", label: "USB Mic", kind: "input", isDefault: false },
        { id: "mic-default", label: "Default Mic", kind: "input", isDefault: true },
        { id: "spk-default", label: "Default Speaker", kind: "output", isDefault: true },
      ],
      service: {
        selectInput: (id) => recordedOps.push({ op: "selectInput", arg: id }),
        selectOutput: (id) => recordedOps.push({ op: "selectOutput", arg: id }),
        setInputVolume: (v) => recordedOps.push({ op: "setInputVolume", arg: v }),
        setOutputVolume: (v) => recordedOps.push({ op: "setOutputVolume", arg: v }),
        setInputMode: (m) => recordedOps.push({ op: "setInputMode", arg: m }),
        setUserMute: (m) => recordedOps.push({ op: "setUserMute", arg: m }),
        setSelfDeaf: (d) => recordedOps.push({ op: "setSelfDeaf", arg: d }),
        setPttKey: (k) => recordedOps.push({ op: "setPttKey", arg: k }),
      },
    });

    // Initial state must be voice-activity
    assert.equal(controller.getState().inputMode.kind, "voice-activity");
    const vadGroup = host.querySelector("#voice-devices-vad-settings");
    const pttGroup = host.querySelector("#voice-devices-ptt-settings");
    const pttToggle = host.querySelector("#voice-devices-ptt");
    assert.ok(vadGroup && pttGroup && pttToggle, "VAD and PTT DOM groups must exist");
    assert.equal(vadGroup.getAttribute("hidden"), null, "VAD settings visible in voice-activity mode");
    assert.notEqual(pttGroup.getAttribute("hidden"), null, "PTT settings hidden in voice-activity mode");

    // Switch to Push-to-Talk via setInputMode with releaseDelayMs clamping (e.g. 2500 -> 2000)
    controller.setInputMode({ kind: "push-to-talk", releaseDelayMs: 2500 });
    const pttState = controller.getState();
    assert.equal(pttState.inputMode.kind, "push-to-talk");
    if (pttState.inputMode.kind === "push-to-talk") {
      assert.equal(pttState.inputMode.releaseDelayMs, 2000, "releaseDelayMs must clamp to 2000 ms");
    }
    assert.equal(pttToggle.getAttribute("aria-checked"), "true");
    assert.notEqual(vadGroup.getAttribute("hidden"), null, "VAD settings hidden in push-to-talk mode");
    assert.equal(pttGroup.getAttribute("hidden"), null, "PTT settings visible in push-to-talk mode");

    // Switch back to Voice Activity via setInputMode with thresholdDb clamping (-120 -> -100)
    controller.setInputMode({ kind: "voice-activity", thresholdDb: -120 });
    const vadState = controller.getState();
    assert.equal(vadState.inputMode.kind, "voice-activity");
    if (vadState.inputMode.kind === "voice-activity") {
      assert.equal(vadState.inputMode.thresholdDb, -100, "thresholdDb must clamp to [-100, 0]");
    }
    assert.equal(pttToggle.getAttribute("aria-checked"), "false");
    assert.equal(vadGroup.getAttribute("hidden"), null);
    assert.notEqual(pttGroup.getAttribute("hidden"), null);

    const modeCalls = recordedOps.filter((x) => x.op === "setInputMode");
    assert.equal(modeCalls.length, 2);
    assert.equal(modeCalls[0]?.arg?.kind, "push-to-talk");
    assert.equal(modeCalls[1]?.arg?.kind, "voice-activity");

    controller.destroy();
  });

  it("KVS-12 [Transport Guard on Input Mode Switch]: conn.setTransportOptions forces vadUseKrisp: false across Voice Activity (inputMode: 1) and Push-to-Talk (inputMode: 2)", () => {
    const { attachConnCallbacks } = loadKrispStabilityHelpers(bridgeJs);
    /** @type {Array<any>} */
    const transportHistory = [];
    /** @type {Record<string, any>} */
    const mockConn = {
      setTransportOptions(/** @type {any} */ opts) {
        transportHistory.push(opts);
      },
    };
    attachConnCallbacks(mockConn);

    // 1. Voice Activity (inputMode: 1) with malicious vadUseKrisp: true
    mockConn.setTransportOptions({
      inputMode: 1,
      inputModeOptions: {
        vadThreshold: -45,
        vadAutoThreshold: 3,
        vadUseKrisp: true,
        vadLeading: 5,
        vadTrailing: 25,
      },
    });

    // 2. Switch to Push-to-Talk (inputMode: 2) with pttReleaseDelay and vadUseKrisp: true
    mockConn.setTransportOptions({
      inputMode: 2,
      inputModeOptions: {
        pttReleaseDelay: 180,
        vadThreshold: -45,
        vadUseKrisp: true,
      },
    });

    // 3. Switch back to Voice Activity (inputMode: 1) with manual threshold (-58 dB)
    mockConn.setTransportOptions({
      inputMode: 1,
      inputModeOptions: {
        vadThreshold: -58,
        vadAutoThreshold: -1,
        vadUseKrisp: true,
        vadLeading: 5,
        vadTrailing: 25,
      },
    });

    assert.equal(transportHistory.length, 3);
    assert.equal(transportHistory[0].inputMode, 1);
    assert.equal(transportHistory[0].inputModeOptions.vadUseKrisp, false);
    assert.equal(transportHistory[1].inputMode, 2);
    assert.equal(transportHistory[1].inputModeOptions.pttReleaseDelay, 180);
    assert.equal(transportHistory[1].inputModeOptions.vadUseKrisp, false);
    assert.equal(transportHistory[2].inputMode, 1);
    assert.equal(transportHistory[2].inputModeOptions.vadThreshold, -58);
    assert.equal(transportHistory[2].inputModeOptions.vadUseKrisp, false);
  });

  it("KVS-13 [Audio Device Switching & Hotplug Fallback]: switches input/output devices and falls back to default on hotplug without losing mode or volume state", () => {
    const doc = new ShimDocument();
    const host = doc.createElement("section");
    host.id = "voice-devices";
    doc.body.append(host);

    /** @type {Array<{ op: string, arg: any }>} */
    const serviceCalls = [];
    /** @type {Array<any>} */
    const deviceEvents = [];
    const controller = createVoiceDevicesSettings({
      host: /** @type {any} */ (host),
      devices: [
        { id: "mic-usb", label: "Система: DM30 USB Microphone", kind: "input", isDefault: false },
        { id: "mic-builtin", label: "Система: Built-in Analog Stereo", kind: "input", isDefault: true },
        { id: "spk-hdmi", label: "Система: HDMI Audio", kind: "output", isDefault: false },
        { id: "spk-headset", label: "Система: HyperX Virtual Surround", kind: "output", isDefault: true },
      ],
      service: {
        selectInput: (id) => serviceCalls.push({ op: "selectInput", arg: id }),
        selectOutput: (id) => serviceCalls.push({ op: "selectOutput", arg: id }),
        setInputVolume: (v) => serviceCalls.push({ op: "setInputVolume", arg: v }),
        setOutputVolume: (v) => serviceCalls.push({ op: "setOutputVolume", arg: v }),
        setInputMode: (m) => serviceCalls.push({ op: "setInputMode", arg: m }),
        setUserMute: (m) => serviceCalls.push({ op: "setUserMute", arg: m }),
        setSelfDeaf: (d) => serviceCalls.push({ op: "setSelfDeaf", arg: d }),
        setPttKey: (k) => serviceCalls.push({ op: "setPttKey", arg: k }),
      },
      onDeviceEvent: (ev) => deviceEvents.push(ev),
    });

    // Initially selects isDefault devices
    assert.equal(controller.getState().selectedInput, "mic-builtin");
    assert.equal(controller.getState().selectedOutput, "spk-headset");

    // Switch input to USB mic and output to HDMI, set custom mode & volumes
    controller.selectInput("mic-usb");
    controller.selectOutput("spk-hdmi");
    controller.setInputVolume(80);
    controller.setOutputVolume(145);
    controller.setInputMode({ kind: "push-to-talk", releaseDelayMs: 320 });

    assert.equal(controller.getState().selectedInput, "mic-usb");
    assert.equal(controller.getState().selectedOutput, "spk-hdmi");
    assert.equal(controller.getState().inputVolume, 80);
    assert.equal(controller.getState().outputVolume, 145);
    assert.equal(controller.getState().inputMode.kind, "push-to-talk");

    // Hotplug event: mic-usb is unplugged; controller must fallback to mic-builtin while preserving PTT & volumes
    controller.setDevices([
      { id: "mic-builtin", label: "Система: Built-in Analog Stereo", kind: "input", isDefault: true },
      { id: "spk-hdmi", label: "Система: HDMI Audio", kind: "output", isDefault: false },
      { id: "spk-headset", label: "Система: HyperX Virtual Surround", kind: "output", isDefault: true },
    ]);

    assert.equal(controller.getState().selectedInput, "mic-builtin");
    assert.equal(controller.getState().selectedOutput, "spk-hdmi");
    assert.equal(controller.getState().inputVolume, 80);
    assert.equal(controller.getState().outputVolume, 145);
    assert.equal(controller.getState().inputMode.kind, "push-to-talk");
    assert.equal(deviceEvents.length, 1);
    assert.equal(deviceEvents[0].type, "DeviceFallback");
    assert.equal(deviceEvents[0].previousId, "mic-usb");
    assert.equal(deviceEvents[0].fallbackId, "mic-builtin");

    controller.destroy();
  });

  it("KVS-14 [End-to-End DV_BRIDGE_JS Command Loop]: executes ready -> set_state -> loopback in VM and enforces vadUseKrisp: false on every call", () => {
    /** @type {((line: string) => void) | null} */
    let onLineCallback = null;
    /** @type {Array<Record<string, any>>} */
    const dvTransportCalls = [];
    /** @type {Array<Record<string, any>>} */
    const connTransportCalls = [];
    /** @type {Array<{ active: boolean, opts: Record<string, any> }>} */
    const loopbackCalls = [];
    /** @type {Array<number>} */
    const inputVolumes = [];
    /** @type {Array<number>} */
    const outputVolumes = [];
    /** @type {Array<string>} */
    const krispPaths = [];

    const mockDv = {
      setImageDataAllocator: () => {},
      initialize: () => {},
      setKrispPath: (/** @type {string} */ p) => krispPaths.push(p),
      setTransportOptions: (/** @type {Record<string, any>} */ opts) => dvTransportCalls.push(opts),
      setInputVolume: (/** @type {number} */ v) => inputVolumes.push(v),
      setOutputVolume: (/** @type {number} */ v) => outputVolumes.push(v),
      setLoopback: (/** @type {boolean} */ active, /** @type {Record<string, any>} */ opts) =>
        loopbackCalls.push({ active, opts }),
      VoiceConnection: function (
        /** @type {string} */ _userId,
        /** @type {Record<string, any>} */ _cfg,
        /** @type {(err: any, info: any) => void} */ cb,
      ) {
        const inst = {
          setTransportOptions: (/** @type {Record<string, any>} */ opts) => connTransportCalls.push(opts),
          setSelfMute: () => {},
          setSelfDeafen: () => {},
          setNoInputThreshold: () => {},
          setPingInterval: () => {},
          setPingCallback: () => {},
          configureConnectionRetries: () => {},
          destroy: () => {},
        };
        setImmediate(() => cb(null, { protocol: "udp", address: "127.0.0.1", port: 50001 }));
        return inst;
      },
    };

    const sandbox = {
      require: (/** @type {string} */ mod) => {
        if (mod === "fs") {
          return {
            mkdirSync: () => {},
            existsSync: () => false,
            readFileSync: () => Buffer.alloc(0),
            writeFileSync: () => {},
          };
        }
        if (mod === "http") {
          return {
            createServer: () => ({ on: () => {}, listen: () => {} }),
          };
        }
        if (mod === "readline") {
          return {
            createInterface: () => ({
              on: (/** @type {string} */ ev, /** @type {(line: string) => void} */ fn) => {
                if (ev === "line") onLineCallback = fn;
              },
            }),
          };
        }
        if (mod.includes("dv_scope_fix")) return { pushRemoteFrame: () => {} };
        if (mod.includes("discord_voice.node")) return mockDv;
        return {};
      },
      process: {
        env: {},
        stdin: {},
        stdout: { write: () => {} },
        exit: () => {},
      },
      Buffer,
      URL,
      Map,
      Set,
      Math,
      Number,
      String,
      Boolean,
      Array,
      Object,
      JSON,
      Date,
      setInterval: () => 1,
      clearInterval: () => {},
    };

    vm.createContext(sandbox);
    vm.runInContext(bridgeJs, sandbox);
    assert.equal(krispPaths.length, 1, "setKrispPath must be called once on startup");
    assert.ok(typeof onLineCallback === "function", "readline line handler must be registered");
    const sendCmd = /** @type {(line: string) => void} */ (onLineCallback);

    // 1. Send ready command with krisp: true
    sendCmd(
      JSON.stringify({
        cmd: "ready",
        userId: "384299970632876032",
        channelId: "1176785373989445703",
        ssrc: 1234,
        krisp: true,
        selfMute: false,
        selfDeafen: false,
      }),
    );

    return new Promise((resolve) => {
      setImmediate(() => {
        // After ready callback fires:
        assert.ok(dvTransportCalls.length >= 1, "dv.setTransportOptions called on ready");
        assert.equal(dvTransportCalls.at(-1)?.noiseCancellation, true);
        assert.equal(dvTransportCalls.at(-1)?.noiseSuppression, false);

        const readyVadCall = connTransportCalls.find((c) => c.inputModeOptions);
        assert.ok(readyVadCall, "conn.setTransportOptions with inputModeOptions called on ready");
        assert.equal(readyVadCall.inputModeOptions.vadUseKrisp, false);

        // 2. Toggle Krisp off + manual VAD sensitivity (-55 dB) via set_state
        sendCmd(
          JSON.stringify({
            cmd: "set_state",
            krisp: false,
            automaticGainControl: false,
            autoSensitivity: false,
            vadThreshold: -55,
            inputVolume: 75,
            outputVolume: 150,
          }),
        );

        assert.equal(dvTransportCalls.at(-1)?.noiseCancellation, false);
        assert.equal(dvTransportCalls.at(-1)?.noiseSuppression, true);
        assert.equal(dvTransportCalls.at(-1)?.automaticGainControl, false);

        const manualVadCalls = connTransportCalls.filter((c) => c.inputModeOptions);
        const latestVad = manualVadCalls.at(-1);
        assert.ok(latestVad);
        assert.equal(latestVad.inputModeOptions.vadUseKrisp, false);
        assert.equal(latestVad.inputModeOptions.vadThreshold, -55);
        assert.equal(latestVad.inputModeOptions.vadAutoThreshold, -1);

        // 3. Re-enable Krisp + auto sensitivity via set_state
        sendCmd(
          JSON.stringify({
            cmd: "set_state",
            krisp: true,
            automaticGainControl: true,
            autoSensitivity: true,
            vadThreshold: -40,
          }),
        );
        assert.equal(dvTransportCalls.at(-1)?.noiseCancellation, true);
        assert.equal(dvTransportCalls.at(-1)?.noiseSuppression, false);

        // 4. Trigger mic test loopback on and off
        sendCmd(JSON.stringify({ cmd: "loopback", active: true }));
        sendCmd(JSON.stringify({ cmd: "loopback", active: false }));
        assert.equal(loopbackCalls.length, 2);
        assert.equal(loopbackCalls[0]?.active, true);
        assert.equal(loopbackCalls[0]?.opts.noiseCancellation, true);
        assert.equal(loopbackCalls[0]?.opts.noiseSuppression, false);
        assert.equal(loopbackCalls[0]?.opts.vadUseKrisp, false);
        assert.equal(loopbackCalls[1]?.active, false);
        assert.equal(loopbackCalls[1]?.opts.vadUseKrisp, false);

        // Verify no call in connTransportCalls ever had vadUseKrisp !== false when inputModeOptions was present
        for (const call of connTransportCalls) {
          if (call.inputModeOptions) {
            assert.equal(
              call.inputModeOptions.vadUseKrisp,
              false,
              "Every conn.setTransportOptions call with inputModeOptions must have vadUseKrisp: false",
            );
          }
        }
        resolve();
      });
    });
  });

  it("KVS-15 [Backend BFF Audio Device & VAD State Contract]: data.rs set_system_audio_device routes device changes, Krisp, AGC, and VAD thresholds to native voice bridge", () => {
    const dataRs = fs.readFileSync(DATA_RS_PATH, "utf8");
    const fnIdx = dataRs.indexOf("pub fn set_system_audio_device(");
    const nextFnIdx = dataRs.indexOf("pub fn record_sent_message(", fnIdx);
    assert.ok(fnIdx > 0, "set_system_audio_device must exist in data.rs");
    const fnBody = dataRs.slice(fnIdx, nextFnIdx > fnIdx ? nextFnIdx : fnIdx + 40000);

    // Verifies Krisp, AGC, auto_sensitivity, vad_threshold handlers
    assert.match(fnBody, /if\s+kind\s*==\s*"krisp"/);
    assert.match(fnBody, /LOCAL_KRISP_ENABLED\.store\(enabled,\s*Ordering::SeqCst\)/);
    assert.match(fnBody, /if\s+kind\s*==\s*"agc"/);
    assert.match(fnBody, /if\s+kind\s*==\s*"auto_sensitivity"/);
    assert.match(fnBody, /if\s+kind\s*==\s*"vad_threshold"/);
    assert.match(fnBody, /\.clamp\(-85,\s*-5\)/);

    // Verifies input/output device selection and AUDIO_DEVICE_EPOCH increment
    assert.match(fnBody, /let\s+is_input\s*=\s*kind\s*==\s*"input"\s*\|\|\s*kind\s*==\s*"source"/);
    assert.match(fnBody, /selected_input_device\(\)\.lock\(\)/);
    assert.match(fnBody, /selected_output_device\(\)\.lock\(\)/);
    assert.match(fnBody, /AUDIO_DEVICE_EPOCH\.fetch_add\(1,\s*Ordering::SeqCst\)/);
  });

  it("KVS-16 [Live Settings Enhancer Parity]: live-settings-enhancer.js wires input/output device dropdowns, Krisp selector, VAD sensitivity, and PTT mode toggle", () => {
    const enhancerJs = fs.readFileSync(LIVE_SETTINGS_PATH, "utf8");

    // Device switching IPC calls
    assert.match(
      enhancerJs,
      /invoke\(VOICE_SET_AUDIO_DEVICE_IPC_CMD,\s*\{\s*kind:\s*"input",\s*deviceId:\s*id\s*\}\)/,
      "Mic dropdown must invoke voice_set_audio_device with kind: 'input'",
    );
    assert.match(
      enhancerJs,
      /invoke\(VOICE_SET_AUDIO_DEVICE_IPC_CMD,\s*\{\s*kind:\s*"output",\s*deviceId:\s*id\s*\}\)/,
      "Speaker dropdown must invoke voice_set_audio_device with kind: 'output'",
    );

    // Krisp dropdown & input profile switching
    assert.match(
      enhancerJs,
      /kind:\s*"krisp",\s*muted:\s*Boolean\(prefs\.krispEnabled\)/,
      "Noise cancellation dropdown must invoke voice_set_audio_device with kind: 'krisp'",
    );
    assert.match(
      enhancerJs,
      /kind:\s*"vad_threshold",\s*deviceId:\s*String\(manualVadDb\)/,
      "Manual VAD slider must invoke voice_set_audio_device with kind: 'vad_threshold'",
    );

    // Push-to-Talk switch & hotkey recording
    assert.match(
      enhancerJs,
      /findToggle\("voice\.ptt_mode"\)/,
      "PTT switch must bind to canonical voice.ptt_mode toggle",
    );
    assert.match(
      enhancerJs,
      /pttRecordBtn\.id\s*=\s*"live-ptt-record-btn"/,
      "PTT hotkey record button must exist in live settings enhancer",
    );
  });
});
