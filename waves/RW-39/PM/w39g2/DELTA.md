# W39g / итерация 2 — DELTA: SPEC-P3-02 ↔ RE-EXTRA (D-14)

Дата: 2026-10-23. PM: DeepSeek. Источник: `RE-EXTRA.md` / `re-extra.json` (W39f, D-14),
`DECISIONS.md` D-13. Все изменения в спеке помечены `← RE-D14`.

## 0. Итог

| Метрика | Итерация 1 | Итерация 2 |
|---|---|---|
| строк-пунктов | 135 | **224** (211 компонентных + 13 ключевых чисел) |
| строк с `[A]` | 85 | 72 |
| строк **только** `[A]` | 73 | **45** (42 исходных + 3 новых по делу: CMP-20, SET-15, KEY-18) |
| закрыто `[A]` (стали `[M]/[D]/[L]/[T]/[F]` или получили факт) | — | **31** |
| кодов `[D]` | 0 | 131 |
| новых разделов | 10 | 13 (+§1.1, §9 эмодзи, §10 настройки, §11 клавиатура) |

Проверки: `rg -n "url\(|@font-face"` — пусто; token-check — `tokens ok 98`.

## 1. Композер (§4)

| id | было | стало | источник | W39h |
|:-|:-|:-|:-|:-|
| K-10 | 2000 `[A]`, «нужен контракт backend» | 2000 / Nitro 4000 `[D]` | C-04 | `features/chat.js`, `composer.css` |
| K-11 | — | порог счётчика: остаток ≤ 200 (`max/10`) | C-05 | `features/chat.js` |
| K-12 | — | вложений ≤ 10 | C-11 | `features/chat.js` |
| CMP-03 | padding `11px 16px` `[M]`/`[A]` | редактор: `1rem`/`1.375rem`, `padding-inline: 0 10px` `[D]` | C-14 | `composer.css` |
| CMP-04 | «max 5 строк» `[A]` | `max-height: 50vh` + скролл `[D]` (в спайке 56 фиксирован) | C-14 | `composer.css` |
| CMP-05 | «Написать сообщение…» `[A]` | шаблон «Написать {канал}» / «Сообщение для @name» `[D]` | C-02, C-03 | `chat.js` |
| CMP-06 | счётчик с 1800 `[A]` | остаток ≤ 200 + 3 текста лимита `[D]` | C-05, C-06 | `chat.js`, `composer.css` |
| CMP-07 | `aria-disabled` `[A]` | в референсе disabled-композер с текстом «У вас недостаточно прав…», без кнопок `[M]/[D]` | C-01 | `composer.css` |
| CMP-09 | attach + emoji + gift `[A]` | порядок **gift → gif → sticker → emoji → apps → submit**, gap `--space-8`, ≤ 6 `[D]` | C-09, C-16 | `composer.css`, `chat.js` |
| CMP-10 | 32×32 `[A]` | кнопка эмодзи radius 8, иконка 20, hover-токен `[D]` | C-13 | `composer.css` |
| CMP-11 | attach `[A]` | aria «Отправить файл», paste/drag, `Ctrl+Shift+U` `[D]` | C-10 | `chat.js` (disabled) |
| CMP-12 | Ctrl+E `[F]` | + слой пикера `inset-inline-start: 86` `[D]` | C-13 | `chat.js` |
| CMP-13 | 2000 `[A]` | 2000/4000, вставка > 4000 → «отправить файлом» `[D]` | C-04, C-12 | `chat.js` |
| CMP-16 | нет | `Enter` отправить / `Shift+Enter` перенос / `Ctrl+Enter` при disableEnterToSubmit / code-block `[D]` | C-07 | `chat.js` |
| CMP-17 | нет | `↑` в пустом поле — редактировать последнее `[D]` | C-08 | `chat.js` |
| CMP-18 | нет | вложения ≤ 10 + счётчик `[D]` | C-11 | `chat.js` |
| CMP-19 | нет | отдельная submit-кнопка существует, скрыта экспериментом `[D]` | C-15 | — (не рисуем) |
| CMP-20 | — | notice «Отправка недоступна» — наше решение, в дампе полосы нет `[A]` | C-17 | `composer.css` |

## 2. Чат-вью (§1, §1.1, §2, §3)

