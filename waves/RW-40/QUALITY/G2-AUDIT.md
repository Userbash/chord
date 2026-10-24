# G2-AUDIT — frontend/UI и тесты (read-only)

Дата: 2026-10-24. Автор: G2-аудитор (subagent). PM: DeepSeek.
Задача: `Q-AUDIT-G2`. Режим: **только чтение**, без live/сети/окна, без правок кода и тестов.
HEAD: `0239543` (workspace грязный — чужие изменения не трогались).
Источник требований: `waves/RW-40/QUALITY/STRATEGY.md` §2/§9/§10, `QUALITY/BASELINE-QUALITY.md`,
`TEST-POLICY.md`, `waves/RW-40/PARITY/DESIGN-PARITY.md`.

## 0. Проверки (факт, exit code, лог)

| Команда | Результат | Лог |
|---|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-auth.test.mjs` | exit **0**, tests **212**, pass **212**, fail **0**, skip **0**, 1333 ms | `waves/RW-40/PM/qa-g2/node-test.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit **0**, вывод пустой | `waves/RW-40/PM/qa-g2/tsc.log` |

Дополнительно (не входит в гейт, но воспроизводимо): два read-only пробника на harness
(`/tmp/opencode/g2-probe.mjs`, `/tmp/opencode/g2-probe2.mjs`) подтвердили G2-01, G2-02,
G2-03, G2-05, G2-06 эмпирически (выводы процитированы в находках; в репозиторий не кладутся).

Вывод по гейтам: **212/0 и tsc 0** — регрессий нет; аудит ищет долг, а не падения.
`P0` в границах G2 **не найдено**: `innerHTML/outerHTML/eval` в UI нет, единственный
динамический `href` проходит whitelist `http(s):` (`embeds.js:37-47`), секреты в DOM/логах
не найдены (тесты `assertSecretGone`). Ниже — P1/P2/P3.

## 1. Метрики production-модулей (измерено)

| Файл | Строк | Байт | Функц. единиц* | `addEventListener` | `querySelector*` | Ответственностей (наблюдение) |
|---|---:|---:|---:|---:|---:|---|
| `features/search-overlay.js` | 1049 | 43 446 | ~55 | 14 | 1 | 5: DOM-шапка+7 секций, локальный скан, серверный fetch/state, нормализация ответа, сортировки |
| `features/settings.js` | 746 | 32 816 | ~34 | 9 | 0 | 4: таблицы-спеки, DOM-сборка 3 панелей, hydrate/save+debounce, nav |
| `features/chat.js` | 577 | 25 102 | ~21 | 3 | 0 | 4: генерация строк, виртуализация/tombstone, отправка+optimiistic, hover-toolbar wiring |
| `features/auth.js` | 241 | 26 659 | ~31 | 12 | 2 | 5: DOM-сборка, state-machine, пароль/token/restore, MFA, captcha/ошибки — **в 6 строках 5 300+ символов** |
| `features/members.js` | 452 | ~17 КБ | ~9 | 3 | 1 | 3: экран, 6 групп, панель |
| `features/emoji.js` | 520 | 21 КБ | ~13 | 5 | 3 | 2: данные 10 категорий, пикер |
| `app.js` | 304 | 12 КБ | ~15 | 0 | 6 | сборка + 2 диагностических прогона |

\* `rg '^\s*(async )?function | const x = (…)=>'` — включая inline-стрелки; не «сложность».

Худшая читаемость не в LOC, а в **плотности**: `auth.js:120` — 1622 символа, `:123` — 903,
`:182` — 993, `:235` — 764. Это один `setState`/`fail`/`selectMethod`, где смешаны 7 состояний,
disabled-атрибуты, фокус и тексты. Юнит-тесты это терпят, человек — нет.

## 2. Находки

Формат: `ID | file:line | симптом | почему плохо | P | минимальный фикс | тест | выигрыш`.
Тесты — **red-first/characterization**, без правки ожиданий под код.

### 2.1 P1

**G2-01 | `features/composer.js:132-136` | любое нажатие клавиши при активном reply отменяет reply**
`onKeydown`: после ветки `Enter` идёт `if (reply) { clearReply(); return; }` без проверки
`event.key === "Escape"`. Пробник: `data-reply=true` → `keydown {key:"x"}` → `data-reply=false`.
Почему плохо: пользователь выбирает «Ответить», начинает печатать — режим ответа молча
пропадает, сообщение уйдёт не в ветку; регрессию не ловит ни один тест (проверяется только клик).
P1. Фикс: `if (event.key === "Escape" && reply) { clearReply(); return; }`; остальные клавиши
не трогают reply (Enter уже обработан выше).
Тест: hover→«Ответить»→`composer-input` `keydown {key:"a"}` → `data-reply` остаётся `"true"`;
`Escape` → `"false"`; после отправки reply очищается.
Выигрыш: 1 условие; закрывает потерю состояния в основном действии чата.

**G2-02 | `app.js:52-56`, `features/errors.js:70-93`, `components/primitives.css:135-147`, `style.css:101` | глобальный error-overlay терминальный: полноэкранный, без закрытия, не снимается при успехе**
`surface()` кладёт `.error-state` в `.content`; `.error-state{position:absolute;inset:0}`, а у
`.content` нет `position:relative` → containing block = viewport. `clearErrorState` вызывается
**только** из `showErrorState`. Пробник: 401 поиска → `.error-state=1`; успешный локальный
поиск → всё ещё `1`. Текст подсказки «…или перезапустите спайк» и тесты
(`features:1046,1236`) показывают, что оверлей задуман как аварийный экран — но он же
используется для **восстановимых** ошибок (401/429/сеть в поиске, отправке, реакциях) и не
имеет dismiss: любая такая ошибка навсегда накрывает UI до следующей ошибки. Это превращает
штатный retry-сценарий в перезапуск приложения.
P1. Фикс (минимальный): разделить fatal/recoverable: recoverable — строка статуса/тост со
своим контейнером; fatal — оверлей + кнопка «Закрыть» (`clearErrorState(content)` по успеху и
по клику); `.content{position:relative}` — чтобы оверлей не выходил за контент.
Тест (characterization): ошибка → есть `.error-state`; клик «Закрыть» → 0; ошибка → успешный
поиск/отправка → 0; отдельный meta/geom-ассерт, что `.error-state` не выходит за
`getBoundingClientRect(.content)`.
Выигрыш: восстановление после ошибки без перезапуска; −1 скрытый глобальный блокер.

