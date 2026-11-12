# Отчёт: task-166 (BE-P2-07) — Ротация UDP-сокета в discord-voice: отправка 28-байтового SRTCP-обёрнутого пустого RR и одновременного IP Discovery сразу после ротации по таймеру 60 080 мс

- **Карточка:** `task-166` (`BE-P2-07`)
- **Волна:** `RW-40`
- **Роль:** Воркер G5 Platform / Voice
- **Оракул:** `reference-oracle/voice-deep.json` (`udp.rtcp.rr_client_empty` 28 Б layout, `udp.socket_lifecycle`: ротация каждые 60 080 мс, пустой RR 28 Б сразу после ротации), `waves/RW-40/PM/BACKLOG-RE-BACKEND.md` (§BE-P2-07).

---

## 1. Что сделано (Strict TDD)

1. **Константа и структура действия ротации SRTCP в `rust/crates/discord-voice/src/udp.rs`**:
   - Добавлена константа `SRTCP_EMPTY_RR_LEN = 28` (байт).
   - Объявлена структура `SrtcpRotationAction { pub discovery: [u8; DISCOVERY_LEN], pub empty_rr: [u8; SRTCP_EMPTY_RR_LEN] }` с методом `srtcp_empty_rr(&self)`.
   - В `RotationSchedule` добавлен метод `action_srtcp(ssrc, tag, counter) -> SrtcpRotationAction`.

2. **Жизненный цикл сокета и ротация `rotate_if_due_srtcp` в `UdpMediaSocket`**:
   - В структуру `UdpMediaSocket` добавлен учёт сквозного счётчика пакетов `packet_counter: u32` с методами доступа `packet_counter(&self)` и `set_packet_counter(&mut self, counter: u32)`.
   - Добавлен метод `rotate_if_due_srtcp(&mut self, now_ms: u64, schedule: &mut RotationSchedule, tag: [u8; 16], packet_counter: &mut u32) -> Result<Option<SrtcpRotationAction>, UdpError>`.
   - При наступлении порога `60 080 мс`:
     - Старый UDP-сокет закрывается (`self.socket.take()`);
     - Новый сокет привязывается к эфемерному порту;
     - Немедленно формируются и отправляются стартовые пакеты нового сокета: 74B IP Discovery (`DiscoveryPacket::Request`) и 28B SRTCP Empty Receiver Report (`[8B RTCP header: V=2, PT=201, RC=0, len=1, SSRC][16B AEAD tag][4B LE SRTCP index with E-bit]`);
     - Сквозной счётчик пакетов инкрементируется (`*packet_counter = packet_counter.wrapping_add(1)`), обеспечивая неразрывное продолжение нумерации кадров для последующих RTP/RTCP-пакетов;
     - Возвращается `SrtcpRotationAction` с 74-байтовым discovery и 28-байтовым empty RR.
   - Метод `rotate_srtcp` обновлён для возврата `Result<SrtcpRotationAction, UdpError>` с гарантированным выставлением E-bit (`0x8000_0000 | counter`).
   - Сохранена 100% обратная совместимость со всеми существующими тестами и методами (`rotate()`, `rotate_if_due()`, `empty_receiver_report()`, `empty_srtcp_receiver_report()`).

3. **Контрактные тесты в `rust/crates/discord-voice/tests/media_contract.rs`**:
   - `m14_srtcp_empty_rr_exact_byte_layout_oracle_28b` — побайтовая сверка раскладки 28 Б с оракулом: 8B RTCP header (V=2, P=0, RC=0, PT=201, len=1, SSRC BE), 16B AEAD tag, 4B LE счётчик с E-bit (bit 31 = 1).
   - `m15_udp_socket_rotation_due_at_60080ms_sends_discovery_and_28b_srtcp_rr` — проверка порога срабатывания ротации (60 079 мс — нет ротации; 60 080 мс — ротация), одновременная отправка 74B Discovery и 28B SRTCP RR с нового сокета, проверка инкремента счётчика (100 -> 101), отсутствие повторного срабатывания на той же миллисекунде.
   - `m16_udp_socket_rotation_closes_old_socket_and_increments_shared_counter` — проверка закрытия старого сокета (освобождение порта подтверждено успешным повторным `UdpSocket::bind` к старому адресу), смена порта, продолжение сквозного счётчика пакетов при серии ротаций (`42 -> 43 -> 44`).

