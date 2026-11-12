# ТЗ RW-08 — REST-клиент (P1-07 / T-REST-01…03): параллельная волна

**Проект:** discord-lab + Rust-клиент. Корень: `/home/user/orca/project`; cargo-воркспейс: `rust/`.
**Карточка:** P1-07 (`docs/rust-client/PLAN.md`) — «REST-клиент: base v9, 6 заголовков, коды,
ретраи, таймауты»; тесты T-REST-01…03 (`docs/rust-client/TEST-PLAN.md`).
**Дата старта:** 2026-10-19. **Тип:** первый параллельный запуск (PROCESS.md §3.13, пожелания
владельца): 3 воркера одновременно + рефакторинг-воркер + независимый ревьюер.

## 0. Решения волны (координатор)

1. **Новых зависимостей нет.** Волна закрывает DoD на уровне данных и политики:
   каталог маршрутов и заголовки/коды — чистые данные без сети; таймауты/ретраи — политика над
   транспорт-замыканием, проверяется на **локальном стабе** `127.0.0.1` (tokio + `std` из уже
   разрешённых зависимостей). Выбор реального HTTP-транспорта (TLS; `reqwest` или руками на
   tokio/rustls) — **открытый вопрос владельцу**, волну не блокирует: после решения политика
   подключается к реальному клиенту без изменения тестов. `Cargo.toml` / `Cargo.lock` не трогаем.
2. **База v9** — существующая константа `discord_core::API_V9_BASE` (`rust/crates/discord-core/src/lib.rs:44`,
   `"https://discord.com/api/v9"`). Новых констант базы не вводим.
3. **Сеть — только loopback** (`127.0.0.1`, эфемерный порт). К `discord.com` не ходим.
4. **Контракты заморожены до старта** (типы/сигнатуры/тест-данные) — воркеры не блокируют друг
   друга; общие файлы (`mod.rs`, `lib.rs`, `tests/data/*`) правит только координатор.
5. **Оракулы — в репозитории**, без лаборатории: `docs/re/rest-d02.json` + jq-выдержки в
   `rust/crates/discord-core/tests/data/` (провенанс — §3.3).

## 1. Файлы и границы (пересечений нет)

**Каркас координатора — заморожен, воркеры не меняют:**

| Файл | Что внутри |
|---|---|
| `rust/crates/discord-core/src/lib.rs` | добавлен `pub mod rest;` |
| `rust/crates/discord-core/src/rest/mod.rs` | объявления модулей |
| `rust/crates/discord-core/src/rest/routes.rs` | каркас типов/сигнатур (контракт §4.1; данные и логика — W8a) |
| `rust/crates/discord-core/src/rest/headers.rs` | пустой модуль (владелец — W8b) |
| `rust/crates/discord-core/src/rest/retry.rs` | пустой модуль (владелец — W8c) |
| `rust/crates/discord-core/tests/data/rest-routes-47.tsv` | 47 строк; read-only (§3.3) |
| `rust/crates/discord-core/tests/data/rest-crossref-47.tsv` | 47 строк; read-only (§3.3) |
| `rust/crates/discord-core/tests/data/rest-auth-6.tsv` | 6 строк; read-only (§3.3) |

**Воркеры (каждый — только свои два файла):**

| Воркер | Продукт | Тест | Тест-план |
|---|---|---|---|
| W8a | `rust/crates/discord-core/src/rest/routes.rs` | `rust/crates/discord-core/tests/rest_routes.rs` | T-REST-01 |
| W8b | `rust/crates/discord-core/src/rest/headers.rs` | `rust/crates/discord-core/tests/rest_headers.rs` | T-REST-02 |
| W8c | `rust/crates/discord-core/src/rest/retry.rs` | `rust/crates/discord-core/tests/rest_retry.rs` | T-REST-03 |

**Запрещено всем:** `Cargo.toml`/`Cargo.lock`; любые другие `src/`/`tests/`; `docs/`; `fixtures/`;
`TASKS.md`, `PROCESS.md`, `AGENTS.md`, `waves/` (кроме чтения); `logs/`, `functions/`, `evidence/`,
`reference-oracle/`; `git commit/push/reset/checkout`; `cargo clean`; новые зависимости; сеть
дальше loopback. Чужие тест-данные и тесты не редактируются — расхождение = вопрос координатору.

