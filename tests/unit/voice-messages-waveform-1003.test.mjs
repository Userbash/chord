// @ts-check
/**
 * Контрактные и модульные тесты голосовых сообщений с Waveform в чате (OGG Opus 48kHz mono 32kbps,
 * 256 байт base64 waveform, флаг IS_VOICE_MESSAGE = 8192, интерактивный плеер GPUI 1x/1.5x/2x) — TASK-1003:
 * - SUB-1003-0: Статический и структурный контракт `rust/crates/chord-voice/src/voice_message.rs`
 *   и `rust/chord-gpui/src/chat/voice_message_player.rs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()`
 *   в prod-коде, лимиты `scripts/quality-gate.mjs` < 50 строк на функцию и < 800 строк на модуль,
 *   `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings` без предупреждений,
 *   0 утечек в `secret-scan.mjs`).
 * - SUB-1003-1: Сверка констант, дизайн-токенов (`#2b2d31`, `#383a40`, `#ffffff`, `#23a55a`, `#3f4147`),
 *   геометрии (`Audio Waveform Player` `280px x 48px`, `padding: 8px`, `spacing: 6px`; `Audio Recorder`
 *   `48kHz mono buffer`), интерактивных состояний (`waveform-bar`, `mic-record`, `scrub-bar`), флага
 *   `IS_VOICE_MESSAGE = 8192` и живого вложения `voice-message.ogg` с независимыми RE-оракулами
 *   (`waves/kanban-board.json`, `docs/re/rest-d02.json`, `waves/RW-40/ATTACH-RENDER-RE.md`, `waves/RW-40/ATTACH-RE.md`).
 * - SUB-1003-2: Независимый оракул захвата микрофона через `cpal` (48kHz mono, кадры 20ms = 960 сэмплов),
 *   кодирования в контейнер OGG Opus 32 kbps VBR (`OggS`, `OpusHead`, `OpusTags`, CRC-32), расчёта RMS-амплитуд
 *   и нормализации в 256-байтный массив `0..=255` (base64 `waveform`, 344 символа) и формирования REST-запросов
 *   загрузки (`POST /channels/{id}/attachments` -> `PUT` -> `POST /channels/{id}/messages` с `flags: 8192`).
 * - SUB-1003-3: Независимый оракул интерактивного плеера `VoiceMessagePlayer` (`rust/chord-gpui/src/chat/voice_message_player.rs`):
 *   декодирование base64 `waveform`, рендеринг 32 / 64 / 128 полосок волны с динамической подсветкой прогресса
 *   (`#23a55a`), мгновенная перемотка кликом по волне (`seek_by_click_x`) и переключение скорости
 *   `1x -> 1.5x -> 2x -> 1x` с сохранением высоты тона (WSOLA pitch preservation).
 * - SUB-1003-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   `48_000 Hz`, `960` сэмплов, `256` байт waveform, `64/128` полосок, `0.5..1200.0 с`, Snowflake `0`/`u64::MAX`,
 *   2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки `400/401/403/404/413/429/5xx`;
 *   пустые/тишина/DTX `[0xF8, 0xFF, 0xFE]`; прогрев `HTTP 202`; права `VIEW_CHANNEL`/`SEND_MESSAGES`/`ATTACH_FILES`/
 *   `SEND_VOICE_MESSAGES`/`AGE_GATED_SPACES`; fallback микрофона `cpal` и refresh CDN URL; маскирование токенов `mfa.***`).
 * - SUB-1003-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs` для обоих модулей.
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
const VOICE_MOD_REL_PATH = "rust/crates/chord-voice/src/voice_message.rs";
const VOICE_MOD_PATH = join(ROOT, VOICE_MOD_REL_PATH);
const PLAYER_MOD_REL_PATH = "rust/chord-gpui/src/chat/voice_message_player.rs";
const PLAYER_MOD_PATH = join(ROOT, PLAYER_MOD_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const ATTACH_RENDER_RE_PATH = join(ROOT, "waves", "RW-40", "ATTACH-RENDER-RE.md");
const ATTACH_RE_PATH = join(ROOT, "waves", "RW-40", "ATTACH-RE.md");
const ATTACH_RENDER_SPEC_PATH = join(
  ROOT,
  "waves",
  "RW-40",
  "DESIGN",
  "ATTACHMENTS-RENDER-SPEC.md",
);

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
  contentScanVersion: 1,
  flags: 8192,
});

/** @type {string} */
let workDir = "";
/** @type {string} */
let voiceTestBinPath = "";
/** @type {string} */
let playerTestBinPath = "";

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
 * Независимый JS-оракул расчёта 256-байтного нормализованного waveform (0..=255) и его Base64-представления
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
    const end = Math.max(start + 1, Math.floor(((i + 1) * frameRms.length) / 256));
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

