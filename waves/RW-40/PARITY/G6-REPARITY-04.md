# G6-REPARITY-04 — финальная матрица RW-40 (после L-02-UI-FIX, M-07-BE3 и QA-гейта)

Дата: 2026-10-24, снимок **15:56 +05**. HEAD `0239543`, дерево **dirty** (103 изменённых
/untracked пути; `rust/gui-spike/**` и `discord-core/src/gateway/**` вне git).
Воркер: **G6 Parity** (read-only). PM: DeepSeek.

Режим: единственная запись — этот файл. `rust/**`, `ui/**`, `tests/**`, `scripts/**`, гейт,
e2e, браузер, CDP, сеть, Discord, live, `cargo`, `npm`, `node --test`, мутации —
**не запускались и не менялись**. `PARITY-MATRIX.md`, `GAP-BACKLOG.md`, `QUALITY/BUGS.md`,
`QUALITY/QA-MATRIX-02.md`, `MERGE-MANIFEST.md`, `TASKS.md`, `QUEUE.md`, `STATE.md`,
`waves/db.jsonl`, `TDD-PLAN.md` — **не** менялись.

> **Путь.** В ТЗ указаны корневые `PARITY/`, `PM/`, `QUALITY/`. Фактическое дерево —
> `waves/RW-40/{PARITY,PM,QUALITY}`; отчёт записан туда. `QUALITY/SEC-L02.md` лежит
> в корне (`/QUALITY/SEC-L02.md`).

Метки: **[V]** — проверено в этом проходе (grep/чтение/md5/JSON/awk);
**[R]** — из отчёта карточки, лога writer'а, гейта или матрицы.
**Ни один live-факт здесь не заявляется: E3 = 0.**

---

## 0. Резюме

1. **Предпосылка ТЗ «после L02 UI fix, M-07 и QA гейта» выполнена по коду, но не по приёмке.**
   К 15:54 оба фикса приземлились с red/green-логами и отчётами
   (`PM/l02-ui/FIX-REPORT.md` 15:54:32, `PM/m07-be3/REPORT.md` 15:54), но **ни один
   не имеет `ACCEPT.md` и повторного review**, а гейт на этом дереве **не запускался**.
   Последний worker-гейт — `10:22:26Z` (15:22), release — `09:37:37Z` (14:37).
2. **Ни одна строка матрицы не сдвинулась.** Счётчики (независимый awk-оракул по
   `PARITY-MATRIX.md`): **91 строка, implemented 21 / partial 36 / absent 34 / unknown 0**;
   P0 5 / P1 32 / P2 40 / P3 14.
3. **Strict-recheck (наш класс, не PM): 15 / 42 / 34** — те же 6 понижений
   (`G-01`, `C-02`, `C-07`, `M-06`, `M-07`, `M-13`). `verified (E3) = 0`.
4. **`parity-ok` строк — 0 из 91.** Любая запись `implemented` = «код + offline-тесты»,
   не паритет: live-окна не было.
5. **L-02-UI-FIX: готово к повторному review, приёмки нет.**
   `projectReady` теперь применяет пустой READY как полный снапшот
   (`dispatch-model.js:515-517`) **[V]**; `duplicate` требует `store.source==="dispatch"`,
   поэтому **первый** пустой READY после boot не теряется **[V]** (FIX-REPORT §1);
   `rail.applyGuilds([])` вызывает `renderEmpty()` (`app.js:626-632`, `renderEmpty:546`)
   **[V]**; self-ID — только строка или `Number.isSafeInteger`
   (`asSnowflake`, `dispatch-model.js:81-85`; `app.js:1194-1196`) **[V]**.
   Тесты: B16/B16b/B17/C15/C15b/C16 **[V]**. Логи: red 15:52:31; второй red
   `fix-red2` 15:53:52 (B16b/C15b — «первый пустой READY на чистом store»); green
   **43/43** 15:54:00; targeted **347/347** 15:54:06; узкий tsc пуст 15:54:04 **[R]**.
   Нет: re-review Codex, `ACCEPT.md`, полного `tsc` и гейта.
6. **M-07-BE3: badge-поля спроецированы, UI их не читает.** `ReadStateEntry` теперь
   `{channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount,
   badgeCount, lastAckedId, readStateType}` (`data.rs:539-553`) **[V]**; `lastAckedId` —
   decimal-строка (JS-safe integer не вмещает `1294268550826950656`) **[V]**.
   Логи: red 15:52:38 (compile 15+8), targeted 30+15, gui-final EXIT:0, fmt/clippy EXIT:0
   **[R]**. **`flags`/`badgeCount`/`readStateType` в `ui/**` = 0 вхождений** **[V]** →
   M-07 остаётся `partial`, badge на экране не появился.
7. **QA-гейт: патч есть, прогона нет.** `failOnSkips`+`minPass`+split live в
   `scripts/gate.mjs` (`89c67ad0…`, 15:44) **[V]**; meta-тесты 17/17 **[R]**.
   Но `.gate/worker-…T10-22` не содержит полей `e2e`/`acceptance` **[V]**, т.е. снят
   старым гейтом; `typecheck` (16 ошибок `tsc` в `dispatch_ui.test.mjs`) и `meta`
   (`plan-sync`: 2 файла без строк в `TDD-PLAN.md §8`) остаются красными **[R/V]**.
8. **Принято 2 среза** (`PM/l02/ACCEPT.md` — backend-срез L-02; `PM/c02-be/ACCEPT.md`).
   Без `ACCEPT.md` — 10 срезов окна: l02-core, l02-ui, c02-before, p03-ui, g05-ui,
   m07-be2, m07-be3, m07-ui, search-fix, gate-skips **[V]**
   (`find waves/RW-40/PM -name 'ACCEPT*.md'` → 2 файла).
9. **C-02 older UI по-прежнему `absent`:** backend `before`/`latest`/`hasOlder` готов,
   `chat.js` не передаёт `before`/`position:"latest"`/`olderCursor` (grep 0) **[V]**.