**G2-04 | `features/actions.js:205-210,233-248`; `features/auth.js:38`; `index.html:175` | модальный auth-гейт без focus-trap; Tab уходит в `aria-hidden` shell**
`topLayer()` возвращает узел только для `settings`/`emoji`; при `router.current==="chat"` (а
auth-гейт живёт поверх chat) `trapTab` ничего не делает и не `preventDefault`. `#auth-gate`
имеет `role=dialog aria-modal=true`, а `.shell` в это время `aria-hidden=true` — фокус уходит
в скрытое для AT дерево. Это нарушение и логики модалки, и «AT не должен фокусировать hidden».
P1 (доступность/клавиатура). Фикс: `topLayer()` добавляет
`document.querySelector('#auth-gate:not([data-state="ready"])')`; при открытом гейте —
`document.querySelector(".shell")?.setAttribute("inert","")` (или `tabindex=-1` на один узел,
если inert нет в шиме) и `preventDefault` на Tab.
Тест: `no-session` → focus login → 30×Tab → `activeElement` внутри `#auth-gate`; после `ready`
Tab снова ходит по shell; Tab не попадает в `.shell` при `aria-hidden`.
Выигрыш: один общий путь focus-trap вместо второго ad-hoc цикла; закрывает WCAG-риск.

### 2.2 P2

**G2-03 | `features/quick-switcher.js:100-101` | empty-state инвертирован**
`if (visible.length > 0) emptyNode.removeAttribute("hidden"); else setAttribute("hidden","")`.
Пробник: 8 options → «Ничего не найдено» **видно** (`hidden=null`); 0 options → **скрыто**.
Почему плохо: прямое противоречие состоянию (не блокирует, но вводит в заблуждение); тест
`gui-spike-features.test.mjs:208` проверяет только `textContent`, а не видимость, поэтому баг
зелёный. P2. Фикс: поменять ветви местами.
Тест: с результатами `empty.hidden !== null`; при 0 — `hidden === null`; плюс проверка
`hidden` у `.quick-switcher-hint` не меняется.
Выигрыш: 2 строки, закрывает целый класс «empty-состояние врёт».

**G2-05 | `features/search-overlay.js:1004-1011` | `applyFilter(patch)` игнорирует всё, кроме `date`**
JSDoc обещает `Partial<Filters>`, фактически пишется только `date`, затем `refresh()` читает
контролы. Пробник: `applyFilter({from:"zzz-no-such-author"})` → `matches 400→400`, поле `from`
пустое. API не используется `app.js` и не покрыт тестами (grep: только объявление).
P2 (публичная диагностическая поверхность молчит). Фикс: для каждого ключа patch писать в
соответствующий контрол (`text/select` → `.value`, `pinned` → `aria-checked`) и один `refresh()`;
либо удалить метод как мёртвый (решение PM). Тест: `applyFilter({from})` → `matches` падает,
поле заполнено; `applyFilter({pinned:true})` → все matches с `pinned`; невалидная дата →
`UIError` + `data-state=error`.
Выигрыш: либо −8 LOC мёртвого API, либо +8 LOC честного контракта.

**G2-06 | `features/actions.js:123-127` | `aria-expanded` кнопки эмодзи не сбрасывается при закрытии**
`emoji` ставит `true`, `emoji-close` только `switchScreen("chat")`. Пробник: open/close →
`"true"/"true"`. P2 (ложное состояние для AT). Фикс: в `emoji-close` (и в
`closeTopLayer`, если текущий экран `emoji`) ставить `aria-expanded="false"`.
Тест: клик emoji → `"true"`; клик close и Esc → `"false"`; для settings-кнопки аналогично
проверить отсутствие/наличие expanded (сейчас его нет — согласовать контракт).
Выигрыш: 2 строки; состояние кнопки = состоянию экрана.

**G2-07 | `features/search-overlay.js:966-980,852-910` | нет debounce/abort: каждый символ = `search_remote`**
`input` → `show()` → `refresh()` → `fetchRemote(false)`; защита только `requestSeq` (ответы
старых запросов отбрасываются), но запросы уже улетели. На реальном Discord это 429 на
быстром наборе; в шиме не видно, т.к. тесты вызывают `setQuery` один раз.
P2. Фикс: debounce 250 мс на `input`/`header input` (не на кнопке «Показать ещё») +
ранний выход, если `query.trim()` не изменился; `AbortController` не нужен — seq уже есть.
Тест: 5 `input` подряд → после `flushTimers(250)` ровно 1 `search_remote`; «Показать ещё» —
сразу; `query` без изменений не шлёт второй запрос.
Выигрыш: −80% запросов при наборе; без изменения выдачи.

**G2-08 | `features/auth.js:18-32` | `errText` классифицирует любой `Error`/строку как «Нет связи с сервером»; `parse/internal` не используются**
`if (typeof err === "string" || err instanceof Error) return offline`. Tauri-ошибки часто
приходят строкой — любая ошибка парсинга/валидации показывается как «проверьте сеть».
`AUTH_MESSAGES.parse/internal` объявлены и мертвы. P2 (неверная причина → неверное действие
пользователя). Фикс: строка/`Error` → `AUTH_MESSAGES.internal` (или `parse` по префиксу),
`offline` — только `kind==="Network"`/`code===0`/явный network; `internal` использовать в
default. Тест: `fail({kind:"Internal"})` → «Внутренняя ошибка», не «сеть»; строка `"boom"` →
internal; `{kind:"Network"}` → offline; `parse` — на `SyntaxError` из адаптера.
Существующий logout-кейс (`auth.test.mjs:1229`) допускает и `no-session`, и `error`, поэтому
уточнение маппинга не требует правки его ожиданий.
Выигрыш: −2 мёртвых константы, корректный recovery-текст.

