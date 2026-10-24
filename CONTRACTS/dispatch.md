# CONTRACTS/dispatch.md — gateway dispatch → UI (L-02, первый backend-срез)

Дата: 2026-10-24. Владельцы: G1 (backend: `net.rs`), G2 (UI — следующий срез).
Источники: `waves/RW-40/PARITY/G6-BATCH-MERGE-01.md` §2.4 (L-02), `PARITY-MATRIX.md`
строки L-01/L-02, `rust/crates/discord-core/src/{dispatch.rs,ws.rs,gateway/supervisor.rs}`,
кадры `fixtures/gateway/by-type/*.json`, `fixtures/gw-proto/msgs-run2-95974_13072.jsonl`.

## 1. Статус и граница

Сделано (backend, включая хук ядра L-02-CORE-FINAL):

- конверт события `{kind, data}` и Tauri-событие `gateway://dispatch`;
- фильтр по имени провода, вырезание секретов READY, конвертация ETF `d` → JSON;
- отдельный поток доставки (`DispatchChannel`): `try_send`, backpressure-счётчик,
  порядок сохраняется; ошибка эмита не глотается (счётчик + `lastError`);
- дедуп по `s` в пределах соединения; сброс на `Ready`/`Reconnecting`;
- `net_dispatch_stats` — наблюдаемость для UI/QA без payload и секретов.

Не сделано и не выдаётся за сделанное:

- UI-подписка на `gateway://dispatch` и проекции по `kind` (G2; см. §10);
- типизированные проекции (guilds/channels/members/reactions) не строятся: UI
  получает `d` как есть, иначе пришлось бы выдумывать поля;
- live/окно/сеть не использовались.

## 2. Что реально доступно в ядре (проверено по коду)

| Слой | Что отдаёт | Диспатчи? |
|---|---|---|
| `GatewayPipe` (`ws.rs`) | `events()`, `take_events()`, `unknown_dispatches()`, `last_ready()` | **да**, но пайп живёт внутри одной сессии |
| `connect_and_run_resume` (`ws.rs`) | `RunOutcome`; `RunOptions { identify, on_dispatch }` | **да**: `on_dispatch` дренирует `take_events()` на каждом кадре |
| `Supervisor` (`gateway/supervisor.rs`) | `SupervisorEvent::{Ready, Reconnecting, Stopped}` + `SupervisorConfig::on_dispatch` | **да**: хук пробрасывается в `RunOptions` каждой попытки |
| `NetController`/`CoreGatewayDriver` (`gui-spike/src/net.rs`) | `GatewayStatus` (L-01); `GatewayEvent::Dispatch` | **да**: `core_supervisor_config` подключает оба callback |

Конкретно: `Supervisor::run_until` создаёт `let mut pipe = GatewayPipe::new(...)` на
каждую попытку соединения; `connect_and_run_resume` после `push_binary` вызывает
`flush_dispatches` — при `Some(on_dispatch)` события (`take_events()`, включая raw)
уходят обработчику сразу, при `None` копятся как раньше. Хук `with_event_handler`
по-прежнему относится только к жизненному циклу.

Побочное наблюдение (P2) закрыто тем же хуком: при подключённом обработчике буфер
`GatewayPipe::events` дренируется на каждом кадре, а не растёт до конца сессии.

## 3. Конверт события

Tauri-событие: **`gateway://dispatch`** (`net::DISPATCH_EVENT`), payload —
ровно два поля:

```json
{ "kind": "PRESENCE_UPDATE", "data": { "...": "..." } }
```

- `kind` — имя типа в проводе (`t`), без переименований и без выдуманных вариантов;
- `data` — `d` целиком; поля не добавляются, не переименовываются, не приводятся
  к «удобному» виду. Атомы ETF остаются строками (как в дампе `by-type`),
  `Nil` → `null`, `Tuple`/`List` → массив, `Map` → объект (ключ-целое → строка);
- числа `Infinity`/`NaN` (в наблюдаемых кадрах отсутствуют) → `null`: JSON их не
  представляет, паники нет;
- `kind`-фильтр по имени, а не по `DispatchKind`: `MESSAGE_REACTION_REMOVE` в ядре
  неизвестен (raw), но форвардится по имени провода (§5).

## 4. Какие события форвардятся

`net::FORWARDED_DISPATCHES` (6 имён):

