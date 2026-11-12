# ТЗ RW-07 — роутер диспатчей (P1-06 / F-GW-06 / T-GW-08)

**Проект:** discord-lab + Rust-клиент. Корень: `/home/user/orca/project`; cargo-воркспейс: `rust/`.
**Цель:** op0-диспатчи шлюза → типизированные события по 21 наблюдаемому типу (`F-GW-06`);
неизвестный тип — raw-событие (`kind: None`), имя сохраняется, есть накопительный счётчик;
payload `d` **не разбирается** (разбор — P2-01) и передаётся вызывающему целиком.

Роль-волна RW-07 = карточка плана **P1-06** (`docs/rust-client/PLAN.md:57`), тест **T-GW-08**
(`docs/rust-client/TEST-PLAN.md:62`). T-DATA-01 (разбор READY-гильдии) — это P2-01, в этой волне
НЕ делается (роутер отдаёт payload без разбора).

## 0. Файлы (только эти; больше ничего)

1. `rust/crates/discord-core/src/dispatch.rs` — **новый** модуль.
2. `rust/crates/discord-core/src/ws.rs` — правка: очередь событий в `GatewayPipe`.
3. `rust/crates/discord-core/src/lib.rs` — правка: `pub mod dispatch;` + ре-экспорты.
4. `rust/crates/discord-core/tests/dispatch_router.rs` — **новый** тест-файл (11 кейсов).
5. Артефакты вне git: `/tmp/opencode/rw-07/` — `red.log`, `green.log`, `dispatch_router-red.rs`, `report.md`.

**Запрещено:** `Cargo.toml`/`Cargo.lock` (зависимостей достаточно: `zstd` — уже dev-dependency
`discord-core`; сеть выключена, всё с `--offline`); `TASKS.md`; `docs/**`; `README.md`;
чужие тесты/модули/файлы; `logs/`, `functions/`, `evidence/`; `reference-oracle/` — только
чтение; `git commit/push`; правка тестов под код и ослабление ассертов; спавн Discord.

## 1. Контракт (точный; имена и семантика — предмет приёмки)

### 1.1 `dispatch.rs`

```rust
//! Роутер диспатчей шлюза (RW-07 / P1-06): op0 → типизированные события; неизвестные → raw.
use discord_codec::etf::EtfValue;

/// 21 наблюдаемый тип op0 (F-GW-06, `gateway-protocol.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchKind { /* ровно 21 вариант, см. таблицу */ }

impl DispatchKind {
    /// Все 21; каждый ровно один раз (порядок свободный, но фиксированный в коде).
    pub const ALL: [DispatchKind; 21] = [ /* … */ ];
    /// Точное совпадение (регистрозависимо), `name` — имя провода.
    pub fn from_name(name: &str) -> Option<Self>;
    /// Имя типа в проводе (`"MESSAGE_CREATE"`, `"READY"`, …).
    pub fn as_str(self) -> &'static str;
}

/// Событие op0 после роутинга. Payload не парсится — в этом волна P1-06.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchEvent {
    /// `Some(kind)` — известный тип; `None` — неизвестный (raw-лог).
    pub kind: Option<DispatchKind>,
    /// Имя типа как пришло (для известных — `kind.as_str()`); не теряется и для raw.
    pub name: String,
    /// `s` диспатча; `None` — поля нет либо не Int ≥ 0.
    pub seq: Option<u64>,
    /// `d` целиком (клонирование — плата за владение; оптимизация не нужна).
    /// `d` отсутствует → `EtfValue::Nil`.
    pub data: EtfValue,
}

