# BRIEF NET-E1 — gateway: URL, супервайзер, op40, close-коды

Владелец: NET-E1 (Core). PM: DeepSeek. Основание: `CORE-AUDIT` P0-3, `AUTH-RE` G-04…G-20.
Запуск: параллельно NET-C2 (другой target-dir). Сеть — только стабы.

## Файлы (только эти)
- `rust/crates/discord-core/src/ws.rs`, `src/gateway.rs`
- `rust/crates/discord-core/src/gateway/…` (новый supervisor-модуль, если нужно)
- `rust/crates/discord-core/src/lib.rs` — re-export
- `rust/crates/discord-core/tests/{gateway_supervisor.rs,ws_transport.rs}` (расширить)
- логи: `waves/RW-39/NET/PM/nete1/`
READ-ONLY: остальные крейты, gui-spike, fixtures.

## Задачи (red-first)
1. `GatewayUrl`: база `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream`;
   нормализация `resume_gateway_url` (схема/параметры), тест.
2. **op40**: `Op::QosHeartbeat` (40) с `{seq,qos}`; отправка по HELLO-интервалу; входящий
   op1 — триггер немедленного heartbeat; op11 ACK; таймаут → reconnect. Сохранить op1 как
   fallback (feature/флаг), тест на оба.
3. **Supervisor**: connect → HELLO → IDENTIFY/RESUME → dispatch/heartbeat → close:
   - 4004 → `Stop(Unauthorized)` (logout), 4013/4014 → `Stop` (без бесконечного reconnect);
   - op7 → reconnect; op9 `d=true` → RESUME, `d=false` → IDENTIFY;
   - backoff через `plan_reconnect`, лимит попыток, отмена.
4. IDENTIFY по RE: capabilities 1734653, `compress:false`, properties + `client_app_state`,
   `is_fast_connect:false`, `gateway_connect_reasons`, `installation_id?`; без `intents/shard`.
5. Тесты на стаб-сервере/чистой машине состояний; без реального Discord.

## Проверки (root target)
```bash
cd rust && cargo test --offline -p discord-core
cargo clippy --offline -p discord-core --all-targets -- -D warnings
cargo fmt -p discord-core -- --check
```
Окно/GUI/сеть/commit — запрещены.

## Отчёт
`NET/PM/nete1/REPORT.md`: URL/op40/supervisor → red → green, тесты, md5, остаток.
