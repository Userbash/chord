# Отчёт воркера — `bugfix/task-128-clippy-clean-all-targets` (task-128: clippy `-D warnings` для gui-spike)

## 1. Batch / Карточка
- **Волна**: `RW-40`, отдел `G1`
- **Карточка / Ветка**: `task-128` (Review 45A: предупреждения `cargo clippy --all-targets -- -D warnings` в `rust/gui-spike`)
- **Роль**: G1 Backend Engineer (`tom_private` не задействован)
- **Инструменты**: `clippy 0.1.99 (b940084d7e 2026-09-28)`, `rustc 1.99.0`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`
- **Независимый оракул**: сам `clippy` с `-D warnings` (код выхода), тесты `net_contract` / `ipc_wiring_contract` без правок.

## 2. Что сделано
Оба предупреждения — `clippy::type_complexity`. Правка только выносит тип в `type`-алиас, поведение и проверки тестов не менялись.

1. `rust/gui-spike/src/main.rs` (`mod wiring_tests`, около строки 1703):
   ```rust
   /// Журнал PUT-вызовов fake-транспорта: `(upload_url, content-type, тело)`.
   type PutCalls = std::sync::Arc<std::sync::Mutex<Vec<(String, String, Vec<u8>)>>>;

   struct FakePutTransport {
       status: u16,
       puts: PutCalls,
   }
   ```
2. `rust/gui-spike/tests/net_contract.rs` (около строки 782):
   ```rust
   /// Колбэк доставки событий, который драйвер получает в `connect`.
   type ReadyCallback = Arc<dyn Fn(GatewayEvent) + Send + Sync>;

   struct DeferredReadyGateway {
       callback: Arc<Mutex<Option<ReadyCallback>>>,
   }
   ```
Стиль совпадает с уже принятыми алиасами в `tests/attachments_contract.rs` (`type PutCalls = Arc<Mutex<Vec<(String, String, Vec<u8>)>>>;`). Ни один assert, фильтр или `#[ignore]` не тронут.

## 3. RED (`red.log`)
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target \
  cargo clippy --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
```
`error: very complex type used` → `tests/net_contract.rs:782:15`, `src/main.rs:1703:15`; `EXIT=101`.

## 4. GREEN (worktree `bugfix/task-128-clippy-clean-all-targets`, `green.log`)
```sh
cargo clippy --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings   # EXIT=0
cargo fmt    --manifest-path rust/gui-spike/Cargo.toml -- --check                      # EXIT=0
cargo test   --manifest-path rust/gui-spike/Cargo.toml --test net_contract --test ipc_wiring_contract
#   ipc_wiring_contract: ok. 9 passed; 0 failed
#   net_contract:        ok. 24 passed; 0 failed
#   EXIT=0
```
Дополнительно: `cargo test --bin discord-gui-spike` — `ok. 12 passed; 0 failed`, `EXIT=0` (тестовый модуль `main.rs`, где живёт `FakePutTransport`, компилируется и проходит).

### Оговорки к среде (не относятся к моей правке)
- Назначенный worktree изначально был равен `unstable`/`main` (`4e24048`) и не содержал кода RW-40; по решению координатора актуальные `rust/gui-spike/src/*.rs` и `tests/*.rs` скопированы из `/home/user/orca/project` (там то же дерево грязное). Дополнительно я скопировал `rust/gui-spike/ui/` (в worktree было 27 `*.js`, оракул `ipc_wiring_contract` требует ≥30; в проекте 32).
- Для прогона `--all-targets` в worktree локально добавлена одна строка `pub mod dm_friends;` в `rust/gui-spike/src/lib.rs`: без неё чужой `tests/dm_friends_contract.rs` (карточка task-112) не компилируется (`E0432: unresolved import discord_gui_spike::dm_friends`). В `/home/user/orca/project` эта строка **не** вносилась — `src/lib.rs` не мой файл.

## 5. Состояние `/home/user/orca/project` после переноса правки
Исправленные файлы скопированы туда; md5 совпадают с worktree.
```sh
cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test net_contract --test ipc_wiring_contract -- -D warnings  # EXIT=0
cargo fmt    --manifest-path rust/gui-spike/Cargo.toml -- --check                                                      # EXIT=0
cargo clippy --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings                                    # EXIT=101
```
Полный `--all-targets` в проекте по-прежнему красный, но **только** на чужом коде карточки task-112, который правился параллельно и после моих прогонов:
- `tests/dm_friends_contract.rs:17` → `E0432: unresolved import discord_gui_spike::dm_friends` (нет `pub mod dm_friends` в `src/lib.rs`);
- `tests/ipc_wiring_contract.rs:1143` → `UI-команда discord_rest_request из ui/features/dm-friends.js не зарегистрирована в generate_handler! main.rs` (файл `ui/features/dm-friends.js` создан в 21:11, уже после моих прогонов).
Оба `type_complexity` из Review 45A в проекте больше не воспроизводятся.

## 6. Файлы + md5
| Файл | md5 |
|---|---|
| `rust/gui-spike/src/main.rs` | `88e54b69be3a90146576dc47e2ffd20a` |
| `rust/gui-spike/tests/net_contract.rs` | `e3a98bdb2b9144c0a395dcc6ec4109bf` |
| `rust/gui-spike/src/lib.rs` (только worktree, `pub mod dm_friends;`) | `155ad039f672518b8298fb1b52789560` (проект, без изменений) |
| `waves/RW-40/G1/task-128-clippy-clean/red.log` | см. файл |
| `waves/RW-40/G1/task-128-clippy-clean/green.log` | см. файл |

`git diff` в проекте по двум файлам: `main.rs` — +986/−2 строки (включая чужой несохранённый код RW-40), `net_contract.rs` — +105/−0; мой вклад в них — 3 строки алиаса + 1 поле в каждом (см. §2).

## 7. Риски
- Полная зелёная приёмка `--all-targets` в `/home/user/orca/project` возможна только после того, как task-112 зарегистрирует `discord_rest_request` и подключит `pub mod dm_friends;` — это вне моих файлов, требует координатора.
- Worktree синхронизирован вручную на момент 21:09–21:11; дальше он расходится с проектом (task-112 продолжает писать). Для повторного прогона нужна повторная синхронизация `src/*.rs`, `tests/*.rs`, `ui/`.
- `package-lock.json` в worktree был изменён до меня — не трогал.
