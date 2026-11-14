//! Contract tests for P1-2.3: VP8/VP9 Video Encoder & Keyframe/Bitrate Controller
//! in `rust/chord-gpui/src/video/encoder.rs`.

#![forbid(unsafe_code)]
#![deny(warnings)]

#[path = "../src/video/mod.rs"]
mod video;

use video::encoder::{
    DEFAULT_KEYFRAME_INTERVAL_FRAMES, MAX_ENCODER_BITRATE_BPS, MAX_ENCODER_DIMENSION,
    MAX_ENCODER_FPS, MIN_ENCODER_BITRATE_BPS, MIN_ENCODER_DIMENSION, PICTURE_ID_MODULO,
    RawVideoFrame, VP8_START_CODE, VP9_SYNC_CODE, VideoCodec, VideoEncoder, VideoEncoderConfig,
    VideoEncoderError, VideoFramePixelFormat, parse_vp8_keyframe_dimensions,
    parse_vp9_keyframe_header, verify_packet_checksum,
};
use video::sender::{DEFAULT_VIDEO_PAYLOAD_TYPE, RTP_FIXED_HEADER_LEN, RtpHeader, VideoRtpSender};

fn make_i420_frame(width: u32, height: u32, pts_us: u64, seed: u8) -> RawVideoFrame {
    let y_len = (width * height) as usize;
    let uv_len = y_len / 2;
    let mut data = Vec::with_capacity(y_len + uv_len);
    for i in 0..(y_len + uv_len) {
        data.push(seed.wrapping_add((i % 251) as u8));
    }
    RawVideoFrame::new_tight(width, height, VideoFramePixelFormat::I420, pts_us, data)
}

fn make_nv12_frame(width: u32, height: u32, pts_us: u64, seed: u8) -> RawVideoFrame {
    let total = ((width * height * 3) / 2) as usize;
    let data: Vec<u8> = (0..total)
        .map(|i| seed.wrapping_add(((i * 7) % 253) as u8))
        .collect();
    RawVideoFrame::new_tight(width, height, VideoFramePixelFormat::Nv12, pts_us, data)
}

fn make_rgba_frame(width: u32, height: u32, pts_us: u64, seed: u8) -> RawVideoFrame {
    let total = (width * height * 4) as usize;
    let data: Vec<u8> = (0..total)
        .map(|i| seed.wrapping_add(((i * 3) % 255) as u8))
        .collect();
    RawVideoFrame::new_tight(width, height, VideoFramePixelFormat::Rgba8, pts_us, data)
}

