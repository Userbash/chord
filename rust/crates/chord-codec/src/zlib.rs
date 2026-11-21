#![forbid(unsafe_code)]

//! # Streaming `zlib-ng` / `flate2` Decompressor for Discord Gateway Payloads (`STAGE-05.1.2`)
//!
//! Discord Gateway (`?compress=zlib-stream`) compresses all server-to-client packets within a
//! single continuous RFC 1950 zlib stream per WebSocket connection:
//! - The 2-byte RFC 1950 zlib header (`CMF`/`FLG`, typically `0x78 0x9c`) is emitted only once at
//!   the start of the connection (frame 0).
//! - At the end of each Gateway payload, the server performs a `Z_SYNC_FLUSH`, appending the
//!   4-byte sync-flush trailer `[0x00, 0x00, 0xff, 0xff]` ([`ZLIB_SYNC_FLUSH_SUFFIX`]).
//! - Large payloads may be fragmented across multiple WebSocket binary messages; intermediate
//!   fragments do **not** end with `[0x00, 0x00, 0xff, 0xff]` and are buffered until the trailer
//!   arrives.
//! - The sliding-window LZ77 dictionary (32 KiB) persists across all frames in the session,
//!   yielding **>= 80% traffic savings** on repetitive Gateway JSON/ETF payloads.

use crate::zstd::{ZstdError, ZstdStream};
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use std::fmt;

/// Discord Gateway `Z_SYNC_FLUSH` trailer marking the end of a logical compressed message.
pub const ZLIB_SYNC_FLUSH_SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

/// Reusable output scratch buffer capacity per inflate pass (64 KiB).
const TARGET_CAPACITY: usize = 64 * 1024;

/// Maximum decompressed bytes emitted by a single [`ZlibStream::push`] call (16 MiB zip-bomb guard).
pub const MAX_DECOMPRESSED_CHUNK_BYTES: usize = 16 * 1024 * 1024;

/// Maximum compressed bytes buffered in [`ZlibStream`] awaiting `Z_SYNC_FLUSH` (8 MiB OOM guard).
pub const MAX_PENDING_COMPRESSED_BYTES: usize = 8 * 1024 * 1024;

/// Errors produced by [`ZlibStream`] and [`GatewayDecompressor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZlibError {
    /// Initial stream bytes do not form a valid RFC 1950 zlib header (`CMF`/`FLG`).
    InvalidHeader {
        /// Raw CMF byte.
        cmf: u8,
        /// Raw FLG byte.
        flg: u8,
    },
    /// Underlying `flate2` / `zlib-rs` stream rejected corrupt DEFLATE data.
    Decode {
        /// Diagnostic message from `flate2`.
        msg: String,
    },
    /// Decompressed output for a single Gateway message exceeded the zip-bomb cap.
    ChunkTooLarge {
        /// Bytes produced so far.
        produced: usize,
        /// Configured limit in bytes.
        limit: usize,
    },
    /// Unflushed pending compressed fragments exceeded the memory cap without a sync-flush suffix.
    PendingOverflow {
        /// Pending compressed byte count.
        pending: usize,
        /// Configured limit in bytes.
        limit: usize,
    },
    /// Underlying `zstd-stream` error when using [`GatewayDecompressor`].
    Zstd(ZstdError),
    /// The stream previously encountered a terminal error and must be reset or recreated.
    Poisoned,
}

impl fmt::Display for ZlibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader { cmf, flg } => write!(
                f,
                "invalid RFC 1950 zlib header (cmf=0x{cmf:02x}, flg=0x{flg:02x})"
            ),
            Self::Decode { msg } => write!(f, "zlib stream decompression error: {msg}"),
            Self::ChunkTooLarge { produced, limit } => write!(
                f,
                "decompressed Gateway chunk ({produced} B) exceeds limit ({limit} B)"
            ),
            Self::PendingOverflow { pending, limit } => write!(
                f,
                "pending compressed Gateway buffer ({pending} B) exceeds limit ({limit} B)"
            ),
            Self::Zstd(err) => write!(f, "{err}"),
            Self::Poisoned => write!(f, "zlib stream is poisoned after a prior error"),
        }
    }
}

