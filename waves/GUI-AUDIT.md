# GUI-аудит Rust-клиента: что доступно офлайн и что блокирует окно

- Дата: 2026-10-20 (снимок чисел — 12:32; в дереве параллельно идут RW-10…RW-12,
  поэтому счётчики тестов и состав `ready/*` меняются прямо во время чтения).
- Область: `rust/crates/discord-ui`, `discord-app`, `discord-platform`, офлайн-кэш cargo,
  системные библиотеки, кандидаты GUI-тулкитов.
- Код не менялся. Единственный артефакт — этот файл.
- Метод: чтение крейтов/доков, `cargo tree --offline`, `cargo metadata --offline`,
  инвентаризация `~/.cargo/registry`, `pkg-config`, пробные сборки в `/tmp/opencode`
  (не в репозитории), smoke-запуск окна.

## 0. Резюме

1. `discord-ui`, `discord-app`, `discord-platform` — **каркасы**, а не GUI: 21, 14 и 30 строк,
   только константы/баннер и по одному тесту. Окна, event loop, виджетов и экранов нет.
2. Сетевой бэкенд уже сильный: `discord-core` — **187 тестов**, воркспейс — **229/229** в
   текущем рабочем дереве (включая незакоммиченные RW-10/RW-11/RW-12). Это снимок 12:32;
   в `waves/STATE.md` записано 220/178, принятый baseline RW-09 — 158/158 (core 116).
   Для UI уже появился `ReadyData` (P2-01, RW-12, **не принят**), но редьюсера P2-02 и
   стабильного контракта модели всё ещё нет.
3. В офлайн-кэше **нет** Tauri, wry, tao, iced, egui/eframe, winit, slint, fltk, softbuffer,
   pixels, webkit2gtk. `cargo metadata --offline` для каждого даёт `no matching package`.
4. В кэше **есть** полный GTK4-стек: `gtk4 0.11.5` + `libadwaita 0.9.2` и транзитивные
   крейты, 23 `.crate`-файла / 2,0 МиБ сжатых исходников. Это **единственный практически
   доступный GUI без сети**.
5. GTK4 проверен фактически: сборка с нуля **18,93 с** (debug) / **22,27 с** (release),
   release-бинарник **549 КБ**, окно живёт на Wayland и X11. Интеграция с `discord-core`
   в копии рабочего workspace — **19,38 с**, без сети.
6. Системные библиотеки готовы для всех основных кандидатов (GTK4, WebKitGTK 4.1, GTK3,
   xkbcommon, Wayland, X11, Vulkan, EGL). Блокер — **исходники Rust-крейтов**, а не система.
7. ADR-1 говорит «Tauri» (решение владельца 2026-10-19), а комментарии `discord-ui` —
   «ADR-1: iced по умолчанию» и «egui — только прототипы». Это расхождение надо снять
   решением владельца, иначе P0-01 будет ходить по кругу.
8. Рекомендация: **не добавлять Tauri/iced/egui вслепую**. Сделать P0-01a на GTK4 как
   замер резервного нативного тулкита (это прямо входит в DoD P0-01), параллельно
   запросить у владельца разрешение на Tauri 2. После сравнения зафиксировать ADR-1.

## 1. Что реально реализовано в GUI-крейтах

### 1.1 Инвентарь

