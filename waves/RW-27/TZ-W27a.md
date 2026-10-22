# ТЗ W27a — P7-01b ListFormat (RW-27 / B09)

Роль/место: CLIENT-1, файлы — только твои.
Batch: RW-27; зависимости: P7-01 accepted, `discord-i18n` 33 теста зелёные.

## Цель

Добавить `Intl.ListFormat`-совместимый headless-форматтер списков для 31 включённой локали.
Данные CLDR/ICU уже сгенерированы и заморожены; реализация не должна их выдумывать.

## Файлы (только эти)

- `rust/crates/discord-i18n/src/list.rs` (новый)
- `rust/crates/discord-i18n/src/list_data.rs` (скопировать из `waves/RW-27/generated/list_data.rs`)
- `rust/crates/discord-i18n/src/lib.rs` (только добавить `pub mod list;` и `mod list_data;` —
  `list_data` приватный, наружу не экспортируется)
- `rust/crates/discord-i18n/tests/list_contract.rs` (новый)

Запрещено: `Cargo.toml`, другие модули/тесты i18n, `format.rs`/`format_data.rs`/`plural.rs`,
доки, golden, сеть, коммиты, `unsafe`.

## Контракт (заморожен)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListStyle { Long, Short, Narrow }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListType { Conjunction, Disjunction, Unit }

pub fn format_list(locale: &str, items: &[&str], style: ListStyle, kind: ListType) -> String;
```

- Резолв локали — через `crate::locale::{canonical_locale, is_enabled}`; неизвестная или
  выключенная (`he`, `ar`, `id`, `pt-PT`, `sk`) → `en-US`.
- `items.is_empty()` → `String::new()`; 1 элемент → `items[0].to_owned()`.
- 2 элемента → `a + pair + b`.
- 3+ → `a + start + b`, затем для `items[2..len-1]` добавить `middle + item`, затем
  `end + last`.
- Поиск паттерна: линейно по `LIST_PATTERNS`; ключ `(locale, style.as_str(), kind.as_str())`.
  Если строки нет — fallback на `en-US` с тем же style/kind; если и его нет — вернуть
  элементы через `", "` (не паниковать).
- `list_data.rs` — сгенерированный файл, **не редактировать вручную**; шапка уже содержит
  провенанс. Скопируй его байт-в-байт.

## Кейсы (red-first)

1. `en-US` long conjunction: 0/1/2/3/5 → `""`, `"A"`, `"A and B"`, `"A, B, and C"`,
   `"A, B, C, D, and E"`.
2. `ru` long conjunction/disjunction: `"A, B и C"`, `"A, B или C"`.
3. `zh-CN` long unit, 4 элемента → `"ABCD"` (пустые разделители).
4. Все 1620 кейсов из `waves/RW-27/fixtures/list-oracle.json` — загрузить `include_str!`
   и сравнить без serde: файл простой JSON; допустимо распарсить `discord_json` (уже
   зависимость воркспейса? нет — у i18n нет зависимости; тогда выбери 200–300
   репрезентативных кейсов, захардкоженных из фикстуры, и добавь тест-матрицу по
   локалям/стилям/типам). Не добавляй зависимостей.
5. Неизвестная/выключенная локаль (`xx`, `he`) → en-US.
6. short/narrow, disjunction/unit — по крайней мере по 3 локали (en-US, de, ja).

Минимум 8 тестов, бюджет 8–12. Каждый тест — поведенческий, без `unwrap()` продукта.

## Шаги

1. Скопируй `generated/list_data.rs` в `src/list_data.rs`.
2. Напиши `tests/list_contract.rs` **до** реализации; зафиксируй red-лог с exit code
   (compile-red допустим: `format_list` ещё нет).
3. Реализуй `list.rs` минимально; добавь модули в `lib.rs`.
4. `cargo fmt -p discord-i18n`, targeted green:
   `CARGO_TARGET_DIR=/tmp/opencode/target-w27a cargo test --offline -p discord-i18n --test list_contract`.
5. Мутации 5–8 (правка `list.rs`, restore по md5): pair→start, start↔end, middle→start,
   fallback locale→requested, 3+ ветка без `middle`, пустой список→`" "`.
6. Отчёт по PROCESS §4.2 в `waves/RW-27/workers/W27a/REPORT.md` + red/green логи + md5.

## DoD

- red-лог exit 101; green targeted; 8–12 тестов;
- мутации 5–8 caught, продукт восстановлен по md5;
- `list_data.rs` побайтово равен `waves/RW-27/generated/list_data.rs` (сверь `md5sum`);
- полный gate не запускать — это batch-лейн TL-2.
