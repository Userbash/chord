# RE-EXTRA — недостающие UI-контракты для P3-02 (чат-вью)

- Карточка: **W39f** (RE-воркер), волна RW-39. Дата: 2026-10-23.
- Источник: дамп `/home/sanya/re-deep` (**read-only**, ассеты не копировались). Бинарь/рантайм через REA не
  требовался — все факты сняты чтением JSON/HTML/PNG/JS/CSS дампа (метод указан у каждого факта).
- Назначение: дополнить `docs/re/frontend-d03.md` и `DESIGN/SPEC.md` там, где для P3-02 не хватало
  контрактов: композер, панель участников, эмодзи-пикер, настройки, клавиатура/фокус, чат-вью.
- Машиночитаемая копия: `waves/RW-39/DESIGN/re-extra.json` (те же id).
- Геометрия shell не дублируется: числа рамок/панелей — в `DESIGN/layout.json`, здесь только то, чего там нет.

## 0. Коды и методы

| Код | Значение |
|---|---|
| `[D]` | факт из дампа (JSON/HTML/JS/CSS/каталог i18n) |
| `[M]` | измерено/увидено на lossless PNG дампа (кроп во `/tmp`, в репозиторий не копировался) |
| `[T]` | токен из `ui/tokens.json` |
| `[A]` | наше допущение (в дампе прямого подтверждения нет) |

Обозначения методов: `json` — разбор JSON дампа; `css` — правило из CSS-чанка в `decoded/`;
`js` — модуль веб-бандла (`decoded/<hash>_0`, номер модуля в скобках); `i18n` — `ru-catalog.json` /
`en-catalog.json` (ключ → AST, текст собран склейкой строк); `png` — визуальный осмотр кропа lossless PNG.

## 1. Композер

| id | значение/правило | код | источник | метод |
|---|---|---|---|---|
| C-01 | В референсе композер **disabled**: вместо поля — текст «У вас недостаточно прав, чтобы отправлять сообщения на этом канале.», кнопок вложения/эмодзи/подарка нет (не хватает права `SEND_MESSAGES`) | [M] | `ui/00-app.png` (кроп `y 1020…1106`, `x 370…1800` CSS) | png |
| C-02 | Подпись/плейсхолдер композера — шаблон «Написать {канал}» (en «Message {channel}»); в дампе `aria-label="Написать #🔴┃поговорить-тут"` | [D] | `ui8/20a-searchbar.json` (`inputs[2].aria`); `ru-catalog.json` ключ `8lzR/R` | json + i18n |
| C-03 | Вариант для ЛС: «Message @name» / «Сообщение для @name» | [D] | `en-catalog.json` / `ru-catalog.json`, ключ `0ZQw/X` | i18n |
| C-04 | Лимит сообщения **2000** символов; с Nitro — **4000** | [D] | `decoded/3eefa7dae1e2c234_0`, модуль constants: `nY=2e3` (export `uvi`), `nK=4e3` (export `CS1`) | js |
| C-05 | Счётчик символов появляется, когда осталось ≤ `max/10` = **200** (для 2000); порог переопределяется `showRemainingCharsAfterCount` | [D] | `decoded/3eefa…`, модуль `263582` (`f=h>(d??o??u/10)`) | js |
| C-06 | Тексты счётчика: «Осталось символов: {count}», «Достигнут лимит длины сообщения», «Сообщение слишком длинное», «Достигнуто макс. количество символов»; апселл Nitro: «Благодаря подписке Nitro количество символов в сообщении увеличено до {maxLength}!» | [D] | `ru-catalog.json` ключи `qH8uFW`, `tU6YQ7`, `YSRIqa`, `c2Jqed`, `vcvHa0` | i18n |
| C-07 | Отправка: `Enter` — отправить; `Shift+Enter` — перенос строки; `Ctrl+Enter` отправляет и при выключенном enter-to-submit; `Enter` игнорируется внутри незакрытого блока кода | [D] | `decoded/f0a78ad5dbc04c2f_0`, `handleKeyPress`/`handleKeyDown` (~смещение 53411) | js |
| C-08 | `↑` в пустом редакторе — редактировать последнее сообщение (`FOCUS_MESSAGES {atEnd:true}`) | [D] | `decoded/f0a78…` (~124134) | js |
| C-09 | Порядок кнопок: слева от поля — **attach**; справа — ряд `buttons__74017` (gap `--space-8`), порядок добавления: **gift → gif → sticker → emoji (или совмещённая expression) → apps → submit** | [D] | `decoded/f0a78…` (JSX композера), `decoded/3eefa…` модуль `846558` | js |
| C-10 | Attach-кнопка: aria «Отправить файл»; файлы также вставляются из буфера и drag&drop; шорткат `Ctrl+Shift+U` | [D] | `3eefa` (`846558`), `2ac5158769dd2c7f_0` (реестр: `UPLOAD_FILE`) | js |
| C-11 | Не больше **10** вложений на сообщение (схема `array().max(10)`, константа `XgB=10`) | [D] | `decoded/3eefa…` (`attachments:…max(10)`, constants `n4`) | js |
| C-12 | Вставка текста длиннее **4000** символов предлагает отправить файлом (`uploadPromptCharacterCount = CS1 = 4000`) | [D] | `decoded/f0a78…` (`uploadPromptCharacterCount:ea.CS1`) | js |
| C-13 | Кнопка эмодзи: класс `emojiButton__74017 button__74017` (radius 8, hover `--interactive-background-selected`), иконка 20; пикер открывается слоем `expressionPickerPositionLayer__74017` со `inset-inline-start:86px` | [D] | `decoded/2e60d16dae6e5d86_0` (CSS `962953`), `decoded/792f0283e57e7880_0` (карта классов) | css |
| C-14 | Поле: `font-size:1rem`, `line-height:1.375rem`, высота 56 (`--custom-channel-textarea-text-area-height`), `padding-inline:0 10px`, max-height `50vh` + скролл (`--custom-channel-textarea-text-area-max-height`) | [D]/[T] | `decoded/2e60…` (CSS), `ui/tokens.json` | css + token |
| C-15 | Вариант с отдельной кнопкой отправки существует и включается конфигом (`submit:{button:!0, clearOnSubmit:!0, disableEnterToSubmit:…}`), но скрыт экспериментом `isSubmitButtonEnabled`; при выключенном enter-to-submit `Ctrl+Enter` — штатная отправка | [D] | `decoded/3eefa…` (конфиги `submit`), `decoded/f0a78…` (флаги `isSubmitButtonEnabled`) | js |
| C-16 | Кнопок в ряду максимум 6; при сжатии панели «эмодзи+gif+sticker» могут заменяться одной кнопкой expression | [D] | `decoded/3eefa…` (`846558`, ветки `showAllButtons`/`expressionButtonsHidden`) | js |
| C-17 | Подсказка композера (notice) в референсе отсутствует: полоса «Отправка недоступна» — наше решение, не дамп | [A] | `ui/00-app.png` (notice-полосы нет) | png |

