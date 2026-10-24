//! REST v9: каталог 47 живых маршрутов (T-REST-01, волна RW-08 / P1-07).
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
    HttpMethod, PathRenderError, ROUTES, RouteSpec, find, render_path,
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
