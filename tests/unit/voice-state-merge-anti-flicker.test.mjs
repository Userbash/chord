// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";
import {
  isConfirmedVoiceStateRecord,
  applyVoiceStateUpdateEvent,
  computeConfirmedVoiceParticipants,
} from "../../rust/gui-spike/ui/modules/channel-tree-view.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");
const APP_JS_PATH = path.join(REPO_ROOT, "rust/gui-spike/ui/app.js");

/**
 * Извлекает и компилирует функции нормализации и бесконфликтного слияния голосовых состояний
 * из `rust/gui-spike/ui/app.js` в изолированном VM-контексте.
 *
 * @param {string} appJs
 */
function loadVoiceStateMergeHelpers(appJs) {
  const flagIdx = appJs.indexOf("function isTruthyVoiceFlag(");
  const cleanIdx = appJs.indexOf("function cleanDiscordStr(");
  const normIdx = appJs.indexOf("function normalizeVoiceGuildKey(");
  const mergeGuildIdx = appJs.indexOf("function mergeVoiceStatesForGuild(");
  const mergeSnapIdx = appJs.indexOf("function mergeVoiceStatesSnapshot(");
  const timerIdx = appJs.indexOf("function getVoiceChannelTimer(");

  assert.ok(flagIdx > 0, "isTruthyVoiceFlag must exist in app.js");
  assert.ok(cleanIdx > flagIdx, "cleanDiscordStr must exist in app.js");
  assert.ok(normIdx > cleanIdx, "normalizeVoiceGuildKey must exist in app.js");
  assert.ok(
    mergeGuildIdx > normIdx,
    "mergeVoiceStatesForGuild must be defined in app.js after normalizeVoiceGuildKey",
  );
  assert.ok(
    mergeSnapIdx > mergeGuildIdx,
    "mergeVoiceStatesSnapshot must be defined in app.js after mergeVoiceStatesForGuild",
  );
  assert.ok(timerIdx > mergeSnapIdx, "getVoiceChannelTimer must follow voice state merge helpers");

  const utilSnippet = appJs.slice(flagIdx, appJs.indexOf("function hydrateGuildMembersAndRoles("));
  const voiceMergeSnippet = appJs.slice(normIdx, timerIdx);

  /** @type {any} */
  const sandbox = {
    HOME_GUILD_ID: "@me",
    isConfirmedVoiceStateRecord,
    activeVoiceConnection: null,
    Map,
    Set,
    Array,
    Object,
    String,
    Boolean,
    Number,
  };
  vm.createContext(sandbox);
  vm.runInContext(`${utilSnippet}\n${voiceMergeSnippet}`, sandbox);

  return {
    /** @type {(gid: any) => string} */
    normalizeVoiceGuildKey: sandbox.normalizeVoiceGuildKey,
    /** @type {(map: Map<string, any[]>, gid: any, incoming: any[] | null | undefined) => any[]} */
    mergeVoiceStatesForGuild: sandbox.mergeVoiceStatesForGuild,
    /** @type {(map: Map<string, any[]>, rawVs: Record<string, any> | null | undefined) => string} */
    mergeVoiceStatesSnapshot: sandbox.mergeVoiceStatesSnapshot,
  };
}

