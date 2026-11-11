# SEC-L02 — security review L-02 (dispatch → `gateway://dispatch` → UI)

Дата снимка: **2026-10-24 16:12 +05**. Ревьюер: **G4b Security reviewer** (READ-ONLY).
HEAD `0239543`, рабочее дерево грязное: L-02 не закоммичен (`net.rs`, `main.rs`, `ui/**`,
`tests/unit/dispatch_ui.test.mjs`, `gateway_supervisor.rs` — untracked/modified).
Сеть, живой Discord, окно и `cargo` не запускались; код, тесты, доки и golden-каталоги
не менялись. Записан только этот файл.

Предмет: `waves/RW-40/PM/l02-core/{REPORT.md,FIX-REPORT.md,REVIEW.md,REVIEW-FIX.md}`,
`waves/RW-40/PM/l02-ui/{REPORT.md,REVIEW.md,FIX-REPORT.md,fix-*.log}`,
`CONTRACTS/dispatch.md`, `rust/gui-spike/src/{net.rs,main.rs}`,
`rust/gui-spike/ui/features/dispatch-{model,view,bridge}.js`, `ui/app.js`,
`tests/unit/dispatch_ui.test.mjs`, `rust/gui-spike/tests/net_contract.rs`,
`rust/crates/discord-core/src/{dispatch.rs,ws.rs,gateway/supervisor.rs,ready.rs,resume.rs}`,
`.gate/*.json`, `waves/RW-40/**`, `fixtures/gateway/by-type/*`.

## 0. Вердикт

| Класс | Кол-во |
|---|---|
| **P0** | **0** |
| **P1** | **1** |
| **P2** | **7** |
| **P3** | **10** (таблица §4) |

**RETURN.** Утечки перечисленных в ТЗ READY-секретов (`session_id`, `token`,
`access_token`, `refresh_token`, `qos_token`, `resume_gateway_url`, `auth_session_id_hash`)
в событие/DOM/лог/артефакт не найдено; обхода аутентификации нет. Блокирует приёмку
**P1 SEC-L02-01**: дедуп `s` переживает новый `connect`, поэтому после `logout → login`
READY и префикс новой сессии молча уходят в `duplicates`; UI остаётся на состоянии
прошлой сессии. Остальное — минимизация READY, backpressure без приоритета,
`Debug`-канал к сырому `d`, гигиена `session_id`, слабость redaction-evidence,
тихий no-op `connect` после терминального Stop и разрыв evidence-цепочки.

## 1. Снимок проверенного (md5, 16:12)

| Файл | md5 сейчас | Что в отчётах | Статус |
|---|---|---|---|
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` | `e13d4e8…` | совпадает |
| `rust/gui-spike/src/main.rs` | `336863b5b1ffca169b851a2095798f01` | `c003a9e…` (`REPORT.md`) | **дрейф** (правка комментариев 15:29, в FIX-REPORT не внесена) |
| `rust/gui-spike/tests/net_contract.rs` | `fe8aa92146e9dcf8b943335dfad652c8` | `fe8aa92…` (FIX-REPORT) | совпадает |
| `rust/crates/discord-core/src/ws.rs` | `8703c1d695cd0af26458393eb3c1e3f9` | `8703c1d…` (FIX-REPORT) | совпадает |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` | `5180b63…` | совпадает |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `7f23d85a4d3b0e54c58d765b8b879c58` | `7f23d85…` (FIX-REPORT) | совпадает |
| `rust/gui-spike/ui/app.js` | `1ac5c36976a064120529c132e61cfb54` | `1ac5c36…` (UI FIX-REPORT) | совпадает |
| `ui/features/dispatch-model.js` | `37dacc61d7daf74c144681637dded1f5` | `37dacc6…` (UI FIX-REPORT) | совпадает |
| `ui/features/dispatch-view.js` | `fd1842ac442dae108f80b3872f163f05` | `fd1842a…` | совпадает |
| `ui/features/dispatch-bridge.js` | `1cad6f1324b2fe508866d29097792443` | `1cad6f1…` | совпадает |
| `tests/unit/dispatch_ui.test.mjs` | `f2806f3f4088b0527f6a31ff1d7bce2a` | `f2806f3…` (UI FIX-REPORT) | совпадает |
| `CONTRACTS/dispatch.md` | `4bfeef6c5851b994b75d4cd3d8a32ab1` | `4bfeef6…` | совпадает |

