// @ts-check
/**
 * TASK-628 (G2 Frontend): модульные тесты расширенных настроек (Advanced Settings UI):
 * Горячие клавиши (Keybinds), Звуковая панель (Soundboard) и Видеонарезки (Clips)
 * (`rust/gui-spike/ui/features/advanced-settings.js`, `rust/gui-spike/ui/styles/advanced-settings.css`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/decoded/d8b0ffc3255e8113_0` (`@152000..170000` Keybinds,
 *   `@198479` Soundboard Volume & Entrance Sounds, `@516500..525500` Clips Settings);
 * - `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0` (модуль `274372` ClipsStore,
 *   модуль `696016` `SECONDS_30`/`MINUTES_1`/`MINUTES_2`, `Ot = "alt+c"`,
 *   модуль `753070` `RESOLUTION_480..1440/SOURCE`, `FPS_15/30/60`, модуль `994778` keybind labels);
 * - `/home/sanya/re-deep/decoded/fe99f9e2d77f4c0b_0` (модуль `226640` опции длины/разрешения/FPS клипов);
 * - `/home/sanya/re-deep/decoded/e2135c73a7dbde68_0` (`@2363` Keybind Recorder `RECORDING` / `DEFAULT`);
 * - `/home/sanya/re-deep/ui8/23-settings-voice.json` и `/home/sanya/re-deep/ru-catalog.json`;
 * - `rust/gui-spike/ui/tokens.css` (100% привязка CSS к дизайн-токенам без сырых цветов).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";

import {
  ADVANCED_SETTINGS_STORAGE_KEY,
  ADVANCED_SETTINGS_TABS_RU,
  CLIPS_FPS_OPTIONS,
  CLIPS_LENGTH_SECONDS,
  CLIPS_RESOLUTION_OPTIONS,
  DEFAULT_CLIPS_KEYBIND,
  DEFAULT_CLIPS_SETTINGS,
  GLOBAL_ENTRANCE_GUILD_ID,
  KEYBIND_ACTION_TYPES,
  MAX_SOUNDBOARD_VOLUME_PERCENT,
  MIN_SOUNDBOARD_VOLUME_PERCENT,
  clampClipBitratePercent,
  clampSoundboardVolumePercent,
  createAdvancedSettingsController,
  detectKeybindConflicts,
  formatClipFrameRateLabelRu,
  formatClipLengthLabelRu,
  formatClipResolutionLabelRu,
  getKeybindActionDescriptionRu,
  getKeybindActionLabelRu,
  isValidClipFrameRate,
  isValidClipResolution,
  isValidKeybindAction,
  normalizeClipLengthSeconds,
  resolveEntranceSoundForGuild,
  soundboardPercentToFloat,
  validateClipsStorageLocation,
} from "../../rust/gui-spike/ui/features/advanced-settings.js";

const ADVANCED_SETTINGS_JS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/features/advanced-settings.js", import.meta.url),
);
const ADVANCED_SETTINGS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/advanced-settings.css", import.meta.url),
);
const TOKENS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
);

/**
 * Создаёт изолированный DOM-контейнер на базе ShimDocument.
 * @returns {{ doc: ShimDocument, container: any }}
 */
function createTestHarness() {
  const doc = new ShimDocument();
  const container = /** @type {any} */ (doc.createElement("div"));
  doc.body.appendChild(container);
  return { doc, container };
}