10. **P-03: `TYPING_START` не форвардится** — `FORWARDED_DISPATCHES: [&str; 6]`
    (`net.rs:113-120`), `TYPING` в `net.rs` — 0 вхождений **[V]**. Индикатор offline есть
    (20 тестов), live-индикатор невозможен by design.
11. **G-05 = 2/10** (leave, copy-id; invite `capability=false`) **[V]**; спека rev.4
    (15:48) — docs-only, PM не принял **[V]**.
12. **G-01b `absent`**: `UiSettings` = ровно 5 полей (`settings_store.rs:137-148`) **[V]**,
    `last_guild_id`/`lastGuildId` нет ни в Rust, ни в UI **[V]**.
13. **Вердикт: `deferred`** для parity; **merge — `block`** до трёх условий §5.
    Не `parity-ok` (E3=0, L-02/M-07 partial, 0 accepted), не `block` по продукту
    (P0-дефектов нет; обе красные фазы гейта — не продукт).

---

## 1. Что реально приземлилось между 15:49 и 15:54

`G6-REPARITY-03` (15:49) зафиксировал состояние «фиксы в полёте». За 5 минут до этого
снимка оба writer'а дописали код и логи. Это **не приёмка** и **не повод двигать матрицу**,
но это уже не «ничего не сделано» — фиксирую раздельно: код/логи/приёмка.

| Срез | Что в дереве | Логи (red → green) | Чего нет |
|---|---|---|---|
| **L-02-UI-FIX** | `dispatch-model.js` `37dacc61…` (15:53:58), `app.js` `1ac5c369…` (15:52:56), `dispatch_ui.test.mjs` `f2806f3f…` (15:53:49) **[V]** | red `fix-red.log` 15:52:31; 2-й red `fix-red2.log` 15:53:52 (B16b/C15b — первый пустой READY); green **43/43** `fix-green.log` 15:54:00; targeted **347/347** 15:54:06; узкий `tsc` пуст 15:54:04; `FIX-REPORT.md` 15:54:32 **[R]** | re-review Codex; `ACCEPT.md`; полный `tsc` (проектный); гейт на дереве |
| **M-07-BE3** | `data.rs` `8013271d…` (15:52:57) — badge-поля; `read_state_contract.rs` `fde688f3…`, `data_contract.rs` `e31c9127…` **[V]** | red `red.log` 15:52:38 (compile: 15+8); green targeted **30+15** (final 15:53:39); gui-final EXIT:0 15:53:53; `read-state-ui` EXIT:0; fmt/clippy EXIT:0; `REPORT.md` 15:54 **[R]** | review/ACCEPT; **UI-потребитель badge** (`ui/**`: `flags`/`badgeCount` = 0) **[V]**; `CONTRACTS/read-state.md` устарел (сам REPORT §риски) |
| **QA-GATE-SKIPS** | `gate.mjs` `89c67ad0…` (15:44), `test-policy.test.mjs` `62d7c60b…` **[V]** | red 5/17 → green 17/17 **[R]** | прогон `gate:worker`/`gate:release` на новом гейте; зелёные `typecheck`/`meta`; ACCEPT |
| **QA-M2-04** (CI, в полёте) | `ci.yml` `608e2683…` (15:52): job `qa-browser`, pinned Chromium `131.0.6778.85`, preflight `CHROME_BIN`, meta `ci-browser`, fixture-e2e без `live-*` **[V]** | — (Actions не запускался) | прогон job'а; ACCEPT |
| **QA-M2-02** (harness, в полёте) | `tests/helpers/fake-tauri.mjs` (15:56), `m2-scenarios.json` (15:55), `tests/meta/e2e-required.test.mjs` (new) **[V]** | — | 7 `tests/e2e/m2-*.e2e.test.mjs` ещё нет; red/green нет |

**Процессный факт [V]:** одновременно работали 4 code-writer'а (`L02-UI-FIX` G2,
`M-07-BE` G1, `QA-M2-02` G4, `QA-M2-04` G5) + 2 read-only (`SEC-L02-FINAL`, `G3-ST-01`).
Устав §5.3 — «≤2 кодовых воркера». Дерево менялось **во время** этого прохода
(`dispatch_ui.test.mjs` — три раза за 15:52–15:54). Все md5 §7 — снимок, не фиксация.

---

## 2. Матрица 91 функции (снимок 15:56)

Колонки: **Матрица** — состояние `G6-SYNC-01` (14:34), владелец PM; **Recheck-04** —
strict-класс этого прохода; **Δ** — изменение strict-класса относительно `G6-REPARITY-03`.
`impl*` = implemented offline, E3 нет; `impl*↓` = de facto partial (§3).
`—` в Δ значит «класс не двигался»; содержание остатка при этом могло измениться (помечено
в тексте).

### 2.1 Auth / session (9)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| A-01 | Вход по логину/паролю | P0 | implemented (live unverified) | impl* | — | live-вход N0-01 не выполнен → E3 нет |
| A-02 | MFA TOTP/SMS/backup/password | P1 | implemented (live unverified) | impl* | — | 5 методов offline; live-прогона нет |
| A-03 | Captcha-гейт | P1 | implemented (offline) | impl* | — | live sitekey/flow `[A]` |
| A-04 | QR / one-time login | P2 | absent (UI disabled) | absent | — | протокол/поллинг не сняты |
| A-05 | Passkey / WebAuthn | P2 | absent (UI disabled) | absent | — | Linux-WebAuthn нет |
| A-06 | Восстановление сессии | P1 | implemented (keyring `[A]`) | impl* | — | live restore/битый keyring не проверены |
| A-07 | Logout / инвалидация | P1 | implemented (live unverified) | impl* | — | live stop/401 после logout нет |
| A-08 | Регистрация/сброс пароля | P3 | absent | absent | — | решение PM: вне desktop scope |
| A-09 | Сессии / logout others | P2 | absent | absent | — | нет экрана и вызова |

