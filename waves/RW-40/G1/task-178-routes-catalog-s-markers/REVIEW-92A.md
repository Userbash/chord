# Техническое ревью (Review 92A): task-178 (BE-P3-02 & BE-P3-09) — маркировка `[S]`-расширений в каталоге `ROUTES` и сверка `docs/re/rest-d02.json` с лабораторным оракулом

- **Ветка:** `feature/task-178-routes-catalog-s-markers`
- **Пулл-реквест:** `PR-63` (`feature/task-178-routes-catalog-s-markers -> unstable`)
- **Ревьюер:** Technical Reviewer (Review 92A)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED (Ready for Merge)**
- **Критичность:** `P0 = 0`, `P1 = 0`, `P2 = 0`, `P3 = 3`
- **Мутации:** `5/5 (100%) KILLED`

---

## 1. Состав изменений (scope)

`git status --porcelain` в worktree:

```
 M package-lock.json
 M rust/crates/discord-core/src/rest/routes.rs
 M rust/crates/discord-core/tests/rest_routes.rs
```

Разрешённые ТЗ файлы — только два (`routes.rs`, `rest_routes.rs`). Третья строка (`package-lock.json`) — не правка воркера и не часть задачи: дифф там ровно синхронизация lockfile с уже лежащими в `package.json` полями `bin`/`engines` (6 строк), а mtime `00:20:45` совпадает со временем checkout worktree (`00:20:44` у всех файлов), тогда как `routes.rs` изменён в `00:24:56`. В коммит PR эту строку включать нельзя; код-ревью она не затрагивает.

`git diff --check` — чисто (exit `0`).

| Файл | md5 в worktree | md5 в `REPORT.md` §5 |
|---|---|---|
| `rust/crates/discord-core/src/rest/routes.rs` | `8d5def34c76ecfcc942a17be70db111e` | совпадает |
| `rust/crates/discord-core/tests/rest_routes.rs` | `eb5e84ace4b4f0da84b6b693d4322848` | совпадает |

**Совместимость с `unstable`.** `HEAD` ветки = `c07e610` (Merge PR-58), `unstable` = `b8183f5`; ветка отстала на 2 коммита. Проверено: `git diff --name-only HEAD unstable` не содержит ни одного файла `discord-core` (`rest/` и `rest_*` — пусто; затронуты только `discord-platform`, `discord-voice`, `gui-spike`). Слияние не даёт ни конфликтов, ни ошибок компиляции — новый код не зависит ни от одного изменённого там API. Блокирующего риска, как в Review 87A у PR-58, здесь нет.

---

## 2. Тесты и линтер

