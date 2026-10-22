# Повторное независимое ревью P2-01 (round 2)

**Ревьюер:** независимый субагент (не автор исправлений).  
**Дата:** 2026-10-20.  
**Режим:** только чтение исходников, тестов, логов, DESIGN/TZ и фикстур; `cargo` писал только
в `target/`. Временный пробник — вне репозитория (`/tmp/opencode/p2-01-r2-probe`). В репозитории
записан только этот отчёт. Живой Discord, attach/walk и сеть не использовались.

**Снимок:** рабочее дерево dirty (RW-10/RW-11 + RW-12), коммитов нет. Проверенные md5:

| Файл | md5 |
|---|---|
| `src/ready/value.rs` | `090bf9441acd8459d310ee93d2242dd6` |
| `src/ready/snapshot.rs` | `d88df7bc6ace6c44ca3fd81dae8b3c59` |
| `src/ready/social.rs` | `33386b7304367ff98d06e37c8d1f4e64` |
| `src/ready/guild.rs` | `2bfbea7ae9cfee092696768339f28c88` |
| `tests/ready_review.rs` | `f7f3a6d32df16521f96b84532662662a` |
| `tests/ready_core.rs` | `04a5aa28d971041c45a2fd85ee422d5f` |

## 1. Итоговый вердикт

**Не принято (формально).** Новых P0/P1 нет. P1-1 и исходные P2-1…P2-3 по поведению закрыты; P3-1/P3-2
закрыты; P3-3 отложен и не блокирует. Однако после уточнения контракта остались два **P2**,
которые не позволяют закрыть карточку как соответствующую замороженному A1:

- **P2-4** — `Ready::parse` для не-map `d` возвращает `WrongType`, а обновлённые DESIGN §3.5 и
  TZ §2 предписывают `NotReady`; тест `r2` покрывает только `from_dispatch`.
- **P2-5** — сигнатуры `parse_guilds` и `parse_user` изменены в коде, но замороженные §3.4,
  TZ P-2 и псевдокод §3.5 всё ещё описывают старые сигнатуры.

Оба пункта — контрактно-документальные, не потеря данных; тесты, fmt и clippy зелёные. Если
координатор вернёт в контракте исходное разделение (`from_dispatch` → `NotReady`,
`Ready::parse` → `WrongType`) и синхронизирует §3.4/TZ, P2-4/P2-5 снимаются без изменения
рантайма. Если же выбран вариант «`Ready::parse` → `NotReady`», нужна одна строка кода и
регрессионный тест.

## 2. Закрытие находок первого ревью

| Находка | Статус round 2 | Доказательство |
|---|---|---|
| **P1-1** атом-строки | **Закрыта** | `value::text` принимает `Str` и `Atom`, кроме `nil`/`true`/`false` (`value.rs:41-50`); `r1_atom_strings_are_text` зелёный; пробник: `data_mode=Some("full")`, `session_type=Some("normal")`, `status=Some("invisible")`, `client=Some("desktop")`, `os=Some("linux")` |
| **P2-1** `from_dispatch` при не-map `d` | **Закрыта для `from_dispatch`**; для `Ready::parse` — см. P2-4 | `snapshot.rs:159-165`: нет `d` → `Missing`, не-map → `NotReady`; пробник: `from_dispatch(d=5) → Err(NotReady)`; `r2` зелёный |
| **P2-2** молчаливые дефолты вложенных полей | **Закрыта** | `parse_user`/`parse_private_channel`/`parse_relationship`/`parse_client_info`/`parse_session`/`parse_member` получают `path` + `&mut ReadyDiagnostics`; `r3` зелёный; пробник даёт `d.private_channels[0].flags`, `d.private_channels[0].recipient_ids[0]`, `d.merged_members[0][0].flags` |
| **P2-3** сдвиг `merged_members` после malformed-гильдии | **Закрыта для skip; для дубликатов — см. P3-4** | `ParsedGuilds.raw_positions` (`guild.rs:177-222`), привязка по сырому индексу (`snapshot.rs:249-271`); пробник: raw `[bad, id2]` + группы `[100,200]` → гильдия `2` получает `[200]`, `malformed=["guilds[0]","merged_members[0]"]`; `r4` зелёный |
| **P3-1** явный `merged_members: []` | **Закрыта** | `snapshot.rs:251-260` различает отсутствие/`nil` и явный список; `r5` зелёный; пробник: `malformed=["merged_members:0 vs guilds:2"]` |
| **P3-2** префикс `d.` у `ignored` | **Закрыта** | `guild.rs:241,563,583,603,620,642,659` и `social.rs:30,50,67,87,102,109` формируют `d.`; `r6` и обновлённый A8 зелёные; пробник даёт `d.*` |
| **P3-3** публичный `value` | **Отложена; не блокирует** | `ready.rs:17` остаётся `pub mod value`; интеграционные тесты (`ready_fixture.rs`) используют `value::*` как оракул формы `EtfValue`. Это лишняя публичная поверхность, но не безопасность и не данные; backlog приемлем |

