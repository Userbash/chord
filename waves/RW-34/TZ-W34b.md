# TZ W34b — P6-02d расширенные hints уведомлений (headless)

Волна: RW-34. Дата: 2026-10-23. Крейт: `discord-platform`, поверх P6-02b (RW-32).

## 1. Замороженный контракт

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub urgency: Option<Urgency>,
    pub category: Option<String>,
    pub desktop_entry: Option<String>,
    pub image_path: Option<String>,
    pub sound_name: Option<String>,
    pub resident: Option<bool>,
    pub transient: Option<bool>,
    pub suppress_sound: Option<bool>,
}
```

Инварианты:

- Ключи freedesktop: `urgency` (`y`), `category` (`s`), `desktop-entry` (`s`),
  `image-path` (`s`), `sound-name` (`s`), `resident` (`b`), `transient` (`b`),
  `suppress-sound` (`b`).
- Порядок записи фиксирован: urgency, category, desktop-entry, image-path, sound-name,
  resident, transient, suppress-sound.
- Пустые строки (`category`, `desktop_entry`, `image_path`, `sound_name`) пропускаются;
  `Some(false)` для boolean — запись с `false` (не пропуск).
- Сигнатура `Notify` — `susssasa{sv}i`, не меняется; `notify()` = `Hints::default()`.
- Без `unsafe`; изменения аддитивные, существующие поля/поведение не ломаются.

## 2. Оракул

libdbus-мок (`waves/RW-34/dbus/mock_hints2.py`) журналирует `ключ:сигнатура:значение`;
`fixtures/dbus/hints-ext.json` — байты `Notify` от libgio и снимок демаршалинга.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| e01 | все 8 hints | 8 записей, типы y/s/b |
| e02 | image-path + sound-name | строки в моке |
| e03 | transient/suppress-sound | bool, включая false |
| e04 | пустые строки | пропущены |
| e05 | порядок ключей | фиксированный |
| e06 | Unicode/4 КиБ | без паники |
| e07 | байты a{sv} | round-trip + libgio |

Бюджет: 7–9 тестов; мутации 4–6 (ключ, тип, пропуск пустой, порядок).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w34b cargo test --offline -p discord-platform --test notifications_contract
```

## 5. DoD

red → green → мутации → отчёт `waves/RW-34/REPORT-W34b.md`.
