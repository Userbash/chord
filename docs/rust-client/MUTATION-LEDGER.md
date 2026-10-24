# Mutation ledger: что ломали, что поймали, чему научились

Реестр мутационных прогонов Rust-клиента по документам волн `waves/B01`, `waves/RW-17` …
`waves/RW-35`. Цель — не «процент ради процента», а доказательство, что критическая ветка
контракта действительно покрыта тестом, а выживший мутант становится regression-тестом или
явным backlog-item.

Источник факта — логи и отчёты волн; сводные цифры `waves/STATE.md` §1. Если отчёт волны
отсутствует, мутанты волны в реестр не добавляются: «нет доказательства» ≠ «ноль мутантов».

## 1. Сводка

| Показатель | Принятый baseline (RW-35 = 6/6) | С локальными f11/f13 (RW-35 = 8/8) |
|---|---:|---:|
| Запланировано мутантов | **281** | **283** |
| Поймано (`KILLED`, ожидаемый тест упал) | **278** | **280** |
| Эквивалентные (вне знаменателя, обоснованы) | **2** | **2** |
| `INCONCLUSIVE` (не засчитаны) | **1** | **1** |
| Catch rate по неэквивалентным | **278/279 = 99,6 %** | **280/281 = 99,6 %** |

Правила счёта:

- `KILLED` — только падение **ожидаемого** targeted-теста при зелёном baseline; compile error,
  timeout, ошибка harness, скрытый `SKIP` и падение «не того» теста — `INCONCLUSIVE`.
- Эквивалентный мутант не входит в знаменатель, но обязан иметь письменное обоснование:
  почему ни один вход не различает поведение.
- После каждого мутанта продукт восстанавливается, md5 сверяется; после restore нужен
  `touch`/`cargo clean -p`, иначе cargo может переиспользовать мутированный rlib.
- Мутационный скрипт не должен менять продукт «насовсем»: `finally`/restore + проверка md5
  обязательны; SIGKILL не должен оставлять дерево в мутированном состоянии (см. §5).

## 2. По волнам

| Волна | Карточки / область | Мутантов | KILLED | Eq | Inconcl. | Restore |
|---|---|---:|---:|---:|---:|---|
| B01 | P2-10 voice states, P2-11 typing | 16 | 16 | 0 | 0 | md5 |
| RW-17 | P2-07 read-state | 7 | 7 | 0 | 0 | md5 |
| RW-18 | P2-09 presence | 13 | 13 | 0 | 0 | md5 |
| RW-21 | P1-01 ETF, P1-02 zstd, P2-08 settings | 24 | 24 | 0 | 0 | md5 |
| RW-22 | P3-15 i18n, P4-01 voice signaling, P7-04 API, P8-03 CI | 18 | 18 | 0 | 0 | копия |
| RW-23 | P4-02 UDP, P8-04 SBOM | 22 | 21 | 0 | 1 | md5 |
| RW-24 | P7-01 i18n full | 20 | 20 | 0 | 0 | md5 |
| RW-25 | P4-03 SRTP, P7-02 perf, P8-01 updater | 18 | 18 | 0 | 0 | md5 |
| RW-26 | P4-07 RTCP/stats, P4-11 regions | 19 | 19 | 0 | 0 | md5 |
| RW-27 | P7-01b list, P6-03 autostart, P6-04 deep links | 22 | 20 | 2 | 0 | md5 |
| RW-28 | P7-01c relative time | 8 | 8 | 0 | 0 | md5 |
| RW-29 | P4-04 RTP, P4-12 reconnect, P7-01e timestamps | 23 | 23 | 0 | 0 | md5 |
| RW-30 | P7-01d duration, P6-02 D-Bus notifications | 18 | 18 | 0 | 0 | md5 |
| RW-31 | P6-06 power/idle, P2-03 SQLite | 14 | 14 | 0 | 0 | md5 |
| RW-32 | P6-07 spellcheck, P6-02b hints | 14 | 14 | 0 | 0 | md5 |
| RW-33 | P1-10 keyring | 7 | 7 | 0 | 0 | md5 |
| RW-34 | P6-02c abstract D-Bus, P6-02d hints ext | 12 | 12 | 0 | 0 | md5 |
| RW-35 | P6-06c fd-passing, logind inhibitor | 6 (8 локально) | 6 (8) | 0 | 0 | md5 |
| **Итого** | | **281 (283)** | **278 (280)** | **2** | **1** | |

