# ТЗ RW-11 — P1-09 / P1-11 / P1-12 (три независимых куска одновременно)

**Метод:** §3.13. Контракты заморожены каркасами с `todo!()`. Карточки не зависят от
транспорта RW-10: чистые функции, интеграция — следующей волной.

## 0. Общие правила

- Файлы — только свои; общие (`rest/mod.rs`, `Cargo.toml`, `lib.rs`) правит координатор.
- Публичные сигнатуры каркаса не менять. Red-first: тест → прогон с `# EXIT=...` → копия теста
  в `waves/RW-11/reports/<id>-test.red.rs` → минимальная реализация → green.
- Новых зависимостей и сети нет. Не трогать golden/`/home/sanya/re-deep`. Без commit/push.
- `npm run gate:worker` не запускать. Отчёт §4.2 → `waves/RW-11/reports/<id>.md`.

| Воркер | Карточка | Файлы (только эти) |
|---|---|---|
| W11a | P1-09 | `rust/crates/discord-core/src/rest/rate_limit.rs`, `tests/rest_rate_limit.rs` |
| W11b | P1-11 | `rust/crates/discord-core/src/rest/telemetry.rs`, `tests/rest_telemetry.rs` |
| W11c | P1-12 | `rust/crates/discord-core/src/rest/ack.rs`, `tests/rest_ack.rs` |

## 1. W11a — rate-limit/ошибки (T-REST-05/06)

Оракул: `docs/re/rest-d02.md` §3 — единственный 429 (`GET /users/@me/survey`), коды
200/204/304/404/429; тела/поля ответов в захвате отсутствуют (§5), поэтому семантика —
RFC 6585 §4 + документированные Discord-поля. Заголовки в тестах синтетические, это
зафиксировано в ТЗ.

Кейсы:

1. 429 + `retry-after: 1` → `RateLimited{retry_after: 1s, global:false}`.
2. 429 без заголовка, тело `{"message":"...","retry_after":0.25,"global":true}` → 250 мс,
   `global:true`.
3. Приоритет: `retry-after: 2` + тело `0.5` → 2 с.
4. Fallback `x-ratelimit-reset-after: 1.5`; `x-ratelimit-remaining: 4`; `x-ratelimit-bucket: abc`
   → поля разобраны.
5. Битые значения (`retry-after: abc`, `retry_after:-1`, `NaN`) не паникуют; для 429
   `retry_after` = 0, если валидного источника нет.
6. Не-429 → `None`; 401 → `Unauthorized`; 500/503 → `Server`; 404 → `Client`; 200/302 → `None`.
7. `retry_delay`: 429 → `retry_after`, но не больше `max`; 5xx → `base`, `base*2`, `base*4`
   для attempt 1/2/3, cap `max`; `attempt=u32::MAX` не переполняется; 401/4xx → `None`.
8. `should_retry`: 429 — GET и POST true; 5xx — GET/PUT/DELETE true, POST/PATCH false;
   401/404 false.

## 2. W11b — телеметрия off (T-SEC-04)

Оракул: `docs/re/rest-d02.md` §3/§7 — `/science` (88 запросов), `/beaker` (1), `/track` не
наблюдался; политика — не отправлять.

Кейсы:

1. Блокируются ровно первые сегменты: `/science`, `/science?x=1`, `/science/events`, `/beaker`,
   `/track`, `/beaker?x=1`.
2. Не блокируются: `/scientific`, `/users/@me/science`, `/tracks`, `/`, `science` (без слэша),
   `/science%2Ffoo`.
3. `guard_target` для заблокированного → `Err(Blocked)`, для разрешённого → `Ok` с тем же
   срезом (проверить указатель/равенство).
4. Пустой target → `Ok("")`.

## 3. W11c — ack/read-state (T-REST-07)

Оракул: `docs/re/rest-d02.md` §4 — `POST /channels/{id}/messages/{id}/ack` ×5, тело
`{"token":null,"last_viewed":4310}` (requestId `ready|57992.7577`).

Кейсы:

1. `build_ack_target("123","456")` → `/channels/123/messages/456/ack`.
2. Пустой id → `EmptyId`; `12a`, `../x`, `+1`, ` 1` → `NonNumericId`.
3. Тело без токена: `{"token":null,"last_viewed":4310}` — точная строка (порядок ключей из
   оракула).
4. Токен `"abc"` → `{"token":"abc","last_viewed":1}`.
5. Экранирование: `a"b\c\n` → `{"token":"a\"b\\c\u000a","last_viewed":1}`; control-символы
   `\u0000`…`\u001f` — `\u00XX` в нижнем регистре; UTF-8 `привет` проходит без изменений.
6. `last_viewed = u64::MAX` → без потерь.

## 4. Приёмка

Полный `cargo test --offline`, clippy/fmt; независимое ревью; при необходимости — ниты red-first.
