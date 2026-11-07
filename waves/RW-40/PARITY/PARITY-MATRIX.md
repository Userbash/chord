# PARITY-MATRIX — эталон ↔ наше приложение

Дата: 2026-10-24. G6 Lead. Источник функций: `FUNCTION-INVENTORY.md`; приоритеты —
`GAP-BACKLOG.md`. Колонки единые для всех доменов.
Синхронизация после batch: `G6-SYNC-01.md` (строки G-01/C-02/G-03/G-03a/G-04/G-05/M-07/
P-03/L-01/M-06/ST-01/V-08, §14-счётчики). **2026-10-30, RW-40 `PARITY-SYNC-07`**: строки
AT-01/M-13/S-02 сверены с `RE-DEEP-REA.md` (+ `VOICE/STREAM-GATEWAY-CONTRACT.md`), G-01/C-07/M-07 —
с отчётами Wave 2; §14 пересчитан. **2026-10-30, RW-40 `PARITY-SYNC-08`**: M-13/AT-01/G-01/M-07 —
по ACCEPT Wave 3–4 (`b7b-search`, `b2a-attach/TIER-FIX`, `g01b-ui/SETTINGS-FIX`, `m07-ui-badge`),
L-02 — по ACCEPT `l02-core`/`l02-ui` + `STUB-27-DISPATCH`, MB-03/S-02 — по отчётам `stub16-profile-ui`
и `stub27-*`; `S-02` `absent → partial`, `L-02` `partial → implemented`; §14 пересчитан.
**2026-10-30, RW-40 `PARITY-SYNC-10` (приёмка 21 кодового среза и старт интеграционной волны Wave 8)**:
- 21 ACCEPTED срез учтён: `M-10` (`absent → partial`, бэкенд пинов `pins_threads.rs` 6/6 pass, 5/5 мутаций, ACCEPT `m10-pins-be`), `M-11` (`absent → partial`, бэкенд веток `pins_threads.rs` 6/6 pass, 5/5 мутаций, ACCEPT `m10-pins-be`), `V-07` (`absent → partial`, бэкенд аудио-устройств `devices.rs` 5/5 pass, 5/5 мутаций, ACCEPT `v07-devices`);
- Полная приёмка принятых ранее пакетов: `P-03` (UI-индикатор набора `typing-indicator.js` 38/38 pass, 5/5 мутаций, ACCEPT `p03-typing-ui`), `G-06` (UI-модалка инвайтов `invite-modal.js` 35/35 pass, 5/5 мутаций, ACCEPT `stub05-invites-ui`), `M-08` (AST-парсер и DOM-рендерер Markdown/Mentions `markdown.js` 48/48 pass, 5/5 мутаций, ACCEPT `m08-markdown`), `L-02`/`S-02` (интеграция `STREAM_*` в `dispatch.rs` 583/583 pass, 5/5 мутаций, ACCEPT `stub27-dispatch`);
- Запущена интеграционная волна Wave 8 (`INT-01-RUST-WIRING`, `INT-02-UI-CHAT-MARKDOWN-TYPING`, `INT-03-UI-APP-PANELS-MODALS`, `G05-REST-MUTATIONS-FIX`, Codex Review 15A/15B).
**2026-10-30, RW-40 `PARITY-SYNC-11` (24 принятых среза, `833/833` unit)**:
- `M-08` — `partial → implemented (live unverified)`: рендерер `markdown.js` встроен в `chat.js` (`INT-02-UI-CHAT-MARKDOWN-TYPING` ACCEPT, `renderMessageMarkdown`, 53/53 pass, 5/5 мутаций);
- `AT-01` — уточнён: пол `20 МиБ` (`Math.max(0x1400000, tier)`), `Tier2/3` 50/100 МиБ (`B2a-ATTACH-TIER-FIX-PASS2` ACCEPT, 17/17, 4/4), UI-лимит 20 МиБ синхронизирован (`B2b-ATTACH-UI-20MIB-SYNC` REVIEW-PASS4, 29/29, 5/5); статус остался `partial`;
- `MB-03` — поповер `#user-profile-popover` принят (`STUB-16-MEMBER-PROFILE-POPOVER-FIX` ACCEPT, 13/13, 5/5), статус `partial`;
- `G-06`/`M-10`/`M-11`/`C-04` — UI-модули и панели подключены в `app.js`, но фронтенд вызывает `create_invite`/`pins_list`/`threads_list`, которых нет в `main.rs`; статусы остались `partial` (находка G6, P1);
- `P-03` — исходящий `send_typing` подключён, входящий `TYPING_START` в UI не маршрутизирован; статус остался `partial`.
`implemented (live unverified)` — offline-паритет, E3 нет: **не** приёмка PM и **не** `parity-ok`.

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
| G-01 | guilds | Рейл: список/выбор/переключение | `UI6:01, UI:00-app.png` · [M] · visual; **источник списка `[L]` READY.guilds[]** (`002-READY.json`, 7 гильдий) | implemented (live unverified) | live READY (P0-01); иконки/монограммы — G-11. Restore `lastGuildId` закрыт: поле в `UiSettings` (G-01b-CONTRACT ACCEPT), запись только по клику (G-01b-UI ACCEPT), поле переживает `settings_save` и перезапуск (G-01b-UI-SETTINGS-FIX: P2 снят) | список из READY, выбор, пустое/ошибка/retry, клавиатура; restore сохранённой гильдии с откатом на первую, запись только по клику пользователя; тумблер настроек не стирает выбор | `tests/unit/guild-rail.test.mjs` (12 = 8 + 3 G-01b + 1 settings-fix) + READY-оракул | G2+G1 | P1 | — | 0.9 |
| G-02 | guilds | Добавить/создать сервер | `S162:1682 (GUILDS enum)` · [D] · call-site 0 | absent | endpoint `GET\|POST /guilds` не определён | RE-endpoint → модалка → create/join | `guilds_contract.rs` (new) | G1+G2 | P3 | 1.33 | 0.4 |
| G-03 | guilds | Вход по инвайту | `NC:1.1#1,1.2` · [S] · body; live codes/TTL [A] | implemented (live unverified) | live-коды/TTL/USER_BANNED-текст `[A]` (RE-2); screening не снят | resolve → preview → accept ровно `{code}`; `session_id`/`invite_instance_id`/`received_installation_id` nullable и не подставляются; 401/403/404/429 видны | `invites_contract.rs` (14) + `join-server.test.mjs` (24) | G1+G2 | P1 | — | 0.75 |
| G-03a | guilds | Войти в сервер из preview/lurker | `D02:101,138,225` · [L] · 204×2; requestId ready\|57992.6989, run2\|95974.7409 | implemented (live unverified) | live 204 из UI не снят; 403/429 `[A]`; плитка не добавляется (204 ≠ членство) | `PUT /guilds/{id}/members/@me?lurker=true` + `{}` → 204; кнопка только при `guildId` из resolve; 1 клик = 1 IPC; status «открыт для просмотра» | `guilds_contract.rs` (13+11) + `join-server.test.mjs` (+8) | G1+G2 | P1 | — | 0.9 |
| G-04 | guilds | Выход с сервера | `NC:1.1#4` · [S] · response/codes [A] | implemented (live unverified) | live-коды `[A]`: 403 owner, 404, 429 `retry_after` | confirm (`alertdialog`) → `{guildId, lurking}` → 2xx → плитка удалена; owner-пункт disabled | `guilds_contract.rs` + `gui-spike-features.test.mjs` (+10) | G1+G2 | P1 | — | 0.8 |
| G-05 | guilds | Меню сервера | `UI2:D1` · [M] · live screen measurement | partial: поповер/ARIA/клавиатура; **активны 2 из 10** | 8 пунктов — статичные заглушки; invite disabled до G-06/capability; нет прав из backend | 10 активных пунктов по правам, клавиатура, disabled-причины | `gui-spike-features.test.mjs` | G2+G3 | P1 | 5.0 | 0.9 |
| G-06 | guilds | Инвайты: создать/список/отозвать | `NC:1.1#8-11` · [S] · codes [A]; спека `DESIGN/INVITES-SPEC.md` | partial: backend создания инвайтов принят (STUB-05-INVITES-BACKEND ACCEPT, `invites.rs` 20/20 pass, 5/5 мутаций); UI модалки `#invite-create-modal` принят (STUB-05-INVITES-MODAL-UI ACCEPT, `invite-modal.js`, 35/35 pass, 5/5 мутаций), смонтирована в `app.js` (INT-03) | модалка вызывает `invoke("create_invite")`, в `main.rs` зарегистрирована `create_channel_invite` → путь UI не доходит до IPC (находка G6-PARITY-SYNC-11, P1; тесты подменяют `invoke`); нет списка активных инвайтов канала и отзыва (`DELETE /invites/{code}`); live-коды/TTL `[A]` (RE-2) | модалка создания, выбор max_age/max_uses/guest/temporary, copy link, отзыв, ошибки 429/403 | `invites_contract.rs` (20) + `join-server.test.mjs` (35) | G1+G2 | P2 | 3.5 | 0.75 |
| G-07 | guilds | Роли/каналы/права сервера | `S162:340,409,97` · [S] · UI [A] | absent | UI не снят; payload/коды [A] | out of RW-40 scope; design-first | `guild_admin_contract.rs` | G1+G2+G3 | P3 | 1.33 | 0.35 |
| G-08 | guilds | Unread/mention сервера | `UI6:01` · [M] | partial: статичный `is-unread` | нет счётчиков/read_state | бейджи каналов/рейла, сброс по ack | `read_state_ui.test.mjs` | G2+G1 | P2 | 3.0 | 0.7 |

