# L-02 — dispatch → UI, первый backend-срез (red-first)

Дата: 2026-10-24. Воркер: **G1 Network/Backend**. PM: DeepSeek.
Статус: **готово к приёмке** (backend-контракт; живой источник в ядре отсутствует —
предложение в §2, фейка нет). Окно/live/сеть/commit не использовались.

## 1. Контракт (источник)

- `waves/RW-40/PARITY/G6-BATCH-MERGE-01.md` §2.4 п.4 (L-02): READY/CHUNK/PRESENCE/
  `MESSAGE_REACTION_*` → UI без reload; UI-маппинг — следующий срез.
- `PARITY-MATRIX.md` L-02: «partial: reducer/core, UI на fixtures; нет привязки
  событий к экранам».
- Ядро: `discord-core/src/dispatch.rs` (21 наблюдённый тип), `ws.rs` (`GatewayPipe`),
  `gateway/supervisor.rs` (жизненный цикл).
- Оракулы payload'ов: `fixtures/gateway/by-type/002-READY.json`,
  `009-GUILD_MEMBERS_CHUNK.json`, `005-GUILD_MEMBER_LIST_UPDATE.json`,
  `010-PRESENCE_UPDATE.json`; счётчики типов — `tests/dispatch_router.rs`,
  `fixtures/gw-proto/facts-frozen.json`.

## 2. Что доступно в ядре (главный вывод)

| Слой | Диспатчи наружу |
|---|---|
| `GatewayPipe::take_events()` | да, но пайп живёт внутри `Supervisor::run_until` |
| `connect_and_run_resume` / `RunOptions { identify }` | нет хука на кадр |
| `SupervisorEvent` | **только** `Ready`/`Reconnecting`/`Stopped` |

`Supervisor::run_until` создаёт `GatewayPipe` на каждую попытку и никто не вызывает
`take_events()`: op0 копятся в `pipe.events` до конца соединения и исчезают.
Следовательно, **transport dispatch не отдаёт** — «прикрутить» Tauri-событие к
живому потоку без правки ядра нельзя. Поэтому:

- реализован полный контракт и канал, но `net::CORE_DISPATCH_AVAILABLE = false`,
  `net_dispatch_stats().sourceAvailable = false` с причиной (`sourceDetail`);
- дизайн-предложение (аддитивный `RunOptions.on_dispatch` + проброс из
  `SupervisorConfig`, ~60–80 строк, поведение при `None` не меняется) —
  `CONTRACTS/dispatch.md` §6. Заодно закрывает рост `pipe.events` на длинной сессии
  (дренажа сейчас нет — P2, вне файлов карточки).
- ничего не сфабриковано: `FakeGateway::dispatch` подаёт **сырой**
  `discord_core::DispatchEvent` тем же путём, что будущий боевой драйвер.

## 3. Сделано

`rust/gui-spike/src/net.rs` (аддитивно, L-02-блок):

- `GatewayEvent::Dispatch(DispatchEvent)` — сырое op0; драйвер не переписывает.
- `DispatchPayload { kind, data }` (serde) — ровно два поля; `kind` — имя провода.
- `FORWARDED_DISPATCHES` (6): READY, GUILD_MEMBERS_CHUNK, GUILD_MEMBER_LIST_UPDATE,
  PRESENCE_UPDATE, MESSAGE_REACTION_ADD, MESSAGE_REACTION_REMOVE. `LIST_UPDATE`
  добавлен к трём именам ТЗ по evidence (70 кадров `by-type`, 1048 в union против
  11 chunk) — иначе список участников не обновляется.
- `etf_to_json`: атом → строка (как в дампе `by-type`), `Nil` → `null`,
  Tuple/List → массив, Map → объект (int-ключ → строка), NaN/Inf → `null`,
  нестроковый ключ → `DispatchError::UnsupportedKey` (не паника).
- `map_dispatch`: фильтр по имени → JSON → **рекурсивное** вырезание секретов
  (`session_id`, `resume_gateway_url`, `auth_session_id_hash`, `client_secret`,
  `password`, `token`, `access_token`, `refresh_token`, `*_token`). READY реально
  несёт session/resume/qos — в UI они не уходят; счётчик `redacted`.
- `DispatchEmitter` + `DispatchChannel`: отдельный поток `gateway-dispatch`,
  очередь 256, `try_send` (источник не ждёт UI), порядок одного потребителя,
  `queued/delivered/emitFailed/overflow/lastError`; ошибка эмита и переполнение
  не глотаются и не логируются.
- `NetController::with_emitter` (старый `new` сохранён), дедуп `s <= lastSeq` в
  соединении, сброс на `Ready`/`Reconnecting`; `NetController::dispatch_stats`.
- `NetService::with_emitter`, `dispatch_stats`; `NetService::new` — без канала.
- `FakeGateway::dispatch(name, seq, data)` — сырой op0, kind через `DispatchKind::from_name`.

`rust/gui-spike/src/main.rs`:

