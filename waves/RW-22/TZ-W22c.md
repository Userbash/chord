# ТЗ W22c — P7-04: read-only API surface

Волна: RW-22 / B03. Владелец: W22c. Работать в `/home/user/orca/project`, не коммитить.

## 0. Обязательное чтение

`AGENTS.md`, `PROCESS.md` §4, `waves/RW-22/WAVE-PLAN.md`, `docs/re/rest-d02.md` §2,
`docs/rust-client/PLAN.md` (P7-04), `docs/rust-client/TEST-PLAN.md` (T-REST-09),
`rust/crates/discord-core/src/rest/routes.rs`, `rest/mod.rs`.

Твои файлы — **только**:
`rust/crates/discord-core/src/rest/resources.rs`,
`rust/crates/discord-core/src/rest/mod.rs` (только `pub mod resources;` и реэкспорт, если нужен),
`rust/crates/discord-core/tests/rest_resources.rs`,
`rust/crates/discord-core/Cargo.toml` (только если нужен `discord-json`; лучше не нужен).
Не трогай другие модули, `lib.rs`, `routes.rs`, доки, `waves/*`.

## 1. Метод

Strict TDD. Сначала тесты, red-лог:

```bash
cd /home/user/orca/project/rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core --test rest_resources \
  2>&1 | tee ../waves/RW-22/red-w22c.log
```

Red — compile-red/behavior-red, exit 101. Затем минимальная реализация до green,
`green-w22c.log`. Тесты не подгонять.

## 2. Контракт

Новый модуль `rest::resources` — **только чтение** подтверждённых D-02 ресурсов:

| ResourceKind | Путь GET | Статусы D-02 |
|---|---|---|
| Quest | `/quests/@me` | 200 |
| QuestDecision | `/quests/decision` | 200 |
| QuestGetDecisions | `/quests/get-decisions` | 200 |
| Promotion | `/promotions` | 200 |
| Entitlements | `/users/@me/entitlements` | 200 |
| ApplicationEntitlements | `/users/@me/applications/{id}/entitlements` | 200 |
| Library | `/users/@me/library` | 200 |
| SavedMessages | `/users/@me/saved-messages` | 404 (не выдавать за данные) |

Публично:

```rust
pub enum ResourceKind { Quest, QuestDecision, QuestGetDecisions, Promotion, Entitlements,
                        ApplicationEntitlements, Library, SavedMessages }
pub struct ReadOnlyResource { pub kind: ResourceKind, pub path_template: &'static str,
                              pub observed_statuses: &'static [u16] }
pub const RESOURCES: [ReadOnlyResource; 8];
pub fn resource(kind: ResourceKind) -> &'static ReadOnlyResource;
pub fn path(kind: ResourceKind, application_id: Option<&str>) -> Result<String, ResourceError>;
```

- `path`: для `ApplicationEntitlements` требует `Some(id)`, id — только ASCII-цифры (1..=20),
  иначе `ResourceError::MissingId`/`InvalidId`; для остальных `None`; лишний id — ошибка
  `UnexpectedId` (не молча игнорировать).
- Результат — относительный путь без `/api/v9` (как `routes::ROUTES`), без query, без
  абсолютного URL; использовать `routes::render_path`/`RouteSpec`, чтобы не дублировать
  шаблон. `resource(kind)` возвращает статическую запись; проверь, что каждый путь есть в
  `routes::ROUTES` с `HttpMethod::Get` (тест).
- `ResourceError` — `Debug/Display/PartialEq`; никаких паник.
- `ReadOnlyResource` не должен иметь метода write; любые попытки POST/PUT/PATCH/DELETE
  отклоняются уже тем, что каталог содержит только GET. Добавь тест, который проверяет
  отсутствие таких маршрутов в `RESOURCES` и наличие точного пути в `ROUTES`.
- `SavedMessages.observed_statuses == [404]`; `path` всё равно возвращает путь (это read-only
  маршрут), но документация/тест фиксируют, что 404 не «пустой успех».

## 3. Обязательные тесты (минимум)

1. 8 kind → 8 точных путей; `resource` и `RESOURCES` согласованы.
2. Каждый путь найден в `routes::ROUTES` как GET; ни одного write-метода.
3. `path` для application entitlements с числовым id; missing/empty/нецифровой/слишком
   длинный/лишний id для остальных.
4. `SavedMessages` статус 404 сохранён; `Quest` 200.
5. Нет query-параметров/абсолютных URL (`path` не содержит `?`, `://`).
6. `Display` ошибок не содержит пользовательских значений целиком/секретов.
7. Повторный вызов детерминирован; `RESOURCES` непуст и без duplicate kind.

## 4. Отчёт

Верни red/green команды+exit, файлы, число тестов, риски. Не запускай workspace.
