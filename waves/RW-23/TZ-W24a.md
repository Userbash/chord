# ТЗ W24a — P4-02: UDP media transport

Волна: RW-23 / B04. Владелец: W24a. Работать в `/home/user/orca/project`, не коммитить.

## 0. Обязательное чтение

`AGENTS.md`, `PROCESS.md` §4, `waves/RW-23/WAVE-PLAN.md`, `docs/re/voice-deep.md` §2.1–2.5,
`docs/rust-client/PLAN.md` (P4-02), `docs/rust-client/TEST-PLAN.md` (T-VOICE-04…06),
`rust/crates/discord-voice/src/lib.rs`, `reference-oracle/voice/strace-media-s3.jsonl`,
`rtc1-frames.jsonl`, `rtc2-frames.jsonl`.

Твои файлы — только `rust/crates/discord-voice/src/udp.rs` и
`rust/crates/discord-voice/tests/udp.rs`. `src/lib.rs` не трогай: C добавит `pub mod udp;`.
Новых зависимостей нет; только std.

## 1. Метод

Strict TDD. Сначала тесты, red-лог:

```bash
cd /home/user/orca/project/rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-voice --test udp \
  2>&1 | tee ../waves/RW-23/red-w24a.log
```

Red — compile-red/behavior-red, exit 101. Затем минимальная реализация до green,
`green-w24a.log`. Тесты не подгонять.

## 2. Контракт

Реализуй ровно контракт `WAVE-PLAN.md` §3. Публичные имена:

```rust
pub const DISCOVERY_LEN: usize = 74;
pub const KEEPALIVE_LEN: usize = 8;
pub const KEEPALIVE_INTERVAL_MS: u64 = 5_000;
pub const MEDIA_ROTATION_MS: u64 = 60_080;
pub const KEEPALIVE_REQUEST: [u8; 8];
pub const KEEPALIVE_RESPONSE: [u8; 8];

pub enum DiscoveryPacket { Request { ssrc: u32, port: u16 }, Response { ssrc: u32, address: String, port: u16 } }
pub enum UdpError { InvalidLength, InvalidType, InvalidLengthField, InvalidAddress, InvalidPort, UnexpectedPeer, UnexpectedSsrc, Io(std::io::Error), Timeout, Closed }
pub struct RotationSchedule { /* private */ }
pub struct UdpMediaSocket { /* private */ }
pub fn empty_receiver_report(ssrc: u32) -> [u8; 8]; // чистый RTCP RR
pub fn empty_srtcp_receiver_report(ssrc: u32, tag: [u8; 16], counter: u32) -> [u8; 28];
```

- `DiscoveryPacket::parse(&[u8]) -> Result<Self, UdpError>`; `encode(&self) -> Result<[u8; 74], UdpError>` с валидацией Response.
- `RotationSchedule::new(started_at_ms: u64)`, `due(now_ms) -> bool`,
  `action(ssrc) -> RotationAction { discovery: [u8;74], empty_rr: [u8;8] }`;
  `rotate_srtcp(tag, counter)` отправляет 28-байтный SRTCP RR; `rotate_if_due(now, schedule)`
  связывает расписание с сокетом.
  Сокет реально перебиндивается в `UdpMediaSocket::rotate`, который возвращает новый
  `local_addr` и отправляет discovery + пустой RR на сохранённый сервер; `encode` возвращает
  `Result` и валидирует Response; `recv_*` проверяют peer и SSRC.
- `UdpMediaSocket::bind(addr: &str) -> Result<Self, UdpError>`; `local_addr`, `send_to`, `recv_from`, `send_discovery(server, ssrc)`, `recv_discovery`, `send_keepalive(server)`, `recv_keepalive`, `send_empty_rr(server, ssrc)`, `rotate() -> Result<RotationAction, UdpError>`, `rotate_srtcp(tag, counter) -> Result<RotationAction, UdpError>`, `rotate_if_due(now, schedule)`, `set_read_timeout`.
- Таймауты — `Duration`; `recv_*` возвращают `UdpError::Timeout` на WouldBlock/TimedOut.
- `rotate` закрывает старый сокет (drop), биндит новый на `0.0.0.0:0`, сохраняет ssrc/сервер,
  отправляет discovery + пустой RR. Не отправляй ничего в реальный Discord.
- `Debug` сокета не печатает адрес/секреты (только `local_addr` можно).

## 3. Обязательные тесты (минимум)

1. RE oracle: первые 3 пакета `strace-media-s3.jsonl` — request 74 (type1/len/ssrc; port 0
   в первом и 33801 во втором), response 74 (address/port BE), keepalive 8.
2. Request encode байт-в-байт равен capture (`00010046000023f8...` для port 0 и
   `...8409` для port 33801), response round-trip.
3. Отклонения: len 73/75, type 3, len field 0x45, address с control/не-ASCII/мусор после NUL,
   response port 0.
4. Keepalive: константы, 8 Б, interval 5 000.
5. Rotation: due 60 079=false, 60 080=true; action содержит discovery с тем же ssrc и RR 8;
   `rotate_if_due` выполняет ротацию; `rotate_srtcp` даёт capture-пакет 28;
   `UdpMediaSocket::rotate` меняет local port (loopback), старый сокет закрыт.
6. Loopback: UDP echo/server thread на 127.0.0.1:0, send/recv discovery, keepalive, timeout.
7. Empty RR: 8 Б `0x80 0xC9 00 01` + ssrc BE; SRTCP RR 28 Б = RR + 16 tag + 4 LE counter.
8. Panic-free: все префиксы 74-байтного пакета и случайные 0..128 Б.

## 4. Отчёт

Red/green команды+exit, файлы, тесты, мутации, риски. Не запускай workspace.
