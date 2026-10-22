//! Контракт P6-04: разбор `discord://` deep-link и маршрутизация.
//!
//! Независимый оракул:
//! - `docs/re/main-arch.md` §9.4 — дословная RE-цитата `extractUrlFromArgs`
//!   (`--url` только при наличии `--` после него, иначе argv с индекса 1 с префиксом `discord://`);
//! - строки бандлов `/home/sanya/re-deep/out/bundles/*.js` (только чтение):
//!   `discord://login/one-time?token=…`, `discord://action/oauth2/authorize`,
//!   `discord://app/open#…`, `discord://app/settings/nitro`, `discord:///library/${id}/launch`;
//! - `discord://channels/...` и `discord://-/channels/...` — форма навигации каналов.

use discord_platform::deep_link::{
    DeepLink, DeepLinkError, MAX_DEEP_LINK_LEN, Route, extract_deep_link, parse_deep_link, route,
};

fn parse(raw: &str) -> DeepLink<'_> {
    match parse_deep_link(raw) {
        Ok(link) => link,
        Err(err) => panic!("ожидался валидный deep link {raw:?}, получено {err:?}"),
    }
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn d01_app_settings_route() {
    let link = parse("discord://app/settings/nitro");
    assert_eq!(link.raw, "discord://app/settings/nitro");
    assert_eq!(link.host, "app");
    assert_eq!(link.path, "/settings/nitro");
    assert_eq!(link.query, None);
    assert_eq!(link.fragment, None);
    assert_eq!(
        route(&link),
        Route::AppSettings {
            section: "nitro".to_owned()
        }
    );
}

#[test]
fn d02_empty_host_library_route() {
    let link = parse("discord:///library/42/launch");
    assert_eq!(link.host, "");
    assert_eq!(link.path, "/library/42/launch");
    assert_eq!(
        route(&link),
        Route::Library {
            id: "42".to_owned()
        }
    );
}

#[test]
fn d03_login_one_time_token_is_redacted_in_debug() {
    let link = parse("discord://login/one-time?token=secret-token");
    assert_eq!(link.host, "login");
    assert_eq!(link.path, "/one-time");
    assert_eq!(link.query, Some("token=secret-token"));

    let parsed_route = route(&link);
    assert_eq!(
        parsed_route,
        Route::LoginOneTime {
            token: Some("secret-token".to_owned())
        }
    );
    let debug = format!("{parsed_route:?}");
    assert!(
        !debug.contains("secret-token"),
        "Debug не должен печатать токен: {debug}"
    );
    assert!(debug.contains("<redacted>"), "ожидалась redaction: {debug}");

    let without_token = route(&parse("discord://login/one-time"));
    assert_eq!(
        without_token,
        Route::LoginOneTime { token: None },
        "отсутствующий токен — валидная форма"
    );
    let none_debug = format!("{without_token:?}");
    assert!(none_debug.contains("None"), "{none_debug}");
    assert!(!none_debug.contains("<redacted>"), "{none_debug}");
}

#[test]
fn d04_oauth2_authorize_route() {
    let link = parse("discord://action/oauth2/authorize?client_id=1&scope=identify&state=x");
    assert_eq!(link.host, "action");
    assert_eq!(link.path, "/oauth2/authorize");
    assert_eq!(link.query, Some("client_id=1&scope=identify&state=x"));
    assert_eq!(route(&link), Route::OAuth2Authorize);
}

#[test]
fn d05_channels_routes() {
    let dashed = parse("discord://-/channels/1/2");
    assert_eq!(dashed.host, "-");
    assert_eq!(dashed.path, "/channels/1/2");
    assert_eq!(
        route(&dashed),
        Route::Channels {
            path: "/channels/1/2".to_owned()
        }
    );

    let direct = parse("discord://channels/@me");
    assert_eq!(direct.host, "channels");
    assert_eq!(direct.path, "/@me");
    assert_eq!(
        route(&direct),
        Route::Channels {
            path: "/channels/@me".to_owned()
        }
    );

    let empty_host = parse("discord:///channels/@me/123");
    assert_eq!(
        route(&empty_host),
        Route::Channels {
            path: "/channels/@me/123".to_owned()
        }
    );
}

