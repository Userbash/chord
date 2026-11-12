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

use chord_platform::dbus::{BusAddress, Connection, DbusError, Value};
use chord_platform::notifications::{
    Event, Hints, IncomingMessageNotification, Notification, NotificationDispatchDecision,
    NotificationEvent, NotificationRouter, Notifier, Urgency, redact_secrets,
};
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
        Self::start_with_mock(
            "waves/RW-32/dbus/mock_hints.py",
            "discord-lab-w32b",
            extra_mock_args,
        )
    }

    /// Тот же стенд, но с другим скриптом мока (W34b использует копию
    /// `waves/RW-34/dbus/mock_hints2.py`); префикс tempdir отличает прогоны.
    fn start_with_mock(
        mock_script: &str,
        temp_prefix: &str,
        extra_mock_args: &[&str],
    ) -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(HINTS_SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(HINTS_SKIP_NO_PYTHON.to_owned());
        }

        let dir = std::env::temp_dir().join(format!(
            "{temp_prefix}-{}-{}",
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
            .arg(repo_path(mock_script))
            .arg("--address")
            .arg(&address)
            .arg("--log")
            .arg(&journal)
            .args(extra_mock_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| abort(daemon_pid, &dir, format!("запуск {mock_script}: {err}")))?;

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
                ..Hints::default()
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
                ..Hints::default()
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
// =================================================================================
// W34b (P6-02d): расширенные hints `a{sv}`. Оракул — копия libdbus-мока
// `waves/RW-34/dbus/mock_hints2.py` (журнал `ключ:сигнатура:значение` в порядке провода)
// и байты `Notify`, собранные libgio (`fixtures/dbus/hints-ext.json`). Rust-код в
// оракуле не участвует, поэтому неверный ключ, тип варианта, пропуск пустой строки или
// порядок записей видны как расхождение.
// =================================================================================

use chord_platform::dbus::message::{self, MessageType};
use chord_platform::notifications;

const EXT_MOCK: &str = "waves/RW-34/dbus/mock_hints2.py";
const EXT_TEMP_PREFIX: &str = "discord-lab-w34b";

macro_rules! ext_harness {
    () => {
        match HintsHarness::start_with_mock(EXT_MOCK, EXT_TEMP_PREFIX, &[]) {
            Ok(harness) => harness,
            Err(reason) => {
                println!("SKIP: {reason}");
                return;
            }
        }
    };
}

/// Полный набор из восьми hints; значения совпадают с набором `all_eight` фикстуры.
fn full_hints() -> Hints {
    Hints {
        urgency: Some(Urgency::Normal),
        category: Some("im.received".to_owned()),
        desktop_entry: Some("discord-lab".to_owned()),
        image_path: Some("/usr/share/icons/discord-lab.png".to_owned()),
        sound_name: Some("message-new-instant".to_owned()),
        resident: Some(true),
        transient: Some(true),
        suppress_sound: Some(true),
    }
}

/// Запись журнала `hints` → тройки (ключ, сигнатура варианта, значение).
fn hint_entries(field: &str) -> Vec<(&str, &str, &str)> {
    if field.is_empty() {
        return Vec::new();
    }
    field
        .split('|')
        .map(|entry| {
            let mut parts = entry.splitn(3, ':');
            (
                parts.next().unwrap_or(""),
                parts.next().unwrap_or(""),
                parts.next().unwrap_or(""),
            )
        })
        .collect()
}

// --- e01: все 8 hints — 8 записей, варианты y/s/b --------------------------------

#[test]
fn e01_all_eight_hints_reach_mock_with_freedesktop_types() {
    let harness = ext_harness!();
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let id = notifier
        .notify_with_hints(&sample_notification(), &full_hints())
        .expect("Notify со всеми восемью hints");
    assert!(id >= 100, "id из ответа мока: {id}");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "8", "все восемь записей a{{sv}}");
    let entries = hint_entries(hints_field(&record));
    assert_eq!(entries.len(), 8, "журнал: {:?}", hints_field(&record));
    // Ключи и типы вариантов заданы freedesktop: y — urgency, s — строки, b — флаги.
    for expected in [
        ("urgency", "y", "1"),
        ("category", "s", "im.received"),
        ("desktop-entry", "s", "discord-lab"),
        ("image-path", "s", "/usr/share/icons/discord-lab.png"),
        ("sound-name", "s", "message-new-instant"),
        ("resident", "b", "1"),
        ("transient", "b", "1"),
        ("suppress-sound", "b", "1"),
    ] {
        assert!(
            entries.contains(&expected),
            "нет записи {expected:?} в {:?}",
            hints_field(&record)
        );
    }
}

// --- e02: image-path и sound-name — варианты s -----------------------------------

#[test]
fn e02_image_path_and_sound_name_are_string_variants() {
    let harness = ext_harness!();
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    notifier
        .notify_with_hints(
            &sample_notification(),
            &Hints {
                image_path: Some("file:///tmp/avatar.png".to_owned()),
                sound_name: Some("alarm-clock-elapsed".to_owned()),
                ..Hints::default()
            },
        )
        .expect("Notify с image-path/sound-name");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "2");
    assert_eq!(
        hints_field(&record),
        "image-path:s:file:///tmp/avatar.png|sound-name:s:alarm-clock-elapsed",
        "ключи freedesktop `image-path`/`sound-name` с вариантом `s`"
    );
}

