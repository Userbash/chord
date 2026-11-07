# Приёмка дизайн-спецификации: `RW-40` / `G3-NOTIFICATIONS-SPEC` (`N-05`, Pass 2)

- **Статус**: `ACCEPTED` (Design Spec #6)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Спецификация**: `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md`
- **RE-оракул**: `waves/RW-40/NOTIFICATIONS-RE.md`
- **Независимое ревью**: `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC-REVIEW-PASS2.md` — `PASS` (`P0=0, P1=0, P2=0`)

## Проверенные инварианты

1. **Иерархия и каскад уведомлений**: `channel -> category -> guild`, подавление `suppress_everyone` / `suppress_roles`, 6 пресетов таймера мьюта (`900`, `3600`, `10800`, `28800`, `86400`, `-1`).
2. **Матрица упоминаний (`P1` закрыт)**: звуковые и desktop-уведомления для `@user`, `@role`, `@everyone`/`@here` требуют `!allowNoMessages` (`!isMuted && resolvedMessageNotifications !== 2`), при этом счётчик бейджей упоминаний продолжает расти при `NO_MESSAGES`.
3. **Клавиатура и ARIA (`P2` закрыт)**: roving `tabindex` + `ArrowLeft`/`ArrowRight` для радио-кнопок строк `.notif-override-table`, `Space` для чекбоксов, открытие `#channel-notif-menu` с клавиатуры и возврат фокуса по `Escape`.
4. **CSS custom properties**: все 66 ссылок `var(--...)` присутствуют в `rust/gui-spike/ui/tokens.css`.
