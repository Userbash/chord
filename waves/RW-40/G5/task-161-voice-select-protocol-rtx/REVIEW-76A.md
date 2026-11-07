# Technical Review: task-161 (BE-P1-08) — Codec & SelectProtocol RTX Fields, rtc_connection_id & experiments in discord-voice

- **Карточка:** `task-161` (`BE-P1-08`)
- **Волна:** `RW-40`
- **Ревьюер:** Technical Reviewer (G5 Platform / Voice Review)
- **Вердикт:** **PASS / Ready for Merge**
- **Оракул:** `/home/sanya/re-deep/voice/rtc1-frames.jsonl` (кадр op1 SelectProtocol), `waves/RW-40/PM/BACKLOG-RE-BACKEND.md` (§BE-P1-08).

---

## 1. Проверенные файлы и контрольные суммы (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/Cargo.toml` | modified | `dca6d4174797aeed0064c9f0a0c74fa2` |
| `rust/crates/discord-voice/src/lib.rs` | modified | `6e1436bc535b36e55dd1d97778a61622` |
| `rust/crates/discord-voice/src/session.rs` | modified | `691ef3ab823041b6cf822493f289807f` |
| `rust/crates/discord-voice/tests/signaling.rs` | modified | `8da9219f6d257768fe1e604af612ceda` |

Файлы после проверки мутаций полностью восстановлены, все md5sum в точности совпадают с исходными.

---

## 2. Результаты запуска тестовых наборов

Команда: `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-voice`

- `unittests src/lib.rs`: 14 passed
- `tests/crypto_vectors.rs`: 16 passed
- `tests/devices_contract.rs`: 5 passed
- `tests/media_contract.rs`: 13 passed
- `tests/reception_contract.rs`: 10 passed
- `tests/reconnect_contract.rs`: 10 passed
- `tests/regions_contract.rs`: 10 passed
- `tests/rtcp_contract.rs`: 16 passed
- `tests/signaling.rs`: 14 passed (включая новые тесты спецификации и сверки с RE-оракулом)
- `tests/srtp_contract.rs`: 22 passed
- `tests/stats_contract.rs`: 12 passed
- `tests/udp.rs`: 15 passed
- **Итого:** 147 passed; 0 failed; 0 ignored; 0 measured.
- **Clippy:** `cargo clippy -p discord-voice --all-targets` — 0 warnings, clean.
- **Fmt:** `cargo fmt --package discord-voice -- --check` — clean.

---

## 3. Проверка ручных мутаций (5/5 убито)

1. **Мутация 1 (Изменение RTX payload type для AV1 с 102 на 199):**
   - Файл: `rust/crates/discord-voice/src/lib.rs`
   - Результат: Тест `test_default_client_codecs_specification` упал (`left: Some(199), right: Some(102)`).
   - Статус: **KILLED** (1/5)
2. **Мутация 2 (Пропуск сериализации поля encode в SelectProtocol::to_frame_json):**
   - Файл: `rust/crates/discord-voice/src/lib.rs`
   - Результат: Тест `test_select_protocol_wire_parity_with_rtc1_oracle` упал (`assertion left == right failed: left: None, right: Some(true)`).
   - Статус: **KILLED** (2/5)
3. **Мутация 3 (Опущение rtc_connection_id в SelectProtocol::to_frame_json):**
   - Файл: `rust/crates/discord-voice/src/lib.rs`
   - Результат: Тест `test_select_protocol_wire_parity_with_rtc1_oracle` упал (`assertion left == right failed: left: None, right: Some("5363ba05-d6fa-4736-8c04-0d09144e7a8d")`).
   - Статус: **KILLED** (3/5)
4. **Мутация 4 (Игнорирование поля experiments при разборе parse_select_protocol):**
   - Файл: `rust/crates/discord-voice/src/lib.rs`
   - Результат: Тест `test_select_protocol_wire_parity_with_rtc1_oracle` упал (`assertion left == right failed: left: [], right: ["fixed_keyframe_interval"]`).
   - Статус: **KILLED** (4/5)
5. **Мутация 5 (Присвоение rtx_payload_type: Some(121) кодеку opus вместо None):**
   - Файл: `rust/crates/discord-voice/src/lib.rs`
   - Результат: Тест `test_default_client_codecs_specification` упал (`assertion left == right failed: left: Some(121), right: None`).
   - Статус: **KILLED** (5/5)

---

## 4. Архитектурная оценка и безопасность

- Строгая обратная совместимость: сериализация и парсинг старых кадров `SelectProtocol` без расширенных полей работают штатно (проверено `test_select_protocol_backward_compatibility_without_new_fields`).
- Побайтовое соответствие wire-формату: `to_frame_json()` в точности дублирует поля `address`, `port`, `mode` на верхнем уровне объекта `d` и в `d.data`, как это делает реальный клиент Discord в трассе `/home/sanya/re-deep/voice/rtc1-frames.jsonl`.
- Чистота кода: соблюдены все правила проекта, запреты `unsafe` (`unsafe_code = "forbid"`), отсутствуют предупреждения линтера и форматирования.

---

## 5. Итоговый вердикт

- Замечаний `P0`: 0
- Замечаний `P1`: 0
- Замечаний `P2`: 0
- **Итог:** **PASS / Ready for Merge**
