// @ts-check
/**
 * V-07-UI-VOICE-DEVICES (RW-40): изолированный контроллер настроек «Голос и видео».
 *
 * Контракт-оракул:
 * - `rust/crates/discord-voice/src/devices.rs` — hot-plug с откатом на `isDefault`,
 *   события `DeviceFallback` / `NoDeviceAvailable`, инвариант
 *   `effective_mute = user_mute || self_deaf`, `threshold_db: i8`,
 *   громкости микрофона `0..=100` и вывода `0..=200`;
 * - `waves/RW-40/DESIGN/VOICE.md` §2.4, §3.1–§3.5 — mute/deafen, селекты устройств,
 *   hot-plug без автопереключения, PTT и клампинг громкости;
 * - `waves/RW-40/DESIGN/voice-spec.json` — `devices.sliders` (`OX2Bnr`, `aUJ062`),
 *   `copy` (`hHMYbb`, `dl18zb`, `tG4Np5`, `o+BJQR`, `GCNMM8`, `sWmtI6`, `hK51Yg`,
 *   `nWRdnl`).
 *
 * Границы карточки:
 * - контроллер не ходит в сеть/IPC сам: все изменения уходят в инжектированный
 *   `service` (в `app.js` его нет), а состояние живёт в памяти;
 * - медиа-API и запись звука запрещены (VOICE.md §5), имена устройств не логируются;
 * - разметка создаётся через `host.ownerDocument`, поэтому один код-путь работает
 *   и в браузере, и в DOM-шиме тестов RW-38;
 * - ARIA-контракт: `role="slider"` + `aria-valuemin|max|now|valuetext` у ползунков,
 *   `role="switch"` + `aria-checked` у тумблеров, `role="status"|"alert"` у уведомлений.
 */

/** Минимальная громкость (0%). */
export const MIN_VOLUME = 0;

/** Максимальная громкость микрофона (`OX2Bnr`: 0..=100%). */
export const MAX_INPUT_VOLUME = 100;

/** Максимальная громкость вывода (`aUJ062`: 0..=200%). */
export const MAX_OUTPUT_VOLUME = 200;

/** Громкость микрофона по умолчанию (100%). */
export const DEFAULT_INPUT_VOLUME = 100;

/** Громкость вывода по умолчанию (100%). */
export const DEFAULT_OUTPUT_VOLUME = 100;

/** Порог VAD по умолчанию (−40 дБ, `DEFAULT_VAD_THRESHOLD_DB`). */
export const DEFAULT_VAD_THRESHOLD_DB = -40;

/** Границы `threshold_db: i8` из контракта Rust. */
export const MIN_VAD_THRESHOLD_DB = -128;

/** Верхняя граница `threshold_db: i8` из контракта Rust. */
export const MAX_VAD_THRESHOLD_DB = 127;

/** Нижняя граница UI-шкалы порога чувствительности VAD (−100 дБ, VOICE-DEVICES-SETTINGS-SPEC §3.4). */
export const MIN_UI_VAD_THRESHOLD_DB = -100;

/** Верхняя граница UI-шкалы порога чувствительности VAD (0 дБ, VOICE-DEVICES-SETTINGS-SPEC §3.4). */
export const MAX_UI_VAD_THRESHOLD_DB = 0;

/** Нижняя граница индикатора проверки микрофона (−100 дБ). */
export const MIN_MIC_METER_DB = -100;

/** Верхняя граница индикатора проверки микрофона (0 дБ). */
export const MAX_MIC_METER_DB = 0;

/** `[A]` задержка отпускания PTT по умолчанию (мс): значения в дампе нет. */
export const DEFAULT_PTT_RELEASE_DELAY_MS = 20;

/** Верхняя граница `releaseDelayMs` из карточки V-07 (0..2000 мс). */
export const MAX_PTT_RELEASE_DELAY_MS = 2000;

/** Подписи `[D]` из `voice-spec.json` → `copy` и `devices`. */
const COPY = Object.freeze({
  inputTitle: "Устройство ввода",
  outputTitle: "Устройство вывода",
  videoTitle: "Камера",
  inputVolume: "Громкость микрофона",
  outputVolume: "Громкость динамика",
  ptt: "Режим рации",
  pttKey: "Горячая клавиша для режима рации",
  pttDelay: "Задержка отключения в режиме рации",
  mute: "Заглушить",
  deafen: "Откл. звук",
  noKey: "Горячие клавиши не назначены",
  micEmpty: "Микрофон не выбран",
  outputEmpty: "Устройство вывода не выбрано",
  videoEmpty: "Нет видеоустройств",
  section: "Голос и видео",
  /** `[A]`: подписи нет в срезе дампа (VG-08), берём нейтральную. */
  vadThreshold: "Порог активации по голосу",
  micTestTitle: "Проверка микрофона",
  micTestStart: "Давайте проверим",
  micTestStop: "Остановить проверку",
  micTestMeter: "Уровень сигнала микрофона",
  micTestIdle: "Нажмите «Давайте проверим», чтобы протестировать микрофон",
  micTestActive: "Проверка микрофона активна",
});