| `kind` | Evidence | Кадры/числа |
|---|---|---|
| `READY` | `[L]` | `fixtures/gateway/by-type/002-READY.json`; 4 в union 7 снимков |
| `GUILD_MEMBERS_CHUNK` | `[L]` | `009-GUILD_MEMBERS_CHUNK.json`; 11 в union |
| `GUILD_MEMBER_LIST_UPDATE` | `[L]` | `005-…json`; 70 кадров `by-type`, 1048 в union — реальный поток участников |
| `PRESENCE_UPDATE` | `[L]` | `010-…json`; 29 кадров `by-type`, 107 в union |
| `MESSAGE_REACTION_ADD` | `[L]` (счётчик), форма `[A]` | 1 в union; кадра `by-type` нет — payload не парсится |
| `MESSAGE_REACTION_REMOVE` | `[A]` | в захватах **0**; в `DispatchKind::ALL` отсутствует → raw |

`MESSAGE_CREATE`/`MESSAGE_UPDATE`/`TYPING_START` и остальные 15 типов **не**
форвардятся: это отдельные карточки (M-01/M-06/P-03), срез L-02 их не расширяет.
Неизвестный/непопадающий тип не паникует: `filtered += 1`.

`GUILD_MEMBER_LIST_UPDATE` добавлен к трём именам из ТЗ осознанно: в живом потоке
участники приходят им (1048 против 11 chunk), без него список участников не
обновится. Остальные 20 имён ядра не включены, чтобы не превращать канал в
трансляцию всего потока.

## 5. Секреты

READY несёт `session_id`, `resume_gateway_url`, `auth_session_id_hash`,
`qos_token`, `analytics_token` — этого достаточно для перехвата сессии, поэтому
`map_dispatch` рекурсивно вырезает ключи:

```
session_id, resume_gateway_url, auth_session_id_hash, client_secret,
password, token, access_token, refresh_token, *…_token
```

Правила:

- вырезание на любой глубине (в `GUILD_MEMBER_LIST_UPDATE` `session_id` лежит в
  `member.presence.activities[]`; он тоже вырезается);
- значения не заменяются заглушками и не логируются — ключа просто нет;
- число вырезанных ключей — `redacted` в статистике, payload — нет;
- токен драйвера (`TokenStore`) в `DispatchPayload` не попадает никогда: источник
  payload — только `d`; ошибки эмита содержат имя события, не данные.

Проверка — тест `ready_payload_drops_session_secrets_and_keeps_data`:
рекурсивный обход оракула находит секреты до конвертации и требует ноль после,
при этом `guilds`/`user.id` сохраняются.

## 6. Хук ядра (реализовано в L-02-CORE-FINAL)

Минимальный аддитивный хук (поведение при `None` не меняется):

```rust
// ws.rs
pub struct RunOptions {
    pub identify: Option<IdentifyParams>,
    /// Новый: вызывается на каждый op0 сразу после разбора кадра.
    pub on_dispatch: Option<Arc<dyn Fn(&DispatchEvent) + Send + Sync>>,
}

// в connect_and_run_resume после pipe.push_binary(...):
if let Some(handler) = opts.on_dispatch.as_ref() {
    for event in pipe.take_events() {
        handler(&event); // контракт: не блокировать (try_send/канал)
    }
}

// gateway/supervisor.rs: SupervisorConfig { on_dispatch: Option<…> }
// и передача его в RunOptions при каждой попытке соединения.
```

Почему так:

- `take_events()` уже публичен и возвращает ровно те op0, что видит роутер;
- хук в `RunOptions` не меняет сигнатуры `Supervisor::run_until` и не требует
  хранить пайп снаружи;
- гуи-сторона уже готова: `CoreGatewayDriver` передаёт `DispatchEvent` в тот же
  callback, что и жизненный цикл, `DispatchChannel` делает `try_send`.

Альтернативы и почему отклонены:

1. Запускать `connect_and_run_resume` из gui-spike самостоятельно и дренировать
   пайп после соединения — нет live-доставки (события только после close) и
   дублируется политика reconnect/resume супервайзера.
2. Добавить `GatewayPipe::with_dispatch_handler` — тот же объём правок в
   `supervisor.rs` (пайп создаётся внутри), но выше риск задеть инварианты
   декодера.
