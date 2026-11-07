# G-05-REST — меню сервера: отчёт G2 Frontend (Lane B)

Дата: 2026-10-30. Воркер: **G2 Frontend Lane B**. Карточка: **G-05-REST** (8 из 10 пунктов
меню). Контракт: `waves/RW-40/DESIGN/G05-REST-SPEC.md` rev. 4 (§2.3, §3.9, §3.10, §4,
§7, §9). База: HEAD `4e24048`, дерево dirty (чужие правки не трогались).

## 1. Статус

**done (frontend, offline).** Закрыты дефекты **D1–D7, D9** и UI-часть **D10** (текст
статуса без ложного успеха). Все 8 неактивных пунктов — честные `aria-disabled` с
машиночитаемой причиной (`data-disabled-reason`), `aria-describedby` на общий скрытый
узел и `title`; клик/Enter/Space по ним — полный no-op. Диалоги/URL/TTL/поля для пунктов
1, 3–8 **не выдумывались**: по §3.1 и §8 у них нет контракта записи (RE-2/G-06), поэтому
их «честное действие» сейчас — disabled с причиной, а не фальшивый успех.

`D8` (проекция `ownerId`/`userGuildSettings`/capability в `ready()`) — зона G1; UI готов
потреблять атрибуты `data-owner`, `data-lurking`, `data-capability-invite`,
`data-notifications-label`, `data-hide-muted-channels`. Сети, окна, cargo, git commit не
было.

## 2. Что сделано

| Дефект / требование | Код | Тест |
|---|---|---|
| **D1** disabled = no-op: guard в диспетчере `[data-action]` + в обработчике invite | `actions.js`: click-диспетчер, `guild-invite`, `"guild-leave"` | `D1: клик по каждому disabled-пункту…`, `Enter/Space на disabled…`, `capability=true не включает invite…` |
| **D2** roving `tabindex` переезжает с фокусом (↑/↓/Home/End) | `actions.js`: `setRoving`, keydown меню | `D2: ↑/↓/Home/End: tabindex=0 ровно у сфокусированного enabled` |
| **D3** Esc не переключает экран; Tab закрывает меню без `preventDefault`; клик вне не крадёт фокус у контрола | `actions.js`: `stopPropagation` в меню, ветка `Tab` в body, `isFocusableControl` | `D3: Esc … settings`, `Tab закрывает меню…`, два теста клика вне |
| **D4** #10: успех только из resolved `writeText`; нет API/reject/пустой id → видимая ошибка; id не логируется | `actions.js`: `copyGuildId` | 4 теста `D4: копирование ID…` |
| **D6** причина disabled: `data-disabled-reason` ∈ 3 значения, приоритет owner > capability-unknown > not-implemented, `aria-describedby` + `title` | `actions.js`: `DISABLED_REASON_TEXT`, `disabledReason`, `applyDisabledReason`, узлы `#menu-reason-*` | `D6: у каждого disabled-пункта…`, `приоритет причин…`, `один polite-статус…` |
| **D7** 429: контрол disabled, текст уменьшается раз в секунду до 0, затем один ручной повтор, авто-ретраев нет | `actions.js`: `startLeaveCooldown`, `stopLeaveCooldown`, guard в `confirmLeave` | `D7: 3 с → тик каждую секунду…` |
| **D9** геометрия: кламп ширины/высоты меню, разделитель 8/0 (17 между группами), двухстрочный 44, чекбокс 20×20, иконка 20 | `server-menu.css`: `.server-menu`, `.menu-item.has-subtitle`, `.menu-item-check`, `.menu-item-icon`, `.menu-sep` | `D9/D5: геометрия CSS…` (4 статических теста) |
| **D5** `.dialog` `max-height: calc(100vh - 48px)` + `overflow-y:auto`, backdrop прокручивается, ширина карточки не изменилась | `server-menu.css`: `.dialog`, `.dialog-backdrop` | `D5: диалог и backdrop прокручиваются…` |
| #4 subtitle только из проекции (#2.2) | `actions.js`: `refreshMenuState` + `has-subtitle` | `без данных subtitle не рендерится…` |
| #5 `aria-checked` из явной строки (`"false"` ≠ истина) + визуальный чекбокс | `actions.js`: `refreshMenuState`; CSS чекбокса | `aria-checked #5…`, `визуальные узлы D9…` |
| **T-05-14** типизированные ошибки leave: `client{status}`, `storage`, `invalidArg`, `missingToken`, `invalidResponse` больше не падают в «результат неизвестен» | `actions.js`: `leaveFailureText`, `statusOf` | `T-05-14: … видимый отказ`, `timeout… результат неизвестен` |
| #2 capability не выводится из ролей: false/true/absent → disabled | `actions.js`: `refreshMenuState` + guard | `T-05-08: false/true/absent…` |
| Один `#server-status`, disabled-активация его не пишет | `actions.js` | `T-05-16: статус меню единственный` |

