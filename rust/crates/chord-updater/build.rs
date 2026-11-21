//! Build script for chord-updater: embeds selectable CHORD_RELEASE_CHANNEL at compile time.
use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CHORD_RELEASE_CHANNEL");

    let raw = env::var("CHORD_RELEASE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    let normalized = raw.trim().to_ascii_lowercase();
    let channel = if normalized.is_empty() {
        "stable".to_string()
    } else {
        normalized
    };

    match channel.as_str() {
        "stable" | "canary" | "ptb" | "development" => {}
        other => panic!(
            "Invalid CHORD_RELEASE_CHANNEL '{other}': expected one of stable, canary, ptb, development"
        ),
    }

    println!("cargo:rustc-env=CHORD_RELEASE_CHANNEL={channel}");
}
