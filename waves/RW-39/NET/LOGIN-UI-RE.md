# LOGIN-UI-RE — окно входа Discord 1.0.162: разметка, captcha, 2FA

**Волна:** RW-39, отдел NET, роль **NET-R2** (RE login UI). PM: DeepSeek. Дата: 2026-10-23.
**Режим:** read-only. Сеть не использовалась, Discord не запускался, ассеты не копировались, окно не открывалось.
**Машиночитаемая версия:** `waves/RW-39/NET/login-ui-re.json`.
**Логи:** `waves/RW-39/NET/PM/netr2/` (`00-md5-inputs.log` … `09-window.log`).
**Метки:** `[D]` — прямая цитата кода/артефакта; `[M]` — измеренное значение (computed CSS-токены
живого клиента); `[A]` — вывод/реконструкция без прямого доказательства (аналог `[T]` в AUTH-RE).
Колонка «метод»: `static` (чтение JS/CSS), `static+computed` (код + живые токены), `analysis`.

**Коротко.** Восстановлены: компоновка окна входа (480/784 px, тёмная тема, две колонки
«форма + QR»), все поля и тексты, точные стили/токены, состояния ошибок, двухшаговая регистрация,
QR-логин (включая его протокол — частично закрывает пробел N-02 из `AUTH-RE.md`), механика captcha
(сервисы, sitekey, поля, заголовки, коды) и полный 2FA-экран (5 методов, SMS-send, слайды,
таймеры, ошибки). Чего нет: разметки модалки captcha (lazy-чанк не попал в дамп), рендерера
hCaptcha, скриншота/DOM-снимка именно окна входа.

---

## 1. Провенанс (входы и md5)

| Ключ | Файл | Что это | Размер | md5 |
|---|---|---|---|---|
| `LOGIN_CHUNK` | `decoded/dbd05750a1cb3484_0` | `/assets/ae9016b5ac149349.js` — login/register/invite-чанк | 149 094 | `b581000d340631f5ffdada7d6c61797a` |
| `MFA_CHUNK` | `decoded/8fed1c84bcaee32d_0` | `/assets/9efedacca22a4bc6.js` — весь 2FA-UI | 9 759 | `e0c020f12d8fe0fc6b90925736414f85` |
| `WEB` | `decoded/4d2a64cb5df5fc4e_0` | `/assets/web.4d4b2f75fdea5470.js` — основной бандл | 12 410 287 | `934b1289b7ebf17bc0fd09569b23b438` |
| `QR_CHUNK` | `decoded/f0cac74b6ccdca5e_0` | `/assets/0c2bfaf5557c575e.js` — QR/remote-auth UI | 22 033 | `8b11ff1a7f8451fe85b489b6e1310e8c` |
| `LOGIN_HTML` | `decoded/43644898c0897078_0` | `https://discord.com/login` (SPA-shell + `GLOBAL_ENV`) | 74 817 | `4fbb7d4afc37f27d16f65a66ba146d27` |
| `CSS_AUTH` | `decoded/a92f46d41f311d85_0` | `348181.a3b3bc62ee2f1223.css` — authBox/поля/кнопки | 50 278 | `4f589a8a0ca3305d31aba91564ed3fcf` |
| `CSS_LOGINPAGE` | `decoded/68156f1de4207c34_0` | `351200.ec1294199b2e7a31.css` — layout входа | 1 784 | `e21b964cebe355ac15b3a7d52a7185d5` |
| `CSS_INPUT` | `decoded/9844453f9a221800_0` | `964940.62a0e9d626b739d9.css` — field/input | 45 662 | `b3eac679ddf507720e99749d3ceefe3e` |
| `CSS_QR` | `decoded/7d04e08304d9e681_0` | `461123.de02227c725f5838.css` — QR + `authCaptcha` | 2 802 | `a5f56df860a295b412a728349971a39f` |
| `CSS_MFA` | `decoded/33af86ca2e7626d9_0` | `715528.a04399038393a931.css` — MFA | 1 371 | `4fe504d86a1852328606138f5019a1cb` |
| `CSS_MODAL` | `decoded/0efa31d45bd04ece_0` | `555628.eb33ac71a27a7069.css` — Modal | 8 902 | `b8c6782a0edc227b7648eb5c24889b61` |
| `CSS_AUTH2` | `decoded/024ca22b022f0c24_0` | `9c4f0bfdbdf4f0e8.css` — sidebar-вариант входа | 4 959 | `e6c6f6109aa2cf8ac829b28102fb0418` |
| `TOKENS` | `ui/tokens.json` | 4795 computed CSS-токенов живого клиента (dark) | — | — |
| `MAIN_ARCH` | `docs/re/main-arch.md` | окно/`BrowserWindow` | — | — |
| `I18N` | `en-catalog.json`, `ru-catalog.json` | тексты по ключам | — | — |

Смещения — **байтовые** (`@NNNN`), как в `AUTH-RE.md`; все файлы ASCII-only, поэтому offset == позиция
в UTF-8-строке. Живые значения токенов — из `ui/tokens.json` (сняты с работающего клиента, тема dark),
это `[M]` для значений токенов, но не доказательство того, что каждый токен применён именно в login.

---

