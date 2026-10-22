# RW-25 / B07 — P3 headless UI models (batch TZ)

Дата: 2026-10-22. Статус: **в работе** (code phase).
Волна ведёт headless-остатки P3 без GUI-зависимостей (тулкит всё ещё заблокирован
решением владельца по зависимостям). Никаких новых crate-зависимостей, только `serde`.

## Карточки и владельцы

| Карточка | Модуль | Владелец | Размер | Тестов (план) |
|---|---|---|---|---|
| P3-03 Отправка сообщений | `composer.rs` | W25a / p3-message-composition | L | 16–24 |
| P3-11 QuickSwitcher | `quick_switcher.rs` | W25a / p3-message-composition | S | 10–14 |
| P3-12 Компоненты (12 контрактов) | `components.rs` | W25b / p3-components | L | 20–28 |
| P3-13 Токены и темы | `tokens.rs` | W25b / p3-components | M | 10–16 |
| P3-05 Реакции | `reactions.rs` | W25c / p3-reactions | M | 14–18 |
| P3-06 Меню сообщения | `message_menu.rs` | W25c / p3-reactions | M | 12–16 |
| P3-10 Эмодзи-пикер | `emoji_picker.rs` | W25c / p3-reactions | M | 10–14 |
| P3-08 Список участников | `member_list.rs` | TL-1 (main) | L | 16–24 |
| P3-09 Поиск-оверлей | `search_overlay.rs` | TL-1 (main) | L | 16–24 |

## Файловая матрица (пересечений нет)

- W25a worktree: `rust/crates/discord-ui/src/{composer,quick_switcher}.rs`, `lib.rs`.
- W25b worktree: `rust/crates/discord-ui/src/{components,tokens}.rs`, `lib.rs`.
- W25c worktree: `rust/crates/discord-ui/src/{reactions,message_menu,emoji_picker}.rs`, `lib.rs`.
- TL-1 main: `rust/crates/discord-ui/src/{member_list,search_overlay}.rs`, `lib.rs`,
  `waves/RW-25/*`.

`lib.rs` меняется в каждом worktree изолированно; при сведении TL-1 объединяет `pub mod`
единожды в main. Конфликтов нет, потому что worktree — отдельные git-каталоги и target-dir.

## Оракулы (read-only)

| Источник | Что берём |
|---|---|
| `docs/re/frontend-d03.md` §1 | 7 секций фильтров, 9 «Содержит», 3 сортировки, REST `messages/search`, группы результатов |
| `docs/re/frontend-d03.md` §2 | 3 вкладки пикера, 10 категорий rail в точном порядке, 50 видимых/17 locked |
| `docs/re/frontend-d03.md` §3 | группы «Server Booster — 2», «В сети — 735», строки member/name/subText |
| `docs/re/frontend-d03.md` §5 | 12 компонентов: варианты, размеры, состояния, aria, токены |
| `docs/re/frontend-d03.md` §6 | точные значения 19 ключевых токенов темы dark |
| `/home/sanya/re-deep/ui/tokens.json` | 4795 CSS-переменных (счётчик подтверждён) |
| `docs/rust-client/PLAN.md` P3-01…P3-16 | DoD и размеры карточек |

## Границы

- Только headless-модели: никакого рендера, окна, GPU, сети.
- Модели самодостаточны (без `discord-core`), чтобы не тянуть cross-crate API в UI-каркас.
- Не вводим wire-поля, которых нет в RE-дампе (например, `REST ACK`, unread-count, query D-02).
- Мутации/гейт/ревью — после green всех трёх воркеров, один batch gate.

## Артефакты волны

```
waves/RW-25/
  BATCH-TZ.md          — этот файл
  w25a-red.log         — red P3-03/P3-11 (worktree W25a)
  w25a-green.log       — green P3-03/P3-11
  w25b-red.log         — red P3-12/P3-13 (worktree W25b)
  w25b-green.log       — green P3-12/P3-13
  w25c-red.log         — red P3-05/P3-06/P3-10 (worktree W25c)
  w25c-green.log       — green P3-05/P3-06/P3-10
  main-red.log         — red P3-08/P3-09 (main)
  main-green.log       — green P3-08/P3-09
  REPORT-B07.md        — сводный отчёт после gate
  TEST-MATRIX-B07.md   — карточка → тест → оракул
```
