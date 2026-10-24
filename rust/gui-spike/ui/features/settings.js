// @ts-check
/**
 * Настройки (SPEC-P3-02 §10, D-17): 18 разделов навигации по порядку RE S-01,
 * контент только у трёх — «Голос и видео» (38 тумблеров = 12 + 22 + 4),
 * «Внешний вид» (плотность сообщений, S-14) и «Уведомления» (честная заглушка,
 * SET-15). Остальные 15 разделов — disabled, потому что их состава в дампе нет.
 *
 * Контракт тумблера не меняется: `.toggle` + `.toggle-label` +
 * `input.toggle-input[role=switch]` + `.toggle-track > .toggle-thumb`,
 * `aria-checked` синхронизируется на `click` и `change`.
 */
import { byId } from "./dom.js";
import { UIError } from "./errors.js";

/** SET-01 (RE S-01): 18 разделов, порядок зафиксирован дампом. */
export const SETTINGS_SECTIONS = /** @type {const} */ ([
  "Учётная запись",
  "Данные и конфиденциальность",
  "Разрешения для сообщений",
  "Уведомления",
  "Nitro",
  "Буст сервера",
  "Подписки",
  "Склад подарков",
  "Выставление счетов",
  "Голос и видео",
  "Внешний вид",
  "Специальные возможности",
  "Система",
  "Язык и время",
  "Зарегистрированные игры",
  "Конфиденциальность активности",
  "Подключённые приложения",
  "Разработчик",
]);

/** D-17: контент реализован только у трёх разделов; остальные disabled. */
export const SETTINGS_ENABLED = ["Уведомления", "Голос и видео", "Внешний вид"];

/**
 * ST-01 (RE `ui8/23-settings-categories.json`): профиль навигации — 2 строки
 * оракула (`ShadowByte` + «Редактировать профили») рядом с аватаром-монограммой.
 * В дампе нет handle, поэтому его здесь тоже нет.
 */
export const SETTINGS_PROFILE = /** @type {const} */ ({
  name: "ShadowByte",
  monogram: "SB",
  action: "Редактировать профили",
});

/**
 * ST-01: 4 подпункта «Учётной записи» — единственные, развёрнутые в дампе.
 * Состав подпунктов остальных 17 разделов в захвате свёрнут (`[A]`), поэтому
 * он не воспроизводится.
 */
export const SETTINGS_ACCOUNT_ITEMS = [
  "Информация об учётной записи",
  "Пароль и безопасность",
  "Статус учётной записи",
  "Семейный центр",
];

/** ST-01: причина disabled-раздела (title + aria-describedby). */
export const SETTINGS_SECTION_REASON = "Раздел не реализован: состава панели в дампе нет (D-17)";
/** ST-01: причина disabled-подпункта: имя известно, панель — нет. */
export const SETTINGS_ITEM_REASON = "Подраздел не реализован: контента панели в дампе нет (ST-01)";
/** ST-01: редактирования профиля в спайке нет. */
export const SETTINGS_PROFILE_REASON = "Редактирование профиля не реализовано в спайке (ST-01)";
/** ST-01: «Выйти» без IPC-моста — честно disabled, а не мёртвая кнопка. */
export const SETTINGS_LOGOUT_REASON = "Выход недоступен: нет IPC-моста (ST-01)";


/** SET-04: 10 блоков «Голоса» в порядке дампа. */
export const SETTINGS_BLOCKS = [
  "Голос",
  "Профиль ввода",
  "Чувствительность ввода",
  "Шумоподавление",
  "Эхо/рация/АРУ",
  "Камера",
  "Стриминг",
  "Звуки",
  "Связанные настройки",
  "Расширенные",
];

/**
 * SET-06/07/08: 38 тумблеров. `checked` — стартовое состояние из дампа (S-06/S-08),
 * а не «все включены»: выключены QoS, рация, оверлей стрима и запись для диагностики.
 *
 * `id` — стабильный ASCII-ключ файла настроек (B8b): русская подпись может
 * измениться при i18n (C4), поэтому в `data-setting-id` и `settings_save`
 * уходит только он. id не меняются и не переиспользуются.
 * @typedef {{ block: string, label: string, checked: boolean, id: string, sound?: boolean }} ToggleSpec
 * @type {ToggleSpec[]}
 */
