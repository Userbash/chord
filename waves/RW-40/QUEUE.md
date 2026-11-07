# QUEUE — пул задач, приоритеты и порядок

Новая волна: `POOL-06.md` (предыдущие: `POOL-05`, `POOL-04`).

Сводный план и owners: `MASTER-PLAN.md`.

Дата: 2026-10-24. PM. Источник: `TASKS.md`, `PARITY/GAP-BACKLOG.md`,
`QUALITY/BUGS.md`, `STUB-TRACKER.md`, `MERGE-POLICY.md`.

## 1. Классы приоритета

| Класс | Смысл | Правило |
|---|---|---|
| **P0** | live/security/legal stop-line | выше всего; merge запрещён без фикса |
| **P1A** | доверие: gate/e2e/security/reliability | раньше parity |
| **P1B** | ежедневный паритет: серверы, каналы, голос | по WSJF |
| **P2** | вторичный паритет/полнота | после P1 |
| **P3** | полировка/редкое | backlog |

`Urgency`: **Now** = P0/P1A + активный batch; **Next** = P1B; **Later** = P2/P3.
WSJF-lite — внутри класса; `confidence<0.5` → RE-spike.

## 2. Пул сейчас

| Слот | Кто | Статус |
|---|---|---|
| Code #1 | G1 | **L-02-CORE — active (Orca writer)** |
| Code #2 | — | queued: C-02-BEFORE после L-02-CORE |
| Design | G2 | **L-02-UI-CONTRACT — read-only active** |
| Audit | G6 | **G6-MERGE-AUDIT — read-only active** |
| QA | G4 | **QA-E2E-PREP — read-only active** |
| Platform | G5 | **VOICE-PREREQ — read-only active** |
| Design #2 | G3 | **G05-REST-SPEC — read-only active** |
| Live | — | blocked (нет токена/окна) |

## 3. Очередь P0

| Rank | ID | Задача | Owner | Статус | Зависимость |
|---:|---|---|---|---|---|
| 1 | P0-01 | live READY | PM+G1+G4 | blocked | токен + окно |
| 2 | P0-02 | live send | PM+G1+G4 | blocked | P0-01 |
| 3 | P0-03 | секреты не в логах | G4b | offline green | live-подтверждение |

## 4. Очередь P1A — доверие/security

| Rank | ID | Задача | Owner | Статус | Зависимость |
|---:|---|---|---|---|---|
| 1 | QA4-02 | workspace в gate | G4 | **in work** | — |
| 2 | PLAT-01 | не держать mutex через keyring/сеть | G1 | ready | — |
| 3 | SEC-02 | keyring error не маскировать в Network | G1 | ready | typed error |
| 4 | PLAT-02 | live keyring-кейсы не «passed» | G4 | ready | — |
| 5 | QA4-01b | `walk` fixture | G4 | ready | QA4-01a |
| 6 | PLAT-11 | IPC не принимает путь | G4b | ready | attachments/capture |
| 7 | QA4-03 | поведенческий token test | G1+G4 | ready | — |
| 8 | PLAT-10 | mute/deafen → voice-state | G5+G2 | ready | voice session |

## 5. Очередь P1B — паритет

| Rank | ID | Задача | Owner | WSJF | Статус |
|---:|---|---|---|---:|---|
| 1 | L-01 | reconnect/offline UI | G1+G2 | 6.0 | ready |
| 2 | G-01 | READY → rail серверов | G2+G1 | 5.5 | contract needed |
| 3 | C-02 | переключение канала/история | G2+G1 | 5.0 | L-02 |
| 4 | M-07 | read state/ack | G1+G2 | 4.5 | M-01 |
| 5 | MB-01 | live members | G1+G2 | 4.5 | L-02 |
| 6 | P-01/P-02 | presence из live | G1+G2 | 4.5 | L-02 |
| 7 | P-03 | typing send/indicator | G1+G2 | 4.5 | L-02 |
| 8 | G-03a | enter guild UI | G2+G1 | 4.2 | backend D-76 |
| 9 | G-03 | invite join | G1+G2 | 4.0 | live-коды [A] |
| 10 | DP-02 | D-04 26/48/+22, member 42 | G2+G4 | — | — |
| 11 | V-08 | voice UI на fake | G2+G3 | 3.67 | VOICE D-94 |
| 12 | V-01…V-04 | voice session/UDP/SRTP | G1+G4b | — | live voice |
| 13 | V-05 | Opus | G5+G1 | — | V-04 |

## 6. Очередь P2

