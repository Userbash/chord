# Отчёт воркера: task-154 (`FE-P1-02` Go Live & Stream Tiles UI)

- **Batch / Волна**: `RW-40` (Отдел G2 Frontend, WebView UI & DOM/A11y)
- **Карточка**: `task-154` / `FE-P1-02` (`waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Спецификация и оракулы**:
  - `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` (§2.1–§2.4, §3.1–§3.3, §4, §5)
  - `waves/RW-40/DESIGN/voice-spec.json`
  - `rust/gui-spike/ui/tokens.css`
- **Ворктри**: `/home/sanya/orca/workspaces/project/feature-task-154-voice-go-live-ui`

---

## 1. Что сделано

1. **Strict TDD / Red-тесты (`tests/unit/voice_ui.test.mjs`)**:
   - Добавлен тестовый suite `FE-P1-02 / task-154: Go Live & Stream Tiles UI в voice.js` из 12 профильных тестов (в дополнение к 44 существующим тестам V-08, итого 56 тестов в `voice_ui.test.mjs`, а совместно с `gui-spike-dom.test.mjs` — 85 тестов):
     - рендер сетки видеоплиток `#voice-stage-grid` с элементами `.voice-tile` участников;
     - активная говорящая рамка `.is-speaking` (и `data-speaking="true"`) при `speaking: true` и её снятие при прекращении речи;
     - плитка стрима Go Live с бейджем 'В ЭФИРЕ' (`#stream-live-badge` / `.badge-live`, `data-is-stream="true"`);
     - переключение фокуса (`#voice-stage-focused`) с фиксацией выбранной плитки (`data-focused-user-id`, `.is-focused`), переключением на другую плитку и снятием фокуса по повторному клику;
     - модалка запуска стрима Go Live (`#go-live-modal`, `role="dialog"`, `aria-modal="true"`) с вкладками 'Приложения' / 'Экраны' и переключением `aria-selected`;
     - выбор разрешения (`720p` / `1080p` / `1440p`) и частоты кадров (`30` / `60`) с `aria-checked`;
     - кнопка «Прямой эфир» (`#go-live-submit-btn`), отправляющая `voice_stream_create` / `onStartStream` с выбранными параметрами и закрывающая модалку;
     - элементы управления активным стримом: просмотр чужого стрима (`.stream-watch-btn` -> `voice_stream_watch` / `onWatchStream`);
     - пауза стрима (`#stream-pause-btn` -> `voice_stream_set_paused` / `onPauseStream`) с переключением `aria-pressed` и текста «Приостановить стрим» / «Возобновить стрим»;
     - остановка стрима (`#stream-stop-btn` -> `voice_stream_delete` / `onStopStream`);
     - оверлей возрастного ограничения `AGE_GATED_SPACES` (`#voice-age-gated-overlay`) с блокировкой сетки/предпросмотра (`data-blocked="true"`) до нажатия кнопки подтверждения (`#voice-age-gated-confirm-btn`);
     - сквозная интеграция сцены и модалки в `createVoicePanel`.
   - Зафиксирован red-лог падения: `/home/sanya/orca/project/waves/RW-40/G2/task-154-voice-go-live-ui/red.log`.

2. **Реализация в `rust/gui-spike/ui/features/voice.js`**:
   - Расширены модели `VoiceParticipant`, `VoiceSnapshot` и `VoiceService` поддержкой стримов (`isStream`, `stream`), `ageGated`, `focusedTileId`, а также сигналов управления стримом (`voice_stream_create`, `voice_stream_delete`, `voice_stream_watch`, `voice_stream_set_paused`, `onStartStream`, `onWatchStream`, `onPauseStream`, `onStopStream`, `onConfirmAge`).
   - Реализована и экспортирована функция `createVoiceStageGrid(options)`:
     - тулбар `#stream-stage-bar` с контролами паузы (`#stream-pause-btn`) и остановки (`#stream-stop-btn`);
     - фокусная зона `#voice-stage-focused` с переключением фокуса и кнопкой сворачивания;
     - сетка `#voice-stage-grid` с карточками `.voice-tile`, динамическими классами `.is-speaking`, `.is-stream`, `.is-focused`, бейджем `#stream-live-badge` (`.badge-live`) и кнопкой просмотра `.stream-watch-btn`;
     - оверлей `#voice-age-gated-overlay` с блокировкой предпросмотра (`data-blocked="true"`) и снятием после подтверждения возраста (`#voice-age-gated-confirm-btn`).
   - Реализована и экспортирована функция `createGoLiveModal(options)`:
     - диалог `#go-live-modal` (`role="dialog"`, `aria-modal="true"`);
     - вкладки переключения источников 'Приложения' / 'Экраны' (`#go-live-tab-apps`, `#go-live-tab-screens`);
     - селекторы качества (разрешения `720p`/`1080p`/`1440p` и FPS `30`/`60`);
     - кнопка «Прямой эфир» (`#go-live-submit-btn`), вызывающая `voice_stream_create` / `onStartStream`.
   - Экспортирован фасад `createVoiceStreamController(options)`.
   - В `createVoicePanel` добавлена поддержка опций `stageHost`, `modalHost` и методов `stageGrid`, `stageFocused`, `goLiveModal`, `ageGatedOverlay`, `openGoLiveModal`, `closeGoLiveModal`, `watchStream`, `setStreamPaused`, `stopStream`, `confirmAgeGated`.
   - Полное соблюдение правил: отсутствие сырых цветов, медиа-API браузера (`mediaDevices`, `getUserMedia` и т.д.), отсутствие `console.log`.

---

## 2. Red-прогон (до реализации)

- **Команда**: `node --test tests/unit/voice_ui.test.mjs`
- **Код выхода**: `1`
- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-154-voice-go-live-ui/red.log`

```text
file:///home/sanya/orca/workspaces/project/feature-task-154-voice-go-live-ui/tests/unit/voice-ui.test.mjs:32
  createGoLiveModal,
  ^^^^^^^^^^^^^^^^^
SyntaxError: The requested module '../../rust/gui-spike/ui/features/voice.js' does not provide an export named 'createGoLiveModal'
✖ tests/unit/voice_ui.test.mjs (25.549949ms)
ℹ tests 1 | pass 0 | fail 1
```

---

## 3. Green-прогон (после реализации)

- **Команда**: `node --test tests/unit/voice_ui.test.mjs tests/unit/gui-spike-dom.test.mjs`
- **Код выхода**: `0`
- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-154-voice-go-live-ui/green.log`

```text
ℹ tests 85
ℹ suites 9
ℹ pass 85
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 266.489453
```

---

## 4. Targeted & TypeScript проверки

- `npx tsc -p rust/gui-spike/ui/tsconfig.json` -> **exit 0** (0 ошибок)
- `npx tsc -p tsconfig.json` -> **exit 0** (0 ошибок)
- В основном дереве `/home/sanya/orca/project`:
  - `node --test tests/unit/voice_ui.test.mjs tests/unit/gui-spike-dom.test.mjs` -> **94/94 pass** (включая P-03-UI typing)
  - `npx tsc -p tsconfig.json` -> **exit 0**

---

## 5. Файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/voice.js` | modified | `652f974268db0f7ddee22304fd2a692c` |
| `tests/unit/voice-ui.test.mjs` | modified | `dd4a96a1d3c56644ea168e09f549fee7` |
| `tests/unit/voice_ui.test.mjs` | symlink/copy | `dd4a96a1d3c56644ea168e09f549fee7` |

---

## 6. Риски и примечания

- Реализация строго следует MVP-контракту `F-01` (Вариант B) без сырых медиа-кадров через IPC в WebView.
- Сохранены все инварианты `V-08`: 12 состояний голосовой связи, клавиатурный Tab-порядок кнопок панели и отсутствие внешних сетевых вызовов из WebView.
