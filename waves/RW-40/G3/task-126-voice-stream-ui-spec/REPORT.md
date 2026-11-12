# Отчёт воркера — `task-126` (G3 Design, Pass 2: правки по REVIEW 49A)

- **Карточка / Задача**: `task-126` (`task_e0f63ee49af7`, dispatch `ctx_5a5a38b3bd39`)
- **Роль**: G3 Design Specialist (слот Dev-1, DeepSeek)
- **Ветка**: `design/task-126-voice-stream-ui-spec` (базовый `HEAD`: `4e240481e59ff4d49732ac8bd5ba08a006363748`)
- **Дата**: `2026-10-30`
- **Статус**: `DONE` (design-only; продуктовый код, тесты и гейт не менялись)
- **Артефакт**: `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` — 1549 строк, `md5 48a0ea7daff38341a3a38554bd59282c`

---

## 1. Что сделано

Закрыты оба замечания P1 и все девять P2 из `waves/RW-40/PM/task126-voice-stream-spec/REVIEW.md` (вердикт был RETURN, P0 = 0). P3 закрыты попутно, потому что правки лежали в тех же абзацах. Спека осталась design-контрактом: код, тесты и гейт не трогались.

### 1.1. Закрытие замечаний

| № | Замечание | Что изменено | Где |
|---|---|---|---|
| **P1-1** | `REPORT.md` описывал другой артефакт | Отчёт переписан под фактический файл: реальные ID (54), 9 оракулов `T-STREAM-UI-*-01` вместо прежнего заявления о 41 оракуле, убраны матрица 4-значных кодов закрытия вебсокета и «таблица переходов», которой в спеке нет. Все числа ниже сняты командами из §3 | этот файл, §5 |
| **P1-2** | Контролы зрителя отправляли owner-only опкоды | `#stream-leave-watch` → только `LocalStreamAction::Close` (удаление записи + `CloseStreamRtc` при открытом транспорте), без `op 19`. `#stream-fullscreen-pause` удалён из разметки: `op 22` требует `live_stream_key()`, а полный экран открыт для чужого стрима. `#stream-ping-retry` перенесён из оверлея зрителя в панель сцены владельца: `ping_stream()` тоже идёт по `live_stream_key()` и у зрителя даёт `NoActiveStream`. Роли разведены в §1.1 (строки 19/21/22), §2.2.3 (вводный абзац + разметка), §4 (2b, 3, 5a), §5.2 | §1.1, §2.2.3, §4, §5.2 |
| **P2-1** | Неверный md5 `tokens.css` | Указан `21d570a6bf9c4b5d77b00d3d9b3d1d08` (`ui/`), для `waves/RW-39/DESIGN/tokens.css` — `7ce9c1f03e26056243e13df3237b0a9d`. Перечислены пять фактических расхождений, а не три: шапка вендоринга, `--positive-fact` (в DESIGN отдельно, в `ui/` свёрнут в комментарий), комментарии раздела 10, комментарий `compact`, блок `@media (max-width: 959px)` (DP-01) | §0.1 п. 1 |
| **P2-2** | `CloseStreamRtc` при повторном `STREAM_CREATE` | Колонка §1.2 разделена по слоям: `StreamState::apply` (`Created`) возвращает `Vec::new()` `[A]`; `VoiceSession::on_stream_dispatch` при `stream_transport_ready == true` эмитит `StreamSessionEffect::CloseStreamRtc` `[S]`. Оракул D7 проверяет слои раздельно | спека §1.2, §6 (D7) |
| **P2-3** | `StreamAction::Ready` закрывал «все потоки» | Формулировка исправлена на «только потоки с `transport_ready == true`»; отдельно сказано, что поток в `Active` без `STREAM_SERVER_UPDATE` эффекта не даёт (`stream.rs:1444-1455`) | спека §6 (D7) |
| **P2-4** | `data-reason` не связан с кодами ошибок | Новый §2.4: три источника и явный маппинг — `ui_data_reason()` (permission-channel / safety-rate-limited / viewer-full), `StreamError::code()` (11 строк, включая правило для `stream.field.id` по точке вызова), UI-only (7 значений). В словарь добавлены `invalid-command` и `invalid-paused`; Rust-коды в DOM не попадают | §2.3, §2.4, §4 (5b) |
| **P2-5** | Сцена объявлена оверлеем, CSS клал её в поток | Выбран оверлей: `.stream-stage` — `position: absolute; inset: 0` внутри `main.content`; сцена монтируется сиблингом `#chat-header`/`#screen-chat` и не сжимает чат. Хост-требование к G2 — `main.content { position: relative }`. Синхронизированы §2.1, §3.1, §4, §5.1; добавлено правило выхода `Escape` при неактивном стриме | §2.1, §3.1, §4, §5.2 |
| **P2-6** | «Полный реестр» неполон, `idle` приписан `StreamPhase` | Добавлен §2.3.1 — все 54 `id` канонической разметки с тегом, блоком и назначением (проверка биекции — §3). У `#stream-stage[data-stream-phase]` значение `idle` помечено UI-only: `as_lowercase()` возвращает только шесть значений `stream.rs:1076-1090` | §2.3, §2.3.1 |
| **P2-7** | Оракул F-04 проверял контейнер | Геометрия `28px / padding 0 8px / radius 4px` отнесена к `.voice-action` (`voice.css:108-119`); у `.voice-actions` остаётся `display:flex; flex-wrap:wrap; gap:2px` (`voice.css:245-248`). Исправлены §0.3 п. 6, комментарий разметки и assert D8 | спека §0.3, §2.2.1, §6 (D8) |
| **P2-8** | В ветке нет нормативных источников кода | В §0.1 добавлен п. 7: перечислено, каких файлов нет в ветке, что ссылки `stream.rs:NNN` относятся к `main`, и что `TEST-POLICY.md` §13 есть только в `main` (174 строки против 113 в ветке). То же вынесено в риски (§5 п. 4) | спека §0.1 п. 7; отчёт §5 п. 4 |
| **P2-9** | В §4 нет `stopping` и `disabled` | Добавлена строка `3b. stopping` (фаза и ключ держатся до `STREAM_DELETE`, контролы заблокированы, таймаут 10 000 мс → `stop-timeout`). Строка 6 переименована в `disabled` и дополнена правилами для `picking`/`preview` (право отозвано — пикер не вызывается, `#stream-start` блокируется) | §4 |
| P3-1 | «5 узлов», размечено 7 | Заголовок §2.2 — «всех 7 узлов контракта» | §2.2 |
| P3-2 | `data-view="fullscreen"` нигде не используется | Значение убрано из реестра, ARIA-таблицы и §4; полный экран живёт только в `#stream-fullscreen` | §2.3, §4, §5.1 |
| P3-3 | Фокус-трап пропускал пункты меню | Порядок `Tab` включает `#stream-quality-auto`/`#stream-quality-source`, когда меню открыто | §5.2 |

