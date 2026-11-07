// @ts-check
/**
 * Контроллер звуковой панели (Voice Soundboard Picker) и загрузки кастомных звуков (task-260 / FE-P0-17).
 *
 * Отвечает за:
 * - экспорт констант лимитов звуковой панели (`MIN_SOUNDBOARD_NAME_CHARS = 2`,
 *   `MAX_SOUNDBOARD_NAME_CHARS = 32`, `MAX_SOUNDBOARD_BYTES = 524288`);
 * - чистую функцию `isValidSoundboardName(name)` для валидации имени звука (`2..=32` символов после `trim()`);
 * - чистую функцию `clampSoundboardVolume(volume)` для нормализации громкости в диапазон `0..1`
 *   (возвращает `1` при нечисловом или `NaN` значении);
 * - чистую функцию `filterSoundboardSounds(sounds, query)` для фильтрации доступных звуков
 *   (`available !== false`) по регистронезависимой подстроке `name` с сортировкой по
 *   `name.localeCompare(..., "ru")` по возрастанию и tie-break по `String(soundId)` по возрастанию;
 * - контроллер `createSoundboardPickerController(options)`, рендерящий диалог
 *   `[data-soundboard-picker]` (`role="dialog"`, `aria-label="Звуковая панель"`),
 *   поиск `[data-soundboard-search]`, ползунок громкости `[data-soundboard-volume]`,
 *   кнопки звуков `[data-soundboard-play="<soundId>"]` и форму загрузки кастомного звука
 *   (`[data-soundboard-name-input]`, `[data-soundboard-datauri-input]`,
 *   `[data-soundboard-upload-btn]`, `[data-soundboard-error]`).
 *
 * Оракул: `/home/sanya/re-deep/rest-static.json`
 * (`GUILD_SOUNDBOARD_SOUNDS POST /guilds/{arg}/soundboard-sounds`,
 * `SEND_SOUNDBOARD_SOUND POST /channels/{arg}/send-soundboard-sound`,
 * `web.4d4b2f75fdea5470.js`).
 *
 * @module ui/features/soundboard-picker
 */

/** Минимальная допустимая длина имени звука саундборда после `trim()`. */
export const MIN_SOUNDBOARD_NAME_CHARS = 2;

/** Максимальная допустимая длина имени звука саундборда после `trim()`. */
export const MAX_SOUNDBOARD_NAME_CHARS = 32;

/** Максимальный размер аудиофайла саундборда в байтах (512 КБ). */
export const MAX_SOUNDBOARD_BYTES = 524288;

/** Обязательный префикс Data URI загружаемого аудиофайла. */
const DATA_AUDIO_URI_PREFIX = "data:audio/";

/** Сообщения об ошибках валидации загрузки кастомного звука. */
const INVALID_NAME_ERROR_RU =
  "Недопустимое имя звука: укажите от 2 до 32 символов.";
const INVALID_AUDIO_URI_ERROR_RU =
  "Недопустимый аудиофайл: укажите корректный Data URI с префиксом data:audio/ размером до 512 КБ.";

/**
 * @typedef {object} SoundboardSoundItem
 * @property {string} soundId
 * @property {string} name
 * @property {string} [emojiName]
 * @property {number} [volume]
 * @property {boolean} [available]
 */

/**
 * @typedef {object} SoundboardUploadPayload
 * @property {string} name
 * @property {string} soundDataUri
 * @property {number} volume
 */

/**
 * @typedef {object} SoundboardPickerStatePatch
 * @property {ReadonlyArray<SoundboardSoundItem>} [sounds]
 * @property {number} [volume]
 * @property {string} [query]
 * @property {string} [error]
 */

/**
 * @typedef {object} SoundboardPickerStateSnapshot
 * @property {SoundboardSoundItem[]} sounds
 * @property {number} volume
 * @property {string} query
 * @property {string} error
 */

