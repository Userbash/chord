# RW-40 / G3-DESIGN-MSG — сообщения: реакции, вложения, меню, треды и пины

Статус: дизайн-контракт. Дата: 2026-10-24. Владелец: Design.
Вход: `waves/RW-39/DESIGN/SPEC-P3-02.md`, `tokens.css`, `DOM-CONTRACT.md`,
`reference-oracle/ui/01-msg-menu-items.json`, `ui/00-app.png`, `ui8/*`,
`waves/RW-39/NET/LOGIN-SPEC.md`.
Ассеты RE не копируются: используются только наблюдения, тексты и размеры.

## 0. Легенда и правила

| Метка | Значение | Источник |
|-|-|-|
| `[M]` | измерено на пикселях | `reference-oracle/ui/00-app.png`, `reference-oracle/ui8/*` |
| `[T]` | токен | `waves/RW-39/DESIGN/tokens.css` |
| `[D]` | факт DOM/CSS/JSON дампа | `reference-oracle/ui/01-msg-menu-items.json`, `ui8/*`, `SPEC-P3-02.md` |
| `[A]` | решение или пробел контракта | это документ; требуется подтверждение до реализации |

Каждое числовое значение ниже имеет метку и источник. Если значение выражено токеном,
метка относится к самому токену; литерал в скобках повторяет значение токена для ревью.
Коды `[A]` не маскируют отсутствие замера. Все цвета, интервалы, радиусы, размеры текста,
слоёв и controls берутся из `tokens.css`; отсутствующий токен — gap `[A]`.

Общий DOM: сообщение — `article[role="article"]`, контент —
`#message-content-<id>`, аксессуары — `#message-accessories-<id>`, строка имеет
`data-message-id`, `data-state` и `aria-labelledby` по `DOM-CONTRACT` §7/§8.
Действия делегируются через `data-action`; фокус всегда возвращается на сообщение или
открывший control. Состояния ошибок показываются текстом `--danger-text` и не теряются.

## 1. Reactions

### 1.1 Вид и взаимодействие

| Элемент | Контракт | Метка и источник |
|-|-|-|
| Hover-панель | При hover/focus строки появляется action-toolbar над правым краем сообщения; кнопки: добавить реакцию, ответить, меню. Hover-фон строки `--row-hover-bg`; слой `--z-popover`. | `[M]` `tokens.css` `--row-hover-bg`; `[T]` `--z-popover`; расположение `[A]` gap, RE геометрия панели не снята |
| Picker | Используется существующий expression picker: ширина `--emoji-picker-w` (498), высота `--emoji-picker-h` (440), rail `--emoji-rail-w` (48), ячейка `--emoji-cell` (44), колонок `--emoji-cols` (9), gap `--emoji-gap` (4). Поиск — `#emoji-search`, закрытие Escape/наружным кликом. | `[T]`/`[M]` `tokens.css` и `ui8/21*-emoji*.json` |
| Пилюля | Ряд в `#message-accessories-<id>`; emoji 16, счётчик 12, высота 24, padding по `--space-4`; перенос разрешён. Высота 24 и emoji 16 — gap, пока нет RE-замера. | `[A]` `SPEC-P3-02.md` MSG-23/26; `[T]` `--font-size-small`, `--space-4` |
| Добавление | Выбор emoji создаёт optimistic reaction; повторный клик своей пилюли снимает её. Событие: `{messageId, emoji, op:add|remove}`. | `[A]` поведение C-06; payload gap |
| Своя реакция | `data-selected="true"`, `aria-pressed="true"`, фон `--accent-soft`, текст `--text-accent`; счётчик включает пользователя. | `[T]` токены; `[A]` атрибуты |
| Чужая реакция | `data-selected="false"`, `aria-pressed="false"`, фон `--surface-3`, текст `--text`; hover `--state-hover`. | `[T]` токены; `[A]` выбор surface |
| Счётчик | Показывает целое число участников; `0` не рисуется. При загрузке — disabled skeleton/aria-busy; после ответа — обычное состояние. | `[A]` gap, формат числа |

### 1.2 Состояния и ошибки