/**
 * Независимый JS-оракул декодирования base64 `waveform` и ресемплинга в `barCount` полосок (0..1)
 * по RE-контракту `ATTACH-RENDER-RE.md` §9.1 (`@6133343`).
 * @param {string} base64Str
 * @param {number} barCount
 * @returns {number[]}
 */
function oracleDecodeWaveformBars(base64Str, barCount) {
  const safeCount = Math.max(1, barCount);
  if (!base64Str || typeof base64Str !== "string") {
    return new Array(safeCount).fill(0);
  }
  const buf = Buffer.from(base64Str, "base64");
  if (buf.length === 0) {
    return new Array(safeCount).fill(0);
  }
  const rawAmps = Array.from(buf, (b) => b / 255);
  const bars = new Array(safeCount).fill(0);
  if (rawAmps.length <= safeCount) {
    for (let i = 0; i < rawAmps.length; i += 1) {
      bars[i] = rawAmps[i] ?? 0;
    }
    return bars;
  }
  const bucketSize = rawAmps.length / safeCount;
  for (let i = 0; i < safeCount; i += 1) {
    const start = Math.floor(i * bucketSize);
    const end = Math.max(start + 1, Math.floor((i + 1) * bucketSize));
    let sum = 0;
    for (let j = start; j < end && j < rawAmps.length; j += 1) {
      sum += rawAmps[j] ?? 0;
    }
    bars[i] = sum / (end - start);
  }
  return bars;
}

