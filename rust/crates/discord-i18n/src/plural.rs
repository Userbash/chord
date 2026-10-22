//! CLDR cardinal and ordinal plural categories (P7-01).
//!
//! Contract: `waves/RW-24/WAVE-PLAN.md` §3.2. The implementation is owned by W25a.
//! Integer values can be classified exactly (`plural_category_integer`) without an f64
//! round-trip, so categories of `u64`/`i64` magnitudes above 2^53 stay correct.

/// CLDR plural category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    /// Stable CLDR spelling used by catalog branch names.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

/// CLDR plural type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralType {
    Cardinal,
    Ordinal,
}

/// CLDR operands for one value.
#[derive(Debug, Clone, Copy)]
struct Operands {
    n: f64,
    i: u64,
    v: usize,
    f: u64,
    t: u64,
    n_mod_10: f64,
    n_mod_100: f64,
    i_mod_10: u64,
    i_mod_100: u64,
    exact_million: bool,
}

impl Operands {
    fn from_f64(value: f64) -> Self {
        let n = if value.is_finite() { value.abs() } else { 0.0 };
        let text = n.to_string();
        let fraction = text.split_once('.').map_or("", |(_, f)| f);
        let v = fraction.len();
        let i = text
            .split('.')
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(u64::MAX);
        let f = fraction.parse::<u64>().unwrap_or(0);
        let t = fraction.trim_end_matches('0').parse::<u64>().unwrap_or(0);
        Self {
            n,
            i,
            v,
            f,
            t,
            n_mod_10: n % 10.0,
            n_mod_100: n % 100.0,
            i_mod_10: i % 10,
            i_mod_100: i % 100,
            exact_million: v == 0 && i != 0 && i.is_multiple_of(1_000_000),
        }
    }

    fn from_u64(value: u64) -> Self {
        Self {
            n: value as f64,
            i: value,
            v: 0,
            f: 0,
            t: 0,
            n_mod_10: (value % 10) as f64,
            n_mod_100: (value % 100) as f64,
            i_mod_10: value % 10,
            i_mod_100: value % 100,
            exact_million: value != 0 && value.is_multiple_of(1_000_000),
        }
    }
}

/// Select a CLDR category for `value` in `locale`.
#[must_use]
pub fn plural_category(locale: &str, value: f64, kind: PluralType) -> PluralCategory {
    select(locale, kind, Operands::from_f64(value))
}

/// Select a CLDR category for a non-negative integer without an f64 round-trip.
///
/// Equivalent to `plural_category(locale, value as f64, kind)` for `value <= 2^53`, and
/// exact above it (digit-based rules such as Russian `one`/`few`/`many`).
#[must_use]
pub fn plural_category_integer(locale: &str, value: u64, kind: PluralType) -> PluralCategory {
    select(locale, kind, Operands::from_u64(value))
}