### 2.2 Guilds (9)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| G-01 | Рейл: список/выбор/переключение | P1 | implemented (live unverified) | **impl*↓** | — | READY 7 гильдий + `guild-rail` (8); `lastGuildId` нет → **G-01b absent** **[V]**; L-02-UI-FIX закрыл «пустой READY» offline (код 15:52), приёмки нет |
| G-02 | Добавить/создать сервер | P3 | absent | absent | — | endpoint `GET\|POST /guilds` не определён |
| G-03 | Вход по инвайту | P1 | implemented (live unverified) | impl* | — | `invites_contract` (14) + `join-server` (24); live-коды/TTL `[A]` |
| G-03a | Войти из preview/lurker | P1 | implemented (live unverified) | impl* | — | backend `[L]` 204×2; live 204 из UI не снят |
| G-04 | Выход с сервера | P1 | implemented (live unverified) | impl* | — | confirm + `{guildId, lurking}`; live-коды `[A]` |
| G-05 | Меню сервера | P1 | partial: **активны 2 из 10** | partial | — | `SERVER_MENU_SPEC` 10 пунктов, активны `guild-leave`/`guild-copy-id`, invite `data-capability-invite="false"` **[V]**; спека rev.4 (15:48) docs-only, не принята |
| G-06 | Инвайты: создать/список/отозвать | P2 | absent | absent | — | API/UI нет; коды `[A]` |
| G-07 | Роли/каналы/права сервера | P3 | absent | absent | — | design-first |
| G-08 | Unread/mention сервера | P2 | partial: статичный `is-unread` | partial | — | `badgeCount` теперь в IPC (M-07-BE3 15:52) **[V]**, но UI его не читает (0) → счётчика нет |

### 2.3 Channels / navigation (6)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| C-01 | Дерево: категории, text/voice/forum | P1 | partial: дерево из READY | partial | — | типы 0/2/4/5/15; нет open voice/forum, collapse |
| C-02 | Переключение канала + история | P1 | implemented (live unverified) | **impl*↓** | — | backend `messages_page(…, before?, position?, …)`, `hasOlder`/`olderCursor` (15:28) **[V]**; `chat.js` **0** вхождений `before`/`position:"latest"`/`olderCursor` **[V]**; live-истории нет |
| C-03 | Unread/mention канала | P2 | partial: статичный класс | partial | — | `data-mentions` из READY работает (M-07-UI), badge/ack-счётчик нет |
| C-04 | Шапка: пины/ветки/участники/поиск | P2 | partial | partial | — | поиск/участники есть; pins/threads stub |
| C-05 | Создание/редактирование канала | P3 | absent | absent | — | UI не снят |
| C-07 | QuickSwitcher `Ctrl+K` | P2 | implemented (B6) | **impl*↓** | — | свитчер читает `dataset.guild`, а продукт пишет `data-guild-id` (`quick-switcher.js:67,144,150`) **[V]** → выбор no-op; `openChannel`/`messages_page` не вызывает |

### 2.4 Messages (14)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| M-01 | История/пагинация | P1 | partial: fixtures + PageStore | partial | — | forward-paging есть; **older/tail UI нет**; live-курсоров нет |
| M-02 | Отправка | P0 | implemented (live unverified) | impl* | — | live send/401/429 → окно N0-02 |
| M-03 | Редактирование | P1 | implemented (live unverified) | impl* | — | тело edit `[A]` |
| M-04 | Удаление | P1 | implemented (live unverified) | impl* | — | live 403/tombstone не проверены |
| M-05 | Ответ (reply) | P1 | implemented (live unverified) | impl* | — | `message_reference`; live-поле `[A]` |
| M-06 | Реакции | P1 | implemented (live unverified) | **impl*↓** | — | ADD/REMOVE в `FORWARDED_DISPATCHES` (6) **[V]**; L-02-UI-FIX landed, но ACCEPT нет; live PUT/DELETE/429 нет |
| M-07 | Read state / ack | P1 | implemented (live unverified) | **impl*↓** | — | ack/`lastViewed`/unread-полоса есть; IPC отдаёт `flags`+`badgeCount`/`lastAckedId`/`readStateType` (M-07-BE3) **[V]**, **UI использует 0 полей badge/flags** **[V]** |
| M-08 | Markdown/embeds/mentions | P1 | partial: только embeds | partial | — | нет markdown/mentions/emoji |
| M-09 | Контекстное меню (15) | P2 | partial: 4 команды | partial | — | forward/bookmark/reminder/TTS/report stub |
| M-10 | Пины | P2 | absent (stub) | absent | — | нет панели/загрузки |
| M-11 | Ветки | P2 | absent (stub) | absent | — | нет списка/создания |
| M-12 | Forward/bookmark/reminder/TTS/report | P3 | absent | absent | — | endpoints `[A]` |
| M-13 | Поиск по серверу | P1 | implemented (4/7) | **impl*↓** | — | SEARCH-FIX: `data-guild-id` + regression на реальном `index.html` (2/2, red 0/2) **[R]**; ACCEPT/браузера/live нет |
| M-14 | Эмодзи-пикер | P2 | implemented (static) | impl* | — | статичный набор; кастомные/стикеры нет |

### 2.5 Members (4)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| MB-01 | Список: роли + статусы | P1 | partial: 24k synthetic | partial | — | L-02-UI даёт live-строки панели (`data-user-id`), но **6 групп нет**, `#member-list` не live |
| MB-02 | Роли/цвета/права | P2 | partial: synthetic | partial | — | live-ролей/цветов нет |
| MB-03 | Профиль-поповер | P2 | partial: stub aria | partial | — | нет поповера/полей |
| MB-04 | Полный поиск участников | P2 | absent (deferred) | absent | — | RE-4 request/response |