const TOGGLE_SPECS = [
  // Голос (4)
  { block: "Голос", label: "Предлагать устройства", checked: true, id: "voice.suggest_devices" },
  { block: "Голос", label: "Предупреждать при отсутствии звука", checked: true, id: "voice.warn_no_sound" },
  { block: "Голос", label: "Предупреждать при смене канала", checked: true, id: "voice.warn_channel_change" },
  { block: "Голос", label: "Приоритет QoS", checked: false, id: "voice.qos_priority" },
  // Чувствительность ввода (1)
  { block: "Чувствительность ввода", label: "Автоматически определять чувствительность ввода", checked: true, id: "input.auto_sensitivity" },
  // Шумоподавление (1)
  { block: "Шумоподавление", label: "Эхоподавление", checked: true, id: "noise.echo_cancel" },
  // Эхо/рация/АРУ (3)
  { block: "Эхо/рация/АРУ", label: "Режим рации", checked: false, id: "voice.ptt_mode" },
  { block: "Эхо/рация/АРУ", label: "Автоматическая регулировка усиления", checked: true, id: "voice.agc" },
  { block: "Эхо/рация/АРУ", label: "Расширенная активация", checked: true, id: "voice.advanced_activation" },
  // Камера (1)
  { block: "Камера", label: "Предпросмотр видео", checked: true, id: "video.preview" },
  // Стриминг (2)
  { block: "Стриминг", label: "Предпросмотр стрима", checked: true, id: "stream.preview" },
  { block: "Стриминг", label: "Приглушать стрим", checked: true, id: "stream.mute" },
  // Звуки (22): 4 видимых + 18 под «Показать ещё 18 звуков».
  { block: "Звуки", label: "Откл. звук", checked: true, id: "sounds.deafen", sound: true },
  { block: "Звуки", label: "Включить звук в динамиках", checked: true, id: "sounds.undeafen", sound: true },
  { block: "Звуки", label: "Заглушить", checked: true, id: "sounds.mute", sound: true },
  { block: "Звуки", label: "Вкл. микрофон", checked: true, id: "sounds.unmute", sound: true },
  { block: "Звуки", label: "Камера включена", checked: true, id: "sounds.camera_on", sound: true },
  { block: "Звуки", label: "Камера выключена", checked: true, id: "sounds.camera_off", sound: true },
  { block: "Звуки", label: "Голосовая связь отключена", checked: true, id: "sounds.voice_disconnected", sound: true },
  { block: "Звуки", label: "Активация рации", checked: true, id: "sounds.ptt_activation", sound: true },
  { block: "Звуки", label: "Деактивация рации", checked: true, id: "sounds.ptt_deactivation", sound: true },
  { block: "Звуки", label: "Пользователь присоединился", checked: true, id: "sounds.user_join", sound: true },
  { block: "Звуки", label: "Пользователь вышел", checked: true, id: "sounds.user_leave", sound: true },
  { block: "Звуки", label: "Пользователь перемещён", checked: true, id: "sounds.user_moved", sound: true },
  { block: "Звуки", label: "Исходящий звонок", checked: true, id: "sounds.outgoing_call", sound: true },
  { block: "Звуки", label: "Стрим начался", checked: true, id: "sounds.stream_started", sound: true },
  { block: "Звуки", label: "Стрим остановлен", checked: true, id: "sounds.stream_stopped", sound: true },
  { block: "Звуки", label: "Зритель присоединился", checked: true, id: "sounds.viewer_join", sound: true },
  { block: "Звуки", label: "Зритель вышел", checked: true, id: "sounds.viewer_leave", sound: true },
  { block: "Звуки", label: "Начало активности", checked: true, id: "sounds.activity_start", sound: true },
  { block: "Звуки", label: "Завершение активности", checked: true, id: "sounds.activity_end", sound: true },
  { block: "Звуки", label: "Присоединение к активности", checked: true, id: "sounds.activity_join", sound: true },
  { block: "Звуки", label: "Покинул активность", checked: true, id: "sounds.activity_leave", sound: true },
  { block: "Звуки", label: "Приглашение выступить", checked: true, id: "sounds.activity_invite", sound: true },
  // Расширенные (4)
  { block: "Расширенные", label: "OpenH264", checked: true, id: "advanced.openh264" },
  { block: "Расширенные", label: "Оверлей стрима", checked: false, id: "advanced.stream_overlay" },
  { block: "Расширенные", label: "Запись звука для диагностики", checked: false, id: "advanced.diagnostic_recording" },
  { block: "Расширенные", label: "Журнал отладки", checked: true, id: "advanced.debug_log" },
];

/** Стабильные id тумблеров в порядке DOM: миграция старых настроек по индексу. */
export const SETTINGS_TOGGLE_IDS = TOGGLE_SPECS.map((spec) => spec.id);

