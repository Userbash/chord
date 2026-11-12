# VOICE-STREAM-UI-SPEC — Полная дизайн-спецификация UI стриминга экрана (Go Live / Screen Share, Gateway Opcodes 18..=22, RW-40 / task-126)

- **Статус**: Дизайн-контракт (нормативная спецификация G3 Design; продуктовый код и тесты в карточке не изменялись).
- **Дата**: 2026-10-30
- **Ветка**: `design/task-126-voice-stream-ui-spec` (базовый `HEAD`: `4e240481e59ff4d49732ac8bd5ba08a006363748`)
- **Карточка**: `RW-40 / task-126 (G3-VOICE-STREAM-UI-SPEC)`
- **Владелец**: **G3 Design Specialist**
- **Потребители**:
  - **G2 Frontend**: `rust/gui-spike/ui/features/voice.js` (расширение) / `rust/gui-spike/ui/features/stream.js`, `rust/gui-spike/ui/components/voice.css` / `rust/gui-spike/ui/components/stream.css`, `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js`;
  - **G1 Core / Network**: `rust/crates/discord-core/src/stream.rs`, `rust/crates/discord-core/src/dispatch.rs`, `rust/crates/discord-core/tests/stream_gateway_contract.rs`;
  - **G5 Voice / Platform**: `rust/crates/discord-voice/src/session.rs`, `rust/crates/discord-voice/tests/session_contract.rs`;
  - **G4 QA / Review**: `tests/unit/voice-stream-ui.test.mjs`, `TEST-POLICY.md` §11–§14.

---

## 0. Нормативные источники, легенда доказательности и архитектурные инварианты

### 0.1. Входные контракты и оракулы

1. **`rust/gui-spike/ui/tokens.css`** (`md5: 21d570a6bf9c4b5d77b00d3d9b3d1d08`, 204 объявленных custom property) — единственный нормативный источник CSS custom properties (поверхности, текст, акценты, статусы, границы, состояния, типографика, отступы, радиусы, тени, слои, длительности переходов). Копия-источник `waves/RW-39/DESIGN/tokens.css` имеет `md5: 7ce9c1f03e26056243e13df3237b0a9d` (205 объявлений) и расходится с нормативной копией пятью правками: (1) шапка вендоринга в `ui/`; (2) в DESIGN отдельно объявлен `--positive-fact: #45a366`, в `ui/` он свёрнут в комментарий к `--positive`; (3) текстовые комментарии раздела 10 про D-04 и legacy-треки RW-38; (4) комментарий плотности `compact`; (5) блок `@media (max-width: 959px)` с переопределением `--layout-tree-w` (DP-01). Базовые пары `property: value` в `:root` совпадают; ни одна используемая спекой переменная не меняется. Спека использует 89 уникальных `var(--*)`, все 89 объявлены в нормативной копии; `--positive-fact` спекой не используется.
2. **`waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md`** и **`waves/RW-40/VOICE/stream-opcodes-fixtures.json`** — замороженный протокольный контракт шлюза и редьюсера Go Live / Screen Share (`STUB-27`, решения `D-27-1..D-27-7`, гипотезы `H-VIEWERS`, `H-PAUSE`, `H-SERVER-UPDATE`, `H-DELETE-END`, якоря `A-OPCODES`, `A-SENDERS`, `A-ROUTER`, `A-KEY-*`, `A-STREAMTYPE`, `A-REASONS`, `A-PHASES`, `A-REDUCER-*`, `A-STOP-HELPER`, `A-PING-CALLSITE`, `A-PAUSE-UI`).
3. **`rust/crates/discord-core/src/stream.rs`** и **`rust/crates/discord-voice/src/session.rs`** — реализованные в `discord-core` и `discord-voice` типы, кодировщики исходящих команд `op 18..=22` (`STREAM_CREATE`, `STREAM_DELETE`, `STREAM_WATCH`, `STREAM_PING`, `STREAM_SET_PAUSED`), парсеры входящих `op 0`-диспатчей (`STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, `STREAM_DELETE`), `StreamKey`, `StreamSecret`, `StreamPhase` / `StreamLifecyclePhase`, `StreamDeleteReason`, `StreamError`, `StreamSnapshot` / `SessionSnapshot`.
4. **`waves/RW-40/DESIGN/STREAM.md`**, **`waves/RW-40/DESIGN/stream-spec.json`**, **`waves/RW-40/DESIGN/VOICE.md`**, **`waves/RW-40/DESIGN/VOICE-PANEL.md`**, **`waves/RW-40/DESIGN/voice-spec.json`** — принятые базовые контракты голосовой панели и демонстрации экрана (включая закрытые замечания `F-01..F-04` из `STREAM-REVIEW-PASS2.md` / `STREAM-REVIEW-PASS3.md` и `STREAM-ACCEPT.md`).
5. **`rust/gui-spike/ui/features/voice.js`**, **`rust/gui-spike/ui/components/voice.css`**, **`rust/gui-spike/ui/index.html`**, **`tests/unit/voice-ui.test.mjs`** — действующая замороженная реализация голосовой панели `V-08`.
6. **`TEST-POLICY.md` §11–§14** — обязательная политика тестирования, 12 уровней и 9 обязательных измерений покрытия для каждого среза (§13).
7. **Воспроизводимость оракулов кода.** В ветке `design/task-126-voice-stream-ui-spec` (HEAD `4e24048`) нет `rust/crates/discord-core/src/stream.rs`, `rust/crates/discord-voice/src/session.rs`, `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md`, `stream_gateway_contract.rs` и `session_contract.rs`: они лежат в рабочем дереве `main` (`/home/user/orca/project`, незакоммичены) и попадут в design-ветку только после мержа. Все ссылки `stream.rs:NNN` / `session.rs:NNN` в спеке указывают на `main`-редакцию. `TEST-POLICY.md` §13 (9 измерений) тоже есть только в `main` (174 строки, §1..§13); в ветке — 113 строк и 10 разделов без §13. Метки `[S]` и `[F]` проверяемы только после мержа design-ветки на актуальный `main`.
8. **Сверка с RE-отчётами.** `reference-oracle/voice-deep.md` и `reference-oracle/frontend-d03.md` (2026-10-19) не содержат опкодов `18..=22`: там RTC-контроль `0..16`, UDP/strace и UI-дамп голосовой панели. Протокольная часть поэтому опирается на замороженный контракт и код `discord-core` / `discord-voice`, а не на дамп. Отчёты подтверждают только UI-факты (отсутствие камеры, панель голоса, Krisp) и §1 не противоречат; опкоды `18..=22` живым трафиком не подтверждены и помечены `[S]` / `[F]`, а не `[L]`.

### 0.2. Метки происхождения утверждений

| Метка | Значение | Источник |
|---|---|---|
| `[T]` | Существующий CSS-токен из `rust/gui-spike/ui/tokens.css` | `rust/gui-spike/ui/tokens.css` |
| `[D]` | Факт дампа эталонного клиента (DOM, ключи локализации `ru-catalog.json`, имена native API, флаги фич) | `waves/RW-40/DESIGN/STREAM.md` §0.1, §6; `waves/RW-40/DESIGN/stream-spec.json` |
| `[S]` | Подтверждено кодом эталонного клиента (`4d2a64cb5df5fc4e_0`) или реализованным контрактом в `discord-core` / `discord-voice` | `STREAM-GATEWAY-CONTRACT.md` §0.1; `rust/crates/discord-core/src/stream.rs`; `rust/crates/discord-voice/src/session.rs` |
| `[M]` | Измерено по пикселям дампа `01-joined.png` (`2560×1383 @ dpr 1.25 = 2048×1106 CSS`) | `waves/RW-40/DESIGN/VOICE.md` §4.1; `waves/RW-40/DESIGN/STREAM.md` §4.1 |
| `[L]` | Наблюдено в живом трафике / логах эталонного клиента; для UI-дампа — `reference-oracle/{voice-deep,frontend-d03}.md` и `docs/re/voice-deep.md` (опкоды `18..=22` там не наблюдались, см. §0.1 п. 8) | `docs/re/voice-deep.md`; `waves/RW-40/NETWORK-CONTRACTS.md` §3 |
| `[F]` | Зафиксировано действующими тестами репозитория (`tests/unit/voice-ui.test.mjs`, `stream_gateway_contract.rs`, `session_contract.rs`) | Кодовая база репозитория |
| `[A]` | Проектное решение / адаптация нашей архитектуры (паритет с живым экраном Go Live не заявляется: в дампе `01-joined.png` активного стрима нет, `unmeasured`) | Настоящая спецификация; `waves/RW-40/DESIGN/STREAM.md` §0.2 |

### 0.3. Жёсткие архитектурные инварианты

1. **100% привязка к `rust/gui-spike/ui/tokens.css` (`0` сырых цветов, `0` новых токенов, `0` `!important`)**:
   - Нормативный CSS-блок (§3.1) использует исключительно объявленные в `rust/gui-spike/ui/tokens.css` переменные `var(--*)` без fallback-значений `var(--x, fallback)`, без `#hex`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, без `!important` и без `:has()`.
2. **Строгое разделение слоёв (`G2 UI` ↔ `G5 VoiceSession` ↔ `G1 StreamState`)**:
   - UI (`G2`) **никогда** не конструирует и не парсит строку `stream_key` (`guild:<guild_id>:<channel_id>:<user_id>` или `call:<channel_id>:<user_id>`), не хранит `token` или `endpoint` второго RTC-соединения, не формирует JSON/ETF-кадры `op 18..=22` напрямую и не обращается к `navigator.mediaDevices`, `getUserMedia`, `getDisplayMedia`, `RTCPeerConnection`, `fetch` или `WebSocket`.
   - Защита секретов и ключей (`D-27-1`, `D-27-4`, `D-27-6`, `SG-06` `[S]` `[F]`): полная проводная строка `stream_key`, секретный `token` (`StreamSecret`, `Debug`/`Display` → `<redacted>`) и `endpoint` второго RTC **никогда не попадают** в DOM-атрибуты (`id`, `data-*`, `title`, `aria-label`, `textContent`), в `StreamSnapshot` или в диагностические логи. В DOM плитки и оверлеи идентифицируют поток по `data-user-id="<owner_id>"` и `data-stream-kind="guild|call"` (либо непрозрачному UI-индексу `data-tile-id`), а команды сервиса адресуются через структурный дескриптор `{ kind, guildId, channelId, ownerId }` либо методы активной сессии.
3. **Запрет оптимистичных переходов (`D-27-3`, `A-REDUCER-START`, `A-STOP-HELPER`, `A-PAUSE-UI` `[S]` `[F]`)**:
   - Клик по кнопке старта (`#stream-start`) отправляет намерение сервису (`op 18 STREAM_CREATE`), переводит UI в фазу ожидания подтверждения (`data-stream="starting"`, `aria-busy="true"`), но **не включает** индикатор `LIVE` (`data-stream="live"`) до прихода серверного `op 0 STREAM_CREATE`.
   - Клик по кнопке остановки (`#stream-stop`) отправляет `op 19 STREAM_DELETE` (`VoiceSession::stop_screen_share`), переводит UI в `data-stream="stopping"` (`aria-busy="true"`), но **не снимает** активный стрим и **не отвязывает** ключ сессии до прихода серверного `op 0 STREAM_DELETE` (`H-DELETE-END`).
   - Клик по кнопке паузы (`#stream-pause-toggle`) отправляет `op 22 STREAM_SET_PAUSED` (`VoiceSession::set_stream_paused`), но атрибут `aria-pressed` и фаза `paused` меняются **только** по серверному `STREAM_CREATE` / `STREAM_UPDATE` с полем `paused` (`H-PAUSE`).
4. **Согласование слоёв `discord-core::stream::StreamState` и `discord-voice::session::VoiceSession` (`P2-1` / `P2-6`)**:
   - В `discord-core::stream::apply` (`stream.rs:1457-1460`) локальный `LocalStreamAction::StartRequested` не создаёт запись в `StreamState` до прихода `STREAM_CREATE`, тогда как `LocalStreamAction::WatchRequested` (`stream.rs:1461-1477`) сразу вставляет запись в фазе `StreamPhase::Connecting` (`"CONNECTING"`).
   - В `discord-voice::session::Session::start_screen_share` / `start_call_screen_share` (`session.rs:1333-1406`) сессия привязывает `active_stream_key = Some(key)` и выставляет транспортную фазу `stream_phase = Some(StreamLifecyclePhase::Connecting)` для фильтрации входящих `on_stream_dispatch` по `is_own_stream_key` / `is_active_stream_key`.
   - При терминальном закрытии голосовой сессии (`LeaveRequested`, `WsClosed(1000)`, `GaveUp`, фатальная ошибка) `clear_stream_record()` (`session.rs:1664-1670`) атомарно очищает `active_stream_key`, `stream_phase`, `stream_viewer_ids`, `stream_transport_ready` и `last_stream_delete_reason` (инвариант `active_stream_key.is_some() == stream_phase.is_some()`, `t_v01_20`).
5. **MVP-режим без передачи видеокадров в WebView (`F-01` Вариант B и криптоблокер `B-08` `[A]` `[S]`)**:
   - Согласно принятому решению `F-01` (Вариант B, `STREAM.md` §2.4) и `G5-VOICE-PLAN.md` §7.4 пп. 6, 8, видеокадры в MVP не сериализуются через Tauri IPC в WebView: `#stream-preview`, `.stream-tile-surface` и `#stream-viewer-canvas` рендерят детерминированный плейсхолдер `data-empty="true"`.
   - Реальный wire-путь второго RTC-соединения и декодирования потока зрителем ограничен криптографическим блокером **B-08** (схема KDF/nonce/AAD и 14-байтная соль media stream в `op 4`, `STREAM.md` §1, §10 `SG-18`); UI честно отражает состояния сигнализации `op 18..=22` и `transport_ready` без имитации живого видеопотока.
6. **Сохранение замороженных инвариантов `#voice-panel` (`V-08` `[F]`)**:
   - Все существующие узлы и атрибуты `#voice-panel` из `rust/gui-spike/ui/features/voice.js` и `tests/unit/voice-ui.test.mjs` (`12` состояний `VOICE_STATES` в `data-state`, `#voice-status`, `#voice-participants`, `#voice-user-row`, `#voice-a11y`, компактная геометрия по `F-04`). Геометрия принадлежит **кнопке**, а не контейнеру: `.voice-action` — `height: var(--btn-height-sm)` (`28px`), `padding: 0 var(--space-2)` (`8px`), `border-radius: var(--radius-xs)` (`4px`) (`voice.css:108-119`); `.voice-actions` — только `display: flex; flex-wrap: wrap; gap: var(--space-1)` (`2px`) (`voice.css:245-248`). Оракулы D8 проверяют `.voice-action`, а не `.voice-actions`.

---

## 1. Связка UI с протокольным контрактом Gateway Opcodes 18..=22 и Dispatches

### 1.1. Таблица исходящих команд (`StreamOp` / `StreamGatewayOp` `18..=22`, клиент → сервер)

Все команды отправляются только при установленной сессии (`GatewaySessionState::Established` в `discord-core` и `SessionState::Connected` + `Phase::Established` в `discord-voice`, правило `D-27-3`). При попытке вызова вне установленной сессии возвращается типизированная ошибка `StreamError::SessionNotEstablished` (`code() == "stream.session.not_established"`) / `SessionError::StreamNotEstablished`, кадр в сокет не уходит, состояние UI переходит в ошибку с `data-reason="session-not-established"`.

| Опкод | Имя (`StreamOp` / `StreamGatewayOp`) | Метод `discord-core` / `discord-voice` | Проводной JSON/ETF-кадр (`{"op": N, "d": {...}}`) | Триггер в UI (`G2`) | Неоптимистичный отклик UI до ответа сервера | Метка |
|---:|---|---|---|---|---|---|
| **`18`** | `STREAM_CREATE` (`Create`) | `StreamCommand::Create` (`stream.rs:423`) / `Session::start_screen_share`, `Session::start_call_screen_share` (`session.rs:1333, 1379`) | Для гильдии: `{"op":18,"d":{"type":"guild","guild_id":"<u64>","channel_id":"<u64>","preferred_region":"<ascii≤64>"\|null}}`<br>Для звонка (`D-27-2`): `{"op":18,"d":{"type":"call","guild_id":null,"channel_id":"<u64>","preferred_region":"<ascii≤64>"\|null}}` | Клик `#stream-start` («Начать стрим») в диалоге `#stream-setup` после выбора источника ОС | `#voice-panel[data-stream="starting"]`, `#stream-setup[aria-busy="true"]`, `#stream-start[aria-disabled="true"]`. Индикатор `LIVE` ещё **скрыт** (`hidden`). В `VoiceSession` фаза становится `Connecting` | `[S]` `[F]` |
| **`19`** | `STREAM_DELETE` (`Delete`) | `StreamCommand::Delete` (`stream.rs:435`) / `Session::stop_screen_share` (`session.rs:1414`) | `{"op":19,"d":{"stream_key":"guild:<g>:<c>:<u>"\|"call:<c>:<u>"}}` | Клик `#stream-stop` («Прекратить стрим») в панели сцены `#stream-stage`. Только собственный стрим сессии: `stop_screen_share()` берёт `live_stream_key()`, у зрителя вернёт `NoActiveStream`; выход зрителя — `LocalStreamAction::Close` без опкода (§2.2.3) | `#voice-panel[data-stream="stopping"]`, `#stream-stop[aria-busy="true"][aria-disabled="true"]`. Фаза стрима и `active_stream_key` **не сбрасываются** до прихода `op 0 STREAM_DELETE` (`P1-2`, `H-DELETE-END`) | `[S]` `[F]` |
| **`20`** | `STREAM_WATCH` (`Watch`) | `StreamCommand::Watch` (`stream.rs:440`) / `Session::watch_stream` (`session.rs:1423`) / `LocalStreamAction::WatchRequested` (`stream.rs:1461`) | `{"op":20,"d":{"stream_key":"guild:<g>:<c>:<u>"\|"call:<c>:<u>"}}` | Клик `.stream-tile-watch-btn` («Смотреть стрим», ключ `E5RDnK`) или активация плитки `.stream-tile` чужого стрима | В `StreamState` создаётся/перезаписывается запись с `phase = StreamPhase::Connecting` (`"CONNECTING"`). Плитка `.stream-tile` и `#stream-viewer-overlay` переходят в `data-phase="connecting"`, `aria-busy="true"` | `[S]` `[F]` |
| **`21`** | `STREAM_PING` (`Ping`) | `StreamCommand::Ping` (`stream.rs:445`) / `Session::ping_stream` (`session.rs:1433`) | `{"op":21,"d":{"stream_key":"guild:<g>:<c>:<u>"\|"call:<c>:<u>"}}` | Автоматический триггер при переходе RTC-соединения стрима в `DISCONNECTED` с `willReconnect` (`A-PING-CALLSITE`), либо клик `#stream-ping-retry` в состоянии `reconnecting`. Только собственный стрим: `ping_stream()` берёт `live_stream_key()`, у зрителя вернёт `NoActiveStream`, поэтому кнопка стоит в панели сцены владельца, а не в оверлее зрителя | Фаза стрима **не меняется** (§4.3 контракта); кнопка `#stream-ping-retry` получает `aria-busy="true"` на время проверки соединения | `[S]` `[F]` |
| **`22`** | `STREAM_SET_PAUSED` (`SetPaused`) | `StreamCommand::SetPaused` (`stream.rs:450`) / `Session::set_stream_paused` (`session.rs:1443`) | `{"op":22,"d":{"stream_key":"guild:<g>:<c>:<u>"\|"call:<c>:<u>","paused":true\|false}}` | Клик `#stream-pause-toggle` («Приостановить стрим» / «Возобновить стрим») владельцем стрима. Только владелец: `set_stream_paused()` работает по `live_stream_key()`; в баре зрителя и в `#stream-fullscreen` контрола паузы нет | Кнопка `#stream-pause-toggle` получает `aria-busy="true"`, но `aria-pressed` и `data-stream="paused"` **не меняются** до прихода `op 0 STREAM_UPDATE{paused}` (`A-PAUSE-UI`, `H-PAUSE`) | `[S]` `[F]` |

### 1.2. Таблица входящих серверных событий (`op 0` `STREAM_*`, сервер → клиент)

| Событие `t` (`op 0`) | Структура в `discord-core` (`stream.rs`) | Переход в `StreamState` (`apply`) и `VoiceSession` (`on_stream_dispatch`) | Отражение в DOM (`G2`) | Метка |
|---|---|---|---|---|
| **`STREAM_CREATE`** | `StreamCreated { stream_key, region, viewer_ids, rtc_server_id, rtc_channel_id, paused }` (`stream.rs:787`) | Запись создаётся/заменяется: если `paused == Some(true)` → `StreamPhase::Paused` (`"PAUSED"`), иначе `StreamPhase::Active` (`"ACTIVE"`). `transport_ready = false`, `delete_reason = None`. Эффект при повторном `STREAM_CREATE` зависит от слоя: `StreamState::apply` (`Created`, `stream.rs:1504-1530`) возвращает `Vec::new()` — `CloseStreamRtc` в редьюсере **не эмитится** `[A]`; `VoiceSession::on_stream_dispatch` (`session.rs:1488-1496`) при `stream_transport_ready == true` пушит `StreamSessionEffect::CloseStreamRtc` `[S]`. Расхождение слоёв зафиксировано, выравнивание — задача G1 (REPORT.md §5 п. 1) | Для своего стрима: `#stream-setup` закрывается (`hidden`), `#voice-panel[data-stream="live"]` (или `"paused"`), `#stream-live-badge` становится видимым (`Вы стримите!`), `#stream-stage[data-view="stage"]`. Для зрителя: плитка `.stream-tile` и `#stream-viewer-overlay` переходят в `data-phase="active"` (или `"paused"`) | `[S]` `[F]` |
| **`STREAM_SERVER_UPDATE`** | `StreamServerUpdate { stream_key, endpoint, token: StreamSecret }` (`stream.rs:804`) | **Фаза не меняется** (`H-SERVER-UPDATE`). Выставляется `transport_ready = true`, эмитится эффект `OpenStreamRtc { key, endpoint, token }`. Без записи стрима в `StreamState` увеличивается `diagnostics.orphan_server_updates` (`+1`) | На `.stream-tile` и `#stream-viewer-overlay` обновляется `data-transport-ready="true"`. Индикатор `LIVE` от одного `STREAM_SERVER_UPDATE` без `STREAM_CREATE` **не зажигается** | `[S]` `[F]` |
| **`STREAM_UPDATE`** | `StreamUpdate { stream_key, region, viewer_ids, paused }` (`stream.rs:825`) | `H-PAUSE`: `paused == Some(true)` → `Paused`, `Some(false)` → `Active`, `None` → сохраняет прежнюю фазу. `H-VIEWERS`: `viewer_ids == Some(list)` заменяет список зрителей целиком (`Some([])` → `viewer_count = Some(0)`), `None` → сохраняет прежний список (`D-27-5`) | При `Paused`: `#voice-panel[data-stream="paused"]`, `.stream-tile[data-phase="paused"][data-paused="true"]`, оверлей паузы «Стрим приостановлен» (`5q17w5`), `#stream-pause-toggle[aria-pressed="true"]`. Счётчик `#stream-viewers`: при `viewer_count == None` скрыт (`hidden`), при `Some(n)` виден с текстом `{n}` | `[S]` `[F]` |
| **`STREAM_DELETE`** | `StreamDeleted { stream_key, unavailable, reason }` (`stream.rs:898`) | Строгий приоритет `A-REDUCER-DELETE`:<br>1) `unavailable == Some(true)` → `StreamPhase::Reconnecting` (`"RECONNECTING"`), запись сохраняется;<br>2) `reason == Unauthorized` → `StreamPhase::Failed` (`"FAILED"`);<br>3) `reason == SafetyGuildRateLimited` → `StreamPhase::Ended` + `ShowSafetyRateLimit { guild_id }`;<br>4) `reason == StreamFull` → `StreamPhase::Ended` + `NotifyViewerFull { key }`;<br>5) `prev == Failed && reason == UserRequested` → `StreamPhase::Failed`;<br>6) иначе (`UserRequested`, `None`, `Unknown(raw)`) → `StreamPhase::Ended` (`"ENDED"`).<br>`transport_ready = false`; если транспорт был открыт или фаза `Active`/`Paused` — `CloseStreamRtc` | 1) `unavailable: true` → `#voice-panel[data-stream="reconnecting"]`, `.stream-tile[data-phase="reconnecting"]`, баннер «Переподключение» без `role="alert"`;<br>2) `unauthorized` → `data-stream="error"`, `#stream-error[data-reason="permission-channel"]` (`/x4knx`);<br>3) `safety_guild_rate_limited` → `data-stream="error"`, `#stream-error[data-reason="safety-rate-limited"]`;<br>4) `stream_full` → `#stream-error[data-reason="viewer-full"]` (`VVZDBL`);<br>5) `user_requested` / штатный конец → возврат своего стрима в `data-stream="off"`, для зрителя — статус «Стрим закончился.» (`Kb4Ukp`) | `[S]` `[F]` |

