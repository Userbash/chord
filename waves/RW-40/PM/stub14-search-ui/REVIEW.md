# REVIEW — STUB-14-UI-SEARCH-FILTERS

Дата: 2026-10-30. Режим: независимое read-only ревью; cargo не запускался.

## Вердикт

**RETURN. P0=0, P1=1, P2=1.**

### P1 — числовые `author_id` и `mentions` ломаются в локальном fallback

`rust/gui-spike/ui/features/search-overlay.js:724-734` после remote-ответа сравнивает
`filters.from` и `filters.mentions` с отображаемым `message.author`. Значения,
которые карточка отправляет как `author_id`/`mentions` (например `42` или `77`),
обычно не равны имени автора; при текущем backend `search.rs`, который, как указано
в REPORT, игнорирует `params.query`, такой результат отбрасывается локально и
фильтр возвращает пусто даже при совпадении. Нужны независимые поля/id в
нормализованном сообщении либо отказ от ложного локального применения до поддержки
backend. Добавить тест с числовым author/mention и matching server message.

### P2 — невалидные календарные даты нормализуются вместо отказа

`rust/gui-spike/ui/features/search.js:95-100` проверяет только regex и `Date.parse`.
`Date.parse("2026-02-31T...")` нормализует дату в март, поэтому в query уходят
`min_id/max_id` для другой даты; аналогичный `isIsoDate` в
`search-overlay.js:110-112` принимает тот же класс значений. Проверка должна
сравнивать компоненты UTC с исходными и явно отклонять такие даты.

## Проверки

- red: `red.log` — 3 новых теста failed, 2 passed, exit 1.
- green: `green.log` — 105 passed / 0 failed, tsc exit 0.
- md5 совпали с REPORT: search-overlay b6f122..., search.js 87379a...,
  тест 9b0995....
- В добавленных тестах нет `.only`, необоснованных `skip`, удаления assert или
  ослабления порогов; query проверяется независимым snowflake-оракулом.

Ограничение backend, отмеченное в REPORT, не снимает P1: UI обязан сохранять
корректную семантику fallback до появления отдельной backend-карточки.
