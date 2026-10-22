# BATCH RW-26 — B07: голосовой транспорт, углубление (headless)

Дата: 2026-10-22. Team Lead: TL-1. Tech Lead: TL-2.
Статус: **frozen** (контракты заморожены до кода).

## 1. Состав batch

| Карточка | Seat | Файлы (только эти) | Зависит от | Оракул |
|---|---|---|---|---|
| P4-07 Статистика | CORE-V1 (TL-2) | `discord-voice/src/{rtcp,reception,stats}.rs`, `tests/{rtcp,reception,stats}_contract.rs` | P4-02 `[x]`, P4-03 `[x]` | `waves/RW-26/research/RE-VOICE-STATS.md`, `fixtures/rtcp-voice-udp4-server.{bin,json}`, `fixtures/rtp-udp4-client.json`, RFC 3550 |
| P4-11 Регионы/латентность | CORE-V2 (codex) | `discord-voice/src/regions.rs`, `tests/regions_contract.rs` | P4-01 `[x]` | `waves/RW-26/research/RE-REGIONS.md`, `fixtures/regions-rtc.json`, engine-log `ranked regions` |

Общий файл (только TL-2): `discord-voice/src/lib.rs` — добавляет `pub mod`.
Запрещено: чужие модули и тесты, `Cargo.toml`/`Cargo.lock`, доки, golden (`logs/`,
`functions/`, `evidence/`, `/home/sanya/re-deep`), сеть, `git commit/push`, `unsafe`.

## 2. Замороженный контракт

- P4-07: `rtcp::parse_compound` (SR/RR + SRTCP-трейлер), `reception::ReceptionStats`
  (RFC 3550 loss/jitter по открытым RTP-заголовкам), `stats::{RttEstimator, StatsWindow}`.
  Полные сигнатуры — `TZ-W26a.md` §3.
- P4-11: `regions::{RegionEndpoint, parse_rtc_regions, RegionLatency, RegionProbe,
  plan_probes, ranked_region_names, select_measured_region}`; wire-формат пробы **не
  изобретается** (UNKNOWN в RE-REGIONS §2).
- Общие инварианты: никаких паник на недоверенном вводе, лимиты на размер/число блоков,
  ошибки типизированы, `Debug` без секретов.

## 3. Кейсы (red-first)

| ID | Карточка | Проверка | Ожидание |
|---|---|---|---|
| r01 | P4-07 | реальный RR 32 Б + tag 16 Б | 1 RR, 1 блок, `trailer_len=16` |
| r02 | P4-07 | реальный SR 28 Б + tag+counter 20 Б | 1 SR, `trailer_len=20`, NTP сходится |
| r03 | P4-07 | синтетический RR с блоком | fraction/cumulative(24-bit signed)/EHS/jitter/LSR/DLSR |
| r04 | P4-07 | compound RR+SDES | 2 пакета, трейлер 0 |
| r05 | P4-07 | V=1, обрезанная длина, P-бит | типизированные ошибки, без паники |
| r06 | P4-07 | 8 RTP-заголовков из fixture | jitter 222.141 ед. / 4.628 мс, loss 0 |
| r07 | P4-07 | wrap seq, reorder, duplicate | loss/duplicate по RFC A.3 |
| r08 | P4-07 | RTT по SR/RR паре | `(recv-sent) - dlsr/65.536`, LSR=0 → None |
| r09 | P4-07 | окно статистики | capacity, latest, avg/max, loss delta |
| r10 | P4-11 | парсинг реального `regions-rtc.json` | 5 регионов × 5 endpoint'ов |
| r11 | P4-11 | geo-порядок READY + измеренные RTT | frankfurt,rotterdam,paris,milan,brussels |
| r12 | P4-11 | tie/unknown/manual | geo-порядок, затем лексика; manual только из доступных |
| r13 | P4-11 | план проб/таймаут | детерминированный выбор endpoint, timeout |

Бюджет: P4-07 L → 16–24 теста; P4-11 M → 10–16 тестов. Мутации: P4-07 8–12, P4-11 5–8.

## 4. Команды

```sh
# worker targeted (свой target-dir, чтобы не блокировать соседа)
CARGO_TARGET_DIR=/home/sanya/.cache/discord-lab-target-rw26a \
  cargo test --offline -p discord-voice --test rtcp_contract --test reception_contract --test stats_contract
CARGO_TARGET_DIR=/home/sanya/.cache/discord-lab-target-rw26b \
  cargo test --offline -p discord-voice --test regions_contract

# batch gate (один раз, TL-2/OPS-1, после green обоих)
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline --workspace
npm run gate:worker
```

## 5. Definition of done

- [ ] red-лог с exit code; green targeted;
- [ ] файлы + md5; `TEST-CORRECTIONS.md` при расхождении;
- [ ] мутации пойманы, продукт восстановлен по md5;
- [ ] batch gate passed;
- [ ] независимое ревью (codex + TL-1), P0/P1/P2=0;
- [ ] `accepted` + `db.jsonl` + `STATE.md` + STATUS/ROADMAP/TASKS/map один раз.