| Крейт | Файлы/строки | Зависимости | Тесты | Фактическое состояние |
|---|---|---|---|---|
| `discord-ui` | `src/lib.rs`, 21 строка | нет | 1 | Две константы: `COMPONENT_CONTRACTS = 12`, `UI_TERMINAL_FALLBACK = "egui (только прототипы)"`. Виджетов, состояния, экранов нет. |
| `discord-app` | `src/main.rs`, 14 строк | `discord-core` | 0 | Печатает баннер: heartbeat 41 250 мс и `API_V9_BASE`; выходит. Event loop/UI нет. |
| `discord-platform` | `src/lib.rs`, 30 строк | нет | 1 | Константы наследия Electron: 168 каналов, 43 whitelist, 35 bridge, 7 notifications. Трея/уведомлений/буфера обмена нет. |
| `discord-core` | 27 `.rs`-файлов (`src/`, включая `rest/*`, `ready/*`) + 18 интеграционных тестов | tokio, rustls, tungstenite, ruzstd… | **187** | Шлюз, WS-транспорт, IDENTIFY/READY-сводка, RESUME, роутер 21 диспатча, REST-каталог/заголовки/ретраи, HTTP/1.1-транспорт, rate-limit, telemetry-off, ack; в рабочем дереве — `ReadyData` (P2-01) и REST-фасад I1, оба ещё не приняты. |
| `discord-store` | `src/lib.rs`, 25 строк | нет | 1 | Константы таблиц; SQLite/миграций нет. |
| `discord-i18n` | `src/lib.rs`, 54 строки | нет | 2 | Константы локалей и демо-плюрализации; рантайма нет. |
| `discord-media` | `src/lib.rs`, 26 строк | нет | 1 | Константы кодеков; конвейера нет. |
| `discord-voice` | `src/lib.rs`, 48 строк | нет | 1 | Константы RTC; сигнализации/UDP/SRTP нет. |

Проверка:

```text
$ cd rust && cargo test --offline --workspace
passed=229 failed=0 ignored=0
$ cargo test --offline -p discord-core
passed=187 failed=0 ignored=0
```

Важно: 229 — это **текущее рабочее дерево**, в котором есть незакоммиченные RW-10/RW-11/RW-12
(`git status`: новые `rest/http1.rs`, `rest/transport.rs`, `rest/client.rs`, `ready_data.rs`,
`ready/*` и т.д.). В `waves/STATE.md` на это же время записано 220/178 — числа ещё
сдвигаются. Принятый baseline по `waves/STATE.md` (RW-09) — 158/158 (core 116). GUI-план
нельзя строить на непринятых волнах без отдельной приёмки.

### 1.2 Основной цикл и поток событий

`discord-app::main` — не цикл, а два `println!`:

```rust
// rust/crates/discord-app/src/main.rs
fn main() {
    println!("discord-rs: каркас P0 — heartbeat {} мс, REST {}", ...);
    println!("Реализация — волнами P1–P8 ...");
}
```

Транспорт шлюза устроен так, что **владеет циклом** и не отдаёт события наружу до выхода:

```rust
// rust/crates/discord-core/src/ws.rs
pub async fn connect_and_run_resume(
    url: &str,
    pipe: &mut GatewayPipe,
    opts: RunOptions,
    resume: Option<ResumeParams>,
) -> Result<RunOutcome, TransportError>
```

Внутри — `loop { ... pipe.push_binary(...) ... }`; `GatewayPipe::take_events()` вызывается
только после возврата из функции. Пока соединение живо, `&mut GatewayPipe` занят, поэтому
GUI не может дренировать `events` параллельно. Буфер событий растёт линейно
(`ws.rs:95`: «вызывающий обязан дренировать»), это уже отмечено как N4 в `STATE.md`.

**Следствие:** выбор тулкита не решает главную проблему. Для живого UI нужен отдельный
API-приёмник событий (`EventSink`/`mpsc`/callback) или разбиение цикла на шаги. Это
отдельная небольшая волна в `discord-core` без новых зависимостей.

### 1.3 Данных для экранов пока нет

`ReadySummary` (`rust/crates/discord-core/src/ready.rs`) содержит только числа и ID:
`guilds`, `private_channels`, `users`, `read_state_entries`, `user_id`, `session_id` и т.п.
В рабочем дереве уже появился `ready_data.rs` (`ReadyData`: пользователи, гильдии, каналы,
роли, эмодзи, ЛС, read_state, сессии, участники) и параллельно пишется `ready/*`, но всё это
**не закоммичено и не принято** (RW-12/P2-01). Редьюсер `(state, event) -> (state, effects)` —
P2-02 — отсутствует. `DispatchEvent` отдаёт `data: EtfValue` сырым.

Поэтому до приёмки P2-01/P2-02 любой GUI может показывать только **демо/фикстурные** данные
(например, `fixtures/gateway/by-type/002-READY.json`) и счётчики, но не стабильный реальный
список каналов и сообщений.

### 1.4 Расхождение ADR-1

