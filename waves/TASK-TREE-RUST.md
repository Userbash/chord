# Дерево задач Rust-клиента (актуализация 2026-10-21, freeze B01)

Источник плана: `docs/rust-client/PLAN.md` (90 карточек). Статусы: `[x]` принято,
`[~]` в работе/на приёмке, `[ ]` можно начинать, `[!]` заблокировано внешним решением.
Факт и тесты — `docs/rust-client/STATUS.md`; batch-план — `docs/rust-client/WAVE-PLAN.md`;
роли и WIP — `docs/rust-client/TEAM.md`; точка восстановления — `waves/STATE.md`.

## 1. Факт

- Rust-воркспейс: **515 passed / 0 failed**, `discord-core` — **473**.
  `cargo fmt --check` и `cargo clippy --workspace --all-targets -D warnings` — чисто.
- Гейт P2-09: **passed**, `waves/RW-18/gate-worker.json`.
- Принято карточек: **18/90** — `P0-02`, `P0-03`, `P1-03`…`P1-09`, `P1-11`, `P1-12`,
  `P2-01`, `P2-02`, `P2-04`, `P2-05`, `P2-06`, `P2-07`, `P2-09`.
- **P2-09** presence принят 2026-10-21: review round 2 P0/P1/P2=0, P3=8 backlog; 54 теста,
  мутации 13/13, gate passed.
- Осталось **72**: P0 5, P1 3, P2 4, P3 16, P4 12, P4.5 3, P5 7, P6 10, P7 7, P8 5.
- Крейты: `discord-codec` + `discord-core` рабочие, `discord-conformance` тестовый,
  остальные 7 — каркасы. GUI-приложения пока нет.

## 2. Дерево зависимостей

```text
P0-02/03 [x], P1-03…09/11/12 [x]
        │
        ├── P1-01 ETF full [ ] (fuzz/границы)
        ├── P1-02 zstd stream strict [ ] (нит s==113)
        ├── P1-10 keyring [!] ────── P5/P6 platform
        │
        ├── P2-01 READY [x] ── P2-02 state reducer [x]
        │        │                 ├── P2-04 members [x] ── P2-05 chunks [x]
        │        │                 ├── P2-07 read-state [x]
        │        │                 ├── P2-09 presence [x]
        │        │                 ├── P2-10 voice states [ ]
        │        │                 ├── P2-11 typing [ ]
        │        │                 └── P2-08 settings-proto [ ]
        │        │
        │        └── P2-06 history (память) [x] ── P3-02 chat ── P3-03 send ── P3-04 media
        │                 │
        │                 └── P2-03 SQLite [!] (после разрешения)
        │
        ├── I1 REST-фасад [x] ── P3-03/P3-09/P2-06 (живые запросы)
        │
        └── P0-01 GUI spike [!] ── P3-01 shell ── P3-12 components
                                                   ├── P3-05…P3-11
                                                   └── P6 platform
```

## 3. Параллельные дорожки и роли

| Дорожка | Карточки | Статус | Роль/seat | Зависит от |
|---|---|---|---|---|
| **D1. State** | P2-02 | **принято** | CORE-1/2 | P2-01 `[x]` |
| **D2. Social state** | P2-07/P2-09/P2-10/P2-11/P2-08 | P2-07/P2-09 `[x]`; P2-10/P2-11 B01; P2-08 B02 | CORE-1/2/3 | P2-02/P2-04 |
| **D3. Members/history** | P2-04/P2-05/P2-06 | **принято** | CORE-1/2 | P2-02 |
| **D4. Protocol debt** | P1-01/P1-02/P0-04/P0-07 | `[ ]` ready | CORE-1/2, RE-1 | P0-02/P0-03 |
| **D5. Client/i18n** | P3-15, P3-01 (headless), P3-12 | P3-15 ready; UI — `[!]` | CLIENT-1/2 | GUI/P2-02 |
| **D6. Voice/media** | P4-01, P4-02… | P4-01 ready headless; остальное `[!]` | VOICE-1 | P1-06/P4-01 |
| **D7. Platform** | P1-10, P2-03 | `[!]` | CLIENT-2 | keyring/SQLite |
| **D8. Release** | P7-04, P8-03 | `[ ]` ready | OPS-1 | P1-07 |

Правило: D2/D4 — текущий batch B01/B02; D5/D6/D7 ждут решений владельца.
P3-15 и i18n-модели можно делать headless до GUI.

