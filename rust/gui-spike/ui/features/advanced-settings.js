// @ts-check
/**
 * TASK-628 (G2 Frontend): модуль расширенных настроек пользователя (Advanced Settings UI):
 * - Горячие клавиши (Keybinds): пользовательские сочетания клавиш, запись с клавиатуры,
 *   детектор конфликтов со встроенными и кастомными биндами, защита `managed` биндов;
 * - Звуковая панель (Soundboard): громкость `0..100%`, глобальные и серверные звуки
 *   присоединения (`Entrance Sounds`) с fallback, предпросмотр и загрузка кастомных звуков;
 * - Видеонарезки (Clips): включение записи клипов, длительность (`30`/`60`/`120` сек.),
 *   разрешение (`480p`/`720p`/`1080p`/`1440p`/`Источник`), частота кадров (`15`/`30`/`60` FPS),
 *   качество битрейта (`0..100%`), запись голоса в клипах, напоминания, папка хранения
 *   и предупреждение о слабом оборудовании.
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/decoded/d8b0ffc3255e8113_0` (`@152000..170000` Keybinds,
 *   `@198479` Soundboard Volume & Entrance Sounds, `@516500..525500` Clips Settings);
 * - `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0` (модуль `274372` ClipsStore,
 *   модуль `696016` `SECONDS_30`/`MINUTES_1`/`MINUTES_2`, `Ot = "alt+c"`,
 *   модуль `753070` `RESOLUTION_480..1440/SOURCE`, `FPS_15/30/60`, модуль `994778` keybind labels);
 * - `/home/sanya/re-deep/decoded/fe99f9e2d77f4c0b_0` (модуль `226640` опции клипов);
 * - `/home/sanya/re-deep/decoded/e2135c73a7dbde68_0` (`@2363` Keybind Recorder);
 * - `/home/sanya/re-deep/ui8/23-settings-voice.json` и `/home/sanya/re-deep/ru-catalog.json`.
 *
 * @module ui/features/advanced-settings
 */

import {
  DEFAULT_SHORTCUTS,
  formatShortcutAriaLabelRu,
  normalizeKeyCombo,
} from "./keybinds.js";
import {
  MAX_SOUNDBOARD_BYTES,
  MAX_SOUNDBOARD_NAME_CHARS,
  MIN_SOUNDBOARD_NAME_CHARS,
  isValidSoundboardName,
} from "./soundboard-picker.js";

/** Ключ сохранения расширенных настроек в локальном хранилище. */
export const ADVANCED_SETTINGS_STORAGE_KEY = "discord.advanced_settings";

/**
 * @typedef {"keybinds" | "soundboard" | "clips"} AdvancedSettingsTabId
 */

/**
 * @typedef {object} AdvancedSettingsTabEntry
 * @property {AdvancedSettingsTabId} id
 * @property {string} labelRu
 */

/**
 * Справочник вкладок расширенных настроек по RE-каталогу (`T9DA2K`, `OWQNYr`, `z2jK6X`).
 * @type {ReadonlyArray<Readonly<AdvancedSettingsTabEntry>>}
 */
export const ADVANCED_SETTINGS_TABS_RU = Object.freeze([
  Object.freeze({ id: /** @type {const} */ ("keybinds"), labelRu: "Горячие клавиши" }),
  Object.freeze({ id: /** @type {const} */ ("soundboard"), labelRu: "Звуковая панель" }),
  Object.freeze({ id: /** @type {const} */ ("clips"), labelRu: "Видеонарезки" }),
]);

/**
 * @typedef {object} KeybindActionEntry
 * @property {string} id
 * @property {string} value
 * @property {string} labelRu
 * @property {string} descriptionRu
 */

/**
 * Справочник действий пользовательских горячих клавиш по RE-модулям `994778` и `d8b0ffc3255e8113_0`.
 * @type {ReadonlyArray<Readonly<KeybindActionEntry>>}
 */
export const KEYBIND_ACTION_TYPES = Object.freeze([
  Object.freeze({
    id: "unassigned",
    value: "UNASSIGNED",
    labelRu: "Не назначено",
    descriptionRu:
      "Эта команда ничего не делает! Пожалуйста, выберите, что вам нужно из выпадающего меню.",
  }),
  Object.freeze({
    id: "push-to-talk",
    value: "PUSH_TO_TALK",
    labelRu: "Рация (обычный режим)",
    descriptionRu: "Удерживайте, чтобы временно включать микрофон в режиме режим рации.",
  }),
  Object.freeze({
    id: "push-to-talk-priority",
    value: "PUSH_TO_TALK_PRIORITY",
    labelRu: "Рация (приоритетный режим)",
    descriptionRu:
      "В режиме рации нужно удерживать кнопку, чтобы включить микрофон с приоритетом в разговоре.",
  }),
  Object.freeze({
    id: "push-to-mute",
    value: "PUSH_TO_MUTE",
    labelRu: "Обратная рация",
    descriptionRu:
      "Удерживайте, чтобы временно отключить ваш микрофон в режиме активации по голосу.",
  }),
  Object.freeze({
    id: "vad-priority",
    value: "VAD_PRIORITY",
    labelRu: "Приоритет в режиме активации по голосу",
    descriptionRu:
      "В режиме активации по голосу удерживайте кнопку, чтобы говорить с приоритетом.",
  }),
  Object.freeze({
    id: "toggle-mute",
    value: "TOGGLE_MUTE",
    labelRu: "Вкл./выкл. микрофон",
    descriptionRu: "Включает/выключает микрофон.",
  }),
  Object.freeze({
    id: "toggle-deafen",
    value: "TOGGLE_DEAFEN",
    labelRu: "Вкл./выкл. звук в динамиках",
    descriptionRu:
      "Включает/выключает воспроизведение звука из динамиков. Микрофон включается/выключается в то же время.",
  }),
  Object.freeze({
    id: "toggle-voice-mode",
    value: "TOGGLE_VOICE_MODE",
    labelRu: "Вкл./выкл. обнаружение активации по голосу",
    descriptionRu: "Переключение между режимом рации и активацией по голосу.",
  }),
  Object.freeze({
    id: "toggle-streamer-mode",
    value: "TOGGLE_STREAMER_MODE",
    labelRu: "Вкл./выкл. режим стримера",
    descriptionRu: "Переключает режим стримера.",
  }),
  Object.freeze({
    id: "toggle-voice-channel-chat",
    value: "TOGGLE_VOICE_CHANNEL_CHAT",
    labelRu: "Включить или отключить чат голосового канала",
    descriptionRu:
      "Включить или отключить панель текстового чата в голосовом канале, к которому вы присоединились.",
  }),
  Object.freeze({
    id: "toggle-camera",
    value: "TOGGLE_CAMERA",
    labelRu: "Вкл./выкл. камеру",
    descriptionRu: "Вкл./выкл. вашу камеру в голосовом канале.",
  }),
  Object.freeze({
    id: "toggle-go-live-streaming",
    value: "TOGGLE_GO_LIVE_STREAMING",
    labelRu: "Вкл./выкл. демонстрацию экрана",
    descriptionRu: "Запускает/останавливает стрим в текущем голосовом канале.",
  }),
  Object.freeze({
    id: "navigate-back",
    value: "NAVIGATE_BACK",
    labelRu: "Перейти назад",
    descriptionRu: "Перейдите назад на истории страниц",
  }),
  Object.freeze({
    id: "navigate-forward",
    value: "NAVIGATE_FORWARD",
    labelRu: "Перейти вперёд",
    descriptionRu: "Перейдите вперёд на истории страниц",
  }),
  Object.freeze({
    id: "switch-to-voice-channel",
    value: "SWITCH_TO_VOICE_CHANNEL",
    labelRu: "Переключиться на голосовой канал",
    descriptionRu: "Быстрое подключение к выбранному голосовому каналу.",
  }),
  Object.freeze({
    id: "disconnect-from-voice-channel",
    value: "DISCONNECT_FROM_VOICE_CHANNEL",
    labelRu: "Отключиться от голосового канала",
    descriptionRu: "Отключает от текущего голосового канала.",
  }),
  Object.freeze({
    id: "soundboard",
    value: "SOUNDBOARD",
    labelRu: "Открыть звуковую панель",
    descriptionRu: "Открыть звуковую панель возле курсора мыши в приложении или игровом оверлее.",
  }),
  Object.freeze({
    id: "soundboard-hold",
    value: "SOUNDBOARD_HOLD",
    labelRu: "Открыть звуковую панель (зажать)",
    descriptionRu:
      "Зажмите, чтобы открыть звуковую панель возле курсора мыши в приложении или игровом оверлее.",
  }),
  Object.freeze({
    id: "save-clip",
    value: "SAVE_CLIP",
    labelRu: "Сохранить клип",
    descriptionRu:
      "Создаёт клип из последних 30 секунд – 2 минут вашего стрима в зависимости от выбранных вами настроек.",
  }),
]);

/** Минимальная громкость звуковой панели в процентах (`d8b0ffc3255e8113_0 @198479`). */
export const MIN_SOUNDBOARD_VOLUME_PERCENT = 0;

/** Максимальная громкость звуковой панели в процентах (`d8b0ffc3255e8113_0 @198479`). */
export const MAX_SOUNDBOARD_VOLUME_PERCENT = 100;

/** Громкость звуковой панели по умолчанию в процентах. */
export const DEFAULT_SOUNDBOARD_VOLUME_PERCENT = 100;

/** Идентификатор глобального звука присоединения («Все серверы», `23-settings-voice.json`). */
export const GLOBAL_ENTRANCE_GUILD_ID = "all";

