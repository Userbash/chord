# G6-REPARITY-02 — пересчёт parity после L-02-CORE / C-02-PAGE

Дата: 2026-10-24, снимок **15:27–15:30 +05**. HEAD `0239543`, дерево **dirty**.
Воркер: **G6 Parity** (read-only). PM: DeepSeek.
Режим: единственная запись — этот файл. Продукт (`rust/**`, `ui/**`), тесты, `scripts/**`,
гейт, e2e, браузер, CDP, сеть, Discord, live, `cargo`, `npm`, `node --test`, мутации —
не запускались и не менялись. `PARITY-MATRIX.md`, `GAP-BACKLOG.md`, `QUALITY/BUGS.md`,
`MERGE-MANIFEST.md`, `TASKS.md`, `QUEUE.md`, `STATE.md`, `db.jsonl` — **не** менялись.

Метки evidence: **[V]** — проверено в этом проходе чтением/grep/md5/JSON; **[R]** — взято
из отчёта карточки или матрицы. Ни один live-факт здесь не заявляется.

> **Снимок нестабилен.** По `ORCA-WORKERS.md` (mtime 15:26) в этом checkout работают
> **два writer'а**: `L-02-UI-IMPL` (`ui/**`, `tests/unit/dispatch_ui.test.mjs`) и
> `C-02-BEFORE` (`data.rs`/`messages.rs`). На 15:27 файлов L-02-UI в дереве ещё нет,
> `data.rs`/`messages.rs` — mtime 14:05/13:25. Любой md5 ниже — снимок, не фиксация;
> приёмку делать только после остановки обоих writer'ов.

---

## 0. Резюме

1. **Счётчики матрицы не изменились**: 91 строка, `implemented 21 / partial 36 / absent 34 /
   unknown 0`; P0 5 / P1 32 / P2 40 / P3 14. Пересчёт выполнен независимым awk-оракулом
   по самому markdown (§7).
2. **Strict-recheck: 15 / 42 / 34** (те же 6 понижений, что в `G6-RECHECK-01`): `C-02`,
   `C-07`, `M-06`, `M-07`, `M-13`, `G-01`. Из них `G-01` и `C-02` — **пограничные**:
   их остатки (`lastGuildId`, older/tail) в бэклоге вынесены в отдельные карточки
   (`G-01b`, `M-01`), поэтому при такой трассировке класс остаётся `implemented`
   (тогда 17 / 40 / 34). Вердикт от этого не меняется.
3. **L-02-CORE: ядро готово к приёмке, но не принято.** Отчёт `PM/l02-core/REPORT.md`
   (15:22) + md5 совпали с деревом; worker-гейт `10:22:26Z` (15:22 local) **новее**
   последней правки `net.rs` (15:21:14) и зелёный: unit 651, workspace 1104 pass + 12 skip.
   Нет: `PM/l02-core/ACCEPT.md`, mutation-proof `session_id`, release-гейта.
4. **L-02-UI: кода нет.** `gateway://dispatch` и `listen(` в `ui/**` — **0 вхождений** [V];
   `dispatch_ui.test.mjs` отсутствует [V]. Контракт `PM/l02-ui/PLAN.md` заморожен и готов
   к работе, но L-02 остаётся `partial`.
5. **C-02-PAGE: сделано и покрыто** (8 тестов, unit 651 в гейте 10:22), но листает только
   **вперёд**: `data.rs::cursor_start` = `position + 1`, параметра `before` нет [V].
   «Старее»/открытие на последнем сообщении по-прежнему не реализуемы; C-02-BEFORE —
   активный writer.
6. **P-03-UI: матрица устарела в обратную сторону.** UI реализован offline (20 тестов,
   `send_typing` [V], TTL/guard/slowmode), но входящий `TYPING_START` не форвардится —
   индикатор не может сработать от live-события. Класс `partial` сохраняется, **gap другой**.
7. **Вердикт: `deferred`** (не `parity-ok`, не `block`). Причины — §5.

---

## 1. Таблица 91 функции

Колонки: **Матрица** — состояние, зафиксированное `G6-SYNC-01` (14:34);
**Recheck** — strict-класс этого прохода. `impl*` = implemented offline, E3 нет;
`impl*↓` = de facto partial (см. §2.1); `impl*↑` = кандидат на повышение после приёмки.

### 1.1 Auth / session (9)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| A-01 | Вход по логину/паролю | P0 | implemented (live unverified) | impl* | `auth_contract.rs`; live-вход N0-01 не выполнен → E3 нет |
| A-02 | MFA TOTP/SMS/backup/password | P1 | implemented (live unverified) | impl* | 5 методов offline; live-прогона нет |
| A-03 | Captcha-гейт | P1 | implemented (offline) | impl* | live sitekey/flow `[A]` |
| A-04 | QR / one-time login | P2 | absent (UI disabled) | absent | протокол/поллинг не сняты |
| A-05 | Passkey / WebAuthn | P2 | absent (UI disabled) | absent | Linux-WebAuthn нет |
| A-06 | Восстановление сессии | P1 | implemented (keyring `[A]`, live unverified) | impl* | live restore/битый keyring не проверены |
| A-07 | Logout / инвалидация | P1 | implemented (live unverified) | impl* | live stop/401 после logout нет |
| A-08 | Регистрация/сброс пароля | P3 | absent | absent | решение PM: вне desktop scope |
| A-09 | Сессии / logout others | P2 | absent | absent | нет экрана и вызова |

### 1.2 Guilds (9)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| G-01 | Рейл: список/выбор/переключение | P1 | implemented (live unverified) | **partial↓** | READY 7 гильдий + `guild-rail` (8) **[V]**; `UiSettings` без `last_guild_id` → `G-01b` (`settings_store.rs:137`) |
| G-02 | Добавить/создать сервер | P3 | absent | absent | endpoint `GET\|POST /guilds` `[A]` |
| G-03 | Вход по инвайту | P1 | implemented (live unverified) | impl* | `invites_contract` (14) + `join-server` (24); live-коды/TTL `[A]` |
| G-03a | Войти из preview/lurker | P1 | implemented (live unverified) | impl* | backend `[L]` 204×2; live 204 из UI не снят; 403/429 `[A]` |
| G-04 | Выход с сервера | P1 | implemented (live unverified) | impl* | confirm + `{guildId, lurking}`; live-коды `[A]` |
| G-05 | Меню сервера | P1 | partial: **активны 2 из 10** | partial | `[V]` leave + copy-id; invite `capability=false`; спека rev.3 (не accepted) содержит **9 дефектов** |
| G-06 | Инвайты: создать/список/отозвать | P2 | absent | absent | API/UI нет; коды `[A]` |
| G-07 | Роли/каналы/права сервера | P3 | absent | absent | design-first |
| G-08 | Unread/mention сервера | P2 | partial: статичный `is-unread` | partial | нет счётчиков/read_state/бейджей |