`idle` — пилюля доступна; `hover` — `--state-hover`; `selected` — `--accent-soft`;
`pending` — `aria-busy="true"`, повторный клик заблокирован; `failed` — пилюля
возвращается к предыдущему счётчику, рядом live-region с «Не удалось изменить реакцию» и
кнопкой повтора. Ошибка сети, отсутствие прав и удалённое сообщение различаются в payload,
но пользователь видит понятный текст и может повторить действие. При offline control
disabled через `--state-disabled-opacity`.

## 2. Attachments

### 2.1 Вход и dropzone

`[data-action="attach"]` — кнопка композера с `aria-label="Прикрепить файл"`; доступен
клик, paste и drag-and-drop. Drag-over добавляет `data-drop="active"` на dropzone и фон
`--accent-soft`; dropzone — область композера, точная геометрия `[A] gap`.
Размер кнопки — `--btn-height` (32), иконка — `--icon-size` (20), радиус — `--btn-radius`;
значения `[T]` из `tokens.css`.

### 2.2 Очередь, превью и отмена

| Состояние | Контракт | Метка и источник |
|-|-|-|
| queued | После выбора появляется чип/карточка в accessories; имя обрезается ellipsis, размер показывается `--font-size-small`. | `[T]` токены; `[A]` расположение/формат |
| uploading | Карточка содержит progressbar `0…100%`, `aria-valuenow`, отмену и текст ошибки; прогресс обновляется monotonic. | `[A]` диапазон/геометрия; `[T]` `--font-size-small` |
| preview | Изображение/видео — локальное превью с `--radius-sm`; прочие файлы — иконка `--icon-size-lg` (24) и имя. Высота/ширина preview `[A]` gap. | `[T]` токены; `[A]` размеры без RE-замера |
| cancel | Отмена удаляет только pending item, отзывает object URL и оставляет composer usable; повторная отправка не дублирует файл. | `[A]` жизненный цикл |
| complete | Карточка переносится в `#message-accessories-<id>` после успешной отправки; ссылка/скачивание — отдельное действие. | `[D]` RE V-14; `[A]` переход |
| failed | Сохраняется имя, показывается `--danger-text`, причина и «Повторить»/«Отменить»; ошибка не замалчивается. | `[T]` `--danger-text`; `[A]` copy |

Лимит вложений на сообщение — `≤ 10` `[D]`, RE C-11 / K-12. Лимит размера файла,
совокупный байтовый лимит, допустимые MIME-типы и retry budget в доступных входах не найдены:
это `[A]` gap и не должны быть захардкожены в UI до решения Backend/Network. При превышении
известного лимита сообщение должно назвать лимит; неизвестный лимит даёт «Сервер отклонил файл».

## 3. Message menu

Источник состава — `reference-oracle/ui/01-msg-menu-items.json`: **15 пунктов** `[D]`.
Первые 4 — быстрые реакции; остальные 11 — команды. Контейнер использует `--menu-min-w`
(188), `--menu-pad` (8), `--menu-border` (1), `--menu-radius`, `--shadow-popover`, строки
`--row-menu-item` (32); все значения `[T]`, кроме ширины/строки, которые в tokens.css
помечены `[A]`. Порядок фиксирован источником.

| # | Пункт | Disabled | Действие/подтверждение | Источник |
|-|-|-|-|-|
| 1 | `:thumbsup: :+1: :thumbup: :thumbs_up:` | нет | add/remove reaction | `[D]` menu JSON |
| 2 | `:fire: :flame:` | нет | add/remove reaction | `[D]` menu JSON |
| 3 | `:regional_indicator_f:` | нет | add/remove reaction | `[D]` menu JSON |
| 4 | `:Linux:` | нет | add/remove reaction | `[D]` menu JSON |
| 5 | Добавить реакцию | по capability | открыть picker | `[D]` menu JSON; `[A]` rule |
| 6 | Переслать | по capability | выбрать канал/получателей, confirm перед отправкой | `[D]` menu JSON; `[A]` confirm |
| 7 | Скопировать текст | нет | clipboard, toast success/error | `[D]` menu JSON |
| 8 | Добавить в закладки | по capability | toggle, optimistic rollback при ошибке | `[D]` menu JSON; `[A]` rule |
| 9 | Создать напоминание | по capability | открыть form; confirm submit | `[D]` menu JSON; `[A]` confirm |
| 10 | Приложения | по capability | открыть app submenu; пустое состояние допустимо | `[D]` menu JSON; `[A]` submenu |
| 11 | Отметить как непрочитанное | нет | mark from message onward, close menu | `[D]` menu JSON |
| 12 | Скопировать ссылку на сообщение | нет | clipboard, toast success/error | `[D]` menu JSON |
| 13 | Зачитать сообщение | по capability | invoke OS/browser speech; error toast | `[D]` menu JSON; `[A]` capability |
| 14 | Пожаловаться на сообщение | по capability | modal with reason; confirm destructive submit | `[D]` menu JSON; `[A]` confirm |
| 15 | Копировать ID сообщения | по capability | clipboard, toast success/error | `[D]` menu JSON |

