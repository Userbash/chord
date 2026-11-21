//! HTTP/1.1-фрейминг без I/O (волна RW-10 / W10a).
//!
//! **Провенанс оракулов** (независимые источники):
//! - RFC 9112 §6 (framing), §7.1 (пример chunked `Wikipedia`), RFC 9110 §5 (поля);
//! - `docs/re/rest-d02.md` §3: REST Discord — HTTP/1.1, тела небольшие, `Content-Length`.
//!
//! Тесты сверяют точные байты запроса и разбор ответа, а не подгоняются под реализацию.

use chord_core::rest::http1::{
    ChunkedParse, DEFAULT_USER_AGENT, HeadParse, Http1Error, MAX_CHUNK_LINE_BYTES, MAX_HEAD_BYTES,
    MAX_TRAILER_BYTES, content_length, decode_chunked, decode_deflate, decode_gzip,
    decode_response_body, decode_response_body_with_limit, encode_request, header, is_chunked,
    parse_response_head,
};
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::transport::{self, HttpRequest};

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

// ---------------------------------------------------------------------------
// Кодирование запроса
// ---------------------------------------------------------------------------

#[test]
fn encode_request_get_has_no_content_length() {
    let bytes = encode_request(
        HttpMethod::Get,
        "/users/@me",
        &[pair("Host", "discord.com"), pair("Authorization", "Bot x")],
        b"",
    )
    .expect("GET кодируется");
    assert_eq!(
        bytes,
        b"GET /users/@me HTTP/1.1\r\nHost: discord.com\r\nAuthorization: Bot x\r\n\r\n"
    );
}

#[test]
fn encode_request_post_adds_content_length() {
    let bytes = encode_request(
        HttpMethod::Post,
        "/channels/1/messages",
        &[pair("Host", "discord.com")],
        br#"{"a":12}"#,
    )
    .expect("POST кодируется");
    assert_eq!(
        bytes,
        b"POST /channels/1/messages HTTP/1.1\r\nHost: discord.com\r\nContent-Length: 8\r\n\r\n{\"a\":12}"
    );
}

#[test]
fn encode_request_rejects_injection_and_bad_target() {
    for target in ["relative", "/x\r\nHost: evil", "/x y", "/x#frag"] {
        assert!(
            matches!(
                encode_request(HttpMethod::Get, target, &[], b""),
                Err(chord_core::rest::http1::Http1Error::InvalidHeader(_))
            ),
            "{target:?}: недопустимый target обязан быть InvalidHeader"
        );
    }

    for headers in [
        vec![pair("Bad Name", "v")],
        vec![pair("X", "a\r\nY: b")],
        vec![pair("", "v")],
        vec![pair("X:", "v")],
    ] {
        assert!(
            matches!(
                encode_request(HttpMethod::Get, "/x", &headers, b""),
                Err(chord_core::rest::http1::Http1Error::InvalidHeader(_))
            ),
            "{headers:?}: недопустимое поле обязано быть InvalidHeader"
        );
    }
}

#[test]
fn encode_request_rejects_transfer_encoding() {
    let error = encode_request(
        HttpMethod::Post,
        "/channels/1/messages",
        &[pair("Transfer-Encoding", "chunked")],
        b"hello",
    )
    .expect_err("клиент не кодирует chunked-запрос");
    assert!(matches!(error, Http1Error::InvalidHeader(_)));
}

#[test]
fn encode_request_checks_supplied_content_length() {
    let headers = [pair("Content-Length", "1")];
    assert!(matches!(
        encode_request(HttpMethod::Post, "/x", &headers, b"ab"),
        Err(chord_core::rest::http1::Http1Error::InvalidContentLength(_))
    ));

    let headers = [pair("Content-Length", "2")];
    let bytes = encode_request(HttpMethod::Post, "/x", &headers, b"ab").expect("совпадает");
    assert!(bytes.ends_with(b"\r\n\r\nab"));
}

// ---------------------------------------------------------------------------
// Разбор заголовка ответа
// ---------------------------------------------------------------------------

#[test]
fn parse_head_200() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n";
    match parse_response_head(raw).expect("заголовок") {
        HeadParse::Complete {
            status,
            headers,
            consumed,
        } => {
            assert_eq!(status, 200);
            assert_eq!(consumed, raw.len());
            assert_eq!(
                headers,
                vec![
                    pair("Content-Type", "application/json"),
                    pair("Content-Length", "2")
                ]
            );
        }
        HeadParse::Incomplete => panic!("полный заголовок не может быть Incomplete"),
    }
}

