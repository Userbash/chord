# M-08 — markdown/упоминания сообщений: отчёт G2 Frontend Lane C

Дата: 2026-10-30. Воркер: G2 Frontend Lane C (RW-40). Карточка: M-08
(`waves/RW-40/PARITY/PARITY-MATRIX.md`, строка M-08: «нет markdown/mentions/кастом-emoji →
рендер по `I162`, безопасные ссылки»).

## 1. Статус

**done (frontend, offline).** Новый модуль `rust/gui-spike/ui/features/markdown.js`:
разбор и рендер сообщений Discord — код, форматирование, спойлеры, цитаты, упоминания —
с P0-защитой от XSS. Тесты — новый describe в `tests/unit/gui-spike-model.test.mjs`
(15 кейсов), существующие 34 кейса файла не изменены (диф: +270 / −0). `app.js`, `chat.js`,
`actions.js`, `composer.js`, `members.js`, `search.js`, `index.html` не тронуты: интеграция
рендера в строку чата — отдельный integration-патч Team Lead.

## 2. Сделано

| Пункт ТЗ | Где | Проверка (тест) |
|---|---|---|
| `parseMessageMarkdown(text, context)` → токены без DOM | `parseMessageMarkdown`, `parseBlocks`, `parseInline` | `parseMessageMarkdown: форматы становятся токенами…` |
| `renderMessageMarkdown(container, text, context)` → DOM через `container.ownerDocument` | `renderMessageMarkdown`, `renderToken` | все рендер-кейсы |
| XSS: сырой HTML — только текст (`<script>`, `<img onerror>`) | `renderToken` (`textContent`/`createTextNode`), нет HTML-парсинга строк | `сырой HTML остаётся текстом…` |
| Ссылки только `https://`/`http://`; `javascript:`, `data:`, `vbscript:`, `file:`, `//host` — текст | `safeHref` (`new URL` + белый список схем) | `опасные схемы не становятся ссылкой` |
| `rel="noopener noreferrer"`, `target="_blank"` | `renderToken` → `case "link"` | `ссылки: http(s) получают rel=noopener noreferrer…` |
| Блок кода ```lang, инлайн-код; внутри markdown/упоминания не парсятся | `readCodeSpan`, `parseBlocks` (FENCE_RE) | `код: markdown и упоминания внутри не парсятся` |
| `**жирный**`, `*курсив*`, `~~зачёркнуто~~`, `\|\|спойлер\|\|` | `readMark`, `MARKERS` | `жирный/курсив/зачёркнутый рендерятся элементами`, `спойлер…` |
| Спойлер: `data-spoiler="hidden"` → `"revealed"` по клику/Enter/Space, `tabindex="0"` | `configureSpoiler`, `revealSpoiler` | `спойлер: data-spoiler=hidden, клик/Enter/Space раскрывают` |
| Цитата `> quote` → `<blockquote>` | `parseBlocks` (QUOTE_RE) | `цитата > quote → blockquote…` |
| `<@123>`/`<@!123>`, `<#456>`, `<@&789>`, `@everyone`, `@here` | `readMention`, `readBroadcast`, `mentionNode` | `упоминания…`, `@everyone и @here — только целым словом` |
| Fallback `@пользователь` / `#канал` / `@роль`; битые id — текст | `mentionLabel`, `MENTION_RE` (только цифры, ≤32) | `неизвестные id → fallback…` |
| Данные `context` — недоверенные (только текст) | `mentionNode` → `textContent` | `данные context не инжектят HTML` |

Формат токенов: `text`, `code`, `code_block{language}`, `bold|italic|strikethrough|spoiler`
(`children`), `quote` (`children`), `link{href, children}`, `user_mention|channel_mention|
role_mention{id, label}`, `everyone|here{label}`. Предел вложенности форматирования — 8
(защита от патологического ввода).

## 3. Red → green

- RED: `node --test tests/unit/gui-spike-model.test.mjs` → **exit 1**, `tests 48 / pass 34 /
  fail 14` (`waves/RW-40/PM/m08-markdown/red.log`). 15 новых кейсов, 14 падают с
  `Error: M-08: … не реализован`; пятнадцатый — статическая проверка исходника на
  `innerHTML`/`insertAdjacentHTML` — в red-фазе проходит тривиально, потому что заглушка
  этих API не содержит (проверка «красной» быть не может по смыслу, она стережёт green-код).
- GREEN: та же команда → **exit 0**, `tests 48 / pass 48 / fail 0`
  (`waves/RW-40/PM/m08-markdown/green.log`).
- Первый green-прогон вскрыл реальный дефект: граница `@everyone` использовала ASCII `\w`,
  поэтому `почта@everyone.com` распознавалось как упоминание. Исправлено на Unicode-класс
  `[\p{L}\p{N}_@#]`; тест не менялся.

## 4. Targeted

`node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-features.test.mjs
tests/meta/test-policy.test.mjs` → **exit 0**, `tests 165 / pass 165 / fail 0`
(`waves/RW-40/PM/m08-markdown/targeted.log`).

## 5. Oracle

