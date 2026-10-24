# MERGE-GATE — окно слияния и передача в тестирование (RW-39)

Правило: **ничего не сливаем, пока все части не готовы**. Окно открывается только при
выполнении всех предусловий ниже. Коммиты/пуш выполняет координатор (владелец); PM готовит
пакет, прогоняет гейт и передаёт QA.

## 1. Предусловия окна (все обязательны)

| # | Условие | Проверка |
|---|---|---|
| 1 | W39h (frontend) принят PM | `PM/w39h/REPORT.md`, DOM/features green |
| 2 | W39g2 (delta RE) принят, дельта применена | `PM/w39g2/DELTA.md`, спека обновлена |
| 3 | W39i/W39i2 (backend) приняты | `cargo test` 53+, clippy/fmt 0, `search_page` зарегистрирован |
| 4 | Codex-Review frontend/backend = PASS (P0=P1=0) | `PM/w39e/REVIEW-codex.md`, `PM/w39h/REVIEW-codex.md` |
| 5 | Нет незакрытых P0/P1 в `LOOP-LEDGER` | ledger |
| 6 | Живое окно не запускалось с момента последнего изменения `ui/**` | `window-budget.mjs --check` |
| 7 | md5-манифест всех изменённых файлов снят | `PM/md5-merge.txt` |

## 2. Ветка и коммит (координатор)

```bash
git switch -c dev/rw-39-integration
# PM отдаёт список файлов + сообщение коммита; координатор коммитит
git add <список из PM>
git commit -m "RW-39: дизайн-паритет + P3-02 чат-вью (frontend/backend, QA-ready)"
```

Запрещено: пушить в `main` до QA. Ветка `dev/rw-39-integration` — единственная точка слияния.

## 3. Полный гейт (один раз, после коммита)

```bash
# 1. Node: typecheck + meta + unit + smoke (живое окно: 2 запуска)
npm run gate:worker
# 2. Полный релизный гейт: e2e + coverage + golden (ещё 1 живой запуск)
npm run gate:release
# 3. Rust workspace (основной, 1069+ тестов)
cd rust && cargo test --offline --workspace
# 4. GUI-спайк (отдельный workspace)
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
# 5. Записать живой прогон в бюджет
node waves/RW-39/PM/window-budget.mjs --record --reason "merge gate E9"
```

## 4. Интеграция с ядром (W39j, до QA)

Цель: UI получает данные не из «своего» JSON-слоя, а через адаптер к ядру.

1. `gui-spike/src/core_bridge.rs` — path-зависимости на `discord-core`/`discord-store`
   (офлайн), маппинг READY/MESSAGE_CREATE/MEMBER_LIST в типы ядра.
2. Контрактный тест: для одних и тех же `fixtures/` счётчики/ids из BFF и из ядра совпадают.
3. Tauri-команды `ready/messages_page/members_page/settings/search_page` вызывают адаптер;
   ошибки ядра → `DataError` без паник.
4. Без сети, без новых внешних крейтов; при конфликте версий — эскалация PM.

## 5. Передача QA (E9)

Артефакты: ветка + коммит, `.gate/*.json`, md5-манифест, `QA/PLAN-E9.md`, `QA/REPORT-E9.md`,
скриншот окна и AT-SPI-снимок, `metrics.json` (FPS/PSS).

QA возвращает: дефекты P0/P1/P2 с repro; PM возвращает воркерам только с repro-командой.
Приёмка RW-39 — только после QA PASS и P0/P1=0.