// 1. Config validation (`VideoEncoderConfig::validate`):
//    - Rejects width/height < 64 or > 3840 or odd dimensions if subsampled YUV420/NV12.
//    - Rejects fps == 0 or > 120.
//    - Rejects min_bitrate_bps > target_bitrate_bps or target_bitrate_bps > max_bitrate_bps
//      or out-of-range bitrates (< 100_000 or > 25_000_000).
#[test]
fn config_validation_enforces_dimensions_fps_and_bitrate_bounds() {
    let valid = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60);
    assert!(valid.validate().is_ok());
    assert_eq!(
        valid.keyframe_interval_frames,
        DEFAULT_KEYFRAME_INTERVAL_FRAMES
    );

    // Exact boundary dimensions: 64x64 and 3840x2160 / 3840x3840
    let min_dim = VideoEncoderConfig::new(
        VideoCodec::Vp8,
        MIN_ENCODER_DIMENSION,
        MIN_ENCODER_DIMENSION,
        1,
    );
    assert!(min_dim.validate().is_ok());

    let max_dim = VideoEncoderConfig::new(
        VideoCodec::Vp9,
        MAX_ENCODER_DIMENSION,
        MAX_ENCODER_DIMENSION,
        MAX_ENCODER_FPS,
    );
    assert!(max_dim.validate().is_ok());

    // Below minimum dimension (< 64)
    let too_narrow = VideoEncoderConfig::new(VideoCodec::Vp8, 63, 720, 60);
    assert_eq!(
        too_narrow.validate().unwrap_err(),
        VideoEncoderError::InvalidDimensions {
            width: 63,
            height: 720
        }
    );
    let too_short = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 63, 60);
    assert_eq!(
        too_short.validate().unwrap_err(),
        VideoEncoderError::InvalidDimensions {
            width: 1280,
            height: 63
        }
    );

    // Above maximum dimension (> 3840)
    let too_wide = VideoEncoderConfig::new(VideoCodec::Vp8, 3841, 2160, 60);
    assert_eq!(
        too_wide.validate().unwrap_err(),
        VideoEncoderError::InvalidDimensions {
            width: 3841,
            height: 2160
        }
    );
    let too_tall = VideoEncoderConfig::new(VideoCodec::Vp8, 1920, 3842, 60);
    assert_eq!(
        too_tall.validate().unwrap_err(),
        VideoEncoderError::InvalidDimensions {
            width: 1920,
            height: 3842
        }
    );

    // Odd dimensions rejected for subsampled I420 and NV12, allowed for RGBA8
    let odd_i420 = VideoEncoderConfig::new(VideoCodec::Vp8, 1281, 720, 60)
        .with_pixel_format(VideoFramePixelFormat::I420);
    assert_eq!(
        odd_i420.validate().unwrap_err(),
        VideoEncoderError::OddSubsampledDimensions {
            width: 1281,
            height: 720,
            format: VideoFramePixelFormat::I420,
        }
    );

    let odd_nv12 = VideoEncoderConfig::new(VideoCodec::Vp9, 1280, 721, 60)
        .with_pixel_format(VideoFramePixelFormat::Nv12);
    assert_eq!(
        odd_nv12.validate().unwrap_err(),
        VideoEncoderError::OddSubsampledDimensions {
            width: 1280,
            height: 721,
            format: VideoFramePixelFormat::Nv12,
        }
    );

    let odd_rgba = VideoEncoderConfig::new(VideoCodec::Vp8, 1281, 721, 60)
        .with_pixel_format(VideoFramePixelFormat::Rgba8);
    assert!(odd_rgba.validate().is_ok());

    // FPS == 0 or > 120 rejected
    let zero_fps = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 0);
    assert_eq!(
        zero_fps.validate().unwrap_err(),
        VideoEncoderError::InvalidFps(0)
    );
    let high_fps = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 121);
    assert_eq!(
        high_fps.validate().unwrap_err(),
        VideoEncoderError::InvalidFps(121)
    );

    // Bitrate ordering and range bounds
    let exact_bitrate_bounds = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_bitrates(
            MIN_ENCODER_BITRATE_BPS,
            MIN_ENCODER_BITRATE_BPS,
            MAX_ENCODER_BITRATE_BPS,
        );
    assert!(exact_bitrate_bounds.validate().is_ok());

    let below_min_bitrate = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_bitrates(99_999, 500_000, 2_000_000);
    assert_eq!(
        below_min_bitrate.validate().unwrap_err(),
        VideoEncoderError::BitrateOutOfRange {
            min_bps: 99_999,
            target_bps: 500_000,
            max_bps: 2_000_000,
        }
    );

    let above_max_bitrate = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_bitrates(500_000, 2_000_000, 25_000_001);
    assert_eq!(
        above_max_bitrate.validate().unwrap_err(),
        VideoEncoderError::BitrateOutOfRange {
            min_bps: 500_000,
            target_bps: 2_000_000,
            max_bps: 25_000_001,
        }
    );

    let inverted_min_target = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_bitrates(2_000_000, 1_500_000, 4_000_000);
    assert_eq!(
        inverted_min_target.validate().unwrap_err(),
        VideoEncoderError::InvalidBitrateOrder {
            min_bps: 2_000_000,
            target_bps: 1_500_000,
            max_bps: 4_000_000,
        }
    );

    let inverted_target_max = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_bitrates(500_000, 5_000_000, 4_000_000);
    assert_eq!(
        inverted_target_max.validate().unwrap_err(),
        VideoEncoderError::InvalidBitrateOrder {
            min_bps: 500_000,
            target_bps: 5_000_000,
            max_bps: 4_000_000,
        }
    );

    // Keyframe interval == 0 rejected
    let zero_kf_interval =
        VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60).with_keyframe_interval(0);
    assert_eq!(
        zero_kf_interval.validate().unwrap_err(),
        VideoEncoderError::InvalidKeyframeInterval(0)
    );
}