// --- e03: transient/suppress-sound — b, включая false ----------------------------

#[test]
fn e03_transient_and_suppress_sound_are_bools_including_false() {
    let harness = ext_harness!();
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    for (transient, suppress_sound) in [(true, false), (false, true)] {
        notifier
            .notify_with_hints(
                &sample_notification(),
                &Hints {
                    transient: Some(transient),
                    suppress_sound: Some(suppress_sound),
                    ..Hints::default()
                },
            )
            .expect("Notify с transient/suppress-sound");
    }

    let first = hints_notify_record(&harness, 0);
    assert_eq!(first[6], "2");
    assert_eq!(
        hints_field(&first),
        "transient:b:1|suppress-sound:b:0",
        "false — это `b:0`, а не пропуск записи"
    );
    let second = hints_notify_record(&harness, 1);
    assert_eq!(second[6], "2");
    assert_eq!(
        hints_field(&second),
        "transient:b:0|suppress-sound:b:1",
        "ключи не должны слипаться/меняться местами"
    );
}

// --- e04: пустые строки новых полей не отправляются ------------------------------

#[test]
fn e04_empty_new_strings_are_skipped() {
    let harness = ext_harness!();
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    notifier
        .notify_with_hints(
            &sample_notification(),
            &Hints {
                urgency: Some(Urgency::Low),
                category: Some(String::new()),
                desktop_entry: Some(String::new()),
                image_path: Some(String::new()),
                sound_name: Some(String::new()),
                resident: Some(true),
                ..Hints::default()
            },
        )
        .expect("Notify с пустыми строками");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "2", "пустые строки не должны стать записями");
    assert_eq!(hints_field(&record), "urgency:y:0|resident:b:1");

    // Только пустые строки → dict с 0 записей (не null и не записи с "").
    notifier
        .notify_with_hints(
            &sample_notification(),
            &Hints {
                category: Some(String::new()),
                desktop_entry: Some(String::new()),
                image_path: Some(String::new()),
                sound_name: Some(String::new()),
                ..Hints::default()
            },
        )
        .expect("Notify только с пустыми строками");
    let empty = hints_notify_record(&harness, 1);
    assert_eq!(empty[6], "0");
    assert_eq!(hints_field(&empty), "");
}

// --- e05: порядок ключей фиксирован ----------------------------------------------

#[test]
fn e05_key_order_is_frozen() {
    let harness = ext_harness!();
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    // Поля заданы в обратном порядке: порядок записей должен следовать контракту,
    // а не порядку инициализации структуры.
    let hints = Hints {
        suppress_sound: Some(true),
        transient: Some(true),
        resident: Some(true),
        sound_name: Some("bell".to_owned()),
        image_path: Some("/tmp/bell.png".to_owned()),
        desktop_entry: Some("discord-lab".to_owned()),
        category: Some("im.received".to_owned()),
        urgency: Some(Urgency::Critical),
    };
    notifier
        .notify_with_hints(&sample_notification(), &hints)
        .expect("Notify со всеми hints");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(
        hints_field(&record),
        "urgency:y:2|category:s:im.received|desktop-entry:s:discord-lab|\
         image-path:s:/tmp/bell.png|sound-name:s:bell|resident:b:1|transient:b:1|\
         suppress-sound:b:1",
        "порядок: urgency, category, desktop-entry, image-path, sound-name, \
         resident, transient, suppress-sound"
    );
}

