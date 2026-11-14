//! Контрактные тесты H-FIX-6 (async `RwLock` для `DaemonState.messages`) и
//! H-FIX-7 (полное чтение HTTP/1.1 тела по `Content-Length` без обрезки на 4096 байтах).

use std::collections::HashMap;
use std::time::Duration;

use chord_gpui_e2e_harness::{
    CaptureReadyTrace, HydratedChannel, HydratedGuild, HydratedUser, MockGatewayDaemon,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn synthetic_trace() -> CaptureReadyTrace {
    let user = HydratedUser {
        id: "384299970632876032".to_string(),
        username: "tester".to_string(),
        global_name: Some("Tester".to_string()),
        discriminator: "0".to_string(),
        avatar: None,
    };
    let channel_id = "1163408591315812392".to_string();
    let guild_id = "1014537851418980372".to_string();
    let guild = HydratedGuild {
        id: guild_id.clone(),
        name: "Test Guild".to_string(),
        icon: None,
        owner_id: Some(user.id.clone()),
        channels: vec![HydratedChannel {
            id: channel_id.clone(),
            guild_id: guild_id.clone(),
            name: "general".to_string(),
            kind: 0,
            position: 0,
            topic: None,
        }],
    };
    CaptureReadyTrace {
        gateway_request_id: "req-1".to_string(),
        gateway_url: "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream".to_string(),
        attached_guild_id: guild_id,
        attached_channel_id: channel_id,
        heartbeat_interval_ms: 41_250,
        session_id: "sess-1".to_string(),
        current_user: user,
        guilds: vec![guild],
        messages_by_channel: HashMap::new(),
        raw_wsrecv_frames: Vec::new(),
        hello_etf_frame: vec![0x83],
        ready_etf_frame: vec![0x83],
    }
}

async fn read_http1_response(stream: &mut TcpStream) -> (u16, Vec<u8>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut tmp).await.expect("read http1 response");
        assert!(
            n > 0,
            "unexpected EOF while reading HTTP/1.1 response headers"
        );
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let head_str = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = head_str.lines();
    let status_line = lines.next().expect("status line");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .expect("numeric status");

    let mut content_length = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = v.trim().parse().expect("valid content-length");
        }
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream
            .read(&mut tmp)
            .await
            .expect("read http1 response body");
        assert!(n > 0, "unexpected EOF while reading HTTP/1.1 response body");
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);
    (status, body)
}

#[tokio::test]
async fn hfix7_http1_post_large_body_over_16kib_and_fragmented_tcp_writes_succeed() {
    let mut daemon = MockGatewayDaemon::start(synthetic_trace())
        .await
        .expect("start MockGatewayDaemon");
    let addr = daemon.addr();
    let channel_id = "1163408591315812392";

    // Build a ~20 KiB JSON body where `padding` (18 KiB) precedes the 1,900-char `content`.
    // Any truncation at 4096 bytes cuts off before `content` and produces invalid JSON.
    let expected_content: String = (0..1900)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let padding = "P".repeat(18_600);
    let body_str = format!(
        r#"{{"nonce":"large-body-nonce-01","padding":"{padding}","content":"{expected_content}"}}"#
    );
    let body_bytes = body_str.as_bytes();
    assert!(
        body_bytes.len() > 20 * 1024,
        "Payload must exceed 20 KiB, got {} bytes",
        body_bytes.len()
    );

    let req_head = format!(
        "POST /api/v9/channels/{channel_id}/messages HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Content-Type: application/json\r\n\
         content-length: {}\r\n\
         \r\n",
        body_bytes.len()
    );

    // Send across two fragmented TCP writes with a small delay to exercise multi-read loop
    let mut stream = TcpStream::connect(addr).await.expect("connect tcp");
    let mut first_chunk = req_head.into_bytes();
    let split_at = 1024;
    first_chunk.extend_from_slice(&body_bytes[..split_at]);

    stream
        .write_all(&first_chunk)
        .await
        .expect("write first chunk");
    stream.flush().await.expect("flush first chunk");
    tokio::time::sleep(Duration::from_millis(15)).await;

    stream
        .write_all(&body_bytes[split_at..])
        .await
        .expect("write remaining body chunk");
    stream.flush().await.expect("flush second chunk");

    let (post_status, post_body) = read_http1_response(&mut stream).await;
    assert!(
        post_status == 200 || post_status == 201,
        "Expected 200/201 for 20 KiB HTTP/1.1 POST, got {post_status}: {}",
        String::from_utf8_lossy(&post_body)
    );
    let post_json: serde_json::Value =
        serde_json::from_slice(&post_body).expect("valid POST response JSON");
    assert_eq!(
        post_json["content"].as_str(),
        Some(expected_content.as_str())
    );

    // Verify via subsequent HTTP/1.1 GET /api/v9/channels/<channel_id>/messages
    let mut get_stream = TcpStream::connect(addr).await.expect("connect tcp for GET");
    let get_req =
        format!("GET /api/v9/channels/{channel_id}/messages HTTP/1.1\r\nHost: {addr}\r\n\r\n");
    get_stream
        .write_all(get_req.as_bytes())
        .await
        .expect("write GET");
    let (get_status, get_body) = read_http1_response(&mut get_stream).await;
    assert_eq!(get_status, 200);
    let msgs: Vec<serde_json::Value> =
        serde_json::from_slice(&get_body).expect("parse GET messages JSON");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["content"].as_str(), Some(expected_content.as_str()));

    daemon.shutdown().await.expect("shutdown daemon");
}

