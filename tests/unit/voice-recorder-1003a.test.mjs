// @ts-check
/**
 * Контрактные и модульные тесты аудио-рекордера голосовых сообщений и вычисления Waveform (TASK-1003A):
 * - SUB-1003A-0: Статический и компиляционный контракт `rust/crates/discord-voice/src/voice_message_recorder.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на модуль, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1003A-1: Сверка констант OGG Opus (`48kHz mono 32kbps VBR`), флага `IS_VOICE_MESSAGE = 8192`,
 *   дизайн-токенов карточки `TASK-1003A` (`#1e1f22`, `#2b2d31`, `#ffffff`, `#23a55a`, `#3f4147`),
 *   геометрии `Audio Recorder` (`48kHz mono buffer`, `padding: 0px`, `spacing: 0px`), интерактивных
 *   состояний `mic-record` (`idle -> hover mic -> recording audio`) и живого вложения `voice-message.ogg`
 *   с независимыми RE-оракулами (`waves/kanban-board.json`, `/home/sanya/re-deep/rest-d02.json`,
 *   `docs/re/rest-d02.json`, `waves/RW-40/ATTACH-RENDER-RE.md`, `waves/RW-40/ATTACH-RE.md`).
 * - SUB-1003A-2: Буфер захвата `cpal` (`CpalVoiceRecorderBuffer`, 48 кГц mono, кадр 20 мс = 960 сэмплов),
 *   расчёт среднеквадратичной амплитуды (RMS) на 20 мс кадр (`compute_frame_rms_20ms`), кодирование
 *   в контейнер OGG Opus 32 kbps VBR (`OggS`, `OpusHead`, `OpusTags`, CRC-32, DTX тишина `[0xF8, 0xFF, 0xFE]`)
 *   и точная синхронизация длительности записи с количеством сэмплов и granule position.
 * - SUB-1003A-3: Нормализация амплитуд RMS в диапазон `0..=255` (`256` байт) и экспорт в Base64 строку
 *   (`344` символа) для REST, а также построение запросов загрузки вложения и отправки сообщения
 *   (`POST /channels/{id}/attachments` -> `PUT` -> `POST /channels/{id}/messages` с `flags: 8192`
 *   и заголовком `audio/ogg; codecs=opus`).
 * - SUB-1003A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   `48_000 Hz`, `960` сэмплов, `256` байт waveform, `0.5..=1200.0 с`, Snowflake `0`/`u64::MAX`,
 *   `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустые/тишина DTX; прогрев `HTTP 202`; права
 *   `VIEW_CHANNEL`/`SEND_MESSAGES`/`ATTACH_FILES`/`SEND_VOICE_MESSAGES`/`AGE_GATED_SPACES`;
 *   восстановление при отключении микрофона `cpal`; маскирование токенов `mfa.***`).
 * - SUB-1003A-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`
 *   (ошибочный `sample_rate`, нарушение нормализации `u8`, рассинхронизация длительности, подмена `flags: 8192`,
 *   искажение битрейта Opus 32 kbps).
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
const RECORDER_MOD_REL_PATH =
  "rust/crates/discord-voice/src/voice_message_recorder.rs";
const RECORDER_MOD_PATH = join(ROOT, RECORDER_MOD_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const RE_DEEP_REST_D02_PATH = "/home/sanya/re-deep/rest-d02.json";
const ATTACH_RENDER_RE_PATH = join(
  ROOT,
  "waves",
  "RW-40",
  "ATTACH-RENDER-RE.md",
);
const ATTACH_RE_PATH = join(ROOT, "waves", "RW-40", "ATTACH-RE.md");

const LIVE_VOICE_ATTACHMENT_ORACLE = Object.freeze({
  messageId: "1359580913880662296",
  channelId: "1344356603997978745",
  attachmentId: "1359580913469751459",
  filename: "voice-message.ogg",
  size: 26921,
  durationSecs: 8.119976997375488,
  waveform:
    "AAAAAAAAAAAAABwnJBArEiAnJQAAACQTDBkAEg4DJycgIBsDDgMBAQAADBsOExkDIBUeDhwOGxAQEwcSEgUAAAAAAAAAAAAAAAAAAAAAAAAA",
  contentType: "audio/ogg",
  mimeCodec: "audio/ogg; codecs=opus",
  flags: 8192,
});

/** @type {string} */
let workDir = "";
/** @type {string} */
let recorderTestBinPath = "";

/**
 * Компилирует Rust-модуль через `rustc --test --edition 2024 -D warnings`.
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
 * Независимый JS-оракул расчёта 256-байтного нормализованного waveform (`0..=255`) и его Base64-представления
 * по массиву PCM-сэмплов (48 кГц моно, кадры по 20 мс = 960 сэмплов).
 * @param {Float32Array} pcmSamples
 * @returns {{ frameRms: number[], waveformBytes: Uint8Array, waveformBase64: string }}
 */
