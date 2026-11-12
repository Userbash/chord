// @ts-check
/**
 * TASK-113 / N1-07 (RW-40): поведенческие unit-тесты UI-модуля личных сообщений и друзей.
 *
 * Независимые оракулы:
 * - `fixtures/gateway/by-type/002-READY.json` (read-only: 17 `private_channels`,
 *   60 `relationships` [18 друзей type=1, 42 блокировки type=2], 146 `users`);
 * - `waves/RW-40/DM-FRIENDS-RE.md` (§0–§12: 24 REST-маршрута, `POST /users/@me/channels`
 *   всегда с `{recipients:[...]}`, 0 вызовов `GET /users/@me/channels`, Pomelo
 *   `discriminator: null`, код `80013` stranger confirmation, 7 Gateway-диспатчей);
 * - `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (§1–§6: `#home-rail-btn` с `guildId='@me'`,
 *   дерево ЛС `#dm-channel-list`, вкладки друзей `online/all/pending/blocked/add_friend`
 *   с WAI-ARIA `tablist/tab/tabpanel` и roving `tabindex` `ArrowLeft`/`ArrowRight`,
 *   модалка `#group-dm-dialog` до 9 друзей = 10 участников, диалог `#stranger-confirm-dialog`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  HOME_GUILD_ID,
  PRIVATE_CHANNEL_TYPES,
  CHANNEL_FLAGS,
  RELATIONSHIP_TYPES,
  RELATIONSHIP_FILTERS,
  FRIENDS_TABS,
  MAX_GROUP_DM_RECIPIENTS,
  MAX_GROUP_DM_PARTICIPANTS,
  DM_FRIENDS_ERROR_CODES,
  DM_FRIENDS_ENDPOINTS,
  parseEtfBool,
  parseSnowflakeBigInt,
  buildUserMap,
  resolveUserDisplayName,
  resolveUserTag,
  resolveUserAvatar,
  formatParticipantsSubtext,
  hydratePrivateChannel,
  sortPrivateChannels,
  normalizeRelationship,
  classifyRelationships,
  filterRelationshipsByTab,
  parseFriendRequestInput,
  normalizeRecipientsPayload,
  validateNoteResponse,
  applyDmFriendsDispatch,
  createDmFriendsController,
  insertHomeRailButton,
} from "../../rust/gui-spike/ui/features/dm-friends.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const rootDir = new URL("../../", import.meta.url);
const readyPath = fileURLToPath(
  new URL("fixtures/gateway/by-type/002-READY.json", rootDir),
);
const specPath = fileURLToPath(
  new URL("waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md", rootDir),
);
const rePath = fileURLToPath(
  new URL("waves/RW-40/DM-FRIENDS-RE.md", rootDir),
);

const READY = JSON.parse(readFileSync(readyPath, "utf8"));
const SPEC_TEXT = readFileSync(specPath, "utf8");
const RE_TEXT = readFileSync(rePath, "utf8");

/** @type {any[]} */
const READY_PRIVATE_CHANNELS = READY?.d?.private_channels ?? [];
/** @type {any[]} */
const READY_RELATIONSHIPS = READY?.d?.relationships ?? [];
/** @type {any[]} */
const READY_USERS = READY?.d?.users ?? [];

/**
 * Независимый оракул BigInt для `last_message_id` (обрабатывает ETF `"nil"` и `null` как `0n`).
 * @param {any} raw
 * @returns {bigint}
 */
function oracleBigInt(raw) {
  if (raw === null || raw === undefined || raw === "nil" || raw === "") return 0n;
  return BigInt(String(raw));
}

/**
 * Создаёт записывающий мок REST/IPC `request` и `invoke`.
 * @param {(req: { method: string, url: string, body?: any, query?: any }) => Promise<any>} [customRest]
 */
function createTransportMock(customRest) {
  /** @type {Array<{ method: string, url: string, body?: any, query?: any }>} */
  const restCalls = [];
  /** @type {Array<{ cmd: string, args: any }>} */
  const invokeCalls = [];

  /**
   * @param {{ method: string, url: string, body?: any, query?: any }} req
   * @returns {Promise<any>}
   */
  const request = async (req) => {
    restCalls.push(req);
    if (customRest) return customRest(req);
    return { status: 200, ok: true, body: {} };
  };

  /**
   * @param {string} cmd
   * @param {any} [args]
   * @returns {Promise<any>}
   */
  const invoke = async (cmd, args) => {
    invokeCalls.push({ cmd, args });
    return { ok: true };
  };

  return { restCalls, invokeCalls, request, invoke };
}

/**
 * Собирает тестовый DOM-каркас спайка (`#guild-rail`, `#channel-tree`, `.content`,
 * `#chat-header`, `#screen-chat`, `#server-status`).
 * @param {{
 *   privateChannels?: any[],
 *   relationships?: any[],
 *   users?: any[],
 *   presences?: Record<string, any>,
 *   request?: (req: { method: string, url: string, body?: any, query?: any }) => Promise<any>,
 *   timers?: { setInterval: (fn: () => void, ms: number) => any, clearInterval: (handle: any) => void },
 * }} [opts]
 */
function setupDomHarness(opts = {}) {
  const doc = new ShimDocument();

  const statusNode = doc.createElement("div");
  statusNode.id = "server-status";
  statusNode.setAttribute("role", "status");
  statusNode.setAttribute("aria-live", "polite");
  doc.body.append(statusNode);

  // 1. #guild-rail
  const rail = doc.createElement("aside");
  rail.id = "guild-rail";
  rail.className = "rail";
  rail.setAttribute("role", "navigation");
  rail.setAttribute("aria-label", "Серверы и экраны");

  const brand = doc.createElement("div");
  brand.className = "brand";
  brand.title = "discord-lab";
  brand.setAttribute("aria-hidden", "true");
  brand.textContent = "DL";
  rail.append(brand);

  const guildList = doc.createElement("div");
  guildList.className = "guild-list";
  guildList.setAttribute("role", "navigation");
  guildList.setAttribute("aria-label", "Серверы");

  const g1 = doc.createElement("button");
  g1.className = "guild-item is-selected";
  g1.setAttribute("type", "button");
  g1.setAttribute("data-guild-id", "696738350706458685");
  g1.dataset.guild = "696738350706458685";
  g1.setAttribute("aria-label", "Сервер ITSKIDON PC BOOST");
  g1.setAttribute("aria-current", "true");
  g1.setAttribute("tabindex", "0");
  g1.textContent = "IP";

  const g2 = doc.createElement("button");
  g2.className = "guild-item";
  g2.setAttribute("type", "button");
  g2.setAttribute("data-guild-id", "777000111222333444");
  g2.dataset.guild = "777000111222333444";
  g2.setAttribute("aria-label", "Сервер Rust Guild");
  g2.setAttribute("aria-current", "false");
  g2.setAttribute("tabindex", "-1");
  g2.textContent = "RG";

  guildList.append(g1, g2);
  const railSep = doc.createElement("div");
  railSep.className = "rail-sep";
  railSep.setAttribute("role", "separator");
  rail.append(guildList, railSep);
  doc.body.append(rail);

  // 2. #channel-tree
  const tree = doc.createElement("aside");
  tree.id = "channel-tree";
  tree.className = "tree";
  tree.setAttribute("role", "navigation");
  tree.setAttribute("aria-label", "Каналы сервера");
  tree.setAttribute("data-nav-mode", "guild");

  const treeHeader = doc.createElement("header");
  treeHeader.className = "tree-header";
  const treeGuildTitle = doc.createElement("h2");
  treeGuildTitle.className = "tree-guild";
  treeGuildTitle.textContent = "ITSKIDON PC BOOST";
  const guildMenuBtn = doc.createElement("button");
  guildMenuBtn.className = "icon-btn";
  guildMenuBtn.setAttribute("type", "button");
  guildMenuBtn.setAttribute("data-action", "guild-menu");
  guildMenuBtn.setAttribute("aria-label", "Меню сервера");
  treeHeader.append(treeGuildTitle, guildMenuBtn);

  const treeScroll = doc.createElement("nav");
  treeScroll.className = "tree-scroll";
  const guildChannelSection = doc.createElement("section");
  guildChannelSection.className = "channel-group";
  guildChannelSection.textContent = "НОВОСТИ";
  treeScroll.append(guildChannelSection);

  const voicePanel = doc.createElement("section");
  voicePanel.id = "voice-panel";
  voicePanel.className = "voice-panel";
  const userBar = doc.createElement("footer");
  userBar.id = "user-bar";
  userBar.className = "user-bar";
  tree.append(treeHeader, treeScroll, voicePanel, userBar);
  doc.body.append(tree);

  // 3. .content (#chat-header + #screen-chat)
  const content = doc.createElement("main");
  content.className = "content";
  const chatHeader = doc.createElement("header");
  chatHeader.id = "chat-header";
  chatHeader.className = "chat-header";
  const chatTitle = doc.createElement("h1");
  chatTitle.className = "chat-title";
  const chatTitleText = doc.createElement("span");
  chatTitleText.className = "chat-title-text";
  chatTitleText.textContent = "новости";
  chatTitle.append(chatTitleText);
  chatHeader.append(chatTitle);

  const screenChat = doc.createElement("section");
  screenChat.id = "screen-chat";
  screenChat.className = "screen active";
  screenChat.setAttribute("role", "region");
  screenChat.setAttribute("aria-label", "Чат");
  screenChat.setAttribute("aria-hidden", "false");
  content.append(chatHeader, screenChat);
  doc.body.append(content);

  const { restCalls, invokeCalls, request, invoke } = createTransportMock(opts.request);

  const controller = createDmFriendsController({
    document: /** @type {any} */ (doc),
    request,
    invoke,
    privateChannels: opts.privateChannels ?? READY_PRIVATE_CHANNELS,
    relationships: opts.relationships ?? READY_RELATIONSHIPS,
    users: opts.users ?? READY_USERS,
    presences: opts.presences ?? {},
    timers: opts.timers,
  });

  return {
    doc,
    rail,
    tree,
    content,
    chatHeader,
    screenChat,
    statusNode,
    restCalls,
    invokeCalls,
    controller,
  };
}

