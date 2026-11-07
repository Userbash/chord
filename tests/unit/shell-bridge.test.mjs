// @ts-check
/**
 * T-236 (FE-P0-09): контракт реестра фич оболочки UI и единого моста контроллеров (`shell-bridge.js`).
 *
 * Проверяет:
 * - замороженный список слоёв `SHELL_MODAL_LAYERS`;
 * - регистрацию контроллеров `registerController(layerId, controllerInstance)` и валидацию `layerId`;
 * - `openLayer(layerId, payload)` с взаимным исключением 5 модальных окон
 *   (`quick-switcher`, `settings-modal`, `profile-modal`, `invite-modal`, `forward-modal`)
 *   и сохранением стека `activeOverlayStack` в порядке LIFO;
 * - `closeTopLayer()` и `closeAllLayers()` в порядке LIFO;
 * - `handleGlobalKeyDown(event)` через `normalizeKeyCombo(event)` (`Ctrl+K`, `Ctrl+F`, `Escape`);
 * - делегирование `onDispatchIpc` и `onNavigate`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  SHELL_MODAL_LAYERS,
  createShellFeatureBridge,
} from "../../rust/gui-spike/ui/features/shell-bridge.js";

/**
 * Создаёт стаб контроллера слоя со счётчиками вызовов `open` и `close`.
 * @param {string} id
 * @param {string[]} [log]
 */
function makeMockController(id, log = []) {
  /** @type {unknown[]} */
  const openPayloads = [];
  let openCount = 0;
  let closeCount = 0;
  return {
    id,
    openPayloads,
    get openCount() {
      return openCount;
    },
    get closeCount() {
      return closeCount;
    },
    /** @param {unknown} [payload] */
    open(payload) {
      openCount += 1;
      openPayloads.push(payload);
      log.push(`open:${id}`);
    },
    close() {
      closeCount += 1;
      log.push(`close:${id}`);
    },
  };
}

describe("SHELL_MODAL_LAYERS", () => {
  it("содержит точный упорядоченный список из 10 слоёв оболочки и заморожен", () => {
    assert.deepEqual(SHELL_MODAL_LAYERS, [
      "quick-switcher",
      "settings-modal",
      "profile-modal",
      "invite-modal",
      "forward-modal",
      "expression-picker",
      "context-menu",
      "threads-drawer",
      "search-panel",
      "inbox-popover",
    ]);
    assert.equal(Object.isFrozen(SHELL_MODAL_LAYERS), true);
    assert.throws(() => {
      // @ts-expect-error проверка заморозки в рантайме
      SHELL_MODAL_LAYERS.push("custom-layer");
    }, TypeError);
  });
});

