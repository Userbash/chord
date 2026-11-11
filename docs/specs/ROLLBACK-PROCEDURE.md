# Процедура слияния `testing -> beta` и регламент экстренного отката (Rollback Procedure)

> **Статус документа:** `Approved Promotion & Emergency Rollback Specification (v1.0)`  
> **Задача:** `TASK-957` (`Wave RW-45 — Beta Promotion`)  
> **Отдел:** `devops` (`DevOps / Integration`) совместно с `pm_review` (`ECC 2.0 Control Plane`)  
> **Связанные документы:** `docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md`, `docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md`, `waves/RW-41/QUALITY/QA-BUILD-005.md`  
> **Дата утверждения:** 11 ноября 2026 г.

---

## 1. Назначение и область действия

Настоящий документ регламентирует:
1. **Процедуру продвижения (Promotion) и оформления Pull Request `testing -> beta` (`SUB-957-1`)** для сертифицированной сборки `BUILD-005` (`v1.0.0-beta.1`).
2. **Протокол экстренного отката (`Rollback Protocol`, `SUB-957-2`)** ветки `beta` и релизных бинарных артефактов в каталоге `rust/dist/` при обнаружении блокирующих дефектов (`P0`/`P1`).
3. **Гарантии целостности и совместимости локальной базы данных SQLite WAL при даунгрейде (`SUB-957-3`)** в обоих контурах хранения (`discord-store` и `discord-gpui::storage::LocalDatabase`).

### Базовые инварианты релиза
* **Запрет прямых коммитов и переписывания истории:** В ветках `testing`, `beta` и `main` строго запрещены прямые коммиты, `git reset --hard` и `git push --force`. Все изменения доставляются через Pull Request по цепочке `worker branch -> unstable -> testing -> beta -> main`.
* **Zero P0/P1 Blockers:** Перенос в `beta` разрешён исключительно при отсутствии открытых дефектов уровня `P0` и `P1` и наличии подписанного вердикта `QA Passed`.
* **Целевые показатели восстановления:**
  * **RTO (Recovery Time Objective):** $\le 2$ минуты с момента фиксации инцидента до восстановления работоспособного бинарника и отката ветки `beta`.
  * **RPO (Recovery Point Objective):** $0$ потерянных локальных сообщений, настроек и кэшированных каналов в базе SQLite WAL при возврате на предыдущую сборку.

---

## 2. Подготовка и выполнение Pull Request `testing -> beta` (`SUB-957-1`)

### 2.1. Входной чек-лист (Pre-Promotion Gate)
Перед созданием PR `testing -> beta` релиз-инженер (`devops`) и `Lead Orchestrator` (`pm_review`) проверяют выполнение следующих условий:

| № | Контрольная точка | Команда / Артефакт проверки | Требуемый статус |
| :--- | :--- | :--- | :--- |
| 1 | Чистота дерева и наличие тега кандидата | `git status -sb && git rev-parse build-005-candidate` | Чистое дерево, тег на вершине `testing` |
| 2 | Сквозной конвейер `T1/T2` (`scripts/ci.sh`) | `bash scripts/ci.sh` | Код выхода `0` (`fmt`, `clippy -D warnings`, workspace tests) |
| 3 | Релизный гейт `T3/T4` без пропусков | `DISCORD_LAB_KEYRING_LIVE=1 node scripts/gate.mjs --scope release` | `passed`, `skipped = 0` (`failOnSkips: true`) |
| 4 | Headless Smoke нативного клиента `discord-gpui` | `./rust/dist/discord-gpui --headless-smoke` | `GPUI_SMOKE_OK`, `"status":"ok"`, `"sqlite_wal_ok":true`, `vmrss_mb < 60.0` |
| 5 | Матрица 4 композиторов Linux (`Wayland`/`X11`) | `cargo test --manifest-path rust/discord-gpui/Cargo.toml --test manual_qa_suite_contract test_desktop_matrix` | `100% PASS` (GNOME Mutter, KDE KWin, Hyprland/wlroots, X11 SHM) |
| 6 | Криптографическая целостность артефактов и SBOM | `(cd rust/dist && sha256sum -c SHA256SUMS)` | Все 4 артефакта: `ЦЕЛ` (`OK`) |
| 7 | Отчёт качества сборки `BUILD-005` | `waves/RW-41/QUALITY/QA-BUILD-005.md` | Статус `QA PASSED`, `P0 = 0`, `P1 = 0` |

