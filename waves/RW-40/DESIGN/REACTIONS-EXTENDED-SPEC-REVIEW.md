# DeepSeek Review 38A — REACTIONS-EXTENDED-SPEC (RW-40 / task-114)

Дата: 2026-10-30
Объект: `waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md` (ветка `design/task-114-reactions-picker-spec`, 1452 строки)
Режим: независимый read-only аудит кода и спецификаций; записан только этот файл.
Роль: технический ревьюер (не автор спецификации).

## Вердикт: RETURN

**P0 = 0, P1 = 10, P2 = 14.**

Нормативный CSS формально чист: все 83 уникальные переменные объявлены в `tokens.css`,
сырых цветов и `!important` в CSS-блоке нет. Но спецификация в текущем виде **не может быть
реализована как написано**: матрица контраста и таблица геометрии посчитаны по чужой палитре,
IPC-контракт ошибок не совпадает с сериализацией `ReactionError`, а DOM `.msg-toolbar` и
пикера противоречит не менее чем восьми замороженным проверкам в пяти файлах тестов и
`DOM-CONTRACT.md`. Ниже — точные места,
фактические значения и минимальный набор правок для повторного ревью.

---

## 1. Метод и воспроизводимость

Проверки выполнены механически, не «на глаз». Скрипты и промежуточные файлы лежали в
`/tmp/opencode/rw40rev/` (вне репозитория):

```sh
# 1) вырезать нормативный CSS-блок (строки 857–1413 спецификации)
awk '/^```css$/{f=1;next} /^```$/{if(f){f=0}} f' \
  waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md > /tmp/opencode/rw40rev/spec-css.css

# 2) используемые переменные
grep -oE 'var\(--[a-zA-Z0-9_-]+' spec-css.css | sed 's/var(//' | sort -u

# 3) объявленные токены
grep -oE '^\s*--[a-zA-Z0-9_-]+\s*:' rust/gui-spike/ui/tokens.css | sed 's/^ *//;s/ *:$//' | sort -u

# 4) разница (пусто = все имена объявлены)
comm -23 used.txt declared.txt

# 5) запрещённые конструкции в CSS-блоке
grep -nE '#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(|!important' spec-css.css
```

Результат:

| Метрика | Значение |
|---|---|
| `var(--...)` вхождений в CSS-блоке | **275** |
| уникальных имён | **83** |
| имён, отсутствующих в `tokens.css` | **0** |
| `var(--x, fallback)` (маскировка отсутствующего токена) | **0** |
| сырых `#hex` / `rgb()` / `rgba()` / `hsl()` в CSS-блоке | **0** |
| `!important` в CSS-блоке | **0** |

Контраст пересчитан по WCAG 2.2 §1.4.3/1.4.11 (sRGB, относительная яркость, альфа-композитинг
слоёв `--state-*`/`--accent-soft` поверх базовой поверхности), скрипт
`/tmp/opencode/rw40rev/contrast.py`.

---

## 2. Токены и CSS: что действительно в порядке

Это подтверждено и не требует правок:

1. **100% биекция имён.** Все 83 имени из CSS-блока объявлены в `rust/gui-spike/ui/tokens.css`
   (204 объявления). Выдуманных токенов нет; запрещённые в §0.2 `--warning-text`,
   `--surface-overlay`, `--mention-self-bg` в CSS не используются.
2. **Ни одного сырого цвета и ни одного `!important`** в нормативном CSS (строки 857–1413).
   Проверка §8 п.1 для `emoji.css` и `chat.css` тоже проходит: `grep` по ним даёт 0 совпадений.
3. Ссылки на существующие токены `--menu-border: 1px`, `--radius-round`, `--space-0: 0`
   синтаксически валидны в тех местах, где они употреблены (`border: var(--space-0)` = `border: 0`).
4. `.emoji-grid` c `repeat(var(--emoji-cols), var(--emoji-cell))` — валидный CSS Grid.
5. Контраст **текста** на реальной палитре проходит AA во всех парах, которые проверяет
   спецификация (см. §3 ниже): фактическая палитра даже щедрее, чем указано в §7.2.

То есть дефект не в токенизации, а в **числах и контрактах**, которые вокруг неё написаны.

---

## 3. Контраст WCAG 2.2 AA: пересчёт по реальным токенам

### 3.1. Палитра в §7.2 не из репозитория

`§7.2` цитирует значения, которых нет ни в `tokens.css`, ни в `THEME-SPEC.md`:

