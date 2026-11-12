# Приёмка PM: task-178 (BE-P3-02 & BE-P3-09) — Разделение каталога `ROUTES` (`47` D-02 vs `2` `[S]`-расширения) и сверка `rest-d02.json`

- **PR:** PR-63 (`feature/task-178-routes-catalog-s-markers -> unstable`)
- **Статус:** `Ready for Technical Review` (передано Техническому ревьюеру в Review 92A)
- **Дата:** 2026-10-31
- **Автор ревью:** Product Manager (PM)

## Проверка соответствия ТЗ и DoD
1. **Маркировка `[S]`-расширений в `routes.rs` (BE-P3-02):**
   - Добавлены константы `CAPTURED_D02_ROUTE_COUNT: usize = 47` и `STATIC_EXTENSION_ROUTE_COUNT: usize = 2`.
   - Добавлен метод `RouteSpec::is_static_extension(&self) -> bool`, возвращающий `true` для статических расширений вне захвата D-02 (`POST /channels/{id}/messages`, `GET /guilds/{id}/messages/search` внутри `ROUTES` и 4 спецификации в `EXTENSIONS`) и `false` для 47 захваченных D-02 маршрутов.
2. **Контракт синхронизации `docs/re/rest-d02.json` с оракулом (BE-P3-09):**
   - Тест `rest_d02_copy_matches_lab_oracle` проверяет MD5 и 47 пар `(method, pathTemplate)` в `docs/re/rest-d02.json` и сверяет с `reference-oracle/rest-d02.json`, подтверждая, что единственное отличие между ними — санкционированная замена `REDACTED` в `reqBodySample` согласно `docs/re/README.md`.
3. **Тесты и чистота:**
   - Red -> Green подтверждён (`51/51 PASS`: `18/18` в `rest_routes` + `33/33` lib-тестов).
   - Clippy (`-D warnings`) и `rustfmt --check` без замечаний, write-scope строго соблюдён.

## Решение PM
**ПРИНЯТО (Accepted for Technical Review).** Передаётся на Техническое ревью (Review 92A) с проверкой 5/5 мутаций.
