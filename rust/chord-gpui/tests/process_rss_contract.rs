#[path = "../src/process_rss.rs"]
mod process_rss;

use process_rss::{current_process_rss_kb, parse_proc_status, read_proc_status};

#[test]
fn proc_status_reads_the_real_vmrss_field_in_kibibytes() {
    let status = "Name:\tchord\nVmSize:\t987654 kB\nVmRSS:\t65432 kB\nThreads:\t1\n";
    assert_eq!(parse_proc_status(status).expect("valid VmRSS"), 65_432);
}

#[test]
fn missing_proc_file_returns_an_explicit_measurement_error() {
    let path = std::env::temp_dir()
        .join(format!("chord-rss-missing-{}", std::process::id()))
        .join("status");
    assert!(!path.exists(), "the negative fixture must not exist");
    let error = read_proc_status(&path).expect_err("missing /proc must never become 1024 KiB");
    assert!(error.to_string().contains("cannot read"));
}

#[test]
fn missing_or_ambiguous_vmrss_returns_an_error() {
    for status in ["", "VmSize: 1024 kB\n", "VmRSS: 1 kB\nVmRSS: 2 kB\n"] {
        assert!(
            parse_proc_status(status).is_err(),
            "invalid status: {status:?}"
        );
    }
}

#[test]
fn invalid_zero_wrong_unit_and_overflow_values_never_pass_the_memory_budget() {
    for value in ["0", "-1", "+1", "NaN", "18446744073709551616"] {
        assert!(parse_proc_status(&format!("VmRSS: {value} kB\n")).is_err());
    }
    for status in ["VmRSS: 1024 MB", "VmRSS: 1024", "VmRSS: 1024 kB extra"] {
        assert!(
            parse_proc_status(status).is_err(),
            "invalid status: {status:?}"
        );
    }
}

#[test]
fn windows_working_set_bytes_round_up_without_underreporting() {
    for (bytes, kibibytes) in [("1", 1), ("1024", 1), ("1025", 2), ("1048576", 1024)] {
        assert_eq!(
            process_rss::working_set_bytes_to_kb(bytes).expect("valid bytes"),
            kibibytes
        );
    }
}

#[test]
fn invalid_windows_working_set_values_return_an_error() {
    for bytes in [
        "",
        "0",
        "-1",
        "text",
        "18446744073709551615",
        "18446744073709551616",
    ] {
        assert!(
            process_rss::working_set_bytes_to_kb(bytes).is_err(),
            "invalid bytes: {bytes:?}"
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
#[test]
fn native_measurement_reports_positive_current_process_memory() {
    let measured = current_process_rss_kb().expect("native RSS measurement must be available");
    assert!(measured > 0);
    println!("native process {} RSS: {measured} KiB", std::process::id());
}

#[cfg(target_os = "linux")]
#[test]
fn linux_measurement_increases_when_resident_pages_are_touched() {
    let before = current_process_rss_kb().expect("initial RSS");
    let mut pages = vec![0_u8; 32 * 1024 * 1024];
    for page in pages.chunks_mut(4096) {
        page[0] = 123;
    }
    std::hint::black_box(&pages);
    let after = current_process_rss_kb().expect("RSS after committing pages");
    assert!(after > before + 16 * 1024, "before={before}, after={after}");
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
#[test]
fn unsupported_platform_returns_an_explicit_error() {
    assert!(current_process_rss_kb().is_err());
}