**G2-09 | `features/auth.js:91,123,182` | один слот `timer`: интервалы перезаписываются и не чистятся**
`timer` — `let timer`, ставят и rate-limit (`fail`), и backup-отсчёт (`selectMethod`). Второй
`setInterval` затирает id первого; старый продолжает тикать; `ready()`/`logout` не чистят.
P2 (утечка, гонка текстов «повторите через N» и «сохраните коды»). Фикс: `let timers=[]`,
`clearTimers()` в `setState("ready"|"no-session")`, `push` вместо присваивания.
Тест: 429 → backup-отсчёт → `env.timers.size` не растёт после ready/no-session; после
`flushTimers` заголовок submit не меняется позже, чем нужно; повторный 429 не оставляет два
тика.
Выигрыш: −1 класс утечек; тест на `timers` уже поддержан harness.

**G2-10 | `features/actions.js:89-103,108-110,129-132` | «мёртвые affordance»: `aria-expanded` без панели**
`guild-menu`, `threads`, `pins`, `profile` только переключают `aria-expanded`, без
`aria-controls`, без поповера. AT получает «раскрыто», пользователь — ничего. При этом
`inbox/attach/gift` сделаны честно (`aria-disabled`+`title`). P2 (несогласованный контракт
«нет мёртвых кнопок»). Фикс: до Phase 3 — `aria-disabled=true`+`title` как у inbox; либо
`aria-controls` на реально существующий узел. Тест: для каждого `[aria-expanded]` есть
`aria-controls`, указывающий на существующий id, **или** `aria-disabled="true"` + `title`.
Выигрыш: единый контракт affordance; −4 «лживых» состояния.

**G2-11 | `features/message-toolbar.js:237-246` | подтверждение удаления липнет к панели и к следующей строке**
После первого клика текст кнопки навсегда «Подтвердить удаление»; `deletePending` сбрасывается
только при confirm; `open()/close()` не сбрасывают ни текст, ни pending. Следующая строка
видит уже «подтверждённое» состояние — одно нажатие выполняет destructive-команду.
P2 (риск случайного удаления). Фикс: `resetDelete()` в `close()` и при `open(index!==current)`;
хранить pending как `Set`, но текст кнопки вычислять из `deletePending.has(current)`.
Тест: A→confirm→hover B → label «Удалить сообщение», один клик не шлёт `delete_message`;
повторный confirm A — шлёт ровно один; после успеха/ошибки состояние сброшено.
Выигрыш: −1 скрытое состояние, защита destructive-клика.

**G2-12 | `features/auth.js:68` | `document.createElement("svg")` создаёт HTML-элемент, не SVG**
В браузере `createElement("svg")` — `HTMLUnknownElement` в HTML-namespace: QR-заглушка не
рендерится (атрибуты есть, `path d` невидим). Шим это не ловит: `createElementNS` в harness
(`gui-dom-harness.mjs:542-548`) игнорирует namespace, тест `auth.test.mjs:405-415` проверяет
только `width/height/aria-hidden`. P2 (визуальный дефект, невидимый тестам). Фикс:
`document.createElementNS("http://www.w3.org/2000/svg","svg")` + `path`.
Тест: `svg.namespaceURI === "http://www.w3.org/2000/svg"` (в шиме — сохранить namespace в
`createElementNS`, это аддитивное усиление harness); плюс `svg.tagName==="svg"`, children=1.
Выигрыш: QR снова виден; тест ловит namespace-класс ошибок.

**G2-13 | `features/embeds.js:60-66` | ссылка embed навигирует webview из приложения**
`<a href>` без `target`, без `preventDefault`; клик в Tauri-webview уводит окно с `index.html`
(нет навигационного guard). Whitelist протокола есть — это хорошо, но escape из app остаётся.
P2 (robustness; потенциально «белый экран»). Фикс: `link.addEventListener("click", e =>
e.preventDefault())` + честный `title`/`aria-disabled` (как image-заглушка) или `target="_blank"`
с внешним opener, когда он появится. Тест: клик по `.embed-title` не меняет
`document.location`/не удаляет `.shell`; `href` остаётся только `http(s)` (существующий тест
дополнить `javascript:`/`data:`).
Выигрыш: приложение нельзя «вынести» одним кликом.

**G2-14 | `features/members.js:305-306,400-410` | `selectGroup` молча ничего не делает для незагруженной группы; click-листенер навешивается при каждом `renderMemberPanel`**
`firstInGroup` вернул `null` → `return` после установки `aria-pressed` (кнопка «нажата», эффекта
нет). `renderMemberPanel` вызывается из `app.js:186-193`; повторный вызов (тест/refresh)
добавит второй click-листенер на тот же контейнер. P2. Фикс: вынести `container.addEventListener`
до/вне рендера (или `{once:false}` с флагом), при `null` — видимый статус/`aria-live` «в
загруженном окне группы нет». Тест: вызвать `renderMemberPanel` дважды → один обработчик
(клик меняет ровно один `aria-current`); выбрать группу, которой нет в окне → `aria-pressed`
не должен оставаться единственным сигналом (есть статус), `refreshGroupChips` не падает.
Выигрыш: идемпотентность панели; −дубли листенеров.

**G2-15 | `features/settings.js:674-690` | навигация настроек молча игнорирует неизвестный раздел и сравнивает по `textContent`**
`PANE_BY_SECTION[key] ?? "none"` → `return` без статуса/ошибки; активный пункт ищется
`button.textContent === key` (дубль логики `screens.js:51-58`, но по тексту, а не по ключу).
P2/P3. Фикс: `data-section` на кнопке, сравнение по нему; неизвестный ключ →
`showStatus`/`onError` (ошибка не глотается). Тест: клик по disabled-разделу не меняет
`data-active-pane`; подмена текста кнопки не ломает активный пункт; `showPane("nope")` даёт
видимый статус, не тишину.
Выигрыш: −строковый оракул; навигация по ключу.

