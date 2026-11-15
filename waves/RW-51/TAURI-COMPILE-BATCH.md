# Transitional Tauri native compile batch

The user requested actual cross-platform compilation and publication of the Tauri transition. The existing core/GPUI workflow did not build `rust/gui-spike`. A separate six-target native Tauri lane now builds the real `chord-client` release with embedded frontend, preserves contract results and uploads complete source/hash-bound artifacts. Existing core/GPUI checks remain unchanged.

The local Linux x86_64 release **compiled successfully**, including `tauri/custom-protocol`: **30,948,608 bytes**, SHA256 `48a5f50797cc30f23dc1601afdb747896b7353471e0712b2cf0521c07a6c5717`. This is the actual Tauri executable, not a GPUI binary or synthetic artifact. It is retained outside immutable release directories. No GUI/runtime qualification is inferred. All three dependency locks are unchanged in this first configuration slice.

A Windows source check failed with **72 errors** from unconditional Unix/platform interfaces. The source audit identifies the required platform and credential boundaries. Actual Windows/macOS/ARM Tauri compilation remains pending native jobs and source repairs. A macOS core compile is not a Tauri application compile.

Validation: new plus old CI contracts **13/13**; full meta **216/216**; plain actionlint succeeds. Five initial source-contract mutations were detected. Independent review caught a partial-artifact upload bug; a new regression was RED, then GREEN after upload was bound to successful staging. The independent review has zero open findings and approves configuration only.

The worker gate still reports **2614 JS unit passed / 34 failed**, with all other recorded phases passing and their ignored/skipped cases retained. No failing test or priority was removed. Two gitleaks findings are exact SHA256 digests of public theme source files; only the exact immutable finding fingerprints are excluded, with their bytes independently verified. Scanner rerun remains pending CI.

Native CPU/GUI/audio/capture, credentials and ARM feature qualification remain separate. The Linux bridge, OS entropy, metrics/path and fallback findings in the [source audit](reports/TAURI-PLATFORM-AUDIT.json) are not silently claimed fixed.

Commands, per-target state and evidence: [TAURI-COMPILE-BATCH.json](TAURI-COMPILE-BATCH.json).
