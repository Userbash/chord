use discord_voice::udp::{
    DISCOVERY_LEN, DiscoveryPacket, KEEPALIVE_INTERVAL_MS, KEEPALIVE_LEN, KEEPALIVE_REQUEST,
    KEEPALIVE_RESPONSE, MEDIA_ROTATION_MS, RotationSchedule, UdpError, UdpMediaSocket,
    empty_receiver_report, empty_srtcp_receiver_report, keepalive_request, keepalive_response,
};
use std::net::UdpSocket;
use std::time::Duration;

const REQUEST_HEX: &str = "00010046000023f8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const REQUEST_WITH_PORT_HEX: &str = "00010046000023f8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008409";
const RESPONSE_HEX: &str = "00020046000023f839352e3136342e39312e39380000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000089b3";

fn decode_hex(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    assert_eq!(bytes.len() % 2, 0, "hex fixture must have an even length");
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("bad test hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn loopback_socket() -> Option<UdpSocket> {
    match UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => Some(socket),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skip: loopback UDP unavailable: {error}");
            None
        }
        Err(error) => panic!("loopback bind failed: {error}"),
    }
}

fn loopback_client() -> Option<UdpMediaSocket> {
    match UdpMediaSocket::bind("127.0.0.1:0") {
        Ok(socket) => Some(socket),
        Err(UdpError::Io(error)) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skip: loopback UDP unavailable: {error}");
            None
        }
        Err(error) => panic!("loopback client bind failed: {error}"),
    }
}

#[test]
fn discovery_matches_strace_request_and_response_oracles() {
    let request = decode_hex(REQUEST_HEX);
    let request_with_port = decode_hex(REQUEST_WITH_PORT_HEX);
    let response = decode_hex(RESPONSE_HEX);
    assert_eq!(request.len(), 74);
    assert_eq!(&request[..8], &[0, 1, 0, 0x46, 0, 0, 0x23, 0xf8]);
    assert_eq!(&response[..8], &[0, 2, 0, 0x46, 0, 0, 0x23, 0xf8]);
    assert_eq!(
        DiscoveryPacket::Request {
            ssrc: 9208,
            port: 0
        }
        .encode()
        .unwrap(),
        request.as_slice()
    );
    assert_eq!(
        DiscoveryPacket::parse(&request),
        Ok(DiscoveryPacket::Request {
            ssrc: 9208,
            port: 0
        })
    );
    assert_eq!(request_with_port.len(), 74);
    assert_eq!(
        DiscoveryPacket::parse(&request_with_port),
        Ok(DiscoveryPacket::Request {
            ssrc: 9208,
            port: 33801
        })
    );
    assert_eq!(
        DiscoveryPacket::Request {
            ssrc: 9208,
            port: 33801
        }
        .encode()
        .unwrap(),
        request_with_port.as_slice()
    );
    assert_eq!(
        DiscoveryPacket::parse(&response),
        Ok(DiscoveryPacket::Response {
            ssrc: 9208,
            address: "95.164.91.98".into(),
            port: 35251,
        })
    );
}

#[test]
fn discovery_receivers_reject_oversized_datagrams() {
    let Some(server) = loopback_socket() else {
        return;
    };
    let Some(client) = loopback_client() else {
        return;
    };
    let client_addr = client.local_addr().unwrap();
    server
        .send_to(&[0; DISCOVERY_LEN + 1], client_addr)
        .unwrap();
    assert_eq!(client.recv_discovery(), Err(UdpError::InvalidLength));
    server
        .send_to(&[0; KEEPALIVE_LEN + 1], client_addr)
        .unwrap();
    assert_eq!(client.recv_keepalive(), Err(UdpError::InvalidLength));
}

