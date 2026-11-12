# Независимое ревью W9r — волна RW-09 (X-Super-Properties, P1-08 / T-REST-04)

**Ревьюер:** субагент W9r (вторая модель, PROCESS.md §4.4 / ТЗ §9).
**Дата:** 2026-10-20. **Рабочая папка ревью:** `/tmp/opencode/rw-09/w9r/`.
**Область:** 3 src-файла (`base64.rs`, `super_properties.rs`, `identity.rs`), 3 теста, тест-данные
(`tests/data/*`), отчётность воркеров, red-артефакты. Рефакторинг W9d — проверен на
не-поведенческую эквивалентность.
**Отчёты воркеров — проверялись независимо, на слово не принимались.**

## Вердикт: **ПРИНЯТО С НИТАМИ** (блокеров нет)

- 21 кейс (B1–B5, P1–P8, I1–I8) зелёные; воркспейс 158/158; clippy/fmt чисто; `gate:worker` passed.
- Red-first подтверждён собственными прогонами заглушек: 0/5, 0/8, 0/8, `# EXIT=101`.
- Оракулы (снимок, выдержки, python3-векторы, §6.2) пересчитаны своими командами — совпали
  побайтово/позначно.
- Мутации: **20/21 поймано** таргетными тестами; 1 выжившая — только классификация варианта
  ошибки в не покрытом кейсе (разбор ниже, нит N1).
- Границы соблюдены: `Cargo.toml`/`Cargo.lock` без diff; `rest/mod.rs` md5 `0eba823bf46c2958cac102dbb42cbb06`;
  golden и снимок не изменялись; лишних файлов нет.

## 1. Оракулы — независимая перепроверка

| Проверка | Команда (своя) | Результат |
|---|---|---|
| md5 снимка | `md5sum capture-run2.snap.jsonl` vs `snapshot-md5.txt` | `abfe14b76cec1e559c3b45522bc44b7a` — совпал (ТЗ §3.1) |
| Заголовок 95974.2323 | `jq -r 'select(.ev=="req" and .requestId=="95974.2323") \| (.headers//{}) \| to_entries[] \| select((.key\|ascii_downcase)=="x-super-properties") \| .value'` | получен; запрос `GET https://discord.com/api/v9/channels/1176785373989445703/messages?limit=12` |
| `.b64` побайтово | `cmp extracted.b64 tests/data/rest-super-properties-2323.b64` | идентично; 1084 симв. + `\n` (1085 Б), оканчивается `==` |
| `.json` побайтово | `cmp <(head -c 811 tests/data/...json) extracted.json` | идентично первые 811 Б; в файле есть хвостовой `\n` (812 Б) |
| md5 выдержек vs ТЗ §3.2 | `md5sum tests/data/rest-*` | `b9e8f843…`, `90b06ded…`, `95e2f861…`, `8b5bb8c9…` — все 4 совпали |
| Векторы base64 | python3 `base64.b64encode(bytes.fromhex(hex))` по `rest-base64-vectors.tsv` | 10/10 совпало (`empty`…`one-00`), decode тоже |
| 22 поля/типы | свой `jq to_entries→tsv` vs `rest-super-properties-22.tsv` | побайтово идентично; типы 18 string, 2 number (641004/94186), 1 boolean (false), 1 null |
| Значения | `distro="\"Arch Linux\""`, `window_manager="KDE,unknown"`, порядок 22 ключей | совпало с TSV и §6.2 |
| `docs/re/rest-d02.md` | md5 `c7e61929677efab6bdac86a05a778fce`; §6.2 JSON-блок | блок побайтово равен живому JSON (порядок и значения) |
| Счётчик «221» | по ДВУМ снимкам: 71 (ready) + 150 (run2) = 221, все с Authorization | так и есть; в одном run2 — 150 (ТЗ §3.1 суммирует оба — корректно) |

Дополнительно: в `.json` md5 из ТЗ (`90b06ded…`) включает хвостовой `\n` — соответствует
формулировке «811 байт + `\n`».

Команды снятия из ТЗ §3.1 воспроизведены дословно и дают файлы, идентичные `tests/data/*`.

## 2. Red-верификация (независимая, с заглушками)