### 1.3 Channels / navigation (6)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| C-01 | Дерево: категории, text/voice/forum | P1 | partial: дерево из READY | partial | типы 0/2/4/5/15; нет open voice/forum, collapse |
| C-02 | Переключение канала + история | P1 | implemented (live unverified) | **partial↓** | реальные id + C-02-PAGE (8) **[R]**; листает только вперёд, older/tail нет; live-истории нет |
| C-03 | Unread/mention канала | P2 | partial: статичный класс | partial | нет mention/ack-счётчика |
| C-04 | Шапка: пины/ветки/участники/поиск | P2 | partial | partial | поиск/участники есть; pins/threads stub |
| C-05 | Создание/редактирование канала | P3 | absent | absent | UI не снят |
| C-07 | QuickSwitcher `Ctrl+K` | P2 | implemented (B6) | **partial↓** | **[V]** свитчер читает `dataset.guild`, рейл отдаёт `data-guild-id` → выбор no-op; канал не грузит историю |

### 1.4 Messages (14)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| M-01 | История/пагинация | P1 | partial: fixtures + PageStore | partial | live-истории/курсоров нет; forward-paging живёт в C-02 |
| M-02 | Отправка | P0 | implemented (live unverified) | impl* | live send/401/429 → окно N0-02 |
| M-03 | Редактирование | P1 | implemented (live unverified) | impl* | тело edit `[A]` |
| M-04 | Удаление | P1 | implemented (live unverified) | impl* | live 403/tombstone не проверены |
| M-05 | Ответ (reply) | P1 | implemented (live unverified) | impl* | `message_reference`; live-поле `[A]` |
| M-06 | Реакции | P1 | implemented (live unverified) | **partial↓** | pills/исходящие на реальных id **[V]**; входящие `MESSAGE_REACTION_*` не доходят (нет L-02 UI); live PUT/DELETE/429 нет; REMOVE `[A]` |
| M-07 | Read state / ack | P1 | implemented (live unverified) | **partial↓** | ack/`lastViewed` есть; `data.rs` отдаёт `flags`, UI берёт только `mentionCount` **[V]**; badge не проецируется |
| M-08 | Markdown/embeds/mentions | P1 | partial: только embeds | partial | нет markdown/mentions/emoji |
| M-09 | Контекстное меню (15) | P2 | partial: 4 команды | partial | forward/bookmark/reminder/TTS/report stub |
| M-10 | Пины | P2 | absent (stub) | absent | нет панели/загрузки |
| M-11 | Ветки | P2 | absent (stub) | absent | нет списка/создания |
| M-12 | Forward/bookmark/reminder/TTS/report | P3 | absent | absent | endpoints `[A]` |
| M-13 | Поиск по серверу | P1 | implemented (4/7, live unverified) | **partial↓** | **[V]** `app.js:1206` берёт `dataset.guild` (пусто) → `search_remote` не включается |
| M-14 | Эмодзи-пикер | P2 | implemented (static) | impl* | статичный набор; кастомные/стикеры нет |

### 1.5 Members (4)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| MB-01 | Список: роли + статусы | P1 | partial: 24k synthetic | partial | live-членов/группировки нет |
| MB-02 | Роли/цвета/права | P2 | partial: synthetic | partial | live-ролей/цветов нет |
| MB-03 | Профиль-поповер | P2 | partial: stub aria | partial | нет поповера/полей |
| MB-04 | Полный поиск участников | P2 | absent (deferred) | absent | RE request/response |

### 1.6 Presence / typing (5)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| P-01 | Статусы online/idle/dnd/offline | P1 | partial: synthetic + core | partial | live-статусов нет |
| P-02 | Обновления PRESENCE_UPDATE | P1 | partial: reducer, UI не связан | partial | нет применения к UI (L-02-UI) |
| P-03 | Индикатор набора + POST /typing | P1 | partial: backend есть, UI отсутствует | partial | **UI теперь есть offline** (20 тестов, `send_typing` **[V]**); входящий `TYPING_START` не форвардится → индикатор не зажигается от live |
| P-04 | Кастомный статус/активность | P2 | absent | absent | нет редактора/отображения |
| P-05 | Смена своего статуса | P2 | absent | absent | нет меню/IDENTIFY/UPDATE |

### 1.7 Voice (11)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| V-01 | Вход/выход из voice | P1 | absent (crate only) | absent | нет session manager/UI |
| V-02 | RTC-сигнализация | P1 | partial: парсер/тесты | partial | нет live-WS/сессии |
| V-03 | UDP discovery/keepalive | P1 | partial: примитивы | partial | нет live-UDP |
| V-04 | SRTP | P1 | partial: KDF/AEAD тесты | partial | нет live encrypt/decrypt |
| V-05 | Opus/джиттер/DTX | P1 | partial: jitter, нет Opus | partial | нет encode/decode |
| V-06 | Статистика/reconnect/регионы | P2 | partial: policy/stats | partial | нет live RTT/потерь |
| V-07 | Устройства/mute/deafen/PTT | P1 | absent (settings fake) | absent | нет устройств/эффекта |
| V-08 | Голосовой UI | P1 | partial: панель на fake store | partial | реального voice-backend нет (V-01…V-05) |
| V-09 | Шумоподавление Krisp | P2 | absent | absent | нет модели/интеграции |
| V-10 | DAVE/MLS E2EE | P2 | absent | absent | нет MLS |
| V-11 | Voice messages/clips/soundboard | P3 | absent | absent | контракт не снят |