// --- e06: Unicode и 4 КиБ --------------------------------------------------------

#[test]
fn e06_unicode_and_long_new_hints_are_delivered_without_panic() {
    let harness = ext_harness!();
    let mut notifier = Notifier::connect(&harness.bus_address()).expect("Notifier::connect");

    let long_unicode = "я".repeat(2048); // 4096 байт в UTF-8
    let long_ascii = "a".repeat(4096);
    assert_eq!(long_unicode.len(), 4096, "длина в байтах, не в символах");
    let hints = Hints {
        category: Some("im.📬".to_owned()),
        image_path: Some(long_unicode.clone()),
        sound_name: Some(long_ascii.clone()),
        ..Hints::default()
    };

    let id = notifier
        .notify_with_hints(&sample_notification(), &hints)
        .expect("длинные строки не должны ломать маршалинг");
    assert!(id >= 100, "id из ответа мока: {id}");

    let record = hints_notify_record(&harness, 0);
    assert_eq!(record[6], "3");
    assert_eq!(
        hints_field(&record),
        format!("category:s:im.📬|image-path:s:{long_unicode}|sound-name:s:{long_ascii}")
    );
}

// --- e07: байты a{sv} против libgio + round-trip ---------------------------------

/// Минимальный JSON-ридер фикстуры (как в `dbus_contract.rs`, без внешних зависимостей).
#[derive(Debug, Clone, PartialEq)]
enum ExtJson {
    Null,
    Bool(bool),
    Num(i64),
    Str(String),
    Arr(Vec<ExtJson>),
    Obj(Vec<(String, ExtJson)>),
}

impl ExtJson {
    fn get(&self, key: &str) -> Option<&Self> {
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

    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Num(value) => Some(*value),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        self.as_i64().and_then(|value| u64::try_from(value).ok())
    }

    fn as_arr(&self) -> Option<&[Self]> {
        match self {
            Self::Arr(items) => Some(items),
            _ => None,
        }
    }
}

