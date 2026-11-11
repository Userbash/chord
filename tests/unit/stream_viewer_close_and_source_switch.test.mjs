// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("Stream Viewer Close, Bottom Preview Teardown & Live Source Switch Parity", () => {
  const dataRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/data.rs"), "utf8");
  const wsRs = fs.readFileSync(path.join(REPO_ROOT, "rust/crates/discord-core/src/ws.rs"), "utf8");
  const appJs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/ui/app.js"), "utf8");

  it("SV1: ws.rs node runner always configures screen capturer -1 (kFullDesktopScreenId) for XShm hook parity on Wayland", () => {
    // Under Wayland, passing window GUID {...} or "0" directly to WebRTC SetScreenshareSourceLinux
    // causes "Screenshare: Failed to select source: 0" and "SetDesktopScreenshare: (nil)", resulting in 0 bytes sent.
    // The node runner must always configure type: "screen", sourceId: "-1" so ScreenCapturerX11::SelectSource(-1) succeeds
    // and calls hook_XShmGetImage, which dynamically captures either the window or workspace via KWin D-Bus.
    const fnIdx = wsRs.indexOf("function applyScreenBroadcast(");
    assert.ok(fnIdx > 0, "applyScreenBroadcast must exist in ws.rs");
    const fnBody = wsRs.slice(fnIdx, fnIdx + 3000);
    assert.match(
      fnBody,
      /type:\s*"screen",\s*sourceId:\s*"-1"/,
      "applyScreenBroadcast must pass type: 'screen', sourceId: '-1' to conn.setDesktopSourceWithOptions",
    );
  });

  it("SV2: data.rs set_system_audio_device parses raw window GUIDs without colon prefix", () => {
    const fnIdx = dataRs.indexOf("if kind == \"set_capture_source\"");
    assert.ok(fnIdx > 0, "set_capture_source must exist in data.rs");
    const fnBody = dataRs.slice(fnIdx, fnIdx + 1000);
    assert.match(
      fnBody,
      /raw\.starts_with\('\{'\)/,
      "set_capture_source must directly recognize raw window GUIDs starting with '{'",
    );
  });

  it("SV3: app.js openLiveStreamStageViewer provides separate stop button and close/minimize cross button", () => {
    const fnIdx = appJs.indexOf("function openLiveStreamStageViewer(");
    assert.ok(fnIdx > 0, "openLiveStreamStageViewer must exist in app.js");
    const fnBody = appJs.slice(fnIdx, fnIdx + 18000);
    assert.match(
      fnBody,
      /stopStageBtn/,
      "openLiveStreamStageViewer must have a dedicated stopStageBtn",
    );
    assert.match(
      fnBody,
      /closeStageBtn/,
      "openLiveStreamStageViewer must have a dedicated closeStageBtn for minimizing/closing viewer",
    );
    assert.match(
      fnBody,
      /rightControls\.append\(changeSourceBtn,\s*minimizeStageBtn,\s*stopStageBtn,\s*closeStageBtn\)/,
      "openLiveStreamStageViewer must append changeSourceBtn, minimizeStageBtn, stopStageBtn, and closeStageBtn to rightControls",
    );
  });

  it("SV4: app.js stopStreamBtn and disconnect clean up both live-stream-stage-viewer and live-stream-pip-preview without selector typo", () => {
    assert.ok(
      !appJs.includes('getElementById("live-stream-pip")') &&
        !appJs.includes("getElementById('live-stream-pip')"),
      "app.js must not contain the typo getElementById('live-stream-pip'); must use 'live-stream-pip-preview'",
    );
    const fnIdx = appJs.indexOf("function renderLiveVoicePanel()");
    assert.ok(fnIdx > 0, "renderLiveVoicePanel must exist in app.js");
    const fnBody = appJs.slice(fnIdx, fnIdx + 16000);
    assert.match(
      fnBody,
      /document\.getElementById\("live-stream-stage-viewer"\)\?\.remove\(\)/,
      "renderLiveVoicePanel stopStreamBtn must remove live-stream-stage-viewer from DOM",
    );
    assert.match(
      fnBody,
      /document\.getElementById\("live-stream-pip-preview"\)\?\.remove\(\)/,
      "renderLiveVoicePanel stopStreamBtn must remove live-stream-pip-preview from DOM",
    );
  });

  it("SV5: app.js supports live source switching without dropping stream connection across Stage Viewer, PIP, and Voice Panel", () => {
    assert.match(
      appJs,
      /switchLiveStreamSource/,
      "app.js must define switchLiveStreamSource for dynamic in-stream source changes",
    );
    assert.match(
      appJs,
      /stage-change-source-btn/,
      "Stage Viewer must expose #stage-change-source-btn during self stream",
    );
    assert.match(
      appJs,
      /pip-change-source-btn/,
      "PIP preview must expose #pip-change-source-btn during self stream",
    );
    assert.match(
      appJs,
      /voice-panel-change-source-btn/,
      "Voice panel must expose #voice-panel-change-source-btn during self stream",
    );
  });
});
