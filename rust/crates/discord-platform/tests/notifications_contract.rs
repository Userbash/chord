//! Контракт P6-02: `org.freedesktop.Notifications` через собственный D-Bus-клиент.
//!
//! Независимый оракул — приватный `dbus-daemon` (`--session --print-address --fork`) и
//! мок-сервис `waves/RW-30/dbus/mock_service.py` на **libdbus** (`python3-dbus`), который
//! отвечает на вызовы, пишет журнал (TSV) и эмитит сигналы. Rust-код здесь — только клиент,
//! поэтому совпадение поведения не может быть «подогнано» под свою же реализацию.
//!
//! Skip-aware: нет `dbus-daemon`, `python3-dbus`/`gi` или мок не поднялся → тест печатает
//! `SKIP: <причина>` и завершается успешно (окружение недоступно, а не падение).
//!
//! Live-сервис на сессионной шине не трогается: `n16` только читает возможности и лишь при
//! `DISCORD_LAB_LIVE_NOTIFY=1`; уведомления наружу не отправляются.

use discord_platform::dbus::{BusAddress, Connection, DbusError, Value};
use discord_platform::notifications::{Hints, Notification, NotificationEvent, Notifier, Urgency};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SKIP_NO_DAEMON: &str = "нет dbus-daemon";
const SKIP_NO_PYTHON: &str = "нет python3 с модулями dbus и gi";
const MOCK_DESTINATION: &str = "lab.Mock";
const MOCK_PATH: &str = "/lab/Mock";
const MOCK_INTERFACE: &str = "lab.Mock.Notifications";

