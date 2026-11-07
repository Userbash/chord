# Независимое ревью — DeepSeek Review 31A: N-05-UI-NOTIFICATIONS

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Ревью read-only: продуктовый код, тесты, спека, RE и логи не изменялись. Записан только этот файл.

## Вердикт

**RETURN. P0 = 0, P1 = 1, P2 = 7.**

Контракт `NOTIFICATIONS-RE.md` §1–§12 и `NOTIFICATIONS-SPEC.md` §1–§5 выполнен по существу в перечисленном объёме: пресеты, истечение mute, каскад, матрица упоминаний, индикаторы, модалка, таблица с roving `tabindex` и стрелками, закрытие `#channel-notif-menu` по `Escape` с возвратом фокуса, тела IPC. Red/green/mutation evidence формально соответствует `TEST-POLICY.md`: 14/14 поведенческих падений до кода, 14/14 зелёных на финальных файлах, 5/5 мутантов убиты, baseline md5 восстановлен. Возврат — из-за одного дефекта исполнения, который тесты физически не могут поймать (P1), плюс семь не блокирующих замечаний.

## P1 (блокирует приёмку)

1. **`NodeList.filter` в `getEnabledChannelMenuItems` — TypeError в реальном webview.**
   `rust/gui-spike/ui/features/notifications.js:1830-1837` вызывает
   `channelMenu.querySelectorAll('[role="menuitem"], [role="menuitemradio"]').filter(...)`.
   В браузере `querySelectorAll` возвращает `NodeList`, у которого нет `filter`; проект это уже
   учитывает — ср. `features/actions.js:339` (`[...container.querySelectorAll(...)].filter(...)`).
   Последствия в продукте:
   - `openChannelMenu` (`:2064-2075`) → `setChannelMenuRovingFocus` → исключение: меню
     открывается (`hidden` уже снят), но `tabindex="0"` не выставляется и фокус не переносится;
   - любой `keydown` внутри меню (`handleChannelMenuKeydown`, `:1949-1992`) падает до ветки
     `Escape`, поэтому `closeChannelMenu({restoreFocus:true})` не вызывается: меню не
     закрывается, `aria-expanded` остаётся `"true"`, фокус на триггер не возвращается.

   То есть ровно тот пункт приёмки, который тест объявляет проверенным
   (`tests/unit/notifications-ui.test.mjs:897-962`), в браузере не работает.

   Почему это не поймано:
   - `tests/support/gui-dom-harness.mjs:417-419,689-705` — `querySelectorAll` шима возвращает
     настоящий `Array` (результат `.filter`), поэтому тест проходит на расширении, которого нет
     в DOM;
   - DOM-хэндлы в модуле имеют тип `any` (`options.document`, `channelMenu`), поэтому `tsc`
     тоже молчит — я отдельно прогнал файл с опциями `rust/gui-spike/ui/tsconfig.json`
     (`--strict --checkJs --noUncheckedIndexedAccess`): `exit 0`, ошибка не видна.

   Минимум для снятия: `Array.from(channelMenu.querySelectorAll(...))` либо spread, и red-тест,
   который подаёт коллекцию без `filter` (например, вариант шима, возвращающий `NodeList`-подобный
   объект), иначе регрессия снова пройдёт незамеченной.

## Проверка контракта (что сходится)

1. **Пресеты таймера.** `MUTE_TIMER_PRESETS = [900, 3600, 10800, 28800, 86400, -1]` и порядок
   id `15-minutes…forever` — `notifications.js:13-23`; совпадает с `NOTIFICATIONS-RE.md §4`
   (`@8349873` `Xx`, `@6225743` `b8()`) и `NOTIFICATIONS-SPEC.md §1.3.1`.
   `buildMuteConfig` (`:111-121`) для `-1` даёт `end_time: null`, для положительного окна —
   `now + window*1000` в ISO, как требует `@11993`.
2. **Истечение mute.** `evaluateMuteState` (`:139-169`) повторяет формулу `@2364848`:
   `muted && (endTimeMs === null || endTimeMs >= nowMs)`; бессрочный mute без `mute_config`
   остаётся `isMuted`, исторический `end_time` 2021 г. снимает mute (`isExpired`), `remainingMs`
   считается по времени, а не по строке. Строгий разбор ETF-атомов — `parseStrictBool` (`:84-88`),
   баг `Boolean("false") === true` закрыт.
