# PLAT-02-PLAN — live keyring-кейсы не должны быть «passed без skip»

- batch/карточка: RW-40 / PLAT-02 (`QUALITY/PLATFORM-AUDIT.md` §3, P1), G4 QA, PM — DeepSeek.
- дата: 2026-10-24. **Статус: план, read-only.** Код не менялся, live/сеть/окно/commit не
  использовались. Все утверждения ниже — из чтения файлов на HEAD.
- Вход: `QUALITY/PLATFORM-AUDIT.md:180` (PLAT-02), `rust/crates/discord-platform/tests/keyring_contract.rs`
  (md5 `49e339786446173b9d71997826d4dbf4`), `TEST-POLICY.md` (md5 `eb9087faa42bf89ac3de62ef97a24bbc`),
  `scripts/gate.mjs` (md5 `27d7d39e7d54ec53bdfe5ca71e0408c1`, baseline после QA4-02).

## 1. Почему `return` в тесте не создаёт skip и gate видит passed

Факт из кода (`keyring_contract.rs:37-77`, `184-302`):

```rust
fn skip(reason: &str) { println!("SKIP: {reason}"); }      // :37 — только stdout
fn keyring(require_live: bool) -> Option<(Keyring, MutexGuard<'static, ()>)> {
    if require_live && !live_enabled() { skip(...); return None; }   // :62-65
    ...
}
#[test] fn k01_...() { let Some((keyring, _guard)) = keyring(true) else { return; }; ... }
```

1. **В stable-libtest нет динамического skip.** Единственный способ, который libtest и cargo
   понимают как «не выполнялся» — атрибут `#[ignore]` (исполнение по `--ignored`). Ранний
   `return` — это обычное завершение функции: тест **выполнен успешно**, `passed += 1`.
2. **`println!("SKIP: …")` — не маркер, а текст.** Его не парсит ни libtest, ни `gate.mjs`:
   `cargoTestCounts` (`gate.mjs:100-114`) читает только строки
   `test result: … N passed; M failed; K ignored`. При `return` строка выглядит как
   `test result: ok. 12 passed; 0 failed; 0 ignored` → `skipped: 0`, вердикт `passed`
   (`gate.mjs:397`). Вывод теста в tail фазы не попадает вовсе — tail печатается только для
   упавших фаз (`gate.mjs:381-384`).
3. **Причина пропуска невидима приёмке**: в отчёте нет ни `skipped`, ни списка кейсов, ни
   причины. Это прямо нарушает `TEST-POLICY.md` §2.3 («прятать падения: skip… console.log
   вместо assert») и §9 DoD («skip только с причиной и не на дисплее»).
4. **Проблема шире k01–k08.** k09–k12 вызывают `keyring(false)`, но `Keyring::connect()`
   всё равно нужен сервис (`keyring.rs:97`), а `store`/`lookup`/`clear` валидируют ключ уже
   после получения экземпляра. Значит в гейте без сервиса **не выполняется ни один из 12
   кейсов**, при этом лог говорит `12 passed; 0 ignored`. Это и есть evidence PLAT-02.
5. **Опасность не только в отчётности.** Ранний `return` одинаково «зеленит» три разных
   состояния: «live не включён», «сервиса нет», «`secret-tool` нет». Различить их по логу
   невозможно, а security-кейс `k02` (leak, 2000 итераций) и `k12` (redaction) выглядят
   покрытыми, не будучи запущенными ни разу.

## 2. Как правильно: один маркер, три различимых состояния

### 2.1 Один маркер

- Маркер — **`#[ignore = "live-keyring: <причина>"]`** на каждом из 12 кейсов. Причина
  статична (её видно в `cargo test -- --list --ignored` и в отчёте волны), а не вычисляется
  в рантайме.
- Хелпер `skip()` **удаляется**; конструкции `let Some(x) = ... else { return; }` и
  `if !has_secret_tool() { skip(...); return; }` в тестах запрещены (ловится meta-тестом §3).
- Default-прогон (`cargo test -p discord-platform --test keyring_contract`) не запускает
  ни одного live-кейса и печатает `0 passed; 0 failed; 12 ignored` → gate показывает
  `skip 12` и вердикт `passed_with_skips` + warning. «Нет среды» перестаёт быть
  рантайм-состоянием теста и становится видимым состоянием гейта.

### 2.2 Один helper, который в live-фазе падает, а не молчит

В live-фазе (`--ignored`) каждый кейс обязан получить экземпляр или **упасть** с типизированной
причиной. Один вход, три префикса (grep-контракт):

