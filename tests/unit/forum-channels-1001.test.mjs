// @ts-check
/**
 * Контрактные и модульные тесты форумных каналов `GUILD_FORUM` (type 15) и карточек постов (TASK-1001):
 * - SUB-1001-0: Статический и компиляционный контракт нативного модуля `rust/discord-gpui/src/channels/forum_view.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   и прогон встроенных Rust-тестов).
 * - SUB-1001-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-1001`,
 *   `docs/re/gateway-schema.json` и `docs/re/gateway-schema.md` для `available_tags`, `default_reaction_emoji`,
 *   `default_sort_order`, `default_forum_layout`, `default_tag_setting`, `default_auto_archive_duration`,
 *   `docs/re/frontend-d03.json` и `rust/gui-spike/src/forum.rs`).
 * - SUB-1001-2: Парсер канала `type: 15` (`GUILD_FORUM`) из Gateway `CHANNEL_CREATE`/`GUILD_CREATE`,
 *   поддержка тегов `available_tags` (`id`, `name`, `emoji_id`, `emoji_name`, `moderated`), фильтрация по тегам
 *   (`match_some` / `match_all`, подсветка активных тегов `#5865f2`), сортировка (`LatestActivity=0`, `CreationDate=1`)
 *   с приоритетом `pinned`.
 * - SUB-1001-3: Сетка карточек постов `ForumGridView` (`container: "Forum Grid"`, `100% x 100%`, `padding: 16px`,
 *   `spacing: 12px`, карточка `96px`, 2-колоночная раскладка Gallery и 1-колоночная List, заголовок 15px,
 *   аватар автора 20px, до 3 видимых тегов + бейдж `+N`, превью первого вложения без спойлера `72x72`,
 *   счётчик ответов с капом `99+`, счётчик реакций, состояния `surface -> border-accent -> opened` и A11y-клавиатура).
 * - SUB-1001-4: Пагинация `GET /channels/{id}/threads/active`, покрытие всех 9 измерений `TEST-POLICY.md`
 *   и проверка CLI `scripts/verify-forum.mjs` (коды выхода `0 / 1 / 2`).
 * - SUB-1001-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/channels/forum_view.rs`
 *   и `scripts/verify-forum.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/channels/forum_view.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const VERIFY_SCRIPT_REL_PATH = "scripts/verify-forum.mjs";
const VERIFY_SCRIPT_PATH = join(ROOT, VERIFY_SCRIPT_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GATEWAY_SCHEMA_JSON_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const GATEWAY_SCHEMA_MD_PATH = join(ROOT, "docs", "re", "gateway-schema.md");
const FRONTEND_D03_JSON_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const SPIKE_FORUM_RS_PATH = join(ROOT, "rust", "gui-spike", "src", "forum.rs");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `forum_view.rs` через `rustc --test --edition 2024 -D warnings`.
 * @param {string} sourcePath
 * @param {string} outputBin
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function compileRustModuleTests(sourcePath, outputBin) {
  const res = spawnSync(
    "rustc",
    [
      "--test",
      "--edition",
      "2024",
      "-D",
      "warnings",
      sourcePath,
      "-o",
      outputBin,
    ],
    {
      cwd: ROOT,
      encoding: "utf8",
      timeout: 30_000,
    },
  );
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

/**
 * Запускает скомпилированный тестовый бинарник с опциональным фильтром теста.
 * @param {string} binPath
 * @param {string} [filter]
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function runCompiledRustTests(binPath, filter) {
  const args = filter ? [filter, "--nocapture"] : ["--nocapture"];
  const res = spawnSync(binPath, args, {
    cwd: ROOT,
    encoding: "utf8",
    timeout: 15_000,
  });
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

describe("TASK-1001: Форумные каналы GUILD_FORUM (type 15), фильтрация тегов, сетка постов и счётчики ответов", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "forum-channels-1001-"));
    testBinPath = join(workDir, "forum_view_1001_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1001-0: проверяет статический контракт rust/discord-gpui/src/channels/forum_view.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );
    assert.ok(
      existsSync(VERIFY_SCRIPT_PATH),
      `Отсутствует скрипт верификации ${VERIFY_SCRIPT_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде forum_view.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800);

    const secretFindings = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    assert.equal(secretFindings.length, 0);

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой: ${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(RUST_MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test: ${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения Rust-тестов: ${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1001-1: сверяет дизайн-токены, раскладку, состояния forum-post-card и схему канала type 15 с независимыми оракулами", async () => {
    const {
      FORUM_DESIGN_TOKENS,
      FORUM_LAYOUT_SPEC,
      FORUM_POST_CARD_STATES,
      FORUM_PROTOCOL_CONSTANTS,
      verifyForumOracleParity,
    } = await import("../../scripts/verify-forum.mjs");

    // 1. Оракул waves/kanban-board.json (карточка TASK-1001)
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1001 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1001");
    assert.ok(card1001, "Карточка TASK-1001 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(FORUM_DESIGN_TOKENS, card1001.design_tokens);
    assert.deepEqual(FORUM_LAYOUT_SPEC, card1001.layout);
    assert.deepEqual(FORUM_POST_CARD_STATES, card1001.interactive_states[0]);

    // 2. Оракул docs/re/gateway-schema.json и docs/re/gateway-schema.md
    const gwSchemaJson = JSON.parse(readFileSync(GATEWAY_SCHEMA_JSON_PATH, "utf8"));
    const tagPaths0x6a = gwSchemaJson.encoding.tagPaths["tag tag=0x6a"];
    assert.ok(
      tagPaths0x6a.some(
        (/** @type {any} */ entry) =>
          entry[0] === "guilds[].channels[].available_tags" && entry[1] === 67,
      ),
    );
    const gwSchemaMd = readFileSync(GATEWAY_SCHEMA_MD_PATH, "utf8");
    assert.match(gwSchemaMd, /\|\s*`available_tags`\s*\|\s*array\\\|null\s*\|\s*nil\/list\s*\|\s*68\/272\s*\|/);
    assert.match(gwSchemaMd, /\|\s*`default_reaction_emoji`\s*\|\s*object\\\|null\s*\|\s*str\/map\s*\|\s*68\/272\s*\|/);
    assert.match(gwSchemaMd, /\|\s*`default_sort_order`\s*\|\s*null\s*\|\s*str\s*\|\s*68\/272\s*\|/);
    assert.match(gwSchemaMd, /\|\s*`default_forum_layout`\s*\|\s*int\s*\|\s*int\s*\|\s*1\/272\s*\|\s*2\s*\|/);

    // 3. Оракул rust/gui-spike/src/forum.rs
    const spikeForumRs = readFileSync(SPIKE_FORUM_RS_PATH, "utf8");
    assert.match(spikeForumRs, /pub const MAX_FORUM_APPLIED_TAGS: usize = 5;/);
    assert.match(spikeForumRs, /pub const MAX_FORUM_TITLE_CHARS: usize = 100;/);
    assert.match(spikeForumRs, /pub const MAX_FORUM_CONTENT_CHARS: usize = 2000;/);

    assert.equal(FORUM_PROTOCOL_CONSTANTS.CHANNEL_TYPE_GUILD_FORUM, 15);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.CHANNEL_TYPE_PUBLIC_THREAD, 11);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.REQUIRE_TAG_FLAG, 16);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.MAX_AVAILABLE_TAGS, 20);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.MAX_APPLIED_TAGS, 5);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.MAX_CARD_VISIBLE_TAGS, 3);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.CARD_HEIGHT_PX, 96);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.AVATAR_SIZE_PX, 20);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.TITLE_FONT_PX, 15);
    assert.equal(FORUM_PROTOCOL_CONSTANTS.PREVIEW_THUMB_PX, 72);

    const parity = verifyForumOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      gatewaySchemaJsonPath: GATEWAY_SCHEMA_JSON_PATH,
      gatewaySchemaMdPath: GATEWAY_SCHEMA_MD_PATH,
      frontendD03Path: FRONTEND_D03_JSON_PATH,
      spikeForumRsPath: SPIKE_FORUM_RS_PATH,
      rustModulePath: RUST_MODULE_PATH,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);

    const rustOracleTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001_1_forum_channel_parser_and_tokens",
    );
    assert.equal(rustOracleTest.status, 0, rustOracleTest.stderr || rustOracleTest.stdout);
  });

  it("SUB-1001-2: проверяет парсинг GUILD_FORUM (type 15), тегов available_tags и фильтрацию/сортировку постов", async () => {
    const {
      parseGatewayForumChannel,
      filterAndSortForumPosts,
      countForumPostsByTag,
    } = await import("../../scripts/verify-forum.mjs");

    // 1. Парсер канала type 15 из CHANNEL_CREATE / GUILD_CREATE
    const parsed = parseGatewayForumChannel({
      id: "110022003300440055",
      guild_id: "990088007700660055",
      type: 15,
      name: "rust-gpui-architect",
      flags: 16, // REQUIRE_TAG
      default_sort_order: 0,
      default_forum_layout: 2,
      default_tag_setting: "match_all",
      default_auto_archive_duration: 4320,
      default_reaction_emoji: { emoji_id: null, emoji_name: "🔥" },
      available_tags: [
        { id: "101", name: "Bug Report", emoji_id: null, emoji_name: "🐛", moderated: false },
        { id: "102", name: "Release Announcement", emoji_id: "555001", emoji_name: "shipit", moderated: true },
        { id: "101", name: "Duplicate Tag", emoji_id: null, emoji_name: null, moderated: false },
      ],
    });
    assert.equal(parsed.channelType, 15);
    assert.equal(parsed.requireTag, true);
    assert.equal(parsed.defaultSortOrder, 0);
    assert.equal(parsed.defaultForumLayout, 2);
    assert.equal(parsed.defaultTagSetting, "all");
    assert.equal(parsed.defaultAutoArchiveDuration, 4320);
    assert.equal(parsed.availableTags.length, 2);
    assert.equal(parsed.availableTags[1]?.moderated, true);
    assert.equal(parsed.availableTags[1]?.emojiId, "555001");

    // Отказ при неверном типе канала (не 15)
    assert.throws(
      () =>
        parseGatewayForumChannel({
          id: "110022003300440055",
          guild_id: "990088007700660055",
          type: 0,
          name: "general",
        }),
      /GUILD_FORUM.*15/,
    );

    // 2. Фильтрация по тегам (any / all), поисковому запросу и сортировка с закреплёнными постами первыми
    const samplePosts = [
      {
        id: "2001",
        parentChannelId: "110022003300440055",
        title: "Падение шейдера на Wayland 150%",
        authorId: "7001",
        authorName: "developer",
        createdAtMs: 1_700_000_100_000,
        lastActivityMs: 1_700_000_500_000,
        appliedTags: ["101"],
        pinned: false,
        archived: false,
        messageCount: 14,
        reactions: [{ emojiName: "🔥", count: 5 }],
      },
      {
        id: "2002",
        parentChannelId: "110022003300440055",
        title: "Правила публикации в форуме",
        authorId: "7002",
        authorName: "moderator",
        createdAtMs: 1_700_000_050_000,
        lastActivityMs: 1_700_000_200_000,
        appliedTags: ["101", "102"],
        pinned: true,
        archived: false,
        messageCount: 3,
        reactions: [{ emojiName: "🔥", count: 42 }],
      },
      {
        id: "2003",
        parentChannelId: "110022003300440055",
        title: "Релиз сборки 0.9.0",
        authorId: "7003",
        authorName: "devops",
        createdAtMs: 1_700_000_400_000,
        lastActivityMs: 1_700_000_900_000,
        appliedTags: ["102"],
        pinned: false,
        archived: false,
        messageCount: 128,
        reactions: [{ emojiName: "🔥", count: 19 }],
      },
      {
        id: "2004",
        parentChannelId: "110022003300440055",
        title: "Старый архивный тред",
        authorId: "7004",
        authorName: "ghost",
        createdAtMs: 1_690_000_000_000,
        lastActivityMs: 1_700_009_999_000,
        appliedTags: ["101", "102"],
        pinned: false,
        archived: true,
        messageCount: 8,
        reactions: [],
      },
    ];

    // По умолчанию: архивный 2004 отброшен, закреплённый 2002 первый, затем 2003 (активность 900) и 2001 (активность 500)
    const defaultSorted = filterAndSortForumPosts(samplePosts, {
      availableTagIds: ["101", "102"],
      selectedTagIds: [],
      matchMode: "any",
      sortOrder: 0,
    });
    assert.deepEqual(
      defaultSorted.map((p) => p.id),
      ["2002", "2003", "2001"],
    );

    // Фильтр matchMode: "all" по ["101", "102"] оставляет только 2002
    const allMatched = filterAndSortForumPosts(samplePosts, {
      availableTagIds: ["101", "102"],
      selectedTagIds: ["101", "102"],
      matchMode: "all",
      sortOrder: 0,
    });
    assert.deepEqual(
      allMatched.map((p) => p.id),
      ["2002"],
    );

    // Сортировка по дате создания (sortOrder = 1): 2002 (pinned), затем 2003 (created 400), затем 2001 (created 100)
    const byCreated = filterAndSortForumPosts(samplePosts, {
      availableTagIds: ["101", "102"],
      selectedTagIds: [],
      matchMode: "any",
      sortOrder: 1,
    });
    assert.deepEqual(
      byCreated.map((p) => p.id),
      ["2002", "2003", "2001"],
    );

    // Подсчёт активных постов по тегам
    const tagCounts = countForumPostsByTag(samplePosts);
    assert.deepEqual(tagCounts, { "101": 2, "102": 2 });

    const rustFilterTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001_2_tag_filtering_and_sorting",
    );
    assert.equal(rustFilterTest.status, 0, rustFilterTest.stderr || rustFilterTest.stdout);
  });

  it("SUB-1001-3: проверяет компоновку сетки ForumGridView, карточки постов (96px, теги +N, превью вложения, счётчики ответов 99+ и реакций) и A11y-клавиатуру", async () => {
    const {
      computeForumGridLayout,
      navigateForumGridKeyboard,
    } = await import("../../scripts/verify-forum.mjs");

    const posts = [
      {
        id: "3001",
        parentChannelId: "110022003300440055",
        title: "Адаптивная сетка карточек форума в GPUI",
        authorId: "8001",
        authorName: "ui_engineer",
        createdAtMs: 1_700_010_000_000,
        lastActivityMs: 1_700_020_000_000,
        appliedTags: ["101", "102", "103", "104", "105"],
        pinned: true,
        archived: false,
        messageCount: 142,
        reactions: [
          { emojiName: "🔥", count: 12 },
          { emojiName: "👍", count: 7 },
        ],
        firstAttachment: {
          id: "9001",
          url: "https://cdn.discordapp.com/attachments/110/9001/preview.png",
          width: 640,
          height: 360,
          isImage: true,
          spoiler: false,
        },
      },
      {
        id: "3002",
        parentChannelId: "110022003300440055",
        title: "Спойлер-вложение не должно рендерить открытый превью-тамбнейл",
        authorId: "8002",
        authorName: "sec_reviewer",
        createdAtMs: 1_700_005_000_000,
        lastActivityMs: 1_700_015_000_000,
        appliedTags: ["101", "102"],
        pinned: false,
        archived: false,
        messageCount: 99,
        reactions: [{ emojiName: "🔥", count: 4 }],
        firstAttachment: {
          id: "9002",
          url: "https://cdn.discordapp.com/attachments/110/9002/spoiler.png",
          width: 640,
          height: 360,
          isImage: true,
          spoiler: true,
        },
      },
    ];

    const gridGallery = computeForumGridLayout(posts, {
      viewportWidthPx: 960,
      layoutView: 2, // GALLERY -> 2 колонки при ширине >= 600px
      selectedTagIds: ["101"],
      hoveredPostId: "3002",
      openedPostId: "3001",
    });

    assert.equal(gridGallery.container, "Forum Grid");
    assert.equal(gridGallery.dimensions, "100% x 100%");
    assert.equal(gridGallery.paddingPx, 16);
    assert.equal(gridGallery.spacingPx, 12);
    assert.equal(gridGallery.columns, 2);
    assert.equal(gridGallery.activeTagAccentColor, "#5865f2");
    assert.equal(gridGallery.cards.length, 2);

    const card1 = gridGallery.cards[0];
    assert.ok(card1);
    assert.equal(card1.postId, "3001");
    assert.equal(card1.cardHeightPx, 96);
    assert.equal(card1.titleFontPx, 15);
    assert.equal(card1.avatarSizePx, 20);
    assert.deepEqual(card1.visibleTagIds, ["101", "102", "103"]);
    assert.equal(card1.overflowTagCount, 2);
    assert.equal(card1.overflowBadge, "+2");
    assert.equal(card1.replyCount, 142);
    assert.equal(card1.replyBadgeText, "99+");
    assert.equal(card1.reactionCount, 19);
    assert.equal(card1.hasThumbnailPreview, true);
    assert.equal(card1.thumbnailSizePx, 72);
    assert.equal(card1.state, "opened");
    assert.equal(card1.borderToken, "#5865f2");

    const card2 = gridGallery.cards[1];
    assert.ok(card2);
    assert.equal(card2.postId, "3002");
    assert.equal(card2.replyCount, 99);
    assert.equal(card2.replyBadgeText, "99");
    assert.equal(card2.hasThumbnailPreview, false);
    assert.equal(card2.state, "border-accent");
    assert.equal(card2.borderToken, "#5865f2");

    // Навигация стрелками клавиатуры по 2-колоночной сетке (4 карточки)
    let navState = { focusedIndex: 0, totalCards: 4, columns: 2, openedIndex: -1 };
    navState = navigateForumGridKeyboard(navState, "ArrowRight");
    assert.equal(navState.focusedIndex, 1);
    navState = navigateForumGridKeyboard(navState, "ArrowDown");
    assert.equal(navState.focusedIndex, 3);
    navState = navigateForumGridKeyboard(navState, "ArrowLeft");
    assert.equal(navState.focusedIndex, 2);
    navState = navigateForumGridKeyboard(navState, "Enter");
    assert.equal(navState.openedIndex, 2);
    navState = navigateForumGridKeyboard(navState, "Home");
    assert.equal(navState.focusedIndex, 0);

    const rustGridTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001_3_post_grid_cards_and_keyboard_nav",
    );
    assert.equal(rustGridTest.status, 0, rustGridTest.stderr || rustGridTest.stdout);
  });

  it("SUB-1001-4: проверяет пагинацию GET /channels/{id}/threads/active, все 9 измерений TEST-POLICY.md и CLI scripts/verify-forum.mjs (коды 0/1/2)", async () => {
    const {
      paginateActiveForumThreads,
      evaluateForumNineDimensions,
      redactForumDiagnostics,
      runCli,
    } = await import("../../scripts/verify-forum.mjs");

    // 1. Пагинация GET /channels/{id}/threads/active с защитой от архивных тредов и цикла курсоров
    const pageResult = paginateActiveForumThreads(
      [
        {
          cursor: "5001",
          hasMore: true,
          threads: [
            { id: "5001", parentChannelId: "110", title: "Пост 1", archived: false },
            { id: "5002", parentChannelId: "110", title: "Пост 2", archived: false },
          ],
        },
        {
          cursor: "5002",
          hasMore: true,
          threads: [
            { id: "5003", parentChannelId: "110", title: "Пост 3", archived: false },
          ],
        },
        {
          // Повторный курсор 5002 -> защита от бесконечного цикла пагинации
          cursor: "5002",
          hasMore: true,
          threads: [
            { id: "5004", parentChannelId: "110", title: "Дубликат страницы", archived: false },
          ],
        },
      ],
      { channelId: "110" },
    );
    assert.equal(pageResult.route, "/channels/110/threads/active");
    assert.equal(pageResult.threads.length, 3);
    assert.equal(pageResult.loopDetected, true);
    assert.equal(pageResult.hasMore, false);

    // 2. Покрытие всех 9 измерений TEST-POLICY.md
    const dimReport = evaluateForumNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 3. Маскирование секретов и токенов в логах диагностики
    const rawDiag =
      'ForumFetch GET /channels/110/threads/active Authorization: Bearer mfa.secret_forum_token_123456 token="MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz0123456789abcdef"';
    const safeDiag = redactForumDiagnostics(rawDiag);
    assert.doesNotMatch(safeDiag, /mfa\.secret_forum_token/);
    assert.doesNotMatch(safeDiag, /MTIzNDU2Nzg5MDEyMzQ1Njc4/);
    assert.match(safeDiag, /\[REDACTED_TOKEN\]/);

    // 4. Коды выхода CLI: 0 (успех), 1 (неизвестный аргумент), 2 (недоступный оракул)
    const outJsonPath = join(workDir, "forum-report.json");
    const outMdPath = join(workDir, "forum-report.md");
    assert.equal(runCli(["--help"]), 0);
    assert.equal(runCli(["--out-json", outJsonPath, "--out-md", outMdPath]), 0);
    assert.ok(existsSync(outJsonPath));
    assert.ok(existsSync(outMdPath));
    assert.equal(runCli(["--unknown-flag"]), 1);
    assert.equal(runCli(["--oracle", join(workDir, "non-existent-oracle.json")]), 2);

    const rustDimTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001_4_pagination_and_nine_dimensions",
    );
    assert.equal(rustDimTest.status, 0, rustDimTest.stderr || rustDimTest.stdout);
  });

  it("SUB-1001-5: убивает 5/5 контрольных мутаций в forum_view.rs и scripts/verify-forum.mjs (100% mutation kill rate)", async () => {
    const { runForumMutationSuite } = await import("../../scripts/verify-forum.mjs");

    // 1. Мутационный гейт JS-скрипта (5/5)
    const jsMutReport = runForumMutationSuite();
    assert.equal(jsMutReport.total, 5);
    assert.equal(jsMutReport.killed, 5);
    assert.deepEqual(jsMutReport.survivors, []);

    // 2. Мутационный гейт Rust-модуля (5/5 через перекомпиляцию rustc --test)
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1001-RS-01",
        moduleName: "forum_view",
        operator: "ChannelTypeGuildForumTamper",
        from: "pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 15;",
        to: "pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 0;",
      },
      {
        id: "MUT-1001-RS-02",
        moduleName: "forum_view",
        operator: "CardHeightPxTamper",
        from: "pub const FORUM_CARD_HEIGHT_PX: u32 = 96;",
        to: "pub const FORUM_CARD_HEIGHT_PX: u32 = 48;",
      },
      {
        id: "MUT-1001-RS-03",
        moduleName: "forum_view",
        operator: "PinnedPriorityInversion",
        from: "b.pinned.cmp(&a.pinned)",
        to: "a.pinned.cmp(&b.pinned)",
      },
      {
        id: "MUT-1001-RS-04",
        moduleName: "forum_view",
        operator: "ReplyBadgeCapTamper",
        from: "pub const MAX_BADGE_CAP: u32 = 99;",
        to: "pub const MAX_BADGE_CAP: u32 = 999;",
      },
      {
        id: "MUT-1001-RS-05",
        moduleName: "forum_view",
        operator: "ModeratedTagPermissionBypass",
        from: "if tag.moderated && !can_manage_threads {",
        to: "if false && tag.moderated && !can_manage_threads {",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в forum_view.rs`,
      );
      return {
        id: m.id,
        moduleName: m.moduleName,
        operator: m.operator,
        mutatedSource: originalRust.replace(m.from, m.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutSrcPath = join(workDir, `mutant_${idx}.rs`);
      const mutBinPath = join(workDir, `mutant_${idx}_bin`);
      spawnSync("sh", ["-c", `cat > "${mutSrcPath}"`], {
        input: mutant.mutatedSource,
        encoding: "utf8",
      });
      const comp = compileRustModuleTests(mutSrcPath, mutBinPath);
      if (comp.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
      const run = runCompiledRustTests(mutBinPath);
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test_failed" : "survived",
      };
    });

    const afterMd5 = createHash("md5").update(readFileSync(RUST_MODULE_PATH, "utf8")).digest("hex");
    assert.equal(afterMd5, originalMd5, "MD5 исходного файла forum_view.rs не должен измениться");
    assert.equal(gateResult.passed, true, `Выжившие мутанты: ${gateResult.survivors.join(", ")}`);
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
  });
});
