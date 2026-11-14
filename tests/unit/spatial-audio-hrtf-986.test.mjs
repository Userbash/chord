// @ts-check
/**
 * Контрактные и модульные тесты пространственного 3D-позиционирования звука участников (HRTF)
 * (`rust/chord-voice/src/spatial_audio.rs`, ITD/ILD расчёт, HRTF convolver, 10 участников < 2% CPU) — TASK-986:
 * - SUB-986-0: Статический и структурный контракт `rust/chord-voice/src/spatial_audio.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, лимиты `scripts/quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на модуль, `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-986-1: Сверка констант 3D Spatial Audio / Steam Audio (Phonon), путей `default.sofa`,
 *   `api_binaural_effect.cpp`, `api_hrtf.cpp`, `distance_attenuation.cpp`, `overlap_save_convolution_effect.cpp`,
 *   NAPI-символов `isSpatialAudioEnabled` / `setSpatialAudioStatusCallback`, дизайн-токенов и состояний
 *   `peer-avatar-3d` (`center`, `drag position`, `panned`) с независимыми оракулами `waves/kanban-board.json`,
 *   `docs/re/native-re.md`, `docs/re/native-re.json` и `rust/crates/chord-voice/src/reception.rs`.
 * - SUB-986-2: Независимый физико-математический оракул ISO KEMAR / модели сферической головы Вудворта
 *   (`a = 0.0875 м`, `c = 343.0 м/с`, межушная разность времени `ITD` до `0.7 мс`, межушная разность уровней
 *   `ILD` до `20.0 дБ` по азимуту `-180°..+180°` и углу места `-90°..+90°`) и закона обратных квадратов/дистанции `1/r`.
 * - SUB-986-3: Бинауральная HRTF-свёртка (Overlap-Save FIR Convolver + дробная линия задержки ITD)
 *   моно-потока участника (48 кГц, 20 мс = 960 сэмплов) в стерео-сигнал с проверкой фазового опережения,
 *   ослабления теневого уха, фронтально-тыловой фильтрации ушной раковины и непрерывности на стыках кадров.
 * - SUB-986-4: Бенчмарк пространственного микширования 10 одновременных участников с проверкой бюджета
 *   нагрузки CPU `< 2.0%` (`MAX_SPATIAL_CPU_LOAD_PCT = 2.0`) и покрытие всех 9 измерений `TEST-POLICY.md`
 *   (позитивные, негативные, граничные `1.99%` vs `2.00%` CPU, азимут `-180°..+180°`, дистанция `1..50 м`,
 *   Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`,
 *   ошибки `401/403/404/413/429/5xx`, тишина/DTX `[0xF8, 0xFF, 0xFE]`, прогрев `HTTP 202`, права доступа,
 *   деградация `SofaHrtf -> KemarBinaural -> StereoPanning -> MonoBypass`, маскирование токенов `mfa.***`).
 * - SUB-986-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/chord-voice/src/spatial_audio.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const NATIVE_RE_MD_PATH = join(ROOT, "docs", "re", "native-re.md");
const NATIVE_RE_JSON_PATH = join(ROOT, "docs", "re", "native-re.json");
const CRATE_RECEPTION_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "chord-voice",
  "src",
  "reception.rs",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-voice/src/spatial_audio.rs` через `rustc --test --edition 2024 -D warnings`.
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
      "-C",
      "opt-level=2",
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
 * Независимый физико-математический оракул расчёта бинауральных параметров (ISO KEMAR + формула Вудворта):
 * - Радиус головы KEMAR `a = 0.0875 м`, скорость звука `c = 343.0 м/с`
 * - `sin(theta) = sin(azimuth) * cos(elevation)`
 * - `ITD (сек) = (a / c) * (|theta| + |sin(theta)|) * sign(theta)`
 * - `ILD (дБ) = 20.0 * sin(theta)`
 * - Затухание дистанции `1 / max(distance_m, 1.0)` (с клампом до `50.0 м`)
 * @param {number} azimuthDeg
 * @param {number} elevationDeg
 * @param {number} distanceM
 */
