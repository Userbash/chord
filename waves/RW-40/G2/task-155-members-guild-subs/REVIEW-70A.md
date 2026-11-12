# Рецензия Technical Reviewer (Review 70A) — task-155

- **Карточка:** `task-155` (FE-P1-03: Guild Subscriptions OP 37 & Role Grouping в `rust/gui-spike/ui/features/members.js`).
- **Воркер-отчёт:** `/home/user/orca/project/waves/RW-40/G2/task-155-members-guild-subs/REPORT.md`.
- **Рабочая директория:** `/home/user/orca/workspaces/project/feature-task-155-members-guild-subs`.
- **Файлы на ревью:**
  - `rust/gui-spike/ui/features/members.js` (MD5: `6b4e06afe0314b64d8302354fd702291`)
  - `tests/unit/members-subscriptions.test.mjs` (MD5: `2e04c232bb8d4ed643a027e342678215`)

---

## 1. Проверка контрактов и запусков

### 1.1. Тесты
Команда:
```bash
node --test tests/unit/members-subscriptions.test.mjs tests/unit/gui-spike-dom.test.mjs
```
Результат:
- `GUI spike: настоящий app.js в DOM-шиме`: 25 passed.
- `чат-вью P3-02`: 4 passed.
- `computeSubscriptionRanges (OP 37 windows)`: 7 passed.
- `createGuildSubscriptionsTracker (OP 37 tracking & deduplication)`: 3 passed.
- `groupMembersByRoleOrStatus & renderMembersList`: 4 passed.
- **Всего: 43 passed, 0 failed, 0 skipped** (353ms).

Дополнительно проверен смежный сьют диспетчера:
```bash
node --test tests/unit/dispatch_ui.test.mjs
```
- **57 passed, 0 failed** (498ms).

### 1.2. Статический анализ типов (Typecheck)
Команды:
```bash
npx tsc -p tsconfig.json
npx tsc -p rust/gui-spike/ui/tsconfig.json
```
Результат:
- **0 ошибок** (exit code 0). Все JSDoc типы соответствуют TypeScript strict-mode.

---

## 2. Анализ реализации

1. **`computeSubscriptionRanges(scrollTop, viewportHeight, rowHeight = 44)`:**
   - Чанки выравниваются строго по 100 элементов вида `[c * 100, c * 100 + 99]`.
   - Первый базовый чанк `[0, 99]` гарантированно входит в набор (`new Set([0])`).
   - Результат дедуплицируется, сортируется по возрастанию и обрезается до `.slice(0, 3)` (максимум 3 чанка на канал).
   - Безопасная обработка отрицательного `scrollTop`, нулевой/отрицательной высоты вьюпорта и высоты строки.

2. **`createGuildSubscriptionsTracker(options)`:**
   - Формирует каноническую структуру исходящего опкода Discord Gateway OP 37:
     `{ subscriptions: { [guildId]: { typing: true, threads: true, activities: true, channels: { [channelId]: ranges } } } }`.
   - Дедупликация: `syncSubscription` сравнивает текущие `guildId`, `channelId` и диапазоны через `areRangesEqual` с предыдущими отправленными, предотвращая спам вызовами `onSubscribe`.
   - Поддерживает конфигурируемый `debounceMs` (по умолчанию 50 мс), ручной `flush`, переключение каналов `switchChannel`/`setChannel`, привязку DOM-ноды `attachSidebar`/`attach` и корректную отписку/очистку таймеров `destroy`/`dispose`.

3. **`groupMembersByRoleOrStatus(members, options)` и `renderMembersList(container, members, options)`:**
   - Поддерживает hoisted-группы из `GUILD_MEMBER_LIST_UPDATE` (`groups: [{ id, name, count, roleColor }]`).
   - Честно сохраняет `count` группы из шлюза при его наличии.
   - Нормализует цвета ролей (`formatRoleColor`) из числового представления (например, `16711680`) или hex в `#rrggbb`.
   - Сохраняет полную обратную совместимость со статусными группами (`online`, `idle`, `dnd`, `offline`) при отсутствии явного списка `groups`.
   - `renderMembersList`: генерирует семантичные DOM-элементы `.member-group`, заголовок `.member-group-title` с атрибутом `data-height="42"`, строками `.member` с соответствующими `aria-label`, аватарами и статус-дотами.

---

## 3. Мутационное тестирование (5/5 убито)

Выполнено 5 независимых мутаций в `rust/gui-spike/ui/features/members.js`.

| # | Описание мутации | Локация | Ожидаемый эффект | Результат теста | Статус |
|---|---|---|---|---|---|
| **M1** | Удаление гарантированного первого чанка `[0, 99]`: `new Set()` вместо `new Set([0])` | `members.js:1087` | Падение тестов при скролле вниз, на стыках и в трекере | `✖ includes [0, 99] at scroll`, `✖ chunk boundaries`, `✖ limits to 3`, `✖ tracker scroll` | **Убит (4 теста)** |
| **M2** | Ослабление лимита чанков: `.slice(0, 5)` вместо `.slice(0, 3)` | `members.js:1094` | Нарушение лимита протокола OP 37 (максимум 3 чанка) | `✖ ограничивает количество чанков максимум 3 на канал` (`5 !== 3`) | **Убит** |
| **M3** | Отключение дедупликации подписок: `if (false && ... areRangesEqual)` | `members.js:1213` | Повторные спам-вызовы `onSubscribe` при неизменных диапазонах и скролле | `✖ не спамит повторными вызовами onSubscribe` (`2 !== 1`), `✖ обрабатывает прокрутку с дебаунсом` | **Убит (2 теста)** |
| **M4** | Сброс счётчика группы из шлюза: `count: 0` вместо `g.count` | `members.js:1381` | Потеря оригинального значения счётчика участников из шлюза | `✖ поддерживает hoisted-группы с точным сохранением count` (`1 !== 100`) | **Убит** |
| **M5** | Изменение высоты заголовка группы: `data-height="40"` вместо `"42"` | `members.js:1528` | Нарушение контракта высоты строки группы (токен D-13 / 42px) | `✖ renderMembersList рендерит группы с заголовками data-height=42` (`'40' !== '42'`) | **Убит** |

После проведения мутационного тестирования исходный файл `rust/gui-spike/ui/features/members.js` полностью восстановлен.
Контрольная сумма сверена:
- `6b4e06afe0314b64d8302354fd702291  rust/gui-spike/ui/features/members.js` (совпадает с исходной).
- `2e04c232bb8d4ed643a027e342678215  tests/unit/members-subscriptions.test.mjs` (совпадает с исходной).

---

## 4. Замечания и риски

- **P3 (информационно):** В будущем при интеграции `groupMembersByRoleOrStatus` в виртуализированный список `VirtualList` в `dispatch-view.js` потребуется учитывать, что заголовки групп имеют высоту 42px (как и обычные строки участников по D-04), поэтому смещение строк сохранит линейность при добавлении заголовков в общую модель элементов.
- **Замечаний категорий P0 и P1 нет.**

---

## 5. Итоговый вердикт

**ВЕРДИКТ: PASS / Ready for Merge**
- Контракт OP 37 и группировки участников полностью покрыт тестами и спецификацией.
- 0 ошибок компиляции и линтинга типов TypeScript.
- 5/5 мутантов убиты соответствующими тестами.
- Целостность файлов сохранена.