## 2. Окно и компоновка

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| WIN-01 | Отдельного login-окна нет: `/login` — маршрут внутри главного окна Electron. Главное окно: `1280×720`, `minWidth/minHeight` по умолчанию `940×500`, `frame:false`, `title:"Discord"`, фон `getBackgroundColor()` | [D] | `docs/re/main-arch.md:218-230`; `core/bundle.js @141300..@142450` | static |
| WIN-02 | Фон приложения (под логином): `.appMount__51fd7` — `position:absolute; width/height:100%`, `background: var(--background-base-lowest)` = `hsl(240 5.263% 7.451%)` ≈ `#121214` | [D]+[M] | `c8010f8e039835e3_0` (419070…css); `TOKENS --background-base-lowest` | static+computed |
| WIN-03 | `/login` рендерится компонентом `lC` через `s7 = iT(lC)`; маршрут объявлен в таблице `rE` (`path: D.BVt.LOGIN`) | [D] | `LOGIN_CHUNK @129389..@130389` (`s7=iT(lC)`), `@136631` | static |
| WIN-04 | `iT` — анимационная обёртка (scale/translateY на появление), её CSS-модуль `706192` в декодированном наборе отсутствует (внешний центр-контейнер — gap) | [D]+[A] | `LOGIN_CHUNK @47816` | static |
| WIN-05 | Коробка входа: базово `480px`, у логина включён `expanded` → класс `authBoxExpanded__921c5 authBox__921c5`, ширина **784px** | [D] | `LOGIN_CHUNK @63565`; `CSS_AUTH @27248`; карта классов `2790bad9686ed34c_0` (`PR`) | static |
| WIN-06 | Внутри коробки: `discordLogo` (130×36, `margin-bottom:16px`, SVG dark `bbbc3d376d38e7bc.svg` / light `34120ace1e4e4d29.svg`) + `centeringWrapper` (`text-align:center; width:100%; min-height:540px`) | [D] | `WEB @k` (`854378`); `CSS_AUTH @26763`, `@26908`, `@27514` | static |
| WIN-07 | Десктопная компоновка логина: `Stack direction=horizontal gap=64`: форма (`mainLoginContainer`, `flex-grow:1`) + QR-блок (`240px`). Расчётная ширина формы = `784 − 2×32 − 64 − 240 = 416px` | [D]+[A] | `LOGIN_CHUNK @63565`; `CSS_LOGINPAGE @289`; `CSS_QR @0` | static |
| WIN-08 | `@media (max-width:830px)`: expanded-коробка ужимается до `max-width:480px`, QR-блок (`qrLogin:not(.qrLoginHorizontal)`) скрывается | [D] | `CSS_AUTH @27975`, `@28376`; `CSS_QR @1038` | static |
| WIN-09 | `@media (max-width:485px)`: полноэкранный мобильный режим — `background:linear-gradient(to left bottom,#3d4046,#1e1e23)`, `padding:20px 16px`, коробка на весь экран | [D] | `CSS_AUTH @27489` | static |
| WIN-10 | Тема логина всегда **dark**, независимо от темы пользователя: `AuthBox` по умолчанию `theme=NJ8.DARK` + `disableAdaptiveTheme:true` | [D] | `WEB @854378` (`let k=function(...){theme:d=m.NJ8.DARK...}`) | static |
| WIN-11 | Встраиваемый вариант (invite sidebar, `min-width:900px`): `direction=vertical gap=24`, QR переключается в `horizontal` (QR слева, текст справа, `gap:24`), коробка получает `authBox__3689c` и в сайдбаре — `width:100%`, прозрачный фон | [D] | `LOGIN_CHUNK @106768` (`authBoxClassName:sP.sL`); `CSS_AUTH2 @3770`; `CSS_QR @246` | static |

---

