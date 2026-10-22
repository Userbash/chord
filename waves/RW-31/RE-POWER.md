# RE: питание, ингибиторы и idle (P6-06)

Дата: 2026-10-22. Источник — только чтение `/home/sanya/re-deep/backend-ipc.md`,
`main-arch.md`, `native-re.md`, `docs/re/architecture.md`.

## 1. Что делает клиент

IPC-каналы питания (`backend-ipc.md`, раздел «питание», 8 каналов):

| Канал | Смысл |
|---|---|
| `DISCORD_POWER_MONITOR_GET_SYSTEM_IDLE_TIME` | запрос времени бездействия системы |
| `DISCORD_POWER_MONITOR_SUSPEND` / `RESUME` | push: система уходит в сон / просыпается |
| `DISCORD_POWER_MONITOR_LOCK_SCREEN` / `UNLOCK_SCREEN` | push: блокировка/разблокировка |
| `POWER_SAVE_BLOCKER_BLOCK_DISPLAY_SLEEP` / `UNBLOCK_DISPLAY_SLEEP` / `CLEANUP_DISPLAY_SLEEP` | ингибитор гашения экрана |

Main (`main-arch.md` §10.1) держит `POWER_MONITOR_*` в `core #4531`; на Linux idle берётся из
`discord_utils.isWaylandIdleAvailable()/getWaylandSystemIdleTimeMs()` с fallback
(`core #4531 @82812`). `discord_utils` (19,2 МБ) содержит `WaylandIdleManager` и X11-idle
(`docs/re/architecture.md`, `native-re.md` §WaylandIdleManager, лог `Wayland idle manager shut down`).

## 2. Отображение на Linux

- Экран в звонке не гасится ингибитором: `org.freedesktop.ScreenSaver.Inhibit/UnInhibit`
  (сессионная шина) — универсальный freedesktop-путь, не требует нативного модуля.
- Сон/пробуждение: `org.freedesktop.login1.Manager.PrepareForSleep(b)` (системная шина);
  ингибитор сна `Inhibit("sleep", …)` возвращает **fd**, поэтому в этой карточке не
  реализуется (нужен SCM_RIGHTS; вынесено в бэклог).
- Idle-время: `ScreenSaver.GetSessionIdleTime` доступен в KDE; на других окружениях метод
  может отсутствовать — контракт возвращает `Option<u64>`, а не ошибку.
- Авто-статус: значения `online/idle/dnd` подтверждены в gateway-схеме
  (`docs/re/gateway-schema.md`: атомы `online`×312, `idle`×116, `dnd`×92). **Точные пороги**
  «idle через N минут» в RE не найдены — вынесены в `IdlePolicy` как конфигурация
  (DEFAULT 600/1800 помечен как продуктовое значение, не факт клиента).

## 3. Ограничения

- Wayland `ext-idle-notify-v1` и X11 `XScreenSaver` без клиентских библиотек не реализуются;
  D-Bus `GetSessionIdleTime` — единственный headless-путь.
- Live-проверка на рабочем столе — только skip-aware/по флагу, без реального гашения экрана.
- Wire-совместимость с нативным `discord_utils` не заявляется: это самостоятельная
  freedesktop-реализация.
