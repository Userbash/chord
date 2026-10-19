# Ревью RW-08 (W8r) — REST-клиент (P1-07 / T-REST-01…03)

**Вердикт: принято с нитами.** Блокеров нет: red-first воспроизведён, оракулы пересчитаны
независимо, границы соблюдены, все гейты зелёные, 14/14 запланированных мутаций пойманы.
Ниты — low/info (устаревший md5 в отчёте W8a, тайминговая хрупкость S2–S4, непокрытый
`sleep(backoff)` и тестовые буферы без лимита); продукт менять не требуется.

- Ревьюер: W8r (независимо от авторов), дата: 2026-10-20.
- Область: `rust/crates/discord-core/src/rest/{mod,routes,headers,retry}.rs`,
  `tests/rest_{routes,headers,retry}.rs`, `tests/data/rest-*.tsv`, `lib.rs`.
- Артефакты ревью: `/tmp/opencode/review-rw08/` (логи, red-репродукция, мутации).

---

## 1. Оракулы — независимый пересчёт (не подгонка)

Источники проверены по md5:

```text
87d7a05d8cd11f3b8ae37aa5c9811c9e  docs/re/rest-d02.json
c7e61929677efab6bdac86a05a778fce  docs/re/rest-d02.md
```

Выдержки пересчитаны **своими** jq-командами (ТЗ §3.2) и сверены с файлами `tests/data/*`
побайтово:

```bash
jq -r '.endpoints[] | [.method, .path, (.reqBodyKeys|join(",")), (.statuses|map(tostring)|join("|"))] | @tsv' \
  docs/re/rest-d02.json > /tmp/opencode/review-rw08/routes.tsv
# md5: 2db6b39bcba96bc3a2d480fea9461a33
# diff с tests/data/rest-routes-47.tsv: пусто

jq -r '.crossRef.details[] | [.method, .path, .verdict, (.staticKey//"-"), (.staticPath//"-"), (.staticMethod//"-")] | @tsv' \
  docs/re/rest-d02.json > /tmp/opencode/review-rw08/crossref.tsv
# md5: 1569384026aa85ba2854f87a7d7048dd
# diff с tests/data/rest-crossref-47.tsv: пусто

jq -r '"Authorization\t\(.auth.withAuthorization)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\n…"' \
  docs/re/rest-d02.json > /tmp/opencode/review-rw08/auth.tsv
# md5: 144c34c24ecd4645540be518e69fbe3b
# diff с tests/data/rest-auth-6.tsv: пусто
```

Факты §3.1 (независимый Python-разбор JSON):

```text
endpoints 47
methods {'GET': 40, 'POST': 3, 'PUT': 2, 'PATCH': 2}
statuses [200, 204, 304, 404, 429]
body routes 6
placeholders {'{id}': 15, '{num}': 2}
crossref 47 {'static-confirmed': 44, 'static-method-unresolved': 2, 'static-absent': 1}
confirmed path/method ok 44
auth {'withAuthorization': 221, 'withSuperProperties': 221, 'withLocale': 221,
      'withTimezone': 221, 'withDebugOptions': 221} full 133
unique endpoints 47
```

Тела запроса совпадают с §3.1, включая сентинелы `(empty-object)` и `(non-json)`:

```text
POST  /channels/{id}/messages/{id}/ack        token,last_viewed
POST  /users/@me/billing/user-offer           allow_triggers
PUT   /guilds/{id}/members/@me                (empty-object)
POST  /beaker                                  token,events
PATCH /channels/{id}/explicit-media            message_ids
PATCH /users/@me/settings-proto/{num}          (non-json)
```

**Вывод:** выдержки в репозитории — точный срез JSON-оракула; тесты не подгонялись под код.

---

## 2. Red-first — логи и независимая репродукция

### 2.1 Логи воркеров

