# task-178 — BE-P3-02 + BE-P3-09: маркировка `[S]`-расширений и сверка оракула D-02

- **Воркер:** G1 Backend, task-178 (`feature/task-178-routes-catalog-s-markers`)
- **Карточки:** BE-P3-02 (каталог 49 vs 47), BE-P3-09 (синхронизация `docs/re/` и `routes.rs`)
- **Write-scope:** `rust/crates/discord-core/src/rest/routes.rs`,
  `rust/crates/discord-core/tests/rest_routes.rs` — других файлов не касался.

## Сделано

`routes.rs`:

- Шапка модуля приведена к фактическому составу: 49 маршрутов = 47 endpoints D-02 +
  2 `[S]`/raw-расширения (`POST /channels/{id}/messages`, `GET /guilds/{id}/messages/search`);
  отдельно описаны 4 расширения в `EXTENSIONS` (typing, invite `GET`/`POST`, поиск по каналу).
- Добавлены `CAPTURED_D02_ROUTE_COUNT: usize = 47` и `STATIC_EXTENSION_ROUTE_COUNT: usize = 2`.
- Добавлен `RouteSpec::is_static_extension()`: `true` для всех спецификаций вне захвата D-02
  (2 внутри `ROUTES` + 4 в `EXTENSIONS`), `false` для 47 строк D-02.
- Добавлена `GUILD_SEARCH_PATH`; guild-поиск в `ROUTES` теперь ссылается на неё.

`tests/rest_routes.rs`:

- `catalog_counts_separate_d02_and_static_extensions`: `ROUTES.len() == 47 + 2 == 49`;
  ровно 2 расширения внутри `ROUTES` (POST сообщения + guild search); остальные 47 — D-02;
  все 4 спецификации из `EXTENSIONS` отвечают `is_static_extension() == true`.
- `rest_d02_copy_matches_lab_oracle`: md5 `docs/re/rest-d02.json` =
  `87d7a05d8cd11f3b8ae37aa5c9811c9e`, 47 пар `(method, path)`; если есть
  `reference-oracle/rest-d02.json` — его md5 = `9b450c34a2277b04989830b53b02f012`,
  пары `(method, path)` совпадают, а пофайловый diff — ровно одна строка `reqBodySample`
  с `REDACTED` в копии репозитория (санитайзер `docs/re/README.md`). Любое другое
  расхождение валит тест. Если лаборатории нет — проверяются 47 endpoints копии.
- MD5 считается в тесте локальной реализацией RFC 1321 (внешних крейтов нет).

## TDD

- **Red:** `red.log` — `cargo test ... --test rest_routes --lib` падает на компиляции
  (`E0425`/`E0432`/`E0599`: нет `CAPTURED_D02_ROUTE_COUNT`, `STATIC_EXTENSION_ROUTE_COUNT`,
  `is_static_extension`).
- **Green:** `green.log` — 18/18 тестов `rest_routes` (16 прежних без изменений + 2 новых),
  33/33 lib-теста; `cargo clippy ... -- -D warnings` — exit 0.

## Команды

```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target \
  cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib
CARGO_TARGET_DIR=/home/user/orca/project/rust/target \
  cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib -- -D warnings
rustfmt --edition 2021 --check \
  rust/crates/discord-core/src/rest/routes.rs rust/crates/discord-core/tests/rest_routes.rs  # exit 0
```

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/rest/routes.rs` | `8d5def34c76ecfcc942a17be70db111e` |
| `rust/crates/discord-core/tests/rest_routes.rs` | `eb5e84ace4b4f0da84b6b693d4322848` |

## Риски и замечания

- `docs/re/rest-d02.json` не совпадает с лабораторным оракулом байт-в-байт **по политике
  санитайзера** (единственная строка `reqBodySample`); тест фиксирует именно это
  расхождение и падает на любом другом. Согласовано с координатором (Option A).
- `is_static_extension()` опирается на явный список 6 пар «метод, путь»; при добавлении
  нового расширения его нужно внести и в `STATIC_EXTENSION_ROUTES`, иначе тест на состав
  расширений не покажет проблему для новых путей (счётчик проверяет только 2 в `ROUTES`).
- Полный `gate:worker` не запускался (batch-режим, targeted-уровень `T0`); `cargo fmt`
  по пакету содержит прежний дрейф в `gateway.rs`/`settings/mod.rs` — изменённые файлы
  проходят `rustfmt --check` чисто.
