# Независимое ревью Pass 2 — N-05-UI-NOTIFICATIONS

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Ветка: `rw40/review-n05-ui-pass2`. Ревью read-only: продуктовый код, тесты, спека и логи
не изменялись. Записан только этот файл.

## Вердикт

**PASS. P0 = 0, P1 = 0, P2 = 6 (не блокируют).**

Все восемь замечаний `REVIEW.md` (1 P1 + 7 P2) закрыты по существу, а не по формулировке.
Проверял не только текст правок, но и поведение: прогнал `node --test` на тесте карточки
(17/17) и на мета-тесте (17/17), прогнал `tsc` по самому `notifications.js` (`exit 0`),
воспроизвёл мутанта MUT-1 побайтово (`md5 c76e6a38…`, тест падает), а также отдельно
воспроизвёл откат `marksUnread` (тест падает на нужном ассерте). MD5 всех файлов и логов
совпадают с `REPORT.md` и `mutations.log`. Правки в `notifications.js` ограничены 7 темами
диффа (26 удалённых строк), постороннего кода и ослаблений тестов нет.

## Закрытие замечаний

| ID | Что проверено | Итог |
|---|---|---|
| **P1.1** | `getEnabledChannelMenuItems` (`notifications.js:2130-2137`) оборачивает `querySelectorAll` в `Array.from(...)` до `.filter`. Тест (`tests/unit/notifications-ui.test.mjs:1036-1063`) подменяет `menu.querySelectorAll` через `Object.defineProperty` на итерируемый объект без `.filter`, проверяет `typeof …filter === "undefined"`, затем прогоняет `openChannelMenu` (`tabindex="0"` + фокус), `ArrowDown` и `Escape` с возвратом фокуса на триггер. | **CLOSED** |
| **P2.1** | Focus trap: `isElementHiddenInModal` / `getModalFocusableElements` / `handleModalKeydown` (`:1919-1985`) циклически переводят `Tab`/`Shift+Tab` по видимым контролам внутри `#guild-notif-modal`, пропуская `hidden`, `disabled`, `aria-disabled="true"` и `tabindex="-1"`. `Escape` сохранён (`:1955-1962`). Обработчик навешен на модалку и 5 статических контролов (`:2111-2121`), динамические строки таблицы пробрасывают событие (`:1836`, `:1873`, `:1896`). Тест: `:1121-1168` (вперёд, с последнего на первый, назад). | **CLOSED** |
| **P2.2** | Таймер истечения: `scheduleMuteExpiryTimers` / `clearMuteExpiryTimers` (`:1355-1414`) ставят `setTimeout` на `max(0, remainingMs)` для гильдии и каждого канала с активным временным mute, локально сбрасывают `muted=false`, `mute_config=null`, перерисовывают модалку и меню, объявляют статус. Планирование — на каждом `renderOverrideTable` (`:1908`) и `renderChannelMenu` (`:2202`), очистка — при закрытии и `destroy` (`:2353`, `:2391`, `:2421`). Тест: `:1170-1262` (обе задержки, срабатывание, `aria-checked`, скрытие строки окна). | **CLOSED** |
| **P2.3** | 429: `isRateLimitedError` / `extractRetryAfterSeconds` / `setModalControlsDisabled` / `startRateLimitCooldown` / `stopRateLimitCooldown` (`:1421-1509`). Контролы получают `disabled` + `aria-disabled`, повторные submit блокируются (`:1540`, `:1594`), текст — «Слишком много запросов. Повторите через {n} с.», после нуля — `ready` и ровно один ручной повтор. Тест: `:1264-1353` (3 с, тики 2/1/0, блокировка повтора, успешный повтор). Дополнительно сверил форму ошибки: `NotificationError` в `rust/gui-spike/src/notifications.rs:772-795` сериализуется как `{kind:"rateLimited", retry_after_ms}`, и фронт читает именно эти поля — обработка не «тестовая». | **CLOSED** |
| **P2.4** | `resolvedUnread` разрешается по битам канал → категория → гильдия → уровень (`:490-506`), `marksUnread` для обычного сообщения требует `resolvedUnread === ALL_MESSAGES` (`:593-599`). Тест: `:504-560` — ONLY_MENTIONS → `false`, ALL_MESSAGES → `true`, бит `1024` при `message_notifications=1` → `true`, бит `512` при `0` → `false`. | **CLOSED** |
| **P2.5** | Сброс override всегда кладёт `flags: clearUnreadFlags(Number(ov.flags ?? 0), CHANNEL_UNREAD_MASK)` в тело (`:1887-1895`). Тест: `:992-1035` — запись без ключа `flags` даёт `{message_notifications:3, muted:false, flags:0}`. | **CLOSED** |
| **P2.6** | Прогнал сам: `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target es2022 --module es2022 --moduleResolution bundler rust/gui-spike/ui/features/notifications.js` → `exit 0`; `npx tsc -p rust/gui-spike/ui/tsconfig.json` → `exit 0`. В `red.log:172` есть `EXIT_CODE=1`, в `green.log` — `UNIT_EXIT=0`, `META_EXIT=0`, три `TSC_*_EXIT=0`, `EXIT_CODE=0`. | **CLOSED** |
| **P2.7** | `applyChannelIndicatorToDom` (`:714-735`) ищет `#channel-tree[data-hide-muted]`, выставляет `data-muted`, `data-muted-hidden` и синхронно `hidden`, оставляя видимыми каналы с `aria-current="page"` и с упоминаниями. Тест: `:693-737` (attr `true`, `aria-current`, attr `false`). | **CLOSED** |

## Evidence: что проверил сам