Структурные добавления в DOM меню: `.menu-item-text`, `.menu-item-icon` (#10, глиф «ID» —
наш, ассеты Discord не копировались), `.menu-item-check` (#5, `aria-hidden`), скрытые
узлы причин `#menu-reason-not-implemented|capability-unknown|owner`. Разметка `index.html`
не менялась.

## 3. Red → green

| Фаза | Команда | Exit | Результат | Лог |
|---|---|---|---|---|
| RED | `node --test tests/unit/server-menu.test.mjs` | **1** | 27 тестов: 6 pass / **21 fail** | `waves/RW-40/PM/g05-rest/red.log` |
| GREEN | то же | **0** | 27 / 27 pass, 0 fail | `waves/RW-40/PM/g05-rest/green.log` |

Падения red — по заявленным причинам (§7.1), не по синтаксису: нет `data-disabled-reason`
(D6), не переезжает `tabindex` (D2), Tab не закрывает меню (D3), ложный успех копирования
при reject/без API (D4), кнопка активна после 429 (D7), нет subtitle из проекции/
`aria-checked`, нет узлов чекбокса/иконки, CSS без 44/20/8 и `max-height` (D9/D5),
типизированные ошибки leave уходят в «неизвестно» (T-05-14), invite при `capability=true`
срабатывает (D1).

## 4. Проверки

| Проверка | Команда | Exit | Результат | Лог |
|---|---|---|---|---|
| Targeted batch | `node --test gui-spike-features, gui-spike-dom, gui-spike-model, gui-spike-auth, gui-spike-regressions, join-server, guild-rail` | 0 | **246 / 246 pass** | `targeted.log` |
| Геометрия | `node waves/RW-39/PM/w39c/geometry-check.mjs` | 0 | 31/31 в допуске | `geometry.log` |
| tsc UI | `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **0** | 0 ошибок | `tsc-ui.log` |
| tsc actions.js отдельно | `npx tsc --ignoreConfig … features/actions.js` | **0** | 0 ошибок | `tsc-actions.log` |
| tsc root | `npx tsc -p tsconfig.json` | 1 | 24 ошибки — **все** в `tests/unit/composer.test.mjs` (чужой untracked WIP); в `actions.js`/`server-menu.test.mjs` **0** | `tsc.log` |

Атрибуция ошибок tsc проверена: `grep "error TS" tsc.log | sed 's/(.*//' | sort -u` →
только `tests/unit/composer.test.mjs`. Сам `features/composer.js` к концу прогона
владелец починил (`tsc-ui` снова 0). Пока шла работа, чужой `composer.js` дважды
пересохранялся в промежуточном состоянии: один прогон `node --test` по GUI-наборам падал
с `doc.addEventListener is not a function`, после сохранения — снова зелено. Это чужая
карточка (композер), к моим файлам отношения не имеет; после её merge прогон нужно
повторить.

`node --test tests/unit/` (весь unit) → 743 pass / 1 fail: единственный fail —
`tests/unit/composer.test.mjs` (нет экспорта `ATTACH_LIMITS`, чужой WIP). Мои наборы
зелёные.

Полный `gate:worker`/cargo/окно не запускались: по ТЗ карточки — только targeted + tsc;
cargo запрещён.

## 5. Изменение существующего теста (согласовано)

Разрешение PM на правку двух ассертов получено в сессии 2026-10-30. Основание:
G05-REST-SPEC §7.2 (строка «403/429») и §3.9/D7. Ослабления нет — проверок стало больше.

| Файл:строки | old | new | Основание |
|---|---|---|---|
| `tests/unit/gui-spike-features.test.mjs:3087-3088` | `disabled === null` («кнопка снова активна»); `calls === 2` (повтор сразу) | `disabled !== null` во время отсчёта; `calls === 2` без автоповтора; 3×`flushTimers()` → `disabled === null`; ручной клик → `calls === 3` | §3.9/D7: 429 блокирует до 0, затем ровно один ручной повтор |

Остальной файл не менялся (диф — ровно этот блок). Subtitle-ассерт (`:2896-2899`) не
трогался: узел `.menu-item-subtitle` сохранён как hidden-fallback, поэтому старый тест
остаётся зелёным, а без проекции вторая строка не рендерится (см. §7 риск 4).

## 6. Файлы и md5

```
4effc2bec384b30ba074bb26933a6863  rust/gui-spike/ui/features/actions.js
6825a2b81d010e931337549ddc7648ae  rust/gui-spike/ui/components/server-menu.css
83ed20625a33779ea173431c3d0da836  tests/unit/server-menu.test.mjs
3c85261013fe463427ef59c1ea4bf6a8  tests/unit/gui-spike-features.test.mjs
ec1887389098c5750ce0bdfd84e61e5b  waves/RW-40/PM/g05-rest/green.log
3f904e8fed55d45f04968de0a53eb8a4  waves/RW-40/PM/g05-rest/red.log
68db6fe66a42f4be9d112cfe3786c927  waves/RW-40/PM/g05-rest/targeted.log
a41824cb3720e9d207fb05972c9fb03e  waves/RW-40/PM/g05-rest/geometry.log
db1632b9563db0df8c8c43226228d61c  waves/RW-40/PM/g05-rest/tsc-ui.log
c4e901aa112f39365f2493734fe9d8f9  waves/RW-40/PM/g05-rest/tsc.log
8fa53252b1a404d87edd3ab36d3b547b  waves/RW-40/PM/g05-rest/tsc-actions.log
```

Не менялись: `app.js`, `index.html`, `chat.js`, `quick-switcher.js`, `composer.js`,
`search.js`, `tokens.css`, `tests/support/gui-dom-harness.mjs`, `src/**`.

## 7. Риски и блокеры для приёмки

1. **`plan-sync` красный.** `tests/unit/server-menu.test.mjs` — новый файл, строки в
   `TDD-PLAN.md §8` нет (в дереве её тоже не было). `TDD-PLAN.md` — зона TL; готовую
   строку нужно добавить:
   `` | `tests/unit/server-menu.test.mjs` | — | `server-menu`, `data-disabled-reason`, `menu-reason`, `guild-copy-id`, `leave_guild` | ``.
   Сейчас meta-тест дополнительно падает на чужом `tests/unit/composer.test.mjs`.
2. **`tsc`/gate заблокированы чужим WIP.** `tests/unit/composer.test.mjs` (untracked,
   16:37) даёт 24 ошибки и роняет `npx tsc -p tsconfig.json`; его же набор падает в
   полном `node --test tests/unit/`. `tsc-ui` и `tsc` по `actions.js` — 0 ошибок. Нужен
   повтор после merge владельца композера.
3. **D10 — носитель видимого статуса не решён** (§0.4, §8 Q8). `#server-status` остаётся
   `visually-hidden`; T-05-06 проверяет корректность текста и отсутствие ложного успеха,
   но «видимое подтверждение» зрячему пользователю не заявляется выполненным. Toast/строку
   не выдумывал.
4. **Subtitle #4.** Статичный текст в `SERVER_MENU_SPEC` сохранён как скрытый fallback
   ради совместимости существующего ассерта. Видимо он не рендерится без
   `data-notifications-label`. Если PM хочет буквальное «узла нет» — нужна правка
   `gui-spike-features.test.mjs:2896-2899` по §7.2 (сейчас не согласована).
5. **Новые `[A]`-строки** для типизированных ошибок (§5 требует утверждения):
   «Сервер отклонил запрос (код {n}). Вы остаётесь на сервере.», «Хранилище токена
   недоступно. Войдите снова.», «Внутренняя ошибка. Плитка сервера сохранена.»
   (`missingToken` переиспользует «Сессия истекла. Войдите снова.»).
6. **Интеграция проекции G1.** UI читает `data-notifications-label` и
   `data-hide-muted-channels` с `#server-menu` (строки `"true"`/`"false"`), но `app.js`
   их пока не ставит — это D8/зона G1. До проекции #4/#5 выглядят честно «без данных».
7. **#2 включится только с G-06**: при `capability=true` причина меняется на
   `not-implemented`, диалог/URL/код/TTL не создаются.

Не запускались: `cargo`, `npm run gate:worker`, окно/live, сеть, `git commit/push`.
