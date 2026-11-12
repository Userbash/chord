#!/usr/bin/env python3
"""Run the RW-24 mutation matrix against the current green i18n implementation."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path("/home/user/orca/project")
RUST = ROOT / "rust"
OUT = ROOT / "waves/RW-24/mutations"
LOCK = "/tmp/opencode/discord-cargo.lock"

MUTATIONS = [
    {
        "id": 1,
        "file": "rust/crates/discord-i18n/src/plural.rs",
        "old": '            "ru" | "uk" => {\n                if v == 0 && i_mod_10 == 1 && i_mod_100 != 11 {\n                    cat("one")\n                } else if v == 0 && (2..=4).contains(&i_mod_10) && !(12..=14).contains(&i_mod_100) {',
        "new": '            "ru" | "uk" => {\n                if v == 0 && i_mod_10 == 1 && i_mod_100 != 11 {\n                    cat("one")\n                } else if v == 0 && (2..=5).contains(&i_mod_10) && !(12..=14).contains(&i_mod_100) {',
        "target": "plural_contract",
        "filter": "every_oracle_cardinal_and_ordinal_sample_matches",
    },
    {
        "id": 2,
        "file": "rust/crates/discord-i18n/src/plural.rs",
        "old": '                } else if (2..=4).contains(&i_mod_10) && !(12..=14).contains(&i_mod_100) {\n                    cat("few")\n                } else {\n                    cat("many")\n                }\n            }\n            "ru" | "uk" => {',
        "new": '                } else if (2..=4).contains(&i_mod_10) {\n                    cat("few")\n                } else {\n                    cat("many")\n                }\n            }\n            "ru" | "uk" => {',
        "target": "plural_contract",
        "filter": "every_oracle_cardinal_and_ordinal_sample_matches",
    },
    {
        "id": 3,
        "file": "rust/crates/discord-i18n/src/plural.rs",
        "old": '            "ar" => {\n                if n == 0.0 {\n                    cat("zero")\n                } else if n == 1.0 {\n                    cat("one")\n                } else if n == 2.0 {\n                    cat("two")',
        "new": '            "ar" => {\n                if n == 0.0 {\n                    cat("two")\n                } else if n == 1.0 {\n                    cat("one")\n                } else if n == 2.0 {\n                    cat("zero")',
        "target": "plural_contract",
        "filter": "every_oracle_cardinal_and_ordinal_sample_matches",
    },
    {
        "id": 4,
        "file": "rust/crates/discord-i18n/src/plural.rs",
        "old": '            "fr" => {\n                if i == 0 || i == 1 {\n                    cat("one")\n                } else if exact_million {\n                    cat("many")',
        "new": '            "fr" => {\n                if i == 0 || i == 1 {\n                    cat("one")\n                } else if exact_million {\n                    cat("other")',
        "target": "plural_contract",
        "filter": "every_oracle_cardinal_and_ordinal_sample_matches",
    },
    {
        "id": 5,
        "file": "rust/crates/discord-i18n/src/plural.rs",
        "old": "                if i % 100 / 10 != 1 {",
        "new": "                if i % 100 / 10 != 2 {",
        "target": "plural_contract",
        "filter": "every_oracle_cardinal_and_ordinal_sample_matches",
    },
    {
        "id": 6,
        "file": "rust/crates/discord-i18n/src/plural.rs",
        "old": '                if (i == 1 && v == 0) || (i == 0 && v != 0) {\n                    cat("one")',
        "new": '                if i == 1 && v == 0 {\n                    cat("one")',
        "target": "plural_contract",
        "filter": "every_oracle_cardinal_and_ordinal_sample_matches",
    },
    {
        "id": 7,
        "file": "rust/crates/discord-i18n/src/format.rs",
        "old": '    let group = if options.use_grouping { data.group } else { "" };',
        "new": '    let group = if options.use_grouping {\n        if locale.starts_with("ru") { " " } else { data.group }\n    } else {\n        ""\n    };',
        "target": "format_contract",
        "filter": "all_locale_numbers_match_the_independent_intl_oracle",
    },
    {
        "id": 8,
        "file": "rust/crates/discord-i18n/src/format.rs",
        "old": "        out.push_str(data.decimal);",
        "new": '        out.push_str(if locale.starts_with("fr") { "." } else { data.decimal });',
        "target": "format_contract",
        "filter": "all_locale_numbers_match_the_independent_intl_oracle",
    },
    {
        "id": 9,
        "file": "rust/crates/discord-i18n/src/format.rs",
        "old": "            data.group_secondary as usize,",
        "new": "            data.group_secondary as usize + 1,",
        "target": "format_contract",
        "filter": "all_locale_numbers_match_the_independent_intl_oracle",
    },
    {
        "id": 10,
        "file": "rust/crates/discord-i18n/src/format.rs",
        "old": "                ('M', _) => data.months_long[p.month - 1].to_owned(),",
        "new": "                ('M', _) => data.months_short[p.month - 1].to_owned(),",
        "target": "format_contract",
        "filter": "all_locale_dates_and_times_match_the_independent_intl_oracle",
    },
    {
        "id": 11,
        "file": "rust/crates/discord-i18n/src/format.rs",
        "old": "        parts(timestamp_secs, data.year_offset),\n        data,\n        false,",
        "new": "        parts(timestamp_secs, 0),\n        data,\n        false,",
        "target": "format_contract",
        "filter": "all_locale_dates_and_times_match_the_independent_intl_oracle",
    },
    {
        "id": 12,
        "file": "rust/crates/discord-i18n/src/format.rs",
        "old": "                ('a', _) => data.day_periods[usize::from(p.hour >= 12)].to_owned(),",
        "new": "                ('a', _) => data.day_periods[usize::from(p.hour < 12)].to_owned(),",
        "target": "format_contract",
        "filter": "all_locale_dates_and_times_match_the_independent_intl_oracle",
    },
    {
        "id": 13,
        "file": "rust/crates/discord-i18n/src/markdown.rs",
        "old": '        MarkdownNode::Bold(nodes) => wrap("**", "**", nodes),',
        "new": '        MarkdownNode::Bold(nodes) => wrap("*", "*", nodes),',
        "target": "markdown_contract",
        "filter": "renders_the_23_frozen_markdown_rules",
    },
    {
        "id": 14,
        "file": "rust/crates/discord-i18n/src/markdown.rs",
        "old": 'fn safe_url(url: &str) -> bool {\n    let url = url.trim().to_ascii_lowercase();\n    ["http://", "https://", "mailto:"]\n        .iter()\n        .any(|prefix| url.starts_with(prefix))\n}',
        "new": "fn safe_url(url: &str) -> bool {\n    let _ = url;\n    true\n}",
        "target": "markdown_contract",
        "filter": "rejects_unsafe_urls_but_keeps_labels",
    },
    {
        "id": 15,
        "file": "rust/crates/discord-i18n/src/markdown.rs",
        "old": "            if depth >= 11 {",
        "new": "            if depth >= 12 {",
        "target": "markdown_contract",
        "filter": "list_depth_limit_keeps_the_thirteenth_level_literal",
    },
    {
        "id": 16,
        "file": "rust/crates/discord-i18n/src/markdown.rs",
        "old": "                    nodes.push(MarkdownNode::Text(ch.to_string()));",
        "new": '                    nodes.push(MarkdownNode::Text(format!("\\\\{ch}")));',
        "target": "markdown_contract",
        "filter": "renders_the_23_frozen_markdown_rules",
    },
    {
        "id": 17,
        "file": "rust/crates/discord-i18n/src/locale.rs",
        "old": "pub fn is_enabled(code: &str) -> bool {\n    is_supported(code)\n        && !DISABLED_LOCALES\n            .iter()\n            .any(|candidate| candidate.eq_ignore_ascii_case(code))\n}",
        "new": "pub fn is_enabled(code: &str) -> bool {\n    is_supported(code)\n}",
        "target": "locale_contract",
        "filter": "registry_and_disabled_locales_match_the_frozen_inventory",
    },
    {
        "id": 18,
        "file": "rust/crates/discord-i18n/src/locale.rs",
        "old": "    if resolved == DEFAULT_LOCALE {\n        vec![resolved]\n    } else {\n        vec![resolved, DEFAULT_LOCALE.to_owned()]\n    }",
        "new": "    vec![resolved]",
        "target": "locale_contract",
        "filter": "locale_resolution_uses_available_enabled_exact_then_language_then_default",
    },
    {
        "id": 19,
        "file": "rust/crates/discord-i18n/src/lib.rs",
        "old": "            let b = opts\n                .get(&exact)",
        "new": "            let b = opts\n                .get(\"=__never__\")",
        "target": "i18n_contract",
        "filter": "exact_selector_offset_and_pound_use_plural_rules",
    },
    {
        "id": 20,
        "file": "rust/crates/discord-i18n/src/lib.rs",
        "old": "            let style = DateStyle::parse(style).unwrap_or(DateStyle::Medium);\n            format::format_date(value as i64, locale, style)",
        "new": "            let style = DateStyle::Medium;\n            format::format_date(value as i64, locale, style)",
        "target": "i18n_contract",
        "filter": "catalog_date_time_and_number_nodes_render_from_icu_oracle",
    },
]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    for mutation in MUTATIONS:
        path = ROOT / mutation["file"]
        original = path.read_text()
        old = mutation["old"]
        if original.count(old) != 1:
            results.append({**mutation, "status": "invalid-pattern", "count": original.count(old)})
            print(f"#{mutation['id']}: INVALID pattern count={original.count(old)}", flush=True)
            continue
        before = md5(path)
        try:
            path.write_text(original.replace(old, mutation["new"], 1))
            command = [
                "flock",
                LOCK,
                "cargo",
                "test",
                "--offline",
                "-p",
                "discord-i18n",
                "--test",
                mutation["target"],
                mutation["filter"],
                "--",
                "--exact",
            ]
            completed = subprocess.run(
                command,
                cwd=RUST,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=180,
            )
            log_path = OUT / f"mutation-{mutation['id']:02d}.log"
            log_path.write_text("$ " + " ".join(command) + "\n" + completed.stdout)
            compile_error = "error[E" in completed.stdout or "could not compile" in completed.stdout
            status = "inconclusive" if compile_error else ("caught" if completed.returncode != 0 else "missed")
            results.append({**mutation, "status": status, "exit": completed.returncode})
            print(f"#{mutation['id']}: {status} exit={completed.returncode}", flush=True)
        finally:
            path.write_text(original)
            after = md5(path)
            if before != after:
                results.append({**mutation, "status": "restore-mismatch", "before": before, "after": after})
                print(f"#{mutation['id']}: RESTORE MISMATCH", flush=True)
    report = {
        "total": len(results),
        "caught": sum(1 for item in results if item["status"] == "caught"),
        "missed": sum(1 for item in results if item["status"] == "missed"),
        "inconclusive": sum(1 for item in results if item["status"] == "inconclusive"),
        "results": results,
    }
    (OUT / "results.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({k: report[k] for k in ("total", "caught", "missed", "inconclusive")}, indent=2))
    return 0 if report["missed"] == 0 and report["inconclusive"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