- `TauriDispatchEmitter` → `emit("gateway://dispatch", &DispatchPayload)`, ошибка
  возвращается строкой без payload/секретов; подключён в `setup`
  (`NetService::with_emitter`).
- новая read-only команда `net_dispatch_stats` (без аргументов, токен не читает),
  зарегистрирована в `invoke_handler!`.
- `wiring_tests`: +3 теста (эмиттер/канал/команда), регистрация команды добавлена
  в существующий тест.

`CONTRACTS/dispatch.md` (новый): конверт, таблица evidence по 6 `kind`,
политика секретов, семантика доставки/дедупа, точное предложение по ядру с
альтернативами и оценкой, остаток (UI, REMOVE `[A]`).

`rust/gui-spike/tests/net_contract.rs` (аддитивно, 14 тестов L-02) — оракул: кадры
`by-type` + роутер ядра, не текущая реализация. Покрыто: `d` поле-в-поле
(PRESENCE, CHUNK); counts/ops и вложенные `session_id` (LIST_UPDATE); READY —
рекурсивный оракул секретов до/после; фильтр/raw/битый `d` без паники; конверт
ровно `{kind,data}`; порядок и неизменность статуса; ошибка эмита; блокирующий UI
не блокирует источник (backpressure); дедуп + reconnect без переигровки;
`sourceAvailable=false`; ETF→JSON (Nil/atom/int/float/tuple/list/int-ключ/Inf).

## 4. RED (до продуктового кода)

`waves/RW-40/PM/l02/red.log` — тесты написаны раньше модуля:

```
error[E0432]: unresolved imports `discord_gui_spike::net::etf_to_json`,
`…::map_dispatch`, `…::DispatchChannel`, `…::DispatchEmitter`, `…::DispatchError`,
`…::DispatchPayload`, `…::DispatchStats`, `…::CORE_DISPATCH_AVAILABLE`,
`…::DISPATCH_EVENT`, `…::FORWARDED_DISPATCHES`
```

Первый green-прогон вскрыл два **тестовых** оракула (не ослабление продукта):

1. порядок ключей JSON: `serde_json::Map` сортирует → проверяется набор
   `["data","kind"]`, а не порядок;
2. `GUILD_MEMBER_LIST_UPDATE` содержит `member.presence.activities[].session_id`
   (в фикстуре `"REDACTED"`) → «побайтовое» равенство заменено на: counts/ops/
   groups равны, секреты вырезаны на глубине, `redacted` = числу секретов
   оракула. Прод-политика не менялась (вырезание глубже — строже).

## 5. GREEN / targeted

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test --test net_contract` | **21 passed** (7 старых + 14 новых) | `targeted.log` |
| `cargo test --bin discord-gui-spike` | **9 passed** (6 старых + 3 новых) | `targeted-main.log` |
| `cargo test --manifest-path rust/gui-spike/Cargo.toml` | **268 passed, 0 failed** (baseline 251, +17) | `green.log` |
| `cargo test -p discord-core` | **574 passed, 0 failed** (baseline 574, не тронут) | `core.log` |
| `cargo clippy … --all-targets -- -D warnings` | чисто | `clippy.log` |
| `cargo fmt … -- --check` | чисто | `fmt.log` |
| `node --test tests/meta/ipc-path.test.mjs` | 12/12 | — |
| `node --test tests/meta/rust-client-sync.test.mjs` | 14/14 | — |

Ядро не менялось; UI (`ui/**`) не менялся; сеть/окно/live не запускались.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/net.rs` | `f21143f3c926db747e07f380df008dd0` |
| `rust/gui-spike/src/main.rs` | `0082fb4b8e4626519daf070a03d0e6c6` |
| `rust/gui-spike/tests/net_contract.rs` | `0d0ff80d5a0a6b57b220a651a587b5b2` |
| `CONTRACTS/dispatch.md` | `58f678f98cbf68b6640b00d6a0f87bcb` |

`rust/gui-spike/**` вне git-базы (как и в G6-отчёте): диффом не проверяется,
md5 — единственный слепок.

## 7. Риски / остаток

1. **Block (вне файлов карточки):** живой источник — хук ядра (`CONTRACTS/dispatch.md`
   §6). До него `gateway://dispatch` пуст, `sourceAvailable: false` — это видно UI,
   а не маскируется.
2. `MESSAGE_REACTION_REMOVE` в захватах **0** и отсутствует в 21 типе ядра:
   форвардится по имени `[A]`, форма не парсится. После первого live-захвата —
   решение PM: добавить тип в ядро или оставить raw.
3. UI-подписка и проекции по `kind` — G2, отдельный срез (`ui/**` read-only здесь).
4. Дедуп `s` — в пределах соединения; реплей после RESUME — ответственность ядра
   (resume от последнего `s`). Своего буфера/переигровки нет.
5. Backpressure: очередь 256; при переполнении событие отбрасывается и считается
   (`overflow`), источник не блокируется. Порог — наша политика (не RE-число).
6. `P2` в ядре: `GatewayPipe::events` не дренируется супервайзером — рост памяти
   на длинной сессии; предложенный хук его устраняет.
