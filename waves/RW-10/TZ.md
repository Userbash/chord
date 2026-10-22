# ТЗ RW-10 — HTTP/1.1-транспорт REST (вариант A, решение владельца 2026-10-20)

**Карточка:** инфраструктура к P1-07/P1-09 (живой REST). **Метод:** §3.13 — два независимых
куска одновременно, контракт заморожен каркасом с `todo!()`.

## 0. Общие правила (оба воркера)

- Файлы — **только свои** из таблицы ниже. Общие файлы (`Cargo.toml`, `rest/mod.rs`, `lib.rs`,
  фикстуры, доки) правит координатор.
- Публичные типы/сигнатуры в каркасе **не менять** (тела `todo!()` заменяются реализацией).
- Red-first: сначала тест-файл, прогон и падение зафиксировать (`# EXIT=...`), копию теста
  сохранить в `waves/RW-10/reports/<id>-test.red.rs` **до** реализации. Потом минимальный код.
- Оракул — RFC 9110/9112 и фикстуры/оракулы из ТЗ, **не** подгонка под реализацию.
- Сеть — только `127.0.0.1`, каждый кейс под `tokio::time::timeout(5 c)`. Новых зависимостей нет.
- Не трогать `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep`.
- Не делать commit/push. `npm run gate:worker` не запускать — финальный гейт у координатора.
- Отчёт по PROCESS.md §4.2 → `waves/RW-10/reports/<id>.md`.

| Воркер | Файлы (только эти) |
|---|---|
| W10a | `rust/crates/discord-core/src/rest/http1.rs`, `rust/crates/discord-core/tests/rest_http1.rs` |
| W10b | `rust/crates/discord-core/src/rest/transport.rs`, `rust/crates/discord-core/tests/rest_transport.rs` |

Каркас: `rest/http1.rs`, `rest/transport.rs` — типы и доккомментарии заморожены.

## 1. W10a — чистый HTTP/1.1-фрейминг

**Кейсы (red-first):**

1. `encode_request_get`: GET `/users/@me`, поля `Host`, `Authorization`, пустое тело → точные
   байты `GET /users/@me HTTP/1.1\r\n...\r\n\r\n`, без `Content-Length`.
2. `encode_request_post_adds_length`: тело `{"a":12}` (8 Б) → `Content-Length: 8` добавлен
   после переданных полей; точные байты.
3. `encode_request_rejects_injection`: CR/LF в target/значении, пробел/двоеточие в имени →
   `InvalidHeader`; невалидный target без `/` → `InvalidHeader`.
4. `encode_request_checks_content_length`: передан `Content-Length: 1`, тело 2 Б →
   `InvalidContentLength`.
5. `parse_head_200`: полный ответ с `Content-Type` и `Content-Length: 2` → `Complete{200,...}`,
   `consumed` = длина заголовка с `\r\n\r\n`.
6. `parse_head_incomplete_and_limit`: префикс → `Incomplete`; буфер без `\r\n\r\n` длиннее
   `MAX_HEAD_BYTES` → `HeadTooLarge`.
7. `parse_head_versions_and_malformed`: HTTP/1.0 ok; HTTP/2 → `UnsupportedVersion`; строка без
   кода/мусор → `MalformedHead`; заголовок без `:` → `MalformedHead`; obs-fold (строка с
   ведущим пробелом) → `MalformedHead`.
8. `header_case_insensitive`: `Retry-After` находится по `retry-after`; отсутствующее → `None`.
9. `content_length_rules`: нет поля → `None`; `5` → `Some(5)`; два одинаковых `5` → `Some(5)`;
   разные → `InvalidContentLength`; `abc`/`-1` → `InvalidContentLength`.
10. `chunked_flag_rules` (обновлено решением ревью 2026-10-20, nit P2-1): `Transfer-Encoding:
    chunked` → true; `gzip, chunked` / `gzip` / любой не одиночный `chunked` →
    `UnsupportedTransferEncoding`; `chunked` + `Content-Length` → `ConflictingFraming`.
    `Transfer-Encoding` в запросе запрещён (`encode_request`).
11. `decode_chunked_rfc_example`: `4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n` → `Wikipedia`,
    `consumed` = весь буфер (пример RFC 9112 §7.1 — независимый оракул).