#[tokio::test]
async fn hfix6_concurrent_get_and_post_requests_succeed_without_data_loss_or_deadlock() {
    let mut daemon = MockGatewayDaemon::start(synthetic_trace())
        .await
        .expect("start MockGatewayDaemon");
    let addr = daemon.addr();
    let channel_id = "1163408591315812392";

    let writer_count = 12usize;
    let reader_count = 12usize;
    let mut handles = Vec::new();

    for i in 0..writer_count {
        handles.push(tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.expect("writer connect");
            let pad = "X".repeat(6000);
            let content = format!("concurrent-msg-{i:02}-{}", "m".repeat(500));
            let body = format!(r#"{{"pad":"{pad}","content":"{content}"}}"#);
            let req = format!(
                "POST /api/v9/channels/{channel_id}/messages HTTP/1.1\r\n\
                 Host: {addr}\r\n\
                 Content-Length: {}\r\n\
                 \r\n\
                 {body}",
                body.len()
            );
            stream.write_all(req.as_bytes()).await.expect("writer send");
            let (status, resp_body) = read_http1_response(&mut stream).await;
            assert!(
                status == 200 || status == 201,
                "Concurrent POST #{i} failed with {status}: {}",
                String::from_utf8_lossy(&resp_body)
            );
        }));
    }

    for _ in 0..reader_count {
        handles.push(tokio::spawn(async move {
            for _ in 0..3 {
                let mut stream = TcpStream::connect(addr).await.expect("reader connect");
                let req = format!(
                    "GET /api/v9/channels/{channel_id}/messages HTTP/1.1\r\nHost: {addr}\r\n\r\n"
                );
                stream.write_all(req.as_bytes()).await.expect("reader send");
                let (status, resp_body) = read_http1_response(&mut stream).await;
                assert_eq!(status, 200);
                let arr: Vec<serde_json::Value> =
                    serde_json::from_slice(&resp_body).expect("reader valid JSON");
                assert!(arr.len() <= writer_count);
            }
        }));
    }

    for h in handles {
        h.await.expect("task join");
    }

    let mut final_stream = TcpStream::connect(addr).await.expect("final GET connect");
    let get_req =
        format!("GET /api/v9/channels/{channel_id}/messages HTTP/1.1\r\nHost: {addr}\r\n\r\n");
    final_stream
        .write_all(get_req.as_bytes())
        .await
        .expect("final GET write");
    let (status, body) = read_http1_response(&mut final_stream).await;
    assert_eq!(status, 200);
    let msgs: Vec<serde_json::Value> = serde_json::from_slice(&body).expect("final JSON");
    assert_eq!(
        msgs.len(),
        writer_count,
        "All {writer_count} concurrent POSTs must be stored without data loss"
    );

    daemon.shutdown().await.expect("shutdown daemon");
}

