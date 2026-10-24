# G3-STREAM-REVIEW — STREAM / Go Live: что подтверждено, что blocked

Карточка: **G3-STREAM-REVIEW**. Владелец: G3 Design. Дата: 2026-10-24.
Режим: **read-only**. HEAD `0239543`. Код, тесты, дизайн-документы и `waves/*` не менялись;
создан только этот файл.

Читано (все пути — от корня репозитория):

| Документ | Где реально лежит | md5 |
|-|-|-|
| STREAM-контракт | `waves/RW-40/DESIGN/STREAM.md` (938 строк) | `dd897e184246d42762c1771474f051db` |
| Машиночитаемая версия | `waves/RW-40/DESIGN/stream-spec.json` | `1452d0c49112893d23ee42d80955b593` |
| S-01…S-04 | `waves/RW-40/PARITY/PARITY-MATRIX.md` §8 | — |
| Инвентарь функций | `waves/RW-40/PARITY/FUNCTION-INVENTORY.md` §8 | — |
| Голос (дизайн) | `waves/RW-40/DESIGN/VOICE.md` §2, §4, §5, §9–§11 | `157a5cbdaa6feebb885f56715c91b194` |
| Voice prereq | `waves/RW-40/VOICE/G5-PREREQ.md` | `5dbd92c07150c55571bb7421f220167d` |
| Платформенный план | `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` | — |
| RE голоса | `docs/re/voice-deep.md` | `f324a3766150f93c2b8d99f3c00a4a78` |
| RE нативных модулей | `docs/re/native-re.md` | `182d4d48cc3376f8e9a0537706dad3e5` |
| Сеть/шлюз | `waves/RW-40/NETWORK-CONTRACTS.md` §3, §5 (B-08…B-16) | `79ed101289ac9630fbf5687b2238915b` |
| Медиа-крейт | `rust/crates/discord-media/{Cargo.toml,src/lib.rs}` | 26 строк кода |

> **О путях.** В постановке указаны `DESIGN/STREAM.md`, `PARITY/PARITY-MATRIX.md`,
> `DESIGN/VOICE.md`, `VOICE/G5-PREREQ.md`. Каталогов `DESIGN/`, `PARITY/`, `VOICE/` в корне
> **нет**; все они существуют только как `waves/RW-40/…`. Отчёт поэтому положен в
> `waves/RW-40/DESIGN/STREAM-REVIEW.md`.

Метки — как в контракте: `[T]` токен проекта · `[M]` пиксельный замер · `[D]` факт дампа ·
`[L]` живой захват · `[S]` код/протокол эталона · `[H]` факт хоста · `[A]` наше решение/не
подтверждено · `[F]` заморожено тестами.

## 0. Вердикт

1. **Дизайн-контракт состоятелен по форме.** Метки расставлены честно, паритет по экранам стрима
   не заявлен (`checks.parity_claim=false`), выдуманных px нет, 54/54 строк копирайта сверены с
   `ru-catalog.json`, 35/35 токенов существуют. Валидатор воспроизведён: `checks=23 failures=0`,
   замер ряда воспроизведён: `6/6`.
2. **Кода нет ни одного слоя стрима.** В `rust/`, `src/`, `tests/` — **0** совпадений по
   `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_DELETE`, `stream_key`,
   `presentNativeScreenSharePicker`, `desktopCapturer`, `org.freedesktop.portal.ScreenCast`,
   `pw_stream`, `v4l2`, `isVaapiEnabled`. `discord-media` — 26 строк констант, без зависимостей,
   никем не подключён.
3. **Что есть:** разбор видеометаданных RTC (`video:true`, `streams[{rid:"100",quality:100}]`,
   `video_codec`) с 11 тестами `signaling.rs`; gateway-поля `self_video`/`self_stream` в
   `discord-core/src/state/voice/*`; голосовая панель на фейковом store. Кнопок камеры/экрана в
   панели **нет вовсе** (вопреки «disabled-заглушкам» из D-02).
4. **Блокеры:** RE-3 (события и старт/стоп), PL-07 (consent до S-02), отсутствие PipeWire-клиента,
   D-Bus без `t`/u64, отсутствие камеры на хосте, зелёный voice MVP. **Не указан в STREAM.md:**
   B-08 (схема SRTP KDF/nonce/AAD) — а второй RTC и зритель идут по тому же SRTP.
5. **Три конфликта между документами/кодом:** (а) превью-кадры в UI против «кадры не уходят в
   рендерер/IPC в MVP»; (б) камера `v4l2` против нерешённого portal-vs-v4l2; (в) ряд кнопок
   32/8/8 против фактических 28/2/4 в `voice.css`.
6. **Находок:** 4 × P1, 9 × P2, 5 × P3 (18). Все — уровня документа/дизайна, кода не касаются.
   Ни одна не отменяет контракт; P1 требуют решения PM до реализации.

## 1. Что реально есть

