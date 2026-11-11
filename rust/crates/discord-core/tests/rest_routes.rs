//! REST v9: каталог 49 маршрутов — 47 живых строк D-02 плюс 2 `[S]`/raw-расширения
//! (T-REST-01, волна RW-08 / P1-07; маркировка расширений — RW-40 BE-P3-02).
//!
//! **Провенанс оракулов** (независимые артефакты захвата D-02, не подгонка под реализацию):
//! - источник `docs/re/rest-d02.json`, md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`:
//!   47 уникальных API-маршрутов — GET 40, POST 3, PUT 2, PATCH 2 (DELETE 0); статусы
//!   {200, 204, 304, 404, 429}; плейсхолдеры `{id}` ×15 и `{num}` ×2; тело запроса — у 6
//!   маршрутов (ключи дословно, сентинелы `(empty-object)` / `(non-json)`).
//! - выдержка `tests/data/rest-routes-47.tsv` (47 строк), md5
//!   `2db6b39bcba96bc3a2d480fea9461a33`:
//!   `jq -r '.endpoints[] | [.method, .path, (.reqBodyKeys|join(",")), (.statuses|map(tostring)|join("|"))] | @tsv' docs/re/rest-d02.json`
//! - выдержка `tests/data/rest-crossref-47.tsv` (47 строк), md5
//!   `1569384026aa85ba2854f87a7d7048dd`:
//!   `jq -r '.crossRef.details[] | [.method, .path, .verdict, (.staticKey//"-"), (.staticPath//"-"), (.staticMethod//"-")] | @tsv' docs/re/rest-d02.json`
//! - кросс-сверка: 44 static-confirmed, 2 static-method-unresolved, 1 static-absent; у 44
//!   подтверждённых путь сходится после `strip("/api/v9")` и `{id}`/`{num}` → `{arg}`.
//!
//! Проверка выдержек: `md5sum docs/re/rest-d02.json rust/crates/discord-core/tests/data/rest-*.tsv`.
//! Данные лежат в репозитории: нет файла — падение, не skip (ТЗ RW-08 §2.2).

use discord_core::rest::routes::{
    CAPTURED_D02_ROUTE_COUNT, CAPTURED_QUERY_PARAMS, CHANNEL_SEARCH_PATH, ChannelMessagesParams,
    DEFAULT_PINS_LIMIT, HttpMethod, INVITE_PATH, MAX_PINS_LIMIT, MessageSearchParams,
    PathRenderError, QueryBuilder, QueryError, QueryParamSpec, QueryParamType, ROUTES, RouteSpec,
    STATIC_EXTENSION_ROUTE_COUNT, TYPING_PATH, ThreadSearchParams, build_channel_messages_path,
    build_channel_pins_path, build_channel_search_path, build_guild_search_path,
    build_thread_search_path, encode_query_component, find, render_path,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// База v9 в путях выдержек (в каталоге пути относительные, без базы).
const API_V9_PREFIX: &str = "/api/v9";

/// Выдержка «метод, путь, ключи тела, статусы» (47 строк).
const ROUTES_TSV: &str = "rest-routes-47.tsv";

/// Выдержка кросс-сверки «метод, путь, вердикт, staticKey, staticPath, staticMethod» (47 строк).
const CROSSREF_TSV: &str = "rest-crossref-47.tsv";

fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// Читает TSV без заголовка; пустые строки пропускаются.
fn read_rows(name: &str) -> Vec<Vec<String>> {
    let path = data_path(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("не прочитан {}: {error}", path.display()));
    let rows: Vec<Vec<String>> = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_string).collect())
        .collect();
    assert!(!rows.is_empty(), "пустая выдержка {}", path.display());
    rows
}

/// Имя метода из выдержки → вариант enum.
fn parse_method(name: &str) -> HttpMethod {
    match name {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        other => panic!("неизвестный HTTP-метод в выдержке: {other}"),
    }
}

/// Срезает базу `/api/v9` (у каждой строки выдержки она есть).
fn strip_base(path: &str) -> &str {
    path.strip_prefix(API_V9_PREFIX)
        .unwrap_or_else(|| panic!("путь без базы {API_V9_PREFIX}: {path}"))
}

/// Нормализация статической карты: `{id}`/`{num}` → `{arg}`.
fn normalize_path(path: &str) -> String {
    path.replace("{id}", "{arg}").replace("{num}", "{arg}")
}

