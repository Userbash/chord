# Отчёт по задаче task-170: REST Retry Policy & Client Parity (BE-P2-01, BE-P3-04)

- **Воркер:** G1 Backend Worker
- **Ветка:** `feature/task-170-rest-retry-policy`
- **Дата:** 2026-10-31
- **Статус:** Выполнено (Strict TDD Red -> Green, 100% обратная совместимость)

---

## 1. Что сделано

1. **RE-паритет серверных статусов (`BE-P2-01`):**
   - Зафиксирован массив повторяемых серверных статусов `RETRYABLE_SERVER_STATUSES = [502, 504, 507, 522, 523, 524, 598, 599]` согласно эталонному JS-бандлу Discord (`web.4d4b2f75fdea5470.js@10 692 340`, `E = new Set([502, 504, 507, 598, 599, 522, 523, 524])`).
   - Реализован предикат `is_retryable_server_status(status: u16) -> bool`, возвращающий `true` для 8 повторяемых кодов и `false` для 500, 501, 503, 505 и клиентских ошибок.

2. **Клиентская политика разбора `retry-after` и паритет ретраев (`BE-P2-01`):**
   - Определены константы `DEFAULT_RETRY_AFTER = Duration::from_secs(5)` и `MAX_RETRY_AFTER = Duration::from_secs(60)` (оракул: бандл @10 693 500 `f(t.headers, t.body) ?? 5`, cap 60 с).
   - Реализована функция `parse_retry_after_or_default(status: u16, headers: &[(String, String)], body: &[u8]) -> Option<Duration>`: для 429 и 503 разбирает заголовок `retry-after` (секунды) или JSON-поле `retry_after` (float секунды), при отсутствии использует дефолт 5 с, с верхним ограничением 60 с; для других статусов возвращает `None`.
   - Реализована функция `should_retry_client_parity(method: HttpMethod, failure: &RestFailure, explicit_retries: u32) -> bool`: возвращает `true` для 429 и 503; для статусов из `RETRYABLE_SERVER_STATUSES` возвращает `true`, если метод идемпотентен (`is_idempotent(method)`) или переданы явные попытки (`explicit_retries > 0`); возвращает `false` для 500, 501, 505 и 401/4xx.
   - Реализована функция `retry_delay_client_parity(attempt: u32, failure: &RestFailure, headers: &[(String, String)], body: &[u8]) -> Option<Duration>`: для 429 и 503 применяет `parse_retry_after_or_default` (503 с `retry-after: 3` ждёт 3 с, без заголовка — 5 с, cap 60 с); для повторяемых серверных статусов вычисляет экспоненциальную паузу `1s * 2^(attempt-1)` с потолком 60 с; для 500/501/505 и 401/4xx возвращает `None`.

3. **Расширение метаданных rate-limit (`BE-P3-04`):**
   - `RateLimitInfo` расширен полями `limit: Option<u32>` (`x-ratelimit-limit`) и `scope: Option<String>` (`x-ratelimit-scope`: user/global/shared).
   - Добавлены конструкторы и билдер-методы: `RateLimitInfo::new(retry_after)`, `RateLimitInfo::with_limit`, `RateLimitInfo::with_scope`, а также реализация `Default`.
   - Сохранена 100% обратная совместимость со всеми существующими тестами и сопоставлениями структуры.

---

## 2. Red-прогон

- **Команда:**
  ```bash
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_retry --test rest_client --lib 2>&1 | tee /home/user/orca/project/waves/RW-40/G1/task-170-rest-retry-policy/red.log
  ```
- **Результат:** 15 ошибок компиляции в `rest_retry.rs` (отсутствие констант `DEFAULT_RETRY_AFTER`, `MAX_RETRY_AFTER`, `RETRYABLE_SERVER_STATUSES`, функций `is_retryable_server_status`, `parse_retry_after_or_default`, `retry_delay_client_parity`, `should_retry_client_parity`, полей `limit` и `scope`, а также конструктора `RateLimitInfo::new`).
- **Лог:** сохранён в `waves/RW-40/G1/task-170-rest-retry-policy/red.log`.

---

## 3. Green-прогон

- **Команда:**
  ```bash
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_retry --test rest_client --lib 2>&1 | tee /home/user/orca/project/waves/RW-40/G1/task-170-rest-retry-policy/green.log
  ```
- **Результат:** Все тесты успешно пройдены:
  - `rest_retry`: 16 passed, 0 failed (11 исходных + 5 новых contract-тестов P1–P5)
  - `rest_client`: 31 passed, 0 failed
  - `lib` unittests: 33 passed, 0 failed
  - `rest_rate_limit`: 10 passed, 0 failed
- **Лог:** сохранён в `waves/RW-40/G1/task-170-rest-retry-policy/green.log`.

---

## 4. Targeted-проверки и Clippy

- `cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_retry --test rest_client --lib -- -D warnings`: чистый прогон (0 warnings / 0 errors).

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | MD5 |
|---|---|
| `rust/crates/discord-core/src/rest/rate_limit.rs` | `67a48bdafe13ba5fd9fba894ce7c3908` |
| `rust/crates/discord-core/tests/rest_retry.rs` | `3c593d47c5ebc1f0f5970a956c7b9fc1` |
| `waves/RW-40/G1/task-170-rest-retry-policy/red.log` | `8c66e2c3fe8f070ca01bf36573c4ee4e` |
| `waves/RW-40/G1/task-170-rest-retry-policy/green.log` | `378051e7fb508d530f2526e95267e7c8` |
| `waves/RW-40/G1/task-170-rest-retry-policy/REPORT.md` | (этот файл) |

---

## 6. Риски и обратная совместимость

- **Обратная совместимость:** 100%. Прежние методы `retry_delay`, `should_retry`, `classify_failure` и `rate_limit_info` сохранены без изменения поведения и сигнатур.
- **Политика отсутствия сетевых вызовов:** все тесты работают на чистых структурах данных без обращения к внешним ресурсам.
- **Защита от переполнения:** экспоненциальный бэкофф использует `saturating_sub(1)` и `checked_shl` с насыщением до `u32::MAX`, задержка строго ограничена `MAX_RETRY_AFTER = 60s`.