/** Клавиши-модификаторы не назначаются как PTT. */
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "AltGraph", "CapsLock"]);

/**
 * @typedef {"input" | "output"} AudioDeviceKind
 * @typedef {"input" | "output" | "video"} VoiceDeviceKind
 * @typedef {{ id: string, label: string, kind: VoiceDeviceKind, isDefault: boolean }} AudioDevice
 * @typedef {{ kind: "voice-activity", thresholdDb: number }} VoiceActivityMode
 * @typedef {{ kind: "push-to-talk", releaseDelayMs: number }} PushToTalkMode
 * @typedef {VoiceActivityMode | PushToTalkMode} VoiceInputMode
 * @typedef {{
 *   type: "DeviceFallback",
 *   kind: VoiceDeviceKind,
 *   previousId: string,
 *   fallbackId: string,
 * } | {
 *   type: "NoDeviceAvailable",
 *   kind: VoiceDeviceKind,
 *   previousId: string | null,
 * }} DeviceEvent
 */

/**
 * Сервис настроек: все методы необязательны. Ошибка синхронного вызова или
 * отклонённый промис показываются в `role="alert"`, а не глотаются.
 * @typedef {{
 *   selectInput?: (id: string) => unknown,
 *   selectOutput?: (id: string) => unknown,
 *   selectVideo?: (id: string) => unknown,
 *   setInputVolume?: (percent: number) => unknown,
 *   setOutputVolume?: (percent: number) => unknown,
 *   setInputMode?: (mode: VoiceInputMode) => unknown,
 *   setUserMute?: (mute: boolean) => unknown,
 *   setSelfDeaf?: (deaf: boolean) => unknown,
 *   setPttKey?: (key: string | null) => unknown,
 *   startMicTest?: () => unknown,
 *   stopMicTest?: () => unknown,
 * }} VoiceDevicesService
 */

/**
 * Привязка к уже существующим DOM-контролам панели настроек (`settings.js`),
 * чтобы не нарушать инварианты RW-38/RW-39 по числу `select` / `input[type=range]` /
 * `input[type=checkbox]`.
 * @typedef {{
 *   inputSelect?: HTMLSelectElement | null,
 *   outputSelect?: HTMLSelectElement | null,
 *   videoSelect?: HTMLSelectElement | null,
 *   inputVolumeRange?: HTMLInputElement | null,
 *   outputVolumeRange?: HTMLInputElement | null,
 *   pttToggle?: HTMLInputElement | null,
 *   micTestButton?: HTMLButtonElement | null,
 *   mountContainer?: HTMLElement | null,
 * }} VoiceDevicesBoundControls
 */

/**
 * @typedef {{
 *   host: HTMLElement,
 *   devices?: readonly AudioDevice[],
 *   service?: VoiceDevicesService,
 *   onDeviceEvent?: ((event: DeviceEvent) => void) | null,
 *   vadMinDb?: number,
 *   vadMaxDb?: number,
 *   controls?: VoiceDevicesBoundControls | null,
 * }} VoiceDevicesOptions
 */

/**
 * Приводит целое к диапазону; нечисловое значение оставляет прежним.
 * @param {number} value
 * @param {number} min
 * @param {number} max
 * @param {number} fallback
 * @returns {number}
 */
function clampInt(value, min, max, fallback) {
  const numeric = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(numeric)) return fallback;
  return Math.min(max, Math.max(min, Math.round(numeric)));
}

/**
 * Нормализует список устройств: пустые id отбрасываются, `kind` — `input`,
 * `output` или `video`.
 * @param {readonly AudioDevice[]} rows
 * @returns {AudioDevice[]}
 */
function normalizeDevices(rows) {
  /** @type {AudioDevice[]} */
  const list = [];
  for (const row of rows) {
    const id = String(row.id).trim();
    if (!id) continue;
    /** @type {VoiceDeviceKind} */
    const kind =
      row.kind === "output" ? "output" : row.kind === "video" ? "video" : "input";
    list.push({
      id,
      label: String(row.label),
      kind,
      isDefault: row.isDefault === true,
    });
  }
  return list;
}

/**
 * Видимое имя клавиши: пробел не читается скринридером как « ».
 * @param {string} key
 * @returns {string}
 */
function keyLabel(key) {
  if (key === " ") return "Пробел";
  return key;
}

/**
 * Контроллер настроек аудио- и видео-устройств, громкости, режима активации,
 * проверки микрофона и mute/deafen.
 * @param {VoiceDevicesOptions} options
 */
