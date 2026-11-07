# Приёмка карточки V-01-RTC-FIX — Голосовая сессия и переподключение (`discord-voice`)

- **Статус:** ACCEPTED
- **Дата:** 2026-10-30
- **Независимое ревью:** `waves/RW-40/PM/v01-rtc/REVIEW-PASS2.md` — **PASS (`P0=0, P1=0, P2=0`)**
- **Отчёт:** `waves/RW-40/PM/v01-rtc/REPORT-FIX.md`
- **Файлы:**
  - `rust/crates/discord-voice/src/session.rs`
  - `rust/crates/discord-voice/tests/session_contract.rs`
- **Проверки:**
  - `147/147 passed` (`discord-voice`), сохранение эффектов реконнекта на `TransportFailed` и `Oversize`, честная проверка loopback UDP bind.
  - Мутации: `7/7 KILLED` (`5/5` в первом цикле + `2/2` в `REPORT-FIX.md`).
