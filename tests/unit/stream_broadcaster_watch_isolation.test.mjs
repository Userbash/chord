// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("Stream Broadcaster vs Viewer Isolation & Signal Guard (TASK-770 / TASK-771)", () => {
  const appJsPath = path.join(REPO_ROOT, "rust/gui-spike/ui/app.js");
  const mainRsPath = path.join(REPO_ROOT, "rust/gui-spike/src/main.rs");
  const wsRsPath = path.join(REPO_ROOT, "rust/crates/discord-core/src/ws.rs");

  const appJs = fs.readFileSync(appJsPath, "utf8");
  const mainRs = fs.readFileSync(mainRsPath, "utf8");
  const wsRs = fs.readFileSync(wsRsPath, "utf8");

  it("SB1: openLiveStreamStageViewer NEVER invokes stream_watch for broadcasters (isSelfStream)", () => {
    // In openLiveStreamStageViewer, stream_watch must strictly be guarded by !isSelfStream
    const stageViewerIdx = appJs.indexOf("function openLiveStreamStageViewer");
    assert.ok(stageViewerIdx > 0, "openLiveStreamStageViewer must exist in app.js");

    const stageViewerSnippet = appJs.slice(stageViewerIdx, stageViewerIdx + 3000);

    // Verify that the stream_watch call is inside if (!isSelfStream)
    const pattern = /if\s*\(!isSelfStream\)\s*\{[\s\S]*?kind:\s*["']stream_watch["'][\s\S]*?\}/;
    assert.match(
      stageViewerSnippet,
      pattern,
      "stream_watch must be strictly enclosed inside if (!isSelfStream) block in openLiveStreamStageViewer",
    );
  });

  it("SB2: voice_stream_start in main.rs does not duplicate queue_live_stream_create when connect_or_update_live_voice_ext already sent it", () => {
    const streamStartIdx = mainRs.indexOf("async fn voice_stream_start");
    assert.ok(streamStartIdx > 0, "voice_stream_start must exist in main.rs");
    const streamStartSnippet = mainRs.slice(streamStartIdx, streamStartIdx + 3000);

    // Ensure queue_live_stream_create is not called redundantly after connect_or_update_live_voice_ext with self_stream=true
    const hasConnectExt = streamStartSnippet.includes("connect_or_update_live_voice_ext");
    assert.ok(hasConnectExt, "voice_stream_start must call connect_or_update_live_voice_ext");

    // Check that we don't have back-to-back queue_live_stream_create right after connect_or_update_live_voice_ext
    const duplicateCall = /connect_or_update_live_voice_ext[\s\S]*?queue_live_stream_create/;
    assert.doesNotMatch(
      streamStartSnippet,
      duplicateCall,
      "voice_stream_start must not call queue_live_stream_create immediately after connect_or_update_live_voice_ext(..., true)",
    );
  });

  it("SB3: ws.rs drains native_stream_cmd_mutex or shared queue so stream commands are not stalled", () => {
    const drainIdx = wsRs.lastIndexOf("native_stream_cmd_mutex()");
    assert.ok(drainIdx > 0, "native_stream_cmd_mutex must exist in ws.rs");

    // The drain logic must check native_stream_cmd_mutex even if is_stream_conn is false or drain both
    const loopSnippet = wsRs.slice(drainIdx - 400, drainIdx + 800);
    const drainsBothOrStream =
      loopSnippet.includes("native_stream_cmd_mutex()") &&
      (loopSnippet.includes("is_stream_conn") || loopSnippet.includes("cmds"));
    assert.ok(drainsBothOrStream, "stream commands must be drained during voice loop");
  });

  it("SB4: ws.rs applyScreenBroadcast runs for broadcasters on primary voice connection without requiring isStreamWorker", () => {
    const fnIdx = wsRs.indexOf("function applyScreenBroadcast(");
    assert.ok(fnIdx > 0, "applyScreenBroadcast must exist in ws.rs");
    const fnSnippet = wsRs.slice(fnIdx, fnIdx + 400);

    // applyScreenBroadcast must NOT be guarded by !isStreamWorker, which would block broadcasters on primary voice connection
    assert.doesNotMatch(
      fnSnippet,
      /if\s*\(!conn\s*\|\|\s*!isStreamWorker\)/,
      "applyScreenBroadcast must not require isStreamWorker because screen broadcaster runs on primary voice connection",
    );
  });
});
