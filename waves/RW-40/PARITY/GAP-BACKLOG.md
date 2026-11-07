# GAP-BACKLOG — очередь паритета G6

Дата: 2026-10-24. Основание: `PARITY-MATRIX.md` (91 строка), `FUNCTION-INVENTORY.md`.
Правила: P0 всегда выше; P1 → P2 → P3; внутри класса — WSJF-lite `(BV+TC+RR)/JS`.
`Conf < 0.5` → сначала RE-spike, не код. Live-функции — только с окном PM.
Синхронизировано по `G6-SYNC-01.md`: 7 P1-строк ушли в `implemented (live unverified)`,
их остатки — в §2.4 (из очереди не выведены, но код по ним не планируется до live-окна).
Синхронизировано **2026-10-30** (`PARITY-MATRIX.md`, RW-40 `GAP-BACKLOG-SYNC`): L-02 выведен
в §2.4 (ACCEPT), S-02 `absent → partial` (маршрутизация `STREAM_*` сдана), M-13/AT-01/MB-03
сужены по ACCEPT и отчётам Wave 3–4; §7 пересчитан под 22/37/32.
Синхронизировано **2026-10-30** (RW-40, `PARITY-SYNC-09`):
- V-01 (`absent → partial`, ACCEPT `v01-rtc`), G-06 (`absent → partial`, ACCEPT `stub05-invites-be`);
- M-13/S-02 обновлены по ACCEPT `stub14-search-ui`, `stub27-core` и `STREAM-REVIEW-PASS3`;
- Отражены завершённые в коде Wave 6 и переданные на Codex-ревью Batch 13A/13B: P-03, G-06 (UI), M-08, M-10, M-11, V-07;
- §7 пересчитан под 22 implemented / 39 partial / 30 absent (всего 91).

## 0. Как читать

- **RE-гейт** — без подтверждения контракта код запрещён (устав §4.1).
- **Live-окно** — 1 окно на приёмку, тестовый аккаунт, без чужих данных.
- **Параллелизм** — ≤2 кодовых воркера; один `cargo` на target-dir.
- `—` в WSJF — функция реализована (offline), остаётся live-верификация в N0-окне,
  либо работа вне очереди RW-40.

## 1. P0 — stop-the-line (5)

| ID | Gap | Owner | Live-окно | DoD | Test |
|---|---|---|---|---|---|
| P0-01 | Нет live READY 1.0.166 | G1+G4, PM | да, 1 | READY + guild/channel в логе, токен не в логе | live-харнесс N0-01 |
| P0-02 | Нет live send | G1+G4, PM | да, 1 (после P0-01) | сообщение видно в канале; 401/429 | live-харнесс N0-02 |
| P0-03 | Нет live-подтверждения, что секреты не в логах | G4b | да (то же окно) | grep token/password/key = 0; redaction | `REVIEW-auth.md` + grep |
| A-01 | Live-вход по паролю не прогнан | G1/G4 | да | вход → ready; капча/429 | `auth_contract.rs` + live |
| M-02 | Live-отправка не сверена с эталоном | G1+G2 | да | optimistic→ok→reload | `messages_contract.rs` + live |

> A-01/M-02 — не «два P0»: это части окон P0-01/P0-02, выделены для трассировки.

## 2. P1 — ежедневный паритет (32)

### 2.1 Топ-9 по WSJF-lite (принятые строки из топа выведены; L-02 → §2.4)

