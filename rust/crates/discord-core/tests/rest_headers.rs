//! REST v9: auth-заголовки и классы кодов ответа (волна RW-08, P1-07 / T-REST-02).
//!
//! **Провенанс оракулов** (независимые артефакты захвата, не подгонка под реализацию):
//! - `docs/re/rest-d02.json` — md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`;
//!   `docs/re/rest-d02.md` — md5 `c7e61929677efab6bdac86a05a778fce`.
//! - Выдержки в репозитории (read-only): `tests/data/rest-auth-6.tsv` — 6 строк, md5
//!   `144c34c24ecd4645540be518e69fbe3b`; `tests/data/rest-routes-47.tsv` — 47 строк, md5
//!   `2db6b39bcba96bc3a2d480fea9461a33`.
//! - Полная jq-команда выдержки auth (повторяет файл побайтово; md5 выше):
//!   ```text
//!   jq -r '"Authorization\t\(.auth.withAuthorization)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\nX-Super-Properties\t\(.auth.withSuperProperties)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\nX-Discord-Locale\t\(.auth.withLocale)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\nX-Discord-Timezone\t\(.auth.withTimezone)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\nX-Debug-Options\t\(.auth.withDebugOptions)\t\(.totals.apiRequestsWithFullAuthHeaderSet)\nAccept-Language\t-\t\(.totals.apiRequestsWithFullAuthHeaderSet)"' docs/re/rest-d02.json
//!   ```
//! - Выдержка маршрутов: `jq -r '.endpoints[] | [.method, .path, (.reqBodyKeys|join(",")),
//!   (.statuses|map(tostring)|join("|"))] | @tsv' docs/re/rest-d02.json`.
//! - Контрольные числа JSON: `.totals.apiRequests = 133`, `.totals.apiUniqueEndpoints = 47`,
//!   `.totals.apiRequestsWithFullAuthHeaderSet = 133`; `.auth.withAuthorization = 221`
//!   (218 `Authorization` + 3 `authorization` — регистр не влияет на контракт); union
//!   статусов — `jq -r '[.endpoints[].statuses[]] | unique | @json'` →
//!   `[200,204,304,404,429]`.
//! - H4: в выдержке пути относительные и начинаются с `/api/v9` (формат §3.2), поэтому
//!   «путь начинается с базы» проверяется для полного URL `https://discord.com` + путь.
//!
//! Тест-данные обязательны: файла нет — падение, а не skip (выдержки лежат в репозитории).

use discord_core::API_V9_BASE;
use discord_core::rest::headers::{
    AUTH_HEADER_NAMES, AuthHeaderValues, ResponseClass, auth_headers, classify_status,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const AUTH_TSV: &str = "rest-auth-6.tsv";
const ROUTES_TSV: &str = "rest-routes-47.tsv";

/// Оракул §3.1: 6 имён полного auth-набора API-запросов, порядок — как в выдержке.
const OBSERVED_AUTH_HEADERS: [&str; 6] = [
    "Authorization",
    "X-Super-Properties",
    "X-Discord-Locale",
    "X-Discord-Timezone",
    "X-Debug-Options",
    "Accept-Language",
];

/// Различимые значения для проверки пар (ни одно не пустое и не повторяется).
const AUTH_VALUES: [&str; 6] = ["tok-1", "sp-2", "loc-3", "tz-4", "dbg-5", "al-6"];

/// Строки TSV из `tests/data` (пустые строки отброшены, колонки — по `\t`).
fn data_rows(name: &str) -> Vec<Vec<String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("нет тест-данных {}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_owned).collect())
        .collect()
}

/// H1: имена и порядок шести auth-заголовков совпадают с литералом §3.1 и колонкой 1
/// выдержки `rest-auth-6.tsv`.
#[test]
fn auth_header_names_exact_order() {
    assert_eq!(
        AUTH_HEADER_NAMES, OBSERVED_AUTH_HEADERS,
        "AUTH_HEADER_NAMES — 6 имён в порядке §3.1"
    );

    let rows = data_rows(AUTH_TSV);
    assert_eq!(rows.len(), 6, "строк в {AUTH_TSV}");
    let mut names = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            row.len(),
            3,
            "строка {index}: колонки header,total,apiFullSet"
        );
        names.push(row[0].as_str());
    }
    assert_eq!(
        AUTH_HEADER_NAMES.as_slice(),
        names.as_slice(),
        "колонка 1 {AUTH_TSV} = AUTH_HEADER_NAMES"
    );
}

/// H2: `auth_headers` отдаёт 6 пар в порядке имён; значения — входные, без потерь и подмен.
#[test]
fn auth_headers_pairs_values() {
    let values = AuthHeaderValues {
        authorization: AUTH_VALUES[0],
        super_properties: AUTH_VALUES[1],
        locale: AUTH_VALUES[2],
        timezone: AUTH_VALUES[3],
        debug_options: AUTH_VALUES[4],
        accept_language: AUTH_VALUES[5],
    };

    let pairs = auth_headers(&values);
    assert_eq!(pairs.len(), AUTH_HEADER_NAMES.len(), "пар ровно 6");
    for (index, (name, value)) in pairs.iter().enumerate() {
        assert_eq!(*name, AUTH_HEADER_NAMES[index], "имя пары #{index}");
        assert_eq!(*value, AUTH_VALUES[index], "значение пары #{index}");
    }
}