12. `decode_chunked_extensions_trailers`: `5;foo=bar\r\nhello\r\n0\r\nX-T: v\r\n\r\n` → `hello`.
13. `decode_chunked_incomplete_and_errors`: любой префикс валидного буфера → `Incomplete` или
    `Complete` с тем же телом (property по всем точкам разреза); битый размер/нет CRLF →
    `MalformedChunk`; тело больше `max_body` → `BodyTooLarge`; пустой вход → `Incomplete`.

**DoD:** red-лог и red-копия; все кейсы зелёные; `cargo test --offline -p discord-core --test
rest_http1`; `cargo clippy --offline -p discord-core --all-targets -- -D warnings`; `cargo fmt
--check`; отчёт.

## 2. W10b — TCP/TLS-транспорт

**Фикстуры (координатор, read-only):** цепочка CA → leaf (заменена 2026-10-20: первый
self-signed cert был с `CA:TRUE` и rustls отвергал его как end-entity).
- `tests/data/rest-test-ca.der` — корень (md5 `0a2452d28227a8d08cb7306a99a7cd84`,
  `basicConstraints=critical,CA:TRUE`);
- `tests/data/rest-test-cert.der` — серверный leaf (md5 `1ede6c5282c288d70d8b23f6ed080784`,
  SAN `DNS:localhost, IP:127.0.0.1`, `CA:FALSE`, EKU serverAuth);
- `tests/data/rest-test-key.der` — ключ leaf PKCS#8 DER (md5 `a7ce1e5779d8e217158daa7ab805c2ce`).
Серверная сторона — `tokio_rustls::TlsAcceptor` + `rustls::ServerConfig` (leaf+key);
клиент доверяет только `rest-test-ca.der`.

**Кейсы (red-first):**

1. `endpoint_parse`: `https://discord.com/api/v9` → scheme `https`, host `discord.com`, port 443,
   base_path `/api/v9`; `target("/users/@me")` → `/api/v9/users/@me`.
2. `endpoint_parse_local`: `http://127.0.0.1:PORT` → port из URL, base_path пуст; target `/x` →
   `/x`; base с хвостовым `/` нормализуется.
3. `endpoint_rejects`: `ftp://`, пустой хост, кривой порт, query/fragment в базе →
   `InvalidBase`/`UnsupportedScheme`; target без `/` или с CR/LF → `InvalidTarget`.
4. `get_plain_tcp`: стаб `127.0.0.1:0` принимает соединение, независимо читает запрос, проверяет
   строку `GET /api/v9/users/@me HTTP/1.1`, `Host`, `Connection: close`, отсутствие тела; отвечает
   `Content-Length: 2` + `{}`; клиент отдаёт 200/`{}`/поля.
5. `post_body`: стаб проверяет `Content-Length` и точное тело; ответ 204 без тела → тело пусто.
6. `chunked_response`: ответ chunked (`4 Wiki...`) → декодированное тело.
7. `read_until_close`: ответ без `Content-Length`, `Connection: close`, тело и закрытие → тело
   прочитано.
8. `too_large`: `Content-Length` больше `max_body_bytes` → `TooLarge`.
9. `malformed_response`: мусор вместо статуса → `Protocol`.
10. `tls_custom_root`: TLS-стаб с leaf+key, клиент с `RootCertStore` из `rest-test-ca.der`,
    база `https://127.0.0.1:PORT` (SAN IP) → 200 и тело.
11. `tls_untrusted_root`: тот же стаб, клиент с webpki-корнями → `Tls` (не паника).
12. `connect_refused`: закрытый порт → `Connect`/`Io`, не паника.
13. `auto_headers`: без `Host`/`Connection` клиент добавляет `Host: host[:port]` и
    `Connection: close`; с явными — не дублирует.

Каждый сетевой кейс — под `timeout(5 c)`.

**DoD:** red-лог и red-копия; зелёный `cargo test --offline -p discord-core --test
rest_transport`; остальные core-тесты; clippy `-D warnings`; fmt; отчёт.

## 3. Приёмка волны (координатор)

- Границы файлов, red-копии, независимые оракулы, отсутствие молчаливых ошибок.
- Полный `cargo test --offline`, clippy, fmt; `npm run gate:worker`; ревью отдельным агентом;
  не-поведенческий рефакторинг — отдельным воркером после зелёного.
