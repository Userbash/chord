# REVIEW-161A — task-247 (UI events modal, FE-P0-11)

- **Рецензент**: Technical Reviewer (DeepSeek, dispatch `ctx_1e3cb1e16e4f`)
- **Ветка / карточка**: `feature/task-247-ui-events-modal` / `task-247`
- **Проверяемые файлы**:
  - `rust/gui-spike/ui/features/events-modal.js` — md5 `3e14e44521110a282863f7f855a0358b`
  - `tests/unit/events-modal.test.mjs` — md5 `3dd0638bb74b2c5507e9722a07d5f36c`

## Verdict: APPROVED

Набор тестов защищает контракт модуля: 5 из 5 внесённых мутаций убиты, файл после каждой мутации восстановлен байт в байт по md5 из `REPORT.md`.

## Прогон targeted-теста

```
node --test tests/unit/events-modal.test.mjs
exit=0; fail 0; cancelled 0; skipped 0; todo 0
```

Тест-харнесс не зависит от реального DOM: `FakeElement` реализует `replaceChildren`, `querySelectorAll`, `dispatchEvent` и `aria-*` атрибуты, поэтому проверки идут по фактам дерева, а не по мокам поведения.

## Мутационное тестирование (5/5)

Полный лог: `mutations-161A.log` (этот каталог).

| # | Мутация | Ожидание | Результат |
|---|---------|----------|-----------|
| M1 | `filterEventsByTab`, ветка `upcoming`: `status !== 1` → пропускать и `status === 3` | падение сортировки/состава upcoming | KILLED, `exit=1`, actual `['ev-50','ev-10','ev-20','ev-30']` vs expected `['ev-10','ev-20','ev-30']` |
| M2 | `normalizeEventTabId`: `"active"` возвращает `"upcoming"` | падение проверки активной вкладки | KILLED, `exit=1`, actual `'upcoming'` vs expected `'active'` |
| M3 | `compareEventsAsc`: `startA < startB` возвращает `1` (инверсия порядка) | падение порядка карточек | KILLED, `exit=1`, actual `['ev-2','ev-1']` vs expected `['ev-1','ev-2']` |
| M4 | `aria-selected` у вкладки всегда `"false"` | падение a11y-контракта вкладок | KILLED, `exit=1`, actual `'false'` vs expected `'true'` |
| M5 | RSVP: `nextInterested = wasInterested` вместо `!wasInterested` | падение переключения RSVP и `onToggleRsvp` | KILLED, `exit=1`, actual `[{eventId:'ev-2',nextInterested:false}]` vs expected `{...true}` |

Восстановление: после каждой мутации `md5 = 3e14e44521110a282863f7f855a0358b` — совпадает с исходным и с `REPORT.md`.

```
mutations killed = 5/5
final md5 = 3e14e44521110a282863f7f855a0358b
-> BYTE-IDENTICAL
```

## Замечания

- Блокирующих (`P0`) и существенных (`P1`) замечаний нет.
- Форма создания вызывает `onCreateEvent` только при непустых (после `trim`) `name` и `scheduledStartTime`; округление `entityType` через `Math.trunc` с fallback `3` соответствует оракулу `rest-static.json` (типы сущностей `1..=3`).
- Не блокирует: контроллер не снимает обработчики в `destroy()`, а только помечает `destroyed` и очищает контейнер. Дерево отбрасывается целиком, утечек нет, но при повторном `destroy()` на удержанных ссылках слушатели остаются — допустимо для текущего контракта.
- `close()` не скрывает DOM (только вызывает `onClose()`); поведение совпадает с тестами и ТЗ, отмечено как сознательное решение.

## Границы проверки

- Полный `gate:worker` и `git commit` не запускались — режим ревью-задачи это запрещает.
- Проверены только `events-modal.js`, `events-modal.test.mjs` и `REPORT.md`; чужие модули, доки §8 и `waves/*` не изменялись.

**Итог: APPROVED — 5/5 мутаций убиты, файл восстановлен байт в байт, targeted-тест зелёный.**