#[test]
fn parse_head_incomplete_and_limit() {
    assert_eq!(
        parse_response_head(b"HTTP/1.1 200 OK\r\nContent-Len").expect("префикс"),
        HeadParse::Incomplete
    );

    let mut huge = b"HTTP/1.1 200 OK\r\n".to_vec();
    huge.extend(std::iter::repeat_n(b'X', MAX_HEAD_BYTES + 1));
    assert!(matches!(
        parse_response_head(&huge),
        Err(chord_core::rest::http1::Http1Error::HeadTooLarge)
    ));
}

#[test]
fn parse_head_versions_and_malformed() {
    let ok10 = b"HTTP/1.0 204 No Content\r\n\r\n";
    assert_eq!(
        parse_response_head(ok10),
        Ok(HeadParse::Complete {
            status: 204,
            headers: Vec::new(),
            consumed: ok10.len(),
        })
    );

    assert!(matches!(
        parse_response_head(b"HTTP/2 200 OK\r\n\r\n"),
        Err(Http1Error::UnsupportedVersion(version)) if version == "HTTP/2"
    ));

    for raw in [
        &b"garbage\r\n\r\n"[..],
        &b"HTTP/1.1 abc OK\r\n\r\n"[..],
        &b"HTTP/1.1 200 OK\r\nBroken\r\n\r\n"[..],
        &b"HTTP/1.1 200 OK\r\n Folding: yes\r\n\r\n"[..],
    ] {
        assert!(
            matches!(parse_response_head(raw), Err(Http1Error::MalformedHead(_))),
            "{:?}: обязан быть MalformedHead",
            String::from_utf8_lossy(raw)
        );
    }
}

#[test]
fn header_lookup_is_case_insensitive() {
    let headers = [pair("Retry-After", "1"), pair("X-Test", "a")];
    assert_eq!(header(&headers, "retry-after"), Some("1"));
    assert_eq!(header(&headers, "x-test"), Some("a"));
    assert_eq!(header(&headers, "missing"), None);
}

#[test]
fn content_length_rules() {
    assert_eq!(content_length(&[]), Ok(None));
    assert_eq!(content_length(&[pair("Content-Length", "5")]), Ok(Some(5)));
    assert_eq!(
        content_length(&[pair("Content-Length", "5"), pair("content-length", "5")]),
        Ok(Some(5))
    );
    assert!(matches!(
        content_length(&[pair("Content-Length", "5"), pair("Content-Length", "6")]),
        Err(Http1Error::InvalidContentLength(_))
    ));
    for bad in ["abc", "-1", "1.5", "+5", "5 5", ""] {
        assert!(
            matches!(
                content_length(&[pair("Content-Length", bad)]),
                Err(Http1Error::InvalidContentLength(_))
            ),
            "{bad:?}: обязан быть InvalidContentLength"
        );
    }
}

#[test]
fn chunked_flag_rules() {
    assert_eq!(is_chunked(&[]), Ok(false));
    assert_eq!(
        is_chunked(&[pair("Transfer-Encoding", "chunked")]),
        Ok(true)
    );
    assert_eq!(
        is_chunked(&[pair("Transfer-Encoding", "gzip, chunked")]),
        Err(Http1Error::UnsupportedTransferEncoding(
            "gzip,chunked".to_string()
        ))
    );
    assert_eq!(
        is_chunked(&[pair("Transfer-Encoding", "gzip")]),
        Err(Http1Error::UnsupportedTransferEncoding("gzip".to_string()))
    );
    assert_eq!(
        is_chunked(&[
            pair("Transfer-Encoding", "chunked"),
            pair("Content-Length", "3")
        ]),
        Err(Http1Error::ConflictingFraming)
    );
    // Любое TE вместе с CL — контрабанда framing (не только chunked).
    assert_eq!(
        is_chunked(&[
            pair("Transfer-Encoding", "gzip"),
            pair("Content-Length", "3")
        ]),
        Err(Http1Error::ConflictingFraming)
    );
}

// ---------------------------------------------------------------------------
// Chunked-тело
// ---------------------------------------------------------------------------

#[test]
fn decode_chunked_rfc_example() {
    let raw = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
    assert_eq!(
        decode_chunked(raw, 1024).expect("RFC 9112 §7.1"),
        ChunkedParse::Complete {
            body: b"Wikipedia".to_vec(),
            consumed: raw.len()
        }
    );
}

