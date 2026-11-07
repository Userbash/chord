# Независимое ревью — task-109-ipc-oracle-strict (закрытие P2-1/P2-2/P2-3 из INT-06)

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Ветка: `bugfix/task-109-ipc-oracle-strict`. Продуктовый код ревьюером не менялся:
обе мутационные правки откатывались из копии, финальные MD5 совпадают с REPORT.md.

## Вердикт

**PASS. P0 = 0, P1 = 0, P2 = 0.**
Все три замечания P2 из `waves/RW-40/PM/int06-notif-ipc/REVIEW.md` закрыты по существу, а не
подменой формулировок: оракул действительно разбирает `AUTH_COMMANDS`/`JOIN_COMMANDS`/
`NET_COMMANDS`/`READY_COMMAND` и подтверждает 38/38 (проверено независимым парсером),
`into_patch` возвращает `Err(NotificationError::InvalidChannelId)` при чужом канале,
поле `override` принимается командой `update_channel_notification_override` и маппится Tauri
в ключ `"override"`. Артефакты red/green/mutations воспроизводятся, MD5 всех пяти файлов из
REPORT.md совпадают. Неблокирующие замечания — в конце (P3).

## Проверка закрытия P2 (по существу, а не по тексту отчёта)

### P2-1 — оракул 38/38 UI IPC-команд

| Что проверено | Результат | Evidence |
|---|---|---|
| `extract_ui_invoke_commands` разбирает константы | да: `UI_IPC_COMMAND_CONSTANTS` = `AUTH_COMMANDS`, `JOIN_COMMANDS`, `NET_COMMANDS`, `READY_COMMAND`, поиск `const <NAME>` + первого `{...}`/строки до `;` | `tests/ipc_wiring_contract.rs:826-831,899-949` |
| Канонический список из 38 | да, `ALL_38_UI_IPC_COMMANDS` + `assert_eq!(len, 38)` + точное равенство отсортированного множества | `tests/ipc_wiring_contract.rs:833-872,1157-1168` |
| Регистрация каждой найденной команды в `generate_handler!` | да, `registered.contains(&cmd)` | `tests/ipc_wiring_contract.rs:1143-1146` |
| Объявление `fn <cmd>(` в `main.rs` | да | `tests/ipc_wiring_contract.rs:1147-1150` |
| Обход всех `ui/**/*.js` | да, 32 файла, включая `features/notifications.js` | `tests/ipc_wiring_contract.rs:951-975,1124-1138` |
| Самотест самого оракула (литералы + 4 константы + `invokeFn`) | да, отдельный `assert_eq!` на 8 командах | `tests/ipc_wiring_contract.rs:1102-1122` |

Независимый оракул (свой парсер на Node, не код теста): 32 JS-файла, 38 уникальных команд,
26 литеральных `invoke("…")` + 13 имён из четырёх констант (пересечение по `auth_logout`).
Сверка с `main.rs`: **missing registration = []**, **missing fn decl = []**, 49 записей в
`generate_handler!`, дублей нет. Список 38 команд совпал с `ALL_38_UI_IPC_COMMANDS`
побайтово по составу.

Живучесть теста проверена вручную: удаление `auth_status,` из `generate_handler!` даёт
`panicked at tests/ipc_wiring_contract.rs:1143: UI-команда auth_status из ui/features/auth.js
не зарегистрирована` (FAILED). То есть проверка не вакуумная.

### P2-2 — нет тихого fallback на чужой канал

`ChannelOverrideInput::into_patch` (`src/main.rs:841-860`):

- конверт содержит запрошенный `channel_id` → `Ok(patch)`;
- конверт пуст → `Ok(ChannelNotificationOverridePatch::default())`;
- конверт непуст и не содержит запрошенный `channel_id` → `Err(NotificationError::InvalidChannelId)`;
- `.into_values().next()` в файле отсутствует; `resolve` возвращает `Result` и пробрасывает
  ошибку через `?` в команде (`src/main.rs:869-884,930-937`).

Поведенчески подтверждено тестом `wiring_tests::invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input`
(`main.rs:2072-2105`): `foreign_envelope.into_patch("ch-announce") == Err(InvalidChannelId)`,
`resolve(..., Some(foreign_envelope), None) == Err(InvalidChannelId)`, пустой конверт → `Ok(default)`.
Независимая мутация (возврат `into_values().next()`): падает поведенческий unit-тест
(`main.rs:2082`) и падает контрактный тест — оба реально ловят регрессию.

### P2-3 — поддержка поля `override`

