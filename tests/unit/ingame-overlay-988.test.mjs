// @ts-check
/**
 * Контрактные и модульные тесты глобальных хоткеев и Wayland/X11 In-Game Overlay (TASK-988):
 * - SUB-988-0: Статический контракт нативного модуля `rust/chord-gpui/src/ingame_overlay.rs`
 *   (`#![forbid(unsafe_code)]`, лимиты `scripts/quality-gate.mjs`: функции < 50 строк,
 *   модуль < 800 строк, 0 `.unwrap()`/`.expect()` в продуктовом коде, чистый `rustfmt --check --edition 2024`
 *   и компиляция `rustc --test --edition 2024 -D warnings` без предупреждений).
 * - SUB-988-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-988`,
 *   `docs/re/main-arch.md` §1.1/§2.3 `--overlay-host` и `outOfProcessOverlay`, `docs/re/backend-ipc.json`
 *   каналы `DISCORD_WINDOW_IS_ALWAYS_ON_TOP`, `DISCORD_WINDOW_SET_ALWAYS_ON_TOP`, `DISCORD_GET_MOUSE_COORDINATES`,
 *   `docs/re/native-re.md` §5 `discord_utils.node` `inputEventRegister`/`inputWatchAll`/`setObservedGamesCallback`),
 *   протокол Wayland `ext-foreign-toplevel-list-v1` и портал `org.freedesktop.portal.GlobalShortcuts`.
 * - SUB-988-2: Прозрачный клик-сквозь (`input passthrough`) в режиме HUD (`wl_surface.set_input_region`
 *   с пустым `wl_region` под Wayland и `XShapeCombineRegion` `ShapeInput` под X11, CSS `pointer-events: none`),
 *   переходы `overlay-chat-bubble` (`semi-transparent -> opaque -> typing`), глобальные хоткеи и запрет
 *   одиночных символьных биндов на скан-кодах Linux `[24..33], [38..46], [52..58]`.
 * - SUB-988-3: Отображение говорящих в верхнем левом углу (`anchor: "top-left"`, `x: 16, y: 16`,
 *   `padding: 16px`, `spacing: 8px`, индикатор активности речи `#23a55a`, Voice OP 5 `SPEAKING`)
 *   и жизненный цикл полноэкранных окон игр через `ext-foreign-toplevel-list-v1` (`toplevel -> app_id -> title -> identifier -> done -> closed`).
 * - SUB-988-4: Покрытие всех 9 измерений `TEST-POLICY.md` (позитивные, негативные, граничные Snowflake
 *   `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`,
 *   границы скан-кодов Linux, ошибки REST `401/403/404/413/429/5xx`, пустые состояния, `skeleton`/`HTTP 202`,
 *   права `VIEW_CHANNEL`/`SPEAK`/`AGE_GATED_SPACES`, fallback `Wayland -> X11`, маскирование секретов).
 * - SUB-988-5: Мутационный гейт `5/5` (`100%` kill rate) для Rust и JS модулей по контракту `scripts/quality-gate.mjs`.
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
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/ingame_overlay.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const JS_MODULE_REL_PATH = "rust/gui-spike/ui/features/ingame-overlay.js";
const JS_MODULE_PATH = join(ROOT, JS_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const MAIN_ARCH_MD_PATH = join(ROOT, "docs", "re", "main-arch.md");
const BACKEND_IPC_JSON_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const NATIVE_RE_MD_PATH = join(ROOT, "docs", "re", "native-re.md");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `ingame_overlay.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-988: Глобальные хоткеи и Wayland In-Game Overlay (ingame_overlay.rs + ingame-overlay.js)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "ingame-overlay-988-"));
    testBinPath = join(workDir, "ingame_overlay_988_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-988-0: модули rust/chord-gpui/src/ingame_overlay.rs и rust/gui-spike/ui/features/ingame-overlay.js существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Модуль ${RUST_MODULE_REL_PATH} должен существовать`,
    );
    assert.ok(
      existsSync(JS_MODULE_PATH),
      `Модуль ${JS_MODULE_REL_PATH} должен существовать`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /#!\[forbid\(unsafe_code\)\]/,
      "Rust-модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = rustSource.split("#[cfg(test)]")[0] ?? rustSource;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде ingame_overlay.rs запрещены .unwrap() и .expect()",
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

    const jsSource = readFileSync(JS_MODULE_PATH, "utf8");
    const jsLines = jsSource.split("\n").length;
    assert.ok(
      jsLines > 150 && jsLines < 800,
      `Ожидался размер JS-модуля 150..800 строк, получено: ${jsLines}`,
    );

    const secretScanRust = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    const secretScanJs = scanTextForSecrets(jsSource, JS_MODULE_REL_PATH);
    assert.equal(secretScanRust.length, 0, "В Rust-модуле не должно быть захардкоженных секретов");
    assert.equal(secretScanJs.length, 0, "В JS-модуле не должно быть захардкоженных секретов");

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
  });

  it("SUB-988-1: сверяет дизайн-токены, состояния overlay-chat-bubble, протокол ext-foreign-toplevel-list-v1 и IPC/NAPI контракты с независимыми оракулами репозитория", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      OVERLAY_HUD_LAYOUT,
      OVERLAY_DESIGN_TOKENS,
      OVERLAY_CHAT_BUBBLE_STATES,
      EXT_FOREIGN_TOPLEVEL_LIST_V1,
      GLOBAL_SHORTCUTS_PORTAL,
      DISCORD_OVERLAY_IPC_CHANNELS,
      DISCORD_UTILS_NAPI_EXPORTS,
      OVERLAY_RENDERER_EVENTS,
      LINUX_FORBIDDEN_SINGLE_SCANCODES,
      detectOverlayBackend,
      verifyOverlayOracleParity,
    } = await import("../../rust/gui-spike/ui/features/ingame-overlay.js");

    // 1. Независимый оракул: карточка TASK-988 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card988 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-988");
    assert.ok(card988, "Карточка TASK-988 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(OVERLAY_HUD_LAYOUT, card988.layout);
    assert.deepEqual(OVERLAY_DESIGN_TOKENS, card988.design_tokens);
    assert.deepEqual(OVERLAY_CHAT_BUBBLE_STATES, card988.interactive_states[0]);

    // 2. Протокол Wayland ext-foreign-toplevel-list-v1
    assert.equal(
      EXT_FOREIGN_TOPLEVEL_LIST_V1.INTERFACE_LIST,
      "ext_foreign_toplevel_list_v1",
    );
    assert.equal(
      EXT_FOREIGN_TOPLEVEL_LIST_V1.INTERFACE_HANDLE,
      "ext_foreign_toplevel_handle_v1",
    );
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.PROTOCOL_VERSION, 1);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.REQUEST_STOP, 0);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.REQUEST_DESTROY, 1);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.EVENT_TOPLEVEL, 0);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.EVENT_FINISHED, 1);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.HANDLE_EVENT_CLOSED, 0);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.HANDLE_EVENT_DONE, 1);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.HANDLE_EVENT_TITLE, 2);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.HANDLE_EVENT_APP_ID, 3);
    assert.equal(EXT_FOREIGN_TOPLEVEL_LIST_V1.HANDLE_EVENT_IDENTIFIER, 4);

    // 3. Портал глобальных хоткеев XDG Desktop Portal
    assert.equal(
      GLOBAL_SHORTCUTS_PORTAL.INTERFACE,
      "org.freedesktop.portal.GlobalShortcuts",
    );
    assert.equal(
      GLOBAL_SHORTCUTS_PORTAL.OBJECT_PATH,
      "/org/freedesktop/portal/desktop",
    );
    assert.deepEqual(GLOBAL_SHORTCUTS_PORTAL.METHODS, [
      "CreateSession",
      "BindShortcuts",
      "ListShortcuts",
    ]);
    assert.deepEqual(GLOBAL_SHORTCUTS_PORTAL.SIGNALS, [
      "Activated",
      "Deactivated",
      "ShortcutsChanged",
    ]);

    // 4. Независимые оракулы docs/re/backend-ipc.json, docs/re/main-arch.md и docs/re/native-re.md
    const ipcData = JSON.parse(readFileSync(BACKEND_IPC_JSON_PATH, "utf8"));
    const ipcNames = new Set((ipcData.channels ?? []).map((/** @type {any} */ ch) => ch.name));
    for (const ch of DISCORD_OVERLAY_IPC_CHANNELS) {
      assert.ok(ipcNames.has(ch), `Канал ${ch} должен существовать в docs/re/backend-ipc.json`);
    }

    const mainArchText = readFileSync(MAIN_ARCH_MD_PATH, "utf8");
    assert.match(mainArchText, /--overlay-host/);
    assert.match(mainArchText, /discord_overlay2\/standalone_host\.js/);
    assert.match(mainArchText, /outOfProcessOverlay/);
    assert.match(mainArchText, /Discord Overlay/);

    const nativeReText = readFileSync(NATIVE_RE_MD_PATH, "utf8");
    for (const fnName of DISCORD_UTILS_NAPI_EXPORTS) {
      assert.ok(
        nativeReText.includes(fnName),
        `Экспорт ${fnName} должен присутствовать в оракуле docs/re/native-re.md`,
      );
    }
    assert.equal(LINUX_FORBIDDEN_SINGLE_SCANCODES.length, 26); // [24..33]=10, [38..46]=9, [52..58]=7 -> 26
    assert.equal(OVERLAY_RENDERER_EVENTS.INITIALIZED, "overlay_initialized");
    assert.equal(OVERLAY_RENDERER_EVENTS.LOCKED, "overlay_locked");
    assert.equal(OVERLAY_RENDERER_EVENTS.UNLOCKED, "overlay_unlocked");
    assert.equal(OVERLAY_RENDERER_EVENTS.PIN_TOGGLED, "overlay_pin_toggled");

    // 5. Автоопределение бэкенда оверлея (Wayland ext-foreign-toplevel-list-v1 -> X11 XShape -> Headless)
    const waylandBackend = detectOverlayBackend({
      XDG_SESSION_TYPE: "wayland",
      XDG_CURRENT_DESKTOP: "KDE",
      WAYLAND_DISPLAY: "wayland-0",
      DISPLAY: ":0",
    });
    assert.equal(waylandBackend.backend, "wayland-ext-foreign-toplevel-v1");
    assert.equal(waylandBackend.passthroughMechanism, "wl_surface.set_input_region(empty)");
    assert.equal(waylandBackend.globalShortcutsBackend, "org.freedesktop.portal.GlobalShortcuts");

    const x11Backend = detectOverlayBackend({
      XDG_SESSION_TYPE: "x11",
      XDG_CURRENT_DESKTOP: "i3",
      WAYLAND_DISPLAY: "",
      DISPLAY: ":0",
    });
    assert.equal(x11Backend.backend, "x11-override-redirect-xshape");
    assert.equal(x11Backend.passthroughMechanism, "XShapeCombineRegion(ShapeInput, empty)");

    const parity = verifyOverlayOracleParity({
      kanbanBoardPath: KANBAN_BOARD_PATH,
      mainArchMdPath: MAIN_ARCH_MD_PATH,
      backendIpcJsonPath: BACKEND_IPC_JSON_PATH,
      nativeReMdPath: NATIVE_RE_MD_PATH,
    });
    assert.equal(parity.passed, true, `Несовпадение с оракулами: ${parity.reasons.join("; ")}`);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_988_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-988-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_988_1_.*ok/);
  });

  it("SUB-988-2: обеспечивает прозрачный клик-сквозь (input passthrough) в режиме HUD, переходы overlay-chat-bubble и глобальные хоткеи с защитой скан-кодов Linux", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      IngameOverlayController,
      validateLinuxGlobalKeybind,
    } = await import("../../rust/gui-spike/ui/features/ingame-overlay.js");

    const overlay = new IngameOverlayController({
      viewportWidth: 1920,
      viewportHeight: 1080,
      backend: "wayland-ext-foreign-toplevel-v1",
    });

    // 1. В режиме HUD по умолчанию: полный input passthrough (прозрачный клик-сквозь)
    const hudSnap = overlay.getSnapshot();
    assert.equal(hudSnap.mode, "hud");
    assert.equal(hudSnap.overlayEvent, "overlay_locked");
    assert.equal(hudSnap.inputPassthrough, true);
    assert.equal(hudSnap.cssPointerEvents, "none");
    assert.equal(hudSnap.background, "transparent");
    assert.equal(hudSnap.dimensions, "transparent 100vw x 100vh");
    assert.deepEqual(hudSnap.waylandInputRegionRects, []);
    assert.equal(hudSnap.chatBubbleState, "semi-transparent");

    // Пробный клик мыши в любую точку экрана (включая область спикеров и баббла чата) в режиме HUD проходит сквозь в игру
    const clickInHud1 = overlay.routePointerHit(24, 24);
    const clickInHud2 = overlay.routePointerHit(960, 540);
    assert.equal(clickInHud1.forwardedToGame, true);
    assert.equal(clickInHud1.interceptedByOverlay, false);
    assert.equal(clickInHud2.forwardedToGame, true);
    assert.equal(clickInHud2.interceptedByOverlay, false);

    // 2. Разблокировка оверлея глобальным хоткеем Shift+Backquote (toggle_overlay_lock)
    const dispatchUnlock = overlay.dispatchGlobalShortcut("Shift+Backquote");
    assert.equal(dispatchUnlock.handled, true);
    assert.equal(dispatchUnlock.action, "toggle_overlay_lock");

    const unlockedSnap = overlay.getSnapshot();
    assert.equal(unlockedSnap.mode, "interactive");
    assert.equal(unlockedSnap.overlayEvent, "overlay_unlocked");
    assert.equal(unlockedSnap.inputPassthrough, false);
    assert.equal(unlockedSnap.cssPointerEvents, "auto");
    assert.ok(unlockedSnap.waylandInputRegionRects.length > 0);

    // 3. Машина интерактивных состояний overlay-chat-bubble: semi-transparent -> opaque (hover) -> typing (active)
    assert.equal(overlay.setChatBubbleInteraction("default"), "semi-transparent");
    assert.equal(overlay.setChatBubbleInteraction("hover"), "opaque");
    assert.equal(overlay.setChatBubbleInteraction("active"), "typing");

    // Повторное нажатие Shift+Backquote возвращает режим HUD и прозрачный клик-сквозь
    overlay.dispatchGlobalShortcut("Shift+Backquote");
    const relockedSnap = overlay.getSnapshot();
    assert.equal(relockedSnap.mode, "hud");
    assert.equal(relockedSnap.inputPassthrough, true);
    assert.equal(relockedSnap.chatBubbleState, "semi-transparent");
    assert.deepEqual(relockedSnap.waylandInputRegionRects, []);

    // 4. Валидация глобальных хоткеев Linux по оракулу input_event.js / input_capture.js
    const validMute = validateLinuxGlobalKeybind({
      combo: "Ctrl+Shift+M",
      scanCodes: [29, 42, 58],
      hasModifier: true,
      mouseButtons: [],
      captureDurationMs: 420,
    });
    assert.equal(validMute.valid, true);

    // Одиночные символьные клавиши на скан-кодах Linux [24..33], [38..46], [52..58] запрещены без модификатора
    for (const forbiddenScan of [24, 33, 38, 46, 52, 58]) {
      const bareChar = validateLinuxGlobalKeybind({
        combo: `Scan${forbiddenScan}`,
        scanCodes: [forbiddenScan],
        hasModifier: false,
        mouseButtons: [],
        captureDurationMs: 200,
      });
      assert.equal(bareChar.valid, false, `Скан-код ${forbiddenScan} без модификатора должен отклоняться`);
      assert.equal(bareChar.reason, "forbidden_single_character_scancode");
    }

    // Левая кнопка мыши (код 1 на Linux) запрещена в глобальных биндах
    const leftClickBind = validateLinuxGlobalKeybind({
      combo: "Mouse1",
      scanCodes: [],
      hasModifier: false,
      mouseButtons: [1],
      captureDurationMs: 150,
    });
    assert.equal(leftClickBind.valid, false);
    assert.equal(leftClickBind.reason, "left_mouse_button_excluded");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_988_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-988-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_988_2_.*ok/);
  });

  it("SUB-988-3: отображает говорящих участников в верхнем левом углу (16px padding, 8px spacing) и отслеживает полноэкранные игры через ext-foreign-toplevel-list-v1", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      IngameOverlayController,
      renderOverlayHudHtml,
    } = await import("../../rust/gui-spike/ui/features/ingame-overlay.js");

    const overlay = new IngameOverlayController({
      viewportWidth: 1920,
      viewportHeight: 1080,
      backend: "wayland-ext-foreign-toplevel-v1",
    });

    // 1. Жизненный цикл Wayland ext-foreign-toplevel-list-v1: toplevel -> app_id -> title -> identifier -> done
    overlay.onWaylandForeignToplevelEvent({
      type: "toplevel",
      handleId: "tl-101",
    });
    overlay.onWaylandForeignToplevelEvent({
      type: "app_id",
      handleId: "tl-101",
      appId: "steam_app_730",
    });
    overlay.onWaylandForeignToplevelEvent({
      type: "title",
      handleId: "tl-101",
      title: "Counter-Strike 2",
    });
    overlay.onWaylandForeignToplevelEvent({
      type: "identifier",
      handleId: "tl-101",
      identifier: "kwin-toplevel-730-a1b2",
    });

    // До события done состояние дескриптора ещё находится в стадии накопления (pending)
    assert.equal(overlay.getAttachedToplevel(), null);

    overlay.onWaylandForeignToplevelEvent({
      type: "done",
      handleId: "tl-101",
    });
    const attached = overlay.getAttachedToplevel();
    assert.ok(attached, "После события done полноэкранная игра должна быть привязана к оверлею");
    assert.equal(attached.appId, "steam_app_730");
    assert.equal(attached.title, "Counter-Strike 2");
    assert.equal(attached.identifier, "kwin-toplevel-730-a1b2");

    // 2. Отображение говорящих в верхнем левом углу (anchor: "top-left", x=16, y=16, spacing=8)
    overlay.updateVoiceSpeakers([
      {
        userId: "100000000000000001",
        username: "Alex",
        speakingFlags: 1, // MICROPHONE
        muted: false,
        deafened: false,
        volumePercent: 100,
      },
      {
        userId: "100000000000000002",
        username: "Maria",
        speakingFlags: 2, // SOUNDSHARE
        muted: false,
        deafened: false,
        volumePercent: 125,
      },
      {
        userId: "100000000000000003",
        username: "SilentPeer",
        speakingFlags: 0,
        muted: true,
        deafened: false,
        volumePercent: 80,
      },
    ]);

    const layout = overlay.computeSpeakersTopLeftLayout();
    assert.equal(layout.anchor, "top-left");
    assert.equal(layout.originX, 16);
    assert.equal(layout.originY, 16);
    assert.equal(layout.paddingPx, 16);
    assert.equal(layout.spacingPx, 8);
    assert.equal(layout.items.length, 3);

    // Активные говорящие идут первыми, выровнены по верхнему левому углу с шагом 40 + 8 = 48px
    assert.equal(layout.items[0]?.userId, "100000000000000001");
    assert.equal(layout.items[0]?.speaking, true);
    assert.equal(layout.items[0]?.x, 16);
    assert.equal(layout.items[0]?.y, 16);
    assert.equal(layout.items[0]?.speakingRingColor, "#23a55a");

    assert.equal(layout.items[1]?.userId, "100000000000000002");
    assert.equal(layout.items[1]?.speaking, true);
    assert.equal(layout.items[1]?.x, 16);
    assert.equal(layout.items[1]?.y, 64); // 16 + 40 + 8

    assert.equal(layout.items[2]?.userId, "100000000000000003");
    assert.equal(layout.items[2]?.speaking, false);
    assert.equal(layout.items[2]?.x, 16);
    assert.equal(layout.items[2]?.y, 112); // 64 + 40 + 8

    const html = renderOverlayHudHtml(overlay.getSnapshot(), layout);
    assert.match(html, /data-container="Overlay HUD"/);
    assert.match(html, /data-anchor="top-left"/);
    assert.match(html, /data-passthrough="true"/);
    assert.match(html, /overlay-chat-bubble/);
    assert.match(html, /rgba\(43,45,49,0\.85\)/);

    // Закрытие полноэкранного окна игры через событие closed открепляет оверлей
    overlay.onWaylandForeignToplevelEvent({
      type: "closed",
      handleId: "tl-101",
    });
    assert.equal(overlay.getAttachedToplevel(), null);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_988_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-988-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_988_3_.*ok/);
  });

  it("SUB-988-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake/2000 символов/25 МБ/offset=9975/0..200%/99+, ошибки 401..5xx, права, fallback и маскирование секретов)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluateOverlayPolicyDimensions,
      redactOverlayTelemetry,
    } = await import("../../rust/gui-spike/ui/features/ingame-overlay.js");

    const report = evaluateOverlayPolicyDimensions();
    assert.equal(report.allPassed, true, `Сбой измерений TEST-POLICY: ${report.failures.join("; ")}`);
    assert.equal(report.dimensionsChecked, 9);
    assert.equal(report.dimensions.positive, true);
    assert.equal(report.dimensions.negative, true);
    assert.equal(report.dimensions.boundary, true);
    assert.equal(report.dimensions.errors, true);
    assert.equal(report.dimensions.empty, true);
    assert.equal(report.dimensions.loading, true);
    assert.equal(report.dimensions.permissions, true);
    assert.equal(report.dimensions.recovery, true);
    assert.equal(report.dimensions.observability, true);

    const sampleLog =
      'Overlay session auth="Bearer mfa.abCDef1234567890_secretTokenValue" token=dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz0123456789abcdef restore_token=fd91bc02-77a1-4b90-8812-cc9911223344';
    const redacted = redactOverlayTelemetry(sampleLog);
    assert.doesNotMatch(redacted, /mfa\.abCDef1234567890_secretTokenValue/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.doesNotMatch(redacted, /fd91bc02-77a1-4b90-8812-cc9911223344/);
    assert.match(redacted, /\[REDACTED\]/);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_988_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-988-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_988_4_.*ok/);
  });

  it("SUB-988-5: проходит мутационный гейт 5/5 (100% kill rate) для Rust и JS реализаций по контракту scripts/quality-gate.mjs", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const rustMutationSpecs = [
      {
        id: "M1-RUST-PASSTHROUGH-INVERT",
        operator: "InputPassthroughBrokenInHudMode",
        from: "Self::Hud => true,",
        to: "Self::Hud => false,",
      },
      {
        id: "M2-RUST-TOPLEFT-ORIGIN-SHIFT",
        operator: "SpeakersTopLeftAnchorCorrupted",
        from: "pub const OVERLAY_PADDING_PX: u32 = 16;",
        to: "pub const OVERLAY_PADDING_PX: u32 = 64;",
      },
      {
        id: "M3-RUST-BUBBLE-HOVER-STATE",
        operator: "ChatBubbleHoverStateCorrupted",
        from: 'Self::Hover => "opaque",',
        to: 'Self::Hover => "semi-transparent",',
      },
      {
        id: "M4-RUST-SCANCODE-GUARD-BYPASS",
        operator: "ForbiddenSingleScanCodeAllowed",
        from: "(24..=33).contains(&code) || (38..=46).contains(&code) || (52..=58).contains(&code)",
        to: "false",
      },
      {
        id: "M5-RUST-TOPLEVEL-DONE-BYPASS",
        operator: "ForeignToplevelDoneCommitSkipped",
        from: "self.committed.insert(handle_id.to_string(), state);",
        to: "let _ = state;",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна существовать в ingame_overlay.rs`,
      );
      return {
        id: spec.id,
        moduleName: "ingame_overlay.rs",
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

    assert.equal(
      rustGate.passed,
      true,
      `Мутационный гейт Rust не пройден: ${rustGate.verdict} (выжили: ${rustGate.survivors.join(", ")})`,
    );
    assert.equal(rustGate.killed, 5);
    assert.equal(rustGate.total, 5);
    assert.equal(rustGate.killRate, 1);

    const originalJs = readFileSync(JS_MODULE_PATH, "utf8");
    const jsMutationSpecs = [
      {
        id: "M1-JS-PASSTHROUGH-INVERT",
        operator: "HudInputPassthroughDisabled",
        from: 'const inputPassthrough = this.mode === "hud";',
        to: "const inputPassthrough = false;",
      },
      {
        id: "M2-JS-TOPLEFT-PADDING-SHIFT",
        operator: "TopLeftOriginPaddingShifted",
        from: "export const OVERLAY_PADDING_PX = 16;",
        to: "export const OVERLAY_PADDING_PX = 64;",
      },
      {
        id: "M3-JS-BUBBLE-HOVER-STATE",
        operator: "ChatBubbleHoverStateBroken",
        from: 'hover: "opaque",',
        to: 'hover: "semi-transparent",',
      },
      {
        id: "M4-JS-SCANCODE-BYPASS",
        operator: "LinuxSingleScanCodeGuardBypassed",
        from: "if (LINUX_FORBIDDEN_SINGLE_SCANCODES.includes(code)) {",
        to: "if (false) {",
      },
      {
        id: "M5-JS-TOPLEVEL-DONE-BYPASS",
        operator: "ForeignToplevelDoneCommitSkipped",
        from: "this.committedToplevels.set(id, { ...pending });",
        to: "void pending;",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна существовать в ingame-overlay.js`,
      );
      return {
        id: spec.id,
        moduleName: "ingame-overlay.js",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_overlay_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { IngameOverlayController, OVERLAY_PADDING_PX, OVERLAY_CHAT_BUBBLE_STATES, validateLinuxGlobalKeybind } from ${JSON.stringify(mutantJsPath)};`,
          "const ctrl = new IngameOverlayController();",
          "assert.equal(ctrl.getSnapshot().inputPassthrough, true);",
          "assert.equal(OVERLAY_PADDING_PX, 16);",
          'assert.equal(OVERLAY_CHAT_BUBBLE_STATES.hover, "opaque");',
          'assert.equal(validateLinuxGlobalKeybind({ combo: "Q", scanCodes: [24], hasModifier: false }).valid, false);',
          'ctrl.onWaylandForeignToplevelEvent({ type: "toplevel", handleId: "tl-1" });',
          'ctrl.onWaylandForeignToplevelEvent({ type: "done", handleId: "tl-1" });',
          "assert.ok(ctrl.getAttachedToplevel());",
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
