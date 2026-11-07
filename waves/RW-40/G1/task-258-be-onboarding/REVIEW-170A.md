# REVIEW-170A — Technical Review PR-141 / task-258 (`BE-P1-30`, onboarding)

- **Ревьюер**: Технический ревьюер (DeepSeek), token-lean режим
- **PR**: 141, ветка `feature/task-258-be-onboarding`
- **Объект**: `rust/gui-spike/src/onboarding.rs`, `rust/gui-spike/tests/onboarding_contract.rs`, `REPORT.md`
- **Дата**: 2026-10-31
- **CARGO_TARGET_DIR**: `/home/sanya/orca/project/rust/target`

## 1. Вердикт

**APPROVED** — P0 = 0, P1 = 0, мутации **5/5 убиты**, targeted-тест зелёный (5/5), md5 полностью совпадают с `REPORT.md`.

## 2. Сверка md5 с REPORT.md

| Файл | md5 в REPORT.md | md5 фактический | Итог |
|---|---|---|---|
| `rust/gui-spike/src/onboarding.rs` | `41aac1af42000e1485b60d168dee0757` | `41aac1af42000e1485b60d168dee0757` | совпадает |
| `rust/gui-spike/tests/onboarding_contract.rs` | `5c856c3207f76f3a3ea3e9f348919417` | `5c856c3207f76f3a3ea3e9f348919417` | совпадает |
| `rust/gui-spike/src/lib.rs` | `12e7a931384fc53bfaf9b70e87f67803` | `12e7a931384fc53bfaf9b70e87f67803` | совпадает |
| `waves/RW-40/G1/task-258-be-onboarding/red.log` | `3a9203c0535126653271bdf41e315af5` | `3a9203c0535126653271bdf41e315af5` | совпадает |
| `waves/RW-40/G1/task-258-be-onboarding/green.log` | `dfbfa74c3e17154f18f0d70f7d8cbcfb` | `dfbfa74c3e17154f18f0d70f7d8cbcfb` | совпадает |

Экспорт `pub mod onboarding;` присутствует в `rust/gui-spike/src/lib.rs:55`.

## 3. Targeted-прогон

Команда:

```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test onboarding_contract
```

До мутаций и после восстановления — `exit=0`, `5 passed; 0 failed`:

```text
test onboarding_constants_match_discord_contract ... ok
test build_guild_welcome_screen_and_onboarding_routes_validate_snowflake_and_format_paths ... ok
test validate_welcome_screen_request_enforces_description_and_channels_count_bounds ... ok
test validate_welcome_screen_request_validates_channel_entries ... ok
test welcome_screen_structs_serde_roundtrip ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 4. Мутационная матрица (5/5 убиты)

Каждая мутация применялась к `rust/gui-spike/src/onboarding.rs` по одной, затем запускался targeted-тест; после каждого прогона файл восстанавливался из бэкапа побайтово и сверялся `md5sum` (контроль — `41aac1af42000e1485b60d168dee0757`).

| # | Мутация | md5 мутанта | exit | Убившая проверка |
|---|---|---|---|---|
| M1 | `MAX_WELCOME_CHANNELS: usize = 5` → `6` | `a45f49c8f1c57e750f1d17b049eadfd3` | 101 | `onboarding_constants_match_discord_contract`, `validate_welcome_screen_request_enforces_description_and_channels_count_bounds` |
| M2 | граница описания экрана `1..=MAX_WELCOME_DESCRIPTION_CHARS` → `+ 1` (140 → 141, off-by-one) | `5f0e0bfd99968ccad7c50ad12c854ae4` | 101 | `validate_welcome_screen_request_enforces_description_and_channels_count_bounds` |
| M3 | `entry.description.trim().chars().count()` → без `.trim()` | `3ff7e5fe569285163bce3ec3a26de1b6` | 101 | `validate_welcome_screen_request_validates_channel_entries` |
| M4 | `validate_snowflake`: удалено условие `value.starts_with('0')` | `0639f4651b1ed7e769e108fe49a6b733` | 101 | `build_guild_welcome_screen_and_onboarding_routes_validate_snowflake_and_format_paths`, `validate_welcome_screen_request_validates_channel_entries` |
| M5 | маршрут онбординга `/onboarding` → `/onboardin` | `0614e99adfb14419773405e87dda1e3b` | 101 | `build_guild_welcome_screen_and_onboarding_routes_validate_snowflake_and_format_paths` |

Итог: 5/5 мутантов дали `exit=101` с падением именованных тестов; после каждого восстановления md5 возвращался к `41aac1af42000e1485b60d168dee0757`.

**Методическая заметка.** Первый проход матрицы использовал восстановление `cp -p`, из-за чего mtime возвращался в прошлое и cargo не пересобирал тестовый бинарь: финальный green-прогон показал ложное падение с текстом мутанта M5. Матрица была повторена с принудительной пересборкой (`cp` + `touch` после каждого восстановления); результаты в таблице выше — из второго, чистого прохода, финальный прогон после него `exit=0`, 5 passed. Это дефект процедуры ревью, не кода PR.

## 5. Содержательная проверка

Соответствие контракту Discord API v9 и оракулу `/home/sanya/re-deep/rest-static.json` (`GUILD_ONBOARDING`, `GUILD_WELCOME_SCREEN`):

- Маршруты `/api/v9/guilds/{guild_id}/welcome-screen` и `/api/v9/guilds/{guild_id}/onboarding` — совпадают с `REPORT.md` §2.
- Константы: `0/1` режимы, `140` описание экрана, `5` каналов, `50` описание канала.
- Snowflake строго `1..=20` ASCII-цифр без ведущего `'0'`; проверка `value.bytes().all(is_ascii_digit)` делает `len()` (байты) эквивалентным числу символов, инъекция в путь невозможна — в маршрут попадают только цифры.
- `trim()` применяется до подсчёта `chars()` как для описания экрана, так и для описания канала; строка из пробельных символов отвергается.
- `emoji_id` валидируется только при `Some`, `None` допустим; `enabled: bool` и `welcome_channels: Vec<_>` сериализуются без `skip_serializing_if`, что соответствует ожидаемому JSON (`description: null`, `emoji_id: null`), зафиксированному в тесте round-trip.
- Тесты независимы от реализации: проверяют контрактные строки маршрутов, границы 1/140/141, 0/5/6, ASCII/Unicode (`Ж`, `Ю`), 21-значные ID; negative-кейсы перечислены явно.

Неблокирующие замечания (P2/P3, не влияют на вердикт):

- **P3**: `emoji_name` не ограничивается по длине — лимита в контракте нет, поведение допустимо; стоит зафиксировать комментарием, чтобы не выглядело пропуском.
- **P3**: дубликаты `channel_id` в `welcome_channels` не отвергаются; Discord-контракт этого не требует, но если BFF хочет раннюю ошибку — отдельная карточка.
- **P3**: `validate_snowflake` возвращает `String` с русским текстом; коды ошибок не типизированы — согласуется с остальными модулями BFF, менять не требуется.

## 6. Финальное состояние

- `md5sum rust/gui-spike/src/onboarding.rs` = `41aac1af42000e1485b60d168dee0757`
- `git status` после ревью: изменёнными остаются только файлы PR (`src/lib.rs`, untracked `src/onboarding.rs`, `tests/onboarding_contract.rs`) плюс предсуществующие `package-lock.json` и `node_modules`.
- Дрейф `rust/gui-spike/Cargo.lock`, возникший при прогонах, откатан (`git checkout -- rust/gui-spike/Cargo.lock`).
- Тестовый файл не изменялся; полный `gate:worker` не запускался; `git commit` не выполнялся.
