# RW-40 / G3 — VOICE-DEVICES-SETTINGS-SPEC: панель настроек «Голос и видео» (V-07-UI)

Статус: дизайн-контракт (спецификация; продуктовый код и тесты не изменялись). Дата: 2026-10-30.  
Владелец: **G3 Design** (карточка `G3-VOICE-DEVICES-SETTINGS-SPEC`).  
Потребители: **G2 Frontend** (карточка `V-07-UI`, модули `rust/gui-spike/ui/features/voice-devices-settings.js`, `rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/ui/components/settings.css`), **G5 Voice / G1 Backend** (контракты `rust/crates/discord-voice/src/devices.rs` и `rust/gui-spike/src/voice_devices.rs`).

Нормативные источники и оракулы:
- `rust/gui-spike/ui/tokens.css` — единственный нормативный источник CSS-токенов (поверхности, текст, акценты, статусы, границы, состояния, типографика, отступы, радиусы, тени, слои, длительности переходов);
- `rust/crates/discord-voice/src/devices.rs` и `rust/gui-spike/src/voice_devices.rs` — детерминированная модель аудио-устройств, реконсиляция hot-plug списка в `VoiceDevicesState::update_device_list` с расчётом кандидата отката на `is_default == true` (или первое доступное) и событиями `DeviceFallback` / `NoDeviceAvailable` **без самопроизвольного вызова `service.select*`**, ошибки выбора `EmptyId` / `NotFound` / `WrongKind`, инвариант `effective_mute = user_mute || self_deaf`, режимы активации `VoiceActivity { threshold_db: i8 }` и `PushToTalk { release_delay_ms: u32 }`, клампинг громкости ввода `0..=100%` и вывода `0..=200%`, редактирование меток устройств в `fmt::Debug`;
- `waves/RW-40/DESIGN/VOICE.md` (§2.4, §3.1–§3.6, §4, §5, §6.3) и `waves/RW-40/DESIGN/voice-spec.json` (`devices.sliders`: `OX2Bnr` `0..=100` = `100`, `aUJ062` `0..=200` = `100`; `devices.hotplug.autoswitch: false`; `devices.sensitivity.slider`: `0..=100` = `26.109350171958468`, API `setNoInputThreshold`; ключи локализации `hHMYbb`, `dl18zb`, `nuFtHH`, `snEhlu`, `DGq/PR`, `OX2Bnr`, `aUJ062`, `tG4Np5`, `o+BJQR`, `yept4k`, `GCNMM8`, `nWRdnl`, `vxfv7v`, `5rPt+j`, `fAYU2G`, `JMyQin`, `VHI4+Y`, `GByLar`, `lY6j47`, `JsbzjA`, `gyljWE`, `Vd/rEX`, `bgn75v`, `MA+OZh`, `gF8HJo`, `sWmtI6`, `hK51Yg`);
- `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` (§5.1, §5.4, §6, §7.4) — системные инварианты ALSA / PipeWire / XDG Desktop Portal `GlobalShortcuts` и защита приватности имён аудио-устройств ОС (PII);
- `waves/RW-40/THEME-SPEC.md` (§2, §4) — правила контраста WCAG 2.2 AA в тёмной и светлой темах без добавления новых имён переменных сверх `rust/gui-spike/ui/tokens.css`.

---

## 0. Легенда, правила токенизации и архитектурные инварианты

| Метка | Значение | Источник |
|---|---|---|
| `[T]` | Существующий CSS-токен из `rust/gui-spike/ui/tokens.css` | `rust/gui-spike/ui/tokens.css` |
| `[D]` | Подтверждённый факт дампа (DOM, строки i18n, диапазоны ползунков, API) | `waves/RW-40/DESIGN/VOICE.md`, `waves/RW-40/DESIGN/voice-spec.json` |
| `[S]` | Подтверждённый контракт модели и рантайма | `rust/crates/discord-voice/src/devices.rs`, `rust/gui-spike/src/voice_devices.rs` |
| `[A]` | Проектное решение / адаптация для `V-07-UI` | Настоящая спецификация |
| `[F]` | Зафиксировано контрактными тестами | `rust/crates/discord-voice/tests/devices_contract.rs`, `tests/unit/voice-devices-settings.test.mjs` |

### 0.1. Строгая привязка к `rust/gui-spike/ui/tokens.css` (`100% token binding`)

1. **Только существующие токены**: все визуальные правила панели «Голос и видео» используют исключительно кастомные свойства `var(--*)`, объявленные в `rust/gui-spike/ui/tokens.css`. Введение новых имён переменных, а также любых сырых цветовых литералов в CSS-правилах и спецификации запрещено.
2. **Без внешних ассетов**: запрещены внешние изображения, иконочные шрифты и сетевые ссылки. Используются только системные шрифты `var(--font-sans)` и `var(--font-mono)` (`rust/gui-spike/ui/tokens.css` §5), CSS-геометрия и встроенная векторная графика с `currentColor`.
3. **Приватность и запрет прямого захвата из UI (`VOICE.md` §5)**:
   - UI-контроллер не обращается к `navigator.mediaDevices`, `getUserMedia`, `getDisplayMedia`, `startLocalAudioRecording`, `startRecordingRawSamples`, `setAecDump`, `setLoopback`, `fetch`, `WebSocket` и `console.*`.
   - Человекочитаемые названия устройств ОС (`AudioDevice.label`, например `Система: Easy Effects Source`) отображаются исключительно в DOM-элементах пользовательского интерфейса (`<option>`, текст уведомления об откате `#voice-devices-notice`) и никогда не записываются в диагностические логи (в Rust `fmt::Debug` поле `label` принудительно заменяется на `<redacted>`).
4. **Детерминированный однопутевой DOM**: вся разметка создаётся через `host.ownerDocument.createElement(...)` без `innerHTML`, что обеспечивает идентичное дерево элементов в рабочем WebView и в тестовом DOM-шиме (`ShimDocument`).

---

## 1. Контракт данных и констант (`voice_devices.rs` ↔ `V-07-UI`)

### 1.1. Нормативные числовые константы и диапазоны

Все границы диапазонов и значения по умолчанию синхронизированы между Rust-контрактом (`rust/crates/discord-voice/src/devices.rs` / `rust/gui-spike/src/voice_devices.rs`), JSON-оракулом (`waves/RW-40/DESIGN/voice-spec.json`) и UI-контроллером:

| Константа / Параметр | Тип (Rust / JS) | Минимум | Максимум | По умолчанию | Шаг | Единица / Формат в UI | Оракул / Метка |
|---|---|---:|---:|---:|---:|---|---|
| `MIN_VOLUME` | `u16` / `number` | `0` | — | — | `1` | `%` (`"0%"`) | `[D]` `[S]` `[F]` |
| `MAX_INPUT_VOLUME` / `DEFAULT_INPUT_VOLUME` (`OX2Bnr`) | `u16` / `number` | `0` | `100` | `100` | `1` | `%` (`"{N}%"`, дефолт `"100%"`) | `[D]` (`OX2Bnr`), `[S]` `[F]` |
| `MAX_OUTPUT_VOLUME` / `DEFAULT_OUTPUT_VOLUME` (`aUJ062`) | `u16` / `number` | `0` | `200` | `100` | `1` | `%` (`"{N}%"`, дефолт `"100%"`) | `[D]` (`aUJ062`), `[S]` `[F]` |
| `DEFAULT_VAD_THRESHOLD_DB` (`threshold_db` / `vadThresholdDb`) | `i8` / `number` | `-128` (`MIN_VAD_THRESHOLD_DB`) | `127` (`MAX_VAD_THRESHOLD_DB`) | `-40` | `1` | `дБ` (`"{N} дБ"`, дефолт `"-40 дБ"`) | `[S]` `[A]` `[F]` (`i8` в `devices.rs`; `[A]` authored adaptation относительно `0..=100` в `voice-spec.json`) |
| Чувствительность ввода дампа (`devices.sensitivity.slider`, `setNoInputThreshold`) | `number` | `0` | `100` | `60` (в дампе `26.109350171958468` при включённом авто `lY6j47`) | `1` | `%` / `0..100` (связь с `threshold_db`: `thresholdDb = clamp_i8(round(pct) - 100, -128, 127)`) | `[D]` (`0..100` в `voice-spec.json` / `VOICE.md` §3.4), `[A]` (формула связи с `i8` дБ) |
| `release_delay_ms` (`pttReleaseDelayMs`, `GCNMM8`) | `u32` / `number` | `0` (`MIN_PTT_RELEASE_DELAY_MS`) | `2000` (`MAX_PTT_RELEASE_DELAY_MS`) | `200` (контракт `voice_devices.rs` / `devices_contract.rs`) | `1` | `мс` (`"{N} мс"`, дефолт `"200 мс"`) | `[S]` `[A]` `[F]` (`0..=2000` мс) |
| Шкала проверки микрофона (`micTestLevelDb`) | `i8` / `number` | `-100` | `0` | `-100` (тишина) | `1` | `дБ` (`"{N} дБ"`) | `[A]` (`role="meter"`) |

