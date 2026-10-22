//! Контракт P6-03: XDG autostart-файл и модель бейджа Unity Launcher API.
//!
//! Независимый оракул — спецификация XDG Desktop Entry 1.5:
//! - `[Desktop Entry]`, `Type=Application`, `Name=`, `Exec=`, `Terminal=false`;
//! - экранирование Exec: аргумент с пробелом/табом/`"`/`\`/`$`/`%` — в двойных кавычках,
//!   `"`→`\"`, `\`→`\\`, `$`→`\$`, `` ` ``→`` \` ``, `%`→`%%`, пустой аргумент → `""`;
//! - `X-GNOME-Autostart-enabled=false` отключает автозапуск (RE: ключ наблюдался в клиенте);
//! - Unity Launcher API: свойства `count`, `count-visible`, `urgent` (wire/D-Bus не заявлен).

use discord_platform::autostart::{
    AutostartEntry, AutostartError, Badge, BadgeValue, app_uri, autostart_path, badge_pairs,
    desktop_entry,
};
use std::path::{Path, PathBuf};

fn entry<'a>(
    name: &'a str,
    exec: &'a str,
    args: &'a [&'a str],
    hidden: bool,
) -> AutostartEntry<'a> {
    AutostartEntry {
        name,
        exec,
        args,
        hidden,
    }
}

#[test]
fn a01_minimal_entry_matches_desktop_entry_shape() {
    let text = match desktop_entry(&entry(
        "Discord Lab",
        "/usr/bin/discord-lab",
        &["--start-minimized"],
        false,
    )) {
        Ok(text) => text,
        Err(err) => panic!("валидный entry отклонён: {err:?}"),
    };

    assert_eq!(
        text,
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Discord Lab\n\
         Exec=/usr/bin/discord-lab --start-minimized\n\
         X-GNOME-Autostart-enabled=true\n\
         Terminal=false\n"
    );
    assert!(!text.contains("Hidden=true"));
    assert!(!text.contains("Terminal=true"));
}

#[test]
fn a02_hidden_disables_autostart_via_gnome_key() {
    let text = match desktop_entry(&entry("Discord Lab", "/usr/bin/discord-lab", &[], true)) {
        Ok(text) => text,
        Err(err) => panic!("валидный entry отклонён: {err:?}"),
    };

    // Выбран один механизм (ТЗ: «выбери одно и зафиксируй тестом»):
    // hidden → X-GNOME-Autostart-enabled=false, без Hidden=true.
    assert!(
        text.contains("\nX-GNOME-Autostart-enabled=false\n"),
        "{text}"
    );
    assert!(!text.contains("X-GNOME-Autostart-enabled=true"), "{text}");
    assert!(!text.contains("Hidden=true"), "{text}");
}

#[test]
fn a03_exec_escapes_spaces_quotes_and_field_codes() {
    let text = match desktop_entry(&entry(
        "Discord Lab",
        "/opt/Discord Lab/discord",
        &["--flag", "hello world", "say \"hi\"", "100%"],
        false,
    )) {
        Ok(text) => text,
        Err(err) => panic!("валидный entry отклонён: {err:?}"),
    };

    assert!(
        text.contains(
            "Exec=\"/opt/Discord Lab/discord\" --flag \"hello world\" \"say \\\"hi\\\"\" \"100%%\"\n"
        ),
        "{text}"
    );
}

#[test]
fn a04_exec_escapes_shell_reserved_and_empty_args() {
    let text = match desktop_entry(&entry(
        "Discord Lab",
        "/usr/bin/discord-lab",
        &["a\\b", "$HOME", "`id`", "", "tab\there"],
        false,
    )) {
        Ok(text) => text,
        Err(err) => panic!("валидный entry отклонён: {err:?}"),
    };

    assert!(
        text.contains(
            "Exec=/usr/bin/discord-lab \"a\\\\b\" \"\\$HOME\" \"\\`id\\`\" \"\" \"tab\there\"\n"
        ),
        "{text}"
    );
}

