# ТЗ W22a — P3-15: i18n runtime base

Волна: RW-22 / B03. Владелец: W22a. Работать в `/home/user/orca/project`, не коммитить.

## 0. Обязательное чтение

`AGENTS.md`, `PROCESS.md` §4, `waves/RW-22/WAVE-PLAN.md`, `docs/re/i18n-deep.md` §1–3, §7,
`docs/rust-client/PLAN.md` (P3-15), `docs/rust-client/TEST-PLAN.md` (T-I18N-01…03),
`rust/crates/discord-json/src/lib.rs`, `rust/crates/discord-i18n/src/lib.rs`.

Твои файлы — **только** `rust/crates/discord-i18n/**` (включая его `Cargo.toml`). Не трогай
`Cargo.toml` workspace, `Cargo.lock`, другие крейты, доки, `waves/*`. Нужен только
`discord-json = { workspace = true }`.

## 1. Метод

Strict TDD. Сначала тесты (сейчас в крейте только тривиальный lib-тест), зафиксируй red-лог:

```bash
cd /home/user/orca/project/rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-i18n \
  2>&1 | tee ../waves/RW-22/red-w22a.log
```

Red должен быть содержательным (compile-red по отсутствующим API или behavior-red, exit 101).
Затем минимальная реализация до green, повторный лог — `green-w22a.log`. Не правь тесты под код.

## 2. Публичный контракт

Можно добавить внутренние модули, но корень `discord_i18n` должен экспортировать:

```rust
pub const MESSAGE_KEY_LEN: usize = 6;
pub const TOTAL_LOCALES: usize = 36;
pub const ENABLED_LOCALES: usize = 31;
pub const DISABLED_LOCALES: [&str; 5];
pub const SUPPORTED_LOCALES: [&str; 36];
pub const DEFAULT_LOCALE: &str = "en-US";
pub fn xxh64(input: &[u8], seed: u64) -> u64;
pub fn message_key(id: &str) -> String;
pub fn resolve_locale(requested: &str, available: &[&str]) -> String;
```

- `message_key`: `xxh64(id.as_bytes(), 0)` → 8 байт **LE** → стандартный base64 → первые 6
  символов. Проверочные векторы: `ru → +irF5O`, `en-US → +dXJII`, `AKA → l1QVfj`,
  `OK → TyCVIq`, `PS4 → fJyyQn`, `PS5 → oElqN5`. Дополнительно сверь с
  `reference-oracle/ru-catalog.json`: `hPBScv` — реальный ключ, его значение
  «Закрыть всплывающее окно»; обратный хеш от id проверять не нужно.
- `xxh64` — реализация xxHash64 (seed, little-endian чтение, avalanche); без `unsafe`.
- Список 36 локалей — из `docs/re/i18n-deep.md` §7 / `reference-oracle/inline/lang-list.json`;
  отключённые ровно `["pt-PT","sk","he","ar","id"]`.
- `resolve_locale`: exact → case-insensitive → язык (`ru-RU`→`ru`) → default → `en-US`;
  отключённая/неизвестная не выбирается; пустой requested → default. Функция возвращает код
  из `available`, иначе `DEFAULT_LOCALE`; не паникует.

## 3. AST и каталог

`Catalog::parse(&discord_json::JsonValue) -> Result<Catalog, AstError>` для keyless AST:

- `0 Literal` — `[0,"text"]`;
- `1 Argument` — `[1,"name"]`;
- `2 Number` — `[2,"name"]` (опционально третий skeleton);
- `3 Date` / `4 Time` — `[3,"name","medium"]` (стиль опционален);
- `5 Select` — `[5,"var",{options}]`;
- `6 Plural` — `[6,"var",{options},offset,"cardinal"|"ordinal"]`;
- `7 Pound` — `[7]`;
- `8 Tag` — `[8,"$b",children,control]` (control опционален/может быть null/массив).

