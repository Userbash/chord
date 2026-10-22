Batch B07 — headless (no GUI/window needed)
Card: P7-01 markdown contract — emoji + slash command nodes reachable.
File matrix (TEAM.md): only `rust/crates/discord-i18n/src/markdown.rs` assigned to current fix.
Golden method (CODE-STANDARD.md): RE spec → frozen contract → red → minimal green → mutation → review.
Red: `emoji_and_slash_command_nodes_are_reachable` failed (parser returned None for `</cmd:123>`).
Green: passes; all 13 markdown_contract tests pass.
Mutation: replaced `map_or(... ? ...)` with self-contained emoji block (no function-level short-circuit).
Independent review: confirmed against `docs/re/i18n-deep.md` §3 regexes (line 116–118) and `frontend-d03.md` §5.
No docs updated (pending accepted per PROCESS.md §8); `waves/STATE.md` not modified (Team Lead sync after acceptance).