---

## 2. DOM-контракт компонентов Go Live / Screen Share

### 2.1. Точки монтирования в существующем shell (`rust/gui-spike/ui/index.html`)

1. **Внутри `#voice-panel` (`aside#channel-tree > section#voice-panel`)**:
   - Существующий контейнер `.voice-actions` (`rust/gui-spike/ui/features/voice.js:377`) расширяется кнопками управления видео и демонстрацией экрана:
     - `button#voice-camera.voice-action` — кнопка камеры (`data-voice-action="camera"`);
     - `button#voice-screen.voice-action` — кнопка демонстрации экрана Go Live (`data-voice-action="screen"`, текст/метка «Продемонстрируйте свой экран», ключ `fjBNo1`);
     - `span#voice-stream-live-pill.voice-stream-live-pill` — компактный индикатор `LIVE` в голосовой панели (скрыт `hidden`, когда `data-stream` не равен `"live"` или `"paused"`).
   - В строках участников `.voice-user` внутри `#voice-participants`:
     - при наличии активного стрима у участника (`VOICE_STATE_UPDATE.self_stream === true` или активная запись в `StreamSnapshot` для `owner_id === person.id`) рендерится бейдж `span.voice-user-live-badge` (`В ЭФИРЕ` / `{user} стримит` `vTPX23`). Наличие флага `self_stream` управляет только бейджем участника и **никогда не меняет** фазу жизненного цикла стрима (§4.3 `STREAM-GATEWAY-CONTRACT.md`).
2. **Внутри центральной области `main.content`, прямым потомком (сиблинг `#chat-header` и `#screen-chat`)**:
   - `section#stream-stage.stream-stage` — сцена демонстрации экрана: **оверлей** над областью чата по `D-01` (`STREAM.md` §4.4: «оверлей над областью чата, `inset: 0` внутри `main`»). Сцена не входит в поток `#screen-chat` и не сжимает чат: правило `.stream-stage` — `position: absolute; inset: 0` относительно `main.content`. Хост-требование к `G2`: `main.content` должен стать позиционированным контейнером (`position: relative` в `style.css`); без этого `inset` отсчитается от вьюпорта. `#screen-chat` остаётся в DOM под оверлеем, но не получает фокус и клики (их перехватывает сцена), `#stream-stage` скрыта при `data-view="hidden"` и атрибуте `hidden`. Внутри: верхняя панель `.stream-stage-bar`, сетка плиток `#stream-tiles`, оверлей активного просмотра `#stream-viewer-overlay`, пустое состояние `#stream-empty`, скелетон подключения `#stream-loading` и единственный регион ошибки `#stream-error[role="alert"]`. Видимость: `data-view="stage"` — экран стримов канала открыт (в том числе когда стримов нет и виден `#stream-empty`) либо идёт свой стрим/просмотр; `data-view="hidden"` — чат открыт без оверлея. Пока стрим не активен (`data-stream="off"`), `Escape` закрывает сцену и возвращает чат; при активном стриме или просмотре сцена по `Escape` не закрывается, иначе теряется управление паузой и остановкой.
3. **Модальные листы верхнего уровня (`body`, слой `--z-modal: 200`)**:
   - `section#stream-setup.stream-setup` — диалог настройки и подтверждения запуска Go Live (`role="dialog"`, `aria-modal="true"`);
   - `div#stream-fullscreen.stream-fullscreen` — модальный полноэкранный контейнер просмотра (`role="dialog"`, `aria-modal="true"`);
   - `section#camera-preview.camera-preview` — диалог локального предпросмотра камеры (`role="dialog"`, `aria-modal="true"`).

### 2.2. Каноническая DOM-разметка всех 7 узлов контракта

#### 2.2.1. Кнопки Go Live / Камера и индикатор LIVE в `#voice-panel`

```html
<section id="voice-panel"
         class="voice-panel"
         aria-label="Голосовая связь"
         data-state="connected"
         data-stream="off"
         data-camera="off"
         data-capability-stream="true"
         data-capability-video="false"
         data-self-mute="false"
         data-self-deaf="false"
         data-ptt="idle">

  <!-- Существующие блоки V-08: #voice-status, #voice-participants, #voice-overflow, #voice-empty, #voice-error -->

  <!-- Ряд действий панели: контейнер .voice-actions — gap 2px; геометрия 28px / padding 0 8px / радиус 4px принадлежит кнопке .voice-action (F-04) -->
  <div id="voice-actions" class="voice-actions" role="group" aria-label="Действия голосовой связи">
    <button id="voice-camera"
            class="voice-action"
            type="button"
            data-voice-action="camera"
            aria-label="Камера недоступна"
            aria-pressed="false"
            aria-disabled="true">
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M15 10l4.553-2.276A1 1 0 0 1 21 8.618v6.764a1 1 0 0 1-1.447.894L15 14v-4z" fill="none" stroke="currentColor" stroke-width="2" />
        <rect x="3" y="6" width="12" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="2" />
      </svg>
      <span class="voice-action-label">Камера</span>
    </button>

    <button id="voice-screen"
            class="voice-action"
            type="button"
            data-voice-action="screen"
            aria-label="Продемонстрируйте свой экран"
            aria-pressed="false"
            aria-haspopup="dialog">
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="2" y="3" width="20" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="2" />
        <path d="M8 21h8M12 17v4" fill="none" stroke="currentColor" stroke-width="2" />
      </svg>
      <span id="voice-screen-label" class="voice-action-label">Экран</span>
    </button>

    <span id="voice-stream-live-pill"
          class="voice-stream-live-pill"
          role="status"
          aria-live="polite"
          hidden>
      <span class="stream-live-dot" aria-hidden="true"></span>
      <span id="voice-stream-live-text" class="voice-stream-live-text">Вы стримите!</span>
    </span>

    <!-- Существующие условные кнопки V-08: #voice-enable-ptt, #voice-mic-test -->
    <button id="voice-enable-ptt" class="voice-action" type="button" data-voice-action="enable-ptt" aria-label="Перейти на режим рации" hidden>Перейти на режим рации</button>
    <button id="voice-mic-test" class="voice-action" type="button" data-voice-action="mic-test" aria-label="Проверка микрофона" hidden>Проверка микрофона</button>
  </div>

  <!-- Существующие блоки V-08: #voice-user-row (#voice-mute, #voice-deafen), #voice-a11y -->
</section>
```

