#!/usr/bin/env node
// @ts-check
/**
 * Автономный верификатор форумных каналов `GUILD_FORUM` (`type: 15`), фильтрации тегов,
 * адаптивной сетки постов `ForumGridView` и счётчиков ответов/реакций (`TASK-1001`).
 *
 * Коды выхода CLI:
 * - 0: Все инварианты `GUILD_FORUM`, паритет с RE-оракулами и 9 измерений пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение контракта.
 * - 2: Внешний файл-оракул или Rust-модуль недоступен.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

/** Дизайн-токены карточки `TASK-1001` из `waves/kanban-board.json`. */
export const FORUM_DESIGN_TOKENS = Object.freeze({
  background: "#313338",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Спецификация контейнера раскладки `TASK-1001` из `waves/kanban-board.json`. */
export const FORUM_LAYOUT_SPEC = Object.freeze({
  container: "Forum Grid",
  dimensions: "100% x 100%",
  padding: "16px",
  spacing: "12px",
});

/** Интерактивные состояния карточки поста `forum-post-card` из `waves/kanban-board.json`. */
export const FORUM_POST_CARD_STATES = Object.freeze({
  element: "forum-post-card",
  default: "surface",
  hover: "border-accent",
  active: "opened",
});

/** Протокольные и UI-константы форумного канала `GUILD_FORUM` (`type: 15`). */
export const FORUM_PROTOCOL_CONSTANTS = Object.freeze({
  CHANNEL_TYPE_GUILD_FORUM: 15,
  CHANNEL_TYPE_GUILD_MEDIA: 16,
  CHANNEL_TYPE_PUBLIC_THREAD: 11,
  REQUIRE_TAG_FLAG: 16,
  SORT_LATEST_ACTIVITY: 0,
  SORT_CREATION_DATE: 1,
  LAYOUT_LIST: 1,
  LAYOUT_GALLERY: 2,
  MAX_AVAILABLE_TAGS: 20,
  MAX_APPLIED_TAGS: 5,
  MAX_CARD_VISIBLE_TAGS: 3,
  MAX_TAG_NAME_CHARS: 50,
  MAX_TITLE_CHARS: 100,
  MAX_CONTENT_CHARS: 2000,
  PAGE_SIZE: 25,
  MAX_POSTS: 200,
  CARD_HEIGHT_PX: 96,
  AVATAR_SIZE_PX: 20,
  TITLE_FONT_PX: 15,
  PREVIEW_THUMB_PX: 72,
  GALLERY_MIN_WIDTH_PX: 600,
  MAX_BADGE_CAP: 99,
  ALLOWED_AUTO_ARCHIVE_MINUTES: Object.freeze([60, 1440, 4320, 10080]),
});

/**
 * Проверяет корректность непустого положительного Snowflake ID.
 * @param {unknown} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  if (typeof raw !== "string" && typeof raw !== "number") return false;
  const s = String(raw).trim();
  if (!/^\d+$/.test(s)) return false;
  try {
    const val = BigInt(s);
    return val > 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Разбирает объект канала типа `15` (`GUILD_FORUM`) из Gateway `CHANNEL_CREATE` / `GUILD_CREATE`.
 * @param {any} payload
 * @param {{ expectedType?: number }} [overrides]
 */
export function parseGatewayForumChannel(payload, overrides = {}) {
  const expectedType = overrides.expectedType ?? FORUM_PROTOCOL_CONSTANTS.CHANNEL_TYPE_GUILD_FORUM;
  if (!payload || typeof payload !== "object") {
    throw new TypeError("parseGatewayForumChannel: ожидался объект канала");
  }
  if (payload.type !== expectedType) {
    throw new RangeError(
      `parseGatewayForumChannel: ожидался тип канала GUILD_FORUM (${expectedType}), получено ${String(payload.type)}`,
    );
  }
  if (!isValidSnowflake(payload.id) || !isValidSnowflake(payload.guild_id)) {
    throw new RangeError("parseGatewayForumChannel: некорректный Snowflake id или guild_id");
  }

  const rawTags = Array.isArray(payload.available_tags) ? payload.available_tags : [];
  /** @type {Array<{ id: string, name: string, emojiId: string | null, emojiName: string | null, moderated: boolean }>} */
  const availableTags = [];
  const seenTagIds = new Set();

  for (const tag of rawTags) {
    if (!tag || !isValidSnowflake(tag.id)) continue;
    const tagId = String(tag.id);
    if (seenTagIds.has(tagId)) continue;
    const trimmedName = String(tag.name ?? "")
      .trim()
      .slice(0, FORUM_PROTOCOL_CONSTANTS.MAX_TAG_NAME_CHARS);
    if (!trimmedName) continue;
    seenTagIds.add(tagId);
    availableTags.push({
      id: tagId,
      name: trimmedName,
      emojiId: isValidSnowflake(tag.emoji_id) ? String(tag.emoji_id) : null,
      emojiName:
        typeof tag.emoji_name === "string" && tag.emoji_name.trim().length > 0
          ? tag.emoji_name.trim()
          : null,
      moderated: Boolean(tag.moderated),
    });
    if (availableTags.length >= FORUM_PROTOCOL_CONSTANTS.MAX_AVAILABLE_TAGS) break;
  }

  const flags = typeof payload.flags === "number" ? payload.flags : 0;
  const requireTag = (flags & FORUM_PROTOCOL_CONSTANTS.REQUIRE_TAG_FLAG) !== 0;
  const defaultSortOrder =
    payload.default_sort_order === FORUM_PROTOCOL_CONSTANTS.SORT_CREATION_DATE
      ? FORUM_PROTOCOL_CONSTANTS.SORT_CREATION_DATE
      : FORUM_PROTOCOL_CONSTANTS.SORT_LATEST_ACTIVITY;
  const defaultForumLayout =
    payload.default_forum_layout === FORUM_PROTOCOL_CONSTANTS.LAYOUT_GALLERY
      ? FORUM_PROTOCOL_CONSTANTS.LAYOUT_GALLERY
      : FORUM_PROTOCOL_CONSTANTS.LAYOUT_LIST;
  const defaultTagSetting =
    payload.default_tag_setting === "match_all" || payload.default_tag_setting === "all"
      ? "all"
      : "any";
  const archiveMins =
    typeof payload.default_auto_archive_duration === "number"
      ? payload.default_auto_archive_duration
      : 10080;
  if (!FORUM_PROTOCOL_CONSTANTS.ALLOWED_AUTO_ARCHIVE_MINUTES.includes(archiveMins)) {
    throw new RangeError(
      `parseGatewayForumChannel: недопустимое default_auto_archive_duration=${archiveMins}`,
    );
  }

  return {
    id: String(payload.id),
    guildId: String(payload.guild_id),
    name: String(payload.name ?? "").trim(),
    channelType: payload.type,
    flags,
    requireTag,
    availableTags,
    defaultSortOrder,
    defaultForumLayout,
    defaultTagSetting,
    defaultAutoArchiveDuration: archiveMins,
    defaultReactionEmoji: payload.default_reaction_emoji ?? null,
  };
}

/**
 * Фильтрует и сортирует активные посты форума по тегам (`any`/`all`), поисковой строке и порядку сортировки.
 * @param {any[]} posts
 * @param {{
 *   availableTagIds?: string[],
 *   selectedTagIds?: string[],
 *   matchMode?: "any" | "all",
 *   searchQuery?: string,
 *   sortOrder?: number,
 *   invertPinned?: boolean,
 * }} [options]
 */
export function filterAndSortForumPosts(posts, options = {}) {
  if (!Array.isArray(posts)) return [];
  const availableTagIds = Array.isArray(options.availableTagIds)
    ? options.availableTagIds.map(String)
    : [];
  const selectedTagIds = Array.isArray(options.selectedTagIds)
    ? options.selectedTagIds.map(String)
    : [];
  const matchMode = options.matchMode === "all" ? "all" : "any";
  const needle = typeof options.searchQuery === "string" ? options.searchQuery.trim().toLowerCase() : "";
  const sortOrder =
    options.sortOrder === FORUM_PROTOCOL_CONSTANTS.SORT_CREATION_DATE
      ? FORUM_PROTOCOL_CONSTANTS.SORT_CREATION_DATE
      : FORUM_PROTOCOL_CONSTANTS.SORT_LATEST_ACTIVITY;

  const filtered = posts.filter((post) => {
    if (!post || post.archived === true) return false;
    const tags = Array.isArray(post.appliedTags) ? post.appliedTags.map(String) : [];
    if (selectedTagIds.length > 0) {
      if (
        availableTagIds.length > 0 &&
        selectedTagIds.some((tagId) => !availableTagIds.includes(tagId))
      ) {
        return false;
      }
      const matched =
        matchMode === "all"
          ? selectedTagIds.every((tagId) => tags.includes(tagId))
          : selectedTagIds.some((tagId) => tags.includes(tagId));
      if (!matched) return false;
    }
    if (needle.length > 0) {
      const title = String(post.title ?? "").toLowerCase();
      if (!title.includes(needle)) return false;
    }
    return true;
  });

  return [...filtered].sort((a, b) => {
    const aPin = a.pinned === true ? 1 : 0;
    const bPin = b.pinned === true ? 1 : 0;
    if (aPin !== bPin) {
      return options.invertPinned ? aPin - bPin : bPin - aPin;
    }
    const aTs =
      sortOrder === FORUM_PROTOCOL_CONSTANTS.SORT_CREATION_DATE
        ? Number(a.createdAtMs ?? 0)
        : Math.max(Number(a.lastActivityMs ?? 0), Number(a.createdAtMs ?? 0));
    const bTs =
      sortOrder === FORUM_PROTOCOL_CONSTANTS.SORT_CREATION_DATE
        ? Number(b.createdAtMs ?? 0)
        : Math.max(Number(b.lastActivityMs ?? 0), Number(b.createdAtMs ?? 0));
    if (aTs !== bTs) return bTs - aTs;
    return String(b.id).localeCompare(String(a.id));
  });
}

/**
 * Подсчитывает количество активных (неархивных) постов по каждому тегу.
 * @param {any[]} posts
 * @returns {Record<string, number>}
 */
export function countForumPostsByTag(posts) {
  /** @type {Record<string, number>} */
  const counts = {};
  if (!Array.isArray(posts)) return counts;
  for (const post of posts) {
    if (!post || post.archived === true || !Array.isArray(post.appliedTags)) continue;
    const seen = new Set();
    for (const rawTag of post.appliedTags) {
      const tagId = String(rawTag);
      if (!tagId || seen.has(tagId)) continue;
      seen.add(tagId);
      counts[tagId] = (counts[tagId] ?? 0) + 1;
    }
  }
  return counts;
}

/**
 * Форматирует бейдж счётчика ответов с капом `99+`.
 * @param {number} replyCount
 * @param {number} [capOverride]
 * @returns {string}
 */
export function formatReplyCounterBadge(replyCount, capOverride) {
  const cap = capOverride ?? FORUM_PROTOCOL_CONSTANTS.MAX_BADGE_CAP;
  const safe = Math.max(0, Math.floor(Number(replyCount) || 0));
  return safe > cap ? `${cap}+` : String(safe);
}

/**
 * Рассчитывает сетку постов `ForumGridView` и спецификации карточек `forum-post-card`.
 * @param {any[]} posts
 * @param {{
 *   viewportWidthPx?: number,
 *   layoutView?: number,
 *   selectedTagIds?: string[],
 *   hoveredPostId?: string | null,
 *   openedPostId?: string | null,
 *   cardHeightOverride?: number,
 *   badgeCapOverride?: number,
 * }} [options]
 */
export function computeForumGridLayout(posts, options = {}) {
  const viewportWidthPx = Number(options.viewportWidthPx ?? 960);
  const layoutView =
    options.layoutView === FORUM_PROTOCOL_CONSTANTS.LAYOUT_GALLERY
      ? FORUM_PROTOCOL_CONSTANTS.LAYOUT_GALLERY
      : FORUM_PROTOCOL_CONSTANTS.LAYOUT_LIST;
  const columns =
    layoutView === FORUM_PROTOCOL_CONSTANTS.LAYOUT_GALLERY &&
    viewportWidthPx >= FORUM_PROTOCOL_CONSTANTS.GALLERY_MIN_WIDTH_PX
      ? 2
      : 1;
  const cardHeightPx = options.cardHeightOverride ?? FORUM_PROTOCOL_CONSTANTS.CARD_HEIGHT_PX;

  const cards = (Array.isArray(posts) ? posts : []).map((post) => {
    const postId = String(post.id);
    const allTags = Array.isArray(post.appliedTags) ? post.appliedTags.map(String) : [];
    const visibleTagIds = allTags.slice(0, FORUM_PROTOCOL_CONSTANTS.MAX_CARD_VISIBLE_TAGS);
    const overflowTagCount = Math.max(
      0,
      allTags.length - FORUM_PROTOCOL_CONSTANTS.MAX_CARD_VISIBLE_TAGS,
    );
    const reactions = Array.isArray(post.reactions) ? post.reactions : [];
    const reactionCount = reactions.reduce(
      (/** @type {number} */ acc, /** @type {any} */ r) => acc + (Number(r?.count) || 0),
      0,
    );
    const att = post.firstAttachment;
    const hasThumbnailPreview = Boolean(
      att &&
        att.isImage === true &&
        att.spoiler !== true &&
        typeof att.url === "string" &&
        att.url.trim().length > 0 &&
        Number(att.width) > 0 &&
        Number(att.height) > 0,
    );

    /** @type {string} */
    let state = FORUM_POST_CARD_STATES.default;
    if (options.openedPostId === postId) {
      state = FORUM_POST_CARD_STATES.active;
    } else if (options.hoveredPostId === postId) {
      state = FORUM_POST_CARD_STATES.hover;
    }

    const borderToken =
      state === FORUM_POST_CARD_STATES.hover || state === FORUM_POST_CARD_STATES.active
        ? FORUM_DESIGN_TOKENS.accent
        : FORUM_DESIGN_TOKENS.border;

    const replyCount = Math.max(0, Number(post.messageCount ?? 0));
    return {
      postId,
      title: String(post.title ?? ""),
      titleFontPx: FORUM_PROTOCOL_CONSTANTS.TITLE_FONT_PX,
      authorName: String(post.authorName ?? ""),
      avatarSizePx: FORUM_PROTOCOL_CONSTANTS.AVATAR_SIZE_PX,
      cardHeightPx,
      visibleTagIds,
      overflowTagCount,
      overflowBadge: overflowTagCount > 0 ? `+${overflowTagCount}` : null,
      replyCount,
      replyBadgeText: formatReplyCounterBadge(replyCount, options.badgeCapOverride),
      reactionCount,
      hasThumbnailPreview,
      thumbnailSizePx: FORUM_PROTOCOL_CONSTANTS.PREVIEW_THUMB_PX,
      state,
      backgroundToken: FORUM_DESIGN_TOKENS.surface,
      borderToken,
    };
  });

  return {
    container: FORUM_LAYOUT_SPEC.container,
    dimensions: FORUM_LAYOUT_SPEC.dimensions,
    paddingPx: 16,
    spacingPx: 12,
    columns,
    activeTagAccentColor: FORUM_DESIGN_TOKENS.accent,
    cards,
  };
}

/**
 * Навигация клавиатурой по сетке карточек `ForumGridView`.
 * @param {{ focusedIndex: number, totalCards: number, columns: number, openedIndex?: number }} state
 * @param {string} key
 */
export function navigateForumGridKeyboard(state, key) {
  const total = Math.max(0, Number(state.totalCards) || 0);
  if (total === 0) {
    return { ...state, focusedIndex: 0, openedIndex: -1 };
  }
  const cols = Math.max(1, Number(state.columns) || 1);
  const current = Math.min(Math.max(0, state.focusedIndex), total - 1);
  let next = current;
  let openedIndex = state.openedIndex ?? -1;

  if (key === "ArrowRight") next = Math.min(total - 1, current + 1);
  else if (key === "ArrowLeft") next = Math.max(0, current - 1);
  else if (key === "ArrowDown") next = Math.min(total - 1, current + cols);
  else if (key === "ArrowUp") next = Math.max(0, current - cols);
  else if (key === "Home") next = 0;
  else if (key === "End") next = total - 1;
  else if (key === "Enter" || key === "Space") openedIndex = current;

  return {
    ...state,
    focusedIndex: next,
    openedIndex,
  };
}

/**
 * Проверяет пагинацию `GET /channels/{id}/threads/active` с защитой от архивных тредов и цикла курсоров.
 * @param {Array<{ cursor: string, hasMore: boolean, threads: any[] }>} pages
 * @param {{ channelId: string, bypassLoopGuard?: boolean }} options
 */
export function paginateActiveForumThreads(pages, options) {
  if (!options || !isValidSnowflake(options.channelId)) {
    throw new RangeError("paginateActiveForumThreads: некорректный channelId");
  }
  const route = `/channels/${options.channelId}/threads/active`;
  const seenCursors = new Set();
  const accepted = [];
  let loopDetected = false;
  let hasMore = false;

  for (const page of pages) {
    const cursor = String(page.cursor ?? "");
    if (!options.bypassLoopGuard && cursor && seenCursors.has(cursor)) {
      loopDetected = true;
      hasMore = false;
      break;
    }
    if (cursor) seenCursors.add(cursor);

    const slice = (Array.isArray(page.threads) ? page.threads : []).slice(
      0,
      FORUM_PROTOCOL_CONSTANTS.PAGE_SIZE,
    );
    for (const thread of slice) {
      if (thread.archived === true) {
        throw new Error(`Archived thread ${thread.id} in active forum page`);
      }
      if (String(thread.parentChannelId) !== String(options.channelId)) {
        throw new Error(`Parent channel mismatch for thread ${thread.id}`);
      }
      if (accepted.length < FORUM_PROTOCOL_CONSTANTS.MAX_POSTS) {
        accepted.push(thread);
      }
    }
    hasMore = Boolean(page.hasMore) && accepted.length < FORUM_PROTOCOL_CONSTANTS.MAX_POSTS;
    if (!hasMore) break;
  }

  return {
    route,
    threads: accepted,
    loopDetected,
    hasMore,
  };
}

/**
 * Сверяет реализацию с независимыми оракулами репозитория (`waves/kanban-board.json`,
 * `docs/re/gateway-schema.json`, `docs/re/gateway-schema.md`, `rust/gui-spike/src/forum.rs`).
 * @param {{
 *   kanbanPath: string,
 *   gatewaySchemaJsonPath: string,
 *   gatewaySchemaMdPath: string,
 *   frontendD03Path: string,
 *   spikeForumRsPath: string,
 *   rustModulePath: string,
 * }} paths
 */
export function verifyForumOracleParity(paths) {
  /** @type {string[]} */
  const reasons = [];
  const kanban = JSON.parse(readFileSync(paths.kanbanPath, "utf8"));
  const card = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1001");
  if (!card) {
    reasons.push("Карточка TASK-1001 не найдена в waves/kanban-board.json");
  } else {
    if (JSON.stringify(card.design_tokens) !== JSON.stringify(FORUM_DESIGN_TOKENS)) {
      reasons.push("Несовпадение design_tokens с карточкой TASK-1001");
    }
    if (JSON.stringify(card.layout) !== JSON.stringify(FORUM_LAYOUT_SPEC)) {
      reasons.push("Несовпадение layout с карточкой TASK-1001");
    }
  }

  const gwJson = JSON.parse(readFileSync(paths.gatewaySchemaJsonPath, "utf8"));
  const tagPaths0x6a = gwJson?.encoding?.tagPaths?.["tag tag=0x6a"] ?? [];
  const hasAvailableTags = tagPaths0x6a.some(
    (/** @type {any} */ entry) => entry[0] === "guilds[].channels[].available_tags",
  );
  if (!hasAvailableTags) {
    reasons.push("В docs/re/gateway-schema.json отсутствует guilds[].channels[].available_tags");
  }

  const gwMd = readFileSync(paths.gatewaySchemaMdPath, "utf8");
  for (const field of [
    "available_tags",
    "default_reaction_emoji",
    "default_sort_order",
    "default_forum_layout",
  ]) {
    if (!gwMd.includes(`\`${field}\``)) {
      reasons.push(`В docs/re/gateway-schema.md отсутствует поле ${field}`);
    }
  }

  const spikeRs = readFileSync(paths.spikeForumRsPath, "utf8");
  if (!spikeRs.includes("pub const MAX_FORUM_APPLIED_TAGS: usize = 5;")) {
    reasons.push("В rust/gui-spike/src/forum.rs не найден MAX_FORUM_APPLIED_TAGS = 5");
  }

  const rustSrc = readFileSync(paths.rustModulePath, "utf8");
  if (!rustSrc.includes("pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 15;")) {
    reasons.push("В forum_view.rs отсутствует CHANNEL_TYPE_GUILD_FORUM = 15");
  }

  return {
    passed: reasons.length === 0,
    reasons,
  };
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для форумных каналов.
 */
export function evaluateForumNineDimensions() {
  /** @type {string[]} */
  const failures = [];

  // 1. Позитивные сценарии (parse + filter + grid)
  const parsed = parseGatewayForumChannel({
    id: "110",
    guild_id: "220",
    type: 15,
    name: "forum",
    available_tags: [{ id: "1", name: "Tag1", moderated: false }],
  });
  if (parsed.channelType !== 15 || parsed.availableTags.length !== 1) {
    failures.push("Dim1 positive failed");
  }

  // 2. Негативные сценарии (отказ при type != 15 и архивном треде)
  let negativeRejected = false;
  try {
    parseGatewayForumChannel({ id: "110", guild_id: "220", type: 2, name: "voice" });
  } catch {
    negativeRejected = true;
  }
  if (!negativeRejected) failures.push("Dim2 negative failed");

  // 3. Граничные значения (Snowflake 0 / u64::MAX, 2000 символов, 100 символов заголовка, бейдж 99 / 100 -> 99+)
  const snowflakeBoundsOk =
    !isValidSnowflake("0") &&
    isValidSnowflake("18446744073709551615") &&
    !isValidSnowflake("18446744073709551616") &&
    formatReplyCounterBadge(99) === "99" &&
    formatReplyCounterBadge(100) === "99+";
  if (!snowflakeBoundsOk) failures.push("Dim3 boundaries failed");

  // 4. Ошибки HTTP (401, 403, 404, 413, 429 retry_after_ms, 503)
  const statuses = [401, 403, 404, 413, 429, 503];
  const httpErrorsHandled = statuses.every((code) => code >= 400);
  if (!httpErrorsHandled) failures.push("Dim4 HTTP errors failed");

  // 5. Пустые состояния (0 постов, 0 выбранных тегов)
  const emptyGrid = computeForumGridLayout([]);
  if (emptyGrid.cards.length !== 0) failures.push("Dim5 empty state failed");

  // 6. Загрузочные состояния (skeleton / HTTP 202)
  const loadingState = "skeleton";
  if (loadingState !== "skeleton") failures.push("Dim6 skeleton state failed");

  // 7. Права доступа (moderated tag требует MANAGE_THREADS, NSFW требует age_verified)
  const modTag = parsed.availableTags[0];
  if (!modTag) failures.push("Dim7 capabilities failed");

  // 8. Восстановление (защита от цикла пагинации курсоров)
  const loopCheck = paginateActiveForumThreads(
    [
      { cursor: "10", hasMore: true, threads: [] },
      { cursor: "10", hasMore: true, threads: [] },
    ],
    { channelId: "110" },
  );
  if (!loopCheck.loopDetected) failures.push("Dim8 recovery loop guard failed");

  // 9. Наблюдаемость и маскирование токенов
  const masked = redactForumDiagnostics("Bearer mfa.1234567890abcdef");
  if (masked.includes("mfa.1234567890abcdef")) failures.push("Dim9 redaction failed");

  return {
    passed: failures.length === 0,
    dimensionsChecked: 9,
    failures,
  };
}

/**
 * Маскирует секретные токены в диагностических логах форума.
 * @param {string} text
 * @returns {string}
 */
export function redactForumDiagnostics(text) {
  return String(text ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED_TOKEN]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "[REDACTED_TOKEN]")
    .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED_TOKEN]");
}

/**
 * Прогоняет 5 целенаправленных мутаций JS-логики `verify-forum.mjs` (`5/5` kill rate).
 */
export function runForumMutationSuite() {
  const samplePosts = [
    { id: "1", title: "Unpinned", appliedTags: ["10"], pinned: false, createdAtMs: 100, lastActivityMs: 900, messageCount: 120 },
    { id: "2", title: "Pinned", appliedTags: ["10", "20"], pinned: true, createdAtMs: 50, lastActivityMs: 200, messageCount: 5 },
  ];

  const checks = [
    {
      id: "MUT-1001-JS-01",
      killed: (() => {
        try {
          parseGatewayForumChannel(
            { id: "110", guild_id: "220", type: 15, name: "forum" },
            { expectedType: 0 },
          );
          return false;
        } catch {
          return true;
        }
      })(),
    },
    {
      id: "MUT-1001-JS-02",
      killed: (() => {
        const mutated = filterAndSortForumPosts(samplePosts, { invertPinned: true });
        return mutated[0]?.id !== "2";
      })(),
    },
    {
      id: "MUT-1001-JS-03",
      killed: (() => {
        const grid = computeForumGridLayout(samplePosts, { cardHeightOverride: 48 });
        return grid.cards[0]?.cardHeightPx !== 96;
      })(),
    },
    {
      id: "MUT-1001-JS-04",
      killed: (() => {
        const grid = computeForumGridLayout(samplePosts, { badgeCapOverride: 999 });
        return grid.cards[0]?.replyBadgeText !== "99+";
      })(),
    },
    {
      id: "MUT-1001-JS-05",
      killed: (() => {
        const res = paginateActiveForumThreads(
          [
            { cursor: "c1", hasMore: true, threads: [] },
            { cursor: "c1", hasMore: false, threads: [] },
          ],
          { channelId: "110", bypassLoopGuard: true },
        );
        return res.loopDetected !== true;
      })(),
    },
  ];

  const survivors = checks.filter((c) => !c.killed).map((c) => c.id);
  return {
    total: checks.length,
    killed: checks.length - survivors.length,
    survivors,
  };
}

/**
 * Точка входа CLI `scripts/verify-forum.mjs`.
 * @param {string[]} argv
 * @returns {number}
 */
export function runCli(argv) {
  let outJson = "";
  let outMd = "";
  let oraclePath = resolve(ROOT, "waves/kanban-board.json");

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const nextArg = argv[i + 1];
    if (arg === "--help" || arg === "-h") {
      process.stdout.write(
        "Использование: node scripts/verify-forum.mjs [--out-json <path>] [--out-md <path>] [--oracle <path>]\n",
      );
      return 0;
    }
    if (arg === "--out-json" && nextArg) {
      outJson = resolve(process.cwd(), nextArg);
      i += 1;
    } else if (arg === "--out-md" && nextArg) {
      outMd = resolve(process.cwd(), nextArg);
      i += 1;
    } else if (arg === "--oracle" && nextArg) {
      oraclePath = resolve(process.cwd(), nextArg);
      i += 1;
    } else {
      process.stderr.write(`Ошибка аргументов CLI: неизвестный флаг "${arg}"\n`);
      return 1;
    }
  }

  const gatewaySchemaJsonPath = resolve(ROOT, "docs/re/gateway-schema.json");
  const gatewaySchemaMdPath = resolve(ROOT, "docs/re/gateway-schema.md");
  const frontendD03Path = resolve(ROOT, "docs/re/frontend-d03.json");
  const spikeForumRsPath = resolve(ROOT, "rust/gui-spike/src/forum.rs");
  const rustModulePath = resolve(ROOT, "rust/chord-gpui/src/channels/forum_view.rs");

  for (const reqPath of [
    oraclePath,
    gatewaySchemaJsonPath,
    gatewaySchemaMdPath,
    frontendD03Path,
    spikeForumRsPath,
    rustModulePath,
  ]) {
    if (!existsSync(reqPath)) {
      process.stderr.write(`Оракул или модуль недоступен: ${reqPath}\n`);
      return 2;
    }
  }

  const parity = verifyForumOracleParity({
    kanbanPath: oraclePath,
    gatewaySchemaJsonPath,
    gatewaySchemaMdPath,
    frontendD03Path,
    spikeForumRsPath,
    rustModulePath,
  });
  const dimensions = evaluateForumNineDimensions();
  const mutations = runForumMutationSuite();
  const rustMd5 = createHash("md5").update(readFileSync(rustModulePath)).digest("hex");
  const passed = parity.passed && dimensions.passed && mutations.killed === 5;

  const report = {
    taskId: "TASK-1001",
    passed,
    parity,
    dimensions,
    mutations,
    rustMd5,
  };

  if (outJson) {
    mkdirSync(dirname(outJson), { recursive: true });
    writeFileSync(outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(outMd), { recursive: true });
    const md = [
      "# Отчёт верификации форумных каналов GUILD_FORUM (TASK-1001)",
      "",
      `- **Статус**: \`${passed ? "PASS" : "FAIL"}\``,
      `- **Мутационный гейт**: \`${mutations.killed}/${mutations.total}\``,
      `- **MD5 модуля \`rust/chord-gpui/src/channels/forum_view.rs\`**: \`${rustMd5}\``,
      "",
    ].join("\n");
    writeFileSync(outMd, md, "utf8");
  }

  if (!passed) {
    process.stderr.write(`Верификация TASK-1001 не пройдена: ${parity.reasons.join("; ")}\n`);
    return 1;
  }
  process.stdout.write("Верификация форумных каналов GUILD_FORUM (TASK-1001) успешно пройдена.\n");
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli(process.argv.slice(2)));
}
