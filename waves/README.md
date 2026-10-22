# waves — журнал волн Rust-клиента (точка восстановления)

Этот каталог — постоянная «база данных» процесса: по нему можно восстановить,
**что уже сделано, чем это доказано и что делать дальше**, после сбоя, перезагрузки
или паузы («ушёл спать»). Каталог в git; рабочие черновики остаются в
`/tmp/opencode/rw-*` (tmpfs — исчезают при перезагрузке, канон здесь).

- `db.jsonl` — журнал событий: append-only, по одному JSON на строку. Строки не
  переписываются, новые события только дописываются в конец.
- `STATE.md` — снимок «где мы сейчас»: принятые волны, следующая волна, открытые
  вопросы и порядок действий для продолжения.
- `RW-XX/` — артефакты волны: ТЗ, red/green-логи (`# EXIT=…`, md5), отчёт, копии и
  бэкапы тестов/кода, логи мутаций и гейта.
- `reviews/` — независимые ревью (`review-*.md`) и рабочие артефакты ревьюеров
  (скрипты мутаций, логи, бэкапы до/после).
- `_TEMPLATES/` — заготовки batch-ТЗ, test-matrix, отчёта воркера и mutation-таблицы;
  копируются в `waves/RW-XX/` и заполняются, а не переизобретаются каждую волну.

С 2026-10-20 волна — это **batch из 3–6 карточек** с одним integration-патчем и одним общим
gate; review идёт после gate, параллельно по карточкам, а доки обновляются один раз после
accepted. Роли, WIP и pipeline — `docs/rust-client/TEAM.md`; batch-план — `WAVE-PLAN.md`.
Воркер сдаёт targeted red/green; полный `gate:worker` — у integration/QA.

## Формат `db.jsonl` (схема v1)

Обязательные поля: `v` (=1), `ts` (ISO-дата), `event`, `summary`.
Дополнительно: `wave` (`RW-NN`), `card` (`P0-02`…), `tests` (`{wave, core, codec, workspace}`),
`md5` (путь файла → полный md5 на момент приёмки), `gate` (имя файла в `.gate/`),
`commit` (хэш), `evidence` (путь в репозитории).

Словарь событий: `wave-planned`, `red`, `green`, `gate`, `review`, `accepted`, `commit`,
`note`, `push`.

## Чтение и восстановление

```sh
tail -n 5 waves/db.jsonl | jq -c .                 # последние события
jq -c 'select(.wave=="RW-07")' waves/db.jsonl      # вся история волны
jq -r '[.wave // "-", .event, (.summary|tostring)] | @tsv' waves/db.jsonl
cat waves/STATE.md                                  # «где мы» и следующий шаг
```

Продолжение после сбоя/сна: `git log --oneline -3` → `cat waves/STATE.md` →
`tail waves/db.jsonl` → карточка из `docs/rust-client/PLAN.md` и `TASKS.md §RW` →
факт — `docs/rust-client/STATUS.md`, дальше — `docs/rust-client/ROADMAP.md` →
старт волны с ТЗ в `waves/RW-XX/` (регламент — `PROCESS.md` §3–§4).

## Правила

1. **append-only**: `db.jsonl` только дополняется; события прошлых волн не редактируются.
2. **Каждая принятая волна** имеет папку `RW-XX/`; после приёмки координатор дописывает
   событие и обновляет `STATE.md` (стережёт мета-тест `tests/meta/waves-sync.test.mjs`).
3. **Только текст**: `md/txt/log/rs/json/md5/sh/mjs/py/jsonl/hexl/toml/lock`. Бинарники,
   сборки (`target/`, `mut-target/`) и сырые дампы (`.zst`, `.bin`, распаковки) в журнал
   не попадают — их место в лаборатории `/home/sanya/re-deep/` и в `/tmp` (черновики).
4. **Секреты**: артефакты санитизированы (`REDACTED`, `<PUB_IP>`); новые — только через
   фильтр `fixtures/tools/make-fixtures.mjs` и правила `docs/re/README.md`.

## Чем доказана каждая волна (быстрый указатель)

