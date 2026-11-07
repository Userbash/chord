# ACCEPT — task-291 (`G5 Voice L2 / P1`, `RE-22-01`: `ConnectionQuality`, `PingHistory` (199/20), `RtcConnectionStatsSnapshot`)

- **PR:** `PR-170` (`branch:feature/task-291-voice-stats-re22`)
- **Статус PM:** `APPROVED` (`Ready for Technical Review` -> `TR-198A`)
- **Проверка DoD:**
  1. В `rust/crates/discord-voice/src/stats.rs` по RE-оракулу (`RE-GAP-22`, модули `613681`, `206959`, `978171`) реализованы константы (`250/500 мс`, `5%/10%`), `ConnectionQuality`, `PingSample`, `PingHistory` (лимит `199`, среднее по первым `min(len, 20)`, `ping_bad_count` при `> 500`, 3 последовательных таймаута -> `4000 мс`), `compute_outbound_loss_rate_pct`, `short_hostname` и `RtcConnectionStatsSnapshot`.
  2. Все тесты `discord-voice` (`36/36` lib + контрактные таргеты) и `cargo clippy --tests -- -D warnings` проходят без предупреждений.