> [!IMPORTANT]
> **Примечание о дефолте задержки отпускания PTT (`release_delay_ms`) и authored adaptation `[A]` для шкалы порога VAD (`vadThresholdDb`):**
> 1. По контракту `rust/gui-spike/src/voice_devices.rs` и интеграционному тесту `t_v07_04_voice_activity_and_ptt_release_delay_and_mute_blocking` (`rust/crates/discord-voice/tests/devices_contract.rs:314-316`) нормативный диапазон слайдера `#voice-devices-ptt-delay` составляет `0..=2000` мс (`min="0"`, `max="2000"`), а каноническое значение по умолчанию — `200` мс (`"200 мс"`). Любое начальное значение в пределах `0..=2000` мс (включая `200` мс по контракту `voice_devices.rs` или `20` мс в автономном пресете `voice-devices-settings.js`) обязано синхронно отражаться в `pttDelay.value`, `aria-valuenow`, `aria-valuetext`, `#voice-devices-ptt-delay-value` и `state.pttReleaseDelayMs`. При вводе значения вне диапазона применяется строгий целочисленный клампинг `Math.min(2000, Math.max(0, Math.round(value)))`.
> 2. **Маркировка `[A]` authored adaptation для `vadThresholdDb` (`-128..=127` дБ) и связь со шкалой чувствительности `0..=100` (`voice-spec.json`):** в RE-дампе (`VOICE.md` §3.4 и `voice-spec.json` → `devices.sensitivity.slider`) ползунок чувствительности ввода использует нормализованную шкалу `0..=100` (`aria-valuemin="0"`, `aria-valuemax="100"`, `aria-valuenow="26.109350171958468"`, API `setNoInputThreshold` `[D]`). В изолированном контроллере `V-07-UI` слайдер `#voice-devices-vad-threshold` является **авторской проектной адаптацией `[A]` (непаритетным с `0..=100` прямым контролом инженерного порога)**, напрямую привязанной к типу Rust-модели `VoiceInputMode::VoiceActivity { threshold_db: i8 }` (`-128..=127` дБ, `DEFAULT_VAD_THRESHOLD_DB = -40` дБ `[S]` `[F]`). Для согласования со шкалой чувствительности `0..=100` (`VOICE.md` §3.4 / `settings.js` `input.sensitivity`) фиксируется линейное отображение рабочего окна `-100..=0` дБ (`[A]`): `thresholdDb = clamp_i8(Math.round(sensitivityPercent) - 100, -128, 127)` и обратное `sensitivityPercent = Math.min(100, Math.max(0, thresholdDb + 100))` (при этом `0%` соответствует `-100` дБ, `60%` — дефолту `-40` дБ, замер дампа `26.109%` — `-74` дБ, `100%` — `0` дБ).

### 1.2. Типы данных и события модели

```ts
type AudioDeviceKind = "input" | "output";

interface AudioDevice {
  id: string;        // Непустой стабильный идентификатор устройства
  label: string;     // Человекочитаемое имя для UI (например, "Система: Easy Effects Source"), PII — не логировать
  kind: AudioDeviceKind;
  isDefault: boolean; // Системное устройство по умолчанию для данного kind
}

type VoiceInputMode =
  | { kind: "voice-activity"; thresholdDb: number }   // thresholdDb клампится в -128..=127 (i8)
  | { kind: "push-to-talk"; releaseDelayMs: number }; // releaseDelayMs клампится в 0..=2000 (u32)

type DeviceEvent =
  | {
      type: "DeviceFallback";
      kind: AudioDeviceKind;
      previousId: string;
      fallbackId: string;
    }
  | {
      type: "NoDeviceAvailable";
      kind: AudioDeviceKind;
      previousId: string | null;
    };

interface VoiceDevicesStateSnapshot {
  devices: AudioDevice[];
  selectedInput: string | null;
  selectedOutput: string | null;
  userMute: boolean;
  selfDeaf: boolean;
  effectiveMute: boolean; // Всегда равно (userMute || selfDeaf)
  inputMode: VoiceInputMode;
  vadThresholdDb: number;
  pttReleaseDelayMs: number;
  inputVolume: number;    // 0..=100
  outputVolume: number;   // 0..=200
  pttKey: string | null;
  pttCapturing: boolean;
}
```

### 1.3. Контракт методов сервиса (`VoiceDevicesService`)

Контроллер принимает инъекцию `service` (все методы опциональны; при отсутствии метода локальное состояние обновляется детерминированно в памяти):
- `selectInput(id: string): unknown` — вызывается ровно один раз при валидном явном выборе входного устройства пользователем;
- `selectOutput(id: string): unknown` — вызывается ровно один раз при валидном явном выборе выходного устройства пользователем;
- `setInputVolume(percent: number): unknown` — получает уже клампированное целое число `0..=100`;
- `setOutputVolume(percent: number): unknown` — получает уже клампированное целое число `0..=200`;
- `setInputMode(mode: VoiceInputMode): unknown` — получает актуальный объект режима с клампированным `thresholdDb` (`-128..=127`) или `releaseDelayMs` (`0..=2000`);
- `setUserMute(mute: boolean): unknown` — получает булев флаг явного пользовательского mute;
- `setSelfDeaf(deaf: boolean): unknown` — получает булев флаг полного отключения звука;
- `setPttKey(key: string | null): unknown` — получает назначенную клавишу PTT (или `null` при сбросе);
- `startMicTest?(): unknown` / `stopMicTest?(): unknown` — опциональное управление проверкой микрофона по явному клику пользователя.

Если любой метод `service` выбрасывает синхронное исключение или возвращает отвергнутый `Promise`, контроллер перехватывает ошибку и выводит в `#voice-devices-notice` сообщение `Не удалось сохранить настройку: <описание ошибки>` с `role="alert"` и `aria-live="assertive"`, не разрушая DOM и не сбрасывая применённое в UI состояние.

---

## 2. Раскладка, геометрия и DOM-структура панели «Голос и видео»

### 2.1. Позиционирование внутри экрана настроек

Внутри оверлея `#screen-settings` (`.settings-shell` → `.settings-body` → `.settings`) раздел «Голос и видео» (`section.settings-pane[data-pane="voice"]`) располагается в правой скроллируемой колонке:
- Максимальная ширина контентных блоков: `max-width: var(--layout-content-max-w)` (`720px` `[T]`).
- Внутренние отступы контейнера `.settings`: `padding: var(--space-6) var(--modal-pad-x) var(--space-9)` (`16px 24px 32px` `[T]`).
- Фон области контента: `var(--bg-chat)` (`[T]`).
- Заголовок вкладки: `<h3 class="settings-pane-title">Голос и видео</h3>` (`font-size: var(--font-size-small)`, `font-weight: var(--font-weight-semibold)`, `letter-spacing: var(--letter-spacing-caps)`, `text-transform: uppercase`, `color: var(--text-muted)` `[T]`).
- Подзаголовки функциональных блоков: `<h4 class="settings-block-title">` (`margin: var(--space-6) 0 var(--space-2)`, `font-size: var(--font-size-ui)`, `font-weight: var(--font-weight-semibold)`, `line-height: var(--line-height-ui)`, `color: var(--text-strong)` `[T]`).

### 2.2. Каноническое DOM-дерево компонента `#voice-devices` (`V-07-UI`)

Контроллер `createVoiceDevicesSettings({ host, devices, service, onDeviceEvent })` монтируется в корневой элемент `section#voice-devices` и формирует следующую нормативную структуру с фиксированными `id`, классами и ARIA-атрибутами:

```html
<section id="voice-devices"
         class="voice-devices"
         aria-label="Голос и видео"
         data-effective-mute="false">

  <!-- 1. Выбор устройства ввода (микрофона) -->
  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Устройство ввода</span>
    <select id="voice-devices-input"
            class="voice-devices-select settings-select"
            aria-label="Устройство ввода">
      <option value="mic-builtin" title="Система: Built-in Microphone">Система: Built-in Microphone</option>
      <option value="mic-usb" title="Система: USB Microphone">Система: USB Microphone</option>
    </select>
  </div>

  <!-- 2. Выбор устройства вывода (динамиков / наушников) -->
  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Устройство вывода</span>
    <select id="voice-devices-output"
            class="voice-devices-select settings-select"
            aria-label="Устройство вывода">
      <option value="spk-headset" title="Система: Headset Earphone">Система: Headset Earphone</option>
      <option value="spk-hdmi" title="Система: HDMI Output">Система: HDMI Output</option>
    </select>
  </div>

  <!-- 3. Ползунок громкости микрофона (0..100%, дефолт 100%, оракул OX2Bnr) -->
  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Громкость микрофона</span>
    <input id="voice-devices-input-volume"
           class="voice-devices-range settings-slider"
           type="range"
           min="0"
           max="100"
           step="1"
           value="100"
           role="slider"
           aria-label="Громкость микрофона"
           aria-orientation="horizontal"
           aria-valuemin="0"
           aria-valuemax="100"
           aria-valuenow="100"
           aria-valuetext="100%" />
  </div>
  <output id="voice-devices-input-volume-value"
          class="voice-devices-value settings-slider-value">100%</output>

  <!-- 4. Ползунок громкости вывода (0..200%, дефолт 100%, оракул aUJ062) -->
  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Громкость динамика</span>
    <input id="voice-devices-output-volume"
           class="voice-devices-range settings-slider"
           type="range"
           min="0"
           max="200"
           step="1"
           value="100"
           role="slider"
           aria-label="Громкость динамика"
           aria-orientation="horizontal"
           aria-valuemin="0"
           aria-valuemax="200"
           aria-valuenow="100"
           aria-valuetext="100%" />
  </div>
  <output id="voice-devices-output-volume-value"
          class="voice-devices-value settings-slider-value">100%</output>

  <!-- 5. Индикатор и блок проверки микрофона (Mic Test) -->
  <div id="voice-devices-mic-test"
       class="voice-devices-mic-test"
       data-test-state="idle">
    <div class="voice-devices-mic-test-controls">
      <button id="voice-devices-mic-test-btn"
              class="voice-devices-action settings-action"
              type="button"
              aria-pressed="false"
              aria-label="Проверка микрофона">Проверка микрофона</button>
      <div id="voice-devices-mic-meter"
           class="voice-devices-meter"
           role="meter"
           aria-label="Уровень сигнала микрофона"
           aria-valuemin="-100"
           aria-valuemax="0"
           aria-valuenow="-100"
           aria-valuetext="-100 дБ">
        <div class="voice-devices-meter-fill" aria-hidden="true"></div>
        <div class="voice-devices-meter-threshold" aria-hidden="true"></div>
      </div>
    </div>
    <p id="voice-devices-mic-status"
       class="voice-devices-mic-hint"
       role="status"
       aria-live="polite">Проверка микрофона. Начните, когда будете готовы.</p>
  </div>

  <!-- 6. Настройки режима «Активация по голосу» (Voice Activity, видимы при mode === "voice-activity") -->
  <div id="voice-devices-vad-settings"
       class="voice-devices-group"
       role="group"
       aria-label="Порог активации по голосу">
    <input id="voice-devices-vad-threshold"
           class="voice-devices-range settings-slider"
           type="range"
           min="-128"
           max="127"
           step="1"
           value="-40"
           role="slider"
           aria-label="Порог активации по голосу"
           aria-orientation="horizontal"
           aria-valuemin="-128"
           aria-valuemax="127"
           aria-valuenow="-40"
           aria-valuetext="-40 дБ" />
    <output id="voice-devices-vad-threshold-value"
            class="voice-devices-value settings-slider-value">-40 дБ</output>
  </div>

  <!-- 7. Переключатель режима ввода: «Активация по голосу» (false) / «Режим рации» (true) -->
  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Режим рации</span>
    <input id="voice-devices-ptt"
           class="voice-devices-switch"
           type="checkbox"
           role="switch"
           aria-checked="false"
           aria-label="Режим рации" />
  </div>

  <!-- 8. Настройки «Режима рации (Push-to-Talk)» (скрыты атрибутом hidden при mode === "voice-activity") -->
  <div id="voice-devices-ptt-settings"
       class="voice-devices-group"
       role="group"
       aria-label="Режим рации"
       hidden>
    <input id="voice-devices-ptt-delay"
           class="voice-devices-range settings-slider"
           type="range"
           min="0"
           max="2000"
           step="1"
           value="200"
           role="slider"
           aria-label="Задержка отключения в режиме рации"
           aria-orientation="horizontal"
           aria-valuemin="0"
           aria-valuemax="2000"
           aria-valuenow="200"
           aria-valuetext="200 мс" />
    <output id="voice-devices-ptt-delay-value"
            class="voice-devices-value settings-slider-value">200 мс</output>
    <button id="voice-devices-ptt-key"
            class="voice-devices-key settings-action"
            type="button"
            aria-label="Горячая клавиша для режима рации"
            data-capturing="false"
            data-ptt-status="ready">Горячие клавиши не назначены</button>
  </div>

  <!-- 9. Тумблеры Заглушить (userMute) и Откл. звук (selfDeaf) + индикатор эффективного mute -->
  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Заглушить</span>
    <input id="voice-devices-user-mute"
           class="voice-devices-switch"
           type="checkbox"
           role="switch"
           aria-checked="false"
           aria-label="Заглушить" />
  </div>

  <div class="voice-devices-field settings-field">
    <span class="voice-devices-label settings-field-label">Откл. звук</span>
    <input id="voice-devices-self-deaf"
           class="voice-devices-switch"
           type="checkbox"
           role="switch"
           aria-checked="false"
           aria-label="Откл. звук" />
  </div>

  <p id="voice-devices-effective-mute"
     class="voice-devices-effective"
     aria-live="polite"
     data-effective-mute="false">Микрофон активен</p>

  <!-- 10. Живой регион уведомлений hot-plug и ошибок (role="status" | "alert") -->
  <p id="voice-devices-notice"
     class="voice-devices-notice"
     role="status"
     aria-live="polite"
     hidden></p>
</section>
```

---

## 3. Детальная спецификация функциональных узлов

### 3.1. Выбор входного и выходного аудио-устройства (`#voice-devices-input`, `#voice-devices-output`)

1. **Инициализация выбора по умолчанию (`isDefault`)**:
   - При первичной загрузке списка устройств (`options.devices`) контроллер разделяет их по направлению (`kind === "input"` и `kind === "output"`).
   - Для каждого направления выбирается устройство с `isDefault === true` (`is_default == true` в Rust), **даже если оно стоит не первым в массиве**.
   - Если в непустом списке данного направления нет ни одного устройства с `isDefault === true`, выбирается первое доступное устройство этого направления (`list[0]`).
   - Первичная инициализация **не эмитит** события `DeviceFallback` или `NoDeviceAvailable` и не показывает баннер `#voice-devices-notice`.
2. **Отрисовка `<option>` и длинные имена устройств (`VOICE.md` §3.1)**:
   - Каждая опция `<option>` получает `option.value = device.id`, `option.textContent = device.label` и `option.title = device.label`.
   - В DOM текст `<option>` **никогда не обрезается** программно; визуальное усечение длинных названий в закрытом селекте выполняется исключительно CSS-правилом `text-overflow: ellipsis; white-space: nowrap; overflow: hidden;`, а полное имя всегда доступно по наведению через `title`.
3. **Явный выбор пользователем (`change` / `selectInput` / `selectOutput`)**:
   - При выборе существующего `id` подходящего направления (`kind`):
     - `selectedInput` (или `selectedOutput`) синхронно обновляется;
     - `<select>` отображает выбранный `id`;
     - уведомление `#voice-devices-notice` очищается и скрывается (`hidden`);
     - вызывается `service.selectInput(id)` (или `service.selectOutput(id)`) ровно 1 раз.
   - При попытке выбрать пустой `id` (`EmptyId`), несуществующий `id` (`NotFound`) или `id` устройства противоположного направления (`WrongKind`, например выходное устройство как микрофон):
     - метод возвращает `false`;
     - текущий выбор `selectedInput` / `selectedOutput` и `select.value` **остаются прежними**;
     - `service.selectInput` / `service.selectOutput` **не вызывается**;
     - в `#voice-devices-notice` выводится сообщение `Устройство не найдено.` с `role="alert"`, `aria-live="assertive"` и снятым атрибутом `hidden`.

