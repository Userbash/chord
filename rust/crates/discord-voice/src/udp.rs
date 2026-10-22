//! Headless UDP discovery and media-socket primitives.

use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

pub const DISCOVERY_LEN: usize = 74;
pub const KEEPALIVE_LEN: usize = 8;
pub const KEEPALIVE_INTERVAL_MS: u64 = 5_000;
pub const MEDIA_ROTATION_MS: u64 = 60_080;
pub const KEEPALIVE_REQUEST: [u8; 8] = [0x13, 0x37, 0xca, 0xfe, 1, 0, 0, 0];
pub const KEEPALIVE_RESPONSE: [u8; 8] = [0x13, 0x37, 0xf0, 0x0d, 1, 0, 0, 0];

/// Keepalive request with the 32-bit sequence observed in the capture.
#[must_use]
pub fn keepalive_request(sequence: u32) -> [u8; KEEPALIVE_LEN] {
    let mut bytes = KEEPALIVE_REQUEST;
    bytes[4..8].copy_from_slice(&sequence.to_le_bytes());
    bytes
}

/// Keepalive response with the echoed sequence number.
#[must_use]
pub fn keepalive_response(sequence: u32) -> [u8; KEEPALIVE_LEN] {
    let mut bytes = KEEPALIVE_RESPONSE;
    bytes[4..8].copy_from_slice(&sequence.to_le_bytes());
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryPacket {
    Request {
        ssrc: u32,
        port: u16,
    },
    Response {
        ssrc: u32,
        address: String,
        port: u16,
    },
}

#[derive(Debug)]
pub enum UdpError {
    InvalidLength,
    InvalidType,
    InvalidLengthField,
    InvalidAddress,
    InvalidPort,
    UnexpectedPeer,
    UnexpectedSsrc,
    Io(io::Error),
    Timeout,
    Closed,
}

impl PartialEq for UdpError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidLength, Self::InvalidLength)
            | (Self::InvalidType, Self::InvalidType)
            | (Self::InvalidLengthField, Self::InvalidLengthField)
            | (Self::InvalidAddress, Self::InvalidAddress)
            | (Self::InvalidPort, Self::InvalidPort)
            | (Self::UnexpectedPeer, Self::UnexpectedPeer)
            | (Self::UnexpectedSsrc, Self::UnexpectedSsrc)
            | (Self::Timeout, Self::Timeout)
            | (Self::Closed, Self::Closed) => true,
            (Self::Io(left), Self::Io(right)) => left.kind() == right.kind(),
            _ => false,
        }
    }
}

impl Eq for UdpError {}

impl fmt::Display for UdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => f.write_str("invalid UDP packet length"),
            Self::InvalidType => f.write_str("invalid UDP packet type"),
            Self::InvalidLengthField => f.write_str("invalid UDP packet length field"),
            Self::InvalidAddress => f.write_str("invalid discovery address"),
            Self::InvalidPort => f.write_str("invalid discovery port"),
            Self::UnexpectedPeer => f.write_str("UDP packet from unexpected peer"),
            Self::UnexpectedSsrc => f.write_str("UDP packet with unexpected SSRC"),
            Self::Io(error) => write!(f, "UDP I/O error: {error}"),
            Self::Timeout => f.write_str("UDP receive timeout"),
            Self::Closed => f.write_str("UDP socket is closed"),
        }
    }
}

impl std::error::Error for UdpError {}

impl From<io::Error> for UdpError {
    fn from(error: io::Error) -> Self {
        if matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ) {
            Self::Timeout
        } else {
            Self::Io(error)
        }
    }
}

