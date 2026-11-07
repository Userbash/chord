# REVIEW-200A — Техническое ревью PR-173 (`task-293` / `BE-INT-16`: `#[tauri::command] fn invoke_bff_domain` в `rust/gui-spike/src/main.rs`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-200A
- **Проверяемая ветка:** `feature/task-293-be-bff-ipc-wiring`, HEAD `648ba83` = `unstable` (коммитов в PR нет, ревью — по состоянию рабочего дерева)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/feature-task-293-be-bff-ipc-wiring`
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, `wiring_tests` `13/13` PASS, мета-тест `ipc-path` `12/12` PASS
- **Объект:** `rust/gui-spike/src/main.rs` — `+66 / −1` строк (команда, регистрация, тест)

## 1. Целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до ревью | MD5 после тестов и мутаций |
| --- | --- | --- | --- |
| `rust/gui-spike/src/main.rs` | `91d37d09759e87f2cd6e73ca1b55f8aa` | совпал ✅ | `91d37d09759e87f2cd6e73ca1b55f8aa` ✅ |
| `rust/gui-spike/Cargo.lock` | не изменялся | `7d66c56…` = HEAD | восстановлен к HEAD после прогонов ✅ |

`git diff --numstat unstable -- rust/gui-spike/src/main.rs` → `66 1`. Удалена ровно одна строка (`settings_save` без запятой в `generate_handler!`), ни одна существующая функция и ни один существующий тест не переписаны: вставки локализованы после `settings_save` (стр. 1165–1175), в хвосте `generate_handler!` (стр. 1264) и в конце `mod wiring_tests` (стр. 2191–2241).

`git status --porcelain`:

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/src/main.rs
```

`D node_modules` и `M package-lock.json` — шум дерева, к PR не относятся: симлинк `node_modules` (`120000 4b95952c…`) в рабочей копии заменён каталогом, а lock отличается блоками `bin`/`engines` пакета `discord-lab`, которые уже есть в `HEAD:package.json`. Mtime подтверждает: `package.json` `12:14:20` → `package-lock.json` `12:14:21` (единый `npm install`), `main.rs` — `12:16:11`. См. `P2-1`.

## 2. Что именно проверялось в диффе

```rust
#[tauri::command]
fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action)
}
```

| Проверка | Результат |
| --- | --- |
| Команда зарегистрирована в `tauri::generate_handler![…]` последней, после `settings_save` | ✅ стр. 1264 |
| Аргумент назван `action`, путь/`PathBuf`/`File` в IPC не попал (PLAT-11) | ✅ подтверждено мета-тестом |
| Делегирование ровно в `resolve_bff_domain_action` без своей логики, без токена и без сети | ✅ тело — один вызов; токен в сигнатуре отсутствует |
| Команда синхронная, не `async`: нет `.await`, нет `blocking()` | ✅ тело — валидация + `serde_json::to_value`, спавн не нужен |
| Тип ошибки `String` совместим с сигнатурой роутера | ✅ `resolve_bff_domain_action -> Result<BffResolvedAction, String>`, потерь кодов нет |
| Отличие от литерала `TASK.md`: там `resolve_bff_domain_action(&action)` | ✅ вызов по значению обязателен — в базе `648ba83` параметр принимается по значению (`bff_domains_router.rs:108`), `&action` не собрался бы |

Оракул маршрута и тела сверен с RE-дампом:

| Артефакт теста | Оракул `/home/sanya/re-deep/` | Совпадение |
| --- | --- | --- |
| `PATCH /api/v9/guilds/{guild_id}/welcome-screen` | `rest-static.md:448` — `GET\|PATCH /guilds/{arg}/welcome-screen` | ✅ |
| тело `{description, welcome_channels, enabled}` | `rest-static.json:1550`, `out/callsites.json:4683` — `{description,welcome_channels,enabled}`; `callsites.json:4691` — `patch({url: GUILD_WELCOME_SCREEN(e), body:{description, welcome_channels, enabled}})` | ✅ |

