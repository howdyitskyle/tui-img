use crate::cache::get_unique_path;
use crate::models::{ColorSpace, ImageFile, ImageSettings, OutputFormat};
use anyhow::{Context, Result};
#[cfg(feature = "animation")]
use image::AnimationDecoder;
use oxipng::Interlacing;
use std::fs::{self, File};
use std::io::Write as IoWrite;
use std::path::Path;
#[cfg(feature = "animation")]
use std::path::PathBuf;
#[cfg(feature = "animation")]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "animation")]
static FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "animation")]
fn rand_suffix() -> u64 {
    FRAME_COUNTER.fetch_add(1, Ordering::Relaxed)
}

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
    pub source_name: String,
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
    #[cfg(feature = "animation")]
    if file.is_animated {
        if file.settings.extract_frames {
            return extract_frames_to_output(file, global_format);
        }
        let target_format = global_format.unwrap_or(file.settings.output_format);
        let source_ext = file.extension().unwrap_or_default().to_lowercase();
        let resolved_format = if target_format == OutputFormat::Same {
            match source_ext.as_str() {
                "gif" => OutputFormat::Gif,
                "webp" => OutputFormat::Webp,
                _ => target_format,
            }
        } else {
            target_format
        };
        if matches!(
            resolved_format,
            OutputFormat::Gif | OutputFormat::Webp | OutputFormat::Png
        ) {
            return convert_animated(file, output_path, global_format);
        }
        // APNG→Same: copy original if no settings changes, else re-encode
        if target_format == OutputFormat::Same && source_ext == "png" {
            let has_custom_settings = file.settings.max_width.is_some()
                || file.settings.max_height.is_some()
                || file.settings.color_space != ColorSpace::Rgb;
            if !has_custom_settings {
                let final_output_path = if file.settings.overwrite {
                    output_path.to_path_buf()
                } else {
                    get_unique_path(output_path)
                };
                ensure_dir_exists(&final_output_path)?;
                fs::copy(&file.path, &final_output_path)?;
                let file_size = final_output_path.metadata()?.len();
                let result_name = final_output_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output")
                    .to_string();
                return Ok((file_size, result_name));
            }
            return convert_animated(file, output_path, global_format);
        }
        // Non-animation target: fall through to load first frame
    }

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
        #[cfg(feature = "avif")]
        OutputFormat::Avif => compress_avif(&processed, &final_output_path, &file.settings),
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