## 2. Общие процедуры воркера (strict TDD)

Рабочая папка: `/home/user/orca/project`; прогоны — `cd rust && cargo test --offline …`.
Логи/артефакты: `/tmp/opencode/rw-08/w8a|w8b|w8c/` (каталоги создать).

1. **Заглушка (red-основа).** В своём продукт-файле — полный контракт §4/§5/§6 с телами-заглушками
   так, чтобы файл компилировался, а тесты падали **ассертами** (не компиляцией). Никаких
   `todo!()`/`unimplemented!()`/`panic!()` в заглушке.
2. **Финальный тест.** Пишешь тест-файл целиком (кейсы своего раздела). Тест читает только
   публичный API `discord_core::rest::…` + тест-данные §3.3; путь к данным —
   `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/<файл>")`. Нет файла → падение
   (не skip): выдержки лежат в репозитории.
3. **Red-прогон и фиксация.**
   ```bash
   cd /home/user/orca/project/rust
   bash -c 'cargo test --offline -p discord-core --test rest_<имя> 2>&1 | tee /tmp/opencode/rw-08/w8X/red.log; echo "# EXIT=${PIPESTATUS[0]}" | tee -a /tmp/opencode/rw-08/w8X/red.log'
   md5sum crates/discord-core/tests/rest_<имя>.rs | tee -a /tmp/opencode/rw-08/w8X/red.log
   cp crates/discord-core/tests/rest_<имя>.rs /tmp/opencode/rw-08/w8X/rest_<имя>-red.rs
   ```
   В `report.md` — точное распределение `N passed / M failed` и `# EXIT=`.
4. **Реализация (green).** Только свой продукт-файл. Тест после red **не менять**; если правка
   теста неизбежна — red повторяется заново (md5 в red-логе должен совпасть с финальным).
5. **Green-прогоны.** Свой таргет **×5** подряд (нестабильность = блокер), затем
   `cargo test --offline -p discord-core` (если падает в чужом файле — подождать/повторить; чужое
   не трогать). `rustfmt crates/discord-core/src/rest/<файл>.rs crates/discord-core/tests/rest_<имя>.rs`;
   `cargo clippy --offline -p discord-core --all-targets -- -D warnings` (чужие замечания — не
   править, отметить).
6. **Отчёт** по PROCESS.md §4.2 в `/tmp/opencode/rw-08/w8X/report.md`: сделано; red-прогон
   (команда, `N/M`, `# EXIT`, md5 теста, копия); green-прогон (×5, число тестов); clippy/fmt;
   файлы + md5; риски/вопросы. Финальное сообщение координатору — краткое резюме + путь к отчёту.

**Общие правила качества:** ошибки не глотать (последствие или документированный best-effort);
никаких паник на ожидаемых данных; доккомментарии на русском; тесты сверяют **оракул**, а не
реализацию; комментарий в тесте — провенанс (источник, md5, команда, счётчики).

## 3. Оракулы и тест-данные

### 3.1 Факты `docs/re/rest-d02.json` (проверены координатором 2026-10-19)

- 47 уникальных живых API-маршрутов (`method,path` не повторяются); методов:
  **GET 40, POST 3, PUT 2, PATCH 2** (DELETE в захвате нет — вариант enum оставлен для будущего).
- Плейсхолдеры путей: `{id}` ×15, `{num}` ×2; других нет.
- 6 маршрутов с телом запроса (ключи — дословно, включая сентинелы):
  | Метод | Путь (без базы) | `body_keys` |
  |---|---|---|
  | POST | `/channels/{id}/messages/{id}/ack` | `token,last_viewed` |
  | POST | `/users/@me/billing/user-offer` | `allow_triggers` |
  | PUT | `/guilds/{id}/members/@me` | `(empty-object)` |
  | POST | `/beaker` | `token,events` |
  | PATCH | `/channels/{id}/explicit-media` | `message_ids` |
  | PATCH | `/users/@me/settings-proto/{num}` | `(non-json)` |
