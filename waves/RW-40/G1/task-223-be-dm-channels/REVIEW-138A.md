# Техническое ревью 138A — task-223 (`dm_channels`)

**PR**: #109 · ветка `feature/task-223-be-dm-channels` · отдел G1 (Backend)
**Ревьюер**: DeepSeek V4.1 Flash (Technical Reviewer)
**Объём**: `rust/gui-spike/src/dm_channels.rs`, `rust/gui-spike/tests/dm_channels_contract.rs`, `REPORT.md`

## Вердикт: APPROVED

Targeted-прогон зелёный: `cargo test --manifest-path rust/gui-spike/Cargo.toml --test dm_channels_contract`
→ `8 passed; 0 failed`. Мутационное тестирование: **8/8 убито** (требование 5/5 выполнено с запасом).
Файл `dm_channels.rs` после мутаций восстановлен байт-в-байт, md5 совпал с `REPORT.md`.

## Мутации

| # | Мутация | Результат | Убивший тест |
|---|---|---|---|
| M1 | `MAX_GROUP_DM_RECIPIENTS: 9 → 10` | KILLED | `dm_channels_constants_match_discord_contract`, `validate_create_dm_request_enforces_recipient_count_bounds`, `build_create_dm_body_propagates_validation_errors` |
| M2 | `MAX_GROUP_DM_NAME_CHARS: 100 → 99` | KILLED | `dm_channels_constants_match_discord_contract` |
| M3 | Отключена проверка пустого `recipient_ids` (`if false`) | KILLED | `validate_create_dm_request_enforces_recipient_count_bounds`, `build_create_dm_body_propagates_validation_errors` |
| M4 | Верхняя граница `len() > MAX` → `len() >= MAX` | KILLED | `validate_create_dm_request_enforces_recipient_count_bounds`, `build_create_dm_body_returns_recipient_id_for_single_and_recipients_for_group` |
| M5 | Отключена проверка дубликатов (`if false`) | KILLED | `validate_create_dm_request_rejects_duplicate_recipients`, `build_create_dm_body_propagates_validation_errors` |
| M6 | Убрана проверка ведущего нуля Snowflake (`starts_with('0')`) | KILLED | `validate_create_dm_request_checks_canonical_positive_u64_snowflakes`, `build_group_dm_recipient_route_returns_canonical_path_and_validates_inputs`, `build_create_dm_body_propagates_validation_errors` |
| M7 | Маршрут `recipients` → `recipient` | KILLED | `build_group_dm_recipient_route_returns_canonical_path_and_validates_inputs` |
| M8 | Ветка одиночного DM `len() == 1 → == 2` | KILLED | `build_create_dm_body_returns_recipient_id_for_single_and_recipients_for_group` |

## Контракт и код

- Сверка с оракулом по `REPORT.md`: `USER_CHANNELS` (`POST /users/@me/channels`) — одиночный DM `{"recipient_id": ...}`, Group DM `{"recipients": [...]}`; `CHANNEL_RECIPIENT` (`/channels/{channel_id}/recipients/{user_id}`); лимит `9` получателей, `100` символов названия. Комментарии-источники в модуле указывают точные строки `rest-static.json`.
- `validate_snowflake` отклоняет пустую строку, `0`, ведущие нули, не-ASCII-цифры, переполнение `u64`; `1` и `u64::MAX` принимаются. Тесты покрывают и одиночный, и групповой список.
- Дубликаты ловятся по префиксу `[..idx]`, включая несмежные; сложность O(n²) допустима при `n ≤ 9`.
- `build_create_dm_body` пробрасывает ошибки валидации, поэтому обходить `validate_create_dm_request` нельзя.
- Границы из 9 измерений покрыты: позитив/негатив (`u64::MAX`, `u64::MAX+1`), границы (`1`/`9`/`10`), пустой список, дубликаты, канонический маршрут.
- md5 после мутаций: `f66582c3a8cd321ba88207f3d74cc9d0` — совпадает с `REPORT.md`; тесты `265727fd8085f43126fac015bd423d8a` без изменений.
- Замечаний уровня P0/P1 нет. P2: `MAX_GROUP_DM_NAME_CHARS` проверяется только константой — валидатора названия в карточке нет, это не нарушение текущего ТЗ.

`git commit` не выполнялся.