| Воркер | Лог | Распределение | `# EXIT` | md5 теста в логе | md5 финального теста | Red-копия |
|---|---|---|---|---|---|---|
| W8a | `red-fmt.log` (финальный, после fmt) | 1 passed / 7 failed | 101 | `f67f3a4a3f83cb301e3fd44cb3e5e260` | `f67f3a4a…` | `rest_routes-red.rs` — побайтово равен |
| W8a | `red.log` (ранний, до fmt) | 1 passed / 7 failed | 101 | `ae203dec7708d2f8164e374a946db62a` | — | ранняя копия не сохранена |
| W8b | `red.log` | 2 passed / 3 failed | 101 | `05aba4024176d913667092eb8cf1b3a9` | `05aba402…` | равен |
| W8c | `red.log` | 0 passed / 10 failed | 101 | `1b7e1c5935c3bd2199c6f5791baaf6dd` | `1b7e1c59…` | равен |

Проверка копий:

```text
md5sum /tmp/opencode/rw-08/w8a/rest_routes-red.rs  → f67f3a4a…
md5sum /tmp/opencode/rw-08/w8b/rest_headers-red.rs → 05aba402…
md5sum /tmp/opencode/rw-08/w8c/rest_retry-red.rs   → 1b7e1c59…
diff с финальными tests/rest_*.rs — пусто
```

W8a: тест был переформатирован после раннего red; финальный red (`red-fmt.log`, 00:04)
снят уже на финальном тесте, md5 совпадает. Распределение и упавшие кейсы идентичны раннему.
Это соответствует §2.4 ТЗ (после правки теста red повторён).

### 2.2 Репродукция red своими заглушками

Временные заглушки по §4.2/§5.2/§6.3: `ROUTES` из 47 пустых `Get`, `find → None`,
`render_path → Ok(path)`; `AUTH_HEADER_NAMES = [""; 6]`, `auth_headers → [("", ""); 6]`,
`classify_status → Other`; `is_idempotent → true`, `attempt_once → Err(Timeout)`,
`execute_with_retries → Err(NotRetried{Timeout})` без вызова замыкания. Файлы продукта
восстановлены, md5 проверены.

| Цель | Результат | Совпадение с логом воркера |
|---|---|---|
| `rest_routes` | 1 passed / 7 failed, EXIT=101 | точь-в-точь: U1–U5, U7, U8 падают, U6 проходит |
| `rest_headers` | 2 passed / 3 failed, EXIT=101 | H1, H2, H5 падают; H3, H4 проходят |
| `rest_retry` | 0 passed / 10 failed, EXIT=101 | все N1–N5, S1–S5 падают |

Списки упавших кейсов и строки паники совпали с оригиналами; падения — ассерты, не
`todo!`/`unimplemented!`/компиляция. Логи:
`/tmp/opencode/review-rw08/red-repro-rest_{routes,headers,retry}.log`.

---

## 3. Границы и целостность

`git status --short` в начале и в конце ревью идентичен:

```text
 M AGENTS.md
 M PROCESS.md
 M TASKS.md
 M rust/crates/discord-core/src/lib.rs
 M waves/STATE.md
 M waves/db.jsonl
?? rust/crates/discord-core/src/rest/
?? rust/crates/discord-core/tests/data/
?? rust/crates/discord-core/tests/rest_headers.rs
?? rust/crates/discord-core/tests/rest_retry.rs
?? rust/crates/discord-core/tests/rest_routes.rs
?? waves/RW-08/
```

- `lib.rs` — ровно одна строка `+pub mod rest;` (`git diff` показывает только её).
- Новые продуктовые файлы — только `src/rest/{mod,routes,headers,retry}.rs`, три теста и три TSV.
- `Cargo.toml`/`Cargo.lock` не тронуты:

```text
$ git diff --stat -- rust/Cargo.toml rust/Cargo.lock
(пусто)
md5: rust/Cargo.toml 900f1e7bbce483b28fcc102638f07f23
     rust/Cargo.lock d067aaa1645cf5153ca47acdcd91af7c
```

- Соседние модули RW-01…RW-07 не менялись: `find rust -newermt '2026-10-19 23:29'` даёт только
  `lib.rs`, `src/rest/*` и тесты волны; остальные `src/**/*.rs` старее старта волны.
- `docs/` нет в `git status`; `docs/re/rest-d02.{json,md}` — mtime 2026-10-19 14:24/14:36
  (до волны), md5 совпали.