### 2.3 P3 (стиль/мёртвый код/склейки)

**G2-16 | `features/search-overlay.js:839-846` + `app.js:52-56` | двойной `console.error` на одну ошибку поиска**
`renderRemoteError` делает `reportError(...)` и затем `onError(reported)` → `surface()` →
`reportError` снова. Пробник: две одинаковые строки в логе. P3 (шум в gate-логах
`consoleErrors`, поведение не ломает). Фикс: оставить `onError` (surface сам логирует) или
`reportError` без `onError`. Тест: `search_remote` 401 → `env.consoleErrors.length === 1`;
`data-state=error` и `.error-state` присутствуют.
Выигрыш: −1 строка/ошибка, чистые логи.

**G2-17 | `app.js:207-304` | diagnostics смешаны с boot: `autoBench`/`reportUiSnapshot` в боевом модуле, `console.log`**
`app.js:228` (`console.log("[autobench]")`) и `metrics.js:125` (`console.log("[bench]")`) —
логи в production-модулях (BASELINE Q-06). `reportUiSnapshot` кликает по rail/табам/тумблерам
и ходит по экранам при каждом старте — это тест-сценарий в runtime. P3. Фикс: вынести в
`ui/diagnostics.js`, включать по `spike_config` флагу (сейчас autobench уже флаг, snapshot —
нет); логи — через `console.debug`/удалить. Тест: meta-grep «нет `console.log` в `ui/**` кроме
`errors.js`»; snapshot не отправляется, когда `spike_config.diagnostics=false`.
Выигрыш: −100 LOC из `app.js`; runtime не делает тестовый сценарий без запроса.

**G2-18 | `app.js:99-101,104-107` | доступ к `__TAURI__` размазан и небезопасен**
`boot` берёт `tauri` один раз и строит `invoke`, но `configureReactionTransport` вручную
повторяет `window.__TAURI__.core.invoke` (лишний путь). Сам `invoke` вызывает
`tauri.core.invoke` без проверки: без `__TAURI__` (browser/preview) boot не падает, но первое
же действие (reply/send/настройки) бросает синхронный `TypeError` внутри click/keydown-листенера
— uncaught-исключение вместо видимой ошибки; `settings.js:739-744` при этом guard делает.
P3. Фикс: один `createBridge()` (в `dom.js`/`bridge.js`), который возвращает `null`/бросает
`UIError("IPC_FAILED")`; `configureReactionTransport` получает тот же `invoke`.
Тест: импорт `app.js` без `window.__TAURI__`, затем Enter в композере → видимый `IPC_FAILED`
(либо тихий no-op счётчик), без uncaught TypeError в `consoleErrors`;
`configureReactionTransport` получает функцию из bridge (шпион).
Выигрыш: −1 дубль; предсказуемая ошибка среды.

**G2-19 | `features/chat.js:188,286,394,560` | геометрия задублирована в JS-магией (`26`, `-2`, `34`)**
`text.style.height = max(20, rowHeight-2)` c fallback `26`; unread высота `34` в `data-height`
и в геттере; `data-row-height="26/48/+22"` строкой. Модель высот уже есть
(`model.js:223-241`, `ROW_HEIGHTS`). Риск регрессии DP-02: правка токена/модели не изменит
эти числа, а `Math.max(20,…)` тихо клампит при height=0 (tombstone).
P3 (регрессионная точка DP-02, саму DP-02 не дублирую). Фикс: брать `ROW_HEIGHTS` и
`UNREAD_COUNT`/токен из одного места; `-2` → именованная константа `ROW_TEXT_INSET`; для
tombstone не задавать inline-height. Тест: числа в `data-row-height`/`data-height` совпадают с
`ROW_HEIGHTS`/токеном (`getComputedStyle` в браузере, в шиме — parse `tokens.css` как оракул);
`row.style.height===0` → у `.text` нет фиктивной высоты.
Выигрыш: −3 магических числа; DP-02 не «разъедется» между CSS и JS.

**G2-20 | `app.js:287` | `UI_SNAPSHOT.searchMatches` всегда из локальной выдачи**
В remote-режиме `matchesCount` возвращает `lastMatches` (локальный скан, часто 0), хотя
пользователь видит серверные результаты. P3 (диагностика/evidence врёт). Фикс:
`search.matchesCount` учитывать `mode` (`remoteCount`/`remoteTotal`) или добавить
`search.remoteCount` в snapshot рядом. Тест: remote-ответ 12 сообщений → snapshot
`searchMatches===12` (или явные `searchMode/searchRemoteCount`).
Выигрыш: snapshot пригоден как evidence.

**G2-21 | `features/dom.js:24`, `features/settings.js:237`, `features/embeds.js:19`, `features/search-overlay.js:86` | 4 копии «создать узел + класс + textContent»**
Семантика идентична (span/div + className + textContent). Фикс: оставить `dom.make(tag,cls,text)`
(он уже экспортирован) и удалить 3 локальные обёртки; `node()` в search/embeds = `make(tag,…)`.
Тест: существующие DOM-контракты (класс/текст) без изменений; characterization на 1 строку
каждого типа. Выигрыш: **≈ −25 LOC**, один путь создания узлов.

**G2-22 | `app.js:122,181`; `actions.js:40-44`; `composer.js:250-256`; `quick-switcher.js:113,175-177`; `emoji.js:384,466`; `search-overlay.js:379…` | 7 вариантов focus-guard (`typeof focus==="function"`, `?.focus?.()`, без guard)**
`quick-switcher.js:175` вызывает `closeButton.focus()` без guard — в шиме `focus()` есть
(`gui-dom-harness.mjs:234`), но это единственное место без общей защиты; остальные 6 копий
решают одну задачу по-разному. Фикс: один `focusNode(node)` в `dom.js`
(он уже есть в `actions.js:40`), использовать везде. Тест: Tab в switcher/auth на harness без
патча `focus` даёт тот же фокус-лог; behavior не меняется. Выигрыш: **≈ −15 LOC**, одна
семантика фокуса.