- **md5 совпадают с `REPORT.md`:** `notifications.js` `001ff70a7538378d16d52fd3b6b10117`,
  тест `22238cac9b6e56b5ba9aa3f33c4484f7`, `red.log` `d3147eae023ba2a2b1e1e7c77196cb04`,
  `green.log` `f0705fc9a1843749b681597e90a25310`, `mutations.log` `ec2097df0a7cc71f1333bfc61854c891`.
  Baseline в `mutations.log` совпадает с текущим файлом и с §5 отчёта.
- **Прогоны:** `node --test tests/unit/notifications-ui.test.mjs` → 17 passed, 0 failed, 0 skipped;
  `node --test tests/meta/test-policy.test.mjs` → 17 passed. Плюс `tsc` (см. P2.6).
- **RED честный:** `red.log` — 7 падений ровно по P1.1/P2.1/P2.2/P2.3/P2.4/P2.5/P2.7,
  ссылки `415:3, 628:3, 895:3, 1030:3, 1121:3, 1170:3, 1264:3` совпадают с определениями
  текущих тестов построчно. Падения по существу: `filter is not a function` на старом
  `:1833`, `false !== true` на focus trap, `actual: undefined` для таймера, `actual: "Rate limited"`
  вместо «Повторите через 3 с.», `expected {…, flags: 0}` при сбросе.
- **Мутации:** MUT-1 воспроизвёл независимо на копии в `/tmp` — md5 мутанта совпал
  (`c76e6a38e8f76056f1e0135aa65854e7`), тест упал с тем же `TypeError`; откат `marksUnread`
  к старой формуле тоже убивается нужным ассертом. Остальные три мутанта проверены по
  текстам падений в логе: они ссылаются на реальные ассерты теста, не на побочные эффекты.
- **Дифф против staging (страховка от посторонних правок):** в `notifications.js` 26 удалённых
  строк — только заменяемые фрагменты семи фиксов, добавлений вне тем нет. В тесте удалений
  ассертов нет вообще: добавлены 4 новых теста, один `.filter` в самом тесте заменён на
  `Array.from(...).filter`, переименован заголовок теста меню.
- **Ослаблений нет:** в двух файлах 0 `.only`, 0 `.skip`/`todo`, 0 `@ts-ignore`, 0 `assert.ok(true)`.
  Строка §8 для теста в `TDD-PLAN.md:381` на месте.

## P2 (не блокируют)

1. **429-cooldown сбрасывается закрытием модалки.** `closeGuildModal` вызывает
   `stopRateLimitCooldown` (`:2353`), `openGuildModal` — тоже (`:2338`). Проверил пробой:
   `Escape` во время отсчёта закрывает модалку, повторное открытие даёт `data-state="ready"`,
   и следующий клик снова уходит в IPC (`calls` = 2). Формально §4.1 требует держать контрол
   `disabled` на время `retry_after_ms`; обход требует ручного закрытия/открытия, поэтому не
   блокирую, но «ровно один ручной повтор» превращается в «один на каждое открытие». Либо
   держать дедлайн в области контроллера, либо записать это решение в `DECISIONS.md`.
2. **Отчёт переоценивает покрытие очистки таймеров.** В `REPORT.md` §1 (P2.2) сказано
   «Покрыто unit-тестом», но тест вызывает `controller.destroy()` (`test:1259-1261`) без
   ассерта на `clearTimeout`, а путь `closeGuildModal`/`closeChannelMenu` не покрыт вовсе.
   Код очистки есть и выглядит верным; не хватает двух строк проверки
   (`scheduled.every(t => t.cleared)`) или правки формулировки отчёта.
3. **`marksUnread` строже буквы §1.5.3.** В строке 4 добавлено `!isOptInSuppressed`, а в
   строках 2–3 к `!isChannelMuted` добавлен `!isCategoryMuted`. Прежнее поведение было дальше
   от спеки, так что это не регресс, но отклонение нигде не зафиксировано. Дописать в
   `DECISIONS.md` или убрать лишние условия.
4. **Состояние staging перед слиянием.** В `staging-rw-40` лежит промежуточный тест
   (`md5 46f56c5a…`: уже с ассертом `marksUnread === false`, но без 6 новых тестов) и старый
   продукт `e9a4e8b8…`; `node --test` там сейчас красный (1 fail). Слияние должно перенести
   оба файла одной парой (`001ff70a…` / `22238cac…`), иначе ветка останется красной.
5. **Два механизма скрытия канала.** `applyChannelIndicatorToDom` пишет `hidden`, тогда как
   §3.2 задаёт CSS-фильтр `#channel-tree[data-hide-muted="true"] .channel-item.is-muted:not([aria-current="page"]):not(.has-mention)`
   (спека, строка 1245). Логика дублируется в JS и в CSS — при интеграции нужно оставить один
   источник истины, иначе рассинхрон при смене `aria-current` без перерисовки.
6. **Атрибут `data-hide-muted` пока никем не выставляется** (`index.html:54` его не содержит,
   `actions.js:511` читает только `data-hide-muted-channels` у меню). Тест создаёт дерево
   вручную, поэтому «совместимость» проверена на синтетике; живая связка появится в
   интеграционном лейне — там и нужно закрыть `data-hide-muted` ↔ `hide_muted_channels`.

## Источники вердикта

`waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (§1.3.2, §1.4, §1.5.3, §2.2, §3.2, §4.1, §5.2),
`waves/RW-40/PM/n05-ui-notifications/{REVIEW.md,REPORT.md,red.log,green.log,mutations.log}`,
`rust/gui-spike/ui/features/notifications.js`, `tests/unit/notifications-ui.test.mjs`,
`tests/support/gui-dom-harness.mjs`, `rust/gui-spike/src/notifications.rs`,
`rust/gui-spike/src/lib.rs`, `waves/RW-40/DESIGN/G05-REST-SPEC.md` §3.0, `TEST-POLICY.md`.
