# REVIEW-166A — task-252 (webhooks-editor.js, FE-P0-14)

- **Ревьюер**: Technical Reviewer (mutation testing, token-lean mode)
- **Ветка / worktree**: `feature/task-252-ui-webhooks-editor`
- **Дата**: 2026-10-31
- **Объём ревью**: `rust/gui-spike/ui/features/webhooks-editor.js` (374 строки), `tests/unit/webhooks-editor.test.mjs` (468 строк), `REPORT.md`

## Verdict: APPROVED

## 1. Targeted-тесты (baseline)

```
node --test tests/unit/webhooks-editor.test.mjs
→ exit=0, tests 9, pass 9, fail 0 (4 suites)
```

## 2. Мутационное тестирование: 5/5 KILLED

Каждая мутация — правка `rust/gui-spike/ui/features/webhooks-editor.js` через `sed -i`,
затем `node --test tests/unit/webhooks-editor.test.mjs`, затем восстановление файла из
байтовой копии. Лог: `MUTATION-166A.log`.

| # | Мутация | Ожидаемое нарушение контракта | Результат |
|---|---------|-------------------------------|-----------|
| M1 | `MIN_WEBHOOK_NAME_CHARS = 1` → `0` | пустая строка `""` становится валидным именем | KILLED (exit=1, pass 8 / fail 1) |
| M2 | `MAX_WEBHOOK_NAME_CHARS = 80` → `81` | имя из 81 символа проходит границу | KILLED (exit=1, pass 8 / fail 1) |
| M3 | `lower === "clyde"` → `lower === "clydex"` | `ClYdE`/`clyde` перестаёт отсекаться | KILLED (exit=1, pass 7 / fail 2) |
| M4 | `lower.includes("discord")` → `lower.includes("zzz-discord")` | запрет подстроки `discord` без учёта регистра ломается | KILLED (exit=1, pass 7 / fail 2) |
| M5 | `***` → `XX` в `maskWebhookTokenUrl` | токен маскируется не тем литералом | KILLED (exit=1, pass 7 / fail 2) |

Итог: `5/5` (100%). Ни одна мутация не выжила — тесты проверяют контракт, а не текущую реализацию.

## 3. Восстановление файла

```
== restored ==
d36b437e62317a54fd4b7f9c7cea64d5  rust/gui-spike/ui/features/webhooks-editor.js
byte-identical: yes
```

md5 совпадает с зафиксированным в `REPORT.md` (`d36b437e62317a54fd4b7f9c7cea64d5`);
`cmp` с исходной копией — побайтовое совпадение. Тестовый файл не изменялся
(`d90df3860665f11accc26f29551317e3`, совпадает с `REPORT.md`).

## 4. Замечания

- Блокирующих замечаний нет (`P0 = 0`, `P1 = 0`).
- Оракул соблюдён: границы `1..=80` после `trim`, запрет `clyde`/`discord` без учёта регистра,
  маскирование `<id>/***` для `/api/webhooks/` и `/api/v9/webhooks/`, `WEBHOOK_TYPE_LABELS_RU`
  заморожен и покрыт `Object.isFrozen` + `assert.throws` на запись.
- A11y-контракт на месте: `role="region"`, `aria-label="Вебхуки"`, `role="alert"` у ошибки.
- Наблюдение (не блокирует): `maskWebhookTokenUrl` отбрасывает хвост после токена
  (`.../<id>/token?wait=true` → `.../<id>/***`) — это осознанное поведение по JSDoc и тестам.
- `gate:worker` и `git commit` в этом ревью не запускались по условиям ТЗ (token-lean mode);
  приёмка ограничена targeted-тестами и мутациями.
