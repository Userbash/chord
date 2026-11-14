//! Контракт P1-4.1: адаптер `MESSAGE_CREATE` -> `org.freedesktop.Notifications.Notify()`.
//!
//! Проверяет все 9 измерений фильтрации, форматирования, UTF-8 обрезки и интеграцию
//! с `Notifier::notify_message` поверх приватного `dbus-daemon` и `mock_service.py`.

use chord_platform::dbus::BusAddress;
use chord_platform::notifications::{
    MessageNotificationContext, MessageNotificationEvent, NotificationLevel, Notifier,
    PreparedMessageNotification, Urgency, prepare_message_notification,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SKIP_NO_DAEMON: &str = "нет dbus-daemon";
const SKIP_NO_PYTHON: &str = "нет python3 с модулями dbus и gi";

struct Harness {
    address: String,
    journal: PathBuf,
    dir: PathBuf,
    daemon_pid: u32,
    mock: Child,
}

impl Harness {
    fn start() -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(SKIP_NO_PYTHON.to_owned());
        }

        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w2-4-{}-{}",
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

    fn wait_for_member(&self, member: &str, count: usize) -> Vec<Vec<String>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records: Vec<Vec<String>> = fs::read_to_string(&self.journal)
                .unwrap_or_default()
                .lines()
                .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<_>>())
                .filter(|fields| fields.first().map(String::as_str) == Some(member))
                .collect();
            if records.len() >= count || Instant::now() > deadline {
                return records;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
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

fn base_event() -> MessageNotificationEvent {
    MessageNotificationEvent {
        message_id: "100001".to_owned(),
        channel_id: "200001".to_owned(),
        channel_name: "general".to_owned(),
        guild_name: Some("Chord Lab".to_owned()),
        author_id: "300002".to_owned(),
        author_name: "Alice".to_owned(),
        content: "Привет, команда!".to_owned(),
        is_dm: false,
        mentions_current_user: false,
        mentions_everyone: false,
        has_attachments: false,
    }
}

fn base_context() -> MessageNotificationContext {
    MessageNotificationContext {
        current_user_id: "300001".to_owned(),
        user_dnd: false,
        app_focused: false,
        active_channel_id: Some("200999".to_owned()),
        channel_muted: false,
        channel_level: NotificationLevel::AllMessages,
        suppress_everyone: false,
        desktop_entry: Some("discord-gpui".to_owned()),
    }
}

#[test]
fn m01_suppresses_self_author() {
    let mut event = base_event();
    event.author_id = "300001".to_owned();
    event.is_dm = true;
    event.mentions_current_user = true;

    let ctx = base_context();
    assert_eq!(prepare_message_notification(&event, &ctx), None);
}

#[test]
fn m02_suppresses_all_notifications_when_dnd_enabled() {
    let mut ctx = base_context();
    ctx.user_dnd = true;

    let dm = MessageNotificationEvent {
        is_dm: true,
        ..base_event()
    };
    let mention = MessageNotificationEvent {
        mentions_current_user: true,
        ..base_event()
    };
    let normal = base_event();

    assert_eq!(prepare_message_notification(&dm, &ctx), None);
    assert_eq!(prepare_message_notification(&mention, &ctx), None);
    assert_eq!(prepare_message_notification(&normal, &ctx), None);
}

#[test]
fn m03_active_channel_suppresses_only_when_app_focused() {
    let mut event = base_event();
    event.is_dm = true;

    let mut ctx = base_context();
    ctx.active_channel_id = Some(event.channel_id.clone());
    ctx.app_focused = true;
    assert_eq!(prepare_message_notification(&event, &ctx), None);

    ctx.app_focused = false;
    assert!(prepare_message_notification(&event, &ctx).is_some());
}

#[test]
fn m04_muted_channel_policy_filters_normal_and_everyone_but_allows_direct_mention() {
    let mut ctx = base_context();
    ctx.channel_muted = true;
    ctx.channel_level = NotificationLevel::OnlyMentions;
    ctx.suppress_everyone = true;

    let normal = base_event();
    assert_eq!(prepare_message_notification(&normal, &ctx), None);

    let everyone = MessageNotificationEvent {
        mentions_everyone: true,
        ..base_event()
    };
    assert_eq!(prepare_message_notification(&everyone, &ctx), None);

    let direct_mention = MessageNotificationEvent {
        mentions_current_user: true,
        ..base_event()
    };
    assert!(prepare_message_notification(&direct_mention, &ctx).is_some());

    ctx.channel_muted = false;
    ctx.channel_level = NotificationLevel::Nothing;
    assert_eq!(prepare_message_notification(&direct_mention, &ctx), None);
}

#[test]
fn m05_formatting_and_urgency_hierarchy() {
    let ctx = base_context();

    let dm = MessageNotificationEvent {
        is_dm: true,
        guild_name: None,
        channel_name: String::new(),
        ..base_event()
    };
    let PreparedMessageNotification {
        notification: dm_notif,
        hints: dm_hints,
    } = prepare_message_notification(&dm, &ctx).expect("DM should emit");
    assert_eq!(dm_notif.summary, "Alice");
    assert_eq!(dm_hints.urgency, Some(Urgency::Critical));
    assert_eq!(dm_hints.category.as_deref(), Some("im.received"));
    assert_eq!(dm_hints.desktop_entry.as_deref(), Some("discord-gpui"));

    let mention = MessageNotificationEvent {
        mentions_current_user: true,
        ..base_event()
    };
    let PreparedMessageNotification {
        notification: mention_notif,
        hints: mention_hints,
    } = prepare_message_notification(&mention, &ctx).expect("mention should emit");
    assert_eq!(mention_notif.summary, "Alice (#general)");
    assert_eq!(mention_hints.urgency, Some(Urgency::Normal));
    assert_eq!(mention_hints.category.as_deref(), Some("im.received"));

    let normal = base_event();
    let PreparedMessageNotification {
        notification: normal_notif,
        hints: normal_hints,
    } = prepare_message_notification(&normal, &ctx).expect("unmuted AllMessages should emit");
    assert_eq!(normal_notif.summary, "Alice (#general)");
    assert_eq!(normal_hints.urgency, Some(Urgency::Low));
    assert_eq!(normal_hints.category.as_deref(), Some("im.received"));
}

#[test]
fn m06_empty_content_with_and_without_attachments() {
    let ctx = base_context();

    let empty_no_attach = MessageNotificationEvent {
        content: "   \n\t ".to_owned(),
        has_attachments: false,
        ..base_event()
    };
    assert_eq!(prepare_message_notification(&empty_no_attach, &ctx), None);

    let empty_with_attach = MessageNotificationEvent {
        content: String::new(),
        has_attachments: true,
        ..base_event()
    };
    let prepared =
        prepare_message_notification(&empty_with_attach, &ctx).expect("attachment should emit");
    assert_eq!(prepared.notification.body, "[Вложение]");
}

#[test]
fn m07_utf8_multibyte_safe_truncation() {
    let ctx = base_context();
    // 2000-char string mixing Cyrillic and 4-byte emoji
    let pattern = "Привет🚀мир🔥";
    let long_content: String = pattern.chars().cycle().take(2000).collect();
    assert_eq!(long_content.chars().count(), 2000);

    let event = MessageNotificationEvent {
        content: long_content,
        ..base_event()
    };
    let prepared = prepare_message_notification(&event, &ctx).expect("long message should emit");
    let char_count = prepared.notification.body.chars().count();
    assert!(char_count <= 256, "expected <= 256 chars, got {char_count}");
    assert!(prepared.notification.body.ends_with('…'));
}

#[test]
fn m08_notifier_notify_message_sends_over_dbus_and_suppresses_when_filtered() {
    let harness = match Harness::start() {
        Ok(harness) => harness,
        Err(reason) => {
            println!("SKIP: {reason}");
            return;
        }
    };

    let mut notifier = Notifier::connect(&harness.bus_address()).expect("connect mock dbus");
    let ctx = base_context();

    // Filtered message -> Ok(None), nothing sent to D-Bus
    let self_msg = MessageNotificationEvent {
        author_id: ctx.current_user_id.clone(),
        ..base_event()
    };
    assert_eq!(
        notifier.notify_message(&self_msg, &ctx).expect("call ok"),
        None
    );

    // Emitted mention -> Ok(Some(id)), recorded by mock_service.py
    let mention = MessageNotificationEvent {
        mentions_current_user: true,
        content: "Привет из mock D-Bus!".to_owned(),
        ..base_event()
    };
    let id = notifier
        .notify_message(&mention, &ctx)
        .expect("notify_message ok")
        .expect("should return notification id");
    assert!(id > 0);

    let calls = harness.wait_for_member("Notify", 1);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0][1], "Discord");
    assert_eq!(calls[0][4], "Alice (#general)");
    assert_eq!(calls[0][5], "Привет из mock D-Bus!");
}
