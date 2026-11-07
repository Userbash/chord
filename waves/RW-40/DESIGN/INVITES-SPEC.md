# RW-40 / G3 — INVITES-SPEC: модалка создания инвайта (STUB-05) и карточка/принятие инвайта (STUB-07)

Статус: дизайн-контракт (спецификация; продуктовый код не менялся). Дата: 2026-10-30.  
Владелец: **G3 Design**. Потребители: **G2 Frontend** (STUB-05 UI, STUB-07 UI), **G1 Core** (IPC/REST-клиент инвайтов).  
Связанные задачи:
- `STUB-05`: модалка создания приглашения на сервер из `#server-menu` (пункт 2 `guild-invite`) и кнопки быстрого инвайта у заголовка канала в дереве каналов.
- `STUB-07`: карточка превью инвайта в чате (`.invite-card`) и модалка принятия инвайта (`#invite-accept-dialog`).

Нормативные документы:
- `waves/RW-40/RE-DEEP-REA.md` §3 (создание сервера и join) и §4 (STUB-05/STUB-07: создание `POST /channels/{id}/invites`, резолв `GET /invites/{code}`, принятие `POST /invites/{code}`, коды ошибок);
- `waves/RW-40/DESIGN/G05-REST-SPEC.md` (контракт меню сервера, пункт 2 `guild-invite`, диалоги `.dialog-backdrop`, `.dialog`, паттерны ошибок D7, a11y, фокус D2/D3);
- `rust/gui-spike/ui/tokens.css` (источник дизайн-токенов DL: поверхности, акценты, типографика, радиусы, тени, z-index).

---

## 0. Легенда, правила и ограничения

| Метка | Значение | Источник |
|---|---|---|
| `[D]` | Факт из дампа / live-бандла | `RE-DEEP-REA.md` §4 (`[D] 4d2a64cb5df5fc4e_0@632264`, `@1933819`, `@635659`, `@5764857`) |
| `[T]` | Токен проекта из `ui/tokens.css` | `rust/gui-spike/ui/tokens.css` |
| `[A]` | Проектное решение / адаптация под архитектуру DL | Данный документ (проверяемо до кода) |
| `[F]` | Зафиксировано тестом | DOM-контракт |

### Базовые требования спецификации:
1. **Строгая привязка к токенам**: Никаких "сырых" `hex`, `rgb(...)` (кроме существующих внутри токенов) и никаких `!important`. Все цвета, шрифты, отступы, радиусы, высоты и тени задаются только через `var(--token)`.
2. **Отсутствие внешних ассетов**: Иконки — системные глифы UTF-8 (`✕`, `📋`, `🔗`, `⏱️`, `⚙️`) либо простые геометрические псевдоэлементы CSS. Никаких растровых картинок и внешних веб-шрифтов (`tokens.css` §5).
3. **Честная обработка отказов**: Ошибки 429 (с `retry_after`), 403, 404, 10006 не замалчиваются. Реальный clipboard API с обработкой reject (никаких фиктивных подтверждений копирования при ошибке).
4. **Доступность (WAI-ARIA) и клавиатура**: Модалки используют `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `aria-describedby`, trap-фокус, закрытие по `Escape` с обязательным возвратом фокуса на триггер (opener), без паразитного всплытия на закрытие других слоёв (`stopPropagation`).

---

## 1. Спецификация STUB-05 — модалка создания приглашения

### 1.1 Контекст и точки вызова (Triggers)
Модалка создания инвайта вызывается из двух точек интерфейса:
1. **Пункт 2 меню сервера** (`#server-menu .menu-item[data-action="guild-invite"]`):
   - Триггер: `data-action="guild-invite"`, `aria-haspopup="dialog"`.
   - Контекст: текущая активная гильдия `activeGuildId` и её default/выбранный канал `activeChannelId`.
2. **Кнопка канала** в дереве каналов (`#channel-tree .channel-item .channel-invite-btn`):
   - Триггер: кнопка с иконкой `+👤` / `🔗`, `data-action="channel-create-invite"`, `data-channel-id="{channel_id}"`.
   - Контекст: конкретный канал гильдии.