Заглушки: base64 `encode→""`, `decode→Err(InvalidLength(0))`; super_properties `to_json→""`,
`from_json/from_base64→Err(Json("заглушка"))`, `field_names()→["";22]`; identity — замороженный
каркас `06d80ecf79ec57377c67c76f16f0ef28` (тот же файл, что у W9c, `identity-stub.rs`).
Бэкапы — `/tmp/opencode/rw-09/w9r/keep/`; после каждого прогона — восстановление + md5-контроль.

| Таргет | Результат (мой прогон) | Лог |
|---|---|---|
| `rest_base64` | `0 passed; 5 failed`, `# EXIT=101` | `/tmp/opencode/rw-09/w9r/logs/w9r-red-rest_base64.log` |
| `rest_super_properties` | `0 passed; 8 failed`, `# EXIT=101` | `…/w9r-red-rest_super_properties.log` |
| `rest_identity` | `0 passed; 8 failed`, `# EXIT=101` | `…/w9r-red-rest_identity.log` |

Сверка md5 тестов red-логов с текущими (финальными) файлами — **совпали**:

| Тест | md5 (red-лог) | md5 (сейчас) | Красная копия |
|---|---|---|---|
| `rest_base64.rs` | `ddd771c5e732949d4cbdb0e0a49ff756` (w9a-red-fmt.log) | совпал | `waves/RW-09/red/rest_base64-red.rs` = тот же md5 |
| `rest_super_properties.rs` | `34dc62487dfb99553d7f99449fb67911` (w9b-red.log) | совпал | совпал |
| `rest_identity.rs` | `e6c721047d944b0d270565513d90df49` (w9c-red.log) | совпал | совпал |

Исторические red-логи (w9a-red.log `b5605152…`, w9c-red-first/unformatted) — согласуются с
хронологией в отчётах; финальный red каждого модуля сделан на той же редакции теста, что лежит
в репозитории. Архивные копии в `waves/RW-09/red/` md5-идентичны первоисточникам
`/tmp/opencode/rw-09/w9*/` (проверено).

## 3. Мутации — 21 цикл (бэкап → мутация → прогон таргета → восстановление → md5)

Полные логи: `/tmp/opencode/rw-09/w9r/logs/mut-<имя>.log`, машиночитаемо —
`/tmp/opencode/rw-09/w9r/mutations.jsonl`. После каждого цикла src-файл восстанавливался из
бэкапа, md5 сверялся с baseline (`fb49dae8…` / `6ef2ac9d…` / `25d50052…`) — расхождений нет.

