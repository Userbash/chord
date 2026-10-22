//! Discord message markdown: 23 rules, safe normalized rendering (P7-01).
//!
//! Contract: `waves/RW-24/WAVE-PLAN.md` §3.4 and `TZ-W25b.md` §4.
//! The implementation is owned by W25b.

#![allow(clippy::collapsible_if, clippy::manual_strip)]

/// Bounded, renderer-neutral markdown tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownNode {
    Text(String),
    Sequence(Vec<MarkdownNode>),
    Bold(Vec<MarkdownNode>),
    Underline(Vec<MarkdownNode>),
    Strike(Vec<MarkdownNode>),
    Italic(Vec<MarkdownNode>),
    InlineCode(String),
    CodeBlock {
        language: String,
        code: String,
    },
    Spoiler(Vec<MarkdownNode>),
    BlockQuote(Vec<MarkdownNode>),
    Heading {
        level: u8,
        children: Vec<MarkdownNode>,
    },
    Subtext(Vec<MarkdownNode>),
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<Vec<MarkdownNode>>,
    },
    Silent,
    UserMention {
        id: String,
        nickname: bool,
    },
    Everyone,
    Here,
    RoleMention(String),
    ChannelMention(String),
    SlashCommand {
        name: String,
        id: String,
    },
    CustomEmoji {
        animated: bool,
        name: String,
        id: String,
    },
    Emoji {
        name: String,
        skin_tone: Option<u8>,
    },
    Timestamp {
        unix: i64,
        style: Option<char>,
    },
    Soundboard {
        guild_id: String,
        sound_id: String,
    },
    StaticRoute {
        route: String,
        id: Option<String>,
    },
    Link {
        label: String,
        url: String,
    },
    AutoLink(String),
}

/// Parse a safe subset of Discord markdown into a bounded tree.
#[must_use]
pub fn parse_markdown(input: &str) -> MarkdownNode {
    parse_blocks(input, 0)
}

