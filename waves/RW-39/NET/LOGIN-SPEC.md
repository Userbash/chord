# LOGIN-SPEC — окно входа «как в оригинале» (RW-39 / NET)

Статус: дизайн-спецификация, вход для NET-D2 / NET-F1 / NET-E2 / QA-AUTH.
Дата: 2026-10-23. Автор: NET-DESIGN. PM: DeepSeek.
Контракт поведения: `NET/AUTH-CONTRACT.md` (заморожен). Значения: `DESIGN/tokens.css`.
Машиночитаемая версия: `NET/login-spec.json` (schema `rw39/login-spec-v1`).
Логи: `NET/PM/netdesign/` (`probe.txt`, `token-check.txt`, `md5.txt`).

## 0. Легенда меток и источники

| Метка | Значение | Источник |
|-|-|-|
| `[M]` | измерено на пикселях | `/home/sanya/re-deep/ui/00-app.png`, `ui8/*` (REFERENCES R-02, R-06) |
| `[T]` | значение токена | `waves/RW-39/DESIGN/tokens.css` + дамп `ui/tokens.json` (4795 переменных, dark) |
| `[D]` | прямо из дампа 1.0.162 | CSS/JS в `/home/sanya/re-deep`: `out/bundles/*.css`, `decoded/*` (модули и байтовые смещения — ниже) |
| `[A]` | наше решение / расчёт | обосновано в тексте; за измерение не выдаётся |

Правила: каждое число имеет метку и источник; размеры берутся из `tokens.css`, где токен есть;
где токена нет — значение помечено `[D]` и рядом указан ближайший токен, которым оно заменяется
(замена всегда явно названа). Ассеты Discord не копируются: иконки — inline-SVG, «логотип» на
узком экране — текстовая марка `DL`, QR рисуется локально (не картинка из дампа).

### 0.1 Что реально найдено в дампе (и где)

| Артефакт | Что даёт | Метка |
|-|-|-|
| `decoded/43644898c0897078_0` (`/login` HTML, 74 817 Б) | `GLOBAL_ENV` (BUILD_NUMBER 641004, REMOTE_AUTH_ENDPOINT), точка монтирования | `[D]` |
| `decoded/dbd05750a1cb3484_0` = `out/bundles/..._ae9016b5ac149349.js` (149 094 Б, chunk 786142) | вся логика окна: форма, поля, QR-панель, MFA, ошибки | `[D]` |
| `decoded/f0cac74b6ccdca5e_0` (22 033 Б) | модуль QR-входа `52423` (Remote Auth, состояния, тексты) | `[D]` |
| `decoded/8fed1c84bcaee32d_0` (9 759 Б) | модуль MFA `572469`: слайды, лимиты, SMS, backup | `[D]` |
| `out/bundles/discord.com_assets_348181.a3b3bc62ee2f1223.css.bin` | `authBox` 480 / `authBoxExpanded` 784, паддинг, радиус, тень, мобильный режим | `[D]` |
| `out/bundles/discord.com_assets_351200.ec1294199b2e7a31.css.bin` | `mainLoginContainer`, `header`, `needAccount`, `goBackButton`, `horizontalAuthBox` 960 | `[D]` |
| `out/bundles/discord.com_assets_461123.de02227c725f5838.css.bin` | `qrLogin` 240, `qrCodeContainer` 176×176, overlay 50, `authCaptcha` 480 | `[D]` |
| `out/bundles/discord.com_assets_715528.a04399038393a931.css.bin` | MFA-модалка: паддинги 24/12, список методов 50+8 | `[D]` |
| `out/bundles/discord.com_assets_964940.62a0e9d626b739d9.css.bin` | новый input: высота 40, радиус 8, border, focus/error | `[D]` |
| `out/bundles/discord.com_assets_419070.5a76dee70a3767eb.css.bin` | `custom-auth-box-auth-box-padding: 32px`, `appMount` фон | `[D]` |

**Чего в дампе нет (и это не выдумывается):**
1. `NET/LOGIN-UI-RE.md` отсутствует — работа велась по самому дампу; A-01…A-08 покрывают только сеть.
2. Нет ни одного PNG/JPEG именно окна входа: `ui/00-app.png` — экран `ready`. Поэтому геометрия
   окна — `[D]` из CSS (точные px), цвета — `[M]/[T]`, а «как выглядит вместе» — `[A]` (сборка).
3. `DiscordLogo` в дампе — картинка; мы её **не** берём (бренд `DL`), размер 36×112 сохранён `[D]`.
4. QR-иконка в центре кода — ассет; у нас заглушка-иконка inline-SVG 50×50 `[A]` при размере `[D]`.
5. Значения `secondary-*`/hover-состояния окна входа в статике не снимались — взяты токены.

**Ключевой вывод по «вкладкам вход/QR»:** вкладок в этой сборке **нет**. QR — это вторая колонка
той же карточки: `authBoxExpanded` 784 `[D]`, flex `direction=horizontal, gap=64` `[D]`
(`align-items=center`), QR-колонка 240 `[D]`. На ширине ≤830 px QR скрывается `[D]`
(`@media (max-width:830px){.qrLogin:not(.qrLoginHorizontal){display:none}}`), карточка
возвращается к 480 `[D]`. Поэтому в спеке нет переключателя вкладок; вместо него — панель
`#auth-qr`, которая существует/скрыта по ширине и по состоянию.


## 1. Окно и оверлей

### 1.1 `#auth-gate` (обязателен по контракту §4)

