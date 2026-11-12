# waves — Rust Client Wave Journal (Recovery Point)

This directory serves as the persistent database of the development process:
it allows reconstructing **what has been implemented, how it was proven, and what comes next**,
after a failure, reboot, or pause. Tracked in git.

- `db.jsonl` — Event log: append-only, one JSON object per line. Past events are immutable.
- `STATE.md` — Current snapshot: accepted waves, active sprint, open questions, and next steps.
- `RW-XX/` — Wave artifacts: specifications, red/green test logs (`# EXIT=…`, MD5), reports, code backups, mutation logs.
- `reviews/` — Independent reviews (`review-*.md`) and reviewer artifacts.
- `_TEMPLATES/` — Reusable templates for batch specs, test matrices, worker reports, and mutation tables.

Waves represent **batches of 3–6 task cards** with unified integration testing and automated quality gates.
Roles and pipeline rules are defined in `docs/rust-client/TEAM.md`; batch roadmap in `WAVE-PLAN.md`.

## `db.jsonl` Format (Schema v1)

Mandatory fields: `v` (=1), `ts` (ISO timestamp), `event`, `summary`.
Optional metadata: `wave` (`RW-NN`), `card` (`P0-02`…), `tests` (`{wave, core, codec, workspace}`),
`md5` (file path -> MD5 at acceptance), `gate` (file in `.gate/`),
`commit` (hash), `evidence` (repository path).

Event vocabulary: `wave-planned`, `red`, `green`, `gate`, `review`, `accepted`, `commit`, `note`, `push`.

## Reading and State Recovery

```sh
tail -n 5 waves/db.jsonl | jq -c .                 # Latest events
jq -c 'select(.wave=="RW-07")' waves/db.jsonl      # Complete wave history
jq -r '[.wave // "-", .event, (.summary|tostring)] | @tsv' waves/db.jsonl
cat waves/STATE.md                                  # Current status and next actions
```

Recovery procedure: `git log --oneline -3` -> `cat waves/STATE.md` -> `tail waves/db.jsonl` -> task card in `PLAN.md` / `TASKS.md` -> implementation verified against `STATUS.md`.

## Operational Rules

1. **append-only**: `db.jsonl` is strictly append-only; historical wave records are never edited.
2. **Every accepted wave** maintains an `RW-XX/` artifact directory; upon acceptance, the coordinator logs the event and synchronizes `STATE.md` (verified by `tests/meta/waves-sync.test.mjs`).
3. **Plain text only**: `md`, `txt`, `log`, `rs`, `json`, `md5`, `sh`, `mjs`, `py`, `jsonl`, `hexl`, `toml`, `lock`. Binary builds and raw dumps reside in oracle directories or `/tmp`.
4. **Sanitization**: All logs and test traces sanitize sensitive data (`REDACTED`, `<PUB_IP>`).

## Wave Verification Ledger (Quick Reference)

| Wave | Card | Red-first | Log Artifacts (in `waves/`) | Status |
|---|---|---|---|---|
| RW-01 | P0-02 | 9+12, final 21/21 | `RW-01/red*.txt`, `RW-01/review/` | Accepted |
| RW-02 | P0-03 | 9/9 | `RW-02/red.txt`, `RW-02/reviewer/` | Accepted |
| RW-03 | P1-03 | 12/12 | `RW-03/red*.txt`, `RW-03/review.md` | Accepted |
| RW-04 | P1-03 | 10/10 | `RW-04/red*.txt`, `RW-04/review.md` | Accepted |
| RW-05 | P1-04 | 12F/1P | `RW-05/red*.txt`, `RW-05/mut/` | Accepted |
| RW-06 | P1-05 | 11/12 | `RW-06/red*.txt`, `RW-06/red-gap-tests.txt` | Accepted |
| RW-07 | P1-06 | 3P/8F | `RW-07/red*.log`, `RW-07/snapshot/` | Accepted |
| RW-08 | P1-07 | 1/7, 2/3, 0/10 | `RW-08/red/`, `RW-08/reports/` | Accepted |
| RW-09 | P1-08 | 0/5, 0/8, 0/8 | `RW-09/red/`, `RW-09/reports/` | Accepted |
| RW-10 | P1-07 infra | 16/3, 13/1 | `RW-10/reports/` | Accepted |
| RW-11 | P1-09/11/12 | 10+4+9 | `RW-11/reports/` | Accepted |
| RW-12 | P2-01 + I1 | 0/7, I1 13/3 | `RW-12/reports/` | Accepted |
| RW-13 | P2-02 | runtime 4F + compile-red | `RW-13/` red/green, gate | Accepted |
| RW-14 | P2-06 | compile-red + behavior-red | `RW-14/` red/green, gate | Accepted |
| RW-15 | P2-04 | compile-red + P2-1 red | `RW-15/` red/green, gate | Accepted |
| RW-16 | P2-05 | 3 targeted red | `RW-16/` red/green, gate, mutation | Accepted |
| RW-17 | P2-07 | W17a/b/c + P2-1 s11 | `RW-17/` red/green, gate, mutation 7/7 | Accepted |
| RW-18 | P2-09 | W18a/b/c + P2-1/2 + P3 | `RW-18/` red/green, gate, mutation 13/13 | Accepted |
| RW-19 | P2-10 | W19a/b/c | `RW-19/` contract, tests, red/green | Accepted |
| RW-20 | P2-11 | W20a/b/c | `RW-20/` contract, fixture 28 START, red/green | Accepted |
| RW-21 | P2-08/P1-01/P1-02/P0-04/P0-07 | B02 batch | `RW-21/` DESIGN, REPORT-B02, TEST-MATRIX, red/green, gate, mutations | Accepted |
| RW-22 | P3-15/P4-01/P7-04/P8-03 | B03 batch | `RW-22/` WAVE-PLAN, DESIGN-FOUNDATION, REPORT-B03, TEST-MATRIX-B03, red/green, gate, mutations 18/18 | Accepted |\n