- Golden `logs/`, `functions/`, `evidence/` — mtime не новее 2026-10-18, в `git status` нет.
- `waves/` изменён только координатором (mtime 23:38–23:39, до первых тестов воркеров 23:46+);
  это журнал/ТЗ волны, не продукт воркеров. `waves/RW-08/` содержит только `TZ.md`.
- Новых зависимостей нет: `Cargo.*` без diff, в `src/rest` используются уже разрешённые
  `tokio`/`std`.

Проверенные md5 (старт → финал, все совпали):

| Файл | Старт | Финал |
|---|---|---|
| `src/rest/routes.rs` | `f66b819f48756ca0d234900a77a39bd6` | `f66b819f48756ca0d234900a77a39bd6` |
| `src/rest/headers.rs` | `720be6fb046840e441a24e333ef5452d` | `720be6fb046840e441a24e333ef5452d` |
| `src/rest/retry.rs` | `8a6dc5efbea08f898118cd40b51bf1f4` | `8a6dc5efbea08f898118cd40b51bf1f4` |
| `tests/rest_routes.rs` | `f67f3a4a3f83cb301e3fd44cb3e5e260` | `f67f3a4a3f83cb301e3fd44cb3e5e260` |
| `tests/rest_headers.rs` | `05aba4024176d913667092eb8cf1b3a9` | `05aba4024176d913667092eb8cf1b3a9` |
| `tests/rest_retry.rs` | `1b7e1c5935c3bd2199c6f5791baaf6dd` | `1b7e1c5935c3bd2199c6f5791baaf6dd` |
| `tests/data/rest-routes-47.tsv` | `2db6b39bcba96bc3a2d480fea9461a33` | `2db6b39bcba96bc3a2d480fea9461a33` |
| `tests/data/rest-crossref-47.tsv` | `1569384026aa85ba2854f87a7d7048dd` | `1569384026aa85ba2854f87a7d7048dd` |
| `tests/data/rest-auth-6.tsv` | `144c34c24ecd4645540be518e69fbe3b` | `144c34c24ecd4645540be518e69fbe3b` |
| `docs/re/rest-d02.json` | `87d7a05d8cd11f3b8ae37aa5c9811c9e` | `87d7a05d8cd11f3b8ae37aa5c9811c9e` |
| `docs/re/rest-d02.md` | `c7e61929677efab6bdac86a05a778fce` | `c7e61929677efab6bdac86a05a778fce` |
| `rust/Cargo.toml` | — | `900f1e7bbce483b28fcc102638f07f23` |
| `rust/Cargo.lock` | — | `d067aaa1645cf5153ca47acdcd91af7c` |

---

## 4. Мутации продукта

Метод: одна временная правка в `src/rest/*.rs` → `cargo test --offline -p discord-core
--test <target>` → восстановление из копии → md5. Все правки компилировались; падения —
ассерты тестов, не ошибки сборки. Полный лог: `/tmp/opencode/review-rw08/mutations.log`.

