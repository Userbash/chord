# Приёмка карточки STUB-05-INVITES-BACKEND — Создание приглашений канала (`POST /channels/{id}/invites`)

- **Статус:** ACCEPTED
- **Дата:** 2026-10-30
- **Независимое ревью:** `waves/RW-40/PM/stub05-invites-be/REVIEW.md` — **PASS (`P0=0, P1=0`)**
- **Отчёт:** `waves/RW-40/PM/stub05-invites-be/REPORT.md`
- **Файлы:**
  - `rust/gui-spike/src/invites.rs`
  - `rust/gui-spike/tests/invites_contract.rs`
- **Проверки:**
  - `20/20 passed` (`invites_contract`), валидация `channel_id`, RE-таблиц `max_age` (`0, 1800, 3600, 21600, 43200, 86400, 604800`) и `max_uses` (`0, 1, 5, 10, 25, 50, 100`), сериализация `flags` и маппинг ошибок (`429`, `403`, `404`).
  - Мутации: `5/5 KILLED`.
