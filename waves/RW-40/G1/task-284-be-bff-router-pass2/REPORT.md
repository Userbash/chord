# Отчёт — task-284 (`BE-FIX-01`, Pass 2: `bff_domains_router.rs`) / RW-40 / G1

- **batch/карточка:** `RW-40` / `task-284` (`BE-FIX-01`, `P1`, `L2`), Pass 2 по `REVIEW-182A` (P3-1 и P3-2).
- **Ветка / worktree:** `bugfix/task-284-be-bff-router-pass2`, `/home/sanya/orca/workspaces/project/bugfix-task-284-be-bff-router-pass2`, база `886ec1e` (`unstable`-линия), `git commit` не выполнялся.
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`), `TZ=Asia/Shanghai` 14:43–15:05 (суббота — окно DeepSeek разрешено).

## 1. Сделано

`REVIEW-182A` дал два P3, оба закрыты в этом проходе.

1. **P3-1 — HTTP-метод в результате роутера.** В `rust/gui-spike/src/bff_domains_router.rs`
   добавлены `pub struct BffResolvedAction { pub method: &'static str, pub route: String,
   pub body: serde_json::Value }` и `pub fn resolve_bff_domain_action(req: BffDomainRequest) ->
   Result<BffResolvedAction, String>`. Метод берётся из домена: `PATCH` для
   `UpdateGuildOnboarding`, `POST` для `InvokeSlashCommand`, `SendSoundboardSound`,
   `CreateStageInstance`. Маршрут и тело не дублируются — они делегируются существующему
   `dispatch_bff_domain_action`, поэтому две функции не могут разойтись. `BffResolvedAction`
   сериализуем (`Serialize` для IPC/логов); `Deserialize` не выводится, потому что `method` —
   `&'static str`. `dispatch_bff_domain_action`, `SUPPORTED_BFF_DOMAINS`, `is_supported_bff_domain`
   и `BffDomainAction` не изменены (обратная совместимость `build/ dispatch` сохранена).
2. **P3-2 — wire-формат `BffDomainRequest` закреплён тестом.** Существующий
   `bff_domain_action_serde_roundtrip_all_variants` проверял только симметрию
   `serialize → deserialize`; новый тест сравнивает сериализованное значение с буквальным JSON,
   поэтому `#[serde(rename_all = ...)]` или `#[serde(tag = ...)]` теперь ломают тест, а не молча
   меняют контракт BFF/IPC.

Расхождение с формулировкой ТЗ, зафиксировано осознанно (см. §7): вариант `TriggerSlashCommand`
в крейте называется `InvokeSlashCommand` — использовано реальное имя; буквальный JSON
`{"domain":"soundboard","action":...}` не является текущим wire-форматом `BffDomainRequest`
(enum externally tagged), поэтому по оговорке ТЗ «или текущего wire-формата» закреплён текущий
формат, а плоский конверт `{domain, action}` дополнительно проверен как отвергаемый.

## 2. Red

```
$ CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract
error[E0432]: unresolved imports `discord_gui_spike::bff_domains_router::resolve_bff_domain_action`,
  `discord_gui_spike::bff_domains_router::BffResolvedAction`
  --> tests/bff_domains_router_contract.rs:13:58
error: could not compile `discord-gui-spike` (test "bff_domains_router_contract") due to 1 previous error
exit_code=101
```

Лог: `red.log` (md5 `eb6f1e8099faa86e50c1934c3e10b84e`). Падение именно на отсутствующем
продуктовом API, а не на синтаксисе теста: оба новых теста добавлены до реализации.

## 3. Green

```
$ CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit_code=0

$ CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract --lib -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.93s
0 warnings, exit_code=0

$ rustfmt --edition 2021 --check rust/gui-spike/src/bff_domains_router.rs \
  rust/gui-spike/tests/bff_domains_router_contract.rs
exit_code=0

$ CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --lib
test result: ok. 17 passed; 0 failed
exit_code=0
```

Лог: `green.log` (md5 `335205b65578726d92d49c46f590a608`). 8 прежних тестов сохранены,
10 = 8 + 2 новых.

## 4. Targeted и стабильность

- `targeted`: `bff_domains_router_contract` — `10/10 PASS` (md5 теста
  `d851e83f8786ac4ff345219e4b654c19`).
- Регрессия по крейту: `--lib` — `17/17 PASS`.
- Стабильность: контрактный файл прогнан 4 раза подряд (red, первый green, два прогона после
  мутаций) — один и тот же результат `10 passed`, флаки нет. `Cargo.lock` (`rust/gui-spike`) после
  каждого прогона откатывался (`git checkout -- rust/gui-spike/Cargo.lock`), в финальном
  `git status` его нет.

## 5. Oracle

Методы и маршруты сверены с read-only дампом `/home/sanya/re-deep/rest-static.md`, не с кодом
реализации:

| Домен | Ожидание теста | Оракул (файл:строка) |
|---|---|---|
| `slash_commands` | `POST /api/v9/interactions` | `rest-static.md:453` — `POST /interactions` (`Endpoints.INTERACTIONS`) |
| `onboarding` | `PATCH /api/v9/guilds/{guild_id}/welcome-screen` | `rest-static.md:448` — `GET\|PATCH /guilds/{arg}/welcome-screen` (`Endpoints.GUILD_WELCOME_SCREEN`) |
| `soundboard` | `POST /api/v9/channels/{channel_id}/send-soundboard-sound` | `rest-static.md:244` — `POST /channels/{arg}/send-soundboard-sound` (`Endpoints.SEND_SOUNDBOARD_SOUND`) |
| `stage_instances` | `POST /api/v9/stage-instances` | `rest-static.md:524` — `POST /stage-instances` (`Endpoints.STAGE_INSTANCES`) |

`REVIEW-182A` §3 ссылался на строки `751`/`525` для `interactions`/`stage-instances`; содержимое
совпадает, номера строк в дампе сдвинуты — в отчёте указаны фактические (`453`/`524`).
Первые три домена `POST`/`POST`/`POST`, `onboarding` — единственный `PATCH`; тест проверяет
метод для всех четырёх вариантов и сверяет `route`/`body` с выводом `dispatch_bff_domain_action`.

## 6. Мутации (self-check, не заменяет TR)

| # | Мутация в `src/bff_domains_router.rs` | MD5 мутанта | Убийца | Результат |
|---|---|---|---|---|
| M1 | `PATCH` → `POST` в ветке `UpdateGuildOnboarding` | `4c3e9a6d85d5f759ba390e5dca788042` | `resolve_bff_domain_action_returns_http_method_route_and_body` | KILLED (`9 passed; 1 failed`) |
| M2 | `#[serde(rename_all = "snake_case")]` на `BffDomainAction` | `1f052978eb0917bc3bf8959c774bc7c6` | `bff_domain_request_wire_format_matches_tagged_domain_action` | KILLED (`9 passed; 1 failed`) |

Итог: `2/2 KILLED`. Восстановление — `cp` без `-p` (mtime обновлён, крейт пересобран), финальный
md5 `c2e178bd8db5a104d5b8b2978e6b23ed`; после восстановления прогон `10 passed`, `rustfmt` — `0`.
Логи мутаций — в `green.log`, блоки 5 и 6.

## 7. Риски и вопросы

1. **Wire-формат: решение по оговорке ТЗ.** ТЗ предлагало буквальный JSON
   `{"domain":"soundboard","action":{...}}`, но текущий `BffDomainRequest` — externally tagged
   enum (`{"SendSoundboardSound":{...}}`), и переход на плоский конверт был бы ломающим изменением
   вне scope Pass 2. Закреплён текущий формат; плоский конверт явно проверен как отвергаемый.
   Если PM/TR хочет именно `{domain, action}` — это отдельная карточка с миграцией `serde`-атрибутов
   и правкой обоих тестов.
2. **`method` не проверяется на согласованность с `route` в коде.** Инвариант держится тем, что
   `resolve_bff_domain_action` — единственная точка сборки `BffResolvedAction`, а матч по домену
   рядом с вызовом `dispatch`. Тест фиксирует пары домен → метод; отдельная таблица методов
   (дублирующая `dispatch`) сознательно не заводилась.
3. **`BffResolvedAction` без `Deserialize`** — следствие `method: &'static str` из ТЗ. Если план
   запроса понадобится принимать из IPC/тестов как JSON, тип поля придётся менять на `String`
   или `Cow<'static, str>`.
4. **`BffDomainAction` остаётся без `#[non_exhaustive]`** (P3-4 из `REVIEW-182A`) — не входило
   в scope Pass 2, риск прежний: пятый домен станет ломающим изменением для внешних `match`.
5. **Предсуществующий дрейф дерева** (не от этой задачи): ` D node_modules`, ` M package-lock.json`
   были грязными до старта. `rust/gui-spike/Cargo.lock` откатан после прогонов.

## 8. Gate

- `T0` (уровень batch): targeted `bff_domains_router_contract` `10/10`, `clippy -D warnings` — `0`,
  `rustfmt --check` — `0`, `--lib` `17/17`.
- `gate:worker` (`T1`) в batch не запускается — его выполняет integration-лейн. Воркер его не
  запускал и вердикт по нему не заявляет.

## 9. Файлы

| Файл | MD5 |
|---|---|
| `rust/gui-spike/src/bff_domains_router.rs` | `c2e178bd8db5a104d5b8b2978e6b23ed` |
| `rust/gui-spike/tests/bff_domains_router_contract.rs` | `d851e83f8786ac4ff345219e4b654c19` |
| `waves/RW-40/G1/task-284-be-bff-router-pass2/red.log` | `eb6f1e8099faa86e50c1934c3e10b84e` |
| `waves/RW-40/G1/task-284-be-bff-router-pass2/green.log` | `335205b65578726d92d49c46f590a608` |
| `waves/RW-40/G1/task-284-be-bff-router-pass2/REPORT.md` | этот файл |

`rust/gui-spike/src/lib.rs` не менялся (md5 `93d19a31d69ded1e448f330e79db6fd2`): модуль уже
экспортирован, новые публичные элементы доступны через существующий `pub use`-путь.
`git commit`/`push` не выполнялись.

## 10. Следующий шаг

PR в `unstable` → приёмка PM (соответствие `REVIEW-182A` P3-1/P3-2) → Technical Review
(мутации 5/5 на финальных байтах + сверка с `/home/sanya/re-deep/rest-static.md`).