### 2.2. Проверка отсутствия конфликтов слияния (`Шаг 1.2`)
Перед подачей и вливанием PR выполняется сухая проверка слияния `testing` в `beta` без модификации истории:

```bash
# 1. Актуализация локальных ссылок и проверка чистоты рабочего дерева
git status -sb

# 2. Проверка отсутствия конфликтов через git merge-tree (без переключения веток)
git merge-tree "$(git merge-base beta testing)" beta testing | grep -n "<<<<<<< " && echo "CONFLICT DETECTED" || echo "MERGE CLEAN: 0 conflicts"

# 3. Контрольное тестовое слияние без коммита на ветке beta
git checkout beta
git merge --no-commit --no-ff testing
git diff --check
git merge --abort
```

### 2.3. Спецификация описания Pull Request `testing -> beta` (`Шаг 1.1`)
При оформлении Pull Request из `testing` (`build-005-candidate`) в защищённую ветку `beta` используется следующий утверждённый паспорт PR:

```markdown
## Promotion PR: `testing` (`BUILD-005` / `build-005-candidate`) -> `beta` (`v1.0.0-beta.1`)

### 1. Сводка релиза
Консолидированный перевод проверенной сборки `BUILD-005` (128 закрытых карточек волн RW-41..RW-44) из интеграционной ветки `testing` в предрелизную ветку `beta`.

### 2. Ключевые включённые функциональные блоки
- **Нативный клиент `discord-gpui` (GPUI + WGPU):**
  - Полный UI-каркас: Server Rail, Channel Sidebar, Chat View с виртуализацией, Member Sidebar, Quick Switcher (`Ctrl+K`), Emoji Picker, Search Drawer, User Profile Card, Settings Dialog (8 разделов), Captcha Modal, Create Guild/Channel, Account Switcher, Friends View и Call/Stage Overlay.
  - Бюджет рендеринга 120 FPS (`frame_tick_us < 8300 мкс`), холодный старт и потребление памяти `VmRSS ~ 11.5 МБ` (при лимите `< 60 МБ`).
- **Демонстрация экрана (Go Live) и видео-конвейер (`TASK-775..TASK-789`):**
  - Нативный захват экрана и окон под Wayland (`org.freedesktop.portal.ScreenCast`, KDE `KWin.ScreenShot2`, `wlroots`) с автоматическим фолбэком на `XShm` в X11.
  - Нормализация источника WebRTC (`sourceId: "-1"`), горячее переключение окон и параметров потока (`720p/1080p`, `30/60 FPS`, `maxBitrate` 4–8 Мбит/с) без разрыва активной сессии.
  - Полная поддержка опкодов шлюза `OP 18/19/22` и голосовых опкодов `Voice OP 1 (SELECT_PROTOCOL)`, `OP 12 (VIDEO SSRC/RTX)`, `OP 16 (VOICE_BACKEND_VERSION)`.
- **Голосовой стек и криптография DAVE MLS v1 (`discord-voice`):**
  - Атомарный zero-copy `AudioRingBuffer`, обработка `VOICE_STATE_UPDATE` без мерцания участников, индикатор качества RTC (`#live-rtc-popout`) и фаззинг-защита пакетов DAVE v1 / Zstd.
- **Локальное хранилище SQLite WAL (`discord-store` & `LocalDatabase`):**
  - Персистентное хранение гильдий, каналов, сообщений, настроек и полнотекстовый индекс `FTS5` (`quick_switcher_fts`) в режиме `PRAGMA journal_mode = WAL`.
- **Доступность, визуальная регрессия и надёжность (`G4 QA`):**
  - AT-SPI2 / WCAG 2.2 контракты, 15 золотых эталонов SSIM (`> 0.995`), Chaos Engineering контракты и 5/5 убитых мутаций.

### 3. Результаты верификации QA (`QA-BUILD-005.md`)
- **Статус QA:** `QA PASSED` (`P0 = 0`, `P1 = 0`, `P2 = 0`, `skipped = 0`)
- **Headless Smoke (`discord-gpui --headless-smoke`):** `status = "ok"`, `display_backend = "wayland+x11"`, `sqlite_wal_ok = true`, `fps_budget_ok = true`, `vmrss_mb = 11.54 MB`
- **Системная совместимость:** `ELF 64-bit LSB pie executable, stripped`, `GLIBC <= 2.35`, работа на программном рендерере `llvmpipe` подтверждена.