## 3. Экран входа (`/login`)

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| LU-01 | Порядок блоков: заголовок → поле e-mail/телефон → поле пароля → «Забыли пароль?» → кнопка «Вход» → «Нужна учётная запись? Зарегистрироваться» | [D] | `LOGIN_CHUNK @61504..@62950` | static |
| LU-02 | Заголовок: `P.hE` (heading-xl/semibold, 24px/600, `text-strong`) → **«С возвращением!»** (`7fNJgA`), `margin-bottom:8px`; подзаголовок `P.tK` (text-md/normal, 16px/400, `text-default`) → **«Мы так рады видеть вас снова!»** (`euS7r4`) | [D] | `LOGIN_CHUNK @61504`; `WEB @854378` (`C`,`O`); `I18N` | static |
| LU-03 | Если в state `passwordReset` — подзаголовок заменяется на «Your password has been reset. Log in with your new password.» (`N9U/9X`; в RU-каталоге ключа нет → fallback EN) | [D] | `LOGIN_CHUNK @61332`; `I18N` | static |
| LU-04 | На Android-web подзаголовок не выводится (`if(false===isAndroidWeb())`); на desktop — выводится | [D] | `LOGIN_CHUNK @61420..@61540` | static |
| LU-05 | Поле 1 — `ev.A` (country-aware input): label **«Адрес электронной почты или номер телефона»** (`tUjnxr`), `required`, `autoComplete:"username webauthn"`, `autoCapitalize:"none"`, `autoCorrect:"off"`, `spellCheck:"false"`, значение из query `email`/`login` | [D] | `LOGIN_CHUNK @61902..@62100`; `I18N` | static |
| LU-06 | Ошибки поля 1 берутся из `errors.login ?? errors.email` (первый элемент массива); при ошибке `password` фокус на пароль, иначе на login/email | [D] | `LOGIN_CHUNK @58970..@59300` (`lg`,`k`) | static |
| LU-07 | Поле 2 — `P.pd` (AuthBox input): label **«Пароль»** (`CIGa+7`), `type:"password"`, `name:"password"`, `autoComplete:"current-password"`, `required`, `spellCheck:"false"`, ошибка из `errors.password` | [D] | `LOGIN_CHUNK @62110..@62240`; `I18N` | static |
| LU-08 | «Забыли пароль?» (`wWIufs`) — текстовая кнопка 14px (`text-sm/normal`), блок `margin-top:4px; margin-bottom:20px`; клик сначала фокусирует поле логина, затем `forgotPassword(login)` | [D] | `LOGIN_CHUNK @59764`, `@62350`; `CSS_LOGINPAGE @118` | static |
| LU-09 | Кнопка «Вход» (`dKhVQN`): `type=submit`, `fullWidth`, primary, размер md → `min-height:38px` + рамка = **40px**, `loading` при `loginStatus===LOGGING_IN`; обёртка `margin-bottom:8px` | [D] | `LOGIN_CHUNK @62560..@62600`; `CSS_AUTH @3105` | static |
| LU-10 | «Нужна учётная запись?» (`tmE73r`, 14px/16px) + «Зарегистрироваться» (`pV8xeR`, ссылка 14px, `margin-inline-start:4px`, `display:inline-block`), блок `margin-top:4px` | [D] | `LOGIN_CHUNK @62754..@62900`; `CSS_LOGINPAGE @118`, `@170` | static |
| LU-11 | Переход в регистрацию: `mode=register`, сохраняет введённый e-mail в query; для invite/gift/template — соответствующий путь, иначе `/register` (+`redirect_to`) | [D] | `LOGIN_CHUNK @62900..@63100` | static |
| LU-12 | Мультиаккаунт: если есть сохранённые аккаунты и пользователь не отклонил выбор — экран «Выбрать учётную запись» (`bVbB63`), текст «Выберите учётную запись для входа или добавьте новую.» (`0M5fN7`), кнопка «Вход» (`DSN+hw`), «Добавить учётную запись» (`9g2mqT`), «Назад» (`1MrpWO`) | [D] | `LOGIN_CHUNK @57800..@58800`; `I18N` | static |
| LU-13 | При мультиаккаунтном логине запрос идёт с пустым заголовком `authorization:""` (не наследовать текущий токен) | [D] | `WEB @551990`; `AUTH-RE A-03` | static |
| LU-14 | Пароль-лесс/passkey: поле логина объявлено как `username webauthn` (conditional UI), и в QR-блок передаётся `conditionalMediationAbortController`; отдельный вход по passkey — ссылка «Или войти с помощью ключа доступа» (`/kpMDt`) | [D] | `LOGIN_CHUNK @61902`, `@63695`; `QR_CHUNK @52423` | static |
| LU-15 | Состояние «detecting account» (browser handoff): спиннер + «Обнаружение учётной записи» (`S6RMNA`) + «Пытаемся обнаружить учётную запись Discord в вашем браузере…» (`YZiJbh`) | [D] | `LOGIN_CHUNK @69400..@69600` | static |
| LU-16 | Вход по одноразовой ссылке (`/login/one-time`): экраны «Вход в Discord» (`qq4tjT`)/«Приложение Discord не может быть открыто» + «Нам не удалось найти приложение…» (`CVxYRo`), «Недействительная ссылка для входа» (`RtCSr1`) + «Вернуться ко входу» (`j3cG2p`) | [D] | `LOGIN_CHUNK @129887..@130389` | static |
| LU-17 | Mobile-web handoff (`?handoff_token=…`): «Уже вошли в приложение Discord?» (`uJ1JsY`) + «Выполните вход в одно касание.» (`GHVWAs`), ошибки «Невозможно получить данные авторизации.» (`EPt55r`) / «Срок действия сеанса авторизации истёк…» (`g87kTp`), кнопка «Открыть приложение» (`NcC759`); обмен `POST HANDOFF_EXCHANGE {key,handoff_token}` | [D] | `LOGIN_CHUNK @56900..@57800`; `LOGIN_CHUNK @3044..@3962` | static |
| LU-18 | Подтверждение по IP (код 70007): отдельный экран «Введите код подтверждения, чтобы подтвердить свою личность» (`w55Oco`) + «Мы не узнаём это место…» (`CfRZBj`) + ссылка «Отправить код ещё раз» | [D] | `LOGIN_CHUNK @69700..@69900`; `I18N` | static |
| LU-19 | Сброс пароля по SMS-коду: «Введите код подтверждения, чтобы сбросить пароль» (`+xqy3d`) + «Мы отправили сообщение на номер…» (`ef4uZ7`) | [D] | `LOGIN_CHUNK @69900..@70100`; `I18N` | static |
| LU-20 | Экраны-заглушки: «Аккаунт запланирован к удалению» (`ZFWofo`) / «Аккаунт отключён» с кнопкой «Вернуться к авторизации» (`JhDw5o`) и «Передумали? …» (`js2rr5`) | [D] | `LOGIN_CHUNK @54200..@55200`; `I18N` | static |

---