describe("TASK-113 / N1-07 DM & Friends UI: оракул 002-READY.json, сортировка ЛС и Group DM (1a)", () => {
  it("T-N107-READY-01: константы и размеры коллекций 002-READY.json совпадают с RE-оракулом", () => {
    assert.ok(
      RE_TEXT.includes("POST /users/@me/channels") && RE_TEXT.includes("80013"),
      "в DM-FRIENDS-RE.md должны быть зафиксированы маршрут POST /users/@me/channels и код 80013",
    );
    assert.ok(
      SPEC_TEXT.includes("#friends-tablist") && SPEC_TEXT.includes("#group-dm-dialog"),
      "в DM-FRIENDS-SPEC.md должны быть описаны #friends-tablist и #group-dm-dialog",
    );

    assert.equal(HOME_GUILD_ID, "@me");
    assert.equal(PRIVATE_CHANNEL_TYPES.DM, 1);
    assert.equal(PRIVATE_CHANNEL_TYPES.GROUP_DM, 3);
    assert.equal(CHANNEL_FLAGS.PINNED, 2);
    assert.equal(CHANNEL_FLAGS.IS_SPAM, 32);
    assert.equal(RELATIONSHIP_TYPES.NONE, 0);
    assert.equal(RELATIONSHIP_TYPES.FRIEND, 1);
    assert.equal(RELATIONSHIP_TYPES.BLOCKED, 2);
    assert.equal(RELATIONSHIP_TYPES.PENDING_INCOMING, 3);
    assert.equal(RELATIONSHIP_TYPES.PENDING_OUTGOING, 4);
    assert.equal(RELATIONSHIP_FILTERS.SPAM, 1);
    assert.equal(RELATIONSHIP_FILTERS.IGNORED, 2);
    assert.equal(MAX_GROUP_DM_RECIPIENTS, 9);
    assert.equal(MAX_GROUP_DM_PARTICIPANTS, 10);
    assert.equal(DM_FRIENDS_ERROR_CODES.RELATIONSHIP_INVALID_NO_CONFIRMATION, 80013);
    assert.equal(DM_FRIENDS_ERROR_CODES.TOO_MANY_RECIPIENTS, 30004);
    assert.equal(DM_FRIENDS_ENDPOINTS.USER_CHANNELS, "/users/@me/channels");
    assert.equal(DM_FRIENDS_ENDPOINTS.USER_GROUP_DM_SHELL, "/users/@me/channels/group-dm-shell");
    assert.deepEqual(FRIENDS_TABS, ["online", "all", "pending", "blocked", "add_friend"]);

    // 002-READY.json: 17 private_channels, 60 relationships, 146 users
    assert.equal(READY_PRIVATE_CHANNELS.length, 17);
    assert.equal(READY_RELATIONSHIPS.length, 60);
    assert.equal(READY_USERS.length, 146);
  });

  it("T-N107-SORT-01: сортирует 17 private_channels из 002-READY.json по BigInt(last_message_id ?? 0) по убыванию и разрешает имена/аватары из 146 READY.users", () => {
    const usersById = buildUserMap(READY_USERS);
    assert.equal(usersById.size, 146, "все 146 пользователей из READY.users должны быть в карте");

    // Независимый расчёт порядка 17 каналов по BigInt(last_message_id ?? 0) по убыванию
    const expectedOrderIds = [...READY_PRIVATE_CHANNELS]
      .sort((a, b) => {
        const la = oracleBigInt(a.last_message_id);
        const lb = oracleBigInt(b.last_message_id);
        if (lb !== la) return lb > la ? 1 : -1;
        const ia = oracleBigInt(a.id);
        const ib = oracleBigInt(b.id);
        return ib > ia ? 1 : ib < ia ? -1 : 0;
      })
      .map((ch) => String(ch.id));

    assert.equal(expectedOrderIds[0], "1213176899493306439", "первый канал — самый свежий last_message_id");
    assert.equal(expectedOrderIds[1], "1369333759416991854");
    assert.equal(expectedOrderIds[2], "1293192721237938187");
    assert.equal(
      expectedOrderIds[16],
      "1346114403149549638",
      "канал с last_message_id='nil' должен быть последним (0n)",
    );

    const hydrated = READY_PRIVATE_CHANNELS.map((ch) => hydratePrivateChannel(ch, usersById));
    const sorted = sortPrivateChannels(hydrated);

    assert.equal(sorted.length, 17);
    assert.deepEqual(
      sorted.map((ch) => ch.id),
      expectedOrderIds,
      "порядок sortPrivateChannels обязан точно совпадать с оракулом BigInt(last_message_id ?? 0) desc",
    );

    // Проверяем разрешение имён и аватаров каждого из 17 каналов против READY.users
    for (const ch of sorted) {
      assert.equal(ch.type, 1, `канал ${ch.id} должен иметь type=1 (DM)`);
      assert.equal(ch.guild_id, null, "у приватного канала guild_id всегда null");
      assert.equal(ch.recipient_ids.length, 1, `у DM ${ch.id} ровно 1 получатель`);
      const recId = ch.recipient_ids[0];
      assert.ok(recId && usersById.has(recId), `получатель ${recId} обязан резолвиться в READY.users`);
      const rawUser = READY_USERS.find((u) => String(u.id) === recId);
      assert.ok(rawUser);

      const expectedName =
        (rawUser.global_name && rawUser.global_name !== "nil" ? rawUser.global_name : null) ||
        (rawUser.display_name && rawUser.display_name !== "nil" ? rawUser.display_name : null) ||
        rawUser.username;
      assert.equal(ch.name, expectedName, `имя канала ${ch.id} должно совпадать с именем получателя`);
      assert.ok(ch.avatar && typeof ch.avatar.initials === "string" && ch.avatar.initials.length >= 1);
      if (rawUser.avatar && rawUser.avatar !== "nil") {
        assert.ok(
          ch.avatar.url && ch.avatar.url.includes(rawUser.avatar),
          `аватар ${ch.id} должен включать хеш ${rawUser.avatar}`,
        );
      } else {
        assert.equal(ch.avatar.url, null);
      }
    }
  });

  it("отображает типы 1 (DM) и 3 (Group DM с числом участников, кастомным именем и fallback через запятую, плюс приоритет PINNED=2)", () => {
    const usersById = buildUserMap(READY_USERS);
    const u1 = READY_USERS[0];
    const u2 = READY_USERS[1];
    const u3 = READY_USERS[2];

    // 1. Group DM с кастомным именем и 3 получателями (= 4 участника вместе с @me)
    const namedGroup = hydratePrivateChannel(
      {
        id: "712345678901234567",
        type: 3,
        name: "Команда релиза",
        owner_id: u1.id,
        last_message_id: "1560000000000000000",
        recipients: [u1, u2, u3],
      },
      usersById,
    );
    assert.equal(namedGroup.type, 3);
    assert.equal(namedGroup.name, "Команда релиза");
    assert.equal(namedGroup.participantCount, 4);
    assert.equal(namedGroup.subtext, "4 участника");

    // 2. Group DM без кастомного имени (name: "") и с локальным ником в nicks -> имена через запятую
    const unnamedGroup = hydratePrivateChannel(
      {
        id: "712345678901234568",
        type: 3,
        name: "",
        last_message_id: "1550000000000000000",
        recipient_ids: [u1.id, u2.id],
        nicks: { [u1.id]: "Тимлид Рома" },
      },
      usersById,
    );
    assert.equal(unnamedGroup.type, 3);
    assert.ok(unnamedGroup.name.includes("Тимлид Рома"), "должен учитываться локальный ник из channel.nicks");
    assert.ok(unnamedGroup.name.includes(resolveUserDisplayName(usersById.get(String(u2.id)))));
    assert.equal(unnamedGroup.participantCount, 3);
    assert.equal(unnamedGroup.subtext, "3 участника");

    // 3. Склонение числа участников для Group DM
    assert.equal(formatParticipantsSubtext(1), "1 участник");
    assert.equal(formatParticipantsSubtext(2), "2 участника");
    assert.equal(formatParticipantsSubtext(4), "4 участника");
    assert.equal(formatParticipantsSubtext(5), "5 участников");
    assert.equal(formatParticipantsSubtext(10), "10 участников");

    // 4. Закреплённый канал (flags & 2) идёт выше незакреплённого даже с меньшим last_message_id
    const pinnedDm = hydratePrivateChannel(
      {
        id: "696738350706458685",
        type: 1,
        flags: CHANNEL_FLAGS.PINNED,
        last_message_id: "1000000000000000000",
        recipient_ids: [u1.id],
      },
      usersById,
    );
    const sortedWithPin = sortPrivateChannels([unnamedGroup, namedGroup, pinnedDm]);
    assert.equal(sortedWithPin[0]?.id, pinnedDm.id, "PINNED (flags & 2) обязан быть первым");
    assert.equal(sortedWithPin[1]?.id, namedGroup.id);
    assert.equal(sortedWithPin[2]?.id, unnamedGroup.id);
  });
});

