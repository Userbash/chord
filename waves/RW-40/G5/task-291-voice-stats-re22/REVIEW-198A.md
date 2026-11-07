# REVIEW-198A — Техническое ревью PR-170 (`task-291` / `RE-22-01`: `ConnectionQuality`, `PingHistory`, `RtcConnectionStatsSnapshot`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-198A
- **Проверяемая ветка:** `feature/task-291-voice-stats-re22`, HEAD `d7eff40` (Merge PR-168), коммитов в PR нет
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/feature-task-291-voice-stats-re22`
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, файл после каждой мутации восстановлен побайтно (MD5 `d5debf4c…` до и после цикла)
- **Объект:** `rust/crates/discord-voice/src/stats.rs` — `+476 / −0` строк, единственный изменённый путь PR

## 1. Целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до ревью | MD5 после тестов и мутаций |
| --- | --- | --- | --- |
| `rust/crates/discord-voice/src/stats.rs` | `d5debf4cf1831c5a6e473fbedabfe447` | совпал ✅ | `d5debf4cf1831c5a6e473fbedabfe447` ✅ |
| `rust/crates/discord-voice/src/lib.rs` | не изменялся | `pub mod stats;` на месте (строка 21) | `git diff` пуст ✅ |
| `rust/crates/discord-voice/src/session.rs` | не изменялся | — | `git diff` пуст ✅ |

`git diff --numstat`: `476 0 rust/crates/discord-voice/src/stats.rs`. Удалений нет, значит ни один существующий тест модуля (`ntp_middle_is_high_word`) и ни одна существующая функция не переписаны. Вставка локализована после строки 469 (`impl SessionStats`) и в `#[cfg(test)] mod tests`.

## 2. Состав изменений (`git status --porcelain`)

```
 D node_modules
 M package-lock.json
 M rust/crates/discord-voice/src/stats.rs
```

- `stats.rs` — предмет PR-170 (476 строк: константы, `ConnectionQuality`, `PingSample`, `PingHistory`, `compute_outbound_loss_rate_pct`, `short_hostname`, `RtcConnectionStatsSnapshot`, 22 новых теста).
- `D node_modules`, `M package-lock.json` — шум дерева, к PR не относятся: симлинк `node_modules` (`120000 …`) отсутствует в рабочей копии, lock отличается блоками `bin`/`engines` пакета `discord-lab` и был изменён до начала работы (mtime `12:05:54` против `12:07:27` у `stats.rs`). **При merge брать только `rust/crates/discord-voice/src/stats.rs`, не `git add -A`** (см. `P2-6`).

## 3. Сверка с оракулом `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`

Проверял поведение, а не только текст: вырезал эталонные фрагменты из бандла по смещениям и переписал их на Python один-в-один, после чего прогнал те же входные векторы, что в Rust-тестах. Лог: `logs/review-198a-oracle.log`, скрипт `logs/oracle-check-198a.py`. Итог: `RESULT: ALL CHECKS MATCH`.