| # | ID | Gap | Owner | WSJF | Зависит | DoD / test |
|---:|---|---|---|---:|---|---|
| 1 | G-05 | Меню сервера: активно 2 из 10 пунктов | G2+G3 | 5.0 | G-06, права из backend | 10 активных пунктов, disabled-причины, клавиатура; `gui-spike-features.test.mjs` |
| 2 | P-03 | Typing: backend есть, UI `#chat-typing-indicator` реализован в Wave 6 (на ревью Codex 13A) | G1+G2 | 4.5 | L-02 (TYPING_START), D-114 | `send_typing` + «Печатает…» `aria-live="polite"`, троттлинг 8 с, guard ≤5, slowmode; `typing_contract.rs` + `gui-spike-dom.test.mjs` |
| 3 | MB-01 | Участники только synthetic | G1+G2 | 4.5 | L-02 | live-члены, группы, виртуализация; `member_list_*` + UI |
| 4 | P-01 | Статусы не из live | G1+G2 | 4.5 | L-02 | статус в списке/панели; `presence_reducer.rs` |
| 5 | P-02 | PRESENCE_UPDATE не доходит до UI | G1+G2 | 4.5 | L-02 | reducer → UI, reconnect; `presence_reducer.rs` |
| 6 | C-01 | Дерево: нет open voice/forum, collapse | G2+G1 | 3.67 | L-02 | типы, категории, collapse; DOM-тест |
| 7 | M-01 | История только fixtures + 1 страница | G1+G2 | 3.67 | P0-01 | REST-страницы, `hasMore/nextCursor`, ≤500, scroll; `messages_contract.rs` |
| 8 | V-08 | Голосовой UI на fake store | G2+G3 | 3.67 | V-01…V-05 | панель/участники/ping/speaking на реальном состоянии; `voice-ui.test.mjs` |
| 9 | V-04 | SRTP только тесты | G1+G4b | 3.5 | V-03 | live encrypt/decrypt, no plaintext; `srtp_contract.rs` |

Снято с топа как `implemented (live unverified)`: L-01 (6.0), G-01 (5.5), C-02 (5.0),
G-04 (4.5), M-07 (4.5), L-02 (4.33) — их остатки в §2.4. Ниже топа: **G-03, G-03a** (приняты, D-116/D-122/D-128),
**M-06** (принят, D-131). Голосовая цепочка (`V-02 → V-03 → V-04 → V-05 → V-07 → V-08`) остаётся
XL и стартует после N1 (§2.3).

### 2.2 Остальные P1 (14 здесь + 9 в топе + 9 в §2.4 = 32)

| ID | Gap | Owner | WSJF | Зависит / RE | DoD / test |
|---|---|---|---:|---|---|
| A-02 | MFA live не прогнан | G1/G4 | — | окно N0 | 5 методов, ошибки; `auth_contract.rs` |
| A-03 | Captcha live не прогнан | G1/G4 | — | окно N0 | интерактив, отмена; `auth_contract.rs` |
| A-06 | Restore live не прогнан | G1 | — | окно N0 | restore/clear/идемпотентность; `auth_contract.rs` |
| A-07 | Logout live не прогнан | G1 | — | окно N0 | stop→clear→state; `auth_contract.rs` |
| M-03 | Edit live не сверен | G1+G2 | — | окно N0 | 403/обновление; `messages_contract.rs` |
| M-04 | Delete live не сверен | G1+G2 | — | окно N0 | tombstone/403; `messages_contract.rs` |
| M-05 | Reply live не сверен | G1+G2 | — | окно N0 | quote/reference; `messages_contract.rs` |
| M-08 | Markdown/mentions AST-парсер и DOM-рендер реализованы в Wave 6 (на ревью Codex 13A); открыто: связывание с `chat.js`, кастом-эмодзи | G2 | 3.0 | i18n markdown, `TYPING-MARKDOWN-SPEC.md` | рендер по `I162`; `gui-spike-model.test.mjs` (49) |
| M-13 | Поиск live не сверен; backend 8 фильтров принят (B7b), UI оверлея фильтров принят (STUB-14 ACCEPT), остались `202`-ретрай и `include_nsfw` | G1+G2 | — | окно N0 | live-поиск/пагинация; `search_contract.rs` (16) + `search-remote-real-index.test.mjs` (108) |
| V-01 | Сессия voice принята (V-01-RTC-FIX ACCEPT); открыто UI-связывание сессии с голосовым каналом и live-прогон | G1 | 2.6 | V-02…V-05 | join→op2/op4→media→leave; `session_contract.rs` (147) |
| V-02 | RTC WS только парсер | G1 | 3.25 | V-01 | op0/1/2/4/8, heartbeat; `signaling.rs` |
| V-03 | UDP только примитивы | G1 | 3.25 | V-02 | discovery/select/keepalive/rotation; `udp.rs` |
| V-05 | Нет Opus | G5+G1 | 3.0 | V-04 | 48 кГц/PT120/DTX, loopback; `media_contract.rs` |
| V-07 | Backend устройств, хот-плага и PTT реализован в Wave 6 (на ревью Codex 13B); открыто UI в настройках | G5 | 2.25 | V-05 | in/out, hot-plug, PTT, mute; `devices_contract.rs` (5) |

