# REPORT — NET-DESIGN (окно входа)

**Статус:** выполнено, готово к приёмке PM/Team Lead.
**Дата:** 2026-10-23. **Роль:** NET-DESIGN. **PM:** DeepSeek.
**Выход:** `NET/LOGIN-SPEC.md` (570 строк) + `NET/login-spec.json` (schema `rw39/login-spec-v1`).
**Логи:** `NET/PM/netdesign/` (`probe.txt`, `token-check.txt`, `md5.txt`).

## 1. Что сделано

| Пункт брифа | Где | Статус |
|---|---|---|
| 1. окно/оверлей: размер, центр, фон, радиус, тень | LOGIN-SPEC §1 | есть, все числа с метками |
| 2. вкладки «Вход»/«QR» | §0.1, §1.3, §4 | **вкладок в дампе нет** — QR это вторая колонка; описано как есть |
| 3. поля login/password, show/hide, autocomplete | §3.1–3.3 | есть (email/phone + пароль + токен) |
| 4. кнопки «Войти», «Забыли пароль?», «Зарегистрироваться» | §3.4 | есть + secondary/restore/back |
| 5. captcha-зона (интерактив, без обхода) | §5 | есть, 480/78, 24×16, policy P0 |
| 6. MFA-экран (totp/sms/backup/password + webauthn) | §6 | есть, слайды и лимиты из модуля 572469 |
| 7. ошибки 401/429/captcha/сеть, `role=alert` | §7 | 13 кодов, позиции и тексты |
| 8. состояния `checking/no-session/validating/captcha/mfa/ready/error` | §8 | 7/7 + подсостояния |
| 9. a11y: фокус, tab-порядок, labels, aria-live, контраст | §10 | 12 пунктов + tab-order списки |
| 10. адаптив 1280×820 и 2048×1106 | §11 | 6 условий, брейкпоинты 830/485 |
| машиночитаемо: states/elements/sizes/tokens | `login-spec.json` | 7 / 54 / 55 / 55 токенов |

Сводно: **7 состояний**, **54 элемента**, **50 текстов**, **13 ошибок**, **55 размеров**,
**8 пробелов/рисков**, **6 условий адаптива**.

## 2. Что `[M]`, а что `[A]` (честно)

**`[M]` (измерено на пикселях, `ui/00-app.png` / R-02, R-06):**
- фон окна `#121214` (`--bg-app`), карточка `#242429` (`--surface-2`), поле `#1e1f22` (выведено из 12% чёрного),
  `#2c2d32` (`--surface-4`) для captcha-блока, `#202024` (`--surface-1`) для пунктов MFA.

**`[D]` (точные px из CSS/JS 1.0.162):** 480 / 784 / 960 (invite), 32 (padding), 8 (radius),
64 (gap), 240 (QR-колонка), 176×176 (QR-контейнер), 160 (QR), 50 (overlay/пункт MFA),
442 (MFA-модалка), 40 (input), 38 (кнопка), 24/16/12 (паддинги), 10 с (backup_ack),
лимиты 6/6/10/8/11, брейкпоинты 830/485, тень `0 2px 10px rgba(0,0,0,.2)`.

**`[T]`:** все цвета/типографика/спейсинг/радиусы/тени, у которых есть токен; список — §12.