- Статусы (union по 47): **200, 204, 304, 404, 429**. Не-200 строки:
  `GET /games/detectable/exclusions` 304; `POST /users/@me/billing/user-offer` 204;
  `GET /users/@me/referrals/eligibility` 404; `GET /users/@me/saved-messages` 404;
  `PUT /guilds/{id}/members/@me` 204; `GET /users/@me/notes/{id}` 404; `POST /beaker` 204;
  `PATCH /channels/{id}/explicit-media` 204;
  `PUT /channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` 204;
  `GET /users/@me/survey` **429**.
- Auth: 133/133 API-запроса — полный набор из **6 заголовков**; всего с `Authorization` — 221
  (218 `Authorization` + 3 `authorization`, регистр не влияет на контракт).
  | Заголовок | Всего | Полный набор (API) |
  |---|---|---|
  | `Authorization` | 221 | 133 |
  | `X-Super-Properties` | 221 | 133 |
  | `X-Discord-Locale` | 221 | 133 |
  | `X-Discord-Timezone` | 221 | 133 |
  | `X-Debug-Options` | 221 | 133 |
  | `Accept-Language` | — (счётчика в JSON нет; §6.1) | 133 |
- Кросс-сверка со статикой (`crossRef`): **44 static-confirmed, 2 static-method-unresolved,
  1 static-absent**; для 44 подтверждённых путь сходится после `strip("/api/v9")` и замены
  `{id}`/`{num}` → `{arg}` (проверено: 44/44, 0 расхождений), метод входит в `staticMethod`
  (список через `|`). Не подтверждены: `GET /guilds/{id}/application-command-index`
  (method-unresolved), `POST /beaker` (absent),
  `PUT /channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` (method-unresolved).

Источники (md5): `docs/re/rest-d02.json` — `87d7a05d8cd11f3b8ae37aa5c9811c9e`;
`docs/re/rest-d02.md` — `c7e61929677efab6bdac86a05a778fce`;
`fixtures/rest/rest-static.json` — `8e0ead46addbb2f54f143343d0de2108` (статическая карта, 904
записи; напрямую в этой волне не парсится — её учитывает `crossRef`, см. выше).

### 3.2 Выдержки для тестов (созданы координатором; read-only)

| Файл | Строк | md5 | Команда генерации (jq) |
|---|---|---|---|
| `tests/data/rest-routes-47.tsv` | 47 | `2db6b39bcba96bc3a2d480fea9461a33` | `jq -r '.endpoints[] \| [.method, .path, (.reqBodyKeys\|join(",")), (.statuses\|map(tostring)\|join("|"))] \| @tsv' docs/re/rest-d02.json` |
| `tests/data/rest-crossref-47.tsv` | 47 | `1569384026aa85ba2854f87a7d7048dd` | `jq -r '.crossRef.details[] \| [.method, .path, .verdict, (.staticKey//"-"), (.staticPath//"-"), (.staticMethod//"-")] \| @tsv' docs/re/rest-d02.json` |
| `tests/data/rest-auth-6.tsv` | 6 | `144c34c24ecd4645540be518e69fbe3b` | `jq -r '"Authorization\t\(.auth.withAuthorization)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\n…"' docs/re/rest-d02.json` (полная команда — в шапке теста) |

Форматы: TSV, `\t`, без заголовка; `rest-routes-47.tsv` — `method, path (с /api/v9),
body_keys через запятую ("" = нет тела), statuses через '|'`; `rest-crossref-47.tsv` —
`method, path, verdict, staticKey, staticPath, staticMethod`; `rest-auth-6.tsv` —
`header, total, apiFullSet`.

## 4. Воркер W8a — каталог маршрутов (T-REST-01)

Файлы: `rust/crates/discord-core/src/rest/routes.rs`,
`rust/crates/discord-core/tests/rest_routes.rs`. Заглушка уже лежит в `routes.rs` (контракт §4.1);
данные и логику пишет W8a, сигнатуры не меняет.

