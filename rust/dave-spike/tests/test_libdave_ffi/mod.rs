//! Safe RAII Rust FFI wrapper (`501.1`) around Discord's official `libdave` C ABI (`cpp/dave.h`).
//!
//! All `unsafe` C-ABI calls are strictly isolated inside this FFI boundary module with
//! single-owner `NonNull` handles, deterministic `Drop` destructors, and explicit bounds checks.
#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr::{self, NonNull};
use std::slice;

use dave_spike::bridge::{CommitWelcomeOutput, DaveBackend, DaveBackendKind};
use dave_spike::protocol::{
    Codec, DAVE_OPUS_SILENCE_PACKET, DaveError, MediaType, ProposalsOperation,
    generate_displayable_code,
};

#[repr(C)]
pub struct OpaqueDaveSession {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OpaqueDaveCommitResult {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OpaqueDaveWelcomeResult {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OpaqueDaveKeyRatchet {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OpaqueDaveEncryptor {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OpaqueDaveDecryptor {
    _private: [u8; 0],
}

pub type DAVESessionHandle = *mut OpaqueDaveSession;
pub type DAVECommitResultHandle = *mut OpaqueDaveCommitResult;
pub type DAVEWelcomeResultHandle = *mut OpaqueDaveWelcomeResult;
pub type DAVEKeyRatchetHandle = *mut OpaqueDaveKeyRatchet;
pub type DAVEEncryptorHandle = *mut OpaqueDaveEncryptor;
pub type DAVEDecryptorHandle = *mut OpaqueDaveDecryptor;

pub const DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS: i32 = 0;
pub const DAVE_DECRYPTOR_RESULT_CODE_SUCCESS: i32 = 0;

unsafe extern "C" {
    pub fn daveMaxSupportedProtocolVersion() -> u16;
    pub fn daveFree(ptr: *mut c_void);

    pub fn daveSessionCreate(
        context: *mut c_void,
        auth_session_id: *const i8,
        mls_failure_callback: Option<unsafe extern "C" fn(*const i8, *const i8, *mut c_void)>,
        user_data: *mut c_void,
    ) -> DAVESessionHandle;
    pub fn daveSessionDestroy(session: DAVESessionHandle);
    pub fn daveSessionInit(
        session: DAVESessionHandle,
        version: u16,
        group_id: u64,
        self_user_id: *const i8,
    ) -> bool;
    pub fn daveSessionReset(session: DAVESessionHandle);
    pub fn daveSessionGetProtocolVersion(session: DAVESessionHandle) -> u16;
    pub fn daveSessionSetExternalSender(
        session: DAVESessionHandle,
        external_sender: *const u8,
        length: usize,
    ) -> bool;
    pub fn daveSessionGetMarshalledKeyPackage(
        session: DAVESessionHandle,
        key_package: *mut *mut u8,
        length: *mut usize,
    ) -> bool;
    pub fn daveSessionProcessProposals(
        session: DAVESessionHandle,
        op_type: u8,
        proposals: *const u8,
        length: usize,
        recognized_user_ids: *const u64,
        recognized_user_ids_length: usize,
        commit_bytes: *mut *mut u8,
        commit_length: *mut usize,
        welcome_bytes: *mut *mut u8,
        welcome_length: *mut usize,
    ) -> bool;
    pub fn daveSessionProcessCommit(
        session: DAVESessionHandle,
        commit: *const u8,
        length: usize,
    ) -> DAVECommitResultHandle;
    pub fn daveSessionProcessWelcome(
        session: DAVESessionHandle,
        welcome: *const u8,
        length: usize,
        recognized_user_ids: *const u64,
        recognized_user_ids_length: usize,
    ) -> DAVEWelcomeResultHandle;
    pub fn daveSessionGetKeyRatchet(
        session: DAVESessionHandle,
        user_id: *const i8,
    ) -> DAVEKeyRatchetHandle;
    pub fn daveSessionGetLastEpochAuthenticator(
        session: DAVESessionHandle,
        authenticator: *mut *mut u8,
        length: *mut usize,
    ) -> bool;
    pub fn daveSessionGetEpoch(session: DAVESessionHandle, out_epoch: *mut u64) -> bool;
    pub fn daveSessionGetPairwiseFingerprint(
        session: DAVESessionHandle,
        version: u16,
        user_id: *const i8,
        fingerprint: *mut *mut u8,
        length: *mut usize,
    ) -> bool;

    pub fn daveCommitResultIsFailed(result: DAVECommitResultHandle) -> bool;
    pub fn daveCommitResultIsIgnored(result: DAVECommitResultHandle) -> bool;
    pub fn daveCommitResultGetRosterMemberIds(
        result: DAVECommitResultHandle,
        roster_ids: *mut *mut u64,
        roster_length: *mut usize,
    );
    pub fn daveCommitResultDestroy(result: DAVECommitResultHandle);

    pub fn daveWelcomeResultIsFailed(result: DAVEWelcomeResultHandle) -> bool;
    pub fn daveWelcomeResultGetRosterMemberIds(
        result: DAVEWelcomeResultHandle,
        roster_ids: *mut *mut u64,
        roster_length: *mut usize,
    );
    pub fn daveWelcomeResultDestroy(result: DAVEWelcomeResultHandle);

    pub fn daveKeyRatchetDestroy(key_ratchet: DAVEKeyRatchetHandle);
    pub fn daveKeyRatchetDeriveKeyAndNonceForTest(
        base_secret: *const u8,
        base_secret_len: usize,
        generation: u32,
        out_key16: *mut u8,
        out_nonce12: *mut u8,
    ) -> bool;
    pub fn daveAes128GcmTruncated8EncryptForTest(
        key16: *const u8,
        nonce12: *const u8,
        aad: *const u8,
        aad_len: usize,
        plaintext: *const u8,
        plaintext_len: usize,
        ciphertext_out: *mut u8,
        tag8_out: *mut u8,
    ) -> bool;

    pub fn daveEncryptorCreate() -> DAVEEncryptorHandle;
    pub fn daveEncryptorDestroy(encryptor: DAVEEncryptorHandle);
    pub fn daveEncryptorSetKeyRatchet(
        encryptor: DAVEEncryptorHandle,
        key_ratchet: DAVEKeyRatchetHandle,
    );
    pub fn daveEncryptorSetPassthroughMode(encryptor: DAVEEncryptorHandle, passthrough: bool);
    pub fn daveEncryptorGetMaxCiphertextByteSize(
        encryptor: DAVEEncryptorHandle,
        media_type: i32,
        frame_size: usize,
    ) -> usize;
    pub fn daveEncryptorEncrypt(
        encryptor: DAVEEncryptorHandle,
        media_type: i32,
        codec: i32,
        frame: *const u8,
        frame_length: usize,
        encrypted_frame: *mut u8,
        encrypted_frame_capacity: usize,
        bytes_written: *mut usize,
    ) -> i32;

    pub fn daveDecryptorCreate() -> DAVEDecryptorHandle;
    pub fn daveDecryptorDestroy(decryptor: DAVEDecryptorHandle);
    pub fn daveDecryptorTransitionToKeyRatchet(
        decryptor: DAVEDecryptorHandle,
        key_ratchet: DAVEKeyRatchetHandle,
    );
    pub fn daveDecryptorTransitionToPassthroughMode(
        decryptor: DAVEDecryptorHandle,
        passthrough: bool,
    );
    pub fn daveDecryptorGetMaxPlaintextByteSize(
        decryptor: DAVEDecryptorHandle,
        media_type: i32,
        encrypted_frame_size: usize,
    ) -> usize;
    pub fn daveDecryptorDecrypt(
        decryptor: DAVEDecryptorHandle,
        media_type: i32,
        encrypted_frame: *const u8,
        encrypted_frame_length: usize,
        frame: *mut u8,
        frame_capacity: usize,
        bytes_written: *mut usize,
    ) -> i32;

    pub fn daveTestCreateExternalSender(
        label: *const u8,
        label_len: usize,
        out_package: *mut *mut u8,
        out_package_len: *mut usize,
        out_private_handle: *mut *mut c_void,
    ) -> bool;
    pub fn daveTestDestroyExternalSender(private_handle: *mut c_void);
    pub fn daveTestSignAddProposal(
        private_handle: *mut c_void,
        group_id: u64,
        epoch: u64,
        key_package: *const u8,
        key_package_len: usize,
        out_payload: *mut *mut u8,
        out_payload_len: *mut usize,
    ) -> bool;
    pub fn daveTestSignRemoveProposal(
        private_handle: *mut c_void,
        group_id: u64,
        epoch: u64,
        removed_user_id: u64,
        out_payload: *mut *mut u8,
        out_payload_len: *mut usize,
    ) -> bool;
}

unsafe fn take_dave_bytes(ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        if !ptr.is_null() {
            unsafe { daveFree(ptr.cast()) };
        }
        return Vec::new();
    }
    let out = unsafe { slice::from_raw_parts(ptr, len) }.to_vec();
    unsafe { daveFree(ptr.cast()) };
    out
}

unsafe fn take_dave_u64s(ptr: *mut u64, len: usize) -> Vec<u64> {
    if ptr.is_null() || len == 0 {
        if !ptr.is_null() {
            unsafe { daveFree(ptr.cast()) };
        }
        return Vec::new();
    }
    let out = unsafe { slice::from_raw_parts(ptr, len) }.to_vec();
    unsafe { daveFree(ptr.cast()) };
    out
}

struct OwnedDecryptor(NonNull<OpaqueDaveDecryptor>);

impl OwnedDecryptor {
    fn new() -> Result<Self, DaveError> {
        let raw = unsafe { daveDecryptorCreate() };
        NonNull::new(raw).map(Self).ok_or(DaveError::BackendFailure(
            "daveDecryptorCreate returned null",
        ))
    }
}

impl Drop for OwnedDecryptor {
    fn drop(&mut self) {
        unsafe { daveDecryptorDestroy(self.0.as_ptr()) };
    }
}

/// Safe RAII backend implementing [`DaveBackend`] over Discord's official `libdave` C ABI (`501.1`).
pub struct LibdaveFfiBackend {
    session: NonNull<OpaqueDaveSession>,
    encryptor: NonNull<OpaqueDaveEncryptor>,
    decryptors: BTreeMap<u64, OwnedDecryptor>,
    self_user_id: u64,
    channel_id: u64,
    protocol_version: u16,
    roster: Vec<u64>,
    ready: bool,
}

impl LibdaveFfiBackend {
    pub fn new(protocol_version: u16, user_id: u64, channel_id: u64) -> Result<Self, DaveError> {
        if protocol_version == 0 || protocol_version > Self::max_supported_version() {
            return Err(DaveError::UnsupportedProtocolVersion(protocol_version));
        }
        let session_raw =
            unsafe { daveSessionCreate(ptr::null_mut(), ptr::null(), None, ptr::null_mut()) };
        let session = NonNull::new(session_raw)
            .ok_or(DaveError::BackendFailure("daveSessionCreate returned null"))?;
        let enc_raw = unsafe { daveEncryptorCreate() };
        let Some(encryptor) = NonNull::new(enc_raw) else {
            unsafe { daveSessionDestroy(session.as_ptr()) };
            return Err(DaveError::BackendFailure(
                "daveEncryptorCreate returned null",
            ));
        };
        let uid_c = CString::new(user_id.to_string())
            .map_err(|_| DaveError::InvalidArgument("invalid user_id string"))?;
        let ok = unsafe {
            daveSessionInit(
                session.as_ptr(),
                protocol_version,
                channel_id,
                uid_c.as_ptr(),
            )
        };
        if !ok {
            unsafe {
                daveEncryptorDestroy(encryptor.as_ptr());
                daveSessionDestroy(session.as_ptr());
            }
            return Err(DaveError::BackendFailure("daveSessionInit failed"));
        }
        Ok(Self {
            session,
            encryptor,
            decryptors: BTreeMap::new(),
            self_user_id: user_id,
            channel_id,
            protocol_version,
            roster: Vec::new(),
            ready: false,
        })
    }

