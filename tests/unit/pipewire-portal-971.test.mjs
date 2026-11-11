// @ts-check
/**
 * Контрактные и модульные тесты интеграции PipeWire ScreenCast Portal и DMA-BUF (TASK-971):
 * - SUB-971-1: Протокол XDG Desktop Portal `org.freedesktop.portal.ScreenCast`
 *   (`CreateSession` -> `SelectSources` -> `Start` -> `OpenPipeWireRemote`),
 *   валидация D-Bus путей и токенов, сохранение и маскирование `restore_token`.
 * - SUB-971-2: Передача файловых дескрипторов через `SCM_RIGHTS` (`sendmsg`/`recvmsg`
 *   по реальному `AF_UNIX` `socketpair`), RAII-обёртка `OwnedPortalFd` (`F_DUPFD_CLOEXEC`),
 *   автоматическое закрытие побочных дескрипторов и гарантия `0` утечек FD.
 * - SUB-971-3: Согласование аппаратных буферов `SPA_DATA_DmaBuf` (DRM FourCC `BGRA8888`,
 *   `RGBA8888`, `NV12`, `YUV420`, модификаторы `DRM_FORMAT_MOD_LINEAR` и вендорные тайлы,
 *   откат на `SPA_DATA_MemFd` SHM при `DRM_FORMAT_MOD_INVALID`, проверка геометрии 64..7680x4320 и stride).
 * - SUB-971-4: Автоматическое восстановление сессии при аварийном перезапуске `pipewire.service`
 *   (закрытие устаревшего FD без утечек, переоткрытие `OpenPipeWireRemote` или тихий реконнект
 *   по `restore_token`, пересогласование DMA-BUF / SHM fallback, непрерывность телеметрии).
 * - SUB-971-5: Статический гейт качества (`checkRustSourceSizeLimits`, `rustfmt --edition 2024`)
 *   и мутационный гейт `5/5` (100% kill rate).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
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
const MODULE_REL_PATH = "rust/discord-gpui/src/pipewire_portal.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `pipewire_portal.rs` через `rustc --test --edition 2024 -D warnings`.
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
 * Запускает скомпилированный тестовый бинарник с фильтром теста.
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

describe("TASK-971: Интеграция PipeWire ScreenCast Portal и DMA-BUF (rust/discord-gpui/src/pipewire_portal.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "pipewire-portal-971-"));
    testBinPath = join(workDir, "pipewire_portal_971_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-971-0: модуль rust/discord-gpui/src/pipewire_portal.rs существует, проходит rustfmt 2024, лимиты размера (<50 строк на fn, <800 на файл) и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(sizeCheck.fileLines > 100 && sizeCheck.fileLines < 800);

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл: ${fmtCheck.stderr || fmtCheck.stdout}`,
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

  it("SUB-971-1: создаёт сессию ScreenCast (CreateSession -> SelectSources -> Start -> OpenPipeWireRemote), валидирует токены/пути и маскирует restore_token", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "PORTAL_BUS_NAME",
      "PORTAL_OBJECT_PATH",
      "SCREENCAST_INTERFACE",
      "PortalSessionConfig",
      "PortalSessionState",
      "PipeWirePortalManager",
      "redact_restore_token",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен быть определён символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_971_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-971-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_971_1_.*ok/);
  });

  it("SUB-971-2: передаёт файловые дескрипторы PipeWire через реальный Unix SCM_RIGHTS (sendmsg/recvmsg) без утечек FD при дублировании, ошибках и закрытии", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "SCM_RIGHTS",
      "SOL_SOCKET",
      "MSG_CTRUNC",
      "MAX_FDS_PER_MESSAGE",
      "OwnedPortalFd",
      "send_fds_scm_rights",
      "recv_fds_scm_rights",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать контракт SCM_RIGHTS: ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_971_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-971-2 (SCM_RIGHTS & 0 FD leaks) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_971_2_.*ok/);
  });

  it("SUB-971-3: согласует параметры DMA-BUF (DRM FourCC, линейные и тайловые модификаторы, откат в MemFd SHM при DRM_FORMAT_MOD_INVALID, границы 64..7680x4320 и stride)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "DRM_FORMAT_MOD_LINEAR",
      "DRM_FORMAT_MOD_INVALID",
      "DRM_FOURCC_BGRA8888",
      "DRM_FOURCC_RGBA8888",
      "DRM_FOURCC_NV12",
      "DRM_FOURCC_YUV420",
      "DmaBufNegotiationRequest",
      "NegotiatedDmaBufConfig",
      "negotiate_dma_buf_stream",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать контракт DMA-BUF: ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_971_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-971-3 (DMA-BUF negotiation) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_971_3_.*ok/);
  });

  it("SUB-971-4: восстанавливает активную сессию захвата при перезапуске pipewire.service без утечек FD и без потери restore_token", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "recover_after_pipewire_restart",
      "DaemonRestartReport",
      "PortalTelemetrySnapshot",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать контракт восстановления сессии: ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_971_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-971-4 (pipewire.service restart recovery) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_971_4_.*ok/);
  });

  it("SUB-971-5: убивает 5/5 контрольных мутаций модуля pipewire_portal.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-pipewire_portal-01",
        operator: "ScmRightsTruncatedFlagBypass",
        from: "if (message.msg_flags & MSG_CTRUNC) != 0 {",
        to: "if false && (message.msg_flags & MSG_CTRUNC) != 0 {",
      },
      {
        id: "MUT-pipewire_portal-02",
        operator: "DmaBufInvalidModifierBypass",
        from: "*m != DRM_FORMAT_MOD_INVALID",
        to: "true",
      },
      {
        id: "MUT-pipewire_portal-03",
        operator: "DaemonRestartCloseOldFdOmission",
        from: "let closed_old_fd = self.release_active_fd();",
        to: "let closed_old_fd = None;",
      },
      {
        id: "MUT-pipewire_portal-04",
        operator: "RestoreTokenRedactionBypass",
        from: "\"[REDACTED]\".to_string()",
        to: "token.to_string()",
      },
      {
        id: "MUT-pipewire_portal-05",
        operator: "StrideValidationBypass",
        from: "if candidate_stride < min_stride {",
        to: "if false && candidate_stride < min_stride {",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "pipewire_portal.rs",
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
