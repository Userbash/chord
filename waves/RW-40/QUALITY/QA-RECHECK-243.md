# QA-RECHECK-243 — повторный QA-гейт после фиксов Team Lead

Дата: 2026-11-05. Роль: **G4 QA**. Задание: повторный QA-гейт после фиксов
`tokio::sync`, `core_bridge` (`flags`/`sticker_items`), экспорт `tray`.

Проверяемое дерево: `/home/sanya/orca/project` (main, dirty), `HEAD 4e24048`.
Worktree `g4-task-243-qa-recheck` стоит на том же `4e24048`, но **не содержит**
трёх фиксов: они есть только в рабочем дереве main. Поэтому все команды ниже
выполнены в main; иначе гейт проверял бы старый код.

## Вердикт

**Полный QA-гейт — FAIL: 4 команды из 5 красные.**

При этом три заявленных фикса подтверждены targeted-проверками:

| Фикс | Где | Проверка | Итог |
|---|---|---|---|
| `tokio::sync` | `rust/Cargo.toml:22`, `rust/crates/discord-core/Cargo.toml:13` | `cargo tree -p discord-core -e features` показывает `tokio feature "sync"`; `cargo test -p discord-core --lib` — 33/33 | **PASS** |
| `core_bridge`: `flags`, `sticker_items` | `rust/gui-spike/tests/core_bridge.rs:272-273` | `cargo test --test core_bridge` в `rust/gui-spike` — 6/6 | **PASS** |
| экспорт `tray` | `rust/crates/discord-platform/src/lib.rs:16` | `cargo test -p discord-platform --test tray_contract` — 10/10 | **PASS** |

Оставшиеся красные гейты вызваны не этими тремя фиксами, а незавершённой
интеграцией соседних задач в рабочем дереве. Ниже — точные блокеры.

## 1. Команды и результаты

| # | Команда | Exit | Результат |
|---|---|---|---|
| 1 | `cd rust && CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --workspace --exclude discord-voice` | **101** | Компиляция `discord-core` упала: 21 ошибка в `tests/dispatch_types.rs`; тесты не запускались |
| 2 | `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test -p discord-core --lib` | **0** | `33 passed; 0 failed` |
| 3 | `cd .. && npm run typecheck` | **1** | 159 ошибок `tsc`; `tsconfig.agent.json` не запускался из-за `&&` |
| 4 | `node --test tests/meta/*.test.mjs` | **1** | `tests 102`, `pass 96`, `fail 6` |
| 5 | `node --test tests/unit/*.test.mjs` | **1** | `tests 1794`, `pass 1790`, `fail 4` |

### 1.1. Workspace Rust — exit 101

Сборка остановилась на тестовом таргете `discord-core`:

```text
error[E0432]: unresolved imports `discord_core::dispatch::RoutedStreamDispatch`,
  `discord_core::dispatch::StreamDispatchKind`,
  `discord_core::dispatch::route_stream_dispatch`,
  `discord_core::dispatch::route_stream_dispatch_with_diagnostics`
  --> crates/discord-core/tests/dispatch_types.rs:20:19

error[E0599]: no method named `to_stream_action` found for struct `DispatchEvent`
  --> crates/discord-core/tests/dispatch_types.rs:415:28
...
error: could not compile `discord-core` (test "dispatch_types") due to 21 previous errors
```

Причина: в рабочем `rust/crates/discord-core/src/dispatch.rs` (1736 строк,
md5 `e33cc7d7821be717bfa6d3a5006462e2`) нет stream-API, которое ожидает
untracked-тест `tests/dispatch_types.rs` (661 строка, md5
`af859f628901986aa523b01ea1c507b1`). В коммите `8e5500c` (task-299, PR-178)
тот же файл весит 1889 строк и содержит `StreamDispatchKind`,
`route_stream_dispatch` и методы `DispatchEvent::*_stream*`. В main это
изменение не попало, хотя `src/stream.rs` и `pub mod stream;` уже есть.

Команда 1 поэтому не даёт информации об остальных крейтах: они не дошли до
запуска тестов. После починки `dispatch.rs` гейт нужно повторить целиком.

### 1.2. `discord-core --lib` — exit 0

```text
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
```

Библиотека ядра здорова; красный workspace — это именно интеграционный
тест-таргет, а не сама библиотека.

### 1.3. Typecheck — exit 1, 159 ошибок

