//! Контракт P6-06: питание и idle через freedesktop D-Bus (`ScreenSaver`, `login1`).
//!
//! Независимый оракул — приватный `dbus-daemon` (`--session --print-address --fork`) и
//! мок-сервисы `waves/RW-31/power/mock_power.py` на **libdbus** (`python3-dbus`): они
//! регистрируют `org.freedesktop.ScreenSaver` и `org.freedesktop.login1`, журналируют
//! вызовы (TSV) и эмитят `PrepareForSleep`/чужие сигналы. Rust-код здесь — только клиент,
//! поэтому поведение не может быть «подогнано» под свою же реализацию.
//!
//! Skip-aware: нет `dbus-daemon`, `python3-dbus`/`gi` или мок не поднялся → тест печатает
//! `SKIP: <причина>` и завершается успешно (окружение недоступно, а не падение).
//!
//! Live-проверка (`p11`) выключена по умолчанию и только читает состояние
//! (`IsActive`/`GetSessionIdleTime`): экран не гасится, ингибитор не ставится.

use discord_platform::dbus::{BusAddress, Connection, DbusError, Value};
use discord_platform::power::{
    IdlePolicy, LOGIN1_DESTINATION, LOGIN1_INTERFACE, LOGIN1_PATH, PowerEvent, PowerMonitor,
    PresenceStatus, SCREENSAVER_DESTINATION, SCREENSAVER_INTERFACE, SCREENSAVER_PATH, SLEEP_SIGNAL,
    ScreenSaver, SleepInhibitor,
};
use std::fs;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SKIP_NO_DAEMON: &str = "нет dbus-daemon";
const SKIP_NO_PYTHON: &str = "нет python3 с модулями dbus и gi";
const MOCK_DESTINATION: &str = "lab.Mock";
const MOCK_PATH: &str = "/lab/Mock";
const MOCK_INTERFACE: &str = "lab.Mock.Power";

/// Приватная шина + мок-сервисы; всё убирается в `Drop`.
struct Harness {
    address: String,
    journal: PathBuf,
    dir: PathBuf,
    daemon_pid: u32,
    mock: Child,
}

