# Technical Review 172A — PR-143 (`task-257` / `FE-P0-16`)

- **Ветка:** `feature/task-257-ui-moderation-panel` (`HEAD 850fab9`)
- **Ревьюер:** Технический ревьюер (DeepSeek, token-lean)
- **Review ID:** 172A
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0=0`, `P1=0`, мутации `5/5` убиты.

## 1. Целостность артефактов

| Файл | md5 | Сверка с `REPORT.md` |
| --- | --- | --- |
| `rust/gui-spike/ui/features/moderation-panel.js` | `25231dfa4b572e1cf8ae89a261d73d9b` | совпадает |
| `tests/unit/moderation-panel.test.mjs` | `c97150c308b5dcecaa26ff1937888373` | совпадает |
| `rust/gui-spike/ui/features/moderation-panel.js` (после всех мутаций) | `25231dfa4b572e1cf8ae89a261d73d9b` | восстановлен побайтово |

Тестовый файл не изменялся — md5 до и после прогона одинаков.

## 2. Targeted-прогон (до мутаций)

```
node --test tests/unit/moderation-panel.test.mjs
tests 8 | suites 3 | pass 8 | fail 0 | cancelled 0 | skipped 0 | exit=0
```

## 3. Ручные мутации (по одной, с восстановлением из бэкапа)

Каждая мутация: правка `moderation-panel.js` → `node --test tests/unit/moderation-panel.test.mjs` → фиксация `exit` и числа упавших тестов → `cp` бэкапа обратно → `md5sum` сверен с базовым.

| ID | Мутация | Ожидаемый тест | exit | Убита | md5 мутанта | md5 после восстановления |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | `POSITIVE_SNOWFLAKE_RE = /^[1-9]\d{0,19}$/` → `/^[0-9]\d{0,20}$/` (пропускает `0` и 21-значный ID) | T-257-05 | 1 | **YES** | `0dc7e132a94c6328823ec9d9408ab7c0` | `25231dfa4b572e1cf8ae89a261d73d9b` |
| M2 | `MAX_BAN_REASON_LENGTH = 512` → `1024` (пропускает причину в 513 символов) | T-257-05 | 1 | **YES** | `ec6779cb5b6c69bd5249ce6fd5021182` | `25231dfa4b572e1cf8ae89a261d73d9b` |
| M3 | снят `.toLowerCase()` у `opts.query` в `filterAuditEntries` (регистрозависимый поиск) | T-257-03 | 1 | **YES** | `11846e1791dd10048e2403f27de70e58` | `25231dfa4b572e1cf8ae89a261d73d9b` |
| M4 | `result.push({ ...item })` → `result.push(item)` (нет клонирования записей журнала) | T-257-02 | 1 | **YES** | `0ea1e2d1eaaa9593f3ffdbed9bd6f4ae` | `25231dfa4b572e1cf8ae89a261d73d9b` |
| M5 | `unbanBtn.setAttribute("data-mod-unban-btn", userId)` → `""` (селектор разбана ломается) | T-257-06 | 1 | **YES** | `b72a6e1146295577439104457d0dd92f` | `25231dfa4b572e1cf8ae89a261d73d9b` |

Убийства не синтаксические — каждая падает на целевом `AssertionError`:

- **M1:** `actual: 1, expected: 0` (нулевой Snowflake принят формой).
- **M2:** `actual: 1, expected: 0` (причина 513 символов принята).
- **M3:** `actual: [], expected: [ 'a-2' ]` (поиск `NOISY` перестал находить `NoisyUser`).
- **M4:** `AssertionError: элементы должны клонироваться`.
- **M5:** `AssertionError: должна быть кнопка разбана для первого пользователя` (`actual: null`).

Полные логи: `REVIEW-172A-mutations.log`.

## 4. Проверка контракта и oracle-сверка (чтение кода)

- `BAN_DELETE_OPTIONS_RU`: `Object.freeze` на массиве и каждом элементе, значения `0 / 3600 / 86400 / 604800` — совпадают с `MAX_BAN_DELETE_MESSAGE_SECONDS = 604_800` в `rust/gui-spike/src/moderation.rs`.
- Валидация бана: `POSITIVE_SNOWFLAKE_RE` = 1..20 цифр, старшая цифра `1..9` (`0` отклоняется), `reason.length <= 512` по `MAX_AUDIT_REASON_CHARS`; пустая причина допустима (тест T-257-05 п.7). Контракт соответствует oracle.
- `filterAuditEntries`: не мутирует вход (клонирование записей), `actionType <= 0`/отсутствие — без фильтра, `query` ищется в `actorName`/`targetName`/`reason` без учёта регистра (включая кириллицу).
- `createModerationPanelController`: ARIA (`role="region"`, `aria-label="Модерация сервера"`), селекторы `[data-mod-ban-form]`, `[data-mod-target-input]`, `[data-mod-reason-input]`, `[data-mod-delete-select]`, `[data-mod-ban-submit]`, `[data-mod-ban-id]`, `[data-mod-unban-btn]`, `[data-mod-audit-search]`, `[data-mod-audit-entry-id]` совпадают с ТЗ и тестами.
- `setState/getState/destroy`: снимок состояния клонируется (внешняя мутация `getState().bans` не влияет на контроллер), `destroy()` отключает обработчики через флаг `destroyed` и снимает корень из контейнера.
- Нет обращений к сети, нет `git commit`, нет правок чужих файлов; тестовый файл не тронут; полный `gate:worker` не запускался (режим batch/T0).

## 5. Замечания

**P0:** нет. **P1:** нет.

**P2 (не блокирует):**

1. `deleteMessageSeconds` в `handleBanSubmit` ограничен только `>= 0`; верхняя граница `MAX_BAN_DELETE_MESSAGE_SECONDS = 604800` не проверяется. Источник значения — фиксированный `<select>`, поэтому эксплуатация возможна лишь при подмене `value` из консоли; при появлении внешнего источника опций добавить клампинг к `604_800`.

**P3 (стиль/UX, на усмотрение G2):**

2. Поля формы бана и поиска размечены только `placeholder` без `aria-label`/`<label>` — для скринридеров это слабее; ARIA-аудит волны стоит закрыть явными метками.
3. `destroy()` в ветке fallback вызывает `replaceElementChildren(container, [])`, то есть чистит контейнер целиком, а не только панель. При контракте «контейнер принадлежит контроллеру» это допустимо, но стоит зафиксировать в JSDoc.
4. `replaceElementChildren` без `replaceChildren` и без array-like `children` не очищает прежние узлы перед `appendChild`; на реальном DOM ветка недостижима (`replaceChildren` есть), но шиму лучше добавить очистку.

## 6. Итог

`P0=0`, `P1=0`, targeted `8/8 pass`, мутации `5/5` убиты, восстановление побайтовое, md5 совпадает с `REPORT.md`.
**APPROVED** — `Ready for Merge` (за `P2/P3` — отдельные карточки, блокировки нет).