## 3. Channels / navigation

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| C-01 | channels | Дерево: категории, text/voice/forum | `UI6:01` · [M] | partial: дерево из READY, типы 0/2/4/5/15 (C-02) | нет открытия voice/forum-представления, collapse, порядка категорий сверх READY | типы, категории, collapse, live-данные | `gui-spike-dom.test.mjs` | G2+G1 | P1 | 3.67 | 0.85 |
| C-02 | channels | Переключение канала + история | `D2:70,180` · [L] · history live; UI switch `[M]` | implemented (live unverified) | история — только первая страница (`hasMore/nextCursor` не догружаются) → M-01; G6-D-01/D-02 закрыты (D-120) | выбор → канал → история (`messages_page` реальным id); типы 0/5 открывают чат, 2/4/15 — нет; per-channel окно/scroll | `channel-switch.test.mjs` (10) + `MESSAGE_*`-оракул | G2+G1 | P1 | — | 0.9 |
| C-03 | channels | Unread/mention в канале | `UI6:01` · [M] | partial: статичный класс | нет mention_count/ack | маркер + счётчик + сброс | `read_state_ui.test.mjs` | G2+G1 | P2 | 3.0 | 0.7 |
| C-04 | channels | Шапка: пины/ветки/участники/поиск | `D2:109-110` · [L] · UI panels [M] | partial: поиск/участники есть; панели пинов/веток (`pins-threads.js`) и модалка инвайтов смонтированы в `app.js` (INT-03 pass 3: 97/97 targeted, 5/5 мутаций) | панели вызывают `pins_list`/`threads_list`, которых нет в `main.rs` (`list_channel_pins`/`list_channel_threads`), у `threads_list` не передан `archived` → реальный путь падает (P1 G6-PARITY-SYNC-11); inbox disabled (N-03); live не проверен | панели/пустые состояния/ошибки | `guild-rail.test.mjs` (15) + `channel-paging.test.mjs` (19) | G2 | P2 | 3.0 | 0.8 |
| C-05 | channels | Создание/редактирование канала | `S162:200,340` · [S] · UI [A] | absent | UI не снят | design-first, P3 | `guild_admin_contract.rs` | G2+G3 | P3 | 1.33 | 0.35 |
| C-07 | channels | QuickSwitcher `Ctrl+K` | `F162:118` · [S] · code registry; screen [M] | implemented (B6, live unverified) | live-каналы не связаны; C-07-FIX (на ревью): гильдия ищется по `data-guild-id`, legacy `data-guild` не читается; выбор в свитчере не пишет `lastGuildId` | каналы/гильдии из live, ранжирование | `gui-spike-features.test.mjs` + `gui-spike-regressions.test.mjs` (C-07) | G2 | P2 | — | 0.9 |