### 2.6 Presence / typing (5)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| P-01 | Статусы online/idle/dnd/offline | P1 | partial: synthetic + core | partial | — | `PRESENCE_UPDATE` → панель offline (L-02-UI-FIX) **[V код]**; live-статусов нет |
| P-02 | Обновления PRESENCE_UPDATE | P1 | partial: reducer, UI не связан | partial | — | UI-подписчик появился offline, приёмки нет; live нет |
| P-03 | Индикатор набора + POST /typing | P1 | partial: backend, UI offline | partial | — | `send_typing`+индикатор (20 тестов) **[R]**; **`TYPING_START` не форвардится**: `FORWARDED_DISPATCHES` = 6, `TYPING` в `net.rs` = 0 **[V]** |
| P-04 | Кастомный статус/активность | P2 | absent | absent | — | нет редактора/отображения |
| P-05 | Смена своего статуса | P2 | absent | absent | — | нет меню/IDENTIFY/UPDATE |

### 2.7 Voice (11)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| V-01 | Вход/выход из voice | P1 | absent (crate only) | absent | — | контракт `V01-SESSION-CONTRACT.md` (15:46) готов, кода нет |
| V-02 | RTC-сигнализация | P1 | partial: парсер/тесты | partial | — | нет live-WS/сессии |
| V-03 | UDP discovery/keepalive | P1 | partial: примитивы | partial | — | нет live-UDP |
| V-04 | SRTP | P1 | partial: KDF/AEAD тесты | partial | — | нет live encrypt/decrypt |
| V-05 | Opus/джиттер/DTX | P1 | partial: jitter, нет Opus | partial | — | нет encode/decode |
| V-06 | Статистика/reconnect/регионы | P2 | partial: policy/stats | partial | — | нет live RTT/потерь |
| V-07 | Устройства/mute/deafen/PTT | P1 | absent (settings fake) | absent | — | нет устройств/эффекта |
| V-08 | Голосовой UI | P1 | partial: панель на fake store | partial | — | реального voice-backend нет |
| V-09 | Шумоподавление Krisp | P2 | absent | absent | — | нет модели/интеграции |
| V-10 | DAVE/MLS E2EE | P2 | absent | absent | — | нет MLS |
| V-11 | Voice messages/clips/soundboard | P3 | absent | absent | — | контракт не снят |

### 2.8 Video / Go Live (4)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| S-01 | Камера: preview/enable | P2 | absent | absent | — | нет захвата/превью |
| S-02 | Screen share / Go Live | P2 | absent | absent | — | нет capture/сигнализации (RE-3) |
| S-03 | Зрители/качество/simulcast | P2 | absent | absent | — | нет RTP-видео/декода |
| S-04 | Activities | P3 | absent | absent | — | контракт запуска `[A]` |

### 2.9 Attachments (3)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| AT-01 | Загрузка файла | P2 | absent (кнопка disabled) | absent | — | **blocked**: метод/поля/лимиты (RE-1) |
| AT-02 | Drag&drop/paste | P2 | absent | absent | — | нет триггеров |
| AT-03 | Рендер вложений | P2 | absent (только embeds) | absent | — | нет схемы attachment |

### 2.10 Settings (8)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| ST-01 | Навигация настроек | P2 | partial: 18 разделов, 3 панели | partial | — | 63 DOM-строки; подпункты 17 разделов `[A]`; G3-ST-01 в работе |
| ST-02 | Голос/видео: 38 тумблеров | P2 | partial: нет эффекта | partial | — | устройств/аудио нет |
| ST-03 | Внешний вид: тема/плотность/zoom | P2 | partial | partial | — | zoom/полного light нет |
| ST-04 | Язык/время + i18n-рантайм | P2 | partial: crate не подключён | partial | — | нет `t()`/каталогов в UI |
| ST-05 | Уведомления: настройки | P2 | partial: панель-заглушка | partial | — | нет полей/эффекта |
| ST-06 | Горячие клавиши | P3 | absent | absent | — | нет редактора |
| ST-07 | Аккаунт/профиль | P3 | absent | absent | — | поля не сняты |
| ST-08 | Персистентность/синхронизация | P2 | partial: локальный файл `[A]` | partial | — | **5 полей**, `version:1` (`settings_store.rs:137-148`) **[V]**; settings-proto sync нет |

### 2.11 Notifications / DND (5)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| N-01 | Системные уведомления | P2 | partial: D-Bus crate не подключён | partial | — | нет вызова из UI |
| N-02 | DND/режим концентрации | P2 | partial | partial | — | нет подавления |
| N-03 | Inbox/упоминания | P2 | absent (кнопка disabled) | absent | — | UI/счётчики не сняты |
| N-04 | Бейдж/прогресс | P3 | absent | absent | — | нет Unity badge |
| N-05 | Переопределения сервер/канал | P2 | absent | absent | — | нет меню/уровней |

### 2.12 Platform (8)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| PL-01 | Трей/фон | P2 | absent | absent | — | нет трея |
| PL-02 | Автозапуск/старт свёрнутым | P2 | partial: модель `.desktop` | partial | — | нет записи/UI |
| PL-03 | Deep links + single instance | P2 | partial: парсер `discord://` | partial | — | нет wiring/second-instance |
| PL-04 | Апдейтер | P3 | partial: manifest/plan | partial | — | нет скачивания/замены |
| PL-05 | Spellcheck | P3 | partial: hunspell FFI | partial | — | нет UI/словарей |
| PL-06 | Питание/idle | P2 | partial: power/idle crate | partial | — | порог idle `[A]` |
| PL-07 | Consent/разрешения захвата | P3 | absent | absent | — | consent-flow не снят |
| PL-08 | Окно: badge/progress/flash/PiP | P3 | absent | absent | — | нет вызовов |

### 2.13 Live / connectivity (5)

| ID | Функция | P | Матрица | Recheck-04 | Δ | Остаток / evidence |
|---|---|---|---|---|---|---|
| L-01 | Reconnect/offline state в UI | P1 | implemented (live unverified) | impl* | — | `#net-status` + retry; live resume/4013/4014 нет |
| L-02 | Dispatch → UI | P1 | partial: reducer/core, UI на fixtures | partial | — | **core**: P1/P2 фикса закрыты кодом (`ws.rs` 15:43), ACCEPT нет; **UI**: fix landed 15:52, re-review/ACCEPT нет; live-доставки нет |
| P0-01 | Live READY | P0 | partial (offline) | partial | — | E3 нет |
| P0-02 | Live send | P0 | partial (offline) | partial | — | E3 нет |
| P0-03 | Секреты не в логах | P0 | implemented (offline) | impl* | — | SEC-L02: redaction по ключам корректна **[V]**; live-грепа нет |