/// Приватная шина + мок-сервис; всё убирается в `Drop`.
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
            "discord-lab-w30b-{}-{}",
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
            .arg(repo_path("waves/RW-30/dbus/mock_service.py"))
            .arg("--address")
            .arg(&address)
            .arg("--log")
            .arg(&journal)
            .args(extra_mock_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| abort(daemon_pid, &dir, format!("запуск mock_service.py: {err}")))?;

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
                    format!("мок-сервис завершился ({status}): {stderr}"),
                ));
            }
            if Instant::now() > deadline {
                return Err(abort(
                    daemon_pid,
                    &dir,
                    "мок-сервис не поднялся за 10 с".to_owned(),
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

/// Убирает приватный демон и tempdir при ошибке старта (skip-путь без `Drop`).
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

fn python_dbus_available() -> bool {
    Command::new("python3")
        .args(["-c", "import dbus, gi"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
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

fn sample_notification() -> Notification {
    Notification {
        app_name: "Discord Lab".to_owned(),
        summary: "Alice".to_owned(),
        body: "hello".to_owned(),
        // -1 — «таймаут сервера по умолчанию» (derive(Default) даёт 0, поэтому явно).
        timeout_ms: -1,
        ..Notification::default()
    }
}

// --- n02/n06: auth EXTERNAL + Hello ----------------------------------------------

#[test]
fn n02_auth_external_and_hello_yield_unique_name() {
    let harness = harness!(&[]);
    let connection = Connection::connect(&harness.bus_address())
        .unwrap_or_else(|err| panic!("connect + Hello: {err:?}"));

    let name = connection.unique_name();
    assert!(name.starts_with(":1."), "уникальное имя: {name:?}");
    assert!(!name.is_empty(), "уникальное имя пустое");
}

// --- n07: GetCapabilities и GetServerInformation ---------------------------------

#[test]
fn n07_capabilities_and_server_information_come_from_mock() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    assert_eq!(
        notifier.capabilities().expect("GetCapabilities"),
        vec![
            "body".to_owned(),
            "actions".to_owned(),
            "persistence".to_owned(),
            "icon-static".to_owned(),
        ]
    );
    assert_eq!(
        notifier.server_information().expect("GetServerInformation"),
        (
            "MockNotify".to_owned(),
            "discord-lab".to_owned(),
            "1.0".to_owned(),
            "1.2".to_owned(),
        )
    );

    assert_eq!(harness.journal_member("GetCapabilities").len(), 1);
    assert_eq!(harness.journal_member("GetServerInformation").len(), 1);
}

// --- n08: Notify → id, поля доходят до сервиса -----------------------------------

#[test]
fn n08_notify_returns_server_id_and_mock_sees_fields() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let id = notifier
        .notify(&sample_notification())
        .expect("Notify с валидными полями");
    assert_eq!(id, 100, "id должен прийти из ответа мока");

    let records = harness.wait_for_member("Notify", 1);
    let fields = records.first().expect("журнал Notify пуст");
    // Notify\tapp_name\treplaces\tapp_icon\tsummary\tbody\thints_len\tactions\ttimeout
    assert_eq!(fields.len(), 9, "формат журнала: {fields:?}");
    assert_eq!(fields[1], "Discord Lab");
    assert_eq!(fields[2], "0");
    assert_eq!(fields[4], "Alice");
    assert_eq!(fields[5], "hello");
    assert_eq!(fields[6], "0", "hints должен быть пустым a{{sv}}");
    assert_eq!(fields[8], "-1", "timeout по умолчанию");
}

// --- n09: actions парами ---------------------------------------------------------

#[test]
fn n09_notify_actions_reach_mock_as_pairs() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let notification = Notification {
        actions: vec![
            ("open".to_owned(), "Open".to_owned()),
            ("reply".to_owned(), "Reply".to_owned()),
        ],
        ..sample_notification()
    };
    notifier.notify(&notification).expect("Notify с actions");

    let records = harness.wait_for_member("Notify", 1);
    let fields = records.first().expect("журнал Notify пуст");
    // Спека freedesktop: `actions` на проводе — `as` парами id/label; модель — Vec<(id, label)>.
    assert_eq!(fields[7], "open:Open|reply:Reply");
}

// --- n10: CloseNotification ------------------------------------------------------

#[test]
fn n10_close_notification_reaches_mock() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let id = notifier.notify(&sample_notification()).expect("Notify");
    notifier.close(id).expect("CloseNotification");

    let records = harness.wait_for_member("CloseNotification", 1);
    let fields = records.first().expect("журнал CloseNotification пуст");
    assert_eq!(fields[1], id.to_string());
}

// --- n11: ERROR-ответ → DbusError::Remote ----------------------------------------

#[test]
fn n11_error_reply_maps_to_remote_error() {
    let harness = harness!(&["--fail-notify"]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let error = notifier
        .notify(&sample_notification())
        .expect_err("Notify обязан вернуть Remote");
    match error {
        DbusError::Remote { name, message } => {
            assert_eq!(
                name, "org.freedesktop.Notifications.Error.MockRejected",
                "имя ошибки из ERROR-ответа"
            );
            assert_eq!(message, "mock rejected notification");
        }
        other => panic!("ожидался DbusError::Remote, получено {other:?}"),
    }

    assert_eq!(
        harness.journal_member("Notify").len(),
        1,
        "сервис получил вызов до ошибки"
    );
}

// --- n12: ActionInvoked / NotificationClosed -------------------------------------

#[test]
fn n12_action_and_close_signals_become_events() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");
    notifier
        .listen_actions()
        .expect("AddMatch ActionInvoked/NotificationClosed");

    let id = notifier.notify(&sample_notification()).expect("Notify");
    let mut control = Connection::connect(&harness.bus_address()).expect("control connect");

    control
        .call(
            MOCK_DESTINATION,
            MOCK_PATH,
            MOCK_INTERFACE,
            "EmitAction",
            &[
                ('u', Value::U32(id)),
                ('s', Value::String("open".to_owned())),
            ],
        )
        .expect("EmitAction(id, open)");
    assert_eq!(
        notifier.next_event().expect("ActionInvoked"),
        NotificationEvent::Action {
            id,
            action: "open".to_owned(),
        }
    );

    control
        .call(
            MOCK_DESTINATION,
            MOCK_PATH,
            MOCK_INTERFACE,
            "EmitClosed",
            &[('u', Value::U32(id)), ('u', Value::U32(2))],
        )
        .expect("EmitClosed(id, 2)");
    assert_eq!(
        notifier.next_event().expect("NotificationClosed"),
        NotificationEvent::Closed { id, reason: 2 }
    );
}

// --- n13: чужие сигналы пропускаются ---------------------------------------------

#[test]
fn n13_foreign_signals_are_skipped() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");
    notifier.listen_actions().expect("AddMatch");

    let id = notifier.notify(&sample_notification()).expect("Notify");
    let mut control = Connection::connect(&harness.bus_address()).expect("control connect");

    // EmitForeign шлёт сигнал другого интерфейса и ActionInvoked с чужим телом `s`.
    control
        .call(
            MOCK_DESTINATION,
            MOCK_PATH,
            MOCK_INTERFACE,
            "EmitForeign",
            &[('u', Value::U32(id))],
        )
        .expect("EmitForeign");
    control
        .call(
            MOCK_DESTINATION,
            MOCK_PATH,
            MOCK_INTERFACE,
            "EmitAction",
            &[
                ('u', Value::U32(id)),
                ('s', Value::String("reply".to_owned())),
            ],
        )
        .expect("EmitAction(id, reply)");

    // Битый ActionInvoked и чужой интерфейс не должны стать событием или ошибкой.
    assert_eq!(
        notifier
            .next_event()
            .expect("валидный ActionInvoked после чужих"),
        NotificationEvent::Action {
            id,
            action: "reply".to_owned(),
        }
    );
    assert_eq!(harness.journal_member("EmitForeign").len(), 1);
}

// --- n15: повторный close и закрытая сессия --------------------------------------

#[test]
fn n15_repeated_close_and_closed_session_survive() {
    let mut harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let id = notifier.notify(&sample_notification()).expect("Notify");
    notifier.close(id).expect("первый CloseNotification");
    notifier.close(id).expect("повторный CloseNotification");
    assert_eq!(
        harness.wait_for_member("CloseNotification", 2).len(),
        2,
        "сервис должен получить оба CloseNotification"
    );

    harness.kill_daemon();
    match notifier.close(id) {
        Ok(()) => {}
        Err(DbusError::Io(_)) | Err(DbusError::Protocol(_)) | Err(DbusError::Timeout) => {}
        Err(other) => panic!("неожиданная ошибка закрытой сессии: {other:?}"),
    }
}

// --- защита от пустых обязательных полей -----------------------------------------

#[test]
fn n08b_notify_requires_app_name_and_summary() {
    let harness = harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let empty_app = Notification {
        app_name: String::new(),
        ..sample_notification()
    };
    let empty_summary = Notification {
        summary: String::new(),
        ..sample_notification()
    };

    assert!(matches!(
        notifier.notify(&empty_app),
        Err(DbusError::Protocol(_))
    ));
    assert!(matches!(
        notifier.notify(&empty_summary),
        Err(DbusError::Protocol(_))
    ));
    assert!(
        harness.journal_member("Notify").is_empty(),
        "невалидный Notify не должен уходить на шину"
    );
}

// --- n16: live-сервис — только чтение и только по флагу --------------------------

#[test]
fn n16_live_service_is_flag_gated_and_read_only() {
    if std::env::var("DISCORD_LAB_LIVE_NOTIFY").as_deref() != Ok("1") {
        println!("SKIP: live-уведомления выключены (DISCORD_LAB_LIVE_NOTIFY != 1)");
        return;
    }
    let Some(address) = BusAddress::from_env() else {
        println!("SKIP: нет DBUS_SESSION_BUS_ADDRESS");
        return;
    };
    let mut notifier = match Notifier::connect(&address) {
        Ok(notifier) => notifier,
        Err(err) => {
            println!("SKIP: live-сервис недоступен: {err:?}");
            return;
        }
    };
    // Уведомление не отправляется даже с флагом: тест проверяет только доступность сервиса.
    match notifier.capabilities() {
        Ok(capabilities) => println!("live capabilities: {capabilities:?}"),
        Err(err) => println!("SKIP: GetCapabilities на live-шине: {err:?}"),
    }
}

// =================================================================================
// W32b (P6-02b): hints `a{sv}`. Оракул — `waves/RW-32/dbus/mock_hints.py` на libdbus:
// журнал `Notify` пишет `ключ:сигнатура:значение` в порядке провода, поэтому неверный
// тип варианта, ключ, пропуск пустой строки или порядок видны как расхождение строки.
// =================================================================================

const HINTS_SKIP_NO_DAEMON: &str = "нет dbus-daemon";
const HINTS_SKIP_NO_PYTHON: &str = "нет python3 с модулями dbus и gi";

/// Приватная шина + мок hints; отличается от `Harness` только скриптом мока
/// (`waves/RW-32/dbus/mock_hints.py`) и префиксом временного каталога.
struct HintsHarness {
    address: String,
    journal: PathBuf,
    dir: PathBuf,
    daemon_pid: u32,
    mock: Child,
}

impl HintsHarness {
    fn start(extra_mock_args: &[&str]) -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(HINTS_SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(HINTS_SKIP_NO_PYTHON.to_owned());
        }

        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w32b-{}-{}",
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
            .arg(repo_path("waves/RW-32/dbus/mock_hints.py"))
            .arg("--address")
            .arg(&address)
            .arg("--log")
            .arg(&journal)
            .args(extra_mock_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| abort(daemon_pid, &dir, format!("запуск mock_hints.py: {err}")))?;

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
                    format!("мок-сервис завершился ({status}): {stderr}"),
                ));
            }
            if Instant::now() > deadline {
                return Err(abort(
                    daemon_pid,
                    &dir,
                    "мок-сервис не поднялся за 10 с".to_owned(),
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
            .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<String>>())
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
}

impl Drop for HintsHarness {
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

macro_rules! hints_harness {
    ($extra:expr) => {
        match HintsHarness::start($extra) {
            Ok(harness) => harness,
            Err(reason) => {
                println!("SKIP: {reason}");
                return;
            }
        }
    };
}

/// Запись журнала `Notify` по индексу (ждёт, пока мок допишет).
fn hints_notify_record(harness: &HintsHarness, index: usize) -> Vec<String> {
    let records = harness.wait_for_member("Notify", index + 1);
    records.get(index).cloned().unwrap_or_else(|| {
        panic!(
            "журнал Notify: ожидалась запись #{index}, есть {}",
            records.len()
        )
    })
}

/// Поле `hints` журнала: `ключ:сигнатура:значение|…` (пусто для пустого `a{sv}`).
fn hints_field(record: &[String]) -> &str {
    record.get(9).map(String::as_str).unwrap_or("")
}

// --- q01: notify без hints → пустой a{sv} (0 записей, не null) --------------------

#[test]
fn q01_notify_without_hints_sends_empty_dict() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let id = notifier
        .notify(&sample_notification())
        .expect("Notify без hints");
    assert!(id >= 100, "id из ответа мока: {id}");

    let first = hints_notify_record(&harness, 0);
    // Notify\tapp\treplaces\tapp_icon\tsummary\tbody\thints_len\tactions\ttimeout\thints
    assert_eq!(first.len(), 10, "формат журнала: {first:?}");
    assert_eq!(first[6], "0", "hints_len: пустой a{{sv}}");
    assert_eq!(hints_field(&first), "", "a{{sv}} пустой, а не null");

    // `notify` — ровно `notify_with_hints(..., Hints::default())`.
    notifier
        .notify_with_hints(&sample_notification(), &Hints::default())
        .expect("Notify с Hints::default()");
    let second = hints_notify_record(&harness, 1);
    assert_eq!(second[6], "0", "Hints::default() — тоже 0 записей");
    assert_eq!(hints_field(&second), "");
}

// --- q02: urgency → вариант y (0/1/2) --------------------------------------------

#[test]
fn q02_urgency_is_byte_variant_with_contract_values() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    for urgency in [Urgency::Low, Urgency::Normal, Urgency::Critical] {
        notifier
            .notify_with_hints(
                &sample_notification(),
                &Hints {
                    urgency: Some(urgency),
                    ..Hints::default()
                },
            )
            .expect("Notify с urgency");
    }

    for (index, code) in ["0", "1", "2"].iter().enumerate() {
        let record = hints_notify_record(&harness, index);
        assert_eq!(
            hints_field(&record),
            format!("urgency:y:{code}"),
            "Low/Normal/Critical → байты 0/1/2 в варианте `y`"
        );
    }
}

