// @ts-check
/**
 * TASK-612 (G2 Frontend): модульные тесты визарда выбора шаблонов создания сервера
 * (`rust/gui-spike/ui/features/create-guild.js`, `rust/gui-spike/ui/styles/create-guild.css`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` §5.5 (Modal `CreateGuildModal`: `role="dialog"`,
 *   `aria-modal="true"`, `aria-labelledby`, focus trap модуль `617986`, `--modal-width-small: 442px`);
 * - `/home/sanya/re-deep/decoded/a72f8c0298aeb52e_0` (модуль `819638`: шаги визарда шаблонов и настройки);
 * - `/home/sanya/re-deep/decoded/9164065a2404f2fe_0` и `/home/sanya/re-deep/ru-catalog.json`
 *   (шаблоны: `'Свой шаблон'`, `'Для меня и друзей'`, `'Учебная группа'`, `'Клуб или сообщество'`);
 * - `/home/sanya/re-deep/decoded/496925fc1facf986_0` (модуль `356343`: загрузка значка base64 DataURI).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  CREATE_GUILD_TEMPLATES,
  CREATE_GUILD_TEXT,
  MAX_GUILD_NAME_CHARS,
  MIN_GUILD_NAME_CHARS,
  VALID_TEMPLATES,
  createCreateGuildController,
  createCreateGuildWizard,
  normalizeGuildName,
  resolveGuildTemplate,
  validateGuildIconDataUri,
  validateGuildName,
} from "../../rust/gui-spike/ui/features/create-guild.js";

const CREATE_GUILD_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/create-guild.css", import.meta.url),
);
const TOKENS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
);

const SAMPLE_PNG_DATA_URI =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
const SAMPLE_WEBP_DATA_URI = "data:image/webp;base64,UklGRh4AAABXRUJQVlA4TBEAAAAvAAAAAAfQ//73v/+BiOh/AAA=";

/**
 * Создаёт изолированное тестовое окружение с ShimDocument.
 * @param {Record<string, any>} [customOpts]
 */
function createWizardTestEnv(customOpts = {}) {
  const doc = new ShimDocument();
  const root = doc.createElement("div");
  root.id = "app-root";
  doc.body.append(root);

  const opener = doc.createElement("button");
  opener.id = "guild-add-button";
  opener.type = "button";
  opener.setAttribute("aria-expanded", "false");
  root.append(opener);

  /** @type {Array<{ cmd: string, args: any }>} */
  const calls = [];
  const invoker = async (/** @type {string} */ cmd, /** @type {any} */ args) => {
    calls.push({ cmd, args });
    if (typeof customOpts.invoker === "function") {
      return customOpts.invoker(cmd, args);
    }
    return {
      id: "888999000111222333",
      name: args?.name ?? "Новый сервер",
      icon: args?.icon ?? null,
      channels: [],
    };
  };

  const wizard = createCreateGuildWizard({
    document: doc,
    root,
    invoker,
    ...customOpts,
  });

  return { doc, root, opener, calls, wizard };
}

describe("TASK-612: Шаблоны создания сервера (RE-контракт)", () => {
  it("содержит все 4 обязательных шаблона ('Свой шаблон', 'Для меня и друзей', 'Учебная группа', 'Клуб или сообщество')", () => {
    assert.equal(Array.isArray(CREATE_GUILD_TEMPLATES), true);
    assert.equal(Object.isFrozen(CREATE_GUILD_TEMPLATES), true);
    assert.equal(CREATE_GUILD_TEMPLATES.length, 4);

    const labels = CREATE_GUILD_TEMPLATES.map((tpl) => tpl.label);
    assert.deepEqual(labels, [
      "Свой шаблон",
      "Для меня и друзей",
      "Учебная группа",
      "Клуб или сообщество",
    ]);

    assert.equal(VALID_TEMPLATES.includes("custom"), true);
    assert.equal(VALID_TEMPLATES.includes("friends"), true);
    assert.equal(VALID_TEMPLATES.includes("gaming"), true);
    assert.equal(VALID_TEMPLATES.includes("study"), true);
    assert.equal(VALID_TEMPLATES.includes("community"), true);

    assert.equal(resolveGuildTemplate("custom")?.label, "Свой шаблон");
    assert.equal(resolveGuildTemplate("gaming")?.label, "Для меня и друзей");
    assert.equal(resolveGuildTemplate("friends")?.label, "Для меня и друзей");
    assert.equal(resolveGuildTemplate("study")?.label, "Учебная группа");
    assert.equal(resolveGuildTemplate("community")?.label, "Клуб или сообщество");
    assert.equal(resolveGuildTemplate("Учебная группа")?.id, "study");
    assert.equal(resolveGuildTemplate("unknown-template"), null);
  });

  it("рендерит 4 карточки шаблонов на шаге выбора и переходит к настройке выбранного шаблона", () => {
    const { doc, opener, wizard } = createWizardTestEnv({ currentUsername: "Саня" });
    wizard.open(opener);

    assert.equal(wizard.getStep(), "template");
    const cards = doc.querySelectorAll(".template-card");
    assert.equal(cards.length, 4, "должны рендериться 4 карточки шаблонов");

    const renderedLabels = cards.map((card) => {
      const labelEl = card.querySelector(".template-label");
      return labelEl ? labelEl.textContent : "";
    });
    assert.deepEqual(renderedLabels, [
      "Свой шаблон",
      "Для меня и друзей",
      "Учебная группа",
      "Клуб или сообщество",
    ]);

    // Выбираем шаблон 'Учебная группа' (Study)
    const studyCard = /** @type {any} */ (doc.querySelector('[data-template="study"]'));
    assert.ok(studyCard, "карточка шаблона 'Учебная группа' должна существовать");
    studyCard.click();

    assert.equal(wizard.getStep(), "customize");
    assert.equal(wizard.getSelectedTemplate(), "study");

    const title = doc.getElementById("create-guild-title");
    assert.equal(title?.textContent, CREATE_GUILD_TEXT.customizeTitle);
  });
});

