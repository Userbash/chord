# RW-51 harness resolution and operator exception

Date: 2026-11-15. Scope: ECC workers for the requested Chord library-modernization plan and its admitted tasks.

The repository's internal model policy in `AGENTS.md` section 3 prohibits Codex and DeepSeek/OpenCode. The operator was shown the exact conflict and explicitly replied: **“Да, запускай через Codex”** (“Yes, launch through Codex”). This user instruction authorizes the Codex exception for this work. Other file, testing, safety and acceptance rules remain applicable.

The configured Gemini alias `gemini-4-argon-medium` returned HTTP404. The configured Google model-catalog request returned HTTP400 `FAILED_PRECONDITION`, reporting that the user location is unsupported. These were provider failures, not ECC installation failures. No geographic bypass, alternate unconfigured host or fabricated model identity was used.

Use the existing native Codex CLI/ChatGPT login, with the actual configured model `gpt-6.1-sol` and explicit medium effort for the initial planning audits. Record actual resolved model/effort per ECC session; do not label it as the old Gemini alias.

The first admitted phase is source-only analysis in isolated source worktrees. The coordinator persists returned reports. A completed planning session is not proof that application code or its migration tests have been implemented, passed review or shipped.

Source of truth for launches and outcomes: `DISPATCH-RECEIPTS.json`, the real ECC session store and captured output. Failed configuration attempts remain visible and are not counted as successful AI work.