| id | было | стало | источник | W39h |
|:-|:-|:-|:-|:-|
| MSG-02 | гаттер 80 `[L]` | + `padding-inline-start: 5rem` `[D]` | V-13 | `chat.css` |
| MSG-03 | «отступы 11/11» `[A]` | compact: `padding-block: 2`, аватар `1em` inline (`top: .18em`), время в gutter `[D]` | V-13 | `chat.css` |
| MSG-04 | 48 `[L]` | + CSS `min-height: 44` (расчёт 48 сохраняется) `[D]` | V-12 | `chat.css` |
| MSG-06 | +22 `[L]` | + `padding-block: 2`, `min-height: 22` `[D]` | V-12 | `chat.css` |
| MSG-09 | `left: 16px` `[L]` | `inset-inline-start: 12px` (контент 64 = 12+40+12) `[D]` | V-12 | `chat.css`, `chat.js` |
| MSG-11 | имя `[M]` | + `h2.header` внутри `article` `[D]` (в спайке — без `article`, `[F]`) | V-10 | `chat.js` (частично) |
| MSG-12 | время `HH:mm` `[M]` | формат `DD.MM.YYYY, HH:mm` + `<i> — </i>` + скрытая полная дата `[D]` | V-11 | `chat.js` |
| MSG-14 | текст `[M]` | + `#message-content-<id>` `[D]` | V-09 | `chat.js` |
| MSG-27 | 15 пунктов `[M]` | + кнопки действий в отдельном контейнере `[D]` | V-14 | `actions.js` |
| MSG-32 | ARIA `[F]`/`[A]` | в дампе `role="article"` + `aria-labelledby` `[D]`; `[F]`-структура приоритетна → P3-03 | V-10 | — (вопрос в P3-03) |
| MSG-33 | нет | клик по аватару → профиль, `pointer-events` по классу `[D]` | V-15 | `chat.js`, `app.js` |
| MSG-34 | нет | `chat-messages-<channel>-<id>`; в дампе `search-result-<id>` `[D]/[A]` | V-16 | `chat.js` |
| MSG-35 | окно `[F]` | подтверждено `scroller`/`messageListItem` `[D]` | V-20 | — |
| VW-01 | 3 icon-btn (SPEC §7.4) | 4 icon-btn: ветки, колокол, закреплённые, участники `[M]` | V-01 | `chat.css`, `index.html` (новый id — через PM) |
| VW-02 | поиск 32 `[A]` | pill 32 + поле 244×28, placeholder «Искать «{guild}»» `[D]` | V-02, D-13 | `chat.css` |
| VW-03 | нет | топик канала в дампе отсутствует `[M]` | V-03 | — |
| VW-04 | линия unread `[A]` | `border-top: thin solid --border-subtle`, высота 0 `[D]` | V-04 | `chat.css` |
| VW-05 | плашка 32 `[M]` | 32 контент / 34 border-box, `--unread-banner-h`, radius `0 0 8px 8px`, pad-inline 16, top 0, z 3 `[D]` | V-05, D-13 | `chat.css`, `chat.js` |
| VW-06 | нет | «Перейти к последним»: `bottom: 28`, pad-inline 12, pad-bottom 8 `[D]` | V-06 | `chat.css`, `chat.js` |
| VW-07 | текст `[M]` | «Новые сообщения» / «Перейти к последним сообщениям» `[D]` | V-07 | `chat.js` |
| VW-08 | divider 12/16 `[A]` | 12/13 semibold, pad `2px 4px`, radius 8, caps 10 bold uppercase `[D]` | V-08 | `chat.css` |
| VW-09 | нет | DOM-цепочка cozy + `#message-accessories` `[D]` | V-09 | `chat.js` |
| VW-12 | 48/26 `[L]` | + CSS-минимумы 44/22, `header margin-inline-start: -72` `[D]` | V-12 | `chat.css` |
| VW-13 | compact `[A]` | метрики compact `[D]` | V-13 | `chat.css` |
| VW-17 | notice 24 `[A]` | блок 24 (D-13) + `padding: 6px 12px`, border-bottom `[D]` | V-17 | `chat.css` |
| VW-19 | нет | один компонент divider: дата / `isUnread` `[D]` | V-19 | `chat.js` |
| K-07 | 32 `[M]` | 34 border-box `--unread-banner-h` | V-05, D-13 | `chat.css` |
| EMB-11 | «сетка 2 колонки» `[A]` | + лимит 10 `[D]` | C-11 | `chat.js` |
| EMB-13 | нет | `#message-accessories-<id>` `[D]` | V-14 | `chat.js` |
| DIV-01 | 32 `[M]` | 34 border-box `--unread-banner-h` | V-05, D-13 | `chat.css` |
| DIV-02 | `--accent` | `--accent` (brand-500) `[D]` | V-05 | `chat.css` |
| DIV-03 | 14/18, pad 16 `[M]` | radius `0 0 8px 8px`, top 0, z 3, pad-inline 16 `[D]` | V-05 | `chat.css` |
| DIV-04 | «линия none» `[M]` | `border-top: thin solid`, высота 0, цвет `[A]` → `--danger` | V-04 | `chat.css` |
| DIV-07 | 12/16 `[A]` | 12/13 semibold, caps 10 bold uppercase `[D]` | V-08 | `chat.css` |
| DIV-09 | bg `--surface-2`, pad 16 `[A]` | `padding: 6px 12px`, border-bottom `--border-subtle` `[D]` | V-17 | `chat.css` |
| DIV-10 | нет | плашка «Перейти к последним» `[D]` | V-06 | `chat.css`, `chat.js` |
| DIV-11 | нет | общий компонент divider `[D]` | V-19 | `chat.js` |
| DIV-12 | нет | unread/bar пиксельно не подтверждены `[M]` | V-18 | тесты, не скриншот |

