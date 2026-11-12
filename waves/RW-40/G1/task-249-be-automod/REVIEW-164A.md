# Техническое ревью 164A — task-249 (`BE-P1-28`, `automod`)

**Verdict: APPROVED**

- **Wave / отдел**: `RW-40` / `G1` (Backend).
- **Карточка**: `task-249` — BFF-контракт правил автомодерации гильдии.
- **Ветка**: `feature/task-249-be-automod`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Ревизуемые артефакты**: `rust/gui-spike/src/automod.rs`, `rust/gui-spike/tests/automod_contract.rs`, отчёт `waves/RW-40/G1/task-249-be-automod/REPORT.md`.
- **Режим**: token-lean, только targeted-прогон; workspace-тесты, `clippy` по всему крейту и `git commit` не выполнялись.

## 1. Соответствие контракту

| Элемент | Ожидание | Факт в `src/automod.rs` |
|---|---|---|
| `AUTOMOD_EVENT_MESSAGE_SEND` | `1` | `u8 = 1` (стр. 18) |
| `AUTOMOD_TRIGGER_KEYWORD/SPAM/KEYWORD_PRESET/MENTION_SPAM` | `1/3/4/5` | `1/3/4/5` (стр. 21–30) |
| `AUTOMOD_ACTION_BLOCK_MESSAGE/SEND_ALERT/TIMEOUT` | `1/2/3` | `1/2/3` (стр. 33–39) |
| `MAX_AUTOMOD_NAME_CHARS` | `100` | `100` (стр. 42) |
| `MAX_KEYWORD_FILTER_ITEMS` | `1000` | `1000` (стр. 45) |
| `MAX_KEYWORD_ITEM_CHARS` | `60` (приватная) | `60`, используется только в `validate_automod_rule_request` (стр. 54, 126) |
| `MAX_MENTION_TOTAL_LIMIT` | `50` | `u8 = 50` (стр. 48) |
| `MAX_TIMEOUT_DURATION_SECS` | `2_419_200` | `u32 = 2_419_200` (стр. 51) |

Маршрутизация совпадает с REST-оракулом (`/guilds/{arg}/auto-moderation/rules` и `.../rules/{arg}`, `rest-static.json:8353-8380` по ссылке из шапки модуля): сборка идёт через `/api/v9/...`, оба сегмента валидируются как положительные Snowflake `1..=20` ASCII-цифр без ведущего `'0'`.

Валидация покрывает все ветки контракта: имя (`trim()` + `chars().count()`, `1..=100`), `event_type == 1`, `KEYWORD` (`1..=1000` элементов по `1..=60` символов после `trim()`), `MENTION_SPAM` (`Some(1..=50)`), `SPAM`/`KEYWORD_PRESET` без доп. требований, неизвестные `trigger_type` — `Err`; `action_type` `1|2|3`, при `TIMEOUT` обязателен `timeout_duration_secs` в `1..=2_419_200`. Сообщения об ошибках содержат имя поля и фактическое значение — диагностируемо.

Безопасность и граничные случаи: `chars().count()` вместо `len()` (Unicode-корректно, 61 «Ы» и 101 «Я» отклоняются), отрицательные и пробельные Snowflake отсекаются, `u8`/`u32`-сужающие переполнения невозможны по типам, `-D warnings`-чистота подтверждена автором в `REPORT.md` §5.

Замечаний уровня `P0`/`P1` нет. Наблюдение `P3` (не блокирует): `MAX_KEYWORD_ITEM_CHARS` приватна, а в `REPORT.md` §2 константа перечислена наравне с публичными — при экспорте в UI-слой её придётся открыть; сейчас это осознанное решение, тесты не зависят от её видимости.

## 2. RED → GREEN

- RED зафиксирован в `red.log`: `error[E0432]: unresolved import discord_gui_spike::automod`, `EXIT_CODE=101` — тест писался до модуля.
- GREEN (воспроизведено в этом ревью независимым прогоном):

```text
running 7 tests
test automod_constants_match_discord_contract ... ok
test build_guild_automod_route_formats_collection_and_item_routes_and_validates_snowflakes ... ok
test validate_automod_rule_request_enforces_name_bounds_and_event_type ... ok
test validate_automod_rule_request_validates_action_types_and_timeout_duration ... ok
test validate_automod_rule_request_validates_mention_spam_spam_preset_and_unknown_triggers ... ok
test upsert_automod_rule_request_serde_roundtrip ... ok
test validate_automod_rule_request_validates_keyword_trigger_bounds ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
EXIT=0
```

Тесты сверяются с контрактом (значения констант, точные строки маршрутов, границы `0/1/60/61/100/101/1000/1001/50/51/2_419_200/2_419_201`), а не с текущей реализацией. Проверки на границах включают валидные и невалидные значения для каждой размерности — 9 обязательных измерений закрыты по позитивным, негативным, граничным и ошибочным сценариям; «пустое» состояние покрыто пустым именем и пустым `keyword_filter`.

## 3. Мутационное тестирование — 5/5 убито

Каждая мутация вносилась в `rust/gui-spike/src/automod.rs`, прогонялся targeted-тест, затем файл восстанавливался из байтовой копии.

| # | Мутация | Результат |
|---|---|---|
| 1 | `MAX_AUTOMOD_NAME_CHARS: 100 → 101` | FAILED — `automod_constants_match_discord_contract` (6 passed; 1 failed), exit=101 |
| 2 | `AUTOMOD_TRIGGER_SPAM: 3 → 2` | FAILED — `automod_constants_match_discord_contract` + `..._mention_spam_spam_preset_and_unknown_triggers` (5 passed; 2 failed), exit=101 |
| 3 | `1..=MAX_KEYWORD_ITEM_CHARS` → `1..=MAX_KEYWORD_ITEM_CHARS + 1` | FAILED — `..._validates_keyword_trigger_bounds` (6 passed; 1 failed), exit=101 |
| 4 | `1..=MAX_MENTION_TOTAL_LIMIT` → `1..=MAX_MENTION_TOTAL_LIMIT - 1` | FAILED — `..._mention_spam_spam_preset_and_unknown_triggers` (6 passed; 1 failed), exit=101 |
| 5 | удалено условие `value.starts_with('0')` в `validate_snowflake` | FAILED — `build_guild_automod_route_..._validates_snowflakes` (6 passed; 1 failed), exit=101 |

**Мутационный счёт: 5/5 (100%).** Ни одна мутация не выжила — тесты реально держат границы констант, длин и валидацию Snowflake.

## 4. Восстановление файла и write-scope

- `rust/gui-spike/src/automod.rs` после всех мутаций: `md5 = f789253558955d8a123fd2b6adf99d48` — совпадает с `REPORT.md` §6.
- `cmp` с байтовой копией до начала мутаций: `BYTE_IDENTICAL=yes`.
- `rust/gui-spike/tests/automod_contract.rs`: `md5 = d439a60fb588560edc577315a2b45d98` — совпадает с `REPORT.md` §6.
- `rust/gui-spike/Cargo.lock` перегенерировался при прогоне (тот же дрейф `zstd`, что описан в `REPORT.md` §7) и откатан через `git checkout` — вне write-scope не осталось изменений, внесённых ревью.
- `package-lock.json` был изменён в дереве до начала ревью, не трогался.
- `git commit` не выполнялся, workspace-тесты и `clippy` не запускались (token-lean).

## 5. Итог

Контракт реализован точно, тесты независимы от реализации, мутации 5/5 убиты, файлы восстановлены байт-в-байт. **APPROVED** — карточку `task-249` можно переводить в `Ready for Merge` после приёмки PM.