3. **Каскад.** `resolveEffectiveNotifications` (`:412-513`): канал `!= 3` → категория `!= 3` →
   гильдия `∈ {0,1,2}` → `default_message_notifications` (с фоллбэком на `1`, а не `0`).
   `isMuted = guild || category || channel`, `allowNoMessages` = mute ∨ `NO_MESSAGES` ∨ opt-in
   подавление (`:474-486`); opt-in считается как «включён на гильдии и не выставлен ни у канала,
   ни у категории» — совпадает с §1.5 п.2.
4. **Матрица упоминаний.** `evaluateIncomingMessageNotification` (`:544-582`): прямое `@user`
   инкрементит бейдж всегда (включая `muted` и `NO_MESSAGES`), `@role` — при `!suppress_roles`,
   `@everyone`/`@here` — при `!suppress_everyone`; звук/desktop требует `!allowNoMessages` и
   неподавленного типа упоминания; обычное сообщение уведомляет только при `ALL_MESSAGES`.
   Совпадает с §1.5 п.3.
5. **Биты и `channel_overrides`.** `setUnreadFlag`/`clearUnreadFlags` (`:179-193`) сохраняют
   `4096|2048`; `normalizeChannelOverrides` (`:208-260`) принимает массив и словарь;
   `hasVisibleNotificationOverride` (`:270-282`) реализует фильтр §1.4 п.2;
   `mergeGuildSettingsUpdate` (`:293-371`) не стирает карту при апдейте без ключа
   `channel_overrides` и игнорирует устаревший `version`.
6. **Индикаторы.** `computeChannelIndicatorState`/`applyChannelIndicatorToDom` (`:624-722`) и
   `computeGuildRailIndicatorState`/`applyGuildRailIndicatorToDom` (`:744-800`): бейдж `99+`,
   точка непрочитанного без дубля при бейдже, `🔕`, скрытие по `hide_muted_channels` с
   сохранением каналов с упоминаниями и `aria-current="page"`, левая пилюля гильдии скрыта при
   mute/выборе, бейдж упоминаний гильдии остаётся видимым при `data-muted="true"`, русские
   `aria-label`. Соответствует §2.4, §4.2, §5.1.
7. **Модалка.** `#guild-notifications-dialog` > `#guild-notif-modal` (`:925-941`), `role="dialog"`,
   `aria-modal`, `aria-labelledby`/`aria-describedby`, секции mute/уровня/подавления/таблицы,
   `#guild-notif-mute-window-row`, `#guild-notif-window-select` с 6 окнами,
   `#guild-notif-large-warning` при `level === 0 && memberCount >= 2500`, `#guild-notif-error`,
   первичный фокус на `#guild-notif-mute-switch` (`:2041`). Структура повторяет §2.2.
8. **Таблица `.notif-override-table`.** 5 колонок в порядке Канал / Все / Упоминания / Ничего /
   Mute (`:1166-1177`); roving `tabindex` по трём радиокнопкам строки, циклические
   `ArrowLeft`/`ArrowRight` (и `ArrowUp`/`ArrowDown`) с переносом `tabindex="0"` и выбором уровня
   (`:1557-1617`); чекбокс mute с `Space`/`Enter` и `preventDefault` (`:1642-1648`); сброс
   `{message_notifications:3, muted:false, flags:PW(flags)}` (`:1661-1672`) без удаления записи и
   без затирания `collapsed`. Соответствует §1.4 п.3, §5.2 п.4.
9. **Меню канала.** `#channel-notif-menu` с 6 пресетами, 4 уровнями (включая `3`), быстрым
   снятием mute, `role="menuitemradio"`, roving `tabindex`, `ArrowUp/Down/Home/End`, `Escape` с
   `preventDefault` + `stopPropagation` + `stopImmediatePropagation` и возвратом фокуса на
   `opener` с `aria-expanded="false"` (`:1209-1294`, `:1949-1992`, `:2081-2092`) — §2.3, §5.2 п.3.
   (Сам P1 выше ломает это в браузере, но логика и тестовый контракт написаны верно.)
