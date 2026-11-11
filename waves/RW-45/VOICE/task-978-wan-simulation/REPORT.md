# Отчёт воркера — TASK-978 (`G5 Voice / Platform`)

- **Карточка:** `TASK-978` — Эмуляция WAN профилей (`150ms` RTT, `5%` loss, `30ms` jitter), NACK/RTX и Opus PLC
- **Волна:** `RW-45` (`VOICE` / `QUALITY`)
- **Ветка:** `feature/sprints-1-5-build-004`
- **Статус:** `Ready for Product Review`

---

## 1. Сделано

1. **Строгий TDD (RED → GREEN → Mutation 5/5):**
   - Сначала написан контрактный тест `tests/unit/wan-network-profiles-978.test.mjs` (`4` подтеста `SUB-978-1..4`), проверяющий профиль `WAN 3G/LTE` (`150ms` RTT, `5.0%` loss, `30ms` jitter), сверку с эталонными RE-оракулами (`fixtures/voice/rtp-pt120-oracle.json` и `fixtures/voice/rtc-frames.jsonl`), непрерывность голоса за счёт Opus PLC и In-Band FEC (`LBRR`), побитовую сборку/разборку `RTCP Generic NACK` (`PT=205, FMT=1`, RFC 4585) с битовой маской `BLP` (`16` seq) и `RTP RTX` (`RFC 4588`, OSN `2` байта, `PT=102 -> 101`, `SSRC 17845 -> 17844`), 100% восстановление видео-ключевых кадров, граничные пороги `5.0%` vs `10.0%` vs `10.1%` (`Discord WebRTC Error 2003 — High Packet Loss > 10%`), валидацию SRTP `aead_aes256_gcm_rtpsize` и CLI-коды возврата (`0`/`1`).
   - Зафиксирован `RED`-прогон до создания `scripts/wan-simulation.mjs` (`waves/RW-45/VOICE/task-978-wan-simulation/red.log` — `4 fail`).
2. **Стенд эмуляции WAN (`scripts/wan-simulation.mjs`):**
   - Реализован детерминированный эмулятор канала `WanLinkEmulator` (`SplitMix32`, `rttMs=150`, `packetLossPct=5.0`, `jitterMs=30`), консиллер `OpusPlcConcealer` (Pitch-Paced PLC с коэффициентом затухания `0.85^n` и In-Band FEC `LBRR` из пакета `N+1` при доставке до дедлайна воспроизведения), кодек `RTCP Generic NACK` (`buildRtcpNack`/`parseRtcpNack`) и `RTP RTX` (`buildRtxPacket`/`restoreRtxPacket`) с поддержкой One-Byte заголовка `0xBEDE`, а также верификация RE-оракулов `verifyWanOracleParity` и CLI-раннер.
   - Сгенерированы отчёт `waves/RW-45/QUALITY/WAN-SIMULATION-REPORT.md` и JSON-сводка `waves/RW-45/VOICE/task-978-wan-simulation/wan-summary.json`.

---

## 2. RED-прогон

Команда:
```sh
node --test tests/unit/wan-network-profiles-978.test.mjs
```
Результат (`waves/RW-45/VOICE/task-978-wan-simulation/red.log`):
- `tests 4 | pass 0 | fail 4` (`ERR_MODULE_NOT_FOUND: Cannot find module '.../scripts/wan-simulation.mjs'`).

---

## 3. GREEN-прогон и Targeted Gate (`T0`)

Команды:
```sh
node scripts/wan-simulation.mjs
node --test tests/unit/wan-network-profiles-978.test.mjs
```
Результат (`waves/RW-45/VOICE/task-978-wan-simulation/green.log`):
- `[TASK-978] WAN Simulation PASS: RTT=150ms, loss=5%, jitter=30ms, continuity=100%, NACK/RTX=100%`
- `tests 4 | suites 1 | pass 4 | fail 0` (`133.70 ms`)
- Непрерывность голоса (`audioContinuityPct`): `100.0%` (`underrunFrames = 0`, `fecRecoveredFrames = 14`, `plcConcealedFrames = 11` на `500` фреймов при `5.0%` потерь)
- Восстановление ключевых кадров (`keyframeRecoveryPct`): `100.0%` (`20/20` ключевых кадров, `27/27` потерянных пакетов восстановлено через `RTCP NACK PT=205` + `RTP RTX SSRC=17845`)

---

## 4. Мутационное тестирование (`5/5 KILLED`)

Лог: `waves/RW-45/VOICE/task-978-wan-simulation/mutation.log`
- `M1` (`WAN_DEFAULT_PROFILE.rttMs: 150 -> 120`): **KILLED** (`SUB-978-1`, `SUB-978-4`)
- `M2` (`WAN_DEFAULT_PROFILE.packetLossPct: 5.0 -> 2.0`): **KILLED** (`SUB-978-1`, `SUB-978-4`)
- `M3` (`RTCP_RTPFB_PT: 205 -> 204`): **KILLED** (`SUB-978-3`)
- `M4` (`evaluateWanSimulationResult: packetLossPct > 10.0 -> >= 10.0`): **KILLED** (`SUB-978-4`)
- `M5` (`recoverMissingVideoViaRtx: parsedNack.lostSequences -> .slice(1)`): **KILLED** (`SUB-978-3`, `SUB-978-4`)
- Итог: **`5/5` мутаций убито (`100%`)**, исходный файл восстановлен побайтово (`MD5 match=true`).

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `scripts/wan-simulation.mjs` | `56cced9b7dc6fdb4cee1014baa9bc925` |
| `tests/unit/wan-network-profiles-978.test.mjs` | `9c853a1fdb3e22ad14385bcc21e57611` |
| `waves/RW-45/QUALITY/WAN-SIMULATION-REPORT.md` | `41059d58d9e4bcccfe826f6e91c01203` |
| `waves/RW-45/VOICE/task-978-wan-simulation/wan-summary.json` | `54f5dd094789124d29e685b4f9b2ce50` |
| `waves/RW-45/VOICE/task-978-wan-simulation/red.log` | `bcd83dd07f6c1d61fb1eed3293065400` |
| `waves/RW-45/VOICE/task-978-wan-simulation/green.log` | `f0e29325bebc98af50efe550a5531ecf` |
| `waves/RW-45/VOICE/task-978-wan-simulation/mutation.log` | `e75b861a389b50a55d8e7a0db335c543` |

---

## 6. Риски и ограничения

- Эмуляция канала использует детерминированный ГПСЧ (`SplitMix32`) с фиксированным зерном `0x97802026`, чтобы исключить флаки в CI и гарантировать воспроизводимость `5.0%` потерь (`25/500` пакетов) и джиттера `±30ms`.
- Никакие чужие файлы в рабочем дереве не затрагивались.
