// @ts-check
/**
 * M2-05 (fixture browser): плотность/тема → сохранение → **reload** → восстановление.
 *
 * Почему именно browser: unit-тесты проверяют гидратацию и whitelist сохранения,
 * но не переживание перезагрузки страницы. Здесь настоящий `Page.reload`:
 * fake `settings_save` пишет payload в `localStorage`, fake `settings_load`
 * после reload читает его, продукт применяет `html[data-density]`/`html[data-theme]`.
 *
 * Оракул: форма `UiSettings` — `{version, toggles, density, theme, locale}`,
 * значения из белых списков (`cozy|compact`, `dark|light`), дефолт `cozy/dark`.
 * В сохранённом payload не должно быть чужих ключей и секретов.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { m2FixtureData, openApp } from "../helpers/ui-fixture.mjs";

const data = m2FixtureData();
const DENSITY = String(data.settings.compact);
const THEME = String(data.settings.light);

/**
 * Открывает экран настроек и вкладку «Внешний вид».
 * @param {import("../helpers/ui-fixture.mjs").UiFixture} ui
 */
async function openAppearance(ui) {
  await ui.click('.rail-btn[data-screen="settings"]');
  await ui.waitFor(
    "(() => { const screen = document.querySelector('#screen-settings');"
      + " return screen !== null && screen.classList.contains('active'); })()",
    { label: "экран настроек" },
  );
  await ui.click('.settings-nav-item[data-section="Внешний вид"]');
  await ui.waitFor(
    "(() => { const pane = document.querySelector('#settings .settings-pane[data-pane=\"appearance\"]');"
      + " return pane !== null && !pane.hasAttribute('hidden'); })()",
    { label: "вкладка «Внешний вид»" },
  );
}

describe("M2-05: settings density/theme → reload persistence (реальный index.html)", () => {
  it("compact/light сохраняются в UiSettings и восстанавливаются после reload", async (t) => {
    const ui = await openApp(t);
    await openAppearance(ui);

    // 1. Исходное состояние — дефолты из settings_load (cozy/dark).
    assert.equal(await ui.eval("document.documentElement.getAttribute('data-density')"), "cozy");
    assert.equal(await ui.eval("document.documentElement.getAttribute('data-theme')"), "dark");

    // 2. Клик по «Компактное» и «Светлая» меняет атрибуты корня и aria-pressed.
    const densityBefore = await ui.count(".settings-density-option");
    assert.equal(densityBefore, 2, "ожидались две опции плотности");
    await ui.clickNth(".settings-density-option", 0);
    await ui.clickNth(".settings-theme-option", 1);
    assert.equal(
      await ui.eval("document.documentElement.getAttribute('data-density')"),
      DENSITY,
      "data-density не применился",
    );
    assert.equal(
      await ui.eval("document.documentElement.getAttribute('data-theme')"),
      THEME,
      "data-theme не применился",
    );
    assert.deepEqual(
      await ui.eval(
        "[...document.querySelectorAll('.settings-density-option')].map((node) => node.getAttribute('aria-pressed'))",
      ),
      ["true", "false"],
    );
    assert.deepEqual(
      await ui.eval(
        "[...document.querySelectorAll('.settings-theme-option')].map((node) => node.getAttribute('aria-pressed'))",
      ),
      ["false", "true"],
    );

    // 3. Сохранение: ровно payload UiSettings, без чужих ключей и секретов.
    // Boot also saves the selected guild. Wait for the debounced appearance save.
    await ui.waitForCall("settings_save", {
      where: (args) => args?.settings?.density === DENSITY && args?.settings?.theme === THEME,
    });
    const saves = await ui.calls("settings_save");
    const saved = saves[saves.length - 1]?.args?.settings;
    assert.ok(saved && typeof saved === "object", "settings_save без settings");
    assert.deepEqual(
      Object.keys(saved).sort(),
      [...data.settings.allowedKeys].sort(),
      "в payload попали чужие ключи",
    );
    assert.equal(saved.density, DENSITY);
    assert.equal(saved.theme, THEME);
    assert.doesNotMatch(JSON.stringify(saved), /token|password|DO-NOT-LEAK/i, "в настройки попал секрет");

    // 4. Reload: настройки читаются из fake-store, атрибуты восстанавливаются.
    await ui.reload();
    await ui.waitFor("window.__QA__.count('settings_load') >= 1", { label: "settings_load после reload" });
    await ui.waitFor(
      "document.documentElement.getAttribute('data-density') === " + JSON.stringify(DENSITY),
      { label: "плотность восстановлена после reload" },
    );
    assert.equal(
      await ui.eval("document.documentElement.getAttribute('data-theme')"),
      THEME,
      "тема не восстановлена после reload",
    );

    // 5. UI после reload отражает сохранённое (aria-pressed), а не дефолт.
    await openAppearance(ui);
    assert.deepEqual(
      await ui.eval(
        "[...document.querySelectorAll('.settings-density-option')].map((node) => node.getAttribute('aria-pressed'))",
      ),
      ["true", "false"],
    );
    assert.deepEqual(
      await ui.eval(
        "[...document.querySelectorAll('.settings-theme-option')].map((node) => node.getAttribute('aria-pressed'))",
      ),
      ["false", "true"],
    );
    assert.equal(await ui.callCount("settings_load"), 1, "после reload settings_load не вызван");
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });
});
