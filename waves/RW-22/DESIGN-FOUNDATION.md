# RW-22 — foundation `discord-json`

Дата: 2026-10-21. Статус: принято вместе с B03.

## 1. Зачем свой парсер

P3-15 (каталоги i18n), P4-01 (RTC-кадры) и P7-04 (D-02 oracle) работают с недоверенным
JSON. Внешняя зависимость `serde_json` не добавлялась: по правилам волны новые зависимости
запрещены, а `Cargo.lock` уже содержит только согласованный набор. Поэтому foundation —
небольшой строгий parser/writer в `rust/crates/discord-json`, без `unsafe` и без сети.

## 2. Контракт

```rust
pub const MAX_DEPTH: usize = 128;
pub const MAX_NODES: usize = 4_000_000;

pub enum JsonValue {
    Null, Bool(bool), Number(JsonNumber), String(String),
    Array(Vec<JsonValue>), Object(BTreeMap<String, JsonValue>),
}
pub struct JsonNumber(String); // исходный лексический вид
pub struct JsonError { pub offset: usize, pub kind: JsonErrorKind }
pub fn parse(input: &str) -> Result<JsonValue, JsonError>;
pub fn to_json_string(value: &JsonValue) -> String;
```

`JsonNumber` хранит исходную строку, чтобы не терять `u64`/точность; вызывающий код
явно выбирает `as_u64`/`as_i64`/`as_f64`. `BTreeMap` даёт детерминированный порядок
ключей при записи и стабильные тесты.

## 3. Инварианты безопасности

- RFC 8259: только допустимые литералы, строки, escapes, числа; trailing data — ошибка.
- duplicate object keys — ошибка (`DuplicateKey`), а не last-wins.
- control-символы внутри строки — ошибка; `\uD83D\uDE00` собирается в один scalar,
  одиночный surrogate — ошибка.
- глубина > `MAX_DEPTH` и число значений > `MAX_NODES` — типизированная ошибка.
- ошибка содержит только байтовое смещение и категорию, но не входные данные; это важно
  для token/secret в голосовых кадрах.
- parser не паникует на усечённом/случайном входе; truncation-тесты гоняют все префиксы
  реальных каталогов/кадров.
- writer экранирует control-символы и кавычки, сохраняет Unicode; round-trip проверен на
  реальных кадрах.

## 4. Границы

- Это не JSON Schema и не «мягкий» парсер: неизвестные поля сохраняются как данные, но
  duplicate/битые значения отклоняются.
- Нет чисел с плавающей точкой в writer: числа пишутся из `JsonNumber` как есть; `NaN`/
  `Infinity` в JSON не существуют.
- Лимиты — защита от DoS, а не бюджет памяти приложения; 4M узлов — верхняя граница,
  а не целевой размер.
- P7-01 (полный i18n) и P4-02 (UDP/медиа) могут добавить свои модели поверх foundation,
  не меняя parser.

## 5. Доказательства

- `rust/crates/discord-json/tests/json_contract.rs` — 11 тестов: грамматика, duplicate,
  surrogate, depth/node, ошибки без payload, writer round-trip, реальные ru/en каталоги,
  реальные RTC-кадры, truncation.
- Реальные каталоги: ru 23 836, en 23 903 ключа.
- Реальные кадры: `rtc1-frames.jsonl`/`rtc2-frames.jsonl` (skip-aware при отсутствии lab).
- Мутации foundation: `mutations/m-json-01.log` … `m-json-03.log` — 3/3 caught.
