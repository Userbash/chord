# Ревью: INT-07-UI-APP-MOUNT — монтаж `voiceDevices` и `notifications` в `app.js`

- **Ветка / HEAD:** `rw40/int07-ui-app-mount` / `4e24048`.
- **Объект ревью:** `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/index.html`,
  `tests/unit/gui-spike-features.test.mjs`, артефакты `waves/RW-40/PM/int07-ui-mount/`.
- **Ревьюер:** DeepSeek Review 35A (независимая проверка, автору код не принадлежит).
- **Дата:** 2026-10-30.
- **Вердикт:** **RETURN** — 2 × P1, 8 × P2. Монтаж технически работает и покрыт
  тестами, но два входных контрола остаются `aria-disabled` и при этом выполняют
  действия, а сохранение уведомлений уходит в IPC-команды, которых нет в
  `generate_handler!`. В таком виде фича в `unstable` не mergе-готова.

---

## 1. Что проверено

### 1.1. MD5 против `REPORT.md` §5

| Файл | В отчёте | Измерено | Итог |
|---|---|---|---|
| `rust/gui-spike/ui/app.js` | `d3a3efe6b18ecfe6fe004e81a81ba961` | `d3a3efe6b18ecfe6fe004e81a81ba961` | совпадает |
| `rust/gui-spike/ui/index.html` | `ea9649ab18041644d5ed0ac11761a2dc` | `ea9649ab18041644d5ed0ac11761a2dc` | совпадает |
| `tests/unit/gui-spike-features.test.mjs` | `38f60853e7d30f14a21f897477a5c778` | `38f60853e7d30f14a21f897477a5c778` | совпадает |
| `waves/RW-40/PM/int07-ui-mount/red.log` | `de60d8d22d00135eb2f167d658d31f72` | `de60d8d22d00135eb2f167d658d31f72` | совпадает |
| `waves/RW-40/PM/int07-ui-mount/green.log` | `0f5d224569f9f7072a8c5c29023db180` | `0f5d224569f9f7072a8c5c29023db180` | совпадает |
| `waves/RW-40/PM/int07-ui-mount/mutations.log` | `46bcfd31645b70f2b373d1103ee371ac` | `46bcfd31645b70f2b373d1103ee371ac` | совпадает |

### 1.2. Прогоны (повторены ревьюером)

```bash
node --test tests/unit/gui-spike-features.test.mjs
# tests 102 | pass 102 | fail 0 | exit 0

node --test tests/unit/gui-spike-dom.test.mjs
# tests 38 | pass 38 | fail 0 | exit 0

node --test tests/meta/test-policy.test.mjs
# tests 17 | pass 17 | fail 0 | exit 0

npx tsc -p rust/gui-spike/ui/tsconfig.json   # UI_TSC_EXIT=0
npx tsc -p tsconfig.json                     # ROOT_TSC_EXIT=0
```

Совместный прогон трёх наборов из ТЗ: `tests 157 | pass 157 | fail 0 | exit 0`.

### 1.3. Логи red/green/mutations

- `red.log`: `tests 102 | pass 100 | fail 2`, оба падения — новые кейсы
  `INT-07-UI-APP-MOUNT` с сообщениями `currentApp().voiceDevices не инициализирован`
  и `currentApp().notifications не инициализирован`. RED-first соблюдён.
- `green.log`: `tests 102 | pass 102 | fail 0`, затем оба `tsc` с `exit 0`.
  Смежных наборов в логе **нет** (см. P2-7).
- `mutations.log`: 5 мутантов, все `KILLED`, после каждого restore с `match=True`,
  финальный контроль `102 pass`, `FINAL_MD5=d3a3...` совпадает.
  Мутант `MUT-2-guild-notif-wrong-guild-id` воспроизведён ревьюером независимо:
  `app.js → 8668d52282b8882cd1745f8a4bbede84`, `pass 101 / fail 1`,
  падение `guildId в контроллере не 777`, после restore MD5 снова `d3a3...`.

### 1.4. Функциональная проверка поведения (DOM-шим)

Подтверждено на реальном `app.js`:

- `currentApp().voiceDevices` и `currentApp().notifications` существуют, у
  `voiceDevices` есть `getState` / `setDevices`.
- `#voice-devices` смонтирован внутри `#settings-dialog` → `.settings-body` →
  `#settings-modal`; `#settings-modal` лежит рядом с `#settings`, а не внутри него,
  поэтому 38 тумблеров и 3 `h3` в `#settings` сохраняются (проверено тестами и
  пробой: `.toggle`/`.toggle-input` = 38/38 даже при открытой модалке).
- Клик по `#server-menu-notifications` открывает `#guild-notifications-dialog`,
  подставляет гильдию из `rail` (`id=777`, имя «Гильдия Рубин»), заполняет
  `#notif-override-channel-select` из `tree.channels` и шлёт
  `update_guild_notification_settings` с `guildId=777`.
