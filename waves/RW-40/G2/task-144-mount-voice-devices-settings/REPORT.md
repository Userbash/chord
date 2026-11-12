# Отчёт воркера: task-144 (`FE-P0-02` Mount Voice & Video Settings)

- **Batch / Волна**: `RW-40` (Отдел G2 Frontend)
- **Карточка**: `task-144` / `FE-P0-02` (`waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`, `waves/RW-40/RE-AUDIT/RE-GAP-FRONTEND-UI.md`)
- **Спецификация и оракулы**:
  - `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (§2, §3.1–§3.5, §5, §8)
  - `waves/RW-40/DESIGN/voice-spec.json` (`devices.sliders`: `OX2Bnr` `0..100 = 100`, `aUJ062` `0..200 = 100`, `copy`)
  - `rust/crates/discord-voice/src/devices.rs` (`DEFAULT_INPUT_VOLUME`, `MAX_INPUT_VOLUME`, `DEFAULT_OUTPUT_VOLUME`, `MAX_OUTPUT_VOLUME`, `DEFAULT_VAD_THRESHOLD_DB`)
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-144-mount-voice-devices-settings` (`HEAD = 4e24048`)

---

## 1. Что сделано

1. **Red-first тесты (`tests/unit/voice-devices-settings.test.mjs`)**:
   - Добавлен suite `FE-P0-02 / task-144: монтирование createVoiceDevicesSettingsController в settings.js` из 6 профильных тестов (в дополнение к 17 существующим тестам `V-07`, итого 23 теста):
     - экспорт `createVoiceDevicesSettingsController` и его автоматическое монтирование при открытии вкладки «Голос и видео» (`"Голос и видео"`, `"voice"`, `"voice-video"`);
     - инициализация и выбор списков устройств ввода (`#voice-devices-input`), вывода (`#voice-devices-output`) и видео (`#voice-devices-video`) с выбором `isDefault`-устройств и пробросом `selectInput` / `selectOutput` / `selectVideo` в сервис;
     - инициализация и клампинг ползунков громкости микрофона (`0..100`, `#voice-devices-input-volume`) и динамика (`0..200`, `#voice-devices-output-volume`);
     - работа переключателя режима активации VAD/PTT (`#voice-devices-ptt`, `#voice-devices-vad-settings`, `#voice-devices-ptt-settings` с `role="group"`) и порога чувствительности `-100..0` дБ (`#voice-devices-vad-threshold`, по умолчанию `-40` дБ);
     - проверка микрофона (`#voice-devices-mic-test`, `#voice-devices-mic-test-btn`, `#voice-devices-mic-meter` с `role="meter"` `-100..0` дБ, `#voice-devices-mic-status`) со стартом/остановкой и автоматическим сбросом при `destroy()`;
     - корректный вызов `destroy()` контроллера при смене вкладки настроек (`showPane("appearance")`, `showPane("notifications")`, клик по пункту `#settings-nav`) и при закрытии модалки настроек (`settings.close()`, `settings.destroy()`, клик по `[data-action="settings-close"]`).
2. **Адаптер и расширение контроллера (`rust/gui-spike/ui/features/voice-devices-settings.js`)**:
   - Экспортирован `createVoiceDevicesSettingsController` (с UI-шкалой порога чувствительности `MIN_UI_VAD_THRESHOLD_DB = -100` .. `MAX_UI_VAD_THRESHOLD_DB = 0` дБ по умолчанию; `createVoiceDevicesSettings` сохраняет `i8`-диапазон `-128..127` дБ для обратной совместимости с `V-07`).
   - Добавлена поддержка видеоустройств (`kind: "video"`, `#voice-devices-video`, `selectVideo`, `selectedVideo` в `getState()`), секции проверки микрофона (`#voice-devices-mic-test`, `#voice-devices-mic-test-btn`, `#voice-devices-mic-meter`, `#voice-devices-mic-status`, `startMicTest`, `stopMicTest`, `setMicTestLevelDb`), `isDestroyed()` и режима привязки `controls` к существующим DOM-контролам `settings.js`.
3. **Монтирование в `rust/gui-spike/ui/features/settings.js`**:
   - В `createSettingsConsole` добавлено монтирование `createVoiceDevicesSettingsController` при показе вкладки «Голос и видео» (`"Голос и видео"`, `"voice"`, `"voice-video"`) и размонтирование (`destroy()`) при переходе на другие вкладки (`"Внешний вид"` / `"appearance"`, `"Уведомления"` / `"notifications"`) или закрытии модалки настроек (`close()`, `destroy()`, `[data-action="settings-close"]`).
   - Привязка к существующим контролам вкладки `[data-pane="voice"]` сохраняет все замороженные инварианты RW-38/RW-39 (`38` чекбоксов `.toggle` / `[role="switch"]`, `4` `select`, `4` `input[type="range"]`, `4` `.settings-slider-value`, `10` блоков `h4.settings-block-title`).

---

## 2. Red-прогон (до реализации)

- **Команда**: `node --test tests/unit/voice-devices-settings.test.mjs`
- **Код выхода**: `1` (`tests 23, pass 17, fail 6`)
- **Лог**: `/home/user/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/red.log`

