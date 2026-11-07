# REPORT — task-278 (`DEVOPS-P1-08`: актуализация манифеста `BUILD-003.md` под `PR-125..PR-155`)

- **batch/карточка:** RW-40 / `DEVOPS-P1-08` / `task-278`, dispatch `ctx_3969795149d7`, ветка `devops/task-278-build-003-manifest-sync`.
- **Основание:** `P2-2` из `waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md` и блокер §4 из `waves/RW-40/G6/task-273-release-notes-005/RELEASE-NOTES-005.md`.
- **Тип работы:** документация. Продуктовый код, тесты и конфиги не менялись, `git commit` не выполнялся.

## Сделано

1. `waves/RW-40/BUILDS/BUILD-003.md` переписан под фактическое содержимое `testing` = `d047fe4` (`Build-003 sync: merge PR-153 (bff_domains_router) and PR-154 (composer-popovers-bridge)`):
   - шапка с HEAD, окном `ebdb966..d047fe4`, 31 PR в 22 merge-коммитах, датой редакции и статусом `In Testing`;
   - §1 — счётчики ветки, таблица 8 Rust BFF-модулей (включая `bff_domains_router.rs`, `PR-153`), таблица 23 UI-модулей (включая мосты `shell-controllers-registry`, `chat-helpers-bridge`, `composer-popovers-bridge`, `app-shell-wiring`) и матрица всех 31 PR с карточкой, merge-коммитом, артефактом, тест-оракулом, номером ревью и состоянием подключения;
   - §2 — команды QA: `T0` impact, 23 unit-набора, целевой `tsc`, 8 контрактов Rust окна + регрессия + workspace-крейты, требования к отчёту;
   - §3 — корректные пути оракулов: `/home/sanya/re-deep/...` и `fixtures/gateway/by-type/002-READY.json` вместо несуществующего `logs/golden/gateway/002-READY.json` (замечание `P2-5` прошлой редакции);
   - §4 — известные ограничения (`P1` разрывы IPC/монтирования, `P0` корневой `typecheck`, отставание `beta`/`main`), §5 — команды воспроизводимости, §6 — MD5 всех 62 файлов состава и история редакций.
2. `waves/RW-40/DEVOPS/task-278-build-003-manifest-sync/REPORT.md` — этот отчёт.

## Red — состояние до правки

Источник — версия манифеста в `testing` (авторитетное «до»), `md5 61a925d2463e35984b1e29aad5c4d2b7` (совпадает с записью QA):

```sh
git show testing:waves/RW-40/BUILDS/BUILD-003.md > /tmp/opencode/build-003-before.md
md5sum /tmp/opencode/build-003-before.md
grep -c 'PR-1[2-5][0-9]' /tmp/opencode/build-003-before.md        # 0
grep -c 'PR-0[89]\|PR-1[012]' /tmp/opencode/build-003-before.md   # 6
grep -c 'bff_domains_router\|composer-popovers-bridge\|app-shell-wiring'  # 0 на каждый
grep -c 'logs/golden/gateway/002-READY.json'                      # 1 (путь отсутствует в репозитории)
```

Вывод: манифест не содержал ни одного PR окна, ни трёх новых модулей, ни верного пути к фикстуре `READY`. `exit 1` для проверки состава (ожидаемые строки отсутствуют).

## Green — состояние после правки

```sh
f=waves/RW-40/BUILDS/BUILD-003.md
grep -cE '^\| `PR-1[2-5][0-9]`' $f    # 31 (строка матрицы на каждый PR окна)
grep -c 'bff_domains_router' $f       # 7
grep -c 'composer-popovers-bridge' $f # 9
grep -c 'app-shell-wiring' $f         # 9
grep -c 'fixtures/gateway/by-type/002-READY.json' $f  # 1
grep -c 'manifest-path rust/gui-spike/Cargo.toml' $f  # 2 команды
grep -c 'manifest-path rust/Cargo.toml' $f            # 3 (1 в тексте + 2 команды)
md5sum $f                             # db6ffc0462a6517b22894eafb363651e
```

Все `grep` дают ожидаемые ненулевые значения, `exit 0`.

## Targeted-проверки утверждений манифеста (не QA-вердикт)

Прогон на worktree, содержимое которого по `tests/unit/`, `rust/gui-spike/{src,ui/features,tests}` побайтово равно `testing` (`git diff --stat testing HEAD -- <эти пути>` пуст):

| Проверка | Команда | Результат |
|---|---|---|
| Unit 23 набора | `node --test` по списку §2.2 | `tests 236`, `suites 46`, `pass 236`, `fail 0`, `skipped 0`, `exit 0` |
| Типы UI | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | пустой вывод, `exit 0` |
| Корневой typecheck | `npx tsc -p tsconfig.json --noEmit` | `exit 1`, 149 `error TS` в 26 файлах (зафиксировано как `P0` в §4.2 манифеста) |
| Test-таргеты Rust | `cargo metadata --manifest-path rust/gui-spike/Cargo.toml --no-deps` | все 8 имён `*_contract` присутствуют (`automod`, `bff_domains_router`, `expressions`, `onboarding`, `slash_commands`, `soundboard`, `stage_instances`, `webhooks`) |
| Состав ветки | `git ls-tree` по `testing` | 43 теста (40 `*_contract.rs`), 114 unit, 97 features, 38 `src/*.rs` (36 модулей), 52 `#[tauri::command]`, 35 `pub mod` |
| Разрыв интеграции | `git grep` по `rust/gui-spike/ui/**` | `app-shell-wiring` только в своём файле; `app.js` (27 `import`) его не импортирует; `SHELL_MODAL_LAYERS` = 10 слоёв |

