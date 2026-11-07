// @ts-check
/**
 * N-05-UI-NOTIFICATIONS (RW-40): поведенческие unit-тесты модуля настроек уведомлений.
 *
 * Независимые оракулы:
 * - `fixtures/gateway/by-type/002-READY.json` (`d.user_guild_settings`: 58 записей,
 *   1328 `channel_overrides` у 52 записей, распределения `message_notifications`,
 *   `notify_highlights`, `flags`, `muted`, `mute_config`);
 * - `waves/RW-40/NOTIFICATIONS-RE.md` (§1–§12: перечисления 0/1/2/3, 6 пресетов таймера
 *   `900, 3600, 10800, 28800, 86400, -1`, формула `isMuted`, каскад разрешения,
 *   двойная форма `channel_overrides`, маршруты PATCH);
 * - `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (§1–§6: матрица упоминаний и `allowNoMessages`,
 *   индикаторы `#guild-rail` и `#channel-tree`, модалка `#guild-notif-modal` в
 *   `#guild-notifications-dialog`, таблица `.notif-override-table` с roving `tabindex`
 *   и навигацией `ArrowLeft`/`ArrowRight`, контекстное меню `#channel-notif-menu`
 *   с возвратом фокуса по `Escape`, IPC `update_guild_notification_settings` и
 *   `update_channel_notification_override`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  MUTE_TIMER_PRESETS,
  MUTE_TIMER_OPTIONS,
  MESSAGE_NOTIFICATION_LEVELS,
  NOTIFY_HIGHLIGHTS,
  GUILD_NOTIFICATION_FLAGS,
  CHANNEL_NOTIFICATION_FLAGS,
  GUILD_UNREAD_MASK,
  CHANNEL_UNREAD_MASK,
  LARGE_GUILD_MEMBER_THRESHOLD,
  parseStrictBool,
  buildMuteConfig,
  evaluateMuteState,
  setUnreadFlag,
  clearUnreadFlags,
  normalizeChannelOverrides,
  hasVisibleNotificationOverride,
  mergeGuildSettingsUpdate,
  resolveEffectiveNotifications,
  evaluateIncomingMessageNotification,
  computeChannelIndicatorState,
  applyChannelIndicatorToDom,
  computeGuildRailIndicatorState,
  applyGuildRailIndicatorToDom,
  createNotificationSettingsController,
} from "../../rust/gui-spike/ui/features/notifications.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const rootDir = new URL("../../", import.meta.url);
const readyPath = fileURLToPath(
  new URL("fixtures/gateway/by-type/002-READY.json", rootDir),
);
const specPath = fileURLToPath(
  new URL("waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md", rootDir),
);
const rePath = fileURLToPath(new URL("waves/RW-40/NOTIFICATIONS-RE.md", rootDir));

const READY = JSON.parse(readFileSync(readyPath, "utf8"));
const SPEC_TEXT = readFileSync(specPath, "utf8");
const RE_TEXT = readFileSync(rePath, "utf8");

/** @type {any[]} */
const READY_ENTRIES = READY?.d?.user_guild_settings?.entries ?? [];

/** Момент времени после захвата дампа (2026-10-30T19:00:00.000Z). */
const NOW_MS = Date.parse("2026-10-30T19:00:00.000Z");

/**
 * Создаёт записывающий мок IPC `invoke`.
 * @param {(cmd: string, args: any) => Promise<any>} [customImpl]
 */
function createInvokeMock(customImpl) {
  /** @type {Array<{ cmd: string, args: any }>} */
  const calls = [];
  /**
   * @param {string} cmd
   * @param {any} args
   * @returns {Promise<any>}
   */
  const invoke = (cmd, args) => {
    calls.push({ cmd, args });
    if (customImpl) return customImpl(cmd, args);
    return Promise.resolve({ ok: true });
  };
  return { calls, invoke };
}

/**
 * Создаёт чистый DOM-документ шима и контроллер настроек уведомлений.
 * @param {{
 *   guildId?: string,
 *   guildName?: string,
 *   memberCount?: number,
 *   guildEntry?: any,
 *   channels?: any[],
 *   invoke?: (cmd: string, args: any) => Promise<any>,
 *   now?: () => number,
 * }} [opts]
 */
function setupController(opts = {}) {
  const doc = new ShimDocument();
  const statusNode = doc.createElement("div");
  statusNode.id = "server-status";
  statusNode.setAttribute("role", "status");
  doc.body.append(statusNode);

  const triggerBtn = doc.createElement("button");
  triggerBtn.id = "guild-menu-trigger";
  triggerBtn.setAttribute("type", "button");
  triggerBtn.setAttribute("data-action", "guild-menu");
  doc.body.append(triggerBtn);

  const channelBellBtn = doc.createElement("button");
  channelBellBtn.id = "chat-header-bell";
  channelBellBtn.setAttribute("type", "button");
  channelBellBtn.setAttribute("data-action", "channel-notif-menu");
  channelBellBtn.setAttribute("aria-haspopup", "menu");
  channelBellBtn.setAttribute("aria-expanded", "false");
  channelBellBtn.setAttribute("aria-controls", "channel-notif-menu");
  doc.body.append(channelBellBtn);

  const { calls, invoke } = createInvokeMock(opts.invoke);

  const defaultChannels = [
    { id: "cat-news", name: "НОВОСТИ", type: 4, parent_id: null, position: 0 },
    { id: "ch-announce", name: "объявления", type: 0, parent_id: "cat-news", position: 1 },
    { id: "ch-general", name: "общий", type: 0, parent_id: "cat-news", position: 2 },
    { id: "ch-flood", name: "флудилка", type: 0, parent_id: null, position: 3 },
  ];

  const defaultEntry = {
    guild_id: opts.guildId ?? "1014537851418980372",
    message_notifications: 1,
    suppress_everyone: false,
    suppress_roles: false,
    muted: false,
    mute_config: null,
    mobile_push: true,
    notify_highlights: 0,
    flags: 0,
    hide_muted_channels: false,
    mute_scheduled_events: false,
    version: 331,
    channel_overrides: [
      {
        channel_id: "ch-announce",
        collapsed: false,
        flags: 4096,
        message_notifications: 1,
        mute_config: null,
        muted: false,
      },
      {
        // Запись только ради collapsed=true и OPT_IN_ENABLED=4096 (не должна попадать в таблицу)
        channel_id: "ch-general",
        collapsed: true,
        flags: 4096,
        message_notifications: 3,
        mute_config: null,
        muted: false,
      },
    ],
  };

  const controller = createNotificationSettingsController({
    document: /** @type {any} */ (doc),
    invoke,
    guildId: opts.guildId ?? "1014537851418980372",
    guildName: opts.guildName ?? "ITSKIDON PC BOOST",
    memberCount: opts.memberCount ?? 1200,
    guildEntry: opts.guildEntry ?? defaultEntry,
    channels: opts.channels ?? defaultChannels,
    now: opts.now ?? (() => NOW_MS),
  });

  return {
    doc,
    statusNode,
    triggerBtn,
    channelBellBtn,
    calls,
    invoke,
    controller,
  };
}