| Волна | Карточка | Red-first | Лог-артефакты (в `waves/`) | Приёмка |
|---|---|---|---|---|
| RW-01 | P0-02 | 9+12, финал 21/21 | `RW-01/red*.txt`, `RW-01/review/` | `TASKS.md §RW` |
| RW-02 | P0-03 | 9/9 | `RW-02/red.txt`, `RW-02/reviewer/` | `TASKS.md §RW` |
| RW-03 | P1-03 | 12/12 | `RW-03/red*.txt`, `RW-03/review.md` | `TASKS.md §RW` |
| RW-04 | P1-03 | 10/10 | `RW-04/red*.txt`, `RW-04/review.md` | `TASKS.md §RW` |
| RW-05 | P1-04 | 12F/1P | `RW-05/red*.txt`, `RW-05/mut/` | `reviews/review-rw-05-06.md` |
| RW-06 | P1-05 | 11/12 | `RW-06/red*.txt`, `RW-06/red-gap-tests.txt` | `reviews/review-rw-05-06.md` |
| RW-07 | P1-06 | 3P/8F | `RW-07/red*.log`, `RW-07/snapshot/` | `reviews/review-rw-07.md` |
| RW-08 | P1-07 | 1/7, 2/3, 0/10 | `RW-08/red/`, `RW-08/reports/` | `reviews/review-rw-08.md` |
| RW-09 | P1-08 | 0/5, 0/8, 0/8 | `RW-09/red/`, `RW-09/reports/` | `reviews/review-rw-09.md` |
| RW-10 | P1-07 infra | 16/3, 13/1 | `RW-10/reports/` | `reviews/review-rw-10-11-round2.md` |
| RW-11 | P1-09/11/12 | 10+4+9 | `RW-11/reports/` | `reviews/review-rw-10-11-round2.md` |
| RW-12 | P2-01 + I1 | 0/7 (review), I1 13/3 | `RW-12/reports/` | `reviews/review-p2-01-round3.md`, `review-i1.md` |
| RW-13 | P2-02 | runtime 4F + compile-red | `RW-13/` red/green, gate | `reviews/review-p2-02.md` |
| RW-14 | P2-06 | compile-red + behavior-red | `RW-14/` red/green, gate | `reviews/review-p2-06.md` |
| RW-15 | P2-04 | compile-red + P2-1 red | `RW-15/` red/green, gate | `reviews/review-p2-04.md` |
| RW-16 | P2-05 | 3 targeted red | `RW-16/` red/green, gate, mutation | `reviews/review-p2-05.md` |
| RW-17 | P2-07 | W17a/b/c + P2-1 s11 | `RW-17/` red/green, gate, mutation 7/7 | `reviews/review-p2-07.md` |
| RW-18 | P2-09 | W18a/b/c + P2-1/2 + P3 | `RW-18/` red/green, gate, mutation 13/13 | **принято** 2026-10-21, P0/P1/P2=0, P3=8 (`reviews/review-p2-09-round2.md`) |
| RW-19 | P2-10 | W19a/b/c | `RW-19/` контракт, тесты, red/green | **принято** 2026-10-21, P0/P1/P2=0, P3=11 (`reviews/review-p2-10-round2.md`) |
| RW-20 | P2-11 | W20a/b/c | `RW-20/` контракт, fixture 28 START, red/green | **принято** 2026-10-21, P0/P1/P2=0, P3=10 (`reviews/review-p2-11.md`) |
| RW-21 | P2-08/P1-01/P1-02/P0-04/P0-07 | B02 batch | `RW-21/` DESIGN, REPORT-B02, TEST-MATRIX, red/green, gate, mutations | **принято** 2026-10-21, P0/P1/P2=0 (`REPORT-B02.md`) |
| RW-22 | P3-15/P4-01/P7-04/P8-03 | B03 batch | `RW-22/` WAVE-PLAN, DESIGN-FOUNDATION, REPORT-B03, TEST-MATRIX-B03, red/green, gate, mutations 18/18, review-A/B/C | **принято** 2026-10-21, P0/P1/P2=0 (`REPORT-B03.md`) |

Примечания:

- Копии `.nw` в `RW-01/review/` сохранены в минифицированном виде (без пробелов) —
  ранняя практика той волны; начиная с RW-06 побайтовые копии red-версий тестов лежат
  рядом (`resume_reconnect-red-gap.rs`, `dispatch_router-red.rs`,
  `dispatch_router-gap-red.rs`) и в бэкапах `reviews/`.
- Тяжёлые артефакты ревьюеров (сборки `mut-target/`, отладочные бинарники, распаковки
  `.zst/.bin`) не переносились — исходники остаются в `/tmp/opencode/review-*` до
  перезагрузки и воспроизводимы скриптами из этого журнала.