#[tokio::test]
async fn hfix7_truncated_eof_mid_body_does_not_panic_and_daemon_stays_healthy() {
    let mut daemon = MockGatewayDaemon::start(synthetic_trace())
        .await
        .expect("start MockGatewayDaemon");
    let addr = daemon.addr();
    let channel_id = "1163408591315812392";

    // Connect and declare Content-Length: 25000, but drop write-side after sending only a partial body.
    // A compliant Content-Length reader must treat early EOF as an IO error and close without dispatching.
    {
        let mut bad_stream = TcpStream::connect(addr).await.expect("connect truncated");
        let partial_req = format!(
            "POST /api/v9/channels/{channel_id}/messages HTTP/1.1\r\n\
             Host: {addr}\r\n\
             Content-Length: 25000\r\n\
             \r\n\
             {{\"content\":\"truncated-eof\"}}"
        );
        bad_stream
            .write_all(partial_req.as_bytes())
            .await
            .expect("write partial");
        bad_stream.shutdown().await.expect("shutdown write half");
        let mut resp_buf = Vec::new();
        let _ = bad_stream.read_to_end(&mut resp_buf).await;
        assert!(
            resp_buf.is_empty(),
            "Truncated EOF before Content-Length must abort with IO error instead of dispatching partial body, got: {}",
            String::from_utf8_lossy(&resp_buf)
        );
    }

    tokio::time::sleep(Duration::from_millis(20)).await;

    // Verify daemon is still alive and serves subsequent requests normally
    let mut good_stream = TcpStream::connect(addr).await.expect("connect healthy");
    let body = r#"{"content":"healthy-after-truncated-eof"}"#;
    let good_req = format!(
        "POST /api/v9/channels/{channel_id}/messages HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {body}",
        body.len()
    );
    good_stream
        .write_all(good_req.as_bytes())
        .await
        .expect("write good req");
    let (status, resp_body) = read_http1_response(&mut good_stream).await;
    assert_eq!(status, 200);
    let msg: serde_json::Value = serde_json::from_slice(&resp_body).expect("good json");
    assert_eq!(msg["content"].as_str(), Some("healthy-after-truncated-eof"));
    let msg_id = msg["id"].as_str().expect("msg id").to_string();

    // Verify PATCH and DELETE message mutations over async RwLock::write().await
    let mut patch_stream = TcpStream::connect(addr).await.expect("connect patch");
    let patch_body = format!(
        r#"{{"padding":"{}","content":"patched-over-rwlock"}}"#,
        "Z".repeat(16_500)
    );
    let patch_req = format!(
        "PATCH /api/v9/channels/{channel_id}/messages/{msg_id} HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {patch_body}",
        patch_body.len()
    );
    patch_stream
        .write_all(patch_req.as_bytes())
        .await
        .expect("write patch");
    let (patch_status, patch_resp) = read_http1_response(&mut patch_stream).await;
    assert_eq!(patch_status, 200);
    let patched_json: serde_json::Value = serde_json::from_slice(&patch_resp).expect("patch json");
    assert_eq!(
        patched_json["content"].as_str(),
        Some("patched-over-rwlock")
    );

    let mut del_stream = TcpStream::connect(addr).await.expect("connect delete");
    let del_req = format!(
        "DELETE /api/v9/channels/{channel_id}/messages/{msg_id} HTTP/1.1\r\nHost: {addr}\r\n\r\n"
    );
    del_stream
        .write_all(del_req.as_bytes())
        .await
        .expect("write delete");
    let (del_status, _) = read_http1_response(&mut del_stream).await;
    assert_eq!(del_status, 204);

    daemon.shutdown().await.expect("shutdown daemon");
}
