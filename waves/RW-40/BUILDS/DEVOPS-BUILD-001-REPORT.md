# DevOps-отчёт по сборке `BUILD-001` — проверка, мониторинг, продвижение и откат

- **Карточка:** `devops/task-115-build-001-testing` (DevOps / Интегратор).
- **Предмет:** `waves/RW-40/BUILDS/BUILD-001.md` в централизованной ветке `testing`.
- **Дата и окно:** 2026-10-30, 19:55–20:25 +05:00.
- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748` («RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence», committer date 2026-10-24T16:22:06+05:00).
- **Рабочее дерево:** грязное — 50 изменённых отслеживаемых файлов и 145 неотслеживаемых (см. §6, D-1).
- **Режим:** продуктовый код, тесты и golden-каталоги только чтение. Сборка шла в `/tmp/opencode` и `/home/sanya/.cache/build001-target`; `git commit/push` не выполнялся. Созданы этот отчёт и каталог `BUILDS/logs/` с логами прогонов.
- **Артефакты прогонов:** `waves/RW-40/BUILDS/logs/` (`quick-ci-build-001.log`, `meta-full-build-001.log`, `unit-sets-build-001.log`, `gate-worker-build-001.log`, `cargo-contracts-build-001.log`, `rebuild-release.log`, `rebuild-from-head.log`), `.gate/worker-2026-10-30T15-19-12-481Z.json`.

## 1. Вердикт

**Сборка `BUILD-001` воспроизводима по исходникам, но не воспроизводима как артефакт и не готова к продвижению в `beta`/`main`.** Быстрые проверки зелёные (meta 34/34, оба `tsc` — код 0), полный `gate:worker` даёт `passed_with_skips` (879/879 unit, 1142/1142 Rust pass + 12 ожидаемых ignored, 21/22 smoke), контрактные тесты Rust из `BUILD-001` §2 проходят 35/35. При этом:

- закоммиченный бинарь `rust/dist/discord-rs` построен **до** HEAD (mtime 2026-10-21, коммит 2026-10-24) и не совпадает с пересборкой из HEAD;
- `rust/dist/SHA256SUMS` **неверен**: строка SBOM не сходится с закоммиченным `sbom.spdx.json`, `sha256sum -c` падает;
- рабочее дерево `testing` не соответствует ни одному коммиту, поэтому «продвигать» сейчас нечего без предварительной фиксации;
- в окружении нет браузера и live-endpoint, поэтому `gate:release`/`gate:qualify` не выполнялись и выполниться не могут — это блокер выкладки, а не «skip»;
- версия `0.1.0` уже занята тегом `v0.1.0` (коммит `4188bd0`, 2026-10-18), новый тег под `BUILD-001` без bump невозможен.

Итог: **условно годен для QA в `testing`; продвижение в `beta` — только после закрытия D-1…D-3 и прогона `gate:release`/`gate:qualify` в среде с браузером.**

## 2. Выполненные проверки

| # | Команда | Результат | Лог |
|---:|---|---|---|
| 1 | `node --test tests/meta/test-policy.test.mjs tests/meta/ipc-path.test.mjs tests/meta/plan-sync.test.mjs` | 34 теста: pass 34, fail 0, skip 0; код 0 | `logs/quick-ci-build-001.log` |
| 2 | `npx tsc -p tsconfig.json` | код 0, вывод пуст | там же |
| 3 | `npx tsc -p rust/gui-spike/ui/tsconfig.json` | код 0, вывод пуст | там же |
| 4 | `node --test --test-concurrency=1 'tests/meta/**/*.test.mjs'` (сверка с `BUILD-001` §2.1: «102/102») | 102 теста: pass 102, fail 0, skip 0 | `logs/meta-full-build-001.log` |
| 5 | `node --test` по пяти unit-наборам из `BUILD-001` §2.3 | 183 теста: pass 183, fail 0, skip 0 | `logs/unit-sets-build-001.log` |
| 6 | `npm run gate:worker` | вердикт `passed_with_skips`; meta 102/102, unit 879/879, `spike-rust` ok, `workspace-rust` 1154 (1142 pass, 12 ignored), smoke 22 (21 pass, 1 skip) | `logs/gate-worker-build-001.log`, `.gate/worker-2026-10-30T15-19-12-481Z.json` |
| 7 | `cd rust/gui-spike && cargo test --offline --test notifications_contract --test ipc_wiring_contract --test attachments_contract` | 35 тестов: 23 + 8 + 4, fail 0 | `logs/cargo-contracts-build-001.log` |
| 8 | `bash scripts/ci.sh --dry-run` | план CI корректен, все 7 шагов перечислены, код 0 | вывод в сессии |
| 9 | `node scripts/sbom.mjs --check --output …` (двукратно) | SPDX-2.3, 100 packages, sha256 `71900e99…f5a5` оба раза — детерминирован | `/tmp/opencode/sbom-a.json` |
| 10 | `bash scripts/release.sh --output /tmp/opencode/dist-rebuild` | сборка 11,9 с, бинарь 445 072 Б, sha256 `3a9be209…500b` | `logs/rebuild-release.log` |
| 11 | повторная сборка в тот же путь + сборка из чистого `git archive HEAD` | тот же sha256 `3a9be209…500b` — сборка детерминирована и не зависит от абсолютного пути исходников | `logs/rebuild-from-head.log` |

**Не выполнялось и почему:** `npm run gate:release` и `gate:qualify` — нет браузера (`CHROME_BIN`/`CHROME_PATH` не заданы, chromium/chrome/brave в `PATH` нет), fixture-e2e уйдёт в skip, а гейт трактует skip как failed; live-фаза требует `DISCORD_LAB_LIVE_CDP` (окно с живым Discord не согласовано). `npm ci` не запускался, чтобы не пересоздавать общий `node_modules` при параллельных воркерах. `cargo fmt`/`clippy`/`cargo-deny` в этом прогоне не запускались (входят в `scripts/ci.sh`, не в быстрые проверки карточки); `gate:worker` их тоже не содержит.

## 3. Воспроизводимость сборки

### 3.1. Что измерено

| Объект | Значение |
|---|---|
| Коммит HEAD (ветка `testing`) | `4e24048`, 2026-10-24T16:22:06+05:00 |
| Закоммиченный бинарь `rust/dist/discord-rs` | 445 040 Б, mtime **2026-10-21 12:58**, sha256 `8816a414…484c` |
| Пересборка из HEAD (`git archive` → `/tmp/opencode/build-001-src`, offline) | 445 072 Б, sha256 `3a9be209…500b` |
| Пересборка в рабочем дереве (дважды) | тот же `3a9be209…500b` — побайтово совпадает с HEAD-сборкой |
| `rust/dist/SHA256SUMS` | `discord-rs` — совпадает; `sbom.spdx.json` — **не совпадает** |
| Фактический `sbom.spdx.json` | sha256 `71900e99…f5a5`; в `SHA256SUMS` записан `cd9235ee…f65a3` |
| `sha256sum -c SHA256SUMS` | `discord-rs: ЦЕЛ` / `sbom.spdx.json: ПОВРЕЖДЁН`, код 1 |
| Проверка `--version` | CLI `discord-lab --version` → `0.1.0`; Rust `discord-rs --version` печатает баннер P0, не semver |

Вывод по детерминизму: релизная сборка **детерминирована при одинаковых исходниках**: две сборки из рабочего дерева и одна из чистого архива HEAD дали один и тот же sha256, несмотря на разные абсолютные пути. Значит, расхождение с закоммиченным артефактом объясняется не флагами сборки, а тем, что артефакт собран из другого (более старого) состояния дерева.

### 3.2. Разрывы воспроизводимости

1. **Артефакт не прослеживается до коммита.** Бинарь в `rust/dist/` датирован 2026-10-21, HEAD — 2026-10-24. Между ними 24 коммита от тега `v0.1.0`. Привязки «артефакт ↔ коммит» в `BUILD-001.md` нет: указана только ветка.
2. **Контрольные суммы расходятся.** `SHA256SUMS` содержит устаревший хеш SBOM, поэтому проверка целостности релизного каталога всегда красная.
3. **Toolchain не закреплён.** `rust-toolchain.toml` в репозитории нет; CI использует `dtolnay/rust-toolchain@stable` (плавающий stable), локально `cargo 1.99.0` / `rustc 1.99.0` / `rustfmt 1.10.0-stable`. MSRV в `rust/Cargo.toml` — `1.87`, это не пин. Любой релиз на другом stable может дать другой бинарь.
4. **Node не закреплён в репозитории.** `engines.node >= 24`, CI ставит `node-version: "24"`, локально `v26.13.0` (nvm). `.nvmrc` нет. Проверки зелёные на 26, но CI прогоняет 24 — расхождение версий не покрыто.
5. **Версия пакета не менялась.** `package.json` = `rust/Cargo.toml` = `0.1.0`, а `v0.1.0` уже указывает на `4188bd0` от 2026-10-18. Продвижение `BUILD-001` требует нового номера (минимум `0.2.0`) и нового тега.
6. **`scripts/release.sh` игнорирует `CARGO_TARGET_DIR`.** Скрипт жёстко берёт `rust/target/release/discord-rs`; при внешнем target-dir шаг `install` падает с «не найден бинарь» (воспроизведено: код 1), хотя `cargo build` завершился успешно. Это ломает CI-кэш и любые вынесенные target-каталоги.
7. **Грязное дерево.** 50 изменённых отслеживаемых файлов (включая `rust/crates/discord-core/src/{lib,dispatch}.rs`, `rust/crates/discord-voice/src/lib.rs`, 11 фронтенд-модулей, 16 тестов) и 145 неотслеживаемых, из них 19 под `rust/` (14 Rust-исходников и тестов + 5 UI-модулей). Сборка из такого дерева не соответствует ни одному коммиту; `BUILD-001.md` при этом утверждает, что сборка сделана из `testing`. Замечание: пересборки из грязного дерева и из чистого HEAD совпали побайтово — добавленный код (например, `pub mod stream;`) в бинарь `discord-app` не попадает из-за отсечения неиспользуемого кода. Это удобно для проверки, но означает, что бинарь **не доказывает** наличие новых срезов.

### 3.3. Что нужно, чтобы артефакт стал воспроизводимым

- После приёмки — один коммит на `testing`, `git status` чист (PROCESS §3.10 preflight).
- Добавить `rust-toolchain.toml` (`channel = "1.99.0"`, компоненты `rustfmt`, `clippy`) и `.nvmrc` (`24`) — или явно записать в отчёте, что версии не пинятся.
- Собирать артефакт одним шагом: `bash scripts/release.sh`, затем `sha256sum -c rust/dist/SHA256SUMS`.
- Записывать в `BUILD-001.md` коммит, sha256 бинаря и SBOM, версию toolchain/node.
- Починить `release.sh` (`BIN="${CARGO_TARGET_DIR:-$RUST_DIR/target}/release/discord-rs"`) либо запретить внешний target-dir явной проверкой.
- Для побайтовой воспроизводимости между машинами добавить `--remap-path-prefix` (иначе путь исходников может просочиться в незастрипленный бинарь).

## 4. Целостность окружения

### 4.1. Версии и инструменты (фактические)

| Компонент | Значение | Требование/CI |
|---|---|---|
| Node | `v26.13.0` (nvm) | `engines >= 24`; CI — `24` |
| npm / npx | `11.20.0` | lockfileVersion `package-lock.json` |
| Cargo | `1.99.0 (5f94df478 2026-08-27)` | CI — stable (плавающий) |
| rustc | `1.99.0 (b940084d7 2026-09-28)` | MSRV `1.87` |
| rustfmt | `1.10.0-stable` | есть |
| secret-tool | `/usr/bin/secret-tool` | нужен live-keyring-фазе |
| Браузер | **нет** (`CHROME_BIN`/`CHROME_PATH` пусты, chromium/chrome/brave в `PATH` нет) | обязателен для `qa-browser`/`gate:release` |
| `DISCORD_LAB_LIVE_CDP` | не задан | обязателен только для `gate:qualify` (T3) |

### 4.2. Переменные окружения

| Переменная | Где читается | Обязательность | Текущее значение | Последствие отсутствия |
|---|---|---|---|---|
| `CHROME_BIN` / `CHROME_PATH` | `scripts/gate.mjs` (`browserPreflight`), `tests/meta/ci-browser.test.mjs` | для `release`/`qualify` | unset | e2e-фаза `failed`, вердикт не `passed` |
| `DISCORD_LAB_LIVE_CDP` | `scripts/gate.mjs` (`e2eLivePhase`) | только `qualify` | unset | фаза `unmet`, qualification не выдаётся |
| `DISCORD_LAB_KEYRING_LIVE` | `scripts/gate.mjs` (`keyringLiveEnv`), `rust/crates/discord-platform/src/keyring.rs` | ставит сам гейт (`=1`) | unset | live-кейсы k01–k12 остаются ignored (12 шт.) |
| `CDP_ENDPOINT` / `CDP_TARGET` | `scripts/cdp.mjs` | ручные live-прогоны | unset | live-команды недоступны |
| `SOURCE_DATE_EPOCH` | `scripts/sbom.mjs` | нет (дефолт `0`) | unset | SBOM всё равно детерминирован |
| `SBOM_TARGET` | `scripts/sbom.mjs` | нет | unset | берётся host target |

Проверено: ни одна из этих переменных не влияет на быстрые проверки и `gate:worker`; в CI `DISCORD_LAB_LIVE_CDP` намеренно снимается через `env -u`, а `qa-live` отключён (`if: ${{ false }}`). Секретов в логах и отчётах нет.

### 4.3. Зависимости и lock-файлы

| Файл | md5 |
|---|---|
| `package-lock.json` | `f3f697bac7ec91a68946b807fce953a4` |
| `rust/Cargo.lock` | `9a1d126aeadf20e8a42c569bfedfd931` |
| `rust/gui-spike/Cargo.lock` | `7d66c56ef6f7e6e06cb7885a7d9a4bb7` |

`npm ls --depth=0` сходится с `package.json`: `frida@17.23.2`, `@types/frida-gum@19.12.0`, `@types/node@24.19.2`, `typescript@7.0.2`. Прямых зависимостей вне lock-файлов нет. `rust/gui-spike` — отдельный workspace, поэтому `BUILD-001` §2.4 требует заходить в `rust/gui-spike` (см. §6, D-7).

## 5. Целостность логов и CI-скриптов

### 5.1. Контрольные суммы CI-контура

| Файл | md5 | Примечание |
|---|---|---|
| `scripts/ci.sh` | `fb5ad676b5184d0f2b374e520f590e98` | 7 шагов, офлайн по умолчанию, `--dry-run` работает |
| `scripts/release.sh` | `29b5d5bb67e3d61956c0422c9181fe0d` | игнорирует `CARGO_TARGET_DIR` (D-6) |
| `scripts/gate.mjs` | `89c67ad0a8f087b87626090b2b5899d9` | golden-фаза молча пропускается без манифеста (D-10) |
| `scripts/sbom.mjs` | `a78ed7f90efca742a17d480937c4c6c4` | детерминирован, 100 packages |
| `.github/workflows/ci.yml` | `608e26837c10996aae11d2c29fc6d320` | триггер только `push: [main]` + PR; `testing` не проверяется (D-8) |
| `tsconfig.json` | `72f7720d495af744dbc7674cc3ed20ad` | — |
| `rust/gui-spike/ui/tsconfig.json` | `d0dd2b8bc3f44f39651ae5fc91071b51` | — |

### 5.2. Найденные расхождения в логах

1. **Логи указывают на другой корень репозитория.** `waves/RW-40/QUALITY/gate-worker-wave9.log` и соседние ссылаются на `/home/sanya/orca/project/.gate/worker-2026-10-30T14-36-10-163Z.json`, тогда как сборка идёт в `/home/sanya/orca/workspaces/project/testing`. Префикс `/home/sanya/orca/project` встречается 4607 раз в `waves/`, `docs/`, `tests/` (`plan-sync.test.mjs:137`, `test-policy.test.mjs:139` и др.). По этим путям артефактов гейта в текущем дереве нет.
2. **`.gate/` в `testing` отсутствовал до этого прогона** — единственный отчёт гейта теперь `worker-2026-10-30T15-19-12-481Z.json`. Каталог в `.gitignore`, поэтому в репозиторий не попадает; при продвижении отчёт нужно приложить к PR/волне вручную.
3. **Ссылки на `/tmp` мертвы.** `waves/window-runs.jsonl` (13 записей) ссылается на PNG-снимки в `/tmp`; на месте 5 из 13, остальные 8 отсутствуют. Сами записи содержат метрики (`chatRenderedRows 30`, `memberRenderedRows 22`, `settingsToggles 38`), но доказательство окна недолговечно.
4. **Smoke skip — не «зелёно».** `gate:worker` пропустил 1 smoke-тест: «полный реальный voice-лог: stdout и CSV совпадают с независимым оракулом» — нет `logs/calls-voice-28170.jsonl`. Причина: golden-каталоги `/logs/`, `/functions/`, `/evidence/` в этом workspace отсутствуют (в `.gitignore`, в дерево не попадают). Это ровно тот случай, когда «пропуск прохождением не считается», и он попал в вердикт `passed_with_skips`, а не `passed`.
5. **Golden-сверка может не выполняться молча.** В `scripts/gate.mjs` при отсутствии `.gate/golden.manifest` фаза `golden` возвращает `ok: true` с пометкой «сверка пропущена». В `release` это скрывает отсутствие golden-данных; для `release`/`qualify` расхождение стоит трактовать как `unmet`.
6. **CI не видит `testing`.** `.github/workflows/ci.yml` запускается на `push` в `main` и на PR. Пуш в `testing`/`beta` CI не запускает, значит `BUILD-001` в `testing` не проверен ни одной джобой; `qa-browser` и `package` отработают только после промоушена в `main` — то есть уже после решения о продвижении.

## 6. Дефекты и риски

| ID | Уровень | Дефект | Доказательство | Действие |
|---|---|---|---|---|
| D-1 | **P0** | Рабочее дерево `testing` не соответствует коммиту: 50 M + 145 ?? (Rust, тесты, UI) | `git status --porcelain`, 195 строк | Зафиксировать принятые изменения одним коммитом, остальное — в `waves/`/stash; до этого продвижение запрещено |
| D-2 | **P0** | `rust/dist/SHA256SUMS` неверен для SBOM; `sha256sum -c` код 1 | §3.1 | Пересобрать артефакт и `SHA256SUMS` одним прогоном `release.sh`, приложить вывод проверки |
| D-3 | **P0** | Закоммиченный бинарь старше HEAD и не совпадает с пересборкой | mtime 2026-10-21 vs коммит 2026-10-24; sha256 `8816a414…` vs `3a9be209…` | Пересобрать из зафиксированного коммита, записать hash и коммит в `BUILD-001.md` |
| D-4 | P1 | Нет `rust-toolchain.toml` и `.nvmrc`; CI на плавающем stable и Node 24, локально Node 26 | §4.1, §3.2 | Запинить toolchain и Node либо зафиксировать политику «проверяем на двух версиях» |
| D-5 | P1 | `gate:release`/`gate:qualify` не выполнены: нет браузера и live-env | §2 | Прогнать в среде с `CHROME_BIN` и согласованным окном `DISCORD_LAB_LIVE_CDP` |
| D-6 | P1 | `release.sh` игнорирует `CARGO_TARGET_DIR`, падает на install | воспроизведено, код 1 | Учесть переменную в пути к бинарю |
| D-7 | P1 | Инструкция `BUILD-001` §2.4 неверна: `-p discord-gui-spike` не находится из `rust/` (отдельный workspace) | `error: package ID specification discord-gui-spike did not match any packages` | Заменить на `cd rust/gui-spike && cargo test --offline --test …` |
| D-8 | P1 | CI не запускается на `testing`/`beta` | `.github/workflows/ci.yml` | Добавить ветки или гонять `scripts/ci.sh` перед промоушеном вручную |
| D-9 | P1 | Golden-каталоги отсутствуют, smoke skip 1, golden-фаза может молча пройти | §5.2.4–5 | Подложить golden-данные на машину сборки; для release/qualify считать отсутствие манифеста `unmet` |
| D-10 | P2 | Версия 0.1.0 занята тегом `v0.1.0` (коммит `4188bd0`) | `git rev-list --count v0.1.0..HEAD` = 24 | Поднять версию до 0.2.0 перед тегом |
| D-11 | P2 | Логи ссылаются на старый корень `/home/sanya/orca/project` и на удалённые `/tmp`-файлы | §5.2.1, §5.2.3 | Нормализовать пути в логах, складывать снимки в `waves/` |
| D-12 | P2 | `discord-rs --version` печатает баннер P0, а не версию | §3.1 | Добавить `--version` с semver — иначе проверка «версия = тег» невозможна |
| D-13 | P2 | `cargo-deny` не проверялся, `npm ci` не прогонялся | §2 | Прогнать на CI-раннере (`scripts/ci.sh --online`) |

## 7. Процедура продвижения `testing` → `beta` → `main`

Сейчас `beta`, `main`, `unstable`, `testing` и `qa/task-116-qa-build-001` указывают на один коммит `4e24048`. Продвижение — fast-forward без merge-конфликтов, но именно поэтому оно опасно: «зелёная» ветка не доказывает, что артефакт соответствует коммиту (D-1…D-3).

### 7.1. Предусловия (блокирующие)

- [ ] D-1 закрыт: рабочее дерево чистое (`git status --porcelain` пуст), принятые срезы в коммите.
- [ ] `npm run gate:worker` — `passed` (не `passed_with_skips`), лог приложен.
- [ ] `CHROME_BIN` задан, `npm run gate:release` — `verdict: passed`, fixture-e2e без skip.
- [ ] Версия поднята (`package.json`, `rust/Cargo.toml`) и совпадает в обоих местах.
- [ ] `npm run gate:qualify` записан, `node scripts/gate.mjs --verify-qualification .gate/qualification-<v>-<ts>.json` — ok.
- [ ] `sha256sum -c rust/dist/SHA256SUMS` — все строки `OK`.
- [ ] Fallback-пакет/тег предыдущей версии доступен (`v0.1.0` → `4188bd0`; tarball собрать и сохранить).
- [ ] Окно согласовано с владельцем; живые операции — вне его активной работы.

### 7.2. Шаги (выполняет L1/Team Lead; воркер не коммитит и не пушит)

| Шаг | Действие | Команда | Критерий успеха |
|---:|---|---|---|
| 0 | Заморозка: остановить воркеров, собрать принятые изменения | `git status --porcelain` | пусто |
| 1 | Быстрые проверки на `testing` | `node --test tests/meta/{test-policy,ipc-path,plan-sync}.test.mjs`, `npx tsc -p tsconfig.json`, `npx tsc -p rust/gui-spike/ui/tsconfig.json` | коды 0 |
| 2 | Полный worker-гейт | `npm run gate:worker` | `Вердикт: passed`, 0 skip |
| 3 | Release-гейт (нужен браузер) | `CHROME_BIN=/path/chrome npm run gate:release` | `passed`, e2e fixture pass > 0, coverage 90/80/90 |
| 4 | Поднять версию | правка `package.json` + `rust/Cargo.toml`, коммит `release: v0.2.0` | версии равны |
| 5 | Qualification | `npm run gate:qualify`; `node scripts/gate.mjs --verify-qualification .gate/qualification-0.2.0-<ts>.json` | sha256 и коммит совпадают |
| 6 | Собрать fallback | `npm pack --pack-destination /srv/fallback/` для текущей версии | tarball на месте |
| 7 | Продвинуть в `beta` | `git switch beta && git merge --ff-only testing` | указатель `beta` = `testing` |
| 8 | Проверить `beta` | повторить шаги 1–2 на `beta`; при браузере — шаг 3 | зелёно |
| 9 | Выдержать окно `beta` | 24 часа наблюдения по §9; баги — только patch-циклом | нет P0/P1 |
| 10 | Продвинуть в `main` | `git switch main && git merge --ff-only beta`; `git push origin main` | CI-джобы `test`, `qa-browser`, `package` зелёные |
| 11 | Тег | `git tag -a v0.2.0 -m "BUILD-001"` и `git push origin v0.2.0` — только после qualification | тег указывает на проверенный коммит |
| 12 | Проверка артефакта | скачать `discord-rs-linux-x64` из CI, `sha256sum -c`, сверить с журналом | хеши совпадают |

Правила, которые нельзя обходить: тег ставится после qualification (PROCESS §3.12); kit руками не правится — любая правка tarball = новый `gate:qualify` (§3.7); версия = тег = `package.json` (§4.5); `--spawn` и live-прогоны — только вручную и в согласованное окно (§3.10).

### 7.3. Кто что делает

- **L1 / Team Lead:** коммиты, merge, push, тег, запуск `gate:release`/`gate:qualify`, решение о switch.
- **DevOps (эта карточка):** проверки окружения, отчёт, мониторинг, подготовка команд отката.
- **QA:** приёмка fixture-e2e, подтверждение счётчиков и отсутствия skip.
- **Владелец:** окно живых операций и решение о выкладке.

## 8. План отката (rollback plan)

### 8.1. Уровни и триггеры

| Уровень | Когда | Действие | Время |
|---|---|---|---|
| R1. До пуша | merge в `beta`/`main` сделан, но не отправлен | `git switch <branch> && git reset --hard <prev>`; ветка возвращается на `4e24048`/`v0.1.0` | < 1 мин |
| R2. После пуша, до тега | CI красный или найден P0 | `git revert -m 1 <merge-commit>` (или revert конкретных коммитов), push; тег не ставить | 5–10 мин |
| R3. После тега/выкладки | регрессия в проде, P0/P1 | вернуть fallback-пакет и предыдущий бинарь, снять тег, выпустить patch | 15–30 мин |
| R4. Флаги | новая функция мешает, код откатывать не нужно | снять `DISCORD_LAB_LIVE_CDP`, `--spawn`, `CDP_ENDPOINT`; перезапуск | < 5 мин |
| R5. Данные | изменение формата `logs/`/`functions/` | старый код обязан читать новые файлы (PROCESS §6); golden не перезаписывается, откат — только чтением | по обстоятельствам |

Триггеры R1/R2: падение любой быстрой проверки, `gate:release` ≠ `passed`, e2e skip > 0, расхождение sha256, падение CI.
Триггеры R3: падение запуска, регрессия входа/отправки сообщений, утечка секрета, рост PSS выше 300 МБ, зависание окна.

### 8.2. Команды отката

```sh
# R1 — локально, до пуша
git switch beta && git reset --hard 4e240481e59ff4d49732ac8bd5ba08a006363748
git switch main && git reset --hard 4e240481e59ff4d49732ac8bd5ba08a006363748

