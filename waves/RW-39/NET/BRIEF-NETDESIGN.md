# BRIEF NET-DESIGN — дизайн окна входа «как в оригинале»

Владелец: NET-DESIGN. PM: DeepSeek. Read-only по дампу, без сети/окна.
Вход: `NET/AUTH-CONTRACT.md`, `NET/LOGIN-UI-RE.md` (если готов), `NET/AUTH-RE.md` A-01…A-08,
`/login` HTML в `reference-oracle/app-asar|decoded`, `ui/00-app.png`, `DESIGN/tokens.css`,
`DESIGN/REFERENCES.md`.
Если `LOGIN-UI-RE.md` ещё нет — работай по `/login` и `AUTH-RE`, пробелы помечай `[A]`.

## Выход
- `NET/LOGIN-SPEC.md` — спецификация окна входа:
  1. окно/оверлей: размер, центрирование, фон, радиус, тень;
  2. вкладки: «Вход» / «QR» (если есть в дампе);
  3. поля: login (email/phone), password, show/hide, autocomplete;
  4. кнопки: «Войти», «Забыли пароль?», «Зарегистрироваться»;
  5. captcha-зона: контейнер, размер, placeholder, пояснение «решает пользователь»;
  6. MFA-экран: методы (totp/sms/backup/password), код, «Отправить SMS», таймер/повтор;
  7. ошибки: 401/429/captcha/сеть — тексты и позиция; `role=alert`;
  8. состояния: `checking/no-session/validating/captcha/mfa/ready/error`;
  9. a11y: фокус, tab-порядок, labels, `aria-live`, контраст;
  10. адаптив 1280×820 (окно спайка) и 2048×1106 (референс).
- `NET/login-spec.json` — машиночитаемо (states, elements, sizes, tokens).
- Логи `NET/PM/netdesign/`.

## Правила
- Каждое число — `[M]/[T]/[D]/[A]` + источник; значения — из `tokens.css`, не «на глаз».
- Ассеты Discord не копировать; шрифты/emoji системные, иконки inline.
- Пароль/код **не** показывать в спеке как значения; только поля.
- Никаких советов по обходу captcha/анти-детекту.

## Проверка
```bash
rg -n "url\(|@font-face" waves/RW-39/NET/LOGIN-SPEC.md || true
node -e "const fs=require('fs');const t=fs.readFileSync('waves/RW-39/DESIGN/tokens.css','utf8');const s=fs.readFileSync('waves/RW-39/NET/LOGIN-SPEC.md','utf8');const used=[...s.matchAll(/--[a-z0-9-]+/g)].map(m=>m[0]);const miss=[...new Set(used)].filter(v=>!t.includes(v+':'));if(miss.length){console.error(miss);process.exit(1)}console.log('tokens ok',new Set(used).size)"
```

## Отчёт
`NET/PM/netdesign/REPORT.md`: состояния покрыты, что `[M]`/`[A]`, чего нет в дампе, md5.
