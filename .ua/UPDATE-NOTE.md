# .ua — Understand-Anything Graph (updated 2026-10-20)

Canonical graph: `knowledge-graph.json` — **97 files, 232 nodes, 432 edges, 8 layers,
12-step tour**. Language: English, full analysis, `autoUpdate=false`.

## What Changed Since Initial Analysis

The initial scan (`intermediate/scan-result.json`, batches) was taken on 2026-10-20 before P2-01 acceptance.
After acceptance the graph was updated incrementally:

- Removed legacy `rust/crates/discord-core/src/ready_data.rs` and its test — 12 nodes and
  15 edges (classes/functions/`tested_by`);
- Added `rust/crates/discord-core/tests/ready_review.rs` — 1 file node and 5 `tested_by` edges
  from `ready.rs`/`snapshot.rs`/`guild.rs`/`social.rs`/`value.rs`;
- Updated `layers` (testing), `tour` (step 10), `project.analyzedAt`, `fingerprints.json`,
  `meta.json`, and `intermediate/scan-result.json` (97 files).

`intermediate/batch-*.json`, `assembled-graph.json`, `layers.json`, `tour.json` — **raw
snapshot from the initial analysis**; they were not rebuilt. To fully rebuild the graph, run
Understand-Anything again against `.ua/.understandignore` (covers the Rust workspace
and profile docs; wave archive, golden, and legacy RE-CLI are excluded). The canonical result is
`knowledge-graph.json` only.