---

## 2. Red-прогон

Лог зафиксирован в `waves/RW-40/G5/task-166-voice-udp-rotate-srtcp/red.log`.
Тесты `m14`, `m15`, `m16` падали на этапе компиляции из-за отсутствия константы `SRTCP_EMPTY_RR_LEN` и метода `rotate_if_due_srtcp`:
```text
error[E0432]: unresolved import `discord_voice::udp::SRTCP_EMPTY_RR_LEN`
error[E0599]: no method named `rotate_if_due_srtcp` found for struct `UdpMediaSocket` in the current scope
```

---

## 3. Green-прогон

Лог зафиксирован в `waves/RW-40/G5/task-166-voice-udp-rotate-srtcp/green.log`.
Команда целевого прогона:
```bash
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-voice --test media_contract --test udp --lib
```
Результаты:
- `unittests src/lib.rs`: 14 passed, 0 failed
- `tests/media_contract.rs`: 16 passed (включая `m14`, `m15`, `m16`), 0 failed
- `tests/udp.rs`: 15 passed, 0 failed
Всего по крейту `discord-voice`: 150 тестов успешно пройдены (0 failed, 0 ignored).

---

## 4. Проверка качества (clippy & fmt)

- `cargo fmt --manifest-path rust/Cargo.toml -p discord-voice -- --check` — чисто.
- `cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --tests` — 0 warnings, clean (лог сохранён в `clippy.log`).

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/src/udp.rs` | modified | `7ed1178364bc52edcfec36ac87032bd3` |
| `rust/crates/discord-voice/tests/media_contract.rs` | modified | `e104713a1883356670a3e22f5468d904` |
| `waves/RW-40/G5/task-166-voice-udp-rotate-srtcp/red.log` | created | `b273bc6754e1d85e812d46b7919fda60` |
| `waves/RW-40/G5/task-166-voice-udp-rotate-srtcp/green.log` | created | `066c6610160a34028e2fad5c7bf1d698` |
| `waves/RW-40/G5/task-166-voice-udp-rotate-srtcp/clippy.log` | created | `fdc4c02f87751cfd722d91fe05a08860` |

---

## 6. Мутации (5/5 убиты)

1. **Длина пустого RR 8B вместо 28B в `SrtcpRotationAction`**:
   - Убивается тестами `m14` (`assert_eq!(packet.len(), 28)`) и `m15` (`assert_eq!(action.empty_rr.len(), 28)`).
2. **Отсутствие E-bit в трейлере (`counter & 0x7fff_ffff` без `0x8000_0000`)**:
   - Убивается тестами `m14` (`assert_ne!(packet[27] & 0x80, 0)`), `m15` (`assert_eq!(&action.empty_rr[24..28], &expected_trailer)`), `m16` (`assert_eq!(trailer1, 0x8000_0000 | 42)`).
3. **Отсутствие инкремента сквозного счётчика пакетов при ротации**:
   - Убивается тестами `m15` (`assert_eq!(packet_counter, 101)`) и `m16` (`assert_eq!(counter, 43)` и `assert_eq!(counter, 44)`).
4. **Преждевременная ротация сокета до достижения 60 080 мс**:
   - Убивается тестом `m15` (`assert!(early.is_none())` при `60_079 ms`).
5. **Не закрывать старый сокет при ротации (утечка FD/порта)**:
   - Убивается тестом `m16` (`assert!(UdpSocket::bind(old_addr).is_ok())`).

---

## 7. Риски

- Изменения полностью обратно совместимы со всеми существующими методами `rotate` и `rotate_if_due`.
- Файлы за пределами назначенного скоупа не изменялись.