Команды и артефакты:

- `waves/<волна>/run-mutations*.py` — сценарий; `waves/<волна>/mutations*.log` — сводка;
  `waves/<волна>/mutations/` — логи отдельных мутантов.
- Ранние волны (B01, RW-17, RW-18) — ручные/скриптовые прогоны; формат логов отличается,
  но md5-восстановление зафиксировано в `MUTATIONS.md`.
- RW-35: принятый baseline — 6/6; локальные f11/f13 добавляют два теста и два мутанта
  (дубликат fd-индекса, лимит `MAX_FDS_PER_MESSAGE`), поэтому текущий dirty-прогон — 8/8.
  Эти изменения требуют отдельного ревью/приёмки и до неё в baseline не входят.

## 3. Классы мутантов

Классификация — по наблюдаемому дефекту, а не по файлу. Один и тот же класс должен
проверяться на каждом уровне, где он применим.

| Класс | Что ломается | Примеры из проекта | Где ловится |
|---|---|---|---|
| Граница / off-by-one | `>` ↔ `>=`, длина, индекс, глубина, лимит | B02 depth `>`→`>=`; RW-17 max-guard; RW-22 JSON depth; RW-23 discovery length; RW-28 `value < 0` → `<= 0`; RW-35 fd limit | unit + contract |
| Инверсия ветки | `==` ↔ `!=`, if/else, resumable ↔ identify | B01 `Some/None`; RW-21 tuple threshold; RW-29 reconnect branch; RW-30 locale fallback | unit + integration |
| Снятие проверки | duplicate, stale, range, NUL, control chars, peer/SSRC | RW-18 stale guard; RW-21 duplicate-key; RW-22 unsafe URL; RW-23 peer check; RW-27 control chars; RW-33 schema match; RW-35 `MSG_CTRUNC` | contract + adversarial |
| Порядок / tie-break | выбор scope, региона, медианы, сортировка | RW-18 displayable tie-break; RW-22 region tie; RW-26 median even-count | unit + oracle |
| Формат / кодировка | endianness, знак, разделители, заголовок | B04 keepalive BE/LE; RW-21 big-int sign; RW-30 NBSP; RW-31 SQLite flags; RW-35 `UNIX_FDS` header | byte-exact contract |
| Жизненный цикл ресурса | fd, Drop, cleanup, счётчик, таймер | RW-32 Drop; RW-35 inhibitor fd, `close` на error-path, `sendmsg` cmsg | integration + live |
| Redaction / секреты | `Debug`, `Display`, ошибки, raw-поля | RW-21 settings Debug; RW-22 voice raw; RW-27 DeepLink token | unit + security |
| Протокольные константы | opcode, tag, type, field presence | B02 ETF tags; B04 RR type; RW-25 SRTP tag; RW-35 message type | conformance |
| Крипто | S-box, HMAC pad, AAD, nonce/counter | RW-25 AES S-box, KDF pad, AAD, nonce reuse | KAT + NIST/RFC |
| Лимиты / caps | max fields, depth, fd, buffer, queue | RW-21 max fields; RW-24 markdown depth; RW-31 flags; RW-35 fd caps | boundary + fuzz |
| Tooling / CI | шаги CI, allowlist, precedence, генератор | RW-22 `ci.sh`/`release.sh`; RW-23 SBOM allowlist; RW-27 generator | meta + dry-run |
| Тестовая слабость | тавтологичный oracle, одинаковый вход, production-константа | B01 V3/V5; RW-27 M3/M4; RW-34 boundary constant; RW-35 shared fd counter | review + mutation |

## 4. Survivors, эквивалентные и `INCONCLUSIVE`