#[test]
fn catalog_shape_48_unique() {
    // Кейс U1: D-02 плюс отдельный POST-контракт отправки сообщения.
    assert_eq!(ROUTES.len(), 49, "число маршрутов каталога");
    let mut seen = HashSet::new();
    for spec in &ROUTES {
        assert!(!spec.path.is_empty(), "пустой путь: {spec:?}");
        assert!(spec.path.starts_with('/'), "путь без '/': {}", spec.path);
        assert!(
            !spec.path.contains("api/v9"),
            "путь содержит базу: {}",
            spec.path
        );
        assert!(
            !spec.path.chars().any(char::is_whitespace),
            "пробел в пути: {}",
            spec.path
        );
        assert!(
            seen.insert((spec.method, spec.path)),
            "дубль пары (метод, путь): {spec:?}"
        );
    }
}

#[test]
fn guild_search_route_is_catalogued() {
    let spec = find(HttpMethod::Get, "/guilds/{id}/messages/search")
        .expect("маршрут серверного поиска гильдии");
    assert!(spec.body_keys.is_empty());
}

#[test]
fn catalog_matches_live_capture() {
    // Кейс U2: каждая строка выдержки ↔ маршрут каталога (в обе стороны), body_keys — дословно.
    let rows = read_rows(ROUTES_TSV);
    assert_eq!(rows.len(), 47, "строк в выдержке маршрутов");
    let mut captured = HashSet::new();
    for row in &rows {
        assert_eq!(row.len(), 4, "колонки строки: {row:?}");
        let method = parse_method(&row[0]);
        let path = strip_base(&row[1]);
        let spec = find(method, path)
            .unwrap_or_else(|| panic!("в каталоге нет маршрута {method:?} {path}"));
        assert_eq!(spec.method, method, "метод {path}");
        assert_eq!(spec.path, path, "путь {path}");
        assert_eq!(
            spec.body_keys.join(","),
            row[2],
            "body_keys {method:?} {path}"
        );
        assert!(
            captured.insert((method, path.to_string())),
            "дубль строки {method:?} {path}"
        );
    }
    assert_eq!(captured.len(), 47, "число строк захвата D-02");
    for spec in &ROUTES {
        if spec.method == HttpMethod::Get && spec.path == "/guilds/{id}/messages/search" {
            continue;
        }
        if spec.method == HttpMethod::Post && spec.path == "/channels/{id}/messages" {
            assert_eq!(spec.body_keys, &["content", "nonce", "tts"]);
            continue;
        }
        assert!(
            rows.iter()
                .any(|row| row[0] == spec.method.as_str() && strip_base(&row[1]) == spec.path),
            "маршрут каталога без строки захвата: {spec:?}"
        );
    }
}

#[test]
fn method_distribution_matches_capture() {
    // Кейс U3: распределение методов в выдержке и каталоге — GET 40, POST 3, PUT 2, PATCH 2,
    // DELETE 0 (DELETE в захвате D-02 не встречался).
    let rows = read_rows(ROUTES_TSV);
    let expected: [(&str, usize); 5] = [
        ("GET", 40),
        ("POST", 3),
        ("PUT", 2),
        ("PATCH", 2),
        ("DELETE", 0),
    ];
    let mut total = 0;
    for (name, count) in expected {
        let capture = rows.iter().filter(|row| row[0] == name).count();
        assert_eq!(capture, count, "выдержка: метод {name}");
        let catalog = ROUTES
            .iter()
            .filter(|spec| {
                spec.method.as_str() == name
                    && !(spec.method == HttpMethod::Post && spec.path == "/channels/{id}/messages")
                    && !(spec.method == HttpMethod::Get
                        && spec.path == "/guilds/{id}/messages/search")
            })
            .count();
        assert_eq!(catalog, count, "каталог: метод {name}");
        total += count;
    }
    assert_eq!(total, 47, "сумма по методам");
    assert_eq!(rows.len(), total, "все строки выдержки посчитаны");
}

#[test]
fn body_keys_only_on_six_routes() {
    // Кейс U4: тело — ровно у 6 маршрутов; ключи и сентинелы дословно из захвата (§3.1 ТЗ).
    let with_body: Vec<&RouteSpec> = ROUTES
        .iter()
        .filter(|spec| {
            !spec.body_keys.is_empty()
                && !(spec.method == HttpMethod::Post && spec.path == "/channels/{id}/messages")
        })
        .collect();
    assert_eq!(with_body.len(), 6, "маршруты с телом: {with_body:?}");
    let expected: [(HttpMethod, &str, &[&str]); 6] = [
        (
            HttpMethod::Post,
            "/channels/{id}/messages/{id}/ack",
            &["token", "last_viewed"],
        ),
        (
            HttpMethod::Post,
            "/users/@me/billing/user-offer",
            &["allow_triggers"],
        ),
        (
            HttpMethod::Put,
            "/guilds/{id}/members/@me",
            &["(empty-object)"],
        ),
        (HttpMethod::Post, "/beaker", &["token", "events"]),
        (
            HttpMethod::Patch,
            "/channels/{id}/explicit-media",
            &["message_ids"],
        ),
        (
            HttpMethod::Patch,
            "/users/@me/settings-proto/{num}",
            &["(non-json)"],
        ),
    ];
    for (method, path, keys) in expected {
        let spec = find(method, path)
            .unwrap_or_else(|| panic!("в каталоге нет маршрута {method:?} {path}"));
        assert_eq!(spec.body_keys, keys, "body_keys {method:?} {path}");
    }
    for spec in &with_body {
        assert!(
            expected
                .iter()
                .any(|(method, path, _)| *method == spec.method && *path == spec.path),
            "лишний маршрут с телом: {spec:?}"
        );
    }
}

