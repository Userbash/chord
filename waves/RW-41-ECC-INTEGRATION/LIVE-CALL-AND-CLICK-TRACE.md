# Live Call, Click, Screenshot & Code Correlation Trace

- **Обновлено:** `2026-11-10T19:19:56.102Z`
- **Лог активности:** `/tmp/discord-full-user-activity.log`
- **Директория скриншотов:** `/tmp/trace-captures` (снято: `126`)
- **Последний нативный вызов GDB:** `data::set_system_audio_device() [Audio / Voice Switch]`

## Таблица корреляции: Клик / Событие UI -> Код (JS/Rust) -> Статус -> Скриншот

| Время | Тип | Действие / Элемент UI | Вызов на уровне кода (JS -> IPC -> Rust) | Статус / Результат | Скриншот |
|---|---|---|---|---|---|
| `00:19:55` | `UI_CLICK` | <button> "Отключиться" (282,687) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0126-click_button.png` |
| `00:19:47` | `IPC_OK` | After click "Сообщения" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (6ms) | `shot-0125-click_chat-list.png` |
| `00:19:45` | `UI_CLICK` | <div> "Сообщения" (495,618) | `ui/features/chat.js::initChatScreen / messageContextMenu` | dispatched | `shot-0125-click_chat-list.png` |
| `00:19:45` | `IPC_OK` | After click "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (1ms) | `shot-0124-click_live-voice-status-trigger.png` |
| `00:19:45` | `IPC_OK` | After click "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (1ms) | `shot-0124-click_live-voice-status-trigger.png` |
| `00:19:43` | `UI_CLICK` | <button> "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" (183,693) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0124-click_live-voice-status-trigger.png` |
| `00:19:39` | `IPC_OK` | After click "🎮CS:S Player🎮голос" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4708:18->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/main.rs::voice_connect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (20ms) | — |
| `00:19:39` | `IPC_OK` | After click "🎮CS:S Player🎮голос" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4809:20->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=output) -> discord-voice` | ✅ ok (20ms) | — |
| `00:19:39` | `IPC_OK` | After click "🎮CS:S Player🎮голос" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4793:20->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=input) -> discord-voice` | ✅ ok (20ms) | — |
| `00:19:39` | `UI_CLICK` | <button> "🎮CS:S Player🎮голос" (135,630) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:19:38` | `UI_CLICK` | <button> "🎮CS:S Player🎮голос" (131,638) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0123-click_button.png` |
| `00:19:38` | `UI_CLICK` | <button> "🔥Стрим костра🔥голос" (131,639) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:19:36` | `IPC_OK` | After click "Отключиться" | `playVoiceUiSound@tauri://localhost/app.js:4013:16->disconnectFromLiveVoiceChannel@tauri://localhost/app.js:4869:23 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=ui_sound) -> discord-voice` | ✅ ok (21ms) | `shot-0122-click_button.png` |
| `00:19:36` | `IPC_OK` | After click "Отключиться" | `disconnectFromLiveVoiceChannel@tauri://localhost/app.js:4891:18->@tauri://localhost/app.js:12458:37 => rust/gui-spike/src/main.rs::voice_disconnect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (20ms) | — |
| `00:19:36` | `UI_CLICK` | <button> "Отключиться" (280,695) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0122-click_button.png` |
| `00:19:34` | `IPC_OK` | After click "Сообщения" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (7ms) | `shot-0121-click_chat-list.png` |
| `00:19:29` | `IPC_OK` | After click "Сообщения" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (1ms) | `shot-0121-click_chat-list.png` |
| `00:19:21` | `UI_CLICK` | <div> "Сообщения" (562,578) | `ui/features/chat.js::initChatScreen / messageContextMenu` | dispatched | `shot-0121-click_chat-list.png` |
| `00:19:21` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (0ms) | — |
| `00:19:21` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (1ms) | — |
| `00:19:20` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4708:18->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/main.rs::voice_connect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (30ms) | — |
| `00:19:20` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4809:20->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=output) -> discord-voice` | ✅ ok (28ms) | — |
| `00:19:20` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4793:20->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=input) -> discord-voice` | ✅ ok (28ms) | — |
| `00:19:20` | `UI_CLICK` | <button> "🎮CS:GO Player🎮0005" (148,573) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:19:19` | `UI_CLICK` | <button> "🎮CS:GO Player🎮0005" (165,554) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0120-click_button.png` |
| `00:19:15` | `IPC_OK` | After click "Отключиться" | `playVoiceUiSound@tauri://localhost/app.js:4013:16->disconnectFromLiveVoiceChannel@tauri://localhost/app.js:4869:23 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=ui_sound) -> discord-voice` | ✅ ok (21ms) | `shot-0119-click_button.png` |
| `00:19:15` | `IPC_OK` | After click "Отключиться" | `disconnectFromLiveVoiceChannel@tauri://localhost/app.js:4891:18->@tauri://localhost/app.js:12458:37 => rust/gui-spike/src/main.rs::voice_disconnect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (21ms) | — |
| `00:19:15` | `UI_CLICK` | <button> "Отключиться" (291,686) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0119-click_button.png` |
| `00:19:13` | `UI_CLICK` | <div> "Сообщения" (553,427) | `ui/features/chat.js::initChatScreen / messageContextMenu` | dispatched | `shot-0118-click_chat-list.png` |
| `00:19:13` | `IPC_OK` | After click "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (21ms) | `shot-0117-click_live-voice-status-trigger.png` |
| `00:19:13` | `IPC_OK` | After click "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (20ms) | `shot-0117-click_live-voice-status-trigger.png` |
| `00:18:51` | `UI_CLICK` | <button> "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" (113,689) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0117-click_live-voice-status-trigger.png` |
| `00:18:47` | `IPC_OK` | After click "Подтвердить" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4708:18->@tauri://localhost/app.js:4969:32 => rust/gui-spike/src/main.rs::voice_connect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (12ms) | — |
| `00:18:47` | `UI_CLICK` | <button> "Подтвердить" (765,502) | `ui/app.js::globalShellHandler` | dispatched | — |
| `00:18:46` | `UI_CLICK` | <button> "🔕 AFK-ROOM [ZzZz]голос" (171,657) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0116-click_button.png` |
| `00:18:45` | `IPC_OK` | After click "Повторить подключение к голосовому каналу" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4708:18->retryActiveVoiceConnection@tauri://localhost/app.js:4571:30 => rust/gui-spike/src/main.rs::voice_connect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (10ms) | — |
| `00:18:45` | `UI_CLICK` | <button> "Повторить подключение к голосовому каналу" (139,707) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:18:44` | `UI_CLICK` | <div> "Вы уверены?✕Кажется, вы находитесь в другом голосовом канале" (157,628) | `ui/app.js::globalShellHandler` | dispatched | — |
| `00:18:44` | `UI_CLICK` | <button> "🔕 AFK-ROOM [ZzZz]голос" (157,628) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0115-click_button.png` |
| `00:18:43` | `UI_CLICK` | <div> "Вы уверены?✕Кажется, вы находитесь в другом голосовом канале" (157,621) | `ui/app.js::globalShellHandler` | dispatched | — |
| `00:18:42` | `UI_CLICK` | <button> "🔕 AFK-ROOM [ZzZz]голос" (157,621) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:18:41` | `UI_CLICK` | <button> "🔕 AFK-ROOM [ZzZz]голос" (157,618) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0114-click_button.png` |
| `00:18:39` | `UI_CLICK` | <div> "Сообщения" (479,518) | `ui/features/chat.js::initChatScreen / messageContextMenu` | dispatched | `shot-0113-click_chat-list.png` |
| `00:18:39` | `IPC_OK` | After click "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (1ms) | `shot-0112-click_live-voice-status-trigger.png` |
| `00:18:39` | `IPC_OK` | After click "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" | `loadReadState@tauri://localhost/features/chat.js:3810:43->ackIfNeeded@tauri://localhost/features/chat.js:4024:39 => rust/gui-spike/src/main.rs::read_state -> rust/gui-spike/src/data.rs` | ✅ ok (1ms) | `shot-0112-click_live-voice-status-trigger.png` |
| `00:18:22` | `UI_CLICK` | <button> "О голосовом звонкеRTC… · 0.0%Ожидание ответа сервера Discord" (154,690) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0112-click_live-voice-status-trigger.png` |
| `00:18:18` | `IPC_OK` | After click "Подтвердить" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4708:18->@tauri://localhost/app.js:4969:32 => rust/gui-spike/src/main.rs::voice_connect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (12ms) | — |
| `00:18:18` | `UI_CLICK` | <button> "Подтвердить" (750,481) | `ui/app.js::globalShellHandler` | dispatched | — |
| `00:18:17` | `UI_CLICK` | <button> "🔥Стрим костра🔥голос" (143,562) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:18:17` | `UI_CLICK` | <button> "🔥Стрим костра🔥голос" (142,560) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0111-click_button.png` |
| `00:18:13` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4708:18->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/main.rs::voice_connect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (22ms) | — |
| `00:18:13` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4809:20->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=output) -> discord-voice` | ✅ ok (21ms) | — |
| `00:18:13` | `IPC_OK` | After click "🎮CS:GO Player🎮0005" | `connectToLiveVoiceChannel@tauri://localhost/app.js:4793:20->onSelectVoice@tauri://localhost/app.js:13974:32 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=input) -> discord-voice` | ✅ ok (21ms) | — |
| `00:18:13` | `UI_CLICK` | <button> "🎮CS:GO Player🎮0005" (150,578) | `ui/app.js::createChannelTree -> channelNode` | dispatched | — |
| `00:18:13` | `UI_CLICK` | <button> "🎮CS:GO Player🎮0005" (149,579) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0110-click_button.png` |
| `00:18:11` | `UI_CLICK` | <button> "🎦Кинотеатр🎦голос" (149,582) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0109-click_button.png` |
| `00:18:10` | `IPC_OK` | After click "Отключиться" | `playVoiceUiSound@tauri://localhost/app.js:4013:16->disconnectFromLiveVoiceChannel@tauri://localhost/app.js:4869:23 => rust/gui-spike/src/data.rs::set_system_audio_device(kind=ui_sound) -> discord-voice` | ✅ ok (17ms) | `shot-0108-click_button.png` |
| `00:18:10` | `IPC_OK` | After click "Отключиться" | `disconnectFromLiveVoiceChannel@tauri://localhost/app.js:4891:18->@tauri://localhost/app.js:12458:37 => rust/gui-spike/src/main.rs::voice_disconnect -> discord-voice::{session,udp,srtp,rtcp,reception,devices}` | ✅ ok (17ms) | — |
| `00:18:10` | `UI_CLICK` | <button> "Отключиться" (283,685) | `ui/app.js::createChannelTree -> channelNode` | dispatched | `shot-0108-click_button.png` |
| `00:17:50` | `UI_CLICK` | <button> "✕" (827,125) | `ui/app.js::openVoiceChannelInviteModal` | dispatched | `shot-0107-click_button.png` |