| Токен | §7.2 (Dark) | `tokens.css` | §7.2 (Light) | `THEME-SPEC.md` §2 |
|---|---|---|---|---|
| `--text` | `#dbdee1` | `#efeff1` | `#2e3338` | `#3f4048` |
| `--text-strong` | `#f2f3f5` | `#fbfbfb` | `#060607` | `#323339` |
| `--text-muted` | `#a5aab3` | `#96979e` | `#4e5058` | `#696a73` |
| `--text-subtle` | `#949ba4` | `#abacb2` | `#5c5e66` | `#5f606a` |
| `--text-accent` | `#99a4ff` | `#a5b0ff` | `#3c45a5` | `#4654c0` |
| `--danger-text` | `#ff7a7e` | `#ff9a9e` | `#a12828` | `#bd2934` |
| `--danger-fill` | `#d83c3e` | `#cf3a3f` | `#c93b3b` | `#cf3a3f` |
| `--warning` | `#f0b232` | `#fdb833` | `#b27808` | `#fdb833` |
| `--text-on-warning` | `#111214` | `#121214` | `#ffffff` | `#323339` |
| `--surface-1` | `#232428` | `#202024` | `#ebedef` | `#efeff1` |
| `--surface-2` | `#2b2d31` | `#242429` | `#f2f3f5` | `#f3f3f4` |
| `--surface-3` | `#35373c` | `#28282d` | `#e3e5e8` | `#ebeced` |
| `--bg-chat` | `#313338` | `#1a1a1e` | `#ffffff` | `#ffffff` |
| `--focus-ring` | `#5865f2` | `#a5b0ff` | `#4752c4` | `#4654c0` |

Причина видна: §7.2 взял палитру Discord из дампа, а не палитру проекта. Дополнительно,
`tokens.css` объявляет только `color-scheme: dark` и не содержит ни одного light-блока —
light-тема существует только как документ `THEME-SPEC.md` (карточка C3a, «docs only; код не
менялся»). Спецификация сертифицирует состояние, которое нельзя запустить.

### 3.2. Фактические коэффициенты (Dark, `tokens.css`)

| Пара из §7.2 | §7.2 заявляет | Факт | Норма | Итог |
|---|---|---|---|---|
| `.reaction-count`: `--text` на `--state-hover/--bg-chat` | 9.88 | **12.79** | ≥4.5 | PASS |
| `.reaction[data-me] .reaction-count`: `--text-accent` на `--accent-soft/--bg-chat` | 5.54 | **7.19** | ≥4.5 | PASS |
| Рамка выбранной пилюли: `--accent` vs `--bg-chat` | 3.18 | **3.77** | ≥3.0 | PASS |
| `.emoji-preview-name`: `--text-strong` на `--surface-1` | 13.86 | **15.69** | ≥4.5 | PASS |
| `.emoji-empty-title`: `--text-strong` на `--surface-2` | 13.86 | **14.93** | ≥4.5 | PASS |
| `.emoji-section-header` / `.emoji-empty-hint`: `--text-muted` на `--surface-2` | 5.96 | **5.31** | ≥4.5 | PASS |
| `::placeholder`: `--text-subtle` на `--surface-input` | 5.61 | **7.28** | ≥4.5 | PASS |
| `.reaction-picker-banner`: `--danger-text` на `--surface-3` | 5.12 | **7.24** | ≥4.5 | PASS |
| Баннер 429: `--text-strong` на `--surface-3` | 10.78 | **14.18** | ≥4.5 | PASS |
| Бейдж 429: `--text-on-warning` на `--warning` | 10.02 | **10.76** | ≥4.5 | PASS |
| Бейдж ошибки: `--text-on-danger` на `--danger-fill` | 4.56 | **4.85** | ≥4.5 | PASS (на грани) |
| Фокус-кольцо `--focus-ring` vs `--surface-2` | 3.48 | **7.54** | ≥3.0 | PASS |

Ни одна цифра §7.2 не воспроизводится, хотя вердикты PASS случайно совпали. Для ревью это
значит: **сертификация контраста не доказана**, её нельзя принять как evidence.

### 3.3. Non-text контраст нарушает собственный инвариант §0.2

§0.2 п.1 объявляет «контраст интерактивных границ и non-text индикаторов ≥ 3.0:1». Нормативный
CSS это нарушает в состоянии по умолчанию (самом частом):

| Элемент | Цвет | Фон | Факт | Норма |
|---|---|---|---|---|
| `.reaction` рамка `--border-subtle` (12% `#97979f`) | `#29292d` | `--bg-chat #1a1a1e` | **1.20** | 3.0 |
| `.reaction` заливка `--state-hover` (6% белого) | `#28282c` | `--bg-chat` | **1.18** | 3.0 |
| `.reaction:hover` рамка `--border` (20%) | `#333338` | `--bg-chat` | **1.38** | 3.0 |
| `.msg-toolbar` рамка `--border-subtle` | `#323237` | `--surface-2` | **1.21** | 3.0 |
| `.emoji-head` разделитель `--divider` (6%) | `#2d2d31` | `--surface-1` | **1.18** | 3.0 |

