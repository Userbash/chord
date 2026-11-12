# ТЗ RW-12 / I1 — REST-фасад поверх транспорта (интеграционная дорожка)

**Карточка:** инфраструктура к P1-07/P1-09/P1-11; закрывает разрыв «модули есть, но не
соединены». **Зависимости:** RW-10 (http1/transport) и RW-11 (rate_limit/telemetry) — уже в
дереве, тесты зелёные. **Сеть:** только loopback-стабы, без живого Discord.

## 0. Границы

| Воркер | Файлы (только эти) |
|---|---|
| I1 | `rust/crates/discord-core/src/rest/client.rs`, `rust/crates/discord-core/tests/rest_client.rs` |

Общие файлы (`rest/mod.rs`, `Cargo.toml`, `Cargo.lock`) правит координатор. Новых зависимостей
нет. Golden и `reference-oracle/` — только чтение. Коммитов нет.

## 1. Контракт (заморожен)

Публичные типы/функции `rest::client`:

```rust
pub struct ApiAuth { authorization, super_properties, locale, timezone,
                     debug_options, accept_language: String }
pub struct ApiRetryPolicy { max_attempts: u32, max_retry_after: Duration,
                            backoff: Duration, attempt_timeout: Duration }
pub struct ApiConfig { base: String, auth: ApiAuth, http: HttpClientConfig,
                       retry: ApiRetryPolicy }
pub struct ApiClient { /* Endpoint + config */ }
pub enum ApiError { InvalidBase(String), InvalidRequest(String), TelemetryBlocked,
                    Timeout { attempts: u32 }, Transport(HttpError), Unauthorized,
                    RateLimited { retry_after: Duration, attempts: u32 },
                    Server { status: u16, attempts: u32 }, Client { status: u16 } }

impl ApiClient {
    pub fn new(config: ApiConfig) -> Result<Self, ApiError>;
    pub async fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ApiError>;
    pub async fn get(&self, target: &str) -> Result<HttpResponse, ApiError>;
}
```

- `ApiAuth` — ровно шесть auth-заголовков из `rest::headers`; `authorization` — полное значение
  (`"Bot …"`/токен), остальные — как есть; пустые не отбрасываются (политика вызывающего).
- `ApiRetryPolicy::default`: `max_attempts=3`, `max_retry_after=30 c`, `backoff=250 мс`,
  `attempt_timeout=30 c`; `max_attempts=0` трактуется как 1.
- `ApiConfig::default`: база `API_V9_BASE`, пустой `ApiAuth`, `HttpClientConfig::default()`.

## 2. Поведение `execute`

1. `Endpoint::target` → при ошибке `InvalidRequest`; `telemetry::guard_target` →
   `TelemetryBlocked` **до** сети.
2. Добавить отсутствующие auth-заголовки (порядок `Authorization`, `X-Super-Properties`,
   `X-Discord-Locale`, `X-Discord-Timezone`, `X-Debug-Options`, `Accept-Language`); явные
   заголовки вызывающего не дублировать (сравнение без регистра). Для непустого тела —
   `Content-Type: application/json`, если не задан.
3. Попытка под `attempt_timeout`; транспорт — `send_with`.
4. `classify_failure`:
   - 429 → спать `min(retry_after, max_retry_after)` и повторять **любой** метод, пока есть
     попытки; исчерпание → `RateLimited{retry_after, attempts}`;
   - 5xx → повторять только идемпотентные (GET/PUT/DELETE) с экспоненциальным `backoff`
     (1×, 2×, 4×…, не больше `max_retry_after`); иначе `Server`;
   - 401 → `Unauthorized` без повтора; прочие 4xx → `Client` без повтора;
   - успех → `HttpResponse`.
5. Таймаут попытки/транспортная ошибка: `Connect`/`Io`/таймаут повторяются только для
   идемпотентных; TLS/Protocol/TooLarge/Invalid* — сразу `Transport`. Исчерпание таймаута →
   `Timeout{attempts}`.
6. Ошибки не глотать: каждый исход — вариант `ApiError`; 429/5xx сохраняют статус/задержку.

## 3. Кейсы (red-first)

1. `auth_headers_added`: GET через loopback-стаб; в запросе ровно 6 auth-заголовков с точными
   значениями и в ожидаемом порядке; ответ 200.
2. `explicit_headers_not_duplicated`: явный `Authorization` не дублируется.
3. `content_type_for_body`: POST с телом → `Content-Type: application/json`, `Content-Length`
   равен длине; тело дословно.
4. `telemetry_blocked_before_network`: `/science`, `/beaker?x=1`, `/track#f` → `TelemetryBlocked`
   при базе на закрытый порт (доказывает отсутствие соединения); `/science%2Ffoo` разрешён.
5. `retry_429_then_success`: стаб отдаёт 429 `Retry-After: 0.01`, затем 200; ровно 2 запроса,
   успех, пауза ≥ 10 мс.
6. `retry_429_for_post`: POST 429 → 200; повтор разрешён и для неидемпотентного.
7. `retry_5xx_idempotent`: GET 500 → 200; 2 запроса.
8. `no_retry_5xx_post`: POST 500; 1 запрос; `Server{status:500, attempts:1}`.
9. `no_retry_401`: 401; 1 запрос; `Unauthorized`.
10. `no_retry_404`: 404; 1 запрос; `Client{status:404}`.
11. `exhausted_429`: всегда 429; `max_attempts=2`; 2 запроса; `RateLimited{attempts:2}`.
12. `invalid_base`: `ftp://…`, `https:///api`, кривой порт → `InvalidBase`.
13. `attempt_timeout`: стаб принимает и молчит; `attempt_timeout=20 мс`; GET → `Timeout`
    (не зависание); для POST — одна попытка.
14. `lookalike_path_allowed`: `/science%2Ffoo` доходит до стаба и получает 200.

## 4. DoD

Red-лог и red-копия до реализации; green; `cargo test --offline -p discord-core --test
rest_client`; `cargo clippy --offline -p discord-core --all-targets -- -D warnings`; `cargo fmt
--check`; отчёт `waves/RW-12/reports/i1.md`; `#8`-строка — в `TDD-PLAN.md` только по указанию
координатора.
