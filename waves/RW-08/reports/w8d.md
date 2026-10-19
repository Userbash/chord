# Отчёт W8d — не-поведенческий рефакторинг REST-модулей (RW-08)

Волна RW-08 (P1-07), этап после зелёного (`waves/RW-08/TZ.md` §8). Дата: 2026-10-20.

- **сделано:** не-поведенческий проход по трём `src`-файлам волны; тесты-спека не редактировались.
  - `rust/crates/discord-core/src/rest/routes.rs`:
    - актуализирован модульный доккомментарий (описание модуля вместо process-метки «задача W8a»);
    - `#[must_use]` на `HttpMethod::as_str` (чистый запрос, результат нельзя терять);
    - доккомментарии к полям `RouteSpec` и полям варианта `PathRenderError::PartsMismatch`;
    - литералы плейсхолдеров `{id}`/`{num}` вынесены в приватную константу `PLACEHOLDERS`;
      `placeholder_count` и `next_placeholder` используют её (единый источник набора).
  - `rust/crates/discord-core/src/rest/headers.rs`:
    - актуализирован модульный доккомментарий;
    - доккомментарии к полям `AuthHeaderValues`;
    - `#[must_use]` на `auth_headers` и `classify_status`;
    - доккомментарии к вариантам `ResponseClass` (200/204/304/404/429).
  - `rust/crates/discord-core/src/rest/retry.rs`:
    - актуализирован модульный доккомментарий;
    - `#[must_use]` на `is_idempotent`;
    - цикл `execute_with_retries` уплощён: ветка «лимит исчерпан» и `sleep(backoff)` теперь
      внутри одного `Err(error)`, без `Err(_)`-заглушки; порядок вызовов/пауз и семантика
      (`NotRetried`/`Exhausted`) не менялись.
  - Публичные типы, поля, варианты и сигнатуры §4.1/§5.1/§6.1 не менялись; тесты не менялись.
- **red:** не применимо — этап W8d идёт **после** зелёного (§8 ТЗ), red-цикл не предусмотрен.
  Базовый прогон до правок: `cargo test --offline -p discord-core` → 94 passed / 0 failed,
  `EXIT=0` (исходное дерево уже зелёное, отчёты W8a/W8b/W8c).
- **green:**
  - `cd /home/sanya/orca/project/rust && cargo test --offline -p discord-core` — **×3 подряд**,
    каждый прогон **94 passed / 0 failed**, `EXIT=0`; логи
    `/tmp/opencode/rw-08/w8d/test-package-{1,2,3}.log` (2 unit + 12 dispatch + 15 gateway +
    16 identify + 5 headers + 10 retry + 8 routes + 13 resume + 13 ws; doc-tests 0);
  - `cargo test --offline --workspace` → **136 passed / 0 failed**, `EXIT=0`
    (`/tmp/opencode/rw-08/w8d/test-workspace.log`) — ровно ожидаемое число;
  - `cargo clippy --offline --workspace --all-targets -- -D warnings` → `EXIT=0`, без замечаний
    (`/tmp/opencode/rw-08/w8d/clippy.log`);
  - `cargo fmt --check` → `EXIT=0`, вывод пуст (`/tmp/opencode/rw-08/w8d/fmt.log`);
  - `rustfmt --edition 2024` применялся только к трём `src/rest/*.rs`, не к тестам.
- **gate:worker:** **passed** — `npm run gate:worker` → `# GATE_EXIT=0`; typecheck ✔,
  meta 25/25, unit 169/169, smoke 20/20, skip 0, warnings 0; отчёт гейта
  `.gate/worker-2026-10-19T19-13-04-224Z.json`, лог `/tmp/opencode/rw-08/w8d/gate-worker.log`.