impl std::error::Error for ZlibError {}

impl From<ZstdError> for ZlibError {
    fn from(err: ZstdError) -> Self {
        Self::Zstd(err)
    }
}

/// Telemetry counters tracking wire compression efficiency (`>= 80%` traffic savings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompressionStats {
    /// Total compressed wire bytes consumed by the decompressor.
    pub compressed_bytes: u64,
    /// Total decompressed payload bytes emitted.
    pub decompressed_bytes: u64,
    /// Number of complete Gateway messages decompressed.
    pub messages_decompressed: u64,
}

impl CompressionStats {
    /// Fractional traffic savings in `0.0..=1.0`: `1.0 - (compressed_bytes / decompressed_bytes)`.
    pub fn traffic_savings_ratio(&self) -> f64 {
        if self.decompressed_bytes == 0 || self.compressed_bytes >= self.decompressed_bytes {
            return 0.0;
        }
        1.0 - (self.compressed_bytes as f64 / self.decompressed_bytes as f64)
    }

    /// Percentage of bandwidth saved (`0.0..=100.0`).
    pub fn traffic_savings_percent(&self) -> f64 {
        self.traffic_savings_ratio() * 100.0
    }
}

/// Validates the 2-byte RFC 1950 zlib header (`CMF` and `FLG`).
fn is_valid_rfc1950_zlib_header(cmf: u8, flg: u8) -> bool {
    let method = cmf & 0x0f;
    let cinfo = (cmf >> 4) & 0x0f;
    let check = (u16::from(cmf) << 8) | u16::from(flg);
    method == 8 && cinfo <= 7 && check.is_multiple_of(31)
}

/// Stateful streaming `zlib-stream` decompressor backed by `flate2::Decompress` (`zlib-rs` / `zlib-ng`).
pub struct ZlibStream {
    decompressor: Decompress,
    pending: Vec<u8>,
    target: Vec<u8>,
    header_checked: bool,
    dead: bool,
    compressed_bytes: u64,
    decompressed_bytes: u64,
    messages_decompressed: u64,
    max_decompressed_chunk: usize,
    max_pending_compressed: usize,
}

impl ZlibStream {
    /// Creates a new streaming `zlib-stream` decompressor with default security limits.
    pub fn new() -> Self {
        Self::with_limits(MAX_DECOMPRESSED_CHUNK_BYTES, MAX_PENDING_COMPRESSED_BYTES)
    }

    /// Creates a new streaming `zlib-stream` decompressor with custom chunk and pending limits.
    pub fn with_limits(max_decompressed_chunk: usize, max_pending_compressed: usize) -> Self {
        Self {
            decompressor: Decompress::new(true),
            pending: Vec::new(),
            target: vec![0u8; TARGET_CAPACITY],
            header_checked: false,
            dead: false,
            compressed_bytes: 0,
            decompressed_bytes: 0,
            messages_decompressed: 0,
            max_decompressed_chunk,
            max_pending_compressed,
        }
    }

    /// Pushes an incoming WebSocket binary frame fragment into the stream.
    ///
    /// - If the accumulated buffer does not yet end with [`ZLIB_SYNC_FLUSH_SUFFIX`]
    ///   (`[0x00, 0x00, 0xff, 0xff]`), returns `Ok(Vec::new())` while waiting for remaining fragments.
    /// - Once the `Z_SYNC_FLUSH` suffix arrives, inflates and returns the complete Gateway payload
    ///   while retaining the sliding-window dictionary for subsequent frames.
    pub fn push(&mut self, frame: &[u8]) -> Result<Vec<u8>, ZlibError> {
        if self.dead {
            return Err(ZlibError::Poisoned);
        }

        let new_pending = self.pending.len().saturating_add(frame.len());
        if new_pending > self.max_pending_compressed {
            self.dead = true;
            self.pending.clear();
            return Err(ZlibError::PendingOverflow {
                pending: new_pending,
                limit: self.max_pending_compressed,
            });
        }
        self.pending.extend_from_slice(frame);

        if !self.pending.ends_with(&ZLIB_SYNC_FLUSH_SUFFIX) {
            return Ok(Vec::new());
        }

        if !self.header_checked {
            if self.pending.len() < 2 {
                return Ok(Vec::new());
            }
            let cmf = self.pending[0];
            let flg = self.pending[1];
            if !is_valid_rfc1950_zlib_header(cmf, flg) {
                self.dead = true;
                self.pending.clear();
                return Err(ZlibError::InvalidHeader { cmf, flg });
            }
            self.header_checked = true;
        }

        self.inflate_pending()
    }

