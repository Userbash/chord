// @ts-check
/**
 * Контрактные и модульные тесты автозапуска с системой (XDG Autostart & Start Minimized — TASK-1015):
 * - SUB-1015-0: Статический и компиляционный контракт `rust/discord-gpui/src/platform/autostart.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1015-1: Сверка карточек `TASK-1015`, `TASK-1015A`, `TASK-1015B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Autostart Service`
 *   (`CLI & Config`, `padding: 0px`, `spacing: 0px`), `Autostart Config` (`Desktop Entry`), `Headless Init`
 *   (`RAM < 8MB`), интерактивных состояний `autostart-toggle` (`off` -> `highlight toggle` -> `enabled`),
 *   `autostart-file` (`absent` -> `writing` -> `registered`), `tray-icon` (`idle in tray` -> `tooltip` -> `window mapped`),
 *   оракула `docs/feature-backlog.md` (`R4-04 Автозапуск`) и IPC-каналов RE-дампа `/home/sanya/re-deep/backend-ipc.json`
 *   (`DISCORD_APP_GET_OPEN_ON_START`, `DISCORD_TOGGLE_OPEN_ON_STARTUP`, `DISCORD_TOGGLE_START_MINIMIZED`,
 *   `DISCORD_TOGGLE_MINIMIZE_TO_TRAY`, `DISCORD_UPDATE_OPEN_ON_STARTUP`).
 * - SUB-1015-2: Генератор и жизненный цикл XDG Autostart `.desktop`-файла в `~/.config/autostart/discord-gpui.desktop`
 *   (формат FreeDesktop Desktop Entry 1.5, экранирование `Exec` с ключом `--start-minimized`, `X-GNOME-Autostart-enabled`,
 *   валидация прав `0644`, атомарная запись, обновление при смене `START_MINIMIZED` и удаление при отключении).
 * - SUB-1015-3: Разбор флагов CLI (`--start-minimized`, `--start-inactive`) и старт свёрнутым в системный трей
 *   без маппинга окна WGPU (`wgpu_window_mapped = false`, `skip_taskbar = true`, бюджет `VmRSS < 8 МБ`)
 *   до первого клика по иконке трея или сигнала `second-instance`, а также сворачивание в трей при закрытии окна.
 * - SUB-1015-4: Диспетчеризация IPC-каналов `core/#4973` и покрытие всех 9 обязательных измерений `TEST-POLICY.md`
 *   (позитивные, негативные, граничные Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`,
 *   `offset=9975`, громкость `0..200%`, бейдж `99+`, лимит ОЗУ `8192 КБ`, права `0644`, коды `400..5xx`,
 *   пустые состояния, `skeleton`/`HTTP 202`, `AGE_GATED_SPACES`, fallback с XDG Portal на `.desktop` и маскирование секретов).
 * - SUB-1015-5: Мутационный гейт `5/5` (`100%` kill rate) для `autostart.rs` по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/discord-gpui/src/platform/autostart.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FEATURE_BACKLOG_PATH = join(ROOT, "docs", "feature-backlog.md");
const BACKEND_IPC_DOC_PATH = join(ROOT, "docs", "re", "backend-ipc.md");
const RE_BACKEND_IPC_JSON_PATH = "/home/sanya/re-deep/backend-ipc.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/platform/autostart.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1015: Автозапуск с системой (XDG Autostart & Start Minimized — rust/discord-gpui/src/platform/autostart.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "autostart-xdg-1015-"));
    testBinPath = join(workDir, "autostart_xdg_1015_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1015-0: модуль rust/discord-gpui/src/platform/autostart.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} обязан существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "Модуль обязан объявлять #![forbid(unsafe_code)]",
    );

    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде autostart.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines > 200 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 200..800 строк, получено: ${sizeCheck.fileLines}`,
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

    const secretReport = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(
      secretReport.length,
      0,
      `Обнаружены секреты в исходнике: ${JSON.stringify(secretReport)}`,
    );

    const compiled = compileRustModuleTests(MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc:\n${compiled.stderr}\n${compiled.stdout}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );
  });

  it("SUB-1015-1: сверяет карточки TASK-1015, TASK-1015A, TASK-1015B, дизайн-токены (#1e1f22, #2b2d31, #ffffff, #5865f2, #3f4147), интерактивные состояния, оракул R4-04 и IPC-каналы RE-дампа", () => {
    assert.ok(existsSync(KANBAN_BOARD_PATH), "Файл kanban-board.json обязан существовать");
    const kanban = JSON.parse(readFileSync(KANBAN_BOARD_PATH, "utf8"));
    const cards = kanban.cards ?? [];

    const card1015 = cards.find(/** @param {{ id: string }} c */ (c) => c.id === "TASK-1015");
    assert.ok(card1015, "Карточка TASK-1015 обязана присутствовать в kanban-board.json");
    assert.equal(card1015.priority, "P2");
    assert.equal(card1015.department, "DevOps / Integration");
    assert.equal(card1015.layout.container, "Autostart Service");
    assert.equal(card1015.layout.dimensions, "CLI & Config");
    assert.equal(card1015.layout.padding, "0px");
    assert.equal(card1015.layout.spacing, "0px");
    assert.equal(card1015.design_tokens.background, "#1e1f22");
    assert.equal(card1015.design_tokens.surface, "#2b2d31");
    assert.equal(card1015.design_tokens.text_primary, "#ffffff");
    assert.equal(card1015.design_tokens.accent, "#5865f2");
    assert.equal(card1015.design_tokens.border, "#3f4147");

    const toggleState = card1015.interactive_states?.find(
      /** @param {{ element: string }} s */ (s) => s.element === "autostart-toggle",
    );
    assert.ok(toggleState, "Интерактивное состояние autostart-toggle обязано присутствовать");
    assert.equal(toggleState.default, "off");
    assert.equal(toggleState.hover, "highlight toggle");
    assert.equal(toggleState.active, "enabled");

    const card1015a = cards.find(/** @param {{ id: string }} c */ (c) => c.id === "TASK-1015A");
    assert.ok(card1015a, "Карточка TASK-1015A обязана присутствовать");
    assert.equal(card1015a.layout.container, "Autostart Config");
    assert.equal(card1015a.layout.dimensions, "Desktop Entry");
    assert.equal(card1015a.interactive_states?.[0]?.element, "autostart-file");
    assert.equal(card1015a.interactive_states?.[0]?.default, "absent");
    assert.equal(card1015a.interactive_states?.[0]?.hover, "writing");
    assert.equal(card1015a.interactive_states?.[0]?.active, "registered");

    const card1015b = cards.find(/** @param {{ id: string }} c */ (c) => c.id === "TASK-1015B");
    assert.ok(card1015b, "Карточка TASK-1015B обязана присутствовать");
    assert.equal(card1015b.layout.container, "Headless Init");
    assert.equal(card1015b.layout.dimensions, "RAM < 8MB");
    assert.equal(card1015b.interactive_states?.[0]?.element, "tray-icon");
    assert.equal(card1015b.interactive_states?.[0]?.default, "idle in tray");
    assert.equal(card1015b.interactive_states?.[0]?.hover, "tooltip");
    assert.equal(card1015b.interactive_states?.[0]?.active, "window mapped");

    assert.ok(existsSync(FEATURE_BACKLOG_PATH), "Оракул docs/feature-backlog.md обязан существовать");
    const backlog = readFileSync(FEATURE_BACKLOG_PATH, "utf8");
    assert.match(
      backlog,
      /R4-04.*Автозапуск.*--start-minimized.*autoStart\.ts.*shell: autostart/,
      "Оракул R4-04 Автозапуск обязан присутствовать в docs/feature-backlog.md",
    );

    assert.ok(existsSync(BACKEND_IPC_DOC_PATH), "Документ docs/re/backend-ipc.md обязан существовать");
    const ipcDoc = readFileSync(BACKEND_IPC_DOC_PATH, "utf8");
    for (const ch of [
      "APP_GET_OPEN_ON_START",
      "TOGGLE_OPEN_ON_STARTUP",
      "TOGGLE_START_MINIMIZED",
      "TOGGLE_MINIMIZE_TO_TRAY",
      "UPDATE_OPEN_ON_STARTUP",
    ]) {
      assert.ok(ipcDoc.includes(ch), `Канал ${ch} обязан быть зафиксирован в docs/re/backend-ipc.md`);
    }

    if (existsSync(RE_BACKEND_IPC_JSON_PATH)) {
      const reIpcRaw = readFileSync(RE_BACKEND_IPC_JSON_PATH, "utf8");
      assert.ok(reIpcRaw.includes("DISCORD_TOGGLE_OPEN_ON_STARTUP"));
      assert.ok(reIpcRaw.includes("DISCORD_TOGGLE_START_MINIMIZED"));
    }

    const source = readFileSync(MODULE_PATH, "utf8");
    const requiredTokens = [
      "AUTOSTART_SERVICE_CONTAINER",
      "AUTOSTART_SERVICE_DIMENSIONS",
      "AUTOSTART_CONFIG_CONTAINER",
      "AUTOSTART_CONFIG_DIMENSIONS",
      "HEADLESS_INIT_CONTAINER",
      "HEADLESS_INIT_DIMENSIONS",
      "TOKEN_BG",
      "TOKEN_SURFACE",
      "TOKEN_TEXT_PRIMARY",
      "TOKEN_ACCENT",
      "TOKEN_BORDER",
      "DESKTOP_FILE_NAME",
      "CLI_FLAG_START_MINIMIZED",
      "CLI_FLAG_START_INACTIVE",
      "DESKTOP_FILE_MODE",
      "MINIMIZED_RAM_LIMIT_KB",
      "IPC_APP_GET_OPEN_ON_START",
      "IPC_TOGGLE_OPEN_ON_STARTUP",
      "IPC_TOGGLE_START_MINIMIZED",
      "IPC_TOGGLE_MINIMIZE_TO_TRAY",
      "IPC_UPDATE_OPEN_ON_STARTUP",
    ];
    for (const token of requiredTokens) {
      assert.ok(
        source.includes(token),
        `Константа ${token} обязана быть объявлена в autostart.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1015_1_oracle_and_tokens");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1015_1_oracle_and_tokens завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1015_1_oracle_and_tokens .* ok/);
  });

  it("SUB-1015-2: генерирует валидный XDG Autostart desktop-файл в ~/.config/autostart/discord-gpui.desktop с флагом --start-minimized, правами 0644, обновлением и удалением при отключении", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1015_2_xdg_desktop_file_lifecycle");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1015_2_xdg_desktop_file_lifecycle завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1015_2_xdg_desktop_file_lifecycle .* ok/);
  });

  it("SUB-1015-3: обрабатывает флаг --start-minimized и запускает клиент свёрнутым в системный трей без открытия окна WGPU (RAM < 8 МБ) до первого клика по трею", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1015_3_minimized_tray_launch");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1015_3_minimized_tray_launch завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1015_3_minimized_tray_launch .* ok/);
  });

  it("SUB-1015-4: обрабатывает IPC-каналы core/#4973 и покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400..5xx, skeleton, права 0644, fallback Portal и маскирование секретов)", () => {
    const run = runCompiledRustTests(testBinPath, "test_sub_1015_4_ipc_and_nine_dimensions");
    assert.equal(
      run.status,
      0,
      `Тест test_sub_1015_4_ipc_and_nine_dimensions завершился с ошибкой:\n${run.stderr}\n${run.stdout}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1015_4_ipc_and_nine_dimensions .* ok/);
  });

  it("SUB-1015-5: убивает 5/5 контрольных мутаций модуля autostart.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(source).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-autostart-01",
        operator: "StartMinimizedExecFlagOmitted",
        from: "if self.start_minimized {",
        to: "if false && self.start_minimized {",
      },
      {
        id: "MUT-autostart-02",
        operator: "GnomeAutostartEnabledInverted",
        from: 'let enabled_str = if self.enabled { "true" } else { "false" };',
        to: 'let enabled_str = if self.enabled { "false" } else { "true" };',
      },
      {
        id: "MUT-autostart-03",
        operator: "MinimizedLaunchSkipsWgpuInverted",
        from: "let wgpu_window_mapped = !minimized;",
        to: "let wgpu_window_mapped = minimized;",
      },
      {
        id: "MUT-autostart-04",
        operator: "TrayClickWindowMappingBypassed",
        from: "self.wgpu_window_mapped = true;",
        to: "self.wgpu_window_mapped = false;",
      },
      {
        id: "MUT-autostart-05",
        operator: "UninstallFileRemovalBypassed",
        from: "fs::remove_file(&path).map_err(|_| AutostartError::IoError(\"remove_file\"))?;",
        to: "let _ = &path;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "autostart.rs",
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
    assert.equal(afterMd5, originalMd5, "MD5 исходного файла autostart.rs не должен измениться");

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