### 1.8 Video / Go Live (4)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| S-01 | Камера: preview/enable | P2 | absent | absent | нет захвата/превью |
| S-02 | Screen share / Go Live | P2 | absent | absent | нет capture/сигнализации (RE-3) |
| S-03 | Зрители/качество/simulcast | P2 | absent | absent | нет RTP-видео/декода |
| S-04 | Activities | P3 | absent | absent | контракт запуска `[A]` |

### 1.9 Attachments (3)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| AT-01 | Загрузка файла | P2 | absent (кнопка disabled) | absent | **blocked**: метод/поля/лимиты (RE-1) |
| AT-02 | Drag&drop/paste | P2 | absent | absent | нет триггеров |
| AT-03 | Рендер вложений | P2 | absent (только embeds) | absent | нет схемы attachment |

### 1.10 Settings (8)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| ST-01 | Навигация настроек | P2 | partial: 18 разделов, 3 панели | partial | 63 DOM-строки; подпункты 17 разделов `[A]` |
| ST-02 | Голос/видео: 38 тумблеров | P2 | partial: нет эффекта | partial | устройств/аудио нет |
| ST-03 | Внешний вид: тема/плотность/zoom | P2 | partial | partial | zoom/полного light нет |
| ST-04 | Язык/время + i18n-рантайм | P2 | partial: crate не подключён | partial | нет `t()`/каталогов в UI |
| ST-05 | Уведомления: настройки | P2 | partial: панель-заглушка | partial | нет полей/эффекта |
| ST-06 | Горячие клавиши | P3 | absent | absent | нет редактора |
| ST-07 | Аккаунт/профиль | P3 | absent | absent | поля не сняты |
| ST-08 | Персистентность/синхронизация | P2 | partial: локальный файл `[A]` | partial | нет settings-proto sync |

### 1.11 Notifications / DND (5)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| N-01 | Системные уведомления | P2 | partial: D-Bus crate не подключён | partial | нет вызова из UI |
| N-02 | DND/режим концентрации | P2 | partial | partial | нет подавления |
| N-03 | Inbox/упоминания | P2 | absent (кнопка disabled) | absent | UI/счётчики не сняты |
| N-04 | Бейдж/прогресс | P3 | absent | absent | нет Unity badge |
| N-05 | Переопределения сервер/канал | P2 | absent | absent | нет меню/уровней |

### 1.12 Platform (8)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| PL-01 | Трей/фон | P2 | absent | absent | нет трея |
| PL-02 | Автозапуск/старт свёрнутым | P2 | partial: модель `.desktop` | partial | нет записи/UI |
| PL-03 | Deep links + single instance | P2 | partial: парсер `discord://` | partial | нет wiring/second-instance |
| PL-04 | Апдейтер | P3 | partial: manifest/plan | partial | нет скачивания/замены |
| PL-05 | Spellcheck | P3 | partial: hunspell FFI | partial | нет UI/словарей |
| PL-06 | Питание/idle | P2 | partial: power/idle crate | partial | порог idle `[A]` |
| PL-07 | Consent/разрешения захвата | P3 | absent | absent | consent-flow не снят |
| PL-08 | Окно: badge/progress/flash/PiP | P3 | absent | absent | нет вызовов |

### 1.13 Live / connectivity (5)

| ID | Функция | P | Матрица | Recheck | Evidence / остаток |
|---|---|---|---|---|---|
| L-01 | Reconnect/offline state в UI | P1 | implemented (live unverified) | impl* | `#net-status` + retry; live resume/4013/4014 нет |
| L-02 | Dispatch → UI | P1 | partial: reducer/core, UI на fixtures | partial | core-хук готов к приёмке; **UI-подписчика нет** (`gateway://dispatch` = 0) **[V]** |
| P0-01 | Live READY | P0 | partial (offline) | partial | нет E3 |
| P0-02 | Live send | P0 | partial (offline) | partial | нет E3 |
| P0-03 | Секреты не в логах | P0 | implemented (offline) | impl* | live-грепа token/password/key нет |

### 1.14 Сходимость таблицы

| Класс | Матрица | Strict recheck | Δ |
|---|---:|---:|---:|
| verified (E3) | 0 | **0** | 0 |
| implemented (offline) | 21 | **15** | −6 |
| partial | 36 | **42** | +6 |
| absent | 34 | **34** | 0 |
| unknown | 0 | **0** | 0 |
| **Итого** | **91** | **91** | 0 |

Пограничные: `G-01` и `C-02` — если их остатки считаются отдельными карточками
(`G-01b`, `M-01`, как они и лежат в бэклоге), класс остаётся `implemented`, и тогда
строгая раскладка **17 / 40 / 34**. Обе раскладки сходятся к 91.

---

## 2. Что реально изменилось после `G6-SYNC-01`

### 2.1 Какие `implemented` стали de facto partial (6)

Понижение не меняет матрицу (владелец — PM); это флаг, что «функция + тесты» ≠
end-to-end.

| ID | Почему не end-to-end | Evidence | Что вернёт `implemented` |
|---|---|---|---|
| **C-02** | История листается только **вперёд**: `cursor_start` = `position + 1`, `next_cursor` = id последней строки, `before` нет; подгрузка на нижнем крае; открытия на последнем сообщении нет | `data.rs:814`, `:1278-1296` **[V]**; C-02-PAGE §6 **[R]** | письменное решение PM: forward-only в DoD **или** `before`-курсор + tail-open |
| **C-07** | Свитчер адресует гильдии `node.dataset.guild`, рейл выставляет `data-guild-id` → target `""`, выбор плитки no-op; выбор канала не вызывает `openChannel`/`messages_page` | `quick-switcher.js:67,144,150`; `app.js:489,492` **[V]** | fallback на `data-guild-id` (прецедент — `actions.js:367`) + вызов `openChannel` |
| **M-06** | Свои реакции рисуются/шлются на реальных id, но чужие `MESSAGE_REACTION_ADD/REMOVE` не доходят до UI (нет подписчика) | `chat-reactions` (9); `net.rs:113-120` **[V]** | L-02-UI mapping реакций (REMOVE — `[A]`, 0 кадров) |
| **M-07** | Ack работает, но `data.rs` отдаёт `flags`, а UI читает только `mentionCount`; unread без упоминаний и badge-счётчики не выводятся | `data.rs:514-528`; `chat.js:927,965-971` **[V]** | G1 badge/`flags`-проекция + G2 badge/clear-on-ack (G-08/C-03) |
| **M-13** | `app.js:1206` передаёт `guildId` из `dataset.guild` (пусто) → серверный `search_remote` не включается | `app.js:1206`, `search-overlay.js:121` **[V]** | `data-guild-id` + тест, что `search_remote` получает реальный guildId |
| **G-01** | Список/выбор из READY есть, но `lastGuildId` не сохраняется — после рестарта выбор теряется | `settings_store.rs:137-148` (нет поля) **[V]** | `G-01b`: поле + миграция + restore-тест |