describe("N-05 UI Notifications: константы, пресеты таймера и оракул 002-READY.json", () => {
  it("MUTE_TIMER_PRESETS и перечисления совпадают с RE-оракулом и спецификацией", () => {
    assert.ok(
      RE_TEXT.includes("ALL_MESSAGES=0") && RE_TEXT.includes("ONLY_MENTIONS=1"),
      "в NOTIFICATIONS-RE.md должны быть коды message_notifications",
    );
    assert.ok(
      SPEC_TEXT.includes("selected_time_window") && SPEC_TEXT.includes("86400"),
      "в NOTIFICATIONS-SPEC.md должны быть пресеты mute_config",
    );

    assert.deepEqual(
      [...MUTE_TIMER_PRESETS],
      [900, 3600, 10800, 28800, 86400, -1],
      "6 канонических пресетов длительности mute (15м, 1ч, 3ч, 8ч, 24ч, бессрочно)",
    );
    assert.equal(MUTE_TIMER_OPTIONS.length, 6);
    assert.deepEqual(
      MUTE_TIMER_OPTIONS.map((opt) => opt.id),
      ["15-minutes", "1-hour", "3-hours", "8-hours", "24-hours", "forever"],
    );
    assert.deepEqual(
      MUTE_TIMER_OPTIONS.map((opt) => opt.selected_time_window),
      [900, 3600, 10800, 28800, 86400, -1],
    );

    assert.deepEqual(MESSAGE_NOTIFICATION_LEVELS, {
      ALL_MESSAGES: 0,
      ONLY_MENTIONS: 1,
      NO_MESSAGES: 2,
      NULL: 3,
    });
    assert.deepEqual(NOTIFY_HIGHLIGHTS, {
      NULL: 0,
      DISABLED: 1,
      ENABLED: 2,
    });
    assert.deepEqual(GUILD_NOTIFICATION_FLAGS, {
      UNREADS_ALL_MESSAGES: 2048,
      UNREADS_ONLY_MENTIONS: 4096,
      OPT_IN_CHANNELS_OFF: 8192,
      OPT_IN_CHANNELS_ON: 16384,
    });
    assert.deepEqual(CHANNEL_NOTIFICATION_FLAGS, {
      UNREADS_ONLY_MENTIONS: 512,
      UNREADS_ALL_MESSAGES: 1024,
      FAVORITED: 2048,
      OPT_IN_ENABLED: 4096,
      NEW_FORUM_THREADS_OFF: 8192,
      NEW_FORUM_THREADS_ON: 16384,
    });
    assert.equal(GUILD_UNREAD_MASK, 2048 | 4096);
    assert.equal(CHANNEL_UNREAD_MASK, 512 | 1024);
    assert.equal(LARGE_GUILD_MEMBER_THRESHOLD, 2500);
  });

  it("buildMuteConfig и evaluateMuteState: расчёт истечения таймера, бессрочный mute и строгий разбор ETF-строк", () => {
    // Пресет 1 час (3600 с)
    const cfg1h = buildMuteConfig(3600, NOW_MS);
    assert.equal(cfg1h.selected_time_window, 3600);
    assert.equal(cfg1h.end_time, new Date(NOW_MS + 3_600_000).toISOString());

    // Пресет «Пока не включу» (-1) -> end_time: null
    const cfgForever = buildMuteConfig(-1, NOW_MS);
    assert.equal(cfgForever.selected_time_window, -1);
    assert.equal(cfgForever.end_time, null);

    // Бессрочный mute без mute_config (muted: true, mute_config: null)
    const indefiniteNoCfg = evaluateMuteState({ muted: true, mute_config: null }, NOW_MS);
    assert.equal(indefiniteNoCfg.isMuted, true);
    assert.equal(indefiniteNoCfg.isTemporarilyMuted, false);
    assert.equal(indefiniteNoCfg.isExpired, false);
    assert.equal(indefiniteNoCfg.remainingMs, null);

    // Бессрочный mute со строковыми атомами дампа ("true", end_time: "nil")
    const indefiniteNilAtom = evaluateMuteState(
      { muted: "true", mute_config: { end_time: "nil", selected_time_window: -1 } },
      NOW_MS,
    );
    assert.equal(indefiniteNilAtom.isMuted, true);
    assert.equal(indefiniteNilAtom.isTemporarilyMuted, false);
    assert.equal(indefiniteNilAtom.selectedTimeWindow, -1);

    // Активный временный mute (ещё 15 минут)
    const activeTemp = evaluateMuteState(
      {
        muted: true,
        mute_config: {
          end_time: new Date(NOW_MS + 900_000).toISOString(),
          selected_time_window: 900,
        },
      },
      NOW_MS,
    );
    assert.equal(activeTemp.isMuted, true);
    assert.equal(activeTemp.isTemporarilyMuted, true);
    assert.equal(activeTemp.isExpired, false);
    assert.equal(activeTemp.remainingMs, 900_000);

    // Истёкший mute из дампа (2021 год, микросекунды + смещение)
    const expired2021 = evaluateMuteState(
      {
        muted: "true",
        mute_config: {
          end_time: "2021-02-27T17:14:08.418000+00:00",
          selected_time_window: 900,
        },
      },
      NOW_MS,
    );
    assert.equal(expired2021.isMuted, false, "истёкший end_time обязан снимать эффективный mute");
    assert.equal(expired2021.isTemporarilyMuted, false);
    assert.equal(expired2021.isExpired, true);

    // Строгий разбор булевых: строка "false" не должна становиться true (закрытие D8)
    assert.equal(parseStrictBool("false", true), false);
    assert.equal(parseStrictBool("true", false), true);
    assert.equal(parseStrictBool("unexpected-atom", false), false);
    const stringFalseEntry = evaluateMuteState({ muted: "false", mute_config: null }, NOW_MS);
    assert.equal(stringFalseEntry.isMuted, false);
  });

  it("сверка парсера с реальным дампом 002-READY.json (58 гильдий, 1328 overrides, 3 истёкших mute)", () => {
    assert.equal(READY_ENTRIES.length, 58, "в READY ровно 58 записей user_guild_settings");

    let totalOverrides = 0;
    let rawMutedGuilds = 0;
    let rawMutedGuildsWithCfg = 0;
    let rawMutedChannels = 0;
    let effectiveMutedChannels = 0;
    let expiredChannels = 0;

    for (const entry of READY_ENTRIES) {
      const guildMutedRaw = parseStrictBool(entry.muted, false);
      if (guildMutedRaw) {
        rawMutedGuilds += 1;
        if (entry.mute_config && entry.mute_config !== "nil") {
          rawMutedGuildsWithCfg += 1;
        }
      }
      const overrides = entry.channel_overrides ?? [];
      totalOverrides += overrides.length;
      for (const ov of overrides) {
        if (parseStrictBool(ov.muted, false)) {
          rawMutedChannels += 1;
        }
        const state = evaluateMuteState(ov, NOW_MS);
        if (state.isMuted) effectiveMutedChannels += 1;
        if (state.isExpired) expiredChannels += 1;
      }
    }

    assert.equal(totalOverrides, 1328, "в READY ровно 1328 channel_overrides");
    assert.equal(rawMutedGuilds, 22, "22 гильдии с muted=true в дампе");
    assert.equal(rawMutedGuildsWithCfg, 21, "у 21 из 22 заглушённых гильдий есть mute_config");
    assert.equal(rawMutedChannels, 42, "42 канала с muted=true в дампе");
    assert.equal(expiredChannels, 3, "3 канала в дампе имеют исторический истёкший end_time 2021 г.");
    assert.equal(effectiveMutedChannels, 39, "39 эффективно заглушённых каналов (42 - 3 истёкших)");
  });
});

