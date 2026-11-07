// @ts-check
/**
 * FE-P2-01 (RW-40): доступный Tooltip вместо сырых `title` на иконках.
 *
 * Оракулы — артефакты, не реализация:
 *   - `/home/sanya/re-deep/frontend-d03.md` §5 п.7: компонент Tooltip, CSS-класс
 *     `tooltip_c36707` (+ `tooltipContent`, `tooltipDisablePointerEvents_c36707`);
 *   - `/home/sanya/re-deep/ru-catalog.json`: русские строки клиента — подписи
 *     иконок сверяются с каталогом, а не с константами модуля;
 *   - `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` §3 FE-P2-01: 12 подсказок на
 *     иконках, задержка 400 мс, `role="tooltip"`, Esc скрывает;
 *   - `rust/gui-spike/ui/tokens.css`: `--z-tooltip`, `--tooltip-radius`,
 *     `--tooltip-pad-x/y` — стили берут значения оттуда, не из литералов.
 *
 * Шим не всплывает события и не тикает таймерами: контроллер получает
 * инъецированные `setTimeoutFn`/`clearTimeoutFn`, тест двигает их сам.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  ICON_TOOLTIPS,
  TOOLTIP_SHOW_DELAY_MS,
  createTooltipController,
} from "../../rust/gui-spike/ui/features/tooltip.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimDocument} Doc */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} El */

/* ---------- независимые оракулы ---------- */

/**
 * Подписи, извлечённые из `/home/sanya/re-deep/ru-catalog.json` (ru-локаль
 * клиента 1.0.162) и `frontend-d03.md` §5 п.7. Список в тесте, а не чтение
 * дампа: тест не зависит от наличия лаборатории и не сверяется с модулем.
 * «Прикрепить вложения» — единственная строка каталога про вложение:
 * в ТЗ FE-P2-01 подпись иконки задана как «Прикрепить файл».
 */
const ORACLE_RU_TOOLTIPS = [
  "Заглушить",
  "Откл. звук",
  "Настройки пользователя",
  "Настройки ввода",
  "Настройки вывода",
  "Закреплённые сообщения",
  "Ветки",
  "Показать список участников",
  "Почта",
  "Поиск",
  "Добавить реакцию",
  "Прикрепить вложения",
  "Выбрать эмодзи",
];

/** Расхождение ТЗ с каталогом: подпись иконки вложения. */
const ORACLE_ATTACH_LABEL = "Прикрепить вложения";

const PRIMITIVES_CSS = readFileSync(
  new URL("../../rust/gui-spike/ui/components/primitives.css", import.meta.url),
  "utf8",
);

/** Канонические подписи иконок из ТЗ (FE-P2-01). @type {Record<string, string>} */
const REQUIRED_TOOLTIPS = {
  mute: "Заглушить",
  deafen: "Откл. звук",
  settings: "Настройки пользователя",
  voice_settings_input: "Настройки ввода",
  voice_settings_output: "Настройки вывода",
  pins: "Закреплённые сообщения",
  threads: "Ветки",
  members: "Показать список участников",
  inbox: "Почта",
  search: "Поиск",
  add_reaction: "Добавить реакцию",
  attach: "Прикрепить файл",
  emoji: "Выбрать эмодзи",
};

/* ---------- инъецированные таймеры ---------- */

/** @typedef {{ callback: () => void, delay: number }} TimerEntry */

/** Планировщик: колбэки выполняются только по `flush()`, задержки записываются. */
function makeClock() {
  let sequence = 0;
  /** @type {Map<number, TimerEntry>} */
  const pending = new Map();
  /** @type {number[]} */
  const delays = [];
  const state = { cancelled: 0 };
  return {
    delays,
    pending,
    state,
    /**
     * @param {() => void} callback
     * @param {number} delay
     * @returns {number}
     */
    setTimeout(callback, delay) {
      sequence += 1;
      delays.push(delay);
      pending.set(sequence, { callback, delay });
      return sequence;
    },
    /** @param {number} handle */
    clearTimeout(handle) {
      if (pending.delete(handle)) state.cancelled += 1;
    },
    /** Выполнить всё, что запланировано сейчас (снимок: новые таймеры не крутятся). */
    flush() {
      const callbacks = [...pending.values()].map((entry) => entry.callback);
      pending.clear();
      for (const callback of callbacks) callback();
    },
  };
}

/**
 * @param {Doc} doc
 * @param {Record<string, any>} [options]
 */
function makeController(doc, options = {}) {
  const clock = makeClock();
  const controller = createTooltipController(doc, {
    setTimeoutFn: clock.setTimeout,
    clearTimeoutFn: clock.clearTimeout,
    ...options,
  });
  return { clock, controller };
}