### 1.2 REST-контракт создания инвайта (`[D] RE §4.3`, `[A]` flags)
- **Маршрут**: `POST /channels/{channel_id}/invites`
- **Тело запроса (`CreateInviteRequest`)**:
  ```json
  {
    "max_age": 86400,
    "max_uses": 0,
    "temporary": false,
    "unique": true
  }
  ```
  > **Канонический [D] контракт vs проектное расширение [A]:**  
  > В обязательном каноническом RE-контракте Discord (`RE-DEEP-REA.md §3` и §4.3 схема `ck.e$.CREATE_CHANNEL_INVITE`) вызов создания инвайта включает **исключительно** поля: `channel_id`, `max_age`, `max_uses`, `temporary`, `unique` и target-поля (`target_type`, `target_user_id`, `target_application_id`). Поле `flags` и значение `IS_GUEST_INVITE` **НЕ входят** в обязательный `[D]` контракт RE.  
  > Поле `flags` и битовая маска `IS_GUEST_INVITE = 1 << 0` (значение `1`) — это **опциональное проектное расширение `[A]`** (условная проектная адаптация клиента для гостевого доступа к конкретным каналам). При штатном создании инвайта поле `flags` не является обязательным и не передаётся (либо передаётся равным `0`).
- **Параметры полей и доступные опции**:
  - `max_age` (число, секунды действия): **7 вариантов** из Discord RE (1800, 3600, 21600, 43200, 86400, 604800, 0):
    1. `1800` — «30 минут»
    2. `3600` — «1 час»
    3. `21600` — «6 часов»
    4. `43200` — «12 часов»
    5. `86400` — «1 день» (по умолчанию для стандартного инвайта)
    6. `604800` — «7 дней»
    7. `0` — «Бессрочно» (Never)
  - `max_uses` (число, количество использований): **7 вариантов** из Discord RE §3.3:
    1. `0` — «Без ограничений» (по умолчанию)
    2. `1` — «1 использование»
    3. `5` — «5 использований»
    4. `10` — «10 использований»
    5. `25` — «25 использований»
    6. `50` — «50 использований»
    7. `100` — «100 использований»
  - `temporary` (булево, `[D]`):
    - `false` (по умолчанию) — обычный участник;
    - `true` — «Предоставить временное членство» (пользователь автоматически исключается при отключении, если ему не выдана роль).
  - `flags` (число, опционально, **проектное расширение `[A]`**, не входит в обязательный `[D]` контракт RE §3 CREATE_CHANNEL_INVITE):
    - В обязательном RE-контракте Discord схема `CREATE_CHANNEL_INVITE` не содержит поля `flags`.
    - Гостевой режим с битовой маской `IS_GUEST_INVITE = 1 << 0` (значение `1`) является опциональным проектным расширением `[A]` (соответствует внутренней константе `INVITE_FLAG_IS_GUEST` в `rust/gui-spike/src/invites.rs`).
    - `0` (по умолчанию / при выключенном тумблере) — обычное приглашение;
    - `1` (`1 << 0` = `IS_GUEST_INVITE`) — тумблер «Гостевое приглашение» (опциональная адаптация: доступ только к выбранному каналу без полноправного членства на сервере).
- **Ответ 200/201 OK (`InviteResponse`)**:
  ```json
  {
    "code": "aBcD123XyZ",
    "type": 0,
    "expires_at": "2026-10-31T12:00:00.000Z",
    "guild": { "id": "1014537851418980372", "name": "ITSKIDON PC BOOST", "icon": "..." },
    "channel": { "id": "1014537851418980375", "name": "новости", "type": 0 },
    "inviter": { "id": "...", "username": "..." },
    "max_age": 86400,
    "max_uses": 0,
    "temporary": false,
    "uses": 0,
    "flags": 0
  }
  ```
  > Поле `flags` в объекте ответа также является опциональным проектным расширением `[A]`; обязательные поля канонического `[D]` ответа: `code`, `expires_at`, `max_age`, `max_uses`, `temporary`, `uses`.

### 1.3 DOM-структура модалки создания `#create-invite-dialog`
Модалка монтируется в корневом оверлее `body` аналогично `#join-server-dialog` и `#leave-server-dialog`.