describe("TASK-612: Валидация названия сервера (1..=100 символов, обрезка пробелов)", () => {
  it("проверяет границы 1..=100 символов после обрезки пробелов и отклоняет невалидные значения", () => {
    assert.equal(MIN_GUILD_NAME_CHARS, 1);
    assert.equal(MAX_GUILD_NAME_CHARS, 100);

    // Негативные и пустые значения
    assert.equal(validateGuildName(""), false);
    assert.equal(validateGuildName("   \t\n  "), false);
    assert.equal(validateGuildName(null), false);
    assert.equal(validateGuildName(undefined), false);
    assert.equal(validateGuildName(42), false);

    // Граничные: ровно 1 символ и 1 символ с пробелами по краям
    assert.equal(validateGuildName("Я"), true);
    assert.equal(validateGuildName("a"), true);
    assert.equal(validateGuildName("   A   "), true);
    assert.equal(normalizeGuildName("   Сервер друзей   "), "Сервер друзей");

    // Граничные: 100 и 101 символ
    assert.equal(validateGuildName("Б".repeat(100)), true);
    assert.equal(validateGuildName(`  ${"Б".repeat(100)}  `), true);
    assert.equal(validateGuildName("Б".repeat(101)), false);
  });

  it("обрезает пробелы перед отправкой в create_guild и принимает имя из 1 символа в форме", async () => {
    const { doc, calls, wizard } = createWizardTestEnv();
    wizard.open();
    wizard.selectTemplate("gaming");

    const nameInput = /** @type {any} */ (doc.getElementById("create-guild-name"));
    const submitBtn = /** @type {any} */ (doc.getElementById("create-guild-submit"));
    assert.ok(nameInput);
    assert.ok(submitBtn);

    // Пробелы -> кнопка неактивна
    nameInput.value = "    ";
    nameInput.dispatchEvent("input");
    assert.equal(submitBtn.disabled, true);
    assert.equal(nameInput.getAttribute("aria-invalid"), "true");

    // 101 символ -> кнопка неактивна
    nameInput.value = "X".repeat(101);
    nameInput.dispatchEvent("input");
    assert.equal(submitBtn.disabled, true);
    assert.equal(nameInput.getAttribute("aria-invalid"), "true");

    // 1 символ с пробелами по краям -> кнопка активна, при отправке пробелы обрезаются
    nameInput.value = "   Z   ";
    nameInput.dispatchEvent("input");
    assert.equal(submitBtn.disabled, false);
    assert.equal(nameInput.getAttribute("aria-invalid"), "false");

    await wizard.submit();
    assert.equal(calls.length, 1);
    assert.equal(calls[0]?.cmd, "create_guild");
    assert.equal(calls[0]?.args?.name, "Z");
    assert.equal(calls[0]?.args?.template, "friends");
  });
});

