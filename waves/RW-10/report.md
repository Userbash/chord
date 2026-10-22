# Отчёт волны RW-10 — HTTP/1.1-транспорт REST (вариант A)

- **Дата:** 2026-10-20. **Карточка:** инфраструктура P1-07/P1-09 (живой REST).
- **Решение:** вариант A — ручной HTTP/1.1 на `tokio` + `tokio-rustls`; прямые зависимости
  `tokio-rustls 0.26` и `httparse 1` уже в `Cargo.lock`/офлайн-кэше, сетевых загрузок нет.
- **Состав:** W10a — чистый фрейминг (`http1`), W10b — TCP/TLS (`transport`); куски
  независимы, запускались одновременно (§3.13).

## 1. Результат

- **30 тестов после нитов:** `rest_http1` 16/16, `rest_transport` 14/14 (на приёмке волны было
  27: 14+13).
- `cargo test --offline -p discord-core` — **166 passed, 0 failed** (на момент приёмки волны;
  после I1 — 178, после нитов и P-1 — 192); воркспейс — **208/208** (после нитов — 234).
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — чисто;
  `cargo fmt --all -- --check` — чисто.
- `gate:worker` — **passed**, `.gate/worker-2026-10-20T07-01-09-367Z.json`.
- Новых сетевых вызовов в тестах нет: только `127.0.0.1`, каждый кейс под таймаутом.

## 2. Red / green

| Кусок | Red | Green |
|---|---|---|
| W10a | `6 passed; 8 failed` (`w10a-red.log`), затем после усиления точных кейсов `12 passed; 2 failed` (`w10a-red2.log`), `# EXIT=101` | `14 passed; 0 failed`, `# EXIT=0` (`w10a-green.log`) |
| W10b | исходный каркас `1 passed; 12 failed` (`w10b-red.log`); после исправления тестовой обвязки mutation-red на финальном тесте `4 passed; 9 failed` (`w10b-red-final.log`), `# EXIT=101` | `13 passed; 0 failed`, `# EXIT=0` (`w10b-green.log`) |

На момент приёмки волны red-копии совпадали с финальными тестами:
`w10a-test.red.rs` = `tests/rest_http1.rs` = `1e51f01e6b84bfec34d60155c967b690`;
`w10b-test.red.rs` = `tests/rest_transport.rs` = `f243bcf757aa20be4f0aac11f8adbcaa`.
После нитов тесты изменились: текущие md5 — `c8b0b7e5…` / `b154833b…` (см. §7); полный red
на финальной редакции не переснимался — процессный долг.
Исходники: `http1.rs` = `de8abac9ac8b401d56cc1c2a65707fd7`,
`transport.rs` = `f51ca0a995e8a6ee08b8c660299ed24a`.

## 3. Что реализовано

- **HTTP/1.1:** строка запроса, поля, автоматический `Content-Length`, точные байты;
  запрет CR/LF-инъекций, пробелов/двоеточий в имени, не-ASCII/фрагментов в target;
  разбор статуса/полей (HTTP/1.0/1.1), лимит заголовка, запрет obs-fold; `Content-Length`
  (формат/повторы), `Transfer-Encoding: chunked` и конфликт CL+TE; chunked с расширениями,
  трейлерами, лимитом и `Incomplete` на любом префиксе.
- **Транспорт:** `Endpoint` (IPv4/IPv6, порт, base_path, нормализация `/`), target,
  DNS-перебор, TCP connect timeout, TLS с проверкой цепочки/SAN, таймаут рукопожатия,
  `Host`/`Connection: close`, чтение по CL/chunked/until-close, лимит тела, ошибки без паник.
- **Фикстуры TLS:** CA `rest-test-ca.der` (`0a2452d2…`), leaf `rest-test-cert.der`
  (`1ede6c52…`, SAN `DNS:localhost, IP:127.0.0.1`), ключ `rest-test-key.der` (`a7ce1e57…`).
  Первый self-signed cert с `CA:TRUE` rustls отверг как end-entity — заменён цепочкой CA→leaf.

## 4. Файлы

`rust/crates/discord-core/src/rest/http1.rs`, `transport.rs`, `rest/mod.rs`;
`tests/rest_http1.rs`, `tests/rest_transport.rs`; `tests/data/rest-test-{ca,cert,key}.der`;
`waves/RW-10/{TZ.md,report.md,reports/*}`; `rust/Cargo.toml`, `discord-core/Cargo.toml`,
`rust/Cargo.lock`.

