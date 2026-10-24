# L-02-CORE-FINAL — хук dispatch-кадра в ядре (red-first)

Дата: 2026-10-24. Воркер: **G1 Core/Network** (отдельный Orca-воркер). PM: DeepSeek.
Статус: **готово к приёмке**. Сеть/live/окно/commit не использовались; сетевые кейсы —
только локальный WS-стаб `127.0.0.1`.

## 1. Что сделано

### Ядро (`discord-core`)

`rust/crates/discord-core/src/ws.rs`:

- `pub type DispatchHandler = Arc<dyn Fn(&DispatchEvent) + Send + Sync>`;
- `RunOptions { identify, on_dispatch: Option<DispatchHandler> }` — ручной `Debug`
  (`<handler>`), `Clone`/`Default` сохранены; `RunOptions::none()` — оба поля `None`;
- в `connect_and_run_resume` сразу после `pipe.push_binary(...)` (до `apply_effects`,
  чтобы кадр, закрывающий соединение, не терял свои op0) вызывается
  `flush_dispatches(pipe, &opts)`: при `Some` — `pipe.take_events()` отдаётся
  обработчику по одному и буфер очищается; при `None` — буфер не трогается
  (поведение прежнее).

`rust/crates/discord-core/src/gateway/supervisor.rs`:

- `SupervisorConfig.on_dispatch: Option<DispatchHandler>` (в `Debug` — `<handler>`);
- `SupervisorConfig::with_dispatch_handler(...)`;
- в `run_until` хук клонируется в `RunOptions` каждой попытки соединения.

### GUI-слой (`rust/gui-spike`)

`src/net.rs`:

- `core_supervisor_config(token, events)` — один callback получает и жизненный цикл
  (`map_supervisor_event`), и сырые `GatewayEvent::Dispatch(event.clone())`; вынесено
  из `CoreGatewayDriver::connect`, чтобы проводку можно было проверить без сети;
- `CORE_DISPATCH_AVAILABLE = true` — только потому, что хук реально подключён
  (драйвер вызывает `core_supervisor_config`, а не заглушку);
- `CORE_DISPATCH_DETAIL` называет механизм (`RunOptions.on_dispatch`).

`src/main.rs` — только комментарии (`sourceAvailable: true`), код команд не менялся.

`CONTRACTS/dispatch.md` — §1/§2/§6/§8/§9/§10 приведены к фактическому состоянию
(хук реализован; UI-срез G2 остаётся).

### Реальные события, без выдумок

Хук отдаёт **ровно** то, что положил в `GatewayPipe::events` роутер `route_dispatch`:
`name` — имя провода, `seq` — `s`, `data` — `d` целиком; raw-тип сохраняется
(`kind: None`, имя не теряется). Ничего не синтезируется и не переименовывается.
Фильтр L-02 (6 имён), вырезание секретов и конверт `{kind,data}` остаются в
контроллере, как раньше.

## 2. RED (до продуктового кода)

`waves/RW-40/PM/l02-core/red.log` — тест на fake dispatch (локальный WS-стаб отдаёт
HELLO → IDENTIFY → READY + PRESENCE_UPDATE + raw `MESSAGE_REACTION_REMOVE`) написан
раньше хука:

```
error[E0599]: no method named `with_dispatch_handler` found for struct `SupervisorConfig`
   --> crates/discord-core/tests/gateway_supervisor.rs:825:36
```

После появления поля в `RunOptions` полный прогон вскрыл ещё 4 механических
компиляционных места в чужих core-тестах (`RunOptions { identify: ... }` без нового
поля). Исправлено добавлением `on_dispatch: None` — поведение тестов не менялось:
`tests/identify_ready.rs` (2 места), `tests/resume_reconnect.rs` (2 места).

