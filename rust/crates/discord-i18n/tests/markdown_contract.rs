use discord_i18n::markdown::{MarkdownNode, parse_markdown, render_markdown};

#[test]
fn renders_the_23_frozen_markdown_rules() {
    let cases = [
        ("**bold**", "**bold**"),
        ("__under__", "__under__"),
        ("~~strike~~", "~~strike~~"),
        ("*em*", "*em*"),
        ("_em_", "*em*"),
        ("`code`", "`code`"),
        ("```rs\nlet x = 1;\n```", "```rs\nlet x = 1;\n```"),
        ("||spoiler||", "||spoiler||"),
        ("> quote", "> quote"),
        ("# H1", "# H1"),
        ("## H2", "## H2"),
        ("### H3", "### H3"),
        ("-# small", "-# small"),
        ("- a\n- b", "- a\n- b"),
        ("1. a\n2. b", "1. a\n2. b"),
        ("@silent", "@silent"),
        ("<@123>", "<@123>"),
        ("<@!123>", "<@!123>"),
        ("@everyone", "@everyone"),
        ("@here", "@here"),
        ("<@&123>", "<@&123>"),
        ("<#123>", "<#123>"),
        ("</cmd arg:123>", "</cmd arg:123>"),
        ("<a:wave:123456789012345678>", "<a:wave:123456789012345678>"),
        ("<:wave:123456789012345678>", "<:wave:123456789012345678>"),
        (":wave:", ":wave:"),
        (":wave::skin-tone-3:", ":wave::skin-tone-3:"),
        ("<t:1712345678:R>", "<t:1712345678:R>"),
        ("<sound:123:456>", "<sound:123:456>"),
        ("<id:home>", "<id:home>"),
        ("<id:browse:123>", "<id:browse:123>"),
        ("[x](https://a.test)", "[x](https://a.test)"),
        ("<https://a.test>", "[https://a.test](https://a.test)"),
        ("https://a.test", "[https://a.test](https://a.test)"),
        (r"\*", "*"),
        (r"\\", "\\"),
        (r"\a", r"\a"),
    ];

    for (index, (input, expected)) in cases.iter().enumerate() {
        assert_eq!(
            render_markdown(&parse_markdown(input)),
            *expected,
            "rule {}: {input:?}",
            index + 1
        );
    }
}

#[test]
fn rejects_unsafe_urls_but_keeps_labels() {
    for input in [
        "[x](javascript:alert(1))",
        "[x](data:text/html,evil)",
        "[x](vbscript:run)",
    ] {
        assert_eq!(render_markdown(&parse_markdown(input)), "x", "{input}");
    }
    assert_eq!(
        render_markdown(&parse_markdown("<https://safe.test>")),
        "[https://safe.test](https://safe.test)"
    );
    assert_eq!(
        render_markdown(&parse_markdown("[x](https://safe.test)")),
        "[x](https://safe.test)"
    );
}

#[test]
fn keeps_html_as_inert_text_and_preserves_unclosed_markers() {
    for input in [
        "<script>alert(1)</script>",
        "**open",
        "||open",
        "****",
        "```\nbody",
        "#### H4",
        "#tag",
        "-#text",
    ] {
        assert_eq!(render_markdown(&parse_markdown(input)), input, "{input}");
    }
    assert_eq!(render_markdown(&parse_markdown("")), "");
    assert_eq!(render_markdown(&parse_markdown("@silentx")), "@silentx");
    assert_eq!(render_markdown(&parse_markdown("<@abc>")), "<@abc>");
    assert_eq!(render_markdown(&parse_markdown("<:x:123>")), "<:x:123>");
}

#[test]
fn supports_nested_styles_and_protects_code() {
    for (input, expected) in [
        ("**bold *em***", "**bold *em***"),
        ("**`x`**", "**`x`**"),
        ("\u{005c}*", "*"),
    ] {
        assert_eq!(render_markdown(&parse_markdown(input)), expected, "{input}");
    }
}

#[test]
fn arbitrary_utf8_input_never_panics() {
    for input in ["🦀🦀", "🦀https://safe.test", "текст🦀**x**", "🦀"] {
        let _ = render_markdown(&parse_markdown(input));
    }
}