    pub fn max_supported_version() -> u16 {
        unsafe { daveMaxSupportedProtocolVersion() }
    }

    pub fn pairwise_fingerprint(
        &self,
        version: u16,
        peer_user_id: u64,
    ) -> Result<Vec<u8>, DaveError> {
        let uid_c = CString::new(peer_user_id.to_string())
            .map_err(|_| DaveError::InvalidArgument("invalid peer user_id"))?;
        let mut ptr_out: *mut u8 = ptr::null_mut();
        let mut len_out: usize = 0;
        let ok = unsafe {
            daveSessionGetPairwiseFingerprint(
                self.session.as_ptr(),
                version,
                uid_c.as_ptr(),
                &mut ptr_out,
                &mut len_out,
            )
        };
        if !ok {
            return Err(DaveError::InvalidState("pairwise fingerprint unavailable"));
        }
        Ok(unsafe { take_dave_bytes(ptr_out, len_out) })
    }

    fn install_ratchets_for_roster(&mut self, members: Vec<u64>) -> Result<(), DaveError> {
        let self_c = CString::new(self.self_user_id.to_string())
            .map_err(|_| DaveError::InvalidArgument("invalid self user id"))?;
        let self_ratchet =
            unsafe { daveSessionGetKeyRatchet(self.session.as_ptr(), self_c.as_ptr()) };
        if self_ratchet.is_null() {
            return Err(DaveError::BackendFailure(
                "missing own key ratchet from libdave",
            ));
        }
        unsafe {
            daveEncryptorSetKeyRatchet(self.encryptor.as_ptr(), self_ratchet);
            daveKeyRatchetDestroy(self_ratchet);
        }

        for &uid in &members {
            if uid == self.self_user_id {
                continue;
            }
            let uid_c = CString::new(uid.to_string())
                .map_err(|_| DaveError::InvalidArgument("invalid member user id"))?;
            let peer_ratchet =
                unsafe { daveSessionGetKeyRatchet(self.session.as_ptr(), uid_c.as_ptr()) };
            if peer_ratchet.is_null() {
                return Err(DaveError::BackendFailure(
                    "missing peer key ratchet from libdave",
                ));
            }
            let dec = match self.decryptors.entry(uid) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => e.insert(OwnedDecryptor::new()?),
            };
            unsafe { daveDecryptorTransitionToKeyRatchet(dec.0.as_ptr(), peer_ratchet) };
            unsafe { daveKeyRatchetDestroy(peer_ratchet) };
        }
        self.decryptors.retain(|uid, _| members.contains(uid));
        self.roster = members;
        self.ready = true;
        Ok(())
    }
}

impl Drop for LibdaveFfiBackend {
    fn drop(&mut self) {
        self.decryptors.clear();
        unsafe {
            daveEncryptorDestroy(self.encryptor.as_ptr());
            daveSessionDestroy(self.session.as_ptr());
        }
    }
}

impl DaveBackend for LibdaveFfiBackend {
    fn kind(&self) -> DaveBackendKind {
        DaveBackendKind::LibdaveFfi
    }

