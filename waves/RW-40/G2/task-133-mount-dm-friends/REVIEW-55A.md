# Review 55A — task-133 / FE-P0-01: Монтаж `createDmFriendsController` в `app.js` и DOM-хостов в `index.html`

- **Дата**: 2026-10-30
- **Ревьюер**: Технический ревьюер (не автор кода)
- **Worktree**: `/home/sanya/orca/workspaces/project/feature-task-133-mount-dm-friends-app`
- **Ветка**: `feature/task-133-mount-dm-friends-app` (`HEAD 4e24048`)
- **Карточка**: `task-133` (`FE-P0-01` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Оракулы**: `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md`, `waves/RW-40/DM-FRIENDS-RE.md`

Продуктовый код и тесты при ревью не изменялись: выполнялись только чтение, прогоны тестов и `tsc`, а также 6 мутационных проверок с немедленным восстановлением исходников и сверкой MD5 до и после каждого прогона. Живой Discord и внешние процессы не запускались.

---

## Вердикт

**PASS — Ready for Merge.** `P0 = 0`, `P1 = 0`, `P2 = 3`, `P3 = 2`.

Все 5 обязательных DOM-хостов (`#home-rail-btn`, `#dm-channel-list`, `#friends-view`, `#group-dm-dialog`, `#stranger-confirm-dialog`) смонтированы в `rust/gui-spike/ui/index.html` и синхронизированы с `tests/support/gui-dom-harness.mjs` без нарушения замороженных инвариантов (`4` экрана `.screen`, `38` тумблеров `.toggle`, `gui-spike-dom.test.mjs`, `guild-rail.test.mjs`, `INT-07` и `ipc-path.test.mjs`). Контроллер `createDmFriendsController` подключён в `rust/gui-spike/ui/app.js` с поддержкой входа/выхода из режима `Home ("@me")`, персистентности `lastGuildId: "@me"` / `homeMode: true`, гидратации `users` / `relationships` / `private_channels` / `presences` из `ready()` и открытия каналов ЛС через `chat.openChannel`.

---

## 1. Сводка проверок и артефактов

| Проверка | Результат |
|---|---|
| `node --test tests/unit/gui-spike-features.test.mjs tests/unit/dm-friends-ui.test.mjs tests/meta/ipc-path.test.mjs` | exit 0, **200 passed / 0 failed / 0 skipped** |
| Расширенный прогон (`+ gui-spike-dom.test.mjs`, `guild-rail.test.mjs`, `test-policy.test.mjs`) | exit 0, **225 passed / 0 failed / 0 skipped** (30 suites) |
| `npx tsc -p tsconfig.json` | exit 0, `0 errors` |
| `npx tsc -p tsconfig.agent.json` | exit 0, `0 errors` |
| Мутационные проверки (`app.js` + `index.html`) | **5/5 требуемых + 1 бонусная = 6/6 KILLED** (`review-55a-mutation.log`) |
| Проверка `TEST-POLICY` (`.only` / `.skip` / `it.todo`) | чисто (`0` совпадений) |
| Сверка MD5 всех 4 файлов с `REPORT.md` | **4/4 совпадают до байта** |
| Проверка Red-first (`red.log` → `green.log`) | `red.log`: 12 падений в сьюте `FE-P0-01 / task-133` (`111 pass / 12 fail`); `green.log`: `209 pass / 0 fail` |

### MD5 проверенных файлов и логов

| Файл / Артефакт | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/ui/app.js` | `e83caa7d335328eeb06599d5cd4093e1` | совпадает с `REPORT.md` |
| `rust/gui-spike/ui/index.html` | `04f3e0d3d532219b8187938172b6fc50` | совпадает с `REPORT.md` |
| `tests/support/gui-dom-harness.mjs` | `9e3f4f81953ba4e8caf7992b790ef60d` | совпадает с `REPORT.md` |
| `tests/unit/gui-spike-features.test.mjs` | `116bed757c1bb53e6ac9965dfd680805` | совпадает с `REPORT.md` |
| `waves/RW-40/G2/task-133-mount-dm-friends/red.log` | `384728a269d2a761837bd61726a4c64c` | проверен |
| `waves/RW-40/G2/task-133-mount-dm-friends/green.log` | `f989a8403b994b55002d6b027bd77e33` | проверен |
| `waves/RW-40/G2/task-133-mount-dm-friends/review-55a-mutation.log` | `a8c4e00e573bdc1ddbe808daa780eb97` | создан ревьюером |

---

## 2. Технический разбор изменений

### 2.1 `rust/gui-spike/ui/index.html` и `tests/support/gui-dom-harness.mjs`
1. **5 обязательных хостов `DM-FRIENDS-SPEC.md`**:
   - `#home-rail-btn` (`button.brand.home-rail-btn[type="button"][aria-label="Личные сообщения и Друзья"][aria-current="false"][tabindex="-1"]` с `.brand-monogram` и `.guild-mention-badge[hidden]`) заменил статический `div.brand` в `#guild-rail` (`index.html:116`, `gui-dom-harness.mjs:820-835`).
   - `#dm-channel-list` (`ul.channel-list.dm-channel-list[role="list"][hidden]`) добавлен в `#channel-tree .tree-scroll` (`index.html:155`, `gui-dom-harness.mjs:905-916`).
   - `#friends-view` (`section.friends-view[role="region"][aria-label="Друзья"][hidden]`) размещён в `main.content` рядом с 4 экранами `.screen`, но без класса `.screen` — инвариант `screenCount === 4` сохранён (`index.html:267`, `gui-dom-harness.mjs:1118-1124`).
   - `#group-dm-dialog` и `#stranger-confirm-dialog` (`div.dialog-backdrop[role="presentation"][hidden]`) добавлены в `body` (`index.html:298-299`, `gui-dom-harness.mjs:1152-1166`).
2. **`ShimElement.insertBefore` (`gui-dom-harness.mjs:321-327`)**:
   - Реализован метод `insertBefore(child, reference)` на `ShimElement`, необходимый для корректной вставки узлов в `dm-friends.js` и `createGuildRail.ensureList()`.

### 2.2 `rust/gui-spike/ui/app.js`
1. **Персистентность выбора Home (`readLastGuildId`, `app.js:370-375`)**:
   - Принимает `value === HOME_GUILD_ID` (`"@me"`) или `bag.homeMode === true` из ответа `settings_load`, возвращая `"@me"`.
2. **Интеграция с `createGuildRail` (`app.js:391-550`, `648-653`, `734-744`, `1482-1508`)**:
   - Метод `selectHome(origin, focus)` снимает `.is-selected` и `aria-current` со всех плиток гильдий, сохраняет roving `tabindex="0"` на последней активной плитке гильдии (`lastGuildIndex`), активирует `#home-rail-btn` (`.is-selected`, `aria-current="page"`, `tabindex="0"`), вызывает `onEnterHome()` → `dmFriendsController?.selectHome()` и при `origin === "user"` сохраняет выбор через `onChoose("@me")` → `rememberGuild("@me")`.
   - При выборе гильдии (`select(id, ...)`) и выходе из Home вызывается `onLeaveHome(id)` → `dmFriendsController?.selectGuild(id)` и сбрасывается состояние `#home-rail-btn`.
   - При `rail.load()` вызывается `onReadyPayload(payload)` → `hydrateDmFriendsFromReady(payload)`, а если `preferred === "@me"` (включая случай пустого списка гильдий `items.length === 0`), рейл восстанавливает режим Home.
3. **Сохранение `#dm-channel-list` в `createChannelTree` (`app.js:1116-1210`)**:
   - При перерисовках серверного дерева (`renderTree`, `renderEmpty`, `renderError`, `renderLoading`) в режиме `"home"` (`tree.getAttribute("data-nav-mode") === "home"`) перерисовка пропускается, чтобы не затереть DOM-дерево Home, а в режиме `"guild"` после `scroll.replaceChildren()` вызывается `appendHiddenDmList()`, чтобы `#dm-channel-list[hidden]` всегда оставался в DOM.
4. **Монтирование `createDmFriendsController` и открытие чата ЛС (`app.js:1872-2047`)**:
   - `mountDmFriends(readyPayload)` нормализует `users`, `relationships`, `private_channels` / `privateChannels` и `presences` (поддерживая как массив, так и словарь `presences`), передаёт их в `createDmFriendsController` и подключает `onSelectChannel`.
   - В `onSelectChannel` переключение на канал ЛС переводит роутер на `"chat"`, скрывает `#friends-view`, показывает `#chat-header` и `#screen-chat.active` и открывает сообщения через `chat.openChannel({ id: channelId, name: channel?.name ?? channelId })`.
   - Экспортирован `currentApp().dmFriends` (Proxy к актуальному инстансу контроллера + тестовые хуки `setRestHandler` / `getRestCalls`).

### 2.3 `tests/unit/gui-spike-features.test.mjs`
- Добавлен сьют `FE-P0-01 / task-133: монтирование Home / ЛС / друзей в app.js и index.html` из 12 поведенческих тестов (`:4313-4814`), проверяющих наличие 5 хостов в `index.html` и DOM, клик и клавиатуру (`Enter`) на `#home-rail-btn`, возврат на гильдию, восстановление из `lastGuildId: "@me"` и `homeMode: true`, рендер данных `ready()`, открытие канала ЛС (`messages_page`), возврат в `#friends-view` через `#dm-nav-friends`, фильтрацию вкладок/поиска, форму добавления в друзья с кодом ошибки `80000`, модалки `#group-dm-dialog` и `#stranger-confirm-dialog` (код `80013`) и отсутствие `console.error` / `console.warn`.

---

## 3. Мутационные проверки (6/6 KILLED)

Полный журнал сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-133-mount-dm-friends/review-55a-mutation.log`. После каждой мутации файл восстанавливался и сверялся по MD5 (`e83caa7d335328eeb06599d5cd4093e1` для `app.js`, `04f3e0d3d532219b8187938172b6fc50` для `index.html`).

| # | Файл и место | Мутация | Статус | Убившие тесты |
|---|---|---|---|---|
| **M1** | `app.js:373` (`readLastGuildId`) | Удалена ветка `if (value === HOME_GUILD_ID \|\| bag.homeMode === true) return HOME_GUILD_ID;` | **KILLED** (exit 1, fail 2) | `FE-P0-01` тесты 4 и 5 (восстановление Home при старте из `lastGuildId='@me'` и `homeMode: true`) |
| **M2** | `app.js:1500` (`onEnterHome`) | Отключён вызов `dmFriendsController?.selectHome()` в `onEnterHome` | **KILLED** (exit 1, fail 2) | `FE-P0-01` тесты 4 и 5 (`#channel-tree` не переходит в `data-nav-mode="home"` при восстановлении) |
| **M3** | `app.js:1503` (`onLeaveHome`) | Отключён вызов `dmFriendsController?.selectGuild(guildId)` в `onLeaveHome` | **KILLED** (exit 1, fail 1) | `FE-P0-01` тест 3 (возврат из Home на гильдию не переключает `data-nav-mode="guild"` и не скрывает `#friends-view`) |
| **M4** | `app.js:1991-1992` (`mountDmFriends`) | Подмена `relationships` и `privateChannels` на `[]` при передаче в `createDmFriendsController` | **KILLED** (exit 1, fail 5) | `FE-P0-01` тесты 6, 7, 8, 9, 11 |
| **M5** | `app.js:2004` (`onSelectChannel`) | Отключён вызов `chat.openChannel({ id: channelId, ... })` при выборе канала ЛС | **KILLED** (exit 1, fail 1) | `FE-P0-01` тест 7 (`messages_page` не вызывается для открытого канала ЛС) |
| **M6** *(бонус)* | `index.html:267` | Замена `id="friends-view"` → `id="friends-view-broken"` в статической разметке | **KILLED** (exit 1, fail 2) | `gui-spike-dom.test.mjs` («шим-разметка совпадает с ui/index.html») и `FE-P0-01` тест 1 |

---

## 4. Находки и замечания

### P0 — нет (0)

### P1 — нет (0)

### P2 (не блокируют мерж карточки)

- **P2-1. Небольшая статика/рантайм-асимметрия атрибута `data-guild-id="@me"` на `#home-rail-btn` (`index.html:116` vs `gui-dom-harness.mjs:824`).**
  В `tests/support/gui-dom-harness.mjs:824` атрибут `data-guild-id="@me"` задаётся при создании кнопки `#home-rail-btn`, тогда как в `rust/gui-spike/ui/index.html:116` он не прописан в статическом HTML и выставляется синхронно при `boot()` через `createDmFriendsController` (`ensureDom()`). В рантайме и тестах оба дерева идентичны (`data-guild-id="@me"` и `data-action="nav-home"` присутствуют сразу после `boot()`), но при последующей синхронизации разметки `data-guild-id="@me"` и `data-action="nav-home"` можно прописать и в статике `index.html`.
- **P2-2. Локальный REST-адаптер `sendDmRest` в `rust/gui-spike/ui/app.js:1901-1942`.**
  Поскольку выделенные Tauri IPC-команды для мутаций DM/relationships ещё не зарегистрированы в `generate_handler!` (`rust/gui-spike/src/main.rs`), а мета-тест `INT-07` (`gui-spike-features.test.mjs:4193-4230`) строго проверяет `UI invoke ⊆ generate_handler!`, `sendDmRest` в `app.js` работает как сессионный/in-memory адаптер (с возможностью подмены через `dmFriends.setRestHandler` в тестах). Это явно отражено в `REPORT.md` §4 и архитектурно согласовано с аналогичным решением для `voice-devices` в `INT-07`. После регистрации соответствующих команд в `main.rs` достаточно переключить `sendDmRest` на `invoke(...)`.
- **P2-3. Отсутствие `<link rel="stylesheet" href="components/dm-friends.css" />` в `index.html`.**
  В `DM-FRIENDS-SPEC.md` §0.2 п.5 описано подключение `components/dm-friends.css`, однако сам CSS-файл в этой ветке ещё не создан (отдельная карточка стилей). Неподключение несуществующего файла в `index.html` в рамках `task-133` — верное решение (иначе WebView получал бы 404).

### P3 (косметика)

- **P3-1. Двойной слушатель `click`/`keydown` на `#home-rail-btn` (`app.js:2011-2019` и `features/dm-friends.js:658-669`).**
  И сам `createDmFriendsController`, и `mountDmFriends` в `app.js` вешают обработчик клика на `#home-rail-btn`. В результате клик пользователя вызывает `dmFriendsController.selectHome()` напрямую и повторно через `rail.selectHome("user", false)` → `onEnterHome()`. Так как `selectHome()` в `dm-friends.js` идемпотентен, побочных эффектов нет, но при чистке кода достаточно одного маршрута через `rail.selectHome`.
- **P3-2. Пересоздание `createDmFriendsController` в `mountDmFriends(payload)` при `ready()` (`app.js:1948-2023`).**
  На старте `boot()` контроллер создаётся с пустыми коллекциями (`mountDmFriends(null)`), а после ответа `ready()` пересоздаётся с данными (`hydrateDmFriendsFromReady`). Слушатели на `#home-rail-btn` при этом корректно снимаются через массив `homeBtnListeners`, а `wasHome` восстанавливается. В будущем метод `controller.hydrate(payload)` позволил бы обновлять стор без пересоздания инстанса.

---

## 5. Рекомендация

**PASS / Ready for Merge.** Требования карточки `task-133` (`FE-P0-01`) выполнены полностью, TDD-цикл и 6/6 мутационных проверок подтверждены, регрессий в существующих unit- и meta-тестах нет.
