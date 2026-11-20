//! Unified transparent adapter `DaveVoiceBridge` (`502.1`):
//! `Voice Gateway (Op 18..22 & Op 21..31) -> DaveSessionManager -> DaveBackend (LibdaveFfi | DaveySession)`.

use crate::protocol::{
    Codec, DAVE_PROTOCOL_VERSION_V1, DaveError, MAX_DAVE_RESETS, MAX_VOICE_PARTICIPANTS, MediaType,
    OP_DAVE_EXECUTE_TRANSITION, OP_DAVE_MLS_ANNOUNCE_COMMIT, OP_DAVE_MLS_COMMIT_WELCOME,
    OP_DAVE_MLS_EXTERNAL_SENDER, OP_DAVE_MLS_INVALID_COMMIT_WELCOME, OP_DAVE_MLS_KEY_PACKAGE,
    OP_DAVE_MLS_PROPOSALS, OP_DAVE_MLS_WELCOME, OP_DAVE_PREPARE_EPOCH, OP_DAVE_PREPARE_TRANSITION,
    OP_DAVE_READY_FOR_TRANSITION, OP_STREAM_CREATE, OP_STREAM_DELETE, OP_STREAM_PING,
    OP_STREAM_SET_PAUSED, OP_STREAM_WATCH, ProposalsOperation,
};

/// Discriminator identifying which underlying E2EE engine powers `DaveVoiceBridge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaveBackendKind {
    LibdaveFfi,
    DaveyRustCrypto,
}

/// Output of `DaveBackend::process_proposals` ready for Opcode `28` (`dave_mls_commit_welcome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitWelcomeOutput {
    pub commit: Vec<u8>,
    pub welcome: Option<Vec<u8>>,
}

/// Pluggable backend contract uniting official C++ `libdave` FFI (`501.1`) and Serein `davey` (`501.2`).
pub trait DaveBackend {
    fn kind(&self) -> DaveBackendKind;
    fn protocol_version(&self) -> u16;
    fn user_id(&self) -> u64;
    fn channel_id(&self) -> u64;
    fn is_ready(&self) -> bool;
    fn epoch(&self) -> Option<u64>;
    fn member_ids(&self) -> Vec<u64>;
    fn reset(&mut self) -> Result<(), DaveError>;
    fn set_external_sender(&mut self, external_sender: &[u8]) -> Result<(), DaveError>;
    fn create_key_package(&mut self) -> Result<Vec<u8>, DaveError>;
    fn process_proposals(
        &mut self,
        operation: ProposalsOperation,
        proposals: &[u8],
        expected_users: &[u64],
    ) -> Result<Option<CommitWelcomeOutput>, DaveError>;
    fn process_commit(&mut self, commit: &[u8]) -> Result<(), DaveError>;
    fn process_welcome(&mut self, welcome: &[u8], expected_users: &[u64]) -> Result<(), DaveError>;
    fn epoch_authenticator(&self) -> Option<Vec<u8>>;
    fn voice_privacy_code(&self) -> Option<String>;
    fn set_passthrough_mode(&mut self, passthrough: bool);
    fn encrypt(
        &mut self,
        media_type: MediaType,
        codec: Codec,
        frame: &[u8],
    ) -> Result<Vec<u8>, DaveError>;
    fn decrypt(
        &mut self,
        sender_uid: u64,
        media_type: MediaType,
        frame: &[u8],
    ) -> Result<Vec<u8>, DaveError>;
}

/// Preserved Main Gateway stream signaling event (`Opcodes 18..=22`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamSignalingEvent {
    Create { payload: String },
    Delete { payload: String },
    Watch { payload: String },
    Ping { payload: String },
    SetPaused { payload: String },
}

/// Outbound Voice Gateway control message emitted by `DaveVoiceBridge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeOutboundFrame {
    Binary { opcode: u8, payload: Vec<u8> },
    JsonReadyForTransition { opcode: u8, transition_id: u16 },
    JsonInvalidCommitWelcome { opcode: u8, transition_id: u16 },
}