/** Допустимые длительности клипов в секундах (`30 сек.`, `1 мин.`, `2 мин.`, RE `696016` / `226640`). */
export const CLIPS_LENGTH_SECONDS = Object.freeze([30, 60, 120]);

/** Допустимые разрешения записи клипов (`480p`, `720p`, `1080p`, `1440p`, `0 = Источник`, RE `753070`). */
export const CLIPS_RESOLUTION_OPTIONS = Object.freeze([480, 720, 1080, 1440, 0]);

/** Допустимые значения частоты кадров клипов (`15`, `30`, `60` FPS, RE `753070` / `226640`). */
export const CLIPS_FPS_OPTIONS = Object.freeze([15, 30, 60]);

/** Горячая клавиша сохранения клипа по умолчанию (`Alt+C`, RE `696016` `Ot`, `QyB/jK`). */
export const DEFAULT_CLIPS_KEYBIND = "Alt+C";

/**
 * @typedef {object} ClipsSettingsState
 * @property {boolean} clipsEnabled
 * @property {number} clipsLength
 * @property {number} resolution
 * @property {number} frameRate
 * @property {number} bitratePercent
 * @property {boolean} voiceAudioEnabled
 * @property {boolean} remindersEnabled
 * @property {string} clipKeybind
 * @property {string} storageLocation
 * @property {boolean} hardwareBelowMinimum
 */

/**
 * Настройки клипов по умолчанию (`3eefa7dae1e2c234_0 @3629890` модуль `274372`).
 * @type {Readonly<ClipsSettingsState>}
 */
export const DEFAULT_CLIPS_SETTINGS = Object.freeze({
  clipsEnabled: false,
  clipsLength: 30,
  resolution: 1080,
  frameRate: 30,
  bitratePercent: 75,
  voiceAudioEnabled: true,
  remindersEnabled: true,
  clipKeybind: DEFAULT_CLIPS_KEYBIND,
  storageLocation: "Discord Clips",
  hardwareBelowMinimum: false,
});

/**
 * @typedef {object} CustomKeybindItem
 * @property {string} id
 * @property {string} action
 * @property {string} shortcut
 * @property {boolean} enabled
 * @property {boolean} [managed]
 */

/**
 * @typedef {object} SoundboardSoundOption
 * @property {string} soundId
 * @property {string} name
 * @property {string} [emojiName]
 * @property {boolean} [available]
 */

/**
 * @typedef {object} SoundboardSettingsState
 * @property {number} volumePercent
 * @property {string} selectedGuildId
 * @property {Record<string, string>} entranceSounds
 * @property {SoundboardSoundOption[]} sounds
 */

/**
 * @typedef {object} GuildOptionItem
 * @property {string} id
 * @property {string} name
 */

/**
 * @typedef {object} KeybindConflictReport
 * @property {boolean} hasConflict
 * @property {boolean} hasBuiltInConflict
 * @property {string | null} builtInAction
 * @property {boolean} hasCustomConflict
 * @property {string[]} conflictingIds
 * @property {string} messageRu
 */

/**
 * @typedef {object} AdvancedSettingsStorage
 * @property {(key: string) => string | null} getItem
 * @property {(key: string, value: string) => void} setItem
 */

/**
 * @typedef {object} AdvancedSettingsControllerOptions
 * @property {any} container
 * @property {AdvancedSettingsTabId} [initialTab]
 * @property {ReadonlyArray<Partial<CustomKeybindItem>>} [keybinds]
 * @property {Partial<SoundboardSettingsState>} [soundboard]
 * @property {Partial<ClipsSettingsState>} [clips]
 * @property {ReadonlyArray<GuildOptionItem>} [guilds]
 * @property {AdvancedSettingsStorage | null} [storage]
 * @property {(keybinds: CustomKeybindItem[]) => void} [onKeybindsChange]
 * @property {(soundboard: SoundboardSettingsState) => void} [onSoundboardChange]
 * @property {(clips: ClipsSettingsState) => void} [onClipsChange]
 * @property {(soundId: string, volumeFloat: number) => void} [onPlaySound]
 * @property {(payload: { name: string, soundDataUri: string, volume: number }) => void} [onUploadSound]
 */

/** Сообщения об ошибках и предупреждениях из RE-каталога (`ru-catalog.json`). */
const BUILTIN_CONFLICT_WARNING_RU =
  "Это конфликтует со встроенным сочетанием клавиш. Мы отключили встроенное сочетание.";
const CUSTOM_CONFLICT_WARNING_RU =
  "Это сочетание клавиш уже назначено для другого пользовательского действия.";
const HARDWARE_WARNING_RU =
  "Ваш компьютер не отвечает минимальным требованиям для создания клипов — производительность может снизиться, если вы включите функцию «Клипы».";
const INVALID_SOUNDBOARD_NAME_ERROR_RU =
  "Недопустимое имя звука: укажите от 2 до 32 символов.";
const INVALID_SOUNDBOARD_URI_ERROR_RU =
  "Недопустимый аудиофайл: укажите корректный Data URI с префиксом data:audio/ размером до 512 КБ.";

/**
 * Проверяет, входит ли идентификатор действия в `KEYBIND_ACTION_TYPES`.
 *
 * @param {unknown} action
 * @returns {action is string}
 */
export function isValidKeybindAction(action) {
  return (
    typeof action === "string" &&
    KEYBIND_ACTION_TYPES.some((entry) => entry.value === action)
  );
}

/**
 * Возвращает русскую подпись действия горячей клавиши (`994778`).
 *
 * @param {unknown} action
 * @returns {string}
 */
export function getKeybindActionLabelRu(action) {
  if (typeof action !== "string") {
    return "Не назначено";
  }
  const found = KEYBIND_ACTION_TYPES.find((entry) => entry.value === action);
  return found ? found.labelRu : action;
}

/**
 * Возвращает русское описание действия горячей клавиши (`d8b0ffc3255e8113_0`).
 *
 * @param {unknown} action
 * @returns {string}
 */
export function getKeybindActionDescriptionRu(action) {
  if (typeof action !== "string") {
    return KEYBIND_ACTION_TYPES[0]?.descriptionRu ?? "";
  }
  const found = KEYBIND_ACTION_TYPES.find((entry) => entry.value === action);
  return found ? found.descriptionRu : "";
}

/**
 * Определяет конфликты комбинации клавиш `comboString` со встроенными (`DEFAULT_SHORTCUTS`)
 * и пользовательскими горячими клавишами (`7lQlw3`).
 *
 * @param {ReadonlyArray<Partial<CustomKeybindItem>>} keybinds
 * @param {string | null | undefined} comboString
 * @param {string} [excludeId]
 * @returns {KeybindConflictReport}
 */
export function detectKeybindConflicts(keybinds, comboString, excludeId) {
  const normalizedCombo = typeof comboString === "string" ? comboString.trim() : "";
  if (!normalizedCombo) {
    return {
      hasConflict: false,
      hasBuiltInConflict: false,
      builtInAction: null,
      hasCustomConflict: false,
      conflictingIds: [],
      messageRu: "",
    };
  }

  const lowerTarget = normalizedCombo.toLowerCase();
  /** @type {string | null} */
  let builtInAction = null;
  for (const [actionName, defaultCombo] of Object.entries(DEFAULT_SHORTCUTS)) {
    if (defaultCombo.toLowerCase() === lowerTarget) {
      builtInAction = actionName;
      break;
    }
  }

  /** @type {string[]} */
  const conflictingIds = [];
  if (Array.isArray(keybinds)) {
    for (const item of keybinds) {
      if (!item || typeof item !== "object") continue;
      if (excludeId && item.id === excludeId) continue;
      if (item.enabled === false) continue;
      const itemCombo = typeof item.shortcut === "string" ? item.shortcut.trim() : "";
      if (itemCombo && itemCombo.toLowerCase() === lowerTarget && typeof item.id === "string") {
        conflictingIds.push(item.id);
      }
    }
  }

  const hasBuiltInConflict = builtInAction !== null;
  const hasCustomConflict = conflictingIds.length > 0;
  const hasConflict = hasBuiltInConflict || hasCustomConflict;

  let messageRu = "";
  if (hasBuiltInConflict) {
    messageRu = BUILTIN_CONFLICT_WARNING_RU;
  } else if (hasCustomConflict) {
    messageRu = CUSTOM_CONFLICT_WARNING_RU;
  }

  return {
    hasConflict,
    hasBuiltInConflict,
    builtInAction,
    hasCustomConflict,
    conflictingIds,
    messageRu,
  };
}

/**
 * Ограничивает громкость звуковой панели в процентах диапазоном `0..100`
 * (округляет до целого, при нечисловом или `NaN` возвращает `100`).
 *
 * @param {unknown} percent
 * @returns {number}
 */
export function clampSoundboardVolumePercent(percent) {
  if (typeof percent !== "number" || Number.isNaN(percent) || !Number.isFinite(percent)) {
    return DEFAULT_SOUNDBOARD_VOLUME_PERCENT;
  }
  const rounded = Math.round(percent);
  if (rounded < MIN_SOUNDBOARD_VOLUME_PERCENT) {
    return MIN_SOUNDBOARD_VOLUME_PERCENT;
  }
  if (rounded > MAX_SOUNDBOARD_VOLUME_PERCENT) {
    return MAX_SOUNDBOARD_VOLUME_PERCENT;
  }
  return rounded === 0 ? 0 : rounded;
}

