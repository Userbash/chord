# REVIEW-136A — PR-107 / task-220 (FE-P1-20): UI member list controller

- **Ветка**: `feature/task-220-ui-member-list`
- **Ревьюер**: Technical Reviewer (DeepSeek V4.1 Flash), терминал `term_e9994ada`
- **Дата**: 2026-10-31
- **Артефакты**: `rust/gui-spike/ui/features/member-list.js`, `tests/unit/member-list.test.mjs`, `REPORT.md`

## Verdict: APPROVED

## Targeted test

`node --test tests/unit/member-list.test.mjs` → `tests 7, suites 2, pass 7, fail 0, skipped 0`, `rc=0`.

## Mutation testing: 5/5 killed

Каждая мутация вносилась в `member-list.js`, прогонялся targeted-тест, затем файл восстанавливался из побайтовой копии. Baseline до и после: `rc=0, fail=0`.

| # | Мутация | Результат |
|---|---|---|
| M1 | `resolveMemberRoleHexColor`: `pos > bestPos` → `pos < bestPos` (младшая цветная роль вместо старшей) | KILLED (rc=1, fail=1) |
| M2 | Отключена offline-корзина: `status === "offline"` → недостижимое значение (offline-участники уходят в hoisted-группу) | KILLED (rc=1, fail=3) |
| M3 | `compareMembersRu`: прямой порядок → обратный (`b.localeCompare(a)`) | KILLED (rc=1, fail=1) |
| M4 | `if (member.isOwner === true)` → `!== true` (корона у не-владельцев) | KILLED (rc=1, fail=1) |
| M5 | `toggleVisibility`: `visible = !visible` → `visible = false` | KILLED (rc=1, fail=1) |

**Итог: 5/5 (100%).**

## Восстановление файла

```
e91643fa1484861b4f60d1dee992b026  rust/gui-spike/ui/features/member-list.js
```

MD5 после всех мутаций совпадает с `REPORT.md` — исходное состояние восстановлено побайтово.

## Проверки по существу

- **Контракт RE-оракула**: `aside[data-member-list]`, заголовки `NAME — COUNT` (`Админы — 2`), группы по убыванию `role.position`, корзины `В сети` / `Не в сети`, корона владельца, `customStatus`, HEX-цвет роли — покрыты тестами.
- **Неизменяемость входов**: `groupMembersByHoistedRole` не мутирует `members`/`roles` (`.slice().sort`, новые `Set`), тест проверяет через `deepEqual`.
- **Границы**: `null`-вход, пустой массив, пустые группы, не-hoisted роли, `color: 0` → нет цвета, отображение с ведущими нулями (`#00aaff`), `displayName: ""` → fallback на `username`.
- **A11y**: `aria-label` на `aside`, `role="group"` + `aria-label` с счётчиком, `role="listitem"`, `tabindex="0"`, синхронизация `hidden`/`aria-hidden`.
- **STYLE/GATES**: `// @ts-check` + JSDoc, ESM, чужие файлы не затронуты, `git commit` не выполнялся.

## Замечания (не блокирующие)

- **P3**: у элементов с `tabindex="0"` нет keyboard-обработчика (`Enter`/`Space`) для вызова `onSelectMember` — доступность только мышью. Требует отдельной a11y-задачи, вне write-scope карточки.
- **P3**: часть публичного API не покрыта тестами: `visible: false` при создании, `isVisible()`, `getGroups()`, алиас `setMembers`, перегрузка `createMemberListController(container, options)`.
- **P3**: как и указано в `REPORT.md`, монтаж контроллера в `app.js`/`screens.js` вне scope — интеграционная задача.

## Блокеры

Нет. `P0 = 0`, `P1 = 0`.
