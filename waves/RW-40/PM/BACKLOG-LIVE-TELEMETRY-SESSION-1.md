# PM Backlog — Live User Telemetry Session #1 (`2026-11-01T12:16:00Z` – `12:21:33Z`)

- **Источник данных**: `/tmp/live-ui-telemetry.jsonl` (`786` записей) и `/tmp/discord-gui-spike.log` (`860` строк).
- **Процесс**: `/home/sanya/orca/project/rust/target/debug/discord-gui-spike` (`PID 301514`).

## 1. Хронология действий пользователя и вызванных функций

| Время (UTC) | Действие пользователя (`ui_interaction`) | Модуль и функция (`module` -> `handler`) | Вызванные IPC-команды (`ipc_invoke`) и длительность |
|---|---|---|---|
| `12:16:01.047Z` | Клик `Восстановить сессию` (`#auth-restore`) | `ui/app.js` -> `globalShellHandler` | `net_connect` (11ms), `ready` (55ms), `voice_audio_devices` (26ms), `channels` (18ms), `members_page` (122ms), `messages_page` (1046ms) |
| `12:16:23.321Z` | Клик по голосовому каналу `Терраформация` (`1344356603997978745`) | `ui/app.js` -> `createChannelTree -> channelNode` | `voice_connect(guildId=1014537851418980372, channelId=1344356603997978745, selfMute=false, selfDeaf=false)` (9ms) |
| `12:16:37.160Z` | Клик `Отключить звук и микрофон` (`data-action="deafen"`) | `ui/app.js` -> `createChannelTree -> channelNode` | `voice_set_audio_device(input, muted=true)` (19ms), `voice_set_audio_device(output, muted=true)` (19ms), `voice_connect(selfMute=true, selfDeaf=true)` (20ms) |
| `12:16:37.860Z` | Клик `Включить звук` (`data-action="deafen"`) | `ui/app.js` -> `createChannelTree -> channelNode` | `voice_set_audio_device(input, muted=false)` (17ms), `voice_set_audio_device(output, muted=false)` (17ms), `voice_connect(selfMute=false, selfDeaf=false)` (17ms) |
| `12:16:45.774Z` | Клик по голосовому каналу `📞┃Качки` (`1271435029163151391`) + `Подтвердить` (`#voice-switch-confirm-modal`) | `ui/app.js` -> `createChannelTree -> channelNode` / `globalShellHandler` | Двойной `voice_connect(channelId=1271435029163151391, selfMute=true)` (8ms + 8ms) |
| `12:16:54.421Z` | Клик `Включить микрофон` (`data-action="mute"`) | `ui/app.js` -> `createChannelTree -> channelNode` | `voice_set_audio_device(input, muted=false)` (15ms), `voice_connect(selfMute=false)` (15ms) -> **через 1.8 с (`12:16:56.246Z`) самопроизвольный откат в `muted=true`!** |
| `12:17:03.257Z` | Переключение текстовых каналов `#🔴┃поговорить-тут` (`1058348665342808104`), `🧰┃сообщения` (`1056563701764857896`) | `ui/app.js` -> `createChannelTree -> channelNode` | `messages_page` (606ms), `ack_message` (403ms) + `ack_channel_message` (393ms), 4x `read_state` |
| `12:17:09.316Z` | Клик `Отключиться` (`#voice-panel`) | `ui/app.js` -> `createChannelTree -> channelNode` | `voice_disconnect(guildId=1014537851418980372)` (22ms) |
| `12:17:21.084Z` – `12:17:45.044Z` | Переход в `Личные сообщения и Друзья` (`#home-rail-btn`), открытие ЛС `ДраКс-Иван` и `revqair`, клик `+ Прикрепить файл` (`composer-attach`), возврат в `👥 Друзья` | `ui/features/guild-rail.js`, `ui/app.js`, `ui/features/composer.js` | `messages_page` (620ms), `ack_message` + `ack_channel_message`, `composer-file-input` |
| `12:17:46.660Z` – `12:18:00.043Z` | Переключение серверов `Generations of Players` -> `Игровое сообщество ⚛MONOLIT⚛`, вход в голосовой канал `🏙 На чердаке у Карлсона 🏙` (`351403515723120651`, 4 участника) | `ui/features/guild-rail.js`, `ui/app.js` -> `createChannelTree -> channelNode` | `voice_connect(guildId=274093052673392641, channelId=351403515723120651)` (4ms); в RTC-логах ошибка `binary op 25` и `binary op 30` |
| `12:18:11.333Z` – `12:19:09.125Z` | Открытие `⚙ Настройки пользователя`, запуск `Проверка микрофона`, выбор устройства вывода `🎧Easy Effects Sink`, переключение `Krisp` / `Пусто`, поповер `Подавление шума с помощью Krisp` (`#live-krisp-popout` -> `Проверка` / `Стоп`) | `ui/features/settings.js` -> `createSettingsConsole`, `ui/app.js` -> `globalShellHandler` | `voice_set_audio_device(mic_test_start)`, `voice_set_audio_device(output, easyeffects_sink)`, `voice_set_audio_device(krisp, false/true)`, `voice_set_audio_device(mic_test_stop)` |
| `12:20:08.007Z` – `12:21:32.549Z` | Клик `Отключить звук и микрофон` (`deafen`), переход на сервер `ITSKIDON PC BOOST`, клик `Показать всех (104) ↓` (`member-panel-all`), переход в ЛС и обратно в `⚛MONOLIT⚛` | `ui/features/members.js` -> `renderMemberRow`, `ui/features/guild-rail.js` | `members_page(limit=100)` + `members_page(cursor=378216428500811776, limit=100)` (109ms + 127ms) |

