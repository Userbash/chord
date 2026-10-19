### ТЗ RW-04 — Транспорт шлюза: WS-клиент + пайп (P1-03, продолжение; T-GW-01/03 полностью)

**Разрешение владельца (2026-10-19)**: разово разрешено скачать новые крейты (tokio,
tokio-tungstenite, rustls, webpki-roots, futures-util) и dev-зависимость `zstd`. Сеть — только
crates.io и сборка; **никаких подключений к живому Discord**; все тесты — локальный стаб
`127.0.0.1`.

**Цель**: поверх ядра сессии RW-03 — рабочий WS-клиент шлюза: соединение (`ws://` и `wss://`
через rustls+ring), чтение бинарных кадров, конвейер zstd→ETF-термы→`Session`, отправка heartbeat
по эффектам, таймеры (срок удара/зомби), исходы `Reconnect`/`Closed` и ошибки без паник.
Живой протокол: server→client — непрерывный zstd-поток (WS-сообщения = фрагменты одного кадра),
client→server — несжатые ETF-термы.

**Файлы (только эти)**:
- `rust/Cargo.toml` — `[workspace.dependencies]`: + tokio, tokio-tungstenite, rustls,
  webpki-roots, futures-util, zstd.
- `rust/crates/discord-core/Cargo.toml` — deps (+ dev-deps для тестов).
- `rust/Cargo.lock` — обновляется автоматически.
- `rust/crates/discord-core/src/ws.rs` — новый модуль (пайп + соединение + цикл).
- `rust/crates/discord-core/src/lib.rs` — `pub mod ws;`, re-export ключевых типов, статус.
- `rust/crates/discord-core/tests/ws_transport.rs` — новый тест-файл (+ при необходимости
  `rust/crates/discord-core/tests/common/mod.rs`).

**Зависимости (стартовые версии; точные — по факту сборки, фиксирует Cargo.lock)**:
```toml
tokio = { version = "1.53", features = ["rt-multi-thread", "macros", "net", "time"] }
tokio-tungstenite = { version = "0.30", features = ["rustls-tls-webpki-roots"] }
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }
webpki-roots = "1"
futures-util = { version = "0.3", default-features = false, features = ["sink", "std"] }
# dev (тесты): zstd = "0.14" (энкодер для стаба), tokio/tokio-tungstenite/futures-util — те же
```
Требования: TLS-провайдер — **ring** (не aws-lc: без cmake-усложнений). Если rustls в связке с
tokio-tungstenite потребует установленного провайдера — один раз вызывать
`rustls::crypto::ring::default_provider().install_default()` при подключении (ошибку «уже
установлен» не считать фатальной, не паниковать). Если ring не собирается — сообщить в отчёте и
оставить только `ws://`, но сначала попробуй ring.

**API (границы; детали — на усмотрение воркера)**:
```rust
pub struct GatewayPipe { /* ZstdStream + tail: Vec<u8> + Session + счётчики */ }
impl GatewayPipe {
    pub fn new(session: Session) -> Self;
    pub fn push_binary(&mut self, msg: &[u8], now_ms: u64) -> Result<Vec<Effect>, TransportError>;
    pub fn session(&self) -> &Session;
    pub fn frames_received(&self) -> u64;
    pub fn bytes_received(&self) -> u64;
    pub fn terms_parsed(&self) -> u64;
}
pub fn encode_heartbeat(d: Option<u64>) -> Vec<u8>; // ETF-терм {op:1,d} с ведущим 0x83
pub enum RunOutcome { Reconnect { reason: ReconnectReason }, Closed { code: Option<u16> } }
pub enum TransportError { /* Connect/Ws/Zstd/Frame(GatewayError)/… */ }
// impl Display + std::error::Error; никаких паник/unwrap на входных данных
pub async fn connect_and_run(url: &str, pipe: &mut GatewayPipe) -> Result<RunOutcome, TransportError>;
```
Пайп: `ZstdStream::push(msg)` → новые байты в хвост; пока хвост начинается с `0x83` — `etf::decode`
(он принимает префикс); `EtfError::UnexpectedEof` → ждать следующий push (хвост копится);
успех → `session.on_value(term, now_ms)`, снять `consumed`; прочие ошибки → `TransportError::Frame`.