/**
 * Кнопка-иконка в теле документа.
 * @param {Doc} doc
 * @param {Record<string, string>} [attrs]
 * @returns {El}
 */
function addTarget(doc, attrs = {}) {
  const node = doc.createElement("button");
  node.setAttribute("type", "button");
  for (const [name, value] of Object.entries(attrs)) node.setAttribute(name, value);
  doc.body.appendChild(node);
  return node;
}

/**
 * Единственный общий элемент подсказки.
 * @param {Doc} doc
 * @returns {any}
 */
function tooltipNode(doc) {
  const nodes = doc.querySelectorAll("#discord-tooltip");
  assert.equal(nodes.length, 1, `#discord-tooltip должен быть один, найдено ${nodes.length}`);
  return nodes[0];
}

/* =============================================================================
 * Константы и каталог подписей
 * ========================================================================== */

describe("FE-P2-01: константы и каталог подсказок", () => {
  it("задержка показа — 400 мс из ТЗ, а не произвольное число", () => {
    assert.equal(TOOLTIP_SHOW_DELAY_MS, 400);
  });

  it("каталог заморожен: ≥12 иконок, непустые уникальные подписи", () => {
    assert.equal(Object.isFrozen(ICON_TOOLTIPS), true, "ICON_TOOLTIPS не заморожен");
    const entries = Object.entries(ICON_TOOLTIPS);
    assert.ok(entries.length >= 12, `иконок ${entries.length} < 12`);
    for (const [key, label] of entries) {
      assert.equal(typeof label, "string", `${key}: подпись не строка`);
      assert.notEqual(String(label).trim(), "", `${key}: пустая подпись`);
    }
    const labels = entries.map(([, label]) => label);
    assert.equal(new Set(labels).size, labels.length, "подписи дублируются между иконками");
  });

  it("подписи совпадают с ТЗ, а 12 из 13 — с каталогом клиента", () => {
    for (const [key, label] of Object.entries(REQUIRED_TOOLTIPS)) {
      assert.equal(
        /** @type {any} */ (ICON_TOOLTIPS)[key],
        label,
        `подпись ${key} разошлась с ТЗ`,
      );
    }
    /** @type {string[]} */
    const fromCatalog = [];
    for (const label of Object.values(REQUIRED_TOOLTIPS)) {
      if (ORACLE_RU_TOOLTIPS.includes(label)) fromCatalog.push(label);
      else assert.equal(label, "Прикрепить файл", `подпись «${label}» отсутствует в каталоге`);
    }
    assert.equal(
      fromCatalog.length,
      12,
      "с каталогом клиента совпало не 12 подписей — оракул изменился",
    );
    assert.ok(ORACLE_RU_TOOLTIPS.includes(ORACLE_ATTACH_LABEL), "оракул вложения потерян");
  });
});

/* =============================================================================
 * Общий элемент и задержка 400 мс
 * ========================================================================== */

describe("FE-P2-01: общий элемент подсказки", () => {
  it("один #discord-tooltip в body: role=tooltip, классы клиента, скрыт по умолчанию", () => {
    const doc = new ShimDocument();
    makeController(doc);
    const node = tooltipNode(doc);
    assert.equal(node.getAttribute("role"), "tooltip");
    assert.ok(node.className.split(" ").includes("tooltip"), "нет класса .tooltip");
    assert.ok(node.className.split(" ").includes("tooltip_c36707"), "нет класса tooltip_c36707");
    assert.equal(node.hasAttribute("hidden"), true, "подсказка видна без события");
    assert.equal(node.getAttribute("aria-hidden"), "true");
    assert.equal(node.id, "discord-tooltip");
  });

  it("второй контроллер переиспользует элемент, а не создаёт дубль", () => {
    const doc = new ShimDocument();
    makeController(doc);
    makeController(doc);
    assert.equal(tooltipNode(doc).hasAttribute("hidden"), true);
  });

  it("mouseenter показывает через 400 мс: текст, data-open, placement, aria-describedby", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Заглушить" });
    controller.attach(doc.body);

    const node = tooltipNode(doc);
    target.dispatchEvent("mouseenter");
    assert.deepEqual(clock.delays, [400], "задержка показа не 400 мс");
    assert.equal(clock.pending.size, 1, "показ не запланирован");
    assert.equal(node.hasAttribute("hidden"), true, "подсказка показана до задержки");

    clock.flush();
    assert.equal(node.hasAttribute("hidden"), false, "подсказка не показана после задержки");
    assert.equal(node.textContent, "Заглушить");
    assert.equal(node.getAttribute("aria-hidden"), "false");
    assert.equal(node.getAttribute("data-open"), "true");
    assert.equal(node.getAttribute("data-placement"), "top");
    assert.equal(target.getAttribute("aria-describedby"), "discord-tooltip");
  });
});