### 2.3 Голосовая цепочка (P1, но XL)

`V-02 → V-03 → V-04 → V-05 → V-07 → V-08`. WSJF низкий из-за `job_size`, но это
release-critical: без неё нет «клиента как Discord». В Wave 6 принята сессия `V-01` (`session.rs`)
и реализован бэкенд аудио-устройств `V-07` (`devices.rs`). Запуск live — после N1, ≤2 воркера,
только с окном PM на live-голос (два тестовых клиента / второй участник с согласия).

### 2.4 Принято в batch — остатки (9; из очереди выведены, паритетом не считать)

Код по этим строкам **принят** (offline, D-112…D-131 + ACCEPT Wave 3–4); открыто только
перечисленное. Это не `parity-ok`: E3 нет, live-окно не проводилось.

| ID | Принято | Остаток | Где закрывается |
|---|---|---|---|
| G-01 | D-112 + G-01b-CONTRACT/G-01b-UI ACCEPT | live READY (P0-01); restore `lastGuildId` закрыт полностью — поле переживает `settings_save` и перезапуск (G-01b-UI-SETTINGS-FIX, P2 снят); монограммы/иконки — G-11 | окно N0 |
| C-02 | D-117/D-120 | пагинация истории (`hasMore/nextCursor`), окно ≤500 | M-01 |
| G-03 | D-116/D-122 | live-коды/TTL, `USER_BANNED`/screening (`[A]`), геометрия модалки `unmeasured` | RE-2 + N1 |
| G-03a | D-76/D-128 | live 204 из UI; 403/429 `[A]` | RE-2 + N1 |
| G-04 | D-89/D-92 | live-коды 403 owner/404/429 `retry_after` (`[A]`) | RE-2 + N1 |
| M-07 | D-126 + M-07-BE3/M-07-UI-BADGE ACCEPT | live ack; `badgeCount`/`flags` канала выведены и контракт `CONTRACTS/read-state.md` на 9 полей; открыты бейджи рейла/сервера (G-08) и канала (C-03) | G-08/C-03 + N0 |
| L-01 | D-106 | live resume/4013/4014 (G1); данные в экраны — L-02 | N0 + L-02 |
| M-06 | D-131 | live PUT/DELETE/429; `MESSAGE_REACTION_*` dispatch; кнопка add-reaction `[A]` | L-02 + N0 |
| L-02 | `l02-core`/`l02-ui` ACCEPT + STUB-27-DISPATCH / CORE ACCEPT | live READY/CHUNK/PRESENCE в экранах (E3); `TYPING_START` → индикатор — P-03-UI (в Wave 6 на ревью 13A); `PRESENCE_UPDATE` → UI — P-02 | окно N0 + P-02/P-03 |

## 3. P2 — вторичный паритет (40)

Порядок: сначала ежедневные (unread, настройки, уведомления), затем медиа.
`AT-01` — RE-1 снят (маршрут/тело/лимиты), backend слотов и tier-лимиты приняты; открыты
`PUT` upload и UI. `AT-02/03` — `[M]/[D]`.