- `docs/rust-client/ARCHITECTURE.md` §8: «ADR-1: UI-оболочка — **Tauri** (решение владельца,
  2026-10-19)»; резерв — iced/Slint.
- `docs/rust-client/PLAN.md` P0-01: «Спайк UI-оболочки (**Tauri**)».
- `rust/crates/discord-ui/src/lib.rs`: «Тулк­ит выбирается спайком P0-01 (**ADR-1: iced по
  умолчанию**)»; `UI_TERMINAL_FALLBACK = "egui (только прототипы)"`.

Это не косметика: от выбора зависит и P0-01, и P3-01/P3-12, и состав загрузки. Нужно
решение владельца и одна запись в ADR-1.

## 2. Офлайн-кэш cargo

### 2.1 Cargo.lock и дерево

```text
$ cd rust && cargo tree --offline | grep -Ei 'gtk|tauri|wry|iced|egui|winit|slint'
(пусто)
$ grep -c '^\[\[package\]\]' rust/Cargo.lock
84
```

В `Cargo.lock` 84 пакета, GUI-крейтов нет. `discord-ui` и `discord-platform` — листья без
зависимостей; `discord-app` тянет только `discord-core`.

### 2.2 Что лежит в кэше

```text
$ ls ~/.cargo/registry/cache/*/ | wc -l
151
$ du -sh ~/.cargo/registry/cache/*/
17M
```

GTK4-стек (все исходники и `.crate` уже локально):

| Компонент | Версия |
|---|---|
| `gtk4`, `gtk4-sys`, `gtk4-macros` | 0.11.5 |
| `gdk4`, `gdk4-sys`, `gsk4`, `gsk4-sys` | 0.11.5 |
| `glib`, `glib-macros`, `glib-sys` | 0.22.10 / 0.22.9 |
| `gio`, `gio-sys`, `gobject-sys` | 0.22.10 / 0.22.9 |
| `pango`, `pango-sys` | 0.22.9 |
| `cairo-rs`, `cairo-sys-rs` | 0.22.9 |
| `graphene-rs`, `graphene-sys` | 0.22.8 / 0.22.9 |
| `gdk-pixbuf`, `gdk-pixbuf-sys` | 0.22.0 / 0.22.9 |
| `libadwaita`, `libadwaita-sys` | 0.9.2 |
| `system-deps`, `toml*`, `serde_core` и transitive | локально |

Суммарный объём только GTK4-стека — **2,0 МиБ** (23 `.crate`-файла); вместе со служебными
`system-deps`/`toml*`/`serde_core` — 32 файла, 2,4 МиБ. В локальном индексе
`~/.cargo/registry/index` — 162 записи; среди GUI-имён есть только `gtk4` (плюс `gdk4`,
`gsk4`, `glib`, `pango`, `cairo-rs`, `libadwaita`). Записей `tauri`, `wry`, `tao`, `iced`,
`egui`, `eframe`, `winit`, `slint`, `fltk`, `softbuffer`, `pixels`, `webkit2gtk` нет.

### 2.3 Проверка кандидатов `cargo metadata --offline`

Для каждого кандидата создавался минимальный манифест в `/tmp/opencode`:

```text
tauri          error: no matching package named `tauri` found
wry            error: no matching package named `wry` found
tao            error: no matching package named `tao` found
iced           error: no matching package named `iced` found
egui           error: no matching package named `egui` found
eframe         error: no matching package named `eframe` found
winit          error: no matching package named `winit` found
slint          error: no matching package named `slint` found
fltk           error: no matching package named `fltk` found
softbuffer     error: no matching package named `softbuffer` found
pixels         error: no matching package named `pixels` found
webkit2gtk     error: no matching package named `webkit2gtk` found
```