```html
<div id="create-invite-dialog" class="dialog-backdrop" role="presentation" hidden>
  <div class="dialog invite-modal" role="dialog" aria-modal="true"
       aria-labelledby="create-invite-title"
       aria-describedby="create-invite-desc"
       data-state="initial">
    
    <!-- Шапка с заголовком и кнопкой закрытия -->
    <header class="dialog-header">
      <h2 id="create-invite-title" class="dialog-title">Пригласить друзей на сервер</h2>
      <p id="create-invite-desc" class="dialog-hint">
        Ссылка пригласит пользователя в канал <strong id="create-invite-channel-name">#новости</strong>
      </p>
      <button class="icon-btn dialog-close" type="button" data-action="create-invite-close" aria-label="Закрыть">
        <span aria-hidden="true">✕</span>
      </button>
    </header>

    <!-- Блок готовой ссылки и копирования -->
    <div class="invite-link-section">
      <label class="dialog-field-label" for="invite-link-input">Отправьте эту ссылку друзьям</label>
      <div class="invite-link-box">
        <input id="invite-link-input" class="dialog-input invite-link-input" type="text"
               readonly spellcheck="false"
               value="https://discord.gg/..."
               aria-label="Ссылка-приглашение" />
        <button id="invite-copy-btn" class="btn btn-primary invite-copy-btn" type="button"
                data-action="create-invite-copy">
          Копировать
        </button>
      </div>
      <p id="invite-copy-feedback" class="dialog-hint invite-copy-status" role="status" aria-live="polite"></p>
    </div>

    <!-- Раскрывающийся блок расширенных настроек параметров -->
    <div class="invite-settings-accordion">
      <button id="invite-settings-toggle" class="invite-settings-toggle" type="button"
              data-action="create-invite-toggle-settings"
              aria-expanded="false" aria-controls="invite-settings-panel">
        <span class="invite-settings-icon" aria-hidden="true">⚙️</span>
        <span>Настроить ссылку приглашения</span>
        <span class="invite-accordion-arrow" aria-hidden="true">▼</span>
      </button>

      <form id="invite-settings-panel" class="invite-settings-panel" hidden>
        <!-- Срок действия (max_age) -->
        <div class="dialog-field">
          <label class="dialog-field-label" for="invite-max-age">Устареет через</label>
          <select id="invite-max-age" class="dialog-select" name="max_age">
            <option value="1800">30 минут</option>
            <option value="3600">1 час</option>
            <option value="21600">6 часов</option>
            <option value="43200">12 часов</option>
            <option value="86400" selected>1 день</option>
            <option value="604800">7 дней</option>
            <option value="0">Бессрочно</option>
          </select>
        </div>

        <!-- Максимальное количество использований (max_uses) -->
        <div class="dialog-field">
          <label class="dialog-field-label" for="invite-max-uses">Максимальное количество использований</label>
          <select id="invite-max-uses" class="dialog-select" name="max_uses">
            <option value="0" selected>Без ограничений</option>
            <option value="1">1 использование</option>
            <option value="5">5 использований</option>
            <option value="10">10 использований</option>
            <option value="25">25 использований</option>
            <option value="50">50 использований</option>
            <option value="100">100 использований</option>
          </select>
        </div>

        <!-- Тумблер временного членства (temporary) -->
        <div class="dialog-toggle-row">
          <div class="toggle-text-block">
            <label class="dialog-toggle-label" for="invite-temporary-toggle">Предоставить временное членство</label>
            <span class="dialog-hint">Пользователь будет исключён при отключении, если ему не назначена роль</span>
          </div>
          <input id="invite-temporary-toggle" class="dialog-switch" type="checkbox" name="temporary" />
        </div>

        <!-- Тумблер гостевого приглашения: опциональное проектное расширение [A] (flags: 1 << 0) -->
        <div class="dialog-toggle-row">
          <div class="toggle-text-block">
            <label class="dialog-toggle-label" for="invite-guest-toggle">Гостевое приглашение [A]</label>
            <span class="dialog-hint">Опциональное расширение: даёт доступ только к текущему каналу без постоянного членства</span>
          </div>
          <input id="invite-guest-toggle" class="dialog-switch" type="checkbox" name="guest" />
        </div>

        <button id="invite-generate-btn" class="btn btn-secondary invite-generate-btn" type="button"
                data-action="create-invite-generate">
          Сгенерировать новую ссылку
        </button>
      </form>
    </div>

    <!-- Блок отображения ошибок и лимитов -->
    <div id="create-invite-error" class="dialog-error invite-error-banner" role="alert" aria-live="assertive" hidden>
      <span class="invite-error-icon" aria-hidden="true">⚠️</span>
      <span id="create-invite-error-text" class="invite-error-text"></span>
    </div>

    <!-- Футер модалки -->
    <footer class="dialog-actions">
      <button class="btn btn-secondary" type="button" data-action="create-invite-close">Готово</button>
    </footer>

  </div>
</div>
```

### 1.4 Состояния модалки создания (`data-state`)