### 4.1 Контракт (заморожен)

```rust
pub enum HttpMethod { Get, Post, Put, Patch, Delete }
impl HttpMethod { pub fn as_str(self) -> &'static str; } // "GET"/"POST"/"PUT"/"PATCH"/"DELETE"

pub struct RouteSpec {
    pub method: HttpMethod,
    pub path: &'static str,               // относительный, без "/api/v9"
    pub body_keys: &'static [&'static str], // дословно из оракула; пусто = тела нет
}
pub const ROUTES: [RouteSpec; 47];
pub fn find(method: HttpMethod, path: &str) -> Option<&'static RouteSpec>; // точное равенство
pub enum PathRenderError { PartsMismatch { expected: usize, got: usize }, InvalidPart(String) }
pub fn render_path(spec: &RouteSpec, parts: &[&str]) -> Result<String, PathRenderError>;
```

`render_path`: число плейсхолдеров = число вхождений `{id}`/`{num}` в шаблоне (по порядку);
`parts.len() != count` → `PartsMismatch { expected: count, got: parts.len() }`; часть пустая или
содержит `{`/`}` → `InvalidPart(part.to_string())`; иначе подстановка слева направо, `Ok(строка)`.

### 4.2 Кейсы теста `rest_routes.rs`

- **U1 `catalog_shape_47_unique`** — `ROUTES.len() == 47`; пары `(as_str, path)` уникальны; все
  пути начинаются с `/`, не содержат `api/v9`, непустые, без пробелов.
- **U2 `catalog_matches_live_capture`** — по `rest-routes-47.tsv`: для каждой строки срезать
  `/api/v9` и найти маршрут; метод/путь/`body_keys` (join `,`) совпадают; плюс равенство множеств
  в обе стороны (каждая строка ↔ каждый элемент `ROUTES`).
- **U3 `method_distribution_matches_capture`** — из выдержки и из `ROUTES`: GET 40, POST 3, PUT 2,
  PATCH 2, DELETE 0.
- **U4 `body_keys_only_on_six_routes`** — маршрутов с непустыми `body_keys` ровно 6; явные проверки
  сентинелов: `PUT /guilds/{id}/members/@me` → `["(empty-object)"]`,
  `PATCH /users/@me/settings-proto/{num}` → `["(non-json)"]`, `POST …/ack` →
  `["token","last_viewed"]` и остальные три из §3.1.
- **U5 `find_roundtrip_and_rejects`** — для всех `ROUTES`: `find(method, path)` возвращает этот
  маршрут; отрицания: `find(Get, "/api/v9/channels/{id}/messages")` = `None` (с базой не ищем),
  `find(Post, "/channels/{id}/messages")` = `None`, `find(Get, "/channels/{id}/messagesX")` = `None`.
- **U6 `method_names_cover_enum`** — `as_str` для всех 5 вариантов: GET/POST/PUT/PATCH/DELETE;
  четыре наблюдаемых метода входят в этот набор.
- **U7 `render_path_substitutes_and_rejects`** — успех: `/channels/{id}/messages` + `["123"]`;
  `/channels/{id}/messages/{id}/ack` + `["c1","m2"]` → `/channels/c1/messages/m2/ack`;
  `/applications/public` + `[]`; `/users/@me/settings-proto/{num}` + `["42"]`. Ошибки:
  `[]` для `{id}` → `PartsMismatch{expected:1,got:0}`; `["a","b"]` для одного → `{1,2}`;
  `["x"]` для маршрута без плейсхолдеров → `{0,1}`; `["a{b}"]` → `InvalidPart("a{b}")`;
  `[""]` → `InvalidPart("")`.
- **U8 `crossref_static_confirms_catalog`** — по `rest-crossref-47.tsv`: 47 строк; вердикты
  44/2/1; каждый `(method, path)` есть в каталоге (база срезана); для `static-confirmed`:
  метод входит в `staticMethod` (split `|`) и `normalize(path)` (`{id}`/`{num}` → `{arg}`) равен
  `staticPath`; для остальных `staticMethod == "-"` и (для `static-absent`) `staticPath == "-"`.