/// op0 + строковый `t` → событие; иначе `None` (не диспатч).
pub fn route_dispatch(value: &EtfValue) -> Option<DispatchEvent>;
```

Семантика `route_dispatch` (по шагам, строго):

1. `op`: `value.get("op").and_then(EtfValue::as_int) == Some(0)` — иначе `None`
   (op10/op11/op7/op9, отсутствие `op`, `op` не Int — всё `None`; неизвестным типом это
   не считается и в счётчик raw не попадает).
2. `t`: только `EtfValue::Atom(name) | EtfValue::Str(name)` (как `ReadySummary::from_dispatch`);
   иначе `None`.
3. `kind = DispatchKind::from_name(name)`; `name` сохраняется как пришло (для известных он
   равен `kind.as_str()` — проверяется тестом).
4. `seq`: `value.get("s").and_then(EtfValue::as_int)`, затем `u64::try_from(..).ok()`
   (отрицательный/`"nil"`/отсутствие → `None`).
5. `data`: `value.get("d").cloned().unwrap_or(EtfValue::Nil)`.
6. Неизвестный `t` → `Some(DispatchEvent { kind: None, .. })` — это raw, не ошибка.

### 1.2 Таблица 21 типа (вариант ⇄ имя провода) — единственно верная

| Имя провода | Вариант |
|---|---|
| `CHANNEL_INFO` | `ChannelInfo` |
| `GUILD_MEMBERS_CHUNK` | `GuildMembersChunk` |
| `GUILD_MEMBER_LIST_UPDATE` | `GuildMemberListUpdate` |
| `MESSAGE_ACK` | `MessageAck` |
| `MESSAGE_CREATE` | `MessageCreate` |
| `MESSAGE_POLL_VOTE_ADD` | `MessagePollVoteAdd` |
| `MESSAGE_REACTION_ADD` | `MessageReactionAdd` |
| `MESSAGE_UPDATE` | `MessageUpdate` |
| `PRESENCE_UPDATE` | `PresenceUpdate` |
| `READY` | `Ready` |
| `READY_SUPPLEMENTAL` | `ReadySupplemental` |
| `RESUMED` | `Resumed` |
| `SESSIONS_REPLACE` | `SessionsReplace` |
| `SOUNDBOARD_SOUNDS` | `SoundboardSounds` |
| `THREAD_LIST_SYNC` | `ThreadListSync` |
| `TYPING_START` | `TypingStart` |
| `USER_SETTINGS_PROTO_UPDATE` | `UserSettingsProtoUpdate` |
| `VOICE_CHANNEL_START_TIME_UPDATE` | `VoiceChannelStartTimeUpdate` |
| `VOICE_CHANNEL_STATUS_UPDATE` | `VoiceChannelStatusUpdate` |
| `VOICE_SERVER_UPDATE` | `VoiceServerUpdate` |
| `VOICE_STATE_UPDATE` | `VoiceStateUpdate` |

### 1.3 `ws.rs` — `GatewayPipe`

Добавить **только** (существующие поля/методы/сигнатуры не менять):

- поля: `events: Vec<DispatchEvent>`, `unknown_dispatches: u64` (инициализация в `new`);
- в цикле `push_binary`, после блока op9, **до** `self.tail.drain(..consumed)`:

```rust
if let Some(event) = route_dispatch(&value) {
    if event.kind.is_none() {
        self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
    }
    self.events.push(event);
}
```

- методы:

```rust
/// Диспатчи, накопленные с последнего дренажа (в порядке потока), включая raw.
pub fn events(&self) -> &[DispatchEvent];
/// Забирает все накопленные события (дренаж; вызывающий обязан разгружать буфер).
pub fn take_events(&mut self) -> Vec<DispatchEvent>;   // std::mem::take
/// Диспатчей с неизвестным типом (raw) — накопительно, дренаж не сбрасывает.
pub fn unknown_dispatches(&self) -> u64;
```

Док-комментарии: буфер растёт линейно с потоком — вызывающий дренирует (см. `take_events`);
неизвестный тип не молчит: событие с `kind: None` + счётчик.

### 1.4 `lib.rs`

- `pub mod dispatch;` (модули — по алфавиту);
- `pub use dispatch::{DispatchEvent, DispatchKind, route_dispatch};`
- допустимо одно упоминание RW-07 в doc-шапке статуса; существующие константы
  (`DISPATCH_COUNT_R7 = 1340`, `DISPATCH_TYPE_COUNT_R7 = 21`) не трогать.

## 2. Кейсы (red-first) — `tests/dispatch_router.rs`, 11 штук

Тест-файл самодостаточный: хелперы скопировать по паттерну соседних файлов (не импортировать
из чужих тестов): `wire` (= `etf::encode`), `zstd_message` (**завершённый** zstd-кадр,
уровень 1, `WindowLog(10)` — как `resume_reconnect.rs:204`), `hello_bytes(41250)`
(`resume_reconnect.rs:192`), `unhex`, `read_opt`, `lab_root`
(`env RE_LAB_ROOT`, дефолт `reference-oracle/`), `json_int_field`/`json_str_field`
(`resume_reconnect.rs:438/452` — мини-парсер верхнеуровневых полей jsonl), синтетические
термы через `map(vec![...])`/`list(vec![...])`-паттерн соседних тестов.

**Юнит (без сокетов):**

- **U1 `kind_table_covers_21_observed_types`**: `DispatchKind::ALL.len() == 21`; множество
  `ALL.map(as_str)` (BTreeSet) == оракульный список 21 имени (§3.1); дублей нет;
  `DISPATCH_TYPE_COUNT_R7 == 21`.
- **U2 `kind_from_name_exact_roundtrip`**: для всех `kind` из `ALL`:
  `from_name(kind.as_str()) == Some(kind)`; `from_name("guild_member_list_update") == None`
  (регистрозависимо); `from_name("") == None`; `from_name("nil") == None`.
- **U3 `route_known_keeps_seq_and_payload`**: term `{op:0, t:Atom("MESSAGE_CREATE"), s:42,
  d:List([Int(7), Str("x")])}` → `Some`: `kind == Some(MessageCreate)`, `name == "MESSAGE_CREATE"`,
  `seq == Some(42)`, `data == List([Int(7), Str("x")])`. Второй под-кейс: `t` как `Str("READY")`
  → `Some`, `kind == Some(Ready)` (оба представления `t` допустимы).
- **U4 `route_unknown_is_raw_with_name`**: `t:Atom("FUTURE_EVENT_X")`, `s:5`,
  `d:Map([("k", Int(1))])` → `kind == None`, `name == "FUTURE_EVENT_X"`, `seq == Some(5)`,
  `data == Map([("k", Int(1))])`; второй под-кейс с `t` как `Str`.
- **U5 `route_ignores_non_dispatches`** (негативный контроль): op10/op11/op7/op9 → `None`;
  op0 без `t` → `None`; op0 `t:Int(5)` → `None`; без `op` → `None`.
- **U6 `route_seq_and_data_optional`**: op0 `t:TYPING_START` без `s` → `seq == None`;
  без `d` → `data == EtfValue::Nil`; `s:Int(-1)` → `seq == None`.

**Пайп (офлайн, через `zstd_message` + `push_binary`, без сокетов):**

- **P1 `pipe_routes_dispatches_in_order`**: один кадр с термами `[hello_bytes(41250),
  READY(s=1, d={guilds: []}), MESSAGE_CREATE(s=2), FUTURE(s=3)]` → один `push_binary`:
  `events()` == 3: `[Ready s1, MessageCreate s2, kind None s3]` (порядок и имена);
  `unknown_dispatches() == 1`; `unknown` — вызывающему видно как `kind: None`;
  `session().last_seq() == Some(3)`; `terms_parsed() == 4`; `last_ready().is_some()`.
- **P2 `pipe_take_events_drains_once`**: тот же фид: `take_events().len() == 3`, затем
  `events().is_empty()`, повторный `take_events()` пуст; `unknown_dispatches() == 1`
  (накопительно, не сброшен).
- **P3 `pipe_hello_only_yields_no_events`** (негативный контроль): только HELLO →
  `events().is_empty()`, `unknown_dispatches() == 0`.

**Lab (skip-aware: нет файла → `eprintln!("skip: …")` + `return`):**

- **L1 `lab_all_dispatch_types_route_to_kinds`** (T-GW-08, union): просканировать все 7 файлов
  `{lab_root}/gw-proto/msgs-*.jsonl` (список в §3.1); по строкам с `op == 0` собрать `(t, s)`
  (`json_str_field("t")`, `json_int_field("s")`); assert: уникальных типов **21**, всего **1340**,
  потиповые числа — точно §3.1; сверить с `DISPATCH_COUNT_R7`/`DISPATCH_TYPE_COUNT_R7`.
  Затем каждый `(t, s)` → синтетический терм `{op:0, t:Atom(t), s:Int(s), d:Map([])}` →
  `route_dispatch` == `Some`, `kind == Some(…)`, `name == t`; накопить потиповые счётчики и
  сверить с §3.1; терминов с `kind: None` — **0**.
- **L2 `lab_real_stream_routes_known_kinds`** (реальные кадры): `{lab_root}/gw-proto/
  zstd-frames-95974_1389.hexl` — `frames.len() == 137`, сжатых байт `104_066`;
  подать все кадры в `ZstdStream::new()` (паттерн `gateway_session.rs::lab_hello_via_zstd_pipeline`),
  накопить распакованное (`out.len() == 759_280`) и разобрать термы циклом `etf::decode`
  (хвост `704` Б остаётся — assert). Assert: термов **44**, из них op0 **36** (s = 1..36),
  не-диспатчей **8**; потиповые числа — точно §3.2; все `kind == Some`; raw — **0**;
  первый routed — `Ready` с `seq == Some(1)`, последний routed — `seq == Some(36)`.

Негативные контроли (U2 на заглушке, U5, P3) при RED проходят по построению — это нормально,
зафиксировать в логе/отчёте. Все прочие кейсы обязаны упасть.

## 3. Оракулы и провенанс (перепроверены координатором 2026-10-19)

### 3.1 Union диспатчей — 7 файлов `msgs-*.jsonl`, 21 тип, 1340 op0

Файлы: `msgs-run1-57992_5112.jsonl`, `msgs-run2-95974_{1389,5532,8269,8608,10029,13072}.jsonl`.

| Тип | Кол-во |
|---|---|
| `GUILD_MEMBER_LIST_UPDATE` | 1048 |
| `PRESENCE_UPDATE` | 107 |
| `MESSAGE_CREATE` | 31 |
| `TYPING_START` | 28 |
| `MESSAGE_UPDATE` | 24 |
| `SOUNDBOARD_SOUNDS` | 21 |
| `GUILD_MEMBERS_CHUNK` | 11 |
| `SESSIONS_REPLACE` | 8 |
| `VOICE_STATE_UPDATE` | 8 |
| `MESSAGE_ACK` | 8 |
| `THREAD_LIST_SYNC` | 7 |
| `CHANNEL_INFO` | 7 |
| `VOICE_CHANNEL_START_TIME_UPDATE` | 7 |
| `READY` | 4 |
| `READY_SUPPLEMENTAL` | 4 |
| `MESSAGE_POLL_VOTE_ADD` | 4 |
| `VOICE_SERVER_UPDATE` | 3 |
| `VOICE_CHANNEL_STATUS_UPDATE` | 3 |
| `RESUMED` | 3 |
| `USER_SETTINGS_PROTO_UPDATE` | 3 |
| `MESSAGE_REACTION_ADD` | 1 |

Сумма = 1340, типов = 21 (совпадает с `F-GW-06` и константами `lib.rs`).

### 3.2 Реальный сокет `95974.1389` (факты RW-02/RW-04)

137 кадров, 104 066 сжатых Б → 824 816 Б распаковки; доступно **759 280** (окно ruzstd 65 536);
**44** полных терма (последний — s=36), в хвосте 704 Б обрезанного терма. Из 44:
**36 op0** (s = 1..36) и **8 не-диспатчей** (op10/op11). Потиповые числа op0-префикса (s ≤ 36):

| Тип | Кол-во |
|---|---|
| `GUILD_MEMBER_LIST_UPDATE` | 13 |
| `PRESENCE_UPDATE` | 7 |
| `MESSAGE_UPDATE` | 6 |
| `MESSAGE_CREATE` | 3 |
| `SESSIONS_REPLACE` | 2 |
| `THREAD_LIST_SYNC` | 2 |
| `READY` | 1 |
| `READY_SUPPLEMENTAL` | 1 |
| `GUILD_MEMBERS_CHUNK` | 1 |

Сумма = 36. В полном потоке 113 диспатчей (таймлайн `timeline-95974_1389.tsv`) — хвост
удержан декодером, не потерян (нит P1-02); в тесте это закомментировать.

## 4. RED-процедура (обязательна; доказательства — в логах)

1. Написать `tests/dispatch_router.rs` **целиком** (финальная редакция).
2. Заглушки, чтобы файл компилировался и падал ассертами (не компиляция, а именно ассерты):
   - `dispatch.rs`: enum со всеми 21 вариантом; `pub const ALL: [DispatchKind; 0] = [];`;
     `from_name` → `None`; `as_str` → `""`; struct `DispatchEvent` как в контракте;
     `route_dispatch` → `None`;
   - `ws.rs`: поля/методы по контракту есть, но в цикле роутер **не** вызывается;
   - `lib.rs`: модуль/ре-экспорты есть.
   Ожидание: падают U1, U3, U4, U6, P1, P2, L1, L2 (8); проходят негативные контроли U2
   (пустой `ALL` гасит цикл), U5, P3 (3). Любое другое распределение — разбираться, не подгонять.
3. Прогон и фиксация (из `rust/`):

```bash
cd /home/user/orca/project/rust
bash -c 'cargo test --offline -p discord-core --test dispatch_router 2>&1 | tee /tmp/opencode/rw-07/red.log; echo "# EXIT=${PIPESTATUS[0]}" | tee -a /tmp/opencode/rw-07/red.log'
md5sum crates/discord-core/tests/dispatch_router.rs | tee -a /tmp/opencode/rw-07/red.log
cp crates/discord-core/tests/dispatch_router.rs /tmp/opencode/rw-07/dispatch_router-red.rs
md5sum /tmp/opencode/rw-07/dispatch_router-red.rs | tee -a /tmp/opencode/rw-07/red.log
```

   Ожидаемый `# EXIT=101`. **Любая** правка теста после red → red заново: md5 в red-логе
   обязан совпасть с финальным тестом (нит N4 ревью RW-05/06).
