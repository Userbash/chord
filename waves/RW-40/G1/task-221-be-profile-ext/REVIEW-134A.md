# Техническое ревью 134A — task-221 (`profile_ext`)

- **PR**: PR-105, ветка `feature/task-221-be-profile-ext`
- **Ревьюер**: DeepSeek V4.1 Flash (Technical Review)
- **Объём**: `rust/gui-spike/src/profile_ext.rs`, `rust/gui-spike/tests/profile_ext_contract.rs`, `rust/gui-spike/src/lib.rs`
- **Гейт**: targeted only (`profile_ext_contract`), по режиму TOKEN-LEAN

## Вердикт: APPROVED

## 1. Проверка контракта

| Проверка | Результат |
|---|---|
| `cargo test --test profile_ext_contract` (до мутаций) | `8 passed; 0 failed`, exit 0 |
| `cargo test --test profile_ext_contract` (после восстановления файла) | файл восстановлен побайтово, md5 совпал |
| md5 `src/profile_ext.rs` | `ebd1724dfa9f513a9ef4d3759c6cccdd` — совпадает с REPORT.md §6 |
| md5 `tests/profile_ext_contract.rs` | `9a0ac1708db14644ecb37896a1321a6c` — совпадает с REPORT.md |
| md5 `src/lib.rs` | `0ac6d016e1e0ae30a54d20556e91c973` — совпадает с REPORT.md |
| Посторонние правки | нет; `Cargo.lock` в рабочем дереве не изменён, `git commit` не выполнялся |

Контракт соответствует оракулу `reference-oracle/rest-static.json`:
`GET /users/{user_id}/profile?with_mutual_guilds=..&with_mutual_friends=..[&guild_id=..]`
и `PUT /users/@me/notes/{target_user_id}`; лимиты `190 / 40 / 256` символов Unicode и
24-битный `accent_color <= 0xFF_FFFF`.

Тесты проверяют независимые границы (позитив, негатив, `"1"`/`u64::MAX`, `u64::MAX + 1`,
ведущие нули, многобайтовый UTF-8 ровно на границе и на границе +1), а не текущую реализацию.

## 2. Мутационное тестирование — 5/5 killed

Каждая мутация применялась отдельно к `src/profile_ext.rs`, прогонялся targeted-тест,
файл восстанавливался из бэкапа с проверкой md5.

| # | Мутация | Ожидание | Факт (тест) | Итог |
|---|---|---|---|---|
| M1 | `MAX_BIO_CHARS: 190 -> 191` | kill | exit 101, `profile_ext_constants_match_discord_contract` FAILED (7 passed / 1 failed) | killed |
| M2 | убран `value.starts_with('0')` в `validate_canonical_snowflake` | kill | exit 101, `build_user_profile_route_validates_canonical_snowflake_bounds` + `validate_user_note_builds_route_and_enforces_snowflake_and_char_limits` FAILED (6/2) | killed |
| M3 | `chars > MAX_BIO_CHARS` -> `chars >= MAX_BIO_CHARS` | kill | exit 101, `validate_profile_patch_enforces_bio_char_limit` FAILED (7/1) | killed |
| M4 | `color > MAX_ACCENT_COLOR` -> `color >= MAX_ACCENT_COLOR` | kill | exit 101, `validate_profile_patch_enforces_24bit_accent_color_bounds` FAILED (7/1) | killed |
| M5 | ключ запроса `&guild_id=` -> `&guild=` | kill | exit 101, `build_user_profile_route_formats_query_with_and_without_guild_id` + `..._canonical_snowflake_bounds` FAILED (6/2) | killed |

**Итого: 5/5 мутаций убито (100%).**

## 3. Восстановление

После мутаций `rust/gui-spike/src/profile_ext.rs` восстановлен из `/tmp/opencode/profile_ext.rs.bak`:
md5 `ebd1724dfa9f513a9ef4d3759c6cccdd` совпал с исходным и с REPORT.md. Рабочее дерево
не содержит остаточных изменений.

### Дрейф `rust/gui-spike/Cargo.lock` при ревью

Прогон `cargo test` в рамках ревью перегенерировал `rust/gui-spike/Cargo.lock`
(добавление `zstd`, `zstd-safe`, `zstd-sys`, `jobserver` от `discord-codec`).
Файл откатан через `git checkout -- rust/gui-spike/Cargo.lock` (md5 `7d66c56ef6f7e6e06cb7885a7d9a4bb7`).
`package-lock.json` был изменён до старта задачи и не трогался.

## 4. Замечания (не блокирующие)

- `P2`: `validate_canonical_snowflake` дублируется по смыслу с другими модулями (`profile.rs`, `dm_friends.rs`); кандидат на общий хелпер в отдельной задаче.
- `P3`: в `validate_profile_patch` возврат `String`-ошибок вместо типизированного enum — согласуется с текущим стилем BFF, менять вне scope.

Блокирующих замечаний `P0`/`P1` нет.