describe("TASK-628: Чистые функции и константы (Keybinds, Soundboard, Clips)", () => {
  it("экспортирует вкладки ADVANCED_SETTINGS_TABS_RU и справочник KEYBIND_ACTION_TYPES по RE-оракулу", () => {
    assert.equal(Object.isFrozen(ADVANCED_SETTINGS_TABS_RU), true);
    assert.deepEqual(
      ADVANCED_SETTINGS_TABS_RU.map((t) => ({ id: t.id, labelRu: t.labelRu })),
      [
        { id: "keybinds", labelRu: "Горячие клавиши" },
        { id: "soundboard", labelRu: "Звуковая панель" },
        { id: "clips", labelRu: "Видеонарезки" },
      ],
    );

    assert.equal(Object.isFrozen(KEYBIND_ACTION_TYPES), true);
    assert.equal(isValidKeybindAction("UNASSIGNED"), true);
    assert.equal(isValidKeybindAction("PUSH_TO_TALK"), true);
    assert.equal(isValidKeybindAction("TOGGLE_MUTE"), true);
    assert.equal(isValidKeybindAction("TOGGLE_DEAFEN"), true);
    assert.equal(isValidKeybindAction("SOUNDBOARD"), true);
    assert.equal(isValidKeybindAction("SOUNDBOARD_HOLD"), true);
    assert.equal(isValidKeybindAction("SAVE_CLIP"), true);
    assert.equal(isValidKeybindAction("UNKNOWN_ACTION"), false);
    assert.equal(isValidKeybindAction(null), false);

    assert.equal(getKeybindActionLabelRu("UNASSIGNED"), "Не назначено");
    assert.equal(getKeybindActionLabelRu("PUSH_TO_TALK"), "Рация (обычный режим)");
    assert.equal(getKeybindActionLabelRu("TOGGLE_MUTE"), "Вкл./выкл. микрофон");
    assert.equal(getKeybindActionLabelRu("SOUNDBOARD"), "Открыть звуковую панель");
    assert.equal(getKeybindActionLabelRu("SAVE_CLIP"), "Сохранить клип");
    assert.ok(
      getKeybindActionDescriptionRu("SAVE_CLIP").includes("30 секунд"),
      "описание SAVE_CLIP должно соответствовать RE-ключу z3Wbam",
    );
  });

  it("detectKeybindConflicts определяет конфликты со встроенными и пользовательскими горячими клавишами", () => {
    const customList = [
      { id: "kb-1", action: "TOGGLE_MUTE", shortcut: "Ctrl+Shift+M", enabled: true },
      { id: "kb-2", action: "SOUNDBOARD", shortcut: "Alt+S", enabled: true },
      { id: "kb-3", action: "SAVE_CLIP", shortcut: "Alt+S", enabled: true },
      { id: "kb-4", action: "PUSH_TO_TALK", shortcut: "Alt+S", enabled: false },
    ];

    const builtInConflict = detectKeybindConflicts(customList, "Ctrl+K", "kb-1");
    assert.equal(builtInConflict.hasBuiltInConflict, true);
    assert.equal(builtInConflict.builtInAction, "QUICK_SWITCHER");

    const customConflict = detectKeybindConflicts(customList, "Alt+S", "kb-2");
    assert.equal(customConflict.hasCustomConflict, true);
    assert.deepEqual(customConflict.conflictingIds, ["kb-3"]);
    assert.equal(customConflict.hasConflict, true);

    const noConflict = detectKeybindConflicts(customList, "Ctrl+Alt+P", "kb-2");
    assert.equal(noConflict.hasBuiltInConflict, false);
    assert.equal(noConflict.hasCustomConflict, false);
    assert.equal(noConflict.hasConflict, false);
  });

  it("нормализует громкость звуковой панели (0..100% и 0..1) и разрешает звуки присоединения с fallback", () => {
    assert.equal(MIN_SOUNDBOARD_VOLUME_PERCENT, 0);
    assert.equal(MAX_SOUNDBOARD_VOLUME_PERCENT, 100);
    assert.equal(GLOBAL_ENTRANCE_GUILD_ID, "all");

    assert.equal(clampSoundboardVolumePercent(-15), 0);
    assert.equal(clampSoundboardVolumePercent(0), 0);
    assert.equal(clampSoundboardVolumePercent(26.4), 26);
    assert.equal(clampSoundboardVolumePercent(100), 100);
    assert.equal(clampSoundboardVolumePercent(180), 100);
    assert.equal(clampSoundboardVolumePercent(Number.NaN), 100);
    assert.equal(clampSoundboardVolumePercent("invalid"), 100);

    assert.equal(soundboardPercentToFloat(0), 0);
    assert.equal(soundboardPercentToFloat(50), 0.5);
    assert.equal(soundboardPercentToFloat(100), 1);

    const entranceMap = {
      all: "snd-global",
      "guild-1": "snd-guild-1",
    };
    assert.equal(resolveEntranceSoundForGuild(entranceMap, "guild-1"), "snd-guild-1");
    assert.equal(resolveEntranceSoundForGuild(entranceMap, "guild-2"), "snd-global");
    assert.equal(resolveEntranceSoundForGuild(entranceMap, "all"), "snd-global");
    assert.equal(resolveEntranceSoundForGuild({}, "guild-1"), null);
  });

  it("валидирует и форматирует параметры клипов (длительность, разрешение, FPS, битрейт, путь хранения)", () => {
    assert.deepEqual(CLIPS_LENGTH_SECONDS, [30, 60, 120]);
    assert.deepEqual(CLIPS_RESOLUTION_OPTIONS, [480, 720, 1080, 1440, 0]);
    assert.deepEqual(CLIPS_FPS_OPTIONS, [15, 30, 60]);
    assert.equal(DEFAULT_CLIPS_KEYBIND, "Alt+C");
    assert.equal(DEFAULT_CLIPS_SETTINGS.clipsEnabled, false);
    assert.equal(DEFAULT_CLIPS_SETTINGS.clipsLength, 30);
    assert.equal(DEFAULT_CLIPS_SETTINGS.resolution, 1080);
    assert.equal(DEFAULT_CLIPS_SETTINGS.frameRate, 30);

    // Поддержка секунд (30/60/120) и миллисекунд из RE (30000/60000/120000)
    assert.equal(normalizeClipLengthSeconds(30), 30);
    assert.equal(normalizeClipLengthSeconds(60), 60);
    assert.equal(normalizeClipLengthSeconds(120), 120);
    assert.equal(normalizeClipLengthSeconds(30000), 30);
    assert.equal(normalizeClipLengthSeconds(60000), 60);
    assert.equal(normalizeClipLengthSeconds(120000), 120);
    assert.equal(normalizeClipLengthSeconds(45), 30);
    assert.equal(normalizeClipLengthSeconds("invalid"), 30);

    assert.equal(formatClipLengthLabelRu(30), "30 сек.");
    assert.equal(formatClipLengthLabelRu(60), "1 мин.");
    assert.equal(formatClipLengthLabelRu(120), "2 мин.");

    assert.equal(isValidClipResolution(1080), true);
    assert.equal(isValidClipResolution(0), true);
    assert.equal(isValidClipResolution(999), false);
    assert.equal(formatClipResolutionLabelRu(1080), "1080p");
    assert.equal(formatClipResolutionLabelRu(0), "Источник");

    assert.equal(isValidClipFrameRate(15), true);
    assert.equal(isValidClipFrameRate(30), true);
    assert.equal(isValidClipFrameRate(60), true);
    assert.equal(isValidClipFrameRate(45), false);
    assert.equal(formatClipFrameRateLabelRu(60), "60 кадров в секунду");

    assert.equal(clampClipBitratePercent(-5), 0);
    assert.equal(clampClipBitratePercent(75), 75);
    assert.equal(clampClipBitratePercent(140), 100);
    assert.equal(clampClipBitratePercent(Number.NaN), 75);

    assert.equal(validateClipsStorageLocation("/home/user/Videos/Discord Clips").valid, true);
    assert.equal(validateClipsStorageLocation("Discord Clips").valid, true);
    assert.equal(validateClipsStorageLocation("").valid, false);
    assert.equal(validateClipsStorageLocation("   ").valid, false);
    assert.equal(validateClipsStorageLocation("../secret").valid, false);
    assert.equal(validateClipsStorageLocation("clips\0bad").valid, false);
  });
});