#[test]
fn response_round_trips_and_rejects_malformed_fields() {
    let packet = DiscoveryPacket::Response {
        ssrc: 0x12345678,
        address: "127.0.0.1".into(),
        port: 19329,
    };
    assert_eq!(
        DiscoveryPacket::parse(&packet.encode().unwrap()),
        Ok(packet)
    );
    let ipv6 = DiscoveryPacket::Response {
        ssrc: 0x12345678,
        address: "::1".into(),
        port: 19329,
    };
    assert_eq!(DiscoveryPacket::parse(&ipv6.encode().unwrap()), Ok(ipv6));
    let valid = decode_hex(RESPONSE_HEX);
    for size in [73, 75] {
        let mut bytes = valid.clone();
        bytes.resize(size, 0);
        assert_eq!(DiscoveryPacket::parse(&bytes), Err(UdpError::InvalidLength));
    }
    let mut bytes = valid.clone();
    bytes[1] = 3;
    assert_eq!(DiscoveryPacket::parse(&bytes), Err(UdpError::InvalidType));
    let mut bytes = valid.clone();
    bytes[3] = 0x45;
    assert_eq!(
        DiscoveryPacket::parse(&bytes),
        Err(UdpError::InvalidLengthField)
    );
    let mut bytes = valid.clone();
    bytes[8] = 0x7f;
    assert_eq!(
        DiscoveryPacket::parse(&bytes),
        Err(UdpError::InvalidAddress)
    );
    let mut bytes = valid.clone();
    bytes[8..72].fill(0);
    bytes[8..19].copy_from_slice(b"not-an-ipv4");
    assert_eq!(
        DiscoveryPacket::parse(&bytes),
        Err(UdpError::InvalidAddress)
    );
    let mut bytes = valid.clone();
    bytes[8] = b'a';
    bytes[9] = 0;
    bytes[10] = b'x';
    assert_eq!(
        DiscoveryPacket::parse(&bytes),
        Err(UdpError::InvalidAddress)
    );
    let mut bytes = valid;
    bytes[72] = 0;
    bytes[73] = 0;
    assert_eq!(DiscoveryPacket::parse(&bytes), Err(UdpError::InvalidPort));
    assert_eq!(
        DiscoveryPacket::parse(&[0; 73]),
        Err(UdpError::InvalidLength)
    );
}

#[test]
fn encode_rejects_invalid_response() {
    let invalid_address = "x".repeat(65);
    for address in [String::new(), invalid_address, "not-an-ip".into()] {
        assert_eq!(
            DiscoveryPacket::Response {
                ssrc: 9208,
                address,
                port: 19329,
            }
            .encode(),
            Err(UdpError::InvalidAddress)
        );
    }
    assert_eq!(
        DiscoveryPacket::Response {
            ssrc: 9208,
            address: "127.0.0.1".into(),
            port: 0,
        }
        .encode(),
        Err(UdpError::InvalidPort)
    );
}

#[test]
fn receivers_reject_foreign_peer_and_wrong_ssrc() {
    let Some(server) = loopback_socket() else {
        return;
    };
    let Some(foreign) = loopback_socket() else {
        return;
    };
    let Some(mut client) = loopback_client() else {
        return;
    };
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let client_addr = client.local_addr().unwrap();
    let server_addr = server.local_addr().unwrap();
    client.send_discovery(server_addr, 9208).unwrap();

    let response = DiscoveryPacket::Response {
        ssrc: 9208,
        address: "127.0.0.1".into(),
        port: 19329,
    };
    let valid = response.encode().unwrap();
    foreign.send_to(&valid, client_addr).unwrap();
    assert_eq!(client.recv_discovery(), Err(UdpError::UnexpectedPeer));

    let wrong_ssrc = DiscoveryPacket::Response {
        ssrc: 9209,
        address: "127.0.0.1".into(),
        port: 19329,
    }
    .encode()
    .unwrap();
    server.send_to(&wrong_ssrc, client_addr).unwrap();
    assert_eq!(client.recv_discovery(), Err(UdpError::UnexpectedSsrc));

    server.send_to(&valid, client_addr).unwrap();
    assert_eq!(client.recv_discovery(), Ok(response));

    foreign.send_to(&KEEPALIVE_RESPONSE, client_addr).unwrap();
    assert_eq!(client.recv_keepalive(), Err(UdpError::UnexpectedPeer));
    server.send_to(&KEEPALIVE_RESPONSE, client_addr).unwrap();
    assert_eq!(client.recv_keepalive(), Ok(KEEPALIVE_RESPONSE));
}