`rust/gui-spike/**` и `tests/unit/dispatch_ui.test.mjs` вне git-базы (`git ls-files` → 0),
поэтому md5 — единственный слепок; UI FIX-REPORT §4 привязан к текущим файлам, основной
`l02-ui/REPORT.md` — нет (37/37, старые хеши) — см. SEC-L02-08.

## 2. Ответы на вопросы ТЗ

### 2.1 Секреты в event/DOM/log/artifact — **не попадают (P0 нет)**

- Политика: точные `session_id`, `resume_gateway_url`, `auth_session_id_hash`,
  `client_secret`, `password`, `token`, `access_token`, `refresh_token` + суффикс
  `_token` (`net.rs:172-184`), рекурсивно по объектам/массивам (`net.rs:221-242`),
  ко всем 6 форвардящимся типам (`net.rs:113-120`, `248-262`). `qos_token` и
  `analytics_token` попадают под `_token`.
- В Tauri уходит ровно `{kind,data}` (`net.rs:133-139`), `DispatchStats` payload не
  содержит (`net.rs:455-484`), эмиттер не логирует (`main.rs:62-70`), в
  `net_dispatch_stats` нет ни `println!`, ни чтения токена (`main.rs:654-657`).
- Токен драйвера в payload не попадает: источник — только `d`; `IdentifyParams`/
  `ResumeParams` редактируют токен в `Debug` (`identify.rs:154-170`, `resume.rs:32-41`),
  `SupervisorConfig::Debug` печатает `<handler>` (`supervisor.rs:98-114`).
- В `.gate/*.json` — **0** вхождений имён `session_id`/`access_token`/`refresh_token`/
  `qos_token`; в логах `waves/RW-40/**` нет значений токенов. В фикстурах значения
  секретов уже `"REDACTED"` — это ограничение доказательства, см. SEC-L02-06.
- `unsafe`/FFI/`no_mangle` в `net.rs`, `main.rs`, `ws.rs`, `supervisor.rs` — 0;
  `println!` в проверяемых файлах есть только в `report_bench`/`report_ui_snapshot` и
  не получает payload dispatch (`main.rs:92,99`, снапшот — только счётчики, `app.js:1486`).
- Соседний канал `auth://state` проверен на границе: `AuthState` не содержит токена
  (`auth.rs:234-245`).

### 2.2 `try_send`/overflow/backpressure — **источник не блокируется, но security-значимое теряется без приоритета**

- Источник не блокируется: отдельный поток `gateway-dispatch`, `try_send`
  (`net.rs:292-368`); переполнение → `overflow += 1` + `lastError`; тест
  `blocked_ui_channel_never_blocks_the_gateway_source` (`net_contract.rs:707-742`).
- Потеря наблюдается **только агрегатно** и только если кто-то вызовет
  `net_dispatch_stats`: `overflowByKind` нет, READY не имеет приоритета и может быть
  выброшен; UI **не вызывает** `net_dispatch_stats` (grep по `ui/**` → 0), поэтому
  `overflow`/`emitFailed` не видны пользователю и нет resync — P2 SEC-L02-04.
- `deliver()` возвращает `bool`, но `on_dispatch` его игнорирует (`net.rs:431-433`);
  решение «повторить/сообщить» не принимается, потеря видна лишь в счётчике.

### 2.3 UI payload/секреты; self ID — **payload не рендерится; self ID исправлен**

- Модель не хранит `data` целиком: проекции берут только `guilds`/`user.id`, поля
  участников, presence, реакции (`dispatch-model.js:469-824`). `dispatch-view.js`
  пишет через `textContent`/`setAttribute`; `innerHTML`/`insertAdjacentHTML` во всём
  `ui/**` — **0** (единственное упоминание — комментарий `dom.js:7`).
