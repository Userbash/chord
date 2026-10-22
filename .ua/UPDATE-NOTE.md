# .ua — граф Understand-Anything (обновление 2026-10-20)

Канонический граф: `knowledge-graph.json` — **97 файлов, 232 узла, 432 ребра, 8 слоёв,
12-шаговый tour**. Язык — русский, анализ полный, `autoUpdate=false`.

## Что изменилось после первичного анализа

Первичный скан (`intermediate/scan-result.json`, батчи) снят 2026-10-20 до приёмки P2-01.
После неё граф обновлён инкрементально:

- удалён legacy `rust/crates/discord-core/src/ready_data.rs` и его тест — 12 узлов и
  15 рёбер (классы/функции/`tested_by`);
- добавлен `rust/crates/discord-core/tests/ready_review.rs` — 1 file-узел и 5 рёбер
  `tested_by` от `ready.rs`/`snapshot.rs`/`guild.rs`/`social.rs`/`value.rs`;
- обновлены `layers` (testing), `tour` (шаг 10), `project.analyzedAt`, `fingerprints.json`,
  `meta.json` и `intermediate/scan-result.json` (97 файлов).

`intermediate/batch-*.json`, `assembled-graph.json`, `layers.json`, `tour.json` — **сырой
снимок первичного анализа**; они не пересобирались. Для полной пересборки графа нужно
заново запустить Understand-Anything по `.ua/.understandignore` (он охватывает Rust-воркспейс
и профильные доки; архив волн, golden и legacy RE-CLI исключены). Канонический результат —
только `knowledge-graph.json`.
