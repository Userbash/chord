# task-134 — исправление P0 сборки BUILD-004

- **Карточка:** `task-134` (`task_236e9c53090d`, `ctx_2d740bd18419`), ветка `bugfix/task-134-fix-build004-integration`
- **Роль:** Integration / Backend-Frontend Engineer (DeepSeek)
- **Основание:** `waves/RW-40/QUALITY/QA-BUILD-004-REPORT.md`, дефекты `P0-1`, `P0-2`, `P0-3`
- **Дата:** 2026-10-30
- **Вердикт по трём P0:** закрыты, обязательный объём QA зелёный (6/6, 15/15, 19/19, 102/102)

## Сделано

| Дефект | Файл | Изменение |
|---|---|---|
| P0-1 | `rust/gui-spike/src/lib.rs` | Добавлены `pub mod dm_friends;` (после `data`) и `pub mod voice_devices;` (после `typing`) с док-комментариями в стиле соседних модулей. Контрактные тесты видят модули как часть крейта `discord_gui_spike`, а не через `#[path]`. |
| P0-2 | `rust/gui-spike/ui/features/dm-friends.js` | В `sendRest` убран fallback `options.invoke("discord_rest_request", req)` и поддельный успех `{ status: 200, ok: true, body: {} }`. REST идёт только через переданный адаптер `options.request`; без адаптера функция бросает `Error` с методом и URL, чтобы вызывающий код получил честный отказ через `reportActionError`, а не тихий успех. JSDoc `invoke?` оставлен: его передают монтажные тесты, тип не сужался. |
| P0-3 | `TDD-PLAN.md` §8 | Добавлена строка `tests/unit/dm-friends-ui.test.mjs` → `rust/gui-spike/ui/features/dm-friends.js` с токенами `createDmFriendsController`, `DM_FRIENDS_ENDPOINTS`, `FRIENDS_TABS`, `applyDmFriendsDispatch`, `sortPrivateChannels`. Строка стоит сразу после `gui-spike-features`, формат — ровно три колонки, как требует `tests/meta/plan-sync.test.mjs`. |

`discord_rest_request` в репозитории не реализована, поэтому выбран второй вариант из QA-рекомендации
(«убрать `invoke`-fallback и требовать `options.request` явно»), а не регистрация команды в `main.rs`:
`main.rs` в моём ТЗ отсутствует.

## Red-прогон (до правки, дерево `/home/sanya/orca/project`)

Проверял в `/home/sanya/orca/project`, потому что в рабочем worktree задачи нет части untracked-файлов
сборки (`attachments.rs`, `notifications.rs`, `pins_threads.rs`, `voice-devices-settings.js`,
`notifications.js`, `pins-threads.js`, `markdown.js`, `invite-modal.js`, `tests/e2e/m2-*.mjs`), и крейт
там не компилируется по причинам, не относящимся к task-134. Состояние трёх целевых файлов в
`/home/sanya/orca/project`, `unstable`, `testing` и в worktree было побайтово одинаковым
(`lib.rs 155ad039…`, `dm-friends.js be2e2854…`, `TDD-PLAN.md 69a6ee4f…`).

| Команда | Результат | Код |
|---|---|---|
| `cargo test --manifest-path rust/gui-spike/Cargo.toml --test dm_friends_contract --test voice_devices_contract` | `error[E0432]: unresolved import discord_gui_spike::dm_friends`; `voice_devices_contract` не запускался | 101 |
| `node --test tests/meta/*.test.mjs` | 101/102, падает `plan-sync`: «тестовые файлы без строки в §8 → tests/unit/dm-friends-ui.test.mjs» | 1 |
| `node --test tests/unit/dm-friends-ui.test.mjs tests/unit/gui-spike-features.test.mjs` | 129/130, падает `INT-07`: `discord_rest_request (features/dm-friends.js:1351)` | 1 |

Логи: `logs/red-cargo.log`, `logs/red-meta.log`, `logs/red-unit.log`.

## Green-прогон (после правки, дерево `/home/sanya/orca/project`)

| Команда | Результат | Код |
|---|---|---|
| `cargo test --manifest-path rust/gui-spike/Cargo.toml --test dm_friends_contract --test voice_devices_contract` | `dm_friends_contract` 6/6, `voice_devices_contract` 15/15 | 0 |
| `node --test tests/meta/*.test.mjs` | 102/102, 17 suites | 0 |
| `node --test tests/unit/dm-friends-ui.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-dom.test.mjs` | 168/168 | 0 |

По файлам: `dm-friends-ui` 19/19 (4 suites), `gui-spike-features` 111/111 (15 suites), `gui-spike-dom` 38/38.
Логи: `logs/green-cargo.log`, `logs/green-meta.log`, `logs/green-unit.log`, `logs/dm-friends-ui.log`,
`logs/gui-spike-features.log`, `logs/gui-spike-dom.log`.