| Слой | Факт | Доказательство | Метка |
|-|-|-|-|
| Медиа-крейт | `AUDIO_CODEC="opus"`, `VIDEO_CODEC="AV1"`, `ENGINE_CODECS[4]`, 1 тест; `unsafe_code="forbid"`; нет `[dependencies]`; нет ни одного `use discord_media` в воркспейсе | `discord-media/src/lib.rs:1-26`, `Cargo.toml` | `[S]` |
| RTC-сигнализация | `StreamSpec{kind,ssrc,rtx_ssrc,rid,quality,active}`, `Identify.video/streams`, `Ready.streams`, `SessionDescription.video_codec`; тесты сверяют `rid="100"`, `quality=100`, `video_codec` с кадрами из `fixtures/voice/rtc1-frames.jsonl` | `discord-voice/src/lib.rs:121-289`, `tests/signaling.rs` (11) | `[S]`/`[L]` |
| Gateway-состояние | `self_video`, `self_stream` парсятся и редуцируются; `session_id` не сохраняется | `core/src/state/voice/{types,parser,reducer}.rs` | `[S]` |
| Аудио-RTP | 13 контрактных тестов по golden-фикстуре PT120 (оракул — pcap, не реализация) | `discord-voice/tests/media_contract.rs`, `fixtures/voice/rtp-pt120-oracle.json` | `[L]` |
| Видео-RTP | **нет** ни кода, ни фикстуры (видео-поток не снимался) | `ls fixtures/voice` (6 файлов, только audio/rtc/session3) | `[A]` |
| Захват экрана | в нашем коде — нет; в эталоне — портал `ScreenCast` + `pw_stream_*` (`BaseCapturerPipeWire`), PipeWire через `dlopen` | `docs/re/native-re.md:272-286` | `[D]` |
| Камера | в эталоне — `getVideoInputDevices`/`setVideoInputDevice`; на хосте камеры нет: `/dev/video*` отсутствует, портал `IsCameraPresent=false` | `G5-VOICE-PLAN.md` §1, §7.3 | `[D]`/`[H]` |
| UI | панель 12 состояний на фейке, `#voice-panel[data-state]`, `#voice-error`, `#voice-empty`; **нет** `#voice-screen`, `#voice-camera`, `#stream-*`, `data-stream` | `ui/features/voice.js` (710), `ui/index.html:83` | `[F]` |
| UI-сервис/IPC | команд `stream.*`/`camera.*` нет в `main.rs`; `CONTRACTS/` содержит только `dispatch.md`, `read-state.md` | `gui-spike/src/main.rs`, `ls CONTRACTS` | `[A]` |
| Доки | STREAM.md (938) + stream-spec.json (36 G2 + 12 G4 + 10 G5 кейсов, 14 ошибок, 14 рисков, 17 пробелов) | валидатор 23/23 | `[A]` |

Чего нет и это подтверждено поиском: `STREAM_*`-диспатчи, `stream_key`, второй RTC, энкод, mux,
захват, consent-flow, права `canStream`/`canVideo`, любые экраны стрима.

## 2. Что blocked RE-3 / live

| Блокер | Что именно нельзя | Почему не офлайн | Источник | Влияет на |
|-|-|-|-|-|
| **RE-3 (B-11)** | какой REST-вызов порождает `STREAM_CREATE`; живой `STREAM_*` (0 событий в 1340 op0) | событий нет ни в одном из 4 capture; схема только по коду | `NETWORK-CONTRACTS` §3.3, B-11 | S-02, S-03 |
| **RE-3** | коды/тела ответов `POST …/notify`, `PATCH …/stream`, `GET/POST …/preview` | вызовов не было | там же | S-02 |
| **RE-3 (SG-05)** | семантика `viewer_ids` (полный/дельта) и `paused` | ни одного события | `STREAM.md` §10 | S-03, UI paused |
| **RE-3 (B-10)** | `quality` enum `eq.Y4`/`eq.Cl`, выбор rid/quality зрителем | enum не инлайнится в бандл | `NETWORK-CONTRACTS` §3.1 | S-03 |
| **B-08** | шифрование/дешифровка медиа: `SrtpContext` требует 32 Б ключ + **14 Б соль**, op4 даёт только `secret_key` 32 Б; KDF/nonce/AAD — наша изобретённая схема | ключ вырезан, схема не восстановлена | `G5-PREREQ` C-05/§4 п.4, `NETWORK-CONTRACTS` B-08 | S-02, S-03 (весь wire-путь) |
| **PL-07** | consent-flow (что спрашивает ОС, что мы) | не снят; витрина — только строки каталога | `GAP-BACKLOG` §5, `PARITY-MATRIX` PL-07 | S-02 (согласие — **до**, не после) |
| **V-03/voice MVP** | транспорт не зелёный: RTC manager, UDP-live, SRTP-live, Opus/Dave отсутствуют | нет live-тонов и двух клиентов | `G5-PREREQ` §0/§8, `NETWORK-ROADMAP` §10 п.6 | S-02, S-03 |
| **PipeWire-клиент** | нет клиента портала (шаг 4 плана G5); `pw_stream_*` в нашем коде = 0 | требует Rust-callback в C + `catch_unwind`-дисциплина (D-9) | `G5-VOICE-PLAN` §2.2, B-3 | захват кадра |
| **D-Bus `t`/u64** | `Value` не знает `x/t/d/n/q`; fd — только `call_with_fds` | типовая дыра клиента | `discord-platform/src/dbus/mod.rs:17-19`, B-5 | `OpenPipeWireRemote`, portal `Session`-поля |
| **Камера (B-1)** | любой live-DoD камеры | камеры физически нет (`/dev/video*`, `IsCameraPresent=false`) | `G5-VOICE-PLAN` §1, §11 | S-01 (только fake/property) |
| **X11-путь** | захват на X11: портал-или-XShm не решён (D-3 рекомендует portal-only) | не проверялся | `G5-VOICE-PLAN` §7.2 | S-02 на X11-сессии |
| **Системный звук (B-16)** | шаринг звука | `soundshare_loopback` win32-only, захвата не было | `NETWORK-CONTRACTS` §3.4 | вне MVP (D-04) |
| **Права канала (SG-12)** | `canStream`/`canVideo` и коды отказа | источник права не снят | `STREAM.md` §10 | активация кнопок |
| **Lock/sleep** | останов захвата при блокировке экрана/сне | в `power.rs` нет lock-сигнала (B-9) | `G5-VOICE-PLAN` B-9 | индикатор «стримлю» |
| **`codec survey is not available`** | смысл ошибки в логе при дисконнекте | одно наблюдение | `voice-deep.md` §3, SG-11 | диагностика |