describe("N-05 UI Notifications: каскад разрешения, allowNoMessages и матрица бейджей упоминаний", () => {
  it("resolveEffectiveNotifications: каскад канал (!=3) -> родительская категория (!=3) -> гильдия -> дефолт", () => {
    const guildEntry = {
      guild_id: "g-1",
      message_notifications: 1, // ONLY_MENTIONS
      muted: false,
      suppress_everyone: false,
      suppress_roles: false,
      flags: 0,
      channel_overrides: [
        { channel_id: "cat-1", message_notifications: 0, muted: false }, // ALL_MESSAGES на категории
        { channel_id: "ch-explicit-2", message_notifications: 2, muted: false }, // NO_MESSAGES на канале
        { channel_id: "ch-inherit-cat", message_notifications: 3, muted: false }, // NULL -> наследует cat-1 (0)
        { channel_id: "ch-inherit-guild", message_notifications: 3, muted: false }, // NULL без категории -> гильдия (1)
      ],
    };

    // 1. Явный уровень на канале (2 = NO_MESSAGES) перекрывает категорию (0) и гильдию (1)
    const resChannel = resolveEffectiveNotifications({
      guildEntry,
      channel: { id: "ch-explicit-2", parent_id: "cat-1" },
      nowMs: NOW_MS,
    });
    assert.equal(resChannel.resolvedMessageNotifications, 2);
    assert.equal(resChannel.source, "channel");
    assert.equal(resChannel.allowNoMessages, true, "при NO_MESSAGES allowNoMessages === true");
    assert.equal(resChannel.allowDesktopNotifications, false);

    // 2. На канале NULL (3) -> берётся родительская категория cat-1 (0 = ALL_MESSAGES)
    const resCategory = resolveEffectiveNotifications({
      guildEntry,
      channel: { id: "ch-inherit-cat", parent_id: "cat-1" },
      nowMs: NOW_MS,
    });
    assert.equal(resCategory.resolvedMessageNotifications, 0);
    assert.equal(resCategory.source, "category");
    assert.equal(resCategory.allowNoMessages, false);
    assert.equal(resCategory.allowDesktopNotifications, true);
    assert.equal(resCategory.allowAllMessages, true);

    // 3. На канале NULL (3), у категории нет override -> берётся гильдия (1 = ONLY_MENTIONS)
    const resGuild = resolveEffectiveNotifications({
      guildEntry,
      channel: { id: "ch-inherit-guild", parent_id: "cat-missing" },
      nowMs: NOW_MS,
    });
    assert.equal(resGuild.resolvedMessageNotifications, 1);
    assert.equal(resGuild.source, "guild");
    assert.equal(resGuild.allowNoMessages, false);
    assert.equal(resGuild.allowDesktopNotifications, true);
    assert.equal(resGuild.allowAllMessages, false);

    // 4. Неизвестное значение на уровне гильдии (например, 3 или 99) фоллбэчится на defaultMessageNotifications (1), а не на 0
    const resFallback = resolveEffectiveNotifications({
      guildEntry: { guild_id: "g-bad", message_notifications: 3 },
      channel: { id: "ch-any", parent_id: null },
      defaultMessageNotifications: 1,
      nowMs: NOW_MS,
    });
    assert.equal(resFallback.resolvedMessageNotifications, 1);
    assert.equal(resFallback.source, "default");
  });

  it("allowNoMessages и матрица упоминаний (@user, @role, @everyone/@here): звук/desktop vs бейдж mention_count", () => {
    // Сценарий А: канал на уровне NO_MESSAGES (2), но не заглушен, suppress_everyone=false, suppress_roles=false
    const guildNoMessages = {
      guild_id: "g-1",
      message_notifications: 2, // NO_MESSAGES
      muted: false,
      suppress_everyone: false,
      suppress_roles: false,
    };

    // Прямое @user при NO_MESSAGES: desktop подавлен (allowNoMessages=true), но бейдж РАСТЁТ!
    const dmUserAtNoMessages = evaluateIncomingMessageNotification({
      guildEntry: guildNoMessages,
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: true, mentionsRole: false, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(dmUserAtNoMessages.allowNoMessages, true);
    assert.equal(dmUserAtNoMessages.shouldNotifyDesktop, false, "NO_MESSAGES подавляет звук/desktop даже для @user");
    assert.equal(dmUserAtNoMessages.incrementsMentionBadge, true, "@user всегда увеличивает бейдж mention_count");
    assert.equal(dmUserAtNoMessages.mentionBadgeDelta, 1);

    // Сценарий Б: сервер полностью заглушен (muted=true)
    const guildMuted = {
      guild_id: "g-1",
      message_notifications: 0, // ALL_MESSAGES
      muted: true,
      mute_config: null,
      suppress_everyone: false,
      suppress_roles: false,
    };
    const userAtMuted = evaluateIncomingMessageNotification({
      guildEntry: guildMuted,
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: true, mentionsRole: false, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(userAtMuted.allowNoMessages, true);
    assert.equal(userAtMuted.shouldNotifyDesktop, false, "mute сервера подавляет desktop-уведомление");
    assert.equal(userAtMuted.incrementsMentionBadge, true, "прямое @user увеличивает бейдж даже при muted=true");

    // Сценарий В: @role при suppress_roles=false vs suppress_roles=true
    const roleAllowed = evaluateIncomingMessageNotification({
      guildEntry: { ...guildNoMessages, message_notifications: 1, suppress_roles: false },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: true, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(roleAllowed.incrementsMentionBadge, true);
    assert.equal(roleAllowed.shouldNotifyDesktop, true);

    const roleSuppressed = evaluateIncomingMessageNotification({
      guildEntry: { ...guildNoMessages, message_notifications: 0, suppress_roles: true },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: true, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(roleSuppressed.incrementsMentionBadge, false, "suppress_roles=true блокирует бейдж от @role");
    assert.equal(roleSuppressed.shouldNotifyDesktop, false, "suppress_roles=true блокирует desktop от @role");

    // Сценарий Г: @everyone/@here при suppress_everyone=false vs suppress_everyone=true
    const everyoneAllowed = evaluateIncomingMessageNotification({
      guildEntry: { ...guildNoMessages, message_notifications: 1, suppress_everyone: false },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: false, mentionsEveryone: true },
      nowMs: NOW_MS,
    });
    assert.equal(everyoneAllowed.incrementsMentionBadge, true);
    assert.equal(everyoneAllowed.shouldNotifyDesktop, true);

    const everyoneSuppressed = evaluateIncomingMessageNotification({
      guildEntry: { ...guildNoMessages, message_notifications: 0, suppress_everyone: true },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: false, mentionsEveryone: true },
      nowMs: NOW_MS,
    });
    assert.equal(everyoneSuppressed.incrementsMentionBadge, false, "suppress_everyone=true блокирует бейдж");
    assert.equal(everyoneSuppressed.shouldNotifyDesktop, false, "suppress_everyone=true блокирует desktop");

    // Сценарий Д: обычное сообщение без упоминаний уведомляет только при ALL_MESSAGES (0) и !allowNoMessages,
    // а marksUnread === true только когда эффективный unread разрешает ALL_MESSAGES (NOTIFICATIONS-SPEC §1.5.3 строка 4)
    const plainAtMentions = evaluateIncomingMessageNotification({
      guildEntry: { ...guildNoMessages, message_notifications: 1 },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: false, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(plainAtMentions.incrementsMentionBadge, false);
    assert.equal(plainAtMentions.shouldNotifyDesktop, false);
    assert.equal(
      plainAtMentions.marksUnread,
      false,
      "при ONLY_MENTIONS (без UNREADS_ALL_MESSAGES) обычное сообщение не помечает канал непрочитанным (§1.5.3 строка 4)",
    );

    const plainAtAll = evaluateIncomingMessageNotification({
      guildEntry: { ...guildNoMessages, message_notifications: 0 },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: false, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(plainAtAll.shouldNotifyDesktop, true);
    assert.equal(plainAtAll.marksUnread, true, "при ALL_MESSAGES обычное сообщение помечает канал непрочитанным");

    // Гибридный пресет (§1.2.4): message_notifications = 1 (ONLY_MENTIONS), но бит флага UNREADS_ALL_MESSAGES (1024) выставлен
    const plainAtHybridUnreadAll = evaluateIncomingMessageNotification({
      guildEntry: {
        ...guildNoMessages,
        message_notifications: 1,
        channel_overrides: [
          {
            channel_id: "ch-1",
            message_notifications: 1,
            muted: false,
            flags: CHANNEL_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES,
          },
        ],
      },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: false, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(plainAtHybridUnreadAll.shouldNotifyDesktop, false);
    assert.equal(
      plainAtHybridUnreadAll.marksUnread,
      true,
      "бит UNREADS_ALL_MESSAGES (1024) разрешает точку непрочитанного при message_notifications=1",
    );

    // Явный бит UNREADS_ONLY_MENTIONS (512) блокирует точку непрочитанного от обычных сообщений
    const plainWithExplicitUnreadMentions = evaluateIncomingMessageNotification({
      guildEntry: {
        ...guildNoMessages,
        message_notifications: 0,
        channel_overrides: [
          {
            channel_id: "ch-1",
            message_notifications: 0,
            muted: false,
            flags: CHANNEL_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS,
          },
        ],
      },
      channel: { id: "ch-1", parent_id: null },
      message: { mentionsUser: false, mentionsRole: false, mentionsEveryone: false },
      nowMs: NOW_MS,
    });
    assert.equal(plainWithExplicitUnreadMentions.marksUnread, false);
  });

  it("битовые флаги UNREADS_*, двойная форма channel_overrides и фильтр видимых строк таблицы", () => {
    // Сохранение старших битов OPT_IN_ENABLED (4096) и FAVORITED (2048) при смене UNREADS_*
    const initialFlags = CHANNEL_NOTIFICATION_FLAGS.OPT_IN_ENABLED | CHANNEL_NOTIFICATION_FLAGS.FAVORITED; // 6144
    const withUnreadMentions = setUnreadFlag(
      initialFlags,
      CHANNEL_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS,
      CHANNEL_UNREAD_MASK,
    );
    assert.equal(withUnreadMentions, 6144 | 512);

    const switchedToUnreadAll = setUnreadFlag(
      withUnreadMentions,
      CHANNEL_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES,
      CHANNEL_UNREAD_MASK,
    );
    assert.equal(
      switchedToUnreadAll,
      6144 | 1024,
      "mD обязан сбросить старый бит 512, выставить 1024 и сохранить 4096|2048",
    );

    const cleared = clearUnreadFlags(switchedToUnreadAll, CHANNEL_UNREAD_MASK);
    assert.equal(cleared, 6144, "PW обязан сбросить только пару 512|1024, не трогая 4096|2048");

    // Нормализация массива и словаря channel_overrides
    const fromArray = normalizeChannelOverrides([
      { channel_id: "100", collapsed: true, flags: 4096, message_notifications: 3, muted: false },
      { channel_id: "200", collapsed: false, flags: 4096, message_notifications: 1, muted: false },
    ]);
    const fromMap = normalizeChannelOverrides({
      "100": { channel_id: "100", collapsed: true, flags: 4096, message_notifications: 3, muted: false },
      "200": { channel_id: "200", collapsed: false, flags: 4096, message_notifications: 1, muted: false },
    });
    assert.deepEqual(Object.keys(fromArray), ["100", "200"]);
    assert.deepEqual(fromArray, fromMap);

    // Фильтр таблицы: канал 100 (только collapsed=true и OPT_IN=4096, level=3, muted=false) НЕ виден
    assert.equal(hasVisibleNotificationOverride(fromArray["100"], NOW_MS), false);
    // Канал 200 (level=1 != 3) ВИДЕН
    assert.equal(hasVisibleNotificationOverride(fromArray["200"], NOW_MS), true);

    // Слияние частичного USER_GUILD_SETTINGS_UPDATE без channel_overrides не затирает существующие overrides
    const merged = mergeGuildSettingsUpdate(
      {
        guild_id: "g-1",
        message_notifications: 1,
        suppress_everyone: false,
        version: 10,
        channel_overrides: fromArray,
      },
      {
        guild_id: "g-1",
        suppress_everyone: true,
        version: 11,
      },
      1,
    );
    assert.equal(merged.suppress_everyone, true);
    assert.equal(Object.keys(merged.channel_overrides).length, 2, "существующие overrides обязаны сохраниться");
  });
});

describe("N-05 UI Notifications: индикаторы непрочитанного и бейджей в #guild-rail и #channel-tree", () => {
  it("computeChannelIndicatorState и applyChannelIndicatorToDom: точка, бейдж 99+, mute и hide_muted_channels", () => {
    const doc = new ShimDocument();
    const btn = doc.createElement("button");
    btn.className = "channel-item";

    // 1. Непрочитанный канал с 3 упоминаниями
    const unreadWithMentions = computeChannelIndicatorState({
      name: "объявления",
      unread: true,
      mentionCount: 3,
      isChannelMuted: false,
      isCategoryMuted: false,
      resolvedMessageNotifications: 1,
      hideMutedChannels: false,
      isCurrentPage: false,
    });
    assert.equal(unreadWithMentions.unread, true);
    assert.equal(unreadWithMentions.hasMention, true);
    assert.equal(unreadWithMentions.badgeText, "3");
    assert.equal(unreadWithMentions.showUnreadDot, false, "при бейдже упоминаний точка ::after не дублируется");
    assert.equal(unreadWithMentions.ariaLabel, "объявления, непрочитано, 3 упоминания");

    applyChannelIndicatorToDom(btn, unreadWithMentions);
    assert.equal(btn.classList.contains("is-unread"), true);
    assert.equal(btn.classList.contains("has-mention"), true);
    assert.equal(btn.getAttribute("data-mention-count"), "3");
    assert.equal(btn.querySelector(".mention-badge")?.textContent, "3");
    assert.equal(btn.querySelector(".mention-badge")?.getAttribute("aria-hidden"), "true");

    // 2. Заглушённый канал без упоминаний при hide_muted_channels=true скрывается
    const mutedNoMentions = computeChannelIndicatorState({
      name: "флудилка",
      unread: true,
      mentionCount: 0,
      isChannelMuted: true,
      isCategoryMuted: false,
      resolvedMessageNotifications: 2,
      hideMutedChannels: true,
      isCurrentPage: false,
    });
    assert.equal(mutedNoMentions.isMuted, true);
    assert.equal(mutedNoMentions.showUnreadDot, false, "заглушённый канал не показывает точку непрочитанного");
    assert.equal(mutedNoMentions.showMuteIcon, true);
    assert.equal(mutedNoMentions.hiddenByHideMuted, true, "заглушённый канал без упоминаний скрыт при hide_muted_channels");
    assert.equal(mutedNoMentions.ariaLabel, "флудилка, канал заглушён");

    // 3. Заглушённый канал с прямым упоминанием (mentionCount=150 -> "99+") НЕ скрывается при hide_muted_channels=true
    const mutedWithMentions = computeChannelIndicatorState({
      name: "флудилка",
      unread: true,
      mentionCount: 150,
      isChannelMuted: true,
      isCategoryMuted: false,
      resolvedMessageNotifications: 2,
      hideMutedChannels: true,
      isCurrentPage: false,
    });
    assert.equal(mutedWithMentions.isMuted, true);
    assert.equal(mutedWithMentions.hasMention, true);
    assert.equal(mutedWithMentions.badgeText, "99+");
    assert.equal(
      mutedWithMentions.hiddenByHideMuted,
      false,
      "заглушённый канал с прямым упоминанием остаётся видимым при hide_muted_channels=true",
    );

    // 4. Совместимость applyChannelIndicatorToDom с #channel-tree[data-hide-muted="true"] и data-muted-hidden / hidden (§2.2, §3.2)
    const channelTree = doc.createElement("div");
    channelTree.id = "channel-tree";
    channelTree.setAttribute("data-hide-muted", "true");
    const treeItem = doc.createElement("li");
    const mutedBtnInTree = doc.createElement("button");
    mutedBtnInTree.className = "channel-item";
    treeItem.append(mutedBtnInTree);
    channelTree.append(treeItem);
    doc.body.append(channelTree);

    const mutedStateDefault = computeChannelIndicatorState({
      name: "архив",
      unread: false,
      mentionCount: 0,
      isChannelMuted: true,
      isCategoryMuted: false,
      resolvedMessageNotifications: 2,
    });
    applyChannelIndicatorToDom(mutedBtnInTree, mutedStateDefault);
    assert.equal(mutedBtnInTree.getAttribute("data-muted"), "true");
    assert.equal(
      mutedBtnInTree.getAttribute("data-muted-hidden"),
      "true",
      "внутри #channel-tree[data-hide-muted='true'] заглушённый канал без упоминаний получает data-muted-hidden='true'",
    );
    assert.equal(
      mutedBtnInTree.getAttribute("hidden"),
      "",
      "внутри #channel-tree[data-hide-muted='true'] заглушённый канал скрывается атрибутом hidden",
    );

    // Если канал активен (aria-current="page") — он остаётся видимым даже внутри #channel-tree[data-hide-muted="true"]
    mutedBtnInTree.setAttribute("aria-current", "page");
    applyChannelIndicatorToDom(mutedBtnInTree, mutedStateDefault);
    assert.equal(mutedBtnInTree.getAttribute("data-muted"), "true");
    assert.equal(mutedBtnInTree.getAttribute("data-muted-hidden"), "false");
    assert.equal(mutedBtnInTree.getAttribute("hidden"), null);

    // При отключении фильтра #channel-tree[data-hide-muted="false"] канал снова виден
    mutedBtnInTree.removeAttribute("aria-current");
    channelTree.setAttribute("data-hide-muted", "false");
    applyChannelIndicatorToDom(mutedBtnInTree, mutedStateDefault);
    assert.equal(mutedBtnInTree.getAttribute("data-muted-hidden"), "false");
    assert.equal(mutedBtnInTree.getAttribute("hidden"), null);
  });

  it("computeGuildRailIndicatorState и applyGuildRailIndicatorToDom: левая пилюля и бейдж на плитке сервера", () => {
    const doc = new ShimDocument();
    const tile = doc.createElement("button");
    tile.className = "guild-item";

    // Незаглушённый сервер с непрочитанным и 5 упоминаниями
    const activeGuild = computeGuildRailIndicatorState({
      name: "ITSKIDON PC BOOST",
      unread: true,
      mentionCount: 5,
      isGuildMuted: false,
      isSelected: false,
    });
    assert.equal(activeGuild.showUnreadPill, true);
    assert.equal(activeGuild.showMentionBadge, true);
    assert.equal(activeGuild.badgeText, "5");
    applyGuildRailIndicatorToDom(tile, activeGuild);
    assert.equal(tile.getAttribute("data-unread"), "true");
    assert.equal(tile.getAttribute("data-muted"), "false");
    assert.equal(tile.getAttribute("data-mention-count"), "5");
    assert.equal(tile.querySelector(".guild-mention-badge")?.textContent, "5");

    // Заглушённый сервер с упоминаниями: пилюля скрыта, но бейдж упоминаний виден
    const mutedGuildWithMention = computeGuildRailIndicatorState({
      name: "ITSKIDON PC BOOST",
      unread: true,
      mentionCount: 2,
      isGuildMuted: true,
      isSelected: false,
    });
    assert.equal(mutedGuildWithMention.showUnreadPill, false, "у заглушённой гильдии нет левой пилюли непрочитанного");
    assert.equal(mutedGuildWithMention.showMentionBadge, true, "бейдж упоминаний остаётся видимым при mute гильдии");
  });
});

describe("N-05 UI Notifications: контроллер createNotificationSettingsController (#guild-notif-modal и #channel-notif-menu)", () => {
  it("открывает модалку #guild-notif-modal, рендерит секции, радиогруппу .notif-level-list и баннер >= 2500 участников", async () => {
    const { doc, triggerBtn, calls, controller } = setupController({
      memberCount: 3000,
    });

    triggerBtn.focus();
    controller.openGuildModal({ triggerElement: triggerBtn });

    const backdrop = doc.getElementById("guild-notifications-dialog");
    const modal = doc.getElementById("guild-notif-modal");
    assert.ok(backdrop, "должен существовать #guild-notifications-dialog");
    assert.ok(modal, "должен существовать #guild-notif-modal");
    assert.equal(backdrop.getAttribute("hidden"), null, "модалка должна быть открыта");
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");
    assert.equal(doc.getElementById("guild-notif-subtitle")?.textContent, "ITSKIDON PC BOOST");

    // Первичный фокус на тумблере #guild-notif-mute-switch (NOTIFICATIONS-SPEC §5.2 п.4)
    const muteSwitch = doc.getElementById("guild-notif-mute-switch");
    assert.ok(muteSwitch);
    assert.equal(doc.activeElement?.id, muteSwitch.id, "первичный фокус должен ставиться на #guild-notif-mute-switch");

    // Радиогруппа базового уровня (.notif-level-list): выбран level=1 (tabindex="0"), остальные tabindex="-1"
    const levelCards = modal.querySelectorAll(".notif-level-card");
    assert.equal(levelCards.length, 3);
    assert.equal(levelCards[0]?.getAttribute("aria-checked"), "false");
    assert.equal(levelCards[0]?.getAttribute("tabindex"), "-1");
    assert.equal(levelCards[1]?.getAttribute("aria-checked"), "true");
    assert.equal(levelCards[1]?.getAttribute("tabindex"), "0");
    assert.equal(levelCards[2]?.getAttribute("aria-checked"), "false");
    assert.equal(levelCards[2]?.getAttribute("tabindex"), "-1");

    const largeWarning = doc.getElementById("guild-notif-large-warning");
    assert.ok(largeWarning);
    assert.notEqual(largeWarning.getAttribute("hidden"), null, "при level=1 баннер большого сервера скрыт");

    // Переключение на level=0 («Все сообщения») через клавишу ArrowUp на радиогруппе
    levelCards[1]?.dispatchEvent("keydown", { key: "ArrowUp" });
    await Promise.resolve();

    assert.equal(calls.length, 1);
    assert.equal(calls[0]?.cmd, "update_guild_notification_settings");
    assert.deepEqual(calls[0]?.args.patch, { message_notifications: 0 });
    assert.deepEqual(calls[0]?.args.body, {
      guilds: { "1014537851418980372": { message_notifications: 0 } },
    });

    assert.equal(levelCards[0]?.getAttribute("aria-checked"), "true");
    assert.equal(levelCards[0]?.getAttribute("tabindex"), "0");
    assert.equal(levelCards[1]?.getAttribute("aria-checked"), "false");
    assert.equal(levelCards[1]?.getAttribute("tabindex"), "-1");
    assert.equal(
      largeWarning.getAttribute("hidden"),
      null,
      "при выборе level=0 на сервере с memberCount >= 2500 показывается предупреждение Dh5p5j",
    );
  });

  it("тумблеры подавления (suppress_everyone, suppress_roles, notify_highlights 2/1) и отсутствие оптимистичной мутации до ответа IPC", async () => {
    /** @type {((val: any) => void) | null} */
    let resolveIpc = null;
    const { doc, calls, controller } = setupController({
      invoke: () =>
        new Promise((resolve) => {
          resolveIpc = resolve;
        }),
    });

    controller.openGuildModal();
    const modal = doc.getElementById("guild-notif-modal");
    const suppressEveryone = doc.getElementById("guild-notif-suppress-everyone");
    const highlightsSwitch = doc.getElementById("guild-notif-highlights");
    assert.ok(modal);
    assert.ok(suppressEveryone);
    assert.ok(highlightsSwitch);

    assert.equal(suppressEveryone.getAttribute("aria-checked"), "false");
    assert.equal(highlightsSwitch.getAttribute("aria-checked"), "true", "notify_highlights=0 трактуется как включено");

    // Нажатие Space на тумблере suppress_everyone запускает pending без оптимистичного переключения aria-checked
    const spaceEvent = suppressEveryone.dispatchEvent("keydown", { key: " " });
    assert.equal(spaceEvent.prevented, true, "Space на switch вызывает preventDefault()");
    assert.equal(modal.getAttribute("data-state"), "pending");
    assert.equal(
      suppressEveryone.getAttribute("aria-checked"),
      "false",
      "aria-checked не должен оптимистично меняться до завершения IPC",
    );

    // Завершаем IPC
    assert.ok(resolveIpc);
    /** @type {(val: any) => void} */ (resolveIpc)({ ok: true });
    await Promise.resolve();
    await Promise.resolve();

    assert.equal(modal.getAttribute("data-state"), "ready");
    assert.equal(suppressEveryone.getAttribute("aria-checked"), "true");
    assert.equal(calls[0]?.cmd, "update_guild_notification_settings");
    assert.deepEqual(calls[0]?.args.patch, { suppress_everyone: true });

    // Переключение тумблера notify_highlights отправляет 1 (DISABLED) при выключении и 2 (ENABLED) при включении
    highlightsSwitch.dispatchEvent("keydown", { key: " " });
    /** @type {(val: any) => void} */ (resolveIpc)({ ok: true });
    await Promise.resolve();
    await Promise.resolve();

    assert.equal(highlightsSwitch.getAttribute("aria-checked"), "false");
    assert.deepEqual(calls[1]?.args.patch, { notify_highlights: 1 });

    highlightsSwitch.dispatchEvent("keydown", { key: " " });
    /** @type {(val: any) => void} */ (resolveIpc)({ ok: true });
    await Promise.resolve();
    await Promise.resolve();

    assert.equal(highlightsSwitch.getAttribute("aria-checked"), "true");
    assert.deepEqual(calls[2]?.args.patch, { notify_highlights: 2 });
  });

  it("таблица .notif-override-table: roving tabindex, навигация ArrowLeft/ArrowRight по радио, чекбокс mute и сброс override", async () => {
    const { doc, calls, controller } = setupController();
    controller.openGuildModal();

    const tbody = doc.getElementById("notif-override-tbody");
    assert.ok(tbody);

    // В фикстуре 2 записи в channel_overrides, но у ch-general только collapsed=true и flags=4096 (level=3),
    // поэтому в таблице должна отрендериться ровно 1 видимая строка — ch-announce!
    const rows = tbody.querySelectorAll(".notif-override-row");
    assert.equal(rows.length, 1, "в таблице рендерятся только каналы с реальным переопределением уведомлений");
    assert.equal(rows[0]?.getAttribute("data-channel-id"), "ch-announce");

    const radios = rows[0]?.querySelectorAll(".notif-cell-radio") ?? [];
    assert.equal(radios.length, 3, "в строке 3 радиокнопки уровня (0, 1, 2)");

    // Изначально у ch-announce message_notifications = 1 -> вторая радиокнопка имеет tabindex="0", остальные "-1"
    assert.equal(radios[0]?.getAttribute("aria-checked"), "false");
    assert.equal(radios[0]?.getAttribute("tabindex"), "-1");
    assert.equal(radios[1]?.getAttribute("aria-checked"), "true");
    assert.equal(radios[1]?.getAttribute("tabindex"), "0");
    assert.equal(radios[2]?.getAttribute("aria-checked"), "false");
    assert.equal(radios[2]?.getAttribute("tabindex"), "-1");

    // Нажатие ArrowRight на активной радиокнопке (level=1) переводит фокус и roving tabindex на level=2 («Ничего»)
    radios[1]?.focus();
    radios[1]?.dispatchEvent("keydown", { key: "ArrowRight" });
    await Promise.resolve();

    const updatedRow = tbody.querySelector('.notif-override-row[data-channel-id="ch-announce"]');
    assert.ok(updatedRow);
    const updatedRadios = updatedRow.querySelectorAll(".notif-cell-radio");
    assert.equal(updatedRadios[2]?.getAttribute("aria-checked"), "true");
    assert.equal(updatedRadios[2]?.getAttribute("tabindex"), "0");
    assert.equal(updatedRadios[0]?.getAttribute("tabindex"), "-1");
    assert.equal(updatedRadios[1]?.getAttribute("tabindex"), "-1");
    assert.equal(
      doc.activeElement === updatedRadios[2],
      true,
      "фокус должен перейти на выбранную радиокнопку level=2",
    );

    assert.equal(calls.at(-1)?.cmd, "update_channel_notification_override");
    assert.equal(calls.at(-1)?.args.channelId, "ch-announce");
    assert.deepEqual(calls.at(-1)?.args.override, {
      message_notifications: 2,
      muted: false,
    });

    // Нажатие ArrowLeft на level=2 возвращает на level=1
    updatedRadios[2]?.dispatchEvent("keydown", { key: "ArrowLeft" });
    await Promise.resolve();

    const rowAfterLeft = tbody.querySelector('.notif-override-row[data-channel-id="ch-announce"]');
    const radiosAfterLeft = rowAfterLeft?.querySelectorAll(".notif-cell-radio") ?? [];
    assert.equal(radiosAfterLeft[1]?.getAttribute("aria-checked"), "true");
    assert.equal(radiosAfterLeft[1]?.getAttribute("tabindex"), "0");
    assert.equal(radiosAfterLeft[2]?.getAttribute("tabindex"), "-1");

    // Активация чекбокса mute клавишей Space
    const muteCheckbox = rowAfterLeft?.querySelector(".notif-cell-checkbox");
    assert.ok(muteCheckbox);
    assert.equal(muteCheckbox.getAttribute("aria-checked"), "false");
    const chkEvent = muteCheckbox.dispatchEvent("keydown", { key: " " });
    assert.equal(chkEvent.prevented, true);
    await Promise.resolve();

    const rowAfterMute = tbody.querySelector('.notif-override-row[data-channel-id="ch-announce"]');
    assert.equal(
      rowAfterMute?.querySelector(".notif-cell-checkbox")?.getAttribute("aria-checked"),
      "true",
    );
    assert.equal(calls.at(-1)?.cmd, "update_channel_notification_override");
    assert.equal(calls.at(-1)?.args.override.muted, true);

    // Сброс (удаление) override кнопкой .notif-override-remove отправляет { message_notifications: 3, muted: false, flags: 4096 }
    const removeBtn = rowAfterMute?.querySelector(".notif-override-remove");
    assert.ok(removeBtn);
    removeBtn.click();
    await Promise.resolve();

    assert.equal(calls.at(-1)?.cmd, "update_channel_notification_override");
    assert.deepEqual(calls.at(-1)?.args.override, {
      message_notifications: 3,
      muted: false,
      flags: 4096,
    });
    assert.equal(
      tbody.querySelectorAll(".notif-override-row").length,
      0,
      "после сброса в message_notifications=3 и muted=false строка исчезает из видимой таблицы",
    );
    assert.equal(
      doc.getElementById("notif-override-empty")?.getAttribute("hidden"),
      null,
      "показывается пустое состояние #notif-override-empty",
    );

    // P2.5: Сброс override для записи, в которой изначально отсутствовал ключ flags (75 из 1328 записей дампа),
    // всё равно обязан отправлять flags: PW(flags ?? 0) = 0 в теле PATCH (§1.4 п.3, §1.6.3)
    const noFlagsSetup = setupController({
      guildEntry: {
        guild_id: "1014537851418980372",
        message_notifications: 1,
        muted: false,
        version: 1,
        channel_overrides: [
          {
            channel_id: "ch-flood",
            collapsed: false,
            message_notifications: 0,
            mute_config: null,
            muted: false,
          },
        ],
      },
    });
    noFlagsSetup.controller.openGuildModal();
    const noFlagsRemoveBtn = noFlagsSetup.doc.querySelector(
      '.notif-override-row[data-channel-id="ch-flood"] .notif-override-remove',
    );
    assert.ok(noFlagsRemoveBtn);
    noFlagsRemoveBtn.click();
    await Promise.resolve();
    assert.deepEqual(
      noFlagsSetup.calls.at(-1)?.args.override,
      {
        message_notifications: 3,
        muted: false,
        flags: 0,
      },
      "сброс override обязан всегда отправлять поле flags: PW(flags ?? 0), даже если в исходной записи не было ключа flags",
    );
  });

  it("контекстное меню канала #channel-notif-menu: пресеты mute, уровни, roving tabindex и возврат фокуса по Escape (включая NodeList без .filter)", async () => {
    const { doc, channelBellBtn, calls, controller } = setupController();

    const menu = doc.getElementById("channel-notif-menu");
    assert.ok(menu, "должно существовать меню #channel-notif-menu");

    // P1.1: эмулируем браузерный NodeList без Array.prototype.filter на #channel-notif-menu.querySelectorAll
    const rawQuerySelectorAll = menu.querySelectorAll.bind(menu);
    Object.defineProperty(menu, "querySelectorAll", {
      configurable: true,
      value(/** @type {string} */ selector) {
        const arr = rawQuerySelectorAll(selector);
        /** @type {any} */
        const nodeListLike = {
          length: arr.length,
          [Symbol.iterator]: () => arr[Symbol.iterator](),
        };
        for (let i = 0; i < arr.length; i += 1) {
          nodeListLike[i] = arr[i];
        }
        return nodeListLike;
      },
    });
    assert.equal(
      typeof /** @type {any} */ (menu.querySelectorAll('[role="menuitem"]')).filter,
      "undefined",
      "querySelectorAll у #channel-notif-menu должен возвращать NodeList-подобный итерируемый объект без .filter",
    );

    channelBellBtn.focus();
    controller.openChannelMenu({
      channelId: "ch-flood",
      triggerElement: channelBellBtn,
    });

    assert.equal(menu.getAttribute("hidden"), null);
    assert.equal(menu.getAttribute("data-channel-id"), "ch-flood");
    assert.equal(channelBellBtn.getAttribute("aria-expanded"), "true");

    // Проверка roving tabindex: ровно один видимый пункт имеет tabindex="0" и фокус
    const enabledItems = Array.from(
      menu.querySelectorAll('[role="menuitem"], [role="menuitemradio"]'),
    ).filter((item) => item.getAttribute("hidden") === null);
    assert.ok(enabledItems.length >= 10, "6 пресетов mute + 4 опции уровня уведомлений");
    const tabbable = enabledItems.filter((item) => item.getAttribute("tabindex") === "0");
    assert.equal(tabbable.length, 1, "ровно один пункт меню имеет tabindex=0");
    assert.equal(doc.activeElement === tabbable[0], true);

    // Навигация ArrowDown перемещает фокус и tabindex="0" на второй пункт
    tabbable[0]?.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(enabledItems[0]?.getAttribute("tabindex"), "-1");
    assert.equal(enabledItems[1]?.getAttribute("tabindex"), "0");
    assert.equal(doc.activeElement === enabledItems[1], true);

    // Выбор пресета mute «На 1 час» (data-window="3600")
    const preset1h = menu.querySelector('[data-action="channel-mute-preset"][data-window="3600"]');
    assert.ok(preset1h);
    preset1h.click();
    await Promise.resolve();

    assert.equal(calls.at(-1)?.cmd, "update_channel_notification_override");
    assert.equal(calls.at(-1)?.args.channelId, "ch-flood");
    assert.equal(calls.at(-1)?.args.override.muted, true);
    assert.equal(calls.at(-1)?.args.override.mute_config.selected_time_window, 3600);
    assert.equal(
      calls.at(-1)?.args.override.mute_config.end_time,
      new Date(NOW_MS + 3_600_000).toISOString(),
    );

    // Повторно открываем меню и закрываем по Escape — фокус обязан вернуться на channelBellBtn
    controller.openChannelMenu({
      channelId: "ch-flood",
      triggerElement: channelBellBtn,
    });
    let bodyKeydownFired = false;
    doc.body.addEventListener("keydown", () => {
      bodyKeydownFired = true;
    });

    const escEvent = menu.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(escEvent.prevented, true);
    assert.equal(bodyKeydownFired, false, "Escape в меню не должен всплывать на body");
    assert.notEqual(menu.getAttribute("hidden"), null, "меню #channel-notif-menu должно закрыться");
    assert.equal(channelBellBtn.getAttribute("aria-expanded"), "false");
    assert.equal(
      doc.activeElement?.id,
      channelBellBtn.id,
      "после закрытия #channel-notif-menu по Escape фокус обязан вернуться на триггер",
    );
  });

  it("focus trap внутри #guild-notif-modal: Tab и Shift+Tab циклически переключают фокус (NOTIFICATIONS-SPEC §5.2 п.4)", () => {
    const { doc, triggerBtn, controller } = setupController();
    triggerBtn.focus();
    controller.openGuildModal({ triggerElement: triggerBtn });

    const modal = doc.getElementById("guild-notif-modal");
    assert.ok(modal);

    const closeHeaderBtn = modal.querySelector('header [data-action="guild-notif-close"]');
    const muteSwitch = doc.getElementById("guild-notif-mute-switch");
    const activeLevelCard = modal.querySelector('.notif-level-card[tabindex="0"]');
    const doneBtn = modal.querySelector('footer [data-action="guild-notif-close"]');
    assert.ok(closeHeaderBtn);
    assert.ok(muteSwitch);
    assert.ok(activeLevelCard);
    assert.ok(doneBtn);

    // При открытии фокус стоит на #guild-notif-mute-switch
    assert.equal(doc.activeElement, muteSwitch);

    // Tab с #guild-notif-mute-switch переводит фокус на активную радио-карточку уровня (пропуская скрытый select и tabindex="-1")
    const tabFromMute = muteSwitch.dispatchEvent("keydown", { key: "Tab" });
    assert.equal(tabFromMute.prevented, true);
    assert.equal(doc.activeElement, activeLevelCard);

    // Переводим фокус на последнюю кнопку «Готово» в подвале и нажимаем Tab -> цикл на первую кнопку закрытия в шапке
    doneBtn.focus();
    assert.equal(doc.activeElement, doneBtn);
    const tabWrapForward = doneBtn.dispatchEvent("keydown", { key: "Tab" });
    assert.equal(tabWrapForward.prevented, true);
    assert.equal(
      doc.activeElement,
      closeHeaderBtn,
      "Tab на последнем элементе модалки обязан циклически переводить фокус на первый элемент",
    );

    // Shift+Tab на первом элементе модалки циклически возвращает фокус на последний элемент («Готово»)
    const tabWrapBackward = closeHeaderBtn.dispatchEvent("keydown", {
      key: "Tab",
      shiftKey: true,
    });
    assert.equal(tabWrapBackward.prevented, true);
    assert.equal(
      doc.activeElement,
      doneBtn,
      "Shift+Tab на первом элементе модалки обязан циклически переводить фокус на последний элемент",
    );
  });

  it("таймер истечения mute: при активном временном mute планируется таймер на max(0, endMs - nowMs), снимающий mute и обновляющий UI (§1.3.2 п.2)", () => {
    const origSetTimeout = globalThis.setTimeout;
    const origClearTimeout = globalThis.clearTimeout;
    /** @type {Array<{ id: number, fn: () => void, delayMs: number, cleared: boolean }>} */
    const scheduled = [];
    let nextTimerId = 1;
    let currentNow = NOW_MS;

    globalThis.setTimeout = /** @type {any} */ ((/** @type {() => void} */ fn, /** @type {number} */ delayMs) => {
      const rec = { id: nextTimerId++, fn, delayMs: Number(delayMs ?? 0), cleared: false };
      scheduled.push(rec);
      return rec.id;
    });
    globalThis.clearTimeout = /** @type {any} */ ((/** @type {number} */ id) => {
      const found = scheduled.find((item) => item.id === id);
      if (found) found.cleared = true;
    });

    try {
      const guildEndIso = new Date(NOW_MS + 900_000).toISOString();
      const chEndIso = new Date(NOW_MS + 300_000).toISOString();
      const { doc, controller } = setupController({
        now: () => currentNow,
        guildEntry: {
          guild_id: "1014537851418980372",
          message_notifications: 1,
          muted: true,
          mute_config: {
            selected_time_window: 900,
            end_time: guildEndIso,
          },
          version: 5,
          channel_overrides: [
            {
              channel_id: "ch-announce",
              collapsed: false,
              message_notifications: 1,
              muted: true,
              mute_config: {
                selected_time_window: 900,
                end_time: chEndIso,
              },
            },
          ],
        },
      });

      controller.openGuildModal();
      const muteSwitch = doc.getElementById("guild-notif-mute-switch");
      const windowRow = doc.getElementById("guild-notif-mute-window-row");
      assert.equal(muteSwitch?.getAttribute("aria-checked"), "true");
      assert.equal(windowRow?.getAttribute("hidden"), null);

      const activeTimers = scheduled.filter((t) => !t.cleared);
      const guildTimer = activeTimers.find((t) => t.delayMs === 900_000);
      const channelTimer = activeTimers.find((t) => t.delayMs === 300_000);
      assert.ok(guildTimer, "должен быть запланирован таймер истечения mute сервера на 900000 мс");
      assert.ok(channelTimer, "должен быть запланирован таймер истечения mute канала на 300000 мс");

      // Срабатывает таймер истечения канала (через 300 с)
      currentNow = NOW_MS + 300_000;
      channelTimer.fn();
      const chCheckboxAfter = doc.querySelector(
        '.notif-override-row[data-channel-id="ch-announce"] .notif-cell-checkbox',
      );
      assert.equal(
        chCheckboxAfter?.getAttribute("aria-checked"),
        "false",
        "по срабатыванию таймера истечения канала чекбокс mute в таблице сбрасывается в false",
      );

      // Срабатывает таймер истечения сервера (через 900 с)
      currentNow = NOW_MS + 900_000;
      guildTimer.fn();
      assert.equal(
        muteSwitch?.getAttribute("aria-checked"),
        "false",
        "по срабатыванию таймера истечения mute сервера тумблер #guild-notif-mute-switch переходит в false",
      );
      assert.notEqual(
        windowRow?.getAttribute("hidden"),
        null,
        "строка выбора длительности скрывается после истечения таймера mute",
      );
      assert.equal(controller.getState().guildEntry.muted, false);

      // При закрытии/уничтожении контроллера активные таймеры очищаются
      controller.destroy();
    } finally {
      globalThis.setTimeout = origSetTimeout;
      globalThis.clearTimeout = origClearTimeout;
    }
  });

  it("429 rate-limit с retry_after: контрол остаётся disabled и показывает посекундный отсчёт 'Повторите через {n} с.' (§4.1)", async () => {
    const origSetInterval = globalThis.setInterval;
    const origClearInterval = globalThis.clearInterval;
    /** @type {Array<{ id: number, fn: () => void, intervalMs: number, cleared: boolean }>} */
    const intervals = [];
    let nextIntervalId = 1;

    globalThis.setInterval = /** @type {any} */ ((/** @type {() => void} */ fn, /** @type {number} */ intervalMs) => {
      const rec = { id: nextIntervalId++, fn, intervalMs: Number(intervalMs ?? 0), cleared: false };
      intervals.push(rec);
      return rec.id;
    });
    globalThis.clearInterval = /** @type {any} */ ((/** @type {number} */ id) => {
      const found = intervals.find((item) => item.id === id);
      if (found) found.cleared = true;
    });

    try {
      let shouldRateLimit = true;
      const { doc, calls, controller } = setupController({
        invoke: async () => {
          if (shouldRateLimit) {
            const err = new Error("Rate limited");
            /** @type {any} */ (err).status = 429;
            /** @type {any} */ (err).retry_after_ms = 3000;
            throw err;
          }
          return { ok: true };
        },
      });

      controller.openGuildModal();
      const modal = doc.getElementById("guild-notif-modal");
      const errorAlert = doc.getElementById("guild-notif-error");
      const suppressEveryone = doc.getElementById("guild-notif-suppress-everyone");
      assert.ok(modal);
      assert.ok(errorAlert);
      assert.ok(suppressEveryone);

      suppressEveryone.dispatchEvent("keydown", { key: " " });
      await Promise.resolve();
      await Promise.resolve();

      assert.equal(calls.length, 1);
      assert.equal(modal.getAttribute("data-state"), "error");
      assert.equal(errorAlert.getAttribute("hidden"), null);
      assert.match(
        errorAlert.textContent,
        /Повторите через 3 с\./u,
        "при 429 с retry_after_ms=3000 должно выводиться сообщение 'Повторите через 3 с.'",
      );
      assert.equal(
        suppressEveryone.getAttribute("disabled") !== null ||
          suppressEveryone.getAttribute("aria-disabled") === "true",
        true,
        "контрол обязан оставаться disabled на время 429 retry_after",
      );

      // Попытка повторного нажатия во время активного 429-отсчёта блокируется (IPC не вызывается)
      suppressEveryone.dispatchEvent("keydown", { key: " " });
      await Promise.resolve();
      assert.equal(calls.length, 1, "во время 429 cooldown повторные запросы не отправляются");

      const activeInterval = intervals.find((i) => !i.cleared && i.intervalMs === 1000);
      assert.ok(activeInterval, "должен быть запущен посекундный интервал (1000 мс) обратного отсчёта 429");

      // Тик 1 -> 2 с.
      activeInterval.fn();
      assert.match(errorAlert.textContent, /Повторите через 2 с\./u);
      // Тик 2 -> 1 с.
      activeInterval.fn();
      assert.match(errorAlert.textContent, /Повторите через 1 с\./u);
      // Тик 3 -> 0 с. -> разблокировка и переход в ready
      activeInterval.fn();
      assert.equal(activeInterval.cleared, true, "после достижения 0 интервал очищается");
      assert.equal(suppressEveryone.getAttribute("disabled"), null, "disabled снимается после истечения 429");
      assert.equal(modal.getAttribute("data-state"), "ready");

      // Теперь ручной повтор разрешён и проходит успешно
      shouldRateLimit = false;
      suppressEveryone.dispatchEvent("keydown", { key: " " });
      await Promise.resolve();
      await Promise.resolve();
      assert.equal(calls.length, 2);
      assert.equal(suppressEveryone.getAttribute("aria-checked"), "true");
    } finally {
      globalThis.setInterval = origSetInterval;
      globalThis.clearInterval = origClearInterval;
    }
  });

  it("тумблер mute сервера, селект 6 окон длительности, DM (@me) маршрут и закрытие модалки по Escape с возвратом фокуса", async () => {
    const { doc, triggerBtn, calls, controller } = setupController();

    triggerBtn.focus();
    controller.openGuildModal({ triggerElement: triggerBtn });

    const modal = doc.getElementById("guild-notif-modal");
    const muteSwitch = doc.getElementById("guild-notif-mute-switch");
    const windowRow = doc.getElementById("guild-notif-mute-window-row");
    const windowSelect = /** @type {any} */ (doc.getElementById("guild-notif-window-select"));
    const expiryText = doc.getElementById("guild-notif-mute-expiry");
    assert.ok(modal);
    assert.ok(muteSwitch);
    assert.ok(windowRow);
    assert.ok(windowSelect);
    assert.ok(expiryText);

    assert.notEqual(windowRow.getAttribute("hidden"), null, "строка выбора окна скрыта при muted=false");
    assert.equal(windowSelect.children.length, 6, "в селекте 6 канонических окон длительности");

    // Включаем mute сервера тумблером
    muteSwitch.dispatchEvent("keydown", { key: " " });
    await Promise.resolve();

    assert.equal(muteSwitch.getAttribute("aria-checked"), "true");
    assert.equal(windowRow.getAttribute("hidden"), null, "строка выбора длительности видна при muted=true");
    assert.deepEqual(calls.at(-1)?.args.patch, { muted: true });

    // Выбираем окно 28800 (8 часов)
    windowSelect.value = "28800";
    windowSelect.emit("change");
    await Promise.resolve();

    assert.equal(calls.at(-1)?.cmd, "update_guild_notification_settings");
    assert.deepEqual(calls.at(-1)?.args.patch, {
      muted: true,
      mute_config: {
        selected_time_window: 28800,
        end_time: new Date(NOW_MS + 28_800_000).toISOString(),
      },
    });

    // Закрытие модалки по Escape возвращает фокус на triggerBtn
    const escModal = modal.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(escModal.prevented, true);
    assert.notEqual(doc.getElementById("guild-notifications-dialog")?.getAttribute("hidden"), null);
    assert.equal(doc.activeElement?.id, triggerBtn.id, "Escape закрывает модалку и возвращает фокус на opener");

    // Проверка маршрута для DM (@me): тело не оборачивается в { guilds: ... }
    const dmSetup = setupController({ guildId: "@me" });
    dmSetup.controller.openGuildModal();
    const dmRolesSwitch = dmSetup.doc.getElementById("guild-notif-suppress-roles");
    assert.ok(dmRolesSwitch);
    dmRolesSwitch.dispatchEvent("keydown", { key: " " });
    await Promise.resolve();
    assert.equal(dmSetup.calls[0]?.args.endpoint, "/users/@me/guilds/@me/settings");
    assert.deepEqual(dmSetup.calls[0]?.args.body, { suppress_roles: true });
  });

  it("добавление канала в таблицу через #notif-override-channel-select и обработка ошибок IPC (#guild-notif-error)", async () => {
    let shouldFail = false;
    const { doc, calls, controller } = setupController({
      invoke: async () => {
        if (shouldFail) {
          const err = new Error("Недостаточно прав или сбой сети");
          /** @type {any} */ (err).status = 500;
          throw err;
        }
        return { ok: true };
      },
    });

    controller.openGuildModal();
    const select = /** @type {any} */ (doc.getElementById("notif-override-channel-select"));
    const addBtn = doc.querySelector('[data-action="notif-override-add"]');
    const tbody = doc.getElementById("notif-override-tbody");
    const errorAlert = doc.getElementById("guild-notif-error");
    assert.ok(select);
    assert.ok(addBtn);
    assert.ok(tbody);
    assert.ok(errorAlert);

    // Добавляем канал ch-flood в таблицу переопределений
    select.value = "ch-flood";
    addBtn.click();
    await Promise.resolve();

    assert.equal(calls.at(-1)?.cmd, "update_channel_notification_override");
    assert.equal(calls.at(-1)?.args.channelId, "ch-flood");
    assert.equal(tbody.querySelectorAll(".notif-override-row").length, 2);

    // Отказ IPC переводит модалку в data-state="error" и показывает #guild-notif-error без мутации состояния
    shouldFail = true;
    const mobileSwitch = doc.getElementById("guild-notif-mobile-push");
    assert.ok(mobileSwitch);
    assert.equal(mobileSwitch.getAttribute("aria-checked"), "true");

    mobileSwitch.dispatchEvent("keydown", { key: " " });
    await Promise.resolve();
    await Promise.resolve();

    assert.equal(doc.getElementById("guild-notif-modal")?.getAttribute("data-state"), "error");
    assert.equal(errorAlert.getAttribute("hidden"), null, "баннер ошибки должен стать видимым");
    assert.match(errorAlert.textContent, /Недостаточно прав или сбой сети/u);
    assert.equal(
      mobileSwitch.getAttribute("aria-checked"),
      "true",
      "при ошибке IPC состояние тумблера не должно измениться",
    );
  });
});
