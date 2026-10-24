# PARITY-MATRIX — эталон ↔ наше приложение

Дата: 2026-10-24. G6 Lead. Источник функций: `FUNCTION-INVENTORY.md`; приоритеты —
`GAP-BACKLOG.md`. Колонки единые для всех доменов.
Синхронизация после batch: `G6-SYNC-01.md` (строки G-01/C-02/G-03/G-03a/G-04/G-05/M-07/
P-03/L-01/M-06/ST-01/V-08, §14-счётчики). `implemented (live unverified)` — offline-паритет,
E3 нет: **не** приёмка PM и **не** `parity-ok`.

## Легенда

- **Ref**: `источник · level` (коды источников — в `FUNCTION-INVENTORY.md §0`; `B162` — web-бандл эталона).
- **level**: `[L]` live-захват · `[S]` call-site в коде эталона · `[D]` только Enum/путь · `[M]` измерение (DOM/скрин/каталог) · `[A]` нет подтверждения. Поле `confirmed` из `rest-static.json` не используется: оно `false` у всех 904 записей.
- **App**: `implemented` — сквозной путь в приложении + offline-тесты; `partial` — часть слоёв
  (crate/headless/UI-заглушка), функция не пригодна end-to-end; `absent` — кода нет;
  `unknown` — не оценено; `verified` — подтверждено live.
- **P**: P0 stop-the-line; P1 ежедневный паритет; P2 вторичный; P3 backlog.
- **WSJF**: `(BV+TC+RR)/JS` по уставу §5.2; `—` — не в очереди (implemented/verified/out-of-scope).
- **Conf**: уверенность в evidence строки (не в оценке WSJF).
- `live unverified` — функция реализована и покрыта offline-тестами, но E3 нет.

## 1. Auth / session

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| A-01 | auth | Вход по логину/паролю | `S162:172` · [S] · endpoint+body by call-site; live login [A] | implemented (live unverified) | нет live-логина 1.0.166; UX-ошибки не сверены | live-вход, токен только в keyring, 401/429/капча | `auth_contract.rs` + live N0-01 | G1/G4 | P0 | P0 | 0.9 |
| A-02 | auth | MFA TOTP/SMS/backup/password | `S162:174-175` · [S] | implemented (live unverified) | live-MFA не прогнан | все 5 методов, ошибки/таймаут, без кода в логах | `auth_contract.rs` | G1/G4 | P1 | — | 0.85 |
| A-03 | auth | Captcha-гейт | `S162:51,171` · [S] | implemented (offline) | live-captcha 1.0.166; sitekey/flow | интерактив без обхода; `X-Captcha-*`; отмена | `auth_contract.rs` | G1/G4 | P1 | — | 0.8 |
| A-04 | auth | QR / one-time login | `S162:176` · [S] · QR flow [A] | absent (UI disabled) | нет протокола/поллинга | QR-сессия на тестовом аккаунте | `auth_qr_contract.rs` (new) | G1 | P2 | 1.67 | 0.55 |
| A-05 | auth | Passkey / WebAuthn | `S162:166-167,177` · [S] · Linux WebAuthn [A] | absent (UI disabled) | нет Linux-WebAuthn | челлендж/подпись/ошибки | `auth_passkey_contract.rs` (new) | G1/G5 | P2 | 1.25 | 0.5 |
| A-06 | auth | Восстановление сессии | `S162:65-67` · [S] · safeStorage code | implemented (keyring [A], live unverified) | live-restore и битый keyring | restore → ready; logout → clear; идемпотентность | `auth_contract.rs` | G1 | P1 | — | 0.85 |
| A-07 | auth | Logout / инвалидация | `S162:101,173` · [S] | implemented (live unverified) | live-stop gateway, 401 после logout | stop → clear → `auth://state`; повтор OK | `auth_contract.rs`, `net_contract.rs` | G1 | P1 | — | 0.85 |
| A-08 | auth | Регистрация/сброс пароля | `S162:168,178-179` · [S] | absent | web-flow вне desktop scope | решение PM: not planned | — | PM | P3 | — | 0.6 |
| A-09 | auth | Сессии / logout others | `S162:181-182` · [S] | absent | нет экрана и вызова | список сессий, отзыв, 401 | `auth_sessions_contract.rs` (new) | G1 | P2 | 3.0 | 0.6 |