| ID | Задача | Owner |
|---|---|---|
| Q-02 | split `search-overlay` | G2 |
| Q-04 | split `data.rs`/`auth.rs` | G1 |
| G1-03/04/05 | mutex, Drop join, dead file/backups | G1 |
| G2-05/… | search filters, P2/P3 UI | G2 |
| M-10/M-11 | pins/threads | G2+G1 |
| AT-01…03 | attachments | G1+G2 — **blocked** upload |
| N-01/N-02/N-03 | notifications/DND/inbox | G5+G2 |
| ST-01…08 | settings sections/sync/i18n | G1+G2 |
| MB-03 | profile popover | G2 |
| PL-01 | tray/background | G5 |
| S-01…S-04 | camera/screen/Go Live | G5+G1 |

## 7. P3 / backlog

QR/passkey, platform badge/deep links/updater, gift (RE), activities, clips/soundboard,
owner/role admin, create server/channel.

## 8. Blocked pool

| Задача | Причина |
|---|---|
| P0 live READY/send | нет тестового токена/окна |
| G-03 invite live | нет live-кодов/TTL |
| AT attachments | нет upload-контракта |
| Voice session/DAVE | нужен live voice + зависимости |
| e2e fixture | нет `CHROME_BIN`/`walk` |
| Video/Go Live | нет capture/STREAM контракта |

## 9. Рекомендуемый порядок на свободный слот

1. **G1** → PLAT-01 (P1A).
2. **G2** → V-08 voice UI на fake (P1B) или Q-02 (P2).
3. **G3** → DP-02 spec.
4. **G4** → QA4-02 → QA4-01b.
5. **G4b/G5** → PLAT-11/Q-08.

Правило: один слот — один P1A/P1B срез; P2 не вытесняет P1; P0 всегда выше.

## 10. G6-REPORT-02 — добавлено в пул (2026-10-24)

| Rank | ID | Задача | Owner | P | Готовность |
|---:|---|---|---|---|---|
| 0 | G6-D-01 | live channelId/messageId blocker | G1+G2 | **P0** | C-02 |
| 1 | G-01 | READY → rail, выбор, restore | G2+G1 | P1 | ready (fixture) |
| 2 | M-07 | ack из UI | G1+G2 | P1 | ready |
| 3 | C-02 | channel switch + history | G2+G1 | P1 | ready |
| 4 | P-03 | POST /typing + indicator | G1+G2 | P1 | ready, TTL config |
| 5 | G-03 | resolve+accept invite | G1+G2 | P1 | `[S]` + live codes `[A]` |
| 6 | ST-01 | settings profile + 4 subitems | G2 | P1 | S, WSJF 6.0 |

`/typing`/`/invites` нет в frozen D-02 каталоге: отдельный `[S]`-оракул, не «добавить по памяти».
P0-02 live send **нельзя** принимать до C-02/G6-D-01.

## 11. Актуальный пул (2026-10-24, после P-03/M-06)

| P | ID | Задача | Owner | Статус | Зависимость |
|---|---|---|---|---|---|
| P0 | P0-01/02 | live READY/send | PM+G1+G4 | blocked | токен+окно |
| P1A | L-02 | dispatch→UI: READY/CHUNK/PRESENCE/REACTION | G1+G2 | ready | G6-D-03 |
| P1A | QA-DEDUP | убрать дубли help/version/Frida | G4 | ready | — |
| P1A | QA-E2E | 7 сценариев + CHROME_BIN | G4 | ready | браузер/CI |
| P1B | C-02-PAGE | hasMore/nextCursor, позиция канала | G2 | ready | messages_page |
| P1B | G-05-REST | 8 из 10 пунктов меню | G2+G3 | ready | SERVER-MENU |
| P1B | G-01b | restore lastGuildId | G2+G1 | deferred | UiSettings |
| P1B | M-07-UNREAD | unread без mentions/badge ack | G1+G2 | partial | read_state |
| P1B | M-06-LIVE | live reactions/429 | G2+G4 | partial | live |
| P1B | G-03-LIVE | invite codes/TTL/USER_BANNED | G1+G2 | partial | окно RE-2 |
| P2 | PLAT-11a | channelId allowlist | G1 | ready | — |
| P2 | Q-02 | split search-overlay | G2 | queued | — |
| P2 | Q-04 | split data/auth | G1 | queued | — |
| P2 | V-04/05 | portal/libopus/ALSA | G5+G1 | ready | plan D-113 |
| P2 | N-01 | notifications policy | G5+G2 | ready | L-02 |
| P2 | M-10/11 | pins/threads | G2+G1 | queued | — |
| P2 | AT | attachments | G1+G2 | blocked | upload RE |
| P3 | QR/passkey | A-04/05 | G1+G5 | queued | — |
| P3 | platform | tray/deep links/updater | G5 | queued | — |
| P3 | video | camera/screen/Go Live | G5+G1 | blocked | RE/live |

**Свободные code slots:** 2/2. **Parity:** deferred до L-02/e2e.