#[test]
fn find_roundtrip_and_rejects() {
    // Кейс U5: find — точное равенство по методу и шаблону; база и префиксы не принимаются.
    for spec in &ROUTES {
        assert_eq!(
            find(spec.method, spec.path),
            Some(spec),
            "roundtrip {spec:?}"
        );
    }
    assert_eq!(
        find(HttpMethod::Get, "/api/v9/channels/{id}/messages"),
        None,
        "путь с базой не ищется"
    );
    assert_eq!(
        find(HttpMethod::Post, "/channels/{id}/messages").map(|spec| spec.body_keys),
        Some(["content", "nonce", "tts"].as_slice()),
        "POST отправляет сообщение"
    );
    assert_eq!(
        find(HttpMethod::Get, "/channels/{id}/messagesX"),
        None,
        "префикс не принимается"
    );
}

#[test]
fn method_names_cover_enum() {
    // Кейс U6: имена всех 5 вариантов; четыре наблюдаемых метода входят в набор.
    let all = [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
    ];
    let names: Vec<&str> = all.iter().map(|method| method.as_str()).collect();
    assert_eq!(names.as_slice(), ["GET", "POST", "PUT", "PATCH", "DELETE"]);
    let observed = [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
    ];
    for method in observed {
        assert!(
            names.contains(&method.as_str()),
            "наблюдаемый метод не покрыт: {method:?}"
        );
    }
    assert_eq!(
        HttpMethod::Delete.as_str(),
        "DELETE",
        "DELETE зарезервирован для будущего"
    );
}

#[test]
fn render_path_substitutes_and_rejects() {
    // Кейс U7: подстановка плейсхолдеров слева направо; части проверяются до рендера.
    let messages = find(HttpMethod::Get, "/channels/{id}/messages").expect("маршрут сообщений");
    assert_eq!(
        render_path(messages, &["123"]),
        Ok("/channels/123/messages".to_string())
    );

    let ack = find(HttpMethod::Post, "/channels/{id}/messages/{id}/ack").expect("маршрут ack");
    assert_eq!(
        render_path(ack, &["c1", "m2"]),
        Ok("/channels/c1/messages/m2/ack".to_string())
    );

    let public = find(HttpMethod::Get, "/applications/public").expect("публичный маршрут");
    assert_eq!(
        render_path(public, &[]),
        Ok("/applications/public".to_string())
    );

    let settings =
        find(HttpMethod::Get, "/users/@me/settings-proto/{num}").expect("маршрут настроек");
    assert_eq!(
        render_path(settings, &["42"]),
        Ok("/users/@me/settings-proto/42".to_string())
    );

    assert_eq!(
        render_path(messages, &[]),
        Err(PathRenderError::PartsMismatch {
            expected: 1,
            got: 0
        })
    );
    assert_eq!(
        render_path(messages, &["a", "b"]),
        Err(PathRenderError::PartsMismatch {
            expected: 1,
            got: 2
        })
    );
    assert_eq!(
        render_path(public, &["x"]),
        Err(PathRenderError::PartsMismatch {
            expected: 0,
            got: 1
        })
    );
    assert_eq!(
        render_path(messages, &["a{b}"]),
        Err(PathRenderError::InvalidPart("a{b}".to_string()))
    );
    assert_eq!(
        render_path(messages, &[""]),
        Err(PathRenderError::InvalidPart(String::new()))
    );
}

