### ТЗ RW-03 — Сессия шлюза Discord (прототип без транспорта), P1-03 / T-GW-01…03

**Цель**: детерминированное ядро сессии шлюза в крейте `discord-core` (модуль `gateway`): обработка
HELLO, планирование heartbeat (интервал 41 250 мс; первый удар — jitter), учёт ACK и RTT, детектор
зомби (нет ACK за интервал → `Reconnect{Zombie}`), отслеживание `s`-последовательности, реакции на
op7/op9. **Транспорт (сокет/TLS/WS) — не в этой волне** (RW-04: нужны крейты tokio/tungstenite и
отдельное разрешение владельца); здесь — чистая логика на виртуальных часах: `now_ms: u64`
передаётся вызывающим, `std::time` в модуле не используется (детерминизм).

**Файлы (только эти)**:
- `rust/crates/discord-core/src/gateway.rs` — новый модуль (реализация + доккомментарии по-русски).
- `rust/crates/discord-core/src/lib.rs` — точечная правка: `pub mod gateway;`, `pub use discord_codec
  as codec;` (ре-экспорт кодека для потребителей и интеграционных тестов), re-export ключевых типов
  `gateway`, обновить строку статуса.
- `rust/crates/discord-core/tests/gateway_session.rs` — новый тест-файл.

**Запрещено**: другие файлы/крейты, `Cargo.toml`/`Cargo.lock` (**новых зависимостей нет**), `docs/`,
`TASKS.md`, golden (`logs/`, `functions/`, `evidence/`), сеть, коммиты. Формат — только
`cargo fmt -p discord-core`.

**Публичный API** (форма — на усмотрение воркера в этих границах; тесты — только через публичный API):

```rust
pub enum Op { Hello, Heartbeat, HeartbeatAck, Dispatch, Reconnect, InvalidSession, Unknown(i64) }
pub struct Hello { pub heartbeat_interval_ms: u64 }
pub enum ReconnectReason { Zombie, ServerRequest, InvalidSession }
pub enum Effect { SendHeartbeat { d: Option<u64> }, Reconnect { reason: ReconnectReason } }
pub enum GatewayError { Decode { msg: String }, Malformed { msg: String } }
pub struct Session { /* приватное состояние */ }
impl Session {
    pub fn new(jitter_seed: u64) -> Self;
    pub fn on_term(&mut self, term: &[u8], now_ms: u64) -> Result<Vec<Effect>, GatewayError>;
    pub fn on_value(&mut self, value: &EtfValue, now_ms: u64) -> Result<Vec<Effect>, GatewayError>;
    pub fn on_timer(&mut self, now_ms: u64) -> Vec<Effect>;
    pub fn next_deadline_ms(&self) -> Option<u64>;
    pub fn hello(&self) -> Option<&Hello>;
    pub fn last_seq(&self) -> Option<u64>;
    pub fn rtt_ms(&self) -> Option<u64>;
    pub fn beats_emitted(&self) -> u64;
    pub fn is_dead(&self) -> bool;
}
```

**Семантика (точная; проверена симуляцией координатора на реальном таймлайне)**:
1. `on_value(op10 HELLO)`: `d.get("heartbeat_interval").as_int()` → `hello`; первый удар планируется
   на `now + (jitter_seed % interval)`. Повторный HELLO до первого удара — пере-планировать; после
   первого удара — игнорировать.
2. `on_timer(now)`:
   - мёртв → пусто;
   - есть неответный удар (`pending`) и `now ≥ pending_sent + interval` → `Effect::Reconnect{Zombie}`,
     сессия мертва;
   - иначе, если `now ≥ next_due` → `Effect::SendHeartbeat{d: last_seq}` (до первого диспатча
     `d = None`), `pending = now`, `next_due = None`.
3. `on_value(op11 ACK)`: если есть `pending` → `rtt = now − pending_sent`, `next_due = now + interval`,
   `pending = None`. ACK без `pending` — игнорируется.
4. `on_value(op0)`: `s` (int) → `last_seq`. `t`/`d` в этой волне не анализируются.
5. `op7` → `Reconnect{ServerRequest}`; `op9` → `Reconnect{InvalidSession}`; сессия мертва.
6. `op1/op2/op40/…` и любые неизвестные значения / мусорные термы — без эффектов и паник (счётчик
   опционально); ошибки декодирования → `GatewayError::Decode`.
7. `next_deadline_ms()`: min(следующий удар, дедлайн зомби) либо `None`.
8. `unsafe` запрещён линтом; никаких `unwrap`/паник на входных данных.

**Оракулы**:
- HELLO-терм (138 Б hex; из захвата, op10, `heartbeat_interval = 41250`; ведущий `0x83` + 137 Б терма):
  `8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c`