4. Реализация (green). Если после green правились продуктовые файлы — `touch` их и перепрогнать
   (инцидент залипшего rlib, см. `TASKS.md:678`); финальные прогоны — на чистой затронутой сборке.

## 5. GREEN-процедура

Из `rust/` (все с `--offline`; лог в `/tmp/opencode/rw-07/green.log`, с `# EXIT=`):

1. `cargo test --offline -p discord-core --test dispatch_router` → 11/11.
2. `cargo test --offline -p discord-core` → 70 (59 существующих не сломаны).
3. `cargo test --offline --workspace` → 112.
4. `cargo test --offline -p discord-core --test dispatch_router` ещё 4 раза (×5 стабильно).
5. `cargo clippy --offline --workspace --all-targets -- -D warnings` → чисто.
6. `cargo fmt` (при необходимости) и `cargo fmt --check` → чисто.
7. Из корня `/home/user/orca/project`: `npm run gate:worker` → passed; запомнить путь
   `.gate/worker-*.json`.
8. `md5sum` всех четырёх изменённых файлов — в отчёт.

## 6. Отчёт (§4.2, `/tmp/opencode/rw-07/report.md`)

```
- сделано: …
- red: <команда> → <N failed/M passed, # EXIT=…, md5 теста>;
- green: <команды> → <результаты, N/N; md5 файлов>;
- gate:worker: <вердикт + файл .gate/…>;
- файлы: <4 файла + артефакты>;
- риски/вопросы: <…, включая оговорку про окно ruzstd и defer T-DATA-01 → P2-01>.
```

Финальное сообщение воркера — то же по смыслу + пути к логам/отчёту.

## 7. DoD

- [ ] red-прогон зафиксирован: `red.log` с `# EXIT=101`, распределением падений и md5 теста;
      побайтовая red-копия `/tmp/opencode/rw-07/dispatch_router-red.rs`;
- [ ] green: `dispatch_router` 11/11, `discord-core` 70, воркспейс 112, ×5 стабильно;
      clippy `-D warnings` и `fmt --check` чисто; `npm run gate:worker` passed;
- [ ] отчёт §4.2; md5 всех файлов; в отчёте — явная оговорка, что T-DATA-01 не входит (P2-01);
- [ ] границы: изменены только 4 файла ТЗ (`git status` / md5).