| ID | Файл | Мутация | Тест | Результат | Упавшие кейсы |
|---|---|---|---|---|---|
| M01 | headers.rs | имя `X-Discord-Locale` испорчено | rest_headers | поймана (4/1) | `auth_header_names_exact_order` |
| M02 | headers.rs | порядок locale/timezone перепутан | rest_headers | поймана (4/1) | `auth_header_names_exact_order` |
| M03 | headers.rs | 404 → `Other(404)` | rest_headers | поймана (4/1) | `classify_status_observed_and_other` |
| M04 | headers.rs | значения locale/timezone перепутаны | rest_headers | поймана (4/1) | `auth_headers_pairs_values` |
| M05 | routes.rs | путь `/channels/{id}/messages` испорчен | rest_routes | поймана (5/3) | `catalog_matches_live_capture`, `crossref_static_confirms_catalog`, `render_path_substitutes_and_rejects` |
| M06 | routes.rs | `find` по префиксу (`path.starts_with(spec.path)`) | rest_routes | поймана (5/3) | `catalog_matches_live_capture`, `find_roundtrip_and_rejects`, `render_path_substitutes_and_rejects` |
| M07 | routes.rs | `render_path` без проверки числа частей | rest_routes | поймана (7/1) | `render_path_substitutes_and_rejects` |
| M08 | routes.rs | `render_path` без проверки пустых/скобочных частей | rest_routes | поймана (7/1) | `render_path_substitutes_and_rejects` |
| M10 | routes.rs | `body_keys` неполные (`token` вместо `token,last_viewed`) | rest_routes | поймана (6/2) | `body_keys_only_on_six_routes`, `catalog_matches_live_capture` |
| M11 | retry.rs | `is_idempotent(Post) = true` | rest_retry | поймана (7/3) | `is_idempotent_table`, `non_idempotent_single_attempt`, `stub_timeout_post_not_retried` |
| M12 | retry.rs | POST/PATCH ретраятся (лимит не форсируется в 1) | rest_retry | поймана (8/2) | `non_idempotent_single_attempt`, `stub_timeout_post_not_retried` |
| M13 | retry.rs | `max_attempts` игнорируется (фикс 10) | rest_retry | поймана (8/2) | `idempotent_exhausts_and_clamps`, `stub_all_timeout_exhausts` |
| M14 | retry.rs | таймаут попытки снят | rest_retry | поймана (6/4) | `attempt_once_maps_timeout`, `stub_all_timeout_exhausts`, `stub_timeout_post_not_retried`, `stub_timeout_then_retry_wins` |
| M15 | retry.rs | неверный счётчик `attempts` (на 1 меньше) | rest_retry | поймана (6/4) | `idempotent_retries_to_success`, `stub_close_is_transport_then_retry`, `stub_get_success_first_try`, `stub_timeout_then_retry_wins` |
| M16* | retry.rs | удалён `sleep(limits.backoff)` | rest_retry | **выжила (10/0)** | — |

`M09` была no-op заготовкой и не запускалась; в таблице 14 плановых валидных мутаций.
**Итог: 14/14 плановых пойманы, 0 выживших среди них. Одна внеплановая (M16) выжила —
это тест-пробел, см. нит N3.** Продукт после M16 восстановлен, md5 `8a6dc5ef…`, тесты 10/10.

Замечание по методике: после восстановления через `cp -p` (сохранение mtime) Cargo один раз
использовал устаревший артефакт от мутации M15 — прогон показал `attempts: 0/1`; после
`touch` исходников и пересборки — 10/10 зелёных, md5 не изменились. Это артефакт ревьюера,
не код волны; финальные прогоны чистые.

---

## 5. Контракты §4.1/§5.1/§6.1

Сигнатуры и типы совпадают с ТЗ:

- `HttpMethod` (`Get/Post/Put/Patch/Delete`), `as_str`, `RouteSpec`, `ROUTES: [RouteSpec; 47]`,
  `find`, `PathRenderError`, `render_path` — без изменений контракта.
- `AUTH_HEADER_NAMES: [&str; 6]` в порядке §3.1; `AuthHeaderValues`, `auth_headers`,
  `ResponseClass` (200/204/304/404/429 + `Other`), `classify_status`.
- `RetryLimits`, `AttemptError`, `RetryError`, `Retried<T>`, `is_idempotent`,
  `attempt_once`, `execute_with_retries` — сигнатуры §6.1.

Семантика проверена по коду и тестам:

- `render_path`: сначала число частей (`PartsMismatch`), затем пустая/со скобками часть
  (`InvalidPart`), затем подстановка слева направо; U7 фиксирует обе ветки и приоритет.
- `RetryLimits.max_attempts = 0` → одна попытка (N5); POST/PATCH — ровно одна попытка,
  `NotRetried` (N3, S3); каждая попытка под `attempt_timeout` (N2, S2–S4); `sleep(backoff)`
  между попытками (код; тест-пробел — N3).
- 429/5xx ретраями не управляются — соответствует §6.1 (вне волны, P1-09).
- H4: в выдержке пути относительные (`/api/v9/...`), поэтому тест проверяет префикс
  `/api/v9`, константу `API_V9_BASE` и полный URL `https://discord.com` + путь с границей
  сегмента. Это корректная и документированная интерпретация §5.2; буквальная проверка
  «путь из TSV начинается с `https://discord.com/api/v9`» невозможна.

---

## 6. Сеть, безопасность, утечки

