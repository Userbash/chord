# Отчёт B04 / RW-23 — P4-02 + P8-04

Обновлено: 2026-10-21. Статус: **принят 2026-10-21**; gate passed, targeted/workspace зелёные, review-фиксы закрыты.

## 1. Состав и результат

| Карточка | Результат | Тесты |
|---|---|---|
| P4-02 UDP media transport | discovery 74 Б, keepalive 8 Б/5 с с LE-счётчиком, ротация +60,08 с, RTCP/SRTCP RR, loopback-сокет | `tests/udp.rs` — **15/15** |
| P8-04 reproducible SBOM/licenses | SPDX 2.3 на **85** locked-пакетов, allowlist + строгий SPDX-парсер, CI-хук, SBOM в release checksum | `tests/sbom_contract.rs` — **5/5** |
| Регрессия | B03 не сломана | workspace **638 passed / 0 failed**; core **530** |

## 2. P4-02: что подтверждено RE

- `strace-media-s3.jsonl`: request/response — 74 Б, `type=1/2`, `len=0x0046`, `ssrc=9208`;
  address 64 Б NUL-padded; port BE @72. Второй request содержит **port=33801 (0x8409)**,
  поэтому `DiscoveryPacket::Request { ssrc, port }`; первый request с port 0 также
  закреплён oracle-тестом.
- Keepalive: `1337cafe` / `1337f00d`, 8 Б, интервал 5 с; sequence — **little-endian**
  (`01 00 00 00`), а не BE. `send_keepalive` инкрементирует счётчик, `recv_keepalive`
  проверяет magic и peer.
- Ротация: `MEDIA_ROTATION_MS=60_080`; `rotate_if_due(now, schedule)` связывает
  `RotationSchedule::due` с сокетом; `rotate()`/`rotate_srtcp()` перебиндивают сокет,
  отправляют discovery с фактическим local port, затем RR; старый сокет закрыт.
- **RR:** чистый RTCP empty RR — 8 Б `80 c9 00 01` + ssrc BE. Capture-пакет 28 Б — это
  SRTCP-обёртка: RR 8 Б + 16 Б tag + 4 Б LE counter. `empty_srtcp_receiver_report` и
  `rotate_srtcp(tag, counter)` воспроизводят ровно эти 28 Б; `rotate()` отправляет
  незашифрованный 8-байтный RR, SRTCP-крипто — P4-03.
- `encode()` возвращает `Result`, валидирует Response (IP, длина <64, port 1..=65535) и не
  обрезает адрес. `recv_discovery` проверяет peer и SSRC; `recv_keepalive` — peer;
  oversized datagrams отклоняются.

## 3. P8-04: SBOM и лицензии

- `scripts/sbom.mjs` offline: `cargo metadata --offline --filter-platform <host>` для
  лицензий/связей, `Cargo.lock` — авторитетный полный набор **85** пакетов (включая
  target-specific). Для 14 платформенных пакетов без локального манифеста используется
  точный version-pinned fallback; иначе `NOASSERTION` и `--check` fail.
- Namespace — sha256 `Cargo.lock`; `SOURCE_DATE_EPOCH` (default 0); две генерации
  byte-for-byte равны. Relationships: 11 `DESCRIBES`, 127 `DEPENDS_ON`, все ссылки на
  существующие SPDXID.
- Allowlist: MIT, Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0, Unlicense,
  0BSD, CDLA-Permissive-2.0. `LLVM-exception` разрешён только как exception в `WITH`;
  невалидные токены, хвостовые операторы и `NOASSERTION` отклоняются.
- `scripts/ci.sh` после deny вызывает `sbom.mjs --check`; `scripts/release.sh` включает
  `sbom.spdx.json` в `SHA256SUMS`; same-file сравнение через `-ef`, поэтому `rust/dist` и
  `rust/./dist` не ломают install. `cargo-deny` остаётся optional, обязательный
  license-check не маскируется.

