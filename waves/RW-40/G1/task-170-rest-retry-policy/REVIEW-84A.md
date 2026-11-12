# Technical Review Report (Review 84A) — PR-55 (task-170 / BE-P2-01 & BE-P3-04)

- **Задача**: task-170 / BE-P2-01 & BE-P3-04 (REST Retry Policy {502,504,507,522,523,524,598,599}, {429,503} retry-after handling & RateLimitInfo limit/scope)
- **Ветка**: `feature/task-170-rest-retry-policy`
- **Пулл-реквест**: `PR-55` (`feature/task-170-rest-retry-policy -> unstable`)
- **Ревьюер**: Technical Reviewer (Review 84A)
- **Дата**: 2026-10-31
- **Вердикт**: **APPROVED (Ready for Merge)** (`P0 = 0`, `P1 = 0`, Mutations: `5/5 KILLED`)

---

## 1. Проверка скоупа и затронутых файлов

Строго соблюдены правила §1.2 AGENTS.md:
Изменены **только** разрешённые в ТЗ файлы:
- `rust/crates/discord-core/src/rest/rate_limit.rs` (MD5: `67a48bdafe13ba5fd9fba894ce7c3908`)
- `rust/crates/discord-core/tests/rest_retry.rs` (MD5: `3c593d47c5ebc1f0f5970a956c7b9fc1`)

Остальные файлы репозитория не модифицировались. Рабочая копия чиста относительно базового коммита ветки.

---

## 2. Результаты статического анализа и тестов

1. **Targeted cargo test**:
   ```bash
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_retry --test rest_client --test rest_rate_limit --lib
   ```
   - `lib.rs`: 33 passed, 0 failed.
   - `tests/rest_client.rs`: 31 passed, 0 failed.
   - `tests/rest_rate_limit.rs`: 10 passed, 0 failed.
   - `tests/rest_retry.rs`: 16 passed, 0 failed.
   - **Суммарно**: 90/90 passed (`0 failed`, `0 flakes`).

2. **Cargo clippy**:
   ```bash
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_retry --lib -- -D warnings
   ```
   - Статус: Clean (0 warnings, 0 errors).

---

## 3. Верификация мутационного тестирования (5/5 KILLED)

Проверена устойчивость тестов к пяти обязательным контрольным мутациям:

1. **Mutation 1 (Retrying non-retryable server status 500, 501, or 505)**:
   - *Описание мутации*: ошибочное включение статусов 500, 501 или 505 в `RETRYABLE_SERVER_STATUSES` или возврат `true` для них в `should_retry_client_parity` / задержки в `retry_delay_client_parity`.
   - *Убийство*:
     - `test_retryable_server_statuses_parity` проверяет точный состав `RETRYABLE_SERVER_STATUSES` и явно утверждает `!is_retryable_server_status(500)`, `!is_retryable_server_status(501)`, `!is_retryable_server_status(505)`.
     - `test_should_retry_client_parity` тестирует методы GET и POST для `RestFailure::Server { status: 500 }`, `501`, `505` и требует `assert!(!should_retry_client_parity(...))` даже при `explicit_retries > 0`.
     - `test_retry_delay_client_parity` проверяет `retry_delay_client_parity(1, &failure_500, &[], b"") == None` и даже при наличии заголовка `retry-after: 3` требует `None`.
   - *Статус*: **KILLED**.

2. **Mutation 2 (Not retrying 503 or ignoring `retry-after: 3` on 503)**:
   - *Описание мутации*: отказ от повтора 503 в `should_retry_client_parity`, возврат `None` в `parse_retry_after_or_default(503, ...)` или возврат дефолтных 5 с вместо переданных 3 с из заголовка `retry-after: 3`.
   - *Убийство*:
     - `test_should_retry_client_parity` явно проверяет `assert!(should_retry_client_parity(HttpMethod::Get, &failure_503, 0))` и `assert!(should_retry_client_parity(HttpMethod::Post, &failure_503, 0))` для 503.
     - `test_parse_retry_after_or_default` проверяет `parse_retry_after_or_default(503, &[("retry-after", "3")], b"") == Some(Duration::from_secs(3))`.
     - `test_retry_delay_client_parity` проверяет, что для 503 с `retry-after: 3` задержка строго равна `Some(Duration::from_secs(3))`.
   - *Статус*: **KILLED**.

