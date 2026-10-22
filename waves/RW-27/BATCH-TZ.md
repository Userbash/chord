# BATCH RW-27 — B09: headless i18n + platform (без GUI и новых зависимостей)

Дата: 2026-10-22. Team Lead: TL-1. Tech Lead: TL-2.
Статус: **frozen** (контракты заморожены до кода).

## 1. Состав batch

| Карточка | Seat | Файлы (только эти) | Зависит от | Оракул |
|---|---|---|---|---|
| P7-01b ListFormat | CLIENT-1 / W27a | `discord-i18n/src/{list.rs,list_data.rs,lib.rs}`, `discord-i18n/tests/list_contract.rs` | P7-01 `[x]` | Node 24 `Intl.ListFormat`: `waves/RW-27/fixtures/list-oracle.json`, `generated/list_data.rs` |
| P6-04 Deep links | PLATFORM-1 / W27b | `discord-platform/src/{deep_link.rs,lib.rs}`, `discord-platform/tests/deep_link_contract.rs` | P3-01 (headless-часть) | `docs/re/main-arch.md` §9.4, строки бандлов `/home/sanya/re-deep/out/bundles/*.js` |
| P6-03 Автозапуск + бейдж | PLATFORM-1 / W27b | `discord-platform/src/{autostart.rs,lib.rs}`, `discord-platform/tests/autostart_contract.rs` | P3-01 (headless-часть) | XDG Desktop Entry 1.5, Unity Launcher API; RE: `X-GNOME-Autostart-enabled` |

Общие файлы: `discord-i18n/src/lib.rs` — только W27a; `discord-platform/src/lib.rs` — только W27b.
Крейты разные, пересечения файлов нет. TL-2 принимает интеграционный дифф и запускает batch gate.

Запрещено: чужие модули/тесты, `Cargo.toml`/`Cargo.lock`, `discord-core`, `discord-ui`, доки
`STATE/STATUS/ROADMAP/TASKS/map`, golden (`logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep`
на запись), сеть, live Discord, `git commit/push`, `unsafe`.

## 2. Замороженный контракт

### P7-01b — ListFormat

```rust
pub enum ListStyle { Long, Short, Narrow }
pub enum ListType { Conjunction, Disjunction, Unit }
pub fn format_list(locale: &str, items: &[&str], style: ListStyle, kind: ListType) -> String;
```

- 31 **включённая** локаль (`locale::SUPPORTED_LOCALES` минус `DISABLED_LOCALES`),
  fallback для неизвестной/выключенной локали — `en-US`.
- Пустой список → `""`; один элемент → он сам без изменений.
- Данные — `list_data.rs`, сгенерирован из ICU; ручная правка запрещена.

### P6-04 — Deep links

```rust
pub enum DeepLinkError { Empty, WrongScheme, TooLong, Malformed }
pub struct DeepLink<'a> { pub raw: &'a str, pub host: &'a str, pub path: &'a str, pub query: Option<&'a str>, pub fragment: Option<&'a str> }
pub fn parse_deep_link(raw: &str) -> Result<DeepLink<'_>, DeepLinkError>;
pub fn extract_deep_link(args: &[String]) -> Option<&str>;
pub enum Route { LoginOneTime { token: Option<String> }, OAuth2Authorize, AppOpen, AppSettings { section: String }, Library { id: String }, Channels { path: String }, Unknown }
pub fn route(link: &DeepLink<'_>) -> Route;
```

- Схема — регистронезависимая `discord://`; пустой хост допустим (`discord:///library/1/launch`).
- Предел `MAX_DEEP_LINK_LEN = 4096`; управляющие символы (кроме `%`-encoded) отклоняются.
- `extract_deep_link` повторяет RE `extractUrlFromArgs`: `--url` валиден только при наличии `--`
  после него; иначе первый аргумент (с индекса 1), начинающийся с `discord://`.
- `Debug` для `Route::LoginOneTime` **не печатает токен** (redaction).
- Подтверждённые RE маршруты: `login/one-time`, `action/oauth2/authorize`, `app/open`,
  `app/settings/<section>`, `library/<id>/launch`, `channels/...`. Остальное — `Unknown`.

### P6-03 — Автозапуск + бейдж

