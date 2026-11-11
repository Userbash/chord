// @ts-check
/**
 * Контрактные и модульные тесты UI-карточки опроса `PollCardView` с анимированными шкалами
 * результатов и бейджем победителя (TASK-1004B):
 * - SUB-1004B-0: Статический и компиляционный контракт `rust/discord-gpui/src/chat/poll_view.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1004B-1: Сверка карточек `TASK-1004B` и `TASK-1004` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`, заливка прогресс-бара `#5865f233`),
 *   геометрии `Poll Card` (`100% x auto`, `padding: 12px`, `spacing: 8px`), интерактивных состояний
 *   `option-row` (`idle` -> `hover-bg` -> `selected`) и `poll-option` (`radio` -> `hover bg` -> `voted`),
 *   компонентов `/home/sanya/re-deep/frontend-d03.json` (`Button` `button_a22cb0`, `Badge` `numberBadge__463b7` /
 *   `iconBadge__463b7` / `badge_e7c728`, `Switch` `role="switch"` / `BaseRadioGroup`, `Tooltip` `tooltip_c36707`,
 *   `Avatar` `avatar_fdd9da` / `avatarStack__44b0c`, фильтр поиска `"опрос"`) и модулей `modules-index.json`
 *   (`290794`, `355682`, `412136`, `789646`, `803931` `pollContainer_b7e1cb`, `969632` `poll_question_text`).
 * - SUB-1004B-2: GPUI-компонент `PollCardViewSpec` и строки вариантов `PollOptionRowViewSpec`,
 *   анимированная интерполяция шкалы прогресса 120 FPS (`ease_out_cubic`, `interpolate_progress_bar`,
 *   заливка цветом `#5865f233`, расчёт ширины в пикселях и процентах с защитой от деления на ноль).
 * - SUB-1004B-3: Определение лидирующего и победившего варианта по завершении опроса (`WinnerBadgeKind`:
 *   `None`, `LeadingActive`, `SoleWinner`, `TiedWinner`), бейдж победителя при истечении дедлайна или
 *   финализации, тултип проголосовавших (`format_voter_tooltip_ru`) и клавиатурная навигация по опциям
 *   (`ArrowDown`, `ArrowUp`, `Home`, `End`).
 * - SUB-1004B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, вопрос `1..300`, ответ `1..55`, `1..10` вариантов,
 *   2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки HTTP
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустые `0` голосов; `skeleton`/`HTTP 202`;
 *   права `VIEW_CHANNEL`/`SEND_MESSAGES`/`SEND_POLLS`/`AGE_GATED_SPACES`; восстановление/откат;
 *   маскирование секретов).
 * - SUB-1004B-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/chat/poll_view.rs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";
import {
  computePollResults as spikeComputePollResults,
  formatPollExpiryRu as spikeFormatPollExpiryRu,
} from "../../rust/gui-spike/ui/features/poll-progress.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/chat/poll_view.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_DEEP_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const DOCS_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/chat/poll_view.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1004B: UI-карточка опроса PollCardView с анимированными шкалами результатов (#5865f233) и бейджем победителя (rust/discord-gpui/src/chat/poll_view.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "poll-view-1004b-"));
    testBinPath = join(workDir, "poll_view_1004b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1004B-0: проверяет статический и компиляционный контракт rust/discord-gpui/src/chat/poll_view.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024, 0 .unwrap()/.expect() и отсутствие секретов)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле poll_view.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде poll_view.rs запрещены .unwrap() и .expect()",
    );

    for (const requiredSymbol of [
      'pub const POLL_CARD_CONTAINER: &str = "Poll Card";',
      'pub const POLL_CARD_DIMENSIONS: &str = "100% x auto";',
      "pub const POLL_CARD_PADDING_PX: u32 = 12;",
      "pub const POLL_CARD_SPACING_PX: u32 = 8;",
      'pub const TOKEN_PROGRESS_FILL_HEX: &str = "#5865f233";',
      'pub const OPTION_ROW_ELEMENT: &str = "option-row";',
      "pub enum OptionRowInteractiveState",
      "pub enum WinnerBadgeKind",
      "pub struct ProgressAnimationSpec",
      "pub struct PollOptionRowViewSpec",
      "pub struct PollCardViewSpec",
      "pub fn interpolate_progress_bar(",
      "pub fn resolve_winner_badge(",
      "pub fn build_poll_card_view(",
    ]) {
      assert.ok(
        rustSource.includes(requiredSymbol),
        `В poll_view.rs должен присутствовать контракт TASK-1004B: ${requiredSymbol}`,
      );
    }

    const sizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

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
      `rustfmt --check завершился с ошибкой:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(RUST_MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения встроенных Rust-тестов:\n${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1004B-1: сверяет карточку TASK-1004B в waves/kanban-board.json и оракул re-deep/frontend-d03.json (Button button_a22cb0, Badge numberBadge__463b7/iconBadge__463b7, Switch role=switch, Tooltip tooltip_c36707, Avatar avatarStack__44b0c и фильтр «опрос»)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);

    const kanban = JSON.parse(kanbanRaw);
    const card1004B = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1004B",
    );
    assert.ok(
      card1004B,
      "Карточка TASK-1004B должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1004B.department, "G2 Frontend");
    assert.equal(card1004B.priority, "P1");
    assert.deepEqual(card1004B.layout, {
      container: "Poll Card",
      dimensions: "100% x auto",
      padding: "12px",
      spacing: "8px",
    });
    assert.deepEqual(card1004B.design_tokens, {
      background: "#2b2d31",
      surface: "#1e1f22",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1004B.interactive_states, [
      {
        element: "option-row",
        default: "idle",
        hover: "hover-bg",
        active: "selected",
      },
    ]);

    const card1004 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1004",
    );
    assert.ok(card1004, "Карточка TASK-1004 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(card1004.interactive_states[0], {
      element: "poll-option",
      default: "radio",
      hover: "hover bg",
      active: "voted",
    });

    const d03Path = existsSync(RE_DEEP_FRONTEND_D03_PATH)
      ? RE_DEEP_FRONTEND_D03_PATH
      : DOCS_FRONTEND_D03_PATH;
    const d03 = JSON.parse(readFileSync(d03Path, "utf8"));
    assert.equal(d03.meta.task, "D-03");

    const searchScreen = d03.screens.find(
      (/** @type {any} */ s) => s.name === "search",
    );
    assert.ok(searchScreen);
    assert.ok(
      searchScreen.structure.filters.containsOptions.includes("опрос"),
      "В фильтрах поиска frontend-d03.json должна присутствовать опция «опрос»",
    );

    const buttonComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Button",
    );
    assert.ok(buttonComp);
    assert.match(buttonComp.evidence[1], /button_a22cb0/);
    assert.match(buttonComp.evidence[1], /active_a22cb0/);

    const badgeComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Badge",
    );
    assert.ok(badgeComp);
    assert.match(badgeComp.evidence[0], /numberBadge__463b7/);
    assert.match(badgeComp.evidence[0], /iconBadge__463b7/);
    assert.match(badgeComp.evidence[1], /badge_e7c728/);

    const switchComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Switch",
    );
    assert.ok(switchComp);
    assert.ok(switchComp.variants.includes("radio-like toggle"));
    assert.ok(
      switchComp.tokens.includes("--switch-background-selected-default"),
    );

    const tooltipComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Tooltip",
    );
    assert.ok(tooltipComp);
    assert.match(tooltipComp.evidence[0], /tooltip_c36707/);

    const avatarComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Avatar",
    );
    assert.ok(avatarComp);
    assert.match(avatarComp.evidence[0], /avatar_fdd9da/);
    assert.match(avatarComp.evidence[2], /avatarStack__44b0c/);

    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modulesIdx["803931"].strings.includes("pollContainer_b7e1cb"));
      assert.ok(modulesIdx["355682"].strings.includes("end-poll-early"));
      assert.ok(modulesIdx["969632"].strings.includes("poll_question_text"));
    }

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const marker of [
      "#2b2d31",
      "#1e1f22",
      "#ffffff",
      "#5865f2",
      "#3f4147",
      "#5865f233",
      "button_a22cb0",
      "numberBadge__463b7",
      "iconBadge__463b7",
      "tooltip_c36707",
      "avatar_fdd9da",
      "avatarStack__44b0c",
      "pollContainer_b7e1cb",
      "end-poll-early",
      "poll_question_text",
    ]) {
      assert.ok(
        rustSource.includes(marker),
        `В poll_view.rs должен быть зафиксирован маркер оракула: ${marker}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1004b_1_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1004B-1 в Rust упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004b_1_.*ok/);
  });

  it("SUB-1004B-2: проверяет GPUI-компонент PollCardViewSpec, анимированную заливку прогресс-бара цветом #5865f233 и сверку процентов с эталоном poll-progress.js", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const spikeRes = spikeComputePollResults(
      [
        { answerId: "1", text: "Rust + GPUI 120 FPS" },
        { answerId: "2", text: "Electron WebView" },
        { answerId: "3", text: "Tauri Hybrid" },
      ],
      [
        { id: "1", count: 6, meVoted: true },
        { id: "2", count: 1, meVoted: false },
        { id: "3", count: 3, meVoted: false },
      ],
    );
    assert.equal(spikeRes.totalVotes, 10);
    assert.deepEqual(
      spikeRes.items.map((i) => i.percentage),
      [60, 10, 30],
    );
    assert.equal(spikeRes.items[0]?.isLeading, true);
    assert.equal(spikeRes.items[0]?.meVoted, true);

    const nowMs = 1_762_000_000_000;
    assert.equal(spikeFormatPollExpiryRu(nowMs, nowMs), "Опрос завершён");
    assert.equal(
      spikeFormatPollExpiryRu(nowMs + 2 * 3_600_000, nowMs),
      "Осталось 2 ч.",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1004b_2_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1004B-2 (GPUI PollCardViewSpec и анимация прогресс-бара #5865f233) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004b_2_.*ok/);
  });

  it("SUB-1004B-3: проверяет индикатор победившего варианта по завершении опроса (SoleWinner, TiedWinner, LeadingActive), тултип проголосовавших и клавиатурную навигацию", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_1004b_3_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1004B-3 (индикатор победителя и навигация) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004b_3_.*ok/);
  });

  it("SUB-1004B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 1..10 ответов, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400/401/403/404/413/429/5xx, пустые 0 голосов, skeleton/202, права SEND_POLLS/AGE_GATED_SPACES и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_1004b_4_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1004B-4 (9 измерений TEST-POLICY.md) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004b_4_.*ok/);
  });

  it("SUB-1004B-5: убивает 5/5 контрольных мутаций модуля poll_view.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(RUST_MODULE_PATH), "Исходник модуля должен существовать");
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-1004B-01",
        operator: "ProgressFillColorTamper",
        from: 'pub const TOKEN_PROGRESS_FILL_HEX: &str = "#5865f233";',
        to: 'pub const TOKEN_PROGRESS_FILL_HEX: &str = "#00000000";',
      },
      {
        id: "MUT-1004B-02",
        operator: "OptionRowActiveStateTamper",
        from: 'Self::Selected => "selected",',
        to: 'Self::Selected => "idle",',
      },
      {
        id: "MUT-1004B-03",
        operator: "WinnerBadgeExpiredGateBypass",
        from: "if !expired {",
        to: "if expired {",
      },
      {
        id: "MUT-1004B-04",
        operator: "ProgressInterpolationZeroed",
        from: "let current_pct = from_clamped + (to_clamped - from_clamped) * eased;",
        to: "let current_pct = 0.0_f32;",
      },
      {
        id: "MUT-1004B-05",
        operator: "SendPollsPermissionCheckBypassed",
        from: "if !is_private && (permissions & PERM_SEND_POLLS) == 0 {",
        to: "if false && !is_private && (permissions & PERM_SEND_POLLS) == 0 {",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в poll_view.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "poll_view.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
      };
    });

    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");

      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compilation_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      if (run.status !== 0) {
        return { killed: true, reason: "test_assertion_failed" };
      }
      return { killed: false, reason: "mutant_survived" };
    });

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(afterMd5, originalMd5, "MD5 исходного файла poll_view.rs не должен измениться");

    assert.equal(
      gate.passed,
      true,
      `Мутационный гейт не пройден (выжили: ${gate.survivors.join(", ")})`,
    );
    assert.equal(gate.killed, 5);
    assert.equal(gate.total, 5);
    assert.equal(gate.killRate, 1.0);
  });
});