## 4. Messages

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| M-01 | messages | История/пагинация | `D2:70,180` · [L] | partial: fixtures + PageStore | нет live-истории/курсоров | REST-страницы, ≤500 окно, scroll | `messages_contract.rs`, `data_contract.rs` | G1+G2 | P1 | 3.67 | 0.85 |
| M-02 | messages | Отправка | `S162:223` · [S] · body [A] | implemented (live unverified) | тело/optimistic live не сверены | live send, 401/429, retry | `messages_contract.rs` + live N0-02 | G1+G2 | P0 | P0 | 0.9 |
| M-03 | messages | Редактирование | `S162:228` · [S] · body [A] | implemented (live unverified) | тело edit [A] | edit → обновление строки, 403 | `messages_contract.rs` | G1+G2 | P1 | — | 0.75 |
| M-04 | messages | Удаление | `S162:228` · [S] | implemented (live unverified) | live-удаление/403 | tombstone, confirm, ошибка | `messages_contract.rs` | G1+G2 | P1 | — | 0.8 |
| M-05 | messages | Ответ (reply) | `B162:message_reference` · [S] · call-site reply; live field [A] | implemented (live unverified) | reference/тело [A] | ответ → quote → live | `messages_contract.rs` | G1+G2 | P1 | — | 0.7 |
| M-06 | messages | Реакции | `D2:108` · [L] | implemented (live unverified) | live PUT/DELETE/429; `MESSAGE_REACTION_*` dispatch — L-02; bar-кнопка «add-reaction 👍» не снята `[A]` | pills/quick-реакции на реальном `channel:<channelId>:<item.id>`; optimistic + откат; bench не затронут | `chat-reactions.test.mjs` (9) | G1+G2 | P1 | — | 0.85 |
| M-07 | messages | Read state / ack | `D2:73,207` · [L] · `{token,last_viewed}`, 200×5, `last_viewed=4310` | implemented (live unverified) | live ack; проекция отдаёт 9 полей (`badgeCount`/`lastAckedId`/`readStateType` — M-07-BE3 ACCEPT); UI выводит `badgeCount` и `flags` канала (M-07-UI-BADGE ACCEPT 2026-10-30), бейджи рейла/сервера (G-08) и канала (C-03) ещё статичны; `CONTRACTS/read-state.md` синхронизирован на 9 полей | ack на open/focus/scroll-to-bottom, дедуп + throttle; `last_viewed` — счётчик read_state, `token` nullable и **не** bearer; badge/`null` ≠ 0, упоминания приоритетнее бейджа, `flags` не интерпретируется | `rest_ack.rs` (9) + `read_state_contract.rs` (15) + `read-state-ui.test.mjs` (13) | G1+G2 | P1 | — | 0.9 |
| M-08 | messages | Markdown/embeds/mentions | `G162:433-438` · [L] · markdown rules [S]; спека `TYPING-MARKDOWN-SPEC.md` | implemented (live unverified): embeds + AST-парсер и XSS-safe DOM-рендерер `markdown.js` (M-08 ACCEPT, 48/48, 5/5) встроены в `chat.js` через `renderMessageMarkdown` (INT-02-UI-CHAT-MARKDOWN-TYPING ACCEPT, 53/53 pass, 5/5 мутаций) | кастом-эмодзи/стикеры; live-рендер не сверен с эталоном (E3) | рендер по `I162`, безопасные ссылки, экранирование XSS, spoiler click/keys | `gui-spike-model.test.mjs` (53) | G2 | P1 | — | 0.85 |
| M-09 | messages | Контекстное меню (15) | `UI:01` · [M] | partial: 4 команды, остальные stub | нет forward/bookmark/reminder/TTS/report | пункты по правам, клавиатура | `gui-spike-features.test.mjs` | G2 | P2 | 2.67 | 0.85 |
| M-10 | messages | Пины | `D2:109` · [L]; спека `DESIGN/PINS-THREADS-PROFILE-SPEC.md` §2 | partial: backend принят (M-10-M-11-PINS-THREADS-BACKEND ACCEPT, `pins_threads.rs` 6/6 pass, 5/5 мутаций) + панель `#chat-pins-panel` (STUB-10, `pins-threads.js`, 19/19, 5/5), подключена в `app.js` (INT-03) | `app.js` вызывает `pins_list`, в `main.rs` зарегистрирована `list_channel_pins` → панель не получает данные (P1 G6-PARITY-SYNC-11); live-пины не сверены | список, jump, ошибки 404/403 | `pins_threads_contract.rs` (6) + `channel-paging.test.mjs` (19) | G1+G2 | P2 | 3.0 | 0.8 |
| M-11 | messages | Ветки | `D2:110` · [L]; спека `DESIGN/PINS-THREADS-PROFILE-SPEC.md` §3 | partial: backend принят (M-10-M-11-PINS-THREADS-BACKEND ACCEPT, `pins_threads.rs` 6/6 pass, 5/5 мутаций) + панель `#chat-threads-panel` с вкладками active/archived (STUB-10, 19/19, 5/5), подключена в `app.js` | `threads_list` → `list_channel_threads`, обязательный `archived` не передаётся; создание ветки из сообщения отсутствует; live не сверен | список/создание/архив | `pins_threads_contract.rs` (6) + `channel-paging.test.mjs` (19) | G1+G2 | P2 | 2.0 | 0.75 |
| M-12 | messages | Forward/bookmark/reminder/TTS/report | `UI:01` · [M] · endpoints [A] | absent | endpoint'ы не сняты | после RE; P3 | `message_actions_contract.rs` | G1+G2 | P3 | 1.33 | 0.4 |
| M-13 | messages | Поиск по серверу | `F162:1, 20f` · [L] · live network capture; **фильтры подтверждены** `RE-DEEP-REA §2.1–2.4` (таблица `queryKey`) | implemented (live unverified) | backend `search.rs` принят (B7b ACCEPT); UI оверлея и фильтров поиска принят (STUB-14-UI-SEARCH-FIX ACCEPT, `search-overlay.js` + `search.js` 108/108 pass, 5/5 мутаций); открыто: `202`-ретрай (≤5, `retry-after`), `include_nsfw` не реализованы; `link_hostname`/`attachment_extension`/`attachment_filename` — вне UI; live-поиск и пагинация не сверены | live-поиск, пагинация `offset` шагом 25, ошибки/202 | `search_contract.rs` (16) + `search-remote-real-index.test.mjs` (108) | G1+G2 | P1 | — | 0.9 |
| M-14 | messages | Эмодзи-пикер | `F162:2` · [M] | implemented (static) | нет кастомных эмодзи/стикеров | live-эмодзи, поиск, locked | `gui-spike-features.test.mjs` | G2 | P2 | — | 0.8 |

