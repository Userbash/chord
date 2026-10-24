# STUB-TRACKER — ход замены заглушек

Дата: 2026-10-24. PM. Обновлять после каждого accepted batch (`D-*`).
Статусы: `open` · `in work` · `blocked` · `accepted` · `deferred`.
Ссылки: `STUB-MAP.md`, `STUB-IMPLEMENTATION-PLAN.md`, `PARITY-MATRIX.md`.

## Сводка

| Показатель | Значение |
|---|---|
| Всего карточек | 35 |
| in work | 0 |
| accepted (частично) | 4 (STUB-02, STUB-04, STUB-06, STUB-32) |
| blocked | 1 (STUB-13) |
| open / queued | 30 |
| P1 | 22 |
| P2 | 11 |
| P3 | 2 |

## Phase 0 — trust/live

| ID | Задача | Owner | Статус | Next / evidence |
|---|---|---|---|---|
| STUB-31 | `walk` CLI | G4 | **accepted (D-123)** | e2e CHROME_BIN |
| STUB-32 | `cdp eval/clicktext/list/shot` | G4 | **accepted (D-90)** | e2e нужен CHROME_BIN |
| STUB-33 | e2e fixture 4 skip | G4 | open | после 31/32; skip≠pass |
| STUB-34 | gate/workspace | G4 | open | `cargo test --workspace`, CLI coverage |
| STUB-35 | token behavior test | G1+G4 | open | keyring→Network behavior |
| P0-01 | live READY | PM+G1+G4 | blocked | токен+окно |
| P0-02 | live send | PM+G1+G4 | blocked | после P0-01 |

## Phase 1 — servers/channels/text

| ID | Задача | Owner | Статус | Next |
|---|---|---|---|---|
| STUB-01 | rail из READY | G2+G1 | **accepted (D-112)** | G-01b restore |
| STUB-02 | guild menu | G2+G3 | **accepted (D-92)** | invite/owner C2 |
| STUB-04 | enter guild UI | G2+G1 | **accepted (D-128)** | live 204 |
| STUB-05 | invite join | G1+G2 | **accepted (D-116/D-122)** | live codes `[A]`; G-03a UI |
| STUB-06 | leave server | G1+G2 | **accepted (D-89/D-92)** | invite/owner C2 |
| STUB-08 | channel tree | G2+G1 | open | C-01 |
| STUB-09 | channel switch | G2+G1 | **accepted (D-120)** | history paging |
| STUB-14 | search filters | G2+G1 | partial | 3 server + 6 local |
| STUB-21 | read state/unread | G1+G2 | **accepted (D-126)** | badge/live |
| STUB-22 | typing/presence | G1+G2 | open | P-01…03; G6 top-5 |
| DP-02 | 26/48/+22, member 42 | G2+G4 | **accepted (D-102)** | legacy tokens P2 |

## Phase 2 — completeness

| ID | Задача | Owner | Статус | Next |
|---|---|---|---|---|
| STUB-03 | create server | G1+G2+G3 | open | RE endpoint `[D]/[A]` |
| STUB-07 | invites | G1+G2 | open | G-06 |
| STUB-10 | pins/threads | G2+G1 | open | M-10/11 |
| STUB-11 | inbox | G2+G1 | open | N-03 |
| STUB-12 | gift | G6+G2 | open | RE (нет G6-строки) |
| STUB-13 | attachments | G1+G2 | **blocked** | upload-контракт |
| STUB-16 | profile popover | G2 | open | MB-03 |
| STUB-19 | settings sections | G2+G3 | partial | ST-01…08 |
| STUB-28 | notifications | G5+G2 | partial | N-01/02 |
| STUB-29 | settings sync | G1 | partial | ST-08 |
| STUB-30 | i18n | G1+G2 | partial | ST-04 |

## Phase 3+ — voice/video/platform

| ID | Задача | Owner | Статус | Next |
|---|---|---|---|---|
| STUB-26 | voice session | G1+G5 | partial | V-01…06, live voice |
| STUB-17 | mute/deafen/PTT | G5+G2 | open | V-07; VOICE spec D-94 |
| STUB-18 | voice UI | G2+G3 | **UI accepted (D-115)** | voice live-gate |
| STUB-27 | video/Go Live | G5+G1 | **design accepted (D-109)** | live/consent blocked |
| STUB-20 | QR/passkey | G1+G5 | open | A-04/05 |
| STUB-15 | live members | G1+G2 | open | MB-01 |
| STUB-23 | live history | G1+G2 | partial | M-01 |
| STUB-24 | live guild list | G1+G2 | **accepted (D-112)** | live E3 |
| STUB-25 | member list | G1+G2 | open | MB-01 |

## Правило обновления

1. После accepted batch: обновить строку + ссылку на D-*.
2. `in work` без evidence 2 batch → вернуть/декомпозировать.
3. `blocked` — указать конкретный контракт/окно, не «нет времени».
4. Удалять заглушку из карты только после G6 parity + PM accept.