#[test]
fn d06_rejects_empty_scheme_length_and_controls() {
    assert_eq!(parse_deep_link(""), Err(DeepLinkError::Empty));
    assert_eq!(
        parse_deep_link("https://discord.com/app"),
        Err(DeepLinkError::WrongScheme)
    );
    assert_eq!(
        parse_deep_link("discord:app/open"),
        Err(DeepLinkError::Malformed),
        "discord: без // — Malformed (ТЗ)"
    );
    assert_eq!(parse_deep_link("no-scheme"), Err(DeepLinkError::Malformed));
    assert_eq!(
        parse_deep_link("discord://ap p/open"),
        Err(DeepLinkError::Malformed),
        "пробел в host запрещён"
    );
    assert_eq!(
        parse_deep_link("discord ://app/open"),
        Err(DeepLinkError::Malformed),
        "пробел в scheme запрещён"
    );

    for raw in [
        "discord://app/open\n",
        "discord://app/open\r",
        "discord://app/open\u{1f}",
        "discord://app/open\u{7f}",
        "discord://app/open?x=\u{0}",
    ] {
        assert_eq!(
            parse_deep_link(raw),
            Err(DeepLinkError::Malformed),
            "control chars в raw: {raw:?}"
        );
    }

    // `%`-encoded control-последовательность — обычный текст, не control char.
    assert!(parse_deep_link("discord://app/open?x=%0A").is_ok());

    // Граница лимита: 4096 включительно, 4097 — TooLong.
    let at_limit = format!("discord://app/open#{}", "a".repeat(MAX_DEEP_LINK_LEN - 19));
    assert_eq!(at_limit.len(), MAX_DEEP_LINK_LEN);
    assert!(parse_deep_link(&at_limit).is_ok(), "4096 байт допустимы");
    let over_limit = format!("{at_limit}a");
    assert_eq!(parse_deep_link(&over_limit), Err(DeepLinkError::TooLong));
    assert_eq!(
        parse_deep_link(&format!(
            "https://discord.com/{}",
            "x".repeat(MAX_DEEP_LINK_LEN)
        )),
        Err(DeepLinkError::TooLong),
        "лимит проверяется до схемы"
    );
}

#[test]
fn d07_extract_deep_link_matches_re() {
    // RE: for (let i = 1; i < args.length; i++) if (args[i].toLowerCase().startsWith("discord://"))
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--flag", "DISCORD://app/open"])),
        Some("DISCORD://app/open")
    );
    assert_eq!(
        extract_deep_link(&args(&["discord://app/open"])),
        None,
        "argv[0] не сканируется"
    );
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--flag", "https://example.com"])),
        None
    );

    // RE: --url без `--` → null.
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--url", "discord://app/open"])),
        None
    );
    // RE: `--` раньше `--url` → null (сравнение индексов).
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--", "--url", "discord://app/open"])),
        None
    );
    // RE: `--` после `--url` → аргумент сразу после `--`.
    assert_eq!(
        extract_deep_link(&args(&[
            "discord-lab",
            "--url",
            "ignored",
            "--",
            "discord://login/one-time?token=x"
        ])),
        Some("discord://login/one-time?token=x")
    );
    // RE не валидирует содержимое аргумента после `--`.
    assert_eq!(
        extract_deep_link(&args(&[
            "discord-lab",
            "--url",
            "ignored",
            "--",
            "https://example.com"
        ])),
        Some("https://example.com")
    );
    // RE: args[passThroughArgsIndex + 1] === undefined → null.
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--url", "x", "--"])),
        None
    );
    assert_eq!(extract_deep_link(&[]), None);
}