#[test]
fn a05_rejects_empty_and_control_fields() {
    assert_eq!(
        desktop_entry(&entry("", "/usr/bin/discord-lab", &[], false)),
        Err(AutostartError::EmptyName)
    );
    assert_eq!(
        desktop_entry(&entry("Discord Lab", "", &[], false)),
        Err(AutostartError::EmptyExec)
    );

    for bad in ["bad\nname", "bad\rname", "bad\0name"] {
        assert_eq!(
            desktop_entry(&entry(bad, "/usr/bin/discord-lab", &[], false)),
            Err(AutostartError::ControlCharacter),
            "name: {bad:?}"
        );
        assert_eq!(
            desktop_entry(&entry("Discord Lab", bad, &[], false)),
            Err(AutostartError::ControlCharacter),
            "exec: {bad:?}"
        );
        assert_eq!(
            desktop_entry(&entry("Discord Lab", "/usr/bin/discord-lab", &[bad], false)),
            Err(AutostartError::ControlCharacter),
            "arg: {bad:?}"
        );
    }

    // Валидный entry с control-символом в последнем аргументе не должен
    // возвращать частично собранный файл.
    assert_eq!(
        desktop_entry(&entry(
            "Discord Lab",
            "/usr/bin/discord-lab",
            &["ok", "bad\narg"],
            false
        )),
        Err(AutostartError::ControlCharacter)
    );
}

#[test]
fn a06_autostart_path_is_xdg_autostart_file() {
    assert_eq!(
        autostart_path(Path::new("/home/u/.config")),
        PathBuf::from("/home/u/.config/autostart/discord-lab.desktop")
    );
    assert_eq!(
        autostart_path(Path::new("")),
        PathBuf::from("autostart/discord-lab.desktop")
    );
}

#[test]
fn a07_badge_pairs_are_stable_and_typed() {
    let badge = Badge {
        count: 3,
        urgent: true,
        visible: true,
    };
    assert_eq!(
        badge_pairs(&badge),
        vec![
            ("count", BadgeValue::Count(3)),
            ("count-visible", BadgeValue::Visible(true)),
            ("urgent", BadgeValue::Urgent(true)),
        ]
    );

    let zero = Badge {
        count: 0,
        urgent: false,
        visible: false,
    };
    assert_eq!(
        badge_pairs(&zero),
        vec![
            ("count", BadgeValue::Count(0)),
            ("count-visible", BadgeValue::Visible(false)),
            ("urgent", BadgeValue::Urgent(false)),
        ]
    );

    let max = Badge {
        count: u64::MAX,
        urgent: false,
        visible: true,
    };
    let pairs = badge_pairs(&max);
    assert_eq!(pairs.len(), 3);
    assert_eq!(pairs[0], ("count", BadgeValue::Count(u64::MAX)));
}

#[test]
fn a09_exec_quotes_xdg_reserved_characters() {
    // Desktop Entry 1.5: аргумент с reserved-символом обязан быть в кавычках,
    // иначе шелл-подобный парсер разобьёт его на части (`it's` → `it` + `s`).
    let reserved = [
        "it's", "a>b", "a<b", "a~b", "a|b", "a&b", "a;b", "a*b", "a?b", "a#b", "a(b)", "a)b",
    ];
    let text = match desktop_entry(&entry(
        "Discord Lab",
        "/usr/bin/discord-lab",
        &reserved,
        false,
    )) {
        Ok(text) => text,
        Err(err) => panic!("валидный entry отклонён: {err:?}"),
    };

    for arg in reserved {
        assert!(
            text.contains(&format!("\"{arg}\"")),
            "аргумент {arg:?} не закавычен: {text}"
        );
    }
}

#[test]
fn a08_app_uri_is_unity_application_uri() {
    assert_eq!(
        app_uri("discord-lab.desktop"),
        "application://discord-lab.desktop"
    );
    assert_eq!(app_uri(""), "application://");
}