- `contextmenu` на `.channel-item` открывает `#channel-notif-menu` с
  `data-channel-id` канала; выбор уровня шлёт `update_channel_notification_override`
  с правильными `guildId`/`channelId`.
- Колокол `[data-action="inbox"]` открывает меню для `chat.channelId`.
- RED→GREEN и мутации для этих веток честные.

### 1.5. Что не проверялось

Полный `npm run gate:worker`, `cargo`-воркспейс и живое окно Tauri — по ТЗ карточки
это batch-гейт и Rust-лейн; в ревью не входят.

---

## 2. P1 — блокирует приёмку

### P1-1. `aria-disabled`-контролы выполняют действия; клавиатура и AT до них не доходят

**Где:** `app.js:1661-1685`, `index.html:107`, `features/actions.js:52`, `:466-470`, `:928-935`.

**Что происходит.** Оба «входа» в N-05 — это элементы, которые доска объявляет
недоступными:

- `#server-menu-notifications` (`features/actions.js:52`) создан с
  `data-static-disabled="true"`, без `data-action`, с `aria-haspopup="menu"`.
  `refreshMenuState()` на каждом открытии ставит `aria-disabled="true"`,
  `tabindex="-1"` и `aria-describedby="menu-reason-not-implemented"`; CSS
  `.menu-item[aria-disabled="true"]` гасит его до `--state-disabled-opacity` и
  убирает hover (`components/server-menu.css:82-90`).
  При этом `app.js:1664-1668` вешает на него прямой `click`-листенер, и клик
  мышью открывает модалку. Проба подтверждает: `aria-disabled=true`,
  `tabindex=-1`, `data-action=null`, но `#guild-notifications-dialog` получает
  `hidden=null`.
- Колокол `[data-action="inbox"]` в `index.html:107` имеет
  `aria-disabled="true"` и `title="Уведомления (Phase 3)"`; VW-01-тесты
  (`gui-spike-features.test.mjs:359-367`, `gui-spike-dom.test.mjs:272-277`)
  фиксируют его как disabled-affordance. `app.js:1674-1684` вешает свой
  листенер в обход guard'а `initActions` (`actions.js:928-935`) и открывает
  `#channel-notif-menu`.

**Почему блокирует.**

1. Пользователь видит серый, «недоступный» пункт меню и колокол, но клик по ним
   работает — прямое нарушение контракта disabled (D1 в `actions.js`: «disabled —
   полный no-op»), зафиксированного существующими тестами.
2. Клавиатурой до пункта меню не добраться: `enabledEntries()` его исключает,
   roving tabindex его не ставит, Enter/Space обрабатывает только enabled-пункты.
   Модалка уведомлений сервера доступна только мышью.
3. `aria-haspopup="menu"` обещает подменю, а открывается диалог; у колокола нет
   `aria-haspopup`/`aria-controls`, хотя `NOTIFICATIONS-SPEC.md:1350` требует
   `button[data-action="channel-notif-menu"]` с этими атрибутами.
4. Новые тесты закрепляют это состояние: проверяют, что клик по disabled-пункту
   открывает модалку, и нигде не проверяют `aria-disabled`/`aria-haspopup`.
   То есть тест кодирует обход контракта, а не контракт.

**Что нужно.** Либо привести контролы в согласованное состояние: в
`features/actions.js` убрать `disabled`/`data-static-disabled` у пункта #4, дать
ему `data-action="guild-notifications"`; в `index.html` снять `aria-disabled` и
`title` у колокола, добавить `aria-haspopup="menu"`,
`aria-controls="channel-notif-menu"`, обновить VW-01-тесты (`gui-spike-dom`,
`gui-spike-features`) и записать изменение контракта в `DECISIONS.md`
(`TEST-POLICY.md` §5). Либо не вешать листенеры на disabled-элементы вообще.

Оба варианта выходят за файловую матрицу карточки (`features/actions.js`,
`gui-spike-dom.test.mjs` — чужие файлы), поэтому это остановка и вопрос Team Lead,
а не «допишу по-тихому».

### P1-2. Модалка уведомлений сохраняет через IPC-команды, которых нет в `generate_handler!`

**Где:** `app.js:1575-1578`, `features/notifications.js:1327-1350`, `:1376-1400`,
`rust/gui-spike/src/main.rs:1067`.

**Что происходит.** Контроллер на каждое переключение шлёт
`invoke("update_guild_notification_settings", …)` и
`invoke("update_channel_notification_override", …)`. В `main.rs` эти команды не
зарегистрированы: `grep` по `main.rs` не находит ни одного из имён, в
`generate_handler![` (строка 1067) их нет, `#[tauri::command]`-обёрток тоже нет
(функции есть только в `lib.rs:232-258`). В реальном окне Tauri первый же клик по
«Заглушить сервер» или уровню канала даст ошибку неизвестной команды; контроллер
покажет `#guild-notif-error` и останется в `data-state="error"`.