| ID | Gap | Owner | WSJF | Зависит / RE | DoD / test |
|---|---|---|---:|---|---|
| AT-03 | Нет рендера вложений | G2 | 4.5 | **RE-схема attachment** | image/video/file/audio; `attachments_render.test.mjs` |
| ST-05 | Уведомления: панель-заглушка | G2 | 4.5 | N-05 | уровни/звуки/persist; `settings_contract.rs` |
| N-05 | Нет per-guild/channel override | G2+G1 | 4.5 | — | меню уровней; `notification_overrides.test.mjs` |
| ST-01 | Навигация: 18 разделов/3 панели, подпункты 17 разделов не сняты | G2+G3 | 6.0 | `[A]`-подпункты, дизайн | 63 DOM-строки (профиль 2 + 18 разделов/«Выйти»×3 + 4 подпункта); профиль/4 подпункта/«Выйти» — приняты D-118; фикстура-оракул `tests/fixtures/settings-nav-63.tsv` (new) |
| AT-02 | Нет drag&drop/paste | G2 | 4.0 | AT-01 | drop/paste → upload; `attachments_contract.rs` |
| G-06 | Backend создания инвайтов принят (STUB-05 ACCEPT); UI модалки `#invite-create-modal` реализован в Wave 6 (на ревью Codex 13A); открыто: список активных инвайтов канала и отзыв (`DELETE`) | G1+G2 | 3.5 | коды/TTL [A] (RE-2), `INVITES-SPEC.md` | модалка/link/revoke; `invites_contract.rs` (20) + `join-server.test.mjs` (35) |
| PL-01 | Нет трея | G5 | 3.5 | — | иконка/меню/mute/close→tray; `tray_contract.rs` |
| V-06 | Нет live stats/reconnect (базовая логика реконнекта в `session.rs` принята) | G1 | 3.33 | V-05 | RTT/потери/reconnect; `stats_contract.rs`, `session_contract.rs` |
| ST-08 | Нет settings-proto sync | G1 | 3.33 | — | local+server, миграции; `settings_proto.rs` |
| A-09 | Нет списка сессий | G1 | 3.0 | — | список/отзыв; `auth_sessions_contract.rs` |
| G-08 | Unread сервера статичен | G2+G1 | 3.0 | M-07 | бейджи/read_state; UI-тест |
| C-03 | Unread канала статичен | G2+G1 | 3.0 | M-07 | маркер/счётчик; UI-тест |
| C-04 | Шапка: backend пинов/веток реализован в Wave 6 (M-10-M-11, на ревью 13B); нет панелей pins/threads в UI | G2 | 3.0 | M-10/11 | панели/пустые состояния; UI-тест |
| M-10 | Backend хранения и REST-маршрутов пинов реализован в Wave 6 (на ревью Codex 13B); нет UI-панели и jump | G1+G2 | 3.0 | `PINS-THREADS-PROFILE-SPEC.md` §2 | список/jump; `pins_threads_contract.rs` (6) |
| MB-02 | Роли/цвета не из live | G1+G2 | 3.0 | L-02 | роль-цвет/hoist; UI-тест |
| MB-03 | Поповер профиля реализован (STUB-16): 4 состояния, ARIA, клавиатура; данные synthetic, действия не подключены | G2 | 3.0 | REST-профиль/права | карточка/роли/действия; `profile_popout.test.mjs` + `gui-spike-regressions.test.mjs` (11) |
| P-04 | Нет кастомного статуса | G1+G2 | 3.0 | P-05 | статус+activity; контракт-тест |
| P-05 | Нет смены своего статуса | G1+G2 | 3.0 | — | меню/IDENTIFY/UPDATE; контракт-тест |
| ST-02 | 38 тумблеров без эффекта; бэкенд устройств реализован в Wave 6 (V-07, на ревью 13B); нет связывания | G5+G2 | 3.0 | V-07 | устройства/проверка; `settings_contract.rs` + `devices_contract.rs` |
| ST-03 | Нет zoom/полного light | G2 | 3.0 | — | темы/density/zoom; UI-тест |
| N-01 | D-Bus не подключён | G5+G2 | 3.0 | — | notify/close/action; `notifications_contract.rs` |
| N-02 | Нет DND-подавления | G5+G2 | 3.0 | N-01 | DND/авто-idle; `dnd_contract.rs` |
| N-03 | Нет inbox | G2+G1 | 3.0 | — | список/jump; `inbox.test.mjs` |
| PL-02 | Автозапуск — только модель | G5 | 3.0 | — | install/uninstall; `autostart_contract.rs` |
| PL-03 | Deep links не подключены | G5 | 3.0 | — | open-url→route/один инстанс; `deep_link_contract.rs` |
| AT-01 | Backend слотов + tier-лимиты 10/50/100 МиБ приняты (B2a + TIER-FIX); нет `PUT` upload с докачкой `308`, прогресса и UI | G1+G2 | 3.0 | RE-1 остаток: коды 40005/413/429, сумма 500 МБ/1 ГиБ | upload→send; `attachments_contract.rs` (17) + `upload_contract.rs` |
| ST-04 | i18n-crate не подключён | G1+G2 | 2.0 | — | ru/en/plural/rich; `i18n_ui.test.mjs` |
| M-11 | Backend веток реализован в Wave 6 (на ревью Codex 13B); нет UI-панели активных/архивных веток | G1+G2 | 2.0 | `PINS-THREADS-PROFILE-SPEC.md` §3 | список/создание; `pins_threads_contract.rs` (6) |
| M-09 | Меню сообщения частично | G2 | 2.67 | — | 15 пунктов/права; UI-тест |
| MB-04 | members-search deferred | G1+G2 | 2.5 | **RE request/response** | поиск; `members_search_contract.rs` |
| V-09 | Нет Krisp | G5 | 2.33 | V-05 | RNNoise-совместимо; `krisp_contract.rs` |
| PL-06 | Idle только модель | G5 | 2.33 | — | inhibit/idle→status; `power_contract.rs` |
| V-10 | Нет DAVE/MLS | G1+G4 | 2.2 | V-05 | framing/keys/интероп; `dave_contract.rs` |
| S-01 | Нет камеры; выбор portal Camera vs v4l2 открыт (`D-2`) | G5 | 1.75 | V-05 | v4l2/portal/preview; `camera_contract.rs` |
| S-02 | Byte-level оракул шлюза `stream.rs` принят (STUB-27-CORE ACCEPT) + маршрутизация `STREAM_*` + дизайн `STREAM-REVIEW-PASS3` PASS (F-02..F-04 закрыты); capture/consent/UI нет | G5+G1 | 1.6 | V-03, PL-07 | consent→поток→stop; `stream_contract.rs` + `stream_gateway_contract.rs` (11) |
| S-03 | Нет зрителей/качества; B-08 блокирует wire-путь | G1+G5 | 1.4 | S-02 | rid/quality/декод; `video_rtp_contract.rs` |
| A-04 | Нет QR-входа | G1 | 1.67 | RE one-time | QR-сессия; `auth_qr_contract.rs` |
| A-05 | Нет passkey | G1/G5 | 1.25 | Linux WebAuthn | челлендж/подпись; `auth_passkey_contract.rs` |
| C-07 | QuickSwitcher есть, live-каналы не связаны | G2 | — | L-02 | каналы/гильдии из live; UI-тест |
| M-14 | Эмодзи-пикер статичен | G2 | — | — | кастомные эмодзи/стикеры; UI-тест |

