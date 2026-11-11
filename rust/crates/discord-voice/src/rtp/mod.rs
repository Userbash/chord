//! Пакеты и пакетизаторы RTP для голосовых и видеопотоков Discord (`discord-voice::rtp`).

pub mod video;

pub use video::{
    AV1_AGGR_N_NEW_CODED_VIDEO_SEQ, AV1_AGGR_Y_FRAGMENTED, AV1_AGGR_Z_CONTINUATION, AV1_OBU_FRAME,
    AV1_OBU_FRAME_HEADER, AV1_OBU_METADATA, AV1_OBU_SEQUENCE_HEADER, AV1_OBU_TEMPORAL_DELIMITER,
    AV1_OBU_TILE_GROUP, DEFAULT_720P60_BITRATE_BPS, DEFAULT_VIDEO_FPS, GW_OP_STREAM_CREATE,
    GW_OP_STREAM_DELETE, GW_OP_STREAM_PING, GW_OP_STREAM_SET_PAUSED, GW_OP_STREAM_WATCH,
    H264_FU_HEADER_END, H264_FU_HEADER_START, H264_NAL_AUD, H264_NAL_F_BIT, H264_NAL_FU_A,
    H264_NAL_IDR, H264_NAL_NRI_MASK, H264_NAL_PPS, H264_NAL_SEI, H264_NAL_SPS, H264_NAL_STAP_A,
    H264_NAL_TYPE_MASK, H264PacketizationKind, MAX_GO_LIVE_VIDEO_BITRATE_BPS,
    MAX_VIDEO_RTP_PAYLOAD_LEN, MIN_VIDEO_BITRATE_BPS, RTP_AV1_PAYLOAD_TYPE,
    RTP_AV1_RTX_PAYLOAD_TYPE, RTP_H264_PAYLOAD_TYPE, RTP_H264_RTX_PAYLOAD_TYPE,
    RTP_H265_PAYLOAD_TYPE, RTP_H265_RTX_PAYLOAD_TYPE, RTP_VP8_PAYLOAD_TYPE,
    RTP_VP8_RTX_PAYLOAD_TYPE, RtpVideoPacketizer, VIDEO_CLOCK_RATE, VIDEO_TICKS_30FPS,
    VIDEO_TICKS_60FPS, VideoRtpError, VideoRtpPacket, build_stream_create_call,
    build_stream_create_guild, build_stream_delete, build_stream_ping, build_stream_set_paused,
    build_stream_watch, clamp_video_bitrate_bps, encode_h264_fu_a, encode_h264_stap_a,
    packetize_av1_bitstream_with_mtu, packetize_av1_obus_with_mtu, packetize_h264_annex_b_with_mtu,
    parse_av1_obu_stream, parse_h264_annex_b, video_ticks_for_elapsed_ms, video_ticks_for_fps,
};