- **файлы:** (md5 до → после)

  | Файл | md5 до | md5 после |
  |---|---|---|
  | `rust/crates/discord-core/src/rest/routes.rs` | `0aab077171d97fc98f944e4b75f7c350` | `f66b819f48756ca0d234900a77a39bd6` |
  | `rust/crates/discord-core/src/rest/headers.rs` | `8bdb480e79d422a7d7231113f92684c9` | `720be6fb046840e441a24e333ef5452d` |
  | `rust/crates/discord-core/src/rest/retry.rs` | `804d68c4eca93cf4c869af87771ffb92` | `8a6dc5efbea08f898118cd40b51bf1f4` |

  Тесты-спека (md5 до = после, не редактировались):

  | Файл | md5 до | md5 после | Статус |
  |---|---|---|---|
  | `rust/crates/discord-core/tests/rest_routes.rs` | `f67f3a4a3f83cb301e3fd44cb3e5e260` | `f67f3a4a3f83cb301e3fd44cb3e5e260` | совпал |
  | `rust/crates/discord-core/tests/rest_headers.rs` | `05aba4024176d913667092eb8cf1b3a9` | `05aba4024176d913667092eb8cf1b3a9` | совпал |
  | `rust/crates/discord-core/tests/rest_retry.rs` | `1b7e1c5935c3bd2199c6f5791baaf6dd` | `1b7e1c5935c3bd2199c6f5791baaf6dd` | совпал |

  Копии 6 файлов до/после: `/tmp/opencode/rw-08/w8d/{before,after}/`; md5-снимки:
  `/tmp/opencode/rw-08/w8d/md5-{before,after}.txt`.
- **diff:** полные `diff -u` лежат в `/tmp/opencode/rw-08/w8d/diff-{routes,headers,retry}.patch`
  (сводный — `diff-all.patch`); ниже — они же.

### diff `routes.rs`

```diff
--- a/rust/crates/discord-core/src/rest/routes.rs
+++ b/rust/crates/discord-core/src/rest/routes.rs
@@ -1,7 +1,8 @@
 //! Каталог REST-маршрутов v9 (T-REST-01, волна RW-08 / P1-07).
 //!
-//! **Каркас координатора (заморожен):** типы/сигнатуры — контракт; данные и логика — задача
-//! воркера W8a (`waves/RW-08/TZ.md` §2).
+//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §4.1). Модуль
+//! хранит 47 маршрутов живого захвата D-02, ищет их по точной паре «метод, шаблон» и
+//! подставляет части в плейсхолдеры `{id}`/`{num}`.
 //!
 //! Оракул: `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`) — 47 живых
 //! маршрутов; выдержка для тестов — `tests/data/rest-routes-47.tsv` (47 строк).
@@ -18,6 +19,7 @@
 
 impl HttpMethod {
     /// Имя метода в проводе (`"GET"`, `"POST"`, …).
+    #[must_use]
     pub fn as_str(self) -> &'static str {
         match self {
             HttpMethod::Get => "GET",
@@ -32,8 +34,11 @@
 /// Маршрут: метод, шаблон пути (относительный, без базы `/api/v9`) и ключи тела запроса.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub struct RouteSpec {
+    /// HTTP-метод запроса.
     pub method: HttpMethod,
+    /// Шаблон пути относительно базы (без `/api/v9`).
     pub path: &'static str,
+    /// Ключи тела запроса дословно из оракула; пусто — тела нет.
     pub body_keys: &'static [&'static str],
 }
 
@@ -153,7 +158,12 @@
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub enum PathRenderError {
     /// Число частей не совпало с числом плейсхолдеров `{id}`/`{num}`.
-    PartsMismatch { expected: usize, got: usize },
+    PartsMismatch {
+        /// Сколько плейсхолдеров в шаблоне.
+        expected: usize,
+        /// Сколько частей передано.
+        got: usize,
+    },
     /// Часть пустая или содержит `{`/`}`.
     InvalidPart(String),
 }
@@ -192,15 +202,23 @@
     Ok(rendered)
 }
 
+/// Плейсхолдеры шаблонов пути (набор зафиксирован оракулом D-02).
+const PLACEHOLDERS: [&str; 2] = ["{id}", "{num}"];
+
 /// Число плейсхолдеров `{id}`/`{num}` в шаблоне.
 fn placeholder_count(template: &str) -> usize {
-    template.matches("{id}").count() + template.matches("{num}").count()
+    PLACEHOLDERS
+        .iter()
+        .copied()
+        .map(|placeholder| template.matches(placeholder).count())
+        .sum()
 }
 
 /// Ближайший слева плейсхолдер `{id}`/`{num}`: позиция и литерал.
 fn next_placeholder(template: &str) -> Option<(usize, &'static str)> {
-    ["{id}", "{num}"]
-        .into_iter()
+    PLACEHOLDERS
+        .iter()
+        .copied()
         .filter_map(|placeholder| {
             template
                 .find(placeholder)
```

