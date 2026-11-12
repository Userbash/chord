# REVIEW — PR-153 / task-269 (`BE-INT-12`: `bff_domains_router.rs`)

- **Карточка:** `task-269` (`BE-INT-12`) — единый типизированный роутер команд BFF для доменов `slash_commands`, `onboarding`, `soundboard`, `stage_instances`.
- **PR:** `PR-153`, ветка `feature/task-269-be-bff-domains-router`, worktree `/home/user/orca/workspaces/project/feature-task-269-be-bff-domains-router`.
- **База:** `unstable` `b35c3e1` (`feat(ui): merge PR-146 (task-261 slash-command-picker)`), рабочее дерево без коммита (`git commit` не выполнялся).
- **Ревьюер:** DeepSeek (`TR-182A`), 31.10.2026, `TZ=Asia/Shanghai` 14:20–14:30 (суббота — окно DeepSeek разрешено).
- **Вердикт:** `APPROVED`.
- **Счётчики:** `P0=0`, `P1=0`, `P2=0`, `P3=5`; мутации `5/5` убиты; targeted `8/8 PASS`; `clippy -D warnings` — `0` предупреждений; `rustfmt --check` — чисто.

## 1. Границы scope и состояние дерева

`git status --short` на старте ревью:

```text
 D node_modules
 M package-lock.json
 M rust/gui-spike/src/lib.rs
?? rust/gui-spike/src/bff_domains_router.rs
?? rust/gui-spike/tests/bff_domains_router_contract.rs
```

Scope задачи — ровно три файла PR; всё остальное к PR не относится:

| Файл | MD5 | Совпал с ACCEPT/ТЗ |
|---|---|---|
| `rust/gui-spike/src/bff_domains_router.rs` | `b59afec01f123e50b04e685276ae003d` | да |
| `rust/gui-spike/src/lib.rs` | `93d19a31d69ded1e448f330e79db6fd2` | да |
| `rust/gui-spike/tests/bff_domains_router_contract.rs` | `cf7dcc67c12e2e047a182814823cecd4` | да |

`git diff rust/gui-spike/src/lib.rs` — одна строка `pub mod bff_domains_router;` в конце списка модулей. Ничего лишнего.

`rust/Cargo.lock` не изменён ни на старте, ни после прогонов.

Предсуществующий дрейф рабочего дерева (не от PR-153, JS-файлы PR не трогает):

- `package-lock.json` — в `HEAD` `package.json` уже содержит `bin` и `engines`, а закоммиченный lock их не отражает; дерево грязное до начала задачи.
- `node_modules` — в индексе симлинк (`120000`), в worktree отсутствует.
- `rust/gui-spike/Cargo.lock` — закоммиченный lock устарел относительно `rust/crates/discord-codec` (`ruzstd` → `zstd`). Любой `cargo test` без `--offline` перегенерирует его (`+41/−16`). После всех прогонов откатан: `git checkout -- rust/gui-spike/Cargo.lock`; финальный `git status` lock не показывает.

Итог по scope: посторонних правок, внесённых PR, нет. Оба дрейфа (npm и lock) существовали до задачи и должны уходить отдельным chore, а не в этот PR.

## 2. Прогоны

Команда из `TASK-TR-182A.md` §2 неприменима: `--manifest-path rust/Cargo.toml -p discord-gui-spike` даёт `error: package ID specification 'discord-gui-spike' did not match any packages`, потому что `rust/gui-spike` — отдельный workspace (`[workspace]` в его `Cargo.toml`). Прогоны выполнены корректной формой с тем же `CARGO_TARGET_DIR`:

```sh
export CARGO_TARGET_DIR=/home/user/orca/project/rust/target
cargo test   --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract
cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract --lib -- -D warnings
rustfmt --edition 2021 --check rust/gui-spike/src/bff_domains_router.rs rust/gui-spike/tests/bff_domains_router_contract.rs
```

Результаты:

- `bff_domains_router_contract`: `8 passed; 0 failed; 0 ignored`, `exit=0`.
- `clippy`: `Finished dev profile`, `0` предупреждений, `exit=0`.
- `rustfmt --check`: `exit=0`.
- Контроль регрессии по крейту: `cargo test --manifest-path rust/gui-spike/Cargo.toml --lib` — `17 passed; 0 failed`.

