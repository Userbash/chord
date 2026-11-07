// @ts-check
/**
 * Unit-тесты модуля `rust/gui-spike/ui/features/pins-threads.js`:
 * - STUB-10 (M-10 и M-11): `createPinsThreadsPanels`
 * - FE-P0-06 (task-152): `createPinsThreadsController` и DOM-адаптер `mountPinsThreadsDom(rootEl, controllerOrOpts)`
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument, ShimElement } from "../support/gui-dom-harness.mjs";
import * as pinsThreadsModule from "../../rust/gui-spike/ui/features/pins-threads.js";
import {
  createPinsThreadsPanels,
  createPinsThreadsController,
  mountPinsThreadsDom,
} from "../../rust/gui-spike/ui/features/pins-threads.js";

const mod = {
  ...pinsThreadsModule,
  createPinsThreadsController,
  mountPinsThreadsDom,
};

/**
 * Создаёт изолированное DOM-окружение для тестов пинов и веток.
 * @param {{ withHeaderButtons?: boolean }} [opts]
 */
function createDomEnv(opts = {}) {
  const { withHeaderButtons = true } = opts;
  const doc = new ShimDocument();
  const rootEl = doc.createElement("div");
  rootEl.id = "chat-workspace";
  rootEl.className = "content";

  const header = doc.createElement("header");
  header.id = "chat-header";
  header.className = "chat-header";

  const actions = doc.createElement("div");
  actions.className = "chat-actions";

  /** @type {ShimElement | null} */
  let btnPins = null;
  /** @type {ShimElement | null} */
  let btnThreads = null;

  if (withHeaderButtons) {
    btnPins = doc.createElement("button");
    btnPins.id = "btn-TogglePins";
    btnPins.type = "button";
    btnPins.className = "icon-btn";
    btnPins.setAttribute("data-action", "pins");
    btnPins.setAttribute("aria-label", "Закреплённые сообщения");
    btnPins.textContent = "📌";

    btnThreads = doc.createElement("button");
    btnThreads.id = "btn-ToggleThreads";
    btnThreads.type = "button";
    btnThreads.className = "icon-btn";
    btnThreads.setAttribute("data-action", "threads");
    btnThreads.setAttribute("aria-label", "Ветки");
    btnThreads.textContent = "🧵";

    actions.append(btnPins, btnThreads);
  }

  header.append(actions);
  rootEl.append(header);
  doc.body.append(rootEl);

  return { doc, rootEl, header, actions, btnPins, btnThreads };
}

describe("STUB-10 (M-10 и M-11): базовый контроллер createPinsThreadsPanels", () => {
  it("прогрессивно создаёт #chat-pins-panel и #chat-threads-panel и переключает их взаимно исключающе", () => {
    const { doc, rootEl, btnPins, btnThreads } = createDomEnv({ withHeaderButtons: true });
    assert.ok(btnPins && btnThreads);

    const ctrl = createPinsThreadsPanels({
      document: doc,
      container: rootEl,
      pinsButton: btnPins,
      threadsButton: btnThreads,
    });

    assert.equal(ctrl.pinsPanel.id, "chat-pins-panel");
    assert.equal(ctrl.threadsPanel.id, "chat-threads-panel");
    assert.equal(btnPins.getAttribute("aria-expanded"), "false");
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");

    btnPins.click();
    assert.equal(btnPins.getAttribute("aria-expanded"), "true");
    assert.equal(ctrl.pinsPanel.hasAttribute("hidden"), false);
    assert.equal(ctrl.threadsPanel.hasAttribute("hidden"), true);

    btnThreads.click();
    assert.equal(btnThreads.getAttribute("aria-expanded"), "true");
    assert.equal(ctrl.threadsPanel.hasAttribute("hidden"), false);
    assert.equal(btnPins.getAttribute("aria-expanded"), "false");
    assert.equal(ctrl.pinsPanel.hasAttribute("hidden"), true);

    ctrl.destroy();
  });
});