fn load_ext_fixture() -> ExtJson {
    let path = repo_path("fixtures/dbus/hints-ext.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("чтение {}: {err}", path.display()));
    ext_json_parse(&text).unwrap_or_else(|err| panic!("{} — битый JSON: {err}", path.display()))
}

fn ext_json_parse(text: &str) -> Result<ExtJson, String> {
    fn skip_ws(bytes: &[u8], pos: &mut usize) {
        while *pos < bytes.len() && (bytes[*pos] as char).is_ascii_whitespace() {
            *pos += 1;
        }
    }

    fn hex4(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
        let digits = bytes
            .get(*pos..*pos + 4)
            .ok_or_else(|| "обрыв \\u-escape".to_owned())?;
        let text = std::str::from_utf8(digits).map_err(|err| err.to_string())?;
        *pos += 4;
        u32::from_str_radix(text, 16).map_err(|err| format!("\\u{text}: {err}"))
    }

    fn string(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
        // Открывающая кавычка.
        *pos += 1;
        let mut out = String::new();
        while let Some(&byte) = bytes.get(*pos) {
            *pos += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = *bytes.get(*pos).ok_or_else(|| "обрыв escape".to_owned())?;
                    *pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let code = hex4(bytes, pos)?;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        other => return Err(format!("неизвестный escape \\{}", other as char)),
                    }
                }
                _ => {
                    // UTF-8: собираем байт и декодируем по накопленной строке.
                    let start = *pos - 1;
                    let width = match byte {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    let end = start + width;
                    let chunk = bytes
                        .get(start..end)
                        .ok_or_else(|| "обрыв UTF-8".to_owned())?;
                    let text = std::str::from_utf8(chunk).map_err(|err| err.to_string())?;
                    out.push_str(text);
                    *pos = end;
                }
            }
        }
        Err("незакрытая строка".to_owned())
    }

    fn value(bytes: &[u8], pos: &mut usize) -> Result<ExtJson, String> {
        skip_ws(bytes, pos);
        let byte = *bytes
            .get(*pos)
            .ok_or_else(|| "неожиданный конец JSON".to_owned())?;
        match byte {
            b'{' => {
                *pos += 1;
                let mut entries = Vec::new();
                loop {
                    skip_ws(bytes, pos);
                    if bytes.get(*pos) == Some(&b'}') {
                        *pos += 1;
                        return Ok(ExtJson::Obj(entries));
                    }
                    if bytes.get(*pos) != Some(&b'"') {
                        return Err(format!("ожидался ключ объекта на позиции {pos}"));
                    }
                    let key = string(bytes, pos)?;
                    skip_ws(bytes, pos);
                    if bytes.get(*pos) != Some(&b':') {
                        return Err(format!("ожидалось ':' на позиции {pos}"));
                    }
                    *pos += 1;
                    let item = value(bytes, pos)?;
                    entries.push((key, item));
                    skip_ws(bytes, pos);
                    match bytes.get(*pos) {
                        Some(b',') => *pos += 1,
                        Some(b'}') => {
                            *pos += 1;
                            return Ok(ExtJson::Obj(entries));
                        }
                        _ => return Err(format!("ожидалось ',' или '}}' на позиции {pos}")),
                    }
                }
            }
            b'[' => {
                *pos += 1;
                let mut items = Vec::new();
                loop {
                    skip_ws(bytes, pos);
                    if bytes.get(*pos) == Some(&b']') {
                        *pos += 1;
                        return Ok(ExtJson::Arr(items));
                    }
                    items.push(value(bytes, pos)?);
                    skip_ws(bytes, pos);
                    match bytes.get(*pos) {
                        Some(b',') => *pos += 1,
                        Some(b']') => {
                            *pos += 1;
                            return Ok(ExtJson::Arr(items));
                        }
                        _ => return Err(format!("ожидалось ',' или ']' на позиции {pos}")),
                    }
                }
            }
            b'"' => Ok(ExtJson::Str(string(bytes, pos)?)),
            b't' if bytes.get(*pos..*pos + 4) == Some(&b"true"[..]) => {
                *pos += 4;
                Ok(ExtJson::Bool(true))
            }
            b'f' if bytes.get(*pos..*pos + 5) == Some(&b"false"[..]) => {
                *pos += 5;
                Ok(ExtJson::Bool(false))
            }
            b'n' if bytes.get(*pos..*pos + 4) == Some(&b"null"[..]) => {
                *pos += 4;
                Ok(ExtJson::Null)
            }
            b'-' | b'0'..=b'9' => {
                let start = *pos;
                while bytes
                    .get(*pos)
                    .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'-')
                {
                    *pos += 1;
                }
                let text =
                    std::str::from_utf8(&bytes[start..*pos]).map_err(|err| err.to_string())?;
                text.parse::<i64>()
                    .map(ExtJson::Num)
                    .map_err(|err| format!("число {text}: {err}"))
            }
            other => Err(format!(
                "неожиданный символ {} на позиции {pos}",
                other as char
            )),
        }
    }

    let mut pos = 0;
    let parsed = value(text.as_bytes(), &mut pos)?;
    skip_ws(text.as_bytes(), &mut pos);
    if pos != text.len() {
        return Err(format!("мусор после JSON на позиции {pos}"));
    }
    Ok(parsed)
}

fn ext_json_hex_decode(text: &str) -> Vec<u8> {
    assert!(
        text.len().is_multiple_of(2),
        "hex нечётной длины: {}",
        text.len()
    );
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16)
                .unwrap_or_else(|err| panic!("hex {}: {err}", &text[index..index + 2]))
        })
        .collect()
}