// --- q03: category → вариант s ---------------------------------------------------

#[test]
fn q03_category_is_string_variant() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    notifier
        .notify_with_hints(
            &sample_notification(),
            &Hints {
                category: Some("email.arrived".to_owned()),
                ..Hints::default()
            },
        )
        .expect("Notify с category");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "1");
    assert_eq!(hints_field(&record), "category:s:email.arrived");
}

// --- q04: desktop-entry + resident (s + b) ---------------------------------------

#[test]
fn q04_desktop_entry_and_resident_have_string_and_bool_variants() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    for resident in [true, false] {
        notifier
            .notify_with_hints(
                &sample_notification(),
                &Hints {
                    desktop_entry: Some("discord-lab".to_owned()),
                    resident: Some(resident),
                    ..Hints::default()
                },
            )
            .expect("Notify с desktop-entry/resident");
    }

    let first = hints_notify_record(&harness, 0);
    assert_eq!(
        hints_field(&first),
        "desktop-entry:s:discord-lab|resident:b:1"
    );
    let second = hints_notify_record(&harness, 1);
    assert_eq!(
        hints_field(&second),
        "desktop-entry:s:discord-lab|resident:b:0",
        "false — это b:0, а не отсутствие записи"
    );
}

// --- q05: все hints вместе → 4 записи в фиксированном порядке --------------------

