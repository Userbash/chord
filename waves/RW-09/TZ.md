# ТЗ RW-09 — X-Super-Properties (P1-08 / T-REST-04): параллельная волна

**Проект:** discord-lab + Rust-клиент. Корень: `/home/user/orca/project`; cargo-воркспейс: `rust/`.
**Карточка:** P1-08 (`docs/rust-client/PLAN.md`) — «генератор X-Super-Properties (22 поля),
конфигурируемо, политика "честные значения"»; тест T-REST-04 (`docs/rust-client/TEST-PLAN.md`,
L1: «22 поля, base64 JSON, round-trip»; оракул — `docs/re/rest-d02.md` §6.2).
**Дата старта:** 2026-10-20. **Тип:** вторая параллельная волна (PROCESS.md §3.13): 3 воркера
одновременно + рефакторинг-воркер W9d + независимый ревьюер W9r. Предшественник P1-07 (RW-08)
закрыт.

## 0. Решения волны (координатор)

1. **Новых зависимостей нет** и не требуется: `serde`/`serde_json`/`uuid` отсутствуют в
   `Cargo.lock`, поэтому base64 (RFC 4648), JSON-запись/разбор 22 полей и UUID v4 реализуются
   в волне вручную (маленькие, полностью покрываемые тестами). `Cargo.toml`/`Cargo.lock` не
   трогаем; сеть — только `127.0.0.1` (в этой волне сеть не нужна вовсе).
2. **Оракулы — живой санитизированный снимок** `reference-oracle/d02-snapshot` (чтение):
   базовый64-значение заголовка `X-Super-Properties` из запроса `95974.2323` и его
   побайтовый JSON. Выдержки заморожены в `rust/crates/discord-core/tests/data/` (§3).
3. **Политика «свои честные значения»** (§6.1): среда (ОС, ядро, архитектура, локаль,
   дистрибутив, дисплей, оконный менеджер) считывается честно из системы; поля идентичности
   нашего клиента (`browser = "discord-lab"`, версия = версия крейта, build = 0) — свои, не
   имитируют чужую сборку; телеметрические UUID — свежие v4. Всё конфигурируемо; тесты
   фиксируют каждое поле.
4. **Контракты заморожены** (каркас §4/§5/§6 уже в репозитории, md5 в §3.4): воркеры не меняют
   сигнатуры; общие файлы (`rest/mod.rs`, `tests/data/*`) правит только координатор.
5. Красная линия по разрешениям: живые запросы к Discord не делаются (генератор — данные).

## 1. Файлы и границы (пересечений нет)

**Каркас координатора — заморожен (md5 §3.4), воркеры не меняют:**

| Файл | Что внутри |
|---|---|
| `rust/crates/discord-core/src/rest/mod.rs` | добавлены `pub mod base64/identity/super_properties` |
| `rust/crates/discord-core/src/rest/base64.rs` | контракт W9a (заглушка) |
| `rust/crates/discord-core/src/rest/super_properties.rs` | контракт W9b (заглушка) |
| `rust/crates/discord-core/src/rest/identity.rs` | контракт W9c (заглушка) |
| `rust/crates/discord-core/tests/data/rest-super-properties-2323.b64` | живой base64 (1084 симв.) |
| `rust/crates/discord-core/tests/data/rest-super-properties-2323.json` | побайтовый JSON из base64 |
| `rust/crates/discord-core/tests/data/rest-super-properties-22.tsv` | 22 строки: поле, тип, значение |
| `rust/crates/discord-core/tests/data/rest-base64-vectors.tsv` | 10 векторов (python3) |

**Воркеры (каждый — только свои два файла):**

| Воркер | Продукт | Тест |
|---|---|---|
| W9a | `rust/crates/discord-core/src/rest/base64.rs` | `rust/crates/discord-core/tests/rest_base64.rs` |
| W9b | `rust/crates/discord-core/src/rest/super_properties.rs` | `rust/crates/discord-core/tests/rest_super_properties.rs` |
| W9c | `rust/crates/discord-core/src/rest/identity.rs` | `rust/crates/discord-core/tests/rest_identity.rs` |