Это не косметика: у невыбранной пилюли реакции граница — единственный признак того, что это
кнопка (глиф и число — контент, а не граница контрола). WCAG 2.2 SC 1.4.11 требует 3:1 для
визуальной информации, нужной для идентификации компонента. Заменить на `--border-strong`
недостаточно: 44% альфы дают ≈2.2:1. Нужно дизайнерское решение (тинтированная граница уровня
`--accent`, либо явная заливка-подложка с контрастом ≥3:1), а не подстановка токена.

Матрица §7.2 эти пары просто не включает, поэтому дефект не виден в её таблице.

---

## 4. Сверка с реализацией: REST, IPC, Gateway, JS

### 4.1. Что совпало (проверено построчно)

- Маршрут `{base}/channels/{channel_id}/messages/{message_id}/reactions/{encoded_emoji}/@me` —
  `rust/gui-spike/src/reactions.rs:126-129`, `API_V9_BASE = "https://discord.com/api/v9"`
  (`discord-core/src/lib.rs:76`).
- `active=true` → `PUT` с телом `b"{}"`, `active=false` → `DELETE` без тела —
  `reactions.rs:130-140`.
- `percent_encode_emoji` (`reactions.rs:185-198`): unreserved `A-Za-z0-9-._~`, остальное `%XX`;
  таблица §6.2 верна, включая `👍 → %F0%9F%91%8D`, `party:id → party%3A...`,
  `❤️ (U+2764 U+FE0F) → %E2%9D%A4%EF%B8%8F`. Подтверждено `tests/reactions_contract.rs:85-89`.
- `MAX_EMOJI_CHARS = 256`, `chars().count()`, числовые id — `reactions.rs:16,169-182`.
- 401/403/404/429/5xx → типизированные варианты — `reactions.rs:200-202` +
  `rest_failure.rs:63-78`. Контрактный тест `reactions_contract.rs:217-237`.
- Оба события в белом списке: `FORWARDED_DISPATCHES` содержит `MESSAGE_REACTION_ADD` и
  `MESSAGE_REACTION_REMOVE` (`rust/gui-spike/src/net.rs:113-120`).
- `.row` — ровно два прямых ребёнка (`.author`, `.text`), `.reactions-row` монтируется внутрь
  `.text[data-stack="true"]` — `chat.js:2528-2548`, тест `gui-spike-dom.test.mjs`.
- Пилюля: `type=button`, `data-emoji`, `data-me`, `aria-pressed`,
  `aria-label="Реакция ${emoji}, ${count}"`, ровно два `span`, глиф с `aria-hidden` —
  `chat.js:207-224` совпадает с §2.1 дословно.
- `QUICK_REACTIONS = ["👍","❤️","😂","😮"]` — `rows.js:49`; четыре кнопки
  `[data-command="react"]` заморожены тестом.
- Слой состояния: реакции живут в UI (`reactions.js`) + `dispatch-model.js`, в
  `rust/crates/discord-core/src/state/reducer.rs` обработки реакций нет вообще (только имена
  событий в `dispatch.rs:33,119`). Спецификация это размещение описывает верно — упрёк не к ней;
  просто заявленная в задании «reducer.toggle_reaction» в коде отсутствует.

### 4.2. Расхождения

1. **`ReactionError` сериализуется иначе, чем описано.** В `reactions.rs:18-19`
   `#[serde(tag = "kind", rename_all = "camelCase")]` → `kind` = `invalidArg`, `unauthorized`,
   `forbidden`, `notFound`, **`rateLimited`**, `client`, `server`, `network`, `storage`,
   `invalidResponse`. Поля не переименовываются (`retry_after_ms` остаётся snake_case).
   Контракт это пинит: `reactions_contract.rs:112-113` требует `kind == "storage"`.
   Спецификация же вводит `kind == "rate_limited"` / `not_found` / `invalid_arg`, добавляет
   несуществующее поле `"message"` (§3.2.4) и теряет варианты `client` и `invalidResponse`.
   Матчер, написанный по спеке, не сработает ни на одной ошибке.
2. **Тексты `message` не из кода.** `Display` даёт «некорректные параметры реакции»,
   «сессия истекла», «нет доступа к каналу», «сообщение или канал не найдены»
   (`reactions.rs:44-61`), без `(403)`, `(404)`, без `{msg}`. UI-тексты по §3.2.5 — новые
   строки, это нормально, но колонка «Русский текст `message` из `reactions.rs`» ложная.