`gtk4` в такой проверке резолвится, но `cargo metadata` всё равно требует пакет, которого
нет в кэше: в GTK-пробнике это `serde_derive 1.0.229`, в копии рабочего workspace —
`r-efi 6.0.0` (target-specific зависимость `getrandom`/`rand`/`tungstenite`, для Linux не
собирается). Это особенность `cargo metadata` (резолвит все target'ы), а не сборки:
`cargo build --locked --offline` и `cargo test --locked --offline` проходят. Для CI
использовать `--locked --offline`, а lock обновлять отдельным контролируемым шагом.

### 2.4 GTK4: фактические замеры

Пробник в `/tmp/opencode/gtk4-probe` (не в репозитории): `gtk4 = "0.11.5"`, окно
`ApplicationWindow` + `Label`.

```text
$ cargo build --offline
Finished `dev` profile ... in 18.93s
$ cargo build --release --offline
Finished `release` profile ... in 22.27s
$ ls -lh target/release/gtk4-probe
-rwxr-xr-x 549K target/release/gtk4-probe
```

Release-бинарник динамически линкуется с системными `libgtk-4.so.1`, `libglib-2.0`,
`libpango`, `libcairo`, `libgdk_pixbuf`. Release-target с зависимостями — **345 МБ**,
debug-target после нескольких сборок — около **1 ГБ** (incremental), сам debug-бинарник —
56 МБ.

Smoke-запуск (окно на реальной сессии, затем `timeout`):

```text
$ GDK_BACKEND=wayland timeout 3s ./target/debug/gtk4-probe
exit=124        # event loop жив, окно создано
$ GDK_BACKEND=x11 timeout 3s ./target/debug/gtk4-probe
exit=124
```

Интеграция с ядром: копия рабочего workspace, в `discord-app` добавлен `gtk4`, в окне
выводятся `HEARTBEAT_INTERVAL_MS` и `API_V9_BASE` из `discord-core`:

```text
$ cargo build --offline -p discord-app
Finished `dev` profile ... in 19.38s
-rwxr-xr-x 62M target/debug/discord-rs
```

GTK4 добавляет в lock **51 пакет** (84 → 135). 50 из них уже в кэше; отсутствует только
`serde_derive 1.0.229` — он не нужен для `cargo build`/`cargo test`, но мешает
`cargo metadata`/`cargo fetch` в офлайне. `libadwaita` 0.9.2 собирается офлайн за 1,7 с;
feature `v4_24` (системный GTK 4.24.1) также собирается.

Ограничение MSRV: `gtk4 0.11.5` требует **Rust 1.92**, в `rust/Cargo.toml` записано
`rust-version = "1.87"`. Фактический rustc — 1.99, сборка идёт; но политику MSRV надо
обновить, иначе CI на 1.87 упадёт.

Проверка `unsafe_code = "forbid"`: пробник с `#![forbid(unsafe_code)]` и макросом
`glib::clone!` собирается. Базовый safe-API GTK4 и `clone!` совместимы с запретом unsafe;
для `CompositeTemplate`/некоторых макросов это надо проверить отдельно.

## 3. Системные библиотеки

`pkg-config --modversion`:

| Библиотека | Версия | Для чего |
|---|---|---|
| `gtk4` | **4.24.1** | GTK4-путь |
| `libadwaita-1` | **1.10.0** | современные виджеты GTK4 |
| `gtk+-3.0` | 3.24.52 | Tauri/wry (GTK3) |
| `webkit2gtk-4.1` | **2.54.1** | системный WebView для Tauri |
| `javascriptcoregtk-4.1` | 2.54.1 | JS в WebKitGTK |
| `libsoup-3.0` | 3.8.0 | HTTP WebKitGTK |
| `xkbcommon` / `xkbcommon-x11` | 1.13.2 | ввод/раскладки |
| `wayland-client` | 1.26.0 | Wayland |
| `x11` / `xcb` | 1.8.14 / 1.17.0 | X11/XWayland |
| `vulkan` | 1.4.363 | wgpu/iced/egui |
| `egl` / `glesv2` | 1.5 / 3.2 | GL-рендер |
| `librsvg-2.0` | 2.63.3 | SVG-иконки |
| `gstreamer-1.0` | 1.28.8 | медиа WebKitGTK |
| `libsecret-1` | 0.21.8.2 | будущий keyring (P1-10) |
| `appindicator3-0.1` | 12.10.0 | будущий трей |
| `dbus-1` | 1.16.2 | уведомления/трей |
| `glycin-2` | 2.2.3 | загрузка изображений GTK 4.24 |

Чего нет:

- `gtk4-layer-shell` — не нужен для обычного окна, понадобится для оверлеев/панелей.
- `patchelf` — нет; нужен только для упаковки Tauri-бандлов (AppImage/DEB), не для dev-запуска.
- Каталог loaders `gdk-pixbuf` не найден; при этом `pkg-config` GTK 4.24 показывает Glycin
  2.2.3. Форматы аватаров (PNG/WebP) надо проверить в спайке, а не считать данностью.

Сессия: `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-1`, `DISPLAY=:0` (XWayland).
Оба backend GTK4 проверены smoke-запуском.

## 4. Кандидаты GUI без сети

Оценки объёма/времени для **отсутствующих** крейтов — ориентировочные: точный список
транзитивных зависимостей и размер офлайн получить нельзя, локального индекса нет.
Точные цифры даст `cargo fetch` + `cargo tree` после разрешения сети.

| Вариант | Офлайн сейчас | Что загрузить | Системные зависимости | Риск | До первого окна | Вывод |
|---|---|---|---|---|---|---|
| **Tauri 2 + wry/tao** | **Нет** | `tauri 2.x`, `tauri-build 2.x` + ~200–300 транзитивных (оценка 20–40 МБ) | WebKitGTK 4.1, GTK3, libsoup3, librsvg — есть; `patchelf` нет | Средний/высокий: версии, frontend-стек, WebKit Wayland/IME, размер | 5–10 мин первой сборки (оценка) | ADR-1; основной путь **после разрешения владельца** |
| **iced 0.13** | **Нет** | `iced 0.13` + ~100–150 (оценка 10–20 МБ) | Vulkan/EGL/X11/Wayland есть | Средний: wgpu, text/IME, кастомные виджеты | 3–6 мин (оценка) | Резерв №1 по ADR; офлайн не проверен |
| **egui/eframe 0.32** | **Нет** | `eframe 0.32` + ~80–120 (оценка 8–15 МБ) | Vulkan/EGL/X11/Wayland есть | Средний: immediate-mode, accessibility, IME | 2–4 мин (оценка) | Прототипы/инструменты, не полный клиент |
| **winit + softbuffer/pixels** | **Нет** | `winit 0.30`, `softbuffer 0.4`, text/font-крейты (~50–80, оценка 5–10 МБ) | X11/Wayland/EGL есть | Высокий: нет виджетов/текста, всё вручную | 2–4 мин до пустого окна | Не для P3; только свой рендерер |
| **Системный WebView без Tauri** | **Нет binding** | `webkit2gtk`, `gtk 0.18`, `javascriptcore-rs`, `soup3` | WebKitGTK 4.1 есть | Высокий: GTK3, ручной IPC, raw FFI/unsafe | 5–10 мин (оценка) | Не рекомендуется: `unsafe_code=forbid` |
| **TUI (std/ANSI)** | **Да** | 0 | нет | Низкий, но это не GUI | минуты | Временный debug-клиент, не продукт |
| **GTK4 0.11.5 (+libadwaita 0.9.2)** | **Да** | **0 для сборки** (GTK-стек 2,0 МиБ уже в кэше) | GTK4 4.24.1, glib/pango/cairo/gdk-pixbuf/graphene — есть | Средний: отклонение от ADR-1, CSS/токены, непроверенная виртуализация 100k/24k | **18,93 с** (проверено) | **Единственный доступный офлайн GUI; P0-01-резерв** |

Пояснения:

- **Tauri** — не только Rust-крейты: в репозитории нет `frontend/`, `tauri.conf.json` и
  Tauri CLI; фронтенд-стек (vanilla HTML/CSS/JS или фреймворк) не выбран. Tauri даёт
  Rust-бэкенд + web-view; если «клиент на Rust» означает ещё и UI на Rust, это надо
  проговорить с владельцем отдельно.
- **Системный WebView без Tauri** технически возможен (WebKitGTK 4.1 установлен), но
  безопасного Rust-binding в кэше нет; raw FFI потребует `unsafe`, который запрещён
  линтом крейтов. Это не «бесплатный» путь.
- **TUI** полезен как отладочный интерфейс шлюза, но не закрывает P0-01 (нет FPS/RAM
  окна, виртуализации, ввода/IME).

## 5. Что можно сделать без новых зависимостей уже сейчас

| Слой | Можно | Нельзя без зависимостей/ядра |
|---|---|---|
| Модель состояния | `AppState`, `Screen`, выбранная гильдия/канал, статус соединения, composer-state — чистые структуры | Принятый контракт P2-01 (`ReadyData` в дереве, но не принят) и редьюсер P2-02 |
| Командный слой | `UiAction` (SelectGuild/Channel, ComposerChanged, Send, OpenSettings, QuickSwitcher, EmojiQuery), `reduce(state, action)` | Живой вызов REST/шлюза без event-sink и приёмки REST-фасада I1 (в дереве green, RW-12) |
| Экраны как данные | гильд-рейл, дерево каналов, ЛС-список, элементы сообщений, настройки — view-model структуры | Реальные данные, картинки, ввод/IME |
| Виртуализация | чистая математика `visible_range(scroll, heights, viewport)`, группировка сообщений, day-разделители | Отрисовка без тулкита; бенчмарк 100k/24k |
| Токены/контракты | таблица 12 контрактов D-03, токены dark/light/cozy/compact как Rust-структуры | 1:1 CSS-переменные; в GTK CSS нужен слой генерации |
| TUI | ANSI-рендер demo-состояния, `--demo` на фикстурах | Живые события без `EventSink` |
| GTK4-окно | окно, гильд-рейл, дерево, чат-заглушка, настройки-заглушка на demo-данных | Продуктовый UI до решения ADR-1 |

Важное расхождение по токенам: `docs/re/frontend-d03.md` §1 фиксирует **4795** CSS-переменных
(`ui/tokens.json`), а `ARCHITECTURE.md`/PLAN говорят о **5132**. Перед P3-13 нужно выбрать
источник и зафиксировать число; «1:1 CSS-переменные» в GTK4 не гарантированы — понадобится
генерация CSS/темы из Rust-токенов.

### Минимальный набор для первого окна

```toml
# Отдельный спайк-проект (например, rust/spikes/gtk4 или /tmp), НЕ discord-app:
# так ADR-1 остаётся обратимым, а продуктовый workspace не получает GTK4 до решения.
[dependencies]
gtk4 = "0.11.5"          # сборка без сети; см. оговорку про metadata ниже
# опционально:
# libadwaita = "0.9.2"   # современные виджеты, тоже локально
```

Этого достаточно для окна с гильд-рейлом/деревом/заглушкой чата. 50 из 51 нового пакета
lock уже в кэше; `cargo build --offline` проверен. Но даже офлайн-добавление меняет
`Cargo.toml`, `Cargo.lock` (84 → 135) и MSRV (1.87 → 1.92), поэтому это отдельный
спайк-крейт с разрешения координатора, а не тихая правка `discord-app`. `discord-ui`
остаётся нейтральным (state/actions/view-model/токены); GTK4-адаптер — `discord-ui-gtk`
только после ADR-1. Это **не** означает, что GTK4 выбран основным тулкитом — это спайк/резерв.

## 6. Рекомендация и план на 2–3 волны

### 6.1 Решение

1. **Сейчас, без сети:** P0-01a — GTK4-спайк как «замер резервного нативного тулкита»
   (это явно входит в DoD P0-01) + UI-логика в `discord-ui` без тулкита. Спайк — отдельный
   крейт/каталог, чтобы не тащить GTK4 в продуктовый workspace и не менять MSRV до ADR-1;
   на изменение `Cargo.toml`/`Cargo.lock` нужно разрешение координатора (сети не нужно).
2. **Параллельно:** запросить у владельца разрешение на Tauri 2 — если он хочет следовать
   ADR-1. Без разрешения Tauri-плечо P0-01 невозможно: крейтов нет, `cargo metadata
   --offline` падает.
3. **После сравнения:** зафиксировать ADR-1 числами (FPS/RAM/сборка/старт). Если Tauri
   разрешён и проходит — Tauri основа, GTK4 остаётся резервом. Если Tauri не разрешён или
   провалил метрики — ADR-1 меняется на GTK4 (или iced, если владелец разрешит загрузку).
4. **Не добавлять iced/egui/winit** «на всякий случай»: это ещё три загрузки и три
   разных UI-каркаса, а ADR-1 уже называет резервом iced.

### 6.2 Что просить у владельца (если Tauri)

Одноразовое разрешение на сеть для `cargo fetch` (без live Discord, без телеметрии):

- прямые: `tauri = "2"`, `tauri-build = "2"` (build-dependency);
- транзитивные: `wry`, `tao`, `webkit2gtk`, `javascriptcore-rs`, `soup3`, `gtk 0.18`,
  `serde`/`serde_derive` и др. — ориентировочно 200–300 пакетов, 20–40 МБ;
- точные версии фиксируются `Cargo.lock` после загрузки; офлайн-список версий сейчас
  получить нельзя, потому что локального индекса для этих крейтов нет;
- системные зависимости уже стоят; `patchelf` отсутствует, но нужен только для бандлинга,
  не для `cargo run`.

Альтернативные запросы (если владелец передумает по Tauri):

- `iced = "0.13"` — ориентировочно 100–150 пакетов, 10–20 МБ;
- `eframe = "0.32"` — ориентировочно 80–120 пакетов, 8–15 МБ (только прототип);
- `winit = "0.30"` + `softbuffer = "0.4"` — 50–80 пакетов, 5–10 МБ (слишком низкий уровень).

### 6.3 Волны

| Волна | Карточки | Содержание | Зависит от | Сеть | DoD |
|---|---|---|---|---|---|
| **G1** | **P0-01a** (резерв) | `discord-ui` без тулкита: `AppState`, `UiAction`, `reduce`, view-model гильд-рейла/дерева/ЛС, токены, `visible_range`; отдельный GTK4-спайк: окно + 3 экрана (чат-заглушка 100k, эмодзи-пикер, настройки) на demo-данных; замеры FPS/RAM/сборки/старта | — | **Нет** | Окно на Wayland/X11; 3 экрана; метрики; unit-тесты UI-логики; `cargo test --locked --offline`; без изменений golden |
| **G2** | **P0-01b** + core event sink | Решение по ADR-1: либо Tauri-спайк тех же 3 экранов после разрешения, либо повышение GTK4-спайка до `discord-ui-gtk`. В `discord-core` — приёмник событий (`EventSink`/`mpsc`/callback) вместо накопления `events`; тест «события приходят во время соединения»; TUI/debug-клиент как побочный продукт | G1; разрешение владельца (для Tauri) | Tauri — да; GTK4 — нет | Сравнительная таблица Tauri/GTK4; ADR-1 с числами; event sink покрыт тестом; `--demo` работает |
| **G3** | **P3-01 + P3-12/P3-13** (вертикальный срез) | Каркас окна: гильд-рейл, дерево каналов, ЛС-список, чат-заглушка, настройки-категории; компоненты Button/Input/Menu/Modal/Toggle; токены dark/light/cozy/compact; подключение к P2-02 (или fixture-модели до неё); бенчмарки 100k сообщений/24k участников | G2; P2-02 | Нет (fixtures) | T-UI-03…05, T-UI-11…15 (частично); 60 fps на 24k/100k или зафиксированный провал; gate |

Что **не** входит в эти 3 волны: полный P3 (16 карточек), P3-02 chat-view с embeds/реакциями,
P3-03 отправка, P3-04 вложения, P3-09 поиск, P3-16 полные настройки. Это следующие волны
после P2-01/P2-02/P2-06. Честный результат 3 волн — **работающий каркас и 3 экрана**, а не
«полный Discord».

## 7. Риски

| Риск | Почему | Митигация |
|---|---|---|
| ADR-1 расходится с кодом | ARCHITECTURE/PLAN — Tauri, `discord-ui` — iced/egui | Решение владельца + одна запись ADR-1 до G2 |
| Tauri-крейтов нет офлайн | `cargo metadata --offline` → `no matching package` | Запрос сети/`cargo fetch`; без него Tauri-плечо P0-01 невозможно |
| Tauri тянет web-фронтенд | Нет `frontend/`, `tauri.conf.json`, Tauri CLI; фронтенд-стек не выбран | Выбрать vanilla HTML/CSS/JS для спайка или явно согласовать фреймворк |
| GTK4 — отклонение от ADR-1 | Владелец выбрал Tauri | Оформить как «резервный замер» P0-01, не как тихую замену |
| GTK4 MSRV 1.92 | Workspace заявляет 1.87 | Поднять `rust-version` или не добавлять GTK4 в основной workspace до решения |
| Lock хрупок офлайн | `cargo metadata` требует отсутствующий пакет (`serde_derive 1.0.229` в GTK-пробнике, `r-efi 6.0.0` в копии workspace) | Для сборки/тестов — `--locked --offline`; `metadata`/`fetch` — отдельным шагом с сетью; не считать падение `metadata` поломкой сборки |
| Виртуализация GTK4 не проверена | 100k сообщений/24k участников — бюджет 60 fps | Бенчмарк в G1/G3; при провале — Tauri/iced |
| Нет event sink | `connect_and_run_resume` держит `&mut GatewayPipe` до закрытия | Отдельная волна в `discord-core`; без неё live UI невозможен |
| READY-модель не принята | `ReadyData` есть в рабочем дереве, но P2-01 не принят; P2-02 отсутствует | G1/G3 на fixture/demo; реальные данные — после приёмки P2-01/P2-02 |
| Токены 4795 vs 5132 | D-03 и ARCHITECTURE расходятся | Зафиксировать источник; генерировать CSS/тему из Rust-токенов |
| GTK4 CSS не 1:1 | GTK CSS — не CSS web-клиента | Слой токенов + генерация; не обещать пиксель-в-пиксель |
| `unsafe_code=forbid` | Raw WebView/X11 FFI запрещён | Только safe-обёртки (gtk4/iced/egui); Tauri/wry — safe API |
| Аватары/форматы | `gdk-pixbuf` loaders не найдены, есть Glycin 2.2.3 | Проверить PNG/WebP в спайке; при необходимости — системный loader/`image` |
| Текущее дерево не принято | RW-10/RW-11/RW-12 незакоммичены | GUI-волны стартовать от принятого baseline или дождаться приёмки |
| Live Discord/ToS | Нельзя трогать без окна владельца | Только fixtures/demo; live-smoke — отдельное окно |

## 8. Приложение: ключевые команды

```bash
# состояние и тесты (снимок 12:32)
cd rust
cargo tree --offline | grep -Ei 'gtk|tauri|wry|iced|egui|winit|slint'   # пусто
cargo test --locked --offline --workspace                             # 229/229
cargo test --locked --offline -p discord-core                         # 187

# кэш/индекс
ls ~/.cargo/registry/cache/*/ | wc -l                                  # 151
du -sh ~/.cargo/registry/cache/*/                                      # 17M
find ~/.cargo/registry/index -type f -path '*/.cache/*' | wc -l        # 162

# кандидаты (минимальный манифест в /tmp, не workspace)
cargo metadata --offline --format-version 1                            # tauri: no matching package

# система
pkg-config --modversion gtk4                                           # 4.24.1
pkg-config --modversion webkit2gtk-4.1                                 # 2.54.1
pkg-config --modversion xkbcommon                                      # 1.13.2

# GTK4-пробник (в /tmp, не в репозитории)
cd /tmp/opencode/gtk4-probe
cargo build --offline                                                  # 18.93s
cargo build --release --offline                                        # 22.27s
GDK_BACKEND=wayland timeout 3s ./target/debug/gtk4-probe; echo $?      # 124 — окно живо
GDK_BACKEND=x11 timeout 3s ./target/debug/gtk4-probe; echo $?          # 124
```

Итог: **офлайн сейчас доступен ровно один GUI-путь — GTK4**. Всё остальное требует
разрешения на сеть и загрузку крейтов. Ближайший разумный шаг — использовать GTK4 для
P0-01a (резервный замер) в отдельном спайк-крейте и одновременно запросить Tauri 2
у владельца; выбор основного тулкита зафиксировать в ADR-1 после сравнения, а не по
комментарию в `discord-ui`.
