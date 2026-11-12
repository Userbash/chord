# REPORT: `DESIGN-P1-18` (`task-275-app-shell-wiring-spec`) — App Shell Wiring & Composer Popovers UX/DOM Spec

## 1. Batch / Карточка
- **Волна / Отдел**: `RW-40` / `G3 Design (UX/UI & Specs)`
- **Задача**: `DESIGN-P1-18` (`task-275-app-shell-wiring-spec`, Orca Task ID: `task_0b486e56c15e`, Dispatch ID: `ctx_a25a8d3c7193`)
- **Рабочая ветка**: `design/task-275-app-shell-wiring-spec` (базовый `HEAD`: `b35c3e1120db4ae42c0b0da15649a5d83baaa224`)
- **Статус**: Выполнено (`Ready for Product Review` / `Ready for Technical Review`)

---

## 2. Сделано
1. **Нормативная UX/DOM/A11y спецификация (`docs/specs/APP-SHELL-WIRING-SPEC.md`, 721 строка)**:
   - Спроектирована и верифицирована против эталонного RE-дампа `reference-oracle/frontend-d03.md` (`§2`, `§4`, `§5.5–§5.12`, `§8`), `reference-oracle/ru-catalog.json` и `rust/gui-spike/ui/tokens.css` сквозная интеграция 5 верхнеуровневых модулей оболочки:
     - `rust/gui-spike/ui/features/app-shell-wiring.js` (`createAppShellWiring`);
     - `rust/gui-spike/ui/features/shell-controllers-registry.js` (`GUILD_ADMIN_PANELS`, `createGuildAdminPanelRouter`);
     - `rust/gui-spike/ui/features/chat-helpers-bridge.js` (`buildChannelHeaderBadgesModel`, `formatChannelTopicTooltip`);
     - `rust/gui-spike/ui/features/composer-popovers-bridge.js` (`COMPOSER_POPOVER_LAYERS`, `createComposerPopoversBridge`);
     - `rust/gui-spike/ui/features/shell-bridge.js` (`SHELL_MODAL_LAYERS`, `EXCLUSIVE_MODAL_LAYERS`, `createShellFeatureBridge`).
   - Зафиксирована шестиуровневая шкала наложения слоёв `Z-Index` (`--z-base: 0`, `--z-header: 10`, `--z-popover: 100`, `--z-modal: 200`, `--z-tooltip: 300`, `--z-toast: 400`) и таблица биекции между каноническими именами ТЗ (`quick_switcher`, `notification_inbox`, `pins_drawer`, `threads_drawer`, `user_profile_card`) и полноэкранными/региональными панелями гильдий (`roles_matrix`, `audit_log`, `automod`, `scheduled_events`, `webhooks`, `expressions`, `moderation`, `onboarding`), включая двунаправленное взаимное вытеснение (`openModalLayer` закрывает `activeAdminPanel`, `openAdminPanel` закрывает `activeOverlayStack` в LIFO-порядке).
   - Специфицирован конечный автомат взаимного вытеснения поповеров композера (`slash_command_picker`, `soundboard_picker`, `emoji_picker`, `gif_picker`, `attachment_dropzone`), правила авто-детекта черновика `/command` (`^\/([^\s/]+)$`), двухприоритетное ранжирование слэш-команд (`startsWith` -> `includes`, `limit = 10`), лимиты громкости `0..200%` и пакетов вложений (`<= 10` файлов, `<= 25 МБ`).
   - Зафиксирована полная матрица клавиатурной навигации (`Ctrl+K`, `Ctrl+F`, `Ctrl+P`, `Ctrl+/`, `Alt+Shift+ArrowUp`, циклические `ArrowUp`/`ArrowDown`, `Tab`, `Enter`, приоритетный `Escape`), каноническая DOM-разметка, таблица атрибутов WAI-ARIA 1.2 (`role="dialog"`, `aria-modal`, `aria-expanded`, `aria-activedescendant`, `role="listbox"`, `role="option"`, `aria-selected`, `role="switch"`, `aria-checked`, `role="alert"`, `role="status"`), адаптивные брейкпоинты (`1024px`, `1280px`, `1440px`, `<720px`, `959px`, `1600px`), матрица 5 состояний (`Empty`, `Loading`, `Error`, `Populated`, `Capabilities`), 9 обязательных измерений тестовых оракулов (`T-SHELL-WIRING-{NEG,BND,ERR,EMP,LOAD,CAP,REC,CMP,OBS}-01`) и 6 мутационных оракулов (`M-SHELL-WIRING-01..06`).
