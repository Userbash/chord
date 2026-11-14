// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("Stream Broadcaster Handshake, Source Normalization & Format Parity", () => {
  const dataRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/data.rs"), "utf8");
  const mainRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/main.rs"), "utf8");
  const wsRs = fs.readFileSync(path.join(REPO_ROOT, "rust/crates/chord-core/src/ws.rs"), "utf8");

  it("SB1: data.rs reset_live_stream_state resets LOCAL_SELF_STREAM to false", () => {
    const fnIdx = dataRs.indexOf("pub fn reset_live_stream_state()");
    assert.ok(fnIdx > 0, "reset_live_stream_state must exist in data.rs");
    const fnBody = dataRs.slice(fnIdx, fnIdx + 600);
    assert.match(
      fnBody,
      /LOCAL_SELF_STREAM\.store\(false,\s*(?:std::sync::atomic::)?Ordering::[A-Za-z]+\)/,
      "reset_live_stream_state must reset LOCAL_SELF_STREAM to false",
    );
  });

  it("SB2: discord-core ws exposes clear_last_stream_key and is called on stream reset", () => {
    assert.match(
      wsRs,
      /pub\s+fn\s+clear_last_stream_key\(\)/,
      "ws.rs must expose clear_last_stream_key()",
    );
    assert.match(
      dataRs,
      /chord_core::ws::clear_last_stream_key\(\)/,
      "data.rs reset_live_stream_state must invoke clear_last_stream_key()",
    );
  });

  it("SB3: connect_or_update_live_voice_ext does not delay stream toggle with 120ms coalesce timer", () => {
    const fnIdx = dataRs.indexOf("pub fn connect_or_update_live_voice_ext(");
    assert.ok(fnIdx > 0, "connect_or_update_live_voice_ext must exist in data.rs");
    const fnBody = dataRs.slice(fnIdx, fnIdx + 18000);
    assert.match(
      fnBody,
      /stream_or_video_changed/,
      "connect_or_update_live_voice_ext must track stream_or_video_changed",
    );
    assert.match(
      fnBody,
      /!stream_or_video_changed\s*&&\s*!channel_changed|!\(stream_or_video_changed\s*\|\|\s*channel_changed\)/,
      "connect_or_update_live_voice_ext must bypass coalesce delay when self_stream or self_video changes",
    );
  });

  it("SB4: ws.rs normalizes sourceId for screen (extracting '0' from 'screen:0')", () => {
    assert.match(
      wsRs,
      /currentSourceId\s*=\s*t\.startsWith\("screen:"\)\s*\?\s*t\.slice\(7\)/,
      "ws.rs must normalize screen:0 to 0",
    );
  });

  it("SB5: ws.rs capture_kwin_dbus_to_ring checks KWin pixel format and uses CaptureWorkspace for entire screen", () => {
    assert.match(
      wsRs,
      /const\s+char\*\s+method\s*=\s*"CaptureWorkspace";/,
      "ws.rs must use CaptureWorkspace for full desktop capture",
    );
    assert.match(
      wsRs,
      /g_variant_lookup\(dict,\s*"format",\s*"u",\s*&src_format\);/,
      "ws.rs must inspect pixel format from KWin ScreenShot2",
    );
  });

  it("SB6: ws.rs clear_desktop_source handles stream stop in node bridge", () => {
    assert.match(
      wsRs,
      /clearDesktopSource/,
      "ws.rs bridge must support clearDesktopSource when stream stops",
    );
  });

  it("SB7: ws.rs applyScreenBroadcast and set_stream_source use kFullDesktopScreenId = '-1' and type: 'screen'", () => {
    const bcastIdx = wsRs.indexOf("function applyScreenBroadcast(");
    assert.ok(bcastIdx > 0, "applyScreenBroadcast must exist in ws.rs");
    const bcastBody = wsRs.slice(bcastIdx, bcastIdx + 3200);
    assert.match(
      bcastBody,
      /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"-1"/,
      "applyScreenBroadcast must pass type: 'screen', sourceId: '-1' (kFullDesktopScreenId) to setDesktopSourceWithOptions",
    );
    assert.doesNotMatch(
      bcastBody,
      /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"0"/,
      "applyScreenBroadcast must not pass sourceId: '0' which fails ScreenCapturerX11::SelectSource",
    );

    const switchIdx = wsRs.indexOf('else if (cmd === "set_stream_source")');
    assert.ok(switchIdx > 0, "set_stream_source handler must exist in ws.rs");
    const switchBody = wsRs.slice(switchIdx, switchIdx + 1200);
    assert.match(
      switchBody,
      /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"-1"/,
      "set_stream_source must pass type: 'screen', sourceId: '-1' (kFullDesktopScreenId) to setDesktopSourceWithOptions",
    );
    assert.doesNotMatch(
      switchBody,
      /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"0"/,
      "set_stream_source must not pass sourceId: '0' which fails ScreenCapturerX11::SelectSource",
    );
  });
});
