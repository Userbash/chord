# STUB-IMPLEMENTATION-PLAN — замены заглушек, изменения, ответственные

Дата: 2026-10-24. PM. Основание: `STUB-MAP.md` (35 карточек), `PARITY-MATRIX.md`,
`NETWORK-CONTRACTS.md`, `QUALITY/STRATEGY.md`.

## 0. Принципы

1. Заглушка не становится функцией без `[L]/[S]/[D]`; `[M]/[A]` — дизайн/RE-задача.
2. Сначала тест/contract, потом код; UI-заглушка удаляется только вместе с обработчиком.
3. Нельзя менять тест под код; e2e skip и source-scrape не считаются доказательством.
4. ≤2 кодовых воркера; один `cargo`; parity gate G6 обязателен.
5. Live/секреты — только по окну PM; чужие данные не захватывать.

## 1. Порядок выполнения (Now / Next / Later)

### Phase 0 — доверие и live-база (P0/P1)

| ID | Задача | Owner | Зависит | Exit criteria |
|---|---|---|---|---|
| P0-01 | live READY | PM+G4+G1 | токен+окно | READY+guild/channel, token не в логе |
| P0-02 | live send | PM+G4+G1 | P0-01 | сообщение в канале |
| STUB-31/32 | CLI `walk`, `cdp eval/clicktext/list/shot` | G4 | — | fixture-тесты без live зелёные |
| STUB-33/34 | e2e fixture + gate/workspace | G4 | 31/32 | e2e не skip; workspace в gate |
| STUB-35 | поведенческий token test | G1+G4 | — | keyring→Network проверен |

### Phase 1 — серверы, каналы, базовый чат (P1)

| ID | Задача | Owner | Зависит | Exit criteria |
|---|---|---|---|---|
| STUB-01/24 | READY → rail | G2+G1 | P0-01 | список/выбор/restore |
| STUB-04 | enter guild UI | G2 | G-03a backend | 204 → канал/список |
| STUB-06 | leave UI | G1+G2 | G-04 backend | confirm → 2xx → список |
| STUB-05 | invite join | G1+G2 | live-коды `[A]` | ошибки 4xx видимы |
| STUB-02 | guild menu | G2+G3 | меню-спека | 10 пунктов, права, клавиатура |
| STUB-08/09 | tree + channel switch | G2+G1 | READY/C-02 | выбор → история |
| STUB-14 | search filters | G2+G1 | контракт | 3 серверных + 6 local честно |
| STUB-21/22 | read state, typing/presence | G1+G2 | L-02 | unread/«печатает…» |
| DP-02 | 26/48/+22, member 42 | G2+G4 | D-04 | высоты/виртуализация |

### Phase 2 — полнота текста и уведомления (P2)

| ID | Задача | Owner | Зависит | Exit criteria |
|---|---|---|---|---|
| STUB-10 | pins/threads | G2+G1 | M-10/11 | панели/empty/error/jump |
| STUB-13 | attachments | G1+G2 | upload RE | upload/прогресс/превью |
| STUB-16 | profile popover | G2 | MB-03 | карточка/роли/действия |
| STUB-28 | notifications/DND | G5+G2 | N-01/02 | notify, mute, click→channel |
| STUB-19 | settings sections | G2+G3 | ST-01…08 | контент/disabled-причины |
| STUB-29/30 | settings sync/i18n | G1+G2 | ST-04/08 | persist, ru/en |
| STUB-11 | inbox | G2+G1 | N-03 | список/счётчики/jump |

### Phase 3 — голос (P1 release-critical, но XL)

| ID | Задача | Owner | Зависит | Exit criteria |
|---|---|---|---|---|
| STUB-26/V-01…04 | RTC session, UDP, SRTP | G1+G4b | live voice окно | join/leave, ключи не в логе |
| V-05 | Opus/DTX/CN | G5+G1 | V-04 | loopback 48 кГц |
| STUB-17/18 | devices/PTT/voice UI | G5+G2+G3 | V-05 | mute/deafen/PTT/ping |
| V-06 | stats/reconnect | G1 | V-05 | RTT/потери, rejoin |
| V-10 | DAVE/MLS | G1+G4 | V-05 | 2-client interop |

