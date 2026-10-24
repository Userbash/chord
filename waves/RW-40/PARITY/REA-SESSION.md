# REA-SESSION — журнал G6 (RW-40)

Дата: 2026-10-24. Оператор: G6 Lead / REA operator (subagent). PM: DeepSeek.
Режим: **read-only, без live, без запуска Discord, без сети, без cargo, без commit**.

## 1. Цель сессии

Инвентаризировать функции установленного Discord 1.0.166 и дампа 1.0.162, сравнить с нашим
приложением (`rust/gui-spike/ui`, `rust/gui-spike/src`, `rust/crates/**`) и выпустить
`FUNCTION-INVENTORY.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`, evidence.

Уровни evidence: `[L]` live-захват · `[S]` call-site в коде эталона · `[D]` только Enum/путь ·
`[M]` измерение (DOM/скрин/каталог) · `[A]` нет подтверждения. Поле `confirmed` из
`rest-static.json` не используется: оно `false` у всех 904 записей, включая live-эндпоинты.

## 2. Что выполнено (инструменты)

| # | Действие | Инструмент/команда | Результат |
|---|---|---|---|
| 1 | Статический анализ `app.asar` 1.0.166 | `rea.analyze_javascript_application(input_path=…, format=asar, detail=summary)` | `ev_1869…c96`; 977 модулей, 506 874 AST-узла, 3214 findings; coverage partial |
| 2 | Разведка graph: IPC-каналы | `rea.inspect_analysis_view(kind=javascript-facts, kinds=[ipc-channel])` | 0 записей: Electron API в бандле не распознан (см. §5) |
| 3 | Точные смещения 1.0.166 | read-only распаковка asar в `/tmp/opencode/ref166` + `grep`/Python `find` | `open-url@23596`, `openOrQueueUrl@22606`, `DESKTOP_CAPTURER@23953`, `NOTIFICATIONS@63242`, 203 IPC-константы |
| 4 | Инвентарь 1.0.166 | `sha256sum`, `ls -l` модулей | voice 127,8 МБ, krisp 62,1 МБ, utils 23,7 МБ |
| 5 | Дамп 1.0.162 | чтение `/home/sanya/re-deep/**`, `docs/re/**` | REST/gateway/voice/UI/i18n/main-arch |
| 6 | Наше приложение | `rg`, `sed`, `wc` по `rust/gui-spike/**`, `rust/crates/**` | реализовано/partial/absent по 91 функции |
| 7 | Экспорт REA | `rea.export_evidence_bundle(path=…/EVIDENCE/REA-EVIDENCE.json)` | 56 923 673 Б, 3 records, 0 unknowns |
| 8 | Санитизация вывода | bundle (57 МБ) вынесен в `/tmp/opencode/rea-evidence-rw40.json` | это нормально: в репозитории только выдержки/хеши, полный граф не коммитится |

Никаких `open_binary`, `inspect_electron_page`, `list_electron_targets`, live-сессий,
нажатий, запусков приложения не было. `device.spawn`/attach не вызывались.

## 3. Evidence

| Артефакт | Значение |
|---|---|
| REA evidence id | `ev_18693603b46870b9d3a9f78abfb5d626f5f9ebbd90632e57ef930d147c88396c` |
| app.asar sha256 | `0b29757f8b8194092e9ea57eb45182694a8c92fb5011e0cbc1eff0a8a690cdf1` |
| core.asar sha256 | `e24a5b8393384dc885f5e1dabb48d8b6833731f01c40460c99815a9169a12770` |
| Discord ELF sha256 | `37d7e3540277d878985c8f973e2313ada9601adb5bd8116e29f5bb4201ca2809` |
| build_info | stable / 1.0.166 / `stable-linux-x64` |
| NETWORK-CONTRACTS sha256 | `35cf9d0ec2a2485699c881e035ea7908f2af6c1d6ee70522e5a41bd8b2a46fcb` |
| Полный REA bundle | `/tmp/opencode/rea-evidence-rw40.json`, 56 923 673 Б, sha256 `1a7583cc…39f22` |

Выдержки: `EVIDENCE/ref-166-excerpts.md`, `EVIDENCE/ref-162-ui-excerpts.md`.
Хеши: `EVIDENCE/hashes.txt`. Границы: `EVIDENCE/README.md`.

## 4. Маршрут анализа