// 2. Frame validation (`RawVideoFrame` I420 / NV12 / RGBA8):
//    - Rejects empty buffer, mismatched width/height, undersized stride,
//      or truncated frame buffer (< expected_bytes).
#[test]
fn frame_validation_rejects_empty_mismatched_undersized_stride_and_truncated_buffers() {
    let config = VideoEncoderConfig::new(VideoCodec::Vp8, 128, 128, 30)
        .with_pixel_format(VideoFramePixelFormat::I420);
    let mut encoder = VideoEncoder::new(config).expect("valid config");

    // Empty buffer
    let empty_frame = RawVideoFrame::new(128, 128, 128, VideoFramePixelFormat::I420, 0, Vec::new());
    assert_eq!(
        empty_frame.validate().unwrap_err(),
        VideoEncoderError::EmptyFrameBuffer
    );
    assert_eq!(
        encoder.encode_frame(&empty_frame).unwrap_err(),
        VideoEncoderError::EmptyFrameBuffer
    );

    // Mismatched width / height against encoder config
    let wrong_dims = make_i420_frame(256, 128, 0, 0x10);
    assert_eq!(
        encoder.encode_frame(&wrong_dims).unwrap_err(),
        VideoEncoderError::FrameDimensionMismatch {
            expected_width: 128,
            expected_height: 128,
            actual_width: 256,
            actual_height: 128,
        }
    );

    // Undersized stride for I420 (< width)
    let undersized_stride_i420 = RawVideoFrame::new(
        128,
        128,
        120,
        VideoFramePixelFormat::I420,
        0,
        vec![0x80; (128 * 128 * 3) / 2],
    );
    assert_eq!(
        encoder.encode_frame(&undersized_stride_i420).unwrap_err(),
        VideoEncoderError::InvalidStride {
            stride_bytes: 120,
            minimum_stride_bytes: 128,
        }
    );

    // Undersized stride for RGBA8 (< width * 4)
    let rgba_bad_stride = RawVideoFrame::new(
        128,
        128,
        128 * 4 - 4,
        VideoFramePixelFormat::Rgba8,
        0,
        vec![0xFF; 128 * 128 * 4],
    );
    assert_eq!(
        rgba_bad_stride.validate().unwrap_err(),
        VideoEncoderError::InvalidStride {
            stride_bytes: 508,
            minimum_stride_bytes: 512,
        }
    );

    // Truncated buffer for I420 (< expected_bytes = 24_576)
    let expected_i420 = (128 * 128 * 3) / 2;
    let truncated_i420 = RawVideoFrame::new(
        128,
        128,
        128,
        VideoFramePixelFormat::I420,
        0,
        vec![0x42; expected_i420 - 1],
    );
    assert_eq!(
        encoder.encode_frame(&truncated_i420).unwrap_err(),
        VideoEncoderError::TruncatedFrameBuffer {
            actual_bytes: expected_i420 - 1,
            expected_bytes: expected_i420,
        }
    );

    // Truncated buffer for NV12 and RGBA8
    let expected_nv12 = (128 * 128 * 3) / 2;
    let truncated_nv12 = RawVideoFrame::new(
        128,
        128,
        128,
        VideoFramePixelFormat::Nv12,
        0,
        vec![0x11; expected_nv12 - 5],
    );
    assert_eq!(
        truncated_nv12.validate().unwrap_err(),
        VideoEncoderError::TruncatedFrameBuffer {
            actual_bytes: expected_nv12 - 5,
            expected_bytes: expected_nv12,
        }
    );

    let expected_rgba = 128 * 128 * 4;
    let truncated_rgba = RawVideoFrame::new(
        128,
        128,
        128 * 4,
        VideoFramePixelFormat::Rgba8,
        0,
        vec![0x22; expected_rgba - 1],
    );
    assert_eq!(
        truncated_rgba.validate().unwrap_err(),
        VideoEncoderError::TruncatedFrameBuffer {
            actual_bytes: expected_rgba - 1,
            expected_bytes: expected_rgba,
        }
    );

    // Valid NV12 and RGBA8 frames encode cleanly when configured
    let mut nv12_enc = VideoEncoder::new(
        VideoEncoderConfig::new(VideoCodec::Vp8, 128, 128, 30)
            .with_pixel_format(VideoFramePixelFormat::Nv12),
    )
    .unwrap();
    let nv12_pkt = nv12_enc
        .encode_frame(&make_nv12_frame(128, 128, 0, 0x33))
        .expect("valid NV12 frame");
    assert!(nv12_pkt.is_keyframe);
    assert!(verify_packet_checksum(&nv12_pkt.payload));

    let mut rgba_enc = VideoEncoder::new(
        VideoEncoderConfig::new(VideoCodec::Vp9, 128, 128, 30)
            .with_pixel_format(VideoFramePixelFormat::Rgba8),
    )
    .unwrap();
    let rgba_pkt = rgba_enc
        .encode_frame(&make_rgba_frame(128, 128, 0, 0x77))
        .expect("valid RGBA8 frame");
    assert!(rgba_pkt.is_keyframe);
    assert!(verify_packet_checksum(&rgba_pkt.payload));
}