- `ChannelOverrideInput::OverrideField` с `#[serde(rename = "override", alias = "override_field", alias = "overrideField")]` (`main.rs:833-836`), вариант объявлен **до** `Direct`, поэтому `{"override": {...}}` не «съедается» прямым патчем.
- Команда принимает `r#override: Option<ChannelNotificationOverridePatch>` (`main.rs:916`), `resolve` ставит его первым в цепочке (`main.rs:878-883`).
- Ключ IPC проверен по исходникам `tauri-macros-2.7.4/src/command/wrapper.rs:479-502`: `r#override` → `unraw()` → `"override"` → `to_lower_camel_case()` → `"override"` (default `ArgumentCase::Camel`, `wrapper.rs:51`). Совпадает с `notifications.js:1610` (`override: overridePatch`).
- Поведенчески: `{"override": {...}}` → `into_patch` возвращает именно патч, а не пустой default (`main.rs:2133-2154`); вызовы только с `r#override` и только с `override_field` тоже дают нужный патч (`main.rs:2111-2132`).

## MD5 (сверено с REPORT.md §5)

| Файл | Фактический MD5 | Сверка |
|---|---|---|
| `rust/gui-spike/src/main.rs` | `24bb37a8f233213ba7c59dc7b9fb1b22` | = REPORT.md, = baseline в mutations.log |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `01929cf0e635a2e4ef946e416a61801f` | = REPORT.md |
| `waves/.../red.log` | `ce62fc6c62dcc996d69f7a53068269db` | = REPORT.md |
| `waves/.../green.log` | `cf3ac9478a0b6b800dded041a1b6da8a` | = REPORT.md |
| `waves/.../mutations.log` | `fcc1274d02d76fd0e5796ee0f3a0d45a` | = REPORT.md |

Baseline восстанавливался после каждой моей мутации; финальный MD5 `main.rs` равен baseline.

## Прогоны (на текущих байтах, после откатов)

```text
cargo test --offline -p discord-gui-spike --test ipc_wiring_contract  → 9 passed; 0 failed
cargo test --offline -p discord-gui-spike --bin discord-gui-spike     → 12 passed; 0 failed
cargo clippy --offline -p discord-gui-spike --lib --bins --test ipc_wiring_contract -- -D warnings → 0
rustfmt --edition 2021 --check src/main.rs tests/ipc_wiring_contract.rs → exit 0
node --test tests/meta/test-policy.test.mjs   → 17 pass / 0 fail
node --test tests/meta/ipc-path.test.mjs      → 12 pass / 0 fail
```

Skip/`only`/`ignore` в `ipc_wiring_contract.rs` отсутствуют. RED-лог соответствует заявленному:
`EXIT_CODE=101`, `7 passed; 2 failed`, падения на `1456:5` и `1311:5`, оба текста совпадают с
текущими assert'ами. Полный `gate:worker` — зона integration-лейна, этим ревью не подтверждается.

## Замечания (неблокирующие, P3)

1. **`mutations.log` недоказывает MUT-3/MUT-4 по контрактному тесту.** Команда мутаций —
   `cargo test --bins --test ipc_wiring_contract`; после падения bin-таргета cargo останавливается,
   поэтому в логе виден только `wiring_tests::…` (11 passed / 1 failed), а второй убийца из таблицы
   REPORT.md §4 (`channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field`)
   в логе отсутствует. Я проверил вручную: контрактный тест на мутанте MUT-3 действительно падает,
   то есть таблица REPORT.md верна, но лог её не подтверждает. Стоит либо гонять таргеты раздельно,
   либо добавить `--no-fail-fast`.
2. **Слепое пятно оракула на будущее.** 38/38 держится на литералах `invoke(...)`/`invokeFn`/`invokeReaction`
   и четырёх известных константах. Новая UI-команда, вызванная через пятую карту
   (`invoke(SOME_NEW_MAP.x)`), не будет видна: `seen_ui_commands.len()` останется 38 и равенство с
   каноническим списком пройдёт ложно. Для текущего дерева дыр нет (проверено независимым
   парсером: все call-site либо литералы, либо известные константы; `members_page`/`search_page`/
   `settings` UI не вызывает вовсе), но защита от дрейфа неполная. Дешёвое усиление — мета-проверка,
   что каждый `invoke(`-подобный вызов в JS либо литерал, либо `*_COMMANDS`/`READY_COMMAND`.
3. **Контрактный тест `channel_override_input_into_patch_...` — source-text, а не поведенческий.**
   Он сверяет уплотнённый текст `enum`/`impl` (`tests/ipc_wiring_contract.rs:1431-1484`) и сломается
   от безобидного рефакторинга (переименование локальной переменной, rustfmt). Поведение при этом
   закрыто unit-тестом в `main.rs`, так что дефекта нет — но это хрупкая проверка, а не контракт.
4. **`endpoint` и `body` из `notifications.js:1606-1613` не объявлены в сигнатуре и молча
   игнорируются** (Tauri допускает лишние ключи). Поведение корректно: путь и тело PATCH заново
   строит `notifications.rs`, для DM `guild_id="@me"` даёт `/users/@me/guilds/@me/settings`, как и
   `endpoint` в JS; но мёртвые ключи в UI стоит убрать. Это наследие INT-06, вне scope task-109.

P0: 0. P1: 0. P2: 0. Функциональных дефектов в закрытии трёх замечаний не найдено; PR можно
сливать в `unstable` после обычного `gate:worker` на integration-лейне.