#[test]
fn crossref_static_confirms_catalog() {
    // Кейс U8: кросс-сверка со статикой — 44/2/1; пути сходятся после нормализации плейсхолдеров.
    let rows = read_rows(CROSSREF_TSV);
    assert_eq!(rows.len(), 47, "строк в кросс-сверке");
    let mut confirmed = 0usize;
    let mut unresolved = 0usize;
    let mut absent = 0usize;
    for row in &rows {
        assert_eq!(row.len(), 6, "колонки строки: {row:?}");
        let method = parse_method(&row[0]);
        let path = strip_base(&row[1]);
        assert!(
            find(method, path).is_some(),
            "в каталоге нет маршрута {method:?} {path}"
        );
        let static_path = row[4].as_str();
        let static_method = row[5].as_str();
        match row[2].as_str() {
            "static-confirmed" => {
                confirmed += 1;
                assert!(
                    static_method.split('|').any(|name| name == method.as_str()),
                    "метод {method:?} отсутствует в staticMethod {static_method:?} ({path})"
                );
                assert_eq!(
                    normalize_path(path),
                    static_path,
                    "staticPath для {method:?} {path}"
                );
            }
            "static-method-unresolved" => {
                unresolved += 1;
                assert_eq!(static_method, "-", "неразрешённый метод: {method:?} {path}");
            }
            "static-absent" => {
                absent += 1;
                assert_eq!(static_method, "-", "absent: метод {method:?} {path}");
                assert_eq!(static_path, "-", "absent: путь {method:?} {path}");
            }
            other => panic!("неизвестный вердикт кросс-сверки: {other}"),
        }
    }
    assert_eq!(
        (confirmed, unresolved, absent),
        (44, 2, 1),
        "вердикты кросс-сверки"
    );
}

#[test]
fn captured_query_params_catalog_matches_re_oracle_45() {
    // 45 параметров из D-02 (rest-d02.json queryParams)
    assert_eq!(CAPTURED_QUERY_PARAMS.len(), 45);
    let expected_core = [
        "archived",
        "before",
        "around",
        "limit",
        "offset",
        "sort_by",
        "sort_order",
        "tag_setting",
        "q",
        "client_heartbeat_session_id",
        "client_ad_session_id",
    ];
    for param in expected_core {
        assert!(
            CAPTURED_QUERY_PARAMS.contains(&param),
            "параметр {param} отсутствует в CAPTURED_QUERY_PARAMS"
        );
    }

    // Проверяем наличие query_params спецификаций у ключевых маршрутов
    let messages = find(HttpMethod::Get, "/channels/{id}/messages").expect("messages route");
    assert!(
        messages.query_params.iter().any(|q| q.name == "limit"),
        "messages limit param"
    );
    assert!(
        messages.query_params.iter().any(|q| q.name == "before"),
        "messages before param"
    );

    let pins = find(HttpMethod::Get, "/channels/{id}/messages/pins").expect("pins route");
    assert!(
        pins.query_params.iter().any(|q| q.name == "limit"),
        "pins limit param"
    );

    let threads =
        find(HttpMethod::Get, "/channels/{id}/threads/search").expect("threads search route");
    let archived = threads
        .query_params
        .iter()
        .find(|q| q.name == "archived")
        .expect("archived param in thread search");
    assert!(archived.required, "archived is required for thread search");
    assert_eq!(archived.param_type, QueryParamType::Boolean);

    let search = find(HttpMethod::Get, "/guilds/{id}/messages/search").expect("guild search route");
    assert!(
        search.query_params.iter().any(|q| q.name == "content"),
        "content param in search"
    );
}

#[test]
fn rfc3986_percent_encoding_matches_capture_and_standards() {
    // Unreserved characters: A-Z, a-z, 0-9, -, _, ., ~
    assert_eq!(
        encode_query_component(
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~"
        ),
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~"
    );

    // Reserved and special characters
    assert_eq!(encode_query_component("+"), "%2B");
    assert_eq!(encode_query_component("#"), "%23");
    assert_eq!(encode_query_component("&"), "%26");
    assert_eq!(encode_query_component(" "), "%20");
    assert_eq!(encode_query_component("="), "%3D");
    assert_eq!(encode_query_component("?"), "%3F");
    assert_eq!(encode_query_component("%"), "%25");
    assert_eq!(encode_query_component("a+b#c&d e"), "a%2Bb%23c%26d%20e");

    // Cyrillic and emoji from live capture capture-run2.jsonl
    let wire_query1 = "#💥┃правилатест";
    let encoded1 = encode_query_component(wire_query1);
    assert_eq!(
        encoded1,
        "%23%F0%9F%92%A5%E2%94%83%D0%BF%D1%80%D0%B0%D0%B2%D0%B8%D0%BB%D0%B0%D1%82%D0%B5%D1%81%D1%82"
    );

    let wire_query2 = "ProBot#🔴┃поговорить-тут";
    let encoded2 = encode_query_component(wire_query2);
    assert_eq!(
        encoded2,
        "ProBot%23%F0%9F%94%B4%E2%94%83%D0%BF%D0%BE%D0%B3%D0%BE%D0%B2%D0%BE%D1%80%D0%B8%D1%82%D1%8C-%D1%82%D1%83%D1%82"
    );
}