## 3. Новые находки

| ID | Severity | Файл:строка | Суть | Доказательство | Предлагаемый фикс |
|---|---|---|---|---|---|
| **P2-4** | **P2** | `src/ready/snapshot.rs:173-178`; `DESIGN-P2-01.md:142,596`; `TZ-P2-01.md:32-35` | `Ready::parse` при не-map `d` возвращает `WrongType`, хотя обновлённый §3.5/TZ говорит `NotReady`. `Ready::parse` не может отличить «d отсутствует» — это свойство `from_dispatch`; формулировка §3.5 шага 1 смешивает два входа. | Пробник: `Ready::parse(&Int(5))` → `Err(WrongType { path:"d", expected:"map", found:"int" })`; `Ready::from_dispatch(d=5)` → `Err(NotReady)`. `r2` проверяет только `from_dispatch`. В первом ревью для `Ready::parse` предлагалось оставить `WrongType`; после уточнения текста код не менялся. | Синхронизировать одно из двух: **(A)** вернуть в DESIGN §2.3/§3.5/TZ исходное разделение: `from_dispatch` не-map → `NotReady`, `Ready::parse` не-map → `WrongType`; или **(B)** изменить `Ready::parse` на `NotReady`, убрать из §3.5 шага 1 «d отсутствует» для payload-парсера и добавить тест `Ready::parse(&Int(5))`. Вариант A соответствует первому ревью и точнее для payload-парсера; вариант B — буквальному тексту TZ. |
| **P2-5** | **P2 (контракт; без рантайм-эффекта)** | `src/ready/guild.rs:186`; `src/ready/social.rs:250-254`; `DESIGN-P2-01.md:561,564,751-752,802`; `TZ-P2-01.md:24` | Код изменил замороженные сигнатуры: `parse_guilds` теперь возвращает `ParsedGuilds`, `parse_user` принимает `path`/`diagnostics`. DESIGN §3.4, §3.5-псевдокод и TZ P-2 всё ещё фиксируют старые сигнатуры и «сигнатуры не меняются». | `parse_guilds(...)->ParsedGuilds` против §3.4 `-> Vec<Guild>`; `parse_user(value, path, diagnostics)` против §3.4 `parse_user(value)`. Код и тесты зелёные; это расхождение контракта, не рантайма. | Обновить §3.4, §3.5-псевдокод и TZ P-2, добавив `ParsedGuilds`/новые аргументы; либо сохранить старые сигнатуры и вынести позиционную карту во внутренний helper. Зафиксировать решение в отчёте. |
| **P3-4** | P3 | `src/ready/guild.rs:199-210`; `src/ready/snapshot.rs:262-267`; `DESIGN-P2-01.md:610-612` | Для **дубликата** гильдии не зафиксировано, какая группа `merged_members` побеждает. Текущая реализация оставляет только последнюю группу, а первую непустую группу помечает `malformed`. | Пробник: guilds `[id1 "first", id1 "second"]`, groups `[ [100], [200] ]` → гильдия `1` имеет `name="second"`, `members=[200]`, `malformed=["merged_members[0]"]`, `duplicates=["guilds:1"]`. Контракт говорит лишь «last wins» и «группа дедуплицированной гильдии не приклеивается к соседней», но не определяет победителя группы. | Явно записать в §2.3/§3.5: «для дубликата гильдии побеждает группа последнего сырого вхождения; группы предыдущих — `malformed`» и добавить тест. Если нужна первая группа/слияние — исправить до приёмки. |
| **P3-5** | P3 | `reports/p2-01-review-red.md5:1`; `reports/p2-01-review-fix.md5:7` | В файлах md5 записан старый отпечаток red-копии `e091…`, тогда как фактическая red-копия и текущий тест — `f7f3…`. Текст отчёта (`p2-01-review-fixes.md:28,54`) уже содержит правильный `f7f3…`; red-final/green-final логи не содержат явных строк `# EXIT=101/# EXIT=0`. | `md5sum` фактических файлов: `f7f3a6d32df16521f96b84532662662a`; `cat p2-01-review-red.md5` — `e091…`; `red-final.log` заканчивается `test result: FAILED. 0 passed; 6 failed`, без `# EXIT`; green-final — `6 passed; 0 failed`, без `# EXIT`. На тестовые результаты не влияет; независимый прогон подтверждает. | Пересоздать/дописать md5-файлы после финального red (или пометить их как исторические) и добавить в логи строку с командой/exit-кодом. |