10. **IPC.** `update_guild_notification_settings` (`:1327-1341`) с телом
    `{guilds:{[id]:patch}}` для гильдии и `{...patch}` для `@me`; `update_channel_notification_override`
    (`:1376-1396`) с телом `{channel_overrides:{[chId]:...}}`. Оба имени совпадают с публичными
    обёртками `rust/gui-spike/src/lib.rs:232-261`; тела — с `NOTIFICATIONS-RE.md §9.3` и
    `NOTIFICATIONS-SPEC.md §1.6.2–§1.6.3`. Мутации до ответа IPC нет: `aria-checked` меняется
    только после `renderModal()` в success-ветке.

## Evidence review (TEST-POLICY §3–§7)

- **md5 совпадают с отчётом и текущими файлами:** `notifications.js` `e9a4e8b8b50b6f59180bbc6ac13593f2`,
  тест `d8589a94b372da89a16602c7175a4316`, `red.log` `fc3105c502a35bf1009ef8505028be9a`,
  `green.log` `47cd66b2dd6741a0a9ec81a1048af240`, `mutations.log` `5432380f6f2352e0180eee566522973f`.
- **RED:** `red.log` — `tests 14 | pass 0 | fail 14`, все падения `AssertionError [ERR_ASSERTION]`,
  `ERR_MODULE_NOT_FOUND` нет. Ссылки в логе (`192:3`, `247:3`, `313:3`, `353:3`, `375:12`,
  `971:12`, `1037:12`) совпадают с финальным тестом построчно; значения из заглушки видны
  (`actual: []` для пресетов, `0 !== 3600`, `-1 !== 2`). Red снят в 19:26, продуктовый код
  записан в 19:33 — red-first по времени подтверждается.
- **GREEN:** воспроизвёл сам `node --test tests/unit/notifications-ui.test.mjs` → 14 passed, 0 failed,
  0 skipped, `exit 0`; `node --test tests/meta/test-policy.test.mjs` → 17 passed, `exit 0`.
  `green.log` содержит `TSC_OK=0`.
- **MUTATION:** `mutations.log` — 5/5 `KILLED`, exit 1 у каждого, тексты падений совпадают с
  реальными ассертами теста (например, «39 эффективно заглушённых каналов (42 - 3 истёкших)» —
  `tests/unit/notifications-ui.test.mjs:348`, «после закрытия #channel-notif-menu по Escape фокус
  обязан вернуться на триггер» — `:960`). После каждого мутанта `match=true`, итоговый
  baseline `e9a4e8b8…` равен текущему файлу, `post-restore exit 0`.
- **Оракул независимо пересчитан** по незатронутой фикстуре (`git status fixtures/` чист):
  `jq`/`node` дают 58 записей, 1328 `channel_overrides`, 22 заглушённые гильдии, 42 заглушённых
  канала, 3 истёкших `end_time`, 39 эффективно заглушённых каналов, уровни гильдии `{0:19, 1:36, 2:3}`.
  Всё совпадает с ассертами теста и `NOTIFICATIONS-RE.md §12`.
- **Ослаблений нет:** в двух новых файлах 0 `.only`, 0 `.skip`/`todo`, 0 `@ts-ignore`, 0
  `assert.ok(true)`; оракулы (`fixtures/`, `NOTIFICATIONS-SPEC.md`, `NOTIFICATIONS-RE.md`) не
  переписаны. Строка §8 для нового теста в `TDD-PLAN.md` присутствует (`:381`).

## P2 (не блокируют сами по себе)

1. **Focus trap модалки не реализован.** `NOTIFICATIONS-SPEC.md §5.2 п.4` требует циклический
   `Tab`/`Shift+Tab` внутри `#guild-notif-modal`. В коде есть только обработчик `Escape`
   (`notifications.js:1813-1820`), при этом комментарий `:1812` обещает «Escape и focus trap».
   Ловушка фокуса не покрыта тестом; фраза в комментарии вводит в заблуждение.
2. **Нет таймера истечения mute.** §1.3.2 п.2 ждёт `setTimer` на `max(0, endMs - nowMs)` и
   локальный сброс `muted = false` (аналог `GUILD_MUTE_EXPIRED`/`CHANNEL_MUTE_EXPIRED`).
   В модуле нет ни `setTimeout`, ни перерисовки по истечении: открытая модалка/меню показывают
   устаревший mute до следующего `renderModal`. Расчёт `isExpired`/`remainingMs` при этом верный.