function oracleComputeWaveform256(pcmSamples) {
  const frameSize = 960;
  /** @type {number[]} */
  const frameRms = [];
  for (let offset = 0; offset < pcmSamples.length; offset += frameSize) {
    const end = Math.min(pcmSamples.length, offset + frameSize);
    const count = end - offset;
    if (count === 0) break;
    let sumSq = 0;
    for (let i = offset; i < end; i += 1) {
      const s = Math.max(-1.0, Math.min(1.0, pcmSamples[i] ?? 0));
      sumSq += s * s;
    }
    frameRms.push(Math.sqrt(sumSq / count));
  }

  const waveformBytes = new Uint8Array(256);
  if (frameRms.length === 0) {
    return {
      frameRms,
      waveformBytes,
      waveformBase64: Buffer.from(waveformBytes).toString("base64"),
    };
  }

  let peakRms = 0;
  for (const r of frameRms) {
    if (r > peakRms) peakRms = r;
  }

  if (peakRms <= 1e-7) {
    return {
      frameRms,
      waveformBytes,
      waveformBase64: Buffer.from(waveformBytes).toString("base64"),
    };
  }

  for (let i = 0; i < 256; i += 1) {
    const start = Math.floor((i * frameRms.length) / 256);
    const end = Math.max(
      start + 1,
      Math.floor(((i + 1) * frameRms.length) / 256),
    );
    let sum = 0;
    let cnt = 0;
    for (let j = start; j < end && j < frameRms.length; j += 1) {
      sum += frameRms[j] ?? 0;
      cnt += 1;
    }
    const avg = cnt > 0 ? sum / cnt : 0;
    const norm = Math.round((avg / peakRms) * 255);
    waveformBytes[i] = Math.max(0, Math.min(255, norm));
  }

  return {
    frameRms,
    waveformBytes,
    waveformBase64: Buffer.from(waveformBytes).toString("base64"),
  };
}

