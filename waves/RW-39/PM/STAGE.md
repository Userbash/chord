# PM / STAGE — доска готовности и порядка работ

Обновлено: 2026-10-23 (сессия PM). Читать вместе с `PLAYBOOK.md` и `LOOP-LEDGER.md`.

## 0. Готовность инструментов

| Средство | Статус | Примечание |
|---|---|---|
| `rea` (MCP) | **ready** | цель не открыта: `open_binary(<path>)` перед RE-сессией; Evidence в отчёт |
| `socraticode` (MCP) | **degraded** | нет Docker → семантический индекс недоступен; **замена: `rg`/`sed`** |
| `context7` (MCP) | **ready** | Tauri → `/websites/tauri_app`, `/websites/rs_tauri_2_9_3` |
| `github` (MCP) | ready | только чтение релизов/тегов, без push |
| `basic-memory` (MCP) | **ready** | 0.23.2, проект `main` — долговременная память волн |
| Скиллы | ready | `discord-lab-*`, `token-efficiency`, `context-engineering-collection`, `orchestration` |
| Контекст-пак | **ready** | `PROJECT-CONTEXT.md` (90 строк) + `waves/RW-39/PM/*` |
| Гейты | ready | `npm run gate:worker`, `cargo test --offline --workspace` (по одному) |
| Codex-Design | **ready** | `PM/run-codex-design.sh <gaps|verify|embeds|themes|menu|all>` — read-only анализ дампа |
| Codex-Review | **ready** | `codex exec -s read-only` — ревью кода (E4 прошёл: 4×P1 найдены и закрыты) |
| Анти-петля | **ready** | `loop-guard.mjs` (3-я попытка только со сменой подхода) + `WINDOW-BUDGET.md` |

Правило экономии: скиллы не грузятся в контекст целиком — берём раздел по роли; вместо
полных доков — `rg -n`/`sed -n` и PM-бриф.

## 0.1. Защита от зацикливания (введено 2026-10-23)

Диагноз: петля была не в коде, а в **способе проверки**. `gate:worker` открывает живое окно
дважды (app smoke + AT-SPI/X11-проба), а каждый red/green-цикл воркера — ещё 3–5 раз
(в логах W38f ≥ 8 запусков окна). Окно флейкует (падение за 1,6–3 с), поэтому PM повторял
ту же команду вместо смены подхода.

| Механизм | Файл | Правило |
|---|---|---|
| Бюджет окна | `WINDOW-BUDGET.md` | итерации — только DOM/model/geometry (сейчас 47/47, 0 skip, ~120 мс); окно — 1 раз на приёмку, кэш по ключу UI+бинаря |
| Кэш прогонов | `window-budget.mjs` | `--key` → `--check` → `--record --reason`; повтор по тому же ключу запрещён |
| Лимит попыток | `loop-guard.mjs` | 1–2 можно, 3-я — `--new-approach --note`, 4-я — `--owner-approved`, 5-я — блок |
| Сигнатуры | `LOOP-LEDGER.md` | повтор двух одинаковых сигнатур = `LOOP-DETECTED` |

## 1. Волны и этапы

| Этап | Что | Воркер | Статус | Приёмка |
|---|---|---|---|---|
| E1 | Phase 1a: `REFERENCES.md`, `tokens.json`, `layout.json` | W39a | **accepted** (D-01) | md5/196 токенов/54 блока/ассетов нет — проверено PM |
| E1b | Правка 16 расхождений в `tokens.css`+SPEC (D-02) | W39b2 | **accepted** (D-06) | `gen-tokens --check` ok, 57/0/0, layout 65 блоков |
| E1' | Phase 1b: `SPEC.md`, `tokens.css`, `DOM-CONTRACT.md` | W39b | **accepted** | 202 токена, метки `[M]/[T]/[L]/[A]/[F]`, ассетов нет, `ui/tokens.css` перевендорен |
| E2 | RW-38: P1-1, P1-2 (+P2-1, P2-3) | W38f | **accepted** (D-05) | md5 7/7, DOM 21/21 |
| E2' | Независимое ревью RW-38 | REV-2 | **PASS** (P0=0, P1=0) | живое окно 0/1, каждая находка с командой |
| E3a | Верстка shell + 4 экрана | W39c2 | **accepted** (D-07): `index.html` 173 строки, DOM 21/21 | шим обновлён аддитивно, контракт listbox |
| E3b | Фронтенд-интеракции | W39d | **accepted** (D-09): 92/92, tsc 0, geometry 31/31 | мутация 44 → 3 fail |
| E4 | Backend BFF на `fixtures/` | W39e + W39e2 | **accepted** (D-08, D-12): 22+20+1, clippy/fmt 0, 4×P1 Codex закрыты | UI→invoke — отдельная карточка |
| E6 | Мутации + порт features-оракула | QA-1 | **running** | все мутанты caught, новый тест 9/9 |

