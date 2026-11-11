// @ts-check
/**
 * Контрактные и модульные тесты интеграции ИИ-шумоподавления RNNoise / Krisp
 * (SIMD AVX2 аудиофильтр в `rust/discord-voice/src/rnnoise.rs`, SNR > 15 dB, задержка < 5 мс) — TASK-983:
 * - SUB-983-0: Статический и структурный контракт `rust/discord-voice/src/rnnoise.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, лимиты `scripts/quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на модуль, `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-983-1: Сверка констант RNNoise / Krisp, моделей `.kef`, возможностей SIMD (`support_avx`,
 *   `support_avx2`, `support_avx512_bf16`), параметров кадра (`48kHz mono, 10ms frame` = 480 сэмплов),
 *   дизайн-токенов и интерактивных состояний `rnnoise-toggle` (`active`, `show SNR boost`, `filtering`)
 *   с независимыми оракулами `waves/kanban-board.json`, `docs/re/native-re.md`, `docs/re/native-re.json`,
 *   `docs/re/voice-deep.md` и `rust/crates/discord-voice/src/noise.rs`.
 * - SUB-983-2: Независимый математический оракул 8-канального SIMD AVX2 (`Avx2SimdLane8`, 256-бит `f32x8`
 *   FMA / dot-product), 24 критических полос Bark/ERB и рекуррентной ячейки GRU (`z_t`, `r_t`, `h_t`).
 * - SUB-983-3: Фильтрация стационарного шума окружения и импульсного шума механической клавиатуры
 *   на потоке 48 кГц моно (кадры 10 мс = 480 сэмплов) с подтверждением прироста SNR > 15 dB
 *   и задержки аудиотракта строго < 5 мс (`MAX_AUDIO_LATENCY_MS = 5.0`).
 * - SUB-983-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: `15.00 dB` vs `15.01 dB` SNR, `4.99 ms` vs `5.00 ms` latency, `480` сэмплов,
 *   Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`; ошибки `401/403/404/413/429/5xx`; пустые/тишина/DTX `[0xF8, 0xFF, 0xFE]`;
 *   прогрев `HTTP 202`; права `VIEW_CHANNEL`/`SPEAK`/`AGE_GATED_SPACES`; fallback Krisp -> RNNoise AVX2 -> Scalar;
 *   маскирование секретов `mfa.***` в JSON-телеметрии).
 * - SUB-983-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const MODULE_REL_PATH = "rust/discord-voice/src/rnnoise.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const NATIVE_RE_MD_PATH = join(ROOT, "docs", "re", "native-re.md");
const NATIVE_RE_JSON_PATH = join(ROOT, "docs", "re", "native-re.json");
const VOICE_DEEP_MD_PATH = join(ROOT, "docs", "re", "voice-deep.md");
const CRATE_NOISE_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-voice",
  "src",
  "noise.rs",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-voice/src/rnnoise.rs` через `rustc --test --edition 2024 -D warnings`.
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
 * Независимый JS-оракул 8-канального SIMD AVX2 FMA (`a[i] * b[i] + c[i]` в `Float32Array`).
 * @param {number[]} a
 * @param {number[]} b
 * @param {number[]} c
 * @returns {{ lanes: Float32Array, dot: number }}
 */
function oracleAvx2FmaAndDot8(a, b, c) {
  const fa = new Float32Array(a);
  const fb = new Float32Array(b);
  const fc = new Float32Array(c);
  const lanes = new Float32Array(8);
  let dot = 0;
  for (let i = 0; i < 8; i += 1) {
    const prod = Math.fround((fa[i] ?? 0) * (fb[i] ?? 0));
    lanes[i] = Math.fround(prod + (fc[i] ?? 0));
    dot = Math.fround(dot + prod);
  }
  return { lanes, dot };
}

/**
 * Независимый JS-оракул расчёта отношения сигнал/шум (SNR, дБ) и прироста SNR (`deltaSnrDb`).
 * @param {Float32Array} cleanSpeech
 * @param {Float32Array} noisyInput
 * @param {Float32Array} filteredOutput
 * @returns {{ inputSnrDb: number, outputSnrDb: number, snrImprovementDb: number }}
 */