1. **`initial` / `loading`**:
   - Отправка запроса `POST /channels/{channel_id}/invites` при открытии (или получении дефолтной ссылки).
   - Поле `#invite-link-input` содержит плейсхолдер `Генерация ссылки...` с `aria-busy="true"`.
   - Кнопка `#invite-copy-btn` отключена: `disabled="true"`.
   - Селекты и переключатели временно `disabled="true"`.
2. **`populated` (успех)**:
   - В `#invite-link-input` подставляется канонический URL: `https://discord.gg/${res.code}`.
   - Поле доступно для выделения (`select()`), кнопка `#invite-copy-btn` активна.
   - **Копирование в буфер обмена**:
     - Обработчик вызывает `navigator.clipboard.writeText(url)`.
     - При resolve:
       - Кнопка временно меняет класс на `.btn-positive` и текст на `Скопировано!`.
       - В `#invite-copy-feedback` выводится «Ссылка скопирована в буфер обмена».
       - Через 2000 мс восстанавливается текст `Копировать`.
     - При reject (или отсутствии разрешения):
       - Текст и цвет меняются на ошибку: `Не удалось скопировать`.
       - Выполняется резервный `input.select()`, чтобы пользователь мог скопировать через `Ctrl+C`.
       - Ложный статус успеха **запрещён** (согласно дефекту D4 из G-05).
3. **`error-429` (Rate limited)**:
   - Сервер ответил `429 Too Many Requests` с заголовком `Retry-After: {seconds}` или телом `{retry_after: number}`.
   - Контейнер `#create-invite-error` отображается (`hidden=false`).
   - Кнопки генерации и копирования блокируются: `disabled="true"`.
   - Текст ошибки: `Слишком много запросов. Повторите через {n} с.`
   - Запускается таймер обратного отсчёта каждую секунду до 0 (паттерн D7 из `G05-REST-SPEC.md`).
   - По истечении таймера баннер скрывается, кнопка генерации снова становится активной для одной ручной попытки.
4. **`error-403` (Missing Permissions)**:
   - Сервер вернул `403 Forbidden` (`CREATE_INSTANT_INVITE` отсутствует у пользователя в канале).
   - Текст ошибки: `У вас нет прав на создание приглашений в этот канал.`
   - Вся форма настроек и блок генерации блокируются.

### 1.5 a11y, фокус и клавиатура
- **Открытие**:
  - Сохраняется `triggerElement` (активный элемент до открытия модалки).
  - Фокус устанавливается в `#invite-link-input` (или на кнопку закрытия, если ссылка ещё генерируется).
- **Tab-навигация**:
  - Focus trap циклически переключает фокус только внутри элементов модалки (`input`, `button`, `select`).
- **Escape**:
  - Нажатие `Escape` перехватывается, вызывает `e.stopPropagation()` и `e.preventDefault()`.
  - Модалка скрывается (`hidden = true`).
  - Фокус возвращается на `triggerElement` (opener), не вызывая побочного закрытия экрана или других панелей (устранение бага D3).

---

## 2. Спецификация STUB-07 — карточка инвайта в чате и принятие

### 2.1 Контекст рендеринга
Инвайт распознаётся в теле сообщения регулярным выражением вида:  
`(?:https?:\/\/)?(?:www\.)?(?:discord\.gg|discord\.com\/invite)\/([a-zA-Z0-9-]+)`  
В месте распознавания инвайта под текстом сообщения рендерится карточка превью инвайта (`.invite-card`).

### 2.2 Жизненный цикл и REST-маршруты (`[D] RE §4.1, §4.2`)

1. **Резолв превью (`GET /invites/{code}`)**:
   - Оракул: `[D] 4d2a64cb5df5fc4e_0@5764857` (RE §4.2).
   - Запрос: `GET /invites/{code}` с query-параметрами (`ResolveInviteQuery`):
     - `with_counts=true` (`!0`) — запрос счётчиков `approximate_member_count` и `approximate_presence_count`;
     - `with_expiration=true` (`!0`) — запрос метки времени истечения `expires_at`;
     - `with_permissions=true` (`!0`) — запрос флагов разрешений;
     - `guild_scheduled_event_id` (string, опционально) — ID запланированного события гильдии;
     - `target_channel_id` (string, опционально) — ID целевого канала;
     - `target_message_id` (string, опционально) — ID целевого сообщения;
     - `inputValue` (string, опционально) — введённая пользователем строка ссылки;
     - `with_games` (boolean, условный: `!!withGames || undefined`) — передаётся условно при контексте игр/активностей;
     - `with_guild_experiments` (boolean, условный: `!!withGuildExperiments || undefined`) — передаётся условно при запросе экспериментов гильдии.
   - Пример URL: `GET /invites/aBcD123XyZ?with_counts=true&with_expiration=true&with_permissions=true`
   - Ответ `200 OK`:
     ```json
     {
       "code": "aBcD123XyZ",
       "type": 0,
       "expires_at": "2026-10-31T12:00:00.000Z",
       "guild": {
         "id": "1014537851418980372",
         "name": "ITSKIDON PC BOOST",
         "icon": "a_1234567890abcdef",
         "description": "Лучшее игровое сообщество",
         "verification_level": 1,
         "features": ["VANITY_URL", "COMMUNITY"]
       },
       "channel": { "id": "1014537851418980375", "name": "новости", "type": 0 },
       "approximate_member_count": 24229,
       "approximate_presence_count": 735,
       "inviter": { "id": "...", "username": "ShadowByte" }
     }
     ```