#[test]
fn query_builder_assembly_and_spec_validation() {
    let mut qb = QueryBuilder::new();
    assert_eq!(qb.build(), "");
    assert_eq!(qb.build_with_path("/test"), "/test");

    qb.param("q", "hello world");
    qb.param_display("limit", 25);
    qb.param_bool("archived", true);
    assert_eq!(qb.build(), "?q=hello%20world&limit=25&archived=true");
    assert_eq!(
        qb.build_with_path("/api/search"),
        "/api/search?q=hello%20world&limit=25&archived=true"
    );

    // Валидация RouteSpec с обязательным параметром
    static MOCK_QUERY_PARAMS: [QueryParamSpec; 2] = [
        QueryParamSpec::required("archived", QueryParamType::Boolean),
        QueryParamSpec::optional("limit", QueryParamType::Integer),
    ];
    let mock_spec = RouteSpec {
        method: HttpMethod::Get,
        path: "/mock",
        body_keys: &[],
        query_params: &MOCK_QUERY_PARAMS,
    };

    let mut qb2 = QueryBuilder::new();
    qb2.param_display("limit", 10);
    assert_eq!(
        qb2.validate_spec(&mock_spec),
        Err(QueryError::MissingRequiredParam("archived"))
    );

    qb2.param_bool("archived", false);
    assert_eq!(qb2.validate_spec(&mock_spec), Ok(()));
}

#[test]
fn channel_pins_path_matches_oracle_and_validates() {
    // Эталон из rest-static.json PINS.cacheEvidence:
    // https://discord.com/api/v9/channels/1058348665342808104/messages/pins?limit=25
    let oracle_channel = "1058348665342808104";

    // По умолчанию дефолт limit=25
    assert_eq!(DEFAULT_PINS_LIMIT, 25);
    assert_eq!(MAX_PINS_LIMIT, 50);

    let path_default = build_channel_pins_path(oracle_channel, None).expect("pins default");
    assert_eq!(
        path_default,
        "/channels/1058348665342808104/messages/pins?limit=25"
    );

    let path_explicit = build_channel_pins_path(oracle_channel, Some(25)).expect("pins 25");
    assert_eq!(
        path_explicit,
        "/channels/1058348665342808104/messages/pins?limit=25"
    );

    let path_custom = build_channel_pins_path(oracle_channel, Some(50)).expect("pins 50");
    assert_eq!(
        path_custom,
        "/channels/1058348665342808104/messages/pins?limit=50"
    );

    // Валидация границы: 0 недопустим
    match build_channel_pins_path(oracle_channel, Some(0)) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
        other => panic!("ожидалась ошибка InvalidParamValue для limit=0: {other:?}"),
    }

    // Валидация границы: > 50 недопустим для pins
    match build_channel_pins_path(oracle_channel, Some(51)) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
        other => panic!("ожидалась ошибка InvalidParamValue для limit=51: {other:?}"),
    }

    // Ошибка в пути (пустой channel_id)
    match build_channel_pins_path("", None) {
        Err(QueryError::Path(PathRenderError::InvalidPart(_))) => {}
        other => panic!("ожидалась ошибка PathRenderError для пустого канала: {other:?}"),
    }
}

#[test]
fn thread_search_path_matches_oracle_and_validates() {
    // Эталон из rest-static.json THREAD_SEARCH.cacheEvidence:
    // https://discord.com/api/v9/channels/1058348665342808104/threads/search?archived=true&sort_by=last_message_time&sort_order=desc&limit=25&tag_setting=match_some&offset=0
    let oracle_channel = "1058348665342808104";

    let params = ThreadSearchParams::new(true);
    let path = build_thread_search_path(oracle_channel, &params).expect("thread search path");
    assert_eq!(
        path,
        "/channels/1058348665342808104/threads/search?archived=true&sort_by=last_message_time&sort_order=desc&limit=25&tag_setting=match_some&offset=0"
    );

    // DoD: threads без archived не уходит (ошибка до сети)
    let missing_archived = ThreadSearchParams::empty();
    assert_eq!(
        build_thread_search_path(oracle_channel, &missing_archived),
        Err(QueryError::MissingRequiredParam("archived"))
    );

    // Валидация limit
    let mut invalid_limit = ThreadSearchParams::new(false);
    invalid_limit.limit = Some(0);
    match build_thread_search_path(oracle_channel, &invalid_limit) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
        other => panic!("ожидалась ошибка InvalidParamValue: {other:?}"),
    }

    invalid_limit.limit = Some(101);
    match build_thread_search_path(oracle_channel, &invalid_limit) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
        other => panic!("ожидалась ошибка InvalidParamValue: {other:?}"),
    }
}

