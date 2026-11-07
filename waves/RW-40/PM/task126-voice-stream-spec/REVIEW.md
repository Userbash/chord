# Независимое ревью — task-126 (VOICE-STREAM-UI-SPEC)

**Вердикт: RETURN.** P0 = 0, P1 = 2, P2 = 9, P3 = 3.

- **Карточка:** `task-126`, Review 49A (слот Dev-1, DeepSeek).
- **Ветка:** `design/task-126-voice-stream-ui-spec`, HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`.
- **Оракулы:** `VOICE-STREAM-UI-SPEC.md` (`md5 44daa51a3c8bf953a428edebf9f67bf8`, 1438 строк),
  `REPORT.md`, `rust/crates/discord-core/src/stream.rs`, `rust/crates/discord-voice/src/session.rs`,
  `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md`, `stream-opcodes-fixtures.json`,
  `rust/gui-spike/ui/tokens.css`, `TEST-POLICY.md` §13, `/home/sanya/re-deep/{voice-deep,frontend-d03}.md`.
- **Режим:** read-only. Записан только этот файл. Продуктовый код, тесты и гейт не менялись.

Код-оракул в ветке отсутствует: `rust/crates/discord-core/src/stream.rs`, `session.rs`,
`STREAM-GATEWAY-CONTRACT.md`, `stream_gateway_contract.rs` и `session_contract.rs` есть в рабочем дереве
`/home/sanya/orca/project` (main, dirty), но не в `design/task-126`. Поэтому сверка кода шла по main;
ссылки спеки (`stream.rs:423`, `session.rs:1333` и т. д.) указывают именно туда и совпадают.

---

## 1. Что проверено и воспроизведено

| Проверка | Команда | Результат |
|---|---|---|
| CSS-блок | `awk '/^```css$/{f=1;next} /^```$/{if(f){f=0}} f' … > /tmp/vs.css && wc -l` | 788 строк — совпадает с отчётом |
| Сырые цвета / `!important` / `:has()` / fallback | `grep -cE '#[0-9a-fA-F]{3,8}\|rgba?\(\|hsla?\(\|!important\|:has\(\|var\(--[a-zA-Z0-9_-]+,'` | 0 |
| Биекция токенов | `comm -23` used vs declared в `tokens.css` | 0 необъявленных; 89 уникальных `var(--*)` |
| Контраст WCAG | пересчёт `L`/`CR` по значениям `tokens.css` | все 30 пар сошлись с таблицей §3.2 (расхождение только в округлении 4.30 vs 4.29) |
| 9 измерений | сверка с `TEST-POLICY.md` §13 (главное дерево, 174 строки) | названия и порядок D1…D9 совпадают |
| Опкоды 18…22 | `stream.rs:360-450`, `session.rs:290-460` | имена, числовые коды, поля `d`, `guild_id: null` для call, `paused: bool` — совпадают с §1.1 |
| Гейт сессии | `stream.rs:579-596`, `session.rs:1309-1313` | `StreamError::SessionNotEstablished` / `SessionError::StreamNotEstablished` — как в §1.1 |
| Приоритет `STREAM_DELETE` | `stream.rs:1579-1636`, `session.rs:1558-1636` | все 6 правил `A-REDUCER-DELETE` совпадают с §1.2 |
| Ключ и границы | `stream.rs:21-24, 160-220` | `MAX_KEY_BYTES=128`, `MAX_REGION_BYTES=64`, 68 Б канонический guild — как в §6/D2 |
| Маскирование | `StreamKey` `Debug`/`Display`, `StreamSecret` | `<redacted>` для guild/token; §6/D9 выполним |
| Запрещённые API в UI | `grep` по `rust/gui-spike/ui/` | 0 вхождений `getDisplayMedia`, `getUserMedia`, `presentNativeScreenSharePicker` и т. п. |

`/home/sanya/re-deep/voice-deep.md` и `frontend-d03.md` не противоречат спеке: опкодов `18..=22` в них нет
(там RTC-опкоды 0…16 и UI-дамп), поэтому протокольная часть опирается на контракт и код, а не на дамп.
Это корректно оговорено в §0.1 и §0.2.

---

## 2. P1 — блокирует приёмку

### P1-1. `REPORT.md` описывает другой артефакт, а не этот файл

Отчёт и спека имеют одинаковый md5, значит отчёт заявлен про эту ревизию. Фактически пять ключевых
утверждений отчёта в спеке отсутствуют:

| `REPORT.md` | Заявлено | В спеке |
|---|---|---|
| строка 21 | «матрица 4-значных кодов закрытия (`4001..4022`, `VoiceCloseCode`)» | `grep -c '4001\|VoiceCloseCode\|4022'` → **0** |
| строки 26–30 | `#stream-stage-header`, `#stream-viewer-pill`, `#stream-watch-leave`, `#stream-quality-btn`, `#stream-quality-popover`, `#stream-picker-sheet`, `#stream-source-tabs`, `#stream-source-grid`, `#stream-audio-toggle`, `#stream-quality-preset-group`, `#stream-region-select`, `#stream-start-confirm`, `#stream-fullscreen-overlay`, `#stream-fullscreen-pause-toggle` | ни одного такого **id**; `stream-quality-btn` есть только как класс, остальные имена в спеке не встречаются. Спека использует `#stream-stage-bar`, `#stream-quality`, `#stream-quality-menu`, `#stream-setup`, `#stream-leave-watch`, `#stream-fullscreen`, `#stream-fullscreen-pause` |
| строка 39 | «10 UI-состояний … `stopping`, `disabled`» | §4 — 8 строк; строк `stopping` и `disabled` нет |
| строка 40 | «детерминированная таблица переходов (Текущее × Событие → Новое × Эффект)» | §4 (строки 1328–1341) — только таблица видимости, переходов нет |
| строки 47, 78 | «41 тест-оракул `T-VS-01..T-VS-41`, ≥ 3 на измерение» | `grep -c 'T-VS-'` → **0**; в §6 ровно 9 оракулов `T-STREAM-UI-*-01`, по одному на измерение |