3. **Сигнатура команды.** `main.rs:392-411`: `State<'_, Arc<AuthBackend>>`, а не
   `State<'_, SharedBackend>`; строка `main.rs:95` — это другой код. Отсутствие токена даёт
   `ReactionError::Unauthorized` (unit-вариант, без текста), ошибка хранилища — `Storage`.
4. **`validate()` не делает `trim()`.** §6.1 обещает «`emoji.trim()` не пуст», а
   `reactions.rs:169-177` проверяет `emoji.is_empty()` без trim: строка из пробелов проходит
   валидацию и уходит в сеть как `%20%20%20`. Либо чинить Rust, либо убрать обещание.
5. **429-задержка.** Функции `parse_retry_after_ms` в `reactions.rs` нет (она есть в
   `invites.rs`, `notifications.rs`, `pins_threads.rs` — похоже, скопировано оттуда). Разбор
   `Retry-After`/тела живёт в `discord-core/src/rest/rate_limit.rs:143-175` и доезжает до UI
   только через `ApiError::RateLimited{retry_after}` → `RestFailure::from_api_error`
   (`rest_failure.rs:89-91`). Сырой статус даёт `retry_after_ms: 0`
   (`rest_failure.rs:71`, `:96`). Кроме того, `ApiClient` сам ретраит 429 с backoff до
   `max_attempts` (`client.rs:399-430`) — то есть откат оптимистичного состояния и баннер
   появляются не сразу, а после исчерпания повторов; в спеке этого слоя нет вообще.
   Пример `{"retry_after_ms": 1500}` из §3.2.4 недостижим на пути `from_status`, а правило
   «`0` → показываем 1 с» нигде не зафиксировано (оно есть только неявно в
   `Math.max(1, ceil(...))`).
6. **JS-контроллер описан не тем API.** Фактически:
   `requestReaction(key, emoji, base, { element, channelId, messageId, onSettled })`
   (`reactions.js:165-209`), `toggleReaction(key, emoji, base, options) → boolean`
   (`:127-148`). Никаких «message-объектов», `onOptimistic`, `onError`, `previous`,
   `{active, list}` нет. `overrides` — `Map<key, Map<emoji, {count, me}>>` (`:23`), а не
   `Map<string, Array<...>>`. Ключ pending — `${String(key)}:${emoji}` (`:166`), где
   channel-ключ = `channel:<channelId>:<messageId>` (`:43-45`); формула
   `` `${channelId}:${messageId}:${emoji}` `` из §0.2 п.5 неверна.
7. **`projectReaction` устроен иначе.** `dispatch-model.js:776-820` —
   `projectReaction(store, kind, data, counters)`: держит `Map` ключ `channel:message:emoji`
   с `Set<userId>`, дедуплицирует повторный ADD/REMOVE, возвращает
   `{type:"reaction", channelId, messageId, emoji, count, me, removed}` — **абсолютный счётчик**,
   не `{userId, delta}`. §6.3 описывает другую модель (дельты ±1) и другую защиту от эха.
   При этом фактическая защита от эха — `chat.js:3677-3700` (`applyReaction`):
   базовая линия `liveReactionCounts`, `displayCount = currentCount - previousLive + liveCount`
   и `nextMe = change.me ? !change.removed : currentMe`, плюс `adjustReactionOverride`
   (`reactions.js:225-251`). Это не «isSelf && delta > 0» из §6.3.
8. **Триггер «Добавить реакцию» сейчас не открывает пикер.** `message-toolbar.js:204-206`:
   `command === "react" || command === "add-reaction"` → `requestReaction(..., emoji || "👍")`,
   то есть пункт «Добавить реакцию» ставит 👍. Узлов `.reaction-add-btn`,
   `#reaction-picker-popover`, `#reaction-picker-banner`, `#emoji-loading-state`,
   `#emoji-empty-state`, `.emoji-grid-viewport`, `.emoji-section-header` и атрибутов
   `data-reacted`, `data-state` на карточке в `ui/` нет ни одного (проверено `grep -rl`).
   Спецификация помечает эти состояния `[F]` — маркер «зафиксированный контракт в коде и
   тестах» к ним неприменим.

---

## 5. DOM, WAI-ARIA и тестовые оракулы

### 5.1. Замороженные тесты, которые спека ломает