### 3.2. Реакция на Hot-Plug и согласование Fallback (`VoiceDevicesState::update_device_list` ↔ `VOICE.md` §3.2)

> [!IMPORTANT]
> **Единый согласованный контракт Hot-Plug (`devices.rs:288–338` ↔ `VOICE.md` §3.2 / `voice-spec.json` `devices.hotplug.autoswitch: false`):**
> - **Поведение аудиосервиса и панели голосовой связи (`VOICE.md` §3.2, `voice-spec.json` `autoswitch: false`, `[D]`/`[A]`)**: событие hot-plug (`setDeviceChangeCallback`) **никогда не вызывает `service.selectInput` / `service.selectOutput` (`setInputDevice` / `setOutputDevice`) самостоятельно** (`0 вызовов`). Фактическое переключение аудиопотока в рантайме происходит **только по явному клику/выбору пользователя**. Если во время разговора исчез активный микрофон, соединение не рвётся, а панель связи `#voice-panel` переходит в диагностическое состояние `data-state="device-missing"` с текстом `bgn75v` / `MA+OZh` («Убедитесь, что вы выбрали правильное устройство ввода.»), ожидая возврата устройства или явного подтверждения выбора пользователем.
> - **Поведение детерминированной модели и `<select>` в окне настроек (`VoiceDevicesState::update_device_list` в `devices.rs:288–338` и `setDevices` в `V-07-UI`, `[S]`/`[A]`/`[F]`)**: поскольку отключённое устройство `previousId` отсутствует в новом списке `<option>`, локальная модель настроек синхронизирует указатель выбора `selectedInput` / `selectedOutput` и `select.value` на расчётный кандидат отката `fallbackId` (`isDefault === true` или первое доступное `list[0]`, а при пустом списке — `null` с `aria-disabled="true"`), эмитит диагностическое событие `DeviceFallback` / `NoDeviceAvailable` и показывает уведомление в `#voice-devices-notice`. Эта реконсиляция локальной модели и `<select>` **не является автоматическим вызовом `service.select*`** и служит предвыбранным кандидатом отката до явного действия пользователя.

При поступлении обновлённого списка устройств от системного перечислителя (`controller.setDevices(nextDevices)` / `VoiceDevicesState::update_device_list`):

| Сценарий Hot-Plug | Локальный указатель модели настроек (`selectedInput` / `selectedOutput`) | Событие `onDeviceEvent` (`DeviceEvent`) | Вызов `service.select*` (`autoswitch: false`) | Состояние `<select>` в настройках, `#voice-devices-notice` и `#voice-panel` |
|---|---|---|---|---|
| **1. Активное устройство осталось в списке** (добавилось новое, в т.ч. новое `isDefault`, или отключилось невыбранное устройство) | Сохраняется текущий `selectedInput` / `selectedOutput`; переключения на новое устройство не происходит (`VOICE.md` §3.2, `autoswitch: false`) | Событий нет (`[]`) | **0 вызовов** | `<select>` обновляет список `<option>`, сохраняя текущее `value`; `#voice-devices-notice` и `#voice-panel` не изменяются |
| **2. Активное устройство отключено, в списке есть системное `isDefault === true`** | Локальная реконсиляция модели (`update_device_list` `[S]` `[F]`) выставляет кандидат отката на устройство с `isDefault === true` (независимо от его индекса в списке) без вызова аудиосервиса | `{ type: "DeviceFallback", kind, previousId, fallbackId }` | **0 вызовов** (правило `VOICE.md` §3.2: не вызывает `service.select*` самостоятельно; явный вызов — только по `change` / клику пользователя) | `<select>` отображает кандидат `fallbackId`; `#voice-devices-notice` виден (`hidden` снят), `role="status"`, `aria-live="polite"`, текст: `Устройство «<previous.label>» отключено. Выбрано «<fallback.label>».`; в активном звонке `#voice-panel` отражает `device-missing` (`MA+OZh`) без разрыва разговора до возврата устройства или явного выбора |
| **3. Активное устройство отключено, `isDefault` нет, но есть иное устройство того же `kind`** | Локальная реконсиляция модели (`update_device_list` `[S]` `[F]`) выставляет кандидат отката на первое доступное устройство данного направления (`list[0]`) без вызова аудиосервиса | `{ type: "DeviceFallback", kind, previousId, fallbackId }` | **0 вызовов** (правило `VOICE.md` §3.2: не вызывает `service.select*` самостоятельно) | `<select>` отображает кандидат `fallbackId`; `#voice-devices-notice` виден, `role="status"`, `aria-live="polite"`, текст: `Устройство «<previous.label>» отключено. Выбрано «<fallback.label>».`; в активном звонке `#voice-panel` отражает `device-missing` (`MA+OZh`) без разрыва разговора |
| **4. Все устройства направления `kind` отключены** (`list.length === 0`) | Сбрасывается в `null` | `{ type: "NoDeviceAvailable", kind, previousId }` (`previousId` равен прежнему `id` или `null`) | **0 вызовов** | `<select>` получает `aria-disabled="true"` и единственную заглушку `<option value="">Микрофон не выбран</option>` (для `input`) или `<option value="">Устройство вывода не выбрано</option>` (для `output`); `#voice-devices-notice` виден, `role="alert"`, `aria-live="assertive"`, текст: `Нет доступных микрофонов.` или `Нет доступных устройств вывода.`; `#voice-panel` отражает `device-missing` (`bgn75v` / `MA+OZh`) |

### 3.3. Регулировка громкости ввода и вывода (`OX2Bnr`, `aUJ062`)

1. **Громкость микрофона (`#voice-devices-input-volume`)**:
   - Диапазон по оракулу `OX2Bnr`: `min="0"`, `max="100"`, `step="1"`, дефолт `100`.
   - Любое входное значение округляется и клампится в `0..=100` (`set_input_volume` в Rust / `setInputVolume` в UI).
   - Подпись значения в `#voice-devices-input-volume-value` и `aria-valuetext`: `"{inputVolume}%"` (например, `"100%"`).
2. **Громкость динамика / вывода (`#voice-devices-output-volume`)**:
   - Диапазон по оракулу `aUJ062`: `min="0"`, `max="200"`, `step="1"`, дефолт `100`.
   - Любое входное значение округляется и клампится в `0..=200` (`set_output_volume` в Rust / `setOutputVolume` в UI).
   - Подпись значения в `#voice-devices-output-volume-value` и `aria-valuetext`: `"{outputVolume}%"` (например, `"150%"`, `"200%"`).
3. **Независимость каналов**:
   - Изменение громкости вывода никогда не затрагивает громкость ввода и наоборот.

### 3.4. Переключатель режима ввода: «Активация по голосу» vs «Режим рации (Push-to-Talk)» и шкала порога VAD (`[A]`)