describe("TASK-628: Интерактивный UI-контроллер расширенных настроек — Горячие клавиши (Keybinds)", () => {
  it("рендерит пустое состояние, добавляет горячие клавиши, записывает комбо с клавиатуры и переключает активность", () => {
    const { container } = createTestHarness();
    /** @type {any[]} */
    const keybindChanges = [];

    const controller = createAdvancedSettingsController({
      container,
      initialTab: "keybinds",
      onKeybindsChange: (next) => keybindChanges.push(next),
    });

    const root = container.querySelector("[data-advanced-settings]");
    assert.ok(root, "корневой элемент [data-advanced-settings] должен быть смонтирован");
    assert.equal(root.getAttribute("data-active-tab"), "keybinds");

    const emptyEl = container.querySelector("[data-keybinds-empty]");
    assert.ok(emptyEl, "при пустом списке должен отображаться [data-keybinds-empty]");
    assert.equal(
      emptyEl.textContent,
      "Вы ещё не добавили ни одной пользовательской горячей клавиши.",
    );

    // Нажимаем кнопку «Добавить горячие клавиши»
    const addBtn = container.querySelector("[data-keybind-add]");
    assert.ok(addBtn, "кнопка [data-keybind-add] должна присутствовать");
    assert.equal(addBtn.textContent, "Добавить горячие клавиши");
    addBtn.dispatchEvent("click");

    assert.equal(keybindChanges.length, 1);
    const createdId = keybindChanges[0][0].id;
    assert.ok(createdId);
    assert.equal(container.querySelector("[data-keybinds-empty]"), null);

    const row = container.querySelector(`[data-keybind-row="${createdId}"]`);
    assert.ok(row, "строка новой горячей клавиши должна появиться в DOM");

    // Выбираем действие TOGGLE_MUTE
    const actionSelect = container.querySelector(`[data-keybind-action-select="${createdId}"]`);
    assert.ok(actionSelect);
    actionSelect.value = "TOGGLE_MUTE";
    actionSelect.dispatchEvent("change");
    assert.equal(controller.getState().keybinds[0]?.action, "TOGGLE_MUTE");

    // Запускаем запись горячей клавиши
    const recordBtn = container.querySelector(`[data-keybind-record-btn="${createdId}"]`);
    assert.ok(recordBtn);
    assert.equal(recordBtn.textContent, "Установить горячие клавиши");
    recordBtn.dispatchEvent("click");

    assert.equal(controller.getState().recordingKeybindId, createdId);
    const recordBtnActive = container.querySelector(`[data-keybind-record-btn="${createdId}"]`);
    assert.equal(recordBtnActive.getAttribute("data-recording"), "true");
    assert.equal(recordBtnActive.textContent, "Сохранить");

    // Нажатие одного модификатора игнорируется
    controller.handleKeyRecordEvent({ key: "Control", ctrlKey: true });
    assert.equal(controller.getState().recordingKeybindId, createdId);

    // Нажатие валидного комбо Ctrl+Shift+M сохраняет бинд и завершает запись
    controller.handleKeyRecordEvent({
      key: "m",
      ctrlKey: true,
      shiftKey: true,
    });
    assert.equal(controller.getState().recordingKeybindId, null);
    assert.equal(controller.getState().keybinds[0]?.shortcut, "Ctrl+Shift+M");

    const comboBadge = container.querySelector(`[data-keybind-combo="${createdId}"]`);
    assert.ok(comboBadge);
    assert.equal(comboBadge.textContent, "Ctrl+Shift+M");

    // Конфликт со встроенным TOGGLE_MUTE (Ctrl+Shift+M) отображается в [data-keybind-conflict]
    const conflictEl = container.querySelector(`[data-keybind-conflict="${createdId}"]`);
    assert.ok(conflictEl, "предупреждение о конфликте должно рендериться");
    assert.ok(conflictEl.textContent.includes("конфликтует"));

    // Переключатель активности
    const toggleBtn = container.querySelector(`[data-keybind-toggle="${createdId}"]`);
    assert.ok(toggleBtn);
    assert.equal(toggleBtn.getAttribute("aria-checked"), "true");
    toggleBtn.dispatchEvent("click");
    assert.equal(controller.getState().keybinds[0]?.enabled, false);
    assert.equal(
      container.querySelector(`[data-keybind-toggle="${createdId}"]`).getAttribute("aria-checked"),
      "false",
    );

    // Удаление горячей клавиши
    const deleteBtn = container.querySelector(`[data-keybind-delete="${createdId}"]`);
    assert.ok(deleteBtn);
    deleteBtn.dispatchEvent("click");
    assert.equal(controller.getState().keybinds.length, 0);
    assert.ok(container.querySelector("[data-keybinds-empty]"));

    controller.destroy();
  });

  it("защищает системные (managed: true) горячие клавиши от удаления и смены действия, поддерживает отмену записи по Escape", () => {
    const { container } = createTestHarness();
    const controller = createAdvancedSettingsController({
      container,
      initialTab: "keybinds",
      keybinds: [
        {
          id: "managed-clip",
          action: "SAVE_CLIP",
          shortcut: "Alt+C",
          enabled: true,
          managed: true,
        },
      ],
    });

    assert.equal(controller.removeKeybind("managed-clip"), false);
    assert.equal(
      controller.updateKeybind("managed-clip", { action: "TOGGLE_MUTE" }),
      false,
    );
    assert.equal(container.querySelector('[data-keybind-delete="managed-clip"]'), null);

    // Начинаем запись и отменяем клавишей Escape — прежний бинд Alt+C сохраняется
    controller.startRecordingKeybind("managed-clip");
    assert.equal(controller.getState().recordingKeybindId, "managed-clip");
    controller.handleKeyRecordEvent({ key: "Escape" });
    assert.equal(controller.getState().recordingKeybindId, null);
    assert.equal(controller.getState().keybinds[0]?.shortcut, "Alt+C");

    controller.destroy();
  });
});

