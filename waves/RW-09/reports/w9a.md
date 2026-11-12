- сделано: base64 RFC 4648 вручную (без крейтов) — `encode` (канонический, `=`-паддинг) и
  `decode` с типизированными ошибками (`InvalidLength`/`InvalidChar`/`InvalidPadding`) в
  `rust/crates/discord-core/src/rest/base64.rs`; полный тест-файл
  `rust/crates/discord-core/tests/rest_base64.rs` — кейсы B1–B5 (10 python3-векторов из
  `tests/data/rest-base64-vectors.tsv`, живой заголовок `rest-super-properties-2323.b64`
  1084 симв. → 811 байт, типизированные ошибки, длины/паддинг, неканонические хвостовые биты).
  Проверено: целевой прогон ×5 + ×5 после fmt, сверка с python3-оракулом (снятие md5 — отдельно,
  в провенансе теста), полный прогон крейта, clippy, gate.
- red: `cd rust && bash -c 'cargo test --offline -p discord-core --test rest_base64 2>&1 | tee
  /tmp/opencode/rw-09/w9a/red.log; echo "# EXIT=${PIPESTATUS[0]}" | tee -a ...'`
  → `# EXIT=101`; `test result: FAILED. 0 passed; 5 failed` (B1–B5, падения ассертами заглушки:
  `encode` → `""`, `decode` → `Err(InvalidLength(0))`); md5 теста в red
  `b560515213739c74d0aa8b4d30f17cfc`; red-копия — `/tmp/opencode/rw-09/w9a/rest_base64-red.rs`.
  (Первый прогон сломался на опечатке формата `}` в сообщении ассерта — поправлено до
  зафиксированного red; сам red перезапущен и записан заново.)
- green: тот же прогон ×5 подряд → `# EXIT=0`, `5 passed; 0 failed` каждый
  (`green-1..5.log`); после `rustfmt` — ещё ×5 (`green-final-1..5.log`), все `# EXIT=0`, 5/5.
  Md5: src `f240f0695c512ae7d06159ec1ebb2cec`, тест `f691aef64184eb47aa22df259b2584b8`
  (`/tmp/opencode/rw-09/w9a/md5-final.txt`). Rustfmt после red изменил только форматирование
  теста (md5 `b5605152…` → `f691aef6…`, ассерты не менялись) — см. риски.
- gate:worker: **passed** — typecheck ✔, meta 25/25 ✔, unit 169/169 ✔, smoke 20/20 ✔;
  `.gate/worker-2026-10-19T20-23-43-626Z.json`; лог `gate-worker.log` (`# EXIT=0`).
- файлы: создан только свой тест `rust/crates/discord-core/tests/rest_base64.rs`; реализовано
  тело только своего продукта `rust/crates/discord-core/src/rest/base64.rs` (сигнатуры
  замороженного каркаса не менялись, md5 заглушки `03ddd6ebecd8ed5c727a732ea2b6e246`).
  Чужие/запретные файлы не трогались: `Cargo.toml`/`Cargo.lock`, `mod.rs`
  (`0eba823bf46c2958cac102dbb42cbb06`), тест-данные (`8b5bb8c9…`, `b9e8f843…`, `90b06ded…`,
  `95e2f861…`), `docs/`, `waves/`, golden, `reference-oracle/` — без изменений.
- доп. прогоны: `cargo test --offline -p discord-core` → `# EXIT=101`, но падения чужие —
  in-progress тесты W9c `tests/rest_identity.rs` (`integration_base64_headers`,
  `uuid_v4_shape`); `rest_base64` в этом же прогоне 5/5 ok, остальные таргеты до него зелёные,
  таргеты после (и `rest_super_properties`, которого ещё нет) не выполнялись из-за остановки
  cargo; лог `core-tests.log`. `cargo clippy --offline -p discord-core --all-targets --
  -D warnings` → `# EXIT=0` (`clippy.log`). `rustfmt --check` по двум своим файлам — чисто.
- риски/вопросы:
  1. **Противоречие в ТЗ §4 (порядок проверок).** Проза: «длина → паддинг/символы», но кейс B3
     требует `"Zm9v\n"` (5 байт) → `InvalidChar{byte:b'\n', index:4}` — при проверке длины
     первой это невозможно (`InvalidLength(5)`). Реализовано по B3: паддинг → символы →
     длина; для строк только из алфавита некратная длина даёт `InvalidLength` (`"AA"` → 2), как
     и требует B3. Порядок задокументирован в код-комментарии. Прошу подтвердить B3 как
     источник истины (иначе нужно менять либо B3, либо прозу); важно для мутационного ревью W9r.
  2. Rustfmt обязателен по процедуре, но применён после red — если это считается «правкой
     теста», координатор может перезапустить red с финальным (отформатированным) тестом на
     заглушке: содержимое ассертов не менялось, red-копия сохранена.
  3. Ненулевые хвостовые биты игнорируются (`"Zh=="`/`"Zm9="` → канонизация через `encode`) —
     это заморожено §4 п.4 и покрыто B5.
  4. §8-строку реестра для нового тест-файла по ТЗ правит координатор (docs запрещены воркерам).
  5. Живые REST-запросы не делались; сеть/новые зависимости не использовались; Discord не
     спавнился.