// 3. Keyframe generation & VP8/VP9 bitstream frame headers (`P1-2.3`):
//    - First encoded frame MUST be a keyframe (`is_keyframe == true`).
//    - VP8 keyframes contain the valid uncompressed 3-byte start code `[0x9d, 0x01, 0x2a]`
//      followed by little-endian `width` (`& 0x3FFF`) and `height` (`& 0x3FFF`) per RFC 6386 §9.1.
//    - VP9 keyframes start with valid 2-bit frame marker `0b10` (`(b0 >> 6) == 2`),
//      `key_frame == 0` (show/intra) bit, and VP9 sync code `[0x49, 0x83, 0x42]`.
//    - Subsequent frames before `keyframe_interval_frames` are inter-frames (`is_keyframe == false`),
//      then automatically emit a keyframe at the cadence boundary (`frame_index % keyframe_interval_frames == 0`).
//    - Calling `encoder.request_keyframe()` (triggered by incoming RTCP PLI / FIR feedback)
//      forces the very next encoded frame to be a keyframe regardless of cadence.
//    - Resolution change (`reconfigure(...)` or frame dimension change) resets cadence and forces a keyframe.
#[test]
fn vp8_and_vp9_keyframe_headers_cadence_pli_forcing_and_resolution_change() {
    // --- VP8 test ---
    let vp8_cfg = VideoEncoderConfig::new(VideoCodec::Vp8, 640, 360, 60).with_keyframe_interval(4);
    let mut vp8_enc = VideoEncoder::new(vp8_cfg).expect("valid VP8 config");

    // Frame 0: MUST be a keyframe with RFC 6386 §9.1 header
    let f0 = vp8_enc
        .encode_frame(&make_i420_frame(640, 360, 0, 10))
        .unwrap();
    assert!(f0.is_keyframe);
    assert_eq!(f0.frame_index, 0);
    // RFC 6386 §9.1: byte 0 bit 0 == 0 (keyframe), bit 4 == 1 (show_frame)
    assert_eq!(f0.payload[0] & 0x01, 0, "VP8 keyframe tag bit 0 must be 0");
    assert_ne!(f0.payload[0] & 0x10, 0, "VP8 show_frame bit 4 must be 1");
    assert_eq!(&f0.payload[3..6], &VP8_START_CODE);
    let raw_w = u16::from_le_bytes([f0.payload[6], f0.payload[7]]) & 0x3FFF;
    let raw_h = u16::from_le_bytes([f0.payload[8], f0.payload[9]]) & 0x3FFF;
    assert_eq!((raw_w, raw_h), (640, 360));
    assert_eq!(parse_vp8_keyframe_dimensions(&f0.payload), Some((640, 360)));
    assert!(verify_packet_checksum(&f0.payload));

    // Frames 1, 2, 3: inter-frames (is_keyframe == false)
    for idx in 1..4u64 {
        let inter = vp8_enc
            .encode_frame(&make_i420_frame(640, 360, idx * 16_667, (idx * 20) as u8))
            .unwrap();
        assert!(
            !inter.is_keyframe,
            "frame {idx} before cadence must be inter-frame"
        );
        assert_eq!(inter.frame_index, idx);
        assert_eq!(
            inter.payload[0] & 0x01,
            1,
            "VP8 inter-frame tag bit 0 must be 1"
        );
        assert_eq!(parse_vp8_keyframe_dimensions(&inter.payload), None);
        assert!(verify_packet_checksum(&inter.payload));
    }

    // Frame 4 (4 % 4 == 0): automatic cadence keyframe
    let f4 = vp8_enc
        .encode_frame(&make_i420_frame(640, 360, 4 * 16_667, 99))
        .unwrap();
    assert!(f4.is_keyframe, "frame at cadence boundary must be keyframe");
    assert_eq!(f4.frame_index, 4);
    assert_eq!(&f4.payload[3..6], &VP8_START_CODE);

    // Frame 5: inter-frame, then trigger RTCP PLI / FIR `request_keyframe()` before Frame 6
    let f5 = vp8_enc
        .encode_frame(&make_i420_frame(640, 360, 5 * 16_667, 111))
        .unwrap();
    assert!(!f5.is_keyframe);

    vp8_enc.request_keyframe();
    assert!(vp8_enc.is_keyframe_requested());

    let f6_pli = vp8_enc
        .encode_frame(&make_i420_frame(640, 360, 6 * 16_667, 122))
        .unwrap();
    assert!(
        f6_pli.is_keyframe,
        "PLI request_keyframe() must force immediate keyframe"
    );
    assert!(!vp8_enc.is_keyframe_requested());
    assert_eq!(vp8_enc.stats().forced_keyframes, 1);

    // Resolution change via `reconfigure(...)` resets cadence and forces a keyframe
    let new_vp8_cfg =
        VideoEncoderConfig::new(VideoCodec::Vp8, 320, 180, 60).with_keyframe_interval(4);
    vp8_enc.reconfigure(new_vp8_cfg).expect("valid reconfigure");
    assert_eq!(vp8_enc.stats().resolution_changes, 1);

    let f_reconf = vp8_enc
        .encode_frame(&make_i420_frame(320, 180, 7 * 16_667, 130))
        .unwrap();
    assert!(f_reconf.is_keyframe, "reconfigure must force keyframe");
    assert_eq!(
        parse_vp8_keyframe_dimensions(&f_reconf.payload),
        Some((320, 180))
    );

    // Next 3 frames after reconfigure (cadence 1, 2, 3) must be inter-frames, then cadence 4 is keyframe
    for step in 1..4u64 {
        let pkt = vp8_enc
            .encode_frame(&make_i420_frame(320, 180, (7 + step) * 16_667, 140))
            .unwrap();
        assert!(
            !pkt.is_keyframe,
            "step {step} after reconfigure should be inter-frame"
        );
    }
    let f_reconf_cadence = vp8_enc
        .encode_frame(&make_i420_frame(320, 180, 11 * 16_667, 150))
        .unwrap();
    assert!(
        f_reconf_cadence.is_keyframe,
        "4th frame after reconfigure must hit reset cadence boundary"
    );

    // Resolution change via dynamic frame dimension change (`encode_frame_auto_reconfigure`)
    let f_dyn_res = vp8_enc
        .encode_frame_auto_reconfigure(&make_i420_frame(256, 144, 12 * 16_667, 160))
        .expect("dynamic resolution change should succeed");
    assert!(f_dyn_res.is_keyframe);
    assert_eq!((f_dyn_res.width, f_dyn_res.height), (256, 144));
    assert_eq!(
        parse_vp8_keyframe_dimensions(&f_dyn_res.payload),
        Some((256, 144))
    );
    assert_eq!(vp8_enc.stats().resolution_changes, 2);

    // --- VP9 test ---
    let vp9_cfg = VideoEncoderConfig::new(VideoCodec::Vp9, 640, 360, 60).with_keyframe_interval(3);
    let mut vp9_enc = VideoEncoder::new(vp9_cfg).expect("valid VP9 config");

    let vp9_f0 = vp9_enc
        .encode_frame(&make_i420_frame(640, 360, 0, 50))
        .unwrap();
    assert!(vp9_f0.is_keyframe);
    let b0 = vp9_f0.payload[0];
    // 2-bit frame marker `0b10` (`(b0 >> 6) == 2`)
    assert_eq!(b0 >> 6, 0b10, "VP9 frame_marker must be 0b10");
    // `key_frame == 0` (show/intra) bit
    let key_frame_bit = (b0 >> 2) & 0x01;
    assert_eq!(
        key_frame_bit, 0,
        "VP9 key_frame bit must be 0 for intra frame"
    );
    // VP9 sync code `[0x49, 0x83, 0x42]`
    assert_eq!(&vp9_f0.payload[1..4], &VP9_SYNC_CODE);
    assert_eq!(parse_vp9_keyframe_header(&vp9_f0.payload), Some((640, 360)));
    assert!(verify_packet_checksum(&vp9_f0.payload));

    // VP9 inter-frame has frame_marker 0b10 and key_frame bit == 1
    let vp9_f1 = vp9_enc
        .encode_frame(&make_i420_frame(640, 360, 16_667, 60))
        .unwrap();
    assert!(!vp9_f1.is_keyframe);
    assert_eq!(vp9_f1.payload[0] >> 6, 0b10);
    assert_eq!((vp9_f1.payload[0] >> 2) & 0x01, 1);
    assert_eq!(parse_vp9_keyframe_header(&vp9_f1.payload), None);
}

