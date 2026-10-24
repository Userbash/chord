// @ts-check
/**
 * Эмодзи-пикер: 10 вкладок-глифов и **плоский** `#emoji-grid` (role=listbox,
 * ячейки role=option) — D-07, без `.emoji-row[role=row]`.
 *
 * Клавиатура (OBS-01): roving tabindex, стрелки с шагом сетки 9, Home/End,
 * Enter/Space — выбор; `aria-selected` ровно у одной ячейки и одной вкладки.
 * `aria-live` — только у предпросмотра (не у FPS).
 */
import { byId } from "./dom.js";
import { UIError } from "./errors.js";

/** Колонок в сетке (--emoji-cols). */
export const EMOJI_COLS = 9;

/**
 * @typedef {{ glyph: string, name: string }} Emoji
 * @typedef {{ name: string, glyph: string, items: Emoji[] }} EmojiCategory
 */

/** @type {EmojiCategory[]} */
export const EMOJI_CATEGORIES = [
  {
    name: "Часто используемые",
    glyph: "🕘",
    items: [
      { glyph: "🔥", name: "огонь" },
      { glyph: "😀", name: "улыбка" },
      { glyph: "😂", name: "смех до слёз" },
      { glyph: "❤️", name: "красное сердце" },
      { glyph: "👍", name: "палец вверх" },
      { glyph: "🎉", name: "праздник" },
      { glyph: "😍", name: "влюблённость" },
      { glyph: "😭", name: "громкий плач" },
      { glyph: "🙏", name: "спасибо" },
      { glyph: "💀", name: "череп" },
      { glyph: "✅", name: "галочка" },
      { glyph: "👀", name: "глаза" },
    ],
  },
  {
    name: "Смайлы",
    glyph: "😀",
    items: [
      { glyph: "😀", name: "улыбка" },
      { glyph: "😃", name: "улыбка с большими глазами" },
      { glyph: "😄", name: "смеющиеся глаза" },
      { glyph: "😁", name: "ухмылка" },
      { glyph: "😆", name: "хохот" },
      { glyph: "😅", name: "пот на лбу" },
      { glyph: "🤣", name: "катаюсь от смеха" },
      { glyph: "😊", name: "милая улыбка" },
      { glyph: "🙂", name: "лёгкая улыбка" },
      { glyph: "🙃", name: "перевёрнутая улыбка" },
      { glyph: "😉", name: "подмигивание" },
      { glyph: "😌", name: "спокойствие" },
      { glyph: "😍", name: "влюблённость" },
      { glyph: "🥰", name: "обожание" },
      { glyph: "😘", name: "воздушный поцелуй" },
      { glyph: "😋", name: "вкусно" },
      { glyph: "😛", name: "показывает язык" },
      { glyph: "🤪", name: "безумное лицо" },
      { glyph: "🤑", name: "деньги в глазах" },
      { glyph: "🤔", name: "размышление" },
    ],
  },
  {
    name: "Люди и жесты",
    glyph: "🧑",
    items: [
      { glyph: "👋", name: "привет" },
      { glyph: "🤚", name: "поднятая ладонь" },
      { glyph: "✋", name: "стоп" },
      { glyph: "🖖", name: "вулканский салют" },
      { glyph: "👌", name: "ок" },
      { glyph: "🤏", name: "щипотка" },
      { glyph: "✌️", name: "победа" },
      { glyph: "🤞", name: "скрещённые пальцы" },
      { glyph: "🤟", name: "люблю тебя" },
      { glyph: "👈", name: "указывает налево" },
      { glyph: "👉", name: "указывает направо" },
      { glyph: "👆", name: "указывает вверх" },
      { glyph: "👇", name: "указывает вниз" },
      { glyph: "👍", name: "палец вверх" },
      { glyph: "👎", name: "палец вниз" },
      { glyph: "✊", name: "поднятый кулак" },
      { glyph: "👏", name: "аплодисменты" },
      { glyph: "🙌", name: "поднятые руки" },
      { glyph: "🙏", name: "спасибо" },
      { glyph: "💪", name: "сила" },
    ],
  },
  {
    name: "Еда и напитки",
    glyph: "🍔",
    items: [
      { glyph: "🍎", name: "яблоко" },
      { glyph: "🍐", name: "груша" },
      { glyph: "🍊", name: "апельсин" },
      { glyph: "🍋", name: "лимон" },
      { glyph: "🍌", name: "банан" },
      { glyph: "🍉", name: "арбуз" },
      { glyph: "🍇", name: "виноград" },
      { glyph: "🍓", name: "клубника" },
      { glyph: "🍒", name: "вишня" },
      { glyph: "🍑", name: "персик" },
      { glyph: "🥝", name: "киви" },
      { glyph: "🍅", name: "помидор" },
      { glyph: "🥑", name: "авокадо" },
      { glyph: "🥦", name: "брокколи" },
      { glyph: "🍞", name: "хлеб" },
      { glyph: "🧀", name: "сыр" },
      { glyph: "🍕", name: "пицца" },
      { glyph: "🍔", name: "бургер" },
      { glyph: "🍟", name: "картошка фри" },
      { glyph: "☕", name: "кофе" },
    ],
  },
  {
    name: "Природа",
    glyph: "🌿",
    items: [
      { glyph: "🌿", name: "ветка" },
      { glyph: "🌱", name: "росток" },
      { glyph: "🌳", name: "дерево" },
      { glyph: "🌲", name: "ель" },
      { glyph: "🌵", name: "кактус" },
      { glyph: "🌴", name: "пальма" },
      { glyph: "🌸", name: "цветок" },
      { glyph: "🌺", name: "гибискус" },
      { glyph: "🌻", name: "подсолнух" },
      { glyph: "🌹", name: "роза" },
      { glyph: "🍀", name: "клевер" },
      { glyph: "🍁", name: "кленовый лист" },
      { glyph: "🌊", name: "волна" },
      { glyph: "🌙", name: "луна" },
      { glyph: "⭐", name: "звезда" },
      { glyph: "☀️", name: "солнце" },
      { glyph: "🌈", name: "радуга" },
      { glyph: "❄️", name: "снежинка" },
      { glyph: "🌧️", name: "дождь" },
      { glyph: "⚡", name: "молния" },
    ],
  },
  {
    name: "Транспорт",
    glyph: "🚗",
    items: [
      { glyph: "🚗", name: "машина" },
      { glyph: "🚕", name: "такси" },
      { glyph: "🚌", name: "автобус" },
      { glyph: "🚎", name: "троллейбус" },
      { glyph: "🏎️", name: "гоночный болид" },
      { glyph: "🚓", name: "полиция" },
      { glyph: "🚑", name: "скорая" },
      { glyph: "🚒", name: "пожарная" },
      { glyph: "🚚", name: "грузовик" },
      { glyph: "🚜", name: "трактор" },
      { glyph: "🛴", name: "самокат" },
      { glyph: "🚲", name: "велосипед" },
      { glyph: "🛵", name: "мопед" },
      { glyph: "🏍️", name: "мотоцикл" },
      { glyph: "✈️", name: "самолёт" },
      { glyph: "🚀", name: "ракета" },
      { glyph: "🚁", name: "вертолёт" },
      { glyph: "⛵", name: "парусник" },
      { glyph: "🚢", name: "корабль" },
      { glyph: "🚂", name: "поезд" },
    ],
  },
  {
    name: "Предметы",
    glyph: "💡",
    items: [
      { glyph: "💡", name: "лампа" },
      { glyph: "🔦", name: "фонарь" },
      { glyph: "🕯️", name: "свеча" },
      { glyph: "💻", name: "ноутбук" },
      { glyph: "🖥️", name: "монитор" },
      { glyph: "⌨️", name: "клавиатура" },
      { glyph: "🖱️", name: "мышь" },
      { glyph: "📱", name: "телефон" },
      { glyph: "☎️", name: "телефонная трубка" },
      { glyph: "📷", name: "камера" },
      { glyph: "🎧", name: "наушники" },
      { glyph: "🎤", name: "микрофон" },
      { glyph: "📚", name: "книги" },
      { glyph: "✏️", name: "карандаш" },
      { glyph: "📝", name: "заметка" },
      { glyph: "📌", name: "канцелярская кнопка" },
      { glyph: "🔑", name: "ключ" },
      { glyph: "🔒", name: "замок" },
      { glyph: "🧰", name: "ящик с инструментами" },
      { glyph: "🎁", name: "подарок" },
    ],
  },
  {
    name: "Символы",
    glyph: "❤️",
    items: [
      { glyph: "❤️", name: "красное сердце" },
      { glyph: "🧡", name: "оранжевое сердце" },
      { glyph: "💛", name: "жёлтое сердце" },
      { glyph: "💚", name: "зелёное сердце" },
      { glyph: "💙", name: "синее сердце" },
      { glyph: "💜", name: "фиолетовое сердце" },
      { glyph: "🖤", name: "чёрное сердце" },
      { glyph: "🤍", name: "белое сердце" },
      { glyph: "✅", name: "галочка" },
      { glyph: "❌", name: "крестик" },
      { glyph: "❗", name: "восклицательный знак" },
      { glyph: "❓", name: "вопросительный знак" },
      { glyph: "♻️", name: "переработка" },
      { glyph: "⚠️", name: "внимание" },
      { glyph: "⏳", name: "песочные часы" },
      { glyph: "🔔", name: "звонок" },
      { glyph: "🔕", name: "без звука" },
      { glyph: "💤", name: "сон" },
      { glyph: "💯", name: "сто очков" },
      { glyph: "🔥", name: "огонь" },
    ],
  },
  {
    name: "Флаги",
    glyph: "🏳️",
    items: [
      { glyph: "🏳️", name: "белый флаг" },
      { glyph: "🏴", name: "чёрный флаг" },
      { glyph: "🏁", name: "клетчатый флаг" },
      { glyph: "🚩", name: "красный флаг" },
      { glyph: "🏳️‍🌈", name: "радужный флаг" },
      { glyph: "🇷🇺", name: "Россия" },
      { glyph: "🇺🇸", name: "США" },
      { glyph: "🇩🇪", name: "Германия" },
      { glyph: "🇫🇷", name: "Франция" },
      { glyph: "🇯🇵", name: "Япония" },
      { glyph: "🇬🇧", name: "Британия" },
      { glyph: "🇨🇦", name: "Канада" },
      { glyph: "🇧🇷", name: "Бразилия" },
      { glyph: "🇮🇹", name: "Италия" },
      { glyph: "🇪🇸", name: "Испания" },
      { glyph: "🇰🇷", name: "Корея" },
      { glyph: "🇨🇳", name: "Китай" },
      { glyph: "🇺🇦", name: "Украина" },
      { glyph: "🇫🇮", name: "Финляндия" },
      { glyph: "🇸🇪", name: "Швеция" },
    ],
  },
  {
    name: "Прочее",
    glyph: "🎮",
    items: [
      { glyph: "🎮", name: "геймпад" },
      { glyph: "🕹️", name: "джойстик" },
      { glyph: "🎲", name: "игральная кость" },
      { glyph: "🧩", name: "пазл" },
      { glyph: "🎯", name: "цель" },
      { glyph: "🎨", name: "палитра" },
      { glyph: "🎬", name: "кино" },
      { glyph: "🎵", name: "нота" },
      { glyph: "🎸", name: "гитара" },
      { glyph: "🎹", name: "пианино" },
      { glyph: "🥁", name: "барабан" },
      { glyph: "🏆", name: "кубок" },
      { glyph: "🏅", name: "медаль" },
      { glyph: "⚽", name: "футбольный мяч" },
      { glyph: "🏀", name: "баскетбольный мяч" },
      { glyph: "🎾", name: "теннисный мяч" },
      { glyph: "🎳", name: "боулинг" },
      { glyph: "🧠", name: "мозг" },
      { glyph: "🤖", name: "робот" },
      { glyph: "👾", name: "пришелец" },
    ],
  },
];

