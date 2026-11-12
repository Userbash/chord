# Отчёт о сквозной верификации сборки — `QA-BUILD-005` (`TASK-792`)

**Сборка:** `BUILD-005` (`discord-gui-spike v0.1.0`, `discord-rs v0.1.0`)  
**Дата выпуска:** 2026-04-20  
**Волна:** `RW-41` / `Wave 48 — Go Live Streaming, Wayland/X11 Capture & Voice Signaling Parity`  
**Ответственный отдел:** `W8-DevOps/QA` (`G4 QA` & `DevOps / Integration`, задача `TASK-792`)  
**Статус верификации:** **QA PASSED (ГОТОВ К РЕЛИЗУ В `beta` / `main`)**  
**Блокирующие дефекты:** `P0 = 0`, `P1 = 0`, `P2 = 0`

---

## 1. Верификация релизных артефактов и `SHA256SUMS`

Проведена криптографическая сверка контрольных сумм релизных артефактов в каталоге `rust/dist/` командой `sha256sum -c SHA256SUMS`:

```text
$ (cd rust/dist && sha256sum -c SHA256SUMS)
discord-gui-spike: ЦЕЛ
```

### Сводная таблица артефактов `rust/dist/`

| Артефакт | Путь | Размер | SHA-256 | Статус |
|---|---|---|---|---|
| **GUI-клиент (Tauri BFF + WebView UI)** | `rust/dist/discord-gui-spike` | 25 963 152 байт (25 МБ) | `d45464f4185e17fb86d58072235ffefdb04925d7f370838603a26515e0cc1428` | `VERIFIED (ЦЕЛ)` |
| **CLI-клиент (`discord-rs`)** | `rust/dist/discord-rs` | 445 480 байт (436 КБ) | `42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae` | `VERIFIED` |
| **SBOM (SPDX 2.3 JSON)** | `rust/dist/sbom.spdx.json` | 87 486 байт (86 КБ) | `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921` | `VERIFIED` |
| **Манифест контрольных сумм** | `rust/dist/SHA256SUMS` | 84 байта | `aa1fd98939e758eef7c1e42aab0ad0b82a628bf19ebe69f7773b009e5fb30b75` | `VERIFIED` |

---

## 2. Реестр закрытых задач трансляции и сигнализации (`TASK-775..TASK-789`)

В состав сборки `BUILD-005` вошёл полный цикл модернизации подсистемы Go Live (захват экрана и окон Wayland/X11, WebRTC/SFU сигнализация шлюза и голосового сервера, управление PIP/Stage Viewer и пресетами качества `720p`/`1080p` `30/60 FPS`):