    fn protocol_version(&self) -> u16 {
        unsafe { daveSessionGetProtocolVersion(self.session.as_ptr()) }
    }

    fn user_id(&self) -> u64 {
        self.self_user_id
    }

    fn channel_id(&self) -> u64 {
        self.channel_id
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn epoch(&self) -> Option<u64> {
        let mut ep = 0u64;
        let ok = unsafe { daveSessionGetEpoch(self.session.as_ptr(), &mut ep) };
        ok.then_some(ep)
    }

    fn member_ids(&self) -> Vec<u64> {
        self.roster.clone()
    }

    fn reset(&mut self) -> Result<(), DaveError> {
        let uid_c = CString::new(self.self_user_id.to_string())
            .map_err(|_| DaveError::InvalidArgument("invalid self user id"))?;
        unsafe {
            daveSessionReset(self.session.as_ptr());
            daveSessionInit(
                self.session.as_ptr(),
                self.protocol_version,
                self.channel_id,
                uid_c.as_ptr(),
            );
        }
        self.decryptors.clear();
        self.roster.clear();
        self.ready = false;
        Ok(())
    }

    fn set_external_sender(&mut self, external_sender: &[u8]) -> Result<(), DaveError> {
        if external_sender.is_empty() {
            return Err(DaveError::MalformedWire("empty external sender"));
        }
        let ok = unsafe {
            daveSessionSetExternalSender(
                self.session.as_ptr(),
                external_sender.as_ptr(),
                external_sender.len(),
            )
        };
        if !ok {
            return Err(DaveError::MalformedWire("libdave rejected external sender"));
        }
        self.roster = vec![self.self_user_id];
        Ok(())
    }

    fn create_key_package(&mut self) -> Result<Vec<u8>, DaveError> {
        let mut ptr_out: *mut u8 = ptr::null_mut();
        let mut len_out: usize = 0;
        let ok = unsafe {
            daveSessionGetMarshalledKeyPackage(self.session.as_ptr(), &mut ptr_out, &mut len_out)
        };
        if !ok {
            return Err(DaveError::BackendFailure(
                "daveSessionGetMarshalledKeyPackage failed",
            ));
        }
        Ok(unsafe { take_dave_bytes(ptr_out, len_out) })
    }

    fn process_proposals(
        &mut self,
        operation: ProposalsOperation,
        proposals: &[u8],
        expected_users: &[u64],
    ) -> Result<Option<CommitWelcomeOutput>, DaveError> {
        let op_u8 = match operation {
            ProposalsOperation::Append => 0u8,
            ProposalsOperation::Revoke => 1u8,
        };
        let mut commit_ptr: *mut u8 = ptr::null_mut();
        let mut commit_len: usize = 0;
        let mut welcome_ptr: *mut u8 = ptr::null_mut();
        let mut welcome_len: usize = 0;
        let ok = unsafe {
            daveSessionProcessProposals(
                self.session.as_ptr(),
                op_u8,
                proposals.as_ptr(),
                proposals.len(),
                expected_users.as_ptr(),
                expected_users.len(),
                &mut commit_ptr,
                &mut commit_len,
                &mut welcome_ptr,
                &mut welcome_len,
            )
        };
        if !ok {
            return Err(DaveError::MalformedWire(
                "libdave proposal validation failed",
            ));
        }
        let commit = unsafe { take_dave_bytes(commit_ptr, commit_len) };
        let welcome = unsafe { take_dave_bytes(welcome_ptr, welcome_len) };
        if commit.is_empty() {
            return Ok(None);
        }
        Ok(Some(CommitWelcomeOutput {
            commit,
            welcome: (!welcome.is_empty()).then_some(welcome),
        }))
    }

    fn process_commit(&mut self, commit: &[u8]) -> Result<(), DaveError> {
        if commit.is_empty() {
            return Err(DaveError::MalformedWire("empty commit"));
        }
        let handle = unsafe {
            daveSessionProcessCommit(self.session.as_ptr(), commit.as_ptr(), commit.len())
        };
        if handle.is_null() {
            return Err(DaveError::MalformedWire("null commit result"));
        }
        let failed = unsafe { daveCommitResultIsFailed(handle) };
        let ignored = unsafe { daveCommitResultIsIgnored(handle) };
        let mut ids_ptr: *mut u64 = ptr::null_mut();
        let mut ids_len: usize = 0;
        if !failed && !ignored {
            unsafe { daveCommitResultGetRosterMemberIds(handle, &mut ids_ptr, &mut ids_len) };
        }
        let members = unsafe { take_dave_u64s(ids_ptr, ids_len) };
        unsafe { daveCommitResultDestroy(handle) };
        if failed || ignored {
            return Err(DaveError::CommitRejected);
        }
        self.install_ratchets_for_roster(members)
    }

    fn process_welcome(&mut self, welcome: &[u8], expected_users: &[u64]) -> Result<(), DaveError> {
        if welcome.is_empty() {
            return Err(DaveError::MalformedWire("empty welcome"));
        }
        let handle = unsafe {
            daveSessionProcessWelcome(
                self.session.as_ptr(),
                welcome.as_ptr(),
                welcome.len(),
                expected_users.as_ptr(),
                expected_users.len(),
            )
        };
        if handle.is_null() {
            return Err(DaveError::WelcomeRejected);
        }
        let failed = unsafe { daveWelcomeResultIsFailed(handle) };
        let mut ids_ptr: *mut u64 = ptr::null_mut();
        let mut ids_len: usize = 0;
        if !failed {
            unsafe { daveWelcomeResultGetRosterMemberIds(handle, &mut ids_ptr, &mut ids_len) };
        }
        let members = unsafe { take_dave_u64s(ids_ptr, ids_len) };
        unsafe { daveWelcomeResultDestroy(handle) };
        if failed {
            return Err(DaveError::WelcomeRejected);
        }
        self.install_ratchets_for_roster(members)
    }

    fn epoch_authenticator(&self) -> Option<Vec<u8>> {
        let mut ptr_out: *mut u8 = ptr::null_mut();
        let mut len_out: usize = 0;
        let ok = unsafe {
            daveSessionGetLastEpochAuthenticator(self.session.as_ptr(), &mut ptr_out, &mut len_out)
        };
        if !ok {
            return None;
        }
        Some(unsafe { take_dave_bytes(ptr_out, len_out) })
    }

    fn voice_privacy_code(&self) -> Option<String> {
        let auth = self.epoch_authenticator()?;
        generate_displayable_code(&auth, 30, 5).ok()
    }

    fn set_passthrough_mode(&mut self, passthrough: bool) {
        unsafe { daveEncryptorSetPassthroughMode(self.encryptor.as_ptr(), passthrough) };
        for dec in self.decryptors.values() {
            unsafe { daveDecryptorTransitionToPassthroughMode(dec.0.as_ptr(), passthrough) };
        }
    }

    fn encrypt(
        &mut self,
        media_type: MediaType,
        codec: Codec,
        frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        if !self.ready {
            return Err(DaveError::NotReady);
        }
        if media_type == MediaType::Audio && frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        let cap = unsafe {
            daveEncryptorGetMaxCiphertextByteSize(
                self.encryptor.as_ptr(),
                media_type.as_i32(),
                frame.len(),
            )
        };
        let mut out = vec![0u8; cap];
        let mut written = 0usize;
        let rc = unsafe {
            daveEncryptorEncrypt(
                self.encryptor.as_ptr(),
                media_type.as_i32(),
                codec.as_i32(),
                frame.as_ptr(),
                frame.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
            )
        };
        if rc != DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS {
            return Err(DaveError::EncryptionFailed);
        }
        out.truncate(written);
        Ok(out)
    }

    fn decrypt(
        &mut self,
        sender_user_id: u64,
        media_type: MediaType,
        encrypted_frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        if media_type == MediaType::Audio && encrypted_frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        let dec = self
            .decryptors
            .get(&sender_user_id)
            .ok_or(DaveError::UnknownSender(sender_user_id))?;
        let cap = unsafe {
            daveDecryptorGetMaxPlaintextByteSize(
                dec.0.as_ptr(),
                media_type.as_i32(),
                encrypted_frame.len(),
            )
        };
        let mut out = vec![0u8; cap.max(DAVE_OPUS_SILENCE_PACKET.len())];
        let mut written = 0usize;
        let rc = unsafe {
            daveDecryptorDecrypt(
                dec.0.as_ptr(),
                media_type.as_i32(),
                encrypted_frame.as_ptr(),
                encrypted_frame.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
            )
        };
        if rc != DAVE_DECRYPTOR_RESULT_CODE_SUCCESS {
            return Err(DaveError::DecryptionFailed);
        }
        out.truncate(written);
        Ok(out)
    }
}

/// RAII helper for C++ OpenSSL external sender test authority.
pub struct CppExternalDelivery {
    pub external_package: Vec<u8>,
    handle: NonNull<c_void>,
}

impl CppExternalDelivery {
    pub fn new(label: &[u8]) -> Self {
        let mut pkg_ptr: *mut u8 = ptr::null_mut();
        let mut pkg_len: usize = 0;
        let mut priv_ptr: *mut c_void = ptr::null_mut();
        let ok = unsafe {
            daveTestCreateExternalSender(
                label.as_ptr(),
                label.len(),
                &mut pkg_ptr,
                &mut pkg_len,
                &mut priv_ptr,
            )
        };
        assert!(ok, "daveTestCreateExternalSender must succeed");
        Self {
            external_package: unsafe { take_dave_bytes(pkg_ptr, pkg_len) },
            handle: NonNull::new(priv_ptr).expect("non-null external sender handle"),
        }
    }