Требования: неизвестный тип/форма — типизированная ошибка с путём/индексом, не паника;
recursion depth ограничить; duplicate option keys отклонить; literal/argument имена не терять.
`Catalog::get(key)`, `Catalog::len`, `Catalog::is_empty`; каталог — `BTreeMap<String, Ast>`.

## 4. Translate

```rust
pub enum Value { Str(String), Int(i64), Float(f64) }
pub struct I18n { /* locale -> Catalog, current, default */ }
pub fn new(default_locale: &str) -> I18n;
pub fn insert_catalog(&mut self, locale: &str, catalog: Catalog) -> Result<(), LocaleError>;
pub fn set_locale(&mut self, locale: &str) -> Result<(), LocaleError>;
pub fn translate(&self, key: &str, args: &BTreeMap<String, Value>) -> Result<String, TranslateError>;
```

- fallback: current → default (`en-US`) → `TranslateError::MissingMessage` (не пустая строка
  молча; диагностика содержит key/locale, но не секреты).
- `Argument` без значения — `TranslateError::MissingArgument{name}`.
- `Plural`: точный `=N` (включая `=0`, `=-1`), иначе категория CLDR для `ru`/`en`; `#` —
  форматированное число (ru — пробел/NBSP-группировка, en — запятая); `offset` вычитать;
  `ordinal` поддержать для en (1st/2nd/3rd/4th), ru cardinal; нет ветки — ошибка.
- `Select`: точное сравнение, `other` fallback; нет — ошибка.
- `Tag`: `$b→**…**`, `$i→*…*`, `$del→~~…~~`, `$code→`…``, `$p→…\n\n`, `$link` → `[text](url)`
  (url из control/аргумента, если подтверждён; иначе только children, без выдуманного URL);
  неизвестный `$tag` — ошибка; hook-тег без `$` рендерит детей.
- Number: базовое `Int`/`Float`; Date/Time — типизированная `UnsupportedNode` (P7-01), не паника.

## 5. Базовый rich-markdown

Добавь безопасный парсер/рендер сообщений (не HTML): `MarkdownNode` + `parse_markdown` +
`render_markdown` (или эквивалент). Минимум: `**bold**`, `*em*`, `_em_`, `__underline__`,
`~~strike~~`, `||spoiler||`, `` `code` ``, fenced code с языком, `[t](url)`, escape `\*`,
переносы строк. Ссылки: разрешать только `http`, `https`, `mailto`; `javascript:`, `data:`,
`vbscript:` — не превращать в ссылку (текст остаётся). HTML-теги — литералы, не исполнять.
Тесты: вложенность, незакрытые маркеры, экранирование, XSS-попытки, пустой вход.

## 6. Обязательные тесты (минимум)

1. hash: 6 векторов + `message_key` длина/алфавит + 81 сверка xxh64 с `xxhsum`? Если `xxhsum`
   недоступен — пропустить с причиной, не выдумывать. Достаточно 6 эталонов + known-answer
   для пустой строки.
2. locale: 36/31/5, exact/case/base/fallback/disabled.
3. catalog: реальные `ru-catalog.json` и `en-catalog.json` из `reference-oracle/` (skip если
   нет), 23 836/23 903 ключей, `hPBScv`, `323362` AST с `$link`, plural `r/EUap`.
4. translate: literal, argument, fallback requested→en, missing arg, missing message.
5. plural ru: 1/2/5/11/21/22/101 для `r/EUap`; `=0`; `#`; offset; en one/other.
6. rich tags: `$b`, `$i`, `$del`, `$code`, `$p`, `$link`; unknown tag error; hook children.
7. markdown: strong/em/underline/strike/spoiler/code/fence/link/escape/XSS; no panic on 10k
   truncations.
8. `Debug`/ошибки не содержат значения каталога целиком (no accidental huge dump).

## 7. Отчёт

Верни: red-команду+exit, green-команду+exit, список файлов, число тестов, риски/unknowns,
что сознательно отложено в P7-01. Не запускай `cargo test --workspace` (это integration lane).
