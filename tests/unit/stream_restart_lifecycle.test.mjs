// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("Stream Restart Lifecycle & Broadcaster Teardown Contracts", () => {
  const dataRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/data.rs"), "utf8");
  const mainRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/main.rs"), "utf8");
  const appJs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/ui/app.js"), "utf8");

  it("SR1: data.rs defines reset_live_stream_state clearing stream tokens and stopping RTC", () => {
    assert.match(
      dataRs,
      /pub\s+fn\s+reset_live_stream_state\(\)/,
      "data.rs must expose reset_live_stream_state()",
    );
    assert.match(
      dataRs,
      /guard\.stream_launched_token\s*=\s*None/,
      "reset_live_stream_state must clear stream_launched_token",
    );
    assert.match(
      dataRs,
      /discord_core::ws::stop_live_stream_rtc_session\(\)/,
      "reset_live_stream_state must call stop_live_stream_rtc_session()",
    );
  });

  it("SR2: voice_stream_start and voice_stream_stop invoke data::reset_live_stream_state", () => {
    const startIdx = mainRs.indexOf("async fn voice_stream_start");
    assert.ok(startIdx > 0, "voice_stream_start must exist in main.rs");
    const startBody = mainRs.slice(startIdx, startIdx + 1800);
    assert.match(
      startBody,
      /data::reset_live_stream_state\(\)/,
      "voice_stream_start must call reset_live_stream_state before creating stream",
    );

    const stopIdx = mainRs.indexOf("async fn voice_stream_stop");
    assert.ok(stopIdx > 0, "voice_stream_stop must exist in main.rs");
    const stopBody = mainRs.slice(stopIdx, stopIdx + 1800);
    assert.match(
      stopBody,
      /data::reset_live_stream_state\(\)/,
      "voice_stream_stop must call reset_live_stream_state",
    );
  });

  it("SR3: voice_disconnect tears down active stream via delete queue and resets state", () => {
    const disconnIdx = mainRs.indexOf("async fn voice_disconnect");
    assert.ok(disconnIdx > 0, "voice_disconnect must exist in main.rs");
    const disconnBody = mainRs.slice(disconnIdx, disconnIdx + 1000);
    assert.match(
      disconnBody,
      /queue_live_stream_delete/,
      "voice_disconnect must delete active stream",
    );
    assert.match(
      disconnBody,
      /data::reset_live_stream_state\(\)/,
      "voice_disconnect must call reset_live_stream_state()",
    );
  });

  it("SR4: app.js stopStreamBtn in voice panel calls voice_stream_stop", () => {
    const btnPattern = /stopStreamBtn\.addEventListener\("click",\s*\(\)\s*=>\s*\{[\s\S]*?invoke\("voice_stream_stop"/;
    assert.match(
      appJs,
      btnPattern,
      "stopStreamBtn click handler in voice panel must call voice_stream_stop",
    );
  });

  it("SR5: app.js Stage Viewer separates 'Прекратить стрим' (stopStageBtn) from minimize '✕' (closeStageBtn) for broadcaster", () => {
    assert.match(
      appJs,
      /stopStageBtn\.textContent\s*=\s*"Прекратить стрим"/,
      "stopStageBtn must show 'Прекратить стрим' for broadcaster",
    );
    assert.match(
      appJs,
      /stopStageBtn\.addEventListener\("click",\s*\(\)\s*=>\s*\{\s*closeStageViewer\(true\);/,
      "stopStageBtn must invoke closeStageViewer(true) to stop stream",
    );
    assert.match(
      appJs,
      /closeStageBtn\.addEventListener\("click",\s*\(\)\s*=>\s*\{\s*closeStageViewer\(false\);/,
      "closeStageBtn for broadcaster must invoke closeStageViewer(false) to minimize into PIP",
    );
    const viewerClosePattern = /if\s*\(isSelfStream\)\s*\{[\s\S]*?if\s*\(stopStream\s*&&\s*activeVoiceConnection\)[\s\S]*?invoke\("voice_stream_stop"/;
    assert.match(
      appJs,
      viewerClosePattern,
      "closeStageViewer must invoke voice_stream_stop when isSelfStream and stopStream are true",
    );
  });
});