Это нарушает TEST-POLICY §4 (доказательство = команда → exit code → артефакт) и §2.5: координатор и
имплементор получают отчёт, по которому нельзя воспроизвести ни DOM, ни покрытие. До исправления отчёт
нельзя прикладывать к приёмке, даже если сама спека станет верной.

### P1-2. Контролы зрителя отправляют owner-only опкоды

Спека прямо предписывает зрителю `op 19` и `op 22`:

- `VOICE-STREAM-UI-SPEC.md:405`: `<!-- Кнопка выхода из просмотра чужого стрима (op 19 STREAM_DELETE / LocalStreamAction::Close) -->`;
- `:1334`: «`#stream-leave-watch` … (`LocalStreamAction::Close` / `op 19`)»;
- `:464`: `#stream-fullscreen-pause` с `data-stream-action="toggle-pause"` в баре просмотра.

В коде это не работает. `Session::watch_stream` (`session.rs:1423-1431`) только строит команду и **не**
привязывает `active_stream_key`/`stream_phase`. Поэтому `stop_screen_share` (`:1414-1420`) и
`set_stream_paused` (`:1443-1451`) для зрителя вернут `SessionError::NoActiveStream`, а не закроют просмотр.
`live_stream_key()` (`:1276-1288`) отдаёт только ключ собственного стрима сессии.

Выход зрителя — это `LocalStreamAction::Close` в `discord-core` (`stream.rs:1483-1490`: удаление записи +
`CloseStreamRtc` при открытом транспорте), без опкода. `op 22` в полноэкранном баре должен быть либо скрыт
для зрителя, либо удалён из контракта. Сейчас два основных пути просмотра («выйти» и «пауза») описаны
неисполнимо.

---

## 3. P2 — исправить до повторного ревью

### P2-1. Неверный md5 `tokens.css` в нормативных источниках

`VOICE-STREAM-UI-SPEC.md:20` заявляет `md5: 25f8fb33eb8bc4637ff20ef55b35785d`. Фактически:
`rust/gui-spike/ui/tokens.css` → `21d570a6bf9c4b5d77b00d3d9b3d1d08`, `waves/RW-39/DESIGN/tokens.css` →
`7ce9c1f03e26056243e13df3237b0a9d`. Ни один из них не равен заявленному. Содержимое при этом сходится
(89/89 токенов), поэтому это провенанс, а не подмена: поправить хеш и указать, что копия в `ui/`
отличается от DESIGN тремя текстовыми правками (шапка, DP-01, DP-02).

### P2-2. `CloseStreamRtc` при повторном `STREAM_CREATE`: в редьюсере его нет

