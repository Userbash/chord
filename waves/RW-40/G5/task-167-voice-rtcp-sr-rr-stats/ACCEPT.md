# Приёмка PM: task-167 (BE-P2-08) — RTCP 60B SRTCP Sender Report (~5 с) и 52B серверный Receiver Report с расчётом потерь, джиттера и RTT в stats

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-167-voice-rtcp-sr-rr-stats`
- **Пулл-реквест:** `PR-52` (`feature/task-167-voice-rtcp-sr-rr-stats -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **Константы, планировщик и сборка 60B SRTCP SR / разбор 52B серверного RR в `rtcp.rs`**:
   - Реализованы `RTCP_SR_INTERVAL_MS = 5_000`, `SRTCP_SR_WIRE_LEN = 60`, `SRTCP_RR_WIRE_LEN = 52`, `RtcpSenderScheduler`, `build_sender_report` (28B), `build_srtcp_sender_report` (60B с E-bit), `build_receiver_report` (32B), `build_srtcp_receiver_report` (52B), `is_server_receiver_report`, `parse_server_receiver_report`.
2. **Метрики потерь, джиттера и RTT в `stats.rs`**:
   - Реализованы `StatsSample::from_report_block` (Opus 48 кГц jitter в мс, `fraction_lost`, `cumulative_lost`) и `SessionStats` с безопасным учётом `on_incoming_rr_packet(..., has_key)` без расшифровки при отсутствии ключа.
3. **Контрактные тесты в `stats_contract.rs`**:
   - 8 новых контрактных тестов сверены с эталоном `strace-media-s3.jsonl`, `154/154 PASS`, `clippy 0 warnings`.

Передано на **Техническое ревью (Review 81A)** (`5/5` мутаций).
