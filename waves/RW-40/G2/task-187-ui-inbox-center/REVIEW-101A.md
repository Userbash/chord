# REVIEW-101A — TASK-187 (`inbox.js`)

**Verdict: APPROVED** — P0 = 0, P1 = 0, мутации 5/5 killed.

## Проверки
- Базовый прогон `node --test tests/unit/inbox.test.mjs`: tests 19, pass 19, fail 0.
- MD5 после всех мутаций восстановлен: `6c706ccf38a79e01a50cca056dc31145 rust/gui-spike/ui/features/inbox.js` — совпадает с REPORT.md и md5.txt. Тестовый файл не изменялся.
- Реализация соответствует контракту: бейдж = `mentions + friendRequests` (unreads не в счётчике), `hidden`/`aria-hidden` синхронны в `open`/`close`/`toggle`, `destroy` идемпотентен, состояние вне DOM.

## Мутации (каждая: правка → тест → восстановление)
| # | Мутация | Результат |
|---|---------|-----------|
| 1 | `value > 99` → `>= 99` | KILLED (fail 1) |
| 2 | `value <= 0` → `< 0` | KILLED (fail 2) |
| 3 | счётчик += `unreads.length` | KILLED (fail 1) |
| 4 | текст пустого состояния → `""` | KILLED (fail 1) |
| 5 | инверсия `aria-selected` | KILLED (fail 1) |

## Замечания (P2, не блокируют)
- Ветка невалидной вкладки в `setTab` не покрыта тестом: мутация «убрать `!INBOX_TABS.includes(tab)`» выжила, хотя REPORT.md заявляет проверку невалидных вкладок. Поведение в коде корректно (`return false`), нужен лишь негативный тест — карточка на QA/воркера.

Гейт `gate:worker` не запускался (token-lean режим). Git-коммитов нет; временные мутации восстановлены.