/// Transparent Voice Gateway (`Op 18..22` / `Op 21..31`) session manager & E2EE bridge (`502.1`).
pub struct DaveVoiceBridge<B: DaveBackend> {
    backend: B,
    dm_peer: Option<u64>,
    participants: Vec<u64>,
    announced: Vec<u64>,
    pending_mls_transition: Option<u16>,
    pending_protocol_transition: Option<(u16, u16)>,
    active_protocol_version: u16,
    ready: bool,
    waiting_for_peer: bool,
    resets: u8,
}

impl<B: DaveBackend> DaveVoiceBridge<B> {
    pub fn new(backend: B, dm_peer: Option<u64>) -> Result<Self, DaveError> {
        let own = backend.user_id();
        if own == 0 || dm_peer == Some(0) {
            return Err(DaveError::InvalidArgument(
                "participant user_id cannot be 0",
            ));
        }
        let mut participants = vec![own];
        if let Some(peer) = dm_peer
            && peer != own
        {
            participants.push(peer);
        }
        Ok(Self {
            backend,
            dm_peer,
            participants,
            announced: vec![own],
            pending_mls_transition: None,
            pending_protocol_transition: None,
            active_protocol_version: 0,
            ready: false,
            waiting_for_peer: false,
            resets: 0,
        })
    }

    #[must_use]
    pub fn backend_kind(&self) -> DaveBackendKind {
        self.backend.kind()
    }

    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    #[must_use]
    pub const fn is_waiting_for_peer(&self) -> bool {
        self.waiting_for_peer
    }

    #[must_use]
    pub fn voice_privacy_code(&self) -> Option<String> {
        self.backend.voice_privacy_code()
    }

    /// Handles Go Live / Stream signaling opcodes `18..=22` (`what_to_keep`).
    pub fn handle_stream_opcode(
        &self,
        opcode: u8,
        json_payload: &str,
    ) -> Result<StreamSignalingEvent, DaveError> {
        if json_payload.trim().is_empty() {
            return Err(DaveError::MalformedWire("empty stream opcode payload"));
        }
        let payload = json_payload.to_string();
        match opcode {
            OP_STREAM_CREATE => Ok(StreamSignalingEvent::Create { payload }),
            OP_STREAM_DELETE => Ok(StreamSignalingEvent::Delete { payload }),
            OP_STREAM_WATCH => Ok(StreamSignalingEvent::Watch { payload }),
            OP_STREAM_PING => Ok(StreamSignalingEvent::Ping { payload }),
            OP_STREAM_SET_PAUSED => Ok(StreamSignalingEvent::SetPaused { payload }),
            other => Err(DaveError::UnsupportedOpcode(other)),
        }
    }

    /// Registers newly connected voice channel participants (`CLIENTS_CONNECT`, max 64).
    pub fn connect_users(&mut self, users: &[u64]) -> Result<bool, DaveError> {
        let own = self.backend.user_id();
        if users
            .iter()
            .any(|&u| u == 0 || self.dm_peer.is_some_and(|p| u != own && u != p))
        {
            return Err(DaveError::UnrecognizedParticipant(0));
        }
        let mut next_part = self.participants.clone();
        let mut next_ann = self.announced.clone();
        for &u in users {
            if !next_part.contains(&u) {
                if next_part.len() >= MAX_VOICE_PARTICIPANTS {
                    return Err(DaveError::ParticipantLimitExceeded(MAX_VOICE_PARTICIPANTS));
                }
                next_part.push(u);
            }
            if !next_ann.contains(&u) {
                next_ann.push(u);
            }
        }
        let changed = next_part != self.participants || next_ann != self.announced;
        if changed {
            self.participants = next_part;
            self.announced = next_ann;
            self.waiting_for_peer = false;
        }
        Ok(changed)
    }

    /// Handles `CLIENT_DISCONNECT` for a departing voice participant.
    pub fn disconnect_user(&mut self, user_id: u64) -> Result<bool, DaveError> {
        if user_id == self.backend.user_id() {
            return Err(DaveError::InvalidState("local user disconnected from call"));
        }
        let before = self.announced.len();
        self.announced.retain(|&u| u != user_id);
        if !self.backend.member_ids().contains(&user_id) {
            self.participants.retain(|&u| u != user_id);
        }
        let changed = self.announced.len() != before;
        if changed {
            self.ready = false;
            self.waiting_for_peer = false;
        }
        Ok(changed)
    }

