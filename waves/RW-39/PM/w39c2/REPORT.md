# Отчёт W39c2 (Markup/CSS) — канонический `index.html` + обвязка CSS

Дата: 2026-10-23. PM: DeepSeek. Итерация 2 (продолжение W39c). Окно/probe/cargo не запускались.

## 0. Статус: **RED — один блокер вне моей файловой зоны**

| Проверка | Exit | Результат |
|---|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs` | **1** | **20/21**, падает только тест №1 «шим-разметка совпадает с `ui/index.html`» |
| `node --test tests/unit/gui-spike-model.test.mjs` | 0 | 26/26 pass |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | 0 | вывод пуст |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | 0 | 31/31 в допуске |
| `node waves/RW-39/PM/w39c2/markup-check.mjs` (доп., мой оракул) | 0 | 38/38 проверок разметки |

Причина red — **ровно та, что описана в `DOM-CONTRACT.md` §9.1–§9.2 (план A)**: тест №1
собирает все `id="…"`/`class="…"` из `ui/index.html` и требует наличия каждого узла в
`tests/support/gui-dom-harness.mjs:buildDom`. Харнесс по-прежнему строит DOM старого шима
(нет 14 id, 61 класса из §3):

```
AssertionError [ERR_ASSERTION]: шим-DOM не повторяет #guild-rail из index.html
```

Правка `tests/**` моим ТЗ запрещена (файловая матрица: «только эти файлы», `tests/**` —
read-only), поэтому патч харнесса **подготовлен и проверен, но не применён**.

**Что нужно от PM (одна команда):**

```bash
git apply waves/RW-39/PM/w39c2/harness-planA.patch   # либо: patch -p1 < ...harness-planA.patch
node --test tests/unit/gui-spike-dom.test.mjs        # → 21/21
```

Патч аддитивный (только `buildDom`, ни один assert не меняется), повторяет §3 один-в-один,
сохраняет `boxHeight` (`#chat-list=800`, `#emoji-grid=600`, `#member-list=800`, `#settings=800`)
и оставляет списки пустыми. Доказательство: он применён к **копии** `tests/` в
`/tmp/opencode/applycheck` (репозиторий не тронут), прогон с реальным `ui/` — **21/21 pass,
exit 0** (`dom-with-planA.log`). Исходник функции — `builddom-planA.js`, диф —
`harness-planA.patch` (346 строк).

Альтернатива, если PM хочет иначе: явно разрешить W39c2 применить патч — применю и
перепрогоню (файл вне моей зоны, поэтому сам не трогал).

## 1. Что заменено в `ui/index.html`

Взято дерево `DOM-CONTRACT.md` §3 (строки 61–272) целиком; сверка машинная,
`waves/RW-39/PM/w39c2/markup-check.mjs` §9: построчная нормализация против скелета из
контракта → **0 расхождений**, кроме двух задокументированных ниже. 53 → 173 строки.

Состав: `.topbar` (role=banner, `.screen-title` aria-level=2 + aria-live, метрики, `#bench`) →
`.shell` → `#guild-rail.rail` (бренд `DL`, 3 `.guild-item[data-guild]`, `.rail-sep`,
4 `.rail-btn[data-screen]`) + `.workspace` → `#channel-tree.tree` (шапка, 2 группы каналов,
`#user-bar`) + `main.content` (`#chat-header` **sibling** экранов, `#screen-chat` c `.chat-main`,
`#chat-list`, `.chat-notice`, `#composer`, `#member-panel`; `#screen-emoji` c `.emoji-card`
498×440; `#screen-members` c тулбаром и `#member-list`; `#screen-settings` c `.settings-shell`).

Инварианты RW-38 (§2) сохранены и проверены статически: 4 `.screen` в порядке
chat/emoji/members/settings, 4 `.rail-btn` в том же порядке, `data-screen` — **только** на
`.rail-btn` (4 вхождения), `data-active-screen="chat"` на body, тексты `.screen-title`
дословно, `#chat-list`/`#member-list` пустые с `data-row-height`, `#emoji-grid` — `listbox`
с ячейками `option` (генерация), `#settings` без навигации (nav отдельно, без `h3`).

### Две осознанные правки против §3

1. **`<link rel="stylesheet" href="tokens.css" />`** вместо `components/tokens.css` — по
   решению PM D-05 (токены вендорятся в `ui/tokens.css`). Список остальных 8 `<link>` —
   как в §3; `header.css`/`composer.css` остаются через `@import` в `chat.css` (D-05).
2. **`#emoji-grid role="listbox"`** (в §3 — `role="grid"`) и без `aria-colcount` — оракул
   RW-38 (тест P1-2, заморожен) требует `role="listbox"` и ячейки `role="option"`;
   PM-brief W39c2 подтверждает: «`#emoji-grid` = role=listbox, ячейки role=option».
   `aria-colcount`/`aria-rowcount` — атрибуты `grid` (D-05), к `listbox` не применимы;
   вернуть вместе с `role="grid"`, когда тест будет пересмотрен. Это конфликт §3/D-05 с
   frozen-тестом — **прошу зафиксировать в контракте** (§3 и §10.1), иначе следующий воркер
   снова упрётся.

Ассеты: `url()` — 0, `@font-face` — 0, спрайтов/логотипов нет; бренд — текст `DL`; эмодзи
системные. Слово «discord» встречается 2 раза и только как имя проекта (`<title>` и
`title` у бренда) — дословно из §3.

## 2. Какие CSS-селекторы поправлены и почему

Числа — только `var(--…)` из `ui/tokens.css` (511 ссылок, 0 неразрешённых); магических px нет.

| Файл | Правка | Почему |
|---|---|---|
| `style.css` §4 | добавлены `.content:has(#screen-chat.active) #chat-header` и `.content:has(#screen-chat:not(.active)) #chat-header{display:none}` | §3.1 переключает `#chat-header` по `body[data-active-screen]`, но текущий `app.js` (RW-38) этот атрибут **не обновляет** (`grep activeScreen` — 0). `:has()` читает фактический `.screen.active`; специфичность (2,2,0) выше `body[attr]`-правила (1,1,1), поэтому стейл-атрибут не мешает. После W39d (§10.2) оба правила совпадают — блок можно удалить, поведение не изменится |
| `style.css` §4 | `.workspace:has(#screen-settings.active) #channel-tree{display:none}` | то же для «оверлея настроек» (дерево прячется), пока `data-active-screen` стейл |
| `emoji.css` | `.emoji-grid`: `display:flex; flex-direction:column` → `display:grid; grid-template-columns: repeat(var(--emoji-cols), var(--emoji-cell)); gap: var(--emoji-gap); align-content:start` | `app.js` (RW-38) кладёт `.emoji-cell` **прямо** в `#emoji-grid`, без `.emoji-row`. Старый flex-column давал 1 ячейку в ряд вместо 9 (498−48−16=434 ≥ 9×44+8×4=428). Сетка на токенах, без `display:contents` |
| `emoji.css` | `.emoji-row{grid-column:1/-1; display:grid; grid-template-columns:repeat(…)…}` | совместимость вперёд: когда W39d начнёт генерировать строки `role=row` (§7.3), они займут всю ширину и останутся грид-контейнерами. Сейчас правило не совпадает ни с одним узлом |
| `emoji.css` | `.tab`: `overflow:hidden; text-overflow:ellipsis; white-space:nowrap` | `app.js` пишет в вкладку название категории (текст), а не глиф (§7.3); без клиппинга текст вылезал из рейла 48px. После W39d (глиф + `title`) правило безвредно |
| `settings.css` | блок-мост `.toggle > input[type=checkbox]:not(.toggle-input)` (40×24 = `--switch-w/h`, `appearance:none`, кружок `--switch-thumb` через `radial-gradient`) | `app.js` рисует `<label class=toggle><span>…</span><input>` без `.toggle-track` (§7.4), т.е. выключенный «голый» чекбокс. Селектор `:not(.toggle-input)` сам перестаёт совпадать, когда W39d выдаст каноническую разметку — удалять ничего не нужно. `url()` не используется (градиент — не ассет) |

Остальные селекторы W39c уже соответствовали §3 (`#guild-rail`, `.tree-*`, `.user-*`,
`.chat-*`, `.emoji-card/head/body/preview`, `.members-*`, `.settings-*`, `.toggle-*`,
`.visually-hidden`) — не менялись. Проверено: все 76 классов из §3 имеют CSS-правило
(скрипт в §5 отчёта W39c, повторён).

## 3. Проверки (вывод)

```text
$ node --test tests/unit/gui-spike-dom.test.mjs
ℹ tests 21  ℹ pass 20  ℹ fail 1     exit=1
✖ шим-разметка совпадает с ui/index.html по id, классам и rail-кнопкам
  AssertionError: шим-DOM не повторяет #guild-rail из index.html

$ node --test tests/unit/gui-spike-model.test.mjs
ℹ tests 26  ℹ pass 26  ℹ fail 0     exit=0

$ npx tsc -p rust/gui-spike/ui/tsconfig.json
(пусто)                             exit=0

$ node waves/RW-39/PM/w39c/geometry-check.mjs
все 31 геометрических значений в допуске   exit=0

$ node waves/RW-39/PM/w39c2/markup-check.mjs
разметка: все проверки пройдены     exit=0

# контроль блокера: копия tests/ + harness-planA.patch (репозиторий не тронут)
$ node --test tests/unit/gui-spike-dom.test.mjs
ℹ tests 21  ℹ pass 21  ℹ fail 0     exit=0
```

Логи: `dom.log`, `model.log`, `tsc.log`, `geometry.log`, `markup.log`, `dom-with-planA.log`,
`md5.log`, `harness-planA.patch`, `builddom-planA.js`, `markup-check.mjs`.

## 4. md5

| Файл | md5 | Статус |
|---|---|---|
| `rust/gui-spike/ui/index.html` | `3dcf9bc418f4b9efe3cd507a84aeb91c` | заменён на §3 |
| `rust/gui-spike/ui/style.css` | `f660b65d28c27b7b64d3c881ed2a862b` | +2 правила видимости |
| `rust/gui-spike/ui/components/emoji.css` | `ae696723dbcab8b2b70cd7231e000880` | сетка 9 колонок + `.emoji-row` + clip вкладки |
| `rust/gui-spike/ui/components/settings.css` | `57a2cf2b342ff7d03fe802af057127b7` | +мост для голого чекбокса |
| `components/primitives.css` | `943b3aa129f4cd952718b4d4191786d3` | не менялся (было в W39c) |
| `components/rail.css` | `92b4f40b0bc962e15e4275bd42eed749` | не менялся |
| `components/tree.css` | `3f084ac11bca8e612dadf8a1d08e619d` | не менялся |
| `components/chat.css` | `11a01cc14e4bda6823b9c742da9f5966` | не менялся |
| `components/header.css` | `987d7b6b5643b558c4494a10639b8f67` | не менялся |
| `components/composer.css` | `4aaefb2c9fe632367ee105d8cf878106` | не менялся |
| `components/members.css` | `30074605780e75cc66404b7e38b90523` | не менялся |
| `rust/gui-spike/ui/tokens.css` | `3c88f2d7cbdd35dac13a66206aecc7dc` | **read-only, не трогал** (md5 отличается от отчёта W39c `70915fff…` — похоже, PM уже перевендорил после E1b; geometry 31/31 проходит) |

## 5. Остаток (честно)

1. **Блокер green: харнесс `tests/support/gui-dom-harness.mjs`** — нужен план A (§9.2 п.6:
   «правка харнесса + новый `index.html` — одним изменением»). Патч готов и проверен
   (21/21). Без него тест №1 красный у любого корректного §3.
2. **`app.js` (RW-38, зона W39d) не обновляет `body[data-active-screen]`.** Мой CSS-мост на
   `:has()` нивелирует это в окне; канонический путь — §10.2. После W39d мост можно снять
   (правила совпадут), но `style.css` — моя зона, поэтому пометка здесь.
3. **Разметка, которую генерирует текущий `app.js`, не совпадает с §7.3–§7.4** (вне моей зоны):
   вкладки эмодзи — текст вместо глифа (в рейле 48px обрезается), ячейки — `div.role=option`
   без `tabindex`/roving, в настройках голый чекбокс без `.toggle-track` (закрыт CSS-мостом),
   `#member-panel-list` и `#settings-nav` пустые (панель участников и навигация настроек в окне
   будут пустыми до W39d).
4. **Конфликт контракта:** §3/§4.4/D-05 (`role="grid"` + `aria-colcount`) vs frozen тест
   (`role="listbox"`) — выбран тест; §3/§10.1 стоит привести в соответствие, иначе W39d
   получит противоречивые требования.
5. Визуальная сверка по SPEC §14, `gui-window-probe.py`, AT-SPI, `cargo`, `gate:worker` —
   **не запускались** (WINDOW-BUDGET: один прогон у PM).
6. Коммитов/пушей нет, сеть не использовалась, `ui/tokens.css`, `app.js`, `model.js`,
   `ui/features/**`, `src/**`, `tests/**`, `waves/**` (кроме этой папки логов) не менялись.