### 2.14 Сходимость

| Класс | Матрица | Strict recheck-04 | Δ к REPARITY-03 |
|---|---:|---:|---:|
| verified (E3) | 0 | **0** | 0 |
| implemented (offline) | 21 | **15** | 0 |
| partial | 36 | **42** | 0 |
| absent | 34 | **34** | 0 |
| unknown | 0 | **0** | 0 |
| **Итого** | **91** | **91** | 0 |

**Условный сдвиг (не применён, требует ACCEPT + гейта):** если `L-02-CORE` и `L-02-UI`
примут как landed, строка L-02 переходит `partial → implemented (offline)`, и strict
становится **16 / 41 / 34**. `M-07` при этом **не двигается**: badge-поля есть в IPC,
но UI их не показывает (§4). Ни в одном варианте `verified` не появляется — live нет.

Пограничные: `G-01`/`C-02` при отдельной трассировке остатков (`G-01b`, `M-01`) дают
**17 / 40 / 34**. Все варианты сходятся к 91.

### 2.15 Независимый оракул (markdown, не реализация)

```bash
cd waves/RW-40/PARITY
sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3);
  gsub(/^ +| +$/,"",app); gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app);
  a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
```

Фактический вывод (15:55, этот проход):

```text
app absent 34
app implemented 21
app partial 36
prio P0 5 / P1 32 / P2 40 / P3 14
total 91
```

Сверка ID: 91/91, без пропусков и дублей; домены:
`A 9 · G 9 · C 6 · M 14 · MB 4 · P 5 · V 11 · S 4 · AT 3 · ST 8 · N 5 · PL 8 · L 2 + P0 3`.

---

## 3. Шесть `implemented`, которые не end-to-end

Понижение не меняет матрицу (владелец — PM); это флаг «функция + тесты ≠ end-to-end».

| ID | Почему не end-to-end | Evidence | Что вернёт `implemented` |
|---|---|---|---|
| **C-02** | backend older/latest готов, UI листает только вперёд | `data.rs:13-35,1329-1393` **[V]**; `chat.js`: 0 `before`/`olderCursor` **[V]** | `C-02-UI-OLDER` + ACCEPT + live |
| **C-07** | свитчер не вызывает `openChannel`; гильдии ищутся по `dataset.guild`, которого продукт не ставит | `quick-switcher.js:67,144,150` **[V]**; рейл — `data-guild-id` (`app.js:513`) **[V]** | переход на `data-guild-id` + вызов `openChannel` + тест |
| **M-06** | входящий путь есть offline, но L-02-UI не принят; live PUT/DELETE/429 нет | ADD/REMOVE в `FORWARDED_DISPATCHES` **[V]** | L-02-UI ACCEPT + live-реакции |
| **M-07** | badge-поля спроецированы, но UI их не читает; `flags` в `ui/**` = 0 | `data.rs:539-553` **[V]**; `ui/**` grep `flags`/`badgeCount` = 0 **[V]** | M-07-UI-badge (потребление) + live ack |
| **M-13** | false-green закрыт offline, но не принят и не подтверждён браузером | SEARCH-FIX 2/2, red 0/2 **[R]** | ACCEPT + browser-e2e сценария 3 |
| **G-01** | `lastGuildId` не сохраняется — после рестарта выбор теряется | `settings_store.rs:137-148` (5 полей) **[V]** | `G-01b` (поле + миграция + restore-тест) |

Не понижались: `M-14` (статичный пикер — это и есть функция), `A-03`/`A-06`
(`[A]`-оговорки), `P0-03` (offline-redaction — сама функция), `L-01` (UI есть,
live — только подтверждение).

---

## 4. Блокеры: закрыть / оставить

Статусы: **`CLOSED`** — есть приёмка + evidence на замороженном дереве;
**`FIXED-NOT-ACCEPTED`** — код и логи есть, приёмки/гейта нет;
**`OPEN`** — функции/пути нет. Ни один блокер **не** становится `CLOSED` от факта
«код появился» (`TEST-POLICY.md`, `MERGE-POLICY` §2).