fn ext_json_hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// `Hints` из набора фикстуры (ключи/значения заданы оракулом).
fn ext_json_hints(items: &[ExtJson]) -> Hints {
    let mut hints = Hints::default();
    for item in items {
        let key = item
            .get("key")
            .and_then(ExtJson::as_str)
            .expect("hints.key");
        let signature = item
            .get("signature")
            .and_then(ExtJson::as_str)
            .expect("hints.signature");
        let value = item.get("value").expect("hints.value");
        match (key, signature) {
            ("urgency", "y") => {
                hints.urgency = Some(match value.as_u64() {
                    Some(0) => Urgency::Low,
                    Some(1) => Urgency::Normal,
                    Some(2) => Urgency::Critical,
                    other => panic!("{key}: неожиданная urgency {other:?}"),
                });
            }
            ("category", "s") => hints.category = value.as_str().map(str::to_owned),
            ("desktop-entry", "s") => hints.desktop_entry = value.as_str().map(str::to_owned),
            ("image-path", "s") => hints.image_path = value.as_str().map(str::to_owned),
            ("sound-name", "s") => hints.sound_name = value.as_str().map(str::to_owned),
            ("resident", "b") => hints.resident = Some(ext_json_bool(value, key)),
            ("transient", "b") => hints.transient = Some(ext_json_bool(value, key)),
            ("suppress-sound", "b") => hints.suppress_sound = Some(ext_json_bool(value, key)),
            other => panic!("неизвестная запись hints: {other:?}"),
        }
    }
    hints
}

fn ext_json_bool(value: &ExtJson, key: &str) -> bool {
    match value {
        ExtJson::Bool(flag) => *flag,
        other => panic!("{key}: ожидался bool, получено {other:?}"),
    }
}

/// Ожидаемый `a{sv}` из фикстуры: ключи, порядок и типы вариантов заданы libgio/libdbus.
fn ext_json_hints_value(items: &[ExtJson]) -> Value {
    let mut entries = Vec::new();
    for item in items {
        let key = item
            .get("key")
            .and_then(ExtJson::as_str)
            .expect("hints.key");
        let signature = item
            .get("signature")
            .and_then(ExtJson::as_str)
            .expect("hints.signature");
        let value = item.get("value").expect("hints.value");
        let inner = match signature {
            "y" => Value::Byte(
                u8::try_from(value.as_u64().expect("hints.y")).expect("hints.y в диапазоне u8"),
            ),
            "s" => Value::String(value.as_str().expect("hints.s").to_owned()),
            "b" => Value::Bool(ext_json_bool(value, key)),
            other => panic!("{key}: неизвестная сигнатура варианта {other:?}"),
        };
        entries.push((
            Value::String(key.to_owned()),
            Value::Variant(Box::new(inner)),
        ));
    }
    Value::Dict(entries)
}

/// Тело `Notify` из общих полей фикстуры и нашего `a{sv}`.
fn ext_notify_body(golden: &ExtJson, hints: &Hints) -> Vec<Value> {
    let notify = golden.get("notify").expect("hints-ext.json: notify");
    let text = |key: &str| {
        notify
            .get(key)
            .and_then(ExtJson::as_str)
            .unwrap_or_else(|| panic!("hints-ext.json: notify.{key}"))
            .to_owned()
    };
    let actions = notify
        .get("actions")
        .and_then(ExtJson::as_arr)
        .expect("hints-ext.json: notify.actions")
        .iter()
        .map(|item| Value::String(item.as_str().expect("notify.action").to_owned()))
        .collect();
    let timeout_ms = i32::try_from(
        notify
            .get("timeout_ms")
            .and_then(ExtJson::as_i64)
            .expect("hints-ext.json: notify.timeout_ms"),
    )
    .expect("timeout_ms в диапазоне i32");

    vec![
        Value::String(text("app_name")),
        Value::U32(0),                // replaces_id
        Value::String(String::new()), // app_icon
        Value::String(text("summary")),
        Value::String(text("body")),
        Value::Array(actions),
        hints.to_dbus(),
        Value::I32(timeout_ms),
    ]
}