#[test]
fn channel_messages_path_matches_oracle_and_validates() {
    // Эталон из rest-static.json MESSAGES.cacheEvidence:
    // https://discord.com/api/v9/channels/1056563701764857896/messages?limit=12
    let oracle_channel = "1056563701764857896";

    let params = ChannelMessagesParams {
        limit: Some(12),
        ..Default::default()
    };
    let path = build_channel_messages_path(oracle_channel, &params).expect("messages path");
    assert_eq!(path, "/channels/1056563701764857896/messages?limit=12");

    // Границы limit: 1 и 100 допустимы
    let params_min = ChannelMessagesParams {
        limit: Some(1),
        ..Default::default()
    };
    assert_eq!(
        build_channel_messages_path(oracle_channel, &params_min).unwrap(),
        "/channels/1056563701764857896/messages?limit=1"
    );

    let params_max = ChannelMessagesParams {
        limit: Some(100),
        ..Default::default()
    };
    assert_eq!(
        build_channel_messages_path(oracle_channel, &params_max).unwrap(),
        "/channels/1056563701764857896/messages?limit=100"
    );

    // Границы limit: 0 и 101 недопустимы
    let params_zero = ChannelMessagesParams {
        limit: Some(0),
        ..Default::default()
    };
    match build_channel_messages_path(oracle_channel, &params_zero) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
        other => panic!("ожидалась ошибка limit=0: {other:?}"),
    }

    let params_overflow = ChannelMessagesParams {
        limit: Some(101),
        ..Default::default()
    };
    match build_channel_messages_path(oracle_channel, &params_overflow) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
        other => panic!("ожидалась ошибка limit=101: {other:?}"),
    }

    // Курсоры before / after / around
    let params_before = ChannelMessagesParams {
        limit: Some(50),
        before: Some("1056563701764857800"),
        ..Default::default()
    };
    assert_eq!(
        build_channel_messages_path(oracle_channel, &params_before).unwrap(),
        "/channels/1056563701764857896/messages?limit=50&before=1056563701764857800"
    );

    // Взаимная исключаемость курсоров
    let conflict1 = ChannelMessagesParams {
        limit: Some(50),
        before: Some("1056563701764857800"),
        after: Some("1056563701764857900"),
        ..Default::default()
    };
    assert_eq!(
        build_channel_messages_path(oracle_channel, &conflict1),
        Err(QueryError::MutuallyExclusiveParams {
            param1: "before",
            param2: "after",
        })
    );

    let conflict2 = ChannelMessagesParams {
        limit: Some(50),
        before: Some("1056563701764857800"),
        around: Some("1056563701764857900"),
        ..Default::default()
    };
    assert_eq!(
        build_channel_messages_path(oracle_channel, &conflict2),
        Err(QueryError::MutuallyExclusiveParams {
            param1: "before",
            param2: "around",
        })
    );

    let conflict3 = ChannelMessagesParams {
        limit: Some(50),
        after: Some("1056563701764857800"),
        around: Some("1056563701764857900"),
        ..Default::default()
    };
    assert_eq!(
        build_channel_messages_path(oracle_channel, &conflict3),
        Err(QueryError::MutuallyExclusiveParams {
            param1: "after",
            param2: "around",
        })
    );

    // Невалидный нечисловой/отрицательный курсор
    let invalid_cursor = ChannelMessagesParams {
        limit: Some(50),
        before: Some("-1"),
        ..Default::default()
    };
    match build_channel_messages_path(oracle_channel, &invalid_cursor) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "before"),
        other => panic!("ожидалась ошибка для before=-1: {other:?}"),
    }

    let alpha_cursor = ChannelMessagesParams {
        limit: Some(50),
        after: Some("xyz"),
        ..Default::default()
    };
    match build_channel_messages_path(oracle_channel, &alpha_cursor) {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "after"),
        other => panic!("ожидалась ошибка для after=xyz: {other:?}"),
    }
}