- Таймлайн: `{RE_LAB_ROOT:-reference-oracle/}/gw-proto/timeline-95974_1389.tsv`
  (md5 `23f38750227ae79166748323199054d7`; формат: 3 строки `#`-заголовка, затем
  `i \t ts \t op \t s \t t`; `-` = nil). Провенанс: `capture-run2.jsonl` (ts) +
  `gw-proto/msgs-run2-95974_1389.jsonl` (op/s/t); генератор — `reference-oracle/gw-make-timeline.mjs`.
- Факты таймлайна (перепроверены координатором): событий 137; op10 = 1 (первое, ts 1792391312488);
  op11 = 23; op0 = 113 (s: 1..113 монотонно); first_ack_delta = 1214 мс; last_ack_delta = 658927 мс;
  max ACK-дельта = 41 263 мс; ≥9 дельт в [41 050; 41 300] (каденция 41 250); в ACK-потоке есть
  «лишние» ACK между ударами (парные суммы ≈ 41 250) — не сопоставлять с ударами.
- Симуляция координатора (та же семантика п.1–4) для всех 41 250 сидов: зомби не срабатывает
  ни разу; число ударов 11–12 (seed = 1214 → 12).
- Пайплайн (тест 10): `{RE_LAB_ROOT}/gw-proto/zstd-frames-95974_1389.hexl` — после подачи первых
  3 кадров первые 138 Б расшифровки равны HELLO-терму (проверено RW-02).

**Кейсы (red-first; имена латиницей, комментарии по-русски)**:

Юнит (без lab-файлов; значения `EtfValue` строятся напрямую):
1. `hello_term_parses_interval` — `on_term(HELLO_HEX, 1000)` → `hello().heartbeat_interval_ms == 41250`;
   `next_deadline_ms() == Some(1000 + jitter_seed % 41250)`.
2. `first_beat_delay_is_seed_mod_interval` — сиды `[0, 1, 1214, 41249]`: дедлайн == `t + seed % 41250`,
   и всегда `< t + 41250`.
3. `first_beat_has_null_sequence` — `new(10)`, HELLO на t=1000: `on_timer(1010)` →
   `[SendHeartbeat{d: None}]`; `next_deadline_ms() == Some(42260)` (1010 + 41250, дедлайн зомби);
   `on_timer(1011)` → пусто.
4. `beats_carry_last_sequence` — диспатчи s=1..3 → удар → `d == Some(3)`.
5. `ack_measures_rtt_and_reschedules` — удар на 1000, ACK на 1222 → `rtt_ms() == Some(222)`,
   `next_deadline_ms() == Some(1222 + 41250)`.
6. `missed_ack_is_zombie` — удар на 1000, ACK нет: `on_timer(1000 + 41250)` → `[Reconnect{Zombie}]`;
   `is_dead()`; дальнейшие `on_timer`/`on_value` — пусто/Ok.
7. `op7_and_op9_reconnect` — `{op:7}` → `Reconnect{ServerRequest}`; `{op:9}` →
   `Reconnect{InvalidSession}` (op9 в захвате не наблюдался — комментарий).
8. `unknown_ops_and_garbage_do_not_panic` — op40/999 и ~100 мусорных термов (`on_term`) → без паник;
   мусор → `Err(Decode)`.
9. `heartbeat_term_roundtrips` — `etf::encode({op:1, d:Some(42)})` → `etf::decode` → op=1, d=42;
   то же для `d: None`; длина терма ≤ 64 Б.

Lab (skip-aware; нет файла → `eprintln!("skip: …")` + return; прочие ошибки чтения не глотать):
10. `lab_hello_via_zstd_pipeline` — `ZstdStream` по `zstd-frames-95974_1389.hexl`: подать первые
    3 кадра → первые 138 Б расшифровки == HELLO_HEX; `Session.on_term` → интервал 41250.
11. `lab_timeline_facts` — из tsv: 137 событий; первое — op10; op11 = 23; op0 = 113 и s: 1..113
    монотонно; first_ack_delta == 1214; last_ack_delta == 658927; все дельты ≤ 41263;
    дельт в [41050; 41300] ≥ 9.
12. `lab_replay_no_zombie` — `Session::new(1214)`; проход по tsv: на каждом событии `on_timer(ts)`,
    затем `on_value` (для op10/op0/op11; lab-термов нет — значения строятся из полей строки);
    assert: `is_dead() == false`; `beats_emitted()` ∈ 11..=12; `last_seq() == Some(113)`;
    `rtt_ms().is_some()`.

**DoD**: red-прогон (заглушка) — все новые тесты падают (`/tmp/opencode/rw-03/red.txt`); green:
`cd rust && cargo test --offline -p discord-core` и весь воркспейс (`cargo test --offline`);
`cargo clippy --offline -p discord-core --all-targets -- -D warnings`; `cargo fmt -p discord-core -- --check`;
`npm run gate:worker` — passed; отчёт §4.2 + md5 файлов. §8-строка не требуется (Rust-тесты вне
meta-регистра).
