# BRIEF W39b2 — выравнивание токенов по дампу (D-02)

Владелец: W39b2 (Design, продолжение W39b). PM: DeepSeek. Итерация: 1.
Основание: `waves/RW-39/PM/DECISIONS.md` D-02, отчёт `waves/RW-39/PM/w39a/REPORT.md`.

## Файлы (только эти)

- `waves/RW-39/DESIGN/tokens.css` — 16 значений + 1 метка (`--status-streaming` → `[A]`) + шаг 30
- `waves/RW-39/DESIGN/SPEC.md` — только строки таблиц значений, затронутые этими правками
- `waves/RW-39/DESIGN/tokens.json` — **не править руками**, перегенерировать `gen-tokens.mjs`
- логи: `waves/RW-39/PM/w39b2/`

`DOM-CONTRACT.md`, `REFERENCES.md`, `layout.json`, `ui/**` — read-only.

## Что сделать

1. Применить значения дампа: `--danger #da3e44`, `--warning #fdb833`, `--positive #3d9e60`
   (+ отдельный `[M]` факт `#45a366`, если он нужен как текст), `--text-inverse`, `--accent-hover`,
   `--accent-soft 16 %`, границы (Δ9) — точные значения из `w39a/REPORT.md`.
2. `--status-streaming`: имя оставить, метку сменить на `[A]` + строка обоснования в SPEC.
3. Шаг строки канала — 30 (28 + 2 gap); в комментарии к токену зафиксировать, что 28 — высота.
4. Пересчитать контрастные комментарии (`4.52`, `4.85` и т.п.) под новые значения — не оставлять
   ложные числа.
5. **D-04 (переменные высоты строк)**: добавить токены `--row-h-continuation: 26px`,
   `--row-h-group-start: 48px`, `--row-line-extra: 22px`, `--row-h-member: 42px`; убрать
   константные `44/40` из SPEC; в `DOM-CONTRACT.md` §9.2 обновить числа и пометить, что
   `#emoji-grid` = `grid` + `row` (структурная часть OBS-01). Тесты **не** править — это W39d.
6. Перегенерировать `tokens.json`, убедиться, что `verify-tokens` даёт **0 mismatch** (кроме
   осознанных `[A]`), `gen-tokens --check` — чисто.
7. `[F]`-числа RW-38 по DOM/ARIA не менять; высоты строк — только через D-04.

## Проверка

```bash
node waves/RW-39/PM/w39a/gen-tokens.mjs --check
node waves/RW-39/PM/w39a/verify-tokens.mjs
node waves/RW-39/PM/w39a/verify-layout.mjs
rg -n "url\(|@font-face" waves/RW-39/DESIGN/tokens.css || true
```

## Отчёт

`waves/RW-39/PM/w39b2/REPORT.md`: строка «до → после → основание», результат verify, md5
`tokens.css`/`tokens.json`, что осталось. Без commit/push.