## 2. Панель участников

| id | значение/правило | код | источник | метод |
|---|---|---|---|---|
| M-01 | Порядок групп: **hoisted-роли** (в порядке `getSortedRoles`, т.е. по убыванию позиции) → **«В сети»** (online) → **«Не в сети»** (offline) | [D] | `decoded/f2772fb4bff12a25_0` (`ep()`: `getSortedRoles().filter(hoist)` + `ONLINE` + `OFFLINE`), `3eefa` (`Y()` → `hoistRoleId`) | js |
| M-02 | Секция участника = его `hoistRoleId`, если статус не `offline/invisible`; иначе `"online"` (если hoisted роли нет) или `"offline"` | [D] | `decoded/4d2a64cb5df5fc4e_0` (`calculateNewState`) | js |
| M-03 | Внутри секции сортировка по `(nick ?? globalName).toLowerCase()` (алфавит, регистронезависимо), вставка в отсортированную позицию | [D] | `decoded/4d2a…` (`rebuild()`, `findUserIdSortedPosition`) | js |
| M-04 | Текст заголовка группы: «{label} — {count}» (NBSP перед тире: `\u00a0— `); видимый текст `aria-hidden`, доступное имя — тултип/строка с count | [D] | `decoded/f2772fb4…` (JSX группы), `ui8/22-members.json` (`"Server Booster\u00a0— 2"`) | js + json |
| M-05 | Снятые группы: «Server Booster — 2», «В сети — 735»; offline-группа в срез **не попала** (панель не докручена до низа) | [D] | `ui8/22-members.json`, `ui8/22-members.jpg` | json |
| M-06 | Видимое окно: 35 строк (34 участника + заголовки), 2 группы, поиска в панели нет (`search: []`) | [D] | `ui8/22-members.json` (`rowCount`, `groupCount`, `search`) | json |
| M-07 | Строка участника: `.member__5d473{margin-inline-start:8px; max-width:calc(var(--custom-member-list-width) - 16px)}`, `.memberInner__5d473{padding-block:4px}`; аватар 32, имя 14/18, статус-текст 12/16 | [D] | `decoded/79e79bb7b4b2a133_0` (CSS), `ui8/22-members.json` (`classTokens`) | css + json |
| M-08 | Заголовок группы: `.membersGroup_c8ffbb{padding-block:16px 4px; padding-inline:12px 4px}` (=`--space-lg/--space-xxs`,`--space-md/--space-xxs`); имя группы — ellipsis; у бустеров — wheel-иконка (`margin-inline-end:4px`) и hover-цвет | [D]/[T] | `decoded/1377da32500f7fbe_0`, `ui/tokens.json` | css + token |
| M-09 | Offline-строки приглушены: `.offline__5d473{opacity:.3}`, на hover `opacity:1` | [D] | `decoded/79e79bb7b4b2a133_0` | css |
| M-10 | Панель: `border-inline-start:1px solid var(--border-subtle)`, фон `--custom-channel-members-bg`, список `padding:0 0 10px`, ширина 256 (`--custom-member-list-width`) | [D]/[T] | `decoded/1377da32…`, `ui/tokens.json` | css + token |
| M-11 | Локализация заголовков: «В сети» / «Не в сети»; группы-роли берут имя роли как есть | [D] | `ru-catalog.json` ключи `WbGtnH`, `Vv0abJ` | i18n |
| M-12 | Иконка роли в заголовке группы: `roleIcon_c8ffbb{margin-inline-end:4px}`; показывается, если у роли есть icon/unicodeEmoji | [D] | `decoded/1377da32…`, `c6debfeca50bb9fd_0` | css |
| M-13 | Пустые секции не рисуются: список групп фильтруется по `userIds.length > 0` | [D] | `decoded/f2772fb4…` (`g.filter(e=>e.userIds.length>0)`) | js |
| M-14 | Кол-во в заголовке — размер секции на момент рендера (735 online), а не всего сервера; общее число участников в дампе не снималось | [D]/[A] | `ui8/22-members.json`, `docs/re/frontend-d03.md` §7.8 | json |

