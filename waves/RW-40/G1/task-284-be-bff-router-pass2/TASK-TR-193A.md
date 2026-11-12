# ТЗ Технического ревью — TR-193A (`task-284` / `PR-164`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/user/orca/workspaces/project/bugfix-task-284-be-bff-router-pass2`.
Файлы:
- `rust/gui-spike/src/bff_domains_router.rs` (`c2e178bd8db5a104d5b8b2978e6b23ed`)
- `rust/gui-spike/tests/bff_domains_router_contract.rs` (`d851e83f8786ac4ff345219e4b654c19`)

## Шаги (коротко, без лишних чтений)
1. Запусти `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract` (`10/10 PASS`).
2. Выполни **5 точечных мутаций** в `rust/gui-spike/src/bff_domains_router.rs` (включая мутации на метод `"PATCH"`/`"POST"` в `resolve_bff_domain_action`), при восстановлении НЕ используй `cp -p` (чтобы `mtime` обновлялся!), убедись что `5/5` убиты, восстанови файл, откати `rust/gui-spike/Cargo.lock` и сверь MD5 (`c2e178bd8db5a104d5b8b2978e6b23ed`).
3. Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/G1/task-284-be-bff-router-pass2/REVIEW-193A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
