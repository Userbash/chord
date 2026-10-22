# ТЗ W27b — P6-04 Deep links + P6-03 Автозапуск/бейдж (RW-27 / B09)

Роль/место: PLATFORM-1, файлы — только твои.
Batch: RW-27; зависимости: P3-01 headless-часть, `discord-platform` — каркас (1 тест).

## Цель

Две headless-карточки платформы без GUI/D-Bus/новых зависимостей: разбор `discord://`
deep-link (RE-подтверждённые формы) и генерация XDG autostart-файла + модель бейджа.

## Файлы (только эти)

- `rust/crates/discord-platform/src/deep_link.rs` (новый)
- `rust/crates/discord-platform/src/autostart.rs` (новый)
- `rust/crates/discord-platform/src/lib.rs` (только `pub mod deep_link; pub mod autostart;`)
- `rust/crates/discord-platform/tests/deep_link_contract.rs` (новый)
- `rust/crates/discord-platform/tests/autostart_contract.rs` (новый)

Запрещено: `Cargo.toml`, другие крейты/тесты, доки, golden (`/home/sanya/re-deep` — только
чтение), сеть, D-Bus, live Discord, коммиты, `unsafe`.

## P6-04 — контракт

```rust
pub const MAX_DEEP_LINK_LEN: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum DeepLinkError { Empty, WrongScheme, TooLong, Malformed }

#[derive(Debug, PartialEq, Eq)]
pub struct DeepLink<'a> {
    pub raw: &'a str,
    pub host: &'a str,
    pub path: &'a str,
    pub query: Option<&'a str>,
    pub fragment: Option<&'a str>,
}

pub fn parse_deep_link(raw: &str) -> Result<DeepLink<'_>, DeepLinkError>;
pub fn extract_deep_link(args: &[String]) -> Option<&str>;

#[derive(Clone, PartialEq, Eq)]
pub enum Route {
    LoginOneTime { token: Option<String> },
    OAuth2Authorize,
    AppOpen,
    AppSettings { section: String },
    Library { id: String },
    Channels { path: String },
    Unknown,
}

pub fn route(link: &DeepLink<'_>) -> Route;
```

- Схема сравнивается ASCII-регистронезависимо; `discord:` без `//` — `Malformed`.
- `discord:///library/42/launch` → `host=""`, `path="/library/42/launch"`.
- Query/fragment отрезаются до path; пустые `?`/`#` → `None`.
- Отказ: пустая строка, не-discord схема, длина > 4096, control chars (`< 0x20` или 0x7F)
  в raw, пробелы в scheme/host.
- `extract_deep_link` — точная RE-семантика `extractUrlFromArgs` (`docs/re/main-arch.md`
  §9.4): если есть `--url` и после него есть `--`, вернуть аргумент сразу после `--`;
  иначе первый аргумент с индекса 1, начинающийся (case-insensitive) с `discord://`.
- `Route`: подтверждено RE — `login/one-time` (+`token` из query), `action/oauth2/authorize`,
  `app/open`, `app/settings/<section>`, `library/<id>/launch`, `channels/...` (включая
  `discord://-/channels/...`). Всё остальное — `Unknown`. Никаких выдуманных маршрутов.
- `Route::LoginOneTime` реализует `Debug` вручную: токен не печатается (`"<redacted>"`).

## P6-03 — контракт

```rust
#[derive(Debug, PartialEq, Eq)]
pub struct AutostartEntry<'a> {
    pub name: &'a str,
    pub exec: &'a str,
    pub args: &'a [&'a str],
    pub hidden: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AutostartError { EmptyName, EmptyExec, ControlCharacter }

pub fn desktop_entry(entry: &AutostartEntry<'_>) -> Result<String, AutostartError>;
pub fn autostart_path(config_home: &std::path::Path) -> std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeValue { Count(u64), Visible(bool), Urgent(bool) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Badge { pub count: u64, pub urgent: bool, pub visible: bool }

pub fn badge_pairs(badge: &Badge) -> Vec<(&'static str, BadgeValue)>;
pub fn app_uri(desktop_file: &str) -> String;
```

- Desktop Entry: `[Desktop Entry]\nType=Application\nName=<name>\nExec=<exec + args>\n`
  `X-GNOME-Autostart-enabled=<true|false>\nTerminal=false\n`; `hidden=true` → дополнительно
  `Hidden=true\n` (или `X-GNOME-Autostart-enabled=false` — выбери одно и зафиксируй тестом).
- Экранирование Exec по Desktop Entry spec: аргумент с пробелом/табом/кавычкой/`\`/`$`/`%`
  заключается в двойные кавычки, `"` → `\"`, `\` → `\\`, `$` → `\$`, `` ` `` → `` \` ``,
  `%` → `%%`; пустой аргумент → `""`.
- Отказ: пустые `name`/`exec`, `\n`/`\r`/`\0` в любом поле.
- `autostart_path` = `<config_home>/autostart/discord-lab.desktop` (без создания файлов).
- `badge_pairs` — стабильный порядок: `count`, `count-visible`, `urgent`.
- `app_uri("discord-lab.desktop")` → `"application://discord-lab.desktop"`.
- Wire/D-Bus не реализуется и не заявляется.

## Кейсы

d01–d07 — см. `waves/RW-27/BATCH-TZ.md` §3. a01–a05 — там же.
Минимум: deep links 10 тестов, autostart 6 тестов.

## Шаги

1. Red-first: тесты → red-лог с exit code (compile-red допустим).
2. Минимальный green; `cargo fmt -p discord-platform`.
3. Targeted:
   `CARGO_TARGET_DIR=/tmp/opencode/target-w27b cargo test --offline -p discord-platform --test deep_link_contract --test autostart_contract`.
4. Мутации: deep links 6–10, autostart 4–6; restore по md5.
5. Отчёт `waves/RW-27/workers/W27b/REPORT.md` + red/green логи + md5.
6. Полный gate не запускать — batch-лейн TL-2.
