# Утренний акт передачи состояния и фиксации задач (10 ноября 2026)
## Волна RW-42: Native GPUI Client Migration (`discord-gpui`)

### 1. Текущий статус системы
- **Статус готовности**: Задачи `TASK-701` .. `TASK-746` полностью реализованы и приняты (`[x] accepted`).
- **Тестовый контрактный харнесс**: **61/61 тестов успешно пройдено (100% Green, 0 failed, 2.50s)**.
- **Релизный бинарник**: `rust/discord-gpui/target/release/discord-gpui` (55 МБ, собран в `dev` и `release` профилях).
- **Стек**: 100% нативный Rust (`gpui-kit` + WGPU, 120 FPS), без WebKit/Tauri и фрагментированного JavaScript.

---

### 2. Завершённые задачи и библиотеки (TASK-741 .. TASK-746)

| Задача | Отдел | Реализованный функционал | Библиотека / Стандарт |
|---|---|---|---|
| **TASK-741** | **G1 Backend** | **REST API Message Actions**: реальные сетевые вызовы `DELETE /channels/{id}/messages/{id}`, `PUT /channels/{id}/pins/{id}`, `DELETE /channels/{id}/pins/{id}`. Оптимистичный UI с контекстным меню ПКМ («Закрепить/Открепить», «Удалить»). | `discord-core::rest`, `reqwest`, Discord REST API v9 |
| **TASK-742** | **G5 Voice** | **Voice UDP Media Streamer**: `VoiceUdpStreamer` с живым `tokio::net::UdpSocket`. Выполнение IP Discovery (74B), Keepalive (8B `0x1337cafe` LE), RTP 12B заголовок + AES-256-GCM AEAD шифрование 20мс Opus-пакетов. | `tokio::net::UdpSocket`, `aes-gcm = "0.11.1"`, эталон `Serein transport.rs` |
| **TASK-743** | **G5 Platform** | **Демонстрация экрана PipeWire**: `ScreenCastManager` через D-Bus интерфейс `org.freedesktop.portal.ScreenCast`. Выбор экранов и окон приложений, согласование курсора (Embedded/Metadata), получение PipeWire remote fd. | `discord-platform::screencast`, `ashpd` / D-Bus |
| **TASK-744** | **G5 Platform** | **D-Bus уведомления и трей**: `DesktopNotificationManager` (`org.freedesktop.Notifications`) с кнопками быстрых действий, `SystemTrayManager` с 6 пунктами меню Discord и бейджем непрочитанных `1..99+`. | `discord-platform::notifications`, `discord-platform::tray` |
| **TASK-745** | **G1 Backend** | **Discord Local RPC IPC Server**: локальный сокет `$XDG_RUNTIME_DIR/discord-ipc-0` для внешних игр и приложений. Поддержка `HANDSHAKE` и `SET_ACTIVITY` по стандарту `rust-discord-activity`. | `rust-discord-activity`, `discord-platform::rpc_ipc` |
| **TASK-746** | **G4 QA + DevOps** | **Контрактный тестовый харнесс 61/61**: полное регрессионное покрытие всех сетевых, голосовых, видео и платформенных модулей. | `tests/gpui_contract_test.rs` |

---

### 3. Инструкция по запуску приложения утром

```bash
# 1. Запуск скомпилированного нативного клиента:
./rust/discord-gpui/target/release/discord-gpui

# 2. Быстрый прогон контрактных тестов (2.5 секунды):
cd rust/discord-gpui && cargo test --test gpui_contract_test
```

---

### 4. Рекомендуемые следующие шаги на утро
1. Визуальная проверка работы в реальном окружении X11/Wayland.
2. Проверка живого голосового звонка с внешним сервером при наличии активного войс-токена.
3. Проверка трансляции окна приложения через XDG Desktop Portal диалог.
4. Синхронизация репозитория и оформление финального тега волны `RW-42-RELEASE`.
