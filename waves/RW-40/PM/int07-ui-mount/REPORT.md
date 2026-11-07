# Отчёт: INT-07-UI-APP-MOUNT — подключение контроллеров голосовых устройств (V-07) и уведомлений (N-05) в `app.js` (G2 Frontend)

- **Batch / карточка:** `RW-40` / `INT-07-UI-APP-MOUNT`, отдел **G2 Frontend**.
- **Ветка / HEAD:** `rw40/int07-ui-app-mount` / `4e24048`.
- **Контракты и оракулы:**
  - `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (`V-07-UI-VOICE-DEVICES`, `rust/gui-spike/ui/features/voice-devices-settings.js`: `createVoiceDevicesSettings`);
  - `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (`N-05-UI-NOTIFICATIONS`, `rust/gui-spike/ui/features/notifications.js`: `createNotificationSettingsController`);
  - `waves/RW-40/PARITY/MERGE-READINESS-13.md` (§1 п.2, §3.5: монтирование `voice-devices-settings.js` и `notifications.js` в `rust/gui-spike/ui/app.js`).
- **Границы файлов:** изменены только разрешённые файлы `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/index.html`, `tests/unit/gui-spike-features.test.mjs` и артефакты в `waves/RW-40/PM/int07-ui-mount/`.

---

## 1. Сделано

1. **Подключение `createVoiceDevicesSettings` в модалку настроек (`#settings-dialog` / `#settings-modal` > `#voice-devices`):**
   - В `rust/gui-spike/ui/app.js` импортирован `createVoiceDevicesSettings` из `./features/voice-devices-settings.js` и объявлен дефолтный набор входного и выходного аудио-устройств `DEFAULT_VOICE_DEVICES` (`default-input`, `default-output`, `isDefault: true`).
   - В `boot()` оболочка настроек `.settings-shell` получает `id="settings-dialog"`, внутри `.settings-body` создаётся/используется панель `#settings-modal.settings-voice-panel` с секцией `#voice-devices` (вне контейнера `#settings`, чтобы сохранить строгий инвариант 38 чекбоксов и 3 `h3`-групп RW-38 в `#settings`).
   - Экземпляр `voiceDevices` монтируется в `#voice-devices` и экспортируется в `currentApp().voiceDevices`.
2. **Подключение `createNotificationSettingsController` и привязка к состоянию текущей гильдии и каналов (`rail` / `tree` / `chat`):**
   - В `rust/gui-spike/ui/app.js` импортирован и инициализирован `createNotificationSettingsController({ document, invoke })`; экземпляр экспортирован в `currentApp().notifications`.
   - Для сохранения инварианта `all(app, ".toggle").length === 38` и `all(app, ".toggle-input").length === 38` в `gui-spike-dom.test.mjs` и `gui-spike-features.test.mjs`, переключатели внутри `#guild-notifications-dialog` переведены на классы `.notif-toggle` / `.notif-toggle-input`.
   - Пункт меню сервера «Параметры уведомлений» (`#server-menu [data-menu-index="3"]`) получил `id="server-menu-notifications"` и обработчик клика `syncNotificationsWithGuild`, который передаёт в `notifications.openGuildModal(...)` актуальные `guildId` и `guildName` выбранной в `rail` гильдии, сохранённый `guildEntry` этой гильдии и список каналов `tree.channels`.
   - В `createChannelTree` добавлен колбэк `onContextMenu` на `.channel-item`, а на кнопку колокола шапки чата `[data-action="inbox"]` — обработчик клика, открывающие `#channel-notif-menu` (`openChannelNotifications`) для выбранного или активного в `chat.channelId` канала с предварительной синхронизацией контекста гильдии и `tree.channels`.
3. **Разметка `rust/gui-spike/ui/index.html`:**
   - Синхронизированы host-узлы `#settings-dialog`, `#settings-modal`, `#voice-devices`, `#guild-notifications-dialog` и `#channel-notif-menu` с DOM-шимом (тест соответствия `index.html` и шима в `gui-spike-dom.test.mjs` проходит без расхождений).

---

## 2. RED-прогон

- **Команда:**
  ```bash
  node --test tests/unit/gui-spike-features.test.mjs
  ```
- **Код возврата:** `EXIT_CODE=1`
- **Результат:** `tests 102 | pass 100 | fail 2` — оба новых поведенческих теста в блоке `INT-07-UI-APP-MOUNT` упали до реализации подключения в `app.js`:
  - `AssertionError [ERR_ASSERTION]: currentApp().voiceDevices не инициализирован`
  - `AssertionError [ERR_ASSERTION]: currentApp().notifications не инициализирован`
- **Артефакт:** `waves/RW-40/PM/int07-ui-mount/red.log` (MD5: `de60d8d22d00135eb2f167d658d31f72`).

---

## 3. GREEN-прогон и targeted-проверки

- **Команды (`waves/RW-40/PM/int07-ui-mount/green.log`):**
  ```bash
  node --test tests/unit/gui-spike-features.test.mjs
  npx tsc -p rust/gui-spike/ui/tsconfig.json
  npx tsc -p tsconfig.json
  ```