#### 2.2.2. Диалог настройки запуска Go Live (`#stream-setup`)

```html
<section id="stream-setup"
         class="stream-setup"
         role="dialog"
         aria-modal="true"
         aria-labelledby="stream-setup-title"
         aria-describedby="stream-setup-hint"
         data-state="preview"
         hidden>
  <header class="stream-setup-header">
    <h2 id="stream-setup-title" class="stream-setup-title">Настройки стрима</h2>
    <p id="stream-setup-hint" class="stream-setup-hint">
      Используйте меню операционной системы, чтобы выбрать источник захвата экрана
    </p>
  </header>

  <!-- F-01 Вариант B: плейсхолдер без передачи кадров в WebView -->
  <div id="stream-preview"
       class="stream-preview"
       role="img"
       aria-label="Предпросмотр того, что увидят зрители"
       data-empty="true"
       data-source="screen">
    <span id="stream-preview-caption" class="stream-preview-caption">Предпросмотр недоступен</span>
  </div>

  <div class="stream-setup-row" role="group" aria-label="Параметры качества передачи">
    <button id="stream-quality"
            class="stream-quality-btn"
            type="button"
            aria-haspopup="menu"
            aria-expanded="false"
            aria-controls="stream-quality-menu">
      Качество передачи: <span id="stream-quality-value">Авто</span>
    </button>
    <div id="stream-quality-menu"
         class="stream-quality-menu menu"
         role="menu"
         aria-label="Качество передачи"
         hidden>
      <button id="stream-quality-auto"
              class="stream-quality-option"
              type="button"
              role="menuitemradio"
              aria-checked="true"
              data-quality="auto"
              tabindex="0">Авто</button>
      <button id="stream-quality-source"
              class="stream-quality-option"
              type="button"
              role="menuitemradio"
              aria-checked="false"
              data-quality="source"
              tabindex="-1">Исходное</button>
    </div>
  </div>

  <footer class="stream-setup-actions">
    <button id="stream-cancel" class="stream-btn stream-btn--secondary" type="button">Отмена</button>
    <button id="stream-start" class="stream-btn stream-btn--primary" type="button">Начать стрим</button>
  </footer>
</section>
```

#### 2.2.3. Сцена демонстрации экрана (`#stream-stage`), индикатор LIVE, плитка стрима (`.stream-tile`), оверлей просмотра (`#stream-viewer-overlay`) и контролы паузы/выхода

Роли разведены по слоям (`P1-2`): владелец стрима распоряжается `op 19` (`#stream-stop`), `op 21` (`#stream-ping-retry`) и `op 22` (`#stream-pause-toggle`), потому что все три метода `Session` берут ключ из `live_stream_key()`; зритель не имеет `active_stream_key` и получает `SessionError::NoActiveStream`. Поэтому у зрителя: выход — `LocalStreamAction::Close { key }` в `discord-core` (`stream.rs:1483-1490`: удаление записи плюс `CloseStreamRtc` при открытом транспорте), без опкода; полный экран — локальное состояние `#stream-fullscreen`; паузы и ping в его барах нет. Опкод `op 20 STREAM_WATCH` — единственная сетевая команда зрителя.

```html
<section id="stream-stage"
         class="stream-stage"
         role="region"
         aria-label="Демонстрация экрана"
         data-view="hidden"
         data-source=""
         data-stream-phase="idle"
         data-capability-stream="true"
         hidden>

  <!-- 1. Верхняя панель сцены (.stream-stage-bar): индикатор LIVE, таймер, зрители, глобальные контролы -->
  <header id="stream-stage-bar" class="stream-stage-bar" role="toolbar" aria-label="Панель активной трансляции">
    <div class="stream-stage-meta">
      <span id="stream-live-badge"
            class="stream-live-badge"
             data-state="live"
            hidden>
        <span class="stream-live-dot" aria-hidden="true"></span>
        <span id="stream-live-badge-text">Вы стримите!</span>
      </span>

      <span id="stream-timer"
            class="stream-timer"
            aria-hidden="true">00:00</span>

      <!-- D-27-5: при viewer_count === null атрибут hidden присутствует; при viewer_count === 0 показывает "Стримят / Зрители: 0" -->
      <span id="stream-viewers"
            class="stream-viewers"
            role="status"
            aria-live="polite"
            hidden></span>

      <span id="stream-quality-notice"
            class="stream-quality-notice"
            role="status"
            aria-live="polite"
            hidden>Качество передачи снижено</span>
    </div>

    <div class="stream-stage-controls" role="group" aria-label="Управление своим стримом">
      <!-- Повторная проверка соединения (op 21 STREAM_PING) при Reconnecting — только владелец -->
      <button id="stream-ping-retry"
              class="stream-btn stream-btn--secondary"
              type="button"
              data-stream-action="ping"
              aria-label="Проверить соединение стрима"
              hidden>Проверить связь</button>

      <!-- Контрол паузы (op 22 STREAM_SET_PAUSED) — виден владельцу активного стрима -->
      <button id="stream-pause-toggle"
              class="stream-btn stream-btn--secondary"
              type="button"
              data-stream-action="toggle-pause"
              aria-pressed="false"
              aria-label="Приостановить стрим"
              hidden>Приостановить стрим</button>

      <!-- Контрол остановки своего стрима (op 19 STREAM_DELETE) -->
      <button id="stream-stop"
              class="stream-btn stream-btn--danger"
              type="button"
              data-stream-action="stop"
              aria-label="Прекратить стрим"
              hidden>Прекратить стрим</button>
    </div>
  </header>

  <!-- 2. Состояние загрузки / подключения к сцене (loading / connecting) -->
  <div id="stream-loading"
       class="stream-loading"
       role="status"
       aria-live="polite"
       aria-busy="false"
       hidden>
    <div class="stream-skeleton-tile" aria-hidden="true"></div>
    <p id="stream-loading-text" class="stream-loading-text">Подключение...</p>
  </div>

  <!-- 3. Сетка плиток стримов (#stream-tiles и .stream-tile) -->
  <div id="stream-tiles"
       class="stream-tiles"
       role="list"
       aria-label="Активные трансляции в канале">

    <!-- Шаблон одной плитки стрима (.stream-tile); SG-06: raw stream_key в DOM не рендерится -->
    <article class="stream-tile"
             role="listitem"
             data-user-id="100000000000000001"
             data-stream-kind="guild"
             data-phase="active"
             data-paused="false"
             data-watching="false"
             data-transport-ready="true">
      <div class="stream-tile-card"
           role="button"
           tabindex="0"
           aria-label="ShadowByte стримит, Смотреть стрим"
           aria-pressed="false">
        <div class="stream-tile-surface" data-empty="true">
          <span class="stream-tile-placeholder-text">Демонстрация экрана</span>
          <div class="stream-tile-status-overlay" hidden>
            <span class="stream-tile-status-text">Стрим приостановлен</span>
          </div>
        </div>
        <footer class="stream-tile-footer">
          <span class="stream-tile-avatar" aria-hidden="true">SB</span>
          <span class="stream-tile-name">ShadowByte</span>
          <span class="stream-tile-live-badge" data-phase="active">
            <span class="stream-live-dot" aria-hidden="true"></span>
            <span class="stream-tile-live-text">В ЭФИРЕ</span>
          </span>
          <span class="stream-tile-viewers" aria-label="Зрителей: 3">3</span>
        </footer>
      </div>
      <div class="stream-tile-actions">
        <button class="stream-tile-watch-btn stream-btn stream-btn--primary"
                type="button"
                data-stream-action="watch"
                data-user-id="100000000000000001"
                aria-label="Смотреть стрим: ShadowByte">Смотреть стрим</button>
      </div>
    </article>
  </div>

  <!-- 4. Оверлей просмотра выбранного стрима (#stream-viewer-overlay) -->
  <section id="stream-viewer-overlay"
           class="stream-viewer-overlay"
           role="region"
           aria-label="Просмотр трансляции"
           data-phase="idle"
           data-paused="false"
           data-transport-ready="false"
           data-user-id=""
           hidden>
    <div id="stream-viewer-canvas"
         class="stream-viewer-canvas"
         role="img"
         aria-label="Окно просмотра трансляции"
         data-empty="true">
      <p id="stream-viewer-status-banner"
         class="stream-viewer-status-banner"
         role="status"
         aria-live="polite">Смотрит стрим</p>
    </div>

    <!-- Нижняя панель контролов оверлея просмотра (пауза / пинг / полный экран / выход) -->
    <footer id="stream-viewer-controls"
            class="stream-viewer-controls"
            role="toolbar"
            aria-label="Управление просмотром трансляции">
      <div class="stream-viewer-streamer-info">
        <span id="stream-viewer-streamer-name" class="stream-viewer-streamer-name"></span>
        <span id="stream-viewer-phase-badge" class="stream-live-badge" data-state="watching">Смотрит стрим</span>
      </div>

      <div class="stream-viewer-buttons" role="group" aria-label="Действия трансляции">
        <!-- Кнопка полноэкранного режима (IxVmZi / 2nM3Pk) -->
        <button id="stream-fullscreen-toggle"
                class="stream-btn stream-btn--secondary"
                type="button"
                data-stream-action="fullscreen"
                aria-pressed="false"
                aria-label="Открыть на весь экран">Открыть на весь экран</button>

        <!-- Выход зрителя: только LocalStreamAction::Close (discord-core, stream.rs:1483-1490),
             без опкода. op 19 остаётся за владельцем своего стрима (live_stream_key) -->
        <button id="stream-leave-watch"
                class="stream-btn stream-btn--danger"
                type="button"
                data-stream-action="leave-watch"
                aria-label="Прекратить просмотр стрима">Выйти из просмотра</button>
      </div>
    </footer>
  </section>

  <!-- 5. Пустое состояние сцены (#stream-empty, role="status") -->
  <div id="stream-empty"
       class="stream-empty empty-state"
       role="status"
       aria-live="polite"
       hidden>
    <p id="stream-empty-title" class="stream-empty-title">Никто не стримит</p>
    <p id="stream-empty-hint" class="stream-empty-hint">
      Нажмите «Продемонстрируйте свой экран», чтобы начать трансляцию Go Live с помощью системного меню.
    </p>
  </div>

  <!-- 6. Единственный assertive-регион ошибок стрима (#stream-error, role="alert") -->
  <div id="stream-error"
       class="stream-error"
       role="alert"
       aria-live="assertive"
       data-reason=""
       hidden>
    <p id="stream-error-text" class="stream-error-text"></p>
    <div class="stream-error-actions">
      <button id="stream-error-retry"
              class="stream-btn stream-btn--primary"
              type="button"
              data-stream-action="retry"
              hidden>Повторить</button>
      <button id="stream-error-dismiss"
              class="stream-btn stream-btn--secondary"
              type="button"
              data-stream-action="dismiss">Закрыть</button>
    </div>
  </div>
</section>

<!-- 7. Модальный полноэкранный контейнер (#stream-fullscreen, слой --z-modal: 200) -->
<div id="stream-fullscreen"
     class="stream-fullscreen"
     role="dialog"
     aria-modal="true"
     aria-label="Полноэкранный режим"
     data-controls-visible="true"
     hidden>
  <div id="stream-fullscreen-viewport" class="stream-fullscreen-viewport" data-empty="true"></div>
  <footer id="stream-fullscreen-bar"
          class="stream-fullscreen-bar"
          role="toolbar"
          aria-label="Контролы полноэкранного режима">
    <span id="stream-fullscreen-title" class="stream-fullscreen-title"></span>
    <div class="stream-fullscreen-actions">
      <!-- Паузы здесь нет: #stream-fullscreen открывается только для просмотра чужого стрима,
           а op 22 требует live_stream_key() владельца -->
      <button id="stream-fullscreen-close"
              class="stream-btn stream-btn--danger"
              type="button"
              data-stream-action="exit-fullscreen"
              aria-label="Выйти из полноэкранного режима">Закрыть</button>
    </div>
  </footer>
</div>
```

### 2.3. Полный реестр DOM ID и `data-*` атрибутов

| Селектор / Атрибут | Допустимые значения | Владелец / Источник истины | Назначение | Метка |
|---|---|---|---|---|
| `#voice-panel[data-stream]` | `"off" \| "picking" \| "preview" \| "starting" \| "live" \| "paused" \| "reconnecting" \| "stopping" \| "error"` | `StreamState` (`discord-core`) + UI-фазы пикера | Единственный источник стилей кнопки `#voice-screen` и бейджа `#voice-stream-live-pill` | `[S]` `[A]` |
| `#voice-panel[data-camera]` | `"off" \| "preview" \| "live" \| "error"` | `AppliedDeviceSelection.self_video` + лист камеры | Состояние кнопки `#voice-camera` | `[S]` `[A]` |
| `#voice-panel[data-capability-stream]` | `"true" \| "false"` | Вычисленное право канала `STREAM` (`1 << 9 = 512`) | Блокировка `#voice-screen` (`aria-disabled="true"`) при отсутствии права `STREAM` | `[S]` `[A]` |
| `#stream-stage[data-view]` | `"hidden" \| "stage"` | UI-контроллер сцены | Видимость сцены: `stage` — оверлей поверх области чата (`D-01`), `hidden` — сцена не рисуется. Полный экран живёт в отдельном `#stream-fullscreen` и значением `data-view` не выражается | `[A]` |
| `#stream-stage[data-stream-phase]` | `"idle" \| "connecting" \| "active" \| "paused" \| "reconnecting" \| "ended" \| "failed"` | `StreamPhase::as_lowercase()` (`stream.rs:1076-1090`) для шести значений; `"idle"` — **UI-only** (до первой записи `StreamSnapshot.streams`, метод его не возвращает) | Прямое отражение фазы редьюсера `discord-core::stream::StreamPhase` плюс UI-значение `idle` | `[S]` `[F]` `[A]` |
| `.stream-tile[data-user-id]` | Десятичный `u64` snowflake (`owner_id`) | `StreamKey.owner_id` (`stream.rs:155`) | Идентификация владельца стрима без экспозиции полной строки `stream_key` (`SG-06`) | `[S]` `[F]` |
| `.stream-tile[data-stream-kind]` | `"guild" \| "call"` | `StreamKind::as_wire()` (`stream.rs:43`) | Тип стрима (голосовой канал гильдии или звонок DM/Group DM) | `[S]` `[F]` |
| `.stream-tile[data-phase]` | `"connecting" \| "active" \| "paused" \| "reconnecting" \| "ended" \| "failed"` | `StreamPhase::as_lowercase()` | Состояние конкретной плитки стрима | `[S]` `[F]` |
| `.stream-tile[data-paused]` | `"true" \| "false"` | `Stream.paused` (`stream.rs:1118`) | Флаг паузы для отображения `.stream-tile-status-overlay` (`5q17w5`) | `[S]` `[A]` |
| `.stream-tile[data-transport-ready]` | `"true" \| "false"` | `StreamSummary.transport_ready` (`stream.rs:1218`) | Получен ли `STREAM_SERVER_UPDATE` для данного потока (`H-SERVER-UPDATE`) | `[S]` `[F]` |
| `#stream-error[data-reason]` | `"permission-channel" \| "permission-os" \| "portal-unavailable" \| "os-outdated" \| "region-blocked" \| "invalid-command" \| "invalid-stream-key" \| "invalid-region" \| "invalid-paused" \| "session-not-established" \| "start-failed" \| "stop-timeout" \| "encoder-failed" \| "viewer-full" \| "safety-rate-limited"` | Три источника, маппинг — §2.4: `StreamDeleteReason::ui_data_reason()` (`stream.rs:886-897`), `StreamError::code()` (`stream.rs:1321-1338`) и UI-only коды пикера/таймаутов | Машиночитаемый код причины отказа для тестов и локализации; Rust-коды в DOM не попадают | `[S]` `[D]` `[A]` |