#[test]
fn e07_hints_bytes_match_libgio_and_round_trip() {
    let golden = load_ext_fixture();
    assert_eq!(
        golden.get("hints_signature").and_then(ExtJson::as_str),
        Some("a{sv}"),
        "hints — именно a{{sv}}"
    );
    assert_eq!(
        golden.get("signature").and_then(ExtJson::as_str),
        Some(notifications::NOTIFY_SIGNATURE),
        "сигнатура Notify не меняется"
    );

    let cases = golden
        .get("cases")
        .and_then(ExtJson::as_arr)
        .expect("hints-ext.json: cases");
    assert!(
        cases.len() >= 5,
        "ожидались наборы all_eight/image_and_sound/bools_false/…, есть {}",
        cases.len()
    );

    for case in cases {
        let name = case
            .get("name")
            .and_then(ExtJson::as_str)
            .expect("case.name");
        let items = case
            .get("hints")
            .and_then(ExtJson::as_arr)
            .expect("case.hints");
        let hints = ext_json_hints(items);
        let expected = ext_json_hints_value(items);

        // Структура, типы вариантов и порядок записей — как в эталоне оракула.
        assert_eq!(hints.to_dbus(), expected, "{name}: форма a{{sv}}");

        let hex = case
            .get("notify_hex")
            .and_then(ExtJson::as_str)
            .expect("case.notify_hex");
        let bytes = ext_json_hex_decode(hex);
        let libgio = message::parse(&bytes)
            .unwrap_or_else(|err| panic!("{name}: байты libgio не разобраны: {err:?}"));
        assert_eq!(libgio.kind, MessageType::MethodCall);
        assert_eq!(libgio.member(), Some("Notify"));
        assert_eq!(
            libgio.signature,
            notifications::NOTIFY_SIGNATURE,
            "{name}: сигнатура вызова"
        );
        assert_eq!(
            libgio.body.get(6),
            Some(&expected),
            "{name}: libdbus/libgio демаршалит другой a{{sv}}"
        );

        // Побайтовое совпадение с libgio: выравнивание массива 4, dict-entry 8,
        // сигнатуры вариантов и порядок записей.
        let body = ext_notify_body(&golden, &hints);
        let rebuilt = message::build(
            MessageType::MethodCall,
            libgio.serial,
            &libgio.fields,
            &libgio.signature,
            &body,
        )
        .unwrap_or_else(|err| panic!("{name}: сборка сообщения: {err:?}"));
        assert_eq!(
            ext_json_hex_encode(&rebuilt),
            ext_json_hex_encode(&bytes),
            "{name}: байты a{{sv}} расходятся с libgio"
        );

        let round_trip = message::parse(&rebuilt).expect("разбор собранного сообщения");
        assert_eq!(round_trip.body, body, "{name}: round-trip тела");
        assert_eq!(
            round_trip.body.get(6),
            Some(&expected),
            "{name}: round-trip a{{sv}}"
        );
    }
}

// =================================================================================
// task-174 / BE-P2-14: NotificationRouter (DND, dedup, deep-link, secret redaction)
// =================================================================================

fn sample_incoming(msg_id: &str) -> IncomingMessageNotification {
    IncomingMessageNotification {
        message_id: msg_id.to_string(),
        guild_id: Some("guild_42".to_string()),
        channel_id: "chan_100".to_string(),
        author_id: "user_bob".to_string(),
        author_name: "Bob".to_string(),
        content: "Hello @Alice!".to_string(),
        mentions_self: true,
        is_dm: false,
    }
}

#[test]
fn r01_router_suppresses_self_author() {
    let mut router = NotificationRouter::new();
    router.self_user_id = Some("user_me".to_string());

    let mut msg = sample_incoming("msg_001");
    msg.author_id = "user_me".to_string();

    let decision = router.evaluate(&msg);
    assert_eq!(decision, NotificationDispatchDecision::SuppressedSelfAuthor);

    // Other author passes
    msg.author_id = "user_other".to_string();
    let decision = router.evaluate(&msg);
    assert!(matches!(
        decision,
        NotificationDispatchDecision::Send { .. }
    ));
}

#[test]
fn r02_router_suppresses_dnd() {
    let mut router = NotificationRouter::new();
    router.dnd_enabled = true;

    let msg = sample_incoming("msg_002");
    let decision = router.evaluate(&msg);
    assert_eq!(decision, NotificationDispatchDecision::SuppressedDnd);

    // Disabled DND allows evaluate
    router.dnd_enabled = false;
    let decision = router.evaluate(&msg);
    assert!(matches!(
        decision,
        NotificationDispatchDecision::Send { .. }
    ));
}