    /// Transitions a sole announced channel member into epoch-0 waiting state (`Serein` invariant).
    pub fn enter_sole_member_waiting(&mut self) -> Result<(), DaveError> {
        if self.announced.len() != 1 {
            return Err(DaveError::InvalidState(
                "cannot enter sole-member waiting with peers present",
            ));
        }
        if self.backend.epoch().unwrap_or(0) != 0 {
            self.backend.reset()?;
        }
        self.pending_mls_transition = None;
        self.pending_protocol_transition = None;
        self.ready = false;
        self.waiting_for_peer = true;
        Ok(())
    }

    /// Handles Opcode `21` (`dave_protocol_prepare_transition`) with downgrade protection.
    pub fn handle_prepare_transition(
        &mut self,
        opcode: u8,
        transition_id: u16,
        protocol_version: u16,
    ) -> Result<Vec<BridgeOutboundFrame>, DaveError> {
        if opcode != OP_DAVE_PREPARE_TRANSITION {
            return Err(DaveError::UnsupportedOpcode(opcode));
        }
        if protocol_version == 0 && self.active_protocol_version >= DAVE_PROTOCOL_VERSION_V1 {
            return Err(DaveError::DowngradeRejected {
                current: self.active_protocol_version,
                requested: protocol_version,
            });
        }
        if protocol_version > DAVE_PROTOCOL_VERSION_V1 {
            return Err(DaveError::UnsupportedProtocolVersion(protocol_version));
        }
        self.pending_protocol_transition = Some((transition_id, protocol_version));
        if transition_id == 0 {
            self.handle_execute_transition(OP_DAVE_EXECUTE_TRANSITION, 0)?;
            return Ok(Vec::new());
        }
        Ok(vec![BridgeOutboundFrame::JsonReadyForTransition {
            opcode: OP_DAVE_READY_FOR_TRANSITION,
            transition_id,
        }])
    }

    /// Handles Opcode `24` (`dave_protocol_prepare_epoch`). When `epoch == 1`, resets & emits Opcode `26` `KeyPackage`.
    pub fn handle_prepare_epoch(
        &mut self,
        opcode: u8,
        epoch: u64,
        protocol_version: u16,
    ) -> Result<Vec<BridgeOutboundFrame>, DaveError> {
        if opcode != OP_DAVE_PREPARE_EPOCH {
            return Err(DaveError::UnsupportedOpcode(opcode));
        }
        if protocol_version != DAVE_PROTOCOL_VERSION_V1 {
            return Err(DaveError::UnsupportedProtocolVersion(protocol_version));
        }
        if epoch == 1 {
            self.backend.reset()?;
            let kp = self.backend.create_key_package()?;
            return Ok(vec![BridgeOutboundFrame::Binary {
                opcode: OP_DAVE_MLS_KEY_PACKAGE,
                payload: kp,
            }]);
        }
        Ok(Vec::new())
    }

    /// Handles Opcode `22` (`dave_protocol_execute_transition`), activating prepared MLS/protocol transitions.
    pub fn handle_execute_transition(
        &mut self,
        opcode: u8,
        transition_id: u16,
    ) -> Result<(), DaveError> {
        if opcode != OP_DAVE_EXECUTE_TRANSITION {
            return Err(DaveError::UnsupportedOpcode(opcode));
        }
        let is_proto = self
            .pending_protocol_transition
            .is_some_and(|(id, _)| id == transition_id);
        let is_mls = self.pending_mls_transition == Some(transition_id);
        if !is_proto && !is_mls {
            return Err(DaveError::InvalidState("unexpected DAVE transition_id"));
        }
        if let Some((_, ver)) = self
            .pending_protocol_transition
            .take_if(|(id, _)| *id == transition_id)
        {
            self.active_protocol_version = ver;
            self.backend.set_passthrough_mode(ver == 0);
        }
        if is_mls {
            if !self.backend.is_ready() {
                return Err(DaveError::NotReady);
            }
            self.pending_mls_transition = None;
            self.active_protocol_version = DAVE_PROTOCOL_VERSION_V1;
            self.backend.set_passthrough_mode(false);
            self.ready = true;
        }
        Ok(())
    }