describe("TASK-628: Интерактивный UI-контроллер расширенных настроек — Звуковая панель (Soundboard)", () => {
  it("управляет громкостью звуковой панели, глобальными/серверными звуками присоединения и загрузкой кастомных звуков", () => {
    const { container } = createTestHarness();
    /** @type {any[]} */
    const soundboardChanges = [];
    /** @type {Array<{ soundId: string, volume: number }>} */
    const playedSounds = [];
    /** @type {any[]} */
    const uploadedSounds = [];

    const controller = createAdvancedSettingsController({
      container,
      initialTab: "soundboard",
      guilds: [
        { id: "g-1", name: "Киберспорт" },
        { id: "g-2", name: "Лаборатория" },
      ],
      soundboard: {
        volumePercent: 80,
        selectedGuildId: "all",
        entranceSounds: { all: "s-1" },
        sounds: [
          { soundId: "s-1", name: "Фанфары", emojiName: "🎺", available: true },
          { soundId: "s-2", name: "Гудок", emojiName: "📯", available: true },
        ],
      },
      onSoundboardChange: (next) => soundboardChanges.push(next),
      onPlaySound: (soundId, volume) => playedSounds.push({ soundId, volume }),
      onUploadSound: (payload) => uploadedSounds.push(payload),
    });

    const volumeSlider = container.querySelector("[data-soundboard-volume-slider]");
    const volumeValue = container.querySelector("[data-soundboard-volume-value]");
    assert.ok(volumeSlider);
    assert.ok(volumeValue);
    assert.equal(volumeValue.textContent, "80%");

    // Изменяем громкость ползунком на 26% (как в RE-дампе 23-settings-voice.json)
    volumeSlider.value = "26";
    volumeSlider.dispatchEvent("input");
    assert.equal(controller.getState().soundboard.volumePercent, 26);
    assert.equal(
      container.querySelector("[data-soundboard-volume-value]").textContent,
      "26%",
    );
    assert.ok(soundboardChanges.length >= 1);

    // Предпросмотр звука передаёт громкость в масштабе 0..1 (0.26)
    const previewBtn = container.querySelector('[data-soundboard-preview-btn="s-2"]');
    assert.ok(previewBtn);
    previewBtn.dispatchEvent("click");
    assert.deepEqual(playedSounds, [{ soundId: "s-2", volume: 0.26 }]);

    // Переключаем сервер на g-1 и назначаем серверный звук присоединения s-2
    const guildSelect = container.querySelector("[data-entrance-guild-select]");
    assert.ok(guildSelect);
    guildSelect.value = "g-1";
    guildSelect.dispatchEvent("change");

    const optionS2 = container.querySelector('[data-entrance-sound-option="s-2"]');
    assert.ok(optionS2);
    optionS2.dispatchEvent("click");

    assert.equal(controller.getEntranceSoundForGuild("g-1"), "s-2");
    assert.equal(controller.getEntranceSoundForGuild("g-2"), "s-1"); // fallback на глобальный

    // Очистка звука присоединения для g-1 («Пусто»)
    const clearBtn = container.querySelector("[data-entrance-sound-clear]");
    assert.ok(clearBtn);
    clearBtn.dispatchEvent("click");
    assert.equal(controller.getEntranceSoundForGuild("g-1"), "s-1");

    // Форма загрузки кастомного звука: невалидное имя -> ошибка, валидные данные -> вызов onUploadSound
    const nameInput = container.querySelector("[data-soundboard-name-input]");
    const uriInput = container.querySelector("[data-soundboard-datauri-input]");
    const uploadBtn = container.querySelector("[data-soundboard-upload-btn]");
    assert.ok(nameInput);
    assert.ok(uriInput);
    assert.ok(uploadBtn);

    nameInput.value = "A";
    uriInput.value = "data:audio/ogg;base64,T2dnUw==";
    uploadBtn.dispatchEvent("click");
    assert.equal(uploadedSounds.length, 0);
    assert.ok(
      container.querySelector("[data-soundboard-error]").textContent.includes("от 2 до 32"),
    );

    nameInput.value = "Победный звук";
    uriInput.value = "data:audio/ogg;base64,T2dnUw==";
    uploadBtn.dispatchEvent("click");
    assert.equal(uploadedSounds.length, 1);
    assert.deepEqual(uploadedSounds[0], {
      name: "Победный звук",
      soundDataUri: "data:audio/ogg;base64,T2dnUw==",
      volume: 0.26,
    });
    assert.equal(container.querySelector("[data-soundboard-error]").textContent, "");

    controller.destroy();
  });
});