2. **Реестр CSS-токенов (`/home/user/orca/project/waves/RW-40/DESIGN/APP-SHELL-WIRING-TOKENS.md`, 129 строк)**:
   - Оформлена полная таблица всех CSS-переменных из `rust/gui-spike/ui/tokens.css` (Z-index слои, геометрия `--layout-*`, `--modal-w-*`, `--emoji-picker-*`, `--badge-*`, `--tooltip-*`, палитра поверхностей и текста, границы, скругления, тени, спейсинг, анимации), адаптивные переопределения на брейкпоинтах `1024px`, `1280px`, `1440px`, `959px`, `1600px` и таблица расчёта контрастности WCAG 2.1 AA (`>= 4.5:1` текст, `>= 3.0:1` UI).

---

## 3. Red-прогон (проверка до синхронизации имен токенов с `tokens.css`)
На этапе первичной механической сверки (`LC_ALL=C comm -23`) между списком `var(--*)` черновика спецификации и `rust/gui-spike/ui/tokens.css` было выявлено расхождение имен алиасов (`33` токена старой номенклатуры).

---

## 4. Green-прогон и механический дизайн-гейт (`T0`)
После приведения всех CSS-переменных к точному словарю `rust/gui-spike/ui/tokens.css` выполнен механический дизайн-гейт:

```text
=== 1. Raw colors / !important / :has( / fallback inside CSS blocks ===
0
=== 2. Undeclared var(--*) against rust/gui-spike/ui/tokens.css ===
0
=== 3. Test oracle count (9 dimensions) ===
9
=== 4. Mutation count (>= 5) ===
6
```

---

## 5. Targeted / Gate
- Задача `docs-only` (`G3 Design`): продуктовый код `rust/` и `rust/gui-spike/ui/*.js` не изменялся.
- Проверены все механические инварианты дизайн-гейта (`0` сырых цветов `#hex`/`rgb`/`hsl` в CSS-блоках, `0` `!important`, `0` `:has()`, `0` `var(--x, fallback)`, `0` необъявленных `var(--*)` относительно `rust/gui-spike/ui/tokens.css`, `9/9` измерений тестовых оракулов, `6/6` мутаций).

---

## 6. Файлы и контрольные суммы (`md5sum`)

| Файл | Назначение | Строк | `md5sum` |
| :--- | :--- | :---: | :--- |
| `docs/specs/APP-SHELL-WIRING-SPEC.md` | Нормативная UX/DOM/A11y/CSS спецификация интеграции оболочки и поповеров композера | `721` | `7c1010344ae3e4a8b5f01b7d8c68bd84` |
| `/home/user/orca/project/waves/RW-40/DESIGN/APP-SHELL-WIRING-TOKENS.md` | Полный реестр CSS-токенов, адаптивных брейкпоинтов и контраста WCAG 2.1 AA | `129` | `84f323f410b99766aaaab45d99743573` |
| `waves/RW-40/DESIGN/APP-SHELL-WIRING-TOKENS.md` | Копия реестра CSS-токенов в рабочей ветке `design/task-275-app-shell-wiring-spec` | `129` | `84f323f410b99766aaaab45d99743573` |
| `rust/gui-spike/ui/tokens.css` (read-only оракул) | Единый источник CSS-переменных | `392` | `21d570a6bf9c4b5d77b00d3d9b3d1d08` |

---

## 7. Риски и открытые вопросы
- **Риски**: отсутствуют (`P0 = 0`, `P1 = 0`). Все 100% используемых CSS-переменных уже объявлены в `rust/gui-spike/ui/tokens.css`, а контракты слоёв полностью согласованы с `app-shell-wiring.js`, `shell-controllers-registry.js`, `chat-helpers-bridge.js`, `composer-popovers-bridge.js` и `shell-bridge.js`.
