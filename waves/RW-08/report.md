# Отчёт волны RW-08 — REST-клиент (P1-07 / T-REST-01…03)

**Дата:** старт 2026-10-19, приёмка 2026-10-20. **Карточка:** P1-07 (`docs/rust-client/PLAN.md`).
**Метод:** первая параллельная волна по `PROCESS.md` §3.13 (пожелания владельца) — три воркера
одновременно (W8a/W8b/W8c, непересекающиеся файлы) + отдельный рефакторинг-воркер W8d +
независимый ревьюер W8r. Контракты и тест-данные заморожены координатором до старта
(`TZ.md`). Новых зависимостей нет; сеть — только `127.0.0.1` (реальный TLS-транспорт —
открытый вопрос владельцу, §«Открытые вопросы»).

## Итог

- `discord-core` — **95** тестов, воркспейс — **137/137**; clippy `-D warnings` и `cargo fmt
  --check` чисто.
- Новых кейсов волны — **24**: T-REST-01 `rest_routes` 8, T-REST-02 `rest_headers` 5,
  T-REST-03 `rest_retry` 11. Каждый таргет — ×5 подряд зелёный (у W8c дополнительно ×3).
- Ревью W8r: **«принято с нитами»**, блокеров нет; 14/14 плановых мутаций пойманы; нит
  покрытия (выжившая мутация удаления `sleep(backoff)`) закрыт red-first кейсом N6.

## Red-first (по воркерам)

| Воркер | Продукт | Тест (финальный md5) | Red | `# EXIT` | Артефакты |
|---|---|---|---|---|---|
| W8a | `rest/routes.rs` | `tests/rest_routes.rs` `f67f3a4a…` | 1 passed / 7 failed (U1–U5, U7, U8; U6 — негативный контроль каркаса) | 101 | `red/w8a-red-fmt.log`, `red/rest_routes-red.rs` |
| W8b | `rest/headers.rs` | `tests/rest_headers.rs` `05aba402…` | 2 passed / 3 failed (H1, H2, H5; H3/H4 — контроль выдержки/базы) | 101 | `red/w8b-red.log`, `red/rest_headers-red.rs` |
| W8c | `rest/retry.rs` | `tests/rest_retry.rs` `1b7e1c59…` → `fee504ca…` | 0/10, после кейса N6 — 0/11 | 101 | `red/w8c-red.log`, `red/w8c-red-gap.log`, `red/rest_retry-red.rs`, `red/rest_retry-gap-red.rs` |

Пересъёмки red (обе — по правилу «тест после red не менялся»):

1. `cargo fmt` (style edition 2024) переставил импорты в `rest_routes.rs` → red W8a переснят на
   финальном тесте (`red-fmt`, md5 `f67f3a4a…`); ранний лог сохранён как `red/w8a-red.log`.
2. Закрытие нита W8r (пауза ретраев) добавило кейс N6 в `rest_retry.rs` → red переснят на
   финальном тесте (`red-gap`, 0/11, md5 `fee504ca…`); исторический лог — `red/w8c-red.log`.

Red-копии побайтово равны финальным тестам (md5 в логах = md5 файлов в `red/`); продукт при
закрытии нита не менялся (реализация восстановлена из копии, md5 совпал).

## Green, гейты, рефакторинг

- Полный пакет `-p discord-core` — 95/0; воркспейс — 137/0; `npm run gate:worker` — passed
  (meta 25 / unit 169 / smoke 20).
- W8d (не-поведенческий рефакторинг 3 `src`-файлов): доккомментарии, `#[must_use]`, константа
  `PLACEHOLDERS`, уплощение цикла ретраев; тесты-спека и контракты не менялись (md5 тестов до =
  после), patch — `reports/w8d-diff.patch`.
- Закрытие нита покрытия: кейс **N6** `backoff_pause_between_attempts` — red 0/11
  (`# EXIT=101`, md5 `fee504ca…`), green 11/11 ×5; контрольная мутация «удалить
  `sleep(backoff)`» теперь ловится: `пауза между попытками не выдержана: прошло 6.54µs при
  backoff 60ms` (`red/w8c-mutation-sleep.log`, `# EXIT=101`).

## Принятые файлы (md5)

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/rest/routes.rs` | `f66b819f48756ca0d234900a77a39bd6` |
| `rust/crates/discord-core/src/rest/headers.rs` | `720be6fb046840e441a24e333ef5452d` |
| `rust/crates/discord-core/src/rest/retry.rs` | `8a6dc5efbea08f898118cd40b51bf1f4` |
| `rust/crates/discord-core/src/rest/mod.rs` | `5412be37655eba58d5341efcb7becc9f` |
| `rust/crates/discord-core/src/lib.rs` | `263334a6730e056de9e4cc495f572423` |
| `rust/crates/discord-core/tests/rest_routes.rs` | `f67f3a4a3f83cb301e3fd44cb3e5e260` |
| `rust/crates/discord-core/tests/rest_headers.rs` | `05aba4024176d913667092eb8cf1b3a9` |
| `rust/crates/discord-core/tests/rest_retry.rs` | `fee504ca9458dcc67004cdf68f0caa43` |
| `tests/data/rest-routes-47.tsv` | `2db6b39bcba96bc3a2d480fea9461a33` |
| `tests/data/rest-crossref-47.tsv` | `1569384026aa85ba2854f87a7d7048dd` |
| `tests/data/rest-auth-6.tsv` | `144c34c24ecd4645540be518e69fbe3b` |

Тест-данные — jq-выдержки из `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`),
пересчитаны ревьюером независимо побайтово.

## Ревью (W8r)

`waves/reviews/review-rw-08.md`: оракулы пересчитаны своими командами 1:1; red воспроизведён
заглушками (1/7, 2/3, 0/10); границы чисты (`Cargo.*` без diff, соседи/доки/golden не тронуты);
**14/14 мутаций**; перезапуск гейтов совпал с отчётами. Ниты (low/info): устаревший md5 в
отчёте W8a — аннотирован здесь и в `red/w8a-red-fmt.log`; тайминговые нижние границы S2–S4 —
принятый риск (флак-риск низкий, `CASE_TIMEOUT` 5 с); выжившая мутация `sleep` — закрыта N6.

## Открытые вопросы / риски

- **Реальный HTTP-транспорт к `discord.com` (TLS; `reqwest` или руками на tokio/rustls)** —
  решение владельца до первой живой REST-волны (P1-08/P1-09/P2-06); политика волны
  подключается к нему без изменения тестов.
- 429/Retry-After и 5xx-бэкофф — P1-09; генератор X-Super-Properties — P1-08.
- S2–S4 зависят от нижних границ таймингов; верхних границ нет.

## Артефакты

- `waves/RW-08/TZ.md` — ТЗ (контракты, кейсы, процедуры).
- `waves/RW-08/red/` — red-логи (`w8a-red-fmt`, `w8a-red`, `w8b-red`, `w8c-red`, `w8c-red-gap`,
  `w8c-mutation-sleep`) и red-копии тестов.
- `waves/RW-08/reports/` — отчёты воркеров W8a–W8d, patch рефакторинга.
- `waves/RW-08/final/` — принятые снимки `rest/*.rs`, `lib.rs`.
- `waves/reviews/review-rw-08.md` — независимое ревью.
- Черновики: `/tmp/opencode/rw-08/`, `/tmp/opencode/review-rw-08.md` (вне git).
