# DeepSeek Review 37A — M-13-SEARCH-202-NSFW-UI

**Вердикт: RETURN (P0 = 0, P1 = 3, P2 = 8).**

Ревью ветки `rw40/m13-search-202-ui` перед слиянием в `unstable`. HEAD:
`4e240481e59ff4d49732ac8bd5ba08a006363748`. Спецификация принята и заморожена:
`SEARCH-202-NSFW-SPEC.md` md5 `4d07bc2fa8ecaa964e99f546096b6bd8`, RE-досье
`SEARCH-202-NSFW-RE.md` md5 `e5a2d7bc734a86644002fc8bd4b31ad0` — совпадают с
`SEARCH-202-NSFW-SPEC-ACCEPT.md`.

Функциональное ядро M-13 (202-retry, формула NSFW-гейта, snowflake-границы,
потолок пагинации) реализовано и подтверждено тестами. Блокируют приёмку три
дефекта: два в CSS (несуществующие токены и контраст 4.42:1 вместо 4.85:1) и
один в персистентности согласия NSFW. Ни один из них тестами не покрыт.

## 1. Прогоны и evidence

Окружение: Node `v26.13.0`, TypeScript `7.0.2`.

| Команда | Результат |
|---|---|
| `node --test tests/unit/search-remote-real-index.test.mjs` | `tests 18`, `pass 18`, `fail 0`, `skipped 0` |
| `node --test tests/meta/test-policy.test.mjs` | `tests 17`, `pass 17`, `fail 0` |
| обе команды вместе | `tests 35`, `pass 35`, `fail 0` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit `0` |
| `node --test tests/unit/gui-spike-features.test.mjs` (дополнительно) | `tests 100`, `pass 100`, `fail 0` |

MD5 совпадают с `REPORT.md:103-109` полностью:

```
d43971eba87ff56d59a70c9d7ed2a0e2  rust/gui-spike/ui/features/search.js
0bcea1b4a328b0794ee371f090f12530  rust/gui-spike/ui/features/search-overlay.js
fb419354a5f66bc4394b156f5fbb40f8  rust/gui-spike/ui/components/chat.css
bc7ee7fbd5f1b49417e71c2f94104f29  tests/unit/search-remote-real-index.test.mjs
d5fa2fbae82022a3289bd5ecde3576ed  waves/RW-40/PM/m13-search-ui/red.log
52565df50ec828b248a214d0758d0f04  waves/RW-40/PM/m13-search-ui/green.log
4530fd4b48c9d14201860cf1dc70b848  waves/RW-40/PM/m13-search-ui/mutations.log
```

Red/green-логи воспроизводят заявленные прогоны. `mutations.log` — 5/5 KILLED,
baseline MD5 восстановлен (`match=true`). Тестовый файл изменён только
добавлением (+799 строк, удалённых строк нет), `search.js` — тоже только
добавление (+675 строк), ослаблений существующих проверок не найдено.

## 2. Что сходится с контрактом

- **HTTP 202.** `computeIndexingRetryDelayMs` игнорирует тело `202`
  (`retry_after`, `documents_indexed`), читает `Retry-After` через `parseInt`,
  при `0`/`0.25`/отсутствии заголовка отдаёт `5000` мс, поддерживает IPC-поле
  `retry_after_ms` (`search.js:517-533`). Один таймер `indexingPollId`,
  отмена в `reset()`, при вводе, смене сервера и `close()`
  (`search-overlay.js:737-742, 1359-1364, 1549-1552, 1683-1691`).
- **Лимит 5 повторов.** `indexingAttempts > 5` → `data-indexing-state="exhausted"`,
  `aria-valuetext="5 из 5 попыток"`, кнопка `.search-indexing-retry-btn`, новый
  таймер не создаётся (`search-overlay.js:1486-1502`). Повтор уходит с
  `attempts: 1..5` (`search-overlay.js:1053`, `search.js:685-687`).
- **Глубокая историческая индексация.** На `200 OK` с
  `doing_deep_historical_index: true` карточки рендерятся, баннер получает
  `data-indexing-state="historical"`, текст `formatHistoricalIndexedStatus(8450)`
  дословно даёт `4Y3O+O` (`search.js:540-557`, `search-overlay.js:1432-1442`).
- **NSFW-формула.** `AGE_GATED_SPACES = 1`, `ADULT_VERIFICATION_STATUSES = [3, 4]`,
  проверка маски и статуса выполняется до чтения стора; при `!didAgree` ключ
  `include_nsfw` удаляется из `query`, а не передаётся как `false`
  (`search.js:420-443, 497-507`). Режимы `prompt`/`underage` и блокировка
  сетевого запроса до согласия подтверждены `T-M13-NSFW-01..04`.
