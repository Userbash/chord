//! # discord-i18n
//!
//! Локализация: хеширование ключей `runtimeHashMessageKey` (xxh64 → base64),
//! плурализация CLDR, цепочка fallback `запрошенная → en-US → MissingMessage`.
//!
//! Спека: `docs/re/i18n-deep.md` §1–3, §7 (36 кодов, включено 31).
//! План: P3-15 (базовый рантайм) → P7-01 (полные 36 локалей/даты); тесты: T-I18N-*.

pub mod duration;
mod duration_data;
pub mod format;
mod format_data;
pub mod list;
mod list_data;
pub mod locale;
pub mod markdown;
pub mod plural;
pub mod relative;
mod relative_data;
pub mod timestamp;

/// Длина вычисляемого ключа: base64(xxh64) → 6 символов (`[0..6]`).
pub const MESSAGE_KEY_LEN: usize = 6;

pub use locale::{
    DEFAULT_LOCALE, DISABLED_LOCALES, ENABLED_LOCALES, SUPPORTED_LOCALES, TOTAL_LOCALES,
    canonical_locale, fallback_chain, is_enabled, is_supported, locale_language, resolve_locale,
};
pub use markdown::{MarkdownNode, parse_markdown, render_markdown};

/// xxHash64, seedable and implemented without unsafe code.
#[must_use]
pub fn xxh64(input: &[u8], seed: u64) -> u64 {
    const P1: u64 = 11_400_714_785_074_694_791;
    const P2: u64 = 14_029_467_366_897_019_727;
    const P3: u64 = 1_609_587_929_392_839_161;
    const P4: u64 = 9_650_029_242_287_828_579;
    const P5: u64 = 2_870_177_450_012_600_261;
    fn round(acc: u64, lane: u64) -> u64 {
        acc.wrapping_add(lane.wrapping_mul(P2))
            .rotate_left(31)
            .wrapping_mul(P1)
    }
    fn merge(acc: u64, val: u64) -> u64 {
        (acc ^ round(0, val)).wrapping_mul(P1).wrapping_add(P4)
    }
    let mut i = 0;
    let mut h;
    if input.len() >= 32 {
        let (mut v1, mut v2, mut v3, mut v4) = (
            seed.wrapping_add(P1).wrapping_add(P2),
            seed.wrapping_add(P2),
            seed,
            seed.wrapping_sub(P1),
        );
        while i + 32 <= input.len() {
            v1 = round(
                v1,
                u64::from_le_bytes(input[i..i + 8].try_into().expect("8 bytes")),
            );
            v2 = round(
                v2,
                u64::from_le_bytes(input[i + 8..i + 16].try_into().expect("8 bytes")),
            );
            v3 = round(
                v3,
                u64::from_le_bytes(input[i + 16..i + 24].try_into().expect("8 bytes")),
            );
            v4 = round(
                v4,
                u64::from_le_bytes(input[i + 24..i + 32].try_into().expect("8 bytes")),
            );
            i += 32;
        }
        h = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        h = merge(merge(merge(merge(h, v1), v2), v3), v4);
    } else {
        h = seed.wrapping_add(P5);
    }
    h = h.wrapping_add(input.len() as u64);
    while i + 8 <= input.len() {
        let k = round(
            0,
            u64::from_le_bytes(input[i..i + 8].try_into().expect("8 bytes")),
        );
        h = (h ^ k).rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        i += 8;
    }
    if i + 4 <= input.len() {
        h = (h ^ u64::from(u32::from_le_bytes(
            input[i..i + 4].try_into().expect("4 bytes"),
        ))
        .wrapping_mul(P1))
        .rotate_left(23)
        .wrapping_mul(P2)
        .wrapping_add(P3);
        i += 4;
    }
    for b in &input[i..] {
        h = (h ^ u64::from(*b).wrapping_mul(P5))
            .rotate_left(11)
            .wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

/// Hash an internal message id and return the first six base64 characters of its LE digest.
#[must_use]
pub fn message_key(id: &str) -> String {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = xxh64(id.as_bytes(), 0).to_le_bytes();
    let mut out = String::with_capacity(8);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64[(n & 63) as usize] as char);
        }
    }
    out.truncate(MESSAGE_KEY_LEN);
    out
}