Не понижались, хотя остатки есть: `M-14` (статичный пикер — это и есть функция),
`A-03`/`A-06` (`[A]`-оговорки), `P0-03` (offline-redaction — сама функция),
`L-01` (UI реализован, live только подтверждение).

### 2.2 Что закрыто offline

| Срез / строка | Что закрыто | Evidence | Что осталось |
|---|---|---|---|
| **L-02-CORE** *(не строка матрицы)* | хук `RunOptions.on_dispatch`, `with_dispatch_handler`, дренаж `GatewayPipe::take_events`, проводка `core_supervisor_config`, `CORE_DISPATCH_AVAILABLE=true` | отчёт 15:22; core 575, gui 269, `net_contract` 22, clippy/fmt; гейт 10:22Z после последней правки **[R/V]** | ACCEPT, mutation-proof, release-гейт, live-E3 |
| **C-02-PAGE** *(часть C-02)* | `nextCursor/hasMore`, окно ≤500, дедуп, ошибки/retry, якорь; 8 тестов | `channel-paging.test.mjs` (8) **[V]**; unit 651 в гейте 10:22 | older/tail; live-история |
| **P-03-UI** *(часть P-03)* | исходящий `send_typing`, «Печатает…», TTL 10 с, троттлинг 8 с `[A]`, guard ≤5, 401/429/slowmode; 20 тестов | `typing-indicator.js`, `send_typing` **[V]**; `typing-ui` (20) | входящий `TYPING_START`; live |
| G-01 (D-112) | рейл из READY, выбор/переключение | `guild-rail` (8) | G-01b, live READY |
| C-02 (D-117/D-120) | реальные `channelId`/`messageId`, переключение | `channel-switch` (10) | paging (выше), live |
| G-03 (D-116/122) | resolve → preview → accept `{code}` | `invites_contract` (14) + `join-server` (24) | live-коды/TTL, screening |
| G-03a (D-76/128) | lurker `PUT …?lurker=true` + `{}` | 204×2 `[L]`, tests | live 204 из UI, 403/429 |
| G-04 (D-89/92) | leave + confirm + typed errors | `guilds_contract` + features | live-коды 403/404/429 |
| M-07 (D-124/126) | ack на open/focus/scroll, `last_viewed`, `token` nullable | `rest_ack` (9) + `read_state_contract` (12) + `read-state-ui` | badge/`flags` (см. §2.1), live ack |
| L-01 (D-106) | `#net-status`, retry, reconnect-статусы | `gui-spike-features` (+8) | live resume/4013/4014 |
| M-06 (D-131) | pills/quick-реакции на реальных id | `chat-reactions` (9) | входящие события (L-02-UI), live |
| G6-D-01/D-02 | ложные `"news"`/`String(index)` устранены | `BUGS.md` **[V]** | live send/edit/delete |
| G6-D-04/D-05 | доки: размер ST-01, Ref G-01 | `G6-SYNC-01` | `settings-nav-63.tsv` (оракул) |

### 2.3 Что требует live / E3

**Ни одна строка не `verified`: E3 = 0.** Live-окно N0 снимает:

| Блок | Строки | Минимум |
|---|---|---|
| N0 (1 окно) | P0-01, P0-02, P0-03 | READY + guild/channel в логе; сообщение видно в канале; 401/429; `grep token/password/key = 0` |
| N0 (то же окно) | A-01, A-02, A-03, A-06, A-07, M-02…M-07, M-13, C-02/M-01, G-01, L-01, L-02 | вход/MFA/капча, restore/logout, send/edit/delete/reply, реакции, ack, поиск, история, READY-рейл, resume-коды, «событие дошло до окна» |
| N1 (после RE-2) | G-03, G-03a, G-04, G-06 | live-коды/TTL, 204 из UI, owner-leave, invite CRUD |
| Voice-окно | V-01…V-08 | два клиента, запись только своего микрофона |
| Stream-окно | S-02, S-03 | свой экран/камера, consent, stop |
| Live-меню | G-05 | реальные права/capability |
| Presence/members | MB-01, MB-02, P-01, P-02 | live-члены/роли/статусы |
| Live e2e | 2 кейса `live-discord` | `CDP_ENDPOINT` + окно |

Всё, что помечено `impl*`, без окна остаётся `live unverified` — это **не** приёмка PM
и **не** `parity-ok`.

### 2.4 Что нового появилось в фактах (дельта к `G6-RECHECK-01`)

1. **Гейт 10:22:26Z (15:22 local) новее `G6-RECHECK-01`**: вердикт
   `passed_with_skips`; unit **651**, workspace **1104 pass / 12 skip**, meta 65, smoke 22;
   **фазы e2e/coverage/keyring-live нет**; у `spike-rust` в JSON только `ok`/ms, без
   `counts` [V]. То есть L-02-CORE покрыт worker-гейтом (фаза `spike-rust` + workspace),
   но release-scope прогона после него не было; 12 skip — не pass.
2. **`net.rs` md5 `e13d4e8c…` совпал с отчётом** L-02-CORE (ранее фиксировался дрейф
   `d07d16fe…` → `e13d4e8c…`) [V]. Дрейф `channel-paging.test.mjs` сохраняется:
   факт `54184e41…` vs отчёт `5d98e960…` [V] — нужен red/green-лог на правку (R-03).
3. **P-03-UI** и **C-02-PAGE** попали в unit 651; `dispatch_ui.test.mjs` не существует [V].
4. **G-05**: спека rev.3 (15:26) — не accepted; в ней 9 дефектов текущего кода
   (disabled invite срабатывает, roving tabindex, Esc всплывает, ложный «ID скопирован»,
   `max-height` диалогов, disabled-причины, 429-countdown, owner/capability-проекция).