**Запрещено всем:** `Cargo.toml`/`Cargo.lock`; любые другие `src/`/`tests/`; `docs/`; `fixtures/`;
`TASKS.md`, `PROCESS.md`, `AGENTS.md`, `waves/` (кроме чтения); `logs/`, `functions/`, `evidence/`,
`reference-oracle/` (в этой волне — только чтение уже снятых выдержек); git-мутации;
`cargo clean`; новые зависимости; сеть. Чужие тест-данные и чужие тесты не редактируются.

## 2. Общие процедуры воркера (strict TDD)

Рабочая папка: `/home/user/orca/project`; прогоны — `cd rust && cargo test --offline …`.
Логи/артефакты: `/tmp/opencode/rw-09/w9a|w9b|w9c/` (каталоги создать).

1. **Заглушка уже в репозитории** (контракт с пустыми телами; тесты падают ассертами, без
   `todo!()`/`panic!()`).
2. **Финальный тест-файл целиком** (кейсы своего раздела). Путь к данным —
   `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/<файл>")`; отсутствие файла — падение
   (не skip). В шапке теста — провенанс: источник (снимок, requestId, md5), команды, счётчики.
3. **Red + фиксация** (шаблон команд; `w8X` → свой):
   ```bash
   cd /home/user/orca/project/rust
   bash -c 'cargo test --offline -p discord-core --test rest_<имя> 2>&1 | tee /tmp/opencode/rw-09/w9X/red.log; echo "# EXIT=${PIPESTATUS[0]}" | tee -a /tmp/opencode/rw-09/w9X/red.log'
   md5sum crates/discord-core/tests/rest_<имя>.rs | tee -a /tmp/opencode/rw-09/w9X/red.log
   cp crates/discord-core/tests/rest_<имя>.rs /tmp/opencode/rw-09/w9X/rest_<имя>-red.rs
   ```
4. **Green:** реализация только в своём продукт-файле; тест после red не менять (иначе red
   заново). Свой таргет ×5; затем `cargo test --offline -p discord-core` (чужие падения — не
   править, отметить); `rustfmt` своих двух файлов; `cargo clippy --offline -p discord-core
   --all-targets -- -D warnings` (чужие замечания — не править); `npm run gate:worker`.
5. **Отчёт** по PROCESS.md §4.2 → `/tmp/opencode/rw-09/w9X/report.md`; финальное сообщение —
   краткое резюме + вопрос(ы), если что-то не сходится с ТЗ.

**Качество:** ошибки не глотать; никаких паник на ожидаемых данных; доккомментарии на русском;
тесты сверяют оракул (файл/вектор/правило), а не реализацию.

## 3. Оракулы и тест-данные

### 3.1 Живой источник

- `reference-oracle/d02-snapshot/capture-run2.snap.jsonl`,
  md5 `abfe14b76cec1e559c3b45522bc44b7a` (сверено с `snapshot-md5.txt`);
- запрос `requestId = "95974.2323"` (GET `…/api/v9/channels/{id}/messages?limit=12`),
  заголовок `X-Super-Properties`: **1084 символа base64**, декодируется в **811 байт** JSON
  (22 ключа, ключи в порядке из захвата); заголовок присутствует в 221 запросе (все
  авторизованные) — `docs/re/rest-d02.md` §6.1–6.2;
- `docs/re/rest-d02.md` md5 `c7e61929677efab6bdac86a05a778fce` — человекочитаемый разбор полей
  (§6.2, таблица 22 полей).

Команды снятия (выполнены координатором, проверены python3 — b64 → JSON → b64 круг сходится):

```bash
SNAP=reference-oracle/d02-snapshot/capture-run2.snap.jsonl
jq -r 'select(.ev=="req" and .requestId=="95974.2323") | (.headers // {}) | to_entries[]
       | select(.key|ascii_downcase=="x-super-properties") | .value' "$SNAP" > rest-super-properties-2323.b64
base64 -d < rest-super-properties-2323.b64 > rest-super-properties-2323.json
jq -r 'to_entries[] | [.key, (.value|type), (.value|tostring)] | @tsv' rest-super-properties-2323.json \
  > rest-super-properties-22.tsv
```

### 3.2 Выдержки в репозитории (read-only)

| Файл | Объём | md5 |
|---|---|---|
| `tests/data/rest-super-properties-2323.b64` | 1084 симв. + `\n` | `b9e8f8430d8b496df50da9419e760a2d` |
| `tests/data/rest-super-properties-2323.json` | 811 байт + `\n` | `90b06ded88a98dd05d3b821f6bedc52b` |
| `tests/data/rest-super-properties-22.tsv` | 22 строки | `95e2f8616043041431fcef2d38d6fac5` |
| `tests/data/rest-base64-vectors.tsv` | 10 строк | `8b5bb8c97841773bd13e0fb9f29e8b51` |