describe("TASK-113 / N1-07 DM & Friends UI: классификация и фильтрация 60 relationships по типам 1..4 и вкладкам (1b)", () => {
  it("T-N107-READY-01 & T-N107-TABS-01: классифицирует 60 relationships из 002-READY.json (18 друзей type=1, 42 блокировки type=2)", () => {
    const usersById = buildUserMap(READY_USERS);

    // Независимый подсчёт по 002-READY.json
    const oracleCounts = READY_RELATIONSHIPS.reduce(
      (acc, r) => {
        acc[r.type] = (acc[r.type] || 0) + 1;
        assert.equal(r.id, r.user_id, "в 002-READY.json у всех 60 записей id === user_id");
        assert.ok(usersById.has(String(r.user_id)), `user_id ${r.user_id} обязан быть в READY.users`);
        return acc;
      },
      /** @type {Record<number, number>} */ ({}),
    );
    assert.deepEqual(oracleCounts, { 1: 18, 2: 42 });

    const summary = classifyRelationships(READY_RELATIONSHIPS, usersById);
    assert.equal(summary.friendCount, 18);
    assert.equal(summary.blockedCount, 42);
    assert.equal(summary.incomingCount, 0);
    assert.equal(summary.outgoingCount, 0);
    assert.equal(summary.spamCount, 0);
    assert.equal(summary.pendingIgnoredCount, 0);
    assert.equal(summary.pendingCount, 0);

    // Вкладка "all" возвращает ровно 18 друзей, отсортированных по имени
    const allFriends = filterRelationshipsByTab(READY_RELATIONSHIPS, "all", { usersById });
    assert.equal(allFriends.length, 18);
    assert.ok(allFriends.every((r) => r.type === RELATIONSHIP_TYPES.FRIEND));

    // Вкладка "blocked" возвращает ровно 42 заблокированных пользователя
    const blockedUsers = filterRelationshipsByTab(READY_RELATIONSHIPS, "blocked", { usersById });
    assert.equal(blockedUsers.length, 42);
    assert.ok(blockedUsers.every((r) => r.type === RELATIONSHIP_TYPES.BLOCKED));
  });

  it("фильтрует типы 1=Friend, 2=Blocked, 3=Incoming, 4=Outgoing по вкладкам Online / All / Pending / Blocked / Add Friend и считает pendingCount по формуле RE @9318383", () => {
    const usersById = buildUserMap(READY_USERS);
    const friendIds = READY_RELATIONSHIPS.filter((r) => r.type === 1).map((r) => String(r.user_id));
    const [fOnline, fIdle, fDnd, fStreaming, fOffline, fIgnored] = friendIds;

    const presencesById = {
      [String(fOnline)]: { status: "online" },
      [String(fIdle)]: { status: "idle" },
      [String(fDnd)]: { status: "dnd" },
      [String(fStreaming)]: { status: "online", activities: [{ type: 1, name: "Twitch" }] },
      [String(fOffline)]: { status: "offline" },
      [String(fIgnored)]: { status: "online" },
    };

    const syntheticRelationships = [
      ...READY_RELATIONSHIPS.map((r) =>
        String(r.user_id) === fIgnored ? { ...r, user_ignored: true } : r,
      ),
      // 3 входящих (type=3): 1 обычная, 1 спам, 1 игнорируемая
      {
        id: "900000000000000001",
        user_id: "900000000000000001",
        type: 3,
        since: "2026-10-20T10:00:00.000Z",
        is_spam_request: false,
        user_ignored: false,
        user: { id: "900000000000000001", username: "incoming_normal", global_name: "Incoming Normal", discriminator: "0" },
      },
      {
        id: "900000000000000002",
        user_id: "900000000000000002",
        type: 3,
        since: "2026-10-21T10:00:00.000Z",
        is_spam_request: true,
        user_ignored: false,
        user: { id: "900000000000000002", username: "incoming_spam", global_name: "Incoming Spam", discriminator: "0" },
      },
      {
        id: "900000000000000003",
        user_id: "900000000000000003",
        type: 3,
        since: "2026-10-22T10:00:00.000Z",
        is_spam_request: false,
        user_ignored: true,
        user: { id: "900000000000000003", username: "incoming_ignored", global_name: "Incoming Ignored", discriminator: "0" },
      },
      // 2 исходящих (type=4)
      {
        id: "900000000000000004",
        user_id: "900000000000000004",
        type: 4,
        since: "2026-10-19T10:00:00.000Z",
        is_spam_request: false,
        user_ignored: false,
        user: { id: "900000000000000004", username: "outgoing_older", global_name: "Outgoing Older", discriminator: "0" },
      },
      {
        id: "900000000000000005",
        user_id: "900000000000000005",
        type: 4,
        since: "2026-10-25T10:00:00.000Z",
        is_spam_request: false,
        user_ignored: false,
        user: { id: "900000000000000005", username: "outgoing_newer", global_name: "Outgoing Newer", discriminator: "0" },
      },
    ];

    const stats = classifyRelationships(syntheticRelationships, usersById, presencesById);
    assert.equal(stats.friendCount, 18);
    assert.equal(stats.blockedCount, 42);
    assert.equal(stats.incomingCount, 3);
    assert.equal(stats.spamCount, 1);
    assert.equal(stats.pendingIgnoredCount, 1);
    assert.equal(stats.pendingCount, 1, "pendingCount = max(3 - 1 - 1, 0) = 1");
    assert.equal(stats.outgoingCount, 2);

    // Online: только 4 неигнорируемых друга в сети (online, idle, dnd, streaming); offline и ignored исключены
    const onlineTab = filterRelationshipsByTab(syntheticRelationships, "online", {
      usersById,
      presencesById,
    });
    assert.equal(onlineTab.length, 4);
    const onlineIds = new Set(onlineTab.map((r) => r.user_id));
    assert.ok(onlineIds.has(String(fOnline)));
    assert.ok(onlineIds.has(String(fIdle)));
    assert.ok(onlineIds.has(String(fDnd)));
    assert.ok(onlineIds.has(String(fStreaming)));
    assert.ok(!onlineIds.has(String(fOffline)), "offline друг не должен попадать во вкладку Online");
    assert.ok(!onlineIds.has(String(fIgnored)), "игнорируемый друг (!isIgnored) не должен попадать во вкладку Online");

    // Pending (по умолчанию): 1 входящий без спама/игнора + 2 исходящих = 3; сначала type=3, затем type=4 по убыванию since
    const pendingTab = filterRelationshipsByTab(syntheticRelationships, "pending", {
      usersById,
      presencesById,
    });
    assert.deepEqual(
      pendingTab.map((r) => r.user_id),
      ["900000000000000001", "900000000000000005", "900000000000000004"],
    );

    // Pending (режим Спам и игнор): ровно 2 входящих (spam + ignored)
    const spamTab = filterRelationshipsByTab(syntheticRelationships, "pending", {
      usersById,
      presencesById,
      showSpamAndIgnored: true,
    });
    assert.deepEqual(
      spamTab.map((r) => r.user_id),
      ["900000000000000003", "900000000000000002"],
    );

    // Add Friend вкладка возвращает пустой список карточек
    const addFriendTab = filterRelationshipsByTab(syntheticRelationships, "add_friend", {
      usersById,
      presencesById,
    });
    assert.deepEqual(addFriendTab, []);
  });
});

