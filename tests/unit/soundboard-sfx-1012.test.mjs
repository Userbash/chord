// @ts-check
/**
 * Контрактные и модульные тесты серверной звуковой панели (Soundboard SFX — TASK-1012 / TASK-1012A / TASK-1012B / TASK-1012C):
 * - SUB-1012-0: Статический и компиляционный контракт `rust/crates/discord-voice/src/soundboard.rs`
 *   и `scripts/verify-soundboard.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде,
 *   соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1012-1: Сверка карточек `TASK-1012`, `TASK-1012A`, `TASK-1012B`, `TASK-1012C` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Soundboard Grid`
 *   (`320px x 360px`, `padding: 8px`, `spacing: 6px`), интерактивных состояний `sound-button`
 *   (`sfx tile` -> `scale-105` -> `playing sfx`), `sfx-trigger` (`idle` -> `preview` -> `playing sfx`),
 *   `mixer-dsp` (`bypass` -> `mix` -> `tanh limiter`), событий шлюза `SOUNDBOARD_SOUNDS` (`21` в дампе
 *   `docs/re/gateway-protocol.json`), маршрутов REST (`GET /soundboard-default-sounds`,
 *   `POST /channels/{id}/send-soundboard-sound`, `POST /channels/{id}/send-soundboard-echo`,
 *   `POST /guilds/{id}/soundboard-sounds`, `PATCH|DELETE /guilds/{id}/soundboard-sounds/{sound_id}`,
 *   `GET /guilds/{id}/top-soundboard-sounds`, `GET /soundboard-sounds/{sound_id}/guild/{guild_id}`)
 *   и модулей RE-дампа (`209932`, `796774`, `813564`, `904054`, `125831`, `528772`) с независимыми оракулами.
 * - SUB-1012-2: Протокол Voice Gateway Opcode 18 (`SOUNDBOARD_SOUND`), парсинг входящего события
 *   Opcode 19 / `SPEAKING` (`SOUND_SHARE = 1 << 1`), построение REST-запросов и локальное кэширование
 *   аудио-сэмплов в формате Opus 48 кГц stereo (`960` сэмплов/канал на кадр `20 мс`, лимит `512 КБ`, LRU-вытеснение).
 * - SUB-1012-3: Линейный микшер звуковых эффектов в исходящий поток RTP Opus (`PT = 120`, шаг timestamp `960`)
 *   с регулируемым уровнем громкости (`904054`) и софт-клиппингом через `tanh()` без искажения речи,
 *   а также автоматическая остановка микширования по окончании сэмпла без лишних циклов CPU.
 * - SUB-1012-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   Snowflake `0`/`u64::MAX`, имя звука `2..=32` симв., размер файла `524288` Б, избранное `250`,
 *   `2000` символов сообщения, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`, бейдж `99+`;
 *   ошибки `400/401/403/404/413/429/5xx`; пустые `soundboard_sounds: null`; `skeleton`/`HTTP 202`;
 *   права `CONNECT`/`SPEAK`/`USE_SOUNDBOARD = 1 << 42`/`USE_EXTERNAL_SOUNDS = 1 << 45`/`AGE_GATED_SPACES`;
 *   восстановление при сбое CDN и смене канала; маскирование секретов) и CLI `scripts/verify-soundboard.mjs` (`0 / 1 / 2`).
 * - SUB-1012-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-voice/src/soundboard.rs`
 *   и `scripts/verify-soundboard.mjs` по контракту `scripts/quality-gate.mjs`.
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
  MAX_SOUNDBOARD_BYTES as SPIKE_MAX_SOUNDBOARD_BYTES,
  MAX_SOUNDBOARD_NAME_CHARS as SPIKE_MAX_SOUNDBOARD_NAME_CHARS,
  MIN_SOUNDBOARD_NAME_CHARS as SPIKE_MIN_SOUNDBOARD_NAME_CHARS,
  clampSoundboardVolume as spikeClampSoundboardVolume,
  filterSoundboardSounds as spikeFilterSoundboardSounds,
  isValidSoundboardName as spikeIsValidSoundboardName,
} from "../../rust/gui-spike/ui/features/soundboard-picker.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/crates/discord-voice/src/soundboard.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-soundboard.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GATEWAY_PROTOCOL_JSON_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-voice/src/soundboard.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1012: Серверная звуковая панель (Soundboard SFX — rust/crates/discord-voice/src/soundboard.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "soundboard-sfx-1012-"));
    testBinPath = join(workDir, "soundboard_sfx_1012_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1012-0: модули rust/crates/discord-voice/src/soundboard.rs и scripts/verify-soundboard.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Модуль ${RUST_MODULE_REL_PATH} должен существовать`,
    );
    assert.ok(
      existsSync(SCRIPT_PATH),
      `Скрипт ${SCRIPT_REL_PATH} должен существовать`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "Rust-модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = rustSource.split("#[cfg(test)]")[0] ?? rustSource;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде soundboard.rs запрещены .unwrap() и .expect()",
    );

    const rustSizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      rustSizeCheck.passed,
      true,
      `Нарушение лимитов размера Rust-кода: ${rustSizeCheck.reason}`,
    );
    assert.ok(
      rustSizeCheck.fileLines > 200 && rustSizeCheck.fileLines < 800,
      `Ожидался размер файла 200..800 строк, получено: ${rustSizeCheck.fileLines}`,
    );

    const jsSource = readFileSync(SCRIPT_PATH, "utf8");
    const jsLines = jsSource.split("\n").length;
    assert.ok(
      jsLines > 150 && jsLines < 800,
      `Ожидался размер JS-скрипта 150..800 строк, получено: ${jsLines}`,
    );

    const secretScanRust = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    const secretScanJs = scanTextForSecrets(jsSource, SCRIPT_REL_PATH);
    assert.equal(secretScanRust.length, 0, "В Rust-модуле не должно быть захардкоженных секретов");
    assert.equal(secretScanJs.length, 0, "В JS-скрипте не должно быть захардкоженных секретов");

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл:\n${fmtCheck.stderr || fmtCheck.stdout}`,
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
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );

    const rustRun = runCompiledRustTests(testBinPath);
    assert.equal(
      rustRun.status,
      0,
      `Встроенные тесты soundboard.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1012-1: сверяет дизайн-токены, раскладку Soundboard Grid, состояния sound-button / mixer-dsp, маршруты REST и события SOUNDBOARD_SOUNDS с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      SOUNDBOARD_GRID_LAYOUT,
      SOUNDBOARD_DESIGN_TOKENS,
      SOUNDBOARD_BUTTON_STATES,
      SOUNDBOARD_PROTOCOL_CONSTANTS,
      LIVE_SOUNDBOARD_SOUNDS_ORACLE,
      resolveSoundboardButtonState,
      resolveMixerDspState,
      verifySoundboardOracleParity,
    } = await import("../../scripts/verify-soundboard.mjs");

    // 1. Оракул: карточки TASK-1012, TASK-1012A, TASK-1012B, TASK-1012C в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1012 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1012");
    const card1012A = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1012A");
    const card1012B = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1012B");
    const card1012C = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1012C");
    assert.ok(card1012 && card1012A && card1012B && card1012C, "Карточки TASK-1012..1012C должны присутствовать в kanban-board.json");
    assert.deepEqual(SOUNDBOARD_GRID_LAYOUT, card1012.layout);
    assert.deepEqual(SOUNDBOARD_DESIGN_TOKENS, card1012.design_tokens);
    assert.deepEqual(SOUNDBOARD_BUTTON_STATES, card1012.interactive_states[0]);

    // 2. Интерактивные состояния sound-button (sfx tile -> scale-105 -> playing sfx) и mixer-dsp (bypass -> mix -> tanh limiter)
    assert.deepEqual(resolveSoundboardButtonState({ playing: false, hovered: false }), {
      element: "sound-button",
      state: "default",
      label: "sfx tile",
    });
    assert.deepEqual(resolveSoundboardButtonState({ playing: false, hovered: true }), {
      element: "sound-button",
      state: "hover",
      label: "scale-105",
    });
    assert.deepEqual(resolveSoundboardButtonState({ playing: true, hovered: false }), {
      element: "sound-button",
      state: "active",
      label: "playing sfx",
    });

    assert.equal(resolveMixerDspState({ activeSfx: false, limiting: false }), "bypass");
    assert.equal(resolveMixerDspState({ activeSfx: true, limiting: false }), "mix");
    assert.equal(resolveMixerDspState({ activeSfx: true, limiting: true }), "tanh limiter");

    // 3. Оракул docs/re/gateway-protocol.json (SOUNDBOARD_SOUNDS = 21) и паритет с soundboard-picker.js
    const gwProto = JSON.parse(readFileSync(GATEWAY_PROTOCOL_JSON_PATH, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.SOUNDBOARD_SOUNDS, 21);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.VOICE_OP_SOUNDBOARD_SOUND, 18);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.VOICE_OP_SPEAKING_SOUNDBOARD, 19);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.SPEAKING_FLAG_SOUND_SHARE, 2);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.OPUS_SAMPLE_RATE_HZ, 48_000);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.OPUS_CHANNELS, 2);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.FRAME_SAMPLES_PER_CHANNEL_20MS, 960);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.RTP_OPUS_PAYLOAD_TYPE, 120);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.MIN_NAME_CHARS, SPIKE_MIN_SOUNDBOARD_NAME_CHARS);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.MAX_NAME_CHARS, SPIKE_MAX_SOUNDBOARD_NAME_CHARS);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.MAX_SOUND_BYTES, SPIKE_MAX_SOUNDBOARD_BYTES);
    assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.MAX_FAVORITE_SOUNDS, 250);
    assert.ok(LIVE_SOUNDBOARD_SOUNDS_ORACLE.length >= 4);

    // 4. Полная сверка с RE-оракулами
    const parity = verifySoundboardOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      gatewayProtocolPath: GATEWAY_PROTOCOL_JSON_PATH,
      restD02Path: REST_D02_JSON_PATH,
      reRestStaticPath: existsSync(RE_REST_STATIC_PATH) ? RE_REST_STATIC_PATH : undefined,
      reModulesIndexPath: existsSync(RE_MODULES_INDEX_PATH) ? RE_MODULES_INDEX_PATH : undefined,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.gatewaySoundboardSoundsCount, 21);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1012_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1012-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1012_1_.*ok/);
  });

  it("SUB-1012-2: формирует Voice Gateway Opcode 18 (SOUNDBOARD_SOUND), парсит Opcode 19 / SPEAKING (SOUND_SHARE), строит маршруты REST и кэширует Opus 48 кГц stereo сэмплы", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      buildVoiceSoundboardOp18Payload,
      parseVoiceSoundboardSpeakingEvent,
      buildSendSoundboardSoundRequest,
      buildSendSoundboardEchoRequest,
      buildGuildSoundboardUploadRequest,
      buildSoundboardCdnUrl,
      SoundboardAudioCache,
    } = await import("../../scripts/verify-soundboard.mjs");

    // 1. Формирование Voice Gateway Opcode 18 (SOUNDBOARD_SOUND)
    const op18 = buildVoiceSoundboardOp18Payload({
      soundId: "1102645616632397904",
      sourceGuildId: "274093052673392641",
      volume: 0.82,
      emojiId: "1024454092543758356",
      emojiName: null,
      ssrc: 17843,
    });
    assert.equal(op18.op, 18);
    assert.equal(op18.d.sound_id, "1102645616632397904");
    assert.equal(op18.d.source_guild_id, "274093052673392641");
    assert.equal(op18.d.volume, spikeClampSoundboardVolume(0.82));
    assert.equal(op18.d.ssrc, 17843);

    // 2. Парсинг входящего события Voice Gateway Opcode 19 / SPEAKING (флаг SOUND_SHARE = 1 << 1)
    const speakingEv = parseVoiceSoundboardSpeakingEvent({
      op: 19,
      d: {
        user_id: "275258435199827968",
        ssrc: 9208,
        speaking: 3, // MICROPHONE (1) | SOUND_SHARE (2)
        sound_id: "1102645616632397904",
        volume: 0.99,
      },
    });
    assert.equal(speakingEv.isSoundboardActive, true);
    assert.equal(speakingEv.isMicrophoneActive, true);
    assert.equal(speakingEv.ssrc, 9208);
    assert.equal(speakingEv.soundId, "1102645616632397904");
    assert.equal(speakingEv.volume, 0.99);

    // 3. Построение REST-запросов SEND_SOUNDBOARD_SOUND / SEND_SOUNDBOARD_ECHO / GUILD_SOUNDBOARD_SOUNDS
    const sendCustom = buildSendSoundboardSoundRequest({
      channelId: "1294815650186788916",
      soundId: "1102645616632397904",
      guildId: "274093052673392641",
      emojiId: "1024454092543758356",
      emojiName: null,
    });
    assert.equal(sendCustom.method, "POST");
    assert.equal(sendCustom.path, "/channels/1294815650186788916/send-soundboard-sound");
    assert.equal(sendCustom.body.source_guild_id, "274093052673392641");

    const sendDefault = buildSendSoundboardSoundRequest({
      channelId: "1294815650186788916",
      soundId: "1",
      guildId: "0",
      emojiId: null,
      emojiName: "🦆",
    });
    assert.equal("source_guild_id" in sendDefault.body, false);

    const echoReq = buildSendSoundboardEchoRequest({
      channelId: "1294815650186788916",
      soundId: "1102645616632397904",
      sourceGuildId: "274093052673392641",
    });
    assert.equal(echoReq.method, "POST");
    assert.equal(echoReq.path, "/channels/1294815650186788916/send-soundboard-echo");

    const uploadReq = buildGuildSoundboardUploadRequest({
      guildId: "274093052673392641",
      name: "Хабар принёс",
      soundDataUri: "data:audio/ogg;base64,T2dnUwACAAAAAAAAAAA=",
      volume: 1.0,
      emojiId: "1024454092543758356",
      emojiName: null,
    });
    assert.equal(uploadReq.method, "POST");
    assert.equal(uploadReq.path, "/guilds/274093052673392641/soundboard-sounds");
    assert.equal(spikeIsValidSoundboardName(uploadReq.body.name), true);

    // 4. Кэширование сэмплов Opus 48 кГц stereo на диске/в памяти с контролем целостности и LRU-вытеснением
    assert.equal(
      buildSoundboardCdnUrl("1102645616632397904"),
      "https://cdn.discordapp.com/soundboard-sounds/1102645616632397904",
    );

    const cache = new SoundboardAudioCache({ maxBytes: 1_000 });
    const entry1 = cache.putSample({
      soundId: "1102645616632397904",
      sampleRateHz: 48_000,
      channels: 2,
      opusPayloadBytes: new Uint8Array(600).fill(0x4f),
      pcmStereoSamples: new Float32Array(1920).fill(0.25),
      nowMs: 1_000,
    });
    assert.equal(entry1.sampleRateHz, 48_000);
    assert.equal(entry1.channels, 2);
    assert.equal(entry1.byteLength, 600);
    assert.equal(cache.getSample("1102645616632397904", 1_010)?.soundId, "1102645616632397904");

    // Вставка второго сэмпла на 600 Б должна вытеснить первый по лимиту maxBytes = 1000 Б
    cache.putSample({
      soundId: "1102650064771485746",
      sampleRateHz: 48_000,
      channels: 2,
      opusPayloadBytes: new Uint8Array(600).fill(0x50),
      pcmStereoSamples: new Float32Array(1920).fill(0.5),
      nowMs: 1_020,
    });
    assert.equal(cache.getSample("1102645616632397904", 1_030), null);
    assert.equal(cache.getSample("1102650064771485746", 1_030)?.soundId, "1102650064771485746");
    assert.equal(cache.stats().evictions, 1);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1012_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1012-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1012_2_.*ok/);
  });

  it("SUB-1012-3: микширует звук саундборда в локальный RTP Opus поток с софт-клиппингом tanh() и автоматически останавливает микширование по окончании сэмпла", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      computeEffectiveSoundboardVolume,
      softClipTanhSample,
      SoundboardRtpMixer,
      LIVE_SOUNDBOARD_SOUNDS_ORACLE,
    } = await import("../../scripts/verify-soundboard.mjs");

    // 1. Формула эффективной громкости из RE-модуля 904054: min(soundVol * (settingVol / 100) * min(outputVol / 100, 1), 1)
    assert.equal(computeEffectiveSoundboardVolume(1.0, 100, 100), 1.0);
    assert.equal(computeEffectiveSoundboardVolume(0.5, 80, 100), 0.4);
    assert.equal(computeEffectiveSoundboardVolume(1.0, 100, 150), 1.0);
    assert.equal(computeEffectiveSoundboardVolume(0.8, 0, 100), 0.0);

    // 2. Софт-клиппинг tanh(): отсутствие жёсткого клиппинга при сложении громкой речи (0.85) и громкого SFX (0.85)
    const loudMix = softClipTanhSample(0.85, 0.85, 1.0);
    assert.ok(loudMix > 0.9 && loudMix < 1.0, `Ожидался мягкий лимитер < 1.0, получено ${loudMix}`);
    const quietMix = softClipTanhSample(0.2, 0.0, 1.0);
    assert.ok(Math.abs(quietMix - Math.tanh(0.2)) < 1e-6);

    // 3. Поток RTP Opus (20 мс = 960 отсчётов на канал @ 48 кГц, 2 канала = 1920 сэмплов) и авто-остановка
    const mixer = new SoundboardRtpMixer({
      ssrc: 17843,
      initialSequence: 100,
      initialTimestamp: 48_000,
      settingVolumePercent: 100,
      outputVolumePercent: 100,
    });

    // 2 кадра по 20 мс (2 * 1920 = 3840 stereo float сэмплов)
    const sfxSamples = new Float32Array(3840).fill(0.6);
    mixer.startSfxPlayback({
      soundId: LIVE_SOUNDBOARD_SOUNDS_ORACLE[0]?.soundId ?? "1102645616632397904",
      soundVolume: 0.9,
      pcmStereoSamples: sfxSamples,
    });
    assert.equal(mixer.isActive(), true);

    const micFrame = new Float32Array(1920).fill(0.5);
    const frame1 = mixer.mixNext20msRtpFrame(micFrame);
    assert.equal(frame1.payloadType, 120);
    assert.equal(frame1.sequence, 100);
    assert.equal(frame1.timestamp, 48_000);
    assert.equal(frame1.speakingFlags, 3); // MICROPHONE (1) | SOUND_SHARE (2)
    assert.equal(frame1.dspState, "tanh limiter");
    assert.equal(frame1.peakAbs <= 1.0, true);
    assert.equal(mixer.isActive(), true);

    const frame2 = mixer.mixNext20msRtpFrame(micFrame);
    assert.equal(frame2.sequence, 101);
    assert.equal(frame2.timestamp, 48_960);
    assert.equal(mixer.isActive(), false); // сэмпл из 2 кадров завершился автоматически!

    // 3-й кадр идёт в режиме bypass без накладных расходов микширования SFX
    const frame3 = mixer.mixNext20msRtpFrame(micFrame);
    assert.equal(frame3.sequence, 102);
    assert.equal(frame3.timestamp, 49_920);
    assert.equal(frame3.speakingFlags, 1); // только MICROPHONE (1), SOUND_SHARE снят
    assert.equal(frame3.dspState, "bypass");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1012_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1012-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1012_3_.*ok/);
  });

  it("SUB-1012-4: покрывает все 9 обязательных измерений TEST-POLICY.md и проверяет CLI scripts/verify-soundboard.mjs (коды 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluateSoundboardNineDimensions,
      redactSoundboardTelemetry,
      runSoundboardCli,
      LIVE_SOUNDBOARD_SOUNDS_ORACLE,
    } = await import("../../scripts/verify-soundboard.mjs");

    // 1. Все 9 измерений TEST-POLICY.md
    const dimReport = evaluateSoundboardNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 2. Сверка поиска звуков с эталоном soundboard-picker.js
    const spikeFiltered = spikeFilterSoundboardSounds(LIVE_SOUNDBOARD_SOUNDS_ORACLE, "хабар");
    assert.equal(spikeFiltered.length, 1);
    assert.equal(spikeFiltered[0]?.soundId, "1102645616632397904");

    // 3. Маскирование токенов в логах диагностики саундборда
    const rawLog =
      'POST /api/v9/channels/1294815650186788916/send-soundboard-sound Authorization="Bearer mfa.secret_soundboard_token_1012" token=dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactSoundboardTelemetry(rawLog);
    assert.doesNotMatch(redacted, /mfa\.secret_soundboard_token_1012/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    // 4. Проверка CLI scripts/verify-soundboard.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "soundboard-report.json");
    const outMd = join(workDir, "SOUNDBOARD-REPORT.md");
    assert.equal(runSoundboardCli(["--help"]), 0);
    const cliOk = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliOk.status, 0, `CLI завершился с ошибкой: ${cliOk.stderr}`);
    assert.ok(existsSync(outJson));
    assert.ok(existsSync(outMd));

    const cliBadArg = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--invalid-arg"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliBadArg.status, 1);

    const cliMissingOracle = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1012.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1012_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1012-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1012_4_.*ok/);
  });

  it("SUB-1012-5: убивает 5/5 контрольных мутаций в rust/crates/discord-voice/src/soundboard.rs и scripts/verify-soundboard.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1012-RS-01",
        operator: "VoiceOpSoundboardCodeTamper",
        from: "pub const VOICE_OP_SOUNDBOARD_SOUND: u8 = 18;",
        to: "pub const VOICE_OP_SOUNDBOARD_SOUND: u8 = 99;",
      },
      {
        id: "MUT-1012-RS-02",
        operator: "SpeakingFlagSoundShareTamper",
        from: "pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 1;",
        to: "pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 5;",
      },
      {
        id: "MUT-1012-RS-03",
        operator: "SoftClipTanhBypassTamper",
        from: "(mic + sfx * gain).tanh()",
        to: "(mic - sfx * gain)",
      },
      {
        id: "MUT-1012-RS-04",
        operator: "RtpTimestampStepTamper",
        from: "self.timestamp = self.timestamp.wrapping_add(step);",
        to: "self.timestamp = self.timestamp.wrapping_add(100);",
      },
      {
        id: "MUT-1012-RS-05",
        operator: "UseSoundboardPermissionBypass",
        from: "if !is_private && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
        to: "if false && !is_private && (permissions & PERM_USE_SOUNDBOARD) == 0 {",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в soundboard.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "soundboard.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
      };
    });

    const rustGate = evaluateMutationGate(rustMutants, (mutant, index) => {
      const mutantSrcPath = join(workDir, `mutant_${index + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${index + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");
      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test_failed" : "survived",
      };
    });

    const afterRustMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(afterRustMd5, originalRustMd5, "MD5 исходного файла soundboard.rs не должен измениться");
    assert.equal(
      rustGate.passed,
      true,
      `Мутационный гейт Rust не пройден: ${rustGate.verdict} (выжили: ${rustGate.survivors.join(", ")})`,
    );
    assert.equal(rustGate.killed, 5);
    assert.equal(rustGate.total, 5);
    assert.equal(rustGate.killRate, 1);

    const originalJs = readFileSync(SCRIPT_PATH, "utf8");
    const jsMutationSpecs = [
      {
        id: "MUT-1012-JS-01",
        operator: "VoiceOp18ConstantTamper",
        from: "VOICE_OP_SOUNDBOARD_SOUND: 18,",
        to: "VOICE_OP_SOUNDBOARD_SOUND: 99,",
      },
      {
        id: "MUT-1012-JS-02",
        operator: "SoundButtonActiveLabelTamper",
        from: 'active: "playing sfx",',
        to: 'active: "idle",',
      },
      {
        id: "MUT-1012-JS-03",
        operator: "SoftClipTanhFormulaTamper",
        from: "return Math.tanh(safeMic + safeSfx * safeGain);",
        to: "return 0;",
      },
      {
        id: "MUT-1012-JS-04",
        operator: "EffectiveVolumeFormulaTamper",
        from: "return Math.min(safeSound * (safeSetting / 100) * Math.min(safeOutput / 100, 1), 1);",
        to: "return 0;",
      },
      {
        id: "MUT-1012-JS-05",
        operator: "SendSoundboardMethodTamper",
        from: 'method: "POST",',
        to: 'method: "GET",',
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-soundboard.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-soundboard.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_soundboard_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_soundboard_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { SOUNDBOARD_BUTTON_STATES, SOUNDBOARD_PROTOCOL_CONSTANTS, buildSendSoundboardSoundRequest, computeEffectiveSoundboardVolume, softClipTanhSample } from ${JSON.stringify(mutantJsPath)};`,
          "assert.equal(SOUNDBOARD_PROTOCOL_CONSTANTS.VOICE_OP_SOUNDBOARD_SOUND, 18);",
          'assert.equal(SOUNDBOARD_BUTTON_STATES.active, "playing sfx");',
          "assert.ok(softClipTanhSample(0.5, 0.5, 1.0) > 0.7);",
          "assert.equal(computeEffectiveSoundboardVolume(1.0, 100, 100), 1.0);",
          'assert.equal(buildSendSoundboardSoundRequest({ channelId: "1294815650186788916", soundId: "1102645616632397904", guildId: "274093052673392641" }).method, "POST");',
        ].join("\n"),
        "utf8",
      );
      const res = spawnSync(process.execPath, [harnessPath], {
        cwd: ROOT,
        encoding: "utf8",
        timeout: 10_000,
      });
      return {
        killed: res.status !== 0,
        reason: res.status !== 0 ? "test_failed" : "survived",
      };
    });

    assert.equal(
      jsGate.passed,
      true,
      `Мутационный гейт JS не пройден: ${jsGate.verdict} (выжили: ${jsGate.survivors.join(", ")})`,
    );
    assert.equal(jsGate.killed, 5);
    assert.equal(jsGate.total, 5);
    assert.equal(jsGate.killRate, 1);
  });
});
