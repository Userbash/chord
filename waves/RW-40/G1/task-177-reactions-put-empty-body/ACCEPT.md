# Приёмка PM: task-177 (BE-P3-01) — PUT реакции без тела (REACTION_PUT_EMPTY_BODY = b""), 204 без Content-Type и обратная совместимость

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-177-reactions-put-empty-body`
- **Пулл-реквест:** `PR-60` (`feature/task-177-reactions-put-empty-body -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **`REACTION_PUT_EMPTY_BODY = b""`, `ReactionTransport::put_empty_body` и `DiscordReactionApi::add_reaction_empty_body` в `reactions.rs`**:
   - В точном соответствии с оракулом `RE/rest-d02.json` (`endpoints[38]`: `reqBodyKeys: []`, `reqBodyRequests: 0`, `requestsWithContentType: 0`, `status: 204`) отправляет `PUT /channels/{id}/messages/{id}/reactions/{emoji}/@me` с 0-байтовым телом `b""`.
   - Сохранена 100% обратная совместимость с существующими тестами `reactions_contract.rs` (`add_reaction` / `toggle`).
2. **Контрактные тесты в `reactions_contract.rs`**:
   - Все 16 контрактных тестов и 17 lib-тестов проходят (`33/33 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 89A)** (`5/5` мутаций).
