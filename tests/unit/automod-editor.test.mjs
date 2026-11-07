// @ts-check
/**
 * Unit-тесты контроллера редактора правил автомодерации и фильтра ключевых слов (task-251 / FE-P0-13).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  AUTOMOD_ACTION_LABELS_RU,
  AUTOMOD_TRIGGER_LABELS_RU,
  createAutoModEditorController,
  parseKeywordListInput,
} from "../../rust/gui-spike/ui/features/automod-editor.js";

function makeContainer() {
  const doc = new ShimDocument();
  const container = doc.createElement("div");
  doc.body.appendChild(container);
  return container;
}

describe("automod-editor: словари меток триггеров и действий", () => {
  it("T-251-01: AUTOMOD_TRIGGER_LABELS_RU и AUTOMOD_ACTION_LABELS_RU заморожены и содержат точные русские метки", () => {
    assert.equal(Object.isFrozen(AUTOMOD_TRIGGER_LABELS_RU), true);
    assert.equal(Object.isFrozen(AUTOMOD_ACTION_LABELS_RU), true);

    assert.deepEqual(
      { ...AUTOMOD_TRIGGER_LABELS_RU },
      {
        1: "Фильтр ключевых слов",
        3: "Защита от спама",
        4: "Часто упоминаемые слова",
        5: "Спам упоминаниями",
      },
    );

    assert.deepEqual(
      { ...AUTOMOD_ACTION_LABELS_RU },
      {
        1: "Блокировать сообщение",
        2: "Отправить предупреждение",
        3: "Тайм-аут участника",
      },
    );
  });
});

describe("automod-editor: parseKeywordListInput", () => {
  it("T-251-02: разбивает по запятым и переводам строк, обрезает пробелы и отбрасывает пустые токены", () => {
    const raw = "  спам , реклама\nфишинг\r\n ,  \n\n скам , ";
    assert.deepEqual(parseKeywordListInput(raw), ["спам", "реклама", "фишинг", "скам"]);
    assert.deepEqual(parseKeywordListInput("   , \n \r\n "), []);
    assert.deepEqual(parseKeywordListInput(""), []);
    assert.deepEqual(parseKeywordListInput(/** @type {any} */ (null)), []);
  });

  it("T-251-03: сохраняет токен длиной ровно 60 символов и отбрасывает токены длиннее 60 символов", () => {
    const exact60 = "а".repeat(60);
    const over61 = "б".repeat(61);
    const result = parseKeywordListInput(`${exact60}, ${over61}, нормальное`);
    assert.deepEqual(result, [exact60, "нормальное"]);
  });

  it("T-251-04: дедуплицирует без учёта регистра с сохранением первого встреченного регистра", () => {
    const raw = "DiscordNitro, discordnitro, DISCORDNITRO, FreeSteam, freesteam, ФриНитро, фринитро";
    assert.deepEqual(parseKeywordListInput(raw), ["DiscordNitro", "FreeSteam", "ФриНитро"]);
  });

  it("T-251-05: ограничивает результат максимум 1000 ключевыми словами", () => {
    const words = Array.from({ length: 1050 }, (_, idx) => `kw-${idx + 1}`);
    const parsed = parseKeywordListInput(words.join(", "));
    assert.equal(parsed.length, 1000);
    assert.equal(parsed[0], "kw-1");
    assert.equal(parsed[999], "kw-1000");
  });
});