#[test]
fn rotate_if_due_integrates_schedule_and_socket() {
    let Some(server) = loopback_socket() else {
        return;
    };
    let server_addr = server.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let mut packets = Vec::new();
        let mut buf = [0; 128];
        for _ in 0..3 {
            let (size, peer) = server.recv_from(&mut buf).unwrap();
            packets.push(buf[..size].to_vec());
            server.send_to(&buf[..size], peer).unwrap();
        }
        packets
    });
    let Some(mut client) = loopback_client() else {
        return;
    };
    client.send_discovery(server_addr, 9208).unwrap();
    let mut schedule = RotationSchedule::new(0);
    assert!(
        client
            .rotate_if_due(60_079, &mut schedule)
            .unwrap()
            .is_none()
    );
    let action = client
        .rotate_if_due(60_080, &mut schedule)
        .unwrap()
        .expect("rotation is due");
    assert_eq!(
        DiscoveryPacket::parse(&action.discovery),
        Ok(DiscoveryPacket::Request {
            ssrc: 9208,
            port: client.local_addr().unwrap().port(),
        })
    );
    assert_eq!(action.empty_rr, empty_receiver_report(9208));
    assert!(
        client
            .rotate_if_due(60_080, &mut schedule)
            .unwrap()
            .is_none()
    );
    let packets = thread.join().unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(packets[0].len(), DISCOVERY_LEN);
    assert_eq!(packets[1].len(), DISCOVERY_LEN);
    assert_eq!(packets[2].len(), 8);
}

#[test]
fn rotate_srtcp_sends_captured_28_byte_rr() {
    let Some(server) = loopback_socket() else {
        return;
    };
    let server_addr = server.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let mut packets = Vec::new();
        let mut buf = [0; 128];
        for _ in 0..3 {
            let (size, _) = server.recv_from(&mut buf).unwrap();
            packets.push(buf[..size].to_vec());
        }
        packets
    });
    let Some(mut client) = loopback_client() else {
        return;
    };
    client.send_discovery(server_addr, 9208).unwrap();
    let tag = [
        0x4c, 0x43, 0x9e, 0x13, 0x57, 0xc7, 0xd7, 0xe3, 0x85, 0xee, 0x66, 0xd1, 0x20, 0x58, 0xbf,
        0xe0,
    ];
    client.rotate_srtcp(tag, 0x8000_0016).unwrap();
    let packets = thread.join().unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(packets[1].len(), DISCOVERY_LEN);
    assert_eq!(
        packets[2],
        empty_srtcp_receiver_report(9208, tag, 0x8000_0016)
    );
}

#[test]
fn keepalive_constants_match_re_and_interval() {
    assert_eq!(KEEPALIVE_LEN, 8);
    assert_eq!(KEEPALIVE_REQUEST, [0x13, 0x37, 0xca, 0xfe, 1, 0, 0, 0]);
    assert_eq!(KEEPALIVE_RESPONSE, [0x13, 0x37, 0xf0, 0x0d, 1, 0, 0, 0]);
    assert_eq!(KEEPALIVE_INTERVAL_MS, 5_000);
}

#[test]
fn keepalive_sequence_matches_capture_and_increments() {
    assert_eq!(keepalive_request(1), KEEPALIVE_REQUEST);
    assert_eq!(keepalive_response(1), KEEPALIVE_RESPONSE);
    assert_eq!(keepalive_request(2), [0x13, 0x37, 0xca, 0xfe, 2, 0, 0, 0]);
    assert_eq!(
        keepalive_response(0x0a),
        [0x13, 0x37, 0xf0, 0x0d, 0x0a, 0, 0, 0]
    );
}

