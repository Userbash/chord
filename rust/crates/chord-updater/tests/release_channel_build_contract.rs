//! R4-09: Build-time release channel selection contract tests.
use chord_updater::ReleaseChannel;

#[test]
fn build_time_release_channel_defaults_to_valid_channel() {
    let channel = ReleaseChannel::build_time();
    assert!(
        matches!(
            channel,
            ReleaseChannel::Stable
                | ReleaseChannel::Canary
                | ReleaseChannel::Ptb
                | ReleaseChannel::Development
        ),
        "build_time() must return a valid ReleaseChannel"
    );
    assert_eq!(
        channel.as_str(),
        ReleaseChannel::BUILD_RELEASE_CHANNEL,
        "BUILD_RELEASE_CHANNEL must match ReleaseChannel::build_time().as_str()"
    );
}

#[test]
fn parse_release_channel_handles_stable_and_canary_case_insensitively() {
    assert_eq!(
        ReleaseChannel::parse("stable"),
        Some(ReleaseChannel::Stable)
    );
    assert_eq!(
        ReleaseChannel::parse("STABLE"),
        Some(ReleaseChannel::Stable)
    );
    assert_eq!(
        ReleaseChannel::parse("canary"),
        Some(ReleaseChannel::Canary)
    );
    assert_eq!(
        ReleaseChannel::parse("Canary"),
        Some(ReleaseChannel::Canary)
    );
    assert_eq!(ReleaseChannel::parse("ptb"), Some(ReleaseChannel::Ptb));
    assert_eq!(
        ReleaseChannel::parse("development"),
        Some(ReleaseChannel::Development)
    );
    assert_eq!(ReleaseChannel::parse("nightly"), None);
    assert_eq!(ReleaseChannel::parse(""), None);
}
