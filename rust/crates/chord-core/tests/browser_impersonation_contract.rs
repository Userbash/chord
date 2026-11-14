//! Контрактные тесты browser impersonation headers для `ApiClient` (P2-1.1).

use std::time::Duration;

use chord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy, HeaderConfig};
use chord_core::rest::headers::{
    BrowserImpersonationProfile, DEFAULT_BROWSER_USER_AGENT, DEFAULT_ORIGIN, DEFAULT_REFERER,
    DEFAULT_SEC_CH_UA, DEFAULT_SEC_CH_UA_MOBILE, DEFAULT_SEC_CH_UA_PLATFORM,
    DEFAULT_SEC_FETCH_DEST, DEFAULT_SEC_FETCH_MODE, DEFAULT_SEC_FETCH_SITE,
};
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{HttpClientConfig, HttpRequest};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const CASE_TIMEOUT: Duration = Duration::from_secs(2);

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

async fn read_request<S: AsyncRead + Unpin>(stream: &mut S) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(head_end) = find(&buffer, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..head_end]);
            let content_length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buffer.len() >= head_end + 4 + content_length {
                return buffer;
            }
        }
        let read = stream.read(&mut chunk).await.expect("чтение запроса");
        if read == 0 {
            return buffer;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

async fn spawn_once(response: Vec<u8>) -> (u16, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let request = read_request(&mut stream).await;
        stream.write_all(&response).await.expect("write");
        stream.shutdown().await.expect("shutdown");
        request
    });
    (port, handle)
}

fn ok_response() -> Vec<u8> {
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec()
}

fn test_config(port: u16, auth: ApiAuth) -> ApiConfig {
    ApiConfig {
        base: format!("http://127.0.0.1:{port}/api/v9"),
        auth,
        http: HttpClientConfig::default(),
        retry: ApiRetryPolicy {
            max_attempts: 1,
            max_retry_after: Duration::from_secs(1),
            backoff: Duration::from_millis(10),
            attempt_timeout: Duration::from_secs(2),
        },
    }
}

#[test]
fn browser_impersonation_constants_and_desktop_profile_match_contract() {
    let profile = BrowserImpersonationProfile::chrome_linux_desktop();
    assert_eq!(profile, BrowserImpersonationProfile::desktop_default());

    assert_eq!(profile.user_agent, DEFAULT_BROWSER_USER_AGENT);
    assert!(profile.user_agent.contains("Chrome/"));
    assert!(profile.user_agent.contains("Electron/"));

    assert_eq!(profile.sec_ch_ua, DEFAULT_SEC_CH_UA);
    assert!(!profile.sec_ch_ua.is_empty());

    assert_eq!(profile.sec_ch_ua_mobile, DEFAULT_SEC_CH_UA_MOBILE);
    assert_eq!(profile.sec_ch_ua_mobile, "?0");

    assert_eq!(profile.sec_ch_ua_platform, DEFAULT_SEC_CH_UA_PLATFORM);
    assert_eq!(profile.sec_ch_ua_platform, "\"Linux\"");

    assert_eq!(profile.sec_fetch_dest, DEFAULT_SEC_FETCH_DEST);
    assert_eq!(profile.sec_fetch_dest, "empty");

    assert_eq!(profile.sec_fetch_mode, DEFAULT_SEC_FETCH_MODE);
    assert_eq!(profile.sec_fetch_mode, "cors");

    assert_eq!(profile.sec_fetch_site, DEFAULT_SEC_FETCH_SITE);
    assert_eq!(profile.sec_fetch_site, "same-origin");

    assert_eq!(profile.origin, DEFAULT_ORIGIN);
    assert_eq!(profile.origin, "https://discord.com");

    assert_eq!(profile.referer, DEFAULT_REFERER);
    assert_eq!(profile.referer, "https://discord.com/channels/@me");
}