5. **MERGE-MANIFEST устарел**: §4 говорит «P-03 typing UI отсутствует», «C-02 — только
   первая страница», L-02-CORE/UI в окне нет. Это процессный риск приёмки (см. §6, §8).

---

## 3. Отдельно: L-02-CORE и C-02-PAGE

### 3.1 L-02-CORE — готов к приёмке, не принят

| Проверка | Состояние | Evidence |
|---|---|---|
| red-first | тест написан до хука (`E0599 with_dispatch_handler`) | `red.log` 15:16 **[R]** |
| код ядра | `RunOptions.on_dispatch`, `SupervisorConfig::with_dispatch_handler`, дренаж после `push_binary` | `ws.rs`, `supervisor.rs`, md5 совпали с отчётом **[V]** |
| проводка | `core_supervisor_config` + `CORE_DISPATCH_AVAILABLE=true` | `net.rs:126` **[V]** |
| targeted | gateway_supervisor 12/12, core 575, gui 269, `net_contract` 22/22 | логи 15:17–15:22 **[R]** |
| clippy/fmt | чисто (core и gui) | `clippy-core.log`, `clippy-gui.log`, fmt-логи **[R]** |
| гейт | `worker 10:22:26Z` — **после** mtime `net.rs` 15:21:14; вердикт `passed_with_skips`, `spike-rust` ok (counts в JSON нет) | JSON гейта **[V]** |
| ACCEPT | **нет** `PM/l02-core/ACCEPT.md` | каталог **[V]** |
| mutation-proof `session_id` | **не выполнен** (условие `PM/l02/ACCEPT.md §3.3`) | **[R]** |
| release-гейт | **нет**; worker-гейт без e2e/coverage/keyring-live | JSON **[V]** |
| live | не требуется для core, нужен для `sourceAvailable=true` в живой сессии | — |

**Вывод:** срез можно принимать; до этого приёмка L-02 (строки матрицы) невозможна.
Приёмка L-02-CORE **не** закрывает L-02 — это половина.

### 3.2 C-02-PAGE — сделано, но направление не решено

| Проверка | Состояние |
|---|---|
| первая страница | `{channelId, limit:100}` без `cursor`/`token` **[R]** |
| подгрузка | `{channelId, cursor: nextCursor, limit:100}` у нижнего края, single-flight **[R]** |
| окно | ≤500 строк, `data-page-state="limit"`, шестой запрос не уходит **[R]** |
| дедуп/якорь | по `id`, существующая строка побеждает; `scrollTop` не сбрасывается **[R]** |
| ошибки | `role="alert"` + retry того же шага, первая страница цела **[R]** |
| тесты | `channel-paging.test.mjs` 8/8; suite 368; unit 651 **[R/V]** |
| **направление** | `cursor_start` = `position + 1` → **только вперёд**; `before` нет **[V]** |
| older/tail | не реализованы; открытие — начало истории **[R]** |
| live | не проверялся |

**Что это значит:** C-02-PAGE закрыл ровно тот остаток, который был записан в матрице
(«история — только первая страница»), но не сделал C-02 полной: «старее» и открытие на
последнем сообщении требуют либо решения PM «forward-only достаточно», либо
`before`-курсора (C-02-BEFORE, активный writer). Это же блокирует M-01.

---

## 4. Точные критерии закрытия

Ниже — проверяемые условия. `[x]` — выполнено, `[ ]` — нет.

### 4.1 L-02 (UI ещё нет)

**Половина CORE**
- [x] код хука + дренаж; [x] targeted-логи; [x] md5 совпали; [x] worker-гейт после
      последней правки; [x] clippy/fmt.
- [ ] `PM/l02-core/ACCEPT.md` с вердиктом и md5-фиксацией (файла нет).
- [ ] mutation-proof redaction: убрать вырезание `session_id` → `net_contract` **падает**
      (сейчас не выполнено; без этого redaction доказан только зелёным «на себя»).
- [ ] release-scope прогон на одном дереве (meta/unit/spike/workspace/smoke **+ e2e
      + coverage + keyring-live**), вердикт читается по `counts`.
- [ ] живая проверка «событие дошло до окна» — только для `verified`, не для приёмки core.

**Половина UI** (контракт `PM/l02-ui/PLAN.md`, файлы ещё не созданы)
- [ ] `listen("gateway://dispatch")`, один подписчик, `start()` после auth ready,
      `stop()` → unlisten, повторный `start()` не дублирует.
- [ ] нормализация конверта `{kind,data}` (без `s`; дедуп по бизнес-ключам:
      `guildId`, `userId`, `chunk`, `channelId:messageId:emoji:userId:action`).
- [ ] 6 `kind` → экраны без reload: `READY` (рейл/`.tree-guild`/`#member-online`),
      `GUILD_MEMBERS_CHUNK`, `GUILD_MEMBER_LIST_UPDATE` (панель, `data-user-id`),
      `PRESENCE_UPDATE` (LWW по `ts`), `MESSAGE_REACTION_ADD/REMOVE` (пилюли,
      clamp 0, `me`).
- [ ] инварианты §4.4 PLAN: число IPC `ready/channels/messages_page` не растёт,
      `#chat-list` не пересобирается, фокус/экран/`scrollTop` не меняются.
- [ ] negative N1–N13 (unknown kind/malformed/dup/out-of-range/секреты/нет транспорта).
- [ ] `sourceAvailable=false` → видимое «нет источника» (`data-dispatch-source="absent"`),
      а не пустой чат; `net_dispatch_stats` читается.
- [ ] секреты не попадают в DOM/атрибуты/снапшот/счётчики.
- [ ] `tests/unit/dispatch_ui.test.mjs` (A1–A3, B1–B15+B7a, C1–C14, D1–D5), red-first.
- [ ] PM-приёмка UI + гейт; после этого L-02 → `implemented (live unverified)`;
      `verified` — только после live-окна.
- [ ] **вне L-02**: `MESSAGE_CREATE`, `TYPING_START`, `MESSAGE_ACK` не форвардятся —
      отдельные карточки (иначе L-02 расползётся).