## 2. Guilds / servers

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| G-01 | guilds | Рейл: список/выбор/переключение | `UI6:01, UI:00-app.png` · [M] · visual; **источник списка `[L]` READY.guilds[]** (`002-READY.json`, 7 гильдий) | implemented (live unverified) | restore `lastGuildId` — G-01b (поле в `UiSettings`); live READY (P0-01); иконки/монограммы — G-11 | список из READY, выбор, пустое/ошибка/retry, клавиатура; restore — отдельная карточка G-01b | `tests/unit/guild-rail.test.mjs` (8) + READY-оракул | G2+G1 | P1 | — | 0.9 |
| G-02 | guilds | Добавить/создать сервер | `S162:1682 (GUILDS enum)` · [D] · call-site 0 | absent | endpoint `GET\|POST /guilds` не определён | RE-endpoint → модалка → create/join | `guilds_contract.rs` (new) | G1+G2 | P3 | 1.33 | 0.4 |
| G-03 | guilds | Вход по инвайту | `NC:1.1#1,1.2` · [S] · body; live codes/TTL [A] | implemented (live unverified) | live-коды/TTL/USER_BANNED-текст `[A]` (RE-2); screening не снят | resolve → preview → accept ровно `{code}`; `session_id`/`invite_instance_id`/`received_installation_id` nullable и не подставляются; 401/403/404/429 видны | `invites_contract.rs` (14) + `join-server.test.mjs` (24) | G1+G2 | P1 | — | 0.75 |
| G-03a | guilds | Войти в сервер из preview/lurker | `D02:101,138,225` · [L] · 204×2; requestId ready\|57992.6989, run2\|95974.7409 | implemented (live unverified) | live 204 из UI не снят; 403/429 `[A]`; плитка не добавляется (204 ≠ членство) | `PUT /guilds/{id}/members/@me?lurker=true` + `{}` → 204; кнопка только при `guildId` из resolve; 1 клик = 1 IPC; status «открыт для просмотра» | `guilds_contract.rs` (13+11) + `join-server.test.mjs` (+8) | G1+G2 | P1 | — | 0.9 |
| G-04 | guilds | Выход с сервера | `NC:1.1#4` · [S] · response/codes [A] | implemented (live unverified) | live-коды `[A]`: 403 owner, 404, 429 `retry_after` | confirm (`alertdialog`) → `{guildId, lurking}` → 2xx → плитка удалена; owner-пункт disabled | `guilds_contract.rs` + `gui-spike-features.test.mjs` (+10) | G1+G2 | P1 | — | 0.8 |
| G-05 | guilds | Меню сервера | `UI2:D1` · [M] · live screen measurement | partial: поповер/ARIA/клавиатура; **активны 2 из 10** | 8 пунктов — статичные заглушки; invite disabled до G-06/capability; нет прав из backend | 10 активных пунктов по правам, клавиатура, disabled-причины | `gui-spike-features.test.mjs` | G2+G3 | P1 | 5.0 | 0.9 |
| G-06 | guilds | Инвайты: создать/список/отозвать | `NC:1.1#8-11` · [S] · codes [A] | absent | нет API/UI; коды [A] | модалка, copy link, revoke, ошибки | `invites_contract.rs` (new) | G1+G2 | P2 | 3.5 | 0.55 |
| G-07 | guilds | Роли/каналы/права сервера | `S162:340,409,97` · [S] · UI [A] | absent | UI не снят; payload/коды [A] | out of RW-40 scope; design-first | `guild_admin_contract.rs` | G1+G2+G3 | P3 | 1.33 | 0.35 |
| G-08 | guilds | Unread/mention сервера | `UI6:01` · [M] | partial: статичный `is-unread` | нет счётчиков/read_state | бейджи каналов/рейла, сброс по ack | `read_state_ui.test.mjs` | G2+G1 | P2 | 3.0 | 0.7 |