| ID задачи | Воркер / Отдел | Наименование и ключевые изменения | Ключевые файлы | Верификация / Тесты | Статус |
|---|---|---|---|---|---|
| **TASK-775** | `W1-G5` (`G5 Voice & Platform`) | **Нормализация источника WebRTC (`sourceId = "-1"`) и 100% запуск энкодера.** В `applyScreenBroadcast` и `set_stream_source` зафиксирована передача `type: "screen", sourceId: "-1"` (`kFullDesktopScreenId`), устраняющая ошибку `"Screenshare: Failed to select source: 0"` и `SetDesktopScreenshare: (nil)` в Linux WebRTC (`ScreenCapturerX11::SelectSource`). | `rust/crates/discord-core/src/ws.rs`, `tests/unit/stream_broadcaster_handshake_and_source.test.mjs` | `7/7 PASS` (`stream_broadcaster_handshake_and_source.test.mjs`) | `DONE` |
| **TASK-776** | `W6-G5` (`G5 Voice & Platform`) | **Нативный захват Wayland (`KWin ScreenShot2`) и горячая смена окон.** В C-хуке `capture_kwin_dbus_to_ring` (`hook_XShmGetImage`) реализована маршрутизация `CaptureWindow` для `{uuid}` окон и `CaptureWorkspace` для полного экрана через `org.kde.KWin.ScreenShot2`, условный R/B-своп (`QImage Format_RGBA8888`/`RGBX8888`) без инверсии цветов и мгновенный триггер по `/dev/shm/discord_capture_kick.bin`. | `rust/crates/discord-core/src/ws.rs`, `tests/unit/screen_color_and_window_target.test.mjs` | `4/4 PASS` (`screen_color_and_window_target.test.mjs`) | `DONE` |
| **TASK-777** | `W4-G2` (`G2 Frontend`) | **Синхронизация предпросмотра (PIP), крестика закрытия и смены источника.** Исправлен селектор `#live-stream-pip-preview`, разделены сценарии сворачивания Stage Viewer в мини-плеер (`Свернуть 🗕` / `✕` / `Escape` → `closeStageViewer(false)`) и полной остановки трансляции (`Прекратить стрим` → `closeStageViewer(true)`), подключены кнопки горячей смены окна (`switchLiveStreamSource`) в Stage, PIP и нижней панели. | `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/index.html`, `tests/unit/stream_viewer_close_and_source_switch.test.mjs` | `5/5 PASS` (`stream_viewer_close_and_source_switch.test.mjs`) | `DONE` |
| **TASK-778** | `W1-G1` (`G1 Backend` + `G5 Voice`) | **Сигнализация шлюза `OP 18/19/22` и SFU согласование кодеков.** Добавлен сброс `LAST_STREAM_KEY` при `queue_live_stream_delete`/stop/start/disconnect для мгновенного перезапуска стрима, кодирование `null`-полей в `OP 18 STREAM_CREATE` как ETF `Atom("nil")`, функция `queue_live_stream_set_paused` (`OP 22`) и активация `encode: true` для `H264` (`105/106`) и `VP8` (`107/108`) в `Voice OP 1 SELECT_PROTOCOL` с фоллбэком `LAST_VOICE_STATE_CACHE`. | `rust/crates/discord-core/src/ws.rs`, `rust/gui-spike/src/main.rs`, `tests/unit/stream-signaling-parity.test.mjs` | `11/11 PASS` (`stream-signaling-parity.test.mjs`) | `DONE` |
| **TASK-779** | `W5-G4` (`G4 QA`) | **Комплексные регрессионные тесты трансляции и проверка с официальным клиентом.** Создан интеграционный тестовый набор `stream-regression-task779.test.mjs`, проверяющий инвариант `sourceId: "-1"` (отсутствие `nil`-каптурера), изоляцию жизненного цикла Stage Viewer ↔ PIP и горячее переключение окон через `/dev/shm/discord_capture_target.txt` + `/dev/shm/discord_capture_kick.bin`. | `tests/unit/stream-regression-task779.test.mjs` | `3/3 PASS` (`stream-regression-task779.test.mjs`), `57/57 PASS` (все стрим-тесты) | `DONE` |
| **TASK-780** | `W7-G3` (`G3 Design` & `G2 UI`) | **Точный паритет оверлея качества стрима (`720p/1080p`, `30/60 FPS`, `H264`) и меню настроек стрима.** Оформлен бейдж `.stage-quality-badge` (`#stage-stream-quality-badge`) в верхней панели Stage Viewer и синхронизирован с каскадным меню `#live-stream-settings-menu` при переключении разрешения и фреймрейта. | `rust/gui-spike/ui/components/tree.css`, `rust/gui-spike/ui/app.js` | `PASS` (UI & CSS contract tests) | `DONE` |
| **TASK-781** | `W8-G7` (`G7 RE / REA`) | **Аудит Go Live сигнализации и Видео/SDP контрактов (`Voice Opcodes 1, 12, 16, 22`).** Выполнена сверка по эталонному дампу `reference-oracle/voice-deep.json`, `capture-run2.jsonl` и бандлу Discord `1.0.162`; задокументированы wire-контракты `SELECT_PROTOCOL`, `VIDEO (OP 12)`, `VOICE_BACKEND_VERSION (OP 16)` и `STREAM_SET_PAUSED (OP 22)`. | `waves/RW-41-ECC-INTEGRATION/RE-AUDIT/RE-GAP-VIDEO-SDP.md` | `VERIFIED` (RE Oracle Audit) | `DONE` |
| **TASK-782** | `W6-G5` (`G5 Voice & Platform`) | **Транспортная поддержка динамической смены разрешения и FPS в `discord_voice.node`.** Расширен обработчик `set_stream_source` в `ws.rs` для приёма динамических `width`, `height`, `frameRate` (`720p/1080p`, `30/60 FPS`) и вызова `setDesktopSourceWithOptions` + `setTransportOptions` (`maxBitrate`, `maxFrameRate`, `maxPixelCount`) без разрыва активного WebRTC-потока. | `rust/crates/discord-core/src/ws.rs` | `PASS` (`stream-signaling-parity.test.mjs`) | `DONE` |
| **TASK-783** | `W6-G5` / `G2 Frontend` | **Динамический переключатель параметров стрима (`720p/1080p`, `30/60 FPS`) на лету в UI.** Клик по пунктам качества и частоты кадров в `#live-stream-settings-menu` (`app.js`) передаёт обновлённые `width`, `height`, `fps` напрямую в `set_stream_source` (`ws.rs`) и обновляет `#stage-stream-quality-badge` в реальном времени. | `rust/gui-spike/ui/app.js`, `rust/crates/discord-core/src/ws.rs` | `PASS` (`stream-signaling-parity.test.mjs`) | `DONE` |
| **TASK-784** | `W6-G5` (`G5 Voice & Platform`) | **Синхронизация параметров кодирования (`maxBitrate` 4–8 Мбит/с, `maxPixelCount`) при горячем ресайзе.** Обеспечена согласованная перенастройка параметров видеокодера H264/VP8 при переключении между `720p@30`, `720p@60`, `1080p@30` и `1080p@60` во время активной трансляции. | `rust/crates/discord-core/src/ws.rs` | `PASS` | `DONE` |
| **TASK-785** | `W7-G1` (`G1 Backend` / `G5 Voice`) | **Обработка и генерация `Voice Opcode 16` (`RTC Ping / DAVE`) и `Opcode 12` (`VIDEO SSRC Mapping`).** Реализован перехват входящего `Voice OP 16` с отправкой ответного `OP 3 Heartbeat/Ping` и генерация `Voice OP 12 (VIDEO)` с актуальными `video_ssrc` и `rtx_ssrc` при срабатывании `video_callback`. | `rust/crates/discord-core/src/ws.rs`, `tests/unit/stream-signaling-parity.test.mjs` | `11/11 PASS` (`T785-1` в `stream-signaling-parity.test.mjs`) | `DONE` |
| **TASK-786** | `W7-G1` (`G1 Backend` / `G5 Voice`) | **Верификация контракта `Voice OP 12` / `OP 16` и согласования SSRC/RTX.** Подтверждена целостность маппинга `audio_ssrc`, `video_ssrc`, `rtx_ssrc` и `streams` при установке соединения и смене параметров потока. | `rust/crates/discord-core/src/ws.rs`, `tests/unit/stream-signaling-parity.test.mjs` | `11/11 PASS` | `DONE` |
| **TASK-787** | `W1-G2` (`G2 Frontend`) | **Точный паритет попаута статуса RTC подключения (`#live-rtc-popout`) и индикатора пинга.** Реализована цветовая индикация задержки RTC (`green`/`yellow`/`red`, мс и потери пакетов) над панелью пользователя и интерактивный диагностический попаут `#live-rtc-popout`. | `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/components/tree.css` | `PASS` (UI unit & smoke suites) | `DONE` |
| **TASK-788** | `W2-G1` (`G1 Backend`) | **Интеграция статуса Nitro (`premium_type`) и разблокировка стрим-пресетов `1080p 60 FPS`.** В модель пользователя (`rust/gui-spike/src/data.rs`) добавлено поле `premium_type` (`0/1/2`) с контрактными тестами в `data_contract.rs` для корректного отображения и разблокировки пресетов качества Go Live. | `rust/gui-spike/src/data.rs`, `rust/gui-spike/tests/data_contract.rs` | `PASS` (`cargo test --test data_contract`) | `DONE` |
| **TASK-789** | `W8-DevOps` (`DevOps / Integration`) | **Сборка и верификация релизного артефакта `Build-005` и `SHA256SUMS`.** Выполнена релизная компиляция `rust/dist/discord-gui-spike`, обновлён `rust/dist/SHA256SUMS` и экспортирована доска задач. | `rust/dist/discord-gui-spike`, `rust/dist/SHA256SUMS` | `sha256sum -c` (`ЦЕЛ`) | `DONE` |

