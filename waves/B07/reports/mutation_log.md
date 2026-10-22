Mutation (1): `parse_object` emoji branch — removed `map_or(... ? ...)` that propagated `None` from the function body. Replaced with self-contained `emoji_rest` match that only returns early on valid emoji (`<a:...>` / `<:...>`), allowing slash command (`</...>`) branch to execute afterward.
Reference: docs/re/i18n-deep.md §3 regex lines 116–118.