## 4. Red / green / review

| Что | Лог | Результат |
|---|---|---|
| W24a red | `red-w24a.log` | exit 101 |
| W24a green | `green-w24a.log` | 8/8, exit 0 |
| W24a fix red/green | `red-w24a-fix.log`, `targeted-udp-final.log` | 10/10, exit 0 |
| Review A red/green | `red-review-a-p2.log`, `green-review-a-p2.log` | 15/15, exit 0 |
| W24b red | `red-w24b.log` | exit 101 |
| W24b fix red/green | `red-w24b-fix.log`, `targeted-sbom-final.log` | 5/5, exit 0 |
| Review B red/green | `red-review-b-p1p2.log`, `green-review-b-p1p2.log` | 5/5, exit 0 |
| fmt / clippy | `clippy-b04-review.log` | fmt чисто, clippy `-D warnings` 0 |
| workspace | `test-workspace-b04-review.log` | 638 passed / 0 failed |
| CI actual | `ci-actual-b04-review.log` | `ci: OK`, bench 2 755 116 ops/s |
| release | `rust/dist/` | `discord-rs` + `sbom.spdx.json` + `SHA256SUMS` |
| gate | `waves/RW-23/gate-worker-b04.log`; final `gate-worker-b04-final.log` / `.gate/worker-2026-10-21T08-08-46-482Z.json` | **passed** |

## 5. Независимое ревью

- **Review A (P4-02):** P0/P1/P2 = 0/0/4; все 4 P2 закрыты red-first:
  RR 8/28, encode Result, peer/SSRC validation, `rotate_if_due`. P3 skip-aware loopback
  остаётся в backlog (в текущем окружении loopback прошёл).
- **Review B (P8-04):** P0/P1/P2 = 0/2/3. P1 «SBOM без locked deps» был частично верен:
  host-only 71 → теперь все 85 locked-пакетов. P1 «невалидные символы» закрыт
  токенизатором с проверкой пробелов. P2 allowlist/LLVM, release same-file и тестовые
  дыры закрыты; CDLA внесён в контракт, LLVM — только exception.
- Авторы свой код не ревьюили; review-агенты только читали.

## 6. Мутации

Поймано **21**, inconclusive **1** (не засчитана). Подробности — `MUTATIONS-B04.md`.
Ключевые review-мутации: encode-validation, peer-check, rotate_if_due, RR-header,
invalid-token, license-fallback, LLVM-standalone — все caught (exit 101).

## 7. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-voice/src/udp.rs` | `6b9282be2b67d82ff6adca42cf90cd5e` |
| `rust/crates/discord-voice/tests/udp.rs` | `a4c0a20ad9bc4478f9f7d01725a77c18` |
| `scripts/sbom.mjs` | `2f4a3ffe29a00cd3bf6e37d32632abc9` |
| `scripts/ci.sh` | `fb5ad676b5184d0f2b374e520f590e98` |
| `scripts/release.sh` | `29b5d5bb67e3d61956c0422c9181fe0d` |
| `rust/crates/discord-conformance/tests/sbom_contract.rs` | `51a5e76619ae17d92bfa0e735b81d955` |

Артефакты: `discord-rs` sha256 `8816a414...484c`, `sbom.spdx.json` sha256 `cd9235ee...65a3`.

## 8. Риски / P3

- License fallback для 14 platform-пакетов — version-pinned и проверяется allowlist, но
  является офлайн-допущением; при обновлении lock его нужно ревьюить.
- `cargo-deny` не установлен; CI печатает skip, обязательный license-check выполняется.
- Loopback-тесты skip-aware при `PermissionDenied` и отмечаются в stderr; в текущем
  окружении прошли. Это P3 backlog (нет нативного skip в stable Rust).
- Нет live Discord, DNS, TLS, SRTP/DAVE: P4-02 — только UDP-примитивы и loopback;
  SRTCP-обёртка принимает tag/counter, крипто — P4-03.
