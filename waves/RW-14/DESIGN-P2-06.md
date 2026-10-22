# DESIGN P2-06 — in-memory history и пагинация (RW-14)

Статус: заморожен 2026-10-20. Границы: память, без SQLite/UI/сети/новых зависимостей.
`waves/RW-14/RESEARCH.md` фиксирует подтверждённое и неизвестное; этот документ — контракт.

## 1. Scope

Реализуется **memory-only ядро**:

- per-channel окно сообщений с ограничением `channel_window`;
- слияние REST-страниц (уже типизированных) и live-сообщений;
- пагинация `latest`/`before`/`after`/`around`, курсор и generation-токен;
- детерминированные селекторы.

Не реализуется: REST-JSON, сеть, SQLite, перезапуск, таймауты/отмена (адаптер), tombstones
(`MESSAGE_DELETE` нет среди подтверждённых типов), UI. `State.messages` не меняется и остаётся
неограниченным; history — отдельный ограниченный кэш.

## 2. Публичные имена

```rust
pub mod history;

pub use history::{
    Cursor, History, HistoryConfig, HistoryDiagnostics, HistoryError, LiveOutcome,
    MessagePage, PageApply, PageError, PageRequest, PageToken,
};
```

`history::{types, pagination, selectors}` — публичные модули для тестов; `ChannelHistory`,
`WindowDirection` — `pub(crate)`.

## 3. История и окно

```rust
pub struct HistoryConfig { pub channel_window: usize, pub page_limit: u16 }
impl Default for HistoryConfig { fn default() -> Self { Self { channel_window: 100, page_limit: 50 } } }

pub struct History { /* config, BTreeMap<u64, ChannelHistory>, diagnostics */ }

impl History {
    pub fn new(config: HistoryConfig) -> Result<Self, HistoryError>;
    pub fn config(&self) -> HistoryConfig;
    pub fn diagnostics(&self) -> &HistoryDiagnostics;
    pub fn apply_live(&mut self, message: Message) -> LiveOutcome;
}
```

`History::new`: `channel_window == 0` → `HistoryError::ZeroWindow`; `page_limit == 0` →
`HistoryError::ZeroPageLimit`. `Default` создаёт валидную конфигурацию без паники.

`ChannelHistory`:

```rust
pub(crate) struct ChannelHistory {
    pub(crate) messages: BTreeMap<u64, Message>,
    pub(crate) has_older: Option<bool>,
    pub(crate) has_newer: Option<bool>,
    pub(crate) direction: WindowDirection,
    pub(crate) anchor: Option<u64>,
    pub(crate) generation: u64,
}
pub(crate) enum WindowDirection { Latest, Before, After, Around }
```

Правила `apply_live`:

1. Канал создаётся при необходимости (`Direction::Latest` по умолчанию).
2. Если id уже есть — заменить (Live last-wins), вернуть `Replaced`; окно не растёт.
3. Если направление `Before`/`Around` и id больше текущего `newest` — **не вставлять**,
   `has_newer = Some(true)`, `diagnostics.deferred_live += 1`, вернуть `Deferred`.
4. Иначе вставить; при переполнении вытеснить по направлению (см. §5); вернуть `Inserted`.
   Для `Latest`/`After` `has_newer = Some(false)` ставится только если live-id действительно новее
   текущего `newest`; более старая live-запись не должна ложно объявлять «новее нет».

`HistoryDiagnostics` (вектор `duplicates` ограничен 256 id, чтобы диагностика не росла
без границ; `PageApply.duplicates` считает повторы текущей страницы):

```rust
pub struct HistoryDiagnostics {
    pub duplicates: Vec<u64>,      // id, встреченный повторно в одной странице (cap 256)
    pub evicted: usize,            // всего вытеснено
    pub stale_pages: usize,        // отклонённые токены
    pub channel_mismatches: usize, // страницы с чужим channel_id
    pub deferred_live: usize,      // live, отложенные при прокрутке вверх
}
```

## 4. Страницы и курсоры

```rust
pub enum Cursor { Latest, Before(u64), After(u64), Around(u64) }
pub struct PageRequest { pub channel_id: u64, pub cursor: Cursor, pub limit: u16 }

pub struct MessagePage {
    pub messages: Vec<Message>,
    pub has_older: Option<bool>,
    pub has_newer: Option<bool>,
}
pub struct PageToken { /* channel_id, generation, cursor, limit; поля приватны */ }
pub struct PageApply { pub inserted: usize, pub replaced: usize, pub evicted: usize, pub duplicates: usize }
```

`PageRequest`:

- `new(channel_id, cursor, limit)` — без валидации;
- `from_effect(effect: &state::Effect, max_limit: u16) -> Result<Option<PageRequest>, PageError>`:
  `FetchMessages { before: None }` → `Latest`; `Some(id)` → `Before(id)`; другой эффект → `Ok(None)`;
  `limit == 0` → `ZeroLimit`; `limit > max_limit` → `LimitTooLarge { limit, max }`.