impl Harness {
    fn start(extra_mock_args: &[&str]) -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(SKIP_NO_PYTHON.to_owned());
        }

        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w31a-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("tempdir {}: {err}", dir.display()))?;

        let started = Command::new("dbus-daemon")
            .args(["--session", "--print-address=1", "--print-pid=1", "--fork"])
            .output()
            .map_err(|err| format!("запуск dbus-daemon: {err}"))?;
        if !started.status.success() {
            return Err(format!(
                "dbus-daemon завершился: {}",
                String::from_utf8_lossy(&started.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&started.stdout).into_owned();
        let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
        let address = lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал адрес".to_owned())?
            .trim()
            .to_owned();
        let daemon_pid: u32 = lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал pid".to_owned())?
            .trim()
            .parse()
            .map_err(|err| format!("pid dbus-daemon: {err}"))?;

        let journal = dir.join("journal.tsv");
        let mut mock = Command::new("python3")
            .arg(repo_path("waves/RW-31/power/mock_power.py"))
            .arg("--address")
            .arg(&address)
            .arg("--log")
            .arg(&journal)
            .args(extra_mock_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| abort(daemon_pid, &dir, format!("запуск mock_power.py: {err}")))?;

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if fs::read_to_string(&journal)
                .map(|text| text.contains("READY"))
                .unwrap_or(false)
            {
                break;
            }
            if let Ok(Some(status)) = mock.try_wait() {
                let stderr = read_child_stderr(&mut mock);
                return Err(abort(
                    daemon_pid,
                    &dir,
                    format!("мок-питание завершился ({status}): {stderr}"),
                ));
            }
            if Instant::now() > deadline {
                return Err(abort(
                    daemon_pid,
                    &dir,
                    "мок-питание не поднялся за 10 с".to_owned(),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Ok(Self {
            address,
            journal,
            dir,
            daemon_pid,
            mock,
        })
    }

    fn bus_address(&self) -> BusAddress {
        BusAddress::parse(&self.address).expect("адрес приватного демона")
    }

    fn journal(&self) -> Vec<Vec<String>> {
        fs::read_to_string(&self.journal)
            .unwrap_or_default()
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect()
    }

    fn journal_member(&self, member: &str) -> Vec<Vec<String>> {
        self.journal()
            .into_iter()
            .filter(|fields| fields.first().map(String::as_str) == Some(member))
            .collect()
    }

    /// Ждёт появления `count` записей журнала (мок пишет их асинхронно).
    fn wait_for_member(&self, member: &str, count: usize) -> Vec<Vec<String>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records = self.journal_member(member);
            if records.len() >= count || Instant::now() > deadline {
                return records;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn kill_daemon(&mut self) {
        let _ = Command::new("kill")
            .arg(self.daemon_pid.to_string())
            .status();
        self.daemon_pid = 0;
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.mock.kill();
        let _ = self.mock.wait();
        if self.daemon_pid != 0 {
            let _ = Command::new("kill")
                .arg(self.daemon_pid.to_string())
                .status();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Убирает приватный демон и tempdir при ошибке старта (skip-путь без `Harness`).
fn abort(daemon_pid: u32, dir: &Path, reason: String) -> String {
    let _ = Command::new("kill").arg(daemon_pid.to_string()).status();
    let _ = fs::remove_dir_all(dir);
    reason
}

fn read_child_stderr(child: &mut Child) -> String {
    use std::io::Read as _;
    let mut text = String::new();
    if let Some(stderr) = child.stderr.as_mut() {
        let _ = stderr.read_to_string(&mut text);
    }
    text.trim().to_owned()
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Та же проба, но с явным PATH — используется в `p12` для проверки skip-ветки.
fn command_exists_in_path(path: &str, name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("PATH={path} command -v {name}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn python_dbus_available() -> bool {
    Command::new("python3")
        .args(["-c", "import dbus, gi"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Готовность окружения без запуска стенда: причина skip или `Ok(())`.
fn environment_ready() -> Result<(), String> {
    if !command_exists("dbus-daemon") {
        return Err(SKIP_NO_DAEMON.to_owned());
    }
    if !python_dbus_available() {
        return Err(SKIP_NO_PYTHON.to_owned());
    }
    Ok(())
}

fn unique_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (COUNTER.fetch_add(1, Ordering::Relaxed) << 32)
}

/// Запускает стенд или печатает причину skip и выходит из теста.
macro_rules! harness {
    ($extra:expr) => {
        match Harness::start($extra) {
            Ok(harness) => harness,
            Err(reason) => {
                println!("SKIP: {reason}");
                return;
            }
        }
    };
}

fn control_connection(harness: &Harness) -> Connection {
    Connection::connect(&harness.bus_address()).expect("управляющее соединение с моком")
}

fn call_control(connection: &mut Connection, member: &str, body: &[(char, Value)]) {
    connection
        .call(MOCK_DESTINATION, MOCK_PATH, MOCK_INTERFACE, member, body)
        .unwrap_or_else(|err| panic!("{member}: {err:?}"));
}

// --- JSON-фикстура (минимальный парсер, как в dbus_contract) ---------------------

#[derive(Debug)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn parse(text: &str) -> Result<Self, String> {
        let mut parser = JsonParser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let value = parser.value()?;
        parser.skip_ws();
        if parser.pos != parser.bytes.len() {
            return Err(format!("мусор после JSON на позиции {}", parser.pos));
        }
        Ok(value)
    }

    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Num(value) if *value >= 0.0 => Some(*value as u64),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Self::Arr(items) => Some(items),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl JsonParser<'_> {
    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "ожидался {:?} на позиции {}",
                byte as char, self.pos
            ))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.bytes.get(self.pos).copied() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => self.number(),
            other => Err(format!(
                "неожиданный байт {other:?} на позиции {}",
                self.pos
            )),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(value)
        } else {
            Err(format!("ожидался литерал {text} на позиции {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while matches!(self.bytes.get(self.pos), Some(byte) if byte.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            while matches!(self.bytes.get(self.pos), Some(byte) if byte.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let text =
            std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|err| err.to_string())?;
        text.parse::<f64>()
            .map(Json::Num)
            .map_err(|err| format!("число {text:?}: {err}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&byte) = self.bytes.get(self.pos) else {
                return Err("незакрытая строка".to_owned());
            };
            self.pos += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(&escaped) = self.bytes.get(self.pos) else {
                        return Err("незакрытый escape".to_owned());
                    };
                    self.pos += 1;
                    out.push(match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        other => return Err(format!("escape \\{}", other as char)),
                    });
                }
                _ => out.push(byte as char),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                other => return Err(format!("ожидался ',' или ']', получено {other:?}")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Obj(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(entries));
                }
                other => return Err(format!("ожидался ',' или '}}', получено {other:?}")),
            }
        }
    }
}

fn load_fixture(name: &str) -> Json {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/dbus")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("фикстура {}: {err}", path.display()));
    Json::parse(&text).unwrap_or_else(|err| panic!("JSON {}: {err}", path.display()))
}

fn fixture_service<'a>(golden: &'a Json, destination: &str) -> &'a Json {
    golden
        .get("services")
        .and_then(Json::as_arr)
        .expect("power.json: services")
        .iter()
        .find(|service| service.get("destination").and_then(Json::as_str) == Some(destination))
        .unwrap_or_else(|| panic!("power.json: нет сервиса {destination}"))
}

fn fixture_method<'a>(service: &'a Json, member: &str) -> &'a Json {
    service
        .get("methods")
        .and_then(Json::as_arr)
        .expect("power.json: methods")
        .iter()
        .find(|method| method.get("member").and_then(Json::as_str) == Some(member))
        .unwrap_or_else(|| panic!("power.json: нет метода {member}"))
}

// --- p01: границы IdlePolicy ------------------------------------------------------

#[test]
fn p01_idle_policy_boundaries() {
    let policy = IdlePolicy::DEFAULT;
    assert_eq!(policy.idle_after_secs, 600);
    assert_eq!(policy.dnd_after_secs, 1800);

    assert_eq!(policy.status(0), PresenceStatus::Online);
    assert_eq!(policy.status(599), PresenceStatus::Online);
    assert_eq!(policy.status(600), PresenceStatus::Idle);
    assert_eq!(policy.status(601), PresenceStatus::Idle);
    assert_eq!(policy.status(1799), PresenceStatus::Idle);
    assert_eq!(policy.status(1800), PresenceStatus::Dnd);
    assert_eq!(policy.status(u64::MAX), PresenceStatus::Dnd);

    // Нулевые пороги — «сразу idle/dnd», без переполнения и делений.
    let immediate_idle = IdlePolicy {
        idle_after_secs: 0,
        dnd_after_secs: 10,
    };
    assert_eq!(immediate_idle.status(0), PresenceStatus::Idle);
    let immediate_dnd = IdlePolicy {
        idle_after_secs: 0,
        dnd_after_secs: 0,
    };
    assert_eq!(immediate_dnd.status(0), PresenceStatus::Dnd);

    // idle_after == dnd_after → Dnd при `>=`.
    let equal = IdlePolicy {
        idle_after_secs: 5,
        dnd_after_secs: 5,
    };
    assert_eq!(equal.status(4), PresenceStatus::Online);
    assert_eq!(equal.status(5), PresenceStatus::Dnd);
}

// --- p02: Inhibit → cookie и поля доходят до сервиса ------------------------------

#[test]
fn p02_inhibit_returns_cookie_and_mock_sees_fields() {
    // Фикстура libdbus фиксирует адреса/сигнатуры, с которыми сверяется клиент.
    let golden = load_fixture("power.json");
    let screensaver = fixture_service(&golden, SCREENSAVER_DESTINATION);
    assert_eq!(
        screensaver.get("path").and_then(Json::as_str),
        Some(SCREENSAVER_PATH)
    );
    assert_eq!(
        screensaver.get("interface").and_then(Json::as_str),
        Some(SCREENSAVER_INTERFACE)
    );
    let inhibit = fixture_method(screensaver, "Inhibit");
    assert_eq!(inhibit.get("signature").and_then(Json::as_str), Some("ss"));
    assert_eq!(
        inhibit.get("reply_signature").and_then(Json::as_str),
        Some("u")
    );
    assert_eq!(inhibit.get("reply").and_then(Json::as_u64), Some(41));

    let harness = harness!(&[]);
    let mut screen_saver =
        ScreenSaver::connect(&harness.bus_address()).expect("ScreenSaver::connect");

    let cookie = screen_saver
        .inhibit("Discord Lab", "In call")
        .expect("Inhibit с валидными полями");
    assert_eq!(cookie, 41, "cookie должен прийти из ответа мока");

    let records = harness.wait_for_member("Inhibit", 1);
    let fields = records.first().expect("журнал Inhibit пуст");
    assert_eq!(fields.len(), 4, "формат журнала: {fields:?}");
    assert_eq!(fields[1], "Discord Lab");
    assert_eq!(fields[2], "In call");
    assert_eq!(fields[3], "41");
}

// --- p03: UnInhibit, fallback Uninhibit и неизвестный cookie ----------------------

#[test]
fn p03_uninhibit_reaches_mock_and_unknown_cookie_is_remote_error() {
    let harness = harness!(&[]);
    let mut screen_saver =
        ScreenSaver::connect(&harness.bus_address()).expect("ScreenSaver::connect");

    let cookie = screen_saver
        .inhibit("Discord Lab", "In call")
        .expect("Inhibit");
    screen_saver.uninhibit(cookie).expect("UnInhibit");

    let records = harness.wait_for_member("UnInhibit", 1);
    assert_eq!(
        records.first().map(|fields| fields[1].as_str()),
        Some("41"),
        "каноническое имя метода — UnInhibit"
    );
    assert!(
        harness.journal_member("Uninhibit").is_empty(),
        "fallback не должен вызываться, если UnInhibit поддержан"
    );

    // Неизвестный cookie — ошибка сервера (не паника).
    match screen_saver.uninhibit(9999) {
        Err(DbusError::Remote { name, .. }) => {
            assert_eq!(
                name, "org.freedesktop.ScreenSaver.Error.InvalidCookie",
                "имя ошибки из журнала/ERROR-ответа"
            );
        }
        other => panic!("ожидался DbusError::Remote для неизвестного cookie, получено {other:?}"),
    }

    // Сервис знает только `Uninhibit`: клиент обязан попробовать алиас.
    let alias_harness = harness!(&["--force-uninhibit-alias"]);
    let mut alias_client =
        ScreenSaver::connect(&alias_harness.bus_address()).expect("ScreenSaver::connect (alias)");
    let alias_cookie = alias_client
        .inhibit("Discord Lab", "In call")
        .expect("Inhibit (alias)");
    alias_client
        .uninhibit(alias_cookie)
        .expect("fallback Uninhibit после UnknownMethod");
    assert_eq!(
        alias_harness.wait_for_member("Uninhibit", 1).len(),
        1,
        "fallback-метод должен дойти до сервиса"
    );
}

// --- p04: IsActive ----------------------------------------------------------------

#[test]
fn p04_is_active_reflects_mock_state() {
    let harness = harness!(&["--active"]);
    let mut screen_saver =
        ScreenSaver::connect(&harness.bus_address()).expect("ScreenSaver::connect");

    assert!(
        screen_saver.is_active().expect("IsActive=true"),
        "мок стартует активным"
    );
    let mut control = control_connection(&harness);
    call_control(&mut control, "SetActive", &[('b', Value::Bool(false))]);
    assert!(
        !screen_saver.is_active().expect("IsActive=false"),
        "значение после SetActive(false)"
    );
    assert_eq!(harness.wait_for_member("IsActive", 2).len(), 2);
}

// --- p05: GetSessionIdleTime: Some / UnknownMethod / UnknownInterface / Err --------

#[test]
fn p05_idle_seconds_handles_unsupported_and_failing_services() {
    let harness = harness!(&["--idle-seconds", "1234"]);
    let mut screen_saver =
        ScreenSaver::connect(&harness.bus_address()).expect("ScreenSaver::connect");

    assert_eq!(
        screen_saver.idle_seconds().expect("GetSessionIdleTime"),
        Some(1234)
    );

    let mut control = control_connection(&harness);
    call_control(
        &mut control,
        "SetIdleMode",
        &[('s', Value::String("unknown-method".to_owned()))],
    );
    assert_eq!(
        screen_saver.idle_seconds().expect("UnknownMethod → None"),
        None,
        "отсутствующий метод — не ошибка"
    );

    call_control(
        &mut control,
        "SetIdleMode",
        &[('s', Value::String("unknown-interface".to_owned()))],
    );
    assert_eq!(
        screen_saver
            .idle_seconds()
            .expect("UnknownInterface → None"),
        None,
        "отсутствующий интерфейс — не ошибка"
    );

    call_control(
        &mut control,
        "SetIdleMode",
        &[('s', Value::String("error".to_owned()))],
    );
    match screen_saver.idle_seconds() {
        Err(DbusError::Remote { name, .. }) => {
            assert_eq!(name, "org.freedesktop.ScreenSaver.Error.MockFailure");
        }
        other => panic!("прочие ошибки обязаны быть Err, получено {other:?}"),
    }
}

// --- p06: PrepareForSleep(true/false) ---------------------------------------------

#[test]
fn p06_prepare_for_sleep_maps_to_events() {
    // Фикстура libdbus: адрес менеджера и сигнатура сигнала совпадают с клиентом.
    let golden = load_fixture("power.json");
    let login1 = fixture_service(&golden, LOGIN1_DESTINATION);
    assert_eq!(login1.get("path").and_then(Json::as_str), Some(LOGIN1_PATH));
    assert_eq!(
        login1.get("interface").and_then(Json::as_str),
        Some(LOGIN1_INTERFACE)
    );
    let signals = login1
        .get("signals")
        .and_then(Json::as_arr)
        .expect("power.json: signals");
    assert!(
        signals.iter().any(|signal| {
            signal.get("member").and_then(Json::as_str) == Some(SLEEP_SIGNAL)
                && signal.get("signature").and_then(Json::as_str) == Some("b")
                && signal
                    .get("body")
                    .and_then(Json::as_arr)
                    .and_then(|body| body.first())
                    .and_then(Json::as_bool)
                    == Some(true)
        }),
        "power.json: PrepareForSleep(b=true)"
    );

    let harness = harness!(&[]);
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");
    monitor.subscribe().expect("AddMatch PrepareForSleep");
    let mut control = control_connection(&harness);

    call_control(
        &mut control,
        "EmitPrepareForSleep",
        &[('b', Value::Bool(true))],
    );
    assert_eq!(
        monitor
            .next_event(Duration::from_secs(5))
            .expect("true → GoingToSleep"),
        Some(PowerEvent::GoingToSleep)
    );

    call_control(
        &mut control,
        "EmitPrepareForSleep",
        &[('b', Value::Bool(false))],
    );
    assert_eq!(
        monitor
            .next_event(Duration::from_secs(5))
            .expect("false → WakingUp"),
        Some(PowerEvent::WakingUp)
    );

    assert_eq!(harness.wait_for_member("EmitPrepareForSleep", 2).len(), 2);
}

// --- p07: чужие сигналы и битое тело пропускаются ---------------------------------

#[test]
fn p07_foreign_signals_are_skipped() {
    let harness = harness!(&[]);
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");
    monitor.subscribe().expect("AddMatch");
    let mut control = control_connection(&harness);

    // EmitForeign шлёт сигнал другого интерфейса и PrepareForSleep с телом `s`.
    call_control(&mut control, "EmitForeign", &[]);
    // После мусора первым валидным событием обязан стать WakingUp, а не GoingToSleep.
    call_control(
        &mut control,
        "EmitPrepareForSleep",
        &[('b', Value::Bool(false))],
    );
    assert_eq!(
        monitor
            .next_event(Duration::from_secs(5))
            .expect("валидное событие после чужих"),
        Some(PowerEvent::WakingUp)
    );

    call_control(
        &mut control,
        "EmitPrepareForSleep",
        &[('b', Value::Bool(true))],
    );
    assert_eq!(
        monitor
            .next_event(Duration::from_secs(5))
            .expect("следующее событие"),
        Some(PowerEvent::GoingToSleep)
    );
    assert_eq!(harness.journal_member("EmitForeign").len(), 1);
}

// --- p08: таймаут next_event ------------------------------------------------------

#[test]
fn p08_next_event_timeout_is_none() {
    let harness = harness!(&[]);
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");
    monitor.subscribe().expect("AddMatch");

    let started = Instant::now();
    assert_eq!(
        monitor
            .next_event(Duration::from_millis(150))
            .expect("таймаут — не ошибка"),
        None
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "таймаут должен срабатывать по переданному Duration"
    );
}

// --- p09: пустые app_name/reason --------------------------------------------------

#[test]
fn p09_inhibit_requires_app_name_and_reason() {
    let harness = harness!(&[]);
    let mut screen_saver =
        ScreenSaver::connect(&harness.bus_address()).expect("ScreenSaver::connect");

    assert!(matches!(
        screen_saver.inhibit("", "In call"),
        Err(DbusError::Protocol(_))
    ));
    assert!(matches!(
        screen_saver.inhibit("Discord Lab", ""),
        Err(DbusError::Protocol(_))
    ));
    assert!(matches!(
        screen_saver.inhibit("", ""),
        Err(DbusError::Protocol(_))
    ));
    assert!(
        harness.journal_member("Inhibit").is_empty(),
        "невалидный Inhibit не должен уходить на шину"
    );
}

// --- p10: закрытое соединение — ошибка, без паники --------------------------------

#[test]
fn p10_closed_connection_returns_error_without_panic() {
    let mut harness = harness!(&[]);
    let mut screen_saver =
        ScreenSaver::connect(&harness.bus_address()).expect("ScreenSaver::connect");
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");
    monitor.subscribe().expect("AddMatch");

    harness.kill_daemon();

    match screen_saver.is_active() {
        Err(DbusError::Io(_)) | Err(DbusError::Protocol(_)) | Err(DbusError::Timeout) => {}
        other => panic!("закрытая сессия обязана дать DbusError, получено {other:?}"),
    }
    match monitor.next_event(Duration::from_millis(100)) {
        Err(DbusError::Io(_)) | Err(DbusError::Protocol(_)) | Err(DbusError::Timeout) => {}
        // Гонка: шина успела закрыться так, что сигналов нет — тоже допустимый исход.
        Ok(None) => {}
        other => panic!("закрытая сессия обязана дать DbusError/None, получено {other:?}"),
    }
}

// --- p11: live-сервис — только чтение и только по флагу ---------------------------

#[test]
fn p11_live_service_is_flag_gated_and_read_only() {
    if std::env::var("DISCORD_LAB_LIVE_POWER").as_deref() != Ok("1") {
        println!("SKIP: live-питание выключено (DISCORD_LAB_LIVE_POWER != 1)");
        return;
    }
    let Some(address) = BusAddress::from_env() else {
        println!("SKIP: нет DBUS_SESSION_BUS_ADDRESS");
        return;
    };
    let mut screen_saver = match ScreenSaver::connect(&address) {
        Ok(client) => client,
        Err(err) => {
            println!("SKIP: live ScreenSaver недоступен: {err:?}");
            return;
        }
    };
    // Только чтение: Inhibit на живом рабочем столе не вызывается.
    match screen_saver.is_active() {
        Ok(active) => println!("live IsActive: {active}"),
        Err(err) => println!("SKIP: live IsActive: {err:?}"),
    }
    match screen_saver.idle_seconds() {
        Ok(idle) => println!("live idle: {idle:?}"),
        Err(err) => println!("SKIP: live GetSessionIdleTime: {err:?}"),
    }
}

// --- p12: нет dbus-daemon/python-dbus → skip с причиной ---------------------------

#[test]
fn p12_missing_environment_is_skip_with_reason() {
    // Проба с заведомо пустым PATH: dbus-daemon не должен «находиться».
    assert!(
        !command_exists_in_path("/nonexistent", "dbus-daemon"),
        "пустой PATH не должен давать dbus-daemon"
    );

    match environment_ready() {
        Ok(()) => {
            assert!(command_exists("dbus-daemon"), "окружение объявлено готовым");
            assert!(python_dbus_available(), "окружение объявлено готовым");
        }
        Err(reason) => {
            // Именно этот путь срабатывает в CI без python3-dbus: тест не падает.
            println!("SKIP: {reason}");
        }
    }
}

// --- W35a (P6-06c): sleep-inhibitor через fd (`Inhibit(h)`) -----------------------
//
// Оракул — libdbus-мок `waves/RW-35/power/mock_logind.py` на приватном `dbus-daemon`:
// `org.freedesktop.login1.Manager.Inhibit(ssss) -> h` отдаёт настоящий memfd, журналирует
// аргументы и умеет отвечать ERROR (`error`) или «не смог выделить fd» (`no-fd`).
// Валидность fd проверяется через `/proc/self/fd`, утечки — по числу открытых fd.

const LOGIND_CONTROL_DESTINATION: &str = "lab.Mock.Login1";
const LOGIND_CONTROL_PATH: &str = "/lab/Mock";
const LOGIND_CONTROL_INTERFACE: &str = "lab.Mock.Login1";

/// Приватная шина + мок logind; всё убирается в `Drop`.
struct LogindHarness {
    address: String,
    journal: PathBuf,
    dir: PathBuf,
    daemon_pid: u32,
    mock: Child,
}

impl LogindHarness {
    fn start(inhibit_mode: &str) -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(SKIP_NO_PYTHON.to_owned());
        }

        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w35a-power-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("tempdir {}: {err}", dir.display()))?;

        let started = Command::new("dbus-daemon")
            .args(["--session", "--print-address=1", "--print-pid=1", "--fork"])
            .output()
            .map_err(|err| format!("запуск dbus-daemon: {err}"))?;
        if !started.status.success() {
            let _ = fs::remove_dir_all(&dir);
            return Err(format!(
                "dbus-daemon завершился: {}",
                String::from_utf8_lossy(&started.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&started.stdout).into_owned();
        let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
        let address = lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал адрес".to_owned())?
            .trim()
            .to_owned();
        let daemon_pid: u32 = match lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал pid".to_owned())
            .and_then(|line| {
                line.trim()
                    .parse()
                    .map_err(|err| format!("pid dbus-daemon: {err}"))
            }) {
            Ok(pid) => pid,
            Err(reason) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(reason);
            }
        };

        let journal = dir.join("journal.tsv");
        let mut mock = Command::new("python3")
            .arg(repo_path("waves/RW-35/power/mock_logind.py"))
            .arg("--address")
            .arg(&address)
            .arg("--log")
            .arg(&journal)
            .arg("--inhibit-mode")
            .arg(inhibit_mode)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| abort(daemon_pid, &dir, format!("запуск mock_logind.py: {err}")))?;

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if fs::read_to_string(&journal)
                .map(|text| text.contains("READY"))
                .unwrap_or(false)
            {
                break;
            }
            if let Ok(Some(status)) = mock.try_wait() {
                let stderr = read_child_stderr(&mut mock);
                return Err(abort(
                    daemon_pid,
                    &dir,
                    format!("mock_logind.py завершился ({status}): {stderr}"),
                ));
            }
            if Instant::now() > deadline {
                return Err(abort(
                    daemon_pid,
                    &dir,
                    "mock_logind.py не поднялся за 10 с".to_owned(),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Ok(Self {
            address,
            journal,
            dir,
            daemon_pid,
            mock,
        })
    }

    fn bus_address(&self) -> BusAddress {
        BusAddress::parse(&self.address).expect("адрес приватного демона")
    }

    fn journal_member(&self, member: &str) -> Vec<Vec<String>> {
        fs::read_to_string(&self.journal)
            .unwrap_or_default()
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<_>>())
            .filter(|fields| fields.first().map(String::as_str) == Some(member))
            .collect()
    }

    fn wait_for_member(&self, member: &str, count: usize) -> Vec<Vec<String>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records = self.journal_member(member);
            if records.len() >= count || Instant::now() > deadline {
                return records;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Переключает режим мока через управляющее соединение (оракул, не наш клиент).
    fn set_inhibit_mode(&self, mode: &str) {
        let mut control =
            Connection::connect(&self.bus_address()).expect("управляющее соединение с моком");
        control
            .call(
                LOGIND_CONTROL_DESTINATION,
                LOGIND_CONTROL_PATH,
                LOGIND_CONTROL_INTERFACE,
                "SetInhibitMode",
                &[('s', Value::String(mode.to_owned()))],
            )
            .unwrap_or_else(|err| panic!("SetInhibitMode({mode}): {err:?}"));
    }
}

impl Drop for LogindHarness {
    fn drop(&mut self) {
        let _ = self.mock.kill();
        let _ = self.mock.wait();
        if self.daemon_pid != 0 {
            let _ = Command::new("kill")
                .arg(self.daemon_pid.to_string())
                .status();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

macro_rules! logind_harness {
    ($mode:expr) => {
        match LogindHarness::start($mode) {
            Ok(harness) => harness,
            Err(reason) => {
                println!("SKIP: {reason}");
                return;
            }
        }
    };
}

/// Число открытых fd, чей `/proc/self/fd`-линк содержит `needle`.
///
/// Счёт по содержимому линка, а не по всему `/proc/self/fd`: тесты бинарника идут
/// параллельно и открывают свои сокеты, общий счётчик был бы недетерминирован.
/// Сериализует тесты, которые считают fd по общим именам memfd мока.
static INHIBIT_FD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn inhibit_fd_lock() -> std::sync::MutexGuard<'static, ()> {
    INHIBIT_FD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn count_fds_matching(needle: &str) -> usize {
    fs::read_dir("/proc/self/fd")
        .map(|entries| {
            entries
                .filter_map(|entry| fs::read_link(entry.ok()?.path()).ok())
                .filter(|target| target.to_string_lossy().contains(needle))
                .count()
        })
        .unwrap_or(0)
}

fn fd_is_open(fd: RawFd) -> bool {
    Path::new(&format!("/proc/self/fd/{fd}")).exists()
}

fn fd_link(fd: RawFd) -> String {
    fs::read_link(format!("/proc/self/fd/{fd}"))
        .map(|target| target.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// --- f02: mock Inhibit → fd и аргументы в журнале ---------------------------------

#[test]
fn f02_inhibit_sleep_returns_fd_and_journals_arguments() {
    let _fd_guard = inhibit_fd_lock();
    let harness = logind_harness!("ok");
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");

    let inhibitor = monitor
        .inhibit_sleep("In call")
        .expect("Inhibit(sleep, discord-lab, In call, block)");
    let raw = inhibitor.fd();
    assert!(raw >= 0, "fd ингибитора: {raw}");
    assert!(fd_is_open(raw), "fd {raw} не открыт");
    assert!(
        fd_link(raw).contains("memfd:discord-lab-inhibit"),
        "fd {raw} — не memfd мока: {:?}",
        fd_link(raw)
    );

    let records = harness.wait_for_member("Inhibit", 1);
    let fields = records.first().expect("журнал Inhibit пуст");
    assert_eq!(
        fields,
        &vec![
            "Inhibit".to_owned(),
            "sleep".to_owned(),
            "discord-lab".to_owned(),
            "In call".to_owned(),
            "block".to_owned(),
        ],
        "аргументы Inhibit в журнале оракула"
    );

    // ERROR от сервиса — ошибка, не паника и не «пустой» ингибитор.
    harness.set_inhibit_mode("error");
    match monitor.inhibit_sleep("In call") {
        Err(DbusError::Remote { name, .. }) => assert_eq!(
            name, "org.freedesktop.login1.NotAuthorized",
            "имя ошибки из ответа мока"
        ),
        other => panic!("ожидался DbusError::Remote, получено {other:?}"),
    }

    // Сервис не смог отдать fd — тоже ошибка; fd не появился.
    harness.set_inhibit_mode("no-fd");
    assert!(
        matches!(
            monitor.inhibit_sleep("In call"),
            Err(DbusError::Remote { .. })
        ),
        "ответ без fd обязан быть ошибкой"
    );
    assert_eq!(harness.wait_for_member("Inhibit", 3).len(), 3);
}

// --- f03: Drop закрывает fd ровно один раз ---------------------------------------

#[test]
fn f03_drop_closes_inhibitor_fd_exactly_once() {
    let _fd_guard = inhibit_fd_lock();
    let harness = logind_harness!("ok");
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");

    assert_eq!(count_fds_matching("discord-lab-inhibit"), 0);
    let inhibitor: SleepInhibitor = monitor.inhibit_sleep("In call").expect("Inhibit");
    let raw = inhibitor.fd();
    assert_eq!(
        count_fds_matching("discord-lab-inhibit"),
        1,
        "ингибитор держит ровно один fd"
    );
    assert!(fd_is_open(raw), "fd {raw} должен быть открыт");

    drop(inhibitor);
    assert_eq!(
        count_fds_matching("discord-lab-inhibit"),
        0,
        "Drop обязан закрыть fd (close ровно один раз)"
    );
    assert!(!fd_is_open(raw), "fd {raw} остался открыт после Drop");
}

// --- f08: пустой why → Protocol, без вызова ---------------------------------------

#[test]
fn f08_empty_why_is_protocol_and_never_reaches_bus() {
    let harness = logind_harness!("ok");
    let mut monitor = PowerMonitor::connect(&harness.bus_address()).expect("PowerMonitor::connect");

    assert!(
        matches!(monitor.inhibit_sleep(""), Err(DbusError::Protocol(_))),
        "пустой why обязан быть Protocol"
    );
    assert!(
        harness.journal_member("Inhibit").is_empty(),
        "невалидный Inhibit не должен уходить на шину"
    );
}

// --- f10: live logind — только по флагу и skip-aware ------------------------------

#[test]
fn f10_live_logind_inhibit_is_flag_gated_and_skip_aware() {
    if std::env::var("DISCORD_LAB_POWER_LIVE").as_deref() != Ok("1") {
        println!("SKIP: live logind выключен (DISCORD_LAB_POWER_LIVE != 1)");
        return;
    }
    let value = std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
        .unwrap_or_else(|_| "unix:path=/run/dbus/system_bus_socket".to_owned());
    let Some(address) = BusAddress::parse(&value) else {
        println!("SKIP: адрес системной шины не разобран: {value:?}");
        return;
    };
    let mut monitor = match PowerMonitor::connect(&address) {
        Ok(monitor) => monitor,
        Err(err) => {
            println!("SKIP: live logind недоступен: {err:?}");
            return;
        }
    };
    match monitor.inhibit_sleep("discord-lab live check") {
        Ok(inhibitor) => {
            let raw = inhibitor.fd();
            assert!(raw >= 0, "live fd ингибитора: {raw}");
            assert!(
                Path::new(&format!("/proc/self/fd/{raw}")).exists(),
                "live fd {raw} не открыт"
            );
            println!("INFO: live logind Inhibit: fd={raw}");
            drop(inhibitor);
        }
        Err(err) => {
            // Live-окружение без logind/прав — skip, а не падение.
            println!("SKIP: live Inhibit недоступен: {err:?}");
        }
    }
}