**Закрытие строки L-02:** `[x] core accepted` + `[x] UI accepted` + `[x] гейт на одном
дереве` → `implemented (live unverified)`; `[x] live-событие в окне` → `verified`.

### 4.2 G6-D-03

Дефект **переформулировать**: текущий evidence («`ready`/`members_page`/`search_page`/
`settings`/`enter_guild` = 0 invoke») неверен для `ready` (вызывается через
`READY_COMMAND = "ready"`) и `enter_guild` (UI-алиас `join_guild`,
`guilds.rs:393-398`).

- [ ] (а) `enter_guild` — documented alias + тест, что UI-глагол `join_guild` идёт в ту же
      команду; из дефекта убрать.
- [ ] (б) `members_page` — вызов из экрана участников (`invoke("members_page")`) + DOM-тест
      на фикстуре READY; сейчас 0 вхождений **[V]**.
- [ ] (в) `search_page` — решение PM (локальный поиск vs backend) + тест; сейчас 0.
- [ ] (г) `settings` — решение PM (статические секции vs backend-команда) + тест;
      сейчас только `settings_load/save`.
- [ ] (д) evidence в `QUALITY/BUGS.md` переписать по факту (правка документа, не продукта).
- [ ] Закрытие **не** «одним L-02»: L-02 закрывает только `ready`/часть данных;
      (б)–(г) — отдельные вызовы/решения.

**DoD D-03:** все 4 пункта либо реализованы с тестом, либо явно сняты письменным решением
PM; после этого статус `closed` с ссылкой на тесты.

### 4.3 C-02 older

Нужно **письменное решение PM** (развилка):

- **Вариант A — forward-only принят.** Обновить DoD C-02/GAP (история читается от начала
  к концу; «старее» не входит), закрыть C-02 как `implemented (live unverified)`;
  M-01 оставить на live-историю. Кода не требуется.
- **Вариант B — older нужен.** G1 (C-02-BEFORE, `data.rs`/`messages.rs`):
  - [ ] параметр `before` у `messages_page` + `cursor_start_before` (строки **до** курсора),
        `next_cursor` = id первой строки страницы;
  - [ ] тест-оракул на двух страницах: старшие/младшие snowflake, нет дублей, порядок,
        якорь при prepend (UI-компенсация уже есть в `chat.js`);
  - [ ] tail-open: открытие на последнем сообщении (либо `before` от `last_message_id`,
        либо ≤5 проходов — отдельная политика);
  - [ ] окно ≤500 и остановка на `hasMore=false` сохраняются;
  - [ ] приёмка + гейт; live-захват истории — в N0.
- В обоих вариантах: live-история реального канала (M-01) остаётся E3.

### 4.4 P-03 UI

Код уже есть; закрытие — приёмка + входящий путь.

- [x] исходящий `send_typing({channelId})` из реального канала; 1 отправка на интервал
      `TYPING_RESEND_MS = 8000` `[A]`; guard ≤5; bench не отправляет.
- [x] индикатор `#typing-indicator`, `role=status`, `aria-live="polite"`, TTL 10 с,
      себя не показывает, 401/429/slowmode без цикла; 20 тестов.
- [x] гейт 10:22 включает `typing-ui` (unit 651) **[V]**.
- [ ] `PM/p03-ui/ACCEPT.md` (файла нет).
- [ ] входящий `TYPING_START`: G1 добавляет имя в `FORWARDED_DISPATCHES` (+`net_contract`
      с `[L]`-кадром), G2 маппит событие в `typing.start` (+DOM-тест из события).
      **Без этого индикатор не зажигается в реальном канале** — P-03 не end-to-end.
- [ ] live (опционально): второй клиент печатает → индикатор.
- После: P-03 → `implemented (live unverified)`, матричный текст «UI отсутствует»
  заменить на фактический gap.

### 4.5 G-05

- [x] спека 10 пунктов rev.3 (не accepted) — единая таблица, capability/disabled/route.
- [x] в коде активны ровно 2: `guild-leave`, `guild-copy-id`; invite — при
      `capability=true`, которую никто не выставляет (`actions.js:77-78`).
- [ ] принять/поправить 9 дефектов §0.4 спеки: disabled-пункт = no-op (сейчас invite
      срабатывает), roving `tabindex` за фокусом, `Tab`/Esc (stopPropagation, не
      переключать экран), `#10` без ложного «скопировано», `max-height` диалогов,
      disabled-причины (`data-disabled-reason` + `aria-describedby`/`title`),
      429-countdown, `ownerId`/`isOwner` из READY, геометрия 17px/44-46/20×20.
