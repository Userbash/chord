**CHANGES**

- **P1 — [lib.rs](/home/sanya/orca/project/rust/crates/discord-i18n/src/lib.rs:515): exact plural selectors are checked after applying `offset`.** The contract checks `=N` against the original value, then subtracts the offset only for category selection. Minimal reproducer: plural node with `offset = 2` and branches `=3` / `other`, rendered with `n = 3`; it should choose `=3`, but this implementation checks `=1`.

- **P2 — [plural.rs](/home/sanya/orca/project/rust/crates/discord-i18n/src/plural.rs:288): Hebrew cardinal `many` is missing.** CLDR selects `many` for nonzero integer multiples of 10; this code returns `other`. Reproducer: `cardinal("he", 10)` should be `many`. Add oracle samples for 10 and 20.

- **P2 — [format.rs](/home/sanya/orca/project/rust/crates/discord-i18n/src/format.rs:295): date/time output replaces NBSP and narrow NBSP with ordinary spaces.** That discards ICU spacing present in locale patterns and timezone names, so exact oracle strings can differ. Reproducer: compare French or English short-time output with the ICU oracle where the pattern contains U+202F; assert exact separator preservation.

- **P2 — [format.rs](/home/sanya/orca/project/rust/crates/discord-i18n/src/format.rs:185): unknown currency output does not match the frozen fallback contract.** It formats using the locale’s separators and fraction digits, while the contract requires `"1234.50 XXX"`. Reproducer: `format_currency(1234.5, "en-US", "XXX")` should return `"1234.50 XXX"`.

- **P2 — [lib.rs](/home/sanya/orca/project/rust/crates/discord-i18n/src/lib.rs:471): Number AST nodes silently accept string arguments.** The integration contract specifies `InvalidArgument` for non-numeric values, but strings are returned unchanged. Reproducer: render `[2,"n"]` with `n = Value::Str("abc")`; expect `TranslateError::InvalidArgument`.

No commands or tests were run. Read-only inspection found no network calls or embedded secrets in the reviewed implementation. The code retains eight `lib.rs` unit tests, but I did not execute them; exact oracle coverage for currency fallback, string-valued Number nodes, and plural exact selectors with offset is absent.