#[test]
fn r03_router_suppresses_active_channel_only_when_window_focused() {
    let mut router = NotificationRouter::new();
    router.active_channel_id = Some("chan_100".to_string());

    let msg = sample_incoming("msg_003");

    // Window NOT focused -> does NOT suppress even if channel matches
    router.window_focused = false;
    let decision = router.evaluate(&msg);
    assert!(matches!(
        decision,
        NotificationDispatchDecision::Send { .. }
    ));

    // Window focused + active channel matches -> suppresses
    let msg2 = sample_incoming("msg_003b");
    router.window_focused = true;
    let decision = router.evaluate(&msg2);
    assert_eq!(
        decision,
        NotificationDispatchDecision::SuppressedActiveChannel
    );

    // Window focused + different channel -> does NOT suppress
    let mut msg3 = sample_incoming("msg_003c");
    msg3.channel_id = "chan_other".to_string();
    let decision = router.evaluate(&msg3);
    assert!(matches!(
        decision,
        NotificationDispatchDecision::Send { .. }
    ));
}

#[test]
fn r04_router_suppresses_when_not_mention_and_not_dm() {
    let mut router = NotificationRouter::new();

    let mut msg = sample_incoming("msg_004a");
    msg.mentions_self = false;
    msg.is_dm = false;

    let decision = router.evaluate(&msg);
    assert_eq!(
        decision,
        NotificationDispatchDecision::SuppressedNotMentionOrDm
    );

    // Mentions self -> passes
    let mut msg_mention = sample_incoming("msg_004b");
    msg_mention.mentions_self = true;
    msg_mention.is_dm = false;
    assert!(matches!(
        router.evaluate(&msg_mention),
        NotificationDispatchDecision::Send { .. }
    ));

    // DM without mention -> passes
    let mut msg_dm = sample_incoming("msg_004c");
    msg_dm.mentions_self = false;
    msg_dm.is_dm = true;
    assert!(matches!(
        router.evaluate(&msg_dm),
        NotificationDispatchDecision::Send { .. }
    ));
}

#[test]
fn r05_router_dedup_and_bounded_fifo_eviction() {
    let mut router = NotificationRouter::with_capacity(3);

    let msg1 = sample_incoming("msg_1");
    let msg2 = sample_incoming("msg_2");
    let msg3 = sample_incoming("msg_3");
    let msg4 = sample_incoming("msg_4");

    // First send succeeds
    assert!(matches!(
        router.evaluate(&msg1),
        NotificationDispatchDecision::Send { .. }
    ));

    // Immediate duplicate is suppressed
    assert_eq!(
        router.evaluate(&msg1),
        NotificationDispatchDecision::SuppressedDuplicate
    );

    // Push msg2 and msg3 -> queue has msg1, msg2, msg3
    assert!(matches!(
        router.evaluate(&msg2),
        NotificationDispatchDecision::Send { .. }
    ));
    assert!(matches!(
        router.evaluate(&msg3),
        NotificationDispatchDecision::Send { .. }
    ));

    // Push msg4 -> FIFO capacity is 3, msg1 should be evicted
    assert!(matches!(
        router.evaluate(&msg4),
        NotificationDispatchDecision::Send { .. }
    ));

    // msg1 was evicted, so it can be evaluated again
    assert!(matches!(
        router.evaluate(&msg1),
        NotificationDispatchDecision::Send { .. }
    ));

    // msg2, msg3, msg4: msg2 was evicted by msg1!
    assert!(matches!(
        router.evaluate(&msg2),
        NotificationDispatchDecision::Send { .. }
    ));
}

#[test]
fn r06_router_builds_notification_and_deep_links() {
    let mut router = NotificationRouter::new();

    // 1. Guild message
    let guild_msg = IncomingMessageNotification {
        message_id: "m_guild".to_string(),
        guild_id: Some("guild_777".to_string()),
        channel_id: "chan_888".to_string(),
        author_id: "user_carol".to_string(),
        author_name: "Carol".to_string(),
        content: "Hey team!".to_string(),
        mentions_self: true,
        is_dm: false,
    };
    match router.evaluate(&guild_msg) {
        NotificationDispatchDecision::Send {
            notification,
            deep_link_url,
        } => {
            assert_eq!(deep_link_url, "discord://-/channels/guild_777/chan_888");
            assert_eq!(notification.summary, "Carol");
            assert_eq!(notification.body, "Hey team!");
            assert_eq!(notification.replaces_id, 0);
            assert_eq!(
                notification.actions,
                vec![("default".to_string(), "Open".to_string())]
            );
        }
        other => panic!("expected Send, got {other:?}"),
    }

    // 2. DM message
    let dm_msg = IncomingMessageNotification {
        message_id: "m_dm".to_string(),
        guild_id: None,
        channel_id: "dm_555".to_string(),
        author_id: "user_dave".to_string(),
        author_name: "Dave".to_string(),
        content: "Direct message here".to_string(),
        mentions_self: false,
        is_dm: true,
    };
    match router.evaluate(&dm_msg) {
        NotificationDispatchDecision::Send {
            notification,
            deep_link_url,
        } => {
            assert_eq!(deep_link_url, "discord://-/channels/@me/dm_555");
            assert_eq!(notification.summary, "Dave");
            assert_eq!(notification.body, "Direct message here");
            assert_eq!(
                notification.actions,
                vec![("default".to_string(), "Open".to_string())]
            );
        }
        other => panic!("expected Send, got {other:?}"),
    }
}