```
KEYRING-LIVE: not-enabled: DISCORD_LAB_KEYRING_LIVE != 1 (ожидалось 1)
KEYRING-LIVE: service-unavailable: Keyring::connect: <code>: <message>
KEYRING-LIVE: tool-missing: secret-tool не найден в PATH
```

| состояние | где возникает | поведение |
|---|---|---|
| **нет среды** (в дефолтном гейте) | worker/release без live | `#[ignore]` → `ignored=12` в счёте гейта, список кейсов в отчёте |
| **live не включён** | `--ignored` без `DISCORD_LAB_KEYRING_LIVE=1` | `panic!("KEYRING-LIVE: not-enabled: …")` → фаза `failed` |
| **сервис недоступен** | `--ignored`, флаг есть, connect error | `panic!("KEYRING-LIVE: service-unavailable: {code}: {message}")` → `failed` |
| **нет `secret-tool`** (k07/k08) | `--ignored`, PATH без CLI | `panic!("KEYRING-LIVE: tool-missing: …")` → `failed` |

Дополнительно фаза `keyring-live` делает **preflight** (env + `command -v secret-tool` +
print `secret-tool --version`) до запуска cargo, чтобы «tool-missing» и «not-enabled» не
маскировались одним упавшим кейсом, а попадали в `note`/tail фазы.

Правило: `require_live` больше не параметр, который возвращает `Option`. Либо кейс помечен
`#[ignore]` (и тогда его запускает только live-фаза), либо он не зависит от сервиса вовсе.
Третьего пути нет.

### 2.3 Что делать с k09–k12 (нельзя «потерять» security)

k09/k10/k11 требуют `Keyring`, k12 проверяет `Debug` самого `Keyring` ⇒ без сервиса они
непроверяемы, пока API заморожен. Поэтому:

- k01–k12 **переносятся в live-набор целиком** (12 × `#[ignore]`), без удаления кейсов,
  без изменения `RSS_LIMIT_KILOBYTES = 4 * 1024`, `LEAK_ITERATIONS = 2000`,
  `LEAK_SECRET_BYTES = 4096` и без единого ослабленного assert.
- Офлайн-проверяемая часть инварианта redaction выносится в **новый**
  `rust/crates/discord-platform/tests/keyring_offline_contract.rs` (не заменяет k12, а
  дополняет; k12 остаётся в live-наборе):
  - `kr01` — `KeyringError::InvalidKey`/`InvalidSecret`: `Debug` и `Display` не содержат
    значения секрета и не печатают `0x`;
  - `kr02` — `SCHEMA_NAME == "com.discordlab.Token"` (значение контракта схемы);
  - `kr03` — `From<FfiError>` → `KeyringError::Secret { code, message }` без секрета.
  Эти три идут в дефолтном гейте (`ignored = 0`) — security-инвариант остаётся под гейтом
  даже без live.
- **Отклонённая по умолчанию альтернатива:** `#[doc(hidden)] pub fn offline_for_tests()`
  либо `#[cfg(test)]`-конструктор `Keyring` без `Service`. Это меняет прод-структуру
  (`Keyring{service, schema}`/`Service` с `NonNull`) ради тестов и выходит за рамки PLAT-02
  (карточка про маркировку, не про API). Допустимо только письменным решением владельца
  `discord-platform` (G5/G1) отдельной карточкой.

### 2.4 «Не ослаблять security-кейсы» — проверяемые признаки

- Ни один из `k01…k12` не удалён и не переименован; число `#[test]` в файле остаётся 12.
- Ни один assert/threshold/RSS-порог не изменён; `k02` по-прежнему выполняет 2000 lookup
  по 4096 байт.