describe("TASK-113 / N1-07 DM & Friends UI: переключение Home/Friends (guildId='@me'), дерево ЛС и WAI-ARIA вкладки (1c)", () => {
  it("T-N107-DM-GET-01 & Home Navigation: переключает guildId='@me' по клику на #home-rail-btn без вызовов GET /users/@me/channels, рендерит дерево ЛС и восстанавливает шапку гильдии при возврате", () => {
    const { doc, rail, tree, content, chatHeader, screenChat, restCalls, controller } = setupDomHarness();

    const homeBtn = doc.getElementById("home-rail-btn");
    assert.ok(homeBtn, "#home-rail-btn должен быть смонтирован в #guild-rail");
    assert.equal(homeBtn.getAttribute("data-guild-id"), "@me");
    assert.equal(homeBtn.getAttribute("data-action"), "nav-home");

    // Переключаемся в режим Home/Friends кликом по #home-rail-btn
    homeBtn.click();

    assert.equal(controller.getGuildId(), "@me", "активный guildId в режиме Home обязан быть '@me'");
    assert.equal(homeBtn.classList.contains("is-selected"), true);
    assert.equal(homeBtn.getAttribute("aria-current"), "page");

    const guildItems = rail.querySelectorAll(".guild-item");
    for (const item of guildItems) {
      assert.equal(item.classList.contains("is-selected"), false, "с плиток гильдий должен сниматься .is-selected");
      assert.equal(item.getAttribute("aria-current"), "false");
    }

    assert.equal(tree.getAttribute("data-nav-mode"), "home");
    assert.equal(tree.getAttribute("aria-label"), "Личные сообщения");
    assert.equal(content.getAttribute("data-home-subview"), "friends");
    assert.equal(screenChat.classList.contains("active"), false, "в режиме friends #screen-chat не должен оставаться .active (P1.1)");

    // T-N107-DM-GET-01: 0 вызовов GET /users/@me/channels
    const getChannelsCalls = restCalls.filter(
      (c) => c.method === "GET" && c.url === "/users/@me/channels",
    );
    assert.equal(getChannelsCalls.length, 0, "клиент никогда не вызывает GET /users/@me/channels");

    // Дерево ЛС отрендерило все 17 каналов в точном порядке убывания last_message_id
    const dmRows = tree.querySelectorAll(".dm-channel-row");
    assert.equal(dmRows.length, 17);
    assert.equal(dmRows[0]?.getAttribute("data-channel-id"), "1213176899493306439");
    assert.equal(dmRows[16]?.getAttribute("data-channel-id"), "1346114403149549638");

    // Проверка структуры строки: .dm-channel-item и сосед .dm-close-btn (не вложен внутрь кнопки)
    const firstRow = dmRows[0];
    assert.ok(firstRow);
    const firstChannelBtn = firstRow.querySelector(".dm-channel-item");
    const firstCloseBtn = firstRow.querySelector(".dm-close-btn");
    assert.ok(firstChannelBtn && firstCloseBtn);
    assert.equal(firstCloseBtn.parentNode, firstRow, ".dm-close-btn обязан быть соседом .dm-channel-item в <li class='dm-channel-row'>");

    // Клавиатурная навигация в дереве ЛС: ArrowDown с #dm-nav-friends на первый канал, ArrowRight на .dm-close-btn, ArrowLeft обратно
    const navFriendsBtn = doc.getElementById("dm-nav-friends");
    assert.ok(navFriendsBtn);
    assert.equal(navFriendsBtn.getAttribute("tabindex"), "0");
    navFriendsBtn.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(doc.activeElement, firstChannelBtn);
    assert.equal(firstChannelBtn.getAttribute("tabindex"), "0");
    assert.equal(navFriendsBtn.getAttribute("tabindex"), "-1");

    firstChannelBtn.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(doc.activeElement, firstCloseBtn);
    assert.equal(firstCloseBtn.getAttribute("tabindex"), "0");

    firstCloseBtn.dispatchEvent("keydown", { key: "ArrowLeft" });
    assert.equal(doc.activeElement, firstChannelBtn);

    // Клик по каналу ЛС открывает переписку в #screen-chat
    firstChannelBtn.click();
    assert.equal(content.getAttribute("data-home-subview"), "dm");
    assert.equal(screenChat.classList.contains("active"), true);
    assert.equal(firstChannelBtn.getAttribute("aria-current"), "page");
    const titleNode = chatHeader.querySelector(".chat-title-text");
    assert.equal(titleNode?.textContent, firstRow.querySelector(".dm-item-name")?.textContent);

    // Возврат в гильдию кликом по плитке сервера восстанавливает .tree-guild и guild-menu (P1.4)
    const firstGuildTile = guildItems[0];
    assert.ok(firstGuildTile);
    firstGuildTile.click();
    assert.equal(controller.getGuildId(), "696738350706458685");
    assert.equal(homeBtn.classList.contains("is-selected"), false);
    assert.equal(homeBtn.getAttribute("aria-current"), "false");
    assert.equal(tree.getAttribute("data-nav-mode"), "guild");
    assert.equal(tree.getAttribute("aria-label"), "Каналы сервера");
    assert.ok(tree.querySelector(".tree-guild"), "при возврате в гильдию заголовок .tree-guild должен восстанавливаться");
    assert.ok(tree.querySelector('[data-action="guild-menu"]'), "кнопка меню сервера должна восстанавливаться");
  });

  it("T-N107-A11Y-01: вкладки друзей реализуют WAI-ARIA роли tablist/tab/tabpanel и циклический roving tabindex по ArrowRight/ArrowLeft/Home/End", () => {
    const { doc, controller } = setupDomHarness({
      presences: {
        "194374444821643266": { status: "online" },
        "205241178625409024": { status: "idle" },
      },
    });
    controller.selectHome();

    const friendsView = doc.getElementById("friends-view");
    const tablist = doc.getElementById("friends-tablist");
    const tabpanel = doc.getElementById("friends-tabpanel");
    const addPanel = doc.getElementById("friends-add-panel");
    assert.ok(friendsView && tablist && tabpanel && addPanel);

    assert.equal(tablist.getAttribute("role"), "tablist");
    assert.equal(tablist.getAttribute("aria-orientation"), "horizontal");
    assert.equal(tabpanel.getAttribute("role"), "tabpanel");
    assert.equal(addPanel.getAttribute("role"), "tabpanel");

    const tabs = tablist.querySelectorAll('[role="tab"]');
    assert.equal(tabs.length, 5);

    // Изначально выбрана вкладка "online" (В сети — 2)
    assert.equal(tabs[0]?.getAttribute("data-tab"), "online");
    assert.equal(tabs[0]?.getAttribute("aria-selected"), "true");
    assert.equal(tabs[0]?.getAttribute("tabindex"), "0");
    assert.equal(tabs[1]?.getAttribute("aria-selected"), "false");
    assert.equal(tabs[1]?.getAttribute("tabindex"), "-1");
    assert.equal(tabpanel.getAttribute("aria-labelledby"), "friends-tab-online");
    assert.equal(tabpanel.hasAttribute("hidden"), false);
    assert.equal(addPanel.hasAttribute("hidden"), true);
    assert.equal(doc.getElementById("friends-section-count")?.textContent, "2");
    assert.equal(doc.querySelectorAll("#friends-list .friend-row").length, 2);

    // ArrowRight -> "all" (Все — 18)
    tabs[0]?.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(doc.activeElement, tabs[1]);
    assert.equal(tabs[1]?.getAttribute("data-tab"), "all");
    assert.equal(tabs[1]?.getAttribute("aria-selected"), "true");
    assert.equal(tabs[1]?.getAttribute("tabindex"), "0");
    assert.equal(tabs[0]?.getAttribute("aria-selected"), "false");
    assert.equal(tabs[0]?.getAttribute("tabindex"), "-1");
    assert.equal(tabpanel.getAttribute("aria-labelledby"), "friends-tab-all");
    assert.equal(doc.getElementById("friends-section-count")?.textContent, "18");
    assert.equal(doc.querySelectorAll("#friends-list .friend-row").length, 18);

    // ArrowRight -> "pending" (Ожидание — 0, пустое состояние)
    tabs[1]?.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(doc.activeElement, tabs[2]);
    assert.equal(tabs[2]?.getAttribute("data-tab"), "pending");
    assert.equal(doc.getElementById("friends-section-count")?.textContent, "0");
    assert.equal(doc.getElementById("friends-empty-state")?.hasAttribute("hidden"), false);

    // ArrowRight -> "blocked" (Заблокированные — 42)
    tabs[2]?.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(doc.activeElement, tabs[3]);
    assert.equal(tabs[3]?.getAttribute("data-tab"), "blocked");
    assert.equal(doc.getElementById("friends-section-count")?.textContent, "42");
    assert.equal(doc.querySelectorAll("#friends-list .friend-row").length, 42);

    // ArrowRight -> "add_friend" (скрывает #friends-tabpanel, открывает #friends-add-panel)
    tabs[3]?.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(doc.activeElement, tabs[4]);
    assert.equal(tabs[4]?.getAttribute("data-tab"), "add_friend");
    assert.equal(tabs[4]?.getAttribute("aria-selected"), "true");
    assert.equal(tabpanel.hasAttribute("hidden"), true);
    assert.equal(addPanel.hasAttribute("hidden"), false);

    // Циклический переход ArrowRight с последней вкладки -> первая ("online")
    tabs[4]?.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(doc.activeElement, tabs[0]);
    assert.equal(tabs[0]?.getAttribute("aria-selected"), "true");
    assert.equal(tabpanel.hasAttribute("hidden"), false);
    assert.equal(addPanel.hasAttribute("hidden"), true);

    // Циклический переход ArrowLeft с первой вкладки -> последняя ("add_friend")
    tabs[0]?.dispatchEvent("keydown", { key: "ArrowLeft" });
    assert.equal(doc.activeElement, tabs[4]);
    assert.equal(tabs[4]?.getAttribute("aria-selected"), "true");

    // Клавиша Home -> первая ("online"), End -> последняя ("add_friend")
    tabs[4]?.dispatchEvent("keydown", { key: "Home" });
    assert.equal(doc.activeElement, tabs[0]);
    tabs[0]?.dispatchEvent("keydown", { key: "End" });
    assert.equal(doc.activeElement, tabs[4]);
  });

  it("T-N107-FRIEND-ADD-01 & T-N107-FRIEND-PUT-01: форма Add Friend отправляет Pomelo username с discriminator:null, классический тег с числом и обрабатывает код 80013 через #stranger-confirm-dialog", async () => {
    let strangerAttempt = 0;
    const relationshipsWithPending = [
      ...READY_RELATIONSHIPS,
      {
        id: "315473023476170753",
        user_id: "315473023476170753",
        type: 3,
        since: "2026-10-30T12:00:00.000Z",
        is_spam_request: false,
        user_ignored: false,
      },
    ];

    const { doc, restCalls, controller } = setupDomHarness({
      relationships: relationshipsWithPending,
      request: async (req) => {
        if (req.method === "PUT" && req.url === "/users/@me/relationships/315473023476170753") {
          strangerAttempt += 1;
          if (!req.body?.confirm_stranger_request) {
            const err = /** @type {any} */ (new Error("Confirmation required"));
            err.status = 400;
            err.body = { code: 80013, message: "Relationship invalid no confirmation" };
            throw err;
          }
          return { status: 204, ok: true, body: null };
        }
        if (req.method === "POST" && req.url === "/users/@me/relationships") {
          if (req.body?.username === "notfound_user") {
            const err = /** @type {any} */ (new Error("Invalid Discord Tag"));
            err.status = 400;
            err.body = { code: 80004 };
            throw err;
          }
          return { status: 204, ok: true, body: null };
        }
        return { status: 204, ok: true, body: null };
      },
    });

    controller.selectHome();
    controller.selectFriendsTab("add_friend");

    // 1. Проверка чистого парсера parseFriendRequestInput
    assert.deepEqual(parseFriendRequestInput("frienduser"), {
      ok: true,
      payload: { username: "frienduser", discriminator: null },
    });
    assert.deepEqual(parseFriendRequestInput("novaknight#4821"), {
      ok: true,
      payload: { username: "novaknight", discriminator: 4821 },
    });
    assert.equal(parseFriendRequestInput("invalid#tag").ok, false);

    const form = doc.getElementById("add-friend-form");
    const input = /** @type {any} */ (doc.getElementById("add-friend-input"));
    const submitBtn = /** @type {any} */ (doc.getElementById("add-friend-submit"));
    const feedback = doc.getElementById("add-friend-feedback");
    assert.ok(form && input && submitBtn && feedback);
    assert.equal(submitBtn.disabled, true);

    // 2. Отправка Pomelo username (без # -> discriminator: null)
    input.value = "frienduser";
    input.emit("input");
    assert.equal(submitBtn.disabled, false);
    await controller.submitAddFriendForm();

    const pomeloCall = restCalls.find(
      (c) => c.method === "POST" && c.url === "/users/@me/relationships" && c.body?.username === "frienduser",
    );
    assert.ok(pomeloCall, "должен быть вызван POST /users/@me/relationships для Pomelo");
    assert.equal(pomeloCall.body.discriminator, null, "для Pomelo discriminator обязан быть строго null");
    assert.equal(feedback.getAttribute("data-kind"), "success");

    // 3. Отправка классического тега username#1234 -> discriminator: 1234 (число)
    input.value = "novaknight#4821";
    input.emit("input");
    await controller.submitAddFriendForm();
    const tagCall = restCalls.find(
      (c) => c.method === "POST" && c.url === "/users/@me/relationships" && c.body?.username === "novaknight",
    );
    assert.ok(tagCall);
    assert.strictEqual(tagCall.body.discriminator, 4821, "для тега #4821 discriminator обязан быть числом 4821");

    // 4. Ошибка 80004 (RELATIONSHIP_INVALID_DISCORD_TAG)
    input.value = "notfound_user";
    input.emit("input");
    await controller.submitAddFriendForm();
    assert.equal(feedback.getAttribute("data-kind"), "error");
    assert.match(feedback.textContent, /Не удалось найти пользователя/u);

    // 5. Обработка кода 80013 (RELATIONSHIP_INVALID_NO_CONFIRMATION) при принятии входящей заявки
    controller.selectFriendsTab("pending");
    const acceptBtn = doc.querySelector('[data-action="accept-friend"][data-user-id="315473023476170753"]');
    assert.ok(acceptBtn, "во вкладке Ожидание должна быть кнопка Принять для 315473023476170753");

    await controller.acceptFriendRequest("315473023476170753", acceptBtn);
    assert.equal(strangerAttempt, 1);

    const strangerDialog = doc.getElementById("stranger-confirm-dialog");
    assert.ok(strangerDialog);
    assert.equal(strangerDialog.hasAttribute("hidden"), false, "при коде 80013 должен открыться #stranger-confirm-dialog");
    const confirmBtn = strangerDialog.querySelector('[data-action="accept-stranger-confirm"]');
    assert.ok(confirmBtn);

    // Подтверждаем приём заявки от незнакомца -> повторный PUT с confirm_stranger_request: true
    await controller.confirmStrangerRequest();
    assert.equal(strangerAttempt, 2);
    assert.equal(strangerDialog.hasAttribute("hidden"), true);

    const putCalls = restCalls.filter(
      (c) => c.method === "PUT" && c.url === "/users/@me/relationships/315473023476170753",
    );
    assert.equal(putCalls.length, 2);
    assert.equal(putCalls[0]?.body.confirm_stranger_request, false);
    assert.equal(putCalls[1]?.body.confirm_stranger_request, true);
    assert.equal(putCalls[1]?.body.type, 1);
  });

  it("T-N107-GDM-MODAL-01 & T-N107-DM-POST-01: модалка Group DM ограничивает выбор до 9 друзей (10 участников с @me), отправляет POST /users/@me/channels с {recipients:[...]}, использует кэш для одиночного DM и обрабатывает 30004", async () => {
    let failWithTooMany = false;
    const { doc, restCalls, controller } = setupDomHarness({
      request: async (req) => {
        if (req.method === "POST" && req.url === "/users/@me/channels") {
          if (failWithTooMany) {
            const err = /** @type {any} */ (new Error("Too many recipients"));
            err.status = 400;
            err.body = { code: 30004 };
            throw err;
          }
          const recIds = /** @type {string[]} */ (req.body.recipients);
          const recUsers = recIds.map(
            (id) => READY_USERS.find((u) => String(u.id) === id) ?? { id, username: `user_${id}`, discriminator: "0" },
          );
          return {
            status: 200,
            ok: true,
            body: {
              id: "1600000000000000999",
              type: recIds.length > 1 ? 3 : 1,
              name: recIds.length > 1 ? "Новая группа" : "",
              last_message_id: "1600000000000001000",
              recipients: recUsers,
            },
          };
        }
        return { status: 200, ok: true, body: {} };
      },
    });

    controller.selectHome();

    // 1. Открытие существующего одиночного DM из кэша 002-READY.json (user 315473023476170753 -> channel 696738350706458685) без HTTP POST
    const cachedId = await controller.openPrivateChannel(["315473023476170753"]);
    assert.equal(cachedId, "696738350706458685");
    assert.equal(
      restCalls.filter((c) => c.url === "/users/@me/channels").length,
      0,
      "открытие существующего DM type=1 из кэша не должно вызывать POST /users/@me/channels",
    );

    // 2. Открытие модалки создания Group DM (#group-dm-dialog)
    const createBtn = doc.getElementById("create-group-dm-btn");
    assert.ok(createBtn);
    createBtn.click();

    const gdmDialog = doc.getElementById("group-dm-dialog");
    const remainingSpan = doc.getElementById("group-dm-remaining");
    const friendList = doc.getElementById("group-dm-friend-list");
    assert.ok(gdmDialog && remainingSpan && friendList);
    assert.equal(gdmDialog.hasAttribute("hidden"), false);
    assert.equal(remainingSpan.textContent, "9");

    const options = friendList.querySelectorAll('[role="option"]');
    assert.equal(options.length, 18, "в списке выбора должны быть все 18 друзей из 002-READY.json");

    // 3. Выбираем 9 друзей (= 10 участников включая @me)
    for (let i = 0; i < 9; i += 1) {
      options[i]?.click();
    }
    assert.equal(remainingSpan.textContent, "0");
    assert.equal(doc.querySelectorAll("#group-dm-chips .group-dm-chip").length, 9);

    // Остальные 9 невыбранных опций должны получить aria-disabled="true"
    assert.equal(options[9]?.getAttribute("aria-disabled"), "true");
    // Попытка выбрать 10-го друга игнорируется (потолок 9 друзей = 10 участников)
    options[9]?.click();
    assert.equal(doc.querySelectorAll("#group-dm-chips .group-dm-chip").length, 9);
    assert.equal(options[9]?.getAttribute("aria-selected"), "false");

    // 4. Ошибка 30004 TOO_MANY_RECIPIENTS оставляет модалку открытой и показывает #group-dm-error
    failWithTooMany = true;
    await controller.submitGroupDmModal();
    const errorEl = doc.getElementById("group-dm-error");
    assert.ok(errorEl);
    assert.equal(errorEl.hasAttribute("hidden"), false);
    assert.match(errorEl.textContent, /максимальное число участников/u);
    assert.equal(gdmDialog.hasAttribute("hidden"), false);

    // 5. Успешное создание Group DM шлёт POST /users/@me/channels с { recipients: [...] } (без recipient_id!)
    failWithTooMany = false;
    await controller.submitGroupDmModal();

    const postCall = restCalls.find((c) => c.method === "POST" && c.url === "/users/@me/channels");
    assert.ok(postCall, "должен быть вызван POST /users/@me/channels");
    assert.ok(Array.isArray(postCall.body.recipients), "в теле обязан быть массив recipients");
    assert.equal(postCall.body.recipients.length, 9);
    assert.equal("recipient_id" in postCall.body, false, "в POST /users/@me/channels никогда нет поля recipient_id");
    assert.equal(gdmDialog.hasAttribute("hidden"), true, "после успешного создания модалка закрывается");
  });

  it("T-N107-GDM-SHELL-01, T-N107-NOTE-01 & T-N107-DISPATCH-01: поддерживает group-dm-shell (201 vs 200), валидацию note_user_id и 7 Gateway-диспатчей", async () => {
    const { restCalls, controller } = setupDomHarness({
      request: async (req) => {
        if (req.method === "POST" && req.url === "/users/@me/channels/group-dm-shell") {
          return { status: 200, ok: true, body: { id: "shell-channel-1", type: 3, recipients: [] } };
        }
        if (req.method === "PUT" && req.url === "/channels/shell-channel-1/recipients/194374444821643266") {
          return { status: 201, ok: true, body: { id: "upgraded-group-2", type: 3, recipients: [] } };
        }
        if (req.method === "PUT" && req.url === "/channels/upgraded-group-2/recipients/205241178625409024") {
          return { status: 204, ok: true, body: null };
        }
        return { status: 200, ok: true, body: {} };
      },
    });

    // 1. group-dm-shell единственным шлёт { recipient_id }
    const shellId = await controller.createGroupDmShell("194374444821643266");
    assert.equal(shellId, "shell-channel-1");
    const shellCall = restCalls.find((c) => c.url === "/users/@me/channels/group-dm-shell");
    assert.deepEqual(shellCall?.body, { recipient_id: "194374444821643266" });

    // PUT /channels/{id}/recipients/{user_id}: статус 201 переключает id канала, 204 сохраняет прежний
    const afterFirstAdd = await controller.addRecipientToGroup("shell-channel-1", "194374444821643266");
    assert.equal(afterFirstAdd, "upgraded-group-2");
    const afterSecondAdd = await controller.addRecipientToGroup("upgraded-group-2", "205241178625409024");
    assert.equal(afterSecondAdd, "upgraded-group-2");

    // 2. Валидация заметок (T-N107-NOTE-01): несовпадение note_user_id выбрасывает Error("Invalid response from server")
    assert.throws(
      () => validateNoteResponse({ note_user_id: "other-id", note: "test" }, "194374444821643266"),
      /Invalid response from server/u,
    );
    assert.equal(
      validateNoteResponse({ note_user_id: "194374444821643266", note: "Друг по Rust" }, "194374444821643266").note,
      "Друг по Rust",
    );

    // 3. Обработка 7 Gateway-диспатчей (T-N107-DISPATCH-01)
    let state = applyDmFriendsDispatch(
      { privateChannels: [], relationships: [], users: [] },
      {
        type: "READY",
        d: {
          private_channels: READY_PRIVATE_CHANNELS.slice(0, 2),
          relationships: [
            { id: "555", user_id: "555", type: 4, since: "2026-10-30T00:00:00Z", is_spam_request: false, user_ignored: false },
          ],
          users: READY_USERS.slice(0, 5),
        },
      },
    );
    assert.equal(state.privateChannels.length, 2);
    assert.equal(state.relationships.length, 1);

    // Идемпотентный CHANNEL_CREATE + CHANNEL_RECIPIENT_ADD/REMOVE + CHANNEL_DELETE
    state = applyDmFriendsDispatch(state, {
      type: "CHANNEL_CREATE",
      d: {
        id: "777888999",
        type: 3,
        name: "Тестовая группа",
        recipients: [{ id: "8881", username: "alpha", discriminator: "0" }],
      },
    });
    state = applyDmFriendsDispatch(state, {
      type: "CHANNEL_CREATE",
      d: {
        id: "777888999",
        type: 3,
        name: "Тестовая группа (обновлена)",
        recipients: [{ id: "8881", username: "alpha", discriminator: "0" }],
      },
    });
    assert.equal(state.privateChannels.length, 3, "повторный CHANNEL_CREATE с тем же id обязан быть идемпотентным");

    state = applyDmFriendsDispatch(state, {
      type: "CHANNEL_RECIPIENT_ADD",
      d: {
        channel_id: "777888999",
        user: { id: "8882", username: "beta", discriminator: "0" },
        nick: "Бета",
      },
    });
    const grp = state.privateChannels.find((c) => c.id === "777888999");
    assert.deepEqual(grp?.recipient_ids, ["8881", "8882"]);

    state = applyDmFriendsDispatch(state, {
      type: "CHANNEL_RECIPIENT_REMOVE",
      d: { channel_id: "777888999", user: { id: "8881" } },
    });
    assert.deepEqual(
      state.privateChannels.find((c) => c.id === "777888999")?.recipient_ids,
      ["8882"],
    );

    state = applyDmFriendsDispatch(state, {
      type: "CHANNEL_DELETE",
      d: { id: "777888999" },
    });
    assert.equal(state.privateChannels.length, 2);

    // RELATIONSHIP_UPDATE: переход 4 (PENDING_OUTGOING) -> 1 (FRIEND) порождает friendRequestAcceptedUser
    state = applyDmFriendsDispatch(state, {
      type: "RELATIONSHIP_UPDATE",
      d: {
        id: "555",
        type: 1,
        user: { id: "555", username: "accepted_friend", discriminator: "0" },
      },
    });
    assert.equal(state.friendRequestAcceptedUser?.id, "555");

    // RELATIONSHIP_REMOVE без поля user корректно удаляет связь по id
    state = applyDmFriendsDispatch(state, {
      type: "RELATIONSHIP_REMOVE",
      d: { id: "555", user_ignored: false },
    });
    assert.equal(state.relationships.length, 0);
  });
});

