# Независимое ревью — G05-REST-MUTATIONS

Дата: 2026-10-30. HEAD: `4e24048`. Продуктовый код и тесты read-only.

## Вердикт

**RETURN. P0=0, P1=1.**

### P1 — два целевых мутанта D1 проходят

В mutation evidence M1 (удалён guard `aria-disabled` в click-dispatcher) и M1b
(удалён guard в `guild-invite`) каждый отдельно завершаются exit 0, 27/27. M1c ловится,
только когда удалены оба guard одновременно. Это не доказывает, что тесты ловят регрессию
каждого из двух заявленных защитных путей. TEST-POLICY §3 требует считать missed-мутант
незакрытым; диагностическое объяснение о дублировании guard-ов не является waiver.
P1 из предыдущего ревью поэтому остаётся открытым.

## Evidence

- MD5 совпадают с отчётом: `actions.js` `4effc2bec384b30ba074bb26933a6863`,
  `server-menu.test.mjs` `83ed20625a33779ea173431c3d0da836`.
- `node --test tests/unit/server-menu.test.mjs` → exit 0, 27 passed, 0 failed.
- M2–M6 пойманы; M1 и M1b missed; M1c caught. Файл восстановлен к базовому MD5.
- `cargo` не запускался.