- Снапшот содержит только счётчики (`app.js:1486`); bridge-статистика — числа без
  payload (`dispatch-bridge.js:76-93`, `208-210`); ошибки model/view изолированы и не
  логируют payload (`dispatch-bridge.js:127-153`).
- `selfUserId`: авторитетный источник — `ready()` IPC (`app.js:1189-1196`) через
  `asSnowflake`; READY dispatch — fallback (`dispatch-model.js:81-85`, `488-489`);
  `me` реакций сравнивает строки (`dispatch-model.js:814`). Unsafe integer отвергается
  (`""`), тесты B17/C16, пустой READY — B16/B16b/C15/C15b; итого 43/43.
- Остаток: прочие ID (`guild_id`, `user.id` участника, `user_id` реакции) проходят
  через `asString`, не `asSnowflake` (`dispatch-model.js:63-71,346,723,779`). В
  наблюдаемых кадрах ID — строки, поэтому P3, но unsafe numeric ID на границе IPC
  потеряет точность.

### 2.4 Тесты/доказательства — **red-first есть, независимость слабее заявленной**

- Core FIX закрыл оба P1 из `REVIEW-L02-CORE`: `flush_dispatches` вызывается **до**
  `?` (`ws.rs:648-656`, функция `701-710`), red-лог `fix-red-core-flush.log`
  (`left: []`, `right: ["READY"]`), green `fix-green-core-targeted.log` (14/14),
  `fix-green-net-contract.log` (23/23). `REVIEW-FIX.md` — ACCEPT.
- UI FIX закрыл оба P2 из `REVIEW-L02-UI`: пустой READY — полный снапшот
  (`dispatch-model.js:492-517`), self ID — `asSnowflake`; red `fix-red.log` (4 fail),
  `fix-red2.log` (2 fail) → green 43/43 (`fix-green.log`). **`FIX-REPORT.md`
  существует** (15:54) и привязан к md5; устарел основной `REPORT.md` (37/37) —
  SEC-L02-08.
- Redaction-evidence: READY-тест (`net_contract.rs:476-521`) и UI B15/C9
  (`dispatch_ui.test.mjs:445,1096`) проверяют отсутствие значений, но значения в
  фикстуре уже `"REDACTED"`. Реально доказано отсутствие **ключей**. Синтетический
  `exact_token_and_nested_auth_tokens_are_redacted_and_counted` (`:524-566`) —
  единственный кейс с реальными значениями.
- `.only`/`.skip`/`#[ignore]` в назначенных тестах нет; `fixture()` skip-aware
  (`net_contract.rs:131-147`).

## 3. Findings P1/P2

### SEC-L02-01 — **P1**: дедуп `s` переживает новый `connect`; READY и префикс сессии теряются молча

**Где:** `net.rs:396-401` (`reset()` только по событиям), `403-417` (дедуп
`seq <= last`, при повторе `last_seq` не двигается), `764-767` (sink создаётся один раз
на контроллер), `813-854` (`connect` не сбрасывает состояние), `857-861` (`disconnect`
тоже), `827-835` (сброс только на `Ready`/`Reconnecting`); `supervisor.rs:283`
(`ready_emitted` — один раз на супервайзер), `308-312` (`SupervisorEvent::Ready`
эмитится **после** возврата `connect_and_run_resume`), `303-306` (при `shutdown` —
ранний `return` без `absorb`/`Ready`); `main.rs:539-557` (`auth_logout` действительно
зовёт `net.disconnect()`); `app.js:1373-1378` (logout снимает подписку, store/`selfUserId`
не очищает).

**Суть.** `CoreGatewayDriver::connect` создаёт новый супервайзер, но `DispatchSink`
(и его `last_seq`) живёт в `NetController` и между соединениями не сбрасывается.
`SupervisorEvent::Ready` приходит только по завершении сессии, а `disconnect` завершает
её через `Cancelled` — без `Ready`. Сценарий «выход → вход → connect»:

1. сессия A отдала `s = 1..N`, `last_seq = N`;
2. `auth_logout` → `net.disconnect()` → `last_seq` остаётся `N`;
3. новый `connect`, сессия B (IDENTIFY) начинает `s` с 1;
4. каждый диспатч B с `s <= N` уходит в `duplicates` и не доезжает до UI; READY
   (`s = 1`) теряется **всегда**; сброс придёт только с `Ready` в конце сессии B.