### 1.2. Сверка с RE-отчётами

`reference-oracle/voice-deep.md` (132 строки) и `reference-oracle/frontend-d03.md` (188 строк) прочитаны целиком. Опкодов `18..=22` там нет: в `voice-deep.md` — RTC-контроль `0..16` (Hello/Identify/Select Protocol/Ready/Session Description), UDP/strace и ротация сокета; в `frontend-d03.md` — дамп поиска, эмодзи, участников и настроек, включая «Голос и видео» с Krisp и плейсхолдером «Стриминг». Противоречий со спекой нет: UI-факты (отсутствие камеры, панель голоса) совпадают, протокольная часть опирается на замороженный контракт и код, а не на дамп. Это зафиксировано в §0.1 п. 8 и в легенде `[L]`.

---

## 2. Что теперь в спеке (факт, а не замысел)

| Показатель | Значение | Как проверено |
|---|---:|---|
| Строк в файле | 1549 | `wc -l` |
| `md5` | `48a0ea7daff38341a3a38554bd59282c` | `md5sum` |
| DOM `id` в разметке §2.2 | 54 уникальных | `grep -oE '[^a-z-]id="[a-z0-9-]+"'` |
| Строк в §2.3.1 (реестр ID) | 54 | `grep -c '^| `#'` |
| Оракулов в спеке §6 | 9 (`T-STREAM-UI-{NEG,BND,ERR,EMP,LOAD,CAP,REC,CMP,OBS}-01`), по одному на измерение | `grep -oE 'T-STREAM-UI-[A-Z]+-01' \| sort -u \| wc -l` |
| Строк матрицы §4 | 9 (состояния `picking`/`preview`/`starting` сведены в 2a, `error`/`ended` — в 5b) | таблица §4 |
| Мутантов в §7 | 6 | таблица §7 |
| CSS-строк | 788 | `awk '/^```css$/{f=1;next} /^```$/{if(f){f=0}} f'` |
| Уникальных `var(--*)` | 89, все объявлены в `tokens.css` | `comm -23 used declared` → 0 |
| Сырые цвета / `!important` / `:has()` / fallback | 0 / 0 / 0 / 0 | `grep -cE` |

Таблицы переходов «(Текущее × Событие → Новое × Эффект)» в спеке нет: переходы описаны в §1.2 (`STREAM_CREATE`/`UPDATE`/`DELETE`, приоритет `A-REDUCER-DELETE`) и в матрице §4 как состояния с доступными действиями. Матрицы 4-значных кодов закрытия голосового вебсокета в спеке тоже нет: она относится к контракту голосового соединения, а не к UI-срезу стриминга.

