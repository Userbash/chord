# Приёмка PM: task-166 (BE-P2-07) — Ротация UDP-сокета в discord-voice (28B SRTCP Empty RR + 74B IP Discovery каждые 60 080 мс)

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-166-voice-udp-rotate-srtcp`
- **Пулл-реквест:** `PR-51` (`feature/task-166-voice-udp-rotate-srtcp -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **`SRTCP_EMPTY_RR_LEN = 28` и `SrtcpRotationAction` в `udp.rs`**:
   - В `rust/crates/discord-voice/src/udp.rs` добавлены `SRTCP_EMPTY_RR_LEN = 28`, структура `SrtcpRotationAction` (74B `discovery` + 28B `empty_rr`) и `RotationSchedule::action_srtcp`.
2. **Ротация `rotate_if_due_srtcp` каждые `60 080 мс` с закрытием старого сокета и сквозным счётчиком**:
   - Закрывает старый сокет (`self.socket.take()`), привязывает новый сокет к эфемерному порту, отправляет 74B IP Discovery и 28B SRTCP Empty RR с E-bit (`0x8000_0000 | counter`) и инкрементирует сквозной счётчик пакетов (`packet_counter`).
3. **Контрактные тесты**:
   - `m14_srtcp_empty_rr_exact_byte_layout_oracle_28b`, `m15_udp_socket_rotation_due_at_60080ms_sends_discovery_and_28b_srtcp_rr`, `m16_udp_socket_rotation_closes_old_socket_and_increments_shared_counter` проходят (`150/150 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 80A)** (`5/5` мутаций).
