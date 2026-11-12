# REPORT — TASK-616 (G2 Frontend): UI просмотра чужого стрима и режим PiP (`rust/gui-spike/ui/features/stream-viewer.js` + `rust/gui-spike/ui/styles/stream-viewer.css`)

- **batch/карточка:** `RW-40 / TASK-616` (G2 Frontend — UI просмотра чужого стрима и режим PiP)
- **Ветка:** `feature/task-616-stream-viewer-ui`
- **Рабочее дерево:** `/home/user/orca/workspaces/project/task-616-stream-viewer-ui`
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (согласно правилам `AGENTS.md` §6).

---

## 1. Сделано (`PROCESS.md` §4.2)

- **batch/карточка:** `RW-40 / TASK-616`;
- **сделано:**
  1. Реализован модуль `rust/gui-spike/ui/features/stream-viewer.js` (с ре-экспортом в `stream-viewer-ui.js` для полной совместимости) в строгом соответствии с ТЗ координатора и `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` (§2.2.3, §2.3.1, §4, §5):
     - Экспортированы константы `STREAM_VIEWER_MODES` (`'stage'`, `'pip'`, `'fullscreen'`, `'hidden'`), `STREAM_VIEWER_ZOOM_LIMITS` (min `1.0`, max `5.0`, default `1.0`, double click `2.25`, step `1.25`) и `DOCUMENT_PIP_TRIGGER_ID` (`'__document_pip_trigger'`).
     - Реализованы чистые утилиты `clampStreamZoomScale(val)` и `formatStreamVolumeLabel(vol, muted)`.
     - Реализован DOM-контроллер `createStreamViewerController(containerEl, options)`:
       - **Контейнер видеопотока с кнопками управления**:
         - Кнопка Play/Pause (`#stream-viewer-play-pause`, `[data-stream-action="play-pause"]`, `aria-pressed`, `"⏸ Пауза"` / `"▶ Возобновить"`), вызывающая `voice_stream_set_paused` и колбэк `onPauseToggle`.
         - Кнопка Mute/Unmute (`#stream-viewer-mute-toggle`, `[data-stream-action="mute"]`, иконки `🔊`/`🔇`, `aria-pressed`), ползунок громкости `#stream-viewer-volume-slider` (`0%..200%`, шаг 1) и числовая индикация `#stream-viewer-volume-label` (`"100%"`, `"ВЫКЛ"`), синхронизация через `voice_set_audio_device` (`stream_audio_volume`).
         - Кнопка Fullscreen (`#stream-fullscreen-toggle`, `[data-stream-action="fullscreen"]`, `"⛶ На весь экран"`) и модальный полноэкранный контейнер `#stream-fullscreen[role="dialog"][aria-modal="true"]` с заголовком и кнопкой закрытия.
         - Кнопка Toggle PiP (`#stream-viewer-pip-toggle`, `[data-stream-action="pip"]`, `"⧉ Мини-плеер"`).
       - **Режим Picture-in-Picture (PiP) в углу экрана**:
         - Плавающий мини-плеер `#stream-viewer-pip[data-pip-active="true"][role="region"]` в углу экрана с шапкой (имя стримера `.stream-pip-title`, бейдж `.stream-pip-live-badge`, пилюля канала `.stream-pip-channel`), кнопкой разворачивания обратно в сцену `#stream-viewer-pip-expand`, вьюпортом `.stream-pip-viewport`, Mute-кнопкой `#stream-viewer-pip-mute` и кнопкой закрытия `#stream-viewer-pip-close` (выход из просмотра).
         - Автоматический переход в PiP при переключении каналов в интерфейсе (`onChannelSwitch(channelId)`).
         - Интеграция с Document PiP триггером `DOCUMENT_PIP_TRIGGER_ID` (`__document_pip_trigger`) из RE-дампа (`docs/re/backend-ipc.json`, `DISCORD_WINDOW_OPEN_DOCUMENT_PIP`).
       - **Бейджи качества/FPS и DAVE MLS шифрования**:
         - Бейдж качества и FPS (`#stream-viewer-quality-badge`, `.stream-quality-badge`, например `"720p • 60 FPS"`).
         - Статус сквозного шифрования DAVE MLS (`#stream-viewer-encryption-badge`, `.stream-encryption-badge`, `data-encrypted="true"`, `"🔒 End-to-End Encrypted"`).
       - **Элементы масштабирования (Zoom & Pan)**: кнопки `#stream-viewer-zoom-out` (`−`), `#stream-viewer-zoom-reset` (`100%`), `#stream-viewer-zoom-in` (`+`), `#stream-viewer-zoom-200` (`200%`), поддержка колеса мыши и двойного клика (100% ↔ 225%).
       - **Клавиатурные хоткеи и WAI-ARIA**: поддержка клавиш `P` (PiP), `F` (Fullscreen), `M` (Mute), Пробел (Play/Pause), `+`/`-`/`0` (Zoom), `Escape` (выход из fullscreen / оверлея).
  2. Создан файл стилей `rust/gui-spike/ui/styles/stream-viewer.css`:
     - 100% token-binding через `rust/gui-spike/ui/tokens.css` (0 сырых цветов `#hex`/`rgb`/`hsl`, 0 `!important`, 0 `:has()`, 0 fallback в `var(--*)`).
     - Оформление кнопок `.stream-btn`, оверлея `.stream-viewer-overlay`, холста `.stream-viewer-canvas`, плавающего окна `.stream-viewer-pip`, бейджей `.stream-live-badge`, `.stream-quality-badge`, `.stream-encryption-badge` и полноэкранного режима `.stream-fullscreen`.
  3. Экспорты подключены в `rust/gui-spike/ui/features/voice-stage-ui.js`.
  4. Написаны модульные тесты `tests/unit/stream-viewer.test.mjs` и `tests/unit/stream-viewer-ui.test.mjs` (20 тестов в 9 сьютах) и зарегистрированы в `TDD-PLAN.md` §8.
