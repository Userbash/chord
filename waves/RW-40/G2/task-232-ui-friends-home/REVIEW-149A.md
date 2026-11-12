# REVIEW-149A — TASK-232 (FE-P1-25) `friends-home.js`, G2

- **Карточка**: `task-232` — UI контроллер домашнего экрана ЛС и друзей (вкладки «В сети», «Все», «Ожидание», «Заблокированные», «Добавить в друзья»)
- **Ревьюер**: Technical Review / QA-верификация (DeepSeek, 149A)
- **Ветка**: `feature/task-232-ui-friends-home`
- **Предмет ревью**: `rust/gui-spike/ui/features/friends-home.js` (605 строк), `tests/unit/friends-home.test.mjs` (608 строк), `REPORT.md`
- **Verdict: APPROVED**

## 1. Соответствие контракту

| Требование | Проверка | Итог |
| --- | --- | --- |
| `RELATIONSHIP_TYPES` заморожен, `FRIEND=1, BLOCKED=2, PENDING_INCOMING=3, PENDING_OUTGOING=4` | тест «экспортирует замороженные константы…», сверено с `4d2a64cb5df5fc4e_0@258005` | OK |
| `FRIENDS_TABS_RU` заморожен, 5 русских подписей | тот же тест, `4d2a64cb5df5fc4e_0@243700` (`FRIENDS_SECTIONS`) | OK |
| `filterRelationshipsByTab` — 5 правил вкладок, регистронезависимый поиск по `displayName || username`, сортировка `localeCompare("ru")`, без мутации входа | тесты `online`/`all`/`pending`/`blocked`/`add` и тест поиска | OK |
| `createFriendsHomeController` — `[data-friends-home]`, 5 `[data-friends-tab]` с `aria-selected`, бейдж `[data-friends-pending-badge]` только `type === 3`, `[data-friends-search]`, `[data-friends-list]`, `[data-friend-row]` с кнопками dm/accept/remove | DOM-тесты 1–2 | OK |
| Вкладка `add` — форма `[data-add-friend-form]`, `[data-add-friend-input]`, `[data-add-friend-submit]`, вызов `onSendFriendRequest(username.trim())`, пустой ввод не отправляется | DOM-тест 3 | OK |
| A11y: `role="region"`/`region` label, `role="tablist"`/`tab`, `aria-selected`, `tabindex` ротация, `aria-label` у кнопок и поиска | чтение кода, `syncHeaderAndTabsDom` | OK |

Тесты сверяют контракт с RE-оракулом (`reference-oracle/decoded/…`), а не с текущей реализацией: коды типов, набор вкладок и состав «В сети» (`online | idle | dnd`) заданы независимо от кода.

## 2. Целевой прогон

```
node --test tests/unit/friends-home.test.mjs
→ exit 0, tests 7, suites 2, pass 7, fail 0
```

Совпадает с `green.log` из `REPORT.md`.

## 3. Мутационное тестирование 5/5

Мутации вносились в рабочую копию `friends-home.js`, после каждой — целевой прогон, затем файл восстанавливался из байтового снимка.

| # | Мутация | Результат |
| --- | --- | --- |
| M1 | `RELATIONSHIP_TYPES.FRIEND: 1 → 0` | **KILLED** (exit 1) |
| M2 | `ONLINE_STATUSES` без `"dnd"` (`["online","idle"]`) | **KILLED** (exit 1) |
| M3 | Бейдж считает `PENDING_OUTGOING` вместо `PENDING_INCOMING` | **KILLED** (exit 1) |
| M4 | Вкладка `"all"` инвертирована (`relType !== FRIEND`) | **KILLED** (exit 1) |
| M5 | Инверсия проверки пустого ввода в `handleSendFriendRequest` | **KILLED** (exit 1) |

**Итог: 5/5 (100%).** Выживших мутантов нет.

## 4. Восстановление файла

```
baseline md5:  859219bd3678198921343ce71e26dd79 (20188 байт)
restored md5:  859219bd3678198921343ce71e26dd79
match: True   bytes equal: True
post-restore:  node --test tests/unit/friends-home.test.mjs → exit 0
```

MD5 совпадает с зафиксированным в `REPORT.md` (строка 32). Рабочее дерево не содержит остаточных мутаций.

## 5. Замечания

Блокирующих (`P0`/`P1`) замечаний нет.

Наблюдения без блокировки:
- `destroy()` удаляет только `root`, слушатели на вкладках и форме не снимаются — для текущего жизненного цикла (одна страница на контроллер) безопасно; при повторном `create` на том же контейнере старые узлы собираются GC вместе с `root`.
- `role="listitem"` у `[data-friend-row]` при контейнере `role="list"` корректен; вложенные кнопки не ломают список.
- `addFriendSubmitBtn` дополнительно вешает `click` с `preventDefault`, дублируя `submit` формы — идемпотентно, но при реальном браузере `preventDefault` на клике не блокирует submit, поэтому двойного вызова `onSendFriendRequest` не будет.

## 6. Границы работы

- Полный `gate:worker` не запускался (режим ТЗ карточки), `git commit` не выполнялся.
- Изменённых продуктовых файлов нет: единственный артефакт ревью — этот документ.