## 5. Members

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| MB-01 | members | Список: роли + статусы | `F162:55-62` · [M] | partial: 24k synthetic, 6 групп | нет live-данных/сортировки | live-члены, группы, виртуализация | `member_list_*.rs` + UI | G1+G2 | P1 | 4.5 | 0.8 |
| MB-02 | members | Роли/цвета/права | `G162:397` · [L] · roles in READY; UI [M] | partial: synthetic | нет live-ролей/цветов | роль-цвет, hoist, права | `presence_types.rs` + UI | G1+G2 | P2 | 3.0 | 0.7 |
| MB-03 | members | Профиль-поповер | `UI5:04` · [M] · спека `DESIGN/PINS-THREADS-PROFILE-SPEC.md` §1, §4–6 | partial: поповер `#user-profile-popover` реализован (STUB-16-MEMBER-PROFILE-POPOVER-FIX ACCEPT, `members.js`: блок STUB-16 — 10/10, файл 13/13, 5/5 мутаций): 4 состояния (`empty/loading/error/populated`), WAI-ARIA, Escape/outside-click, токены `tokens.css` | данные берутся из synthetic-списка (MB-01), а не из REST-профиля; кнопки действий без обработчиков; live не проверен | карточка участника из live, роли/права, действия | `gui-spike-regressions.test.mjs` (13, блок STUB-16 — 10) | G2 | P2 | 3.0 | 0.75 |
| MB-04 | members | Полный поиск участников | `NC:1.5` · [S] · 202 retry; pagination [A] | absent (deferred) | нет пагинации/кодов (RE) | RE request/response → поиск | `members_search_contract.rs` | G1+G2 | P2 | 2.5 | 0.5 |