## 3. Channels / navigation

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| C-01 | channels | Дерево: категории, text/voice/forum | `UI6:01` · [M] | partial: дерево из READY, типы 0/2/4/5/15 (C-02) | нет открытия voice/forum-представления, collapse, порядка категорий сверх READY | типы, категории, collapse, live-данные | `gui-spike-dom.test.mjs` | G2+G1 | P1 | 3.67 | 0.85 |
| C-02 | channels | Переключение канала + история | `D2:70,180` · [L] · history live; UI switch `[M]` | implemented (live unverified) | история — только первая страница (`hasMore/nextCursor` не догружаются) → M-01; G6-D-01/D-02 закрыты (D-120) | выбор → канал → история (`messages_page` реальным id); типы 0/5 открывают чат, 2/4/15 — нет; per-channel окно/scroll | `channel-switch.test.mjs` (10) + `MESSAGE_*`-оракул | G2+G1 | P1 | — | 0.9 |
| C-03 | channels | Unread/mention в канале | `UI6:01` · [M] | partial: статичный класс | нет mention_count/ack | маркер + счётчик + сброс | `read_state_ui.test.mjs` | G2+G1 | P2 | 3.0 | 0.7 |
| C-04 | channels | Шапка: пины/ветки/участники/поиск | `D2:109-110` · [L] · UI panels [M] | partial: поиск/участники есть, пины/ветки — stub | нет панелей pins/threads, inbox disabled | панели/пустые состояния/ошибки | `gui-spike-features.test.mjs` | G2 | P2 | 3.0 | 0.8 |
| C-05 | channels | Создание/редактирование канала | `S162:200,340` · [S] · UI [A] | absent | UI не снят | design-first, P3 | `guild_admin_contract.rs` | G2+G3 | P3 | 1.33 | 0.35 |
| C-07 | channels | QuickSwitcher `Ctrl+K` | `F162:118` · [S] · code registry; screen [M] | implemented (B6) | live-каналы не связаны | каналы/гильдии из live, ранжирование | `gui-spike-features.test.mjs` | G2 | P2 | — | 0.9 |