describe("TASK-1003: Голосовые сообщения с Waveform в чате (OGG Opus 48kHz mono 32kbps, 256B waveform, IS_VOICE_MESSAGE=8192, плеер 1x/1.5x/2x)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "voice-messages-1003-"));
    voiceTestBinPath = join(workDir, "voice_message_1003_test");
    playerTestBinPath = join(workDir, "voice_message_player_1003_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1003-0: модули voice_message.rs и voice_message_player.rs существуют, запрещают unsafe, проходят rustfmt 2024, лимиты (<50 строк на fn, <800 на файл) и компилируются без предупреждений", () => {
    const modulesToCheck = [
      { relPath: VOICE_MOD_REL_PATH, absPath: VOICE_MOD_PATH, binPath: voiceTestBinPath },
      { relPath: PLAYER_MOD_REL_PATH, absPath: PLAYER_MOD_PATH, binPath: playerTestBinPath },
    ];
    for (const { relPath, absPath, binPath } of modulesToCheck) {
      assert.ok(
        existsSync(absPath),
        `Модуль ${relPath} должен существовать`,
      );

      const source = readFileSync(absPath, "utf8");
      assert.match(
        source,
        /^#!\[forbid\(unsafe_code\)\]/m,
        `Модуль ${relPath} обязан содержать #![forbid(unsafe_code)]`,
      );

      const prodSection = source.split("#[cfg(test)]")[0] ?? "";
      assert.equal(
        prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
        false,
        `В продакшен-коде модуля ${relPath} запрещены .unwrap() и .expect()`,
      );

      const sizeCheck = checkRustSourceSizeLimits(relPath, source);
      assert.equal(
        sizeCheck.passed,
        true,
        `Нарушение лимитов размера кода в ${relPath}: ${sizeCheck.reason}`,
      );
      assert.ok(
        sizeCheck.fileLines > 200 && sizeCheck.fileLines < 800,
        `Ожидался размер файла ${relPath} 200..800 строк, получено: ${sizeCheck.fileLines}`,
      );

      const secretFindings = scanTextForSecrets(source, relPath);
      assert.equal(
        secretFindings.length,
        0,
        `В исходнике ${relPath} не должно быть незамаскированных секретов`,
      );

      const fmtCheck = spawnSync(
        "rustfmt",
        ["--check", "--edition", "2024", absPath],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        fmtCheck.status,
        0,
        `rustfmt --check не прошёл для ${relPath}:\n${fmtCheck.stderr || fmtCheck.stdout}`,
      );

      const compiled = compileRustModuleTests(absPath, binPath);
      assert.equal(
        compiled.status,
        0,
        `Ошибка компиляции rustc --test -D warnings для ${relPath}:\n${compiled.stderr}`,
      );
      assert.equal(
        compiled.stderr.includes("warning:"),
        false,
        `Не должно быть предупреждений компилятора в ${relPath}:\n${compiled.stderr}`,
      );
    }
  });

  it("SUB-1003-1: сверяет константы OGG Opus (48kHz mono 32kbps), флаг IS_VOICE_MESSAGE (8192), дизайн-токены, интерактивные состояния и живое вложение voice-message.ogg с независимыми RE-оракулами", () => {
    assert.ok(existsSync(voiceTestBinPath), "Бинарник voice_message должен быть собран");
    assert.ok(existsSync(playerTestBinPath), "Бинарник voice_message_player должен быть собран");

    // 1. Независимый оракул: карточки TASK-1003, TASK-1003A, TASK-1003B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1003 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1003");
    const card1003A = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1003A");
    const card1003B = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1003B");
    assert.ok(card1003, "Карточка TASK-1003 должна присутствовать в waves/kanban-board.json");
    assert.ok(card1003A, "Карточка TASK-1003A должна присутствовать в waves/kanban-board.json");
    assert.ok(card1003B, "Карточка TASK-1003B должна присутствовать в waves/kanban-board.json");

    assert.equal(card1003.layout.container, "Audio Waveform Player");
    assert.equal(card1003.layout.dimensions, "280px x 48px");
    assert.equal(card1003.layout.padding, "8px");
    assert.equal(card1003.layout.spacing, "6px");
    assert.deepEqual(card1003.design_tokens, {
      background: "#2b2d31",
      surface: "#383a40",
      text_primary: "#ffffff",
      accent: "#23a55a",
      border: "#3f4147",
    });
    assert.deepEqual(card1003.interactive_states[0], {
      element: "waveform-bar",
      default: "muted",
      hover: "highlight",
      active: "playing",
    });
    assert.deepEqual(card1003A.interactive_states[0], {
      element: "mic-record",
      default: "idle",
      hover: "hover mic",
      active: "recording audio",
    });
    assert.deepEqual(card1003B.interactive_states[0], {
      element: "scrub-bar",
      default: "muted bars",
      hover: "hover time",
      active: "seek position",
    });

    // 2. Независимый оракул: docs/re/rest-d02.json (маршрут /api/v9/channels/{id}/messages)
    const restD02Raw = readFileSync(REST_D02_JSON_PATH, "utf8");
    assert.ok(
      restD02Raw.includes('"/api/v9/channels/{id}/messages"'),
      "Маршрут /api/v9/channels/{id}/messages должен быть в docs/re/rest-d02.json",
    );
    assert.ok(
      restD02Raw.includes("GET|POST /channels/{arg}/messages [MESSAGES]"),
      "Статический маршрут GET|POST /channels/{arg}/messages [MESSAGES] должен быть в docs/re/rest-d02.json",
    );

    // 3. Независимый оракул: waves/RW-40/ATTACH-RENDER-RE.md и waves/RW-40/ATTACH-RE.md
    const attachRenderRe = readFileSync(ATTACH_RENDER_RE_PATH, "utf8");
    assert.ok(attachRenderRe.includes("IS_VOICE_MESSAGE=8192"));
    assert.ok(attachRenderRe.includes(LIVE_VOICE_ATTACHMENT_ORACLE.attachmentId));
    assert.ok(attachRenderRe.includes(LIVE_VOICE_ATTACHMENT_ORACLE.waveform));
    assert.ok(attachRenderRe.includes('canPlayType("audio/ogg; codecs=opus")'));

    const attachRe = readFileSync(ATTACH_RE_PATH, "utf8");
    assert.ok(attachRe.includes("POST /channels/{id}/attachments"));
    assert.ok(attachRe.includes("MESSAGE_CREATE_ATTACHMENT_UPLOAD"));
    assert.ok(attachRe.includes("X-Discord-Original-MD5"));
    assert.ok(attachRe.includes("uploaded_filename"));

    const attachRenderSpec = readFileSync(ATTACH_RENDER_SPEC_PATH, "utf8");
    assert.ok(attachRenderSpec.includes('data-media-type="VOICE_MESSAGE"'));
    assert.ok(attachRenderSpec.includes("decodeWaveform"));

    // 4. Проверяем фиксацию констант и токенов в обоих Rust-модулях
    const voiceSource = readFileSync(VOICE_MOD_PATH, "utf8");
    const playerSource = readFileSync(PLAYER_MOD_PATH, "utf8");
    for (const tokenVal of Object.values(card1003.design_tokens)) {
      assert.ok(
        voiceSource.includes(tokenVal) && playerSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} должен быть зафиксирован в обоих модулях`,
      );
    }

    const runVoice = runCompiledRustTests(voiceTestBinPath, "test_sub_1003_1_");
    assert.equal(runVoice.status, 0, `Тесты SUB-1003-1 в voice_message упали:\n${runVoice.stdout}\n${runVoice.stderr}`);
    assert.match(runVoice.stdout, /test tests::test_sub_1003_1_.*ok/);

    const runPlayer = runCompiledRustTests(playerTestBinPath, "test_sub_1003_1_");
    assert.equal(runPlayer.status, 0, `Тесты SUB-1003-1 в voice_message_player упали:\n${runPlayer.stdout}\n${runPlayer.stderr}`);
    assert.match(runPlayer.stdout, /test tests::test_sub_1003_1_.*ok/);
  });

  it("SUB-1003-2: кодирует буфер микрофона cpal (48kHz mono, 20ms = 960 сэмплов) в OGG Opus 32 kbps, вычисляет 256 байт base64 waveform по RMS и формирует REST-запрос с флагом IS_VOICE_MESSAGE (8192)", () => {
    assert.ok(existsSync(voiceTestBinPath), "Бинарник voice_message должен быть собран");

    // Независимый JS-оракул расчёта 256-байтного waveform на синтетическом голосовом сигнале (10 кадров по 960 сэмплов = 200 мс)
    const totalSamples = 960 * 10;
    const pcm = new Float32Array(totalSamples);
    for (let f = 0; f < 10; f += 1) {
      const amp = (f + 1) / 10;
      for (let i = 0; i < 960; i += 1) {
        pcm[f * 960 + i] = amp * Math.sin((2 * Math.PI * 440 * i) / 48_000);
      }
    }
    const oracleWave = oracleComputeWaveform256(pcm);
    assert.equal(oracleWave.frameRms.length, 10);
    assert.equal(oracleWave.waveformBytes.length, 256);
    assert.equal(oracleWave.waveformBase64.length, 344);
    assert.equal(oracleWave.waveformBytes[255], 255);
    assert.ok((oracleWave.waveformBytes[0] ?? 0) >= 24 && (oracleWave.waveformBytes[0] ?? 0) <= 27);

    const voiceSource = readFileSync(VOICE_MOD_PATH, "utf8");
    for (const requiredSymbol of [
      "SAMPLE_RATE_48KHZ",
      "CHANNELS_MONO",
      "OPUS_BITRATE_32KBPS",
      "OPUS_FRAME_SAMPLES_20MS",
      "WAVEFORM_BYTES_LEN",
      "MESSAGE_FLAG_IS_VOICE_MESSAGE",
      "CpalMicCaptureBuffer",
      "VoiceMessageOggOpusRecorder",
      "compute_frame_rms_20ms",
      "generate_waveform_256_bytes",
      "encode_waveform_base64",
      "decode_waveform_base64",
      "build_voice_message_send_payload",
    ]) {
      assert.match(
        voiceSource,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле voice_message.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const runVoice = runCompiledRustTests(voiceTestBinPath, "test_sub_1003_2_");
    assert.equal(runVoice.status, 0, `Тесты SUB-1003-2 в voice_message упали:\n${runVoice.stdout}\n${runVoice.stderr}`);
    assert.match(runVoice.stdout, /test tests::test_sub_1003_2_.*ok/);
  });

  it("SUB-1003-3: рендерит 32/64/128 полосок волны в VoiceMessagePlayer с подсветкой прогресса (#23a55a), мгновенной перемоткой по клику и переключением скорости 1x / 1.5x / 2x с сохранением питча", () => {
    assert.ok(existsSync(playerTestBinPath), "Бинарник voice_message_player должен быть собран");

    // Проверяем независимым JS-оракулом декодирование живого waveform из дампа (84 байта -> 32, 64 и 128 полосок)
    const bars32 = oracleDecodeWaveformBars(LIVE_VOICE_ATTACHMENT_ORACLE.waveform, 32);
    const bars64 = oracleDecodeWaveformBars(LIVE_VOICE_ATTACHMENT_ORACLE.waveform, 64);
    const bars128 = oracleDecodeWaveformBars(LIVE_VOICE_ATTACHMENT_ORACLE.waveform, 128);
    assert.equal(bars32.length, 32);
    assert.equal(bars64.length, 64);
    assert.equal(bars128.length, 128);
    assert.equal(bars32[0], 0);
    assert.ok(bars64.some((v) => v > 0.05 && v <= 1.0));
    assert.ok(bars128.some((v) => v > 0.05 && v <= 1.0));

    const playerSource = readFileSync(PLAYER_MOD_PATH, "utf8");
    for (const requiredSymbol of [
      "PlaybackSpeed",
      "WaveformBarState",
      "ScrubBarState",
      "WaveformBarRenderSpec",
      "VoiceMessagePlayer",
      "decode_waveform_bars",
      "compute_bar_count_for_width",
      "format_voice_duration_mm_ss",
      "seek_by_click_x",
      "cycle_playback_speed",
      "apply_wsola_pitch_preserving_speed",
    ]) {
      assert.match(
        playerSource,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле voice_message_player.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const runPlayer = runCompiledRustTests(playerTestBinPath, "test_sub_1003_3_");
    assert.equal(runPlayer.status, 0, `Тесты SUB-1003-3 в voice_message_player упали:\n${runPlayer.stdout}\n${runPlayer.stderr}`);
    assert.match(runPlayer.stdout, /test tests::test_sub_1003_3_.*ok/);
  });

  it("SUB-1003-4: покрывает все 9 измерений TEST-POLICY.md (граничные 48kHz/960 сэмплов/256 байт, Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, ошибки 400..5xx, тишина/DTX, HTTP 202, права доступа, fallback микрофона/refresh CDN и маскирование секретов)", () => {
    assert.ok(existsSync(voiceTestBinPath), "Бинарник voice_message должен быть собран");
    assert.ok(existsSync(playerTestBinPath), "Бинарник voice_message_player должен быть собран");

    const runVoice = runCompiledRustTests(voiceTestBinPath, "test_sub_1003_4_");
    assert.equal(runVoice.status, 0, `Тесты SUB-1003-4 в voice_message упали:\n${runVoice.stdout}\n${runVoice.stderr}`);
    assert.match(runVoice.stdout, /test tests::test_sub_1003_4_.*ok/);

    const runPlayer = runCompiledRustTests(playerTestBinPath, "test_sub_1003_4_");
    assert.equal(runPlayer.status, 0, `Тесты SUB-1003-4 в voice_message_player упали:\n${runPlayer.stdout}\n${runPlayer.stderr}`);
    assert.match(runPlayer.stdout, /test tests::test_sub_1003_4_.*ok/);
  });

  it("SUB-1003-5: убивает 5/5 контрольных мутаций в voice_message.rs и 5/5 мутаций в voice_message_player.rs (100% mutation kill rate)", () => {
    const voiceSource = readFileSync(VOICE_MOD_PATH, "utf8");
    const playerSource = readFileSync(PLAYER_MOD_PATH, "utf8");

    const voiceMutationSpecs = [
      {
        id: "MUT-voice-msg-01",
        operator: "SampleRate48kHzTampered",
        from: "pub const SAMPLE_RATE_48KHZ: u32 = 48_000;",
        to: "pub const SAMPLE_RATE_48KHZ: u32 = 44_100;",
      },
      {
        id: "MUT-voice-msg-02",
        operator: "VoiceMessageFlagTampered",
        from: "pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 8_192;",
        to: "pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 4_096;",
      },
      {
        id: "MUT-voice-msg-03",
        operator: "OpusBitrate32kbpsTampered",
        from: "pub const OPUS_BITRATE_32KBPS: u32 = 32_000;",
        to: "pub const OPUS_BITRATE_32KBPS: u32 = 16_000;",
      },
      {
        id: "MUT-voice-msg-04",
        operator: "WaveformRmsPeakNormalizationBypass",
        from: "let scaled = ((avg / peak) * 255.0).round().clamp(0.0, 255.0) as u8;",
        to: "let scaled = 0_u8;",
      },
      {
        id: "MUT-voice-msg-05",
        operator: "OggCapturePatternTampered",
        from: "pub const OGG_CAPTURE_PATTERN: [u8; 4] = *b\"OggS\";",
        to: "pub const OGG_CAPTURE_PATTERN: [u8; 4] = *b\"OggX\";",
      },
    ];

    const voiceMutants = voiceMutationSpecs.map((spec) => {
      assert.ok(
        voiceSource.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в voice_message.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "voice_message.rs",
        operator: spec.operator,
        mutatedSource: voiceSource.replace(spec.from, spec.to),
      };
    });

    const voiceGate = evaluateMutationGate(voiceMutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `voice_mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `voice_mutant_${idx + 1}_bin`);
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
      voiceGate.passed,
      true,
      `Мутационный гейт для voice_message.rs не пройден (выжили: ${voiceGate.survivors.join(", ")})`,
    );
    assert.equal(voiceGate.killed, 5);
    assert.equal(voiceGate.killRate, 1.0);

    const playerMutationSpecs = [
      {
        id: "MUT-voice-player-01",
        operator: "SpeedCycleOrderBroken",
        from: "Self::OneX => Self::OnePointFiveX,",
        to: "Self::OneX => Self::OneX,",
      },
      {
        id: "MUT-voice-player-02",
        operator: "AccentColorTokenTampered",
        from: "pub const TOKEN_ACCENT: &str = \"#23a55a\";",
        to: "pub const TOKEN_ACCENT: &str = \"#000000\";",
      },
      {
        id: "MUT-voice-player-03",
        operator: "ClickSeekRatioInverted",
        from: "let ratio = (click_x / width_px).clamp(0.0, 1.0);",
        to: "let ratio = (1.0 - click_x / width_px).clamp(0.0, 1.0);",
      },
      {
        id: "MUT-voice-player-04",
        operator: "PlayedBarComparisonInverted",
        from: "let played = progress_ratio > 0.0 && bar_ratio < progress_ratio;",
        to: "let played = progress_ratio > 0.0 && bar_ratio > progress_ratio;",
      },
      {
        id: "MUT-voice-player-05",
        operator: "PitchPreservationBrokenOnSpeedChange",
        from: "let preserved_pitch_hz = input_pitch_hz;",
        to: "let preserved_pitch_hz = input_pitch_hz * speed.multiplier();",
      },
    ];

    const playerMutants = playerMutationSpecs.map((spec) => {
      assert.ok(
        playerSource.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в voice_message_player.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "voice_message_player.rs",
        operator: spec.operator,
        mutatedSource: playerSource.replace(spec.from, spec.to),
      };
    });

    const playerGate = evaluateMutationGate(playerMutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `player_mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `player_mutant_${idx + 1}_bin`);
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
      playerGate.passed,
      true,
      `Мутационный гейт для voice_message_player.rs не пройден (выжили: ${playerGate.survivors.join(", ")})`,
    );
    assert.equal(playerGate.killed, 5);
    assert.equal(playerGate.killRate, 1.0);
  });
});