Команды из ТЗ, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`:

```
cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib
```

- `lib`: **33 passed**, 0 failed
- `rest_routes`: **18 passed**, 0 failed (16 прежних без правок + 2 новых)
- Итого **51/51 passed**, exit `0`

```
cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib -- -D warnings
```

- **0 warnings / 0 errors**, exit `0`

`rustfmt --edition 2021 --check` по обоим файлам воркера — exit `0`. Заявленный в `REPORT.md` red-прогон соответствует `red.log`: `E0432` на импортах `CAPTURED_D02_ROUTE_COUNT`/`STATIC_EXTENSION_ROUTE_COUNT`, `E0599` на `is_static_extension` — тесты писались до кода, не подгонялись.

---

## 3. Мутации (5/5 KILLED)

Каждая мутация применялась к рабочему дереву (файл целиком восстановлен после прогона из побайтовой копии `/tmp/opencode/routes.rs.orig`), прогонялись ровно те же `--test rest_routes --lib`, затем файл возвращался. После всех пяти md5 снова `8d5def34c76ecfcc942a17be70db111e`.

| # | Мутация | Внесённое изменение | Результат | Убивающий тест |
|---|---|---|---|---|
| **M1** | Счётчик D-02 завышен | `CAPTURED_D02_ROUTE_COUNT: usize = 47` → `49` | **KILLED** (`16 passed; 2 failed`, exit `101`) | `catalog_counts_separate_d02_and_static_extensions` (assert 47) и `rest_d02_copy_matches_lab_oracle` (47 endpoints против константы) |
| **M2** | Счётчик расширений обнулён | `STATIC_EXTENSION_ROUTE_COUNT: usize = 2` → `0` | **KILLED** (`17 passed; 1 failed`, exit `101`) | `catalog_counts_separate_d02_and_static_extensions` («расширения внутри ROUTES», а также `ROUTES.len() == 47 + 0`) |
| **M3** | Классификатор всегда врёт | тело `is_static_extension` → `false` | **KILLED** (`17 passed; 1 failed`, exit `101`) | `catalog_counts_separate_d02_and_static_extensions` (`расширения в ROUTES: []`) |
| **M4** | Потеряно расширение `POST /channels/{id}/messages` | строка удалена из `STATIC_EXTENSION_ROUTES`, длина массива `6` → `5` | **KILLED** (`17 passed; 1 failed`, exit `101`) | `catalog_counts_separate_d02_and_static_extensions` (сравнение множества пар: остался только guild search) |
| **M5** | Потеряно расширение `GET /guilds/{id}/messages/search` | строка удалена из `STATIC_EXTENSION_ROUTES`, длина массива `6` → `5` | **KILLED** (`17 passed; 1 failed`, exit `101`) | `catalog_counts_separate_d02_and_static_extensions` (остался только `POST /channels/{id}/messages`) |

Логи и диффы мутаций: `/tmp/opencode/mutations/M{1..5}_*.{log,diff}`. Lib-тесты (`33 passed`) при каждой мутации оставались зелёными — это ожидаемо, они не касаются каталога; убивают мутации именно контрактные тесты `rest_routes`.

---

## 4. Сверка с RE-оракулом (независимые артефакты)

Проверено вручную, помимо тестов:

- `md5sum docs/re/rest-d02.json` = `87d7a05d8cd11f3b8ae37aa5c9811c9e`, `md5sum /home/sanya/re-deep/rest-d02.json` = `9b450c34a2277b04989830b53b02f012` — совпадает с константами теста.
- `diff` копий — **ровно одна строка** (`1528c1528`), и это `"reqBodySample"` с `REDACTED…` в копии репозитория против реального base64 тела `settings-proto` в лаборатории. Политика замены зафиксирована в `docs/re/README.md` («пример тела `settings-proto` заменён на `REDACTED`»), то есть расхождение санкционировано, а не «подогнано».
- `jq '.endpoints | length' docs/re/rest-d02.json` = **47**; пары `(method, path)` из оракула (со срезом `/api/v9`) побайтово совпадают с выдержкой `rust/crates/discord-core/tests/data/rest-routes-47.tsv` — `diff` пустой. Существующий тест `catalog_matches_live_capture` держит соответствие TSV ↔ `ROUTES`, значит 47 «нерасширенных» строк `ROUTES` действительно равны захвату D-02, а не просто посчитаны.
- Классификация расширений подтверждена по оракулу: в 47 endpoints **нет** `POST /channels/{id}/messages`, `GET /guilds/{id}/messages/search`, `/typing`, `/invites/{code}`, `/channels/{id}/messages/search`; есть только `GET /channels/{id}/messages` (история). То есть 6 промаркированных пар не пересекаются с D-02, а `GET /channels/{id}/messages` корректно остаётся `false` (проверяется отдельным assert).
- Провенанс guild-поиска «сырой `capture-run2.jsonl`» не выдуман: в `/home/sanya/re-deep/capture-run2.jsonl` 22 вхождения `/guilds/<id>/messages/search?...` с параметрами `content/sort_by/sort_order/search_session_id/search_query_id/offset` — ровно тот набор, что в `MESSAGES_SEARCH_QUERY_PARAMS`.

`docs/re/rest-d02.json` в этом PR не менялся (md5 копии тот же, что и до задачи) — BE-P3-09 закрыт именно тестом-стражем, а не правкой оракула.

---

## 5. Замечания P3 (не блокируют)

1. **Ручная синхронизация списка расширений.** `is_static_extension` опирается на приватный `STATIC_EXTENSION_ROUTES` (`routes.rs:128`); тест проверяет состав двух расширений в `ROUTES` и то, что все 4 известные спецификации из `EXTENSIONS` помечены. Пятое расширение, добавленное только в `EXTENSIONS`, ни один тест не поймает. Риск отражён в `REPORT.md` §«Риски»; при следующем касании файла стоит сделать тест по `EXTENSIONS`, а не по списку из четырёх литеральных пар.
2. **Парсер оракула завязан на отступ.** `oracle_method_path_pairs` (`tests/rest_routes.rs:940`) ищет ключи по префиксу ровно из 6 пробелов. При переформатировании JSON тест упадёт громко (assert `47`), а не тихо пропустит проверку, поэтому это не дефект, но при правке формата парсер придётся обновить.
3. **`md5_hex` — локальная реализация RFC 1321 на ~70 строк** (`tests/rest_routes.rs:982`). Внешних крейтов в тестах нет, поэтому решение оправдано; корректность подтверждена тем, что константы теста совпадают с системным `md5sum`. Дублировать её в других тестах не стоит — при появлении третьего потребителя лучше вынести в общий тестовый модуль.

Отдельно проверено и претензий нет: новых `unwrap()`/`expect()` в продуктовом коде не появилось (в `routes.rs` — только `#[must_use]`-метод над константным списком, без аллокаций и паник); `GUILD_SEARCH_PATH` (`routes.rs:375`) объявлен рядом с остальными путями и используется и в `ROUTES`, и в `STATIC_EXTENSION_ROUTES`, поэтому строка и метка не могут разъехаться по значению; публичные `CAPTURED_D02_ROUTE_COUNT` (`routes.rs:199`) и `STATIC_EXTENSION_ROUTE_COUNT` (`routes.rs:203`) задокументированы, `missing_docs` чист; посторонние тесты и общие модули (`src/lib.rs`, `state/*`, `rest/mod.rs`) не тронуты.

---

## 6. Итог

- **P0 = 0.**
- **P1 = 0.** Ветка отстаёт от `unstable` на 2 коммита, но они не касаются `discord-core` — merge проходит без конфликтов и ошибок сборки.
- **P2 = 0.** **P3 = 3** (см. §5).
- **Мутации: 5/5 убито (100%).**
- **Тесты: 51/51 passed, clippy `-D warnings` и `rustfmt --check` — чисто, write-scope соблюдён (в коммит не должен попасть только `package-lock.json`).**
- **Вердикт: APPROVED (Ready for Merge).**

Перевод в `Ready for Merge`: код и тесты принимаются как есть. Интегратору при коммите PR-63 исключить `package-lock.json` (посторонний bootstrap-дрейф lockfile) и слить ветку в `unstable`; после merge ветку удалить. Документацию (`STATUS`/`ROADMAP`/`TASKS`/`map`/`STATE`) обновляет Team Lead после приёмки, в этом ревью она не правилась.