Параллельность: ≤ 2 фоновых воркера, `cargo` — один. E3a/E3b/E4 — строго последовательно
(общий `ui/`, общий target-dir).

## 2. Разрыв петли (LOOP-01)

- Симптом: RW-38 FAIL (P1=2), PM не может принять и не может править; ревью прозой, без repro.
- Решение: исполняемые критерии + назначенный исполнитель + независимый ревьюер.
- Кэп: **2 попытки на находку**; 3-я — только со сменой подхода (запись в ledger);
  4-я — только с решением владельца.

## 3. Следующие действия PM

1. W39b2 (running) → при green принять E1b, перевендорить `tokens.css` в `ui/` (W39c2).
2. REV-2 (running) → при PASS(P0=P1=0) закрыть E2 и записать accepted в журнал; при FAIL —
   вернуть только с исполняемыми критериями (не более 1 возврата, далее смена подхода).
3. W39c2: канонический `index.html` из DOM-CONTRACT §3 → DOM/model/geometry green.
4. W39d frontend → W39e backend (cargo строго по одному).
5. Один batch-gate на E3/E4 (окно — 1 раз, `window-budget.mjs --record`), затем QA-1 и REV-1.
5. После приёмки: `db.jsonl`/`STATE` — координатор (не PM-агент).

## 4. План этапов v2 (2026-10-23, после E1/E3a/E4)

| Этап | Луч | Воркеры | Выход | Гейт | Статус |
|---|---|---|---|---|---|
| E5 | frontend | W39d | `app.js`/`features/*`, D-04 высоты, listbox | DOM+model+bench+tsc+geometry | running |
| E5r | ревью backend | Codex CLI (read-only) | `PM/w39e/REVIEW-codex.md` | P0/P1=0 | running |
| E5re | RE-данные | W39f | `DESIGN/RE-EXTRA.md` + `re-extra.json` | провенанс, ассеты не скопированы | running |
| E6 | QA | QA-1 | мутации: высоты 26/48, listbox, пагинация, ошибки BFF | все мутанты caught | после E5 |
| E7 | приёмка | PM + REV-1 | один `gate:worker` + одна живая проба | P0/P1=0, 0 skip | после E6 |
| E8a | дизайн P3-02 | W39g + W39g2 | **accepted** (D-13, D-17): 224 пункта, 31 `[A]` закрыт, tokens 98 | дельта RE внесена |
| E8b | frontend P3-02 | W39h + W39h2 + W39h3 | **accepted** (D-18/D-21/D-22): 129/129, Codex PASS (D-23) | слияние: ждём W39j |
| E8c | backend P3-02 | W39i + W39i2 | **accepted** (D-15, D-16): 29+20+1, `search_page` зарегистрирован | зеркало фильтров → W39j |

Правила параллельности: ≤ 2 кодовых воркера одновременно; `cargo` — один; окно — один раз
на приёмку (`window-budget.mjs`); Codex-ревью — read-only, не блокирует кодовый луч.
Каждый lane закрывается своим набором команд; общий gate — только на E7.

### Что уже добыто и чего не хватает

- Есть: токены (202), layout (65 блоков), DOM-контракт, BFF 17 тестов, канонический markup.
- Нужно для P3-02: поведение композера, группировка участников, категории эмодзи, разделы
  настроек, клавиши — источник `reference-oracle/` (W39f).
- Живой RE-прогон спайка: один запуск в отдельной сессии (Xvfb/AT-SPI + PNG), запись в
  `window-runs.jsonl` с `reason=acceptance E7`.

## 5. Передача дизайна (2026-10-23)

- Пакет: `waves/RW-39/HANDOFF.md` + `DESIGN/ref-manifest.json` (23 референса, md5, 0 missing).
- Брифы: `BRIEF-W39g` (дизайн P3-02), `BRIEF-W39h` (frontend), `BRIEF-W39i` (backend).
- Исключение из лимита 2: W39g — docs-only (без cargo/окна/тестов), поэтому запущен третьим
  параллельно с W39e2 (backend) и W39f (RE); риск ресурсов нулевой, запись здесь.
- W39h стартует после приёмки W39g; W39i — после приёмки W39e2 (общий `data.rs`).