export function createVoiceDevicesSettings(options) {
  const {
    host,
    service = {},
    onDeviceEvent = null,
    vadMinDb = MIN_VAD_THRESHOLD_DB,
    vadMaxDb = MAX_VAD_THRESHOLD_DB,
    controls = null,
  } = options;
  const doc = /** @type {Document} */ (host.ownerDocument);
  const boundMode = controls !== null && typeof controls === "object";

  /** @type {AudioDevice[]} */
  let devices = normalizeDevices(options.devices ?? []);
  /** @type {string | null} */
  let selectedInput = null;
  /** @type {string | null} */
  let selectedOutput = null;
  /** @type {string | null} */
  let selectedVideo = null;
  let userMute = false;
  let selfDeaf = false;
  /** @type {"voice-activity" | "push-to-talk"} */
  let mode = "voice-activity";
  let vadThresholdDb = clampInt(DEFAULT_VAD_THRESHOLD_DB, vadMinDb, vadMaxDb, DEFAULT_VAD_THRESHOLD_DB);
  let pttReleaseDelayMs = DEFAULT_PTT_RELEASE_DELAY_MS;
  let inputVolume = DEFAULT_INPUT_VOLUME;
  let outputVolume = DEFAULT_OUTPUT_VOLUME;
  /** @type {string | null} */
  let pttKey = null;
  let capturing = false;
  let micTesting = false;
  let micTestLevelDb = MIN_MIC_METER_DB;
  let destroyed = false;

  /* ---------- разметка (без innerHTML: DOM-шим RW-38) ---------- */

  /**
   * @param {string} tag
   * @param {string} [className]
   * @param {string} [text]
   * @returns {HTMLElement}
   */
  function el(tag, className = "", text = "") {
    const node = doc.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  /**
   * @param {string} id
   * @param {string} label
   * @param {number} min
   * @param {number} max
   * @param {boolean} [useDivSlider]
   * @returns {HTMLInputElement}
   */
  function range(id, label, min, max, useDivSlider = false) {
    const node = /** @type {HTMLInputElement} */ (
      doc.createElement(useDivSlider ? "div" : "input")
    );
    node.id = id;
    node.className = "voice-devices-range";
    if (!useDivSlider) node.type = "range";
    else node.setAttribute("tabindex", "0");
    node.setAttribute("min", String(min));
    node.setAttribute("max", String(max));
    node.setAttribute("step", "1");
    node.setAttribute("role", "slider");
    node.setAttribute("aria-label", label);
    node.setAttribute("aria-orientation", "horizontal");
    node.setAttribute("aria-valuemin", String(min));
    node.setAttribute("aria-valuemax", String(max));
    return node;
  }

  /**
   * @param {string} id
   * @param {string} label
   * @param {boolean} [useButtonSwitch]
   * @returns {HTMLInputElement}
   */
  function switchControl(id, label, useButtonSwitch = false) {
    const node = /** @type {HTMLInputElement} */ (
      doc.createElement(useButtonSwitch ? "button" : "input")
    );
    node.id = id;
    node.className = "voice-devices-switch";
    if (!useButtonSwitch) {
      node.type = "checkbox";
      node.setAttribute("role", "switch");
    } else {
      node.setAttribute("type", "button");
      node.setAttribute("aria-pressed", "false");
    }
    node.setAttribute("aria-checked", "false");
    node.setAttribute("aria-label", label);
    return node;
  }

  /**
   * @param {string} id
   * @returns {HTMLElement}
   */
  function readout(id) {
    const node = el("output", "voice-devices-value");
    node.id = id;
    return node;
  }

  /**
   * @param {HTMLInputElement} node
   * @param {string} id
   * @param {string} label
   * @param {number} min
   * @param {number} max
   * @returns {HTMLInputElement}
   */
  function decorateRange(node, id, label, min, max) {
    node.id = id;
    node.setAttribute("min", String(min));
    node.setAttribute("max", String(max));
    node.setAttribute("step", "1");
    node.setAttribute("role", "slider");
    node.setAttribute("aria-label", label);
    node.setAttribute("aria-orientation", "horizontal");
    node.setAttribute("aria-valuemin", String(min));
    node.setAttribute("aria-valuemax", String(max));
    return node;
  }

  host.setAttribute("aria-label", COPY.section);

  const inputSelect =
    controls?.inputSelect ?? /** @type {HTMLSelectElement} */ (doc.createElement("select"));
  inputSelect.id = "voice-devices-input";
  if (!controls?.inputSelect) inputSelect.className = "voice-devices-select";
  inputSelect.setAttribute("aria-label", COPY.inputTitle);

  const outputSelect =
    controls?.outputSelect ?? /** @type {HTMLSelectElement} */ (doc.createElement("select"));
  outputSelect.id = "voice-devices-output";
  if (!controls?.outputSelect) outputSelect.className = "voice-devices-select";
  outputSelect.setAttribute("aria-label", COPY.outputTitle);

  const videoSelect =
    controls?.videoSelect ?? /** @type {HTMLSelectElement} */ (doc.createElement("select"));
  videoSelect.id = "voice-devices-video";
  if (!controls?.videoSelect) videoSelect.className = "voice-devices-select";
  videoSelect.setAttribute("aria-label", COPY.videoTitle);

  const inputVolumeRange = controls?.inputVolumeRange
    ? decorateRange(controls.inputVolumeRange, "voice-devices-input-volume", COPY.inputVolume, MIN_VOLUME, MAX_INPUT_VOLUME)
    : range("voice-devices-input-volume", COPY.inputVolume, MIN_VOLUME, MAX_INPUT_VOLUME);
  const outputVolumeRange = controls?.outputVolumeRange
    ? decorateRange(controls.outputVolumeRange, "voice-devices-output-volume", COPY.outputVolume, MIN_VOLUME, MAX_OUTPUT_VOLUME)
    : range("voice-devices-output-volume", COPY.outputVolume, MIN_VOLUME, MAX_OUTPUT_VOLUME);
  const inputVolumeValue = readout("voice-devices-input-volume-value");
  const outputVolumeValue = readout("voice-devices-output-volume-value");

  const micTestBox = el("div", "voice-devices-group");
  micTestBox.id = "voice-devices-mic-test";
  micTestBox.setAttribute("role", "group");
  micTestBox.setAttribute("aria-label", COPY.micTestTitle);
  const micTestButton =
    controls?.micTestButton ?? /** @type {HTMLButtonElement} */ (doc.createElement("button"));
  micTestButton.id = "voice-devices-mic-test-btn";
  if (!controls?.micTestButton) micTestButton.className = "voice-devices-mic-btn";
  micTestButton.setAttribute("type", "button");
  micTestButton.removeAttribute("aria-disabled");
  micTestButton.setAttribute("aria-pressed", "false");
  micTestButton.textContent = COPY.micTestStart;
  const micMeter = el("div", "voice-devices-mic-meter");
  micMeter.id = "voice-devices-mic-meter";
  micMeter.setAttribute("role", "meter");
  micMeter.setAttribute("aria-label", COPY.micTestMeter);
  micMeter.setAttribute("aria-valuemin", String(MIN_MIC_METER_DB));
  micMeter.setAttribute("aria-valuemax", String(MAX_MIC_METER_DB));
  micMeter.setAttribute("aria-valuenow", String(MIN_MIC_METER_DB));
  micMeter.setAttribute("aria-valuetext", `${MIN_MIC_METER_DB} дБ`);
  const micStatus = el("p", "voice-devices-mic-status", COPY.micTestIdle);
  micStatus.id = "voice-devices-mic-status";
  micStatus.setAttribute("role", "status");
  micStatus.setAttribute("aria-live", "polite");
  if (!controls?.micTestButton) {
    micTestBox.append(micTestButton, micMeter, micStatus);
  } else {
    micTestBox.append(micMeter, micStatus);
  }

  const vadSettings = el("div", "voice-devices-group");
  vadSettings.id = "voice-devices-vad-settings";
  vadSettings.setAttribute("role", "group");
  vadSettings.setAttribute("aria-label", COPY.vadThreshold);
  const vadThreshold = range(
    "voice-devices-vad-threshold",
    COPY.vadThreshold,
    vadMinDb,
    vadMaxDb,
    boundMode,
  );
  const vadThresholdValue = readout("voice-devices-vad-threshold-value");
  vadSettings.append(vadThreshold, vadThresholdValue);

  const pttSettings = el("div", "voice-devices-group");
  pttSettings.id = "voice-devices-ptt-settings";
  pttSettings.setAttribute("role", "group");
  pttSettings.setAttribute("aria-label", COPY.ptt);
  const pttDelay = range("voice-devices-ptt-delay", COPY.pttDelay, 0, MAX_PTT_RELEASE_DELAY_MS, boundMode);
  const pttDelayValue = readout("voice-devices-ptt-delay-value");
  const pttKeyButton = /** @type {HTMLButtonElement} */ (doc.createElement("button"));
  pttKeyButton.id = "voice-devices-ptt-key";
  pttKeyButton.className = "voice-devices-key";
  pttKeyButton.type = "button";
  pttKeyButton.setAttribute("aria-label", COPY.pttKey);
  pttKeyButton.setAttribute("data-capturing", "false");
  pttSettings.append(pttDelay, pttDelayValue, pttKeyButton);

  const pttToggle = controls?.pttToggle ?? switchControl("voice-devices-ptt", COPY.ptt);
  pttToggle.id = "voice-devices-ptt";
  pttToggle.setAttribute("aria-label", COPY.ptt);
  const userMuteToggle = switchControl("voice-devices-user-mute", COPY.mute, boundMode);
  const selfDeafToggle = switchControl("voice-devices-self-deaf", COPY.deafen, boundMode);

  const effectiveMuteNode = el("p", "voice-devices-effective", "Микрофон активен");
  effectiveMuteNode.id = "voice-devices-effective-mute";
  effectiveMuteNode.setAttribute("aria-live", "polite");
  effectiveMuteNode.setAttribute("data-effective-mute", "false");

  const notice = el("p", "voice-devices-notice");
  notice.id = "voice-devices-notice";
  notice.setAttribute("role", "status");
  notice.setAttribute("aria-live", "polite");
  notice.setAttribute("hidden", "");

  /**
   * @param {string} label
   * @param {HTMLElement} control
   * @returns {HTMLElement}
   */
  function field(label, control) {
    const box = el("div", "voice-devices-field");
    const caption = el("span", "voice-devices-label", label);
    box.append(caption, control);
    return box;
  }

  if (boundMode) {
    const mountContainer = controls?.mountContainer ?? host;
    mountContainer.replaceChildren(
      inputVolumeValue,
      outputVolumeValue,
      micTestBox,
      vadSettings,
      pttSettings,
      userMuteToggle,
      selfDeafToggle,
      effectiveMuteNode,
      notice,
    );
  } else {
    host.replaceChildren(
      field(COPY.inputTitle, inputSelect),
      field(COPY.outputTitle, outputSelect),
      field(COPY.videoTitle, videoSelect),
      field(COPY.inputVolume, inputVolumeRange),
      inputVolumeValue,
      field(COPY.outputVolume, outputVolumeRange),
      outputVolumeValue,
      micTestBox,
      vadSettings,
      field(COPY.ptt, pttToggle),
      pttSettings,
      field(COPY.mute, userMuteToggle),
      field(COPY.deafen, selfDeafToggle),
      effectiveMuteNode,
      notice,
    );
  }

  /* ---------- состояние и уведомления ---------- */

  const inputs = () => devices.filter((device) => device.kind === "input");
  const outputs = () => devices.filter((device) => device.kind === "output");
  const videos = () => devices.filter((device) => device.kind === "video");

  /** @returns {boolean} */
  function effectiveMute() {
    return userMute || selfDeaf;
  }

  /** @returns {VoiceInputMode} */
  function currentMode() {
    return mode === "push-to-talk"
      ? { kind: "push-to-talk", releaseDelayMs: pttReleaseDelayMs }
      : { kind: "voice-activity", thresholdDb: vadThresholdDb };
  }

  /**
   * @param {unknown} error
   * @returns {string}
   */
  function describeError(error) {
    return error instanceof Error ? error.message : String(error);
  }

  /**
   * @param {string} message
   * @param {"status" | "alert"} level
   */
  function notify(message, level) {
    if (!message) {
      notice.textContent = "";
      notice.setAttribute("role", "status");
      notice.setAttribute("aria-live", "polite");
      notice.setAttribute("hidden", "");
      return;
    }
    notice.setAttribute("role", level);
    notice.setAttribute("aria-live", level === "alert" ? "assertive" : "polite");
    notice.textContent = message;
    notice.removeAttribute("hidden");
  }

  /**
   * Вызов сервиса: синхронная ошибка и отклонённый промис дают alert.
   * @template T
   * @param {((value: T) => unknown) | undefined} fn
   * @param {T} value
   */
  function invoke(fn, value) {
    if (typeof fn !== "function") return;
    try {
      const result = fn(value);
      const pending = /** @type {{ then?: unknown, catch?: (handler: (error: unknown) => void) => unknown }} */ (result);
      if (pending !== null && typeof pending === "object" && typeof pending.then === "function" && typeof pending.catch === "function") {
        pending.catch((error) => {
          if (!destroyed) notify(`Не удалось сохранить настройку: ${describeError(error)}`, "alert");
        });
      }
    } catch (error) {
      if (!destroyed) notify(`Не удалось сохранить настройку: ${describeError(error)}`, "alert");
    }
  }

  /* ---------- устройства и hot-plug ---------- */

  /**
   * @param {HTMLSelectElement} select
   * @param {AudioDevice[]} list
   * @param {string | null} selected
   * @param {string} emptyLabel
   */
  function fillSelect(select, list, selected, emptyLabel) {
    select.replaceChildren();
    for (const device of list) {
      const option = doc.createElement("option");
      option.value = device.id;
      option.textContent = device.label;
      option.title = device.label;
      select.append(option);
    }
    if (list.length === 0) {
      const empty = doc.createElement("option");
      empty.value = "";
      empty.textContent = emptyLabel;
      select.append(empty);
      select.value = "";
      select.setAttribute("aria-disabled", "true");
      return;
    }
    select.removeAttribute("aria-disabled");
    select.value = selected ?? list[0]?.id ?? "";
  }

  function renderDevices() {
    fillSelect(inputSelect, inputs(), selectedInput, COPY.micEmpty);
    fillSelect(outputSelect, outputs(), selectedOutput, COPY.outputEmpty);
    fillSelect(videoSelect, videos(), selectedVideo, COPY.videoEmpty);
  }

  /**
   * @param {VoiceDeviceKind} kind
   * @param {string | null} previousId
   * @param {AudioDevice[]} list
   * @returns {{ selected: string | null, event: DeviceEvent | null }}
   */
  function reconcile(kind, previousId, list) {
    if (previousId !== null && list.some((device) => device.id === previousId)) {
      return { selected: previousId, event: null };
    }
    const fallback = list.find((device) => device.isDefault) ?? list[0] ?? null;
    if (!fallback) {
      return { selected: null, event: { type: "NoDeviceAvailable", kind, previousId } };
    }
    if (previousId === null) {
      return { selected: fallback.id, event: null };
    }
    return {
      selected: fallback.id,
      event: { type: "DeviceFallback", kind, previousId, fallbackId: fallback.id },
    };
  }

  /**
   * @param {DeviceEvent} event
   * @param {AudioDevice[]} previousDevices
   */
  function announceEvent(event, previousDevices) {
    if (typeof onDeviceEvent === "function") {
      try {
        onDeviceEvent(event);
      } catch (error) {
        notify(`Ошибка обработчика событий устройств: ${describeError(error)}`, "alert");
      }
    }
    if (event.type === "DeviceFallback") {
      const previous = previousDevices.find((device) => device.id === event.previousId);
      const fallback = devices.find((device) => device.id === event.fallbackId);
      notify(
        `Устройство «${previous?.label ?? event.previousId}» отключено. Выбрано «${fallback?.label ?? event.fallbackId}».`,
        "status",
      );
      return;
    }
    notify(
      event.kind === "input" ? "Нет доступных микрофонов." : "Нет доступных устройств вывода.",
      "alert",
    );
  }

  /**
   * Hot-plug: сохраняет присутствующее устройство, иначе откатывается на
   * `isDefault === true` (или первое доступное), при пустом списке — `null`.
   * @param {readonly AudioDevice[]} next
   * @returns {DeviceEvent[]}
   */
  function setDevices(next) {
    if (destroyed) return [];
    const previousDevices = devices;
    devices = normalizeDevices(next);
    const inputResult = reconcile("input", selectedInput, inputs());
    const outputResult = reconcile("output", selectedOutput, outputs());
    const videoResult = reconcile("video", selectedVideo, videos());
    selectedInput = inputResult.selected;
    selectedOutput = outputResult.selected;
    selectedVideo = videoResult.selected;
    renderDevices();
    /** @type {DeviceEvent[]} */
    const events = [];
    if (inputResult.event) events.push(inputResult.event);
    if (outputResult.event) events.push(outputResult.event);
    for (const event of events) announceEvent(event, previousDevices);
    return events;
  }

  /**
   * @param {VoiceDeviceKind} kind
   * @param {string} id
   * @returns {boolean}
   */
  function selectDevice(kind, id) {
    if (destroyed) return false;
    const device = devices.find((row) => row.kind === kind && row.id === id);
    if (!device) {
      notify("Устройство не найдено.", "alert");
      renderDevices();
      return false;
    }
    if (kind === "input") selectedInput = device.id;
    else if (kind === "output") selectedOutput = device.id;
    else selectedVideo = device.id;
    renderDevices();
    notify("", "status");
    const fn =
      kind === "input"
        ? service.selectInput
        : kind === "output"
          ? service.selectOutput
          : service.selectVideo;
    invoke(fn, device.id);
    return true;
  }

  /**
   * @param {string} id
   * @returns {boolean}
   */
  function selectInput(id) {
    return selectDevice("input", id);
  }

  /**
   * @param {string} id
   * @returns {boolean}
   */
  function selectOutput(id) {
    return selectDevice("output", id);
  }

  /**
   * @param {string} id
   * @returns {boolean}
   */
  function selectVideo(id) {
    return selectDevice("video", id);
  }

  /* ---------- громкость ---------- */

  function renderInputVolume() {
    const now = String(inputVolume);
    inputVolumeRange.value = now;
    inputVolumeRange.setAttribute("aria-valuenow", now);
    inputVolumeRange.setAttribute("aria-valuetext", `${now}%`);
    inputVolumeValue.textContent = `${now}%`;
  }

  function renderOutputVolume() {
    const now = String(outputVolume);
    outputVolumeRange.value = now;
    outputVolumeRange.setAttribute("aria-valuenow", now);
    outputVolumeRange.setAttribute("aria-valuetext", `${now}%`);
    outputVolumeValue.textContent = `${now}%`;
  }

  /**
   * @param {number} percent
   * @returns {number}
   */
  function setInputVolume(percent) {
    if (destroyed) return inputVolume;
    inputVolume = clampInt(percent, MIN_VOLUME, MAX_INPUT_VOLUME, inputVolume);
    renderInputVolume();
    invoke(service.setInputVolume, inputVolume);
    return inputVolume;
  }

  /**
   * @param {number} percent
   * @returns {number}
   */
  function setOutputVolume(percent) {
    if (destroyed) return outputVolume;
    outputVolume = clampInt(percent, MIN_VOLUME, MAX_OUTPUT_VOLUME, outputVolume);
    renderOutputVolume();
    invoke(service.setOutputVolume, outputVolume);
    return outputVolume;
  }

  /* ---------- проверка микрофона ---------- */

  function renderMicTest() {
    micTestButton.setAttribute("aria-pressed", micTesting ? "true" : "false");
    micTestButton.textContent = micTesting ? COPY.micTestStop : COPY.micTestStart;
    const level = String(micTestLevelDb);
    micMeter.setAttribute("aria-valuenow", level);
    micMeter.setAttribute("aria-valuetext", `${level} дБ`);
    micStatus.textContent = micTesting ? COPY.micTestActive : COPY.micTestIdle;
  }

  function startMicTest() {
    if (destroyed || micTesting) return;
    micTesting = true;
    renderMicTest();
    invoke(service.startMicTest, undefined);
  }

  function stopMicTest() {
    if (!micTesting) return;
    micTesting = false;
    micTestLevelDb = MIN_MIC_METER_DB;
    renderMicTest();
    invoke(service.stopMicTest, undefined);
  }

  /**
   * @param {number} db
   * @returns {number}
   */
  function setMicTestLevelDb(db) {
    if (destroyed) return micTestLevelDb;
    micTestLevelDb = clampInt(db, MIN_MIC_METER_DB, MAX_MIC_METER_DB, micTestLevelDb);
    renderMicTest();
    return micTestLevelDb;
  }

  /* ---------- режим активации и PTT ---------- */

  function renderMode() {
    const ptt = mode === "push-to-talk";
    pttToggle.checked = ptt;
    pttToggle.setAttribute("aria-checked", ptt ? "true" : "false");
    if (ptt) pttSettings.removeAttribute("hidden");
    else pttSettings.setAttribute("hidden", "");
    if (ptt) vadSettings.setAttribute("hidden", "");
    else vadSettings.removeAttribute("hidden");

    const threshold = String(vadThresholdDb);
    vadThreshold.value = threshold;
    vadThreshold.setAttribute("aria-valuenow", threshold);
    vadThreshold.setAttribute("aria-valuetext", `${threshold} дБ`);
    vadThresholdValue.textContent = `${threshold} дБ`;

    const delay = String(pttReleaseDelayMs);
    pttDelay.value = delay;
    pttDelay.setAttribute("aria-valuenow", delay);
    pttDelay.setAttribute("aria-valuetext", `${delay} мс`);
    pttDelayValue.textContent = `${delay} мс`;
    renderPttKey();
  }

  function renderPttKey() {
    pttKeyButton.setAttribute("data-capturing", capturing ? "true" : "false");
    if (capturing) {
      pttKeyButton.textContent = "Нажмите клавишу…";
      return;
    }
    pttKeyButton.textContent =
      pttKey === null ? COPY.noKey : `Клавиша: ${keyLabel(pttKey)}`;
  }

  /**
   * @param {VoiceInputMode} next
   */
  function setInputMode(next) {
    if (destroyed) return;
    if (next.kind === "push-to-talk") {
      mode = "push-to-talk";
      pttReleaseDelayMs = clampInt(
        next.releaseDelayMs,
        0,
        MAX_PTT_RELEASE_DELAY_MS,
        pttReleaseDelayMs,
      );
    } else if (next.kind === "voice-activity") {
      mode = "voice-activity";
      vadThresholdDb = clampInt(
        next.thresholdDb,
        vadMinDb,
        vadMaxDb,
        vadThresholdDb,
      );
    } else {
      const unknownMode = /** @type {{ kind?: unknown }} */ (next);
      throw new TypeError(`voice-devices: неизвестный режим ${String(unknownMode.kind)}`);
    }
    renderMode();
    invoke(service.setInputMode, currentMode());
  }

  function beginPttKeyCapture() {
    if (destroyed) return;
    capturing = true;
    renderPttKey();
    pttKeyButton.focus();
  }

  function cancelPttKeyCapture() {
    if (!capturing) return;
    capturing = false;
    renderPttKey();
  }

  /**
   * @param {string | null} key
   */
  function setPttKey(key) {
    if (destroyed) return;
    pttKey = key === null || key === "" ? null : key;
    capturing = false;
    renderPttKey();
    invoke(service.setPttKey, pttKey);
  }

  function clearPttKey() {
    setPttKey(null);
  }

  /* ---------- mute / deafen ---------- */

  function renderMute() {
    const effective = effectiveMute();
    userMuteToggle.checked = userMute;
    userMuteToggle.setAttribute("aria-checked", userMute ? "true" : "false");
    selfDeafToggle.checked = selfDeaf;
    selfDeafToggle.setAttribute("aria-checked", selfDeaf ? "true" : "false");
    host.setAttribute("data-effective-mute", effective ? "true" : "false");
    effectiveMuteNode.setAttribute("data-effective-mute", effective ? "true" : "false");
    effectiveMuteNode.textContent = effective ? "Микрофон отключён" : "Микрофон активен";
  }

  /**
   * @param {boolean} mute
   */
  function setUserMute(mute) {
    if (destroyed) return;
    userMute = mute === true;
    renderMute();
    invoke(service.setUserMute, userMute);
  }

  /**
   * @param {boolean} deaf
   */
  function setSelfDeaf(deaf) {
    if (destroyed) return;
    selfDeaf = deaf === true;
    renderMute();
    invoke(service.setSelfDeaf, selfDeaf);
  }

  /* ---------- события ---------- */

  inputSelect.addEventListener("change", () => {
    if (destroyed) return;
    selectInput(inputSelect.value);
  });
  outputSelect.addEventListener("change", () => {
    if (destroyed) return;
    selectOutput(outputSelect.value);
  });
  videoSelect.addEventListener("change", () => {
    if (destroyed) return;
    selectVideo(videoSelect.value);
  });
  inputVolumeRange.addEventListener("input", () => {
    if (destroyed) return;
    setInputVolume(Number(inputVolumeRange.value));
  });
  outputVolumeRange.addEventListener("input", () => {
    if (destroyed) return;
    setOutputVolume(Number(outputVolumeRange.value));
  });
  vadThreshold.addEventListener("input", () => {
    if (destroyed) return;
    setInputMode({ kind: "voice-activity", thresholdDb: Number(vadThreshold.value) });
  });
  pttDelay.addEventListener("input", () => {
    if (destroyed) return;
    setInputMode({ kind: "push-to-talk", releaseDelayMs: Number(pttDelay.value) });
  });
  pttToggle.addEventListener("click", () => {
    if (destroyed) return;
    setInputMode(
      pttToggle.checked
        ? { kind: "push-to-talk", releaseDelayMs: pttReleaseDelayMs }
        : { kind: "voice-activity", thresholdDb: vadThresholdDb },
    );
  });
  userMuteToggle.addEventListener("click", () => {
    if (destroyed) return;
    const nextMute = boundMode ? !userMute : userMuteToggle.checked;
    setUserMute(nextMute);
  });
  selfDeafToggle.addEventListener("click", () => {
    if (destroyed) return;
    const nextDeaf = boundMode ? !selfDeaf : selfDeafToggle.checked;
    setSelfDeaf(nextDeaf);
  });
  micTestButton.addEventListener("click", () => {
    if (destroyed) return;
    if (micTesting) stopMicTest();
    else startMicTest();
  });
  pttKeyButton.addEventListener("click", () => {
    if (destroyed) return;
    if (capturing) cancelPttKeyCapture();
    else beginPttKeyCapture();
  });
  pttKeyButton.addEventListener("keydown", (event) => {
    if (destroyed || !capturing) return;
    event.preventDefault();
    const key = "key" in event ? String(event.key) : "";
    if (key === "Escape") {
      cancelPttKeyCapture();
      return;
    }
    if (key === "" || MODIFIER_KEYS.has(key)) return;
    setPttKey(key);
  });

  /* ---------- начальная привязка (без hot-plug уведомлений) ---------- */

  const initialInput = reconcile("input", null, inputs());
  const initialOutput = reconcile("output", null, outputs());
  const initialVideo = reconcile("video", null, videos());
  selectedInput = initialInput.selected;
  selectedOutput = initialOutput.selected;
  selectedVideo = initialVideo.selected;
  renderDevices();
  renderInputVolume();
  renderOutputVolume();
  renderMicTest();
  renderMode();
  renderMute();

  /** @returns {{
   *   devices: AudioDevice[],
   *   selectedInput: string | null,
   *   selectedOutput: string | null,
   *   selectedVideo: string | null,
   *   userMute: boolean,
   *   selfDeaf: boolean,
   *   effectiveMute: boolean,
   *   inputMode: VoiceInputMode,
   *   vadThresholdDb: number,
   *   pttReleaseDelayMs: number,
   *   inputVolume: number,
   *   outputVolume: number,
   *   pttKey: string | null,
   *   pttCapturing: boolean,
   *   micTesting: boolean,
   *   micTestLevelDb: number,
   * }} */
  function getState() {
    return {
      devices: devices.map((device) => ({ ...device })),
      selectedInput,
      selectedOutput,
      selectedVideo,
      userMute,
      selfDeaf,
      effectiveMute: effectiveMute(),
      inputMode: currentMode(),
      vadThresholdDb,
      pttReleaseDelayMs,
      inputVolume,
      outputVolume,
      pttKey,
      pttCapturing: capturing,
      micTesting,
      micTestLevelDb,
    };
  }

  function destroy() {
    if (destroyed) return;
    if (micTesting) stopMicTest();
    destroyed = true;
    capturing = false;
    if (boundMode) {
      inputSelect.removeAttribute("id");
      outputSelect.removeAttribute("id");
      videoSelect.removeAttribute("id");
      inputVolumeRange.removeAttribute("id");
      outputVolumeRange.removeAttribute("id");
      pttToggle.removeAttribute("id");
      micTestButton.removeAttribute("id");
      micTestButton.removeAttribute("aria-pressed");
      const mountContainer = controls?.mountContainer ?? null;
      if (mountContainer) mountContainer.replaceChildren();
    } else {
      host.replaceChildren();
    }
  }

  return {
    element: host,
    getState,
    setDevices,
    selectInput,
    selectOutput,
    selectVideo,
    setInputVolume,
    setOutputVolume,
    setInputMode,
    setUserMute,
    setSelfDeaf,
    setPttKey,
    clearPttKey,
    beginPttKeyCapture,
    cancelPttKeyCapture,
    startMicTest,
    stopMicTest,
    setMicTestLevelDb,
    isDestroyed: () => destroyed,
    destroy,
  };
}

/**
 * Создаёт контроллер настроек «Голос и видео» с UI-границей чувствительности
 * −100..0 дБ по умолчанию (`VOICE-DEVICES-SETTINGS-SPEC.md` §3.4, FE-P0-02).
 * @param {VoiceDevicesOptions} options
 */
export function createVoiceDevicesSettingsController(options) {
  return createVoiceDevicesSettings({
    vadMinDb: MIN_UI_VAD_THRESHOLD_DB,
    vadMaxDb: MAX_UI_VAD_THRESHOLD_DB,
    ...options,
  });
}