P3-3 (публичный `value`) считаю **не блокирующим**: это вопрос формы API/тестовой
связанности, а не данных, безопасности или контракта READY. Отложение в backlog корректно;
при отдельной задаче можно сделать `pub(crate)` + тестовый helper/feature.

## 4. Red-first и полные прогоны

### 4.1. Red-final

- `diff -u waves/RW-12/reports/p2-01-review-test.red.rs rust/crates/discord-core/tests/ready_review.rs`
  — **пусто**; md5 обеих сторон `f7f3a6d32df16521f96b84532662662a`.
- `diff -u waves/RW-12/reports/p2-01-review-ready-core.red.rs rust/crates/discord-core/tests/ready_core.rs`
  — **пусто**; md5 обеих сторон `04a5aa28d971041c45a2fd85ee422d5f`.
- Номера строк в `p2-01-review-red-final.log` (`91, 107, 131, 163, 181, 211`) совпадают с
  текущим `ready_review.rs`; это подтверждает, что red снят на финальном тексте тестов.
- `p2-01-review-red-final.log`: `0 passed; 6 failed`; это **mutation-red**: по mtime
  продуктовые файлы восстановлены в `14:33:48`, а red-лог — `14:33:46`; предупреждения
  `unused variable` относятся к временно откатанному коду. Это не «зелёный под тест», а
  проверка, что финальные тесты падают без исправлений. Первый red на новой спеке
  (`p2-01-review-red-review.log`) — тоже `0/6`.
- В `green-final` — `6 passed; 0 failed`; независимый
  `cargo test --offline -p discord-core --test ready_review` — **6/6, exit 0**.

Замечание P3-5: `p2-01-review-red.md5` и последняя строка `p2-01-review-fix.md5` содержат
старый `e091…`; фактическая копия — `f7f3…`. Текст отчёта правильный, но машинные md5-файлы
нужно обновить, иначе аудит red-first выглядит противоречиво.

### 4.2. Полные прогоны (независимо, offline)

| Команда | Результат |
|---|---|
| `cargo test --offline -p discord-core` | **241 passed, 0 failed**, exit 0 (25 suites) |
| `cargo test --offline --workspace` | **283 passed, 0 failed**, exit 0 (45 suites) |
| `cargo fmt --all -- --check` | exit 0, без вывода |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0, без предупреждений |

Это совпадает с `p2-01-review-core-green.log` (241) и
`p2-01-review-workspace-green-final.log` (283). Прогоны выполнены уже после восстановления
финальных продуктовых файлов; текущие md5 совпадают с отчётом.

## 5. Проверка регрессий (независимый пробник)

Пробник `/tmp/opencode/p2-01-r2-probe` использует публичный `Ready::parse`/`from_dispatch` и
`EtfValue`; в репозиторий не входит. Ключевой вывод:

