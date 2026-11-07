// @ts-check
/**
 * Контроллер автодополнения и выбора слэш-команд приложений (task-261 / FE-P0-18).
 *
 * RE-оракулы и контракты BFF:
 * - `/home/sanya/re-deep/rest-static.json:7720-7731` (`Endpoints.INTERACTIONS: "POST /interactions"`);
 * - `/home/sanya/re-deep/rest-static.json:9399-9421` (`APPLICATION_COMMAND_INDEX_CHANNEL`, `APPLICATION_COMMAND_INDEX_GUILD`);
 * - `rust/gui-spike/src/slash_commands.rs` (имя слэш-команды `1..=32` символов `[a-z0-9_-]`, до `25` опций).
 *
 * Отвечает за:
 * - константы `MAX_SLASH_COMMAND_NAME_CHARS = 32` и `MAX_SLASH_COMMAND_OPTIONS = 25`;
 * - чистую валидацию имени команды `isValidSlashCommandName(name)`;
 * - разбор черновика ввода композера `parseSlashDraftInput(input)`;
 * - фильтрацию и ранжирование слэш-команд `filterSlashCommands(commands, query)` с приоритетом префикса `name`,
 *   сортировкой по `name.localeCompare(..., "en")` и tie-break по `String(id)` без мутации входа;
 * - DOM-контроллер `createSlashCommandPickerController(options)` (`[data-slash-picker]`, `role="listbox"`,
 *   `aria-label="Команды приложений"`, элементы `[data-slash-cmd-id="<id>"]` с `role="option"`).
 *
 * @module ui/features/slash-command-picker
 */

/**
 * Максимальная длина имени слэш-команды в символах.
 * @type {32}
 */
export const MAX_SLASH_COMMAND_NAME_CHARS = 32;

/**
 * Максимальное число опций у одной слэш-команды приложения.
 * @type {25}
 */
export const MAX_SLASH_COMMAND_OPTIONS = 25;

/** Регулярное выражение допустимого имени слэш-команды (`[a-z0-9_-]`). */
const SLASH_COMMAND_NAME_PATTERN = /^[a-z0-9_-]+$/;

/**
 * @typedef {object} SlashCommandOptionItem
 * @property {string} [name]
 * @property {string} [description]
 * @property {number} [type]
 * @property {boolean} [required]
 */

/**
 * @typedef {object} SlashCommandItem
 * @property {string} id
 * @property {string} name
 * @property {string} [description]
 * @property {ReadonlyArray<SlashCommandOptionItem>} [options]
 */

/**
 * @typedef {object} ParsedSlashDraft
 * @property {string} commandName
 * @property {string} rawArgs
 */

/**
 * @typedef {object} SlashCommandPickerStatePatch
 * @property {ReadonlyArray<SlashCommandItem>} [commands]
 * @property {string} [query]
 * @property {number} [activeIndex]
 */

/**
 * @typedef {object} SlashCommandPickerStateSnapshot
 * @property {SlashCommandItem[]} commands
 * @property {SlashCommandItem[]} filteredCommands
 * @property {string} query
 * @property {number} activeIndex
 */

/**
 * @typedef {object} SlashCommandPickerControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<SlashCommandItem>} [commands]
 * @property {string} [query]
 * @property {number} [activeIndex]
 * @property {(command: SlashCommandItem) => void} [onSelectCommand]
 */

/**
 * Проверяет корректность имени слэш-команды приложения:
 * возвращает `true`, если `typeof name === "string"` и `name.trim()` имеет длину
 * `1..=32` (`MAX_SLASH_COMMAND_NAME_CHARS`) и соответствует `/^[a-z0-9_-]+$/`; иначе `false`.
 *
 * @param {unknown} name
 * @returns {boolean}
 */
export function isValidSlashCommandName(name) {
  if (typeof name !== "string") {
    return false;
  }
  const trimmed = name.trim();
  if (trimmed.length < 1 || trimmed.length > MAX_SLASH_COMMAND_NAME_CHARS) {
    return false;
  }
  return SLASH_COMMAND_NAME_PATTERN.test(trimmed);
}

/**
 * Разбирает строку черновика композера, начинающуюся с `'/'`:
 * - возвращает `null`, если `input` не строка, не начинается с `'/'` или сразу после `'/'` пробел/пусто;
 * - иначе возвращает `{ commandName, rawArgs }`, где `commandName` приведён к нижнему регистру.
 *
 * @param {unknown} input
 * @returns {ParsedSlashDraft | null}
 */