#[test]
fn decode_chunked_extensions_and_trailers() {
    let raw = b"5;foo=bar\r\nhello\r\n0\r\nX-T: v\r\n\r\n";
    assert_eq!(
        decode_chunked(raw, 1024).expect("extensions/trailers"),
        ChunkedParse::Complete {
            body: b"hello".to_vec(),
            consumed: raw.len()
        }
    );
}

#[test]
fn decode_chunked_incomplete_at_every_split() {
    let raw = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
    for split in 0..=raw.len() {
        match decode_chunked(&raw[..split], 1024).expect("префикс") {
            ChunkedParse::Incomplete => {
                assert!(split < raw.len(), "полный буфер не может быть Incomplete");
            }
            ChunkedParse::Complete { body, consumed } => {
                assert_eq!(
                    body, b"Wikipedia",
                    "разрез {split}: тело не должно меняться"
                );
                assert_eq!(consumed, raw.len(), "разрез {split}: consumed");
                assert_eq!(split, raw.len(), "Complete раньше конца буфера");
            }
        }
    }
}

#[test]
fn decode_chunked_errors() {
    for malformed in [
        &b"zz\r\nhello\r\n0\r\n\r\n"[..],
        &b"-1\r\n"[..],
        &b"\r\n"[..],
        &b"3\r\nabcX\r\n0\r\n\r\n"[..],
        &b"FFFFFFFFFFFFFFFFF\r\n"[..],
    ] {
        assert!(
            matches!(
                decode_chunked(malformed, 1024),
                Err(Http1Error::MalformedChunk(_))
            ),
            "{:?}: обязан быть MalformedChunk",
            String::from_utf8_lossy(malformed)
        );
    }
    // Тело больше лимита.
    assert!(matches!(
        decode_chunked(b"5\r\nhello\r\n0\r\n\r\n", 4),
        Err(Http1Error::BodyTooLarge)
    ));
    // Объявленный размер уже больше лимита — ошибка до ожидания данных.
    assert!(matches!(
        decode_chunked(b"10\r\n0123456789abcdef", 4),
        Err(Http1Error::BodyTooLarge)
    ));
    // Пустой вход — не ошибка, а нехватка данных.
    assert_eq!(
        decode_chunked(b"", 1024).expect("пусто"),
        ChunkedParse::Incomplete
    );
}

#[test]
fn decode_chunked_rejects_oversized_size_line_and_trailers() {
    // Строка размера длиннее лимита: иначе буфер растёт до бесконечности.
    let oversized_line = format!("{}\r\n", "0".repeat(MAX_CHUNK_LINE_BYTES + 1));
    assert!(matches!(
        decode_chunked(oversized_line.as_bytes(), 1024),
        Err(Http1Error::MalformedChunk(_))
    ));

    // Трейлеры суммарно длиннее лимита.
    let mut oversized_trailers = Vec::from(&b"0\r\n"[..]);
    for _ in 0..(MAX_TRAILER_BYTES / 8 + 2) {
        oversized_trailers.extend_from_slice(b"X-T: v\r\n");
    }
    oversized_trailers.extend_from_slice(b"\r\n");
    assert!(matches!(
        decode_chunked(&oversized_trailers, 1024),
        Err(Http1Error::MalformedChunk(_))
    ));

    // Незавершённый трейлер тоже не может расти бесконечно.
    let mut unterminated = Vec::from(&b"0\r\n"[..]);
    unterminated.extend(std::iter::repeat_n(b'x', MAX_TRAILER_BYTES + 1));
    assert!(matches!(
        decode_chunked(&unterminated, 1024),
        Err(Http1Error::MalformedChunk(_))
    ));
}

// ---------------------------------------------------------------------------
// Контракт BE-P2-03: DEFAULT_USER_AGENT и Content-Encoding (gzip / deflate)
// ---------------------------------------------------------------------------

#[test]
fn default_user_agent_constant_matches_oracle() {
    // Оракул: rest-d02.json auth.superProperties.browser_user_agent
    const EXPECTED_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36";
    assert_eq!(DEFAULT_USER_AGENT, EXPECTED_UA);
    assert_eq!(transport::DEFAULT_USER_AGENT, EXPECTED_UA);
}

#[test]
fn decode_response_body_identity_and_absent() {
    let payload = b"hello uncompressed world";

    // Без Content-Encoding
    let decoded = decode_response_body(&[], payload).expect("identity default");
    assert_eq!(decoded, payload);

    // С явным Content-Encoding: identity
    let decoded = decode_response_body(&[pair("Content-Encoding", "identity")], payload)
        .expect("identity explicit");
    assert_eq!(decoded, payload);
}