Независимые источники, а не текущий код: правила markdown/упоминаний Discord-клиента
(`PARITY-MATRIX` M-08, ссылка `I162`) и требования карточки (список схем, атрибуты ссылки,
`data-*-id`, `data-spoiler`). DOM-оракул — шим `tests/support/gui-dom-harness.mjs`
(`ShimDocument` создаётся напрямую, `app.js` не импортируется). Проверки формулируются по
наблюдаемому DOM (элементы, атрибуты, текст), а не по внутренним функциям модуля.

## 6. Мутации

5 мутаций, все **CAUGHT** (`waves/RW-40/PM/m08-markdown/mutations.log`):

| Мутация | Инвариант | Убивший тест |
|---|---|---|
| M1-UNSAFE-SCHEME (`safeHref` отдаёт любой протокол) | ссылки только http(s) | `опасные схемы не становятся ссылкой` |
| M2-REL-NOOPENER (`rel` без `noopener`) | защита от tabnabbing | `ссылки: http(s) получают rel=noopener noreferrer…` |
| M3-SPOILER-ANY-KEY (раскрытие любой клавишей) | активация только Enter/Space | `спойлер: data-spoiler=hidden, клик/Enter/Space раскрывают` |
| M4-CODE-AS-MARKUP (инлайн-код не распознаётся) | код не парсит markdown/упоминания | `parseMessageMarkdown…`, `код: markdown и упоминания внутри не парсятся` |
| M5-MENTION-ID-ANY (id любой длины/символов) | id упоминания — только цифры | `упоминания: пользователи, каналы, роли…`, `неизвестные id → fallback…`, `данные context не инжектят HTML` |

Baseline/restore: базовый md5 `85f1cbd92bec6e9d5fc157bd2e8fdeb8`; после каждой мутации файл
восстановлен из копии, md5 совпал с базовым (см. `mutations.log`). Финальный green-прогон
после восстановления — `tests 48 / pass 48`.

## 7. Генерация / стабильность

Не применимо: генераторов нет, парсер синхронный и детерминированный, гонок и общих
ресурсов нет. Повторные прогоны для флейков не требовались.

## 8. Typecheck

- `npx tsc -p tsconfig.json`: **0 ошибок в моих файлах** (`markdown.js`,
  `gui-spike-model.test.mjs`), `@ts-ignore` в них — 0.
- В рабочем дереве остаются 2 чужие ошибки: `tests/unit/gui-spike-dom.test.mjs(1014,24)` и
  `(1014,29)` — `Parameter 'cmd'/'args' implicitly has an 'any' type` в блоке `P-03-UI`
  (`typing-indicator`), который правит другая лейна. Файл вне моей файловой матрицы, поэтому
  не правился; из-за него `npm run typecheck` в batch сейчас красный.
- `npx tsc -p tsconfig.agent.json` → exit 0.

## 9. Гейт

Полный `npm run gate:worker` не запускался: по AGENTS §3 в batch воркер прогоняет только
targeted-тесты своего модуля, полный гейт делает integration-лейн. Targeted (165 тестов) —
зелёный. Для приёмки нужно, чтобы integration-лейн закрыл ошибки `P-03-UI` в
`gui-spike-dom.test.mjs` и прогнал `gate:worker`.

## 10. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/markdown.js` (новый) | `85f1cbd92bec6e9d5fc157bd2e8fdeb8` |
| `tests/unit/gui-spike-model.test.mjs` (+270 / −0, новый describe в конце) | `e975a4d45da485beebcefb0319db950f` |
| `waves/RW-40/PM/m08-markdown/red.log` | `ad44b2c7d77edfb2e361f697603c09b4` |
| `waves/RW-40/PM/m08-markdown/green.log` | `fc3afccecd7ccf6e7033cec838b2bb21` |
| `waves/RW-40/PM/m08-markdown/targeted.log` | `6a1d4bf0470d7fd68a7ad1ce9e9ad864` |
| `waves/RW-40/PM/m08-markdown/mutations.log` | `9ab23dc8c096bc8335e32dc716675acb` |

## 11. Риски и вопросы

1. **Не подключено к строке чата.** `chat.js`/`app.js` не менялись по ТЗ; пока M-08 — библиотека
   с тестами. Нужен integration-патч: `renderMessageMarkdown` в `renderChatRow`, словари
   `context.users/channels/roles` из демо-данных и раскладка `context` в `messages`-модели.
2. **Стилей нет.** Классы `md-code`, `md-code-block`, `md-spoiler`, `mention--user|channel|role`
   не описаны в CSS — визуально сообщение отрендерится невыразительно. CSS вне моей матрицы.
3. **Нет автолинка bare-URL.** Контракт M-08 требует markdown-ссылки `[текст](url)`; голая
   `https://…` остаётся текстом. Если оракул Discord (`I162`) требует автолинк — отдельная
   карточка с тестом.
4. **Одностороннее раскрытие спойлера** (обратно не скрывается) — как в Discord; если нужен
   toggle, это изменение контракта и отдельный тест.
5. **Вложенность форматирования ограничена 8** и «незакрытый маркер = литерал» — сознательные
   упрощения; на патологических входах деградация в текст, без исключений.
6. **`npm run typecheck` красный из-за чужого файла** (`P-03-UI`, `gui-spike-dom.test.mjs:1014`).

## 12. Следующий шаг

Team Lead: приёмка карточки M-08, затем integration-патч (п. 11.1) и CSS для `md-*`/`mention`.