/// Render a markdown tree as normalized Discord markdown.
#[must_use]
pub fn render_markdown(node: &MarkdownNode) -> String {
    match node {
        MarkdownNode::Text(s) => s.clone(),
        MarkdownNode::Sequence(nodes) => nodes.iter().map(render_markdown).collect(),
        MarkdownNode::Bold(nodes) => wrap("**", "**", nodes),
        MarkdownNode::Underline(nodes) => wrap("__", "__", nodes),
        MarkdownNode::Strike(nodes) => wrap("~~", "~~", nodes),
        MarkdownNode::Italic(nodes) => wrap("*", "*", nodes),
        MarkdownNode::InlineCode(s) => format!("`{}`", s),
        MarkdownNode::CodeBlock { language, code } => {
            let lang = if language.is_empty() {
                String::new()
            } else {
                format!("{language}\n")
            };
            format!("```{lang}{code}\n```")
        }
        MarkdownNode::Spoiler(nodes) => wrap("||", "||", nodes),
        MarkdownNode::BlockQuote(nodes) => render_nodes(nodes)
            .split('\n')
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        MarkdownNode::Heading { level, children } => format!(
            "{} {}",
            "#".repeat((*level).clamp(1, 3) as usize),
            render_nodes(children)
        ),
        MarkdownNode::Subtext(nodes) => format!("-# {}", render_nodes(nodes)),
        MarkdownNode::List {
            ordered,
            start,
            items,
        } => items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let body = render_nodes(item).replace('\n', "\n  ");
                if *ordered {
                    let number = start
                        .unwrap_or(1)
                        .saturating_add(u64::try_from(i).unwrap_or(u64::MAX));
                    format!("{number}. {body}")
                } else {
                    format!("- {body}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        MarkdownNode::Silent => "@silent".into(),
        MarkdownNode::UserMention { id, nickname } => {
            format!("<@{}{}>", if *nickname { "!" } else { "" }, id)
        }
        MarkdownNode::Everyone => "@everyone".into(),
        MarkdownNode::Here => "@here".into(),
        MarkdownNode::RoleMention(id) => format!("<@&{id}>"),
        MarkdownNode::ChannelMention(id) => format!("<#{id}>"),
        MarkdownNode::SlashCommand { name, id } => format!("</{name}:{id}>"),
        MarkdownNode::CustomEmoji { animated, name, id } => {
            format!("<{}:{name}:{id}>", if *animated { "a" } else { "" })
        }
        MarkdownNode::Emoji { name, skin_tone } => match skin_tone {
            Some(tone) => format!(":{name}::skin-tone-{tone}:"),
            None => format!(":{name}:"),
        },
        MarkdownNode::Timestamp { unix, style } => match style {
            Some(style) => format!("<t:{unix}:{style}>"),
            None => format!("<t:{unix}>"),
        },
        MarkdownNode::Soundboard { guild_id, sound_id } => format!("<sound:{guild_id}:{sound_id}>"),
        MarkdownNode::StaticRoute { route, id } => match id {
            Some(id) => format!("<id:{route}:{id}>"),
            None => format!("<id:{route}>"),
        },
        MarkdownNode::Link { label, url } => {
            if safe_url(url) {
                format!("[{label}]({url})")
            } else {
                label.clone()
            }
        }
        MarkdownNode::AutoLink(url) => {
            if is_http_url(url) {
                format!("[{url}]({url})")
            } else {
                url.clone()
            }
        }
    }
}

fn render_nodes(nodes: &[MarkdownNode]) -> String {
    nodes.iter().map(render_markdown).collect()
}
fn wrap(open: &str, close: &str, nodes: &[MarkdownNode]) -> String {
    format!("{open}{}{close}", render_nodes(nodes))
}

fn parse_blocks(input: &str, depth: usize) -> MarkdownNode {
    if depth >= 32 {
        return MarkdownNode::Text(input.to_owned());
    }
    let lines: Vec<&str> = input.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(first) = line.strip_prefix("```") {
            if let Some(end) =
                (i + 1..lines.len()).find(|&j| lines[j].trim_end_matches('\r') == "```")
            {
                let (language, body_start) = if first.is_empty() {
                    (String::new(), i + 1)
                } else {
                    (first.to_owned(), i + 1)
                };
                out.push(MarkdownNode::CodeBlock {
                    language,
                    code: lines[body_start..end].join("\n"),
                });
                i = end + 1;
                if i < lines.len() {
                    out.push(MarkdownNode::Text("\n".into()));
                }
                continue;
            }
        }
        if line.starts_with('>') {
            let mut quote = Vec::new();
            while i < lines.len() && lines[i].starts_with('>') {
                quote.push(
                    lines[i]
                        .strip_prefix('>')
                        .unwrap_or("")
                        .strip_prefix(' ')
                        .unwrap_or(lines[i].strip_prefix('>').unwrap_or("")),
                );
                i += 1;
            }
            out.push(MarkdownNode::BlockQuote(vec![parse_inline(
                &quote.join("\n"),
                0,
            )]));
            if i < lines.len() {
                out.push(MarkdownNode::Text("\n".into()));
            }
            continue;
        }
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=3).contains(&hashes) && trimmed[hashes..].starts_with(char::is_whitespace) {
            let text = trimmed[hashes..].trim().trim_end_matches('#').trim_end();
            out.push(MarkdownNode::Heading {
                level: hashes as u8,
                children: vec![parse_inline(text, 0)],
            });
            i += 1;
            if i < lines.len() {
                out.push(MarkdownNode::Text("\n".into()));
            }
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("-# ") {
            out.push(MarkdownNode::Subtext(vec![parse_inline(text, 0)]));
            i += 1;
            if i < lines.len() {
                out.push(MarkdownNode::Text("\n".into()));
            }
            continue;
        }
        if let Some((indent, ordered, _, _)) = list_item(line) {
            out.push(parse_list(&lines, &mut i, indent, ordered, 0));
            if i < lines.len() {
                out.push(MarkdownNode::Text("\n".into()));
            }
            continue;
        }
        out.push(parse_inline(line, 0));
        i += 1;
        if i < lines.len() {
            out.push(MarkdownNode::Text("\n".into()));
        }
    }
    sequence(out)
}

fn sequence(mut nodes: Vec<MarkdownNode>) -> MarkdownNode {
    if nodes.len() == 1 {
        nodes.remove(0)
    } else {
        MarkdownNode::Sequence(nodes)
    }
}

fn parse_list(
    lines: &[&str],
    i: &mut usize,
    indent: usize,
    ordered: bool,
    depth: usize,
) -> MarkdownNode {
    let mut items = Vec::new();
    let mut start = None;
    while *i < lines.len() {
        let Some((item_indent, item_ordered, item_start, content)) = list_item(lines[*i]) else {
            break;
        };
        if item_indent != indent || item_ordered != ordered {
            break;
        }
        if start.is_none() {
            start = item_start;
        }
        let mut item = vec![parse_inline(content, 0)];
        *i += 1;
        while *i < lines.len() {
            let Some((child_indent, child_ordered, _, _)) = list_item(lines[*i]) else {
                break;
            };
            if child_indent <= indent {
                break;
            }
            if depth >= 11 {
                item.push(MarkdownNode::Text("\n".into()));
                item.push(MarkdownNode::Text(lines[*i].to_owned()));
                *i += 1;
                continue;
            }
            item.push(MarkdownNode::Text("\n".into()));
            item.push(parse_list(lines, i, child_indent, child_ordered, depth + 1));
        }
        items.push(item);
    }
    MarkdownNode::List {
        ordered,
        start,
        items,
    }
}

fn valid_slash_name(name: &str) -> bool {
    let command = name.split_whitespace().next().unwrap_or("");
    !command.is_empty()
        && command.chars().count() <= 32
        && command
            .chars()
            .all(|c| c == '-' || c == '_' || c.is_alphabetic() || c.is_numeric())
}

fn list_item(line: &str) -> Option<(usize, bool, Option<u64>, &str)> {
    let s = line.trim_start();
    let indent = line[..line.len() - s.len()].chars().count();
    if let Some(rest) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")) {
        return Some((indent, false, None, rest));
    }
    let dot = s.find(". ")?;
    let digits = &s[..dot];
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Some((indent, true, digits.parse().ok(), &s[dot + 2..]));
    }
    None
}

