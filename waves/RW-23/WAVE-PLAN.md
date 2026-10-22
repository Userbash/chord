# RW-23 / B04 — план волны

Обновлено: 2026-10-21. Статус: контракты заморожены до кода.

## 1. Состав

| Карточка | Что закрываем | Тесты | Файлы (владелец) |
|---|---|---|---|
| P4-02 | UDP-транспорт: discovery 74 Б, keepalive 8 Б/5 с, ротация +60,08 с, пустой RR | T-VOICE-04…06 | `rust/crates/discord-voice/src/udp.rs`, `src/lib.rs` (только `mod`/re-export), `tests/udp.rs` (W24a) |
| P8-04 | Воспроизводимость/лицензии: offline SBOM SPDX 2.3, allowlist лицензий, CI-хук | T-HARN-07 | `scripts/sbom.mjs`, `scripts/ci.sh`, `scripts/release.sh`, `rust/crates/discord-conformance/tests/sbom_contract.rs` (W24b) |
| foundation | `discord-json`/i18n/voice P4-01 уже приняты; новых общих зависимостей нет | — | — |

P7-01 (полный i18n) — следующий batch, не входит в B04: он не блокирует P4-02/P8-04 и
требует отдельного большого контракта по 36 локалям/датам/markdown.

## 2. Файловая матрица (пересечения запрещены)

- **W24a (voice UDP):** `rust/crates/discord-voice/src/udp.rs`, `rust/crates/discord-voice/tests/udp.rs`;
  `src/lib.rs` меняет только C (mod/reexport).
- **W24b (SBOM/licenses):** `scripts/sbom.mjs`, `scripts/ci.sh`, `scripts/release.sh`,
  `rust/crates/discord-conformance/tests/sbom_contract.rs`.
- **C / integration lane:** `rust/crates/discord-voice/src/lib.rs` (только `pub mod udp;` и
  re-export), `waves/RW-23/**`, доки после accepted.
- Общие `rust/Cargo.toml`, `Cargo.lock`, `discord-core`, `discord-i18n` не трогаются.

## 3. Контракт P4-02

- `DiscoveryPacket` — ровно 74 Б: `type: u16 BE` (1 request / 2 response), `len: u16 BE = 0x46`,
  `ssrc: u32 BE`, 64 Б ASCII address NUL-padded, `port: u16 BE` на offset 72.
  В request port — локальный UDP-порт (0 или фактический, capture: 0 и 33801); в response —
  внешний порт 1..=65535.
- `Request { ssrc, port }`: type 1, len 0x46, ssrc, 64 нуля, port. `Response { ssrc, address, port }`:
  type 2, ASCII address (валидный IPv4/IPv6-хост без пробелов/control/`\0` внутри), порт
  1..=65535.
- `parse` отклоняет длину != 74, type не 1/2, len != 0x46, не-ASCII/не-NUL-padding address,
  port 0 для response, хвост после NUL не нулевой; request port 0 допустим; ошибки без
  сырых секретов.
- `Keepalive` — 8 Б: client `13 37 ca fe 01 00 00 00`, server `13 37 f0 0d 01 00 00 00`;
  `KEEPALIVE_INTERVAL_MS = 5_000`.
- `RotationSchedule` — due на `>= 60_080` мс от старта; `UdpMediaSocket::rotate_if_due`
  связывает расписание с сокетом: новый сокет, discovery с фактическим local port, затем
  RTCP/SRTCP RR; старый сокет закрыт; повторная ротация не раньше следующего окна.
- `empty_receiver_report(ssrc)` — 8 Б RTCP: `0x80 0xC9 00 01` + ssrc BE. Это чистый пустой
  RR; 28-байтный пакет из capture — SRTCP-обёртка (RR + 16 Б tag + 4 Б LE counter), поэтому
  отдельная `empty_srtcp_receiver_report(ssrc, tag, counter) -> [u8;28]`.
- `rotate()` отправляет 8-байтный незашифрованный RR (P4-02); `rotate_srtcp(tag, counter)`
  отправляет ровно 28-байтный capture-пакет; SRTCP-крипто — P4-03.
- `UdpMediaSocket`: `bind("0.0.0.0:0")`, read/write timeout, `send_discovery`,
  `recv_discovery` (peer + SSRC), `send_keepalive`/`recv_keepalive` (peer), `send_empty_rr`,
  `send_srtcp_empty_rr`, `local_addr`, `rotate`, `rotate_srtcp`, `rotate_if_due`.
  `encode` возвращает `Result` и не обрезает невалидный Response. Никаких DNS/connect к
  Discord; только loopback в тестах.
- Никаких `unsafe`, паник на внешних данных, новых зависимостей.

## 4. Контракт P8-04

- `scripts/sbom.mjs` — offline: `cargo metadata --offline --filter-platform <host>` для
  лицензий/связей, `Cargo.lock` — авторитетный полный набор пакетов (85, включая
  target-specific); для 14 платформенных пакетов без локального манифеста — точный
  version-pinned license fallback; SPDX 2.3 JSON, deterministic namespace/hash; пакеты
  workspace и все locked deps; `licenseDeclared`; `--check` проверяет allowlist (MIT,
  Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0, Unlicense, 0BSD,
  CDLA-Permissive-2.0; `LLVM-exception` только как exception в `WITH`); OR-выражение
  проходит, если есть разрешённая альтернатива; неизвестная лицензия/невалидный токен —
  exit 1. `--check-license` не принимает `LLVM-exception` как самостоятельную лицензию.
- `scripts/ci.sh` вызывает SBOM после deny и до release; `--dry-run` печатает шаг; сеть не
  используется. `scripts/release.sh` кладёт SBOM рядом с бинарником, если он есть; same-file
  сравнение через `-ef`, чтобы `rust/dist` и `rust/./dist` не ломали install.
- `tests/sbom_contract.rs`: SPDX валиден, число пакетов равно `Cargo.lock`, licenseDeclared
  у всех, DESCRIBES/DEPENDS_ON ссылаются на существующие SPDXID, две генерации
  byte-for-byte равны, `--check` проходит, запрещённая/невалидная лицензия отклоняется,
  ci dry-run и actual hook проверяются, сетевых клиентов нет.
- `cargo deny` остаётся optional: если инструмента нет — явный skip, но собственный
  license-check обязателен и не маскируется.

## 5. Red-first и приёмка

1. W24a/W24b пишут тесты первыми, red-логи с exit 101.
2. Targeted: `cargo test --offline -p discord-voice --test udp`, `cargo test --offline -p discord-conformance --test sbom_contract`.
3. Мутации: минимум 6 P4-02 (offset/len/type/rotation/keepalive/RR) + 4 P8-04 (allowlist/
   SPDX field/lock parse/ci hook); review-фиксы дополнительно мутируются (encode, peer,
   rotate_if_due, tokenizer, fallback, LLVM exception).
4. Один batch gate; независимое review Codex CLI; P0/P1/P2 — red-first; P3 — backlog.
5. После accepted — journal/STATE/STATUS/ROADMAP/TASKS/map один раз; коммит — координатор.