## 3. Эмодзи-пикер

| id | значение/правило | код | источник | метод |
|---|---|---|---|---|
| E-01 | Карточка: min-width **498px**, height **440px**, radius 8 (`--custom-emoji-picker-border-radius`) | [T] | `ui/tokens.json` (`--custom-emoji-picker-constants-*`) | token |
| E-02 | Контент-вкладки (3): «Гифки», «Стикеры», «Эмодзи» (`aria-selected=true`) | [D] | `ui8/21-emoji-picker.json`, `ui8/21b-emoji-full.json` | json |
| E-03 | Поиск: placeholder «Найдите идеальный эмодзи», aria «Поиск» | [D] | `ui8/21-emoji-picker.json` (`searchPh`,`searchAria`) | json |
| E-04 | Видимый рейл (10): `top guild emoji`, `recent`, `<имя сервера>`, `people`, `nature`, `food`, `activity`, `travel`, `objects`, `symbols` — именно в этом порядке | [D] | `ui8/21d-emoji-categories.json` | json |
| E-05 | `aria-label` вкладки = «сырой» англ. id категории; локализованное имя — только в тултипе: «Люди», «Природа», «Еда и напитки», «Виды деятельности», «Путешествия», «Объекты», «Символы», «Флаги», recent — «Часто используемые», top — «Лучшие эмодзи сервера {guild}» | [D] | `ui8/21d…`, `ru-catalog.json` (`GX594D`, `egIBDH`, `ldm9aY`, `O783tR`, `w33hIP`, `gWm7Mk`, `QXMYAb`, `vvaizu`, `5TvaSm`, `W6Wi1X`) | json + i18n |
| E-06 | Полный enum категорий: `recent`, `favorites`, `top guild emoji`, `custom`, `people`, `nature`, `food`, `activity`, `travel`, `objects`, `symbols`, `flags`, `premium emoji`, `soundmoji`, `suggested`. `flags` в видимый рейл не попал (10 из 11 юникод-категорий) | [D] | `decoded/3eefa…`, модуль `732139` (`R2`) | js |
| E-07 | Глифы вкладок: recent → Clock, top guild → Trophy, activity → GameController, symbols → Heart, flags → Flag, favorites → Star; people/nature/food/travel/objects — минифицированные компоненты (имена не восстановимы из дампа) | [D] | `decoded/e903362d141767ee_0` (`to` — switch категорий) | js |
| E-08 | Размеры рейла: guild-иконка 32, unicode-иконка 24, padding иконки 4, высота пункта 48, разделитель 1px с отступами 12px | [T] | `ui/tokens.json` (`--custom-emoji-picker-constants-*`) | token |
| E-09 | Сетка: **9 колонок** (`rowSize` по умолчанию 9), ряды фиксированной длины | [D] | `decoded/3eefa…`, модуль `818645` (`rowSize=9`), `decoded/e903…` (grid rows) | js |
| E-10 | Наблюдённые объёмы: 50 эмодзи + 17 locked в гильдии, 42 в ЛС; поиск `:fire` → 10 (гильдия) / 11 (ЛС); секция «Доступно с подпиской Nitro» | [D] | `ui8/21-emoji-picker.json`, `21b-emoji-full.json`, `21-emoji-search.json` | json |
| E-11 | Нормализация поиска: срезать ведущее/хвостовое `:`, `toLowerCase`, удалить все пробелы и `_`, экранировать regex, искать **без учёта регистра** по цепочке имён эмодзи (name/uniqueName/все имена), в кандидате `_` тоже удаляется | [D] | `decoded/e903…` (`Y`: `replace(/^:/,"").replace(/:$/,"")`), `decoded/3eefa…` (`searchWithoutFetchingLatest`) | js |
| E-12 | Результаты поиска делятся на `unlocked`/`locked` (locked — премиум), `count:0` = без ограничения, сортировка по frecency-score, затем по имени | [D] | `decoded/3eefa…` (`searchWithoutFetchingLatest`, `getSearchResultsOrder`) | js |
| E-13 | «Недавние» = часто используемые (`getFrequentlyUsedEmojis…`), для реакций — отдельный список (`getFrequentlyUsedReactionEmojis…`) | [D] | `decoded/e903…` (`K`, `$`) | js |
| E-14 | Переключатель тона кожи: aria «Выбрать цвет кожи эмодзи (Пусто)» | [D] | `ui8/21-emoji-picker.json` (`skinTone`) | json |
| E-15 | Секции сетки: «Лучшие эмодзи сервера {guild}», «Часто используемые» (в ЛС — те же + серверные); серверные секции сворачиваемые (`EXPAND_OR_COLLAPSE_EMOJIS`) | [D] | `ui8/21-emoji-picker.json` (`sections`), `decoded/e903…` | json + js |
| E-16 | Поиск не ограничен текущей категорией: запрос идёт по всему набору (unicode + custom) выбранного канала/интенции; категория — только для листания | [D] | `decoded/e903…` (`Y` → `searchWithoutFetchingLatest({channel, query, intention})`) | js |