## 6. Presence / typing

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P-01 | presence | Статусы online/idle/dnd/offline/invisible | `G162:76,293` · [L] | partial: synthetic + core | статусы не из live | live-статус в списке/панели | `presence_reducer.rs` + UI | G1+G2 | P1 | 4.5 | 0.85 |
| P-02 | presence | Обновления PRESENCE_UPDATE | `G162:152,397` · [L] | partial: parse/reducer, UI не связан | нет применения к UI | reducer → UI, reconnect | `presence_reducer.rs` | G1+G2 | P1 | 4.5 | 0.8 |
| P-03 | presence | Индикатор набора + POST /typing | `S162:253, NC:1.6` · [S] · TYPING_START live [L]; TTL `[A]`; спека `TYPING-MARKDOWN-SPEC.md` | partial: backend `typing_contract.rs` (13) + UI-контроллер `#chat-typing-indicator` (P-03-UI ACCEPT, 38/38, 5/5); исходящий `send_typing` подключён в `app.js` (троттлинг 8 с, guard ≤5 каналов) | входящий `TYPING_START` не маршрутизирован из dispatch в индикатор (в UI нет подписки); live TYPING_START не сверен | `POST /channels/{id}/typing` без тела → 200; индикатор `aria-live="polite"`, троттлинг 8 с, guard ≤5 каналов, slowmode 429 | `typing_contract.rs` (13) + `gui-spike-dom.test.mjs` (38) + `typing-ui.test.mjs` (20) | G1+G2 | P1 | 4.5 | 0.85 |
| P-04 | presence | Кастомный статус/активность | `G162:250` · [L] | absent | нет редактора/отображения | статус+activity, 429 | `presence_activities_contract.rs` | G1+G2 | P2 | 3.0 | 0.6 |
| P-05 | presence | Смена своего статуса | `G162:293, UI3:B-panels` · [L] · invisible in READY | absent | нет меню статуса | меню, IDENTIFY/UPDATE, persist | `presence_update_contract.rs` | G1+G2 | P2 | 3.0 | 0.6 |

## 7. Voice

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| V-01 | voice | Вход/выход из voice-канала | `V162:23-52` · [L] | partial: crate + менеджер голосовой сессии принят (V-01-RTC-FIX ACCEPT, `session.rs` 147/147 pass, 7/7 мутаций) | нет UI-связывания сессии с голосовым каналом и кнопкой отключения | join → op2/op4 → media; leave 1000, реконнект при TransportFailed/Oversize | `signaling.rs` + `session_contract.rs` (147) | G1 | P1 | 2.6 | 0.85 |
| V-02 | voice | RTC-сигнализация | `V162:23-52` · [L] | partial: парсер/тесты + сессия `session.rs` | нет live-WS сессии | op0/1/2/4/8, heartbeat 13.75 с | `signaling.rs` + `session_contract.rs` | G1 | P1 | 3.25 | 0.8 |
| V-03 | voice | UDP discovery/keepalive/rotation | `V162:53-90` · [L] | partial: примитивы + socket-бинд в `session.rs` | нет live-UDP | discovery 74 Б, select, keepalive 5 с, +60 с | `udp.rs` + `session_contract.rs` | G1 | P1 | 3.25 | 0.8 |
| V-04 | voice | SRTP | `V162:65-90` · [L] · decrypt [A] | partial: KDF/AEAD тесты | нет live encrypt/decrypt | ключи op4, tag16/tail4, нет plaintext | `srtp_contract.rs`, `crypto_vectors.rs` | G1+G4b | P1 | 3.5 | 0.8 |
| V-05 | voice | Opus/джиттер/DTX | `V162:91-118` · [L] · jitter model [M] | partial: jitter, нет Opus | нет encode/decode | 48 кГц, PT120, DTX/CN, loopback | `media_contract.rs` + loopback | G5+G1 | P1 | 3.0 | 0.7 |
| V-06 | voice | Статистика/reconnect/регионы | `V162:119-123` · [M] · ping UI; reconnect code [S] | partial: policy/stats + `session.rs` | нет live RTT/потерь | RTT/потери, reconnect, регион | `stats_contract.rs`, `session_contract.rs` | G1 | P2 | 3.33 | 0.7 |
| V-07 | voice | Устройства/mute/deafen/PTT | `F162:63-83` · [M] · settings screen | partial: backend аудио-устройств принят (V-07-VOICE-DEVICES-CONTRACT ACCEPT, `devices.rs` 5/5 pass, 5/5 мутаций) | UI выбора аудио-устройств в настройках, привязка к системному PulseAudio/PipeWire | выбор in/out, hot-plug, PTT, mute/deafen, redaction меток | `devices_contract.rs` (5) | G5 | P1 | 2.25 | 0.8 |
| V-08 | voice | Голосовой UI: панель/участники/ping | `VUI:01` · [M] | partial: панель на fake store (D-115) | нет реального voice-backend (V-01…V-05); `[M]` speaking-анимация — не паритет | join/leave, участники, speaking, ping; `data-state`, aria-pressed | `voice-ui.test.mjs` (44) | G2+G3 | P1 | 3.67 | 0.85 |
| V-09 | voice | Шумоподавление Krisp | `V162:91-118` · [L] · engine log | absent | нет модели/интеграции | RNNoise-совместимо, вкл/выкл | `krisp_contract.rs` | G5 | P2 | 2.33 | 0.6 |
| V-10 | voice | DAVE/MLS E2EE | `V162:91-118` · [L] · init; 2-client [A] | absent | нет MLS | framing/ключи/ротация/интероп | `dave_contract.rs` | G1+G4 | P2 | 2.2 | 0.6 |
| V-11 | voice | Voice messages/clips/soundboard | `C166@11203` · [D] · clips enum; soundboard live [L] | absent | контракт не снят | запись/клипы/soundboard | `clips_contract.rs` | G5+G2 | P3 | 2.0 | 0.35 |