    fn inflate_pending(&mut self) -> Result<Vec<u8>, ZlibError> {
        let mut out = Vec::new();
        let mut offset = 0usize;

        while offset < self.pending.len() || !out.is_empty() {
            let (status, taken, produced) = self.inflate_step(offset, &mut out)?;
            offset += taken;
            if status == Status::StreamEnd || (taken == 0 && produced == 0) {
                break;
            }
        }

        if offset < self.pending.len() {
            return Err(self.fail_decode("unconsumed bytes after Z_SYNC_FLUSH boundary"));
        }

        self.pending.clear();
        self.decompressed_bytes = self.decompressed_bytes.saturating_add(out.len() as u64);
        self.messages_decompressed = self.messages_decompressed.saturating_add(1);
        Ok(out)
    }

    fn inflate_step(
        &mut self,
        offset: usize,
        out: &mut Vec<u8>,
    ) -> Result<(Status, usize, usize), ZlibError> {
        let before_in = self.decompressor.total_in();
        let before_out = self.decompressor.total_out();
        let status = self
            .decompressor
            .decompress(&self.pending[offset..], &mut self.target, FlushDecompress::Sync)
            .map_err(|err| self.fail_decode(&err.to_string()))?;

        let taken = (self.decompressor.total_in() - before_in) as usize;
        let produced = (self.decompressor.total_out() - before_out) as usize;
        self.compressed_bytes = self.compressed_bytes.saturating_add(taken as u64);

        if produced > 0 {
            let next_len = out.len().saturating_add(produced);
            if next_len > self.max_decompressed_chunk {
                self.dead = true;
                self.pending.clear();
                return Err(ZlibError::ChunkTooLarge {
                    produced: next_len,
                    limit: self.max_decompressed_chunk,
                });
            }
            out.extend_from_slice(&self.target[..produced]);
        }
        Ok((status, taken, produced))
    }

    fn fail_decode(&mut self, msg: &str) -> ZlibError {
        self.dead = true;
        self.pending.clear();
        ZlibError::Decode {
            msg: msg.to_string(),
        }
    }

    /// Resets the decompressor state for a new Gateway WebSocket session without reallocating buffers.
    pub fn reset(&mut self) {
        self.decompressor.reset(true);
        self.pending.clear();
        self.header_checked = false;
        self.dead = false;
        self.compressed_bytes = 0;
        self.decompressed_bytes = 0;
        self.messages_decompressed = 0;
    }

    /// Returns current compression and traffic-savings statistics.
    pub fn stats(&self) -> CompressionStats {
        CompressionStats {
            compressed_bytes: self.compressed_bytes,
            decompressed_bytes: self.decompressed_bytes,
            messages_decompressed: self.messages_decompressed,
        }
    }

    /// Total compressed wire bytes consumed by the decompressor.
    pub fn compressed_consumed(&self) -> u64 {
        self.compressed_bytes
    }

    /// Total decompressed payload bytes emitted so far.
    pub fn decompressed_produced(&self) -> u64 {
        self.decompressed_bytes
    }

    /// Total number of complete Gateway messages decompressed.
    pub fn messages_decompressed(&self) -> u64 {
        self.messages_decompressed
    }