## 3. Участники (§5)

| id | было | стало | источник | W39h |
|:-|:-|:-|:-|:-|
| MEM-01 | 256 `[L]` | + `border-inline-start: 1px solid --border-subtle` `[D]` | M-10 | `members.css` |
| MEM-02 | 6 групп `[M]` | + в срезе 2 группы, поиска нет `[D]` | M-05, M-06 | — |
| MEM-03 | 35 строк `[M]` | 34 участника + заголовки, `search: []` `[D]` | M-06 | — |
| MEM-04 | «3 роли + 3 статуса» `[A]` | hoisted-роли по позиции → «В сети» → «Не в сети»; 4+1+1 `[D]` | M-01, M-02 | `members.js` |
| MEM-05 | блок 24 `[A]` | блок 42 `--row-member-group` (16 + 24 + 2) `[M]` | M-08, D-13 | `members.css`, `members.js` |
| MEM-06 | текст 12/16 `[A]/[M]` | «{label} — {count}» с NBSP, aria-hidden видимого текста, pad-inline 12/4 `[D]` | M-04, M-08 | `members.js` |
| MEM-07 | 42/4×8 `[L]` | + `margin-inline-start: 8`, `max-width: calc(100% - 16px)` `[D]` | M-07 | `members.css` |
| MEM-09 | аватар 32 `[L]` | подтверждено `[D]` | M-07 | — |
| MEM-10 | 16/20 `[M]` vs 14/18 `[A]` | **14/18**, статус 12/16 `[D]` | M-07 | `members.css` |
| MEM-12 | «Участники — N» `[L]` | count = размер секции (735 online), не всего сервера `[D]` | M-14 | `members.js` |
| MEM-13 | тулбар 48 `[A]` | только `#screen-members` (D-13) `[D]` | M-06, D-13 | `members.js` |
| MEM-15 | hover/selected `[A]` | + offline `opacity: .3`, hover → 1 `[D]` | M-09 | `members.css` |
| MEM-18 | aria `[A]` | + aria-hidden у видимого текста заголовка `[D]` | M-04 | `members.js` |
| MEM-19 | нет | пустые секции не рендерятся `[D]` | M-13 | `members.js` |
| MEM-20 | нет | иконка роли 4 px, wheel у бустеров `[D]` | M-08, M-12 | `members.js`, `members.css` |
| MEM-21 | нет | фон/бордер панели, список `padding: 0 0 10px` `[D]` | M-10 | `members.css` |
| MEM-22 | нет | сортировка по `(nick ?? globalName).toLowerCase()` `[D]` | M-03 | `members.js` |

## 4. Эмодзи / настройки / клавиатура (§9–§11)

