# Приёмка дизайн-спецификации: `RW-40` / `G3-AT03-RENDER-SPEC`

- **Статус**: `ACCEPTED`
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Спецификация**: `waves/RW-40/DESIGN/ATTACHMENTS-RENDER-SPEC.md` (`md5: 9e93ec4a3de64d96c7e7b7c296fc06fa`)
- **RE-оракул**: `waves/RW-40/ATTACH-RENDER-RE.md`
- **Независимое ревью**: `waves/RW-40/DESIGN/ATTACHMENTS-RENDER-SPEC-REVIEW.md` — `PASS` (`P0=0, P1=0, P2=0`)

## Проверенные инварианты

1. Полное покрытие типов вложений (`image`, `video`, `audio`/`voice message` с `waveform` + `duration_secs`, `file`), фильтрация `IS_THUMBNAIL = 2`, бейдж `IS_REMIX = 4`, спойлеры `IS_SPOILER = 8` и префикс `SPOILER_`, `MessageFlags.IS_VOICE_MESSAGE = 8192`.
2. Все CSS custom properties строго соответствуют существующим токенам `rust/gui-spike/ui/tokens.css`.
3. DOM-контракт двух прямых дочерних узлов `.row` (`.meta`, `.text`) сохранён.