## 4. Блокеры

| ID | Блокер | Доказательство | Что нужно |
|---|---|---|---|
| B1 | GUI-крейтов нет в офлайн-кэше (`tauri`/`iced`/`egui`/`winit`/`wry`/`tao`) | `Cargo.lock`, `waves/GUI-AUDIT.md` | разрешение на загрузку конкретного набора **или** временный TUI/headless |
| B2 | `keyring` нет в локе/кэше | P1-10 | разрешение + Secret Service/KWallet |
| B3 | `rusqlite`/`libsqlite3-sys` нет в локе/кэше | P2-03 | разрешение + системный SQLite |
| B4 | Живой Discord нельзя трогать без окна | AGENTS.md | окно владельца для live-smoke |
| B5 | Rate limit фоновых агентов | RW-18/RW-19 | не более 2 запусков, перезапуск по одному |

## 5. Текущий batch: B01 (P2-09 accepted + P2-10 + P2-11)

1. **P2-09 принят:** review round 2 P0/P1/P2=0, P3=8 backlog; accepted в `db.jsonl`.
2. **Research/freeze (готово):** voice states — snapshot `003` + 8 live updates, official
   bundle; typing — TTL 10 000 мс, 28 START; P2-08 — structural settings-proto (B02).
   Контракты: `RW-19/DESIGN-P2-10.md`, `RW-20/DESIGN-P2-11.md`, `RW-21/DESIGN-P2-08.md`.
3. **Кодовая пара:** W19 voice ∥ W20 typing, **два активных воркера**, подволны
   `a` types/parser → `b` reducer → `c` selectors; targeted red/green.
4. **Integration lane (TL-2):** I0 — модули/store-поля/READY snapshot; I1 — единый API-патч
   `EventPayload`, routes, `apply_frame_at`, `MESSAGE_CREATE` hook, re-exports.
5. **Приёмка:** QA/mutation → один batch gate → два ревьюера параллельно → accepted → docs.
6. **P2-08** — B02/RW-21, не третий code worker в B01.

B02 (P1-01/P1-02/P0-04/P0-07/P2-08) и B03 (P3-15/P4-01/P7-04/P8-03) описаны в
`WAVE-PLAN.md`; B01 file matrix — `waves/B01/BATCH-TZ.md`.

## 6. Параллелизм: что уже сделано

- RW-10: W10a (framing) ∥ W10b (TCP/TLS); ревью — отдельный агент.
- RW-11: W11a (rate-limit) ∥ W11b (telemetry) ∥ W11c (ack); ревью — тот же независимый агент.
- RW-12: W12a (`snapshot`+`guild`) ∥ W12b (`social`) ∥ W12c (`read_state`); I1 — отдельной
  дорожкой; ревью P2-01 — три раунда, автор кода не ревьюил.
- RW-13: W13a (types) ∥ W13b (reducer) ∥ W13c (selectors); независимое ревью — два раунда,
  P1/P2 закрыты red-first; финальный gate после синхронизации тест-архива.
- RW-14: W14a (types/window) ∥ W14b (pagination/merge) ∥ W14c (selectors); review P2-1 —
  red-first; P3-2/cap/expect исправлены; gate passed.
- RW-15: W15a (types/parser) ∥ W15b (reducer/ops) ∥ W15c (selectors); review P2-1 —
  red-first; P3 закрыты; gate passed.
- RW-16: W16a (parser/types) ∥ W16b (assembly/reducer) ∥ W16c (selectors/effect); P2-1
  duplicate-index закрыт red-first; gate passed.
- RW-17: W17a (types/parser + routing) → W17b (reducer/ack) ∥ W17c (selectors); P2-1
  channel fallback закрыт red-first; review P0/P1/P2=0; mutation 7/7.
- RW-18: W18a (presence types/parser) ∥ W18b (reducer) ∥ W18c (selectors); review round 1
  P2=2 + P3=14 закрыты red-first; mutation 13/13; gate passed; round 2 — в работе.
- С 2026-10-20 введён batch-конвейер: 3–6 карточек, один integration-лейн общих файлов,
  targeted у воркеров, один общий gate, review после gate (`TEAM.md`).
- С 2026-10-21 code WIP ограничен **двумя активными воркерами** (W19 voice ∥ W20 typing);
  подволны a/b/c, I0 store + I1 API; остальные seats — QA/review/gate/docs.