Порядок разблокировки (по документам, не наше решение): PL-07 → RE-3 → окно Stream (1 сессия,
свой экран + зритель) → код S-02/S-03; камера — отдельным железом.

## 3. Consent / UI / quality / simulcast / stream key — только подтверждённое

### 3.1 Consent

| Утверждение | Статус | Чем подтверждено |
|-|-|-|
| В эталоне есть системный диалог выбора источника и индикатор записи | `[D]` строки | `sbbPhW`, `ie1mgY` в `ru-catalog.json` |
| Consent-flow эталона (порядок, свои диалоги, «запомнить») | **не снят** | ни скриншота, ни HTML; PL-07 отсутствует |
| Наше правило «захват только по клику, без захвата при монтировании/входе/настройках» | `[A]`, но **согласовано** с VOICE.md §5.1 и с уже принятым тестом запрещённых API | `VOICE.md` §5, `voice-ui.test.mjs:681-698` |
| «Нет второй галочки, которая что-то запоминает» | `[A]` | наше решение; в дампе не наблюдалось |
| Индикатор всегда виден, стоп доступен всегда | `[A]` | наше решение |
| Право канала на показ экрана | `[A]`-трактовка строки `PpfzUE`; имя права/бита не подтверждено | SG-12 |

**Вывод:** подтверждено только существование **текстов** про системный диалог. Сам consent —
наш дизайн; обещать «как в Discord» нельзя.

### 3.2 UI

| Утверждение | Статус |
|-|-|
| В эталоне 4 кнопки ряда: «Камера недоступна» (`fauxDisabled`), «Продемонстрируйте свой экран» (`aria-pressed=false`), «Начать активность», «Открыть звуковую панель» | `[D]` (два HTML-дампа, 61 403/61 414 Б) |
| Экранов Go Live (превью, лист настроек, тайлы, fullscreen, счётчик) в дампе нет | `[D]` (отсутствие), `unmeasured` |
| В нашем UI этих кнопок нет вообще | подтверждено: в `voice.js` нет ни «Камера», ни «экран», ни `data-stream` |
| Геометрия листа/сцены/тайлов/fullscreen | `[T]`+`[A]`, `unmeasured` — паритет не заявлен |
| Единственный измеренный элемент — ряд панели эталона | `[M]`, воспроизведено: `79.2/80.0/80.0`, gap `7.2→8`, шаг `86.4/87.2` |

### 3.3 Качество

Подтверждено (`[L]`/`[S]`): `rid:"100"` — строка, `quality:100` — число; `type` различает
`video`/`screen`; профиль одной машины `1280×720@30`, `2 500 000` (min `150 000`), sink wants
`20 fps`, `maxPixelCount 921 600`, адаптация через 15 с; `experiments:["fixed_keyframe_interval"]`.

Не подтверждено: значения `eq.Y4`/`eq.Cl`; выбор rid/quality **зрителем**; negotiated-потолки у
зрителя; поддержка H265 зрителями; vaapi на этой машине.

Контракт UI «`auto`/`source`, две опции» — `[A]`, и это корректная реакция на дыру: список
«480/720/1080/4K» не снят, хардкодить 720p/2.5 Мбит/20 fps запрещено (правило §11.3).

### 3.4 Simulcast

| Утверждение | Статус |
|-|-|
| Код-путь: второй слой `{rid:"50", quality:eq.Cl}` при `isSimulcastSupported()` **и** `enableSimulcast` | `[S]` (call-site в бандле) |
| Живьём второй слой включался | **нет**; в op0/op2 всегда один слой `rid:"100"` |
| Значение `quality` для `rid:"50"` | `[A]`, в бандле не инлайнится |
| Применимость второго слоя к **screen** | не установлено: в цитируемом описании второй слой имеет `type:VIDEO` |
| Наша реализация | отсутствует; UI-модель `auto/source` simulcast не адресует — это осознанно |

### 3.5 Stream key