### diff `headers.rs`

```diff
--- a/rust/crates/discord-core/src/rest/headers.rs
+++ b/rust/crates/discord-core/src/rest/headers.rs
@@ -1,7 +1,8 @@
 //! REST v9: auth-заголовки и коды ответов (T-REST-02, волна RW-08 / P1-07).
 //!
-//! **Каркас координатора (заморожен):** типы/сигнатуры — контракт; данные и логика — задача
-//! воркера W8b (`waves/RW-08/TZ.md` §5).
+//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §5.1). Модуль
+//! содержит шесть имён auth-заголовков полного набора API-запросов, значения этих заголовков
+//! у вызывающего и классификацию наблюдённых HTTP-статусов.
 //!
 //! Оракул: `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`) — 133 API-запроса
 //! с полным набором из 6 auth-заголовков; выдержка для тестов —
@@ -25,11 +26,17 @@
 /// debug-опции и `Accept-Language`. Строки живут не меньше возвращаемых пар.
 #[derive(Debug, Clone, Copy)]
 pub struct AuthHeaderValues<'a> {
+    /// Значение заголовка `Authorization`.
     pub authorization: &'a str,
+    /// Значение заголовка `X-Super-Properties`.
     pub super_properties: &'a str,
+    /// Значение заголовка `X-Discord-Locale`.
     pub locale: &'a str,
+    /// Значение заголовка `X-Discord-Timezone`.
     pub timezone: &'a str,
+    /// Значение заголовка `X-Debug-Options`.
     pub debug_options: &'a str,
+    /// Значение заголовка `Accept-Language`.
     pub accept_language: &'a str,
 }
 
@@ -37,6 +44,7 @@
 ///
 /// Имена — статические (`'static`), значения заимствуются у [`AuthHeaderValues`]; пустые
 /// значения не подменяются и не отбрасываются — политика заполнения у вызывающего.