pub fn apply_processing(img: image::DynamicImage, settings: &ImageSettings) -> image::DynamicImage {
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

pub fn compress_jpeg(
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

pub fn compress_png(
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
    options.interlace = if settings.progressive {
        Some(Interlacing::Adam7)
    } else {
        Some(Interlacing::None)
    };

    let output = oxipng::optimize_from_memory(&buffer, &options)?;

    let mut file = File::create(output_path)?;
    file.write_all(&output)?;

    Ok(output.len() as u64)
}

pub fn compress_webp(
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

pub fn compress_gif(
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

pub fn compress_tiff(
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

pub fn compress_bmp(
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

pub fn compress_tga(
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

#[cfg(feature = "avif")]
pub fn compress_avif(
    img: &image::DynamicImage,
    output_path: &Path,
    settings: &ImageSettings,
) -> Result<u64> {
    use ravif::{Encoder, Img, RGBA8};

    let rgba = img.to_rgba8();
    let raw = rgba.into_raw();
    let pixels: Vec<RGBA8> = raw
        .chunks_exact(4)
        .map(|c| RGBA8::new(c[0], c[1], c[2], c[3]))
        .collect();
    let img_ref = Img::new(
        pixels.as_slice(),
        img.width() as usize,
        img.height() as usize,
    );

    let quality = settings.quality as f32;
    let encoder = Encoder::new().with_quality(quality);
    let avif = encoder.encode_rgba(img_ref)?;

    let mut file = File::create(output_path)?;
    file.write_all(&avif.avif_file)?;

    Ok(avif.avif_file.len() as u64)
}

#[allow(dead_code)]
pub fn compress_image_to_path(
    input_path: &Path,
    output_path: &Path,
    format: OutputFormat,
    quality: u8,
    webp_lossless: bool,
) -> Result<()> {
    let img = image::open(input_path).context("Failed to open image")?;
    let settings = ImageSettings {
        output_format: format,
        quality,
        color_space: ColorSpace::Rgb,
        remove_exif: true,
        progressive: false,
        png_compression: 6,
        webp_lossless,
        max_width: None,
        max_height: None,
        overwrite: false,
        backup: false,
        output_directory: None,
        extract_frames: false,
    };

    ensure_dir_exists(output_path)?;

    match format {
        OutputFormat::Jpeg => compress_jpeg(&img, output_path, &settings)?,
        OutputFormat::Png => compress_png(&img, output_path, &settings)?,
        OutputFormat::Webp => compress_webp(&img, output_path, &settings)?,
        OutputFormat::Gif => compress_gif(&img, output_path, &settings)?,
        OutputFormat::Tiff => compress_tiff(&img, output_path, &settings)?,
        OutputFormat::Bmp => compress_bmp(&img, output_path, &settings)?,
        OutputFormat::Tga => compress_tga(&img, output_path, &settings)?,
        #[cfg(feature = "avif")]
        OutputFormat::Avif => compress_avif(&img, output_path, &settings)?,
        OutputFormat::Same => anyhow::bail!("Cannot use OutputFormat::Same for compression"),
    };

    Ok(())
}

#[cfg(feature = "animation")]
fn extract_frames_to_path(
    file: &ImageFile,
    base_dir: &Path,
    global_format: Option<OutputFormat>,
) -> Result<(u64, String)> {
    let stem = file.path.file_stem().unwrap_or_default().to_string_lossy();
    let frames_dir = base_dir.join(format!("{}_frames", stem));
    ensure_dir_exists(&frames_dir.join("placeholder"))?;

    let target_format = global_format.unwrap_or(file.settings.output_format);
    let output_ext = match target_format {
        OutputFormat::Same => file.extension().unwrap_or_else(|| "png".to_string()),
        _ => target_format.extension().to_string(),
    };

    let file_vec = fs::read(&file.path).context("Failed to read file for frame extraction")?;
    let source_ext = file.extension().unwrap_or_default().to_lowercase();

    let frame_count = match source_ext.as_str() {
        "gif" => extract_gif_frames(&file_vec, &frames_dir, &output_ext, &file.settings)?,
        "webp" => extract_webp_frames(&file_vec, &frames_dir, &output_ext, &file.settings)?,
        "png" => extract_apng_frames(&file_vec, &frames_dir, &output_ext, &file.settings)?,
        _ => anyhow::bail!("Frame extraction not supported for .{}", source_ext),
    };

    let output_name = format!("{}/", frames_dir.display());
    Ok((frame_count, output_name))
}

#[cfg(feature = "animation")]
fn extract_frames_to_output(
    file: &ImageFile,
    global_format: Option<OutputFormat>,
) -> Result<(u64, String)> {
    let stem = file.path.file_stem().unwrap_or_default().to_string_lossy();
    let parent = file.path.parent().unwrap_or(Path::new("."));
    let source_ext = file.extension().unwrap_or_default().to_lowercase();

    let target_format = global_format.unwrap_or(file.settings.output_format);
    let output_ext = match target_format {
        OutputFormat::Same => file.extension().unwrap_or_else(|| "png".to_string()),
        _ => target_format.extension().to_string(),
    };

    let frames_dir = get_unique_frames_dir(parent, &stem);
    ensure_dir_exists(&frames_dir.join("placeholder"))?;

    let file_vec = fs::read(&file.path).context("Failed to read file for frame extraction")?;

    let _count = match source_ext.as_str() {
        "gif" => extract_gif_frames(&file_vec, &frames_dir, &output_ext, &file.settings)?,
        "webp" => extract_webp_frames(&file_vec, &frames_dir, &output_ext, &file.settings)?,
        "png" => extract_apng_frames(&file_vec, &frames_dir, &output_ext, &file.settings)?,
        _ => anyhow::bail!("Frame extraction not supported for .{}", source_ext),
    };

    let total_bytes: u64 = fs::read_dir(&frames_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();

    let output_name = frames_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("frames")
        .to_string();
    Ok((total_bytes, output_name))
}

#[cfg(feature = "animation")]
fn get_unique_frames_dir(parent: &Path, stem: &str) -> PathBuf {
    let base = parent.join(format!("{}_frames", stem));
    if !base.exists() {
        return base;
    }
    for i in 2.. {
        let candidate = parent.join(format!("{}_frames_{}", stem, i));
        if !candidate.exists() {
            return candidate;
        }
    }
    base
}

#[cfg(feature = "animation")]
fn extract_gif_frames(
    data: &[u8],
    frames_dir: &Path,
    ext: &str,
    settings: &ImageSettings,
) -> Result<u64> {
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = decoder
        .read_info(std::io::Cursor::new(data))
        .context("Failed to decode GIF")?;

    let mut frame_idx: u32 = 0;
    let mut delays: Vec<u32> = Vec::new();
    while let Some(frame) = decoder
        .read_next_frame()
        .context("Failed to read GIF frame")?
    {
        delays.push(frame.delay as u32 * 10);
        frame_idx += 1;
        let rgba_img = image::RgbaImage::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.buffer.to_vec(),
        )
        .context("Failed to create RgbaImage from GIF frame")?;
        let dyn_img = image::DynamicImage::ImageRgba8(rgba_img);
        let processed = apply_processing(dyn_img, settings);
        let frame_path = frames_dir.join(format!("frame_{:03}.{}", frame_idx, ext));
        encode_frame(&processed, &frame_path, ext)?;
    }

    write_delays(frames_dir, &delays)?;
    Ok(frame_idx as u64)
}

#[cfg(feature = "animation")]
fn extract_webp_frames(
    data: &[u8],
    frames_dir: &Path,
    ext: &str,
    settings: &ImageSettings,
) -> Result<u64> {
    let cursor = std::io::Cursor::new(data);
    let decoder = image::codecs::webp::WebPDecoder::new(cursor).context("Failed to decode WebP")?;
    let frames: Vec<image::Frame> = decoder
        .into_frames()
        .collect_frames()
        .context("Failed to collect WebP frames")?;

    if frames.is_empty() {
        let cursor = std::io::Cursor::new(data);
        let img = image::DynamicImage::from_decoder(image::codecs::webp::WebPDecoder::new(cursor)?)
            .context("Failed to decode WebP as static image")?;
        let processed = apply_processing(img, settings);
        let frame_path = frames_dir.join(format!("frame_{:03}.{}", 1, ext));
        encode_frame(&processed, &frame_path, ext)?;
        write_delays(frames_dir, &[100])?;
        return Ok(1);
    }

    let total = frames.len() as u64;
    let mut delays: Vec<u32> = Vec::with_capacity(frames.len());
    for (i, frame) in frames.into_iter().enumerate() {
        let (numer, denom) = frame.delay().numer_denom_ms();
        delays.push(numer.checked_div(denom).unwrap_or(100));
        let rgba_buf = frame.buffer().clone();
        let dyn_img = image::DynamicImage::ImageRgba8(rgba_buf);
        let processed = apply_processing(dyn_img, settings);
        let frame_path = frames_dir.join(format!("frame_{:03}.{}", i + 1, ext));
        encode_frame(&processed, &frame_path, ext)?;
    }

    write_delays(frames_dir, &delays)?;
    Ok(total)
}

#[cfg(feature = "animation")]
fn extract_apng_frames(
    data: &[u8],
    frames_dir: &Path,
    ext: &str,
    settings: &ImageSettings,
) -> Result<u64> {
    let cursor = std::io::Cursor::new(data);
    let decoder = image::codecs::png::PngDecoder::new(cursor).context("Failed to decode PNG")?;

    let frames = match decoder.apng() {
        Ok(apng_decoder) => apng_decoder
            .into_frames()
            .collect_frames()
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    if frames.is_empty() {
        let cursor = std::io::Cursor::new(data);
        let img = image::DynamicImage::from_decoder(image::codecs::png::PngDecoder::new(cursor)?)
            .context("Failed to decode PNG as static image")?;
        let processed = apply_processing(img, settings);
        let frame_path = frames_dir.join(format!("frame_{:03}.{}", 1, ext));
        encode_frame(&processed, &frame_path, ext)?;
        write_delays(frames_dir, &[100])?;
        return Ok(1);
    }

    let total = frames.len() as u64;
    let mut delays: Vec<u32> = Vec::with_capacity(frames.len());
    for (i, frame) in frames.into_iter().enumerate() {
        let (numer, denom) = frame.delay().numer_denom_ms();
        delays.push(numer.checked_div(denom).unwrap_or(100));
        let rgba_buf = frame.buffer().clone();
        let dyn_img = image::DynamicImage::ImageRgba8(rgba_buf);
        let processed = apply_processing(dyn_img, settings);
        let frame_path = frames_dir.join(format!("frame_{:03}.{}", i + 1, ext));
        encode_frame(&processed, &frame_path, ext)?;
    }

    write_delays(frames_dir, &delays)?;
    Ok(total)
}

#[cfg(feature = "animation")]
fn encode_frame(img: &image::DynamicImage, path: &Path, ext: &str) -> Result<()> {
    match ext {
        "png" => {
            img.write_to(
                &mut std::io::BufWriter::new(File::create(path)?),
                image::ImageFormat::Png,
            )?;
        }
        "webp" => {
            let rgba = img.to_rgba8();
            let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
            let webp_data = encoder.encode_lossless();
            let bytes: &[u8] =
                unsafe { std::slice::from_raw_parts(webp_data.as_ptr(), webp_data.len()) };
            let mut file = File::create(path)?;
            file.write_all(bytes)?;
        }
        "gif" => {
            img.write_to(
                &mut std::io::BufWriter::new(File::create(path)?),
                image::ImageFormat::Gif,
            )?;
        }
        _ => {
            img.write_to(
                &mut std::io::BufWriter::new(File::create(path)?),
                image::ImageFormat::Png,
            )?;
        }
    }
    Ok(())
}

#[cfg(feature = "animation")]
fn resolve_output_path(output_path: &Path, ext: &str, settings: &ImageSettings) -> PathBuf {
    let base_output_path = {
        let out_stem = output_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        output_path.with_file_name(format!("{}.{}", out_stem, ext))
    };
    if settings.overwrite {
        base_output_path
    } else {
        get_unique_path(&base_output_path)
    }
}

#[cfg(feature = "animation")]
pub fn assemble_frames(
    file: &ImageFile,
    output_path: &Path,
    global_format: Option<OutputFormat>,
) -> Result<(u64, String)> {
    let frames_dir = &file.path;
    if !frames_dir.is_dir() {
        anyhow::bail!("Frames path is not a directory: {:?}", frames_dir);
    }

    let (frame_paths, delays) = collect_frame_files(frames_dir)?;
    if frame_paths.is_empty() {
        anyhow::bail!("No frame files found in {:?}", frames_dir);
    }

    let target_format = global_format.unwrap_or(file.settings.output_format);
    let resolved_format = match target_format {
        OutputFormat::Png => OutputFormat::Png,
        OutputFormat::Gif => OutputFormat::Gif,
        OutputFormat::Webp => OutputFormat::Webp,
        _ => OutputFormat::Gif,
    };

    let temp_dir = std::env::temp_dir().join(format!(
        "tui_img_assembly_{}_{}",
        std::process::id(),
        rand_suffix()
    ));
    let _ = fs::create_dir_all(&temp_dir);
    let result = (|| -> Result<(u64, String)> {
        let mut processed_paths: Vec<PathBuf> = Vec::new();
        for fp in &frame_paths {
            let img = image::open(fp).context("Failed to open frame for assembly")?;
            let processed = apply_processing(img, &file.settings);
            let out_name = fp.file_stem().unwrap_or_default();
            let out_path = temp_dir.join(out_name).with_extension("png");
            processed.save(&out_path)?;
            processed_paths.push(out_path);
        }

        let out_ext = resolved_format.extension().to_string();
        let final_output_path = resolve_output_path(output_path, &out_ext, &file.settings);
        ensure_dir_exists(&final_output_path)?;

        match resolved_format {
            OutputFormat::Gif => assemble_gif(&processed_paths, &final_output_path, &delays),
            OutputFormat::Webp => assemble_webp(&processed_paths, &final_output_path, &delays),
            OutputFormat::Png => assemble_apng(&processed_paths, &final_output_path, &delays),
            _ => anyhow::bail!("Unsupported assembly format"),
        }
    })();
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

#[cfg(feature = "animation")]
fn convert_animated(
    file: &ImageFile,
    output_path: &Path,
    global_format: Option<OutputFormat>,
) -> Result<(u64, String)> {
    let target_format = global_format.unwrap_or(file.settings.output_format);
    let source_ext = file.extension().unwrap_or_default().to_lowercase();

    let resolved_format = if target_format == OutputFormat::Same {
        match source_ext.as_str() {
            "gif" => OutputFormat::Gif,
            "webp" => OutputFormat::Webp,
            "png" => OutputFormat::Png,
            _ => target_format,
        }
    } else {
        target_format
    };

    let stem = file.path.file_stem().unwrap_or_default().to_string_lossy();

    match resolved_format {
        OutputFormat::Png => {
            let temp_dir = std::env::temp_dir().join(format!(
                "tui_img_frames_{}_{}",
                std::process::id(),
                rand_suffix()
            ));
            let _ = fs::create_dir_all(&temp_dir);
            let result = (|| -> Result<(u64, String)> {
                extract_frames_to_path(file, &temp_dir, Some(OutputFormat::Png))?;
                let frames_dir = temp_dir.join(format!("{}_frames", stem));
                let (frame_paths, delays) = collect_frame_files(&frames_dir)?;
                let final_output_path = resolve_output_path(output_path, "png", &file.settings);
                ensure_dir_exists(&final_output_path)?;
                assemble_apng(&frame_paths, &final_output_path, &delays)
            })();
            let _ = fs::remove_dir_all(&temp_dir);
            result
        }
        OutputFormat::Gif => {
            let temp_dir = std::env::temp_dir().join(format!(
                "tui_img_frames_{}_{}",
                std::process::id(),
                rand_suffix()
            ));
            let _ = fs::create_dir_all(&temp_dir);
            let result = (|| -> Result<(u64, String)> {
                extract_frames_to_path(file, &temp_dir, Some(OutputFormat::Png))?;
                let frames_dir = temp_dir.join(format!("{}_frames", stem));
                let (frame_paths, delays) = collect_frame_files(&frames_dir)?;
                let final_output_path = resolve_output_path(output_path, "gif", &file.settings);
                ensure_dir_exists(&final_output_path)?;
                assemble_gif(&frame_paths, &final_output_path, &delays)
            })();
            let _ = fs::remove_dir_all(&temp_dir);
            result
        }
        OutputFormat::Webp => {
            let temp_dir = std::env::temp_dir().join(format!(
                "tui_img_frames_{}_{}",
                std::process::id(),
                rand_suffix()
            ));
            let _ = fs::create_dir_all(&temp_dir);
            let result = (|| -> Result<(u64, String)> {
                extract_frames_to_path(file, &temp_dir, Some(OutputFormat::Png))?;
                let frames_dir = temp_dir.join(format!("{}_frames", stem));
                let (frame_paths, delays) = collect_frame_files(&frames_dir)?;
                let final_output_path = resolve_output_path(output_path, "webp", &file.settings);
                ensure_dir_exists(&final_output_path)?;
                assemble_webp(&frame_paths, &final_output_path, &delays)
            })();
            let _ = fs::remove_dir_all(&temp_dir);
            result
        }
        _ => anyhow::bail!("convert_animated called with non-animation format"),
    }
}

#[cfg(feature = "animation")]
fn collect_frame_files(dir: &Path) -> Result<(Vec<PathBuf>, Vec<u32>)> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .context("Failed to read frames directory")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext, "png" | "webp" | "gif"))
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();
    paths.sort();
    let delays = read_delays(dir).unwrap_or_else(|| vec![100; paths.len()]);
    Ok((paths, delays))
}

#[cfg(feature = "animation")]
const DELAYS_FILE: &str = "delays.txt";

#[cfg(feature = "animation")]
fn write_delays(frames_dir: &Path, delays: &[u32]) -> Result<()> {
    let content: String = delays.iter().map(|d| format!("{}\n", d)).collect();
    fs::write(frames_dir.join(DELAYS_FILE), content)?;
    Ok(())
}

#[cfg(feature = "animation")]
fn read_delays(frames_dir: &Path) -> Option<Vec<u32>> {
    let content = fs::read_to_string(frames_dir.join(DELAYS_FILE)).ok()?;
    Some(
        content
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
            .collect(),
    )
}

#[cfg(feature = "animation")]
fn assemble_gif(
    frame_paths: &[PathBuf],
    output_path: &Path,
    delays: &[u32],
) -> Result<(u64, String)> {
    use std::io::BufWriter;

    if frame_paths.is_empty() {
        anyhow::bail!("No frames to assemble");
    }

    let first_img =
        image::open(&frame_paths[0]).context("Failed to open first frame for GIF assembly")?;
    let width = first_img.width() as u16;
    let height = first_img.height() as u16;

    let file = File::create(output_path)?;
    let mut buf_writer = BufWriter::new(file);
    let mut encoder = gif::Encoder::new(&mut buf_writer, width, height, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;

    for (frame_path, &delay_ms) in frame_paths.iter().zip(delays.iter()) {
        let img = image::open(frame_path).context("Failed to open frame for GIF assembly")?;
        let resized = if img.width() as u16 != width || img.height() as u16 != height {
            img.resize_exact(
                width as u32,
                height as u32,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        };
        let rgba = resized.to_rgba8();
        let mut frame = gif::Frame::from_rgba(width, height, &mut rgba.into_raw());
        frame.delay = (delay_ms / 10).max(1) as u16;
        encoder.write_frame(&frame)?;
    }

    drop(encoder);
    buf_writer.flush()?;
    drop(buf_writer);

    let file_size = output_path.metadata()?.len();
    let result_name = output_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output.gif")
        .to_string();

    Ok((file_size, result_name))
}

#[cfg(feature = "animation")]
fn assemble_webp(
    frame_paths: &[PathBuf],
    output_path: &Path,
    delays: &[u32],
) -> Result<(u64, String)> {
    if frame_paths.is_empty() {
        anyhow::bail!("No frames to assemble");
    }

    let first_img =
        image::open(&frame_paths[0]).context("Failed to open first frame for WebP assembly")?;
    let width = first_img.width();
    let height = first_img.height();

    let mut config =
        webp::WebPConfig::new().map_err(|_| anyhow::anyhow!("Failed to init WebP config"))?;
    config.method = 4;
    config.pass = 10;

    let mut encoder = webp::AnimEncoder::new(width, height, &config);

    let mut owned_frames: Vec<(Vec<u8>, u32, u32)> = Vec::new();
    for frame_path in frame_paths {
        let img = image::open(frame_path).context("Failed to open frame for WebP assembly")?;
        let rgba = img.to_rgba8();
        owned_frames.push((rgba.into_raw(), width, height));
    }

    let mut timestamp: i32 = 0;
    for (i, (rgba_data, w, h)) in owned_frames.iter().enumerate() {
        let delay_ms = delays.get(i).copied().unwrap_or(100) as i32;
        encoder.add_frame(webp::AnimFrame::new(
            rgba_data,
            webp::PixelLayout::Rgba,
            *w,
            *h,
            timestamp,
            None,
        ));
        timestamp += delay_ms;
    }

    let webp_data = encoder
        .try_encode()
        .map_err(|e| anyhow::anyhow!("Animated WebP encode failed: {:?}", e))?;
    let mut file = File::create(output_path)?;
    file.write_all(&webp_data)?;

    let file_size = output_path.metadata()?.len();
    let result_name = output_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output.webp")
        .to_string();

    Ok((file_size, result_name))
}

#[cfg(feature = "animation")]
fn assemble_apng(
    frame_paths: &[PathBuf],
    output_path: &Path,
    delays: &[u32],
) -> Result<(u64, String)> {
    use png::{BitDepth, BlendOp, ColorType, DisposeOp};

    if frame_paths.is_empty() {
        anyhow::bail!("No frames to assemble");
    }

    let first_img =
        image::open(&frame_paths[0]).context("Failed to open first frame for APNG assembly")?;
    let width = first_img.width();
    let height = first_img.height();

    let file = File::create(output_path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_animated(frame_paths.len() as u32, 0)?;
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);

    let mut writer = encoder.write_header()?;

    for (i, frame_path) in frame_paths.iter().enumerate() {
        let img = image::open(frame_path).context("Failed to open frame for APNG assembly")?;
        let rgba = img.to_rgba8();

        let delay_ms = delays.get(i).copied().unwrap_or(100);
        let num = delay_ms.min(u16::MAX as u32).max(1) as u16;
        let denom = 1000u16;

        writer.set_frame_delay(num, denom)?;
        writer.set_dispose_op(DisposeOp::None)?;
        writer.set_blend_op(BlendOp::Source)?;
        writer.write_image_data(rgba.as_raw())?;
    }

    writer.finish()?;

    let file_size = output_path.metadata()?.len();
    let result_name = output_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output.png")
        .to_string();

    Ok((file_size, result_name))
}
