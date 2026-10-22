# Отчёт волны RW-11 — rate-limit, телеметрия-off, ack

- **Дата:** 2026-10-20. **Карточки:** P1-09 (rate-limit/ошибки), P1-11 (телеметрия off),
  P1-12 (ack/read-state). **Метод:** §3.13 — три независимых куска одновременно.

## 1. Результат

- **23 новых теста:** `rest_rate_limit` 10/10, `rest_telemetry` 4/4, `rest_ack` 9/9.
- На приёмке волны: `discord-core` **166/166**, воркспейс **208/208**; после I1 —
  178/220. `clippy -D warnings`/`fmt` — чисто.
- `gate:worker` — **passed**, `.gate/worker-2026-10-20T07-01-09-367Z.json`.
- Живой Discord не трогали; сеть — только loopback (никогда не понадобилась: все три
  модуля чистые).

## 2. Red / green

| Кусок | Red | Green |
|---|---|---|
| W11a rate-limit | `0 passed; 10 failed` (`w11a-red-fmt.log`), `# EXIT=101` | `10 passed; 0 failed`, `# EXIT=0` (`w11a-green.log`) |
| W11b telemetry | `0 passed; 4 failed` (`w11b-red.log`), `# EXIT=101` | `4 passed; 0 failed`, `# EXIT=0` (`w11b-green.log`) |
| W11c ack | `0 passed; 9 failed` (`w11c-red.log`), `# EXIT=101` | `9 passed; 0 failed`, `# EXIT=0` (`w11c-green.log`) |

Red-копии = финальным тестам:
`w11a-test.red.rs` = `rest_rate_limit.rs` = `a7cee2fa4c393d7f6f0febdee283ecf8`;
`w11b-test.red.rs` = `rest_telemetry.rs` = `6b49c869152f90b3ef3e4581ac2735d0`;
`w11c-test.red.rs` = `rest_ack.rs` = `cd30ad8e36539c50aff0985642619b83`.
Исходники: `rate_limit.rs` = `e9f0e2f4ab15c4895ec28680815bda8b`,
`telemetry.rs` = `641218af368017aa75dc8c0af22a75f1` (исторический; текущий
`c04c2e10…` — doc-only удаление `todo!()`, поведение не менялось — nit N8 ревью),
`ack.rs` = `0d5518ca4d532d7d17546b7855c0d140`.

W11b/W11c были прерваны лимитом субагентов после red и частичной реализации; координатор
довёл green, переснял red на финальных тестах и провёл md5-аудит. W11a завершился позже,
его реализация совпала с контрактом и принята после повторного прогона.

## 3. Что реализовано

- **P1-09:** регистронезависимый поиск `retry-after`/`x-ratelimit-*`; приоритет
  `retry-after` → тело `retry_after` → `x-ratelimit-reset-after`; парсинг целых/дробных
  секунд с отбрасыванием `NaN/∞/отрицательных`; минимальный JSON-сканер (ключ в строке не
  считается полем); классификация 429/401/5xx/4xx; 429-cap, экспонента 5xx с saturating-
  арифметикой; `should_retry` по идемпотентности методов.
- **P1-11:** `TELEMETRY_SEGMENTS = ["science","beaker","track"]`; блокируется ровно первый
  сегмент, query/fragment отбрасываются, сравнение точное/регистрозависимое; lookalike
  (`/science%2Ffoo`) разрешён.
- **P1-12:** `/channels/{channel}/messages/{message}/ack`, ASCII-снежинки; тело
  `{"token":null}`/`{"token":"…","last_viewed":…}` с JSON-экранированием и `u64::MAX` без
  потери точности.

## 4. Файлы

`rust/crates/discord-core/src/rest/{rate_limit,telemetry,ack}.rs`, `rest/mod.rs`;
`tests/rest_{rate_limit,telemetry,ack}.rs`; `waves/RW-11/{TZ.md,report.md,reports/*}`.
Новых зависимостей нет.

## 5. Риски и ограничения

- `Retry-After` как HTTP-date не поддерживается (Discord шлёт секунды); нет jitter и
  глобального bucket-менеджера — это политика вызывающего/следующая волна.
- JSON-сканер плоский (тела ошибок Discord плоские); вложенный `retry_after` не ищется.
- Телеметрия проверяет только путь; другой хост/тело запроса — вне политики.
- `guard_target` и `build_ack_*` были библиотечными до I1; теперь телеметрия встроена в
  `rest::client::ApiClient`, ack-вызов — следующий интеграционный шаг.

## 6. Ревью

Вердикт независимого ревьюера — `waves/reviews/review-rw-10-11.md`: **«принято с нитами»**,
P0-блокеров нет; 50 тестов RW-10/RW-11 зелёные. P1/P2-ниты волны закрыты в RW-10
(см. `waves/RW-10/report.md` §7). По RW-11 остались P3-ниты: глубина JSON-сканера,
нормализация telemetry-пути, неточные ссылки на `rest-d02.md`, процессные замечания.
Повторное независимое ревью 2026-10-20 — **«принято с нитами»** (P0/P1/P2 нет; P3-3/P3-4/
P3-6 и P3-1/P3-2/P3-5 — очередь; doc-only дрейф md5 `telemetry.rs` зафиксирован выше).
Полный отчёт: `waves/reviews/review-rw-10-11-round2.md`.