describe("TASK-612: Загрузка иконки сервера (base64 DataURI)", () => {
  it("валидирует формат base64 DataURI для изображений (png, jpeg, jpg, gif, webp)", () => {
    assert.equal(validateGuildIconDataUri(null), true);
    assert.equal(validateGuildIconDataUri(undefined), true);
    assert.equal(validateGuildIconDataUri(""), true);

    assert.equal(validateGuildIconDataUri(SAMPLE_PNG_DATA_URI), true);
    assert.equal(validateGuildIconDataUri(SAMPLE_WEBP_DATA_URI), true);
    assert.equal(validateGuildIconDataUri("data:image/jpeg;base64,/9j/4AAQSkZJRg=="), true);
    assert.equal(validateGuildIconDataUri("data:image/gif;base64,R0lGODlhAQABAAAAACw="), true);

    // Негативные сценарии
    assert.equal(validateGuildIconDataUri("data:text/plain;base64,SGVsbG8="), false);
    assert.equal(validateGuildIconDataUri("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="), false);
    assert.equal(validateGuildIconDataUri("data:image/png;base64,"), false);
    assert.equal(validateGuildIconDataUri("data:image/png;base64,!!!not-base64!!!"), false);
    assert.equal(validateGuildIconDataUri("https://cdn.discordapp.com/icons/1/a.png"), false);
    assert.equal(validateGuildIconDataUri(12345), false);
  });

  it("обновляет превью иконки при загрузке base64 DataURI, поддерживает сброс и передаёт icon в IPC", async () => {
    const { doc, calls, wizard } = createWizardTestEnv();
    wizard.open();
    wizard.selectTemplate("community");

    const iconInput = /** @type {any} */ (doc.getElementById("create-guild-icon-input"));
    const iconPreview = /** @type {any} */ (doc.getElementById("create-guild-icon-preview"));
    const clearBtn = /** @type {any} */ (doc.getElementById("create-guild-icon-clear"));
    const errorBox = /** @type {any} */ (doc.getElementById("create-guild-error"));

    assert.ok(iconInput, "поле загрузки иконки должно присутствовать");
    assert.ok(iconPreview, "контейнер превью иконки должен присутствовать");
    assert.ok(clearBtn, "кнопка очистки иконки должна присутствовать");
    assert.equal(wizard.getIcon(), null);
    assert.equal(clearBtn.hidden, true);

    // Попытка загрузить невалидный DataURI показывает ошибку
    const badResult = wizard.setIcon("data:text/html;base64,PGh0bWw+");
    assert.equal(badResult, false);
    assert.equal(wizard.getIcon(), null);
    assert.equal(errorBox.hidden, false);

    // Загрузка валидного PNG base64 DataURI через событие change на input
    iconInput.value = SAMPLE_PNG_DATA_URI;
    iconInput.dispatchEvent("change");

    assert.equal(wizard.getIcon(), SAMPLE_PNG_DATA_URI);
    assert.equal(clearBtn.hidden, false);
    const previewImg = iconPreview.querySelector("img.create-guild-icon-img");
    assert.ok(previewImg, "превью должно содержать элемент <img>");
    assert.equal(previewImg?.getAttribute("src"), SAMPLE_PNG_DATA_URI);

    // Очистка иконки кнопкой сброса
    clearBtn.click();
    assert.equal(wizard.getIcon(), null);
    assert.equal(clearBtn.hidden, true);
    assert.equal(iconPreview.querySelector("img.create-guild-icon-img"), null);

    // Повторная установка через setIcon и отправка формы
    assert.equal(wizard.setIcon(SAMPLE_WEBP_DATA_URI), true);
    const nameInput = /** @type {any} */ (doc.getElementById("create-guild-name"));
    nameInput.value = "Клуб инженеров";
    nameInput.dispatchEvent("input");

    await wizard.submit();
    assert.equal(calls.length, 1);
    assert.equal(calls[0]?.args?.name, "Клуб инженеров");
    assert.equal(calls[0]?.args?.icon, SAMPLE_WEBP_DATA_URI);
    assert.equal(calls[0]?.args?.template, "community");
  });
});

