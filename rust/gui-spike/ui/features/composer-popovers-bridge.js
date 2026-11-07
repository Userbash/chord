// @ts-check
/**
 * task-270 (FE-INT-13): единый мост поповеров композера, автодополнения
 * слэш-команд и звуковой панели (`composer-popovers-bridge.js`).
 *
 * Связывает чистые функции разбора/фильтрации из:
 * - `./slash-command-picker.js` (`parseSlashDraftInput`, `filterSlashCommands`);
 * - `./soundboard-picker.js` (`filterSoundboardSounds`, `clampSoundboardVolume`).
 *
 * @module ui/features/composer-popovers-bridge
 */

import {
  parseSlashDraftInput,
  filterSlashCommands,
} from "./slash-command-picker.js";

import {
  filterSoundboardSounds,
  clampSoundboardVolume,
} from "./soundboard-picker.js";

/**
 * @typedef {"slash-command-picker"
 *   | "soundboard-picker"
 *   | "emoji-picker"
 *   | "attachment-upload"} ComposerPopoverLayerId
 */

/**
 * @typedef {import("./slash-command-picker.js").SlashCommandItem} SlashCommandItem
 * @typedef {import("./soundboard-picker.js").SoundboardSoundItem} SoundboardSoundItem
 */

/**
 * @typedef {Readonly<{
 *   activePopover: "slash-command-picker" | null,
 *   commandName: string | null,
 *   rawArgs: string
 * }>} ResolvedComposerDraftPopover
 */

/**
 * @typedef {Readonly<{
 *   activePopover: ComposerPopoverLayerId | null,
 *   commandName: string | null,
 *   rawArgs: string,
 *   volume: number
 * }>} ComposerPopoversStateSnapshot
 */

/**
 * @typedef {object} ComposerPopoversBridgeOptions
 * @property {ReadonlyArray<SoundboardSoundItem>} [sounds]
 * @property {ReadonlyArray<SlashCommandItem>} [commands]
 * @property {number} [volume]
 * @property {(cmd: SlashCommandItem) => void} [onSelectSlashCommand]
 * @property {(soundId: string, volume: number) => void} [onPlaySoundboardSound]
 * @property {(command: string, payload?: Record<string, unknown>) => unknown} [onDispatchIpc]
 */

/**
 * Создаёт копию слэш-команды с глубоким клонированием `options`, чтобы внешний код
 * не мог изменить внутреннее состояние моста через выданные элементы.
 *
 * @param {SlashCommandItem} cmd
 * @returns {SlashCommandItem}
 */
function cloneSlashCommandItem(cmd) {
  return {
    ...cmd,
    options: Array.isArray(cmd.options) ? cmd.options.map((option) => ({ ...option })) : [],
  };
}

/**
 * Канонический замороженный список поповеров композера сообщений.
 * @type {readonly ComposerPopoverLayerId[]}
 */
export const COMPOSER_POPOVER_LAYERS = Object.freeze([
  "slash-command-picker",
  "soundboard-picker",
  "emoji-picker",
  "attachment-upload",
]);

/**
 * Определяет состояние поповера слэш-команд по тексту черновика композера:
 * - использует `parseSlashDraftInput(draftText)` из `./slash-command-picker.js`;
 * - если `parseSlashDraftInput` возвращает не-null объект `{ commandName, rawArgs }`,
 *   возвращает `{ activePopover: 'slash-command-picker', commandName, rawArgs }`;
 * - иначе возвращает `{ activePopover: null, commandName: null, rawArgs: '' }`.
 *
 * @param {unknown} draftText
 * @returns {ResolvedComposerDraftPopover}
 */
export function resolveComposerDraftPopover(draftText) {
  const parsed = parseSlashDraftInput(draftText);
  if (parsed !== null) {
    return Object.freeze({
      activePopover: "slash-command-picker",
      commandName: parsed.commandName,
      rawArgs: parsed.rawArgs,
    });
  }
  return Object.freeze({
    activePopover: null,
    commandName: null,
    rawArgs: "",
  });
}

/**
 * Создаёт единый мост управления поповерами композера, слэш-командами и саундбордом.
 *
 * @param {ComposerPopoversBridgeOptions} [options]
 */
