// @ts-check
/**
 * Контрактные и модульные тесты автономной верификации системных библиотек ldd,
 * совместимости с GLIBC <= 2.35 и софтверного рендеринга llvmpipe (TASK-953):
 * - SUB-953-1: Инспекция зависимостей релизного бинарника `rust/dist/discord-gpui`
 *   через `ldd` и `objdump -p`, проверка `GLIBC <= 2.35` и отсутствия жёсткой привязки
 *   к проприетарным библиотекам NVIDIA/AMD (`libcuda`, `libnvidia-*`, `libamdhip64`).
 * - SUB-953-2: Проверка холодного старта и `--headless-smoke` при софтверном рендеринге
 *   Mesa `llvmpipe` (`LIBGL_ALWAYS_SOFTWARE=1`, `MESA_LOADER_DRIVER_OVERRIDE=llvmpipe`).
 * - SUB-953-3: Проверка полноты и структуры документа `docs/SYSTEM-REQUIREMENTS.md`
 *   (ядро Linux 5.15+, дистрибутивы с GLIBC 2.35+, графические серверы Wayland/X11,
 *   таблица разделяемых библиотек и результаты верификации).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "verify-system-requirements.mjs");
const DOC_PATH = join(ROOT, "docs", "SYSTEM-REQUIREMENTS.md");
const DIST_BIN_PATH = join(ROOT, "rust", "dist", "discord-gpui");

describe("TASK-953: Автономная верификация системных библиотек ldd, GLIBC <= 2.35 и llvmpipe", () => {
  it("SUB-953-1 (Шаг 1.1 & 1.3): анализирует вывод ldd, проверяет 0 отсутствующих библиотек и отсутствие проприетарных драйверов NVIDIA/AMD", async () => {
    const {
      parseLddOutput,
      detectProprietaryGpuLibraries,
      inspectBinaryLdd,
    } = await import("../../scripts/verify-system-requirements.mjs");

    // 1. Реальная инспекция rust/dist/discord-gpui через ldd
    const liveLdd = inspectBinaryLdd(DIST_BIN_PATH);
    assert.equal(liveLdd.passed, true, `Неожиданная ошибка ldd: ${liveLdd.reasons.join("; ")}`);
    assert.equal(liveLdd.missingLibraries.length, 0, "Все разделяемые библиотеки должны разрешаться");
    assert.equal(liveLdd.proprietaryLibraries.length, 0, "Не должно быть жёсткой привязки к NVIDIA/AMD");

    for (const requiredLib of [
      "libc.so.6",
      "libm.so.6",
      "libgcc_s.so.1",
      "libasound.so.2",
      "libxcb.so.1",
      "libxkbcommon.so.0",
      "libxkbcommon-x11.so.0",
    ]) {
      assert.ok(
        liveLdd.libraries.some((entry) => entry.name === requiredLib),
        `В списке ldd должна присутствовать системная библиотека ${requiredLib}`,
      );
    }

    // 2. Негативный сценарий: отсутствующая библиотека (=> not found)
    const missingSample = [
      "\tlinux-vdso.so.1 (0x00007fff00001000)",
      "\tlibxcb.so.1 => /usr/lib/libxcb.so.1 (0x00007f0000100000)",
      "\tlibmissing_audio.so.9 => not found",
      "\tlibc.so.6 => /usr/lib/libc.so.6 (0x00007f0000000000)",
    ].join("\n");
    const parsedMissing = parseLddOutput(missingSample);
    assert.equal(parsedMissing.missingLibraries.length, 1);
    assert.equal(parsedMissing.missingLibraries[0], "libmissing_audio.so.9");

    // 3. Негативный сценарий: жёсткая привязка к проприетарным библиотекам NVIDIA и AMD
    const proprietaryList = detectProprietaryGpuLibraries([
      "libxcb.so.1",
      "libcuda.so.1",
      "libnvidia-encode.so.1",
      "libamdhip64.so.6",
      "libc.so.6",
    ]);
    assert.deepEqual(proprietaryList, [
      "libcuda.so.1",
      "libnvidia-encode.so.1",
      "libamdhip64.so.6",
    ]);
  });

  it("SUB-953-1 (Шаг 1.2): проверяет требование GLIBC <= 2.35 по секции Version References из objdump -p", async () => {
    const {
      compareGlibcVersions,
      evaluateGlibcVersionReferences,
      inspectBinaryGlibc,
    } = await import("../../scripts/verify-system-requirements.mjs");

    // 1. Граничные проверки компаратора версий GLIBC (2.34 < 2.35 == 2.35 < 2.36)
    assert.ok(compareGlibcVersions("2.2.5", "2.35") < 0);
    assert.ok(compareGlibcVersions("2.34", "2.35") < 0);
    assert.equal(compareGlibcVersions("2.35", "2.35"), 0);
    assert.ok(compareGlibcVersions("2.36", "2.35") > 0);
    assert.ok(compareGlibcVersions("2.39", "2.35") > 0);

    // 2. Позитивный и граничный анализ секции Version References:
    //    - обязательные символы libc.so.6 <= GLIBC_2.35 (вкл. GLIBC_2.34 и GLIBC_2.35)
    //    - слабые символы (флаг 0x02 VER_FLG_WEAK, например pidfd_spawnp@GLIBC_2.39) не нарушают запуск на glibc 2.35
    const validObjdumpSample = [
      "Version References:",
      "  required from libasound.so.2:",
      "    0x01772779 0x00 06 ALSA_0.9",
      "  required from libm.so.6:",
      "    0x09691a75 0x00 10 GLIBC_2.2.5",
      "    0x069691b5 0x00 22 GLIBC_2.35",
      "  required from libc.so.6:",
      "    0x09691a75 0x00 03 GLIBC_2.2.5",
      "    0x069691b4 0x00 02 GLIBC_2.34",
      "    0x069691b9 0x02 25 GLIBC_2.39",
    ].join("\n");

    const evalValid = evaluateGlibcVersionReferences(validObjdumpSample, "2.35");
    assert.equal(evalValid.passed, true);
    assert.equal(evalValid.maxMandatoryLibcVersion, "2.34");
    assert.equal(evalValid.maxBaselineGlibcVersion, "2.35");
    assert.deepEqual(evalValid.weakVersions, ["2.39"]);

    // 3. Негативный сценарий: обязательный символ (0x00) в libc.so.6 требует GLIBC_2.36 > 2.35
    const invalidObjdumpSample = [
      "Version References:",
      "  required from libc.so.6:",
      "    0x09691a75 0x00 03 GLIBC_2.2.5",
      "    0x069691b4 0x00 02 GLIBC_2.34",
      "    0x069691b6 0x00 11 GLIBC_2.36",
    ].join("\n");

    const evalInvalid = evaluateGlibcVersionReferences(invalidObjdumpSample, "2.35");
    assert.equal(evalInvalid.passed, false);
    assert.match(evalInvalid.reasons.join("; "), /GLIBC_2\.36/);

    // 4. Реальная инспекция релизного бинарника rust/dist/discord-gpui
    const liveGlibc = inspectBinaryGlibc(DIST_BIN_PATH, "2.35");
    assert.equal(liveGlibc.passed, true, `Проверка GLIBC не прошла: ${liveGlibc.reasons.join("; ")}`);
    assert.equal(liveGlibc.maxMandatoryLibcVersion, "2.34");
    assert.ok(compareGlibcVersions(liveGlibc.maxBaselineGlibcVersion, "2.35") <= 0);
  });

  it("SUB-953-2 (Шаг 2.1 & 2.2): запускает discord-gpui --headless-smoke на программном рендерере Mesa llvmpipe", async () => {
    const {
      buildLlvmPipeEnv,
      runLlvmPipeSmokeVerification,
    } = await import("../../scripts/verify-system-requirements.mjs");

    // 1. Проверка переменных окружения софтверного рендеринга
    const env = buildLlvmPipeEnv({ CUSTOM_VAR: "1" });
    assert.equal(env.LIBGL_ALWAYS_SOFTWARE, "1");
    assert.equal(env.MESA_LOADER_DRIVER_OVERRIDE, "llvmpipe");
    assert.equal(env.GALLIUM_DRIVER, "llvmpipe");
    assert.equal(env.CUSTOM_VAR, "1");

    // 2. Реальный холодный старт rust/dist/discord-gpui --headless-smoke под llvmpipe
    const smoke = runLlvmPipeSmokeVerification(DIST_BIN_PATH);
    assert.equal(smoke.passed, true, `llvmpipe smoke упал: ${smoke.reasons.join("; ")}`);
    assert.equal(smoke.exitCode, 0);
    assert.equal(smoke.payload?.status, "ok");
    assert.equal(smoke.payload?.sqlite_wal_ok, true);
    assert.equal(smoke.payload?.fps_budget_ok, true);
    assert.ok(
      typeof smoke.payload?.vmrss_mb === "number" && smoke.payload.vmrss_mb > 0 && smoke.payload.vmrss_mb < 60.0,
      `VmRSS (${smoke.payload?.vmrss_mb} МБ) должен быть в пределах < 60 МБ`,
    );
  });

  it("SUB-953-3 (Шаг 3.1): проверяет CLI scripts/verify-system-requirements.mjs и документ docs/SYSTEM-REQUIREMENTS.md", () => {
    const tmp = mkdtempSync(join(tmpdir(), "sys-req-953-"));
    try {
      const outJson = join(tmp, "sys-req-report.json");
      const run = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(run.status, 0, `stderr: ${run.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт верификации должен быть создан");

      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-953");
      assert.equal(report.status, "passed");
      assert.equal(report.ldd.passed, true);
      assert.equal(report.glibc.passed, true);
      assert.equal(report.llvmpipe.passed, true);

      // Коды выхода CLI: 1 при неверном аргументе, 2 при отсутствии бинарника
      const badArg = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badArg.status, 1);

      const missingBin = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--binary", "/nonexistent/discord-gpui-missing"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingBin.status, 2);

      // Проверка целевого документа docs/SYSTEM-REQUIREMENTS.md
      assert.ok(existsSync(DOC_PATH), "Документ docs/SYSTEM-REQUIREMENTS.md должен существовать");
      const docText = readFileSync(DOC_PATH, "utf8");
      assert.match(docText, /TASK-953/);
      assert.match(docText, /GLIBC_2\.35|glibc\s*[<≤=]+\s*2\.35|GLIBC\s*2\.35/i);
      assert.match(docText, /5\.15\+/);
      assert.match(docText, /Ubuntu 22\.04 LTS/);
      assert.match(docText, /Debian 12/);
      assert.match(docText, /Fedora 38\+/);
      assert.match(docText, /LIBGL_ALWAYS_SOFTWARE=1/);
      assert.match(docText, /MESA_LOADER_DRIVER_OVERRIDE=llvmpipe/);
      assert.match(docText, /libxcb\.so\.1/);
      assert.match(docText, /libxkbcommon\.so\.0/);
      assert.match(docText, /libasound\.so\.2/);
      assert.match(docText, /Wayland/);
      assert.match(docText, /X11/);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
