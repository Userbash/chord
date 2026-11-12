//! Региональные RTC endpoint'ы и headless-модель latency.
//!
//! Формат latency-пробы и метрика ранжирования не подтверждены wire-capture;
//! медиана здесь является самосогласованной моделью для выбора региона.

use crate::VoiceError;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionEndpoint {
    pub region: String,
    pub ips: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionError {
    Empty,
    InvalidJson,
    InvalidShape,
    MissingRegion,
    MissingIps,
    DuplicateRegion(String),
    InvalidEndpoint(String),
    TooManyRegions,
    TooManyEndpoints,
    NoEndpoints,
}

pub fn parse_rtc_regions(json: &str) -> Result<Vec<RegionEndpoint>, RegionError> {
    let value = chord_json::parse(json).map_err(|_| RegionError::InvalidJson)?;
    let items = value.as_array().ok_or(RegionError::InvalidShape)?;
    if items.is_empty() {
        return Err(RegionError::Empty);
    }
    if items.len() > 64 {
        return Err(RegionError::TooManyRegions);
    }
    let mut seen_regions = BTreeSet::new();
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or(RegionError::InvalidShape)?;
        let region = object
            .get("region")
            .ok_or(RegionError::MissingRegion)?
            .as_str()
            .ok_or(RegionError::InvalidShape)?
            .to_owned();
        if !seen_regions.insert(region.clone()) {
            return Err(RegionError::DuplicateRegion(region));
        }
        let endpoints = object.get("ips").ok_or(RegionError::MissingIps)?;
        let endpoints = endpoints.as_array().ok_or(RegionError::InvalidShape)?;
        if endpoints.is_empty() {
            return Err(RegionError::NoEndpoints);
        }
        if endpoints.len() > 32 {
            return Err(RegionError::TooManyEndpoints);
        }
        let mut ips = Vec::with_capacity(endpoints.len());
        let mut seen_ips = BTreeSet::new();
        for endpoint in endpoints {
            let endpoint = endpoint.as_str().ok_or(RegionError::InvalidShape)?;
            validate_endpoint(endpoint)?;
            if seen_ips.insert(endpoint.to_owned()) {
                ips.push(endpoint.to_owned());
            }
        }
        if ips.is_empty() {
            return Err(RegionError::NoEndpoints);
        }
        result.push(RegionEndpoint { region, ips });
    }
    Ok(result)
}

fn validate_endpoint(endpoint: &str) -> Result<(), RegionError> {
    let invalid = || RegionError::InvalidEndpoint(endpoint.into());
    let (host, port, bracketed) = if let Some(rest) = endpoint.strip_prefix('[') {
        let close = rest.find(']').ok_or_else(invalid)?;
        let host = &rest[..close];
        let port = rest[close + 1..].strip_prefix(':').ok_or_else(invalid)?;
        if host.is_empty() || host.contains(['[', ']']) {
            return Err(invalid());
        }
        (host, port, true)
    } else {
        let (host, port) = endpoint.rsplit_once(':').ok_or_else(invalid)?;
        if host.is_empty() || host.contains(':') {
            return Err(invalid());
        }
        (host, port, false)
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let port_text = port;
    let port = port_text.parse::<u32>().map_err(|_| invalid())?;
    if port_text != port.to_string() || !(1..=65_535).contains(&port) {
        return Err(invalid());
    }
    if bracketed {
        host.parse::<std::net::Ipv6Addr>().map_err(|_| invalid())?;
    } else {
        host.parse::<std::net::Ipv4Addr>().map_err(|_| invalid())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct RegionLatency {
    samples: BTreeMap<String, Vec<u64>>,
}

impl RegionLatency {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, region: &str, rtt_ms: u64) {
        self.samples
            .entry(region.to_owned())
            .or_default()
            .push(rtt_ms);
    }

    pub fn median_ms(&self, region: &str) -> Option<u64> {
        let mut values = self.samples.get(region)?.clone();
        if values.is_empty() {
            return None;
        }
        values.sort_unstable();
        Some(values[(values.len() - 1) / 2])
    }

    pub fn samples(&self, region: &str) -> &[u64] {
        self.samples.get(region).map_or(&[], Vec::as_slice)
    }

    pub fn measured(&self) -> Vec<(String, u32)> {
        self.samples
            .keys()
            .filter_map(|region| {
                self.median_ms(region)
                    .and_then(|v| u32::try_from(v).ok().map(|v| (region.clone(), v)))
            })
            .collect()
    }
}

pub fn ranked_region_names(latency: &RegionLatency, geo: &[String]) -> Vec<String> {
    let mut names = BTreeSet::new();
    names.extend(geo.iter().cloned());
    names.extend(latency.samples.keys().cloned());
    let geo_pos = |name: &str| {
        geo.iter()
            .position(|item| item == name)
            .unwrap_or(usize::MAX)
    };
    let mut names: Vec<_> = names.into_iter().collect();
    names.sort_by(|left, right| {
        let lm = latency.median_ms(left);
        let rm = latency.median_ms(right);
        lm.is_none()
            .cmp(&rm.is_none())
            .then_with(|| lm.cmp(&rm))
            .then_with(|| geo_pos(left).cmp(&geo_pos(right)))
            .then_with(|| left.cmp(right))
    });
    names
}

pub fn select_measured_region(
    latency: &RegionLatency,
    geo: &[String],
    manual: Option<&str>,
) -> Result<String, VoiceError> {
    crate::select_region(&latency.measured(), geo, manual)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionProbe {
    pub region: String,
    pub endpoint: String,
    pub sequence: u64,
    pub sent_at_ms: u64,
}

pub fn plan_probes(
    regions: &[RegionEndpoint],
    sequence: u64,
    now_ms: u64,
    max_per_region: usize,
) -> Vec<RegionProbe> {
    if max_per_region == 0 {
        return Vec::new();
    }
    regions
        .iter()
        .filter(|r| !r.ips.is_empty())
        .flat_map(|region| {
            (0..max_per_region).map(move |offset| {
                let seq = sequence + offset as u64;
                RegionProbe {
                    region: region.region.clone(),
                    endpoint: region.ips[(seq as usize) % region.ips.len()].clone(),
                    sequence: seq,
                    sent_at_ms: now_ms,
                }
            })
        })
        .collect()
}

pub fn complete_probe(probe: &RegionProbe, received_at_ms: u64, timeout_ms: u64) -> Option<u64> {
    let rtt = received_at_ms.checked_sub(probe.sent_at_ms)?;
    (rtt <= timeout_ms).then_some(rtt)
}