| id | было | стало | источник | W39h |
|:-|:-|:-|:-|:-|
| EMO-04 | 10 категорий `[F]` без порядка | порядок: top guild · recent · сервер · people · nature · food · activity · travel · objects · symbols `[D]` | E-04 | `emoji.js` |
| EMO-05 | aria = локализованное имя | aria = англ. id, локализация в `title` `[D]` (у нас оставлен локализованный aria для AT) | E-05 | `emoji.js` |
| EMO-07 | глифы `[A]` | 6 подтверждённых: Clock, Trophy, GameController, Heart, Flag, Star `[D]` | E-07 | `emoji.js` |
| EMO-09 | 9 колонок `[M]` | `rowSize=9` `[D]` — подтверждено | E-09 | — |
| EMO-11 | фильтр по подстроке `[F]` | нормализация: `:`/пробелы/`_`, lower, regex-escape, без регистра `[D]` | E-11 | `emoji.js` |
| EMO-12 | нет | unlocked → locked, frecency, затем имя `[D]` | E-12 | `emoji.js` |
| EMO-13 | «недавние» `[A]` | frecency; для реакций отдельный список `[D]` | E-13 | `emoji.js` |
| EMO-15 | заголовки секций `[A]` | «Лучшие эмодзи сервера», «Часто используемые», сворачиваемые `[D]` | E-15 | `emoji.js` |
| EMO-16 | поиск по текущей категории (SPEC §9.2) | поиск глобальный, категория — только листание `[D]` | E-16 | `emoji.js`, `SPEC.md` (PM) |
| SET-01 | 6 разделов `[A]` | **18** + «Выйти», порядок зафиксирован `[D]` | S-01 | `settings.js`, `settings.css` |
| SET-02 | нет | 4 подпункта «Учётной записи» `[D]` | S-02 | `settings.js` |
| SET-03 | нет | поиск «Поиск» `[D]` | S-03 | `settings.js` |
| SET-04 | 3 группы `[F]` | 10 блоков «Голоса» в порядке дампа `[D]` | S-04 | `settings.js` |
| SET-05 | 38 тумблеров `[F]` | 38 = 12 + 22 (4 + 18) + 4 `[D]` | S-05 | `settings.js` |
| SET-06 | состояния `[F]` все checked | 12 функциональных со снятыми состояниями из дампа `[D]` | S-06 | `settings.js` |
| SET-07 | нет | 22 звуковых, все вкл, 18 скрыты `[D]` | S-07 | `settings.js` |
| SET-08 | нет | 4 расширенных (OpenH264/H.264-оверлей/диагностика/лог) `[D]` | S-08 | `settings.js` |
| SET-09 | нет | 4 селекта с значениями `[D]` | S-09 | `settings.js` |
| SET-10 | нет | 4 ползунка (100/100/50/26.11, подписи %) `[D]` | S-10 | `settings.js` |
| SET-11 | нет | радиогруппа «Профиль ввода»: 3 `[D]` | S-11 | `settings.js` |
| SET-12 | нет | 6 кнопок вкладки `[D]` | S-12 | `settings.js` |
| SET-13 | нет | «Связанные настройки» `[D]` | S-13 | `settings.js` |
| SET-14 | нет | «Внешний вид»: Компактное/Уютное → `data-density` `[D]` | S-14 | `settings.js`, `chat.js` |
| SET-15 | — | «Уведомления» не открывались — `[A]` | S-15 | — |
| KEY-01 | Ctrl+K `[A]` | `Ctrl+K` — QuickSwitcher `[D]` | K-01 | `app.js`/`keys.js` |
| KEY-02 | Ctrl+E `[F]` | `Ctrl+E/G/S/P/I` — пикер/GIF/стикеры/пины/inbox `[D]` | K-02 | `app.js` |
| KEY-03 | нет | `Ctrl+U` участники, `Ctrl+F`/`Ctrl+Shift+F` поиск `[D]` | K-03 | `app.js` |
| KEY-04 | Esc `[A]` | `Esc` закрывает слой/пикер/модалку, в сообщении — фокус в поле `[D]` | K-04 | `app.js` |
| KEY-05 | нет | `Tab` и любой символ вне полей → фокус в композер `[D]` | K-05 | `app.js` |
| KEY-06 | Enter `[F]` | `Enter` = отправка (в спайке выключена) `[D]` | K-06 | `chat.js` |
| KEY-07 | нет | `↑` — последнее сообщение `[D]` | K-07 | `chat.js` |
| KEY-09 | PageUp/Down `[F]` | + `Shift+PageUp` к непрочитанному, `Shift+Esc` пометить прочитанным `[D]` | K-09 | `chat.js` |
| KEY-11 | нет | `Ctrl+Shift+U` attach `[D]` | K-11 | `chat.js` |
| KEY-14 | нет | `E/Backspace/P/+/R/F/S/Ctrl+C/Alt+Enter` для сообщения в фокусе `[D]` | K-14 | `actions.js` |
| KEY-15 | нет | диалог шорткатов: 6 групп `[D]` | K-15 | — (вне Phase 2) |
| KEY-16 | нет | `Shift+F10` контекстное меню `[D]` | K-16 | `actions.js` |
| KEY-18 | — | Tab-обход/focus-trap не подтверждены — `[A]` | K-18 | — |

