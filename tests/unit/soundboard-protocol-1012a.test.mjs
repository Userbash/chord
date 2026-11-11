// @ts-check
/**
 * Контрактные и модульные тесты протокола Voice Gateway Opcode 18 (`SOUNDBOARD_SOUND`),
 * приёма Opcode 19 (`SPEAKING` soundboard) и кэша аудиосэмплов `48kHz stereo` (TASK-1012A):
 * - SUB-1012A-0: Статический и компиляционный контракт `rust/crates/discord-voice/src/soundboard_protocol.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1012A-1: Сверка карточки `TASK-1012A` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Soundboard Protocol`
 *   (`Voice WS`, `padding: 0px`, `spacing: 0px`), интерактивных состояний `sfx-trigger`
 *   (`idle` -> `preview` -> `playing sfx`) и независимых RE-оракулов (`/home/sanya/re-deep/voice-deep.md`,
 *   `docs/re/voice-deep.md`, `docs/re/gateway-protocol.json`, `docs/re/rest-d02.json`,
 *   `/home/sanya/re-deep/rest-static.json`).
 * - SUB-1012A-2: Протокол Voice WS: формирование исходящего кадра Opcode 18 (`SOUNDBOARD_SOUND`:
 *   `sound_id`, `volume`, `source_guild_id`, `emoji_id`, `emoji_name`, `ssrc`), парсинг входящего
 *   кадра Opcode 19 / Opcode 5 (`SPEAKING` с битовой маской `MICROPHONE = 1 << 0`, `SOUND_SHARE = 1 << 1`,
 *   `PRIORITY = 1 << 2`), построение REST-запроса `POST /api/v9/channels/{id}/send-soundboard-sound`
 *   и генерация CDN URL `https://cdn.discordapp.com/soundboard-sounds/{sound_id}`.
 * - SUB-1012A-3: Локальный и дисковый кэш аудиосэмплов `SoundboardSampleCache` в формате Opus `48kHz stereo`
 *   (`48_000 Hz`, `2` канала, `960` сэмплов/канал на `20 мс` = `1_920` stereo float отсчётов,
 *   лимит `524_288` Б на звук, сохранение `.opus` на диск, контроль целостности FNV-1a64,
 *   защита от Path Traversal и LRU-вытеснение по бюджету байт).
 * - SUB-1012A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`,
 *   громкость `0..200%`, бейдж `99+`, избранное `250`, имя `2..=32` симв.; ошибки HTTP
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустые; `skeleton`/`HTTP 202`; права
 *   `CONNECT`/`SPEAK`/`USE_SOUNDBOARD = 1 << 42`/`USE_EXTERNAL_SOUNDS = 1 << 45`/`AGE_GATED_SPACES`;
 *   самовосстановление при повреждении дискового кэша; маскирование токенов в логах).
 * - SUB-1012A-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-voice/src/soundboard_protocol.rs`.
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
const RUST_MODULE_REL_PATH =
  "rust/crates/discord-voice/src/soundboard_protocol.rs";
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
const DOCS_REST_D02_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const RE_DEEP_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-voice/src/soundboard_protocol.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1012A: Voice Gateway Opcode 18 (SOUNDBOARD_SOUND), прием Opcode 19 (SPEAKING soundboard) и кэш сэмплов 48kHz stereo", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "soundboard-protocol-1012a-"));
    testBinPath = join(workDir, "soundboard_protocol_1012a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1012A-0: проверяет статический контракт rust/crates/discord-voice/src/soundboard_protocol.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
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
      "В продуктовом коде soundboard_protocol.rs запрещены .unwrap() и .expect()",
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

  it("SUB-1012A-1: сверяет карточку TASK-1012A в waves/kanban-board.json, дизайн-токены, геометрию Soundboard Protocol (Voice WS), состояния sfx-trigger и RE-оракул voice-deep.md", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1012A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1012A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1012A",
    );
    assert.ok(
      card1012A,
      "Карточка TASK-1012A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1012A.department, "G5 Voice / Platform");
    assert.deepEqual(card1012A.layout, {
      container: "Soundboard Protocol",
      dimensions: "Voice WS",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1012A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1012A.interactive_states[0], {
      element: "sfx-trigger",
      default: "idle",
      hover: "preview",
      active: "playing sfx",
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

    // 3. Оракул: docs/re/gateway-protocol.json (SOUNDBOARD_SOUNDS = 21)
    const gwProto = JSON.parse(readFileSync(DOCS_GATEWAY_PROTO_PATH, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.SOUNDBOARD_SOUNDS, 21);

    // 4. Оракул: docs/re/rest-d02.json и /home/sanya/re-deep/rest-static.json
    const restD02Raw = readFileSync(DOCS_REST_D02_PATH, "utf8");
    assert.ok(restD02Raw.includes("/api/v9/soundboard-default-sounds"));
    if (existsSync(RE_DEEP_REST_STATIC_PATH)) {
      const restStaticRaw = readFileSync(RE_DEEP_REST_STATIC_PATH, "utf8");
      assert.ok(restStaticRaw.includes("SEND_SOUNDBOARD_SOUND"));
      assert.ok(restStaticRaw.includes("/channels/{arg}/send-soundboard-sound"));
    }

    // 5. Сверка констант в исходнике soundboard_protocol.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1012A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1012A должен присутствовать в soundboard_protocol.rs`,
      );
    }
    assert.ok(rustSource.includes("Soundboard Protocol"));
    assert.ok(rustSource.includes("Voice WS"));
    assert.ok(rustSource.includes("sfx-trigger"));
    assert.ok(rustSource.includes("~/.cache/discord-gpui/soundboard/"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1012a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012a_1_.*ok/);
  });

  it("SUB-1012A-2: проверяет отправку Voice Opcode 18 (SOUNDBOARD_SOUND), прием Opcode 19 (SPEAKING soundboard), REST send-soundboard-sound и CDN URL", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "VOICE_OP_SOUNDBOARD_SOUND",
      "VOICE_OP_SPEAKING_SOUNDBOARD",
      "SPEAKING_FLAG_MICROPHONE",
      "SPEAKING_FLAG_SOUND_SHARE",
      "SPEAKING_FLAG_PRIORITY",
      "VoiceOp18SoundboardPayload",
      "VoiceOp19SpeakingSoundboardEvent",
      "SoundboardRestEndpointRequest",
      "build_send_soundboard_sound_rest",
      "build_soundboard_cdn_url",
      "clamp_soundboard_volume",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_protocol.rs должен присутствовать символ ${symbol}`,
      );
    }
    assert.ok(
      rustSource.includes("https://cdn.discordapp.com/soundboard-sounds/"),
      "Базовый префикс CDN саундборда должен присутствовать в soundboard_protocol.rs",
    );
    assert.ok(
      rustSource.includes("/send-soundboard-sound"),
      "Маршрут /send-soundboard-sound должен присутствовать в soundboard_protocol.rs",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1012a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012a_2_.*ok/);
  });

  it("SUB-1012A-3: проверяет кэш аудиосэмплов SoundboardSampleCache в формате Opus 48kHz stereo (ОЗУ + диск .opus, FNV-1a64, защита от Path Traversal и LRU-вытеснение)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "SAMPLE_RATE_48KHZ",
      "CHANNELS_STEREO",
      "SAMPLES_PER_CHANNEL_20MS",
      "INTERLEAVED_STEREO_SAMPLES_20MS",
      "MAX_SOUNDBOARD_SOUND_BYTES",
      "Stereo48kSampleEntry",
      "SoundboardSampleCache",
      "validate_cache_key",
      "fnv1a64_checksum",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_protocol.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012a_3_.*ok/);
  });

  it("SUB-1012A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права USE_SOUNDBOARD/USE_EXTERNAL_SOUNDS/AGE_GATED_SPACES, самовосстановление при порче диска и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "evaluate_soundboard_permissions",
      "validate_platform_boundaries",
      "classify_soundboard_http_status",
      "format_badge_cap",
      "redact_soundboard_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В soundboard_protocol.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1012a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1012A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1012a_4_.*ok/);
  });

  it("SUB-1012A-5: убивает 5/5 контрольных мутаций в rust/crates/discord-voice/src/soundboard_protocol.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль soundboard_protocol.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1012A-RS-01",
        moduleName: "soundboard_protocol",
        operator: "VoiceOp18ConstantTamper",
        from: "pub const VOICE_OP_SOUNDBOARD_SOUND: u8 = 18;",
        to: "pub const VOICE_OP_SOUNDBOARD_SOUND: u8 = 98;",
      },
      {
        id: "MUT-1012A-RS-02",
        moduleName: "soundboard_protocol",
        operator: "SpeakingSoundShareBitTamper",
        from: "pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 1;",
        to: "pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 6;",
      },
      {
        id: "MUT-1012A-RS-03",
        moduleName: "soundboard_protocol",
        operator: "SampleRate48kHzGuardTamper",
        from: "if sample_rate_hz != SAMPLE_RATE_48KHZ || channels != CHANNELS_STEREO {",
        to: "if false && (sample_rate_hz != SAMPLE_RATE_48KHZ || channels != CHANNELS_STEREO) {",
      },
      {
        id: "MUT-1012A-RS-04",
        moduleName: "soundboard_protocol",
        operator: "LruByteEvictionLoopBypass",
        from: "while self.used_bytes.saturating_add(needed_bytes) > self.max_bytes",
        to: "while false && self.used_bytes.saturating_add(needed_bytes) > self.max_bytes",
      },
      {
        id: "MUT-1012A-RS-05",
        moduleName: "soundboard_protocol",
        operator: "UseSoundboardPermissionCheckBypass",
        from: "if !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
        to: "if false && !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в soundboard_protocol.rs: ${m.from}`,
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
      "MD5 исходного файла soundboard_protocol.rs не должен измениться после мутационного гейта",
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