describe("TASK-612: Доступность (A11y): стрелки, Escape и фокус-ловушка", () => {
  it("поддерживает навигацию стрелками (ArrowDown/ArrowUp/Home/End) по шаблонам с roving tabindex", () => {
    const { doc, opener, wizard } = createWizardTestEnv();
    wizard.open(opener);

    const cards = doc.querySelectorAll(".template-card");
    assert.equal(cards.length, 4);

    // По умолчанию первая карточка активна в roving tabindex
    assert.equal(cards[0]?.getAttribute("tabindex"), "0");
    assert.equal(cards[1]?.getAttribute("tabindex"), "-1");
    assert.equal(cards[2]?.getAttribute("tabindex"), "-1");
    assert.equal(cards[3]?.getAttribute("tabindex"), "-1");

    // ArrowDown -> переход ко 2-й карточке ('Для меня и друзей')
    cards[0]?.focus();
    const downEvt = cards[0]?.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(downEvt?.prevented, true);
    assert.equal(doc.activeElement, cards[1]);
    assert.equal(cards[1]?.getAttribute("tabindex"), "0");
    assert.equal(cards[0]?.getAttribute("tabindex"), "-1");

    // End -> переход к последней (4-й) карточке ('Клуб или сообщество')
    const endEvt = cards[1]?.dispatchEvent("keydown", { key: "End" });
    assert.equal(endEvt?.prevented, true);
    assert.equal(doc.activeElement, cards[3]);
    assert.equal(cards[3]?.getAttribute("tabindex"), "0");

    // ArrowDown с последней -> циклический переход к 1-й карточке ('Свой шаблон')
    const wrapDownEvt = cards[3]?.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(wrapDownEvt?.prevented, true);
    assert.equal(doc.activeElement, cards[0]);

    // ArrowUp с 1-й -> циклический переход к последней (4-й) карточке
    const wrapUpEvt = cards[0]?.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(wrapUpEvt?.prevented, true);
    assert.equal(doc.activeElement, cards[3]);

    // Home -> возврат к 1-й карточке
    const homeEvt = cards[3]?.dispatchEvent("keydown", { key: "Home" });
    assert.equal(homeEvt?.prevented, true);
    assert.equal(doc.activeElement, cards[0]);
  });

  it("закрывает диалог по клавише Escape и возвращает фокус и aria-expanded='false' на триггер", () => {
    const { doc, opener, wizard } = createWizardTestEnv();
    wizard.open(opener);

    const dialog = /** @type {any} */ (doc.getElementById("create-guild-dialog"));
    assert.equal(dialog.hidden, false);
    assert.equal(opener.getAttribute("aria-expanded"), "true");

    const escEvt = dialog.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(escEvt.prevented, true);
    assert.equal(dialog.hidden, true);
    assert.equal(dialog.classList.contains("is-open"), false);
    assert.equal(opener.getAttribute("aria-expanded"), "false");
    assert.equal(doc.activeElement, opener, "фокус должен вернуться на кнопку-триггер");
  });

  it("удерживает фокус внутри модального окна (Focus Trap на Tab и Shift+Tab)", () => {
    const { doc, opener, wizard } = createWizardTestEnv();
    wizard.open(opener);
    wizard.selectTemplate("custom");

    const dialog = /** @type {any} */ (doc.getElementById("create-guild-dialog"));
    const closeBtn = /** @type {any} */ (dialog.querySelector(".modal-close"));
    const submitBtn = /** @type {any} */ (doc.getElementById("create-guild-submit"));

    assert.ok(closeBtn, "первый фокусируемый элемент (.modal-close)");
    assert.ok(submitBtn, "последний фокусируемый элемент (#create-guild-submit)");

    // Tab на последнем элементе -> переносит фокус на первый элемент
    submitBtn.focus();
    assert.equal(doc.activeElement, submitBtn);
    const tabEvt = submitBtn.dispatchEvent("keydown", { key: "Tab", shiftKey: false });
    assert.equal(tabEvt.prevented, true);
    assert.equal(doc.activeElement, closeBtn);

    // Shift+Tab на первом элементе -> переносит фокус на последний элемент
    closeBtn.focus();
    const shiftTabEvt = closeBtn.dispatchEvent("keydown", { key: "Tab", shiftKey: true });
    assert.equal(shiftTabEvt.prevented, true);
    assert.equal(doc.activeElement, submitBtn);
  });
});

describe("TASK-612: CSS-стили визарда (tokens.css контракт)", () => {
  it("create-guild.css существует, использует токены из tokens.css и не содержит сырых цветов", () => {
    assert.equal(existsSync(CREATE_GUILD_CSS_PATH), true, "файл create-guild.css должен существовать");
    const css = readFileSync(CREATE_GUILD_CSS_PATH, "utf8");
    const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");

    for (const selector of [
      "#create-guild-dialog",
      ".create-guild-box",
      ".template-list",
      ".template-card",
      ".create-guild-icon-uploader",
      ".create-guild-icon-preview",
      ".create-guild-input",
      ".create-guild-footer",
    ]) {
      assert.equal(css.includes(selector), true, `ожидался селектор ${selector} в create-guild.css`);
    }

    // Отсутствие сырых цветов (#hex, rgb, hsl) и !important
    assert.equal(/#[0-9a-fA-F]{3,8}\b/.test(css), false, "в create-guild.css запрещены сырые hex-цвета");
    assert.equal(/\b(?:rgb|rgba|hsl|hsla)\s*\(/.test(css), false, "в create-guild.css запрещены rgb/hsl-цвета");
    assert.equal(css.includes("!important"), false, "в create-guild.css запрещён !important");

    // Все используемые CSS-переменные объявлены в tokens.css
    const usedVars = [...css.matchAll(/var\(\s*(--[a-z0-9-]+)\s*\)/gi)].map((m) => m[1] ?? "");
    assert.ok(usedVars.length >= 8, "должны использоваться дизайн-токены из tokens.css");
    for (const tokenName of usedVars) {
      assert.equal(
        tokensCss.includes(`${tokenName}:`),
        true,
        `токен ${tokenName} должен быть определён в rust/gui-spike/ui/tokens.css`,
      );
    }
  });
});