## 4. Messages

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| M-01 | messages | История/пагинация | `D2:70,180` · [L] | partial: fixtures + PageStore | нет live-истории/курсоров | REST-страницы, ≤500 окно, scroll | `messages_contract.rs`, `data_contract.rs` | G1+G2 | P1 | 3.67 | 0.85 |
| M-02 | messages | Отправка | `S162:223` · [S] · body [A] | implemented (live unverified) | тело/optimistic live не сверены | live send, 401/429, retry | `messages_contract.rs` + live N0-02 | G1+G2 | P0 | P0 | 0.9 |
| M-03 | messages | Редактирование | `S162:228` · [S] · body [A] | implemented (live unverified) | тело edit [A] | edit → обновление строки, 403 | `messages_contract.rs` | G1+G2 | P1 | — | 0.75 |
| M-04 | messages | Удаление | `S162:228` · [S] | implemented (live unverified) | live-удаление/403 | tombstone, confirm, ошибка | `messages_contract.rs` | G1+G2 | P1 | — | 0.8 |
| M-05 | messages | Ответ (reply) | `B162:message_reference` · [S] · call-site reply; live field [A] | implemented (live unverified) | reference/тело [A] | ответ → quote → live | `messages_contract.rs` | G1+G2 | P1 | — | 0.7 |
| M-06 | messages | Реакции | `D2:108` · [L] | implemented (live unverified) | live PUT/DELETE/429; `MESSAGE_REACTION_*` dispatch — L-02; bar-кнопка «add-reaction 👍» не снята `[A]` | pills/quick-реакции на реальном `channel:<channelId>:<item.id>`; optimistic + откат; bench не затронут | `chat-reactions.test.mjs` (9) | G1+G2 | P1 | — | 0.85 |
| M-07 | messages | Read state / ack | `D2:73,207` · [L] · `{token,last_viewed}`, 200×5, `last_viewed=4310` | implemented (live unverified) | unread без mentions и badge-счётчики не выводятся (G-08/C-03); live ack | ack на open/focus/scroll-to-bottom, дедуп + throttle; `last_viewed` — счётчик read_state, `token` nullable и **не** bearer; badge/`null` ≠ 0 | `rest_ack.rs` (9) + `read_state_contract.rs` (12) + `read-state-ui.test.mjs` (8) | G1+G2 | P1 | — | 0.85 |
| M-08 | messages | Markdown/embeds/mentions | `G162:433-438` · [L] · markdown rules [S] | partial: только embeds | нет markdown/mentions/кастом-emoji | рендер по `I162`, безопасные ссылки | `gui-spike-features.test.mjs` + i18n markdown | G2 | P1 | 3.0 | 0.75 |
| M-09 | messages | Контекстное меню (15) | `UI:01` · [M] | partial: 4 команды, остальные stub | нет forward/bookmark/reminder/TTS/report | пункты по правам, клавиатура | `gui-spike-features.test.mjs` | G2 | P2 | 2.67 | 0.85 |
| M-10 | messages | Пины | `D2:109` · [L] | absent (stub) | нет панели/загрузки | список, jump, ошибки | `pins_threads_contract.rs` | G1+G2 | P2 | 3.0 | 0.8 |
| M-11 | messages | Ветки | `D2:110` · [L] | absent (stub) | нет панели/создания | список/создание/архив | `pins_threads_contract.rs` | G1+G2 | P2 | 2.0 | 0.7 |
| M-12 | messages | Forward/bookmark/reminder/TTS/report | `UI:01` · [M] · endpoints [A] | absent | endpoint'ы не сняты | после RE; P3 | `message_actions_contract.rs` | G1+G2 | P3 | 1.33 | 0.4 |
| M-13 | messages | Поиск по серверу | `F162:1, 20f` · [L] · live network capture | implemented (4/7 фильтров, live unverified) | 6 фильтров/лимиты не подтверждены | live-поиск, пагинация, ошибки | `search_contract.rs` | G1+G2 | P1 | — | 0.85 |
| M-14 | messages | Эмодзи-пикер | `F162:2` · [M] | implemented (static) | нет кастомных эмодзи/стикеров | live-эмодзи, поиск, locked | `gui-spike-features.test.mjs` | G2 | P2 | — | 0.8 |

## 5. Members

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| MB-01 | members | Список: роли + статусы | `F162:55-62` · [M] | partial: 24k synthetic, 6 групп | нет live-данных/сортировки | live-члены, группы, виртуализация | `member_list_*.rs` + UI | G1+G2 | P1 | 4.5 | 0.8 |
| MB-02 | members | Роли/цвета/права | `G162:397` · [L] · roles in READY; UI [M] | partial: synthetic | нет live-ролей/цветов | роль-цвет, hoist, права | `presence_types.rs` + UI | G1+G2 | P2 | 3.0 | 0.7 |
| MB-03 | members | Профиль-поповер | `UI5:04` · [M] | partial: stub aria | нет поповера/полей | карточка, роли, действия | `profile_popout.test.mjs` | G2 | P2 | 3.0 | 0.6 |
| MB-04 | members | Полный поиск участников | `NC:1.5` · [S] · 202 retry; pagination [A] | absent (deferred) | нет пагинации/кодов (RE) | RE request/response → поиск | `members_search_contract.rs` | G1+G2 | P2 | 2.5 | 0.5 |