## 4. Стили и токены (точные значения)

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| ST-01 | Коробка: `background: var(--modal-background)` = `hsl(240 6.494% 15.098%)` ≈ `#242429`; `border-radius: var(--radius-sm)` = **8px**; `box-shadow: var(--legacy-elevation-high)` = `0 2px 10px 0 hsl(0 0% 0% / .2)`; `padding: var(--custom-auth-box-auth-box-padding)` = **32px**; `font-size:18px`; `color: var(--text-muted)` = `hsl(232.5 3.96% 60.392%)` ≈ `#96979e` | [D]+[M] | `CSS_AUTH @26908`; `TOKENS` | static+computed |
| ST-02 | Шрифт: `--font-primary` = `"gg sans","Noto Sans","Helvetica Neue",Helvetica,Arial,sans-serif`; semibold = 600 | [M] | `TOKENS` | computed |
| ST-03 | Заголовок: `heading-xl/semibold` → `font-size:24px; font-weight:600; line-height:1.25`; `text-strong` ≈ `#fbfbfb` | [D]+[M] | `CSS` (`.heading-xl\/semibold__9a9f9`, `058616e449204655_0`); `TOKENS` | static+computed |
| ST-04 | Подзаголовок: `text-md/normal` → `16px/400`, `line-height:1.25`, `text-default` ≈ `#efeff1` | [D]+[M] | там же; `TOKENS` | static+computed |
| ST-05 | Ссылки внутри коробки: `color: var(--text-link)` = `hsl(212.795 82.564% 61.765%)` ≈ `#4d96ee`; `:hover` — `text-decoration:underline` | [D]+[M] | `CSS_AUTH @27000`; `TOKENS` | static+computed |
| ST-06 | Метка поля (FormField): `text-md/medium` → 16px/500, `text-strong`; звёздочка `*` — `text-md/normal`, `text-feedback-critical`; метка и контрол в grid, `margin-bottom:8px`, `gap:4px` | [D] | `WEB @452027`; `CSS_INPUT @1602`, `@1784` | static |
| ST-07 | Поле ввода: высота `--control-input-height-md` = **40px**; рамка `1px solid var(--input-border-default)` = белый ~20% (`hsl(240 4% 60.784%/.2)`); радиус `--radius-sm` 8px; фон `--input-background-default` = чёрный 12%; текст `--input-text-default` ≈ `#efeff1`; placeholder `--input-placeholder-text-default` ≈ `#8f9096` | [D]+[M] | `CSS_INPUT @5055`; `CSS_AUTH @11015`, `@11796`; `TOKENS` | static+computed |
| ST-08 | Фокус: `border-color: var(--input-border-active)` (= brand `#5865f2`) + `outline:2px solid` c `outline-offset:-2px`; hover меняет только border | [D]+[M] | `CSS_INPUT @5993`; `TOKENS` | static+computed |
| ST-09 | Ошибка поля: `data-error=true` → border `--input-border-error-default` ≈ `#f87e7a`, фон `--input-background-error-default` (красный 4%), текст `--input-text-error-default`; hover — `outline:2px solid` error | [D]+[M] | `CSS_INPUT @6212`, `@6571`; `TOKENS` | static+computed |
| ST-10 | Текст ошибки: иконка + `text-xs/normal` (12px/400), `text-feedback-critical` ≈ `#f87e7a`; контейнер `display:grid; gap:4px`; у MFA-ошибки `margin-top:8px` | [D]+[M] | `WEB @281595`; `CSS_INPUT @3929`; `CSS_MFA @517`; `TOKENS` | static+computed |
| ST-11 | Кнопка: `border-radius:8px`, `border:1px solid transparent`; размер по умолчанию `md` → `min-height:38px` + рамка = **40px**, padding `7px 15px`; primary: фон `--control-primary-background-default` = `#5865f2`, текст белый, hover `#4452bb`; secondary: фон белый 12%, текст ≈ `#fbfbfb` | [D]+[M] | `CSS_AUTH @60`, `@3105`, `@3650`, `@4158`; `TOKENS` | static+computed |
| ST-12 | «Забыли пароль?»/«Зарегистрироваться»/«Нужна учётная запись?» — `text-sm/normal` 14px, line-height `1.2857`; блок регистрации `margin-bottom:20px`, ссылка `margin-inline-start:4px` | [D] | `LOGIN_CHUNK`; `CSS_LOGINPAGE @118`, `@170` | static |
| ST-13 | QR-блок: ширина **240px**; контейнер QR **176×176**, `margin-bottom:32px`; сам QR-компонент рендерится `size=160` (160×160), `level:"M"`, `bg #ffffff`, `fg #000000`; оверлей-логотип **50×50** (`/assets/dd05fd1ea37e7747.png`) по центру; в horizontal-варианте QR `flex-shrink:0; margin-bottom:0`, текст слева, `gap:24` | [D] | `CSS_QR @0`, `@378`, `@849`; `QR_CHUNK @906118`, `@414121`; `f0cac @2393` | static |
| ST-14 | Login-раскладка: `header__1d5fc` (flex column, center, 100%), `mainLoginContainer__1d5fc` (flex column, align-start, flex-grow:1), `needAccount__1d5fc` 14px/16px, `smallRegisterLink__1d5fc` inline-block + `margin-inline-start:4px` | [D] | `CSS_LOGINPAGE @0..@613` | static |
| ST-15 | Спиннер QR: `wanderingCubes`, `aria-busy=true`, `aria-label` «QR code login is loading» (`BUGkVF`); при готовности объявляется «QR code login is now ready» (`j2p125`) | [D] | `QR_CHUNK @2393`; `I18N` | static |

---

## 5. Ошибки входа (что и где показывается)

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| ER-01 | Ошибки 50035 (`INVALID_FORM_BODY`) при `oldFormErrors:true` приходят плоским списком; UI раскладывает их по `login`, `email`, `password` и рисует под соответствующим полем; поле получает `data-error` | [D] | `WEB @10694717`; `LOGIN_CHUNK @58970`; `CSS_INPUT @6212` | static |
| ER-02 | После появления ошибок фокус переносится: `errors.password` → поле пароля; иначе `errors.email`/`errors.login` → поле логина | [D] | `LOGIN_CHUNK @58970..@59150` | static |
| ER-03 | Клиентской проверки «обязательное поле» в форме логина нет — только HTML `required`; кнопка не блокируется по пустым полям (кроме состояния загрузки). Текст «Password is required.» (`R98xD5`) относится к странице сброса пароля, не к логину | [D] | `LOGIN_CHUNK @62110..@62600`; `LOGIN_CHUNK @163050` (`reset_password`) | static |
| ER-04 | Коды логина: `50018 INVALID_PASSWORD`, `50022 INVALID_PHONE_NUMBER`, `40002 EMAIL_VERIFICATION_REQUIRED`, `70007 PHONE_VERIFICATION_REQUIRED`, `20011 ACCOUNT_SCHEDULED_FOR_DELETION`, `20013 ACCOUNT_DISABLED`; `suspended_user_token` → экран ACCOUNT_STANDING (не обычная ошибка) | [D] | `WEB @551777`, enum `t02`; AUTH-RE E-07/E-10 | static |
| ER-05 | 401 + `code 60003 MFA_REQUIRED` + `body.mfa` → step-up MFA-модалка поверх действия; 401 без этого — локальная обработка вызывающего | [D] | `WEB @1004970`, `@1002375` | static |
| ER-06 | 400 + `body.captcha_key` → модалка captcha и повтор запроса с `X-Captcha-*` (см. §8) | [D] | `WEB @1004547` | static |
| ER-07 | 429/503 — пауза per-URL по `retry-after` (или 5 c), backoff 1…60 c; 502/504/507/598/599/522/523/524 — ретраи (`retries:2` у логина/MFA) | [D] | `WEB @10692340`; AUTH-RE E-01/E-02/E-09 | static |
| ER-08 | Любая ошибка логина, не отнесённая к специальным экранам, throw-ится в компонент и превращается в field-errors; глобального «красного баннера» в форме логина нет | [D] | `LOGIN_CHUNK @59150..@59700`; `WEB @8979206` (`LOGIN_FAILURE`) | static |

---

