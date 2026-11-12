# REVIEW-137A — PR-108 / task-222 (FE-P1-21): UI-контроллер автодополнения композера

- **Задача**: `task-222`, ветка `feature/task-222-ui-composer-autocomplete`, база `unstable` (`69dcb8f`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Ревьюер**: DeepSeek (Technical Review, REVIEW-137A)
- **Режим**: token-lean (без docs/PROCESS/reference-oracle, без `gate:worker` и полного прогона тестов)

## Verdict: APPROVED

## Проверено

1. **Targeted-тест**: `node --test tests/unit/composer-autocomplete.test.mjs` → `tests 8, suites 2, pass 8, fail 0` (40 ms), 0 skipped.
2. **MD5 совпадает с REPORT.md**:
   ```
   79eb7acc7351aa330549229624298e1d  rust/gui-spike/ui/features/composer-autocomplete.js
   5453420db7fd309ff029a126dd471253  tests/unit/composer-autocomplete.test.mjs
   ```
3. **Контракт и реализация сверены по коду**:
   - `detectAutocompleteTrigger`: слэш только от начала строки `/^\/([a-z0-9_-]*)$/i`; `@`/`#` только в начале строки или после `\s`; корректный `startIndex`; `null` для email, URL-якоря, после пробела, пустого и невалидного ввода.
   - `createComposerAutocompleteController`: `role="listbox"` + `data-composer-autocomplete`, `role="option"` + `aria-selected`, русские заголовки секций, лимит 10, циклический `moveSelection(delta)`, замена токена на `<@id>` / `<#id>` / `/command ` с вызовом `onInsertReplacement({ newText, newCursorIndex, item })` и закрытием списка, клик мышью и `close()`.
   - Стиль: ESM, `// @ts-check` + JSDoc, без `innerHTML` (сборка через `createElement`/`replaceChildren`), ошибки не глотаются (`TypeError` при отсутствии `container`).
   - Никаких чужих файлов в скоупе PR не затронуто.

## Мутации: 5/5 убито

Каждая мутация вносилась в `rust/gui-spike/ui/features/composer-autocomplete.js`, прогонялся targeted-тест, затем файл восстанавливался побайтово из исходного буфера.

| # | Мутация | Результат теста | Убито | Убивший тест |
|---|---------|-----------------|-------|--------------|
| M1 | `AUTOCOMPLETE_MAX_SUGGESTIONS = 10` → `20` | exit 1, fail 2 | да | «экспортирует … лимит … = 10», «ограничивает число опций до 10 …» |
| M2 | Слэш-регекс `/^\/([a-z0-9_-]*)$/i` → `/\/([a-z0-9_-]*)$/i` (снят якорь начала строки) | exit 1, fail 1 | да | «распознаёт триггер слэш-команды только в начале строки …» |
| M3 | Токен пользователя `` `<@${id}>` `` → `` `@${id}` `` | exit 1, fail 2 | да | «confirmSelection() заменяет токен …», «выбор опции кликом мыши …» |
| M4 | В `moveSelection` убран циклический модуль: `(((i+step)%len)+len)%len` → `(i+step)%len` | exit 1, fail 1 | да | «… циклическую навигацию moveSelection(delta)» |
| M5 | Заголовок канала `"ТЕКСТОВЫЕ КАНАЛЫ"` → `"КАНАЛЫ"` (расходится с оракулом `ru-catalog.json`) | exit 1, fail 1 | да | «рендерит listbox … заголовками КОМАНДЫ / УЧАСТНИКИ И РОЛИ / ТЕКСТОВЫЕ КАНАЛЫ» |

После восстановления: `md5sum` = `79eb7acc7351aa330549229624298e1d` (совпадает с исходным и с REPORT.md), повторный прогон — `pass 8, fail 0`.

## Замечания (не блокирующие, P2)

- Монтаж контроллера в `composer.js` / `app.js` вне write-scope карточки — требуется отдельная интеграционная задача; сейчас модуль не подключён к реальному композеру.
- В рабочем дереве `git status` показывает изменённый `package-lock.json` (+6 строк). Ревьюер его не трогал; убедиться перед merge, что он не попадёт в PR.
- Оба файла задачи — новые (`??`), в коммит не включались. `git commit` не выполнялся.

## Итог

Контракт покрыт независимыми проверками (DOM-шим, точные строки замены, циклическая навигация, оракульные русские заголовки), все 5 мутаций убиты, MD5 стабилен. **APPROVED** — передать PM на продуктовую приёмку.