#[test]
fn decode_response_body_gzip_uncompressed_block() {
    // RFC 1952 gzip с RFC 1951 BTYPE=00 (uncompressed store)
    // Payload: "hello" (CRC32: 0x3610a686, ISIZE: 5)
    let gzip_hello = vec![
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // header
        0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l',
        b'o', // bfinal=1, len=5, nlen=~5, data
        0x86, 0xa6, 0x10, 0x36, 0x05, 0x00, 0x00, 0x00, // crc32, isize
    ];

    let decoded = decode_response_body(&[pair("Content-Encoding", "gzip")], &gzip_hello)
        .expect("gzip uncompressed block");
    assert_eq!(decoded, b"hello");

    let direct = decode_gzip(&gzip_hello, 1024).expect("decode_gzip");
    assert_eq!(direct, b"hello");
}

#[test]
fn decode_response_body_gzip_dynamic_huffman_oracle() {
    // Эталонный сжатый поток из gzip RFC 1952 (dynamic Huffman block):
    // Текст: "Hello, Discord REST gzip compression world!"
    let raw_gzip: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 243, 72, 205, 201, 201, 215, 81, 112, 201, 44, 78, 206,
        47, 74, 81, 8, 114, 13, 14, 81, 72, 175, 202, 44, 80, 72, 206, 207, 45, 40, 74, 45, 46,
        206, 204, 207, 83, 40, 207, 47, 202, 73, 81, 4, 0, 2, 108, 207, 249, 43, 0, 0, 0,
    ];

    let decoded = decode_response_body(&[pair("Content-Encoding", "gzip")], raw_gzip)
        .expect("gzip dynamic huffman");
    assert_eq!(
        decoded,
        b"Hello, Discord REST gzip compression world!".as_slice()
    );
}

#[test]
fn decode_response_body_gzip_with_flags_fname_fcomment_fextra_fhcrc() {
    // Заголовок gzip с флагами FEXTRA (0x04), FNAME (0x08), FCOMMENT (0x10), FHCRC (0x02):
    // Флаги: 0x04 | 0x08 | 0x10 | 0x02 = 0x1e
    let mut data = vec![
        0x1f, 0x8b, 0x08, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // header
        // FEXTRA: 2 bytes len + extra data
        0x04, 0x00, b'E', b'X', b'T', b'R', // FNAME: null-terminated
        b't', b'e', b's', b't', b'.', b't', b'x', b't', 0x00,
        // FCOMMENT: null-terminated
        b'n', b'o', b't', b'e', 0x00, // FHCRC: 2 bytes crc16
        0x12, 0x34,
    ];
    // Deflate: uncompressed "hi"
    // bfinal=1, btype=00, len=2, nlen=~2
    data.extend_from_slice(&[0x01, 0x02, 0x00, 0xfd, 0xff, b'h', b'i']);
    // Footer: CRC32 of "hi" is 0xd8932aac, isize = 2
    data.extend_from_slice(&[0xac, 0x2a, 0x93, 0xd8, 0x02, 0x00, 0x00, 0x00]);

    let decoded = decode_response_body(&[pair("Content-Encoding", "gzip")], &data)
        .expect("gzip with full header flags");
    assert_eq!(decoded, b"hi");
}

#[test]
fn decode_response_body_deflate_zlib_and_raw() {
    // zlib-wrapped deflate (RFC 1950 + RFC 1951):
    // Текст: "Hello, Discord REST deflate compression world!"
    let zlib_data: &[u8] = &[
        120, 156, 243, 72, 205, 201, 201, 215, 81, 112, 201, 44, 78, 206, 47, 74, 81, 8, 114, 13,
        14, 81, 72, 73, 77, 203, 73, 44, 73, 85, 72, 206, 207, 45, 40, 74, 45, 46, 206, 204, 207,
        83, 40, 207, 47, 202, 73, 81, 4, 0, 123, 154, 16, 151,
    ];
    let decoded = decode_response_body(&[pair("Content-Encoding", "deflate")], zlib_data)
        .expect("deflate zlib");
    assert_eq!(
        decoded,
        b"Hello, Discord REST deflate compression world!".as_slice()
    );

    // Raw deflate:
    let raw_deflate = &zlib_data[2..zlib_data.len() - 4];
    let decoded = decode_deflate(raw_deflate, 1024).expect("raw deflate");
    assert_eq!(
        decoded,
        b"Hello, Discord REST deflate compression world!".as_slice()
    );
}

