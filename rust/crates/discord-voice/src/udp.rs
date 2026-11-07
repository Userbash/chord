//! Headless UDP discovery and media-socket primitives.

use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

pub const DISCOVERY_LEN: usize = 74;
pub const KEEPALIVE_LEN: usize = 8;
pub const KEEPALIVE_INTERVAL_MS: u64 = 5_000;
pub const MEDIA_ROTATION_MS: u64 = 60_080;
pub const SRTCP_EMPTY_RR_LEN: usize = 28;
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

    /// Кодирует 74-байтный запрос UDP IP Discovery с портом 0 и пустым адресом.
    pub fn encode_request(ssrc: u32) -> Result<[u8; DISCOVERY_LEN], UdpError> {
        Self::encode_request_with_port(ssrc, "", 0)
    }

    /// Кодирует 74-байтный запрос UDP IP Discovery с указанием порта в последних двух байтах [72..74].
    pub fn encode_request_with_port(
        ssrc: u32,
        address: &str,
        port: u16,
    ) -> Result<[u8; DISCOVERY_LEN], UdpError> {
        let mut bytes = [0u8; DISCOVERY_LEN];
        bytes[1] = 1;
        bytes[3] = 0x46;
        bytes[4..8].copy_from_slice(&ssrc.to_be_bytes());
        if !address.is_empty() {
            let address_bytes = address.as_bytes();
            if address_bytes.len() >= 64
                || !address_bytes.is_ascii()
                || address_bytes
                    .iter()
                    .any(|byte| byte.is_ascii_control() || *byte == b' ')
                || address.parse::<std::net::IpAddr>().is_err()
            {
                return Err(UdpError::InvalidAddress);
            }
            bytes[8..8 + address_bytes.len()].copy_from_slice(address_bytes);
        }
        bytes[72..74].copy_from_slice(&port.to_be_bytes());
        Ok(bytes)
    }
}

/// Кодирует 74-байтный запрос UDP IP Discovery (`type = 1`, `length = 70`, `ssrc` BE, 66 нулевых байт).
pub fn encode_request(ssrc: u32) -> Result<[u8; DISCOVERY_LEN], UdpError> {
    DiscoveryPacket::encode_request(ssrc)
}