2. **Принятие инвайта (`POST /invites/{code}`)**:
   - Оракул: `[D] 4d2a64cb5df5fc4e_0@635659` (RE §4.1).
   - Маршрут: `POST /invites/{code}`
   - Контекст запроса: `context: { ...context, invite_guild_scheduled_event_id }`, `oldFormErrors: true`.
     - `invite_guild_scheduled_event_id` (string, опционально) — ID запланированного события гильдии, передаваемый в объекте контекста запроса.
   - Тело запроса (`AcceptInviteRequest`):
     ```json
     {
       "session_id": "d6183157...",
       "invite_instance_id": "...",
       "received_installation_id": "..."
     }
     ```
     - `session_id` (string | null): gateway-сессия клиента (nullable, в бандле может отдавать null);
     - `invite_instance_id` (string | null, опционально): идентификатор инстанса инвайта из ответа резолва;
     - `received_installation_id` (string | null, опционально / условно): передаётся только если инвайт открыт по ссылке установки приложения (`received_installation_id: p`, чистится после успешного принятия).
   - Ответ `200 OK`: объект инвайта (`channel`, `guild`, `inviter`, `new_member`, `flags`, `expires_at`…). Если `c.new_member && !c.show_verification_form` — вход завершён, иначе клиентом открывается форма проверки участника.

3. **Матрица ошибок принятия инвайта (`[D] RE §4.1`, enum `@322893`)**:

| Код / Ключ ошибки | Имя ошибки | Реакция клиента / Сообщение в интерфейсе | Источник |
|---|---|---|---|
| `10006` | `UNKNOWN_INVITE` | Тост/баннер: «Срок действия приглашения истёк или оно недействительно.» | `[D]` RE §4.1 |
| `10004` | `UNKNOWN_GUILD` | Ошибка: «Сервер не найден или был удалён.» | `[D]` RE §4.1 |
| `10007` | `UNKNOWN_MEMBER` | Ошибка: «Не удалось определить участника для входа.» | `[D]` RE §4.1 |
| `40007` | `USER_BANNED` | Запрет входа: «Вы заблокированы на этом сервере.» | `[D]` RE §4.1 |
| `50035` | `INVALID_FORM_BODY` | Ошибка формы: «Некорректные параметры тела запроса.» | `[D]` RE §4.1 |
| `30001` | `TOO_MANY_USER_GUILDS` | Лимит серверов: «Достигнут лимит серверов для вашей учётной записи.» | `[D]` RE §4.1 |
| `USER_GUILD_JOIN_LARGE_GUILD_UNDERAGE_DISALLOWED` | Возрастное ограничение | Блокировка входа несовершеннолетним участникам в крупные сообщества (`[D] @635600`). | `[D]` RE §4.1 |
| `USER_GUILD_JOIN_AGE_RESTRICTED_IOS_DISALLOWED` | Возрастное ограничение (платформенное) | Запрет входа в сообщества с возрастным ограничением 18+ (`[D] @635600`). | `[D]` RE §4.1 |
| `429` | `RATE_LIMITED` | Превышен лимит запросов: «Слишком много попыток. Повторите через {n} с.» Блокировка с кулдауном. | `[T]`/`[A]` D7 |

### 2.3 DOM-структура карточки превью инвайта в чате (`.invite-card`)