1. **Устав/план** → `G6-CHARTER.md`, `NETWORK-ROADMAP.md`, `FULL-REPORT.md`, `TASKS.md`.
2. **Наш код** → `ui/app.js`, `model.js`, `features/*` (settings/members/actions/chat/
   search-overlay/composer/message-toolbar), `src/main.rs` (27 Tauri-команд), `net.rs`,
   `messages.rs`, `data.rs`, крейты `discord-core` (dispatch/rest/state), `discord-voice`,
   `discord-platform`, `discord-i18n`, `discord-media`, `discord-updater`.
3. **Эталон 1.0.166** → REA summary → распаковка asar (read-only) → точные смещения IPC/deep links.
4. **Дамп 1.0.162** → REST-таблица, gateway-счётчики, voice-сессия, UI-трейсы (меню сервера,
   каналы, голосовая панель, настройки), i18n-каталог.
5. **NETWORK-CONTRACTS** → `waves/RW-40/NETWORK-CONTRACTS.md`: join/leave/invite/typing/
   members-search/Go Live; уровни `[L]/[S]/[D]/[M]/[A]`.
6. **Сверка** → 91 строка матрицы (90 → 91: добавлена G-03a), 5 P0 / 32 P1 / 40 P2 / 14 P3.

## 5. Покрытие и ограничения REA

| Факт | Значение |
|---|---|
| Файлов в asar | 5 (bundle.js, splash preload, data/package, ассеты) |
| JS-файлов разобрано | 2 |
| Модулей (webpack/rspack) | 977 |
| AST-узлов | 506 874 |
| Findings | 3 214 |
| Semantic unknowns | 20 502 (call-flow/object-flow/data-flow) |
| Electron IPC (REA) | 0 операций/каналов — статический анализатор не увидел Electron API в этом бандле |
| graph.coverage | partial |

**Следствие:** IPC-инвентарь (203 константы) получен из `core.asar:mainScreenPreload.js`
текстовым разбором (E1), а не из semantic graph. Это не ослабляет evidence (файл и смещения
хешируются), но означает: «0 каналов у REA» ≠ «каналов нет».

## 6. Version-gap

| Слой | Версия | Что подтверждает |
|---|---|---|
| Desktop main/preload, deep links, IPC, tray, capture | **1.0.166** | E1 |
| Renderer UI (серверы, каналы, сообщения, участники, настройки, голосовая панель) | **1.0.162** | E3 (dump) |
| REST/gateway/voice live | **1.0.162** | E3 |

Расхождения не сглаживались: в матрице UI-строки имеют `F162/UI2/UI6/VUI`, desktop-строки —
`A166/C166`. Для 1.0.166 renderer не снимался.

## 7. Ключевые находки

1. **203 IPC-события** 1.0.166: notifications, tray, capture stream, desktopCapturer,
   power/lock-screen, spellcheck, WebAuthn, badge/progress, window controls.
2. **Deep links**: `open-url`/`second-instance`/`openOrQueueUrl`; `setAsDefaultProtocolClient`
   в asar нет (регистрация установщиком).
3. **Голос**: main не грузит `discord_voice`; движок в renderer/preload, нативный модуль
   127,8 МБ + Krisp 62,1 МБ; трей умеет mute/deafen/voice settings.
4. **Go Live**: UI-кнопка и capture IPC — `[D]`; gateway `STREAM_*`/stream_key/viewer — `[A]`.
   Gateway-события в дампе отсутствуют; контракт не подтверждён.
5. **Вложения**: путь `/channels/{id}/attachments` — `[D]` (Enum); метод/multipart/поля/лимиты — `[A]`.
   `attachments` в MESSAGE_CREATE всегда `null` → схема `[A]`.
6. **Guild lifecycle**: enter из preview `PUT /guilds/{id}/members/@me?lurker=true` + `{}` → **204** — `[L]`;
   join invite тело `{session_id,invite_instance_id,received_installation_id}` — `[S]`; leave
   `DELETE /users/@me/guilds/{id}` + `{lurking}` — `[S]`; live-коды/TTL — `[A]`.
7. **i18n**: 23 836 ключей, 31/36 локалей, собственный рантайм; наш `discord-i18n` — 16 523
   строки, но не подключён к UI.
8. **DND**: статус и «режим концентрации» — `[S]` (строки); правила подавления — `[A]`.

## 8. Чего в сессии не было

- запуска Discord/Electron/live-окна;
- CDP/AT-SPI/скриншотов 1.0.166;
- сетевых запросов, логина, токенов;
- `cargo`/`npm`/тестов (запрещено условием; использованы отчёты прошлых волн);
- записи в `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep`, установленный Discord;
- изменений продуктового кода/чужих документов.