## 4. Настройки

| id | значение/правило | код | источник | метод |
|---|---|---|---|---|
| S-01 | 18 разделов + «Выйти», порядок: Учётная запись · Данные и конфиденциальность · Разрешения для сообщений · Уведомления · Nitro · Буст сервера · Подписки · Склад подарков · Выставление счетов · Голос и видео · Внешний вид · Специальные возможности · Система · Язык и время · Зарегистрированные игры · Конфиденциальность активности · Подключённые приложения · Разработчик | [D] | `ui8/23-settings-categories.json` (глубина 1, 63 строки) | json |
| S-02 | Подпункты «Учётной записи» (4): Информация об учётной записи · Пароль и безопасность · Статус учётной записи · Семейный центр | [D] | там же (строки `item_e4d939`) | json |
| S-03 | Поле поиска в навигации настроек: placeholder «Поиск» | [D] | `ui8/23-settings-categories.json` (`searchPh`) | json |
| S-04 | «Голос и видео», порядок разделов: **Голос** (Микрофон, Динамик, проверка) · **Профиль ввода** · **Настройки чувствительности ввода** · **Шумоподавление** · **Эхоподавление / Режим рации / АРУ** · **Камера** · **Стриминг** · **Звуки** · **Связанные настройки** (Уведомления, Звуковая панель) · **Расширенные** | [D] | `ui8/23-settings-voice.json` (`headings`, `text`) | json |
| S-05 | Тумблеров в «Голос и видео» — **38**: 12 функциональных + 22 звуковых (4 видимых + 18 под «Показать ещё 18 звуков») + 4 в «Расширенных» | [D] | `ui8/23-settings-voice.json` (`switchCount`, `switches`) | json |
| S-06 | Функциональные тумблеры (12) и снятые состояния: авто-чувствительность **вкл**, эхоподавление **вкл**, режим рации **выкл**, АРУ **вкл**, расширенная активация по голосу **вкл**, предложение аудиоустройств **вкл**, предупреждение об отсутствии звука **вкл**, предупреждение о смене голосового канала **вкл**, QoS **выкл**, предпросмотр видео (всегда) **вкл**, предпросмотр стрима **вкл**, приглушение стрима **вкл** | [D] | `ui8/23-settings-voice.json` | json |
| S-07 | Звуковые тумблеры (22): «Откл. звук», «Включить звук в динамиках», «Заглушить», «Вкл. микрофон», «Камера включена/выключена», «Голосовая связь отключена», «Активация/Деактивация режима рации», «Пользователь присоединился/вышел/перемещён», «Исходящий звонок», «Стрим начался/остановлен», «Зритель присоединился/вышел», «Начало/Завершение активности», «Присоединение к активности», «Покинул активность», «Приглашение выступить» — все **вкл** | [D] | `ui8/23-settings-voice.json` (`switches[12..33]`) | json |
| S-08 | «Расширенные» (4): OpenH264 **вкл**, оверлей с информацией о стриме **выкл**, запись звука для диагностики **выкл**, ведение журнала отладки **вкл**; отдельный тумблер «Показывать настройки диагностики» | [D] | `ui8/23-settings-voice.json` (`switches[34..37]`, `text`) | json |
| S-09 | Селекты (4): Микрофон «Система: Easy Effects Source», Динамик «Система: Easy Effects Sink», Камера «Нет видеоустройств», Шумоподавление «Krisp» | [D] | `ui8/23-settings-voice.json` (`selects`) | json |
| S-10 | Ползунки (4): микрофон 0–100 = 100; динамик 0–200 = 100; сила приглушения стрима 1–100 = 50 (подпись 49 %); громкость звуковой панели 0–100 = 26.11 (подпись 26 %) | [D] | `ui8/23-settings-voice.json` (`sliders`) | json |
| S-11 | Радиогруппа «Профиль ввода» (3): «Изоляция голоса» (только голос, Discord уберёт шум), «Студия» (открытый микрофон без обработки), «Пользовательский» (все кнопки) | [D] | `ui8/23-settings-voice.json` (`text`) | json |
| S-12 | Кнопки вкладки (6): «Проверка микрофона» / «Прекратить проверку», «Проверить видео», «Загрузить журналы», «Показать папку», «Сброс» | [D] | `ui8/23-settings-voice.json` (`buttons`) | json |
| S-13 | «Связанные настройки»: «Уведомления» (звуки новых сообщений и звонков) и «Звуковая панель» (громкость + звук присоединения, селект «Выберите сервер» / «Все серверы», «Выбрать звук» / «Изменить звук») | [D] | `ui8/23-settings-voice.json` (`text`) | json |
| S-14 | «Внешний вид» — известные подписи (i18n): «Отображение сообщений» с вариантами «Компактное» / «Уютное» (кнопка-подсказка «Включить уютный режим») | [D] | `ru-catalog.json` (`ZEoGMd`, `7iegX4`, `T7G4Y0`, `CTpwD7`) | i18n |
| S-15 | Вкладка «Уведомления» в дампе не открывалась: есть только пункт навигации и ссылка из «Голоса». Состав/тумблеры — нет данных | [D]/[A] | `ui8/23-settings-categories.json`, `23-settings-voice.json` | json |