| Волна | ID | Что случилось | Решение |
|---|---|---|---|
| B01 | V3 | `voice_start_time >= 0` → `> 0` выжил: не было `Some(0)` | red-first `v10`: `Some(0)` — валидный epoch; повторно caught |
| B01 | V5 | `voice_users_in_channel` без guild выжил: одинаковый `channel_id` только в одной гильдии | red-first `v13`: два guild с `channel_id=10`; повторно caught |
| RW-23 | #10 | recv buffer 128→74 дал `INCONCLUSIVE`: loopback-тест skip из-за среды | не засчитан; loopback-путь остаётся skip-aware, нужен стабильный harness |
| RW-27 | M3 | `middle` → `start` эквивалентен: во всех 279 строках `middle == start` | задокументирован; M3b (`middle` → `pair`) ловит цикл |
| RW-27 | M4 | fallback `en-US` недостижим: таблица полная 31×3×3 | задокументирован как defensive; не считается дырой теста |
| RW-30 | — | ложный `KILLED` при скрытом `SKIP` | правило: skip классифицируется по ID/причине, не по подстроке |
| RW-31 | — | хрупкий mutation anchor и устаревшие md5 после `fmt` | anchor — точная строка; md5 пересчитывается после финального форматирования |
| RW-33 | — | не покрыты бинарный NUL, область schema-match, тихий live-skip | добавлены k-кейсы; live-флаг при ошибке среды падает |
| RW-34 | — | граница abstract через production-константу; генератор вне матрицы | захардкодить 107/108; генератор включён в матрицу и хеши |
| RW-35 | f11/f13 | не было тестов на дубликат fd-индекса и лимит отправки | добавлены тесты (dirty, ждут ревью); мутанты m7/m8 теперь KILLED |
| RW-35 | f03 | флак ~15 % из-за общего memfd-счётчика | мьютекс/сериализация, 8× стабильно |

Вывод: почти все survivors — не «код плохой», а **слабое место теста или оракула**.
Поэтому каждый survivor обязан получить один из трёх исходов: regression-тест, отдельная
карточка с ID, либо письменное доказательство эквивалентности. Prose-риск без теста
не закрывает P1/P2.

## 5. Уроки и правила

1. **Мутация должна менять наблюдаемое значение.** Иначе она эквивалентна и ничего не
   доказывает. Для stale/duplicate нужен второй вход с тем же ключом и другим payload.
2. **Один мутант — одна ветка.** Комбинированная правка маскирует причину падения и делает
   лог непригодным для ревью.
3. **Baseline важнее catch rate.** Сначала зелёный targeted без запрещённых skip, потом мутация.
4. **Skip — не pass и не KILLED.** Явные live/environment skip перечисляются по ID; если
   мутант опирается на skipped-утверждение, это `INCONCLUSIVE`.
5. **Restore проверяется по md5 и кэшу.** `cp -p` возвращает старый mtime; нужен `touch` или
   `cargo clean -p`, затем повторный targeted.
6. **Скрипт не должен оставлять продукт мутированным.** Запись во временную копию/`finally`,
   проверка md5, отказ от `SIGKILL`-небезопасной схемы; иначе следующий агент тестирует мутант.
7. **Генератор — часть матрицы.** Вход, версия runtime, команда и хеш выхода входят в ТЗ;
   два запуска должны дать одинаковый хеш.
8. **Эквивалентность — с доказательством.** «Не поймали» ≠ «эквивалент». Нужно показать,
   почему на замороженных данных нет различающего входа, и какой мутант проверяет соседнюю ветку.
9. **Мутационный бюджет — по рискам, не по проценту.** S 3–5, M 5–8, L 8–12, XL 10–14;
   критические ветви (крипто, auth, FFI, ресурсы) — 100 % классов.
10. **Регрессия из каждого P0/P1/P2.** Если ревью нашло дефект, которого не было в тестах,
    после фикса добавляется тест; иначе дефект вернётся.

## 6. Воспроизведение

```sh
cd /home/sanya/orca/project
python3 waves/RW-35/run-mutations-w35a.py --filter m7-duplicate-index-allowed
python3 waves/RW-28/run-mutations.py
# общий batch-прогон — только в своём cargo-слоте, не параллельно
```

Для новой волны: скопировать `waves/_TEMPLATES/MUTATIONS.md`, заполнить таблицу
`mutant → test ID → результат → restore md5`, сохранить логи в `waves/<волна>/mutations/`
и добавить строку в §2 этого реестра.

## 7. Ограничения

- Реестр покрывает волны, где есть отчёт/лог; ad-hoc мутации ранних волн RW-01…RW-16
  (например, review-скрипты `waves/reviews/rw-05-06`) не сведены в общую таблицу.
- Цифры не являются KPI качества: 100 % на слабом наборе хуже 90 % на риск-ориентированном.
- Wire-совместимость с Discord там, где нет ключевого материала (SRTP/DAVE), мутациями
  не доказывается; это отдельное ограничение контракта.
