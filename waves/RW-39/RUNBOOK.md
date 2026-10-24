# RW-39 — RUNBOOK: дизайн-паритет + frontend/backend

## Этапы

| Этап | Воркеры | Выход | Гейт |
|---|---|---|---|
| 1 | W39a (REA), W39b (Design) | `DESIGN/REFERENCES.md`, `tokens.json`, `layout.json`, `SPEC.md`, `tokens.css`, `DOM-CONTRACT.md` | PM: провенанс, юр. границы, полнота |
| 2 | W39c (Markup), W39d (Frontend) | `ui/index.html`, `ui/style.css`, `ui/components/*.css`, `ui/app.js`, `ui/features/*.js` | DOM/AT-SPI, gate, визуальная сверка |
| 3 | W39e (Backend) | `src/data.rs`, `tests/data_contract.rs`, Tauri-команды | контрактные тесты, `cargo test` |
| QA/REV | QA-1, REV-1 | `mutations*.log`, `REVIEW*.md` | P0/P1=0, мутации |

## Проверки после Phase 2

```bash
node --test tests/unit/gui-spike-dom.test.mjs
node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs
GUI_SPIKE_E2E=1 cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test launch
python3 scripts/gui-window-probe.py --json /tmp/opencode/gui-probe.json
npm run gate:worker
```

## Визуальная сверка

- Референс: `/home/sanya/re-deep/ui/00-app.png` (read-only).
- Своё окно: `scripts/gui-window-probe.py` сохраняет `/tmp/opencode/gui-window.png`.
- Сверять: rail 64–72, tree ~240, header ~48, composer ~56, member panel ~240;
  цвета/типографика — из `DESIGN/tokens.css`; системные шрифты/emoji.

## Юридические границы

- Не копировать PNG/шрифты/иконки/звуки Discord в репозиторий.
- Иконки — собственные inline-SVG или открытые (источник + лицензия в SPEC).
- Бренд — `DL`, без логотипов Discord.

## Backend (Phase 3)

- Источник данных — `fixtures/` (санитизировано), без сети.
- Tauri-команды: `ready`, `messages_page`, `members_page`, `settings`.
- Ошибки типизированы; тесты `tests/data_contract.rs`.