describe("TASK-628: Интерактивный UI-контроллер расширенных настроек — Видеонарезки (Clips)", () => {
  it("управляет включением клипов, длительностью, разрешением, FPS, битрейтом, записью голоса и предупреждением об оборудовании", () => {
    const { container } = createTestHarness();
    /** @type {any[]} */
    const clipsChanges = [];

    const controller = createAdvancedSettingsController({
      container,
      initialTab: "clips",
      clips: {
        clipsEnabled: false,
        hardwareBelowMinimum: true,
      },
      onClipsChange: (next) => clipsChanges.push(next),
    });

    // Предупреждение о слабом оборудовании отображается (RE: SIxrIF)
    const hwWarn = container.querySelector("[data-clips-hardware-warning]");
    assert.ok(hwWarn, "при hardwareBelowMinimum=true должен отображаться баннер предупреждения");
    assert.ok(hwWarn.textContent.includes("минимальным требованиям"));

    // Пока клипы выключены, селекты длительности/разрешения/FPS заблокированы
    const lengthSelectDisabled = container.querySelector("[data-clips-length-select]");
    assert.ok(lengthSelectDisabled);
    assert.equal(lengthSelectDisabled.disabled, true);

    // Включаем создание клипов через тумблер [data-clips-enabled-switch]
    const enableSwitch = container.querySelector("[data-clips-enabled-switch]");
    assert.ok(enableSwitch);
    assert.equal(enableSwitch.getAttribute("aria-checked"), "false");
    enableSwitch.dispatchEvent("click");

    assert.equal(controller.getState().clips.clipsEnabled, true);
    assert.equal(
      container.querySelector("[data-clips-enabled-switch]").getAttribute("aria-checked"),
      "true",
    );
    assert.equal(container.querySelector("[data-clips-length-select]").disabled, false);

    // Меняем длительность на 60 сек. (1 мин.), разрешение на 1440p, FPS на 60
    const lengthSelect = container.querySelector("[data-clips-length-select]");
    lengthSelect.value = "60";
    lengthSelect.dispatchEvent("change");
    assert.equal(controller.getState().clips.clipsLength, 60);

    const resSelect = container.querySelector("[data-clips-resolution-select]");
    resSelect.value = "1440";
    resSelect.dispatchEvent("change");
    assert.equal(controller.getState().clips.resolution, 1440);

    const fpsSelect = container.querySelector("[data-clips-fps-select]");
    fpsSelect.value = "60";
    fpsSelect.dispatchEvent("change");
    assert.equal(controller.getState().clips.frameRate, 60);

    // Меняем битрейт (качество клипов) на 90%
    const bitrateSlider = container.querySelector("[data-clips-bitrate-slider]");
    bitrateSlider.value = "90";
    bitrateSlider.dispatchEvent("input");
    assert.equal(controller.getState().clips.bitratePercent, 90);

    // Переключаем запись голоса в клипах и напоминания
    const voiceSwitch = container.querySelector("[data-clips-voice-switch]");
    voiceSwitch.dispatchEvent("click");
    assert.equal(controller.getState().clips.voiceAudioEnabled, false);

    const remindersSwitch = container.querySelector("[data-clips-reminders-switch]");
    remindersSwitch.dispatchEvent("click");
    assert.equal(controller.getState().clips.remindersEnabled, false);

    // Изменяем и сбрасываем горячую клавишу клипа к Alt+C
    assert.equal(controller.updateClipsSettings({ clipKeybind: "Ctrl+Shift+C" }), true);
    assert.equal(controller.getState().clips.clipKeybind, "Ctrl+Shift+C");
    const resetKeybindBtn = container.querySelector("[data-clips-keybind-reset]");
    assert.ok(resetKeybindBtn);
    resetKeybindBtn.dispatchEvent("click");
    assert.equal(controller.getState().clips.clipKeybind, "Alt+C");

    // Обновление папки сохранения и защита от path traversal
    assert.equal(controller.updateClipsSettings({ storageLocation: "../bad" }), false);
    assert.equal(
      controller.updateClipsSettings({ storageLocation: "/home/sanya/Videos/Clips" }),
      true,
    );
    assert.equal(controller.getState().clips.storageLocation, "/home/sanya/Videos/Clips");
    assert.ok(clipsChanges.length >= 5);

    controller.destroy();
  });

  it("переключает вкладки, сохраняет состояние в storage и сбрасывает к значениям по умолчанию", () => {
    const { container } = createTestHarness();
    /** @type {Map<string, string>} */
    const memory = new Map();
    const storage = {
      getItem: (/** @type {string} */ k) => memory.get(k) ?? null,
      setItem: (/** @type {string} */ k, /** @type {string} */ v) => {
        memory.set(k, v);
      },
    };

    const controller = createAdvancedSettingsController({
      container,
      storage,
    });

    assert.equal(controller.getState().activeTab, "keybinds");
    assert.equal(controller.setTab("invalid-tab"), false);
    assert.equal(controller.setTab("soundboard"), true);
    assert.equal(controller.getState().activeTab, "soundboard");

    const clipTabBtn = container.querySelector('[data-advanced-tab="clips"]');
    assert.ok(clipTabBtn);
    clipTabBtn.dispatchEvent("click");
    assert.equal(controller.getState().activeTab, "clips");

    controller.setSoundboardVolume(45);
    controller.setClipsEnabled(true);
    assert.ok(memory.has(ADVANCED_SETTINGS_STORAGE_KEY));

    // Повторная гидратация из storage восстанавливает сохранённые настройки
    const { container: container2 } = createTestHarness();
    const hydrated = createAdvancedSettingsController({
      container: container2,
      storage,
    });
    assert.equal(hydrated.getState().soundboard.volumePercent, 45);
    assert.equal(hydrated.getState().clips.clipsEnabled, true);

    // Сброс к дефолтам
    const resetState = hydrated.resetDefaults();
    assert.equal(resetState.soundboard.volumePercent, 100);
    assert.equal(resetState.clips.clipsEnabled, false);
    assert.equal(resetState.keybinds.length, 0);

    controller.destroy();
    hydrated.destroy();
  });
});

