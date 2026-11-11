// @ts-check
/**
 * Контрактные и модульные тесты интерактивного GPUI-плеера голосовых сообщений `VoiceMessagePlayer` (TASK-1003B):
 * - SUB-1003B-0: Статический и компиляционный контракт `rust/discord-gpui/src/chat/voice_player.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на модуль, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1003B-1: Сверка карточки `TASK-1003B` в `waves/kanban-board.json` (`container: "Waveform Bar"`,
 *   `dimensions: "280px x 48px"`, `padding: "8px"`, `spacing: "6px"`, дизайн-токены `#2b2d31`, `#383a40`,
 *   `#ffffff`, `#23a55a`, `#3f4147`, интерактивные состояния `scrub-bar`: `muted bars -> hover time -> seek position`),
 *   артефактов RE-оракула `/home/sanya/re-deep/voice/ui7/` (`log.json`, `voice-run.log`, `15-popout.json`),
 *   `waves/RW-40/ATTACH-RENDER-RE.md` (§6 `tG` `@6144184`, §9.1 `@6133343`, `@5935271`),
 *   `waves/RW-40/DESIGN/ATTACHMENTS-RENDER-SPEC.md` (§4.2 `.attachment-voice-card`) и `ru-catalog.json`
 *   (`AlHqHT`, `3XohGn`, `LgCPMt`, `c8U6xd`).
 * - SUB-1003B-2: GPUI шейдер/холст волны (`WaveformShaderUniforms`, `WaveformCanvasQuad`, `WaveformBarRenderSpec`,
 *   `build_gpui_waveform_canvas_quads`) — декодирование 256-байтного/живого base64 `waveform`, рендеринг
 *   32 / 64 / 128 полосок волны с динамической подсветкой прогресса (`#23a55a` для `played`, `#383a40` для `muted`,
 *   `#ffffff` для `highlight`), расчёт ширины холста по длительности и числа отыгранных сегментов `t_`.
 * - SUB-1003B-3: Мгновенная перемотка кликом и перетаскиванием по волне (`seek_by_click_x`, `seek_by_drag_ratio`,
 *   предпросмотр времени при наведении `hover_time_at_x`, состояние `ScrubBarState::SeekPosition`, событие
 *   `MEDIA_PLAYBACK_POSITION_UPDATE`) и переключение кнопки скорости `1x -> 1.5x -> 2x -> 1x` с сохранением
 *   высоты тона (`apply_wsola_pitch_preserving_speed`, `MEDIA_PLAYBACK_RATE_UPDATE`,
 *   `VOICE_MESSAGE_PLAYBACK_STARTED` с автопаузой предыдущего плеера).
 * - SUB-1003B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   `64/128` полосок, `280px x 48px`, Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`,
 *   `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки `400/401/403/404/413/429 retry_after_ms/5xx`;
 *   пустые/тишина; прогрев `HTTP 202`; права `VIEW_CHANNEL`/`AGE_GATED_SPACES`; refresh CDN URL `?ex=`;
 *   маскирование секретов `mfa.***`).
 * - SUB-1003B-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
const VOICE_PLAYER_REL_PATH = "rust/discord-gpui/src/chat/voice_player.rs";
const VOICE_PLAYER_PATH = join(ROOT, VOICE_PLAYER_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const ATTACH_RENDER_RE_PATH = join(
  ROOT,
  "waves",
  "RW-40",
  "ATTACH-RENDER-RE.md",
);
const ATTACH_RENDER_SPEC_PATH = join(
  ROOT,
  "waves",
  "RW-40",
  "DESIGN",
  "ATTACHMENTS-RENDER-SPEC.md",
);
const RE_DEEP_UI7_DIR = "/home/sanya/re-deep/voice/ui7";
const RE_DEEP_RU_CATALOG_PATH = "/home/sanya/re-deep/ru-catalog.json";

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
  flags: 8192,
});

/** @type {string} */
let workDir = "";
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

/**
 * Независимый JS-оракул расчёта ширины холста волны по длительности `d` из RE-бандла `@6133343` (`tf`):
 * `6 * Math.floor(((d <= 0.5 ? 40 : d >= 45 ? 294 : ((Math.min(d, 45) - 0.5) / 44.5) * 254 + 40) + 4) / 6) - 4`.
 * @param {number} durationSecs
 * @returns {number}
 */