`rest-base64-vectors.tsv`: `имя \t hex(вход) \t base64(python3 b64encode)` — кейсы `empty`, `f`,
`fo`, `foo`, `foob`, `fooba`, `foobar`, `bytes-00-3f` (64 байта), `bytes-00-ff` (256 байт),
`one-00`.

### 3.3 Факты живого JSON (проверены)

- 22 поля в порядке: `os`, `browser`, `release_channel`, `client_version`, `os_version`,
  `os_arch`, `app_arch`, `system_locale`, `has_client_mods`, `client_launch_id`,
  `browser_user_agent`, `browser_version`, `window_manager`, `distro`, `runtime_environment`,
  `display_server`, `client_build_number`, `native_build_number`, `client_event_source`,
  `launch_signature`, `client_heartbeat_session_id`, `client_app_state`.
- Типы: 18 строк, 2 числа (`u32`: 641004, 94186), 1 bool (`false`), 1 null
  (`client_event_source`).
- Особые значения: `distro` = `"\"Arch Linux\""` (кавычки — часть строки, так в `PRETTY_NAME`);
  `window_manager` = `"KDE,unknown"`; base64 заканчивается на `==`.
- JSON компактный (без пробелов), UTF-8.

### 3.4 Замороженный каркас (md5 до старта)

| Файл | md5 |
|---|---|
| `src/rest/base64.rs` | `03ddd6ebecd8ed5c727a732ea2b6e246` |
| `src/rest/super_properties.rs` | `c463b954b8c265f4e9f773e91d6ae153` |
| `src/rest/identity.rs` | `06d80ecf79ec57377c67c76f16f0ef28` |
| `src/rest/mod.rs` | `0eba823bf46c2958cac102dbb42cbb06` |

## 4. Воркер W9a — base64 (RFC 4648)

Файлы: `src/rest/base64.rs`, `tests/rest_base64.rs`. Контракт (не менять):

```rust
pub enum Base64Error {
    InvalidLength(usize),                                 // len % 4 != 0; аргумент — длина в байтах
    InvalidChar { byte: u8, index: usize },               // символ вне A-Za-z0-9+/
    InvalidPadding { index: usize },                      // '=' не в хвосте/больше двух/не по длине
}
pub fn encode(data: &[u8]) -> String;                     // канонический, с '='-паддингом
pub fn decode(text: &str) -> Result<Vec<u8>, Base64Error>;
```

Семантика `decode` (заморожена; порядок проверок: паддинг/символы → длина → данные; см. Поправку
координатора в конце §4):

1. `text.len() % 4 != 0` → `InvalidLength(text.len())`.
2. Хвостовой ряд `=`: 0–2 символа; `=` в неторчащей позиции, либо >2, либо несоответствие
   (`data_chars % 4 == 1`) → `InvalidPadding { index }`, где `index` — позиция первого `=`.
3. Любой символ данных вне алфавита → `InvalidChar { byte, index }` (байт и позиция).
4. Пустая строка → `Ok(vec![])`. Ненулевые хвостовые биты последней группы **игнорируются**
   (декодер не строгий к неканоничности; задокументировать в коде и покрыть тестом).

Кейсы (`B1`–`B5`):

- **B1** `rfc_vectors` — все 10 строк `rest-base64-vectors.tsv`: `decode(b64) == hex-байты`,
  `encode(hex-байты) == b64` (включая 256-байтный вектор).
- **B2** `live_header_roundtrip` — `decode(rest-super-properties-2323.b64)` == байты
  `rest-super-properties-2323.json` (без хвостового `\n`); `encode(json)` == живой b64;
  проверка `==`-паддинга и длины 1084.
- **B3** `errors_are_typed` — `"AA"` → `InvalidLength(2)`; `"A==="` → `InvalidPadding{index:1}`;
  `"===="` → `InvalidPadding{index:0}`; `"=AAA"` → `InvalidPadding{index:0}`;
  `"AA=A"` → `InvalidPadding{index:2}`; `"AA!A"` → `InvalidChar{byte:b'!',index:2}`;
  `"Zm9v\n"` → `InvalidChar{byte:b'\n',index:4}`; `""` → `Ok(vec![])`.