describe("TASK-628: Контракт стилей advanced-settings.css и безопасность кода", () => {
  it("CSS-файл styles/advanced-settings.css использует только токены из tokens.css без сырых цветов и !important", () => {
    assert.equal(
      existsSync(ADVANCED_SETTINGS_CSS_PATH),
      true,
      "файл rust/gui-spike/ui/styles/advanced-settings.css должен существовать",
    );
    const css = readFileSync(ADVANCED_SETTINGS_CSS_PATH, "utf8");
    const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");

    for (const selector of [
      "[data-advanced-settings]",
      ".advanced-settings",
      ".advanced-settings-tabs",
      "[data-advanced-tab]",
      "[data-keybind-row]",
      "[data-keybind-record-btn]",
      "[data-soundboard-volume-slider]",
      "[data-clips-enabled-switch]",
      "[data-clips-hardware-warning]",
    ]) {
      assert.equal(
        css.includes(selector),
        true,
        `ожидался селектор ${selector} в advanced-settings.css`,
      );
    }

    assert.equal(css.includes("!important"), false, "в advanced-settings.css запрещён !important");
    assert.equal(/:has\(/i.test(css), false, "в advanced-settings.css запрещён :has()");
    assert.equal(
      /#[0-9a-fA-F]{3,8}\b/.test(css),
      false,
      "в advanced-settings.css запрещены сырые hex-цвета",
    );
    assert.equal(
      /\b(?:rgb|rgba|hsl|hsla)\s*\(/.test(css),
      false,
      "в advanced-settings.css запрещены сырые rgb/hsl-цвета",
    );
    assert.equal(
      /url\(|@import|https?:\/\//.test(css),
      false,
      "в advanced-settings.css запрещены внешние ассеты",
    );

    const declaredTokens = new Set(
      Array.from(tokensCss.matchAll(/(--[a-zA-Z0-9_-]+)\s*:/g)).map((m) => m[1]),
    );
    const usedTokens = Array.from(css.matchAll(/var\(\s*(--[a-zA-Z0-9_-]+)/g)).map((m) => m[1]);
    assert.ok(usedTokens.length >= 10, "advanced-settings.css должен использовать токены из tokens.css");
    for (const token of usedTokens) {
      assert.equal(
        declaredTokens.has(token),
        true,
        `токен ${token} не объявлен в rust/gui-spike/ui/tokens.css`,
      );
    }
  });

  it("код advanced-settings.js не вызывает запрещённые нативные/медиа API и не пишет в console", () => {
    const js = readFileSync(ADVANCED_SETTINGS_JS_PATH, "utf8");
    const forbidden = [
      "navigator.mediaDevices",
      "getUserMedia",
      "getDisplayMedia",
      "startRecordingRawSamples",
      "startLocalAudioRecording",
      "setAecDump",
      "getScreenPreviews",
      "getWindowPreviews",
    ];
    for (const needle of forbidden) {
      assert.equal(js.includes(needle), false, `advanced-settings.js содержит запрещённое «${needle}»`);
    }
    assert.equal(/console\.(log|warn|error)/.test(js), false, "advanced-settings.js пишет в console");
  });
});