1. **Два взаимоисключающих режима (`VoiceInputMode`)**:
   - **Активация по голосу (`VoiceActivity`, дефолт)**:
     - Тумблер `#voice-devices-ptt` находится в состоянии `checked = false`, `aria-checked="false"`.
     - Группа `#voice-devices-vad-settings` (`role="group"`, `aria-label="Порог активации по голосу"`) видима (атрибут `hidden` отсутствует); группа `#voice-devices-ptt-settings` (`role="group"`, `aria-label="Режим рации"`) скрыта атрибутом `hidden`.
     - **Явная маркировка `[A]` authored adaptation для `#voice-devices-vad-threshold` (`vadThresholdDb`, `-128..=127` дБ) и связь с `devices.sensitivity.slider` (`0..=100` в `voice-spec.json`)**:
       - В RE-дампе (`VOICE.md` §3.4 и `voice-spec.json` → `devices.sensitivity.slider`) чувствительность ввода представлена ползунком `0..=100` (`value_in_dump: 26.109350171958468`, секция `GByLar` «Настройки чувствительности ввода», тумблер `lY6j47` «Автоматически определять чувствительность ввода», API `setNoInputThreshold` `[D]`).
       - В компоненте `V-07-UI` ползунок `#voice-devices-vad-threshold` является **авторской адаптацией `[A]` (отдельным непаритетным контролом инженерной шкалы дБ)**, напрямую отображающим контракт Rust-модели `VoiceInputMode::VoiceActivity { threshold_db: i8 }` (`-128..=127` дБ, шаг `1` дБ, дефолт `DEFAULT_VAD_THRESHOLD_DB = -40` дБ `[S]` `[A]` `[F]`). Значение выводится в `#voice-devices-vad-threshold-value` и `aria-valuetext` в формате `"{vadThresholdDb} дБ"`.
       - **Формула связи со шкалой чувствительности `0..=100` (`[A]`)**: при трансляции между процентной шкалой чувствительности оракула `sensitivityPercent ∈ [0, 100]` (`VOICE.md` §3.4 / `settings.js` `input.sensitivity`, API `setNoInputThreshold`) и порогом `threshold_db: i8` (`devices.rs`) применяется линейное отображение рабочего диапазона `-100..=0` дБ:
         - из `0..=100` в дБ: `thresholdDb = clamp_i8(Math.round(sensitivityPercent) - 100, -128, 127)` (`0%` → `-100` дБ, `26.109%` из дампа → `-74` дБ, `60%` → `-40` дБ = `DEFAULT_VAD_THRESHOLD_DB`, `100%` → `0` дБ);
         - из дБ в `0..=100` (и для позиции маркера `.voice-devices-meter-threshold` на шкале `#voice-devices-mic-meter`): `sensitivityPercent = Math.min(100, Math.max(0, vadThresholdDb + 100))`.
     - При включённом тумблере «Автоматически определять чувствительность ввода» (`lY6j47`, `input.auto_sensitivity` в `settings.js`) ползунок ручного порога получает `aria-disabled="true"` (`VOICE.md` §3.4), а при выключении авто-режима возвращает управление ручному порогу `vadThresholdDb`.
   - **Режим рации (`PushToTalk`)**:
     - При переключении `#voice-devices-ptt` в `checked = true` (`aria-checked="true"`) активный режим становится `{ kind: "push-to-talk", releaseDelayMs: pttReleaseDelayMs }`.
     - Группа `#voice-devices-vad-settings` скрывается атрибутом `hidden`; группа `#voice-devices-ptt-settings` становится видимой (атрибут `hidden` снимается).
     - Ранее настроенное значение `vadThresholdDb` сохраняется в памяти контроллера и восстанавливается без потерь при обратном выключении `#voice-devices-ptt`.

### 3.5. Захват клавиши PTT (`#voice-devices-ptt-key`) и состояние системной блокировки `blocked` (`vxfv7v`)

Кнопка `#voice-devices-ptt-key` (`type="button"`, `aria-label="Горячая клавиша для режима рации"`, ключ `o+BJQR`) реализует конечный автомат назначения клавиши рации и индикации системных ограничений:

| Состояние кнопки | Атрибуты `data-capturing` / `data-ptt-status` | Значение `pttKey` | Видимый текст (`textContent`) | Визуальное оформление (токены `tokens.css`) |
|---|---|---|---|---|
| **Не назначена (по умолчанию)** | `data-capturing="false"`, `data-ptt-status="ready"` | `null` | `Горячие клавиши не назначены` (`nWRdnl`) | Фон `var(--surface-2)`, граница `var(--menu-border) solid var(--border)`, текст `var(--text-muted)` |
| **Ожидание нажатия клавиши (захват)** | `data-capturing="true"`, `data-ptt-status="ready"` | прежнее (`null` или `string`) | `Нажмите клавишу…` | Фон `var(--accent-soft)`, граница `var(--menu-border) solid var(--accent)`, текст `var(--text-strong)`, кольцо фокуса `var(--focus-ring)` |
| **Клавиша назначена** | `data-capturing="false"`, `data-ptt-status="ready"` | непустая строка `key` | `Клавиша: <keyLabel(key)>` (для `" "` → `Клавиша: Пробел`) | Фон `var(--surface-2)`, граница `var(--menu-border) solid var(--border-strong)`, текст `var(--text-strong)` |
| **Заблокирована системой / без фокуса (`blocked`, `vxfv7v`)** `[D]` `[A]` | `data-capturing="false"`, `data-ptt-status="blocked"` (на `#voice-panel`: `data-ptt="blocked"`) | непустая строка `key` (или `null`) | Сохраняет подпись клавиши (`Клавиша: <keyLabel(key)>` или `Горячие клавиши не назначены`); в `#voice-devices-notice` (`role="alert"`) выводится сообщение `vxfv7v` | Кнопка: граница `var(--menu-border) solid var(--warning)`, текст `var(--text-strong)`; баннер `#voice-devices-notice[role="alert"]`: граница `var(--danger)`, текст `var(--danger-text)` |

Правила обработки ввода при захвате клавиши (`data-capturing="true"`) и перехода в состояние `blocked` (`vxfv7v`):
1. **Вход в захват**: клик по `#voice-devices-ptt-key` (или вызов `beginPttKeyCapture()`) переводит кнопку в `data-capturing="true"`, меняет текст на `Нажмите клавишу…` и устанавливает фокус на `#voice-devices-ptt-key`. Повторный клик по кнопке во время захвата отменяет захват (`cancelPttKeyCapture()`).
2. **Перехват `keydown`**: пока `data-capturing="true"`, обработчик `keydown` вызывает `event.preventDefault()`, чтобы нажатие `Space` или `Escape` не прокручивало страницу и не закрывало родительское окно настроек.
3. **Отмена по `Escape` или при потере фокуса во время захвата (`blur` / `visibilitychange`)**: нажатие `Escape`, уход фокуса с окна/кнопки во время активного захвата вызывает `cancelPttKeyCapture()`, возвращает `data-capturing="false"` и восстанавливает предыдущее значение `pttKey` и подпись кнопки **без вызова** `service.setPttKey`.
4. **Игнорирование одиночных модификаторов**: нажатие клавиш из множества `{"Shift", "Control", "Alt", "Meta", "AltGraph", "CapsLock"}` (или пустого `key`) игнорируется — захват остаётся активным (`data-capturing="true"`), `service.setPttKey` не вызывается.
5. **Фиксация рабочей клавиши**: любая другая клавиша (например, `" "` → отображается как `Клавиша: Пробел`, `"KeyV"` / `"v"` → `Клавиша: v`, `"F4"` → `Клавиша: F4`) сохраняется в `pttKey`, переводит `data-capturing="false"`, обновляет текст кнопки и вызывает `service.setPttKey(key)` ровно 1 раз.
6. **Переход в состояние `blocked` (`vxfv7v`) при потере фокуса окна или недоступности `GlobalShortcuts` (`VOICE.md` §3.3, `G5-VOICE-PLAN.md` §5.4, `[D]`/`[A]`)**:
   - Если активен режим `PushToTalk`, но глобальный перехват клавиш ОС недоступен (отсутствует или отклонён XDG Desktop Portal `GlobalShortcuts` в сессии Wayland, нет системных прав доступа к вводу `5rPt+j` / `fAYU2G`, либо окно приложения потеряло фокус в ограниченном режиме `JMyQin` / `VHI4+Y`), PTT переходит в состояние `blocked`:
     - если в момент потери фокуса шёл захват клавиши (`data-capturing="true"`), захват немедленно отменяется (`cancelPttKeyCapture()`);
     - на `#voice-devices-ptt-key` выставляется атрибут `data-ptt-status="blocked"` (а на `#voice-panel` — `data-ptt="blocked"`, `VOICE.md` §3.3);
     - в живой регион `#voice-devices-notice` выводится доступное сообщение о блокировке с `role="alert"`, `aria-live="assertive"` и снятым атрибутом `hidden`, содержащее локализованную строку `vxfv7v` (`[D]`): `«Горячая клавиша режима рации не работает, когда окно не в фокусе или глобальные горячие клавиши недоступны в системе.»` (при работе только в активном окне допускается уточнение ограниченного режима `JMyQin`: `«Режим рации (ограниченный): будет работать, если вкладка Discord активна.»` `VHI4+Y`);
     - в состоянии `blocked` нажатия вне фокуса не активируют передачу (`is_transmitting == false`).
   - При возврате фокуса в окно приложения или успешной регистрации `GlobalShortcuts` состояние `blocked` автоматически снимается (`data-ptt-status="ready"`, на `#voice-panel` — `data-ptt="idle"`), а предупреждение в `#voice-devices-notice` скрывается (`hidden`).

### 3.6. Слайдер задержки отпускания PTT (`#voice-devices-ptt-delay`)

