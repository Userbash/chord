//! Контракт P6-07: spellcheck через FFI к системному `libhunspell` (headless).
//!
//! Независимый оракул — сама библиотека `libhunspell 1.7.5` и синтетический словарь
//! `fixtures/hunspell/test.{aff,dic}` (не копия системных словарей). Тесты копируют фикстуры
//! во временный каталог, чтобы не менять исходники. CLI-оракула у hunspell в окружении нет,
//! поэтому проверяются семантические инварианты контракта (TZ W32a §1/§3).
//!
//! Skip-aware: если системной библиотеки нет, тесты печатают `SKIP: <причина>` и завершаются
//! успешно (окружение недоступно, а не падение).

use chord_platform::spellcheck::{SpellChecker, SpellError, tokenize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SKIP_NO_LIB: &str = "нет системного libhunspell-1.7";
const MAX_SUGGESTIONS: usize = 10;
const RSS_LIMIT_KILOBYTES: u64 = 8 * 1024;

static FIXTURE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Копия фикстур во временном каталоге; удаляется в `Drop`.
struct Fixture {
    dir: PathBuf,
    aff: PathBuf,
    dic: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/hunspell");
        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w32a-{}-{}",
            std::process::id(),
            FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("временный каталог фикстур");
        let aff = dir.join("test.aff");
        let dic = dir.join("test.dic");
        fs::copy(source.join("test.aff"), &aff).expect("копия test.aff");
        fs::copy(source.join("test.dic"), &dic).expect("копия test.dic");
        Self { dir, aff, dic }
    }

    fn aff(&self) -> &Path {
        &self.aff
    }

    fn dic(&self) -> &Path {
        &self.dic
    }

    fn dir(&self) -> &Path {
        &self.dir
    }

    /// Временный файл с заданным содержимым (для кейсов битого словаря).
    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.dir.join(name);
        fs::write(&path, content).expect("запись временного файла");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // best-effort: временный каталог уникален, неудача очистки не влияет на тест.
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Skip-aware проверка наличия библиотеки (среда, а не падение).
fn hunspell_available() -> bool {
    const CANDIDATES: [&str; 6] = [
        "/usr/lib/libhunspell-1.7.so",
        "/usr/lib/x86_64-linux-gnu/libhunspell-1.7.so",
        "/usr/lib/libhunspell.so",
        "/usr/local/lib/libhunspell-1.7.so",
        "/lib/libhunspell-1.7.so",
        "/lib/x86_64-linux-gnu/libhunspell-1.7.so",
    ];
    if CANDIDATES.iter().any(|path| Path::new(path).is_file()) {
        return true;
    }
    println!("SKIP: {SKIP_NO_LIB}");
    false
}

fn checker(fixture: &Fixture) -> SpellChecker {
    SpellChecker::new(fixture.aff(), fixture.dic()).expect("SpellChecker::new на фикстурах")
}

/// Токены как `(срез исходной строки, start, end)` — независимая сверка байтовых смещений.
fn slices(text: &str) -> Vec<(&str, usize, usize)> {
    tokenize(text)
        .iter()
        .map(|token| (&text[token.start..token.end], token.start, token.end))
        .collect()
}

fn texts(text: &str) -> Vec<String> {
    tokenize(text)
        .iter()
        .map(|token| token.text.clone())
        .collect()
}

fn rss_kilobytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

fn dic_header(path: &Path) -> String {
    let file = fs::File::open(path).expect("открытие dic");
    let mut line = String::new();
    BufReader::new(file)
        .read_line(&mut line)
        .expect("чтение заголовка dic");
    line
}

#[test]
fn h01_new_on_fixtures_is_ok() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    // Заголовок словаря — независимая проверка, что скопирован именно наш словарь.
    assert_eq!(dic_header(fixture.dic()).trim(), "6");
    let mut checker = checker(&fixture);
    assert!(checker.check("hello"));
    drop(checker);
    assert!(SpellChecker::new(fixture.aff(), fixture.dic()).is_ok());
}

#[test]
fn h02_check_accepts_dictionary_words() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    let mut checker = checker(&fixture);
    for word in ["hello", "world", "привет", "мир", "don't", "well-known"] {
        assert!(checker.check(word), "слово {word:?} должно быть в словаре");
    }
}

#[test]
fn h03_check_rejects_unknown_words() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    let mut checker = checker(&fixture);
    for word in ["helo", "приветъ", "zzzz", "hello123"] {
        assert!(!checker.check(word), "слова {word:?} нет в словаре");
    }
}

