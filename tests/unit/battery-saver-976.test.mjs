// @ts-check
/**
 * Контрактные и модульные тесты модуля снижения энергопотребления и Battery Saver Mode (TASK-976):
 * - SUB-976-0: Статический контракт `rust/chord-gpui/src/battery_saver.rs`
 *   (`#![forbid(unsafe_code)]`, соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк,
 *   модуль < 800 строк, 0 `.unwrap()`/`.expect()` в prod-коде, чистый `rustfmt --check --edition 2024`
 *   и компиляция `rustc --test --edition 2024 -D warnings`).
 * - SUB-976-1: Сверка с независимыми оракулами (`waves/kanban-board.json` карточка `TASK-976`,
 *   `docs/re/backend-ipc.json` 8 каналов `DISCORD_POWER_MONITOR_*` и `DISCORD_POWER_SAVE_BLOCKER_*`,
 *   `docs/re/main-arch.md` §10.1 `#4531`/`#7420`, `fixtures/dbus/power.json` `login1`/`ScreenSaver`),
 *   парсинг Linux `/sys/class/power_supply` и переходы `power-toggle` (`ac-mode` -> `check battery` -> `saver-active`).
 * - SUB-976-2: Управление частотой кадров (`FrameRateGovernor`): 120 FPS на сети (AC) -> строго 30 FPS
 *   при работе от батареи или неактивности (`33_333` мкс бюджет кадра) -> 0 FPS при `suspend`/`lock-screen`,
 *   управление ингибиторами гашения экрана (`powerSaveBlockerIds`).
 * - SUB-976-3: Коалесценция таймеров (`TimerCoalescer`) и профилировщик C-States:
 *   удержание таймерных пробуждений строго `< 15 wakeups/s` (`MAX_IDLE_WAKEUPS_PER_SEC = 15.0`)
 *   для глубоких состояний сна процессора (`C6..C10 >= 85%`), пропуск пустых кадров при `dirty_regions == 0`.
 * - SUB-976-4: Покрытие всех 9 измерений `TEST-POLICY.md` (позитивные, негативные, граничные
 *   Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, граница `14.99` vs `15.00 wakeups/s`, ошибки `401/403/404/413/429/5xx`, пустые,
 *   `skeleton`/`HTTP 202`, права `VIEW_CHANNEL`/`AGE_GATED_SPACES`, fallback Wayland -> X11/D-Bus idle,
 *   маскирование токенов в JSON-телеметрии).
 * - SUB-976-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const MODULE_REL_PATH = "rust/chord-gpui/src/battery_saver.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const BACKEND_IPC_JSON_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const MAIN_ARCH_MD_PATH = join(ROOT, "docs", "re", "main-arch.md");
const DBUS_POWER_FIXTURE_PATH = join(ROOT, "fixtures", "dbus", "power.json");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `battery_saver.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-976: Модуль снижения энергопотребления и Battery Saver Mode (rust/chord-gpui/src/battery_saver.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "battery-saver-976-"));
    testBinPath = join(workDir, "battery_saver_976_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-976-0: модуль rust/chord-gpui/src/battery_saver.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл) и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /#!\[forbid\(unsafe_code\)\]/,
      "Модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде battery_saver.rs запрещены .unwrap() и .expect()",
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
  });

  it("SUB-976-1: сверяет константы питания, 8 IPC-каналов DISCORD_POWER_*, D-Bus login1/ScreenSaver и дизайн-токены с независимыми оракулами репозитория", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-976 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card976 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-976");
    assert.ok(card976, "Карточка TASK-976 должна существовать в waves/kanban-board.json");
    assert.equal(card976.layout.container, "Power Profiler");
    assert.equal(card976.layout.dimensions, "Battery Saver");

    // 2. Независимый оракул: 8 IPC-каналов питания в docs/re/backend-ipc.json и docs/re/main-arch.md
    const ipcRaw = readFileSync(BACKEND_IPC_JSON_PATH, "utf8");
    const ipcData = JSON.parse(ipcRaw);
    const ipcChannelNames = new Set(
      (ipcData.channels ?? []).map((/** @type {any} */ ch) => ch.name),
    );
    const expectedIpcChannels = [
      "DISCORD_POWER_MONITOR_GET_SYSTEM_IDLE_TIME",
      "DISCORD_POWER_MONITOR_SUSPEND",
      "DISCORD_POWER_MONITOR_RESUME",
      "DISCORD_POWER_MONITOR_LOCK_SCREEN",
      "DISCORD_POWER_MONITOR_UNLOCK_SCREEN",
      "DISCORD_POWER_SAVE_BLOCKER_BLOCK_DISPLAY_SLEEP",
      "DISCORD_POWER_SAVE_BLOCKER_UNBLOCK_DISPLAY_SLEEP",
      "DISCORD_POWER_SAVE_BLOCKER_CLEANUP_DISPLAY_SLEEP",
    ];
    for (const ch of expectedIpcChannels) {
      assert.ok(
        ipcChannelNames.has(ch),
        `Канал ${ch} должен присутствовать в оракуле docs/re/backend-ipc.json`,
      );
    }

    const mainArchText = readFileSync(MAIN_ARCH_MD_PATH, "utf8");
    assert.match(mainArchText, /core #4531/);
    assert.match(mainArchText, /getWaylandSystemIdleTimeMs/);

    // 3. Независимый оракул: fixtures/dbus/power.json
    const dbusPower = JSON.parse(readFileSync(DBUS_POWER_FIXTURE_PATH, "utf8"));
    assert.equal(dbusPower.services[0].destination, "org.freedesktop.ScreenSaver");
    assert.equal(dbusPower.services[1].destination, "org.freedesktop.login1");

    // 4. Проверяем наличие всех констант оракулов и дизайн-токенов в исходнике battery_saver.rs
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card976.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} из TASK-976 должен быть зафиксирован в battery_saver.rs`,
      );
    }
    for (const ch of expectedIpcChannels) {
      assert.ok(
        source.includes(`"${ch}"`),
        `IPC-канал ${ch} должен быть определён в battery_saver.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_976_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-976-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_976_1_.*ok/);
  });

  it("SUB-976-2: снижает частоту кадров со 120 FPS (AC) до строго 30 FPS на батарее / при неактивности и до 0 FPS при suspend/lock-screen", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "AC_ACTIVE_FPS",
      "BATTERY_SAVER_FPS",
      "SUSPENDED_FPS",
      "FRAME_BUDGET_AC_US",
      "FRAME_BUDGET_SAVER_US",
      "PowerSource",
      "PowerToggleState",
      "BatterySaverPolicy",
      "BatterySaverController",
      "evaluate_target_fps",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_976_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-976-2 (30 FPS Battery Saver governor) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_976_2_.*ok/);
  });

  it("SUB-976-3: коалесцирует фоновые таймеры и удерживает частоту пробуждений CPU строго < 15 wakeups/s для глубоких C-States", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "MAX_IDLE_WAKEUPS_PER_SEC",
      "COALESCED_TIMER_QUANTUM_MS",
      "TimerCoalescer",
      "WakeupBenchmarkReport",
      "CStateResidencyEstimate",
      "run_battery_saver_benchmark",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_976_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-976-3 (< 15 wakeups/s & C-states) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_976_3_.*ok/);
  });

  it("SUB-976-4: проверяет все 9 измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, 14.99 vs 15.00 wakeups/s, ошибки 401..5xx, skeleton/202, права, fallback и маскирование токенов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "validate_workload_boundaries",
      "compute_rest_backoff_ms",
      "resolve_linux_idle_time_ms",
      "redact_power_telemetry_secrets",
      "export_power_telemetry_json",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_976_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-976-4 (9 измерений TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_976_4_.*ok/);
  });

  it("SUB-976-5: убивает 5/5 контрольных мутаций модуля battery_saver.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-battery_saver-01",
        operator: "BatteryFpsThrottleBypass",
        from: "pub const BATTERY_SAVER_FPS: u32 = 30;",
        to: "pub const BATTERY_SAVER_FPS: u32 = 60;",
      },
      {
        id: "MUT-battery_saver-02",
        operator: "WakeupSlaStrictInequalityRelaxed",
        from: "if wakeups_per_sec >= MAX_IDLE_WAKEUPS_PER_SEC {",
        to: "if wakeups_per_sec > MAX_IDLE_WAKEUPS_PER_SEC {",
      },
      {
        id: "MUT-battery_saver-03",
        operator: "SuspendZeroFpsBypass",
        from: "if self.system_suspended || self.screen_locked {",
        to: "if false && (self.system_suspended || self.screen_locked) {",
      },
      {
        id: "MUT-battery_saver-04",
        operator: "TimerCoalescingBucketBypass",
        from: "let bucket_ms = deadline_ms.div_ceil(quantum) * quantum;",
        to: "let bucket_ms = deadline_ms;",
      },
      {
        id: "MUT-battery_saver-05",
        operator: "TelemetryTokenRedactionBypass",
        from: "\"mfa.***\".to_string()",
        to: "tok.to_string()",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "battery_saver.rs",
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
