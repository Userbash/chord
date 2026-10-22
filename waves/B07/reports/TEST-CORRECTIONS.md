Batch B07 — Markdown parser fix (P7-01 / emoji + slash command nodes)

Golden reference: docs/re/i18n-deep.md §3 (markdown rules). RE spec confirms:
- Custom emoji: `<a:name:id>` or `<:name:id>` (regex `/^<(a)?:(\w+):(\d+)>/`).
- Slash command: `</name:id>` (regex `/^<\/((?:[-_...]{1,32})...):\d+>/`).

Bug: In `parse_object`, the custom-emoji branch used `map_or((false, inside.strip_prefix(':')?), ...)` — the `?` on `strip_prefix(':')` propagated `None` out of the entire `parse_object` function when the input was not an emoji (e.g. `</cmd:123>`). That prevented the slash-command branch from running.

Fix: Replaced with a self-contained emoji block that only returns early on a valid emoji match, without short-circuiting the function.

Files touched (assigned): `rust/crates/discord-i18n/src/markdown.rs` (only this file per file matrix).
Red: `emoji_and_slash_command_nodes_are_reachable` failed with `parse_markdown("</cmd:123>")` not matching `SlashCommand`. Green: passes after fix.
No test corrections needed — the test contract aligns with RE spec; the parser was wrong.

Separate finding (not fixed, out of card scope): `plural_contract.rs` fails `plural_edge_cases_follow_the_locale_contract` for `ar 3.5` (expects "other", gets "few"). The `ar` cardinal branch in `plural.rs` applies `n_mod_100` ranges to fractional values; CLDR requires `v == 0` guard before integer-range rules. File `plural.rs` is owned by W25a — report to Team Lead.