Условие: предыдущая сессия обработала хотя бы один dispatch (`last_seq = N ≥ 1`);
именно это и происходит на обычном logout после работы.

**Impact.** UI не получает READY новой сессии (гильдии/`user.id`/участники), в
dispatch-store остаются члены/presence прошлой сессии, `selfUserId` — от прошлого
`ready()`; `duplicates` маскирует потерю под «штатные повторы». Это потеря
security-значимых событий без сигнала, а не косметика.

**Почему тесты не ловят.** `FakeGateway::connect` эмитит `Ready` синхронно
(`net.rs:1003`), поэтому в тестах дедуп сбрасывается до первого dispatch. Боевой
супервайзер так не делает (`supervisor.rs:308-312`). `duplicate_sequences_are_dropped…`
(`net_contract.rs:745-778`) проверяет только `Reconnecting`.

**Repro (без сети, не добавлял в репозиторий).** Драйвер без `Ready` в `connect`:
`connect → dispatch(s=5) → disconnect → connect → dispatch(s=1)` — `forwarded` не растёт,
`duplicates` растёт. IPC-вариант: `auth_logout` → login → `net_connect` →
`net_dispatch_stats`: `delivered` стоит, `duplicates` растёт; READY в UI не появляется.
Теста, который это ловит, нет.

**Рекомендация (не вносил):** `sink.reset()` в `NetController::connect` (и в
`Closed(4004)`), либо epoch соединения в ключе дедупа; тест «connect → dispatch →
connect → dispatch(s=1)» на драйвере без `Ready`; очищать dispatch-store на logout.

### SEC-L02-02 — **P2**: сырой `d` (с `session_id`/`qos_token`) живёт в derived `Debug`

**Где:** `dispatch.rs:120-131` (`#[derive(Debug)]` на `DispatchEvent`, `data` — целиком);
`net.rs:85-92` (`GatewayEvent` derive `Debug`, вариант `Dispatch(DispatchEvent)`);
`net_contract.rs:862-866` (`assert_eq!` по `GatewayEvent` — при падении `d` уйдёт в лог);
`ready.rs:29-30` (`ReadySummary` derive `Debug` с `session_id`/`resume_gateway_url`);
`resume.rs:44-50` (`ResumeState` с `session_id`).

**Суть.** Инвариант «секреты не покидают backend» держится на дисциплине «никто не
печатает событие», а не на типе. Любой `{:?}`/`dbg!`/упавший `assert_eq!` печатает
READY `d` с `session_id`, `qos_token`, `auth_session_id_hash`. Канал не гипотетический:
`ReadySummary` уже печатал `session_id` + `resume_gateway_url` в
`waves/RW-05/mut/M8.txt:55-56`. Боевых мест печати сейчас нет (grep `{:?}`/`println!`
по `net.rs`/`main.rs`/`ws.rs`/`supervisor.rs` — чисто), т.е. это тип-барьер, а не
текущая утечка.

**Repro:** `format!("{:?}", route_dispatch(READY-кадр).unwrap())` содержит
`session_id: Str("…")`; падение `assert_eq!` в `net_contract.rs:862` печатает оба
`GatewayEvent` целиком.

**Рекомендация:** ручной `Debug` для `DispatchEvent` (`data: "<redacted>"`), как у
`IdentifyParams`.

### SEC-L02-03 — **P2**: READY `d` уходит в webview целиком, включая аккаунтную PII

**Где:** `net.rs:248-262` (`data = d` целиком), `113-120` (READY в списке);
`CONTRACTS/dispatch.md:53-56` фиксирует это как контракт; UI использует только
`guilds` + `user.id` (`dispatch-model.js:469-497`).