---

## 3. Проверки (design-gate)

Карточка design-only, поэтому «red/green» — это механические оракулы целостности, а не тесты продукта. Команды выполнялись в ветке; `tokens.css` читается из рабочего дерева `main` (в ветке его нет, см. §5 п. 4).

```text
$ awk '/^```css$/{f=1;next} /^```$/{if(f){f=0}} f' waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md > /tmp/vs.css
$ wc -l /tmp/vs.css
788 /tmp/vs.css

$ grep -cE '#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(|!important|:has\(|var\(--[a-zA-Z0-9_-]+,' /tmp/vs.css
0

$ comm -23 \
    <(grep -oE 'var\(--[a-zA-Z0-9_-]+' /tmp/vs.css | sed 's/var(//' | sort -u) \
    <(grep -oE '^\s*--[a-zA-Z0-9_-]+\s*:' rust/gui-spike/ui/tokens.css | sed 's/^ *//;s/ *:$//' | sort -u) | wc -l
0

$ grep -oE 'var\(--[a-zA-Z0-9_-]+' /tmp/vs.css | sed 's/var(//' | sort -u | wc -l
89

$ md5sum rust/gui-spike/ui/tokens.css waves/RW-39/DESIGN/tokens.css
21d570a6bf9c4b5d77b00d3d9b3d1d08  rust/gui-spike/ui/tokens.css
7ce9c1f03e26056243e13df3237b0a9d  waves/RW-39/DESIGN/tokens.css

$ grep -oE 'T-STREAM-UI-[A-Z]+-01' waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md | sort -u | wc -l
9
```

Биекция DOM ID проверена так: список `id` из §2.2 против строк §2.3.1 — `comm -23` и `comm -13` пусты, дублей внутри разметки нет (`uniq -d` пуст).

Контраст §3.2 не пересчитывался заново: правки этого прохода не меняют ни одной пары «передний план / фон». Значения токенов те же, что в прошлой ревизии, а её таблица сошлась с независимым пересчётом ревьюера (30 пар, расхождение только в округлении 4.30 vs 4.29).

---

## 4. Файлы и контрольные суммы

| Файл | Назначение | Строк | `md5sum` |
|---|---|---:|---|
| `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` | Нормативная спека UI стриминга (ветка) | 1549 | `48a0ea7daff38341a3a38554bd59282c` |
| `/home/user/orca/project/waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` | Дубль в основном дереве проекта | 1549 | `48a0ea7daff38341a3a38554bd59282c` |
| `waves/RW-40/G3/task-126-voice-stream-ui-spec/REPORT.md` | Этот отчёт (ветка) | — | — |
| `/home/user/orca/project/waves/RW-40/G3/task-126-voice-stream-ui-spec/REPORT.md` | Дубль отчёта в основном дереве | — | — |

---

## 5. Что осталось вне этой карточки

1. **G1: выровнять `CloseStreamRtc` при повторном `STREAM_CREATE`.** Сейчас эффект есть только в `VoiceSession` (`session.rs:1488-1496`), в `StreamState::apply` — нет (`stream.rs:1504-1530`). Спека фиксирует расхождение метками `[S]`/`[A]`, но код остаётся асимметричным.
2. **G5: привязать ключ просмотра в `Session`.** `watch_stream()` (`session.rs:1423-1431`) не выставляет `active_stream_key`/`stream_phase`, поэтому `stop_screen_share`, `ping_stream` и `set_stream_paused` у зрителя возвращают `NoActiveStream`. UI это обходит (`LocalStreamAction::Close` для выхода, без паузы и ping), но автопинг по `A-PING-CALLSITE` зрителю недоступен. Если для зрителя нужен `op 21`, это отдельная задача G5, а не правка спеки.
3. **G2: хост для оверлея.** `main.content` в `rust/gui-spike/ui/style.css` не позиционирован; без `position: relative` сцена с `inset: 0` накроет вьюпорт вместо области чата.
4. **Ветка не самодостаточна (P2-8).** `stream.rs`, `session.rs`, `STREAM-GATEWAY-CONTRACT.md`, контрактные тесты и `TEST-POLICY.md` §13 есть только в `main`; метки `[S]`/`[F]` станут проверяемыми после мержа design-ветки на актуальный `main`.
5. **P2-4 требует решения владельца словаря.** `permission-channel` означает и серверный `Unauthorized`, и локальный запрет по биту `STREAM`; текст один (`/x4knx`), но источник разный. Если QA нужен различимый код, придётся добавить `permission-local` — это изменение словаря, а не правка формулировки.
