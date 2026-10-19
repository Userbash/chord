# Отчёт RW-07 — роутер диспатчей (P1-06 / F-GW-06 / T-GW-08)

- **сделано:** `dispatch.rs` — `DispatchKind` (21 вариант, `ALL`, `from_name`/`as_str`, точное
  регистрозависимое имя провода), `DispatchEvent` (`kind`/`name`/`seq`/`data` без разбора `d`),
  `route_dispatch` по контракту §1.1 (op0 строго `Int(0)` → `t` только атом/строка → вид →
  `s` как `u64` → `d` целиком, `Nil` при отсутствии); `ws.rs` — поля `events`/`unknown_dispatches`,
  вызов роутера в `push_binary` после блока op9 и до `tail.drain`, `events()`/`take_events()`
  (`std::mem::take`)/`unknown_dispatches()` (накопительный); `lib.rs` — `pub mod dispatch;` по
  алфавиту, ре-экспорт `{DispatchEvent, DispatchKind, route_dispatch}`, одно упоминание RW-07 в
  doc-шапке. Тест-файл `tests/dispatch_router.rs` — 11 кейсов (U1–U6, P1–P3, L1–L2),
  самодостаточные хелперы по паттерну соседних тестов, lab skip-aware.
  Оракулы перепроверены независимо до кода (`verify-oracle.mjs`, node): union 7 файлов —
  1340 op0 / 21 тип, потиповые §3.1 сошлись 1:1; префикс s ≤ 36 — 36 op0, потиповые §3.2 сошлись;
  `.hexl` — 137 кадров / 104 066 сжатых Б.
- **red:** `cargo test --offline -p discord-core --test dispatch_router` → **3 passed / 8 failed**,
  `# EXIT=101`; падают U1, U3, U4, U6, P1, P2, L1, L2; проходят негативные контроли U2 (пустой `ALL`
  гасит цикл), U5, P3 — ровно как предписано ТЗ. md5 теста `4fdfe8a89ceb649da949e17f5092f0c7`,
  побайтовая копия `/tmp/opencode/rw-07/dispatch_router-red.rs` (md5 тот же). Нит процесса: первый
  red был снят до `cargo fmt` (md5 `052b8015…`); после форматирования теста red **переснят** на
  финальной редакции — в `red.log` лежит финальный прогон, md5 совпадает с финальным тестом.
  Побочно: первый red уже подтвердил байтовые утверждения L2 (`out.len()` и разбор до первого op0) —
  падение случилось на `route_dispatch`, а не на числах.
- **green:** `green.log` (все шаги с `# EXIT=`), перед прогонами — `touch` четырёх файлов
  (инцидент залипшего rlib, `TASKS.md:678`):
  1. `cargo test --offline -p discord-core --test dispatch_router` → **11/11**, `# EXIT=0`;
  2. `cargo test --offline -p discord-core` → **70** (59 существующих не сломаны), `# EXIT=0`;
  3. `cargo test --offline --workspace` → **112**, `# EXIT=0`;
  4. `dispatch_router` ещё ×4 → все 11/11, `# EXIT=0` (итого ×5 стабильно);
  5. `cargo clippy --offline --workspace --all-targets -- -D warnings` → чисто, `# EXIT=0`;
  6. `cargo fmt --check` → чисто, `# EXIT=0`.
  md5 после green: `dispatch.rs` `d929b0229def68b378ed2212320aa162`, `ws.rs`
  `fa6e051a9b3f1c8bdeb18c101514d639`, `lib.rs` `fa07ceb6bb50527a33c6abd27d5642ae`,
  `tests/dispatch_router.rs` `4fdfe8a89ceb649da949e17f5092f0c7` (= red-копия).
- **gate:worker:** **passed** — `.gate/worker-2026-10-19T15-51-24-148Z.json` (typecheck / meta 22 /
  unit 169 / smoke 20 — всё зелёное).
- **файлы:** `rust/crates/discord-core/src/dispatch.rs` (новый),
  `rust/crates/discord-core/src/ws.rs`, `rust/crates/discord-core/src/lib.rs`,
  `rust/crates/discord-core/tests/dispatch_router.rs` (новый); артефакты
  `/tmp/opencode/rw-07/{red.log, green.log, dispatch_router-red.rs, report.md, verify-oracle.mjs,
  green-run.sh, green-src/, md5-green.txt}`. Границы: в `rust/` свежие mtime только у этих 4 файлов;
  `Cargo.toml`/`Cargo.lock` не тронуты; `TASKS.md`/`docs/**`/чужие тесты — без изменений.
- **риски/вопросы:**
  1. Окно ruzstd (RW-02, нит P1-02): из 824 816 Б распаковки доступно 759 280; в L2 — 44 полных
     терма (s = 1..36) и 704 Б обрезанного терма в хвосте, тогда как в полном потоке 113 диспатчей
     (s = 1..113, независимый оракул — таймлайн). Это ограничение декодера, не роутера; в тесте
     закомментировано.
  2. **T-DATA-01 (разбор READY-гильдии) в эту волну не входит** — payload `d` не разбирается и
     отдаётся вызывающему целиком; разбор — P2-01.
  3. Буфер `events` растёт линейно с потоком (контракт: вызывающий обязан дренировать
     `take_events()`); счётчик `unknown_dispatches` накопительный и дренажом не сбрасывается —
     по контракту §1.3.
  4. Противоречий в ТЗ не обнаружено: все lab-числа §3.1/§3.2 сошлись при независимой проверке
     и в зелёных прогонах (L1/L2).
