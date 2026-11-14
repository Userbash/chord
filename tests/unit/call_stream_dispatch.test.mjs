// @ts-check
/**
 * Contract tests for CALL_CREATE, CALL_UPDATE, CALL_DELETE, STREAM_CREATE, STREAM_DELETE
 * dispatch handling, incoming call notification & join, and stream lifecycle.
 */
import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const NET_RS = readFileSync(
  new URL("../../rust/gui-spike/src/net.rs", import.meta.url),
  "utf8",
);
const CORE_WS_RS = readFileSync(
  new URL("../../rust/crates/chord-core/src/ws.rs", import.meta.url),
  "utf8",
);
const MAIN_RS = readFileSync(
  new URL("../../rust/gui-spike/src/main.rs", import.meta.url),
  "utf8",
);
const DATA_RS = readFileSync(
  new URL("../../rust/gui-spike/src/data.rs", import.meta.url),
  "utf8",
);
const VOICE_STAGE_RS = readFileSync(
  new URL("../../rust/gui-spike/src/voice_stage.rs", import.meta.url),
  "utf8",
);
const APP_JS = readFileSync(
  new URL("../../rust/gui-spike/ui/app.js", import.meta.url),
  "utf8",
);

describe("Call and Stream Protocol & UI Contracts", () => {
  it("C1: net.rs delivers STREAM_CREATE, STREAM_UPDATE, STREAM_DELETE to webview channel", () => {
    assert.match(
      NET_RS,
      /"STREAM_CREATE"\s*\|\s*"STREAM_UPDATE"\s*\|\s*"STREAM_DELETE"/,
      "net.rs must include STREAM_CREATE, STREAM_UPDATE, STREAM_DELETE in channel.deliver block",
    );
  });

  it("C2: discord-core ws.rs has queue_live_stream_create for OP 18", () => {
    assert.match(
      CORE_WS_RS,
      /pub fn queue_live_stream_create\s*\(/,
      "ws.rs must define pub fn queue_live_stream_create for Gateway OP 18 STREAM_CREATE",
    );
  });

  it("C3: ScreenShareSpec validates @me and null guild_id as call streams", () => {
    assert.match(
      VOICE_STAGE_RS,
      /@me/,
      "voice_stage.rs ScreenShareSpec must handle @me without snowflake error",
    );
  });

  it("C4: voice_stream_start queues OP 18 through the voice-state delegate and uses real self_uid", () => {
    const start = MAIN_RS.indexOf("async fn voice_stream_start(");
    assert.ok(start > 0, "voice_stream_start must exist");
    const startBody = MAIN_RS.slice(start, MAIN_RS.indexOf("\n}", start) + 2);
    assert.match(
      startBody,
      /data::connect_or_update_live_voice_ext\(gid, cid, vmute, vdeaf, vvideo, true\)/,
      "voice_stream_start must delegate with self_stream=true, without duplicating OP 18",
    );
    assert.match(startBody, /data::current_live_self_uid\(\)/, "stream key must use the actual user ID");
    const connect = DATA_RS.indexOf("pub fn connect_or_update_live_voice_ext(");
    assert.ok(connect > 0, "connect_or_update_live_voice_ext must exist");
    const connectBody = DATA_RS.slice(connect, DATA_RS.indexOf("\n}", connect) + 2);
    assert.match(
      connectBody,
      /\n    chord_core::ws::queue_live_voice_state_ext\(\s*clean_gid\.clone\(\),\s*clean_cid\.clone\(\),\s*self_mute,\s*self_deaf,\s*self_video,\s*self_stream,\s*Some\(self_uid\.as_str\(\)\),\s*\)/,
      "the immediate voice-state call must propagate self_stream and the actual user ID",
    );
    const queue = CORE_WS_RS.indexOf("pub fn queue_live_voice_state_ext(");
    assert.ok(queue > 0, "queue_live_voice_state_ext must exist");
    const queueBody = CORE_WS_RS.slice(queue, CORE_WS_RS.indexOf("\n}", queue) + 2);
    assert.match(queueBody, /if self_stream[\s\S]*EtfValue::Int\(18\)/, "self_stream must encode OP 18");
    assert.match(queueBody, /send_live_gateway_frame\(encode\(&stream_env\)\)/, "encoded OP 18 must reach Gateway");
  });

  it("C5: app.js onDispatch handles CALL_CREATE and CALL_DELETE", () => {
    assert.match(
      APP_JS,
      /kind\s*===\s*"CALL_CREATE"/,
      "app.js onDispatch must handle CALL_CREATE",
    );
    assert.match(
      APP_JS,
      /kind\s*===\s*"CALL_DELETE"/,
      "app.js onDispatch must handle CALL_DELETE",
    );
  });

  it("C6: app.js onDispatch handles STREAM_CREATE and STREAM_DELETE", () => {
    assert.match(
      APP_JS,
      /kind\s*===\s*"STREAM_CREATE"/,
      "app.js onDispatch must handle STREAM_CREATE",
    );
    assert.match(
      APP_JS,
      /kind\s*===\s*"STREAM_DELETE"/,
      "app.js onDispatch must handle STREAM_DELETE",
    );
  });

  it("C7: disconnectFromLiveVoiceChannel stops active screen share", () => {
    assert.match(
      APP_JS,
      /disconnectFromLiveVoiceChannel[\s\S]*?voice_stream_stop/,
      "disconnectFromLiveVoiceChannel must call voice_stream_stop when screen share is active",
    );
  });
});