- `query()`: `Latest` → `limit=N`; `Before(id)` → `before=id&limit=N`;
  `Around(id)` → `limit=N&around=id`; `After(id)` → `after=id&limit=N`.
  Порядок первых трёх форм совпадает с capture; `after` — только контракт (0 наблюдений).
- `validate(max_limit)` — те же ошибки; `History::begin_page` вызывает её.

`History::begin_page(&mut self, request) -> Result<PageToken, PageError>`:

- валидирует `limit` по `config.page_limit`;
- создаёт канал, увеличивает `generation`, запоминает `cursor`/`limit` в токене;
- токен нельзя подделать снаружи (поля приватны).

`History::apply_page(&mut self, token, page) -> Result<PageApply, PageError>`:

1. Если `token.generation != channel.generation` → `diagnostics.stale_pages += 1`,
   `Err(StaleToken { expected: channel.generation, got: token.generation })`, данные не меняются.
2. Если любой `message.channel_id != token.channel_id` → `channel_mismatches += 1`,
   `Err(ChannelMismatch { expected, actual, message_id })`; **атомарно**, без мутации.
3. Направление = `token.cursor`; `Around(id)` задаёт `anchor=Some(id)`, остальные — `None`.
4. Дедуп по id внутри страницы: last-wins, каждый повторный id → `duplicates` в диагностике
   и `PageApply.duplicates`.
5. Слияние: existing заменяется (`replaced += 1`), новый вставляется (`inserted += 1`);
   страница не удаляет отсутствующие в ней сообщения. Для приоритета Live > Page канал
   хранит `live_ids` только для сообщений текущего окна; страница не перезаписывает такой id.
   При вытеснении id удаляется из `live_ids`, чтобы множество не росло без границ.
6. `has_older`/`has_newer`: обновлять только `Some`; `None` не затирает уже известное значение.
7. Trim по направлению; `evicted` суммируется в диагностику.
8. Успешный `apply_page` увеличивает `generation`, поэтому повтор того же токена — stale.
   Ошибка generation/channel не увеличивает generation.

Пустая страница валидна: сообщения не меняются, `Some`-границы обновляются. Короткая страница
с `None` не означает конец истории.

## 5. Вытеснение

| Направление | Оставляем |
|---|---|
| `Latest`, `After` | наибольшие id (новые) |
| `Before` | наименьшие id (старые) |
| `Around(anchor)` | ближайшие к anchor; tie — меньший id (старая сторона) |

`Around` сортирует кандидатов по `(abs_diff(id, anchor), id)` и оставляет первые N, затем
восстанавливает возрастание id. `channel_window` — на канал; глобального cap нет.

## 6. Селекторы (только чтение)

```rust
impl History {
    pub fn channel_count(&self) -> usize;
    pub fn total_len(&self) -> usize;
    pub fn len(&self, channel_id: u64) -> usize;
    pub fn is_empty(&self, channel_id: u64) -> bool;
    pub fn messages(&self, channel_id: u64) -> Vec<&Message>;       // ascending id
    pub fn recent(&self, channel_id: u64, limit: usize) -> Vec<&Message>; // newest first
    pub fn message(&self, channel_id: u64, message_id: u64) -> Option<&Message>;
    pub fn oldest(&self, channel_id: u64) -> Option<&Message>;
    pub fn newest(&self, channel_id: u64) -> Option<&Message>;
    pub fn has_older(&self, channel_id: u64) -> Option<bool>;
    pub fn has_newer(&self, channel_id: u64) -> Option<bool>;
    pub fn range(&self, channel_id: u64, from: u64, to: u64) -> Vec<&Message>; // inclusive
    pub fn window_around(&self, channel_id: u64, anchor: u64, limit: usize) -> Vec<&Message>;
}
```

Неизвестный канал: пустые срезы/`None`; `range` при `from > to` пуст. `recent(...,0)` и
`window_around(...,0)` пусты. `window_around` выбирает по `(abs_diff, id)`, возвращает
возрастание. Селекторы не мутируют и не создают канал.

## 7. Ошибки

```rust
pub enum HistoryError { ZeroWindow, ZeroPageLimit }
pub enum PageError {
    ZeroLimit,
    LimitTooLarge { limit: u16, max: u16 },
    StaleToken { expected: u64, got: u64 },
    ChannelMismatch { expected: u64, actual: u64, message_id: u64 },
}
```

`Display` + `std::error::Error`; сообщения не содержат content/author. Ошибки не паникуют.

## 8. Детерминизм и границы

- `BTreeMap` везде; одинаковый вход → структурно равный выход.
- `Debug` для `History` — только счётчики/конфиг, без content.
- Нет I/O, `unsafe`, новых зависимостей.
- `State.messages` не ограничивается; это явное ограничение scope.
- `after` и `around` — контракт/локальная модель; capture подтверждает только формы запросов
  `limit`, `before`, `around`, но не серверную семантику.