**Red-ориентир:** падают U1–U5, U7, U8; U6 проходит по построению (enum из каркаса). Иное
распределение — зафиксировать и разобраться, не подгонять тест.

## 5. Воркер W8b — заголовки и коды (T-REST-02)

Файлы: `rust/crates/discord-core/src/rest/headers.rs`,
`rust/crates/discord-core/tests/rest_headers.rs`.

### 5.1 Контракт (заморожен; сигнатуры не менять)

```rust
pub const AUTH_HEADER_NAMES: [&str; 6]; // порядок: Authorization, X-Super-Properties,
                                        // X-Discord-Locale, X-Discord-Timezone,
                                        // X-Debug-Options, Accept-Language
pub struct AuthHeaderValues<'a> {
    pub authorization: &'a str, pub super_properties: &'a str, pub locale: &'a str,
    pub timezone: &'a str, pub debug_options: &'a str, pub accept_language: &'a str,
}
pub fn auth_headers<'a>(values: &AuthHeaderValues<'a>) -> [(&'static str, &'a str); 6];
pub enum ResponseClass { Ok200, NoContent204, NotModified304, NotFound404, RateLimited429, Other(u16) }
pub fn classify_status(status: u16) -> ResponseClass;
```

### 5.2 Кейсы теста `rest_headers.rs`

- **H1 `auth_header_names_exact_order`** — `AUTH_HEADER_NAMES` равен литералу из 6 имён в порядке
  §3.1 и совпадает с колонкой 1 `rest-auth-6.tsv` (6 строк).
- **H2 `auth_headers_pairs_values`** — `AuthHeaderValues` с различимыми значениями
  (`"tok-1"`, `"sp-2"`, `"loc-3"`, `"tz-4"`, `"dbg-5"`, `"al-6"`): возвращаются 6 пар, имена —
  `AUTH_HEADER_NAMES` в том же порядке, значения — входные.
- **H3 `auth_counts_match_capture`** — по `rest-auth-6.tsv`: 6 строк; `apiFullSet == 133` в каждой;
  `total`: 221 у пяти, `-` у `Accept-Language`; число строк = `AUTH_HEADER_NAMES.len()`
  (факты захвата — в тесте как провенанс; проходит и на заглушке — это контроль выдержки).
- **H4 `base_v9_prefix`** — `discord_core::API_V9_BASE == "https://discord.com/api/v9"`; все 47
  путей из `rest-routes-47.tsv` начинаются с этой базы.
- **H5 `classify_status_observed_and_other`** — 200→`Ok200`, 204→`NoContent204`, 304→
  `NotModified304`, 404→`NotFound404`, 429→`RateLimited429`; 201/301/500/0 → `Other(n)`;
  по выдержке: union статусов ровно `{200,204,304,404,429}`, каждая строка классифицируется
  не в `Other`, `GET /users/@me/survey` → `RateLimited429`.

**Red-ориентир:** падают H1, H2, H5; H3 и H4 проходят (контроль выдержки/существующая база).
Заглушка W8b: `AUTH_HEADER_NAMES = [""; 6]`, `auth_headers` → `[("", ""); 6]`,
`classify_status` → `Other(status)`.

## 6. Воркер W8c — таймауты и ретраи (T-REST-03)

Файлы: `rust/crates/discord-core/src/rest/retry.rs`,
`rust/crates/discord-core/tests/rest_retry.rs`. Использовать `HttpMethod` из
`discord_core::rest::routes` (контракт §4.1 — заморожен).

### 6.1 Контракт (заморожен; сигнатуры не менять)

```rust
pub struct RetryLimits { pub max_attempts: u32, pub attempt_timeout: Duration, pub backoff: Duration }
pub enum AttemptError { Timeout, Transport(String) }
pub enum RetryError { NotRetried { error: AttemptError }, Exhausted { attempts: u32, last: AttemptError } }
pub struct Retried<T> { pub value: T, pub attempts: u32 }
pub fn is_idempotent(method: HttpMethod) -> bool;
pub async fn attempt_once<T, F, Fut>(attempt_timeout: Duration, f: F) -> Result<T, AttemptError>
    where F: FnOnce() -> Fut, Fut: Future<Output = Result<T, AttemptError>>;
pub async fn execute_with_retries<T, F, Fut>(method: HttpMethod, limits: &RetryLimits, attempt: F)
    -> Result<Retried<T>, RetryError>
    where F: FnMut() -> Fut, Fut: Future<Output = Result<T, AttemptError>>;
```

