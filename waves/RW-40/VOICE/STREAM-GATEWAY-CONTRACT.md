# STREAM-GATEWAY-CONTRACT — Go Live / Stream (STUB-27): замороженный контракт шлюза и редьюсера

Дата: 2026-10-30. Owner контракта: **G5 Platform & Voice/Video**; реализация: **G1 Core/Network**;
приёмка: Team Lead (+ G4 по evidence). HEAD: `4e24048`.

Режим: **read-only**. Не запускались `cargo`, сборка, сеть, live-сессия, Discord, установки.
Выполнялись только чтение файлов и read-only команды (`grep -b`, `dd`, `python3` по байтам
артефакта, `git status`). Продуктовый код, тесты, `Cargo.toml`, `fixtures/MANIFEST.json` и гейт
этим документом **не менялись**; созданы только два файла карточки:
`waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` и `waves/RW-40/VOICE/stream-opcodes-fixtures.json`.

Маркировка утверждений: **[L]** — наблюдено живьём/в байтах; **[S]** — якорь в коде эталонного
клиента (байтовое смещение в `4d2a64cb5df5fc4e_0`); **[A]** — наше решение/дефолт, паритет не
заявлен; **[P]** — правит Team Lead.

Входные документы: `waves/RW-40/RE-DEEP-REA.md` §6 (оракул опкодов и полей),
`waves/RW-40/DESIGN/STREAM.md` (дизайн, §1 разделение слоёв, §2.1 состояния, §2.6 start/stop,
§2.10 ошибки, §3.3–3.4 ключ и шлюз, §8.3 G5), `waves/RW-40/DESIGN/STREAM-REVIEW.md` (блокеры
RE-3, PL-07, B-08), `waves/RW-40/NETWORK-CONTRACTS.md` §3.3.

---

## 0. Итог