#[tokio::test]
async fn api_client_sends_browser_impersonation_headers_with_auth_and_fingerprint() {
    let (port, server) = spawn_once(ok_response()).await;
    let sp = SuperProperties::default();
    let auth = ApiAuth::from_token_with_locales("user-token-123", &sp, &["en-US", "en"], "UTC")
        .expect("valid auth");

    let headers = HeaderConfig::new().with_fingerprint("fp-abc-123");
    let client = ApiClient::new_with_headers(test_config(port, auth), headers)
        .expect("client")
        .with_browser_impersonation(BrowserImpersonationProfile::chrome_linux_desktop());

    let response = timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("response");
    assert_eq!(response.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw);

    for expected in [
        &format!("User-Agent: {DEFAULT_BROWSER_USER_AGENT}\r\n"),
        &format!("Sec-CH-UA: {DEFAULT_SEC_CH_UA}\r\n"),
        "Sec-CH-UA-Mobile: ?0\r\n",
        "Sec-CH-UA-Platform: \"Linux\"\r\n",
        "Sec-Fetch-Dest: empty\r\n",
        "Sec-Fetch-Mode: cors\r\n",
        "Sec-Fetch-Site: same-origin\r\n",
        "Origin: https://discord.com\r\n",
        "Referer: https://discord.com/channels/@me\r\n",
        "Authorization: user-token-123\r\n",
        "X-Super-Properties: ",
        "Accept-Language: en-US,en;q=0.9\r\n",
        "X-Discord-Locale: en-US\r\n",
        "X-Discord-Timezone: UTC\r\n",
        "X-Fingerprint: fp-abc-123\r\n",
    ] {
        assert!(
            text.contains(expected),
            "missing header `{expected}` in:\n{text}"
        );
    }
}

#[tokio::test]
async fn explicit_per_request_headers_override_without_duplication() {
    let (port, server) = spawn_once(ok_response()).await;
    let sp = SuperProperties::default();
    let auth = ApiAuth::from_token("user-token-123", &sp, "en-US", "UTC").expect("valid auth");
    let client = ApiClient::new(test_config(port, auth)).expect("client");

    let request = HttpRequest::new(HttpMethod::Get, "/users/@me")
        .header("user-agent", "CustomAgent/9.9")
        .header("referer", "https://discord.com/login")
        .header("SEC-FETCH-SITE", "cross-site");

    let response = timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect("response");
    assert_eq!(response.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw);
    let lower = text.to_ascii_lowercase();

    assert_eq!(lower.matches("\r\nuser-agent:").count(), 1);
    assert!(text.contains("user-agent: CustomAgent/9.9\r\n"));

    assert_eq!(lower.matches("\r\nreferer:").count(), 1);
    assert!(text.contains("referer: https://discord.com/login\r\n"));

    assert_eq!(lower.matches("\r\nsec-fetch-site:").count(), 1);
    assert!(text.contains("SEC-FETCH-SITE: cross-site\r\n"));
}

#[tokio::test]
async fn anonymous_request_emits_browser_headers_without_authorization() {
    let (port, server) = spawn_once(ok_response()).await;
    let sp = SuperProperties::default();
    let auth = ApiAuth::anonymous(&sp, "en-US", "UTC").expect("anonymous auth");
    let client = ApiClient::new(test_config(port, auth)).expect("client");

    let response = timeout(CASE_TIMEOUT, client.get("/experiments"))
        .await
        .expect("timeout")
        .expect("response");
    assert_eq!(response.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw);
    let lower = text.to_ascii_lowercase();

    assert!(!lower.contains("\r\nauthorization:"));
    assert!(text.contains(&format!("User-Agent: {DEFAULT_BROWSER_USER_AGENT}\r\n")));
    assert!(text.contains("Sec-CH-UA-Mobile: ?0\r\n"));
    assert!(text.contains("Origin: https://discord.com\r\n"));
    assert!(text.contains("Referer: https://discord.com/channels/@me\r\n"));
}

#[tokio::test]
async fn without_browser_impersonation_omits_browser_headers() {
    let (port, server) = spawn_once(ok_response()).await;
    let sp = SuperProperties::default();
    let auth = ApiAuth::from_token("user-token-123", &sp, "en-US", "UTC").expect("valid auth");
    let client = ApiClient::new(test_config(port, auth))
        .expect("client")
        .without_browser_impersonation();

    let response = timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("response");
    assert_eq!(response.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw);

    assert!(!text.contains("Sec-CH-UA:"));
    assert!(!text.contains("Sec-Fetch-Dest:"));
    assert!(!text.contains("Origin:"));
    assert!(!text.contains("Referer:"));
}