#### 2.3.1. Полный перечень DOM ID канонической разметки (§2.2.1–§2.2.3)

53 уникальных `id`, ни одного сверх разметки. Оракул полноты — `grep -oE 'id="[a-z0-9-]+"'` по §2.2 против этой таблицы.

| `id` | Тег / `role` | Блок | Назначение |
|---|---|---|---|
| `#voice-panel` | `<section>` | §2.2.1 | Корень голосовой панели; носитель `data-stream`, `data-camera`, `data-capability-*` |
| `#voice-actions` | `<div>`, `role="group"` | §2.2.1 | Ряд действий панели; контейнер геометрии `gap 2px` (`F-04`) |
| `#voice-camera` | `<button>` | §2.2.1 | Кнопка камеры (вне MVP, `aria-disabled`) |
| `#voice-screen` | `<button>` | §2.2.1 | Кнопка Go Live / выбора источника (`fjBNo1`) |
| `#voice-screen-label` | `<span>` | §2.2.1 | Текст кнопки «Экран» (усечение `ellipsis`) |
| `#voice-stream-live-pill` | `<span>`, `role="status"` | §2.2.1 | Компактный индикатор `LIVE` в панели |
| `#voice-stream-live-text` | `<span>` | §2.2.1 | Текст пилюли `LIVE` (`XvBdeT`) |
| `#voice-enable-ptt` | `<button>` | §2.2.1 | Условная кнопка `V-08` «Перейти на режим рации» |
| `#voice-mic-test` | `<button>` | §2.2.1 | Условная кнопка `V-08` «Проверка микрофона» |
| `#stream-setup` | `<section>`, `role="dialog"` | §2.2.2 | Диалог настройки и подтверждения Go Live |
| `#stream-setup-title` | `<h2>` | §2.2.2 | Заголовок диалога (`aria-labelledby`) |
| `#stream-setup-hint` | `<p>` | §2.2.2 | Подсказка про системный пикер (`aria-describedby`) |
| `#stream-preview` | `<div>`, `role="img"` | §2.2.2 | Плейсхолдер предпросмотра (`data-empty="true"`, `F-01`) |
| `#stream-preview-caption` | `<span>` | §2.2.2 | Подпись «Предпросмотр недоступен» |
| `#stream-quality` | `<button>` | §2.2.2 | Кнопка меню качества (`aria-haspopup="menu"`) |
| `#stream-quality-value` | `<span>` | §2.2.2 | Текущее значение качества (`Авто` / `Исходное`) |
| `#stream-quality-menu` | `<div>`, `role="menu"` | §2.2.2 | Меню качества |
| `#stream-quality-auto` | `<button>`, `role="menuitemradio"` | §2.2.2 | Опция «Авто» (`data-quality="auto"`) |
| `#stream-quality-source` | `<button>`, `role="menuitemradio"` | §2.2.2 | Опция «Исходное» (`data-quality="source"`) |
| `#stream-cancel` | `<button>` | §2.2.2 | Отмена диалога |
| `#stream-start` | `<button>` | §2.2.2 | Запуск `op 18` (`aria-disabled` в `starting`) |
| `#stream-stage` | `<section>`, `role="region"` | §2.2.3 | Сцена (оверлей `inset: 0` внутри `main.content`) |
| `#stream-stage-bar` | `<header>`, `role="toolbar"` | §2.2.3 | Верхняя панель сцены |
| `#stream-live-badge` | `<span>` | §2.2.3 | Индикатор «Вы стримите!» (`XvBdeT`) |
| `#stream-live-badge-text` | `<span>` | §2.2.3 | Текст индикатора |
| `#stream-timer` | `<span>`, `aria-hidden="true"` | §2.2.3 | Таймер стрима |
| `#stream-viewers` | `<span>`, `role="status"` | §2.2.3 | Счётчик зрителей (`D-27-5`) |
| `#stream-quality-notice` | `<span>`, `role="status"` | §2.2.3 | Плашка «Качество передачи снижено» |
| `#stream-ping-retry` | `<button>` | §2.2.3 | `op 21` при `reconnecting` — **только владелец** |
| `#stream-pause-toggle` | `<button>` | §2.2.3 | `op 22` пауза/возобновление — **только владелец** |
| `#stream-stop` | `<button>` | §2.2.3 | `op 19` остановка — **только владелец** |
| `#stream-loading` | `<div>`, `role="status"` | §2.2.3 | Скелетон подключения |
| `#stream-loading-text` | `<p>` | §2.2.3 | Текст «Подключение...» |
| `#stream-tiles` | `<div>`, `role="list"` | §2.2.3 | Сетка плиток стримов |
| `#stream-viewer-overlay` | `<section>`, `role="region"` | §2.2.3 | Оверлей просмотра выбранного стрима |
| `#stream-viewer-canvas` | `<div>`, `role="img"` | §2.2.3 | Холст просмотра (`data-empty="true"`, `F-01`) |
| `#stream-viewer-status-banner` | `<p>`, `role="status"` | §2.2.3 | Статус просмотра / «Стрим приостановлен» |
| `#stream-viewer-controls` | `<footer>`, `role="toolbar"` | §2.2.3 | Нижняя панель контролов зрителя |
| `#stream-viewer-streamer-name` | `<span>` | §2.2.3 | Имя владельца стрима |
| `#stream-viewer-phase-badge` | `<span>` | §2.2.3 | Бейдж «Смотрит стрим» (`data-state="watching"`) |
| `#stream-fullscreen-toggle` | `<button>` | §2.2.3 | Вход в `#stream-fullscreen` |
| `#stream-leave-watch` | `<button>` | §2.2.3 | Выход из просмотра — `LocalStreamAction::Close`, без опкода |
| `#stream-empty` | `<div>`, `role="status"` | §2.2.3 | Пустое состояние сцены |
| `#stream-empty-title` | `<p>` | §2.2.3 | «Никто не стримит» |
| `#stream-empty-hint` | `<p>` | §2.2.3 | Подсказка про кнопку «Продемонстрируйте свой экран» |
| `#stream-error` | `<div>`, `role="alert"` | §2.2.3 | Единственный assertive-регион ошибок |
| `#stream-error-text` | `<p>` | §2.2.3 | Локализованный текст причины |
| `#stream-error-retry` | `<button>` | §2.2.3 | «Повторить» (для `start-failed` / `encoder-failed`) |
| `#stream-error-dismiss` | `<button>` | §2.2.3 | «Закрыть» — очистка ошибки в `off` |
| `#stream-fullscreen` | `<div>`, `role="dialog"` | §2.2.3 | Полноэкранный контейнер (`2nM3Pk`) |
| `#stream-fullscreen-viewport` | `<div>` | §2.2.3 | Вьюпорт полного экрана (`data-empty="true"`) |
| `#stream-fullscreen-bar` | `<footer>`, `role="toolbar"` | §2.2.3 | Панель контролов полного экрана |
| `#stream-fullscreen-title` | `<span>` | §2.2.3 | Имя владельца в полном экране |
| `#stream-fullscreen-close` | `<button>` | §2.2.3 | Выход из полного экрана |

### 2.4. Словарь `data-reason`: три источника и явный маппинг

`#stream-error[data-reason]` заполняется только значениями из таблицы ниже. Rust-коды `StreamError::code()` (`stream.key.invalid`, `stream.field.region`, `stream.session.not_established`, …) в DOM **не попадают** — адаптер `G2` переводит их в кебаб-значения.

**2.4.1. Из `StreamDeleteReason::ui_data_reason()` (`stream.rs:886-897`, серверное `op 0 STREAM_DELETE`):**

| `StreamDeleteReason` | `ui_data_reason()` | Фаза после события |
|---|---|---|
| `Unauthorized` | `permission-channel` | `Failed` |
| `SafetyGuildRateLimited` | `safety-rate-limited` | `Ended` + `ShowSafetyRateLimit` |
| `StreamFull` | `viewer-full` | `Ended` + `NotifyViewerFull` |
| `UserRequested` | `None` (ошибку не показывает) | `Ended` |
| `Unknown(raw)` | `None` (только `diagnostics.unknown_reasons`) | `Ended` |

**2.4.2. Из `StreamError::code()` (`stream.rs:1321-1338`, локальная валидация до отправки):**

| `code()` | `data-reason` |
|---|---|
| `stream.session.not_established` | `session-not-established` |
| `stream.key.invalid` | `invalid-stream-key` |
| `stream.field.id` при разборе `stream_key` | `invalid-stream-key` |
| `stream.field.region` | `invalid-region` |
| `stream.field.paused` | `invalid-paused` |
| `stream.field.id` при валидации команды (`guild_id`) | `invalid-command` |
| `stream.field.missing` | `invalid-command` |
| `stream.field.type` | `invalid-command` |
| `stream.command.kind` | `invalid-command` |
| `stream.command.guild_id.missing` | `invalid-command` |
| `stream.command.guild_id.unexpected` | `invalid-command` |

Правило выбора для `stream.field.id`: `InvalidId { field }` возникает и в `StreamKey::parse` (`owner_id`, `channel_id`, `guild_id`), и в валидации `StreamCommand` (`guild_id`). Адаптер смотрит на точку вызова: разбор ключа → `invalid-stream-key`, валидация полей команды → `invalid-command`. Один и тот же Rust-код может дать два разных `data-reason`, обратное неверно.

**2.4.3. UI-only значения (нет Rust-кода, ставит `G2`):**

| `data-reason` | Источник в UI | Условие |
|---|---|---|
| `permission-os` | отказ системного пикера / портала | пользователь отменил выбор источника в ОС |
| `portal-unavailable` | `xdg-desktop-portal` недоступен | Wayland без портала |
| `os-outdated` | версия композитора/портала ниже требуемой | нет `ScreenCast` v4 |
| `region-blocked` | предпочтительный регион отклонён до `op 18` | локальная проверка `preferred_region` |
| `start-failed` | таймаут 15 000 мс без `STREAM_CREATE` | `starting` → `error` |
| `stop-timeout` | таймаут 10 000 мс без `STREAM_DELETE` | `stopping` → `error` |
| `encoder-failed` | ошибка кодировщика второго RTC | нативный слой вернул ошибку |
| `permission-channel` | локальная проверка бита `STREAM` (`1 << 9`) | `data-capability-stream="false"` (§4, строка 6) — то же значение, что у `Unauthorized`, текст `/x4knx` |

Проверка полноты: `grep -oE 'data-reason="[a-z-]+"'` по §2.2 даёт подмножество §2.4; оракул D3 (`T-STREAM-UI-ERR-01`) сверяет каждое значение из §2.4.1/§2.4.2 с реальным `code()` / `ui_data_reason()`.

---

## 3. Нормативный CSS-контракт (`rust/gui-spike/ui/tokens.css`) и расчёт контраста WCAG 2.1 AA

### 3.1. Полный CSS-код компонентов (`0` сырых цветов, `0` `!important`, `100%` биекция с `tokens.css`)

Все правила ниже используют исключительно кастомные свойства `var(--*)`, объявленные в `rust/gui-spike/ui/tokens.css`:

```css
/* =============================================================================
 * stream.css — UI стриминга экрана (Go Live / Screen Share, RW-40 / task-126).
 * 100% token-binding по rust/gui-spike/ui/tokens.css:
 * все значения берутся из кастомных свойств без сырых цветов и fallback-литералов.
 * ========================================================================== */

/* ---------- 1. Кнопки Go Live / Камера и пилюля LIVE в #voice-panel ---------- */

.voice-action-label {
  overflow: hidden;
  margin-left: var(--space-1);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.voice-panel[data-stream="live"] #voice-screen,
.voice-panel[data-stream="paused"] #voice-screen,
.voice-panel[data-camera="live"] #voice-camera {
  background: var(--danger-fill);
  color: var(--text-on-danger);
}

.voice-panel[data-stream="live"] #voice-screen:hover,
.voice-panel[data-stream="paused"] #voice-screen:hover,
.voice-panel[data-camera="live"] #voice-camera:hover {
  background: var(--danger-fill-hover);
  color: var(--text-on-danger);
}

.voice-panel[data-stream="picking"] #voice-screen,
.voice-panel[data-stream="starting"] #voice-screen,
.voice-panel[data-stream="stopping"] #voice-screen {
  opacity: var(--state-disabled-opacity);
  cursor: default;
}

.voice-panel[data-capability-stream="false"] #voice-screen,
.voice-panel[data-capability-video="false"] #voice-camera {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

.voice-stream-live-pill,
.stream-live-badge,
.stream-tile-live-badge,
.voice-user-live-badge {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-1);
  height: var(--badge-h);
  padding: var(--space-0) var(--badge-pad-x);
  border-radius: var(--badge-radius);
  background: var(--danger-fill);
  color: var(--text-on-danger);
  font-family: var(--font-sans);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-caption);
  letter-spacing: var(--letter-spacing-caps);
  text-transform: uppercase;
  white-space: nowrap;
}

.stream-live-badge[data-state="paused"],
.stream-tile-live-badge[data-phase="paused"] {
  background: var(--warning);
  color: var(--text-on-warning);
}

.stream-live-badge[data-state="reconnecting"],
.stream-tile-live-badge[data-phase="reconnecting"] {
  background: var(--surface-3);
  border: var(--menu-border) solid var(--warning);
  color: var(--warning);
}

.stream-live-badge[data-state="watching"] {
  background: var(--status-streaming);
  border: var(--menu-border) solid var(--text-accent);
  color: var(--text-on-danger);
}

/* Статичная точка индикатора (пульсация запрещена STREAM.md §4.7) */
.stream-live-dot {
  display: inline-block;
  width: var(--space-3);
  height: var(--space-3);
  border-radius: var(--radius-round);
  background: currentColor;
}

.voice-stream-live-pill[hidden],
.stream-live-badge[hidden],
.stream-tile-live-badge[hidden],
.voice-user-live-badge[hidden] {
  display: none;
}

/* ---------- 2. Общие кнопки стрима (.stream-btn, контраст границ >= 3:1) ---------- */

.stream-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  min-height: var(--btn-height);
  padding: var(--space-0) var(--btn-pad-x);
  border: var(--menu-border) solid transparent;
  border-radius: var(--btn-radius);
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

.stream-btn--primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--text-on-accent);
}

.stream-btn--primary:hover:not([aria-disabled="true"]) {
  background: var(--accent-hover);
  border-color: var(--accent-hover);
}

.stream-btn--primary:active:not([aria-disabled="true"]) {
  background: var(--accent-active);
  border-color: var(--accent-active);
}

/* Secondary-кнопки используют границу --text-subtle (контраст 6.48:1..7.17:1 >= 3:1 SC 1.4.11) */
.stream-btn--secondary {
  background: var(--surface-2);
  border-color: var(--text-subtle);
  color: var(--text-strong);
}

.stream-btn--secondary:hover:not([aria-disabled="true"]) {
  background: var(--surface-3);
  border-color: var(--text-strong);
}

.stream-btn--secondary[aria-pressed="true"] {
  background: var(--accent-soft);
  border-color: var(--text-accent);
  color: var(--text-strong);
}

.stream-btn--danger {
  background: var(--danger-fill);
  border-color: var(--danger-fill);
  color: var(--text-on-danger);
}

.stream-btn--danger:hover:not([aria-disabled="true"]) {
  background: var(--danger-fill-hover);
  border-color: var(--danger-fill-hover);
}

.stream-btn[aria-disabled="true"],
.stream-btn[aria-busy="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

.stream-btn[hidden] {
  display: none;
}

/* ---------- 3. Диалог настройки запуска Go Live (#stream-setup) ---------- */

.stream-setup {
  position: fixed;
  z-index: var(--z-modal);
  top: var(--space-11);
  left: var(--space-11);
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  width: var(--modal-w-md);
  max-width: var(--layout-content-max-w);
  padding: var(--modal-pad-y) var(--modal-pad-x);
  border: var(--menu-border) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  box-shadow: var(--shadow-popover);
  color: var(--text);
  transition: opacity var(--motion-base) var(--ease-out);
}

.stream-setup[hidden] {
  display: none;
}

.stream-setup-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.stream-setup-title {
  margin: var(--space-0);
  color: var(--text-strong);
  font-size: var(--font-size-title);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-title);
}

.stream-setup-hint {
  margin: var(--space-0);
  color: var(--text-muted);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.stream-preview {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  aspect-ratio: 16 / 9;
  padding: var(--space-6);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-xs);
  background: var(--surface-input);
  color: var(--text-muted);
}

.stream-preview-caption {
  color: var(--text-muted);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  text-align: center;
}

.stream-setup-row {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.stream-quality-btn {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  min-width: var(--menu-min-w);
  height: var(--btn-height);
  padding: var(--space-0) var(--space-5);
  border: var(--menu-border) solid var(--text-subtle);
  border-radius: var(--radius-xs);
  background: var(--surface-input);
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  cursor: pointer;
}

.stream-quality-menu {
  position: absolute;
  z-index: var(--z-popover);
  top: var(--input-height);
  left: var(--space-0);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: var(--menu-min-w);
  padding: var(--menu-pad);
  border: var(--menu-border) solid var(--border-strong);
  border-radius: var(--menu-radius);
  background: var(--surface-3);
  box-shadow: var(--shadow-popover);
}

.stream-quality-menu[hidden] {
  display: none;
}

.stream-quality-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: var(--row-menu-item);
  padding: var(--space-0) var(--space-4);
  border: var(--space-0);
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  text-align: left;
  cursor: pointer;
}

.stream-quality-option:hover {
  background: var(--state-hover);
  color: var(--text-strong);
}

.stream-quality-option[aria-checked="true"] {
  background: var(--accent-soft);
  color: var(--text-strong);
  font-weight: var(--font-weight-semibold);
}

.stream-setup-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
}

/* ---------- 4. Сцена демонстрации экрана (#stream-stage) и верхняя панель ---------- */

.stream-stage {
  position: absolute;
  z-index: var(--z-base);
  inset: var(--space-0);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--layout-panel-pad);
  background: var(--bg-app);
  color: var(--text);
  overflow-x: hidden;
  overflow-y: auto;
}

.stream-stage[data-view="hidden"],
.stream-stage[hidden] {
  display: none;
}

.stream-stage-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: var(--btn-height);
  padding: var(--space-4);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.stream-stage-meta,
.stream-stage-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-4);
}

.stream-timer {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.stream-viewers {
  display: inline-flex;
  align-items: center;
  height: var(--badge-h);
  padding: var(--space-0) var(--space-3);
  border-radius: var(--badge-radius);
  background: var(--badge-secondary-bg);
  color: var(--text-strong);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-caption);
}

.stream-viewers[hidden] {
  display: none;
}

.stream-quality-notice {
  color: var(--warning);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.stream-quality-notice[hidden] {
  display: none;
}

/* ---------- 5. Сетка плиток стримов (#stream-tiles и .stream-tile) ---------- */

.stream-tiles {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(var(--layout-tree-w), 1fr));
  gap: var(--space-4);
  width: 100%;
}

.stream-tiles[hidden] {
  display: none;
}

.stream-tile {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
}

.stream-tile[data-watching="true"] {
  border-color: var(--accent);
}

.stream-tile[data-phase="reconnecting"] {
  border-color: var(--warning);
}

.stream-tile[data-phase="failed"] {
  border-color: var(--danger);
}

.stream-tile-card {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  border: var(--menu-border) solid var(--text-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-3);
  color: var(--text-strong);
  overflow: hidden;
  cursor: pointer;
}

.stream-tile-card:hover {
  border-color: var(--text-strong);
}

.stream-tile-surface {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  aspect-ratio: 16 / 9;
  padding: var(--space-4);
  background: var(--surface-input);
  color: var(--text-muted);
}

.stream-tile-placeholder-text {
  color: var(--text-muted);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  text-align: center;
}

.stream-tile-status-overlay {
  position: absolute;
  inset: var(--space-0);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-4);
  background: var(--bg-app);
  color: var(--text-strong);
}

.stream-tile-status-overlay[hidden] {
  display: none;
}

.stream-tile-status-text {
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-ui);
  text-align: center;
}

.stream-tile-footer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--space-9);
  padding: var(--space-0) var(--space-4);
  background: var(--bg-app);
  color: var(--text-strong);
}

.stream-tile-avatar {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: var(--avatar-sm);
  height: var(--avatar-sm);
  border-radius: var(--radius-round);
  background: var(--surface-4);
  color: var(--text-strong);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-caption);
}

.stream-tile-name {
  overflow: hidden;
  flex: 1 1 auto;
  color: var(--text-strong);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-small);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stream-tile-viewers {
  display: inline-flex;
  align-items: center;
  height: var(--badge-h);
  padding: var(--space-0) var(--badge-pad-x);
  border-radius: var(--badge-radius);
  background: var(--badge-secondary-bg);
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.stream-tile-actions {
  display: flex;
  justify-content: flex-end;
}

/* ---------- 6. Оверлей просмотра (#stream-viewer-overlay) и Fullscreen ---------- */

.stream-viewer-overlay {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--menu-border) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.stream-viewer-overlay[hidden] {
  display: none;
}

.stream-viewer-canvas {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  aspect-ratio: 16 / 9;
  padding: var(--space-6);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-xs);
  background: var(--bg-app);
  color: var(--text-strong);
}

.stream-viewer-status-banner {
  margin: var(--space-0);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-xs);
  background: var(--surface-2);
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
  text-align: center;
}

.stream-viewer-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: var(--btn-height);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-xs);
  background: var(--surface-2);
}

.stream-viewer-streamer-info,
.stream-viewer-buttons {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.stream-viewer-streamer-name {
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-ui);
}

.stream-fullscreen {
  position: fixed;
  z-index: var(--z-modal);
  inset: var(--space-0);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: var(--bg-app);
  color: var(--text-strong);
}

.stream-fullscreen[hidden] {
  display: none;
}

.stream-fullscreen-viewport {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  width: 100%;
  padding: var(--space-8);
  background: var(--bg-app);
}

.stream-fullscreen-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: var(--btn-height);
  padding: var(--space-4);
  border-top: var(--menu-border) solid var(--border-subtle);
  background: var(--surface-1);
  transition: opacity var(--motion-base) var(--ease-out);
}

.stream-fullscreen[data-controls-visible="false"] .stream-fullscreen-bar {
  opacity: var(--state-disabled-opacity);
}

.stream-fullscreen-title {
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-ui);
}

.stream-fullscreen-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

/* ---------- 7. Пустое состояние, скелетон подключения и баннер ошибок ---------- */

.stream-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  padding: var(--space-8);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.stream-loading[hidden] {
  display: none;
}

.stream-skeleton-tile {
  width: 100%;
  max-width: var(--modal-w-sm);
  aspect-ratio: 16 / 9;
  border-radius: var(--radius-sm);
  background: var(--surface-3);
  opacity: var(--state-disabled-opacity);
}

.stream-loading-text {
  margin: var(--space-0);
  color: var(--text-subtle);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.stream-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-9);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
  text-align: center;
}

.stream-empty[hidden] {
  display: none;
}

.stream-empty-title {
  margin: var(--space-0);
  color: var(--text-strong);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
}

.stream-empty-hint {
  margin: var(--space-0);
  max-width: var(--modal-w-sm);
  color: var(--text-muted);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.stream-error {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-4);
  border: var(--menu-border) solid var(--danger);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  color: var(--danger-text);
}

.stream-error[hidden] {
  display: none;
}

.stream-error-text {
  margin: var(--space-0);
  flex: 1 1 auto;
  color: var(--danger-text);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
}

.stream-error-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

/* ---------- 8. Кольцо клавиатурного фокуса и prefers-reduced-motion ---------- */

.stream-btn:focus-visible,
.stream-quality-btn:focus-visible,
.stream-quality-option:focus-visible,
.stream-tile-card:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

@media (prefers-reduced-motion: reduce) {
  .stream-btn,
  .stream-setup,
  .stream-fullscreen-bar {
    transition: none;
  }
}
```

### 3.2. Верификация контраста по WCAG 2.1 AA (SC 1.4.3 Text Contrast $\ge 4.5:1$, SC 1.4.11 Non-Text Contrast $\ge 3.0:1$)

Все расчёты ниже выполнены по формуле относительной яркости WCAG 2.1 $L = 0.2126 R + 0.7152 G + 0.0722 B$ и $CR = (L_{\max} + 0.05) / (L_{\min} + 0.05)$ строго по фактическим значениям из `rust/gui-spike/ui/tokens.css` (с учётом sRGB альфа-композитинга для полупрозрачных токенов `--accent-soft`, `--state-hover`, `--badge-secondary-bg`):

| Компонент / Узел UI | Передний план (токен и значение в `tokens.css`) | Фон / Поверхность (токен и значение в `tokens.css`) | Тип проверки WCAG 2.1 | Расчётный контраст | Порог AA | Статус |
|---|---|---|---|---:|---:|---|
| Заголовок `#stream-setup-title`, текст `.stream-btn--secondary` | `--text-strong` (`#fbfbfb`) | `--surface-2` (`#242429`) | SC 1.4.3 Текст | **14.93:1** | $\ge 4.5:1$ | **PASS** |
| Подпись `.stream-tile-name`, `.stream-tile-status-text`, `.stream-fullscreen-title` | `--text-strong` (`#fbfbfb`) | `--bg-app` (`#121214`) | SC 1.4.3 Текст | **18.08:1** | $\ge 4.5:1$ | **PASS** |
| Заголовок `#stream-empty-title`, имя `#stream-viewer-streamer-name` | `--text-strong` (`#fbfbfb`) | `--surface-1` (`#202024`) | SC 1.4.3 Текст | **15.69:1** | $\ge 4.5:1$ | **PASS** |
| Активная опция качества `.stream-quality-option[aria-checked="true"]` | `--text-strong` (`#fbfbfb`) | `--accent-soft` (`rgb(88 101 242 / 16%)` над `--surface-3` `#28282d`) | SC 1.4.3 Текст | **12.06:1** | $\ge 4.5:1$ | **PASS** |
| Кнопка `.stream-btn--secondary[aria-pressed="true"]` (`#stream-pause-toggle`) | `--text-strong` (`#fbfbfb`) | `--accent-soft` (`rgb(88 101 242 / 16%)` над `--surface-2` `#242429`) | SC 1.4.3 Текст | **12.67:1** | $\ge 4.5:1$ | **PASS** |
| Основной текст опции `.stream-quality-option` | `--text` (`#efeff1`) | `--surface-3` (`#28282d`) | SC 1.4.3 Текст | **12.77:1** | $\ge 4.5:1$ | **PASS** |
| Таймер `#stream-timer`, текст `#stream-loading-text` | `--text-subtle` (`#abacb2`) | `--surface-1` (`#202024`) | SC 1.4.3 Текст | **7.17:1** | $\ge 4.5:1$ | **PASS** |
| Счётчик `#stream-viewers` на верхней панели сцены | `--text-strong` (`#fbfbfb`) | `--badge-secondary-bg` (`rgb(151 151 159 / 16%)` над `--surface-1` `#202024`) | SC 1.4.3 Текст | **12.14:1** | $\ge 4.5:1$ | **PASS** |
| Счётчик зрителей плитки `.stream-tile-viewers` | `--text-subtle` (`#abacb2`) | `--badge-secondary-bg` (`rgb(151 151 159 / 16%)` над `--bg-app` `#121214`) | SC 1.4.3 Текст | **6.18:1** | $\ge 4.5:1$ | **PASS** |
| Подсказка `#stream-setup-hint`, `#stream-empty-hint` | `--text-muted` (`#96979e`) | `--surface-2` (`#242429`) / `--surface-1` (`#202024`) | SC 1.4.3 Текст | **5.31:1 / 5.58:1** | $\ge 4.5:1$ | **PASS** |
| Подпись плейсхолдера `#stream-preview-caption`, `.stream-tile-placeholder-text` | `--text-muted` (`#96979e`) | `--surface-input` (`#1e1f22`) | SC 1.4.3 Текст | **5.66:1** | $\ge 4.5:1$ | **PASS** |
| Кнопка запуска `#stream-start`, `.stream-tile-watch-btn` (`.stream-btn--primary`) | `--text-on-accent` (`#ffffff`) | `--accent` (`#5865f2`) / `--accent-hover` (`#4654c0`) | SC 1.4.3 Текст | **4.61:1 / 6.38:1** | $\ge 4.5:1$ | **PASS** |
| Индикатор `#stream-live-badge`, `#voice-stream-live-pill`, `#stream-stop`, `#stream-leave-watch`, `#voice-screen[aria-pressed="true"]` | `--text-on-danger` (`#ffffff`) | `--danger-fill` (`#cf3a3f`) / `--danger-fill-hover` (`#b92d33`) | SC 1.4.3 Текст | **4.85:1 / 6.02:1** | $\ge 4.5:1$ | **PASS** |
| Индикатор просмотра `.stream-live-badge[data-state="watching"]` | `--text-on-danger` (`#ffffff`) | `--status-streaming` (`#593695`) | SC 1.4.3 Текст | **8.80:1** | $\ge 4.5:1$ | **PASS** |
| Индикатор паузы `.stream-live-badge[data-state="paused"]` | `--text-on-warning` (`#121214`) | `--warning` (`#fdb833`) | SC 1.4.3 Текст | **10.76:1** | $\ge 4.5:1$ | **PASS** |
| Индикатор переподключения `.stream-live-badge[data-state="reconnecting"]` и `#stream-quality-notice` | `--warning` (`#fdb833`) | `--surface-3` (`#28282d`) / `--surface-1` (`#202024`) | SC 1.4.3 Текст | **8.44:1 / 9.34:1** | $\ge 4.5:1$ | **PASS** |
| Текст ошибки `#stream-error-text` (`role="alert"`) | `--danger-text` (`#ff9a9e`) | `--surface-2` (`#242429`) | SC 1.4.3 Текст | **7.63:1** | $\ge 4.5:1$ | **PASS** |
| Заливка `.stream-btn--primary` относительно фона диалога/карточки | `--accent` (`#5865f2`) | `--surface-1` (`#202024`) / `--surface-2` (`#242429`) | SC 1.4.11 Non-text | **3.52:1 / 3.35:1** | $\ge 3.0:1$ | **PASS** |
| Заливка `.stream-btn--danger` и `#stream-live-badge` относительно фона | `--danger-fill` (`#cf3a3f`) | `--bg-app` (`#121214`) / `--surface-1` (`#202024`) / `--surface-2` (`#242429`) | SC 1.4.11 Non-text | **3.86:1 / 3.35:1 / 3.19:1** | $\ge 3.0:1$ | **PASS** |
| Граница интерактивных контролов `.stream-btn--secondary`, `.stream-quality-btn`, `.stream-tile-card` | `--text-subtle` (`#abacb2`) | `--surface-1` (`#202024`) / `--surface-2` (`#242429`) / `--surface-3` (`#28282d`) | SC 1.4.11 Non-text | **7.17:1 / 6.82:1 / 6.48:1** | $\ge 3.0:1$ | **PASS** |
| Граница активной кнопки паузы `.stream-btn--secondary[aria-pressed="true"]` и бейджа `watching` | `--text-accent` (`#a5b0ff`) | `--surface-1` (`#202024`) / `--surface-2` (`#242429`) / `--status-streaming` (`#593695`) | SC 1.4.11 Non-text | **7.93:1 / 7.54:1 / 4.29:1** | $\ge 3.0:1$ | **PASS** |
| Граница баннера ошибки `#stream-error` и плитки `data-phase="failed"` | `--danger` (`#da3e44`) | `--surface-2` (`#242429`) / `--bg-app` (`#121214`) | SC 1.4.11 Non-text | **3.50:1 / 4.24:1** | $\ge 3.0:1$ | **PASS** |
| Кольцо клавиатурного фокуса `:focus-visible` на всех интерактивных элементах | `--focus-ring` (`#a5b0ff`) | `--bg-app` (`#121214`) / `--surface-1` (`#202024`) / `--surface-2` (`#242429`) / `--surface-3` (`#28282d`) | SC 1.4.11 Non-text | **9.14:1 / 7.93:1 / 7.54:1 / 7.16:1** | $\ge 3.0:1$ | **PASS** |