## Oracle

- Git-оракул: `testing` = `d047fe4`, `beta` = `e91acf6`, `main` = `4e24048`; окно `ebdb966..testing` (22 коммита, 31 PR) — `git log --format=%s`.
- `waves/RW-40/G1/task-269-be-bff-domains-router/REVIEW-182A.md` — `APPROVED`, `P0=0`, `P1=0`, `5/5` мутаций.
- `waves/RW-40/G2/task-270-ui-composer-popovers-bridge/REVIEW-183A.md` — `APPROVED`, `P0=0`, `P1=0`, `5/5`.
- `waves/RW-40/G2/task-271-ui-app-shell-wiring/REVIEW-184A.md` — `APPROVED`, `P0=0`, `P1=0`, `5/5`.
- RE-дамп `/home/sanya/re-deep/`: проверено существование `gateway-protocol.md`, `gateway-schema.md`, `rest-d02.md`, `frontend-d03.md`, `i18n-deep.md`, `backend-ipc.md`, `main-arch.md`, `voice-deep.md`, `capture-ready.jsonl`, `decoded/`, `decoded-index.json`, `d02-snapshot/`, `ui/`, `ui2..ui6`, `ui8`, `voice/ui7`, `ru-catalog.json`, `en-catalog.json`.
- Локальная фикстура `fixtures/gateway/by-type/002-READY.json` существует в `testing` (`git cat-file -e` → `0`).

## Генерация / мутации / baseline

- Генерация: не применимо (doc-only, генераторов нет).
- Мутации: не применимо (нет продуктового кода); мутационные прогоны `5/5` взяты из ревью `182A`/`183A`/`184A` и не переигрывались.
- Baseline/restore: файл не отслеживается в `main`-дереве (`waves/RW-40/BUILDS/` — `??`), поэтому восстановление сводится к правке одного файла; продукт и тесты не изменялись (`git status` по `tests/`, `rust/` чист относительно `testing`).
- Стабильность: unit-прогон выполнен один раз на этом срезе; счётчик `236 = 214 (QA на 11070f0) + 22 (2 новых набора)` сходится, повторы не требовались.

## gate:worker

Не запускался: изменены только `.md`-документы вне продуктового дерева, `npm run gate:worker` к ним не относится. Корневой `typecheck` красный независимо от этой задачи (149 ошибок на `d047fe4`, фикс — `task-277`/`FE-FIX-02`), это записано в манифест §4.2 как ограничение сборки.

## Файлы и MD5

```text
db6ffc0462a6517b22894eafb363651e  waves/RW-40/BUILDS/BUILD-003.md
waves/RW-40/DEVOPS/task-278-build-003-manifest-sync/REPORT.md   # md5 не хешируется в себя: снимать командой md5sum
```

MD5 манифеста снят после последней правки; проверка: `md5sum waves/RW-40/BUILDS/BUILD-003.md`.

## Риски и отклонения от ТЗ

1. **Отклонение по `--manifest-path`.** ТЗ просит контрактные тесты Rust «с `--manifest-path rust/Cargo.toml`». Крейт `discord-gui-spike` объявляет собственный `[workspace]` (`rust/gui-spike/Cargo.toml:10`) и в workspace `rust/Cargo.toml` (`members = ["crates/*"]`) не входит — `cargo test --manifest-path rust/Cargo.toml --test automod_contract` не резолвится. Поэтому в манифесте два явных манифеста: `rust/gui-spike/Cargo.toml` для 8 новых контрактов и BFF-регрессии, `rust/Cargo.toml` для крейтов `rust/crates/*` и `-p discord-voice`. Если требуется буквально одна команда для всего, нужен `--workspace`-рефакторинг манифеста — это отдельная карточка.
2. **`P2`. Номера ревью `182A`/`183A`/`184A`** взяты из `REVIEW-*` документов `task-269/270/271`; в `TASKS.md` эти карточки не заведены, поле «Ревью» сверять по файлам.
3. **`P2`. Отчёт QA не покрывает новый хвост.** `QA-BUILD-003-REPORT.md` относится к `11070f0` и 21 набору; наборы `composer-popovers-bridge`, `app-shell-wiring` и 8 Rust-контрактов в вердикт не входят. В §2.6 манифеста поставлено требование повторного прогона на `d047fe4`.
4. **`P1`. `T2`-сценарии недостижимы** до Фазы 0 (`RELEASE-NOTES-005.md` §4): 0 `#[tauri::command]` у 8 BFF-доменов, 0 потребителей у 23 UI-модулей. Манифест это фиксирует, чтобы QA не завёл ложные дефекты «функции нет».
5. **`P2`. `beta` (`e91acf6`) и `main` (`4e24048`) отстают на всю волну** — перенос и откат не репетированы.

## Следующий шаг

Передать манифест на приёмку Team Lead → PM (сверка состава `PR-125..PR-155`) → G4 QA для повторного прогона `T0`/`T1` на `d047fe4` и выпуска новой редакции `QA-BUILD-003-REPORT.md`. `P0-1` (корневой `typecheck`) остаётся блокером `gate:worker` и закрывается карточкой `task-277`.