#[test]
fn h04_suggest_finds_hello_and_is_bounded() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    let mut checker = checker(&fixture);
    let suggestions = checker.suggest("helo");
    assert!(
        suggestions.iter().any(|word| word == "hello"),
        "ожидался вариант hello, получено {suggestions:?}"
    );
    assert!(
        suggestions.len() <= MAX_SUGGESTIONS,
        "вариантов больше {MAX_SUGGESTIONS}: {}",
        suggestions.len()
    );
    assert_eq!(
        suggestions,
        checker.suggest("helo"),
        "порядок вариантов должен быть детерминирован"
    );
}

#[test]
fn h05_add_and_remove_change_check() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    let mut checker = checker(&fixture);
    assert!(!checker.check("zorblax"));
    assert!(checker.add("zorblax"), "hunspell_add должен вернуть успех");
    assert!(
        checker.check("zorblax"),
        "добавленное слово должно проходить check"
    );
    assert!(
        checker.remove("zorblax"),
        "hunspell_remove должен вернуть успех"
    );
    assert!(
        !checker.check("zorblax"),
        "удалённое слово снова не должно проходить check"
    );
}

#[test]
fn h06_tokenize_ascii_offsets_and_punctuation() {
    let text = "Hello, world! It's fine. abc123def";
    assert_eq!(
        slices(text),
        vec![
            ("Hello", 0, 5),
            ("world", 7, 12),
            ("It's", 14, 18),
            ("fine", 19, 23),
            ("abc", 25, 28),
            ("def", 31, 34),
        ]
    );
    // Пунктуация и цифры не приклеиваются; пробелы по краям игнорируются.
    assert_eq!(
        texts("  leading and trailing  "),
        vec!["leading", "and", "trailing"]
    );
    assert_eq!(
        texts("a_b"),
        vec!["a", "b"],
        "подчёркивание разрывает слово"
    );
    assert_eq!(texts("123 456"), Vec::<String>::new());
}

#[test]
fn h07_tokenize_cyrillic_hyphen_apostrophe() {
    let text = "привет, мир! well-known; don't — всё.";
    assert_eq!(
        slices(text),
        vec![
            ("привет", 0, 12),
            ("мир", 14, 20),
            ("well-known", 22, 32),
            ("don't", 34, 39),
            ("всё", 44, 50),
        ]
    );
    // Дефис/апостроф допустимы только внутри слова, не в начале и не в конце.
    assert_eq!(
        texts("hello- -- -dash 'quote' don't stop"),
        vec!["hello", "dash", "quote", "don't", "stop"]
    );
}

#[test]
fn h08_tokenize_skips_urls_emails_code_mentions_and_emoji() {
    let text = concat!(
        "see https://example.com/a?b=1 and mail user@example.com, ",
        "`code here` <@alice> <#general> :emoji_1: hello"
    );
    assert_eq!(texts(text), vec!["see", "and", "mail", "hello"]);
    for token in tokenize(text) {
        assert_eq!(&text[token.start..token.end], token.text);
        assert!(token.start < token.end);
    }
    assert_eq!(texts("https://one.example/path"), Vec::<String>::new());
    assert_eq!(texts("a.b@c.example"), Vec::<String>::new());
    assert_eq!(texts("`let value = 1;`"), Vec::<String>::new());
    assert_eq!(texts(":wave:"), Vec::<String>::new());
}

#[test]
fn h09_empty_and_wordless_texts_are_empty() {
    assert!(tokenize("").is_empty());
    assert!(tokenize("   \t\n\r\n  ").is_empty());
    assert!(tokenize("1234567890 _-.,;:!?()[]{}").is_empty());
    assert!(tokenize("`unterminated code").is_empty());
    assert!(tokenize("<@123> <#456>").is_empty());
}

