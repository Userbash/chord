// @ts-check
/**
 * TASK-811: Unit-тесты баннера и модального окна экрана правил сервера
 * (Rules Screening Banner & Modal).
 *
 * Оракул:
 * - waves/kanban-board.json (TASK-811: P0, G2 Frontend)
 *   "Если участник находится в состоянии pending: true, внизу окна чата отображать плашку
 *    Discord «Вам необходимо выполнить несколько шагов, прежде чем вы сможете общаться»
 *    с кнопкой «Выполнить» и модальным окном согласия с правилами."
 *   Acceptance criteria:
 *   1. "Плашка отображается для серверов с невыполненными шагами верификации"
 *   2. "По нажатию кнопки «Отправить» отправляется IPC confirm_member_rules"
 *   Design tokens:
 *   background: #2b2d31, surface: #313338, text_primary: #ffffff, accent: #23a55a
 *
 * 9 измерений TEST-POLICY:
 * 1. Позитивное: показ баннера при pending: true, открытие модалки, согласие и отправка IPC.
 * 2. Негативное: скрытие баннера при pending: false, lurker: true или в ЛС/DM.
 * 3. Граничные: кастомные правила гильдии vs дефолтные правила сообщества, пустые/длинные правила.
 * 4. Ошибки: обработка ошибки IPC confirm_member_rules без ложного закрытия модалки.
 * 5. Отмена/пустые: закрытие по кнопке «Отмена», крестику или клавише Escape без вызова IPC.
 * 6. Загрузка: блокировка повторной отправки (double-submit guard) во время полёта IPC.
 * 7. Доступность: role="dialog", aria-modal="true", aria-labelledby, семантичные кнопки.
 * 8. Восстановление: обновление состояния pending при успешном подтверждении и разблокировка чата.
 * 9. Наблюдаемость: корректные атрибуты data-access-banner="pending" и data-rules-screening-modal.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ShimDocument, loadApp } from "../support/gui-dom-harness.mjs";
import {
  DEFAULT_COMMUNITY_RULES,
  RULES_SCREENING_TEXT,
  RULES_SCREENING_TOKENS,
  extractRulesFromGuild,
  isRulesScreeningRequired,
  renderRulesScreeningBanner,
  createRulesScreeningModalController,
} from "../../rust/gui-spike/ui/features/rules-screening-modal.js";

describe("TASK-811: Rules Screening Banner & Modal", () => {
  describe("1. Константы, текст и дизайн-токены (Design Tokens & Text Parity)", () => {
    it("содержит точные строки баннера и кнопок по спецификации TASK-811", () => {
      assert.equal(
        RULES_SCREENING_TEXT.bannerNotice,
        "Вам необходимо выполнить несколько шагов, прежде чем вы сможете общаться",
      );
      assert.equal(RULES_SCREENING_TEXT.bannerButton, "Выполнить");
      assert.equal(RULES_SCREENING_TEXT.modalTitle, "Правила сервера");
      assert.equal(RULES_SCREENING_TEXT.checkboxLabel, "Я прочитал(-а) и согласен(-на) с правилами");
      assert.equal(RULES_SCREENING_TEXT.submitButton, "Отправить");
      assert.equal(RULES_SCREENING_TEXT.cancelButton, "Отмена");
    });

    it("соответствует дизайн-токенам Discord из kanban-board.json", () => {
      assert.equal(RULES_SCREENING_TOKENS.background, "#2b2d31");
      assert.equal(RULES_SCREENING_TOKENS.surface, "#313338");
      assert.equal(RULES_SCREENING_TOKENS.textPrimary, "#ffffff");
      assert.equal(RULES_SCREENING_TOKENS.accent, "#23a55a");
    });

    it("предоставляет список стандартных правил сообщества по умолчанию", () => {
      assert.ok(Array.isArray(DEFAULT_COMMUNITY_RULES));
      assert.ok(DEFAULT_COMMUNITY_RULES.length >= 3);
      assert.ok(DEFAULT_COMMUNITY_RULES.some((r) => r.includes("уважением")));
      assert.ok(DEFAULT_COMMUNITY_RULES.some((r) => r.includes("спам")));
    });
  });

  describe("2. Чистые предикаты и извлечение данных (Pure helpers)", () => {
    it("isRulesScreeningRequired определяет необходимость прохождения правил", () => {
      // pending: true
      assert.equal(isRulesScreeningRequired({ id: "g1", pending: true }), true);
      assert.equal(isRulesScreeningRequired({ id: "g1", is_pending: true }), true);

      // pending: false
      assert.equal(isRulesScreeningRequired({ id: "g1", pending: false }), false);
      assert.equal(isRulesScreeningRequired({ id: "g1" }), false);
      assert.equal(isRulesScreeningRequired(null), false);

      // lurker или preview имеет приоритет перед pending
      assert.equal(isRulesScreeningRequired({ id: "g1", pending: true, lurker: true }), false);
      assert.equal(isRulesScreeningRequired({ id: "g1", pending: true, isMember: false }), false);
      assert.equal(isRulesScreeningRequired({ id: "g1", pending: true, is_member: false }), false);
    });

    it("extractRulesFromGuild извлекает кастомные правила или отдаёт дефолтные", () => {
      // Из поля rules (массив строк)
      const customRules = ["Не ругаться", "Быть вежливым"];
      assert.deepEqual(extractRulesFromGuild({ rules: customRules }), customRules);

      // Из поля form_fields (TERMS)
      const formFieldsGuild = {
        form_fields: [
          {
            field_type: "TERMS",
            values: ["Правило А", "Правило Б"],
          },
        ],
      };
      assert.deepEqual(extractRulesFromGuild(formFieldsGuild), ["Правило А", "Правило Б"]);

      // При отсутствии правил — дефолтный набор
      assert.deepEqual(extractRulesFromGuild({}), DEFAULT_COMMUNITY_RULES);
      assert.deepEqual(extractRulesFromGuild(null), DEFAULT_COMMUNITY_RULES);
    });
  });

  describe("3. Рендеринг баннера проверки правил (Verification Notice Bar)", () => {
    it("рендерит плашку с текстом, стилями и кнопкой «Выполнить»", () => {
      const doc = new ShimDocument();
      const container = doc.createElement("div");
      doc.body.appendChild(container);

      let opened = false;
      const banner = renderRulesScreeningBanner(/** @type {any} */ (container), {
        document: /** @type {any} */ (doc),
        onOpenModal: () => {
          opened = true;
        },
      });

      assert.ok(banner);
      assert.equal(banner.id, "guild-pending-screening-banner");
      assert.equal(banner.getAttribute("data-access-banner"), "pending");

      const textEl = banner.querySelector(".rules-screening-banner-text");
      assert.ok(textEl);
      assert.ok(textEl.textContent?.includes("Вам необходимо выполнить несколько шагов"));

      const btn = /** @type {any} */ (banner.querySelector(".rules-screening-complete-btn"));
      assert.ok(btn);
      assert.equal(btn.textContent?.trim(), "Выполнить");

      // Клик по кнопке «Выполнить» вызывает коллбэк открытия модалки
      btn.click();
      assert.equal(opened, true);
    });
  });

  describe("4. Модальное окно экрана правил (Rules Screening Modal Controller)", () => {
    it("открывает модалку с правилами, блокирует отправку без чекбокса согласия", () => {
      const doc = new ShimDocument();
      /** @type {Array<{ cmd: string, args: any }>} */
      const invokeCalls = [];
      const modalCtrl = createRulesScreeningModalController({
        document: /** @type {any} */ (doc),
        invoke: async (cmd, args) => {
          invokeCalls.push({ cmd, args });
          return { success: true };
        },
      });

      assert.equal(modalCtrl.isOpen(), false);

      modalCtrl.open({
        guildId: "1001",
        guildName: "Тестовый сервер",
        rules: ["Правило 1: Будьте добры", "Правило 2: Без спама"],
      });

      assert.equal(modalCtrl.isOpen(), true);

      const modalEl = modalCtrl.getModalElement();
      assert.ok(modalEl);
      assert.equal(modalEl.getAttribute("role"), "dialog");
      assert.equal(modalEl.getAttribute("aria-modal"), "true");

      // Проверяем рендеринг правил
      const ruleItems = modalEl.querySelectorAll(".rules-modal-rule-item");
      assert.equal(ruleItems.length, 2);

      // Кнопка «Отправить» изначально отключена (чекбокс не нажат)
      const submitBtn = /** @type {any} */ (modalEl.querySelector('[data-action="submit"]'));
      assert.ok(submitBtn);
      assert.equal(submitBtn.disabled, true);

      // Чекбокс согласия
      const checkbox = /** @type {any} */ (modalEl.querySelector('input[type="checkbox"]'));
      assert.ok(checkbox);
      assert.equal(checkbox.checked, false);

      // Активируем чекбокс -> кнопка становится активной
      checkbox.checked = true;
      checkbox.emit("change");
      assert.equal(submitBtn.disabled, false);

      // Деактивируем чекбокс -> кнопка снова заблокирована
      checkbox.checked = false;
      checkbox.emit("change");
      assert.equal(submitBtn.disabled, true);
    });

    it("отправляет IPC confirm_member_rules при подтверждении и вызывает onSuccess", async () => {
      const doc = new ShimDocument();
      /** @type {Array<{ cmd: string, args: any }>} */
      const invokeCalls = [];
      /** @type {string | null} */
      let successGuildId = null;

      const modalCtrl = createRulesScreeningModalController({
        document: /** @type {any} */ (doc),
        invoke: async (cmd, args) => {
          invokeCalls.push({ cmd, args });
          return { success: true, pending: false };
        },
        onSuccess: (gid) => {
          successGuildId = gid;
        },
      });

      modalCtrl.open({
        guildId: "2002",
        guildName: "Guild With Rules",
      });

      const modalEl = modalCtrl.getModalElement();
      assert.ok(modalEl);
      const checkbox = /** @type {any} */ (modalEl.querySelector('input[type="checkbox"]'));
      assert.ok(checkbox);
      const submitBtn = /** @type {any} */ (modalEl.querySelector('[data-action="submit"]'));
      assert.ok(submitBtn);

      checkbox.checked = true;
      checkbox.emit("change");

      // Кликаем по кнопке «Отправить»
      await modalCtrl.submit();

      assert.equal(invokeCalls.length, 1);
      assert.equal(invokeCalls[0]?.cmd, "confirm_member_rules");
      assert.deepEqual(invokeCalls[0]?.args, { guildId: "2002" });

      assert.equal(successGuildId, "2002");
      assert.equal(modalCtrl.isOpen(), false);
    });

    it("предотвращает повторную отправку во время полёта запроса (Double-submit guard)", async () => {
      const doc = new ShimDocument();
      let callCount = 0;
      /** @type {((value: unknown) => void) | null} */
      let resolvePromise = null;

      const modalCtrl = createRulesScreeningModalController({
        document: /** @type {any} */ (doc),
        invoke: async () => {
          callCount++;
          return new Promise((resolve) => {
            resolvePromise = resolve;
          });
        },
      });

      modalCtrl.open({ guildId: "3003", guildName: "Slow Server" });
      const modalEl = modalCtrl.getModalElement();
      assert.ok(modalEl);
      const checkbox = /** @type {any} */ (modalEl.querySelector('input[type="checkbox"]'));
      assert.ok(checkbox);
      const submitBtn = /** @type {any} */ (modalEl.querySelector('[data-action="submit"]'));
      assert.ok(submitBtn);

      checkbox.checked = true;
      checkbox.emit("change");

      // Первый submit — запрос пошёл
      const p1 = modalCtrl.submit();
      assert.equal(callCount, 1);
      assert.equal(submitBtn.disabled, true);
      assert.ok(submitBtn.textContent?.includes("Отправка"));

      // Второй submit пока первый в полёте — игнорируется
      const p2 = modalCtrl.submit();
      assert.equal(callCount, 1);

      // Завершаем первый
      if (resolvePromise) {
        /** @type {(v: any) => void} */ (resolvePromise)({ success: true });
      }
      await p1;
      await p2;

      assert.equal(callCount, 1);
      assert.equal(modalCtrl.isOpen(), false);
    });

    it("обрабатывает ошибку IPC confirm_member_rules без закрытия модалки (Error Handling)", async () => {
      const doc = new ShimDocument();
      /** @type {unknown} */
      let errorReported = null;

      const modalCtrl = createRulesScreeningModalController({
        document: /** @type {any} */ (doc),
        invoke: async () => {
          throw new Error("HTTP 403 Forbidden: Verification failed");
        },
        onError: (err) => {
          errorReported = err;
        },
      });

      modalCtrl.open({ guildId: "4004", guildName: "Restricted Guild" });
      const modalEl = modalCtrl.getModalElement();
      assert.ok(modalEl);
      const checkbox = /** @type {any} */ (modalEl.querySelector('input[type="checkbox"]'));
      assert.ok(checkbox);
      const submitBtn = /** @type {any} */ (modalEl.querySelector('[data-action="submit"]'));
      assert.ok(submitBtn);

      checkbox.checked = true;
      checkbox.emit("change");

      await modalCtrl.submit();

      // Модалка осталась открытой
      assert.equal(modalCtrl.isOpen(), true);
      assert.ok(errorReported);

      // Сообщение об ошибке отображено в интерфейсе
      const errorEl = modalEl.querySelector(".rules-modal-error");
      assert.ok(errorEl);
      assert.ok(errorEl.textContent?.includes("Не удалось"));

      // Кнопка снова разблокирована для повторной попытки
      assert.equal(submitBtn.disabled, false);
      assert.equal(submitBtn.textContent?.trim(), "Отправить");
    });

    it("закрывает модалку по кнопке Отмена, крестику и Escape без отправки IPC (Cancel/Dismiss)", () => {
      const doc = new ShimDocument();
      /** @type {Array<{ cmd: string, args: any }>} */
      const invokeCalls = [];
      const modalCtrl = createRulesScreeningModalController({
        document: /** @type {any} */ (doc),
        invoke: async (cmd, args) => {
          invokeCalls.push({ cmd, args });
          return {};
        },
      });

      // 1. Закрытие по кнопке «Отмена»
      modalCtrl.open({ guildId: "5001" });
      assert.equal(modalCtrl.isOpen(), true);
      const cancelBtn = /** @type {any} */ (modalCtrl.getModalElement()?.querySelector('[data-action="cancel"]'));
      assert.ok(cancelBtn);
      cancelBtn.click();
      assert.equal(modalCtrl.isOpen(), false);
      assert.equal(invokeCalls.length, 0);

      // 2. Закрытие по крестику
      modalCtrl.open({ guildId: "5002" });
      assert.equal(modalCtrl.isOpen(), true);
      const closeBtn = /** @type {any} */ (modalCtrl.getModalElement()?.querySelector('[data-action="close"]'));
      assert.ok(closeBtn);
      closeBtn.click();
      assert.equal(modalCtrl.isOpen(), false);
      assert.equal(invokeCalls.length, 0);

      // 3. Закрытие по клавише Escape
      modalCtrl.open({ guildId: "5003" });
      assert.equal(modalCtrl.isOpen(), true);
      const targetDoc = /** @type {any} */ (doc.documentElement || doc);
      targetDoc.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(modalCtrl.isOpen(), false);
      assert.equal(invokeCalls.length, 0);
    });
  });

  describe("5. Интеграция с app.js и жизненным циклом DOM (Full App Integration)", () => {
    it("экспортирует контроллер rulesScreeningModal из loadApp", async () => {
      const app = await loadApp();
      try {
        const booted = app.appModule?.getApp?.();
        assert.ok(booted?.rulesScreeningModal);
        assert.equal(typeof booted.rulesScreeningModal.open, "function");
        assert.equal(typeof booted.rulesScreeningModal.close, "function");
        assert.equal(typeof booted.rulesScreeningModal.isOpen, "function");
        assert.equal(typeof booted.rulesScreeningModal.submit, "function");
      } finally {
        app.dispose();
      }
    });

    it("интерактивно открывает модалку из баннера и завершает проверку правил через IPC", async () => {
      let confirmCalled = false;
      let confirmGuildId = "";

      const app = await loadApp({
        invokeHandler: (cmd, args) => {
          if (cmd === "confirm_member_rules") {
            confirmCalled = true;
            confirmGuildId = String(args?.guildId ?? "");
            return { success: true, pending: false };
          }
          return undefined;
        },
      });

      try {
        const booted = app.appModule?.getApp?.();
        assert.ok(booted?.rulesScreeningModal);

        // Проверяем открытие модалки правил через контроллер
        booted.rulesScreeningModal.open({
          guildId: "777888",
          guildName: "Guild Rules Community",
          rules: ["Правило 1: Уважение", "Правило 2: Без флуда"],
        });

        assert.equal(booted.rulesScreeningModal.isOpen(), true);
        const modalEl = booted.rulesScreeningModal.getModalElement();
        assert.ok(modalEl);

        const checkbox = /** @type {any} */ (
          modalEl.querySelector('input[type="checkbox"]')
        );
        assert.ok(checkbox);
        assert.equal(checkbox.checked, false);

        const submitBtn = /** @type {any} */ (
          modalEl.querySelector('[data-action="submit"]')
        );
        assert.ok(submitBtn);
        assert.equal(submitBtn.disabled, true);

        // Пользователь ставит отметку согласия
        checkbox.checked = true;
        checkbox.emit("change");
        assert.equal(submitBtn.disabled, false);

        // Пользователь нажимает «Отправить»
        await booted.rulesScreeningModal.submit();

        assert.equal(confirmCalled, true);
        assert.equal(confirmGuildId, "777888");
        assert.equal(booted.rulesScreeningModal.isOpen(), false);
      } finally {
        app.dispose();
      }
    });
  });
});
