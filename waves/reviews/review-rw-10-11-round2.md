# Повторное независимое ревью RW-10 / RW-11 (2026-10-20)

**Ревьюер:** независимый субагент (не автор кода). Режим: только чтение исходников, тестов,
логов и ТЗ; живой Discord не трогал; сеть — только loopback-стабы тестов; cargo писал только
в `target/`.

**Снимок:** рабочее дерево dirty (RW-10/RW-11 + параллельные RW-12/`ready_*`). Проверяемые
md5 стабильны в течение ревью.

## 1. Вердикты

- **RW-10 — принято с нитами.** P0/P1/P2 новых нет. P1-1, P2-1, P2-2 закрыты по существу:
  wire-лимит chunked, строгий `is_chunked`, запрет TE в `encode_request`,
  DNS/connect/read/write/TLS-таймауты с `HttpError::Timeout`.
  Открыты P3: N1–N9 (ниже). Блокеров приёмки нет; N1–N3 желательно закрыть до живой
  REST-интеграции.
- **RW-11 — принято с нитами.** P0/P1/P2 нет. Открыты P3-3 (глубина JSON-сканера/`truex`),
  P3-4 (нормализация telemetry-пути, контракт заморожен), P3-6 (ссылки на D-02);
  P3-1/P3-2/P3-5 — статус ниже. `telemetry.rs` имеет doc-only дрейф md5 относительно отчёта
  (поведение не менялось).

## 2. Статус P3 прошлого ревью

| ID | Статус | Комментарий |
|---|---|---|
| P3-1 | **Открыт** | Теста с явными `Host`/`Connection` нет; `get_plain_tcp_with_auto_headers` (`tests/rest_transport.rs:178-214`) проверяет только автозаголовки. Код `has_header` корректен. |
| P3-2 | **Закрыт по коду; тесты неполны** | `+5`/пробелы отвергаются (`http1.rs:248-257`), трейлеры валидируются (`http1.rs:329-343`), расширения ограничены длиной строки, но не валидируются синтаксически (для framing допустимо). Тестов на `+5`, невалидный трейлер и per-line trailer limit нет. |
| P3-3 | **Открыт** | `json_value_start` без глубины, `json_bool` проверяет только префикс `true`/`false` (`rate_limit.rs:74-115, 129-139`); `{"error":{"retry_after":5}}` и `"global":truex` не покрыты. |
| P3-4 | **Открыт** | `first_path_segment` сравнивает сырой первый сегмент (`telemetry.rs:24-35`): `/./science`, `/foo/../science`, `//science`, `/%73cience` проходят. В RW-12 `guard_target` вызывается на сыром target (`client.rs:232`) — инвариант «никакой нормализации ниже guard» сохранён. |
| P3-5 | **Открыт** | `httparse` объявлен (`rust/Cargo.toml:28`, `crates/discord-core/Cargo.toml:14`), но не используется: `rg httparse` находит только Cargo-файлы/lock. |
| P3-6 | **Частично** | `todo!()` из `telemetry.rs:3-4` убран (doc-only), но ссылки на D-02 §3 остались в `http1.rs`, `transport.rs`, `rate_limit.rs`, `telemetry.rs`, тестах RW-10/RW-11 и TZ RW-11:22,45. В D-02 нет `HTTP/1.1`/`Content-Length`; 429 — §2, ack — §4, science/beaker — §2/§7. |
| P3-7 | **Принят с оговоркой** | `nits-red-final` — легитимный mutation-red на финальных тестах, но не первый red-first; не покрывает strict HEX, валидацию трейлеров и wire-limit (для них нет тестов). `nits-red.log` снят на промежуточной редакции теста (строки 232/328 vs финальные 219/333); в отчёте это не оговорено. |

## 3. Новые находки