| # | Блокер | Статус 15:56 | Evidence | Кто/что закрывает |
|---:|---|---|---|---|
| 1 | **L-02 CORE** (потеря op0 + оракул `token`) | **`FIXED-NOT-ACCEPTED`** | `ws.rs:653-655`: `let parsed = pipe.push_binary(...); flush_dispatches(...); let effects = parsed?;` **[V]**; `net_contract.rs` oracle `token`/`access_token`/`refresh_token` **[V]**; red 0/1 → green 14 / 23 **[R]**; md5 = `FIX-REPORT §4` **[V]** | `REVIEW-L02-CORE-FIX` (Codex) + `ACCEPT.md` + гейт |
| 2 | **L-02 UI** (пустой READY, unsafe self-ID) | **`FIXED-NOT-ACCEPTED`** | `projectReady` применяет пустой снапшот `dispatch-model.js:515-517` **[V]**; `asSnowflake` safe-integer `:81-85` + `app.js:1194-1196` **[V]**; B16/B16b/B17/C15/C15b/C16 **[V]**; `FIX-REPORT.md` 15:54:32, green 43/43, targeted 347/347, tsc пуст **[R]** | `REVIEW-L02-UI-02` (Codex) + `ACCEPT.md` + гейт |
| 3 | **C-02 older UI** (открытие на последнем + скролл вверх) | **`OPEN`** | IPC готов (`MessagesPosition`, `before`, `hasOlder`/`olderCursor`) **[V]**; UI-ветки нет **[V]** | `C-02-UI-OLDER` (G2) |
| 4 | **P-03** (входящий индикатор) | **`OPEN`** (исходящее — `FIXED-NOT-ACCEPTED`) | `send_typing`+индикатор 20/20 **[R]**; `FORWARDED_DISPATCHES` = 6 без `TYPING_START`, `TYPING` в `net.rs` = 0 **[V]** | `TYPING_START-FORWARD` (G1 `net.rs`+контракт → G2) |
| 5 | **G-05** (меню 10/10) | **`OPEN`** | активны 2/10, invite `capability=false` **[V]**; спека rev.4 docs-only | PM принимает rev.4 → `G-05-REST` (G2); 10/10 offline без waiver недостижимо (RE-2) |
| 6 | **M-07** (badge/flags на экране) | **`OPEN`** по функции, **`FIXED-NOT-ACCEPTED`** по IPC | `data.rs` badge-поля 15:52 **[V]**; `ui/**` → 0 `flags`/`badgeCount` **[V]**; BE3 green **[R]** | `M-07-BE3` ACCEPT + `M-07-UI-badge` (потребление, G2) |
| 7 | **G-01b** (`lastGuildId`) | **`OPEN`** | `UiSettings` = 5 полей **[V]**; `last_guild_id`/`lastGuildId` = 0 **[V]** | `G-01b` (G1 поле+миграция → G2 restore) |
| 8 | **e2e** (browser-контур) | **`OPEN`** (гейт/сценарии) + **`FIXED-NOT-ACCEPTED`** (CI) | release 14:37: e2e `tests 4 / pass 0 / skipped 4` **[V]**; `failOnSkips`+`minPass` в `gate.mjs` **[V]**, прогона нет; 7 `m2-*` ещё нет (harness `fake-tauri` 15:56 и `e2e-required` появились) **[V]**; Chrome локально нет; CI: 2 Rust-job + новый `qa-browser` (15:52, pinned Chromium, preflight) **[V]** | `QA-M2-02` (7 сценариев) + прогон `qa-browser` + `gate:release` на новом гейте |
| 9 | **live** (E3) | **`OPEN`** | E3 = 0/91; окна не было; `SEC-L02.md §5` прямо перечисляет, что не доказано **[V]** | окно N0 (P0-01 → P0-02 → grep секретов), затем N1/T3 |

Дополнительно к девяти, **не снимается** и не входит в матрицу:

| Finding | Статус | Почему важно |
|---|---|---|
| **SEC-L02-01** (P1): дедуп `s` не сбрасывается на `connect` | **`OPEN`** | `net.rs` md5 `e13d4e8c…` не менялся **[V]**; после relogin/reconnect READY уходит в `duplicates` → UI живёт прошлой сессией |
| `typecheck`: 16 `tsc` в `tests/unit/dispatch_ui.test.mjs` | **`RED` (не подтверждён закрытым)** | Логи `16` ошибок на файле `6118760…` **[R]**; в новом файле 66 JSDoc-тегов, узкий tsc пуст **[V]**, но полный `tsc -p tsconfig.json` G6 не запускал |
| `meta`: `plan-sync` | **`RED`** | `TDD-PLAN.md` не содержит `dispatch_ui.test.mjs` и `search-remote-real-index.test.mjs` (grep 0) **[V]**; 76/77 **[R]** |
| **G6-D-03** evidence в `BUGS.md:90` | **`OPEN` (документ)** | строка всё ещё перечисляет `ready` как «не вызывается»; `ready` вызывается (`app.js` invoke) **[V]**; `enter_guild` — alias `join_guild` |
| `MERGE-MANIFEST §4` | **устарел** | «P-03 typing UI отсутствует», «C-02 — только первая страница», L-02/search-fix не упомянуты |

---

## 5. Вердикт и три условия merge

### 5.1 `deferred` для parity; merge — `block` до условий

| Вопрос | Ответ |
|---|---|
| parity-вердикт | **`deferred`** — ни одна строка не `verified`, ни одна не ухудшилась |
| можно ли merge сейчас | **нет — `block`**: гейт на дереве красный (`typecheck`, `meta`), accepted set неполон |
| продуктовый block (P0/регресс/вымысел) | **нет** — P0-дефектов не найдено |

**Почему не `parity-ok`:**
1. **E3 = 0 из 91** — ни одна функция не подтверждена live; окна N0 не было.
2. **L-02 не end-to-end по приёмке**: core ждёт re-review+ACCEPT, UI — re-review+ACCEPT;
   от L-02 зависят M-06/P-01/P-02/P-03/MB-01.
3. **10 срезов лежат непринятыми** (l02-core, l02-ui, c02-before, p03-ui, g05-ui,
   m07-be2, m07-be3, m07-ui, search-fix, gate-skips) — статус кода опережает приёмку
   на 5–7 карточек; приняты только `l02` (backend) и `c02-be`.
4. **6 `implemented` de facto partial** (§3).
5. **G-05 = 2/10**, спека rev.4 не принята; 10/10 offline недостижимо без waiver.
6. **e2e = 0 pass / 4 skip** в последнем release; браузера и 7 сценариев нет.
7. **P0-01/02 live не сняты.**

**Почему не `block` по продукту:** P0 = 0 (`SEC-L02.md §0`), redaction по ключам корректна,
вымышленных endpoint/полей нет (G6-BATCH-MERGE §3 PASS), обе красные фазы — книжная
(`TDD-PLAN §8`) и тестовая (типизация нового теста), не `ui/**`/`rust/**`-поведение.

### 5.2 Три условия merge (все обязательны)

