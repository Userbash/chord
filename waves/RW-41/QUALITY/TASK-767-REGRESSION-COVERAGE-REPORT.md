# Сводный отчёт о сквозной верификации регрессионного сьюта и покрытии — `TASK-767`

- **Задача:** `TASK-767` — Сквозная верификация регрессионного сьюта (`npm run test:unit`, `npm run test:meta`, `npm run typecheck`, `cargo test -p discord-core`) и сводный отчёт о покрытии
- **Отдел:** `G4 QA (Test Automation & Verification)`
- **Ветка / HEAD:** `feature/sprints-1-5-build-004` (`6f60b11b5b8b5604e42af3f0be53022ec45fbcdd`)
- **Дата и время прогона:** `2026-11-10 17:55 +05:00`
- **Вердикт:** **PASS (100% Green, 0 Failed, 0 Skipped)** — все целевые сьюты и пороги покрытия `TEST-POLICY.md` выполнены.

---

## 1. Сводная таблица выполнения регрессионного сьюта

| Контур проверки | Команда запуска | Всего тестов / таргетов | Пройдено (`Pass`) | Падений (`Fail`) | Пропусков (`Skip`) | Время | Статус |
|---|---|---:|---:|---:|---:|---:|:---:|
| **TypeScript Typecheck** | `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) | 2 проекта TS | 2 | 0 | 0 | ~1.4 с | **PASS** |
| **Meta Suite** | `npm run test:meta` (`node --test 'tests/meta/**/*.test.mjs'`) | 138 тестов (23 сьюта) | 138 | 0 | 0 | 203.89 мс | **PASS** |
| **Unit Suite (JS/UI)** | `npm run test:unit` (`node --test 'tests/unit/**/*.test.mjs'`) | 1978 тестов (392 сьюта) | 1978 | 0 | 0 | 6879.40 мс | **PASS** |
| **Rust Core (`discord-core`)** | `cd rust && cargo test -p discord-core` | 686 тестов (57 таргетов) | 686 | 0 | 0 | 6.80 с | **PASS** |
| **ИТОГО** | **Сквозной регрессионный контур** | **2802 теста** | **2802 (100%)** | **0** | **0** | **~15.2 с** | **PASS** |

---

## 2. Сводный отчёт о покрытии кода (`npm run test:coverage`)

Прогон выполнен командой:
```sh
node --test --experimental-test-coverage --test-coverage-include='src/**' \
  --test-coverage-lines=90 --test-coverage-functions=80 --test-coverage-branches=90 \
  'tests/unit/**/*.test.mjs'