/**
 * EMO-11: нормализация поискового запроса — срезать ведущее/хвостовое `:`,
 * привести к нижнему регистру, убрать пробелы и `_` (как в дампе `Y`).
 * @param {string} raw
 * @returns {string}
 */
export function normalizeEmojiQuery(raw) {
  return String(raw)
    .trim()
    .replace(/^:+/, "")
    .replace(/:+$/, "")
    .replace(/[\s_]+/g, "")
    .toLowerCase();
}

/**
 * EMO-16: глобальный поиск по всем категориям (unicode + custom), не по активной.
 * Дедупликация по глифу+имени (🔥 есть и в «Часто используемых», и в «Символах»),
 * порядок — по имени; данные спайка не содержат locked/frecency, поэтому
 * оставшиеся ключи EMO-12 неприменимы и не имитируются.
 * @param {string} raw
 * @returns {Emoji[]}
 */
export function searchAllEmojis(raw) {
  const needle = normalizeEmojiQuery(raw);
  if (needle === "") return [];
  /** @type {Emoji[]} */
  const found = [];
  const seen = new Set();
  for (const category of EMOJI_CATEGORIES) {
    for (const item of category.items) {
      const key = `${item.glyph}\u0000${item.name}`;
      if (seen.has(key)) continue;
      const haystack = `${normalizeEmojiQuery(item.name)}${normalizeEmojiQuery(item.glyph)}`;
      if (!haystack.includes(needle)) continue;
      seen.add(key);
      found.push(item);
    }
  }
  return found.sort((left, right) => left.name.localeCompare(right.name, "ru"));
}