## 3. Сверка маршрутов с оракулом RE

Маршруты и HTTP-методы в модуле сверены с эталонным дампом `reference-oracle/rest-static.md` (read-only), не с кодом реализации:

| Домен | Маршрут в коде | Оракул |
|---|---|---|
| `slash_commands` | `POST /api/v9/interactions` | `rest-static.md:751` — `POST /interactions` (`web.js:Endpoints.INTERACTIONS`) |
| `onboarding` | `PATCH /api/v9/guilds/{guild_id}/welcome-screen` | `rest-static.md:448` — `GET\|PATCH /guilds/{arg}/welcome-screen` (`web.js:Endpoints.GUILD_WELCOME_SCREEN`) |
| `soundboard` | `POST /api/v9/channels/{channel_id}/send-soundboard-sound` | `rest-static.md:244` — `POST /channels/{arg}/send-soundboard-sound` (`web.js:Endpoints.SEND_SOUNDBOARD_SOUND`) |
| `stage_instances` | `POST /api/v9/stage-instances` | `rest-static.md:525` — `POST /stage-instances` (`web.js:Endpoints.STAGE_INSTANCES`) |

Тела запросов и валидация не дублируются в роутере: он делегирует профильным функциям `slash_commands::build_interactions_route_and_body`, `onboarding::validate_welcome_screen_request` + `build_guild_welcome_screen_route`, `soundboard::build_send_soundboard_route_and_body`, `stage_instances::build_create_stage_instance_route_and_body`. Сигнатуры и возвращаемый тип `Result<(String, Value), String>` совпадают с профильными модулями.

Тесты сверяют контракт с независимыми литералами (точные строки маршрутов, точные JSON-тела, `SUPPORTED_BFF_DOMAINS` целиком), а не с выводом реализации. Это правильный оракул.

## 4. Мутации — 5/5 убиты

Каждая мутация применялась к чистому файлу, прогонялся `bff_domains_router_contract`, затем файл восстанавливался из бэкапа через `cp` без `-p` (mtime обновлялся, крейт пересобирался — иначе `cargo` мог отдать кэш). Логи — `artifacts-182a/mutation-*.log`.

| # | Мутация в `src/bff_domains_router.rs` | MD5 мутанта | Тест-убийца | Паника |
|---|---|---|---|---|
| M1 | из `SUPPORTED_BFF_DOMAINS` удалён `"stage_instances"` | `e4892dde290ffa63ee3d02e11161a807` | `supported_bff_domains_constant_matches_exact_contract` | `left: ["slash_commands", "onboarding", "soundboard"]` vs `right: [.., "stage_instances"]` |
| M2 | `is_supported_bff_domain` всегда возвращает `true` | `0e35eaac1bc178c398c333ad0259f4bb` | `is_supported_bff_domain_accepts_only_registered_domains` | `domain "" must not be supported` |
| M3 | в ветке `UpdateGuildOnboarding` убран вызов `validate_welcome_screen_request` | `899b388ed81947da4c4a6da2ac16bede` | `dispatch_bff_domain_action_propagates_validation_errors_across_all_domains` | `UpdateGuildOnboarding with blank description must fail` |
| M4 | ветка `UpdateGuildOnboarding` строит `onboarding`-маршрут вместо `welcome-screen` | `1cd498263f410ac53b8d109862ef90d6` | `dispatch_update_guild_onboarding_builds_welcome_screen_route_and_body` | `left: ".../onboarding"` vs `right: ".../welcome-screen"` |
| M5 | ветка `SendSoundboardSound` передаёт `request.sound_id` как `channel_id` | `35a73bddc44d0d75493f87cb51e9e95e` | `dispatch_send_soundboard_sound_builds_channel_route_and_body` + `..._propagates_validation_errors_...` | `left: "/api/v9/channels/7074537851418980378/send-soundboard-sound"` |

Итог: `5/5` (`100%`). Ни одна мутация не выжила; тесты ловят и контрактные константы, и валидацию, и маршруты, и подстановку аргументов.

