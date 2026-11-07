# STUB-16-MEMBER-PROFILE-POPOVER-UI (MB-03) — независимое ревью

**Вердикт: RETURN (P1; процессный P2).** Реализация и тесты закрепляют несколько значений, расходящихся с замороженной спецификацией.

## Findings

1. **P1 — неверный публичный id и `aria-controls`.** Спецификация требует `id="user-profile-popover"` и такое же значение `aria-controls` (`waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md:196-208`). Код и тесты используют `member-profile-popover` (`rust/gui-spike/ui/features/members.js:118-132`, `:632-643`, `:869-872`; тест `tests/unit/gui-spike-regressions.test.mjs:311-321`). Это ломает потребителей, которые находят компонент по контрактному id.

2. **P1 — строка участника не является требуемым trigger.** Спецификация допускает `role="button"` либо кнопку внутри строки (`PINS-THREADS-PROFILE-SPEC.md:196-202`). Реализация оставляет `role="listitem"` (`members.js:118`) и добавляет только `tabindex`, поэтому семантика интерактивного trigger не соответствует контракту.

3. **P1 — обязательное закрытие при смене канала/экрана отсутствует.** Обработчики поповера покрывают Escape, кнопку и outside click (`members.js:579-625`), но не подписываются на изменение `body[data-active-screen]` или active channel, требуемое `PINS-THREADS-PROFILE-SPEC.md:296-305`. Открытый поповер может остаться привязанным к старой строке.

4. **P2 — loading state неполный.** В `renderMemberProfileState` loading создаёт только banner, close button и live-текст (`members.js:659-681`); нет avatar placeholder и двух name skeleton blocks, обязательных по спецификации (`PINS-THREADS-PROFILE-SPEC.md:263-267`).

5. **P2 — mutation evidence не воспроизводимо.** Отчёт заявляет ручные мутации, но в каталоге нет mutation log или списка мутантов (`waves/RW-40/PM/stub16-profile-ui/REPORT.md:19` и перечисленные артефакты карточки). TEST-POLICY.md §3/§9 требует артефакт для новых a11y/state invariants.

## Evidence

- RED: `waves/RW-40/PM/stub16-profile-ui/red.log` — exit 1, 8 failures.
- GREEN: `waves/RW-40/PM/stub16-profile-ui/green.log` — 11 passed / 0 failed, skipped 0, todo 0, exit 0; отчёт указывает targeted 11/11, 100/100 и 29/29.
- Зеленые тесты повторяют неверный `member-profile-popover` контракт, поэтому не являются независимой проверкой SPEC.

Cargo в рамках этого ревью не запускался согласно заданию.