#[test]
fn messages_search_path_matches_live_capture() {
    // Живой URL из capture-run2.jsonl:
    // https://discord.com/api/v9/guilds/1014537851418980372/messages/search?content=%23%F0%9F%92%A5%E2%94%83%D0%BF%D1%80%D0%B0%D0%B2%D0%B8%D0%BB%D0%B0%D1%82%D0%B5%D1%81%D1%82&sort_by=timestamp&sort_order=desc&search_session_id=d6183157-1379-4323-9321-2c4743e835a5&search_query_id=cfb6f5e3-11e3-44be-91b2-8360807ed063&offset=0
    let guild_id = "1014537851418980372";
    let search_params = MessageSearchParams {
        content: Some("#💥┃правилатест"),
        sort_by: Some("timestamp"),
        sort_order: Some("desc"),
        search_session_id: Some("d6183157-1379-4323-9321-2c4743e835a5"),
        search_query_id: Some("cfb6f5e3-11e3-44be-91b2-8360807ed063"),
        offset: Some(0),
        ..Default::default()
    };

    let path = build_guild_search_path(guild_id, &search_params).expect("guild search path");
    assert_eq!(
        path,
        "/guilds/1014537851418980372/messages/search?content=%23%F0%9F%92%A5%E2%94%83%D0%BF%D1%80%D0%B0%D0%B2%D0%B8%D0%BB%D0%B0%D1%82%D0%B5%D1%81%D1%82&sort_by=timestamp&sort_order=desc&search_session_id=d6183157-1379-4323-9321-2c4743e835a5&search_query_id=cfb6f5e3-11e3-44be-91b2-8360807ed063&offset=0"
    );

    // Поиск по каналу
    let channel_search_params = MessageSearchParams {
        content: Some("test query"),
        offset: Some(25),
        ..Default::default()
    };
    let channel_path = build_channel_search_path("1056563701764857896", &channel_search_params)
        .expect("channel search path");
    assert_eq!(
        channel_path,
        "/channels/1056563701764857896/messages/search?content=test%20query&offset=25"
    );
}

#[test]
fn catalog_counts_separate_d02_and_static_extensions() {
    // BE-P3-02: 47 строк D-02 + 2 `[S]`/raw-расширения внутри ROUTES = 49.
    assert_eq!(CAPTURED_D02_ROUTE_COUNT, 47, "маршруты захвата D-02");
    assert_eq!(STATIC_EXTENSION_ROUTE_COUNT, 2, "расширения внутри ROUTES");
    assert_eq!(
        ROUTES.len(),
        CAPTURED_D02_ROUTE_COUNT + STATIC_EXTENSION_ROUTE_COUNT,
        "длина ROUTES"
    );

    let extensions: Vec<&RouteSpec> = ROUTES
        .iter()
        .filter(|spec| spec.is_static_extension())
        .collect();
    assert_eq!(
        extensions.len(),
        STATIC_EXTENSION_ROUTE_COUNT,
        "расширения в ROUTES: {extensions:?}"
    );
    let extension_keys: HashSet<(&str, &str)> = extensions
        .iter()
        .map(|spec| (spec.method.as_str(), spec.path))
        .collect();
    let expected: HashSet<(&str, &str)> = [
        ("POST", "/channels/{id}/messages"),
        ("GET", "/guilds/{id}/messages/search"),
    ]
    .into_iter()
    .collect();
    assert_eq!(extension_keys, expected, "состав расширений в ROUTES");

    // Остальные 47 строк ROUTES — захват D-02, расширениями не считаются.
    assert_eq!(
        ROUTES
            .iter()
            .filter(|spec| !spec.is_static_extension())
            .count(),
        CAPTURED_D02_ROUTE_COUNT,
        "маршруты D-02 в ROUTES"
    );
    assert!(
        !find(HttpMethod::Get, "/channels/{id}/messages")
            .expect("маршрут истории сообщений")
            .is_static_extension(),
        "GET /channels/{{id}}/messages — строка D-02, не расширение"
    );

    // EXTENSIONS тоже помечены расширениями: typing, invite GET/POST, поиск по каналу.
    for (method, path) in [
        (HttpMethod::Post, TYPING_PATH),
        (HttpMethod::Get, INVITE_PATH),
        (HttpMethod::Post, INVITE_PATH),
        (HttpMethod::Get, CHANNEL_SEARCH_PATH),
    ] {
        let spec = find(method, path)
            .unwrap_or_else(|| panic!("в каталоге нет расширения {method:?} {path}"));
        assert!(
            spec.is_static_extension(),
            "{method:?} {path} должен быть помечен как расширение"
        );
    }
}