---

## 2. Карточки исправлений для Product Manager (PM) и разработки

### CARD-TEL-001 (`P0` / Отдел `G5 Voice / Platform`): Ошибка `type mismatch` при передаче бинарных пакетов DAVE MLS (`op=25, 27, 29, 30`) в `discord_voice.node`
- **Файлы**: `rust/crates/discord-core/src/ws.rs` (строки `1838–1902`).
- **Фактический результат из телеметрии**:
  При входе пользователя в голосовой канал с людьми (`🏙 На чердаке у Карлсона 🏙`, 4 участника) сервер присылает бинарные кадры DAVE MLS:
  - `[VOICE-RTC] WS RX BINARY op=25 seq=14 payload_len=71` -> `binary op 25 err: Invalid argument at index 0: type mismatch`
  - `[VOICE-RTC] WS RX BINARY op=30 seq=15 payload_len=1815` -> `binary op 30 err: Invalid argument at index 1: type mismatch`
  Из-за этого DAVE MLS Welcome не применяется в `discord_voice.node`, и входящие зашифрованные голосовые пакеты участников не декодируются.
- **Эталон RE (`/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`, `RTCControlSocket.parseWebSocketMessage`)**:
  В эталонном клиенте Discord `parseWebSocketMessage` передаёт в `updateMLSExternalSender(i)`, `processMLSProposals(i, cb)`, `prepareMLSCommitTransition(tid, i.slice(2), cb)` и `processMLSWelcome(tid, i.slice(2), cb)` объект **`Uint8Array`** (`Buffer`), а не `ArrayBuffer`. В `ws.rs:1841` вызывался `toAb(buf)` (`ArrayBuffer`), что приводило к отказу N-API проверки типов в `discord_voice.node`.
- **Требуемое исправление**:
  Передавать `new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength)` (и `new Uint8Array(payload.buffer, payload.byteOffset, payload.byteLength)` для `op=29, 30`) напрямую в методы `conn.updateMLSExternalSender`, `conn.processMLSProposals`, `conn.prepareMLSCommitTransition`, `conn.processMLSWelcome`.

### CARD-TEL-002 (`P1` / Отдел `G2 Frontend`): Самопроизвольный возврат `selfMute: true` через 1.8 с после размута и дублирование `voice_connect` при смене канала
- **Файлы**: `rust/gui-spike/ui/app.js` (`joinVoiceChannel`, `syncLiveShell`, обработчик `data-action="mute"`).
- **Фактический результат из телеметрии**:
  1. В `12:16:46.769Z` при подтверждении перехода в `📞┃Качки` (`#voice-switch-confirm-modal`) отправились сразу два вызова `voice_connect` с `selfMute: true`, хотя в предыдущем канале микрофон был включён (`selfMute: false`), а `📞┃Качки` не является AFK-каналом.
  2. В `12:16:54.421Z` пользователь нажал `Включить микрофон` (`selfMute: false`), но уже в `12:16:56.246Z` (через 1.8 с) фоновый поллинг `ready` (`syncLiveShell`) прочитал устаревший статус шлюза и принудительно отправил `voice_connect(selfMute: true)` и `voice_set_audio_device(input, muted: true)`.