| Функция в PR | Оракул (смещение в дампе) | Эталонное поведение | Совпадение |
| --- | --- | --- | --- |
| `ConnectionQuality::evaluate` | геттер `quality`, mod `613681` @ `2469479` | `state !== RTC_CONNECTED || lastPing === undefined` → `unknown`; `ping > 500 \|\| loss > 10` → `bad`; `ping > 250 \|\| loss > 5` → `average`; иначе `fine`. Строгие `>` на всех четырёх порогах | ✅ 12/12 векторов, включая точные границы `250`, `500`, `5.0`, `10.0` |
| `PingHistory::push_ping` | `_handlePing`, @ `2504200` | `push` затем `while (len >= 200) shift()` → в буфере максимум **199**; `value > 500` → `_pingBadCount++` | ✅ после 200 вставок `len = 199`, фронт `2`, хвост `200`; `250/500` не считаются, `501` считается |
| `PingHistory::average_ping_ms` | `getAveragePing`, @ `2474810` | `slice(0, min(len, 20))`, среднее по **первым** (самым старым) точкам; `0` при пустом буфере | ✅ `1..=25` → среднее по `1..=20` = `10.5`; пустой → `0`; один отсчёт `123` → `123` |
| `PingHistory::note_ping_timeout` | `_handlePingTimeout`, @ `2504320`; `handlePingTimeout` mod `206959` @ `10828444` | `push(seq)`; при `len >= 3` и `s[n-1]==s[n-2]+1 && s[n-2]==s[n-3]+1` вызывается `_handlePing(timeoutMs)`, fallback `4e3` | ✅ `1,2,4,5,6` срабатывает ровно на `6`; `1,2,4` не срабатывает; значение сэмпла `4000` |
| `compute_outbound_loss_rate_pct` | `handleStats`, mod `206959` @ `10812800` | `deltaSent = curr.sent - prev.sent`, `deltaLost = curr.lost - prev.lost`; `deltaSent === 0` → `0`; иначе `clamp(deltaLost/(deltaSent+deltaLost), 0, 1) * 100` | ✅ `(1000,0)→(1090,10)` = `10.0`; `(0,0)→(99,1)` = `1.0`; нулевая дельта = `0.0` |
| `short_hostname` | `getShortHostname`, mod `978171` @ `7459580` (`9829977` в файле) | `null == e ? "" : e.split(".")[0]` | ⚠️ совпадает на `finland10192.discord.media:443`, `plainhost`, `""`; расходится на `host:443` — см. `P2-1` |

`as_str()` проверен по `bFR` (`unknown/fine/average/bad`) из `RE-GAP-22` §2.1, `RtcConnectionStatsSnapshot` — по списку 11 полей `TASK.md` L48: набор полей и типы совпадают дословно.

## 4. Тесты

Все прогоны с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`, `--offline`.

| Проверка | Результат | Лог |
| --- | --- | --- |
| `cargo test -p discord-voice --lib` | exit `0`: `36 passed; 0 failed` | `logs/review-198a-test-lib.log` |
| `cargo test -p discord-voice` (все таргеты) | exit `0`: 194 теста (lib `36`, `crypto_vectors 16`, `devices_contract 5`, `media_contract 16`, `reception_contract 10`, `reconnect_contract 10`, `regions_contract 10`, `rtcp_contract 16`, `signaling 18`, `srtp_contract 22`, `stats_contract 20`, `udp 15`), doc-tests `0` | `logs/review-198a-test-all.log` |
| `cargo clippy -p discord-voice --tests -- -D warnings` | exit `0`, предупреждений нет (`Checking discord-voice` выполнено заново, не из кэша) | `logs/review-198a-clippy.log` |
| `cargo fmt -p discord-voice -- --check` | exit `0` | `logs/review-198a-fmt.log` |

Red-фаза из отчёта перепроверена: `red.log` — `running 36 tests`, `20 passed; 16 failed`, падения именно на утверждениях (`left: 0, right: 11`, `left: None, right: Some(10.0)`, `option.unwrap() on a None value`), а не на компиляции. То есть тесты писались до реализации, а не подгонялись под неё. `green.log` соответствует моему прогону по числу и составу таргетов.

Что реально проверяют тесты (22 новых `#[test]`, было 1):

- пороги `evaluate`: границы `250`/`251`, `500`/`501`, `5.0`/`5.0001`, `10.0`/`10.0001`, отсутствие пинга, `!connected`, `NaN`/`inf`/`-0.1`/`100.1`;
- `as_str` — все четыре значения;
- буфер пингов: пустой, один отсчёт, 200 → 199 с проверкой фронта и хвоста, счётчик `> 500` на `250/500/501/4000`;
- таймауты: ложная серия `1,2,4`, срабатывание на хвосте `4,5,6`, `clear` сбрасывает и серию;
- `average_ping_ms`: первые 20 из 25 (`11`), пустой (`0`), один отсчёт;
- `compute_outbound_loss_rate_pct`: нормальная дельта, нулевая дельта `sent`, регресс `sent` и `lost` → `None`, малая дельта;
- `short_hostname`: домен+порт, пробелы, `host:443`, без точки, пустая строка;
- снимок: все 11 полей, `quality`, `clone`/`PartialEq`.

