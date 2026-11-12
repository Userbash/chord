# Technical Review Report (Review 82A) — PR-53 (task-169 / BE-P2-03)

- **Задача**: task-169 / BE-P2-03 (HTTP/1.1 User-Agent & Content-Encoding gzip/deflate decompression in `discord-core::rest`)
- **Ветка**: `feature/task-169-rest-http1-ua-gzip`
- **Пулл-реквест**: `PR-53`
- **Ревьюер**: Technical Reviewer (Review 82A)
- **Дата**: 2026-10-31
- **Вердикт**: **APPROVED (Ready for Merge)** (`P0 = 0`, `P1 = 0`, Mutations: `5/5 KILLED`)

---

## 1. Проверка скоупа и затронутых файлов

Строго соблюдены правила §1.2 AGENTS.md:
Изменены **только** назначенные в ТЗ файлы:
- `rust/crates/discord-core/src/rest/http1.rs` (MD5: `beda8919e02f9a63b422c1f7d8c8c028`)
- `rust/crates/discord-core/src/rest/transport.rs` (MD5: `1758264133da09263d61d5b82557c812`)
- `rust/crates/discord-core/tests/rest_http1.rs` (MD5: `71d48a8a659a32accfa5ade07789e8bd`)

Никаких посторонних изменений, зависимостей в `Cargo.toml` или других крейтов не затронуто.

---

## 2. Результаты статического анализа и тестов

1. **Targeted cargo test**:
   `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_http1 --test rest_transport --lib`
   - `lib.rs`: 33 passed, 0 failed.
   - `tests/rest_http1.rs`: 27 passed, 0 failed.
   - `tests/rest_transport.rs`: 14 passed, 0 failed.
   - `tests/rest_client.rs`: 26 passed, 0 failed.
   - **Суммарно**: 100/100 passed (`0 failed`, `0 flakes`).

2. **Cargo clippy**:
   `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_http1 --lib -- -D warnings`
   - Статус: Clean (0 warnings, 0 errors).

---

## 3. Верификация мутационного тестирования (5/5 KILLED)

Проверена устойчивость тестов к пяти контрольным мутациям:

1. **Mutation 1 (DEFAULT_USER_AGENT mismatch)**:
   - *Описание мутации*: изменение константы `DEFAULT_USER_AGENT` на любое иное значение / опечатку в номере версии или движке.
   - *Убийство*: тест `default_user_agent_constant_matches_oracle` напрямую сверяет константу с эталоном оракула (`rest-d02.json` -> `Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36`). Тест `transport_auto_user_agent_and_gzip_response` сверяет заголовок на сокете.
   - *Статус*: **KILLED**.

2. **Mutation 2 (Overriding custom User-Agent)**:
   - *Описание мутации*: безусловная перезапись `User-Agent` дефолтным значением даже если вызывающий передал собственный заголовок.
   - *Убийство*: тест `transport_preserves_custom_user_agent` передает кастомный `User-Agent: CustomBot/1.0` и явно проверяет, что он сохранен, а `DEFAULT_USER_AGENT` отсутствует в заголовках запроса.
   - *Статус*: **KILLED**.

3. **Mutation 3 (Skipping CRC32 verification in gzip)**:
   - *Описание мутации*: пропуск проверки чексуммы CRC32 (или возврат `Ok(())` без сопоставления с полем трейлера gzip).
   - *Убийство*: тест `decode_response_body_corrupted_gzip_rejected` проверяет поток gzip с валидными данными, но испорченным CRC32 (`0x00000000`) и ожидает `Err(Http1Error::MalformedBody(_))`.
   - *Статус*: **KILLED**.

4. **Mutation 4 (Missing max_body_bytes limit enforcement in gzip decompression)**:
   - *Описание мутации*: игнорирование переданного `max_body` лимита при распаковке блоков gzip/deflate (потенциальная уязвимость к gzip-бомбам).
   - *Убийство*: тест `decode_response_body_enforces_limit` передает 5-байтовое распакованное тело с лимитом `4` байта и требует `Err(Http1Error::BodyTooLarge)`.
   - *Статус*: **KILLED**.

5. **Mutation 5 (Failing to decompress gzip response body in transport::exchange)**:
   - *Описание мутации*: пропуск вызова декомпрессора в `transport::exchange`, возвращающий сжатые сырые байты в `HttpResponse.body`.
   - *Убийство*: тест `transport_auto_user_agent_and_gzip_response` выполняет реальный сетевой обмен по TCP с сервером, отдающим сжатый gzip `Content-Encoding: gzip`, и ассертит `assert_eq!(resp.body, b"hello")`. Если тело не распаковано, тест падает.
   - *Статус*: **KILLED**.

---

## 4. Замечания по архитектуре и коду

- Реализация DEFLATE/GZIP/ZLIB в `http1.rs` является автономной, не требует внешних зависимостей и корректно соблюдает спецификации RFC 1952, RFC 1951, RFC 1950.
- Обработка заголовков регистронезависима (`k.eq_ignore_ascii_case("user-agent")`, `k.eq_ignore_ascii_case("accept-encoding")`).
- `finalize` функция в `transport.rs` единообразно оборачивает все три пути получения ответа (`chunked`, `Content-Length`, `read_to_end`).
- Обработка ошибок сопоставляет `Http1Error::BodyTooLarge` в `HttpError::TooLarge`.

---

## 5. Итог

- Замечания `P0`: 0
- Замечания `P1`: 0
- Замечания `P2`/`P3`: 0
- **Статус**: Одобрено для слияния в `unstable` (`Ready for Merge`).