| Утверждение | Статус |
|-|-|
| Формат `guild:<guild_id>:<channel_id>:<user_id>` | `[L]` (HTTP-cache, 28 вызовов preview) |
| `GET|POST /streams/{key}/preview` (`{thumbnail}`), `POST /streams/{key}/notify` (`{region}`), `PATCH /streams/{key}/stream` (`{region}`) | `[S]` |
| Какой вызов стартует/останавливает стрим, коды ответов | **не установлено** (`[A]`) |
| Ключ никогда не в UI/логах/`title`/`aria-label` | наше правило `[A]`, согласовано с VOICE.md §5.3 (там же — `session_id`, RTC `token`, `secret_key`) |
| Нужны ли нам `preview`-миниатюры | нет (D-02): сеть + чужие кадры + запрет ассетов |

## 4. Геометрия, состояния, ARIA для будущего UI

### 4.1 Измерено (и почему это не переносится напрямую)

Воспроизведено независимо (`measure.py`, вывод в `/tmp`, golden-файл только читался):

```text
input: /home/sanya/re-deep/voice/ui/01-joined.png size=2560x1383 dpr=1.25
buttons_with_fill=3 widths_css=[79.2, 80.0, 80.0] gap_css=[7.2, 7.2] step_css=[86.4, 87.2]
fill_height_css=26.4  first_fill_x_css=104.0 last_fill_x_css=356.8
RESULT: OK
```

Перенос в наш shell — арифметика (расчёт, не замер):

| | Эталон | Наш shell |
|-|-:|-:|
| Ширина панели | 362.4 CSS `[M]` | **240** (`--layout-tree-w`, VOICE.md D-01) |
| Интерьер (padding 8×2) | 346.4 | 224 |
| 4 равных слота, gap 8 | **80.6** (заливка читается 79.2–80.0) | **50.0** |
| Высота кнопки | 32 `[T]` | 32 `[T]` |
| Gap | 8 `[T]` | 8 `[T]` |
| Радиус | 8 `[T]` | 8 `[T]` |

Фактическая реализация панели (код, `[F]`-зона G2) сейчас другая: `.voice-action` —
`min-width/height: var(--btn-height-sm)` = **28**, `border-radius: var(--radius-xs)` = **4**,
`.voice-actions { gap: var(--space-1) }` = **2**, padding панели `var(--space-2) var(--space-4)` =
4/8. То есть контракт стрима (32/8/8) — это **новый компонент или отклонение**, а не «своих
значений не нужно» (F-04).

### 4.2 `unmeasured` (паритет не заявлен)

Лист `#stream-setup` (`--modal-w-md` 602, pad 24/16, `--surface-2`, `--shadow-popover`,
`--z-modal`, заголовок 20/24), превью (`aspect-ratio: 16/9`, `--radius-xs`, `--surface-input`),
сцена (оверлей `inset:0`, grid `minmax(240px,1fr)`, gap 8, тайл 16/9 `--surface-3`, подпись
`--space-9` 32/12-16, аватар `--avatar-sm` 20, бейдж `--badge-h` 16), fullscreen (fixed, `--z-modal`,
контролы 32+8, авто-скрытие 2500 мс). Всё — только существующие токены (35/35 проверены) плюс
`[A]`; это допустимо для дизайна, но не является паритетом и не может быть приёмкой «по пикселям».

### 4.3 Состояния: аудит матрицы

Матрица `data-stream` (8 состояний) + `data-camera` (4) + `data-view` (3) закрывает основное:
`live` только по событию, `off` после стопа только по `STREAM_DELETE`, идемпотентность
`picking/starting/stopping` — верные инварианты.

Найдено:

1. **Нет переходов в `error` из `picking`.** `permission-channel`, `permission-os`,
   `portal-unavailable`, `os-outdated`, `region-blocked` возникают именно на выборе источника, а
   в таблице §2.1 у `picking` только `→preview` и `→off` (F-06).
2. **Нет `live → error` и `paused → error`.** `encoder-failed` (`e9LMuh`) приходит во время
   стрима; состояния-источника для него нет (F-06).
3. **Состояния зрителя не смоделированы.** `viewer-full` (`VVZDBL`) и `hidden` (`epU4Ec`)
   относятся к просмотру чужого стрима, но `data-stream` описывает только свой; `data-view` знает
   лишь `hidden|stage|fullscreen` (F-06).
4. **`paused` используется, хотя семантика неизвестна** (SG-05). Подпись «Стрим приостановлен»
   может оказаться ложной (например, `paused` = приостановка зрителя) (F-08).
5. **Нет стоп-условия lock/sleep.** G5-план требует останов при блокировке экрана/сне; в STREAM.md
   такого перехода нет, а в `power.rs` нет lock-сигнала (F-10).

### 4.4 ARIA и клавиатура

Верно: `#voice-screen` — `button` с `aria-pressed`/`aria-haspopup="dialog"`; лист —
`role="dialog" aria-modal` + `aria-labelledby`; `#stream-quality` — `aria-haspopup`/
`aria-expanded` + `menuitemradio`/`aria-checked`; `#stream-viewers` — `role="status"`;
индикатор не только цветом; `Esc` закрывает лист/fullscreen, но **не** стопит стрим — верное
разделение.