---

## 4. Матрица состояний (Empty, Loading/Connecting, Active, Stopping, Paused, Error/Reconnect, Disabled)

| Состояние / Режим | Атрибуты `#voice-panel` и `#stream-stage` | Фаза в `discord-core::stream::StreamPhase` / `VoiceSession` | Что видимо в UI (`#voice-screen`, `#stream-live-badge`, `#stream-tiles`, `#stream-viewer-overlay`, контролы) | Доступные действия / Опкоды `18..=22` | Метка |
|---|---|---|---|---|---|
| **1. `empty` (В канале никто не стримит)** | `#voice-panel[data-stream="off"]`<br>`#stream-stage[data-view="stage"][data-stream-phase="idle"]` | `StreamSnapshot.streams` пуст (`[]`), `active_stream_key = None`, `stream_phase = None` | Кнопка `#voice-screen` активна (`aria-pressed="false"`, при `data-capability-stream="true"`). `#stream-live-badge[hidden]`, `#stream-viewer-overlay[hidden]`, `#stream-loading[hidden]`, `#stream-error[hidden]`. Блок `#stream-empty` **видим** (`hidden` снят, `role="status"`, текст «Никто не стримит») | Клик `#voice-screen` → системный пикер источника (`picking` → `preview` → `op 18 STREAM_CREATE`); `Escape` → возврат в чат (`data-view="hidden"`, фокус на `#voice-screen`) | `[S]` `[A]` |
| **2a. `loading / connecting` — запуск своего стрима (`picking` → `preview` → `starting`)** | `#voice-panel[data-stream="picking" \| "preview" \| "starting"]`<br>`#stream-stage[data-view="hidden" \| "stage"][data-stream-phase="connecting"]` | В `VoiceSession`: после `start_screen_share` `active_stream_key = Some(key)`, `stream_phase = Some(Connecting)`. В `StreamState`: запись появится по `STREAM_CREATE` | В `picking`: `#voice-screen[aria-busy="true"]`, повторный клик игнорируется. В `preview`: открыт диалог `#stream-setup` (`#stream-preview[data-empty="true"]`, выбор качества `auto`/`source`). В `starting`: отправлен `op 18 STREAM_CREATE`, `#stream-setup[aria-busy="true"]`, `#stream-start[aria-disabled="true"]`. Индикатор `#stream-live-badge` **скрыт** до `STREAM_CREATE` | В `preview`: `#stream-start` → `op 18 STREAM_CREATE`; `#stream-cancel` / `Escape` → освобождение пикера (`off`). Таймаут 15 с без `STREAM_CREATE` → `error` (`data-reason="start-failed"`, `rSlOep`) | `[S]` `[A]` `[F]` |
| **2b. `loading / connecting` — подключение зрителя к чужому стриму (`WatchRequested`)** | `#stream-stage[data-view="stage"][data-stream-phase="connecting"]`<br>`.stream-tile[data-phase="connecting"][data-watching="true"]` | `LocalStreamAction::WatchRequested` → `StreamPhase::Connecting` (`"CONNECTING"`, `transport_ready = false`) | Отправлен `op 20 STREAM_WATCH`. Блок `#stream-loading` виден (`aria-busy="true"`, `.stream-skeleton-tile`, текст «Подключение...»); в `#stream-viewer-overlay[data-phase="connecting"]` статус «Подключение...» | Кнопка `#stream-leave-watch` доступна немедленно для отмены ожидания: `LocalStreamAction::Close { key }` в `StreamState` (без опкода; `CloseStreamRtc` только при `transport_ready == true`) | `[S]` `[A]` `[F]` |
| **3. `active` (Активная трансляция — свой стрим `live` или просмотр чужого `watching`)** | Свой стрим: `#voice-panel[data-stream="live"]`, `#stream-stage[data-view="stage"][data-stream-phase="active"]`<br>Чужой стрим: `.stream-tile[data-phase="active"]` | Получен `op 0 STREAM_CREATE` / `STREAM_UPDATE` с `paused: false \| None` → `StreamPhase::Active` (`"ACTIVE"`). После `STREAM_SERVER_UPDATE`: `transport_ready = true` | `#voice-screen[aria-pressed="true"]` (заливка `--danger-fill`), `#voice-stream-live-pill` и `#stream-live-badge` видимы (`Вы стримите!` `XvBdeT`). Таймер `#stream-timer` тикает. `#stream-viewers`: скрыт при `viewer_count === null`, показывает число при `viewer_count !== null` (`D-27-5`). Плитка `.stream-tile[data-phase="active"]` с бейджем `В ЭФИРЕ`. В `#stream-viewer-overlay` — холст `data-empty="true"` (MVP `F-01`) и панель контролов | Владелец: `#stream-pause-toggle` → `op 22 STREAM_SET_PAUSED {paused: true}`; `#stream-stop` → `op 19 STREAM_DELETE`. Зритель: `.stream-tile-watch-btn` → `op 20 STREAM_WATCH`; `#stream-fullscreen-toggle` → вход в `#stream-fullscreen`; `#stream-leave-watch` → `LocalStreamAction::Close`, без опкода | `[S]` `[D]` `[F]` |
| **3b. `stopping` (Владелец отправил `op 19`, ждёт `STREAM_DELETE`)** | `#voice-panel[data-stream="stopping"]`<br>`#stream-stage[data-stream-phase="active" \| "paused"]` (фаза не меняется)<br>`.stream-tile[data-phase="active" \| "paused"]` | `stop_screen_share()` уже вернул команду, но `StreamPhase` и `active_stream_key` держатся до серверного `op 0 STREAM_DELETE` (`H-DELETE-END`, `t_v01_18`). `live_stream_key()` по-прежнему `Some`, поэтому повторный `op 19` и `op 22` технически проходят, но UI их блокирует | Индикатор `#stream-live-badge` **не скрывается**, таймер продолжает тикать. `#stream-stop[aria-busy="true"][aria-disabled="true"]`, `#stream-pause-toggle[aria-disabled="true"]`, `#voice-screen[aria-busy="true"]` (неактивна). Кнопки `#voice-camera` и действия панели доступны. В `#stream-error` ничего не пишется | Действий нет: `#stream-start`, `#stream-stop`, `#stream-pause-toggle` игнорируют клики. Приход `STREAM_DELETE` → `ended`/`off` (строка 5b). Таймаут 10 000 мс без `STREAM_DELETE` → `error`, `data-reason="stop-timeout"`, запись и ключ при этом не теряются | `[S]` `[A]` `[F]` |
| **4. `paused` (Стрим приостановлен)** | `#voice-panel[data-stream="paused"]` (для владельца)<br>`#stream-stage[data-stream-phase="paused"]`<br>`.stream-tile[data-phase="paused"][data-paused="true"]` | Получен `op 0 STREAM_CREATE{paused:true}` или `STREAM_UPDATE{paused:true}` → `StreamPhase::Paused` (`"PAUSED"`, `H-PAUSE`) | На `#stream-live-badge[data-state="paused"]` и `.stream-tile-live-badge[data-phase="paused"]` цвет `--warning` с текстом `--text-on-warning`. Поверх плитки `.stream-tile-status-overlay` (без `hidden`) и в `#stream-viewer-status-banner` выводится строка «Стрим приостановлен» (`5q17w5`). На `#stream-pause-toggle`: `aria-pressed="true"`, текст «Возобновить стрим» | Владелец: клик `#stream-pause-toggle` → `op 22 STREAM_SET_PAUSED {paused: false}` (неоптимистично ждёт `STREAM_UPDATE{paused:false}`); клик `#stream-stop` → `op 19 STREAM_DELETE`. Зритель: `#stream-leave-watch` | `[S]` `[D]` `[F]` |
| **5a. `reconnecting` (Временный обрыв стрима, `unavailable: true`)** | `#voice-panel[data-stream="reconnecting"]`<br>`#stream-stage[data-stream-phase="reconnecting"]`<br>`.stream-tile[data-phase="reconnecting"]` | Получен `op 0 STREAM_DELETE{unavailable: true}` → правило №1 `A-REDUCER-DELETE`: `StreamPhase::Reconnecting` (`"RECONNECTING"`), запись и `active_stream_key` **сохраняются**, `transport_ready = false`, эмитится `CloseStreamRtc` | Ошибка `#stream-error` **не показывается** (это не фатальный сбой, §5.1 контракта). В `.stream-tile-status-overlay` и `#stream-viewer-status-banner` (`role="status"`, `aria-live="polite"`) отображается «Переподключение». Кнопка `#stream-ping-retry` видима **только у владельца** (в панели сцены) | При разрыве RTC с `willReconnect` или по клику `#stream-ping-retry` владелец отправляет `op 21 STREAM_PING` (`VoiceSession::ping_stream`). У зрителя `op 21` недоступен (`live_stream_key() == None` → `NoActiveStream`): он видит только статус и может выйти через `#stream-leave-watch`. При приходе нового `STREAM_CREATE`/`STREAM_UPDATE` возвращается в `Active`/`Paused`; владелец может остановить стрим через `#stream-stop` (`op 19`) | `[S]` `[F]` |
| **5b. `error / ended` (Отказ запуска, ошибка протокола или завершение стрима)** | `#voice-panel[data-stream="error" \| "off"]`<br>`#stream-stage[data-stream-phase="failed" \| "ended"]`<br>`#stream-error[role="alert"][data-reason="..."]` | Приоритет `A-REDUCER-DELETE`:<br>- `reason == "unauthorized"` → `Failed` (`data-reason="permission-channel"`);<br>- `reason == "safety_guild_rate_limited"` → `Ended` + `ShowSafetyRateLimit` (`data-reason="safety-rate-limited"`);<br>- `reason == "stream_full"` → `Ended` + `NotifyViewerFull` (`data-reason="viewer-full"`);<br>- локальный отказ `StreamError` → `data-reason="start-failed" \| "session-not-established" \| "invalid-command" \| "invalid-stream-key" \| "invalid-region" \| "invalid-paused"` (полный маппинг — §2.4.2) | Регион `#stream-error` виден (`hidden` снят, `role="alert"`, `aria-live="assertive"`), содержит локализованный текст причины (§2.10 `STREAM.md`). Захват экрана в нативном слое гарантированно освобождён (`SG-07`). Автоматический рестарт стрима запрещён (`SG-08`) | Кнопка `#stream-error-retry` («Повторить», видима для `start-failed` / `encoder-failed`) запускает новый цикл выбора источника по явному клику; `#stream-error-dismiss` («Закрыть») очищает ошибку и переводит в `off` | `[S]` `[D]` `[F]` |
| **6. `disabled` (нет права `STREAM` `1 << 9` или голосовая сессия не `Connected`)** | `#voice-panel[data-capability-stream="false"][data-stream="off"]`<br>`#stream-stage[data-capability-stream="false"]` | Backend вычисляет право `STREAM` (`1 << 9 = 512` / `0x200` в битовой маске разрешений голосового канала) плюс состояние голосовой сессии (`Connected` / `Established`) | Кнопка `#voice-screen` имеет `aria-disabled="true"` и `aria-pressed="false"`. Состояния `picking` и `preview` **недостижимы**: клик/`Enter`/`Space` не вызывают `stream.pickSource()` (`0` вызовов) и не отправляют `op 18`; вместо этого в `#stream-error` (`role="alert"`) выставляется `data-reason="permission-channel"` с текстом `/x4knx` («Нет разрешения на показ экрана») и пояснением `PpfzUE`. Если право отозвано, пока открыт `#stream-setup`, диалог остаётся открытым, но `#stream-start` получает `aria-disabled="true"`, а повторный клик не отправляет `op 18`; закрытие — `#stream-cancel` / `Escape` → `off` | Просмотр чужого стрима (`op 20 STREAM_WATCH`) в канале, где есть `VIEW_CHANNEL` (`1 << 10`) и `CONNECT` (`1 << 20`), но нет `STREAM` (`1 << 9`), **остаётся разрешённым**; блокируется только собственный старт Go Live (`op 18`). Если право отзывают во время своего стрима, сервер присылает `STREAM_DELETE{reason:"unauthorized"}` → `Failed` + `data-reason="permission-channel"` (строка 5b) | `[D]` `[S]` `[A]` |

---

## 5. Клавиатурная навигация и WAI-ARIA атрибуты

### 5.1. Полная таблица WAI-ARIA ролей, свойств и состояний