## Дополнительные проверки

| Проверка | Результат | Код |
|---|---|---|
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | чисто | 0 |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | 0 предупреждений (модули теперь в крейте и линтуются) | 0 |
| `npm run typecheck` (`tsc` 7.0.2, оба конфига) | чисто | 0 |
| `npm run gate:worker` | см. раздел «Gate» | см. там же |

Повтор обязательных проверок в остальных деревьях сборки (файлы побайтово те же):

| Дерево | `dm_friends_contract` + `voice_devices_contract` | meta | unit (dm-friends + features + dom) |
|---|---|---|---|
| `/home/sanya/orca/workspaces/project/testing` | 6/6 + 15/15, exit 0 | 102/102, exit 0 | 168/168, exit 0 |
| `/home/sanya/orca/workspaces/project/unstable` | 6/6 + 15/15, exit 0 | 102/102, exit 0 | 168/168, exit 0 |

## Gate

`npm run gate:worker` в `/home/sanya/orca/project` — **passed**, код 0, фазы:

```text
✔ typecheck      0.3 с
✔ ui-tsc         0.3 с
✔ meta           0.1 с  (102: pass 102, fail 0, skip 0)
✔ unit           2.9 с  (934: pass 934, fail 0, skip 0)
✔ spike-rust    20.7 с
✔ workspace-rust 10.1 с (1158: pass 1146, fail 0, skip 12)
✔ smoke         15.4 с  (22: pass 22, fail 0, skip 0)
Вердикт: passed
```

Полный unit-прогон вырос с 933/934 (QA-BUILD-004) до 934/934: единственное падение было `INT-07`.
Лог: `logs/gate-worker.log`, артефакт гейта `.gate/worker-2026-10-30T16-39-13-422Z.json`.

## Oracle

- `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (md5 `3aacc65a33ca8a24e609c2046c85625f`) — маршруты, коды, сортировка ЛС.
- `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md`, `voice-spec.json` — контракт голосовых устройств.
- `rust/gui-spike/src/main.rs` — источник `generate_handler![…]` для инварианта `INT-07`.
- `tests/meta/plan-sync.test.mjs` — формат §8; строка проверена им же.

Генерация и мутации: не применимо — правка интеграционная (2 строки модулей, удаление fallback, одна
строка таблицы), новых контрактов и генераторов нет.

## Файлы и md5 (финальные)

```text
356efe9e856fce4053e500f8e8bd50ea  rust/gui-spike/src/lib.rs
63cd550d20f7874c946c4040f932ec4d  rust/gui-spike/ui/features/dm-friends.js
591132b4bb81c0554a2aa210bd2ffaab  TDD-PLAN.md
```

Побайтово одинаковы в четырёх деревьях: `/home/sanya/orca/workspaces/project/bugfix-task-134-fix-build004-integration`,
`/home/sanya/orca/project`, `/home/sanya/orca/workspaces/project/unstable`,
`/home/sanya/orca/workspaces/project/testing`.

Не трогал: `rust/gui-spike/src/main.rs`, `dm_friends.rs`, `voice_devices.rs`, `tests/dm_friends_contract.rs`,
`tests/voice_devices_contract.rs`, `tests/unit/dm-friends-ui.test.mjs`, `tests/unit/gui-spike-features.test.mjs`,
`logs/`, `functions/`, `evidence/`, `package.json`, `waves/db.jsonl`, `waves/STATE.md`.

## Риски и что осталось

- **P1-2 не закрыт:** `tests/voice_devices_contract.rs` по-прежнему грузит модуль через
  `#[path = "../src/voice_devices.rs"]`, а не `use discord_gui_spike::voice_devices::…`. Тест зелёный
  (15/15) и теперь модуль в крейте, но сам тест дыру «модуль не подключён» не поймал бы. Файл вне моего ТЗ.
- **P3-1 не закрыт:** `dm-friends.js` не смонтирован в `app.js`/`index.html`. Новый отказ по отсутствию
  `request` в рантайме виден только после реального монтирования; сейчас контроллер недостижим.
- **P1-1 не закрыт:** состав BUILD-004 всё ещё не зафиксирован коммитом; коммиты — уровень L1.
- **Worktree задачи не самодостаточен:** в нём отсутствует часть untracked-файлов сборки, поэтому
  `cargo` и часть node-наборов там красные по чужим причинам. Все прогоны выше сделаны в полном дереве
  `/home/sanya/orca/project`; перед приёмкой стоит либо досоздать worktree из состава сборки, либо
  проверять в `testing`.
- Правка `lib.rs` наложена поверх уже лежавших в рабочем дереве изменений INT-01-RUST-WIRING
  (`attachments`, `notifications`, `pins_threads` и IPC-обёртки) — они не мои и я их не менял.