```html
<div class="invite-card" role="region" aria-label="Приглашение на сервер"
     data-invite-code="aBcD123XyZ"
     data-state="valid">
  
  <!-- Заголовок карточки -->
  <span class="invite-card-header">ВАС ПРИГЛАШАЮТ ПРИСОЕДИНИТЬСЯ К СЕРВЕРУ</span>

  <!-- Контентная область -->
  <div class="invite-card-content">
    
    <!-- Аватар сервера / плейсхолдер -->
    <div class="invite-card-avatar" aria-hidden="true">
      <span class="invite-avatar-text">IP</span>
    </div>

    <!-- Основная информация -->
    <div class="invite-card-info">
      <div class="invite-card-title-row">
        <h3 class="invite-card-guild-name" title="ITSKIDON PC BOOST">ITSKIDON PC BOOST</h3>
      </div>
      
      <!-- Статистика участников (в состоянии valid) -->
      <div class="invite-card-counts">
        <span class="invite-count-item">
          <span class="status-dot status-online" aria-hidden="true"></span>
          <span class="count-value">735</span> в сети
        </span>
        <span class="invite-count-item">
          <span class="status-dot status-offline" aria-hidden="true"></span>
          <span class="count-value">24 229</span> участников
        </span>
      </div>

      <!-- Сообщение об ошибке/истечении (в состояниях expired/error) -->
      <p class="invite-card-error-msg" hidden></p>
    </div>

    <!-- Кнопка действия -->
    <div class="invite-card-action">
      <button class="btn btn-primary invite-btn-join" type="button"
              data-action="invite-accept"
              data-invite-code="aBcD123XyZ">
        Вступить
      </button>
    </div>

  </div>
</div>
```

### 2.4 Состояния карточки инвайта (`data-state`)

1. **`loading`**:
   - `data-state="loading"`, `aria-busy="true"`.
   - Заголовок: `ВАС ПРИГЛАШАЮТ ПРИСОЕДИНИТЬСЯ К СЕРВЕРУ`.
   - Название: плейсхолдер `Загрузка приглашения...`.
   - Счётчики участников скрыты.
   - Кнопка действия: `disabled="true"`, текст `Загрузка...`.
2. **`valid`**:
   - `data-state="valid"`.
   - Заголовок: `ВАС ПРИГЛАШАЮТ ПРИСОЕДИНИТЬСЯ К СЕРВЕРУ`.
   - Реальное имя гильдии, иконка/инициалы, точные счётчики участников.
   - Кнопка: `.btn-primary`, текст `Вступить`, доступна для клика.
3. **`joined`**:
   - `data-state="joined"`.
   - Пользователь уже состоит в этой гильдии (`guildId` найден в списке гильдий клиента).
   - Кнопка действия: `.btn-secondary`, текст `Перейти`, клик переключает активную гильдию в интерфейсе без повторного `POST /invites/{code}`.
4. **`expired` / `error-404` (код 10006 `UNKNOWN_INVITE`)**:
   - `data-state="expired"`.
   - Заголовок: `ПРИГЛАШЕНИЕ НЕ ДЕЙСТВИТЕЛЬНО`.
   - Аватар: серая заглушка со значком `⚠️`.
   - Описание: `Срок действия этого приглашения истёк или оно было удалено.`
   - Счётчики скрыты.
   - Кнопка: `.btn-secondary`, текст `Истекло`, `disabled="true"`.
5. **`rate-limited` (`error-429`)**:
   - `data-state="rate-limited"`.
   - Заголовок: `ПРЕВЫШЕН ЛИМИТ ЗАПРОСОВ`.
   - Текст ошибки: `Слишком много попыток. Повторите через {n} с.`
   - Кнопка: `disabled="true"`, заблокирована на время кулдауна с обратным отсчётом секунд.

---

## 3. Точная привязка к дизайн-токенам (`rust/gui-spike/ui/tokens.css`)

Все правила стилизации формируются **строго из существующих токенов**, без каких-либо hex/rgb и без `!important`.

### 3.1 Модалка создания инвайта (`#create-invite-dialog`)

