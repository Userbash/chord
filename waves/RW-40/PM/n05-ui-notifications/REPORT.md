# Отчёт: N-05-UI-NOTIFICATIONS (Pass 2) — настройки уведомлений сервера и каналов (G2 Frontend)

- **batch/карточка:** `RW-40` / `N-05-UI-NOTIFICATIONS-PASS2`, отдел **G2 Frontend**.
- **контракт-оракул:** `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (§1–§6), `waves/RW-40/NOTIFICATIONS-RE.md` (§1–§12), `fixtures/gateway/by-type/002-READY.json` (`d.user_guild_settings`: 58 записей, 1328 `channel_overrides` у 52 записей), `waves/RW-40/PM/n05-ui-notifications/REVIEW.md` (DeepSeek Review 31A).
- **границы файлов:** изменены исключительно разрешённые файлы `rust/gui-spike/ui/features/notifications.js`, `tests/unit/notifications-ui.test.mjs` и артефакты в `waves/RW-40/PM/n05-ui-notifications/{REPORT.md,red.log,green.log,mutations.log}`.

## 1. Таблица закрытия замечаний ревью (`REVIEW.md`: 1 P1 + 7 P2)

| ID | Замечание из `REVIEW.md` | Как закрыто в `notifications.js` и `notifications-ui.test.mjs` | Статус |
|---|---|---|---|
| **P1.1** | `NodeList.filter` в `getEnabledChannelMenuItems` вызывает `TypeError` в браузерном webview | В `getEnabledChannelMenuItems` используется `Array.from(channelMenu.querySelectorAll('[role="menuitem"], [role="menuitemradio"]')).filter(...)`. В тест меню добавлена подмена `menu.querySelectorAll` через `Object.defineProperty`, возвращающая итерируемый `NodeList`-подобный объект **без** метода `.filter`, с проверкой `openChannelMenu` (`tabindex="0"`, фокус), `ArrowDown` и закрытия по `Escape` с возвратом фокуса на триггер. | **CLOSED** |
| **P2.1** | Focus trap модалки `#guild-notif-modal` не реализован (`NOTIFICATIONS-SPEC.md §5.2 п.4`) | Реализованы `getModalFocusableElements()` и обработка `Tab` / `Shift+Tab` в `handleModalKeydown` с циклическим переводом фокуса между видимыми активными контролами внутри `#guild-notif-modal` (пропуская скрытые элементы и `tabindex="-1"`). Добавлен поведенческий тест на прямой/обратный цикл `Tab` и `Shift+Tab`. | **CLOSED** |
| **P2.2** | Нет таймера истечения временного mute (`NOTIFICATIONS-SPEC.md §1.3.2 п.2`) | Реализованы `scheduleMuteExpiryTimers()` и `clearMuteExpiryTimers()`: для каждого активного временного mute сервера и каналов с `end_time` ставится `setTimeout` на `max(0, endMs - nowMs)`, по срабатыванию которого локально сбрасывается `muted = false`, `mute_config = null` и перерисовывается UI (`renderModal`/`renderChannelMenu`). При закрытии/уничтожении таймеры очищаются (`clearTimeout`). Покрыто unit-тестом. | **CLOSED** |
| **P2.3** | Нет обработки `429 retry_after` (`NOTIFICATIONS-SPEC.md §4.1`) | Реализованы `isRateLimitedError`, `extractRetryAfterSeconds`, `startRateLimitCooldown` и `stopRateLimitCooldown`: при ошибке `429` контролы модалки переводятся в `disabled` (`aria-disabled="true"`), повторные запросы блокируются на время обратного отсчёта, в `#guild-notif-error` выводится посекундное сообщение `«Слишком много запросов. Повторите через {n} с.»`, а по достижении `0` блокировка снимается и модалка возвращается в `ready`. Покрыто unit-тестом. | **CLOSED** |
| **P2.4** | `marksUnread` расходится с `NOTIFICATIONS-SPEC.md §1.5.3` строкой 4 | В `resolveEffectiveNotifications` добавлено разрешение `resolvedUnread` по битам `UNREADS_ALL_MESSAGES` / `UNREADS_ONLY_MENTIONS` (канал → категория → гильдия → `resolvedMessageNotifications`). В `evaluateIncomingMessageNotification` для обычного сообщения без упоминаний `marksUnread === true` только при `!isChannelMuted && !isCategoryMuted && !isOptInSuppressed && resolvedUnread === ALL_MESSAGES`. Обновлён assert в тесте (`plainAtMentions.marksUnread === false`, `plainAtAll.marksUnread === true`, плюс тесты на биты `1024` и `512`). | **CLOSED** |
| **P2.5** | Сброс override не всегда шлёт `flags` (`NOTIFICATIONS-SPEC.md §1.4 п.3, §1.6.3`) | Обработчик `.notif-override-remove` теперь всегда включает `flags: clearUnreadFlags(Number(ov.flags ?? 0), CHANNEL_UNREAD_MASK)` в `resetPatch`, даже если в исходной записи `channel_overrides` ключ `flags` отсутствовал. Добавлен тест на запись без ключа `flags` (`flags: 0` в теле PATCH). | **CLOSED** |
| **P2.6** | Проверка типов не покрывала сам `notifications.js` отдельно, а `red.log` не содержал строку `EXIT_CODE` | Выполнен прямой прогон `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target es2022 --module es2022 --moduleResolution bundler rust/gui-spike/ui/features/notifications.js` (`exit 0`), а также `npx tsc -p tsconfig.json` (`exit 0`) и `npx tsc -p rust/gui-spike/ui/tsconfig.json` (`exit 0`). В `red.log` записана явная строка `EXIT_CODE=1`, в `green.log` — `EXIT_CODE=0`. | **CLOSED** |
| **P2.7** | Согласование скрытия канала в `applyChannelIndicatorToDom` с `#channel-tree[data-hide-muted="true"]` (`§2.2, §3.2`) | `applyChannelIndicatorToDom` проверяет атрибут `data-hide-muted` у родительского `#channel-tree` (наряду с `state.hiddenByHideMuted`), учитывает `aria-current="page"` и `hasMention`, выставляет `data-muted="true"\|"false"`, `data-muted-hidden="true"\|"false"` и синхронно управляет атрибутом `hidden`. Покрыто unit-тестом. | **CLOSED** |

