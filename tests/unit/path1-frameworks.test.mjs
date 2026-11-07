// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import {
  normalizeGuildPayload,
  computeGuildChannelPermissions,
  canViewGuildChannel,
  canSendInGuildChannel,
  filterVisibleGuildChannels,
  resolveMemberGroupRoleTitle,
  createTypedIpcClient,
} from "../../rust/gui-spike/ui/features/ipc-bindings.js";
import { createVirtualizer } from "../../rust/gui-spike/ui/features/virtualizer.js";
import { computeFloatingPosition } from "../../rust/gui-spike/ui/features/floating-popover.js";
import {
  createGuildRailComponent,
  createChannelTreeComponent,
  createDmCallStageComponent,
  createMessageListComponent,
  createInviteModalComponent,
} from "../../rust/gui-spike/ui/features/reactive-components.js";
import { createDiscordReactiveStore } from "../../rust/gui-spike/ui/features/reactive-store.js";

test("ipc-bindings normalizes snake_case and camelCase guild/channel/role payloads and preserves myRoles across syncs", () => {
  const initial = normalizeGuildPayload({
    id: "1014537851418980372",
    properties: { name: "ITSKIDON PC BOOST", owner_id: "999" },
    my_roles: ["1057598992449552425"],
    roles: [
      { id: "1014537851418980372", name: "@everyone", position: 0, permissions: "1071698529857" },
      { id: "1057598992449552425", name: "Модератор", position: 28, color: 3447003, permissions: "1071698529857" },
    ],
    channels: [
      {
        id: "1163408591315812392",
        guild_id: "1014537851418980372",
        name: "новости",
        type: 0,
        permission_overwrites: [{ id: "1014537851418980372", type: 0, allow: "0", deny: "2048" }],
      },
    ],
  });

  assert.equal(initial.id, "1014537851418980372");
  assert.equal(initial.name, "ITSKIDON PC BOOST");
  assert.deepEqual(initial.myRoles, ["1057598992449552425"]);
  assert.deepEqual(initial.my_roles, ["1057598992449552425"]);
  assert.equal(initial.channels[0].guildId, "1014537851418980372");
  assert.equal(initial.channels[0].permissionOverwrites.length, 1);

  // Partial sync without my_roles/roles/channels must NOT wipe existing roles or myRoles
  const merged = normalizeGuildPayload(
    { id: "1014537851418980372", name: "ITSKIDON PC BOOST" },
    initial,
  );
  assert.deepEqual(merged.myRoles, ["1057598992449552425"]);
  assert.equal(merged.roles.length, 2);
  assert.equal(merged.channels.length, 1);
});

test("ipc-bindings computes role-based channel visibility and read-only SEND_MESSAGES permissions accurately", () => {
  const guild = normalizeGuildPayload({
    id: "100",
    owner_id: "999",
    roles: [
      { id: "100", name: "@everyone", position: 0, permissions: "3072" }, // VIEW_CHANNEL (1024) | SEND_MESSAGES (2048)
      { id: "101", name: "Участник", position: 1, permissions: "3072" },
      { id: "102", name: "Модератор", position: 2, permissions: "3072" },
    ],
    my_roles: ["101"],
  });

  const channels = [
    { id: "c_cat_pub", type: 4, name: "ОБЩЕНИЕ", position: 0, permission_overwrites: [] },
    {
      id: "c_rules",
      type: 0,
      parent_id: "c_cat_pub",
      name: "правила",
      position: 0,
      permission_overwrites: [{ id: "100", type: 0, allow: "0", deny: "2048" }],
    },
    {
      id: "c_general",
      type: 0,
      parent_id: "c_cat_pub",
      name: "основной",
      position: 1,
      permission_overwrites: [],
    },
    {
      id: "c_cat_staff",
      type: 4,
      name: "ПЕРСОНАЛ",
      position: 1,
      permission_overwrites: [
        { id: "100", type: 0, allow: "0", deny: "1024" },
        { id: "102", type: 0, allow: "1024", deny: "0" },
      ],
    },
    {
      id: "c_mod",
      type: 0,
      parent_id: "c_cat_staff",
      name: "модераторская",
      position: 0,
      permission_overwrites: [
        { id: "100", type: 0, allow: "0", deny: "1024" },
        { id: "102", type: 0, allow: "1024", deny: "0" },
      ],
    },
  ];

  // As regular member (role 101): sees c_cat_pub, c_rules (read-only), c_general (writable); does NOT see c_cat_staff or c_mod
  assert.equal(canViewGuildChannel(guild, channels[1], channels, "555"), true);
  assert.equal(canSendInGuildChannel(guild, channels[1], channels, "555"), false);
  assert.equal(canSendInGuildChannel(guild, channels[2], channels, "555"), true);
  assert.equal(canViewGuildChannel(guild, channels[4], channels, "555"), false);

  const visibleForMember = filterVisibleGuildChannels(guild, channels, "555");
  assert.deepEqual(
    visibleForMember.map((c) => c.id),
    ["c_cat_pub", "c_rules", "c_general"],
  );

  // As Moderator (role 102): also sees c_cat_staff and c_mod
  const modGuild = normalizeGuildPayload({ ...guild, myRoles: ["101", "102"] });
  const visibleForMod = filterVisibleGuildChannels(modGuild, channels, "555");
  assert.deepEqual(
    visibleForMod.map((c) => c.id),
    ["c_cat_pub", "c_rules", "c_general", "c_cat_staff", "c_mod"],
  );
  assert.equal(
    computeGuildChannelPermissions(modGuild, channels[4], channels, "555") !== 0n,
    true,
  );
});