**Суть.** После redaction в payload остаются (по `002-READY.json`): `user.email`,
`user.phone`, `user.mfa_enabled`, `user.store_country`,
`user.age_verification_status`, `relationships`, `private_channels`, `read_state`,
`sessions[*].client_info`, `user_settings_proto`, `static_client_session_id`; размер
`d` ≈ **579 091 байт**. Для первого экрана это избыточно; при `csp: null` и
`withGlobalTauri: true` (`tauri.conf.json`) любой будущий XSS в webview получает эти
данные. Это не «чужая» утечка (своя учётка в своём окне), но нарушение минимизации на
самом security-значимом событии.

**Repro:** `jq -c .d fixtures/gateway/by-type/002-READY.json | wc -c` → `579091`;
`map_dispatch(route_dispatch(READY))` сохраняет `data.user.email`/`phone`.

**Рекомендация:** allow-list проекция READY (guilds/user.id/username/channels/
read_state по нужде) либо расширение политики на аккаунтные поля; решение зафиксировать
в §5 контракта.

### SEC-L02-04 — **P2**: backpressure дропает **новое** событие любого класса, без приоритета

**Где:** `net.rs:104` (capacity 256), `335-359` (`TrySendError::Full` → `overflow`,
событие выброшено), `431-433` (результат `deliver` не различается), `455-484` (только
агрегат, без типа).

**Суть.** При переполнении отбрасывается пришедшее событие — включая READY новой
сессии, пока UI разгребает хвост `GUILD_MEMBER_LIST_UPDATE` (в захватах 1048 кадров в
union). В статистике — только агрегат `overflow`, без имени типа; UI не показывает
degraded-состояние и не вызывает `net_dispatch_stats`; resync-пути нет. Побочно:
579 КБ × 256 слотов ≈ до 148 МБ удержания при зависшем webview. Инвариант
`forwarded == queued + overflow` нигде не закреплён (при `channel: None` он неверен).

**Repro:** `blocked_ui_channel_never_blocks_the_gateway_source`
(`net_contract.rs:707-742`): capacity 1, третий `deliver` → `overflow=1`, событие
потеряно; подставьте READY — та же картина.

**Рекомендация:** невытесняемый слот для READY/lifecycle или `overflowByKind`;
ограничить размер payload (SEC-L02-03); при `overflow>0` — видимый degraded-статус и
resync.

### SEC-L02-05 — **P2**: реальный captured `session_id` скопирован в новый тест-файл

**Где:** `rust/crates/discord-core/tests/gateway_supervisor.rs:43`
(`const SESSION_ID = "f14ac1f3d408c15027d6d98da172c398"`); то же значение в
`identify_ready.rs:192,538,720`, `resume_reconnect.rs:46`; в артефактах
`waves/RW-01/review/oracle-fresh.txt:7,10`, `waves/RW-05/mut/M8.txt:55-56`,
`waves/RW-05/fmt.txt:8`.

**Суть.** Это не синтетика: значение пришло из живого захвата (в том же capture —
`_trace gateway-prd-arm-us-east1-d`, hex кадра `SESSIONS_REPLACE`). Класс — session
capability (`CONTRACTS/dispatch.md:86-88`). Значение **уже было в HEAD**
(`identify_ready.rs`, `resume_reconnect.rs`), т.е. это унаследованный класс, а не
новая утечка L-02; но untracked `gateway_supervisor.rs` повторно его использует, и
L-02 не устранил нарушение гигиены. Практический риск ограничен (сессия давно
завершена, RESUME требует ещё и токен, которого в репозитории нет); `Debug`-канал
(SEC-L02-02) даёт ему путь в логи прогонов.

**Repro:** `grep -rn f14ac1f3d408c15027d6d98da172c398 rust waves`.

**Рекомендация:** заменить в новых тестах на явно синтетическую константу;
golden-артефакты не переписывать (append-only), зафиксировать класс в отчёте волны;
сессию считать непригодной.

### SEC-L02-06 — **P2**: redaction-evidence опирается на пред-редактированные фикстуры

**Где:** `net_contract.rs:359-386` (оракул `secret_paths` — свой список ключей, копия
`is_secret_key`), `476-521` (READY), `438-474` (вложенные `session_id`);
`fixtures/gateway/by-type/002-READY.json` и `005-…json` — значения секретов уже
`"REDACTED"`; `dispatch-model.js:444-457` (`countSecretKeys` — счётчик, не редактор).

