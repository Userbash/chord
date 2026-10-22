//! HTTP/1.1-фрейминг без I/O (волна RW-10 / W10a).
//!
//! **Провенанс оракулов** (независимые источники):
//! - RFC 9112 §6 (framing), §7.1 (пример chunked `Wikipedia`), RFC 9110 §5 (поля);
//! - `docs/re/rest-d02.md` §3: REST Discord — HTTP/1.1, тела небольшие, `Content-Length`.
//!
//! Тесты сверяют точные байты запроса и разбор ответа, а не подгоняются под реализацию.

use discord_core::rest::http1::{
    ChunkedParse, HeadParse, Http1Error, MAX_CHUNK_LINE_BYTES, MAX_HEAD_BYTES, MAX_TRAILER_BYTES,
    content_length, decode_chunked, encode_request, header, is_chunked, parse_response_head,
};
use discord_core::rest::routes::HttpMethod;

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
                Err(discord_core::rest::http1::Http1Error::InvalidHeader(_))
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
                Err(discord_core::rest::http1::Http1Error::InvalidHeader(_))
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
        Err(discord_core::rest::http1::Http1Error::InvalidContentLength(
            _
        ))
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
        Err(discord_core::rest::http1::Http1Error::HeadTooLarge)
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