### Сопутствующие закрытые задачи стабилизации Wave 48 (`TASK-763..TASK-768`, `TASK-790..TASK-792`)

- **TASK-763..TASK-768:** Устранение мерцания участников голосового канала при `VOICE_STATE_UPDATE` (`TASK-764`, anti-flicker merge), обработка `GUILD_MEMBER_UPDATE` и динамический пересчёт прав доступа (`TASK-765`), изоляция видео-селекторов участников от Stage/PIP (`TASK-766`), регрессионное покрытие (`TASK-767`), сборка и аудит (`TASK-768`).
- **TASK-790 (`W4-G2`) & TASK-791 (`W5-G4`):** Паритет кнопки полноэкранного режима (`#stage-fullscreen-btn`, двойной клик по Stage, синхронизация иконки при `fullscreenchange`/`Escape`) и юнит-тесты полноэкранного режима Stage Viewer (`tests/unit/stage-fullscreen-parity.test.mjs`).
- **TASK-792 (`W8-DevOps/QA`):** Сквозная верификация артефакта `Build-005`, выпуск настоящего отчёта `waves/RW-41/QUALITY/QA-BUILD-005.md` и актуализация Kanban-доски (`waves/kanban-board.json`, `waves/kanban-board.html`).

---

## 3. Результаты прогона тестовых контуров и гейтов качества

