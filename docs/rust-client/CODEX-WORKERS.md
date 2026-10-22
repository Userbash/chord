# Codex CLI: воркеры, роли и ускорение (B02)

Статус: рабочий playbook. Codex CLI — **исполнитель**, а не источник истины: контракт, файловая
матрица и оракулы живут в репозитории. Один и тот же агент не пишет и не ревьюит свою карточку.

## 1. Роли

| Seat | Исполнитель | Права | Выход |
|---|---|---|---|
| TL-1 | координатор (эта сессия) | контракты, общие файлы, приёмка, журнал | DESIGN, решения, accepted |
| CODE-1/2 | Codex CLI `workspace-write` | только файлы своей карточки | продукт + targeted red/green |
| QA-MUT | shell/Codex `workspace-write` | временная мутация, restore по md5 | mutation-лог |
| REV-1 | Codex CLI `read-only` | только чтение | P0/P1/P2/P3 + вердикт |
| DOC-1 | Codex CLI `workspace-write` | только свои docs/отчёт | отчёт/уроки |

Правило: **два активных кодовых воркера**, не больше. Ревью и research могут идти параллельно,
но не пишут продуктовые файлы. Cargo-команды сериализуются общим lock:

```sh
flock /tmp/opencode/discord-cargo.lock cargo test -p <crate> --offline
```

## 2. Запуск Codex CLI

Кодовый воркер (точный список файлов и запрет shared-файлов — в промте):

```sh
codex exec -C /home/sanya/orca/project -s workspace-write --approve-for-me \
  --output-last-message waves/RW-21/worker-P1-01.md \
  "Ты CODE-1. Карточка P1-01. Контракт: waves/RW-21/DESIGN-P1-01.md.
   Файлы: rust/crates/discord-codec/src/etf.rs, src/lib.rs, tests/etf_full.rs.
   Только strict TDD: сначала red-лог, затем минимальный green. Не трогай shared-файлы,
   не запускай workspace/gate, не коммить. Верни отчёт по PROCESS.md §4.2."
```

Независимый ревьюер (read-only, без редактирования):

```sh
codex exec -C /home/sanya/orca/project -s read-only review --ephemeral \
  -o waves/RW-21/reviews/review-P1-01.md \
  "Проверь только P1-01 по DESIGN/TEST-MATRIX/REPORT, исходникам и логам.
   Найди реальные дефекты; P0/P1/P2/P3 с file:line и воспроизведением.
   Игнорируй остальные изменения. Не переписывай тесты под код."
```

Если `review --uncommitted` конфликтует с prompt, используй prompt-only review или
`--base HEAD`; не ослабляй sandbox и не давай ревьюеру `workspace-write`.

## 3. Декомпозиция для скорости

1. **Сначала контракт.** Заморозить API, ошибки, границы, test ID и файловую матрицу.
2. **DAG, не очередь.** Карточки, у которых нет общей зависимости/файла, идут одновременно.
3. **Пара.** Два воркера на batch; третий не запускается «для скорости» — он конфликтует за
   `target-dir` и shared-файлы.
4. **Перекрытие.** Пока B02 в code, B01 в gate/review; пока B02 в gate, B03 в research.
5. **Один gate на batch.** Воркеры сдают targeted; `fmt/clippy/workspace/gate` — один раз.
6. **Ревью после gate, не вместо.** P0/P1/P2 закрываются red-first; P3 — backlog.

Ускорение даёт не число агентов, а: непересекающиеся файлы + замороженный контракт +
последовательный cargo + ранний red + независимый оракул.

## 4. Промт воркера (шаблон)

```text
Карточка: <ID>. Batch: <B02/RW-21>.
Контракт: <DESIGN.md>. Тест-матрица: <TEST-MATRIX.md>. Оракул: <dump/fixture>.
Файлы (только эти): <list>. Запрещено: shared-файлы, чужие тесты, Cargo.toml/package.json,
сеть, live Discord, коммиты, gate.
Шаги: 1) red targeted + лог с exit code; 2) минимальный green; 3) targeted green;
4) 5–6 мутаций с restore по md5; 5) отчёт: сделано/red/green/targeted/md5/риски.
Если контракт неясен — остановись и верни вопрос TL, не выдумывай API.
```

## 5. Промт ревьюера (шаблон)

```text
Ты REV-1, автор кода не ты. Read-only. Проверь <ID> против DESIGN, TEST-MATRIX, REPORT,
исходников, gate/mutation-логов и фикстур.
Ищи: неверные границы/ошибки, утечки секретов, выдуманную семантику, подгонку тестов,
несогласованность с RE-дампом, регрессии API, panic/OOM, отсутствие red-first.
Верни: вердикт; P0/P1/P2/P3 (file:line, сценарий, как воспроизвести); подтверждение
targeted/мутаций по логам; остаточные риски. Не редактируй и не предлагай «поправить тест».
```

## 6. Quality feedback loop

- **Mutation catch rate** — обязательная метрика: для каждой карточки 5–10 мутаций, цель
  100% пойманных; не пойманная мутация = дыра в тесте или мёртвый код.
- **Red-first** — compile-red допустим для нового API, но после green нужны поведенческие
  границы; лог red хранится в `waves/<ID>/logs/`.
- **Independent oracle** — фикстура/дамп/`jq`/md5, а не вывод текущей реализации.
- **P0/P1/P2 = 0** до accepted; P3 не блокирует, но попадает в backlog.
- **Lessons** — после принятой волны 3–5 проверяемых уроков в `CODE-STANDARD.md` §9; повтор
  двух волн повышается в правило.
- **Docs once** — `STATE/STATUS/ROADMAP/TASKS/map` синхронизируются один раз после accepted.

## 7. Ограничения

Не запускать Codex-воркеров на `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep` с
правом записи; не давать `danger-full-access`; не отправлять секреты в промт; не использовать
живой Discord/сеть без approval владельца. Codex CLI вызывает модель по сети — это отдельное
разрешение владельца, как и любой сетевой запуск.