Независимая проверка ожидаемых значений сделана по бандлу (§3) — тесты сверяют не реализацию с собой, а литеральные значения, выведенные из оракула (`210/20 = 10.5 → 11`, `199`, `4000`, `10/(90+10) = 10%`). Единственное исключение — `DEFAULT_PING_TIMEOUT_MS` в assert (`P3-1`).

## 5. Мутации

Методика: `python3`-замена с контролем, что шаблон встречается в файле ровно один раз (иначе `ABORT`), прогон `cargo test -p discord-voice --lib`, восстановление из бэкапа `cp`, сверка MD5. Харнесс `mutations/mutate-198a.sh` падает, если MD5 отличается от `d5debf4cf1831c5a6e473fbedabfe447`; сводка `mutations/run.log`, логи `mutations/M1.log … M5.log`.

| # | Функция | Мутация | Итог | Убивший тест |
| --- | --- | --- | --- | --- |
| M1 | `ConnectionQuality::evaluate` | `ping_ms > PING_FINE_MAX_MS` → `>=` (граница `250` уезжает в `Average`) | убит | `quality_fine_on_low_ping_and_loss` (`35 passed; 1 failed`) |
| M2 | `PingHistory::push_ping` | `while len > MAX_PING_HISTORY_LEN` → `>=` (буфер на 198) | убит | `history_holds_at_most_199_samples` (`35 passed; 1 failed`) |
| M3 | `PingHistory::note_ping_timeout` | `b == a + 1 && c == b + 1` → `b == a && c == b` (серия не распознаётся) | убит | `three_consecutive_timeouts_record_4000_ms`, `clear_resets_samples_counters_and_timeout_history` (`34 passed; 2 failed`) |
| M4 | `PingHistory::average_ping_ms` | `len().min(PING_AVERAGE_WINDOW)` → `len()` (окно 20 снято) | убит | `average_uses_first_twenty_samples_and_rounds` (`35 passed; 1 failed`) |
| M5 | `compute_outbound_loss_rate_pct` | `delta_sent.saturating_add(delta_lost)` → `delta_sent` (знаменатель без потерь) | убит | `loss_rate_uses_deltas`, `loss_rate_small_delta` (`34 passed; 2 failed`) |

**Итого `5/5` (100%).** После каждой мутации и в конце цикла: `d5debf4cf1831c5a6e473fbedabfe447` — файл идентичен исходному, `FINAL … (IDENTICAL)`.

## 6. Отклонения от оракула (spec-level, зафиксированы ТЗ)

Код расходится с оригиналом ровно там, где это предписано `TASK.md`; на входных данных из реального клиента поведение совпадает. Блокирующими не считаю, но каждое нужно занести в `RE-GAP-22`, чтобы следующий parity-аудит не завёл на них баг.

### P2-1. `short_hostname` режет и по `:`, оракул — только по `.`
Оракул: `hostname.split(".")[0]`. Тест `short_hostname_strips_domain_and_port` утверждает `"host:443" → "host"`, чего оракул не делает (`"host:443"`). `TASK.md` L46 требует «до первой `.` или `:`». В реальном клиенте порт отрезается раньше, при разборе endpoint (`ep.A.toURLSafe`), и `hostname` приходит без порта, поэтому расширение недостижимо на боевом пути. **Рекомендация:** зафиксировать как осознанное расширение контракта в `RE-GAP-22` §2.3 (строка `getShortHostname`), не переписывая оракул.