fn parse_inline(input: &str, depth: usize) -> MarkdownNode {
    if depth >= 32 {
        return MarkdownNode::Text(input.to_owned());
    }
    let mut nodes = Vec::new();
    let mut rest = input;
    while !rest.is_empty() {
        if let Some((node, n)) = parse_object(rest) {
            nodes.push(node);
            rest = &rest[n..];
            continue;
        }
        if rest.starts_with('\\') {
            if let Some(ch) = rest[1..].chars().next() {
                if !ch.is_ascii_alphanumeric() && !ch.is_whitespace() {
                    nodes.push(MarkdownNode::Text(ch.to_string()));
                    rest = &rest[1 + ch.len_utf8()..];
                    continue;
                }
            }
        }
        if let Some((node, n)) = parse_link_or_url(rest) {
            nodes.push(node);
            rest = &rest[n..];
            continue;
        }
        let styles = [
            ("**", "**", 0u8),
            ("__", "__", 1),
            ("~~", "~~", 2),
            ("||", "||", 3),
            ("*", "*", 4),
            ("_", "_", 4),
            ("`", "`", 5),
        ];
        let mut matched = None;
        for (open, close, kind) in styles {
            if !rest.starts_with(open) {
                continue;
            }
            if let Some(end) = rest[open.len()..].find(close) {
                if end == 0 {
                    continue;
                }
                let body = &rest[open.len()..open.len() + end];
                if kind == 5 {
                    matched = Some((
                        MarkdownNode::InlineCode(body.to_owned()),
                        open.len() + end + close.len(),
                    ));
                    break;
                }
                if (kind == 0 && body.ends_with('*')) || (kind == 1 && body.ends_with('_')) {
                    continue;
                }
                let children = vec![parse_inline(body, depth + 1)];
                let node = match kind {
                    0 => MarkdownNode::Bold(children),
                    1 => MarkdownNode::Underline(children),
                    2 => MarkdownNode::Strike(children),
                    3 => MarkdownNode::Spoiler(children),
                    _ => MarkdownNode::Italic(children),
                };
                matched = Some((node, open.len() + end + close.len()));
                break;
            }
        }
        if let Some((node, n)) = matched {
            nodes.push(node);
            rest = &rest[n..];
            continue;
        }
        let ch = rest.chars().next().expect("nonempty remainder");
        nodes.push(MarkdownNode::Text(ch.to_string()));
        rest = &rest[ch.len_utf8()..];
    }
    sequence(nodes)
}

