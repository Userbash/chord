# План устранения дефектов и стабилизации клиента (Sprint Wave 48 / ECC 2.0)

## 1. Анализ инцидентов и коренного источника ошибок (Root Cause Analysis)

### Инцидент 1: Аварийное падение `node /tmp/discord_voice_dave_bridge.js` (Signal 11 SEGV)
- **Стек падения**:
  ```
  #0  _ZN5Krisp8AudioSdk3VadIsED1Ev (discord_krisp.node)
  #1  _ZNSt16_Sp_counted_baseILN9__gnu_cxx12_Lock_policyE2EE10_M_releaseEv (discord_krisp.node)
  #2  _ZN5Krisp8AudioSdk3VadIsE6createERKNS0_16VadSessionConfigE (discord_krisp.node)
  #3  krispCreateVadInt16 (discord_krisp.node)
  #4  KrispVADSetup2 (discord_krisp.node)
  #5  discord::KrispWrapper::SetupVAD (discord_voice.node)
  #6  discord::VoiceActivityDetector::Process (discord_voice.node)
  ```
- **Причина**: В `ws.rs:3358` и `3748` передавался флаг `vadUseKrisp: Boolean(currentKrispEnabled)`. В Linux-версии `discord_krisp.node` модель VAD (`krisp-vad-o-v2.kef`) падает с SEGV при динамической инициализации из аудио-потока `webrtc_audio_mo`.
- **Решение**: Разделить шумоподавление Krisp (`noiseCancellation: true` через нейросеть `krisp-nc-o-med-v7.kef`, эффективно убирающую клики клавиатуры и шум) и детекцию речи VAD (`vadUseKrisp: false`, использующую стабильный WebRTC VAD ядра).

---

### Инцидент 2: Проблема с проверкой роли («вам нельзя сюда» / дал-забрал роль)
- **Причина**:
  1. В `rust/gui-spike/src/net.rs` событие шлюза Discord `GUILD_MEMBER_UPDATE` отсутствовало в `FORWARDED_DISPATCHES` и отфильтровывалось ядром.
  2. В `rust/gui-spike/src/data.rs` и `rust/gui-spike/ui/app.js` отсутствовала обработка `GUILD_MEMBER_UPDATE`: при выдаче или снятии роли на сервере Discord кэш `liveMemberRolesByGuildAndUser` не обновлялся до перезахода.
  3. `computeEffectiveChannelPermissions` рассчитывал доступ по старым ролям и блокировал вход.
- **Решение**: Пробросить `GUILD_MEMBER_UPDATE`, `GUILD_ROLE_CREATE`, `GUILD_ROLE_UPDATE`, `GUILD_ROLE_DELETE` через `net.rs`, добавить парсинг в `data.rs` и `dispatch-model.js`, мгновенно обновлять роли пользователя и инвалидировать кэш прав канала.

---

### Инцидент 3: Пропадание и повторное появление участников из официального Discord
- **Причина**:
  В `rust/gui-spike/ui/app.js:11572` функция `refreshLiveReadyVoiceAndUsers()` при каждом вызове делала `liveVoiceStatesByGuild.clear()`. Асинхронный ответ `voice_states_sync` стирал только что прибывшие события `VOICE_STATE_UPDATE` от официальных клиентов Discord, вызывая мерцание и исчезновение участников из дерева каналов.
- **Решение**: Заменить деструктивный `clear()` на бесконфликтное слияние (atomic deep merge) с сохранением подтверждённых участников, а также нормализовать ключи гильдий (`normalizeVoiceGuildKey`).

---

## 2. Распределение задач по 6 воркерам ECC 2.0

| Воркер | Отдел | Задача | Файлы изменений | Контрактная верификация |
|---|---|---|---|---|
| **Worker 1** | `G5 Voice / Platform` | `TASK-763`: Защита от краша Krisp VAD и разделение NC/VAD | `rust/crates/discord-core/src/ws.rs` | `tests/unit/krisp-vad-stability.test.mjs` |
| **Worker 2** | `G1 Backend` | `TASK-764`: Проброс и инжест `GUILD_MEMBER_UPDATE` в Rust BFF | `rust/gui-spike/src/net.rs`, `rust/gui-spike/src/data.rs` | `cargo test -p discord-core`, `--test data_contract` |
| **Worker 3** | `G2 Frontend` | `TASK-765`: Реактивное обновление ролей и прав доступа в UI | `rust/gui-spike/ui/features/dispatch-model.js`, `rust/gui-spike/ui/features/dispatch-view.js`, `rust/gui-spike/ui/app.js` | `npm run typecheck`, `tests/unit/guild-member-update-permissions.test.mjs` |
| **Worker 4** | `G2 Frontend` | `TASK-766`: Бесконфликтный мерж участников голоса (Anti-Flicker) | `rust/gui-spike/ui/app.js` | `tests/unit/voice-state-merge-anti-flicker.test.mjs` |
| **Worker 5** | `G4 QA` | `TASK-767`: Сквозной регрессионный сьют проверок инцидентов | `tests/unit/` | Полный запуск targeted-тестов (100% pass) |
| **Worker 6** | `G6 Tech Writer / PM` | `TASK-768`: Доска задач Kanban, манифест сборки, синхронизация ECC | `waves/kanban-board.json`, `waves/kanban-board.html`, `rust/dist/SHA256SUMS` | Сборка бинарника `discord-gui-spike` + Git Push |