## 8. Video / Go Live

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S-01 | video | Камера: preview/enable | `V162:29-32` · [L] · video capability; camera [A] | absent | нет захвата/превью; выбор portal Camera vs v4l2 открыт (`D-2`) | v4l2/portal, preview, mic отдельно | `camera_contract.rs` | G5 | P2 | 1.75 | 0.6 |
| S-02 | video | Screen share / Go Live start-stop | `VUI:01, C166@23840` · [D] · capture IPC/UI; **STREAM_* подтверждены** `RE-DEEP-REA §6.1–6.3` + `VOICE/STREAM-GATEWAY-CONTRACT.md` (op 18…22, поля, `stream_key`, 6 фаз) | partial: byte-level RE-оракул и редьюсер `stream.rs` принят (STUB-27-CORE-ORACLE-FIX ACCEPT, 11/11 pass, 5/5 мутаций) + маршрутизация 4 серверных `STREAM_*` в `dispatch.rs` + дизайн-ревью `STREAM-REVIEW-PASS3` PASS (F-02..F-04 закрыты); исходящих op 18/19, capture и UI нет | нет capture/consent (PL-07), `STREAM_CREATE`(18)/`STREAM_DELETE`(19), второго RTC и UI; права `canStream` — SG-12; живых `STREAM_*` событий 0 (0 в 1340 op0) | consent → выбор источника → `STREAM_CREATE`(18) → `STREAM_SERVER_UPDATE` → второй RTC → `STREAM_DELETE`(19); без optimistic start; код отказа в IPC | `stream_gateway_contract.rs` (11) + `dispatch_types.rs` (4) | G5+G1 | P2 | 1.6 | 0.8 |
| S-03 | video | Зрители/качество/simulcast | `NC:3.3` · [A] · viewer flow; STREAM_* code is [S] | absent | нет RTP-видео/декода; B-08 (KDF/nonce/AAD) блокирует wire-путь | rid/quality, зритель видит поток | `video_rtp_contract.rs` | G1+G5 | P2 | 1.4 | 0.4 |
| S-04 | video | Activities | `VUI:01` · [M] | absent | контракт запуска | запуск/выход/права | `activities_contract.rs` | G1+G2 | P3 | 1.33 | 0.4 |

## 9. Attachments

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| AT-01 | attachments | Загрузка файла (multipart/прогресс) | `S162:817` · [D] · path only; **RE снят**: `RE-DEEP-REA §1.1–1.8`, `ATTACH-RE.md §2` — маршрут, тело, лимиты, коды в коде эталона | partial: backend слотов + tier-лимиты `Math.max(0x1400000, tier)`: пол 20 МиБ (None/Tier1), 50/100 МиБ (Tier2/3) — B2a-ATTACH-TIER-FIX-PASS2 ACCEPT (17/17, 4/4); UI-лимит 20 МиБ синхронизирован (B2b-ATTACH-UI-20MIB-SYNC, REVIEW-PASS4, `composer.js` 29/29, 5/5) | UI вызывает `attachments_upload_slots`/`attachments_upload_put`; в `main.rs` есть только `request_upload_slots`, PUT-команды нет → реальная передача файла не работает (P1 B2b REVIEW-PASS4); нет прогресса/докачки `308`/`Content-Range`; суммарный лимит 500 МБ / 1 ГиБ не параметризован; живых кодов 40005/413/429 нет | `POST /channels/{id}/attachments` → `{attachments:[{upload_url,upload_filename}]}` → PUT → `uploaded_filename` в `POST /messages`; allowlist `channel_id` до I/O; 40005/413 и 429 `retry-after` | `attachments_contract.rs` (17) + `composer.test.mjs` (29) | G1+G2 | P2 | 3.0 | 0.85 |
| AT-02 | attachments | Drag&drop/paste | `CAT:IJyOUf` · [M] | absent | нет триггеров | drop/paste → upload | `attachments_contract.rs` | G2 | P2 | 4.0 | 0.45 |
| AT-03 | attachments | Рендер вложений | `G162:433-438` · [D] · key null; schema [A] | absent (только embeds) | нет схемы attachment | image/video/file/audio, CDN | `attachments_render.test.mjs` | G2 | P2 | 4.5 | 0.5 |