| Свойство | Значение | Метка | Источник |
|-|-|-|-|
| Позиция | `fixed; inset: 0` | `[A]` | AUTH-CONTRACT §4 (`role=dialog`, `aria-modal`) |
| Слой | `z-index: var(--z-modal)` (200) | `[T]` | tokens.css §12; выше `ready`-шелла, ниже tooltip/toast |
| Фон | `var(--bg-app)` (#121214), непрозрачный | `[M]+[T]` | рейл/дерево/titlebar `#121214` (R-02); `appMount` фон = background-base-lowest (419070) |
| Раскладка | `display:flex; align-items:center; justify-content:center` | `[A]` | оригинал центрирует карточку по окну; в дампе центрирование вынесено наружу `AuthBox` (модуль 854378) |
| Паддинг | `var(--space-8)` (24) | `[T]+[A]` | токен; при 820 px высоты даёт запас под тень |
| Скролл | `overflow:auto` | `[A]` | окно 1280×820 / 800×600 |
| Тень фона | нет | `[A]` | оригинал — страница, а не затемнение; dim-вариант не применяем |
| Скрытие | `[hidden]` + `aria-hidden=true` при `ready` | `[A]` | контракт: «скрыт при ready»; так же делает текущий `features/auth.js` |

Вариант «сессия истекла поверх живого шелла» (logout/4004) использует тот же гейт: фон
`rgb(0 0 0 / 72%)` `[A]` вместо непрозрачного, потому что под ним есть интерфейс. Это
единственное отличие; размеры карточки не меняются.

### 1.2 Карточка `#auth-box`

| Свойство | Значение | Метка | Источник |
|-|-|-|-|
| Ширина, форма | **480** | `[D]` | `authBox` `width:480px` (348181) |
| Ширина, форма + QR | **784** | `[D]` | `authBoxExpanded` `width:784px` (348181) |
| Разложение 784 | 32 + 416 + 64 + 240 + 32 | `[A]` | расчёт из `[D]`: паддинг 32×2, форма 416, gap 64, QR 240 |
| Разложение 480 | 32 + 416 + 32 | `[A]` | тот же паддинг, форма 416 |
| Паддинг | **32** = `var(--space-9)` | `[T]+[D]` | `custom-auth-box-auth-box-padding:32px` (419070); токен `--space-9` |
| Радиус | `var(--radius-sm)` = 8 | `[T]+[D]` | `border-radius:var(--radius-sm)` (348181) |
| Фон | `var(--surface-2)` = #242429 | `[M]+[T]` | `modal-background` = #242429 (дамп tokens.json), совпадает с `--surface-2`; hover-карточка #242429 (R-02) |
| Тень | `var(--shadow-elevation-high)` | `[T]+[A]` | дамп: `legacy-elevation-high = 0 2px 10px 0 rgba(0,0,0,.2)` `[D]`; в токенах ближайшая — elevation-high (8/16/24%) |
| Цвет текста | `var(--text-muted)` (#96979e) | `[T]+[D]` | `color:var(--text-muted)` (348181); контраст 4.7–6.4 (tokens.css §2) |
| Базовый кегль | 18 | `[D]` | `font-size:18px` (348181); все реальные тексты задаются ниже токенами |
| Высота | авто; расчёт ≈ **362** | `[A]` | 32+30+8+22+20+(40+20)+40+(8+18+20)+(38+8)+(8+18)+32 по метрикам §2–§4 |
| `overflow` | `visible` (десктоп); `auto` при `max-height` | `[A]` | чтобы тень не резалась |
| `max-height` | `calc(100% - var(--space-11))` (48) | `[T]+[A]` | запас 24+24 при 820 px |

Внутри карточки: `[logo (скрыт)] → header → form (или MFA) → QR-колонка`. Логотип-блок
`discordLogo` в дампе `display:none` на десктопе и `36×112` на узком `[D]`; у нас это текстовая
марка `DL` 36×112 `[A]` (без чужих логотипов).

### 1.3 Раскладка

```
#auth-gate (bg-app, центр)
└─ #auth-box (480 | 784, radius 8, padding 32)
   ├─ .auth-logo        скрыт ≥485 px; 36×112 <485 px
   ├─ .auth-body        flex, direction=horizontal, gap=var(--space-13)=64, align=center
   │  ├─ #auth-form-col  416 (форма / MFA / captcha)
   │  └─ #auth-qr        240 (скрыт <830 px)
   └─ .auth-foot        (нет в оригинале; резерв под юр. текст — не рисуем)
```

MFA-состояние: карточка остаётся 480 (QR-колонка скрывается вместе с формой) `[A]`, потому что
MFA в дампе — отдельная модалка 442 `[D]`; чтобы не прыгала ширина, при `mfa` карточка = 480 и
внутри — MFA-панель `[A]`. На ≤485 px — как в §11.


## 2. Header карточки

| Элемент | Значение | Метка | Источник |
|-|-|-|-|
| `.auth-header` | `display:flex; flex-direction:column; width:100%` | `[D]` | `header__1d5fc` (351200) |
| Заголовок `#auth-title` | 24 / 30, semibold, `var(--text-strong)` | `[T]+[D]` | `heading-xl/semibold` (24/30, 600) в дампе; токены `--font-size-display`, `--line-height-display`, `--font-weight-semibold` |
| Отступ заголовок→подзаголовок | 8 = `var(--space-4)` | `[T]+[D]` | `marginBottom8` (`QB`, 351200) |
| Подзаголовок `.auth-subtitle` | 16 / 22, normal, `var(--text)` | `[T]+[D]` | `text-md/normal` 16/20 `[D]`; line-height берём 22 из `--line-height-body` |
| Отступ header→форма | 20 = `var(--space-7)` | `[T]+[D]` | `marginTop20` (`QX`) на блоке формы |
| Текст заголовка | «С возвращением!» | `[A]` | дамп `7fNJgA` «Welcome back!» — перевод/бренд `[D]` |
| Текст подзаголовка | «Мы рады снова вас видеть.» | `[A]` | дамп `euS7r4` |
| Подзаголовок после сброса пароля | «Пароль изменён. Войдите с новым паролем.» | `[A]` | дамп `N9U/9X` (notice) |

`id` заголовка обязателен: `#auth-gate aria-labelledby="auth-title"` (§10).


## 3. Форма входа

### 3.1 Поля

| Элемент | Значение | Метка | Источник |
|-|-|-|-|
| `#auth-form` | `display:flex; flex-direction:column` | `[A]` | контракт §4; в дампе поля идут блоком `block` 100% |
| Высота поля | **40** = `var(--input-height)` | `[T]+[D]` | `control-input-height-md:40px` (tokens.json), `min-height:40px` (348181/964940) |
| Радиус поля | `var(--radius-sm)` = 8 | `[T]+[D]` | `custom-input-border-radius = var(--radius-sm)` (964940) |
| Фон поля | `var(--surface-input)` = #1e1f22 | `[T]+[A]` | дамп: `input-background-default` = чёрный 12% поверх #242429 → ≈#1e1f22; в tokens.css это `--surface-input` |
| Граница | 1 px `var(--border-subtle)` | `[T]+[A]` | дамп: `input-border-default` = белый 8% `[D]`; ближайший токен — subtle 12% |
| Граница hover | `var(--border)` | `[T]+[A]` | `input-border-hover` `[D]` |
| Фокус (мышь) | граница `var(--accent)`, без свечения | `[T]+[D]` | `input-border-active` = brand-500 #5865f2 (tokens.json) |
| Фокус (клавиатура) | `outline: var(--focus-ring-width) solid var(--focus-ring); outline-offset: var(--focus-ring-offset)` | `[T]+[A]` | tokens.css §4; в дампе фокус рисуется `outline:2px inset` тем же brand — для AA берём `--focus-ring` #a5b0ff (6.71–9.14) |
| Текст | 16 / 22, `var(--text)` | `[T]+[A]` | `input-text-default` = #efeff1 = `--text`; кегль 16 у поля |
| Placeholder | `var(--text-muted)` (#96979e) | `[T]+[D]` | `input-placeholder-text-default` ≈ #7f8288…#96979e; AA на #1e1f22 ≥4.5 |
| Лейбл | 12 / 16, medium, `var(--text-subtle)` (#abacb2) | `[T]+[A]` | `text-xs/medium` 12/16 `[D]`; цвет — `interactive-text-default` #abacb2 = `--text-subtle` |
| Отступ лейбл→поле | 4 = `var(--space-2)` | `[T]+[A]` | `container` — flex column, gap 4 у нового input `[D]` |
| Отступ поле→поле | 20 = `var(--space-7)` | `[T]+[D]` | `marginBottom20` (`SX`) на поле логина (351200) |
| Ошибка поля | граница 1 px `var(--danger)`, текст `var(--danger-text)` 12/16 | `[T]+[D]` | `data-error` → `input-border-error-default` #f87e7a `[D]`; `--danger-text` — AA-вариант `[T]` |
| `aria-invalid` | `true` при ошибке поля | `[A]` | a11y §10 |
| Обязательность | `required`, звёздочка `*` `aria-hidden=true` | `[A]` | в дампе поля `required:!0` `[D]` |

Контрактные id: `#auth-login` (`autocomplete="username"`), `#auth-password` (`type=password`),
`#auth-token` (`type=password`). Дамп uses `autocomplete="username webauthn"` `[D]` и
`current-password` `[D]` — берём совместимое `username` + отдельный webauthn-путь в QR-панели,
`current-password` для пароля (контракт не запрещает).

### 3.2 Логин (`#auth-login`)

| Свойство | Значение | Метка |
|-|-|-|
| Тип | `text`, `inputmode="email"`, `autocapitalize="none"`, `autocorrect="off"`, `spellcheck="false"` | `[D]` |
| `autocomplete` | `username` | `[D]+[T]` |
| Префикс страны | кнопка-селектор 40×40 (иконка 20) слева внутри поля, `aria-label` «Код страны» | `[A]` |
| Текст лейбла | «Email или телефон» | `[A]` (дамп `tUjnxr`) |
| Placeholder | нет | `[D]` (в дампе плейсхолдера нет) |
| Ошибка | «Введите email или телефон.» / серверная под полем | `[A]` |
| `maxlength` | нет | `[D]` |

Фокус по умолчанию — здесь, **кроме** случая «ошибка только по паролю»: тогда фокус в
`#auth-password` `[D]` (в коде `autoFocus:!k` для логина и `k` для пароля, где `k` = ошибка
пароля без ошибки логина).

### 3.3 Пароль (`#auth-password`)

| Свойство | Значение | Метка |
|-|-|-|
| Тип | `password` → `text` по кнопке | `[T]` (контракт §4) |
| `autocomplete` | `current-password` | `[D]` |
| `spellcheck` | `false` | `[D]` |
| Кнопка показа | 40×40, иконка-глаз 20, `aria-pressed`, `aria-label` «Показать пароль» / «Скрыть пароль», `aria-controls="auth-password"` | `[A]` |
| Очистка | после submit значение стирается до ответа ядра; в DOM/логах не остаётся | `[T]` (контракт §5.1) |
| Ошибка | «Неверный логин или пароль.» под полем (401) | `[A]` |
| Ссылка «Забыли пароль?» | 14 / 18, `var(--text-link)` #4d96ee, `margin-top: var(--space-4)`, `margin-bottom: var(--space-7)` | `[T]+[D]` |

Пароль **никогда** не выводится в тексте спеки, в `title`, в `aria-label`, в логах и в
`#auth-error` (контракт §3/§5).

### 3.4 Кнопки

| Элемент | Значение | Метка | Источник |
|-|-|-|-|
| «Войти» `#auth-submit` | full width, высота **38** = `var(--btn-height-lg)`, radius `var(--btn-radius)` 8 | `[T]+[D]` | `md` button `min-height:38px` (348181); токены |
| Фон | `var(--accent)` #5865f2; hover `var(--accent-hover)`; active `var(--accent-active)` | `[T]+[D]` | `control-primary-background-default` = brand-500 |
| Текст | 16 / 24, medium, `var(--text-on-accent)` | `[T]+[D]` | `button__921c5{font-size:16px;line-height:24px}`; `--font-size-body` |
| Паддинг | 0 15 | `[D]+[A]` | `16`; у нас `var(--btn-pad-x)` 16 |
| Loading | спиннер 20 по центру, текст скрыт (`opacity:0`), кнопка `disabled`, `aria-busy=true` | `[D]+[A]` | `.loading` + spinner 20 (348181) |
| Disabled | `opacity: var(--state-disabled-opacity)` 0.45, `cursor:not-allowed` | `[T]+[A]` | токены §4 |
| «Забыли пароль?» | текстовая кнопка 14/18 `var(--text-link)`, без фона | `[T]+[D]` | `linkButton` padding-inline 0 |
| «Зарегистрироваться» | текстовая кнопка 14/16 `var(--text-link)`, inline после «Нет аккаунта?» | `[T]+[D]` | `needAccount` 14/16 + `smallRegisterLink` margin-inline-start 4 |
| «Нет аккаунта?» | 14/16 `var(--text-muted)` | `[T]+[D]` | `needAccount` |
| «Восстановить сессию» | secondary 14/16, высота 32, видна только при `stored` | `[T]+[A]` | текущий `auth.js` + контракт `auth_restore` |
| «Назад» (мультиаккаунт) | текстовая 14, иконка 16, `margin-bottom:16` | `[D]` | `goBackButton__1d5fc` |

Порядок в DOM: логин → пароль → «Забыли пароль?» → «Войти» → «Нет аккаунта? Зарегистрироваться»
`[D]` (порядок из `lf` в chunk 786142).

### 3.5 Альтернативный вход по токену

Блок `<details id="auth-token-block">` под основным submit (не мешает оригинальному сценарию,
нужен по ADR-2): summary «Войти по токену», 14/18 `var(--text-link)`; при раскрытии —
label «Токен пользователя», поле `#auth-token` (`type=password`, `autocomplete=off`,
`spellcheck=false`, моноширинный `var(--font-mono)` 14/18), кнопка «Войти по токену» secondary,
подпись 12/16 `var(--text-muted)` «Токен уходит сразу в системное хранилище и не попадает в логи».
Токен не отображается нигде, кроме самого поля, и стирается после submit `[T]` (контракт §5).


## 4. QR-панель `#auth-qr`

| Свойство | Значение | Метка | Источник |
|-|-|-|-|
| Колонка | ширина **240**, `margin-inline-start:64` (gap) | `[D]` | `qrLogin` width 240; flex gap 64 (chunk 786142) |
| Заголовок | 16 / 20 semibold, `var(--text)` | `[T]+[D]` | horizontal-вариант: `heading-md/semibold` 16/20, `text-default` |
| Текст | 14 / 18, `var(--text-subtle)` | `[T]+[D]` | `text-sm/normal`, `descriptionColor:text-subtle` |
| QR-контейнер | **176×176**, padding 8, radius `var(--radius-xs)` 4, `margin-bottom:32` = `--space-9` | `[T]+[D]` | `qrCodeContainer` 176×176; padding `space-8`; `margin-bottom:32` |
| QR-код | **160×160**, `image-rendering:pixelated` | `[D]` | `size:160` в модуле 52423 |
| Фон QR | `var(--text-strong)` #fbfbfb; модули — `var(--bg-app)` #121214 | `[T]+[A]` | нужен светлый quiet zone; оба — токены, контраст ≈16:1 |
| Иконка в центре | **50×50**, `aria-hidden` | `[D]+[A]` | `qrCodeOverlay img` 50×50; своя inline-SVG |
| Ссылка «Или войдите с ключом доступа» | 12 / 16 medium, `var(--text-link)`, центр | `[T]+[D]` | `text-xs/medium`, кнопка passkey |
| `role` QR | `img`, `aria-label` «QR-код для входа в мобильном приложении», `tabIndex=0` | `[D]+[T]` | в дампе `role=img`, `aria-label` `SzYj9v` |
| Loading | `aria-busy=true`, `aria-label` «QR-код загружается», спиннер-кубики | `[D]+[T]` | модуль 52423 |
| Готовность | announce «QR-код готов» (`aria-live=polite`) | `[D]+[A]` | `j2p125` |
| После сканирования | аватар 120, заголовок 24/30 «Проверьте телефон», текст 16/22 «Вход как {username}», ссылка 14/18 «Это не я — начать заново» | `[T]+[D]` | `PENDING_TICKET`: avatar `SIZE_120`, heading-xl, text-md, cancel `nOOhwo` |
| Скрытие | `display:none` при ширине ≤830 px и во всех состояниях кроме `no-session` | `[D]+[A]` | media 830 |
| Переподключение | backoff 1.5 с → 30 с, без «бесконечного цикла» | `[D]` | `new Backoff(1500, 30000)` в модуле 52423 |

Тексты: заголовок «Вход по QR-коду» `[A]` (`UPiHaL`), тело «Отсканируйте код в **мобильном
приложении**, чтобы войти.» `[A]` (`Qq+A6i`), passkey «Или войдите с ключом доступа» `[A]`,
«Проверьте телефон» `[A]` (`apGCUT`), «Вход как {username}» `[A]` (`Cbl5JK`), «Это не я —
начать заново» `[A]` (`nOOhwo`).

QR-протокол — чужой флоу (ADR-2 §8, AUTH-RE §8.8). В спеке он описан как **экран**; включать его
в продукт можно только по отдельному решению владельца. Если выключен — панель не рендерится,
карточка 480 (это же состояние при ширине ≤830) `[A]`.


## 5. Captcha-зона `#auth-captcha`

Появляется **только** при `LoginOutcome.kind === "captcha"` (AUTH-CONTRACT §2), никогда
упреждающе. Капча — штатный серверный барьер: показываем официальный виджет, **решает человек**;
никаких solver-сервисов, автоматизации, подмены ответа, обхода или «пробива» — это P0
(ADR-3, AUTH-RE §8.3).

| Свойство | Значение | Метка | Источник |
|-|-|-|-|
| Позиция | full-bleed блок вверху карточки, над header | `[A]` | в дампе `authCaptcha` — отдельный блок 480 с верхним радиусом |
| Ширина | 100% карточки (480 / 784) | `[D]+[A]` | `authCaptcha width:480px` (461123) при карточке 480 |
| Высота | `min-height:78` | `[A]` | типовой размер виджета hCaptcha 303×78; контейнер не схлопывается до загрузки |
| Паддинг | `var(--space-8) var(--space-6)` (24 / 16) | `[T]+[D]` | `padding:24px 16px` |
| Фон | `var(--surface-4)` #2c2d32 | `[M]+[T]` | `primary-630` = #2c2d32 (tokens.json); попаут профиля #2c2d32 (R-06) |
| Радиус | сверху `var(--radius-sm)` 8, снизу 0 | `[T]+[A]` | дамп: `border-radius:5px 5px 0 0` `[D]`; 5 px токена нет → 8 |
| Заголовок | 16/20 semibold, `var(--text-strong)`, «Проверка безопасности» | `[T]+[A]` | — |
| Подпись | 12/16, `var(--text-muted)`, «Капчу решает пользователь. Автоматическое решение и обход запрещены.» | `[T]+[A]` | правило P0 |
| Placeholder до загрузки | рамка 1 px `var(--border-subtle)`, radius `var(--radius-sm)`, фон `var(--surface-3)`, спиннер 20, текст «Загружаем проверку…» | `[T]+[A]` | — |
| Виджет | iframe официального сервиса; наши стили внутрь не проникают | `[A]` | контракт §5.3 |
| `role` контейнера | `group`, `aria-labelledby` на заголовок | `[A]` | a11y |
| Кнопка «Продолжить» | secondary 38, активна после получения ключа от виджета | `[A]` | контракт: `captchaKey` / `captchaRqtoken` |
| Ошибка | `#auth-error`: «Проверка не пройдена — попробуйте ещё раз.» | `[A]` | дамп `HQdHg6` / `3s/vDN` |
| Повтор | новый виджет; счётчик попыток не «крутится» сам | `[A]` | ADR-3 (без циклов) |

Разметка: `#auth-captcha` — пустой контейнер; виджет монтируется в него ядром/вебвью.
`aria-describedby` у контейнера ссылается на подпись. Пока капча не решена, поля формы
`disabled` (state `captcha`), submit не активен `[A]`.


## 6. MFA-экран `#auth-mfa`

Источник: модуль `572469` (chunk 290966) `[D]`, CSS `715528` `[D]`, тексты — en-catalog `[D]`.
В контракте это состояние `mfa`; в дампе — модалка `size:"sm"` и набор слайдов.

### 6.1 Корпус

| Свойство | Значение | Метка | Источник |
|-|-|-|-|
| Ширина | **442** = `var(--modal-w-sm)` | `[T]+[D]` | `size:"sm"` → modal-width-small 442 |
| Фон / радиус | `var(--surface-2)`, `var(--radius-md)` 12 | `[T]+[A]` | модалка; в authBox — 8; внутри карточки наследуем 8 |
| Header | padding `24 24 0` (24 = `--space-8`) | `[T]+[D]` | `header__7c901` |
| Заголовок | 24/30 semibold, `var(--text-strong)`, «Двухфакторная аутентификация» | `[T]+[D]` | `heading-xl/semibold`; дамп `saHocI` |
| Подзаголовок | 20/24 semibold, `var(--text)`, «Этот шаг подтверждает, что это действительно вы.» | `[T]+[D]` | `heading-lg/semibold`; дамп `XQaT+8` |
| Закрытие | кнопка 32×32, `inset-inline-end:12`, `top:12` | `[D]+[A]` | `closeButton__7c901` 12/12; размер 32 — `--btn-height` |
| Content | padding 24 | `[D]` | `content__7c901` |
| Footer | padding-inline 24, padding-bottom 24 | `[D]` | `footer__7c901` |
| Ошибка | 14/18 `var(--danger-text)`, `margin-top:8` | `[T]+[D]` | `error__7c901` |
| Переход слайдов | 180 мс `var(--motion-base)` | `[T]+[A]` | токены; в дампе slide-animator |

### 6.2 Список методов (слайд `select`)

| Свойство | Значение | Метка | Источник |
|-|-|-|-|
| Пункт | высота ≥50, radius 8, border 1 px `var(--border-subtle)`, фон `var(--surface-1)`, `margin-bottom:8`, padding-inline-start 8 | `[T]+[D]` | `listItemContainer__7c901` (50 — высота стрелки, padding 0) |
| Hover / focus | `var(--state-hover)` / `var(--state-selected)` | `[T]+[A]` | токены |
| Текст | 16/20 semibold, `var(--text-strong)` | `[T]+[D]` | `text-md/semibold` |
| Стрелка | 20×20, `var(--text-subtle)` | `[T]+[D]` | icon 20, `interactive-text-default` |
| Порядок | как пришёл `methods[]` от сервера (не сортируем) | `[D]` | `t.methods.map` |
| Методы | `webauthn` «Ключ доступа или ключ безопасности»; `totp` «Приложение-аутентификатор»; `sms` «Код в SMS»; `password` «Пароль аккаунта»; `backup` «Резервный код» | `[A]` | ключи `gTH4Dp/nXKmyf/ZbVwZW/8F6hKS/vhSRKf` |
| Если метод один | слайд открывается сразу, без списка | `[D]` | `methods[0]?.type` |
| «Выбрать другой способ» | secondary, full width, пока методов >1 | `[T]+[D]` | `Tot4EC` «Verify with something else» |

### 6.3 Слайды

| Слайд | Поля и лимиты | Кнопки | Метка | Источник |
|-|-|-|-|-|
| `totp` | label «Код из приложения», placeholder «6 цифр», `maxlength=6`, `minlength=6`, `inputmode=numeric`, `autocomplete=one-time-code`, `spellcheck=false`; автофокус | «Подтвердить» (disabled, пока длина ≠ 6) | `[D]+[A]` | `HZPBOd`, `tARzgo`, `XZ=6` |
| `sms` | при открытии слайда сразу `POST /auth/mfa/sms/send {ticket}`; подзаголовок «Мы отправили сообщение на {phone}.»; label «Код из SMS», `maxlength=10`, `autocomplete=one-time-code`; «Отправить ещё раз» (secondary, loading) | «Подтвердить» (disabled, пока длина ≠ 6) | `[D]+[A]` | `8r6h7+`, `ZF29L6`, `$A=6` |
| `backup` | label «Резервный код», placeholder «8 символов», `maxlength=11`, `minlength=8` (дефисы допускаются, при отправке вырезаются) | «Подтвердить» (disabled, пока <8) | `[D]+[A]` | `C/ZAw/`, `fZSi1D`, `Zp=8`, `pu=11` |
| `backup_ack` | текст-предупреждение «Резервный код одноразовый…» + «Кнопка станет доступна через {n} с.»; счётчик 10 с | «Да, использовать резервный код» disabled, пока счётчик >0 | `[D]+[A]` | `Nixbli`, `tsWkAE`, `GK/Qfd`, `oyjspn` |
| `password` | label «Пароль», `type=password`, `autocomplete=password`, `spellcheck=false`; автофокус | «Подтвердить» (disabled при пустом) | `[D]+[A]` | `CIGa+7` |
| `webauthn` | без поля; кнопка «Подтвердить ключом доступа» (primary, loading); при отказе — «Не удалось. Попробуйте ещё раз.» | — | `[D]+[A]` | `Xr3Eks`, `xSCvBf` |

Таймер/повтор: у TOTP и SMS обратного отсчёта в дампе нет `[D]`; повтор — только кнопкой.
При 429 от `/auth/mfa/*` включается `retryAfterMs` из контракта: кнопка «Отправить ещё раз» и
«Подтвердить» disabled, рядом «Повторите через {N} с.» `[A]`. `backup_ack` — не rate limit, а
страховка от случайного клика (10 с) `[D]`.

SMS-номер показывается **только маскированным**, как пришёл от сервера `[D]`; код и пароль
никогда не показываются значениями и не попадают в DOM после отправки `[T]` (контракт §5).


## 7. Ошибки

Один alert-регион `#auth-error` (`role=alert`, `aria-live=assertive`), позиция — над кнопкой
«Войти», внутри `#auth-form-col`; плюс локальные ошибки полей под полем (`aria-describedby`).
Текст никогда не содержит токен, пароль, код, тело ответа `[T]`.

| Код (контракт §3) | HTTP/случай | Текст (ru) | Позиция | Метка | Источник текста |
|-|-|-|-|-|-|
| `InvalidCredentials` | 401, 40001, 50018, 50022 | «Неверный логин или пароль.» | под паролем + alert | `[A]` | дамп `ayvCBh` |
| `RateLimited` | 429, 503 | «Слишком много попыток. Повторите через {N} с.» | alert; кнопка disabled на N | `[A]` | дамп `zBB9xD` |
| `CaptchaRequired` | 400 + `captcha_key` | «Нужна проверка безопасности.» → показ `#auth-captcha` | alert + блок капчи | `[A]` | дамп `3I8ubP` |
| captcha failed | ответ виджета/сервера | «Проверка не пройдена — попробуйте ещё раз.» | alert, блок капчи остаётся | `[A]` | дамп `HQdHg6` |
| `MfaRequired` | 200 + `mfa` | переключение на `#auth-mfa` (не ошибка) | — | `[T]` | контракт §1 |
| `MfaInvalidCode` | 60008 | «Неверный код.» | alert в MFA | `[A]` | — |
| `MfaInvalidTicket`/`MfaInvalidSession` | 60006, 60009 | «Сессия входа истекла. Войдите снова.» | alert, возврат в `no-session` | `[A]` | — |
| `Network` | таймаут, обрыв | «Нет связи с сервером. Проверьте подключение и повторите.» | alert; кнопка «Повторить» | `[A]` | текущий `auth.js` (`offline`) |
| `Keyring` | нет libsecret | «Не удалось сохранить сессию в системном хранилище.» | alert | `[A]` | текущий `auth.js` (`keyring`) |
| `Parse` | битый JSON | «Неожиданный ответ сервера. Повторите вход.» | alert | `[A]` | текущий `auth.js` (`parse`) |
| `Internal` | 5xx/прочее | «Внутренняя ошибка. Повторите позже.» | alert | `[A]` | текущий `auth.js` (`internal`) |
| 70007 / 40002 | нужен шаг в официальном клиенте | «Требуется подтверждение в официальном клиенте.» | alert | `[A]` | AUTH-RE §9 |
| 20011 / 20013, `suspended_user_token` | аккаунт | «Аккаунт отключён или запланирован к удалению.» | отдельный экран `error`, без retry-цикла | `[A]` | дамп `E-07/E-10`, экраны `i1` |

Правила: 429 не ретраится автоматически (ADR-3, AUTH-RE E-02); 401 не маскируется под сеть;
`retryAfterMs` из контракта — единственный источник числа в «через {N} с.». Ошибка поля и
alert не дублируются: 401 показывается под паролем **и** в alert только при отсутствии поля
(например, пришёл только общий код) `[A]`.


## 8. Состояния (полностью по AUTH-CONTRACT §1)

| Состояние | Что видно | Поля | Фокус | `aria-busy` | Переходы |
|-|-|-|-|-|-|
| `checking` | сплэш: карточка 480 без формы, спиннер 24 (`var(--icon-size-lg)`), текст «Проверяем сохранённую сессию…» 14/18 `var(--text-muted)` | — | нет (фокус на гейте) | true | → `no-session` / `ready` / `error` |
| `no-session` | header + форма + QR (≥831 px) + token-block + «Восстановить сессию» (если `stored`) | активны | `#auth-login` (или `#auth-password` при ошибке пароля) | false | → `validating` |
| `validating` | форма как есть, поля `disabled`, submit — спиннер; подпись «Проверяем…» в status | disabled, `opacity` 0.45 | остаётся на submit | true | → `captcha` / `mfa` / `ready` / `error` |
| `captcha` | форма disabled + `#auth-captcha` сверху | disabled | в виджет (best-effort), «Продолжить» | false | → `validating` после решения |
| `mfa` | вместо формы — `#auth-mfa` (список методов или слайд), QR скрыт | активны поля MFA | первый метод / поле кода | false | → `validating` / `error` |
| `ready` | гейт скрыт (`hidden` + `aria-hidden`), user-bar: имя + «Выйти» `[data-action=auth-logout]` | — | в шелл | false | → `no-session` (logout) |
| `error` | форма + alert, кнопка «Повторить» (кроме 20011/20013) | активны (кроме 429-каунтдауна) | первое проблемное поле / alert | false | → `no-session` |

Подсостояния `no-session`: `token` (блок раскрыт), `restore` (кнопка видна при `stored`),
`qr-scanned` (в QR-панели: «Проверьте телефон»). Подсостояния `mfa`: `select`, `totp`, `sms`,
`backup`, `backup_ack`, `password`, `webauthn` (§6.3). Подсостояния `error`: `401`, `429`,
`captcha`, `network`, `mfa-code`, `account` (§7).

Переходы инициируются только контрактными командами (`auth_login_password`, `auth_mfa`,
`auth_sms`, `auth_restore`, `auth_logout`); UI не переходит в `mfa`/`captcha` по таймеру.


## 9. Тексты (реестр)

Все строки — русские, без бренда Discord в названиях продукта (бренд `DL`), кроме технических
«ключ доступа», «резервный код». Источник смысла — en-catalog дампа `[D]`, формулировка — `[A]`.

| Ключ | Текст |
|-|-|
| `login.title` | С возвращением! |
| `login.subtitle` | Мы рады снова вас видеть. |
| `login.label` | Email или телефон |
| `login.password` | Пароль |
| `login.forgot` | Забыли пароль? |
| `login.submit` | Войти |
| `login.needAccount` | Нет аккаунта? |
| `login.register` | Зарегистрироваться |
| `login.back` | Назад |
| `login.restore` | Восстановить сессию |
| `login.tokenSummary` | Войти по токену |
| `login.tokenLabel` | Токен пользователя |
| `login.tokenNote` | Токен уходит сразу в системное хранилище и не попадает в логи. |
| `qr.title` | Вход по QR-коду |
| `qr.body` | Отсканируйте код в мобильном приложении, чтобы войти. |
| `qr.passkey` | Или войдите с ключом доступа |
| `qr.aria` | QR-код для входа в мобильном приложении |
| `qr.loading` | QR-код загружается |
| `qr.ready` | QR-код готов |
| `qr.scannedTitle` | Проверьте телефон |
| `qr.scannedBody` | Вход как {username} |
| `qr.cancel` | Это не я — начать заново |
| `captcha.title` | Проверка безопасности |
| `captcha.note` | Капчу решает пользователь. Автоматическое решение и обход запрещены. |
| `captcha.loading` | Загружаем проверку… |
| `captcha.continue` | Продолжить |
| `mfa.title` | Двухфакторная аутентификация |
| `mfa.subtitle` | Этот шаг подтверждает, что это действительно вы. |
| `mfa.other` | Выбрать другой способ |
| `mfa.confirm` | Подтвердить |
| `mfa.close` | Отмена |
| `mfa.totp.label` | Код из приложения |
| `mfa.totp.placeholder` | 6 цифр |
| `mfa.sms.sent` | Мы отправили сообщение на {phone}. |
| `mfa.sms.label` | Код из SMS |
| `mfa.sms.resend` | Отправить ещё раз |
| `mfa.backup.label` | Резервный код |
| `mfa.backup.placeholder` | 8 символов |
| `mfa.backupAck.text` | Резервный код одноразовый и нужен для аварийного доступа. |
| `mfa.backupAck.countdown` | Кнопка станет доступна через {n} с. |
| `mfa.backupAck.confirm` | Да, использовать резервный код |
| `mfa.password.label` | Пароль |
| `mfa.webauthn.button` | Подтвердить ключом доступа |
| `mfa.webauthn.error` | Не удалось подтвердить ключ. Попробуйте ещё раз. |
| `status.checking` | Проверяем сохранённую сессию… |
| `status.validating` | Проверяем… |
| `status.rateLimited` | Повторите через {n} с. |
| `status.retry` | Повторить |


## 10. Доступность (a11y)

1. **Диалог.** `#auth-gate`: `role=dialog`, `aria-modal=true`, `aria-labelledby="auth-title"`
   (текст заголовка), без `aria-label`-дубля. Пока гейт открыт, шелл — `aria-hidden=true`;
   при `ready` — наоборот. Ловушка фокуса: Tab/Shift+Tab циклично внутри карточки; клик вне
   карточки не закрывает (вход обязателен) `[A]`.
2. **Порядок табуляции** (`no-session`): `#auth-login` → страна → `#auth-password` → показать
   пароль → «Забыли пароль?» → `#auth-submit` → «Зарегистрироваться» → «Войти по токену» →
   `#auth-token` → QR (`role=img`, `tabIndex=0`) → «ключ доступа» → «Восстановить сессию».
   `[A]` (порядок DOM из дампа `[D]`).
3. **Порядок в MFA**: закрыть → пункты методов → «Выбрать другой способ» → поле → «Подтвердить».
   Смена слайда переводит фокус в первое поле и объявляет заголовок `[A]`.
4. **Метки.** Каждое поле — `<label for>`; звёздочка `*` — `aria-hidden` (обязательность уже в
   `required`); описание/ошибка — `aria-describedby`. Никаких placeholder-вместо-метки `[A]`.
5. **Live-регионы.** `#auth-error` — `role=alert`, `aria-live=assertive` (только сбои);
   `#auth-status` — `aria-live=polite` (проверка, таймер, «QR-код готов»); QR — `aria-busy`.
   Таймер 429 объявляется не чаще раза в секунду `[A]`.
6. **Пароль/код.** Переключатель показа — `button type=button`, `aria-pressed`, `aria-controls`,
   меняющийся `aria-label`. Для MFA-кода переключателя нет. Значения не попадают в
   `aria-label`/`title`/логи `[T]`.
7. **Капча.** Контейнер `role=group` + `aria-labelledby`; до загрузки — `aria-busy=true`;
   виджет сохраняет собственную доступность; альтернативы «решить за пользователя» нет `[A]`.
8. **Контраст (AA 4.5:1).** `--text` на `--surface-2` 11.9–16.3; `--text-muted` 4.7–6.4;
   `--text-subtle` 6.1–8.3; `--text-link` 4.5–6.2; `--danger-text` ≥4.5 на `--surface-2`;
   `--text-on-accent` 4.61 на `--accent`; `--focus-ring` 6.71–9.14 (non-text ≥3) `[T]`.
   QR: `--bg-app` на `--text-strong` ≈16:1 `[A]`.
9. **Фокус.** Видим всегда: 2 px `--focus-ring`, offset 2 `[T]`. `outline` не убирается.
   При открытии — программный фокус (после анимации 0 мс при reduced motion) `[A]`.
10. **Клавиатура.** Enter в поле — submit; Esc в MFA — назад к списку методов (не закрывает
    вход); Esc в капче не закрывает виджет; пробел/Enter на пункте метода — выбор `[A]`.
11. **Движение.** `prefers-reduced-motion` → длительности 0 (токены §12), геометрия та же.
12. **Зум.** 200% без потери контента: карточка получает `max-height` и внутренний скролл
    (§11), горизонтальный скролл не появляется `[A]`.


## 11. Адаптив

| Условие | Поведение | Метка | Источник |
|-|-|-|-|
| **1280×820** (окно спайка) | карточка 784×≈362 по центру; QR виден (1280 > 830); поля 416; отступ окна 24; тень целиком в кадре | `[A]` | расчёт из `[D]`; окно спайка |
| **2048×1106** (референс) | те же 784 (ширина фиксирована, не тянется); вертикальный центр; высота авто | `[D]+[A]` | `width:784px` `[D]`; в дампе окно 2048×1106.4 (REFERENCES §0) |
| ≤ **830** | QR-панель `display:none`; карточка 480; gap исчезает; форма 416 | `[D]` | media 830 (461123) |
| ≤ **485** | карточка `width:100%; height:100%; inset:0; border-radius:0; padding:20px 16px`; фон-градиент `--surface-3 → --bg-app`; логотип `DL` 36×112 виден; контент центрируется, `min-height:540` у обёртки | `[T]+[D]+[A]` | media 485 (348181): градиент `#3d4046→#1e1e23` `[D]`, заменён токенами; логотип 36×112 `[D]`; центрирование `centeringWrapper min-height:540` `[D]` |
| Высота < 460 | карточка `max-height: calc(100% - 48)` и `overflow:auto`; паддинг 16 | `[T]+[A]` | чтобы MFA и капча были достижимы |
| Очень широкий экран | карточка не расширяется (кроме QR-панели), поле не растягивается больше 416 | `[D]` | фиксированные width |
| Печать/скрин | не оптимизируем | `[A]` | — |

Проверка адаптива: 1280×820, 1024×768 (QR виден), 800×600 (QR скрыт, карточка 480 по центру),
485×800 (полноэкранный режим), 360×640 (полноэкранный, вертикальный скролл) `[A]`.


## 12. Токены: карта «значение дампа → токен»

| Дамп | Значение | Токен | Метка |
|-|-|-|-|
| `authBox width` | 480 / 784 | нет токена (компонентный размер) | `[D]` |
| `custom-auth-box-auth-box-padding` | 32 | `--space-9` | `[T]+[D]` |
| `radius-sm` | 8 | `--radius-sm` | `[T]` |
| `modal-background` | #242429 | `--surface-2` | `[T]+[M]` |
| `legacy-elevation-high` | 0 2px 10px rgba(0,0,0,.2) | `--shadow-elevation-high` | `[T]+[A]` |
| `text-muted` | #96979e | `--text-muted` | `[T]+[M]` |
| `text-default` | #efeff1 | `--text` | `[T]` |
| `text-strong` | #fbfbfb | `--text-strong` | `[T]` |
| `input-background-default` | чёрный 12% | `--surface-input` | `[T]+[A]` |
| `input-border-default` | белый 8% | `--border-subtle` | `[T]+[A]` |
| `input-border-active` | #5865f2 | `--accent` | `[T]` |
| `control-input-height-md` | 40 | `--input-height` | `[T]` |
| `text-sm/normal` | 14/18 | `--font-size-ui` / `--line-height-ui` | `[T]` |
| `text-md/normal` | 16/20 | `--font-size-body` / `--line-height-body` (22) | `[T]` |
| `heading-xl/semibold` | 24/30 | `--font-size-display` / `--line-height-display` | `[T]` |
| `heading-lg/semibold` | 20/24 | `--font-size-title` / `--line-height-title` | `[T]` |
| `text-xs/medium` | 12/16 | `--font-size-small` / `--line-height-small` | `[T]` |
| `primary-630` | #2c2d32 | `--surface-4` | `[T]+[M]` |
| `qrLogin width` | 240 | нет токена | `[D]` |
| `qrCodeContainer` | 176×176 | нет токена | `[D]` |
| QR-код | 160×160 | нет токена | `[D]` |
| `gap` формы и QR | 64 | `--space-13` | `[T]+[D]` |
| `margin-bottom` QR | 32 | `--space-9` | `[T]+[D]` |
| `authCaptcha padding` | 24 / 16 | `--space-8` / `--space-6` | `[T]+[D]` |
| `authCaptcha radius` | 5 5 0 0 | `--radius-sm` (8) | `[A]` (5 px токена нет) |
| MFA header/content/footer | 24 / 24 / 24 | `--space-8` | `[T]+[D]` |
| MFA item | 50, gap 8 | `--space-4` (8); 50 — компонентный | `[D]` |
| MFA modal | 442 | `--modal-w-sm` | `[T]+[D]` |
| Кнопка md | 38 | `--btn-height-lg` | `[T]+[D]` |

Значения, которых нет в `tokens.css` (240, 176, 160, 50, 480, 784, 5), — компонентные размеры
структуры, а не «на глаз»: они взяты из CSS/JS дампа и не заменяются произвольными числами.


## 13. Границы, безопасность, что не делаем

1. Пароль и MFA-код: только в памяти на один запрос, затем очистка поля и overwrite; не в
   keyring/логах/DOM/ошибках (контракт §5.1). В спеке нет ни одного значения-примера.
2. Токен: только keyring; не в `localStorage`, не в URL, не в тексте ошибок `[T]`.
3. Капча: только официальный интерактивный виджет; **никаких** solver-сервисов, обхода,
   автоматического решения, «пробива» и подсказок по обходу (ADR-3 P0).
4. 429: показываем `retryAfterMs`, кнопки disabled, без автоматических повторов и циклов.
5. Сеть: только официальные хосты Discord, TLS; окно живых операций — по согласованию PM.
6. Fingerprint/UA/proxy не подделываем (AUTH-RE §8.4); UI не показывает и не логирует их.
7. QR/remote-auth — чужой флоу; экран описан, включение — только решением владельца (ADR-2 §8).
8. Никаких чужих логотипов, шрифтов и картинок: только системный шрифт, свои inline-SVG,
   локально нарисованный QR.


## 14. Миграция текущего спайка

`rust/gui-spike/ui/features/auth.js` + `components/auth.css` сейчас реализуют только
token-import (`idle/validating/ready/error`), карточку `--modal-w-sm` 442 и id `auth-reveal`.
Целевое расхождение (для NET-D2/F1):

| Сейчас | По спеке |
|-|-|
| `idle` | `checking` → `no-session` (7 состояний контракта) |
| карточка 442, radius 12, padding 8/24 | 480 (форма) / 784 (+QR), radius 8, padding 32 |
| только `#auth-token` | `#auth-login` + `#auth-password` + token-блок |
| `#auth-error` есть | сохранить, добавить поля/капчу/MFA |
| нет `#auth-captcha`, `#auth-mfa` | добавить контейнеры, поля и слайды |
| нет QR-панели | `#auth-qr` 240, скрыт ≤830 |
| «Выйти» `#auth-logout` | плюс `data-action="auth-logout"` (контракт §4) |

Старые id/селекторы, на которых держатся тесты RW-38 (`auth-gate`, `auth-card`, `auth-error`,
`auth-status`), сохраняются; новые элементы добавляются, старые не удаляются без отдельного ТЗ.


## 15. Приёмка дизайна (чек-лист QA-AUTH / ревью)

- [ ] 7 состояний контракта достижимы и различимы; переходы только по ответам команд.
- [ ] Все id/роли из AUTH-CONTRACT §4 присутствуют: `#auth-gate`, `#auth-form`, `#auth-login`,
      `#auth-password`, `#auth-token`, `#auth-captcha`, `#auth-mfa`, `#auth-mfa-code`,
      `#auth-error`, `[data-action=auth-logout]`.
- [ ] Ни одно значение пароля/кода/токена не встречается в DOM после submit и в логах.
- [ ] Капча — только интерактивная; в коде нет solver/обхода; тексты не содержат инструкций обхода.
- [ ] 401/429/сеть/captcha/MFA-код показывают свои тексты из §7; 429 не ретраится сам.
- [ ] Фокус, tab-порядок, `role=alert`, `aria-live`, `aria-busy`, `aria-invalid` — по §10.
- [ ] 1280×820 и 2048×1106 выглядят как §11; ≤830 QR скрыт; ≤485 полноэкранный режим.
- [ ] Нет внешних ссылок на изображения и подключённых шрифтовых файлов; все значения — токены
      или помеченные `[D]` компонентные размеры.
- [ ] Сверка с дампом: 480/784, 32, 8, 64, 240, 176, 160, 50, 442, 40, 38 — совпадают.
