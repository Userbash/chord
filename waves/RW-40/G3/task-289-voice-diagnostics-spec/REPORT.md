# REPORT — task-289 (`G3 Design L1 / P2`, `RE-22-03`): дизайн-спецификация `RTCConnectionPopout` и `RTCDebugModal`

- **Карточка:** `RE-22-03` (`waves/RW-40/PM/BACKLOG-RE-22.md`)
- **ТЗ:** `waves/RW-40/G3/task-289-voice-diagnostics-spec/TASK.md`
- **Отдел / исполнитель:** G3 Design / `opencode --model deepseek/deepseek-v4.1-flash`
- **Рабочее дерево:** `/home/user/orca/workspaces/project/design-task-289-voice-diagnostics-spec` (`branch:design/task-289-voice-diagnostics-spec`)
- **База:** `70c0473` (Merge PR-166, task-286)
- **Дата:** 2026-10-31
- **Тип работы:** docs-only. Продуктовый код, тесты, `tokens.css`, `PLAN/STATUS/TASKS/waves/*` не менялись. `git commit/push` не выполнялся.

---

## 1. Сделано

Созданы два файла с идентичным содержимым (811 строк):

1. `docs/specs/VOICE-DIAGNOSTICS-SPEC.md`
2. `waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md`

Структура: §0 источники и теги `[D]/[T]/[S]/[F]/[A]/[M]`, §1 индикатор `#voice-ping`, §2 пороги `quality`, §3 попап `RTCConnectionPopout` (3.1–3.10, включая 3.5.1 `shortHostname`), §4 окно `RTCDebugModal`, §5 CSS-токены и измеренный контраст, §6 ARIA и клавиатура, §7 4-цикл состояний, §8 локализация, §9 приёмка, §10 открытые вопросы.

### 1.1. Все требования ТЗ закрыты

| Требование ТЗ | Где | Статус |
|---|---|---|
| `#voice-ping`: `18×18px`, `padding: 7px`, `border-radius: 8px` | §1.1 | готово |
| Классы `.rtcConnectionQualityFine/Average/Bad` | §1.2 | готово |
| Пороги `fine/average/bad` (`250/500` мс, `5/10` %) | §2.1–2.3 | готово |
| Попап `width: 300px` | §3.2 | готово |
| Canvas-спарклайн `258×80`, история `199` точек | §3.4, §3.5 | готово |
| `shortHostname` | §3.5.1 | готово |
| `Средняя задержка` (`toFixed(0)`, 20 точек), `Последняя задержка`, `Потеря исходящих пакетов` (`toFixed(1)`) | §3.5, §3.6 | готово |
| Баннер предупреждения `badPing=250`, `badLossRate=10` | §3.7 | готово |
| Вкладки `Интеграция` / `Конфиденциальность`, `--secure-frames-columns: 3`, 6×5 цифр, `Копировать` → `Скопировано` | §3.3, §3.10 | готово |
| `RTCDebugModal` `854×480`, 8 секций, `600` точек `{ value, time }` | §4.1–4.3 | готово |
| CSS-токены `--text-feedback-*`, `--background-feedback-*`, `--brand-500` | §5.1 | готово |
| ARIA `role="dialog"`, `aria-labelledby`, `tablist`, `tab`, `tabpanel` | §6.1 | готово |
| Клавиатура `Escape`, `Tab`, `ArrowLeft`/`ArrowRight` | §6.2 | готово |

---

## 2. Верификация по оракулу (вместо red-прогона)

Red-прогон не применялся: задача не пишет продуктовый код и тесты, strict TDD относится к `RE-22-04`. Вместо него выполнена сверка каждого числа с эталонным дампом прямым `grep` по `reference-oracle/`.

**Оракулы:** `decoded/3eefa7dae1e2c234_0` (`md5 934b1289b7ebf17bc0fd09569b23b438`), `decoded/e6811dfca6ffcd34_0` (`md5 8ab1304fc4d29d637bcc1d51dcd35a2a`), `voice/ui/01-voice-panel.html` (`md5 ef5a88b552e9cc90e322f8da598a46dd`), `ru-catalog.json` (`md5 09272320cecfa79d479a2b03df7adce1`).

### 2.1. Подтверждено дословно из дампа