**G2-23 | `screens.js:51-58`, `settings.js:684-689`, `quick-switcher.js:133-146`, `members.js:301-313,407-408` | 4 копии «выбрать один из N»**
Везде `classList.toggle("active")/aria-current` с разными значениями (`page` vs `true`) и
разными источниками идентичности (dataset/текст/`data-index`). Не сливать в «универсальный»
helper целиком (разные домены), но `setCurrent(nodes, isCurrent, {value:"page"|"true"})` в
`dom.js` убирает 4 копии. Тест: существующие aria-current-ассерты + characterization на
смену канала/группы. Выигрыш: ≈ −20 LOC, единая семантика `aria-current`.

**G2-24 | `app.js:32-42` vs `metrics.js:80-105` | два «rAF + guard timeout» каркаса**
Оба решают «rAF не тикает — не висеть». Слить можно, но сигнатуры разные (once vs loop).
**Не рекомендую** в этом batch: выигрыш ≈ 8 LOC, риск задеть bench-поведение высокий.
Оставить как есть, зафиксировать комментарием-ссылкой.

**G2-25 | `features/chat.js:80,451`; `features/reactions.js:18-24`; `features/auth.js:237`; `app.js:72` | скрытые мутабельные глобалы модулей**
`refreshRowHook` — singleton: второй `createChatScreen` перезаписывает хук первого (в тестах
спасает один экземпляр на процесс); `reactions.js` держит `overrides/pending/invoke` на модуль;
`__SPIKE_AUTH_DISABLED__` меняет поведение гейта (документированный live-хук, но не описан в
README UI). Фикс: `refreshRow` передавать в `createChatScreen`-возврат и использовать в
`renderChatRow` через параметр/`WeakMap`; реакции — фабрика `createReactionStore()` с
`configure`; хук описать в `ui/README`/`docs`. Тест: два chat-экрана в одном процессе —
реакция на строке A перерисовывает A, не B; `resetReactions` не нужен между экземплярами.
Выигрыш: −1 глобальный singleton; тесты перестают зависеть от порядка файлов.

**G2-26 | `features/settings.js:403-439,589-597` | слайдеры/селекты интерактивны, но не сохраняются и ни на что не влияют**
`collectSettings` пишет только toggles/density/theme/locale; `sliderField`/`selectField` не имеют
ни `aria-disabled`, ни `title`, меняются визуально и теряются при перезагрузке. P3 (мёртвый
контрол без честной пометки). Фикс: `aria-disabled=true`+title «Phase 3» **или** сохранять в
whitelist. Тест: у каждого `.settings-slider/.settings-select` есть `aria-disabled`+`title` либо
он есть в payload `settings_save`; клик по disabled не меняет значение.
Выигрыш: согласованность с политикой «нет мёртвых кнопок».

**G2-27 | `features/actions.js:161-171,143-158` | `data-action` как конвенция только для статического DOM**
`chat.js:430` создаёт `[data-action="retry"]`, toolbar — `edit-input/edit-save/edit-cancel`,
auth — `data-action="auth-logout"`; ни один не в карте `handlers` (свои листенеры). Если
порядок инициализации изменится (auth раньше `initActions`), клик уйдёт в
`ACTION_FAILED «неизвестный data-action»`. P3. Фикс: либо убрать атрибут у динамических узлов,
либо делегирование на `body` с явным «ignore»-списком. Тест: characterization — клик по
`retry`/`auth-logout` не порождает `ACTION_FAILED` при обоих порядках инициализации.
Выигрыш: −скрытая зависимость от порядка.

**G2-28 | `style.css:166-168` | остался `:has()`-мост для settings**
Комментарий `screens.js:6-8` говорит, что мост стал избыточным; правило живо. P3 (мёртвый CSS,
риск каскада). Не трогать в G2 (файл — зона G3/W39d), зафиксировать для DP-чистки.

**G2-29 | `features/search.js:69`, `features/members.js:316-331` | поиск по списку снимает `is-selected`, но не `aria-current`**
После поиска визуальный выбор снят, `aria-current` на строке остаётся → AT видит выбранную
строку, которой нет. P3. Фикс: снимать оба. Тест: поиск «member_999» после выбора группы →
`aria-current` нет ни у одной строки.