### P2-2. `evaluate` отправляет `loss` вне `0..=100` и `NaN` в `Unknown`
Оригинал: `loss = 150` → `BAD` (условие `> 10`), `loss = NaN` → `FINE` или `AVERAGE` по пингу (оба сравнения `false`). Rust: `Unknown` (guard `TASK.md` L29). Для домена `0..=100` поведение идентично; расхождение — только на мусоре от нативного слоя, и `Unknown` здесь безопаснее «зелёного» индикатора при `NaN`. **Рекомендация:** строка в `RE-GAP-22` §2.2 с явной пометкой «защитное отклонение».

### P2-3. `average_ping_ms` — `u64` с округлением, оригинал отдаёт `f64`
`getAveragePing()` возвращает `10.5`; контракт (`TASK.md` L38) требует `(sum/count).round() as u64`. Для отображения это эквивалент: попап форматирует `toFixed(0)`, аналитика отправляет `Math.round(this.getAveragePing())` (`ping_average` @ `2487592`). Но если позже понадобится дробное среднее (спарклайн, тренды) — брать `average_ping_ms` нельзя, потеряна точность. Плюс не смоделировано условие оригинала `_socket == null → 0`: в Rust-слое сокета нет, `0` отдаётся только для пустого буфера. **Рекомендация:** комментарий в doc-блоке метода и запись в `RE-GAP-22`.

### P2-4. `note_ping_timeout` всегда пишет `4000`, оригинал берёт значение из события
Оракул: `handlePingTimeout=(e,t,n,i)=>{emit(PingTimeout, n, i>0 ? i : 4e3)}` (mod `206959` @ `10828444`), то есть при `i > 0` в буфер попадает фактический таймаут нативного слоя, и только fallback — `4000`. Rust-API `note_ping_timeout(seq, time_ms)` значения не принимает (`TASK.md` L37 требует константу). Пока `PingHistory` не подключён к `session.rs`, это не проявляется; после интеграции при кастомном таймауте (`i > 0`) сэмпл будет занижен/завышен. **Рекомендация:** отдельная карточка на `note_ping_timeout(seq, time_ms, timeout_ms: Option<u64>)` одновременно с интеграцией в `session.rs` (DoD п.2 карточки).

## 7. Замечания без блокировки (`P3`)

- **P3-1. `DEFAULT_PING_TIMEOUT_MS` не запинен литералом.** `three_consecutive_timeouts_record_4000_ms` сравнивает `sample.value_ms` с самой константой: замена `4_000` на любое значение `> 500` (например `3_000`) тест переживёт — `ping_bad_count` всё равно станет `1`. Оракул даёт именно `4e3`. **Рекомендация:** `assert_eq!(sample.value_ms, 4_000);` рядом с проверкой константы.
- **P3-2. Нет кейса `loss = 100.0` и комбинации `500 / 10.0`.** Guard отсекает только `> 100`, значит `100.0` — валидное значение и обязано давать `Bad`; в тестах проверено лишь `100.1 → Unknown`. Ветка достижима, добавление одного assert закрывает границу.
- **P3-3. `timeout_seqs` растёт неограниченно.** Оракул ведёт себя так же (`_pingTimeouts` не чистится), воркер это отметил в `REPORT.md`. При часовом звонке с интервалом пинга `5000 мс` это ~720 `u64` — не проблема; ограничение имеет смысл только вместе с `clear()`-политикой сессии.
- **P3-4. `average_ping_ms` суммирует `u64`.** 20 отсчётов, близких к `u64::MAX`, переполнят `sum` (panic в debug, wrap в release). Реальные RTT — единицы тысяч мс, поэтому недостижимо; `u128` или `fold` с `saturating_add` был бы дешевле нерва.
- **P3-5. `PingHistory::new` без `#[must_use]`**, тогда как `RttEstimator::new` и остальные конструкторы модуля его имеют. Стилистическая неоднородность.
- **P3-6. `REPORT.md` занижает число новых тестов.** Написано «16 новых тестов»; фактически в `stats.rs` `23 #[test]`, из них 1 существовавший (`ntp_middle_is_high_word`) → **22 новых**. 16 — это подмножество, падавшее в red-фазе (остальные 6 проходили на заглушках, отдававших `Unknown`/`0`/`None`). Цифру в отчёте стоит поправить, чтобы приёмка не считала покрытие по красным тестам.