// 4. RTP 90kHz timestamp & monotonic 15-bit `picture_id`:
//    - `timestamp_90khz` advances accurately from `pts_us` (`(pts_us * 90) / 1000` mod `2^32`,
//      wrapping safely at `u32::MAX`).
//    - `picture_id` increments monotonically modulo `0x8000` (15-bit).
#[test]
fn rtp_90khz_timestamp_wrapping_and_15bit_picture_id_monotonicity() {
    let cfg = VideoEncoderConfig::new(VideoCodec::Vp8, 64, 64, 60);
    let mut encoder = VideoEncoder::new(cfg)
        .unwrap()
        .with_initial_picture_id(0x7FFE);

    // 1) pts_us = 0 -> timestamp_90khz = 0, picture_id = 0x7FFE (32766)
    let p0 = encoder
        .encode_frame(&make_i420_frame(64, 64, 0, 1))
        .unwrap();
    assert_eq!(p0.timestamp_90khz, 0);
    assert_eq!(p0.picture_id, 0x7FFE);

    // 2) pts_us = 1_000_000 (1s) -> timestamp_90khz = 90_000, picture_id = 0x7FFF (32767)
    let p1 = encoder
        .encode_frame(&make_i420_frame(64, 64, 1_000_000, 2))
        .unwrap();
    assert_eq!(p1.timestamp_90khz, 90_000);
    assert_eq!(p1.picture_id, 0x7FFF);

    // 3) Wrap picture_id modulo 0x8000 -> 0, and test u32::MAX timestamp wraparound
    // At 90kHz: u32::MAX ticks = 4_294_967_295 ticks -> pts_us = 47_721_858_834 us -> 4_294_967_295 ticks
    // Next tick: pts_us = 47_721_858_845 us -> (47_721_858_845 * 90) / 1000 = 4_294_967_296 = 0 (mod 2^32)
    let pts_at_u32_max = 47_721_858_834u64;
    let p2 = encoder
        .encode_frame(&make_i420_frame(64, 64, pts_at_u32_max, 3))
        .unwrap();
    assert_eq!(p2.picture_id, 0);
    assert_eq!(p2.timestamp_90khz, u32::MAX);

    let pts_wrapped = 47_721_858_845u64;
    let p3 = encoder
        .encode_frame(&make_i420_frame(64, 64, pts_wrapped, 4))
        .unwrap();
    assert_eq!(p3.picture_id, 1);
    assert_eq!(p3.picture_id % PICTURE_ID_MODULO, 1);
    assert_eq!(p3.timestamp_90khz, 0);
}

