use image::ImageDecoder;
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn path_to_tilde(path: &Path) -> String {
    if let Ok(home) = std::env::var("HOME") {
        let home_str = home.trim_end_matches('/');
        let path_str = path.to_string_lossy().trim_end_matches('/').to_string();
        if let Some(after_home) = path_str.strip_prefix(home_str) {
            if after_home.is_empty() {
                return "~".to_string();
            }
            return format!("~{}", after_home);
        }
    }
    path.to_string_lossy().to_string()
}

#[derive(Clone)]
pub struct ImageSettings {
    pub quality: u8,
    pub color_space: ColorSpace,
    pub remove_exif: bool,
    pub output_format: OutputFormat,
    pub output_directory: Option<PathBuf>,
    pub progressive: bool,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub png_compression: u8,
    pub webp_lossless: bool,
    pub overwrite: bool,
    pub backup: bool,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            quality: 85,
            color_space: ColorSpace::Rgb,
            remove_exif: true,
            output_format: OutputFormat::Same,
            output_directory: None,
            progressive: false,
            max_width: None,
            max_height: None,
            png_compression: 6,
            webp_lossless: false,
            overwrite: false,
            backup: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorSpace {
    Rgb,
    Grayscale,
    Rgba,
}

impl ColorSpace {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColorSpace::Rgb => "RGB",
            ColorSpace::Grayscale => "Grayscale",
            ColorSpace::Rgba => "RGBA",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutputFormat {
    Same,
    Jpeg,
    Png,
    Webp,
    Gif,
    Tiff,
    Bmp,
    Tga,
    #[cfg(feature = "avif")]
    Avif,
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Same => "Same",
            OutputFormat::Jpeg => "JPEG",
            OutputFormat::Png => "PNG",
            OutputFormat::Webp => "WebP",
            OutputFormat::Gif => "GIF",
            OutputFormat::Tiff => "TIFF",
            OutputFormat::Bmp => "BMP",
            OutputFormat::Tga => "TGA",
            #[cfg(feature = "avif")]
            OutputFormat::Avif => "AVIF",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Same => "",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::Png => "png",
            OutputFormat::Webp => "webp",
            OutputFormat::Gif => "gif",
            OutputFormat::Tiff => "tiff",
            OutputFormat::Bmp => "bmp",
            OutputFormat::Tga => "tga",
            #[cfg(feature = "avif")]
            OutputFormat::Avif => "avif",
        }
    }