## 6. Presence / typing

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P-01 | presence | Статусы online/idle/dnd/offline/invisible | `G162:76,293` · [L] | partial: synthetic + core | статусы не из live | live-статус в списке/панели | `presence_reducer.rs` + UI | G1+G2 | P1 | 4.5 | 0.85 |
| P-02 | presence | Обновления PRESENCE_UPDATE | `G162:152,397` · [L] | partial: parse/reducer, UI не связан | нет применения к UI | reducer → UI, reconnect | `presence_reducer.rs` | G1+G2 | P1 | 4.5 | 0.8 |
| P-03 | presence | Индикатор набора + POST /typing | `S162:253, NC:1.6` · [S] · TYPING_START live [L]; TTL `[A]` | partial: backend implemented (D-108/D-114), UI отсутствует | нет `send_typing` из UI и индикатора «Печатает…»; `TYPING_RESEND_MS`/≤5 каналов не реализованы; TTL 10 с — `[A]` | `POST /channels/{id}/typing` без тела → 200; индикатор `aria-live="polite"`, троттлинг, guard ≤5, slowmode-сообщение | `typing_contract.rs` (13) + `[S]`-оракул; UI-тест — в карточке P-03-UI | G1+G2 | P1 | 4.5 | 0.8 |
| P-04 | presence | Кастомный статус/активность | `G162:250` · [L] | absent | нет редактора/отображения | статус+activity, 429 | `presence_activities_contract.rs` | G1+G2 | P2 | 3.0 | 0.6 |
| P-05 | presence | Смена своего статуса | `G162:293, UI3:B-panels` · [L] · invisible in READY | absent | нет меню статуса | меню, IDENTIFY/UPDATE, persist | `presence_update_contract.rs` | G1+G2 | P2 | 3.0 | 0.6 |

## 7. Voice

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| V-01 | voice | Вход/выход из voice-канала | `V162:23-52` · [L] | absent (crate only) | нет session manager/UI | join → op2/op4 → media; leave 1000 | `signaling.rs` + `voice_session_contract.rs` | G1 | P1 | 2.6 | 0.85 |
| V-02 | voice | RTC-сигнализация | `V162:23-52` · [L] | partial: парсер/тесты | нет live-WS/сессии | op0/1/2/4/8, heartbeat 13.75 с | `signaling.rs` | G1 | P1 | 3.25 | 0.8 |
| V-03 | voice | UDP discovery/keepalive/rotation | `V162:53-90` · [L] | partial: примитивы | нет live-UDP | discovery 74 Б, select, keepalive 5 с, +60 с | `udp.rs` | G1 | P1 | 3.25 | 0.8 |
| V-04 | voice | SRTP | `V162:65-90` · [L] · decrypt [A] | partial: KDF/AEAD тесты | нет live encrypt/decrypt | ключи op4, tag16/tail4, нет plaintext | `srtp_contract.rs`, `crypto_vectors.rs` | G1+G4b | P1 | 3.5 | 0.8 |
| V-05 | voice | Opus/джиттер/DTX | `V162:91-118` · [L] · jitter model [M] | partial: jitter, нет Opus | нет encode/decode | 48 кГц, PT120, DTX/CN, loopback | `media_contract.rs` + loopback | G5+G1 | P1 | 3.0 | 0.7 |
| V-06 | voice | Статистика/reconnect/регионы | `V162:119-123` · [M] · ping UI; reconnect code [S] | partial: policy/stats | нет live RTT/потерь | RTT/потери, reconnect, регион | `stats_contract.rs`, `reconnect_contract.rs` | G1 | P2 | 3.33 | 0.65 |
| V-07 | voice | Устройства/mute/deafen/PTT | `F162:63-83` · [M] · settings screen | absent (settings fake) | нет устройств/эффекта | выбор in/out, hot-plug, PTT, mute/deafen | `voice_devices_contract.rs` | G5 | P1 | 2.25 | 0.8 |
| V-08 | voice | Голосовой UI: панель/участники/ping | `VUI:01` · [M] | partial: панель на fake store (D-115) | нет реального voice-backend (V-01…V-05); `[M]` speaking-анимация — не паритет | join/leave, участники, speaking, ping; `data-state`, aria-pressed | `voice-ui.test.mjs` (44) | G2+G3 | P1 | 3.67 | 0.85 |
| V-09 | voice | Шумоподавление Krisp | `V162:91-118` · [L] · engine log | absent | нет модели/интеграции | RNNoise-совместимо, вкл/выкл | `krisp_contract.rs` | G5 | P2 | 2.33 | 0.6 |
| V-10 | voice | DAVE/MLS E2EE | `V162:91-118` · [L] · init; 2-client [A] | absent | нет MLS | framing/ключи/ротация/интероп | `dave_contract.rs` | G1+G4 | P2 | 2.2 | 0.6 |
| V-11 | voice | Voice messages/clips/soundboard | `C166@11203` · [D] · clips enum; soundboard live [L] | absent | контракт не снят | запись/клипы/soundboard | `clips_contract.rs` | G5+G2 | P3 | 2.0 | 0.35 |