/**
 * @typedef {{
 *   categories: EmojiCategory[],
 *   getActiveCategory: () => number,
 *   activateTab: (index: number) => void,
 *   render: () => void,
 * }} EmojiConsole
 */

/**
 * @param {{
 *   tabs: HTMLElement,
 *   grid: HTMLElement,
 *   search: HTMLInputElement | null,
 *   previewGlyph?: HTMLElement | null,
 *   previewName?: HTMLElement | null,
 *   onSelect?: (glyph: string, item: Emoji) => void,
 * }} options
 * @returns {EmojiConsole}
 */
export function createEmojiConsole(options) {
  const { tabs, grid, search } = options;
  const previewGlyph = options.previewGlyph ?? null;
  const previewName = options.previewName ?? null;
  const onSelect = options.onSelect ?? (() => {});
  /** @type {HTMLElement[]} */
  const tabButtons = [];
  let activeCategory = 0;
  let activeOption = 0;
  /** @type {Emoji[]} */
  let visible = [];

  /** @param {number} index */
  function updateTabState(index) {
    tabButtons.forEach((tab, tabIndex) => {
      const active = tabIndex === index;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", active ? "true" : "false");
      tab.setAttribute("tabindex", active ? "0" : "-1");
    });
  }

  /**
   * @param {number} index
   * @param {boolean} [moveFocus] true только для клавиатуры: клик не крадёт фокус.
   */
  function setActiveOption(index, moveFocus = false) {
    if (visible.length === 0) {
      activeOption = 0;
      if (previewGlyph) previewGlyph.textContent = "🙂";
      if (previewName) previewName.textContent = "Ничего не найдено";
      return;
    }
    activeOption = Math.max(0, Math.min(visible.length - 1, index));
    const cells = /** @type {HTMLElement[]} */ ([...grid.querySelectorAll(".emoji-cell")]);
    cells.forEach((cell, cellIndex) => {
      const active = cellIndex === activeOption;
      cell.setAttribute("aria-selected", active ? "true" : "false");
      cell.setAttribute("tabindex", active ? "0" : "-1");
    });
    const chosen = visible[activeOption];
    if (chosen && previewGlyph) previewGlyph.textContent = chosen.glyph;
    if (chosen && previewName) previewName.textContent = chosen.name;
    const focused = cells[activeOption];
    // В DOM-шиме у узлов нет focus() — в браузере он есть; best-effort без throw.
    if (moveFocus && focused && typeof focused.focus === "function") focused.focus();
  }

  function render() {
    const query = search ? normalizeEmojiQuery(search.value) : "";
    const category = EMOJI_CATEGORIES[activeCategory] ?? EMOJI_CATEGORIES[0];
    if (!category) {
      throw new UIError("ELEMENT_MISSING", "нет ни одной категории эмодзи");
    }
    // EMO-16: запрос ищет по всему набору; категория — только листание.
    visible = query === "" ? [...category.items] : searchAllEmojis(search?.value ?? "");
    activeOption = 0;
    const cells = visible.map((item) => {
      const cell = document.createElement("button");
      cell.className = "emoji-cell";
      cell.type = "button";
      cell.setAttribute("role", "option");
      cell.setAttribute("aria-label", item.name);
      cell.title = item.glyph;
      cell.textContent = item.glyph;
      cell.addEventListener("click", () => {
        const index = cells.indexOf(cell);
        setActiveOption(index);
        selectOption(index);
      });
      return cell;
    });
    grid.replaceChildren(...cells);
    updateTabState(activeCategory);
    setActiveOption(0);
  }

  /**
   * Выбор эмодзи: предпросмотр уже обновлён, отдельного composer-ввода в спайке нет
   * (Phase 3) — фиксируем выбор в data-атрибуте для диагностики.
   * @param {number} index
   */
  function selectOption(index) {
    const chosen = visible[index];
    if (!chosen) return;
    grid.setAttribute("data-selected", chosen.glyph);
    if (previewName) previewName.textContent = chosen.name;
    if (previewGlyph) previewGlyph.textContent = chosen.glyph;
    // Композер: вставка в позицию курсора (EMO-16/композер §4), не «в конец».
    onSelect(chosen.glyph, chosen);
  }

  /**
   * @param {number} index
   */
  function activateTab(index) {
    activeCategory = Math.max(0, Math.min(EMOJI_CATEGORIES.length - 1, index));
    render();
  }

  EMOJI_CATEGORIES.forEach((category, index) => {
    const tab = document.createElement("button");
    tab.className = "tab";
    tab.type = "button";
    tab.id = `emoji-tab-${index}`;
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-controls", "emoji-grid");
    tab.setAttribute("aria-label", category.name);
    tab.title = category.name;
    tab.textContent = category.glyph;
    tab.addEventListener("click", () => activateTab(index));
    tabButtons.push(tab);
    tabs.appendChild(tab);
  });

  // Клавиатура вкладок: вертикальный tablist (aria-orientation=vertical).
  tabs.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    let next = activeCategory;
    if (key === "ArrowDown" || key === "ArrowRight") next += 1;
    else if (key === "ArrowUp" || key === "ArrowLeft") next -= 1;
    else if (key === "Home") next = 0;
    else if (key === "End") next = EMOJI_CATEGORIES.length - 1;
    else return;
    event.preventDefault();
    activateTab(next);
    const button = tabButtons[activeCategory];
    if (button && typeof button.focus === "function") button.focus();
  });

  // Клавиатура сетки: roving tabindex по плоскому listbox.
  grid.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (visible.length === 0) return;
    /** @type {number | null} */
    let next = null;
    if (key === "ArrowRight") next = activeOption + 1;
    else if (key === "ArrowLeft") next = activeOption - 1;
    else if (key === "ArrowDown") next = activeOption + EMOJI_COLS;
    else if (key === "ArrowUp") next = activeOption - EMOJI_COLS;
    else if (key === "Home") next = 0;
    else if (key === "End") next = visible.length - 1;
    else if (key === "Enter" || key === " ") {
      event.preventDefault();
      selectOption(activeOption);
      return;
    }
    if (next === null) return;
    event.preventDefault();
    setActiveOption(next, true);
  });

  if (search) search.addEventListener("input", () => render());
  render();

  return {
    categories: EMOJI_CATEGORIES,
    getActiveCategory: () => activeCategory,
    activateTab,
    render,
  };
}

/**
 * @param {{ onSelect?: (glyph: string, item: Emoji) => void }} [options]
 * @returns {EmojiConsole}
 */
export function initEmojiConsole(options = {}) {
  const tabs = byId("emoji-tabs");
  const grid = byId("emoji-grid");
  if (!tabs || !grid) throw new UIError("ELEMENT_MISSING", "в DOM нет #emoji-tabs/#emoji-grid");
  const search = /** @type {HTMLInputElement | null} */ (byId("emoji-search"));
  const preview = document.querySelector(".emoji-preview");
  return createEmojiConsole({
    tabs,
    grid,
    search,
    previewGlyph: preview ? /** @type {HTMLElement | null} */ (preview.querySelector(".emoji-preview-glyph")) : null,
    previewName: preview ? /** @type {HTMLElement | null} */ (preview.querySelector(".emoji-preview-name")) : null,
    onSelect: options.onSelect,
  });
}