- **red:** `node --test tests/unit/stream-viewer-ui.test.mjs` → `1 fail` (`Cannot find module .../stream-viewer-ui.js`), `exit code = 1` (лог: `waves/RW-40/G2/task-616-stream-viewer-ui/red.log`);
- **green:** `node --test tests/unit/stream-viewer.test.mjs tests/unit/stream-viewer-ui.test.mjs` → `20/20 pass, 0 fail, 0 skipped`, `exit code = 0` (лог: `waves/RW-40/G2/task-616-stream-viewer-ui/green.log`);
- **targeted:**
  - `node --test tests/unit/stream-viewer.test.mjs tests/unit/stream-viewer-ui.test.mjs tests/unit/voice-stage-ui.test.mjs tests/unit/voice-ui.test.mjs tests/meta/plan-sync.test.mjs` → `91/91 pass, 0 fail, 0 skipped` (`exit code = 0`);
  - `node waves/RW-39/PM/w39c/geometry-check.mjs` → `все 31 геометрических значений в допуске` (`exit code = 0`);
  - `npm run typecheck` → `0 errors` (`exit code = 0`);
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `0 errors` (`exit code = 0`);
- **oracle:**
  - `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` §2.2.3, §2.3.1, §3.1, §4, §5, §6;
  - `reference-oracle/voice-deep.md`;
  - `reference-oracle/frontend-d03.md` §1;
  - `docs/re/backend-ipc.json` (`DISCORD_WINDOW_OPEN_DOCUMENT_PIP`);
  - `rust/gui-spike/ui/tokens.css` (100% биекция токенов);
- **генерация:** не применимо (статический модуль UI и таблица стилей);
- **мутации:** `5/5 KILLED` (`0 survivors`, `0 INCONCLUSIVE`, лог: `waves/RW-40/G2/task-616-stream-viewer-ui/mutations.log`):
  - `M1_pip_mode_switch` (`else if (false && currentMode === 'pip')`): **KILLED** (`exit=1`) — тест `переключение в режим PiP скрывает overlay и монтирует плавающий мини-плеер #stream-viewer-pip`;
  - `M2_document_pip_trigger` (отключение обработчика `__document_pip_trigger`): **KILLED** (`exit=1`) — тест `DOCUMENT_PIP_TRIGGER_ID (__document_pip_trigger) инициирует переключение PiP через OS IPC`;
  - `M3_leave_watch_callback` (отключение вызова `options.onLeaveWatch`): **KILLED** (`exit=1`) — тест `клик по кнопке "Выйти из просмотра" вызывает onLeaveWatch и закрывает просмотр`;
  - `M4_volume_mute_sync` (отключение изменения `isAudioMuted` в `toggleMute`): **KILLED** (`exit=1`) — тест `синхронизирует громкость и mute между stage viewer и PiP плеером`;
  - `M5_zoom_clamp` (`return num` вместо ограничения 1.0..5.0): **KILLED** (`exit=1`) — тест `clampStreamZoomScale надёжно ограничивает масштаб диапазоном 1.0..5.0`;
- **baseline/restore:**
  - До и после всех 5 мутаций проверена контрольная сумма:
    - `rust/gui-spike/ui/features/stream-viewer-ui.js`: `a07ae54d987458a1a90d4a1c7fc956e4` (совпадает);
- **стабильность:** детерминированные синхронные DOM unit-тесты без таймеров и внешних сетевых вызовов;
- **gate:worker:** в соответствии с `PROCESS.md` §3.14 / §5 воркер сдаёт targeted red/green и typecheck; полный batch-gate запускается на integration-лейне;
- **файлы (`md5sum`):**
  ```text
  56c74b733595bc8f76fc2e2cc7fd7c1b  rust/gui-spike/ui/features/stream-viewer.js
  a07ae54d987458a1a90d4a1c7fc956e4  rust/gui-spike/ui/features/stream-viewer-ui.js
  f5c972b4254127953cef31bbf6e9cabd  rust/gui-spike/ui/styles/stream-viewer.css
  43f2cc9a235ad7efddb297f4f60f8c9e  rust/gui-spike/ui/features/voice-stage-ui.js
  4855afcdf8cbc9762239dbb0fec1666f  tests/unit/stream-viewer.test.mjs
  febaad98f9b684b1720dad7ed7c5fe12  tests/unit/stream-viewer-ui.test.mjs
  d19a0ed0c9ed0f45ea1990b41d339307  TDD-PLAN.md
  ```
- **риски/вопросы:** отсутствуют; все существующие тесты `voice-stage-ui` и `voice-ui` полностью сохранены и проходят без регрессий;
- **следующий шаг:** передать задачу `TASK-616` на Product Review (PM) и Technical Review (DeepSeek).