1. **Контракт диапазона**:
   - Элемент: `input#voice-devices-ptt-delay[type="range"][role="slider"]`.
   - Атрибуты: `min="0"`, `max="2000"`, `step="1"`, `aria-label="Задержка отключения в режиме рации"` (`GCNMM8`), `aria-orientation="horizontal"`, `aria-valuemin="0"`, `aria-valuemax="2000"`.
   - Значение по умолчанию по контракту `rust/gui-spike/src/voice_devices.rs` (`rust/crates/discord-voice/tests/devices_contract.rs:315`): `200` мс (`aria-valuenow="200"`, `aria-valuetext="200 мс"`, `#voice-devices-ptt-delay-value` = `"200 мс"`).
2. **Клампинг и синхронизация с моделью передачи**:
   - При событии `input` значение приводится к целому числу в границах `0..=2000` мс: значения `< 0` становятся `0` (`"0 мс"`), значения `> 2000` (например, `5000`) становятся `2000` (`"2000 мс"`).
   - Обновлённое значение немедленно записывается в `pttDelay.value`, `aria-valuenow`, `aria-valuetext`, `pttDelayValue.textContent` (`"{N} мс"`) и передаётся в `service.setInputMode({ kind: "push-to-talk", releaseDelayMs })`.
   - В рантайме (`VoiceDevicesState::is_transmitting`) после отпускания клавиши PTT (`key_down == false`) передача голоса удерживается активной при `now_ms - last_ptt_key_down_ms <= release_delay_ms` и мгновенно прерывается при включении `effective_mute() == true` или отключении микрофона.

### 3.7. Индикатор проверки микрофона (`#voice-devices-mic-test`)

Блок проверки микрофона позволяет пользователю визуально и через скринридер убедиться в поступлении сигнала с выбранного входного устройства без отправки аудио в голосовой канал:

1. **Согласие и приватность (`VOICE.md` §5.1 п.2–3)**:
   - Проверка микрофона запускается **только по явному клику** на кнопку `#voice-devices-mic-test-btn` (`Проверка микрофона`, ключ `gyljWE`). При монтировании панели захват не стартует (`data-test-state="idle"`, `aria-pressed="false"`).
2. **Шкала уровня (`#voice-devices-mic-meter`)**:
   - Семантика: `role="meter"`, `aria-label="Уровень сигнала микрофона"`, `aria-valuemin="-100"`, `aria-valuemax="0"`, `aria-valuenow="{levelDb}"`, `aria-valuetext="{levelDb} дБ"`.
   - Геометрия трека: высота `var(--slider-track-h)` (`8px` `[T]`), радиус `var(--radius-round)` `[T]`, фон `var(--surface-3)` `[T]`.
   - Заполнение `.voice-devices-meter-fill`: ширина пропорциональна `(levelDb + 100) %`, цвет `var(--positive)` `[T]` при превышении порога активации (`levelDb >= vadThresholdDb`) и `var(--warning)` `[T]` при слабом сигнале ниже порога (`levelDb < vadThresholdDb`).
   - Маркер порога `.voice-devices-meter-threshold`: вертикальная риска шириной `var(--focus-ring-width)` (`2px` `[T]`) в позиции `(vadThresholdDb + 100) %`, цвет `var(--text-strong)` `[T]`.
3. **Матрица состояний `#voice-devices-mic-test[data-test-state]`**:

| `data-test-state` | Кнопка `#voice-devices-mic-test-btn` | Текст статуса `#voice-devices-mic-status` | Поведение шкалы `#voice-devices-mic-meter` |
|---|---|---|---|
| `idle` | Текст `Проверка микрофона` (`gyljWE`), `aria-pressed="false"` | `Проверка микрофона. Начните, когда будете готовы.` (`Vd/rEX`) | Заполнение `0%`, `aria-valuenow="-100"`, `aria-valuetext="-100 дБ"` |
| `testing` | Текст `Остановить проверку`, `aria-pressed="true"` | `Говорите в микрофон — уровень сигнала отображается на шкале.` | Заполнение обновляется по входному уровню дБ; переход `transition: width var(--motion-fast) var(--ease-out)` (`0ms` при `prefers-reduced-motion: reduce`) |
| `no-signal` | Текст `Остановить проверку`, `aria-pressed="true"` | `Discord не может обнаружить звук с вашего микрофона. Убедитесь, что вы выбрали правильное устройство ввода.` (`bgn75v` + `MA+OZh`) | Заполнение `0%`, цвет текста статуса `var(--danger-text)` `[T]` |
| `muted` | `aria-disabled="true"` при `effectiveMute === true` или `selectedInput === null` | `Микрофон отключён` (или `Микрофон не выбран`) | Заполнение `0%`, контейнер с `opacity: var(--state-disabled-opacity)` `[T]` |

### 3.8. Инвариант Mute / Deafen (`#voice-devices-user-mute`, `#voice-devices-self-deaf`, `#voice-devices-effective-mute`)

В соответствии с `VoiceDevicesState` (`rust/crates/discord-voice/src/devices.rs:342-385`) и `VOICE.md` §2.4:
1. Эффективный флаг заглушения микрофона вычисляется по формуле `effectiveMute = userMute || selfDeaf`.
2. При включении `#voice-devices-self-deaf` (`selfDeaf = true`):
   - `effectiveMute` автоматически становится `true`;
   - атрибуты `data-effective-mute="true"` выставляются на `#voice-devices` и `#voice-devices-effective-mute`;
   - текст `#voice-devices-effective-mute` меняется на `Микрофон отключён`;
   - явный флаг `userMute` и атрибут `aria-checked` на `#voice-devices-user-mute` **не перезаписываются** (остаются `"false"`, если пользователь не глушил микрофон вручную).
3. При выключении `#voice-devices-self-deaf` (`selfDeaf = false`):
   - `effectiveMute` возвращается к значению явного `userMute` (`false` → `Микрофон активен`, `true` → `Микрофон отключён`).

---

## 4. Спецификация стилей (только токены `rust/gui-spike/ui/tokens.css`)

Все правила ниже используют исключительно переменные, объявленные в `rust/gui-spike/ui/tokens.css`:

```css
/* =============================================================================
 * Панель «Голос и видео» (V-07-UI): устройства, громкость, VAD/PTT, mic-test.
 * 100% token-binding по rust/gui-spike/ui/tokens.css (без сырых цветов и новых токенов).
 * ========================================================================== */

.voice-devices {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: var(--layout-content-max-w);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.voice-devices-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-5);
  align-items: center;
  min-height: var(--input-height);
  max-width: var(--layout-content-max-w);
}

.voice-devices-label {
  overflow: hidden;
  color: var(--text);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Селекты устройств ввода и вывода */
.voice-devices-select {
  width: 100%;
  height: var(--input-height);
  min-width: 0;
  padding: 0 var(--space-5);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-xs);
  background: var(--surface-input);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition:
    border-color var(--motion-fast) var(--ease-out),
    background var(--motion-fast) var(--ease-out);
}

.voice-devices-select:hover:not([aria-disabled="true"]) {
  border-color: var(--border-strong);
}

.voice-devices-select[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

/* Ползунки громкости, порога VAD и задержки PTT */
.voice-devices-range {
  width: 100%;
  height: var(--slider-track-h);
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
  border-radius: var(--radius-round);
  background: var(--surface-3);
  accent-color: var(--accent);
  cursor: pointer;
}

.voice-devices-range[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

.voice-devices-value {
  justify-self: end;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  text-align: right;
}

/* Группы настроек режима активации (VAD / PTT) */
.voice-devices-group {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--space-4) var(--space-5);
  align-items: center;
  padding: var(--space-4);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.voice-devices-group[hidden] {
  display: none;
}

/* Тумблеры (Режим рации, Заглушить, Откл. звук) */
.voice-devices-switch {
   justify-self: end;
  width: var(--switch-w);
  height: var(--switch-h);
  margin: 0;
  border: 0;
  border-radius: var(--radius-round);
  background: var(--surface-3);
  accent-color: var(--accent);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.voice-devices-switch:checked {
  background: var(--accent);
}

/* Кнопка захвата клавиши PTT и кнопка проверки микрофона */
.voice-devices-key,
.voice-devices-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: var(--btn-height);
  padding: 0 var(--btn-pad-x);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--btn-radius);
  background: var(--surface-2);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
  cursor: pointer;
  transition:
    background var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.voice-devices-key:hover,
.voice-devices-action:hover:not([aria-disabled="true"]) {
  background: var(--surface-3);
  border-color: var(--border-strong);
  color: var(--text-strong);
}

.voice-devices-key[data-capturing="true"] {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--text-strong);
}

.voice-devices-key[data-ptt-status="blocked"] {
  border-color: var(--warning);
  color: var(--text-strong);
}

.voice-devices-action[aria-pressed="true"] {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--text-on-accent);
}

.voice-devices-action[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

/* Блок и шкала проверки микрофона */
.voice-devices-mic-test {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.voice-devices-mic-test-controls {
  display: flex;
  gap: var(--space-5);
  align-items: center;
}

.voice-devices-meter {
  position: relative;
  flex: 1;
  height: var(--slider-track-h);
  border-radius: var(--radius-round);
  background: var(--surface-3);
  overflow: hidden;
}

.voice-devices-meter-fill {
  height: 100%;
  border-radius: var(--radius-round);
  background: var(--positive);
  transition: width var(--motion-fast) var(--ease-out);
}

.voice-devices-mic-test[data-test-state="no-signal"] .voice-devices-mic-hint {
  color: var(--danger-text);
}

.voice-devices-meter-threshold {
  position: absolute;
  top: var(--space-0);
  bottom: var(--space-0);
  width: var(--focus-ring-width);
  background: var(--text-strong);
}

.voice-devices-mic-hint {
  margin: var(--space-0);
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

/* Индикатор эффективного состояния микрофона (effectiveMute) */
.voice-devices-effective {
  margin: var(--space-0);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-xs);
  background: var(--surface-1);
  color: var(--positive-text);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-small);
}

.voice-devices-effective[data-effective-mute="true"] {
  color: var(--danger-text);
}

/* Живой регион уведомлений hot-plug (status) и ошибок (alert) */
.voice-devices-notice {
  margin: var(--space-0);
  padding: var(--space-3) var(--space-4);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-xs);
  background: var(--surface-2);
  color: var(--text-subtle);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.voice-devices-notice[role="alert"] {
  border-color: var(--danger);
  color: var(--danger-text);
}

.voice-devices-notice[hidden] {
  display: none;
}

/* Клавиатурный фокус (WCAG 2.2 AA non-text >= 3:1) */
.voice-devices-select:focus-visible,
.voice-devices-range:focus-visible,
.voice-devices-switch:focus-visible,
.voice-devices-key:focus-visible,
.voice-devices-action:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

@media (prefers-reduced-motion: reduce) {
  .voice-devices-select,
  .voice-devices-switch,
  .voice-devices-key,
  .voice-devices-action,
  .voice-devices-meter-fill {
    transition: none;
  }
}
```