Wire-формат самого `BffDomainAction` (externally tagged) запинен буквальным JSON в `tests/bff_domains_router_contract.rs:474–512`, поэтому в `main.rs` дублировать его не требуется.

## 3. Тесты (мои прогоны)

Все прогоны с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target` и `--offline`.

| Проверка | Результат |
| --- | --- |
| `cargo test --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike` | exit `0`: `13 passed; 0 failed`, включая `wiring_tests::invoke_handler_registers_invoke_bff_domain_for_seven_domains ... ok` |
| `node --test tests/meta/ipc-path.test.mjs` | exit `0`: `tests 12 / pass 12 / fail 0` (3 suites), в т.ч. «найденные команды совпадают с зарегистрированными в generate_handler!» — новая команда попадает и в скан `#[tauri::command]`, и в список регистрации |
| `cargo test --test bff_domains_router_contract` (дополнительно, зона делегирования) | exit `0`: `10 passed; 0 failed` |
| `rustfmt --check --edition 2021 rust/gui-spike/src/main.rs` | exit `0`, чисто |
| `cargo clippy --bin discord-gui-spike --all-targets` | 1 warning — `clippy::type_complexity` на `src/main.rs:1718` (тестовый хелпер вложений), в диффе этой строки нет; предупреждение базы |
| Повторный green после отката мутаций | `13 passed; 0 failed` + `12/12` — дерево вернулось в рабочее состояние |

Red-фаза перепроверена по `red.log`: падение на `E0425: cannot find function invoke_bff_domain` в двух местах теста (`2196`, `2216`), то есть тест писался до реализации, и падал ровно на отсутствии функции, а не на посторонних ошибках.

Что реально проверяет новый тест (и это не самоповтор реализации):

1. `lists_command(handler_block(head_source()), "invoke_bff_domain")` — регистрация читается из головы файла до `mod wiring_tests`, поэтому литерал `"invoke_bff_domain"` внутри самого теста ложного «нашлось» не даёт;
2. прямой вызов команды на валидном Snowflake `123456789012345678` даёт литерально ожидаемый `BffResolvedAction` (`method`, `route`, `body`) — значения выведены из RE-дампа (§2), а не из констант роутера;
3. `guild_id: "0"` → `Err`, проверено `is_err()`, то есть ошибка не превращается в панику и не глотается.

## 4. Мутации — `5/5` убиты

Методика: бэкап `main.rs` в `/tmp/opencode/main.rs.task293.orig`, `python3`-замена с контролем «шаблон встречается ровно один раз» (`mutations/mutate-200a.py`), прогон `cargo test --offline --bin discord-gui-spike`, откат `cp` из бэкапа, сверка MD5. Логи — `logs/review-200a-mutation-M1.log … M5.log`.

| # | Что сломано | Мутация | Итог | Убивший тест и сообщение |
| --- | --- | --- | --- | --- |
| M1 | регистрация в IPC | `invoke_bff_domain` удалён из `generate_handler![…]` | убит | `invoke_handler_registers_invoke_bff_domain_for_seven_domains` — «invoke_bff_domain не зарегистрирован в invoke_handler!» (`12 passed; 1 failed`) |
| M2 | маршрут | после делегирования `route.replace("welcome-screen", "onboarding")` | убит | тот же тест — `left: …/onboarding`, `right: …/welcome-screen` |
| M3 | проброс ошибки | `Err(_) => Ok(BffResolvedAction { method: "PATCH", route: "", body: Null })` | убит | тот же тест — «guild_id="0" должен давать Err: Ok(…)» |
| M4 | использование входа | вход отброшен (`let _ = action;`), ответ захардкожен под позитивный кейс | убит | тот же тест — негативный кейс вернул `Ok` вместо `Err` |
| M5 | целостность тела | из `body` удалено поле `description` | убит | тот же тест — `left` без `description`, `right` с `"description": Null` |