#[test]
fn decode_response_body_corrupted_gzip_rejected() {
    // 1. Слишком короткий вход
    assert!(decode_gzip(b"\x1f\x8b\x08", 1024).is_err());

    // 2. Неверная сигнатура
    assert!(
        decode_gzip(
            b"\x00\x00\x08\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00",
            1024
        )
        .is_err()
    );

    // 3. Повреждённый CRC32
    let mut bad_crc = vec![
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x01, 0x05, 0x00, 0xfa, 0xff,
        b'h', b'e', b'l', b'l', b'o', 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00,
        0x00, // bad crc
    ];
    assert!(matches!(
        decode_gzip(&bad_crc, 1024),
        Err(Http1Error::MalformedBody(_))
    ));

    // 4. Повреждённый ISIZE
    bad_crc[20..24].copy_from_slice(&[0x86, 0xa6, 0x10, 0x36]); // restore valid crc
    bad_crc[24..28].copy_from_slice(&[0x99, 0x00, 0x00, 0x00]); // bad isize
    assert!(matches!(
        decode_gzip(&bad_crc, 1024),
        Err(Http1Error::MalformedBody(_))
    ));
}

#[test]
fn decode_response_body_enforces_limit() {
    let gzip_hello = vec![
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x01, 0x05, 0x00, 0xfa, 0xff,
        b'h', b'e', b'l', b'l', b'o', 0x86, 0xa6, 0x10, 0x36, 0x05, 0x00, 0x00, 0x00,
    ];
    // Лимит 4 байта при распаковке 5 байт
    assert_eq!(
        decode_response_body_with_limit(&[pair("Content-Encoding", "gzip")], &gzip_hello, 4),
        Err(Http1Error::BodyTooLarge)
    );
}

#[test]
fn decode_response_body_unsupported_encoding() {
    assert!(matches!(
        decode_response_body(&[pair("Content-Encoding", "br")], b"abc"),
        Err(Http1Error::UnsupportedContentEncoding(_))
    ));
}

#[tokio::test]
async fn transport_auto_user_agent_and_gzip_response() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("port").port();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buffer = [0u8; 4096];
        let n = stream.read(&mut buffer).await.expect("read request");
        let req_text = String::from_utf8_lossy(&buffer[..n]);

        // Проверяем, что запрос содержит User-Agent по умолчанию и Accept-Encoding: gzip
        assert!(
            req_text.contains(&format!("User-Agent: {DEFAULT_USER_AGENT}\r\n")),
            "Request headers must contain DEFAULT_USER_AGENT: {req_text}"
        );
        assert!(
            req_text
                .to_ascii_lowercase()
                .contains("accept-encoding: gzip"),
            "Request headers must contain Accept-Encoding gzip: {req_text}"
        );

        // Отправляем сжатый gzip-ответ
        let gzip_body = vec![
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x01, 0x05, 0x00, 0xfa,
            0xff, b'h', b'e', b'l', b'l', b'o', 0x86, 0xa6, 0x10, 0x36, 0x05, 0x00, 0x00, 0x00,
        ];
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            gzip_body.len()
        );
        stream.write_all(head.as_bytes()).await.expect("write head");
        stream.write_all(&gzip_body).await.expect("write body");
        stream.shutdown().await.expect("shutdown");
    });

    let base = format!("http://127.0.0.1:{port}");
    let req = HttpRequest::new(HttpMethod::Get, "/test");
    let resp = transport::send(&base, &req).await.expect("send request");

    assert_eq!(resp.status, 200);
    // Тело должно быть автоматически распаковано!
    assert_eq!(resp.body, b"hello");

    server_task.await.expect("server task completes");
}

#[tokio::test]
async fn transport_preserves_custom_user_agent() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("port").port();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buffer = [0u8; 4096];
        let n = stream.read(&mut buffer).await.expect("read request");
        let req_text = String::from_utf8_lossy(&buffer[..n]);

        assert!(
            req_text.contains("User-Agent: CustomBot/1.0\r\n"),
            "Custom User-Agent must be preserved: {req_text}"
        );
        assert!(
            !req_text.contains(DEFAULT_USER_AGENT),
            "DEFAULT_USER_AGENT must not overwrite custom User-Agent: {req_text}"
        );

        let head = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
        stream.write_all(head.as_bytes()).await.expect("write");
        stream.shutdown().await.expect("shutdown");
    });

    let base = format!("http://127.0.0.1:{port}");
    let req = HttpRequest::new(HttpMethod::Get, "/custom").header("User-Agent", "CustomBot/1.0");
    let resp = transport::send(&base, &req).await.expect("send request");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"ok");

    server_task.await.expect("server task");
}
