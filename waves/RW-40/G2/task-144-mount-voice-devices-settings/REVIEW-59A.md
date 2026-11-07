# Техническое ревью (Review 59A): task-144 (`FE-P0-02` — Mount `createVoiceDevicesSettingsController` in `settings.js`)

- **Ревьюер**: Технический ревьюер (`Review 59A`)
- **Карточка / Задача**: `task-144` (`FE-P0-02: Mount createVoiceDevicesSettingsController in settings.js`)
- **Волна / Отдел**: `RW-40` / `G2 Frontend`
- **Ворктри**: `/home/sanya/orca/workspaces/project/feature-task-144-mount-voice-devices-settings` (`HEAD = 4e24048`)
- **Проверенные артефакты и спецификации**:
  - `/home/sanya/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/REPORT.md`
  - `/home/sanya/orca/project/waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (§1–§6)
  - `/home/sanya/orca/project/waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` (§3.2)
  - `/home/sanya/orca/project/waves/RW-40/DESIGN/voice-spec.json`
  - `rust/crates/discord-voice/src/devices.rs`
- **Вердикт**: **PASS / Ready for Merge** (`P0 = 0`, `P1 = 0`, `P2 = 0`, `P3 = 2`)

---

## 1. Аудит файловых границ и контрольных сумм (MD5)

Воркер изменил строго назначенные файлы карточки. Общие файлы (`rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/tokens.css`) и golden-каталоги (`logs/`, `functions/`, `evidence/`) не затрагивались.

| Файл | Статус в `git status -s` | MD5 (исходный = после мутаций) | Статус сверки |
|---|---|---|---|
| `rust/gui-spike/ui/features/settings.js` | `M` | `12144dbf2d4ce592b8c9dc91f5e80138` | Совпадает с `REPORT.md` |
| `rust/gui-spike/ui/features/voice-devices-settings.js` | `??` (в worktree) | `dce9faaca993a0ae72f2f5a3a4746d87` | Совпадает с `REPORT.md` |
| `tests/unit/voice-devices-settings.test.mjs` | `??` (в worktree) | `8f09e2751e110df44496c2c4ee0063dc` | Совпадает с `REPORT.md` |

---

## 2. Независимый прогон тестов и TypeScript (Шаг 1)

Выполнены обязательные тестовые наборы и проверки типов:

1. **Профильные и смежные unit-тесты**:
   ```sh
   node --test tests/unit/voice-devices-settings.test.mjs \
               tests/unit/gui-spike-dom.test.mjs \
               tests/unit/gui-spike-features.test.mjs
   ```
   - `tests/unit/voice-devices-settings.test.mjs`: **23/23 pass** (`17` тестов `V-07` + `6` новых тестов `FE-P0-02 / task-144`).
   - `tests/unit/gui-spike-dom.test.mjs`: **29/29 pass**.
   - `tests/unit/gui-spike-features.test.mjs`: **100/100 pass**.
   - **Итого**: `tests 152 | suites 17 | pass 152 | fail 0 | skipped 0 | todo 0` (`exit 0`).

2. **Мета-тесты политики и безопасности IPC**:
   ```sh
   node --test tests/meta/ipc-path.test.mjs tests/meta/test-policy.test.mjs
   ```
   - **Итого**: `tests 29 | suites 5 | pass 29 | fail 0` (`exit 0`).

3. **TypeScript (`// @ts-check`)**:
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json` → **0 ошибок** (`exit 0`).
   - В `npx tsc -p tsconfig.json` по файлам `rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/ui/features/voice-devices-settings.js` и `tests/unit/voice-devices-settings.test.mjs` — **0 ошибок** (единственные предупреждения в worktree от `4e24048` относятся к нетронутому чужому файлу `tests/unit/dispatch_ui.test.mjs`, который уже исправлен в `/home/sanya/orca/project/tests/unit/dispatch_ui.test.mjs`, см. `P3-2`).

4. **Проверка Red-First артефактов**:
   - Проверены `red.log` (`tests 23 | pass 17 | fail 6`, `exit 1` до реализации) и `green.log` (`tests 23 | pass 23 | fail 0`, `exit 0` после реализации) в `/home/sanya/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/`. Порядок Strict TDD соблюдён.

---

## 3. Сверка инвариантов `SET-04..SET-12`, `INT-07` и жизненного цикла монтирования (Шаг 2)

| Инвариант / Требование | Проверка реализации в `settings.js` и `voice-devices-settings.js` | Статус |
|---|---|---|
| **SET-04** (`10` блоков `h4.settings-block-title`, `3` группы `h3`) | В `[data-pane="voice"]` сохранены ровно 10 заголовков `h4.settings-block-title` в порядке дампа; контейнер монтирования `div.settings-voice-devices-mount[data-voice-devices-root="true"]` не добавляет `h3`/`h4`. | **PASS** |
| **SET-05..SET-08** (`38` тумблеров `.toggle` / `input[type="checkbox"][role="switch"]`, `34` вкл / `4` выкл, `22 = 4 + 18` звуковых) | В режиме привязки (`boundMode = controls !== null`) `#voice-devices-ptt` привязывается к существующему тумблеру `voice.ptt_mode`, а `#voice-devices-user-mute` и `#voice-devices-self-deaf` рендерятся как `<button class="voice-devices-switch" type="button" aria-pressed="false" aria-checked="false">`. Число `.toggle` и `input[type="checkbox"]` в `#settings` остаётся строго `38`. | **PASS** |
| **SET-09** (`4` элемента `<select>` в `#settings`) | `inputSelect` (`#voice-devices-input`), `outputSelect` (`#voice-devices-output`) и `videoSelect` (`#voice-devices-video`) привязываются к трём существующим `<select>` вкладки «Голос и видео» (Микрофон, Динамик, Камера); четвёртый `<select>` — «Шумоподавление» (`Krisp` / `Стандартное`). Итого в `#settings` ровно `4` `<select>`. | **PASS** |
| **SET-10** (`4` ползунка `input[type="range"]` и `4` подписи `.settings-slider-value`: `["100 %", "100 %", "49 %", "26 %"]`) | `inputVolumeRange` (`0..100`) и `outputVolumeRange` (`0..200`) привязываются к существующим `input[type="range"]`, а дополнительные слайдеры `#voice-devices-vad-threshold` и `#voice-devices-ptt-delay` в `boundMode` создаются как `<div role="slider" tabindex="0">` с readout-узлами `output.voice-devices-value` (без класса `.settings-slider-value`). | **PASS** |
| **SET-11** (радиогруппа «Профиль ввода» из `3` `[role="radio"]`) | Радиогруппа `Изоляция голоса` / `Студия` / `Пользовательский` (`role="radiogroup"`, 1 активный `aria-checked="true"`) сохранена без изменений. | **PASS** |
| **SET-12** (кнопки действий `.settings-action >= 5` и переход в «Уведомления») | Кнопка `Проверка микрофона` (`#voice-devices-mic-test-btn`) переиспользует существующую кнопку `.settings-action` из блока «Голос», снимая с неё заглушку `aria-disabled` при монтировании контроллера. Ссылка `settings-related-link` («Уведомления») переключает вкладку и размонтирует контроллер. | **PASS** |
| **INT-07** (совместимость со standalone `#voice-devices` и отсутствие неразрешённых IPC-вызовов) | Функция `createVoiceDevicesSettings` (без `controls`) полностью сохраняет контракт `V-07` / `INT-07` (`vadMinDb = -128`, `vadMaxDb = 127`, нативные `<input type="range">` и `<input type="checkbox" role="switch">`). Ни `settings.js`, ни `voice-devices-settings.js` не вызывают незарегистрированные Tauri IPC-команды. | **PASS** |
| **Монтирование при открытии `voice` / `voice-video` / `Голос и видео`** | `PANE_BY_SECTION` отображает `"Голос и видео"`, `"voice"` и `"voice-video"` в ключ `"voice"`. В `showPane(key)` при `pane === "voice"` вызывается `mountVoiceDevices()`, создающий `createVoiceDevicesSettingsController` с порогами `-100..0` дБ (`MIN_UI_VAD_THRESHOLD_DB..MAX_UI_VAD_THRESHOLD_DB`). При повторном открытии через `[data-action="settings-open"], [data-screen="settings"]` на активной вкладке `voice` контроллер также монтируется. | **PASS** |
| **Вызов `destroy()` при смене вкладки или закрытии модалки** | При переходе на любую другую вкладку (`showPane("appearance")`, `showPane("notifications")`, клик по пункту `#settings-nav`), а также при вызове `settings.close()`, `settings.destroy()` или клике по `[data-action="settings-close"]` вызывается `unmountVoiceDevices()`, который вызывает `voiceDevicesController.destroy()` (останавливая активный `micTesting` через `stopMicTest()` и очищая `mountContainer`) и обнуляет ссылку `voiceDevicesController = null`. | **PASS** |