Семантика: идемпотентны `Get`/`Put`/`Delete` (RFC 9110 §9.2.2), не идемпотентны `Post`/`Patch`.
`attempt_once` оборачивает будущее в `tokio::time::timeout` (истёк → `AttemptError::Timeout`).
`execute_with_retries`: не-идемпотентный — ровно 1 попытка (`Err` → `NotRetried`); идемпотентный —
до `max_attempts` (значение 0 трактуется как 1) с `sleep(backoff)` между попытками (`Ok` →
`Retried { value, attempts }`, исчерпание → `Exhausted { attempts, last }`). HTTP-статусы
(429/5xx) в этой волне ретраями **не** управляют — это P1-09.

### 6.2 Тестовый стаб (`127.0.0.1:0`, tokio TcpListener)

Принять соединение → прочитать запрос до `\r\n\r\n` → увеличить счётчик → режим:
`Always200` (ответ `HTTP/1.1 200 OK`, `Content-Length: 2`, тело `ok`, закрыть);
`Hang` (не отвечать; соединение держать); `HangFirst` (1-е — `Hang`, далее `Always200`);
`CloseFirst` (1-е — закрыть без ответа, далее `Always200`). Задачи гасятся в `Drop`
(`JoinHandle::abort`), утечек нет. Клиент в тесте — минимальный HTTP/1.1 по TCP
(`Connection: close`, чтение до EOF, парсинг статуса/тела; пустой ответ → `Transport`).

### 6.3 Кейсы теста `rest_retry.rs`

- **N1 `is_idempotent_table`** — Get/Put/Delete `true`; Post/Patch `false`.
- **N2 `attempt_once_maps_timeout`** — `pending()` + 50 мс → `Err(Timeout)`; мгновенный `Ok(7)` →
  `Ok(7)`.
- **N3 `non_idempotent_single_attempt`** — Post и Patch с замыканием «всегда
  `Err(Transport("boom"))`»: `NotRetried { error: Transport("boom") }`, счётчик вызовов = 1.
- **N4 `idempotent_retries_to_success`** — Get: 1-й `Err(Transport)`, 2-й `Ok(7)` →
  `Retried { value: 7, attempts: 2 }`; Put: `[Timeout, Ok]` → `attempts: 2`.
- **N5 `idempotent_exhausts_and_clamps`** — Get, всегда `Err(Timeout)`, `max_attempts: 3` →
  `Exhausted { attempts: 3, last: Timeout }`, вызовов 3; `max_attempts: 0` → `attempts: 1`,
  вызовов 1.
- **S1 `stub_get_success_first_try`** — стаб `Always200`; Get → `Retried { (200,"ok"), attempts: 1 }`,
  сервер увидел 1 запрос.
- **S2 `stub_timeout_then_retry_wins`** — стаб `HangFirst`, Get, `attempt_timeout 150 мс`,
  `backoff 20 мс`, `max_attempts 3` → `Ok(attempts: 2)`, сервер увидел 2 запроса,
  `elapsed >= 150 мс` (нижняя граница).
- **S3 `stub_timeout_post_not_retried`** — стаб `Hang`, Post, те же лимиты →
  `NotRetried { error: Timeout }`, сервер увидел ровно 1 запрос, `elapsed >= 150 мс`.
- **S4 `stub_all_timeout_exhausts`** — стаб `Hang`, Get, `attempt_timeout 80 мс`, `backoff 10 мс`,
  `max_attempts 3` → `Exhausted { attempts: 3, last: Timeout }`, сервер увидел 3 запроса.
