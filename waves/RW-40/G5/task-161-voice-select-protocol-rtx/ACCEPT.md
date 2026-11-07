# PM Acceptance — task-161 (`BE-P1-08`: `Codec` & `SelectProtocol` RTX Fields, `rtc_connection_id` & `experiments` in `discord-voice`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/crates/discord-voice/Cargo.toml`
  - `rust/crates/discord-voice/src/lib.rs`
  - `rust/crates/discord-voice/src/session.rs`
  - `rust/crates/discord-voice/tests/signaling.rs`
- **Соответствие ТЗ и оракулу:**
  1. Структура `Codec` расширена полями `rtx_payload_type: Option<u8>`, `encode: Option<bool>`, `decode: Option<bool>`, добавлен конструктор `default_client_codecs()` (5 кодеков: `opus`, `AV1`, `H265`, `H264`, `VP8` с RTX PT `102/104/106/108`).
  2. Структура `SelectProtocol` расширена полями `rtc_connection_id: Option<String>` и `experiments: Vec<String>` с побайтовой сверкой против эталонного кадра `op1` из `/home/sanya/re-deep/voice/rtc1-frames.jsonl` и 100% обратной совместимостью.
  3. Все 147 тестов `discord-voice` проходят (`0 fail`, `clippy 0`).
- **Следующий шаг:** Техническое ревью (`Review 76A`) с проверкой `5/5` мутаций.