Дополнительные contextual-команды «Ответить», «Править», «Удалить» не добавляются в эти
15 пунктов: они доступны в action-toolbar/inline flow. «Править» показывается только автору
с правом `MANAGE_MESSAGES` или собственной записи; «Удалить» — при праве удаления; обе
команды disabled, если сообщение уже удалено или запрос в flight. Inline-edit заменяет тело
на textarea, сохраняет `--font-size-body`/`--line-height-body`, Enter сохраняет, Escape
отменяет, отмена восстанавливает исходный текст. Удаление и жалоба требуют confirm; отмена
закрывает confirm без побочного эффекта. Keyboard navigation — roving focus, Escape закрывает
submenu/menu и возвращает фокус на trigger.

## 4. Threads и pins

### 4.1 Панель треда

Reply/«Открыть тред» открывает sibling-панель `#thread-panel` рядом с chat list;
`role="complementary"`, `aria-label="Тред сообщения"`, заголовок содержит исходное
сообщение и кнопку закрытия. Каждая reply — та же структура `article`/accessories, ссылка
на parent имеет `data-action="jump-parent"`. Панель имеет ширину
`--layout-member-panel-w` (240) `[A]`/`[T]` на обычном shell и
`--layout-member-panel-w-wide` (256) `[T]` входного контракта; для RW-40 это reuse токена,
а не новый размер. Фактический RE-замер thread panel отсутствует, поэтому выбор `[A]`.

Состояния: `closed`, `loading` (`aria-busy`), `ready`, `empty`, `error`, `offline`.
Переход по reply подсвечивает parent через `--accent-soft` и скроллит его в виртуализованное
окно; если parent вне окна, PageStore загружает его перед фокусом. Отправка reply использует
composer-контракт и закрывает panel только по явному действию.

### 4.2 Pins

Кнопка `data-action="pins"` в header открывает `#pins-panel` с `role="dialog"` или
`complementary` в зависимости от viewport; список pinned messages использует те же article
и accessories. Переход «Перейти к сообщению» закрывает panel, выбирает канал и прокручивает
к `messageId`; отсутствующий/удалённый pin показывает empty/error с retry. Pin/unpin требует
права, optimistic state и rollback по ошибке. Счётчик pins, pagination, размер panel и
лимит pinned messages не подтверждены входами: `[A]` gap, сервер является источником истины.

### 4.3 Общие accessibility и ошибки

Каждая panel имеет явный label, focus trap только для modal, Escape close и live-region для
loading/error. Сетевой отказ сохраняет последнюю подтверждённую выборку и показывает retry;
неподтверждённое действие не выдаётся за успешное. Ассеты и web-шрифты не добавляются.

## 5. Gap-реестр

| Gap | Что отсутствует | Владелец решения |
|-|-|-|
| G-01 | геометрия reaction toolbar/pill и attachment preview | Design + Frontend |
| G-02 | file byte/MIME/server limits и retry budget | Backend + Network |
| G-03 | thread/pins RE-геометрия, pagination и pin quota | Design + Network |
| G-04 | capability mapping для menu пунктов | Frontend + Backend |

Проверка документа должна найти отсутствие внешних URL и web-шрифтов; все custom properties сверяются
с `waves/RW-39/DESIGN/tokens.css`.
