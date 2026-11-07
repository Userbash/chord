# Техническое ревью: PR-52 / task-167 (BE-P2-08)

- **Карточка:** `task-167` (`BE-P2-08`)
- **Компонент:** `discord-voice` (`rtcp.rs`, `stats.rs`, `tests/stats_contract.rs`)
- **Ветка:** `feature/task-167-voice-rtcp-sr-rr-stats`
- **Ревьюер:** Technical Reviewer (Review 81A)
- **Вердикт:** **APPROVED (Ready for Merge)**
- **Блокирующие дефекты:** `P0 = 0`, `P1 = 0`

---

## 1. Проверка изоляции скоупа и изменений

1. **Проверка файлов (`git status` / `git diff HEAD`):**
   - Изменены строго 3 разрешённых файла:
     - `rust/crates/discord-voice/src/rtcp.rs`
     - `rust/crates/discord-voice/src/stats.rs`
     - `rust/crates/discord-voice/tests/stats_contract.rs`
   - Никакие другие модули, общие структуры или манифесты не затронуты.
2. **MD5 контрольные суммы файлов:**
   - `rust/crates/discord-voice/src/rtcp.rs`: `f244c7815d4f6a65f3603cb1fcf7a406`
   - `rust/crates/discord-voice/src/stats.rs`: `5db68368e2e1bb6ca7524a68186fd504`
   - `rust/crates/discord-voice/tests/stats_contract.rs`: `9b986fda33c7c05f3adba0f094582dda`

---

## 2. Результаты компиляции, тестов и линтеров

- **Тестирование целевых контрактов:**
  `cargo test --manifest-path rust/Cargo.toml -p discord-voice --test stats_contract --test rtcp_contract --lib`
  - `src/lib.rs` (unit-тесты): 14 passed; 0 failed
  - `tests/rtcp_contract.rs`: 16 passed; 0 failed
  - `tests/stats_contract.rs`: 20 passed (включая 8 новых тестов RTCP/SRTCP); 0 failed
- **Полное тестирование крейта `discord-voice`:**
  `cargo test --manifest-path rust/Cargo.toml -p discord-voice`
  - Все тесты (154 теста во всех тест-сьютах: crypto, devices, media, reception, reconnect, rtcp, signaling, srtp, stats, udp) успешно пройдены: 154 passed; 0 failed; 0 ignored.
- **Статический анализ Clippy:**
  `cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --tests -- -D warnings`
  - Чисто: 0 warnings, 0 errors.
- **Форматирование rustfmt:**
  `cargo fmt --manifest-path rust/Cargo.toml -p discord-voice -- --check`
  - Соответствует стилю воркспейса.

---

## 3. Верификация уничтожения мутаций (5/5 KILLED)

1. **Мутация 1 (Длина пакета SR: 60B vs 48B):**
   - *Мутация:* Генерация Sender Report размером 48 байт (без трейлера/extra) вместо стандартного для Discord 60-байтового wire-пакета в `build_srtcp_sender_report`.
   - *Убита:* Тест `srtcp_sender_report_60_byte_wire_layout_matches_re_oracle` проверяет точную длину `assert_eq!(wire.len(), 60)` и побайтовое совпадение с дампом `strace-media-s3.jsonl` (out packet #0): `assert_eq!(wire.as_slice(), oracle_bytes.as_slice())`.
   - *Статус:* **KILLED**.

2. **Мутация 2 (Отсутствие флага E-bit `0x8000_0000` в трейлере SRTCP SR):**
   - *Мутация:* `srtcp_index` записывается напрямую без установки старшего бита `SRTCP_E_BIT = 0x8000_0000`.
   - *Убита:* Тест `srtcp_sender_report_60_byte_wire_layout_matches_re_oracle` валидирует байт 59 (`wire[59] == 0x80`), а также побайтовое совпадение с оракулом трейлера `08000080`.
   - *Статус:* **KILLED**.

3. **Мутация 3 (Интервал периодической отправки SR != 5 000 мс):**
   - *Мутация:* `RTCP_SR_INTERVAL_MS` изменён на другое значение (например, 1 000 или 10 000 мс) или ошибка в логике `RtcpSenderScheduler::is_due` / `time_until_due_ms`.
   - *Убита:* Тест `rtcp_sender_scheduler_5000ms_interval` проверяет константу `assert_eq!(RTCP_SR_INTERVAL_MS, 5_000)`, граничное условие `assert!(!scheduler.is_due(5_999))`, срабатывание `assert!(scheduler.is_due(6_000))` и расчёт оставшегося времени `time_until_due_ms(3_500) == 2_500`.
   - *Статус:* **KILLED**.

4. **Мутация 4 (Интерпретация шифротекста как потерь/джиттера при `has_key = false`):**
   - *Мутация:* Разбор зашифрованного payload'а в `on_incoming_rr_packet` при отсутствии ключа и запись мусорных данных в метрики `latest_fraction_lost`, `latest_jitter_ms` или окно сэмплов.
   - *Убита:* Тест `session_stats_safe_counting_without_key` проверяет, что при `has_key = false`: `undecrypted_rr_count == 1`, `latest_fraction_lost == None`, `latest_jitter_ms == None`, а `window.is_empty() == true`, при этом `server_ssrc` корректно извлекается из открытого 8-байтового заголовка.
   - *Статус:* **KILLED**.

5. **Мутация 5 (Неверная конвертация джиттера для кодека Opus 48 кГц):**
   - *Мутация:* Отсутствие деления на частоту 48 000 Гц при переводе джиттера из тактов RTP в миллисекунды (`jitter_ms = (jitter * 1000) / 48_000`).
   - *Убита:* Тесты `stats_sample_from_report_block_calculates_fraction_cumulative_and_jitter_ms` (240 тактов -> ровно 5.0 мс) и `session_stats_records_loss_jitter_and_rtt_on_report_block` (96 тактов -> ровно 2.0 мс) проверяют точную формулу RFC 3550 для Opus 48 kHz.
   - *Статус:* **KILLED**.

---

## 4. Архитектурное и техническое заключение

- Код полностью соответствует спецификации `docs/re/voice-deep.md`, контрактам `strace-media-s3.jsonl` и требованиям Zero-Idle/Strict TDD.
- Обратная совместимость с существующим кодом `discord-voice` не нарушена (все 154 теста крейта зелёные).
- Git коммит не создавался согласно регламенту ревьюера.
- Рекомендация: **PR-52 готов к слиянию в ветку `unstable`**.