## 6. Регистрация (`/register`)

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| REG-01 | `/register` → `ri = iT(lQ)`; заголовок **«Создать учётную запись»** (`wC4TlR`); коробка обычная (480px, не expanded) | [D] | `LOGIN_CHUNK @84407`, `@84967`, `@131000` | static |
| REG-02 | Два шага: шаг 0 — `Username` + `Your birthday` + кнопка «Далее» (`PDTjLN`); шаг 1 — `E-mail` + `Пароль` + promo-checkbox + consent + кнопка «Создать учётную запись» | [D] | `LOGIN_CHUNK @33186` (`nC`), `@33314` (`nm`) | static |
| REG-03 | Шаг 0: username приводится к нижнему регистру при вводе; под полем — подсказка/валидация 14px (error/available/suggestion: `z7c4bP`, `nDGqqq`); кнопка disabled, пока username пуст или дата рождения не разобрана | [D] | `LOGIN_CHUNK @30200..@30900`, `@28900..@30100` | static |
| REG-04 | Шаг 1: e-mail (`type=email`, `autoComplete:"username"`), пароль (`type=password`, `autoComplete:"new-password"`), promo e-mail (опциональный checkbox, текст зависит от эксперимента), consent-checkbox; кнопка disabled, пока пуст e-mail/пароль, не принят consent или идёт запрос | [D] | `LOGIN_CHUNK @25400..@27600` | static |
| REG-05 | Consent-текст: «Я прочитал(а) и согласен(на) с $link и $link Discord.» (`qMDAP0`); если consent не обязателен — пассивная строка «Нажимая кнопку “Создать учётную запись”, вы соглашаетесь с $link Discord и подтверждаете, что ознакомились с $link» (`KI+BSb`) | [D] | `LOGIN_CHUNK @24168`; `I18N` | static |
| REG-06 | Ошибки шага 1 раскладываются по `email`, `phone`, `password`, `username`, `global_name`, `date_of_birth`, `promotional_email_opt_in`, `fingerprint`, `invite`, `gift_code_sku_id`, `guild_template_code`, `consent` + generic | [D] | `LOGIN_CHUNK @22800..@23400` | static |
| REG-07 | Приглашение/гифт/шаблон меняют шапку и добавляют поля (`invite`, `gift_code_sku_id`, `guild_template_code`); тело `POST /auth/register` — см. AUTH-RE A-21 | [D] | `LOGIN_CHUNK @27600..@28600`; AUTH-RE A-21 | static |
| REG-08 | Переход обратно: «Уже есть учётная запись? Откройте приложение» (`renMUD`) — текстовая кнопка 14px внизу обеих шагов | [D] | `LOGIN_CHUNK @27034`, `@32700`; `I18N` | static |
| REG-09 | Возрастной gate: при `date_of_birth` < 13 — экран «Вам должно быть не менее 13 лет…» (`WqEH4D`) + «Узнать больше» (`hvVgAZ`), отправка блокируется | [D] | `LOGIN_CHUNK @71886..@72100` | static |

---

## 7. QR-вход (не «вкладка», а вторая колонка)

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| QR-01 | Отдельной вкладки нет: на десктопе QR-блок стоит **справа от формы** в той же коробке (`horizontal`-stack, `gap:64`), ширина 240px; на узком экране (<830px) скрывается | [D] | `LOGIN_CHUNK @63695`; `CSS_QR @1038` | static |
| QR-02 | Заголовок блока: «Войти с помощью QR-кода» (`UPiHaL`), `heading-xl/semibold`, `margin-bottom:8px`; описание: «Отсканируйте код с помощью **мобильного приложения Discord**, чтобы сразу же войти в систему.» (`Qq+A6i`, ссылка `$b`) | [D] | `QR_CHUNK @3203`; `I18N` | static |
| QR-03 | Третья строка — ссылка «Или войти с помощью ключа доступа» (`/kpMDt`), `text-sm/normal`, disabled при активном passwordless | [D] | `QR_CHUNK @3203`; `I18N` | static |
| QR-04 | В состоянии загрузки — спиннер `wanderingCubes` с `aria-label` «QR code login is loading» (`BUGkVF`); когда QR готов — объявление «QR code login is now ready» (`j2p125`); у изображения `aria-label` «QR code to log in with the Discord mobile app» (`SzYj9v`), `tabIndex=0` | [D] | `QR_CHUNK @2393`; `I18N` | static |
| QR-05 | Значение QR = `https://discord.com/ra/{fingerprint}`, где fingerprint — base64url(SHA-256(SPKI публичного ключа)); оверлей — PNG-логотип 50×50 | [D] | `QR_CHUNK @6333`, `@6891`, `@414121` | static |
| QR-06 | Состояние «pending_ticket»: аватар 120, заголовок «Проверьте телефон!» (`apGCUT`), текст «Вход: username» (`Cbl5JK`), ссылка «Это не я, начинаем заново» (`nOOhwo`), `margin-top:16px`, цвет `--brand-500` | [D] | `QR_CHUNK @3203`; `CSS_QR @975`; `I18N` | static |
| QR-07 | Транспорт: `wss://remote-auth-gateway.discord.gg/?v=2` (из `GLOBAL_ENV.REMOTE_AUTH_ENDPOINT`); сообщения — JSON `{op:…}` | [D] | `QR_CHUNK @6333`; `LOGIN_HTML @2329` | static |
| QR-08 | Handshake: клиент генерирует RSA-OAEP-2048 (SHA-256) ключ, отправляет `init {encoded_public_key}` (base64 SPKI); сервер — `hello {heartbeat_interval,timeout_ms}`; клиент шлёт `heartbeat`, ждёт `heartbeat_ack`; при `nonce_proof` расшифровывает `encrypted_nonce` и отвечает `nonce_proof {nonce}` | [D] | `QR_CHUNK @1012..@6900` | static |
| QR-09 | `pending_remote_init {fingerprint}`: клиент сверяет fingerprint со своим (иначе ошибка) и показывает QR; `pending_ticket {encrypted_user_payload}` расшифровывается в строку `id:discriminator:avatar:username` (UTF-8), аватар `"0"` → null | [D] | `QR_CHUNK @6891`, `@7628`, `@1949` | static |
| QR-10 | `pending_login {ticket}`: `POST /users/@me/remote-auth/login {ticket}` → `{encrypted_token}`; клиент расшифровывает его приватным ключом → **auth-токен** → `loginToken(token,false)`; затем `onAuthenticateSuccess(fingerprint)` | [D] | `QR_CHUNK @7182`; AUTH-RE A-18 | static |
| QR-11 | `cancel` → перезапуск handshake с новым ключом; при ошибке WS — backoff 1.5…30 c (`new w.A(1500,3e4)`); `EventSource`-подобного fallback нет | [D] | `QR_CHUNK @6900..@7100`, `@8700` | static |
| QR-12 | QR-флоу — чужой (передача учётных данных между устройствами). По уставу NET (AUTH-RE §8.8) **не реализуем**; факты выше нужны только для корректного описания оригинала | [A] | `AUTH-RE §8.8`; `NET/CHARTER.md` | analysis |

