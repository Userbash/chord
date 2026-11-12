# Отчёт: task-175 (BE-P3-05 & BE-P3-06) — VoiceOp extended opcodes 5/7/9/11..13/15..16/21..32 + DAVE/MLS и Discovery Request trailing 2 bytes port layout

- **Карточка:** `task-175` (`BE-P3-05` & `BE-P3-06`, гэпы `F-VC-06` и `F-VC-08`)
- **Волна:** `RW-40`
- **Роль:** Воркер G5 Platform / Voice
- **Оракул:**
  - `RE/voice/strace-media-s3.jsonl` (строки 1 и 2: 74-байтные запросы Discovery Request, последние 2 байта port BE: `00 00` для базового запроса и `84 09` для запроса с локальным портом `33801`);
  - `RE/voice-deep.json` (`signaling.ops`, `unknowns`: opcodes 0..32, протокол DAVE/MLS opcodes 21..31);
  - `waves/RW-40/PM/BACKLOG-RE-BACKEND.md` (§BE-P3-05 и §BE-P3-06);
  - `waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` (§6.7 F-VC-06 и F-VC-08).

---

## 1. Что сделано (Strict TDD)

1. **Расширение `VoiceOp` в `rust/crates/discord-voice/src/lib.rs` (BE-P3-06)**:
   - В перечисление `VoiceOp` добавлены все специфицированные варианты:
     - `Speaking (5)`
     - `Resume (7)`
     - `Resumed (9)`
     - `ClientsConnect (11)`
     - `Video (12)`
     - `ClientDisconnect (13)`
     - `MediaSinkWants (15)`
     - `VoiceBackendVersion (16)`
     - `SecureFramesPrepareProtocolTransition (21)`
     - `SecureFramesExecuteTransition (22)`
     - `SecureFramesReadyForTransition (23)`
     - `SecureFramesPrepareEpoch (24)`
     - `MlsExternalSenderPackage (25)`
     - `MlsKeyPackage (26)`
     - `MlsProposals (27)`
     - `MlsCommitWelcome (28)`
     - `MlsPrepareCommitTransition (29)`
     - `MlsWelcome (30)`
     - `MlsInvalidCommitWelcome (31)`
     - `NoRoute (32)`
     - Нераспознанные опкоды сохраняются в `Unknown(u8)` без потери байта.
   - Реализованы методы:
     - `VoiceOp::is_dave_or_mls(self) -> bool` — предикат принадлежности к DAVE / MLS протоколу (возвращает `true` строго для опкодов `21..=31`, `false` для остальных);
     - `VoiceOp::name(self) -> &'static str` и `as_str(self) -> &'static str` — возвращает каноническое имя опкода в SCREAMING_SNAKE_CASE (например, `"SPEAKING"`, `"MEDIA_SINK_WANTS"`, `"SECURE_FRAMES_PREPARE_PROTOCOL_TRANSITION"`, `"MLS_KEY_PACKAGE"`, `"UNKNOWN"` для `Unknown(_)`);
     - Реализован типаж `fmt::Display` для `VoiceOp`.
   - Проверены все существующие сопоставления (`match`) по `VoiceOp`: `parse_frame` использует ветку `other => VoicePayload::Other { op, raw }`, поэтому добавление вариантов полностью обратно совместимо.

2. **Запись локального порта в последние 2 байта Discovery Request в `rust/crates/discord-voice/src/udp.rs` (BE-P3-05)**:
   - Добавлен метод `DiscoveryPacket::encode_request_with_port(ssrc: u32, address: &str, port: u16) -> Result<[u8; DISCOVERY_LEN], UdpError>`:
     - Кодирует 74 байта: `type = 1` (Request), `length = 0x0046` (70), `ssrc` BE в байтах `4..8`, пустые 64 байта адреса `8..72` (или валидный ASCII IP), и 2 байта big-endian port в `bytes[72..74]` — байт-в-байт соответствует дампу `strace-media-s3.jsonl` (line 2: trailing `84 09` для port `33801`).
   - Добавлен метод `DiscoveryPacket::encode_request(ssrc: u32) -> Result<[u8; DISCOVERY_LEN], UdpError>` (port 0, байт-в-байт с `strace-media-s3.jsonl` line 1: trailing `00 00`) с сохранением 100% обратной совместимости.
   - Добавлены модульные функции `encode_request` и `encode_request_with_port`.
   - В `UdpMediaSocket` добавлен метод `send_discovery_with_local_port(&mut self, server: SocketAddr, ssrc: u32) -> Result<(), UdpError>`, отправляющий 74-байтный discovery-запрос с локальным портом сокета в байтах `72..74`.
   - Метод `send_discovery` перенаправлен на `send_discovery_with_local_port`.

