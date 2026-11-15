// @ts-check
/**
 * Contract tests for call and stream lifecycle:
 * - voice_stream_stop resilience (handles stream_key, streamKey, or auto-derivation)
 * - voice_stream_start permission gating (PERM_STREAM check)
 * - DM REST request routing for call/ring and call/stop-ringing
 * - hydrate_live_guild_blocking guild preservation (no dropped servers)
 * - GUILD_DELETE ignores unavailable: true
 * - openScreenShareModal checks canStreamInChannel
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const DATA_RS = readFileSync(
  new URL("../../rust/gui-spike/src/data.rs", import.meta.url),
  "utf8",
);
const MAIN_RS = readFileSync(
  new URL("../../rust/gui-spike/src/main.rs", import.meta.url),
  "utf8",
);
const VOICE_PLUGIN_RS = readFileSync(
  new URL("../../rust/gui-spike/src/voice_plugin.rs", import.meta.url),
  "utf8",
);
const APP_JS = readFileSync(
  new URL("../../rust/gui-spike/ui/app.js", import.meta.url),
  "utf8",
);

describe("Call and Stream Lifecycle & Guild Safety Contracts", () => {
  it("L1: voice_stream_stop accepts stream_key and streamKey, auto-derives if None", () => {
    const wrapper = MAIN_RS.indexOf("async fn voice_stream_stop(");
    assert.ok(wrapper > 0, "voice_stream_stop IPC wrapper must exist");
    const wrapperBody = MAIN_RS.slice(wrapper, MAIN_RS.indexOf("\n}", wrapper) + 2);
    assert.match(wrapperBody, /voice_plugin::voice_stream_stop\(stream_key, streamKey\)\.await/,
      "the public IPC wrapper must forward both supported argument aliases");
    const start = VOICE_PLUGIN_RS.indexOf("pub async fn voice_stream_stop(");
    assert.ok(start > 0, "voice plugin must implement voice_stream_stop");
    const body = VOICE_PLUGIN_RS.slice(start, VOICE_PLUGIN_RS.indexOf("\n}", start) + 2);
    assert.match(
      body,
      /current_live_voice_target/,
      "voice_stream_stop must auto-derive stream key from current_live_voice_target if not provided",
    );
    assert.match(body, /\.or\(streamKey\)/, "the plugin must accept the camelCase argument alias");
    assert.match(body, /\.unwrap_or_else\(/, "an absent key must enter the derivation fallback");
  });

  it("L2: voice_stream_start checks STREAM permission before initiating stream", () => {
    assert.match(
      DATA_RS,
      /pub fn can_user_stream_in_channel/,
      "data.rs must expose can_user_stream_in_channel",
    );
    const start = VOICE_PLUGIN_RS.indexOf("pub async fn voice_stream_start(");
    assert.ok(start > 0, "voice plugin must implement voice_stream_start");
    const body = VOICE_PLUGIN_RS.slice(start, VOICE_PLUGIN_RS.indexOf("\n}", start) + 2);
    assert.match(
      body,
      /if\s+!data::can_user_stream_in_channel\(/,
      "voice_stream_start must check can_user_stream_in_channel",
    );
    const permission = body.indexOf("data::can_user_stream_in_channel(");
    const connect = body.indexOf("data::connect_or_update_live_voice_ext(");
    assert.ok(connect > permission, "permission denial must be checked before requesting the stream");
  });

  it("L3: data.rs handles dm_rest_request in set_system_audio_device", () => {
    assert.match(
      DATA_RS,
      /kind\s*==\s*"dm_rest_request"/,
      "set_system_audio_device must handle kind == 'dm_rest_request'",
    );
    assert.match(
      DATA_RS,
      /handle_live_dm_rest_request/,
      "data.rs must implement handle_live_dm_rest_request for DM REST calls",
    );
  });

  it("L4: hydrate_live_guild_blocking never drops guilds from local_preview_guilds", () => {
    assert.match(
      DATA_RS,
      /local_preview_guilds\.remove\(&gid\);[\s\S]*?r_guilds\.push/,
      "hydrate_live_guild_blocking must ensure guild is pushed to r_guilds if removed from local_preview_guilds",
    );
  });

  it("L5: app.js GUILD_DELETE ignores unavailable: true events", () => {
    assert.match(
      APP_JS,
      /kind\s*===\s*"GUILD_DELETE"[\s\S]*?unavailable/,
      "app.js GUILD_DELETE handler must ignore events where unavailable is true",
    );
  });

  it("L6: app.js openScreenShareModal verifies canStreamInChannel", () => {
    assert.match(
      APP_JS,
      /openScreenShareModal[\s\S]*?canStreamInChannel/,
      "openScreenShareModal must check canStreamInChannel before opening modal",
    );
  });
});
