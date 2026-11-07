# task-219 — FE-P2-22: Guild Role Color & Permission Bitfield Helper

## Сделано (strict TDD, red → green)

1. Сначала контрактный тест `tests/unit/role-permissions.test.mjs` (10 кейсов).
   Red-прогон: `node --test tests/unit/role-permissions.test.mjs` → exit=1,
   `ERR_MODULE_NOT_FOUND` на `rust/gui-spike/ui/features/role-permissions.js`,
   `pass 0 / fail 1` (лог: `red.log`).
2. Затем модуль `rust/gui-spike/ui/features/role-permissions.js` (`// @ts-check` + JSDoc).
3. Green-прогон: exit=0, `tests 10 / pass 10 / fail 0` (лог: `green.log`).

## Контракт модуля

- `PERMISSION_FLAGS` — `Object.freeze` из 10 битов Discord (ADMINISTRATOR `1n<<3n` …
  MANAGE_THREADS `1n<<34n`), плюс `DEFAULT_ROLE_COLOR = '#99aab5'`.
- `hasPermissionBit(bitfieldStrOrBigInt, flag)` — битфилд как decimal-строка или BigInt
  (ноль в `null`/`''`/`'abc'`/`1.5`/`null`/`undefined`/`{}`), ADMINISTRATOR даёт всё.
- `intColorToHex(colorInt)` — `#rrggbb` через `padStart(6,'0')`, фолбэк `#99aab5`.
- `resolveHighestColoredRole(roles)` — максимум по `position` среди `color > 0`, без мутации
  входа, `null` для пустого/`null` списка.

## Факты и артефакты

- red.log / green.log — в этой же папке; md5 файлов ниже.
- `e32f22ce671001d7dc04d19c05e3f185  rust/gui-spike/ui/features/role-permissions.js`
- `342fd8f9de61ad18461c960faedb7fbe  tests/unit/role-permissions.test.mjs`

## Ограничения

`cargo`/`gate:worker` не запускались (token-lean режим, правки только в UI-слое и `tests/unit`).
Коммит не делался: ветку и слияние ведёт координатор.

Branch: feature/task-219-ui-role-permissions  HEAD: dcba282
