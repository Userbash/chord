// @ts-check
/**
 * Контрактные и модульные тесты UI-сетки постов форума `ForumGridView` с фильтрацией тегов (TASK-1001B):
 * - SUB-1001B-0: Статический и компиляционный контракт `rust/chord-gpui/src/channels/forum_view.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` и наличие контрактов `TASK-1001B`).
 * - SUB-1001B-1: Сверка карточки `TASK-1001B` в `waves/kanban-board.json` (`Forum Grid`, `100% x 100%`,
 *   `padding: 16px`, `spacing: 12px`, токены `#313338`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`,
 *   состояния `post-card`: `card -> border-accent -> open thread`) и оракула `re-deep/frontend-d03.json`
 *   (`badge_e7c728 (tagContainer)`, `numberBadge__463b7`, `avatarWrapper_d8a370`, `hasTags`, `scrollerBase_d125d2`).
 * - SUB-1001B-2: 2-колоночная сетка постов форума (`ForumGridViewSpec`, плашки `96px`, координаты `(row, col)`
 *   при ширине >= 600px, фолбэк в 1 колонку, авторы `20px`, заголовки `15px`, превью `72px` без спойлеров
 *   и бейджи ответов `99+`).
 * - SUB-1001B-3: GPUI рендеринг тегов (`ForumTagChipSpec`, мультивыбор `toggle_tag_selection`,
 *   подсветка активных тегов токеном `#5865f2`, счётчики постов по тегам) и агрегация счётчиков реакций
 *   (`compute_post_reaction_summary`).
 * - SUB-1001B-4: Навигация клавиатурой по 2-колоночной сетке (`ArrowRight`, `ArrowLeft`, `ArrowDown`,
 *   `ArrowUp`, `Home`, `End`, `Enter`, `Space`) и проверка всех 9 измерений `TEST-POLICY.md`.
 * - SUB-1001B-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/chord-gpui/src/channels/forum_view.rs`.
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
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/channels/forum_view.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_DEEP_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const DOCS_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");

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

describe("TASK-1001B: 2-колоночная сетка постов форума ForumGridView (96px плашки), мультивыбор тегов и счётчики реакций", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "forum-view-1001b-"));
    testBinPath = join(workDir, "forum_view_1001b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1001B-0: проверяет статический контракт rust/chord-gpui/src/channels/forum_view.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и наличие структур TASK-1001B)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
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

    // Обязательные контракты TASK-1001B
    for (const requiredSymbol of [
      "pub const POST_CARD_ELEMENT: &str = \"post-card\";",
      "pub const POST_CARD_STATE_DEFAULT: &str = \"card\";",
      "pub const POST_CARD_STATE_HOVER: &str = \"border-accent\";",
      "pub const POST_CARD_STATE_ACTIVE: &str = \"open thread\";",
      "pub const BADGE_TAG_CONTAINER_CLASS: &str = \"badge_e7c728\";",
      "pub struct ForumTagChipSpec",
      "pub struct ForumGridCellSpec",
      "pub struct ForumGridViewSpec",
      "pub fn toggle_tag_selection(",
      "pub fn build_tag_filter_bar(",
      "pub fn compute_post_reaction_summary(",
      "pub fn build_forum_grid_view(",
      "fn test_sub_1001b_forum_grid_view_tags_and_reactions()",
    ]) {
      assert.ok(
        rustSource.includes(requiredSymbol),
        `В forum_view.rs должен присутствовать контракт TASK-1001B: ${requiredSymbol}`,
      );
    }

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

  it("SUB-1001B-1: сверяет карточку TASK-1001B в waves/kanban-board.json и оракул re-deep/frontend-d03.json (Badge badge_e7c728, numberBadge__463b7, Avatar avatarWrapper_d8a370, TextInput hasTags)", () => {
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);

    const kanban = JSON.parse(kanbanRaw);
    const card1001B = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1001B");
    assert.ok(card1001B, "Карточка TASK-1001B должна присутствовать в waves/kanban-board.json");
    assert.equal(card1001B.department, "G2 Frontend");
    assert.deepEqual(card1001B.layout, {
      container: "Forum Grid",
      dimensions: "100% x 100%",
      padding: "16px",
      spacing: "12px",
    });
    assert.deepEqual(card1001B.design_tokens, {
      background: "#313338",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1001B.interactive_states, [
      {
        element: "post-card",
        default: "card",
        hover: "border-accent",
        active: "open thread",
      },
    ]);

    const d03Path = existsSync(RE_DEEP_FRONTEND_D03_PATH)
      ? RE_DEEP_FRONTEND_D03_PATH
      : DOCS_FRONTEND_D03_PATH;
    const d03Raw = readFileSync(d03Path, "utf8");
    const d03 = JSON.parse(d03Raw);
    assert.equal(d03.meta.task, "D-03");

    const badgeComp = d03.components.find((/** @type {any} */ c) => c.name === "Badge");
    assert.ok(badgeComp, "В frontend-d03.json обязан присутствовать компонент Badge");
    assert.ok(
      badgeComp.variants.includes("badge тега сервера (badge_e7c728)"),
      "В Badge должен быть вариант badge_e7c728",
    );
    assert.ok(
      badgeComp.evidence.some((/** @type {string} */ e) => e.includes("numberBadge__463b7")),
      "В Badge должен быть класс numberBadge__463b7",
    );

    const avatarComp = d03.components.find((/** @type {any} */ c) => c.name === "Avatar");
    assert.ok(avatarComp, "В frontend-d03.json обязан присутствовать компонент Avatar");
    assert.ok(
      avatarComp.variants.some((/** @type {string} */ v) => v.includes("avatarWrapper_d8a370")),
    );

    const inputComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "TextInput / TextArea",
    );
    assert.ok(inputComp);
    assert.ok(inputComp.states.includes("hasTags"));

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(rustSource, /pub const BADGE_TAG_CONTAINER_CLASS: &str = "badge_e7c728";/);
    assert.match(rustSource, /pub const NUMBER_BADGE_CLASS: &str = "numberBadge__463b7";/);
    assert.match(rustSource, /pub const AVATAR_WRAPPER_CLASS: &str = "avatarWrapper_d8a370";/);
  });

  it("SUB-1001B-2: проверяет 2-колоночную сетку ForumGridView (96px плашки, координаты row/col, состояния post-card: card -> border-accent -> open thread)", async () => {
    const { computeForumGridLayout } = await import("../../scripts/verify-forum.mjs");

    const posts = [
      {
        id: "4001",
        parentChannelId: "110",
        title: "Пост 1 (закреплённый, открытый тред)",
        authorId: "9001",
        authorName: "alice",
        createdAtMs: 1000,
        lastActivityMs: 5000,
        appliedTags: ["101", "102", "103", "104"],
        pinned: true,
        archived: false,
        messageCount: 120,
        reactions: [
          { emojiName: "🔥", count: 8 },
          { emojiName: "❤️", count: 4 },
        ],
        firstAttachment: {
          id: "701",
          url: "https://cdn.discordapp.com/attachments/110/701/thumb.png",
          width: 400,
          height: 300,
          isImage: true,
          spoiler: false,
        },
      },
      {
        id: "4002",
        parentChannelId: "110",
        title: "Пост 2 (ховер border-accent)",
        authorId: "9002",
        authorName: "bob",
        createdAtMs: 2000,
        lastActivityMs: 4000,
        appliedTags: ["101"],
        pinned: false,
        archived: false,
        messageCount: 15,
        reactions: [{ emojiName: "👍", count: 3 }],
      },
      {
        id: "4003",
        parentChannelId: "110",
        title: "Пост 3 (вторая строка 2-колоночной сетки)",
        authorId: "9003",
        authorName: "charlie",
        createdAtMs: 1500,
        lastActivityMs: 3000,
        appliedTags: ["102"],
        pinned: false,
        archived: false,
        messageCount: 0,
        reactions: [],
      },
    ];

    const grid2Col = computeForumGridLayout(posts, {
      viewportWidthPx: 1280,
      layoutView: 2,
      selectedTagIds: ["101"],
      hoveredPostId: "4002",
      openedPostId: "4001",
    });
    assert.equal(grid2Col.container, "Forum Grid");
    assert.equal(grid2Col.dimensions, "100% x 100%");
    assert.equal(grid2Col.paddingPx, 16);
    assert.equal(grid2Col.spacingPx, 12);
    assert.equal(grid2Col.columns, 2);
    assert.equal(grid2Col.cards.length, 3);
    assert.equal(grid2Col.cards[0]?.cardHeightPx, 96);
    assert.equal(grid2Col.cards[0]?.replyBadgeText, "99+");
    assert.equal(grid2Col.cards[0]?.reactionCount, 12);
    assert.equal(grid2Col.cards[0]?.overflowBadge, "+1");
    assert.equal(grid2Col.cards[1]?.borderToken, "#5865f2");
    assert.equal(grid2Col.cards[2]?.borderToken, "#3f4147");

    const rustTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001b_forum_grid_view_tags_and_reactions",
    );
    assert.equal(rustTest.status, 0, rustTest.stderr || rustTest.stdout);
  });

  it("SUB-1001B-3: проверяет мультивыбор тегов с подсветкой #5865f2, счётчики постов по тегам и агрегацию реакций", async () => {
    const {
      filterAndSortForumPosts,
      countForumPostsByTag,
    } = await import("../../scripts/verify-forum.mjs");

    const posts = [
      {
        id: "1",
        title: "WGPU 120 FPS Grid",
        appliedTags: ["10", "20"],
        pinned: false,
        archived: false,
        createdAtMs: 100,
        lastActivityMs: 500,
        messageCount: 7,
        reactions: [{ emojiName: "🔥", count: 5 }],
      },
      {
        id: "2",
        title: "Tag Filter Multi-Select",
        appliedTags: ["20", "30"],
        pinned: true,
        archived: false,
        createdAtMs: 200,
        lastActivityMs: 300,
        messageCount: 19,
        reactions: [{ emojiName: "🚀", count: 11 }],
      },
    ];

    const counts = countForumPostsByTag(posts);
    assert.deepEqual(counts, { "10": 1, "20": 2, "30": 1 });

    const anyFiltered = filterAndSortForumPosts(posts, {
      availableTagIds: ["10", "20", "30"],
      selectedTagIds: ["10", "30"],
      matchMode: "any",
    });
    assert.deepEqual(
      anyFiltered.map((p) => p.id),
      ["2", "1"],
    );

    const allFiltered = filterAndSortForumPosts(posts, {
      availableTagIds: ["10", "20", "30"],
      selectedTagIds: ["20", "30"],
      matchMode: "all",
    });
    assert.deepEqual(
      allFiltered.map((p) => p.id),
      ["2"],
    );

    const rustTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001b_forum_grid_view_tags_and_reactions",
    );
    assert.equal(rustTest.status, 0, rustTest.stderr || rustTest.stdout);
  });

  it("SUB-1001B-4: проверяет клавиатурную навигацию по 2-колоночной сетке и все 9 измерений TEST-POLICY.md", async () => {
    const {
      navigateForumGridKeyboard,
      evaluateForumNineDimensions,
    } = await import("../../scripts/verify-forum.mjs");

    let state = { focusedIndex: 0, totalCards: 6, columns: 2, openedIndex: -1 };
    state = navigateForumGridKeyboard(state, "ArrowDown");
    assert.equal(state.focusedIndex, 2);
    state = navigateForumGridKeyboard(state, "ArrowRight");
    assert.equal(state.focusedIndex, 3);
    state = navigateForumGridKeyboard(state, "ArrowDown");
    assert.equal(state.focusedIndex, 5);
    state = navigateForumGridKeyboard(state, "ArrowUp");
    assert.equal(state.focusedIndex, 3);
    state = navigateForumGridKeyboard(state, "Space");
    assert.equal(state.openedIndex, 3);
    state = navigateForumGridKeyboard(state, "Home");
    assert.equal(state.focusedIndex, 0);
    state = navigateForumGridKeyboard(state, "End");
    assert.equal(state.focusedIndex, 5);

    const dimReport = evaluateForumNineDimensions();
    assert.equal(dimReport.passed, true, dimReport.failures.join("; "));
    assert.equal(dimReport.dimensionsChecked, 9);

    const rustNavTest = runCompiledRustTests(
      testBinPath,
      "test_sub_1001_3_post_grid_cards_and_keyboard_nav",
    );
    assert.equal(rustNavTest.status, 0, rustNavTest.stderr || rustNavTest.stdout);
  });

  it("SUB-1001B-5: убивает 5/5 контрольных мутаций TASK-1001B в forum_view.rs (100% mutation kill rate)", () => {
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1001B-RS-01",
        moduleName: "forum_view",
        operator: "GalleryTwoColumnTamper",
        from: "Self::Gallery if viewport_width_px >= FORUM_GALLERY_MIN_WIDTH_PX => 2,",
        to: "Self::Gallery if viewport_width_px >= FORUM_GALLERY_MIN_WIDTH_PX => 1,",
      },
      {
        id: "MUT-1001B-RS-02",
        moduleName: "forum_view",
        operator: "CardHeight96PxTamper",
        from: "pub const FORUM_CARD_HEIGHT_PX: u32 = 96;",
        to: "pub const FORUM_CARD_HEIGHT_PX: u32 = 64;",
      },
      {
        id: "MUT-1001B-RS-03",
        moduleName: "forum_view",
        operator: "ActiveTagAccentTokenTamper",
        from: "let background_token = if selected {",
        to: "let background_token = if false {",
      },
      {
        id: "MUT-1001B-RS-04",
        moduleName: "forum_view",
        operator: "MultiSelectTagToggleRemoveBypass",
        from: "if let Some(pos) = next.iter().position(|&id| id == tag_id) {",
        to: "if let Some(pos) = None::<usize> {",
      },
      {
        id: "MUT-1001B-RS-05",
        moduleName: "forum_view",
        operator: "KeyboardGridArrowDownTamper",
        from: "\"ArrowDown\" => ((current + cols).min(total_cards - 1), None),",
        to: "\"ArrowDown\" => (current, None),",
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
      const mutSrcPath = join(workDir, `mutant_1001b_${idx}.rs`);
      const mutBinPath = join(workDir, `mutant_1001b_${idx}_bin`);
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

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(afterMd5, originalMd5, "MD5 исходного файла forum_view.rs не должен измениться");
    assert.equal(
      gateResult.passed,
      true,
      `Выжившие мутанты: ${gateResult.survivors.join(", ")}`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
  });
});