- `keyring-live` — **обязательная** фаза release/qualify; waiver на неё невозможен
  (`TEST-POLICY.md` §6: запрещены waiver'ы на безопасность и на красный gate).
- В отчёте волны — явный список 12 пропущенных кейсов с причиной (мера PLAT-02).
- Мутация (для приёмки): «вернуть `return` в k02» → meta-тест T1 красный; «убрать
  `--ignored` из фазы» → T2 красный.

## 3. Red-first: meta-тест и требования к отчёту гейта

Новый файл `tests/meta/keyring-live.test.mjs` (owner G4, не пересекается с
`test-policy.test.mjs`, который правит параллельная линия). Три теста, все три **сейчас
красные по правильной причине**:

**T1 — «нет раннего return под видом skip»** (RED сейчас: 12 test / 0 ignore)
- разбор `keyring_contract.rs`: нет `fn skip(`, нет вызовов `skip(`; в телах функций с
  `#[test]` нет `return` (кроме `-> ()`-нормы, т.е. pat-return `return;` запрещён);
- для каждого `#[test]` следующий атрибут — `#[ignore = "live-keyring: …"]`;
- `count(#[test]) === 12` и `count(#[ignore]) === 12`.
- RED-сообщение: `k01..k08 завершаются return без skip-маркера; ignored=0`.

**T2 — «гейт печатает skipped>0 и live-фаза не пропускает»** (RED сейчас: фазы нет)
- в `scripts/gate.mjs` существует фаза `keyring-live`:
  `cargo test --offline -p discord-platform --test keyring_contract -- --ignored`,
  `env: { DISCORD_LAB_KEYRING_LIVE: "1" }`, `failOnSkips: true`, `timeoutMs` как у
  `workspace-rust`, preflight (env + `secret-tool`);
- фаза **не** входит в `commonPhases` (worker live не запускает) и добавляется в
  release/qualify рядом с `e2e`/`coverage`;
- `failOnSkips` реализован так: `skipped > 0` ⇒ `result.ok = false` (вердикт `failed`),
  а не только warning;
- `cargoTestCounts` продолжает считать `ignored → skipped` (регресс-assert: синтетическая
  строка `test result: ok. 0 passed; 0 failed; 12 ignored` → `{tests:12, skipped:12}`,
  вердикт `passed_with_skips`);
- assert на текст отчёта: строка `skip 12` в выводе фазы `workspace-rust` и warning
  «пропущено 12 тестов — пропуск прохождением не считается».

**T3 — «артефакт cargo подтверждает ignored»** (RED сейчас: `12 passed; 0 ignored`)
- `cargo test --offline -p discord-platform --test keyring_contract -- --list --ignored`
  → ровно 12 строк `kNN…: test`, имена совпадают с ожидаемым списком k01…k12;
- `cargo test --offline -p discord-platform --test keyring_contract` → в выводе
  `test result: ok. 0 passed; 0 failed; 12 ignored`;
- тест **не** имеет права на skip при отсутствии cargo (гейт всё равно требует cargo для
  `workspace-rust`); при отсутствии cargo — fail с явной причиной, не skip.
- Логи: `waves/RW-40/QUALITY/plat-02/{red,green}-t{1,2,3}.log`.

Почему T3 в meta, а не «посмотреть глазами»: именно этот assert закрывает разрыв
«тест печатает SKIP» → «libtest считает ignored» и не даёт вернуться к `return`.
Стоимость — один уже собранный бинарник `discord-platform` (фазы гейта идут
последовательно, конфликта за `target/` нет).

## 4. Файлы, owner, DoD, риски

### 4.1 Файлы и владельцы

| файл | изменение | owner |
|---|---|---|
| `rust/crates/discord-platform/tests/keyring_contract.rs` | 12 × `#[ignore = "live-keyring: …"]`; удалить `skip()` и все `return`-гейты; хелпер `live_guard()` с тремя `KEYRING-LIVE:`-причинами; пороги/ассерты не трогать | G4 QA (PLAT-02) |
| `rust/crates/discord-platform/tests/keyring_offline_contract.rs` | **new**: `kr01…kr03` (redaction ошибок, `SCHEMA_NAME`, `From<FfiError>`), без live | G4 QA |
| `tests/meta/keyring-live.test.mjs` | **new**: T1–T3 | G4 QA |
| `scripts/gate.mjs` | фаза `keyring-live` (release/qualify, не worker), `failOnSkips`, preflight; `cargoTestCounts` не менять | G4 QA — **только после** приёмки QA4-02 |
| `waves/RW-40/QUALITY/BUGS.md`, `TASKS.md`, `waves/STATE.md`, `waves/db.jsonl` | закрытие PLAT-02 | Team Lead (не воркер) |

`keyring.rs`/`ffi_secret.rs` (прод) **не трогаются** — API заморожен.
Файл `tests/meta/test-policy.test.mjs` не трогается (параллельная линия, untracked).

### 4.2 DoD

1. T1–T3: RED-логи до правки, GREEN после; T1–T3 зелёные, `0 skipped`.
2. `cargo test -p discord-platform --test keyring_contract` → `0 passed; 0 failed; 12 ignored`;
   `cargo test -p discord-platform --test keyring_offline_contract` → `3 passed; 0 ignored`.
3. `npm run gate:worker` → `workspace-rust: ✔ (skip 12)` + warning, `keyring-live` в списке
   фаз **отсутствует**; вердикт `passed_with_skips`, а не `passed`.
4. `gate:release` в согласованном live-окне → фаза `keyring-live` ✔ (`LIVE=1`,
   `org.freedesktop.secrets` поднят, коллекция разблокирована, `secret-tool` есть),
   12/12 passed, `ignored=0`, `failOnSkips` не сработал.
5. Негативная проверка live-фазы (в том же окне): без `LIVE=1` → `failed` с
   `KEYRING-LIVE: not-enabled`; с погашенным сервисом → `failed` с
   `KEYRING-LIVE: service-unavailable` (логи в `plat-02/`).
6. Отчёт волны содержит список 12 ignored-кейсов и причину; диф тестов проверен не автором;
   md5-таблица совпадает; пороги coverage (line/branch 90%) не изменены.

### 4.3 Риски

| # | риск | контроль |
|---|---|---|
| 1 | Release блокируется без live-окна | это цель карточки: security-набор обязателен в окне; waiver запрещён TEST-POLICY §6 |
| 2 | `secret-tool` отсутствует на gate-хосте | preflight с `tool-missing` до cargo; ставится в окне, не «пропускается» |
| 3 | Привычный вердикт `passed` становится `passed_with_skips` (12 ignored) | ожидаемое изменение семантики; фиксируется в отчёте и в `note` фазы |
| 4 | Коллизия в `scripts/gate.mjs` с QA4-02/QA4-03 | брать md5 `27d7d39e…` как baseline, править после приёмки, диф — только аддитивный |
| 5 | k01/k05 могут повиснуть на prompt разблокировки коллекции (известное ограничение libsecret) | окно только с разблокированной коллекцией; `timeoutMs: 900000` уже в фазе; таймаут не увеличивать |
| 6 | k07/k08 зависят от версии CLI | версия `secret-tool` печатается в preflight-логе |

### 4.4 Что нельзя проверить без live (честный список)

`Keyring::connect` к реальному `org.freedesktop.secrets`; store/lookup/clear roundtrip
(k01, k03–k06); **утечка k02** (RSS на 2000 lookup); interop с CLI (k07, k08);
double-clear (k11); `Debug`-redaction самого `Keyring` и запись/чтение реального секрета
(вторая половина k12); поведение при заблокированной коллекции. Без live доказуемы только:
маркировка/счётчики гейта (T1–T3), redaction ошибок `KeyringError` и `SCHEMA_NAME` (kr01–kr03),
и статическое отсутствие `skip()`/`return`-гейтов.

## 5. Связь с QA4-02 (workspace/coverage) и границы

- PLAT-02 **опирается** на QA4-02: `workspace-rust` с `countsKind: "cargo"` и
  `cargoTestCounts(ignored → skipped)` — единственный механизм, через который 12 ignored
  станут видимыми (`skip 12`, warning, `passed_with_skips`). Без QA4-02 правка была бы
  незаметна.
- **Пороги не меняются:** `coverage` (line/branch 90%), `RSS_LIMIT_KILOBYTES`, e2e-поведение,
  `timeoutMs` фаз. Новые тесты (`keyring_offline_contract`, meta) не входят в `test:coverage`
  и не двигают проценты.
- `keyring-live` добавляется только в release/qualify — worker-гейт остаётся быстрым и
  офлайновым; число фаз worker не меняется.
- QA4-03 (coverage CLI) файлов PLAT-02 не касается; `src/cli/run.mjs`, `tests/unit/**` не трогаем.
- Итоговый статус release = `keyring-live ✔` **и** `passed_with_skips` по worker-набору;
  «зелёный» без live-окна для release недействителен.

## 6. Пять шагов исполнения

1. **RED**: написать `tests/meta/keyring-live.test.mjs` (T1–T3), прогнать, сохранить логи
   `plat-02/red-t{1,2,3}.log` (ожидаемо: `12 test / 0 ignore`, фазы нет, `12 passed`).
2. `keyring_contract.rs`: 12 × `#[ignore = "live-keyring: …"]`, удалить `skip()`/`return`,
   ввести `live_guard()` с префиксами `KEYRING-LIVE: {not-enabled|service-unavailable|tool-missing}`;
   добавить `keyring_offline_contract.rs` (kr01–kr03). Ассерты/пороги не менять.
3. `scripts/gate.mjs`: фаза `keyring-live` (release/qualify, `--ignored`, `env LIVE=1`,
   `failOnSkips: true`, preflight) — после приёмки QA4-02, аддитивным дифом.
4. **GREEN + targeted**: T1–T3 зелёные; `--list --ignored` = 12; default = `0 passed;
   12 ignored`; `gate:worker` = `passed_with_skips`, `skip 12`, без `keyring-live`.
5. **Приёмка**: live-окно (`LIVE=1`, сервис+CLI), `gate:release` с `keyring-live ✔` (12/12,
   ignored=0), негативные прогоны `not-enabled`/`service-unavailable`, отчёт с md5 и списком
   ignored; закрытие PLAT-02 делает Team Lead.
