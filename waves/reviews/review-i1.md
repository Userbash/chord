# Независимое ревью I1 — REST-фасад (2026-10-20)

**Ревьюер:** независимый субагент (не автор кода). Только чтение; правок и коммитов нет.
**Область:** `rust/crates/discord-core/src/rest/client.rs`,
`rust/crates/discord-core/tests/rest_client.rs`; контракт `waves/RW-12/TZ.md` §0–§2.

## 1. Итоговый вердикт

**Принято.** Первый вердикт — «принято с нитами» (2×P2, P3). После red-first закрытия
P2-1/P2-2/P3-3…P3-6 повторная проверка (та же сессия ревьюера) подтвердила: блокирующих
находок нет, гейты зелёные. Единственная содержательная незакрытая строка — P3-7
(`AUTH_HEADER_NAMES` вместо литералов) — вынесена в backlog и приёмку не блокирует.

## 2. Находки и закрытие

| ID | Severity | Суть | Статус |
|---|---|---|---|
| P2-1 | P2 | `HttpError::Timeout` (DNS/чтение/запись/TLS-рукопожатие) не повторялся для идемпотентных, хотя §2.5 требует повторять таймаут | **Закрыт**: `is_retryable_transport` += `Timeout`; тесты GET (повтор → 200, 2 соединения) и POST (`Transport(Timeout)`, счётчик соединений = 1) |
| P2-2 | P2 | `HttpError::Protocol(_)` → `InvalidRequest` вместо `Transport` | **Закрыт**: ветка `Protocol` удалена (fallthrough в `Transport`); тест с битым заголовком → `Transport(Protocol)`, 1 соединение |
| P3-3 | P3 | Порог паузы 429 ослаблен: Retry-After 10 мс, проверялось ≥5 мс | **Закрыт**: порог ≥10 мс (test-only усиление) |
| P3-4 | P3 | Не проверялось «ровно 6 auth-заголовков» | **Закрыт**: счётчик каждого имени == 1 (test-only) |
| P3-5 | P3 | `server.await` без таймаута мог подвесить тест | **Закрыт**: все 13 `server.await` обёрнуты в `timeout(CASE_TIMEOUT, …)` |
| P3-6 | P3 (security) | `Debug` у `ApiAuth`/`ApiConfig`/`ApiClient` печатал `authorization` (токен) | **Закрыт**: ручной `Debug` для `ApiAuth`, `authorization` → `<redacted>`; тест на `ApiConfig` и `ApiClient` |
| P3-7 | P3 | `ApiAuth::pairs` дублирует имена заголовков литералами вместо `headers::AUTH_HEADER_NAMES` | **Backlog** по решению координатора; контракт не затронут |

## 3. Проверка закрытия нитов

- Код: `client.rs:350-356` (`Connect | Io | Timeout`), `client.rs:200-208` (fallthrough
  `Protocol` → `Transport`), `client.rs:37-49` (маскирующий `Debug`).
- Новые тесты: `transport_read_timeout_is_retried_for_get` (read_timeout 40 мс, второй стаб
  отвечает 200), `transport_read_timeout_is_not_retried_for_post`,
  `protocol_error_maps_to_transport_without_retry`, `debug_redacts_authorization`.
- Red-final (финальный текст тестов, временный откат трёх код-правок):
  `13 passed; 3 failed`, `# EXIT=101` (`waves/RW-12/reports/i1-nits-red-final.log`;
  red-копия `i1-nits-test.red.rs`, md5 `04255b291783fa7edfde0167aa1de708`).
  Первый red `i1-nits-red.log` отличался только реструктуризацией helper'а (ассерты те же).
- Green-final: `16 passed; 0 failed`, `# EXIT=0`
  (`waves/RW-12/reports/i1-nits-green-final.log`); 10/10 повторов стабильно.
- md5: `client.rs 29f88872d9c03d6190ea776bfd6dfd10`,
  `tests/rest_client.rs 04255b291783fa7edfde0167aa1de708`.
- fmt / clippy: `EXIT=0`; полный `discord-core` — 235 passed, 0 failed.
- P3-3/P3-4/P3-5 — test-only усиления: в red-final они проходят, red-first для них не
  показан (подтверждены инспекцией кода/тестов).

## 4. Контракт §1–§2 (подтверждено)

- Публичные типы/сигнатуры §1 совпадают; `max_attempts=0 → 1`.
- §2.1: `guard_target` до `Endpoint::target` и до сети.
- §2.2: 6 auth-заголовков в порядке `headers::AUTH_HEADER_NAMES`, дедупликация без учёта
  регистра, `Content-Type` только для непустого тела, `Content-Length` из `http1`.
- §2.3: `attempt_timeout` вокруг каждой попытки.
- §2.4: 429 любой метод с `min(retry_after, max_retry_after)`; 5xx только GET/PUT/DELETE,
  экспонента с `checked_shl`/`saturating_mul`; 401/4xx без повтора; исчерпание → правильные
  варианты `RateLimited`/`Server`.
- §2.5: внешний таймаут повторяется только для идемпотентных; `Connect`/`Io`/`Timeout` —
  для идемпотентных; `Tls`/`TooLarge`/`Protocol` → `Transport`.
- §2.6: каждый исход имеет вариант `ApiError`; `Display`/`Error` реализованы.
- Паник/unwrap на входных данных нет; скрытой сети в тестах нет (только
  `127.0.0.1`; `https:///api`/`https://example.com:abc/api` — строки без DNS).

## 5. Не проверено

- Транспортные таймауты/TLS/TooLarge/Protocol через фасад — отдельных тестов в §3 не было;
  после нитов Protocol/Timeout покрыты, TLS/TooLarge — только чтением кода.
- `max_retry_after` как реальный cap, рост 2×/4× и overflow — только чтение `rate_limit.rs`.
- `gate:worker` не перезапускался (вне набора ревью); живой Discord не трогался.
- `ApiClient` пока не подключён к P1-07/09/11 (интеграционная дорожка).