| # | Мутация | Таргет | Упавшие тесты | Итог |
|---|---|---|---|---|
| m01 | encode: не пишется `=`-паддинг | rest_base64 | rfc_vectors, lengths_and_padding, live_header_roundtrip, noncanonical_trailing_bits | CAUGHT |
| m02 | decode: мусорный символ принимается (sextet None→Some(0)) | rest_base64 | errors_are_typed | CAUGHT |
| m03 | decode: снята проверка длины mod 4 | rest_base64 | errors_are_typed | CAUGHT |
| m04 | decode: снято `first_pad % 4 == 1` | rest_base64 | — | **SURVIVED** (нит N1) |
| m05 | field_names: `browser`→`browser_x` | rest_super_properties | p1–p7 | CAUGHT |
| m06 | field_names: `os`/`browser` поменяны | rest_super_properties | p1, p2, p3, p4, p5 | CAUGHT |
| m07 | экранирование `"` снято | rest_super_properties | p3, p4, p5 | CAUGHT |
| m08 | экранирование `\` снято | rest_super_properties | p5 | CAUGHT |
| m09 | неизвестный ключ молча пропускается | rest_super_properties | p6 | CAUGHT |
| m10 | дубликат ключа принимается | rest_super_properties | p6 | CAUGHT |
| m11 | переполнение u32 оборачивается | rest_super_properties | p7 | CAUGHT |
| m12 | to_base64 мимо to_json (без экранирования) | rest_super_properties | p4 | CAUGHT |
| m13 | UUID: версия `0x40` не ставится | rest_identity | wayland_kde_full_build, uuid_v4_shape | CAUGHT |
| m14 | UUID: вариант `0x80` не ставится | rest_identity | wayland_kde_full_build, uuid_v4_shape | CAUGHT |
| m15 | локаль без обрезки `.`/`@`/`_` | rest_identity | locale_parsing, wayland_kde_full_build | CAUGHT |
| m16 | display_server: wayland/x11 перепутаны | rest_identity | display_server_fallbacks | CAUGHT |
| m17 | display_server: WAYLAND раньше XDG_SESSION_TYPE | rest_identity | display_server_fallbacks | CAUGHT |
| m18 | config.browser игнорируется | rest_identity | config_overrides_win | CAUGHT |
| m19 | PRETTY_NAME: кавычки снимаются | rest_identity | distro_sources, wayland_kde_full_build | CAUGHT |
| m20 | map_os: macos не маппится | rest_identity | os_arch_mapping | CAUGHT |
| m21 | map_arch: x86_64 не маппится | rest_identity | os_arch_mapping, wayland_kde_full_build | CAUGHT |

### Разбор выжившей m04 (нит N1, не блокер)

Мутация снятия `|| first_pad % 4 == 1` в `data_length` не ловится тестами. Проверено
standalone-харнессом (`/tmp/opencode/rw-09/w9r/scratch_padding.rs`, вне репозитория):

- сейчас: `"A="→InvalidPadding{1}`, `"AAAAA="→InvalidPadding{5}` (по контракту ТЗ §4 —
  «несоответствие data_chars % 4 == 1 → InvalidPadding{index}»);
- мутант: `"A="→InvalidLength(2)`, `"AAAAA="→InvalidLength(6)` — **всё ещё Err** (другой вариант).

Итог: снятие условия не открывает приёма мусора (при `first_pad % 4 == 1` и валидном хвосте
всегда срабатывает либо `pad_len > 2`, либо финальная проверка длины), меняется только классификация
ошибки на непокрытых входах (`"A="`, `"AAAAA="`, `"AAAAA=="`). Рекомендация: добавить в B3 один
кейс (например `"A=" → InvalidPadding{index:1}`) — координатору, правкой теста, не кодом.

## 4. Границы, гигиена, отсутствие «подгонки»

- `git status` до и после ревью совпадает (набор из ТЗ §1); мои действия не создали в репозитории
  ни одного файла (все артефакты W9r — в `/tmp/opencode/rw-09/w9r/`).
- `rust/Cargo.toml`, `rust/Cargo.lock` — без diff (`git diff --quiet` — чисто); новых зависимостей
  нет (в трёх модулях только `std`).
- `src/rest/mod.rs` — `0eba823bf46c2958cac102dbb42cbb06` (заморожен).
- Тест-данные — md5 из ТЗ §3.2, не менялись; golden (`logs/`, `functions/`, `evidence/`) и снимок
  `reference-oracle/d02-snapshot` — только чтение (ни одного файла новее старта волны; md5 снимка
  совпал с `snapshot-md5.txt`).
- Тесты не «подогнаны»: оракулы — внешние файлы (снимок → выдержки → TSV), §6.2-константы,
  RFC-векторы; нет `#[ignore]`/`should_panic`, нет skip-путей (файл тест-данных отсутствует →
  паника с путём); позитивные и негативные контроли в наличии (p6 «валидный принимается», P7
  границы, I5 четыре сценария и т.д.).
- В src-модулях нет паник/`unwrap` на внешних данных (только `unwrap_or*`); нет сети/процессов/unsafe.
- Архив волны (`waves/RW-09/reports/*`, `red/*`) md5-идентичен первоисточникам воркеров.
- W9d-рефакторинг: `diff.patch` прочитан целиком — замены механически эквивалентны
  (`push_sextet`, `data_length`, `extend_from_slice` вместо трёх push, `JsonWriter`,
  `consume_keyword`, `skip_digits`, `configured_or`/`or_unknown`/`has_env`); публичные сигнатуры,
  поля, тексты/позиции ошибок и порядок обращений к источникам не менялись. md5 тестов до/после
  W9d совпадают с red-логами.

## 5. Политика §6.1 — полнота и честность (оценка)

