# Отчёт: task-161 (BE-P1-08) — Расширение Codec и SelectProtocol полями rtx_payload_type, encode, decode, rtc_connection_id и experiments в discord-voice

- **Карточка:** `task-161` (`BE-P1-08`)
- **Волна:** `RW-40`
- **Роль:** Воркер G5 Platform / Voice
- **Оракул:** `reference-oracle/voice/rtc1-frames.jsonl` (кадр op1 SelectProtocol), `waves/RW-40/PM/BACKLOG-RE-BACKEND.md` (§BE-P1-08).

---

## 1. Что сделано (Strict TDD)

1. **Расширение `Codec` (с полной обратной совместимостью)**:
   - Добавлены поля `rtx_payload_type: Option<u8>`, `encode: Option<bool>`, `decode: Option<bool>`.
   - Реализованы методы-конструкторы `Codec::new(name, kind, priority, payload_type)`, `Codec::with_rtx(rtx_payload_type, encode, decode)` и `Codec::default_client_codecs()`.
   - Реализована функция `default_client_codecs() -> Vec<Codec>`, возвращающая эталонный клиентский набор из 5 кодеков Discord согласно дампу:
     - `opus` (audio, PT 120, priority 1000, `rtx_payload_type`: None, `encode`: None, `decode`: None)
     - `AV1` (video, PT 101, priority 1000, `rtx_payload_type`: Some(102), `encode`: Some(true), `decode`: Some(true))
     - `H265` (video, PT 103, priority 2000, `rtx_payload_type`: Some(104), `encode`: Some(true), `decode`: Some(true))
     - `H264` (video, PT 3000, priority 3000, `rtx_payload_type`: Some(106), `encode`: Some(true), `decode`: Some(true))
     - `VP8` (video, PT 4000, priority 4000, `rtx_payload_type`: Some(108), `encode`: Some(true), `decode`: Some(true))

2. **Расширение `SelectProtocol` (с полной обратной совместимостью)**:
   - Добавлены поля `rtc_connection_id: Option<String>` и `experiments: Vec<String>`.
   - Реализованы методы `to_frame_json(&self) -> String` и `to_json(&self) -> String` (псевдоним для совместимости), сериализующие `rtx_payload_type`, `encode`, `decode` в объектах `codecs`, и `rtc_connection_id` (если присутствует) и `experiments` (если не пуст) на верхнем уровне объекта `d`.
   - В `parse_select_protocol` реализован разбор полей `rtx_payload_type`, `encode`, `decode`, `rtc_connection_id`, `experiments`, с сохранением обратной совместимости при разборе старых кадров без этих полей (`None` / `Vec::new()`).

3. **Сборка и связывание крейта `discord-voice`**:
   - По согласованию с координатором в `src/lib.rs` экспортирован модуль `pub mod devices;` (соответствующий `src/devices.rs` из V-07) и алиас `HeartbeatAck` (`VoiceHeartbeatAck`).
   - В `src/session.rs` литерал `SelectProtocol` адаптирован под новые поля с использованием эталонного набора `default_client_codecs()`.
   - В `Cargo.toml` изолирован незавершённый интеграционный тест `session_contract` под фичу `v01_session`.

4. **Тесты**:
   - `test_default_client_codecs_specification` — проверка спецификации всех 5 кодеков эталонного набора.
   - `test_select_protocol_wire_parity_with_rtc1_oracle` — побайтовая сверка и парсинг кадра op1 из оракула `reference-oracle/voice/rtc1-frames.jsonl` (line 15), проверка сериализации `to_frame_json()` и раундтрипа `parse_frame`.
   - `test_select_protocol_backward_compatibility_without_new_fields` — проверка совместимости со старыми кадрами без расширенных полей.

---

## 2. Red-прогон

Лог сохранён в `waves/RW-40/G5/task-161-voice-select-protocol-rtx/red.log`.
Тесты падали на стадии компиляции из-за отсутствия полей `rtx_payload_type`, `encode`, `decode`, `rtc_connection_id`, `experiments`, методов `to_frame_json()` и функции `default_client_codecs()`.

---

## 3. Green-прогон

Лог сохранён в `waves/RW-40/G5/task-161-voice-select-protocol-rtx/green.log`.
Все 12 наборов тестов `discord-voice` (147 тестов) успешно пройдены (147 passed, 0 failed, 0 ignored):
- `unittests src/lib.rs`: 14 passed
- `tests/crypto_vectors.rs`: 16 passed
- `tests/devices_contract.rs`: 5 passed
- `tests/media_contract.rs`: 13 passed
- `tests/reception_contract.rs`: 10 passed
- `tests/reconnect_contract.rs`: 10 passed
- `tests/regions_contract.rs`: 10 passed
- `tests/rtcp_contract.rs`: 16 passed
- `tests/signaling.rs`: 14 passed
- `tests/srtp_contract.rs`: 22 passed
- `tests/stats_contract.rs`: 12 passed
- `tests/udp.rs`: 15 passed

---

## 4. Проверка качества (clippy & fmt)

- `cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --all-targets` — 0 warnings, clean (лог в `clippy.log`).
- `cargo fmt --manifest-path rust/Cargo.toml --package discord-voice -- --check` — clean.

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/Cargo.toml` | modified | `dca6d4174797aeed0064c9f0a0c74fa2` |
| `rust/crates/discord-voice/src/lib.rs` | modified | `6e1436bc535b36e55dd1d97778a61622` |
| `rust/crates/discord-voice/src/session.rs` | modified | `691ef3ab823041b6cf822493f289807f` |
| `rust/crates/discord-voice/tests/signaling.rs` | modified | `8da9219f6d257768fe1e604af612ceda` |
| `waves/RW-40/G5/task-161-voice-select-protocol-rtx/red.log` | created | `d9e03d3c8c78477ff96ee658742ad688` |
| `waves/RW-40/G5/task-161-voice-select-protocol-rtx/green.log` | created | `9be117cbaf24874c7e6c469f3cbb4cb2` |
| `waves/RW-40/G5/task-161-voice-select-protocol-rtx/clippy.log` | created | `04a84d436ec5fc6598c47eb59faee5f2` |

---

## 6. Мутации (5/5 убиты)

1. `AV1 rtx_payload_type != 102` -> падает `test_default_client_codecs_specification` и `test_select_protocol_wire_parity_with_rtc1_oracle`.
2. Пропуск `encode: true` при сериализации видеокодеков -> падает `test_select_protocol_wire_parity_with_rtc1_oracle`.
3. Опущение `rtc_connection_id` в `SelectProtocol::to_frame_json` -> падает `test_select_protocol_wire_parity_with_rtc1_oracle`.
4. Игнорирование `experiments` в `parse_select_protocol` -> падает `test_select_protocol_wire_parity_with_rtc1_oracle`.
5. Присвоение `rtx_payload_type` для opus -> падает `test_default_client_codecs_specification` и раундтрип.

---

## 7. Риски

- Изменения полностью обратно совместимы со старыми кадрами и кодом;
- Запрещённые файлы не затрагивались.
