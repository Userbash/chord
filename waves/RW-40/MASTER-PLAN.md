# MASTER-PLAN — план, пул задач и распределение по отделам

Дата: 2026-10-24. PM. Основание: `TASKS.md`, `QUEUE.md`, `PARITY/G6-BATCH-MERGE-01.md`,
`G6-SYNC-01.md`, `MERGE-MANIFEST.md`, `QUALITY/*`, `STUB-*`.

## 0. Текущая точка

- 91 функция: **21 implemented / 36 partial / 34 absent**;
- gate **passed_with_skips** (unit 643, workspace 1103/12 ignored, e2e 4 skip);
- G6 parity: **deferred**;
- offline merge: **не выполнен**, P-03/M-06 закрыты;
- live E3: **0**, READY/send не проверены;
- voice: сильные transport/crypto primitives, но нет Opus/devices/UI/live;
- video/Go Live: нет.

## 1. Критический путь

```
L-02 dispatch→UI
  → G6 re-parity
  → merge всех accepted (MERGE-MANIFEST)
  → QA batch (e2e/browser/матрица)
  → live READY/send
  → voice MVP
  → video/Go Live
```

P0 live — выше всех, но требует окна. Offline merge не ждёт live; live — отдельный этап.

## 2. Приоритеты

| Класс | Содержание |
|---|---|
| **P0** | live READY/send, секреты, блокирующий parity |
| **P1A** | L-02, QA-E2E/dedup, PLAT-11a, merge/parity |
| **P1B** | C-02 paging, G-05 2/10, M-07 unread, G-01b, M-06 live, G-03 live, voice MVP |
| **P2** | notifications, settings, pins/threads, Q-02/Q-04, video, attachments (blocked) |
| **P3** | QR/passkey, tray/updater, clips/gift |

## 3. Пул задач (ready/queued)

### P0

| ID | Задача | Owner | Размер | DoD |
|---|---|---|---|---|
| P0-01 | live READY | PM+G1+G4 | M | READY+guild/channel, token no-log, окно 1 |
| P0-02 | live send | PM+G1+G4 | S | сообщение в канале, 401/429, окно |
| P0-03 | secret/log audit live | G4b | S | grep token/password/key=0 |

### P1A — доверие/parity

| ID | Задача | Owner | Размер | Зависит |
|---|---|---|---|---|
| L-02 | dispatch→UI (READY/CHUNK/PRESENCE/REACTION) | G1+G2 | L | G6-D-03 |
| QA-E2E | 7 e2e-сценариев + CHROME_BIN/CI | G4 | M | браузер |
| QA-DEDUP | убрать дубли help/version/Frida | G4 | S | — |
| PLAT-11a | channelId allowlist | G1 | S | — |
| G6-PARITY | повторный parity после L-02 | G6 | S | L-02 |

### P1B — паритет

| ID | Задача | Owner | Размер | Зависит |
|---|---|---|---|---|
| C-02-PAGE | hasMore/nextCursor, позиция канала | G2 | M | messages_page |
| G-05-REST | 8 из 10 пунктов меню | G2+G3 | M | SERVER-MENU |
| G-01b | restore lastGuildId | G2+G1 | S | UiSettings |
| M-07-UNREAD | unread без mentions/badge ack | G1+G2 | M | read_state |
| M-06-LIVE | live reactions/429 | G2+G4 | S | live |
| G-03-LIVE | invite codes/TTL/USER_BANNED | G1+G2 | M | окно RE-2 |
| G-03a-LIVE | live 204/403 | G2+G4 | S | окно |

### Voice MVP

| ID | Задача | Owner | Размер | Зависит |
|---|---|---|---|---|
| V-01 | RTC session manager (WS op8→0→2→4) | G1 | L | live voice |
| V-02 | UDP live (discovery/keepalive/rotation) | G1 | M | V-01 |
| V-03 | SRTP live (op4 keys) | G1+G4b | L | V-02 |
| V-04 | Opus FFI/KAT | G5+G1 | L | libopus, D-113 |
| V-05 | ALSA/PipeWire devices, PTT, mute | G5 | L | V-04 |
| V-06 | voice panel/participants/ping | G2+G3 | M | V-05, VOICE spec |
| V-07 | reconnect/stats/regions | G1 | M | V-04 |
| V-08 | DAVE/MLS | G1+G4 | XL | V-04 |

### P2/P3

| ID | Задача | Owner | Приоритет |
|---|---|---|---|
| N-01/N-02 | notifications policy/DND | G5+G2 | P2 |
| ST-01 | settings subitems/panels | G2+G3 | P2 |
| Q-02/Q-04 | split search-overlay/data-auth | G2/G1 | P2 |
| M-10/M-11 | pins/threads | G2+G1 | P2 |
| S-01/S-02 | camera/screen/Go Live | G5+G1 | P2 |
| S-03/S-04 | viewer/RTP video/activities | G1+G5 | P2/P3 |
| AT-01…03 | attachments | G1+G2 | P2 blocked |
| A-04/A-05 | QR/passkey | G1+G5 | P3 |
| PL-01…08 | tray/deep links/updater | G5 | P3 |

## 4. Распределение по отделам

| Отдел | Первый batch | Второй batch | Не брать без окна/RE |
|---|---|---|---|
| **G1 Backend** | L-02 contract/backend | PLAT-11a, C-02 data | V-01/UDP/SRTP |
| **G2 Frontend** | C-02 paging | G-05 rest, M-07 unread, G-01b | V-06 без V-05 |
| **G3 Design** | G-05 design, DP-02 sync | VOICE/STREAM review | video без spec |
| **G4 QA** | QA-E2E/dedup | parity tests, mutation | live e2e |
| **G4b Security** | DAVE/keyring review | PLAT-11b/symlink | live voice |
| **G5 Voice/Platform** | portal contract + libopus KAT | ALSA null, devices | PipeWire live |
| **G6 Parity** | L-02/merge parity | voice/video matrix | live E3 |
| **PM** | merge, queue, windows | QA handoff, release | live без токена |

## 5. WIP

- **2 code slots** (сейчас оба свободны);
- 1 design lane, 1 QA/security audit lane;
- один `cargo` на target-dir;
- live — 1 окно на приёмку;
- merge — только на границе batch.

## 6. Merge/QA milestones

| Milestone | Условия |
|---|---|
| **M1 offline merge** | L-02 + G6 re-parity + весь accepted set + gate passed |
| **M2 QA handoff** | e2e/browser, QA-матрица, P0/P1=0; live отдельно |
| **M3 voice MVP** | V-01…V-06 + live 2 клиента |
| **M4 video/Go Live** | S-01…S-03 + consent/live |
| **M5 release** | gate:qualify + live + docs |

## 7. Ближайшие 2 batch

### Batch A (текущий)
- G1: L-02 backend/contract.
- G4: QA-E2E + dedup.
- G2: C-02 paging.
- G3: G-05 remaining design.
- G5: portal contract/libopus KAT.
- G6: parity after L-02.

### Batch B
- merge accepted set.
- QA matrix/e2e.
- voice V-01/V-02 + libopus.
- P0 live window.

## 8. Правила

- `[L]/[S]/[D]` — функция; `[M]/[A]` — нет.
- P0/P1/gate fail → return до полного цикла.
- e2e skip ≠ pass.
- partial не выдавать за implemented.
- PCM/ключи/токены не логировать.
- Merge — координатор; PM не коммитит.