**Итого `5/5` (100%).** Во всех пяти прогонах падает ровно целевой тест, остальные 12 остаются зелёными — то есть тест точечный, а не «краснеет от всего». После каждой мутации и в конце цикла MD5 файла — `91d37d09759e87f2cd6e73ca1b55f8aa`, файл идентичен исходному.

## 5. Замечания без блокировки

### P2-1. `package-lock.json` и `node_modules` в рабочем дереве
К PR не относятся (доказательства в §1), но при оформлении PR нельзя делать `git add -A`: в коммит должен попасть только `rust/gui-spike/src/main.rs`. Дополнительно: HEAD ветки равен `unstable 648ba83`, то есть PR-173 ещё не содержит коммита — дифф я принимал по рабочему дереву, и при коммите это нужно повторить (проверить `git show --stat` перед пушем). Лок, который перезаписывает каждый `cargo test` из-за дрейфа базы (`discord-codec` ждёт `zstd`, lock держит `ruzstd`), я откатил к `HEAD`; это отдельная карточка на базу, иначе каждый воркер будет ловить `M Cargo.lock` после любого прогона.

### P2-2. IPC-граница теряет структуру ошибки домена
Команда возвращает `Result<_, String>`, поэтому фронт получит голую строку, а не структурированный код (`SettingsError`/`AttachmentError` в этом же файле сериализуются с кодом). Для валидационных ошибок роутера (`BffDomainAction`) это терпимо, пока UI не различает причины отказа. Когда появится обработка ошибок в webview — нужна карточка в `bff_domains_router` (`BffDomainError` со `code`), менять её здесь нельзя (разрешён один файл). Не блокирует: контракт `String` уже принят в `task-269`/`task-284`, эта карточка лишь выносит его на IPC.

### P3
- **P3-1. Имя теста обещает больше контракта.** `..._for_seven_domains`, фактически 4 домена (`SUPPORTED_BFF_DOMAINS`), проверяется один. Отклонение согласовано координатором и зафиксировано в doc-комментарии теста — принимаю, но при расширении роутера имя станет правдой только вместе с новыми assert.
- **P3-2. Полностью квалифицированные пути в сигнатуре** вместо `use` в шапке файла (все остальные команды используют импортированные типы). Читаемость страдает, поведение — нет; правка косметическая, трогать её в этом PR не нужно.
- **P3-3. Через обёртку проверен только `UpdateGuildOnboarding`.** Обёртка — чистый делегат, поэтому покрытия остальных трёх доменов в `main.rs` не требуется; но если в обёртке появится ветвление (например, домен-специфичный учёт), один assert на второй домен станет обязательным.
- **P3-4. `body: Value` не выражает «нет тела».** Для будущих GET-доменов (`automod`/`expressions`/`webhooks` из `TASK.md`) понадобится `Option<Value>` — это карточка в `bff_domains_router`, воркер её уже отметил в `REPORT.md`.
- **P3-5. `REPORT.md` не упоминает clippy-warning базы** (`type_complexity` на `1718`). Упомянут, но стоит держать в отчётах явную строку «warning базы, не мой» — сейчас она есть, претензий нет; замечание на будущее.

## 6. Соответствие DoD карточки

| Критерий (`TASK-TR-200A.md`) | Статус |
| --- | --- |
| Изменён ровно 1 файл (`rust/gui-spike/src/main.rs`) | ✅ по `rust/`; шум `package-lock.json`/`node_modules` — до начала работы (`P2-1`) |
| `cargo test --bin discord-gui-spike` | ✅ `13 passed; 0 failed` |
| `node --test tests/meta/ipc-path.test.mjs` | ✅ `12/12` |
| Мутации `5/5` с откатом | ✅ `5/5`, MD5 до/после совпал |
| `P0 = 0`, `P1 = 0` | ✅ |

**Вердикт: APPROVED.** Блокирующих замечаний нет; `P2-1` (гигиена коммита) и `P2-2` (структура ошибки на IPC) — к сведению интегратора и на отдельные карточки, мерж не блокируют.
