//! Image format detection and metadata extraction.
//!
//! Detects image files and extracts:
//! - Basic properties: dimensions, format, color depth
//! - EXIF metadata: camera, lens, settings, GPS, date
//! - Analysis: dominant colors, brightness, screenshot detection

use std::collections::HashMap;
use std::io::Cursor;

use image::GenericImageView;

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation, RichDisplay, RichDisplayOption};

pub struct ImageFormat;

/// Common screen resolutions for screenshot detection.
const SCREEN_RESOLUTIONS: &[(u32, u32, &str)] = &[
    // Desktop
    (1920, 1080, "1080p"),
    (2560, 1440, "1440p"),
    (3840, 2160, "4K"),
    (1366, 768, "HD"),
    (1280, 720, "720p"),
    (1440, 900, "WXGA+"),
    (1680, 1050, "WSXGA+"),
    (2560, 1600, "WQXGA"),
    (3440, 1440, "UWQHD"),
    (5120, 2880, "5K"),
    // MacBook
    (2880, 1800, "MacBook Pro 15\" Retina"),
    (2560, 1600, "MacBook Pro 13\" Retina"),
    (3024, 1964, "MacBook Pro 14\" Retina"),
    (3456, 2234, "MacBook Pro 16\" Retina"),
    // iMac
    (4480, 2520, "iMac 24\" 4.5K"),
    (5120, 2880, "iMac 27\" 5K"),
    // Mobile (portrait)
    (1170, 2532, "iPhone 12/13/14"),
    (1179, 2556, "iPhone 14 Pro"),
    (1284, 2778, "iPhone 12/13/14 Pro Max"),
    (1290, 2796, "iPhone 14 Pro Max"),
    (1125, 2436, "iPhone X/XS/11 Pro"),
    (1242, 2688, "iPhone XS Max/11 Pro Max"),
    (1080, 2340, "Android common"),
    (1440, 3200, "Android QHD+"),
    // Tablet
    (2048, 2732, "iPad Pro 12.9\""),
    (1668, 2388, "iPad Pro 11\""),
    (2160, 1620, "iPad 10.2\""),
];

/// Common aspect ratios.
const ASPECT_RATIOS: &[((u32, u32), &str)] = &[
    ((16, 9), "16:9"),
    ((9, 16), "9:16"),
    ((4, 3), "4:3"),
    ((3, 4), "3:4"),
    ((1, 1), "1:1"),
    ((3, 2), "3:2"),
    ((2, 3), "2:3"),
    ((21, 9), "21:9"),
    ((9, 21), "9:21"),
    ((16, 10), "16:10"),
    ((10, 16), "10:16"),
    ((5, 4), "5:4"),
    ((4, 5), "4:5"),
    ((2, 1), "2:1"),
    ((1, 2), "1:2"),
];

/// Image metadata extracted from file.
#[derive(Debug, Clone)]
struct ImageMetadata {
    // Basic properties
    width: u32,
    height: u32,
    format: String,
    color_type: String,
    bit_depth: u8,

    // Computed
    aspect_ratio: Option<String>,
    is_screenshot: Option<String>,

    // EXIF data
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens_model: Option<String>,
    focal_length: Option<String>,
    aperture: Option<String>,
    shutter_speed: Option<String>,
    iso: Option<u32>,
    exposure_comp: Option<String>,
    flash: Option<String>,

    // GPS
    gps_lat: Option<f64>,
    gps_lon: Option<f64>,
    gps_alt: Option<f64>,

    // Date/Time
    date_taken: Option<String>,

    // Other metadata
    software: Option<String>,
    copyright: Option<String>,
    artist: Option<String>,
    orientation: Option<u32>,
    color_space: Option<String>,

    // Analysis
    dominant_colors: Vec<(u8, u8, u8)>,
    average_brightness: Option<f32>,
}

impl ImageFormat {
    /// Detect image format from magic bytes.
    fn detect_format(data: &[u8]) -> Option<&'static str> {
        if data.len() < 12 {
            return None;
        }

