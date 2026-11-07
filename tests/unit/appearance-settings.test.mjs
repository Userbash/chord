// @ts-check
/**
 * T-185 (UI appearance controls): контракт контроллера внешнего вида
 * (тема, масштаб шрифта, отступы групп, уменьшенная анимация).
 *
 * Проверяет: точные наборы шагов, немедленное применение к
 * `doc.documentElement`, гидратацию из storage, отказ на недопустимых
 * значениях без мутации состояния и персистенцию JSON.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  APPEARANCE_STORAGE_KEY,
  FONT_SCALE_STEPS,
  GROUP_SPACING_STEPS,
  THEMES,
  createAppearanceController,
} from "../../rust/gui-spike/ui/features/appearance-settings.js";

/**
 * @param {string|null} [seed]
 * @returns {{ data: Map<string, string>, getItem(k: string): string|null, setItem(k: string, v: string): void, writes: number }}
 */
function makeStorage(seed = null) {
  /** @type {Map<string, string>} */
  const data = new Map();
  if (seed !== null) data.set(APPEARANCE_STORAGE_KEY, seed);
  return {
    data,
    writes: 0,
    /** @param {string} key */
    getItem(key) {
      return data.has(key) ? String(data.get(key)) : null;
    },
    /** @param {string} key @param {string} value */
    setItem(key, value) {
      this.writes += 1;
      data.set(key, String(value));
    },
  };
}

/** @param {ReturnType<typeof makeStorage>} storage */
function persisted(storage) {
  const raw = storage.getItem(APPEARANCE_STORAGE_KEY);
  assert.equal(typeof raw, "string");
  return JSON.parse(String(raw));
}

describe("appearance-settings: константы", () => {
  it("T-185-01: наборы шагов и ключ хранилища зафиксированы и заморожены", () => {
    assert.deepEqual([...FONT_SCALE_STEPS], [87.5, 93.75, 100, 106.25, 112.5, 125, 150]);
    assert.deepEqual([...GROUP_SPACING_STEPS], [0, 4, 8, 16, 24]);
    assert.deepEqual([...THEMES], ["dark", "light", "ash", "onyx"]);
    assert.equal(APPEARANCE_STORAGE_KEY, "discord.appearance");
    for (const list of [FONT_SCALE_STEPS, GROUP_SPACING_STEPS, THEMES]) {
      assert.equal(Object.isFrozen(list), true);
    }
  });
});

describe("appearance-settings: состояние по умолчанию", () => {
  it("T-185-02: без storage контроллер стартует с дефолтами", () => {
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc);
    assert.deepEqual(controller.getState(), {
      theme: "dark",
      fontScale: 100,
      groupSpacing: 16,
      reducedMotion: false,
    });
  });

  it("T-185-03: create применяет значения к documentElement сразу", () => {
    const doc = new ShimDocument();
    createAppearanceController(doc);
    const root = doc.documentElement;
    assert.equal(root.getAttribute("data-theme"), "dark");
    assert.equal(root.getAttribute("data-reduced-motion"), "false");
    assert.equal(root.style.getPropertyValue("--chat-font-scale"), "100%");
    assert.equal(root.style.getPropertyValue("--chat-group-spacing"), "16px");
  });
});

describe("appearance-settings: гидратация", () => {
  it("T-185-04: валидный JSON из storage восстанавливает все 4 контрола", () => {
    const storage = makeStorage(
      JSON.stringify({ theme: "onyx", fontScale: 125, groupSpacing: 24, reducedMotion: true }),
    );
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    assert.deepEqual(controller.getState(), {
      theme: "onyx",
      fontScale: 125,
      groupSpacing: 24,
      reducedMotion: true,
    });
    const root = doc.documentElement;
    assert.equal(root.getAttribute("data-theme"), "onyx");
    assert.equal(root.getAttribute("data-reduced-motion"), "true");
    assert.equal(root.style.getPropertyValue("--chat-font-scale"), "125%");
    assert.equal(root.style.getPropertyValue("--chat-group-spacing"), "24px");
  });

  it("T-185-05: невалидные поля в JSON отбрасываются по одному, остальные применяются", () => {
    const storage = makeStorage(
      JSON.stringify({ theme: "neon", fontScale: 101, groupSpacing: 4, reducedMotion: "yes" }),
    );
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    assert.deepEqual(controller.getState(), {
      theme: "dark",
      fontScale: 100,
      groupSpacing: 4,
      reducedMotion: false,
    });
  });

  it("T-185-06: битый JSON и мусор в storage не ломают контроллер", () => {
    const broken = makeStorage("{not json");
    assert.equal(createAppearanceController(new ShimDocument(), { storage: broken }).getState().theme, "dark");
    const notObject = makeStorage(JSON.stringify(["dark"]));
    assert.equal(createAppearanceController(new ShimDocument(), { storage: notObject }).getState().fontScale, 100);
    const nulled = makeStorage(JSON.stringify(null));
    assert.equal(createAppearanceController(new ShimDocument(), { storage: nulled }).getState().groupSpacing, 16);
  });

  it("T-185-07: падение storage.getItem не мешает старту с дефолтами", () => {
    const doc = new ShimDocument();
    const storage = {
      /** @param {string} _k @returns {string|null} */
      getItem(_k) {
        throw new Error("storage недоступен");
      },
      /** @param {string} _k @param {string} _v */
      setItem(_k, _v) {},
    };
    const controller = createAppearanceController(doc, { storage });
    assert.deepEqual(controller.getState(), {
      theme: "dark",
      fontScale: 100,
      groupSpacing: 16,
      reducedMotion: false,
    });
  });
});

