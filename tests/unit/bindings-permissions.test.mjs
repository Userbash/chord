// @ts-check
import assert from "node:assert/strict";
import test from "node:test";
import {
  LURKER_PERMISSIONS_MASK,
  PERM_ADD_REACTIONS,
  PERM_CONNECT,
  PERM_READ_MESSAGE_HISTORY,
  PERM_SEND_MESSAGES,
  PERM_VIEW_CHANNEL,
  createTypedCommands,
  evaluateChannelAccessDto,
  normalizeGuildDto,
} from "../../rust/gui-spike/ui/bindings.js";
import { computeGuildChannelTreeModel } from "../../rust/gui-spike/ui/components/channel-tree-view.js";

test("LURKER_PERMISSIONS_MASK matches Discord official mask (VIEW_CHANNEL | READ_MESSAGE_HISTORY = 66560)", () => {
  assert.equal(LURKER_PERMISSIONS_MASK, PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY);
  assert.equal(LURKER_PERMISSIONS_MASK, 66560n);
});

test("evaluateChannelAccessDto strips SEND_MESSAGES, ADD_REACTIONS, and CONNECT for lurker/preview guilds", () => {
  const everyonePerms = (
    PERM_VIEW_CHANNEL |
    PERM_READ_MESSAGE_HISTORY |
    PERM_SEND_MESSAGES |
    PERM_ADD_REACTIONS |
    PERM_CONNECT
  ).toString();

  const previewGuild = {
    id: "1014537851418980372",
    name: "ITSKIDON PC BOOST",
    isMember: false,
    lurker: true,
    my_roles: [],
    roles: [{ id: "1014537851418980372", name: "@everyone", permissions: everyonePerms }],
  };

  const textAccess = evaluateChannelAccessDto({
    guild: previewGuild,
    channel: { id: "1014537852002000908", type: 0, name: "поговорить-тут" },
    currentUserId: "1566799713523466370",
  });
  assert.equal(textAccess.canViewChannel, true);
  assert.equal(textAccess.canReadHistory, true);
  assert.equal(textAccess.canSendMessages, false);
  assert.equal(textAccess.canAddReactions, false);
  assert.equal(textAccess.readOnly, true);
  assert.equal(textAccess.isLurker, true);
  assert.match(String(textAccess.deniedReason), /Режим предпросмотра сервера/);

  const voiceAccess = evaluateChannelAccessDto({
    guild: previewGuild,
    channel: { id: "1039511139874242560", type: 2, name: "📞┃Пекарня" },
    currentUserId: "1566799713523466370",
  });
  assert.equal(voiceAccess.canViewChannel, true);
  assert.equal(voiceAccess.canConnectVoice, false);
});

test("computeGuildChannelTreeModel filters channels by role overwrites and reports preview state", () => {
  const guild = {
    id: "g1",
    name: "Test Server",
    isMember: true,
    lurker: false,
    myRoles: ["role_vip"],
    roles: [
      { id: "g1", name: "@everyone", permissions: (PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES).toString() },
      { id: "role_vip", name: "VIP", permissions: "0" },
    ],
  };

  const channels = [
    { id: "ch_public", name: "general", type: 0, permission_overwrites: [] },
    {
      id: "ch_vip",
      name: "vip-only",
      type: 0,
      permission_overwrites: [
        { id: "g1", type: 0, allow: "0", deny: PERM_VIEW_CHANNEL.toString() },
        { id: "role_vip", type: 0, allow: PERM_VIEW_CHANNEL.toString(), deny: "0" },
      ],
    },
    {
      id: "ch_staff",
      name: "staff-only",
      type: 0,
      permission_overwrites: [
        { id: "g1", type: 0, allow: "0", deny: PERM_VIEW_CHANNEL.toString() },
      ],
    },
  ];

  const model = computeGuildChannelTreeModel({
    guild,
    channels,
    currentUserId: "u1",
  });

  assert.equal(model.isPreviewMode, false);
  assert.deepEqual(
    model.visibleChannels.map((c) => c.channel.id),
    ["ch_public", "ch_vip"],
  );
});

test("createTypedCommands normalizes syncLiveState guilds via normalizeGuildDto", async () => {
  const cmds = createTypedCommands(async (cmd) => {
    assert.equal(cmd, "voice_set_audio_device");
    return {
      ok: true,
      live: true,
      currentUser: { id: "u1" },
      dmChannels: [],
      relationships: [],
      guilds: [{ id: "g1", name: "G1", is_member: false, lurker: true, my_roles: ["r1"] }],
      voiceStates: {},
      readStates: {},
      userGuildSettings: {},
    };
  });

  const snap = await cmds.syncLiveState();
  assert.equal(snap.ok, true);
  assert.equal(snap.guilds.length, 1);
  const firstGuild = snap.guilds[0];
  assert.ok(firstGuild);
  assert.equal(firstGuild.isMember, false);
  assert.equal(firstGuild.lurker, true);
  assert.deepEqual(firstGuild.myRoles, []);
  assert.ok(normalizeGuildDto(firstGuild));
});