---

## 8. Captcha

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| CAP-01 | Триггер: любой ответ `400` с `body.captcha_key` (в т.ч. `/auth/login`) перехватывается глобальным `interceptResponse` | [D] | `WEB @1004547` | static |
| CAP-02 | Действие: лениво грузится чанк `"536037"` (модуль `165508`) и модуль `888548`; вызывается `showCaptchaAsync(extractCaptchaPropsFromResponse(body))`, ожидается пользовательский ввод | [D] | `WEB @1004560..@1004700`; `cd0cb5bf249f258f_0` | static |
| CAP-03 | Поля ответа сервера: `captcha_service`, `captcha_sitekey`, `captcha_session_id`, `captcha_rqdata`, `captcha_rqtoken`, `should_serve_invisible`, `user_flow`; маппинг → `{captchaService, sitekey, captchaSessionId, options:{rqdata,rqtoken,serveInvisible,userflow}}` | [D] | `WEB @3240400..@3240600` | static |
| CAP-04 | После решения клиент повторяет исходный запрос с заголовками: `X-Captcha-Key: <captcha_key>` (обязателен), `X-Captcha-Rqtoken: <captcha_rqtoken>` (если есть), `X-Captcha-Session-Id: <captcha_session_id>` (если есть) | [D] | `WEB @1004797..@1004930` | static |
| CAP-05 | Сервисы (enum): `hcaptcha`, `recaptcha`, `recaptcha_enterprise`, `turnstile` | [D] | `WEB @10697585` (модуль `319400`) | static |
| CAP-06 | Собственный рендерер в бандле — **reCAPTCHA Enterprise**: `getSiteKey(provided)` → `provided ?? (production ? "6LeYqFcqAAAAAD6iZesmNgVulsO4PkpBdr6NVG6M" : "6LdtfVMqAAAAAMurhtf2pDhK0oqD4eLqeQPh025y")`; скрипт `https://www.google.com/recaptcha/enterprise.js?render=<sitekey>`; `grecaptcha.enterprise.ready()` → `execute(sitekey,{action})`; при размонтировании скрипт и `.grecaptcha-badge` удаляются | [D] | `WEB @3241322..@3241900` | static |
| CAP-07 | hCaptcha: в клиенте есть только значение сервиса и i18n-строки «Verify by hCaptcha» (`3I8ubP`), «Captcha» (`wjVOPz`); **рендерера hCaptcha в декодированном наборе нет** — он в недоступном lazy-чанке модалки (gap) | [D]+[A] | `WEB @10697585`; `I18N`; `GAP-02` | static |
| CAP-08 | Модалка: `openModalLazy` → компонент с props `{onCaptchaVerify(captcha_key, captcha_rqtoken), captchaService, sitekey, onReject, ...options}`; `Layer`, `modalKey = "Captcha"`; `showCaptchaAsync` резолвится в `{captcha_key, captcha_rqtoken, captcha_session_id}`, отмена → `CaptchaCancelError` | [D] | `cd0cb5bf249f258f_0 @165508` | static |
| CAP-09 | Ошибки/состояния модалки: `CaptchaError = {CANCEL:"cancel", ERROR:"error", EXPIRED:"expired"}`; `CaptchaCancelError("Captcha cancelled")`; при 400+captcha_key сообщение ошибки = «Invalid captcha response. Please try again.» (`3s/vDN`), `code = -1`, `captchaFields = body` | [D] | `WEB @3240103..@3240300`, `@10697814..@10698300`; `I18N` | static |
| CAP-10 | Где рендерится (login): **поверх приложения модальным окном**, не внутри auth-box. CSS-класс `.authCaptcha_e16417` (`width:480px; background:var(--primary-630); border-radius:5px 5px 0 0; padding:24px 16px`) в бандле есть, но в декодированном JS **не referenced** — вероятно, им пользуется тот же недоступный чанк модалки (`[A]`) | [D]+[A] | `CSS_QR @1038`; grep `authCaptcha` = только CSS | static |
| CAP-11 | Коды-триггеры: числового кода у капчи нет — только HTTP 400 + наличие `captcha_key`. Тело может содержать `captcha_key` массивом (ошибка берёт `[0]`) | [D] | `WEB @319400`, `@10697814` | static |
| CAP-12 | Пост-логин gate `REQUIRE_CAPTCHA` (required_action): полноэкранная «Verification Required» (`Iz0kDg`) + текст (`0rqMV5`), отдельный reCAPTCHA-виджет с **фиксированным** sitekey `6Lef5iQTAAAAAKeIvIY-DeexoO3gj7ryl9rLMEnn` (не из ответа сервера), `POST /users/@me/captcha/verify {captcha_key}`; при ошибке виджет перемонтируется (key++), футер: «Think you're seeing this by mistake?» + support/logout | [D] | `WEB @11782128`, `@665095`; `I18N` | static |
| CAP-13 | Вспомогательные endpoint'ы: `/users/@me/captcha/verify`, `/captcha/decider` (тестовый decider) | [D] | `WEB @274110`, `@274146` | static |
| CAP-14 | Аналитика: `CAPTCHA_SERVED`, `CAPTCHA_EVENT`, `CAPTCHA_FAILED`, `RECAPTCHA_MODAL_EVENT`; метрика `captcha_serve_volume_distribution` с тегом `user_flow` (агрегация 30 c) | [D] | `WEB @320028`, `@3240065`, `@3242091` | static |
| CAP-15 | **Только интерактив.** В бандле нет решателя/обхода/стороннего сервиса капчи: клиент показывает виджет человеку и отправляет его ответ. Любые рекомендации по обходу — вне scope и запрещены | [D]+[A] | grep по `hcaptcha/2captcha/anticaptcha/capsolver` = 0; AUTH-RE §8.3 | static |