3. Эмитить диспатчи из `state::apply_dispatch` — типизированные проекции не
   покрывают все поля и добавляют второй источник истины.

Реализация: `ws::DispatchHandler` + `RunOptions.on_dispatch` (при `Some` пайп
дренируется на каждом кадре, при `None` поведение прежнее), поле
`SupervisorConfig.on_dispatch` и `with_dispatch_handler`, в `run_until` — проброс
в `RunOptions` каждой попытки. В gui-spike: `core_supervisor_config` (жизненный
цикл + диспатчи в один callback), `CORE_DISPATCH_AVAILABLE = true`,
`sourceAvailable: true`.

Тесты: `supervisor_forwards_real_op0_frames_to_dispatch_handler` —
локальный WS-стаб отдаёт READY/PRESENCE_UPDATE/raw, хук получает ровно эти
кадры (`name`/`s`/`d` не переписаны); `core_supervisor_config_wires_lifecycle_and_dispatch`
— проводка без сети.

## 7. Доставка и порядок

- `GatewayEvent::Dispatch(DispatchEvent)` — сырое событие от драйвера; контроллер
  делает фильтр → конверт → вырезание → `DispatchChannel::deliver`.
- `DispatchChannel` — отдельный поток `gateway-dispatch`, очередь
  `DISPATCH_QUEUE_CAPACITY = 256`, `try_send`: поток супервайзера/рантайм не ждёт
  UI. Очередь полна → `overflow += 1`, событие отбрасывается, причина в
  `lastError`. Порядок — порядок потока (один потребитель).
- Ошибка эмита (Tauri `emit` вернул `Err`) → `emitFailed += 1` + `lastError`;
  текст без payload и секретов; в лог не пишется.
- Дедуп: в пределах соединения событие с `s <= lastSeq` отбрасывается
  (`duplicates += 1`); `Ready`/`Reconnecting` сбрасывают `lastSeq`, потому что на
  новом соединении `s` начинается заново. Своей переигровки у нас нет: буфера
  доставленных событий нет, после reconnect старое не эмитится повторно. Реплей
  пропущенного — ответственность RESUME в ядре (resume идёт от последнего `s`).

## 8. Наблюдаемость: `net_dispatch_stats`

```
{ sourceAvailable, sourceDetail,
  seen, forwarded, filtered, duplicates, unsupported, redacted,
  queued, delivered, emitFailed, overflow, lastError }
```

- `sourceAvailable=true`, `sourceDetail` называет механизм (`RunOptions.on_dispatch`) —
  UI не должен молча ждать пустой канал, причина всегда видна;
- `unsupported` — `d` не сконвертировался (нестроковый ключ map): событие
  отброшено, паники нет;
- команда read-only, без аргументов и без чтения токена.

## 9. Тесты

`rust/gui-spike/tests/net_contract.rs` (аддитивно, 14 тестов L-02): конверт и
`d` поле-в-поле для PRESENCE/CHUNK; counts+ops и вложенные `session_id` для
LIST_UPDATE; READY-секреты (рекурсивный оракул); фильтр/raw/битый `d` без паники;
порядок и статус; ошибка эмита; блокирующий UI не блокирует источник
(backpressure); дедуп и reconnect без переигровки; `sourceAvailable=true` и
проводка `core_supervisor_config` (без сети).

`rust/gui-spike/src/main.rs` (`#[cfg(test)] wiring_tests`): эмиттер целится в
`DISPATCH_EVENT`, ошибка возвращается, payload не логируется; `NetService`
создаётся с эмиттером; `net_dispatch_stats` зарегистрирован и не читает токен.

## 10. Остаток

1. ~~Ядро: хук §6~~ — сделано (L-02-CORE-FINAL).
2. UI (G2): подписка `listen("gateway://dispatch")`, диспетчеризация по `kind`,
   проекции guilds/channels/members/presence/reactions без reload; сейчас
   `ui/**` не менялся.
3. `MESSAGE_REACTION_REMOVE`: после первого живого захвата — либо добавить в 21
   тип ядра, либо оставить raw; до этого форма события `[A]` и не парсится.
4. Реакции в реальном канале (M-06) и typing (P-03) — отдельные карточки; канал
   L-02 их не транслирует.
5. Live-проверка «событие дошло до окна» — окно PM, здесь запрещено.
