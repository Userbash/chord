// @ts-check
/**
 * Контрактные и модульные тесты бинаурального HRTF фильтра пространственного звука (TASK-1017A):
 * - SUB-1017A-0: Статический и компиляционный контракт `rust/crates/discord-voice/src/spatial_audio.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1017A-1: Сверка карточки `TASK-1017A` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `HRTF Convolver`
 *   (`FFT 512 pt`, `padding: 0px`, `spacing: 0px`), интерактивных состояний `spatial-panner`
 *   (`center` -> `panning` -> `3d localized`), оракула KEMAR HRTF dataset (ISO / MIT KEMAR:
 *   радиус головы `0.0875 м`, скорость звука `343.0 м/с`, `ITD <= 0.7 мс`, `ILD <= 20.0 дБ`)
 *   и RE-оракула `/home/sanya/re-deep/voice-deep.md` (`docs/re/voice-deep.md`).
 * - SUB-1017A-2: Расчет межушной разности времени (`compute_itd_ms`, `compute_itd_samples` до `0.7 мс`)
 *   и разности уровней (`compute_ild_db` до `20 дБ`), `compute_binaural_cues` по азимуту `-180..+180 deg`
 *   и затухания громкости `1/r` (`compute_distance_attenuation`).
 * - SUB-1017A-3: Быстрое сверточное преобразование `HrtfConvolver512` (`fft_radix2_512`, `ifft_radix2_512`,
 *   `convolve_mono_to_stereo_frame`) моно-потока `48kHz` (`960` отсчетов на `20 мс`) в стерео-сигнал
 *   (`1920` чередующихся L/R отсчетов) и микширование 10 пиров (`SpatialAudioMixer10Peers`) с бюджетом
 *   потребления CPU `< 1.5%`.
 * - SUB-1017A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`,
 *   громкость `0..200%`, бейдж `99+`; ошибки HTTP `400/401/403/404/413/429 retry_after_ms/5xx`;
 *   пустые; `skeleton`/`HTTP 202`; права `CONNECT`/`SPEAK`/`AGE_GATED_SPACES`; восстановление при
 *   NaN/поврежденном аудиокадре; маскирование токенов в логах).
 * - SUB-1017A-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-voice/src/spatial_audio.rs`.
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
const RUST_MODULE_REL_PATH = "rust/crates/discord-voice/src/spatial_audio.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DOCS_VOICE_DEEP_PATH = join(ROOT, "docs", "re", "voice-deep.md");
const RE_DEEP_VOICE_DEEP_PATH = "/home/sanya/re-deep/voice-deep.md";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-voice/src/spatial_audio.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1017A: Бинауральный HRTF фильтр 360-градусного панорамирования (ITD/ILD, FFT 512 pt свертка моно в стерео, 1/r дистанция)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "spatial-audio-1017a-"));
    testBinPath = join(workDir, "spatial_audio_1017a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1017A-0: проверяет статический контракт rust/crates/discord-voice/src/spatial_audio.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде spatial_audio.rs запрещены .unwrap() и .expect()",
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

  it("SUB-1017A-1: сверяет карточку TASK-1017A в waves/kanban-board.json, дизайн-токены, геометрию HRTF Convolver (FFT 512 pt), состояния spatial-panner, оракул KEMAR HRTF и voice-deep.md", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1017A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1017A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1017A",
    );
    assert.ok(
      card1017A,
      "Карточка TASK-1017A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1017A.department, "G5 Voice / Platform");
    assert.equal(card1017A.priority, "P2");
    assert.deepEqual(card1017A.layout, {
      container: "HRTF Convolver",
      dimensions: "FFT 512 pt",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1017A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1017A.interactive_states[0], {
      element: "spatial-panner",
      default: "center",
      hover: "panning",
      active: "3d localized",
    });

    // 2. Оракул: /home/sanya/re-deep/voice-deep.md и docs/re/voice-deep.md
    const voiceDeepPath = existsSync(RE_DEEP_VOICE_DEEP_PATH)
      ? RE_DEEP_VOICE_DEEP_PATH
      : DOCS_VOICE_DEEP_PATH;
    const voiceDeepRaw = readFileSync(voiceDeepPath, "utf8");
    assert.match(voiceDeepRaw, /PT=120/);
    assert.match(voiceDeepRaw, /aead_aes256_gcm_rtpsize/);
    assert.match(voiceDeepRaw, /17843/);
    assert.match(voiceDeepRaw, /9208/);
    assert.match(voiceDeepRaw, /960 @48 кГц/);

    // 3. Сверка констант в исходнике spatial_audio.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1017A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1017A должен присутствовать в spatial_audio.rs`,
      );
    }
    assert.ok(rustSource.includes("HRTF Convolver"));
    assert.ok(rustSource.includes("FFT 512 pt"));
    assert.ok(rustSource.includes("spatial-panner"));
    assert.ok(rustSource.includes("3d localized"));
    assert.ok(rustSource.includes("KEMAR"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1017a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1017A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017a_1_.*ok/);
  });

  it("SUB-1017A-2: проверяет расчет межушной задержки ITD (до 0.7 мс) и разности уровней ILD (до 20 дБ) по азимуту -180..+180 deg и затухание 1/r", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "KEMAR_HEAD_RADIUS_M",
      "SPEED_OF_SOUND_M_S",
      "MAX_ITD_MS",
      "MAX_ILD_DB",
      "MIN_AZIMUTH_DEG",
      "MAX_AZIMUTH_DEG",
      "BinauralCues",
      "compute_itd_ms",
      "compute_itd_samples",
      "compute_ild_db",
      "compute_distance_attenuation",
      "compute_binaural_cues",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В spatial_audio.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1017a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1017A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017a_2_.*ok/);
  });

  it("SUB-1017A-3: проверяет FFT-свертку 512 pt (HrtfConvolver512) моно-сигнала 48kHz в стерео и микширование 10 говорящих пиров (CPU < 1.5%)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "FFT_SIZE_512",
      "HRIR_TAPS_LEN",
      "SAMPLES_PER_CHANNEL_20MS",
      "INTERLEAVED_STEREO_SAMPLES_20MS",
      "MAX_SIMULTANEOUS_PEERS",
      "MAX_CPU_USAGE_PERCENT_10_PEERS",
      "Complex32",
      "fft_radix2_512",
      "ifft_radix2_512",
      "HrtfConvolver512",
      "SpatialAudioMixer10Peers",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В spatial_audio.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1017a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1017A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017a_3_.*ok/);
  });

  it("SUB-1017A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права CONNECT/SPEAK/AGE_GATED_SPACES, восстановление NaN-кадра и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "validate_snowflake_id",
      "evaluate_spatial_voice_permissions",
      "validate_platform_boundaries",
      "classify_voice_http_status",
      "format_badge_cap",
      "recover_corrupted_frame",
      "redact_spatial_audio_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В spatial_audio.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1017a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1017A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1017a_4_.*ok/);
  });

  it("SUB-1017A-5: убивает 5/5 контрольных мутаций в rust/crates/discord-voice/src/spatial_audio.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль spatial_audio.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1017A-RS-01",
        moduleName: "spatial_audio",
        operator: "MaxItdConstantTamper",
        from: "pub const MAX_ITD_MS: f32 = 0.7;",
        to: "pub const MAX_ITD_MS: f32 = 0.1;",
      },
      {
        id: "MUT-1017A-RS-02",
        moduleName: "spatial_audio",
        operator: "MaxIldConstantTamper",
        from: "pub const MAX_ILD_DB: f32 = 20.0;",
        to: "pub const MAX_ILD_DB: f32 = 2.0;",
      },
      {
        id: "MUT-1017A-RS-03",
        moduleName: "spatial_audio",
        operator: "InverseDistanceLawTamper",
        from: "Ok(1.0 / clamped)",
        to: "Ok(clamped)",
      },
      {
        id: "MUT-1017A-RS-04",
        moduleName: "spatial_audio",
        operator: "FftConvolverRightGainBypass",
        from: "let gain_r = cues.right_gain * cues.distance_gain;",
        to: "let gain_r = 0.0;",
      },
      {
        id: "MUT-1017A-RS-05",
        moduleName: "spatial_audio",
        operator: "CpuBudgetLimitTamper",
        from: "cpu_usage_percent <= MAX_CPU_USAGE_PERCENT_10_PEERS",
        to: "false && cpu_usage_percent <= MAX_CPU_USAGE_PERCENT_10_PEERS",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в spatial_audio.rs: ${m.from}`,
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
      "MD5 исходного файла spatial_audio.rs не должен измениться после мутационного гейта",
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