## 8. Video / Go Live

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S-01 | video | Камера: preview/enable | `V162:29-32` · [L] · video capability; camera [A] | absent | нет захвата/превью | v4l2, preview, mic отдельно | `camera_contract.rs` | G5 | P2 | 1.75 | 0.6 |
| S-02 | video | Screen share / Go Live start-stop | `VUI:01, C166@23840` · [D] · capture IPC/UI; STREAM [A] | absent | нет capture/сигнализации | consent → выбор источника → поток → stop | `screen_capture_contract.rs` + `stream_contract.rs` | G5+G1 | P2 | 1.6 | 0.45 |
| S-03 | video | Зрители/качество/simulcast | `NC:3.3` · [A] · viewer flow; STREAM_* code is [S] | absent | нет RTP-видео/декода | rid/quality, зритель видит поток | `video_rtp_contract.rs` | G1+G5 | P2 | 1.4 | 0.4 |
| S-04 | video | Activities | `VUI:01` · [M] | absent | контракт запуска | запуск/выход/права | `activities_contract.rs` | G1+G2 | P3 | 1.33 | 0.4 |

## 9. Attachments

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| AT-01 | attachments | Загрузка файла (multipart/прогресс) | `S162:817` · [D] · path only; method/fields [A] | absent (кнопка disabled) | **blocked**: метод/поля/лимиты | RE-контракт → upload → send | `attachments_contract.rs` | G1+G2 | P2 | 3.0 | 0.45 |
| AT-02 | attachments | Drag&drop/paste | `CAT:IJyOUf` · [M] | absent | нет триггеров | drop/paste → upload | `attachments_contract.rs` | G2 | P2 | 4.0 | 0.45 |
| AT-03 | attachments | Рендер вложений | `G162:433-438` · [D] · key null; schema [A] | absent (только embeds) | нет схемы attachment | image/video/file/audio, CDN | `attachments_render.test.mjs` | G2 | P2 | 4.5 | 0.5 |

