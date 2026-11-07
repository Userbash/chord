# Приёмка PM: task-173 (BE-P2-11) — settings-proto lossless wire encode и REST GET/PATCH /users/@me/settings-proto/{type}

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-173-settings-proto-http`
- **Пулл-реквест:** `PR-58` (`feature/task-173-settings-proto-http -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **Lossless wire encode в `wire.rs`**:
   - Реализованы `ProtoField::encode_into`, `ProtoMessage::encode` и `ProtoMessage::set_field`, сохраняющие все поля (включая неизвестные поля типов 0, 1, 2, 5) байт-в-байт при roundtrip на реальных фикстурах `READY` и `UPDATE_*`.
2. **REST GET/PATCH `/users/@me/settings-proto/{type}` в `settings/mod.rs`**:
   - Реализованы `SettingsProto::encode_bytes`, `SettingsProto::encode_base64`, `settings_proto_path`, `build_get_settings_proto_request`, `parse_settings_proto_response`, `build_patch_settings_proto_body` с типизацией ошибок `401/429/5xx/4xx`.
   - Все 60 тестов (`33 lib`, `18 settings_proto`, `9 settings_proto_http`) проходят (`60/60 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 87A)** (`5/5` мутаций).