use crate::format::DateStyle;
use crate::plural::PluralType;
use std::{collections::BTreeMap, fmt};

/// Parsed message expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Ast {
    Literal(String),
    Argument(String),
    Number(String),
    Date(String, String),
    Time(String, String),
    Select(String, BTreeMap<String, Vec<Ast>>),
    Plural(String, BTreeMap<String, Vec<Ast>>, i64, String),
    Pound,
    Tag(String, Vec<Ast>, Option<String>),
    Sequence(Vec<Ast>),
}

/// Invalid catalog AST. Error text contains only structural location, never source values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstError {
    pub path: String,
    pub kind: &'static str,
}
impl fmt::Display for AstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid catalog AST at {}: {}", self.path, self.kind)
    }
}
impl std::error::Error for AstError {}

/// Parsed key-to-message catalog.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    entries: BTreeMap<String, Ast>,
}
impl Catalog {
    pub fn parse(value: &chord_json::JsonValue) -> Result<Self, AstError> {
        let obj = value.as_object().ok_or_else(|| AstError {
            path: "$".into(),
            kind: "expected object",
        })?;
        let mut entries = BTreeMap::new();
        for (key, node) in obj {
            entries.insert(key.clone(), parse_ast(node, format!("$.{key}"), 0)?);
        }
        Ok(Self { entries })
    }
    pub fn get(&self, key: &str) -> Option<&Ast> {
        self.entries.get(key)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
fn parse_ast(v: &chord_json::JsonValue, path: String, depth: usize) -> Result<Ast, AstError> {
    if let Some(s) = v.as_str() {
        return Ok(Ast::Literal(s.into()));
    }
    if depth > 128 {
        return Err(AstError {
            path,
            kind: "depth limit",
        });
    }
    if let Some(a) = v.as_array() {
        if a.first().and_then(chord_json::JsonValue::as_i64).is_none() {
            let nodes = a
                .iter()
                .enumerate()
                .map(|(i, x)| parse_ast(x, format!("{path}[{i}]"), depth + 1))
                .collect::<Result<_, _>>()?;
            return Ok(Ast::Sequence(nodes));
        }
        let bad = || AstError {
            path: path.clone(),
            kind: "invalid node shape",
        };
        let typ = a
            .first()
            .and_then(chord_json::JsonValue::as_i64)
            .ok_or_else(bad)?;
        let str_at = |i: usize| {
            a.get(i)
                .and_then(chord_json::JsonValue::as_str)
                .map(str::to_owned)
                .ok_or_else(bad)
        };
        let children = |i: usize| -> Result<Vec<Ast>, AstError> {
            let vals = a
                .get(i)
                .and_then(chord_json::JsonValue::as_array)
                .ok_or_else(bad)?;
            vals.iter()
                .enumerate()
                .map(|(n, x)| parse_ast(x, format!("{path}[{i}][{n}]"), depth + 1))
                .collect()
        };
        return Ok(match typ {
            0 => Ast::Literal(str_at(1)?),
            1 => Ast::Argument(str_at(1)?),
            2 => Ast::Number(str_at(1)?),
            3 => Ast::Date(
                str_at(1)?,
                a.get(2)
                    .and_then(chord_json::JsonValue::as_str)
                    .unwrap_or("medium")
                    .to_owned(),
            ),
            4 => Ast::Time(
                str_at(1)?,
                a.get(2)
                    .and_then(chord_json::JsonValue::as_str)
                    .unwrap_or("short")
                    .to_owned(),
            ),
            5 | 6 => {
                let name = str_at(1)?;
                let opts = a
                    .get(2)
                    .and_then(chord_json::JsonValue::as_object)
                    .ok_or_else(bad)?;
                let mut parsed = BTreeMap::new();
                for (k, v) in opts {
                    let nodes = if v
                        .as_array()
                        .and_then(|x| x.first())
                        .and_then(chord_json::JsonValue::as_i64)
                        .is_some()
                    {
                        vec![parse_ast(v, format!("{path}[2].{k}"), depth + 1)?]
                    } else {
                        let list = v.as_array().ok_or_else(bad)?;
                        list.iter()
                            .enumerate()
                            .map(|(n, x)| parse_ast(x, format!("{path}[2].{k}[{n}]"), depth + 1))
                            .collect::<Result<_, _>>()?
                    };
                    parsed.insert(k.clone(), nodes);
                }
                if typ == 5 {
                    Ast::Select(name, parsed)
                } else {
                    Ast::Plural(
                        name,
                        parsed,
                        a.get(3)
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(0),
                        a.get(4)
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("cardinal")
                            .into(),
                    )
                }
            }
            7 => Ast::Pound,
            8 => {
                let tag = str_at(1)?;
                let kids = children(2)?;
                Ast::Tag(tag, kids, None)
            }
            _ => {
                return Err(AstError {
                    path,
                    kind: "unknown node type",
                });
            }
        });
    }
    if let Some(a) = v.as_array() {
        let mut nodes = Vec::new();
        for (i, x) in a.iter().enumerate() {
            nodes.push(parse_ast(x, format!("{path}[{i}]"), depth + 1)?)
        }
        return Ok(Ast::Sequence(nodes));
    }
    Err(AstError {
        path,
        kind: "expected node",
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
}
impl Value {
    fn display(&self) -> String {
        match self {
            Self::Str(s) => s.clone(),
            Self::Int(n) => n.to_string(),
            Self::Float(n) => n.to_string(),
        }
    }
    fn number(&self) -> Option<f64> {
        match self {
            Self::Int(n) => Some(*n as f64),
            Self::Float(n) => Some(*n),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocaleError {
    Unsupported(String),
}
impl fmt::Display for LocaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(s) => write!(f, "unsupported locale {s}"),
        }
    }
}
impl std::error::Error for LocaleError {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateError {
    MissingMessage {
        key: String,
        locale: String,
    },
    MissingArgument {
        name: String,
    },
    InvalidArgument {
        name: String,
        expected: &'static str,
    },
    UnsupportedNode {
        node: &'static str,
    },
    MissingBranch {
        name: String,
    },
    UnknownTag {
        name: String,
    },
}
impl fmt::Display for TranslateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TranslateError {}
#[derive(Debug, Clone)]
pub struct I18n {
    catalogs: BTreeMap<String, Catalog>,
    current: String,
    default: String,
}
impl I18n {
    pub fn new(default_locale: &str) -> Self {
        let default = canonical_locale(default_locale)
            .filter(|locale| is_enabled(locale))
            .unwrap_or(DEFAULT_LOCALE)
            .to_owned();
        Self {
            catalogs: BTreeMap::new(),
            current: default.clone(),
            default,
        }
    }
    pub fn insert_catalog(&mut self, locale: &str, catalog: Catalog) -> Result<(), LocaleError> {
        let canonical = canonical_locale(locale)
            .filter(|locale| is_enabled(locale))
            .ok_or_else(|| LocaleError::Unsupported(locale.into()))?;
        self.catalogs.insert(canonical.to_owned(), catalog);
        Ok(())
    }
    pub fn set_locale(&mut self, locale: &str) -> Result<(), LocaleError> {
        if let Some(canonical) = canonical_locale(locale)
            && !is_enabled(canonical)
        {
            return Err(LocaleError::Unsupported(locale.into()));
        }
        let available: Vec<&str> = self.catalogs.keys().map(String::as_str).collect();
        let chosen = resolve_locale(locale, &available);
        if !self.catalogs.contains_key(&chosen) {
            return Err(LocaleError::Unsupported(locale.into()));
        }
        self.current = chosen;
        Ok(())
    }
    pub fn translate(
        &self,
        key: &str,
        args: &BTreeMap<String, Value>,
    ) -> Result<String, TranslateError> {
        let mut chain = vec![self.current.clone()];
        if self.current != self.default {
            chain.push(self.default.clone());
        }
        if !chain.iter().any(|locale| locale == DEFAULT_LOCALE) {
            chain.push(DEFAULT_LOCALE.to_owned());
        }
        for locale in chain {
            if let Some(catalog) = self.catalogs.get(&locale)
                && let Some(ast) = catalog.get(key)
            {
                return render_ast(ast, args, None, &locale);
            }
        }
        Err(TranslateError::MissingMessage {
            key: key.into(),
            locale: self.current.clone(),
        })
    }
}
fn render_ast(
    ast: &Ast,
    args: &BTreeMap<String, Value>,
    plural: Option<f64>,
    locale: &str,
) -> Result<String, TranslateError> {
    Ok(match ast {
        Ast::Literal(s) => s.clone(),
        Ast::Sequence(v) => v
            .iter()
            .map(|x| render_ast(x, args, plural, locale))
            .collect::<Result<Vec<_>, _>>()?
            .concat(),
        Ast::Argument(n) => args
            .get(n)
            .ok_or_else(|| TranslateError::MissingArgument { name: n.clone() })?
            .display(),
        Ast::Number(n) => {
            let value = args.get(n).and_then(Value::number).ok_or_else(|| {
                TranslateError::InvalidArgument {
                    name: n.clone(),
                    expected: "number",
                }
            })?;
            format::format_number_named(value, locale, n)
        }
        Ast::Date(n, style) => {
            let value = args.get(n).and_then(Value::number).ok_or_else(|| {
                TranslateError::InvalidArgument {
                    name: n.clone(),
                    expected: "unix timestamp",
                }
            })?;
            let style = DateStyle::parse(style).unwrap_or(DateStyle::Medium);
            format::format_date(value as i64, locale, style)
        }
        Ast::Time(n, style) => {
            let value = args.get(n).and_then(Value::number).ok_or_else(|| {
                TranslateError::InvalidArgument {
                    name: n.clone(),
                    expected: "unix timestamp",
                }
            })?;
            let style = DateStyle::parse(style).unwrap_or(DateStyle::Short);
            format::format_time(value as i64, locale, style)
        }
        Ast::Pound => plural
            .map(|n| format::format_number(n, locale))
            .unwrap_or_else(|| "#".into()),
        Ast::Select(n, opts) => {
            let val = args
                .get(n)
                .ok_or_else(|| TranslateError::MissingArgument { name: n.clone() })?
                .display();
            let b = opts
                .get(&val)
                .or_else(|| opts.get("other"))
                .ok_or_else(|| TranslateError::MissingBranch { name: n.clone() })?;
            render_ast(&Ast::Sequence(b.clone()), args, plural, locale)?
        }
        Ast::Plural(n, opts, offset, kind) => {
            let v = args
                .get(n)
                .ok_or_else(|| TranslateError::MissingArgument { name: n.clone() })?
                .number()
                .ok_or_else(|| TranslateError::MissingArgument { name: n.clone() })?;
            let adjusted = v - *offset as f64;
            let exact = format!("={v}");
            let plural_type = if kind == "ordinal" {
                PluralType::Ordinal
            } else {
                PluralType::Cardinal
            };
            let category = plural::plural_category(locale, adjusted, plural_type);
            let b = opts
                .get(&exact)
                .or_else(|| opts.get(category.as_str()))
                .or_else(|| opts.get("other"))
                .ok_or_else(|| TranslateError::MissingBranch { name: n.clone() })?;
            render_ast(&Ast::Sequence(b.clone()), args, Some(adjusted), locale)?
        }
        Ast::Tag(tag, kids, _) => {
            let body = render_ast(&Ast::Sequence(kids.clone()), args, plural, locale)?;
            match tag.as_str() {
                "$b" => format!("**{body}**"),
                "$i" => format!("*{body}*"),
                "$del" => format!("~~{body}~~"),
                "$code" => format!("`{body}`"),
                "$p" => format!("{body}\n\n"),
                "$link" => body,
                _ if !tag.starts_with('$') => body,
                _ => return Err(TranslateError::UnknownTag { name: tag.clone() }),
            }
        }
    })
}
/// Демонстрация категорий плурализации ru (CLDR): (n, категория).
pub const PLURAL_RU_DEMO: [(i64, &str); 7] = [
    (1, "one"),
    (2, "few"),
    (5, "many"),
    (11, "many"),
    (21, "one"),
    (22, "few"),
    (101, "one"),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    #[test]
    fn hash_vectors_and_locale_selection() {
        for (input, expected) in [
            ("ru", "+irF5O"),
            ("en-US", "+dXJII"),
            ("AKA", "l1QVfj"),
            ("OK", "TyCVIq"),
            ("PS4", "fJyyQn"),
            ("PS5", "oElqN5"),
        ] {
            assert_eq!(super::message_key(input), expected);
        }
        assert_eq!(super::xxh64(b"", 0), 0xef46_db37_51d8_e999);
        assert_eq!(super::message_key("abc").len(), 6);
        assert_eq!(super::SUPPORTED_LOCALES.len(), 36);
        assert_eq!(
            super::ENABLED_LOCALES + super::DISABLED_LOCALES.len(),
            super::TOTAL_LOCALES
        );
        let available = ["en-US", "ru", "fr"];
        assert_eq!(super::resolve_locale("RU-ru", &available), "ru");
        assert_eq!(super::resolve_locale("fr-FR", &available), "fr");
        assert_eq!(super::resolve_locale("pt-PT", &available), "en-US");
        assert_eq!(super::resolve_locale("", &available), "en-US");
    }

    #[test]
    fn catalog_translate_plural_and_rich_tags() {
        use super::{Catalog, I18n, Value};
        let json = chord_json::parse(r#"{
          "hello":[[0,"Привет "],[1,"name"]],
          "count":[6,"n",{"one":[[7],[0," игра"]],"few":[[7],[0," игры"]],"many":[[7],[0," игр"]],"other":[[7],[0," игр"]]},0,"cardinal"],
          "bold":[8,"$b",[[0,"ok"]]], "select":[5,"x",{"a":[0,"A"],"other":[0,"?"]}],
          "exact0":[6,"n",{"=0":[[0,"ноль"]],"other":[[0,"другое"]]},0,"cardinal"],
          "offset":[6,"n",{"one":[[7],[0," one"]],"other":[[7],[0," other"]]},2,"cardinal"],
          "en_plural":[6,"n",{"one":[[0,"one"]],"other":[[0,"other"]]},0,"cardinal"]
        }"#).unwrap();
        let cat = Catalog::parse(&json).unwrap();
        assert_eq!(cat.len(), 7);
        assert!(cat.get("hello").is_some());
        let mut i = I18n::new("en-US");
        i.insert_catalog("ru", cat).unwrap();
        i.set_locale("ru").unwrap();
        let args = BTreeMap::from([("name".into(), Value::Str("Мир".into()))]);
        assert_eq!(i.translate("hello", &args).unwrap(), "Привет Мир");
        assert!(matches!(
            i.translate("missing", &args),
            Err(super::TranslateError::MissingMessage { .. })
        ));
        assert!(matches!(
            i.translate("hello", &BTreeMap::new()),
            Err(super::TranslateError::MissingArgument { .. })
        ));
        for (n, expected) in [
            (1, "1 игра"),
            (2, "2 игры"),
            (5, "5 игр"),
            (11, "11 игр"),
            (21, "21 игра"),
            (22, "22 игры"),
            (101, "101 игра"),
        ] {
            assert_eq!(
                i.translate("count", &BTreeMap::from([("n".into(), Value::Int(n))]))
                    .unwrap(),
                expected
            );
        }
        let exact_args = |n| BTreeMap::from([("n".into(), Value::Int(n))]);
        assert_eq!(i.translate("exact0", &exact_args(0)).unwrap(), "ноль");
        assert_eq!(i.translate("exact0", &exact_args(1)).unwrap(), "другое");
        assert_eq!(i.translate("offset", &exact_args(3)).unwrap(), "1 one");
        assert_eq!(i.translate("offset", &exact_args(2)).unwrap(), "0 other");
        assert_eq!(i.translate("offset", &exact_args(4)).unwrap(), "2 other");

        let en_json = chord_json::parse(
            r#"{"p":[6,"n",{"one":[[0,"one"]],"other":[[0,"other"]]},0,"cardinal"]}"#,
        )
        .unwrap();
        let mut en = I18n::new("en-US");
        en.insert_catalog("en-US", Catalog::parse(&en_json).unwrap())
            .unwrap();
        assert_eq!(
            en.translate("p", &BTreeMap::from([("n".into(), Value::Int(1))]))
                .unwrap(),
            "one"
        );
        assert_eq!(
            en.translate("p", &BTreeMap::from([("n".into(), Value::Int(2))]))
                .unwrap(),
            "other"
        );

        assert_eq!(i.translate("bold", &BTreeMap::new()).unwrap(), "**ok**");
        for (tag, expected) in [
            ("$i", "*ok*"),
            ("$del", "~~ok~~"),
            ("$code", "`ok`"),
            ("$p", "ok\n\n"),
            ("$link", "ok"),
            ("hook", "ok"),
        ] {
            let raw = format!(r#"{{"x":[8,"{tag}",[[0,"ok"]]]}}"#);
            let mut local = I18n::new("en-US");
            local
                .insert_catalog(
                    "en-US",
                    Catalog::parse(&chord_json::parse(&raw).unwrap()).unwrap(),
                )
                .unwrap();
            assert_eq!(local.translate("x", &BTreeMap::new()).unwrap(), expected);
        }
        let unknown =
            Catalog::parse(&chord_json::parse(r#"{"x":[8,"$unknown",[]]}"#).unwrap()).unwrap();
        let mut check = I18n::new("en-US");
        check.insert_catalog("en-US", unknown).unwrap();
        assert!(matches!(
            check.translate("x", &BTreeMap::new()),
            Err(super::TranslateError::UnknownTag { .. })
        ));
    }

    #[test]
    fn number_formatting_uses_locale_separators_and_keeps_fraction() {
        use super::{Catalog, I18n, Value};
        let en = Catalog::parse(
            &chord_json::parse(r#"{"num":[2,"x"],"fallback":[[0,"en fallback"]]}"#).unwrap(),
        )
        .unwrap();
        let ru = Catalog::parse(&chord_json::parse(r#"{"num":[2,"x"]}"#).unwrap()).unwrap();
        let mut i18n = I18n::new("en-US");
        i18n.insert_catalog("en-US", en).unwrap();
        i18n.insert_catalog("ru", ru).unwrap();

        assert_eq!(
            i18n.translate("num", &BTreeMap::from([("x".into(), Value::Float(1.5))]))
                .unwrap(),
            "1.5"
        );
        i18n.set_locale("ru").unwrap();
        assert_eq!(
            i18n.translate("num", &BTreeMap::from([("x".into(), Value::Float(1.5))]))
                .unwrap(),
            "1,5"
        );
        assert_eq!(
            i18n.translate(
                "num",
                &BTreeMap::from([("x".into(), Value::Int(1_234_567))])
            )
            .unwrap(),
            "1\u{00a0}234\u{00a0}567"
        );
        // Ключ есть только в en-US: requested (ru) → default (en-US).
        assert_eq!(
            i18n.translate("fallback", &BTreeMap::new()).unwrap(),
            "en fallback"
        );
    }

    #[test]
    fn plural_exact_and_fractional_categories_are_cldr_safe() {
        use super::{Catalog, I18n, TranslateError, Value};
        let json = chord_json::parse(
            r#"{"frac":[6,"n",{"=2":[[0,"exact"]],"one":[[0,"one"]],"few":[[0,"few"]],"many":[[0,"many"]],"other":[[0,"other"]]},0,"cardinal"]}"#,
        )
        .unwrap();
        let catalog = Catalog::parse(&json).unwrap();
        let mut i18n = I18n::new("ru");
        i18n.insert_catalog("ru", catalog).unwrap();
        let translate =
            |n| i18n.translate("frac", &BTreeMap::from([("n".into(), Value::Float(n))]));
        assert_eq!(translate(2.0).unwrap(), "exact");
        assert_eq!(translate(1.5).unwrap(), "other");
        assert_eq!(translate(-1.0).unwrap(), "one");
        assert_eq!(translate(21.0).unwrap(), "one");
        assert_eq!(translate(22.0).unwrap(), "few");
        assert_eq!(translate(11.0).unwrap(), "many");
        assert!(matches!(
            translate(2.5),
            Ok(ref value) if value == "other"
        ));
        assert!(!matches!(
            translate(1.5),
            Err(TranslateError::MissingBranch { .. })
        ));
    }

    #[test]
    fn markdown_is_safe_and_handles_unclosed_markers() {
        use super::{parse_markdown, render_markdown};
        for (src, expected) in [
            ("**bold**", "**bold**"),
            ("*em*", "*em*"),
            ("__u__", "__u__"),
            ("~~x~~", "~~x~~"),
            ("||s||", "||s||"),
            ("`c`", "`c`"),
            ("[x](https://a.test)", "[x](https://a.test)"),
            (r#"\*"#, "*"),
        ] {
            assert_eq!(render_markdown(&parse_markdown(src)), expected);
        }
        assert_eq!(
            render_markdown(&parse_markdown("[x](javascript:alert(1))")),
            "x"
        );
        assert_eq!(
            render_markdown(&parse_markdown(
                "[x](https://safe.test) [y](javascript:alert(1))"
            )),
            "[x](https://safe.test) y"
        );
        for input in ["```\n", "```", "```\nbody", "```rs\n"] {
            let _ = parse_markdown(input);
        }
        assert_eq!(
            render_markdown(&parse_markdown("```rs\nlet x = 1;\n```")),
            "```rs\nlet x = 1;\n```"
        );
        assert_eq!(
            render_markdown(&parse_markdown("<script>alert(1)</script>")),
            "<script>alert(1)</script>"
        );
        assert_eq!(render_markdown(&parse_markdown("")), "");
        assert_eq!(render_markdown(&parse_markdown("**open")), "**open");
        for n in 0..10_000 {
            let s = format!("**message {n}* escaped \\* and [link](javascript:alert({n}))");
            for end in (0..=s.len()).step_by(7) {
                if s.is_char_boundary(end) {
                    let _ = parse_markdown(&s[..end]);
                }
            }
        }
    }

    #[test]
    fn real_catalogs_match_independent_inventory() {
        for (path, count) in [
            ("/home/sanya/re-deep/ru-catalog.json", 23_836),
            ("/home/sanya/re-deep/en-catalog.json", 23_903),
        ] {
            let Ok(raw) = std::fs::read_to_string(path) else {
                eprintln!("skip: unavailable oracle {path}");
                continue;
            };
            let value = chord_json::parse(&raw).unwrap();
            let catalog = super::Catalog::parse(&value).unwrap();
            assert_eq!(catalog.len(), count);
            if path.contains("ru-") {
                assert!(matches!(
                    catalog.get("hPBScv"),
                    Some(super::Ast::Sequence(_))
                ));
                let link_message = catalog.get("323362").expect("oracle message exists");
                assert!(format!("{link_message:?}").contains("$link"));
            }
        }
    }

    #[test]
    fn locale_math_is_consistent() {
        assert_eq!(
            super::ENABLED_LOCALES + super::DISABLED_LOCALES.len(),
            super::TOTAL_LOCALES
        );
        assert_eq!(super::MESSAGE_KEY_LEN, 6);
    }

    #[test]
    fn ru_plural_categories() {
        for (n, cat) in super::PLURAL_RU_DEMO {
            let expected = match n % 10 {
                1 if n % 100 != 11 => "one",
                2..=4 if !(12..=14).contains(&(n % 100)) => "few",
                _ => "many",
            };
            assert_eq!(cat, expected, "n={n}");
        }
    }
}