---

## 9. 2FA (MFA)

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| MFA-01 | Вход в MFA: ответ `/auth/login` с `mfa` → `LOGIN_MFA_STEP`; UI рендерится в том же `AuthBox` (`transparent`, `style:{padding:0}`) компонентом `X` из MFA-чанка | [D] | `WEB @551777`, `@8979206`; `LOGIN_CHUNK @63828..@64000` | static |
| MFA-02 | Слайды (ровно в этом порядке): `select`, `webauthn`, `totp`, `sms`, `backup`, `backup_ack`, `password`; стартовый слайд = `methods[0]?.type`, иначе `select` | [D] | `MFA_CHUNK @7616` (`X` функция, конец чанка) | static |
| MFA-03 | Список методов строится фиксированно: `webauthn` (если есть challenge) → `totp` → `backup` → `sms`; `password` в список логина не попадает (слайд есть, но для login недостижим) → `[A]` | [D]+[A] | `WEB @8975978` (`function ea(){…}`) | static |
| MFA-04 | Заголовок: «Двухфакторная аутентификация» (`saHocI`), `heading-xl/semibold`; подзаголовок (если есть) — `heading-lg/semibold`; close-кнопка `top:12px; inset-inline-end:12px`; header padding `24px 24px 0` | [D] | `MFA_CHUNK @1653`, `@1384`; `CSS_MFA @0` | static |
| MFA-05 | Экран выбора: подзаголовок «Эта дополнительная мера подтверждает, что это правда вы.» (`XQaT+8`); список-кнопки: webauthn (`gTH4Dp`), totp (`nXKmyf`), sms (`ZbVwZW`), backup (`vhSRKf`), password (`8F6hKS` = «Использовать ваш пароль»); строка: `background:--background-base-low`, border `--border-subtle`, radius 8, `min-height` ≈ 50px, hover `--interactive-background-hover` | [D] | `MFA_CHUNK @4007`, `@186..@380`; `CSS_MFA @601`, `@1219` | static |
| MFA-06 | TOTP: поле «Введите код аутентификации Discord» (`HZPBOd`), placeholder «6-значный код подтверждения» (`tARzgo`), `autoComplete=one-time-code`, `maxLength=minLength=6`; кнопка «Подтвердить» (`cY+Oob`) disabled, пока длина ≠ 6 | [D] | `MFA_CHUNK @5887`, `@533925` (`XZ=6`) | static |
| MFA-07 | SMS: при открытии слайда `POST /auth/mfa/sms/send {ticket}` (`oldFormErrors:true, rejectWithError:false`), ответ `{phone}`; подзаголовок «Отправка...» (`LQdCQE`) до ответа, затем «Мы отправили сообщение на {phone}. Пожалуйста, введите полученный код.» (`8r6h7+`); маскирование номера клиент **не делает** — показывает `body.phone` как есть | [D] | `MFA_CHUNK @4492`, `@4743`; AUTH-RE A-07 | static |
| MFA-08 | SMS-поле: label `HZPBOd`, placeholder `tARzgo`, `maxLength=10`, `autoComplete=one-time-code`; кнопка подтверждения disabled, пока длина ≠ 6 (`$A=6`); рядом кнопка «Отправить SMS ещё раз?» (`ZF29L6`) с loading-состоянием, повторно шлёт тот же запрос | [D] | `MFA_CHUNK @4492`, `@533925` | static |
| MFA-09 | Backup: сначала предупреждение `backup_ack` — «Подождите! Вы уверены?» (`c5J7O0`), текст `$p$p` (`Nixbli`), countdown **10 c** («...кнопка внизу будет активирована через N сек.», `tsWkAE`; после нуля — `GK/Qfd`), кнопка «Да, использовать резервный код» (`oyjspn`) disabled все 10 c | [D] | `MFA_CHUNK @1809` | static |
| MFA-10 | Backup-поле: label «Введите резервный код Discord» (`C/ZAw/`), placeholder «8-значный резервный код» (`fZSi1D`), `minLength=8`, `maxLength=11`; перед отправкой дефисы удаляются (`h.replace(/-/g,"")`); кнопка disabled при длине < 8 | [D] | `MFA_CHUNK @2481`, `@533925` (`Zp=8`,`pu=11`) | static |
| MFA-11 | WebAuthn: challenge берётся из `methods.find(type==="webauthn").challenge`; кнопка «Аутентификация с помощью ключа доступа или ключа безопасности» (`Xr3Eks`); на embedded+поддержке — нативный `webAuthnAuthenticate`, иначе браузерный `W.J`; результат уходит как `data` с `mfaType:"webauthn"` | [D] | `MFA_CHUNK @6708`, `@7040`; AUTH-RE A-08 | static |
| MFA-12 | WebAuthn-ошибки: при 4xx — `signalUnknownCredential(credential)`, затем «Возникла ошибка. Попробуйте еще раз.» (`xSCvBf`); при прочих — message ошибки; аналитика `extra.error_type="webauthn_api_error"` | [D] | `MFA_CHUNK @6708` | static |
| MFA-13 | Password-метод: label «Пароль» (`CIGa+7`), `type=password`, `autoComplete=password`, `spellCheck=false`; кнопка disabled при пустом поле; отправляется как `data` | [D] | `MFA_CHUNK @3269` | static |
| MFA-14 | Футер: «Подтвердить» (`cY+Oob`, primary, `showConfirm`); если методов > 1 — кнопка «Верифицировать иным способом» (`Tot4EC`, secondary) → слайд `select`; в embedded-режиме close скрыт, снизу ссылка «Вернуться» (`1MrpWO`) | [D] | `MFA_CHUNK @898`, `@1384`; `CSS_MFA @163`, `@546` | static |
| MFA-15 | Отправка: `POST /auth/mfa/{mfaType}` тело `{code, ticket, login_source, gift_code_sku_id, login_instance_id}`, `retries:2`, `oldFormErrors:true`; `login_instance_id` из ответа логина или `getLoginInstanceId()` | [D] | `WEB @553107`; AUTH-RE A-05 | static |
| MFA-16 | Ошибки MFA: `60008 MFA_INVALID_CODE` → показывается `body.message` в поле ошибки; `suspended_user_token` → ACCOUNT_STANDING; прочие — `body.message ?? message`; локальный текст «Возникла ошибка. Попробуйте еще раз.» (`xSCvBf`) для webauthn | [D] | `WEB @553107`; `MFA_CHUNK @749` | static |
| MFA-17 | Отмена: закрытие в логине (`onEarlyClose`) → `LOGIN_RESET` (возврат к форме входа); при открытии как модалки — `onCloseCallback` бросает «Двухфакторная аутентификация отменена» (`N2yb9a`) | [D] | `LOGIN_CHUNK @63828`; `MFA_CHUNK @8944` (`$`) | static |
| MFA-18 | Контейнер модалки (step-up): `Modal size="sm"` → `max-width:400px`, `background:--background-surface-high`, border `--border-subtle`, radius 12, `shadow-high`; `aria-label` «Двухфакторная аутентификация»; `padding-size-sm` (top 8 / bottom 16) | [D]+[M] | `MFA_CHUNK @8817`; `CSS_MODAL @1420`, `@366`, `@723`; `TOKENS` | static+computed |
| MFA-19 | Отдельный step-up helper: `POST /mfa/finish {ticket,mfa_type,data}` → `{token}`; далее запрос повторяется с заголовком `X-Discord-MFA-Authorization: <token>`; коды `60008/60003` считаются «неверный код» (используется для действий, не для логина) | [D] | `MFA_CHUNK @533925` (`/mfa/finish`, `s0`) | static |

