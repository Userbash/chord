# Приёмка PM: task-170 (BE-P2-01 & BE-P3-04) — Политика ретраев REST v9 ({502,504,507,522,523,524,598,599} и {429,503} retry-after) и поля limit/scope в RateLimitInfo

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-170-rest-retry-policy`
- **Пулл-реквест:** `PR-55` (`feature/task-170-rest-retry-policy -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **`RETRYABLE_SERVER_STATUSES` и предикат `is_retryable_server_status` в `rate_limit.rs`**:
   - Зафиксирован набор повторяемых серверных кодов `[502, 504, 507, 522, 523, 524, 598, 599]`; коды `500/501/505` не повторяются.
2. **Клиентский паритет `parse_retry_after_or_default`, `should_retry_client_parity`, `retry_delay_client_parity`**:
   - Для `429` и `503` парсится заголовок `retry-after` или JSON `retry_after` (дефолт 5 с, потолок 60 с; `503` с `retry-after: 3` ждёт 3 с).
3. **Поля `limit` и `scope` в `RateLimitInfo` (`BE-P3-04`)**:
   - Добавлен разбор `x-ratelimit-limit` и `x-ratelimit-scope` при сохранении 100% обратной совместимости.
   - 16 тестов в `rest_retry.rs`, 31 в `rest_client.rs`, 33 в `lib.rs` проходят (`80/80 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 84A)** (`5/5` мутаций).
