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

  it("SV1: ws.rs node runner always configures screen capturer 0 for XShm hook parity on Wayland", () => {
    // Under Wayland, passing window GUID {...} directly to WebRTC SetScreenshareSourceLinux
    // causes "Screenshare: Failed to select source: 0" and "SetScreenshare: (nil)", resulting in 0 bytes sent.
    // The node runner must always configure type: "screen", sourceId: "0" so ScreenCapturerX11 runs
    // and calls hook_XShmGetImage, which dynamically captures either the window or workspace via KWin D-Bus.
    const fnIdx = wsRs.indexOf("function applyScreenBroadcast(");
    assert.ok(fnIdx > 0, "applyScreenBroadcast must exist in ws.rs");
    const fnBody = wsRs.slice(fnIdx, fnIdx + 3000);
    assert.match(
      fnBody,
      /type:\s*"screen",\s*sourceId:\s*"0"/,
      "applyScreenBroadcast must pass type: 'screen', sourceId: '0' to conn.setDesktopSourceWithOptions",
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
  });

  it("SV4: app.js stopStreamBtn in voice panel removes live-stream-stage-viewer from DOM", () => {
    const fnIdx = appJs.indexOf("function renderLiveVoicePanel()");
    assert.ok(fnIdx > 0, "renderLiveVoicePanel must exist in app.js");
    const fnBody = appJs.slice(fnIdx, fnIdx + 16000);
    assert.match(
      fnBody,
      /document\.getElementById\("live-stream-stage-viewer"\)\?\.remove\(\)/,
      "renderLiveVoicePanel stopStreamBtn must remove live-stream-stage-viewer from DOM",
    );
  });

  it("SV5: app.js supports live source switching without dropping stream connection", () => {
    assert.match(
      appJs,
      /switchLiveStreamSource/,
      "app.js must define switchLiveStreamSource for dynamic in-stream source changes",
    );
  });
});