/**
 * @typedef {object} SoundboardPickerControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<SoundboardSoundItem>} [sounds]
 * @property {number} [volume]
 * @property {string} [query]
 * @property {(soundId: string, volume: number) => void} [onPlaySound]
 * @property {(payload: SoundboardUploadPayload) => void} [onUploadSound]
 */

/**
 * Проверяет допустимость имени звука саундборда:
 * возвращает `true`, если `typeof name === "string"` и длина `name.trim()`
 * находится в диапазоне `2..=32` символов; иначе `false`.
 *
 * @param {unknown} name
 * @returns {boolean}
 */
export function isValidSoundboardName(name) {
  if (typeof name !== "string") {
    return false;
  }
  const trimmed = name.trim();
  return (
    trimmed.length >= MIN_SOUNDBOARD_NAME_CHARS &&
    trimmed.length <= MAX_SOUNDBOARD_NAME_CHARS
  );
}

/**
 * Ограничивает громкость саундборда диапазоном `0..1`:
 * - при нечисловом или `NaN` значении возвращает `1`;
 * - при `< 0` возвращает `0`;
 * - при `> 1` возвращает `1`.
 *
 * @param {unknown} volume
 * @returns {number}
 */
export function clampSoundboardVolume(volume) {
  if (typeof volume !== "number" || Number.isNaN(volume)) {
    return 1;
  }
  if (volume < 0) {
    return 0;
  }
  if (volume > 1) {
    return 1;
  }
  return volume === 0 ? 0 : volume;
}

/**
 * Клонирует элемент звука саундборда без мутации исходного объекта.
 *
 * @param {SoundboardSoundItem} item
 * @returns {SoundboardSoundItem}
 */
function cloneSoundboardSound(item) {
  /** @type {SoundboardSoundItem} */
  const copy = {
    ...item,
    soundId: item.soundId !== undefined && item.soundId !== null ? String(item.soundId) : "",
    name: typeof item.name === "string" ? item.name : String(item.name ?? ""),
  };
  return copy;
}

/**
 * Фильтрует массив звуков саундборда по регистронезависимой подстроке `name`
 * (оставляя только доступные звуки с `available !== false`), сортируя результат по
 * `name.localeCompare(..., "ru")` по возрастанию и tie-break по `String(soundId)`
 * по возрастанию, без мутации входного массива.
 *
 * @param {ReadonlyArray<SoundboardSoundItem>} sounds
 * @param {string} [query]
 * @returns {SoundboardSoundItem[]}
 */