Выдержка из `red.log`:
```text
▶ FE-P0-02 / task-144: монтирование createVoiceDevicesSettingsController в settings.js
  ✖ экспортирует createVoiceDevicesSettingsController и монтирует его при открытии вкладки «Голос и видео» (voice / voice-video) (0.659889ms)
  ✖ инициализирует списки устройств ввода, вывода и видео (input / output / video) (1.382198ms)
  ✖ инициализирует ползунки громкости микрофона 0..100 и динамика 0..200 в панели настроек (0.44199ms)
  ✖ переключатель VAD/PTT и порог чувствительности -100..0 дБ работают в панели настроек (0.626399ms)
  ✖ проверка микрофона (mic test): старт/стоп кнопкой, индикатор уровня -100..0 дБ и авто-стоп при destroy() (0.335639ms)
  ✖ вызывает destroy() контроллера при смене вкладки настроек или закрытии модалки настроек (0.44897ms)
✖ FE-P0-02 / task-144: монтирование createVoiceDevicesSettingsController в settings.js (4.119935ms)
ℹ tests 23 | pass 17 | fail 6
```

---

## 3. Green-прогон (после реализации)

- **Команда**: `node --test tests/unit/voice-devices-settings.test.mjs`
- **Код выхода**: `0` (`tests 23, pass 23, fail 0`)
- **Лог**: `/home/user/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/green.log`

Выдержка из `green.log`:
```text
✔ V-07 UI: устройства и режим активации («Голос и видео») (9.524785ms)
▶ FE-P0-02 / task-144: монтирование createVoiceDevicesSettingsController в settings.js
  ✔ экспортирует createVoiceDevicesSettingsController и монтирует его при открытии вкладки «Голос и видео» (voice / voice-video) (1.771549ms)
  ✔ инициализирует списки устройств ввода, вывода и видео (input / output / video) (0.904149ms)
  ✔ инициализирует ползунки громкости микрофона 0..100 и динамика 0..200 в панели настроек (0.79498ms)
  ✔ переключатель VAD/PTT и порог чувствительности -100..0 дБ работают в панели настроек (0.869149ms)
  ✔ проверка микрофона (mic test): старт/стоп кнопкой, индикатор уровня -100..0 дБ и авто-стоп при destroy() (1.04327ms)
  ✔ вызывает destroy() контроллера при смене вкладки настроек или закрытии модалки настроек (0.970579ms)
✔ FE-P0-02 / task-144: монтирование createVoiceDevicesSettingsController в settings.js (6.537536ms)
ℹ tests 23 | pass 23 | fail 0
```

---

## 4. Targeted & TypeScript проверки

- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → **exit 0** (чисто)
- `npx tsc -p tsconfig.json` → **exit 0** по всем файлам задачи (в ветке worktree от `4e24048` устаревшая копия чужого файла `tests/unit/dispatch_ui.test.mjs` содержала до-базовые TS-предупреждения, исправленные в `/home/user/orca/project/tests/unit/dispatch_ui.test.mjs`; с актуальным `dispatch_ui.test.mjs` полный `npx tsc -p tsconfig.json` даёт `exit 0`).
- Смежные DOM/feature-инварианты:
  - `node --test tests/unit/gui-spike-dom.test.mjs` → **29/29 pass** (`exit 0`)
  - `node --test tests/unit/gui-spike-features.test.mjs` → **100/100 pass** (`exit 0`)

---

## 5. Проверка мутациями (TEST-POLICY §5)

Проведены 4 целевые мутации с фиксацией в `/home/user/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/mutations.log`:
1. **M1 (`settings.js`)**: пропуск `unmountVoiceDevices()` при смене вкладки в `showPane()` → **KILLED** (`exit=1`, `AssertionError: при уходе на Внешний вид контроллер уничтожается`).
2. **M2 (`settings.js`)**: пропуск `unmountVoiceDevices()` при закрытии модалки в `close()` → **KILLED** (`exit=1`, `AssertionError: destroy() вызван при close() модалки настроек`).
3. **M3 (`voice-devices-settings.js`)**: отсутствие UI-границ `-100..0` дБ в `createVoiceDevicesSettingsController` → **KILLED** (`exit=1`).
4. **M4 (`voice-devices-settings.js`)**: пропуск `stopMicTest()` внутри `destroy()` → **KILLED** (`exit=1`).

После прогона мутаций файлы восстановлены до байта, MD5 сверены.

---

## 6. Изменённые файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/settings.js` | modified | `12144dbf2d4ce592b8c9dc91f5e80138` |
| `rust/gui-spike/ui/features/voice-devices-settings.js` | modified/added | `dce9faaca993a0ae72f2f5a3a4746d87` |
| `tests/unit/voice-devices-settings.test.mjs` | modified/added | `8f09e2751e110df44496c2c4ee0063dc` |
| `/home/user/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/REPORT.md` | report | — |

Файлы `rust/gui-spike/ui/app.js` и `rust/gui-spike/ui/index.html` **не затрагивались**.

---

## 7. Риски и открытые вопросы

- Нет блокеров. Режимы `createVoiceDevicesSettings` (standalone `#voice-devices` для `V-07` и `INT-07`) и `createVoiceDevicesSettingsController` (интеграция в `[data-pane="voice"]` внутри `settings.js` с сохранением всех инвариантов `SET-04..SET-12`) полностью совместимы и покрыты unit-тестами.