describe("FE-P0-06 / task-152: mountPinsThreadsDom(rootEl, controllerOrOpts)", () => {
  it("привязывается к существующим кнопкам #btn-TogglePins и #btn-ToggleThreads, синхронизирует aria-expanded и переключает #pins-popover (role='dialog') и #threads-sidebar", () => {
    assert.equal(typeof mod.mountPinsThreadsDom, "function", "mountPinsThreadsDom должна быть экспортирована");
    assert.equal(typeof mod.createPinsThreadsController, "function", "createPinsThreadsController должна быть экспортирована");

    const { doc, rootEl, btnPins, btnThreads } = createDomEnv({ withHeaderButtons: true });
    assert.ok(btnPins && btnThreads);

    const controller = mod.createPinsThreadsController({
      pins: [{ id: "pin-1", author: "Alice", content: "Привет", timestamp: "12:00" }],
      activeThreads: [{ id: "th-1", name: "Ветка 1", replyCount: 3, archived: false }],
    });

    const mounted = mod.mountPinsThreadsDom(rootEl, controller);

    const pinsPopover = doc.getElementById("pins-popover");
    const threadsSidebar = doc.getElementById("threads-sidebar");
    const threadViewPanel = doc.getElementById("thread-view-panel");

    assert.ok(pinsPopover, "#pins-popover должен быть создан в DOM");
    assert.ok(threadsSidebar, "#threads-sidebar должен быть создан в DOM");
    assert.ok(threadViewPanel, "#thread-view-panel должен быть создан в DOM");

    assert.equal(pinsPopover.getAttribute("role"), "dialog");
    assert.equal(btnPins.getAttribute("aria-expanded"), "false");
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");
    assert.equal(pinsPopover.hasAttribute("hidden"), true);
    assert.equal(threadsSidebar.hasAttribute("hidden"), true);

    // Открытие поповера пинов
    btnPins.click();
    assert.equal(btnPins.getAttribute("aria-expanded"), "true");
    assert.equal(pinsPopover.hasAttribute("hidden"), false);
    assert.equal(threadsSidebar.hasAttribute("hidden"), true);
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");

    // Переключение на боковую панель веток (взаимное исключение)
    btnThreads.click();
    assert.equal(btnThreads.getAttribute("aria-expanded"), "true");
    assert.equal(threadsSidebar.hasAttribute("hidden"), false);
    assert.equal(btnPins.getAttribute("aria-expanded"), "false");
    assert.equal(pinsPopover.hasAttribute("hidden"), true);

    // Повторный клик закрывает боковую панель веток
    btnThreads.click();
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");
    assert.equal(threadsSidebar.hasAttribute("hidden"), true);

    mounted.destroy();
  });

  it("динамически создаёт #btn-TogglePins и #btn-ToggleThreads при их отсутствии в шапке канала", () => {
    assert.equal(typeof mod.mountPinsThreadsDom, "function");
    const { doc, rootEl } = createDomEnv({ withHeaderButtons: false });

    assert.equal(doc.getElementById("btn-TogglePins"), null);
    assert.equal(doc.getElementById("btn-ToggleThreads"), null);

    const mounted = mod.mountPinsThreadsDom(rootEl, {
      pins: [],
      activeThreads: [],
    });

    const btnPins = doc.getElementById("btn-TogglePins");
    const btnThreads = doc.getElementById("btn-ToggleThreads");
    const pinsPopover = doc.getElementById("pins-popover");
    const threadsSidebar = doc.getElementById("threads-sidebar");

    assert.ok(btnPins, "#btn-TogglePins должна создаваться динамически при отсутствии");
    assert.ok(btnThreads, "#btn-ToggleThreads должна создаваться динамически при отсутствии");
    assert.ok(pinsPopover, "#pins-popover должен быть создан");
    assert.ok(threadsSidebar, "#threads-sidebar должен быть создан");

    assert.equal(btnPins.getAttribute("aria-expanded"), "false");
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");
    assert.equal(pinsPopover.getAttribute("role"), "dialog");

    btnPins.click();
    assert.equal(btnPins.getAttribute("aria-expanded"), "true");
    assert.equal(pinsPopover.hasAttribute("hidden"), false);

    mounted.destroy();
  });

  it("рендерит список закреплённых сообщений в #pins-popover (автор, текст, 'Перейти', 'Открепить' с подтверждением -> unpinMessage) и пустое состояние", async () => {
    assert.equal(typeof mod.mountPinsThreadsDom, "function");
    const { doc, rootEl, btnPins } = createDomEnv({ withHeaderButtons: true });
    assert.ok(btnPins);

    /** @type {string[]} */
    const jumpedIds = [];
    /** @type {string[]} */
    const unpinnedIds = [];

    const mounted = mod.mountPinsThreadsDom(rootEl, {
      pins: [
        { id: "m-101", author: "Sanya", content: "Релиз в пятницу", timestamp: "14:20" },
        { id: "m-102", author: "Bot", content: "Правила канала", timestamp: "15:00" },
      ],
      onJumpToMessage: (/** @type {string} */ id) => {
        jumpedIds.push(id);
      },
      unpinMessage: async (/** @type {string} */ id) => {
        unpinnedIds.push(id);
      },
    });

    btnPins.click();
    const pinsPopover = /** @type {ShimElement} */ (doc.getElementById("pins-popover"));
    assert.ok(pinsPopover);
    assert.equal(pinsPopover.getAttribute("data-state"), "populated");

    let cards = pinsPopover.querySelectorAll(".pin-item");
    assert.equal(cards.length, 2, "должно быть отрендерено 2 закреплённых сообщения");
    assert.ok(cards[0]?.textContent.includes("Sanya"), "автор первого пина");
    assert.ok(cards[0]?.textContent.includes("Релиз в пятницу"), "текст первого пина");
    assert.ok(cards[1]?.textContent.includes("Bot"), "автор второго пина");
    assert.ok(cards[1]?.textContent.includes("Правила канала"), "текст второго пина");

    // Проверка кнопки 'Перейти'
    const jumpBtn = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="jump-to-message"][data-message-id="m-101"]')
    );
    assert.ok(jumpBtn, "кнопка 'Перейти' должна присутствовать");
    assert.equal(jumpBtn.textContent, "Перейти");
    jumpBtn.click();
    assert.deepEqual(jumpedIds, ["m-101"]);

    // Проверка кнопки 'Открепить' с подтверждением (сначала отмена, затем подтверждение)
    const unpinBtn1 = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="unpin-message"][data-message-id="m-101"]')
    );
    assert.ok(unpinBtn1, "кнопка 'Открепить' должна присутствовать");
    assert.equal(unpinBtn1.textContent, "Открепить");
    unpinBtn1.click();

    // До подтверждения unpinMessage ещё не вызван
    assert.deepEqual(unpinnedIds, [], "без подтверждения unpinMessage не должен вызываться");
    const confirmBox = pinsPopover.querySelector(".pin-unpin-confirm");
    assert.ok(confirmBox, "должен появиться блок подтверждения открепления");

    const cancelBtn = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="cancel-unpin"][data-message-id="m-101"]')
    );
    assert.ok(cancelBtn, "кнопка отмены открепления");
    cancelBtn.click();
    assert.equal(pinsPopover.querySelector(".pin-unpin-confirm"), null, "после отмены подтверждение скрыто");
    assert.equal(pinsPopover.querySelectorAll(".pin-item").length, 2);

    // Повторно нажимаем 'Открепить' и подтверждаем
    const unpinBtnAgain = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="unpin-message"][data-message-id="m-101"]')
    );
    unpinBtnAgain.click();
    const confirmBtn1 = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="confirm-unpin"][data-message-id="m-101"]')
    );
    assert.ok(confirmBtn1, "кнопка подтверждения открепления");
    confirmBtn1.click();
    await Promise.resolve();

    assert.deepEqual(unpinnedIds, ["m-101"], "после подтверждения вызван unpinMessage('m-101')");
    cards = pinsPopover.querySelectorAll(".pin-item");
    assert.equal(cards.length, 1, "остался 1 пин после удаления первого");

    // Открепляем второй пин -> переход в пустое состояние
    const unpinBtn2 = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="unpin-message"][data-message-id="m-102"]')
    );
    unpinBtn2.click();
    const confirmBtn2 = /** @type {ShimElement} */ (
      pinsPopover.querySelector('[data-action="confirm-unpin"][data-message-id="m-102"]')
    );
    confirmBtn2.click();
    await Promise.resolve();

    assert.deepEqual(unpinnedIds, ["m-101", "m-102"]);
    assert.equal(pinsPopover.querySelectorAll(".pin-item").length, 0);
    assert.equal(pinsPopover.getAttribute("data-state"), "empty");
    assert.ok(
      pinsPopover.textContent.includes("нет закреплённых сообщений"),
      "должно отображаться пустое состояние #pins-popover",
    );

    mounted.destroy();
  });

  it("рендерит вкладки Активные / Архивные ветки в #threads-sidebar, форму создания новой ветки (1..100 символов, auto_archive_duration -> createThread) и переход внутрь ветки", async () => {
    assert.equal(typeof mod.mountPinsThreadsDom, "function");
    const { doc, rootEl, btnThreads } = createDomEnv({ withHeaderButtons: true });
    assert.ok(btnThreads);

    /** @type {Array<{ name: string, auto_archive_duration: number }>} */
    const createdPayloads = [];
    /** @type {string[]} */
    const openedThreads = [];

    const mounted = mod.mountPinsThreadsDom(rootEl, {
      activeThreads: [
        { id: "t-active-1", name: "Обсуждение UI", replyCount: 7, lastMessageTime: "10:00", archived: false },
      ],
      archivedThreads: [
        { id: "t-arch-1", name: "Старый спринт", replyCount: 24, lastMessageTime: "вчера", archived: true },
      ],
      createThread: async (/** @type {{ name: string, auto_archive_duration: number }} */ payload) => {
        createdPayloads.push({
          name: payload.name,
          auto_archive_duration: payload.auto_archive_duration,
        });
        return {
          id: "t-created-99",
          name: payload.name,
          replyCount: 0,
          archived: false,
          auto_archive_duration: payload.auto_archive_duration,
        };
      },
      onOpenThread: (/** @type {string} */ threadId) => {
        openedThreads.push(threadId);
      },
    });

    btnThreads.click();
    const sidebar = /** @type {ShimElement} */ (doc.getElementById("threads-sidebar"));
    const viewPanel = /** @type {ShimElement} */ (doc.getElementById("thread-view-panel"));
    assert.ok(sidebar && viewPanel);
    assert.equal(sidebar.hasAttribute("hidden"), false);

    // 1. Вкладки Активные / Архивные
    const tabActive = /** @type {ShimElement} */ (sidebar.querySelector("#tab-active-threads"));
    const tabArchived = /** @type {ShimElement} */ (sidebar.querySelector("#tab-archived-threads"));
    assert.ok(tabActive && tabArchived, "вкладки Активные и Архивные должны присутствовать");
    assert.equal(tabActive.getAttribute("aria-selected"), "true");
    assert.equal(tabArchived.getAttribute("aria-selected"), "false");

    let items = sidebar.querySelectorAll(".thread-item");
    assert.equal(items.length, 1);
    assert.ok(items[0]?.textContent.includes("Обсуждение UI"));

    // Переключение на Архивные
    tabArchived.click();
    assert.equal(tabArchived.getAttribute("aria-selected"), "true");
    assert.equal(tabActive.getAttribute("aria-selected"), "false");
    items = sidebar.querySelectorAll(".thread-item");
    assert.equal(items.length, 1);
    assert.ok(items[0]?.textContent.includes("Старый спринт"));

    // Возврат на Активные
    tabActive.click();
    assert.equal(tabActive.getAttribute("aria-selected"), "true");
    items = sidebar.querySelectorAll(".thread-item");
    assert.equal(items.length, 1);

    // 2. Переход внутрь существующей ветки
    const firstThreadItem = /** @type {ShimElement} */ (items[0]);
    firstThreadItem.click();
    assert.deepEqual(openedThreads, ["t-active-1"]);
    assert.equal(viewPanel.hasAttribute("hidden"), false, "#thread-view-panel должен открыться при переходе в ветку");
    assert.equal(viewPanel.getAttribute("data-thread-id"), "t-active-1");
    assert.ok(viewPanel.textContent.includes("Обсуждение UI"), "заголовок открытой ветки в #thread-view-panel");

    // 3. Валидация формы создания новой ветки (1..100 символов + auto_archive_duration)
    const nameInput = /** @type {ShimElement} */ (sidebar.querySelector("#thread-name-input"));
    const durationSelect = /** @type {ShimElement} */ (sidebar.querySelector("#thread-archive-duration"));
    const submitBtn = /** @type {ShimElement} */ (sidebar.querySelector("#btn-create-thread"));
    const formError = /** @type {ShimElement} */ (sidebar.querySelector("#thread-form-error"));
    assert.ok(nameInput && durationSelect && submitBtn && formError, "элементы формы создания ветки должны быть в #threads-sidebar");

    // Пустое название -> ошибка, createThread не вызывается
    nameInput.value = "   ";
    submitBtn.click();
    assert.equal(createdPayloads.length, 0, "пустое название не должно вызывать createThread");
    assert.equal(nameInput.getAttribute("aria-invalid"), "true");
    assert.equal(formError.hasAttribute("hidden"), false);

    // Слишком длинное название (> 100 символов) -> ошибка, createThread не вызывается
    nameInput.value = "А".repeat(101);
    submitBtn.click();
    assert.equal(createdPayloads.length, 0, "название > 100 символов не должно вызывать createThread");
    assert.equal(nameInput.getAttribute("aria-invalid"), "true");
    assert.equal(formError.hasAttribute("hidden"), false);

    // Валидное название (1..100 символов) и выбор auto_archive_duration = 4320
    nameInput.value = "Новая ветка релиза";
    durationSelect.value = "4320";
    durationSelect.emit("change");
    submitBtn.click();
    await Promise.resolve();

    assert.deepEqual(createdPayloads, [
      { name: "Новая ветка релиза", auto_archive_duration: 4320 },
    ]);
    assert.equal(nameInput.getAttribute("aria-invalid"), "false");
    assert.equal(formError.hasAttribute("hidden"), true);
    assert.equal(nameInput.value, "", "поле названия очищается после создания ветки");

    // Новая ветка добавлена в список и открыта в #thread-view-panel
    items = sidebar.querySelectorAll(".thread-item");
    assert.equal(items.length, 2, "новая ветка должна появиться в списке активных веток");
    assert.equal(viewPanel.getAttribute("data-thread-id"), "t-created-99");
    assert.ok(viewPanel.textContent.includes("Новая ветка релиза"));
    assert.deepEqual(openedThreads, ["t-active-1", "t-created-99"]);

    // 4. Закрытие #thread-view-panel кнопкой #btn-close-thread-view
    const closeViewBtn = /** @type {ShimElement} */ (viewPanel.querySelector("#btn-close-thread-view"));
    assert.ok(closeViewBtn, "кнопка закрытия просмотра ветки #btn-close-thread-view");
    closeViewBtn.click();
    assert.equal(viewPanel.hasAttribute("hidden"), true, "#thread-view-panel скрыт после закрытия");

    mounted.destroy();
  });

  it("закрывает поповер пинов и панель веток по Escape и полностью очищает слушатели через destroy()", () => {
    assert.equal(typeof mod.mountPinsThreadsDom, "function");
    const { doc, rootEl, btnPins, btnThreads } = createDomEnv({ withHeaderButtons: true });
    assert.ok(btnPins && btnThreads);

    const mounted = mod.mountPinsThreadsDom(rootEl, {
      pins: [{ id: "p-1", author: "User", content: "Текст", timestamp: "10:00" }],
      activeThreads: [{ id: "t-1", name: "Тред", replyCount: 1, archived: false }],
    });

    const pinsPopover = /** @type {ShimElement} */ (doc.getElementById("pins-popover"));
    const threadsSidebar = /** @type {ShimElement} */ (doc.getElementById("threads-sidebar"));
    assert.ok(pinsPopover && threadsSidebar);

    // Открываем пины -> закрываем по Escape
    btnPins.click();
    assert.equal(pinsPopover.hasAttribute("hidden"), false);
    assert.equal(btnPins.getAttribute("aria-expanded"), "true");

    doc.body.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(pinsPopover.hasAttribute("hidden"), true, "#pins-popover закрыт по Escape");
    assert.equal(btnPins.getAttribute("aria-expanded"), "false");
    assert.equal(doc.activeElement, btnPins, "фокус возвращён на #btn-TogglePins");

    // Открываем ветки -> закрываем по Escape
    btnThreads.click();
    assert.equal(threadsSidebar.hasAttribute("hidden"), false);
    assert.equal(btnThreads.getAttribute("aria-expanded"), "true");

    doc.body.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(threadsSidebar.hasAttribute("hidden"), true, "#threads-sidebar закрыт по Escape");
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");
    assert.equal(doc.activeElement, btnThreads, "фокус возвращён на #btn-ToggleThreads");

    // Уничтожаем адаптер через destroy() и проверяем, что слушатели сняты
    mounted.destroy();

    btnPins.click();
    assert.equal(pinsPopover.hasAttribute("hidden"), true, "после destroy() клик по #btn-TogglePins не открывает поповер");
    assert.equal(btnPins.getAttribute("aria-expanded"), "false");

    btnThreads.click();
    assert.equal(threadsSidebar.hasAttribute("hidden"), true, "после destroy() клик по #btn-ToggleThreads не открывает панель");
    assert.equal(btnThreads.getAttribute("aria-expanded"), "false");
  });
});