export function parseSlashDraftInput(input) {
  if (typeof input !== "string" || !input.startsWith("/")) {
    return null;
  }
  const afterSlash = input.slice(1);
  if (afterSlash.length === 0 || /^\s/.test(afterSlash)) {
    return null;
  }
  const match = /^(\S+)(?:\s+([\s\S]*))?$/.exec(afterSlash);
  if (!match || !match[1]) {
    return null;
  }
  return {
    commandName: match[1].toLowerCase(),
    rawArgs: (match[2] ?? "").trim(),
  };
}

/**
 * Создаёт поверхностную копию объекта команды (и массива `options`, если он задан) без мутации входа.
 *
 * @param {SlashCommandItem} cmd
 * @returns {SlashCommandItem}
 */
function cloneCommandItem(cmd) {
  /** @type {SlashCommandItem} */
  const copy = { ...cmd };
  if (Array.isArray(cmd.options)) {
    copy.options = cmd.options.map((opt) =>
      opt && typeof opt === "object" ? { ...opt } : opt,
    );
  }
  return copy;
}

/**
 * Клонирует массив команд без мутации исходных элементов.
 *
 * @param {ReadonlyArray<SlashCommandItem> | null | undefined} commands
 * @returns {SlashCommandItem[]}
 */
function cloneCommandList(commands) {
  if (!Array.isArray(commands)) {
    return [];
  }
  /** @type {SlashCommandItem[]} */
  const out = [];
  for (const item of commands) {
    if (!item || typeof item !== "object") continue;
    out.push(cloneCommandItem(item));
  }
  return out;
}

/**
 * Фильтрует массив слэш-команд `{ id, name, description, options?: Array }` по префиксу
 * или регистронезависимой подстроке `name`/`description` (с приоритетом префикса `name`),
 * сортируя по `name.localeCompare(..., "en")` по возрастанию и tie-break по `String(id)`
 * по возрастанию, без мутации входного массива.
 *
 * @param {ReadonlyArray<SlashCommandItem> | null | undefined} commands
 * @param {string} [query]
 * @returns {SlashCommandItem[]}
 */
export function filterSlashCommands(commands, query) {
  if (!Array.isArray(commands)) {
    return [];
  }
  const rawQuery = typeof query === "string" ? query.trim().toLowerCase() : "";
  const needle = rawQuery.startsWith("/") ? rawQuery.slice(1).trim() : rawQuery;

  /** @type {SlashCommandItem[]} */
  const matched = [];
  for (const item of commands) {
    if (!item || typeof item !== "object") continue;
    const nameStr = String(item.name ?? "").toLowerCase();
    const descStr = String(item.description ?? "").toLowerCase();

    if (needle.length > 0 && !nameStr.includes(needle) && !descStr.includes(needle)) {
      continue;
    }
    matched.push(cloneCommandItem(item));
  }

  matched.sort((a, b) => {
    const nameA = String(a.name ?? "");
    const nameB = String(b.name ?? "");

    if (needle.length > 0) {
      const aPrefix = nameA.toLowerCase().startsWith(needle) ? 1 : 0;
      const bPrefix = nameB.toLowerCase().startsWith(needle) ? 1 : 0;
      if (aPrefix !== bPrefix) {
        return bPrefix - aPrefix;
      }
    }

    const byName = nameA.localeCompare(nameB, "en");
    if (byName !== 0) {
      return byName;
    }

    const idA = String(a.id ?? "");
    const idB = String(b.id ?? "");
    if (idA < idB) return -1;
    if (idA > idB) return 1;
    return 0;
  });

  return matched;
}

/**
 * Зажимает индекс активного элемента в диапазон `0..count-1` (или `0`, если список пуст).
 *
 * @param {unknown} rawIndex
 * @param {number} count
 * @returns {number}
 */