| # | Условие | Проверка (объективная) |
|---:|---|---|
| **1** | **Gate-green на замороженном дереве.** Writer'ы остановлены, md5 не меняется между прогоном и приёмкой | `gate:worker` и `gate:release` → `acceptance.accepted === true`, `e2e.fixture === "met"`, `skipped === 0`; `typecheck`/`meta` зелёные; в JSON есть `attempts[]` и `ignored[]` |
| **2** | **Accepted set полон и привязан к md5.** L-02-CORE (после re-review) и L-02-UI (после fix+re-review) + SEARCH-FIX и QA-GATE-SKIPS приняты; остальные partial не выдаются за готовые | `ACCEPT.md` у каждой карточки окна; md5 в ACCEPT = md5 в дереве; ни одна карточка не входит в merge «по отчёту» |
| **3** | **Реестры честные.** Открытые P1 закрыты или письменно waiver'нуты; evidence исправлен; матрица синхронизирована с accepted-кодом | `SEC-L02-01` closed/waiver; `BUGS.md` G6-D-03 переписан; `PARITY-MATRIX`/`GAP-BACKLOG`/`MERGE-MANIFEST §4` обновлены; нет строки `implemented` без offline-пути |

**Merge ≠ parity-ok.** Условия 1–3 дают право налить код. `parity-ok` дополнительно требует
E3 по N0 (READY+send+секреты), live-истории/реакций/ack, G-05 10/10 или waiver, закрытых
M-07/C-02/G-01b и `gate:qualify` с `e2e.live === "met"`.

---

## 6. Следующие 5 задач по WSJF

Формула устава §5.2: `(BV + TC + RR) / JS`, шкала 1–5. Правило очереди (`QUEUE.md` §1):
P1A (доверие) раньше P1B (паритет); внутри класса — WSJF. В списке **нет** карточек,
которые уже в работе или на ревью у другого владельца: `L02-UI-FIX` (код+отчёт готовы,
ждёт Codex), `M-07-BE3` (то же), `QA-M2-02`/`QA-M2-04` (пишутся), `SEC-L02-FINAL`
(read-only ревью).

| # | Карточка | Класс | BV | TC | RR | JS | **WSJF** | Owner | Зависит | DoD (что **не** pass) |
|---:|---|---|---:|---:|---:|---:|---:|---|---|---|
| 1 | **REVIEW+ACCEPT-L02-CORE-FIX** | P1A | 4 | 5 | 3 | 1 | **12.0** | Codex + PM | `FIX-REPORT 15:45` | ACCEPT ссылается на md5 `8703c1d6…`/`7f23d85a…`/`fe8aa921…`, Codex подтверждает P1/P2; гейт после 15:45. Не pass: ACCEPT по md5 из `REPORT §6.4` |
| 2 | **META-PLAN-SYNC** (`TDD-PLAN.md §8` +2 файла) | P1A | 3 | 5 | 4 | 1 | **12.0** | TL | meta-лог 15:45 | `plan-sync` зелёный: оба файла описаны ровно по разу. Не pass: «meta не запускали» |
| 3 | **ACCEPT-SEARCH-FIX** (QA-M2-03 / M-13) | P1A | 3 | 5 | 4 | 1 | **12.0** | PM/Codex | `REPORT 15:45` | ACCEPT + запись R-01/M-13 в `BUGS.md`; браузер — отдельной карточкой. Не pass: unit-зелёный вместо e2e |
| 4 | **TYPING_START-FORWARD** (P-03 end-to-end) | P1B | 4 | 3 | 2 | 1 | **9.0** | G1+G2 | `net.rs` свободен | `[L]`-кадр как оракул → `typing.start` → индикатор зажигается из события. Не pass: fake-событие вместо форварда |
| 5 | **SEC-L02-01-DEDUP** (P1 security) | P1A | 4 | 4 | 5 | 2 | **6.5** | G1 | `net.rs` | сброс дедупа `s` на новом `connect` + тест «relogin → READY доставлен». Не pass: комментарий вместо сброса |

**Следующие за пятёркой:** `G-01b` (6.0: 3/2/1/1) → `C-02-UI-OLDER` (5.0: 4/4/2/2) →
`QA-M2-05/07` (4.5: 3/3/3/2) → `M-07-UI-badge` (4.0: 3/3/2/2) → `G-05-REST` (1.2: 3/2/1/5,
после принятия спеки rev.4).

**Оговорки:**
1. Пункты 1–3, 5 — не продуктовый UI; `app.js`/`chat.js` они не занимают.
2. `chat.js` — монополия `C-02-UI-OLDER`; `dispatch-model/view` освободятся после ACCEPT
   L-02-UI; `net.rs` — `TYPING_START` и SEC-L02-01 **только последовательно**.
3. `data.rs` свободен после M-07-BE3; следующая карточка на нём — `M-07`-потребление в UI
   (не `data.rs`) или `PLAT-11a`.
4. `scripts/gate.mjs` — зона TL/G4; правка гейта не должна менять пороги тестов.
5. Любая карточка стартует red-first и принимается по `counts`, не по `verdict`.

---

## 7. Снимок md5 (15:56, только чтение)