**Суть.** Ассерты «значение не попало» (Rust и UI B15/C9) проверяют строку
`"REDACTED"`, т.е. фактически — отсутствие **ключей**. Утечка значения под
несекретным ключом (`sessionId`, `SessionId`, `qosToken`) не детектируется ни кодом
(регистрозависимое сравнение), ни тестом. UI-слой секреты не вырезает вообще — только
считает, причём **без учёта регистра** (`name = key.toLowerCase()`,
`dispatch-model.js:451-453`), тогда как backend сравнивает точное имя
(`net.rs:172-184`): ключ `Session_ID` UI посчитает «вырезанным», а значение уйдёт в
payload. Единственный барьер — `net.rs`.

**Repro (мысленный, без правок):** заменить в фикстуре `session_id` на
`SENTINEL-42` — текущие ассерты пройдут; добавить ключ `sessionId` со `SENTINEL-42` —
backend его сохранит, UI увеличит `redactedKeys`.

**Рекомендация:** sentinel-тест на сериализованный payload + инвариант «payload не
содержит подстрок значений секретов»; e2e через `DispatchChannel`+emitter;
документировать «UI считает, но не редактирует» и согласовать регистр.

### SEC-L02-07 — **P2**: `CoreGatewayDriver::connect` — тихий no-op после терминального Stop

**Где:** `net.rs:653-694` (при `state.is_some()` → `Ok(())`, поток не перезапускается;
`disconnect` — единственный, кто очищает `state`, `696-706`); `supervisor.rs:314-341`
(Stop по 4004/4013/4014/лимиту завершает поток, но `DriverTask` остаётся в `state`);
`main.rs:635-641` (кнопка «Повторить»).

**Суть.** После терминальной остановки супервайзера (лимит реконнектов, 4013/4014,
4004 + повторный вход) `net_connect` возвращает `Ok`, статус ставится `Connecting` и
навсегда застревает — ошибки нет, счётчиков нет, пользователь видит «Подключение…».
Ошибка глотается на границе UI, хотя внутри ядра она есть (`last_error`).

**Repro:** довести супервайзер до `Stop(ReconnectLimit)`/4013 → `net_connect` →
`Ok`, `net_status = connecting` без нового соединения (тест «connect после Stop»
отсутствует).

**Рекомендация:** `connect` должен проверять `thread.is_finished()` и пересоздавать
задачу (или возвращать типизированную ошибку).

### SEC-L02-08 — **P2**: evidence-цепочка не привязана к проверенному коду

**Где:** `l02-core/REPORT.md:94-106` (`main.rs` `c003a9e…` против фактического
`336863b…`; остальные core-хеши — состояние приёмки L-02 и разъяснены в §4 ниже);
`l02-ui/REPORT.md:49` (37/37) и `§4` (`154eec…`/`16411a3…`/`611876…`) против
фактических 43/43 и `37dacc6…`/`1ac5c36…`/`f2806f3…` (актуальные — в UI
`FIX-REPORT.md`); `CONTRACTS/dispatch.md:194` («14 тестов», фактически 23),
`:205-210` («`ui/**` не менялся» — уже неверно); последний gate —
`.gate/worker-2026-10-24T10-22-26-400Z.json` (`finishedAt 10:21:42Z`,
`verdict: passed_with_skips`, commit `0239543`): он **старше** core FIX (15:43) и
UI-фикса (15:52), т.е. **ни один gate-артефакт не покрывает L-02**.

**Суть.** Механизм red-first и FIX-отчёты в порядке, но зелёный артефакт нельзя
однозначно привязать к проверенному коду; `rust/gui-spike/**` вне git, поэтому md5 —
единственная привязка, а основной `REPORT.md` описывает уже несуществующее состояние.
Для security-ревью это ограничивает доверие к «зелёным» логам, а не свидетельствует об
утечке.

**Repro:** `md5sum` по таблице §1; `ls -t .gate/*.json | head -1`; `git ls-files
rust/gui-spike | wc -l` → 0.