    /// Number of compressed fragment bytes currently buffered awaiting `Z_SYNC_FLUSH`.
    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }

    /// Whether the stream is terminally poisoned after an error.
    pub fn is_dead(&self) -> bool {
        self.dead
    }
}

impl Default for ZlibStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported Discord Gateway transport compression modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GatewayCompression {
    /// `?compress=zlib-stream` (RFC 1950 zlib stream with `00 00 ff ff` sync-flush boundaries).
    #[default]
    ZlibStream,
    /// `?compress=zstd-stream` (continuous Zstandard stream).
    ZstdStream,
    /// Uncompressed Gateway frames.
    None,
}

impl GatewayCompression {
    /// Returns the Discord Gateway URL `compress` query parameter value, if applicable.
    pub const fn query_param(self) -> Option<&'static str> {
        match self {
            Self::ZlibStream => Some("zlib-stream"),
            Self::ZstdStream => Some("zstd-stream"),
            Self::None => None,
        }
    }
}

/// Unified Gateway payload decompressor supporting `zlib-stream`, `zstd-stream`, and uncompressed frames.
pub enum GatewayDecompressor {
    /// Streaming `flate2` / `zlib-rs` decompressor (`STAGE-05.1.2`).
    Zlib(ZlibStream),
    /// Streaming `zstd` decompressor.
    Zstd(ZstdStream),
    /// Pass-through uncompressed mode.
    None,
}

impl GatewayDecompressor {
    /// Creates a new decompressor for the specified [`GatewayCompression`] mode.
    pub fn new(mode: GatewayCompression) -> Self {
        match mode {
            GatewayCompression::ZlibStream => Self::Zlib(ZlibStream::new()),
            GatewayCompression::ZstdStream => Self::Zstd(ZstdStream::new()),
            GatewayCompression::None => Self::None,
        }
    }

    /// Returns the active compression mode.
    pub const fn compression(&self) -> GatewayCompression {
        match self {
            Self::Zlib(_) => GatewayCompression::ZlibStream,
            Self::Zstd(_) => GatewayCompression::ZstdStream,
            Self::None => GatewayCompression::None,
        }
    }

    /// Pushes an incoming binary WebSocket frame and returns any decompressed payload bytes.
    pub fn push(&mut self, frame: &[u8]) -> Result<Vec<u8>, ZlibError> {
        match self {
            Self::Zlib(stream) => stream.push(frame),
            Self::Zstd(stream) => stream.push(frame).map_err(ZlibError::Zstd),
            Self::None => Ok(frame.to_vec()),
        }
    }
}

/// Compresses a sequence of Gateway payloads into a shared-context `zlib-stream` wire chunks vector
/// (each chunk ending with [`ZLIB_SYNC_FLUSH_SUFFIX`]).
pub fn compress_zlib_stream_chunks(
    messages: &[&[u8]],
    level: Compression,
) -> Result<Vec<Vec<u8>>, ZlibError> {
    let mut compressor = Compress::new(level, true);
    let mut chunks = Vec::with_capacity(messages.len());
    let mut scratch = vec![0u8; TARGET_CAPACITY];

    for msg in messages {
        let mut chunk = Vec::new();
        let mut offset = 0usize;
        loop {
            let before_in = compressor.total_in();
            let before_out = compressor.total_out();
            let flush = if offset < msg.len() {
                FlushCompress::None
            } else {
                FlushCompress::Sync
            };

            if let Err(err) = compressor.compress(&msg[offset..], &mut scratch, flush) {
                return Err(ZlibError::Decode {
                    msg: err.to_string(),
                });
            }

            let taken = (compressor.total_in() - before_in) as usize;
            let produced = (compressor.total_out() - before_out) as usize;
            offset += taken;
            if produced > 0 {
                chunk.extend_from_slice(&scratch[..produced]);
            }
            if offset == msg.len() && chunk.ends_with(&ZLIB_SYNC_FLUSH_SUFFIX) {
                break;
            }
        }
        chunks.push(chunk);
    }
    Ok(chunks)
}
