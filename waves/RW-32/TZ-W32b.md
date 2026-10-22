# TZ W32b — P6-02b D-Bus hints `a{sv}` для уведомлений

Волна: RW-32. Дата: 2026-10-22. Крейт: `discord-platform`, поверх принятого P6-02 (RW-30).
Сеть/live не используются.

## 1. Замороженный контракт

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency { Low, Normal, Critical }

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub urgency: Option<Urgency>,
    pub category: Option<String>,
    pub desktop_entry: Option<String>,
    pub resident: Option<bool>,
}

impl notifications::Notifier {
    /// Notify с hints (`a{sv}`); `notify()` остаётся эквивалентом `Hints::default()`.
    pub fn notify_with_hints(
        &mut self,
        notification: &Notification,
        hints: &Hints,
    ) -> Result<u32, DbusError>;
}
```

Инварианты:

- Сигнатура вызова остаётся `susssasa{sv}i`; меняется только содержимое `a{sv}`.
- Пустые `Hints` → dict с 0 записей (не `null`), поведение `notify` не меняется.
- `urgency` → ключ `"urgency"`, вариант `y` (Low=0, Normal=1, Critical=2).
- `category` → `"category"`, вариант `s`; `desktop_entry` → `"desktop-entry"`, `s`;
  `resident` → `"resident"`, `b`.
- Пустые строки в `category`/`desktop_entry` не отправляются (пропускаются).
- Ключи уникальны; порядок записи детерминирован: urgency, category, desktop-entry, resident.
- Маршалинг `a{sv}`: выравнивание массива 4, элемента dict-entry 8, `v` = signature + значение;
  ошибок/паник на длинных строках и Unicode нет.
- `unsafe` не добавляется; файлы `dbus/{mod,auth}.rs` не трогаются, `message.rs` — только
  аддитивные функции (существующее поведение не меняется).

## 2. Оракул

Допускается отдельная копия мока (`waves/RW-32/dbus/mock_hints.py`), чтобы не менять формат
журнала RW-30 (решение TL по ревью). libdbus-мок читает hints: журнал
`Notify` дополняется ключами/значениями. `fixtures/dbus/hints.json` — эталонные наборы
(пусто, urgency, category, desktop-entry+resident, всё вместе) и ожидаемые типы вариантов.
Дополнительно — байтовый round-trip через собственный парсер и libdbus `marshal/demarshal`.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| q01 | `notify` без hints | dict 0 записей, id получен |
| q02 | urgency Low/Normal/Critical | байты 0/1/2 в моке |
| q03 | category | строка в моке |
| q04 | desktop-entry + resident | строка + bool |
| q05 | все hints вместе | 4 записи, типы верные |
| q06 | пустые строки category/desktop_entry | пропущены |
| q07 | Unicode/длинная строка (4 КиБ) | без паники, доставлено |
| q08 | signature/байты `a{sv}` | round-trip + libdbus |

Бюджет: 7–9 тестов; мутации 4–6 (тип варианта, ключ, пропуск пустой строки, порядок/наличие).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w32b cargo test --offline -p discord-platform --test notifications_contract --test dbus_contract
```

## 5. DoD

red → green → мутации (`--nocapture`, SKIP=0, restore по md5) → отчёт `waves/RW-32/REPORT-W32b.md`.