function oracleComputeWaveformCanvasWidthPx(durationSecs) {
  const raw =
    durationSecs <= 0.5
      ? 40
      : durationSecs >= 45
        ? 294
        : ((Math.min(durationSecs, 45) - 0.5) / 44.5) * 254 + 40;
  return 6 * Math.floor((raw + 4) / 6) - 4;
}

describe("TASK-1003B: Интерактивный плеер VoiceMessagePlayer (полоски волны 64/128, прогресс, перемотка кликом и скорость 1x/1.5x/2x)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "voice-player-1003b-"));
    playerTestBinPath = join(workDir, "voice_player_1003b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1003B-0: модуль voice_player.rs существует, запрещает unsafe, соблюдает лимиты (<50 строк/fn, <800 строк/файл), проходит rustfmt 2024 и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(VOICE_PLAYER_PATH),
      `Модуль ${VOICE_PLAYER_REL_PATH} должен существовать`,
    );

    const source = readFileSync(VOICE_PLAYER_PATH, "utf8");
    assert.match(
      source,
      /^#!\[forbid\(unsafe_code\)\]/m,
      `Модуль ${VOICE_PLAYER_REL_PATH} обязан содержать #![forbid(unsafe_code)]`,
    );

    const prodSection = source.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      `В продакшен-коде модуля ${VOICE_PLAYER_REL_PATH} запрещены .unwrap() и .expect()`,
    );

    const sizeCheck = checkRustSourceSizeLimits(VOICE_PLAYER_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода в ${VOICE_PLAYER_REL_PATH}: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла ${VOICE_PLAYER_REL_PATH} 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(source, VOICE_PLAYER_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `В исходнике ${VOICE_PLAYER_REL_PATH} не должно быть незамаскированных секретов`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", VOICE_PLAYER_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл для ${VOICE_PLAYER_REL_PATH}:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(
      VOICE_PLAYER_PATH,
      playerTestBinPath,
    );
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings для ${VOICE_PLAYER_REL_PATH}:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора в ${VOICE_PLAYER_REL_PATH}:\n${compiled.stderr}`,
    );

    const runAll = runCompiledRustTests(playerTestBinPath);
    assert.equal(
      runAll.status,
      0,
      `Встроенные тесты ${VOICE_PLAYER_REL_PATH} упали:\n${runAll.stdout}\n${runAll.stderr}`,
    );
    assert.match(runAll.stdout, /test result: ok\./);
  });

  it("SUB-1003B-1: сверяет карточку TASK-1003B в waves/kanban-board.json, дизайн-токены (#2b2d31, #383a40, #ffffff, #23a55a, #3f4147), состояния scrub-bar и RE-оракулы (re-deep/voice/ui7, ATTACH-RENDER-RE.md, ATTACHMENTS-RENDER-SPEC.md, ru-catalog.json)", () => {
    assert.ok(
      existsSync(playerTestBinPath),
      "Бинарник voice_player должен быть собран",
    );

    // 1. Независимый оракул: карточка TASK-1003B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1003B = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1003B",
    );
    assert.ok(
      card1003B,
      "Карточка TASK-1003B должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1003B.department, "G2 Frontend");
    assert.deepEqual(card1003B.layout, {
      container: "Waveform Bar",
      dimensions: "280px x 48px",
      padding: "8px",
      spacing: "6px",
    });
    assert.deepEqual(card1003B.design_tokens, {
      background: "#2b2d31",
      surface: "#383a40",
      text_primary: "#ffffff",
      accent: "#23a55a",
      border: "#3f4147",
    });
    assert.deepEqual(card1003B.interactive_states[0], {
      element: "scrub-bar",
      default: "muted bars",
      hover: "hover time",
      active: "seek position",
    });

    // 2. Независимый оракул: /home/sanya/re-deep/voice/ui7/ и ru-catalog.json
    if (existsSync(RE_DEEP_UI7_DIR)) {
      const ui7LogPath = join(RE_DEEP_UI7_DIR, "log.json");
      const ui7VoiceRunPath = join(RE_DEEP_UI7_DIR, "voice-run.log");
      assert.ok(existsSync(ui7LogPath), "В re-deep/voice/ui7 должен быть log.json");
      assert.ok(
        existsSync(ui7VoiceRunPath),
        "В re-deep/voice/ui7 должен быть voice-run.log",
      );
      const ui7VoiceRun = readFileSync(ui7VoiceRunPath, "utf8");
      assert.ok(
        ui7VoiceRun.includes("requireModule(discord_voice)"),
        "В voice-run.log должен быть зафиксирован вызов requireModule(discord_voice)",
      );
    }

    if (existsSync(RE_DEEP_RU_CATALOG_PATH)) {
      const ruCatalog = JSON.parse(readFileSync(RE_DEEP_RU_CATALOG_PATH, "utf8"));
      assert.deepEqual(ruCatalog["AlHqHT"], ["Воспроизвести голосовое сообщение"]);
      assert.deepEqual(ruCatalog["3XohGn"], [
        "Поставить голосовое сообщение на паузу",
      ]);
      assert.deepEqual(ruCatalog["c8U6xd"], ["Голосовое сообщение"]);
    }

    // 3. Независимый оракул: waves/RW-40/ATTACH-RENDER-RE.md и ATTACHMENTS-RENDER-SPEC.md
    const attachRenderRe = readFileSync(ATTACH_RENDER_RE_PATH, "utf8");
    assert.ok(attachRenderRe.includes("IS_VOICE_MESSAGE=8192"));
    assert.ok(
      attachRenderRe.includes(LIVE_VOICE_ATTACHMENT_ORACLE.attachmentId),
    );
    assert.ok(attachRenderRe.includes(LIVE_VOICE_ATTACHMENT_ORACLE.waveform));
    assert.ok(attachRenderRe.includes("6133343"));
    assert.ok(attachRenderRe.includes("6144184"));

    const attachRenderSpec = readFileSync(ATTACH_RENDER_SPEC_PATH, "utf8");
    assert.ok(attachRenderSpec.includes('data-media-type="VOICE_MESSAGE"'));
    assert.ok(attachRenderSpec.includes("attachment-voice-card"));
    assert.ok(attachRenderSpec.includes("decodeWaveform"));

    // 4. Сверка констант и дизайн-токенов TASK-1003B в исходнике Rust-модуля
    const playerSource = readFileSync(VOICE_PLAYER_PATH, "utf8");
    for (const tokenVal of Object.values(card1003B.design_tokens)) {
      assert.ok(
        playerSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1003B должен быть зафиксирован в ${VOICE_PLAYER_REL_PATH}`,
      );
    }
    assert.ok(playerSource.includes("Waveform Bar"));
    assert.ok(playerSource.includes("280px x 48px"));
    assert.ok(playerSource.includes("scrub-bar"));
    assert.ok(playerSource.includes("muted bars"));
    assert.ok(playerSource.includes("hover time"));
    assert.ok(playerSource.includes("seek position"));

    const run = runCompiledRustTests(playerTestBinPath, "test_sub_1003b_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003B-1 в voice_player упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003b_1_.*ok/);
  });

  it("SUB-1003B-2: рендерит 64/128 полосок волны на GPUI-холсте/шейдере с динамической подсветкой прогресса (#23a55a), расчётом ширины холста и отыгранных сегментов", () => {
    assert.ok(
      existsSync(playerTestBinPath),
      "Бинарник voice_player должен быть собран",
    );

    const bars64 = oracleDecodeWaveformBars(
      LIVE_VOICE_ATTACHMENT_ORACLE.waveform,
      64,
    );
    const bars128 = oracleDecodeWaveformBars(
      LIVE_VOICE_ATTACHMENT_ORACLE.waveform,
      128,
    );
    assert.equal(bars64.length, 64);
    assert.equal(bars128.length, 128);
    assert.equal(bars64[0], 0);
    assert.ok(bars64.some((v) => v > 0.05 && v <= 1.0));
    assert.ok(bars128.some((v) => v > 0.05 && v <= 1.0));

    assert.equal(oracleComputeWaveformCanvasWidthPx(0.3), 38);
    assert.equal(oracleComputeWaveformCanvasWidthPx(45.0), 290);
    assert.equal(
      oracleComputeWaveformCanvasWidthPx(LIVE_VOICE_ATTACHMENT_ORACLE.durationSecs),
      80,
    );

    const playerSource = readFileSync(VOICE_PLAYER_PATH, "utf8");
    for (const requiredSymbol of [
      "BARS_COUNT_64",
      "BARS_COUNT_128",
      "WaveformBarState",
      "WaveformBarRenderSpec",
      "WaveformShaderUniforms",
      "WaveformCanvasQuad",
      "decode_waveform_bars",
      "compute_bar_count_for_width",
      "compute_waveform_canvas_width_px",
      "compute_played_segment_count",
      "build_gpui_waveform_canvas_quads",
    ]) {
      assert.match(
        playerSource,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле voice_player.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(playerTestBinPath, "test_sub_1003b_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003B-2 в voice_player упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003b_2_.*ok/);
  });

  it("SUB-1003B-3: выполняет мгновенную перемотку кликом/перетаскиванием по волне (seek_by_click_x, hover_time_at_x) и переключение скорости 1x / 1.5x / 2x с сохранением высоты тона (WSOLA)", () => {
    assert.ok(
      existsSync(playerTestBinPath),
      "Бинарник voice_player должен быть собран",
    );

    const playerSource = readFileSync(VOICE_PLAYER_PATH, "utf8");
    for (const requiredSymbol of [
      "PlaybackSpeed",
      "ScrubBarState",
      "VoiceMessagePlayer",
      "seek_by_click_x",
      "seek_by_drag_ratio",
      "hover_time_at_x",
      "cycle_playback_speed",
      "apply_wsola_pitch_preserving_speed",
      "build_playback_position_update_json",
      "build_playback_rate_update_json",
    ]) {
      assert.match(
        playerSource,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле voice_player.rs должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(playerTestBinPath, "test_sub_1003b_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003B-3 в voice_player упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003b_3_.*ok/);
  });

  it("SUB-1003B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные 64/128 полосок, Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, ошибки 400..5xx, тишина, HTTP 202, права доступа, refresh CDN и маскирование секретов)", () => {
    assert.ok(
      existsSync(playerTestBinPath),
      "Бинарник voice_player должен быть собран",
    );

    const run = runCompiledRustTests(playerTestBinPath, "test_sub_1003b_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1003B-4 в voice_player упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1003b_4_.*ok/);
  });

  it("SUB-1003B-5: убивает 5/5 контрольных мутаций в voice_player.rs (нарушение цикла скорости 1x/1.5x/2x, подмена акцентного цвета #23a55a, инверсия перемотки кликом, инверсия подсветки прогресса полосок, искажение питча при смене скорости)", () => {
    assert.ok(
      existsSync(VOICE_PLAYER_PATH),
      "Модуль voice_player.rs должен существовать",
    );
    const originalRust = readFileSync(VOICE_PLAYER_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-1003B-RS-01",
        operator: "SpeedCycleOrderBrokenTamper",
        from: "Self::OneX => Self::OnePointFiveX,",
        to: "Self::OneX => Self::OneX,",
      },
      {
        id: "MUT-1003B-RS-02",
        operator: "AccentColorToken23a55aTamper",
        from: 'pub const TOKEN_ACCENT: &str = "#23a55a";',
        to: 'pub const TOKEN_ACCENT: &str = "#000000";',
      },
      {
        id: "MUT-1003B-RS-03",
        operator: "ClickSeekRatioInvertedTamper",
        from: "let ratio = (click_x / width_px).clamp(0.0, 1.0);",
        to: "let ratio = (1.0 - click_x / width_px).clamp(0.0, 1.0);",
      },
      {
        id: "MUT-1003B-RS-04",
        operator: "PlayedBarComparisonInvertedTamper",
        from: "let played = progress_ratio > 0.0 && bar_ratio < progress_ratio;",
        to: "let played = progress_ratio > 0.0 && bar_ratio > progress_ratio;",
      },
      {
        id: "MUT-1003B-RS-05",
        operator: "PitchPreservationBrokenOnSpeedChangeTamper",
        from: "let preserved_pitch_hz = input_pitch_hz;",
        to: "let preserved_pitch_hz = input_pitch_hz * (speed.multiplier() as f32);",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в voice_player.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "voice_player.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `voice_player_mutant_${idx + 1}.rs`);
      const mutantBinPath = join(
        workDir,
        `voice_player_mutant_${idx + 1}_bin`,
      );
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
      .update(readFileSync(VOICE_PLAYER_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла voice_player.rs не должен измениться после мутационного гейта",
    );
    assert.equal(
      gateResult.passed,
      true,
      `Мутационный гейт для voice_player.rs не пройден (выжили: ${gateResult.survivors.join(", ")})`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1.0);
  });
});
