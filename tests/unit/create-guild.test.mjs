// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  createCreateGuildController,
  CREATE_GUILD_TEXT,
  VALID_TEMPLATES,
  validateGuildName,
} from "../../rust/gui-spike/ui/features/create-guild.js";

/**
 * Создаёт тестовое окружение с чистым ShimDocument.
 * @param {Record<string, any>} [customOpts]
 */
function createTestEnv(customOpts = {}) {
  const doc = new ShimDocument();
  const root = doc.createElement("div");
  root.id = "app-root";
  doc.body.append(root);

  /** @type {Array<{ cmd: string, args: any }>} */
  const calls = [];
  const invoker = async (/** @type {string} */ cmd, /** @type {any} */ args) => {
    calls.push({ cmd, args });
    if (customOpts.invoker) {
      return customOpts.invoker(cmd, args);
    }
    return {
      id: "777888999111222333",
      name: args?.name ?? "Новый сервер",
      channels: [],
    };
  };

  const controller = createCreateGuildController({
    document: doc,
    root,
    invoker,
    ...customOpts,
  });

  return { doc, root, calls, controller };
}

describe("STUB-03: Create Guild Modal & Validation", () => {
  it("validateGuildName: проверяет длину от 2 до 100 символов", () => {
    assert.equal(validateGuildName(""), false);
    assert.equal(validateGuildName("   "), false);
    assert.equal(validateGuildName("a"), false);
    assert.equal(validateGuildName("ab"), true);
    assert.equal(validateGuildName("Мой уютный сервер"), true);
    assert.equal(validateGuildName("a".repeat(100)), true);
    assert.equal(validateGuildName("a".repeat(101)), false);
  });

  it("createCreateGuildController: монтирует диалог в DOM и управляет открытием/закрытием", () => {
    const { doc, controller } = createTestEnv();

    const dialog = /** @type {any} */ (doc.getElementById("create-guild-dialog"));
    assert.ok(dialog, "диалог должен быть добавлен в DOM");
    assert.equal(dialog.getAttribute("role"), "dialog");
    assert.equal(dialog.getAttribute("aria-modal"), "true");
    assert.equal(dialog.hidden, true, "по умолчанию диалог скрыт");

    controller.open();
    assert.equal(dialog.hidden, false, "диалог открыт");
    assert.equal(dialog.classList.contains("is-open"), true);

    controller.close();
    assert.equal(dialog.hidden, true, "диалог закрыт");
    assert.equal(dialog.classList.contains("is-open"), false);
  });

  it("createCreateGuildController: переключает шаг выбора шаблона и форму названия сервера", () => {
    const { doc, controller } = createTestEnv();

    controller.open();
    const templateButtons = doc.querySelectorAll(".template-card");
    assert.ok(templateButtons.length >= 3, "как минимум 3 карточки шаблонов");

    // Клик по шаблону "Для меня и друзей"
    const friendsTemplateBtn = /** @type {any} */ (
      doc.querySelector('[data-template="friends"]')
    );
    assert.ok(friendsTemplateBtn, "кнопка шаблона друзей найдена");
    friendsTemplateBtn.click();

    // Должен отобразиться экран настройки сервера
    const nameInput = /** @type {any} */ (
      doc.getElementById("create-guild-name")
    );
    assert.ok(nameInput, "поле ввода названия отображается");
    assert.equal(controller.getSelectedTemplate(), "friends");

    // Кнопка назад возвращает к выбору шаблонов
    const backBtn = /** @type {any} */ (
      doc.querySelector(".create-guild-back")
    );
    assert.ok(backBtn, "кнопка назад есть");
    backBtn.click();
    assert.equal(controller.getStep(), "template");
  });

  it("createCreateGuildController: валидирует имя и блокирует кнопку создания", () => {
    const { doc, controller } = createTestEnv();

    controller.open();
    controller.selectTemplate("custom");

    const nameInput = /** @type {any} */ (
      doc.getElementById("create-guild-name")
    );
    const submitBtn = /** @type {any} */ (
      doc.getElementById("create-guild-submit")
    );

    assert.ok(nameInput);
    assert.ok(submitBtn);

    // Очищаем ввод
    nameInput.value = " ";
    nameInput.dispatchEvent("input");
    assert.equal(submitBtn.disabled, true, "кнопка заблокирована для пустого ввода");

    // Вводим валидное имя
    nameInput.value = "Новый сервер";
    nameInput.dispatchEvent("input");
    assert.equal(submitBtn.disabled, false, "кнопка разблокирована для валидного имени");
  });

  it("createCreateGuildController: отправляет IPC create_guild, блокирует повторный клик и вызывает коллбэк", async () => {
    let createdOutcome = null;

    const { doc, calls, controller } = createTestEnv({
      onCreated: (/** @type {any} */ guild) => {
        createdOutcome = guild;
      },
    });

    controller.open();
    controller.selectTemplate("friends");

    const nameInput = /** @type {any} */ (
      doc.getElementById("create-guild-name")
    );
    const submitBtn = /** @type {any} */ (
      doc.getElementById("create-guild-submit")
    );

    nameInput.value = "Сервер Мечты";
    nameInput.dispatchEvent("input");

    const submitPromise = controller.submit();
    assert.equal(submitBtn.disabled, true, "кнопка отключена во время отправки");
    assert.equal(submitBtn.getAttribute("aria-busy"), "true");

    await submitPromise;

    assert.equal(calls.length, 1);
    const call0 = calls[0];
    assert.ok(call0);
    assert.equal(call0.cmd, "create_guild");
    assert.equal(call0.args?.name, "Сервер Мечты");
    assert.equal(call0.args?.template, "friends");

    assert.ok(createdOutcome);
    assert.equal(/** @type {any} */ (createdOutcome).id, "777888999111222333");
  });

  it("createCreateGuildController: переключает на модалку инвайтов при клике на 'Уже есть приглашение'", () => {
    let joinOpened = false;

    const { doc, controller } = createTestEnv({
      onOpenJoin: () => {
        joinOpened = true;
      },
    });

    controller.open();
    const joinBtn = /** @type {any} */ (
      doc.querySelector(".switch-to-join-btn")
    );
    assert.ok(joinBtn, "ссылка/кнопка перехода к инвайтам найдена");

    joinBtn.click();
    assert.equal(joinOpened, true, "должен вызваться коллбэк onOpenJoin");
    const dialog = /** @type {any} */ (doc.getElementById("create-guild-dialog"));
    assert.equal(dialog?.hidden, true, "диалог создания закрывается при переходе");
  });
});