Найдено:

1. `#stream-tiles` объявлен `role="list"`, а дети — `.stream-tile` с `role="button"`; нужен
   `listitem`-обёртка или `role="group"` (F-07).
2. `#stream-error` объявлен «единственным `role="alert"`», но на том же экране уже есть
   `#voice-error` (`role="alert"`, `voice.js:374`) (F-13).
3. `#stream-error` в DOM-контракте лежит **вне** `#stream-setup[aria-modal="true"]`, а
   `start-failed` возникает, когда лист открыт → часть AT проигнорирует алерт под модалью (F-18).
   Лечить: алерт внутри активного диалога либо не делать лист модальным.
4. `aria-pressed` на `.stream-tile` в fullscreen — это выбор, а не переключатель (F-17).
5. Порядок «сначала текст, потом `hidden=false`» для `role="alert"` не зафиксирован (мелочь, но
   для AT существенно).

## 5. Владение файлами (G2 / G3 / G5, плюс G1 и G4)

| Зона | Реальные файлы | Правила | Риск |
|-|-|-|-|
| **G3 Design** | `waves/RW-40/DESIGN/STREAM.md`, `stream-spec.json`, этот отчёт | только документы; код/тесты/CSS не трогает | нет |
| **G2 UI** | `rust/gui-spike/ui/features/voice.js` (расширение) либо новый `features/stream.js`; `ui/index.html`; `ui/app.js`; `ui/components/voice.css` либо новый `components/stream.css`; `tests/unit/stream-*.test.mjs` (план) | не строит `stream_key`/`token`/`ssrc`/`rid`, не зовёт `getUserMedia`/`getDisplayMedia`, не открывает WS | `index.html`, `app.js`, `style.css`, `tokens.css` — **общие** (правка через TL/интеграцию) |
| **G1 Core/Network** | `rust/crates/discord-voice/src/{lib,udp,media/*}.rs`, `tests/{signaling,stream_contract}.rs`; `discord-core/src/state/voice/*`; IPC — `gui-spike/src/{main,core_bridge,net}.rs` | REST `/streams/*`, `STREAM_*`, второй RTC, rid/quality, RTP; не рисует UI | `main.rs`/`core_bridge.rs` — общие для IPC-команд |
| **G5 Platform** | `rust/crates/discord-platform/src/portal/*` (план G5), `ffi_*.rs`; `discord-media/src/*` — **без FFI** | захват (portal/PipeWire, позже X11/v4l2), энкод, системный звук; не решает продуктовые состояния | `discord-media` и `discord-voice` — `unsafe_code="forbid"`; FFI только в `discord-platform` с `#[allow]`-модулем (D-1, не решён) |
| **G4 QA/Security** | `tests/unit/*`, `tests/meta/*`, `tests/support/gui-dom-harness.mjs`, mutation-скрипты волны | тесты «нет захвата без клика», «ключей в логах нет»; политику не ослабляет | харнесс — общий файл (расширение таймеров — через TL) |

Отдельно: **IPC-контракта на стрим не существует** (`CONTRACTS/` = `dispatch.md`,
`read-state.md`). UI-тесты §8.1 на фейке проверяют только дизайн; чтобы проводка была
проверяема, нужен `CONTRACTS/stream.md` (содержание — G5+G1, правка — TL) — по образцу
`CONTRACTS/read-state.md` (F-12).

## 6. Тест-план (без кода)

### 6.1 Оболочка

- Red-first: тест пишется и падает; в отчёте — команда и лог red, затем green (`TEST-POLICY`).
- Оракулы: `stream-spec.json` — для UI-контракта (прецедент: `voice-ui.test.mjs` использует
  `voice-spec.json`); `ru-catalog.json`, `tokens.css`, golden-фикстуры — для остального.
  Дизайн-контракт — **наш** артефакт, поэтому это регрессия по контракту, а не конформанс;
  конформансом считаются только фикстуры/дампы.
- Запрещено в тестах: `fetch`, `WebSocket`, `navigator.mediaDevices`, `getUserMedia`,
  `getDisplayMedia`, `RTCPeerConnection`, реальные таймеры.
- Среда: `tests/support/gui-dom-harness.mjs` (фейковые таймеры записываются, `rAF` — очередь).
- Гейт: `node --test 'tests/unit/**/*.test.mjs'` — новые файлы подхватываются автоматически
  (`scripts/gate.mjs:242`), править `package.json` не нужно.
- Уровни (`TEST-PLAN` §1.0): S-02 = интеграция → L0–L3 + L6; UI/платформа дополнительно L7.

### 6.2 G2 — UI на фейковом сервисе

| Группа | Плановые ID (из §8.1) | Оракул | Предусловие |
|-|-|-|-|
| Вход/лист/превью/старт/стоп | SL-01…SL-16 | `stream-spec.json` (`states`, `copy`, `dom`) | фейк `stream{pickSource,start,stop,setQuality,onEvent}` — **его нет**, пишется вместе с UI |
| Тайлы/сцена/fullscreen | ST-01…ST-06 | там же | фейковые события `STREAM_*` |
| Камера | SC-01…SC-06 | там же | фейк `camera{preview,enable,disable,devices}` |
| Ошибки | SE-01…SE-04 | `copy` + `errors[].reason` | фейк-события ошибок |
| A11y | SA-01…SA-04 | `dom.aria` + ручной чек `role=list` | — |

