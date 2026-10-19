# Отчёт волны RW-09 — X-Super-Properties (P1-08 / T-REST-04) — принята 2026-10-20

Волна №2 в режиме `PROCESS.md` §3.13: три воркера одновременно (W9a / W9b / W9c), затем
рефакторинг W9d и независимое ревью W9r. Контракты заморожены каркасом-заглушками, ТЗ —
`waves/RW-09/TZ.md` (md5 `5dc28158…`). Новых зависимостей нет; сеть не использовалась;
golden и снимок `/home/sanya/re-deep` — только чтение.

## 1. Итоги

- 21 новый кейс: B1–B5 (base64), P1–P8 (22 поля, JSON, base64), I1–I8 (честная идентичность).
- Тесты: `discord-core` **116**, воркспейс **158/158**; clippy `-D warnings` и `cargo fmt --check`
  чисто (весь воркспейс).
- Гейт: `.gate/worker-2026-10-19T21-15-05-608Z.json` — passed (meta 25 / unit 169 / smoke 20).
- Ревью W9r: **принято с нитами**, блокеров нет, 20/21 мутаций; ниты закрыты (§5).
- `Cargo.toml`/`Cargo.lock` не менялись (разрешение владельца на прямые `tokio-rustls` — для
  будущей волны транспорта, не для этой).

## 2. Оракулы

- Живой снимок `/home/sanya/re-deep/d02-snapshot/capture-run2.snap.jsonl`
  (md5 `abfe14b76cec1e559c3b45522bc44b7a`, сверено с `snapshot-md5.txt`), запрос `95974.2323`,
  заголовок `X-Super-Properties`: 1084 симв. base64 → 811 Б JSON, 22 поля (18 строк, 2×u32,
  1 bool, 1 null).
- Выдержки в `rust/crates/discord-core/tests/data/` (read-only): `rest-super-properties-2323.b64`
  `b9e8f843…`, `…-2323.json` `90b06ded…`, `…-22.tsv` `95e2f861…`, `rest-base64-vectors.tsv`
  `8b5bb8c9…` (10 векторов, python3-оракул).
- Человекочитаемый разбор полей — `docs/re/rest-d02.md` §6.2 (`c7e61929…`).

## 3. Red-first

| Воркер | Тест (md5 финальный) | Red | Примечание |
|---|---|---|---|
| W9a base64 | `rest_base64.rs` `60c50ec3…` | 0/5, `# EXIT=101` | red переснят на финальной редакции (после fmt и кейса N1); плюс mutation-red нита N1 |
| W9b супер-свойства | `rest_super_properties.rs` `34dc6248…` | 0/8, `# EXIT=101` | тест после red не менялся |
| W9c идентичность | `rest_identity.rs` `9946bc26…` | 0/8 (первый), 7/8 (пересъём), `# EXIT=101` | на пересъёме I8 (плумбинг base64/headers) зелёный; политика пинится I1–I7 |

Red-копии = финальным тестам (md5 совпадают): `red/rest_base64-red.rs`, `red/rest_super_properties-red.rs`,
`red/rest_identity-red.rs`. Логи: `red/w9a-red.log` (до fmt), `red/w9a-red-fmt.log` (финальный),
`red/w9a-red-mutation-n1.log`, `red/w9b-red.log`, `red/w9c-red.log` (финальный), плюс промежуточные
`w9c-red-first.log`, `w9c-red-unformatted.log`, `w9c-red-prefmt.log`.

## 4. Рефакторинг W9d и ревью W9r

- **W9d** — не-поведенческий рефакторинг 3 src-файлов: `base64.rs` (приватные `push_sextet`,
  `data_length`), `super_properties.rs` (приватный `JsonWriter`, `consume_keyword`, `skip_digits`),
  `identity.rs` (хелперы `configured_or`, `or_unknown`, `has_env`). Тесты и тест-данные не менялись;
  вне репозитория — differential-прогоны (base64 ~400 тыс. кейсов, JSON ~200 тыс.) без расхождений.
  Патч — `reports/w9d-diff.patch` (`35a01f33…`).
