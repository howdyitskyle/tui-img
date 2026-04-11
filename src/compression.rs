use crate::cache::get_unique_path;
use crate::models::{ColorSpace, ImageFile, ImageSettings, OutputFormat};
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write as IoWrite;
use std::path::Path;

fn ensure_dir_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).context("Failed to create output directory")?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct FileResult {
    pub file_index: usize,
    pub original_size: u64,
    pub new_size: u64,
    pub output_filename: Option<String>,
    pub error: Option<String>,
}

pub enum CompressionEvent {
    Started(usize),
    Progress {
        current: usize,
        total: usize,
        filename: String,
        sub_progress: u8,
    },
    Stage(String),
    FileCompleted(FileResult),
    Completed {
        success_count: usize,
        total_saved: u64,
        results: Vec<FileResult>,
    },
    Cancelled,
}

pub fn compress_image(
    file: &ImageFile,
    output_path: &Path,
    global_format: Option<OutputFormat>,
) -> Result<(u64, String)> {
    let img = image::open(&file.path).context("Failed to open image")?;
    let processed = apply_processing(img, &file.settings);

    let target_format = global_format.unwrap_or(file.settings.output_format);

    let base_output_path = if target_format != OutputFormat::Same {
        let ext = target_format.extension();
        let stem = output_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        output_path.with_file_name(format!("{}.{}", stem, ext))
    } else {
        output_path.to_path_buf()
    };

    let final_output_path = if file.settings.overwrite {
        base_output_path.clone()
    } else {
        get_unique_path(&base_output_path)
    };

    ensure_dir_exists(&final_output_path)?;

    match target_format {
        OutputFormat::Same => match file.extension().as_deref() {
            Some("jpg") | Some("jpeg") => {
                compress_jpeg(&processed, &final_output_path, &file.settings)
            }
            Some("png") => compress_png(&processed, &final_output_path, &file.settings),
            Some("webp") => compress_webp(&processed, &final_output_path, &file.settings),
            Some("gif") => compress_gif(&processed, &final_output_path, &file.settings),
            Some("tiff") | Some("tif") => {
                compress_tiff(&processed, &final_output_path, &file.settings)
            }
            Some("bmp") => compress_bmp(&processed, &final_output_path, &file.settings),
            Some("tga") => compress_tga(&processed, &final_output_path, &file.settings),
            _ => anyhow::bail!("Unsupported format"),
        },
        OutputFormat::Jpeg => compress_jpeg(&processed, &final_output_path, &file.settings),
        OutputFormat::Png => compress_png(&processed, &final_output_path, &file.settings),
        OutputFormat::Webp => compress_webp(&processed, &final_output_path, &file.settings),
        OutputFormat::Gif => compress_gif(&processed, &final_output_path, &file.settings),
        OutputFormat::Tiff => compress_tiff(&processed, &final_output_path, &file.settings),
        OutputFormat::Bmp => compress_bmp(&processed, &final_output_path, &file.settings),
        OutputFormat::Tga => compress_tga(&processed, &final_output_path, &file.settings),
    }?;

    let output_filename = final_output_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file.name.clone());

    Ok((
        std::fs::metadata(&final_output_path)?.len(),
        output_filename,
    ))
}

fn apply_processing(img: image::DynamicImage, settings: &ImageSettings) -> image::DynamicImage {
    let img = match (settings.max_width, settings.max_height) {
        (Some(max_w), Some(max_h)) => {
            let (w, h) = (img.width(), img.height());
            if w > max_w || h > max_h {
                img.resize(max_w, max_h, image::imageops::FilterType::Lanczos3)
            } else {
                img
            }
        }
        (Some(max_w), None) => {
            let (w, h) = (img.width(), img.height());
            if w > max_w {
                let ratio = max_w as f32 / w as f32;
                img.resize(
                    max_w,
                    (h as f32 * ratio) as u32,
                    image::imageops::FilterType::Lanczos3,
                )
            } else {
                img
            }
        }
        (None, Some(max_h)) => {
            let (w, h) = (img.width(), img.height());
            if h > max_h {
                let ratio = max_h as f32 / h as f32;
                img.resize(
                    (w as f32 * ratio) as u32,
                    max_h,
                    image::imageops::FilterType::Lanczos3,
                )
            } else {
                img
            }
        }
        (None, None) => img,
    };

    match settings.color_space {
        ColorSpace::Rgb => img.to_rgb8().into(),
        ColorSpace::Grayscale => image::DynamicImage::ImageLuma8(img.to_luma8()),
        ColorSpace::Rgba => img.to_rgba8().into(),
    }
}

fn compress_jpeg(
    img: &image::DynamicImage,
    output_path: &Path,
    settings: &ImageSettings,
) -> Result<u64> {
    let rgb = img.to_rgb8();
    let mut buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buffer);

    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, settings.quality);
    encoder.encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )?;

    let mut file = File::create(output_path)?;
    file.write_all(&buffer)?;

    Ok(buffer.len() as u64)
}

fn compress_png(
    img: &image::DynamicImage,
    output_path: &Path,
    settings: &ImageSettings,
) -> Result<u64> {
    let mut buffer = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut buffer),
        image::ImageFormat::Png,
    )?;

    let mut options = oxipng::Options::default();
    if settings.png_compression >= 8 {
        options = oxipng::Options::max_compression();
    }
    options.interlace = if settings.progressive { Some(1) } else { None };

    let output = oxipng::optimize_from_memory(&buffer, &options)?;

    let mut file = File::create(output_path)?;
    file.write_all(&output)?;

    Ok(output.len() as u64)
}

fn compress_webp(
    img: &image::DynamicImage,
    output_path: &Path,
    settings: &ImageSettings,
) -> Result<u64> {
    let rgba = img.to_rgba8();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
    let webp_data = if settings.webp_lossless {
        encoder.encode_lossless()
    } else {
        encoder.encode(settings.quality as f32)
    };

    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(webp_data.as_ptr(), webp_data.len()) };
    let mut file = File::create(output_path)?;
    file.write_all(bytes)?;

    Ok(bytes.len() as u64)
}

fn compress_gif(
    img: &image::DynamicImage,
    output_path: &Path,
    _settings: &ImageSettings,
) -> Result<u64> {
    img.write_to(
        &mut std::io::BufWriter::new(File::create(output_path)?),
        image::ImageFormat::Gif,
    )?;
    Ok(output_path.metadata()?.len())
}

fn compress_tiff(
    img: &image::DynamicImage,
    output_path: &Path,
    _settings: &ImageSettings,
) -> Result<u64> {
    img.write_to(
        &mut std::io::BufWriter::new(File::create(output_path)?),
        image::ImageFormat::Tiff,
    )?;
    Ok(output_path.metadata()?.len())
}

fn compress_bmp(
    img: &image::DynamicImage,
    output_path: &Path,
    _settings: &ImageSettings,
) -> Result<u64> {
    img.write_to(
        &mut std::io::BufWriter::new(File::create(output_path)?),
        image::ImageFormat::Bmp,
    )?;
    Ok(output_path.metadata()?.len())
}

fn compress_tga(
    img: &image::DynamicImage,
    output_path: &Path,
    _settings: &ImageSettings,
) -> Result<u64> {
    img.write_to(
        &mut std::io::BufWriter::new(File::create(output_path)?),
        image::ImageFormat::Tga,
    )?;
    Ok(output_path.metadata()?.len())
}