Дополнительно к §8.1 (наши предложения):

- **SL-17** — `picking → error` для каждого из пяти кодов (`permission-channel`, `permission-os`,
  `portal-unavailable`, `os-outdated`, `region-blocked`): состояние `error`, `data-reason`, текст из
  `copy`, фокус в регион ошибки.
- **SL-18** — `live → error` по `encoder-failed (e9LMuh)`: стрим-состояние `error`, источник
  освобождён (счётчик захватов = 0), автоповтора нет.
- **ST-07** — тайл-зритель: `viewer-full` не ломает сцену, остальные тайлы живы.
- **SA-05** — структура списка: каждый ребёнок `#stream-tiles` имеет `listitem` (или роль
  изменена на `group`) — иначе тест красный.
- **SA-06** — `#stream-error` достигаем AT при открытом модальном листе (внутри диалога либо лист
  не `aria-modal`).

### 6.3 G4 — приватность и consent

Расширять существующий механизм, а не дублировать: сейчас запрещённый список проверяется только
по `voice.js` (`voice-ui.test.mjs:681-698`). Нужен мета-тест по **`ui/**/*.js`**, иначе код стрима
в новом файле пройдёт мимо (F-12/план):

| ID | Проверка | Оракул |
|-|-|-|
| SG-01/02/03 | 0 вызовов захвата при монтировании; нет `mediaDevices`; захват только из обработчиков кнопок | grep + fake-счётчик |
| SG-04 | нет `saveClipEx`, `setClipsRecordingEnabled`, `startLocalAudioRecording`, `startRecordingRawSamples`, `setAecDump`, `saveScreenshot` | grep |
| SG-05 | нет `getScreenPreviews`/`getWindowPreviews` | grep |
| SG-06 | `stream_key`, `token`, `endpoint`, `secret_key` отсутствуют в DOM/атрибутах/логах | grep + сериализованный DOM |
| SG-07 | после `stop`/`error` 0 активных захватов | фейк-счётчик |
| SG-08 | нет авто-рестарта после `error` | фейк-журнал вызовов |
| SG-09 | отмена пикера тихая: 0 ошибок, 0 стримов | фейк |
| SG-10/11 | зрители — только число; 0 запросов `/streams/{key}/preview` | фейк + grep |
| SG-12 | без права — 0 вызовов сервиса, 0 захвата | фейк |

### 6.4 G1/G5 — Rust-контракты

| План | Уровень | Оракул | Чего не хватает |
|-|-|-|-|
| `stream_contract.rs`: разбор/сериализация `STREAM_*`, `stream_key` не логируется, `viewer_ids` как `Option<Vec>` | L1–L2 | схема событий из `NETWORK-CONTRACTS` §3.3 — **документ**, не захват | живых JSON нет: это контракт-по-коду, не конформанс |
| `video_rtp_contract.rs`: PT 101/102, rid/quality, rtx | L2 | эталонные байты (T-MEDIA-03) | **фикстуры видео-RTP нет** (F-15) |
| энкод: keyframe/bitrate/размер кадра | L1 | бенчмарк (T-MEDIA-02) | нет базы/порогов; решать PM |
| второй RTC: endpoint/token из `STREAM_SERVER_UPDATE` | L3 | стаб RTC (T-MEDIA-04) | контракт стаба не определён |
| portal/PipeWire: сессия, отмена, 0 потоков | L3 | приватная шина + python-мок (`G5-VOICE-PLAN` шаг 1) | делается в G5-плане, не здесь |
| камера | L1 | fake-бэкенд/property | железо отсутствует (B-1) |
| зритель: N потоков без просадки | L3 | стаб (T-MEDIA-08) | только live |

### 6.5 Тайминги (ограничение харнесса)

`flushTimers()` (`gui-dom-harness.mjs:1309-1315`) выполняет **все** таймеры, игнорируя `delay`.
Поэтому «15 с» и «10 с» и «2500 мс» неразличимы. Варианты (выбрать G4+TL):
(а) тест читает `dom.timers.get(id).delay` и сверяет `15000/10000/2500` — без правки харнесса;
(б) добавить `advance(ms)` в харнесс (общий файл, правка через TL).

### 6.6 Что нельзя проверить офлайн

`STREAM_CREATE`/`DELETE` живьём, выбор источника системным диалогом, приход кадра, второй RTC,
согласование AV1, декод/звук зрителя, права канала, поведение при отзыве согласия в ОС,
lock/sleep-стоп, системный звук, камеру.

## 7. RE-вопросы (RE-3 и окно Stream)

Формат evidence для всех: санитизированный JSONL WS/REST (ключи/токены/`secret_key` вырезаны,
чужие данные не пишутся), окно согласует PM, участие второго клиента — по согласию.

**A. Старт/стоп и REST**

1. Какой запрос порождает `STREAM_CREATE`: `POST /streams/{key}/notify`, `PATCH /streams/{key}/stream`
   или иной? Полная последовательность с таймстампами.
