# Q-G4-REG — отчёт воркера (G4 QA)

Дата: 2026-10-24. Batch: RW-40. PM: DeepSeek. Режим: без live/сети/окна/cargo/gate/commit.

## 1. Сделано

| # | Карточка | Файл | Суть |
|---|---|---|---|
| 1 | **G2-03** (QA4-01, M1) | `rust/gui-spike/ui/features/quick-switcher.js` | empty-state больше не инвертирован: `hidden` снимается только при 0 опций, ставится при >0 (было наоборот). |
| 2 | **G2-11** (QA4-02, M6) | `rust/gui-spike/ui/features/message-toolbar.js` | confirm удаления — производная от `deletePending`/`deleteInFlight` и **текущей** строки: снимается при уходе на другую строку и в `close()`, а также после `onDelete` resolve/reject (`.finally`). Повторный клик до ответа IPC игнорируется. |
| 3 | тесты | `tests/unit/gui-spike-regressions.test.mjs` (new) | 2 `it`: empty-state по `hidden` (8 опций / 0 / снова опции, `hint` не тронут) и две строки delete-confirm (A→B, возврат A, ошибка, успех). |

Harness, `search-overlay.js`, остальной `ui/**`, `src/**`, `gui-spike-features.test.mjs` — **не менялись**.
Аддитивных правок harness не потребовалось.

## 2. Red-first

`waves/RW-40/PM/q-g4-reg/red.log` — прогон на старом коде (2 tests, 2 fail):

| Тест | Падение |
|---|---|
| G2-03 empty | `«Ничего не найдено» видно при 8 опциях` (`true !== false`) |
| G2-11 sticky | `confirm строки A уехал на строку B` (`'Подтвердить удаление' !== 'Удалить сообщение'`) |

Mutation-проверка **финального** теста (правка в рабочем файле, restore по md5 — см. §4):

| Лог | Мутация | Результат |
|---|---|---|
| `red-m1.log` | `quick-switcher.js` empty всегда видим (M1) | **KILLED**, fail 1 |
| `red-m6-open.log` | убрать `resetDelete()` при `open(index!==current)` | **KILLED**, fail 1 (`возврат на A показал старое подтверждение`) |
| `red-m6-finally.log` | не сбрасывать confirm в `.finally` после IPC (M6) | **KILLED**, fail 1 (`после ошибки delete confirm не сброшен`) |

## 3. Green

| Команда | Результат | Лог |
|---|---|---|
| `node --test tests/unit/gui-spike-regressions.test.mjs` | 2 tests / **2 pass / 0 fail** | `green.log` |
| `node --test tests/unit/gui-spike-{dom,model,bench,features,auth}.test.mjs` | 216 / **216 pass / 0 fail / 0 skip** | `unit.log` |
| `node --test tests/unit/gui-spike-*.test.mjs` (весь UI-слой) | 218 / **218 pass / 0 fail** | `/tmp/opencode/q-g4-mut/all-gui-unit.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit **0** | `tsc.log` |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | **31/31** в допуске | `geometry.log` |

Ослаблений нет: старые файлы не правились, 216 прежних тестов зелёные; новый файл только добавляет 2.

## 4. Файлы и md5 (после restore)

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/quick-switcher.js` | `7762a9c8f9fd30463a6ceac26cd460af` |
| `rust/gui-spike/ui/features/message-toolbar.js` | `5d14475b2babbb31494adec78cd42b58` |
| `tests/unit/gui-spike-regressions.test.mjs` | `93b965a5ea5f19ca6bc5d12d44dae063` |

Бэкапы мутаций: `/tmp/opencode/q-g4-mut/{qs,mt}.bak`; после мутаций md5 совпали с таблицей.

## 5. TDD-PLAN §8 — строка для PM

Добавить в таблицу §8 (все токены присутствуют в тесте; формат — 3 колонки):

```markdown
| `tests/unit/gui-spike-regressions.test.mjs` | `rust/gui-spike/ui/features/quick-switcher.js`, `rust/gui-spike/ui/features/message-toolbar.js` | `quick-switcher-empty`, `msg-toolbar`, `delete_message` |
```

## 6. Риски / что осталось

1. **Meta `tests/meta/plan-sync.test.mjs` будет красным**, пока строка §8 не добавлена PM: тест требует строку §8 для каждого `tests/**/*.test.mjs`. Это ожидаемое состояние по ТЗ (TDD-PLAN правит PM).
2. G2-11: `deleteInFlight` защищает от повторного destructive-клика до ответа IPC — поведение новое, но покрыто тестом; в живом Discord не проверялось (запрет).
3. G2-03: `aria-hidden` на empty не ставится — `hidden` уже исключает узел из дерева доступности; отдельный атрибут не нужен.
4. JSDoc-типы нового теста проверены только `tsc` UI-конфига (тесты не входят в него); отдельного `tsc` по `tests/**` в проверках задачи нет.