**Рекомендация:** переснять md5 в момент приёмки, обновить `REPORT.md`/`§9` контракта,
приложить gate-артефакт, покрывающий L-02.

## 4. P3 (не блокируют, в бэклог)

| ID | Finding | Где |
|---|---|---|
| SEC-L02-09 | Асимметрия `Debug`: токен скрыт, `session_id` печатается | `resume.rs:32-50`, `ready.rs:29-30` |
| SEC-L02-10 | `static_client_session_id` вне политики; секрет внутри значения строки не ищется; регистр ключей | `net.rs:172-242` |
| SEC-L02-11 | Булевы атомы приходят строками `"true"/"false"` — ловушка для будущих проекций | `net.rs:194`, `CONTRACTS/dispatch.md:54-56` |
| SEC-L02-12 | Текст ошибок теряется: `Error(_)` → `Error`, `ReconnectLimit` → 4013, `join()` игнорируется | `net.rs:709-717,845`; `supervisor.rs:333-339` |
| SEC-L02-13 | `lastError` не сбрасывается на успехе — «залипшая» диагностика | `net.rs:312-318,344-347` |
| SEC-L02-14 | Безлимитная рекурсия `etf_to_json`/`redact_secrets`: экстремальная вложенность враждебного шлюза → stack overflow (abort) | `net.rs:191-218,221-242` |
| SEC-L02-15 | Dev-байпас входа `SPIKE_AUTH_DISABLED` в релизном бинаре (предсуществующее) | `main.rs:705-710`, `ui/features/auth.js:237` |
| SEC-L02-16 | `csp: null` + `withGlobalTauri: true` (предсуществующее); XSS-путь через dispatch сейчас не просматривается (нет `innerHTML`) | `tauri.conf.json`, `capabilities/default.json` |
| SEC-L02-17 | На logout `dispatch.stop()` не очищает store и оставляет `data-dispatch-source="live"`; защита — только следующий READY (а он теряется, SEC-L02-01) | `app.js:1373-1378`, `dispatch-bridge.js:192-201`, `dispatch-view.js:93-99` |
| SEC-L02-18 | Прочие ID (guild/member/reaction) проходят `asString`, не `asSnowflake`; unsafe numeric ID на границе IPC потеряет точность | `dispatch-model.js:63-71,346,723,779` |

## 5. Что проверено и подтверждено (положительно)

- **Redaction:** точные имена + `_token`, рекурсивно, ко всем 6 типам
  (`net.rs:172-242`, `248-262`); счётчик `redacted` без payload; значения не
  логируются.
- **Конверт:** ровно `{kind,data}` (`net.rs:133-139`), тест на набор ключей JSON
  (`net_contract.rs:406-418`); `Infinity`/`NaN` → `null` без паники (`net.rs:196-198`).
- **Токен:** `DispatchPayload` не содержит токен; `IdentifyParams`/`ResumeParams`
  редактируют токен в `Debug`; `SupervisorConfig::Debug` печатает `<handler>`.
- **Ошибки канала:** `emit`/spawn/`Disconnected` → счётчики + `lastError`
  (`net.rs:300-359`); битый `d` → `unsupported`, не паника (`net.rs:442-449`).
- **Хук ядра:** `RunOptions.on_dispatch` + ручной `Debug`, flush **до**
  `apply_effects` и **до** распространения ошибки (`ws.rs:648-656`, `701-710`);
  проброс в каждую попытку (`supervisor.rs:299-302`); при `None` поведение прежнее.
- **UI:** подписка после `auth ready`, `stop()` на logout (`app.js:1359-1378`);
  `auth_logout` разрывает и сеть (`main.rs:539-557`); ошибки model/view изолированы и
  не логируют payload (`dispatch-bridge.js:127-153`); снапшот — только счётчики;
  `innerHTML` в `ui/**` нет; пустой READY очищает рейл, self ID отвергает unsafe
  integer (B16/B16b/B17/C15/C15b/C16).
- **Артефакты:** `.gate/*.json` — 0 вхождений имён секретов; логи `waves/RW-40/**` не
  содержат значений; `unsafe`/FFI в проверяемых файлах — 0.