#[test]
fn rest_d02_copy_matches_lab_oracle() {
    // BE-P3-09: docs/re/rest-d02.json — санитизированная копия лабораторного оракула.
    // Единственное разрешённое расхождение (docs/re/README.md): пример тела settings-proto
    // заменён на REDACTED; сырое тело остаётся только в /home/sanya/re-deep/.
    const DOC_MD5: &str = "87d7a05d8cd11f3b8ae37aa5c9811c9e";
    const LAB_PATH: &str = "/home/sanya/re-deep/rest-d02.json";
    const LAB_MD5: &str = "9b450c34a2277b04989830b53b02f012";

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let doc_path = repo_root.join("docs/re/rest-d02.json");
    let doc_bytes = fs::read(&doc_path)
        .unwrap_or_else(|error| panic!("не прочитан {}: {error}", doc_path.display()));
    assert_eq!(
        md5_hex(&doc_bytes),
        DOC_MD5,
        "md5 копии репозитория {}",
        doc_path.display()
    );

    let doc_text = String::from_utf8(doc_bytes.clone()).expect("оракул репозитория в UTF-8");
    let doc_pairs = oracle_method_path_pairs(&doc_text);
    assert_eq!(
        doc_pairs.len(),
        CAPTURED_D02_ROUTE_COUNT,
        "endpoints в docs/re/rest-d02.json"
    );

    let lab_path = Path::new(LAB_PATH);
    if !lab_path.exists() {
        // Лаборатория недоступна на хосте: проверена копия репозитория (47 endpoints выше).
        return;
    }

    let lab_bytes = fs::read(lab_path)
        .unwrap_or_else(|error| panic!("не прочитан {}: {error}", lab_path.display()));
    assert_eq!(md5_hex(&lab_bytes), LAB_MD5, "md5 {}", LAB_PATH);

    let lab_text = String::from_utf8(lab_bytes.clone()).expect("оракул лаборатории в UTF-8");
    assert_eq!(
        oracle_method_path_pairs(&lab_text),
        doc_pairs,
        "пары (method, path) расходятся с лабораторией"
    );

    if doc_bytes == lab_bytes {
        return;
    }

    let doc_lines: Vec<&str> = doc_text.lines().collect();
    let lab_lines: Vec<&str> = lab_text.lines().collect();
    assert_eq!(
        doc_lines.len(),
        lab_lines.len(),
        "разное число строк оракула"
    );
    let divergent: Vec<usize> = doc_lines
        .iter()
        .zip(&lab_lines)
        .enumerate()
        .filter(|(_, (doc, lab))| doc != lab)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        divergent.len(),
        1,
        "расхождений больше одного: {:?}",
        &divergent[..divergent.len().min(5)]
    );
    let index = divergent[0];
    let doc_line = doc_lines[index];
    let lab_line = lab_lines[index];
    assert!(
        doc_line.contains("\"reqBodySample\"") && lab_line.contains("\"reqBodySample\""),
        "расхождение не в reqBodySample (строка {}): {:?}",
        index + 1,
        doc_line
    );
    assert!(
        doc_line.contains("REDACTED"),
        "в копии репозитория нет метки REDACTED: {doc_line:?}"
    );
    assert!(
        !lab_line.contains("REDACTED"),
        "в лабораторном оракуле неожиданно REDACTED: {lab_line:?}"
    );
}

/// Пары `(method, path)` из массива `endpoints` оракула (ключи на отступе 6 пробелов).
fn oracle_method_path_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut method: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("      \"method\": \"") {
            method = Some(trim_json_value(rest));
        } else if let Some(rest) = line.strip_prefix("      \"path\": \"") {
            let path = trim_json_value(rest);
            let method = method
                .take()
                .unwrap_or_else(|| panic!("path без предшествующего method: {path}"));
            pairs.push((method, path));
        }
    }
    pairs
}

/// Срезает закрывающую кавычку и запятую JSON-строки.
fn trim_json_value(raw: &str) -> String {
    raw.trim_end_matches(['"', ',']).to_string()
}

/// Константы раундов MD5: `K[i] = floor(2^32 * abs(sin(i + 1)))`.
const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// Сдвиги раундов MD5 (RFC 1321, §3.4).
const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// MD5-дайджест в нижнем hex (RFC 1321) без внешних крейтов: сверка md5 оракула в тесте.
fn md5_hex(input: &[u8]) -> String {
    use std::fmt::Write as _;

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    let mut state = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    for chunk in padded.as_chunks::<64>().0 {
        let mut words = [0u32; 16];
        for (word, bytes) in words.iter_mut().zip(chunk.as_chunks::<4>().0) {
            *word = u32::from_le_bytes(*bytes);
        }
        let [mut a, mut b, mut c, mut d] = state;
        for round in 0..64 {
            let (f, g) = match round {
                0..=15 => ((b & c) | (!b & d), round),
                16..=31 => ((d & b) | (!d & c), (5 * round + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * round + 5) % 16),
                _ => (c ^ (b | !d), (7 * round) % 16),
            };
            let rotated = a
                .wrapping_add(f)
                .wrapping_add(MD5_K[round])
                .wrapping_add(words[g])
                .rotate_left(MD5_S[round]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(rotated);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }

    let mut hex = String::with_capacity(32);
    for word in state {
        for byte in word.to_le_bytes() {
            write!(hex, "{byte:02x}").expect("запись hex в String");
        }
    }
    hex
}