/**
 * Фейковые таймеры для посекундного обратного отсчёта 429 (`retry_after`).
 * @returns {{
 *   setInterval: (fn: () => void, ms: number) => any,
 *   clearInterval: (handle: any) => void,
 *   tick: () => void,
 *   active: () => number,
 * }}
 */
function createFakeTimers() {
  /** @type {Array<{ fn: () => void, ms: number, cleared: boolean }>} */
  const handles = [];
  return {
    setInterval(fn, ms) {
      const handle = { fn, ms, cleared: false };
      handles.push(handle);
      return handle;
    },
    clearInterval(handle) {
      const found = handles.find((item) => item === handle);
      if (found) found.cleared = true;
    },
    tick() {
      for (const handle of handles) if (!handle.cleared) handle.fn();
    },
    active() {
      return handles.filter((handle) => !handle.cleared).length;
    },
  };
}

/**
 * Строгий фейковый рейл: `children` — коллекция без `indexOf`/`splice`, как `HTMLCollection`,
 * а `parentNode` узлов — getter-only, как `Node.parentNode` в браузере.
 * @returns {{
 *   rail: any,
 *   makeNode: (id: string) => any,
 *   children: any[],
 * }}
 */
function createStrictRail() {
  /** @type {any[]} */
  const children = [];
  /** @type {Map<any, any>} */
  const parents = new Map();

  /** @param {string} id @returns {any} */
  const makeNode = (id) => {
    /** @type {any} */
    const node = {
      id,
      remove() {
        const index = children.indexOf(node);
        if (index >= 0) children.splice(index, 1);
        parents.delete(node);
      },
    };
    Object.defineProperty(node, "parentNode", {
      get() {
        return parents.get(node) ?? null;
      },
      enumerable: true,
      configurable: false,
    });
    return node;
  };

  /** @type {any} */
  const rail = {
    get children() {
      return new Proxy(children, {
        get(target, prop, receiver) {
          if (prop === "indexOf" || prop === "splice") return undefined;
          const value = Reflect.get(target, prop, receiver);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
    },
    /**
     * @param {any} node
     * @param {any} reference
     */
    insertBefore(node, reference) {
      const index = children.indexOf(reference);
      if (index < 0) children.push(node);
      else children.splice(index, 0, node);
      parents.set(node, rail);
      return node;
    },
    /** @param {any} node */
    append(node) {
      children.push(node);
      parents.set(node, rail);
      return node;
    },
  };

  return { rail, makeNode, children };
}

describe("TASK-113 P1/P2: монтирование рейла, 429, ошибки DELETE/PUT, focus trap и детерминированная сортировка (2)", () => {
  it("T-N107-DM-MOUNT-01: вставляет #home-rail-btn через insertBefore/remove на HTMLCollection без indexOf/splice и getter-only parentNode", () => {
    const strict = createStrictRail();
    const brand = strict.makeNode("brand");
    const guildList = strict.makeNode("guild-list");
    strict.rail.append(brand);
    strict.rail.append(guildList);

    // Условия реального DOM: у коллекции детей нет indexOf/splice, parentNode нельзя присвоить.
    assert.equal(typeof strict.rail.children.indexOf, "undefined");
    assert.equal(typeof strict.rail.children.splice, "undefined");
    assert.throws(() => strict.rail.children.indexOf(brand), TypeError);
    assert.throws(() => {
      /** @type {any} */ (brand).parentNode = null;
    }, TypeError);

    const homeBtn = strict.makeNode("home-rail-btn");
    assert.doesNotThrow(() => insertHomeRailButton(strict.rail, homeBtn, brand));

    assert.deepEqual(
      strict.children.map((node) => node.id),
      ["home-rail-btn", "guild-list"],
      "#home-rail-btn обязан встать на место .brand перед .guild-list",
    );
    assert.equal(homeBtn.parentNode, strict.rail);
    assert.equal(brand.parentNode, null, "старый .brand должен быть снят через remove()");

    // Без .brand кнопка добавляется в конец рейла.
    const bare = createStrictRail();
    const bareBtn = bare.makeNode("home-rail-btn");
    assert.doesNotThrow(() => insertHomeRailButton(bare.rail, bareBtn, null));
    assert.deepEqual(bare.children.map((node) => node.id), ["home-rail-btn"]);
  });

  it("T-N107-RATE-01: 429 с retry_after_ms показывает сообщение, запускает посекундный отсчёт и снова включает кнопку", async () => {
    const timers = createFakeTimers();
    const request = async () => {
      const err = new Error("rate limited");
      /** @type {any} */ (err).status = 429;
      /** @type {any} */ (err).body = {
        message: "You are being rate limited.",
        retry_after: 2.4,
        global: false,
      };
      throw err;
    };
    const { doc, controller } = setupDomHarness({ request, timers });

    const input = doc.getElementById("add-friend-input");
    assert.ok(input);
    input.value = "frienduser#1234";
    const feedback = doc.getElementById("add-friend-feedback");
    const submit = doc.getElementById("add-friend-submit");
    assert.ok(feedback && submit);

    const result = /** @type {any} */ (await controller.submitAddFriendForm());
    assert.equal(result.ok, false);
    assert.equal(result.rateLimited, true);
    assert.equal(result.retryAfterMs, 2400);
    assert.equal(result.code, 0, "у 429 Discord нет поля body.code");
    assert.match(String(feedback.textContent), /Повторите через 3 с\./u);
    assert.equal(feedback.getAttribute("hidden"), null);
    assert.equal(/** @type {any} */ (submit).disabled, true);
    assert.equal(timers.active(), 1);

    timers.tick();
    assert.match(String(feedback.textContent), /Повторите через 2 с\./u);
    timers.tick();
    timers.tick();
    assert.equal(timers.active(), 0);
    assert.equal(/** @type {any} */ (submit).disabled, false, "по истечении retry_after кнопка снова доступна");
  });

  it("T-N107-RATE-02: 429 в модалке Group DM выводит отсчёт в #group-dm-error и не закрывает модалку", async () => {
    const timers = createFakeTimers();
    const request = async (/** @type {{ url: string }} */ req) => {
      if (req.url === "/users/@me/channels") {
        const err = new Error("rate limited");
        /** @type {any} */ (err).status = 429;
        /** @type {any} */ (err).body = { retry_after: 5 };
        throw err;
      }
      return { status: 200, ok: true, body: {} };
    };
    const { doc, controller } = setupDomHarness({ request, timers });
    const friendRel = READY_RELATIONSHIPS.find((rel) => rel.type === 1);
    assert.ok(friendRel);

    controller.openGroupDmModal(null);
    controller.toggleGroupDmRecipient(String(friendRel.user_id));
    const result = /** @type {any} */ (await controller.submitGroupDmModal());

    assert.equal(result.ok, false);
    assert.equal(result.rateLimited, true);
    assert.equal(result.retryAfterMs, 5000);
    const errorNode = doc.getElementById("group-dm-error");
    const dialog = doc.getElementById("group-dm-dialog");
    assert.ok(errorNode && dialog);
    assert.match(String(errorNode.textContent), /Повторите через 5 с\./u);
    assert.equal(errorNode.getAttribute("hidden"), null);
    assert.equal(dialog.getAttribute("hidden"), null, "модалка остаётся открытой");
    timers.tick();
    assert.match(String(errorNode.textContent), /Повторите через 4 с\./u);
    controller.closeGroupDmModal(false);
  });

  it("T-N107-FRIEND-ERR-01: отказ DELETE/PUT в remove/block/ignore/clear/note/close показывает текст ошибки и не роняет promise", async () => {
    /** @type {number} */
    let failCode = 30059;
    const request = async () => {
      const err = new Error("rest failure");
      /** @type {any} */ (err).body = { code: failCode };
      throw err;
    };
    const { doc, controller, statusNode } = setupDomHarness({ request });
    const friendRel = READY_RELATIONSHIPS.find((rel) => rel.type === 1);
    assert.ok(friendRel);
    const friendId = String(friendRel.user_id);
    const initialRelationships = controller.getRelationships().length;
    const initialChannels = controller.getPrivateChannels().length;

    failCode = 30059;
    const removeRes = /** @type {any} */ (await controller.removeRelationship(friendId));
    assert.equal(removeRes.ok, false);
    assert.equal(removeRes.code, 30059);
    assert.match(String(removeRes.error), /лимит заблокированных пользователей/u);
    assert.match(String(statusNode.textContent), /лимит заблокированных пользователей/u);
    assert.equal(controller.getRelationships().length, initialRelationships, "при ошибке связь не удаляется");

    failCode = 50003;
    const blockRes = /** @type {any} */ (await controller.blockUser(friendId));
    assert.equal(blockRes.ok, false);
    assert.match(String(blockRes.error), /Невозможно выполнить это действие/u);
    const keptRel = controller.getRelationships().find((rel) => rel.user_id === friendId);
    assert.equal(keptRel?.type, 1, "при ошибке PUT пользователь не становится заблокированным");

    failCode = 80001;
    const ignoreRes = /** @type {any} */ (await controller.ignoreUser(friendId));
    assert.equal(ignoreRes.ok, false);
    assert.match(String(ignoreRes.error), /не принимает заявки/u);

    failCode = 30059;
    const clearRes = /** @type {any} */ (await controller.clearPendingRequests(true));
    assert.equal(clearRes.ok, false);
    assert.equal(controller.getRelationships().length, initialRelationships);

    failCode = 50015;
    const noteRes = /** @type {any} */ (await controller.fetchUserNote(friendId));
    assert.equal(noteRes.ok, false);
    assert.match(String(noteRes.error), /Некорректная длина или формат заметки/u);

    failCode = 50003;
    const closeRes = /** @type {any} */ (
      await controller.closePrivateChannel(String(controller.getPrivateChannels()[0]?.id))
    );
    assert.equal(closeRes.ok, false);
    assert.equal(
      controller.getPrivateChannels().length,
      initialChannels,
      "при отказе DELETE закрытый было канал возвращается в дерево",
    );
    assert.match(String(statusNode.textContent), /Невозможно выполнить это действие/u);
    doc.body.setAttribute("data-state", "checked");
  });

  it("T-N107-A11Y-02: focus trap циклит Tab/Shift+Tab, Escape закрывает модалку, возвращает фокус и не меняет экран", () => {
    const { doc, controller } = setupDomHarness();
    doc.body.setAttribute("data-active-screen", "chat");
    const opener = doc.createElement("button");
    doc.body.append(opener);

    controller.openGroupDmModal(opener);
    const dialog = doc.getElementById("group-dm-dialog");
    const search = doc.getElementById("group-dm-search");
    const closeBtn = dialog?.querySelector('[data-action="close-group-dm-modal"]');
    const submit = doc.getElementById("group-dm-submit");
    assert.ok(dialog && search && closeBtn && submit);
    assert.equal(doc.activeElement, search, "первичный фокус — в #group-dm-search");

    const friendRel = READY_RELATIONSHIPS.find((rel) => rel.type === 1);
    assert.ok(friendRel);
    controller.toggleGroupDmRecipient(String(friendRel.user_id));
    assert.equal(/** @type {any} */ (submit).disabled, false);

    submit.focus();
    const tab = dialog.dispatchEvent("keydown", { key: "Tab" });
    assert.equal(tab.prevented, true);
    assert.equal(doc.activeElement, closeBtn, "Tab с последнего элемента возвращает на первый");

    closeBtn.focus();
    dialog.dispatchEvent("keydown", { key: "Tab", shiftKey: true });
    assert.equal(doc.activeElement, submit, "Shift+Tab с первого элемента возвращает на последний");

    const escape = dialog.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(escape.prevented, true);
    assert.equal(dialog.getAttribute("hidden"), "");
    assert.equal(doc.activeElement, opener, "Escape возвращает фокус на opener");
    assert.equal(doc.body.getAttribute("data-active-screen"), "chat");
  });

  it("T-N107-A11Y-03: закрытие ЛС возвращает фокус на соседний канал в #dm-channel-list", async () => {
    const { doc, tree, controller } = setupDomHarness();
    controller.selectHome();
    const rows = tree.querySelectorAll(".dm-channel-row");
    assert.ok(rows.length >= 2);
    const firstId = rows[0]?.getAttribute("data-channel-id") ?? "";
    const secondId = rows[1]?.getAttribute("data-channel-id") ?? "";
    const closeBtn = rows[0]?.querySelector(".dm-close-btn");
    assert.ok(closeBtn);
    closeBtn.focus();
    closeBtn.click();
    await Promise.resolve();

    assert.ok(
      controller.getPrivateChannels().every((ch) => ch.id !== firstId),
      "канал закрыт оптимистично",
    );
    assert.equal(
      doc.activeElement?.getAttribute("data-channel-id"),
      secondId,
      "фокус переходит на следующий канал",
    );
  });

  it("T-N107-GDM-SORT-01: имя группы не зависит от порядка recipients, id сортируются детерминированно, type сохраняется", () => {
    const users = [
      { id: "300", username: "gamma", discriminator: "0" },
      { id: "100", username: "alpha", discriminator: "0" },
      { id: "200", username: "beta", discriminator: "0" },
    ];
    const base = { id: "900", type: 3, nicks: {} };

    const first = hydratePrivateChannel(
      { ...base, recipients: [users[0], users[1], users[2]] },
      users,
      [],
    );
    const second = hydratePrivateChannel(
      { ...base, recipients: [users[2], users[0], users[1]] },
      users,
      [],
    );

    assert.deepEqual(first.recipient_ids, ["100", "200", "300"]);
    assert.deepEqual(second.recipient_ids, ["100", "200", "300"]);
    assert.equal(first.name, "alpha, beta, gamma");
    assert.equal(second.name, first.name, "fallback-имя группы детерминировано");

    // Публичная гидратация не подменяет тип: 0/2/4 не превращаются в DM.
    assert.equal(hydratePrivateChannel({ id: "1", type: 2 }, [], []).type, 2);
    assert.equal(hydratePrivateChannel({ id: "2", type: 4 }, [], []).type, 4);
    assert.equal(hydratePrivateChannel({ id: "3" }, [], []).type, PRIVATE_CHANNEL_TYPES.DM);
  });

  it("T-N107-READY-02: повторный READY полностью заменяет private_channels и relationships", () => {
    const users = [
      { id: "1", username: "first", discriminator: "0" },
      { id: "2", username: "second", discriminator: "0" },
    ];
    const first = applyDmFriendsDispatch(
      {},
      {
        type: "READY",
        d: {
          private_channels: [
            { id: "10", type: 1, recipient_ids: ["1"] },
            { id: "11", type: 1, recipient_ids: ["2"] },
          ],
          relationships: [{ id: "1", user_id: "1", type: 1 }],
          users,
        },
      },
    );
    assert.equal(first.privateChannels.length, 2);
    assert.equal(first.relationships.length, 1);

    const second = applyDmFriendsDispatch(first, {
      type: "READY",
      d: {
        private_channels: [{ id: "12", type: 1, recipient_ids: ["2"] }],
        relationships: [{ id: "2", user_id: "2", type: 2 }],
        users: [users[1]],
      },
    });
    assert.deepEqual(second.privateChannels.map((ch) => ch.id), ["12"]);
    assert.equal(second.relationships.length, 1);
    assert.equal(second.relationships[0]?.type, 2);
    assert.deepEqual([...second.usersById.keys()], ["2"], "READY заменяет users, а не мержит");
  });

  it("T-N107-FRIEND-DEL-01: одиночный DELETE и массовая очистка pending уходят на /users/@me/relationships", async () => {
    const { controller, restCalls } = setupDomHarness();
    const friendRel = READY_RELATIONSHIPS.find((rel) => rel.type === 1);
    assert.ok(friendRel);
    const friendId = String(friendRel.user_id);

    const single = await controller.removeRelationship(friendId);
    assert.equal(single.ok, true);
    assert.ok(
      restCalls.some(
        (call) => call.method === "DELETE" && call.url === `/users/@me/relationships/${friendId}`,
      ),
      "одиночное удаление обязано вызвать DELETE /users/@me/relationships/{id}",
    );

    const bulk = await controller.clearPendingRequests(true);
    assert.equal(bulk.ok, true);
    const bulkCall = restCalls.find(
      (call) => call.method === "DELETE" && call.url === "/users/@me/relationships",
    );
    assert.ok(bulkCall, "массовая очистка обязана вызвать DELETE /users/@me/relationships");
    assert.deepEqual(bulkCall?.query, { relationship_type: 3 });
    assert.deepEqual(bulkCall?.body, { filters: [1, 2] });
  });

  it("T-N107-LIVE-SYNC-01: syncLiveState, handleGatewayEvent и submitAddFriendForm синхронизируют входящие/исходящие заявки и DM-каналы на лету", async () => {
    const { doc, controller } = setupDomHarness({
      request: async (req) => {
        if (req.method === "POST" && req.url === "/users/@me/relationships") {
          return {
            ok: true,
            status: 204,
            body: null,
            relationships: [
              ...READY_RELATIONSHIPS,
              {
                id: "777088",
                user_id: "777088",
                type: 4,
                user: {
                  id: "777088",
                  username: req.body.username,
                  global_name: "Outgoing Target",
                  discriminator: "0",
                },
              },
            ],
          };
        }
        return { ok: true, status: 204, body: null };
      },
    });

    controller.selectHome();
    controller.selectFriendsTab("add_friend");
    const addInput = doc.getElementById("add-friend-input");
    assert.ok(addInput);
    addInput.value = "new_rust_friend";
    const sendRes = await controller.submitAddFriendForm();
    assert.equal(sendRes.ok, true);
    assert.ok(
      controller.getRelationships().some((r) => r.user_id === "777088" && r.type === 4),
      "отправленная заявка (type=4) обязана сразу появиться в списке relationships контроллера",
    );

    assert.equal(typeof controller.syncLiveState, "function", "syncLiveState обязан экспортироваться");
    assert.equal(typeof controller.handleGatewayEvent, "function", "handleGatewayEvent обязан экспортироваться");

    controller.handleGatewayEvent({
      type: "RELATIONSHIP_ADD",
      d: {
        id: "777099",
        type: 3,
        is_spam_request: false,
        user_ignored: false,
        user: {
          id: "777099",
          username: "live_requester",
          global_name: "Live Requester",
          discriminator: "0",
        },
      },
    });
    assert.ok(
      controller.getRelationships().some((r) => r.user_id === "777099" && r.type === 3 && r.displayName === "Live Requester"),
      "входящая заявка из RELATIONSHIP_ADD обязана гидратироваться с именем Live Requester",
    );

    controller.selectFriendsTab("pending");
    const pendingRows = doc.querySelectorAll("#friends-list .friend-row");
    const rowIds = Array.from(pendingRows).map((el) => el.getAttribute("data-user-id"));
    assert.ok(rowIds.includes("777099"), "вкладка Ожидание обязана отображать входящую заявку 777099");
    assert.ok(rowIds.includes("777088"), "вкладка Ожидание обязана отображать исходящую заявку 777088");

    // 4. При переходе в Друзья (selectHome) любые фоновые тестовые экраны (#screen-emoji, #screen-members, #screen-settings) обязаны деактивироваться
    const contentEl = doc.querySelector(".content");
    const screenEmoji = doc.createElement("section");
    screenEmoji.id = "screen-emoji";
    screenEmoji.className = "screen active";
    screenEmoji.setAttribute("aria-hidden", "false");
    const screenMembers = doc.createElement("section");
    screenMembers.id = "screen-members";
    screenMembers.className = "screen active";
    screenMembers.setAttribute("aria-hidden", "false");
    contentEl?.append(screenEmoji, screenMembers);

    controller.selectHome();
    assert.equal(
      screenEmoji.classList.contains("active"),
      false,
      "#screen-emoji не должен оставаться active поверх #friends-view",
    );
    assert.equal(screenEmoji.getAttribute("aria-hidden"), "true");
    assert.equal(
      screenMembers.classList.contains("active"),
      false,
      "#screen-members не должен оставаться active поверх #friends-view",
    );
    assert.equal(screenMembers.getAttribute("aria-hidden"), "true");
  });
});
