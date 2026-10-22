# B02 / P1-01 — ETF full (contract)

Дата: 2026-10-21. Карточка: **P1-01** (M). Зависимость: P0-02 принят. Файлы:
`rust/crates/discord-codec/src/etf.rs`, `lib.rs`, `tests/etf_full.rs`. Без новых зависимостей,
сети, live и изменений golden.

## 1. Цель

Довести ETF-кодек от принятого прототипа до полного контракта: все 11 фактических тегов +
6 поддержанных-не-встреченных, big-int со знаком до цифр, map/list/tuple вложенность,
проверяемый энкодер без молчаливого усечения длин, fuzz без паник/OOM. Декодер уже покрыт
RW-01; P1-01 добавляет **checked encode** и границы, не меняя канонические байты для
обычных значений.

## 2. Публичный контракт

```rust
pub enum EtfEncodeError {
    DepthLimit { limit: usize },
    BinaryTooLong { len: usize },
    AtomTooLong { len: usize },
    ContainerTooLong { kind: &'static str, len: usize, max: usize },
}
pub fn encode_checked(value: &EtfValue) -> Result<Vec<u8>, EtfEncodeError>;
pub fn encode_term_checked(value: &EtfValue) -> Result<Vec<u8>, EtfEncodeError>;
pub fn encode(value: &EtfValue) -> Vec<u8>; // совместимость; panic только на программно
                                            // невозможном превышении wire-лимита
pub fn encode_term(value: &EtfValue) -> Vec<u8>;
```

`encode`/`encode_term` сохраняют текущий API и байты; `encode_checked` — путь для внешних
данных. Лимиты: binary `u32::MAX`, atom `u16::MAX`, tuple/list/map count `u32::MAX`, глубина
`MAX_DEPTH=64` (как у декодера). Ошибка не маскируется и не усекает длину.

Декодер: без изменений публичной модели; `EtfValue::Int(i64)` достаточно для подтверждённых
снежинок; значения > i64 дают `BigIntOverflow` (не выдумываем arbitrary precision).
`LARGE_BIG_EXT`/`LARGE_TUPLE_EXT`/UTF-8 атомы/`FLOAT_EXT` уже поддержаны.

## 3. Тесты (6)

| ID | Проверка |
|---|---|
| e01 | `encode_checked` каноничен для всех вариантов и совпадает с `encode` |
| e02 | nested map/list/tuple до глубины 64 round-trip |
| e03 | глубина 65 → `EtfEncodeError::DepthLimit`, декодер → `DepthLimit` |
| e04 | big-int i64::MIN/MAX и 8-байтовые границы |
| e05 | atom > `u16` → `AtomTooLong` без усечения; binary/container лимиты проверяются checked-путём |
| e06 | fuzz 50 000 буферов + все однобайтовые + усечённые префиксы, без паник/OOM |

## 4. Мутации (6)

M1 снять проверку глубины в encoder; M2 binary length truncate; M3 atom length truncate;
M4 map count truncate; M5 big-int sign after digits; M6 decoder depth off-by-one. Все —
red-first, restore по md5 и `touch`.
