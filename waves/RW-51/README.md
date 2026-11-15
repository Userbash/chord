# RW-51 review package

132 annotated leaf tasks, 26 workstreams. All 220 historical Kanban cards are preserved; the combined board contains 352 cards.

- [Plan and execution strategy](PLAN.md)
- [ARM blocker, CPU/ELF/SDK and native qualification](ARM-BLOCKERS.md)
- [ARM source implementation batch and remaining gates](ARM-SOURCE-BATCH.md), [machine receipt](ARM-SOURCE-BATCH.json)
- [Sequential step 1: locked Rust dependency cache restored](SEQUENTIAL-01-DEPENDENCIES.md)
- [Library decisions and rejection gates](LIBRARY-MATRIX.md)
- [Human task breakdown](TASKS.md), [full machine-readable tasks](TASKS.json)
- [Worker instructions](WORKER-RULES.md)
- [Measured source inventory](INVENTORY.json), [physical-line baseline](SOURCE-BASELINE.csv)
- [ECC installation](ECC-INSTALLATION.md), [operator harness exception](OPERATOR-OVERRIDE.md)
- [Complete handoff mandates](HANDOFF.json), [actual execution receipts](DISPATCH-RECEIPTS.json)
- First ECC reports: [feature ledger](reports/LIB-GOV-01.json), [dependency intake](reports/LIB-GOV-02.json), [RSA contract](reports/LIB-RSA-01.json)
- ARM follow-up: [source audit](reports/LIB-ARM-01.json), [failure ledger](ARM-FAILURE-LEDGER.json)
- Combined [interactive Kanban](../kanban-board.html)

The first three genuine ECC planning sessions completed 34 findings; a fourth ARM audit followed. These planning reports state their source-only limits. Two further genuine ECC sessions reviewed the bounded ARM source implementation. Generic compiler inputs and ELF checks/metadata now have code changes and executable regression tests. Parent cards and application library migrations remain gated by product review, the full batch gate, real target artifacts and native qualification.