- **W9r** (независимо): оракулы пересчитаны своими командами (снимок, выдержки побайтово,
  python3-векторы, 22 поля, «221 запрос» = 71 + 150), red воспроизведён заглушками
  (0/5, 0/8, 0/8), границы чисты, **20/21 мутаций поймано**. Отчёт — `waves/reviews/review-rw-09.md`.
  Политика §6.1 проверена на реальной машине: `Linux / 7.2.10-1-cachyos / x64 / ru / KDE /
  "Arch Linux" / wayland`, свои `discord-lab 0.1.0`, свежие UUID v4.

## 5. Ниты ревью и их закрытие

- **N1/N6 (закрыт red-first):** в `rest_base64.rs` добавлены кейсы `decode("A=")` →
  `InvalidPadding{index:1}` и `decode("AAAAA=")` → `InvalidPadding{index:5}`; мутация «снять
  `first_pad % 4 == 1`» ловится (`InvalidLength(2)` против `InvalidPadding{index:1}`,
  `# EXIT=101`) — лог `red/w9a-red-mutation-n1.log`; red и red-копия пересняты на финальном тесте.
- **N2 (аннотация):** в отчёте W9a указан md5 теста до финального fmt (`f691aef6…`); фактический —
  `60c50ec3…`; red на фактической редакции переснят.
- **N3 (backlog):** политика `config.Some("")` (дословно, не «отсутствие») — внесена в `TASKS.md`
  Backlog.
- **N4 (исправлен):** в шапке `rest_identity.rs` обновлён md5 ТЗ (`5dc28158…`); red переснят.
- **N5 (исправлен):** §4 ТЗ приведён к фактическому порядку проверок (паддинг/символы → длина →
  данные) + Поправка координатора.
- **Наблюдение:** I8 на заглушке зелёный (проверяет только плумбинг base64 + сборку заголовков) —
  принято осознанно: политика фиксируется I1–I7.

## 6. Отступления и замечания

- Поправка ТЗ §4: при расхождении прозы и кейсов приоритет у кейсов (B3/P8) — «паддинг/символы →
  длина»; затронула W9a и W9b.
- `rustfmt` W9a пришёлся после red (ассерты не менялись) — прецедент RW-08/W8a: red переснят
  координатором на финальной редакции теста.
- Критерий T-REST-04 («22 поля, base64 JSON, round-trip») покрыт побайтово на живом заголовке.

## 7. Финальные md5 (снимок — `final/`, манифест — `final/MANIFEST.md`)

| Файл | md5 |
|---|---|
| `src/rest/base64.rs` | `fb49dae8d59f03b2124b83e1c73aa2f4` |
| `src/rest/super_properties.rs` | `6ef2ac9d5fe452fb3eb583254212719c` |
| `src/rest/identity.rs` | `25d5005219ee7e3cc26ded118f017e0d` |
| `src/rest/mod.rs` | `0eba823bf46c2958cac102dbb42cbb06` |
| `tests/rest_base64.rs` | `60c50ec340537f0f88e81bf5359c1f6f` |
| `tests/rest_super_properties.rs` | `34dc62487dfb99553d7f99449fb67911` |
| `tests/rest_identity.rs` | `9946bc26c7014a9778f8ae8d91a12c15` |
| `tests/data/rest-base64-vectors.tsv` | `8b5bb8c97841773bd13e0fb9f29e8b51` |
| `tests/data/rest-super-properties-22.tsv` | `95e2f8616043041431fcef2d38d6fac5` |
| `tests/data/rest-super-properties-2323.b64` | `b9e8f8430d8b496df50da9419e760a2d` |
| `tests/data/rest-super-properties-2323.json` | `90b06ded88a98dd05d3b821f6bedc52b` |

## 8. Вне волны

- Реализация HTTP-транспорта (решение владельца: вариант A — ручной HTTP/1.1 на tokio+tokio-rustls;
  разбор — `waves/TRANSPORT.md` §6) — отдельная волна до первых живых REST-запросов.
- Отправка `X-Super-Properties` в живых запросах; 429/`Retry-After` и 401 — карточка P1-09;
  живой REST-пайплайн — P2-06.