## 8. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| Константы `250/500`, `5%/10%`, `199`, `20`, `4000` по оракулу | ✅ совпадают с бандлом (§3) |
| `ConnectionQuality` + `evaluate` + `as_str` | ✅ 12/12 векторов, строгие `>` на всех порогах |
| `PingHistory`: лимит `199`, среднее по первым `min(len,20)`, `ping_bad_count > 500`, 3 подряд → `4000` | ✅ сверено с `_handlePing` / `_handlePingTimeout` / `getAveragePing` |
| `compute_outbound_loss_rate_pct` по формуле `deltaLost/(deltaSent+deltaLost)*100` | ✅ включая `deltaSent == 0` и регресс счётчиков |
| `short_hostname`, `RtcConnectionStatsSnapshot` (11 полей) | ✅ (расширение `:` — `P2-1`) |
| `cargo test -p discord-voice --lib` | ✅ `36/36` |
| `cargo test -p discord-voice` (все таргеты) | ✅ 194 теста, 0 падений |
| `cargo clippy --tests -- -D warnings` | ✅ exit `0`, 0 предупреждений |
| `cargo fmt --check` | ✅ exit `0` |
| Мутации `5/5` | ✅ 100%, откат побайтный |
| Red-фаза настоящая (тесты раньше кода) | ✅ `20 passed; 16 failed` на утверждениях |
| Никаких `unwrap`/`panic!`/`unsafe` в новом коде | ✅ индексация `len-3..len-1` под guard `len >= 3`, `unwrap_or` — безопасный |
| Границы файлов: только `stats.rs` | ✅ 476/0, `lib.rs`/`session.rs` не тронуты |
| `git commit/push` не выполнялся | ✅ |
| `P0 = 0`, `P1 = 0` | ✅ |

## 9. Артефакты

```
waves/RW-40/G5/task-291-voice-stats-re22/REVIEW-198A.md                     — этот отчёт
waves/RW-40/G5/task-291-voice-stats-re22/logs/review-198a-test-lib.log      — lib 36/36
waves/RW-40/G5/task-291-voice-stats-re22/logs/review-198a-test-all.log      — все таргеты пакета, 194 теста
waves/RW-40/G5/task-291-voice-stats-re22/logs/review-198a-clippy.log        — clippy -D warnings
waves/RW-40/G5/task-291-voice-stats-re22/logs/review-198a-fmt.log           — fmt --check
waves/RW-40/G5/task-291-voice-stats-re22/logs/review-198a-oracle.log        — независимая сверка с бандлом
waves/RW-40/G5/task-291-voice-stats-re22/logs/oracle-check-198a.py          — питоновский оракул-двойник
waves/RW-40/G5/task-291-voice-stats-re22/mutations/mutate-198a.sh           — харнесс с восстановлением и MD5-контролем
waves/RW-40/G5/task-291-voice-stats-re22/mutations/run.log                  — сводка 5/5, MD5 до/после
waves/RW-40/G5/task-291-voice-stats-re22/mutations/M1.log … M5.log          — логи мутантов
```

**Вердикт: APPROVED.** `P0 = 0`, `P1 = 0`, мутации `5/5`, `stats.rs` побайтно равен состоянию из `REPORT.md` (`d5debf4cf1831c5a6e473fbedabfe447`). Следующий статус — `Ready for Merge`; merge одного пути (`rust/crates/discord-voice/src/stats.rs`), запись в `waves/db.jsonl` и исключение шума `node_modules`/`package-lock.json` — за координатором.