describe("appearance-settings: сеттеры и валидация", () => {
  it("T-185-08: setTheme применяет тему и пишет JSON в storage", () => {
    const storage = makeStorage();
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    assert.equal(controller.setTheme("ash"), true);
    assert.equal(doc.documentElement.getAttribute("data-theme"), "ash");
    assert.equal(controller.getState().theme, "ash");
    assert.equal(persisted(storage).theme, "ash");
    assert.equal(storage.writes, 1);
  });

  it("T-185-09: setTheme отвергает чужое значение без мутации и записи в storage", () => {
    const storage = makeStorage();
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    for (const bad of ["neon", "", "DARK", null, undefined, 1]) {
      assert.equal(controller.setTheme(/** @type {any} */ (bad)), false, `значение ${String(bad)}`);
    }
    assert.equal(doc.documentElement.getAttribute("data-theme"), "dark");
    assert.equal(controller.getState().theme, "dark");
    assert.equal(storage.writes, 0);
  });

  it("T-185-10: setFontScale принимает только шаги и пишет проценты в CSS-переменную", () => {
    const storage = makeStorage();
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    for (const step of FONT_SCALE_STEPS) {
      assert.equal(controller.setFontScale(step), true, `шаг ${step}`);
      assert.equal(doc.documentElement.style.getPropertyValue("--chat-font-scale"), `${step}%`);
    }
    const before = controller.getState().fontScale;
    assert.equal(controller.setFontScale(100.5), false);
    assert.equal(controller.setFontScale(200), false);
    assert.equal(controller.setFontScale(/** @type {any} */ ("125")), false);
    assert.equal(controller.getState().fontScale, before);
    assert.equal(doc.documentElement.style.getPropertyValue("--chat-font-scale"), `${before}%`);
    assert.equal(persisted(storage).fontScale, before);
  });

  it("T-185-11: setGroupSpacing принимает 0 и остальные шаги, отвергая прочие", () => {
    const storage = makeStorage();
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    assert.equal(controller.setGroupSpacing(0), true);
    assert.equal(doc.documentElement.style.getPropertyValue("--chat-group-spacing"), "0px");
    assert.equal(controller.setGroupSpacing(24), true);
    assert.equal(doc.documentElement.style.getPropertyValue("--chat-group-spacing"), "24px");
    for (const bad of [1, 12, -4, 32, NaN]) {
      assert.equal(controller.setGroupSpacing(bad), false, `значение ${bad}`);
    }
    assert.equal(controller.getState().groupSpacing, 24);
    assert.equal(persisted(storage).groupSpacing, 24);
  });

  it("T-185-12: setReducedMotion принимает строго boolean", () => {
    const storage = makeStorage();
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    assert.equal(controller.setReducedMotion(true), true);
    assert.equal(doc.documentElement.getAttribute("data-reduced-motion"), "true");
    assert.equal(persisted(storage).reducedMotion, true);
    for (const bad of [1, 0, "true", null, undefined]) {
      assert.equal(controller.setReducedMotion(/** @type {any} */ (bad)), false, `значение ${String(bad)}`);
    }
    assert.equal(controller.getState().reducedMotion, true);
    assert.equal(doc.documentElement.getAttribute("data-reduced-motion"), "true");
  });
});

describe("appearance-settings: getState и resetDefaults", () => {
  it("T-185-13: getState отдаёт копию, внешняя мутация не меняет контроллер", () => {
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage: makeStorage() });
    const snapshot = controller.getState();
    snapshot.theme = "light";
    snapshot.fontScale = 150;
    assert.deepEqual(controller.getState(), {
      theme: "dark",
      fontScale: 100,
      groupSpacing: 16,
      reducedMotion: false,
    });
    assert.notEqual(controller.getState(), controller.getState());
  });

  it("T-185-14: resetDefaults возвращает дефолты, применяет их и перезаписывает storage", () => {
    const storage = makeStorage(
      JSON.stringify({ theme: "light", fontScale: 150, groupSpacing: 0, reducedMotion: true }),
    );
    const doc = new ShimDocument();
    const controller = createAppearanceController(doc, { storage });
    assert.equal(controller.getState().theme, "light");
    const writesBefore = storage.writes;
    controller.resetDefaults();
    assert.deepEqual(controller.getState(), {
      theme: "dark",
      fontScale: 100,
      groupSpacing: 16,
      reducedMotion: false,
    });
    const root = doc.documentElement;
    assert.equal(root.getAttribute("data-theme"), "dark");
    assert.equal(root.getAttribute("data-reduced-motion"), "false");
    assert.equal(root.style.getPropertyValue("--chat-font-scale"), "100%");
    assert.equal(root.style.getPropertyValue("--chat-group-spacing"), "16px");
    assert.deepEqual(persisted(storage), {
      theme: "dark",
      fontScale: 100,
      groupSpacing: 16,
      reducedMotion: false,
    });
    assert.equal(storage.writes, writesBefore + 1);
  });

  it("T-185-15: падение storage.setItem не отменяет применение к DOM", () => {
    const doc = new ShimDocument();
    const storage = {
      /** @param {string} _k @returns {string|null} */
      getItem(_k) {
        return null;
      },
      /** @param {string} _k @param {string} _v */
      setItem(_k, _v) {
        throw new Error("квота исчерпана");
      },
    };
    const controller = createAppearanceController(doc, { storage });
    assert.equal(controller.setTheme("light"), true);
    assert.equal(doc.documentElement.getAttribute("data-theme"), "light");
    assert.equal(controller.getState().theme, "light");
  });
});
