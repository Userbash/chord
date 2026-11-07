# REVIEW-140A — PR-111 (task-228, BE-P1-21 invites contract)

**Verdict: APPROVED**

- Ветка/worktree: `feature/task-228-be-invites-contract` (HEAD `763dda5` + незакоммиченные изменения задачи).
- Scope совпадает с REPORT.md: `rust/gui-spike/src/invites.rs`, `src/lib.rs`, `tests/invites_contract.rs`, отчёт волны. `package-lock.json` изменён до старта задачи, не трогался.
- Targeted: `cargo test --manifest-path rust/gui-spike/Cargo.toml --test invites_contract` -> **25 passed; 0 failed** (exit 0).
- `git commit` не выполнялся.

## Мутации (5/5 killed)

Каждая мутация применялась к `src/invites.rs`, прогонялся targeted-тест, затем файл восстанавливался из копии в `/tmp/opencode/mut/orig.rs`.

| # | Мутация | Ожидание | Результат | Итог |
|---|---|---|---|---|
| M1 | `validate_invite_code`: граница длины `2..=32` -> `1..=32` | fail | FAILED: 23 passed; 2 failed (exit 101) | killed |
| M2 | `validate_invite_code`: удалён `-` из allowlist символов | fail | FAILED: 23 passed; 2 failed (exit 101) | killed |
| M3 | `build_invite_resolve_route`: перепутаны `with_counts` / `with_expiration` | fail | FAILED: 24 passed; 1 failed (exit 101) | killed |
| M4 | `validate_canonical_snowflake`: убрана проверка ведущих нулей | fail | FAILED: 24 passed; 1 failed (exit 101) | killed |
| M5 | `ALLOWED_INVITE_MAX_AGES`: `2592000` -> `2592001` | fail | FAILED: 23 passed; 2 failed (exit 101) | killed |

Восстановление: md5 `src/invites.rs` = `dcf41477b5f5842b51ff389d992e74c4`, тестов `invites_contract.rs` = `1b97e688dafd39d9c5a4463cf7541ed8` — совпадает с REPORT.md.

## Качество контракта

- Тесты проверяют независимые оракулы: точные таблицы RE, канонический Snowflake (`1..=u64::MAX`, отказ `0`, ведущих нулей, `> u64::MAX`), границы кода `2/32` байта и отказ `/ ? # . !` с не-ASCII, полный декартов набор `8 x 6` пар `max_age x max_uses` для маршрута и тела.
- Ошибки — `String` с русским текстом и значениями; сеть не вызывается, поведение детерминировано, тесты не skip-aware и не зависят от окружения.
- Существующие `G-03` / `STUB-05` API не изменены: 20 старых тестов модуля остаются зелёными.

## Замечания

- P2 (унаследовано, вне scope): в модуле две разные allowlist для одного эндпоинта `POST /channels/{id}/invites`: `VALID_INVITE_MAX_AGES` = `[0,1800,3600,21600,43200,86400,604800]` (нет 2592000) против `ALLOWED_INVITE_MAX_AGES` (8 значений, включая 2592000); `VALID_INVITE_MAX_USES` = `[0,1,5,10,25,50,100]` (есть 50) против `ALLOWED_INVITE_MAX_USES` = `[0,1,5,10,25,100]`. Старый `create_channel_invite` (G-03/STUB-05) отклоняет 30-дневный инвайт и принимает 50 использований; нужна отдельная карточка на слияние таблиц. PR-111 это не ломает и не расширяет.
- P2 (наследие): `CreateChannelInviteRequest` дублирует `CreateInviteParams` (поле `channel_id` vs аргумент); для BE-P1-21 это отдельный контракт, консолидацию разумно отложить, но зафиксировать в бэклоге.
- P0/P1 блокирующих: нет. Расхождение таблиц предсуществующее (G-03/STUB-05), PR-111 его не вводит.
