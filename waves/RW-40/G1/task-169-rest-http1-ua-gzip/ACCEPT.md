# Приёмка PM: task-169 (BE-P2-03) — User-Agent и распаковка Content-Encoding: gzip / deflate в HTTP/1.1 транспорте discord-core::rest

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-169-rest-http1-ua-gzip`
- **Пулл-реквест:** `PR-53` (`feature/task-169-rest-http1-ua-gzip -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **`DEFAULT_USER_AGENT` и авто-подстановка в `transport.rs`**:
   - В `http1.rs` и `transport.rs` добавлен `DEFAULT_USER_AGENT` из идентичности Discord 1.0.162, автоматическая подстановка `User-Agent` (если не передан) и `Accept-Encoding: gzip, deflate`.
2. **Автономная распаковка `Content-Encoding: gzip` (RFC 1952) и `deflate` (RFC 1950/1951) в `http1.rs`**:
   - Реализованы `decode_response_body`, `decode_response_body_with_limit`, `decode_gzip`, `decode_deflate`, `decode_deflate_or_zlib` с проверкой CRC32/ISIZE/Adler32 и защитой от gzip-бомб (`max_body_bytes`).
3. **Контрактные тесты в `rest_http1.rs`**:
   - 27 тестов в `rest_http1.rs`, 14 в `rest_transport.rs`, 26 в `rest_client.rs`, 33 в `lib.rs` проходят (`100/100 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 82A)** (`5/5` мутаций).