## 10. Settings

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| ST-01 | settings | Навигация настроек | `F162:63-70` · [M] · дамп `ui8/23-settings-categories.json` (`count:63`, 5 455 Б) | partial: 18 разделов, 3 панели; профиль/4 подпункта/«Выйти» есть (D-118) | 63 DOM-строки = профиль 2 + 18 разделов/«Выйти»×3 + 4 подпункта «Учётной записи»; подпункты 17 разделов свёрнуты → `[A]`; панели подпунктов — `[A]` | навигация 63, disabled-причины в `title`/`aria-describedby`, `aria-current` | `gui-spike-features.test.mjs` + фикстура-оракул `tests/fixtures/settings-nav-63.tsv` (new) | G2+G3 | P2 | 6.0 | 0.85 |
| ST-02 | settings | Голос/видео: 38 тумблеров/устройства | `F162:71-83` · [M] | partial: 38 toggles, нет эффекта | нет устройств/аудио | устройства, проверка, persist | `settings_contract.rs` | G5+G2 | P2 | 3.0 | 0.8 |
| ST-03 | settings | Внешний вид: тема/плотность/zoom | `CAT:K2sFfo, F162:145-158` · [S] · tokens/code | partial: dark/light/density | нет zoom/полного light-набора | темы, density, zoom, persist | `gui-spike-features.test.mjs` | G2 | P2 | 3.0 | 0.7 |
| ST-04 | settings | Язык/время + i18n-рантайм | `I162:14,205` · [S] · runtime; counts [M] | partial: crate 16k строк, не подключён | нет t()/каталогов в UI | ru/en, plural, rich, fallback | `i18n_ui.test.mjs` | G1+G2 | P2 | 2.0 | 0.75 |
| ST-05 | settings | Уведомления: настройки | `S162:680` · [S] · UI [M] | partial: панель-заглушка | нет полей/эффекта | mention levels, sounds, persist | `settings_contract.rs` | G2 | P2 | 4.5 | 0.7 |
| ST-06 | settings | Горячие клавиши | `F162:118` · [S] | absent | нет редактора | keybinds, конфликты, reset | `keybinds.test.mjs` | G2 | P3 | 2.0 | 0.5 |
| ST-07 | settings | Аккаунт/профиль | `F162:63-70` · [M] · fields [A] | absent | поля не сняты | имя/аватар/about, 429 | `profile_settings.test.mjs` | G1+G2 | P3 | 2.0 | 0.4 |
| ST-08 | settings | Персистентность/синхронизация | `D2:95,115` · [L] | partial: локальный файл [A] | нет settings-proto sync | local + server sync, миграции | `settings_contract.rs`, `settings_proto.rs` | G1 | P2 | 3.33 | 0.8 |

## 11. Notifications / DND

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| N-01 | notifications | Системные уведомления | `C166@63242, M162:892-900` · [S] · IPC+native chain | partial: D-Bus crate, не подключён | нет вызова из UI | notify/close/action, click→канал | `notifications_contract.rs` | G5+G2 | P2 | 3.0 | 0.75 |
| N-02 | notifications | DND/режим концентрации | `CAT:gH3Frd,gYEF6m` · [S] · DND string; suppression rules [A] | partial: статус-лейбл + idle policy [A] | нет подавления | DND подавляет notify, авто-idle | `dnd_contract.rs` | G5+G2 | P2 | 3.0 | 0.55 |
| N-03 | notifications | Inbox/упоминания | `CAT` · [M] · strings only | absent (кнопка disabled) | UI/счётчики не сняты | inbox, mention list, jump | `inbox.test.mjs` | G2+G1 | P2 | 3.0 | 0.4 |
| N-04 | notifications | Бейдж/прогресс | `C166@13624` · [S] | absent | нет Unity badge | badge count, clear | `badge_contract.rs` | G5 | P3 | 3.0 | 0.5 |
| N-05 | notifications | Переопределения сервер/канал | `UI2:D1` · [M] | absent | нет меню/уровней | per-guild/channel levels | `notification_overrides.test.mjs` | G2+G1 | P2 | 4.5 | 0.7 |

## 12. Platform

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| PL-01 | platform | Трей/фон | `C166@62339, M162:861-869` · [S] | absent | нет трея | иконка/меню/mute/deafen/close→tray | `tray_contract.rs` | G5 | P2 | 3.5 | 0.7 |
| PL-02 | platform | Автозапуск/старт свёрнутым | `A166@7201, C166@62475` · [S] | partial: модель `.desktop` | нет записи/UI | install/uninstall, start-minimized | `autostart_contract.rs` | G5 | P2 | 3.0 | 0.8 |
| PL-03 | platform | Deep links + single instance | `A166@23596` · [S] | partial: парсер `discord://` | нет wiring/second-instance | open-url → route, один инстанс | `deep_link_contract.rs` | G5 | P2 | 3.0 | 0.8 |
| PL-04 | platform | Апдейтер | `A166@7201, M162:579-591` · [S] | partial: manifest/plan | нет скачивания/замены | подпись, staged, rollback | `updater_contract.rs` | G5 | P3 | 1.75 | 0.65 |
| PL-05 | platform | Spellcheck | `C166@108699` · [S] | partial: hunspell FFI | нет UI/словарей | подчёркивание/замена | `spellcheck_contract.rs` | G5 | P3 | 1.33 | 0.6 |
| PL-06 | platform | Питание/idle | `C166@90009` · [S] · thresholds [A] | partial: power/idle crate | порог idle [A] | inhibit в звонке, idle→status | `power_contract.rs` | G5 | P2 | 2.33 | 0.6 |
| PL-07 | platform | Consent/разрешения захвата | `C166@23840` · [D] · capture IPC; consent flow [A] | absent | consent-flow не снят | явный consent, нет скрытого захвата | `capture_consent.test.mjs` | G4b+G5 | P3 | 3.67 | 0.4 |
| PL-08 | platform | Окно: badge/progress/flash/PiP | `C166@120721` · [S] | absent | нет вызовов | progress, flash, PiP | `window_contract.rs` | G5 | P3 | 2.0 | 0.6 |