2. Что приходит раньше — `STREAM_CREATE` или второе RTC-подключение; что триггерит
   `STREAM_SERVER_UPDATE`.
3. Какой запрос останавливает стрим и что в `STREAM_DELETE.reason` при: своём стопе, стопе из
   системного UI, ошибке энкодера, дисконнекте.
4. Коды/тела ответов `notify`/`stream`/`preview` (2xx/4xx/429, `retry_after`), лимиты.

**B. Семантика событий**

5. `viewer_ids` — полный снимок или дельта? Снять 0→1→2→0.
6. `paused` — пауза стримера, зрителя или серверная? Что при этом видит стример и зритель.
7. Эмитится ли `STREAM_UPDATE` на join/leave зрителя и на смену региона.
8. Смысл `rtc_server_id`/`rtc_channel_id` (это же endpoint второго RTC?).

**C. Качество/rid/simulcast**

9. Числовые значения `eq.Y4` и `eq.Cl` (камера/экран/второй слой).
10. Получает ли screen share второй слой (в источнике второй слой `type:VIDEO`).
11. Что зритель кладёт в свой `Identify.streams[]` и в op15 sink wants для экранного стрима;
    какие `maxResolution`/`maxBitrate` ему приходят.
12. Влияет ли `fixed_keyframe_interval` на GOP и какой интервал.

**D. Транспорт/wire**

13. Тот же SRTP для видео (B-08)? PT 101 внутри того же кадра `[ct][tag16][ctr4]`; RTCP для видео
    шлётся/читается?
14. Второе RTC-подключение делает свой UDP discovery, keepalive 5 с и ротацию +60 с?
15. Применяется ли DAVE к видео-кадрам (per-codec, по `NETWORK-CONTRACTS`) и обязателен ли он для
    видимости стрима.

**E. Захват/ОС**

16. Wayland: точный поток портала (`SourceTypes`, cursor mode, `restore_token`, персистентность),
    привязка диалога (`parent_window`/xdg-foreign), поведение при отмене.
17. X11 (сессия этого хоста): предлагает ли эталон шаринг вообще, каким путём, есть ли пикер.
18. Камера: форма `self_video` в gateway-join, `streams[].active` при включении, payload списка
    устройств, возможен ли стрим+камера одновременно.
19. Системный звук на Linux: есть ли путь (PipeWire monitor) или это фактически win32-only.

**F. Права и ошибки**

20. Какой бит/право гейтит стрим и камеру; что возвращает сервер при отказе; как из этого
    выводятся `canStream`/`canVideo`.
21. Причина `codec survey is not available` (когда и на что влияет).
22. Зачем 28 вызовов `GET /streams/{key}/preview`: каденция, размер тела, нужен ли этот аплоад
    вообще (у нас D-02 — нет).

## 8. Находки

| ID | P | Где | Суть | Кому |
|-|-|-|-|-|
| F-01 | **P1** | STREAM.md §2.4 vs `G5-VOICE-PLAN` §7.4 п.8 | UI-контракт требует локальные кадры в превью; G5-план прямо запрещает кадры в рендерер/IPC в MVP. Либо превью через отдельный нативный слой/окно, либо MVP без превью и другой consent-дизайн | PM + G2 + G5 |
| F-02 | **P1** | STREAM.md §5.3 vs `G5-VOICE-PLAN` §7.3/D-2 | Контракт фиксирует камеру через v4l2, тогда как portal-vs-v4l2 — нерешённое противоречие наших же решений; камеры на хосте нет | PM (D-2) |
| F-03 | **P1** | STREAM.md §1/§10/`live_blockers` | B-08 (KDF/nonce/AAD, 14-байтная соль) не указан, хотя второй RTC и зритель идут по тому же SRTP → wire-путь стрима заблокирован сильнее, чем «нет событий» | G3 (док) + G1/G4b |
| F-04 | **P1** | STREAM.md §4.1 vs `components/voice.css` | «Своих значений не нужно (32/gap 8/pad 8)» неверно: в панели 240 кнопки будут ~50, а текущий код использует 28/2/4. Нужен новый компонент или явное отклонение | G2 + G3 |
| F-05 | P2 | STREAM.md §1 табл., §2.9 | «`self_video` в op4» смешивает gateway op4 (voice join) и RTC op4 (Session Description). В RTC op4 `self_video` нет | G3 |
| F-06 | P2 | STREAM.md §2.1 | Нет переходов `picking→error`, `live→error`, `paused→error`; у 5 из 14 кодов ошибок нет состояния-источника; состояния зрителя (`viewer-full`, `hidden`) не смоделированы | G3 |
| F-07 | P2 | STREAM.md §4.8/§7 | `#stream-tiles` = `role="list"`, дети — `role="button"` без `listitem` | G3 + G2 |
| F-08 | P2 | STREAM.md §2.7 | UI-состояние `paused` строится на неизвестной семантике (SG-05) — риск ложной подписи «Стрим приостановлен». Гейтить до RE-3 | G3 + PM |
| F-09 | P2 | STREAM.md §2.5 | Simulcast-слой описан без оговорки, что в источнике он `type:VIDEO`; применимость к screen не установлена | G3 |
| F-10 | P2 | STREAM.md §2.2/§2.6 vs `G5-VOICE-PLAN` B-9 | Нет стоп-условия при блокировке экрана/сне; в `power.rs` нет lock-сигнала → индикатор может врать | G3 + G5 |
| F-11 | P2 | STREAM.md §2.11/§1 vs `voice.js` | Кнопок камеры/экрана в нашем UI нет вообще; «disabled-заглушки» (VOICE.md D-02) не соответствуют коду | G2 + G3 |
| F-12 | P2 | STREAM.md §8.1 vs `CONTRACTS/` | Сервиса `stream.*`/`camera.*` и IPC-контракта нет; UI-тесты на фейке проверяют только дизайн. Нужен `CONTRACTS/stream.md` | G5 + G1 + TL |
| F-13 | P3 | STREAM.md §2.10/§4.8 | «Единственный `role="alert"`» неверно: на том же экране `#voice-error` | G3 |
| F-14 | P3 | §8.1 SL-09/SL-10 vs харнесс | `flushTimers()` игнорирует `delay` — таймауты 15/10/2500 мс не проверяемы без assert по `delay` или `advance(ms)` | G4 + TL |
| F-15 | P3 | §8.3 SP-05/SP-06 | Нет видео-RTP фикстуры и бенчмарк-базы: оракулы T-MEDIA-02/03 не существуют | G5 + G4 |
| F-16 | P3 | STREAM.md §12 | Проверка «код не менялся» через `git status` слабая: `waves/RW-40` целиком untracked, состояние файлов так не доказывается | G3 |
| F-17 | P3 | STREAM.md §4.8 | `aria-pressed` на тайле в fullscreen — это выбор тайла, а не переключатель | G3 |
| F-18 | P2 | STREAM.md §7 | `#stream-error` вне `aria-modal`-листа: `start-failed` при открытом листе может не объявиться | G3 + G2 |