/* =============================================================================
 * Отмена, скрытие, клавиатура
 * ========================================================================== */

describe("FE-P2-01: отмена и скрытие", () => {
  it("mouseleave на 200 мс отменяет запланированный показ", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Настройки пользователя" });
    controller.attach(doc.body);
    const node = tooltipNode(doc);

    target.dispatchEvent("mouseenter");
    // 200 мс: таймер ещё не выполнялся, показывать нечего.
    assert.equal(node.hasAttribute("hidden"), true);
    target.dispatchEvent("mouseleave");
    assert.equal(clock.pending.size, 0, "таймер показа не снят");
    assert.equal(clock.state.cancelled, 1);
    clock.flush();
    assert.equal(node.hasAttribute("hidden"), true, "отменённая подсказка всё равно показалась");
    assert.equal(target.getAttribute("aria-describedby"), null);
  });

  it("mouseleave после показа скрывает сразу и снимает aria-describedby", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Ветки" });
    controller.attach(doc.body);
    const node = tooltipNode(doc);

    target.dispatchEvent("mouseenter");
    clock.flush();
    assert.equal(node.getAttribute("data-open"), "true");

    target.dispatchEvent("mouseleave");
    assert.equal(node.hasAttribute("hidden"), true, "подсказка не скрыта");
    assert.equal(node.getAttribute("aria-hidden"), "true");
    assert.equal(node.getAttribute("data-open"), "false");
    assert.equal(target.getAttribute("aria-describedby"), null, "aria-describedby остался");
  });

  it("click скрывает подсказку", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Почта" });
    controller.attach(doc.body);
    const node = tooltipNode(doc);
    target.dispatchEvent("mouseenter");
    clock.flush();
    target.dispatchEvent("click");
    assert.equal(node.hasAttribute("hidden"), true);
    assert.equal(target.getAttribute("aria-describedby"), null);
  });

  it("Escape скрывает сразу; прочие клавиши — нет", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Поиск" });
    controller.attach(doc.body);
    const node = tooltipNode(doc);

    target.dispatchEvent("mouseenter");
    clock.flush();
    doc.body.dispatchEvent("keydown", { key: "a" });
    assert.equal(node.hasAttribute("hidden"), false, "посторонняя клавиша скрыла подсказку");

    doc.body.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(node.hasAttribute("hidden"), true, "Escape не скрыл подсказку");
    assert.equal(node.getAttribute("aria-hidden"), "true");
    assert.equal(target.getAttribute("aria-describedby"), null);
  });

  it("Escape без активной подсказки не падает и не трогает состояние", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    addTarget(doc, { "data-tooltip": "Ветки" });
    controller.attach(doc.body);
    doc.body.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(tooltipNode(doc).hasAttribute("hidden"), true);
    assert.equal(clock.pending.size, 0);
  });

  it("клавиатура: focusin показывает после задержки, focusout скрывает", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Показать список участников" });
    controller.attach(doc.body);
    const node = tooltipNode(doc);

    target.dispatchEvent("focusin");
    assert.equal(node.hasAttribute("hidden"), true, "focusin показал подсказку без задержки");
    clock.flush();
    assert.equal(node.hasAttribute("hidden"), false, "focusin не показал подсказку");
    assert.equal(node.textContent, "Показать список участников");

    target.dispatchEvent("focusout");
    assert.equal(node.hasAttribute("hidden"), true, "focusout не скрыл подсказку");
  });
});

/* =============================================================================
 * Миграция title, placement, несколько целей
 * ========================================================================== */