## 2. RED-прогон (Pass 2)

```bash
(node --test tests/unit/notifications-ui.test.mjs; ec=$?; echo "EXIT_CODE=$ec"; exit $ec) > waves/RW-40/PM/n05-ui-notifications/red.log 2>&1
```

- **Результат:** `EXIT_CODE=1` (`tests 17 | pass 10 | fail 7`) — все 7 новых/обновлённых проверок по P1.1, P2.1, P2.2, P2.3, P2.4, P2.5, P2.7 упали до внесения правок в `notifications.js`.
- **Лог:** `waves/RW-40/PM/n05-ui-notifications/red.log` (md5 `d3147eae023ba2a2b1e1e7c77196cb04`).

## 3. GREEN-прогон и проверка типов (`green.log`)

```bash
node --test tests/unit/notifications-ui.test.mjs
node --test tests/meta/test-policy.test.mjs
npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target es2022 --module es2022 --moduleResolution bundler rust/gui-spike/ui/features/notifications.js
npx tsc -p rust/gui-spike/ui/tsconfig.json
npx tsc -p tsconfig.json
```

- `node --test tests/unit/notifications-ui.test.mjs`: **17 passed, 0 failed, 0 skipped** (`UNIT_EXIT=0`).
- `node --test tests/meta/test-policy.test.mjs`: **17 passed, 0 failed** (`META_EXIT=0`).
- `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target es2022 --module es2022 --moduleResolution bundler rust/gui-spike/ui/features/notifications.js`: **0 ошибок** (`TSC_EXPLICIT_EXIT=0`).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json`: **0 ошибок** (`TSC_UI_EXIT=0`).
- `npx tsc -p tsconfig.json`: **0 ошибок** (`TSC_ROOT_EXIT=0`).
- **Лог:** `waves/RW-40/PM/n05-ui-notifications/green.log` (md5 `f0705fc9a1843749b681597e90a25310`).

## 4. Мутационное тестирование (`mutations.log`)

Baseline MD5 `rust/gui-spike/ui/features/notifications.js`: `001ff70a7538378d16d52fd3b6b10117`.
Все **5/5** поведенческих мутаций на финальном файле убиты (`KILLED`), после каждой выполнен restore и сверен baseline MD5 (`match=true`, `Post-restore test exit code: 0`):

| ID | Описание мутации | MD5 мутанта | Exit | Вердикт |
|---|---|---|---:|---|
| `MUT-1-channel-menu-nodelist-filter` | `getEnabledChannelMenuItems` вызывает `.filter` напрямую на результате `querySelectorAll` без `Array.from` (P1.1) | `c76e6a38e8f76056f1e0135aa65854e7` | `1` | **KILLED** |
| `MUT-2-modal-drop-focus-trap` | `handleModalKeydown` игнорирует циклический перехват `Tab`/`Shift+Tab` внутри `#guild-notif-modal` (P2.1) | `bb54096fe0a0eae4f8a884c0fd7b5046` | `1` | **KILLED** |
| `MUT-3-skip-mute-expiry-timer` | `scheduleMuteExpiryTimers` не планирует `setTimeout` при активном временном mute (P2.2) | `ed615263fd3064c9eb71c686e1779afc` | `1` | **KILLED** |
| `MUT-4-ignore-429-rate-limit` | `isRateLimitedError` возвращает `false` на статус `429`, пропуская блокировку `disabled` и отсчёт `retry_after` (P2.3) | `07b0b8b8382af39969a6c231e0e2ca34` | `1` | **KILLED** |
| `MUT-5-marks-unread-ignores-resolved-unread` | `evaluateIncomingMessageNotification` помечает обычное сообщение непрочитанным даже при `ONLY_MENTIONS` (P2.4) | `50073573ceaf022be89d547f8fe8e797` | `1` | **KILLED** |

## 5. Файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/notifications.js` | modified | `001ff70a7538378d16d52fd3b6b10117` |
| `tests/unit/notifications-ui.test.mjs` | modified | `22238cac9b6e56b5ba9aa3f33c4484f7` |
| `waves/RW-40/PM/n05-ui-notifications/red.log` | modified | `d3147eae023ba2a2b1e1e7c77196cb04` |
| `waves/RW-40/PM/n05-ui-notifications/green.log` | modified | `f0705fc9a1843749b681597e90a25310` |
| `waves/RW-40/PM/n05-ui-notifications/mutations.log` | modified | `ec2097df0a7cc71f1333bfc61854c891` |

## 6. Риски и открытые границы

1. **Изоляция модуля:** согласно ТЗ карточки файлы `app.js`, `chat.js`, `index.html` и `TDD-PLAN.md` не изменялись; подключение `createNotificationSettingsController`, `applyChannelIndicatorToDom` и `applyGuildRailIndicatorToDom` к главному экрану и `#server-menu` выполняется в интеграционном лейне.
2. **Гейт уровня batch:** по правилам `AGENTS.md §3` запущены targeted-тесты, `test-policy.test.mjs` и `tsc`; полный `npm run gate:worker` и `cargo` в параллельном воркере не запускались.