| Оракул | Требование | Спека |
|---|---|---|
| `gui-spike-features.test.mjs:396`, `gui-spike-dom.test.mjs:490` | `cell.title === cell.textContent` | `title=":thumbsup:"` при тексте `👍` — **FAIL** |
| `gui-spike-features.test.mjs:386` | `tab.getAttribute("aria-controls") === "emoji-grid"` | в DOM-сниппете вкладок `aria-controls` нет — **FAIL** |
| `gui-spike-features.test.mjs:1159` | `.msg-toolbar` ровно **одна** на чат | «монтируется внутри активной `.row`» — **FAIL** |
| `gui-spike-features.test.mjs:1162` | `data-open === "false"` без hover | атрибута/правила `[data-open]` в CSS нет — **FAIL** |
| `gui-spike-features.test.mjs:1173` | 15 кнопок в панели (R-16) | 5 кнопок — **FAIL** |
| `gui-spike-dom.test.mjs:804` | `#chat-list .msg-toolbar` == 0 | тулбар внутри `.row` → в списке — **FAIL** |
| `chat-reactions.test.mjs:396-398`, `gui-spike-features.test.mjs:1269/1328/1392`, `channel-switch.test.mjs:536/556/577`, `gui-spike-regressions.test.mjs:174` | поиск кнопок по `aria-label` `Изменить/Удалить сообщение`, `[data-command="reply"/"delete"]` | этих 10 кнопок действий (`MESSAGE_ACTIONS` без `add-reaction`) в DOM спеки нет — **FAIL** |

§2.4 утверждает, что Режим A «не ломает существующие unit-тесты». Это неверно минимум по
`title` и `aria-controls`.

### 5.2. ARIA-дефекты самой спеки

1. `aria-pressed` на элементе с `role="option"` (`§2.3`, `§5.1`) — недопустимое сочетание:
   `aria-pressed` определён для `button`, у `option` есть `aria-selected`. Дублировать
   состояние двумя взаимоисключающими механиками нельзя; скринридер прочитает «выбрана,
   нажата» непредсказуемо.
2. `role="tablist"` из 10 вкладок управляет `role="listbox"`, а не `role="tabpanel"`.
   По APG вкладка обязана ссылаться на tabpanel; `tablist → listbox` — антипаттерн.
3. Двумерная навигация (`ArrowDown = +9`) внутри `role="listbox"` отклоняется от APG listbox
   (там только вертикаль). Двумерность — это паттерн `grid`. Спека делает listbox дефолтом,
   а grid оставляет «опциональным режимом» через `gridRole: "grid"` — такой опции у
   `createEmojiConsole(options)` нет (`emoji.js:339-341`), то есть Режим B из §2.4 нереализуем.
4. `role="alert"` + `aria-live="assertive"` на баннере, текст которого переписывается
   **каждую секунду** обратного отсчёта — скринридер будет прерывать пользователя раз в секунду.
   Нужен статичный текст в alert и отдельный `aria-live="off"`/`polite` счётчик.
5. §5.1 разрешает стационарной вкладке `role="region"`, но `index.html:139` уже содержит
   `role="dialog"` на той же `.emoji-card`; это замороженный атрибут, менять — решение PM.
6. `aria-expanded`/`aria-haspopup` есть, а `aria-controls` у `.reaction-add-btn` нет
   (у кнопки в тулбаре он есть) — несогласованность двух триггеров одного поповера.

### 5.3. Полнота оракулов

Новых узлов (`#reaction-picker-popover`, `.reaction-add-btn`, баннер, loading/empty, `data-reacted`,
`data-state`, `data-pending`+`aria-busy`) не покрывает **ни один** тест. §8 требует проверять их
«чек-листом», но чек-лист не содержит ни одного `red`-теста; ссылки на существующие оракулы
указывают не туда (`chat-reactions.test.mjs:139` — ветка фейкового `invoke`
(`messages_page`), а не проверка `aria-label`; `gui-spike-dom.test.mjs:142` — JSDoc функции
`scrollList`).
Дополнительно: `aria-busy` на пилюле спека требует, а код выставляет только `data-pending`
(`reactions.js:177,206`).

---

## 6. P1 — блокеры (10)

**P1-1. Матрица контраста не воспроизводима.** Значения Dark не из `tokens.css`, значения Light
не из `THEME-SPEC.md` и не реализуемы (light-блока в `tokens.css` нет). Все 12 коэффициентов
§7.2 неверны (см. §3.2), сертификация не является evidence.
*Правка:* пересчитать по `tokens.css` (dark) и `THEME-SPEC.md` §2.6 (light), вписать фактические
числа, убрать light-колонку либо пометить её «design-target, не реализовано».

**P1-2. Non-text контраст ниже собственного инварианта.** 1.18–1.38:1 для рамки/заливки
невыбранной пилюли и рамки тулбара при требовании ≥3:1 (§0.2 п.1). Дизайн-решение, не подстановка
токена (см. §3.3).
*Правка:* либо новый токен границы контрола с ≥3:1, либо заливка-подложка, либо явное исключение
в §0.2 с обоснованием WCAG.