## 9. RE-гейты, открытые после сессии

1. **Upload**: метод/multipart/поля/лимиты/refresh-urls — `[A]` (AT-01…03).
2. **Invite/leave live-коды**: 200/204/4xx, TTL `received_installation_id`, owner-leave — `[A]`.
   Тело join — `[S]`, enter — `[L]` 204, leave — `[S]`; их можно кодировать.
3. **Go Live**: `STREAM_*`, stream_key, rid/quality, viewer — `[A]` (S-02/03).
4. **members-search pagination**: 202 retry — `[S]`; пагинация/коды — `[A]` (MB-04).
5. **DND**: правила подавления/уровни — `[A]` (N-02/05).
6. **create server**: endpoint/call-site — `[D]/[A]` (G-02).

До закрытия гейтов строки уровня `[A]` не кодируются. G-03a (`[L]` 204) и тела join/leave
(`[S]`) можно реализовывать с фикстурами; live-коды проверяются в окне N1.

## 10. Воспроизводимость

```bash
# 1. REA summary (MCP)
# analyze_javascript_application(input_path="/home/sanya/.config/discord/app-1.0.166/resources/app.asar",
#   format="asar", detail="summary")

# 2. хеши
sha256sum /home/sanya/.config/discord/app-1.0.166/resources/app.asar \
          /home/sanya/.config/discord/app-1.0.166/modules/discord_desktop_core-1/discord_desktop_core/core.asar

# 3. смещения 1.0.166 (read-only распаковка в /tmp)
python3 - <<'PY'
s=open('/tmp/opencode/ref166/core/mainScreenPreload.js',encoding='utf-8',errors='replace').read()
for p in ('DESKTOP_CAPTURER_GET_SOURCES','NOTIFICATIONS_SEND_NOTIFICATION','ACTIVITY_CAPTURE_STREAM_START'):
    print(p, s.find(p))
PY
```

---

## 11. Сессия 2 — G6-REPORT-02 (decision-ready), 2026-10-24

Режим тот же: **read-only**, без live/окна/сети/`cargo`/commit. Продуктовый код не менялся.

### 11.1 Что сделано

| # | Действие | Источник | Результат |
|---|---|---|---|
| 1 | Сверка 12 приоритетных функций с текущим кодом | `rust/gui-spike/{ui,src}`, `tests/**` | состояние + файлы + gap по каждой |
| 2 | Проверка привязки UI к BFF | `grep` по `ui/**` | UI **не вызывает** `ready`, `messages_page`, `members_page`, `settings`, `search_page`, `join_guild`, `net_*` |
| 3 | Уточнение настройки «63 строки» | `/home/sanya/re-deep/ui8/23-settings-categories.json` (`count:63`) | 63 = профиль 2 + 19×3 обёртки + 4 подпункта; навигация = профиль + 18 разделов + «Выйти» |
| 4 | Уточнение invite-тела | бандл 1.0.162, anchor `_.Bo.post({url:ei.Rsh.INVITE`; `getSessionId():null` | все 3 поля nullable; `invite_instance_id` — `_U(baseCode, message_id)`, опционален |
| 5 | Уточнение ack-тела | `docs/re/rest-d02.md §4`, `fixtures/.../046-MESSAGE_ACK.json` | `{token:null,last_viewed:4310}`; `last_viewed` — счётчик read_state, не id сообщения |
| 6 | Проверка route-каталога | `crates/discord-core/src/rest/routes.rs` | `/typing` и `/invites/{code}` в каталоге 49 записей отсутствуют → нужен письменный протокол расширения |
| 7 | Выход | `PARITY/G6-REPORT-RW40-02.md` | отчёт: 12 карточек, топ-5, блокеры, карта файлов, риски |

Ничего не запускалось: ни окно, ни live-сеть, ни `cargo`/`npm`, ни CDP/AT-SPI.

### 11.2 Находки, меняющие матрицу

1. **G6-D-01:** `channelId: "news"` и `messageId: String(index)` в UI (`chat.js:432,495,509`,
   `app.js:160`) — live send/edit/delete гарантированно уйдут в несуществующий канал/сообщение.
   Это блокирует приёмку P0-02, M-03/M-04; в матрице строка M-02 стоит «live unverified»,
   хотя отказ детерминирован.