describe("TASK-766: Conflict-Free Voice States Merge & Guild Key Normalization (Anti-Flicker)", () => {
  const appJs = fs.readFileSync(APP_JS_PATH, "utf8");

  it("VSM-01 [Negative / Anti-Flicker Invariant]: app.js never calls destructive liveVoiceStatesByGuild.clear()", () => {
    assert.doesNotMatch(
      appJs,
      /liveVoiceStatesByGuild\.clear\s*\(\s*\)/,
      "app.js must never call liveVoiceStatesByGuild.clear() which causes participant flickering",
    );

    const refreshIdx = appJs.indexOf("async function refreshLiveReadyVoiceAndUsers()");
    assert.ok(refreshIdx > 0, "refreshLiveReadyVoiceAndUsers must exist in app.js");
    const refreshBody = appJs.slice(refreshIdx, refreshIdx + 2200);
    assert.match(
      refreshBody,
      /mergeVoiceStatesSnapshot\s*\(\s*liveVoiceStatesByGuild\s*,\s*rawVs\s*\)/,
      "refreshLiveReadyVoiceAndUsers must delegate to mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, rawVs)",
    );
    assert.doesNotMatch(
      refreshBody,
      /liveVoiceStatesByGuild\.delete\s*\(\s*existingGid\s*\)/,
      "refreshLiveReadyVoiceAndUsers must not delete guild voice states when liveGuildsById is not yet hydrated",
    );
  });

  it("VSM-02 [Positive]: preserves real-time VOICE_STATE_UPDATE participants when async voice_states_sync returns partial/stale list", () => {
    const { mergeVoiceStatesSnapshot } = loadVoiceStateMergeHelpers(appJs);
    /** @type {Map<string, any[]>} */
    const liveVoiceStatesByGuild = new Map();

    // 1. Official Discord client user arrives via real-time VOICE_STATE_UPDATE
    applyVoiceStateUpdateEvent(liveVoiceStatesByGuild, {
      t: "VOICE_STATE_UPDATE",
      guild_id: "1056563701764857896",
      channel_id: "2056563701764857897",
      user_id: "200000000000000001",
      session_id: "sess-official-peer",
      self_mute: false,
      self_deaf: false,
      member: {
        nick: "DemoUser",
        roles: ["role-admin"],
        user: { id: "200000000000000001", username: "demouser", global_name: "DemoUser" },
      },
    });

    // 2. Async voice_states_sync returns an older snapshot that only has local user (or another user)
    mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, {
      "1056563701764857896": [
        {
          guild_id: "1056563701764857896",
          channel_id: "2056563701764857897",
          user_id: "1566799713523466370",
          session_id: "sess-local-self",
          self_mute: true,
          self_deaf: false,
        },
      ],
    });

    const participants = computeConfirmedVoiceParticipants({
      voiceStatesByGuild: liveVoiceStatesByGuild,
      guildId: "1056563701764857896",
      channelId: "2056563701764857897",
    });

    assert.deepEqual(
      participants.map((p) => p.userId).sort(),
      ["1566799713523466370", "200000000000000001"],
      "Both real-time VOICE_STATE_UPDATE participant and sync snapshot participant must be preserved without flickering",
    );
  });

  it("VSM-03 [Positive / Guild Key Normalization]: normalizes '', null, 'nil', '@me', and padded snowflakes into canonical guild keys", () => {
    const { normalizeVoiceGuildKey, mergeVoiceStatesSnapshot } = loadVoiceStateMergeHelpers(appJs);

    assert.equal(normalizeVoiceGuildKey(""), "@me");
    assert.equal(normalizeVoiceGuildKey(null), "@me");
    assert.equal(normalizeVoiceGuildKey(undefined), "@me");
    assert.equal(normalizeVoiceGuildKey("nil"), "@me");
    assert.equal(normalizeVoiceGuildKey("null"), "@me");
    assert.equal(normalizeVoiceGuildKey("@me"), "@me");
    assert.equal(normalizeVoiceGuildKey("  1056563701764857896  "), "1056563701764857896");

    /** @type {Map<string, any[]>} */
    const liveVoiceStatesByGuild = new Map();
    // Seed unnormalized keys "" and padded guild id
    liveVoiceStatesByGuild.set("", [
      { user_id: "u-dm-1", channel_id: "dm-call-1", confirmed: true },
    ]);
    liveVoiceStatesByGuild.set(" 1056563701764857896 ", [
      { user_id: "u-guild-1", channel_id: "vc-1", confirmed: true },
    ]);

    mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, {
      "@me": [{ user_id: "u-dm-2", channel_id: "dm-call-1", confirmed: true }],
      "1056563701764857896": [{ user_id: "u-guild-2", channel_id: "vc-1", confirmed: true }],
    });

    assert.equal(liveVoiceStatesByGuild.has(""), false, "Unnormalized empty key must be removed");
    assert.equal(
      liveVoiceStatesByGuild.has(" 1056563701764857896 "),
      false,
      "Whitespace-padded guild key must be removed",
    );

    const dmUsers = (liveVoiceStatesByGuild.get("@me") ?? []).map((v) => v.user_id).sort();
    assert.deepEqual(dmUsers, ["u-dm-1", "u-dm-2"]);

    const guildUsers = (liveVoiceStatesByGuild.get("1056563701764857896") ?? [])
      .map((v) => v.user_id)
      .sort();
    assert.deepEqual(guildUsers, ["u-guild-1", "u-guild-2"]);
  });

  it("VSM-04 [Access / Permissions & Deep Merge]: preserves member.roles, member.nick, and user profile when merging partial updates", () => {
    const { mergeVoiceStatesForGuild } = loadVoiceStateMergeHelpers(appJs);
    /** @type {Map<string, any[]>} */
    const liveVoiceStatesByGuild = new Map();

    // Initial state has rich member & role metadata from READY / GUILD_CREATE
    mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "g-100", [
      {
        user_id: "u-1",
        channel_id: "vc-1",
        self_mute: false,
        self_stream: false,
        member: {
          nick: "Captain",
          roles: ["r-mod", "r-vip"],
          user: { id: "u-1", username: "captain_user", avatar: "av_hash_1" },
        },
      },
    ]);

    // Subsequent update changes self_mute and self_stream but omits member.roles and member.nick
    const merged = mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "g-100", [
      {
        userId: "u-1",
        channelId: "vc-1",
        selfMute: true,
        selfStream: true,
        member: {
          user: { id: "u-1", global_name: "Captain Global" },
        },
      },
    ]);

    assert.equal(merged.length, 1);
    assert.equal(merged[0].self_mute, true);
    assert.equal(merged[0].self_stream, true);
    assert.equal(merged[0].member.nick, "Captain", "Nickname must be preserved across partial merge");
    assert.deepEqual(
      merged[0].member.roles,
      ["r-mod", "r-vip"],
      "Member roles must be preserved across partial merge",
    );
    assert.equal(merged[0].member.user.avatar, "av_hash_1");
    assert.equal(merged[0].member.user.global_name, "Captain Global");
  });

  it("VSM-05 [Negative]: ignores unconfirmed/optimistic/signaling records and removes explicitly disconnected/timed-out users", () => {
    const { mergeVoiceStatesForGuild } = loadVoiceStateMergeHelpers(appJs);
    /** @type {Map<string, any[]>} */
    const liveVoiceStatesByGuild = new Map();

    mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "g-1", [
      { user_id: "u-active", channel_id: "vc-1", confirmed: true },
      { user_id: "u-leaving", channel_id: "vc-1", confirmed: true },
      { user_id: "u-timeout", channel_id: "vc-1", confirmed: true },
    ]);

    const afterUpdate = mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "g-1", [
      // Unconfirmed / optimistic phantom candidate must be ignored
      { user_id: "u-phantom", channel_id: "vc-1", confirmed: false, optimistic: true },
      { user_id: "u-signaling", channel_id: "vc-1", phase: "signaling" },
      // Explicit leave (channel_id: null) must remove u-leaving
      { user_id: "u-leaving", channel_id: null },
      // Timeout must remove u-timeout
      { user_id: "u-timeout", channel_id: "vc-1", phase: "timeout" },
    ]);

    assert.deepEqual(
      afterUpdate.map((v) => v.user_id),
      ["u-active"],
      "Only confirmed active participant must remain; phantom candidates ignored and disconnected/timed-out removed",
    );
  });

  it("VSM-06 [Boundary]: handles channel switch within same guild without duplicate user entries and supports Snowflake boundaries", () => {
    const { mergeVoiceStatesForGuild } = loadVoiceStateMergeHelpers(appJs);
    /** @type {Map<string, any[]>} */
    const liveVoiceStatesByGuild = new Map();

    mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "18446744073709551615", [
      { user_id: "0", channel_id: "vc-room-a", self_mute: false },
      { user_id: "18446744073709551615", channel_id: "vc-room-a", self_mute: true },
    ]);

    // User 18446744073709551615 moves from vc-room-a to vc-room-b
    const updated = mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "18446744073709551615", [
      { user_id: "18446744073709551615", channel_id: "vc-room-b", self_mute: false },
    ]);

    assert.equal(updated.length, 2, "Moving channels must not duplicate user in guild voice states");
    const movedUser = updated.find((v) => v.user_id === "18446744073709551615");
    assert.ok(movedUser);
    assert.equal(movedUser.channel_id, "vc-room-b");
    assert.equal(movedUser.self_mute, false);
  });

  it("VSM-07 [Empty & Error Resilience]: empty sync payload {} or [] and malformed entries do not wipe confirmed participants", () => {
    const { mergeVoiceStatesForGuild, mergeVoiceStatesSnapshot } = loadVoiceStateMergeHelpers(appJs);
    /** @type {Map<string, any[]>} */
    const liveVoiceStatesByGuild = new Map();

    mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "g-resilient", [
      { user_id: "u-keep", channel_id: "vc-keep", self_stream: true },
    ]);

    // Empty array for guild and malformed items
    mergeVoiceStatesForGuild(liveVoiceStatesByGuild, "g-resilient", [
      null,
      undefined,
      /** @type {any} */ ("invalid"),
      {},
    ]);
    // Empty snapshot object and null snapshot
    mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, {});
    mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, null);
    mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, { "g-resilient": [] });

    const remaining = liveVoiceStatesByGuild.get("g-resilient") ?? [];
    assert.equal(remaining.length, 1, "Confirmed participant must survive empty and malformed syncs");
    assert.equal(remaining[0].user_id, "u-keep");
    assert.equal(remaining[0].channel_id, "vc-keep");
    assert.equal(remaining[0].self_stream, true);
  });

  it("VSM-08 [Observability & Digest Determinism]: mergeVoiceStatesSnapshot produces identical digest regardless of key insertion order", () => {
    const { mergeVoiceStatesSnapshot } = loadVoiceStateMergeHelpers(appJs);
    /** @type {Map<string, any[]>} */
    const mapA = new Map();
    /** @type {Map<string, any[]>} */
    const mapB = new Map();

    const digestA = mergeVoiceStatesSnapshot(mapA, {
      "200": [{ user_id: "u2", channel_id: "c2", self_mute: true }],
      "100": [{ user_id: "u1", channel_id: "c1", self_mute: false }],
    });
    const digestB = mergeVoiceStatesSnapshot(mapB, {
      "100": [{ user_id: "u1", channel_id: "c1", self_mute: false }],
      "200": [{ user_id: "u2", channel_id: "c2", self_mute: true }],
    });

    assert.equal(digestA, digestB, "Digest must be deterministic across guild key ordering");
    assert.ok(digestA.includes("100:u1:c1:false:false:false:false:false"));
    assert.ok(digestA.includes("200:u2:c2:true:false:false:false:false"));
  });

  it("VSM-09 [Loading & Recovery]: hydrateGuildMembersAndRoles and onReadyPayload merge voice_states without destructive clear", () => {
    const hydrateIdx = appJs.indexOf("function hydrateGuildMembersAndRoles(");
    const resolveRoleIdx = appJs.indexOf("function resolveUserRoleInfo(");
    assert.ok(hydrateIdx > 0 && resolveRoleIdx > hydrateIdx, "hydrateGuildMembersAndRoles must exist in app.js");
    const hydrateBody = appJs.slice(hydrateIdx, resolveRoleIdx);
    assert.match(
      hydrateBody,
      /mergeVoiceStatesForGuild\s*\(\s*liveVoiceStatesByGuild\s*,\s*gId\s*,\s*rawGuildVs\s*\)/,
      "hydrateGuildMembersAndRoles must merge guild voice_states via mergeVoiceStatesForGuild",
    );

    const readyIdx = appJs.indexOf("onReadyPayload: (payload) => {");
    assert.ok(readyIdx > 0, "onReadyPayload must exist in app.js");
    const readyBody = appJs.slice(readyIdx, readyIdx + 2400);
    assert.doesNotMatch(
      readyBody,
      /liveVoiceStatesByGuild\.clear\s*\(\s*\)/,
      "onReadyPayload must not call liveVoiceStatesByGuild.clear()",
    );
    assert.match(
      readyBody,
      /mergeVoiceStatesSnapshot\s*\(\s*liveVoiceStatesByGuild\s*,\s*rawVs\s*\)/,
      "onReadyPayload must merge rawVs via mergeVoiceStatesSnapshot",
    );
  });
});
