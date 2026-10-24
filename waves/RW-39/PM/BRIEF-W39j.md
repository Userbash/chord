# BRIEF W39j — мост UI/backend к ядру (пилот) + канонический Deserialize

Владелец: W39j (Backend/Integration). PM: DeepSeek.
Цель: доказать, что BFF может работать через ядро, а не только по JSON фикстур.

## Файлы (только эти)
- `rust/gui-spike/Cargo.toml` (path-зависимости, без внешних крейтов)
- `rust/gui-spike/src/core_bridge.rs` (новый)
- `rust/gui-spike/src/data.rs` (только `SearchFilters` derive + делегирование пагинации)
- `rust/gui-spike/src/lib.rs`, `src/main.rs` (экспорт/регистрация при необходимости)
- `rust/gui-spike/tests/core_bridge.rs` (новый), `tests/data_contract.rs` (не ослаблять)
- логи: `waves/RW-39/PM/w39j/`

## Задачи
1. **Path-зависимости**: `discord-core` (и `discord-codec`, если нужно) из `rust/crates/*`,
   офлайн. Внешних крейтов не добавлять; если `cargo --offline` не разрешает — остановиться
   и отчитаться (не вендорить, не тянуть сеть).
2. **core_bridge**: конверсия строки BFF (`MessageRow`) → `discord_core::state::Message`
   (id/channel/author/content/timestamp/edited/pinned/type; нечисловой id → `Parse`,
   без паники). Использовать `discord_core::history::History` для страницы:
   те же ids/порядок/курсор, что отдаёт `messages_page`.
3. **Контрактный тест**: на реальных фикстурах первые страницы BFF и ядра совпадают по ids
   и порядку; пагинация вперёд/назад стабильна; пустая страница валидна.
4. **Канонизация фильтров**: добавить `#[derive(Deserialize)]` в `data.rs::SearchFilters`,
   убрать приватное зеркало `SearchFiltersArg` из `main.rs` (camelCase сохранить).
5. Не менять формат ответов BFF и не ослаблять 53 существующих теста.

## Проверки (один cargo)
```bash
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```
Окно/probe/gate/сеть/commit — запрещены.

## Отчёт
`waves/RW-39/PM/w39j/REPORT.md`: что связали, какие ids совпали, что осталось
(например, settings/voice — следующая итерация), md5.