- **Требуемое исправление**:
  1. Ввести защитное окно локального действия (`lastVoiceActionAt`, 5000 мс), в течение которого `syncLiveShell` не перезаписывает локальные флаги `voiceState.muted` / `voiceState.deafened` устаревшим снимком `ready`.
  2. При переходе между обычными (не-AFK) голосовыми каналами сохранять текущее состояние `voiceState.muted` пользователя и убрать дублирующий вызов `voice_connect`.

### CARD-TEL-003 (`P1` / Отдел `G2 Frontend`): Автоматическая остановка `mic_test` при закрытии окна настроек
- **Файлы**: `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/features/settings.js`.
- **Фактический результат из телеметрии**:
  В `12:18:18.743Z` пользователь запустил `Проверка микрофона` (`mic_test_start`), затем закрыл и заново открыл настройки в `12:18:49.729Z`, чтобы вручную нажать `Прекратить проверку` (`mic_test_stop` в `12:18:50.666Z`).
- **Требуемое исправление**:
  При закрытии модального окна настроек (`closeLiveSettingsModal` / `ESC` / клик по `⚙`) автоматически вызывать `mic_test_stop` и сбрасывать состояние кнопки проверки микрофона.

### CARD-TEL-004 (`P2` / Отдел `G2 Frontend`): Устранение дублирования `ack_message` + `ack_channel_message` и избыточных `read_state` при клике по каналу
- **Файлы**: `rust/gui-spike/ui/app.js`.
- **Фактический результат из телеметрии**:
  На каждый клик по текстовому каналу или ЛС последовательно вызываются `ack_message` (~400 мс) и `ack_channel_message` (~400 мс), а также до 4 вызовов `read_state`.
- **Требуемое исправление**:
  Оставить один вызов `ack_channel_message` и коалесцировать обновление `read_state`.

---

## 3. Выполненные исправления по результатам живой телеметрии (2026-11-01)

1. **`CARD-TEL-001` (`P0` — Дешифрование DAVE MLS и звук участников в [`rust/crates/discord-core/src/ws.rs`](file:///home/sanya/orca/workspaces/project/unstable/rust/crates/discord-core/src/ws.rs#L1633-L1905))**:
   - Исправлена передача бинарных кадров `op=25` (`updateMLSExternalSender`), `op=27` (`processMLSProposals`), `op=29` (`prepareMLSCommitTransition`) и `op=30` (`processMLSWelcome`) в `discord_voice.node` (передаётся `Buffer`/`Uint8Array` вместо `ArrayBuffer`, устранена ошибка `Invalid argument at index 0/1: type mismatch`).
   - Устранён повторный вызов `getMLSKeyPackage` внутри `op=25`, который сбрасывал ключ эпохи после `session_description`.
   - Добавлен повторный вызов `conn.mergeUsers(Array.from(remoteUsers.values()))` после `setTransportOptions` в `session_description` и после `executeSecureFramesTransition`, а также исключена перезапись `ssrc=0` при приходе `OP 18`.
2. **`CARD-TEL-002` (`P0` — Зелёная обводка говорящих и исправление ложных иконок `self_mute`/`self_deaf` в [`rust/crates/discord-core/src/ws.rs`](file:///home/sanya/orca/workspaces/project/unstable/rust/crates/discord-core/src/ws.rs#L2624-L2800), [`rust/gui-spike/src/data.rs`](file:///home/sanya/orca/workspaces/project/unstable/rust/gui-spike/src/data.rs#L853-L1070) и [`rust/gui-spike/ui/app.js`](file:///home/sanya/orca/workspaces/project/unstable/rust/gui-spike/ui/app.js#L3696-L8225))**:
   - Время удержания статуса говорящего (`speaking_users` при `flags != 0` от `OP 5` и `setOnSpeakingCallback`) увеличено с `850 мс` до `8 с` (со сбросом немедленно при приходе `flags == 0`), а частота опроса `voice_rtc_status` ускорена до `220 мс`.
   - В `isChannelAfk` (`ui/app.js`) отключено ложное срабатывание по `guild.afk_channel_id` на обычных голосовых каналах (`📞┃Качки`), а вход в AFK-комнату больше не мутирует пользовательский `selfVoicePrefs.muted` навсегда.
   - При снятии оглушения (`toggleSelfDeafenBoth(false)`) синхронно включаются и динамики (`deafened = false`), и микрофон (`muted = false`), а в списке участников голосового канала для `ShadowByte` больше не отображаются залипшие значки перечёркнутого микрофона/наушников из устаревшего снимка шлюза.
   - Убран нативный браузерный `title="Подключиться к голосовому каналу"`, перекрывавший строку `Выбрать статус канала ✏️`.