**Почему блокирует.** Карточка называется «монтаж контроллеров», но монтирует
путь, который гарантированно падает при любом сохранении. Риск 2 в отчёте это
признаёт, но монтаж всё равно включён — значит, в `unstable` уезжает заведомо
нерабочая кнопка. Нужно либо сначала зарегистрировать команды (G1 / карточка
`N-05-NOTIFICATIONS-BACKEND`, файл `main.rs` — вне границ этой карточки), либо
явно держать модалку в заглушечном состоянии (`aria-disabled` на сохранении,
видимая пометка), а не выпускать рабочий на вид контрол.

---

## 3. P2 — не блокирует, но нужно поправить

### P2-1. Панель голосовых устройств стоит не в той панели настроек и видна на всех 18 разделах

`VOICE-DEVICES-SETTINGS-SPEC.md` §2.1 требует
`section.settings-pane[data-pane="voice"]` внутри `.settings-body` → `.settings`.
Фактически `#settings-modal` — сиблинг `#settings` (`index.html:177`), вне
`#settings`, без `hidden`, без `role`. Проба: после переключения на «Внешний вид»
(`#settings [data-pane="appearance"]` видим, `voice` скрыт) `#settings-modal`
остаётся видимым, `#voice-devices` на месте. То есть панель устройств висит под
«Учётной записью», «Nitro» и любым другим разделом. Плюс `#settings-modal` — не
модалка (нет `role="dialog"`, `aria-modal`, `hidden`), имя вводит в заблуждение.

Причина конфликта: ТЗ карточки прямо называет `#settings-modal` как место монтажа,
а дизайн-спека — панель `voice`. Это расхождение ТЗ и спеки, решает PM/Team Lead:
либо монтаж внутрь `data-pane="voice"` (и тогда нужно пересмотреть инвариант
38 тумблеров, потому что панель окажется внутри `#settings`), либо оставить
сиблинг, но скрывать его вместе с панелью `voice`.

### P2-2. `voiceDevices` смонтирован без `service`: настройки живут только в памяти

`app.js:1568-1571` передаёт только `host` и `devices`. По контракту
(`VOICE-DEVICES-SETTINGS-SPEC.md` §1.3) все изменения уходят в инжектированный
`service`; без него контроллер честно обновляет только локальное состояние.
Итог: выбор устройства, громкости, PTT-клавиши и mute/deafen не сохраняются и не
доходят до Rust — после перезагрузки всё откатывается, ошибки нет, пользователь
ничего не замечает. Плюс список устройств — синтетические `default-input` /
`default-output` (`app.js:42-58`), а не перечисление из
`voice_devices`/`list_audio_devices` (таких команд в `main.rs` тоже нет).
Рекомендация: либо передать `service` с `invoke(...)` и реальным списком, либо
пометить секцию как предварительную (заглушка/`aria-disabled`), а не выдавать
список из двух выдуманных устройств за рабочий выбор.

### P2-3. Переименование `.toggle` → `.notif-toggle` снимает стили тумблеров

`app.js:1583-1590` снимает `toggle` с `label` и `toggle-input` с `input` в
`#guild-notifications-dialog`. CSS для `.notif-toggle` / `.notif-toggle-input` нет
нигде (`grep -rn "notif-toggle" rust/gui-spike/ui/components` — пусто), а
`components/settings.css:129-211` стилизует именно `.toggle` и
`.toggle-input:checked + .toggle-track`. Итог: в модалке уведомлений пропадают
flex-раскладка строки, скрытие нативного чекбокса, фон/сдвиг трека, hover и
focus-ring — остаются голые чекбоксы. Функционально клики работают, визуально
это регресс. Правильнее сузить инвариант 38 в тестах до `#settings` (это чужие
файлы — нужен Team Lead), либо добавить CSS для новых классов.

### P2-4. Модалка показывает выдуманные настройки как реальные

`syncNotificationsWithGuild()` при первом открытии передаёт
`guildEntry: { guild_id }`, и контроллер достраивает дефолты
(`message_notifications=ONLY_MENTIONS`, `mobile_push=true` и т.д.). По
`NOTIFICATIONS-SPEC.md` / `G05-REST-SPEC §2.2 #4` значения и подзаголовок берутся
только из проекции `user_guild_settings`, без данных — скрыты. Сейчас UI
показывает «Только @упоминания» и включённые тумблеры как факт, хотя это
литеральные дефолты. Плюс первый клик по тумблеру отправляет патч, посчитанный
от выдуманного состояния.

