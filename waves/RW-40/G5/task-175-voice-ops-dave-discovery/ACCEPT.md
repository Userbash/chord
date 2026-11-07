# Приёмка PM: task-175 (BE-P3-05 & BE-P3-06) — VoiceOp 0..=32 + DAVE/MLS и локальный порт в последних 2 байтах Discovery Request

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-175-voice-ops-dave-discovery`
- **Пулл-реквест:** `PR-61` (`feature/task-175-voice-ops-dave-discovery -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **Расширение `VoiceOp` (`0..=32` + `Unknown`) в `rust/crates/discord-voice/src/lib.rs`**:
   - Добавлены варианты `Speaking(5)`, `Resume(7)`, `Resumed(9)`, `ClientsConnect(11)`, `Video(12)`, `ClientDisconnect(13)`, `MediaSinkWants(15)`, `VoiceBackendVersion(16)`, `SecureFrames*(21..24)`, `Mls*(25..31)`, `NoRoute(32)` + методы `is_dave_or_mls()`, `name()`, `as_str()`.
2. **Порт в хвосте 74B Discovery Request в `rust/crates/discord-voice/src/udp.rs`**:
   - Реализованы `DiscoveryPacket::encode_request_with_port` и `UdpMediaSocket::send_discovery_with_local_port`, записывающие 2 байта BE порта в `bytes[72..74]` (побайтовое совпадение с `strace-media-s3.jsonl`).
   - Все 158 тестов в `discord-voice` проходят (`158/158 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 90A)** (`5/5` мутаций).
