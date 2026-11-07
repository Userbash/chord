# RW-40 / SEC-L02-01-FIX — сброс `last_seq` в `DispatchSink` на границе `connect`/`disconnect`

- **batch/карточка:** `RW-40 / SEC-L02-01-FIX` (закрытие P1 из `QUALITY/SEC-L02-REVIEW.md`);
- **роль:** Воркер G1 GUI-Spike Rust Backend (`CARGO_TARGET_DIR=/tmp/target-g1-spike`);
- **сделано:**
  1. В `rust/gui-spike/tests/net_contract.rs` добавлен контрактный тест `connect_and_disconnect_reset_sequence_dedup_before_ready` с драйвером `DeferredReadyGateway` (без синхронной отправки `GatewayEvent::Ready` внутри `connect()`), проверяющий доставку op0 с малым `s=1` после `s=5` при переходе через `disconnect() -> connect()`, при отдельном `disconnect()` и при новом `connect()` до прихода `Ready`.
  2. В `rust/gui-spike/src/net.rs` добавлен вызов `self.sink.reset()` в `NetController::connect()`, `NetController::disconnect()` и ветках `GatewayEvent::Closed(_)`.
- **red:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-spike cargo test --offline -p discord-gui-spike --test net_contract --test search_contract)` → `FAILED. 23 passed; 1 failed` (`connect_and_disconnect_reset_sequence_dedup_before_ready`: `assertion left == right failed: после disconnect/connect событие с s=1 не должно отбрасываться дедупликатором`, `left: 1, right: 0`), `exit=101`, лог `waves/RW-40/PM/sec-l02-01/red.log`;
- **green:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-spike cargo test --offline -p discord-gui-spike --test net_contract --test search_contract)` → `ok. 24 passed; 0 failed` (`net_contract`) + `ok. 16 passed; 0 failed` (`search_contract`), `exit=0`, лог `waves/RW-40/PM/sec-l02-01/green.log`;
- **targeted:** `waves/RW-40/PM/sec-l02-01/green.log` — 24 теста `net_contract` (было 23, +1 `connect_and_disconnect_reset_sequence_dedup_before_ready`);
- **oracle:** `QUALITY/SEC-L02-REVIEW.md` §P1 и `CONTRACTS/dispatch.md` §5/§6 (дедупликация sequence `s` действует только в пределах одного соединения и обязана сбрасываться при `connect()`/`disconnect()`/`Closed`, не дожидаясь `GatewayEvent::Ready`);
- **генерация:** не применимо;
- **мутации:** 2/2 `KILLED`, эквивалентных — 0, `INCONCLUSIVE` — 0, survivors — 0 (`waves/RW-40/PM/sec-l02-01/mutations.log`):
  - `MUT-1-drop-connect-reset` (удалён `self.sink.reset()` из `NetController::connect`, md5 `5fedd24f4e94eabdbf042e2296068cf3`) → `KILLED` тестом `connect_and_disconnect_reset_sequence_dedup_before_ready` (`exit=101`);
  - `MUT-2-drop-disconnect-reset` (удалён `self.sink.reset()` из `NetController::disconnect`, md5 `c88e7d949351d82e6d27389d413cbe6c`) → `KILLED` тестом `connect_and_disconnect_reset_sequence_dedup_before_ready` (`exit=101`);
- **baseline/restore:** baseline `rust/gui-spike/src/net.rs` = `b286d1779fc89f1dd6c829dc3a32dba7`, `rust/gui-spike/tests/net_contract.rs` = `573d271204f3c1db601e2df73684f129` (`exit=0`, 0 skip); после каждого мутанта выполнен restore и проверено совпадение md5 (`b286d1779fc89f1dd6c829dc3a32dba7`);
- **стабильность:** детерминированный тест с синхронизацией через `Condvar` (`RecordingEmitter::wait_for` и `wait_for_stat`);
- **gate:worker:** в режиме batch полный `gate:worker` запускается на integration-лейне;
- **файлы:**
  - `rust/gui-spike/src/net.rs` — `b286d1779fc89f1dd6c829dc3a32dba7`
  - `rust/gui-spike/tests/net_contract.rs` — `573d271204f3c1db601e2df73684f129`
  - `waves/RW-40/PM/sec-l02-01/red.log` — `c0ff41c701cb9510e2d6e02d8f8d4a14`
  - `waves/RW-40/PM/sec-l02-01/green.log` — `8f076d771844f66805ac5041f3a60c28`
  - `waves/RW-40/PM/sec-l02-01/mutations.log` — `47b949cc4d03b2081e8e6fa2834229e0`
- **риски/вопросы:** нет; существующие 23 теста `net_contract` не менялись и не ослаблялись;
- **следующий шаг:** передать на ревью `SEC-L02-01-FIX` для снятия `RETURN` по `QUALITY/SEC-L02-REVIEW.md`.
