// @ts-check
/**
 * Контрактные и модульные тесты пользовательских тем оформления и динамической CSS палитры (TASK-985):
 * - SUB-985-0: Статический контракт нативного модуля `rust/discord-gpui/src/custom_themes.rs`
 *   (`#![forbid(unsafe_code)]`, лимиты `scripts/quality-gate.mjs`: функции < 50 строк,
 *   модуль < 800 строк, 0 `.unwrap()`/`.expect()` в продуктовом коде, чистый `rustfmt --check --edition 2024`
 *   и компиляция `rustc --test --edition 2024 -D warnings`).
 * - SUB-985-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-985`,
 *   `rust/gui-spike/ui/tokens.css` 204 базовых токена, `waves/RW-40/theme-spec.json` 57 переопределений Light,
 *   `rust/gui-spike/ui/features/token-map.js` 25 канонических соответствий, `docs/re/frontend-d03.json`,
 *   `docs/reference-projects/sources/Serein/extensions/themes/midnight.serein-extension`).
 * - SUB-985-2: Горячее переключение тем (`dark`, `light`, `midnight`, `amoled` и пользовательских тем)
 *   без перезагрузки интерфейса (`reloaded === false`, `domReloadCount === 0`), динамическая инъекция
 *   CSS-переменных в `documentElement`, машина состояний `theme-preview` (`dark -> highlight theme -> applied`)
 *   и аварийный сброс по `Ctrl+Shift+F12`.
 * - SUB-985-3: Валидация CSS-токенов на контрастность WCAG 2.2 AA (`>= 4.5:1` для текста, `>= 3.0:1` для
 *   нетекстовых элементов и кольца фокуса, `>= 7.0:1` AAA для основного текста) и защита от CSS-инъекций
 *   (`url()`, `@import`, `@font-face`, `expression()`, `javascript:`, `!important`, циклических `var()`).
 * - SUB-985-4: Покрытие всех 9 измерений `TEST-POLICY.md` (позитивные, негативные, граничные `4.49` vs `4.50`,
 *   лимит `8` пользовательских тем, Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`,
 *   громкость `0..200%`, бейдж `99+`, ошибки `401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`,
 *   права доступа, аварийное восстановление, совместимость `.serein-extension` и маскирование секретов).
 * - SUB-985-5: Мутационный гейт `5/5` (`100%` kill rate) для Rust и JS модулей по контракту `scripts/quality-gate.mjs`.
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

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/custom_themes.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const JS_MODULE_REL_PATH = "rust/gui-spike/ui/features/custom-themes.js";
const JS_MODULE_PATH = join(ROOT, JS_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const UI_TOKENS_CSS_PATH = join(ROOT, "rust", "gui-spike", "ui", "tokens.css");
const THEME_SPEC_JSON_PATH = join(ROOT, "waves", "RW-40", "theme-spec.json");
const FRONTEND_D03_JSON_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const SEREIN_MIDNIGHT_PATH = join(
  ROOT,
  "docs",
  "reference-projects",
  "sources",
  "Serein",
  "extensions",
  "themes",
  "midnight.serein-extension",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `custom_themes.rs` через `rustc --test --edition 2024 -D warnings`.
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

/**
 * Создаёт детерминированный тестовый DOM-документ для проверки горячей инъекции CSS без перезагрузки.
 */
function createMockDomDocument() {
  /** @type {Map<string, string>} */
  const attributes = new Map();
  /** @type {Map<string, string>} */
  const inlineProperties = new Map();
  /** @type {Set<string>} */
  const classes = new Set(["theme-dark", "images-dark"]);
  /** @type {Map<string, { id: string, textContent: string }>} */
  const styleElements = new Map();
  let domReloadCount = 0;

  const documentElement = {
    setAttribute(/** @type {string} */ name, /** @type {string} */ value) {
      attributes.set(name, String(value));
    },
    getAttribute(/** @type {string} */ name) {
      return attributes.get(name) ?? null;
    },
    removeAttribute(/** @type {string} */ name) {
      attributes.delete(name);
    },
    classList: {
      add(/** @type {string[]} */ ...tokens) {
        for (const t of tokens) classes.add(t);
      },
      remove(/** @type {string[]} */ ...tokens) {
        for (const t of tokens) classes.delete(t);
      },
      contains(/** @type {string} */ token) {
        return classes.has(token);
      },
      toArray() {
        return [...classes];
      },
    },
    style: {
      setProperty(/** @type {string} */ prop, /** @type {string} */ val) {
        inlineProperties.set(prop, String(val));
      },
      getPropertyValue(/** @type {string} */ prop) {
        return inlineProperties.get(prop) ?? "";
      },
      removeProperty(/** @type {string} */ prop) {
        inlineProperties.delete(prop);
      },
      snapshot() {
        return Object.fromEntries(inlineProperties.entries());
      },
    },
  };

  return {
    documentElement,
    head: {
      appendChild(/** @type {{ id: string, textContent: string }} */ el) {
        styleElements.set(el.id, el);
      },
    },
    createElement(/** @type {string} */ tag) {
      return { tagName: tag.toUpperCase(), id: "", textContent: "" };
    },
    getElementById(/** @type {string} */ id) {
      return styleElements.get(id) ?? null;
    },
    location: {
      reload() {
        domReloadCount += 1;
      },
    },
    getReloadCount() {
      return domReloadCount;
    },
  };
}

