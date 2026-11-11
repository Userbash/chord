// @ts-check
/**
 * Контрактные и модульные тесты автоматического перевода в AFK-канал при простое (TASK-1010):
 * - SUB-1010-0: Статический и компиляционный контракт `rust/discord-gpui/src/voice/afk_monitor.rs`
 *   и `scripts/verify-afk.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде,
 *   соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1010-1: Сверка карточек `TASK-1010`, `TASK-1010A`, `TASK-1010B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#f0b232`, `#3f4147`), раскладки `Voice State Service`
 *   (`background daemon`, `0px`, `0px`), интерактивных состояний (`afk-badge`, `idle-detect`, `voice-afk`),
 *   D-Bus оракула `fixtures/dbus/power.json` (`org.freedesktop.ScreenSaver`, `GetSessionIdleTime -> 1234`,
 *   `org.freedesktop.login1.Manager.PrepareForSleep`), `docs/re/gateway-protocol.json` (`VOICE_STATE_UPDATE = 8`),
 *   `docs/re/gateway-schema.json` и модулей RE-дампа (`885576`, `576991`, `926456`, `185255`, `260509`).
 * - SUB-1010-2: Системный детектор бездействия (`TASK-1010A`): опрос времени простоя через Wayland
 *   `ext-idle-notifier-v1`, D-Bus `org.freedesktop.ScreenSaver` (`GetSessionIdleTime`) и fallback на X11
 *   (`XScreenSaverQueryInfo`), подавление простоя при воспроизведении медиа/стриме/речи (`SPEAKING`),
 *   обработка `PrepareForSleep` (`suspend`/`resume`) и блокировки экрана (`lock-screen`/`unlock-screen`).
 * - SUB-1010-3: Перемещение в `afk_channel_id` по тайм-ауту (`TASK-1010B`): сверка с `guild.afk_timeout`
 *   (`60, 300, 900, 1800, 3600` с) на 7 реальных гильдиях из `/home/sanya/re-deep/gw-proto/msgs-run1-57992_5112.jsonl`,
 *   формирование кадра Gateway Opcode 4 (`VOICE_STATE_UPDATE`) с принудительным `self_mute: true` и
 *   `bypass_idle_update: true`, защита от повторного спама Opcode 4 и пропуска Stage-каналов.
 * - SUB-1010-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   Snowflake `0`/`u64::MAX`, `idle_ms` на границе `afk_timeout`, `60..3600` с, `2000` символов, `10` файлов/`25 МБ`,
 *   `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки D-Bus `UnknownMethod` и HTTP `400/401/403/404/413/429/5xx`;
 *   пустые состояния без `afk_channel_id`; `skeleton`/`HTTP 202`; права `VIEW_CHANNEL`/`CONNECT`/`SPEAK`/`MOVE_MEMBERS`
 *   и `AGE_GATED_SPACES`; восстановление и откат при обрыве шлюза; маскирование секретов) и проверка CLI `scripts/verify-afk.mjs` (`0 / 1 / 2`).
 * - SUB-1010-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/voice/afk_monitor.rs`
 *   и `scripts/verify-afk.mjs` по контракту `scripts/quality-gate.mjs`.
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
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/voice/afk_monitor.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-afk.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DBUS_POWER_FIXTURE_PATH = join(ROOT, "fixtures", "dbus", "power.json");
const GATEWAY_PROTOCOL_JSON_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const GATEWAY_SCHEMA_JSON_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/voice/afk_monitor.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1010: Автоматический перевод в AFK-канал при простое (rust/discord-gpui/src/voice/afk_monitor.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "voice-afk-monitor-1010-"));
    testBinPath = join(workDir, "voice_afk_monitor_1010_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1010-0: модули rust/discord-gpui/src/voice/afk_monitor.rs и scripts/verify-afk.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде afk_monitor.rs запрещены .unwrap() и .expect()",
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
      `Встроенные тесты afk_monitor.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1010-1: сверяет дизайн-токены, раскладку Voice State Service, состояния afk-badge/idle-detect/voice-afk, D-Bus фикстуру и события Gateway с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      AFK_SERVICE_LAYOUT,
      AFK_DESIGN_TOKENS,
      AFK_BADGE_STATES,
      IDLE_DETECT_STATES,
      VOICE_AFK_STATES,
      AFK_PROTOCOL_CONSTANTS,
      LIVE_READY_GUILDS_AFK_ORACLE,
      LIVE_VOICE_STATE_EVENTS_ORACLE,
      resolveAfkBadgeState,
      verifyAfkOracleParity,
    } = await import("../../scripts/verify-afk.mjs");

    // 1. Оракул: карточки TASK-1010, TASK-1010A, TASK-1010B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1010 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1010");
    const card1010a = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1010A");
    const card1010b = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1010B");
    assert.ok(card1010, "Карточка TASK-1010 должна присутствовать в waves/kanban-board.json");
    assert.ok(card1010a, "Карточка TASK-1010A должна присутствовать в waves/kanban-board.json");
    assert.ok(card1010b, "Карточка TASK-1010B должна присутствовать в waves/kanban-board.json");

    assert.deepEqual(AFK_SERVICE_LAYOUT, card1010.layout);
    assert.deepEqual(AFK_DESIGN_TOKENS, card1010.design_tokens);
    assert.deepEqual(AFK_BADGE_STATES, card1010.interactive_states[0]);
    assert.deepEqual(IDLE_DETECT_STATES, card1010a.interactive_states[0]);
    assert.deepEqual(VOICE_AFK_STATES, card1010b.interactive_states[0]);

    // 2. Интерактивные состояния afk-badge: hidden -> visible -> moved to afk
    assert.deepEqual(resolveAfkBadgeState({ movedToAfk: false, hovered: false }), {
      element: "afk-badge",
      state: "default",
      label: "hidden",
    });
    assert.deepEqual(resolveAfkBadgeState({ movedToAfk: false, hovered: true }), {
      element: "afk-badge",
      state: "hover",
      label: "visible",
    });
    assert.deepEqual(resolveAfkBadgeState({ movedToAfk: true, hovered: false }), {
      element: "afk-badge",
      state: "active",
      label: "moved to afk",
    });

    // 3. Оракул fixtures/dbus/power.json (org.freedesktop.ScreenSaver GetSessionIdleTime = 1234)
    const dbusPower = JSON.parse(readFileSync(DBUS_POWER_FIXTURE_PATH, "utf8"));
    const ssService = dbusPower.services.find(
      (/** @type {any} */ s) => s.destination === "org.freedesktop.ScreenSaver",
    );
    assert.ok(ssService);
    const getIdleMethod = ssService.methods.find(
      (/** @type {any} */ m) => m.member === "GetSessionIdleTime",
    );
    assert.equal(getIdleMethod.reply, 1234);
    assert.equal(AFK_PROTOCOL_CONSTANTS.DBUS_SCREENSAVER_DEST, "org.freedesktop.ScreenSaver");
    assert.equal(AFK_PROTOCOL_CONSTANTS.DBUS_IDLE_METHOD, "GetSessionIdleTime");
    assert.equal(AFK_PROTOCOL_CONSTANTS.WAYLAND_IDLE_PROTOCOL, "ext-idle-notifier-v1");

    // 4. Оракул docs/re/gateway-protocol.json (VOICE_STATE_UPDATE = 8) и константы Opcode 4
    const gwProto = JSON.parse(readFileSync(GATEWAY_PROTOCOL_JSON_PATH, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.VOICE_STATE_UPDATE, 8);
    assert.equal(AFK_PROTOCOL_CONSTANTS.GATEWAY_OP_VOICE_STATE_UPDATE, 4);
    assert.deepEqual(AFK_PROTOCOL_CONSTANTS.ALLOWED_AFK_TIMEOUTS_SECS, [60, 300, 900, 1800, 3600]);
    assert.equal(AFK_PROTOCOL_CONSTANTS.VOICE_HARD_IDLE_DISCONNECT_MS, 18_000_000);
    assert.equal(LIVE_READY_GUILDS_AFK_ORACLE.length, 7);
    assert.equal(LIVE_VOICE_STATE_EVENTS_ORACLE.length, 2);

    // 5. Полная сверка паритета с оракулами
    const parity = verifyAfkOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      dbusPowerPath: DBUS_POWER_FIXTURE_PATH,
      gatewayProtocolPath: GATEWAY_PROTOCOL_JSON_PATH,
      gatewaySchemaPath: GATEWAY_SCHEMA_JSON_PATH,
      reModulesIndexPath: existsSync(RE_MODULES_INDEX_PATH) ? RE_MODULES_INDEX_PATH : undefined,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.gatewayVoiceStateUpdateCount, 8);
    assert.equal(parity.dbusFixtureIdleSeconds, 1234);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1010_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1010-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1010_1_.*ok/);
  });

  it("SUB-1010-2: проверяет системный детектор бездействия (Wayland ext-idle-notifier-v1 -> D-Bus ScreenSaver -> X11 fallback) и фильтрацию активности медиа/стримов/речи", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      selectIdleBackend,
      computeEffectiveIdleMs,
      resolveIdleDetectState,
    } = await import("../../scripts/verify-afk.mjs");

    // 1. Выбор бэкенда с приоритетом Wayland ext-idle-notifier-v1 -> D-Bus ScreenSaver -> X11 XScreenSaver
    assert.equal(
      selectIdleBackend({
        waylandExtIdleAvailable: true,
        dbusScreenSaverAvailable: true,
        x11ScreenSaverAvailable: true,
      }),
      "wayland_ext_idle_notifier_v1",
    );
    assert.equal(
      selectIdleBackend({
        waylandExtIdleAvailable: false,
        dbusScreenSaverAvailable: true,
        x11ScreenSaverAvailable: true,
      }),
      "dbus_freedesktop_screensaver",
    );
    assert.equal(
      selectIdleBackend({
        waylandExtIdleAvailable: false,
        dbusScreenSaverAvailable: false,
        x11ScreenSaverAvailable: true,
      }),
      "x11_screensaver",
    );
    assert.equal(
      selectIdleBackend({
        waylandExtIdleAvailable: false,
        dbusScreenSaverAvailable: false,
        x11ScreenSaverAvailable: false,
      }),
      "monotonic_fallback",
    );

    // 2. Подавление простоя при активном видео/аудио стриме или речи пользователя (SPEAKING != 0)
    assert.equal(
      computeEffectiveIdleMs({
        rawIdleMs: 1_234_000,
        mediaPlaying: true,
        activeStreamCount: 0,
        userSpeaking: false,
        systemSuspended: false,
        screenLocked: false,
      }),
      0,
    );
    assert.equal(
      computeEffectiveIdleMs({
        rawIdleMs: 1_234_000,
        mediaPlaying: false,
        activeStreamCount: 1,
        userSpeaking: false,
        systemSuspended: false,
        screenLocked: false,
      }),
      0,
    );
    assert.equal(
      computeEffectiveIdleMs({
        rawIdleMs: 1_234_000,
        mediaPlaying: false,
        activeStreamCount: 0,
        userSpeaking: true,
        systemSuspended: false,
        screenLocked: false,
      }),
      0,
    );

    // 3. Штатный замер простоя без медиа-ингибиторов
    assert.equal(
      computeEffectiveIdleMs({
        rawIdleMs: 1_234_000,
        mediaPlaying: false,
        activeStreamCount: 0,
        userSpeaking: false,
        systemSuspended: false,
        screenLocked: false,
      }),
      1_234_000,
    );

    // 4. Состояния элемента idle-detect: active -> counting -> idle triggered
    assert.equal(resolveIdleDetectState(0, 300_000).label, "active");
    assert.equal(resolveIdleDetectState(120_000, 300_000).label, "counting");
    assert.equal(resolveIdleDetectState(300_000, 300_000).label, "idle triggered");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1010_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1010-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1010_2_.*ok/);
  });

  it("SUB-1010-3: проверяет перевод в afk_channel_id по guild.afk_timeout (1..60 мин), отправку Gateway Opcode 4 с self_mute: true и воспроизведение на реальных гильдиях RE-дампа", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      LIVE_READY_GUILDS_AFK_ORACLE,
      buildVoiceStateUpdateOpcode4,
      VoiceAfkMonitorEngine,
    } = await import("../../scripts/verify-afk.mjs");

    // 1. Формирование кадра Gateway Opcode 4 (VOICE_STATE_UPDATE) на перевод в AFK-канал
    const op4Frame = buildVoiceStateUpdateOpcode4({
      guildId: "274093052673392641",
      channelId: "274955515333836801",
      selfMute: true,
      selfDeaf: false,
      selfVideo: false,
      flags: 2,
    });
    assert.equal(op4Frame.op, 4);
    assert.deepEqual(op4Frame.d, {
      guild_id: "274093052673392641",
      channel_id: "274955515333836801",
      self_mute: true,
      self_deaf: false,
      self_video: false,
      flags: 2,
    });

    // 2. Сценарий на реальной гильдии MONOLIT (id=274093052673392641, afk_channel_id=274955515333836801, afk_timeout=3600s)
    const monolitGuild = LIVE_READY_GUILDS_AFK_ORACLE.find(
      (g) => g.id === "274093052673392641",
    );
    assert.ok(monolitGuild);
    assert.equal(monolitGuild.afkChannelId, "274955515333836801");
    assert.equal(monolitGuild.afkTimeoutSecs, 3600);

    const monitor = new VoiceAfkMonitorEngine({
      guildId: monolitGuild.id,
      currentChannelId: "351403515723120651",
      afkChannelId: monolitGuild.afkChannelId,
      afkTimeoutSecs: monolitGuild.afkTimeoutSecs,
      selfDeaf: false,
      isStageChannel: false,
    });

    // За 1 мс до тайм-аута (3 599 999 мс) — ещё в обычном канале, Opcode 4 не отправляется
    const beforeTimeout = monitor.evaluateIdleTick({
      rawIdleMs: 3_599_999,
      mediaPlaying: false,
      activeStreamCount: 0,
      userSpeaking: false,
      systemSuspended: false,
      screenLocked: false,
    });
    assert.equal(beforeTimeout.shouldMoveToAfk, false);
    assert.equal(beforeTimeout.opcode4Payload, null);
    assert.equal(beforeTimeout.voiceAfkState, "warn afk");

    // Ровно на пороге 3 600 000 мс (60 мин) — отправка Opcode 4 с self_mute: true
    const atTimeout = monitor.evaluateIdleTick({
      rawIdleMs: 3_600_000,
      mediaPlaying: false,
      activeStreamCount: true ? 0 : 1,
      userSpeaking: false,
      systemSuspended: false,
      screenLocked: false,
    });
    assert.equal(atTimeout.shouldMoveToAfk, true);
    assert.equal(atTimeout.bypassIdleUpdate, true);
    assert.ok(atTimeout.opcode4Payload);
    assert.equal(atTimeout.opcode4Payload.op, 4);
    assert.equal(atTimeout.opcode4Payload.d.guild_id, "274093052673392641");
    assert.equal(atTimeout.opcode4Payload.d.channel_id, "274955515333836801");
    assert.equal(atTimeout.opcode4Payload.d.self_mute, true);
    assert.equal(atTimeout.opcode4Payload.d.self_video, false);
    assert.equal(atTimeout.afkBadgeState, "moved to afk");
    assert.equal(atTimeout.voiceAfkState, "moved to afk");

    // Повторный тик в AFK-канале не должен спамить вторым Opcode 4
    const duplicateTick = monitor.evaluateIdleTick({
      rawIdleMs: 3_610_000,
      mediaPlaying: false,
      activeStreamCount: 0,
      userSpeaking: false,
      systemSuspended: false,
      screenLocked: false,
    });
    assert.equal(duplicateTick.shouldMoveToAfk, false);
    assert.equal(duplicateTick.opcode4Payload, null);

    // 3. Гильдия XOS (id=942355007271997450) без afk_channel_id (null) — никогда не переводит в AFK
    const xosGuild = LIVE_READY_GUILDS_AFK_ORACLE.find(
      (g) => g.id === "942355007271997450",
    );
    assert.ok(xosGuild);
    assert.equal(xosGuild.afkChannelId, null);
    const noAfkMonitor = new VoiceAfkMonitorEngine({
      guildId: xosGuild.id,
      currentChannelId: "1294815650186788916",
      afkChannelId: xosGuild.afkChannelId,
      afkTimeoutSecs: xosGuild.afkTimeoutSecs,
    });
    const noAfkRes = noAfkMonitor.evaluateIdleTick({
      rawIdleMs: 900_000,
      mediaPlaying: false,
      activeStreamCount: 0,
      userSpeaking: false,
      systemSuspended: false,
      screenLocked: false,
    });
    assert.equal(noAfkRes.shouldMoveToAfk, false);
    assert.equal(noAfkRes.opcode4Payload, null);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1010_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1010-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1010_3_.*ok/);
  });

  it("SUB-1010-4: покрывает все 9 обязательных измерений TEST-POLICY.md и проверяет CLI scripts/verify-afk.mjs (коды 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluateAfkNineDimensions,
      redactAfkTelemetry,
      runAfkCli,
    } = await import("../../scripts/verify-afk.mjs");

    // 1. Все 9 измерений TEST-POLICY.md
    const dimReport = evaluateAfkNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 2. Маскирование секретов и сессионных токенов в логах диагностики
    const rawLog =
      'Gateway Op 4 VOICE_STATE_UPDATE guild=274093052673392641 Authorization="Bearer mfa.secret_afk_token_1010" session_id="273df5662dc828eb460b491b0f520aba" token=MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactAfkTelemetry(rawLog);
    assert.doesNotMatch(redacted, /mfa\.secret_afk_token_1010/);
    assert.doesNotMatch(redacted, /273df5662dc828eb460b491b0f520aba/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    // 3. Проверка CLI scripts/verify-afk.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "afk-report.json");
    const outMd = join(workDir, "AFK-REPORT.md");
    assert.equal(runAfkCli(["--help"]), 0);
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
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1010.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1010_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1010-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1010_4_.*ok/);
  });

  it("SUB-1010-5: убивает 5/5 контрольных мутаций в rust/discord-gpui/src/voice/afk_monitor.rs и scripts/verify-afk.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1010-RS-01",
        operator: "GatewayOpcode4Tamper",
        from: "pub const GATEWAY_OP_VOICE_STATE_UPDATE: u8 = 4;",
        to: "pub const GATEWAY_OP_VOICE_STATE_UPDATE: u8 = 3;",
      },
      {
        id: "MUT-1010-RS-02",
        operator: "AfkForceSelfMuteTamper",
        from: "self_mute: true,",
        to: "self_mute: false,",
      },
      {
        id: "MUT-1010-RS-03",
        operator: "MediaPlaybackInhibitionBypass",
        from: "if input.media_playing || input.active_stream_count > 0 || input.user_speaking {",
        to: "if false && (input.media_playing || input.active_stream_count > 0 || input.user_speaking) {",
      },
      {
        id: "MUT-1010-RS-04",
        operator: "AfkTimeoutThresholdInversion",
        from: "effective_idle_ms >= timeout_ms",
        to: "effective_idle_ms < timeout_ms",
      },
      {
        id: "MUT-1010-RS-05",
        operator: "DuplicateOpcode4SpamGuardRemoval",
        from: "if self.moved_to_afk || current_channel == afk_channel {",
        to: "if false && (self.moved_to_afk || current_channel == afk_channel) {",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в afk_monitor.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "afk_monitor.rs",
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
    assert.equal(afterRustMd5, originalRustMd5, "MD5 исходного файла afk_monitor.rs не должен измениться");
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
        id: "MUT-1010-JS-01",
        operator: "Opcode4NumberTamper",
        from: "GATEWAY_OP_VOICE_STATE_UPDATE: 4,",
        to: "GATEWAY_OP_VOICE_STATE_UPDATE: 2,",
      },
      {
        id: "MUT-1010-JS-02",
        operator: "AfkBadgeActiveStateTamper",
        from: 'active: "moved to afk",',
        to: 'active: "hidden",',
      },
      {
        id: "MUT-1010-JS-03",
        operator: "MediaInhibitionTamper",
        from: "if (opts.mediaPlaying || (opts.activeStreamCount ?? 0) > 0 || opts.userSpeaking) {",
        to: "if (false) {",
      },
      {
        id: "MUT-1010-JS-04",
        operator: "AfkMoveSelfMuteTamper",
        from: "selfMute: true,",
        to: "selfMute: false,",
      },
      {
        id: "MUT-1010-JS-05",
        operator: "DuplicateOpcode4GuardTamper",
        from: "if (this.movedToAfk || this.currentChannelId === this.afkChannelId) {",
        to: "if (false) {",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-afk.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-afk.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_afk_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_afk_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { AFK_BADGE_STATES, AFK_PROTOCOL_CONSTANTS, computeEffectiveIdleMs, VoiceAfkMonitorEngine } from ${JSON.stringify(mutantJsPath)};`,
          'assert.equal(AFK_BADGE_STATES.active, "moved to afk");',
          "assert.equal(AFK_PROTOCOL_CONSTANTS.GATEWAY_OP_VOICE_STATE_UPDATE, 4);",
          "assert.equal(computeEffectiveIdleMs({ rawIdleMs: 60000, mediaPlaying: true, activeStreamCount: 0, userSpeaking: false }), 0);",
          'const eng = new VoiceAfkMonitorEngine({ guildId: "274093052673392641", currentChannelId: "351403515723120651", afkChannelId: "274955515333836801", afkTimeoutSecs: 60 });',
          "const r1 = eng.evaluateIdleTick({ rawIdleMs: 60000, mediaPlaying: false, activeStreamCount: 0, userSpeaking: false });",
          "assert.equal(r1.shouldMoveToAfk, true);",
          "assert.equal(r1.opcode4Payload?.op, 4);",
          "assert.equal(r1.opcode4Payload?.d.self_mute, true);",
          "const r2 = eng.evaluateIdleTick({ rawIdleMs: 70000, mediaPlaying: false, activeStreamCount: 0, userSpeaking: false });",
          "assert.equal(r2.shouldMoveToAfk, false);",
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
