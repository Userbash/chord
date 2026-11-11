// @ts-check
/**
 * Контрактные и модульные тесты интеграции со Spotify по D-Bus MPRIS (Listen Along — TASK-1013):
 * - SUB-1013-0: Статический и компиляционный контракт `rust/discord-gpui/src/integrations/spotify_mpris.rs`
 *   и `scripts/verify-spotify.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде,
 *   соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1013-1: Сверка карточек `TASK-1013`, `TASK-1013A`, `TASK-1013B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#1db954`, `#3f4147`), геометрии
 *   `Spotify Activity Card` (`100% x auto`, `padding: 8px`, `spacing: 6px`), интерактивных состояний
 *   `listen-along-btn` (`spotify green` -> `brighten` -> `sync playback`), `mpris-event`
 *   (`idle` -> `detect track` -> `status broadcast`), `listen-together` (`button` -> `brighten` -> `sync started`),
 *   события `PRESENCE_UPDATE` (`107` в `docs/re/gateway-protocol.json`), схемы `/home/sanya/re-deep/gateway-schema.json`,
 *   маршрута `POST /content-inventory/users/@me/spotify` (`MY_SPOTIFY_CONTENT_INVENTORY` в `rest-static.json`)
 *   и модулей RE-дампа (`107750`, `170699`, `210528`, `272984`, `402857`, `655116`, `970111`, `981616`).
 * - SUB-1013-2: Подписка на сигналы D-Bus `org.mpris.MediaPlayer2.spotify` (`PropertiesChanged` на
 *   `/org/mpris/MediaPlayer2` и `NameOwnerChanged`), извлечение метаданных `xesam:title` (с усечением > 128
 *   символов до `125 + "..."`), `xesam:artist` (до 5 исполнителей через `"; "` с фильтрацией `";"`),
 *   `xesam:album`, `mpris:trackid` (`/com/spotify/track/<22>` и `spotify:track:<22>`), `mpris:length` (мкс -> мс),
 *   `mpris:artUrl` (`https://i.scdn.co/image/<hex>` -> `spotify:<hex>`) и расчёт `timestamps.{start,end}`
 *   с порогом джиттера `1500` мс.
 * - SUB-1013-3: Публикация активности типа `2` (`LISTENING`) в Gateway Opcode `3` (`STATUS_UPDATE`) с флагами
 *   `PLAY | SYNC = 48` (`0x30`), `id = "spotify:1"`, `party.id = "spotify:<user_id>"`, `sync_id`, воспроизведение
 *   4 живых захватов Spotify из `/home/sanya/re-deep/gw-proto/msgs-run1-57992_5112.jsonl` (`s=4`, `s=32`) и
 *   `msgs-run2-95974_8269.jsonl` (`s=514`, `s=557`), а также сброс статуса при паузе/остановке или закрытии Spotify.
 * - SUB-1013-4: Кнопка «Слушать вместе» (Listen Along) и синхронизация воспроизведения через Spotify Connect Web API
 *   (`PUT https://api.spotify.com/v1/me/player/play`), расчёт прогресса таймлайна трека в реальном времени,
 *   покрытие всех 9 обязательных измерений `TEST-POLICY.md` и проверка CLI `scripts/verify-spotify.mjs` (коды `0 / 1 / 2`).
 * - SUB-1013-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/integrations/spotify_mpris.rs`
 *   и `scripts/verify-spotify.mjs` по контракту `scripts/quality-gate.mjs`.
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
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/integrations/spotify_mpris.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-spotify.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GATEWAY_PROTOCOL_JSON_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const RE_GATEWAY_SCHEMA_PATH = "/home/sanya/re-deep/gateway-schema.json";
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/integrations/spotify_mpris.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1013: Интеграция со Spotify по D-Bus MPRIS (Listen Along — rust/discord-gpui/src/integrations/spotify_mpris.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "spotify-mpris-1013-"));
    testBinPath = join(workDir, "spotify_mpris_1013_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1013-0: модули rust/discord-gpui/src/integrations/spotify_mpris.rs и scripts/verify-spotify.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде spotify_mpris.rs запрещены .unwrap() и .expect()",
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
      `Встроенные тесты spotify_mpris.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1013-1: сверяет дизайн-токены, раскладку Spotify Activity Card, интерактивные состояния и артефакты RE-дампа с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      SPOTIFY_ACTIVITY_CARD_LAYOUT,
      SPOTIFY_MPRIS_BRIDGE_LAYOUT,
      SPOTIFY_PROFILE_CARD_LAYOUT,
      SPOTIFY_DESIGN_TOKENS,
      LISTEN_ALONG_BTN_STATES,
      MPRIS_EVENT_STATES,
      LISTEN_TOGETHER_STATES,
      SPOTIFY_PROTOCOL_CONSTANTS,
      LIVE_SPOTIFY_ACTIVITIES_ORACLE,
      resolveListenAlongButtonState,
      resolveMprisEventState,
      verifySpotifyOracleParity,
    } = await import("../../scripts/verify-spotify.mjs");

    // 1. Оракул: карточки TASK-1013, TASK-1013A, TASK-1013B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1013 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1013");
    const card1013A = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1013A");
    const card1013B = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1013B");
    assert.ok(card1013, "Карточка TASK-1013 должна присутствовать в waves/kanban-board.json");
    assert.ok(card1013A, "Карточка TASK-1013A должна присутствовать в waves/kanban-board.json");
    assert.ok(card1013B, "Карточка TASK-1013B должна присутствовать в waves/kanban-board.json");

    assert.deepEqual(SPOTIFY_ACTIVITY_CARD_LAYOUT, card1013.layout);
    assert.deepEqual(SPOTIFY_MPRIS_BRIDGE_LAYOUT, card1013A.layout);
    assert.deepEqual(SPOTIFY_PROFILE_CARD_LAYOUT, card1013B.layout);
    assert.deepEqual(SPOTIFY_DESIGN_TOKENS, card1013.design_tokens);
    assert.deepEqual(LISTEN_ALONG_BTN_STATES, card1013.interactive_states[0]);
    assert.deepEqual(MPRIS_EVENT_STATES, card1013A.interactive_states[0]);
    assert.deepEqual(LISTEN_TOGETHER_STATES, card1013B.interactive_states[0]);

    // 2. Интерактивные состояния listen-along-btn и mpris-event
    assert.deepEqual(resolveListenAlongButtonState({ syncing: false, hovered: false }), {
      element: "listen-along-btn",
      state: "default",
      label: "spotify green",
    });
    assert.deepEqual(resolveListenAlongButtonState({ syncing: false, hovered: true }), {
      element: "listen-along-btn",
      state: "hover",
      label: "brighten",
    });
    assert.deepEqual(resolveListenAlongButtonState({ syncing: true, hovered: false }), {
      element: "listen-along-btn",
      state: "active",
      label: "sync playback",
    });

    assert.deepEqual(resolveMprisEventState({ broadcasting: false, detecting: false }), {
      element: "mpris-event",
      state: "default",
      label: "idle",
    });
    assert.deepEqual(resolveMprisEventState({ broadcasting: false, detecting: true }), {
      element: "mpris-event",
      state: "hover",
      label: "detect track",
    });
    assert.deepEqual(resolveMprisEventState({ broadcasting: true, detecting: false }), {
      element: "mpris-event",
      state: "active",
      label: "status broadcast",
    });

    // 3. Оракул docs/re/gateway-protocol.json (PRESENCE_UPDATE = 107) и константы протокола
    const gwProto = JSON.parse(readFileSync(GATEWAY_PROTOCOL_JSON_PATH, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.PRESENCE_UPDATE, 107);
    assert.equal(SPOTIFY_PROTOCOL_CONSTANTS.DBUS_BUS_NAME, "org.mpris.MediaPlayer2.spotify");
    assert.equal(SPOTIFY_PROTOCOL_CONSTANTS.DBUS_OBJECT_PATH, "/org/mpris/MediaPlayer2");
    assert.equal(SPOTIFY_PROTOCOL_CONSTANTS.GATEWAY_OPCODE_STATUS_UPDATE, 3);
    assert.equal(SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_TYPE_LISTENING, 2);
    assert.equal(SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_FLAGS_PLAY_SYNC, 48);
    assert.equal(LIVE_SPOTIFY_ACTIVITIES_ORACLE.length, 4);

    // 4. Полная сверка паритета с оракулами
    const parity = verifySpotifyOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      gatewayProtocolPath: GATEWAY_PROTOCOL_JSON_PATH,
      reGatewaySchemaPath: existsSync(RE_GATEWAY_SCHEMA_PATH) ? RE_GATEWAY_SCHEMA_PATH : undefined,
      reRestStaticPath: existsSync(RE_REST_STATIC_PATH) ? RE_REST_STATIC_PATH : undefined,
      reModulesIndexPath: existsSync(RE_MODULES_INDEX_PATH) ? RE_MODULES_INDEX_PATH : undefined,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.presenceUpdateCount, 107);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1013_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1013-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1013_1_.*ok/);
  });

  it("SUB-1013-2: разбирает сигналы D-Bus org.mpris.MediaPlayer2.spotify (PropertiesChanged) и извлекает метаданные трека с нормализацией обложек и таймстемпов", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      extractSpotifyTrackId,
      normalizeSpotifyArtAsset,
      formatSpotifyDetails,
      formatSpotifyStateArtists,
      parseMprisMetadata,
    } = await import("../../scripts/verify-spotify.mjs");

    // 1. Извлечение 22-символьного base62 track_id из mpris:trackid
    assert.equal(
      extractSpotifyTrackId("/com/spotify/track/3cdUEnMVspkAfx6nnxSdTl"),
      "3cdUEnMVspkAfx6nnxSdTl",
    );
    assert.equal(
      extractSpotifyTrackId("spotify:track:6IPwKM3fUUzlElbvKw2sKl"),
      "6IPwKM3fUUzlElbvKw2sKl",
    );
    assert.equal(
      extractSpotifyTrackId("/org/mpris/MediaPlayer2/TrackList/NoTrack"),
      null,
    );
    assert.equal(extractSpotifyTrackId("spotify:local:artist:album:track:180"), null);

    // 2. Нормализация обложки альбома mpris:artUrl -> spotify:<40hex>
    assert.equal(
      normalizeSpotifyArtAsset("https://i.scdn.co/image/ab67616d0000b2736700e765df4db64f4b0c6276"),
      "spotify:ab67616d0000b2736700e765df4db64f4b0c6276",
    );
    assert.equal(
      normalizeSpotifyArtAsset("https://open.spotify.com/image/ab67616d0000b273deade9f354749841f09feca2"),
      "spotify:ab67616d0000b273deade9f354749841f09feca2",
    );
    assert.equal(
      normalizeSpotifyArtAsset("spotify:ab67616d0000b2736700e765df4db64f4b0c6276"),
      "spotify:ab67616d0000b2736700e765df4db64f4b0c6276",
    );

    // 3. Форматирование названия трека (до 128 символов, усечение до 125 + "...") и до 5 артистов через "; "
    const longTitle = "A".repeat(140);
    const truncated = formatSpotifyDetails(longTitle);
    assert.equal(truncated.length, 128);
    assert.ok(truncated.endsWith("..."));

    const artists = ["Miyagi;Test", "Andy Panda", "A3", "A4", "A5", "A6_ignored"];
    assert.equal(
      formatSpotifyStateArtists(artists),
      "MiyagiTest; Andy Panda; A3; A4; A5",
    );

    // 4. Полный разбор сигнала D-Bus PropertiesChanged с проверкой порога джиттера 1500 мс
    const nowMs = 1_792_390_379_228;
    const mprisSignal = {
      interfaceName: "org.mpris.MediaPlayer2.Player",
      playbackStatus: "Playing",
      positionUs: 13_471_000, // 13 471 мс
      metadata: {
        "mpris:trackid": "/com/spotify/track/3cdUEnMVspkAfx6nnxSdTl",
        "mpris:length": 230_222_000, // 230 222 мс
        "mpris:artUrl": "https://i.scdn.co/image/ab67616d0000b2736700e765df4db64f4b0c6276",
        "xesam:title": "Freeman",
        "xesam:artist": ["Miyagi & Andy Panda"],
        "xesam:album": "Freeman",
      },
    };
    const track1 = parseMprisMetadata(mprisSignal, nowMs, null);
    assert.ok(track1);
    assert.equal(track1.trackId, "3cdUEnMVspkAfx6nnxSdTl");
    assert.equal(track1.title, "Freeman");
    assert.equal(track1.artistsJoined, "Miyagi & Andy Panda");
    assert.equal(track1.album, "Freeman");
    assert.equal(track1.largeImage, "spotify:ab67616d0000b2736700e765df4db64f4b0c6276");
    assert.equal(track1.startMs, 1_792_390_365_757);
    assert.equal(track1.endMs, 1_792_390_595_979);
    assert.equal(track1.isPlaying, true);
    assert.equal(track1.isLocal, false);

    // Повторное событие с микро-джиттером 400 мс (< 1500 мс) сохраняет прежний startMs
    const trackJitter = parseMprisMetadata(
      { ...mprisSignal, positionUs: 13_071_000 },
      nowMs,
      track1.startMs,
    );
    assert.equal(trackJitter?.startMs, track1.startMs);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1013_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1013-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1013_2_.*ok/);
  });

  it("SUB-1013-3: формирует Gateway Opcode 3 (STATUS_UPDATE) с активностью типа 2 (LISTENING), воспроизводит 4 живых кадра из RE-дампа и сбрасывает статус при закрытии Spotify", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      LIVE_SPOTIFY_ACTIVITIES_ORACLE,
      SpotifyMprisBridgeEngine,
    } = await import("../../scripts/verify-spotify.mjs");

    const engine = new SpotifyMprisBridgeEngine({
      currentUserId: "366982137616007169",
      sessionId: "5658ed3b2c7e19a11f36e2b86b0db208",
      isPremium: true,
      activeDeviceId: "dev_linux_pipewire_01",
    });

    // 1. Обработка сигнала D-Bus PropertiesChanged -> генерация кадра Gateway Opcode 3 (STATUS_UPDATE)
    const frame = engine.handleDbusPropertiesChanged(
      {
        interfaceName: "org.mpris.MediaPlayer2.Player",
        playbackStatus: "Playing",
        positionUs: 13_471_000,
        metadata: {
          "mpris:trackid": "/com/spotify/track/3cdUEnMVspkAfx6nnxSdTl",
          "mpris:length": 230_222_000,
          "mpris:artUrl": "https://i.scdn.co/image/ab67616d0000b2736700e765df4db64f4b0c6276",
          "xesam:title": "Freeman",
          "xesam:artist": ["Miyagi & Andy Panda"],
          "xesam:album": "Freeman",
        },
      },
      1_792_390_379_228,
    );

    assert.equal(frame.op, 3);
    assert.equal(frame.d.status, "online");
    assert.equal(frame.d.afk, false);
    assert.equal(frame.d.activities.length, 1);

    const act = frame.d.activities[0];
    const oracle0 = LIVE_SPOTIFY_ACTIVITIES_ORACLE[0];
    assert.ok(act && oracle0);
    assert.equal(act.type, 2);
    assert.equal(act.name, "Spotify");
    assert.equal(act.id, "spotify:1");
    assert.equal(act.flags, 48);
    assert.equal(act.sync_id, oracle0.sync_id);
    assert.equal(act.details, oracle0.details);
    assert.equal(act.state, oracle0.state);
    assert.equal(act.party.id, oracle0.party.id);
    assert.equal(act.assets.large_image, oracle0.assets.large_image);
    assert.equal(act.assets.large_text, oracle0.assets.large_text);
    assert.equal(act.timestamps.start, oracle0.timestamps.start);
    assert.equal(act.timestamps.end, oracle0.timestamps.end);

    // 2. Воспроизведение всех 4 живых активностей Spotify из RE-дампа
    for (const liveAct of LIVE_SPOTIFY_ACTIVITIES_ORACLE) {
      const parsed = engine.ingestGatewayPresenceActivity(liveAct);
      assert.equal(parsed.isSpotifyListening, true);
      assert.equal(parsed.syncId, liveAct.sync_id);
      assert.equal(parsed.partyId, liveAct.party.id);
    }

    // 3. Пауза или исчезновение владельца шины D-Bus (NameOwnerChanged new_owner="") очищает активность
    const clearedFrame = engine.handleDbusNameOwnerChanged(
      "org.mpris.MediaPlayer2.spotify",
      ":1.94",
      "",
    );
    assert.equal(clearedFrame.op, 3);
    assert.equal(clearedFrame.d.activities.length, 0);
    assert.equal(engine.getCurrentActivity(), null);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1013_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1013-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1013_3_.*ok/);
  });

  it("SUB-1013-4: проверяет кнопку «Слушать вместе» (Listen Along), таймлайн трека, 9 обязательных измерений TEST-POLICY.md и CLI scripts/verify-spotify.mjs (коды 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      LIVE_SPOTIFY_ACTIVITIES_ORACLE,
      SpotifyMprisBridgeEngine,
      computeTrackProgress,
      evaluateSpotifyNineDimensions,
      redactSpotifyTelemetry,
      runSpotifyCli,
    } = await import("../../scripts/verify-spotify.mjs");

    // 1. Расчёт прогресса таймлайна трека и заморозка при паузе
    const progMid = computeTrackProgress(
      { start: 1_000_000, end: 1_200_000 },
      1_100_000,
      false,
    );
    assert.equal(progMid.progressPercent, 50);
    assert.equal(progMid.elapsedFormatted, "01:40");
    assert.equal(progMid.totalFormatted, "03:20");

    const progPaused = computeTrackProgress(
      { start: 1_000_000, end: 1_200_000 },
      1_180_000,
      true,
      1_050_000,
    );
    assert.equal(progPaused.progressPercent, 25);
    assert.equal(progPaused.elapsedFormatted, "00:50");

    // 2. Синхронизация Listen Along («Слушать вместе») по живой активности друга из RE-дампа
    const engine = new SpotifyMprisBridgeEngine({
      currentUserId: "479659568322969611",
      sessionId: "a5d7f3932c76e53bdc11c2cbc1ebeea2",
      isPremium: true,
      activeDeviceId: "dev_pipewire_main",
    });
    const hostActivity = LIVE_SPOTIFY_ACTIVITIES_ORACLE[1]; // girl in red - we fell in love in october
    assert.ok(hostActivity);

    const syncReq = engine.startListenAlong(
      "293808922463174658",
      hostActivity,
      1_792_390_650_036,
    );
    assert.equal(syncReq.method, "PUT");
    assert.equal(
      syncReq.url,
      "https://api.spotify.com/v1/me/player/play?device_id=dev_pipewire_main",
    );
    assert.deepEqual(syncReq.body, {
      uris: ["spotify:track:6IPwKM3fUUzlElbvKw2sKl"],
      position_ms: 60_000,
    });
    assert.equal(engine.getListenAlongButtonState(false).label, "sync playback");

    // Смена трека у хоста вызывает автоматический ресинк
    const nextHostActivity = LIVE_SPOTIFY_ACTIVITIES_ORACLE[2];
    assert.ok(nextHostActivity);
    const resyncReq = engine.syncWithHostActivity(
      "293808922463174658",
      nextHostActivity,
      1_792_394_432_210,
    );
    assert.ok(resyncReq);
    assert.deepEqual(resyncReq.body.uris, ["spotify:track:3e3ZxVIhHIzltQCZ26zr66"]);
    assert.equal(resyncReq.body.position_ms, 10_000);

    // 3. Все 9 измерений TEST-POLICY.md
    const dimReport = evaluateSpotifyNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 4. Маскирование токенов в логах диагностики
    const rawLog =
      'PUT https://api.spotify.com/v1/me/player/play Authorization="Bearer BQD_spotify_secret_oauth_token_1013" discord_token=mfa.secret_discord_token_1013';
    const redacted = redactSpotifyTelemetry(rawLog);
    assert.doesNotMatch(redacted, /BQD_spotify_secret_oauth_token_1013/);
    assert.doesNotMatch(redacted, /mfa\.secret_discord_token_1013/);
    assert.match(redacted, /\[REDACTED\]/);

    // 5. Проверка CLI scripts/verify-spotify.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "spotify-report.json");
    const outMd = join(workDir, "SPOTIFY-REPORT.md");
    assert.equal(runSpotifyCli(["--help"]), 0);
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
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1013.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1013_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1013-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1013_4_.*ok/);
  });

  it("SUB-1013-5: убивает 5/5 контрольных мутаций в rust/discord-gpui/src/integrations/spotify_mpris.rs и scripts/verify-spotify.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1013-RS-01",
        operator: "ActivityTypeListeningTamper",
        from: "pub const ACTIVITY_TYPE_LISTENING: u8 = 2;",
        to: "pub const ACTIVITY_TYPE_LISTENING: u8 = 0;",
      },
      {
        id: "MUT-1013-RS-02",
        operator: "GatewayStatusUpdateOpcodeTamper",
        from: "pub const GATEWAY_OPCODE_STATUS_UPDATE: u8 = 3;",
        to: "pub const GATEWAY_OPCODE_STATUS_UPDATE: u8 = 1;",
      },
      {
        id: "MUT-1013-RS-03",
        operator: "ActivityFlagsPlaySyncTamper",
        from: "pub const ACTIVITY_FLAGS_PLAY_SYNC: u32 = ACTIVITY_FLAG_PLAY | ACTIVITY_FLAG_SYNC;",
        to: "pub const ACTIVITY_FLAGS_PLAY_SYNC: u32 = 0;",
      },
      {
        id: "MUT-1013-RS-04",
        operator: "TimestampEndCalculationTamper",
        from: "let end_ms = start_ms.saturating_add(duration_ms);",
        to: "let end_ms = start_ms;",
      },
      {
        id: "MUT-1013-RS-05",
        operator: "ListenAlongPremiumCheckBypass",
        from: "if !is_premium {",
        to: "if false && !is_premium {",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в spotify_mpris.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "spotify_mpris.rs",
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
    assert.equal(afterRustMd5, originalRustMd5, "MD5 исходного файла spotify_mpris.rs не должен измениться");
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
        id: "MUT-1013-JS-01",
        operator: "ListeningActivityTypeTamper",
        from: "ACTIVITY_TYPE_LISTENING: 2,",
        to: "ACTIVITY_TYPE_LISTENING: 0,",
      },
      {
        id: "MUT-1013-JS-02",
        operator: "ListenAlongActiveStateTamper",
        from: 'active: "sync playback",',
        to: 'active: "idle",',
      },
      {
        id: "MUT-1013-JS-03",
        operator: "ListenAlongHttpMethodTamper",
        from: 'const method = "PUT";',
        to: 'const method = "POST";',
      },
      {
        id: "MUT-1013-JS-04",
        operator: "TrackProgressClampTamper",
        from: "const progressPercent = Math.min(100, Math.max(0, Math.round((elapsedMs * 100) / durationMs)));",
        to: "const progressPercent = 0;",
      },
      {
        id: "MUT-1013-JS-05",
        operator: "PremiumRequirementBypass",
        from: "if (!params.isPremium) {",
        to: "if (false && !params.isPremium) {",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-spotify.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-spotify.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_spotify_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_spotify_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { SPOTIFY_PROTOCOL_CONSTANTS, LISTEN_ALONG_BTN_STATES, buildListenAlongPlayRequest, computeTrackProgress } from ${JSON.stringify(mutantJsPath)};`,
          "assert.equal(SPOTIFY_PROTOCOL_CONSTANTS.ACTIVITY_TYPE_LISTENING, 2);",
          'assert.equal(LISTEN_ALONG_BTN_STATES.active, "sync playback");',
          'assert.equal(buildListenAlongPlayRequest({ syncId: "3cdUEnMVspkAfx6nnxSdTl", partyId: "spotify:366982137616007169", deviceId: "dev1", isPremium: true, positionMs: 5000 }).method, "PUT");',
          "assert.equal(computeTrackProgress({ start: 1000, end: 3000 }, 2000, false).progressPercent, 50);",
          'assert.throws(() => buildListenAlongPlayRequest({ syncId: "3cdUEnMVspkAfx6nnxSdTl", partyId: "spotify:366982137616007169", deviceId: "dev1", isPremium: false, positionMs: 0 }));',
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