describe("TASK-985: Пользовательские темы оформления и динамическая CSS палитра (custom_themes.rs + custom-themes.js)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "custom-themes-985-"));
    testBinPath = join(workDir, "custom_themes_985_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-985-0: модули custom_themes.rs и custom-themes.js существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), rustfmt 2024 и компилируются без предупреждений", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Модуль ${RUST_MODULE_REL_PATH} должен существовать`,
    );
    assert.ok(
      existsSync(JS_MODULE_PATH),
      `Модуль ${JS_MODULE_REL_PATH} должен существовать`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /#!\[forbid\(unsafe_code\)\]/,
      "Rust-модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = rustSource.split("#[cfg(test)]")[0] ?? rustSource;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде custom_themes.rs запрещены .unwrap() и .expect()",
    );

    const rustSizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      rustSizeCheck.passed,
      true,
      `Нарушение лимитов размера Rust-кода: ${rustSizeCheck.reason}`,
    );
    assert.ok(
      rustSizeCheck.fileLines > 150 && rustSizeCheck.fileLines < 800,
      `Ожидался размер файла 150..800 строк, получено: ${rustSizeCheck.fileLines}`,
    );

    const jsSource = readFileSync(JS_MODULE_PATH, "utf8");
    const jsLines = jsSource.split("\n").length;
    assert.ok(
      jsLines > 120 && jsLines < 800,
      `Ожидался размер JS-модуля 120..800 строк, получено: ${jsLines}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл:\n${fmtCheck.stderr || fmtCheck.stdout}`,
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

    const rustRun = runCompiledRustTests(testBinPath);
    assert.equal(
      rustRun.status,
      0,
      `Встроенные тесты custom_themes.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-985-1: сверяет дизайн-токены, макет Theme Picker, палитры (Midnight, Dark, Light, AMOLED) и интерактивные состояния с независимыми оракулами", async () => {
    const {
      THEME_PICKER_LAYOUT,
      THEME_PICKER_DESIGN_TOKENS,
      THEME_PREVIEW_STATES,
      BUILTIN_THEME_IDS,
      BUILTIN_THEME_PALETTES,
      MAX_INSTALLED_CUSTOM_THEMES,
      EMERGENCY_RESET_SHORTCUT,
      verifyCustomThemesOracleParity,
    } = await import("../../rust/gui-spike/ui/features/custom-themes.js");

    // 1. Независимый оракул: карточка TASK-985 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card985 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-985");
    assert.ok(card985, "Карточка TASK-985 должна существовать в waves/kanban-board.json");
    assert.deepEqual(THEME_PICKER_LAYOUT, card985.layout);
    assert.deepEqual(THEME_PICKER_DESIGN_TOKENS, card985.design_tokens);
    assert.deepEqual(THEME_PREVIEW_STATES, card985.interactive_states[0]);

    // 2. Четыре обязательные темы Discord из описания задачи: Dark, Light, Midnight, AMOLED
    assert.deepEqual(BUILTIN_THEME_IDS, ["dark", "light", "midnight", "amoled"]);
    assert.equal(MAX_INSTALLED_CUSTOM_THEMES, 8);
    assert.equal(EMERGENCY_RESET_SHORTCUT, "Ctrl+Shift+F12");

    for (const id of BUILTIN_THEME_IDS) {
      const palette = BUILTIN_THEME_PALETTES[id];
      assert.ok(palette, `Палитра встроенной темы ${id} должна быть определена`);
      assert.equal(palette.id, id);
      assert.ok(palette.tokens["--bg-app"]);
      assert.ok(palette.tokens["--bg-chat"]);
      assert.ok(palette.tokens["--text-strong"]);
      assert.ok(palette.tokens["--text"]);
      assert.ok(palette.tokens["--accent"]);
    }

    // Проверка истинного чёрного (#000000) для AMOLED и глубокого индиго для Midnight
    assert.equal(BUILTIN_THEME_PALETTES.amoled.tokens["--bg-app"], "#000000");
    assert.equal(BUILTIN_THEME_PALETTES.amoled.tokens["--bg-chat"], "#000000");
    assert.equal(BUILTIN_THEME_PALETTES.light.colorScheme, "light");
    assert.equal(BUILTIN_THEME_PALETTES.dark.colorScheme, "dark");

    // 3. Полная сверка с файлами оракулов репозитория
    const oracleReport = verifyCustomThemesOracleParity({
      kanbanBoardPath: KANBAN_BOARD_PATH,
      uiTokensCssPath: UI_TOKENS_CSS_PATH,
      themeSpecJsonPath: THEME_SPEC_JSON_PATH,
      frontendD03JsonPath: FRONTEND_D03_JSON_PATH,
      sereinMidnightPath: SEREIN_MIDNIGHT_PATH,
    });
    assert.equal(
      oracleReport.passed,
      true,
      `Несовпадение с оракулами: ${oracleReport.reasons.join("; ")}`,
    );
    assert.equal(oracleReport.rootTokenCount, 204);
    assert.equal(oracleReport.lightOverrideCount, 57);
    assert.equal(oracleReport.canonicalTokenMapCount, 25);
    assert.equal(oracleReport.sereinMidnightId, "serein-midnight");

    // Rust-тест оракулов
    const rustOracle = runCompiledRustTests(
      testBinPath,
      "test_sub_985_1_builtin_palettes_and_picker_tokens",
    );
    assert.equal(rustOracle.status, 0, rustOracle.stderr || rustOracle.stdout);
  });

  it("SUB-985-2: переключает темы (Dark, Light, Midnight, AMOLED и пользовательские) на лету без перезагрузки интерфейса и управляет состояниями theme-preview", async () => {
    const {
      createCustomThemeController,
      importSereinThemeExtension,
    } = await import("../../rust/gui-spike/ui/features/custom-themes.js");

    const mockDoc = createMockDomDocument();
    /** @type {Map<string, string>} */
    const backingStore = new Map();
    const storage = {
      getItem(/** @type {string} */ k) {
        return backingStore.get(k) ?? null;
      },
      setItem(/** @type {string} */ k, /** @type {string} */ v) {
        backingStore.set(k, v);
      },
    };

    const controller = createCustomThemeController(mockDoc, { storage });

    // Начальное состояние: default = "dark"
    const initial = controller.getState();
    assert.equal(initial.activeThemeId, "dark");
    assert.equal(initial.previewState, "dark");
    assert.equal(mockDoc.documentElement.getAttribute("data-theme"), "dark");
    assert.equal(mockDoc.getReloadCount(), 0);

    // Наведение на карточку темы в Theme Picker -> состояние "highlight theme"
    const hoverRes = controller.hoverThemePreview("midnight");
    assert.equal(hoverRes.ok, true);
    assert.equal(hoverRes.previewState, "highlight theme");
    assert.equal(hoverRes.highlightedThemeId, "midnight");
    assert.equal(mockDoc.getReloadCount(), 0);

    // Горячее переключение на Midnight, Light, AMOLED без перезагрузки
    for (const targetTheme of ["midnight", "light", "amoled", "dark"]) {
      const switchRes = controller.applyTheme(targetTheme);
      assert.equal(switchRes.ok, true, `Переключение на ${targetTheme} должно быть успешным`);
      assert.equal(switchRes.reloaded, false, "Переключение не должно перезагружать интерфейс");
      assert.equal(switchRes.previewState, "applied");
      assert.equal(switchRes.transitionLatencyMs < 8.33, true, "Вписывается в бюджет 120 FPS");
      assert.equal(mockDoc.getReloadCount(), 0, "location.reload() не должен вызываться");
      assert.equal(mockDoc.documentElement.getAttribute("data-theme"), targetTheme);
      assert.ok(
        mockDoc.documentElement.style.getPropertyValue("--bg-app").length > 0,
        "Токен --bg-app должен быть инжектирован в documentElement.style",
      );
      const styleEl = mockDoc.getElementById("discord-custom-theme-palette");
      assert.ok(styleEl, "Инжектированный тег <style id='discord-custom-theme-palette'> должен присутствовать");
      assert.ok(styleEl.textContent.includes("--bg-app:"));
    }

    // Импорт эталонной темы Serein Midnight (.serein-extension) и горячее применение
    const sereinRaw = readFileSync(SEREIN_MIDNIGHT_PATH, "utf8");
    const imported = importSereinThemeExtension(sereinRaw, "dark");
    assert.equal(imported.ok, true);
    assert.equal(imported.theme?.id, "serein-midnight");

    const regRes = controller.registerCustomTheme(imported.theme);
    assert.equal(regRes.ok, true);

    const applyCustom = controller.applyTheme("serein-midnight");
    assert.equal(applyCustom.ok, true);
    assert.equal(applyCustom.reloaded, false);
    assert.equal(mockDoc.getReloadCount(), 0);
    assert.equal(
      mockDoc.documentElement.getAttribute("data-custom-theme"),
      "serein-midnight",
    );
    assert.equal(
      mockDoc.documentElement.style.getPropertyValue("--bg-app"),
      "#11131f",
    );

    // Аварийный хоткей Ctrl+Shift+F12 мгновенно сбрасывает тему в "dark" без перезагрузки
    const emergency = controller.handleKeyDown({
      ctrlKey: true,
      shiftKey: true,
      key: "F12",
    });
    assert.equal(emergency.handled, true);
    assert.equal(emergency.activeThemeId, "dark");
    assert.equal(mockDoc.documentElement.getAttribute("data-theme"), "dark");
    assert.equal(mockDoc.getReloadCount(), 0);

    // Rust-тест горячего переключения без перезагрузки
    const rustHotSwitch = runCompiledRustTests(
      testBinPath,
      "test_sub_985_2_hot_theme_switch_without_reload",
    );
    assert.equal(rustHotSwitch.status, 0, rustHotSwitch.stderr || rustHotSwitch.stdout);
  });

  it("SUB-985-3: валидирует CSS-токены на WCAG 2.2 AA контрастность (>= 4.5:1 текст, >= 3.0:1 UI/focus) и блокирует CSS-инъекции", async () => {
    const {
      BUILTIN_THEME_PALETTES,
      parseCssColorValue,
      computeRelativeLuminance,
      computeContrastRatio,
      validateThemeWcagContrast,
      sanitizeCustomThemeTokens,
    } = await import("../../rust/gui-spike/ui/features/custom-themes.js");

    // 1. Проверка эталонной формулы яркости и контраста WCAG 2.2
    const white = parseCssColorValue("#ffffff");
    const black = parseCssColorValue("#000000");
    assert.ok(white && black);
    assert.ok(Math.abs(computeRelativeLuminance(white) - 1.0) < 1e-6);
    assert.ok(Math.abs(computeRelativeLuminance(black) - 0.0) < 1e-6);
    assert.ok(Math.abs(computeContrastRatio(white, black) - 21.0) < 0.05);

    // 2. Все 4 встроенные темы (Dark, Light, Midnight, AMOLED) проходят WCAG 2.2 AA аудит (0 нарушений)
    for (const [themeId, palette] of Object.entries(BUILTIN_THEME_PALETTES)) {
      const report = validateThemeWcagContrast(palette.tokens);
      assert.equal(
        report.passed,
        true,
        `Встроенная тема ${themeId} должна проходить WCAG 2.2 AA: ${report.failedPairs.join(", ")}`,
      );
      assert.ok(report.auditedPairs >= 24);
      assert.equal(report.failedPairs.length, 0);
      assert.ok(report.minTextContrast >= 4.5);
      assert.ok(report.minNonTextContrast >= 3.0);
    }

    // 3. Пользовательская тема с низким контрастом текста (#383a40 на #313338) отклоняется валидатором WCAG 2.2 AA
    const lowContrastReport = validateThemeWcagContrast({
      ...BUILTIN_THEME_PALETTES.dark.tokens,
      "--text": "#383a40",
      "--text-muted": "#2e3035",
      "--focus-ring": "#25262a",
    });
    assert.equal(lowContrastReport.passed, false);
    assert.ok(lowContrastReport.failedPairs.length >= 3);

    // 4. Санитизация CSS-токенов блокирует инъекции url(), @import, @font-face, expression(), !important
    const malicious = sanitizeCustomThemeTokens({
      "--bg-app": "#121214",
      "--bg-chat": "url(https://evil.example/track.png)",
      "--surface-1": "expression(alert(1))",
      "--surface-2": "#242429 !important",
      "--text": "javascript:alert(1)",
      "--accent": "@import url('https://evil.example/x.css')",
      "body": "#ffffff",
    });
    assert.equal(malicious.valid, false);
    assert.equal(malicious.sanitizedTokens["--bg-app"], "#121214");
    assert.equal(malicious.rejectedTokens.length, 6);

    // Rust-тест WCAG 2.2 AA и CSS-санитизации
    const rustWcag = runCompiledRustTests(
      testBinPath,
      "test_sub_985_3_wcag_aa_contrast_and_css_sanitizer",
    );
    assert.equal(rustWcag.status, 0, rustWcag.stderr || rustWcag.stdout);
  });

  it("SUB-985-4: покрывает все 9 измерений TEST-POLICY.md (граничные 4.49 vs 4.50, лимит 8 тем, Snowflake, 2000 символов, ошибки 401..5xx, skeleton/202, права, fallback и маскирование)", async () => {
    const {
      evaluateCustomThemesEdgeDimensions,
      redactThemeTelemetryLogs,
    } = await import("../../rust/gui-spike/ui/features/custom-themes.js");

    const dimReport = evaluateCustomThemesEdgeDimensions();
    assert.equal(dimReport.passed, true, `Сбой в 9 измерениях: ${dimReport.failures.join("; ")}`);
    assert.equal(dimReport.dimensionsChecked, 9);
    assert.equal(dimReport.contrastBoundary449Rejected, true);
    assert.equal(dimReport.contrastBoundary450Accepted, true);
    assert.equal(dimReport.nonTextBoundary299Rejected, true);
    assert.equal(dimReport.nonTextBoundary300Accepted, true);
    assert.equal(dimReport.maxThemesCapEnforced, true);
    assert.equal(dimReport.snowflakeBoundsChecked, true);
    assert.equal(dimReport.httpErrorsHandled, true);
    assert.equal(dimReport.skeleton202Handled, true);
    assert.equal(dimReport.capabilityGateChecked, true);
    assert.equal(dimReport.emergencyRecoveryChecked, true);

    const rawLog =
      'ThemeSync Authorization: Bearer mfa.secret_token_1234567890abcd token="MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz0123456789abcdef"';
    const redacted = redactThemeTelemetryLogs(rawLog);
    assert.doesNotMatch(redacted, /mfa\.secret_token/);
    assert.doesNotMatch(redacted, /MTIzNDU2Nzg5MDEyMzQ1Njc4/);
    assert.match(redacted, /\[REDACTED_TOKEN\]/);

    // Rust-тест всех 9 измерений
    const rustDimensions = runCompiledRustTests(
      testBinPath,
      "test_sub_985_4_all_nine_dimensions_contract",
    );
    assert.equal(rustDimensions.status, 0, rustDimensions.stderr || rustDimensions.stdout);
  });

  it("SUB-985-5: убивает 5/5 контрольных мутаций в custom_themes.rs и custom-themes.js (100% mutation kill rate)", async () => {
    const { runCustomThemesMutationSuite } = await import(
      "../../rust/gui-spike/ui/features/custom-themes.js"
    );

    // 1. Мутационный гейт JS-модуля (5/5)
    const jsMutReport = runCustomThemesMutationSuite();
    assert.equal(jsMutReport.total, 5);
    assert.equal(jsMutReport.killed, 5);
    assert.deepEqual(jsMutReport.survivors, []);

    // 2. Мутационный гейт Rust-модуля (5/5 через реальную перекомпиляцию rustc --test)
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const rustMutations = [
      {
        id: "MUT-985-RS-01",
        moduleName: "custom_themes",
        operator: "WcagNormalThresholdLowering",
        from: "pub const WCAG_AA_NORMAL_MIN_RATIO: f64 = 4.5;",
        to: "pub const WCAG_AA_NORMAL_MIN_RATIO: f64 = 1.5;",
      },
      {
        id: "MUT-985-RS-02",
        moduleName: "custom_themes",
        operator: "HotSwitchReloadFlagTamper",
        from: "reloaded: false,",
        to: "reloaded: true,",
      },
      {
        id: "MUT-985-RS-03",
        moduleName: "custom_themes",
        operator: "AmoledTrueBlackTamper",
        from: "(\"--bg-app\", \"#000000\")",
        to: "(\"--bg-app\", \"#2b2d31\")",
      },
      {
        id: "MUT-985-RS-04",
        moduleName: "custom_themes",
        operator: "CssInjectionSanitizerBypass",
        from: "if lower.contains(\"url(\")",
        to: "if lower.contains(\"__never_match_url__\")",
      },
      {
        id: "MUT-985-RS-05",
        moduleName: "custom_themes",
        operator: "MaxCustomThemesLimitTamper",
        from: "pub const MAX_INSTALLED_CUSTOM_THEMES: usize = 8;",
        to: "pub const MAX_INSTALLED_CUSTOM_THEMES: usize = 99;",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в custom_themes.rs`,
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

    assert.equal(gateResult.passed, true, `Выжившие мутанты: ${gateResult.survivors.join(", ")}`);
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
  });
});