#[test]
fn rotation_boundary_and_action_match_capture_contract() {
    assert_eq!(DISCOVERY_LEN, 74);
    assert_eq!(MEDIA_ROTATION_MS, 60_080);
    let mut schedule = RotationSchedule::new(100);
    assert!(!schedule.due(60_179));
    assert!(schedule.due(60_180));
    assert!(!schedule.due(60_180));
    assert!(schedule.due(120_260));
    let action = schedule.action(9208);
    assert_eq!(
        DiscoveryPacket::parse(&action.discovery),
        Ok(DiscoveryPacket::Request {
            ssrc: 9208,
            port: 0
        })
    );
    assert_eq!(action.empty_rr, empty_receiver_report(9208));
}

#[test]
fn empty_receiver_report_has_exact_rtcp_header_and_ssrc() {
    let packet = empty_receiver_report(0x000023f8);
    assert_eq!(packet.len(), 8);
    assert_eq!(packet, [0x80, 0xc9, 0, 1, 0, 0, 0x23, 0xf8]);
}

#[test]
fn empty_srtcp_receiver_report_matches_capture() {
    let tag = [
        0x4c, 0x43, 0x9e, 0x13, 0x57, 0xc7, 0xd7, 0xe3, 0x85, 0xee, 0x66, 0xd1, 0x20, 0x58, 0xbf,
        0xe0,
    ];
    let packet = empty_srtcp_receiver_report(0x000023f8, tag, 0x8000_0016);
    assert_eq!(packet.len(), 28);
    assert_eq!(
        packet,
        [
            0x80, 0xc9, 0x00, 0x01, 0x00, 0x00, 0x23, 0xf8, 0x4c, 0x43, 0x9e, 0x13, 0x57, 0xc7,
            0xd7, 0xe3, 0x85, 0xee, 0x66, 0xd1, 0x20, 0x58, 0xbf, 0xe0, 0x16, 0x00, 0x00, 0x80,
        ]
    );
}

#[test]
fn loopback_send_receive_discovery_keepalive_and_timeout() {
    let Some(server) = loopback_socket() else {
        return;
    };
    server
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let server_addr = server.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let mut buf = [0; 128];
        for _ in 0..3 {
            let (size, peer) = server.recv_from(&mut buf).unwrap();
            server.send_to(&buf[..size], peer).unwrap();
        }
    });
    let Some(mut client) = loopback_client() else {
        return;
    };
    client.send_discovery(server_addr, 9208).unwrap();
    assert_eq!(
        client.recv_discovery().unwrap(),
        DiscoveryPacket::Request {
            ssrc: 9208,
            port: client.local_addr().unwrap().port()
        }
    );
    client.send_keepalive(server_addr).unwrap();
    assert_eq!(client.recv_keepalive().unwrap(), KEEPALIVE_REQUEST);
    client.send_keepalive(server_addr).unwrap();
    assert_eq!(client.recv_keepalive().unwrap(), keepalive_request(2));
    thread.join().unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(10)))
        .unwrap();
    assert_eq!(client.recv_keepalive(), Err(UdpError::Timeout));
}

#[test]
fn rotate_changes_loopback_port_and_closes_old_socket() {
    let Some(server) = loopback_socket() else {
        return;
    };
    let server_addr = server.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let mut buf = [0; 128];
        for _ in 0..3 {
            server.recv_from(&mut buf).unwrap();
        }
    });
    let Some(mut client) = loopback_client() else {
        return;
    };
    let old = client.local_addr().unwrap();
    client.send_discovery(server_addr, 9208).unwrap();
    client.rotate().unwrap();
    let new = client.local_addr().unwrap();
    assert_ne!(old.port(), new.port());
    thread.join().unwrap();
}

#[test]
fn parser_never_panics_on_prefixes_or_arbitrary_packets() {
    let packet = decode_hex(RESPONSE_HEX);
    for end in 0..=packet.len() {
        let _ = DiscoveryPacket::parse(&packet[..end]);
    }
    for len in 0..=128 {
        let bytes: Vec<u8> = (0..len).map(|index| (index * 37 + len) as u8).collect();
        let _ = DiscoveryPacket::parse(&bytes);
    }
}