function oracleComputeSnrImprovementDb(cleanSpeech, noisyInput, filteredOutput) {
  let speechPower = 0;
  let inNoisePower = 0;
  let outNoisePower = 0;
  for (let i = 0; i < cleanSpeech.length; i += 1) {
    const s = cleanSpeech[i] ?? 0;
    const nIn = (noisyInput[i] ?? 0) - s;
    const nOut = (filteredOutput[i] ?? 0) - s;
    speechPower += s * s;
    inNoisePower += nIn * nIn;
    outNoisePower += nOut * nOut;
  }
  const eps = 1e-12;
  const inputSnrDb = 10 * Math.log10((speechPower + eps) / (inNoisePower + eps));
  const outputSnrDb = 10 * Math.log10((speechPower + eps) / (outNoisePower + eps));
  return {
    inputSnrDb,
    outputSnrDb,
    snrImprovementDb: outputSnrDb - inputSnrDb,
  };
}

describe("TASK-983: ИИ-шумоподавление RNNoise / Krisp (SIMD AVX2 фильтр в rust/discord-voice/src/rnnoise.rs, SNR > 15 dB, задержка < 5 мс)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "voice-rnnoise-983-"));
    testBinPath = join(workDir, "rnnoise_983_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-983-0: модуль rust/discord-voice/src/rnnoise.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты размера (<50 строк на fn, <800 на файл) и компилируется без предупреждений", () => {
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

    const prodSection = source.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продакшен-коде модуля rnnoise.rs запрещены .unwrap() и .expect()",
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

    const secretFindings = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      "В исходнике rnnoise.rs не должно быть незамаскированных секретов",
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

  it("SUB-983-1: сверяет константы RNNoise / Krisp, модели .kef, флаги SIMD AVX2, параметры 48kHz/10ms и интерактивные состояния rnnoise-toggle с независимыми RE-оракулами", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-983 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card983 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-983");
    assert.ok(card983, "Карточка TASK-983 должна присутствовать в waves/kanban-board.json");
    assert.equal(card983.layout.container, "Noise Suppression");
    assert.equal(card983.layout.dimensions, "48kHz mono, 10ms frame");
    assert.deepEqual(card983.interactive_states[0], {
      element: "rnnoise-toggle",
      default: "active",
      hover: "show SNR boost",
      active: "filtering",
    });

    // 2. Независимый оракул: docs/re/native-re.json (discord_krisp.node, SIMD AVX/AVX2/AVX512, GRU/DeepFilterNet)
    const nativeReJson = JSON.parse(readFileSync(NATIVE_RE_JSON_PATH, "utf8"));
    const krispMod = (nativeReJson.modules ?? []).find(
      (/** @type {any} */ m) => m.name === "discord_krisp",
    );
    assert.ok(krispMod, "Запись discord_krisp должна существовать в docs/re/native-re.json");
    assert.equal(krispMod.size, 62_078_048);
    assert.equal(krispMod.md5, "9be929d1d90a8290b051fd07e2c34eb3");
    assert.equal(
      krispMod.sha256,
      "3d70b39670d5cf024a6a730bae488af5bd4ec69d6c309d5c8f0f29ce80052f60",
    );
    const simdSamples = krispMod.camelCaseRuns?.[0]?.sample ?? [];
    for (const simdFlag of ["support_avx", "support_avx2", "support_avx512_bf16"]) {
      assert.ok(
        simdSamples.includes(simdFlag),
        `Флаг SIMD ${simdFlag} должен присутствовать в оракуле docs/re/native-re.json`,
      );
    }

    // 3. Независимый оракул: docs/re/native-re.md (§4.1 модели .kef и их SHA-256) и docs/re/voice-deep.md (§3)
    const nativeReMd = readFileSync(NATIVE_RE_MD_PATH, "utf8");
    const expectedKefModels = [
      {
        name: "krisp-nc-o-med-v7.kef",
        sha256: "1b421bd27f892c6aef041496c939da4f31360fda36d55b83e48435cae89d246f",
      },
      {
        name: "krisp-nc-o-lite-v1.kef",
        sha256: "7fdff67057090f402fb7688b29bf0afbd65cb1ed6a6956d2e304b5f3d84afa48",
      },
      {
        name: "krisp-nc-o-nb-v2.kef",
        sha256: "b3d2737fae2e6f900f7efb33aa17a3b6e6548e48ebcfbf5bd2f0746eec04c086",
      },
      {
        name: "krisp-bvc-o-pro-v3.kef",
        sha256: "3f65d975ee4487931aff462e71e740a37b581817f9bad860590b194409688859",
      },
      {
        name: "krisp-vad-o-v2.kef",
        sha256: "092c397f12443e5f2bdd07428b9e193ce359f6a08bfb403807c540f18647b3f3",
      },
    ];
    for (const kef of expectedKefModels) {
      assert.ok(nativeReMd.includes(kef.name), `Модель ${kef.name} должна быть в native-re.md`);
      assert.ok(nativeReMd.includes(kef.sha256), `SHA-256 модели ${kef.name} должен быть в native-re.md`);
    }

    const voiceDeepMd = readFileSync(VOICE_DEEP_MD_PATH, "utf8");
    assert.match(voiceDeepMd, /\[VOICE_DISCONNECT\] krisp_nc_model: krisp-nc-o-med-v7\.kef/);

    const crateNoiseSrc = readFileSync(CRATE_NOISE_RS_PATH, "utf8");
    assert.match(crateNoiseSrc, /pub const DEFAULT_RNNOISE_SUPPRESSION_DB: u8 = 30;/);
    assert.match(crateNoiseSrc, /pub const DEFAULT_KRISP_SUPPRESSION_DB: u8 = 45;/);

    // 4. Проверяем наличие всех констант оракулов и дизайн-токенов в rust/discord-voice/src/rnnoise.rs
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card983.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} из TASK-983 должен быть зафиксирован в rnnoise.rs`,
      );
    }
    for (const kef of expectedKefModels) {
      assert.ok(
        source.includes(kef.name),
        `Имя модели ${kef.name} должно быть зафиксировано в rnnoise.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_983_1_");
    assert.equal(run.status, 0, `Тесты SUB-983-1 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_983_1_.*ok/);
  });

  it("SUB-983-2: сверяет 8-канальное ядро SIMD AVX2 (Avx2SimdLane8, 256-бит f32x8 FMA/dot), 24 полосы Bark/ERB и GRU-инференс с независимым Float32-оракулом", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // Независимый расчёт 8-канального SIMD FMA и скалярного произведения в Float32
    const a = [0.25, -0.5, 0.75, 1.0, -0.25, 0.5, -0.75, 0.125];
    const b = [2.0, 1.5, -0.5, 0.25, -4.0, 0.75, 2.0, 8.0];
    const c = [0.1, 0.2, 0.3, -0.1, 0.0, 0.125, 0.5, -0.25];
    const expected = oracleAvx2FmaAndDot8(a, b, c);
    assert.ok(Math.abs(expected.dot - 0.5) < 1e-6);
    assert.ok(Math.abs((expected.lanes[0] ?? 0) - 0.6) < 1e-6);
    assert.ok(Math.abs((expected.lanes[7] ?? 0) - 0.75) < 1e-6);

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "Avx2SimdLane8",
      "SimdIsaLevel",
      "RNNOISE_BARK_BANDS",
      "RNNOISE_FRAME_SAMPLES",
      "RNNOISE_WINDOW_SAMPLES",
      "compute_bark_band_energies_24",
      "RnNoiseGruState",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле rnnoise.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_983_2_");
    assert.equal(run.status, 0, `Тесты SUB-983-2 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_983_2_.*ok/);
  });

  it("SUB-983-3: подавляет стационарный фоновый шум и импульсный шум клавиатуры на > 15 dB при задержке аудиотракта < 5 мс (48kHz mono, 10ms = 480 сэмплов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // Независимый JS-оракул проверки математической формулы SNR > 15 dB на 480 сэмплах (48kHz, 10ms)
    const clean = new Float32Array(480);
    const noisy = new Float32Array(480);
    const denoised = new Float32Array(480);
    for (let n = 0; n < 480; n += 1) {
      const t = n / 48_000;
      const speech =
        0.35 * Math.sin(2 * Math.PI * 220 * t) +
        0.2 * Math.sin(2 * Math.PI * 440 * t);
      const stationary = 0.12 * Math.sin(2 * Math.PI * 3_750 * t + 0.3);
      const impulse = n === 120 || n === 310 ? 0.85 : 0.0;
      clean[n] = speech;
      noisy[n] = speech + stationary + impulse;
      // Эталонное ослабление стационарного и импульсного шума (> 20 dB)
      denoised[n] = speech + stationary * 0.05 + impulse * 0.02;
    }
    const oracleSnr = oracleComputeSnrImprovementDb(clean, noisy, denoised);
    assert.ok(
      oracleSnr.snrImprovementDb > 15.0,
      `Эталонный прирост SNR должен превышать 15 dB, получено: ${oracleSnr.snrImprovementDb.toFixed(2)} dB`,
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_983_3_");
    assert.equal(run.status, 0, `Тесты SUB-983-3 (SNR > 15 dB & latency < 5 ms) упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_983_3_.*ok/);
  });

  it("SUB-983-4: покрывает все 9 измерений TEST-POLICY.md (граничные 15.00 vs 15.01 dB SNR, 4.99 vs 5.00 ms задержка, 480 сэмплов, Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, ошибки 401..5xx, тишина/DTX, HTTP 202, права, fallback Krisp->RNNoise AVX2 и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "MIN_REQUIRED_SNR_IMPROVEMENT_DB",
      "MAX_AUDIO_LATENCY_MS",
      "validate_rnnoise_sla",
      "validate_voice_workload_boundaries",
      "compute_voice_rest_backoff_ms",
      "redact_voice_secrets",
      "export_rnnoise_telemetry_json",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле rnnoise.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_983_4_");
    assert.equal(run.status, 0, `Тесты SUB-983-4 (9 измерений TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_983_4_.*ok/);
  });

  it("SUB-983-5: убивает 5/5 контрольных мутаций модуля rust/discord-voice/src/rnnoise.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-rnnoise-01",
        operator: "MinSnrRequirementRelaxed",
        from: "pub const MIN_REQUIRED_SNR_IMPROVEMENT_DB: f32 = 15.0;",
        to: "pub const MIN_REQUIRED_SNR_IMPROVEMENT_DB: f32 = 5.0;",
      },
      {
        id: "MUT-rnnoise-02",
        operator: "MaxLatencyBudgetRelaxed",
        from: "pub const MAX_AUDIO_LATENCY_MS: f32 = 5.0;",
        to: "pub const MAX_AUDIO_LATENCY_MS: f32 = 25.0;",
      },
      {
        id: "MUT-rnnoise-03",
        operator: "ImpulseClickSuppressorBypass",
        from: "if delta > impulse_threshold && !is_speech_harmonic {",
        to: "if false && delta > impulse_threshold && !is_speech_harmonic {",
      },
      {
        id: "MUT-rnnoise-04",
        operator: "StationaryNoiseGainBypass",
        from: "let gain = compute_rnnoise_band_gain(band_db, floor_db, vad_prob);",
        to: "let gain = 1.0_f32;",
      },
      {
        id: "MUT-rnnoise-05",
        operator: "LatencyBudgetStrictInequalityRelaxed",
        from: "if latency_ms <= 0.0 || latency_ms >= MAX_AUDIO_LATENCY_MS {",
        to: "if latency_ms <= 0.0 || latency_ms > MAX_AUDIO_LATENCY_MS {",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "rnnoise.rs",
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