**P1-3. Маппинг токен → значение в §4.1/§2.2/§3.1 систематически сдвинут.** Таблица считает
`--space-5` = 8px, `--space-6` = 12px, `--space-7` = 16px, `--space-3` = 4px, `--space-2` = 2px,
`--space-4` = 6px. Фактически: 12, 16, 20, 6, 4, 8 (tokens.css:174-187). Дополнительно неверны
`--search-radius` (объявлен `var(--radius-round)` = пилюля, спека пишет `var(--radius-sm)` = 8px,
tokens.css:293-294), `--state-disabled-opacity` (0.45, спека — 0.48, tokens.css:128),
`--scrollbar-size` (8px, спека — 6px, tokens.css:138), `--focus-ring` (#a5b0ff, спека — #5865f2,
§3.1 состояние 6), `box-shadow: var(--shadow-popover)` (это high **+** stroke, спека пишет только
high). Бюджет ширины арифметически ложен: 48 + 2×12 + 396 + 32 = **500** > 498; при `overflow-x:
hidden` девятая колонка обрезается на 2px, а с вертикальным скроллбаром — до 10px.
*Правка:* пересчитать таблицу по фактическим значениям; либо изменить `padding` грида на
`--space-4` (8px), чтобы бюджет сходился в 498.

**P1-4. Нормативный CSS не раскладывает дефолтное состояние.** `.emoji-card` —
`grid-template-rows: auto auto 1fr auto`, но `#reaction-picker-banner` по умолчанию `hidden`
(`primitives.css:64` — `display:none !important`). В состоянии `ready` в гриде три элемента:
header → row 1, body → row 2 (`auto`), footer → row 3 (`1fr`); тело не получает `1fr` и
выпихивает футер за 440px. Та же ошибка во втором состоянии: `.emoji-grid` и
`.emoji-empty-state`/`.emoji-loading-state` все имеют `flex: 1` в одном flex-контейнере, поэтому
пустое состояние делит область 50/50 с пустой сеткой.
*Правка:* либо фиксированные `grid-template-areas`, либо `grid-template-rows: auto auto 1fr auto`
только при видимом баннере (`.reaction-picker-banner:not([hidden]) ~ …`), либо постоянная строка
баннера нулевой высоты; для viewport — `flex:1` только у видимого состояния.

**P1-5. Контракт ошибок IPC не соответствует сериализации.** `kind` в camelCase
(`rateLimited`, `notFound`, `invalidArg`), без поля `message`, плюс не упомянуты `client` и
`invalidResponse`; сигнатура команды и строка `main.rs` неверны (см. §4.2 п.1-3).
*Правка:* таблица §3.2.5 и JSON-пример §3.2.4 — по `ReactionError` (serde) и `main.rs:392-411`;
добавить `client`/`invalidResponse` с UI-текстами.

**P1-6. Семантика 429.** `parse_retry_after_ms` в `reactions.rs` отсутствует; задержка приходит
только через `ApiError::RateLimited`, а сырой статус даёт 0; собственный retry-цикл `ApiClient`
в спеке не упомянут; пример 1500 мс недостижим на статусном пути; правило для 0 не зафиксировано
(см. §4.2 п.5).
*Правка:* описать оба пути (`ApiError::RateLimited` → реальное значение, иначе 0), зафиксировать
floor (например, 1 с) и то, что баннер появляется после исчерпания автоповторов.

**P1-7. API контроллера и модель синхронизации описаны неверно.** `requestReaction`/
`toggleReaction`, `overrides`, `pending`-ключ (см. §4.2 п.6) и `projectReaction` (абсолютный
`count`, дедуп по `Set<userId>`, а не `delta`; §4.2 п.7). Код, написанный по §0.2 п.5 и §6.3, не
скомпилируется/не сойдётся с существующим состоянием.
*Правка:* переписать §0.2 п.5, §6.1 и §6.3 под фактические сигнатуры либо явно объявить
рефакторинг `reactions.js`/`dispatch-model.js` отдельной карточкой с миграцией тестов.

**P1-8. DOM пикера ломает замороженные оракулы и содержит невалидный ARIA.** `title` вместо
глифа, отсутствие `aria-controls` на вкладках, `aria-pressed` на `option`, `gridRole` без
реализации (см. §5.1, §5.2 п.1-3).
*Правка:* `title` = глиф (или менять тест отдельной карточкой), `aria-controls="emoji-grid"` на
каждой вкладке, оставить только `aria-selected` (+`aria-current` при необходимости), убрать
Режим B или добавить `gridRole` в API консоли.