| Файл | md5 | Сверка |
|---|---|---|
| `rust/crates/discord-core/src/ws.rs` | `8703c1d695cd0af26458393eb3c1e3f9` | = `FIX-REPORT §4` |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` | = `FIX-REPORT` |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `7f23d85a4d3b0e54c58d765b8b879c58` | = `FIX-REPORT §4` |
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` | = L-02; SEC-L02-01 открыт |
| `rust/gui-spike/src/data.rs` | `8013271de7a6444b2b9dba3301a165d4` | **дрейф**: 15:52:57, M-07-BE3 (было `a99647c1…`) |
| `rust/gui-spike/src/main.rs` | `336863b5b1ffca169b851a2095798f01` | = C-02-BEFORE |
| `rust/gui-spike/src/settings_store.rs` | `561a4fa17953e6b0c3b508cc5eb7357b` | UiSettings 5 полей |
| `rust/gui-spike/tests/net_contract.rs` | `fe8aa92146e9dcf8b943335dfad652c8` | = `FIX-REPORT §4` |
| `rust/gui-spike/tests/history_before_contract.rs` | `796293ea6ae985dcb17dfd96a61bf1a1` | = C-02-BEFORE |
| `rust/gui-spike/tests/data_contract.rs` | `e31c912740393ec3bae6c48847ca1ef0` | M-07-BE3 (15:53) |
| `rust/gui-spike/tests/read_state_contract.rs` | `fde688f33614636c2699c55ebba945e9` | M-07-BE3 (15:53) |
| `rust/gui-spike/ui/app.js` | `1ac5c36976a064120529c132e61cfb54` | **дрейф**: L-02-UI-FIX (было `c79d7414…`) |
| `rust/gui-spike/ui/features/chat.js` | `2e918f3cff5dc968ca3117480e306ce4` | = L-02-UI |
| `rust/gui-spike/ui/features/actions.js` | `908bfe2dbf4202cdf28c389d44b2095f` | ≠ `g05-ui`/`m07-ui` → **дрейф** |
| `rust/gui-spike/ui/features/search-overlay.js` | `44b34c0f9642188f54fd96c29afdf7a4` | = SEARCH-FIX |
| `rust/gui-spike/ui/features/quick-switcher.js` | `7762a9c8f9fd30463a6ceac26cd460af` | C-07-путь, `dataset.guild` остался |
| `ui/features/dispatch-model.js` | `37dacc61d7daf74c144681637dded1f5` | **дрейф**: L-02-UI-FIX 15:53:58 (было `154eec12…`) |
| `ui/features/dispatch-view.js` | `fd1842ac442dae108f80b3872f163f05` | = L-02-UI |
| `ui/features/dispatch-bridge.js` | `1cad6f1324b2fe508866d29097792443` | = L-02-UI |
| `ui/features/typing-indicator.js` | `09f9f53870b669a382d52ecadcabb799` | = P-03-UI |
| `tests/unit/dispatch_ui.test.mjs` | `f2806f3f4088b0527f6a31ff1d7bce2a` | **дрейф**: 15:53:49; покрыт green 15:54:00 (43/43) |
| `tests/unit/search-remote-real-index.test.mjs` | `182f6199561e8c6334db68a904526788` | = SEARCH-FIX |
| `tests/unit/typing-ui.test.mjs` | `1a5f4de2b3ae8a4d040786840e172e69` | ≠ `896ae41c…` → **дрейф** |
| `tests/unit/read-state-ui.test.mjs` | `f9d6e653c31e5c8c1ab60ce04352f7d2` | ≠ `80f4db2b…` → **дрейф** |
| `tests/unit/channel-paging.test.mjs` | `54184e41ce632d3fdb47cec9672095e0` | ≠ `5d98e960…` → **дрейф** |
| `scripts/gate.mjs` | `89c67ad0a8f087b87626090b2b5899d9` | = QA-GATE-SKIPS |
| `tests/meta/test-policy.test.mjs` | `62d7c60b80bcf11cc18294c9a0350dc4` | = QA-GATE-SKIPS |
| `.github/workflows/ci.yml` | `608e26837c10996aae11d2c29fc6d320` | QA-M2-04: job `qa-browser` (15:52), прогона нет |
| `TDD-PLAN.md` | `a855716387e0a4d8172fc3eb7812c0a4` | §8 без 2 файлов (правка 15:52 их не добавила) |
| `PARITY/PARITY-MATRIX.md` | `c0789f87d4a9fd5f488e56cf1bc8027e` | эталон, не менялся |

Гейты: worker `10:22:26Z` (15:22): meta 65 / unit 651 / workspace 1104+12 / smoke 22,
**без e2e, coverage, keyring**, JSON **не содержит** `e2e`/`acceptance`/`attempts[]` **[V]**;
release `09:37:37Z` (14:37): e2e `tests 4 / pass 0 / skipped 4`, verdict
`passed_with_skips` **[V]**. Ни один не покрывает дерево 15:28–15:54.

---

## 8. Ограничения и что этот отчёт НЕ доказывает

1. **Не запускались** `cargo`, `npm`, `node --test`, `tsc`, гейт, e2e, браузер, CDP, сеть,
   Discord, live, мутации. Все числа тестов — из `.gate/*.json`, отчётов и логов writer'ов.
2. **Снимок нестабилен.** В 15:52–15:54 четыре code-writer'а правили `data.rs`, `app.js`,
   `dispatch-model.js`, два контрактных теста и `dispatch_ui.test.mjs`; параллельно
   G5 правил `ci.yml` (15:52), G4 — `tests/helpers/**` (15:55–15:56). Последние отчёты
   L-02-UI-FIX и M-07-BE3 появились **в 15:54**, уже после первой версии этого снимка.
   §7 — состояние на 15:56, не фиксация. Любая приёмка по этим md5 «поплывёт» после
   следующей записи; приёмку делать только на остановленном дереве.
3. **`implemented` здесь не live-паритет**: E3 = 0. Строка `impl*` читается как
   «offline-путь + тесты», не как «работает в Discord».
4. `[M]`-строки (V-08 speaking, G-05-заглушки, ST-01 подпункты, N-03) паритетом не
   считаются; `[A]`-строки (TTL typing, live-коды invite/leave, порог idle, consent) —
   авторские решения, не RE-факты.
5. **Успешность фиксов принята по логам writer'ов**, не по собственному прогону. Red-логи
   проверены на факт падения, не на «правильную причину» в каждом assert.
6. **`typecheck` не перепроверен** после L-02-UI-FIX: полный `tsc` не запускался; вывод
   «16 ошибок» относится к прежнему md5 теста. Закрытие — только по свежему гейту.
7. Отчёт **не** закрывает карточки, волну и waiver; `PARITY-MATRIX.md`, `GAP-BACKLOG.md`,
   `QUALITY/BUGS.md`, `TASKS.md`, `QUEUE.md`, `STATE.md`, `db.jsonl`, `MERGE-MANIFEST.md`,
   `TDD-PLAN.md` не изменялись (зона PM/TL).
8. Вердикт `block` для merge — следствие красного гейта и неполного accepted set,
   а **не** продуктового дефекта; точный вердикт следующего `gate:worker` станет известен
   только после прогона на остановленном дереве.
