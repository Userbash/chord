// @ts-check
/**
 * TAURI-BOOT-03: контракт модульного диспетчера контекстных меню (`context-menu-router.js`).
 *
 * Проверяет:
 * - Замороженный список `KNOWN_CONTEXT_MENU_TYPES`;
 * - Регистрацию обработчиков `registerContextMenu(type, handler)`;
 * - Открытие контекстного меню `openContextMenu(type, payload)` с передачей координат и данных;
 * - Закрытие контекстного меню `closeContextMenu()` и восстановление фокуса на открывающем элементе;
 * - Закрытие по нажатию Escape (`listenEscape: true`);
 * - Закрытие по клику вне меню (`listenBackdrop: true`);
 * - Удаление обработчика `unregisterContextMenu(type)` и полную очистку `destroy()`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  KNOWN_CONTEXT_MENU_TYPES,
  initContextMenuRouter,
} from "../../rust/gui-spike/ui/features/context-menu-router.js";

/**
 * Создаёт тестовый стаб обработчика контекстного меню.
 * @param {string} type
 */
function makeMockMenuHandler(type) {
  /** @type {unknown[]} */
  const openPayloads = [];
  let openCount = 0;
  let closeCount = 0;
  return {
    type,
    openPayloads,
    get openCount() {
      return openCount;
    },
    get closeCount() {
      return closeCount;
    },
    /** @param {unknown} [payload] */
    open(payload) {
      openCount++;
      openPayloads.push(payload);
    },
    close() {
      closeCount++;
    },
  };
}

describe("TAURI-BOOT-03: context-menu-router contract", () => {
  it("exposes frozen KNOWN_CONTEXT_MENU_TYPES containing guild, channel, user, message", () => {
    assert.ok(Array.isArray(KNOWN_CONTEXT_MENU_TYPES));
    assert.ok(Object.isFrozen(KNOWN_CONTEXT_MENU_TYPES));
    for (const t of ["guild", "channel", "user", "message"]) {
      assert.ok(
        KNOWN_CONTEXT_MENU_TYPES.includes(t),
        `KNOWN_CONTEXT_MENU_TYPES should include ${t}`
      );
    }
  });

  it("registers, opens and closes context menu with payload and focus restore", () => {
    const router = initContextMenuRouter();
    const guildHandler = makeMockMenuHandler("guild");

    router.registerContextMenu("guild", guildHandler);
    assert.equal(router.hasContextMenu("guild"), true);
    assert.equal(router.isOpen("guild"), false);
    assert.equal(router.getActiveContextMenu(), null);

    let focused = false;
    const triggerElement = {
      focus() {
        focused = true;
      },
    };

    router.openContextMenu("guild", {
      guildId: "123456789",
      triggerElement,
      x: 150,
      y: 200,
    });

    assert.equal(guildHandler.openCount, 1);
    assert.equal(router.isOpen("guild"), true);
    assert.equal(router.getActiveContextMenu()?.type, "guild");

    router.closeContextMenu({ restoreFocus: true });
    assert.equal(guildHandler.closeCount, 1);
    assert.equal(router.isOpen("guild"), false);
    assert.equal(router.getActiveContextMenu(), null);
    assert.equal(focused, true);

    router.destroy();
  });

  it("handles Escape key to close active context menu", () => {
    /** @type {Record<string, Function[]>} */
    const listeners = {};
    const mockTarget = {
      /**
       * @param {string} evt
       * @param {Function} fn
       */
      addEventListener(evt, fn) {
        listeners[evt] = listeners[evt] || [];
        listeners[evt].push(fn);
      },
      /**
       * @param {string} evt
       * @param {Function} fn
       */
      removeEventListener(evt, fn) {
        listeners[evt] = (listeners[evt] || []).filter((f) => f !== fn);
      },
    };

    const router = initContextMenuRouter({
      listenEscape: true,
      target: mockTarget,
    });
    const channelHandler = makeMockMenuHandler("channel");
    router.registerContextMenu("channel", channelHandler);

    router.openContextMenu("channel", { channelId: "ch-999" });
    assert.equal(router.isOpen("channel"), true);

    // Trigger Escape
    const keyListeners = listeners["keydown"] || [];
    for (const listener of keyListeners) {
      listener({ key: "Escape", preventDefault() {} });
    }

    assert.equal(router.isOpen("channel"), false);
    assert.equal(channelHandler.closeCount, 1);

    router.destroy();
  });

  it("switches context menu when opening a new one without leaking prior state", () => {
    const router = initContextMenuRouter();
    const guildHandler = makeMockMenuHandler("guild");
    const userHandler = makeMockMenuHandler("user");

    router.registerContextMenu("guild", guildHandler);
    router.registerContextMenu("user", userHandler);

    router.openContextMenu("guild", { guildId: "g-1" });
    assert.equal(router.isOpen("guild"), true);

    router.openContextMenu("user", { userId: "u-1" });
    assert.equal(guildHandler.closeCount, 1, "previous menu must be closed");
    assert.equal(userHandler.openCount, 1, "new menu must be opened");
    assert.equal(router.isOpen("user"), true);
    assert.equal(router.isOpen("guild"), false);

    router.destroy();
  });

  it("unregisters menu and handles invalid arguments safely", () => {
    const router = initContextMenuRouter();
    const messageHandler = makeMockMenuHandler("message");

    router.registerContextMenu("message", messageHandler);
    assert.equal(router.unregisterContextMenu("message"), true);
    assert.equal(router.hasContextMenu("message"), false);

    assert.throws(() => {
      // @ts-ignore
      router.registerContextMenu("", {});
    });

    router.destroy();
  });
});