| Утверждение | Команда / результат |
|---|---|
| `.largePing__06d62` = `width:18px;height:18px;padding:7px;border-radius:8px` | `grep -o '\.largePing__06d62{[^}]*}'` → совпало полностью, плюс `box-shadow: inset 0 0 0 1px var(--border-muted)` и `margin-inline-end: 8px` |
| Заливки ховера `--background-feedback-positive/warning/critical` | 3 отдельных правила `.largePing__06d62` с этими токенами |
| Геттер `get quality()` (порядок `bad → average → fine`) | `grep -o 'get quality(){[^}]*}'` → код совпал дословно |
| `_handlePing`: потолок `199` (`while (len >= 200) shift()`) | `grep -o '_handlePing(e){...}'` |
| `_handlePingTimeout`: 3 подряд `seq` → `_handlePing(t)` | тот же вывод, условие `seq[n-1]===seq[n-2]+1 && seq[n-2]===seq[n-3]+1` |
| `getAveragePing()` = `slice(0, Math.min(len,20))` | совпало; подтверждён нюанс «первые 20, не последние» |
| Попап `container__50387` / `debugPanelStandalone__50387` = `width:300px` | оба правила |
| Canvas `{dataPoints, width: 258, height: 80}` | `grep -o 'width:258'` + JSX вызова |
| `columns: 3` / `--secure-frames-columns` | `grep -o 'columns:[0-9]'` → ровно одно вхождение `columns:3` |
| `.code__1a1f5` grid + `--secure-frames-columns` | полное правило с `column-gap`, `row-gap`, `padding` |
| `getShortHostname` = `e.split(".")[0]`, `null → ""` | модуль `257120` |
| `toFixed(0)` / `toFixed(1)` в строках метрик | JSX из модуля `978171` |
| `badPing:250`, `badLossRate:10` | `intl.format(b.t["3pFz1P"], {badPing:250,badLossRate:10,url:l})` |
| `854 × 480` | `handleOpenRTCDebugPopout(){ ... {defaultWidth:854,defaultHeight:480} }` |
| 8 секций `zWA` и их порядок | `t$=((o={}).TRANSPORT="TRANSPORT",o.OUTBOUND=…,o.CLIPS="CLIPS",o)` |
| `600` точек ряда | `if (e.length > 600) e.shift()` |
| `30 FPS` графика | `let o = 1e3/30` в модуле `229659` |
| i18n `X58/lN`, `6iv2TF`, `VIBJM+`, `cTQI5t`, `t5VZ88`, `MBY1Pm`, `zC6o3s` | сверено построчно с `ru-catalog.json` |
| `--background-feedback-*` = 8 % альфа | `hsl(var(--opacity-green-8-hsl)/0.0784313725490196)` |
| `--brand-500` = `hsl(234.935 85.556% 64.706%)` | `--brand-500-hsl` из дампа |

### 2.2. Исправлено относительно `RE-GAP-22` (спека фиксирует факт дампа, не пересказ)

1. **`.statsRow_effb26`** — в `RE-GAP-22` §4.2 указано `display: flex; justify-content: space-between`. В дампе: `.statsRow_effb26{align-items:flex-start;display:flex}` + `.statsText_effb26{display:flex;flex-direction:column}`. `space-between` отсутствует; строки идут стопкой с `gap: 8px`. В спеке — фактическое правило (§3.6).
2. **Кнопки действий** — в `RE-GAP-22` описаны как `grid-template-columns: 1fr 1fr`. В дампе: `.buttons_effb26{display:flex;gap:var(--space-8)}` + `.copyButtonWrapper_effb26{flex:1}`. Результат тот же, механизм другой (§3.8).
3. **Фон графика** — в `RE-GAP-22` указан `--background-mod-subtle`. В дампе `.graphContainer_effb26{background-color:var(--background-base-lower);margin-bottom:var(--space-16);padding-bottom:2px}` (§3.4).
4. **Скрытие неактивной панели** — в дампе только `visibility: hidden`, что не убирает элементы из порядка обхода. В спеке добавлены `hidden` + `inert` как обязательное требование a11y (§3.3), помечено `[A]`.
5. **Шкала отступов** — Discord `--space-N` = `N` px, наш `--space-N` — другая шкала. Прямой перенос `var(--space-12)` дал бы `56px` вместо `12px`. Маппинг зафиксирован таблицей (§3.10) и вынесен в механическую проверку (§9.2).

### 2.3. Найдено и закрыто в ходе сверки

При механической проверке спецификации на полноту требований ТЗ обнаружился пропуск: `shortHostname` был описан только косвенно. Добавлен отдельный §3.5.1 с правилом `split(".")[0]`, обработкой `null`, типографикой `text-sm/bold` и поведением при пустом значении.

---

## 3. Прогоны

### 3.1. Механическая проверка полноты спеки

```
$ f=docs/specs/VOICE-DIAGNOSTICS-SPEC.md
$ # 35 утверждений ТЗ: геометрия, пороги, секции, токены, ARIA, i18n
$ echo "MISS=$fail"
MISS=0
```

Все 35 проверок `grep -qF` по обязательным значениям прошли (`width: 18px`, `padding: 7px`, `width: 300px`, `width: 258`, `854`, ``**`199`**``, ``**`600`**``, `--secure-frames-columns: 3`, `role="dialog"`, `role="tabpanel"`, `--brand-500`, `badPing: 250`, `badLossRate: 10`, `getShortHostname` и др.).

### 3.2. Идентичность двух копий