- **B4** `lengths_and_padding` — `encode([])==""`, `encode(b"f")=="Zg=="`, `encode(b"fo")=="Zm8="`,
  `encode(b"foo")=="Zm9v"`; 256-байтный вектор → длина 344 (кратна 4).
- **B5** `noncanonical_trailing_bits` — `"Zh=="` → `Ok(b"f")` (хвостовые биты отброшены) и
  `encode` от неё даёт канонический `"Zg=="`.

**Red-ориентир:** падают B1–B5 (заглушка: `encode` → `""`, `decode` → `Err(InvalidLength(0))`).

**Поправка координатора (2026-10-20, после red W9a):** при расхождении приоритет у кейсов B3 —
проверки символов/паддинга выполняются **раньше** проверки длины (так `"Zm9v\n"` → `InvalidChar
{byte: b'\n', index: 4}`, а не `InvalidLength(5)`); строка «порядок проверок» в начале §4
читается как «паддинг/символы → длина → данные». Порядок задокументирован и в коде W9a.

## 5. Воркер W9b — супер-свойства: JSON и base64

Файлы: `src/rest/super_properties.rs`, `tests/rest_super_properties.rs`. Контракт (не менять):
структура `SuperProperties` (22 поля, типы как в §3.3), `FIELD_COUNT = 22`,
`SuperPropertiesError { Json(String), Base64(Base64Error) }`,
`SuperProperties::{to_json, from_json, to_base64, from_base64}`, `field_names()`.

Правила (заморожены):

- `to_json` — компактный (без пробелов) JSON, порядок полей = `field_names()`; экранирование
  строк по RFC 8259: `"` → `\"`, `\` → `\\`, управляющие `< 0x20` → `\b \f \n \r \t`,
  иначе `\u00XX`; остальной UTF-8 — как есть. `client_event_source: None` → `null`;
  числа `u32` без ведущих нулей.
- `from_json` — объект ровно с этими 22 ключами в любом порядке; пробелы/переводы строк между
  токенами допустимы. Ошибки (`Json(сообщение с позицией)`): пропущенный ключ, неизвестный
  ключ, дубликат, неверный тип (строка↔число↔bool), `null` для не-`client_event_source`,
  число с минусом/дробью/переполнением `u32`, мусор после объекта, незакрытая строка.
- `to_base64` = `base64::encode(to_json().as_bytes())`; `from_base64` = `decode` →
  `String::from_utf8` (не-UTF-8 → `Json(...)`) → `from_json`; ошибка base64 — как
  `SuperPropertiesError::Base64`.

Кейсы (`P1`–`P8`):

- **P1** `fields_are_22_in_order` — `FIELD_COUNT == 22`; `field_names()` построчно равен
  колонке 1 `rest-super-properties-22.tsv`.
- **P2** `parses_live_json_fields` — `from_json(live)` (выдержка) даёт каждое поле по TSV:
  строки дословно (в т.ч. `distro` с кавычками), числа 641004/94186, `has_client_mods=false`,
  `client_event_source=None`.
- **P3** `live_json_roundtrip_bytewise` — `to_json(from_json(live))` == байты выдержки `.json`
  (без хвостового `\n`) — побайтово.
- **P4** `live_base64_roundtrip` — `to_base64(parsed)` == живой b64; `from_base64(live b64)`
  == `parsed`.
- **P5** `escaping_roundtrip` — синтетическая структура со значениями `"`, `\`, перевод строки,
  таб, юникод (`ё`, эмодзи): `from_json(to_json(x)) == x`; в выводе присутствуют `\"`, `\\`,
  `\n`, `\t`; юникод не деградирует.
- **P6** `parser_rejects_bad_input` — пропуск `os`; лишний ключ; дубликат `os`; `"641004"` для
  числа; `-1`; `1.5`; `null` для `browser`; запятая в конце; мусор после `}` — всё `Err(Json)`.
- **P7** `numbers_bounds` — `0` и `4294967295` парсятся; `4294967296` → `Err(Json)`.
- **P8** `base64_layer_errors` — `from_base64("!!!")` → `Err(Base64(InvalidChar…))`;
  `from_base64(base64 от "[]")` → `Err(Json(…))`; `from_base64("AAAAAAAAAAA")` (не `%4`) →
  `Err(Base64(InvalidLength(…)))`.