- Сеть — только loopback: `TcpListener::bind("127.0.0.1:0")` (rest_retry.rs:268);
  `discord.com` встречается только как строковая константа/ассерт `API_V9_BASE`, сетевых
  вызовов к нему нет.
- Секретов/токенов в тест-данных нет: `rest-auth-6.tsv` содержит только имена заголовков и
  счётчики 221/133; в `rest-routes-47.tsv`/`rest-crossref-47.tsv` — методы, пути, ключи,
  статусы, вердикты.
- `unsafe` нет; паники — только ассерты/`expect` тестов на ожидаемых данных, не в продукте.
- `tokio::spawn` — accept-цикл и обработчики; `StubServer::drop` делает `abort()` accept и
  всех сохранённых `JoinHandle` (rest_retry.rs:276, 296–308, 322). `#[tokio::test]` —
  current_thread, гонки «push после drain» нет.
- Бесконечных циклов в продукте нет: `execute_with_retries` ограничен `max_attempts.max(1)`;
  accept-цикл тестовый и гасится в `Drop`.
- Ошибки не глотаются: `attempt_once`/`execute_with_retries` возвращают `Timeout`/`Transport`/
  `Retried`/`Exhausted`; в тестовом стабе игнор записи и чтения — с комментариями о штатной
  гонке/таймауте (best-effort, не продуктовый путь).
- Тестовые буферы `request`/`raw` не ограничены по размеру (см. N4), но это loopback-стаб
  с фиксированным ответом и клиентом под контролем теста; в продукте неограниченных буферов нет.

---

## 7. Гейты (независимый перезапуск)

| Проверка | Команда | Результат |
|---|---|---|
| Workspace tests | `cd rust && cargo test --offline --workspace` | **136 passed, 0 failed, 0 ignored**, EXIT=0 |
| Clippy | `cargo clippy --offline --workspace --all-targets -- -D warnings` | EXIT=0, без замечаний |
| Форматирование | `cargo fmt --check` | EXIT=0, вывод пуст (0 байт) |
| Worker gate | `npm run gate:worker` | **passed**: typecheck ✔, meta 25/25, unit 169/169, smoke 20/20, skip 0 |
| Стабильность | `rest_retry` ×5; `rest_routes`/`rest_headers` ×3 | все прогоны зелёные (10/10, 8/8, 5/5) |

Отчёты воркеров/W8d сходятся: 136 в воркспейсе, 94 в `discord-core`, clippy/fmt/gate зелёные.
Мои прогоны дали те же числа.

---

## 8. Ниты (не блокируют приёмку)

### N1 — low: отчёт W8a устарел по md5 теста
**Доказательство:**
```text
$ md5sum rust/crates/discord-core/tests/rest_routes.rs
f67f3a4a3f83cb301e3fd44cb3e5e260  …
$ grep -n 'ae203dec\|f67f3a4a' /tmp/opencode/rw-08/w8a/report.md
23:  Лог: …/red.log; md5 теста `ae203dec7708d2f8164e374a946db62a`;
41:  `ae203dec7708d2f8164e374a946db62a` (= red-копия);
```
Отчёт утверждает, что финальный тест и red-копия имеют `ae203dec…`, но фактический тест и
`rest_routes-red.rs` — `f67f3a4a…`; финальный red после форматирования — `red-fmt.log` с
`f67f3a4a…`. Ранняя копия теста не сохранена, поэтому побайтово доказать «только fmt» нельзя;
однако финальный red снят на финальном тесте, распределение 1/7 и упавшие кейсы совпадают.
**Рекомендация:** при копировании артефактов в `waves/RW-08/` приложить `red-fmt.log` как
канонический red и поправить md5 в отчёте W8a; ранний `red.log` пометить как «до fmt».

### N2 — low: S2–S4 чувствительны к задержке чтения запроса стабом
**Доказательство:** S3 ждёт ровно 1 запрос при `attempt_timeout = 150 мс`
(`rest_retry.rs:533`), S4 — 3 запроса при 80 мс (`:564`); счётчик растёт только после
чтения `\r\n\r\n` (`:361`). На перегруженной машине accept/read может не успеть до таймаута,
и счётчик будет 0/2 вместо 1/3. Сам W8c отметил это в отчёте (риск 1). В 8 прогонах W8c и
5 моих прогонах флака не было; верхних границ времени нет, только нижние.
**Рекомендация:** на будущее — барьер готовности стаба или повтор одного кейса при флаке;
сейчас соответствует §6.3 ТЗ и не блокирует.

