# REPORT — TASK-620 (G2 Frontend): Унифицированные контекстные меню сообщений и пользователей (ПКМ)

- **batch/карточка:** `RW-40 / TASK-620` (G2 Frontend — Унифицированные контекстные меню сообщений и пользователей)
- **Ветка:** `feature/task-620-context-menu`
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/task-620-context-menu`
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (согласно правилам `AGENTS.md` §6).

---

## 1. Сделано (`PROCESS.md` §4.2)

- **batch/карточка:** `RW-40 / TASK-620`;
- **сделано:**
  1. В `rust/gui-spike/ui/features/context-menu.js` (с сохранением 100% обратной совместимости с существующим контрактом T-227):
     - Реализована функция `buildMessageContextMenuItems(options)`:
       - «Ответить» (`reply`), «Редактировать» (`edit`, для своего сообщения или при наличии прав `canEdit`), «Скопировать текст» (`copy-text`), «Скопировать ID» (`copy-id`), «Удалить» (`delete`, `danger: true`, для своего сообщения или модератора).
     - Реализована функция `buildUserContextMenuItems(options)`:
       - «Профиль» (`profile`), «Упомянуть» (`mention-user`), «Написать в ЛС» (`message-user`), «Заглушить» / «Включить звук» (`mute-user` с учётом флага `isMuted`), «Громкость» (`user-volume`, `type: "slider"`, `min: 0`, `max: 200`, `value: 0..200`), «Скопировать ID» (`copy-id`).
     - Расширен `createContextMenuController`:
       - Поддержка рендеринга слайдера громкости (`role="slider"`, `[data-volume-slider]`, `min="0"`, `max="200"`, `aria-valuenow`, отображение `[data-volume-value]` в процентах).
       - Обработка `input` / `change` событий слайдера с вызовом `onVolumeChange(volume)`.
       - Программные методы `setVolume(volume)` и `getVolume()` с гарантированным зажатием в диапазон `0..200%`.
       - Авто-закрытие меню при клике вне меню (outside click) с безопасным удалением слушателя при `close()`.
       - Авто-позиционирование внутри viewport через `clampMenuCoordinates` с отступом 8px.
     - Реализован хелпер `bindContextMenuTrigger(element, controller, optionsResolver)`:
       - Перехватывает событие правого клика `contextmenu` (ПКМ), предотвращает стандартное браузерное меню (`preventDefault`), извлекает координаты клика и открывает контроллер. Возвращает функцию отписки `unbind()`.
  2. Создан файл стилей `rust/gui-spike/ui/styles/context-menu.css`, на 100% привязанный к дизайн-токенам `tokens.css` (без сырых hex/rgb/hsl цветов, без `!important`):
     - Классы и селекторы: `.context-menu`, `[data-context-menu]`, `.context-menu-item`, `[data-context-item]`, `[data-danger="true"]`, `.context-menu-separator`, `.context-menu-volume-wrap`, `.context-menu-slider`.
  3. Обновлены модульные тесты `tests/unit/context-menu.test.mjs` (всего 19 тестов в 6 сьютах):
     - Новые сьюты для `buildMessageContextMenuItems`, `buildUserContextMenuItems`, слайдера громкости, outside click и `bindContextMenuTrigger`.
- **red:** `node --test tests/unit/context-menu.test.mjs` → `1 fail` (`SyntaxError: The requested module '../../rust/gui-spike/ui/features/context-menu.js' does not provide an export named 'bindContextMenuTrigger'`), лог: `waves/RW-40/G2/task-620-context-menu/red.log`;
- **green:** `node --test tests/unit/context-menu.test.mjs` → `19/19 pass, 0 fail, 0 skipped` (`49.61ms`, `exit code = 0`), лог: `waves/RW-40/G2/task-620-context-menu/green.log`;
- **targeted:**
  - `node --test tests/unit/context-menu.test.mjs tests/meta/plan-sync.test.mjs` → `24/24 pass, 0 fail, 0 skipped` (`exit code = 0`);
  - `npm run typecheck` → `0 errors` (`exit code = 0`);
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `0 errors` (`exit code = 0`);
- **oracle:**
  - `/home/sanya/re-deep/frontend-d03.md` (§5 п.3–4 Menu / ContextMenu: `menu_c1e9c4`, `role="menu"`, `role="menuitem"`, `colorDanger`, `separator_c1e9c4`, `CONTEXT_MENU_OPEN/CLOSE`);
  - `/home/sanya/re-deep/ui/01-msg-menu-items.json`, `01-msg-menu.html`;
  - `rust/gui-spike/ui/tokens.css` (токены `--surface-2`, `--border-subtle`, `--menu-min-w`, `--menu-pad`, `--menu-border`, `--menu-radius`, `--row-menu-item`, `--accent`, `--danger-fill`, `--danger-text`);
- **генерация:** не применимо (статические модули UI и таблица стилей);
- **мутации:** `5/5 KILLED` (`0 survivors`, `0 INCONCLUSIVE`, лог: `waves/RW-40/G2/task-620-context-menu/mutations.log`):
  - `M1_message_unified` (`unified: true` → `unified: false` в `buildMessageContextMenuItems`): **KILLED** (`exit=1`) — тест `buildMessageContextMenuItems формирует унифицированный набор пунктов для своего сообщения`;
  - `M2_user_mute_label` (инверсия подписи `isMuted ? "Включить звук" : "Заглушить"`): **KILLED** (`exit=1`) — тест `buildUserContextMenuItems переключает подпись Заглушить на Включить звук при isMuted: true`;
  - `M3_volume_clamp` (отключение зажатия громкости в диапазон `0..200%` в `setVolume`): **KILLED** (`exit=1`) — тест `поддерживает программные методы setVolume и getVolume с ограничением 0..200%`;
  - `M4_outside_click` (`return` без вызова `close()` в `handleOutsideClick`): **KILLED** (`exit=1`) — тест `закрывает меню по клику вне меню (outside click)`;
  - `M5_contextmenu_prevent_default` (удаление `event.preventDefault()` в `bindContextMenuTrigger`): **KILLED** (`exit=1`) — тест `привязывает contextmenu к целевому элементу, предотвращает default и открывает меню`;
- **baseline/restore:**
  - До и после всех 5 мутаций проверены контрольные суммы:
    - `rust/gui-spike/ui/features/context-menu.js`: `b780a292d01413ed1ae39c5b31b5f3f2`
    - `tests/unit/context-menu.test.mjs`: `f18187827859747d35408e964892ae84`
    - `rust/gui-spike/ui/styles/context-menu.css`: `f83e4b15e89f4d3c7c856d575147a5f4`
- **стабильность:** детерминированные синхронные DOM unit-тесты без сети и внешних таймеров;
- **gate:worker:** в соответствии с `PROCESS.md` §3.14 / §5 воркер сдаёт targeted red/green и typecheck; интеграционный гейт запускается на merge-лейне;
- **файлы (`md5sum`):**
  ```text
  b780a292d01413ed1ae39c5b31b5f3f2  rust/gui-spike/ui/features/context-menu.js
  f18187827859747d35408e964892ae84  tests/unit/context-menu.test.mjs
  f83e4b15e89f4d3c7c856d575147a5f4  rust/gui-spike/ui/styles/context-menu.css
  ca6e7b1d48509e454fed61e8c9dcb5f7  waves/RW-40/G2/task-620-context-menu/red.log
  435353a29dd2080d66b29a3eca9e078d  waves/RW-40/G2/task-620-context-menu/green.log
  1a3ba8fc8683d4a5f4289ee6b0e8c3d4  waves/RW-40/G2/task-620-context-menu/mutations.log
  ```
- **риски/вопросы:** отсутствуют; все существующие экспорты и контракты сохранены, регрессии нет;
- **следующий шаг:** передать задачу `TASK-620` на Product Review (PM) и Technical Review.