Цикл: `select!` по (a) следующее WS-сообщение, (b) сон до `session.next_deadline_ms()`
(в мс от старта, `tokio::time::Instant`). На каждом шаге: `now_ms = start.elapsed().as_millis()`;
таймеры — `session.on_timer(now_ms)`; эффекты: `SendHeartbeat{d}` → `ws.send(Binary(encode_heartbeat(d)))`;
`Reconnect{reason}` → вернуть `RunOutcome::Reconnect`. Входящее: Binary → `push_binary` + его эффекты;
Ping → Pong; Close → `RunOutcome::Closed{code}`; Text → игнор со счётчиком (задокументировать).

**Стаб-сервер в тестах**: `tokio::net::TcpListener` на `127.0.0.1:0` + `tokio_tungstenite::accept_async`;
**непрерывный zstd-поток без завершения кадра**: `zstd::stream::write::Encoder` поверх `Vec<u8>` с
маленьким окном (`CParameter::WindowLog(10)` ≈ 1 КиБ; если параметр недоступен — подбери окно иначе
и опиши в отчёте), после каждого терма — `flush()`, новые байты `Vec` отправить как Binary;
`finish()` **не вызывать**. После каждого терма дописывается «филлер» — валидный терм-строка
(например, `0x83` + ETF `Str` ~1536 Б), чтобы удержание окна ruzstd (RW-02: пока кадр не завершён,
выдаётся `распаковано − окно`) не задерживало полезные термы. Термы кодировать через
`discord_core::codec::etf`.

**Кейсы (red-first; имена латиницей; комментарии по-русски)**:
Юнит (без сокетов; zstd-байты строишь энкодером):
1. `pipe_hello_from_encoded_stream` — поток HELLO+filler → `hello() == 41250`, эффектов нет,
   счётчики кадров/байт/термов растут.
2. `pipe_buffers_partial_term` — терм разрезан между двумя push → диспатч распознан ровно один раз
   (`last_seq`).
3. `pipe_missing_magic_is_error` — первый push без магии → `TransportError` (Zstd/MissingMagic).
4. `heartbeat_encode_roundtrip` — `encode_heartbeat(Some(7))` и `None` → `etf::decode` → op=1, d;
   длина терма ≤ 64 Б.

Стаб (реальные сокеты `127.0.0.1`; всё под `tokio::time::timeout(5 с)`):
5. `ws_hello_and_heartbeats_acked` — интервал 60 мс: стаб принимает ≥2 heartbeat (op=1), шлёт ACK;
   `rtt_ms()` измерен; после N ударов стаб закрывает соединение → `RunOutcome::Closed`.
6. `ws_zombie_without_ack` — стаб не отвечает ACK → `RunOutcome::Reconnect{Zombie}` ≤ 5 с.
7. `ws_server_reconnect_op7` — стаб шлёт op7 → `RunOutcome::Reconnect{ServerRequest}`.
8. `ws_dispatch_updates_heartbeat_seq` — стаб шлёт диспатчи s=1..3, декодирует пришедший heartbeat →
   `d == Some(3)`.
9. `ws_closed_after_stub_close` — стаб закрывает чисто → `Closed` (код из close-кадра либо `None`).

Lab (skip-aware):
10. `lab_capture_stream_feeds_pipe` — `{RE_LAB_ROOT:-/home/sanya/re-deep}/gw-proto/zstd-frames-95974_1389.hexl`
    (137 кадров, hexl без префикса) → без ошибок; `hello() == 41250`, `last_seq() == Some(113)`;
    помнить про удержание окна (выдано 759 280 Б; HELLO — в первой порции).

**Порядок работы (hard TDD)**:
1. Сначала тесты (10) и заглушка `ws.rs` (`todo!()`), затем red-прогон:
   `cd rust && cargo test -p discord-core 2>&1 | tee /tmp/opencode/rw-04/red.txt`
   (первая сборка — с сетью, разрешено владельцем; далее можно `--offline`).
2. Минимальная реализация до green: `cargo test --offline -p discord-core`, затем весь воркспейс.
3. `cargo clippy --offline -p discord-core --all-targets -- -D warnings`; `cargo fmt -p discord-core`
   (в конце — `--check`).
4. `npm run gate:worker` (из корня) — passed.
5. Отчёт §4.2 → `/tmp/opencode/rw-04/report.md`: red/green с командами и числами, gate-артефакт,
   файлы + md5, точные версии крейтов из `Cargo.lock`, риски (флаки таймингов, размер сборки).
   Верни текст отчёта.

**Запрещено**: подключения к не-localhost адресам, golden (`logs/`, `functions/`, `evidence/`),
`docs/`, `TASKS.md`, чужие файлы, коммиты, паники/`unwrap` на входных данных.