3. **Mutation 3 (Wrong DEFAULT_RETRY_AFTER != 5s or MAX_RETRY_AFTER != 60s)**:
   - *Описание мутации*: изменение константы `DEFAULT_RETRY_AFTER` с 5s на другое значение (например, 1s или 10s) или `MAX_RETRY_AFTER` с 60s на иное значение, либо отсутствие ограничения задержки в `min(MAX_RETRY_AFTER)`.
   - *Убийство*:
     - `test_parse_retry_after_or_default` явно утверждает `assert_eq!(DEFAULT_RETRY_AFTER, Duration::from_secs(5))` и `assert_eq!(MAX_RETRY_AFTER, Duration::from_secs(60))`.
     - При отсутствии заголовка `retry-after` для 429 и 503 проверяется точное совпадение с дефолтом `Duration::from_secs(5)`.
     - При значении `retry-after: 120` для 429 или `90` для 503, а также на шаге 7 бэкоффа для 502 проверяется строгое ограничение `Duration::from_secs(60)`.
   - *Статус*: **KILLED**.

4. **Mutation 4 (Ignoring explicit_retries > 0 on POST for 502)**:
   - *Описание мутации*: проверка только идемпотентности метода без учёта `explicit_retries > 0` в `should_retry_client_parity` для кодов из `RETRYABLE_SERVER_STATUSES` (например, для POST 502 возврат `false` даже если `explicit_retries = 1`).
   - *Убийство*:
     - `test_should_retry_client_parity` выполняет сопоставление:
       ```rust
       assert!(!should_retry_client_parity(HttpMethod::Post, &failure_502, 0), "POST 502 без явных попыток НЕ повторяется");
       assert!(should_retry_client_parity(HttpMethod::Post, &failure_502, 1), "POST 502 с explicit_retries=1 повторяется");
       assert!(should_retry_client_parity(HttpMethod::Post, &failure_502, 3), "POST 502 с explicit_retries=3 повторяется");
       ```
     - Любое игнорирование `explicit_retries > 0` немедленно ломает утверждение теста.
   - *Статус*: **KILLED**.

5. **Mutation 5 (Failing to parse `x-ratelimit-limit` or `x-ratelimit-scope`)**:
   - *Описание мутации*: пропуск разбора полей `x-ratelimit-limit` или `x-ratelimit-scope` в `rate_limit_info`, возврат всегда `None`, либо неверный маппинг типов.
   - *Убийство*:
     - `test_rate_limit_info_limit_and_scope` проверяет разбор:
       - `x-ratelimit-limit: 10` -> `info.limit == Some(10)`.
       - `x-ratelimit-scope: user` -> `info.scope == Some("user")`.
       - `x-ratelimit-scope: global` -> `info.scope == Some("global")`.
       - регистронезависимость `X-RateLimit-Scope: shared` -> `info.scope == Some("shared")`.
       - битые значения (`not_a_number`, пустая строка) безопасно парсятся в `None`.
     - Также тестируются билдеры `with_limit` и `with_scope`.
   - *Статус*: **KILLED**.

---

## 4. Архитектурный анализ и обратная совместимость

- **Обратная совместимость (100%)**: все существующие типы, методы и тесты (`classify_failure`, `rate_limit_info`, `retry_delay`, `should_retry`, `RateLimitInfo`) сохранили сигнатуры и поведение. Новые поля `limit: Option<u32>` и `scope: Option<String>` корректно инициализируются дефолтом `None`.
- **Защита от переполнения**: в расчёте экспоненциальной задержки используется `attempt.saturating_sub(1)` и `1u32.checked_shl(exponent).unwrap_or(u32::MAX)` со связкой `saturating_mul` и потолком `min(MAX_RETRY_AFTER)`. Тест на `u32::MAX` попыток подтверждает устойчивость к panic и переполнениям.
- **Соответствие оракулу Discord**: набор серверных кодов `[502, 504, 507, 522, 523, 524, 598, 599]` и константы 5с / 60с полностью соответствуют эталонным чанкам клиента Discord (`web.4d4b2f75fdea5470.js@10 692 340`, `@10 693 500`).

---

## 5. Итог

- Замечания `P0`: 0
- Замечания `P1`: 0
- Замечания `P2`/`P3`: 0
- **Статус**: **APPROVED (Ready for Merge)**.