function oracleComputeBinauralCues(azimuthDeg, elevationDeg, distanceM) {
  const headRadiusM = 0.0875;
  const speedOfSoundMps = 343.0;
  const maxIldDb = 20.0;
  const sampleRateHz = 48_000;

  let normAz = ((azimuthDeg + 180) % 360 + 360) % 360 - 180;
  if (normAz === -180 && azimuthDeg > 0) {
    normAz = 180;
  }
  const clampedEl = Math.max(-90, Math.min(90, elevationDeg));
  const clampedDist = Math.max(1.0, Math.min(50.0, distanceM));

  const azRad = (normAz * Math.PI) / 180;
  const elRad = (clampedEl * Math.PI) / 180;
  const sinTheta = Math.max(-1, Math.min(1, Math.sin(azRad) * Math.cos(elRad)));
  const absTheta = Math.asin(Math.abs(sinTheta));
  const sign = sinTheta >= 0 ? 1 : -1;

  const itdSec = sign * (headRadiusM / speedOfSoundMps) * (absTheta + Math.abs(sinTheta));
  const itdMs = itdSec * 1_000;
  const itdSamples = itdSec * sampleRateHz;
  const ildDb = maxIldDb * sinTheta;
  const distanceGain = 1.0 / clampedDist;

  const shadowGain = Math.pow(10, -Math.abs(ildDb) / 20);
  const leftGain = (ildDb > 0 ? shadowGain : 1.0) * distanceGain;
  const rightGain = (ildDb < 0 ? shadowGain : 1.0) * distanceGain;

  return {
    itdMs,
    itdSamples,
    ildDb,
    distanceGain,
    leftGain,
    rightGain,
  };
}

