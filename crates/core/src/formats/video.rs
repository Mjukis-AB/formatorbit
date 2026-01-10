//! Video file format detection and metadata extraction.
//!
//! Detects video files and extracts:
//! - Duration
//! - Resolution (width x height)
//! - Video codec
//! - Audio codec
//! - Frame rate

use std::io::{Cursor, Read, Seek};

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct VideoFormat;

/// Video metadata extracted from file.
#[derive(Debug, Clone, Default)]
struct VideoMetadata {
    format: String,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    duration_secs: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    frame_rate: Option<f64>,
    audio_sample_rate: Option<u32>,
    audio_channels: Option<u32>,
}

impl VideoFormat {
    /// Check if data is an MP4/M4V file.
    fn is_mp4(data: &[u8]) -> bool {
        // Check for ftyp box at offset 4
        if data.len() >= 12 && &data[4..8] == b"ftyp" {
            // Check for video-related brand codes
            let brand = &data[8..12];
            // Common video brands: mp41, mp42, isom, M4V, avc1, etc.
            // Exclude HEIC/AVIF image brands
            if brand == b"heic" || brand == b"avif" || brand == b"mif1" {
                return false;
            }
            return true;
        }
        false
    }

    /// Check if data is a WebM file.
    fn is_webm(data: &[u8]) -> bool {
        // EBML header magic
        if data.len() >= 4 && data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            // Check for webm doctype
            // This is a simplified check - real EBML parsing would look for DocType element
            let header_slice = &data[..data.len().min(64)];
            return header_slice
                .windows(4)
                .any(|w| w == b"webm" || w == b"matroska"[..4].as_ref());
        }
        false
    }

    /// Check if data is a Matroska (MKV) file.
    fn is_mkv(data: &[u8]) -> bool {
        // Same EBML header as WebM
        if data.len() >= 4 && data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            let header_slice = &data[..data.len().min(64)];
            return header_slice.windows(9).any(|w| w == b"matroska\0");
        }
        false
    }

    /// Detect video format from magic bytes.
    fn detect_video_format(data: &[u8]) -> Option<&'static str> {
        if Self::is_mp4(data) {
            Some("MP4")
        } else if Self::is_mkv(data) {
            Some("MKV")
        } else if Self::is_webm(data) {
            Some("WebM")
        } else {
            None
        }
    }

    /// Parse MP4 using symphonia.
    fn parse_mp4(data: &[u8]) -> Option<VideoMetadata> {
        let cursor = Cursor::new(data.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        let mut hint = Hint::new();
        hint.with_extension("mp4");

        let format_opts = FormatOptions::default();
        let meta_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &meta_opts)
            .ok()?;

        let mut meta = VideoMetadata {
            format: "MP4".to_string(),
            ..Default::default()
        };

        let format = probed.format;

        for track in format.tracks() {
            let params = &track.codec_params;

            // Check if it's a video track by looking at codec type
            let codec = params.codec;
            let codec_name = Self::codec_name(codec);

            if codec_name.contains("H.264")
                || codec_name.contains("H.265")
                || codec_name.contains("VP")
                || codec_name.contains("AV1")
            {
                meta.video_codec = Some(codec_name);

                // Video dimensions would need actual decoding
                // symphonia doesn't expose this directly for video

                // Calculate duration
                if let Some(n_frames) = params.n_frames {
                    if let Some(tb) = params.time_base {
                        let duration = n_frames as f64 * tb.numer as f64 / tb.denom as f64;
                        meta.duration_secs = Some(duration);
                    }
                }
            } else if codec_name.contains("AAC") || codec_name.contains("MP3") {
                meta.audio_codec = Some(codec_name);

                if let Some(rate) = params.sample_rate {
                    meta.audio_sample_rate = Some(rate);
                }

                if let Some(channels) = params.channels {
                    meta.audio_channels = Some(channels.count() as u32);
                }
            }
        }

        Some(meta)
    }

    /// Parse Matroska/WebM container using matroska crate.
    fn parse_matroska<R: Read + Seek>(reader: R, format_name: &str) -> Option<VideoMetadata> {
        let matroska = matroska::Matroska::open(reader).ok()?;

        let mut meta = VideoMetadata {
            format: format_name.to_string(),
            ..Default::default()
        };

        // Get duration from segment info
        if let Some(duration) = matroska.info.duration {
            meta.duration_secs = Some(duration.as_secs_f64());
        }

        // Get track info
        for track in &matroska.tracks {
            match &track.settings {
                matroska::Settings::Video(video) => {
                    meta.width = Some(video.pixel_width as u32);
                    meta.height = Some(video.pixel_height as u32);

                    // Get video codec from CodecID
                    meta.video_codec = Some(Self::matroska_codec_name(&track.codec_id));

                    // Frame rate from default duration
                    if let Some(default_duration) = track.default_duration {
                        let nanos = default_duration.as_nanos() as f64;
                        if nanos > 0.0 {
                            let fps = 1_000_000_000.0 / nanos;
                            meta.frame_rate = Some(fps);
                        }
                    }
                }
                matroska::Settings::Audio(audio) => {
                    meta.audio_codec = Some(Self::matroska_codec_name(&track.codec_id));
                    meta.audio_sample_rate = Some(audio.sample_rate as u32);
                    meta.audio_channels = Some(audio.channels as u32);
                }
                matroska::Settings::None => {}
            }
        }

        Some(meta)
    }

    /// Get human-readable codec name from matroska codec ID.
    fn matroska_codec_name(codec_id: &str) -> String {
        match codec_id {
            "V_VP8" => "VP8".to_string(),
            "V_VP9" => "VP9".to_string(),
            "V_AV1" => "AV1".to_string(),
            "V_MPEG4/ISO/AVC" => "H.264".to_string(),
            "V_MPEGH/ISO/HEVC" => "H.265".to_string(),
            "A_VORBIS" => "Vorbis".to_string(),
            "A_OPUS" => "Opus".to_string(),
            "A_AAC" => "AAC".to_string(),
            "A_FLAC" => "FLAC".to_string(),
            _ => codec_id.to_string(),
        }
    }

    /// Get human-readable codec name from symphonia codec type.
    fn codec_name(codec: symphonia::core::codecs::CodecType) -> String {
        use symphonia::core::codecs::*;

        match codec {
            CODEC_TYPE_AAC => "AAC".to_string(),
            CODEC_TYPE_MP3 => "MP3".to_string(),
            CODEC_TYPE_FLAC => "FLAC".to_string(),
            CODEC_TYPE_VORBIS => "Vorbis".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    /// Parse video file and extract metadata.
    fn parse_video(data: &[u8]) -> Option<VideoMetadata> {
        let format = Self::detect_video_format(data)?;

        match format {
            "MP4" => Self::parse_mp4(data),
            "MKV" | "WebM" => {
                let cursor = Cursor::new(data);
                Self::parse_matroska(cursor, format)
            }
            _ => None,
        }
    }

    /// Format duration as mm:ss or hh:mm:ss.
    fn format_duration(secs: f64) -> String {
        let total_secs = secs as u64;
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{}:{:02}", mins, secs)
        }
    }

    /// Format metadata into human-readable description.
    fn format_description(meta: &VideoMetadata) -> String {
        let mut parts = vec![meta.format.clone()];

        if let (Some(w), Some(h)) = (meta.width, meta.height) {
            parts.push(format!("{}x{}", w, h));
        }

        if let Some(ref codec) = meta.video_codec {
            parts.push(codec.clone());
        }

        if let Some(duration) = meta.duration_secs {
            parts.push(Self::format_duration(duration));
        }

        parts.join(", ")
    }

    /// Build RichDisplay options for UI rendering.
    fn build_rich_display(meta: &VideoMetadata) -> Vec<RichDisplayOption> {
        let mut displays = vec![];
        let mut pairs = vec![];

        pairs.push(("Format".to_string(), meta.format.clone()));

        if let (Some(w), Some(h)) = (meta.width, meta.height) {
            pairs.push(("Resolution".to_string(), format!("{}x{}", w, h)));
        }

        if let Some(ref codec) = meta.video_codec {
            pairs.push(("Video Codec".to_string(), codec.clone()));
        }

        if let Some(fps) = meta.frame_rate {
            pairs.push(("Frame Rate".to_string(), format!("{:.2} fps", fps)));
        }

        if let Some(ref codec) = meta.audio_codec {
            pairs.push(("Audio Codec".to_string(), codec.clone()));
        }

        if let Some(rate) = meta.audio_sample_rate {
            pairs.push(("Audio Sample Rate".to_string(), format!("{} Hz", rate)));
        }

        if let Some(channels) = meta.audio_channels {
            let ch_name = match channels {
                1 => "Mono".to_string(),
                2 => "Stereo".to_string(),
                6 => "5.1 Surround".to_string(),
                8 => "7.1 Surround".to_string(),
                n => format!("{} channels", n),
            };
            pairs.push(("Audio Channels".to_string(), ch_name));
        }

        if let Some(duration) = meta.duration_secs {
            pairs.push(("Duration".to_string(), Self::format_duration(duration)));
        }

        displays.push(RichDisplayOption::new(RichDisplay::KeyValue { pairs }));

        // Add duration display
        if let Some(duration) = meta.duration_secs {
            displays.push(RichDisplayOption::new(RichDisplay::Duration {
                millis: (duration * 1000.0) as u64,
                human: Self::format_duration(duration),
            }));
        }

        displays
    }
}

