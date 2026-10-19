# Отчёт W9b — X-Super-Properties: JSON и base64 (RW-09 / P1-08, T-REST-04)

- **сделано:** реализованы `to_json`/`from_json`/`to_base64`/`from_base64`/`field_names` в
  `rust/crates/discord-core/src/rest/super_properties.rs` (структура/сигнатуры замороженного
  каркаса не менялись; добавлены приватные хелперы записи и однопроходный строгий разборщик
  `Parser`). Тест `rust/crates/discord-core/tests/rest_super_properties.rs` — кейсы P1–P8 (8
  тестов): 22 поля и порядок `field_names()` = колонка 1 TSV/оракул §6.2; разбор живого JSON по
  TSV (значения и типы: 18 строк, 2 `u32`, 1 bool, 1 null); побайтовый `to_json(from_json(live))`
  == выдержка `.json` (811 байт) + идемпотентность + whitespace между токенами; base64-круг
  с живым заголовком (1084 симв., `==`) через `super::base64`; экранирование RFC 8259
  (`\"`, `\\`, `\n`, `\t`, `\b`, `\f`, `\r`, прочие `<0x20` → `\u00XX`, юникод `ё`/🦀 — как есть)
  + разбор `\uXXXX` и суррогатных пар; строгий отказ (пропуск/лишний/дубликат ключа, типы,
  `null` не для `client_event_source`, запятая в конце/двойная, мусор до/после, незакрытое
  и сырой control в строке, негодный escape) с непустым сообщением и позицией; границы `u32`
  (`0`, `4294967295`, переполнение/минус/дробь/экспонента/ведущий ноль); типизированные ошибки
  base64-слоя и не-UTF-8 → `Json(...)`.
- **red:** `cd rust && cargo test --offline -p discord-core --test rest_super_properties` →
  `# EXIT=101`, `test result: FAILED. 0 passed; **8 failed**` (P1–P8 падают на заглушке:
  `field_names()` = пустые строки, `to_json` → `""`, `from_json`/`from_base64` → `Err`);
  лог `/tmp/opencode/rw-09/w9b/red.log`, md5 теста на red
  `34dc62487dfb99553d7f99449fb67911`, копия
  `/tmp/opencode/rw-09/w9b/rest_super_properties-red.rs`. Тест после red не менялся
  (`diff -q` red-копии и текущего файла чист; md5 совпадает).
- **green:** тот же таргет ×5 — `# EXIT=0`, «8 passed; 0 failed» каждый прогон
  (`green-x5.log`, повтор после rustfmt — `green-fmt.log`); `cargo test --offline -p discord-core`
  — `# EXIT=0`, **116 passed / 0 failed** (13 таргетов + doc-tests; чужих падений нет);
  `rustfmt --edition 2024 --check` моих двух файлов — чисто; `cargo clippy --offline -p
  discord-core --all-targets -- -D warnings` — `# EXIT=0`, без замечаний (после `touch` файлов,
  чтобы не использовать чужой кэш).
- **gate:worker:** `npm run gate:worker` → **passed** (typecheck; meta 25/25; unit 169/169;
  smoke 20/20; без skip), отчёт `.gate/worker-2026-10-19T20-32-18-052Z.json`
  (лог `/tmp/opencode/rw-09/w9b/gate-worker.log`).
- **файлы (только эти два):**
  - `rust/crates/discord-core/src/rest/super_properties.rs` — md5 `debbb208b8bb4d10f2e2c8d8fd0fc2d8`
    (до работы, заморожен: `c463b954b8c265f4e9f773e91d6ae153`; сигнатуры/поля/enum не менялись);
  - `rust/crates/discord-core/tests/rest_super_properties.rs` — md5 `34dc62487dfb99553d7f99449fb67911`
    (создан; после red не менялся).
  - границы: `Cargo.*` не тронуты; `rest/mod.rs` `0eba823bf46c2958cac102dbb42cbb06` (заморожен);
    тест-данные `b9e8f843…`, `90b06ded…`, `95e2f861…`, `8b5bb8c9…` — как в ТЗ; `base64.rs`
    (W9a) и `identity.rs` (W9c) не редактировались; git-мутаций не было.
- **риски/вопросы:**
  1. **Порядок проверок в base64 (W9a) против текста ТЗ §4.** В коде W9a проверки идут
     «паддинг → символы данных → длина», а не буквально «длина → …». Это единственный способ
     одновременно удовлетворить B3 (`"Zm9v\n"` — 5 байт → `InvalidChar`, не `InvalidLength`)
     и P8 (`"!!!"` → `InvalidChar`). Мои P4/P8 фиксируют фактический контракт (P8 = формулировка
     ТЗ §5), поэтому расхождения в тестах нет; текст §4 стоит поправить на приёмке
     (вопрос координатору).
  2. `\u00XX` для управляющих — нижний регистр hex (RFC 8259 допускает оба); тест сверяет без
     учёта регистра, контракт не сужен.
  3. md5 выдержек в Rust-тесте не пересчитывается (нет md5-крейта; новых зависимостей нельзя):
     оракулом служат сами байты выдержек, md5 зафиксированы в шапке теста и сверяются W9r/гейтом.
  4. Парсер строго JSON: отвергает ведущие нули/`+`/экспоненту и сырые control-символы — это
     часть «строгого разбора», живых данных не касается (живой JSON разбирается побайтово).
