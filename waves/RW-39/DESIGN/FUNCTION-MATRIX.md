# FUNCTION-MATRIX — UI → backend → ядро (RW-39)

Дата: 2026-10-23. PM: DeepSeek. Назначение: зафиксировать, какая часть интерфейса какой
функцией backend/ядра должна управлять, и где нет тестов.

Обозначения: `[S]` — сейчас работает на статике/фикстурах; `[B]` — есть backend-команда;
`[C]` — цель в ядре; `[–]` — заглушка/disabled; `[G]` — пробел (нет ни реализации, ни теста).

## 1. Слои

```
UI control (index.html)
  └─ features/*.js (handler)
      └─ Tauri invoke (main.rs)          ← сейчас UI ещё не вызывает invoke
          └─ src/data.rs (BFF)           ← читает fixtures/
              └─ core adapter (W39j)     ← ЦЕЛЬ: discord-core / discord-store / discord-ui
```

## 2. Backend (BFF) → ядро

| BFF fn | Что отдаёт | Должен вызывать в ядре | Тесты |
|---|---|---|---|
| `resolve_root` / `fixtures_root` | путь к данным | конфиг приложения (не ядро) | data_contract (косвенно) |
| `ready` | user/guilds/counts | `discord-core::ready::ReadySummary` + `state::apply_dispatch(READY/GUILD_CREATE)` | 27 ссылок |
| `messages_page` | строки канала + cursor | `history::History::{apply_page, page}` + `state::selectors::messages_in_channel` | 23 |
| `members_page` | участники/роли/группы | `state::member_list::MemberList` + `member_chunk` + `selectors::guild_member` | 12 |
| `search_page` | результаты + фильтры | `discord-ui::search_overlay` (модель) + REST `GET /channels/{id}/messages/search` | 32 |
| `settings` | секции/тумблеры | `settings::decode_preloaded_user_settings` / `decode_frecency_user_settings` | 10 |
| `DataError::{kind,io,parse,not_found,invalid_arg}` | типизированные ошибки | маппинг `discord-core::rest::RestError`/`HistoryError` | 41/3/1/1/1 |

## 3. UI → backend → ядро

### 3.1 Shell и навигация

| UI control | Handler (features) | Tauri | Ядро | Статус |
|---|---|---|---|---|
| `.rail-btn[data-screen]` ×4 | `screens.createScreenRouter` | — | `discord-ui::navigation` | `[S]` |
| `.guild-item` ×3 | aria-current | — | `state::selectors::guild` | `[S]` |
| `.group-toggle` ×2 | `actions` toggle | — | UI-only | `[S]` |
| `.channel-item` | selection | — | `state::selectors::channel` | `[S]` |
| `[data-action=toggle-tree]` | `actions` | — | UI-only | `[S]` |
| `[data-action=inbox]` (новый) | `actions` | — | `state::read_state` (unread) | `[–]` |
| `#chat-search` | `search-overlay` | `search_page` | `discord-ui::search_overlay` + REST | `[B]` `[S]` |

### 3.2 Чат

| UI control | Handler | Tauri | Ядро | Статус |
|---|---|---|---|---|
| `#chat-list` scroll | `virtual-list`, `model.PageStore` | `messages_page` | `history::History` + `selectors` | `[S]` |
| строка сообщения | `features/chat`, `rows`, `embeds`, `reactions` | `messages_page` | `history` + `state::selectors::message` | `[S]` |
| hover-панель | `message-toolbar` | — | `discord-ui::chat_view` | `[S]`/`[–]` |
| reply-quote | `chat` | — | `discord-ui::reply_thread` | `[–]` |
| reactions | `reactions` | — | `state` message.reactions | `[–]` |
| day/unread divider | `chat` | `messages_page` (day) | `history` (день) | `[S]` |
| jump-pill | `chat` | — | UI-only | `[S]` |

### 3.3 Композер

| UI control | Handler | Tauri | Ядро | Статус |
|---|---|---|---|---|
| `#composer-input` | `composer` | — | `history::apply_live` (эхо) | `[S]` |
| Enter / Shift+Enter / Ctrl+Enter | `composer` | — | REST `POST /channels/{id}/messages` (нет маршрута в UI) | `[–]` |
| `[data-action=attach]` | `actions` | — | REST upload | `[–]` |
| `[data-action=emoji]` | `actions` | — | emoji contract (нет модуля ядра) | `[G]` |
| `[data-action=gift]` | `actions` | — | REST gifts | `[–]` |
| submit | — | — | REST send | `[–]` |

### 3.4 Эмодзи

| UI control | Handler | Tauri | Ядро | Статус |
|---|---|---|---|---|
| `#emoji-tabs` ×10 | `emoji` | — | `discord-ui` emoji contract (нет модуля) | `[G]` |
| `#emoji-search` глобальный | `emoji` | — | emoji model + custom emoji cache | `[G]` |
| ячейка | `emoji` | — | insertion в composer | `[S]` |
| close | `actions`/`emoji` | — | UI-only | `[S]` |

### 3.5 Участники

| UI control | Handler | Tauri | Ядро | Статус |
|---|---|---|---|---|
| список/скролл | `members`, `virtual-list` | `members_page` | `state::member_list` | `[S]` |
| поиск | `members` | `members_page`/local | `member_list` filter | `[S]` |
| группы (5–6) | `members` | `members_page` | `member_list::groups` | `[S]` |
| «Показать всех» | `actions` | — | navigation | `[S]` |
| профиль | `actions` | — | `discord-ui::user_profile` | `[–]` |
| mute/deafen | `actions` | — | `state::voice` + `discord-voice` | `[–]` |

### 3.6 Настройки

| UI control | Handler | Tauri | Ядро | Статус |
|---|---|---|---|---|
| 38 тумблеров | `settings` | `settings` | `settings::decode_*` | `[S]` `[B]` |
| `#settings-nav` (18) | `settings` | `settings(section)` | `discord-ui` settings state (нет модуля) | `[G]` |
| close | `actions` | — | UI-only | `[S]` |

### 3.7 Клавиши и системное

| UI control | Handler | Tauri | Ядро | Статус |
|---|---|---|---|---|
| Ctrl+F / Ctrl+U / PageUp/Down / Shift+Esc | `actions` | — | UI-only | `[S]` |
| Ctrl+K QuickSwitcher | `actions` | — | `discord-ui::navigation` | `[–]` |
| уведомления (колокол) | — | — | `discord-platform` D-Bus + `read_state` | `[G]` |
| tray / window / dialog | — | — | `discord-ui::{tray_manager,window_manager,dialog_window}` | `[G]` |
| i18n форматы | — | — | `discord-i18n` | `[G]` |
| updater | — | — | `discord-updater` | `[G]` |
| persistence | — | — | `discord-store` (`put_message/messages/kv_*`) | `[G]` |

## 4. Итог

- **Backend** покрыт: 12/12 публичных функций имеют прямые ссылки в тестах.
- **Ядро**: 265 публичных функций в `discord-core`, 28 без прямых тестов; `discord-ui` —
  112/20; `discord-voice` — 84/3 (в т.ч. `srtp_key_derive`).
- **Самый большой разрыв**: UI пока не вызывает Tauri-команды (`invoke`), поэтому связка
  «кнопка → backend → ядро» на живом окне не проверена. Это W39j + E9.
EOF