## 5. Клавиатура и фокус

Реестр по умолчанию — `decoded/2ac5158769dd2c7f_0` (модуль `696760`, ключи `IWg`); описания — `ru-catalog.json`.
`mod` = Ctrl (Linux/Windows).

| id | значение/правило | код | источник | метод |
|---|---|---|---|---|
| K-01 | `Ctrl+K` — быстрый переключатель (QuickSwitcher); «Вкл./выкл. быстрый переключатель» | [D] | реестр (`QUICKSWITCHER_SHOW`), `ru-catalog` `yYsRlD` | js + i18n |
| K-02 | `Ctrl+E` — эмодзи-пикер, `Ctrl+G` — GIF, `Ctrl+S` — стикеры, `Ctrl+P` — закреплённые, `Ctrl+I` — почта/inbox | [D] | реестр (подмодули `877991`, `332779`, `274794`, `TOGGLE_CHANNEL_PINS`, `TOGGLE_INBOX`) | js |
| K-03 | `Ctrl+U` — список участников (toggle), `Ctrl+F`/`Ctrl+Shift+F` — фокус в поиск (с префиллом текущего канала / без) | [D] | реестр (`TOGGLE_USERS`, `FOCUS_SEARCH`) | js |
| K-04 | `Escape` — закрыть модалку (`MODAL_CLOSE`), отклонить звонок (`CALL_DECLINE`), закрыть слой/пикер; в контексте сообщения — «Фокус на текстовом поле» | [D] | `eacbdfad9946a1ad_0`, `0ad109a24de5d314_0`; `ru-catalog` `rrYBEu` | js + i18n |
| K-05 | `Tab` фокусирует композер (если есть `[data-can-focus="true"]`); набор любого печатного символа вне полей ввода также фокусирует композер и вставляет символ (`TEXTAREA_FOCUS = any-character`) | [D] | `decoded/2ac515…`, модуль `82038` | js |
| K-06 | `Enter` без Shift/Alt в композере = отправка (пробрасывается `wasEnterPressed`), см. C-07 | [D] | `decoded/2ac515…`, модуль `82038` | js |
| K-07 | `↑` в пустом композере — редактировать последнее сообщение | [D] | `decoded/f0a78…` | js |
| K-08 | `Ctrl+Alt+↑/↓`, `Ctrl+Shift+[`/`]`, `Ctrl+Tab`/`Ctrl+Shift+Tab` — переход по каналам; `Alt+↑/↓` — по серверам/каналам | [D] | реестр (`176522`, `928531`) | js |
| K-09 | `PageUp`/`PageDown` — скролл чата; `Shift+PageUp` — к первому непрочитанному; `Shift+Esc` — пометить прочитанным / прыжок к настоящему | [D] | реестр (`552049`, `shift+pageup`, `shift+esc`) | js |
| K-10 | `Ctrl+Plus` / `Ctrl+Minus` / `Ctrl+0` — зум ±/сброс | [D] | реестр (`406975`) | js |
| K-11 | `Ctrl+Shift+U` — отправить файл; `Ctrl+Shift+T` — создать группу ЛС; `Ctrl+Shift+Alt+N` — создать ветку; `Ctrl+T` / `Ctrl+W` — вкладка канала открыть/закрыть | [D] | реестр (`UPLOAD_FILE`, `CREATE_DM_GROUP`, `CREATE_THREAD`, `OPEN/CLOSE_CHANNEL_TAB`) | js |
| K-12 | Drag&drop: `↑/↓` — переместить, `Space`/`Enter` — отпустить, `Esc` — отмена | [D] | `ru-catalog` `dmMqay`, `cs/HVH`, `1ioMJQ`; реестр `DND` | i18n + js |
| K-13 | Модалки-карусели: `→/↓/Space` — вперёд, `←/↑` — назад, `Esc` — закрыть | [D] | `decoded/eacbdfad9946a1ad_0` | js |
| K-14 | Действия над сообщением (когда оно в фокусе): `E` — изменить, `Backspace` — удалить, `P` — закрепить, `+` — реакция, `R` — ответить, `F` — переслать, `S` — зачитать, `Ctrl+C` — копировать текст, `Alt+Enter` — пометить непрочитанным | [D] | реестр (`MESSAGE`), `ru-catalog` `fsBWmS`,`xwMqD7`,`CvQ18w`,`lfIHs4`,`5IEsGx`,`zSyDdA`,`yGLjXF`,`JrGD7E`,`RpE9k7` | js + i18n |
| K-15 | Диалог «Комбинации клавиш»: 6 групп в порядке **Навигация · Перетащить мышью · Чат · Голос и видео · Прочее · Сообщение** | [D] | `decoded/2ac515…` (`eg()`), `ru-catalog` `yGE+jg`,`69j6+4`,`/VQax8`,`bI8F5u`,`0Bg9La`,`BAB0yK` | js + i18n |
| K-16 | `Shift+F10` — контекстное меню элемента; `Ctrl+Shift+L` — копировать ссылку на канал | [D] | реестр (`OPEN_CONTEXT_MENU`, `COPY_CHANNEL_LINK`) | js |
| K-17 | `Ctrl+Alt+I` — devtools (на web/discord.com — no-op) | [D] | реестр (`BROWSER_DEVTOOLS`) | js |
| K-18 | Порядок Tab-обхода и наличие focus-trap в поповерах кодом дампа не подтверждены (в SPEC — `[A]`) | [A] | — | — |