```rust
pub struct AutostartEntry<'a> { pub name: &'a str, pub exec: &'a str, pub args: &'a [&'a str], pub hidden: bool }
pub fn desktop_entry(entry: &AutostartEntry<'_>) -> Result<String, AutostartError>;
pub fn autostart_path(config_home: &std::path::Path) -> std::path::PathBuf;
pub enum BadgeValue { Count(u64), Visible(bool), Urgent(bool) }
pub struct Badge { pub count: u64, pub urgent: bool, pub visible: bool }
pub fn badge_pairs(badge: &Badge) -> Vec<(&'static str, BadgeValue)>;
pub fn app_uri(desktop_file: &str) -> String;
```

- Desktop Entry: `[Desktop Entry]`, `Type=Application`, `Name=`, `Exec=` (аргументы
  экранируются по spec: пробелы/кавычки/`%`), `X-GNOME-Autostart-enabled=`, `Terminal=false`.
- Отказ на `\n`, `\r`, `\0`, пустое имя/exec; `%` в exec → `%%` (поле field-code).
- `autostart_path` = `<config_home>/autostart/discord-lab.desktop`.
- Бейдж — модель свойств Unity Launcher API (`count`, `count-visible`, `urgent`), без D-Bus
  и без заявлений о wire-совместимости.

## 3. Кейсы (red-first)

| ID | Карточка | Проверка | Ожидание |
|---|---|---|---|
| l01 | P7-01b | en-US long conjunction, 0/1/2/3/5 | `""`, `A`, `A and B`, `A, B, and C`, `A, B, C, D, and E` |
| l02 | P7-01b | ru long conjunction/disjunction | `A, B и C`, `A, B или C` |
| l03 | P7-01b | zh-CN unit, 4 элемента | склейка без разделителей (`ABCD`) |
| l04 | P7-01b | все 1674 кейса `list-oracle.json` (31 локаль × 3 стиля × 3 типа × 6 размеров; уточнение TL-2 после регенерации таблицы) | точное совпадение |
| l05 | P7-01b | неизвестная/выключенная локаль (`he`, `xx`) | fallback en-US |
| l06 | P7-01b | short/narrow + unit/disjunction | по оракулу |
| d01 | P6-04 | `discord://app/settings/nitro` | host=app, route AppSettings("nitro") |
| d02 | P6-04 | `discord:///library/42/launch` | пустой host, Library("42") |
| d03 | P6-04 | `discord://login/one-time?token=secret` | LoginOneTime(Some), Debug без secret |
| d04 | P6-04 | `discord://action/oauth2/authorize?...` | OAuth2Authorize |
| d05 | P6-04 | `discord://-/channels/1/2` и `discord://channels/@me` | Channels(path) |
| d06 | P6-04 | пустая строка, `https://`, >4096, control chars | типизированные ошибки, без паники |
| d07 | P6-04 | `extract_deep_link` с `--url`/`--`/без них | RE-семантика |
| a01 | P6-03 | минимальный entry | точный desktop-файл |
| a02 | P6-03 | пробелы/кавычки/`%` в exec | spec-экранирование |
| a03 | P6-03 | `\n`/`\0`/пустые поля | `AutostartError`, без паники |
| a04 | P6-03 | путь автозапуска | `<config>/autostart/discord-lab.desktop` |
| a05 | P6-03 | badge pairs | count/count-visible/urgent, порядок стабилен |

Бюджет: ListFormat S/M → 8–12 тестов; Deep links M → 10–16; Autostart S → 6–10.
Мутации: ListFormat 5–8; Deep links 6–10; Autostart 4–6.

## 4. Команды

```sh
# W27a (свой target-dir)
CARGO_TARGET_DIR=/tmp/opencode/target-w27a cargo test --offline -p discord-i18n --test list_contract

# W27b (свой target-dir)
CARGO_TARGET_DIR=/tmp/opencode/target-w27b cargo test --offline -p discord-platform \
  --test deep_link_contract --test autostart_contract

# batch gate — один раз, TL-2
cd rust && cargo fmt --all -- --check && \
  cargo clippy --offline --workspace --all-targets -- -D warnings && \
  cargo test --offline --workspace && cd .. && npm run gate:worker
```

## 5. Definition of done

- [ ] red-лог с exit code (compile-red допустим для нового API); green targeted;
- [ ] файлы + md5; `TEST-CORRECTIONS.md` при расхождении;
- [ ] мутации пойманы, продукт восстановлен по md5;
- [ ] batch gate passed;
- [ ] независимое ревью (codex + TL-1), P0/P1/P2=0;
- [ ] `accepted` + `db.jsonl` + `STATE.md` + STATUS/ROADMAP/TASKS/map один раз.
