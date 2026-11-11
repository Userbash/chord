// @ts-check
/**
 * Контрактные и модульные тесты управления клиентом с геймпада через gilrs и экранной клавиатуры SteamOS (TASK-1014):
 * - SUB-1014-0: Статический и компиляционный контракт `rust/discord-gpui/src/input/gamepad.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1014-1: Сверка карточек `TASK-1014`, `TASK-1014A`, `TASK-1014B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Gamepad Navigation HUD`
 *   (`1280x800 handheld`, `padding: 16px`, `spacing: 12px`), интерактивных состояний `focused-item`
 *   (`outline-accent` -> `pulse focus` -> `activated`), оракула `docs/feature-backlog.md` (`R6-01 SteamOS`),
 *   и оракулов RE-дампа `/home/sanya/re-deep/` (захват Gamescope `capture_gamescope`, Steam Audio).
 * - SUB-1014-2: Драйвер ввода геймпада на базе абстракции `gilrs`: фильтрация осей и кнопок, мёртвая зона стика
 *   (deadzone rejection `0.15`), маппинг крестовины D-pad (`NavUp/Down/Left/Right`), бамперов L1/R1 (`Prev/NextServer`),
 *   триггеров L2/R2 (`Prev/NextChannel`), кнопок действия A (`Activate`), B (`Cancel`), X (`ToggleKeyboard`),
 *   Y (`Search`), и защита от зависших событий при отключении контроллера (`Disconnected`).
 * - SUB-1014-3: Фокусная сетка интерфейса GPUI: 4 зоны навигации (`ServerRail`, `ChannelSidebar`, `ChatTimeline`,
 *   `MessageInputBar`), перемещение D-pad Left/Right между зонами, D-pad Up/Down внутри зоны со скроллом,
 *   яркая неоновая рамка фокуса `#5865f2` (2px), и автоскролл списков каналов и чата.
 * - SUB-1014-4: Интеграция экранной клавиатуры SteamOS через D-Bus сервис `org.kde.kwin.VirtualKeyboard`
 *   (методы `show`, `hide`, `toggle`), хоткей `Steam + X`, URL `steam://open/keyboard`, авто-подъём клавиатуры
 *   при активации поля ввода сообщения, и graceful fallback при недоступности D-Bus сессии.
 * - SUB-1014-5: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, стики `-1.0..1.0`, deadzone `0.01..0.5`, 2000 символов,
 *   10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`, коды `400/401/403/404/413/429/5xx`,
 *   пустые списки, `skeleton`/`HTTP 202`, права `PERM_VIEW_CHANNEL` / `PERM_SEND_MESSAGES` / `AGE_GATED_SPACES`,
 *   восстановление при переподключении геймпада и маскирование секретов в телеметрии ввода).
 * - SUB-1014-6: Мутационный гейт `5/5` (`100%` kill rate) для `gamepad.rs` по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/discord-gpui/src/input/gamepad.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FEATURE_BACKLOG_PATH = join(ROOT, "docs", "feature-backlog.md");
const RE_NATIVE_MODULES_PATH = "/home/sanya/re-deep/native-modules.json";
const RE_NATIVE_RE_PATH = "/home/sanya/re-deep/native-re.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/input/gamepad.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1014: Управление клиентом с геймпада через gilrs и клавиатура SteamOS (rust/discord-gpui/src/input/gamepad.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "gamepad-1014-"));
    testBinPath = join(workDir, "gamepad_1014_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1014-0: модуль rust/discord-gpui/src/input/gamepad.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} обязан существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /#!\[forbid\(unsafe_code\)\]/,
      "Модуль обязан объявлять #![forbid(unsafe_code)]",
    );

    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде gamepad.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines > 150 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 150..800 строк, получено: ${sizeCheck.fileLines}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `Форматирование rustfmt не прошло:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const secretFindings = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены секреты в исходнике: ${JSON.stringify(secretFindings)}`,
    );

    const compiled = compileRustModuleTests(MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc:\n${compiled.stderr}\n${compiled.stdout}`,
    );
  });

  it("SUB-1014-1: сверяет карточки TASK-1014, TASK-1014A, TASK-1014B, дизайн-токены (#1e1f22, #2b2d31, #ffffff, #5865f2, #3f4147), геометрию HUD (1280x800 handheld, padding 16px, spacing 12px) и оракулы RE", () => {
    assert.ok(existsSync(KANBAN_BOARD_PATH), "Файл kanban-board.json обязан существовать");
    const kanban = JSON.parse(readFileSync(KANBAN_BOARD_PATH, "utf8"));
    const cards = kanban.cards ?? [];

    const card1014 = cards.find(/** @param {{ id: string }} c */ (c) => c.id === "TASK-1014");
    assert.ok(card1014, "Карточка TASK-1014 обязана присутствовать в kanban-board.json");
    assert.equal(card1014.priority, "P2");
    assert.equal(card1014.layout.container, "Gamepad Navigation HUD");
    assert.equal(card1014.layout.dimensions, "1280x800 handheld");
    assert.equal(card1014.layout.padding, "16px");
    assert.equal(card1014.layout.spacing, "12px");
    assert.equal(card1014.design_tokens.background, "#1e1f22");
    assert.equal(card1014.design_tokens.surface, "#2b2d31");
    assert.equal(card1014.design_tokens.text_primary, "#ffffff");
    assert.equal(card1014.design_tokens.accent, "#5865f2");
    assert.equal(card1014.design_tokens.border, "#5865f2");

    const focusedItemState = card1014.interactive_states?.find(
      /** @param {{ element: string }} s */ (s) => s.element === "focused-item",
    );
    assert.ok(focusedItemState, "Интерактивное состояние focused-item обязано присутствовать");
    assert.equal(focusedItemState.default, "outline-accent");
    assert.equal(focusedItemState.hover, "pulse focus");
    assert.equal(focusedItemState.active, "activated");

    const card1014a = cards.find(/** @param {{ id: string }} c */ (c) => c.id === "TASK-1014A");
    assert.ok(card1014a, "Карточка TASK-1014A обязана присутствовать");
    assert.equal(card1014a.layout.container, "Gamepad Driver");
    assert.equal(card1014a.design_tokens.border, "#3f4147");

    const card1014b = cards.find(/** @param {{ id: string }} c */ (c) => c.id === "TASK-1014B");
    assert.ok(card1014b, "Карточка TASK-1014B обязана присутствовать");
    assert.equal(card1014b.layout.container, "Handheld Focus Grid");
    assert.equal(card1014b.layout.dimensions, "1280x800");

    assert.ok(existsSync(FEATURE_BACKLOG_PATH), "Файл feature-backlog.md обязан существовать");
    const backlog = readFileSync(FEATURE_BACKLOG_PATH, "utf8");
    assert.match(
      backlog,
      /R6-01.*Steam Deck.*Game Mode.*steamOS\.ts/,
      "Оракул R6-01 Steam Deck обязан быть зафиксирован в docs/feature-backlog.md",
    );

    if (existsSync(RE_NATIVE_MODULES_PATH)) {
      const rawNat = readFileSync(RE_NATIVE_MODULES_PATH, "utf8");
      assert.ok(
        rawNat.includes("capture_gamescope"),
        "Символ capture_gamescope обязан быть зафиксирован в native-modules.json",
      );
    }

    if (existsSync(RE_NATIVE_RE_PATH)) {
      const rawRe = readFileSync(RE_NATIVE_RE_PATH, "utf8");
      assert.ok(
        rawRe.includes("steam_audio"),
        "Пути steam_audio обязаны присутствовать в native-re.json",
      );
    }

    const source = readFileSync(MODULE_PATH, "utf8");
    const requiredTokens = [
      "TOKEN_BG",
      "TOKEN_SURFACE",
      "TOKEN_TEXT_PRIMARY",
      "TOKEN_ACCENT",
      "TOKEN_BORDER",
      "GAMEPAD_CONTAINER",
      "GAMEPAD_DIMENSIONS",
      "GAMEPAD_PADDING",
      "GAMEPAD_SPACING",
      "DBUS_KWIN_SERVICE",
      "DBUS_KWIN_PATH",
      "DBUS_KWIN_INTERFACE",
      "STEAM_SHORTCUT_VIRTUAL_KEYBOARD",
      "STEAM_URL_KEYBOARD",
      "DEFAULT_STICK_DEADZONE",
      "FOCUS_OUTLINE_WIDTH_PX",
      "FOCUS_OUTLINE_COLOR",
    ];
    for (const token of requiredTokens) {
      assert.ok(
        source.includes(token),
        `Константа ${token} обязана быть объявлена в gamepad.rs`,
      );
    }
  });

  it("SUB-1014-2: драйвер ввода геймпада фильтрует оси и мёртвую зону (0.15), маппит D-pad (NavUp/Down/Left/Right), бамперы L1/R1 (серверы), триггеры L2/R2 (каналы), кнопки A/B/X/Y и обрабатывает отключение контроллера", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1014_2_gamepad_input_driver");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1014_2_gamepad_input_driver завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1014_2_gamepad_input_driver .* ok/);
  });

  it("SUB-1014-3: фокусная сетка интерфейса GPUI управляет 4 зонами (ServerRail, ChannelSidebar, ChatTimeline, MessageInputBar), D-pad навигацией, активной подсветкой #5865f2 и авто-скроллом", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1014_3_focus_grid_navigation");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1014_3_focus_grid_navigation завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1014_3_focus_grid_navigation .* ok/);
  });

  it("SUB-1014-4: интеграция экранной клавиатуры SteamOS формирует D-Bus вызов org.kde.kwin.VirtualKeyboard (show/hide/toggle), хоткей Steam + X, steam://open/keyboard и авто-подъём при фокусе поля ввода", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1014_4_steamos_virtual_keyboard");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1014_4_steamos_virtual_keyboard завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1014_4_steamos_virtual_keyboard .* ok/);
  });

  it("SUB-1014-5: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, стики -1.0..1.0, deadzone 0.01..0.5, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400..5xx, skeleton, права и маскирование секретов)", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1014_5_nine_dimensions");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1014_5_nine_dimensions завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1014_5_nine_dimensions .* ok/);
  });

  it("SUB-1014-6: убивает 5/5 контрольных мутаций модуля gamepad.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-gamepad-01",
        operator: "StickDeadzoneRejectionBypassed",
        from: "if val.abs() < self.deadzone {",
        to: "if false && val.abs() < self.deadzone {",
      },
      {
        id: "MUT-gamepad-02",
        operator: "DPadLeftRightZoneTransitionReversed",
        from: "FocusZone::ServerRail => FocusZone::ChannelSidebar,",
        to: "FocusZone::ServerRail => FocusZone::ServerRail,",
      },
      {
        id: "MUT-gamepad-03",
        operator: "NextServerBumperNavigationDisabled",
        from: "self.server_index = (self.server_index + 1) % count;",
        to: "self.server_index = self.server_index % count;",
      },
      {
        id: "MUT-gamepad-04",
        operator: "InputBarActivationVirtualKeyboardBypassed",
        from: "let show_keyboard = current_zone == FocusZone::MessageInputBar;",
        to: "let show_keyboard = false;",
      },
      {
        id: "MUT-gamepad-05",
        operator: "DisconnectedGamepadStateCleanupBypassed",
        from: "self.active_buttons.remove(&id);",
        to: "/* active_buttons not cleaned */",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "gamepad.rs",
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