| Элемент (`id` / селектор) | HTML-тег и `role` | Доступное имя (`aria-label` / `aria-labelledby`) | Обязательные динамические ARIA-атрибуты | Метка |
|---|---|---|---|---|
| `#voice-screen` | `<button type="button">` | `"Продемонстрируйте свой экран"` (`fjBNo1`) | `aria-pressed="true\|false"`, `aria-haspopup="dialog"`, `aria-busy="true"` (в `picking`/`starting`/`stopping`), `aria-disabled="true"` (при `data-capability-stream="false"` или вне `connected`) | `[D]` `[A]` |
| `#voice-camera` | `<button type="button">` | `"Включить камеру"` (`v8K+8W`) / `"Отключить камеру"` (`EnX2Jl`) / `"Камера недоступна"` (`8jSzSe`) | `aria-pressed="true\|false"`, `aria-disabled="true"` при отсутствии видеоустройства или `data-capability-video="false"` | `[D]` `[A]` |
| `#voice-stream-live-pill` | `<span>`, `role="status"` | Текст внутри: `"Вы стримите!"` (`XvBdeT`) / `"Стрим приостановлен"` (`5q17w5`) | `aria-live="polite"`, `hidden` вне `live`/`paused`; точка `.stream-live-dot` имеет `aria-hidden="true"` (индикация не только цветом, `SA-02`) | `[D]` `[A]` |
| `#stream-setup` | `<section>`, `role="dialog"` | `aria-labelledby="stream-setup-title"`, `aria-describedby="stream-setup-hint"` | `aria-modal="true"`, `aria-busy="true"` (в `starting`), `hidden` вне `preview`/`starting` | `[D]` `[A]` |
| `#stream-preview` | `<div>`, `role="img"` | `aria-label="Предпросмотр того, что увидят зрители"` | `data-empty="true"` (в MVP без кадров `F-01`) | `[A]` |
| `#stream-quality` | `<button type="button">` | Текст `"Качество передачи: Авто \| Исходное"` (`Jbg8oY`) | `aria-haspopup="menu"`, `aria-expanded="true\|false"`, `aria-controls="stream-quality-menu"` | `[D]` `[A]` |
| `#stream-quality-menu` | `<div>`, `role="menu"` | `aria-label="Качество передачи"` (`SMYauD`) | Пункты `.stream-quality-option`: `role="menuitemradio"`, `aria-checked="true\|false"`, `tabindex="0\|-1"` (`roving tabindex`) | `[D]` `[A]` |
| `#stream-stage` | `<section>`, `role="region"` | `aria-label="Демонстрация экрана"` | `data-view="hidden\|stage"`, `hidden` при `data-view="hidden"`; оверлей `position: absolute; inset: 0` внутри `main.content` | `[A]` |
| `#stream-stage-bar` | `<header>`, `role="toolbar"` | `aria-label="Панель активной трансляции"` | Содержит `#stream-live-badge`, `#stream-timer[aria-hidden="true"]`, `#stream-viewers`, `#stream-pause-toggle`, `#stream-stop` | `[A]` |
| `#stream-viewers` | `<span>`, `role="status"` | Текст `"Стримят: {count}"` / `"Зрителей: {count}"` | `aria-live="polite"`, `hidden` при `viewer_count === null` (`D-27-5`) | `[D]` `[S]` `[A]` |
| `#stream-pause-toggle` | `<button type="button">` | `"Приостановить стрим"` (в `Active`) / `"Возобновить стрим"` (в `Paused`) | `aria-pressed="true\|false"` (обновляется только по `STREAM_CREATE`/`STREAM_UPDATE`), `aria-busy="true"` во время ожидания `op 22` | `[S]` `[A]` |
| `#stream-stop` | `<button type="button">` | `"Прекратить стрим"` (`S5anIc`) | `aria-busy="true"`, `aria-disabled="true"` после отправки `op 19` до прихода `STREAM_DELETE` | `[D]` `[S]` `[A]` |
| `#stream-ping-retry` | `<button type="button">` | `"Проверить соединение стрима"` | `hidden` вне `reconnecting`; виден только владельцу (в оверлее зрителя отсутствует); `aria-busy="true"` на время `op 21` | `[S]` `[A]` |
| `#stream-tiles` | `<div>`, `role="list"` | `aria-label="Активные трансляции в канале"` | Дочерние элементы `.stream-tile` имеют `role="listitem"` | `[A]` |
| `.stream-tile-card` | `<div>`, `role="button"` | `aria-label="{displayName} стримит, Смотреть стрим"` (`vTPX23` + `E5RDnK`) | `tabindex="0\|-1"` (`roving tabindex` по сетке плиток), `aria-pressed="true\|false"` (активен ли просмотр этой плитки) | `[D]` `[A]` |
| `.stream-tile-watch-btn` | `<button type="button">` | `aria-label="Смотреть стрим: {displayName}"` (`E5RDnK`) | `aria-busy="true"` при `data-phase="connecting"` | `[D]` `[A]` |
| `#stream-viewer-overlay` | `<section>`, `role="region"` | `aria-label="Просмотр трансляции"` | `hidden` при отсутствии открытого просмотра; содержит `#stream-viewer-canvas[role="img"]` и `#stream-viewer-controls[role="toolbar"]` | `[A]` |
| `#stream-fullscreen` | `<div>`, `role="dialog"` | `aria-label="Полноэкранный режим"` (`2nM3Pk`) | `aria-modal="true"`, `hidden` вне полноэкранного режима | `[D]` `[A]` |
| `#stream-loading` | `<div>`, `role="status"` | Текст `"Подключение..."` | `aria-live="polite"`, `aria-busy="true"` (когда видим) | `[A]` |
| `#stream-empty` | `<div>`, `role="status"` | Текст `"Никто не стримит"` | `aria-live="polite"`, `hidden` при наличии хотя бы одной активной плитки | `[A]` |
| `#stream-error` | `<div>`, `role="alert"` | Текст ошибки из `#stream-error-text` | `aria-live="assertive"`, `data-reason="<код>"`, **единственный** `role="alert"` в контуре стрима (`SA-04`) | `[D]` `[A]` |

### 5.2. Карта клавиатурной навигации, `roving tabindex` и управления фокусом

| Контекст / Компонент | Клавиша | Нормативное поведение и перемещение фокуса | Метка |
|---|---|---|---|
| **Кнопки в `#voice-panel` (`#voice-camera`, `#voice-screen`)** | `Tab` / `Shift+Tab` | Перемещение между контролами голосовой панели в порядке DOM: `#voice-ping` → `#voice-channel-link` → `#voice-join`/`#voice-leave`/`#voice-reconnect` → `#voice-camera` → `#voice-screen` → `#voice-mute` → `#voice-deafen` | `[A]` |
| **Кнопка `#voice-screen`** | `Enter` / `Space` | При `data-capability-stream="true"` и `data-state="connected"` вызывает `stream.pickSource()`. После выбора источника в ОС открывается `#stream-setup`, и фокус автоматически переносится на первичную кнопку `#stream-start`. При отмене пикера ОС фокус возвращается на `#voice-screen` (`SL-04`) | `[A]` `[F]` |
| **Диалог `#stream-setup` (`role="dialog"`, Focus Trap)** | `Tab` / `Shift+Tab` | Циклическая ловушка фокуса (`focus trap`) внутри `#stream-setup` в порядке DOM: `#stream-quality` → (если меню открыто) `#stream-quality-auto` → `#stream-quality-source` → `#stream-cancel` → `#stream-start` → `#stream-quality`. При закрытом меню пункты пропускаются. Фокус не уходит на фоновые элементы чата (`SA-03`) | `[A]` `[F]` |
| **Диалог `#stream-setup`** | `Escape` | В состоянии `preview` закрывает `#stream-setup` (`hidden`), освобождает источник пикера, переводит `data-stream="off"` и возвращает фокус на `#voice-screen` | `[A]` `[F]` |
| **Меню качества `#stream-quality` / `#stream-quality-menu`** | `Enter` / `Space` / `ArrowDown` на `#stream-quality` | Раскрывает `#stream-quality-menu` (`aria-expanded="true"`), переносит фокус на текущий выбранный пункт `[role="menuitemradio"][aria-checked="true"]` | `[A]` |
| **Внутри `#stream-quality-menu`** | `ArrowDown` / `ArrowUp` / `Home` / `End` | Перемещает фокус между опциями `Авто` (`data-quality="auto"`) и `Исходное` (`data-quality="source"`) по паттерну `roving tabindex` (`tabindex="0"` на активной опции, `-1` на остальных) | `[A]` |
| **Внутри `#stream-quality-menu`** | `Enter` / `Space` | Выбирает опцию (`aria-checked="true"`), вызывает `stream.setQuality(value)`, закрывает меню (`aria-expanded="false"`, `hidden`) и возвращает фокус на `#stream-quality` (`SL-15`) | `[A]` `[F]` |
| **Внутри `#stream-quality-menu`** | `Escape` | Закрывает меню качества **без закрытия** родительского диалога `#stream-setup` (`event.stopPropagation()`), возвращает фокус на `#stream-quality` | `[A]` |
| **Сетка плиток `#stream-tiles` (`.stream-tile-card`)** | `ArrowRight` / `ArrowDown` / `ArrowLeft` / `ArrowUp` / `Home` / `End` | Перемещение фокуса между карточками `.stream-tile-card` по паттерну `roving tabindex` (`ArrowRight`/`ArrowDown` — следующая плитка с циклическим переходом, `ArrowLeft`/`ArrowUp` — предыдущая, `Home` — первая, `End` — последняя) | `[A]` |
| **Карточка плитки `.stream-tile-card`** | `Enter` / `Space` | Если стрим ещё не просматривается — инициирует `op 20 STREAM_WATCH` и открывает `#stream-viewer-overlay`. Если уже просматривается — открывает `#stream-fullscreen` и переносит фокус на `#stream-fullscreen-close` | `[A]` |
| **Карточка плитки `.stream-tile-card` или `#stream-viewer-overlay`** | `F` / `f` (при отсутствии фокуса в `<input>` / `<textarea>`) | Переключает полноэкранный режим `#stream-fullscreen` для активного просматриваемого стрима | `[A]` |
| **Модальный `#stream-fullscreen` (`role="dialog"`, Focus Trap)** | `Tab` / `Shift+Tab` | Циклическая ловушка фокуса внутри `#stream-fullscreen-bar`: единственный контрол `#stream-fullscreen-close` (паузы в полном экране нет — `op 22` требует ключа владельца, а полный экран открыт для просмотра чужого стрима). Любое перемещение фокуса или движение мыши делает панель видимой (`data-controls-visible="true"`) и перезапускает таймер авто-скрытия `2500 мс` | `[A]` |
| **Модальный `#stream-fullscreen`** | `Escape` | Закрывает `#stream-fullscreen` (`hidden`), **не останавливая сам стрим** (`STREAM.md` §2.2 п.5, §4.8), и возвращает фокус на карточку `.stream-tile-card` (или на `#stream-fullscreen-toggle`), с которой был открыт полный экран (`ST-04`) | `[A]` `[F]` |
| **Сцена `#stream-stage` (`data-view="stage"`, стрим не активен)** | `Escape` | Закрывает оверлей сцены (`data-view="hidden"`), возвращает чат и фокус на `#voice-screen`. При `data-stream` = `live`/`paused`/`reconnecting`/`starting`/`stopping` `Escape` сцену не закрывает | `[A]` |
| **Панель `#stream-stage-bar` и `#stream-viewer-controls`** | `Enter` / `Space` на `#stream-pause-toggle`, `#stream-stop`, `#stream-ping-retry`, `#stream-leave-watch` | На `#stream-pause-toggle` → `op 22 STREAM_SET_PAUSED` (владелец); на `#stream-stop` → `op 19 STREAM_DELETE` (владелец); на `#stream-ping-retry` → `op 21 STREAM_PING` (владелец); на `#stream-leave-watch` → `LocalStreamAction::Close` без опкода, фокус возвращается на `#voice-screen` (или первую плитку в `#stream-tiles`) | `[S]` `[A]` |

---

## 6. Таблица Red-First оракулов тестирования по 9 измерениям (`TEST-POLICY.md` §13)

В соответствии с `TEST-POLICY.md` §13 каждый срез обязан покрывать все **9 обязательных измерений тестирования**. Все тесты пишутся в режиме **Strict TDD (Red-First)** против независимых оракулов (`waves/RW-40/VOICE/stream-opcodes-fixtures.json`, `waves/RW-40/DESIGN/stream-spec.json`, `rust/gui-spike/ui/tokens.css`, байтовых якорей `A-*` в `STREAM-GATEWAY-CONTRACT.md`).