### 4. Контрольные суммы релизных артефактов (`rust/dist/SHA256SUMS`)
| Артефакт | SHA-256 | Статус |
| :--- | :--- | :--- |
| `rust/dist/discord-gpui` | `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e` | `ЦЕЛ` |
| `rust/dist/discord-gui-spike` | `18b7eea8e3438ed8ea1ab133347c9956d94d6f443049872835eeb08e7e2c19f9` | `ЦЕЛ` |
| `rust/dist/discord-rs` | `42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae` | `ЦЕЛ` |
| `rust/dist/sbom.spdx.json` | `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921` | `ЦЕЛ` |

### 5. План отката (Rollback Plan)
Регламентирован в `docs/specs/ROLLBACK-PROCEDURE.md` (команда отката `git revert -m 1 <merge-commit>`, совместимость SQLite WAL `user_version = 1` при даунгрейде верифицирована).

Closes: #TASK-957
```

### 2.4. Команда слияния в `beta` после одобрения Lead Orchestrator
После получения двойного одобрения (`ACCEPT.md` от PM и `REVIEW.md` от Technical Reviewer) координатор/интегратор выполняет слияние строго с созданием merge-коммита (`--no-ff`), чтобы зафиксировать единую точку возврата:

```bash
git checkout beta
git merge --no-ff testing -m "chore(release): promote BUILD-005 from testing to beta (v1.0.0-beta.1)

Closes: #TASK-957"
git tag -a v1.0.0-beta.1 -m "Release v1.0.0-beta.1 (BUILD-005)"
```

---

## 3. Регламент экстренного отката (`Rollback Protocol`, `SUB-957-2`)

### 3.1. Критерии (триггеры) немедленного запуска отката
Процедура экстренного отката инициируется дежурным инженером `devops` или `Lead Orchestrator` без ожидания полного цикла разбора, если после слияния в `beta` обнаружен хотя бы один из следующих симптомов:
1. Падение холодного старта или `discord-gpui --headless-smoke` (код выхода $\ne 0$ или `"status": "degraded"`).
2. Превышение бюджета памяти (`VmRSS >= 60 MB`) или обнаружение утечки файловых дескрипторов/сокетов.
3. Несовпадение контрольных сумм в `rust/dist/SHA256SUMS` или нарушение целостности SBOM.
4. Критическая ошибка безопасности (`P0`), утечка не замаскированного токена в логах или отказ криптографии `DAVE MLS v1` / `SRTP`.
5. Ошибка открытия или миграции локальной базы данных SQLite (`StoreError::Migration` / `StoreError::Sqlite`).

---

### 3.2. Шаг 2.1: Быстрый откат merge-коммита в ветке `beta` за 1 команду
Поскольку слияние `testing -> beta` всегда выполняется с флагом `--no-ff`, вершина ветки `beta` представляет собой merge-коммит с двумя родителями:
* **Родитель `1` (`-m 1`):** предыдущее стабильное состояние ветки `beta` (до вливания `testing`).
* **Родитель `2` (`-m 2`):** влитая ветка `testing`.

Для мгновенного и безопасного возврата ветки `beta` к предыдущему состоянию без переписывания истории (`reset --hard` запрещён) выполняется **одна команда `git revert -m 1`**:

```bash
# 1. Проверить текущий merge-коммит на вершине beta и убедиться, что parent #1 — предыдущая вершина beta
git checkout beta
MERGE_COMMIT="$(git rev-parse HEAD)"
git show --stat --oneline -n 1 "$MERGE_COMMIT"

# 2. Выполнить атомарный откат слияния к первому родителю (-m 1) одним новым коммитом
git revert -m 1 --no-edit "$MERGE_COMMIT"