2. **ST-01:** формулировка «нет 45 подпунктов/панелей» неверна; реальный gap — 7 строк
   (профиль, 4 подпункта, «Выйти»), объём S.
3. **G-03a:** backend принят, но UI-входу нужен `guildId` из preview → без G-03 resolve
   или deep link кнопка не имеет источника; последовательность G-03 → G-03a.
4. **G-01:** источник списка — `[L]` READY.guilds, а не только `[M]`-замер рейла.
5. **N-01:** crate уведомлений готов и покрыт тестами, но `Notifier` нигде не подключён,
   а диспатчи в UI не идут (L-02) → делить на N-01a (policy+adapter) и N-01b (wiring).

### 11.3 Ограничения сессии 2

- renderer 1.0.166 не снимался: все UI-«паритеты» — по 1.0.162;
- invite/leave live-коды, upload, Go Live, voice media не проверялись (нет окна/токена);
- геометрия join/leave/voice-попапов остаётся `unmeasured` (`SERVER-MENU.md §0`, `VOICE.md §4.3`);
- правки `PARITY-MATRIX.md`/`GAP-BACKLOG.md` **не вносились** — предложены списком в §8 отчёта,
  применяет PM после сверки.

---

## 12. Сессия 3 — G6-BATCH-MERGE-01 (приёмка batch), 2026-10-24

Режим тот же: **read-only**, без live/окна/сети/`cargo`/`npm`/commit. Продуктовый код не менялся.
Выход — `PARITY/G6-BATCH-MERGE-01.md`.

### 12.1 Что сделано

| # | Действие | Источник | Результат |
|---|---|---|---|
| 1 | Сверка 23 accepted-карточек batch с текущим кодом | `PM/*/REPORT.md`, `ui/**`, `src/**`, `rust/crates/**` | состояние/остатки по каждой; P0-дефектов нет |
| 2 | Проверка route-расширений | `routes.rs`, `typing_contract.rs`, `invites_contract.rs`, D-02 TSV | `/typing` и `/invites` — отдельные `[S]`-оракулы; `ROUTES`=49, TSV md5 `2db6b39b…` |
| 3 | Проверка READY-проекций | `channels_contract.rs`, `read_state_contract.rs`, `002-READY.json` | `channels` (272+17) и `read_state` (157) сверены с READY; лишних полей нет |
| 4 | Проверка закрытия G6-D-01/D-02 | `chat.js`, `channel-switch.test.mjs` | live-путь на `item.id`/реальном `channelId`; `"news"`/индекс — только bench |
| 5 | Аудит целостности тестов | `tests/**`, gate JSON | `.only`=0; e2e 4 skip ≠ pass; найден md5-drift 6 тестовых файлов (P2) |
| 6 | Проверка журнала | `waves/db.jsonl`, `waves/STATE.md` | 0 событий RW-40; STATE от 2026-10-23 — процессный долг Team Lead |
| 7 | Выход | `PARITY/G6-BATCH-MERGE-01.md` | вердикт `deferred`, 5 до merge / 5 после, QA handoff |

### 12.2 Находки, меняющие план

1. **P-03 — backend-only**: UI индикатора и `send_typing` отсутствуют; строку матрицы нельзя
   переводить в `implemented`.
2. **M-06 в реальном канале**: `renderChannelMessage` не рендерит реакции, быстрые реакции
   скрыты, `reactions.js` остаётся на bench `"news"` — функциональный регресс среза C-02.
3. **MD5-drift** тестов `guild-rail`, `channel-switch`, `read-state-ui`, `gui-spike-features`,
   `gui-spike-model` относительно отчётов карточек; счётчики кейсов совпадают, гейт зелёный,
   но отдельного red/green-лога на дрейф нет (baseline вне git).
4. **Матрица/BUGS не синхронизированы**: G6-D-02…D-05 не заведены, строки batch в
   `PARITY-MATRIX.md` всё ещё описывают состояние до merge.
5. **e2e 4 skip** (нет `CHROME_BIN`/`CDP_ENDPOINT`) остаются skip; `gate:release` —
   `passed_with_skips`, не `passed`.

### 12.3 Ограничения сессии 3

- тесты не перезапускались (числа — из `.gate/release-2026-10-24T09-19-36-946Z.json`);
- live-коды invite/leave, ack 200, typing 200, keyring-live — не проверялись;
- `rust/gui-spike/**` вне git: дифф «до/после» карточек недоступен, сверка по md5/счётчикам.