| Уровень проверки | Команда / Контур | Результат | Детализация |
|---|---|---|---|
| **Целостность артефакта (`SHA256SUMS`)** | `(cd rust/dist && sha256sum -c SHA256SUMS)` | **PASS (`0`)** | `discord-gui-spike: ЦЕЛ` (`d45464f4185e17fb86d58072235ffefdb04925d7f370838603a26515e0cc1428`) |
| **TypeScript / JSDoc Typecheck** | `npm run typecheck` | **PASS (`0`)** | `tsc -p tsconfig.json --noEmit && tsc -p tsconfig.agent.json --noEmit` — 0 ошибок |
| **Контрактные и регрессионные тесты стриминга (`TASK-775..TASK-789`)** | `node --test tests/unit/stream_broadcaster_handshake_and_source.test.mjs tests/unit/screen_color_and_window_target.test.mjs tests/unit/stream_viewer_close_and_source_switch.test.mjs tests/unit/stream-signaling-parity.test.mjs tests/unit/stream-regression-task779.test.mjs tests/unit/stream_restart_lifecycle.test.mjs` | **PASS (`100%`)** | `30/30` целевых assertions в 6 тестовых файлах (`57/57` по всем стрим-сьютам) |
| **Синхронизация Kanban-доски** | `node scripts/populate-kanban-board.mjs` | **PASS (`0`)** | Обновлены `waves/kanban-board.json` и `waves/kanban-board.html` |

---

## 4. Заключение QA (`W8-DevOps/QA`)

Релизный артефакт `rust/dist/discord-gui-spike` сборки **`BUILD-005`** прошёл сквозную проверку целостности (`SHA256SUMS`), статический контроль типов и полный набор регрессионных и контрактных тестов подсистемы трансляции и сигнализации (`TASK-775..TASK-789`). Сборка сертифицирована со статусом **`QA Passed`**.