## 13. Live / connectivity (наши гейты)

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| L-01 | live | Reconnect/offline state в UI | `G162:152` · [M] · UI state; gateway resume [L] | implemented (live unverified) | live resume/close-коды — сторона G1; dispatch данных в экраны — L-02 | `#net-status` (role=status) + poll 2 с + «Повторить»; `Reconnecting/Stopped`, resume/4013/4014 не дублируются | `gui-spike-features.test.mjs` (+8) | G1+G2 | P1 | — | 0.9 |
| L-02 | live | Dispatch → UI: guilds/channels/members | `G162:152,397` · [L] · UI mapping [M] | partial: reducer/core, UI на fixtures | нет привязки событий к экранам | READY/CHUNK/PRESENCE → UI без reload | `dispatch_ui.test.mjs` | G1+G2 | P1 | 4.33 | 0.8 |
| P0-01 | live | Live READY | `G162` · [L] · reference READY; ours [A] | partial (offline) | нет E3 | READY+guild+channel, токен не в логе, окно 1 | live harness N0-01 | G1+G4/PM | P0 | P0 | 0.9 |
| P0-02 | live | Live send | `S162:223` · [S] · body [A] | partial (offline) | нет E3 | сообщение в канале, 401/429, окно 1 | live harness N0-02 | G1+G4/PM | P0 | P0 | 0.9 |
| P0-03 | live | Секреты не в логах | `S162:65-67` · [S] · mechanism; logs [A] | implemented (offline) | нет live-логов | grep token/password/key = 0 | security grep + `REVIEW-auth.md` | G4b | P0 | P0 | 0.9 |

## 14. Сводка

Синхронизировано 2026-10-24 (`G6-SYNC-01.md`): 7 строк переведены в `implemented (live unverified)`,
1 (`V-08`) — из `absent` в `partial`, `P-03` остаётся `partial` (только backend).

| App-статус | Кол-во | Δ к 2026-10-24 (до sync) |
|---|---:|---:|
| implemented / verified | 21 | +7 |
| partial | 36 | −4 |
| absent | 34 | −3 |
| unknown | 0 | 0 |
| **Итого** | **91** | 0 |

| Приоритет | Кол-во |
|---|---:|
| P0 | 5 |
| P1 | 32 |
| P2 | 40 |
| P3 | 14 |

> Пересчёт при изменении строки — обязателен: матрица и `GAP-BACKLOG.md` должны сходиться.
> `implemented (live unverified)` — offline-путь + тесты, **не** E3 и не `parity-ok`.
> Проверка сходимости (независимый оракул — сам markdown, не реализация):

```bash
sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3);
  gsub(/^ +| +$/,"",app); gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app);
  a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
```

## 15. Что matrix НЕ доказывает

1. `implemented` ≠ live: строки без уровня `[L]` у нас помечены `live unverified`; это не приёмка PM.
2. `[A]`-строки (keyring, idle-порог, consent) — авторские решения, не паритет.
3. Строки с `[A]`/`[D]` запрещено кодировать до RE-задачи (устав §4.1).
4. Кол-во `partial` велико потому, что «есть crate» ≠ «есть функция»; это осознанно.
