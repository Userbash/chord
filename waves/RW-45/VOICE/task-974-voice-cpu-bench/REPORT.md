# REPORT — TASK-974: Бенчмарк нагрузки CPU и джиттера в голосовом канале на 25 участников (`scripts/bench-voice-cpu.mjs`)

- **Волна / Карточка:** Wave RW-45 / `TASK-974` (`task_a1742d8e8058`, dispatch `ctx_d4f40a78988a`)
- **Отдел:** `G5 Voice / Platform`
- **Приоритет:** `P1`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 974.1 — Бенчмарк голосового тракта на 25 участников [`scripts/bench-voice-cpu.mjs`](file:///home/user/orca/project/scripts/bench-voice-cpu.mjs):**
   - Реализовано кодирование и разбор открытого заголовка RTP (`encodeRtpHeaderBede`, `parseRtpHeaderBede`: `V=2`, `PT=120` Opus, `48 000 Гц`, кадр `20 мс = 960 сэмплов`, расширение RFC 5285 `0xBEDE` из 2 слов = 8 байт, а также обработка 2 прогревочных 43-байтных DTX-пакетов `0x19f5`/`0x2d6d` из дампа) в строгом соответствии с `reference-oracle/voice-deep.md` (§2.3, §3) и [`rust/crates/discord-voice/src/media/rtp.rs`](file:///home/user/orca/project/rust/crates/discord-voice/src/media/rtp.rs).
   - Реализован крипто-контекст `SrtpAes256GcmRtpSizeCipher` (`aead_aes256_gcm_rtpsize`): 32-байтный `secret_key` из OP 4 `SESSION_DESCRIPTION`, 12-байтный nonce `[trailer_le32, 0..0]`, 24-байтный открытый RTP-заголовок как AAD, 16-байтный тег аутентификации AES-256-GCM, 4-байтный LE трейлер `0x80000000 | counter`, защита от повторов (`RECEIVE_REPLAY_WINDOW = 1024`) и затирание ключа в ОЗУ (`zeroizeKey`).
   - Реализованы трекер приёма `RtpReceptionTracker` по RFC 3550 §6.4.1 / Appendix A.1–A.3, адаптивный джиттер-буфер `AdaptiveVoiceJitterBuffer` (удерживающий задержку воспроизведения `< 25.0 мс`, пик `14.9968 мс`) и микшер `VoiceChannelMixer25` на 25+ участников (до 64) с индивидуальными громкостями `0..=200%`, независимым `streamVolume` и мягким лимитером `softLimitTanh` (`Math.tanh`).
   - Реализован сквозной бенчмарк `runVoiceCpuBenchmark` на 25 пиров (`25 * 50 = 1250 пак/сек` Opus SRTP): измеренная нагрузка CPU составляет `~1.33%` (бюджет `< 5.0%`), задержка джиттер-буфера `14.9968 мс` (бюджет `< 25.0 мс`), межпакетный джиттер RFC 3550 на эталонном оракуле [`fixtures/voice/rtp-pt120-oracle.json`](file:///home/user/orca/project/fixtures/voice/rtp-pt120-oracle.json) (`reference-oracle/voice/voice-udp4.pcap`) равен `222.141 ед. = 4.628 мс`.

2. **Подзадача 974.2 — Контрактный и модульный тест [`tests/unit/voice-cpu-bench-974.test.mjs`](file:///home/user/orca/project/tests/unit/voice-cpu-bench-974.test.mjs) (Strict TDD):**
   - Покрыты все 9 обязательных измерений `TEST-POLICY.md`: позитивные, негативные, граничные (`4.99%` vs `5.00%` CPU, `24.99 мс` vs `25.00 мс` джиттер-буфер, переполнение `sequence 65535 -> 0`, громкость `0..200%`), ошибки аутентификации GCM / подделка AAD / Replay-атака / потери `> 10%` (WebRTC Error 2003), пустые/прогревочные кадры, лимиты `MAX_MIXER_PARTICIPANTS = 64` и коллизии SSRC, восстановление порядка пакетов (`reordered`) и наблюдаемость (`redactVoiceSecrets`, `zeroizeKey`, CLI коды `0 / 1 / 2`).

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-45/VOICE/task-974-voice-cpu-bench/red.log`](file:///home/user/orca/project/waves/RW-45/VOICE/task-974-voice-cpu-bench/red.log) (запуск до создания `scripts/bench-voice-cpu.mjs`):

```text
▶ TASK-974: Бенчмарк нагрузки CPU (<5%) и джиттера (<25мс) в голосовом канале на 25 участников (scripts/bench-voice-cpu.mjs)
  ✖ SUB-974-1: шифрует и расшифровывает RTP Opus (PT=120, 0xBEDE) в режиме aead_aes256_gcm_rtpsize и сверяет раскладку с оракулом fixtures/voice/rtp-pt120-oracle.json (0.767398ms)
  ✖ SUB-974-2: отслеживает потери/переупорядочивание по RFC 3550 и удерживает задержку адаптивного джиттер-буфера < 25 мс (0.274293ms)
  ✖ SUB-974-3: эмулирует звонок на 25 участников (1250 пак/сек Opus SRTP, микшер tanh), подтверждая нагрузку CPU < 5% и джиттер < 25 мс (0.265093ms)
  ✖ SUB-974-4: проверяет граничные условия (4.99% vs 5.00% CPU, 24.99 vs 25.00 мс джиттер, потери >10%) и CLI scripts/bench-voice-cpu.mjs (0.269333ms)
ℹ tests 4 | suites 1 | pass 0 | fail 4
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-45/VOICE/task-974-voice-cpu-bench/green.log`](file:///home/user/orca/project/waves/RW-45/VOICE/task-974-voice-cpu-bench/green.log):

```text
▶ TASK-974: Бенчмарк нагрузки CPU (<5%) и джиттера (<25мс) в голосовом канале на 25 участников (scripts/bench-voice-cpu.mjs)
  ✔ SUB-974-1: шифрует и расшифровывает RTP Opus (PT=120, 0xBEDE) в режиме aead_aes256_gcm_rtpsize и сверяет раскладку с оракулом fixtures/voice/rtp-pt120-oracle.json (4.324131ms)
  ✔ SUB-974-2: отслеживает потери/переупорядочивание по RFC 3550 и удерживает задержку адаптивного джиттер-буфера < 25 мс (0.71893ms)
  ✔ SUB-974-3: эмулирует звонок на 25 участников (1250 пак/сек Opus SRTP, микшер tanh), подтверждая нагрузку CPU < 5% и джиттер < 25 мс (21.169457ms)
  ✔ SUB-974-4: проверяет граничные условия (4.99% vs 5.00% CPU, 24.99 vs 25.00 мс джиттер, потери >10%) и CLI scripts/bench-voice-cpu.mjs (88.73477ms)
ℹ tests 4 | suites 1 | pass 4 | fail 0 | skipped 0
```

---

## 4. Targeted & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/voice-cpu-bench-974.test.mjs`):** `4/4 pass (0 fail, 0 skip)`.
- **TypeScript (`tsc`):** `0 errors` в `scripts/bench-voice-cpu.mjs` и `tests/unit/voice-cpu-bench-974.test.mjs`.
- **Мутационный гейт ([`waves/RW-45/VOICE/task-974-voice-cpu-bench/mutation.log`](file:///home/user/orca/project/waves/RW-45/VOICE/task-974-voice-cpu-bench/mutation.log)):**
  1. `M1` (`participantsCount: 25 -> 24`) — **KILLED**
  2. `M2` (`totalPacketsPerSecond: 1250 -> 1000`) — **KILLED**
  3. `M3` (`RTP_OPUS_PAYLOAD_TYPE: 120 -> 111`) — **KILLED**
  4. `M4` (`metrics.cpuLoadPct >= MAX_CPU_LOAD_PCT -> >`) — **KILLED**
  5. `M5` (`metrics.jitterBufferDelayMs >= MAX_JITTER_BUFFER_DELAY_MS -> >`) — **KILLED**
  - Итог: **5/5 мутаций убито (100% kill rate)**, оригинал файла побайтно восстановлен и проверен по MD5 (`81b7afa14f7c5149ce6988c2a8b234c1`).

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `scripts/bench-voice-cpu.mjs` | `81b7afa14f7c5149ce6988c2a8b234c1` |
| `tests/unit/voice-cpu-bench-974.test.mjs` | `7ffd09b0974ca1eb34aede9b0953d207` |
| `waves/RW-45/QUALITY/VOICE-CPU-BENCH-REPORT.md` | `82dda194c1db0c0f90ba951188d48610` |
| `waves/RW-45/VOICE/task-974-voice-cpu-bench/bench-summary.json` | `48e70716978e529f6023650369264521` |

---

## 6. Риски и замечания

- Ключи сессии `secret_key` затираются в памяти (`zeroizeKey`) после завершения прогона и маскируются в логах (`redactVoiceSecrets`).
- Блокеров нет (`P0 = 0`, `P1 = 0`).