## 5. Восстановление файла

```text
restored md5=b59afec01f123e50b04e685276ae003d
```

Совпадает с MD5 из `TASK-TR-182A.md` и `ACCEPT.md`. После восстановления повторный `cargo test ... --test bff_domains_router_contract` — `8 passed; 0 failed`, `clippy --lib --test -- -D warnings` — `0`, `rustfmt --check` — `0`. `git checkout -- rust/Cargo.lock` не потребовался: файл не менялся; откатан только `rust/gui-spike/Cargo.lock` (см. §1).

## 6. Замечания

**P0:** нет. **P1:** нет. **P2:** нет.

**P3 (не блокируют, предложения в бэклог):**

1. **HTTP-метод не входит в результат роутера.** `dispatch_bff_domain_action` возвращает `(route, body)`, но домены разные по глаголу (3×`POST`, 1×`PATCH`): вызывающая сторона обязана знать метод из документации. Паттерн совпадает с профильными модулями, поэтому это не дефект PR, но единый роутер — естественное место вернуть `(Method, String, Value)` или маленькую структуру `BffDomainRequestPlan`.
2. **Wire-формат serde не закреплён тестом.** `bff_domain_action_serde_roundtrip_all_variants` проверяет симметрию `serialize → deserialize`, но не фиксирует внешний вид JSON. Добавление `#[serde(rename_all = ...)]` или `#[serde(tag = ...)]` пройдёт round-trip и молча изменит контракт BFF/IPC. Стоит добавить одно литеральное сравнение сериализованного значения.
3. **`SUPPORTED_BFF_DOMAINS` и `is_supported_bff_domain` не используются в крейте** — только в тестах. Для публичного API это нормально (нужны UI/IPC-валидации имён), но пока мёртвый код; при появлении строкового диспетчера стоит связать их с `dispatch`.
4. **`BffDomainAction` без `#[non_exhaustive]`.** Пятый домен станет ломающим изменением для внешних `match`. Пока потребителей нет — риск нулевой, отметить на будущее.
5. **Процессные замечания:** команда из `TASK-TR-182A.md` §2 нерабочая (см. §2) — поправить шаблон ТЗ на `--manifest-path rust/gui-spike/Cargo.toml`; закоммиченный `rust/gui-spike/Cargo.lock` устарел (`discord-codec`: `ruzstd` vs `zstd`) — обновить отдельной chore-задачей, иначе каждый прогон `cargo` пачкает дерево.

Прочее, проверенное и признанное корректным: паник и `unwrap`/`expect` в продуктовом коде нет (`serde_json::to_value` обработан через `map_err`), небезопасного кода нет, сообщения об ошибках на русском и с именем поля, doc-комментарии описывают реальные маршруты, экспорт в `lib.rs` минимальный, покрытие — 4 домена × (успех + ошибки + границы `"1"`/20 девяток/`None`), негативные кейсы `is_supported_bff_domain` включают регистр, пробелы, дефисы и пустую строку.

## 7. Артефакты

| Файл | MD5 |
|---|---|
| `artifacts-182a/mutation-m1_drop_stage_domain.log` | `e08bd9a6e9f1cd7b51cfa3eb1d42c005` |
| `artifacts-182a/mutation-m2_supported_always_true.log` | `483a9e8ba3b4648eb4e2fb35d6e735cf` |
| `artifacts-182a/mutation-m3_skip_onboarding_validation.log` | `ddd62674ab31820fa2a896954a8ba3b6` |
| `artifacts-182a/mutation-m4_wrong_onboarding_route.log` | `52f8f365e2fb1d09b33822656e9f5d72` |
| `artifacts-182a/mutation-m5_soundboard_channel_from_sound_id.log` | `1436ec5aa01ccf283a61d27993c9c302` |

## 8. Решение

`APPROVED`: `P0=0`, `P1=0`, мутации `5/5`, целевые тесты и линтеры зелёные, scope чистый, файл после мутаций восстановлен до `b59afec01f123e50b04e685276ae003d`. Блокирующих замечаний для merge в `unstable` нет; P3-замечания §6 можно оформить отдельными карточками. `git commit`/`push` не выполнялись.