Разбивка: **25 ошибок в продуктовом коде, 134 в тестах**.

| Файл | Ошибок | Пример |
|---|---:|---|
| `rust/gui-spike/ui/app.js` | 13 | `recipients` вместо `recipient_ids`; нет `streamSourceId`; нет `select`, `getActiveTarget`; `icon` не входит в `RailGuild` |
| `rust/gui-spike/ui/features/svg-icons.js` | 5 | типы аргументов/возврата |
| `rust/gui-spike/ui/features/invite-parser.js` | 2 | `string \| undefined` в параметре `string` |
| `rust/gui-spike/ui/features/chat.js` | 2 | `renderVideoAttachment` не найден (см. D3); possible `undefined` |
| `rust/gui-spike/ui/features/token-map.js` | 1 | тип индексации |
| `rust/gui-spike/ui/features/inbox.js` | 1 | строковый индекс в `Readonly<{...}>` |
| `rust/gui-spike/ui/features/attachment-validator.js` | 1 | possible `undefined` |
| `tests/unit/profile-modal.test.mjs` | 29 | `FakeNode` не совпадает с `Element` |
| `tests/unit/svg-icons.test.mjs` | 27 | `FakeNode` не совпадает с `Document/Element` |
| `tests/unit/appearance-settings.test.mjs` | 22 | типы настроек |
| `tests/unit/channel-tree.test.mjs` | 17 | типы DOM/модели |
| `tests/unit/locale-switcher.test.mjs` | 9 | типы DOM |
| остальные 13 файлов | 30 | по 1–6 ошибок (`inbox` 6, `embed-formatter` 6, `voice-input-mode` 3, `poll-progress` 3, `markdown-helper` 3 и др.) |

Первые строки вывода:

```text
rust/gui-spike/ui/app.js(5298,40): error TS2551: Property 'recipients' does not exist on type 'HydratedPrivateChannel'. Did you mean 'recipient_ids'?
rust/gui-spike/ui/app.js(8380,53): error TS2339: Property 'streamSourceId' does not exist ...
rust/gui-spike/ui/features/chat.js(3588,27): error TS2552: Cannot find name 'renderVideoAttachment'. Did you mean 'renderAttachments'?
tests/unit/svg-icons.test.mjs(207,36): error TS2345: Argument of type 'FakeNode' is not assignable ...
```

### 1.4. Meta — exit 1, 6 падений из 102

| # | Тест | Причина |
|---|---|---|
| 1 | `keyring-live.test.mjs:105` — «ровно 12 `#[test]` и 12 `#[ignore]`» | В `keyring_contract.rs` теперь **26** `#[test]`, а не 12 |
| 2 | `keyring-live.test.mjs:116` — «у каждого `#[test]` сразу следует `#[ignore]`» | `k13_safe_storage_prefix_and_detection` не имеет `#[ignore = "live-keyring: …"]` |
| 3 | `keyring-live.test.mjs:136` — «имена и число кейсов не изменились» | В списке появился `k25_migrator_with_live_keyring` |
| 4 | `keyring-live.test.mjs:243` — «0 passed и 12 ignored» | Реально `13 passed; 13 ignored`: новые migration-тесты `k13`–`k26` не live, часть из них проходит |
| 5 | `keyring-live.test.mjs:258` — «`--list --ignored` даёт 12 кейсов» | Возвращает 13: добавлен `k25_migrator_with_live_keyring` |
| 6 | `plan-sync.test.mjs:140` — «каждый тестовый файл описан в §8» | **59** новых `tests/unit/*.test.mjs` отсутствуют в `TDD-PLAN.md` §8 |

Падения keyring — это рассинхрон мета-оракула с новым набором тестов
safe-storage/migration, а не падение самого продукта. `PLAT-02 T3` при этом
корректно видит 13 passed / 13 ignored и падает только на жёстком ожидании
«12 ignored».

Полный список 59 файлов без строки в §8 — в приложении.

### 1.5. Unit — exit 1, 4 падения из 1794