/// Кодирует 74-байтный запрос UDP IP Discovery с указанием порта в последних двух байтах [72..74].
pub fn encode_request_with_port(
    ssrc: u32,
    address: &str,
    port: u16,
) -> Result<[u8; DISCOVERY_LEN], UdpError> {
    DiscoveryPacket::encode_request_with_port(ssrc, address, port)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationAction {
    pub discovery: [u8; DISCOVERY_LEN],
    pub empty_rr: [u8; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtcpRotationAction {
    pub discovery: [u8; DISCOVERY_LEN],
    pub empty_rr: [u8; SRTCP_EMPTY_RR_LEN],
}

impl SrtcpRotationAction {
    #[must_use]
    pub fn srtcp_empty_rr(&self) -> &[u8; SRTCP_EMPTY_RR_LEN] {
        &self.empty_rr
    }
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

    #[must_use]
    pub fn action_srtcp(&self, ssrc: u32, tag: [u8; 16], counter: u32) -> SrtcpRotationAction {
        let discovery = DiscoveryPacket::Request { ssrc, port: 0 }
            .encode()
            .unwrap_or([0; DISCOVERY_LEN]);
        let counter_with_e = 0x8000_0000u32 | counter;
        SrtcpRotationAction {
            discovery,
            empty_rr: empty_srtcp_receiver_report(ssrc, tag, counter_with_e),
        }
    }
}

pub struct UdpMediaSocket {
    socket: Option<UdpSocket>,
    bind_addr: String,
    server: Option<SocketAddr>,
    ssrc: Option<u32>,
    keepalive_sequence: u32,
    packet_counter: u32,
}

impl fmt::Debug for UdpMediaSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdpMediaSocket")
            .field("local_addr", &self.local_addr().ok())
            .field("packet_counter", &self.packet_counter)
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
            packet_counter: 0,
        })
    }

    #[must_use]
    pub fn packet_counter(&self) -> u32 {
        self.packet_counter
    }

    pub fn set_packet_counter(&mut self, counter: u32) {
        self.packet_counter = counter;
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

    /// Отправляет 74-байтный UDP IP Discovery запрос с указанием локального порта сокета в байтах [72..74].
    pub fn send_discovery_with_local_port(
        &mut self,
        server: SocketAddr,
        ssrc: u32,
    ) -> Result<(), UdpError> {
        let port = self.local_addr()?.port();
        let packet = DiscoveryPacket::encode_request_with_port(ssrc, "", port)?;
        self.send_to(&packet, server)?;
        self.server = Some(server);
        self.ssrc = Some(ssrc);
        Ok(())
    }

    pub fn send_discovery(&mut self, server: SocketAddr, ssrc: u32) -> Result<(), UdpError> {
        self.send_discovery_with_local_port(server, ssrc)
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
            let counter_with_e = 0x8000_0000u32 | counter;
            self.send_srtcp_empty_rr(server, ssrc, tag, counter_with_e)?;
            self.packet_counter = (counter & 0x7fff_ffff).wrapping_add(1);
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

    /// Rotate and send the captured 28-byte SRTCP-wrapped empty RR with E-bit set.
    pub fn rotate_srtcp(
        &mut self,
        tag: [u8; 16],
        counter: u32,
    ) -> Result<SrtcpRotationAction, UdpError> {
        let (server, ssrc) = self.rebind()?;
        let port = self.local_addr()?.port();
        let discovery = DiscoveryPacket::Request { ssrc, port }.encode()?;
        let counter_with_e = 0x8000_0000u32 | counter;
        let empty_rr = empty_srtcp_receiver_report(ssrc, tag, counter_with_e);
        self.send_to(&discovery, server)?;
        self.send_to(&empty_rr, server)?;
        self.server = Some(server);
        self.ssrc = Some(ssrc);
        self.packet_counter = (counter & 0x7fff_ffff).wrapping_add(1);
        Ok(SrtcpRotationAction {
            discovery,
            empty_rr,
        })
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

    /// Run the schedule and rotate the socket using 28B SRTCP empty RR when 60.08 s window is due.
    ///
    /// Increments `packet_counter` on rotation to preserve the packet sequence.
    pub fn rotate_if_due_srtcp(
        &mut self,
        now_ms: u64,
        schedule: &mut RotationSchedule,
        tag: [u8; 16],
        packet_counter: &mut u32,
    ) -> Result<Option<SrtcpRotationAction>, UdpError> {
        if !schedule.due(now_ms) {
            return Ok(None);
        }
        let current_counter = *packet_counter;
        let action = self.rotate_srtcp(tag, current_counter)?;
        *packet_counter = packet_counter.wrapping_add(1);
        self.packet_counter = *packet_counter;
        Ok(Some(action))
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
pub fn empty_srtcp_receiver_report(
    ssrc: u32,
    tag: [u8; 16],
    counter: u32,
) -> [u8; SRTCP_EMPTY_RR_LEN] {
    let mut bytes = [0; SRTCP_EMPTY_RR_LEN];
    bytes[..8].copy_from_slice(&empty_receiver_report(ssrc));
    bytes[8..24].copy_from_slice(&tag);
    bytes[24..28].copy_from_slice(&counter.to_le_bytes());
    bytes
}

/// Тройка SSRC потока Go Live (`audio_ssrc`, `video_ssrc = audio_ssrc + 1`, `rtx_ssrc = audio_ssrc + 2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSsrcs {
    /// Аудио SSRC стрима из `OP 2 READY`.
    pub audio_ssrc: u32,
    /// Видео SSRC стрима (`audio_ssrc + 1`).
    pub video_ssrc: u32,
    /// RTX SSRC стрима (`audio_ssrc + 2`).
    pub rtx_ssrc: u32,
}

impl StreamSsrcs {
    /// Вычисляет SSRC стрима из базового `audio_ssrc`: `video_ssrc = audio_ssrc + 1`, `rtx_ssrc = audio_ssrc + 2`.
    #[must_use]
    pub const fn from_audio_ssrc(audio_ssrc: u32) -> Self {
        Self {
            audio_ssrc,
            video_ssrc: audio_ssrc.wrapping_add(1),
            rtx_ssrc: audio_ssrc.wrapping_add(2),
        }
    }

    /// Проверяет, принадлежит ли `ssrc` этой тройке (`audio`, `video` или `rtx`).
    #[must_use]
    pub const fn contains(&self, ssrc: u32) -> bool {
        ssrc == self.audio_ssrc || ssrc == self.video_ssrc || ssrc == self.rtx_ssrc
    }
}

/// Алиас двойной UDP-сессии (основной голос + выделенный сокет Go Live).
pub type DualUdpMediaSession = GoLiveUdpSession;

/// Выделенная парная UDP-сессия для одновременной работы основного голосового канала
/// и отдельной сессии демонстрации экрана Go Live (`STREAM_SERVER_UPDATE`).
#[derive(Debug)]
pub struct GoLiveUdpSession {
    voice: UdpMediaSocket,
    stream: Option<UdpMediaSocket>,
    stream_key: Option<String>,
    stream_ssrcs: Option<StreamSsrcs>,
}

impl GoLiveUdpSession {
    /// Создаёт обёртку вокруг основного голосового UDP-сокета без активного стрима.
    #[must_use]
    pub fn new(voice: UdpMediaSocket) -> Self {
        Self {
            voice,
            stream: None,
            stream_key: None,
            stream_ssrcs: None,
        }
    }

    /// Привязывает одновременно основной голосовой UDP-сокет и выделенный UDP-сокет стрима Go Live.
    pub fn bind_dual(
        voice_bind_addr: &str,
        stream_bind_addr: &str,
        stream_key: impl Into<String>,
    ) -> Result<Self, UdpError> {
        let voice = UdpMediaSocket::bind(voice_bind_addr)?;
        let stream = UdpMediaSocket::bind(stream_bind_addr)?;
        Ok(Self {
            voice,
            stream: Some(stream),
            stream_key: Some(stream_key.into()),
            stream_ssrcs: None,
        })
    }

    /// Открывает (или переоткрывает) выделенный UDP-сокет второй сессии Go Live.
    pub fn open_stream(
        &mut self,
        stream_bind_addr: &str,
        stream_key: impl Into<String>,
    ) -> Result<&mut UdpMediaSocket, UdpError> {
        let socket = UdpMediaSocket::bind(stream_bind_addr)?;
        self.stream = Some(socket);
        self.stream_key = Some(stream_key.into());
        self.stream_ssrcs = None;
        self.stream.as_mut().ok_or(UdpError::Closed)
    }

    /// Закрывает выделенный UDP-сокет стрима Go Live, не затрагивая основной голосовой сокет.
    pub fn close_stream(&mut self) -> bool {
        let had_stream = self.stream.take().is_some();
        self.stream_key = None;
        self.stream_ssrcs = None;
        had_stream
    }

    /// Ссылка на основной голосовой UDP-сокет.
    #[must_use]
    pub const fn voice_socket(&self) -> &UdpMediaSocket {
        &self.voice
    }

    /// Мутабельная ссылка на основной голосовой UDP-сокет.
    pub fn voice_socket_mut(&mut self) -> &mut UdpMediaSocket {
        &mut self.voice
    }

    /// Ссылка на выделенный UDP-сокет Go Live, если стрим активен.
    #[must_use]
    pub const fn stream_socket(&self) -> Option<&UdpMediaSocket> {
        self.stream.as_ref()
    }

    /// Мутабельная ссылка на выделенный UDP-сокет Go Live, если стрим активен.
    pub fn stream_socket_mut(&mut self) -> Option<&mut UdpMediaSocket> {
        self.stream.as_mut()
    }

    /// Ключ активного стрима (`stream_key` из `STREAM_SERVER_UPDATE`).
    #[must_use]
    pub fn stream_key(&self) -> Option<&str> {
        self.stream_key.as_deref()
    }

    /// Вычисленная тройка SSRC активного стрима (`audio_ssrc`, `video_ssrc`, `rtx_ssrc`).
    #[must_use]
    pub const fn stream_ssrcs(&self) -> Option<StreamSsrcs> {
        self.stream_ssrcs
    }

    /// Отправляет 74-байтный UDP IP Discovery запрос по выделенному сокету Go Live
    /// и фиксирует `StreamSsrcs` (`video_ssrc = audio_ssrc + 1`, `rtx_ssrc = audio_ssrc + 2`).
    pub fn send_stream_discovery(
        &mut self,
        server: SocketAddr,
        audio_ssrc: u32,
    ) -> Result<StreamSsrcs, UdpError> {
        let stream = self.stream.as_mut().ok_or(UdpError::Closed)?;
        stream.send_discovery(server, audio_ssrc)?;
        let ssrcs = StreamSsrcs::from_audio_ssrc(audio_ssrc);
        self.stream_ssrcs = Some(ssrcs);
        Ok(ssrcs)
    }

    /// Принимает и валидирует 74-байтный UDP IP Discovery ответ из выделенного сокета Go Live.
    pub fn recv_stream_discovery(&self) -> Result<DiscoveryPacket, UdpError> {
        let stream = self.stream.as_ref().ok_or(UdpError::Closed)?;
        stream.recv_discovery()
    }

    /// Отправляет UDP keepalive по выделенному сокету Go Live.
    pub fn send_stream_keepalive(&mut self, server: SocketAddr) -> Result<(), UdpError> {
        let stream = self.stream.as_mut().ok_or(UdpError::Closed)?;
        stream.send_keepalive(server)
    }

    /// Принимает UDP keepalive из выделенного сокета Go Live.
    pub fn recv_stream_keepalive(&self) -> Result<[u8; KEEPALIVE_LEN], UdpError> {
        let stream = self.stream.as_ref().ok_or(UdpError::Closed)?;
        stream.recv_keepalive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_ssrcs_computes_video_and_rtx_from_audio_ssrc() {
        let ssrcs = StreamSsrcs::from_audio_ssrc(50_000);
        assert_eq!(ssrcs.audio_ssrc, 50_000);
        assert_eq!(ssrcs.video_ssrc, 50_001);
        assert_eq!(ssrcs.rtx_ssrc, 50_002);
        assert!(ssrcs.contains(50_000));
        assert!(ssrcs.contains(50_001));
        assert!(ssrcs.contains(50_002));
        assert!(!ssrcs.contains(50_003));
    }

    #[test]
    fn go_live_dual_udp_session_isolates_voice_and_stream_discovery_and_keepalive() {
        let voice_server = match UdpSocket::bind("127.0.0.1:0") {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("bind voice_server: {e}"),
        };
        let stream_server = UdpSocket::bind("127.0.0.1:0").expect("bind stream_server");
        voice_server
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");
        stream_server
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");

        let voice_srv_addr = voice_server.local_addr().expect("voice addr");
        let stream_srv_addr = stream_server.local_addr().expect("stream addr");

        let mut dual = GoLiveUdpSession::bind_dual(
            "127.0.0.1:0",
            "127.0.0.1:0",
            "guild:1014537851418980372:1176785373989445703:384299970632876032",
        )
        .expect("bind_dual");

        assert_eq!(
            dual.stream_key(),
            Some("guild:1014537851418980372:1176785373989445703:384299970632876032")
        );

        let voice_client_port = dual
            .voice_socket()
            .local_addr()
            .expect("voice local")
            .port();
        let stream_client_port = dual
            .stream_socket()
            .expect("stream socket")
            .local_addr()
            .expect("stream local")
            .port();
        assert_ne!(voice_client_port, stream_client_port);

        dual.voice_socket_mut()
            .send_discovery(voice_srv_addr, 9208)
            .expect("voice discovery");
        let stream_ssrcs = dual
            .send_stream_discovery(stream_srv_addr, 40_000)
            .expect("stream discovery");
        assert_eq!(
            stream_ssrcs,
            StreamSsrcs {
                audio_ssrc: 40_000,
                video_ssrc: 40_001,
                rtx_ssrc: 40_002,
            }
        );
        assert_eq!(dual.stream_ssrcs(), Some(stream_ssrcs));

        let mut buf = [0u8; 128];
        let (v_len, v_peer) = voice_server.recv_from(&mut buf).expect("recv voice disc");
        let v_req = DiscoveryPacket::parse(&buf[..v_len]).expect("parse voice req");
        assert_eq!(
            v_req,
            DiscoveryPacket::Request {
                ssrc: 9208,
                port: voice_client_port
            }
        );

        let (s_len, s_peer) = stream_server.recv_from(&mut buf).expect("recv stream disc");
        let s_req = DiscoveryPacket::parse(&buf[..s_len]).expect("parse stream req");
        assert_eq!(
            s_req,
            DiscoveryPacket::Request {
                ssrc: 40_000,
                port: stream_client_port
            }
        );

        let s_resp = DiscoveryPacket::Response {
            ssrc: 40_000,
            address: "95.164.91.98".to_owned(),
            port: 55_123,
        }
        .encode()
        .expect("encode s_resp");
        stream_server
            .send_to(&s_resp, s_peer)
            .expect("send stream resp");

        let parsed_s_resp = dual.recv_stream_discovery().expect("recv_stream_discovery");
        assert_eq!(
            parsed_s_resp,
            DiscoveryPacket::Response {
                ssrc: 40_000,
                address: "95.164.91.98".to_owned(),
                port: 55_123,
            }
        );

        dual.send_stream_keepalive(stream_srv_addr)
            .expect("send stream keepalive");
        let (ka_len, _) = stream_server.recv_from(&mut buf).expect("recv keepalive");
        assert_eq!(&buf[..ka_len], &keepalive_request(1));

        let _ = v_peer;
        assert!(dual.close_stream());
        assert!(dual.stream_socket().is_none());
        assert!(dual.stream_key().is_none());
        assert!(dual.stream_ssrcs().is_none());
    }
}