### 4.1. Реестр использованных токенов (сверка с `rust/gui-spike/ui/tokens.css`)

Каждый токен в таблице ниже проверен по `rust/gui-spike/ui/tokens.css`:

| Категория | Токены из `rust/gui-spike/ui/tokens.css` |
|---|---|
| **Поверхности (§1)** | `var(--bg-chat)`, `var(--surface-1)`, `var(--surface-2)`, `var(--surface-3)`, `var(--surface-input)` |
| **Текст (§2)** | `var(--text-strong)`, `var(--text)`, `var(--text-subtle)`, `var(--text-muted)`, `var(--text-on-accent)` |
| **Акцент и статусы (§3)** | `var(--accent)`, `var(--accent-soft)`, `var(--positive)`, `var(--positive-text)`, `var(--warning)`, `var(--danger)`, `var(--danger-text)` |
| **Границы, состояния, фокус (§4)** | `var(--border-subtle)`, `var(--border)`, `var(--border-strong)`, `var(--state-disabled-opacity)`, `var(--focus-ring)`, `var(--focus-ring-width)`, `var(--focus-ring-offset)` |
| **Типографика (§5)** | `var(--font-sans)`, `var(--font-mono)`, `var(--font-size-small)`, `var(--font-size-ui)`, `var(--line-height-small)`, `var(--line-height-ui)`, `var(--font-weight-medium)`, `var(--font-weight-semibold)`, `var(--letter-spacing-caps)` |
| **Отступы и размеры (§6, §7, §9, §11)** | `var(--space-0)`, `var(--space-2)`, `var(--space-3)`, `var(--space-4)`, `var(--space-5)`, `var(--space-6)`, `var(--space-9)`, `var(--radius-xs)`, `var(--radius-sm)`, `var(--radius-round)`, `var(--layout-content-max-w)`, `var(--modal-pad-x)`, `var(--input-height)`, `var(--btn-height)`, `var(--btn-pad-x)`, `var(--btn-radius)`, `var(--switch-w)`, `var(--switch-h)`, `var(--slider-track-h)`, `var(--menu-border)` |
| **Движение (§12)** | `var(--motion-fast)`, `var(--ease-out)` |

---

## 5. Клавиатурная навигация, WAI-ARIA и доступность (a11y)

### 5.1. Матрица WAI-ARIA атрибутов

| Элемент (`id`) | Тег / `role` | `aria-label` (ключ `voice-spec.json`) | Обязательные динамические атрибуты |
|---|---|---|---|
| `#voice-devices` | `section` | `"Голос и видео"` | `data-effective-mute="true\|false"` |
| `#voice-devices-input` | `select` | `"Устройство ввода"` (`hHMYbb`) | `aria-disabled="true"` при пустом списке входных устройств |
| `#voice-devices-output` | `select` | `"Устройство вывода"` (`dl18zb`) | `aria-disabled="true"` при пустом списке выходных устройств |
| `#voice-devices-input-volume` | `input[type="range"]`, `role="slider"` | `"Громкость микрофона"` (`OX2Bnr`) | `aria-orientation="horizontal"`, `aria-valuemin="0"`, `aria-valuemax="100"`, `aria-valuenow="{0..100}"`, `aria-valuetext="{0..100}%"` |
| `#voice-devices-output-volume` | `input[type="range"]`, `role="slider"` | `"Громкость динамика"` (`aUJ062`) | `aria-orientation="horizontal"`, `aria-valuemin="0"`, `aria-valuemax="200"`, `aria-valuenow="{0..200}"`, `aria-valuetext="{0..200}%"` |
| `#voice-devices-mic-test-btn` | `button[type="button"]` | `"Проверка микрофона"` (`gyljWE`) | `aria-pressed="true\|false"`, `aria-disabled="true"` при `effectiveMute` или отсутствии микрофона |
| `#voice-devices-mic-meter` | `div`, `role="meter"` | `"Уровень сигнала микрофона"` | `aria-valuemin="-100"`, `aria-valuemax="0"`, `aria-valuenow="{-100..0}"`, `aria-valuetext="{-100..0} дБ"` |
| `#voice-devices-vad-settings` | `div`, `role="group"` | `"Порог активации по голосу"` (`[A]`) | `hidden` при активном режиме `push-to-talk` (`role="group"` обязателен для доступного именованного группирования контейнера `div`) |
| `#voice-devices-vad-threshold` | `input[type="range"]`, `role="slider"` | `"Порог активации по голосу"` (`[A]`) | `aria-orientation="horizontal"`, `aria-valuemin="-128"`, `aria-valuemax="127"`, `aria-valuenow="{-128..127}"`, `aria-valuetext="{-128..127} дБ"`, `aria-disabled="true"` при авто-чувствительности `lY6j47` (`[A]` authored adaptation к `threshold_db: i8` и `0..100` из `voice-spec.json`) |
| `#voice-devices-ptt` | `input[type="checkbox"]`, `role="switch"` | `"Режим рации"` (`tG4Np5`) | `aria-checked="true\|false"` |
| `#voice-devices-ptt-settings` | `div`, `role="group"` | `"Режим рации"` (`tG4Np5`) | `hidden` при активном режиме `voice-activity` (`role="group"` обязателен для доступного именованного группирования контейнера `div`) |
| `#voice-devices-ptt-delay` | `input[type="range"]`, `role="slider"` | `"Задержка отключения в режиме рации"` (`GCNMM8`) | `aria-orientation="horizontal"`, `aria-valuemin="0"`, `aria-valuemax="2000"`, `aria-valuenow="{0..2000}"`, `aria-valuetext="{0..2000} мс"` |
| `#voice-devices-ptt-key` | `button[type="button"]` | `"Горячая клавиша для режима рации"` (`o+BJQR`) | `data-capturing="true\|false"`, `data-ptt-status="ready\|blocked"` (при потере фокуса / недоступности `GlobalShortcuts` → `"blocked"` + сообщение `vxfv7v` в `#voice-devices-notice` с `role="alert"`) |
| `#voice-devices-user-mute` | `input[type="checkbox"]`, `role="switch"` | `"Заглушить"` (`sWmtI6`) | `aria-checked="true\|false"` |
| `#voice-devices-self-deaf` | `input[type="checkbox"]`, `role="switch"` | `"Откл. звук"` (`hK51Yg`) | `aria-checked="true\|false"` |
| `#voice-devices-effective-mute` | `p` | — | `aria-live="polite"`, `data-effective-mute="true\|false"` |
| `#voice-devices-notice` | `p`, `role="status"` или `role="alert"` | — | `aria-live="polite"` (для `status` при `DeviceFallback`), `aria-live="assertive"` (для `alert` при `NoDeviceAvailable`, блокировке PTT `vxfv7v`, отказе выбора или ошибке сервиса), `hidden` при отсутствии сообщения |