#[test]
fn renderer_revalidates_manually_constructed_links() {
    let unsafe_link = MarkdownNode::Link {
        label: "x".into(),
        url: "javascript:alert(1)".into(),
    };
    assert_eq!(render_markdown(&unsafe_link), "x");
    let unsafe_auto = MarkdownNode::AutoLink("data:text/html,evil".into());
    assert_eq!(render_markdown(&unsafe_auto), "data:text/html,evil");
}

#[test]
fn ordered_list_number_overflow_does_not_panic() {
    let input = "18446744073709551615. a\n2. b";
    assert_eq!(
        render_markdown(&parse_markdown(input)),
        "18446744073709551615. a\n18446744073709551615. b"
    );
}

fn count_lists(node: &MarkdownNode) -> usize {
    match node {
        MarkdownNode::List { items, .. } => {
            1 + items.iter().flatten().map(count_lists).sum::<usize>()
        }
        MarkdownNode::Sequence(nodes) => nodes.iter().map(count_lists).sum(),
        _ => 0,
    }
}

#[test]
fn list_depth_limit_keeps_the_thirteenth_level_literal() {
    let input = (0..13)
        .map(|level| format!("{}- x", "  ".repeat(level)))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(count_lists(&parse_markdown(&input)), 12);
}

#[test]
fn nested_lists_preserve_structure_and_indent() {
    assert_eq!(
        render_markdown(&parse_markdown("- outer\n  - inner\n    - deep\n- next")),
        "- outer\n  - inner\n    - deep\n- next"
    );
    assert_eq!(
        render_markdown(&parse_markdown("1. outer\n  1. inner\n2. next")),
        "1. outer\n  1. inner\n2. next"
    );
    assert_eq!(
        render_markdown(&parse_markdown("- outer\n  1. inner\n  - mixed")),
        "- outer\n  1. inner\n  - mixed"
    );
}

#[test]
fn emoji_and_slash_command_nodes_are_reachable() {
    assert!(matches!(
        parse_markdown("</cmd:123>"),
        MarkdownNode::SlashCommand { name, id } if name == "cmd" && id == "123"
    ));
    assert!(matches!(
        parse_markdown("<a:wave:123456789012345678>"),
        MarkdownNode::CustomEmoji {
            animated: true,
            name,
            id
        } if name == "wave" && id == "123456789012345678"
    ));
    assert!(matches!(
        parse_markdown("<:wave:123456789012345678>"),
        MarkdownNode::CustomEmoji {
            animated: false,
            name,
            id
        } if name == "wave" && id == "123456789012345678"
    ));
}

#[test]
fn timestamp_and_slash_command_tokens_are_exact() {
    for input in [
        "<t:1712345678:Rbad>",
        "<t:1712345678:RR>",
        "<t:1712345678:>",
        "</:123>",
        "</bad{name:123>",
        "</aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:123>",
    ] {
        assert_eq!(render_markdown(&parse_markdown(input)), input, "{input}");
    }
}

#[test]
fn multiline_blockquote_keeps_the_quote_prefix() {
    assert_eq!(
        render_markdown(&parse_markdown("> first\n> second")),
        "> first\n> second"
    );
}

#[test]
fn parsing_and_rendering_are_bounded_for_arbitrary_boundaries() {
    let alphabet = ['a', '*', '_', '~', '|', '\\', '<', '>', '`', '\n', ' '];
    let mut state = 0x5eed_u64;
    for _ in 0..2_000 {
        let length = (state as usize % 48) + 1;
        let mut input = String::with_capacity(length);
        for _ in 0..length {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            input.push(alphabet[(state as usize >> 32) % alphabet.len()]);
        }
        let rendered = render_markdown(&parse_markdown(&input));
        assert!(!rendered.to_ascii_lowercase().contains("javascript:"));
        assert!(!rendered.to_ascii_lowercase().contains("data:"));
    }

    let input = "a🦀**b**\\*c";
    for (index, _) in input
        .char_indices()
        .chain(std::iter::once((input.len(), '\0')))
    {
        let rendered = render_markdown(&parse_markdown(&input[..index]));
        assert!(!rendered.to_ascii_lowercase().contains("javascript:"));
    }

    let nested_list = format!("{}- x", "  ".repeat(12));
    let _ = render_markdown(&parse_markdown(&nested_list));
}