### P2-5. Меню сервера остаётся открытым под модалкой; синхронизация каналов дёргает модалку

`#server-menu` после клика по `#server-menu-notifications` сохраняет
`hidden=null` (проверено пробой): `body`-листенер `actions.js:1009-1014` не
закрывает меню для клика внутри меню, а своего `closeMenu()` у нового обработчика
нет. Для сравнения, «Покинуть сервер» меню закрывает. В результате под backdrop
остаётся открытое меню с `aria-expanded="true"` у опенера.

Отдельно: `openChannelNotifications()` вызывает
`syncNotificationsWithGuild(null, false)` (`app.js:1651-1658`), а тот делает
полный `openGuildModal()` (`notifications.js:2014-2046`) — `renderModal()`,
снятие `hidden`, `muteSwitch.focus()`, — и сразу `closeGuildModal()`. Побочные
эффекты: скачок фокуса на скрытый контрол, сброс `modalState` из `pending` в
`ready` (нельзя закрыть модалку во время сохранения корректно), лишний рендер.
Контроллеру нужен отдельный метод «обновить гильдию/каналы без открытия», а не
open→close.

### P2-6. Магические fallback'и гильдии и канала

`currentGuildInfo()` (`app.js:1598`) при пустом рейле подставляет `id = "1"` —
IPC уйдёт на несуществующую гильдию. Колокол (`app.js:1676-1680`) при отсутствии
открытого канала берёт `tree.channels[0]?.id`, а первым в списке может быть
категория (`type: 4`); при пустом списке — молчаливый no-op без сообщения.
Лучше: без выбранной гильдии/канала не открывать меню и писать причину в
`#server-status`.

### P2-7. Отчёт и доказательства

1. `green.log` содержит только `gui-spike-features` + два `tsc` (197 строк,
   заканчивается `EXIT_CODE=0`). Заявленные в §3 «смежные наборы: 100 passed»
   в логе отсутствуют. Сами цифры верны — ревьюер повторил прогон и получил
   ровно `100 pass` — но по `TEST-POLICY.md` §4 доказательство — это
   команда → exit code → артефакт; артефакта нет.
2. Заявление «разметка синхронизирована с DOM-шимом» неточно: в
   `tests/support/gui-dom-harness.mjs` нет ни `settings-modal`, ни
   `voice-devices`, ни `guild-notifications-dialog`, ни `channel-notif-menu`.
   Паритет-тест проходит потому, что `app.js` сам создаёт недостающие узлы
   (`app.js:1555-1567`, `notifications.js:925-931`), а не потому, что шим
   повторяет `index.html`. Это два источника истины для одной разметки; тест
   сверяет только наличие id/класса, но не структуру.
3. В `tests/unit/gui-spike-features.test.mjs` лежит хунк карточки `g05-rest`
   (G-04/429, строки 3087-3097). Он задокументирован в
   `waves/RW-40/PM/g05-rest/REPORT.md:90`, к INT-07 отношения не имеет, но
   входит в тот же MD5. Две карточки правили один тестовый файл — это нарушение
   файловой матрицы; в отчёте INT-07 об этом ни слова, авторство хунка по MD5
   не восстановить.

### P2-8. Пробелы в тестах и мутациях

Новые тесты не проверяют: `aria-disabled`/`tabindex`/`aria-haspopup` у входных
контролов, наличие CSS-классов у тумблеров модалки, закрытие `#server-menu` при
открытии модалки, поведение при пустом рейле/канале, `Escape` и возврат фокуса.
Мутации MUT-1…MUT-5 покрывают только happy-path (`devices`, `guildId`,
`channels`, `channelId` в contextmenu и колоколе). Ни одна мутация не убивает
снятие `aria-disabled`-конфликта или переименование классов, потому что таких
проверок нет.

---

## 4. Итог

Сильная сторона работы: монтаж сделан аккуратно по границам файлов, RED-first
соблюдён, 5/5 мутаций воспроизводятся, `tsc` чистый, инвариант 38 тумблеров не
сломан, привязка к актуальной гильдии и `tree.channels` реально работает, а не
декларируется.

Блокируют приёмку два пункта: disabled-контролы, которые действуют (P1-1), и
сохранение в незарегистрированные IPC-команды (P1-2). Оба требуют файлов вне
матрицы карточки, поэтому дальше — решение Team Lead: либо расширить границы
(разрешить `features/actions.js`, `index.html`-триггеры и VW-01-тесты +
`DECISIONS.md`), либо отложить монтаж до регистрации команд в `main.rs`.

После исправления P1-1/P1-2 и ответа на P2-1 (куда всё-таки монтировать панель
`voice` — в `data-pane="voice"` или сиблингом) нужен повторный прогон трёх
наборов, `tsc` и обновлённый `green.log` со смежными наборами в артефакте.
