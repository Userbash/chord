# REVIEW-CHECKLIST — B07 / RW-26

Для независимого ревьюера (codex/REV-1) и SEC-1. Заполняется после batch gate.

## 1. Границы

- [ ] Изменены только файлы карточек: `src/{rtcp,reception,stats,regions}.rs`,
      `tests/{rtcp,reception,stats,regions}_contract.rs` и интеграционный `src/lib.rs`
      (только `pub mod`).
- [ ] `Cargo.toml`/`Cargo.lock` не менялись; внешних зависимостей нет.
- [ ] Golden (`logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep`) не изменялся.
- [ ] Доки до accepted не правились.

## 2. P4-07 — RTCP/RTP/stats

- [ ] `parse_compound` не паникует: все индексы ограничены проверенной длиной;
      `length_words` (u16) не переполняет `usize`; аллокаций по заявленной длине нет.
- [ ] Трейлер SRTCP не путается с пакетом: 16/20 Б после хотя бы одного пакета —
      трейлер; короткий **первый** пакет — `Truncated`.
- [ ] P-бит отвергается явно; compound ограничен `MAX_COMPOUND_PACKETS`.
- [ ] `cumulative_lost` — знаковое 24-битное (мин/макс проверены).
- [ ] `ReceptionStats`: wrap 16-битного seq, реордеринг, дубликаты; джиттер по
      RFC 3550 §6.4.1 совпадает с независимым оракулом 222.141 / 4.628 мс.
- [ ] `RttEstimator`: LSR=0/неизвестный → None; DLSR в 1/65536 с; отрицательный RTT → 0;
      число запомненных SR ограничено.
- [ ] `StatsWindow`: capacity≥1, `loss_delta` только при ≥2 сэмплах, отрицательная
      дельта не «исправляется».
- [ ] В capture тела SR/RR зашифрованы — тесты не выдают шифротекст за семантику.

## 3. P4-11 — регионы

- [ ] JSON-парсер не паникует, лимиты (регионы/endpoint'ы) соблюдены.
- [ ] Endpoint строго `host:port` / `[v6]:port`, порт 1..=65535, без схемы.
- [ ] Дубликаты регионов — ошибка; дубликаты endpoint — схлопываются.
- [ ] Ранжирование воспроизводит engine-лог (frankfurt,rotterdam,paris,milan,brussels),
      tie-break стабилен (geo → лексика).
- [ ] Wire-формат latency-пробы не изобретён; `RegionProbe` — только модель.

## 4. Процесс

- [ ] red-логи с exit code; green targeted; md5 файлов.
- [ ] Мутации: P4-07 ≥ 8, P4-11 ≥ 5; restore по md5.
- [ ] Batch gate: fmt/clippy/workspace/npm — один раз.
- [ ] P0/P1/P2 = 0; P3 — в backlog с ID.