**`[A]` (наше решение, не измерение):**
- центрирование карточки и непрозрачный фон гейта (в дампе центрирование снаружи `AuthBox`);
- замена дамповых значений без токенов: тень → `--shadow-elevation-high`; 5px radius captcha → `--radius-sm`;
  border 8% → `--border-subtle`; фон/модули QR (#fbfbfb / #121214) — из токенов;
- вычисленная высота карточки ≈362 (арифметика из `[D]`, отмечена как расчёт);
- русские тексты (смысл — en-catalog `[D]`, формулировки — `[A]`);
- состояния 429/captcha/error-тексты и поведение кнопок (по AUTH-CONTRACT/ADR-3);
- варианты `max-height`/скролла для низких окон и зума 200%.

**«Как в оригинале» без вкладок:** расширенная карточка = 32 + 416 + 64 + 240 + 32 = 784 `[D]`;
QR скрывается при ≤830 `[D]`. Это зафиксировано как факт дампа, а не как наша компоновка.

## 3. Проверки (полный вывод — `PM/netdesign/token-check.txt`)

```
rg "url\(|@font-face" waves/RW-39/NET/LOGIN-SPEC.md     → (пусто)
token-check из брифа                                    → tokens ok 58
JSON.parse(login-spec.json)                             → valid json
токены в полях login-spec.json                          → 55, missing 0
```

Дополнительно: `states 7 / elements 54 / errors 13`, дублей id нет.

## 4. Чего нет и что осталось (пробелы)

| # | Пробел | Влияние | Как закрывать |
|---|---|---|---|
| G-01 | `NET/LOGIN-UI-RE.md` отсутствует | среднее | сверить §1/§3/§6, когда появится |
| G-02 | нет PNG/JPEG окна входа (`00-app.png` — ready) | среднее | визуальная сверка в согласованном окне; числа уже `[D]` |
| G-03 | hover/active/focus окна входа не снимались | низкое | токены `--state-*`, `--accent-*` |
| G-04 | иконка в центре QR — ассет | низкое | inline-SVG 50×50 |
| G-05 | логотип Discord — ассет | низкое | текстовая марка `DL` 36×112 |
| G-06 | 5px radius / 8% border без токенов | низкое | заменены токенами, помечено `[A]` |
| G-07 | тексты — перевод смысла, не офиц. локализация | низкое | ревью PM/владельца |
| G-08 | QR/remote-auth — чужой флоу | высокое | только по решению владельца; флаг фичи, по умолчанию выкл. |

Не сделано (запрещено условиями): окно/Discord не запускались, сеть не использовалась,
`cargo`/тесты/commit не выполнялись, ассеты не копировались, пароль/код нигде не показаны,
советов по обходу captcha нет.

## 5. Для NET-D2/F1 — дельта к текущему спайку

Текущие `ui/features/auth.js` + `ui/components/auth.css` реализуют только token-import
(`idle/validating/ready/error`, карточка 442, id `auth-reveal`). Целевое: 7 состояний контракта,
карточка 480/784, `#auth-login`/`#auth-password`, `#auth-captcha`, `#auth-mfa`,
`#auth-qr`, `data-action="auth-logout"` (таблица — §14 спеки). Старые id/селекторы, на которых
держатся тесты RW-38, сохраняются.

## 6. md5

Вход (read-only, `reference-oracle/`):

```
4fbb7d4afc37f27d16f65a66ba146d27  decoded/43644898c0897078_0   (login html)
b581000d340631f5ffdada7d6c61797a  decoded/dbd05750a1cb3484_0   (login chunk 786142)
8b11ff1a7f8451fe85b489b6e1310e8c  decoded/f0cac74b6ccdca5e_0   (qr module 52423)
e0c020f12d8fe0fc6b90925736414f85  decoded/8fed1c84bcaee32d_0   (mfa module 572469)
4f589a8a0ca3305d31aba91564ed3fcf  out/bundles/348181...css.bin  (authBox)
e21b964cebe355ac15b3a7d52a7185d5  out/bundles/351200...css.bin  (login layout)
a5f56df860a295b412a728349971a39f  out/bundles/461123...css.bin  (qr + captcha)
4fe504d86a1852328606138f5019a1cb  out/bundles/715528...css.bin  (mfa)
b3eac679ddf507720e99749d3ceefe3e  out/bundles/964940...css.bin  (input)
630d448ac34a78835b2d9b3c201e597c  ui/tokens.json
dfdb70bb7e4b2146a481319c95387e9c  en-catalog.json
62fffb9cb8bdd3429cd45160a77a2525  waves/RW-39/DESIGN/tokens.css
```

Выход:

```
4617048fec45587dfaf0527cfdd28bd2  waves/RW-39/NET/LOGIN-SPEC.md
bf223366567aa02323d6ae2805cf40f6  waves/RW-39/NET/login-spec.json
```

Полный список с путями — `PM/netdesign/md5.txt`.

## 7. Файлы, которых я касался

Созданы (только мои): `NET/LOGIN-SPEC.md`, `NET/login-spec.json`,
`NET/PM/netdesign/{probe.txt,token-check.txt,md5.txt,REPORT.md}`.
Остальное — read-only (дамп, `DESIGN/tokens.css`, `REFERENCES.md`, `AUTH-RE.md`, `AUTH-CONTRACT.md`,
`ADR-3-login.md`, существующий спайк `ui/features/auth.js` и `ui/components/auth.css` — только чтение).
