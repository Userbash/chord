# Отчёт воркера W8c — T-REST-03 (таймауты и ретраи идемпотентных запросов)

Волна RW-08 (P1-07), параллельный запуск. 2026-10-19. Воркер W8c.

- **сделано:** реализованы «таймауты и ретраи» `discord_core::rest::retry` по контракту §6.1 ТЗ
  RW-08 (сигнатуры не менялись; добавлены только derive-ы `Debug/Clone/PartialEq/Eq` для
  ассертов). `is_idempotent`: GET/PUT/DELETE (RFC 9110 §9.2.2), POST/PATCH — нет;
  `attempt_once` — обёртка `tokio::time::timeout` (истёк → `AttemptError::Timeout`);
  `execute_with_retries` — не-идемпотентный: ровно 1 попытка (`NotRetried`), идемпотентный: до
  `max_attempts` (0 → 1) с `sleep(backoff)` (`Retried`/`Exhausted`). Новый тест-файл —
  10 кейсов N1–N5 и S1–S5 (§6.3) на локальном стабе `127.0.0.1:0` (tokio TcpListener, режимы
  Always200/Hang/HangFirst/CloseFirst, счётчик запросов, задачи гасятся в `Drop`) и минимальном
  HTTP/1.1-клиенте теста (`Connection: close`, чтение до EOF, пустой ответ → `Transport`).
  Сеть — только loopback; новых зависимостей нет.

- **red:** `cd rust && bash -c 'cargo test --offline -p discord-core --test rest_retry 2>&1 |
  tee /tmp/opencode/rw-08/w8c/red.log; echo "# EXIT=${PIPESTATUS[0]}" | tee -a
  /tmp/opencode/rw-08/w8c/red.log'` → **0 passed / 10 failed**, `# EXIT=101`; падения — только
  ассерты (ориентир §6.3: «падают N1–N5 и S1–S5»), без `todo!`/паник заглушки.
  md5 теста `1b7e1c5935c3bd2199c6f5791baaf6dd` (= финальному); копия —
  `/tmp/opencode/rw-08/w8c/rest_retry-red.rs` (md5 совпадает).
  Примечание §2.4 ТЗ: первый red-прогон был с md5 `c1b6d513d409b941022450bc719483e3`; после
  `rustfmt` тест переформатирован (семантика не менялась), red **повторён заново** на
  финальном файле — в red-логе финальный md5. Тест после red больше не менялся.

- **green:**
  - свой таргет ×5 подряд: все прогоны `ok. 10 passed; 0 failed; 0 ignored`, EXIT=0
    (`green-run1.log`…`green-run5.log`); дополнительно ещё ×3 (`green-run6/7/8.log`) — стабильно
    (суммарно 8/8 зелёных прогонов; тайминги сходятся — 0.27 с/прогон);
  - полный пакет: `cargo test --offline -p discord-core` → EXIT=0, 10 бинарей, 94 теста,
    все `test result: ok` (`green-package.log`); падений в чужих файлах не было (W8a/W8b на
    момент прогона собирались и проходили);
  - `rustfmt --edition 2024 --check` по обоим своим файлам — чисто;
  - `cargo clippy --offline -p discord-core --all-targets -- -D warnings` — чисто
    (`clippy.log`; по ходу исправлено своё замечание `redundant_closure`:
    `|| attempt()` → `&mut attempt`, после чего green ×5 и полный пакет прогнаны заново).

- **gate:worker:** `npm run gate:worker` → **passed**; отчёт гейта
  `.gate/worker-2026-10-19T18-59-37-801Z.json` (typecheck ✔, meta 25/25, unit 169/169,
  smoke 20/20, 0 skip).

- **файлы** (только мои два):
  - `rust/crates/discord-core/src/rest/retry.rs` — md5 `804d68c4eca93cf4c869af87771ffb92`;
  - `rust/crates/discord-core/tests/rest_retry.rs` — md5 `1b7e1c5935c3bd2199c6f5791baaf6dd`;
  - артефакты: `/tmp/opencode/rw-08/w8c/{red.log, rest_retry-red.rs, green-run*.log,
    green-package.log, clippy.log, report.md}` (плюс страховочная копия
    `retry-impl-final.rs.bak`).
  Границы: `routes.rs`/`headers.rs` и их тесты не трогал; `tests/data/*.tsv` — md5 без
  изменений (`144c34…`, `156938…`, `2db6b39…`, как в ТЗ §3.2); `Cargo.toml`/`Cargo.lock` — без
  diff; golden/`docs`/`waves` — только чтение; git-мутаций не было.

- **риски/вопросы:**
  1. Тайминги — только нижние границы (`elapsed >= attempt_timeout`), как требует §6.3;
     верхних границ нет, но S3/S4 опираются на то, что стаб прочитает запрос до истечения
     короткого таймаута (80–150 мс): на сильно нагруженной машине теоретически возможен
     «сдвиг счётчика»; в 8 прогонах подряд (в т.ч. параллельно с чужими тестами) не
     наблюдалось — флаки не зафиксировано.
  2. `Drop` стаба: `accept.abort()` + `abort()` всех сохранённых `JoinHandle`; остаётся
     теоретическое окно, когда отменённая accept-задача успевает положить уже созданный
     обработчик в список после вычитки (задача всё равно снимается при выключении runtime
     теста) — на поведение кейсов не влияет; зафиксировано как best-effort.
  3. 429/5xx ретраями не управляются (вне волны, P1-09) — соответствует §6.1; реальный
     TLS-транспорт — открытый вопрос владельцу, политика к нему подключается без изменения
     тестов.
  4. Вопросов к координатору нет: контракт §6.1 и кейсы §6.3 реализованы буквально.