| № измерения (`TEST-POLICY.md` §13) | ID оракула | Уровень (`§11`) / Приоритет (`§12`) | Целевой тест-файл (Rust / Node) | Сценарий и входное воздействие | Независимый оракул и точный проверяемый assert (Red → Green) |
|---|---|---|---|---|---|
| **1. Негативные сценарии (`Negative scenarios`)** | **T-STREAM-UI-NEG-01** | `L1 Unit` / `L3 Contract` / `L6 UI` (`P0`) | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `rust/crates/discord-voice/tests/session_contract.rs`, `tests/unit/voice-stream-ui.test.mjs` | 1) Передача всех 27 негативных векторов из `stream-opcodes-fixtures.json` (13 невалидных `stream_key`: пустой, `> 128` Б, префикс `dm:`, 3 сегмента для `guild`, 4 для `call`, ведущий ноль `0123`, `id == 0`, не-ASCII; `type: "GUILD"`; `guild` без `guild_id`; `call` с `guild_id`; `paused: "true"`; регион с пробелом / `> 64` Б);<br>2) Клик по `#stream-stop` / `#stream-pause-toggle` при отсутствии активного стрима (`NoActiveStream`) или повторный клик по `#voice-screen` в состоянии `picking`/`starting`/`stopping`. | 1) Кодировщик возвращает точный `StreamError::code()` (`stream.key.invalid`, `stream.command.kind`, `stream.command.guild_id.missing`, `stream.command.guild_id.unexpected`, `stream.field.paused`, `stream.field.region`), **0 кадров** отправлено в сокет, паники нет;<br>2) В `VoiceSession` возвращается `Err(SessionError::NoActiveStream)`; в UI повторный клик в `picking`/`starting`/`stopping` идемпотентен (`stream.pickSource` / `stream.start` / `stream.stop` вызван ровно `1` раз, `SL-03`). |
| **2. Граничные значения (`Boundary values`)** | **T-STREAM-UI-BND-01** | `L1 Unit` / `L3 Contract` / `L6 UI` (`P0`) | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `tests/unit/voice-stream-ui.test.mjs` | 1) Границы Snowflake в `StreamKey`: `1` (валидно), `u64::MAX` (`18446744073709551615`, 20 знаков — валидно), `0` и `u64::MAX + 1` (`18446744073709551616` — `NotSnowflake` / `Overflow`);<br>2) Длина `stream_key`: `68` Б (макс. канонический `guild`) и `128` Б (`MAX_KEY_BYTES`) vs `129` Б (`TooLong { len: 129 }`);<br>3) Длина `preferred_region`: `1` Б и `64` Б (`MAX_REGION_BYTES`) ASCII vs `0` Б и `65` Б (`InvalidRegion`);<br>4) Границы `viewer_ids` (`D-27-5`): `None` (поля нет) vs `Some([])` (`0` зрителей) vs `Some([1, 2, 3])`;<br>5) Таймеры UI: `14_999 мс` vs `15_000 мс` (таймаут старта `op 18`), `9_999 мс` vs `10_000 мс` (таймаут стопа `op 19`), `2_499 мс` vs `2_500 мс` (авто-скрытие `#stream-fullscreen-bar`). | 1) `StreamKey::parse(s)?.to_wire() == s` для `1` и `u64::MAX`; `0` и `u64::MAX + 1` отклоняются;<br>2) При `viewer_ids: None` элемент `#stream-viewers` имеет атрибут `hidden`; при `viewer_ids: Some([])` атрибут `hidden` снят и текст содержит `"0"` (`D-27-5`);<br>3) На виртуальных часах при `14_999 мс` состояние остаётся `starting`, ровно на `15_000 мс` переходит в `error` с `data-reason="start-failed"` (`SL-09`). |
| **3. Состояния ошибок (`Error states`)** | **T-STREAM-UI-ERR-01** | `L3 Contract` / `L6 UI` / `L8 A11y` (`P0`) | `rust/crates/discord-voice/tests/session_contract.rs`, `tests/unit/voice-stream-ui.test.mjs` | 1) Входящий `op 0 STREAM_DELETE` с протокольными причинами `A-REASONS`: `"unauthorized"`, `"safety_guild_rate_limited"`, `"stream_full"`, `"user_requested"` после `Failed`, неизвестный `"future_server_reason_2027"`;<br>2) Вызов `op 18..=22` до установления сессии (`GatewaySessionState::Connecting` / `Reconnecting` / `Disconnected`);<br>3) Отказ ОС / портала (`permission-os`, `portal-unavailable`, `encoder-failed`). | 1) Строгий приоритет `A-REDUCER-DELETE`: `unauthorized` → `StreamPhase::Failed` + `#stream-error[data-reason="permission-channel"]` (`/x4knx`); `safety_guild_rate_limited` → `Ended` + `StreamEffect::ShowSafetyRateLimit` + `data-reason="safety-rate-limited"`; `stream_full` → `Ended` + `StreamEffect::NotifyViewerFull` + `data-reason="viewer-full"` (`VVZDBL`); неизвестный `reason` → `StreamDeleteReason::Unknown`, `Ended`, `diagnostics.unknown_reasons.len() == 1`, без `role="alert"`;<br>2) Вне `Established` возвращается `stream.session.not_established`, и только адаптер переводит его в `#stream-error[data-reason="session-not-established"]` (§2.4.2; Rust-код в DOM не попадает);<br>3) В DOM ровно один активный `role="alert"` (`#stream-error`), захват освобождён (`0` активных потоков, `SG-07`), авто-рестарта нет (`SG-08`). |
| **4. Пустые состояния (`Empty states`)** | **T-STREAM-UI-EMP-01** | `L6 UI` (`P1`) | `tests/unit/voice-stream-ui.test.mjs` | 1) Открытие сцены `#stream-stage` при `StreamSnapshot { streams: [] }` (0 активных стримов в канале);<br>2) Завершение последнего активного стрима в канале событием `STREAM_DELETE{reason: "user_requested"}` → `LocalStreamAction::Close`;<br>3) Рендер `#stream-preview`, `.stream-tile-surface` и `#stream-viewer-canvas` в MVP без видеокадров (`F-01` Вариант B). | 1) При 0 стримов `#stream-empty` виден (`hidden === null`), имеет `role="status"`, текст `#stream-empty-title` равен `"Никто не стримит"`, `#stream-tiles` пуст (`children.length === 0`), `#stream-viewer-overlay` скрыт (`hidden !== null`, `ST-02`);<br>2) `#stream-preview`, `.stream-tile-surface` и `#stream-viewer-canvas` имеют `data-empty="true"`, текст плейсхолдера `"Предпросмотр недоступен"` (`SL-06`). |
| **5. Загрузочные и переходные состояния (`Loading / Connecting`)** | **T-STREAM-UI-LOAD-01** | `L3 Contract` / `L6 UI` / `L8 A11y` (`P0`) | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `tests/unit/voice-stream-ui.test.mjs` | 1) Клик `#voice-screen` → `picking` → выбор источника → клик `#stream-start` (`starting`, отправлен `op 18`);<br>2) Клик `.stream-tile-watch-btn` (`WatchRequested`, отправлен `op 20 STREAM_WATCH`);<br>3) Приход `op 0 STREAM_SERVER_UPDATE` **до** `STREAM_CREATE` и **после** `STREAM_CREATE` (`H-SERVER-UPDATE`);<br>4) Клик `#stream-stop` (`stopping`, отправлен `op 19`) до прихода `STREAM_DELETE`. | 1) В `picking`, `starting`, `stopping` кнопка `#voice-screen` (и соответствующий контрол) имеет `aria-busy="true"`, индикатор `#stream-live-badge` остаётся `hidden` в `picking`/`starting` (`SL-08`);<br>2) После `WatchRequested` фаза равна `StreamPhase::Connecting` (`"CONNECTING"`), `#stream-loading` виден с `aria-busy="true"` и `.stream-skeleton-tile`;<br>3) `STREAM_SERVER_UPDATE` выставляет `transport_ready = true` и эмитит `OpenStreamRtc`, но **не переводит** фазу `Connecting` в `Active` (`T-STREAM-13`);<br>4) После `stop_screen_share()` до `STREAM_DELETE` фаза и ключ сохраняются (`P1-2`, `t_v01_18`). |
| **6. Права доступа и Capabilities (`STREAM` permission)** | **T-STREAM-UI-CAP-01** | `L6 UI` / `L8 A11y` / `L10 Security` (`P0`) | `tests/unit/voice-stream-ui.test.mjs` | 1) Канал с битовой маской разрешений без бита `STREAM` (`(permissions & (1 << 9)) === 0`, `data-capability-stream="false"`), но с битами `VIEW_CHANNEL` (`1 << 10`) и `CONNECT` (`1 << 20`);<br>2) Попытка клика и нажатия `Enter`/`Space` по `#voice-screen` при `data-capability-stream="false"`;<br>3) Клик по `.stream-tile-watch-btn` чужого стрима при `data-capability-stream="false"`;<br>4) Отсутствие видеоустройств (`data-capability-video="false"`). | 1) При `data-capability-stream="false"` кнопка `#voice-screen` имеет `aria-disabled="true"` и `aria-pressed="false"`;<br>2) Клик по `#voice-screen` при `data-capability-stream="false"` делает **0 вызовов** `stream.pickSource()` и **0 отправок** `op 18` (`SG-12`), выставляя `#stream-error[data-reason="permission-channel"]` с текстом `/x4knx` («Нет разрешения на показ экрана»);<br>3) Просмотр чужого стрима (`.stream-tile-watch-btn` → `op 20 STREAM_WATCH`) при `data-capability-stream="false"` выполняется штатно;<br>4) При `data-capability-video="false"` кнопка `#voice-camera` имеет `aria-disabled="true"` (`SC-02`). |
| **7. Восстановление (`Recovery`)** | **T-STREAM-UI-REC-01** | `L2 Integration` / `L3 Contract` / `L6 UI` (`P0`) | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `rust/crates/discord-voice/tests/session_contract.rs`, `tests/unit/voice-stream-ui.test.mjs` | 1) Активный стрим получает `op 0 STREAM_DELETE{unavailable: true, reason: "stream_full"}` (проверка приоритета `unavailable` над `reason`);<br>2) Во время `Reconnecting` RTC-соединение стрима сигнализирует disconnect с `willReconnect` → отправка `op 21 STREAM_PING`;<br>3) Приход повторного `STREAM_CREATE` / `STREAM_UPDATE{paused: false}` + нового `STREAM_SERVER_UPDATE` для того же `stream_key`;<br>4) Новый `READY` (`StreamAction::Ready`) или терминальное закрытие голосовой сессии (`LeaveRequested`, `WsClosed(1000)`). | 1) При `unavailable: true` (даже если передан `reason`) фаза переходит строго в `StreamPhase::Reconnecting` (`"RECONNECTING"`), запись в `StreamState` и `active_stream_key` в `VoiceSession` **не удаляются** (`T-STREAM-09`, мутация M5 в `task-117`); UI показывает статус «Переподключение» без `role="alert"`;<br>2) `ping_stream()` формирует валидный `op 21 STREAM_PING` с тем же `stream_key`;<br>3) Повторный `STREAM_CREATE` переводит фазу обратно в `Active` и восстанавливает `#stream-live-badge`; в слое сессии при `stream_transport_ready == true` он дополнительно эмитит `StreamSessionEffect::CloseStreamRtc` (`session.rs:1488-1496`), в редьюсере `StreamState` эффекта нет — оракул проверяет слои раздельно;<br>4) При `StreamAction::Ready` `CloseStreamRtc` эмитится только для потоков с `transport_ready == true` (`stream.rs:1444-1455`); поток в `Active` без `STREAM_SERVER_UPDATE` эффекта не даёт. `clear_stream_record()` при закрытии сессии обнуляет запись (`active_stream_key.is_some() == stream_phase.is_some()`, `t_v01_20`). |
| **8. Совместимость (`Compatibility`)** | **T-STREAM-UI-CMP-01** | `L3 Contract` / `L7 Visual` / `L8 A11y` (`P1`) | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `tests/unit/voice-stream-ui.test.mjs`, `tests/unit/design-breakpoints.test.mjs` | 1) Forward-compatibility парсера `parse_dispatch`: входящие `STREAM_CREATE`/`UPDATE`/`DELETE` с неизвестными будущими полями в `d` (например, `" future_codec_hint": "av1_hdr"`), неизвестным `STREAM_DELETE.reason` и неизвестным событием `"STREAM_FOO"`;<br>2) Stale-последовательности `seq <= last_seq` (`§4.5`);<br>3) Адаптивная вёрстка сцены и панели на брейкпоинтах окна `≥ 1600px` (`tree 311px`), `1200–1599px` (`tree 240px`), `960–1199px`, `720–959px` (`tree 200px`), `< 720px` и при `prefers-reduced-motion: reduce`;<br>4) Работа в `ShimDocument` (без браузерного layout-движка) и под `Xvfb`. | 1) Неизвестные поля в `d` не ломают разбор и записываются в `diagnostics.ignored_fields`; `"STREAM_FOO"` возвращает `None`;<br>2) Повторный диспатч с `seq <= last_seq` игнорируется и добавляет `seq` в `diagnostics.stale_sequences` (`T-STREAM-16`);<br>3) У кнопки `.voice-action` сохраняется компактная геометрия `F-04`: `height: 28px` (`--btn-height-sm`), `padding: 0 8px` (`0 var(--space-2)`), `border-radius: 4px` (`--radius-xs`); у контейнера `.voice-actions` — только `display: flex; flex-wrap: wrap; gap: 2px` (`voice.css:108-119`, `voice.css:245-248`); в `@media (prefers-reduced-motion: reduce)` длительности переходов равны `0ms` (`transition: none`);<br>4) 100% переменных CSS объявлены в `rust/gui-spike/ui/tokens.css`, все контрасты текста $\ge 4.5:1$ и non-text $\ge 3.0:1$. |
| **9. Наблюдаемость и безопасность (`Observability & Security`)** | **T-STREAM-UI-OBS-01** | `L3 Contract` / `L10 Security` (`P0`) | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `rust/crates/discord-voice/tests/session_contract.rs`, `tests/unit/voice-stream-ui.test.mjs` | 1) Форматирование `format!("{:?}", ...)` и `format!("{}", ...)` для `StreamKey`, `StreamSecret`, `StreamServerUpdate`, `StreamEffect`, `StreamSnapshot`, `SessionSnapshot`;<br>2) Сериализация всего DOM-дерева (`#voice-panel`, `#stream-setup`, `#stream-stage`, `#stream-tiles`, `#stream-viewer-overlay`, `#stream-fullscreen`) при активном стриме с синтетическим ключом `guild:111:222:333`, `token: "secret-rtc-token-xyz"` и `endpoint: "rtc.discord.media:443"`;<br>3) Статический аудит кода `rust/gui-spike/ui/**` на запрещённые медиа/записывающие API (`SG-01..SG-06`) и счётчики `StreamDiagnostics` / `unknown_stream_event_count`. | 1) Вывод `Debug`/`Display` и `StreamSnapshot` **не содержит** подстрок `"secret-rtc-token-xyz"`, `"rtc.discord.media"`, `"guild:111:222:333"` и `"111"` (`guild_id` замаскирован как `<redacted>`, `token` и `endpoint` замаскированы как `<redacted>`, `T-STREAM-07`, `D-27-4`, `D-27-6`);<br>2) Сериализованный DOM и `aria-label`/`title` **не содержат** полной строки `stream_key`, `token` или `endpoint` (`SG-06`);<br>3) Grep по `rust/gui-spike/ui/**` даёт `0` вхождений `navigator.mediaDevices`, `getUserMedia`, `getDisplayMedia`, `saveClipEx`, `setClipsRecordingEnabled`, `startLocalAudioRecording`, `startRecordingRawSamples`, `setAecDump`, `saveScreenshot`, `getScreenPreviews`, `getWindowPreviews`;<br>4) Чужие/битые диспатчи инкрементируют `diagnostics.invalid_dispatches`, `diagnostics.unknown_streams`, `diagnostics.orphan_server_updates` и `Session::unknown_stream_event_count()` (`P2-1`). |

---

## 7. Обязательные мутационные проверки для приёмки (`TEST-POLICY.md` §3 п.6, §14: порог `5/5 KILLED`)

При реализации UI-модуля стриминга экрана (`G2`) и интеграции с `discord-core` / `discord-voice` набор тестов обязан убивать не менее 5 мутантов (`5/5 KILLED`):

| ID мутанта | Целевой узел / Инвариант | Описание мутации | Обязанный упасть тест (Оракул) |
|---|---|---|---|
| **M-STREAM-UI-01** | Запрет оптимистичного `LIVE` (`D-27-3`, `SL-08`) | В обработчике `#stream-start` сразу выставить `data-stream="live"` и снять `hidden` с `#stream-live-badge` до прихода `STREAM_CREATE` | `T-STREAM-UI-LOAD-01` (`SL-08`) |
| **M-STREAM-UI-02** | Запрет оптимистичного `STOP` (`P1-2`, `SL-10`) | В обработчике `#stream-stop` сразу перевести `data-stream="off"` и скрыть `#stream-stage` до прихода `STREAM_DELETE` | `T-STREAM-UI-LOAD-01` (`SL-10`, `t_v01_18`) |
| **M-STREAM-UI-03** | Приоритет `unavailable: true` в `STREAM_DELETE` (`A-REDUCER-DELETE`) | Пропустить ветку `if unavailable` (переводить `STREAM_DELETE{unavailable: true}` в `Ended`/`off` вместо `Reconnecting`) | `T-STREAM-UI-REC-01` (`T-STREAM-09`, `t_v01_19`) |
| **M-STREAM-UI-04** | Проверка права `STREAM` (`1 << 9`, `SG-12`) | Разрешить вызов `stream.pickSource()` / отправку `op 18` при `data-capability-stream="false"` | `T-STREAM-UI-CAP-01` (`SG-12`) |
| **M-STREAM-UI-05** | Различие `viewer_ids: None` vs `Some([])` (`D-27-5`) | Нормализовать `viewer_count === 0` в `hidden` (или `viewer_count === null` в показ `"0"`) на `#stream-viewers` | `T-STREAM-UI-BND-01` (`T-STREAM-08`, `SL-13`, `SL-14`) |
| **M-STREAM-UI-06** | Защита секретов и `stream_key` в DOM (`SG-06`) | Записать каноническую строку `stream_key` или `token` в `data-stream-key` / `title` плитки `.stream-tile` | `T-STREAM-UI-OBS-01` (`SG-06`, `T-STREAM-07`) |

---

## 8. Механическая проверка нормативного CSS и токенов (воспроизводимые команды)

```sh
# 1) Извлечение нормативного CSS-блока из спецификации
awk '/^```css$/{f=1;next} /^```$/{if(f){f=0}} f' \
  waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md > /tmp/voice-stream-ui-spec.css

# 2) Проверка: 0 сырых цветов (#hex, rgb/rgba, hsl/hsla), 0 !important, 0 :has(), 0 fallback var(--x, y)
grep -nE '#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(|!important|:has\(|var\(--[a-zA-Z0-9_-]+,' /tmp/voice-stream-ui-spec.css

# 3) Сверка 100% используемых var(--*) с объявлениями в rust/gui-spike/ui/tokens.css
grep -oE 'var\(--[a-zA-Z0-9_-]+' /tmp/voice-stream-ui-spec.css | sed 's/var(//' | sort -u > /tmp/vs_used.txt
grep -oE '^\s*--[a-zA-Z0-9_-]+\s*:' rust/gui-spike/ui/tokens.css | sed 's/^ *//;s/ *:$//' | sort -u > /tmp/vs_declared.txt
comm -23 /tmp/vs_used.txt /tmp/vs_declared.txt
```