/**
 * Переводит громкость звуковой панели из процентов (`0..100`) в коэффициент `0..1`.
 *
 * @param {unknown} percent
 * @returns {number}
 */
export function soundboardPercentToFloat(percent) {
  const clamped = clampSoundboardVolumePercent(percent);
  return Number((clamped / 100).toFixed(2));
}

/**
 * Определяет эффективный звук присоединения для сервера `guildId`
 * с автоматическим fallback на глобальный звук (`"all"`).
 *
 * @param {Record<string, string> | null | undefined} entranceSounds
 * @param {string | null | undefined} guildId
 * @returns {string | null}
 */
export function resolveEntranceSoundForGuild(entranceSounds, guildId) {
  if (!entranceSounds || typeof entranceSounds !== "object") {
    return null;
  }
  const normalizedGuild =
    typeof guildId === "string" && guildId.trim().length > 0
      ? guildId.trim()
      : GLOBAL_ENTRANCE_GUILD_ID;

  const specific = entranceSounds[normalizedGuild];
  if (typeof specific === "string" && specific.trim().length > 0) {
    return specific.trim();
  }

  const globalSound = entranceSounds[GLOBAL_ENTRANCE_GUILD_ID];
  if (typeof globalSound === "string" && globalSound.trim().length > 0) {
    return globalSound.trim();
  }

  return null;
}

/**
 * Нормализует длительность клипа к допустимому значению в секундах (`30`, `60`, `120`),
 * поддерживая как секунды, так и миллисекунды из RE (`30000`, `60000`, `120000`).
 *
 * @param {unknown} value
 * @returns {number}
 */
export function normalizeClipLengthSeconds(value) {
  const num = typeof value === "string" && value.trim().length > 0 ? Number(value) : value;
  if (typeof num !== "number" || Number.isNaN(num)) {
    return DEFAULT_CLIPS_SETTINGS.clipsLength;
  }
  if (num === 30000 || num === 60000 || num === 120000) {
    return num / 1000;
  }
  if (CLIPS_LENGTH_SECONDS.includes(num)) {
    return num;
  }
  return DEFAULT_CLIPS_SETTINGS.clipsLength;
}

/**
 * Форматирует русскую подпись длительности клипа (`bTFv/3`, `ICo9Nk`).
 *
 * @param {unknown} seconds
 * @returns {string}
 */
export function formatClipLengthLabelRu(seconds) {
  const normalized = normalizeClipLengthSeconds(seconds);
  if (normalized === 60) return "1 мин.";
  if (normalized === 120) return "2 мин.";
  return `${normalized} сек.`;
}

/**
 * Проверяет допустимость разрешения записи клипов (`480`, `720`, `1080`, `1440`, `0`).
 *
 * @param {unknown} resolution
 * @returns {resolution is number}
 */
export function isValidClipResolution(resolution) {
  return typeof resolution === "number" && CLIPS_RESOLUTION_OPTIONS.includes(resolution);
}

/**
 * Форматирует русскую подпись разрешения клипа (`TEOC0I`, `XjXqzh`).
 *
 * @param {unknown} resolution
 * @returns {string}
 */
export function formatClipResolutionLabelRu(resolution) {
  const num = typeof resolution === "number" ? resolution : DEFAULT_CLIPS_SETTINGS.resolution;
  if (num === 0) {
    return "Источник";
  }
  return `${num}p`;
}

/**
 * Проверяет допустимость частоты кадров записи клипа (`15`, `30`, `60`).
 *
 * @param {unknown} fps
 * @returns {fps is number}
 */
export function isValidClipFrameRate(fps) {
  return typeof fps === "number" && CLIPS_FPS_OPTIONS.includes(fps);
}

/**
 * Форматирует русскую подпись частоты кадров клипа (`Qb44XH`).
 *
 * @param {unknown} fps
 * @returns {string}
 */
export function formatClipFrameRateLabelRu(fps) {
  const num = isValidClipFrameRate(fps) ? fps : DEFAULT_CLIPS_SETTINGS.frameRate;
  return `${num} кадров в секунду`;
}

/**
 * Ограничивает процент битрейта (качества) клипов диапазоном `0..100`
 * (при нечисловом или `NaN` возвращает `75`).
 *
 * @param {unknown} percent
 * @returns {number}
 */
export function clampClipBitratePercent(percent) {
  if (typeof percent !== "number" || Number.isNaN(percent) || !Number.isFinite(percent)) {
    return DEFAULT_CLIPS_SETTINGS.bitratePercent;
  }
  const rounded = Math.round(percent);
  if (rounded < 0) return 0;
  if (rounded > 100) return 100;
  return rounded === 0 ? 0 : rounded;
}

/**
 * Проверяет корректность и безопасность пути папки хранения клипов.
 *
 * @param {unknown} location
 * @returns {{ valid: boolean, normalized: string }}
 */
export function validateClipsStorageLocation(location) {
  if (typeof location !== "string") {
    return { valid: false, normalized: "" };
  }
  const trimmed = location.trim();
  if (trimmed.length === 0 || trimmed.length > 512) {
    return { valid: false, normalized: "" };
  }
  if (trimmed.includes("\0")) {
    return { valid: false, normalized: "" };
  }
  const segments = trimmed.split(/[\\/]+/);
  if (segments.includes("..")) {
    return { valid: false, normalized: "" };
  }
  return { valid: true, normalized: trimmed };
}

/**
 * Проверяет Data URI аудиофайла для кастомного звука саундборда.
 *
 * @param {unknown} dataUri
 * @returns {boolean}
 */