Проверено собственным standalone-харнессом на реальных `SystemSources` (файлы скопированы в
`/tmp`, репозиторий не трогался): получено `os=Linux`, `os_version=7.2.10-1-cachyos` (реальное
ядро машины; совпадает с живым захватом), `os_arch=x64`, `system_locale=ru`, `window_manager=KDE`,
`distro="\"Arch Linux\""` (кавычки — часть значения), `display_server=wayland`, свои поля
(`browser=discord-lab`, `client_version=0.1.0`, build 0), UUID v4 (валидные, разные, версия/вариант
на месте), `to_base64`→`from_base64` без потерь.

- Честность: среда читается из системы/файлов; наш клиент не имитирует чужую сборку — да.
- Конфигурируемость: все 22 поля перекрываются `IdentityConfig` (I7 — все 22 `Some`).
- UUID v4: `(b[6]&0x0f)|0x40`, `(b[8]&0x3f)|0x80`, порядок трёх генераций фиксирован (I1/I6).
- Крайние случаи: пустые env/файлы → `unknown` (проверено харнессом и I3/I5);
  `config.Some("")` возвращается дословно (нит N3); `/dev/urandom` — best-effort задокументирован
  (в `trait`/коде), при сбое формат UUID корректен (нули), риска для тестов нет.
- Полнота покрытия политики таблицы §6.1 — все 22 поля зафиксированы в I1/I7, крайние значения —
  I2–I6, стык слоёв — I8. Полно.

## 6. Прогоны (финальные, после всех циклов)

| Проверка | Команда | Результат |
|---|---|---|
| Воркспейс | `cargo test --offline --workspace` | **158 passed, 0 failed** (лог `logs/workspace-final.log`) |
| Clippy | `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0, чисто |
| Формат | `cargo fmt --check` | exit 0 |
| Гейт | `npm run gate:worker` | **passed**: typecheck ✔, meta 25/25, unit 169/169, smoke 20/20, skip 0; `.gate/worker-2026-10-19T21-07-10-432Z.json` |

## 7. Ниты (не блокеры)

- **N1. Покрытие ветки `first_pad % 4 == 1`** (единственная выжившая мутация m04): тесты не
  фиксируют вариант ошибки для `"A="`/`"AAAAA="`; приём мусора не открывается. Рекомендация —
  один кейс в B3 (координатору).
- **N2. Бухгалтерия md5 в отчёте W9a**: в отчёте финальный md5 теста указан `f691aef6…`, тогда как
  текущий/ red-копия/ w9a-red-fmt.log — `ddd771c5…`; diff между редакциями — только порядок
  импортов (rustfmt), ассерты не менялись, red на текущей редакции подтверждён моим прогоном.
  Red-first в силе; поправить строку при приёмке (или в отчёте волны).
- **N3. `config.Some("")`** возвращается дословно (крайний случай не оговорён в ТЗ; заявлен W9c).
  Семантика выбора вызывающего; если политика хочет «пусто = честное значение» — решить в следующей
  волне/backlog.
- **N4. Ссылка в шапке `rest_identity.rs`** на md5 ТЗ §6.1 `2a1dd6c8…` устарела после правок ТЗ
  (`waves/RW-09/TZ.md` сейчас `16a0e8b8…`) — комментарий, не поведение.
- **N5. Противоречие «порядок проверок» в ТЗ §4** (длина vs паддинг/символы) — уже снято
  «Поправкой координатора» (B3 — источник истины), реализация ей следует; дублировать не нужно,
  но §4 стоит вычитать при закрытии волны.
- **N6. `decode("AA=")`/`decode("AAAA=")`** классифицируются как `InvalidLength` (не
  `InvalidPadding`); канонический base64 таких форм не порождает, контракт их не оговаривает —
  на заметку (связано с N1).

## 8. Что осталось непокрытым / вне области

- Живая отправка заголовка и транспорт — вне волны (ТЗ §10; решение владельца по `waves/TRANSPORT.md`).
- `lints` других крейтов/Node-часть волной не менялись (проверены гейтом).
- Мои артефакты: `/tmp/opencode/rw-09/w9r/{review.md, mutations.jsonl, mutation_runner.py, keep/,
  stubs/, logs/, scratch_padding*.rs, policy/}`.

**Блокеров нет. Вердикт: принято с нитами (N1–N6; N1 — единственный реально новый пункт к
закрытию).**