- **Результаты:**
  - `node --test tests/unit/gui-spike-features.test.mjs`: **102 passed, 0 failed, 0 skipped** (`TEST_EXIT_CODE=0`).
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: **0 ошибок** (`UI_TSC_EXIT_CODE=0`).
  - `npx tsc -p tsconfig.json`: **0 ошибок** (`ROOT_TSC_EXIT_CODE=0`).
  - Смежные наборы (`gui-spike-dom.test.mjs`, `server-menu.test.mjs`, `voice-devices-settings.test.mjs`, `test-policy.test.mjs`): **100 passed, 0 failed, 0 skipped** (`exit 0`).
- **Артефакт:** `waves/RW-40/PM/int07-ui-mount/green.log` (MD5: `0f5d224569f9f7072a8c5c29023db180`).

---

## 4. Мутационное тестирование (`mutations.log`)

- **Базовый файл:** `rust/gui-spike/ui/app.js` (Baseline MD5: `d3a3efe6b18ecfe6fe004e81a81ba961`).
- **Команда прогона каждого мутанта:** `node --test tests/unit/gui-spike-features.test.mjs`.
- **Итог:** **5/5 мутантов убиты (`KILLED`)**, после каждого мутанта выполнен restore и проверено совпадение MD5 (`match=True`).

| ID | Мутация в `rust/gui-spike/ui/app.js` | MD5 мутанта | Exit | Убивший тест / проверка | Вердикт |
|---|---|---|---:|---|---|
| `MUT-1-voice-devices-empty-list` | `devices: DEFAULT_VOICE_DEVICES` → `devices: []` при инициализации `createVoiceDevicesSettings` | `c0ac89f7f9ead40ff91ca348a0a0f524` | `1` | `монтирует createVoiceDevicesSettings в модалку настроек`: `не выбрано входное устройство по умолчанию` | **KILLED** |
| `MUT-2-guild-notif-wrong-guild-id` | `guildId: guild.id` → `guildId: "@me"` в `syncNotificationsWithGuild` | `8668d52282b8882cd1745f8a4bbede84` | `1` | `связывает #server-menu-notifications и #channel-notif-menu`: `guildId в контроллере не 777` | **KILLED** |
| `MUT-3-guild-notif-drop-tree-channels` | `channels: currentGuildChannels()` → `channels: []` в `syncNotificationsWithGuild` | `61dc282f7043fee35aa4ed915121ce19` | `1` | `связывает #server-menu-notifications и #channel-notif-menu`: `селект каналов в модалке не заполнен каналами из tree.channels` | **KILLED** |
| `MUT-4-tree-contextmenu-drop-channel-menu` | `openChannelNotifications(channel.id, button)` → `openChannelNotifications("", button)` в `onContextMenu` дерева каналов | `0db368286400efcd8bb9f9ddd2a4d093` | `1` | `связывает #server-menu-notifications и #channel-notif-menu`: `contextmenu по каналу не открыл #channel-notif-menu` | **KILLED** |
| `MUT-5-inbox-bell-ignore-chat-channel-id` | `chat.channelId \|\|` → `"" \|\|` в обработчике колокола шапки чата `[data-action="inbox"]` | `801dcb51687db664bccb1cddcf045c88` | `1` | `связывает #server-menu-notifications и #channel-notif-menu`: `#channel-notif-menu при открытии из шапки чата не получил chat.channelId` | **KILLED** |

- **Контрольный прогон после восстановления baseline:** `FINAL_CONTROL_EXIT_CODE=0` (`102 pass, 0 fail`), `FINAL_MD5=d3a3efe6b18ecfe6fe004e81a81ba961 (match=True)`.
- **Артефакт:** `waves/RW-40/PM/int07-ui-mount/mutations.log` (MD5: `46bcfd31645b70f2b373d1103ee371ac`).

---

## 5. Файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/app.js` | modified | `d3a3efe6b18ecfe6fe004e81a81ba961` |
| `rust/gui-spike/ui/index.html` | modified | `ea9649ab18041644d5ed0ac11761a2dc` |
| `tests/unit/gui-spike-features.test.mjs` | modified | `38f60853e7d30f14a21f897477a5c778` |
| `waves/RW-40/PM/int07-ui-mount/red.log` | created | `de60d8d22d00135eb2f167d658d31f72` |
| `waves/RW-40/PM/int07-ui-mount/green.log` | created | `0f5d224569f9f7072a8c5c29023db180` |
| `waves/RW-40/PM/int07-ui-mount/mutations.log` | created | `46bcfd31645b70f2b373d1103ee371ac` |

---

## 6. Риски и открытые границы

1. **Гейт уровня batch (`AGENTS.md` §3):** запущены targeted-тесты, смежные unit/meta-наборы и оба `tsc`; полный `npm run gate:worker` и `cargo` выполняются один раз на batch в интеграционном лейне.
2. **IPC-команды уведомлений на стороне Rust (F-5 из `MERGE-READINESS-13.md`):** UI-контроллер `notifications` вызывает `update_guild_notification_settings` и `update_channel_notification_override` через общий `invoke` из `app.js`; регистрация соответствующих `#[tauri::command]` в `rust/gui-spike/src/main.rs` относится к Rust-лейну G1.