        // Check magic bytes
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some("JPEG");
        }
        if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Some("PNG");
        }
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return Some("GIF");
        }
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
            return Some("WebP");
        }
        if data.starts_with(b"BM") {
            return Some("BMP");
        }
        if data.starts_with(&[0x49, 0x49, 0x2A, 0x00])
            || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
        {
            return Some("TIFF");
        }
        if data.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
            return Some("ICO");
        }
        // HEIC/HEIF (ftyp box)
        if data.len() >= 12 && &data[4..8] == b"ftyp" {
            let brand = &data[8..12];
            if brand == b"heic" || brand == b"heix" || brand == b"hevc" || brand == b"mif1" {
                return Some("HEIC");
            }
            if brand == b"avif" {
                return Some("AVIF");
            }
        }

        None
    }

    /// Calculate aspect ratio string.
    fn get_aspect_ratio(width: u32, height: u32) -> Option<String> {
        if width == 0 || height == 0 {
            return None;
        }

        let gcd = Self::gcd(width, height);
        let ratio_w = width / gcd;
        let ratio_h = height / gcd;

        // Check known ratios
        for ((rw, rh), name) in ASPECT_RATIOS {
            // Allow small tolerance for rounding
            let expected_ratio = *rw as f64 / *rh as f64;
            let actual_ratio = width as f64 / height as f64;
            if (expected_ratio - actual_ratio).abs() < 0.02 {
                return Some(name.to_string());
            }
        }

        // Return raw ratio if not a common one
        Some(format!("{}:{}", ratio_w, ratio_h))
    }

    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 {
            a
        } else {
            Self::gcd(b, a % b)
        }
    }

    /// Check if dimensions match a common screen resolution.
    fn detect_screenshot(width: u32, height: u32) -> Option<String> {
        for (w, h, name) in SCREEN_RESOLUTIONS {
            if (width == *w && height == *h) || (width == *h && height == *w) {
                return Some(name.to_string());
            }
        }
        None
    }

    /// Parse EXIF data from image bytes.
    fn parse_exif(data: &[u8]) -> HashMap<String, String> {
        let mut result = HashMap::new();

        let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(data)) else {
            return result;
        };

        for field in exif.fields() {
            let tag_name = format!("{}", field.tag);
            let value = field.display_value().to_string();
            result.insert(tag_name, value);
        }

        result
    }

    /// Extract GPS coordinates from EXIF data.
    fn extract_gps(exif_data: &HashMap<String, String>) -> (Option<f64>, Option<f64>, Option<f64>) {
        let lat = Self::parse_gps_coord(
            exif_data.get("GPSLatitude"),
            exif_data.get("GPSLatitudeRef"),
        );
        let lon = Self::parse_gps_coord(
            exif_data.get("GPSLongitude"),
            exif_data.get("GPSLongitudeRef"),
        );
        let alt = exif_data.get("GPSAltitude").and_then(|s| {
            // Parse "123.4 m" or just "123.4"
            s.split_whitespace().next()?.parse().ok()
        });

        (lat, lon, alt)
    }

    /// Parse GPS coordinate from EXIF format.
    fn parse_gps_coord(coord: Option<&String>, ref_dir: Option<&String>) -> Option<f64> {
        let coord = coord?;

        // EXIF GPS format: "deg min sec" or "deg/1 min/1 sec/100"
        // Try to parse various formats
        let parts: Vec<&str> = coord.split_whitespace().collect();

        let (deg, min, sec) = if parts.len() >= 3 {
            // "37 deg 48 min 12.34 sec" format
            let deg: f64 = parts.first()?.parse().ok()?;
            let min: f64 = parts.get(2).or(parts.get(1))?.parse().ok().unwrap_or(0.0);
            let sec: f64 = parts.get(4).or(parts.get(2))?.parse().ok().unwrap_or(0.0);
            (deg, min, sec)
        } else {
            return None;
        };

        let mut decimal = deg + min / 60.0 + sec / 3600.0;

        // Apply direction
        if let Some(dir) = ref_dir {
            if dir == "S" || dir == "W" {
                decimal = -decimal;
            }
        }

        Some(decimal)
    }

    /// Analyze image for dominant colors and brightness.
    fn analyze_image(img: &image::DynamicImage) -> (Vec<(u8, u8, u8)>, Option<f32>) {
        let (width, height) = img.dimensions();

        // Sample pixels (don't process every pixel for large images)
        let sample_step = ((width * height) / 10000).max(1) as usize;

        let mut color_counts: HashMap<(u8, u8, u8), u32> = HashMap::new();
        let mut total_brightness: f64 = 0.0;
        let mut pixel_count: u64 = 0;

        for (i, pixel) in img.to_rgb8().pixels().enumerate() {
            if i % sample_step != 0 {
                continue;
            }

            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];

            // Quantize colors to reduce unique count (divide by 32 = 8 levels per channel)
            let qr = (r / 32) * 32;
            let qg = (g / 32) * 32;
            let qb = (b / 32) * 32;

            *color_counts.entry((qr, qg, qb)).or_insert(0) += 1;

            // Calculate brightness (perceived luminance)
            let brightness = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            total_brightness += brightness;
            pixel_count += 1;
        }

        // Get top 5 dominant colors
        let mut colors: Vec<_> = color_counts.into_iter().collect();
        colors.sort_by(|a, b| b.1.cmp(&a.1));
        let dominant: Vec<(u8, u8, u8)> = colors.into_iter().take(5).map(|(c, _)| c).collect();

        let avg_brightness = if pixel_count > 0 {
            Some((total_brightness / pixel_count as f64) as f32)
        } else {
            None
        };

        (dominant, avg_brightness)
    }

    /// Parse image and extract all metadata.
    fn parse_image(data: &[u8]) -> Option<ImageMetadata> {
        let format_name = Self::detect_format(data)?;

        // Parse image with image crate
        let img = image::load_from_memory(data).ok()?;
        let (width, height) = img.dimensions();

        let color_type = format!("{:?}", img.color());
        let bit_depth = match img.color() {
            image::ColorType::L8
            | image::ColorType::La8
            | image::ColorType::Rgb8
            | image::ColorType::Rgba8 => 8,
            image::ColorType::L16
            | image::ColorType::La16
            | image::ColorType::Rgb16
            | image::ColorType::Rgba16 => 16,
            image::ColorType::Rgb32F | image::ColorType::Rgba32F => 32,
            _ => 8,
        };

        // Parse EXIF
        let exif_data = Self::parse_exif(data);
        let (gps_lat, gps_lon, gps_alt) = Self::extract_gps(&exif_data);

        // Analyze image
        let (dominant_colors, average_brightness) = Self::analyze_image(&img);

        Some(ImageMetadata {
            width,
            height,
            format: format_name.to_string(),
            color_type,
            bit_depth,
            aspect_ratio: Self::get_aspect_ratio(width, height),
            is_screenshot: Self::detect_screenshot(width, height),
            camera_make: exif_data.get("Make").cloned(),
            camera_model: exif_data.get("Model").cloned(),
            lens_model: exif_data.get("LensModel").cloned(),
            focal_length: exif_data.get("FocalLength").cloned(),
            aperture: exif_data.get("FNumber").cloned(),
            shutter_speed: exif_data.get("ExposureTime").cloned(),
            iso: exif_data
                .get("PhotographicSensitivity")
                .and_then(|s| s.parse().ok()),
            exposure_comp: exif_data.get("ExposureBiasValue").cloned(),
            flash: exif_data.get("Flash").cloned(),
            gps_lat,
            gps_lon,
            gps_alt,
            date_taken: exif_data
                .get("DateTimeOriginal")
                .or(exif_data.get("DateTime"))
                .cloned(),
            software: exif_data.get("Software").cloned(),
            copyright: exif_data.get("Copyright").cloned(),
            artist: exif_data.get("Artist").cloned(),
            orientation: exif_data.get("Orientation").and_then(|s| s.parse().ok()),
            color_space: exif_data.get("ColorSpace").cloned(),
            dominant_colors,
            average_brightness,
        })
    }

    /// Format metadata as description string.
    fn format_description(meta: &ImageMetadata) -> String {
        let mut parts = vec![format!("{} {}×{}", meta.format, meta.width, meta.height)];

        if let Some(ref ratio) = meta.aspect_ratio {
            parts.push(format!("({})", ratio));
        }

        if let Some(ref model) = meta.camera_model {
            parts.push(format!("• {}", model));
        }

        if let Some(ref date) = meta.date_taken {
            parts.push(format!("• {}", date));
        }

        if meta.gps_lat.is_some() {
            parts.push("• GPS".to_string());
        }

        if let Some(ref screen) = meta.is_screenshot {
            parts.push(format!("• Screenshot ({})", screen));
        }

        parts.join(" ")
    }

    /// Build RichDisplay options for the image.
    fn build_rich_display(meta: &ImageMetadata) -> Vec<RichDisplayOption> {
        let mut displays = vec![];

        // Key-value pairs for metadata
        let mut pairs = vec![
            (
                "Dimensions".to_string(),
                format!("{}×{}", meta.width, meta.height),
            ),
            ("Format".to_string(), meta.format.clone()),
            ("Color".to_string(), meta.color_type.clone()),
            ("Bit depth".to_string(), format!("{}-bit", meta.bit_depth)),
        ];

        if let Some(ref ratio) = meta.aspect_ratio {
            pairs.push(("Aspect ratio".to_string(), ratio.clone()));
        }

        if let Some(ref screen) = meta.is_screenshot {
            pairs.push(("Screenshot".to_string(), screen.clone()));
        }

        // Camera info
        if let Some(ref make) = meta.camera_make {
            pairs.push(("Camera make".to_string(), make.clone()));
        }
        if let Some(ref model) = meta.camera_model {
            pairs.push(("Camera model".to_string(), model.clone()));
        }
        if let Some(ref lens) = meta.lens_model {
            pairs.push(("Lens".to_string(), lens.clone()));
        }

        // Exposure settings
        if let Some(ref focal) = meta.focal_length {
            pairs.push(("Focal length".to_string(), focal.clone()));
        }
        if let Some(ref aperture) = meta.aperture {
            pairs.push(("Aperture".to_string(), aperture.clone()));
        }
        if let Some(ref shutter) = meta.shutter_speed {
            pairs.push(("Shutter speed".to_string(), shutter.clone()));
        }
        if let Some(iso) = meta.iso {
            pairs.push(("ISO".to_string(), format!("{}", iso)));
        }

        // Date
        if let Some(ref date) = meta.date_taken {
            pairs.push(("Date taken".to_string(), date.clone()));
        }

        // Software
        if let Some(ref sw) = meta.software {
            pairs.push(("Software".to_string(), sw.clone()));
        }

        // Additional EXIF fields
        if let Some(ref exp) = meta.exposure_comp {
            pairs.push(("Exposure comp".to_string(), exp.clone()));
        }
        if let Some(ref flash) = meta.flash {
            pairs.push(("Flash".to_string(), flash.clone()));
        }
        if let Some(ref copyright) = meta.copyright {
            pairs.push(("Copyright".to_string(), copyright.clone()));
        }
        if let Some(ref artist) = meta.artist {
            pairs.push(("Artist".to_string(), artist.clone()));
        }
        if let Some(orientation) = meta.orientation {
            let orientation_str = match orientation {
                1 => "Normal",
                2 => "Flipped horizontally",
                3 => "Rotated 180°",
                4 => "Flipped vertically",
                5 => "Rotated 90° CW + flipped",
                6 => "Rotated 90° CW",
                7 => "Rotated 90° CCW + flipped",
                8 => "Rotated 90° CCW",
                _ => "Unknown",
            };
            pairs.push(("Orientation".to_string(), orientation_str.to_string()));
        }
        if let Some(ref cs) = meta.color_space {
            pairs.push(("Color space".to_string(), cs.clone()));
        }

        // Brightness
        if let Some(brightness) = meta.average_brightness {
            let level = if brightness < 50.0 {
                "Dark"
            } else if brightness < 128.0 {
                "Medium-dark"
            } else if brightness < 200.0 {
                "Medium-bright"
            } else {
                "Bright"
            };
            pairs.push((
                "Brightness".to_string(),
                format!("{} ({:.0})", level, brightness),
            ));
        }

        displays.push(RichDisplayOption::new(RichDisplay::KeyValue { pairs }));

        // GPS map
        if let (Some(lat), Some(lon)) = (meta.gps_lat, meta.gps_lon) {
            let label = meta.gps_alt.map(|alt| format!("{:.0}m altitude", alt));
            displays.push(RichDisplayOption::new(RichDisplay::Map { lat, lon, label }));
        }

        // Dominant colors
        for (r, g, b) in &meta.dominant_colors {
            displays.push(RichDisplayOption::new(RichDisplay::Color {
                r: *r,
                g: *g,
                b: *b,
                a: 255,
            }));
        }

        displays
    }
}