## 10. Settings

| ID | Домен | Функция | Ref (source/level) | Наше приложение | Gap | DoD | Test | Owner | P | WSJF | Conf |
|---|---|---|---|---|---|---|---|---|---|---|---|
| ST-01 | settings | Навигация настроек | `F162:63-70` · [M] · дамп `ui8/23-settings-categories.json` (`count:63`, 5 455 Б) | partial: 18 разделов, 3 панели; профиль/4 подпункта/«Выйти» есть (D-118) | 63 DOM-строки = профиль 2 + 18 разделов/«Выйти»×3 + 4 подпункта «Учётной записи»; подпункты 17 разделов свёрнуты → `[A]`; панели подпунктов — `[A]` | навигация 63, disabled-причины в `title`/`aria-describedby`, `aria-current` | `gui-spike-features.test.mjs` + фикстура-оракул `tests/fixtures/settings-nav-63.tsv` (new) | G2+G3 | P2 | 6.0 | 0.85 |
| ST-02 | settings | Голос/видео: 38 тумблеров/устройства | `F162:71-83` · [M] | partial: 38 toggles, бэкенд устройств реализован в Wave 6 (V-07, `devices.rs`), нет связывания | связывание тумблеров с `devices.rs` и аудио-рантаймом | устройства, проверка, persist | `settings_contract.rs` + `devices_contract.rs` | G5+G2 | P2 | 3.0 | 0.8 |
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
| L-02 | live | Dispatch → UI: guilds/channels/members | `G162:152,397` · [L] · UI mapping [M]; L-02-CORE/L-02-UI ACCEPT | implemented (live unverified) | E3 нет: live READY/CHUNK/PRESENCE в экранах не проверены; `TYPING_START` → индикатор (P-03-UI в 13A), `PRESENCE_UPDATE` → UI (P-02); `STREAM_*` мост в `stream::apply` сдан (STUB-27-DISPATCH / STUB-27-CORE-ORACLE-FIX ACCEPT) | READY/CHUNK/PRESENCE → UI без reload | `dispatch_ui.test.mjs` (43) + `dispatch_types.rs` (4) | G1+G2 | P1 | — | 0.9 |
| P0-01 | live | Live READY | `G162` · [L] · reference READY; ours [A] | partial (offline) | нет E3 | READY+guild+channel, токен не в логе, окно 1 | live harness N0-01 | G1+G4/PM | P0 | P0 | 0.9 |
| P0-02 | live | Live send | `S162:223` · [S] · body [A] | partial (offline) | нет E3 | сообщение в канале, 401/429, окно 1 | live harness N0-02 | G1+G4/PM | P0 | P0 | 0.9 |
| P0-03 | live | Секреты не в логах | `S162:65-67` · [S] · mechanism; logs [A] | implemented (offline) | нет live-логов | grep token/password/key = 0 | security grep + `REVIEW-auth.md` | G4b | P0 | P0 | 0.9 |

## 14. Сводка

Синхронизировано 2026-10-24 (`G6-SYNC-01.md`): 7 строк переведены в `implemented (live unverified)`,
1 (`V-08`) — из `absent` в `partial`, `P-03` остаётся `partial` (только backend).

Синхронизировано **2026-10-30** (RW-40, `PARITY-SYNC-07`, вход `RE-DEEP-REA.md`,
`VOICE/STREAM-GATEWAY-CONTRACT.md`, отчёты Wave 2): `AT-01` — `absent → partial` (B2a backend
слотов, RE-STUB-13 снят); `M-13` — уточнён (`4/7 → 7/7` UI-фильтров, backend по-прежнему 5
полей); `S-02` — RE-STUB-27 подтверждён и контракт шлюза заморожен, статус остался `absent`;
`G-01`/`C-07`/`M-07` — уточнены Gap/DoD/Test без смены статуса.

Синхронизировано **2026-10-30** (RW-40, `PARITY-SYNC-08`, вход — 12 ACCEPT-файлов,
`STUB-16`/`STUB-27` отчёты, `QUALITY/TEST-INTEGRITY-WAVE3.md`): `S-02` — `absent → partial`
(парсер `stream.rs` + маршрутизация 4 серверных `STREAM_*` в `dispatch.rs`); `L-02` —
`partial → implemented (live unverified)` (`l02-core`/`l02-ui` ACCEPT + `STREAM_*` мост);
`M-13` — backend 8 фильтров принят (B7b), gap сузился до `202`/`include_nsfw`/live;
`AT-01` — tier-лимиты 10/50/100 МиБ приняты (B2a TIER-FIX); `G-01` — P2 «`settings.js`
теряет `lastGuildId`» снят (G-01b-UI-SETTINGS-FIX); `M-07` — UI-бейджи приняты
(M-07-UI-BADGE); `MB-03` — поповер реализован (STUB-16), статус остался `partial`.