- **Тесты:** локально прогнал `node --test tests/unit/dispatch_ui.test.mjs` → **43/43
  pass, 0 skip, 0 fail**; `npx tsc -p rust/gui-spike/ui/tsconfig.json` → **exit 0**.
  Rust — по логам: `fix-green-net-contract.log` 23/23, `fix-green-core-targeted.log`
  14/14, `fix-green-core.log` 577/577 (сумма по бинарям).

## 6. Что не доказано (и не может быть доказано этим ревью)

1. **Live-доставка** «событие дошло до окна»: окно/сеть запрещены; реальный Tauri
   `emit` не проверялся (только `serde_json::to_value` и wiring-тесты).
2. **SEC-L02-01 и SEC-L02-07 прогоном** не покрыты: вывод по коду и порядку событий
   супервайвера; тест, ловящий P1, отсутствует, а `FakeGateway` маскирует различие.
3. **Redaction «по значению»** не проверяется ни кодом, ни тестами (SEC-L02-06).
4. **Враждебный шлюз/MITM по глубине вложенности:** рекурсия без лимита, переполнение
   стека не тестировалось (TLS снижает риск) — SEC-L02-14.
5. **DoS-поверхность крупных `d`:** 579 КБ × 256 слотов; замеров нет.
6. **Другие каналы утечки** (`ready`, `read_state`, `report_*`, `settings`):
   проверялся только `auth://state` на отсутствие токена; полный аудит — вне диффа.
7. **Полный gate:** `cargo` не запускал (параллельные воркеры/read-only); Rust-цифры
   взяты из `fix-*.log`; gate-артефакта, покрывающего L-02, нет (SEC-L02-08).

## 7. Приоритетный список для Team Lead

1. **P1 SEC-L02-01** — сброс дедупа на новом `connect` + тест на драйвере без `Ready`;
   очистка dispatch-store на logout.
2. **P2 SEC-L02-02/03** — тип-барьер на `Debug` для `DispatchEvent`; минимизация
   READY `d` (PII, 579 КБ) с решением в §5 контракта.
3. **P2 SEC-L02-04/07** — приоритет/`overflowByKind`+resync; `connect` после Stop
   не должен быть no-op; UI должен видеть `net_dispatch_stats`.
4. **P2 SEC-L02-05/06** — синтетический `SESSION_ID`, sentinel-тест redaction, e2e
   payload, согласование регистра ключей.
5. **P2 SEC-L02-08** — переснять md5, обновить `REPORT.md`/§9, приложить gate-артефакт,
   покрывающий L-02.
6. **P3 SEC-L02-09…18** — политика `Debug`, `static_client_session_id`, булевы атомы,
   диагностика на границе UI, лимит глубины, dev-байпас/CSP, logout-store,
   `asSnowflake` для остальных ID.

## Приложение. Чем проверялось

- Чтение кода/тестов/логов/отчётов; `md5sum` (таблица §1); `grep` по
  `unsafe`/FFI, `{:?}`/`println!`/`log::`, `innerHTML`, `net_dispatch_stats`,
  `session_id`/`token`/`secret-token` в `.gate` и `waves/RW-40`; `git ls-files`.
- `jq` по `fixtures/gateway/by-type/002-READY.json` (ключи/значения, PII, размер `d` =
  579091), `005-GUILD_MEMBER_LIST_UPDATE.json` (34 вложенных `session_id`).
- `node --test tests/unit/dispatch_ui.test.mjs` → 43/43; `npx tsc -p
  rust/gui-spike/ui/tsconfig.json` → exit 0.
- Логи, на которые опираюсь: `waves/RW-40/PM/l02-core/{fix-red-core-flush,
  fix-red-oracle-token-mutant,fix-green-core-targeted,fix-green-net-contract,
  fix-green-core}.log`, `waves/RW-40/PM/l02-ui/{red,green,fix-red,fix-red2,
  fix-green,targeted-ui,fix-targeted-ui}.log`, `.gate/worker-2026-10-24T10-22-26-400Z.json`.

Правок в код/тесты/доки/golden не вносил; единственный записанный файл — этот отчёт.
