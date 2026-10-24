# RW-39 — PM ACCEPTANCE (Phase 1)

Дата: 2026-10-24. PM: DeepSeek. Вердикт: **Phase 1 принят с условиями**.
Phase 2/3 — в работе/на ревью (см. §4).

## 1. W39a (REA-референсы) — принят

Артефакты и проверки:

| Файл | md5 | Проверка |
|---|---|---|
| `DESIGN/REFERENCES.md` | `3e60c0380d3548894e82b997fc43399a` | 57 match / 0 mismatch / 0 missing; §3 палитра/типографика |
| `DESIGN/layout.json` | `e5b2dcdfa5b7b60da6d7d446c9785994` | 65 блоков с source; 4 экрана; rail 64, tree 240/311, header 49, composer 56 |
| `DESIGN/tokens.json` | `12be8c11a82abbe97ba544505ce75ccc` | 205 токенов (производный от tokens.css) |
| `DESIGN/README.md` | `03989274a33944286f2a7ea5ec90023a` | индекс/воспроизведение/границы |

Независимые проверки W39a: `gen-tokens --check`, `verify-tokens` (205/205 1:1),
`verify-layout` (65 блоков, 4 экрана). Ассеты не копировались; дамп read-only.

## 2. W39b (дизайн-система) — принят с условиями

| Файл | md5 (актуальный) | Содержание |
|---|---|---|
| `DESIGN/SPEC.md` | `340eeac669096af92ccc62fd4a0f3693` | система, 17 компонентов, 4 экрана, контраст AA |
| `DESIGN/tokens.css` | `7ce9c1f03e26056243e13df3237b0a9d` | 205 токенов (+override) |
| `DESIGN/DOM-CONTRACT.md` | `a564f36fe46f68e6895b956c3a6a5a9d` | канонический DOM, ARIA, виртуализация, чек-листы |

**Условия (P2, перенести в Phase 2/карточку):**
1. Эмодзи-ячейка: SPEC 44+gap4 vs RE 48/gap0 — привести к 48 в Phase 2.
2. Switch: SPEC 40×24/thumb18 vs RE 48×24/thumb24 — привести к 48.
3. Settings nav: SPEC 232 vs RE 264 (item 34 vs 36) — привести к 264.
4. P3: notice 24 vs RE 34; members-toolbar в референсе отсутствует; settings max-w 720 vs 740.
5. `tokens.css` регенерирован после отчёта W39b (md5 `7ce9c1f0…`); ссылки в отчёте W39b
   на `62fffb9c…` устарели — Phase 2 использует текущий файл (см. D-39-06).

Эти условия не блокируют верстку: DOM-контракт и токены зафиксированы; размеры
уточняются в Phase 2 без ослабления RW-38-тестов.

## 3. Решения PM

См. `waves/RW-39/PM/DECISIONS.md`:
- **D-39-01** — аддитивная правка `buildDom` под новый shell (источник — DOM-CONTRACT).
- **D-39-02** — 240 базовый / 311–256 на ≥1600.
- **D-39-03** — трек сообщения 44px сохраняется; плотность 26/48 → P3-02b.
- **D-39-04** — `window-runs.jsonl` как live-evidence.
- **D-39-05** — 12 live-keyring skip: причина + доказанный live-прогон 12/12.
- **D-39-06** — актуальный md5 токенов.

## 4. Phase 2/3 — статус

По `HANDOFF.md` роли: W39g (designer `SPEC-P3-02.md`), W39h (frontend `features/*`),
W39i (backend `data.rs`). Реализация уже наблюдается: 9 CSS-модулей, 11 feature-модулей,
`src/data.rs` (ready/messages_page/members_page/settings), контракты search/settings/typing,
auth/regressions/channel-paging. Целевые Node-тесты: **209/209** (dom 21, model 26,
bench 45, features/regressions/auth/paging); Rust-контракты зелёные.

Гейт на 2026-10-24: **passed_with_skips** (typecheck/ui-tsc 0, meta 65/65, unit 651/651,
spike-rust 20/20, workspace-rust 1115 (12 live-keyring ignored), smoke 22/22).
Приёмка Phase 2/3 — после независимого REV-1 (P0/P1=0) и визуальной сверки по SPEC §14.
P2 из §2 (ячейка 48, switch 48, nav 264) — условия приёмки Phase 2.

## 5. Evidence

```
node --test tests/meta/test-policy.test.mjs  → exit 0, 5/5
npm run gate:worker                           → passed_with_skips (см. §4)
DISCORD_LAB_KEYRING_LIVE=1 cargo test … --ignored → exit 0, 12/12
python3 scripts/gui-window-probe.py           → exit 0; PNG 1280×820; window-runs.jsonl
```