// 5. Bitrate adaptation (`on_bandwidth_estimate(&mut self, estimated_bps: u32)`):
//    - Clamps updated `target_bitrate_bps` to `[min_bitrate_bps, max_bitrate_bps]`,
//      updates `VideoEncoderStats`, and respects max frame byte budget proportional to `target_bitrate / fps`.
#[test]
fn bitrate_adaptation_clamps_to_bounds_and_scales_frame_byte_budget() {
    let cfg = VideoEncoderConfig::new(VideoCodec::Vp8, 640, 360, 60)
        .with_bitrates(300_000, 2_400_000, 6_000_000);
    let mut encoder = VideoEncoder::new(cfg).unwrap();

    // Initial budget at 2.4 Mbps / 60 fps = 5,000 bytes
    assert_eq!(encoder.target_bitrate_bps(), 2_400_000);
    assert_eq!(encoder.frame_byte_budget(), 2_400_000 / (8 * 60));

    let pkt_initial = encoder
        .encode_frame(&make_i420_frame(640, 360, 0, 42))
        .unwrap();
    assert!(pkt_initial.payload.len() <= pkt_initial.byte_budget);
    assert_eq!(pkt_initial.byte_budget, 5_000);

    // Clamp below min_bitrate_bps (100_000 -> clamped to 300_000)
    let clamped_low = encoder.on_bandwidth_estimate(100_000);
    assert_eq!(clamped_low, 300_000);
    assert_eq!(encoder.target_bitrate_bps(), 300_000);
    assert_eq!(encoder.stats().current_target_bitrate_bps, 300_000);
    assert_eq!(encoder.stats().bandwidth_updates, 1);
    let low_budget = encoder.frame_byte_budget();
    assert_eq!(low_budget, 300_000 / (8 * 60)); // 625 bytes

    encoder.request_keyframe();
    let pkt_low = encoder
        .encode_frame(&make_i420_frame(640, 360, 16_667, 42))
        .unwrap();
    assert!(
        pkt_low.payload.len() <= low_budget,
        "encoded keyframe {} must respect low bitrate budget {}",
        pkt_low.payload.len(),
        low_budget
    );

    // Clamp above max_bitrate_bps (20_000_000 -> clamped to 6_000_000)
    let clamped_high = encoder.on_bandwidth_estimate(20_000_000);
    assert_eq!(clamped_high, 6_000_000);
    assert_eq!(encoder.target_bitrate_bps(), 6_000_000);
    assert_eq!(encoder.stats().current_target_bitrate_bps, 6_000_000);
    assert_eq!(encoder.stats().bandwidth_updates, 2);
    let high_budget = encoder.frame_byte_budget();
    assert_eq!(high_budget, 6_000_000 / (8 * 60)); // 12,500 bytes

    encoder.request_keyframe();
    let pkt_high = encoder
        .encode_frame(&make_i420_frame(640, 360, 33_334, 42))
        .unwrap();
    assert!(pkt_high.payload.len() <= high_budget);
    assert!(
        pkt_high.payload.len() > pkt_low.payload.len(),
        "higher bitrate keyframe ({}) should carry more detail than low bitrate keyframe ({})",
        pkt_high.payload.len(),
        pkt_low.payload.len()
    );
    assert!(
        pkt_high.quantizer_index < pkt_low.quantizer_index,
        "higher bitrate quantizer ({}) should be finer than low bitrate quantizer ({})",
        pkt_high.quantizer_index,
        pkt_low.quantizer_index
    );
}