| ID | Severity | Файл:строка | Суть | Доказательство | Предлагаемая правка |
|---|---|---|---|---|---|
| N1 | **P3** | `http1.rs:126-131` | Пустой `Transfer-Encoding` + `Content-Length` даёт `Ok(false)`, а не `ConflictingFraming`: проверка пустых токенов стоит до проверки CL. | Чтение кода; `chunked_flag_rules` покрывает только `chunked+CL` и `gzip+CL`; кейса `TE: ""` нет. | Определять наличие TE по заголовку, а не по непустым токенам; пустой TE — `UnsupportedTransferEncoding`; unit-кейс. |
| N2 | **P3** | `transport.rs:405-429`; `http1.rs:306-309` | `BodyTooLarge` из chunked приходит как `HttpError::Protocol(BodyTooLarge)`, а CL/wire — как `HttpError::TooLarge`. | Код; `too_large_body_is_rejected_before_read` проверяет только CL. | Маппить `Http1Error::BodyTooLarge` → `HttpError::TooLarge`; транспортный кейс chunked > `max_body`. |
| N3 | **P3** | `transport.rs:390-392, 424-426, 439-443` | EOF/обрыв тела отдаётся как `Protocol(Incomplete)` или `Protocol(MalformedHead("тело короче Content-Length"))`. | Чтение кода; тестов на обрыв нет. | Ввести `HttpError::UnexpectedEof`/`Truncated` (или `Io`), отделить обрыв тела от битого заголовка; тесты и учёт в ретраях. |
| N4 | **P3** | `transport.rs:438, 448` | `consumed + length` без `checked_add`: при `max_body_bytes ≈ usize::MAX` и `Content-Length: usize::MAX` — overflow. Дефолт 8 МиБ безопасен; config-extreme. | Код; `content_length` может вернуть `usize::MAX`. | `end = consumed.checked_add(length)`; ограничить `max_body_bytes`; тест с `usize::MAX`. |
| N5 | **P3** | `transport.rs:405-429`; `http1.rs:224-327` | На каждом `read` `decode_chunked` заново разбирает весь буфер и собирает тело: O((wire/read)²) CPU при потоке мелких чанков. | Чтение кода; бенчмарка нет. | Инкрементальный декодер либо лимит работы; минимум — документировать. |
| N6 | **P3** | `tests/rest_http1.rs:295-356`; `tests/rest_transport.rs:273-300` | Пробелы покрытия: нет `+5`/` 5`/`5 `, невалидного трейлера, длинного расширения (текущий «oversized size line» — 1025 нулей — ловится 16-значным капом, а не `MAX_CHUNK_LINE_BYTES`), per-line trailer limit, wire-limit, TE+CL на транспорте, обрыва тела, write/TLS/DNS/connect-таймаутов. | Чтение тестов. | Добавить перечисленные кейсы. |
| N7 | **P3** | `tests/rest_transport.rs:279, 287` | Read-timeout тест: сервер спит 200 мс, клиент таймаутит 40 мс; при задержке рантайма >200 мс ответ уже в буфере → флейк. | 3 прогона зелёные, флейка не наблюдал. | Спать 1–2 с (в пределах `CASE_TIMEOUT` 5 с) или не отвечать вовсе. |
| N8 | **P3** | `waves/RW-11/report.md:27-29`; `telemetry.rs:3-4` | В отчёте md5 `telemetry.rs` = `641218af…`, текущий — `c04c2e10…`. Diff — только доккомментарий (убран `todo!()`). | `md5sum`; diff с копией. | Обновить md5/отчёт или зафиксировать doc-only изменение при приёмке. |
| N9 | **P3** | `waves/RW-10/TZ.md:48-49` | ТЗ требует `gzip, chunked` → true и `gzip` → false; реализация/тесты после P2-1 отвергают оба (`UnsupportedTransferEncoding`). | TZ vs `http1.rs:132-135`, `tests/rest_http1.rs:219-228`. | Обновить ТЗ решением ревью (только одиночный `chunked`) или зафиксировать deviation. |

**P0/P1/P2 новых находок нет.**

## 4. Легитимность red/green RW-10

- `nits-red-final.log`: `rest_http1` — 16 tests, 13 passed / 3 failed, `# EXIT=101`
  (`chunked_flag_rules`, `encode_request_rejects_transfer_encoding`,
  `decode_chunked_rejects_oversized_size_line_and_trailers`); `rest_transport` — 14 tests,
  13 passed / 1 failed (`read_timeout_is_enforced`), `# EXIT=101`.
- Строки паник соответствуют текущей редакции тестов; хеши тестовых бинарей совпали с
  независимым прогоном ревьюера.
- `nits-green-final.log`: 16/16 + 14/14, `# EXIT=0`.
- md5 совпадают: `http1.rs 6bedfcf4…`, `transport.rs d8774d4e…`; тесты `c8b0b7e5…`,
  `b154833b…`.
- Восстановление подтверждено: в `/tmp/opencode/nits-backup/` копии с теми же md5; таймлайн
  бэкап → red → восстановление → green. Это mutation-red временным откатом, а не первый
  red-first; отчёт §7 это раскрывает.

**Оговорка:** red-final показывает ровно 4 заявленных падения на финальной редакции тестов,
но size-line limit не откатывался (его red — более ранний `nits-red.log` на промежуточной
редакции), strict HEX/трейлеры/wire-limit тестами не покрыты, а «oversized size line»
(1025 нулей) прошёл бы и без line-limit за счёт 16-значного капа.

## 5. Проверки

| Проверка | Результат |
|---|---|
| md5 src/тестов | `http1 6bed…`, `transport d877…`, `rest_http1 c8b0…`, `rest_transport b154…`, `rate_limit e9f0…`, `telemetry c04c…`, `ack 0d55…` |
| RW-10/11 тесты | 64 passed / 0 failed, `EXIT=0` (16+14+10+11+4+9; RW-10/11 — 53) |
| Полный core | 235 passed / 0 failed, `EXIT=0` на повторном прогоне (счётчик плавал из-за параллельных RW-12) |
| fmt / clippy | `fmt --check` EXIT=0; `clippy -D warnings` EXIT=0 |
| TLS | SAN `localhost,127.0.0.1`, CA/leaf verify OK; `with_root_certificates` + `ServerName`, без `dangerous()` |
| Оракулы D-02 | `HTTP/1.1`/`Content-Length` в D-02 отсутствуют; 429 — §2, ack — §4, science/beaker — §2/§7 |
| Гейт | `.gate/worker-2026-10-20T08-25-42-009Z.json` → passed, dirty |

## 6. Не проверено

- Mutation-red независимо не воспроизводился (read-only режим).
- DNS/connect/write/TLS-handshake таймауты не исполнялись — тестов нет.
- Живой Discord и внешняя сеть не использовались.
- `client.rs`/retry-классификация и `ready_*` — вне области; кросс-волновые N2/N3 должен
  закрыть ревью RW-12.
- Полный workspace и `gate:worker` не перезапускались.

## 7. Итог

RW-10 и RW-11 принимаются **с нитами**: P0/P1/P2-блокеров нет, P1/P2-ниты прошлого ревью
закрыты по существу. Перед живой REST-интеграцией рекомендуются N1–N3 и N6; N4/N5/N7 —
в очередь нитов; N8/N9 — привести отчёт/ТЗ в соответствие с текущими md5 и strict-контрактом.