describe("createShellFeatureBridge — регистрация и стек слоёв", () => {
  it("регистрирует контроллеры валидных слоёв и отклоняет неизвестный layerId", () => {
    const bridge = createShellFeatureBridge({});
    const qs = makeMockController("quick-switcher");

    bridge.registerController("quick-switcher", qs);
    assert.equal(bridge.getController("quick-switcher"), qs);

    assert.throws(() => {
      bridge.registerController("unknown-layer", makeMockController("unknown"));
    }, Error);
    assert.throws(() => {
      bridge.registerController("", makeMockController("empty"));
    }, Error);
  });

  it("openLayer открывает слой, передаёт payload в controller.open и кладёт layerId на вершину activeOverlayStack", () => {
    const log = /** @type {string[]} */ ([]);
    const bridge = createShellFeatureBridge({});
    const searchCtrl = makeMockController("search-panel", log);
    const threadsCtrl = makeMockController("threads-drawer", log);

    bridge.registerController("search-panel", searchCtrl);
    bridge.registerController("threads-drawer", threadsCtrl);

    bridge.openLayer("search-panel", { query: "from:alice" });
    bridge.openLayer("threads-drawer", { channelId: "42" });

    assert.equal(searchCtrl.openCount, 1);
    assert.deepEqual(searchCtrl.openPayloads, [{ query: "from:alice" }]);
    assert.equal(threadsCtrl.openCount, 1);
    assert.deepEqual(threadsCtrl.openPayloads, [{ channelId: "42" }]);
    assert.deepEqual(bridge.activeOverlayStack, ["search-panel", "threads-drawer"]);
    assert.deepEqual(log, ["open:search-panel", "open:threads-drawer"]);
  });

  it("взаимно исключающие модалки закрывают предыдущую открытую модалку перед открытием новой", () => {
    const log = /** @type {string[]} */ ([]);
    const bridge = createShellFeatureBridge({});
    const qsCtrl = makeMockController("quick-switcher", log);
    const settingsCtrl = makeMockController("settings-modal", log);
    const profileCtrl = makeMockController("profile-modal", log);
    const inviteCtrl = makeMockController("invite-modal", log);
    const forwardCtrl = makeMockController("forward-modal", log);
    const searchCtrl = makeMockController("search-panel", log);

    bridge.registerController("quick-switcher", qsCtrl);
    bridge.registerController("settings-modal", settingsCtrl);
    bridge.registerController("profile-modal", profileCtrl);
    bridge.registerController("invite-modal", inviteCtrl);
    bridge.registerController("forward-modal", forwardCtrl);
    bridge.registerController("search-panel", searchCtrl);

    bridge.openLayer("search-panel", { q: "test" });
    bridge.openLayer("quick-switcher", { initialQuery: "#" });
    assert.deepEqual(bridge.activeOverlayStack, ["search-panel", "quick-switcher"]);

    // Открытие settings-modal должно сначала закрыть quick-switcher, но оставить search-panel
    bridge.openLayer("settings-modal", "voice");
    assert.equal(qsCtrl.closeCount, 1);
    assert.equal(settingsCtrl.openCount, 1);
    assert.deepEqual(settingsCtrl.openPayloads, ["voice"]);
    assert.deepEqual(bridge.activeOverlayStack, ["search-panel", "settings-modal"]);

    // Цепочка переключений остальных взаимно исключающих модалок
    bridge.openLayer("profile-modal", { userId: "1" });
    assert.equal(settingsCtrl.closeCount, 1);
    assert.deepEqual(bridge.activeOverlayStack, ["search-panel", "profile-modal"]);

    bridge.openLayer("invite-modal", { guildId: "2" });
    assert.equal(profileCtrl.closeCount, 1);
    assert.deepEqual(bridge.activeOverlayStack, ["search-panel", "invite-modal"]);

    bridge.openLayer("forward-modal", { messageId: "3" });
    assert.equal(inviteCtrl.closeCount, 1);
    assert.deepEqual(bridge.activeOverlayStack, ["search-panel", "forward-modal"]);

    assert.deepEqual(log, [
      "open:search-panel",
      "open:quick-switcher",
      "close:quick-switcher",
      "open:settings-modal",
      "close:settings-modal",
      "open:profile-modal",
      "close:profile-modal",
      "open:invite-modal",
      "close:invite-modal",
      "open:forward-modal",
    ]);
  });

  it("closeTopLayer и closeAllLayers закрывают открытые слои в строгом порядке LIFO", () => {
    const log = /** @type {string[]} */ ([]);
    const bridge = createShellFeatureBridge({});
    const searchCtrl = makeMockController("search-panel", log);
    const inviteCtrl = makeMockController("invite-modal", log);
    const menuCtrl = makeMockController("context-menu", log);
    const pickerCtrl = makeMockController("expression-picker", log);

    bridge.registerController("search-panel", searchCtrl);
    bridge.registerController("invite-modal", inviteCtrl);
    bridge.registerController("context-menu", menuCtrl);
    bridge.registerController("expression-picker", pickerCtrl);

    assert.equal(bridge.closeTopLayer(), null);

    bridge.openLayer("search-panel");
    bridge.openLayer("invite-modal");
    bridge.openLayer("context-menu");
    bridge.openLayer("expression-picker");
    assert.deepEqual(bridge.activeOverlayStack, [
      "search-panel",
      "invite-modal",
      "context-menu",
      "expression-picker",
    ]);

    const firstClosed = bridge.closeTopLayer();
    assert.equal(firstClosed, "expression-picker");
    assert.equal(pickerCtrl.closeCount, 1);
    assert.deepEqual(bridge.activeOverlayStack, [
      "search-panel",
      "invite-modal",
      "context-menu",
    ]);

    const closedRest = bridge.closeAllLayers();
    assert.deepEqual(closedRest, ["context-menu", "invite-modal", "search-panel"]);
    assert.deepEqual(bridge.activeOverlayStack, []);
    assert.equal(bridge.closeTopLayer(), null);

    assert.deepEqual(log.slice(-4), [
      "close:expression-picker",
      "close:context-menu",
      "close:invite-modal",
      "close:search-panel",
    ]);
  });

  it("handleGlobalKeyDown обрабатывает Ctrl+K, Ctrl+F и Escape через normalizeKeyCombo", () => {
    const log = /** @type {string[]} */ ([]);
    const bridge = createShellFeatureBridge({});
    const qsCtrl = makeMockController("quick-switcher", log);
    const searchCtrl = makeMockController("search-panel", log);

    bridge.registerController("quick-switcher", qsCtrl);
    bridge.registerController("search-panel", searchCtrl);

    let prevented = 0;
    const makeKeyEvent = (/** @type {Record<string, unknown>} */ overrides) => ({
      key: "",
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      altKey: false,
      preventDefault() {
        prevented += 1;
      },
      ...overrides,
    });

    // Ctrl+K -> открывает quick-switcher
    assert.equal(
      bridge.handleGlobalKeyDown(makeKeyEvent({ key: "k", ctrlKey: true })),
      true,
    );
    assert.deepEqual(bridge.activeOverlayStack, ["quick-switcher"]);
    assert.equal(qsCtrl.openCount, 1);
    assert.equal(prevented, 1);

    // Ctrl+F (или Cmd+F) -> открывает search-panel
    assert.equal(
      bridge.handleGlobalKeyDown(makeKeyEvent({ key: "f", metaKey: true })),
      true,
    );
    assert.deepEqual(bridge.activeOverlayStack, ["quick-switcher", "search-panel"]);
    assert.equal(searchCtrl.openCount, 1);
    assert.equal(prevented, 2);

    // Escape -> закрывает верхний слой (search-panel)
    assert.equal(
      bridge.handleGlobalKeyDown(makeKeyEvent({ key: "Escape" })),
      true,
    );
    assert.deepEqual(bridge.activeOverlayStack, ["quick-switcher"]);
    assert.equal(searchCtrl.closeCount, 1);

    // Повторный Escape -> закрывает quick-switcher
    assert.equal(
      bridge.handleGlobalKeyDown(makeKeyEvent({ key: "Escape" })),
      true,
    );
    assert.deepEqual(bridge.activeOverlayStack, []);
    assert.equal(qsCtrl.closeCount, 1);

    // Неизвестные комбинации или модификаторы -> false
    assert.equal(
      bridge.handleGlobalKeyDown(makeKeyEvent({ key: "g", ctrlKey: true })),
      false,
    );
    assert.equal(
      bridge.handleGlobalKeyDown(makeKeyEvent({ key: "Control", ctrlKey: true })),
      false,
    );
    assert.equal(bridge.handleGlobalKeyDown(null), false);
  });

  it("проксирует вызовы onDispatchIpc и onNavigate", () => {
    /** @type {Array<{ command: string, payload: unknown }>} */
    const ipcCalls = [];
    /** @type {unknown[]} */
    const navCalls = [];

    const bridge = createShellFeatureBridge({
      onDispatchIpc(command, payload) {
        ipcCalls.push({ command, payload });
        return { ok: true };
      },
      onNavigate(target) {
        navCalls.push(target);
      },
    });

    const res = bridge.dispatchIpc("select_channel", { channelId: "100" });
    bridge.navigate({ guildId: "1", channelId: "100" });

    assert.deepEqual(res, { ok: true });
    assert.deepEqual(ipcCalls, [{ command: "select_channel", payload: { channelId: "100" } }]);
    assert.deepEqual(navCalls, [{ guildId: "1", channelId: "100" }]);
  });
});
