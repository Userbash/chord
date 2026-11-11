// @ts-check
/**
 * Контрактные и модульные тесты рендеринга волнистых опечаток (#f23f43) в TextView
 * и всплывающего контекстного меню замены при клике ПКМ (TASK-1006B):
 * - SUB-1006B-0: Статический и компиляционный контракт `rust/discord-gpui/src/chat/input_spellcheck.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1006B-1: Сверка карточки `TASK-1006B` (и смежных `TASK-1006`, `TASK-1006A`) в `waves/kanban-board.json`
 *   (`container: "Input Decorator"`, `dimensions: "TextView overlay"`, `padding: "0px"`, `spacing: "0px"`,
 *   дизайн-токены `#383a40`, `#2b2d31`, `#ffffff`, `#f23f43`, `#3f4147`, интерактивные состояния
 *   `wavy-underline`: `red-wave` -> `pointer` -> `context menu`) и независимого оракула
 *   `/home/sanya/re-deep/frontend-d03.json` (`TextInput / TextArea` `103557`/`358983` `input__75098`,
 *   `Menu (Mana)` `798819`/`980707` `menu_c1e9c4`/`scroller_c1e9c4`/`separator_c1e9c4`,
 *   `ContextMenu` `837528`/`442433` `CONTEXT_MENU_OPEN`/`CONTEXT_MENU_CLOSE`, `Popover` `179657` `layer__59d0d`,
 *   `Tooltip` `233680` `tooltip_c36707`, `editor__25bec`).
 * - SUB-1006B-2: GPUI-оверлей `InputDecoratorView` (`TextView overlay`), дебаунс расчёта координат `150 мс`
 *   без торможения набора текста (`<= 8.33 мс` бюджет 120 FPS), генерация геометрии красной волнистой линии
 *   `#f23f43` (`WavyUnderlineSegment`, `WavyWavePoint`, расчёт колонок/строк и синусоидальных вершин волны)
 *   и hit-testing курсора (`red-wave` -> `pointer` -> `context menu`).
 * - SUB-1006B-3: Всплывающее контекстное меню при клике ПКМ (`SpellcheckContextMenuPopup`, `menu_c1e9c4`),
 *   ограничение координат попапа в пределах окна (`clamp_popup_to_viewport`), навигация с клавиатуры
 *   (`ArrowDown`, `ArrowUp`, `Escape`), замена слова с опечаткой в 1 клик с пересчётом позиции курсора
 *   (`DISCORD_SPELLCHECK_REPLACE_MISSPELLING`) и добавление/удаление из пользовательского словаря
 *   (`DISCORD_SPELLCHECK_SET_LEARNED_WORDS`).
 * - SUB-1006B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, дебаунс `149 мс` vs `150 мс`, `2000` символов, `10` файлов/`25 МБ`,
 *   `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки HTTP `400/401/403/404/413/429 retry_after_ms/5xx`;
 *   пустые тексты и 0 вариантов замены; `skeleton`/`HTTP 202`; права `VIEW_CHANNEL`/`SEND_MESSAGES`/`AGE_GATED_SPACES`;
 *   восстановление/отмена меню; маскирование секретов в логах телеметрии).
 * - SUB-1006B-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/chat/input_spellcheck.rs`.
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
  WAVY_UNDERLINE_STATES,
  SPELLCHECK_DEBOUNCE_MS,
  SPELLCHECK_DESIGN_TOKENS,
  resolveWavyUnderlineState,
  tokenizeSpellcheckInput,
  SpellcheckerInputSession,
} from "../../scripts/verify-spellchecker.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/chat/input_spellcheck.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_DEEP_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const DOCS_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
const BACKEND_IPC_PATH = join(ROOT, "docs", "re", "backend-ipc.json");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/chat/input_spellcheck.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1006B: Красная волнистая линия #f23f43 в TextView и всплывающее меню вариантов при клике ПКМ (rust/discord-gpui/src/chat/input_spellcheck.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "input-spellcheck-1006b-"));
    testBinPath = join(workDir, "input_spellcheck_1006b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1006B-0: проверяет статический и компиляционный контракт rust/discord-gpui/src/chat/input_spellcheck.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024, 0 .unwrap()/.expect() и отсутствие секретов)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле input_spellcheck.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде input_spellcheck.rs запрещены .unwrap() и .expect()",
    );

    for (const requiredSymbol of [
      'pub const INPUT_DECORATOR_CONTAINER: &str = "Input Decorator";',
      'pub const INPUT_DECORATOR_DIMENSIONS: &str = "TextView overlay";',
      "pub const INPUT_DECORATOR_PADDING_PX: u32 = 0;",
      "pub const INPUT_DECORATOR_SPACING_PX: u32 = 0;",
      'pub const TOKEN_WAVY_UNDERLINE_HEX: &str = "#f23f43";',
      'pub const WAVY_UNDERLINE_ELEMENT: &str = "wavy-underline";',
      "pub const COORDINATE_DEBOUNCE_MS: u64 = 150;",
      "pub enum WavyUnderlineInteractiveState",
      "pub struct WavyWavePoint",
      "pub struct WavyUnderlineSegment",
      "pub struct SpellcheckMenuItemSpec",
      "pub struct SpellcheckContextMenuPopup",
      "pub struct InputSpellcheckDecorator",
      "pub fn generate_wavy_vertices(",
      "pub fn build_wavy_underline_segments(",
    ]) {
      assert.ok(
        rustSource.includes(requiredSymbol),
        `В input_spellcheck.rs должен присутствовать контракт TASK-1006B: ${requiredSymbol}`,
      );
    }

    const sizeCheck = checkRustSourceSizeLimits(
      RUST_MODULE_REL_PATH,
      rustSource,
    );
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

  it("SUB-1006B-1: сверяет карточку TASK-1006B в waves/kanban-board.json и оракул re-deep/frontend-d03.json (TextInput/TextArea input__75098, Menu menu_c1e9c4/scroller_c1e9c4/separator_c1e9c4, ContextMenu CONTEXT_MENU_OPEN/CLOSE, Popover layer__59d0d и 6 IPC-каналов DISCORD_SPELLCHECK_*)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);

    const kanban = JSON.parse(kanbanRaw);
    const card1006B = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1006B",
    );
    assert.ok(
      card1006B,
      "Карточка TASK-1006B должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1006B.department, "G2 Frontend");
    assert.equal(card1006B.priority, "P2");
    assert.deepEqual(card1006B.layout, {
      container: "Input Decorator",
      dimensions: "TextView overlay",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1006B.design_tokens, {
      background: "#383a40",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#f23f43",
      border: "#3f4147",
    });
    assert.deepEqual(card1006B.interactive_states, [
      {
        element: "wavy-underline",
        default: "red-wave",
        hover: "pointer",
        active: "context menu",
      },
    ]);
    assert.deepEqual(card1006B.interactive_states[0], WAVY_UNDERLINE_STATES);

    const d03Path = existsSync(RE_DEEP_FRONTEND_D03_PATH)
      ? RE_DEEP_FRONTEND_D03_PATH
      : DOCS_FRONTEND_D03_PATH;
    const d03 = JSON.parse(readFileSync(d03Path, "utf8"));
    assert.equal(d03.meta.task, "D-03");

    const textInputComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "TextInput / TextArea",
    );
    assert.ok(textInputComp);
    assert.ok(textInputComp.states.includes("invalid/error"));
    assert.ok(textInputComp.tokens.includes("--input-border-error-default"));
    assert.ok(textInputComp.tokens.includes("--input-text-error-default"));
    assert.match(textInputComp.evidence[0], /text-area/);
    assert.match(textInputComp.evidence[1], /input__75098/);

    const menuComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Menu (Mana)",
    );
    assert.ok(menuComp);
    assert.ok(menuComp.variants.includes("separator"));
    assert.match(menuComp.evidence[0], /menu_c1e9c4/);
    assert.match(menuComp.evidence[0], /scroller_c1e9c4/);
    assert.match(menuComp.evidence[0], /separator_c1e9c4/);

    const contextMenuComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "ContextMenu",
    );
    assert.ok(contextMenuComp);
    assert.match(contextMenuComp.evidence[0], /CONTEXT_MENU_OPEN\/CLOSE/);
    assert.match(contextMenuComp.evidence[0], /442433/);

    const popoverComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Popover",
    );
    assert.ok(popoverComp);
    assert.match(popoverComp.tokens[0], /layer__59d0d/);

    const tooltipComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Tooltip",
    );
    assert.ok(tooltipComp);
    assert.match(tooltipComp.evidence[0], /tooltip_c36707/);

    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modulesIdx["638128"].names.includes("SpellcheckStore"));
      assert.ok(modulesIdx["900582"].strings.includes("discord_spellcheck"));
      assert.ok(modulesIdx["442433"].strings.includes("CONTEXT_MENU_OPEN"));
    }

    const ipcDoc = JSON.parse(readFileSync(BACKEND_IPC_PATH, "utf8"));
    const ipcNames = new Set(
      (ipcDoc.channels ?? []).map((/** @type {any} */ ch) => ch.name),
    );
    assert.ok(ipcNames.has("DISCORD_SPELLCHECK_REPLACE_MISSPELLING"));
    assert.ok(ipcNames.has("DISCORD_SPELLCHECK_SET_LEARNED_WORDS"));

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const marker of [
      "Input Decorator",
      "TextView overlay",
      "#383a40",
      "#2b2d31",
      "#ffffff",
      "#f23f43",
      "#3f4147",
      "wavy-underline",
      "red-wave",
      "context menu",
      "input__75098",
      "editor__25bec",
      "menu_c1e9c4",
      "scroller_c1e9c4",
      "separator_c1e9c4",
      "CONTEXT_MENU_OPEN",
      "CONTEXT_MENU_CLOSE",
      "layer__59d0d",
      "tooltip_c36707",
      "--input-border-error-default",
      "--input-text-error-default",
      "DISCORD_SPELLCHECK_REPLACE_MISSPELLING",
      "DISCORD_SPELLCHECK_SET_LEARNED_WORDS",
    ]) {
      assert.ok(
        rustSource.includes(marker),
        `В input_spellcheck.rs должен быть зафиксирован маркер оракула: ${marker}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1006b_1_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1006B-1 в Rust упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006b_1_.*ok/);
  });

  it("SUB-1006B-2: проверяет GPUI-оверлей TextView, дебаунс расчёта координат 150 мс без торможения набора текста и генерацию красной волнистой линии #f23f43 (WavyUnderlineSegment, вершины волны и состояния red-wave -> pointer -> context menu)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    assert.equal(SPELLCHECK_DEBOUNCE_MS, 150);
    assert.equal(SPELLCHECK_DESIGN_TOKENS.accent, "#f23f43");
    assert.equal(resolveWavyUnderlineState("default"), "red-wave");
    assert.equal(resolveWavyUnderlineState("hover"), "pointer");
    assert.equal(resolveWavyUnderlineState("active"), "context menu");

    const sample = "Привет wrold и helo!";
    const tokens = tokenizeSpellcheckInput(sample);
    assert.deepEqual(
      tokens.map((t) => t.text),
      ["Привет", "wrold", "и", "helo"],
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1006b_2_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1006B-2 (GPUI волнистая линия #f23f43 и дебаунс 150 мс) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006b_2_.*ok/);
  });

  it("SUB-1006B-3: проверяет всплывающее меню вариантов при клике ПКМ (menu_c1e9c4, позиционирование во вьюпорте, клавиатурная навигация, замена слова в 1 клик с обновлением курсора и добавление в словарь)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const session = new SpellcheckerInputSession();
    session.onTextInput("Привет wrold и helo!", 1_000);
    assert.equal(session.flushAt(1_149).executed, false);
    const flushed = session.flushAt(1_150);
    assert.equal(flushed.executed, true);
    assert.equal(flushed.misspelledSpans.length, 2);

    const menu = session.openContextMenuOnRightClick(
      (flushed.misspelledSpans[0]?.startByte ?? 0) + 1,
      { x: 240, y: 520 },
    );
    assert.equal(menu.opened, true);
    assert.equal(menu.wavyState, "context menu");
    const replaced = session.selectContextMenuSuggestion(0);
    assert.equal(replaced.text, "Привет world и helo!");
    assert.equal(replaced.ipcChannel, "DISCORD_SPELLCHECK_REPLACE_MISSPELLING");

    const run = runCompiledRustTests(testBinPath, "test_sub_1006b_3_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1006B-3 (контекстное меню ПКМ и замена в 1 клик) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006b_3_.*ok/);
  });

  it("SUB-1006B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 149/150 мс, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400/401/403/404/413/429/5xx, пустые, skeleton/202, права SEND_MESSAGES/AGE_GATED_SPACES, восстановление и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_1006b_4_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1006B-4 (9 измерений TEST-POLICY.md) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006b_4_.*ok/);
  });

  it("SUB-1006B-5: убивает 5/5 контрольных мутаций модуля input_spellcheck.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Исходник модуля input_spellcheck.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-1006B-01",
        operator: "WavyUnderlineColorTamper",
        from: 'pub const TOKEN_WAVY_UNDERLINE_HEX: &str = "#f23f43";',
        to: 'pub const TOKEN_WAVY_UNDERLINE_HEX: &str = "#000000";',
      },
      {
        id: "MUT-1006B-02",
        operator: "CoordinateDebounceThresholdBypass",
        from: "pub const COORDINATE_DEBOUNCE_MS: u64 = 150;",
        to: "pub const COORDINATE_DEBOUNCE_MS: u64 = 10;",
      },
      {
        id: "MUT-1006B-03",
        operator: "WavyUnderlineContextMenuStateTamper",
        from: 'Self::ContextMenu => "context menu",',
        to: 'Self::ContextMenu => "red-wave",',
      },
      {
        id: "MUT-1006B-04",
        operator: "OneClickWordReplacementSkipped",
        from: "self.text.replace_range(span_start..span_end, &replacement);",
        to: "let _ = (span_start, span_end, &replacement);",
      },
      {
        id: "MUT-1006B-05",
        operator: "SendMessagesPermissionGateBypassed",
        from: "if (permissions & PERM_SEND_MESSAGES) == 0 {",
        to: "if false && (permissions & PERM_SEND_MESSAGES) == 0 {",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в input_spellcheck.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "input_spellcheck.rs",
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
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла input_spellcheck.rs не должен измениться",
    );

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
