# Техническое ревью REVIEW-156A — task-242 (BE-P1-26, `expressions`)

- **Ревьюер**: Technical Reviewer (DeepSeek V4.1 Flash), term `term_a0a474ef-874b-42bd-9902-75aff4c73685`
- **Волна / отдел**: `RW-40`, `G1` (Backend)
- **Ветка / worktree**: `feature/task-242-be-expressions`,
  `/home/sanya/orca/workspaces/project/feature-task-242-be-expressions`
- **Предмет ревью**: `rust/gui-spike/src/expressions.rs`,
  `rust/gui-spike/tests/expressions_contract.rs`, отчёт
  `waves/RW-40/G1/task-242-be-expressions/REPORT.md`
- **Дата**: 2026-10-31

## 1. Verdict

**Verdict: APPROVED**

Блокирующих замечаний `P0`/`P1` нет. Мутационное покрытие `5/5 (100%)`, исходный
файл восстановлен байт-в-байт (`md5` совпал с отчётом).

## 2. Проверенный контракт

| Элемент | Ожидание по ТЗ / Discord API v9 | Факт в коде |
|---|---|---|
| `MIN_EMOJI_NAME_CHARS` / `MAX_EMOJI_NAME_CHARS` | `2` / `32` | совпадает |
| `MAX_EMOJI_BYTES` | `256 * 1024` = `262_144` | совпадает |
| `MIN_STICKER_NAME_CHARS` / `MAX_STICKER_NAME_CHARS` | `2` / `30` | совпадает |
| `MAX_STICKER_DESC_CHARS` | `100` | совпадает |
| `STICKER_FORMAT_PNG/APNG/LOTTIE/GIF` | `1/2/3/4` | совпадает |
| `validate_emoji_name` | `trim()` → длина `2..=32`, только `[A-Za-z0-9_]` | совпадает, не-ASCII отклоняется |
| `validate_create_guild_emoji` | имя → `byte_len 1..=MAX_EMOJI_BYTES` → префикс Data URI из 4 MIME + непустой payload → Snowflake ролей | порядок и правила совпадают |
| `validate_snowflake` | `1..=20` ASCII-цифр, без ведущего `'0'` | `is_empty()`, `len() > 20`, `starts_with('0')`, `is_ascii_digit()` |
| `build_guild_emojis_route` | `/api/v9/guilds/{guild_id}/emojis[/{emoji_id}]` | совпадает, `emoji_id` валидируется только при `Some` |

Пустой список `roles` допустим — соответствует поведению Discord (эмодзи без ролевых
ограничений). Пробелы в `name` срезаются только по краям, внутренние пробелы отклоняются
проверкой набора символов. Ошибки возвращаются как `Result<_, String>` с русским текстом,
`panic`/`unwrap` в продуктовом коде отсутствуют.

## 3. Targeted-тесты (green)

Команда:

```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test expressions_contract
```

Результат (`exit=0`):

```text
running 7 tests
test build_guild_emojis_route_formats_collection_and_item_routes_and_validates_snowflakes ... ok
test create_guild_emoji_request_serde_roundtrip ... ok
test expressions_constants_match_discord_contract ... ok
test validate_create_guild_emoji_enforces_byte_len_and_supported_image_data_uris ... ok
test validate_create_guild_emoji_validates_name_and_role_snowflakes ... ok
test validate_emoji_name_accepts_valid_trimmed_ascii_alphanumeric_and_underscore_names ... ok
test validate_emoji_name_rejects_short_long_non_ascii_and_invalid_punctuation ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Компиляция контрактного теста подтверждает, что `pub mod expressions;` в
`rust/gui-spike/src/lib.rs` присутствует и публичный API доступен снаружи крейта.

## 4. Мутационное тестирование (5/5)

Каждая мутация вносилась в `rust/gui-spike/src/expressions.rs` по одной, прогонялся
targeted-тест, затем файл восстанавливался из резервной копии.

| # | Мутация | Ожидание | Факт |
|---|---|---|---|
| M1 | `MIN_EMOJI_NAME_CHARS: 2` → `3` | тесты падают | `exit=101`, `2 failed` |
| M2 | `value.len() > 20` → `> 21` (Snowflake) | тесты падают | `exit=101`, `2 failed` |
| M3 | `value.starts_with('0')` → `'9'` | тесты падают | `exit=101`, `2 failed` |
| M4 | граница `1..=MAX_EMOJI_BYTES` → `0..=MAX_EMOJI_BYTES` | тесты падают | `exit=101`, `1 failed` |
| M5 | `payload.trim().is_empty()` → `payload.is_empty()` | тесты падают | `exit=101`, `1 failed` |

Итог: **убито 5 из 5 мутаций (100%)**. Ни одна мутация не выжила, «зелёного» прохода
под мутацией не зафиксировано.

## 5. Восстановление исходника

```text
BASELINE_MD5   = 9c9aa605e2e4c50f395652ba49004c1d
RESTORED_MD5   = 9c9aa605e2e4c50f395652ba49004c1d
BYTE_IDENTICAL = yes   (cmp с резервной копией)
```

`md5` совпадает со значением из `REPORT.md` §6 (`9c9aa605e2e4c50f395652ba49004c1d`),
рабочее дерево после мутаций не содержит остаточных правок.

## 6. Замечания (неблокирующие)

1. **`P3`**: `validate_snowflake` считает длину в байтах (`value.len()`), а не в символах.
   Для не-ASCII входа это не даёт ложного `Ok` (мультибайтные символы отсекаются
   `is_ascii_digit()`), но порядок проверок делает сообщение об ошибке для 21-символьной
   не-ASCII строки менее точным. Поведение корректное, менять не требуется.
2. **`P3`**: тест `validate_emoji_name_accepts_...` собирает `exact_32` через
   `repeat(4)` от 8-символьного шаблона — читаемо, но проверяет длину ассертом, что
   достаточно.
3. Отмеченный в `REPORT.md` §7 дрейф `Cargo.lock` в write-scope не входит; при текущем
   прогоне `git status` по `rust/gui-spike/Cargo.lock` не проверялся (вне рамок
   token-lean ревью). Требуется подтверждение интегратора перед merge.

## 7. DoD

- [x] Контракт соответствует Discord API v9 и `rest-static.json` (`GUILD_EMOJIS`, `GUILD_EMOJI`).
- [x] Targeted-тесты зелёные (7/7).
- [x] Мутации `5/5`.
- [x] Исходник восстановлен байт-в-байт, `md5` = `9c9aa605e2e4c50f395652ba49004c1d`.
- [x] `P0 = 0`, `P1 = 0`.
- [ ] Полный `gate:worker` / workspace-тесты — вне рамок этого ревью (token-lean), выполняет integration-лейн.
