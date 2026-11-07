# Техническое ревью: task-253 (`BE-P1-29`, Rust Application Slash Commands & Interaction BFF Contract)

- **Ревьюер:** Technical Reviewer (DeepSeek, dispatch `ctx_756e78db26a3`)
- **Карточка:** `RW-40` / `G1` / `task-253` (`BE-P1-29`)
- **Ветка:** `feature/task-253-be-slash-commands`
- **Режим:** token-lean (только `slash_commands.rs`, `slash_commands_contract.rs`, `REPORT.md`)

## Verdict: APPROVED

## 1. Область проверки

| Объект | Состояние |
|---|---|
| `rust/gui-spike/src/slash_commands.rs` | MD5 `3e03aa0be158350afb64ce317a17f15d` (совпадает с `REPORT.md` §4) |
| `rust/gui-spike/tests/slash_commands_contract.rs` | 8 тестов, соответствуют заявленному контракту |
| Targeted-гейт `T0` | `8 passed; 0 failed` |

## 2. Контрактные проверки (по коду)

| Проверка | Реализация | Тест |
|---|---|---|
| Типы взаимодействий `2/3/4/5` | `INTERACTION_TYPE_*: u8` (строки 15–24) | `slash_commands_constants_match_discord_contract` |
| Типы команд `1/2/3` | `COMMAND_TYPE_*: u8` (строки 27–33) | тот же |
| Лимиты `32` / `25` | `MAX_COMMAND_NAME_CHARS`, `MAX_COMMAND_OPTIONS` | тот же |
| Serde round-trip структуры | `InvokeSlashCommandRequest` с `Serialize/Deserialize/Debug/Clone/PartialEq/Eq` | `invoke_slash_command_request_serde_roundtrip` (включая DM `guild_id=None`) |
| Имя команды `1..=32`, `[a-z0-9_-]` | `validate_slash_command_name` | `..._accepts_valid_names_and_boundaries`, `..._rejects_empty_too_long_uppercase_or_invalid_chars` |
| Snowflake `1..=20` цифр, без ведущего `0`, без `+/-/пробелов` | `validate_snowflake` | `..._rejects_invalid_snowflakes_session_name_or_options` |
| `session_id.trim()` непустой | строки 117–119 | тот же |
| `options.len() <= 25` | строки 123–128 | границы `0`/`25` и отказ на `26` |
| Маршрут `/api/v9/interactions` и тело `{type,application_id,guild_id,channel_id,session_id(non-trimmed вход, trimmed в теле),nonce,data{version,id,name,type,options}}` | `build_interactions_route_and_body` | `..._returns_canonical_route_and_payload`, `..._propagates_validation_errors` |

Замечаний по расхождению с RE-оракулом `Endpoints.INTERACTIONS` и с документированным в тесте контрактом не найдено: имя поля `session_id` в теле тримится, `guild_id` в DM сериализуется как `null`, ошибки валидации пробрасываются, а не глотаются.

## 3. Мутационное тестирование (5/5)

Каждая мутация применялась к `rust/gui-spike/src/slash_commands.rs`, прогонялся targeted-тест, затем файл восстанавливался из резервной копии с побайтовой сверкой MD5.

| # | Мутация | Результат |
|---|---|---|
| M1 | `INTERACTION_TYPE_APPLICATION_COMMAND` `2 -> 7` | killed (exit 101) |
| M2 | `MAX_COMMAND_OPTIONS` `25 -> 26` | killed (exit 101) |
| M3 | Верхняя граница имени `32 -> 33` (`MAX_COMMAND_NAME_CHARS + 1`) | killed (exit 101) |
| M4 | `value.len() > 20` -> `> 21` (Snowflake-длина) | killed (exit 101) |
| M5 | Убран `.trim()` у `session_id` в теле запроса | killed (exit 101) |

**Итог: 5/5 убито (100%).** После восстановления MD5 файла — `3e03aa0be158350afb64ce317a17f15d`, идентичен значению из `REPORT.md`.

## 4. Красные флаги

- `P0`: 0. `P1`: 0.
- Тесты не ослаблены, продуктовый код под тесты не подгонялся, отключённых проверок нет.
- Затронутые файлы в рамках ТЗ; `git commit` не выполнялся.

## 5. Итог

Карточка `task-253` (`BE-P1-29`) соответствует контракту, targeted-тест зелёный (`8/8`), мутации `5/5` убиты, файл восстановлен побайтово. Блокирующих замечаний нет — **APPROVED**, можно переводить в `Ready for Merge`.