## 4. P3 — backlog (14)

| ID | Gap | Owner | WSJF | Условие |
|---|---|---|---:|---|
| G-02 | Create server: endpoint не определён | G1+G2 | 1.33 | RE `GET\|POST /guilds` |
| G-07 | Роли/каналы/права сервера | G1+G2+G3 | 1.33 | design-first |
| C-05 | Create/edit канала | G2+G3 | 1.33 | design-first |
| M-12 | Forward/bookmark/reminder/TTS/report | G1+G2 | 1.33 | RE endpoint'ов |
| V-11 | Voice messages/clips/soundboard | G5+G2 | 2.0 | после voice MVP |
| S-04 | Activities | G1+G2 | 1.33 | после Go Live |
| ST-06 | Редактор keybinds | G2 | 2.0 | после i18n |
| ST-07 | Аккаунт/профиль | G1+G2 | 2.0 | RE полей |
| N-04 | Unity badge | G5 | 3.0 | после трея |
| PL-04 | Апдейтер: download/rollback | G5 | 1.75 | подпись/канал |
| PL-05 | Spellcheck UI | G5 | 1.33 | после i18n |
| PL-07 | Consent/разрешения захвата | G4b+G5 | 3.67 | **до S-02** (не после) |
| PL-08 | Window progress/flash/PiP | G5 | 2.0 | после трея |
| A-08 | Регистрация/сброс | PM | — | решение PM: вне desktop scope |