```
$ md5sum docs/specs/VOICE-DIAGNOSTICS-SPEC.md waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md
2c522d775d9b39678208a08462a56a0f  docs/specs/VOICE-DIAGNOSTICS-SPEC.md
2c522d775d9b39678208a08462a56a0f  waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md
```

### 3.3. `npm run test:meta` (`T0`, применимый мета-уровень для docs-only)

```
ℹ tests 102
ℹ pass 97
ℹ fail 5
ℹ skipped 0
```

**5 падений — предсуществующие и не связаны с задачей.** Все они в `tests/meta/keyring-live.test.mjs` (`PLAT-02 T1`, `PLAT-02 T3`):

- `✖ ровно 12 #[test] и 12 #[ignore] с причиной live-keyring`
- `✖ у каждого #[test] сразу следует #[ignore = "live-keyring: …"]`
- `✖ имена и число кейсов не изменились`
- `✖ в дефолтном прогоне 0 passed и 12 ignored`
- `✖ --list --ignored перечисляет ровно 12 кейсов k01…k12`

Причина: в `rust/crates/discord-platform/tests/keyring_contract.rs` присутствует 13-й live-кейс `k25_migrator_with_live_keyring` (добавлен коммитом `b8183f5`), а мета-тест ждёт ровно 12 (`k01…k12`). Мета-тест последний раз менялся в `4e24048`.

Доказательство независимости от задачи: `git status --short tests/meta rust/crates/discord-platform` — пусто, ни один файл этой проверки не тронут; задача добавляет только два `.md`.

`waves-sync` и остальные 97 мета-тестов проходят с новым файлом в `waves/RW-40/DESIGN/`.

### 3.4. Что не прогонялось и почему

- `gate:worker` (`cargo fmt`/`clippy`/`test --workspace`/`e2e`) — не применим: нет Rust/JS-изменений, ни один файл вне `docs/` и `waves/` не затронут.
- `tsc`/линтер — не применим: `.js`/`.mjs` не менялись.

---

## 4. Файлы и контрольные суммы

| Файл | Строк | `md5` |
|---|---|---|
| `docs/specs/VOICE-DIAGNOSTICS-SPEC.md` | 811 | `2c2f1d1158f93c369446c3325d906be8` |
| `waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md` | 811 | `2c2f1d1158f93c369446c3325d906be8` |
| `REPORT.md` (этот файл) | 166 | не фиксируется: файл содержит сам себя |

Оба файла — новые (`??` в `git status`), созданы строго в рамках write-scope ТЗ. Второй файл — побайтовая копия первого, что проверяемо одним `md5sum`.

Не тронуто: `rust/**`, `tests/**`, `scripts/**`, `tokens.css`, `PLAN.md`, `STATUS.md`, `TASKS.md`, `ROADMAP.md`, `docs/map.md`, `waves/STATE.md`, `waves/db.jsonl`.

---

## 5. Риски и передача

| № | Риск | Влияние | Митигация |
|---|---|---|---|
| 1 | `RE-22-04` перенесёт отступы по нашей шкале напрямую (`var(--space-12)` → `56px`) | Визуальный дефект в сетке `Voice Privacy Code` | Маппинг в §3.10 + проверка `! grep -q 'padding: var(--space-12)'` в §9.2 |
| 2 | Скопируют `.statsRow` с `justify-content: space-between` из `RE-GAP-22` | Раскладка метрик разойдётся с эталоном | §3.6 содержит фактическое правило и явно отмечает отличие |
| 3 | `--danger` используют как цвет текста `bad` | Контраст `3.50:1` < AA | §5.3 — измеренная таблица, `--danger` только non-text |
| 4 | Скрытая вкладка останется достижимой с клавиатуры (`visibility: hidden`) | Дефект a11y | §3.3 требует `hidden` + `inert`, §9.2 проверяет наличие `inert` |
| 5 | `slice(0, 20)` даёт среднее по старейшим точкам — выглядит как баг | Спор при приёмке | §3.5 фиксирует это как паритет и выносит в §10 как открытый вопрос для `G5 Voice`/PM |
| 6 | `854×480` — размер окна в оригинале, у нас оверлей | Несовпадение с ожиданием PM | §0.3 и §4.1 явно описывают адаптацию `[A]`, вопрос в §10 |
| 7 | 5 предсуществующих падений `test:meta` припишут этой задаче | Ложный красный гейт | §3.3 с точной причиной, коммитами и доказательством независимости |

**Передача:** спека готова для `RE-22-04` (`G2 Frontend`). Карточки `RE-22-01` (`G5 Voice`) и `RE-22-02` (`G1 Backend`) получают из неё входной контракт: §2 — пороги, §3.5 — буфер и производные, §3.6 — формат строк, §4.3 — ряды, §3.10 — 30-значный код.

Документы `STATUS`/`ROADMAP`/`TASKS`/`map`/`STATE` не обновлялись — по `PROCESS.md` их синхронизирует Team Lead после приёмки.