# R2 — после пуша: обратный коммит, историю не переписываем
git switch main && git revert -m 1 <merge-commit-sha> && git push origin main

# R3 — откат артефакта к предыдущему тегу
git checkout v0.1.0 -- rust/dist/discord-rs rust/dist/SHA256SUMS
sha256sum -c rust/dist/SHA256SUMS        # ожидаем: discord-rs: OK
# для CLI — установка fallback-пакета предыдущей версии
npm i -g /srv/fallback/discord-lab-0.1.0.tgz
discord-lab --version                    # ожидаем 0.1.0

# R3 — снять ошибочный тег (только L1)
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
```

Проверка после отката: `git status` чист; `git rev-parse HEAD` = ожидаемый; `sha256sum -c` = OK; `node bin/discord-lab.mjs inventory --pid 99999999` → код 2; `npm run gate:worker` → `passed`; в `waves/db.jsonl` дописано событие отката с причиной.

### 8.3. Ограничения отката

- Переписывать историю `main` (`reset --hard` + force-push) после пуша **нельзя** — только revert.
- Golden-каталоги `logs/`, `functions/`, `evidence/` не восстанавливаются из git (в `.gitignore`) — резервная копия должна быть на машине до выкладки.
- После `switch` «корректирующий выпуск» = новый patch-номер через полный цикл, а не правка установленного пакета (PROCESS §3.10).
- Артефакт `rust/dist/` в репозитории — не единственный источник: CI кладёт свой (`discord-rs-linux-x64`, хранение 7 дней). После истечения срока откат возможен только из локального fallback.

## 9. Мониторинг после продвижения

| Что | Где смотреть | Норма | Тревога |
|---|---|---|---|
| CI | GitHub Actions, джобы `test`, `qa-browser`, `package` | все зелёные | любой красный → R2 |
| Артефакт | `rust/dist/SHA256SUMS`, `.gate/worker-*.json`, CI-артефакт `qa-browser-gate` | `sha256sum -c` OK, вердикт `passed` | расхождение хеша → R3 |
| Гейты | `.gate/<scope>-<ts>.json`, `waves/RW-40/QUALITY/*.log` | 0 fail, 0 skip в release | skip > 0 → не считать прохождением |
| Runtime GUI | `waves/window-runs.jsonl`, `scripts/gui-spike-bench.mjs` | PSS ≤ 300 МБ, 60 fps, p95 ≤ 18 мс, cold start ≤ 150 мс | рост PSS > 300 МБ, p95 > 25 мс |
| Ошибки запуска | stdout/stderr спайка, `logs/calls-*.jsonl` (если golden подложен) | нет паник/аварийных выходов | любой аварийный выход → R3 |
| Live-контур | `DISCORD_LAB_LIVE_CDP`-сессия, `tests/e2e/live-discord.e2e.test.mjs` | pass, без ручных вмешательств | падение → снять флаг (R4) |

Каденция: первый час — после каждого шага промоушена; первые 24 часа — раз в 4 часа (гейт + артефакт + окно); далее — на закрытии волны. Логи и снимки складывать в `waves/RW-40/BUILDS/logs/`, а не в `/tmp` (D-11). Владелец метрик — DevOps; решение об откате принимает L1 по триггерам §8.1.

## 10. Чек-лист выпуска (PROCESS §4.5) — текущий статус

- [ ] `gate:release` зелёный на слитом коммите — **не выполнен** (нет браузера).
- [ ] `gate:qualify` записан; `--verify-qualification` совпадает — **не выполнен** (нет live-env).
- [ ] fallback-tarball предыдущей версии на месте — **отсутствует**, `v0.1.0` есть только как тег.
- [ ] план выкладки и откат заполнены — **да**, §7 и §8.
- [ ] версия = тег = `package.json`; `git status` чист — **нет**: 0.1.0 = `v0.1.0` (старый коммит), дерево грязное.

## 11. Приложение: воспроизведение проверок

```sh
cd /home/sanya/orca/workspaces/project/testing
node --test tests/meta/test-policy.test.mjs tests/meta/ipc-path.test.mjs tests/meta/plan-sync.test.mjs
npx tsc -p tsconfig.json && npx tsc -p rust/gui-spike/ui/tsconfig.json
npm run gate:worker
( cd rust/gui-spike && cargo test --offline --test notifications_contract --test ipc_wiring_contract --test attachments_contract )
bash scripts/ci.sh --dry-run
node scripts/sbom.mjs --check --output /tmp/opencode/sbom-a.json && sha256sum /tmp/opencode/sbom-a.json
bash scripts/release.sh --output /tmp/opencode/dist-rebuild && sha256sum /tmp/opencode/dist-rebuild/discord-rs
( cd rust/dist && sha256sum -c SHA256SUMS )   # сейчас: sbom.spdx.json ПОВРЕЖДЁН
```

Ожидаемые значения на момент отчёта: meta 34/34 и 102/102, unit 183/183 и 879/879, Rust workspace 1142 pass + 12 ignored, контракты 35/35, SBOM `71900e99…f5a5`, пересборка `3a9be209…500b`. Любое расхождение — повод перечитать §6 и не продвигать сборку.