**P1-9. Архитектура `.msg-toolbar` не соответствует реализации.** Спека: 5 кнопок внутри
`.row`, `z-index: var(--z-header)`, `background: var(--surface-2)`, без `display:none`.
Факт (`message-toolbar.js:59-105`, `chat.css:355-379`): **одна** панель на чат, вне `#chat-list`,
15 кнопок (4 реакции + 11 `MESSAGE_ACTIONS`), видимые текстовые подписи,
`aria-label="Действия с сообщением N"`, скрытие через `display:none` + `[data-open="true"]`,
позиция — inline `top/right` по строке. Прямое применение CSS-блока оставит панель всегда
видимой и сломает 8 замороженных проверок (§5.1).
*Правка:* либо переписать §2.2 под «одна панель на чат, 15 пунктов», либо выделить изменение
архитектуры в отдельную карточку с правкой тестов и `chat.css` (и обосновать, зачем per-row).

**P1-10. Дубли `id` между стационарным экраном и поповером.** `#screen-emoji` уже владеет
`#emoji-search`, `#emoji-tabs`, `#emoji-grid` (`index.html:137-152`), а §2.3 даёт те же id второму
экземпляру `.emoji-card`. При одновременном монтировании `byId()` (`emoji.js:508-510`) привяжется
к первому, `aria-controls` будет указывать не туда, `initEmojiConsole` инициализирует чужой узел.
*Правка:* префикс id на экземпляр (`#reaction-picker-search` и т.д.) либо scoped-запросы от
корня карточки; в §2.3 явно указать, что два экземпляра не сосуществуют.

---

## 7. P2 — точность и поддерживаемость (14)

1. **Номера строк в цитатах в основном ложные.** Заявлено → фактически:
   `tokens.css:235/256/261/297/298/323/324-325/326/327/281` → 194/209/338/293/294/310/311-312/313/314/226
   (ни одна не совпала);
   `emoji.css:17,16,54,53,62-63,66,91,93,103,106,125,126` → 36,34,52,50,62-67,71,98-105,119-128,145-151
   (совпал только селектор `.tab` на 62-67);
   `chat.css:202,203,212,213,215,218,248,249,251,257-258` → 182,183,190,195,198,213,218,355,359,370
   (совпали только 213 и 218 — но это `.reaction-emoji`/`.reaction-count`, а не приписанные им
   свойства); `emoji.js:118,179-214,347-349,400` → 283,455-490,368-369,398;
   `dispatch-model.js:90` → 776; `net.rs:28-29` → 118-119; `main.rs:95` → 392;
   `chat.js:244` → 232, `chat.js:248-249` → 238-240.
2. **`[F]`-маркеры на несуществующих сущностях** (`data-state` пикера, баннер, loading/empty,
   `data-reacted`, `.reaction-add-btn`, `gridRole`) — §3.2.1-3.2.5, §2.4.
3. **`reactionsGroupOf` не возвращает `null` при пустом списке** (§2.1): функция всегда создаёт
   группу (`chat.js:232-244`), а условие проверяет вызывающий (`chat.js:2529`). Формулировка
   «возвращается null» неверна.
4. **`.emoji-row { display: contents }`** противоречит шапке `emoji.css:4-5` («никакого
   display:contents на контейнерах с ролью») и текущему `.emoji-row { display:grid; grid-column:
   1/-1 }`; вдобавок `display:contents` на `role="row"` в ряде браузеров выбрасывает строку из
   a11y-дерева. `DOM-CONTRACT.md` §9.2 (D-05) просит обратное — нужно одно решение, а не два
   документа с противоположными указаниями.
5. **Не сказано, заменяет ли CSS-блок секции `emoji.css`/`chat.css`.** Конфликтуют ≥12 деклараций:
   `.emoji-card` `radius-md` vs `radius-lg`, `display:grid` vs `flex`, `.tab` 32×32 vs 100%×32 и
   `radius-sm` vs `radius-xs`, `.emoji-tabs` padding 8 vs «4 0», gap 4 vs 2,
   `.emoji-grid` padding 12 vs 8 и `grid-auto-rows` vs отсутствие, `.emoji-cell` font 20 vs 24 и
   `radius-sm` vs `radius-xs`, `.emoji-cell[aria-selected]` `state-hover` vs `state-selected`,
   `.emoji-preview` `min-height` vs `height`, `.msg-toolbar` `surface-2` vs `surface-4`.
   Без указания файла и порядка каскада G2 получит неопределённый результат.