export function createComposerPopoversBridge(options = {}) {
  const safeOptions = options && typeof options === "object" ? options : {};

  /** @type {SlashCommandItem[]} */
  const commands = Array.isArray(safeOptions.commands)
    ? safeOptions.commands.map((cmd) => ({ ...cmd }))
    : [];

  /** @type {SoundboardSoundItem[]} */
  const sounds = Array.isArray(safeOptions.sounds)
    ? safeOptions.sounds.map((sound) => ({ ...sound }))
    : [];

  /** @type {ComposerPopoverLayerId | null} */
  let activePopover = null;
  /** @type {string | null} */
  let commandName = null;
  /** @type {string} */
  let rawArgs = "";
  /** @type {number} */
  let volume = clampSoundboardVolume(safeOptions.volume);
  /** @type {SlashCommandItem[]} */
  let filteredSlashCommands = filterSlashCommands(commands, "");

  /**
   * Возвращает замороженный снимок текущего состояния моста.
   *
   * @returns {ComposerPopoversStateSnapshot}
   */
  function getState() {
    return Object.freeze({
      activePopover,
      commandName,
      rawArgs,
      volume,
    });
  }

  /**
   * Обновляет состояние черновика композера:
   * если черновик начинается с валидного слэш-префикса, активирует `'slash-command-picker'`
   * и фильтрует `commands` по `commandName`.
   *
   * @param {unknown} draftText
   * @returns {ComposerPopoversStateSnapshot}
   */
  function updateDraft(draftText) {
    const resolved = resolveComposerDraftPopover(draftText);
    activePopover = resolved.activePopover;
    commandName = resolved.commandName;
    rawArgs = resolved.rawArgs;
    filteredSlashCommands = filterSlashCommands(commands, commandName ?? "");
    return getState();
  }

  /**
   * Проверяет принадлежность `popoverId` к `COMPOSER_POPOVER_LAYERS` (иначе бросает `Error`)
   * и делает указанный поповер активным.
   *
   * @param {string} popoverId
   * @returns {ComposerPopoversStateSnapshot}
   */
  function openPopover(popoverId) {
    if (
      typeof popoverId !== "string" ||
      !COMPOSER_POPOVER_LAYERS.includes(/** @type {ComposerPopoverLayerId} */ (popoverId))
    ) {
      throw new Error(`Неизвестный поповер композера: ${String(popoverId)}`);
    }
    activePopover = /** @type {ComposerPopoverLayerId} */ (popoverId);
    return getState();
  }

  /**
   * Сбрасывает активный поповер композера в `null`.
   *
   * @returns {ComposerPopoversStateSnapshot}
   */
  function closePopover() {
    activePopover = null;
    return getState();
  }

  /**
   * Возвращает отфильтрованный список слэш-команд через `filterSlashCommands`.
   *
   * @returns {SlashCommandItem[]}
   */
  function getFilteredSlashCommands() {
    return filteredSlashCommands.map((cmd) => cloneSlashCommandItem(cmd));
  }

  /**
   * Возвращает отфильтрованный список звуков саундборда через `filterSoundboardSounds`.
   *
   * @param {string} [query]
   * @returns {SoundboardSoundItem[]}
   */
  function getFilteredSoundboardSounds(query) {
    return filterSoundboardSounds(sounds, query);
  }

  /**
   * Нормализует и сохраняет громкость саундборда через `clampSoundboardVolume`.
   *
   * @param {number} vol
   * @returns {number}
   */
  function setSoundboardVolume(vol) {
    volume = clampSoundboardVolume(vol);
    return volume;
  }

  /**
   * Закрывает поповер и уведомляет `onSelectSlashCommand` о выбранной слэш-команде.
   *
   * Невалидный аргумент (не объект, отсутствующее или пустое `name`) отбрасывается:
   * возвращается `null`, колбэк не вызывается, состояние моста не меняется.
   *
   * @param {SlashCommandItem} cmd
   * @returns {SlashCommandItem | null}
   */
  function selectSlashCommand(cmd) {
    if (!cmd || typeof cmd !== "object") {
      return null;
    }
    const selectedName = typeof cmd.name === "string" ? cmd.name.trim() : "";
    if (selectedName.length === 0) {
      return null;
    }
    const selected = /** @type {SlashCommandItem} */ ({ ...cmd });
    activePopover = null;
    commandName = null;
    rawArgs = "";
    filteredSlashCommands = filterSlashCommands(commands, "");
    if (typeof safeOptions.onSelectSlashCommand === "function") {
      safeOptions.onSelectSlashCommand(selected);
    }
    if (typeof safeOptions.onDispatchIpc === "function") {
      safeOptions.onDispatchIpc("invoke_bff_domain", {
        domain: "slash_commands",
        commandName: selected.name,
      });
    }
    return selected;
  }

  /**
   * Проигрывает звук саундборда с текущей нормализованной громкостью через `onPlaySoundboardSound`.
   *
   * @param {string} soundId
   * @returns {number}
   */
  function playSoundboardSound(soundId) {
    if (typeof safeOptions.onPlaySoundboardSound === "function") {
      safeOptions.onPlaySoundboardSound(String(soundId), volume);
    }
    if (typeof safeOptions.onDispatchIpc === "function") {
      safeOptions.onDispatchIpc("invoke_bff_domain", {
        domain: "soundboard",
        soundId: String(soundId),
        volume,
      });
    }
    return volume;
  }

  return {
    updateDraft,
    openPopover,
    closePopover,
    getFilteredSlashCommands,
    getFilteredSoundboardSounds,
    setSoundboardVolume,
    getState,
    selectSlashCommand,
    playSoundboardSound,
  };
}