`:80` утверждает: «Если прежний `transport_ready == true`, эмитится `CloseStreamRtc`». В
`StreamState::apply` ветка `StreamDispatch::Created` (`stream.rs:1504-1530`) молча перезаписывает запись и
возвращает `Vec::new()`; эффект есть только в `VoiceSession::on_stream_dispatch` (`session.rs:1488-1496`).
Колонка §1.2 смешивает оба слоя. Либо пометить отличие (`[S]` для сессии, `[A]` для редьюсера), либо
завести задачу G1 на выравнивание — иначе T-STREAM-UI-REC-01 проверит не то поведение, которое есть.

### P2-3. `StreamAction::Ready` закрывает не «все открытые потоки»

`:1403` (D7): «все открытые потоки эмитируют `CloseStreamRtc`». Код (`stream.rs:1444-1455`) эмитит эффект
только для `transport_ready == true`. Стрим в `Active` без `STREAM_SERVER_UPDATE` не даст эффекта, и
оракул «все» покраснеет на корректном коде. Формулировка: «все потоки с открытым транспортом».

### P2-4. Словарь `data-reason` не связан с кодами ошибок

`:497` объявляет источником `StreamError::code()`, но в списке значений — `invalid-stream-key`,
`invalid-region`, `session-not-established`, а `code()` возвращает `stream.key.invalid`,
`stream.field.region`, `stream.session.not_established` (`stream.rs:1321-1338`). `permission-channel` и
`safety-rate-limited` приходят из `ui_data_reason()` (`stream.rs:886-897`), `start-failed` /
`encoder-failed` — вообще UI-only. Нужна явная таблица «Rust-код → `data-reason`»; сейчас имплементор
либо положит в DOM Rust-код (не из списка), либо придумает маппинг сам, и тест D3 разойдётся со спекой.

### P2-5. Сцена объявлена оверлеем, а CSS кладёт её в поток

`:99` — «оверлей над областью чата по `D-01` (`STREAM.md` §4.6)», монтаж внутрь `#screen-chat`. В
`STREAM.md:483` — «`inset: 0` внутри `main`». Нормативный CSS (`:834-846`) — `.stream-stage { position:
relative; flex: 1 1 auto; }`, без `position: absolute/fixed` и `inset`. В потоке `#screen-chat` сцена
сожмёт чат, а не накроет его. Нужно выбрать одно: overlay (`position:absolute; inset:0`) или отдельный
экран — и синхронизировать §2.1, §3.1 и §4.

### P2-6. «Полный реестр» DOM не полон, `idle` приписан `StreamPhase`

`:481` назван полным, но в нём нет `#stream-quality`, `#stream-quality-menu`, `#stream-quality-value`,
`#stream-start`, `#stream-cancel`, `#stream-timer`, `#stream-viewer-canvas`, `#stream-viewer-controls`,
`#stream-fullscreen-bar`, `#stream-error-text` и др. Отдельно `:489` объявляет источником
`StreamPhase::as_lowercase()`, хотя значение `"idle"` этот метод не возвращает (`stream.rs:1076-1090`).
Либо переименовать раздел в «ключевые атрибуты», либо добить перечень и пометить `idle` как UI-only.

### P2-7. Оракул геометрии F-04 проверяет контейнер, а не кнопки

`:58`, `:124` и `:1404` приписывают `.voice-actions` «28px / padding 0 4px / radius 4px». В
`voice.css:245-248` у контейнера только `display:flex; flex-wrap:wrap; gap: var(--space-1)`. 28px
(`--btn-height-sm`), `padding: 0 var(--space-2)` и `border-radius: var(--radius-xs)` — у `.voice-action`
(`voice.css:108-119`). Тест D8, написанный по этой формулировке, получит `height: 28px`, но `padding: 0`
и `radius: 0` у контейнера. Нужно проверять `.voice-action`, а не `.voice-actions`.

### P2-8. В ветке нет ни одного нормативного источника кода

`§0.1` ссылается на `stream.rs`, `session.rs`, `STREAM-GATEWAY-CONTRACT.md`, `stream-opcodes-fixtures.json`,
`stream_gateway_contract.rs`, `session_contract.rs`; в `design/task-126` их нет (main HEAD тот же, файлы
лежат незакоммиченными в рабочем дереве main). Метки `[S]`/`[F]` в ветке невоспроизводимы, а `TEST-POLICY.md`
в ветке — старая версия на 10 разделов без §13. До мержа design-ветки на актуальный main спека не
самодостаточна; в отчёте это не оговорено.