**Red-ориентир:** падают P1–P8 (заглушка: `to_json` → `""`, `from_json` → `Err`, и т.д.).

## 6. Воркер W9c — честная идентичность и сборка

Файлы: `src/rest/identity.rs`, `tests/rest_identity.rs`. Контракт (не менять): `IdentitySources`
(`env_var`, `read_text`, `random_bytes`, `os`, `arch`), `SystemSources::new()`,
`IdentityConfig` (22 `Option`-поля), `uuid_v4(&dyn IdentitySources)`, `build(&dyn
IdentitySources, &IdentityConfig) -> SuperProperties`.

### 6.1 Политика «свои честные значения» (заморожена; тесты фиксируют)

| Поле | Значение по умолчанию | Источник |
|---|---|---|
| `os` | `Linux` / `Mac OS X` / `Windows` / как есть | `sources.os()` (linux/macos/windows → отображение) |
| `browser` | `discord-lab` | константа (наш клиент; конфигурируемо) |
| `release_channel` | `stable` | константа |
| `client_version` | `env!("CARGO_PKG_VERSION")` (`0.1.0`) | наш крейт |
| `os_version` | `/proc/sys/kernel/osrelease` (trim) | `read_text`; нет → `unknown` |
| `os_arch` | `x86_64→x64`, `aarch64→arm64`, `x86→x86`, иначе как есть | `sources.arch()` |
| `app_arch` | = `os_arch` | — |
| `system_locale` | `LC_ALL` → `LANG` → `unknown`; обрезать после первого `.`/`@`, затем после `_` (`ru_RU.UTF-8` → `ru`, `C` → `C`) | env |
| `has_client_mods` | `false` | константа (у нас нет модов) |
| `client_launch_id` | `uuid_v4(sources)` | random_bytes |
| `browser_user_agent` | `discord-lab/{client_version} ({os}; {os_arch})` | формат |
| `browser_version` | = `client_version` | — |
| `window_manager` | `XDG_CURRENT_DESKTOP` дословно; нет → `unknown` | env |
| `distro` | `/etc/os-release` → значение `PRETTY_NAME` дословно (кавычки сохраняются); нет → `unknown` | `read_text` |
| `runtime_environment` | `native` | константа |
| `display_server` | `XDG_SESSION_TYPE`; нет → `WAYLAND_DISPLAY` → `wayland`; нет → `DISPLAY` → `x11`; нет → `unknown` | env |
| `client_build_number` | `0` (у нас нет сборки; конфигурируемо) | константа |
| `native_build_number` | `0` | константа |
| `client_event_source` | `None` (`null`) | — |
| `launch_signature` | `uuid_v4(sources)` | random_bytes |
| `client_heartbeat_session_id` | `uuid_v4(sources)` | random_bytes |
| `client_app_state` | `focused` | константа (позже обновляет приложение) |

`uuid_v4`: 16 байт из `random_bytes`; версия — `(b[6] & 0x0F) | 0x40`, вариант —
`(b[8] & 0x3F) | 0x80`; формат `8-4-4-4-12`, hex в нижнем регистре. `build`: `config.Some(v)` →
`v`, иначе — правило таблицы. `SystemSources::random_bytes` — чтение `/dev/urandom`
(best-effort: при сбое буфер не меняется — задокументировать).

Кейсы (`I1`–`I8`):

- **I1** `wayland_kde_full_build` — фейковые источники: `os=linux`, `arch=x86_64`,
  env: `LANG=ru_RU.UTF-8`, `XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=KDE`;
  файлы: `/proc/sys/kernel/osrelease` = `6.16.7-arch1-1\n`,
  `/etc/os-release` = `NAME="Arch Linux"\nPRETTY_NAME="Arch Linux"\n`; `random_bytes` отдаёт
  последовательно `0x00..0x0F`, `0x10..0x1F`, `0x20..0x2F`. Ожидание — точная структура:
  `client_launch_id == "00010203-0405-4607-8809-0a0b0c0d0e0f"`,
  `launch_signature == "10111213-1415-4617-9819-1a1b1c1d1e1f"`,
  `client_heartbeat_session_id == "20212223-2425-4627-a829-2a2b2c2d2e2f"`,
  `browser_user_agent == "discord-lab/0.1.0 (Linux; x64)"`, `distro == "\"Arch Linux\""` и все
  остальные поля по §6.1.