/// H3: контроль выдержки auth — 6 строк, полный набор во всех 133 API-запросах, `total` 221
/// у пяти заголовков и `-` у `Accept-Language` (счётчика в JSON нет, §3.1).
#[test]
fn auth_counts_match_capture() {
    let rows = data_rows(AUTH_TSV);
    assert_eq!(rows.len(), 6, "строк в {AUTH_TSV}");
    assert_eq!(
        rows.len(),
        AUTH_HEADER_NAMES.len(),
        "число строк = числу имён заголовков"
    );

    let mut totals = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            row.len(),
            3,
            "строка {index}: колонки header,total,apiFullSet"
        );
        assert_eq!(
            row[2], "133",
            "{}: полный auth-набор во всех 133 API-запросах",
            row[0]
        );
        totals.push(row[1].as_str());
    }
    assert_eq!(
        totals,
        ["221", "221", "221", "221", "221", "-"],
        "total: 221 у пяти заголовков; Accept-Language — без счётчика"
    );
    assert_eq!(
        totals.iter().filter(|total| **total == "221").count(),
        5,
        "пять заголовков с total = 221 (Authorization: 218 + 3 в другом регистре)"
    );
}

/// H4: база v9 — точная константа; все 47 путей выдержки лежат под этой базой
/// (полный URL `https://discord.com` + относительный путь начинается с `API_V9_BASE`).
#[test]
fn base_v9_prefix() {
    assert_eq!(API_V9_BASE, "https://discord.com/api/v9", "база REST v9");
    const ORIGIN: &str = "https://discord.com";
    const BASE_PATH: &str = "/api/v9";

    let rows = data_rows(ROUTES_TSV);
    assert_eq!(rows.len(), 47, "строк в {ROUTES_TSV}");
    for (index, row) in rows.iter().enumerate() {
        assert!(row.len() >= 2, "строка {index}: колонки method,path");
        let path = &row[1];
        assert!(
            path.starts_with(BASE_PATH),
            "{} {}: путь должен начинаться с {BASE_PATH}",
            row[0],
            path
        );
        let full_url = format!("{ORIGIN}{path}");
        assert!(
            full_url.starts_with(API_V9_BASE),
            "{full_url} должен начинаться с {API_V9_BASE}"
        );
        let suffix = &full_url[API_V9_BASE.len()..];
        assert!(
            suffix.is_empty() || suffix.starts_with('/'),
            "{full_url}: после базы — граница сегмента пути, а не продолжение префикса"
        );
    }
}

/// H5: классификация наблюдённых кодов и «прочих»; union статусов выдержки ровно
/// `{200,204,304,404,429}`, все строки классифицируются не в `Other`,
/// `GET /users/@me/survey` — единственный 429.
#[test]
fn classify_status_observed_and_other() {
    assert_eq!(classify_status(200), ResponseClass::Ok200, "200");
    assert_eq!(classify_status(204), ResponseClass::NoContent204, "204");
    assert_eq!(classify_status(304), ResponseClass::NotModified304, "304");
    assert_eq!(classify_status(404), ResponseClass::NotFound404, "404");
    assert_eq!(classify_status(429), ResponseClass::RateLimited429, "429");
    for status in [0_u16, 201, 301, 500] {
        assert_eq!(
            classify_status(status),
            ResponseClass::Other(status),
            "ненаблюдённый статус {status} — Other"
        );
    }

    let rows = data_rows(ROUTES_TSV);
    assert_eq!(rows.len(), 47, "строк в {ROUTES_TSV}");
    let mut union = BTreeSet::new();
    let mut survey_statuses = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            row.len(),
            4,
            "строка {index}: колонки method,path,body_keys,statuses"
        );
        let statuses: Vec<u16> = row[3]
            .split('|')
            .filter(|part| !part.is_empty())
            .map(|part| {
                part.parse().unwrap_or_else(|error| {
                    panic!("{} {}: статус {part:?} не число: {error}", row[0], row[1])
                })
            })
            .collect();
        assert!(
            !statuses.is_empty(),
            "{} {}: строка без статусов",
            row[0],
            row[1]
        );
        for status in &statuses {
            union.insert(*status);
            assert!(
                !matches!(classify_status(*status), ResponseClass::Other(_)),
                "{} {}: наблюдённый статус {status} не должен быть Other",
                row[0],
                row[1]
            );
        }
        if row[0] == "GET" && row[1] == "/api/v9/users/@me/survey" {
            survey_statuses = statuses;
        }
    }

    assert_eq!(
        union,
        BTreeSet::from([200, 204, 304, 404, 429]),
        "union статусов 47 маршрутов"
    );
    assert_eq!(
        survey_statuses,
        [429],
        "GET /users/@me/survey — единственный 429 в захвате"
    );
}