3. **Нет обработки `429 retry_after`.** §4.1 требует держать контрол `disabled` с посекундным
   «Повторите через {n} с.». `submitGuildPatch`/`submitChannelOverridePatch` (`:1351-1365`,
   `:1422-1430`) различают только `401` (и то лишь при переданном `onAuthRequired`), остальное
   сводят к тексту ошибки. Для приёмки это допустимо, если интеграционный лейн закроет
   типизированные ошибки `G05-REST-SPEC §3.0`; в отчёте это не оговорено.
4. **`marksUnread` расходится с §1.5.3 строкой 4.** `:571-572` считает
   `incrementsMentionBadge || !chMutedOrCatMuted`, не разбирая `resolvedUnread` из битов
   `UNREADS_*`. Для канала/гильдии, явно переведённых в «только упоминания», обычное сообщение всё
   равно пометится непрочитанным. Тест `:503` (`plainAtMentions.marksUnread === true`) фиксирует
   именно это поведение, хотя спека требует `resolvedUnread === ALL_MESSAGES`. Потребителя у
   хелпера пока нет, поэтому не блокирую, но контракт и тест надо привести к спеке до интеграции.
5. **Сброс override не всегда шлёт `flags`.** `:1668-1670` добавляет `flags: PW(...)` только если
   поле было в записи; §1.4 п.3 и §1.6.3 показывают `flags` в теле всегда. Для 75 из 1328
   записей без `flags` расхождение безвредно, но это отклонение от буквы контракта и оно нигде не
   задокументировано.
6. **Проверка типов не покрывала новый модуль.** `rust/gui-spike/ui/tsconfig.json` содержит
   `"include": ["app.js"]`, а `app.js` не импортирует `notifications.js` (grep пуст), поэтому
   строка отчёта «`npx tsc -p rust/gui-spike/ui/tsconfig.json`: 0 ошибок» к новому файлу не
   относится. Я прогнал файл отдельным проектом с теми же опциями — `exit 0`, скрытых ошибок нет,
   но evidence в отчёте надо либо усилить (проект с `files: [.../notifications.js]`), либо
   оговорить. Дополнительно: `red.log` не содержит строки с exit code, хотя `TEST-POLICY §4`
   требует фиксировать «команда → exit code → артефакт»; exit 1 в отчёте подтверждается только
   счётом `fail 14`.
7. **Индикаторы пока не подключены к экранам.** `applyChannelIndicatorToDom`/`applyGuildRailIndicatorToDom`
   никем не вызываются, а скрытие канала делается атрибутом `hidden` (`:690-691`), тогда как
   §2.2/§3.2 спеки задают CSS-фильтр `#channel-tree[data-hide-muted="true"]`. Это ожидаемо для
   изолированного модуля (интеграция — отдельный лейн, отчёт это признаёт), но при подключении
   нужно согласовать способ скрытия, иначе канал останется видимым при `data-hide-muted`.

## Что нужно для PASS

1. Исправить P1 (`Array.from`/spread в `getEnabledChannelMenuItems`) и добавить red-тест на
   коллекцию без `filter`, затем повторный targeted-прогон.
2. Прогнать `tsc` по самому `notifications.js` (проект с явным `files`) и приложить вывод в отчёт.
3. Решить по P2.1–P2.3: либо реализовать focus trap, таймер истечения и `429`-countdown, либо
   письменно вынести их в отдельную карточку с указанием, кто закрывает (сейчас в отчёте они не
   упомянуты).
4. Привести `marksUnread` и assert `:503` к §1.5.3 строкой 4 либо зафиксировать отклонение в
   `DECISIONS.md`.

Источники вердикта: `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` §1–§5, `waves/RW-40/NOTIFICATIONS-RE.md`
§1–§12, `TEST-POLICY.md` §2–§7, `rust/gui-spike/ui/features/notifications.js`,
`tests/unit/notifications-ui.test.mjs`, `tests/support/gui-dom-harness.mjs`,
`rust/gui-spike/ui/features/actions.js`, `rust/gui-spike/src/lib.rs`,
`waves/RW-40/PM/n05-ui-notifications/{REPORT.md,red.log,green.log,mutations.log}`,
`fixtures/gateway/by-type/002-READY.json`.
