# Pull Request #04: `rw40/m13-search-202-be` → `unstable`

- **ID задачи:** `task-110` (`M-13-SEARCH-202-NSFW-BACKEND`)
- **Отдел / Воркер:** G1 Backend (`codex`)
- **Исходная ветка:** `rw40/m13-search-202-be` (создана от `unstable`)
- **Целевая ветка:** `unstable`
- **Статус:** `Merged to Unstable` (ветка `rw40/m13-search-202-be` удалена после слияния)

## 1. Описание изменений

- Реализована обработка `HTTP 202 Indexing` в `rust/gui-spike/src/search.rs` по Варианту B (`SEARCH-202-NSFW-SPEC.md §3.1`):
  - `execute()` при `HTTP 202` немедленно возвращает `Ok(SearchResponse { indexing: true, retry_after_ms: Some(delay_ms), documents_indexed: 0, messages: [] })`, разбирая `Retry-After` в целых секундах (`* 1000`, fallback `5000` мс) без чтения тела `202`.
  - `execute_with_retry()` выполняет до 5 повторов с параметром `&attempts=1..5` и при 6-м `202` возвращает `SearchError::Indexing { retry_after_ms, attempts: 5 }`.
  - Поле `documents_indexed: u64` десериализуется с `#[serde(default)]` (`0` при отсутствии).
  - Параметр `include_nsfw=true` добавляется в URL только при `Some(true)`; при `Some(false)` или `None` опускается.
- Расширен контрактный набор `rust/gui-spike/tests/search_contract.rs` до 23 тестов.

## 2. Проверки и ревью

- **Локальные проверки воркера:** `23/23` `search_contract`, `2/2` `--lib search`, `5/5` мутаций `KILLED`, `rustfmt` exit `0`.
- **Продуктовое ревью (PM):** **APPROVED** (`waves/RW-40/PM/m13-search-be/ACCEPT.md`).
- **Техническое ревью (`deepseek/deepseek-v4.1-flash` — Review 34B):** **PASS (`P0=0, P1=0, P2=2`)** (`waves/RW-40/PM/m13-search-be/REVIEW-PASS2.md`).
