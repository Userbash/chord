# MERGE-MANIFEST — что реализовано и что входит в окно

Дата: 2026-10-24. PM. Основание: `MERGE-POLICY.md`, `TASKS.md`, `DECISIONS.md` D-01…D-128.
Важно: **коммитов нет**, дерево dirty. «Окно» — логический batch; координатор должен
включить в integration все accepted файлы, а не только последние карточки.

## 1. Текущее окно (D-128)

| Функция | В окне | Статус |
|---|---|---|
| G-01 READY → рейл | да | accepted |
| C-02 каналы + switch | да | accepted |
| G-03 invite backend/UI | да | accepted |
| G-03a lurker backend/UI | да | accepted |
| G-04 leave backend/UI | да | accepted |
| G-05 меню сервера | да | accepted |
| M-07 ack/read-state backend/UI | да | accepted |
| P-03 typing backend | да | accepted; UI нет |
| PLAT-01/02 | да | accepted |
| QA4-01a/b, QA4-02/03 | да (как QA) | accepted |
| DP-02 | **нет в списке D-128** | accepted |
| L-01 | **нет** | accepted |
| SEC-01/02 | **нет** | accepted |
| Q-TOKEN/G1-01/Q-G2-P1/Q-G4-REG | **нет** | accepted |
| V-08 UI | **нет** | accepted (fake voice) |
| ST-01 | **нет** | accepted |
| PLAT-11 meta | **нет** | accepted |
| QA-IMPACT | **нет** | accepted |

## 2. Реализовано ранее, но не в текущем окне

Это **не «выброшено»**, а baseline, который должен быть в integration:

| Домен | Функции | Evidence |
|---|---|---|
| Auth | login/password, MFA, captcha, token import/restore, logout | D-39…D-62 |
| Messages | send/edit/delete/reply, reactions, search, composer | D-50…D-57, D-64–65 |
| Navigation | QuickSwitcher Ctrl+K | D-59 |
| Settings | persistence, hydration, 38 toggles, тема/density | D-68–69 |
| UI reliability | reply/Escape, error overlay, auth focus-trap, logout-reject | D-66, D-83 |
| Layout | DP-01 breakpoints | D-77 |
| Security/quality | SEC-01/02, G1-01, Q-TOKEN, G1-01-UI | D-96, D-107, D-84, D-86 |
| Quality tooling | walk, cdp, test-impact, PLAT-11 meta | D-90, D-123, D-125, D-127 |

## 3. Реализовано, но не продуктовая функция

| Артефакт | Где |
|---|---|
| C3a тема light/cozy | `THEME-SPEC.md` |
| VOICE spec | `DESIGN/VOICE.md` |
| STREAM spec | `DESIGN/STREAM.md` |
| G6 inventory/report | `PARITY/*` |
| QA audit/strategy | `QUALITY/QA-*` |
| Merge/queue/pipeline docs | `MERGE-POLICY`, `QUEUE`, `PIPELINE` |

## 4. Partial / не готово к merge

| Функция | Состояние |
|---|---|
| P-03 typing UI | backend accepted; UI отсутствует |
| M-07 unread без mentions | не выводится; badge ack нет |
| C-02 history paging | только первая страница |
| G-03 live-коды/TTL | `[A]`, окно |
| G-03a live 204 | backend `[L]`, UI live не проверен |
| V-08 реальный voice | UI fake, backend/live отсутствует |
| ST-01 панели подпунктов | disabled, `[A]` |
| attachments/video/voice session | RE/live-gated |

## 5. Что должен сделать координатор

1. Взять **все accepted** файлы, не только D-128.
2. Включить quality/tooling изменения (`scripts`, tests) — иначе gate не воспроизводится.
3. Не включать partial/blocked как «готовое».
4. После merge: `gate:release`, G6 parity, QA handoff.
5. Коммит/пуш — только координатор.

## 6. Риск

Если integration соберут только по списку D-128, потеряются:
auth, messages, settings, QuickSwitcher, DP-02, L-01, SEC, Q-TOKEN, V-08 UI, ST-01.
Это сломает gate и UI. Поэтому источник истины для merge — **весь accepted set**, а
D-128 — только «новый batch поверх baseline».
