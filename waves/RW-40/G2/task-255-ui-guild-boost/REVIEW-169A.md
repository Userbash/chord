# REVIEW-169A — Technical Review PR-140 (task-255 / FE-P2-33)

**Объект:** `rust/gui-spike/ui/features/guild-boost.js` + `tests/unit/guild-boost.test.mjs`.
**Ревьюер:** Технический ревьюер (DeepSeek), token-lean режим.
**Ветка / HEAD:** `feature/task-255-ui-guild-boost`, HEAD `850fab9`.
**Дата:** 2026-10-31.

## Вердикт

**APPROVED.** `P0 = 0`, `P1 = 0`, мутации `5/5` убиты. Целевой тест зелёный, файл после всех мутаций побайтово восстановлен.

## 1. Сверка артефакта с REPORT.md

| Файл | md5 в REPORT.md | md5 фактически | Итог |
|---|---|---|---|
| `rust/gui-spike/ui/features/guild-boost.js` | `bb3064fac22bf960e3c271553fa3706a` | `bb3064fac22bf960e3c271553fa3706a` | совпадает |
| `tests/unit/guild-boost.test.mjs` | `6fbecd4202057a6bdabef99445074fd5` | `6fbecd4202057a6bdabef99445074fd5` | совпадает |

Тестовый файл не изменялся: md5 до и после ревью одинаковый, в `git status` он остаётся `??` (untracked, как и исходник).

## 2. Targeted-тест (baseline)

```
node --test tests/unit/guild-boost.test.mjs
tests 8 / pass 8 / fail 0 / skipped 0
exit=0
```

## 3. Мутации (M1–M5)

Методика: одна правка в `guild-boost.js` → `node --test tests/unit/guild-boost.test.mjs` → фиксация `exit` → восстановление из `/tmp/opencode/rev169a/guild-boost.js.bak` через `cp -p` → `md5sum` сверен с `bb3064fac22bf960e3c271553fa3706a`. Тестовый файл не правился.

| # | Мутация | Мутированный md5 | exit | pass/fail | Убийца (упавший тест) | Restore md5 | Итог |
|---|---|---|---|---|---|---|---|
| M1 | `BOOST_TIER_THRESHOLDS.TIER_1: 2 -> 3` | `5b8bdad71aa8b6789daed0f335321ca6` | 1 | 2/6 | «BOOST_TIER_THRESHOLDS: frozen и содержит пороги 2/7/14», «resolveBoostTierFromCount: границы уровней» | `bb3064…06a` | KILLED |
| M2 | `count >= TIER_3` -> `count >= TIER_3 + 1` (14 → 15) | `081329dc50be4416d2eac4497639a48e` | 1 | 6/2 | «resolveBoostTierFromCount: границы уровней», «…максимальный уровень, остаток и перки» | `bb3064…06a` | KILLED |
| M3 | `Math.trunc` -> `Math.ceil` в `normalizeBoostCount` | `8c683c780663cbc745eaf05d98b6aadb` | 1 | 7/1 | «resolveBoostTierFromCount: негативные, дробные и мусорные значения» (6.99 → 2 вместо 1) | `bb3064…06a` | KILLED |
| M4 | перки уровня 2: `maxUploadMb: 50 -> 64` | `cb20e2bba9a1f42af921b4734e56eb15` | 1 | 7/1 | «BOOST_TIER_PERKS_RU: 4 уровня, вложенные объекты заморожены» | `bb3064…06a` | KILLED |
| M5 | `tier === 2` → `nextThreshold = TIER_2` вместо `TIER_3` | `6b71bf9cc86a3994cb4e306920a93b1f` | 1 | 7/1 | «formatBoostProgressSummaryRu: остаток бустов до следующего уровня» | `bb3064…06a` | KILLED |

**Итого: 5/5 убиты, 0 выживших.** Логи: `/tmp/opencode/rev169a/M{1..5}.test.log`.

## 4. Содержательная проверка контракта

- `BOOST_TIER_THRESHOLDS` = `{2, 7, 14}` и `BOOST_TIER_PERKS_RU` (50/100/150/250 эмодзи, 25/25/50/100 МБ, 96/128/256/384 кбит/с) совпадают с известными перками бустов Discord — расхождений с эталоном не нашёл.
- Порядок ветвей `resolveBoostTierFromCount` убывающий (3→2→1→0), границы включительные — соответствует ТЗ.
- `normalizeBoostCount`: `Math.max(0, Math.trunc(Number(v) || 0))` корректно гасит `NaN`, `undefined`, `null`, `"abc"`, отрицательные и дробные; `"7"` приводится к 7.
- `formatBoostProgressSummaryRu`: `boostsRemaining` = 0 на максимальном уровне (`nextThreshold === null`), `perks` — ссылка на замороженный объект, объекты верхнего уровня заморожены. Возвращаемая структура не заморожена — не требуется контрактом, помечено как P3.

## 5. Замечания

- **P0:** нет.
- **P1:** нет.
- **P2:** нет.
- **P3 (не блокирует):** `normalizeBoostCount` и `formatBoostProgressSummaryRu` дублируют нормализацию (двойной вызов — безвредно, но лишняя работа); `tierLabel` собирается конкатенацией, i18n-ключи не используются — вне scope карточки (чистые помощники).
- Проверка `// @ts-check` типов и полный `gate:worker` не выполнялись по условию ТЗ (token-lean, только targeted-тест).

## 6. Финальное состояние

```
bb3064fac22bf960e3c271553fa3706a  rust/gui-spike/ui/features/guild-boost.js
6fbecd4202057a6bdabef99445074fd5  tests/unit/guild-boost.test.mjs
```

Файл исходника побайтово идентичен состоянию до ревью; `git commit` не выполнялся.