describe("TASK-1003A: Аудио-рекордер голосовых сообщений 48kHz mono в OGG Opus 32 kbps и RMS-генерация 256 байт base64 формы волны", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "voice-recorder-1003a-"));
    recorderTestBinPath = join(workDir, "voice_recorder_1003a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1003A-0: модуль voice_message_recorder.rs существует, запрещает unsafe, соблюдает лимиты (<50 строк/fn, <800 строк/файл), проходит rustfmt 2024 и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(RECORDER_MOD_PATH),
      `Модуль ${RECORDER_MOD_REL_PATH} должен существовать`,
    );

    const source = readFileSync(RECORDER_MOD_PATH, "utf8");
    assert.match(
      source,
      /^#!\[forbid\(unsafe_code\)\]/m,
      `Модуль ${RECORDER_MOD_REL_PATH} обязан содержать #![forbid(unsafe_code)]`,
    );

    const prodSection = source.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      `В продакшен-коде модуля ${RECORDER_MOD_REL_PATH} запрещены .unwrap() и .expect()`,
    );

    const sizeCheck = checkRustSourceSizeLimits(RECORDER_MOD_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода в ${RECORDER_MOD_REL_PATH}: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла ${RECORDER_MOD_REL_PATH} 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(source, RECORDER_MOD_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `В исходнике ${RECORDER_MOD_REL_PATH} не должно быть незамаскированных секретов`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RECORDER_MOD_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл для ${RECORDER_MOD_REL_PATH}:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(
      RECORDER_MOD_PATH,
      recorderTestBinPath,
    );
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings для ${RECORDER_MOD_REL_PATH}:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора в ${RECORDER_MOD_REL_PATH}:\n${compiled.stderr}`,
    );

    const runAll = runCompiledRustTests(recorderTestBinPath);
    assert.equal(
      runAll.status,
      0,
      `Встроенные тесты ${RECORDER_MOD_REL_PATH} упали:\n${runAll.stdout}\n${runAll.stderr}`,
    );
    assert.match(runAll.stdout, /test result: ok\./);
  });

  it("SUB-1003A-1: сверяет карточку TASK-1003A в waves/kanban-board.json, дизайн-токены (#1e1f22, #2b2d31, #ffffff, #23a55a, #3f4147), состояния mic-record и RE-оракулы (rest-d02.json, ATTACH-RENDER-RE.md, ATTACH-RE.md)", () => {
    assert.ok(
      existsSync(recorderTestBinPath),
      "Бинарник voice_message_recorder должен быть собран",
    );

    // 1. Независимый оракул: карточка TASK-1003A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1003A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1003A",
    );
    assert.ok(
      card1003A,
      "Карточка TASK-1003A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1003A.department, "G5 Voice / Platform");
    assert.deepEqual(card1003A.layout, {
      container: "Audio Recorder",
      dimensions: "48kHz mono buffer",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1003A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#23a55a",
      border: "#3f4147",
    });
    assert.deepEqual(card1003A.interactive_states[0], {
      element: "mic-record",
      default: "idle",
      hover: "hover mic",
      active: "recording audio",
    });

    // 2. Независимый оракул: /home/sanya/re-deep/rest-d02.json и docs/re/rest-d02.json
    const restD02Raw = readFileSync(REST_D02_JSON_PATH, "utf8");
    assert.ok(
      restD02Raw.includes('"/api/v9/channels/{id}/messages"'),
      "Маршрут /api/v9/channels/{id}/messages должен быть в docs/re/rest-d02.json",
    );
    assert.ok(
      restD02Raw.includes("GET|POST /channels/{arg}/messages [MESSAGES]"),
      "Статический маршрут GET|POST /channels/{arg}/messages [MESSAGES] должен быть в docs/re/rest-d02.json",
    );
    if (existsSync(RE_DEEP_REST_D02_PATH)) {
      const reDeepRestRaw = readFileSync(RE_DEEP_REST_D02_PATH, "utf8");
      assert.ok(reDeepRestRaw.includes('"/api/v9/channels/{id}/messages"'));
      assert.ok(
        reDeepRestRaw.includes("GET|POST /channels/{arg}/messages [MESSAGES]"),
      );
    }

    // 3. Независимый оракул: waves/RW-40/ATTACH-RENDER-RE.md и waves/RW-40/ATTACH-RE.md
    const attachRenderRe = readFileSync(ATTACH_RENDER_RE_PATH, "utf8");
    assert.ok(attachRenderRe.includes("IS_VOICE_MESSAGE=8192"));
    assert.ok(
      attachRenderRe.includes(LIVE_VOICE_ATTACHMENT_ORACLE.attachmentId),
    );
    assert.ok(attachRenderRe.includes(LIVE_VOICE_ATTACHMENT_ORACLE.waveform));
    assert.ok(attachRenderRe.includes('canPlayType("audio/ogg; codecs=opus")'));

    const attachRe = readFileSync(ATTACH_RE_PATH, "utf8");
    assert.ok(attachRe.includes("POST /channels/{id}/attachments"));
    assert.ok(attachRe.includes("X-Discord-Original-MD5"));
    assert.ok(attachRe.includes("uploaded_filename"));

    // 4. Сверка дизайн-токенов и геометрии TASK-1003A в исходнике Rust-модуля
    const recorderSource = readFileSync(RECORDER_MOD_PATH, "utf8");
    for (const tokenVal of Object.values(card1003A.design_tokens)) {
      assert.ok(
        recorderSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1003A должен быть зафиксирован в ${RECORDER_MOD_REL_PATH}`,
      );
    }
    assert.ok(recorderSource.includes("Audio Recorder"));
    assert.ok(recorderSource.includes("48kHz mono buffer"));
    assert.ok(recorderSource.includes("mic-record"));
    assert.ok(recorderSource.includes("audio/ogg; codecs=opus"));

    const run = runCompiledRustTests(recorderTestBinPath, "test_sub_1003a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003A-1 в voice_message_recorder упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003a_1_.*ok/);
  });

  it("SUB-1003A-2: реализует буфер захвата cpal (48kHz mono, кадр 20 мс = 960 сэмплов), расчёт RMS на кадр и кодирование в OGG Opus (32 kbps, VBR) с синхронизацией длительности", () => {
    assert.ok(
      existsSync(recorderTestBinPath),
      "Бинарник voice_message_recorder должен быть собран",
    );

    const recorderSource = readFileSync(RECORDER_MOD_PATH, "utf8");
    for (const requiredSymbol of [
      "SAMPLE_RATE_48KHZ",
      "CHANNELS_MONO",
      "OPUS_BITRATE_32KBPS",
      "FRAME_DURATION_20MS",
      "OPUS_FRAME_SAMPLES_20MS",
      "CpalVoiceRecorderBuffer",
      "VoiceMessageOggOpusRecorder",
      "compute_frame_rms_20ms",
      "encode_opus_frame_32kbps_vbr",
      "compute_ogg_crc32",
    ]) {
      assert.match(
        recorderSource,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле voice_message_recorder.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(recorderTestBinPath, "test_sub_1003a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003A-2 в voice_message_recorder упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003a_2_.*ok/);
  });

  it("SUB-1003A-3: нормализует RMS-амплитуды в диапазон 0..=255 (256 байт), экспортирует в base64 (344 символа) и формирует REST-запросы с flags: 8192 и audio/ogg; codecs=opus", () => {
    assert.ok(
      existsSync(recorderTestBinPath),
      "Бинарник voice_message_recorder должен быть собран",
    );

    // Независимый JS-оракул расчёта 256-байтного waveform на 30 кадрах по 960 сэмплов (600 мс)
    const frameCount = 30;
    const pcm = new Float32Array(960 * frameCount);
    for (let f = 0; f < frameCount; f += 1) {
      const amp = (f + 1) / frameCount;
      for (let i = 0; i < 960; i += 1) {
        pcm[f * 960 + i] = amp * Math.sin((2 * Math.PI * 440 * i) / 48_000);
      }
    }
    const oracleWave = oracleComputeWaveform256(pcm);
    assert.equal(oracleWave.frameRms.length, 30);
    assert.equal(oracleWave.waveformBytes.length, 256);
    assert.equal(oracleWave.waveformBase64.length, 344);
    assert.equal(oracleWave.waveformBytes[255], 255);
    assert.ok(
      (oracleWave.waveformBytes[0] ?? 0) >= 7 &&
        (oracleWave.waveformBytes[0] ?? 0) <= 10,
    );

    const recorderSource = readFileSync(RECORDER_MOD_PATH, "utf8");
    for (const requiredSymbol of [
      "WAVEFORM_BYTES_LEN",
      "WAVEFORM_BASE64_LEN",
      "MESSAGE_FLAG_IS_VOICE_MESSAGE",
      "VOICE_MESSAGE_MIME",
      "generate_waveform_256_bytes",
      "encode_waveform_base64",
      "decode_waveform_base64",
      "build_attachment_upload_request_json",
      "build_voice_message_send_payload",
    ]) {
      assert.match(
        recorderSource,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле voice_message_recorder.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(recorderTestBinPath, "test_sub_1003a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003A-3 в voice_message_recorder упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003a_3_.*ok/);
  });

  it("SUB-1003A-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные 48kHz/960 сэмплов/256 байт, Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, ошибки 400..5xx, тишина/DTX, HTTP 202, права доступа, fallback микрофона и маскирование секретов)", () => {
    assert.ok(
      existsSync(recorderTestBinPath),
      "Бинарник voice_message_recorder должен быть собран",
    );

    const run = runCompiledRustTests(recorderTestBinPath, "test_sub_1003a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003A-4 в voice_message_recorder упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003a_4_.*ok/);
  });

  it("SUB-1003A-5: убивает 5/5 контрольных мутаций в voice_message_recorder.rs (ошибочный sample_rate, нарушение нормализации u8, рассинхронизация длительности, подмена flags 8192, искажение битрейта Opus)", () => {
    assert.ok(
      existsSync(RECORDER_MOD_PATH),
      "Модуль voice_message_recorder.rs должен существовать",
    );
    const originalRust = readFileSync(RECORDER_MOD_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-1003A-RS-01",
        operator: "WrongSampleRate48kHzTamper",
        from: "pub const SAMPLE_RATE_48KHZ: u32 = 48_000;",
        to: "pub const SAMPLE_RATE_48KHZ: u32 = 44_100;",
      },
      {
        id: "MUT-1003A-RS-02",
        operator: "U8WaveformNormalizationOverflowOrZeroTamper",
        from: "let scaled = ((avg / peak) * 255.0).round().clamp(0.0, 255.0) as u8;",
        to: "let scaled = 0_u8;",
      },
      {
        id: "MUT-1003A-RS-03",
        operator: "DurationSecsDesynchronizationTamper",
        from: "(self.samples.len() as f64) / f64::from(self.sample_rate_hz)",
        to: "(self.samples.len() as f64) / 1_000.0",
      },
      {
        id: "MUT-1003A-RS-04",
        operator: "VoiceMessageFlag8192Tamper",
        from: "pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 8_192;",
        to: "pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 4_096;",
      },
      {
        id: "MUT-1003A-RS-05",
        operator: "OpusBitrate32kbpsTamper",
        from: "pub const OPUS_BITRATE_32KBPS: u32 = 32_000;",
        to: "pub const OPUS_BITRATE_32KBPS: u32 = 16_000;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в voice_message_recorder.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "voice_message_recorder.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `recorder_mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `recorder_mutant_${idx + 1}_bin`);
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
      .update(readFileSync(RECORDER_MOD_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла voice_message_recorder.rs не должен измениться после мутационного гейта",
    );
    assert.equal(
      gateResult.passed,
      true,
      `Мутационный гейт для voice_message_recorder.rs не пройден (выжили: ${gateResult.survivors.join(", ")})`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1.0);
  });
});
