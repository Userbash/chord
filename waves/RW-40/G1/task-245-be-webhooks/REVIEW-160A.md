# Техническое ревью 160A: task-245 (`BE-P1-27` — Rust Webhooks & Channel Follow BFF Contract)

- **Batch / Карточка:** `RW-40` / `G1` / `task-245` (`BE-P1-27`)
- **Ветка (worktree):** `feature/task-245-be-webhooks`
- **Ревьюер:** Technical Reviewer (DeepSeek)
- **Verdict: APPROVED**

## 1. Объект ревью

| Файл | MD5 на момент ревью |
|---|---|
| `rust/gui-spike/src/webhooks.rs` | `f4aab665ef767512aa9693affebb863d` |
| `rust/gui-spike/tests/webhooks_contract.rs` | `1ec829a980b532208aadd86d01b413b7` |
| `/home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/REPORT.md` | прочитан |

MD5 исходника совпадает с заявленным в `REPORT.md` §4 (`f4aab665ef767512aa9693affebb863d`), теста — с `1ec829a980b532208aadd86d01b413b7`.

## 2. Targeted-прогон (T0)

```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test webhooks_contract
```

Результат: `exit=0`, `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. Совпадает с `green.log` из отчёта. Полный `--workspace`, `clippy`, `fmt` не запускались — по ТЗ это T1 integration-лейна.

## 3. Мутационное тестирование 5/5

Методика: каждый мутант вносился точечной заменой ровно одной строки в `rust/gui-spike/src/webhooks.rs`, прогонялся тот же targeted-тест, затем файл восстанавливался из побайтовой копии. Итоговая сверка: `FINAL_MD5=f4aab665ef767512aa9693affebb863d`, `cmp` с копией — побайтовое совпадение.

| # | Мутация | Ожидание | Факт |
|---|---|---|---|
| M1 | `MAX_WEBHOOK_NAME_CHARS: usize = 80` → `81` | тест падает | killed: `5 passed; 3 failed` |
| M2 | в `validate_snowflake`: `value.len() > 20` → `> 21` | тест падает | killed: `6 passed; 2 failed` |
| M3 | `lowered == "clyde"` → `lowered == "klyde"` | тест падает | killed: `6 passed; 2 failed` |
| M4 | маршрут `/webhooks` → `/webhook` в `build_create_webhook_route_and_body` | тест падает | killed: `7 passed; 1 failed` |
| M5 | `if news_channel_id == target_channel_id` → `!=` (инверсия запрета самоподписки) | тест падает | killed: `6 passed; 2 failed` |

**Итог: 5/5 мутаций убиты (100%).** Ни одна мутация не выжила; откат исходника подтверждён md5 и `cmp`.

Логи: `/tmp/opencode/baseline.log`, `/tmp/opencode/mut_M1_MAX_NAME.log` … `/tmp/opencode/mut_M5_SELF_FOLLOW.log`.

## 4. Проверка контракта по существу

- **Константы:** `WEBHOOK_TYPE_INCOMING = 1`, `WEBHOOK_TYPE_CHANNEL_FOLLOWER = 2`, `WEBHOOK_TYPE_APPLICATION = 3`, границы имени `1..=80` — соответствуют Discord API v9 и ссылкам на `/home/sanya/re-deep/rest-static.json` (`CHANNEL_WEBHOOKS`, `CHANNEL_FOLLOWERS`).
- **Snowflake:** `1..=20` ASCII-цифр, без ведущего `'0'`, отрицательные/пробельные/буквенные отклоняются; границы `"1"` и 20 девяток покрыты тестами.
- **`validate_webhook_name`:** `trim()` применяется до подсчёта длины; длина считается `chars().count()` (`ж` × 80 = 160 байт проходит), запрет `"clyde"` — точное совпадение после `to_lowercase()`, `ClydeBot` разрешён, `"discord"` — подстрока без учёта регистра.
- **Маршруты и тела:** `/api/v9/channels/{channel_id}/webhooks` с `{"name", "avatar"}` (включая `null`) и `/api/v9/channels/{news_channel_id}/followers` с `{"webhook_channel_id"}`; самоподписка отклоняется явной ошибкой.
- **Ошибки:** все ветки возвращают `Err(String)` с русским текстом и именем поля; молчаливого проглатывания нет.
- **Границы файлов:** изменены только `rust/gui-spike/src/webhooks.rs`, `rust/gui-spike/src/lib.rs` (экспорт `pub mod webhooks;`) и новый `tests/webhooks_contract.rs` — в пределах ТЗ, чужие модули не затронуты.

## 5. Замечания

Блокирующих и неблокирующих замечаний нет (`P0 = 0`, `P1 = 0`).

## 6. Вердикт

**APPROVED.** Контракт реализован по оракулу, red/green-дисциплина подтверждена, targeted-тест зелёный (8/8), мутации 5/5 убиты, исходник восстановлен байт-в-байт.
