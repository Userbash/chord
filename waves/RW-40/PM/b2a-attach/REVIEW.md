# REVIEW — B2a-ATTACH-BACKEND

Дата: 2026-10-30. Режим: независимое read-only ревью; cargo не запускался.

## Вердикт

**RETURN. P0=0, P1=1, P2=0.**

### P1 — жёсткий лимит 10 MiB не соответствует tier-контракту

`rust/gui-spike/src/attachments.rs:31-35,324-338` без контекста guild tier отклоняет
любой файл больше 10 MiB до сети. В `waves/RW-40/RE-DEEP-REA.md:§1.5` оракул также
фиксирует tier T2=50 MiB, T3=100 MiB и базовое правило `max(20 MiB, tierLimit)`.
Следовательно, валидный файл для T2/T3 (и размер между 10 и 20 MiB по базовому
правилу) будет ошибочно возвращён как `EntityTooLarge`; тест
`rejects_oversized_file_locally_with_entity_too_large` закрепляет только неверный
hard-code. Нужны tier-aware лимит или явное подтверждённое ограничение scope,
после чего независимый тест должен покрывать границы каждого поддерживаемого tier.

## Проверки

- red: `red.log` — 14 failed / 1 passed, exit 101 до реализации.
- green: `green.log` — 15+2 теста, fmt и clippy exit 0.
- mutations: `mutations.log` — 6/6 killed, restore md5 совпал.
- текущие md5 совпали с REPORT: `attachments.rs` c2e106..., контрактный тест
  cdc8cb..., `lib.rs` f8b5af....
- Ослаблений тестов не обнаружено; `cargo` в ревью не запускался.

## Примечание

Маршрут, wire-ключи, error mapping и связывание слотов по `id` согласуются с
RE; PUT/resume корректно оставлены за B2b.