fn select(locale: &str, kind: PluralType, ops: Operands) -> PluralCategory {
    let language = locale
        .split('-')
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase();
    let Operands {
        n,
        i,
        v,
        f,
        t,
        n_mod_10,
        n_mod_100,
        i_mod_10,
        i_mod_100,
        exact_million,
    } = ops;
    let cat = |s| match s {
        "zero" => PluralCategory::Zero,
        "one" => PluralCategory::One,
        "two" => PluralCategory::Two,
        "few" => PluralCategory::Few,
        "many" => PluralCategory::Many,
        _ => PluralCategory::Other,
    };
    match kind {
        PluralType::Ordinal => match language.as_str() {
            "en" if v == 0 => {
                let m10 = i % 10;
                if i % 100 / 10 != 1 {
                    match m10 {
                        1 => cat("one"),
                        2 => cat("two"),
                        3 => cat("few"),
                        _ => cat("other"),
                    }
                } else {
                    cat("other")
                }
            }
            "fr" | "ro" | "vi" => {
                if n == 1.0 {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "hu" => {
                if n == 1.0 || n == 5.0 {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "it" => {
                if [8.0, 11.0, 80.0, 800.0].contains(&n) {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "sv" => {
                if v == 0 && (1..=2).contains(&i_mod_10) && ![11, 12].contains(&i_mod_100) {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "uk" => {
                if v == 0 && i_mod_10 == 3 && i_mod_100 != 13 {
                    cat("few")
                } else {
                    cat("other")
                }
            }
            "hi" => match n {
                1.0 => cat("one"),
                2.0 | 3.0 => cat("two"),
                4.0 => cat("few"),
                6.0 => cat("many"),
                _ => cat("other"),
            },
            _ => cat("other"),
        },
        PluralType::Cardinal => match language.as_str() {
            "ja" | "ko" | "zh" | "th" | "vi" | "id" => cat("other"),
            "en" | "de" | "nl" | "sv" => {
                if i == 1 && v == 0 {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "da" => {
                if n == 1.0 || (t != 0 && (i == 0 || i == 1)) {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "no" | "fi" | "bg" | "el" | "hu" | "tr" => {
                if n == 1.0 {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "es" => {
                if n == 1.0 {
                    cat("one")
                } else if exact_million {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "pt" if locale.eq_ignore_ascii_case("pt-PT") => {
                if i == 1 && v == 0 {
                    cat("one")
                } else if exact_million {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "pt" => {
                if i == 0 || i == 1 {
                    cat("one")
                } else if exact_million {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "fr" => {
                if i == 0 || i == 1 {
                    cat("one")
                } else if exact_million {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "it" => {
                if i == 1 && v == 0 {
                    cat("one")
                } else if exact_million {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "hi" => {
                if i == 0 || n == 1.0 {
                    cat("one")
                } else {
                    cat("other")
                }
            }
            "pl" => {
                if v != 0 {
                    cat("other")
                } else if i == 1 {
                    cat("one")
                } else if (2..=4).contains(&i_mod_10) && !(12..=14).contains(&i_mod_100) {
                    cat("few")
                } else {
                    cat("many")
                }
            }
            "ru" | "uk" => {
                if v == 0 && i_mod_10 == 1 && i_mod_100 != 11 {
                    cat("one")
                } else if v == 0 && (2..=4).contains(&i_mod_10) && !(12..=14).contains(&i_mod_100) {
                    cat("few")
                } else if v == 0
                    && (i_mod_10 == 0
                        || (5..=9).contains(&i_mod_10)
                        || (11..=14).contains(&i_mod_100))
                {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "cs" | "sk" => {
                if i == 1 && v == 0 {
                    cat("one")
                } else if (2..=4).contains(&i) && v == 0 {
                    cat("few")
                } else if v != 0 {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "hr" => {
                if (v == 0 && i_mod_10 == 1 && i_mod_100 != 11) || (f % 10 == 1 && f % 100 != 11) {
                    cat("one")
                } else if (v == 0 && (2..=4).contains(&i_mod_10) && !(12..=14).contains(&i_mod_100))
                    || ((2..=4).contains(&(f % 10)) && !(12..=14).contains(&(f % 100)))
                {
                    cat("few")
                } else {
                    cat("other")
                }
            }
            "ro" => {
                if i == 1 && v == 0 {
                    cat("one")
                } else if v != 0 || n == 0.0 || (1.0..=19.0).contains(&n_mod_100) {
                    cat("few")
                } else {
                    cat("other")
                }
            }
            "lt" => {
                if f != 0 {
                    cat("many")
                } else if n_mod_10 == 1.0 && !(11.0..=19.0).contains(&n_mod_100) {
                    cat("one")
                } else if (2.0..=9.0).contains(&n_mod_10) && !(11.0..=19.0).contains(&n_mod_100) {
                    cat("few")
                } else {
                    cat("other")
                }
            }
            "ar" => {
                if n == 0.0 {
                    cat("zero")
                } else if n == 1.0 {
                    cat("one")
                } else if n == 2.0 {
                    cat("two")
                } else if v == 0 && (3.0..=10.0).contains(&n_mod_100) {
                    cat("few")
                } else if v == 0 && (11.0..=99.0).contains(&n_mod_100) {
                    cat("many")
                } else {
                    cat("other")
                }
            }
            "he" => {
                if (i == 1 && v == 0) || (i == 0 && v != 0) {
                    cat("one")
                } else if i == 2 && v == 0 {
                    cat("two")
                } else {
                    cat("other")
                }
            }
            _ => {
                if i == 1 && v == 0 {
                    cat("one")
                } else {
                    cat("other")
                }
            }
        },
    }
}

/// Select a cardinal category.
#[must_use]
pub fn cardinal(locale: &str, value: f64) -> PluralCategory {
    plural_category(locale, value, PluralType::Cardinal)
}

/// Select an ordinal category.
#[must_use]
pub fn ordinal(locale: &str, value: f64) -> PluralCategory {
    plural_category(locale, value, PluralType::Ordinal)
}