### Phase 4 — видео/Go Live (P2)

`S-01…S-04`: camera → screen capture → AV1/H264 → RTP/simulcast → Go Live → viewer.
Owner G5+G1+G2+G3; только после voice MVP.

### Phase 5 — платформа (P3)

`PL-01…PL-08`: tray, autostart, badge, deep links, updater, consent; owner G5.
`A-04/A-05`: QR/passkey; owner G1/G5, после текстового MVP.

## 2. Карта изменений

| Домен | UI / docs | Rust / core | Контракты/тесты |
|---|---|---|---|
| Servers | `actions.js`, `app.js`, `SERVER-MENU.md` | `guilds.rs`, `main.rs`, `core/rest` | `guilds_contract.rs` |
| Channels | `chat.js`, `tree.css`, `DOM-CONTRACT` | `data.rs`, reducers | `channel_switch.test.mjs` |
| Messages | `chat.js`, `message-toolbar.js`, `rows.js` | `messages.rs`, `history` | `messages_contract.rs` |
| Pins/threads | new `pins.js`/`threads.js`, `chat.js` | new `pins_threads.rs` | new `pins_threads_contract.rs` |
| Members/presence | `members.js`, `actions.js` | `presence_reducer.rs`, `member_list.rs` | `presence_*`, `member_*` |
| Attachments | `composer.js`, `chat.js`, `embeds.js` | new `attachments.rs`, core routes | `attachments_contract.rs` |
| Notifications | `actions.js`, settings UI | `discord-platform/notifications` | `notifications_contract.rs` |
| Settings/i18n | `settings.js`, `tokens.css` | `settings_store.rs`, `discord-i18n` | `settings_contract`, `i18n_ui` |
| Voice | new `voice.js`, voice CSS | `discord-voice`, `gui-spike/src/voice*` | `voice_session_contract` |
| Video | new `stream.js`, capture UI | `discord-media`, `platform/capture` | `screen_capture_contract` |
| Test/CLI | `src/cli/run.mjs`, `README.md` | `src/core/cdp`, `walk-log` | e2e fixture, gate meta |

## 3. Ответственные (RACI)

| Роль | Ответственность |
|---|---|
| **G1 Core/Backend** | REST/ws/voice core, guild lifecycle, typing/presence backend |
| **G2 Frontend** | UI-заглушки → состояния, DOM/aria, интеграция IPC |
| **G3 Design** | SERVER-MENU/VOICE/STREAM, tokens/geometry, DP-02 |
| **G4 QA** | CLI fixture, mutation, gate/coverage, regression |
| **G4b Security** | secrets/logs, unsafe/FFI, voice/DAVE, consent |
| **G5 Platform** | devices/PTT, capture, notifications, tray |
| **G6 Parity** | evidence/priority gate, запрет `[A]`-функций |
| **PM** | WIP, окна, приёмка, tracker |

## 4. Приоритеты и срочность

| Класс | Срочность | Что |
|---|---|---|
| P0 | **Now** | live READY/send; утечка секретов |
| P1 | **Now/Next** | серверы/каналы, leave, e2e/gate, DP-02, voice MVP |
| P2 | Next/Later | pins/threads, attachments, notifications, settings |
| P3 | Later | QR/passkey, platform, gift |

Правило: P0 всегда выше; P3 не прыгает через P1 без письменного решения.
Внутри P1 — WSJF-lite `(BV+TC+RR)/JS`; `confidence<0.5` → RE-spike.

## 5. Критерии снятия заглушки

1. Обработчик/команда существует и вызывает реальный backend.
2. `[L]/[S]/[D]` источник и contract test; для live — E3.
3. UI: loading/empty/error/disabled, aria/focus, клавиатура.
4. Тест: red-first + regression; mutation для критичного.
5. G6 matrix обновлена; PM принял с evidence.
6. Заглушка удалена из `STUB-MAP`/tracker (не просто скрыта).

## 6. Отслеживание

- `STUB-TRACKER.md` — статусы и next action.
- Обновление: после каждого accepted batch (D-*), не реже одного раза за batch.
- Красный флаг: задача `in work` >2 batch без evidence → возврат/декомпозиция.
- 2 одинаковых отказа → смена подхода; 3-я попытка запрещена.
