# STUB-27-CORE-DISPATCH-INTEGRATION — независимое ревью

**Вердикт: RETURN (P2, evidence).** Функционального дефекта в просмотренном мосте не найдено, red/green evidence согласуется с контрактом стрима, но обязательное mutation proof отсутствует.

## Проверка реализации

- `StreamDispatchKind` содержит четыре канонических имени и точное сопоставление (`rust/crates/discord-core/src/dispatch.rs:145-195`).
- `DispatchEvent` сохраняет 21 старый тип и маршрутизирует `STREAM_*` через `parse_dispatch`, включая диагностический путь (`dispatch.rs:236-291`); верхнеуровневые маршруты находятся в `:305-349`.
- Тесты `dispatch_types.rs` проверяют disjoint-наборы, fixture inbound, негативные кадры и reducer replay; это соответствует `STREAM-GATEWAY-CONTRACT.md`.

## Evidence / TEST-POLICY

- RED: `waves/RW-40/PM/stub27-dispatch/red.log` — exit 101, отсутствовали новые exports/API.
- GREEN: `waves/RW-40/PM/stub27-dispatch/green.log` — `dispatch_types` 4 passed и пакет 583 passed, exit 0; отчёт также содержит clippy/rustfmt exit 0.
- **P2 — mutation artifact отсутствует.** В `REPORT.md` перечислены новые routing invariants и red/green (`REPORT.md:12-45`), но нет mutation log, mutant ids или KILLED/MISSED результатов. По TEST-POLICY.md §3 и §9 новая маршрутизация не принимается только по green.

Запросить mutation run на сохранение 21 canonical types, disjoint stream set, op-0 rejection и diagnostic counters; затем приложить immutable log. Cargo в рамках этого ревью не запускался согласно заданию.