### 5.2. Порядок обхода `Tab` и клавиатурное управление

1. **Последовательность фокуса (`Tab` / `Shift+Tab`)**:
   - В режиме `voice-activity` (когда `#voice-devices-ptt-settings` скрыт `hidden`):
     `#voice-devices-input` → `#voice-devices-output` → `#voice-devices-input-volume` → `#voice-devices-output-volume` → `#voice-devices-mic-test-btn` → `#voice-devices-vad-threshold` → `#voice-devices-ptt` → `#voice-devices-user-mute` → `#voice-devices-self-deaf`.
   - В режиме `push-to-talk` (когда `#voice-devices-vad-settings` скрыт `hidden`, а `#voice-devices-ptt-settings` открыт):
     `#voice-devices-input` → `#voice-devices-output` → `#voice-devices-input-volume` → `#voice-devices-output-volume` → `#voice-devices-mic-test-btn` → `#voice-devices-ptt` → `#voice-devices-ptt-delay` → `#voice-devices-ptt-key` → `#voice-devices-user-mute` → `#voice-devices-self-deaf`.
   - Скрытые через атрибут `hidden` группы (`#voice-devices-vad-settings` или `#voice-devices-ptt-settings` с `role="group"`) автоматически исключаются из tab-порядка браузера.
2. **Клавиатурные команды контролов**:
   - `<select>` (`#voice-devices-input`, `#voice-devices-output`): `ArrowUp` / `ArrowDown` — выбор устройства, `Space` / `Enter` / `Alt+ArrowDown` — раскрытие списка.
   - `input[type="range"][role="slider"]` (громкость, порог VAD, задержка PTT): `ArrowRight` / `ArrowUp` (`+1`), `ArrowLeft` / `ArrowDown` (`-1`), `Home` (`min`), `End` (`max`).
   - `input[type="checkbox"][role="switch"]` (`#voice-devices-ptt`, `#voice-devices-user-mute`, `#voice-devices-self-deaf`): `Space` — переключение состояния и синхронное обновление `aria-checked`.
   - `#voice-devices-ptt-key`:
     - при `data-capturing="false"`: `Enter` или `Space` переводит кнопку в режим захвата клавиши (`data-capturing="true"`);
     - при `data-capturing="true"`: `Escape` или потеря фокуса (`blur` / `visibilitychange`) отменяет захват без изменения привязки; одиночные модификаторы (`Shift`, `Control`, `Alt`, `Meta`, `AltGraph`, `CapsLock`) игнорируются; любая иная клавиша назначается как `pttKey` и завершает захват (`data-capturing="false"`);
     - при потере фокуса окна в режиме `PushToTalk` или недоступности системного портала `GlobalShortcuts`: выставляется `data-ptt-status="blocked"` (на `#voice-panel` — `data-ptt="blocked"`) и в `#voice-devices-notice` (`role="alert"`, `aria-live="assertive"`) озвучивается предупреждение о блокировке `vxfv7v`.
   - Глобальное закрытие экрана настроек по `Escape`: если `#voice-devices-ptt-key` находится в состоянии `data-capturing="true"`, нажатие `Escape` перехватывается кнопкой (`event.preventDefault()`) и отменяет только захват клавиши PTT, **не закрывая** окно настроек `#screen-settings`.

---

## 6. Матрица верификации контракта (`V-07-UI` / `devices_contract.rs`)

| ID проверки | Проверяемое требование спецификации | Ожидаемый результат | Тестовый оракул |
|---|---|---|---|
| **T-V07-UI-01** | Начальная инициализация устройств и громкости | Выбираются устройства с `isDefault === true` (независимо от позиции в списке), громкость ввода `100%` (`0..100`), вывода `100%` (`0..200`), `events = []` | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_01` |
| **T-V07-UI-02** | Длинные названия устройств | Полная строка `label` сохраняется в `option.textContent` и `option.title` без обрезки в DOM | `tests/unit/voice-devices-settings.test.mjs` |
| **T-V07-UI-03** | Явный выбор и защита от `WrongKind` / `NotFound` | Валидный выбор вызывает `service.selectInput`/`selectOutput` 1 раз; чужой `kind` или неизвестный `id` возвращает `false`, не меняет выбор и показывает `role="alert"` | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_01` |
| **T-V07-UI-04** | Hot-plug при сохранении активного устройства | Добавление/удаление других устройств не сбрасывает активный выбор, не вызывает `service.select*` и не эмитит событий | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_02` |
| **T-V07-UI-05** | Hot-plug с отключением активного устройства (согласование Fallback `update_device_list` и `VOICE.md` §3.2) | Локальная реконсиляция модели выставляет кандидат отката на `isDefault === true` (или первое доступное) **без вызова `service.select*` (`0 вызовов`)**, эмитит `DeviceFallback` и показывает `#voice-devices-notice` (`role="status"`, `aria-live="polite"`); в звонке `#voice-panel` отражает `device-missing` (`MA+OZh`) без разрыва разговора | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_02` |
| **T-V07-UI-06** | Hot-plug при полном отключении устройств направления | `selected* = null`, `aria-disabled="true"`, опция `Микрофон не выбран` / `Устройство вывода не выбрано`, эмиссия `NoDeviceAvailable`, показ `#voice-devices-notice` (`role="alert"`, `aria-live="assertive"`), `0 вызовов` `service.select*` | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_02` |
| **T-V07-UI-07** | Клампинг громкости микрофона (`0..100`) и вывода (`0..200`) | Значения `< 0` и `> max` клампятся до `0` и `100`/`200`, синхронно обновляют `aria-valuenow`, `aria-valuetext`, `<output>` и передаются в `service` | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_05` |
| **T-V07-UI-08** | Переключение `VoiceActivity` ↔ `PushToTalk`, `role="group"`, `[A]` шкала `vadThresholdDb` (`-128..=127` дБ ↔ `0..100`) и задержка PTT `0..=2000` мс | `#voice-devices-ptt` переключает видимость именованных групп `#voice-devices-vad-settings[role="group"]` и `#voice-devices-ptt-settings[role="group"]`, сохраняет `vadThresholdDb` (`[A]` `-128..127` дБ, дефолт `-40` дБ, связь с чувствительностью `0..100`: `thresholdDb = clamp_i8(round(pct) - 100, -128, 127)`) и клампит `releaseDelayMs` в `0..2000` мс (дефолт `200` мс по `voice_devices.rs`) | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_04` |
| **T-V07-UI-09** | Захват клавиши PTT (`#voice-devices-ptt-key`) и состояние `blocked` (`vxfv7v`) | Клик включает `data-capturing="true"`, `Escape` / потеря фокуса отменяет без вызова сервиса, `Shift`/`Control`/`Alt`/`Meta` игнорируются, `" "` назначается как `Клавиша: Пробел`; при потере фокуса окна / недоступности `GlobalShortcuts` выставляется `data-ptt-status="blocked"` (`#voice-panel[data-ptt="blocked"]`) и выводится `role="alert"` с текстом `vxfv7v` | `tests/unit/voice-devices-settings.test.mjs` |
| **T-V07-UI-10** | Инвариант `effectiveMute = userMute \|\| selfDeaf` | Включение `selfDeaf` выставляет `effectiveMute = true`, не меняя `userMute` и `aria-checked` микрофона; снятие `selfDeaf` восстанавливает `userMute` | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_03` |
| **T-V07-UI-11** | Приватность и запрет медиа-API | Отсутствие вызовов `console.*`, `navigator.mediaDevices`, `getUserMedia`, `getDisplayMedia`, `startLocalAudioRecording`, `startRecordingRawSamples`, `setAecDump`, `setLoopback`, `fetch`, `WebSocket`; `Debug` в Rust маскирует `label` как `<redacted>` | `tests/unit/voice-devices-settings.test.mjs`, `t_v07_05` |