## 5. RE-гейты (до кода)

| Гейт | Что подтвердить | Блокирует | Как получить |
|---|---|---|---|
| **RE-1 Upload** | **снят** по маршруту, multipart-полям, лимитам tier (10/50/100 МиБ), локальным 413. Остаток: коды 40005/413/429, сумма 500 МБ/1 ГиБ, refresh-urls | AT-01/02/03 | live-захват своей загрузки на тестовом аккаунте |
| **RE-2 Invite/leave codes** | live-коды 200/204/4xx, TTL `received_installation_id`, owner-leave. Тело join `[S]`, enter `[L]` 204, leave `[S]` — уже есть. Бэкенд создания инвайтов принят (`invites.rs`), UI модалки реализован | G-03/G-04/G-06 (только типизация ошибок и список/отзыв) | live на тестовом сервере + запись запроса |
| **RE-3 Go Live** | **снят** (контракт `VOICE/STREAM-GATEWAY-CONTRACT.md` + byte-level оракул `stream.rs` принят): 4 серверных типа, op 18…22, поля, `stream_key`, 6 фаз; маршрутизация в `dispatch.rs`. Дизайн-ревью pass 3 подтверждает закрытие F-02..F-04. Остаток — rid/quality/viewer для S-03 | S-02/S-03 | live-стрим своего экрана, без чужих данных |
| RE-4 members-search | request/response, лимиты | MB-04 | live-поиск по своему серверу |
| RE-5 DND | правила подавления/уровни | N-02/N-05 | live-настройки + статус |
| RE-6 create server | endpoint/тело | G-02 | live-создание тестового сервера (PM) |

## 6. Окна и WIP

1. **Окно N0** (1): P0-01/P0-02/P0-03 + live-верификация A-01/A-06/A-07/M-02…M-08,
   G-01/C-02/M-07/L-01/M-06/M-13 (все — `live unverified` после batch). После него матрица получает
   `[L]`-строки; без окна статус остаётся `live unverified`.
2. **Окно N1** (1, после RE-2): join/leave/invite на тестовом сервере; проверка G-03/G-03a/G-04/G-06.
3. **Окно Voice** (1): два тестовых клиента, запись только своего микрофона; V-01…V-08.
4. **Окно Stream** (1, после RE-3 — контракт уже снят): свой экран/камера, consent, stop; S-02/S-03.
5. Параллельно (Wave 6 завершена): Batch 13A/13B переданы на Codex-ревью (P-03 UI, STUB-05 UI, M-08 UI, M-10/M-11 BE, V-07 BE).
6. Не запускать исходящую часть S-02 (capture, op 18/19) и V-10 до зелёного voice MVP
   (NETWORK-ROADMAP §10); входящая маршрутизация `STREAM_*` и byte-level оракул шлюза уже в коде.

## 7. Проверка сходимости (2026-10-30, после `PARITY-SYNC-09`)

- Все 91 ID матрицы присутствуют в backlog ровно один раз: P0 5 (§1) + P1 32
  (§2.1 9 + §2.2 14 + §2.4 9) + P2 40 (§3) + P3 14 (§4).
- Матрица: implemented/verified 22, partial 39, absent 30, unknown 0, итого 91;
  приоритеты 5/32/40/14 — совпадают с backlog.
- §2.4 — принятые строки с остатками: в очередь не входят, но и `parity-ok` не дают
  (нет E3, live-окна не было).
- Каждый P0/P1 имеет owner, DoD и тест.
- Ни один контракт уровня `[A]` не назначен к коду без RE-гейта; `[L]` G-03a и `[S]` join/leave
  уже закодированы и приняты, live-коды — только RE-2. RE-1 и RE-3 сняты по контракту,
  их остатки — коды загрузки и rid/quality зрителя.
- `[M]`-строки (V-08 speaking, G-05 заглушки, STUB-16 поповер, ST-01 подпункты, N-03) паритетом не считаются.