impl Format for ImageFormat {
    fn id(&self) -> &'static str {
        "image"
    }

    fn name(&self) -> &'static str {
        "Image"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Media",
            description: "Image files with EXIF metadata extraction",
            examples: &["[binary image data]"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Images are binary, but we might receive hex or base64 encoded data
        // For now, we don't parse from string - the bytes come from file reading
        // This would be handled by a file loader that passes bytes directly

        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_image(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "image".to_string(),
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

    fn conversions(&self, value: &CoreValue) -> Vec<crate::types::Conversion> {
        // Parse bytes as image
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        let Some(meta) = Self::parse_image(bytes) else {
            return vec![];
        };

        let mut conversions = vec![];
        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        // Main image interpretation
        conversions.push(crate::types::Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "image-info".to_string(),
            display: description,
            path: vec!["image-info".to_string()],
            steps: vec![],
            is_lossy: false,
            priority: crate::types::ConversionPriority::Structured,
            display_only: true,
            kind: crate::types::ConversionKind::Representation,
            hidden: false,
            rich_display,
        });

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &[
            "img", "photo", "picture", "jpeg", "jpg", "png", "gif", "webp",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_jpeg() {
        let data = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01,
        ];
        assert_eq!(ImageFormat::detect_format(&data), Some("JPEG"));
    }

    #[test]
    fn test_detect_png() {
        let data = [
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        ];
        assert_eq!(ImageFormat::detect_format(&data), Some("PNG"));
    }

    #[test]
    fn test_detect_gif() {
        let data = b"GIF89a......";
        assert_eq!(ImageFormat::detect_format(data), Some("GIF"));
    }

    #[test]
    fn test_aspect_ratio() {
        assert_eq!(
            ImageFormat::get_aspect_ratio(1920, 1080),
            Some("16:9".to_string())
        );
        assert_eq!(
            ImageFormat::get_aspect_ratio(1080, 1920),
            Some("9:16".to_string())
        );
        assert_eq!(
            ImageFormat::get_aspect_ratio(1000, 1000),
            Some("1:1".to_string())
        );
        assert_eq!(
            ImageFormat::get_aspect_ratio(4032, 3024),
            Some("4:3".to_string())
        );
    }

    #[test]
    fn test_screenshot_detection() {
        assert_eq!(
            ImageFormat::detect_screenshot(1920, 1080),
            Some("1080p".to_string())
        );
        assert_eq!(
            ImageFormat::detect_screenshot(2560, 1440),
            Some("1440p".to_string())
        );
        assert_eq!(ImageFormat::detect_screenshot(1234, 5678), None);
    }

    #[test]
    fn test_gcd() {
        assert_eq!(ImageFormat::gcd(1920, 1080), 120);
        assert_eq!(ImageFormat::gcd(16, 9), 1);
        assert_eq!(ImageFormat::gcd(100, 100), 100);
    }
}