// 6. End-to-end integration with existing `video::sender::VideoRtpSender`:
//    - Output `EncodedVideoPacket.payload` + `timestamp_90khz` feeds directly into
//      `VideoRtpSender::packetize_frame` with valid RFC 3550 marker bit on the final RTP fragment.
#[test]
fn end_to_end_encoder_into_video_rtp_sender_fragments_and_sets_rfc3550_marker_bit() {
    let cfg = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_bitrates(500_000, 4_800_000, 8_000_000);
    let mut encoder = VideoEncoder::new(cfg).unwrap();
    let mut sender = VideoRtpSender::new(0xABCD_1234, DEFAULT_VIDEO_PAYLOAD_TYPE).unwrap();

    // Encode a 720p60 keyframe at 4.8 Mbps (budget = 10,000 bytes -> multiple 1200-byte RTP packets)
    let raw_frame = make_i420_frame(1280, 720, 200_000, 0x5A);
    let encoded = encoder
        .encode_frame(&raw_frame)
        .expect("encode 720p keyframe");
    assert!(encoded.is_keyframe);
    assert_eq!(encoded.timestamp_90khz, 18_000);
    assert!(encoded.payload.len() > sender.mtu_payload_bytes());

    let rtp_packets = sender
        .packetize_frame(&encoded.payload, encoded.timestamp_90khz)
        .expect("packetize encoded VP8 keyframe");
    assert!(rtp_packets.len() >= 2);

    let mut reassembled = Vec::with_capacity(encoded.payload.len());
    for (idx, pkt) in rtp_packets.iter().enumerate() {
        let hdr = RtpHeader::decode(pkt).expect("valid RFC 3550 header");
        let is_last = idx + 1 == rtp_packets.len();
        assert_eq!(
            hdr.marker, is_last,
            "RFC 3550 marker bit must be set ONLY on final fragment"
        );
        assert_eq!(hdr.timestamp, encoded.timestamp_90khz);
        assert_eq!(hdr.ssrc, 0xABCD_1234);
        assert_eq!(hdr.payload_type, DEFAULT_VIDEO_PAYLOAD_TYPE);
        reassembled.extend_from_slice(&pkt[RTP_FIXED_HEADER_LEN..]);
    }

    assert_eq!(reassembled, encoded.payload);
    assert_eq!(
        parse_vp8_keyframe_dimensions(&reassembled),
        Some((1280, 720))
    );
    assert!(verify_packet_checksum(&reassembled));
}

