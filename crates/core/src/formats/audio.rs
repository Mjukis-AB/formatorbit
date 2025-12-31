//! Audio file format detection and metadata extraction.
//!
//! Detects audio files and extracts:
//! - Duration
//! - Sample rate, bit depth, channels
//! - Bitrate
//! - ID3 tags (artist, album, title, year, genre, track)

use std::io::Cursor;

use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct AudioFormat;

/// Audio metadata extracted from file.
#[derive(Debug, Clone, Default)]
struct AudioMetadata {
    format: String,
    codec: Option<String>,
    duration_secs: Option<f64>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
    bits_per_sample: Option<u32>,
    bitrate: Option<u32>,

    // ID3/Vorbis tags
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    year: Option<String>,
    genre: Option<String>,
    track: Option<String>,
}

impl AudioFormat {
    /// Check if data is an MP3 file.
    fn is_mp3(data: &[u8]) -> bool {
        // ID3v2 tag
        if data.len() >= 3 && data.starts_with(b"ID3") {
            return true;
        }
        // MP3 frame sync (0xFF followed by 0xE* or 0xF*)
        if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
            return true;
        }
        false
    }

    /// Check if data is a FLAC file.
    fn is_flac(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(b"fLaC")
    }

    /// Check if data is a WAV file.
    fn is_wav(data: &[u8]) -> bool {
        data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WAVE"
    }

    /// Check if data is an OGG file.
    fn is_ogg(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(b"OggS")
    }

    /// Check if data is an AAC file.
    fn is_aac(data: &[u8]) -> bool {
        // ADTS sync word
        if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xF0) == 0xF0 {
            return true;
        }
        false
    }

    /// Detect audio format from magic bytes.
    fn detect_audio_format(data: &[u8]) -> Option<&'static str> {
        if Self::is_mp3(data) {
            Some("MP3")
        } else if Self::is_flac(data) {
            Some("FLAC")
        } else if Self::is_wav(data) {
            Some("WAV")
        } else if Self::is_ogg(data) {
            Some("OGG")
        } else if Self::is_aac(data) {
            Some("AAC")
        } else {
            None
        }
    }

    /// Get format hint for symphonia.
    fn get_hint(format: &str) -> Hint {
        let mut hint = Hint::new();
        match format {
            "MP3" => hint.with_extension("mp3"),
            "FLAC" => hint.with_extension("flac"),
            "WAV" => hint.with_extension("wav"),
            "OGG" => hint.with_extension("ogg"),
            "AAC" => hint.with_extension("aac"),
            _ => &mut hint,
        };
        hint
    }

    /// Parse audio file and extract metadata using symphonia.
    fn parse_audio(data: &[u8]) -> Option<AudioMetadata> {
        let format_name = Self::detect_audio_format(data)?;

        let cursor = Cursor::new(data.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        let hint = Self::get_hint(format_name);
        let format_opts = FormatOptions::default();
        let meta_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &meta_opts)
            .ok()?;

        let mut meta = AudioMetadata {
            format: format_name.to_string(),
            ..Default::default()
        };

        // Get track info
        let mut format = probed.format;
        let tracks = format.tracks();

        // Get first audio track
        if let Some(track) = tracks.first() {
            let params = &track.codec_params;

            // Get codec name
            if params.codec != CODEC_TYPE_NULL {
                meta.codec = Some(Self::codec_name(params.codec));
            }

            // Get sample rate
            if let Some(rate) = params.sample_rate {
                meta.sample_rate = Some(rate);
            }

            // Get channels
            if let Some(channels) = params.channels {
                meta.channels = Some(channels.count() as u32);
            }

            // Get bits per sample
            if let Some(bits) = params.bits_per_sample {
                meta.bits_per_sample = Some(bits);
            }

            // Calculate duration
            if let Some(n_frames) = params.n_frames {
                if let Some(rate) = params.sample_rate {
                    let duration = n_frames as f64 / rate as f64;
                    meta.duration_secs = Some(duration);
                }
            }

            // Estimate bitrate for compressed formats
            if meta.codec.is_some() && meta.duration_secs.is_some() {
                let duration = meta.duration_secs.unwrap();
                if duration > 0.0 {
                    let bitrate = (data.len() as f64 * 8.0 / duration) as u32;
                    meta.bitrate = Some(bitrate / 1000); // kbps
                }
            }
        }

        // Extract metadata tags
        if let Some(metadata) = format.metadata().current() {
            for tag in metadata.tags() {
                let value = tag.value.to_string();
                if value.is_empty() {
                    continue;
                }

                match tag.std_key {
                    Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                        meta.title = Some(value);
                    }
                    Some(symphonia::core::meta::StandardTagKey::Artist) => {
                        meta.artist = Some(value);
                    }
                    Some(symphonia::core::meta::StandardTagKey::Album) => {
                        meta.album = Some(value);
                    }
                    Some(symphonia::core::meta::StandardTagKey::Date) => {
                        meta.year = Some(value);
                    }
                    Some(symphonia::core::meta::StandardTagKey::Genre) => {
                        meta.genre = Some(value);
                    }
                    Some(symphonia::core::meta::StandardTagKey::TrackNumber) => {
                        meta.track = Some(value);
                    }
                    _ => {}
                }
            }
        }

        Some(meta)
    }

    /// Get human-readable codec name.
    fn codec_name(codec: symphonia::core::codecs::CodecType) -> String {
        use symphonia::core::codecs::*;

        match codec {
            CODEC_TYPE_MP3 => "MP3".to_string(),
            CODEC_TYPE_FLAC => "FLAC".to_string(),
            CODEC_TYPE_VORBIS => "Vorbis".to_string(),
            CODEC_TYPE_AAC => "AAC".to_string(),
            CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE => "PCM 16-bit".to_string(),
            CODEC_TYPE_PCM_S24LE | CODEC_TYPE_PCM_S24BE => "PCM 24-bit".to_string(),
            CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE => "PCM 32-bit".to_string(),
            CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE => "PCM Float".to_string(),
            _ => "Unknown".to_string(),
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
    fn format_description(meta: &AudioMetadata) -> String {
        let mut parts = vec![meta.format.clone()];

        if let Some(ref title) = meta.title {
            parts.push(format!("\"{}\"", title));
        }

        if let Some(ref artist) = meta.artist {
            parts.push(format!("by {}", artist));
        }

        if let Some(duration) = meta.duration_secs {
            parts.push(Self::format_duration(duration));
        }

        if let Some(bitrate) = meta.bitrate {
            parts.push(format!("{} kbps", bitrate));
        } else if let Some(bits) = meta.bits_per_sample {
            parts.push(format!("{}-bit", bits));
        }

        parts.join(", ")
    }

    /// Build RichDisplay options for UI rendering.
    fn build_rich_display(meta: &AudioMetadata) -> Vec<RichDisplayOption> {
        let mut displays = vec![];
        let mut pairs = vec![];

        pairs.push(("Format".to_string(), meta.format.clone()));

        if let Some(ref codec) = meta.codec {
            pairs.push(("Codec".to_string(), codec.clone()));
        }

        if let Some(duration) = meta.duration_secs {
            pairs.push(("Duration".to_string(), Self::format_duration(duration)));
        }

        if let Some(rate) = meta.sample_rate {
            pairs.push(("Sample Rate".to_string(), format!("{} Hz", rate)));
        }

        if let Some(channels) = meta.channels {
            let ch_name = match channels {
                1 => "Mono".to_string(),
                2 => "Stereo".to_string(),
                n => format!("{} channels", n),
            };
            pairs.push(("Channels".to_string(), ch_name));
        }

        if let Some(bits) = meta.bits_per_sample {
            pairs.push(("Bit Depth".to_string(), format!("{}-bit", bits)));
        }

        if let Some(bitrate) = meta.bitrate {
            pairs.push(("Bitrate".to_string(), format!("{} kbps", bitrate)));
        }

        // ID3 tags
        if let Some(ref title) = meta.title {
            pairs.push(("Title".to_string(), title.clone()));
        }

        if let Some(ref artist) = meta.artist {
            pairs.push(("Artist".to_string(), artist.clone()));
        }

        if let Some(ref album) = meta.album {
            pairs.push(("Album".to_string(), album.clone()));
        }

        if let Some(ref year) = meta.year {
            pairs.push(("Year".to_string(), year.clone()));
        }

        if let Some(ref genre) = meta.genre {
            pairs.push(("Genre".to_string(), genre.clone()));
        }

        if let Some(ref track) = meta.track {
            pairs.push(("Track".to_string(), track.clone()));
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

impl Format for AudioFormat {
    fn id(&self) -> &'static str {
        "audio"
    }

    fn name(&self) -> &'static str {
        "Audio File"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Media",
            description: "Audio file with metadata extraction",
            examples: &["[binary audio data]"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_audio(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "audio".to_string(),
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

        let Some(meta) = Self::parse_audio(bytes) else {
            return vec![];
        };

        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        vec![Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "audio-info".to_string(),
            display: description,
            path: vec!["audio-info".to_string()],
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
        &["audio", "mp3", "flac", "wav", "ogg", "aac"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_audio_format() {
        assert_eq!(AudioFormat::detect_audio_format(b"ID3..."), Some("MP3"));
        assert_eq!(AudioFormat::detect_audio_format(b"fLaC"), Some("FLAC"));
        assert_eq!(
            AudioFormat::detect_audio_format(b"RIFF....WAVE"),
            Some("WAV")
        );
        assert_eq!(AudioFormat::detect_audio_format(b"OggS"), Some("OGG"));
        assert_eq!(AudioFormat::detect_audio_format(b"not audio"), None);
    }

    #[test]
    fn test_is_mp3() {
        assert!(AudioFormat::is_mp3(b"ID3\x04\x00"));
        assert!(AudioFormat::is_mp3(&[0xFF, 0xFB]));
        assert!(!AudioFormat::is_mp3(b"not mp3"));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(AudioFormat::format_duration(65.0), "1:05");
        assert_eq!(AudioFormat::format_duration(3661.0), "1:01:01");
        assert_eq!(AudioFormat::format_duration(0.0), "0:00");
    }
}
