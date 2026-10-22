# RW-14 — поправки тест-спеки и red-статус

## 1. Первый red W14a/W14b

`red-w14ab.log`, `red-w14ab.exit=101`: тесты `history_types`/`history_pagination` не
компилировались, потому что модуля `discord_core::history` ещё не было. Дополнительно первый
вариант `history_types` использовал `State::default()`, которого в P2-02 нет (есть
`State::from_ready`). Это ошибка тестового setup, не поведения; helper заменён на минимальный
READY через `minimal_ready_frame()`.

После этого red остаётся **compile-red**: проверить поведение до появления модуля нельзя.
Это честно зафиксировано; поведенческий red-first есть у W14c (отсутствующие селекторы) и у
двух поздних правок (root re-exports, счётчики страницы).

## 2. W14c — behavior-red

`red-w14c.log`, `red-w14c.exit=101`: `history_selectors` не компилировался из-за отсутствия
13 методов (`channel_count`, `total_len`, …), затем реализация дала green 8/8
(`green-w14c.log`). Это настоящий red-first для селекторов.

## 3. Поздние red-first правки

| Кейс | Red | Green |
|---|---|---|
| корневые re-exports `discord_core::{Cursor,…,PageToken}` | `red-w14-root.log`, exit 101 | `green-w14-after-root.log`, 20/20 |
| `PageApply.inserted/replaced` считают уникальные id, а не повторы страницы | `red-w14-counts.log`, exit 101 (ожидалось 0, было 1) | `green-w14.log`, 20/20 |

`h07_crate_root_reexports` добавлен после первого ревью (P2-1); `h11` дополнен проверкой
счётчиков после P3-2. Это не подгонка под код: проверяются обещанные DESIGN §2 имена и
семантика «дубликат внутри страницы — не отдельная вставка».

## 4. Архив

- `tests-red/` — первый вариант до модуля/до setup-фикса; md5 в `MD5SUMS.red.txt`.
- `tests-final/` — финальные тесты (types 7, pagination 5, selectors 8 = 20), md5 в
  `MD5SUMS.txt`.
- `tests-red/` не содержит `history_selectors.rs`, потому что W14c начал позже; его red —
  `red-w14c.log`, финальный тест — только в `tests-final/`.
- Реальные фикстуры 023/024/025/026 и синтетические страницы явно разделены в шапках тестов;
  `after`/`around`/`has_more` не выдаются за подтверждённые факты сервера.