fn parse_link_or_url(s: &str) -> Option<(MarkdownNode, usize)> {
    if s.starts_with('[') {
        let close = s.find("](")?;
        let mut nesting = 0usize;
        let mut end = None;
        for (offset, ch) in s[close + 2..].char_indices() {
            match ch {
                '(' => nesting += 1,
                ')' if nesting == 0 => {
                    end = Some(close + 2 + offset);
                    break;
                }
                ')' => nesting -= 1,
                _ => {}
            }
        }
        let end = end?;
        let label = &s[1..close];
        let url = s[close + 2..end].trim();
        if safe_url(url) {
            return Some((
                MarkdownNode::Link {
                    label: label.to_owned(),
                    url: url.to_owned(),
                },
                end + 1,
            ));
        }
        return Some((parse_inline(label, 1), end + 1));
    }
    if s.starts_with('<') {
        if let Some(end) = s.find('>') {
            let url = &s[1..end];
            if is_http_url(url) {
                return Some((MarkdownNode::AutoLink(url.to_owned()), end + 1));
            }
        }
    }
    for scheme in ["https://", "http://"] {
        if s.get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
        {
            let end = s.find(char::is_whitespace).unwrap_or(s.len());
            let mut url = &s[..end];
            while url.ends_with(['.', ',', ':', ';', '"', ')', ']', '>']) {
                url = &url[..url.len() - 1];
            }
            if url.len() > scheme.len() {
                return Some((MarkdownNode::AutoLink(url.to_owned()), url.len()));
            }
        }
    }
    None
}

fn safe_url(url: &str) -> bool {
    let url = url.trim().to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}
fn is_http_url(url: &str) -> bool {
    safe_url(url)
        && (url.to_ascii_lowercase().starts_with("http://")
            || url.to_ascii_lowercase().starts_with("https://"))
}

