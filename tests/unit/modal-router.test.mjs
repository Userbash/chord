// @ts-check
/**
 * TAURI-BOOT-02: контракт модульного диспетчера модальных окон (`modal-router.js`).
 *
 * Проверяет:
 * - Замороженные списки `KNOWN_MODAL_IDS` и `DEFAULT_EXCLUSIVE_MODALS`;
 * - Регистрацию контроллеров модалок `registerModal(id, controller)` и валидацию аргументов;
 * - Открытие и закрытие модальных окон с передачей payload;
 * - Взаимное исключение эксклюзивных модальных окон;
 * - LIFO-порядок закрытия стека через `closeTop()` и `closeAll()`;
 * - Обработку клавиши Escape при `listenEscape: true`;
 * - Интеграцию с объектом приложения (`app.openModal`, `app.closeModal`, `app.closeTopModal`);
 * - Удаление контроллера через `unregisterModal` и очистку через `destroy()`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_EXCLUSIVE_MODALS,
  KNOWN_MODAL_IDS,
  initModalRouter,
} from "../../rust/gui-spike/ui/features/modal-router.js";

/**
 * Создаёт тестовый стаб контроллера модального окна.
 * @param {string} id
 * @param {string[]} [log]
 */
function makeMockModal(id, log = []) {
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

/**
 * Создаёт мок EventTarget для тестирования слушателя событий клавиатуры.
 */
function makeMockTarget() {
  /** @type {Record<string, ((event: any) => void)[]>} */
  const listeners = {};
  return {
    listeners,
    /** @param {string} type @param {(event: any) => void} fn */
    addEventListener(type, fn) {
      listeners[type] = listeners[type] || [];
      listeners[type].push(fn);
    },
    /** @param {string} type @param {(event: any) => void} fn */
    removeEventListener(type, fn) {
      if (!listeners[type]) return;
      listeners[type] = listeners[type].filter((cb) => cb !== fn);
    },
    /** @param {string} type @param {any} event */
    dispatch(type, event) {
      for (const fn of listeners[type] || []) {
        fn(event);
      }
    },
  };
}

describe("KNOWN_MODAL_IDS & DEFAULT_EXCLUSIVE_MODALS", () => {
  it("экспортирует замороженные списки известных и эксклюзивных модалок", () => {
    assert.equal(Object.isFrozen(KNOWN_MODAL_IDS), true);
    assert.equal(Object.isFrozen(DEFAULT_EXCLUSIVE_MODALS), true);
    assert.equal(KNOWN_MODAL_IDS.includes("quick-switcher"), true);
    assert.equal(KNOWN_MODAL_IDS.includes("settings-modal"), true);
    assert.equal(DEFAULT_EXCLUSIVE_MODALS.includes("quick-switcher"), true);
  });
});

describe("initModalRouter — регистрация и монтирование на app", () => {
  it("монтирует методы openModal, closeModal, closeTopModal и modalRouter на app", () => {
    const app = /** @type {Record<string, any>} */ ({});
    const router = initModalRouter(app);

    assert.equal(app.modalRouter, router);
    assert.equal(typeof app.openModal, "function");
    assert.equal(typeof app.closeModal, "function");
    assert.equal(typeof app.closeTopModal, "function");
  });

  it("регистрирует контроллеры и валидирует входные параметры", () => {
    const router = initModalRouter();
    const qs = makeMockModal("quick-switcher");

    router.registerModal("quick-switcher", qs);
    assert.equal(router.hasModal("quick-switcher"), true);
    assert.equal(router.hasModal("unknown"), false);

    assert.throws(() => {
      router.registerModal("", qs);
    }, TypeError);

    assert.throws(() => {
      // @ts-expect-error проверка в рантайме передачи null
      router.registerModal("settings", null);
    }, TypeError);
  });
});

describe("initModalRouter — открытие, закрытие и стек модалок", () => {
  it("open(modalId, payload) вызывает controller.open и кладёт модалку в стек", () => {
    const log = /** @type {string[]} */ ([]);
    const router = initModalRouter();
    const qs = makeMockModal("quick-switcher", log);

    router.registerModal("quick-switcher", qs);
    assert.equal(router.isOpen("quick-switcher"), false);

    router.open("quick-switcher", { query: "general" });
    assert.equal(router.isOpen("quick-switcher"), true);
    assert.equal(router.getActiveModal(), "quick-switcher");
    assert.deepEqual(router.getActiveStack(), ["quick-switcher"]);
    assert.equal(qs.openCount, 1);
    assert.deepEqual(qs.openPayloads, [{ query: "general" }]);
  });

  it("close(modalId) и closeTop() закрывают модалку и удаляют из стека", () => {
    const log = /** @type {string[]} */ ([]);
    const router = initModalRouter();
    const qs = makeMockModal("quick-switcher", log);
    const search = makeMockModal("search-panel", log);

    router.registerModal("quick-switcher", qs);
    router.registerModal("search-panel", search);

    router.open("quick-switcher");
    router.open("search-panel");
    assert.deepEqual(router.getActiveStack(), ["quick-switcher", "search-panel"]);

    const closed = router.closeTop();
    assert.equal(closed, "search-panel");
    assert.equal(search.closeCount, 1);
    assert.deepEqual(router.getActiveStack(), ["quick-switcher"]);

    router.close("quick-switcher");
    assert.equal(qs.closeCount, 1);
    assert.deepEqual(router.getActiveStack(), []);
  });

  it("closeAll() закрывает все открытые модалки в LIFO-порядке", () => {
    const log = /** @type {string[]} */ ([]);
    const router = initModalRouter();
    const m1 = makeMockModal("modal-1", log);
    const m2 = makeMockModal("modal-2", log);
    const m3 = makeMockModal("modal-3", log);

    router.registerModal("modal-1", m1);
    router.registerModal("modal-2", m2);
    router.registerModal("modal-3", m3);

    router.open("modal-1");
    router.open("modal-2");
    router.open("modal-3");

    const closed = router.closeAll();
    assert.deepEqual(closed, ["modal-3", "modal-2", "modal-1"]);
    assert.deepEqual(router.getActiveStack(), []);
    assert.equal(m1.closeCount, 1);
    assert.equal(m2.closeCount, 1);
    assert.equal(m3.closeCount, 1);
  });
});

describe("initModalRouter — взаимное исключение эксклюзивных модалок", () => {
  it("открытие эксклюзивной модалки автоматически закрывает открытую эксклюзивную модалку", () => {
    const log = /** @type {string[]} */ ([]);
    const router = initModalRouter();
    const qs = makeMockModal("quick-switcher", log);
    const settings = makeMockModal("settings-modal", log);

    router.registerModal("quick-switcher", qs);
    router.registerModal("settings-modal", settings);

    router.open("quick-switcher");
    assert.deepEqual(router.getActiveStack(), ["quick-switcher"]);

    // Открываем settings-modal — quick-switcher должен автоматически закрыться
    router.open("settings-modal");
    assert.deepEqual(router.getActiveStack(), ["settings-modal"]);
    assert.equal(qs.closeCount, 1);
    assert.equal(settings.openCount, 1);
    assert.deepEqual(log, ["open:quick-switcher", "close:quick-switcher", "open:settings-modal"]);
  });
});

describe("initModalRouter — Escape-слушатель и удаление", () => {
  it("закрывает верхнюю модалку при нажатии Escape (listenEscape: true)", () => {
    const target = makeMockTarget();
    const log = /** @type {string[]} */ ([]);
    const router = initModalRouter({}, { target, listenEscape: true });
    const qs = makeMockModal("quick-switcher", log);

    router.registerModal("quick-switcher", qs);
    router.open("quick-switcher");
    assert.equal(router.isOpen("quick-switcher"), true);

    let prevented = false;
    target.dispatch("keydown", {
      key: "Escape",
      preventDefault: () => { prevented = true; },
    });

    assert.equal(prevented, true);
    assert.equal(router.isOpen("quick-switcher"), false);
    assert.equal(qs.closeCount, 1);

    router.destroy();
    assert.equal(target.listeners.keydown?.length, 0);
  });

  it("unregisterModal закрывает модалку, если она открыта, и удаляет контроллер", () => {
    const router = initModalRouter();
    const qs = makeMockModal("quick-switcher");

    router.registerModal("quick-switcher", qs);
    router.open("quick-switcher");
    assert.equal(router.isOpen("quick-switcher"), true);

    const deleted = router.unregisterModal("quick-switcher");
    assert.equal(deleted, true);
    assert.equal(router.isOpen("quick-switcher"), false);
    assert.equal(router.hasModal("quick-switcher"), false);
    assert.equal(qs.closeCount, 1);
  });
});