---

## 10. Чего нет в дампе (честные gap'ы)

| # | Пробел | Почему | Как закрывать |
|---|---|---|---|
| GAP-01 | Разметка/скриншот **окна входа** | В дампе нет ни DOM-снимка `/login`, ни скриншота login-экрана: `capture-*.jsonl` снимались уже авторизованными (0 совпадений `authBox`/`Welcome back`); `ui/*.png` — экраны приложения, не вход | CDP-прогон на чистом профиле с `DOM.getDocument`/`Page.captureScreenshot` (окно согласовать), либо рендер нашей реализации и сверка с §2–§7 |
| GAP-02 | Чанк модалки captcha (`536037` → модули `645320` и зависимости `202976/531279/458790/323895/454048/327774/203213`) | Lazy-чанки не попали в HTTP-кэш; в `decoded-index.json` их нет | отдельный захват страницы с искусственным 400+captcha_key (или запросить чанк в согласованном окне) |
| GAP-03 | Рендерер hCaptcha и turnstile | В бандле только enum-значения; фактический рендер — в GAP-02 | то же |
| GAP-04 | Точная ширина/паддинги модалки captcha и её тексты | см. GAP-02; i18n-ключи (`wjVOPz`, `3I8ubP`, `3413d0`, `7T4Zwx`, `gLz3BW`, `Idbp5O`, `aGa8n8`, `NplYbw`) в каталоге есть, но место использования не подтверждено | GAP-02 |
| GAP-05 | Внешний центр-контейнер логина (CSS-модуль `706192`) | Модуль не декодирован (lazy/чанк вне кэша); известно только, что это анимационная обёртка `iT` | догрузить чанк или измерить координаты коробки на живом экране |
| GAP-06 | Точные px для `heading-xl`/`text-md` в login-контексте | Взяты из общего typography-CSS (`058616e449204655_0`), не из login-чанка; совпадение ожидаемо, но не доказано для этой страницы | сверка computed styles на живом окне |
| GAP-07 | RU-текст для `N9U/9X` (password reset notice) | Ключа нет в `ru-catalog.json` (есть EN) | локализация на нашей стороне |
| GAP-08 | Серверные сроки жизни MFA-тикета/кода, маскирование телефона | Всё серверное; клиент показывает `body.phone` без обработки | наблюдение на тестовом аккаунте (без «пробива» лимитов) |
| GAP-09 | Точный вид ошибки «неверный пароль» (какой текст приходит с 50018) | В дампе нет живого ответа `/auth/login` (AUTH-RE N-01); UI просто рендерит `body.message`/field-errors | живой прогон логина (N6), либо наш собственный текст |

---

## 11. Границы

1. Всё выше — **описание оригинала**, не предписание копировать. Ассеты (SVG-логотип, PNG-оверлей QR)
   не копировались; указаны только пути и размеры.
2. Captcha — **только интерактив**: пользователь решает её сам. В бандле нет и не должно быть
   решателей, ферм, подбора, прокси-ротации или «анти-детекта» (AUTH-RE §8.3, §8.7).
3. 2FA-коды не автоматизируются и не подбираются; SMS-send вызывается только по действию пользователя.
4. QR/remote-auth — чужой флоу (передача токена с телефона); по уставу NET не реализуем.
5. Живых запросов, запуска Discord, `cargo`/`npm`, коммитов не было. Все md5 — в §1 и
   `PM/netr2/00-md5-inputs.log`.

---

## 12. Что это даёт для реализации (кратко, без обхода)

- Верстка: тёмная коробка 784×auto (логин) / 480 (регистрация, MFA), padding 32, radius 8,
  две колонки 416 + 240 с gap 64; логотип 130×36; шрифт gg sans.
- Поля: label 16/500, input 40px, radius 8, brand-фокус, error-палитра `#f87e7a`.
- Кнопки: primary `#5865f2` / hover `#4452bb`, 40px, radius 8; secondary — белый 12%.
- Ошибки: field-level (email/login, password) + спец-экраны (70007, 20011/20013, suspended).
- Captcha: наша реализация может либо не показывать её вовсе (мы не обязаны повторять серверный
  барьер), либо, если сервер её потребует, показать **человеку** виджет и повторить запрос с
  `X-Captcha-*`. Никаких обходов.
- 2FA: если решим поддерживать — 5 методов и точные поля описаны; но по AUTH-RE §9 MFA-флоу в
  нашем клиенте не планируется (достаточно «нужен вход в официальном клиенте»).