#[test]
fn h10_broken_or_missing_dictionary_is_error() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();

    let missing_aff = fixture.dir().join("absent.aff");
    match SpellChecker::new(&missing_aff, fixture.dic()) {
        Err(SpellError::Missing { .. }) => {}
        other => panic!("отсутствующий aff: ожидался SpellError::Missing, получено {other:?}"),
    }

    let missing_dic = fixture.dir().join("absent.dic");
    match SpellChecker::new(fixture.aff(), &missing_dic) {
        Err(SpellError::Missing { .. }) => {}
        other => panic!("отсутствующий dic: ожидался SpellError::Missing, получено {other:?}"),
    }

    let empty_dic = fixture.write("empty.dic", "");
    match SpellChecker::new(fixture.aff(), &empty_dic) {
        Err(SpellError::InvalidDictionary { .. }) => {}
        other => panic!("пустой dic: ожидался InvalidDictionary, получено {other:?}"),
    }

    let corrupt_dic = fixture.write("corrupt.dic", "not-a-number\nhello\n");
    match SpellChecker::new(fixture.aff(), &corrupt_dic) {
        Err(SpellError::InvalidDictionary { .. }) => {}
        other => panic!("битый dic: ожидался InvalidDictionary, получено {other:?}"),
    }

    let empty_aff = fixture.write("empty.aff", "");
    match SpellChecker::new(&empty_aff, fixture.dic()) {
        Err(SpellError::InvalidDictionary { .. }) => {}
        other => panic!("пустой aff: ожидался InvalidDictionary, получено {other:?}"),
    }

    let no_set_aff = fixture.write("no-set.aff", "TRY abcdefghijklmnopqrstuvwxyz\n");
    match SpellChecker::new(&no_set_aff, fixture.dic()) {
        Err(SpellError::InvalidDictionary { .. }) => {}
        other => panic!("aff без SET: ожидался InvalidDictionary, получено {other:?}"),
    }

    let junk_dic = fixture.write("junk.dic", "1\n@@@\n");
    match SpellChecker::new(fixture.aff(), &junk_dic) {
        Err(SpellError::InvalidDictionary { .. }) => {}
        other => panic!("мусорная запись dic: ожидался InvalidDictionary, получено {other:?}"),
    }

    let count_mismatch = fixture.write("mismatch.dic", "2\nhello\n");
    match SpellChecker::new(fixture.aff(), &count_mismatch) {
        Err(SpellError::InvalidDictionary { .. }) => {}
        other => panic!("несовпадение счётчика: ожидался InvalidDictionary, получено {other:?}"),
    }
}

#[test]
fn h11_drop_cycles_do_not_leak_or_crash() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    // Прогрев: аллокатор получает свою арену до базового замера.
    for _ in 0..20 {
        let checker = checker(&fixture);
        drop(checker);
    }
    let baseline = rss_kilobytes();
    for _ in 0..100 {
        let checker = checker(&fixture);
        drop(checker);
    }
    if let (Some(before), Some(after)) = (baseline, rss_kilobytes()) {
        let growth = after.saturating_sub(before);
        assert!(
            growth < RSS_LIMIT_KILOBYTES,
            "100 циклов create/drop утекли {growth} KiB (порог {RSS_LIMIT_KILOBYTES} KiB)"
        );
    } else {
        println!("NOTE: /proc/self/status недоступен — RSS-проверка пропущена");
    }
}

#[test]
fn h12_nul_in_path_is_error_without_panic() {
    if !hunspell_available() {
        return;
    }
    let fixture = Fixture::new();
    let bad_aff = Path::new("/tmp/discord-lab\0/test.aff");
    match SpellChecker::new(bad_aff, fixture.dic()) {
        Err(SpellError::Nul { .. }) => {}
        other => panic!("NUL в aff: ожидался SpellError::Nul, получено {other:?}"),
    }
    let bad_dic = Path::new("/tmp/discord-lab\0/test.dic");
    match SpellChecker::new(fixture.aff(), bad_dic) {
        Err(SpellError::Nul { .. }) => {}
        other => panic!("NUL в dic: ожидался SpellError::Nul, получено {other:?}"),
    }
}

// Компиляционное утверждение `!Send + !Sync`: при реализации Send/Sync у `SpellChecker`
// эти impl'ы станут неоднозначными и тест перестанет компилироваться.
trait AmbiguousIfSend<A> {
    fn probe() {}
}
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}

trait AmbiguousIfSync<A> {
    fn probe() {}
}
impl<T: ?Sized> AmbiguousIfSync<()> for T {}
impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}

#[test]
fn h13_not_send_sync_and_tokenize_never_panics() {
    let _ = <SpellChecker as AmbiguousIfSend<_>>::probe;
    let _ = <SpellChecker as AmbiguousIfSync<_>>::probe;

    // Произвольный Unicode не должен паниковать на границах char/байтов.
    for text in [
        "🦀🦀 hello 🦀",
        "日本語 テスト",
        "e\u{0301}cole",
        "\u{fffd}\u{fffd}",
        "''--",
        "`unclosed",
        "https://",
        "<@>",
        "<#",
        ":a:",
        "а-б'в",
    ] {
        let tokens = tokenize(text);
        for token in tokens {
            assert!(token.start < token.end);
            assert_eq!(&text[token.start..token.end], token.text);
        }
    }
    assert_eq!(texts("🦀hello🦀"), vec!["hello"]);
    assert_eq!(texts("日本語"), vec!["日本語"]);
}
