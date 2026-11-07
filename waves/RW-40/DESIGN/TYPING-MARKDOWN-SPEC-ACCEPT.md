# Приёмка дизайн-спецификации: `RW-40` / `G3-TYPING-MARKDOWN-SPEC` (`P-03` + `M-08`)

- **Статус**: `ACCEPTED` (Design Spec #2)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Спецификация**: `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC.md`
- **Проверенные инварианты**: индикатор печати `#typing-indicator` (`TTL=10000ms`, `TYPING_RESEND_MS=8000ms`, русская плюрализация 1/2/3/>3), рендер Markdown (`M-08`: спойлеры `||...||`, упоминания `<@id>`, `<@&role>`, `<#channel>`, `@everyone`/`@here`, `code`/`pre`, `blockquote`), CSS-токены из `rust/gui-spike/ui/tokens.css`, ARIA-роли.
