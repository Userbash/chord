# Live Network, GDB Native Calls & UI Interaction Analysis

- **Обновлено:** `2026-11-10T19:19:57.083Z`
- **Нативные GDB вызовы:** `465`
- **Зафиксированные сетевые взаимодействия:** `810`
- **Действия пользователя в интерфейсе:** `1442`
- **REA эталонная база:** `/home/sanya/re-deep/{rest-d02.md, gateway-schema.md, voice-deep.md}`

## 1. Карта функций интерфейса: работоспособность и сетевое замыкание

| Функция / Компонент интерфейса | Нативный Rust/BFF вызов (GDB) | Сетевой эндпоинт | Статус сети | REA эталон |
|---|---|---|:---:|---|
| **Отправка сообщения (`ChatView`)** | `AppState::send_message` | `POST /channels/{id}/messages` | ✅ Подключено | `rest-d02.md#create-message` |
| **Быстрая реакция 👍 (`ActionBar`)** | `AppState::toggle_reaction` | `PUT /reactions/{emoji}/@me` | ✅ Подключено | `rest-d02.md#add-reaction` |
| **Закрепление сообщения 📌** | `AppState::pin_message` | `PUT /channels/{id}/pins/{id}` | ✅ Подключено | `rest-d02.md#pin-message` |
| **Удаление сообщения 🗑️** | `AppState::delete_message` | `DELETE /messages/{id}` | ✅ Подключено | `rest-d02.md#delete-message` |
| **Голосовой звонок 🔊 (`VoicePanel`)** | `VoiceUdpStreamer::bind_and_stream` | `UDP 74B IP Disc + 8B Keepalive` | ✅ Подключено | `voice-deep.md#protocol-lifecycle` |
| **Демонстрация экрана 🖥️ (`Portal`)** | `ScreenCastManager::start_screencast` | `D-Bus Portal ScreenCast -> PipeWire` | ✅ Подключено | `native-re.md#desktop-portal` |
| **Вступление на сервер (`+`)** | `join_live_discord_invite` | `POST /invites/{code}` | ✅ Подключено | `rest-d02.md#join-invite` |
| **D-Bus уведомления & Трей** | `DesktopNotificationManager` | `org.freedesktop.Notifications` | ✅ Подключено | `native-modules.md` |

## 2. Последние GDB точки останова и нативные вызовы в реальном времени

| Время | Поток | Нативная функция / Хук | Описание назначения |
|---|---|---|---|
| `00:19:55` | `Thread #69` | `data::connect_or_update_live_voice_ext()` | Live Voice/Stream State |
| `00:19:55` | `Thread #72` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:52` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:44` | `Thread #47` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:39` | `Thread #72` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:39` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:39` | `Thread #47` | `data::connect_or_update_live_voice_ext()` | Live Voice/Stream State |
| `00:19:36` | `Thread #69` | `data::connect_or_update_live_voice_ext()` | Live Voice/Stream State |
| `00:19:36` | `Thread #36` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:36` | `Thread #72` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:28` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:20` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:20` | `Thread #72` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:20` | `Thread #47` | `data::connect_or_update_live_voice_ext()` | Live Voice/Stream State |
| `00:19:20` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:15` | `Thread #72` | `data::connect_or_update_live_voice_ext()` | Live Voice/Stream State |
| `00:19:15` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:12` | `Thread #47` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:04` | `Thread #69` | `data::set_system_audio_device()` | Audio / Voice Switch |
| `00:19:04` | `Thread #47` | `data::set_system_audio_device()` | Audio / Voice Switch |

## 3. Сетевая активность и передача данных с бэкенда

| Время | Действие пользователя | Сетевой маршрут / Транспорт | Статус передачи | REA соответствие |
|---|---|---|:---:|---|
| `19:19:39` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:19:20` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:18:47` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:18:45` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:18:18` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:18:13` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:17:30` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:13:18` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:13:06` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:13:04` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:13:02` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:58` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:51` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:43` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:33` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:27` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:23` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:18` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:12:10` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |
| `19:11:50` | **Подключение к голосовому каналу сервера** | `Gateway Op 4 VOICE_STATE_UPDATE -> Voice WS v=8 -> Voice UDP` (WebSocket + UDP (IP Discovery 74B, Keepalive 8B, RTP Opus AEAD)) | `200_OK_NETWORK` | `voice-deep.md:1-120` |

## 4. Задачи для визуальной доски (Kanban) и ECC Control Plane

Все новые события и действия пользователя автоматически анализируются на разрывы (gaps):
- При обнаружении отсутствия сетевого вызова создаётся задача в `waves/kanban-board.json`.
- Решение логируется в ECC Control Plane (`scripts/ecc log-decision`).
- Статус задач отражается на веб-доске: `http://localhost:4321/board`.