/** Версия схемы файла настроек (`settings_store.rs::SETTINGS_VERSION`). */
export const SETTINGS_VERSION = 1;
/** Debounce записи: серия кликов даёт один `settings_save` (B8b). */
export const SAVE_DEBOUNCE_MS = 200;

/** Белые списки значений — ровно контракт `settings_store.rs` (B8a). */
export const SETTINGS_DENSITIES = ["cozy", "compact"];
export const SETTINGS_THEMES = ["dark", "light"];
export const SETTINGS_LOCALES = ["ru", "en"];

/**
 * @param {unknown} value
 * @returns {value is Record<string, any>}
 */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * @param {unknown} value
 * @param {string[]} allowed
 * @param {string} fallback
 * @returns {string}
 */
function pick(value, allowed, fallback) {
  return typeof value === "string" && allowed.includes(value) ? value : fallback;
}

/**
 * Старый ключ → стабильный id: точное совпадение, индекс (`"5"` / `"toggle_5"`)
 * или прежняя русская подпись. Неизвестный ключ → `null` (отбрасывается).
 * @param {string} key
 * @returns {string | null}
 */
function resolveToggleId(key) {
  if (SETTINGS_TOGGLE_IDS.includes(key)) return key;
  const legacyIndex = /^(?:toggle[_-])?(\d+)$/.exec(key);
  if (legacyIndex) return SETTINGS_TOGGLE_IDS[Number(legacyIndex[1])] ?? null;
  const legacyLabel = TOGGLE_SPECS.find((spec) => spec.label === key);
  return legacyLabel ? legacyLabel.id : null;
}

/**
 * Не доверяет файлу настроек: оставляет только известные тумблеры с boolean,
 * значения плотности/темы/языка — из белого списка, дефолты cozy/dark/ru.
 * Чужие/секретные ключи в результат не попадают (B8b п.4).
 * @param {unknown} raw
 * @returns {{ version: number, toggles: Record<string, boolean>, density: string, theme: string, locale: string }}
 */
export function normalizeSettings(raw) {
  const source = isRecord(raw) ? raw : {};
  const rawToggles = isRecord(source.toggles) ? source.toggles : {};
  /** @type {Record<string, boolean>} */
  const toggles = {};
  for (const [key, value] of Object.entries(rawToggles)) {
    if (typeof value !== "boolean") continue;
    const id = resolveToggleId(key);
    if (id !== null) toggles[id] = value;
  }
  return {
    version: SETTINGS_VERSION,
    toggles,
    density: pick(source.density, SETTINGS_DENSITIES, "cozy"),
    theme: pick(source.theme, SETTINGS_THEMES, "dark"),
    locale: pick(source.locale, SETTINGS_LOCALES, "ru"),
  };
}

/**
 * Совместимость с оракулом W39d (`model-bench.mjs` читает SETTINGS_GROUPS`):
 * те же 10 блоков и 38 пунктов, без UI-специфики.
 * @typedef {{ title: string, items: string[] }} SettingsGroup
 * @type {SettingsGroup[]}
 */
export const SETTINGS_GROUPS = SETTINGS_BLOCKS.map((title) => ({
  title,
  items: TOGGLE_SPECS.filter((spec) => spec.block === title).map((spec) => spec.label),
}));

/**
 * @typedef {{
 *   root: HTMLElement,
 *   nav: HTMLElement | null,
 *   toggles: HTMLInputElement[],
 *   groups: SettingsGroup[],
 *   sections: readonly string[],
 *   blocks: string[],
 *   getPane: () => string,
 *   showPane: (key: string) => void,
 * }} SettingsConsole
 */

/** @typedef {"voice" | "appearance" | "notifications" | "none"} PaneKey */

/** @type {Record<string, PaneKey>} */
const PANE_BY_SECTION = {
  "Голос и видео": "voice",
  "Внешний вид": "appearance",
  Уведомления: "notifications",
};

/**
 * @param {{ root: HTMLElement, nav: HTMLElement | null, invoke?: ((cmd: string, args?: any) => Promise<any>) | null }} options
 * @returns {SettingsConsole}
 */