- **Snowflake-оракул.** Строговая сигнатура `searchDateBounds("2026-10-20")`
  сохранена; объектная форма даёт ровно
  `max_id=1477455328051200000` и `min_id=1456436831842205696`
  (`search.js:144-178`).
- **Потолок пагинации.** `MAX_SEARCH_OFFSET = 9975`, `MAX_SEARCH_PAGES = 400`,
  запрос с `offset > 9975` блокируется, `.search-more` скрывается, в статус
  уходит строка `E+2azY` (`search-overlay.js:1543-1547, 1412-1423, 1459-1461`).
- **CSS token-only.** Во всём `chat.css`: `0` сырых `#hex`/`rgb()`/`hsl()` и
  `0` `!important` (проверено `grep -cE`).

## 3. Возврат: P1

### P1-1. Четыре CSS-переменные не объявлены в `tokens.css`

Файл: `rust/gui-spike/ui/components/chat.css`.

| Строка | Использование | Объявление в `tokens.css` |
|---|---|---|
| 1491 | `background: var(--danger-bg)` (чип с отрицанием) | нет |
| 1590 | `animation: search-indexing-spin var(--motion-slow) linear infinite` | нет |
| 1702 | `background: var(--danger-bg)` (NSFW-баннер) | нет |
| 1753 | `padding: var(--btn-pad-y) var(--btn-pad-x)` (кнопки NSFW) | `--btn-pad-y` нет, `--btn-pad-x` есть |
| 1772 | `background: var(--danger-hover)` (hover «Продолжить») | нет |

Проверка: `grep -rn -- "--danger-bg:\|--danger-hover:\|--btn-pad-y:\|--motion-slow:" rust tests`
не находит ни одного определения, включая `tokens.css`. Спецификация §0.2.1
требует: «Каждая CSS-переменная в этой спецификации **обязана** существовать в
`rust/gui-spike/ui/tokens.css`».

Последствие не косметическое: невалидная `var()` делает объявление invalid at
computed-value time. Спиннер индексации не вращается (`animation` → `none`),
фон NSFW-баннера и отрицающего чипа становится прозрачным, у кнопок
`.search-nsfw-btn` схлопывается `padding`. В отличие от P2 это видно сразу в
живом окне.

Исправление: `--danger-bg` → существующий `--surface-2` (или новый токен через
Team Lead, `tokens.css` — общий файл); `--danger-hover` →
`--danger-fill-hover`; `--btn-pad-y` → `--space-1`/`--space-2`; `--motion-slow`
→ новый токен длительности либо `--motion-base`.

### P1-2. Бейдж 18+ и кнопка «Продолжить» не проходят WCAG AA

Файл: `chat.css:1712-1727, 1766-1769`. Бейдж 18+ и destructive-кнопка используют
`background: var(--danger)` + `color: var(--text-on-accent)`.

Спецификация §0.2.2 и таблица §2.2 нормируют для них
`var(--danger-fill)` + `var(--text-on-danger)` (контраст 4.85:1) и прямо
оговаривают: «`--danger` `#da3e44`, который предназначен только для
иконок/рамок non-text ≥ 3:1». Сам `tokens.css:103` фиксирует, что белый на
`--danger` даёт **4.42:1 < AA**. Текст бейджа — 11px, кнопки — 12px, то есть
обычный (не large) текст, порог 4.5:1.

Исправление: `--danger` → `--danger-fill`, `--text-on-accent` →
`--text-on-danger`, hover → `--danger-fill-hover`.

### P1-3. Согласие NSFW не сохраняется в `GuildNSFWAgreeStore`

Файл: `rust/gui-spike/ui/features/search-overlay.js:750-754`.

```js
const desc = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
return desc && "value" in desc && desc.value ? desc.value : null;
```

В браузере и WebView `localStorage` — accessor-свойство (`get`/`set`), а не
data property. Проверено в целевом окружении:
`Object.getOwnPropertyDescriptor(globalThis, "localStorage")` возвращает ключи
`['get','set','enumerable','configurable']`. Значит `"value" in desc` всегда
`false`, функция всегда возвращает `null`, и `recordNsfwAgreement`
(`search-overlay.js:773-786`) пишет согласие только в память. После перезагрузки
пользователь снова получает гейт, хотя спецификация §4.1.3 требует записи карты
`{ [guildId]: true }` в `localStorage` под ключом `GuildNSFWAgreeStore`, когда
хранилище доступно.

