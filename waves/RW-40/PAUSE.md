# PAUSE — снята 2026-10-24 (возобновление)

Статус: **разработка возобновлена** по команде владельца.
Качество/стратегия: `waves/RW-40/QUALITY/STRATEGY.md`, baseline `QUALITY/BASELINE-QUALITY.md`.
Исторический снимок паузы сохранён ниже.

## 1. Статус

- Активных воркеров, subagent-сессий, `cargo`, node-тестов и live-окон нет.
- Новые задачи не запускаются.
- Коммитов нет; дерево остаётся dirty для координатора.

## 2. Состояние на паузу

| Проверка | Результат |
|---|---|
| frontend (dom/model/bench/features/auth) | **212/0** |
| core `cargo test -p discord-core` | **574/0** |
| gui-spike `cargo test` | **161/0** |
| workspace `cargo test --workspace` | **1112/0** |
| meta / unit / smoke | **39/0 · 381/0 · 22/0** |
| e2e | 4 теста, **4 skip** — не pass |
| typecheck | 0 |
| geometry | 31/31 |
| Последний `gate:release` | **passed_with_skips** |
| Live READY/send | **не выполнялись** |

## 3. Принято до паузы

- **A1–A6** — auth/login/captcha/MFA/logout/security.
- **B1–B8** — отправка, реакции, edit/delete/reply, поиск, QuickSwitcher, настройки.
- **B7a/B7b** — guild-search backend/UI.
- **UI-FIX-1** — P1/P2 из Codex-review.
- **C3a** — тема light/cozy (в дампе light-темы нет; 36 [T] / 21 [A]).
- **N1-D** — дизайн меню сервера (join/leave разделены, `confirmed` удалён).
- **G-03a** — backend входа в сервер `PUT ...?lurker=true` → 204 (D-76).
- **DP-01** — адаптив shell 1600/1200/960/720 (D-77/D-78).
- **G6** — инвентарь **91 функции**: 14 implemented, 40 partial, 37 absent;
  P0 5 / P1 32 / P2 40 / P3 14 (D-75, D-76).

## 4. Что не завершено

- **P0 live READY/send** — нужен тестовый аккаунт и окно PM.
- **DP-02** — D-04: 26/48/+22, member 42, height index.
- **G-04** — выход с сервера (`[S]`, коды `[A]`).
- **G-01/L-01** — READY→рейл, reconnect/offline UI.
- **G-03a UI** — вход в сервер из preview.
- **N1-02** — UI меню сервера по принятой спеке.
- **Voice MVP** — RTC-сессия, UDP, SRTP, Opus, устройства/PTT, UI.
- **Video/Go Live**, **attachments** (blocked), **pins/threads**, **DND**.
- **D1–D5** — e2e fixture, QA-матрица, coverage, live, merge gate.

## 5. Как продолжить

1. Прочитать `waves/RW-40/PAUSE.md`, `TASKS.md`, `PARITY/G6-CHARTER.md`.
2. Проверить HEAD и активные процессы; не запускать несколько cargo.
3. Первый поток: **DP-02** (G2/G4) + **G-04** (G1) — файлы не пересекаются.
4. Затем **G-01/L-01**, затем **N1-02 UI**.
5. **P0 live** — только по согласованному окну, 1 окно на приёмку.
6. Полный gate — после batch; не ослаблять тесты и не обходить `test-policy`.

## 6. Правила при возобновлении

- Не откатывать принятые D-75…D-78 и G-03a/DP-01.
- Функция существует только при `[L]/[S]/[D]`; `[M]/[A]` — не доказательство.
- `confirmed` из `rest-static.json` не использовать.
- Тесты не ослаблять: `.only/.skip/todo`, удаление assert, обход gate — P0.
- Live/секреты — только по окну; чужие данные не захватывать.
- Коммит/пуш — координатор, не PM.