3. **Контрактные тесты в `rust/crates/discord-voice/tests/signaling.rs`**:
   - `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` — сверка полного диапазона `0..=32` + `Unknown` по таблице: `VoiceOp::from_u8(code)`, `op.as_u8() == code`, `op.name() == expected_name`, сохранение сырых байтов для нераспознанных опкодов (`33, 77, 99, 128, 200, 255`).
   - `test_voice_op_is_dave_or_mls_predicate` — проверка предиката `is_dave_or_mls()` для всех 256 возможных байтов (строго `21..=31` возвращают `true`).
   - `test_discovery_request_trailing_2_bytes_port_layout` — побайтовая сверка раскладки 74 Б с гекс-оракулами из `strace-media-s3.jsonl`: строка 1 (порт 0) и строка 2 (порт 33801 = `0x8409`), roundtrip-парсинг через `DiscoveryPacket::parse`, проверка отклонения некорректного адреса.
   - `test_udp_media_socket_send_discovery_with_local_port` — loopback UDP интеграционный тест: сокет клиента отправляет discovery, сервер принимает ровно 74 байта с совпадающим локальным портом в байтах `72..74`.

---

## 2. Red-прогон

Лог зафиксирован в `/home/user/orca/project/waves/RW-40/G5/task-175-voice-ops-dave-discovery/red.log`.
Тесты в `signaling.rs` падали на этапе компиляции из-за отсутствия новых вариантов `VoiceOp`, методов `name()`, `is_dave_or_mls()`, `DiscoveryPacket::encode_request_with_port` и `UdpMediaSocket::send_discovery_with_local_port`:
```text
error[E0599]: no variant, associated function, or constant named `Speaking` found for enum `VoiceOp`
error[E0599]: no method named `name` found for enum `VoiceOp`
error[E0599]: no method named `is_dave_or_mls` found for enum `VoiceOp`
error[E0599]: no variant, associated function, or constant named `encode_request_with_port` found for enum `DiscoveryPacket`
error[E0599]: no method named `send_discovery_with_local_port` found for struct `UdpMediaSocket`
```

---

## 3. Green-прогон

Лог зафиксирован в `/home/user/orca/project/waves/RW-40/G5/task-175-voice-ops-dave-discovery/green.log`.
Команда прогона:
```bash
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-voice
```
Результаты:
- `unittests src/lib.rs`: 14 passed
- `tests/crypto_vectors.rs`: 16 passed
- `tests/devices_contract.rs`: 5 passed
- `tests/media_contract.rs`: 16 passed
- `tests/reception_contract.rs`: 10 passed
- `tests/reconnect_contract.rs`: 6 passed
- `tests/regions_contract.rs`: 16 passed
- `tests/signaling.rs`: 18 passed (включая 4 новых контрактных теста)
- `tests/srtp_contract.rs`: 22 passed
- `tests/stats_contract.rs`: 20 passed
- `tests/udp.rs`: 15 passed
- **Всего:** 158 passed, 0 failed, 0 ignored. 100% обратная совместимость со всеми 154 существовавшими тестами.

---

## 4. Проверка качества (clippy & fmt)

- `cargo fmt --manifest-path rust/Cargo.toml -p discord-voice -- --check` — чисто (0 diff).
- `cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --tests -- -D warnings` — чисто (0 warnings, лог в `clippy.log`).

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/src/lib.rs` | modified | `c45a41ef0b569cabfa3596a04659b82e` |
| `rust/crates/discord-voice/src/udp.rs` | modified | `1623b890f2af31886cd7be182d243775` |
| `rust/crates/discord-voice/tests/signaling.rs` | modified | `70cacf9be751349290a5c2f3da888cbb` |
| `waves/RW-40/G5/task-175-voice-ops-dave-discovery/red.log` | created | `37040d84f97cadd7ced76b336ee3fdf8` |
| `waves/RW-40/G5/task-175-voice-ops-dave-discovery/green.log` | created | `b41f6ddae2e2c6914ecd2f0aeba67d9c` |
| `waves/RW-40/G5/task-175-voice-ops-dave-discovery/clippy.log` | created | `8e9b76d54c22d958e63a58356f6295c8` |

---

## 6. Мутации (5/5 убиты)

1. **Неверный опкод для `Speaking` (например, 4 вместо 5)**:
   - Убивается тестом `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` (`assert_eq!(VoiceOp::from_u8(5), VoiceOp::Speaking)` и `assert_eq!(VoiceOp::Speaking.as_u8(), 5)`).
2. **Диапазон `is_dave_or_mls` включает опкод 32 (`NoRoute`) или исключает опкод 21**:
   - Убивается тестом `test_voice_op_is_dave_or_mls_predicate` (проверка границ `20 -> false`, `21 -> true`, `31 -> true`, `32 -> false`).
3. **Имя опкода `op.name()` не совпадает с каноническим (например, lowercase или пустое)**:
   - Убивается тестом `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` (проверка 33 вариантов на точное соответствие SCREAMING_SNAKE_CASE).
4. **Смещение порта в discovery request не в байтах `72..74` (например, little-endian или другое смещение)**:
   - Убивается тестом `test_discovery_request_trailing_2_bytes_port_layout` (`assert_eq!(&req_with_port[72..74], &port.to_be_bytes())` и побайтовым сравнением с оракулом `strace-media-s3.jsonl`).
5. **Потеря сырого байта при неизвестном опкоде в `VoiceOp::Unknown`**:
   - Убивается тестом `test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown` (`VoiceOp::from_u8(raw) == VoiceOp::Unknown(raw)` для `raw in [33, 77, 99, 128, 200, 255]`).

---

## 7. Риски

- Изменения локализованы строго в `discord-voice`, не затрагивают сетевые/внешние зависимости и сохраняют полную обратную совместимость со всеми 154 существующими тестами.