    /// Routes binary MLS Voice Gateway frames (`Opcodes 25, 27, 29, 30`).
    pub fn handle_binary_opcode(
        &mut self,
        opcode: u8,
        payload: &[u8],
    ) -> Result<Vec<BridgeOutboundFrame>, DaveError> {
        match opcode {
            OP_DAVE_MLS_EXTERNAL_SENDER => {
                self.backend.set_external_sender(payload)?;
                Ok(Vec::new())
            }
            OP_DAVE_MLS_PROPOSALS => self.on_mls_proposals(payload),
            OP_DAVE_MLS_ANNOUNCE_COMMIT | OP_DAVE_MLS_WELCOME => {
                self.on_mls_group_transition(opcode, payload)
            }
            other => Err(DaveError::UnsupportedOpcode(other)),
        }
    }

    pub fn encrypt_opus(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if !self.ready {
            return Err(DaveError::NotReady);
        }
        self.backend
            .encrypt(MediaType::Audio, Codec::Opus, opus_frame)
    }

    pub fn decrypt_opus(&mut self, sender_uid: u64, frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if !self.ready {
            return Err(DaveError::NotReady);
        }
        self.backend.decrypt(sender_uid, MediaType::Audio, frame)
    }

    fn on_mls_proposals(&mut self, payload: &[u8]) -> Result<Vec<BridgeOutboundFrame>, DaveError> {
        let (&op_raw, _) = payload
            .split_first()
            .ok_or(DaveError::MalformedWire("empty op 27"))?;
        let op = ProposalsOperation::from_u8(op_raw)?;
        let out = self
            .backend
            .process_proposals(op, payload, &self.participants)?;
        let Some(cw) = out else {
            return Ok(Vec::new());
        };
        let mut wire = cw.commit;
        if let Some(w) = cw.welcome {
            wire.extend_from_slice(&w);
        }
        Ok(vec![BridgeOutboundFrame::Binary {
            opcode: OP_DAVE_MLS_COMMIT_WELCOME,
            payload: wire,
        }])
    }

    fn on_mls_group_transition(
        &mut self,
        opcode: u8,
        payload: &[u8],
    ) -> Result<Vec<BridgeOutboundFrame>, DaveError> {
        if payload.len() < 3 {
            return Err(DaveError::MalformedWire("truncated MLS group transition"));
        }
        let transition_id = u16::from_be_bytes([payload[0], payload[1]]);
        let mls_bytes = &payload[2..];
        let res = if opcode == OP_DAVE_MLS_ANNOUNCE_COMMIT {
            self.backend.process_commit(mls_bytes)
        } else {
            self.backend.process_welcome(mls_bytes, &self.participants)
        };
        if res.is_err() {
            return self.recover_from_invalid_transition(transition_id);
        }
        self.pending_mls_transition = Some(transition_id);
        self.waiting_for_peer = false;
        if transition_id == 0 {
            self.handle_execute_transition(OP_DAVE_EXECUTE_TRANSITION, 0)?;
            return Ok(Vec::new());
        }
        Ok(vec![BridgeOutboundFrame::JsonReadyForTransition {
            opcode: OP_DAVE_READY_FOR_TRANSITION,
            transition_id,
        }])
    }

    fn recover_from_invalid_transition(
        &mut self,
        transition_id: u16,
    ) -> Result<Vec<BridgeOutboundFrame>, DaveError> {
        self.resets = self.resets.saturating_add(1);
        if self.resets > MAX_DAVE_RESETS {
            return Err(DaveError::RecoveryLimitReached);
        }
        self.ready = false;
        self.pending_mls_transition = None;
        self.backend.reset()?;
        let fresh_kp = self.backend.create_key_package()?;
        Ok(vec![
            BridgeOutboundFrame::JsonInvalidCommitWelcome {
                opcode: OP_DAVE_MLS_INVALID_COMMIT_WELCOME,
                transition_id,
            },
            BridgeOutboundFrame::Binary {
                opcode: OP_DAVE_MLS_KEY_PACKAGE,
                payload: fresh_kp,
            },
        ])
    }
}
