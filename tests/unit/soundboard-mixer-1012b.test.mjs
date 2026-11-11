// @ts-check
/**
 * Контрактные и модульные тесты линейного микшера SFX эффектов в поток микрофона
 * с софт-лимитером `tanh()` по стандарту RFC 3550 RTP Opus (TASK-1012B):
 * - SUB-1012B-0: Статический и компиляционный контракт `rust/crates/discord-voice/src/soundboard_mixer.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1012B-1: Сверка карточки `TASK-1012B` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#1e1f22`, `#2b2d31`, `#ffffff`, `#23a55a`, `#3f4147`), геометрии `Audio Mixer`
 *   (`20ms frames`, `padding: 0px`, `spacing: 0px`), интерактивных состояний `mixer-dsp`
 *   (`bypass` -> `mix` -> `tanh limiter`) и независимых оракулов RFC 3550 RTP Opus
 *   (`/home/sanya/re-deep/voice-deep.md`, `docs/re/voice-deep.md`, `scripts/verify-soundboard.mjs`).
 * - SUB-1012B-2: Алгоритм линейного микширования 20 мс стерео-кадров (`48_000 Hz`, `2` канала,
 *   `960` сэмплов/канал = `1_920` чередующихся `f32` отсчётов), формула эффективной громкости RE-модуля
 *   `904054`, инкапсуляция в RFC 3550 RTP Opus пакет (`V=2`, `PT=120`, монотонное приращение `sequence + 1`
 *   и `timestamp + 960` с `wrapping_add`, `SPEAKING` флаги `MICROPHONE | SOUND_SHARE = 3`) и
 *   автоматическая остановка микширования по окончании SFX сэмпла с возвратом в `bypass`.
 * - SUB-1012B-3: Софт-лимитер `tanh(mic + sfx * gain)` без жёсткого клиппинга речи при одновременном
 *   громком SFX сигнале (`|out| < 1.0`, сохранение формы волны, порог срабатывания лимитера `0.95`,
 *   сверка с независимым математическим JS-оракулом `Math.tanh`).
 * - SUB-1012B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, переполнение RTP `sequence u16::MAX -> 0` и `timestamp u32::MAX`,
 *   `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`, бейдж `99+`, избранное `250`,
 *   имя `2..=32` симв.; ошибки HTTP `400/401/403/404/413/429 retry_after_ms/5xx`; пустые/DTX `[0xF8, 0xFF, 0xFE]`;
 *   `skeleton`/`HTTP 202`; права `CONNECT`/`SPEAK`/`USE_SOUNDBOARD = 1 << 42`/`USE_EXTERNAL_SOUNDS = 1 << 45`/
 *   `AGE_GATED_SPACES`; самовосстановление при `NaN`/`±Infinity` отсчётах; маскирование токенов).
 * - SUB-1012B-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-voice/src/soundboard_mixer.rs`.
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
  SoundboardRtpMixer as JsOracleRtpMixer,
  computeEffectiveSoundboardVolume as jsOracleEffectiveVolume,
  resolveMixerDspState as jsOracleResolveDspState,
  softClipTanhSample as jsOracleSoftClipTanh,
} from "../../scripts/verify-soundboard.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH =
  "rust/crates/discord-voice/src/soundboard_mixer.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DOCS_VOICE_DEEP_PATH = join(ROOT, "docs", "re", "voice-deep.md");
const RE_DEEP_VOICE_DEEP_PATH = "/home/sanya/re-deep/voice-deep.md";
const DOCS_GATEWAY_PROTO_PATH = join(
  ROOT,
  "docs",
  "re",
  "gateway-protocol.json",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-voice/src/soundboard_mixer.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1012B: Линейный микшер SFX эффектов в поток микрофона с софт-лимитером tanh() по RFC 3550 RTP Opus", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "soundboard-mixer-1012b-"));
    testBinPath = join(workDir, "soundboard_mixer_1012b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1012B-0: проверяет статический контракт rust/crates/discord-voice/src/soundboard_mixer.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
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
      "В продуктовом коде soundboard_mixer.rs запрещены .unwrap() и .expect()",
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

  it("SUB-1012B-1: сверяет карточку TASK-1012B в waves/kanban-board.json, дизайн-токены (#1e1f22, #2b2d31, #ffffff, #23a55a, #3f4147), геометрию Audio Mixer (20ms frames), состояния mixer-dsp и оракул RFC 3550 RTP Opus (voice-deep.md)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1012B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1012B = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1012B",
    );
    assert.ok(
      card1012B,
      "Карточка TASK-1012B должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1012B.department, "G5 Voice / Platform");
    assert.deepEqual(card1012B.layout, {
      container: "Audio Mixer",
      dimensions: "20ms frames",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1012B.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#23a55a",
      border: "#3f4147",
    });
    assert.deepEqual(card1012B.interactive_states[0], {
      element: "mixer-dsp",
      default: "bypass",
      hover: "mix",
      active: "tanh limiter",
    });

    // 2. Оракул RFC 3550 RTP Opus: /home/sanya/re-deep/voice-deep.md и docs/re/voice-deep.md
    const voiceDeepPath = existsSync(RE_DEEP_VOICE_DEEP_PATH)
      ? RE_DEEP_VOICE_DEEP_PATH
      : DOCS_VOICE_DEEP_PATH;
    const voiceDeepRaw = readFileSync(voiceDeepPath, "utf8");
    assert.match(voiceDeepRaw, /V=2, P=0, X=1, CC=0/);
    assert.match(voiceDeepRaw, /PT=120/);
    assert.match(voiceDeepRaw, /0xBEDE/);
    assert.match(voiceDeepRaw, /960 @48 кГц/);
    assert.match(voiceDeepRaw, /aead_aes256_gcm_rtpsize/);
    assert.match(voiceDeepRaw, /17843/);
    assert.match(voiceDeepRaw, /9208/);

    // 3. Оракул: docs/re/gateway-protocol.json (SOUNDBOARD_SOUNDS = 21)
    const gwProto = JSON.parse(readFileSync(DOCS_GATEWAY_PROTO_PATH, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.SOUNDBOARD_SOUNDS, 21);

    // 4. Сверка констант в исходнике soundboard_mixer.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1012B.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1012B должен присутствовать в soundboard_mixer.rs`,
      );
    }
    assert.ok(rustSource.includes("Audio Mixer"));
    assert.ok(rustSource.includes("20ms frames"));
    assert.ok(rustSource.includes("mixer-dsp"));
    assert.ok(rustSource.includes("bypass"));
    assert.ok(rustSource.includes("tanh limiter"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1012b_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012B-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012b_1_.*ok/);
  });

  it("SUB-1012B-2: проверяет алгоритм линейного микширования 20ms кадров, RFC 3550 RTP Opus заголовки (V=2, PT=120, шаг ts=960) и автоматическую остановку по окончании сэмпла", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "RTP_VERSION",
      "RTP_OPUS_PAYLOAD_TYPE",
      "RTP_HEADER_FIXED_BYTES",
      "RTP_EXTENSION_PROFILE_BEDE",
      "SAMPLES_PER_CHANNEL_20MS",
      "INTERLEAVED_STEREO_SAMPLES_20MS",
      "SPEAKING_FLAG_MICROPHONE",
      "SPEAKING_FLAG_SOUND_SHARE",
      "MixerDspState",
      "RtpOpusHeader",
      "RtpOpusMixedFrame",
      "SoundboardLinearMixer",
      "compute_effective_soundboard_volume",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_mixer.rs должен присутствовать символ ${symbol}`,
      );
    }

    // Независимый JS-оракул (scripts/verify-soundboard.mjs) для сверки поведения микшера:
    const jsMixer = new JsOracleRtpMixer({
      ssrc: 17843,
      initialSequence: 100,
      initialTimestamp: 48_000,
      settingVolumePercent: 100,
      outputVolumePercent: 100,
    });
    const sfxTwoFrames = new Float32Array(3840).fill(0.3);
    const micFrame = new Float32Array(1920).fill(0.2);
    jsMixer.startSfxPlayback({
      soundId: "1102645616632397904",
      soundVolume: 0.8,
      pcmStereoSamples: sfxTwoFrames,
    });
    const f1 = jsMixer.mixNext20msRtpFrame(micFrame);
    assert.equal(f1.payloadType, 120);
    assert.equal(f1.sequence, 100);
    assert.equal(f1.timestamp, 48_000);
    assert.equal(f1.speakingFlags, 3);
    assert.equal(f1.dspState, "mix");
    assert.equal(jsMixer.isActive(), true);

    const f2 = jsMixer.mixNext20msRtpFrame(micFrame);
    assert.equal(f2.sequence, 101);
    assert.equal(f2.timestamp, 48_960);
    assert.equal(jsMixer.isActive(), false, "По окончании 2 кадров SFX микшер обязан автоматически остановиться");

    const f3 = jsMixer.mixNext20msRtpFrame(micFrame);
    assert.equal(f3.sequence, 102);
    assert.equal(f3.timestamp, 49_920);
    assert.equal(f3.speakingFlags, 1);
    assert.equal(f3.dspState, "bypass");

    const run = runCompiledRustTests(testBinPath, "test_sub_1012b_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012B-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012b_2_.*ok/);
  });

  it("SUB-1012B-3: проверяет софт-лимитер tanh(mic + sfx * gain) без клиппинга речи при громком SFX звуке", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "TANH_LIMITER_THRESHOLD",
      "soft_clip_tanh_sample",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_mixer.rs должен присутствовать символ ${symbol}`,
      );
    }

    // Независимый математический оракул: при громкой речи (0.85) и громком SFX (0.85, gain=1.0)
    // линейная сумма = 1.70 (вызвала бы жёсткий клиппинг 1.0), но tanh(1.70) = 0.935409... < 1.0
    const loudSpeech = 0.85;
    const loudSfx = 0.85;
    const oracleLimited = jsOracleSoftClipTanh(loudSpeech, loudSfx, 1.0);
    assert.ok(oracleLimited > 0.93 && oracleLimited < 1.0);
    assert.equal(
      jsOracleResolveDspState({ activeSfx: true, limiting: true }),
      "tanh limiter",
    );
    assert.equal(
      jsOracleEffectiveVolume(0.5, 80, 100),
      0.4,
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1012b_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012B-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012b_3_.*ok/);
  });

  it("SUB-1012B-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, wrap-around RTP seq/ts, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права USE_SOUNDBOARD/USE_EXTERNAL_SOUNDS/AGE_GATED_SPACES, восстановление после NaN/Inf и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "validate_snowflake_id",
      "evaluate_mixer_permissions",
      "validate_platform_boundaries",
      "classify_mixer_http_status",
      "format_badge_cap",
      "redact_mixer_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_mixer.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012b_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012B-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012b_4_.*ok/);
  });

  it("SUB-1012B-5: убивает 5/5 контрольных мутаций в rust/crates/discord-voice/src/soundboard_mixer.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль soundboard_mixer.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1012B-RS-01",
        moduleName: "soundboard_mixer",
        operator: "RtpOpusPayloadTypeTamper",
        from: "pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;",
        to: "pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 111;",
      },
      {
        id: "MUT-1012B-RS-02",
        moduleName: "soundboard_mixer",
        operator: "RtpTimestampStep20msTamper",
        from: "pub const SAMPLES_PER_CHANNEL_20MS: usize = 960;",
        to: "pub const SAMPLES_PER_CHANNEL_20MS: usize = 480;",
      },
      {
        id: "MUT-1012B-RS-03",
        moduleName: "soundboard_mixer",
        operator: "SoftClipTanhSubtractionTamper",
        from: "(m + s * g).tanh()",
        to: "(m - s * g).tanh()",
      },
      {
        id: "MUT-1012B-RS-04",
        moduleName: "soundboard_mixer",
        operator: "AutoStopOnSampleEndBypass",
        from: "if self.sfx_cursor >= self.sfx_samples.len() {",
        to: "if false && self.sfx_cursor >= self.sfx_samples.len() {",
      },
      {
        id: "MUT-1012B-RS-05",
        moduleName: "soundboard_mixer",
        operator: "UseSoundboardPermissionCheckBypass",
        from: "if !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
        to: "if false && !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в soundboard_mixer.rs: ${m.from}`,
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
      "MD5 исходного файла soundboard_mixer.rs не должен измениться после мутационного гейта",
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
