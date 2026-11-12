# REVIEW-54A — task-140 / BE-RE-04, единая `ClientIdentity` (F-ID-01 / F-GW-09)

Дата: 2026-10-30. Ревьюер: Технический ревьюер (DeepSeek), Review 54A.
Ветка: `feature/task-140-unified-client-identity`, worktree
`/home/user/orca/workspaces/project/feature-task-140-unified-client-identity`,
HEAD `4e24048`. Режим: чтение диффа + целевые тесты + 5 мутационных прогонов.

## Вердикт

**PASS. Ready for Merge.** P0 = 0, P1 = 0. P2 = 2, P3 = 3.

Срез закрывает F-ID-01 на уровне механизма: OP2 IDENTIFY и `X-Super-Properties`
теперь могут получать значения только из одного `ClientIdentity`. На продуктовом
уровне находка ещё не закрыта — боевые REST-вызовы в `gui-spike` по-прежнему
строят отдельную идентичность (P2-1, вне write-scope карточки).

## Что проверено

- Дифф затрагивает ровно два разрешённых файла:
  `rust/crates/discord-core/src/identify.rs` (+214/−27) и
  `rust/crates/discord-core/src/rest/identity.rs` (+218). `super_properties.rs`,
  `supervisor.rs`, тесты в `tests/` и `gui-spike` не менялись. `package-lock.json`
  отличается от HEAD, но в отчёте помечен как чужой и в диффе карточки не участвует.
- Оракул выбран независимо от реализации: `handleIdentify` в бандле веб-клиента
  собирает `properties` как `{...getSuperProperties(), client_app_state,
  is_fast_connect, gateway_connect_reasons, installation_id?}`; живые значения —
  `docs/re/rest-d02.json` (`auth.superProperties`, requestId `95974.2323`), там же
  `launch_signature: da02ad21-…`.
- Арифметика полей сходится: 22 поля SP, из них 7 REST-only (`window_manager`,
  `distro`, `runtime_environment`, `display_server`, `native_build_number`,
  `client_event_source`, `client_heartbeat_session_id`), значит общих с IDENTIFY —
  15, включая `launch_signature` (`identify.rs:51,104`). Три IDENTIFY-only поля —
  константы `is_fast_connect = false`, `gateway_connect_reasons = "COLD_START"`,
  `installation_id = None`.
- `ClientIdentity` хранит один `SuperProperties` (`identity.rs:120-123`) и не даёт
  собрать IDENTIFY из второго набора значений: `for_rw_client` делегирует
  `ClientIdentity::rw_client` (`identify.rs:74-78`), `to_identify_properties` —
  `from_client_identity` (`identify.rs:119-121`).
- `rw_client()` кэширован `OnceLock` (`identity.rs:150-153`); UUID генерируются
  ровно три и в фиксированном порядке (`identity.rs:343-346`), поэтому повторные
  вызовы в процессе дают один `client_launch_id`/`launch_signature`.
- `launch_signature` сериализуется в `properties` OP2 (`identify.rs:253-255`) и
  проверяется по декодированному ETF-кадру, а не по внутреннему полю структуры.
- Тесты не ослаблены: старые тесты не правились, `.only`/`#[ignore]`/skip не
  добавлены. Новые кейсы сверяют значения с отдельной фикстурой SP, в которой все
  22 поля различимы, плюс считают вызовы `random_bytes` (3 до и 3 после IDENTIFY).
- Публичные сигнатуры `build_identify`/`build_identify_frame`/`identity::build`
  не менялись. `IdentifyProperties` получил публичное поле `launch_signature`, но
  литеральных конструкторов `IdentifyProperties { … }` вне модуля в репозитории
  нет — компиляция всех тестов это подтверждает.

## Прогоны

Методика: общий `CARGO_TARGET_DIR` используется несколькими worktree, поэтому
первый прогон подхватил чужой устаревший тестовый бинарь (в нём были
`gateway::tests::presence_update_frame_*`, которых в этом HEAD нет: `gateway.rs`
здесь 458 строк). После `touch rust/crates/discord-core/src/lib.rs` бинарь
пересобран из исходников этой ветки; все логи `REVIEW-54A-*` сняты после этого.

| Прогон | Команда | Результат |
|---|---|---|
| targeted lib | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib` | 8 passed, 0 failed, exit 0 (`REVIEW-54A-GREEN-LIB.log`) |
| все тесты discord-core | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | 53 набора `test result: ok`, 0 failed, exit 0 (`REVIEW-54A-FULL.log`) |
| clippy | `cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | exit 0 (`REVIEW-54A-CLIPPY.log`) |
| fmt | `cd rust && cargo fmt --all -- --check` | exit 0 (`REVIEW-54A-FMT.log`) |