## 6. Чат-вью

| id | значение/правило | код | источник | метод |
|---|---|---|---|---|
| V-01 | Шапка: слева иконка типа канала + имя; справа 4 иконочные кнопки (ветки, уведомления/колокол, закреплённые, участники) и поле поиска | [M] | `ui/00-app.png`, кроп `y 32…85` | png |
| V-02 | Поле поиска в шапке: 244×28, aria/плейсхолдер «Искать «ITSKIDON PC BOOST»» (шаблон «Искать «{guild}»») | [D] | `ui8/20a-searchbar.json` (`bars[0]`, `inputs[0].aria`) | json |
| V-03 | Топик канала в снятой шапке отсутствует (у канала нет темы) — вариант «имя + тема» визуально не подтверждён | [M]/[A] | `ui/00-app.png` | png |
| V-04 | Разделитель непрочитанного: высота 0, `border-top:thin solid var(--border-subtle)`, по центру; `--divider-color: var(--background-feedback-notification)`; у `isUnread` цвет границы и текста = divider-color | [D] | `decoded/` CSS `__908e2` | css |
| V-05 | Плашка «Новые сообщения» (new messages bar): высота **32**, фон `--brand-500`, текст белый, radius `0 0 8px 8px`, `padding-inline:16`, `top:0`, `z-index:3` | [D] | CSS `newMessagesBar__0f481` | css |
| V-06 | Плавающая плашка «Перейти к последним сообщениям»: `bottom:28px`, `padding-inline:12`, `padding-bottom:8` | [D] | CSS `jumpToPresentBar__0f481` | css |
| V-07 | Тексты: «Новые сообщения», «Перейти к последним сообщениям» | [D] | `ru-catalog.json` `q7hm3m`, `Sj83P7`/`gpoQsB` | i18n |
| V-08 | Контент разделителя: 12px semibold, line-height 13, padding `2px 4px`, radius 8, фон — фон чата; концевые «капли» (cap/endCap) 13px, endCap 10px bold uppercase; дата-разделитель использует тот же компонент (`hasContent`) | [D] | CSS `__908e2` (`content`, `endCap`, `unreadPillCap`) | css |
| V-09 | Cozy-сообщение (DOM): `li.container__80bf8[role=listitem][aria-setsize][aria-posinset] > .searchResult__80bf8 > .message__80bf8 > .wrapper_c19a55.cozy_c19a55.zalgo_c19a55[role=article] > .contents_c19a55 > (#message-content-<id>.markup__75297.messageContent_c19a55 + #message-accessories-<id>.container_b7e1cb)` | [D] | `ui8/20-search-results.html` | html |
| V-10 | ARIA сообщения: `role="article"`, `aria-labelledby="message-content-<id> message-accessories-<id>"`, заголовок — `h2.header_c19a55` c `aria-labelledby="message-username-<id>"`, `time#message-timestamp-<id>[datetime]` | [D] | `ui8/20-search-results.html` | html |
| V-11 | Формат времени в cozy: `DD.MM.YYYY, HH:mm` (пример «12.10.2026, 12:35»), перед ним `<i class="separator_c19a55"> — </i>`; рядом скрытая полная дата «понедельник, 12 октября 2026 г. в 12:35» | [D] | `ui8/20-search-results.html` | html |
| V-12 | Cozy-метрики: аватар 40 (`--chat-avatar-size`) на `inset-inline-start:12`; контент `margin-inline-start: 64` (`calc(40+12+12)`); wrapper `padding-inline-start:64`, `padding-block:2` (`0.125rem`), `min-height:22` (`1.375rem`); header `margin-inline-start:-72` (`-4.5rem`); `groupStart` `min-height:44` (`2.75rem`) | [D]/[T] | CSS `c19a55`/`5126c`, `ui/tokens.json` | css + token |
| V-13 | Compact: аватар `1em` inline (`position:relative; top:.18em`), wrapper `padding-inline-start:80` (`5rem`), `padding-block:2`, header inline, `groupStart` `min-height:22` (`1.375rem`), timestamp в gutter цветом `--primary-100` | [D]/[T] | CSS `compact_c19a55`, `--custom-message-margin-compact-indent` | css + token |
| V-14 | Вложения/эмбеды рендерятся в `#message-accessories-<id>.container_b7e1cb`; кнопки действий — `.buttonsContainer__80bf8` (в результатах поиска — кнопка «Перейти») | [D] | `ui8/20-search-results.html` | html |
| V-15 | Клик по аватару — открыть профиль (`.avatar_c19a55.clickable_c19a55`), `pointer-events` включается классом `clickable` | [D] | CSS `c19a55` | css |
| V-16 | Элемент списка: `data-list-item-id="NO_LIST___<id>"` (в результатах поиска `NO_LIST___…`), `id="search-result-<id>"` — вне списка канала id будет `chat-messages-<channel>-<id>` (в дампе не снят) | [D]/[A] | `ui8/20-search-results.html` | html |
| V-17 | Notice-полоса канала в референсе не наблюдалась; общий CSS `.channelNotice_e2d7b8{padding:6px 12px; border-bottom:1px solid var(--border-subtle)}` (space-xs/space-md) — это **не** 24px из SPEC | [D]/[M] | `decoded/` CSS `e2d7b8`, `ui/00-app.png` | css + png |
| V-18 | В снятом канале нет ни unread-divider, ни new-messages bar (канал прочитан, отправка запрещена) — эти контракты подтверждены только кодом/CSS, не пиксельно | [M] | `ui/00-app.png` | png |
| V-19 | Разделитель даты и непрочитанного — один компонент: дата без `isUnread`, непрочитанное — с `isUnread` (красная линия/текст) | [D] | CSS `__908e2` (`isUnread`, `hasContent`) | css |
| V-20 | Скролл-контейнер сообщений: `scroller__`/`scrollerInner__` c `scrollerBase_d125d2`, виртуализация (в DOM только окно) — подтверждено классами `messageListItem__5126c` и `scroller_*` | [D] | `ui8/20-search-results.html`, CSS `5126c` | html + css |