+#[must_use]
 pub fn auth_headers<'a>(values: &AuthHeaderValues<'a>) -> [(&'static str, &'a str); 6] {
     [
         (AUTH_HEADER_NAMES[0], values.authorization),
@@ -51,10 +59,15 @@
 /// Класс HTTP-статуса ответа REST; в захвате D-02 наблюдались 200/204/304/404/429.
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
 pub enum ResponseClass {
+    /// `200 OK`.
     Ok200,
+    /// `204 No Content`.
     NoContent204,
+    /// `304 Not Modified`.
     NotModified304,
+    /// `404 Not Found`.
     NotFound404,
+    /// `429 Too Many Requests`.
     RateLimited429,
     /// Любой другой статус (включая ненаблюдённые 2xx/3xx/5xx); код сохраняется.
     Other(u16),
@@ -62,6 +75,7 @@
 
 /// Классификация HTTP-статуса: пять наблюдённых кодов — отдельные варианты, остальные —
 /// [`ResponseClass::Other`] с исходным кодом.
+#[must_use]
 pub fn classify_status(status: u16) -> ResponseClass {
     match status {
         200 => ResponseClass::Ok200,
```

### diff `retry.rs`

```diff
--- a/rust/crates/discord-core/src/rest/retry.rs
+++ b/rust/crates/discord-core/src/rest/retry.rs
@@ -1,6 +1,6 @@
 //! REST v9: таймауты и ретраи идемпотентных запросов (T-REST-03, волна RW-08 / P1-07).
 //!
-//! Владелец — воркер W8c (`waves/RW-08/TZ.md` §6); каркас модуля создан координатором.
+//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §6.1).
 //!
 //! Политика над транспорт-замыканием (без сети: реальный HTTP-клиент/TLS — открытый вопрос
 //! владельцу, P1-08/P1-09; тесты волны работают на локальном стабе `127.0.0.1`).
@@ -61,6 +61,7 @@
 }
 
 /// Идемпотентен ли метод (RFC 9110 §9.2.2): GET/PUT/DELETE — да, POST/PATCH — нет.
+#[must_use]
 pub fn is_idempotent(method: HttpMethod) -> bool {
     matches!(
         method,
@@ -109,17 +110,19 @@
         attempts += 1;
         match attempt_once(limits.attempt_timeout, &mut attempt).await {
             Ok(value) => return Ok(Retried { value, attempts }),
-            Err(error) if attempts >= max_attempts => {
-                return Err(if idempotent {
-                    RetryError::Exhausted {
-                        attempts,
-                        last: error,
-                    }
-                } else {
-                    RetryError::NotRetried { error }
-                });
+            Err(error) => {
+                if attempts >= max_attempts {
+                    return Err(if idempotent {
+                        RetryError::Exhausted {
+                            attempts,
+                            last: error,
+                        }
+                    } else {
+                        RetryError::NotRetried { error }
+                    });
+                }
+                sleep(limits.backoff).await;
             }
-            Err(_) => sleep(limits.backoff).await,
         }
     }
 }
```

- **границы:** изменены только три `src/rest/*.rs`; тесты-спека не тронуты (md5 выше).
  `Cargo.toml`/`Cargo.lock` не менялись (`git status` их не показывает; текущие md5
  `900f1e7bbce483b28fcc102638f07f23` / `d067aaa1645cf5153ca47acdcd91af7c`),
  `tests/data/*`, `src/rest/mod.rs`, `lib.rs`, чужие модули/тесты, `docs/`, `waves/`, golden —
  не менялись. Сеть не использовалась, `cargo clean`/git-мутаций не было. Набор
  `git status --short` после работы совпадает с исходным (до работы в нём уже были изменения
  координатора: `AGENTS.md`, `PROCESS.md`, `TASKS.md`, `lib.rs`, `waves/*`, untracked
  `src/rest/`, `tests/data/`, три теста, `waves/RW-08/`).
- **предложения по тестам (не применены — md5 тестов заморожен):**
  1. `tests/rest_routes.rs::read_rows` и `tests/rest_headers.rs::data_rows` — почти одинаковый
     TSV-ридер (различаются только проверкой непустоты). Общий `tests/common/rest_tsv.rs`
     убрал бы дублирование, но требует правки обоих тест-файлов; вынести в отдельную волну
     с разморозкой спек.
  2. `tests/rest_retry.rs` — восемь повторяющихся литералов `RetryLimits` и повторяющийся
     `within_case("…", execute_with_retries(...))`; хелперы `limits(...)`/`probe(...)`
     сократили бы шум без изменения ассертов (тоже требует правки теста).
  3. `tests/rest_routes.rs` (U7) — повторяющийся `find(...).expect(...)`; локальный хелпер
     `spec(method, path)` возможен, но текущая явность читается нормально.
  4. `OBSERVED_AUTH_HEADERS` в `rest_headers.rs` намеренно дублирует продакшн-константу как
     независимый оракул H1 — **не** дедуплицировать.
- **риски/вопросы:**
  1. `#[must_use]` — только compile-time диагностика; в воркспейсе нет вызовов, игнорирующих
     результаты (проверено clippy `-D warnings`). Для будущих вызывающих это намеренное
     предупреждение, runtime-поведение не меняется.
  2. `PLACEHOLDERS` — теперь единственный источник набора `{id}`/`{num}`; при расширении
     оракула (новый плейсхолдер) правится одна константа, обе функции увидят изменение.
     Набор в захвате D-02 неизменен (15 `{id}` + 2 `{num}`).
  3. Уплощение цикла ретраев не меняет число вызовов замыкания, моменты `sleep(backoff)` и
     варианты ошибок; подтверждено 3× пакетом, воркспейсом и неизменными N1–N5/S1–S5.
  4. Тайминговые кейсы S2–S4 остаются чувствительными к нагрузке машины — это свойство
     тестов W8c (только нижние границы), рефакторинг его не усиливает и не ослабляет.
  5. Вопросов к координатору нет; поведение и публичный контракт не менялись.