describe("FE-P2-01: миграция raw title", () => {
  it("title переносится в data-tooltip и удаляется из DOM (нативная подсказка не дублирует)", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { title: "Откл. звук" });
    controller.attach(doc.body);

    assert.equal(target.getAttribute("title"), null, "сырой title остался в DOM");
    assert.equal(target.getAttribute("data-tooltip"), "Откл. звук");

    target.dispatchEvent("mouseenter");
    clock.flush();
    assert.equal(tooltipNode(doc).textContent, "Откл. звук");
  });

  it("data-tooltip приоритетнее title, title тоже снимается", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { title: "старая подпись", "data-tooltip": "Настройки вывода" });
    controller.attach(doc.body);
    assert.equal(target.getAttribute("data-tooltip"), "Настройки вывода");
    assert.equal(target.getAttribute("title"), null);
    target.dispatchEvent("mouseenter");
    clock.flush();
    assert.equal(tooltipNode(doc).textContent, "Настройки вывода");
  });

  it("data-placement цели прокидывается в элемент подсказки", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { "data-tooltip": "Выбрать эмодзи", "data-placement": "bottom" });
    controller.attach(doc.body);
    target.dispatchEvent("mouseenter");
    clock.flush();
    assert.equal(tooltipNode(doc).getAttribute("data-placement"), "bottom");
  });

  it("attach(root) привязывает несколько целей; показ второй снимает aria-describedby с первой", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const first = addTarget(doc, { "data-tooltip": "Добавить реакцию" });
    const second = addTarget(doc, { "data-tooltip": "Прикрепить файл" });
    assert.equal(controller.attach(doc.body), 2, "attach вернул не число привязанных целей");

    first.dispatchEvent("mouseenter");
    clock.flush();
    assert.equal(first.getAttribute("aria-describedby"), "discord-tooltip");
    second.dispatchEvent("mouseenter");
    clock.flush();
    assert.equal(second.getAttribute("aria-describedby"), "discord-tooltip");
    assert.equal(first.getAttribute("aria-describedby"), null, "aria-describedby остался на первой цели");
    assert.equal(tooltipNode(doc).textContent, "Прикрепить файл");
  });

  it("showFor(target) показывает сразу, без ожидания таймера", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { title: "Ветки" });
    controller.showFor(target);
    const node = tooltipNode(doc);
    assert.equal(node.hasAttribute("hidden"), false, "showFor не показал подсказку");
    assert.equal(node.textContent, "Ветки");
    assert.equal(target.getAttribute("title"), null);
    assert.equal(clock.pending.size, 0, "showFor оставил висящий таймер");
  });
});

/* =============================================================================
 * destroy
 * ========================================================================== */

describe("FE-P2-01: destroy", () => {
  it("снимает таймеры, слушатели и возвращает исходные title", () => {
    const doc = new ShimDocument();
    const { clock, controller } = makeController(doc);
    const target = addTarget(doc, { title: "Заглушить" });
    controller.attach(doc.body);

    target.dispatchEvent("mouseenter");
    assert.equal(clock.pending.size, 1);
    controller.destroy();
    assert.equal(clock.pending.size, 0, "destroy оставил запланированный показ");
    assert.equal(target.getAttribute("title"), "Заглушить", "исходный title не восстановлен");
    assert.equal(target.getAttribute("data-tooltip"), "Заглушить");

    // После destroy события цели ничего не планируют.
    target.dispatchEvent("mouseenter");
    assert.equal(clock.pending.size, 0, "слушатель цели не снят");
    assert.equal(tooltipNode(doc).hasAttribute("hidden"), true);
  });

  it("после destroy attach ничего не привязывает", () => {
    const doc = new ShimDocument();
    const { controller } = makeController(doc);
    controller.destroy();
    const target = addTarget(doc, { "data-tooltip": "Поиск" });
    assert.equal(controller.attach(doc.body), 0);
    target.dispatchEvent("mouseenter");
    assert.equal(tooltipNode(doc).hasAttribute("hidden"), true);
  });
});

/* =============================================================================
 * CSS: стили примитива
 * ========================================================================== */

describe("FE-P2-01: стили примитива в primitives.css", () => {
  it("есть .tooltip/.tooltip_c36707, hidden скрыт, visible не перехватывает мышь", () => {
    assert.match(PRIMITIVES_CSS, /(^|\n)\.tooltip(?=[\s,{.:[])/, "нет правила .tooltip");
    assert.match(PRIMITIVES_CSS, /(^|\n)\.tooltip_c36707(?=[\s,{.:[])/, "нет правила .tooltip_c36707");
    assert.match(PRIMITIVES_CSS, /z-index:\s*var\(--z-tooltip\)/, "слой подсказки не --z-tooltip");
    assert.match(PRIMITIVES_CSS, /pointer-events:\s*none/, "подсказка перехватывает мышь");
    assert.match(PRIMITIVES_CSS, /\.tooltip\[hidden\]/, "нет правила скрытого состояния");
    assert.match(PRIMITIVES_CSS, /border-radius:\s*var\(--tooltip-radius\)/, "радиус не из токена");
    assert.match(PRIMITIVES_CSS, /var\(--tooltip-pad-x\)/, "горизонтальный паддинг не из токена");
    assert.match(PRIMITIVES_CSS, /var\(--tooltip-pad-y\)/, "вертикальный паддинг не из токена");
  });
});