## 7. Чего в дампе нет (честно)

| id | пробел | что это значит для P3-02 |
|---|---|---|
| G-01 | Композер снят только в disabled-состоянии (нет права отправки): видимый плейсхолдер, порядок и ховеры кнопок пиксельно не подтверждены | C-02/C-09/C-13 — из кода; в UI-тестах проверять по контракту, не по скриншоту |
| G-02 | Вкладки «Внешний вид» и «Уведомления» не открывались (есть только подписи навигации) | число тумблеров/группы этих разделов неизвестны (`[A]` в SPEC §9.4) |
| G-03 | Offline-группа участников не попала в срез (панель не докручена), её заголовок/счётчик не снят | M-01/M-02 — правила из кода; счётчик offline — нет данных |
| G-04 | Сортировка внутри role-группы в дампе не видна (только «Server Booster»/«В сети») | M-03 — правило из кода, не из пикселей |
| G-05 | Категория `flags` в видимом рейле эмодзи отсутствует (10 из 11 юникод-категорий); почему — не установлено | E-06: не добавлять flags в спайк без решения PM |
| G-06 | Имена/глифы иконок people/nature/food/travel/objects минифицированы | E-07: переносить только подтверждённые (Clock/Trophy/GameController/Heart/Flag/Star) |
| G-07 | Нет unread-divider, new-messages bar, notice-полосы и топика в референсе | V-04…V-08, V-17 — только CSS/код, пиксельной проверки нет |
| G-08 | `id` строки канала (`chat-messages-…`) и `data-list-item-id` в списке канала не сняты | V-16: id-схема в спайке — `[A]` |
| G-09 | Точные шрифты/иконки референса (спрайты) не переносятся | вне scope (юридическая рамка `DESIGN/README.md` §4) |

## 8. Сводка

| Раздел | Фактов | Лучше всего закрыто |
|---|---|---|
| Композер | 17 | лимиты/счётчик/отправка, порядок кнопок, вложения |
| Участники | 14 | группировка/сортировка/заголовки, offline-стиль |
| Эмодзи | 16 | 10 категорий и порядок, поиск (нормализация/поля), 9 колонок |
| Настройки | 15 | полный состав «Голос и видео» (38 тумблеров, 4 селекта, 4 ползунка) |
| Клавиатура/фокус | 18 | реестр биндов + 6 групп диалога, Esc/Tab/↑ |
| Чат-вью | 20 | unread/divider CSS, cozy/compact шаблон, шапка |
| **Итого** | **100** | |