function clampActiveIndex(rawIndex, count) {
  if (count <= 0) {
    return 0;
  }
  const idx =
    typeof rawIndex === "number" && Number.isFinite(rawIndex) ? Math.trunc(rawIndex) : 0;
  if (idx < 0) return 0;
  if (idx >= count) return count - 1;
  return idx;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (
    containerEl &&
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("Container element must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы родительского элемента без использования `innerHTML`.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === "function") {
    parentEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(parentEl.children)) {
    parentEl.children.length = 0;
  }
  for (const node of nodes) {
    parentEl.appendChild(node);
  }
}

/**
 * Создаёт контроллер пикера автодополнения и вызова слэш-команд приложений.
 *
 * @param {SlashCommandPickerControllerOptions} options
 */
export function createSlashCommandPickerController(options) {
  if (!options || typeof options !== "object" || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {SlashCommandItem[]} */
  let commands = cloneCommandList(options.commands);
  /** @type {string} */
  let query = typeof options.query === "string" ? options.query : "";
  /** @type {SlashCommandItem[]} */
  let filteredCommands = filterSlashCommands(commands, query);
  /** @type {number} */
  let activeIndex = clampActiveIndex(options.activeIndex, filteredCommands.length);
  let destroyed = false;

  const root = doc.createElement("div");
  root.setAttribute("data-slash-picker", "");
  root.setAttribute("role", "listbox");
  root.setAttribute("aria-label", "Команды приложений");

  /**
   * Перерисовывает список опций слэш-команд внутри корневого listbox.
   */
  function renderDom() {
    if (destroyed) return;

    filteredCommands = filterSlashCommands(commands, query);
    activeIndex = clampActiveIndex(activeIndex, filteredCommands.length);

    /** @type {any[]} */
    const optionNodes = [];

    filteredCommands.forEach((cmd, index) => {
      const cmdId = String(cmd.id ?? "");
      const cleanName = String(cmd.name ?? "").replace(/^\/+/, "");
      const descriptionText = String(cmd.description ?? "");

      const optionEl = doc.createElement("div");
      optionEl.setAttribute("role", "option");
      optionEl.setAttribute("data-slash-cmd-id", cmdId);
      optionEl.setAttribute("aria-selected", index === activeIndex ? "true" : "false");

      const nameEl = doc.createElement("span");
      nameEl.setAttribute("data-slash-cmd-name", "");
      nameEl.textContent = `/${cleanName}`;
      optionEl.appendChild(nameEl);

      const descEl = doc.createElement("span");
      descEl.setAttribute("data-slash-cmd-description", "");
      descEl.textContent = descriptionText.length > 0 ? ` — ${descriptionText}` : "";
      optionEl.appendChild(descEl);

      optionEl.addEventListener("click", () => {
        if (destroyed) return;
        activeIndex = index;
        renderDom();
        if (typeof options.onSelectCommand === "function") {
          options.onSelectCommand(cloneCommandItem(cmd));
        }
      });

      optionNodes.push(optionEl);
    });

    replaceElementChildren(root, optionNodes);
  }

  /**
   * Возвращает копию текущего состояния контроллера без возможности внешней мутации.
   *
   * @returns {SlashCommandPickerStateSnapshot}
   */
  function getState() {
    return {
      commands: cloneCommandList(commands),
      filteredCommands: cloneCommandList(filteredCommands),
      query,
      activeIndex,
    };
  }

  /**
   * Частично обновляет состояние контроллера (`commands`, `query`, `activeIndex`) и перерисовывает DOM.
   *
   * @param {SlashCommandPickerStatePatch} [patch]
   * @returns {SlashCommandPickerStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      let queryChanged = false;
      if (Array.isArray(patch.commands)) {
        commands = cloneCommandList(patch.commands);
      }
      if ("query" in patch && patch.query !== undefined) {
        const nextQuery = typeof patch.query === "string" ? patch.query : String(patch.query ?? "");
        if (nextQuery !== query) {
          query = nextQuery;
          queryChanged = true;
        }
      }

      filteredCommands = filterSlashCommands(commands, query);

      if (
        "activeIndex" in patch &&
        typeof patch.activeIndex === "number" &&
        Number.isFinite(patch.activeIndex)
      ) {
        activeIndex = clampActiveIndex(patch.activeIndex, filteredCommands.length);
      } else if (queryChanged) {
        activeIndex = 0;
      } else {
        activeIndex = clampActiveIndex(activeIndex, filteredCommands.length);
      }
    }
    renderDom();
    return getState();
  }

  /**
   * Циклически сдвигает `activeIndex` по отфильтрованному списку команд на `delta`.
   *
   * @param {number} delta
   * @returns {number}
   */
  function moveSelection(delta) {
    if (destroyed) return activeIndex;
    filteredCommands = filterSlashCommands(commands, query);
    const count = filteredCommands.length;
    if (count === 0) {
      activeIndex = 0;
      renderDom();
      return activeIndex;
    }
    const step = typeof delta === "number" && Number.isFinite(delta) ? Math.trunc(delta) : 0;
    activeIndex = (((activeIndex + step) % count) + count) % count;
    renderDom();
    return activeIndex;
  }

  /**
   * Вызывает `onSelectCommand` для текущей активной команды из отфильтрованного списка.
   *
   * @returns {SlashCommandItem | null}
   */
  function selectActive() {
    if (destroyed) return null;
    filteredCommands = filterSlashCommands(commands, query);
    if (filteredCommands.length === 0) {
      return null;
    }
    const idx = clampActiveIndex(activeIndex, filteredCommands.length);
    const selected = filteredCommands[idx];
    if (!selected) {
      return null;
    }
    const cloned = cloneCommandItem(selected);
    if (typeof options.onSelectCommand === "function") {
      options.onSelectCommand(cloned);
    }
    return cloned;
  }

  /**
   * Размонтирует пикер из контейнера.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderDom();
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    moveSelection,
    selectActive,
    destroy,
  };
}