    pub fn sign_add(&self, group_id: u64, epoch: u64, key_package: &[u8]) -> Vec<u8> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let ok = unsafe {
            daveTestSignAddProposal(
                self.handle.as_ptr(),
                group_id,
                epoch,
                key_package.as_ptr(),
                key_package.len(),
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert!(ok, "daveTestSignAddProposal must succeed");
        unsafe { take_dave_bytes(out_ptr, out_len) }
    }

    pub fn sign_remove(&self, group_id: u64, epoch: u64, removed_user_id: u64) -> Vec<u8> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let ok = unsafe {
            daveTestSignRemoveProposal(
                self.handle.as_ptr(),
                group_id,
                epoch,
                removed_user_id,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert!(ok, "daveTestSignRemoveProposal must succeed");
        unsafe { take_dave_bytes(out_ptr, out_len) }
    }
}

impl Drop for CppExternalDelivery {
    fn drop(&mut self) {
        unsafe { daveTestDestroyExternalSender(self.handle.as_ptr()) };
    }
}

pub fn cpp_derive_ratchet_key_and_nonce(
    base_secret: &[u8; 16],
    generation: u32,
) -> ([u8; 16], [u8; 12]) {
    let mut k = [0u8; 16];
    let mut n = [0u8; 12];
    let ok = unsafe {
        daveKeyRatchetDeriveKeyAndNonceForTest(
            base_secret.as_ptr(),
            base_secret.len(),
            generation,
            k.as_mut_ptr(),
            n.as_mut_ptr(),
        )
    };
    assert!(ok, "daveKeyRatchetDeriveKeyAndNonceForTest must succeed");
    (k, n)
}

pub fn cpp_aes128_gcm_truncated8_encrypt(
    key16: &[u8; 16],
    nonce12: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; 8]) {
    let mut ct = vec![0u8; plaintext.len()];
    let mut tag8 = [0u8; 8];
    let ok = unsafe {
        daveAes128GcmTruncated8EncryptForTest(
            key16.as_ptr(),
            nonce12.as_ptr(),
            aad.as_ptr(),
            aad.len(),
            plaintext.as_ptr(),
            plaintext.len(),
            ct.as_mut_ptr(),
            tag8.as_mut_ptr(),
        )
    };
    assert!(ok, "daveAes128GcmTruncated8EncryptForTest must succeed");
    (ct, tag8)
}