```css
/* Фоновая подложка (scrim) модалки: 72% полупрозрачность на базе нормативного токена --bg-app из tokens.css */
#create-invite-dialog.dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-8);
  overflow-y: auto;
  background: color-mix(in srgb, var(--bg-app) 72%, transparent);
}

#create-invite-dialog[hidden] {
  display: none;
}

/* Контейнер модального окна: отступы viewport через токен --space-11 (48px) */
.invite-modal.dialog {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  width: min(var(--modal-w-sm), calc(100vw - var(--space-11)));
  max-height: calc(100vh - var(--space-11));
  overflow-y: auto;
  padding: var(--modal-pad-y) var(--modal-pad-x);
  border-radius: var(--radius-md);
  background: var(--surface-2);
  box-shadow: var(--shadow-popover);
  color: var(--text);
}

/* Шапка модалки */
.dialog-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.dialog-title {
  margin: 0;
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-title);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-title);
}

.dialog-hint {
  margin: 0;
  color: var(--text-muted);
  font-family: var(--font-sans);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.dialog-close {
  position: absolute;
  top: var(--space-4);
  right: var(--space-4);
  width: var(--btn-height);
  height: var(--btn-height);
  border: 0;
  border-radius: var(--radius-round);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}

.dialog-close:hover {
  background: var(--state-hover);
  color: var(--text-strong);
}

.dialog-close:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

/* Поле ссылки и кнопка копирования */
.invite-link-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.dialog-field-label {
  color: var(--text-subtle);
  font-family: var(--font-sans);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  text-transform: uppercase;
}

.invite-link-box {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-input);
}

.invite-link-input {
  flex: 1 1 auto;
  height: var(--input-height);
  padding: 0 var(--space-3);
  border: 0;
  background: transparent;
  color: var(--text-strong);
  font-family: var(--font-mono);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.invite-link-input:focus-visible {
  outline: none;
}

.invite-copy-btn {
  flex: 0 0 auto;
  height: var(--btn-height);
  padding: 0 var(--btn-pad-x);
  border: 0;
  border-radius: var(--btn-radius);
  background: var(--accent);
  color: var(--text-on-accent);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.invite-copy-btn:hover:not(:disabled) {
  background: var(--accent-hover);
}

.invite-copy-btn:active:not(:disabled) {
  background: var(--accent-active);
}

.invite-copy-btn.btn-positive {
  background: var(--positive);
  color: var(--text-on-accent);
}

.invite-copy-btn:disabled {
  opacity: var(--state-disabled-opacity);
  cursor: default;
}

.invite-copy-btn:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.invite-copy-status {
  min-height: var(--line-height-small);
  color: var(--positive-text);
}

/* Аккордеон настроек */
.invite-settings-accordion {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  border-top: 1px solid var(--divider);
  padding-top: var(--space-4);
}

.invite-settings-toggle {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border: 0;
  background: transparent;
  color: var(--text-accent);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  cursor: pointer;
}

.invite-settings-toggle:hover {
  text-decoration: underline;
}

.invite-settings-toggle:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.invite-settings-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-3);
  border-radius: var(--radius-sm);
  background: var(--surface-3);
}

.invite-settings-panel[hidden] {
  display: none;
}

.dialog-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.dialog-select {
  height: var(--input-height);
  padding: 0 var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-input);
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  cursor: pointer;
}

.dialog-select:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: calc(var(--focus-ring-offset) * -1);
}

/* Тумблеры */
.dialog-toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.toggle-text-block {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.dialog-toggle-label {
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
}

.dialog-switch {
  appearance: none;
  width: var(--switch-w);
  height: var(--switch-h);
  border-radius: var(--radius-round);
  background: var(--surface-4);
  position: relative;
  cursor: pointer;
  outline: none;
  transition: background var(--motion-fast) var(--ease-out);
}

.dialog-switch::after {
  content: "";
  position: absolute;
  top: 3px;
  left: 3px;
  width: var(--switch-thumb);
  height: var(--switch-thumb);
  border-radius: var(--radius-round);
  background: var(--text-strong);
  transition: transform var(--motion-fast) var(--ease-out);
}

.dialog-switch:checked {
  background: var(--positive);
}

.dialog-switch:checked::after {
  transform: translateX(16px);
}

.dialog-switch:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

/* Баннер ошибок */
.invite-error-banner {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-sm);
  background: var(--surface-4);
  border-left: 4px solid var(--danger);
  color: var(--danger-text);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
}

.invite-error-banner[hidden] {
  display: none;
}
```

### 3.2 Карточка превью инвайта в чате (`.invite-card`)