- **S5 `stub_close_is_transport_then_retry`** — стаб `CloseFirst`, Get →
  `Ok(attempts: 2)` (транспортная ошибка ретраится для идемпотентных), сервер увидел 2 запроса.

Тайминги — маленькие и с запасом (никаких верхних границ); каждый async-тест — `#[tokio::test]`.
**Red-ориентир:** падают N1–N5 и S1–S5 (заглушка: `is_idempotent → true`, `attempt_once → Err(Timeout)`,
`execute_with_retries → Err(NotRetried{Timeout})` без вызова замыкания).

## 7. После воркеров — агрегация и гейты (координатор)

1. Полный прогон: `cd rust && cargo test --offline --workspace` (113 существующих + новые);
   `cargo clippy --offline --workspace --all-targets -- -D warnings`;
   `cargo fmt --check`.
2. md5-аудит границ: соседние модули/тесты не менялись; `Cargo.toml/lock` — md5 до/после равны.
3. `npm run gate:worker` (meta/unit/smoke) — зелёный; затем этапы W8d и W8r ниже.

## 8. Этап W8d — рефакторинг-воркер (бриф)

После зелёного: не-поведенческий проход по 6 файлам волны (`rest/{routes,headers,retry}.rs` +
3 теста). Можно: переименования локальных переменных, доккомментарии, устранение дублирования,
мелкие хелперы в тестах при неизменных ассертах. **Нельзя:** менять публичные сигнатуры,
менять тест-данные, менять тесты-спеку (md5 тест-файлов до/после равны), трогать чужие файлы.
Обязательно: `cargo test --offline -p discord-core` (×3), clippy, fmt; в отчёте — `git diff`
до/после и md5. Если правок не требуется — так и написать с доказательством (пустой diff).

## 9. Этап W8r — независимое ревью (бриф)

1. Пересчитать выдержки **своими** командами из `docs/re/rest-d02.json` (md5
   `87d7a05d…`) и сверить побайтово/построчно с `tests/data/*`; проверить md5 источников.
2. Воспроизвести red: заглушки → падения по ориентирам §4–§6; проверить, что md5 теста в
   red-логе = финальному; копии red-тестов на месте.
3. Границы: только файлы волны; `Cargo.toml/lock` не тронуты; соседи целы; golden не тронут.
4. Мутации продукта (минимум 10): перепутать имя заголовка; сдвинуть порядок; сломать
   классификацию кода; `render_path` без проверки частей; `find` по префиксу; `is_idempotent`
   для Post; ретрай Post; игнор `max_attempts`; отсутствие таймаута; битый парсинг стаба —
   тесты должны падать.
5. §4.4-чек-лист (ошибки не заглушены, таймеры/задачи очищены, секретов нет, размеры
   ограничены) + отчёт `/tmp/opencode/review-rw-08.md` (в `waves/reviews/` скопирует
   координатор): вердикт «принято/с нитами», список нитов с доказательствами.

## 10. DoD волны

- `rest_routes` — 8 кейсов (T-REST-01), `rest_headers` — 5 (T-REST-02), `rest_retry` — 10
  (T-REST-03): все зелёные, ×5 стабильно; `discord-core` и воркспейс без регрессий.
- clippy `-D warnings` / `fmt --check` чисто; `gate:worker` зелёный.
- Red-first след по каждому воркеру (лог `# EXIT=`, md5 теста, red-копия).
- Сеть — только `127.0.0.1`; новые зависимости — нет; golden/лаборатория не тронуты.
- **Вне волны:** TLS/HTTP-клиент к `discord.com` (открытый вопрос владельцу), 429/Retry-After
  (P1-09), генератор `X-Super-Properties` (P1-08), тела запросов и реальные вызовы (P1-09/P1-12,
  P2-06).

## 11. Артефакты

- Воркеры: `/tmp/opencode/rw-08/w8a|w8b|w8c/{red.log,rest_*-red.rs,report.md}`.
- Координатор после приёмки: `waves/RW-08/{TZ.md,report.md,red-*.log,rest_*-red.rs,…}`,
  `waves/reviews/review-rw-08.md`, событие в `waves/db.jsonl`, обновление `waves/STATE.md`.
