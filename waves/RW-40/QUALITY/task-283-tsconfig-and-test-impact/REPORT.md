# Отчёт — task-283 (`G4 QA L1 / P1`, `QA-FIX-04`)

- **Ветка:** `qa/task-283-tsconfig-and-test-impact`, база `unstable` @ `886ec1e`
- **Разрешённые файлы:** `rust/gui-spike/ui/tsconfig.json`, `scripts/test-impact.mjs`, этот отчёт
- **Коммитов нет** (`git commit` не выполнялся)

## Что сделано

1. `rust/gui-spike/ui/tsconfig.json`: `include` заменён на `["app.js", "features/**/*.js"]`
   вместо трёх перечисленных вручную модулей. Теперь под `checkJs`/`strict` попадают все
   `ui/features/*.js`: `--listFilesOnly` показывает **97** файлов `ui/features/` (до правки
   проверялось 41 — остальные невидимы, если их никто не импортирует из точки входа).
2. `scripts/test-impact.mjs`:
   - в `UI_MODULE_TESTS` добавлены 4 модуля (`shell-controllers-registry.js`,
     `chat-helpers-bridge.js`, `composer-popovers-bridge.js`, `app-shell-wiring.js`) с
     профильным тестом первым и общими UI-контрактами по конвенции таблицы;
   - в `RUST_MODULE_CONTRACTS` добавлен `bff_domains_router.rs → bff_domains_router_contract`
     и по согласованию с координатором — оставшиеся 17 `pub mod` из `lib.rs` (`automod`,
     `dm_channels`, `expressions`, `forum`, `ipc_ext`, `moderation`, `onboarding`, `polls`,
     `profile`, `profile_ext`, `relationships`, `roles`, `scheduled_events`, `slash_commands`,
     `soundboard`, `stage_instances`, `webhooks`). У всех 17 контрактные файлы
     `rust/gui-spike/tests/<mod>_contract.rs` уже лежали на диске; без строки в таблице они
     шли полным прогоном крейта и валили инвариант `tests/unit/test-impact.test.mjs:494`.

## Red (до правки)

- `logs/red-test-impact.txt`: `node scripts/test-impact.mjs --changed <4 UI + bff_domains_router>`
  выдавал полный `frontend()`-блок с `geometry-check.mjs` для UI-модулей и
  `cargo test ... rust/gui-spike/Cargo.toml` без `--test` для `bff_domains_router.rs` —
  ни одного профильного теста, ни контрактного таргета.
- `node --test tests/unit/test-impact.test.mjs` на **немодифицированном** селекторе: 52/53,
  красный инвариант `каждый pub mod из lib.rs покрыт правилом` (первый в списке — `forum`).

## Green (после правки)

| Проверка | Команда | Результат |
| --- | --- | --- |
| UI tsc | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | exit `0`, 97 файлов `ui/features/` (`logs/green-tsc-ui.txt`) |
| JS tsc | `npx tsc -p tsconfig.agent.json --noEmit` | exit `0` |
| Контракт селектора | `node --test tests/unit/test-impact.test.mjs` | 53/53 pass, 0 fail (`logs/green-test-impact-contract.txt`) |
| Профильные тесты 4 UI-модулей | `node --test tests/unit/{shell-controllers-registry,chat-helpers-bridge,composer-popovers-bridge,app-shell-wiring}.test.mjs` | 53/53 pass |
| План селектора | `node scripts/test-impact.mjs --changed ...` | профильные тесты + `--test bff_domains_router_contract --test ipc_contract` (`logs/green-test-impact.txt`) |
| Контракты BFF | `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract --test ipc_contract` | 8 + 20 passed, 0 failed (`logs/green-cargo-contracts.txt`) |

## `gate:worker` — вердикт `failed`, причины вне карточки

Артефакт: `.gate/worker-2026-10-31T06-45-36-464Z.json`.

- `ui-tsc` — **ok**.
- `typecheck`, `meta`, `unit`, `spike-rust`, `workspace-rust`, `smoke` — красные. Проверено
  `git stash` моих двух файлов и повторным прогоном: те же ошибки есть на чистом HEAD.
  - `typecheck`: 49 строк TS-ошибок в `tests/unit/{locale-switcher,markdown-helper,poll-progress,voice-input-mode}.test.mjs`.
  - `meta` 96/102: `plan-sync §8` (тестовые файлы вне §8), `live-keyring` 12 `#[ignore]`-кейсы.
  - `unit` 1930/1931: `dispatch_ui.test.mjs` — `CHANNEL_CREATE` форвардится бэкендом, но теряется UI.
  - `spike-rust` / `workspace-rust`: не компилируются (`voice_stage_contract` импортирует
    отсутствующий `discord_gui_spike::voice_stage`; в workspace — `DispatchEvent::parse_stream`).
  - `smoke`: нет бинарника `rust/gui-spike/target/debug/discord-gui-spike` в рабочем дереве
    (в основном репозитории он есть); на живом дисплее тест требует `cargo build`.

## Файлы и md5

```
d36fa8dd3043b17cd5b965de19e2ba28  rust/gui-spike/ui/tsconfig.json
330caeeac8e6f553df0ecfd25eea84cc  scripts/test-impact.mjs
```

## Риски и замечания

- Побочный эффект окружения: любой `cargo` в этом дереве перезаписывает
  `rust/gui-spike/Cargo.lock` (`ruzstd → zstd`, `jobserver`/`libc` в `cc`) — лок не совпадает
  с манифестом. Правку откатывал (`git checkout -- rust/gui-spike/Cargo.lock`), в диффе её нет.
- `node_modules` в дереве удалён, `package-lock.json` изменён — это состояние базы, не мои правки.
- Расширение `include` до `features/**/*.js` делает `ui-tsc` строже: любая новая фича без
  JSDoc/типов теперь валит гейт. Это цель карточки, но смежным воркерам стоит знать.
- 17 дополнительных маппингов расширяют дифф за пределы исходного ТЗ (вариант A, выбор
  координатора); поведение гейта для этих модулей теперь точечное (`--test <mod>_contract`
  вместо полного прогона крейта), все таргеты проверены на диске и компилируются.