## 9. Что подтверждено независимо (и чем)

| Проверка | Команда/метод | Результат |
|-|-|-|
| Валидатор дизайна | `python3 waves/RW-40/PM/s-design/validate.py --log /tmp/opencode/g3-stream-validate.log` | `checks=23 failures=0`, `RESULT: OK` (лог в `/tmp`, репозиторий не тронут) |
| Замер ряда | `python3 …/measure.py --log /tmp/opencode/g3-stream-measure.log` | `6/6`, значения совпали с отчётом волны |
| Токены | 54 имени из STREAM.md/VOICE.md против `ui/tokens.css` | все существуют |
| Копирайт | валидатор против `/home/sanya/re-deep/ru-catalog.json` | 54/54 |
| `[L]`-факты | сверка с `docs/re/voice-deep.md:23-52,91-111` и `NETWORK-CONTRACTS` §2.3–2.4 | `video:true`, `rid:"100"`, `quality:100`, `active:false`, AV1 PT 101/102, priorities 1000/2000/3000/4000, ssrc N/N+1/N+2 — совпадают |
| `[D]`-факты | `docs/re/native-re.md:272-286,338-362` | портал `ScreenCast`/`Session`/`Request`, `pw_stream_*`, `BaseCapturerPipeWire`, win32-only список — совпадают |
| Отсутствие кода | grep-счётчики: `STREAM_*`, `stream_key`, picker, portal, pw_stream, v4l2, vaapi | 0 совпадений; `discord-media` никем не используется |
| Состояние UI | `voice.js` (710), `index.html:83`, `components/voice.css` | кнопок/атрибутов стрима нет; геометрия 28/2/4 |
| Хост | `G5-VOICE-PLAN` §1 | камеры нет; портал ScreenCast есть (`AvailableSourceTypes=7`) |

## 10. Ограничения этого ревью

- Окно Discord, live-сессии, сеть, `cargo`, `npm run gate:*` — **не запускались**; live-строк нет.
- Golden-каталог `/home/sanya/re-deep` только читался; отчёты валидатора писались в `/tmp`.
- Ревью проверяет **согласованность** документа с кодом/RE и полноту блокеров; оно не доказывает
  работоспособность Go Live (и не должно — до RE-3 это невозможно).
- Оценки «что будет весить/как выглядеть» в нашем shell не делались: замеров нашего UI стрима нет.
- Изменён только этот файл; код, тесты, `waves/*` (кроме нового отчёта) не тронуты.

## 11. Предлагаемая очередь (для PM)

1. Решение по F-01 (превью) и F-02 (камера: portal или v4l2) — они меняют объём S-01/S-02.
2. Внести B-08 и B-9 в `live_blockers` STREAM.md (F-03/F-10), уточнить F-05/F-06/F-09.
3. PL-07 (consent) — до S-02; затем RE-3 с протоколом из §7 (стример + 1 зритель).
4. `CONTRACTS/stream.md` (G5+G1) — до первых UI-тестов проводки.
5. G2: решить судьбу ряда кнопок (F-04) и начать с нуля разметки (F-11), не «включая заглушки».
6. G4: мета-тест запрещённых API по `ui/**/*.js` и решение по `advance(ms)` (F-14).