```text
Ready::parse(&Int(5)): Err(WrongType { path: "d", expected: "map", found: "int" })
from_dispatch(d=5):   Err(NotReady)

atom text/bool/nil:
  data_mode=Some("full") session_type=Some("normal")
  unavailable=false large=true bot=true ad=true
  country=None subdivision=None status=Some("invisible")
  client=Some("desktop") os=Some("linux")
  ignored=["d.subdivision_code"]          # Atom("true") в текстовом поле — корректно ignored
  malformed=[]

nested wrong types:
  ignored=["d.private_channels[0].flags",
           "d.private_channels[0].recipient_ids[0]",
           "d.merged_members[0][0].flags"]

raw merged_members index:
  guilds=[(2, [200])]
  malformed=["guilds[0]", "merged_members[0]"]

explicit []:
  malformed=["merged_members:0 vs guilds:2"]
```

Отсюда:

- атом-текст не ломает `bool`/`nil`: `Atom("true")/Atom("false")` работают как bool,
  `Atom("nil")` — как отсутствие; `Atom("true")` в строковом поле даёт `ignored`, а не текст;
- `from_dispatch` не-map → `NotReady`; `Ready::parse` не-map — см. P2-4;
- вложенные `ignored`-пути начинаются с `d.`;
- сырой индекс `merged_members` сохраняется (гильдия 2 получает группу 200, а не 100);
- явный `[]` при двух гильдиях даёт `malformed`;
- `P2-2`/`P3-1`/`P3-2` на синтетике и в `ready_review` зелёные.

## 6. Границы, артефакты и что не проверялось

- Изменены только `src/ready/{value,snapshot,social,guild}.rs`, `tests/ready_review.rs`,
  `tests/ready_core.rs`, DESIGN/TZ и отчёты; `read_state.rs`, `ready.rs`, `error.rs`,
  `tests/support/*` не менялись в этом раунде. Mtime/содержимое не показывают чужих правок.
- Новых зависимостей и сетевых вызовов нет; `cargo` работал с `--offline`, писал только в
  `target/`. Golden/`logs/`/`functions/`/`evidence/` не изменялись.
- Проверены: DESIGN §2.1–§2.4, §3.3–§3.5, §6; TZ P-1…P-6; `ready_review` 6/6; A8; полный
  fixture-оракул jq: `top=40, guilds=7, channels=272, roles=132, emojis=363, stickers=51,
  private=17, users=146, rels=60, rs=157, rsv=128949, ugs=58, overrides=1328, sessions=1,
  mm=[1×7], user_keys=32`; md5 фикстуры `a1a8fd5662be88108dd25ac619a377f2`.
- Не проверялись: живой Discord/attach/walk, gateway-run, P2-02-интеграция, reducer/SQLite,
  REST/UI/protobuf, presence/voice; это вне P2-01. Реальный захват в этом раунде заново не
  декодировался: атом-формы подтверждены `r1`/пробником и `docs/re/gateway-schema.md:60-77`,
  где зафиксированы `full`, `normal`, `invisible`, `desktop`, `linux` и различие Atom/Str.
- `p2-01-review-fixes.md` не является актом приёмки; приёмку делает координатор.

## 7. Короткий вывод для координатора

1. **P1-1, P2-1 (`from_dispatch`), P2-2, P2-3, P3-1, P3-2 — закрыты**; P3-3 — корректно
   отложен, не блокирует.
2. **P2-4** — существенный блокер: синхронизировать `Ready::parse` и обновлённый
   DESIGN §3.5/TZ §2; предпочтительно вернуть исходное `parse → WrongType`, оставив
   `from_dispatch → NotReady`, либо изменить код на `NotReady` и добавить тест.
3. **P2-5** — формальный контрактный блокер: обновить §3.4/TZ P-2/псевдокод под новые
   сигнатуры; без этого приёмка по замороженному A1 некорректна, хотя рантайм не затронут.
4. **P3-4/P3-5** — определения дубликатов `merged_members` и чистота md5/exit-логов; не
   блокируют данные, но стоит закрыть до события `accepted`.
