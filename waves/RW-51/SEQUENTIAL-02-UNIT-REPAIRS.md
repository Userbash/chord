# Sequential step 2: five unit failures resolved

The observed full JS unit run improved from 39 to **34 failures**: **2614 passed, 34 failed, 0 skipped** out of 2648 tests. Failure-name comparison shows exactly five removed failures and no new failed names. This step remains in progress.

- Stream contracts: three tests followed the pre-extraction `main.rs` implementation. They now check IPC forwarding and the scoped implementation in `voice_plugin.rs`, retaining OP18, user-ID, permission, key-alias and fallback assertions. Linux and native Windows source checks pass13/13; five source-contract mutants are caught. The actual Rust data suite passes44/44. CP-010 records the independent basis.
- Theme oracle: two tests depended on an ignored Serein checkout. The exact upstream theme and licenses are now pinned as tracked test data with commit/blob/SHA256 provenance. All six cases pass, including the existing5/5 mutation case, in a clean detached checkout without the ignored mirror or node_modules. Per-fixture attributes preserve upstream hashes under a simulated CRLF checkout. CP-011 records the decision; application palettes are unchanged.

One genuine ECC source review completed as `ec872267`; the theme snapshot received a separate independent review and primary-source byte verification. The later per-fixture line-ending attributes have an explicit hash-preservation check. These are source/data checks, not native UI, voice or ARM qualification.

The remaining31 package/build checks need real release inputs and SDK qualification. The inspected GitHub release catalog has no compiled assets to restore. The next dependency is a genuine GPUI build and source-bound package pipeline, not generated stand-ins or altered historical checksums. The remaining3 Krisp cases require a deliberate runtime-capability decision: the earlier routing fix forced NC off, while tests still expect its earlier selectable behavior. Neither the DSP contract nor the VAD crash guard was changed by these repairs.

An intermediate meta run caught the new fixture files before they were staged. All six fixture files are now in the Git index and the fixture-tracking check passes. Earlier red/PATH/unstaged runs remain immutable evidence. The full worker gate and parent card remain open.

Exact hashes, commands, reviews and remaining-file counts: [SEQUENTIAL-02-UNIT-REPAIRS.json](SEQUENTIAL-02-UNIT-REPAIRS.json).