Исправление: определять доступность через `typeof window !== "undefined"` и
затем `window.localStorage` в `try/catch` (в Node не трогать getter, чтобы не
ловить `ExperimentalWarning`); либо читать `desc.get.call(globalThis)` при
наличии `desc.get`.

## 4. P2

### P2-1. Тексты расходятся с каталогом `ru-catalog.json`

Нормативные строки §1.2/§3.3/§4.3 заменены своими:

- `.search-indexing-title` при `202`: `"Индексация сообщений сервера…"`
  (`search-overlay.js:588, 1509`) вместо `G3EA+4` `"Индексация"`.
- `.search-indexing-title` при `historical`: `"Архивная индексация продолжается"`
  (`:1435`), бейдж `"Проиндексировано: N"` (`:1436-1438`) вместо `"Индексация"` /
  `"Архив"`.
- `.search-nsfw-desc` в режиме `prompt` (`:637-638`) — свой текст вместо
  `E4Cd5I` «В этом канале содержится контент для взрослых с возрастными
  ограничениями. Хотите продолжить?».
- `.search-indexing-detail` в `exhausted` (`:1495-1496`) — свой текст вместо
  `AXPbZr` «Нам нужно проиндексировать этот сервер перед поиском. Дайте нам
  минуточку.» (§3.3 требует `AXPbZr`).
- Кнопка повтора: `"Повторить поиск"` (`:612`) вместо `"Повторить запрос"` (§3.3).

Тесты проверяют только подстроку `AXPbZr` в состоянии `indexing`, точный текст
`4Y3O+O` и `underage`-описание; остальные строки не покрыты.

### P2-2. Селекторы чипов разошлись с нормативным DOM и §6

`search-overlay.js:947-948` создаёт `span.search-chip-key` и
`span.search-chip-val`; `chat.css:1495-1511` стилизует их же. Нормативная
разметка §2.1 и нормативный CSS §6 используют `span.search-chip-prefix` и
`span.search-chip-value`. Также не выставляется `data-chip-remove` из §2.1.
Внутри реализации это согласовано, поэтому визуально не ломается, но
замороженный контракт нарушен: любой тест/стиль на `search-chip-prefix` или
`data-chip-remove` не сработает. Либо привести имена к §2.1/§6, либо зафиксировать
отступление в спеке через Team Lead.

Дополнительно §6 — нормативный блок, а текущий CSS переработан (фон
`.search-indexing-badge` `--surface-2` вместо `--warning`, `historical`-бар
`--info`, другие отступы). Если это осознанная адаптация, её нужно закрепить в
документе; иначе привести к §6.

### P2-3. NSFW-гейт покрывает только фильтр `in`, результаты не очищаются

`refresh()` (`search-overlay.js:1636-1643`) проверяет только `filters.in` и
инлайн-`in:`. Спецификация §4.2 (п. 1) требует гейт также для текущего открытого
NSFW-канала и `guild.nsfw_level ∈ {1, 3}`, и там же предписывает очищать
`.search-results` до решения пользователя. Сейчас старые карточки остаются
видимыми под баннером, а поиск в NSFW-гильдии без `in:` баннер не показывает.
Сценарий B §4.2 (п. 2) (баннер над общегильдийской выдачей при найденных
NSFW-каналах) не реализован. Сетевой запрос при этом не утекает — это
подтверждено тестом.

### P2-4. Фокус крадётся на каждом `refresh`, «Вернуться» оставляет старое состояние

`showNsfwGate` всегда вызывает `focus()` на `Continue`/`Back`
(`search-overlay.js:852, 859`), а `refresh()` вызывает его на каждый `input`
(`:1696-1702`). Пока баннер открыт, пользователь не может править строку
запроса с клавиатуры: после первого символа фокус уходит на кнопку. §7.1
оговаривает перевод фокуса «при открытии баннера (когда фокус не находится в
`.search-overlay-input`)», то есть не на каждом пересчёте.

`nsfwBackBtn` (`:874-882`) очищает `in`, скрывает баннер и возвращает фокус, но
не вызывает `refresh()`; статус и выдача остаются от предыдущего запроса. По
§4.3 достаточно скрыть баннер и вернуть фокус, но тогда нужен явный сброс
статуса/выдачи, иначе экран врёт.

### P2-5. Двусторонняя синхронизация, удаление чипов и клавиатура неполные

