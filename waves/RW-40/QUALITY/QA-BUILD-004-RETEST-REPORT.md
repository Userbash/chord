# Повторная приёмка QA — Сборка `BUILD-004` (retest)

- **Задача QA:** `qa/task-135-qa-build-004-retest` (`task_76dafd826adb`, `ctx_14f6df16143f`), терминал `term_f78651a3-a356-4661-b7b7-de83d90baa0c`
- **Роль:** G4 QA Engineer (DeepSeek)
- **Проверяемая сборка:** `/home/user/orca/workspaces/project/testing` (ветка `testing`)
- **Манифест сборки:** [waves/RW-40/BUILDS/BUILD-004.md](file:///home/user/orca/project/waves/RW-40/BUILDS/BUILD-004.md) (`md5: 7a53bfc8dbe78518b53bc1311d1035bf`, 17 строк — не менялся)
- **Предыдущий отчёт:** [QA-BUILD-004-REPORT.md](file:///home/user/orca/project/waves/RW-40/QUALITY/QA-BUILD-004-REPORT.md) — `QA Failed`, `P0 = 3`, `P1 = 2`
- **Основание повторной приёмки:** отчёт о фиксе [task-134-fix-build004/REPORT.md](file:///home/user/orca/project/waves/RW-40/G1/task-134-fix-build004/REPORT.md) + слияние PR-18 (`task-125-test-impact-v2`)
- **Дата и время:** 2026-10-30, 21:36–21:52 +05:00
- **Финальный вердикт:** **QA Passed** — `P0 = 0`, `P1 = 0` в объёме retest. `P1-1` (фиксация состава сборки коммитом/тегом) остаётся за L1 — см. §6.

Три P0 и P1-2 закрыты в рабочем дереве, все обязательные наборы зелёные. Продуктовый код во время QA не изменялся; `logs/`, `functions/`, `evidence/` не трогались.

---

## 1. Окружение и состояние сборки

| Параметр | Значение |
|---|---|
| **Git HEAD** | `4e240481e59ff4d49732ac8bd5ba08a006363748` (`RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence`) — тот же, что в первой приёмке |
| **Ветка** | `testing`; `unstable`, `main`, `qa/task-135-qa-build-004-retest` стоят на том же коммите |
| **Рабочее дерево** | грязное: `217` записей `git status --porcelain`, из них `158` — untracked (`??`); фиксы task-134 и PR-18 лежат поверх HEAD, не закоммичены |
| **Node.js / npm** | `v26.13.0` / npm из состава Node |
| **TypeScript (`tsc`)** | `Version 7.0.2` |
| **Rust** | `rustc 1.99.0 (b940084d7 2026-09-28)` / `cargo 1.99.0 (5f94df478 2026-08-27)` |
| **Общий кэш компиляции** | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target` |
| **Логи прогонов** | `waves/RW-40/QUALITY/qa-build-004-retest-logs/` (7 файлов, md5 в §7) |
| **Оракулы** | `DM-FRIENDS-SPEC.md` (`md5 3aacc65a33ca8a24e609c2046c85625f`), `VOICE-DEVICES-SETTINGS-SPEC.md`, `voice-spec.json`, `main.rs` (`generate_handler!`), `tests/meta/plan-sync.test.mjs` |

Контрольные суммы целевых файлов совпали с финальными значениями из отчёта task-134:
`lib.rs 356efe9e…`, `dm-friends.js 63cd550d…`, `TDD-PLAN.md 591132b4…`. То есть приёмка шла ровно по тому состоянию, которое фикс объявил готовым.

---

## 2. Закрытие дефектов

### P0-1 — `pub mod dm_friends; pub mod voice_devices;` в `rust/gui-spike/src/lib.rs` — **закрыт**

Фактическое содержимое (`rust/gui-spike/src/lib.rs:36-60`):

```rust
pub mod data;
/// RW-40/PR-14 (task-112): BFF связей, ЛС и Group DM — `GET /users/@me/relationships`,
/// `POST /users/@me/channels` (код `80013`), Pomelo и заметки.
pub mod dm_friends;
pub mod guilds;
...
pub mod typing;
/// RW-40/PR-17 (task-124): устройства ввода/вывода, громкости, режимы
/// (`VoiceActivity`/`PushToTalk`), `self_mute`/`self_deaf` и тест микрофона.
pub mod voice_devices;
```

Оба модуля объявлены с док-комментариями в стиле соседних. `cargo test --test dm_friends_contract` компилируется и проходит (6/6, §3.1) — ошибка `E0432 unresolved import discord_gui_spike::dm_friends` больше не воспроизводится.

### P0-2 — fallback `discord_rest_request` в `dm-friends.js` — **закрыт**

`sendRest` (`rust/gui-spike/ui/features/dm-friends.js:1345-1353`) больше не вызывает `invoke`:

```js
const sendRest = async (req) => {
  if (typeof options.request === "function") {
    return options.request(req);
  }
  throw new Error(
    `createDmFriendsController: не передан REST-адаптер request; ` +
      `${req.method} ${req.url} не выполнен`,
  );
};
```

- Поиск `discord_rest_request` по `rust/gui-spike/ui`, `rust/gui-spike/src`, `tests`, `scripts` даёт **единственное** вхождение — комментарий `dm-friends.js:1344` с описанием причины удаления. Литерального `invoke("discord_rest_request", …)` в коде нет.
- Инвариант `INT-07: UI invoke ⊆ generate_handler!` проходит: `✔ INT-07: UI invoke ⊆ generate_handler! (4.57ms)`.
- Выбран вариант «требовать `options.request` явно», а не регистрация команды в `main.rs`; это соответствует второй рекомендации QA (`main.rs` в ТЗ task-134 не входил). Ошибка теперь честная и попадает в `reportActionError`, а не превращается в тихий `{status: 200, ok: true}`.

### P0-3 — `tests/unit/dm-friends-ui.test.mjs` в `TDD-PLAN.md` §8 — **закрыт**

Строка `TDD-PLAN.md:373`:

```text
| `tests/unit/dm-friends-ui.test.mjs` | `rust/gui-spike/ui/features/dm-friends.js` | `createDmFriendsController`, `DM_FRIENDS_ENDPOINTS`, `FRIENDS_TABS`, `applyDmFriendsDispatch`, `sortPrivateChannels` |
```

Мета-гейт `plan-sync` зелёный целиком, включая проверку «каждый тестовый файл описан в §8 ровно один раз»:

```text
✔ plan-sync: §8 ↔ тестовые файлы ↔ модули (25.38ms)
  ✔ таблица §8 разбирается и не пуста
  ✔ каждая строка §8 указывает на существующий тест; токены встречаются в нём
  ✔ каждый тестовый файл (unit/meta/smoke/e2e) описан в §8 ровно один раз
  ✔ модули из §8 существуют; каждый src/**/*.mjs приписан к строке
  ✔ строки с одним src-модулем: токены покрывают все экспортируемые функции
```

### P1-2 — прямой импорт `voice_devices` в контракте — **закрыт**

`rust/gui-spike/tests/voice_devices_contract.rs:16-25`:

```rust
use discord_gui_spike::voice_devices::{
    is_modifier_key, sensitivity_percent_to_threshold_db, threshold_db_to_sensitivity_percent,
    AudioDevice, AudioDeviceKind, DeviceCatalog, DeviceEvent, DeviceSelectError, MicTestState,
    ...
};
```

- `#[path = "../src/voice_devices.rs"]` удалён; поиск `#[path` по `rust/gui-spike/tests` не находит ни одного вхождения.
- Контракт идёт через крейт `discord_gui_spike`, то есть теперь падение «модуль не подключён» поймал бы сам тест — дыра, из которой вырос `P0-1`, закрыта на уровне теста.
- `voice_devices_contract` проходит 15/15.

---

## 3. Прогоны

Все команды выполнены в `/home/user/orca/workspaces/project/testing`, коды возврата сняты напрямую с команд.

### 3.1 Контракты Rust

```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test \
  --manifest-path rust/gui-spike/Cargo.toml \
  --test dm_friends_contract --test voice_devices_contract
```

```text
running 6 tests
test error_display_and_debug_never_leak_token_or_sensitive_headers ... ok
test rest_error_responses_preserve_discord_code_on_403_and_404 ... ok
test rest_relationships_put_delete_post_pomelo_and_stranger_80013_contract ... ok
test rest_create_dm_and_group_dm_validates_snowflakes_and_recipients_bounds ... ok
test ready_relationships_parsing_and_tabs_filtering_contract ... ok
test ready_private_channels_and_rest_static_oracle_parity ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 15 tests
test t01_initial_state_and_default_selection_ignores_list_order ... ok
...
test t15_debug_after_device_fallback_does_not_leak_os_device_labels ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Код `0`.** `6/6` и `15/15` — совпадает с ожиданием.

### 3.2 UI-наборы

| Набор | Suites | Tests | Pass | Fail | Код |
|---|---:|---:|---:|---:|---:|
| `tests/unit/dm-friends-ui.test.mjs` | 4 | 19 | 19 | 0 | 0 |
| `tests/unit/gui-spike-features.test.mjs` | 15 | 111 | 111 | 0 | 0 |
| `tests/unit/gui-spike-dom.test.mjs` | 3 | 38 | 38 | 0 | 0 |
| `tests/unit/gui-spike-regressions.test.mjs` | 4 | 13 | 13 | 0 | 0 |

`INT-07` и `INT-07-UI-APP-MOUNT` в `gui-spike-features` — `ok`.

**Расхождение со счётчиком из ТЗ:** в ТЗ retest указано `gui-spike-features.test.mjs (140/140)`. Фактически в файле `111` тестов, все зелёные; md5 файла `f2f86fc8…` не менялся с первой приёмки, где QA уже фиксировал 111. Цифра `140` — это заявка PR-16 в `BUILD-004.md` (и агрегат из отчёта `b2b-attach-ui`: `dom 38 + features ? + regressions ?`), а не фактическое число тестов файла. Считаю это остатком `P2-2` (см. §5), на вердикт не влияет: набор полностью проходит.

### 3.3 Мета-оракулы

```sh
node --test tests/meta/*.test.mjs
```

`tests 102 / suites 17 / pass 102 / fail 0 / skipped 0`, **код `0`**. `plan-sync` проходит (§2, P0-3).

### 3.4 PR-18 `test-impact` v2

```sh
node --test tests/unit/test-impact.test.mjs
```

`tests 53 / suites 9 / pass 53 / fail 0 / skipped 0`, **код `0`**.

### 3.5 Дополнительно: полный unit-прогон и типы

| Команда | Результат | Код |
|---|---|---|
| `npm run test:unit` | `tests 977 / suites 184 / pass 977 / fail 0 / skipped 0` | 0 |
| `npm run typecheck` (tsc 7.0.2) | пустой вывод | 0 |

Полный unit вырос с `933/934` (первая приёмка) и `934/934` (после task-134) до `977/977`. Прирост `+43` даёт ровно PR-18: `tests/unit/test-impact.test.mjs` вырос с 10 до 53 тестов. Новых падений нет.

---

## 4. Сводка по объёму ТЗ

| # | Проверка | Ожидалось | Факт | Код |
|---|---|---|---|---|
| 1 | `dm_friends_contract` | 6/6 | 6/6 | 0 |
| 2 | `voice_devices_contract` | 15/15 | 15/15 | 0 |
| 3 | `dm-friends-ui.test.mjs` | 19/19 | 19/19 | 0 |
| 4 | `gui-spike-features.test.mjs` | 140/140 (по ТЗ) | 111/111 (файл не менялся; 140 — устаревшая заявка манифеста) | 0 |
| 5 | `test-impact.test.mjs` (PR-18) | 53/53 | 53/53 | 0 |
| 6 | `tests/meta/*.test.mjs` | 102/102 | 102/102 | 0 |
| 7 | `npm run test:unit` (доп.) | — | 977/977 | 0 |
| 8 | `npm run typecheck` (доп.) | 0 ошибок | 0 ошибок | 0 |

Все обязательные проверки зелёные, коды возврата `0`.

---

## 5. Остаточные замечания (не блокируют Passed)

| ID | Уровень | Что осталось | Кому |
|---|---|---|---|
| **P1-1** | перенос | Состав `BUILD-004` не зафиксирован в git: HEAD `4e24048`, `158` untracked-файлов (включая `dm_friends.rs`, `voice_devices.rs`, оба контракта, `dm-friends.js`, `dm-friends-ui.test.mjs`), `lib.rs`/`TDD-PLAN.md` — `M`, тега `build-004` нет. Собрать сборку из git-ссылки по-прежнему нельзя. | L1 / DevOps на релизном гейте (по решению координатора — вне объёма retest) |
| **P2-1** | документация | `BUILD-004.md` так и остался 17 строк: только таблица PR, без §2 (команды/коды) и §3 (оракулы). `md5` не менялся. | PM |
| **P2-2** | документация | Счётчик `140/140 unit` в манифесте не соответствует фактическим `111/111` в `gui-spike-features.test.mjs`. | PM |
| **P3-1** | наблюдение | `dm-friends.js` не смонтирован в `app.js`/`index.html`; ветка `feature/task-133-mount-dm-friends-app` существует, но в `testing` не влита. Новый честный отказ по отсутствию `request` проверяется только после монтирования. | PM / Integration |
| **P3-3** | наблюдение | Фиксы и PR-18 живут в рабочем дереве, а не в коммите; повторный прогон в чистом клоне даст красный результат. Тот же корень, что у `P1-1`. | L1 |

Ни один пункт не относится к проверенному объёму `P0-1…P0-3` и `P1-2`; все они были известны до retest.

---

## 6. Вердикт

| Критерий | Значение |
|---|---|
| **P0** | 0 |
| **P1 (объём retest)** | 0 |
| **Обязательные наборы** | зелёные, коды `0` |
| **Вердикт** | **QA Passed** |

`BUILD-004` принимается по объёму повторной приёмки: три P0 и P1-2 закрыты, обязательные наборы (`6/6`, `15/15`, `19/19`, `111/111`, `53/53`, `102/102`) и полный unit `977/977` проходят, типы чистые. Содержательные срезы волны (DM/Friends BFF и UI, voice devices, test-impact v2) подтверждены независимыми оракулами, а не только собственными тестами.

Остаётся процессный хвост `P1-1`: фиксация состава сборки коммитом/тегом — действие уровня L1 (координатор/DevOps) на релизном гейте. До этого момента сборка воспроизводима только из рабочего дерева, не из git-ссылки.

---

## 7. Контрольные суммы

Проверенные артефакты сборки:

```text
7a53bfc8dbe78518b53bc1311d1035bf  waves/RW-40/BUILDS/BUILD-004.md
3aacc65a33ca8a24e609c2046c85625f  waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md

356efe9e856fce4053e500f8e8bd50ea  rust/gui-spike/src/lib.rs
24bb37a8f233213ba7c59dc7b9fb1b22  rust/gui-spike/src/main.rs
6e74f81b47b9579bb0f863889c453479  rust/gui-spike/src/dm_friends.rs
8c4a072084bc33c888dd06bbd8eca89c  rust/gui-spike/src/voice_devices.rs
920b11eef5ef5de7bebf715a9846f5ad  rust/gui-spike/tests/dm_friends_contract.rs
f54f0eec2ba008401e96faadb35a6309  rust/gui-spike/tests/voice_devices_contract.rs
63cd550d20f7874c946c4040f932ec4d  rust/gui-spike/ui/features/dm-friends.js
4c9a732185179f59c3bd2873de8ac971  rust/gui-spike/ui/app.js
4cc52749baac80f9769fc0802fe9f800  rust/gui-spike/ui/index.html

591132b4bb81c0554a2aa210bd2ffaab  TDD-PLAN.md
ff9cfb71e85eea7dbbb7274914da85d7  tests/unit/dm-friends-ui.test.mjs
f2f86fc822a739b5dcb355c3a5759100  tests/unit/gui-spike-features.test.mjs
e39c8a68936367fef0f51fa27a594b31  tests/unit/gui-spike-dom.test.mjs
fa18fd3dcfc924eb2f66f753354f5884  tests/unit/test-impact.test.mjs
17a665bc74da1e751b1ea38bf380e26e  scripts/test-impact.mjs
d47c4347c085bde620d9d6a1b65a36e2  tests/meta/plan-sync.test.mjs
```

Логи прогонов (`waves/RW-40/QUALITY/qa-build-004-retest-logs/`):

```text
7541462762f896b3adf5a42a36304938  cargo-contracts.log
ad1e87d4975c98e5891626e6283439b7  dm-friends-ui.log
58e22cb242337daeef5a90d8ef92e4c6  gui-spike-features.log
dc3700d6a8d5e6d7d2bd21875300ae7f  meta.log
8f7373b41b0f9d5f4a20bb291e512b3b  test-impact.log
9638c5dccefbd8652188ceed072c13d1  tsc.log
24e139c1b36d51502680b7eeaa29088a  unit-full.log
```