- [ ] G1 payload: `ownerId`/`isOwner`, capability-поля; проекция
      `user_guild_settings` (#1/#3/#4/#5 — чтение есть в READY, записи `[RE]`).
- [ ] RE-2: capability/route для #1/#3/#4/#5/#6/#7/#8; #2 — G-06; #8 — локальная
      операция без route.
- [ ] 10/10 активных **или** письменный waiver/перенос части пунктов в отдельные карточки.
- **Честная оценка:** 10/10 offline недостижимо: 6 пунктов требуют RE/чужих карточек
  (#2 — G-06, #6/#7/#8 — RE, #1/#3/#4/#5 — G1-проекция + RE записи). Без waiver G-05
  остаётся P1 `partial` и блокирует `parity-ok`. Live: реальные права.

### 4.6 M-07

- [x] ack на open/focus/scroll-to-bottom, дедуп/throttle, `last_viewed` — счётчик,
      `token` nullable и не bearer; `null ≠ 0`; `rest_ack` (9) + `read_state_contract` (12)
      + `read-state-ui`.
- [ ] G1: badge-проекция `badge_count`/`read_state_type` (сейчас `data.rs` **явно
      исключает** эти поля, `:514-528` **[V]**; в READY 2 badge-записи) — аддитивно,
      с оракулом READY, без выдуманных 0.
- [ ] G2: unread без упоминаний по `flags`, badge-счётчики гильдии/канала
      (G-08/C-03), сброс по ack; тесты «`null` ≠ 0», «unread без mention», «очистка».
- [ ] `MESSAGE_ACK` в UI — только если нужны live-обновления; L-02 его **не** форвардит →
      отдельная forward-карточка.
- [ ] live: один `POST ack` 200 + сброс полосы.
- После: M-07 + G-08 + C-03 закрываются вместе; live-ack — E3.

### 4.7 G-01b

- [ ] `UiSettings.last_guild_id: Option<String>` + миграция/версия; неизвестный id →
      fallback на первую гильдию (не ошибка).
- [ ] round-trip `settings_save`/`settings_load` (тест в `settings_contract.rs`).
- [ ] UI: после `ready()`/`channels` выбрать сохранённую гильдию, если она есть в READY;
      иначе — текущий дефолт; тест в `guild-rail.test.mjs`.
- [ ] приёмка + гейт; live READY — в N0.
- Файлы: `settings_store.rs` (G1) и `app.js` (G2) — **разные владельцы и разное время**:
  UI-часть после L-02-UI (тот же `app.js`).

### 4.8 e2e

- [ ] браузер в среде/образе + `CHROME_BIN`; сейчас в `PATH` нет
      `chromium/google-chrome/brave`, `CHROME_BIN` unset **[V]**.
- [ ] `cdp-browser` + `walk-fixture` — fixture-прогон: **pass 2 / skip 2 (live)**;
      вердикт читается по `counts`, не только по `verdict`.
- [ ] фаза e2e в worker-гейте (сейчас её нет вовсе) **или** явное правило, что
      `gate:worker` не покрывает e2e; Q-09/QA4-01.
- [ ] `failOnSkips` (или эквивалент) — чтобы skip не выглядел как pass.
- [ ] для `parity-ok`: 2 live-кейса с `CDP_ENDPOINT` + окно.
- [ ] release-гейт после merge: e2e **4/4 без skip** (2 fixture + 2 live).

---

## 5. Вердикт

### `deferred` — не `parity-ok`, не `block`

**Почему не `parity-ok`:**
1. **E3 = 0 из 91.** `implemented (live unverified)` — offline-путь + тесты, не паритет.
2. **L-02 не end-to-end:** core готов к приёмке, но ACCEPT/mutation/release-гейта нет;
   UI-подписчика нет вообще (`gateway://dispatch` = 0).
3. **6 `implemented` de facto partial** (C-02, C-07, M-06, M-07, M-13, G-01);
   два из них (G-01, C-02) — пограничные, но остатки реальны.
4. **G-05 — 2/10**, 8 пунктов без подтверждённых capability/route; спека содержит
   9 дефектов текущего кода; 10/10 offline недостижимо без waiver.
5. **e2e — 0 pass / 4 skip**, браузера в среде нет; worker-гейт фазы e2e не содержит.
6. **P0-01/02/03 live не сняты**; окна N0 не было.
7. **Процесс:** `waves/db.jsonl` — 0 событий RW-40; `STATE.md` от 2026-10-23;
   `MERGE-MANIFEST`/`QUEUE`/`TASKS` расходятся с фактом (P-03 UI, C-02-PAGE, L-02-CORE);
   дерево dirty, два writer'а активны → снимок нестабилен.

**Почему не `block`:**
- P0-дефектов в коде нет: секреты вычищаются, вымышленных endpoint/полей нет,
  API-контракты не нарушены.
- Offline-путь и тесты есть; worker-гейт 10:22 зелёный (1104 pass + 12 skip),
  L-02-CORE targeted-логи зелёные.
- Все блокеры снимаются offline-задачами + одним live-окном + браузером; архитектурных
  препятствий нет.

**Что переводит вердикт:**
- → `parity-ok`: L-02-CORE accepted + L-02-UI accepted + release-гейт 4/4 e2e +
  E3 (N0) + G-05 10/10 или waiver + M-07/C-02/G-01b закрыты.
- → `block`: красный гейт на текущем дереве, регресс P0, утечка секретов, вымышленный
  endpoint/поле, или расхождение матрицы и кода больше чем на класс.

---

## 6. Что можно параллелить без конфликта файлов

На снимке **оба writer-слота заняты** (`L-02-UI-IMPL`: `ui/**` +
`tests/unit/dispatch_ui.test.mjs`; `C-02-BEFORE`: `data.rs`/`messages.rs`). Ниже —
очередь, а не разрешение запускать третьего writer'а. Перед назначением PM обязан
сверить `ORCA-WORKERS.md` и `MERGE-MANIFEST` и убедиться, что файл свободен.

### 6.1 Можно **сейчас** (read-only/свои доки, продукт не трогают)

| Кому | Задача | Файлы | Конфликт |
|---|---|---|---|
| G3 | добить спеку G-05 (9 дефектов §0.4, disabled-причины, геометрия) | `DESIGN/G05-REST-SPEC.md` | нет |
| G4 | QA-мутация redaction `session_id` (лог/скрипт, без product) | `QUALITY/*`, отдельный лог | нет |
| G4 | e2e-prep: браузер/`CHROME_BIN`, план counts-приёмки | `QUALITY/QA-E2E-PLAN.md` | нет |
| G4 | SEC-L02 аудит | `QUALITY/SEC-L02.md` | нет |
| G5 | voice prereq/OPUS-KAT (read-only) | `VOICE/*` | нет |
| G6 | этот отчёт + parity-доки | `PARITY/*` | нет |

### 6.2 Кодовая очередь (по одному writer'у на файл)

| # | Задача | Owner | Файлы (монополия) | Предусловие | Конфликт, которого нельзя допустить |
|---:|---|---|---|---|---|
| 1 | **G-01b contract** | G1 | `settings_store.rs`, `settings_contract.rs` | — | не трогать `app.js` (это G2) |
| 2 | **TYPING_START forward** | G1 | `net.rs` (`FORWARDED_DISPATCHES`), `net_contract.rs` | L-02-CORE accepted, `net.rs` свободен | не параллелить с другими forward-правками; `[L]`-кадр как оракул |
| 3 | **PLAT-11a** channelId allowlist | G1 | `main.rs` (+ тест) | L-02-CORE md5 зафиксирован | `main.rs` занят L-02-CORE до приёмки |
| 4 | **G-05 фиксы #5/#10 + 9 дефектов** | G2 | `actions.js`, `gui-spike-features.test.mjs` | спека принята/поправлена | `actions.js` нужен и M-07-UI — не параллелить |
| 5 | **M-07 badge projection** | G1 | `data.rs`, `read_state_contract.rs` | C-02-BEFORE принят | `data.rs` строго последовательно (C-02-BEFORE) |
| 6 | **M-07 unread UI** | G2 | `chat.js`, `read-state-ui.test.mjs` | G1-проекция + L-02-UI | `chat.js` занят C-02-PAGE/M-06/L-02-UI |
| 7 | **G-01b UI restore** | G2 | `app.js`, `guild-rail.test.mjs` | G1-контракт | после L-02-UI (тот же `app.js`) |
| 8 | **R-01/R-02 фикс** | G2 | `app.js`, `search-overlay.js`, `quick-switcher.js` | L-02-UI принят | последовательно с #7 (общий `app.js`) |
| 9 | **e2e fixture** | G4 | `tests/e2e/**`, `tests/helpers/browser.mjs` | браузер в среде; правка `gate.mjs` — TL | не менять unit-тесты; `scripts/gate.mjs` — зона TL |
| 10 | **C-02-BEFORE** (если вариант B) | G1 | `data.rs`, `messages.rs`, `data_contract` | решение PM | уже активен; не трогать `chat.js` (UI готов) |
| 11 | **L-02-UI** | G2 | 3 новых `dispatch-*.js`, `chat.js`/`members.js`/`reactions.js`/`app.js` узко, `tests/unit/dispatch_ui.test.mjs` | приёмка L-02-CORE | уже активен; `app.js`/`chat.js` — монополия |

### 6.3 Безопасные пары (разные файлы, разный риск)

- `#1 settings_store.rs` + `#4 actions.js` — независимы.
- `#2 net.rs` + `#9 tests/e2e/**` — независимы.
- `#3 main.rs` + `#1 settings_store.rs` — независимы, но оба G1: один `cargo` на target-dir.
- `#5 data.rs` + `#7 app.js` — можно **только** после освобождения `data.rs` (C-02-BEFORE)
  и `app.js` (L-02-UI); между собой не конфликтуют.
- **Не параллелить:** `app.js` с L-02-UI/G-01b-UI/R-01/R-02/P-03; `chat.js` с
  C-02-PAGE/M-06/M-07/L-02-UI; `data.rs` с C-02-BEFORE/M-07; `net.rs` с любой второй
  forward-правкой; `actions.js` с M-07-UI; `tests/unit/*` — только владелец карточки.

---

## 7. Проверка сходимости (независимый оракул — markdown)

```bash
cd waves/RW-40/PARITY
sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3);
  gsub(/^ +| +$/,"",app); gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app);
  a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
```

Фактический вывод (2026-10-24, этот проход):

```text
app absent 34
app partial 36
app implemented 21
prio P0 5 / P1 32 / P2 40 / P3 14
total 91
```

Сверка ID: 91/91, без пропусков и дублей; разбивка по доменам:
`A 9 · G 9 · C 6 · M 14 · MB 4 · P 5 · V 11 · S 4 · AT 3 · ST 8 · N 5 · PL 8 · L 2 · P0 3`.

Strict-раскладка этого прохода: **15 / 42 / 34** (сумма 91); пограничная альтернатива
**17 / 40 / 34** — обе сходятся, выбор трассировки за PM.

---

## 8. Ограничения и что этот отчёт НЕ доказывает

1. **Не запускались** `cargo`, `npm`, `node --test`, `gate`, e2e, браузер, CDP, сеть,
   Discord, live, мутации. Числа тестов — из `.gate/*.json` и отчётов карточек.
2. **Снимок нестабилен:** активны два writer'а (L-02-UI, C-02-BEFORE), дерево dirty,
   `rust/gui-spike/**` и `rust/crates/discord-core/src/gateway/**` вне git-базы — диффом
   не проверяются, только md5.
3. `implemented` здесь **не** означает live-паритет: E3 = 0.
4. `[M]`-строки (V-08 speaking, G-05-заглушки, ST-01 подпункты, N-03) паритетом не
   считаются; `[A]`-строки (TTL typing, live-коды invite/leave, порог idle, consent) —
   авторские решения, не RE-факты.
5. Отчёт не закрывает ни карточки, ни волну, ни waiver; `QUALITY/BUGS.md`, `TASKS.md`,
   `QUEUE.md`, `STATE.md`, `db.jsonl`, `MERGE-MANIFEST.md`, `PARITY-MATRIX.md`,
   `GAP-BACKLOG.md` не изменялись (зона PM/TL).
6. `MERGE-MANIFEST` и `QUEUE` устарели относительно факта (P-03 UI, C-02-PAGE,
   L-02-CORE); синхронизация — PM/TL, не G6.

### Снимок md5 (15:27)

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/ws.rs` | `176791cc4e9a85173e8642cab02baa9b` |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` |
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` |
| `rust/gui-spike/src/main.rs` | `c003a9e597eea659beeff5d52e94768c` |
| `rust/gui-spike/ui/app.js` | `78039af36b09fa94e5d92b207f5ce977` |
| `rust/gui-spike/ui/features/chat.js` | `f4c8076386755a76efce9166dba71e50` |
| `rust/gui-spike/ui/features/typing-indicator.js` | `09f9f53870b669a382d52ecadcabb799` |
| `tests/unit/channel-paging.test.mjs` | `54184e41ce632d3fdb47cec9672095e0` |
| `PARITY/PARITY-MATRIX.md` | `c0789f87d4a9fd5f488e56cf1bc8027e` |
| `PARITY/GAP-BACKLOG.md` | `30be4326ab42deefa249309dc5435102` |
| `PARITY/G6-SYNC-01.md` | `8ce7ebda9bbedc0ac0fe14f29c6bfce6` |
| `PARITY/G6-RECHECK-01.md` | `9fff8f06ac1510b95a978c1d295a0b56` |
| `PARITY/G6-RECHECK-01-VERIFY.md` | `cf29d2da63951c6836a72351d7a04bd4` |
| `QUALITY/BUGS.md` | `056ae00dd09ea0000707f6a75d691075` |
| `MERGE-MANIFEST.md` | `0f24e7b44f7c0685860f8d325096fbcc` |

md5 самого отчёта считает координатор после записи (self-hash невозможен).
