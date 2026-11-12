# Ревью-отчёт: task-154 (`FE-P1-02` Go Live & Stream Tiles UI)

- **Роль**: Технический ревьюер (`Review 69A`)
- **Волна / Отдел**: `RW-40` / `G2 Frontend`
- **Карточка**: `task-154` (`FE-P1-02`: Go Live & Stream Tiles UI в `voice.js`)
- **Проверенные файлы**:
  - `rust/gui-spike/ui/features/voice.js` (MD5: `652f974268db0f7ddee22304fd2a692c`)
  - `tests/unit/voice_ui.test.mjs` (MD5: `dd4a96a1d3c56644ea168e09f549fee7`)
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-154-voice-go-live-ui`

---

## 1. Результаты проверок

### 1.1. Тестовый прогон (Node Test Runner)
- **Команда**: `node --test tests/unit/voice_ui.test.mjs tests/unit/gui-spike-dom.test.mjs`
- **Результат**: **85/85 pass** (0 fail, 0 skipped, 9 suites).
  - Сюита `FE-P1-02 / task-154`: 12/12 pass.
  - Сюиты `V-08 voice`: 44/44 pass.
  - Сюита `gui-spike-dom`: 29/29 pass.

### 1.2. Проверка типов TypeScript
- **Команды**:
  - `npx tsc -p tsconfig.json` -> **0 ошибок** (код выхода 0).
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json` -> **0 ошибок** (код выхода 0).

---

## 2. Анализ реализации и соответствия спецификации

1. **Сетка видеоплиток `#voice-stage-grid`**:
   - Формируются плитки `.voice-tile` для участников с корректными атрибутами `data-user-id`, `role="listitem"`.
   - Активная говорящая рамка: класс `.is-speaking` и атрибут `data-speaking="true"` выставляются при `speaking=true` и снимаются при сбросе.
   - Стриминговая плитка: класс `.is-stream`, `data-is-stream="true"`, бейдж `#stream-live-badge` (`.badge-live`) с текстом «В ЭФИРЕ».
   - Фокусный режим: клик по плитке открывает `#voice-stage-focused`, выставляет `data-focused="true"`, `data-focused-user-id`, на активной плитке класс `.is-focused` и `aria-pressed="true"`. Переключение на другую плитку корректно обновляет состояние; повторный клик снимает фокус.

2. **Модальное окно запуска стрима `#go-live-modal`**:
   - Семантика доступности: `role="dialog"`, `aria-modal="true"`, `aria-labelledby="go-live-modal-title"`.
   - Вкладки источников: `#go-live-tab-apps` («Приложения») и `#go-live-tab-screens` («Экраны») с переключением `aria-selected` и `is-active`.
   - Настройки качества: выбор разрешения (`720p`, `1080p`, `1440p`) и FPS (`30`, `60`) с `role="radiogroup"`, `role="radio"` и `aria-checked`.
   - Кнопка «Прямой эфир» (`#go-live-submit-btn`): вызывает `voice_stream_create` / `onStartStream` с выбранными параметрами и скрывает модалку.

3. **Управление стримом**:
   - Просмотр стрима: `.stream-watch-btn` (`data-stream-action="watch"`) вызывает `voice_stream_watch` / `onWatchStream(userId)`.
   - Пауза стрима: `#stream-pause-btn` переключает состояние, вызывает `voice_stream_set_paused` / `onPauseStream(boolean)`, обновляет `aria-pressed` и текст («Приостановить стрим» / «Возобновить стрим»).
   - Остановка стрима: `#stream-stop-btn` вызывает `voice_stream_delete` / `onStopStream()`.

4. **Возрастное ограничение (`AGE_GATED_SPACES`)**:
   - Элемент `#voice-age-gated-overlay` активируется при `snapshot.ageGated === true` и отсутствии подтверждения, блокируя сетку (`data-blocked="true"`).
   - Кнопка `#voice-age-gated-confirm-btn` подтверждает возраст: скрывает оверлей, проставляет `data-confirmed="true"`, сбрасывает блокировку сетки (`data-blocked="false"`) и вызывает `onConfirmAge`.

5. **Интеграция**:
   - `createVoicePanel` экспортирует связки `stageGrid`, `stageFocused`, `goLiveModal`, `ageGatedOverlay`, а также методы управления модалкой и стримом.

---

## 3. Мутационное тестирование (5/5 убито)

Проведено 5 независимых мутаций в `rust/gui-spike/ui/features/voice.js`:

| Мутант | Описание мутации | Затронутый тест | Результат |
|---|---|---|---|
| **M1** | Инверсия/игнорирование признака речи: `const isSpeaking = false;` (вместо `speakingFlag(person.speaking)`) | `активная говорящая рамка (.is-speaking) выставляется при speaking=true и снимается при false` | **KILLED** (`AssertionError: Alice без .is-speaking`) |
| **M2** | Отключение флага стрима: `const hasStream = false;` (вместо `Boolean(person.isStream \|\| person.stream)`) | `плитка стрима Go Live имеет бейдж 'В ЭФИРЕ'` и `просмотр чужого стрима: клик вызывает voice_stream_watch` | **KILLED** (`AssertionError: нет data-is-stream`) |
| **M3** | Отключение активации фокусного режима: `if (false && focusedUserId)` в `setFocused` | `переключение фокуса (#voice-stage-focused) активирует фокусный режим выбранной плитки и переключает его` | **KILLED** (`AssertionError: #voice-stage-focused не открылся после клика`) |
| **M4** | Отключение логики клика кнопки паузы `#stream-pause-btn` | `пауза стрима: контрол паузы вызывает voice_stream_set_paused / onPauseStream и меняет состояние` | **KILLED** (`AssertionError: voice_stream_set_paused не был вызван`) |
| **M5** | Отключение подтверждения в кнопке оверлея `#voice-age-gated-confirm-btn` | `оверлей #voice-age-gated-overlay блокирует предпросмотр до подтверждения` | **KILLED** (`AssertionError: overlay не скрылся после подтверждения`) |

**Итог мутационного тестирования**: **5/5 мутантов убиты (100% kill rate)**.
После завершения мутационного тестирования исходный файл `rust/gui-spike/ui/features/voice.js` восстановлен с точной сверкой MD5: `652f974268db0f7ddee22304fd2a692c`.

---

## 4. Вердикт

- **Дефекты**: отсутствуют (P0: 0, P1: 0, P2: 0, P3: 0).
- **Статус**: **PASS / Ready for Merge**.
