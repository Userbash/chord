// @ts-check
/**
 * Контрактные и модульные тесты интерактивного пикера эмодзи и векторных Lottie стикеров (TASK-1005B):
 * - SUB-1005B-0: Статический и компиляционный контракт `rust/chord-gpui/src/chat/emoji_picker.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1005B-1: Сверка карточки `TASK-1005B` в `waves/kanban-board.json` (`container: "Emoji Modal"`,
 *   `dimensions: "380px x 420px"`, `padding: "8px"`, `spacing: "4px"`, дизайн-токены `#2b2d31`, `#1e1f22`,
 *   `#ffffff`, `#5865f2`, `#3f4147`, интерактивные состояния `emoji-tile`: `tile -> scale-110 -> inserted`)
 *   и артефактов RE-оракула `/home/sanya/re-deep/ui8/` (`21-emoji-picker.json`, `21b-emoji-full.json`,
 *   `21d-emoji-categories.json`, `21-emoji-search.json`, `21c-emoji-full-search.json`, `log-emoji6.json`,
 *   `log-emoji9.json`, `21-emoji-picker.html`) и `docs/re/frontend-d03.json` (экран `emoji`).
 * - SUB-1005B-2: GPUI модалка `Emoji Modal` (`380x420px`, 3 вкладки `Гифки` / `Стикеры` / `Эмодзи`,
 *   навигация по 10 категориям, быстрый поиск `:fire` с отсечением префикса `:`, сетка 8 колонок по 32px
 *   с виртуальным скроллом `scrollerBase_d125d2` / `listItems_affa7e` и состояния `emoji-tile`).
 * - SUB-1005B-3: Вкладка стикеров и покадровый рендеринг анимированных векторных Lottie JSON кадров
 *   (валидация холста <= 1024x1024, лимит 512 KiB, fps 1..240, интерполяция ключевых кадров,
 *   де-премультипликация RGBA8, пауза при `reducedMotion` без наведения и бюджет 120 FPS).
 * - SUB-1005B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, коды `400/401/403/404/413/429/5xx`, пустые результаты поиска, `skeleton`/`HTTP 202`,
 *   права доступа `VIEW_CHANNEL`/`USE_EXTERNAL_EMOJIS`/`USE_EXTERNAL_STICKERS`/`AGE_GATED_SPACES`
 *   и маскирование секретов в логах телеметрии).
 * - SUB-1005B-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const MODULE_REL_PATH = "rust/chord-gpui/src/chat/emoji_picker.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_UI8_DIR = "/home/sanya/re-deep/ui8";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-gpui/src/chat/emoji_picker.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1005B: Интерактивный пикер эмодзи и векторных Lottie стикеров (Попап 380x420px, сетка 8x32px, категории, поиск и Lottie кадры)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "emoji-picker-1005b-"));
    testBinPath = join(workDir, "emoji_picker_1005b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1005B-0: модуль rust/chord-gpui/src/chat/emoji_picker.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "Модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде emoji_picker.rs запрещены .unwrap() и .expect()",
    );

    const secretFindings = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены незамаскированные секреты в исходнике: ${JSON.stringify(secretFindings)}`,
    );

    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(MODULE_PATH, testBinPath);
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

    const allRun = runCompiledRustTests(testBinPath);
    assert.equal(
      allRun.status,
      0,
      `Встроенные тесты emoji_picker.rs упали:\n${allRun.stdout}\n${allRun.stderr}`,
    );
    assert.match(allRun.stdout, /test result: ok\./);
  });

  it("SUB-1005B-1: сверяет карточку TASK-1005B в waves/kanban-board.json (Emoji Modal 380x420px, padding 8px, spacing 4px, emoji-tile: tile -> scale-110 -> inserted) и артефакты оракула /home/sanya/re-deep/ui8/ (21-emoji-picker.json, 21b-emoji-full.json, 21d-emoji-categories.json, 21-emoji-search.json, log-emoji6.json)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-1005B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1005b = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1005B",
    );
    assert.ok(
      card1005b,
      "Карточка TASK-1005B должна существовать в waves/kanban-board.json",
    );
    assert.equal(card1005b.department, "G2 Frontend");
    assert.equal(card1005b.priority, "P2");
    assert.deepEqual(card1005b.layout, {
      container: "Emoji Modal",
      dimensions: "380px x 420px",
      padding: "8px",
      spacing: "4px",
    });
    assert.deepEqual(card1005b.design_tokens, {
      background: "#2b2d31",
      surface: "#1e1f22",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1005b.interactive_states, [
      {
        element: "emoji-tile",
        default: "tile",
        hover: "scale-110",
        active: "inserted",
      },
    ]);

    // 2. Независимый оракул: /home/sanya/re-deep/ui8/ (и docs/re/frontend-d03.json)
    const d03 = JSON.parse(readFileSync(FRONTEND_D03_PATH, "utf8"));
    const emojiScreen = d03.screens.find(
      (/** @type {any} */ s) => s.name === "emoji",
    );
    assert.ok(emojiScreen, "Экран emoji должен присутствовать в frontend-d03.json");
    assert.equal(emojiScreen.counts.tabCount, 3);
    assert.equal(emojiScreen.counts.navCategories, 10);
    assert.equal(emojiScreen.counts.visibleGridChannel, 50);
    assert.equal(emojiScreen.counts.lockedChannel, 17);
    assert.equal(emojiScreen.counts.visibleGridDM, 42);
    assert.equal(emojiScreen.counts.reactionPickerItems, 60);
    assert.equal(emojiScreen.counts.searchFireChannel, 10);
    assert.equal(emojiScreen.counts.searchFireDM, 11);

    if (existsSync(RE_UI8_DIR)) {
      const picker21 = JSON.parse(
        readFileSync(join(RE_UI8_DIR, "21-emoji-picker.json"), "utf8"),
      );
      assert.equal(picker21.gridCount, 50);
      assert.equal(picker21.lockedCount, 17);
      assert.equal(picker21.searchPh, "Найдите идеальный эмодзи");
      assert.equal(picker21.searchAria, "Поиск");
      assert.equal(picker21.skinTone, "Выбрать цвет кожи эмодзи (Пусто)");
      assert.deepEqual(
        picker21.tabs.map((/** @type {any} */ t) => t.text),
        ["Гифки", "Стикеры", "Эмодзи"],
      );
      assert.deepEqual(picker21.sections, [
        "Лучшие эмодзи сервера ITSKIDON PC BOOST",
        "Часто используемые",
      ]);

      const cats21d = JSON.parse(
        readFileSync(join(RE_UI8_DIR, "21d-emoji-categories.json"), "utf8"),
      );
      assert.equal(cats21d.count, 10);
      assert.deepEqual(
        cats21d.items.map((/** @type {any} */ i) => i.aria),
        emojiScreen.structure.navRail.categories,
      );

      const full21b = JSON.parse(
        readFileSync(join(RE_UI8_DIR, "21b-emoji-full.json"), "utf8"),
      );
      assert.equal(full21b.found, true);
      assert.equal(full21b.gridCount, 42);
      assert.equal(full21b.nav[0].aria, "Категории меню выбора реакции");
      assert.equal(full21b.nav[0].cls, "navList__08434");

      const search21 = JSON.parse(
        readFileSync(join(RE_UI8_DIR, "21-emoji-search.json"), "utf8"),
      );
      assert.equal(search21.count, 10);
      assert.deepEqual(search21.sections, ["Доступно с подпиской Nitro"]);

      const search21c = JSON.parse(
        readFileSync(join(RE_UI8_DIR, "21c-emoji-full-search.json"), "utf8"),
      );
      assert.equal(search21c.count, 11);

      const log6 = JSON.parse(
        readFileSync(join(RE_UI8_DIR, "log-emoji6.json"), "utf8"),
      );
      const pickerStruct = log6.find(
        (/** @type {any} */ e) => e.name === "picker-structure",
      );
      assert.ok(pickerStruct);
      assert.equal(pickerStruct.cls, "emojiPicker_c0e32c");
      assert.equal(pickerStruct.gridCount, 60);

      const pickerHtml = readFileSync(
        join(RE_UI8_DIR, "21-emoji-picker.html"),
        "utf8",
      );
      assert.match(pickerHtml, /stickersNavItem__08434/);
      assert.match(pickerHtml, /scrollerBase_d125d2/);
      assert.match(pickerHtml, /listItems_affa7e/);
    }

    // 3. Сверка исходника emoji_picker.rs с оракулами TASK-1005B и re-deep/ui8/
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1005b.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} должен быть зафиксирован в emoji_picker.rs`,
      );
    }
    for (const marker of [
      "Emoji Modal",
      "380px x 420px",
      "emoji-tile",
      "scale-110",
      "inserted",
      "expressionPickerPositionLayer__74017",
      "emojiPicker_c0e32c",
      "scrollerBase_d125d2",
      "listItems_affa7e",
      "navList__08434",
      "stickersNavItem__08434",
      "emojiItem_fc7141",
      "lockedEmoji_d982c2",
      "input__75098",
      "Найдите идеальный эмодзи",
      "Выбрать цвет кожи эмодзи (Пусто)",
      "Доступно с подпиской Nitro",
    ]) {
      assert.ok(
        source.includes(marker),
        `Маркер оракула re-deep/ui8 ${marker} должен присутствовать в emoji_picker.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1005b_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005B-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005b_1_.*ok/);
  });

  it("SUB-1005B-2: проверяет GPUI модалку 380x420px, переключение 3 вкладок (Гифки/Стикеры/Эмодзи), навигацию по 10 категориям, быстрый поиск и сетку 8 колонок по 32px с виртуальным скроллом", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "EMOJI_MODAL_WIDTH_PX",
      "EMOJI_MODAL_HEIGHT_PX",
      "EMOJI_MODAL_PADDING_PX",
      "EMOJI_MODAL_SPACING_PX",
      "EMOJI_GRID_COLUMNS",
      "EMOJI_TILE_SIZE_PX",
      "EmojiTileState",
      "PickerTab",
      "EmojiTileEntry",
      "EmojiGridTileRenderSpec",
      "VirtualGridSliceSpec",
      "EmojiModalLayoutSpec",
      "compute_emoji_modal_layout",
      "compute_virtual_grid_slice",
      "filter_emojis_by_category_and_query",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1005b_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005B-2 (GPUI модалка 380x420px, сетка 8x32px и поиск) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005b_2_.*ok/);
  });

  it("SUB-1005B-3: проверяет вкладку стикеров и покадровый рендерер анимированных Lottie JSON кадров (интерполяция ключевых кадров, де-премультипликация RGBA8, пауза при reducedMotion и бюджет 120 FPS)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "LottieKeyframe",
      "LottieAnimationTrack",
      "LottieFrameRenderResult",
      "unpremultiply_rgba8",
      "interpolate_lottie_keyframes",
      "render_lottie_animation_frame",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1005b_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005B-3 (рендеринг анимированных Lottie JSON кадров) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005b_3_.*ok/);
  });

  it("SUB-1005B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400..5xx, пустые, skeleton/202, права доступа и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "validate_picker_workload_boundaries",
      "format_picker_badge",
      "classify_picker_http_status",
      "redact_picker_secrets",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1005b_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005B-4 (9 измерений TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005b_4_.*ok/);
  });

  it("SUB-1005B-5: убивает 5/5 контрольных мутаций модуля emoji_picker.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(source).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-emoji-picker-01",
        operator: "ModalDimensions380x420Tamper",
        from: "pub const EMOJI_MODAL_WIDTH_PX: u32 = 380;",
        to: "pub const EMOJI_MODAL_WIDTH_PX: u32 = 320;",
      },
      {
        id: "MUT-emoji-picker-02",
        operator: "GridColumns8x32Tamper",
        from: "pub const EMOJI_GRID_COLUMNS: usize = 8;",
        to: "pub const EMOJI_GRID_COLUMNS: usize = 4;",
      },
      {
        id: "MUT-emoji-picker-03",
        operator: "TileHoverStateScale110Tamper",
        from: "Self::Scale110 => \"scale-110\",",
        to: "Self::Scale110 => \"tile\",",
      },
      {
        id: "MUT-emoji-picker-04",
        operator: "SearchColonPrefixNormalizationDisabled",
        from: "let needle = query.trim().trim_matches(':').to_ascii_lowercase();",
        to: "let needle = query.trim().to_ascii_lowercase();",
      },
      {
        id: "MUT-emoji-picker-05",
        operator: "LottieFrameAdvancementFrozenOnPlay",
        from: "let current_frame = track.in_point + (elapsed_frames % span);",
        to: "let current_frame = track.in_point;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "emoji_picker.rs",
        operator: spec.operator,
        mutatedSource: source.replace(spec.from, spec.to),
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
      .update(readFileSync(MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла emoji_picker.rs не должен измениться",
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