fn parse_object(s: &str) -> Option<(MarkdownNode, usize)> {
    if s.starts_with("@silent") && s[7..].chars().next().is_none_or(char::is_whitespace) {
        return Some((MarkdownNode::Silent, 7));
    }
    for (literal, node) in [
        ("@everyone", MarkdownNode::Everyone),
        ("@here", MarkdownNode::Here),
    ] {
        if s.starts_with(literal)
            && s[literal.len()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        {
            return Some((node, literal.len()));
        }
    }
    if s.starts_with('<') {
        let end = s.find('>')?;
        let inside = &s[1..end];
        let digits = |v: &str| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit());
        if let Some(id) = inside
            .strip_prefix("@!")
            .or_else(|| inside.strip_prefix('@'))
        {
            if digits(id) {
                return Some((
                    MarkdownNode::UserMention {
                        id: id.into(),
                        nickname: inside.starts_with("@!"),
                    },
                    end + 1,
                ));
            }
        }
        if let Some(id) = inside.strip_prefix("@&") {
            if digits(id) {
                return Some((MarkdownNode::RoleMention(id.into()), end + 1));
            }
        }
        if let Some(id) = inside.strip_prefix('#') {
            if digits(id) {
                return Some((MarkdownNode::ChannelMention(id.into()), end + 1));
            }
        }
        if let Some(rest) = inside.strip_prefix("t:") {
            let (num, style, style_valid) = match rest.split_once(':') {
                None => (rest, None, true),
                Some((n, st)) => {
                    let mut chars = st.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) if "tTdDfFsSR".contains(c) => (n, Some(c), true),
                        _ => (n, None, false),
                    }
                }
            };
            if style_valid
                && !num.is_empty()
                && num.strip_prefix('-').unwrap_or(num).len() <= 17
                && num
                    .strip_prefix('-')
                    .unwrap_or(num)
                    .bytes()
                    .all(|b| b.is_ascii_digit())
            {
                if let Ok(unix) = num.parse() {
                    return Some((MarkdownNode::Timestamp { unix, style }, end + 1));
                }
            }
        }
        if let Some(rest) = inside.strip_prefix("sound:") {
            if let Some((guild, sound)) = rest.split_once(':') {
                if digits(guild) && digits(sound) {
                    return Some((
                        MarkdownNode::Soundboard {
                            guild_id: guild.into(),
                            sound_id: sound.into(),
                        },
                        end + 1,
                    ));
                }
            }
        }
        if let Some(rest) = inside.strip_prefix("id:") {
            let (route, id) = rest
                .split_once(':')
                .map_or((rest, None), |(r, id)| (r, Some(id)));
            if ["home", "browse", "customize", "guide", "linked-roles"].contains(&route)
                && id.is_none_or(digits)
            {
                return Some((
                    MarkdownNode::StaticRoute {
                        route: route.into(),
                        id: id.map(str::to_owned),
                    },
                    end + 1,
                ));
            }
        }
        // Custom emoji <a:name:123> / <:name:123> — self-contained so it never
        // short-circuits the whole parse_object when inside lacks ':' prefix.
        {
            let emoji_rest = match inside.strip_prefix("a:") {
                Some(rest) => Some((true, rest)),
                None => inside.strip_prefix(':').map(|rest| (false, rest)),
            };
            if let Some((animated, rest)) = emoji_rest {
                if let Some((name, id)) = rest.split_once(':') {
                    if !name.is_empty()
                        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        && (17..=19).contains(&id.len())
                        && digits(id)
                    {
                        return Some((
                            MarkdownNode::CustomEmoji {
                                animated,
                                name: name.into(),
                                id: id.into(),
                            },
                            end + 1,
                        ));
                    }
                }
            }
        }
        if let Some(rest) = inside.strip_prefix('/') {
            if let Some((name, id)) = rest.rsplit_once(':') {
                if valid_slash_name(name) && digits(id) {
                    return Some((
                        MarkdownNode::SlashCommand {
                            name: name.into(),
                            id: id.into(),
                        },
                        end + 1,
                    ));
                }
            }
        }
    }
    if s.starts_with(':') {
        if let Some(end) = s[1..].find(':') {
            let name = &s[1..end + 1];
            if !name.is_empty() && !name.chars().any(char::is_whitespace) {
                let suffix_start = end + 2;
                if let Some(tail) = s[suffix_start..].strip_prefix(":skin-tone-") {
                    if let Some(digits_end) = tail.find(':') {
                        if let Ok(tone) = tail[..digits_end].parse() {
                            return Some((
                                MarkdownNode::Emoji {
                                    name: name.into(),
                                    skin_tone: Some(tone),
                                },
                                suffix_start + ":skin-tone-".len() + digits_end + 1,
                            ));
                        }
                    }
                }
                return Some((
                    MarkdownNode::Emoji {
                        name: name.into(),
                        skin_tone: None,
                    },
                    end + 2,
                ));
            }
        }
    }
    None
}
