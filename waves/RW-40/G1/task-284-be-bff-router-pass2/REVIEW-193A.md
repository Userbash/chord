# REVIEW-193A — TR (`task-284` / PR-164, BFF router Pass 2)

**Вердикт: APPROVED** — `P0=0`, `P1=0`, мутации `5/5`.
Ветка: `bugfix/task-284-be-bff-router-pass2`, HEAD `886ec1e`, `git commit` не выполнялся.
Объём ревью: только `rust/gui-spike/src/bff_domains_router.rs` и `rust/gui-spike/tests/bff_domains_router_contract.rs`.

## 1. Контракт файлов

| Файл | md5 (факт) | md5 (ТЗ) | Совпадение |
|---|---|---|---|
| `rust/gui-spike/src/bff_domains_router.rs` | `c2e178bd8db5a104d5b8b2978e6b23ed` | `c2e178bd8db5a104d5b8b2978e6b23ed` | да |
| `rust/gui-spike/tests/bff_domains_router_contract.rs` | `d851e83f8786ac4ff345219e4b654c19` | `d851e83f8786ac4ff345219e4b654c19` | да |

`rust/gui-spike/Cargo.lock` после прогонов откатан (`git checkout --`), в `git status` его нет.
Предсуществующий дрейф дерева (` D node_modules`, ` M package-lock.json`) к карточке не относится
и в PR-164 попадать не должен.

## 2. Baseline

```
$ CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; exit_code=0

$ ... cargo test --manifest-path rust/gui-spike/Cargo.toml --lib
test result: ok. 17 passed; 0 failed; exit_code=0

$ ... cargo clippy --manifest-path rust/gui-spike/Cargo.toml \
    --test bff_domains_router_contract -- -D warnings
Finished `dev` profile ... in 0.72s   (0 warnings)

$ rustfmt --edition 2021 --check <оба файла>   → exit_code=0
```

Контрактный файл прогнан дважды (до и после блока мутаций) — `10 passed` оба раза, флаки нет.
`10 = 8` прежних тестов `+ 2` новых, ни один прежний тест не удалён.

## 3. Суть правки

`resolve_bff_domain_action(BffDomainRequest) -> Result<BffResolvedAction, String>` — единственная
точка сборки `method + route + body`: метод берётся из варианта домена (`PATCH` только для
`UpdateGuildOnboarding`, `POST` для остальных трёх), а маршрут и тело делегируются существующему
`dispatch_bff_domain_action`, поэтому две функции структурно не могут разойтись — дублирующей
таблицы маршрутов нет. Публичный контракт `dispatch_bff_domain_action`, `SUPPORTED_BFF_DOMAINS`,
`is_supported_bff_domain`, `BffDomainAction` не изменён (обратная совместимость сохранена).

Тесты (`numstat` 115 добавлений / 2 удаления — удалены только две строки `use`-блока, заменённые
расширенным импортом): добавлен `resolve_bff_domain_action_returns_http_method_route_and_body`
(4 домена × метод/маршрут/тело + `route`/`body` сверяются с `dispatch_bff_domain_action` + негативный
срез `channel_id="0"`) и `bff_domain_request_wire_format_matches_tagged_domain_action` (буквальный
JSON внешней теггировки + отказ плоскому конверту `{domain, action}`).

## 4. Сверка с оракулом `/home/sanya/re-deep/rest-static.md` (read-only)

| Домен | Метод/маршрут в тесте | Оракул |
|---|---|---|
| `slash_commands` | `POST /api/v9/interactions` | `rest-static.md:453` — `POST /interactions` (`Endpoints.INTERACTIONS`) |
| `onboarding` | `PATCH /api/v9/guilds/{guild_id}/welcome-screen` | `rest-static.md:448` — `GET\|PATCH /guilds/{arg}/welcome-screen` |
| `soundboard` | `POST /api/v9/channels/{channel_id}/send-soundboard-sound` | `rest-static.md:244` — `POST /channels/{arg}/send-soundboard-sound` |
| `stage_instances` | `POST /api/v9/stage-instances` | `rest-static.md:524` — `POST /stage-instances` (`STAGE_INSTANCE` на `:525` — `DELETE\|PATCH /stage-instances/{arg}`, это другой маршрут) |

`onboarding` — единственный `PATCH` среди четырёх; тест различает `PATCH` коллекции
`stage-instances` и `PATCH /stage-instances/{arg}` (мутация M2 убита, см. §5).

## 5. Мутации (`5/5` убиты)

Каждая мутация применялась к `src/bff_domains_router.rs` по одному вхождению (`assert count == 1`),
затем прогонялся `--test bff_domains_router_contract`, затем файл восстанавливался записью байтов
из копии + `touch` (не `cp -p`; `mtime` обновился с до-мутационного на `2026-10-31 11:47:43 +0500`),
md5 до/после сверен.

| # | Мутация | Результат | Убивший тест |
|---|---|---|---|
| M1 | `PATCH` → `POST` в ветке `UpdateGuildOnboarding` | KILLED (`9 passed; 1 failed`) | `resolve_bff_domain_action_returns_http_method_route_and_body` |
| M2 | `CreateStageInstance` вынесен в арм `PATCH` | KILLED (`9 passed; 1 failed`) | то же |
| M3 | `SendSoundboardSound` перенесён в арм `PATCH` | KILLED (`9 passed; 1 failed`) | то же |
| M4 | `body` в `BffResolvedAction` подменён на `Value::Null` | KILLED (`9 passed; 1 failed`) | то же |
| M5 | `#[serde(rename_all = "snake_case")]` на `BffDomainAction` | KILLED (`9 passed; 1 failed`) | `bff_domain_request_wire_format_matches_tagged_domain_action` |

Итог: `TOTAL_KILLED=5/5`, выживших нет. После восстановления: md5 `c2e178bd8db5a104d5b8b2978e6b23ed`,
повторный прогон — `10 passed`, `rustfmt` — `0`, `Cargo.lock` откатан.

## 6. Замечания

- `P0`: нет.
- `P1`: нет.
- `P2` (не блокирует, к следующей карточке): у `resolve_bff_domain_action` пока нет продуктового
  вызывающего — API доступен, но в Tauri-командах BFF не подключён; это ожидаемо для Pass 2
  (`REVIEW-182A` P3-1 закрыт на уровне модуля), но без отдельной карточки на wiring признак
  «роутер готов к отправке» остаётся неиспользованным в рантайме.
- `P3`: `BffResolvedAction` без `Deserialize` (следствие `method: &'static str`) — осознанно и
  задокументировано; при необходимости принимать план запроса из IPC тип поля придётся менять.
- `P3`: `BffDomainAction` остаётся без `#[non_exhaustive]` (P3-4 из `REVIEW-182A`) — вне scope Pass 2,
  риск прежний: пятый домен станет ломающим изменением для внешних `match`.

## 7. Артефакты

- Скрипт мутаций с восстановлением и md5-проверкой: `/tmp/opencode/mut193a.py`.
- Лог мутаций: `/tmp/opencode/mut193a.log` (md5 `bb67eed5ce48fbfc00a607dc5a5af6ab`).
- `git commit`/`push` не выполнялись, чужие файлы не изменялись.
