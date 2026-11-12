use chord_voice::regions::{
    RegionEndpoint, RegionError, RegionLatency, complete_probe, parse_rtc_regions, plan_probes,
    ranked_region_names, select_measured_region,
};

const FIXTURE: &str = r#"[{"region":"frankfurt","ips":["104.29.146.223:19308","104.29.147.107:19322","104.29.147.224:19341","104.29.147.94:19341","104.29.147.162:19338"]},{"region":"brussels","ips":["104.29.159.158:19306","104.29.154.190:19301","104.29.154.187:19335","104.29.154.193:19295","104.29.154.192:19329"]},{"region":"rotterdam","ips":["104.29.153.173:19315","104.29.153.114:19295","104.29.153.195:19310","104.29.157.194:19313","104.29.153.123:19333"]},{"region":"paris","ips":["104.29.151.114:19302","104.29.159.102:19314","104.29.151.87:19311","104.29.151.46:19324","104.29.144.5:19336"]},{"region":"milan","ips":["104.29.146.176:19336","104.29.146.158:19302","104.29.146.194:19317","104.29.146.45:19332","104.29.146.160:19317"]}]"#;

#[test]
fn fixture_preserves_regions_and_endpoints() {
    let parsed = parse_rtc_regions(FIXTURE).unwrap();
    assert_eq!(parsed.len(), 5);
    assert_eq!(
        parsed.iter().map(|r| r.region.as_str()).collect::<Vec<_>>(),
        ["frankfurt", "brussels", "rotterdam", "paris", "milan"]
    );
    assert!(parsed.iter().all(|r| r.ips.len() == 5));
}

#[test]
fn parser_rejects_shape_and_duplicates() {
    assert_eq!(parse_rtc_regions("[]"), Err(RegionError::Empty));
    assert_eq!(
        parse_rtc_regions(
            "[{\"region\":\"x\",\"ips\":[\"1.1.1.1:1\"]},{\"region\":\"x\",\"ips\":[\"1.1.1.1:2\"]}]"
        ),
        Err(RegionError::DuplicateRegion("x".into()))
    );
    assert_eq!(parse_rtc_regions("{"), Err(RegionError::InvalidJson));
    assert_eq!(parse_rtc_regions("{}"), Err(RegionError::InvalidShape));
    assert_eq!(
        parse_rtc_regions("[{\"ips\":[]}]"),
        Err(RegionError::MissingRegion)
    );
    assert_eq!(
        parse_rtc_regions("[{\"region\":\"x\"}]"),
        Err(RegionError::MissingIps)
    );
}

#[test]
fn parser_validates_endpoints_and_deduplicates() {
    for endpoint in [
        "1.1.1.1",
        "1.1.1.1:0",
        "1.1.1.1:70000",
        "udp://1.1.1.1:1",
        "1.1.1.1:+1",
        "1.1.1.1:01",
    ] {
        assert!(matches!(
            parse_rtc_regions(&format!("[{{\"region\":\"x\",\"ips\":[\"{endpoint}\"]}}]")),
            Err(RegionError::InvalidEndpoint(_))
        ));
    }
    let parsed =
        parse_rtc_regions(r#"[{"region":"x","ips":["[::1]:19308","[::1]:19308"]}]"#).unwrap();
    assert_eq!(parsed[0].ips, ["[::1]:19308"]);
}

#[test]
fn parser_rejects_more_than_thirty_two_endpoints_per_region() {
    let ips = (1..=33)
        .map(|last| format!("\"1.1.1.{last}:1\""))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!("[{{\"region\":\"x\",\"ips\":[{ips}]}}]");
    assert_eq!(parse_rtc_regions(&json), Err(RegionError::TooManyEndpoints));
}

#[test]
fn latency_median_is_sorted_and_even_rounds_down() {
    let mut latency = RegionLatency::new();
    for value in [9, 1, 5] {
        latency.record("x", value);
    }
    assert_eq!(latency.samples("x"), &[9, 1, 5]);
    assert_eq!(latency.median_ms("x"), Some(5));
    latency.record("x", 3);
    assert_eq!(latency.median_ms("x"), Some(3));
}

#[test]
fn ranking_matches_engine_log_and_ties_use_geo_then_lexical() {
    let geo = ["frankfurt", "brussels", "rotterdam", "paris", "milan"].map(str::to_owned);
    let mut latency = RegionLatency::new();
    for (region, rtt) in [
        ("frankfurt", 45),
        ("rotterdam", 55),
        ("paris", 60),
        ("milan", 70),
        ("brussels", 90),
    ] {
        latency.record(region, rtt);
    }
    assert_eq!(
        ranked_region_names(&latency, &geo),
        ["frankfurt", "rotterdam", "paris", "milan", "brussels"]
    );
    let mut tie = RegionLatency::new();
    tie.record("z", 10);
    tie.record("a", 10);
    assert_eq!(
        ranked_region_names(&tie, &["z".into(), "a".into(), "geo-only".into()]),
        ["z", "a", "geo-only"]
    );
}

#[test]
fn unmeasured_regions_follow_measured() {
    let mut latency = RegionLatency::new();
    latency.record("b", 10);
    assert_eq!(
        ranked_region_names(&latency, &["a".into(), "b".into(), "c".into()]),
        ["b", "a", "c"]
    );
}

#[test]
fn selection_checks_manual_against_union() {
    let mut latency = RegionLatency::new();
    latency.record("measured", 3);
    assert_eq!(
        select_measured_region(&latency, &["geo".into()], Some("measured")).unwrap(),
        "measured"
    );
    assert_eq!(
        select_measured_region(&latency, &["geo".into()], Some("geo")).unwrap(),
        "geo"
    );
    assert!(select_measured_region(&latency, &["geo".into()], Some("missing")).is_err());
    assert_eq!(
        select_measured_region(&latency, &["geo".into()], None).unwrap(),
        "measured"
    );
    assert_eq!(
        select_measured_region(&RegionLatency::new(), &["geo".into()], None).unwrap(),
        "geo"
    );
    assert!(select_measured_region(&RegionLatency::new(), &[], None).is_err());
}

#[test]
fn probes_rotate_endpoints_and_limit_per_region() {
    let regions = vec![RegionEndpoint {
        region: "x".into(),
        ips: vec!["1.1.1.1:1".into(), "1.1.1.1:2".into(), "1.1.1.1:3".into()],
    }];
    assert_eq!(
        plan_probes(&regions, 0, 100, 3)
            .iter()
            .map(|p| p.endpoint.as_str())
            .collect::<Vec<_>>(),
        ["1.1.1.1:1", "1.1.1.1:2", "1.1.1.1:3"]
    );
    assert_eq!(
        plan_probes(&regions, 2, 100, 2)
            .iter()
            .map(|p| p.endpoint.as_str())
            .collect::<Vec<_>>(),
        ["1.1.1.1:3", "1.1.1.1:1"]
    );
    assert!(plan_probes(&regions, 0, 100, 0).is_empty());
}

#[test]
fn probe_completion_observes_timeout_and_clock_order() {
    let probe = plan_probes(
        &[RegionEndpoint {
            region: "x".into(),
            ips: vec!["1.1.1.1:1".into()],
        }],
        0,
        100,
        1,
    )
    .remove(0);
    assert_eq!(complete_probe(&probe, 145, 50), Some(45));
    assert_eq!(complete_probe(&probe, 151, 50), None);
    assert_eq!(complete_probe(&probe, 99, 50), None);
}