- Исключение №2: W39i (backend) запущен параллельно W39f (read-only) и W39g (docs-only):
  cargo свободен после W39e2, файлы не пересекаются (`src/data.rs` vs `DESIGN/**`).

- Исключение №3: W39g2 (reconcile спеки, docs-only) параллельно W39h (frontend) и W39i
  (backend): файлы не пересекаются (`SPEC-P3-02.md` vs `ui/**` vs `src/**`).

## 6. Окно слияния и QA (E9)

- Регламент: `PM/MERGE-GATE.md` — предусловия, ветка `dev/rw-39-integration`, полный гейт,
  интеграция с ядром (W39j), передача QA.
- План тестирования: `QA/PLAN-E9.md` — матрица всех кнопок/функций (N/C/M/E/U/S/K/B/P),
  критерии PASS, честные disabled вместо мёртвых кнопок.
- Статус: **не открыто** — ждём W39h (frontend) и применения дельты W39g2 (W39h2).

## 7. Матрица функций и покрытие

- `DESIGN/FUNCTION-MATRIX.md` — UI → Tauri → BFF → ядро, по экранам и контролам.
- `PM/COVERAGE-GAPS.md` — эвристика «нет прямых тестов»: core 28/265, ui 20/112,
  voice 3/84 (в т.ч. `srtp_key_derive`), backend 0/12.
- Сырые данные: `PM/coverage-heuristic.json`.
- Codex-Design `gaps`: `DESIGN/CODEX-REFS-gaps.md` (D-19).

## 8. Приёмка E9 и передача QA

- `gate:release` — **passed_with_skips** (e2e 4 opt-in skip), `cargo test --workspace` — 0 failed.
- Артефакты: `QA/artifacts/*`, `QA/HANDOFF-E9.md`, `PM/md5-merge.txt`.
- Остаток: коммит/ветка `dev/rw-39-integration` — координатор; QA — по `QA/PLAN-E9.md`.

## 9. Сетевой отдел (NET)

- Устав/границы: `NET/CHARTER.md`; этапы N0–N7.
- В работе: NET-A (auth RE по дампу) → `NET/AUTH-RE.md`; NET-B (аудит ядра) → `NET/CORE-AUDIT.md`.
- Далее: ADR-2 (метод входа), NET-C backend auth/keyring, NET-D login UI, NET-E live gateway.
- Жёстко: без обхода блокировок/анти-детекта, токены только в keyring, live — по окну владельца.

## 10. NET: результаты N1/N2 и решение N3

- NET-A: `AUTH-RE.md` (87 фактов), NET-B: `CORE-AUDIT.md` (16 gaps) — приняты (D-27, D-28).
- `ADR-2-auth.md`: вход = импорт токена владельца (keyring), без пароля/MFA/captcha/QR.
- Heartbeat по RE — **op40 `{seq,qos}`**; текущий core шлёт op1 → добавить op40/fallback (NET-C2).
- NET-C1 принят (552), NET-C2 принят (80): auth-команды + keyring-поток.
- В работе NET-E1 (URL/op40/supervisor); далее NET-D (login UI), NET-E (live READY, одно окно).

- Исключение: NET-C2 (gui-spike target) и NET-E1 (root rust target) идут параллельно —
  target-dir разные, блокировки Cargo.lock нет; один `cargo` на каждый target-дерево.

## 11. Программа AUTH (кросс-отдел)

- `NET/PROGRAM-AUTH.md` — W1…W7, DAG, файловая матрица, гейты.
- `NET/AUTH-CONTRACT.md` — замороженный контракт состояний/команд/ошибок/id.
- `NET/NET-FUNCTIONS.md` — карта сетевых функций, передана сетевикам.
- В работе: NET-R2 (RE login UI/captcha/2FA), NET-F1 (password/MFA backend, post),
  NET-DESIGN (LOGIN-SPEC).
- Исключение: NET-DESIGN — docs-only, идёт параллельно с read-only NET-R2 и cargo NET-F1.
- Далее: NET-D2 (login UI по спеке), NET-E2 (connect/READY), QA-AUTH, Codex-Security.

## 12. RW-40 — план продукта (2026-10-24)

- `waves/RW-40/TZ.md` — инвентарь функций (A/C/M/S/V), приоритеты, данные, критерий.
- `waves/RW-40/TASKS.md` — отделы/роли, доступность исполнителей, доска A/B/C/D.
- `waves/RW-40/COMMS.md` — брифы, возвраты, приёмка, стыки, эскалация, качество.
- Текущий фокус: **A1** — окно входа (40/41, tsc 52); затем A2 (captcha-поля) и A3 (connect/READY).