```css
.invite-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: 100%;
  max-width: var(--modal-w-sm);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
  box-shadow: var(--shadow-elevation-low);
  margin-top: var(--space-2);
}

.invite-card-header {
  color: var(--text-muted);
  font-family: var(--font-sans);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  letter-spacing: var(--letter-spacing-caps);
  text-transform: uppercase;
}

.invite-card-content {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.invite-card-avatar {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: var(--avatar-lg);
  height: var(--avatar-lg);
  border-radius: var(--radius-md);
  background: var(--surface-3);
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-semibold);
}

.invite-card-info {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.invite-card-guild-name {
  margin: 0;
  overflow: hidden;
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.invite-card-counts {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  color: var(--text-muted);
  font-family: var(--font-sans);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.invite-count-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-round);
}

.status-dot.status-online {
  background: var(--status-online);
}

.status-dot.status-offline {
  background: var(--status-offline);
}

.invite-card-error-msg {
  margin: 0;
  color: var(--text-subtle);
  font-family: var(--font-sans);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.invite-card-error-msg[hidden] {
  display: none;
}

.invite-card-action {
  flex: 0 0 auto;
  margin-left: auto;
}

.invite-btn-join {
  height: var(--btn-height-lg);
  padding: 0 var(--btn-pad-x);
  border: 0;
  border-radius: var(--btn-radius);
  background: var(--positive);
  color: var(--text-on-accent);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.invite-btn-join:hover:not(:disabled) {
  background: var(--positive-text);
  color: var(--text-inverse);
}

.invite-btn-join:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.invite-btn-join:disabled {
  opacity: var(--state-disabled-opacity);
  cursor: default;
}

/* Модификатор состояния Joined / Expired */
.invite-card[data-state="joined"] .invite-btn-join {
  background: transparent;
  border: 1px solid var(--border-strong);
  color: var(--text-strong);
}

.invite-card[data-state="joined"] .invite-btn-join:hover:not(:disabled) {
  background: var(--state-hover);
}

.invite-card[data-state="expired"] {
  opacity: var(--state-disabled-opacity);
}

.invite-card[data-state="expired"] .invite-card-avatar {
  background: var(--surface-4);
  color: var(--text-muted);
}
```

---

## 4. Матрица тестовых сценариев и контракт верификации

Данная спецификация проверяется компонентными и контрактными тестами:

| ID | Тест-сценарий | Проверка контракта | Файл теста |
|---|---|---|---|
| **T-INV-01** | Открытие модалки инвайта из меню сервера | Клик по пункту `guild-invite` открывает `#create-invite-dialog`, ставит `aria-modal="true"`, заголовок и имя текущего канала | `tests/unit/server-menu.test.mjs` / `gui-spike-features.test.mjs` |
| **T-INV-02** | Полнота опций TTL `max_age` (7 вариантов) | Селект содержит ровно 7 вариантов (1800, 3600, 21600, 43200, 86400, 604800, 0) по RE §4.3 | `tests/unit/invites.test.mjs` |
| **T-INV-03** | Полнота опций `max_uses` (7 вариантов) | Селект содержит ровно 7 вариантов (0, 1, 5, 10, 25, 50, 100) | `tests/unit/invites.test.mjs` |
| **T-INV-04** | Флаги `temporary` (`[D]`) и опциональный `IS_GUEST_INVITE` (`[A]`, 1<<0) | Переключение чекбокса `temporary` передаёт `temporary: true` (`[D]`). Гостевой режим является опциональным проектным расширением `[A]` (не входит в обязательный `[D]` контракт RE §3 CREATE_CHANNEL_INVITE) и при активации передаёт `flags: 1` (`1 << 0`) | `tests/unit/invites.test.mjs` |
| **T-INV-05** | Копирование через реальный `clipboard.writeText` | При клике «Копировать» вызывается `writeText(url)`. При успехе — `Скопировано!`, при reject — `Не удалось скопировать` без ложного статуса | `tests/unit/invites.test.mjs` |
| **T-INV-06** | Обработка 429 и обратный отсчёт `retry_after` | Статус 429 блокирует кнопки, запускает отсчёт `Повторите через {n} с.`, по достижении 0 разблокирует повтор | `tests/unit/invites.test.mjs` |
| **T-INV-07** | Обработка 403 (Missing Permissions) | Отображает баннер с понятной причиной, не роняет приложение | `tests/unit/invites.test.mjs` |
| **T-INV-08** | Escape и фокус модалки | Нажатие Escape закрывает модалку, не всплывает на `body`, возвращает фокус на opener | `tests/unit/invites.test.mjs` |
| **T-INV-09** | 5 состояний карточки инвайта в чате | `loading`, `valid`, `joined`, `expired` (10006), `rate-limited` (429) отображают заданный DOM и правильный `data-state` | `tests/unit/invites.test.mjs` |
| **T-INV-10** | Валидация CSS-токенов | Тест проверяет отсутствие сырых цветов (hex/rgb) и отсутствие `!important` в файле стилей инвайтов | `tests/unit/tokens-lint.test.mjs` |