describe("TASK-986: Пространственное 3D-позиционирование звука участников (HRTF) в rust/chord-voice/src/spatial_audio.rs", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "spatial-audio-hrtf-986-"));
    testBinPath = join(workDir, "spatial_audio_986_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-986-0: модуль rust/chord-voice/src/spatial_audio.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты размера (<50 строк на fn, <800 на файл) и компилируется без предупреждений", () => {
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
      "В продакшен-коде модуля spatial_audio.rs запрещены .unwrap() и .expect()",
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
      "В исходнике spatial_audio.rs не должно быть незамаскированных секретов",
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

  it("SUB-986-1: сверяет константы Steam Audio (Phonon), пути default.sofa/api_hrtf.cpp/api_binaural_effect.cpp, NAPI-символы и состояния peer-avatar-3d с независимыми RE-оракулами", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-986 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card986 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-986");
    assert.ok(card986, "Карточка TASK-986 должна присутствовать в waves/kanban-board.json");
    assert.equal(card986.department, "G5 Voice / Platform");
    assert.equal(card986.layout.container, "Spatial Audio Canvas");
    assert.equal(card986.layout.dimensions, "360 deg");
    assert.deepEqual(card986.interactive_states[0], {
      element: "peer-avatar-3d",
      default: "center",
      hover: "drag position",
      active: "panned",
    });

    // 2. Независимый оракул: docs/re/native-re.json (discord_voice.node + Steam Audio + NAPI)
    const nativeReJson = JSON.parse(readFileSync(NATIVE_RE_JSON_PATH, "utf8"));
    const voiceMod = (nativeReJson.modules ?? []).find(
      (/** @type {any} */ m) => m.name === "discord_voice",
    );
    assert.ok(voiceMod, "Запись discord_voice должна присутствовать в docs/re/native-re.json");
    assert.equal(voiceMod.size, 126_136_024);
    assert.equal(voiceMod.md5, "5fe80ce0f5d89fd53d30b2fb25a6cdac");
    assert.equal(
      voiceMod.sha256,
      "96721305d59a4fdcecfa4eec48daabc50dba96c9213e5d3b514e9690d3b208ca",
    );

    const requiredSteamAudioPaths = [
      "/home/discord/discord/discord_common/native/third_party/steam_audio/build/linux-x64-Release/install/mysofa/share/libmysofa/default.sofa",
      "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/api_binaural_effect.cpp",
      "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/api_hrtf.cpp",
      "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/binaural_effect.cpp",
      "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/distance_attenuation.cpp",
      "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/overlap_save_convolution_effect.cpp",
      "/home/discord/discord/discord_common/native/third_party/steam_audio_src/steam_audio/core/src/core/sofa_hrtf_map.cpp",
    ];
    for (const p of requiredSteamAudioPaths) {
      assert.ok(
        voiceMod.strings.paths.includes(p),
        `Путь Steam Audio ${p} должен присутствовать в оракуле docs/re/native-re.json`,
      );
    }
    assert.ok(
      voiceMod.strings.events.includes("SetSpatialAudioStatusCallback"),
      "Событие SetSpatialAudioStatusCallback должно быть в docs/re/native-re.json",
    );

    // 3. Независимый оракул: docs/re/native-re.md и rust/crates/chord-voice/src/reception.rs
    const nativeReMd = readFileSync(NATIVE_RE_MD_PATH, "utf8");
    assert.match(nativeReMd, /src\/media\/spatial_mixer\.cpp/);
    assert.match(nativeReMd, /spatial_audio_options\.cpp/);
    assert.match(nativeReMd, /VoiceEngine\.isSpatialAudioEnabled\(\)/);
    assert.match(nativeReMd, /setSpatialAudioStatusCallback/);

    const receptionSrc = readFileSync(CRATE_RECEPTION_RS_PATH, "utf8");
    assert.match(receptionSrc, /pub const OPUS_CLOCK_RATE: u32 = 48_000;/);
    assert.match(receptionSrc, /pub const PCM_FRAME_SAMPLES: usize = 960;/);
    assert.match(receptionSrc, /pub const MAX_MIXER_VOLUME: f32 = 2\.0;/);
    assert.match(receptionSrc, /pub const MAX_MIXER_PARTICIPANTS: usize = 64;/);

    // 4. Проверяем фиксацию дизайн-токенов, состояний и RE-констант в spatial_audio.rs
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card986.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} из TASK-986 должен быть зафиксирован в spatial_audio.rs`,
      );
    }
    for (const stateLabel of ["peer-avatar-3d", "center", "drag position", "panned"]) {
      assert.ok(
        source.includes(stateLabel),
        `Интерактивное состояние ${stateLabel} должно быть зафиксировано в spatial_audio.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_986_1_");
    assert.equal(run.status, 0, `Тесты SUB-986-1 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_986_1_.*ok/);
  });

  it("SUB-986-2: сверяет расчёт межушной разности времени (ITD до 0.7 мс по Вудворту) и уровня (ILD до 20 дБ по KEMAR) и затухания 1/r с независимым математическим оракулом", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_986_2_");
    assert.equal(run.status, 0, `Тесты SUB-986-2 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_986_2_.*ok/);

    const markerLine = run.stdout
      .split("\n")
      .find((line) => line.startsWith("SPATIAL_CUES_VECTOR_JSON:"));
    assert.ok(
      markerLine,
      "Тест SUB-986-2 должен вывести контрольный вектор SPATIAL_CUES_VECTOR_JSON",
    );
    const rustVectors = JSON.parse(
      markerLine.slice("SPATIAL_CUES_VECTOR_JSON:".length),
    );
    assert.ok(Array.isArray(rustVectors) && rustVectors.length >= 7);

    for (const item of rustVectors) {
      const oracle = oracleComputeBinauralCues(
        item.azimuth_deg,
        item.elevation_deg,
        item.distance_m,
      );
      assert.ok(
        Math.abs(item.itd_ms - oracle.itdMs) < 1e-4,
        `Несовпадение ITD (мс) при az=${item.azimuth_deg}: rust=${item.itd_ms}, oracle=${oracle.itdMs}`,
      );
      assert.ok(
        Math.abs(item.itd_samples - oracle.itdSamples) < 1e-3,
        `Несовпадение ITD (сэмплы) при az=${item.azimuth_deg}: rust=${item.itd_samples}, oracle=${oracle.itdSamples}`,
      );
      assert.ok(
        Math.abs(item.ild_db - oracle.ildDb) < 1e-4,
        `Несовпадение ILD (дБ) при az=${item.azimuth_deg}: rust=${item.ild_db}, oracle=${oracle.ildDb}`,
      );
      assert.ok(
        Math.abs(item.distance_gain - oracle.distanceGain) < 1e-5,
        `Несовпадение 1/r затухания при dist=${item.distance_m}: rust=${item.distance_gain}, oracle=${oracle.distanceGain}`,
      );
      assert.ok(
        Math.abs(item.left_gain - oracle.leftGain) < 1e-4,
        `Несовпадение усиления левого канала при az=${item.azimuth_deg}`,
      );
      assert.ok(
        Math.abs(item.right_gain - oracle.rightGain) < 1e-4,
        `Несовпадение усиления правого канала при az=${item.azimuth_deg}`,
      );
    }
  });

  it("SUB-986-3: выполняет бинауральную HRTF-свёртку (Overlap-Save FIR Convolver + дробная задержка ITD) моно-потока 48 кГц (960 сэмплов) в стерео без щелчков на стыках кадров", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "HRIR_TAP_COUNT",
      "MAX_ITD_DELAY_SAMPLES",
      "HrtfBinauralConvolver",
      "StereoFrame",
      "convolve_mono_frame",
      "synthesize_kemar_hrir",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле spatial_audio.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_986_3_");
    assert.equal(run.status, 0, `Тесты SUB-986-3 (HRTF convolver) упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_986_3_.*ok/);
  });

  it("SUB-986-4: подтверждает нагрузку CPU < 2% на 10 участников и покрывает все 9 измерений TEST-POLICY.md (граничные, ошибки 401..5xx, тишина/DTX, прогрев 202, права, fallback и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "MAX_SPATIAL_CPU_LOAD_PCT",
      "BENCHMARK_PARTICIPANTS_COUNT",
      "SpatialAudioMixer",
      "benchmark_ten_participants_cpu",
      "validate_spatial_cpu_budget",
      "validate_spatial_workload_boundaries",
      "compute_spatial_rest_backoff_ms",
      "redact_spatial_secrets",
      "export_spatial_telemetry_json",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле spatial_audio.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_986_4_");
    assert.equal(run.status, 0, `Тесты SUB-986-4 (CPU < 2% & 9 измерений) упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_986_4_.*ok/);

    const benchLine = run.stdout
      .split("\n")
      .find((line) => line.startsWith("SPATIAL_BENCH_10_PEERS_JSON:"));
    assert.ok(benchLine, "Тест SUB-986-4 должен вывести метрику SPATIAL_BENCH_10_PEERS_JSON");
    const benchMetrics = JSON.parse(
      benchLine.slice("SPATIAL_BENCH_10_PEERS_JSON:".length),
    );
    assert.equal(benchMetrics.participants, 10);
    assert.ok(
      benchMetrics.cpu_load_pct > 0 && benchMetrics.cpu_load_pct < 2.0,
      `Нагрузка CPU на 10 участников должна быть < 2.0%, получено: ${benchMetrics.cpu_load_pct}%`,
    );
  });

  it("SUB-986-5: убивает 5/5 контрольных мутаций модуля rust/chord-voice/src/spatial_audio.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-spatial-01",
        operator: "WoodworthItdFormulaCorrupted",
        from: "let itd_sec = sign * head_delay * (abs_theta + sin_theta.abs());",
        to: "let itd_sec = 0.0_f32 * sign * head_delay * (abs_theta + sin_theta.abs());",
      },
      {
        id: "MUT-spatial-02",
        operator: "KemarMaxIldAttenuationZeroed",
        from: "let ild_db = MAX_ILD_DB * sin_theta;",
        to: "let ild_db = 0.0_f32 * sin_theta;",
      },
      {
        id: "MUT-spatial-03",
        operator: "InverseDistanceAttenuationBypass",
        from: "let distance_gain = REFERENCE_DISTANCE_M / clamped_dist;",
        to: "let distance_gain = 1.0_f32 + 0.0_f32 * clamped_dist;",
      },
      {
        id: "MUT-spatial-04",
        operator: "MaxCpuLoadBudgetRelaxed",
        from: "pub const MAX_SPATIAL_CPU_LOAD_PCT: f32 = 2.0;",
        to: "pub const MAX_SPATIAL_CPU_LOAD_PCT: f32 = 20.0;",
      },
      {
        id: "MUT-spatial-05",
        operator: "CpuBudgetStrictInequalityRelaxed",
        from: "if cpu_load_pct <= 0.0 || cpu_load_pct >= MAX_SPATIAL_CPU_LOAD_PCT {",
        to: "if cpu_load_pct <= 0.0 || cpu_load_pct > MAX_SPATIAL_CPU_LOAD_PCT {",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "spatial_audio.rs",
        operator: spec.operator,
        mutatedSource: source.replace(spec.from, spec.to),
      };
    });

    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");

      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      assert.equal(
        compiled.status,
        0,
        `Мутант ${mutant.id} должен успешно компилироваться без предупреждений:\n${compiled.stderr}`,
      );
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