// 7. Pixel format fidelity, padded strides, CRC32 tamper detection, and telemetry completeness.
#[test]
fn pixel_formats_padded_stride_crc32_tamper_and_telemetry_completeness() {
    assert_eq!(VideoCodec::default(), VideoCodec::Vp8);
    assert_eq!(VideoCodec::Vp8.as_str(), "VP8");
    assert_eq!(VideoCodec::Vp9.as_str(), "VP9");
    assert_eq!(
        VideoFramePixelFormat::default(),
        VideoFramePixelFormat::I420
    );

    // PixelFormatMismatch rejected in both encode_frame and encode_frame_auto_reconfigure
    let mut enc_i420 =
        VideoEncoder::new(VideoEncoderConfig::new(VideoCodec::Vp8, 128, 128, 30)).unwrap();
    let rgba_frame = make_rgba_frame(128, 128, 0, 10);
    assert_eq!(
        enc_i420.encode_frame(&rgba_frame).unwrap_err(),
        VideoEncoderError::PixelFormatMismatch {
            expected: VideoFramePixelFormat::I420,
            actual: VideoFramePixelFormat::Rgba8,
        }
    );
    assert_eq!(
        enc_i420
            .encode_frame_auto_reconfigure(&rgba_frame)
            .unwrap_err(),
        VideoEncoderError::PixelFormatMismatch {
            expected: VideoFramePixelFormat::I420,
            actual: VideoFramePixelFormat::Rgba8,
        }
    );

    // Height-only mismatch rejected in encode_frame
    let wrong_height = make_i420_frame(128, 256, 0, 10);
    assert_eq!(
        enc_i420.encode_frame(&wrong_height).unwrap_err(),
        VideoEncoderError::FrameDimensionMismatch {
            expected_width: 128,
            expected_height: 128,
            actual_width: 128,
            actual_height: 256,
        }
    );

    // Padded stride (> min_stride) accepted when buffer is sized accordingly
    let padded_stride = 160u32;
    let padded_len = VideoFramePixelFormat::I420.expected_buffer_bytes(padded_stride, 128);
    let padded_frame = RawVideoFrame::new(
        128,
        128,
        padded_stride,
        VideoFramePixelFormat::I420,
        10_000,
        vec![0x90; padded_len],
    );
    let pkt_dark = enc_i420
        .encode_frame(&padded_frame)
        .expect("padded stride valid");
    assert!(verify_packet_checksum(&pkt_dark.payload));

    // Verify RFC 6386 §9.1 19-bit first_part_size in 3-byte VP8 frame tag
    let tag_u24 = u32::from(pkt_dark.payload[0])
        | (u32::from(pkt_dark.payload[1]) << 8)
        | (u32::from(pkt_dark.payload[2]) << 16);
    assert_eq!((tag_u24 >> 5) as usize, pkt_dark.payload.len() - 10);

    // Different pixel content produces different quantized payload and checksum
    enc_i420.request_keyframe();
    let bright_frame = RawVideoFrame::new(
        128,
        128,
        padded_stride,
        VideoFramePixelFormat::I420,
        20_000,
        vec![0x10; padded_len],
    );
    let pkt_bright = enc_i420.encode_frame(&bright_frame).unwrap();
    assert_ne!(pkt_dark.checksum_crc32, pkt_bright.checksum_crc32);
    assert_ne!(pkt_dark.payload, pkt_bright.payload);

    // Tampering with any byte in payload invalidates CRC32 checksum
    let mut tampered = pkt_bright.payload.clone();
    tampered[12] ^= 0xFF;
    assert!(!verify_packet_checksum(&tampered));
    assert!(!verify_packet_checksum(&[0u8; 10]));

    // Header parser negative checks (truncated / bad start or sync codes / bad show_frame bit)
    assert_eq!(parse_vp8_keyframe_dimensions(&[0u8; 5]), None);
    let mut bad_vp8 = pkt_dark.payload.clone();
    bad_vp8[0] &= !0x10; // clear show_frame bit
    assert_eq!(parse_vp8_keyframe_dimensions(&bad_vp8), None);
    let mut bad_vp8_sync = pkt_dark.payload.clone();
    bad_vp8_sync[4] = 0x00;
    assert_eq!(parse_vp8_keyframe_dimensions(&bad_vp8_sync), None);

    assert_eq!(parse_vp9_keyframe_header(&[0u8; 5]), None);
    assert_eq!(parse_vp9_keyframe_header(&[0x00; 12]), None);
    assert_eq!(
        parse_vp9_keyframe_header(&[0x81, 0x49, 0x83, 0x42, 0, 0, 64, 0, 64]),
        None
    );
    assert_eq!(
        parse_vp9_keyframe_header(&[0x83, 0x49, 0x00, 0x42, 0, 0, 64, 0, 64]),
        None
    );

    // Telemetry counters accuracy after 2 keyframes + 1 inter-frame
    let inter_pkt = enc_i420
        .encode_frame(&make_i420_frame(128, 128, 30_000, 55))
        .unwrap();
    let stats = enc_i420.stats();
    assert_eq!(stats.frames_encoded, 3);
    assert_eq!(stats.keyframes_encoded, 2);
    assert_eq!(stats.inter_frames_encoded, 1);
    assert_eq!(stats.forced_keyframes, 1);
    assert_eq!(
        stats.bytes_encoded,
        (pkt_dark.payload.len() + pkt_bright.payload.len() + inter_pkt.payload.len()) as u64
    );
    assert_eq!(stats.last_frame_bytes, inter_pkt.payload.len());
    assert_eq!(stats.last_picture_id, inter_pkt.picture_id);
    assert_eq!(stats.last_timestamp_90khz, inter_pkt.timestamp_90khz);

    // All error Display messages are non-empty
    let errors = [
        VideoEncoderError::InvalidDimensions {
            width: 10,
            height: 10,
        },
        VideoEncoderError::OddSubsampledDimensions {
            width: 65,
            height: 64,
            format: VideoFramePixelFormat::I420,
        },
        VideoEncoderError::InvalidFps(0),
        VideoEncoderError::BitrateOutOfRange {
            min_bps: 1,
            target_bps: 2,
            max_bps: 3,
        },
        VideoEncoderError::InvalidBitrateOrder {
            min_bps: 300_000,
            target_bps: 200_000,
            max_bps: 400_000,
        },
        VideoEncoderError::InvalidKeyframeInterval(0),
        VideoEncoderError::EmptyFrameBuffer,
        VideoEncoderError::FrameDimensionMismatch {
            expected_width: 64,
            expected_height: 64,
            actual_width: 128,
            actual_height: 64,
        },
        VideoEncoderError::PixelFormatMismatch {
            expected: VideoFramePixelFormat::I420,
            actual: VideoFramePixelFormat::Nv12,
        },
        VideoEncoderError::InvalidStride {
            stride_bytes: 32,
            minimum_stride_bytes: 64,
        },
        VideoEncoderError::TruncatedFrameBuffer {
            actual_bytes: 10,
            expected_bytes: 100,
        },
    ];
    for err in errors {
        assert!(!err.to_string().is_empty());
    }
}