- `syncFilterChips` (`search-overlay.js:893-927`) только читает контролы и
  инлайн-токены; инлайн-токены не заполняют `.search-sections`. §8
  `T-M13-CHIPS-01` ожидает «контролы в `.search-sections` синхронизированы».
- Удаление инлайн-чипа (`:956-976`) ищет `chip.raw` в `query.split(/\s+/)`;
  для `from:"Alex Mercer"` токен с пробелом не находится, чип остаётся.
- `.search-chips-clear` (`:992-1008`) не возвращает фокус в
  `.search-overlay-input`, как требует §5.3.3.
- Не реализованы `Backspace` в позиции 0 (переход на последний
  `.search-chip-remove`) и `ArrowLeft`/`ArrowRight` между кнопками чипов (§7.2).
- У `aside.search-overlay` нет `aria-busy` во время запроса/поллинга (§7.1);
  атрибут не выставляется нигде (`:364-369`).

### P2-6. Три отступления в чистых функциях

- `computeIndexingRetryDelayMs` (`search.js:522-527`) при наличии заголовка с
  невалидным значением сразу возвращает `5000` и не смотрит `retry_after_ms`.
  Псевдокод §3.2 сначала проверяет заголовок, затем IPC-поле, затем дефолт;
  при `Retry-After: "0"` + `retry_after_ms: 2000` спека даёт 2000 мс, код — 5000.
- `searchDateBounds({ date: "2026-02-31", before: "2026-03-01" })`
  (`search.js:160-161`) возвращает `null` целиком, тогда как псевдокод §5.2
  пропускает невалидный `date` и применяет валидный `before`.
- `isAgeGatedSpacesRestricted` (`search.js:431-439`) при отсутствии конфига, но
  наличии `age_verification_status` подставляет маску `AGE_GATED_SPACES`.
  Формула §4.1 и `[D]` используют `Number(config?.age_gated_features ?? 0)` →
  `false`. Текущее поведение строже и не описано в отчёте; если это осознанная
  защита, её надо зафиксировать, иначе привести к формуле.

### P2-7. NSFW-контекст не подключён в продукте

`setNsfwContext` (`search-overlay.js:1746-1748`) ни разу не вызывается:
`grep -rn "setNsfwContext" rust/gui-spike/ui` находит только определение.
В `index.html`/`app.js` нет ни `data-nsfw`, ни `data-channel-id`, поэтому
`isChannelNsfw` в живом окне всегда `false`, и гейт не открывается. Фича
работает только в тестах, где контекст подаётся вручную. Это интеграционный
пробел: перед merge нужен вызов из `app.js` (контекст гильдии/пользователя/
каналов) либо отдельная карточка с явным «live unverified» в отчёте.

### P2-8. Evidence и покрытие

- `mutations.log` фиксирует мутантов и exit-коды, но не команды запуска (то же
  замечание, что в `REVIEW.md` AT-03). Для воспроизводимости нужна команда/раннер
  в отчёте или логе.
- Нет теста на CSS: ни один тест не проверяет, что каждый токен из `chat.css`
  объявлен в `tokens.css` (именно поэтому P1-1 прошёл зелёным). Нет теста на
  персистентность `GuildNSFWAgreeStore` (P1-3) и на текст `prompt`-описания.
- `T-M13-CHIPS-01` называется «синхронизация чипов фильтров», но контролы
  `.search-sections` не проверяет; синхронизация в одну сторону (P2-5) не
  ловится.

## 5. Что нужно для PASS

1. Заменить четыре несуществующих токена (P1-1) и перевести бейдж/кнопку на
   `--danger-fill` + `--text-on-danger` (P1-2).
2. Починить определение `localStorage` (P1-3) и добавить тест: запись согласия
   видна после повторного создания оверлея.
3. Привести строки к каталогу `ru-catalog.json` (P2-1) и селекторы чипов к
   §2.1/§6 либо зафиксировать отступление письменно (P2-2).
4. Закрыть поведенческие P2: очистка выдачи при гейте и триггеры
   `guild.nsfw_level`/текущего канала (P2-3), фокус и `refresh` после «Вернуться»
   (P2-4), синхронизация контролов/удаление кавычных токенов/`aria-busy` (P2-5).
5. Подключить `setNsfwContext` в `app.js` или вынести это в отдельную карточку с
   честным статусом (P2-7); добавить команды мутаций и тест CSS-токенов (P2-8).

После фикса: targeted `search-remote-real-index`, `test-policy`, `tsc`, затем
повторное ревью. P0 в текущем срезе нет: тесты не ослаблены, контракт не
подменён, gate-команды зелёные.