MD5 после всех прогонов совпадают с REPORT.md: `identify.rs`
`95cae2fdb08951759ae280e35554ce81`, `rest/identity.rs`
`5f99d3e0c75b1745fa89964f00818c42`. Мутационные прогоны файлы не изменили.

## Мутации (5/5 пойманы)

Каждая мутация применялась к одному файлу, прогонялся только `--lib`, затем файл
восстанавливался из копии. Подробности — `REVIEW-54A-MUTATION-SUMMARY.md` и
`REVIEW-54A-MUTATION-M*.log`.

| # | Мутация | Результат |
|---|---|---|
| M1 | `launch_signature := client_launch_id` | killed, 4 failed |
| M2 | удалить `launch_signature` из `properties` OP2 | killed, 1 failed |
| M3 | `to_identify_properties` игнорирует идентичность | killed, 3 failed |
| M4 | `rw_client()` без кэша `OnceLock` | killed, 1 failed |
| M5 | `os_arch := app_arch` | killed, 2 failed |
| M6 (доп.) | `browser` захардкожен в `"Discord Client"` | killed, 1 failed |

M1–M2 доказывают, что подмена или потеря `launch_signature` видна и в структуре,
и в проводе. M3 — что IDENTIFY действительно берёт переданную идентичность, а не
дефолт. M4 — что кэш UUID проверяется тестом, а не только объявлен в доке. M5 —
что общие поля не подменяются соседними.

## Findings

**P0:** нет.

**P1:** нет.

**P2-1. Боевая проводка не переключена: в живом процессе остаются две
идентичности.** `gui-spike/src/net.rs:634` создаёт `SupervisorConfig::new`, внутри
— `IdentifyProperties::for_rw_client()` → кэшированный `ClientIdentity::rw_client()`
(browser `"Discord Client"`, UA `discord-lab-rs/0.1.0`, locale `en-US`, один набор
UUID). REST-вызовы (`auth.rs:391,401`, `guilds.rs:320`, `invites.rs:282`,
`messages.rs:132,335`, `reactions.rs:105`, `search.rs:165`, `typing.rs:185`)
по-прежнему зовут `identity::build(&SystemSources, …)` и получают browser
`"discord-lab"`, UA `discord-lab/0.1.0 (Linux; x64)`, локаль из окружения и новые
UUID на каждый вызов. Пока эти места не начнут брать один `ClientIdentity`, F-GW-09
воспроизводится в продукте, хотя внутри ядра расхождение уже невозможно. Нужна
отдельная карточка/integration-патч координатора; мерж этого среза не блокирует,
но закрывать F-GW-09 по продукту рано.

**P2-2. Пара версии и build number остаётся несогласованной с эталоном.**
`ClientIdentity::rw_client` сохраняет `client_version = 0.1.0`,
`browser_version = 0.1.0`, `browser_user_agent = discord-lab-rs/0.1.0` при
`client_build_number = 641004` (`identity.rs:156-168`). У Discord 641004 — сборка
`1.0.162` с `browser_version 42.11.12` и Electron-UA. Отчёт фиксирует это как
риск №2 и не меняет без решения PM; согласен: смена затрагивает политику «свои
честные значения» и должна идти отдельной карточкой. Пока пара внутренне
несогласована и остаётся заметной для анти-абьюза/капчи.

**P3-1. `FMT.log` в каталоге карточки противоречит REPORT.md.**
В `FMT.log` записаны диффы и `EXIT=1`, а REPORT §4 утверждает `EXIT=0`. Текущий
файл rustfmt-clean (`cd rust && cargo fmt --all -- --check` — exit 0,
`REVIEW-54A-FMT.log`), то есть лог устарел: форматирование применили после его
снятия, но лог и строку отчёта не обновили. Перед архивацией batch лог надо
перегенерировать; на код это не влияет.

**P3-2. Комментарий в `gateway/supervisor.rs:146` устарел.**
Там написано «Пересекающиеся 14 полей», после добавления `launch_signature` их 15.
Файл вне write-scope воркера; правку делает Team Lead вместе с документацией.

**P3-3. `package-lock.json` изменён в worktree.**
Отчёт честно называет файл чужим и нетронутым; `git status` это подтверждает.
В коммит карточки его включать не нужно, иначе в срез попадёт побочная правка
`bin`/`engines`.

## Границы доказательств

Проверен backend-срез `discord-core`: маппинг 15 общих полей, провод OP2, кэш
UUID, дефолтная идентичность. Не проверялись: живой Discord, реальная отправка
`X-Super-Properties` из `gui-spike` (P2-1), UI и полный `gate:worker` — по процессу
его выполняет integration/QA на batch. Тесты гонялись на Linux/x86_64; ветки
`macos`/`windows` в `map_os` остались без прогона, но их поведение не менялось.
Мутационные прогоны — по одному запуску на мутанта, без серии повторов; тесты
детерминированные, flakes не наблюдались.