test("ipc-bindings resolveMemberGroupRoleTitle never leaks raw Snowflake IDs", () => {
  const roles = [{ id: "1057598992449552425", name: "Модератор" }];
  assert.equal(
    resolveMemberGroupRoleTitle("1057598992449552425", null, roles, []),
    "Модератор",
  );
  assert.equal(
    resolveMemberGroupRoleTitle("999999999999999999", "999999999999999999", [], []),
    "Участники",
  );
  assert.equal(resolveMemberGroupRoleTitle("online", "", [], []), "В сети");
});

test("ipc-bindings createTypedIpcClient wraps invoke and normalizes responses", async () => {
  /** @type {Array<{ cmd: string, args: any }>} */
  const calls = [];
  const client = createTypedIpcClient(async (cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "resolve_invite") {
      return {
        code: "eSrxp2YGUC",
        guild: { id: "1014537851418980372", name: "ITSKIDON PC BOOST" },
        channel: { id: "1014537851867775060", name: "📲┃основной", type: 0 },
      };
    }
    return { ok: true };
  });

  const inv = await client.commands.resolveInvite("https://discord.gg/eSrxp2YGUC");
  assert.equal(calls[0]?.cmd, "resolve_invite");
  assert.equal(inv.code, "eSrxp2YGUC");
  assert.equal(inv.guild?.id, "1014537851418980372");
});

test("virtualizer computes visible items, total size, variable row measurements, and scrollToIndex", () => {
  const v = createVirtualizer({
    count: 1000,
    estimateSize: () => 44,
    overscan: 3,
    scrollOffset: 440,
    viewportSize: 220,
  });

  assert.equal(v.getTotalSize(), 44000);
  const items = v.getVirtualItems();
  // Visible range at offset 440..660 is indices 10..14, with overscan 3 -> 7..17
  assert.equal(items[0]?.index, 7);
  assert.equal(items[items.length - 1]?.index, 17);

  v.measureElement(10, 88);
  assert.equal(v.getTotalSize(), 44044);

  const offsetFor20 = v.scrollToIndex(20, { align: "start" });
  assert.equal(offsetFor20, 20 * 44 + 44);
});

test("floating-popover flips and shifts popovers to stay within viewport bounds", () => {
  const pos = computeFloatingPosition(
    { x: 1220, y: 690, width: 40, height: 24 },
    { width: 240, height: 180 },
    {
      placement: "bottom-start",
      offset: 8,
      padding: 12,
      viewport: { width: 1280, height: 720 },
    },
  );

  // Should flip to top because 690 + 24 + 8 + 180 > 720, and shift left so x + 240 <= 1280 - 12
  assert.equal(pos.placement.startsWith("top"), true);
  assert.equal(pos.x + 240 <= 1280 - 12, true);
  assert.equal(pos.y >= 12, true);
});

test("reactive-components declaratively render GuildRail, ChannelTree, DmCallStage, MessageList, and InviteModal from store signals", () => {
  const store = createDiscordReactiveStore();
  store.hydrateReady({
    user: { id: "555", username: "rustery" },
    guilds: [
      {
        id: "100",
        name: "Rust Server",
        my_roles: ["101"],
        roles: [
          { id: "100", name: "@everyone", position: 0, permissions: "3072" },
          { id: "101", name: "Участник", position: 1, permissions: "3072" },
        ],
      },
    ],
    channels: [
      { id: "c1", guild_id: "100", name: "general", type: 0, position: 0, permission_overwrites: [] },
      {
        id: "c_secret",
        guild_id: "100",
        name: "staff-only",
        type: 0,
        position: 1,
        permission_overwrites: [{ id: "100", type: 0, allow: "0", deny: "1024" }],
      },
    ],
    voiceStates: {
      "@me": [{ user_id: "555", channel_id: "dm_1", self_video: true }],
    },
  });

  const railComp = createGuildRailComponent(store);
  assert.equal(railComp.getSnapshot().items.length, 1);
  assert.equal(railComp.getSnapshot().items[0]?.name, "Rust Server");

  store.selectGuild("100", "c1");
  const treeComp = createChannelTreeComponent(store);
  // Only c1 should be visible because c_secret denies VIEW_CHANNEL (1024)
  assert.deepEqual(
    treeComp.getSnapshot().visibleChannels.map((/** @type {any} */ c) => c.id),
    ["c1"],
  );

  store.selectGuild("@me", "dm_1");
  const callComp = createDmCallStageComponent(store);
  assert.equal(callComp.getSnapshot().visible, true);
  assert.equal(callComp.getSnapshot().participants.length, 1);

  store.AppendOrUpdateMessage("dm_1", { id: "m1", content: "Привет!", author: { id: "555", username: "rustery" } });
  const msgComp = createMessageListComponent(store, { viewportSize: 300, itemSize: 44 });
  assert.equal(msgComp.getSnapshot().totalMessages, 1);

  const inviteComp = createInviteModalComponent({
    code: "eSrxp2YGUC",
    guildName: "ITSKIDON PC BOOST",
    channelName: "📲┃основной",
    onlineCount: 1467,
    memberCount: 24199,
    joined: true,
  });
  assert.equal(inviteComp.getSnapshot().ctaLabel, "Открыть сервер");
});
