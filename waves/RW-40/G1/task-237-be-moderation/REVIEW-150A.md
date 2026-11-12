# REVIEW-150A — task-237 (`BE-P1-24`, `moderation`), техническое ревью

**Вердикт: APPROVED**

- Ревьюер: техническое ревью (мутационное тестирование по политике 5/5).
- Worktree: `feature/task-237-be-moderation`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- Дата: 2026-10-31.

## 1. Что проверялось

Только назначенный срез: `rust/gui-spike/src/moderation.rs`, `rust/gui-spike/tests/moderation_contract.rs`
и `waves/RW-40/G1/task-237-be-moderation/REPORT.md`. Полный `gate:worker`, workspace-тесты и `git commit`
не запускались — по ТЗ ревью targeted-уровня.

Целевой прогон (`cargo test --test moderation_contract`) на исходном дереве до мутаций:

```text
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

## 2. Мутационное тестирование 5/5

Каждая мутация вносилась в `rust/gui-spike/src/moderation.rs` по одной, прогонялся тот же
targeted-тест, затем файл восстанавливался из побайтовой копии.

| # | Мутация | Ожидание | Факт | Убита |
|---|---|---|---|---|
| M1 | `MAX_AUDIT_REASON_CHARS: usize = 512` → `511` | тест констант и границы 512 падает | `FAILED. 6 passed; 2 failed`, exit 101 | да |
| M2 | `char_count > MAX_AUDIT_REASON_CHARS` → `>=` | ровно 512 символов отвергается | `FAILED. 7 passed; 1 failed`, exit 101 | да |
| M3 | убрано условие `value.starts_with('0')` в `validate_snowflake` | `"00"`, `"0123456789"` проходят | `FAILED. 6 passed; 2 failed`, exit 101 | да |
| M4 | `delete_message_seconds > MAX_BAN_DELETE_MESSAGE_SECONDS` → `>=` | граница 604 800 отвергается | `FAILED. 7 passed; 1 failed`, exit 101 | да |
| M5 | `(1..=MAX_AUDIT_LOG_LIMIT).contains(&limit)` → `(1..MAX_AUDIT_LOG_LIMIT)` | `limit=100` отвергается | `FAILED. 7 passed; 1 failed`, exit 101 | да |

Итог: **5/5 мутаций убиты**, ни одна не выжила.

## 3. Восстановление файла

```text
ORIG md5 = b9a7b9c21dcbe962b99b7bad25535c66   (совпадает с REPORT.md §6)
AFTER md5 = b9a7b9c21dcbe962b99b7bad25535c66
cmp orig.rs moderation.rs -> BYTE-IDENTICAL=yes
```

Финальный контрольный прогон после восстановления:

```text
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Файл вернулся к состоянию сдачи побайтово, md5 совпадает с заявленным в отчёте воркера.

## 4. Замечания

- Контракт покрыт по всем заявленным осям: константы Discord API v9 (`512` / `604_800` / `2_419_200` / `100`),
  serde round-trip `BanMemberRequest`, нормализация причины (`None`/пробелы/trim), границы длины причины
  на ASCII и кириллице, каноничность Snowflake (`""`, `"0"`, ведущие нули, знаки, пробелы, `u64::MAX + 1`),
  границы `delete_message_seconds` (`0`/`86_400`/`604_800` — Ok; `604_801`/`1_000_000`/`u32::MAX` — Err),
  границы `limit` (`0`/`101`/`u8::MAX` — Err) и наличие/отсутствие `action_type` в маршруте.
- Тесты сверяют контракт с независимым оракулом (строки маршрутов и лимиты Discord v9), а не с текущей
  реализацией: мутации M1–M5 меняют поведение, тесты ловят каждую.
- Блокирующих замечаний нет: `P0 = 0`, `P1 = 0`.

Вердикт: **APPROVED**, срез готов к приёмке PM и дальнейшей интеграции в `unstable`.
