# RE: уведомления Discord на Linux (P6-02)

Дата: 2026-10-22. Источники — только чтение: `/home/sanya/re-deep/main-arch.md`,
`backend-ipc.md`, `native-re.md`, asar-extract.

## 1. Как это устроено в клиенте

Цепочка A (`main-arch.md` §3.2):

```
renderer --invoke--> DISCORD_NOTIFICATIONS_SEND_NOTIFICATION
  -> main: core #7592 handler -> requireLoadedLib().sendNotification(options)
  -> native: discord_notifications (setDataPath/setCallbacks)
  -> ответ normalizeSendResult
  -> асинхронно: callback(action, identifier, userText, fallbackDeepLink)
     -> sendToAllWindows(DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE)
```

**Ключевой факт для Linux:** ветка загрузки модуля выполняется только для macOS (`isOSX &&
kernelMajor >= 21`) или Windows (`Notification.isSupported()`). На Linux `moduleStatus =
unsupportedStatus(process.platform)`, `sendNotification` всегда бросает «module unavailable»,
а фича `notifications` не объявляется (`main-arch.md` §3.2, §6.2).

Следствие: официальный клиент на Linux **не показывает системные уведомления** штатным
модулем. Для нашего Rust-клиента это означает собственную реализацию
`org.freedesktop.Notifications` (D-Bus), а не перенос нативного модуля.

## 2. Что нужно воспроизвести по смыслу

- `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION` принимает `options` (заголовок/тело/иконка/
  канал/сообщение) и возвращает нормализованный `{ok, id}`; асинхронный callback несёт
  `action`, `identifier`, `userText`, `fallbackDeepLink`.
- Действия уведомления: «открыть» (deep link на сообщение/канал) и «ответить» (inline-ответ),
  плюс настройки `badge/banner/list/sound` (`main-arch.md` §3.2, `setCallbacks`).
- DND/«не беспокоить» — на уровне настроек клиента, не нативного модуля.

## 3. Решение для P6-02 (headless)

- Реализуем **свой D-Bus клиент** (std, без `zbus`/`notify-rust`): auth EXTERNAL, `Hello`,
  method call/reply, `AddMatch` + сигналы.
- Интерфейс `org.freedesktop.Notifications`:
  - `Notify(susssasa{sv}i) -> u`; `actions` — `a(ss)`; `hints` — пустой `a{sv}` в этой карточке
    (полный набор хинтов — отдельным расширением, когда появится UI-потребитель);
  - `CloseNotification(u)`, `GetCapabilities() -> as`, `GetServerInformation() -> ssss`;
  - сигналы `ActionInvoked(us)`, `NotificationClosed(uu)`.
- DND/группировка/история — уровень приложения (P6-01/P3), не этой карточки.
- Wire-совместимость с Electron-модулем Discord **не заявляется**: модуль на Linux не работает,
  наша реализация — самостоятельный D-Bus клиент по спецификации freedesktop.

## 4. Проверка

Оракул — независимая реализация на libdbus (`python3-dbus`): приватный `dbus-daemon` +
мок-сервис, который отвечает на вызовы и эмитит сигналы. Rust-тесты поднимают эту пару и
сверяют поведение; при отсутствии `dbus-daemon`/`python3-dbus` — skip с причиной.
Live-уведомление на рабочем столе не отправляется без `DISCORD_LAB_LIVE_NOTIFY=1`.