| # | Тест | Факт | Ожидание |
|---|---|---|---|
| 1 | `dispatch_ui.test.mjs:264` — `A2: FORWARDED_KINDS` | `FORWARDED_KINDS.length === 30` | `22` (21 имя ядра + `MESSAGE_REACTION_REMOVE`) |
| 2 | `dispatch_ui.test.mjs:307` — `A5: имена вне канонического набора` | `CHANNEL_CREATE` есть в UI | В оракуле ядра его нет |
| 3 | `dispatch_ui.test.mjs:583` — `B14: неизвестный kind` | `MESSAGE_DELETE` обрабатывается, `ignored === 0` | Должен быть `ignored === 1` |
| 4 | `test-impact.test.mjs:494` — «каждый `pub mod` из `lib.rs` покрыт правилом» | `pub mod automod` без `--test automod_contract` | Каждый модуль должен иметь контрактный таргет |

В `rust/gui-spike/ui/features/dispatch-model.js` теперь 30 форвардимых
событий: добавлены `CHANNEL_CREATE/UPDATE/DELETE`, `MESSAGE_DELETE*`,
`THREAD_*`, `VOICE_CHANNEL_*` и другие расширенные типы. Тест
`dispatch_ui.test.mjs` всё ещё написан под старый набор из 22 имён. Сам
`rust/gui-spike/tests/automod_contract.rs` существует, но в таблице
`RUST_MODULE_CONTRACTS` (`scripts/test-impact.mjs`) нет строки
`rust/gui-spike/src/automod.rs → automod_contract`.

## 2. Оставшиеся дефекты

| ID | Приоритет | Гейт | Где | Что не так | Что сделать |
|---|---|---|---|---|---|
| **D1** | **P0** | Rust workspace | `rust/crates/discord-core/src/dispatch.rs`, untracked `tests/dispatch_types.rs` | Нет stream-API (`StreamDispatchKind`, `route_stream_dispatch`, `to_stream_action`, `is_stream`, `stream_kind`, `parse_stream`); workspace не компилирует тесты, 21 ошибка | Перенести stream-часть из `8e5500c` в `dispatch.rs` либо убрать/изолировать тест-таргет; повторить команду 1 |
| **D2** | **P1** | Typecheck | 7 продуктовых файлов + 18 тестовых | 159 ошибок `tsc`: 25 в продукте, 134 в тестах; `tsconfig.agent.json` даже не проверялся | Починить типы в `app.js`/`svg-icons.js`/`chat.js`; обновить `FakeNode`-типы в тестах; добиться exit 0 для обоих tsconfig |
| **D3** | **P1** | Typecheck / runtime | `rust/gui-spike/ui/features/chat.js:3588` | Вызов `renderVideoAttachment(...)` — функции с таким именем нет ни в одном файле; ветка video/GIF embed даст `ReferenceError` в рантайме | Заменить на существующий `renderVideoMediaItem` (проверить сигнатуру) или добавить функцию и тест на video-embed |
| **D4** | **P1** | Meta | `tests/meta/keyring-live.test.mjs`, `rust/crates/discord-platform/tests/keyring_contract.rs` | Мета-оракул ждёт 12 live-кейсов; в файле 26 тестов, 13 passed / 13 ignored, `k13` без ignore, `k25` лишний live | Разделить live-keyring (12) и migration/safe-storage тесты (`k13`–`k26`) в оракуле или в файле; обновить ожидания `--list --ignored` |
| **D5** | **P2** | Meta | `TDD-PLAN.md` §8 | 59 новых unit-тестов не описаны в §8 | Дописать строки §8 либо убрать тесты из дерева, если они чужие/временные |
| **D6** | **P2** | Unit | `tests/unit/dispatch_ui.test.mjs`, `rust/gui-spike/ui/features/dispatch-model.js` | UI форвардит 30 kind, тест ждёт 22; `CHANNEL_CREATE` и `MESSAGE_DELETE` трактуются как «неизвестные», хотя UI их обрабатывает | Обновить оракул теста под 30 kind (или вернуть UI к 22, если расширение не согласовано) |
| **D7** | **P2** | Unit | `tests/unit/test-impact.test.mjs`, `scripts/test-impact.mjs` | `pub mod automod` не получает `--test automod_contract`, хотя контрактный файл есть | Добавить `rust/gui-spike/src/automod.rs: "automod_contract"` в `RUST_MODULE_CONTRACTS` |

## 3. Что это значит для приёмки

1. Три фикса Team Lead можно считать закрытыми: они компилируются и проходят
   targeted-тесты (`core_bridge` 6/6, `tray_contract` 10/10, core lib 33/33).
2. Приёмку всего дерева делать нельзя: D1 блокирует любой полный Rust-гейт,
   D2–D7 держат typecheck/meta/unit красными.
