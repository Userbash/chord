// @ts-check
/**
 * Контрактные и модульные тесты интерфейса панели звуковых эффектов `SoundboardGridView`,
 * анимации нажатия плитки, привязки хоткеев и слайдера громкости SFX (TASK-1012C):
 * - SUB-1012C-0: Статический и компиляционный контракт `rust/chord-gpui/src/voice/soundboard_view.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1012C-1: Сверка карточек `TASK-1012C` и `TASK-1012` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Soundboard Grid`
 *   (`320px x 360px`, `padding: 8px`, `spacing: 6px`), интерактивных состояний `sound-tile`
 *   (`tile` -> `scale-105` -> `playing sound`) и `sound-button` (`sfx tile` -> `scale-105` -> `playing sfx`),
 *   а также независимых RE-оракулов (`/home/sanya/re-deep/voice/ui7/`, `/home/sanya/re-deep/voice/ui/01-candidates.json`,
 *   `docs/re/frontend-d03.json`, `/home/sanya/re-deep/modules-index.json`).
 * - SUB-1012C-2: GPUI сетка `SoundboardGridView` (`320x360px`, 3 колонки плиток `97x76px`, поиск,
 *   фильтрация недоступных звуков `available !== false`, сортировка с приоритетом избранного,
 *   рендер эмодзи/названия/хоткея и 120 FPS анимация нажатия `interpolate_tile_press_animation`).
 * - SUB-1012C-3: Реестр хоткеев `SoundboardHotkeyRegistry` (модификаторы `Ctrl`/`Shift`/`Alt`/`Super`,
 *   защита от одиночных клавиш без модификатора, детекция конфликтов, триггер воспроизведения) и
 *   слайдер громкости SFX в настройках клиента (`0..=200%`, мут `AUDIO_TOGGLE_LOCAL_SOUNDBOARD_MUTE`,
 *   формула эффективного усиления из RE-модуля `904054`).
 * - SUB-1012C-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`,
 *   громкость `0..200%`, бейдж `99+`, избранное `250`, имя `2..=32` симв.; ошибки HTTP
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустые состояния; `skeleton`/`HTTP 202`;
 *   права `CONNECT`/`SPEAK`/`USE_SOUNDBOARD = 1 << 42`/`USE_EXTERNAL_SOUNDS = 1 << 45`/`AGE_GATED_SPACES`;
 *   сброс активного звука при смене канала; маскирование токенов в логах).
 * - SUB-1012C-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/chord-gpui/src/voice/soundboard_view.rs`.
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
  MAX_SOUNDBOARD_BYTES as SPIKE_MAX_SOUNDBOARD_BYTES,
  MAX_SOUNDBOARD_NAME_CHARS as SPIKE_MAX_SOUNDBOARD_NAME_CHARS,
  MIN_SOUNDBOARD_NAME_CHARS as SPIKE_MIN_SOUNDBOARD_NAME_CHARS,
  filterSoundboardSounds as spikeFilterSoundboardSounds,
} from "../../rust/gui-spike/ui/features/soundboard-picker.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/voice/soundboard_view.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_UI7_DIR = "/home/sanya/re-deep/voice/ui7";
const RE_VOICE_UI_CANDIDATES_PATH =
  "/home/sanya/re-deep/voice/ui/01-candidates.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-gpui/src/voice/soundboard_view.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1012C: Сетка кнопок саундборда SoundboardGridView, анимация нажатия, хоткеи и регулировка громкости SFX", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "soundboard-view-1012c-"));
    testBinPath = join(workDir, "soundboard_view_1012c_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1012C-0: проверяет статический и компиляционный контракт rust/chord-gpui/src/voice/soundboard_view.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле soundboard_view.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде soundboard_view.rs запрещены .unwrap() и .expect()",
    );

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
      `Ожидался размер модуля 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены секреты в исходнике: ${JSON.stringify(secretFindings)}`,
    );

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
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Компиляция не должна порождать предупреждений:\n${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения встроенных Rust-тестов:\n${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1012C-1: сверяет карточку TASK-1012C в waves/kanban-board.json (Soundboard Grid 320x360px, padding 8px, spacing 6px, sound-tile: tile -> scale-105 -> playing sound) и RE-оракулы re-deep/voice/ui7/, frontend-d03.json и modules-index.json", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточки TASK-1012C и TASK-1012 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1012C = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1012C",
    );
    const card1012 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1012",
    );
    assert.ok(
      card1012C && card1012,
      "Карточки TASK-1012C и TASK-1012 должны присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1012C.department, "G2 Frontend");
    assert.equal(card1012C.priority, "P2");
    assert.deepEqual(card1012C.layout, {
      container: "Soundboard Grid",
      dimensions: "320px x 360px",
      padding: "8px",
      spacing: "6px",
    });
    assert.deepEqual(card1012C.design_tokens, {
      background: "#2b2d31",
      surface: "#1e1f22",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1012C.interactive_states, [
      {
        element: "sound-tile",
        default: "tile",
        hover: "scale-105",
        active: "playing sound",
      },
    ]);
    assert.deepEqual(card1012.interactive_states, [
      {
        element: "sound-button",
        default: "sfx tile",
        hover: "scale-105",
        active: "playing sfx",
      },
    ]);

    // 2. Оракул: /home/sanya/re-deep/voice/ui7/
    if (existsSync(RE_UI7_DIR)) {
      const voiceRunLog = readFileSync(
        join(RE_UI7_DIR, "voice-run.log"),
        "utf8",
      );
      assert.match(voiceRunLog, /1014537851418980372/);
      assert.match(voiceRunLog, /Голосовая связь подключена/);
      assert.match(voiceRunLog, /button__67645/);
      assert.match(voiceRunLog, /Отключиться/);
      assert.match(voiceRunLog, /requireModule\(discord_voice\)/);

      const ui7Log = JSON.parse(
        readFileSync(join(RE_UI7_DIR, "log.json"), "utf8"),
      );
      const searchBarEntry = ui7Log.find(
        (/** @type {any} */ e) => e.name === "search-bar",
      );
      assert.ok(searchBarEntry);
      assert.equal(searchBarEntry.cls, "search__49676");
      assert.equal(searchBarEntry.w, 244);
      assert.equal(searchBarEntry.h, 28);

      const searchResults13 = JSON.parse(
        readFileSync(join(RE_UI7_DIR, "13-search-results.json"), "utf8"),
      );
      assert.equal(searchResults13.containers[0].cls, "searchResultsWrap_a98f3b");

      const popout15 = JSON.parse(
        readFileSync(join(RE_UI7_DIR, "15-popout.json"), "utf8"),
      );
      assert.match(popout15.text, /ProBot ✨/);
    }

    if (existsSync(RE_VOICE_UI_CANDIDATES_PATH)) {
      const candidates = JSON.parse(
        readFileSync(RE_VOICE_UI_CANDIDATES_PATH, "utf8"),
      );
      const effectsContainer = candidates.find(
        (/** @type {any} */ c) =>
          c.cls === "voiceChannelEffectsContainer__2f4f7",
      );
      assert.ok(effectsContainer);
      assert.equal(effectsContainer.w, 833);
      assert.equal(effectsContainer.h, 469);
    }

    // 3. Оракул: docs/re/frontend-d03.json (Slider slider__4c059, Button button_a22cb0, Scroller scrollerBase_d125d2)
    const d03 = JSON.parse(readFileSync(FRONTEND_D03_PATH, "utf8"));
    const sliderComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Slider",
    );
    assert.ok(sliderComp);
    assert.match(sliderComp.evidence[0], /slider__4c059/);
    assert.ok(sliderComp.tokens.includes("--slider-track-background"));

    // 4. Оракул: /home/sanya/re-deep/modules-index.json
    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      const table = modIdx.modules ?? modIdx;
      for (const modId of [
        "209932",
        "528772",
        "125831",
        "813564",
        "665252",
        "533978",
        "904054",
      ]) {
        assert.ok(
          modId in table,
          `Модуль ${modId} должен присутствовать в modules-index.json`,
        );
      }
    }

    // 5. Сверка маркеров оракула в soundboard_view.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1012C.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1012C должен присутствовать в soundboard_view.rs`,
      );
    }
    for (const marker of [
      "Soundboard Grid",
      "320px x 360px",
      "sound-tile",
      "scale-105",
      "playing sound",
      "sound-button",
      "sfx tile",
      "playing sfx",
      "voiceChannelEffectsContainer__2f4f7",
      "button__67645",
      "search__49676",
      "slider__4c059",
      "ActionBarSoundboardButton",
      "SoundboardStore",
      "SoundboardOverlayStore",
      "1014537851418980372",
    ]) {
      assert.ok(
        rustSource.includes(marker),
        `Маркер оракула ${marker} должен присутствовать в soundboard_view.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012c_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012C-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012c_1_.*ok/);
  });

  it("SUB-1012C-2: проверяет GPUI сетку SoundboardGridView (320x360px, 3 колонки, фильтрация/поиск звуков с паритетом soundboard-picker.js, рендер эмодзи/названия/хоткея и 120 FPS анимацию нажатия)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // Паритет фильтрации с эталонным soundboard-picker.js
    const sampleSounds = [
      {
        soundId: "1102650064771485746",
        name: "Оу мааай",
        emojiName: "🔥",
        available: true,
      },
      {
        soundId: "1327231186044583947",
        name: "Позвонишь",
        emojiName: "☠️",
        available: false,
      },
      {
        soundId: "1102645616632397904",
        name: "Хабар принёс",
        emojiName: "📦",
        available: true,
      },
    ];
    const filteredSpike = spikeFilterSoundboardSounds(sampleSounds, "");
    assert.equal(filteredSpike.length, 2);
    assert.equal(filteredSpike[0]?.soundId, "1102650064771485746");
    assert.equal(filteredSpike[1]?.soundId, "1102645616632397904");
    assert.equal(SPIKE_MIN_SOUNDBOARD_NAME_CHARS, 2);
    assert.equal(SPIKE_MAX_SOUNDBOARD_NAME_CHARS, 32);
    assert.equal(SPIKE_MAX_SOUNDBOARD_BYTES, 524_288);

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "SOUNDBOARD_GRID_WIDTH_PX",
      "SOUNDBOARD_GRID_HEIGHT_PX",
      "SOUNDBOARD_GRID_PADDING_PX",
      "SOUNDBOARD_GRID_SPACING_PX",
      "SOUNDBOARD_GRID_COLUMNS",
      "SoundTileInteractiveState",
      "SoundboardSoundEntry",
      "SoundTileViewSpec",
      "SoundboardGridViewSpec",
      "TilePressAnimationFrame",
      "filter_and_sort_soundboard_entries",
      "interpolate_tile_press_animation",
      "build_soundboard_grid_view",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_view.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012c_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012C-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012c_2_.*ok/);
  });

  it("SUB-1012C-3: проверяет привязку и диспетчеризацию хоткеев SoundboardHotkeyRegistry и слайдер общей громкости SFX в настройках клиента (0..200%, мут и формулу модуля 904054)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "MOD_CTRL",
      "MOD_SHIFT",
      "MOD_ALT",
      "MOD_SUPER",
      "SoundboardHotkeyBinding",
      "SoundboardHotkeyRegistry",
      "SfxVolumeSliderSpec",
      "format_hotkey_chord",
      "compute_effective_sfx_gain",
      "build_sfx_volume_slider_spec",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_view.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012c_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012C-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012c_3_.*ok/);
  });

  it("SUB-1012C-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права USE_SOUNDBOARD/USE_EXTERNAL_SOUNDS/AGE_GATED_SPACES, восстановление при смене канала и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "evaluate_soundboard_view_permissions",
      "validate_soundboard_view_boundaries",
      "classify_soundboard_view_http_status",
      "format_soundboard_badge_cap",
      "redact_soundboard_view_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_view.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012c_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012C-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012c_4_.*ok/);
  });

  it("SUB-1012C-5: убивает 5/5 контрольных мутаций в rust/chord-gpui/src/voice/soundboard_view.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль soundboard_view.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1012C-RS-01",
        moduleName: "soundboard_view",
        operator: "GridWidth320PxConstantTamper",
        from: "pub const SOUNDBOARD_GRID_WIDTH_PX: u32 = 320;",
        to: "pub const SOUNDBOARD_GRID_WIDTH_PX: u32 = 240;",
      },
      {
        id: "MUT-1012C-RS-02",
        moduleName: "soundboard_view",
        operator: "SoundTileActiveStateLabelTamper",
        from: 'Self::PlayingSound => "playing sound",',
        to: 'Self::PlayingSound => "tile",',
      },
      {
        id: "MUT-1012C-RS-03",
        moduleName: "soundboard_view",
        operator: "HotkeyModifierRequirementBypass",
        from: "if modifiers == 0 {",
        to: "if false && modifiers == 0 {",
      },
      {
        id: "MUT-1012C-RS-04",
        moduleName: "soundboard_view",
        operator: "LocalSoundboardMuteGainBypass",
        from: "if local_soundboard_muted {",
        to: "if false && local_soundboard_muted {",
      },
      {
        id: "MUT-1012C-RS-05",
        moduleName: "soundboard_view",
        operator: "UseSoundboardPermissionCheckBypass",
        from: "if !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
        to: "if false && !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в soundboard_view.rs: ${m.from}`,
      );
      return {
        id: m.id,
        moduleName: m.moduleName,
        operator: m.operator,
        mutatedSource: originalRust.replace(m.from, m.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutSrcPath, mutant.mutatedSource, "utf8");
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
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла soundboard_view.rs не должен измениться после мутационного гейта",
    );
    assert.equal(
      gateResult.passed,
      true,
      `Выжившие мутанты: ${gateResult.survivors.join(", ")}`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1);
  });
});