impl Format for VideoFormat {
    fn id(&self) -> &'static str {
        "video"
    }

    fn name(&self) -> &'static str {
        "Video File"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Media",
            description: "Video file with metadata extraction",
            examples: &["[binary video data]"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_video(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "video".to_string(),
                    confidence: 0.95,
                    description,
                    rich_display,
                }];
            }
        }

        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        let Some(meta) = Self::parse_video(bytes) else {
            return vec![];
        };

        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        vec![Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "video-info".to_string(),
            display: description,
            path: vec!["video-info".to_string()],
            steps: vec![],
            is_lossy: false,
            priority: ConversionPriority::Structured,
            display_only: true,
            kind: ConversionKind::Representation,
            hidden: false,
            rich_display,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["video", "mp4", "m4v", "mkv", "webm"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_video_format() {
        // MP4 with ftyp box
        let mp4_data = [
            0u8, 0, 0, 20, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm',
        ];
        assert_eq!(VideoFormat::detect_video_format(&mp4_data), Some("MP4"));

        // EBML header (MKV/WebM)
        let ebml_data = [0x1A, 0x45, 0xDF, 0xA3];
        // Without doctype marker, won't be detected
        assert_eq!(VideoFormat::detect_video_format(&ebml_data), None);

        // Not a video
        assert_eq!(VideoFormat::detect_video_format(b"not video"), None);
    }

    #[test]
    fn test_is_mp4() {
        let mp4_data = [
            0u8, 0, 0, 20, b'f', b't', b'y', b'p', b'm', b'p', b'4', b'1',
        ];
        assert!(VideoFormat::is_mp4(&mp4_data));

        // HEIC is not video
        let heic_data = [
            0u8, 0, 0, 20, b'f', b't', b'y', b'p', b'h', b'e', b'i', b'c',
        ];
        assert!(!VideoFormat::is_mp4(&heic_data));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(VideoFormat::format_duration(65.0), "1:05");
        assert_eq!(VideoFormat::format_duration(3661.0), "1:01:01");
    }

    #[test]
    fn test_matroska_codec_name() {
        assert_eq!(VideoFormat::matroska_codec_name("V_VP9"), "VP9");
        assert_eq!(VideoFormat::matroska_codec_name("V_MPEG4/ISO/AVC"), "H.264");
        assert_eq!(VideoFormat::matroska_codec_name("A_OPUS"), "Opus");
    }
}