### N3 — low: удаление `sleep(backoff)` не ловится тестами
**Доказательство:** мутация M16 — удалить `sleep(limits.backoff).await;`
(`src/rest/retry.rs:124`) — даёт `test result: ok. 10 passed; 0 failed` (EXIT=0,
`/tmp/opencode/review-rw08/mutation-nosleep.log`). S2 проверяет только
`elapsed >= 150 мс` (таймаут), а не паузу 20 мс; S4 время не проверяет. Код волны корректен
(пауза есть), но тесты не фиксируют часть семантики §6.1.
**Рекомендация:** в будущей волне (после разморозки спек) добавить кейс с нижней границей
`elapsed >= (max_attempts-1) * backoff` на маленьких значениях; сейчас — тест-пробел, не
дефект продукта.

### N4 — info: тестовые буферы без верхнего лимита
**Доказательство:** `rest_retry.rs:345–359` накапливает запрос в `Vec` до `\r\n\r\n` без
ограничения; `:412–416` читает ответ через `read_to_end` без лимита. Это только loopback-стаб
с фиксированным ответом `Content-Length: 2` и тестовым клиентом; продуктовые пути не
затронуты. **Действий не требуется**, отмечено для §4.4-чек-листа.

### N5 — info: H4 — интерпретация, а не буквальная проверка
**Доказательство:** `rest_headers.rs:147–177` проверяет `/api/v9` + полный URL
`https://discord.com` + границу сегмента. Это корректно для относительных путей выдержки;
буквальное «путь из TSV начинается с `https://discord.com/api/v9`» невозможно. Интерпретация
описана в шапке теста и отчёте W8b; принято.

---

## 9. Воспроизведение red — команды

```bash
# 1) бэкап продуктовых файлов
cp rust/crates/discord-core/src/rest/{routes,headers,retry}.rs /tmp/opencode/review-rw08/src-backup/

# 2) заглушки §4.2/§5.2/§6.3 (лежат в /tmp/opencode/review-rw08/stubs/)
cp /tmp/opencode/review-rw08/stubs/{routes,headers,retry}.rs rust/crates/discord-core/src/rest/

# 3) red-прогоны
cd rust
cargo test --offline -p discord-core --test rest_routes   # 1 passed / 7 failed, EXIT=101
cargo test --offline -p discord-core --test rest_headers  # 2 passed / 3 failed, EXIT=101
cargo test --offline -p discord-core --test rest_retry    # 0 passed / 10 failed, EXIT=101

# 4) восстановление и проверка
cp /tmp/opencode/review-rw08/src-backup/{routes,headers,retry}.rs rust/crates/discord-core/src/rest/
md5sum rust/crates/discord-core/src/rest/*.rs   # f66b…/720b…/8a6d… — совпали
```

Финальные md5 после всех манипуляций — в таблице §3; `git status` — в §10.

---

## 10. Финальное состояние репозитория

`git status --short` (идентичен стартовому):

```text
 M AGENTS.md
 M PROCESS.md
 M TASKS.md
 M rust/crates/discord-core/src/lib.rs
 M waves/STATE.md
 M waves/db.jsonl
?? rust/crates/discord-core/src/rest/
?? rust/crates/discord-core/tests/data/
?? rust/crates/discord-core/tests/rest_headers.rs
?? rust/crates/discord-core/tests/rest_retry.rs
?? rust/crates/discord-core/tests/rest_routes.rs
?? waves/RW-08/
```

`git diff --stat -- rust/Cargo.toml rust/Cargo.lock` — пусто. Все стартовые md5 совпали
(таблица §3). Временные файлы ревьюера — только в `/tmp/opencode/review-rw08/`; новые
`.gate/worker-*.json`, созданные моими прогонами, удалены (оставлены отчёты воркеров/W8d).
`rust/target/` и `.gate/` — ignored, в git не попадают.

**Итог:** продукт и тесты волны принимаются; ни один нит не требует правки кода в этой волне.
