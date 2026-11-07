# T-230 (FE-P2-25) — Presence & Custom Status Activity Formatter

Ветка: `feature/task-230-ui-presence-activity` (от `unstable`). Гейт: targeted unit-тест.

## Сделано

- `rust/gui-spike/ui/features/presence-activity.js` — чистые функции без DOM:
  `ACTIVITY_TYPES` (frozen 0..5), `MAX_CUSTOM_STATUS_CHARS = 128`,
  `formatActivityLineRu(activity)`, `selectPrimaryActivity(activities)`.
- `tests/unit/presence-activity.test.mjs` — 8 тестов: enum/лимит, пустой вход,
  точные русские префиксы всех 5 типов, CUSTOM из `emoji` + `state`,
  обрезка до 128, отказ на неизвестном типе, приоритет CUSTOM > STREAMING >
  первый валидный, пустой список/`null`/мусор.

## Red -> Green

- red: `node --test tests/unit/presence-activity.test.mjs` → exit 1
  (`Cannot find module ... presence-activity.js`), лог `red.log`.
- green: тот же прогон → exit 0, `tests 8 / pass 8 / fail 0`, лог `green.log`.

## Контракт (принятые решения)

- CUSTOM склеивает непустые `[emoji, state]` через пробел и режется до 128 символов;
  пустой CUSTOM (нет ни `state`, ни `emoji`) формат даёт `""` и в выборе пропускается.
- PLAYING/LISTENING/WATCHING/COMPETING без `name` дают префикс с пустым именем —
  вызывающий UI решает, показывать ли строку.
- Неизвестный `type` → `""`; `selectPrimaryActivity` на не-массиве возвращает `null`.

## Файлы и md5

```
404673c83e872004da699f8a88251bf0  rust/gui-spike/ui/features/presence-activity.js
5ffd9d2f7dd8934a03bbdef8bbb8a06d  tests/unit/presence-activity.test.mjs
```

## Риски / осталось

- Полный `gate:worker` (fmt/clippy/workspace/e2e) не запускался — token-lean режим;
  нужен прогон в integration-лейне.
- Подключение форматтера к строке участника (`members.js`) вне write-scope T-230.
- git commit/push не делались.