### P2-9. §4 не содержит состояний `stopping` и `disabled`

`data-stream="stopping"` есть в §0.3, §1.1 и CSS (`:539-541`), но в матрице §4 строки нет: не описано,
что видно и что заблокировано между `op 19` и `STREAM_DELETE`. Состояние «нет права `STREAM`» свёрнуто в
строку 6 без отдельных правил для `picking/preview`. Отчёт при этом заявляет оба состояния описанными.

---

## 4. P3 — не блокирует

1. `:105` — «всех 5 узлов контракта», а размечено 7 секций (`#voice-panel`, `#stream-setup`,
   `#stream-stage`, `#stream-viewer-overlay`, `#stream-empty`, `#stream-error`, `#stream-fullscreen`).
2. `#stream-stage[data-view]` допускает `"fullscreen"`, но ни CSS, ни §4 это значение не используют
   (полный экран живёт в отдельном `#stream-fullscreen`). Убрать из перечисления или описать.
3. Фокус-трап `#stream-setup` (`:1375`) перечисляет `#stream-quality → #stream-cancel → #stream-start`,
   пропуская пункты открытого `#stream-quality-menu`; при открытом меню `Tab` уйдёт не туда.

---

## 5. Что сделано верно (переделывать не нужно)

- Таблица опкодов §1.1 точно соответствует `StreamGatewayCommand::to_json` (`session.rs:370-420`) и
  `StreamCommand::to_etf` (`stream.rs:517-563`): `type`, строковые `guild_id`/`channel_id`,
  `guild_id: null` для call (D-27-2), `preferred_region: null`, `paused` строгим bool.
- Неоптимистичность старта/стопа/паузы совпадает с `LocalStreamAction` и `Session`: `StartRequested`,
  `StopRequested`, `SetPausedRequested` фазу не меняют (`stream.rs:1457-1489`); `VoiceSession` при старте
  ставит `Connecting` и привязывает ключ (`session.rs:1362-1366`) — спека это честно фиксирует.
- Приоритет `STREAM_DELETE` (unavailable → unauthorized → safety → stream_full → Failed+user_requested →
  ended) совпадает в обоих слоях и в §1.2, включая условный `CloseStreamRtc`.
- `STREAM_SERVER_UPDATE` не меняет фазу, ставит `transport_ready`, считает orphan — совпадает.
- `viewer_ids: None ≠ Some([])` (D-27-5) и скрытие `#stream-viewers` при `None` — совпадает с кодом.
- CSS-контракт §3 действительно проходит механические проверки: 788 строк, 89 токенов, 0 сырых цветов,
  0 `!important`, 0 `:has()`, 0 fallback. Контраст пересчитан независимо — таблица §3.2 верна.
- §6 покрывает все 9 измерений `TEST-POLICY.md` §13 по одному оракулу на измерение; набор из 6 мутантов
  §7 перекрывает обязательный порог `5/5 KILLED`.

---

## 6. Что нужно для PASS

1. Переписать `REPORT.md` под фактический артефакт: реальные ID, реальное число оракулов (9), убрать
   матрицу close-кодов и «таблицу переходов», либо добавить их в спеку.
2. Развести роли владельца и зрителя в §2.2.3/§4: `#stream-leave-watch` → только `LocalStreamAction::Close`
   (без `op 19`); `#stream-fullscreen-pause` — скрыт для зрителя или удалён.
3. Исправить md5 `tokens.css`; синхронизировать `TEST-POLICY.md` ветки с main (появится §13).
4. Уточнить `CloseStreamRtc` для `STREAM_CREATE` (слой `StreamState` vs `VoiceSession`) и для
   `StreamAction::Ready` (только `transport_ready`).
5. Добавить таблицу «`StreamError::code()` → `data-reason`» и строки `stopping`/`disabled` в §4.
6. Согласовать §2.1/§3.1 по сцене: overlay (`inset:0`) или отдельный экран.
7. Починить оракул F-04 на `.voice-action`; дополнить реестр DOM ID.

После правок достаточно повторной сверки §1.2, §2.2.3, §4, §6 и отчёта — остальные разделы менять не
нужно.