impl DiscoveryPacket {
    pub fn parse(bytes: &[u8]) -> Result<Self, UdpError> {
        if bytes.len() != DISCOVERY_LEN {
            return Err(UdpError::InvalidLength);
        }
        let kind = u16::from_be_bytes([bytes[0], bytes[1]]);
        if kind != 1 && kind != 2 {
            return Err(UdpError::InvalidType);
        }
        if u16::from_be_bytes([bytes[2], bytes[3]]) != 0x46 {
            return Err(UdpError::InvalidLengthField);
        }
        let ssrc = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let port = u16::from_be_bytes([bytes[72], bytes[73]]);
        if kind == 1 {
            if bytes[8..72].iter().any(|byte| *byte != 0) {
                return Err(UdpError::InvalidAddress);
            }
            return Ok(Self::Request { ssrc, port });
        }
        let address_bytes = &bytes[8..72];
        if !address_bytes.is_ascii() {
            return Err(UdpError::InvalidAddress);
        }
        let end = address_bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(64);
        if address_bytes[end..].iter().any(|byte| *byte != 0) {
            return Err(UdpError::InvalidAddress);
        }
        let address =
            std::str::from_utf8(&address_bytes[..end]).map_err(|_| UdpError::InvalidAddress)?;
        if address.is_empty()
            || address
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b' ')
        {
            return Err(UdpError::InvalidAddress);
        }
        address
            .parse::<std::net::IpAddr>()
            .map_err(|_| UdpError::InvalidAddress)?;
        if port == 0 {
            return Err(UdpError::InvalidPort);
        }
        Ok(Self::Response {
            ssrc,
            address: address.to_owned(),
            port,
        })
    }

    pub fn encode(&self) -> Result<[u8; DISCOVERY_LEN], UdpError> {
        let mut bytes = [0; DISCOVERY_LEN];
        match self {
            Self::Request { ssrc, port } => {
                bytes[1] = 1;
                bytes[4..8].copy_from_slice(&ssrc.to_be_bytes());
                bytes[72..74].copy_from_slice(&port.to_be_bytes());
            }
            Self::Response {
                ssrc,
                address,
                port,
            } => {
                let address_bytes = address.as_bytes();
                if address_bytes.is_empty()
                    || address_bytes.len() >= 64
                    || !address_bytes.is_ascii()
                    || address_bytes
                        .iter()
                        .any(|byte| byte.is_ascii_control() || *byte == b' ')
                    || address.parse::<std::net::IpAddr>().is_err()
                {
                    return Err(UdpError::InvalidAddress);
                }
                if *port == 0 {
                    return Err(UdpError::InvalidPort);
                }
                bytes[1] = 2;
                bytes[4..8].copy_from_slice(&ssrc.to_be_bytes());
                bytes[8..8 + address_bytes.len()].copy_from_slice(address_bytes);
                bytes[72..74].copy_from_slice(&port.to_be_bytes());
            }
        }
        bytes[3] = 0x46;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationAction {
    pub discovery: [u8; DISCOVERY_LEN],
    pub empty_rr: [u8; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationSchedule {
    started_at_ms: u64,
    rotations: u64,
}

impl RotationSchedule {
    #[must_use]
    pub const fn new(started_at_ms: u64) -> Self {
        Self {
            started_at_ms,
            rotations: 0,
        }
    }

    #[must_use]
    pub fn due(&mut self, now_ms: u64) -> bool {
        let windows = self.rotations.saturating_add(1);
        let due =
            now_ms.saturating_sub(self.started_at_ms) >= MEDIA_ROTATION_MS.saturating_mul(windows);
        if due {
            self.rotations = self.rotations.saturating_add(1);
        }
        due
    }

    #[must_use]
    pub fn action(&self, ssrc: u32) -> RotationAction {
        // A request with port 0 is always encodable; the socket path replaces it
        // with the actual local port when it sends the discovery packet.
        let discovery = DiscoveryPacket::Request { ssrc, port: 0 }
            .encode()
            .unwrap_or([0; DISCOVERY_LEN]);
        RotationAction {
            discovery,
            empty_rr: empty_receiver_report(ssrc),
        }
    }
}

pub struct UdpMediaSocket {
    socket: Option<UdpSocket>,
    bind_addr: String,
    server: Option<SocketAddr>,
    ssrc: Option<u32>,
    keepalive_sequence: u32,
}

impl fmt::Debug for UdpMediaSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdpMediaSocket")
            .field("local_addr", &self.local_addr().ok())
            .finish()
    }
}

impl UdpMediaSocket {
    pub fn bind(addr: &str) -> Result<Self, UdpError> {
        let socket = UdpSocket::bind(addr)?;
        let local_ip = socket.local_addr()?.ip().to_string();
        Ok(Self {
            socket: Some(socket),
            bind_addr: format!("{local_ip}:0"),
            server: None,
            ssrc: None,
            keepalive_sequence: 0,
        })
    }

    fn socket(&self) -> Result<&UdpSocket, UdpError> {
        self.socket.as_ref().ok_or(UdpError::Closed)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, UdpError> {
        Ok(self.socket()?.local_addr()?)
    }

    pub fn send_to(&self, bytes: &[u8], target: SocketAddr) -> Result<usize, UdpError> {
        Ok(self.socket()?.send_to(bytes, target)?)
    }

    pub fn recv_from(&self, bytes: &mut [u8]) -> Result<(usize, SocketAddr), UdpError> {
        Ok(self.socket()?.recv_from(bytes)?)
    }

    pub fn send_discovery(&mut self, server: SocketAddr, ssrc: u32) -> Result<(), UdpError> {
        let port = self.local_addr()?.port();
        let packet = DiscoveryPacket::Request { ssrc, port }.encode()?;
        self.send_to(&packet, server)?;
        self.server = Some(server);
        self.ssrc = Some(ssrc);
        Ok(())
    }

    pub fn recv_discovery(&self) -> Result<DiscoveryPacket, UdpError> {
        let mut bytes = [0; 128];
        let (size, peer) = self.recv_from(&mut bytes)?;
        self.ensure_peer(peer)?;
        if size > DISCOVERY_LEN {
            return Err(UdpError::InvalidLength);
        }
        let packet = DiscoveryPacket::parse(&bytes[..size])?;
        if let Some(expected) = self.ssrc {
            let actual = match &packet {
                DiscoveryPacket::Request { ssrc, .. } | DiscoveryPacket::Response { ssrc, .. } => {
                    *ssrc
                }
            };
            if actual != expected {
                return Err(UdpError::UnexpectedSsrc);
            }
        }
        Ok(packet)
    }

    fn ensure_peer(&self, peer: SocketAddr) -> Result<(), UdpError> {
        if let Some(expected) = self.server
            && peer != expected
        {
            return Err(UdpError::UnexpectedPeer);
        }
        Ok(())
    }

    pub fn send_keepalive(&mut self, server: SocketAddr) -> Result<(), UdpError> {
        self.keepalive_sequence = self.keepalive_sequence.wrapping_add(1);
        self.send_to(&keepalive_request(self.keepalive_sequence), server)?;
        self.server = Some(server);
        Ok(())
    }

    pub fn recv_keepalive(&self) -> Result<[u8; KEEPALIVE_LEN], UdpError> {
        let mut bytes = [0; 128];
        let (size, peer) = self.recv_from(&mut bytes)?;
        self.ensure_peer(peer)?;
        if size != KEEPALIVE_LEN {
            return Err(UdpError::InvalidLength);
        }
        let packet: [u8; KEEPALIVE_LEN] = bytes[..KEEPALIVE_LEN]
            .try_into()
            .map_err(|_| UdpError::InvalidLength)?;
        if packet[..4] == KEEPALIVE_RESPONSE[..4] || packet[..4] == KEEPALIVE_REQUEST[..4] {
            Ok(packet)
        } else {
            Err(UdpError::InvalidType)
        }
    }

    pub fn send_empty_rr(&self, server: SocketAddr, ssrc: u32) -> Result<(), UdpError> {
        self.send_to(&empty_receiver_report(ssrc), server)?;
        Ok(())
    }

    pub fn send_srtcp_empty_rr(
        &self,
        server: SocketAddr,
        ssrc: u32,
        tag: [u8; 16],
        counter: u32,
    ) -> Result<(), UdpError> {
        self.send_to(&empty_srtcp_receiver_report(ssrc, tag, counter), server)?;
        Ok(())
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), UdpError> {
        self.socket()?.set_read_timeout(timeout)?;
        Ok(())
    }

    fn rebind(&mut self) -> Result<(SocketAddr, u32), UdpError> {
        let server = self.server.ok_or(UdpError::InvalidAddress)?;
        let ssrc = self.ssrc.ok_or(UdpError::InvalidAddress)?;
        self.socket.take();
        self.socket = Some(UdpSocket::bind(&self.bind_addr)?);
        Ok((server, ssrc))
    }

    fn rotate_impl(&mut self, srtcp: Option<([u8; 16], u32)>) -> Result<RotationAction, UdpError> {
        let (server, ssrc) = self.rebind()?;
        let port = self.local_addr()?.port();
        let discovery = DiscoveryPacket::Request { ssrc, port }.encode()?;
        let empty_rr = empty_receiver_report(ssrc);
        self.send_to(&discovery, server)?;
        if let Some((tag, counter)) = srtcp {
            self.send_srtcp_empty_rr(server, ssrc, tag, counter)?;
        } else {
            self.send_empty_rr(server, ssrc)?;
        }
        self.server = Some(server);
        self.ssrc = Some(ssrc);
        Ok(RotationAction {
            discovery,
            empty_rr,
        })
    }

    /// Rotate and send an unencrypted empty RR (8 B). SRTCP wrapping is P4-03.
    pub fn rotate(&mut self) -> Result<RotationAction, UdpError> {
        self.rotate_impl(None)
    }

    /// Rotate and send the captured 28-byte SRTCP-wrapped empty RR.
    pub fn rotate_srtcp(
        &mut self,
        tag: [u8; 16],
        counter: u32,
    ) -> Result<RotationAction, UdpError> {
        self.rotate_impl(Some((tag, counter)))
    }

    /// Run the schedule and rotate the socket when its 60.08 s window is due.
    pub fn rotate_if_due(
        &mut self,
        now_ms: u64,
        schedule: &mut RotationSchedule,
    ) -> Result<Option<RotationAction>, UdpError> {
        if !schedule.due(now_ms) {
            return Ok(None);
        }
        self.rotate().map(Some)
    }
}

#[must_use]
pub fn empty_receiver_report(ssrc: u32) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&[0x80, 0xc9, 0, 1]);
    bytes[4..8].copy_from_slice(&ssrc.to_be_bytes());
    bytes
}

#[must_use]
pub fn empty_srtcp_receiver_report(ssrc: u32, tag: [u8; 16], counter: u32) -> [u8; 28] {
    let mut bytes = [0; 28];
    bytes[..8].copy_from_slice(&empty_receiver_report(ssrc));
    bytes[8..24].copy_from_slice(&tag);
    bytes[24..28].copy_from_slice(&counter.to_le_bytes());
    bytes
}