#[test]
fn q05_all_hints_are_four_entries_in_frozen_order() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    notifier
        .notify_with_hints(
            &sample_notification(),
            &Hints {
                urgency: Some(Urgency::Normal),
                category: Some("im.received".to_owned()),
                desktop_entry: Some("discord-lab".to_owned()),
                resident: Some(true),
            },
        )
        .expect("Notify со всеми hints");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "4", "четыре записи a{{sv}}");
    assert_eq!(
        hints_field(&record),
        "urgency:y:1|category:s:im.received|desktop-entry:s:discord-lab|resident:b:1",
        "порядок: urgency, category, desktop-entry, resident"
    );
}

// --- q06: пустые строки не отправляются ------------------------------------------

#[test]
fn q06_empty_category_and_desktop_entry_are_skipped() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    notifier
        .notify_with_hints(
            &sample_notification(),
            &Hints {
                urgency: Some(Urgency::Critical),
                category: Some(String::new()),
                desktop_entry: Some(String::new()),
                resident: Some(true),
            },
        )
        .expect("Notify с пустыми строками");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "2", "пустые строки не должны стать записями");
    assert_eq!(hints_field(&record), "urgency:y:2|resident:b:1");
}

// --- q07: Unicode и длинная строка (4 КиБ) ---------------------------------------

#[test]
fn q07_unicode_and_long_values_are_delivered_without_panic() {
    let harness = hints_harness!(&[]);
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let long_ascii = "a".repeat(4096);
    let long_unicode = "я".repeat(2048); // 4096 байт в UTF-8
    let notification = Notification {
        app_name: "Discord Lab".to_owned(),
        summary: "Юникод 📬".to_owned(),
        body: "тело уведомления".to_owned(),
        timeout_ms: -1,
        ..Notification::default()
    };
    let hints = Hints {
        category: Some(long_ascii.clone()),
        desktop_entry: Some(long_unicode.clone()),
        ..Hints::default()
    };

    let id = notifier
        .notify_with_hints(&notification, &hints)
        .expect("длинные строки не должны ломать маршалинг");
    assert!(id >= 100, "id из ответа мока: {id}");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[4], "Юникод 📬", "summary дошёл без искажений");
    assert_eq!(record[5], "тело уведомления");
    assert_eq!(
        hints_field(&record),
        format!("category:s:{long_ascii}|desktop-entry:s:{long_unicode}")
    );
}
