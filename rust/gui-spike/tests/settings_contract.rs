//! Контракт сохранения UI-настроек (RW-40 / B8a): `settings_load`/`settings_save`.
//!
//! Хранилище — один локальный JSON-файл, без сети, окна и секретов. Оракулы
//! внешние, а не текущая реализация:
//! - значения по умолчанию — отгруженный UI: `ui/index.html` объявляет
//!   `<html lang="ru" data-density="cozy">` и `color-scheme: dark`;
//! - допустимые плотности/темы — те же строки, что рисует
//!   `ui/features/settings.js` (`["compact", ...]`, `["cozy", ...]`) и что
//!   ядро подтверждает из живого дампа (`discord-core/settings/preloaded.rs`:
//!   `Theme` 1 DARK / 2 LIGHT, `UIDensity` 1 COMPACT / 2 COZY);
//! - лимит файла и атомарность (tmp+rename) — требование карточки B8a.
//!
//! Тесты не запускают Tauri-окно, не ходят в сеть и работают только во
//! временных каталогах: `ui/**` и `fixtures/**` не изменяются.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use discord_gui_spike::settings_store::{
    self, Density, Locale, SettingsError, SettingsStore, Theme, UiSettings, MAX_SETTINGS_BYTES,
    MAX_TOGGLES, MAX_TOGGLE_ID_LEN, SETTINGS_VERSION,
};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Временный каталог: уникальный на тест, убирается на выходе
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("dl-b8a-{}-{name}-{unique}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("временный каталог создаётся");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Имена файлов в каталоге — для проверки, что временных `.tmp` не осталось.
    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .expect("каталог читается")
            .map(|entry| {
                entry
                    .expect("элемент каталога")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Настройки «как из UI»: не-дефолтные плотность/тема/язык должны дожить до диска.
fn sample() -> UiSettings {
    let mut toggles = BTreeMap::new();
    toggles.insert("qos_priority".to_string(), false);
    toggles.insert("voice.echo_cancel".to_string(), true);
    UiSettings {
        version: SETTINGS_VERSION,
        toggles,
        density: Density::Compact,
        theme: Theme::Light,
        locale: Locale::En,
        last_guild_id: None,
    }
}

fn write_raw(path: &Path, raw: &str) {
    fs::write(path, raw).expect("файл настроек пишется");
}

fn settings_json() -> Value {
    json!({
        "version": SETTINGS_VERSION,
        "toggles": {},
        "density": "cozy",
        "theme": "dark",
        "locale": "ru",
    })
}

/// Поле из `SettingsError::Invalid`; любой другой вариант — провал теста.
fn invalid_field(error: SettingsError) -> String {
    match error {
        SettingsError::Invalid { field, .. } => field,
        other => panic!("ожидалась Invalid, получено {other:?}"),
    }
}

fn assert_parse(error: SettingsError, path: &Path) {
    match &error {
        SettingsError::Parse { path: actual, .. } => assert_eq!(actual, path),
        other => panic!("ожидался Parse, получено {other:?}"),
    }
    assert!(
        error.to_string().contains(&path.display().to_string()),
        "текст Parse-ошибки должен называть путь: {error}"
    );
}

// ---------------------------------------------------------------------------
// Значения по умолчанию и канонический JSON
// ---------------------------------------------------------------------------

#[test]
fn defaults_follow_shipped_ui_contract() {
    let defaults = UiSettings::default();
    assert_eq!(defaults.version, SETTINGS_VERSION);
    assert_eq!(defaults.density, Density::Cozy);
    assert_eq!(defaults.theme, Theme::Dark);
    assert_eq!(defaults.locale, Locale::Ru);
    assert!(
        defaults.toggles.is_empty(),
        "пустая карта = состояния из UI (38 тумблеров, SET-06/07/08), backend их не выдумывает"
    );

    // Независимый оракул — отгруженная разметка.
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let html = fs::read_to_string(ui.join("index.html")).expect("ui/index.html читается");
    assert!(
        html.contains(r#"<html lang="ru" data-density="cozy">"#),
        "дефолт локали/плотности должен совпадать с ui/index.html"
    );
    assert!(
        html.contains(r#"<meta name="color-scheme" content="dark" />"#),
        "дефолт темы должен совпадать с ui/index.html"
    );

    // Второй оракул — строки плотности из UI-фичи настроек (S-14).
    let settings_js =
        fs::read_to_string(ui.join("features/settings.js")).expect("ui/features/settings.js");
    for value in Density::ALLOWED {
        assert!(
            settings_js.contains(&format!("\"{value}\"")),
            "плотность {value} должна быть тем же словом, что в UI"
        );
    }
    assert_eq!(Density::ALLOWED, ["cozy", "compact"]);
    assert_eq!(Theme::ALLOWED, ["dark", "light"]);
    assert_eq!(Locale::ALLOWED, ["ru", "en"]);
}

#[test]
fn serialized_defaults_are_canonical_and_hold_no_secrets() {
    let value = serde_json::to_value(UiSettings::default()).expect("UiSettings сериализуется");
    assert_eq!(
        value,
        json!({
            "version": SETTINGS_VERSION,
            "toggles": {},
            "density": "cozy",
            "theme": "dark",
            "locale": "ru",
        })
    );
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("объект настроек")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["density", "locale", "theme", "toggles", "version"],
        "в файле только UI-настройки: ни токенов, ни ключей keyring"
    );

    // Любое значение из белого списка десериализуется без потерь.
    for density in Density::ALLOWED {
        for theme in Theme::ALLOWED {
            for locale in Locale::ALLOWED {
                let candidate = json!({
                    "version": SETTINGS_VERSION,
                    "toggles": {"qos_priority": false},
                    "density": density,
                    "theme": theme,
                    "locale": locale,
                });
                let parsed: UiSettings =
                    serde_json::from_value(candidate).expect("значение из белого списка");
                assert_eq!(
                    serde_json::to_value(&parsed).expect("обратная сериализация"),
                    json!({
                        "version": SETTINGS_VERSION,
                        "toggles": {"qos_priority": false},
                        "density": density,
                        "theme": theme,
                        "locale": locale,
                    })
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// load: нет файла → default; битый файл → Parse
// ---------------------------------------------------------------------------

#[test]
fn missing_file_yields_defaults_without_creating_anything() {
    let dir = TempDir::new("missing");
    let path = dir.join("nested/settings.json");
    let store = SettingsStore::new(&path);

    assert_eq!(
        store.load().expect("нет файла — это не ошибка"),
        UiSettings::default()
    );
    assert_eq!(store.path(), path.as_path());
    assert!(!path.exists(), "load не должен создавать файл");
    assert!(
        !dir.join("nested").exists(),
        "load не должен создавать каталоги"
    );
}

#[test]
fn corrupt_json_is_typed_parse_error() {
    let dir = TempDir::new("corrupt");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    for raw in [
        "",
        "{",
        "{\"version\":1,",
        "[]",
        "null",
        "{\"version\":\"один\",\"toggles\":{},\"density\":\"cozy\",\"theme\":\"dark\",\"locale\":\"ru\"}",
        "{\"version\":1,\"toggles\":[],\"density\":\"cozy\",\"theme\":\"dark\",\"locale\":\"ru\"}",
        "{\"version\":1,\"toggles\":{},\"theme\":\"dark\",\"locale\":\"ru\"}",
    ] {
        write_raw(&path, raw);
        let error = store.load().expect_err("битый файл — ошибка, не паника");
        assert_parse(error, &path);
    }

    // Не UTF-8 — тоже Parse, без паники и с путём.
    fs::write(&path, [0xff, 0xfe, 0x00]).expect("пишем не-UTF-8");
    assert_parse(store.load().expect_err("не-UTF-8 — ошибка"), &path);
}

#[test]
fn unknown_fields_are_rejected_and_values_not_echoed() {
    let dir = TempDir::new("unknown");
    let path = dir.join("settings.json");
    let secret = "MTIzNDU2Nzg5MDEyMzQ1Njc4.GaBcDe.СЕКРЕТ";
    write_raw(
        &path,
        &format!(
            r#"{{"version":1,"toggles":{{}},"density":"cozy","theme":"dark","locale":"ru","token":"{secret}"}}"#
        ),
    );

    let error = SettingsStore::new(&path)
        .load()
        .expect_err("посторонние поля (токен) в настройки не попадают");
    assert_parse(error.clone(), &path);
    let text = format!("{error} {error:?}");
    assert!(
        !text.contains(secret) && !text.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4"),
        "ошибка не должна эхо-печатать значение чужого поля: {text}"
    );
}

#[test]
fn invalid_density_theme_and_locale_are_typed_invalid() {
    let dir = TempDir::new("invalid-values");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    let cases: [(&str, &str, [&str; 2]); 3] = [
        ("density", "wide", ["cozy", "compact"]),
        ("theme", "midnight", ["dark", "light"]),
        ("locale", "de", ["ru", "en"]),
    ];
    for (field, raw_value, allowed) in cases {
        let mut value = settings_json();
        value[field] = json!(raw_value);
        write_raw(&path, &value.to_string());

        let error = store
            .load()
            .expect_err("значение вне белого списка — ошибка");
        assert_eq!(invalid_field(error.clone()), field);
        assert_eq!(error.kind(), "Invalid");
        let text = error.to_string();
        for allowed_value in allowed {
            assert!(
                text.contains(allowed_value),
                "текст ошибки должен перечислять допустимое {allowed_value}: {text}"
            );
        }
    }

    // Значение не строкой — это структура, а не значение: Parse.
    let mut value = settings_json();
    value["density"] = json!(2);
    write_raw(&path, &value.to_string());
    assert_parse(store.load().expect_err("density не строка"), &path);
}

#[test]
fn incompatible_version_is_typed_invalid() {
    let dir = TempDir::new("version");
    let path = dir.join("settings.json");
    let mut value = settings_json();
    value["version"] = json!(SETTINGS_VERSION + 1);
    write_raw(&path, &value.to_string());

    let error = SettingsStore::new(&path)
        .load()
        .expect_err("чужая версия схемы не читается молча");
    assert_eq!(invalid_field(error.clone()), "version");
    assert!(
        error.to_string().contains(&SETTINGS_VERSION.to_string()),
        "ошибка называет ожидаемую версию: {error}"
    );
}

#[test]
fn oversize_file_is_rejected_before_parsing() {
    let dir = TempDir::new("oversize-load");
    let path = dir.join("settings.json");
    fs::write(&path, vec![b'x'; MAX_SETTINGS_BYTES + 1]).expect("большой файл");

    let error = SettingsStore::new(&path)
        .load()
        .expect_err("файл больше лимита не читается в память");
    assert_eq!(invalid_field(error), "size");
    assert!(path.exists(), "load ничего не удаляет");
}

// ---------------------------------------------------------------------------
// save: roundtrip, атомарность, каталоги, лимиты
// ---------------------------------------------------------------------------

#[test]
fn save_then_load_roundtrip_is_lossless() {
    let dir = TempDir::new("roundtrip");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    let settings = sample();
    store.save(&settings).expect("сохранение");
    assert_eq!(store.load().expect("чтение после записи"), settings);
    assert_eq!(dir.entries(), ["settings.json"]);

    let raw = fs::read_to_string(&path).expect("файл читается");
    let value: Value = serde_json::from_str(&raw).expect("на диске валидный JSON");
    assert_eq!(value["density"], "compact");
    assert_eq!(value["theme"], "light");
    assert_eq!(value["locale"], "en");
    assert_eq!(value["version"], SETTINGS_VERSION);
    assert_eq!(value["toggles"]["qos_priority"], false);
    assert_eq!(value["toggles"]["voice.echo_cancel"], true);

    // Перезапись другим значением — тоже атомарно и без хвостов.
    store
        .save(&UiSettings::default())
        .expect("перезапись дефолтами");
    assert_eq!(
        store.load().expect("после перезаписи"),
        UiSettings::default()
    );
    assert_eq!(dir.entries(), ["settings.json"]);
}

#[test]
fn atomic_save_leaves_no_temp_files() {
    let dir = TempDir::new("atomic");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    for _ in 0..3 {
        store.save(&sample()).expect("сохранение");
        let names = dir.entries();
        assert_eq!(
            names,
            ["settings.json"],
            "временный файл удалён/переименован"
        );
        assert!(
            names.iter().all(|name| !name.contains(".tmp")),
            "в каталоге не должно остаться tmp: {names:?}"
        );
    }
}

#[test]
fn save_creates_missing_parent_directories() {
    let dir = TempDir::new("mkdir");
    let path = dir.join(".local/settings.json");
    let store = SettingsStore::new(&path);

    store.save(&sample()).expect("каталог создаётся по пути");
    assert!(dir.join(".local").is_dir());
    assert_eq!(store.load().expect("файл на месте"), sample());
}

#[test]
fn save_rejects_more_than_size_limit_without_touching_disk() {
    let dir = TempDir::new("oversize-save");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    let mut toggles = BTreeMap::new();
    let tail = "x".repeat(180);
    for index in 0..1500 {
        toggles.insert(format!("toggle_{index:04}_{tail}"), true);
    }
    assert!(
        toggles.len() <= MAX_TOGGLES,
        "сценарий не должен упираться в счётчик"
    );
    let settings = UiSettings {
        toggles,
        ..UiSettings::default()
    };
    let encoded = serde_json::to_vec(&settings).expect("сериализация");
    assert!(
        encoded.len() > MAX_SETTINGS_BYTES,
        "сценарий должен превышать лимит: {} > {MAX_SETTINGS_BYTES}",
        encoded.len()
    );

    let error = store
        .save(&settings)
        .expect_err("превышение лимита — ошибка");
    assert_eq!(invalid_field(error), "size");
    assert!(!path.exists(), "при отказе файл не создаётся");
}

#[test]
fn save_rejects_bad_toggles_and_incompatible_version() {
    let dir = TempDir::new("toggles");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    let with_key = |key: String| {
        let mut toggles = BTreeMap::new();
        toggles.insert(key, true);
        UiSettings {
            toggles,
            ..UiSettings::default()
        }
    };

    let bad_keys = [
        ("пустой id", String::new()),
        ("пробельный id", "   ".to_string()),
        ("управляющий символ", "bad\nid".to_string()),
        ("длинный id", "x".repeat(MAX_TOGGLE_ID_LEN + 1)),
    ];
    for (label, key) in bad_keys {
        let error = store.save(&with_key(key)).expect_err(label);
        assert_eq!(invalid_field(error), "toggles", "{label}");
    }

    let mut toggles = BTreeMap::new();
    for index in 0..=MAX_TOGGLES {
        toggles.insert(format!("t{index}"), true);
    }
    let too_many = UiSettings {
        toggles,
        ..UiSettings::default()
    };
    assert_eq!(
        invalid_field(store.save(&too_many).expect_err("слишком много тумблеров")),
        "toggles"
    );

    let future = UiSettings {
        version: SETTINGS_VERSION + 1,
        ..UiSettings::default()
    };
    assert_eq!(
        invalid_field(store.save(&future).expect_err("чужая версия схемы")),
        "version"
    );

    assert!(
        !path.exists(),
        "ни один отказ валидации не создаёт файл настроек"
    );
}

// ---------------------------------------------------------------------------
// G-01b: `lastGuildId` — аддитивное поле восстановления выбранного сервера
// ---------------------------------------------------------------------------

/// Действительный snowflake гильдии (19 цифр, влезает в u64).
const GUILD_ID: &str = "1234567890123456789";

#[test]
fn last_guild_id_defaults_to_none_and_stays_out_of_canonical_file() {
    let defaults = UiSettings::default();
    assert_eq!(
        defaults.last_guild_id, None,
        "источник не выдумывается: без явного выбора — None"
    );

    let value = serde_json::to_value(UiSettings::default()).expect("UiSettings сериализуется");
    assert!(
        value.get("lastGuildId").is_none(),
        "None не плодит ключ: файл без выбора остаётся прежним: {value}"
    );
    assert!(
        value.get("last_guild_id").is_none(),
        "snake_case в файле/IPC не появляется: {value}"
    );
}

#[test]
fn last_guild_id_roundtrips_as_camel_case_ipc_field() {
    let dir = TempDir::new("last-guild");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    let settings = UiSettings {
        last_guild_id: Some(GUILD_ID.to_string()),
        ..sample()
    };
    store
        .save(&settings)
        .expect("сохранение выбранного сервера");

    let raw = fs::read_to_string(&path).expect("файл читается");
    let value: Value = serde_json::from_str(&raw).expect("на диске валидный JSON");
    assert_eq!(value["lastGuildId"], GUILD_ID, "поле IPC — camelCase");
    assert!(
        value.get("last_guild_id").is_none(),
        "snake_case-дубля быть не должно: {value}"
    );
    // Аддитивность: существующие поля доезжают без потерь.
    assert_eq!(value["density"], "compact");
    assert_eq!(value["theme"], "light");
    assert_eq!(value["locale"], "en");
    assert_eq!(value["toggles"]["voice.echo_cancel"], true);
    assert_eq!(store.load().expect("чтение после записи"), settings);
}

#[test]
fn legacy_file_without_last_guild_id_migrates_to_none() {
    let dir = TempDir::new("legacy-last-guild");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    // Файл прежней схемы (v1, ровно 5 полей) — как в `.local/settings.json`.
    write_raw(
        &path,
        &json!({
            "version": SETTINGS_VERSION,
            "toggles": {"voice.qos_priority": false},
            "density": "compact",
            "theme": "light",
            "locale": "en",
        })
        .to_string(),
    );

    let loaded = store
        .load()
        .expect("файл без lastGuildId читается (аддитивная миграция)");
    assert_eq!(loaded.last_guild_id, None, "миграция не выдумывает гильдию");
    assert_eq!(loaded.version, SETTINGS_VERSION);
    assert_eq!(loaded.density, Density::Compact);
    assert_eq!(loaded.theme, Theme::Light);
    assert_eq!(loaded.locale, Locale::En);
    assert_eq!(loaded.toggles.get("voice.qos_priority"), Some(&false));
}

#[test]
fn non_snowflake_last_guild_id_normalizes_to_none() {
    let dir = TempDir::new("bad-last-guild");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    let invalid_values = [
        json!(""),
        json!("   "),
        json!("abc"),
        json!("12 34"),
        json!("-1"),
        json!("+1"),
        json!("1.5"),
        json!("123456789012345678901"),
        json!("١٢٣"),
        json!(1234567890123456789u64),
        json!(null),
        json!([]),
        json!({}),
        json!(true),
    ];
    for candidate in invalid_values {
        let mut raw = settings_json();
        raw["lastGuildId"] = candidate.clone();
        write_raw(&path, &raw.to_string());
        let loaded = store
            .load()
            .unwrap_or_else(|error| panic!("{candidate} не должен ломать загрузку: {error}"));
        assert_eq!(
            loaded.last_guild_id, None,
            "значение {candidate} не snowflake → None, а не ошибка"
        );
    }

    // Пустая строка/мусор в файле: остальные настройки не теряются.
    let mut raw = settings_json();
    raw["lastGuildId"] = json!("guild-1");
    raw["density"] = json!("compact");
    write_raw(&path, &raw.to_string());
    let loaded = store.load().expect("битая подсказка не рушит настройки");
    assert_eq!(loaded.last_guild_id, None);
    assert_eq!(loaded.density, Density::Compact);
}

#[test]
fn save_drops_non_snowflake_last_guild_id_without_failing() {
    let dir = TempDir::new("save-bad-last-guild");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(&path);

    for bad in ["", "   ", "guild-1", "123456789012345678901"] {
        let settings = UiSettings {
            last_guild_id: Some(bad.to_string()),
            ..UiSettings::default()
        };
        store
            .save(&settings)
            .unwrap_or_else(|error| panic!("{bad:?} нормализуется, а не ломает save: {error}"));

        let value: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("файл")).expect("JSON");
        assert!(
            value.get("lastGuildId").is_none(),
            "на диск попадает только snowflake: {bad:?} → {value}"
        );
        assert_eq!(store.load().expect("чтение").last_guild_id, None);
    }

    // Верхняя граница u64 (20 цифр) — ещё snowflake и сохраняется.
    let max = u64::MAX.to_string();
    let settings = UiSettings {
        last_guild_id: Some(max.clone()),
        ..UiSettings::default()
    };
    store.save(&settings).expect("20 цифр влезают в u64");
    assert_eq!(store.load().expect("чтение").last_guild_id, Some(max));
}

#[test]
fn validate_rejects_non_snowflake_last_guild_id() {
    for bad in ["not-a-snowflake", " 123", "123 ", "1e3", "١٢٣"] {
        let settings = UiSettings {
            last_guild_id: Some(bad.to_string()),
            ..UiSettings::default()
        };
        let error = settings
            .validate()
            .expect_err("не snowflake — вне контракта");
        assert_eq!(invalid_field(error), "lastGuildId");
    }

    let ok = UiSettings {
        last_guild_id: Some(GUILD_ID.to_string()),
        ..UiSettings::default()
    };
    ok.validate()
        .expect("настоящий snowflake проходит валидацию");
}

// ---------------------------------------------------------------------------
// Ошибки и путь
// ---------------------------------------------------------------------------

#[test]
fn errors_are_typed_and_serialize_with_kind() {
    let dir = TempDir::new("errors");
    let path = dir.join("settings.json");

    // Io: на пути настроек каталог, а не файл.
    fs::create_dir(&path).expect("каталог вместо файла");
    let io = SettingsStore::new(&path)
        .load()
        .expect_err("каталог вместо файла — Io");
    assert!(matches!(io, SettingsError::Io { .. }), "{io:?}");
    assert_eq!(io.kind(), "Io");
    assert_eq!(
        serde_json::to_value(&io).expect("JSON ошибки")["kind"],
        "Io"
    );

    fs::remove_dir(&path).expect("убираем каталог");

    // Parse: битый JSON, путь в JSON ошибки.
    write_raw(&path, "{");
    let parse = SettingsStore::new(&path).load().expect_err("битый JSON");
    let value = serde_json::to_value(&parse).expect("JSON ошибки");
    assert_eq!(value["kind"], "Parse");
    assert!(value["path"].is_string(), "{value}");

    // Invalid: значение вне белого списка, поле в JSON ошибки.
    let mut bad = settings_json();
    bad["density"] = json!("wide");
    write_raw(&path, &bad.to_string());
    let invalid = SettingsStore::new(&path).load().expect_err("density wide");
    let value = serde_json::to_value(&invalid).expect("JSON ошибки");
    assert_eq!(value["kind"], "Invalid");
    assert_eq!(value["field"], "density");
}

#[test]
fn settings_path_prefers_env_and_defaults_to_repo_local() {
    let configured = "/tmp/opencode/dl-settings.json";
    assert_eq!(
        settings_store::resolve_path(Some(configured)),
        PathBuf::from(configured)
    );
    assert_eq!(
        settings_store::resolve_path(Some("   ")),
        settings_store::default_settings_path(),
        "пустая переменная — дефолт, а не каталог «   »"
    );
    assert_eq!(
        settings_store::resolve_path(None),
        settings_store::default_settings_path()
    );

    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate лежит в rust/gui-spike репозитория");
    assert_eq!(
        settings_store::default_settings_path(),
        repo.join(".local").join("settings.json"),
        "дефолт — от корня репозитория, не от $HOME"
    );
}

#[test]
fn store_from_env_uses_dl_settings_path() {
    let dir = TempDir::new("env");
    let path = dir.join("custom/settings.json");

    std::env::set_var("DL_SETTINGS_PATH", &path);
    let store = SettingsStore::from_env();
    let resolved = store.path().to_path_buf();
    std::env::remove_var("DL_SETTINGS_PATH");

    assert_eq!(resolved, path);
    store.save(&sample()).expect("сохранение по env-пути");
    assert!(path.exists());
    assert_eq!(store.load().expect("чтение по env-пути"), sample());
}

// ---------------------------------------------------------------------------
// Tauri wiring: регистрация команд и отсутствие токенов в обёртках
// ---------------------------------------------------------------------------

/// Тело функции верхнего уровня `fn <name>(...) { ... }` из исходника.
fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("fn {name}");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("в main.rs нет {marker}"));
    let rest = &source[start..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn tauri_commands_are_registered_and_carry_no_tokens() {
    let source = include_str!("../src/main.rs");
    let open = "generate_handler![";
    let start = source
        .find(open)
        .expect("main.rs должен объявлять invoke_handler!")
        + open.len();
    let block = &source[start..];
    let block = &block[..block.find(']').expect("invoke_handler! без закрывающей ]")];

    let lists = |command: &str| {
        block
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|name| name == command)
    };

    for command in ["settings_load", "settings_save"] {
        assert!(
            lists(command),
            "{command} не зарегистрирован в invoke_handler!"
        );
        let body = function_body(source, command);
        assert!(
            !body.contains("AuthBackend") && !body.contains("token"),
            "{command} не должен трогать токены/keyring: {body}"
        );
    }
}