#[test]
fn d08_query_and_fragment_are_split_before_path() {
    let link = parse("discord://app/open?x=1#frag");
    assert_eq!(link.path, "/open");
    assert_eq!(link.query, Some("x=1"));
    assert_eq!(link.fragment, Some("frag"));

    // `#` раньше `?`: остаток после `#` — fragment целиком.
    let fragment_first = parse("discord://app/open#frag?x=1");
    assert_eq!(fragment_first.path, "/open");
    assert_eq!(fragment_first.query, None);
    assert_eq!(fragment_first.fragment, Some("frag?x=1"));

    for (raw, query, fragment) in [
        ("discord://app/open?", None, None),
        ("discord://app/open#", None, None),
        ("discord://app/open?#", None, None),
        ("discord://app/open?x=1#", Some("x=1"), None),
        ("discord://app/open#frag", None, Some("frag")),
    ] {
        let parsed = parse(raw);
        assert_eq!(parsed.query, query, "{raw}");
        assert_eq!(parsed.fragment, fragment, "{raw}");
        assert_eq!(parsed.path, "/open", "{raw}");
    }
}

#[test]
fn d09_scheme_and_host_case_insensitive_path_case_sensitive() {
    let link = parse("DISCORD://APP/open");
    assert_eq!(link.host, "APP");
    assert_eq!(route(&link), Route::AppOpen);
    assert_eq!(
        route(&parse("discord://app/OPEN")),
        Route::Unknown,
        "путь чувствителен к регистру"
    );
}

#[test]
fn d10_unconfirmed_routes_are_unknown() {
    for raw in [
        "discord://app/not-open",
        "discord://library/42",
        "discord://app/settings",
        "discord://app/settings/",
        "discord://app/settings/nitro/extra",
        "discord://login/one-time/extra",
        "discord://action/oauth2",
        "discord://channels",
        "discord://-/channels",
        "discord://foo/bar",
        "discord://app/open/extra",
        "discord://-/library/42/launch",
        "discord://",
    ] {
        assert_eq!(route(&parse(raw)), Route::Unknown, "{raw}");
    }
}

#[test]
fn d11_authority_only_and_trailing_slash_paths() {
    let link = parse("discord://?x=1");
    assert_eq!(link.host, "");
    assert_eq!(link.path, "");
    assert_eq!(link.query, Some("x=1"));
    assert_eq!(link.fragment, None);
    assert_eq!(route(&link), Route::Unknown);

    let bare = parse("discord://");
    assert_eq!(bare.host, "");
    assert_eq!(bare.path, "");
    assert_eq!((bare.query, bare.fragment), (None, None));

    let trailing = parse("discord://app/");
    assert_eq!(trailing.host, "app");
    assert_eq!(trailing.path, "/");
    assert_eq!(route(&trailing), Route::Unknown);
}

#[test]
fn d12_token_extraction_edges() {
    for (raw, expected) in [
        ("discord://login/one-time?token=", None),
        ("discord://login/one-time?other=1", None),
        ("discord://login/one-time?a=1&token=t1&token=t2", Some("t1")),
        ("discord://login/one-time?TOKEN=upper", None),
        ("discord://login/one-time?token=a%20b", Some("a%20b")),
    ] {
        assert_eq!(
            route(&parse(raw)),
            Route::LoginOneTime {
                token: expected.map(str::to_owned)
            },
            "{raw}"
        );
    }
}

#[test]
fn d13_deep_link_debug_redacts_query_and_fragment() {
    let link = parse("discord://login/one-time?token=secret-token#frag-secret");
    let debug = format!("{link:?}");
    assert!(!debug.contains("secret"), "Debug утёк секрет: {debug}");
    assert!(!debug.contains("raw"), "Debug печатает raw: {debug}");
    assert!(debug.contains("login"), "host потерян: {debug}");
    assert!(debug.contains("<redacted>"), "нет redaction: {debug}");

    let plain = parse("discord://app/open");
    let plain_debug = format!("{plain:?}");
    assert!(plain_debug.contains("query: None"), "{plain_debug}");
    assert!(plain_debug.contains("fragment: None"), "{plain_debug}");
}
