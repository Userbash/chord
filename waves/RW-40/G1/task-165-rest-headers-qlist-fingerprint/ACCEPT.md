# PM Acceptance — task-165 (`BE-P2-02`: REST Headers `Accept-Language` q-list, `X-Fingerprint`, `X-Installation-ID` & `X-Routing-Key`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/crates/discord-core/src/rest/headers.rs`
  - `rust/crates/discord-core/src/rest/client.rs`
  - `rust/crates/discord-core/tests/rest_client.rs`
- **Соответствие ТЗ и оракулу:**
  1. Реализована `format_accept_language(locales: &[&str]) -> String` с весами `q=0.9..0.1` и `ApiAuth::from_token_with_locales`.
  2. Опциональные заголовки `X-Fingerprint`, `X-Installation-ID`, `X-Routing-Key` (`HeaderConfig`) отправляются только при непустом значении.
  3. 31/31 тестов `rest_client` и 33/33 unit-теста `lib` проходят (`0 fail`, `clippy 0`).
- **Следующий шаг:** Техническое ревью (`Review 79A`) с проверкой `5/5` мутаций.