STUB-27 на уровне шлюза — это **пять исходящих опкодов (18…22)**, **четыре входящих op0-диспатча**
(`STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, `STREAM_DELETE`), **одна грамматика
ключа** (`guild:<guild_id>:<channel_id>:<user_id>` и `call:<channel_id>:<user_id>`) и **чистый
редьюсер фаз** на шесть значений. Ни строки этого в дереве пока нет: `grep` по `STREAM_*`,
`stream_key`, `StreamKey` в `rust/` даёт 0 совпадений — всё ниже создаётся с нуля, поэтому
контракт фиксирует имена, поля, ошибки и переходы **до** кода.

Три уточнения постановки, без которых контракт был бы неверным:

1. **Опкоды 18…22 — только «клиент → сервер».** Серверные события приходят **не** опкодом 19 и
   не опкодом 18, а обычными op0-диспатчами с `t`. Доказательство: подписки клиента
   `eV(["STREAM_CREATE"], …)` / `eV(["STREAM_SERVER_UPDATE"], …)` / `eV(["STREAM_UPDATE"], …)` /
   `eV(["STREAM_DELETE"], …)` — это диспетчер по имени события, а не обработчик опкода **[S]**
   `@4937830…4938406`. Следствие: в фикстурах inbound-вектор обязан быть `{"op":0,"t":"STREAM_*"}`,
   а не `{"op":18}`.
2. **18…22 — пространство `GatewayOp`, а не RTC `VoiceOp`.** У голосового крейта своя нумерация
   (`op21` = DAVE и т. п.); смешивать нельзя. В `discord-core/src/gateway.rs` `Op::from_code`
   **не расширяется**: `Op::from_code(18) == Unknown(18)` остаётся частью принятого RW-03
   (счётчик `ignored_values`), а команды стрима кодирует отдельный тип `stream::StreamOp`.
3. **Live-паритета нет.** `STREAM_*` живьём не приходили ни разу (0 событий в 1340 op0 **[L]**,
   `DESIGN/STREAM.md` §0.2 п. 7). Всё, что ниже, — контракт **по коду**, значения в фикстурах
   синтетические. Живой JSON с сервера в документе не выдумывается.

Решения, замораживаемые этим документом (подробности по месту):

| ID | Решение | Метка |
|---|---|---|
| **D-27-1** | `stream_key` в состоянии хранится структурой `StreamKey`, а не строкой; на провод и в REST уходит каноническая строка | **[S]** |
| **D-27-2** | `STREAM_CREATE` для `call` кодирует `guild_id: null` (явный Nil), не пропуском поля; парсер принимает и отсутствие | **[A]** |
| **D-27-3** | Команды 18…22 отправляются только из `Established`; вне сессии — локальный отказ без очереди и без ретрая | **[S]** |
| **D-27-4** | `token` живёт только в эффекте `OpenStreamRtc` внутри `StreamSecret` с редактированным `Debug`; в состоянии его нет | **[A]** |
| **D-27-5** | `viewer_ids` в событиях — `Option<Vec<u64>>`: `None` (поля нет) ≠ `Some(vec![])` (зрителей нет) | **[A]**/**[S]** |
| **D-27-6** | `Debug` у `StreamKey` не печатает гильдию и не печатает собранную строку ключа | **[A]** |
| **D-27-7** | Неизвестный `STREAM_DELETE.reason` — не ошибка: `Unknown(String)`, фаза `Ended`, счётчик в диагностике | **[S]** |

---

## 0.1 Источник истины и якоря

Артефакт: `4d2a64cb5df5fc4e_0` (декодированный бандл эталонного клиента), лаборатория
`/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`, 12 410 287 Б,
`md5 934b1289b7ebf17bc0fd09569b23b438`. Смещения — **байтовые**, проверены чтением по смещению;
`len` — длина процитированного фрагмента в байтах UTF-8.

| Якорь | Смещение | len | Что доказывает | Цитата (начало) |
|---|---:|---:|---|---|
| `A-OPCODES` | 4 996 751 | 188 | значения 18…22 и их имена | `STREAM_CREATE=18]="STREAM_CREATE",s[s.STREAM_DELETE=19]="STREAM_DELETE"…` |
| `A-SENDERS` | 4 998 992 | 441 | точные payload всех пяти команд | `streamCreate(e,t,n){…}` |
| `A-ROUTER` | 4 937 840 | 576 | четыре серверных события и их поля | `eV(["STREAM_CREATE"],e=>{eW({type:"STREAM_CREATE",streamKey:e.stream_key…` |
| `A-KEY-MODULE` | 5 089 580 | 580 | guard/parse/build ключа, экспорт `_z` | `_z:()=>s,wL:()=>r});…` |
| `A-KEY-PARSE` | 5 089 698 | 256 | разбор ключа, `throw` на неизвестном типе | `function a(e){let t=e.split(":")…` |
| `A-KEY-BUILD` | 5 089 954 | 206 | сборка ключа через `join(":")` | `function s(e){let{streamType:t,guildId:n,…` |
| `A-STREAMTYPE` | 5 084 903 | 45 | литералы типа: `guild` / `call` | `var p=((a={}).GUILD="guild",a.CALL="call",a);` |
| `A-REASONS` | 528 586 | 165 | четыре кода отказа `STREAM_DELETE` | `aO=((th={}).USER_REQUESTED="user_requested"…` |
| `A-PHASES` | 528 441 | 145 | шесть фаз клиента | `aC=((tA={}).CONNECTING="CONNECTING"…` |
| `A-REDUCER-START` | 4 954 827 | 222 | `STREAM_START` под `isSessionEstablished`, выбор региона | `STREAM_START:function(e){…B.streamCreate(t,n,i,e??T.A.getPreferredRegion())}` |
| `A-REDUCER-WATCH` | 4 955 049 | 277 | `STREAM_WATCH` под guard'ом сессии | `STREAM_WATCH:function(e){…B.streamWatch(t)}…` |
| `A-STOP-HELPER` | 4 951 180 | 59 | стоп = `streamDelete` под guard'ом сессии | `function e9(e){B.isSessionEstablished()&&B.streamDelete(e)}` |
| `A-PING-CALLSITE` | 4 952 506 | 155 | когда шлётся op21 | `RTC_CONNECTION_STATE:function(e){…B.streamPing(e.streamKey)…` |
| `A-REDUCER-CREATEUP` | 8 966 402 | 214 | `STREAM_CREATE`/`UPDATE` → `ACTIVE`/`PAUSED`, region/viewer_ids | `function x(e){…state:o?y.XYD.PAUSED:y.XYD.ACTIVE…}` |
| `A-REDUCER-DELETE` | 8 972 115 | 560 | приоритет правил `STREAM_DELETE` | `STREAM_DELETE:function(e){…if(s)u=y.XYD.RECONNECTING;…` |
| `A-PAUSE-UI` | 710 516 | 96 | пауза идёт через `_z` (ключ строится из `{streamType,guildId,channelId,ownerId}`) | `function y(e,t){let n=(0,d._z)(e);…type:"STREAM_SET_PAUSED"…}` |
| `A-REST` | 292 006 | 115 | REST-ветка `/streams/{key}/{preview,notify,stream}` | `STREAM_PREVIEW:e=>\`/streams/${e}/preview\`…` |

Якоря построены на тех же call-site'ах, что §6 `RE-DEEP-REA.md` (`@4996751`, `@4998992`,
`@4937840`, `@4954827`, `@710570`, `@292006`); окна расширены до границ функции/модуля, поэтому
смещение иногда начинается раньше указанного в §6. `A-KEY-*`, `A-STREAMTYPE`, `A-REASONS`,
`A-PHASES`, `A-REDUCER-*`, `A-STOP-HELPER`, `A-PING-CALLSITE` добавлены этой карточкой (тот же
артефакт, тот же способ).

---

## 1. Область и границы

### 1.1 Входит

- исходящие кадры op18…22: точные поля, типы, null-политика, порядок проверок;
- входящие op0-диспатчи `STREAM_*`: поля, обязательность, `Option`-ность, диагностика;
- грамматика и каноническая форма `stream_key`;
- Rust-типы `discord_core::state::stream` (ключ, команда, событие, ошибка, фаза, состояние, эффект);
- переходы редьюсера фаз и правила приоритета для `STREAM_DELETE`;
- коды отказа (протокольные и локальные) и их проекция в `data-reason` UI;
- тест-план G1 на векторных фикстурах и список мутаций для приёмки.

### 1.2 Не входит

- REST `/streams/{key}/preview|notify|stream` — тела и коды не наблюдались (`DESIGN/STREAM.md`
  §3.3): G1 реализует отдельной карточкой, здесь фиксируется только совпадение ключа;
- второй RTC-транспорт, SRTP/KDF, rid/quality, RTP-видео — V-01/V-03/B-08 (`G5-PREREQ` C-05);
- захват экрана/камеры, Portal/PipeWire/v4l2, энкод — G5 (`DESIGN/STREAM.md` §5, §8.3);
- UI, тексты, `data-stream`, тайлы, fullscreen — G2 (`DESIGN/STREAM.md` §2, §6, §8.1);
- consent-flow — PL-07, до реализации не закрыт;
- права канала `canStream`/`canVideo` — SG-12, источник права не снят;
- DAVE/MLS поверх второго RTC — вне STUB-27.

### 1.3 Кто что делает

| Слой | Обязан | Не делает |
|---|---|---|
| **G1** | собирать/разбирать op18…22 и `STREAM_*`, держать `StreamKey`, редьюсер фаз, эффекты транспорта, отдавать коды отказа в IPC | не строит UI-состояния, не решает «спросить ли пользователя», не хардкодит качество |
| **G5** | отдать кадры и метаданные источника; **не** строить ключ, не открывать WS, не слать `STREAM_*` | не пишет `discord-core` |
| **G2** | рисовать по фазе из IPC, `data-reason` из кода отказа | не строит `stream_key`/`token`/`endpoint` (`DESIGN/STREAM.md` §1) |
| **G4** | тесты «нет ключа в логах», «нет optimistic stop», мутации §7.2 | не ослабляет политику ради зелёного |

Правило слоёв (повторяет `DESIGN/STREAM.md` §1): UI вызывает сервис, сервис зовёт G1, G1
формирует ключ и кадры. Строка ключа не появляется ни в DOM, ни в `aria-label`, ни в логах.

---

## 2. Провод

### 2.1 Опкоды

| Код | Имя | Направление | Payload (поля; `null` — явный Nil) | Метка |
|---:|---|---|---|---|
| 18 | `STREAM_CREATE` | клиент → сервер | `type`, `guild_id`, `channel_id`, `preferred_region` | **[S]** `A-OPCODES`, `A-SENDERS` |
| 19 | `STREAM_DELETE` | клиент → сервер | `stream_key` | **[S]** |
| 20 | `STREAM_WATCH` | клиент → сервер | `stream_key` | **[S]** |
| 21 | `STREAM_PING` | клиент → сервер | `stream_key` | **[S]** |
| 22 | `STREAM_SET_PAUSED` | клиент → сервер | `stream_key`, `paused` | **[S]** |
| 0 | `STREAM_CREATE` | сервер → клиент | `t:"STREAM_CREATE"`, `d{stream_key, region, viewer_ids, rtc_server_id, rtc_channel_id, paused}` | **[S]** `A-ROUTER` |
| 0 | `STREAM_SERVER_UPDATE` | сервер → клиент | `t:"STREAM_SERVER_UPDATE"`, `d{stream_key, endpoint, token}` | **[S]** |
| 0 | `STREAM_UPDATE` | сервер → клиент | `t:"STREAM_UPDATE"`, `d{stream_key, region, viewer_ids, paused}` | **[S]** |
| 0 | `STREAM_DELETE` | сервер → клиент | `t:"STREAM_DELETE"`, `d{stream_key, unavailable, reason}` | **[S]** |

Значения кодов заморожены (`A-OPCODES`): 18, 19, 20, 21, 22. Других `STREAM_*` опкодов в
диапазоне нет; 23, 24 и далее — чужие (`REQUEST_GUILD_APPLICATION_COMMANDS=24` и т. д.).

### 2.2 Исходящие кадры

`A-SENDERS` дословно:

```js
streamCreate(e,t,n){let i=arguments.length>3&&void 0!==arguments[3]?arguments[3]:null;
  this.send(e2.STREAM_CREATE,{type:e,guild_id:t,channel_id:n,preferred_region:i})}
streamWatch(e){this.send(e2.STREAM_WATCH,{stream_key:e})}
streamPing(e){this.send(e2.STREAM_PING,{stream_key:e})}
streamDelete(e){this.send(e2.STREAM_DELETE,{stream_key:e})}
streamSetPaused(e,t){this.send(e2.STREAM_SET_PAUSED,{stream_key:e,paused:t})}
```

Правила кодирования (обязательны для G1):

1. Кадр — ровно `{"op":<18…22>,"d":{…}}`; других корневых полей нет (`s`/`t` — только у op0).
2. Набор полей `d` — **ровно** из таблицы §2.1; лишних полей не добавляем.
3. `type` — строка `"guild"` или `"call"` (нижний регистр, литералы `A-STREAMTYPE`). `"GUILD"`,
   `""`, не-строка — локальный отказ `stream.command.kind`, кадр не уходит.
4. `guild_id`: для `guild` — snowflake строкой; для `call` — **явный `null`** (D-27-2). Это наше
   решение: исходящий `STREAM_CREATE` живьём не наблюдался, а явный `null` делает кадр
   детерминированным — отсутствие поля неотличимо от «забыли заполнить».
5. `channel_id` — snowflake строкой (`null` недопустим).
6. `preferred_region` — строка либо `null`; `null` — нормальный случай (`arguments[3]` по
   умолчанию `null`, `A-SENDERS`). Непустая строка обязана быть ASCII, ≤ 64 Б, без пробелов и
   управляющих символов; иначе `stream.field.region`. Значение выбирает вызывающий по правилу
   `channel.rtcRegion ?? call.region ?? preferredRegion` (**[S]** `A-REDUCER-START`), кодировщик
   fallback не делает.
7. `stream_key` — только каноническая строка §2.4; собирать её в кодировщике из частей нельзя
   (принимается `StreamKey`, а не `&str`).
8. `paused` — строго boolean; строка `"true"`, `1`, `null` — `stream.field.paused`.
9. Ошибка валидации = кадр не отправлен и не поставлен в очередь; частичной отправки нет.

### 2.3 Входящие диспатчи

`A-ROUTER` дословно (свёрнуто до полей):

```js
eV(["STREAM_CREATE"],e=>{eW({type:"STREAM_CREATE",streamKey:e.stream_key,region:e.region,
  viewerIds:e.viewer_ids,rtcServerId:e.rtc_server_id,rtcChannelId:e.rtc_channel_id,paused:e.paused})}),
eV(["STREAM_SERVER_UPDATE"],e=>{eW({type:"STREAM_SERVER_UPDATE",streamKey:e.stream_key,
  endpoint:e.endpoint,token:e.token})}),
eV(["STREAM_UPDATE"],e=>{eW({type:"STREAM_UPDATE",streamKey:e.stream_key,region:e.region,
  viewerIds:e.viewer_ids,paused:e.paused})}),
eV(["STREAM_DELETE"],e=>{eW({type:"STREAM_DELETE",streamKey:e.stream_key,
  unavailable:e.unavailable,reason:e.reason})}),
```

Обязательность полей (замораживается; «мягкое» — диагностика, состояние не меняется):

| Событие | Обязательные | Мягкие (`Option`) | Диагностика при нарушении |
|---|---|---|---|
| `STREAM_CREATE` | `stream_key` (строка, валидный ключ) | `region`, `viewer_ids`, `rtc_server_id`, `rtc_channel_id`, `paused` | `invalid_dispatches += 1`, событие отброшено (для `stream_key`); иначе `malformed_fields` |
| `STREAM_SERVER_UPDATE` | `stream_key`, `endpoint` (непустая строка), `token` (непустая строка) | — | `invalid_dispatches += 1`, событие отброшено |
| `STREAM_UPDATE` | `stream_key` | `region`, `viewer_ids`, `paused` | как у CREATE |
| `STREAM_DELETE` | `stream_key` | `unavailable`, `reason` | как у CREATE |

Правила разбора:

- `d` не map → `stream.field.type` (диагностика, не паника).
- `stream_key` не строкой или не по грамматике → `stream.key.invalid`, событие отброшено:
  ключ — идентичность записи, «починить» его нельзя.
- `viewer_ids` — массив строк-snowflake. `None` (поля нет или `null`) и `Some(vec![])` — **разные**
  состояния (D-27-5); UI по `None` прячет бейдж, по `Some([])` показывает «0»
  (`DESIGN/STREAM.md` §2.7). Элемент не snowflake → `malformed_fields`, массив сохраняется без
  битых элементов.
- `paused`/`unavailable` не boolean → `malformed_fields`; поле считается отсутствующим.
- `reason` — строка; известные значения §5.1, прочие → `StreamDeleteReason::Unknown(raw)` (D-27-7),
  `reason` не строка → `malformed_fields`, считается отсутствующим.
- Неизвестные дополнительные поля игнорируются и считаются по имени
  (`diagnostics.ignored_fields`), но не ломают событие: forward-compat.
- `t` сравнивается точно и регистрозависимо; `"stream_create"` — не наше событие.
- `seq` (`s`) учитывается политикой §4.5, самим парсером не проверяется.

### 2.4 Stream key

Грамматика (**[S]** `A-KEY-MODULE`, `A-KEY-PARSE`, `A-KEY-BUILD`; формат подтверждён живым
HTTP-cache **[L]**, `DESIGN/STREAM.md` §3.3):

```
guild:<guild_id>:<channel_id>:<user_id>     4 сегмента, разделитель ":"
call:<channel_id>:<user_id>                 3 сегмента
```

- `user_id` — владелец/стример; в клиенте `ownerId`.
- Сегменты-идентификаторы: десятичные цифры ASCII, 1…20 знаков, значение `> 0`, без ведущих нулей
  (иначе `to_wire` не восстановит исходную строку). Диапазон — `u64`.
- Парсер обязан быть строгим: ровно 4 сегмента для `guild`, ровно 3 для `call`, пустых сегментов
  нет, префикс — только `guild`/`call` (иначе `UnknownKind`). Клиент на неизвестном префиксе
  кидает исключение (`A-KEY-PARSE`), мы возвращаем типизированную ошибку — паники нет.
- Инвариант: `StreamKey::parse(s).to_wire() == s` для любой канонической строки. Проверяется
  property-тестом на всех векторах фикстуры.
- `MAX_KEY_BYTES = 128` — технический предел на вход (максимальная легальная строка — 68 Б:
  `guild:` + 3×20 + 2 двоеточия = 68 Б). Строка длиннее — `stream.key.invalid`, без разбора.
- REST-путь использует **percent-encoded** ключ (`/streams/guild%3A<guild>%3A<channel>%3A<user>/…`
  **[L]**), шлюз — сырую строку с двоеточиями. Кодирование для URL — забота REST-слоя G1, не
  `StreamKey`.
- `Debug`/`Display` ключа: гильдия не печатается, собранная строка не печатается (D-27-6).
  Для логов доступны `kind`, `channel_id`, `owner_id`; полная строка — только явным
  `to_wire()` в точке отправки.

### 2.5 Когда отправляется каждая команда

| Команда | Триггер | Guard | Метка |
|---|---|---|---|
| 18 `CREATE` | локальное действие «Начать стрим» после выбора источника; `preferred_region` уже вычислен | `Established` | **[S]** `A-REDUCER-START` |
| 19 `DELETE` | «Прекратить стрим» / уход из канала / локальная очистка | `Established` | **[S]** `A-STOP-HELPER` |
| 20 `WATCH` | локальное действие «Смотреть» по чужому ключу | `Established` | **[S]** `A-REDUCER-WATCH` |
| 21 `PING` | RTC-соединение **этого** стрима ушло в `DISCONNECTED` с `willReconnect` | `Established` | **[S]** `A-PING-CALLSITE` |
| 22 `SET_PAUSED` | локальный тумблер паузы | `Established` | **[S]** `A-REDUCER-WATCH`, `A-PAUSE-UI` |

D-27-3: если сессия не `Established`, клиент **молча роняет** команду (`if (B.isSessionEstablished())`
и `return !1`). Мы не повторяем молчание: возвращаем `StreamError::SessionNotEstablished`
(код `stream.session.not_established`), кадр не отправляется, состояние не меняется, авторетрая
нет. Это единственное расхождение поведения с эталоном и оно осознанное — иначе UI не отличит
«не отправили» от «отправили и ждём».

### 2.6 Транспорт

`STREAM_SERVER_UPDATE{stream_key, endpoint, token}` — единственный источник второго RTC-подключения
(**отдельного** от голосового; `DESIGN/STREAM.md` §1). Правила:

- `endpoint` и `token` не переиспользуют голосовые значения;
- `token` — секрет: в состоянии не хранится, в `Debug`/`Display`/`snapshot`/ошибках не появляется,
  наружу уходит только эффектом `OpenStreamRtc` в `StreamSecret` (D-27-4);
- `endpoint` — непрозрачная строка; в логах допустим только хост, полный URL — нет;
- событие не меняет фазу (см. §4.3): подключение доказывает транспорт, а не «стрим жив».

---

## 3. Rust-типы (`discord_core::state::stream`)

Модуль новый, чистый, без I/O и без `serde` (в крейте `unsafe_code = "forbid"`, разбор — по
`discord_codec::etf::EtfValue`, как в `state/voice/parser.rs`). Часы и транспорт снаружи.

### 3.1 Ключ и тип

```rust
// crates/discord-core/src/state/stream/key.rs
pub enum StreamKind { Guild, Call }

impl StreamKind {
    pub fn as_wire(self) -> &'static str;           // "guild" | "call"
    pub fn from_wire(raw: &str) -> Option<Self>;    // точное совпадение, регистрозависимо
}

pub struct StreamKey {
    pub kind: StreamKind,
    pub guild_id: Option<u64>,  // Some ⇔ Guild
    pub channel_id: u64,
    pub owner_id: u64,
}

pub enum StreamKeyError {
    Empty,
    TooLong { len: usize },
    UnknownKind { kind: String },
    SegmentCount { kind: StreamKind, expected: usize, found: usize },
    EmptySegment { index: usize },
    NotSnowflake { index: usize },   // не цифры, ведущий ноль, 0, > 20 знаков
    Overflow { index: usize },
}
// derive(Debug, Clone, PartialEq, Eq); Copy невозможен из-за UnknownKind{String}.

impl StreamKey {
    pub fn parse(raw: &str) -> Result<Self, StreamKeyError>;
    pub fn to_wire(&self) -> String;                 // каноническая строка
    pub fn guild(guild_id: u64, channel_id: u64, owner_id: u64) -> Self;
    pub fn call(channel_id: u64, owner_id: u64) -> Self;
}
// Debug — редактированный (D-27-6), Ord/PartialOrd — по (kind, guild_id, channel_id, owner_id),
// чтобы ключ годился в BTreeMap.
```

### 3.2 Команда и опкод

```rust
// crates/discord-core/src/state/stream/command.rs
pub enum StreamOp { Create = 18, Delete = 19, Watch = 20, Ping = 21, SetPaused = 22 }

impl StreamOp {
    pub fn code(self) -> i64;
    pub fn from_code(code: i64) -> Option<Self>;
    pub fn name(self) -> &'static str;               // "STREAM_CREATE" … "STREAM_SET_PAUSED"
}

pub enum StreamCommand {
    Create {
        kind: StreamKind,
        guild_id: Option<u64>,       // Some ⇔ Guild, иначе MissingGuildId/UnexpectedGuildId
        channel_id: u64,
        preferred_region: Option<String>,
    },
    Delete { stream_key: StreamKey },
    Watch  { stream_key: StreamKey },
    Ping   { stream_key: StreamKey },
    SetPaused { stream_key: StreamKey, paused: bool },
}

impl StreamCommand {
    pub fn validate(&self) -> Result<(), StreamError>;
    pub fn op(&self) -> StreamOp;
    pub fn to_etf(&self) -> discord_codec::etf::EtfValue;   // d
    pub fn encode(&self) -> Result<discord_codec::etf::EtfValue, StreamError>; // {"op":N,"d":{…}}
}
```

`encode` вызывает `validate` первым: возвращает `Err` вместо кадра, который «почти правильный».

### 3.3 Событие

```rust
// crates/discord-core/src/state/stream/types.rs
// parse_dispatch — рядом, в parser.rs (чистый разбор EtfValue, как в state/voice/parser.rs)
pub struct StreamCreated {
    pub stream_key: StreamKey,
    pub region: Option<String>,
    pub viewer_ids: Option<Vec<u64>>,
    pub rtc_server_id: Option<String>,
    pub rtc_channel_id: Option<u64>,
    pub paused: Option<bool>,
}

pub struct StreamServerUpdate {
    pub stream_key: StreamKey,
    pub endpoint: String,
    pub token: StreamSecret,          // Debug — "<redacted>", Display — "<redacted>"
}

/// Токен второго RTC: секрет по значению, не `String`.
pub struct StreamSecret(String);

impl StreamSecret {
    pub fn new(token: String) -> Self;
    pub fn expose(&self) -> &str;     // единственный способ достать значение
}

pub struct StreamUpdate {
    pub stream_key: StreamKey,
    pub region: Option<String>,
    pub viewer_ids: Option<Vec<u64>>,
    pub paused: Option<bool>,
}

pub struct StreamDeleted {
    pub stream_key: StreamKey,
    pub unavailable: Option<bool>,
    pub reason: Option<StreamDeleteReason>,
}

pub enum StreamDispatch {
    Created(StreamCreated),
    ServerUpdate(StreamServerUpdate),
    Update(StreamUpdate),
    Deleted(StreamDeleted),
}

/// `None` — не наше событие (имя не из STREAM_*); `Some(Err)` — наше, но битое.
pub fn parse_dispatch(
    name: &str,
    data: &discord_codec::etf::EtfValue,
) -> Option<Result<StreamDispatch, StreamError>>;
```

Внешний `None` важен: диспетчер не должен записывать `STREAM_*` в `unknown_events`, а битый
`STREAM_*` — не должен притворяться «просто неизвестным типом».

### 3.4 Фаза, состояние, эффекты

```rust
// crates/discord-core/src/state/stream/types.rs
pub enum StreamPhase { Connecting, Active, Paused, Reconnecting, Ended, Failed }
// wire-строки как в клиенте: "CONNECTING"|"ACTIVE"|"PAUSED"|"RECONNECTING"|"ENDED"|"FAILED" [S] A-PHASES

pub struct Stream {
    pub key: StreamKey,
    pub phase: StreamPhase,
    pub region: Option<String>,
    pub viewer_ids: Option<Vec<u64>>,
    pub rtc_server_id: Option<String>,
    pub rtc_channel_id: Option<u64>,
    pub paused: bool,
    pub delete_reason: Option<StreamDeleteReason>,
    pub transport_ready: bool,        // true после STREAM_SERVER_UPDATE
}

pub struct StreamState {
    streams: std::collections::BTreeMap<StreamKey, Stream>,
    last_seq: Option<u64>,
    pub diagnostics: StreamDiagnostics,
}

pub struct StreamDiagnostics {
    pub stale_sequences: Vec<u64>,
    pub invalid_dispatches: u64,
    pub malformed_fields: Vec<String>,
    pub unknown_streams: u64,
    pub orphan_server_updates: u64,
    pub unknown_reasons: Vec<String>,
    pub ignored_fields: Vec<String>,
}

pub enum StreamEffect {
    OpenStreamRtc { key: StreamKey, endpoint: String, token: StreamSecret },
    CloseStreamRtc { key: StreamKey },
    ShowSafetyRateLimit { guild_id: u64 },
    NotifyViewerFull { key: StreamKey },
}

impl StreamState {
    /// Проекция наружу (IPC/UI): без токена, endpoint и строки ключа.
    pub fn snapshot(&self) -> StreamSnapshot;
}

pub struct StreamSnapshot {
    pub streams: Vec<StreamSummary>,
}

pub struct StreamSummary {
    pub key: StreamKey,          // Debug редактированный (D-27-6)
    pub phase: StreamPhase,
    pub phase_wire: &'static str,
    pub region: Option<String>,
    pub viewer_count: Option<usize>,   // None ⇔ viewer_ids отсутствовал
    pub transport_ready: bool,
}
```

### 3.5 Ошибка и коды

```rust
// crates/discord-core/src/state/stream/error.rs
pub enum StreamError {
    InvalidKey { reason: StreamKeyError },
    MissingField { field: &'static str },
    WrongFieldType { field: &'static str, expected: &'static str },
    InvalidId { field: &'static str },
    InvalidRegion,
    InvalidPaused,
    WrongKind { expected: StreamKind, found: StreamKind },
    MissingGuildId,
    UnexpectedGuildId,
    SessionNotEstablished,
}

impl StreamError {
    pub fn code(&self) -> &'static str;   // стабильный машинный код для IPC/UI
}
```

Требования: `PartialEq`, `Display` на русском, `std::error::Error`, никакой строки с `token`,
`endpoint` или полной строкой ключа. Коды:

| Вариант | `code()` | Когда |
|---|---|---|
| `InvalidKey` | `stream.key.invalid` | ключ не по грамматике §2.4 |
| `MissingField` | `stream.field.missing` | обязательное поле отсутствует |
| `WrongFieldType` | `stream.field.type` | поле не того типа |
| `InvalidId` | `stream.field.id` | id `0`/не snowflake |
| `InvalidRegion` | `stream.field.region` | регион не ASCII/пустой/длиннее 64 Б |
| `InvalidPaused` | `stream.field.paused` | `paused` не boolean |
| `WrongKind` | `stream.command.kind` | `type` не `guild`/`call` |
| `MissingGuildId` | `stream.command.guild_id.missing` | `type:"guild"`, `guild_id` нет |
| `UnexpectedGuildId` | `stream.command.guild_id.unexpected` | `type:"call"`, `guild_id` есть |
| `SessionNotEstablished` | `stream.session.not_established` | команда вне `Established` (D-27-3) |

---

## 4. Редьюсер: переходы

Вход — `StreamAction`, выход — `Vec<StreamEffect>`; состояние меняется только здесь.

```rust
pub enum StreamAction {
    Dispatch { seq: Option<u64>, event: StreamDispatch },
    Local(LocalStreamAction),
    Ready,                                     // новый READY: сброс
}

pub enum LocalStreamAction {
    StartRequested { kind: StreamKind, guild_id: Option<u64>, channel_id: u64 },
    WatchRequested { key: StreamKey },
    StopRequested { key: StreamKey },
    SetPausedRequested { key: StreamKey, paused: bool },
    Close { key: StreamKey },
}

pub fn apply(state: &mut StreamState, action: StreamAction) -> Vec<StreamEffect>;
```

### 4.1 Переходы по событиям

Оракул — `A-REDUCER-CREATEUP` и `A-REDUCER-DELETE`; порядок правил DELETE скопирован дословно.

| Событие | Условие | Фаза после | Поля | Эффекты |
|---|---|---|---|---|
| `STREAM_CREATE` | всегда; если `transport_ready==true`, сначала `CloseStreamRtc` | `paused==Some(true)` → `Paused`, иначе `Active` | запись **заменяется**: `region`, `viewer_ids`, `rtc_server_id`, `rtc_channel_id`, `paused` из события (`None` — очистка); `transport_ready=false`; `delete_reason=None` | `CloseStreamRtc`, только если прежний транспорт был открыт **[A]** |
| `STREAM_UPDATE` | запись есть или нет (нет — создаётся, как в клиенте) | `paused==Some(true)` → `Paused`, иначе `Active` | мягкие поля: `Some` заменяет, `None` сохраняет прежнее | нет |
| `STREAM_SERVER_UPDATE` | запись есть | **не меняется** | `transport_ready=true` | `OpenStreamRtc{key,endpoint,token}` |
| `STREAM_SERVER_UPDATE` | записи нет | — | состояние не создаётся | `OpenStreamRtc` + `orphan_server_updates += 1` |
| `STREAM_DELETE` | `unavailable==Some(true)` | `Reconnecting` | `delete_reason=reason`; запись **остаётся**, `transport_ready=false` | `CloseStreamRtc*` |
| `STREAM_DELETE` | `reason==Unauthorized` | `Failed` | `delete_reason=Unauthorized` | `CloseStreamRtc*` |
| `STREAM_DELETE` | `reason==SafetyGuildRateLimited` | `Ended` | `delete_reason` | `CloseStreamRtc*`, `ShowSafetyRateLimit{guild_id}` |
| `STREAM_DELETE` | `reason==StreamFull` | `Ended` (fall-through как в клиенте) | `delete_reason` | `CloseStreamRtc*`, `NotifyViewerFull{key}` |
| `STREAM_DELETE` | прежняя фаза `Failed` и `reason==UserRequested` | `Failed` | `delete_reason` | `CloseStreamRtc*` |
| `STREAM_DELETE` | иначе (в т. ч. `reason=None`, `Unknown`) | `Ended` | `delete_reason` | `CloseStreamRtc*` |
| `STREAM_DELETE` | записи нет | — | ничего | `NotifyViewerFull` только при `StreamFull`, иначе пусто; `unknown_streams += 1` |

`CloseStreamRtc*` — условный эффект: эмитится, только если `transport_ready == true` либо фаза до
удаления была `Active`/`Paused` (то есть RTC мог быть открыт). Иначе закрывать нечего, а лишний
`Close` в транспорте — шум. Это единственное место, где мы уточняем клиент: он зовёт `w(r)`
безусловно при `ENDED`, у нас эффект условный **[A]**.

Приоритет для DELETE жёсткий: `unavailable` → `Unauthorized` → `SafetyGuildRateLimited` →
(`Failed`+`UserRequested`) → default. Перестановка правил меняет наблюдаемое состояние и ловится
мутацией §7.2.

### 4.2 Локальные интенты

| Действие | Фаза | Что делает | Почему так | Метка |
|---|---|---|---|---|
| `StartRequested` | — | **ничего** не создаёт | optimistic-старт запрещён (`DESIGN/STREAM.md` §2.6), запись появится только по `STREAM_CREATE` | **[S]**/**[A]** |
| `WatchRequested` | `Connecting` | вставляет запись **и перезаписывает фазу**, даже если запись уже активна (повторный watch = новое подключение) | клиент делает `i.set(key,{…state:CONNECTING})` на `STREAM_WATCH` | **[S]** `A-REDUCER-WATCH` |
| `StopRequested` | **не меняется** | ничего | «офф» только по `STREAM_DELETE`; кнопка не переключается заранее | **[S]**/**[A]** |
| `SetPausedRequested` | **не меняется** | ничего | фазу меняет только `STREAM_UPDATE{paused}` | **[S]** `A-PAUSE-UI` |
| `Close` | запись удаляется | удаляет | `STREAM_CLOSE` в клиенте чистит запись | **[S]** |
| `Ready` | карта очищается | сброс `last_seq`, всех фаз | стримы — runtime-only, в READY их нет | **[A]** |

`Close` эмитит `CloseStreamRtc`, если `transport_ready` (транспорт надо закрыть принудительно).

### 4.3 Что **не** меняет фазу

1. `VOICE_STATE_UPDATE.self_stream` / `self_video` — только «кто стримит» для бейджа
   (`DESIGN/STREAM.md` §1: наличие стрима у участника ≠ жизненный цикл). Фаза стрима из этих
   флагов не выводится **никогда** — иначе UI начнёт врать при рассинхроне.
2. `STREAM_SERVER_UPDATE` — транспорт, не жизнь (§2.6).
3. `RTC_CONNECTION_STATE` — вход в диагностику; единственное следствие — право отправить op21
   (`A-PING-CALLSITE`), фазы не касается.
4. Локальные `StopRequested`/`SetPausedRequested` — см. §4.2.

### 4.4 Гипотезы, которые обязаны быть названы в коде

Как `H-SEQ` в V-01: каждая гипотеза — именованный комментарий и одна функция, чтобы live-правка
стоила одну строку.

| ID | Гипотеза | Основание | Что сломается, если неверно |
|---|---|---|---|
| **H-VIEWERS** | `viewer_ids` в `STREAM_CREATE`/`STREAM_UPDATE` — **полный** список, не дельта; `Some` заменяет прежний целиком | `viewer_ids` читается как массив без накопления **[S]**; семантика live не наблюдалась (`DESIGN/STREAM.md` §2.7) | счётчик зрителей разойдётся; правится одна функция merge |
| **H-PAUSE** | `paused` в событиях — состояние **этого** стрима; отсутствие поля не сбрасывает паузу | `paused` только читается и кладётся в состояние **[S]**; смысл не установлен (SG-05) | пауза «залипнет» или сбросится |
| **H-SERVER-UPDATE** | `STREAM_SERVER_UPDATE` не меняет фазу и приходит до первого RTC-кадра | отдельный обработчик в клиенте **[S]**; порядок live неизвестен | UI покажет «live» раньше картинки |
| **H-DELETE-END** | `Ended` не удаляет запись сам; запись живёт до `Close`/нового `CREATE` | клиент оставляет запись и зовёт `w(r)` | «мёртвые» записи в счётчике |

### 4.5 Последовательности, дубли, идемпотентность

- `seq` (`s`) сравнивается с `last_seq` по правилу `state::reducer`: `seq <= last_seq` — событие
  игнорируется, `stale_sequences.push(seq)`. Исключение — `Ready`.
- Повторный `STREAM_CREATE` с тем же ключом и другим `s` — валидная замена (сервер вправе
  переотправить), идемпотентен по наблюдаемому состоянию.
- Повторный `STREAM_CREATE` с тем же `s` — игнорируется (stale).
- `STREAM_DELETE` дважды: первый даёт `Ended`, второй видит `Ended` и даёт `Ended` (правило
  default), эффект `CloseStreamRtc` второй раз не эмитится (`transport_ready=false`).
- `STREAM_SERVER_UPDATE` дважды: два `OpenStreamRtc` — норма (переподключение RTC), транспорт
  обязан быть идемпотентен по ключу; это требование к V-01/RTC-слою, не к редьюсеру.

---

## 5. Коды отказа

Два разных набора: **протокольные** (приходят в `STREAM_DELETE.reason`) и **локальные**
(возвращаем мы, кадр не уходит). Смешивать их в один enum нельзя: у первого источник — сервер,
у второго — наша валидация.

### 5.1 Протокольные `reason`

`A-REASONS` дословно: `USER_REQUESTED="user_requested"`, `STREAM_FULL="stream_full"`,
`UNAUTHORIZED="unauthorized"`, `SAFETY_GUILD_RATE_LIMITED="safety_guild_rate_limited"`.

```rust
pub enum StreamDeleteReason {
    UserRequested,
    StreamFull,
    Unauthorized,
    SafetyGuildRateLimited,
    Unknown(String),        // D-27-7: сохраняется как есть, фаза Ended, счётчик
}
```

| `reason` | Значение по коду | Фаза | UI-следствие | Метка |
|---|---|---|---|---|
| `user_requested` | стрим остановлен по запросу | `Ended` (или `Failed`, если уже был `Failed`) | тихий `off` | **[S]** |
| `stream_full` | у стрима максимум зрителей | `Ended` | `data-reason="viewer-full"`, текст `VVZDBL` | **[S]**/`[D]` |
| `unauthorized` | нет права/токен отвергнут | `Failed` | `data-reason="permission-channel"` **[A]** | **[S]** |
| `safety_guild_rate_limited` | предохранитель гильдии | `Ended` + модалка | отдельное уведомление (клиент грузит чанк `452729`/`116960`) **[S]**; `data-reason` в `DESIGN/STREAM.md` §2.10 отсутствует — предлагаем `safety-rate-limited` **[A]** | **[S]**/**[A]** |
| неизвестный | сервер новее нас | `Ended` | без текста, только диагностика | **[A]** |

`unavailable: true` — не код отказа, а сигнал «стрим недоступен, ждём переподключения»: фаза
`Reconnecting`, запись сохраняется, ошибку пользователю не показываем.

### 5.2 Локальные коды

Таблица §3.5 — полный список. Дополнительно:

| Ситуация | Код | Что делает G1 |
|---|---|---|
| команда до `Established` | `stream.session.not_established` | не отправляет, не ставит в очередь |
| `stream_key` не разобран | `stream.key.invalid` | не отправляет, отдаёт код в IPC |
| `type` не `guild`/`call` | `stream.command.kind` | то же |
| `guild` без `guild_id` / `call` с `guild_id` | `stream.command.guild_id.missing` / `.unexpected` | то же |
| регион пустой/не ASCII/длиннее 64 Б | `stream.field.region` | то же |
| `paused` не boolean (при разборе) | `stream.field.paused` | событие считается битым, поле игнорируется |

### 5.3 Проекция в UI

`data-reason` из `DESIGN/STREAM.md` §2.10 заполняется по коду, а не по тексту:

| Источник | `data-reason` | Текст/ключ |
|---|---|---|
| `reason=="stream_full"` | `viewer-full` | `VVZDBL` |
| `reason=="unauthorized"` | `permission-channel` | `/x4knx` **[A]**-маппинг |
| `reason=="safety_guild_rate_limited"` | `safety-rate-limited` **[A]** | отдельная модалка |
| локальный отказ старта (`stream.*`) | `start-failed` | `rSlOep` |
| отказ захвата/энкодера (G5) | `permission-os`, `portal-unavailable`, `encoder-failed` | §2.10 |
| `unavailable:true` | — (не ошибка) | строка «Переподключение» **[A]** |
| неизвестный `reason` | — | ничего, диагностика |

Права канала (`canStream`/`canVideo`) в этот контракт не входят: их источник не снят (SG-12),
и придумывать `stream.command.denied` до live нельзя.

---

## 6. Фикстуры

Файл: `waves/RW-40/VOICE/stream-opcodes-fixtures.json`, схема `rw40/stream-opcodes-fixtures-v1`.
Он же — вход для тестов G1 и образец для курированной копии в `fixtures/` (копию и строку в
`fixtures/MANIFEST.json` вносит Lead `[P]`: `MANIFEST.json` — общий файл).

Содержимое: `oracle` (артефакт, md5, размер), `anchors` (§0.1 с цитатами и `len`), `opcodes`,
`outbound` (9 векторов), `inbound` (21 вектор), `negative` (27 векторов), `reducer`
(14 сценариев), `notes`.

Инварианты файла:

1. Валидный JSON (`python3 -m json.tool`), UTF-8, без комментариев.
2. Каждый вектор несёт `anchors: ["A-…"]`; у исходящих — ещё `source.offset` (точечное место
   метода-отправителя).
3. Один `provenance` на файл: `method:"code-derived"`, `liveConfirmed:false`, md5 оракула. Ни один
   серверный payload не выдаётся за захват.
4. Все id — синтетические (`100000000000000001`…), все секреты — `"REDACTED"`, endpoint — домен
   `.invalid`. Ничего из реального профиля.
5. `s` у inbound-векторов синтетические (`9001…`), помечены `"sequence": "synthetic"`.
6. Поля payload — ровно по §2.1/§2.3; каждый `negative` несёт `expectError` (§3.5) либо
   `expectDiagnostic` (мягкий отказ: событие разобрано, поле отброшено).
7. Для каждого `outbound` выполняется `StreamCommand::encode(...) == payload` (побайтово после
   канонизации поля-порядка), для каждого `inbound` — `parse_dispatch(t, d) == ожидаемая структура`.

---

## 7. Тест-план G1/G5

Серия `T-STREAM-nn` — новая, не пересекается с `T-V01-*`, `T-VOICE-*`, `T-MEDIA-*`; строки в
`docs/rust-client/TEST-PLAN.md` вносит Lead `[P]`. Файл тестов:
`rust/crates/discord-core/tests/stream_contract.rs`. Стенд — фикстура, никакой сети, никакого
реального времени, никакого `cargo` в этой карточке.

| ID | Ур. | Проверка | Оракул |
|---|---|---|---|
| **T-STREAM-01** | L1 | таблица опкодов: 18…22 ↔ имена, `from_code(23)==None`, `Op::from_code(18)==Unknown(18)` не изменился | `A-OPCODES`, `gateway.rs` |
| **T-STREAM-02** | L1 | `encode(Create{guild})` даёт ровно `{type,guild_id,channel_id,preferred_region}` и `op:18` | `in/out` векторы |
| **T-STREAM-03** | L1 | `Create{call}` даёт `guild_id: null` (D-27-2); `Create{guild}` без `guild_id` → `MissingGuildId` | `out-create-call`, `negative` |
| **T-STREAM-04** | L1 | `Delete/Watch/Ping/SetPaused` кодируются в `{stream_key}` / `{stream_key,paused}`, `paused` строго bool | `A-SENDERS` |
| **T-STREAM-05** | L1 | ключ: round-trip `parse→to_wire` на всех ключах фикстуры; guild=4 сегмента, call=3 | `A-KEY-*` |
| **T-STREAM-06** | L1 | ключ: 13 negative-векторов ключа дают точный `StreamKeyError`, паники нет | `negative` |
| **T-STREAM-07** | L1 | `Debug`/`Display`/`snapshot` не содержат полной строки ключа, `token`, `endpoint` (grep по `format!("{:?}")`) | `DESIGN/STREAM.md` §1, SG-06 |
| **T-STREAM-08** | L1 | разбор всех 21 inbound-вектора: поля, `Option`-ность, `Some([])` ≠ `None` | `inbound`, D-27-5 |
| **T-STREAM-09** | L1 | `unavailable:true` → `Reconnecting`, запись **остаётся**, `CloseStreamRtc` один раз | `A-REDUCER-DELETE` |
| **T-STREAM-10** | L1 | приоритет DELETE: `unauthorized`→`Failed`, `safety_*`→`Ended`+модалка, `stream_full`→`Ended`+`NotifyViewerFull`, `user_requested` после `Failed`→`Failed` | `A-REDUCER-DELETE` |
| **T-STREAM-11** | L1 | неизвестный `reason` → `Unknown(raw)`, `Ended`, `unknown_reasons` +1, паники нет | D-27-7 |
| **T-STREAM-12** | L1 | `CREATE{paused:true}` → `Paused`; `UPDATE{paused:false}` → `Active`; `UPDATE` без `paused` сохраняет паузу (H-PAUSE) | `A-REDUCER-CREATEUP` |
| **T-STREAM-13** | L1 | `SERVER_UPDATE` не меняет фазу, ставит `transport_ready`, эмитит `OpenStreamRtc`; без записи — `orphan_server_updates` | `A-ROUTER` |
| **T-STREAM-14** | L1 | локальные интенты: `StartRequested` не создаёт запись, `StopRequested`/`SetPausedRequested` фазу не меняют, `WatchRequested` даёт `Connecting` | §4.2 |
| **T-STREAM-15** | L1 | `VOICE_STATE_UPDATE.self_stream=false` не меняет фазу стрима (никакого вывода из флага) | §4.3 |
| **T-STREAM-16** | L1 | stale: повтор с `seq <= last_seq` не меняет состояние и пишет `stale_sequences` | §4.5 |
| **T-STREAM-17** | L1 | битый `STREAM_*` (не map, ключ-мусор, `viewer_ids: "x"`) → диагностика, состояние цело, без паники | `negative` |
| **T-STREAM-18** | L1 | неизвестный `t` (`STREAM_FOO`) → `parse_dispatch` = `None`, в `unknown_events` не попадает как STREAM | §3.3 |
| **T-STREAM-19** | L2 | каждый префикс каждого outbound/inbound кадра (ETF-байты) разбирается без паники (расширение `every_prefix_of_frame_is_panic_free`) | образец `tests/signaling.rs:386` |
| **T-STREAM-20** | L2 | `encode` идемпотентен: повторная сборка даёт равный `EtfValue` (порядок полей не влияет на сравнение) | фикстуры |
| **T-STREAM-21** | L2 | полный реплей 14 сценариев `reducer` из фикстуры: фаза и эффекты по шагам | `reducer` |
| **T-STREAM-22** | L3 | G5↔G1 стык: источник (id/кадры) не порождает `stream_key`; ключ строится только из `{kind,guild,channel,owner}` | §1.3, SG-06 |

### 7.1 Что считается оракулом

Фикстура `stream-opcodes-fixtures.json` + байтовые смещения §0.1. Тест **не** имеет права
сверяться с текущей реализацией (например, «`encode` вызвать дважды и сравнить») как с истиной;
истина — поле `payload`/`expect` фикстуры.

### 7.2 Мутации (обязательны на приёмке G4)

| Мутация | Должен краснеть |
|---|---|
| порядок правил DELETE: `unauthorized` раньше `unavailable` | T-STREAM-09, T-STREAM-10 |
| `unavailable:true` удаляет запись | T-STREAM-09 |
| `stream_full` → `Failed` | T-STREAM-10 |
| неизвестный `reason` → ошибка разбора | T-STREAM-11 |
| `paused` отсутствует → сброс паузы | T-STREAM-12 |
| `SERVER_UPDATE` ставит `Active` | T-STREAM-13 |
| `StopRequested` сразу ставит `Ended` | T-STREAM-14 |
| `self_stream` управляет фазой | T-STREAM-15 |
| `viewer_ids:[]` нормализуется в `None` | T-STREAM-08 |
| `parse` пускает ведущий ноль / 5 сегментов | T-STREAM-06 |
| `Debug` печатает `token`/ключ | T-STREAM-07 |
| `encode` не зовёт `validate` | T-STREAM-03 |

### 7.3 Гейт

В batch: `cargo test -p discord-core --test stream_contract` (targeted) + существующие
`state_*`/`gateway_*` тесты как регрессия (правки `dispatch.rs`/`state/types.rs` — через Lead).
Полный `gate:worker` — один раз на batch в integration-лейне. В этой карточке `cargo` **не
запускался** (занят G1).

### 7.4 G5

| ID | Проверка | Оракул | Метка |
|---|---|---|---|
| SP-08 | второй RTC поднимается строго из `STREAM_SERVER_UPDATE{endpoint,token}` | шлюз (live) | `[A]` |
| SP-11 | G5 не формирует `stream_key`/`token`/`endpoint`: grep по `discord-platform` = 0 | код | **[A]** |
| SP-12 | G5 не логирует ключ и токен: grep по `dbus`/`portal`/`pipewire` = 0 | код | **[A]** |

---

## 8. Файловая матрица

| Файл | Owner | Замечание |
|---|---|---|
| `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` | **G5** | этот документ |
| `waves/RW-40/VOICE/stream-opcodes-fixtures.json` | **G5** | векторные фикстуры, схема §6 |
| `rust/crates/discord-core/src/state/stream/{mod,key,command,types,parser,reducer,error}.rs` | **G1** | новый модуль по образцу `state/voice/`; `pub mod stream;` — 1 строка в `state/mod.rs` |
| `rust/crates/discord-core/src/lib.rs` | **G1** | не меняется: модуль регистрируется в `state/mod.rs` |
| `rust/crates/discord-core/tests/stream_contract.rs` | **G1** | `T-STREAM-01…22` |
| `rust/crates/discord-core/src/dispatch.rs` | **Lead `[P]`** | общий файл: 4 варианта `DispatchKind` + `as_str`/`ALL` (21→25), правка doc-комментария |
| `rust/crates/discord-core/tests/dispatch_router.rs` | **Lead `[P]`** | общий тест: `ALL.len()==21` → 25 (строка 267), иначе регрессия чужая |
| `rust/crates/discord-core/src/state/types.rs` | **Lead `[P]`** | общий файл: `EventPayload::{Stream…}`, `Event::from_routed`, поле `stream: StreamState` в `State` |
| `rust/crates/discord-core/src/state/reducer.rs` | **Lead `[P]`** | общий файл: 4 ветки `EventPayload::Stream*` → `state::stream::reducer::apply` |
| `rust/crates/discord-core/src/state/mod.rs` | **Lead `[P]`** | общий файл: `pub mod stream;` + реэкспорт |
| `rust/crates/discord-core/src/gateway.rs` | **не трогать** | `Op` не расширяется (§0 п. 2) |
| `fixtures/MANIFEST.json`, курированная копия фикстуры | **Lead `[P]`** | общий файл |
| `docs/rust-client/TEST-PLAN.md` (`T-STREAM-01…22`) | **Lead `[P]`** | G1/G5 приносят готовые строки |
| `TASKS.md`, `waves/*`, `docs/map.md` | **Lead `[P]`** | — |

Пересечений нет: G5 в карточке не пишет продуктовый код, G1 не трогает `discord-voice` (ветка
V-01) и не правит `gateway.rs`. Правки `dispatch.rs`/`state/types.rs` — только через Lead
(AGENTS.md, «Общие файлы»).

---

## 9. Блокеры live и открытые вопросы

| ID | Вопрос | Почему нельзя закрыть офлайн | Кто решает |
|---|---|---|---|
| **Q-27-1** | Какой REST-вызов порождает `STREAM_CREATE` и что останавливает стрим | 0 вызовов в capture; тела не сняты (RE-3, B-11) | PM/RE |
| **Q-27-2** | `viewer_ids` — полный список или дельта | ни одного события live (SG-05) | live-окно |
| **Q-27-3** | Смысл `paused` (пауза стрима или приостановка зрителя) | не наблюдалось | live-окно |
| **Q-27-4** | Приходит ли `STREAM_SERVER_UPDATE` до или после `STREAM_CREATE` | порядок не наблюдался | live-окно |
| **Q-27-5** | Может ли `STREAM_DELETE.reason` нести значения вне четырёх | код клиента знает только 4 | live-окно |
| **Q-27-6** | `rtc_server_id`/`rtc_channel_id` — формат и смысл | только проброс в клиенте | live-окно |
| **Q-27-7** | `data-reason` для `safety_guild_rate_limited` | в `DESIGN/STREAM.md` §2.10 строки нет | PM/G3 |

Ответы не блокируют офлайн-часть: все семь вопросов лежат в полях `Option`/`Unknown`, кроме
Q-27-7 (это UI-строка, не протокол).

Жёсткие блокеры релиза STUB-27 (вне этого контракта): PL-07 (consent), V-01/V-03 (транспорт и
SRTP KDF, B-08), PipeWire-клиент, права канала (SG-12).

---

## 10. Что этот документ не утверждает

1. **Не утверждает паритет.** `STREAM_*` живьём не приходили; все payload — по коду клиента.
2. **Не описывает REST** `/streams/*`: тела и коды ответов не наблюдались, кроме совпадения ключа.
3. **Не описывает второй RTC** (endpoint/token/SRTP/rid/quality) — только точку передачи
   `endpoint`/`token` эффектом.
4. **Не решает consent** и права канала: `canStream`/`canVideo` — SG-12, PL-07.
5. **Не задаёт UI-геометрию и тексты** — G2 (`DESIGN/STREAM.md` §4, §6).
6. **Не меняет `gateway.rs`**: опкоды 18…22 не входят в `Op` и не влияют на heartbeat/реконнект.
7. **Не вводит новых зависимостей**: разбор — существующим `discord_codec::etf`, как в
   `state/voice/parser.rs`.
8. **Не содержит секретов**: `token` — только в `StreamSecret` эффекта, значения в фикстурах
   `REDACTED`.