- **I2** `display_server_fallbacks` — без `XDG_SESSION_TYPE` при `WAYLAND_DISPLAY` → `wayland`;
  только `DISPLAY` → `x11`; ничего → `unknown`.
- **I3** `locale_parsing` — `ru_RU.UTF-8` → `ru`; `C` → `C`; `en_US@euro` → `en`; `LC_ALL`
  приоритетнее `LANG`; ничего → `unknown`.
- **I4** `os_arch_mapping` — (`linux`,`x86_64`)→(`Linux`,`x64`); (`macos`,`aarch64`)→
  (`Mac OS X`,`arm64`); (`windows`,`x86`)→(`Windows`,`x86`); (`freebsd`,`riscv64`)→
  (`freebsd`,`riscv64`).
- **I5** `distro_sources` — `PRETTY_NAME` дословно (кавычки сохраняются); нет файла → `unknown`;
  файл без `PRETTY_NAME` → `unknown`.
- **I6** `uuid_v4_shape` — точные строки из I1; формат/регистр (только `0-9a-f`); два вызова
  с разными байтами различны.
- **I7** `config_overrides_win` — конфиг со всеми 22 `Some(...)` (включая `u32` и
  `client_event_source = Some("probe")`) → `build` возвращает ровно их.
- **I8** `integration_base64_headers` — `build(FakeSources, default)` → `to_base64` →
  `from_base64` == исходная структура; `AuthHeaderValues` (из `rest::headers`) с
  `super_properties = sp.to_base64()` собирается без ошибок (6 пар, значение непустое).

**Red-ориентир:** падают I1–I8 (заглушка: `build` → `SuperProperties::default()`,
`uuid_v4` → `""`).

## 7. После воркеров — агрегация и гейты (координатор)

1. `cargo test --offline --workspace` (от RW-08 — 137 тестов + новые); clippy `-D warnings`;
   `cargo fmt --check`.
2. md5-аудит границ: `Cargo.*` без diff; соседи и выдержки целы; golden не тронут.
3. `npm run gate:worker`; затем этапы W9d и W9r (ниже).

## 8. Этап W9d — рефакторинг-воркер (бриф)

После зелёного: не-поведенческий проход по 6 файлам волны. Можно: доккомментарии, локальные
переименования, устранение дублирования, мелкие хелперы тестов при неизменных ассертах и
md5 тест-файлов. Нельзя: менять публичные сигнатуры/поля, тест-данные, трогать чужие файлы.
Обязательно: `cargo test --offline -p discord-core` ×3, clippy, fmt; в отчёте — diff и md5.

## 9. Этап W9r — независимое ревью (бриф)

1. Пересчитать выдержки **своими** командами из снимка (jq/base64/python3) и md5 снимка;
   сверить побайтово с `tests/data/*`.
2. Воспроизвести red заглушками (B/P/I), сверить распределения и md5 тестов в red-логах.
3. Границы, `Cargo.*`, golden, соседи.
4. Мутации (≥12): порядок/имя поля; `=`-паддинг; приём мусора; `\u`-экранирование; границы
   `u32`; версия/вариант UUID; парсинг локали; fallback дисплея; `PRETTY_NAME`; приоритет
   конфига; `to_base64`; лишний ключ.
5. §4.4-чек-лист + отчёт; отдельно — оценка политики §6.1 (честность значений, полнота).

## 10. DoD волны

- 21 кейс (B — 5, P — 8, I — 8), все зелёные ×5; `discord-core`/воркспейс без регрессий;
  clippy/fmt чисто; `gate:worker` зелёный; red-first по каждому воркеру (лог `# EXIT=`, md5,
  red-копия).
- Новых зависимостей нет; сеть не использовалась; golden/снимок не изменялись.
- **Вне волны:** отправка заголовка в живых запросах (после решения по транспорту, см.
  `waves/TRANSPORT.md`), 429/Retry-After (P1-09), интеграция в живой REST-пайплайн (P2-06).

## 11. Артефакты

- Воркеры: `/tmp/opencode/rw-09/w9a|w9b|w9c/{red.log,rest_*-red.rs,report.md}`.
- Координатор после приёмки: `waves/RW-09/{TZ.md,report.md,red/,reports/,final/}`,
  `waves/reviews/review-rw-09.md`, события в `waves/db.jsonl`, обновление `waves/STATE.md`.