```

### 2.1. Соответствие порогам `TEST-POLICY.md` (§5)

| Метрика покрытия | Порог CI (`TEST-POLICY.md`) | Фактическое значение | Запас к порогу | Статус |
|---|---:|---:|---:|:---:|
| **Строки (`Lines`)** | `>= 90.00%` | **99.58%** | `+9.58 п.п.` | **PASS** |
| **Ветвления (`Branches`)** | `>= 90.00%` | **94.04%** | `+4.04 п.п.` | **PASS** |
| **Функции (`Functions`)** | `>= 80.00%` | **92.70%** | `+12.70 п.п.` | **PASS** |

### 2.2. Детализация покрытия по модулям `src/**`

| Модуль (`src/**`) | Строки (`Line %`) | Ветвления (`Branch %`) | Функции (`Funcs %`) | Непокрытые строки |
|---|---:|---:|---:|---|
| `src/adapters/frida.mjs` | 96.00% | 100.00% | 83.33% | `19-21` |
| `src/cli/run.mjs` | 99.13% | 96.23% | 64.29% | `400-401, 403-404` |
| `src/core/args.mjs` | 100.00% | 94.59% | 100.00% | — |
| `src/core/calls.mjs` | 100.00% | 96.43% | 100.00% | — |
| `src/core/cdp/client.mjs` | 100.00% | 89.41% | 100.00% | — |
| `src/core/csv.mjs` | 100.00% | 100.00% | 100.00% | — |
| `src/core/hook-io.mjs` | 99.05% | 96.30% | 94.12% | `66` |
| `src/core/inventory.mjs` | 100.00% | 100.00% | 100.00% | — |
| `src/core/paths.mjs` | 100.00% | 91.67% | 100.00% | — |
| `src/core/processes.mjs` | 100.00% | 100.00% | 100.00% | — |
| `src/core/report/domains.mjs` | 100.00% | 100.00% | 100.00% | — |
| `src/core/report/nm.mjs` | 100.00% | 84.21% | 100.00% | — |
| `src/core/ui/adapters.mjs` | 100.00% | 100.00% | 100.00% | — |
| `src/core/ui/blacklist.mjs` | 100.00% | 100.00% | 100.00% | — |
| `src/core/ui/engine.mjs` | 100.00% | 93.83% | 85.71% | — |
| `src/core/ui/profile.mjs` | 100.00% | 71.43% | 100.00% | — |
| `src/core/ui/walk-log.mjs` | 100.00% | 100.00% | 100.00% | — |
| **Все файлы (`all files`)** | **99.58%** | **94.04%** | **92.70%** | — |

---

## 3. Верификация регрессионных тестов последних инцидентов (Voice / Stream / Gateway / UI)

Все новые и обновлённые регрессионные тесты в `tests/unit/` прошли со статусом `100% PASS`:

| Тестовый сьют | Проверяемый инвариант | Результат |
|---|---|---|
| `tests/unit/krisp-vad-stability.test.mjs` | Защита от краша Krisp VAD (Signal 11 SEGV) и разделение нейросетевого шумоподавления (`noiseCancellation: true`) и WebRTC VAD (`vadUseKrisp: false`) | `10/10 PASS` |
| `tests/unit/stream_viewer_close_and_source_switch.test.mjs` | Wayland `ScreenCapturerX11` (`type: "screen", sourceId: "0"`), парсинг GUID окна `{...}`, раздельное закрытие превью и остановка стрима, горячее переключение источника (`switchLiveStreamSource`) | `5/5 PASS` |
| `tests/unit/stream_broadcaster_handshake_and_source.test.mjs` | Сброс `LOCAL_SELF_STREAM` и `clear_last_stream_key()`, обход 120-мс коалесценции при `stream_or_video_changed`, нормализация `screen:0 -> 0`, `CaptureWorkspace` + проверка формата KWin | `6/6 PASS` |
| `tests/unit/screen_color_and_window_target.test.mjs` | Цветовая коррекция KWin `RGBA <-> BGRA`, маршрутизация целей захвата (`window UUID` vs `screen`), сохранение друзей и ЛС при REST-синхронизации | `3/3 PASS` |
| `tests/unit/stream_restart_lifecycle.test.mjs` | Повторный запуск демонстрации экрана на том же канале, очистка состояния после остановки | `PASS` |
| `tests/unit/call_stream_lifecycle.test.mjs` | Жизненный цикл звонка и стрима, сброс при выходе из голосового канала | `PASS` |
| `tests/unit/call_stream_dispatch.test.mjs` | Обработка событий `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_DELETE` и `VOICE_STATE_UPDATE` | `PASS` |
| `tests/unit/message-delete-voice-exit-ui.test.mjs` | Синхронизация удаления сообщений и выхода из голосового канала | `PASS` |
| `tests/unit/stream-signaling-parity.test.mjs` | Паритет сигнализации стрима (`OP 12 VIDEO`, `OP 5 SPEAKING`, `OP 15 MEDIA_SINK_WANTS`) | `PASS` |
| `tests/unit/guild-rail-incremental.test.mjs` | Инкрементальное обновление серверного рейла без деструктивного пересоздания | `PASS` |

---

## 4. Покрытие контрактных и модульных таргетов `discord-core` (`cargo test -p discord-core`)

Все **57 тестовых таргетов** (`686 тестов`) крейта `discord-core` завершились без ошибок (`0 failed`, `0 ignored`):

| Категория | Тестовые таргеты | Кол-во тестов | Статус |
|---|---|---:|:---:|
| **Unit-тесты ядра (`src/lib.rs`)** | `dispatch`, `gateway`, `gateway::supervisor`, `identify`, `rest::identity`, `rest::search`, `state::presence`, `ws` | 36 | **PASS** |
| **Шлюз, сессия и транспорт** | `dispatch_router` (12), `gateway_session` (15), `gateway_supervisor` (14), `identify` (5), `identify_ready` (16), `op37_and_voice_v8_contract` (5), `resume_reconnect` (13), `stream_gateway_contract` (11), `ws_transport` (19) | 120 | **PASS** |
| **READY и Read State** | `ready_core` (14), `ready_fixture` (3), `ready_read_state` (11), `ready_review` (7), `ready_social` (13), `read_state_reducer` (17), `read_state_selectors` (11), `read_state_types` (12) | 88 | **PASS** |
| **Состояние, каналы, права и история** | `state_contract` (15), `state_reducer` (19), `state_selectors` (8), `state_types` (15), `permissions_resolution_contract` (6), `history_pagination` (5), `history_selectors` (8), `history_types` (7) | 83 | **PASS** |
| **Участники (`Member List / Chunk`)** | `member_chunk_reducer` (13), `member_chunk_selectors` (11), `member_chunk_types` (19), `member_list_reducer` (17), `member_list_selectors` (9), `member_list_types` (14) | 83 | **PASS** |
| **Присутствие, печать и голос (`Presence / Typing / Voice`)** | `presence_reducer` (19), `presence_selectors` (18), `presence_types` (18), `typing_reducer` (5), `typing_selectors` (5), `typing_types` (5), `voice_state_reducer` (8), `voice_state_selectors` (4), `voice_state_types` (6) | 88 | **PASS** |
| **REST v9 и User Settings Proto** | `rest_ack` (9), `rest_base64` (5), `rest_client` (31), `rest_headers` (5), `rest_http1` (27), `rest_identity` (8), `rest_rate_limit` (10), `rest_resources` (6), `rest_retry` (16), `rest_routes` (18), `rest_super_properties` (8), `rest_telemetry` (4), `rest_transport` (14), `rest_user` (10), `settings_proto` (18), `settings_proto_http` (9) | 188 | **PASS** |
| **Всего по `discord-core`** | **57 таргетов** | **686** | **PASS** |