## 5. Прочее

| id | было | стало | источник | W39h |
|:-|:-|:-|:-|:-|
| SRCH-14 | элемент `[A]` | + `.buttonsContainer` и кнопка «Перейти» `[D]` | V-14 | `search.js` |
| SRCH-16 | переход `[A]` | + id `search-result-<id>` `[D]`, длительность `[A]` | V-14, V-16 | `search.js` |
| SRCH-18 | «32 vs 244×28» | pill 32 + поле 244×28 — оба валидны (D-13) `[D]` | V-02 | `chat.css` |
| SRCH-19 | нет | `id="search-result-<id>"`, `data-list-item-id` `[D]` | V-16 | `search.js` |
| QS-03 | Ctrl+K `[A]` | `Ctrl+K` `[D]` | K-01 | `app.js` |
| QS-08 | клавиши `[A]` | + `Esc` `[D]`, остальное `[A]` | K-04 | `app.js` |
| PROF-01 | 268.8×408 (уже `[M]`) | подтверждено D-13 | D-13 | — |
| PROF-10 | открытие по строке `[A]` | + клик по аватару сообщения `[D]` | V-15 | `app.js` |
| PROF-11 | фокус-ловушка `[A]` | `Esc` `[D]`, ловушка `[A]` | K-04, K-18 | `app.js` |

## 6. Осталось `[A]` (45 строк) — что это и почему

- **Меню сообщения**: MSG-28/29/30 (геометрия hover-панели — RE G-01), G-01/G-02 (тексты 15 пунктов не в scope).
- **Реакции**: MSG-23…26 — RE-EXTRA реакций не даёт (нет раздела).
- **Reply-quote**: MSG-19…22 — в дампе нет (RE G-07).
- **Embeds**: EMB-01, 03…08, 10, 12 — геометрия карточек не снята (RE G-01).
- **Группировка**: MSG-31 — RE даёт только DOM, не правило времени/автора.
- **Selected/mention**: MSG-17/18 — в дампе не наблюдались.
- **Divider-блок**: DIV-05/06 (общая высота), DIV-08 (notice 24 — наше решение, D-13).
- **Композер**: CMP-08 (ошибка, вне Phase 2), CMP-14 (focus-ring), CMP-20 (notice — решение спайка).
- **Поиск**: SRCH-01/02/03/06/09/12 — геометрия панели и контролов (origin кропа не логировался).
- **QuickSwitcher**: QS-05/07/09 — геометрия (RE G-01).
- **Профиль**: PROF-02/04/06/07/09 — внутренняя геометрия попаута (в R-06 только bbox).
- **Новые по делу**: SET-15 («Уведомления» не открывались), KEY-18 (Tab/focus-trap не подтверждены).

Итого закрыто **31** `[A]`-строки, добавлено **89** новых фактов (`[D]`/`[M]`), остаток — 42
исходных + 3 новых; каждый остаток имеет запись в §13 спеки.

## 7. Файлы

| Файл | Действие | md5 |
|:-|:-|:-|
| `waves/RW-39/DESIGN/SPEC-P3-02.md` | перезаписан (итерация 2) | `77eb3515e8a4de025d9be2c523a97b3a` |
| `waves/RW-39/PM/w39g2/DELTA.md` | создан | — |

Запросы к W39h: см. колонку «W39h»; правки `ui/index.html` (4-я иконка шапки, новые id)
только через PM — `DOM-CONTRACT` менять нельзя без решения.