Синхронизировано **2026-10-30** (RW-40, `PARITY-SYNC-10`, приёмка 21 кодового среза и старт Wave 8):
- 21 ACCEPTED срез: `M-10` переведён `absent → partial` (бэкенд пинов `pins_threads.rs` 6/6 pass, 5/5 мутаций, ACCEPT `m10-pins-be`), `M-11` переведён `absent → partial` (бэкенд веток `pins_threads.rs` 6/6 pass, 5/5 мутаций, ACCEPT `m10-pins-be`), `V-07` переведён `absent → partial` (бэкенд устройств `devices.rs` 5/5 pass, 5/5 мутаций, ACCEPT `v07-devices`);
- Полная приёмка ранее разработанных компонентов Wave 6: `P-03` (ACCEPT `p03-typing-ui`), `G-06` UI (ACCEPT `stub05-invites-ui`), `M-08` (ACCEPT `m08-markdown`), `L-02`/`S-02` (ACCEPT `stub27-dispatch`);
- Запуск интеграционной волны Wave 8 (`INT-01`, `INT-02`, `INT-03`, `G05-REST-MUTATIONS-FIX`, Codex Review 15A/15B).

Синхронизировано **2026-10-30** (RW-40, `PARITY-SYNC-11`, 24 принятых среза, `npm run test:unit` 833/833):
- `M-08` переведён `partial → implemented (live unverified)`: `markdown.js` встроен в `chat.js` (`INT-02-UI-CHAT-MARKDOWN-TYPING` ACCEPT, 53/53 pass, 5/5 мутаций);
- `AT-01` уточнён без смены статуса: пол `20 МиБ` (`Math.max(0x1400000, tier)`), `Tier2/3` = 50/100 МиБ (`B2a-ATTACH-TIER-FIX-PASS2` ACCEPT, 17/17, 4/4), UI-лимит выровнен (`B2b-ATTACH-UI-20MIB-SYNC` pass 4, 29/29, 5/5); PUT-команды в IPC по-прежнему нет;
- `MB-03` уточнён: `#user-profile-popover`, файл `gui-spike-regressions.test.mjs` 13/13 (блок STUB-16 — 10), 5/5 мутаций (`STUB-16` ACCEPT);
- `G-06`/`M-10`/`M-11`/`C-04` остались `partial`: контроллеры смонтированы в `app.js` (INT-03 pass 3, 97/97, 5/5), но фронтенд вызывает `create_invite`/`pins_list`/`threads_list`, которых нет в `main.rs` (P1 G6-PARITY-SYNC-11);
- `P-03` остался `partial`: исходящий `send_typing` подключён, входящий `TYPING_START` в UI не маршрутизирован.

Синхронизировано **2026-10-30** (RW-40, `PARITY-SYNC-12`, 26 принятых срезов + 2 дизайн-спеки, `npm run test:unit` 834/834):
- Приняты `STUB-10-PINS-THREADS-PANELS-UI` (`ACCEPT.md`, 19/19 pass, 5/5 мутаций) и `G3-TYPING-MARKDOWN-SPEC` (`REVIEW-PASS4.md`, PASS, выравнивание токенов по `tokens.css`);
- Завершён `INT-03-SAVE-QUEUE-FIX` (`PM/g01b-ui/REPORT-PASS4.md`, 98/98 targeted exit 0, 5/5 мутаций killed, `app.js` сериализует сохранение настроек через `saveQueue`, 16/16 тестов в `guild-rail.test.mjs`);
- `G-01` (`implemented (live unverified)`): подтверждено закрытие гонки `settings_save` при быстрых переключениях гильдий;
- `M-10`, `M-11`, `C-04` остаются `partial`: UI-панели пинов и веток смонтированы, но открыт F-1 (несоответствие IPC-команд `pins_list` / `threads_list`);
- Счётчики матрицы сохранены: 23 implemented / 41 partial / 27 absent, итого 91, P0..P3 = 5/32/40/14.

| App-статус | Кол-во | Δ к снимку PARITY-SYNC-10 |
|---|---:|---:|
| implemented / verified | 23 | +1 (`M-08`) |
| partial | 41 | −1 (`M-08`) |
| absent | 27 | 0 |
| unknown | 0 | 0 |
| **Итого** | **91** | 0 |

| Приоритет | Кол-во |
|---|---:|
| P0 | 5 |
| P1 | 32 |
| P2 | 40 |
| P3 | 14 |

> **Сходимость с бэклогом на 2026-10-30 (PARITY-SYNC-11):** `GAP-BACKLOG.md` в назначенные файлы воркера
> не входит и на снимке 18:14 остаётся на 22/39/30 (его последняя синхронизация — G6-SYNC-01/08).
> **Нужна правка Team Lead:** `M-08` → implemented, `AT-01`/`MB-03` — уточнения, `G-06`/`M-10`/`M-11`/`C-04` —
> открытый P1 по именам IPC-команд; целевой баланс 23/41/27. Счётчики P0/P1/P2/P3 не менялись: 5/32/40/14.

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