6. **2D-навигация в listbox и недостижимый Режим B** (§2.4) — см. §5.2 п.3.
7. **Смена `aria-label` грида** на `"Сетка эмодзи (9 колонок)"` против замороженного
   `DOM-CONTRACT.md:347` (`aria-label="Эмодзи"`) — нужно решение PM, а не тихая правка.
8. **`tablist` без `tabpanel`** (§5.2 п.2) — структурная ошибка, отдельная от `aria-controls`.
9. **`role="alert"` с ежесекундным обратным отсчётом** (§3.2.4, §2.1) — AT-спам.
10. **`aria-disabled` без правила no-op.** §3.1 состояние 7 обещает «клик не отправляет запрос»,
    но в §7.1 нет `pointer-events`/`disabled`-правила, а `aria-disabled` клик не блокирует;
    `:hover`-правила это учитывают, а обработчик — нет.
11. **`--warning-text`** запрещён в §0.2 как «выдуманный», но введён `THEME-SPEC.md` §2.5 как
    единственный допустимый текст предупреждения для light. Либо разрешить, либо зафиксировать
    запрет в THEME-SPEC.
12. **`validate()` без `trim`** — расхождение с §6.1 (см. §4.2 п.4).
13. **Нет оракулов для новых узлов**, а §8 ссылается на несуществующие проверки
    (`chat-reactions.test.mjs:139`, `gui-spike-dom.test.mjs:142`) — см. §5.3.
14. **`EMOJI_COLS = 9` против `auto-fill` на <540px.** §4.3 сам признаёт, что шаг стрелок нужно
    пересчитывать по фактическому числу колонок, но `emoji.js:14` — константа 9, и способа
    получить динамическое `cols` из консоли нет; `ArrowDown` будет прыгать на 9 при 6 видимых.

---

## 8. Что признано корректным (не переделывать)

- REST-маршрут, выбор `PUT`/`DELETE`, тело `b"{}"`, percent-encoding и таблица §6.2 — точны.
- Валидация id/длины эмодзи и `MAX_EMOJI_CHARS = 256` — совпадают (кроме `trim`).
- Маппинг 401/403/404/429/5xx на типизированные ошибки — совпадает по статусам.
- Белый список Gateway и оба события реакций — совпадают.
- Инвариант `.row` (2 ребёнка) и размещение `.reactions-row` в `.text[data-stack="true"]` —
  совпадают с кодом и тестами.
- Атрибуты пилюли (`.reaction`), включая `aria-label="Реакция ${emoji}, ${count}"` и два `span`
  с `aria-hidden` на глифе — дословно совпадают с `chat.js:207-224`.
- `QUICK_REACTIONS` (4 эмодзи) и правило разрешения конфликта с RE-дампом (`frecency`) — разумны
  и не ломают контракт «4 кнопки `[data-command="react"]`».
- Запрет `innerHTML`/`insertAdjacentHTML` и требование `textContent`/`setAttribute` — соответствуют
  правилам репозитория.
- Позиционирование поповера (§4.2, формулы `idealLeft`, collision) — логически корректно; правки
  нужны только в константах (`margin` 8 vs `--space-5` 12).

---

## 9. Условия повторного ревью

Спека может получить PASS после того, как:

1. §7.2 пересчитана по `tokens.css` и `THEME-SPEC.md` (с фактами, не с палитрой Discord), а
   light-колонка либо удалена, либо помечена как design-target.
2. Non-text пары добавлены в матрицу и доведены до ≥3:1 (или §0.2 исправлен с обоснованием).
3. §4.1/§2.2/§3.1 переписаны по фактическим значениям токенов; бюджет 498px сходится
   арифметически.
4. Контракт ошибок (§3.2.4, §3.2.5) совпадает с serde `ReactionError`, 429-путь описан вместе
   с retry-циклом `ApiClient` и floor для `retry_after_ms = 0`.
5. §0.2 п.5, §6.1, §6.3 совпадают с фактическими сигнатурами `reactions.js`/`dispatch-model.js`
   (или рефакторинг вынесен в отдельную карточку).
6. DOM пикера и `.msg-toolbar` либо не ломают замороженные тесты, либо изменение тестов
   согласовано отдельным решением PM, а не «не ломает» в тексте.
7. Для каждого нового узла/состояния (`#reaction-picker-popover`, баннер, loading/empty,
   `data-reacted`, `data-pending`) есть red-first оракул с точной ссылкой `файл:строка`.
8. Убраны дубли `id` между `#screen-emoji` и поповером.

До этого реализацию начинать нельзя: часть блокеров (P1-3, P1-4, P1-9) проявится только в
рантайме, а P1-1/P1-5/P1-7/P1-8 дадут ложное «зелёное» на ревью, потому что проверяются по
неверным числам и несуществующим контрактам.