describe("automod-editor: createAutoModEditorController", () => {
  const SAMPLE_RULES = [
    {
      id: "rule-1",
      name: "Стоп-слова в чате",
      triggerType: 1,
      actionType: 1,
      keywordFilter: ["спам", "фишинг"],
      enabled: true,
    },
    {
      id: "rule-2",
      name: "Анти-рейд упоминаниями",
      triggerType: 5,
      actionType: 3,
      keywordFilter: [],
      enabled: false,
    },
  ];

  it("T-251-06: рендерит регион [data-automod-editor] и карточки правил с метками и переключателем role=switch", () => {
    const container = makeContainer();
    createAutoModEditorController({
      container,
      rules: SAMPLE_RULES,
    });

    const region = container.querySelector("[data-automod-editor]");
    assert.ok(region, "регион [data-automod-editor] должен существовать");
    assert.equal(region.getAttribute("role"), "region");
    assert.equal(region.getAttribute("aria-label"), "Автомодерация");

    const card1 = container.querySelector('[data-automod-rule-id="rule-1"]');
    const card2 = container.querySelector('[data-automod-rule-id="rule-2"]');
    assert.ok(card1 && card2, "обе карточки правил должны быть отрендерены");

    assert.equal(card1.textContent.includes("Стоп-слова в чате"), true);
    assert.equal(card1.textContent.includes("Фильтр ключевых слов"), true);
    assert.equal(card1.textContent.includes("Блокировать сообщение"), true);

    assert.equal(card2.textContent.includes("Анти-рейд упоминаниями"), true);
    assert.equal(card2.textContent.includes("Спам упоминаниями"), true);
    assert.equal(card2.textContent.includes("Тайм-аут участника"), true);

    const toggle1 = container.querySelector('[data-automod-rule-toggle="rule-1"]');
    const toggle2 = container.querySelector('[data-automod-rule-toggle="rule-2"]');
    assert.ok(toggle1 && toggle2);
    assert.equal(toggle1.getAttribute("role"), "switch");
    assert.equal(toggle1.getAttribute("aria-checked"), "true");
    assert.equal(toggle2.getAttribute("role"), "switch");
    assert.equal(toggle2.getAttribute("aria-checked"), "false");
  });

  it("T-251-07: клик по [data-automod-rule-toggle] переключает enabled, обновляет aria-checked и вызывает onToggleRule", () => {
    const container = makeContainer();
    /** @type {Array<{ ruleId: string, enabled: boolean }>} */
    const toggled = [];

    const ctrl = createAutoModEditorController({
      container,
      rules: SAMPLE_RULES,
      onToggleRule: (ruleId, enabled) => toggled.push({ ruleId, enabled }),
    });

    container.querySelector('[data-automod-rule-toggle="rule-1"]')?.click();
    assert.deepEqual(toggled, [{ ruleId: "rule-1", enabled: false }]);
    assert.equal(
      container.querySelector('[data-automod-rule-toggle="rule-1"]')?.getAttribute("aria-checked"),
      "false",
    );
    assert.equal(ctrl.getState().rules[0]?.enabled, false);

    container.querySelector('[data-automod-rule-toggle="rule-2"]')?.click();
    assert.deepEqual(toggled, [
      { ruleId: "rule-1", enabled: false },
      { ruleId: "rule-2", enabled: true },
    ]);
    assert.equal(
      container.querySelector('[data-automod-rule-toggle="rule-2"]')?.getAttribute("aria-checked"),
      "true",
    );
    assert.equal(ctrl.getState().rules[1]?.enabled, true);
  });

  it("T-251-08: форма [data-automod-form] сохраняет правило с распарсенными ключевыми словами и игнорирует пустое имя", () => {
    const container = makeContainer();
    /** @type {Array<{ name: string, triggerType: number, actionType: number, keywordFilter: string[] }>} */
    const saved = [];

    createAutoModEditorController({
      container,
      rules: [],
      onSaveRule: (payload) => saved.push(payload),
    });

    const form = container.querySelector("[data-automod-form]");
    const nameInput = container.querySelector("[data-automod-name-input]");
    const triggerSelect = container.querySelector("[data-automod-trigger-select]");
    const actionSelect = container.querySelector("[data-automod-action-select]");
    const keywordsInput = container.querySelector("[data-automod-keywords-input]");
    const saveBtn = container.querySelector("[data-automod-save-btn]");

    assert.ok(form && nameInput && triggerSelect && actionSelect && keywordsInput && saveBtn);

    // Пустое/пробельное имя не должно вызывать onSaveRule
    nameInput.value = "   ";
    keywordsInput.value = "спам, реклама";
    saveBtn.click();
    assert.equal(saved.length, 0);

    // Заполненная форма вызывает onSaveRule с нормализованным payload
    nameInput.value = "  Фильтр ссылок и спама  ";
    triggerSelect.value = "1";
    actionSelect.value = "2";
    keywordsInput.value = " BadLink , badlink\n  Фишинг , " + "x".repeat(61);
    saveBtn.click();

    assert.deepEqual(saved, [
      {
        name: "Фильтр ссылок и спама",
        triggerType: 1,
        actionType: 2,
        keywordFilter: ["BadLink", "Фишинг"],
      },
    ]);
  });

  it("T-251-09: поддерживает setState, иммутабельный getState и destroy", () => {
    const container = makeContainer();
    const firstRule = SAMPLE_RULES[0];
    assert.ok(firstRule);
    const ctrl = createAutoModEditorController({
      container,
      rules: [firstRule],
    });

    const snap = ctrl.getState();
    assert.equal(snap.rules.length, 1);
    const snapRule0 = snap.rules[0];
    assert.ok(snapRule0);
    snapRule0.name = "Мутированное имя";
    snap.rules.push({
      id: "injected",
      name: "Injected",
      triggerType: 3,
      actionType: 1,
      keywordFilter: [],
      enabled: true,
    });
    assert.equal(ctrl.getState().rules.length, 1);
    assert.equal(ctrl.getState().rules[0]?.name, "Стоп-слова в чате");

    ctrl.setState({
      rules: [
        {
          id: "rule-3",
          name: "Частые слова",
          triggerType: 4,
          actionType: 2,
          keywordFilter: ["флуд"],
          enabled: true,
        },
      ],
    });

    assert.equal(container.querySelector('[data-automod-rule-id="rule-1"]'), null);
    const card3 = container.querySelector('[data-automod-rule-id="rule-3"]');
    assert.ok(card3);
    assert.equal(card3.textContent.includes("Часто упоминаемые слова"), true);
    assert.equal(card3.textContent.includes("Отправить предупреждение"), true);

    ctrl.destroy();
    assert.equal(container.querySelector("[data-automod-editor]"), null);
  });
});