    pub fn supports_quality(&self) -> bool {
        match self {
            OutputFormat::Jpeg | OutputFormat::Webp | OutputFormat::Tiff => true,
            #[cfg(feature = "avif")]
            OutputFormat::Avif => true,
            _ => false,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some(OutputFormat::Jpeg),
            "png" => Some(OutputFormat::Png),
            "webp" => Some(OutputFormat::Webp),
            "gif" => Some(OutputFormat::Gif),
            "tiff" | "tif" => Some(OutputFormat::Tiff),
            "bmp" => Some(OutputFormat::Bmp),
            "tga" => Some(OutputFormat::Tga),
            #[cfg(feature = "avif")]
            "avif" => Some(OutputFormat::Avif),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExifData {
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub date_taken: Option<String>,
    pub exposure: Option<String>,
    pub iso: Option<String>,
    pub aperture: Option<String>,
    pub focal_length: Option<String>,
    pub flash: Option<String>,
}

impl ExifData {
    pub fn read_from_file(path: &Path) -> Option<Self> {
        let file = std::fs::File::open(path).ok()?;
        let mut bufreader = std::io::BufReader::new(&file);
        let exif = exif::Reader::new()
            .read_from_container(&mut bufreader)
            .ok()?;

        let get_string = |tag: exif::Tag| -> Option<String> {
            exif.get_field(tag, exif::In::PRIMARY)
                .map(|f| f.display_value().to_string())
        };

        Some(Self {
            camera: get_string(exif::Tag::Model),
            lens: get_string(exif::Tag::LensModel),
            date_taken: get_string(exif::Tag::DateTimeOriginal),
            exposure: get_string(exif::Tag::ExposureTime).map(|v| {
                if v.ends_with(" s") {
                    format!(
                        "1/{}",
                        (1.0 / v.trim_end_matches(" s").parse::<f64>().unwrap_or(1.0)).round()
                            as i32
                    )
                } else {
                    v
                }
            }),
            iso: get_string(exif::Tag::PhotographicSensitivity),
            aperture: get_string(exif::Tag::FNumber),
            focal_length: get_string(exif::Tag::FocalLength),
            flash: get_string(exif::Tag::Flash),
        })
    }
}

#[derive(Clone)]
pub struct CachedImageInfo {
    pub dimensions: Option<(u32, u32)>,
    pub color_type: Option<String>,
    pub file_mtime: u64,
}

pub struct ImageFile {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_parent: bool,
    pub size: u64,
    pub dimensions: Option<(u32, u32)>,
    pub color_type: Option<String>,
    pub needs_exif: bool,
    pub settings: ImageSettings,
    pub queued: bool,
    pub selected: bool,
    pub exif_data: Option<ExifData>,
}

impl ImageFile {
    pub fn new_parent() -> Self {
        Self {
            path: PathBuf::from(".."),
            name: "..".to_string(),
            is_dir: true,
            is_parent: true,
            size: 0,
            dimensions: None,
            color_type: None,
            needs_exif: false,
            settings: ImageSettings::default(),
            queued: false,
            selected: false,
            exif_data: None,
        }
    }

    pub fn new(path: PathBuf) -> Self {
        let is_dir = path.is_dir();
        let mut file = Self::new_lightweight(path, is_dir);

        if !is_dir {
            file.dimensions = image::image_dimensions(&file.path).ok();
            file.color_type = fast_color_type(&file.path);
        }

        file
    }

    pub fn new_lightweight(path: PathBuf, is_dir: bool) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let size = if is_dir {
            0
        } else {
            fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
        };

        let needs_exif = if !is_dir {
            path.extension()
                .and_then(|e| e.to_str())
                .map(|ext| {
                    let lower = ext.to_lowercase();
                    lower == "jpg" || lower == "jpeg"
                })
                .unwrap_or(false)
        } else {
            false
        };

        let settings = ImageSettings::default();

        Self {
            path,
            name,
            is_dir,
            is_parent: false,
            size,
            dimensions: None,
            color_type: None,
            needs_exif,
            settings,
            queued: false,
            selected: false,
            exif_data: None,
        }
    }

    pub fn load_exif_if_needed(&mut self, exif_cache: &mut HashMap<PathBuf, ExifData>) {
        if self.needs_exif && self.exif_data.is_none() {
            self.exif_data = if let Some(cached) = exif_cache.get(&self.path) {
                Some(cached.clone())
            } else {
                let exif = ExifData::read_from_file(&self.path);
                if let Some(ref data) = exif {
                    exif_cache.insert(self.path.clone(), data.clone());
                }
                exif
            };
        }
    }

    pub fn extension(&self) -> Option<String> {
        self.path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
    }

    pub fn format_name(&self) -> String {
        self.extension()
            .map(|e| e.to_uppercase())
            .unwrap_or_else(|| "???".to_string())
    }

    pub fn size_str(&self) -> String {
        if self.is_dir {
            "".to_string()
        } else {
            bytes_to_human(self.size)
        }
    }

    pub fn dimensions_str(&self) -> String {
        self.dimensions
            .map(|(w, h)| format!("{}×{}", w, h))
            .unwrap_or_default()
    }
}

pub fn color_type_str(color: image::ExtendedColorType) -> &'static str {
    match color {
        image::ExtendedColorType::L8 => "L",
        image::ExtendedColorType::La8 => "La",
        image::ExtendedColorType::Rgb8 => "RGB",
        image::ExtendedColorType::Rgba8 => "RGBA",
        image::ExtendedColorType::L16 => "L16",
        image::ExtendedColorType::La16 => "La16",
        image::ExtendedColorType::Rgb16 => "RGB16",
        image::ExtendedColorType::Rgba16 => "RGBA16",
        image::ExtendedColorType::L2 => "L32",
        image::ExtendedColorType::La2 => "La32",
        image::ExtendedColorType::Rgb2 => "RGB32",
        image::ExtendedColorType::Rgba2 => "RGBA32",
        _ => "???",
    }
}

pub fn fast_color_type(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();

    let file = std::fs::File::open(path).ok()?;
    let mmap = unsafe { Mmap::map(&file).ok()? };
    let cursor = io::Cursor::new(&mmap[..]);

    let color = match ext.as_str() {
        "png" => {
            let decoder = image::codecs::png::PngDecoder::new(cursor).ok()?;
            decoder.original_color_type()
        }
        "jpg" | "jpeg" => {
            let decoder = image::codecs::jpeg::JpegDecoder::new(cursor).ok()?;
            decoder.original_color_type()
        }
        "webp" => {
            let decoder = image::codecs::webp::WebPDecoder::new(cursor).ok()?;
            decoder.original_color_type()
        }
        _ => return None,
    };

    Some(color_type_str(color).to_string())
}

pub fn bytes_to_human(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn truncate_str(s: &str, max_len: usize) -> String {
    if max_len < 4 {
        return "...".to_string();
    }
    if s.len() > max_len {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}