## 5. Риски и ограничения

- Только HTTP/1.1, без keep-alive/пула/HTTP2/прокси; каждый запрос — новое TCP/TLS.
- Нет сжатия: клиент не просит `Accept-Encoding`; тела Discord в захвате небольшие.
- `1xx` не обрабатываются отдельно; `HEAD` не используется (нет маршрута).
- Read/write/DNS-таймауты добавлены нитом (см. §7); `ApiClient::attempt_timeout` остаётся
  внешней границей попытки.
- `webpki-roots` — снимок крейта; обновление корней идёт вместе с зависимостью.
- Коллизия двух процессов W10a (11:49–12:03) перезаписала первые red-артефакты; финальное
  состояние объединено, снимок второго процесса — `/tmp/opencode/w10a-concurrent/`,
  подробности — в `reports/w10a.md`.

## 6. Ревью

Независимый ревьюер запущен отдельным агентом; вердикт и находки — в
`waves/reviews/review-rw-10-11.md` (дополняется после завершения).


## 7. Ниты независимого ревью (2026-10-20)

Закрыты P1/P2-находки `waves/reviews/review-rw-10-11.md`:

- **P1 (лимиты chunked):** `decode_chunked` ограничил строку размера (`MAX_CHUNK_LINE_BYTES`
  = 1024), суммарные трейлеры (`MAX_TRAILER_BYTES` = 8 КиБ), запретил `+5` (только `1*HEXDIG`),
  валидирует трейлеры как поля; `exchange` проверяет wire-лимит
  `max_body*8 + 64 КиБ` до чтения.
- **P2 (framing/TE+CL/таймауты):** любое `Transfer-Encoding` вместе с `Content-Length` —
  `ConflictingFraming`; поддерживается только одиночный `chunked`, остальное —
  `UnsupportedTransferEncoding`; `encode_request` отвергает TE. В `HttpClientConfig` добавлены
  `read_timeout`/`write_timeout`; `lookup_host`, запись/flush и каждое чтение обёрнуты в
  `tokio::time::timeout`; добавлен `HttpError::Timeout`.

**Red (полный, финальная редакция тестов):** временный откат нитов (lenient `is_chunked`,
TE разрешён в запросе, лимиты трейлеров сняты, `read_timeout` игнорируется) →
`rest_http1` `13 passed; 3 failed`, `rest_transport` `13 passed; 1 failed`, `# EXIT=101`
(`reports/nits-red-final.log`). После восстановления md5 совпали с зафиксированными (§7),
green — `16+14 passed`, `# EXIT=0` (`reports/nits-green-final.log`). Первый частичный red —
`reports/nits-red.log` (только `rest_http1`).

**Green:** `rest_http1` — 16/16, `rest_transport` — 14/14; `cargo test --offline` — 232/232
(ядро 190/190) на момент нитов; после P-1 — 234/234 (ядро 192/192); fmt/clippy — чисто.

**md5 после нитов:** `http1.rs` — `6bedfcf45814b7d8236854cc82d0f6ee`,
`transport.rs` — `d8774d4ecdcea5e36355247cd42c863e`,
`tests/rest_http1.rs` — `c8b0b7e597d9df32894f205a1809ab9f`,
`tests/rest_transport.rs` — `b154833bf5a7034a7921b25677005b70`.

**Осталось (P3, очередь):** тест явных `Host`/`Connection`; строгость chunk-расширений;
JSON-сканер rate-limit; нормализация telemetry-пути; мёртвый `httparse`; неточные ссылки на
D-02 §2/§4/§7; процессные правила "один писатель — один файл".

## 8. Повторное независимое ревью (2026-10-20)

Вердикт: **«принято с нитами»**, P0/P1/P2 новых нет. P1-1/P2-1/P2-2 подтверждены;
mutation-red `nits-red-final` проверен по логам, md5 и хешам тестовых бинарей (4 падения:
3 в `rest_http1` + `read_timeout_is_enforced`; EXIT=101). Открыты P3 N1–N9: пустой TE+CL,
варианты ошибок chunked/обрыва тела, overflow `max_body`, O(n²) re-decode, пробелы покрытия,
флейк read-timeout, doc-md5 `telemetry.rs`, strict-TE в ТЗ (ТЗ обновлено).
Полный отчёт: `waves/reviews/review-rw-10-11-round2.md`.