function isValidAudioDataUri(dataUri) {
  if (typeof dataUri !== "string") return false;
  const trimmed = dataUri.trim();
  if (!trimmed.startsWith("data:audio/")) return false;
  const commaIdx = trimmed.indexOf(",");
  if (commaIdx === -1) return false;
  const payload = trimmed.slice(commaIdx + 1).trim();
  if (payload.length === 0) return false;
  const header = trimmed.slice(0, commaIdx).toLowerCase();
  const byteLength = header.includes(";base64")
    ? Math.floor((payload.replace(/=+$/, "").length * 3) / 4)
    : payload.length;
  return byteLength > 0 && byteLength <= MAX_SOUNDBOARD_BYTES;
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
 * Проверяет допустимость идентификатора вкладки `ADVANCED_SETTINGS_TABS_RU`.
 *
 * @param {unknown} tabId
 * @returns {tabId is AdvancedSettingsTabId}
 */
function isValidAdvancedTabId(tabId) {
  return (
    typeof tabId === "string" &&
    ADVANCED_SETTINGS_TABS_RU.some((entry) => entry.id === tabId)
  );
}

/**
 * Нормализует список пользовательских горячих клавиш.
 *
 * @param {ReadonlyArray<Partial<CustomKeybindItem>> | undefined} rawList
 * @returns {CustomKeybindItem[]}
 */
function normalizeKeybindsList(rawList) {
  if (!Array.isArray(rawList)) return [];
  /** @type {CustomKeybindItem[]} */
  const out = [];
  let seq = 1;
  for (const item of rawList) {
    if (!item || typeof item !== "object") continue;
    const id =
      typeof item.id === "string" && item.id.trim().length > 0
        ? item.id.trim()
        : `kb-${seq++}`;
    const action = isValidKeybindAction(item.action) ? item.action : "UNASSIGNED";
    const shortcut = typeof item.shortcut === "string" ? item.shortcut.trim() : "";
    const enabled = item.enabled !== false;
    const managed = item.managed === true;
    out.push({ id, action, shortcut, enabled, managed });
  }
  return out;
}

/**
 * Нормализует состояние настроек звуковой панели.
 *
 * @param {Partial<SoundboardSettingsState> | undefined} raw
 * @returns {SoundboardSettingsState}
 */
function normalizeSoundboardState(raw) {
  const src = raw && typeof raw === "object" ? raw : {};
  const volumePercent = clampSoundboardVolumePercent(src.volumePercent);
  const selectedGuildId =
    typeof src.selectedGuildId === "string" && src.selectedGuildId.trim().length > 0
      ? src.selectedGuildId.trim()
      : GLOBAL_ENTRANCE_GUILD_ID;

  /** @type {Record<string, string>} */
  const entranceSounds = {};
  if (src.entranceSounds && typeof src.entranceSounds === "object") {
    for (const [k, v] of Object.entries(src.entranceSounds)) {
      if (typeof v === "string" && v.trim().length > 0) {
        entranceSounds[k] = v.trim();
      }
    }
  }

  /** @type {SoundboardSoundOption[]} */
  const sounds = [];
  if (Array.isArray(src.sounds)) {
    for (const s of src.sounds) {
      if (!s || typeof s !== "object" || !s.soundId) continue;
      sounds.push({
        soundId: String(s.soundId),
        name: typeof s.name === "string" ? s.name : String(s.name ?? ""),
        emojiName: typeof s.emojiName === "string" ? s.emojiName : "",
        available: s.available !== false,
      });
    }
  }

  return {
    volumePercent,
    selectedGuildId,
    entranceSounds,
    sounds,
  };
}

/**
 * Нормализует состояние настроек клипов поверх `DEFAULT_CLIPS_SETTINGS`.
 *
 * @param {Partial<ClipsSettingsState> | undefined} raw
 * @returns {ClipsSettingsState}
 */
function normalizeClipsState(raw) {
  const src = raw && typeof raw === "object" ? raw : {};
  const storageCheck = validateClipsStorageLocation(src.storageLocation);
  return {
    clipsEnabled: typeof src.clipsEnabled === "boolean" ? src.clipsEnabled : DEFAULT_CLIPS_SETTINGS.clipsEnabled,
    clipsLength: normalizeClipLengthSeconds(src.clipsLength),
    resolution: isValidClipResolution(src.resolution)
      ? src.resolution
      : DEFAULT_CLIPS_SETTINGS.resolution,
    frameRate: isValidClipFrameRate(src.frameRate)
      ? src.frameRate
      : DEFAULT_CLIPS_SETTINGS.frameRate,
    bitratePercent: clampClipBitratePercent(src.bitratePercent),
    voiceAudioEnabled:
      typeof src.voiceAudioEnabled === "boolean"
        ? src.voiceAudioEnabled
        : DEFAULT_CLIPS_SETTINGS.voiceAudioEnabled,
    remindersEnabled:
      typeof src.remindersEnabled === "boolean"
        ? src.remindersEnabled
        : DEFAULT_CLIPS_SETTINGS.remindersEnabled,
    clipKeybind:
      typeof src.clipKeybind === "string" && src.clipKeybind.trim().length > 0
        ? src.clipKeybind.trim()
        : DEFAULT_CLIPS_SETTINGS.clipKeybind,
    storageLocation: storageCheck.valid
      ? storageCheck.normalized
      : DEFAULT_CLIPS_SETTINGS.storageLocation,
    hardwareBelowMinimum: src.hardwareBelowMinimum === true,
  };
}

/**
 * Создаёт контроллер расширенных настроек (Горячие клавиши, Звуковая панель, Видеонарезки).
 *
 * @param {AdvancedSettingsControllerOptions} options
 */
export function createAdvancedSettingsController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);
  const storage = options.storage ?? null;

  /** @type {AdvancedSettingsTabId} */
  let activeTab = isValidAdvancedTabId(options.initialTab)
    ? options.initialTab
    : "keybinds";

  /** @type {CustomKeybindItem[]} */
  let keybinds = normalizeKeybindsList(options.keybinds);
  /** @type {SoundboardSettingsState} */
  let soundboard = normalizeSoundboardState(options.soundboard);
  /** @type {ClipsSettingsState} */
  let clips = normalizeClipsState(options.clips);

  // Гидратация из storage, если передан
  if (storage && typeof storage.getItem === "function") {
    try {
      const rawJson = storage.getItem(ADVANCED_SETTINGS_STORAGE_KEY);
      if (typeof rawJson === "string" && rawJson.length > 0) {
        const parsed = JSON.parse(rawJson);
        if (parsed && typeof parsed === "object") {
          if (options.keybinds === undefined && Array.isArray(parsed.keybinds)) {
            keybinds = normalizeKeybindsList(parsed.keybinds);
          }
          if (options.soundboard === undefined && parsed.soundboard) {
            soundboard = normalizeSoundboardState(parsed.soundboard);
          }
          if (options.clips === undefined && parsed.clips) {
            clips = normalizeClipsState(parsed.clips);
          }
        }
      }
    } catch {
      // Игнорируем повреждённый JSON или недоступность storage — работаем с дефолтами
    }
  }

  /** @type {ReadonlyArray<GuildOptionItem>} */
  const guilds = Array.isArray(options.guilds) ? options.guilds : [];

  /** @type {string | null} */
  let recordingKeybindId = null;
  /** @type {string} */
  let soundboardUploadError = "";
  /** @type {string} */
  let soundboardDraftName = "";
  /** @type {string} */
  let soundboardDraftUri = "";
  let keybindSeq = keybinds.length + 1;
  let destroyed = false;

  const root = doc.createElement("div");
  root.className = "advanced-settings";
  root.setAttribute("data-advanced-settings", "");
  root.setAttribute("data-active-tab", activeTab);

  /**
   * Сохраняет текущее состояние в `storage`, если он передан.
   */
  function persistState() {
    if (!storage || typeof storage.setItem !== "function") return;
    try {
      storage.setItem(
        ADVANCED_SETTINGS_STORAGE_KEY,
        JSON.stringify({
          keybinds,
          soundboard,
          clips,
        }),
      );
    } catch {
      // Ошибка квоты storage не должна блокировать работу UI
    }
  }

  /**
   * Возвращает глубокую копию состояния контроллера.
   */
  function getState() {
    return {
      activeTab,
      recordingKeybindId,
      keybinds: keybinds.map((k) => ({ ...k })),
      soundboard: {
        volumePercent: soundboard.volumePercent,
        selectedGuildId: soundboard.selectedGuildId,
        entranceSounds: { ...soundboard.entranceSounds },
        sounds: soundboard.sounds.map((s) => ({ ...s })),
      },
      clips: { ...clips },
    };
  }

  /**
   * Строит секцию вкладки «Горячие клавиши» (`keybinds`).
   */
  function buildKeybindsSection() {
    const section = doc.createElement("section");
    section.className = "advanced-settings-section";
    section.setAttribute("data-advanced-section", "keybinds");
    section.setAttribute("role", "tabpanel");
    section.setAttribute("aria-label", "Горячие клавиши");
    if (activeTab !== "keybinds") {
      section.hidden = true;
      section.setAttribute("hidden", "");
    }

    const header = doc.createElement("div");
    header.className = "advanced-settings-header";

    const titleWrap = doc.createElement("div");
    const title = doc.createElement("h2");
    title.className = "advanced-settings-title";
    title.textContent = "Пользовательские горячие клавиши";
    const subtitle = doc.createElement("p");
    subtitle.className = "advanced-settings-subtitle";
    subtitle.textContent =
      "Горячие клавиши — это пользовательские сочетания клавиш, которые позволяют вам выполнять определённые действия в Discord без необходимости заходить в меню.";
    replaceElementChildren(titleWrap, [title, subtitle]);

    const addBtn = doc.createElement("button");
    addBtn.className = "advanced-btn advanced-btn--primary";
    addBtn.setAttribute("type", "button");
    addBtn.setAttribute("data-keybind-add", "");
    addBtn.textContent = "Добавить горячие клавиши";
    addBtn.addEventListener("click", () => {
      if (destroyed) return;
      addKeybind();
    });

    replaceElementChildren(header, [titleWrap, addBtn]);

    const notice = doc.createElement("div");
    notice.className = "advanced-settings-notice";
    notice.setAttribute("data-keybinds-notice", "");
    notice.textContent = "Когда открыта эта страница, горячие клавиши отключены.";

    /** @type {any[]} */
    const sectionNodes = [header, notice];

    if (keybinds.length === 0) {
      const emptyEl = doc.createElement("div");
      emptyEl.className = "keybinds-empty";
      emptyEl.setAttribute("data-keybinds-empty", "");
      emptyEl.textContent =
        "Вы ещё не добавили ни одной пользовательской горячей клавиши.";
      sectionNodes.push(emptyEl);
    } else {
      const listEl = doc.createElement("div");
      listEl.className = "keybinds-list";
      listEl.setAttribute("data-keybinds-list", "");

      /** @type {any[]} */
      const rowNodes = [];
      for (const item of keybinds) {
        const row = doc.createElement("div");
        row.className = "keybind-row";
        row.setAttribute("data-keybind-row", item.id);

        const controls = doc.createElement("div");
        controls.className = "keybind-row-controls";

        // Селект действия
        const actionSelect = doc.createElement("select");
        actionSelect.className = "advanced-select";
        actionSelect.setAttribute("data-keybind-action-select", item.id);
        actionSelect.setAttribute("aria-label", "Действие горячих клавиш");
        actionSelect.disabled = Boolean(item.managed);
        if (item.managed) {
          actionSelect.setAttribute("disabled", "");
        }

        /** @type {any[]} */
        const optionEls = [];
        for (const actionEntry of KEYBIND_ACTION_TYPES) {
          const opt = doc.createElement("option");
          opt.value = actionEntry.value;
          opt.textContent = actionEntry.labelRu;
          if (actionEntry.value === item.action) {
            opt.selected = true;
          }
          optionEls.push(opt);
        }
        replaceElementChildren(actionSelect, optionEls);
        actionSelect.value = item.action;
        actionSelect.addEventListener("change", (/** @type {any} */ e) => {
          if (destroyed) return;
          const nextAction = e?.target?.value ?? actionSelect.value;
          updateKeybind(item.id, { action: String(nextAction) });
        });

        // Индикатор текущего комбо
        const comboBadge = doc.createElement("span");
        comboBadge.className = "keybind-combo-badge";
        comboBadge.setAttribute("data-keybind-combo", item.id);
        comboBadge.setAttribute(
          "aria-keyshortcuts",
          formatShortcutAriaLabelRu(item.shortcut),
        );
        comboBadge.textContent =
          item.shortcut.length > 0 ? item.shortcut : "Горячие клавиши не назначены";

        // Кнопка записи комбо
        const isRecording = recordingKeybindId === item.id;
        const recordBtn = doc.createElement("button");
        recordBtn.className = "keybind-record-btn";
        recordBtn.setAttribute("type", "button");
        recordBtn.setAttribute("data-keybind-record-btn", item.id);
        recordBtn.setAttribute("data-recording", isRecording ? "true" : "false");
        recordBtn.textContent = isRecording
          ? "Сохранить"
          : item.shortcut.length === 0
            ? "Установить горячие клавиши"
            : "Изменить горячие клавиши";
        recordBtn.addEventListener("click", () => {
          if (destroyed) return;
          if (recordingKeybindId === item.id) {
            stopRecordingKeybind();
          } else {
            startRecordingKeybind(item.id);
          }
        });

        // Переключатель активности
        const toggleBtn = doc.createElement("button");
        toggleBtn.className = "advanced-switch";
        toggleBtn.setAttribute("type", "button");
        toggleBtn.setAttribute("role", "switch");
        toggleBtn.setAttribute("data-keybind-toggle", item.id);
        toggleBtn.setAttribute("aria-checked", item.enabled ? "true" : "false");
        const actionLabel = getKeybindActionLabelRu(item.action);
        toggleBtn.setAttribute(
          "aria-label",
          item.enabled
            ? `Отключить горячие клавиши для команды «${actionLabel}»`
            : `Включить горячие клавиши для команды «${actionLabel}»`,
        );
        toggleBtn.textContent = item.enabled ? "Вкл." : "Выкл.";
        toggleBtn.addEventListener("click", () => {
          if (destroyed) return;
          updateKeybind(item.id, { enabled: !item.enabled });
        });

        /** @type {any[]} */
        const controlItems = [actionSelect, comboBadge, recordBtn, toggleBtn];

        // Кнопка удаления (только для не-managed биндов)
        if (!item.managed) {
          const deleteBtn = doc.createElement("button");
          deleteBtn.className = "advanced-btn advanced-btn--danger";
          deleteBtn.setAttribute("type", "button");
          deleteBtn.setAttribute("data-keybind-delete", item.id);
          deleteBtn.setAttribute("aria-label", "Удалить горячие клавиши");
          deleteBtn.textContent = "Удалить";
          deleteBtn.addEventListener("click", () => {
            if (destroyed) return;
            removeKeybind(item.id);
          });
          controlItems.push(deleteBtn);
        }

        replaceElementChildren(controls, controlItems);

        const descEl = doc.createElement("p");
        descEl.className = "keybind-description";
        descEl.textContent = getKeybindActionDescriptionRu(item.action);

        /** @type {any[]} */
        const rowChildren = [controls, descEl];

        // Предупреждение о конфликте
        const conflict = detectKeybindConflicts(keybinds, item.shortcut, item.id);
        if (conflict.hasConflict) {
          const conflictEl = doc.createElement("div");
          conflictEl.className = "advanced-settings-warning";
          conflictEl.setAttribute("data-keybind-conflict", item.id);
          conflictEl.setAttribute("role", "status");
          conflictEl.textContent = conflict.messageRu;
          rowChildren.push(conflictEl);
        }

        replaceElementChildren(row, rowChildren);
        rowNodes.push(row);
      }

      replaceElementChildren(listEl, rowNodes);
      sectionNodes.push(listEl);
    }

    // Блок горячих клавиш по умолчанию
    const defaultTitle = doc.createElement("h3");
    defaultTitle.className = "advanced-settings-title";
    defaultTitle.textContent = "Горячие клавиши по умолчанию";

    const defaultList = doc.createElement("ul");
    defaultList.className = "default-keybinds-list";
    defaultList.setAttribute("data-default-keybinds-list", "");
    /** @type {any[]} */
    const defaultItems = [];
    for (const [key, combo] of Object.entries(DEFAULT_SHORTCUTS)) {
      const li = doc.createElement("li");
      li.textContent = `${combo} — ${key}`;
      defaultItems.push(li);
    }
    replaceElementChildren(defaultList, defaultItems);
    sectionNodes.push(defaultTitle, defaultList);

    replaceElementChildren(section, sectionNodes);
    return section;
  }

  /**
   * Строит секцию вкладки «Звуковая панель» (`soundboard`).
   */
  function buildSoundboardSection() {
    const section = doc.createElement("section");
    section.className = "advanced-settings-section";
    section.setAttribute("data-advanced-section", "soundboard");
    section.setAttribute("role", "tabpanel");
    section.setAttribute("aria-label", "Звуковая панель");
    if (activeTab !== "soundboard") {
      section.hidden = true;
      section.setAttribute("hidden", "");
    }

    const title = doc.createElement("h2");
    title.className = "advanced-settings-title";
    title.textContent = "Звуковая панель";

    // Ползунок громкости звуковой панели (0..100%)
    const volumeRow = doc.createElement("div");
    volumeRow.className = "soundboard-volume-row";

    const volumeLabel = doc.createElement("label");
    volumeLabel.textContent = "Громкость звуковой панели";

    const volumeSlider = doc.createElement("input");
    volumeSlider.className = "soundboard-slider";
    volumeSlider.setAttribute("type", "range");
    volumeSlider.setAttribute("role", "slider");
    volumeSlider.setAttribute("data-soundboard-volume-slider", "");
    volumeSlider.setAttribute("min", String(MIN_SOUNDBOARD_VOLUME_PERCENT));
    volumeSlider.setAttribute("max", String(MAX_SOUNDBOARD_VOLUME_PERCENT));
    volumeSlider.setAttribute("aria-label", "Громкость звуковой панели");
    volumeSlider.setAttribute("aria-valuenow", String(soundboard.volumePercent));
    volumeSlider.value = String(soundboard.volumePercent);

    const volumeValue = doc.createElement("span");
    volumeValue.setAttribute("data-soundboard-volume-value", "");
    volumeValue.textContent = `${soundboard.volumePercent}%`;

    const onVolumeInput = (/** @type {any} */ e) => {
      if (destroyed) return;
      const raw = Number(e?.target?.value ?? volumeSlider.value);
      setSoundboardVolume(raw);
    };
    volumeSlider.addEventListener("input", onVolumeInput);
    volumeSlider.addEventListener("change", onVolumeInput);

    replaceElementChildren(volumeRow, [volumeLabel, volumeSlider, volumeValue]);

    // Блок «Звуки присоединения» (Entrance Sounds)
    const entranceTitle = doc.createElement("h3");
    entranceTitle.className = "advanced-settings-title";
    entranceTitle.textContent = "Звуки присоединения";

    const entranceDesc = doc.createElement("p");
    entranceDesc.className = "advanced-settings-subtitle";
    entranceDesc.textContent =
      "Выберите звук на звуковой панели, который будет воспроизводиться при присоединении к голосовому каналу.";

    const guildRow = doc.createElement("div");
    guildRow.className = "soundboard-volume-row";

    const guildSelect = doc.createElement("select");
    guildSelect.className = "advanced-select";
    guildSelect.setAttribute("data-entrance-guild-select", "");
    guildSelect.setAttribute("aria-label", "Выберите сервер");

    const allOption = doc.createElement("option");
    allOption.value = GLOBAL_ENTRANCE_GUILD_ID;
    allOption.textContent = "Все серверы";
    if (soundboard.selectedGuildId === GLOBAL_ENTRANCE_GUILD_ID) {
      allOption.selected = true;
    }
    /** @type {any[]} */
    const guildOptions = [allOption];
    for (const g of guilds) {
      const opt = doc.createElement("option");
      opt.value = g.id;
      opt.textContent = g.name;
      if (soundboard.selectedGuildId === g.id) {
        opt.selected = true;
      }
      guildOptions.push(opt);
    }
    replaceElementChildren(guildSelect, guildOptions);
    guildSelect.value = soundboard.selectedGuildId;
    guildSelect.addEventListener("change", (/** @type {any} */ e) => {
      if (destroyed) return;
      const nextGuild = String(e?.target?.value ?? guildSelect.value ?? GLOBAL_ENTRANCE_GUILD_ID);
      soundboard.selectedGuildId = nextGuild;
      persistState();
      render();
    });

    const activeSoundId = resolveEntranceSoundForGuild(
      soundboard.entranceSounds,
      soundboard.selectedGuildId,
    );
    const activeSoundObj = soundboard.sounds.find((s) => s.soundId === activeSoundId);

    const currentSoundBadge = doc.createElement("span");
    currentSoundBadge.setAttribute("data-entrance-current-sound", "");
    currentSoundBadge.textContent = activeSoundObj ? activeSoundObj.name : "Пусто";

    const clearSoundBtn = doc.createElement("button");
    clearSoundBtn.className = "advanced-btn";
    clearSoundBtn.setAttribute("type", "button");
    clearSoundBtn.setAttribute("data-entrance-sound-clear", "");
    clearSoundBtn.textContent = "Пусто";
    clearSoundBtn.addEventListener("click", () => {
      if (destroyed) return;
      setEntranceSound(soundboard.selectedGuildId, null);
    });

    replaceElementChildren(guildRow, [guildSelect, currentSoundBadge, clearSoundBtn]);

    const soundsGrid = doc.createElement("div");
    soundsGrid.className = "soundboard-entrance-grid";
    /** @type {any[]} */
    const soundItemNodes = [];
    for (const s of soundboard.sounds) {
      if (s.available === false) continue;
      const itemWrap = doc.createElement("div");

      const chooseBtn = doc.createElement("button");
      chooseBtn.className = "soundboard-sound-item";
      chooseBtn.setAttribute("type", "button");
      chooseBtn.setAttribute("data-entrance-sound-option", s.soundId);
      chooseBtn.setAttribute(
        "aria-pressed",
        activeSoundId === s.soundId ? "true" : "false",
      );
      const prefix = s.emojiName ? `${s.emojiName} ` : "";
      chooseBtn.textContent = `${prefix}${s.name}`;
      chooseBtn.addEventListener("click", () => {
        if (destroyed) return;
        setEntranceSound(soundboard.selectedGuildId, s.soundId);
      });

      const previewBtn = doc.createElement("button");
      previewBtn.className = "advanced-btn";
      previewBtn.setAttribute("type", "button");
      previewBtn.setAttribute("data-soundboard-preview-btn", s.soundId);
      previewBtn.textContent = "Послушать звук";
      previewBtn.addEventListener("click", () => {
        if (destroyed) return;
        previewSound(s.soundId);
      });

      replaceElementChildren(itemWrap, [chooseBtn, previewBtn]);
      soundItemNodes.push(itemWrap);
    }
    replaceElementChildren(soundsGrid, soundItemNodes);

    // Форма загрузки пользовательского звука на звуковую панель
    const uploadForm = doc.createElement("div");
    uploadForm.className = "soundboard-volume-row";
    uploadForm.setAttribute("data-soundboard-upload-form", "");

    const nameInput = doc.createElement("input");
    nameInput.className = "advanced-input";
    nameInput.setAttribute("type", "text");
    nameInput.setAttribute("data-soundboard-name-input", "");
    nameInput.setAttribute("placeholder", "Название звука");
    nameInput.value = soundboardDraftName;
    nameInput.addEventListener("input", (/** @type {any} */ e) => {
      soundboardDraftName = String(e?.target?.value ?? nameInput.value ?? "");
    });

    const uriInput = doc.createElement("input");
    uriInput.className = "advanced-input";
    uriInput.setAttribute("type", "text");
    uriInput.setAttribute("data-soundboard-datauri-input", "");
    uriInput.setAttribute("placeholder", "data:audio/...");
    uriInput.value = soundboardDraftUri;
    uriInput.addEventListener("input", (/** @type {any} */ e) => {
      soundboardDraftUri = String(e?.target?.value ?? uriInput.value ?? "");
    });

    const uploadBtn = doc.createElement("button");
    uploadBtn.className = "advanced-btn advanced-btn--primary";
    uploadBtn.setAttribute("type", "button");
    uploadBtn.setAttribute("data-soundboard-upload-btn", "");
    uploadBtn.textContent = "Добавить звук на звуковую панель";
    uploadBtn.addEventListener("click", () => {
      if (destroyed) return;
      const rawName = typeof nameInput.value === "string" ? nameInput.value : soundboardDraftName;
      const rawUri = typeof uriInput.value === "string" ? uriInput.value : soundboardDraftUri;
      if (!isValidSoundboardName(rawName)) {
        soundboardUploadError = INVALID_SOUNDBOARD_NAME_ERROR_RU;
        render();
        return;
      }
      if (!isValidAudioDataUri(rawUri)) {
        soundboardUploadError = INVALID_SOUNDBOARD_URI_ERROR_RU;
        render();
        return;
      }
      soundboardUploadError = "";
      soundboardDraftName = "";
      soundboardDraftUri = "";
      if (typeof options.onUploadSound === "function") {
        options.onUploadSound({
          name: rawName.trim(),
          soundDataUri: rawUri.trim(),
          volume: soundboardPercentToFloat(soundboard.volumePercent),
        });
      }
      render();
    });

    const errorEl = doc.createElement("div");
    errorEl.className = "advanced-settings-error";
    errorEl.setAttribute("data-soundboard-error", "");
    errorEl.setAttribute("role", "alert");
    errorEl.textContent = soundboardUploadError;

    replaceElementChildren(uploadForm, [nameInput, uriInput, uploadBtn, errorEl]);

    replaceElementChildren(section, [
      title,
      volumeRow,
      entranceTitle,
      entranceDesc,
      guildRow,
      soundsGrid,
      uploadForm,
    ]);
    return section;
  }

  /**
   * Строит секцию вкладки «Видеонарезки» (`clips`).
   */
  function buildClipsSection() {
    const section = doc.createElement("section");
    section.className = "advanced-settings-section";
    section.setAttribute("data-advanced-section", "clips");
    section.setAttribute("role", "tabpanel");
    section.setAttribute("aria-label", "Настройки клипов");
    if (activeTab !== "clips") {
      section.hidden = true;
      section.setAttribute("hidden", "");
    }

    const title = doc.createElement("h2");
    title.className = "advanced-settings-title";
    title.textContent = "Настройки клипов";

    /** @type {any[]} */
    const nodes = [title];

    if (clips.hardwareBelowMinimum) {
      const hwWarn = doc.createElement("div");
      hwWarn.className = "advanced-settings-warning";
      hwWarn.setAttribute("data-clips-hardware-warning", "");
      hwWarn.setAttribute("role", "alert");
      hwWarn.textContent = HARDWARE_WARNING_RU;
      nodes.push(hwWarn);
    }

    // 1. Включить создание клипов
    const enableRow = doc.createElement("div");
    enableRow.className = "clips-setting-row";
    const enableLabel = doc.createElement("span");
    enableLabel.textContent = "Включить создание клипов";
    const enableSwitch = doc.createElement("button");
    enableSwitch.className = "advanced-switch";
    enableSwitch.setAttribute("type", "button");
    enableSwitch.setAttribute("role", "switch");
    enableSwitch.setAttribute("data-clips-enabled-switch", "");
    enableSwitch.setAttribute("aria-label", "Включить создание клипов");
    enableSwitch.setAttribute("aria-checked", clips.clipsEnabled ? "true" : "false");
    enableSwitch.textContent = clips.clipsEnabled ? "Вкл." : "Выкл.";
    enableSwitch.addEventListener("click", () => {
      if (destroyed) return;
      setClipsEnabled(!clips.clipsEnabled);
    });
    replaceElementChildren(enableRow, [enableLabel, enableSwitch]);
    nodes.push(enableRow);

    const isDisabled = !clips.clipsEnabled;

    // 2. Горячая клавиша для создания клипов
    const keybindRow = doc.createElement("div");
    keybindRow.className = "clips-setting-row";
    const keybindLabel = doc.createElement("span");
    keybindLabel.textContent = "Горячая клавиша для создания клипов";
    const keybindValue = doc.createElement("span");
    keybindValue.className = "keybind-combo-badge";
    keybindValue.setAttribute("data-clips-keybind-value", "");
    keybindValue.textContent = clips.clipKeybind;
    const resetKeybindBtn = doc.createElement("button");
    resetKeybindBtn.className = "advanced-btn";
    resetKeybindBtn.setAttribute("type", "button");
    resetKeybindBtn.setAttribute("data-clips-keybind-reset", "");
    resetKeybindBtn.textContent = "Сброс";
    resetKeybindBtn.disabled = isDisabled;
    if (isDisabled) {
      resetKeybindBtn.setAttribute("disabled", "");
    }
    resetKeybindBtn.addEventListener("click", () => {
      if (destroyed) return;
      resetClipsKeybind();
    });
    replaceElementChildren(keybindRow, [keybindLabel, keybindValue, resetKeybindBtn]);
    nodes.push(keybindRow);

    // 3. Продолжительность клипа (30 / 60 / 120 сек.)
    const lengthRow = doc.createElement("div");
    lengthRow.className = "clips-setting-row";
    const lengthLabel = doc.createElement("label");
    lengthLabel.textContent = "Продолжительность клипа";
    const lengthSelect = doc.createElement("select");
    lengthSelect.className = "advanced-select";
    lengthSelect.setAttribute("data-clips-length-select", "");
    lengthSelect.setAttribute("aria-label", "Продолжительность клипа");
    lengthSelect.disabled = isDisabled;
    if (isDisabled) {
      lengthSelect.setAttribute("disabled", "");
    }
    /** @type {any[]} */
    const lengthOpts = [];
    for (const sec of CLIPS_LENGTH_SECONDS) {
      const opt = doc.createElement("option");
      opt.value = String(sec);
      opt.textContent = formatClipLengthLabelRu(sec);
      if (sec === clips.clipsLength) opt.selected = true;
      lengthOpts.push(opt);
    }
    replaceElementChildren(lengthSelect, lengthOpts);
    lengthSelect.value = String(clips.clipsLength);
    lengthSelect.addEventListener("change", (/** @type {any} */ e) => {
      if (destroyed) return;
      const nextLen = Number(e?.target?.value ?? lengthSelect.value);
      updateClipsSettings({ clipsLength: nextLen });
    });
    replaceElementChildren(lengthRow, [lengthLabel, lengthSelect]);
    nodes.push(lengthRow);

    // 4. Разрешение клипа (480p / 720p / 1080p / 1440p / Источник)
    const resRow = doc.createElement("div");
    resRow.className = "clips-setting-row";
    const resLabel = doc.createElement("label");
    resLabel.textContent = "Разрешение клипа";
    const resSelect = doc.createElement("select");
    resSelect.className = "advanced-select";
    resSelect.setAttribute("data-clips-resolution-select", "");
    resSelect.setAttribute("aria-label", "Разрешение клипа");
    resSelect.disabled = isDisabled;
    if (isDisabled) {
      resSelect.setAttribute("disabled", "");
    }
    /** @type {any[]} */
    const resOpts = [];
    for (const res of CLIPS_RESOLUTION_OPTIONS) {
      const opt = doc.createElement("option");
      opt.value = String(res);
      opt.textContent = formatClipResolutionLabelRu(res);
      if (res === clips.resolution) opt.selected = true;
      resOpts.push(opt);
    }
    replaceElementChildren(resSelect, resOpts);
    resSelect.value = String(clips.resolution);
    resSelect.addEventListener("change", (/** @type {any} */ e) => {
      if (destroyed) return;
      const nextRes = Number(e?.target?.value ?? resSelect.value);
      updateClipsSettings({ resolution: nextRes });
    });
    replaceElementChildren(resRow, [resLabel, resSelect]);
    nodes.push(resRow);

    // 5. Частота кадров клипа (15 / 30 / 60 FPS)
    const fpsRow = doc.createElement("div");
    fpsRow.className = "clips-setting-row";
    const fpsLabel = doc.createElement("label");
    fpsLabel.textContent = "Частота кадров клипа";
    const fpsSelect = doc.createElement("select");
    fpsSelect.className = "advanced-select";
    fpsSelect.setAttribute("data-clips-fps-select", "");
    fpsSelect.setAttribute("aria-label", "Частота кадров клипа");
    fpsSelect.disabled = isDisabled;
    if (isDisabled) {
      fpsSelect.setAttribute("disabled", "");
    }
    /** @type {any[]} */
    const fpsOpts = [];
    for (const fps of CLIPS_FPS_OPTIONS) {
      const opt = doc.createElement("option");
      opt.value = String(fps);
      opt.textContent = formatClipFrameRateLabelRu(fps);
      if (fps === clips.frameRate) opt.selected = true;
      fpsOpts.push(opt);
    }
    replaceElementChildren(fpsSelect, fpsOpts);
    fpsSelect.value = String(clips.frameRate);
    fpsSelect.addEventListener("change", (/** @type {any} */ e) => {
      if (destroyed) return;
      const nextFps = Number(e?.target?.value ?? fpsSelect.value);
      updateClipsSettings({ frameRate: nextFps });
    });
    replaceElementChildren(fpsRow, [fpsLabel, fpsSelect]);
    nodes.push(fpsRow);

    // 6. Качество клипов (битрейт 0..100%)
    const bitrateRow = doc.createElement("div");
    bitrateRow.className = "clips-setting-row";
    const bitrateLabel = doc.createElement("label");
    bitrateLabel.textContent = "Качество клипов";
    const bitrateSlider = doc.createElement("input");
    bitrateSlider.className = "soundboard-slider";
    bitrateSlider.setAttribute("type", "range");
    bitrateSlider.setAttribute("role", "slider");
    bitrateSlider.setAttribute("data-clips-bitrate-slider", "");
    bitrateSlider.setAttribute("min", "0");
    bitrateSlider.setAttribute("max", "100");
    bitrateSlider.setAttribute("aria-label", "Качество клипов");
    bitrateSlider.setAttribute("aria-valuenow", String(clips.bitratePercent));
    bitrateSlider.value = String(clips.bitratePercent);
    bitrateSlider.disabled = isDisabled;
    if (isDisabled) {
      bitrateSlider.setAttribute("disabled", "");
    }
    const bitrateValue = doc.createElement("span");
    bitrateValue.setAttribute("data-clips-bitrate-value", "");
    bitrateValue.textContent = `${clips.bitratePercent}%`;
    const onBitrateInput = (/** @type {any} */ e) => {
      if (destroyed) return;
      const nextBitrate = Number(e?.target?.value ?? bitrateSlider.value);
      updateClipsSettings({ bitratePercent: nextBitrate });
    };
    bitrateSlider.addEventListener("input", onBitrateInput);
    bitrateSlider.addEventListener("change", onBitrateInput);
    replaceElementChildren(bitrateRow, [bitrateLabel, bitrateSlider, bitrateValue]);
    nodes.push(bitrateRow);

    // 7. Разрешить запись моего голоса в клипах (Privacy)
    const voiceRow = doc.createElement("div");
    voiceRow.className = "clips-setting-row";
    const voiceLabel = doc.createElement("span");
    voiceLabel.textContent = "Разрешить запись моего голоса в клипах";
    const voiceSwitch = doc.createElement("button");
    voiceSwitch.className = "advanced-switch";
    voiceSwitch.setAttribute("type", "button");
    voiceSwitch.setAttribute("role", "switch");
    voiceSwitch.setAttribute("data-clips-voice-switch", "");
    voiceSwitch.setAttribute("aria-label", "Разрешить запись моего голоса в клипах");
    voiceSwitch.setAttribute("aria-checked", clips.voiceAudioEnabled ? "true" : "false");
    voiceSwitch.textContent = clips.voiceAudioEnabled ? "Вкл." : "Выкл.";
    voiceSwitch.addEventListener("click", () => {
      if (destroyed) return;
      updateClipsSettings({ voiceAudioEnabled: !clips.voiceAudioEnabled });
    });
    replaceElementChildren(voiceRow, [voiceLabel, voiceSwitch]);
    nodes.push(voiceRow);

    // 8. Включить напоминания
    const remindersRow = doc.createElement("div");
    remindersRow.className = "clips-setting-row";
    const remindersLabel = doc.createElement("span");
    remindersLabel.textContent = "Включить напоминания";
    const remindersSwitch = doc.createElement("button");
    remindersSwitch.className = "advanced-switch";
    remindersSwitch.setAttribute("type", "button");
    remindersSwitch.setAttribute("role", "switch");
    remindersSwitch.setAttribute("data-clips-reminders-switch", "");
    remindersSwitch.setAttribute("aria-label", "Включить напоминания");
    remindersSwitch.setAttribute("aria-checked", clips.remindersEnabled ? "true" : "false");
    remindersSwitch.textContent = clips.remindersEnabled ? "Вкл." : "Выкл.";
    remindersSwitch.disabled = isDisabled;
    if (isDisabled) {
      remindersSwitch.setAttribute("disabled", "");
    }
    remindersSwitch.addEventListener("click", () => {
      if (destroyed) return;
      updateClipsSettings({ remindersEnabled: !clips.remindersEnabled });
    });
    replaceElementChildren(remindersRow, [remindersLabel, remindersSwitch]);
    nodes.push(remindersRow);

    // 9. Место хранения клипов
    const storageRow = doc.createElement("div");
    storageRow.className = "clips-setting-row";
    const storageLabel = doc.createElement("label");
    storageLabel.textContent = "Место хранения";
    const storageInput = doc.createElement("input");
    storageInput.className = "advanced-input";
    storageInput.setAttribute("type", "text");
    storageInput.setAttribute("data-clips-storage-input", "");
    storageInput.setAttribute("aria-label", "Место хранения");
    storageInput.value = clips.storageLocation;
    storageInput.disabled = isDisabled;
    if (isDisabled) {
      storageInput.setAttribute("disabled", "");
    }
    storageInput.addEventListener("change", (/** @type {any} */ e) => {
      if (destroyed) return;
      const nextLoc = String(e?.target?.value ?? storageInput.value ?? "");
      updateClipsSettings({ storageLocation: nextLoc });
    });
    replaceElementChildren(storageRow, [storageLabel, storageInput]);
    nodes.push(storageRow);

    replaceElementChildren(section, nodes);
    return section;
  }

  /**
   * Полная синхронная перерисовка компонента внутри `root`.
   */
  function render() {
    if (destroyed) return;
    root.setAttribute("data-active-tab", activeTab);

    const tabsNav = doc.createElement("div");
    tabsNav.className = "advanced-settings-tabs";
    tabsNav.setAttribute("data-advanced-tabs", "");
    tabsNav.setAttribute("role", "tablist");
    tabsNav.setAttribute("aria-label", "Разделы расширенных настроек");

    /** @type {any[]} */
    const tabBtns = [];
    for (const entry of ADVANCED_SETTINGS_TABS_RU) {
      const btn = doc.createElement("button");
      btn.className = "advanced-settings-tab";
      btn.setAttribute("type", "button");
      btn.setAttribute("role", "tab");
      btn.setAttribute("data-advanced-tab", entry.id);
      const isSelected = entry.id === activeTab;
      btn.setAttribute("aria-selected", isSelected ? "true" : "false");
      btn.textContent = entry.labelRu;
      btn.addEventListener("click", () => {
        if (destroyed) return;
        setTab(entry.id);
      });
      tabBtns.push(btn);
    }
    replaceElementChildren(tabsNav, tabBtns);

    const keybindsSec = buildKeybindsSection();
    const soundboardSec = buildSoundboardSection();
    const clipsSec = buildClipsSection();

    replaceElementChildren(root, [tabsNav, keybindsSec, soundboardSec, clipsSec]);
  }

  /**
   * Переключает активную вкладку (`keybinds` | `soundboard` | `clips`).
   *
   * @param {string} tabId
   * @returns {boolean}
   */
  function setTab(tabId) {
    if (destroyed || !isValidAdvancedTabId(tabId)) {
      return false;
    }
    activeTab = tabId;
    recordingKeybindId = null;
    render();
    return true;
  }

  /**
   * Добавляет новую пользовательскую горячую клавишу.
   *
   * @param {Partial<CustomKeybindItem>} [initial]
   * @returns {CustomKeybindItem}
   */
  function addKeybind(initial = {}) {
    const id =
      typeof initial.id === "string" && initial.id.trim().length > 0
        ? initial.id.trim()
        : `kb-${keybindSeq++}`;
    const action = isValidKeybindAction(initial.action) ? initial.action : "UNASSIGNED";
    const shortcut = typeof initial.shortcut === "string" ? initial.shortcut.trim() : "";
    const enabled = initial.enabled !== false;
    const managed = initial.managed === true;

    /** @type {CustomKeybindItem} */
    const created = { id, action, shortcut, enabled, managed };
    keybinds = [...keybinds, created];
    persistState();
    render();
    if (typeof options.onKeybindsChange === "function") {
      options.onKeybindsChange(keybinds.map((k) => ({ ...k })));
    }
    return { ...created };
  }

  /**
   * Обновляет поля существующей горячей клавиши (`action`, `shortcut`, `enabled`).
   *
   * @param {string} id
   * @param {Partial<Pick<CustomKeybindItem, "action" | "shortcut" | "enabled">>} patch
   * @returns {boolean}
   */
  function updateKeybind(id, patch) {
    if (destroyed || !patch || typeof patch !== "object") return false;
    const idx = keybinds.findIndex((item) => item.id === id);
    if (idx === -1) return false;
    const current = keybinds[idx];
    if (!current) return false;

    if (patch.action !== undefined) {
      if (current.managed || !isValidKeybindAction(patch.action)) {
        return false;
      }
    }

    /** @type {CustomKeybindItem} */
    const updated = {
      ...current,
      action: patch.action !== undefined ? patch.action : current.action,
      shortcut:
        patch.shortcut !== undefined ? String(patch.shortcut).trim() : current.shortcut,
      enabled:
        typeof patch.enabled === "boolean" ? patch.enabled : current.enabled,
    };

    keybinds = keybinds.map((item, i) => (i === idx ? updated : item));
    persistState();
    render();
    if (typeof options.onKeybindsChange === "function") {
      options.onKeybindsChange(keybinds.map((k) => ({ ...k })));
    }
    return true;
  }

  /**
   * Удаляет пользовательскую горячую клавишу (системные `managed: true` удалить нельзя).
   *
   * @param {string} id
   * @returns {boolean}
   */
  function removeKeybind(id) {
    if (destroyed) return false;
    const target = keybinds.find((item) => item.id === id);
    if (!target || target.managed) {
      return false;
    }
    keybinds = keybinds.filter((item) => item.id !== id);
    if (recordingKeybindId === id) {
      recordingKeybindId = null;
    }
    persistState();
    render();
    if (typeof options.onKeybindsChange === "function") {
      options.onKeybindsChange(keybinds.map((k) => ({ ...k })));
    }
    return true;
  }

  /**
   * Включает режим записи сочетания клавиш для указанного `id`.
   *
   * @param {string} id
   * @returns {boolean}
   */
  function startRecordingKeybind(id) {
    if (destroyed) return false;
    const exists = keybinds.some((item) => item.id === id);
    if (!exists) return false;
    recordingKeybindId = id;
    render();
    return true;
  }

  /**
   * Останавливает активный режим записи сочетания клавиш.
   */
  function stopRecordingKeybind() {
    if (destroyed) return;
    recordingKeybindId = null;
    render();
  }

  /**
   * Обрабатывает событие нажатия клавиши в режиме записи горячей клавиши.
   * Нажатие `Escape` без модификаторов отменяет запись без перезаписи прежнего бинда.
   *
   * @param {import("./keybinds.js").KeyEventLike | null | undefined} event
   * @returns {boolean}
   */
  function handleKeyRecordEvent(event) {
    if (destroyed || !recordingKeybindId || !event) {
      return false;
    }
    if (
      event.key === "Escape" &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      !event.shiftKey
    ) {
      recordingKeybindId = null;
      render();
      return true;
    }
    const combo = normalizeKeyCombo(event);
    if (!combo) {
      return false;
    }
    const targetId = recordingKeybindId;
    recordingKeybindId = null;
    return updateKeybind(targetId, { shortcut: combo });
  }

  /**
   * Устанавливает громкость звуковой панели (`0..100%`).
   *
   * @param {unknown} percent
   * @returns {number}
   */
  function setSoundboardVolume(percent) {
    const nextPercent = clampSoundboardVolumePercent(percent);
    soundboard = {
      ...soundboard,
      volumePercent: nextPercent,
    };
    persistState();
    render();
    if (typeof options.onSoundboardChange === "function") {
      options.onSoundboardChange(getState().soundboard);
    }
    return nextPercent;
  }

  /**
   * Назначает или очищает звук присоединения для сервера `guildId` (или `"all"`).
   *
   * @param {string} guildId
   * @param {string | null} soundId
   */
  function setEntranceSound(guildId, soundId) {
    const targetGuild =
      typeof guildId === "string" && guildId.trim().length > 0
        ? guildId.trim()
        : GLOBAL_ENTRANCE_GUILD_ID;
    const nextMap = { ...soundboard.entranceSounds };
    if (typeof soundId === "string" && soundId.trim().length > 0) {
      nextMap[targetGuild] = soundId.trim();
    } else {
      delete nextMap[targetGuild];
    }
    soundboard = {
      ...soundboard,
      entranceSounds: nextMap,
    };
    persistState();
    render();
    if (typeof options.onSoundboardChange === "function") {
      options.onSoundboardChange(getState().soundboard);
    }
  }

  /**
   * Возвращает эффективный звук присоединения для `guildId` (с учётом fallback на `"all"`).
   *
   * @param {string} guildId
   * @returns {string | null}
   */
  function getEntranceSoundForGuild(guildId) {
    return resolveEntranceSoundForGuild(soundboard.entranceSounds, guildId);
  }

  /**
   * Проигрывает предпросмотр звука с текущей громкостью звуковой панели (`0..1`).
   *
   * @param {string} soundId
   */
  function previewSound(soundId) {
    if (destroyed || typeof options.onPlaySound !== "function") return;
    options.onPlaySound(soundId, soundboardPercentToFloat(soundboard.volumePercent));
  }

  /**
   * Включает или выключает запись клипов.
   *
   * @param {boolean} enabled
   * @returns {boolean}
   */
  function setClipsEnabled(enabled) {
    return updateClipsSettings({ clipsEnabled: Boolean(enabled) });
  }

  /**
   * Частично обновляет настройки клипов с валидацией всех параметров.
   *
   * @param {Partial<ClipsSettingsState>} patch
   * @returns {boolean}
   */
  function updateClipsSettings(patch) {
    if (destroyed || !patch || typeof patch !== "object") return false;

    const next = { ...clips };

    if (patch.clipsEnabled !== undefined) {
      if (typeof patch.clipsEnabled !== "boolean") return false;
      next.clipsEnabled = patch.clipsEnabled;
    }
    if (patch.clipsLength !== undefined) {
      const normalizedLen = normalizeClipLengthSeconds(patch.clipsLength);
      if (
        patch.clipsLength !== normalizedLen &&
        patch.clipsLength !== normalizedLen * 1000
      ) {
        return false;
      }
      next.clipsLength = normalizedLen;
    }
    if (patch.resolution !== undefined) {
      if (!isValidClipResolution(patch.resolution)) return false;
      next.resolution = patch.resolution;
    }
    if (patch.frameRate !== undefined) {
      if (!isValidClipFrameRate(patch.frameRate)) return false;
      next.frameRate = patch.frameRate;
    }
    if (patch.bitratePercent !== undefined) {
      if (typeof patch.bitratePercent !== "number" || Number.isNaN(patch.bitratePercent)) {
        return false;
      }
      next.bitratePercent = clampClipBitratePercent(patch.bitratePercent);
    }
    if (patch.voiceAudioEnabled !== undefined) {
      if (typeof patch.voiceAudioEnabled !== "boolean") return false;
      next.voiceAudioEnabled = patch.voiceAudioEnabled;
    }
    if (patch.remindersEnabled !== undefined) {
      if (typeof patch.remindersEnabled !== "boolean") return false;
      next.remindersEnabled = patch.remindersEnabled;
    }
    if (patch.clipKeybind !== undefined) {
      if (typeof patch.clipKeybind !== "string" || patch.clipKeybind.trim().length === 0) {
        return false;
      }
      next.clipKeybind = patch.clipKeybind.trim();
    }
    if (patch.storageLocation !== undefined) {
      const check = validateClipsStorageLocation(patch.storageLocation);
      if (!check.valid) return false;
      next.storageLocation = check.normalized;
    }
    if (patch.hardwareBelowMinimum !== undefined) {
      next.hardwareBelowMinimum = Boolean(patch.hardwareBelowMinimum);
    }

    clips = next;
    persistState();
    render();
    if (typeof options.onClipsChange === "function") {
      options.onClipsChange({ ...clips });
    }
    return true;
  }

  /**
   * Сбрасывает горячую клавишу создания клипа к значению по умолчанию (`Alt+C`).
   */
  function resetClipsKeybind() {
    return updateClipsSettings({ clipKeybind: DEFAULT_CLIPS_KEYBIND });
  }

  /**
   * Сбрасывает все расширенные настройки к значениям по умолчанию.
   */
  function resetDefaults() {
    recordingKeybindId = null;
    soundboardUploadError = "";
    keybinds = [];
    soundboard = normalizeSoundboardState({
      volumePercent: DEFAULT_SOUNDBOARD_VOLUME_PERCENT,
      selectedGuildId: GLOBAL_ENTRANCE_GUILD_ID,
      entranceSounds: {},
      sounds: soundboard.sounds,
    });
    clips = normalizeClipsState(DEFAULT_CLIPS_SETTINGS);
    persistState();
    render();
    return getState();
  }

  /**
   * Демонтирует контроллер из DOM-контейнера.
   */
  function destroy() {
    if (destroyed) return;
    destroyed = true;
    recordingKeybindId = null;
    if (typeof root.remove === "function") {
      root.remove();
    } else if (root.parentNode && typeof root.parentNode.removeChild === "function") {
      root.parentNode.removeChild(root);
    }
  }

  render();
  replaceElementChildren(container, [root]);

  return {
    root,
    getState,
    setTab,
    addKeybind,
    updateKeybind,
    removeKeybind,
    startRecordingKeybind,
    stopRecordingKeybind,
    handleKeyRecordEvent,
    setSoundboardVolume,
    setEntranceSound,
    getEntranceSoundForGuild,
    previewSound,
    setClipsEnabled,
    updateClipsSettings,
    resetClipsKeybind,
    resetDefaults,
    destroy,
    dispose: destroy,
  };
}
