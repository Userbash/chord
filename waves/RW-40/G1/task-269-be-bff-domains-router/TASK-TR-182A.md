# ТЗ Технического ревью — TR-182A (`task-269` / `PR-153`: `bff_domains_router.rs`)

## Контекст
Проверяется рабочая ветка `feature/task-269-be-bff-domains-router` (`/home/user/orca/workspaces/project/feature-task-269-be-bff-domains-router`).
Файлы:
- `rust/gui-spike/src/bff_domains_router.rs` (MD5 `b59afec01f123e50b04e685276ae003d`)
- `rust/gui-spike/src/lib.rs` (MD5 `93d19a31d69ded1e448f330e79db6fd2`)
- `rust/gui-spike/tests/bff_domains_router_contract.rs` (MD5 `cf7dcc67c12e2e047a182814823cecd4`)

## Обязательные шаги проверки
1. Проверить `git status --short` и отсутствие посторонних изменений вне scope задачи (`Cargo.lock` не должен оставаться изменённым).
2. Запустить контрактные тесты и `clippy` с общим `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-gui-spike --test bff_domains_router_contract`
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml -p discord-gui-spike --test bff_domains_router_contract -- -D warnings`
3. Выполнить **5 точечных мутаций** в `rust/gui-spike/src/bff_domains_router.rs` (внимание: при восстановлении бэкапа НЕ использовать `cp -p`, чтобы `mtime` обновлялся и `cargo test` гарантированно пересобирал крейт!). Все `5/5` мутаций должны быть убиты тестами (`bff_domains_router_contract`). После 5-й мутации восстановить файл и сверить MD5 (`b59afec01f123e50b04e685276ae003d`) и `git checkout -- rust/Cargo.lock`.
4. Записать полный отчёт в `/home/user/orca/project/waves/RW-40/G1/task-269-be-bff-domains-router/REVIEW-182A.md` (вердикт `APPROVED` при `P0=0, P1=0` и `5/5` убитых мутациях).
5. Завершить работу вызовом `orca orchestration worker-done`. Не делать `git commit`.