#[test]
fn r07_router_redacts_tokens_mfa_base64_bearer() {
    let mut router = NotificationRouter::new();

    let secret_mfa = "mfa.super_secret_token_123456789";
    let secret_base64 = "MTIzNDU2Nzg5MDEyMzQ1Njc4.GaBcDe.fGhIjKlMnOpQrStUvWxYz012345";
    let secret_bearer = "Bearer token_secret_value_999999";

    let content =
        format!("Check this: {secret_mfa} and {secret_base64} and also {secret_bearer} please.");

    let msg = IncomingMessageNotification {
        message_id: "m_secret".to_string(),
        guild_id: None,
        channel_id: "chan_dm".to_string(),
        author_id: "user_eve".to_string(),
        author_name: "Eve".to_string(),
        content,
        mentions_self: true,
        is_dm: true,
    };

    match router.evaluate(&msg) {
        NotificationDispatchDecision::Send { notification, .. } => {
            assert!(
                !notification.body.contains(secret_mfa),
                "mfa secret leaked: {}",
                notification.body
            );
            assert!(
                !notification.body.contains(secret_base64),
                "base64 token leaked: {}",
                notification.body
            );
            assert!(
                !notification.body.contains("token_secret_value_999999"),
                "bearer token leaked: {}",
                notification.body
            );
            assert!(
                notification.body.contains("<redacted>"),
                "no <redacted> in body: {}",
                notification.body
            );
        }
        other => panic!("expected Send, got {other:?}"),
    }

    // Direct redact_secrets tests
    assert_eq!(redact_secrets("token: mfa.xxx"), "token: <redacted>");
    assert_eq!(
        redact_secrets("clean url: https://discord.com/channels/1/2"),
        "clean url: https://discord.com/channels/1/2"
    );
}

#[test]
fn r08_router_register_sent_and_handle_event() {
    let mut router = NotificationRouter::new();
    let url = "discord://-/channels/@me/chan_42".to_string();

    router.register_sent(101, url.clone());

    // ActionInvoked / Action returns registered deep_link_url
    let action_event = NotificationEvent::Action {
        id: 101,
        action: "default".to_string(),
    };
    assert_eq!(router.handle_event(&action_event), Some(url.clone()));

    // Event alias + ActionInvoked variant also works
    let action_invoked = Event::ActionInvoked {
        id: 101,
        action_key: "default".to_string(),
    };
    assert_eq!(router.handle_event(&action_invoked), Some(url));

    // NotificationClosed removes the entry
    let closed_event = NotificationEvent::Closed { id: 101, reason: 1 };
    assert_eq!(router.handle_event(&closed_event), None);

    // Subsequent action after close returns None
    assert_eq!(router.handle_event(&action_event), None);

    // NotificationClosed variant also works
    router.register_sent(102, "discord://-/channels/1/2".to_string());
    let closed_variant = Event::NotificationClosed { id: 102, reason: 2 };
    assert_eq!(router.handle_event(&closed_variant), None);
    assert_eq!(
        router.handle_event(&NotificationEvent::Action {
            id: 102,
            action: "default".to_string(),
        }),
        None
    );
}

#[test]
fn r09_router_handle_event_unknown_id() {
    let mut router = NotificationRouter::new();
    let event = NotificationEvent::Action {
        id: 999,
        action: "default".to_string(),
    };
    assert_eq!(router.handle_event(&event), None);

    let closed = NotificationEvent::Closed { id: 999, reason: 1 };
    assert_eq!(router.handle_event(&closed), None);
}