export function createSettingsConsole(options) {
  const { root, nav } = options;
  /** @type {((cmd: string, args?: any) => Promise<any>) | null} */
  const invoke = options.invoke ?? null;
  /** @type {HTMLInputElement[]} */
  const toggles = [];
  /** @type {Map<PaneKey, HTMLElement>} */
  const panes = new Map();
  /** @type {HTMLElement[]} */
  const navItems = [];
  /** @type {PaneKey} */
  let currentPane = "voice";
  /** @type {ReturnType<typeof setTimeout> | null} */
  let saveTimer = null;

  /** @param {string} tag @param {string} className @param {string} [text] */
  function element(tag, className, text = "") {
    const node = document.createElement(tag);
    node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  /**
   * ST-01: disabled с видимой причиной — `title` для мыши и
   * `aria-describedby` на скрытый текст для скринридера.
   * @param {HTMLElement} node
   * @param {string} reasonId
   * @param {string} reason
   * @returns {HTMLElement}
   */
  function disableWithReason(node, reasonId, reason) {
    node.setAttribute("disabled", "");
    node.setAttribute("aria-disabled", "true");
    node.title = reason;
    node.setAttribute("aria-describedby", reasonId);
    return node;
  }

  /**
   * @param {string} id
   * @param {string} reason
   * @returns {HTMLElement}
   */
  function reasonNode(id, reason) {
    const node = element("span", "settings-nav-reason", reason);
    node.id = id;
    node.setAttribute("hidden", "");
    return node;
  }


  /**
   * Ключ тумблера: `data-setting-id`, а для разметки без id — стабильный id по
   * индексу (миграция старого DOM, B8b п.3).
   * @param {number} index
   * @returns {string}
   */
  function toggleKey(index) {
    const input = toggles[index];
    const declared = input?.getAttribute("data-setting-id") ?? "";
    return declared !== "" ? declared : SETTINGS_TOGGLE_IDS[index] ?? "";
  }

  /**
   * Гидратация из `settings_load`: тумблеры по id, `data-density`/`data-theme`/
   * `data-locale` + `html[lang]`. Отсутствующие в файле тумблеры сохраняют
   * стартовое состояние из дампа.
   * @param {ReturnType<typeof normalizeSettings>} settings
   */
  function applySettings(settings) {
    const html = document.documentElement;
    html.setAttribute("data-density", settings.density);
    html.setAttribute("data-theme", settings.theme);
    html.setAttribute("data-locale", settings.locale);
    html.setAttribute("lang", settings.locale);
    for (let index = 0; index < toggles.length; index++) {
      const input = toggles[index];
      if (!input) continue;
      const key = toggleKey(index);
      if (key === "" || !Object.hasOwn(settings.toggles, key)) continue;
      input.checked = settings.toggles[key] === true;
      input.setAttribute("aria-checked", input.checked ? "true" : "false");
    }
    syncControls();
  }

  /**
   * Текущее состояние для `settings_save`: только 38 известных id, значения из
   * белых списков. Секретов и чужих ключей здесь появиться не может.
   * @returns {ReturnType<typeof normalizeSettings>}
   */
  function collectSettings() {
    /** @type {Record<string, boolean>} */
    const values = {};
    for (let index = 0; index < toggles.length; index++) {
      const key = toggleKey(index);
      const input = toggles[index];
      if (key === "" || !input) continue;
      values[key] = input.checked === true;
    }
    const html = document.documentElement;
    return normalizeSettings({
      version: SETTINGS_VERSION,
      toggles: values,
      density: html.getAttribute("data-density"),
      theme: html.getAttribute("data-theme"),
      locale: html.getAttribute("data-locale"),
    });
  }

  /**
   * Видимая поверхность ошибок настроек: не оверлей (он бы уничтожил диалог),
   * а строка статуса с `role=status`.
   * @param {string} message
   */
  function showStatus(message) {
    status.textContent = message;
    status.removeAttribute("hidden");
  }

  function clearStatus() {
    status.textContent = "";
    status.setAttribute("hidden", "");
  }

  /** @param {unknown} error @returns {string} */
  function describeError(error) {
    if (isRecord(error) && typeof error.message === "string") return error.message;
    return String(error);
  }

  /**
   * Запись настроек. Ошибка не откатывает UI: состояние уже применено, а
   * пользователь видит, что оно не сохранено (B8b п.2).
   */
  async function persist() {
    if (!invoke) return;
    try {
      await invoke("settings_save", { settings: collectSettings() });
      clearStatus();
    } catch (error) {
      showStatus(`Не удалось сохранить настройки: ${describeError(error)}. Изменения применены, но не сохранены.`);
    }
  }

  /** Debounce: серия изменений за окно даёт ровно один `settings_save`. */
  function scheduleSave() {
    if (!invoke) return;
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void persist();
    }, SAVE_DEBOUNCE_MS);
  }

  /** Читает `settings_load` один раз на старте; ошибка/пусто → дефолты. */
  function loadSettings() {
    if (!invoke) return;
    void invoke("settings_load")
      .then((raw) => {
        applySettings(normalizeSettings(raw));
        clearStatus();
      })
      .catch((error) => {
        applySettings(normalizeSettings(null));
        showStatus(
          `Настройки не прочитаны, используются значения по умолчанию: ${describeError(error)}`,
        );
      });
  }

  /**
   * Тумблер по контракту §7.4.
   * @param {ToggleSpec} spec
   * @returns {HTMLElement}
   */
  function toggleRow(spec) {
    const label = document.createElement("label");
    label.className = "toggle";

    const labelText = element("span", "toggle-label", spec.label);
    const input = document.createElement("input");
    input.className = "toggle-input";
    input.type = "checkbox";
    input.checked = spec.checked;
    input.setAttribute("role", "switch");
    input.setAttribute("aria-checked", spec.checked ? "true" : "false");
    input.setAttribute("aria-label", spec.label);
    input.setAttribute("data-setting-id", spec.id);
    const sync = () => input.setAttribute("aria-checked", input.checked ? "true" : "false");
    input.addEventListener("click", sync);
    input.addEventListener("change", () => {
      sync();
      scheduleSave();
    });
    toggles.push(input);

    const track = element("span", "toggle-track");
    track.setAttribute("aria-hidden", "true");
    track.append(element("span", "toggle-thumb"));

    label.append(labelText, input, track);
    return label;
  }

  /**
   * @param {string} label
   * @param {string[]} values
   * @returns {HTMLElement}
   */
  function selectField(label, values) {
    const box = element("div", "settings-field");
    const caption = element("span", "settings-field-label", label);
    const select = document.createElement("select");
    select.className = "settings-select";
    select.setAttribute("aria-label", label);
    for (const value of values) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value;
      select.append(option);
    }
    select.value = values[0] ?? "";
    box.append(caption, select);
    return box;
  }

  /**
   * SET-10: подпись «N %» — из дампа (49 % при значении 50: `(50−1)/(100−1)`),
   * поэтому она передаётся явно, а не выводится из value.
   * @param {{ label: string, min: number, max: number, value: number, caption: string }} spec
   * @returns {HTMLElement}
   */
  function sliderField(spec) {
    const box = element("div", "settings-field");
    const caption = element("span", "settings-field-label", spec.label);
    const input = document.createElement("input");
    input.className = "settings-slider";
    input.type = "range";
    input.setAttribute("min", String(spec.min));
    input.setAttribute("max", String(spec.max));
    input.setAttribute("aria-label", spec.label);
    input.value = String(spec.value);
    const value = element("span", "settings-slider-value", spec.caption);
    box.append(caption, input, value);
    return box;
  }

  /**
   * @param {string} label
   * @param {(() => void) | null} [onClick]
   * @returns {HTMLElement}
   */
  function actionButton(label, onClick = null) {
    const button = element("button", "settings-action", label);
    button.setAttribute("type", "button");
    if (onClick) button.addEventListener("click", onClick);
    else {
      // Phase 3: честная заглушка с подсказкой, а не мёртвая кнопка.
      button.setAttribute("aria-disabled", "true");
      button.title = "Недоступно в спайке (Phase 3)";
    }
    return button;
  }

  /** SET-11: «Профиль ввода» — радиогруппа из 3 пунктов. */
  function inputProfile() {
    const group = element("div", "settings-radio-group");
    group.setAttribute("role", "radiogroup");
    group.setAttribute("aria-label", "Профиль ввода");
    /** @type {HTMLElement[]} */
    const radios = [];
    ["Изоляция голоса", "Студия", "Пользовательский"].forEach((label, index) => {
      const radio = element("button", "settings-radio", label);
      radio.setAttribute("type", "button");
      radio.setAttribute("role", "radio");
      radio.setAttribute("aria-checked", index === 0 ? "true" : "false");
      radio.addEventListener("click", () => {
        for (const node of radios) node.setAttribute("aria-checked", node === radio ? "true" : "false");
      });
      radios.push(radio);
      group.append(radio);
    });
    return group;
  }

  /** SET-12/13: кнопки вкладки и связанные переходы. */
  function relatedSettings() {
    const box = element("div", "settings-related");
    const toNotifications = element("button", "settings-related-link", "Уведомления");
    toNotifications.setAttribute("type", "button");
    toNotifications.addEventListener("click", () => showPane("notifications"));
    box.append(
      toNotifications,
      actionButton("Выбрать звук"),
      actionButton("Изменить звук"),
    );
    return box;
  }

  /**
   * @param {string} block
   * @returns {HTMLElement[]}
   */
  function blockExtras(block) {
    if (block === "Голос") {
      return [
        selectField("Микрофон", ["Система: Easy Effects Source", "По умолчанию"]),
        selectField("Динамик", ["Система: Easy Effects Sink", "По умолчанию"]),
        sliderField({ label: "Громкость микрофона", min: 0, max: 100, value: 100, caption: "100 %" }),
        sliderField({ label: "Громкость динамика", min: 0, max: 200, value: 100, caption: "100 %" }),
        actionButton("Проверка микрофона"),
      ];
    }
    if (block === "Профиль ввода") return [inputProfile()];
    if (block === "Шумоподавление") {
      return [selectField("Шумоподавление", ["Krisp", "Стандартное"])];
    }
    if (block === "Камера") {
      return [
        selectField("Камера", ["Нет видеоустройств"]),
        actionButton("Проверить видео"),
      ];
    }
    if (block === "Стриминг") {
      // SET-10: 1–100 = 50, подпись из дампа 49 % (floor((50-1)/(100-1)*100)).
      return [sliderField({ label: "Приглушение стрима", min: 1, max: 100, value: 50, caption: "49 %" })];
    }
    if (block === "Звуки") {
      return [sliderField({ label: "Громкость звуковой панели", min: 0, max: 100, value: 26.11, caption: "26 %" })];
    }
    if (block === "Связанные настройки") return [relatedSettings()];
    if (block === "Расширенные") {
      return [
        actionButton("Загрузить журналы"),
        actionButton("Показать папку"),
        actionButton("Сброс"),
      ];
    }
    return [];
  }

  /** @param {PaneKey} key @param {string} title @returns {HTMLElement} */
  function createPane(key, title) {
    const pane = element("section", "settings-pane");
    pane.setAttribute("data-pane", key);
    pane.append(element("h3", "settings-pane-title", title));
    return pane;
  }

  /* ---------- «Голос и видео» ---------- */
  const voice = createPane("voice", "Голос и видео");
  const soundRows = TOGGLE_SPECS.filter((spec) => spec.sound === true);
  for (const block of SETTINGS_BLOCKS) {
    const rows = TOGGLE_SPECS.filter((spec) => spec.block === block && spec.sound !== true);
    const extras = blockExtras(block);
    if (rows.length === 0 && extras.length === 0) continue;
    voice.append(element("h4", "settings-block-title", block));
    voice.append(...extras);
    if (block === "Звуки") {
      // SET-07: 4 видимых звука + 18 за кнопкой.
      voice.append(...soundRows.slice(0, 4).map(toggleRow));
      const extra = element("div", "settings-sounds-extra");
      extra.setAttribute("hidden", "");
      extra.append(...soundRows.slice(4).map(toggleRow));
      const more = element("button", "settings-sounds-more", "Показать ещё 18 звуков");
      more.setAttribute("type", "button");
      more.setAttribute("aria-expanded", "false");
      more.addEventListener("click", () => {
        const hidden = extra.getAttribute("hidden") !== null;
        if (hidden) extra.removeAttribute("hidden");
        else extra.setAttribute("hidden", "");
        more.setAttribute("aria-expanded", hidden ? "true" : "false");
        more.textContent = hidden ? "Скрыть дополнительные звуки" : "Показать ещё 18 звуков";
      });
      voice.append(extra, more);
      continue;
    }
    voice.append(...rows.map(toggleRow));
  }

  /* ---------- «Внешний вид» (S-14) и «Уведомления» (SET-15) ---------- */
  const appearance = createPane("appearance", "Внешний вид");
  appearance.append(element("h4", "settings-block-title", "Отображение сообщений"));
  /** @type {Array<{ value: string, node: HTMLElement }>} */
  const densityOptions = [];
  const density = element("div", "settings-density");
  density.setAttribute("role", "group");
  density.setAttribute("aria-label", "Отображение сообщений");
  for (const [value, label] of /** @type {Array<[string, string]>} */ ([
    ["compact", "Компактное"],
    ["cozy", "Уютное"],
  ])) {
    const button = element("button", "settings-density-option", label);
    button.setAttribute("type", "button");
    button.setAttribute("aria-pressed", "false");
    button.addEventListener("click", () => {
      document.documentElement.setAttribute("data-density", value);
      syncControls();
      scheduleSave();
    });
    densityOptions.push({ value, node: button });
    density.append(button);
  }

  // B8b п.2: тема тоже сохраняется (dark/light), UI — две кнопки как у плотности.
  const themeGroup = element("div", "settings-theme");
  themeGroup.setAttribute("role", "group");
  themeGroup.setAttribute("aria-label", "Тема оформления");
  /** @type {Array<{ value: string, node: HTMLElement }>} */
  const themeOptions = [];
  for (const [value, label] of /** @type {Array<[string, string]>} */ ([
    ["dark", "Тёмная"],
    ["light", "Светлая"],
  ])) {
    const button = element("button", "settings-theme-option", label);
    button.setAttribute("type", "button");
    button.setAttribute("aria-pressed", "false");
    button.addEventListener("click", () => {
      document.documentElement.setAttribute("data-theme", value);
      syncControls();
      scheduleSave();
    });
    themeOptions.push({ value, node: button });
    themeGroup.append(button);
  }

  /** Синхронизация `aria-pressed` плотности/темы с `html[data-*]`. */
  function syncControls() {
    const html = document.documentElement;
    for (const option of densityOptions) {
      option.node.setAttribute(
        "aria-pressed",
        html.getAttribute("data-density") === option.value ? "true" : "false",
      );
    }
    for (const option of themeOptions) {
      option.node.setAttribute(
        "aria-pressed",
        html.getAttribute("data-theme") === option.value ? "true" : "false",
      );
    }
  }
  syncControls();

  appearance.append(
    density,
    themeGroup,
    element(
      "p",
      "settings-stub-note",
      "Плотность и тема переключаются и сохраняются локально; остальные параметры вкладки "
        + "в RE-сессии не подтверждены — это честная заглушка (S-14).",
    ),
  );

  // Видимый статус сохранения/чтения настроек: ошибка не должна быть молчаливой.
  const status = element("p", "settings-save-status", "");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.setAttribute("hidden", "");
  root.append(status);

  const notifications = createPane("notifications", "Уведомления");
  notifications.append(
    element(
      "p",
      "settings-stub-note",
      "Вкладка «Уведомления» в RE-сессии не открывалась: состав и тумблеры неизвестны "
        + "(SET-15). Ничего сверх подтверждённого не рисуем.",
    ),
  );

  panes.set("voice", voice);
  panes.set("appearance", appearance);
  panes.set("notifications", notifications);
  root.append(voice, appearance, notifications);

  /**
   * @param {string} key
   */
  function showPane(key) {
    /** @type {PaneKey} */
    const pane = /** @type {any} */ (PANE_BY_SECTION[key] ?? "none");
    if (pane === "none" || !panes.has(pane)) return;
    currentPane = pane;
    root.setAttribute("data-active-pane", pane);
    for (const [name, node] of panes) {
      if (name === pane) node.removeAttribute("hidden");
      else node.setAttribute("hidden", "");
    }
    for (const button of navItems) {
      const active = button.getAttribute("data-section") === key;
      button.classList.toggle("active", active);
      if (active) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  /**
   * ST-01: «Выйти» → существующая команда `auth_logout` (main.rs:381).
   * Аргументов нет — токен в эту команду не передаётся. Ошибка не глотается:
   * она видна в `role=status`, как и ошибка сохранения настроек. Возврат
   * оболочки в `no-session` — забота auth-гейта (`app.js`), не навигации.
   * @param {HTMLElement} button
   */
  async function performLogout(button) {
    if (!invoke || button.getAttribute("data-logout-state") === "pending") return;
    button.setAttribute("data-logout-state", "pending");
    button.setAttribute("disabled", "");
    button.setAttribute("aria-disabled", "true");
    showStatus("Выходим из аккаунта…");
    try {
      await invoke("auth_logout");
      button.setAttribute("data-logout-state", "done");
      showStatus("Вы вышли из аккаунта.");
    } catch (error) {
      button.setAttribute("data-logout-state", "error");
      showStatus(`Не удалось выйти из аккаунта: ${describeError(error)}.`);
    } finally {
      // Команда идемпотентна на нашей стороне: повторный клик безопасен.
      button.removeAttribute("disabled");
      button.removeAttribute("aria-disabled");
    }
  }

  if (nav) {
    /* ---------- профиль (оракул ST-01: 2 строки) ---------- */
    const profile = element("div", "settings-profile");
    const avatar = element("span", "avatar avatar--lg settings-profile-avatar", SETTINGS_PROFILE.monogram);
    avatar.setAttribute("aria-hidden", "true");
    const profileText = element("span", "settings-profile-text");
    const editProfile = /** @type {HTMLElement} */ (
      disableWithReason(
        element("button", "settings-profile-edit", SETTINGS_PROFILE.action),
        "settings-profile-reason",
        SETTINGS_PROFILE_REASON,
      )
    );
    editProfile.setAttribute("type", "button");
    profileText.append(element("span", "settings-profile-name", SETTINGS_PROFILE.name), editProfile);
    profile.append(avatar, profileText);
    nav.append(profile);

    /* ---------- 18 разделов; у «Учётной записи» — 4 подпункта ---------- */
    for (const section of SETTINGS_SECTIONS) {
      const button = element("button", "settings-nav-item", section);
      button.setAttribute("type", "button");
      button.setAttribute("data-section", section);
      if (!SETTINGS_ENABLED.includes(section)) {
        disableWithReason(button, "settings-nav-reason", SETTINGS_SECTION_REASON);
      }
      button.addEventListener("click", () => {
        if (button.getAttribute("disabled") !== null) return;
        showPane(section);
      });
      navItems.push(button);
      nav.append(button);

      if (section !== "Учётная запись") continue;
      // Оракул: подпункты раскрыты у «Учётной записи» (aria-expanded у родителя).
      button.id = "settings-nav-account";
      button.setAttribute("aria-expanded", "true");
      button.setAttribute("aria-controls", "settings-subnav-account");
      const subnav = element("div", "settings-subnav");
      subnav.id = "settings-subnav-account";
      subnav.setAttribute("role", "group");
      subnav.setAttribute("aria-labelledby", "settings-nav-account");
      for (const item of SETTINGS_ACCOUNT_ITEMS) {
        const sub = element("button", "settings-subnav-item", item);
        sub.setAttribute("type", "button");
        // Имена из дампа, но панелей в нём нет — disabled с причиной.
        disableWithReason(sub, "settings-subnav-reason", SETTINGS_ITEM_REASON);
        subnav.append(sub);
      }
      nav.append(subnav);
    }

    /* ---------- «Выйти» (ST-01: отдельная запись оракула, destructive) ---------- */
    const logout = element("button", "settings-nav-logout", "Выйти");
    logout.setAttribute("type", "button");
    logout.setAttribute("data-logout-state", "idle");
    if (invoke) {
      logout.title = "Выйти из аккаунта";
      logout.addEventListener("click", () => {
        void performLogout(logout);
      });
    } else {
      disableWithReason(logout, "settings-logout-reason", SETTINGS_LOGOUT_REASON);
    }
    nav.append(logout);

    // Скрытые тексты причин: на них ссылается aria-describedby.
    nav.append(
      reasonNode("settings-profile-reason", SETTINGS_PROFILE_REASON),
      reasonNode("settings-nav-reason", SETTINGS_SECTION_REASON),
      reasonNode("settings-subnav-reason", SETTINGS_ITEM_REASON),
      reasonNode("settings-logout-reason", SETTINGS_LOGOUT_REASON),
    );

    /**
     * ST-01: стрелки/Home/End ходят только по доступным пунктам (разделы с
     * контентом + «Выйти»), disabled-разделы и подпункты не получают фокус.
     */
    nav.addEventListener("keydown", (event) => {
      if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
      /** @type {HTMLElement[]} */
      const entries = [];
      for (const node of nav.querySelectorAll("button")) {
        if (node.getAttribute("disabled") !== null) continue;
        if (node.getAttribute("aria-disabled") === "true") continue;
        entries.push(node);
      }
      if (entries.length === 0) return;
      const current = entries.indexOf(/** @type {HTMLElement} */ (document.activeElement));
      const last = entries.length - 1;
      /** @type {number} */
      let next;
      if (event.key === "Home") next = 0;
      else if (event.key === "End") next = last;
      else if (event.key === "ArrowDown") next = current < 0 ? 0 : (current + 1) % entries.length;
      else next = current < 0 ? last : (current - 1 + entries.length) % entries.length;
      event.preventDefault();
      entries[next]?.focus();
    });
  }

  showPane("Голос и видео");
  loadSettings();

  return {
    root,
    nav,
    toggles,
    groups: SETTINGS_GROUPS,
    sections: SETTINGS_SECTIONS,
    blocks: SETTINGS_BLOCKS,
    getPane: () => currentPane,
    showPane,
  };
}

/**
 * @param {{ invoke?: ((cmd: string, args?: any) => Promise<any>) | null }} [options]
 * @returns {SettingsConsole}
 */
export function initSettingsConsole(options = {}) {
  const root = byId("settings");
  if (!root) throw new UIError("ELEMENT_MISSING", "в DOM нет #settings");
  const nav = byId("settings-nav");
  const tauri = /** @type {any} */ (window).__TAURI__;
  /** @type {((cmd: string, args?: any) => Promise<any>) | null} */
  const bridgeInvoke = tauri?.core?.invoke
    ? (cmd, args) => tauri.core.invoke(cmd, args)
    : null;
  const invoke = options.invoke ?? bridgeInvoke;
  return createSettingsConsole({ root, nav, invoke });
}