export function filterSoundboardSounds(sounds, query) {
  if (!Array.isArray(sounds)) {
    return [];
  }

  const normalizedQuery =
    typeof query === "string" ? query.trim().toLowerCase() : "";

  /** @type {SoundboardSoundItem[]} */
  const filtered = [];

  for (const item of sounds) {
    if (!item || typeof item !== "object") {
      continue;
    }
    if (item.available === false) {
      continue;
    }
    const itemName = typeof item.name === "string" ? item.name : String(item.name ?? "");
    if (
      normalizedQuery.length > 0 &&
      !itemName.toLowerCase().includes(normalizedQuery)
    ) {
      continue;
    }
    filtered.push(cloneSoundboardSound(item));
  }

  filtered.sort((a, b) => {
    const byName = a.name.localeCompare(b.name, "ru");
    if (byName !== 0) {
      return byName;
    }
    return String(a.soundId).localeCompare(String(b.soundId));
  });

  return filtered;
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
  throw new TypeError("options.container must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента на переданный массив узлов.
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
 * Нормализует входной массив звуков саундборда без мутации исходного массива.
 *
 * @param {ReadonlyArray<SoundboardSoundItem> | null | undefined} rawSounds
 * @returns {SoundboardSoundItem[]}
 */
function normalizeSoundboardSounds(rawSounds) {
  if (!Array.isArray(rawSounds)) {
    return [];
  }
  /** @type {SoundboardSoundItem[]} */
  const result = [];
  for (const item of rawSounds) {
    if (!item || typeof item !== "object") {
      continue;
    }
    result.push(cloneSoundboardSound(item));
  }
  return result;
}

/**
 * Разбирает значение ползунка громкости из DOM-события или элемента.
 *
 * @param {unknown} rawValue
 * @returns {number}
 */
function parseVolumeInputValue(rawValue) {
  if (typeof rawValue === "number") {
    return clampSoundboardVolume(rawValue);
  }
  if (typeof rawValue === "string") {
    const trimmed = rawValue.trim();
    if (trimmed.length === 0) {
      return 1;
    }
    return clampSoundboardVolume(Number(trimmed));
  }
  return 1;
}

/**
 * Создаёт контроллер звуковой панели (Soundboard Picker) и загрузки кастомных звуков внутри `options.container`.
 *
 * @param {SoundboardPickerControllerOptions} options
 */
export function createSoundboardPickerController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {SoundboardSoundItem[]} */
  let sounds = normalizeSoundboardSounds(options.sounds);
  /** @type {number} */
  let currentVolume = clampSoundboardVolume(options.volume);
  /** @type {string} */
  let currentQuery = typeof options.query === "string" ? options.query : "";
  /** @type {string} */
  let errorText = "";
  let destroyed = false;

  const root = doc.createElement("div");
  root.setAttribute("data-soundboard-picker", "");
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-label", "Звуковая панель");

  const controlsEl = doc.createElement("div");
  controlsEl.setAttribute("data-soundboard-controls", "");

  const searchInput = doc.createElement("input");
  searchInput.setAttribute("type", "search");
  searchInput.setAttribute("data-soundboard-search", "");
  searchInput.setAttribute("placeholder", "Поиск звуков");
  searchInput.setAttribute("aria-label", "Поиск звуков");
  searchInput.value = currentQuery;

  const volumeInput = doc.createElement("input");
  volumeInput.setAttribute("type", "range");
  volumeInput.setAttribute("data-soundboard-volume", "");
  volumeInput.setAttribute("min", "0");
  volumeInput.setAttribute("max", "1");
  volumeInput.setAttribute("step", "0.01");
  volumeInput.setAttribute("aria-label", "Громкость звуковой панели");
  volumeInput.value = String(currentVolume);

  replaceElementChildren(controlsEl, [searchInput, volumeInput]);

  const listEl = doc.createElement("div");
  listEl.setAttribute("data-soundboard-list", "");

  const uploadFormEl = doc.createElement("form");
  uploadFormEl.setAttribute("data-soundboard-upload-form", "");

  const nameInput = doc.createElement("input");
  nameInput.setAttribute("type", "text");
  nameInput.setAttribute("data-soundboard-name-input", "");
  nameInput.setAttribute("placeholder", "Название звука");
  nameInput.value = "";

  const dataUriInput = doc.createElement("input");
  dataUriInput.setAttribute("type", "text");
  dataUriInput.setAttribute("data-soundboard-datauri-input", "");
  dataUriInput.setAttribute("placeholder", "data:audio/...");
  dataUriInput.value = "";

  const uploadBtn = doc.createElement("button");
  uploadBtn.setAttribute("type", "button");
  uploadBtn.setAttribute("data-soundboard-upload-btn", "");
  uploadBtn.textContent = "Загрузить звук";

  const errorEl = doc.createElement("div");
  errorEl.setAttribute("data-soundboard-error", "");
  errorEl.setAttribute("role", "alert");
  errorEl.textContent = "";

  replaceElementChildren(uploadFormEl, [
    nameInput,
    dataUriInput,
    uploadBtn,
    errorEl,
  ]);

  /**
   * Перерисовывает список кнопок звуков в соответствии с `sounds` и `currentQuery`.
   */
  function renderSoundList() {
    if (destroyed) {
      return;
    }

    const visibleSounds = filterSoundboardSounds(sounds, currentQuery);
    /** @type {any[]} */
    const buttons = [];

    for (const sound of visibleSounds) {
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("data-soundboard-play", sound.soundId);
      btn.setAttribute("aria-label", sound.name);
      const prefix =
        typeof sound.emojiName === "string" && sound.emojiName.trim().length > 0
          ? `${sound.emojiName.trim()} `
          : "";
      btn.textContent = `${prefix}${sound.name}`;

      btn.addEventListener("click", () => {
        if (destroyed) {
          return;
        }
        if (typeof options.onPlaySound === "function") {
          options.onPlaySound(sound.soundId, currentVolume);
        }
      });

      buttons.push(btn);
    }

    replaceElementChildren(listEl, buttons);
  }

  searchInput.addEventListener("input", (/** @type {any} */ event) => {
    if (destroyed) {
      return;
    }
    const rawQuery = event?.target?.value ?? searchInput.value ?? "";
    currentQuery = String(rawQuery);
    renderSoundList();
  });

  /**
   * @param {any} event
   */
  function handleVolumeChange(event) {
    if (destroyed) {
      return;
    }
    const rawVal = event?.target?.value ?? volumeInput.value;
    currentVolume = parseVolumeInputValue(rawVal);
    volumeInput.value = String(currentVolume);
  }

  volumeInput.addEventListener("input", handleVolumeChange);
  volumeInput.addEventListener("change", handleVolumeChange);

  /**
   * Валидирует поля формы загрузки и вызывает `options.onUploadSound`.
   */
  function handleUploadSound() {
    if (destroyed) {
      return;
    }

    const rawName = typeof nameInput.value === "string" ? nameInput.value : String(nameInput.value ?? "");
    const rawUri = typeof dataUriInput.value === "string" ? dataUriInput.value : String(dataUriInput.value ?? "");
    const trimmedName = rawName.trim();
    const trimmedUri = rawUri.trim();

    if (!isValidSoundboardName(rawName)) {
      errorText = INVALID_NAME_ERROR_RU;
      errorEl.textContent = errorText;
      return;
    }

    if (
      !trimmedUri.startsWith(DATA_AUDIO_URI_PREFIX) ||
      trimmedUri.length > MAX_SOUNDBOARD_BYTES
    ) {
      errorText = INVALID_AUDIO_URI_ERROR_RU;
      errorEl.textContent = errorText;
      return;
    }

    errorText = "";
    errorEl.textContent = "";

    if (typeof options.onUploadSound === "function") {
      options.onUploadSound({
        name: trimmedName,
        soundDataUri: trimmedUri,
        volume: currentVolume,
      });
    }
  }

  uploadBtn.addEventListener("click", () => {
    handleUploadSound();
  });

  uploadFormEl.addEventListener("submit", (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    handleUploadSound();
  });

  /**
   * Возвращает копию текущего состояния контроллера без мутации.
   *
   * @returns {SoundboardPickerStateSnapshot}
   */
  function getState() {
    return {
      sounds: sounds.map(cloneSoundboardSound),
      volume: currentVolume,
      query: currentQuery,
      error: errorText,
    };
  }

  /**
   * Частично обновляет состояние контроллера и синхронизирует DOM.
   *
   * @param {SoundboardPickerStatePatch} [patch]
   * @returns {SoundboardPickerStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.sounds)) {
        sounds = normalizeSoundboardSounds(patch.sounds);
      }
      if (patch.volume !== undefined) {
        currentVolume = clampSoundboardVolume(patch.volume);
        volumeInput.value = String(currentVolume);
      }
      if (typeof patch.query === "string") {
        currentQuery = patch.query;
        searchInput.value = currentQuery;
      }
      if (typeof patch.error === "string") {
        errorText = patch.error;
        errorEl.textContent = errorText;
      }
    }
    renderSoundList();
    return getState();
  }

  /**
   * Размонтирует контроллер и очищает контейнер.
   */
  function destroy() {
    if (destroyed) {
      return;
    }
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else if (typeof root.remove === "function" && root.parentNode === container) {
      root.remove();
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderSoundList();
  replaceElementChildren(root, [controlsEl, listEl, uploadFormEl]);
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