---

## 4. Мутационное тестирование (Шаг 3: M1..M5)

Выполнены 5 независимых мутаций в `rust/gui-spike/ui/features/settings.js` и `rust/gui-spike/ui/features/voice-devices-settings.js`. Лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-144-mount-voice-devices-settings/review-59a-mutations.log`.

| ID | Файл | Описание мутации | MD5 мутанта | Код выхода | Статус | Убивший тест / Assertion |
|---|---|---|---|---:|---|---|
| **M1** | `settings.js` | Удаление ветки `else unmountVoiceDevices();` в `showPane()` при смене вкладки | `374121351dab42b4e3843f73ccc7ed0e` | `1` | **KILLED** | `AssertionError: при уходе на Внешний вид контроллер уничтожается` / `destroy() вызван при смене вкладки` (`fail 2`) |
| **M2** | `settings.js` | Удаление `unmountVoiceDevices();` из `close()` при закрытии модалки | `136b8afe9a61b37252665b2e70d633a4` | `1` | **KILLED** | `AssertionError: destroy() вызван при close() модалки настроек` (`fail 1`) |
| **M3** | `settings.js` | Удаление алиасов `voice: "voice"` и `"voice-video": "voice"` из `PANE_BY_SECTION` | `c9166831a8dd276bd82c2c5a77055927` | `1` | **KILLED** | `AssertionError: showPane('voice') должен смонтировать контроллер` (`fail 1`) |
| **M4** | `voice-devices-settings.js` | Удаление дефолтных UI-границ `vadMinDb: -100, vadMaxDb: 0` в `createVoiceDevicesSettingsController` | `fa5cd4b8d7ba9414c4415e455dfce511` | `1` | **KILLED** | `переключатель VAD/PTT и порог чувствительности -100..0 дБ работают в панели настроек` (`fail 1`) |
| **M5** | `voice-devices-settings.js` | Удаление `if (micTesting) stopMicTest();` внутри `destroy()` | `5ed6d1761b4cc3cd54da53041b1a45c8` | `1` | **KILLED** | `AssertionError: проверка микрофона остановлена при destroy()` (`fail 1`) |

- **Итог мутаций**: **5 / 5 KILLED (100%)**.
- После завершения всех мутаций исходные файлы восстановлены побайтово, контрольные суммы MD5 сверены (`12144dbf2d4ce592b8c9dc91f5e80138`, `dce9faaca993a0ae72f2f5a3a4746d87`, `8f09e2751e110df44496c2c4ee0063dc`), повторный прогон 152 тестов — `pass 152, fail 0`.

---

## 5. Замечания по приоритетам (P0 / P1 / P2 / P3)

- **P0 (Блокеры / нарушение контрактов или TEST-POLICY)**: 0
- **P1 (Критические ошибки логики / утечки состояния / нарушение инвариантов)**: 0
- **P2 (Существенные недочёты)**: 0
- **P3 (Некритические рекомендации / косметика)**:
  1. **P3-1 (`voice-devices-settings.js:932-982`, очистка слушателей на постоянных `controls` при `destroy()`)**: в режиме `boundMode` обработчики событий (`change`, `input`, `click`), навешанные на долгоживущие DOM-узлы вкладки (`voiceInputSelect`, `voiceOutputSelect`, `voiceVideoSelect`, `voiceInputVolumeRange`, `voiceOutputVolumeRange`, `voicePttToggle`, `voiceMicTestButton`), при вызове `destroy()` блокируются проверкой `if (destroyed) return;`, но сами функции-слушатели не снимаются через `removeEventListener` / `AbortController`. На функциональность и тесты это не влияет (побочных вызовов нет), однако при дальнейшем рефакторинге рекомендуется передавать `{ signal: abortController.signal }` в `addEventListener` и вызывать `abortController.abort()` в `destroy()`.
  2. **P3-2 (`tests/unit/dispatch_ui.test.mjs` в базовом коммите `4e24048`)**: в локальном worktree карточки присутствует до-базовая версия чужого файла `tests/unit/dispatch_ui.test.mjs` с 16 предупреждениями `TS7006`/`TS2339`, которая уже исправлена в основном дереве `/home/sanya/orca/project/tests/unit/dispatch_ui.test.mjs`. В файлах самой карточки `task-144` и в `rust/gui-spike/ui/tsconfig.json` ошибок типизации `0`.

---

## 6. Итоговое заключение

Карточка **task-144 (`FE-P0-02`)** выполнена в полном соответствии со спецификацией, сохраняет все замороженные инварианты `SET-04..SET-12` и `INT-07`, покрыта строгими Red-First тестами и проходит 5/5 мутационных проверок.

**Статус**: **PASS / Ready for Merge**.