3. D1, D4, D5, D6, D7 не связаны с тремя фиксами: это рассинхрон веток и
   оракулов. Их нужно развести по владельцам: stream-dispatch
   (task-299), safe-storage/migration (PLAT-02), dispatch-расширения (task-156),
   test-impact (`automod`).
4. После исправления D1 обязательно повторить команду 1 целиком: сейчас
   неизвестно, компилируются ли остальные workspace-крейты, потому что cargo
   остановился на `discord-core`.

## 4. Оговорки

- `discord-voice` исключён из workspace-прогона по заданию (нет `mockito`);
  его тесты не проверялись.
- Live Discord, сеть, установка пакетов и `git commit/push` не запускались.
- Логи прогонов лежат вне репозитория, в `/tmp/opencode/qa-recheck-243/`:
  `rust-workspace.log`, `rust-core-lib.log`, `typecheck.log`, `meta.log`,
  `unit.log`, `gui-core_bridge.log`, `platform-tray.log`.
- Worktree `g4-task-243-qa-recheck` не трогался, кроме этого отчёта; его
  `package-lock.json` был изменён до начала работы и не относится к гейту.
- md5 проверенных файлов с фиксами:
  `3725e824a0de2b2e3fa21cc0d1829bdb  rust/Cargo.toml`,
  `576ff4ddbe5f95c871cfd1e12ea2e2b2  rust/crates/discord-core/Cargo.toml`,
  `d1305c7f038d6c5fdaf2c70735f1dfde  rust/gui-spike/tests/core_bridge.rs`,
  `36ba2032e097066e6e7186662ee76fdc  rust/crates/discord-platform/src/lib.rs`.

## Приложение. 59 тестовых файлов без строки в §8

```text
tests/unit/appearance-settings.test.mjs
tests/unit/attachment-helper.test.mjs
tests/unit/attachment-validator.test.mjs
tests/unit/attachments-ui.test.mjs
tests/unit/audit-log.test.mjs
tests/unit/automod-editor.test.mjs
tests/unit/channel-gate.test.mjs
tests/unit/channel-tree.test.mjs
tests/unit/composer-autocomplete.test.mjs
tests/unit/context-menu.test.mjs
tests/unit/embed-formatter.test.mjs
tests/unit/event-banner.test.mjs
tests/unit/events-modal.test.mjs
tests/unit/expression-picker.test.mjs
tests/unit/forum-filter.test.mjs
tests/unit/forum-view.test.mjs
tests/unit/friends-home.test.mjs
tests/unit/guild-folders.test.mjs
tests/unit/guild-modals.test.mjs
tests/unit/i18n.test.mjs
tests/unit/inbox.test.mjs
tests/unit/invite-modal.test.mjs
tests/unit/invite-parser.test.mjs
tests/unit/keybinds.test.mjs
tests/unit/locale-switcher.test.mjs
tests/unit/markdown-helper.test.mjs
tests/unit/member-list.test.mjs
tests/unit/members-subscriptions.test.mjs
tests/unit/message-actions.test.mjs
tests/unit/message-media-polls.test.mjs
tests/unit/notifications.test.mjs
tests/unit/pins-threads.test.mjs
tests/unit/plural-format.test.mjs
tests/unit/poll-card.test.mjs
tests/unit/poll-progress.test.mjs
tests/unit/presence-activity.test.mjs
tests/unit/profile-modal.test.mjs
tests/unit/quick-switcher.test.mjs
tests/unit/reactions-ui.test.mjs
tests/unit/role-editor.test.mjs
tests/unit/role-permissions.test.mjs
tests/unit/search-panel.test.mjs
tests/unit/settings-modal.test.mjs
tests/unit/shell-bridge.test.mjs
tests/unit/shortcuts.test.mjs
tests/unit/slowmode.test.mjs
tests/unit/stage-channel.test.mjs
tests/unit/svg-icons.test.mjs
tests/unit/threads-drawer.test.mjs
tests/unit/token-map.test.mjs
tests/unit/tooltip.test.mjs
tests/unit/typing-indicator.test.mjs
tests/unit/unread-bar.test.mjs
tests/unit/user-popout.test.mjs
tests/unit/voice-grid.test.mjs
tests/unit/voice-input-mode.test.mjs
tests/unit/voice-quality.test.mjs
tests/unit/voice-stage-ui.test.mjs
tests/unit/voice-volume.test.mjs
```
