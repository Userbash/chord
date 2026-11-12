# B2a-ATTACH-TIER-FIX — независимое ревью pass 2

**Вердикт: RETURN (P1).** Повторный фикс не закрывает контракт лимита для базового уровня.

## Findings

1. **P1 — неверный базовый лимит.** Независимый оракул в `waves/RW-40/reference-oracle-REA.md:122-125` требует `Math.max(0x1400000, tierLimit)`, то есть минимум 20 MiB; для `NONE`/`TIER_1` с tier limit 10 MiB валидная граница равна 20 MiB. Реализация возвращает 10 MiB для `None` и `Tier1` (`rust/gui-spike/src/attachments.rs:46-64`) и применяет её до сети (`rust/gui-spike/src/attachments.rs:376-393`). Файл `waves/RW-40/ATTACH-RE.md:58-59` повторяет противоречивое описание tier-значений, но не отменяет явную формулу оракула.

Из-за этого файл размером 10 MiB + 1 байт до 20 MiB ошибочно получает `EntityTooLarge`; тесты проверяют именно ошибочное значение и потому не являются независимым подтверждением.

## Evidence / TEST-POLICY

- RED: `waves/RW-40/PM/b2a-attach/red-fix.log` — exit 101, 15 passed / 2 failed.
- GREEN: `waves/RW-40/PM/b2a-attach/green-fix.log` — targeted 17 passed, exit 0; также записаны clippy/fmt проверки.
- MUTATION: `waves/RW-40/PM/b2a-attach/mutations-fix.log` — 4/4 KILLED, что подтверждает различение tier 2/3, но не проверяет независимый 20 MiB base floor.
- По TEST-POLICY.md §3, §5 тестовый оракул должен быть независимым; текущие assertions закрепляют неверный base floor. Приёмка возможна после исправления контракта/теста и нового red → green → mutation цикла.

Cargo в рамках этого ревью не запускался согласно заданию.