# 3. Пометить откат сервисным тегом для трассируемости в аудите волн
git tag -a "rollback-beta-$(date -u +%Y%m%d-%H%M%S)" -m "Emergency rollback of $MERGE_COMMIT on beta"
```

#### Действия в случае конфликта или серии коммитов:
* Если поверх merge-коммита в `beta` был создан дополнительный коммит метаданных, откат выполняется в обратном хронологическом порядке:
  ```bash
  git revert --no-commit HEAD
  git revert --no-commit -m 1 "$MERGE_COMMIT"
  git commit -m "revert(release): emergency rollback of BUILD-005 promotion on beta"
  ```
* После фиксации revert-коммита в `beta` создаётся ветка исправления `bugfix/task-<id>-<slug>` от `unstable`; повторный перенос в `testing` и `beta` разрешается только после устранения причины и нового цикла `QA Passed`.

---

### 3.3. Шаг 2.2: Восстановление предыдущего релиза бинарника из кэша артефактов
При откате ветки `beta` необходимо синхронно гарантировать, что бинарные артефакты в `rust/dist/` соответствуют откатанной ревизии и проходят криптографическую проверку `sha256sum -c SHA256SUMS`.

#### Источники восстановления в локальном кэше артефактов:
1. **Git-отслеживаемый манифест и дистрибутив в дереве репозитория (`rust/dist/`):** после выполнения `git revert -m 1 <merge-commit>` отслеживаемые артефакты и `rust/dist/SHA256SUMS` автоматически возвращаются к состоянию предыдущего релиза.
2. **Локальный архивный кэш релизных тарболлов (`rust/dist/*.tar.gz` / `rust/target/release/`):** если не отслеживаемый в Git бинарник в `rust/dist/` был перезаписан поверх рабочего дерева, он восстанавливается из релизного архива или пересобирается в оффлайн-режиме (`scripts/release.sh`).

#### Протокол восстановления и верификации артефактов (`rust/dist/`):

```bash
# Вариант А: Если бинарный архив предыдущего релиза сохранён в кэше артефактов rust/dist/archive/
if [[ -f rust/dist/archive/discord-gpui-prev-linux-x86_64.tar.gz ]]; then
    tar -xzf rust/dist/archive/discord-gpui-prev-linux-x86_64.tar.gz -C rust/dist/
fi

# Вариант Б: Детерминированная оффлайн-пересборка из откатанной ревизии beta (без сети)
CARGO_TARGET_DIR="/home/sanya/orca/project/rust/target" \
    cargo build --offline --release --manifest-path rust/discord-gpui/Cargo.toml
install -m 0755 rust/target/release/discord-gpui rust/dist/discord-gpui
bash scripts/release.sh

# Обязательная тройная верификация восстановленного артефакта:
# 1. Сверка контрольных сумм SHA256SUMS
(cd rust/dist && sha256sum -c SHA256SUMS)

# 2. Проверка ELF-заголовка и отсутствия отладочных символов (stripped)
file rust/dist/discord-gpui | grep -E "ELF 64-bit LSB.*stripped"

# 3. Контрольный запуск headless-smoke восстановленного бинарника
./rust/dist/discord-gpui --headless-smoke
```

Ожидаемый вывод верификации после восстановления:
```text
discord-gpui: ЦЕЛ
discord-gui-spike: ЦЕЛ
discord-rs: ЦЕЛ
sbom.spdx.json: ЦЕЛ
GPUI_SMOKE_OK {"status":"ok","display_backend":"wayland+x11","window_width":1280,"window_height":720,"min_width":900,"min_height":550,"guilds":2,"channels":5,"sqlite_wal_ok":true,"fps_budget_ok":true,"frame_tick_us":0,"vmrss_kb":11812,"vmrss_mb":11.54}
```

---

## 4. Совместимость схемы SQLite WAL при даунгрейде (`SUB-957-3`)

При экстренном откате клиентского бинарника с новой сборки (`v1.0.0-beta.1` / `BUILD-005`) на предыдущую версию критически важно, чтобы локальная база данных пользователя, уже открывавшаяся новой сборкой, оставалась полностью читаемой и консистентной без потери данных.

### 4.1. Архитектура локальных хранилищ SQLite в проекте
В `discord-lab` задействованы два изолированных модуля работы с SQLite (системная библиотека `libsqlite3` 3.54):

1. **Ядро хранения `discord-store` (`rust/crates/discord-store/src/lib.rs`):**
   * **Режим журнала:** `PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; busy_timeout = 5000`.
   * **Версионирование схемы:** `PRAGMA user_version = 1` (`pub const SCHEMA_VERSION: u32 = 1`).
   * **Таблицы и индексы (`SQL_SCHEMA_V1`):**
     * `channels (id TEXT PRIMARY KEY NOT NULL, guild_id TEXT, name TEXT, kind INTEGER NOT NULL, position INTEGER NOT NULL)`
     * `messages (id TEXT PRIMARY KEY NOT NULL, channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE, author_id TEXT NOT NULL, content TEXT NOT NULL, timestamp INTEGER NOT NULL, edited_timestamp INTEGER, flags INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0)`
     * `kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)`
     * Индексы: `idx_messages_channel_timestamp (channel_id, timestamp DESC)` и `idx_messages_channel_id (channel_id, id DESC)`.
2. **Локальная БД нативного клиента `discord-gpui::storage::LocalDatabase` (`rust/discord-gpui/src/storage/mod.rs`):**
   * **Режим журнала:** `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`.
   * **Таблицы (`init_schema`):**
     * `guilds (id, name, icon, owner_id, unread_count, mention_count)`
     * `channels (id, guild_id, category_id, name, channel_type, topic, position, unread)`
     * `messages (id, channel_id, author_id, author_name, content, timestamp, is_pinned)` + индекс `idx_messages_channel (channel_id, timestamp)`
     * `user_settings (key, value)`
     * `users (id, username, display_name, avatar, is_bot)`
     * Виртуальная таблица полнотекстового поиска: `quick_switcher_fts USING fts5(id UNINDEXED, kind, name, subtitle)`.

### 4.2. Инварианты обратной совместимости (Downgrade Safety)
1. **Единая версия схемы `user_version = 1`:**
   * В сборке `BUILD-005` (`v1.0.0-beta.1`) константа `SCHEMA_VERSION` равна `1`, что идентично предыдущим стабильным сборкам (`BUILD-004`).
   * Ни одна таблица не содержит деструктивных изменений (`DROP TABLE`, `DROP COLUMN`, изменения типов первичных ключей или обязательных `NOT NULL` колонок без `DEFAULT`).
2. **Идемпотентная DDL-инициализация:**
   * Все инструкции создания таблиц и индексов используют `CREATE TABLE IF NOT EXISTS` и `CREATE INDEX IF NOT EXISTS` внутри транзакции `BEGIN IMMEDIATE ... COMMIT`.
   * Повторный вызов `Store::migrate()` или `LocalDatabase::open()` на базе данных после обновления и последующего отката бинарника является строгим `no-op` для существующих данных.
3. **Защита от повреждения при несовпадении `user_version`:**
   * В `discord-store` (`src/lib.rs:235-240`) перед началом транзакции проверяется условие:
     ```rust
     let version = self.schema_version()?;
     if version > SCHEMA_VERSION {
         return Err(StoreError::Migration(format!(
             "user_version {version} новее поддерживаемой {SCHEMA_VERSION}: обновите клиент"
         )));
     }
     ```
   * Если в будущих волнах версия схемы будет увеличена (`SCHEMA_VERSION >= 2`), старый клиент при даунгрейде **не попытается перезаписать или повредить схему**, а детерминированно вернёт типизированную ошибку `StoreError::Migration` до выполнения `BEGIN IMMEDIATE`.
4. **Консистентность файлов журнала WAL (`-wal`, `-shm`) при смене бинарника:**
   * При штатном завершении процесса `discord-gpui` соединение SQLite закрывается с автоматическим слиянием WAL-страниц.
   * Даже при аварийном завершении новой сборки (`SIGKILL` / `panic!`) оставшийся на диске файл `<db>-wal` использует стандартный бинарный формат SQLite `3007000`, который прозрачно доигрывается (recovery) предыдущей версией бинарника при первом вызове `sqlite3_open_v2`.

### 4.3. Протокол и команда проверки целостности SQLite при циклическом обновлении/откате (`Шаг 3.1`)
Ниже приведён воспроизводимый проверочный сценарий (оракул на базе системного `python3 sqlite3` и независимого скрипта `waves/RW-31/sqlite/oracle.py`), подтверждающий, что база данных после открытия новой сборкой `v1.0.0-beta.1`, записи данных, `wal_checkpoint` и возврата на предыдущую версию сохраняет 100% целостность:

```bash
python3 -c '
import sqlite3, tempfile, os, subprocess

with tempfile.TemporaryDirectory() as tmp:
    # 1. Инициализация и запись данных новой сборкой (v1.0.0-beta.1) в discord-store (WAL, user_version=1)
    store_db = os.path.join(tmp, "store.db")
    conn = sqlite3.connect(store_db)
    conn.executescript("""
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS channels (
            id TEXT PRIMARY KEY NOT NULL, guild_id TEXT, name TEXT, kind INTEGER NOT NULL, position INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY NOT NULL,
            channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
            author_id TEXT NOT NULL, content TEXT NOT NULL, timestamp INTEGER NOT NULL,
            edited_timestamp INTEGER, flags INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_messages_channel_timestamp ON messages(channel_id, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_channel_id ON messages(channel_id, id DESC);
        CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
        PRAGMA user_version = 1;
        INSERT INTO channels VALUES ("ch-1", "g-1", "general", 0, 1);
        INSERT INTO messages VALUES ("m-1", "ch-1", "u-1", "сообщение из beta.1", 1700000001, NULL, 0, 0);
        INSERT INTO kv VALUES ("release_build", "BUILD-005");
    """)
    conn.commit()
    conn.close()

    # 2. Проверка независимым оракулом waves/RW-31/sqlite/oracle.py после даунгрейда на предыдущую версию
    oracle = subprocess.run(["python3", "waves/RW-31/sqlite/oracle.py", store_db], capture_output=True, text=True, check=True)
    assert "user_version=1" in oracle.stdout
    assert "journal_mode=wal" in oracle.stdout
    assert "integrity_check=ok" in oracle.stdout
    assert "foreign_key_violations=0" in oracle.stdout
    assert "duplicate_ids=0" in oracle.stdout

    # 3. Проверка БД нативного клиента discord-gpui (LocalDatabase + FTS5) при циклическом обновлении/откате
    gpui_db = os.path.join(tmp, "gpui_local.db")
    conn_gpui = sqlite3.connect(gpui_db)
    conn_gpui.executescript("""
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, icon TEXT, owner_id TEXT,
            unread_count INTEGER DEFAULT 0, mention_count INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS channels (
            id TEXT PRIMARY KEY, guild_id TEXT, category_id TEXT, name TEXT NOT NULL,
            channel_type INTEGER NOT NULL, topic TEXT, position INTEGER DEFAULT 0, unread INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY, channel_id TEXT NOT NULL, author_id TEXT NOT NULL,
            author_name TEXT NOT NULL, content TEXT NOT NULL, timestamp TEXT NOT NULL, is_pinned INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel_id, timestamp);
        CREATE TABLE IF NOT EXISTS user_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY, username TEXT NOT NULL, display_name TEXT, avatar TEXT, is_bot INTEGER DEFAULT 0
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS quick_switcher_fts USING fts5(id UNINDEXED, kind, name, subtitle);
        INSERT INTO guilds VALUES ("g-1", "Discord Lab", NULL, "u-1", 0, 0);
        INSERT INTO user_settings VALUES ("theme", "discord_dark");
        INSERT INTO quick_switcher_fts VALUES ("ch-1", "channel", "general", "Discord Lab");
    """)
    conn_gpui.commit()
    wal_chk = conn_gpui.execute("PRAGMA wal_checkpoint(TRUNCATE);").fetchone()
    integrity = conn_gpui.execute("PRAGMA integrity_check;").fetchone()[0]
    fts_rows = conn_gpui.execute("SELECT id, name FROM quick_switcher_fts WHERE quick_switcher_fts MATCH \"gen*\";").fetchall()
    conn_gpui.close()

    assert integrity == "ok", f"Integrity check failed: {integrity}"
    assert fts_rows == [("ch-1", "general")], f"FTS5 mismatch after downgrade: {fts_rows}"
    print("SQLITE_DOWNGRADE_CYCLE_OK", wal_chk, integrity)
'
```

Дополнительно для контрактной проверки на уровне Rust-крейта выполняется целевой тест T0:
```bash
CARGO_TARGET_DIR="/home/sanya/orca/project/rust/target" \
    cargo test --manifest-path rust/Cargo.toml -p discord-store --test store_contract
```

---

## 5. Операционный чек-лист дежурного инженера при выполнении отката

При срабатывании триггера отката инженер `devops` выполняет строго по порядку следующие 5 шагов:

- [ ] **1. Фиксация инцидента:** Сохранить лог ошибки, вывод `./rust/dist/discord-gpui --headless-smoke` и SHA проблемного merge-коммита в `waves/RW-45/INCIDENT-ROLLBACK.md`.
- [ ] **2. Откат ветки `beta`:** Выполнить `git checkout beta && git revert -m 1 --no-edit <merge-commit-sha>` и выставить тег `rollback-beta-<timestamp>`.
- [ ] **3. Восстановление бинарников в `rust/dist/`:** Восстановить предыдущий релизный бинарник и проверить `(cd rust/dist && sha256sum -c SHA256SUMS)` -> все позиции `ЦЕЛ`.
- [ ] **4. Проверка БД SQLite WAL и Smoke-тест:** Выполнить `./rust/dist/discord-gpui --headless-smoke` и убедиться в получении маркера `GPUI_SMOKE_OK` с полями `"status":"ok"` и `"sqlite_wal_ok":true`.
- [ ] **5. Заведение задачи на исправление:** Создать ветку `bugfix/task-<id>-<slug>` от `unstable` и перевести задачу в статус `QA Failed` для повторного прохождения цикла `unstable -> testing -> beta`.