## 3. GREEN / тесты

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test -p discord-core --test gateway_supervisor` | **12 passed** (11 старых + 1 новый) | `green-core-targeted.log` |
| `cargo test -p discord-core` | **575 passed, 0 failed** (baseline 574, +1) | `green-core.log` |
| `cargo test --test net_contract` (gui-spike) | **22 passed** (21 старых, 1 переписан; +1 новый) | `green-net-contract.log` |
| `cargo test --bin discord-gui-spike` | **9 passed** | `green-gui-bin.log` |
| `cargo test` (весь gui-spike) | **269 passed, 0 failed** (baseline 268, +1) | `green-gui-all.log` |
| `cargo clippy -p discord-core --all-targets -- -D warnings` | чисто | `clippy-core.log` |
| `cargo clippy` (gui-spike, `--all-targets -D warnings`) | чисто | `clippy-gui.log` |
| `cargo fmt --all -- --check` (core и gui-spike) | чисто | `fmt-core.log`, `fmt-gui.log` |

Новые/изменённые тесты:

1. `supervisor_forwards_real_op0_frames_to_dispatch_handler` (core, стаб):
   хук получает `READY`, `PRESENCE_UPDATE` (`s=2`, `d` поле-в-поле), raw
   `MESSAGE_REACTION_REMOVE` (`kind: None`, `s=3`); имена/порядок/данные проверяются
   по кадрам стаба, а не по текущей реализации.
2. `core_supervisor_config_wires_lifecycle_and_dispatch` (net_contract): без сети —
   `config.on_event` и `config.on_dispatch` заполнены, оба идут в один callback;
   `GatewayEvent::Dispatch` несёт событие без переписывания; адрес — боевой.
3. `dispatch_stats_report_core_source_available` (заменяет прежний
   `..._source_gap`): `CORE_DISPATCH_AVAILABLE = true`, `source_available = true`,
   `source_detail` называет `on_dispatch`.

## 4. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/ws.rs` | `176791cc4e9a85173e8642cab02baa9b` |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `84f5fcc1404795faec939a24c15462d1` |
| `rust/crates/discord-core/tests/identify_ready.rs` | `f66237700b15298d86b8488ee4dc3634` |
| `rust/crates/discord-core/tests/resume_reconnect.rs` | `4418d45e351f945ed4046dd946d559c8` |
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` |
| `rust/gui-spike/src/main.rs` | `c003a9e597eea659beeff5d52e94768c` |
| `rust/gui-spike/tests/net_contract.rs` | `19f249688b72317811362f4b6f24a2c0` |
| `CONTRACTS/dispatch.md` | `4bfeef6c5851b994b75d4cd3d8a32ab1` |

`rust/gui-spike/**` вне git-базы (как и в L-02), поэтому md5 — единственный слепок.

Слепок `tests/gateway_supervisor.rs` в §4 исправлен на фактический (`84f5fcc1…`, как в
§6.4): прежнее значение `b9d806c6…` было снято до усиления теста в §6 и не совпадало с
артефактом ревью. Таблицы §4/§6.4 описывают **состояние на момент приёмки L-02**; после
FIX-L02-CORE (`ws.rs`, тесты, oracle) актуальные md5 — в `FIX-REPORT.md`.

Финальная сверка после приведения комментариев `net.rs` к фактическому состоянию
(`source_available` = подключённый источник) и `cargo fmt -p discord-core` (один
перенос строки в `ws.rs`): повторные прогоны — `final-core-575.log` (575/0),
`final-gui-269.log` (269/0), `final-net-contract.log` (22/0),
`final-clippy-gui.log` (чисто); `green-gui-bin.log` — 9/0.

## 5. Остаток / риски

1. **UI (G2):** подписка `listen("gateway://dispatch")` и проекции по `kind` не
   написаны (`ui/**` не трогался). Канал теперь живой, но UI его ещё не слушает.
2. **Live-проверка** «событие дошло до окна» — окно PM, здесь запрещено.
3. **Дедуп/порядок:** без изменений — `DispatchChannel` (очередь 256, `try_send`,
   порядок одного потребителя), дедуп `s` в пределах соединения.
4. **`MESSAGE_REACTION_REMOVE`:** в захватах 0, в 21 типе ядра нет — идёт raw по
   имени провода; после первого live-захвата решить: тип в ядре или raw.
5. **Буфер пайпа:** при подключённом хуке дренируется на каждом кадре; без хука
   (`Supervisor::new` без `with_dispatch_handler`) по-прежнему растёт — это
   осознанное «поведение при `None` не меняется».
6. **Семантика флага:** `CORE_DISPATCH_AVAILABLE` — про наличие источника
   (проводку хука в боевом драйвере), а не про состояние текущего соединения;
   статус (`Offline/Connecting/Ready`) остаётся отдельным полем.
7. **Существующее поведение L-01 (не менялось):** `SupervisorEvent::Ready`/
   `Reconnecting` эмитятся в `run_until` после возврата `connect_and_run_resume`,
   т.е. `NetController` не переходит в `Ready` в момент живого READY. Dispatch
   идёт независимо (хук внутри цикла), но UI-статус может отставать — карточка
   L-01/статус, вне этого среза.

## 6. Независимая проверка и усиление теста (15:26–15:35)

Проверку выполнял второй G1-воркер по той же карточке: в checkout одновременно
работал дубль (`opencode run --title L-02-CORE-DS`, pid 652354, 15:15–15:26),
который и внёс правки §1–§4. Он завершился; файлы после него не переписывались,
md5 §4 совпали. Правки ниже — только аддитивные: один новый тест в
`tests/gateway_supervisor.rs`, продуктовый код (`ws.rs`, `supervisor.rs`,
`net.rs`) не менялся (md5 §4 без изменений).

### 6.1 Что добавлено и почему

Прежний тест `supervisor_forwards_real_op0_frames_to_dispatch_handler` проверяет
события **после** закрытия соединения: мутация «копить и отдать пачкой на close»
им не ловится, хотя контракт (§6–§7 `CONTRACTS/dispatch.md`) требует доставку по
кадру. Добавлен `dispatch_handler_receives_frames_while_connection_is_open`:
локальный WS-стаб держит соединение открытым и **ждёт** появления события у хука,
и только потом шлёт следующий кадр и close. Оракул — порядок кадров стаба.

### 6.2 Мутация (доказательство, что тест ловит старое поведение)

`waves/RW-40/PM/l02-core/mutation-deferred-flush.log`: `flush_dispatches`
перенесён из `Message::Binary`-ветки в `Message::Close`-ветку.

```
test result: FAILED. 1 passed; 1 failed   (exit=101)
failures: dispatch_handler_receives_frames_while_connection_is_open
  "хук не отдал 1 диспатчей за 3 с незакрытого соединения"
```

Прежний тест на мутанте **прошёл** (`1 passed`) — пробел подтверждён. После
отката: `ws.rs` md5 `176791cc4e9a85173e8642cab02baa9b` (исходный), маркеров
`MUTANT` нет, тест снова зелёный.

### 6.3 Повторные прогоны (этот воркер, отдельные логи)

| Команда | Результат | Лог |
|---|---|---|
| `cargo test -p discord-core` | **576 passed, 0 failed** (было 575, +1 новый тест) | `verify-core-576.log` |
| `cargo test --test net_contract` (gui-spike) | **22 passed, 0 failed** | `verify-net-contract-22.log` |
| `cargo test` (весь gui-spike) | **269 passed, 0 failed** | `verify-gui-269.log` |
| `cargo fmt --all -- --check`, `cargo clippy -p discord-core --all-targets -- -D warnings` | exit 0 | `verify-clippy-fmt.log` |

### 6.4 Итоговые md5 (после усиления теста)

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/ws.rs` | `176791cc4e9a85173e8642cab02baa9b` |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `84f5fcc1404795faec939a24c15462d1` |
| `rust/crates/discord-core/tests/identify_ready.rs` | `f66237700b15298d86b8488ee4dc3634` |
| `rust/crates/discord-core/tests/resume_reconnect.rs` | `4418d45e351f945ed4046dd946d559c8` |
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` |
| `rust/gui-spike/src/main.rs` | `c003a9e597eea659beeff5d52e94768c` |
| `rust/gui-spike/tests/net_contract.rs` | `19f249688b72317811362f4b6f24a2c0` |
| `CONTRACTS/dispatch.md` | `4bfeef6c5851b994b75d4cd3d8a32ab1` |

### 6.5 Наблюдения ревью (не блокеры)

1. **Синхронная конвертация в цикле соединения.** `DispatchChannel` разгружает
   только `emit`; `etf_to_json` + вырезание секретов выполняются в потоке
   супервайзера на каждый кадр. Для 6 форвардимых типов объём ограничен, но
   при расширении фильтра это стоит вынести в поток канала (P2).
2. **Смена ожидания в `net_contract`** (`..._source_gap` → `..._source_available`)
   — изменение контракта по `CONTRACTS/dispatch.md` §10 п.1; по `TEST-POLICY` §5
   нужна запись `old → new → основание` в `DECISIONS.md` (файл PM, воркер не
   правит).
3. **Дубль-воркер.** Карточку одновременно выполняли два процесса в одном
   checkout (`LOAD-BALANCE.md`: «не запускать второй L-02 worker»). Итог
   консистентен, но процесс стоит поправить: второй запуск мог затереть правки.
4. `CORE_DISPATCH_AVAILABLE` — про наличие проводки, не про `status` соединения
   (см. §5 п.6); UI должен смотреть на `status` отдельно.