**G2-30 | `features/search-overlay.js:563`, `features/members.js:327` | поиск по подстроке индекса (`String(index).includes(needle)`)**
Запрос «2» совпадает с индексами 2/12/20/…; для демо это удобно, но не документировано и
делает текстовый поиск нечётким. P3. Фикс: убрать индекс из predicate (или явно
документировать «поиск по #номеру» и добавить тест). Тест: запрос `"2026"` не матчит строку
2026 по индексу, только по тексту.
Выигрыш: предсказуемая выдача поиска.

## 3. Split-план крупных модулей (без изменения публичного контракта)

Принцип: резать **по состояниям/DOM-контракту**, а не «по строкам»; старые импорты сохранить
re-export'ом, чтобы тесты (`SEARCH_SECTIONS`, `SETTINGS_*`, `normalizeSettings`) не менялись.

### 3.1 `search-overlay.js` (1049) → 4 модуля

| Новый модуль | Что уходит | Оценка LOC | Контракт |
|---|---|---:|---|
| `search-model.js` | `SEARCH_SECTIONS/SORTS/SLICE`, `REMOTE_SORTS`, `LOCAL_FILTER_KEYS`, `isIsoDate`, `selectedGuildId`, `isSearchError`, `remoteErrorText`, `normalizeRemoteMessage`, `flattenServerMessages` (23-199) | ~180 | re-export из `search-overlay.js` |
| `search-local.js` | `matches`, `hasContent`, `scan`, `sorted`, `renderLocal`, `localStatus` (538-784) | ~170 | зависит от `chat.store` + фильтров |
| `search-remote.js` | remote-state (`remoteState…idleWaiters`), `buildRemoteParams`, `applyRemoteFilters`, `sortRemote`, `fetchRemote`, `renderRemote(Error)`, `renderLoading` (447-537, 786-910) | ~300 | `createRemoteSearch({invoke, guildId, getFilters, getQuery, getSort, onStatus, onResults})` |
| `search-overlay.js` | DOM-шапка/7 секций/сортировка/меню/статус, `show/close/refresh`, публичный объект (211-436, 916-1049) | ~300 | прежний `createSearchOverlay` |

Публичный контракт (panel/input/status/refresh/setQuery/applyFilter/show/close/open/matches/
matchesCount/sort/mode/remoteTotal/remoteCount/whenIdle) остаётся в `search-overlay.js`;
`fresh`-split не меняет `ui/app.js` ни на строку. Риск: `whenIdle`/`inflight` — держать в
одном месте (remote), тесты B7b уже гоняют `whenIdle` — менять только импорт.

### 3.2 `settings.js` (746) → 3 модуля + фасад

| Новый модуль | Что уходит | Оценка LOC |
|---|---|---:|
| `settings-spec.js` | `SETTINGS_SECTIONS/ENABLED/BLOCKS`, `TOGGLE_SPECS`, `SETTINGS_TOGGLE_IDS`, версии/whitelist, `normalizeSettings`, `resolveToggleId`, `SETTINGS_GROUPS` (16-193) | ~185 (данные, почти не меняются) |
| `settings-store.js` | `applySettings`, `collectSettings`, `persist`, `scheduleSave`, `loadSettings`, `describeError`, `showStatus/clearStatus` (262-362) | ~110 |
| `settings-panes.js` | `element`, `toggleRow`, `selectField`, `sliderField`, `actionButton`, `inputProfile`, `relatedSettings`, `blockExtras`, `createPane`, сборка voice/appearance/notifications, density/theme/syncControls (236-669) | ~380 |
| `settings.js` | `createSettingsConsole/initSettingsConsole`, nav, `showPane`, публичный объект (671-746) | ~120 |

Разрез по «данные / сохранение / DOM» убирает главную проблему: сейчас hydrate и DOM-билдеры
переплетены, а `status` объявляется **после** использования в функциях (работает только за счёт
hoisting-времени вызова — хрупко).

### 3.3 `chat.js` (577) → 3 модуля

| Новый модуль | Что уходит | Оценка LOC |
|---|---|---:|
| `chat-data.js` | `makeMessage`, `CHAT_*`, `UNREAD_*`, `createChatStore`, `createDeletionHeights` (20-364) | ~210 |
| `chat-send.js` | `appendLocalMessage`, `sendMessage`, retry, ошибки 401/429, `returnMessageToComposer` (402-442,547-555) | ~100 |
| `chat.js` | `renderChatRow`, unread divider, `createChatScreen/initChatScreen`, toolbar wiring, `jumpTo` | ~270 |

`renderChatRow` и `createUnreadDivider` реэкспортировать (их импортируют тесты/`members`? —
grep: только внутри UI, но `makeMessage` импортирует toolbar-контекст), чтобы не плодить переносы.

### 3.4 `auth.js` (241, 26.6 КБ) → 4 модуля + таблица состояний

| Новый модуль | Что уходит | Оценка |
|---|---|---:|
| `auth-view.js` | построение gate/card/form/qr/captcha/mfa/logout, `hide`, `setBusy` (37-118) | ~200 LOC после нормального форматирования |
| `auth-errors.js` | `AUTH_MESSAGES`, `errText`, `describe` (16-32) | ~40 |
| `auth-machine.js` | `setState` как **таблица** `STATE_RULES` (что скрыто/disabled/aria) + `fail/result` (119-180) | ~120 |
| `auth.js` | провода flow: password/token/restore/mfa/captcha/logout + `createAuthGate` | ~150 |

Главный выигрыш — не LOC, а **снимаемая плотность**: 1622-символьная строка `setState`
превращается в таблицу `state → {visible, disabled, ariaBusy, text}`, которую можно
прочитать/проверить построчно. Characterization-тест: снапшот `data-state`+hidden-матрицы по
7 состояниям **до** рефакторинга, сравнение **после** (без изменения ожиданий).

## 4. Дубли обработчиков/селекторов/state-переходов (карта)

| Паттерн | Копии | Минимальный фикс | Выигрыш |
|---|---|---|---|
| `element/make/node` | `dom.js:24`, `settings:237`, `embeds:19`, `search-overlay:86` | оставить `dom.make`, остальное удалить | −25 LOC |
| focus-guard | `app:122,181`, `actions:40`, `composer:250`, `quick-switcher:113,175`, `emoji:384,466`, `search-overlay:379…` | `dom.focusNode` | −15 LOC, −1 TypeError |
| `aria-current` выбор | `screens:51`, `settings:684`, `quick-switcher:133`, `members:301,407` | `dom.setCurrent` (значение параметром) | −20 LOC |
| `aria-expanded` toggle | `actions:102,109,131` (+`82,96` c разными дефолтами) | `dom.toggleAria`; **сначала** унифицировать контракт «отсутствует = ?» | −10 LOC, 1 семантика |
| «регистрация глобальных клавиш на body» | `actions:250`, `quick-switcher:160` | оставить 2 домена, но зафиксировать порядок (quick-switcher first) тестом | риск −1 |
| `router.switchScreen("chat")` + фокус | `actions:223-227`, `app:177-183`, `screens:87` | один `closeToChat({focus})` | −10 LOC |

**Не предлагать** «универсальный state-manager»/store: у чата/поиска/настроек разные модели
(виртуализация, пагинация, debounce), общий helper сейчас даст только слой и риск. Побеждают
локальные резаки по контракту (см. §3).

## 5. Тесты: дубли, разрез, дыры (без ослабления assert)

### 5.1 Метрики

| Файл | Строк | `it` | `loadApp` | Наблюдение |
|---|---:|---:|---:|---|
| `gui-spike-features.test.mjs` | 2303 | 117 | 14 прямых + `withApp` | 8 `describe`-доменов в одном файле; локальные `byId/all/firstNode/clickRail/key/action` дублируют `dom.test` |
| `gui-spike-auth.test.mjs` | 1425 | 47 | 0 `loadApp` | свой `loadAuthApp` (100-160) + свой `importCounter` вместо `loadApp` |
| `gui-spike-dom.test.mjs` | 853 | 29 | — | те же `byId/one/all/withApp` (~40 LOC) |
| `gui-dom-harness.mjs` | 1397 | — | — | менять только аддитивно; `createElementNS` есть, но без namespace (G2-12) |

Скорость: 212 тестов за **1.33 с** — «медленная локализация» из Q-03 **не подтверждается**.
Проблема тестов не в скорости, а в **дублях setup и в дырах покрытия** (ниже).

### 5.2 Разрез (assert не трогаем)

- `features` → 5 файлов по `describe`: `gui-spike-ui-nav.test.mjs` (152-817),
  `gui-spike-chat.test.mjs` (817-1437), `gui-spike-members-reactions.test.mjs` (1200-1566),
  `gui-spike-search.test.mjs` (1566-2076), `gui-spike-settings.test.mjs` (2076-2303).
  Общие `byId/all/firstNode/chatRow/scrollChat/withApp` → `tests/support/gui-app-helpers.mjs`
  (новый, аддитивный). node:test запускает файлы параллельно — время не вырастет.
- `auth` → 3 файла: `auth-view` (272-452), `auth-flows` (452-928, MFA/captcha),
  `auth-errors-token` (928-1425); `loadAuthApp/domDump/assertSecretGone/type` → тот же
  `gui-app-helpers` (или `gui-auth-helpers`). `ShimElement.prototype.focus` patch — в helper,
  вызывается один раз на файл; поведение то же.
- `loadAuthApp` заменить на расширенный `loadApp({invokeHandler, windowExtras})`: харнесс уже
  умеет `invokeHandler`; добавить `windowExtras` (аддитивно) → −40 LOC и один путь загрузки,
  включая `activeEnv`/`dispose` (сейчас auth-тест свой `importCounter`, но `activeEnv` не
  трогает — при падении до `dispose` возможен «висящий» env).
- Условие: каждый перенос — отдельный commit-эквивалент с **тем же** набором assert; диф
  тестов проверяет не автор (`TEST-POLICY` §5). Никаких `skip`, порогов и «упрощений».

### 5.3 Дыры покрытия → red-first список

| # | Дыра | Тест (оракул) | Ловит |
|---|---|---|---|
| T1 | reply не переживает ввод | `data-reply` после `keydown("a")`; DOM-CONTRACT композера | G2-01 |
| T2 | жизненный цикл `.error-state` | после ошибки → успех/«Закрыть» → 0 узлов; `primitives.css` inset | G2-02 |
| T3 | видимость empty-state switcher | `hidden`-атрибут при 8 и 0 options | G2-03 |
| T4 | focus-trap auth | 30×Tab остаётся в `#auth-gate`; shell `aria-hidden` | G2-04 |
| T5 | `applyFilter` контракт | `from`/`pinned` меняют выдачу и контролы | G2-05 |
| T6 | сброс `aria-expanded` | open→close→`"false"` | G2-06 |
| T7 | debounce поиска | 5 input → 1 `search_remote` после 250 мс | G2-07 |
| T8 | таймеры auth | `env.timers` после ready/no-session; повторный 429 | G2-09 |
| T9 | destructive-confirm | hover другой строки сбрасывает label/pending | G2-11 |
| T10 | SVG namespace | `svg.namespaceURI` (после аддитивного `createElementNS` в шиме) | G2-12 |
| T11 | выход из app по embed-link | клик не навигирует/есть guard | G2-13 |
| T12 | идемпотентность member-panel | двойной `renderMemberPanel` → один листенер | G2-14 |

Все 12 — **characterization/red-first** на текущем контракте, без правки ожиданий под будущий
код. T10 требует аддитивного усиления harness (сохранять namespace) — разрешено стратегией
(«harness менять аддитивно»), но диф ревьюит G4.

## 6. XSS / безопасность DOM — сводка

- `innerHTML/outerHTML/insertAdjacentHTML/document.write/eval/new Function` — **0** в
  `rust/gui-spike/ui/**` (rg).
- Единственный динамический `href` — `embeds.js:57-66`, whitelist `http(s):`, иначе
  не-ссылка с `aria-disabled`; `rel="noreferrer noopener"` есть. Остаточный риск — навигация
  (G2-13), не инъекция.
- Все тексты — `textContent`/`createElement`; серверные сообщения (`search-overlay.js:681-734`)
  тоже через `textContent`. `data-*`/`title` — из строк, не HTML.
- Секреты: `password.value`/`token.value`/`code.value` очищаются сразу; `passValue` живёт до
  captcha-ветки (необходимо); `assertSecretGone` покрывает DOM+логи+IPC. Новых утечек нет.
- `--avatar-hue` — только числа из `%`, CSS-инъекции нет.
- Рекомендация: meta-grep в `tests/meta` на `innerHTML|outerHTML|javascript:|data:text/html` в
  `ui/**` + на `createElement("svg")` (G2-12) — дешёвый сторож.

## 7. Доступность/состояния — сводка

| Состояние | Реализовано | Пробел |
|---|---|---|
| loading (chat/member) | placeholder + `WINDOW_EMPTY`, `RANGE_FAILED` | ошибка списка терминальна (G2-02) |
| loading (search) | `data-state=loading`, «Ищем на сервере…» | нет debounce (G2-07) |
| empty | search «Ничего не найдено», member chips 0, switcher (**инвертирован**, G2-03) | member group без строк — тишина (G2-14) |
| error | `role=alert`, `data-error-code`, status-строки настроек | нет dismiss/clear (G2-02), двойной лог (G2-16) |
| rollback | реакции/редактирование/удаление откатываются; настройки — «применено, но не сохранено» (осознанно) | — |
| focus trap | settings/emoji (Tab-цикл), switcher (2 узла) | auth-модалка (G2-04); toolbar без arrow-roving |
| `aria-expanded` | search sort/menu, tree, settings «ещё звуки» | emoji не сбрасывается (G2-06); 4 кнопки без панели (G2-10) |
| `aria-current` | screens/settings/channels/members | остаётся после поиска (G2-29) |

## 8. Геометрия: где код может регрессировать (DP-02/04/05 не дублирую)

DP-01 закрыт CSS (media 1199/959/719 в `style.css:217-231`, `tokens.css:376-380`) — код это
не ломает, пока inline `style.display` в `actions.js:84,98` ставит `""` (не `flex`), иначе
inline перебьёт media-query. Сейчас корректно; точка регрессии — если кто-то заменит `""` на
`"flex"`.

Риски, которые стоит закрыть тестом **до** DP-02/DP-04:
1. `chat.js:188` — `Math.max(20, rowHeight-2)` + fallback `26`: при смене модели/токена текст
   останется 24 px; при tombstone (`height=0`) — 20 px в нулевой строке.
2. `chat.js:286,560` — `34` в двух местах; DP-02/токен не подхватится.
3. `chat.js:394` — `data-row-height` строкой; DIAG/geometry-check может читать устаревшее.
4. `members.js:391` — `data-height="42"` дублирует `memberHeight()`.
5. `message-toolbar.js:62-64` — `top:4/right:16` магические px вне токенов (позиция панели).
6. `emoji.js:477` — шаг сетки `EMOJI_COLS=9` захардкожен и в JS, и в `--emoji-cols`; при
   смене CSS-колонок клавиатура «прыгает» (DP-05-класс, но фронтовый).

Минимальный фикс: один `geometry.js` (или расширить `model.js`) с константами
`ROW_HEIGHTS`, `UNREAD_H`, `TEXT_INSET`; тест-оракул — `tokens.css`/`model.js` (parse, не
«примерно»). Это не DP-02, а защита от расхождения JS↔CSS при закрытии DP-02.

## 9. Топ-10 (для приёмки) и порядок

| # | ID | Суть | P |
|---|---|---|---|
| 1 | G2-01 | reply отменяется любой клавишей | P1 |
| 2 | G2-02 | error-overlay терминален (fullscreen, без dismiss/clear) | P1 |
| 3 | G2-04 | auth-модалка без focus-trap, Tab в `aria-hidden` shell | P1 |
| 4 | G2-03 | quick-switcher empty-state инвертирован | P2 |
| 5 | G2-05 | `applyFilter` игнорирует поля, кроме даты | P2 |
| 6 | G2-06 | `aria-expanded` эмодзи не сбрасывается | P2 |
| 7 | G2-10 | 4 «мёртвых» aria-expanded без панели | P2 |
| 8 | G2-07 | поиск без debounce: символ = запрос | P2 |
| 9 | G2-11 | подтверждение удаления липнет к другой строке | P2 |
| 10 | G2-09 | auth-таймеры перезаписываются/не чистятся | P2 |

**3 быстрых выигрыша** (каждый ≤10 LOC + red-тест, без рефакторинга):
1. **G2-03** — поменять 2 условия; тест на `hidden` (ловит целый класс empty-багов).
2. **G2-01** — добавить `event.key === "Escape"`; тест на reply+ввод (P1 за 1 строку).
3. **G2-06** — сброс `aria-expanded="false"` в `emoji-close`/`closeTopLayer`; тест AT-состояния.

(Альтернатива вместо G2-06: G2-16 — убрать двойной `console.error`, 1 строка.)

**3 рискованных места** (не начинать без characterization):
1. **G2-02** — глобальная error-поверхность: её считают тесты
   (`features:329,1046,1236`), `virtual-list.fail`, `auth`/search. Менять зону/жизненный
   цикл только после снапшота «кто и когда показывает `.error-state`»; иначе легко ослабить
   существующие проверки видимости ошибок.
2. **G2-04** — focus-trap: затрагивает `actions.js` (общий для всех экранов), auth-тесты
   патчат `focus` глобально; нужен тест на порядок слоёв (search → composer → screen → auth),
   иначе можно «поймать» Tab на неверном слое.
3. **auth `setState` → таблица (§3.4)** — 7 состояний × 47 тестов; любая перестановка
   `hidden/disabled/aria-busy` ломает сразу несколько suites. Только после state-снапшота и с
   ревью не-автора.

## 10. Что НЕ делать (по итогам аудита)

- Не «универсализировать» state/store/search — у доменов разные модели; выигрыша по LOC/риску
  нет (см. §4).
- Не рефакторить `rows.js`/`model.js`/`virtual-list.js` в этом batch: они — оракул геометрии и
  заморожены (ADR-1/D-04).
- Не менять assert/skip/пороги при разрезе тестов; переносы — только механические, диф
  ревьюит G4.
- Не трогать `style.css`/`tokens.css` ради G2-28 (зона G3, DP-очередь).

## 11. Evidence / артефакты

| Артефакт | Путь | md5 |
|---|---|---|
| Этот отчёт | `waves/RW-40/QUALITY/G2-AUDIT.md` | финальный md5 — в ответе PM (файл после записи read-only) |
| node:test лог (212/0) | `waves/RW-40/PM/qa-g2/node-test.log` | `5a3f5161883e35561f6852a89de002d5` |
| tsc лог (exit 0, пустой вывод) | `waves/RW-40/PM/qa-g2/tsc.log` | `f08d1fb4622dd3c68ecdcd075e141912` |
| Пробники (вне репо) | `/tmp/opencode/g2-probe.mjs`, `/tmp/opencode/g2-probe2.mjs` | не сдаются |

Никаких изменений в `rust/gui-spike/ui/**`, `tests/**`, golden-путях и доках процесса не
внесено; live/сеть/окно не запускались.
