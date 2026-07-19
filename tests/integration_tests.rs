use image::ImageFormat;
use std::fs;
use std::path::PathBuf;

fn create_rgb_test_image(
    path: &PathBuf,
    format: ImageFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 0]));
    img.save_with_format(path, format)?;
    Ok(())
}

fn create_rgba_test_image(
    path: &PathBuf,
    format: ImageFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 0, 0, 255]));
    img.save_with_format(path, format)?;
    Ok(())
}

fn get_file_size(path: &PathBuf) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

mod integration {
    use super::*;

    #[test]
    fn test_jpeg_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_jpeg");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.jpg");
        let output_path = temp_dir.join("test_output.jpg");

        create_rgb_test_image(&input_path, ImageFormat::Jpeg).unwrap();

        let input_size = get_file_size(&input_path);
        assert!(input_size > 0, "Input file should exist and have size");

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Jpeg,
            85,
            false,
        );

        assert!(result.is_ok(), "Compression should succeed");
        assert!(output_path.exists(), "Output file should exist");

        let output_size = get_file_size(&output_path);
        assert!(output_size > 0, "Output file should have size");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_png_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_png");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.png");
        let output_path = temp_dir.join("test_output.png");

        create_rgba_test_image(&input_path, ImageFormat::Png).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Png,
            85,
            false,
        );

        assert!(result.is_ok(), "PNG compression should succeed");
        assert!(output_path.exists(), "Output PNG file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_webp_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_webp");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.webp");
        let output_path = temp_dir.join("test_output.webp");

        create_rgba_test_image(&input_path, ImageFormat::WebP).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Webp,
            85,
            false,
        );

        assert!(result.is_ok(), "WebP compression should succeed");
        assert!(output_path.exists(), "Output WebP file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_format_conversion_jpeg_to_png() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_convert");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.jpg");
        let output_path = temp_dir.join("test_output.png");

        create_rgb_test_image(&input_path, ImageFormat::Jpeg).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Png,
            85,
            false,
        );

        assert!(result.is_ok(), "Format conversion should succeed");
        assert!(output_path.exists(), "Converted PNG file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_output_directory_creation() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_mkdir");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.jpg");
        let output_dir = temp_dir.join("subdir").join("nested");
        let output_path = output_dir.join("test_output.jpg");

        create_rgb_test_image(&input_path, ImageFormat::Jpeg).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Jpeg,
            85,
            false,
        );

        assert!(
            result.is_ok(),
            "Compression with directory creation should succeed"
        );
        assert!(output_dir.exists(), "Output directory should be created");
        assert!(output_path.exists(), "Output file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_gif_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_gif");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.gif");
        let output_path = temp_dir.join("test_output.gif");

        create_rgba_test_image(&input_path, ImageFormat::Gif).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Gif,
            85,
            false,
        );

        assert!(result.is_ok(), "GIF compression should succeed");
        assert!(output_path.exists(), "Output GIF file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_tiff_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_tiff");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.tiff");
        let output_path = temp_dir.join("test_output.tiff");

        create_rgba_test_image(&input_path, ImageFormat::Tiff).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Tiff,
            85,
            false,
        );

        assert!(result.is_ok(), "TIFF compression should succeed");
        assert!(output_path.exists(), "Output TIFF file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bmp_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_bmp");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.bmp");
        let output_path = temp_dir.join("test_output.bmp");

        create_rgba_test_image(&input_path, ImageFormat::Bmp).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Bmp,
            85,
            false,
        );

        assert!(result.is_ok(), "BMP compression should succeed");
        assert!(output_path.exists(), "Output BMP file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_tga_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_tga");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.tga");
        let output_path = temp_dir.join("test_output.tga");

        create_rgba_test_image(&input_path, ImageFormat::Tga).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Tga,
            85,
            false,
        );

        assert!(result.is_ok(), "TGA compression should succeed");
        assert!(output_path.exists(), "Output TGA file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[cfg(feature = "avif")]
    #[test]
    fn test_avif_compression_produces_valid_file() {
        let temp_dir = std::env::temp_dir().join("tui_img_test_avif");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let input_path = temp_dir.join("test_input.jpg");
        let output_path = temp_dir.join("test_output.avif");

        // AVIF works best with RGB images
        create_rgb_test_image(&input_path, ImageFormat::Jpeg).unwrap();

        let result = tui_img::compress_image_to_path(
            &input_path,
            &output_path,
            tui_img::OutputFormat::Avif,
            85,
            false,
        );

        assert!(result.is_ok(), "AVIF compression should succeed");
        assert!(output_path.exists(), "Output AVIF file should exist");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[cfg(feature = "animation")]
    mod animation {
        use super::*;
        use std::fs::File;
        use std::io::Write;

        fn rgba_pixels(r: u8, g: u8, b: u8, count: usize) -> Vec<u8> {
            let mut v = Vec::with_capacity(count * 4);
            for _ in 0..count {
                v.extend_from_slice(&[r, g, b, 255]);
            }
            v
        }

        fn create_animated_gif(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
            let mut buf = Vec::new();
            {
                let mut encoder = gif::Encoder::new(&mut buf, 100, 100, &[])?;
                let mut f1 = rgba_pixels(255, 0, 0, 100 * 100);
                let mut f2 = rgba_pixels(0, 255, 0, 100 * 100);
                let mut f3 = rgba_pixels(0, 0, 255, 100 * 100);
                encoder.write_frame(&gif::Frame::from_rgba(100, 100, &mut f1))?;
                encoder.write_frame(&gif::Frame::from_rgba(100, 100, &mut f2))?;
                encoder.write_frame(&gif::Frame::from_rgba(100, 100, &mut f3))?;
            }
            let mut file = File::create(path)?;
            file.write_all(&buf)?;
            Ok(())
        }

        fn create_animated_webp(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
            let rgba1 = rgba_pixels(255, 0, 0, 100 * 100);
            let rgba2 = rgba_pixels(0, 255, 0, 100 * 100);
            let rgba3 = rgba_pixels(0, 0, 255, 100 * 100);

            let mut config: webp::WebPConfig = unsafe { std::mem::zeroed() };
            unsafe {
                let ok = libwebp_sys::WebPConfigInitInternal(
                    &mut config,
                    libwebp_sys::WebPPreset::WEBP_PRESET_DEFAULT,
                    75.0,
                    libwebp_sys::WEBP_ENCODER_ABI_VERSION as i32,
                );
                assert_ne!(ok, 0, "WebPConfigInitInternal failed");
                config.method = 4;
                config.pass = 10;
            }
            let mut encoder = webp::AnimEncoder::new(100, 100, &config);
            encoder.add_frame(webp::AnimFrame::new(
                &rgba1,
                webp::PixelLayout::Rgba,
                100,
                100,
                0,
                None,
            ));
            encoder.add_frame(webp::AnimFrame::new(
                &rgba2,
                webp::PixelLayout::Rgba,
                100,
                100,
                100,
                None,
            ));
            encoder.add_frame(webp::AnimFrame::new(
                &rgba3,
                webp::PixelLayout::Rgba,
                100,
                100,
                200,
                None,
            ));
            let webp_data = encoder.try_encode().map_err(|e| format!("Animated WebP encode failed: {:?}", e))?;
            let mut file = File::create(path)?;
            file.write_all(&webp_data)?;
            Ok(())
        }

        fn create_single_frame_gif(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
            let mut buf = Vec::new();
            {
                let mut encoder = gif::Encoder::new(&mut buf, 100, 100, &[])?;
                let mut rgba = rgba_pixels(255, 0, 0, 100 * 100);
                encoder.write_frame(&gif::Frame::from_rgba(100, 100, &mut rgba))?;
            }
            let mut file = File::create(path)?;
            file.write_all(&buf)?;
            Ok(())
        }

        fn create_single_frame_webp(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
            let rgba = rgba_pixels(255, 0, 0, 100 * 100);
            let encoder = webp::Encoder::new(&rgba, webp::PixelLayout::Rgba, 100, 100);
            let webp_data = encoder.encode_lossless();
            let mut file = File::create(path)?;
            file.write_all(&webp_data)?;
            Ok(())
        }

        fn create_animated_apng(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
            let file = File::create(path)?;
            let w = &mut std::io::BufWriter::new(file);

            let mut encoder = png::Encoder::new(w, 100, 100);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_animated(3, 0)?;
            let mut writer = encoder.write_header()?;

            for _ in 0..3 {
                writer.set_frame_delay(1, 10)?;
                let data = rgba_pixels(255, 0, 0, 100 * 100);
                writer.write_image_data(&data)?;
            }
            writer.finish()?;
            Ok(())
        }

        fn create_single_frame_png(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
            let file = File::create(path)?;
            let w = &mut std::io::BufWriter::new(file);

            let mut encoder = png::Encoder::new(w, 100, 100);
            encoder.set_color(png::ColorType::Rgba);
            let mut writer = encoder.write_header()?;
            let data = rgba_pixels(255, 0, 0, 100 * 100);
            writer.write_image_data(&data)?;
            writer.finish()?;
            Ok(())
        }

        fn make_file(path: &std::path::Path) -> tui_img::ImageFile {
            tui_img::ImageFile::new(path.to_path_buf())
        }

        #[test]
        fn test_animated_gif_to_gif() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_gif_to_gif");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.gif");
            create_animated_gif(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated GIF should be detected as animated");

            let output_path = temp_dir.join("output.gif");
            let result = tui_img::compress_image(&file, &output_path, None);
            assert!(result.is_ok(), "Animated GIF → GIF should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output GIF should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_webp_to_gif() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_webp_to_gif");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.webp");
            create_animated_webp(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated WebP should be detected as animated");

            let output_path = temp_dir.join("output.gif");
            let result = tui_img::compress_image(&file, &output_path, Some(tui_img::OutputFormat::Gif));
            assert!(result.is_ok(), "Animated WebP → GIF should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output GIF should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_webp_to_apng() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_webp_to_apng");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.webp");
            create_animated_webp(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated WebP should be detected as animated");

            let output_path = temp_dir.join("output.png");
            let result = tui_img::compress_image(&file, &output_path, Some(tui_img::OutputFormat::Png));
            assert!(result.is_ok(), "Animated WebP → APNG should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output APNG should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_apng_to_gif() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_apng_to_gif");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.png");
            create_animated_apng(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated PNG should be detected as animated");

            let output_path = temp_dir.join("output.gif");
            let result = tui_img::compress_image(&file, &output_path, Some(tui_img::OutputFormat::Gif));
            assert!(result.is_ok(), "Animated APNG → GIF should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output GIF should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_gif_to_png_first_frame() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_gif_to_png");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.gif");
            create_animated_gif(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated GIF should be detected as animated");

            let output_path = temp_dir.join("output.png");
            let result = tui_img::compress_image(&file, &output_path, Some(tui_img::OutputFormat::Png));
            assert!(result.is_ok(), "Animated GIF → PNG should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output PNG should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_webp_to_webp_same_format() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_webp_to_webp");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.webp");
            create_animated_webp(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated WebP should be detected as animated");

            let output_path = temp_dir.join("output.webp");
            let result = tui_img::compress_image(&file, &output_path, None);
            assert!(result.is_ok(), "Animated WebP → Same (WebP) should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output WebP should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_gif_to_jpeg_first_frame() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_gif_to_jpeg");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("test_animated.gif");
            create_animated_gif(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(file.is_animated, "Animated GIF should be detected as animated");

            let output_path = temp_dir.join("output.jpg");
            let result = tui_img::compress_image(&file, &output_path, Some(tui_img::OutputFormat::Jpeg));
            assert!(result.is_ok(), "Animated GIF → JPEG should succeed: {:?}", result.err());

            let (_, output_name) = result.unwrap();
            let output_file = temp_dir.join(&output_name);
            assert!(output_file.exists(), "Output JPEG should exist");
            assert!(get_file_size(&output_file) > 0, "Output should have size > 0");

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_animated_gif_with_resize() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_anim_resize");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("big_animated.gif");
            create_animated_gif(&input_path).unwrap();

            let mut file = make_file(&input_path);
            file.settings.max_width = Some(50);
            file.settings.max_height = Some(50);

            let output_path = temp_dir.join("output.gif");
            let result = tui_img::compress_image(&file, &output_path, None);
            assert!(result.is_ok(), "Animated GIF with resize should succeed: {:?}", result.err());

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_single_frame_gif_compress() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_single_gif_compress");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("single_frame.gif");
            create_single_frame_gif(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(!file.is_animated, "Single-frame GIF should not be detected as animated");

            let output_path = temp_dir.join("output.gif");
            let result = tui_img::compress_image(&file, &output_path, None);
            assert!(result.is_ok(), "Single-frame GIF compression should succeed: {:?}", result.err());

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_single_frame_webp_compress() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_single_webp_compress");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("single_frame.webp");
            create_single_frame_webp(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(!file.is_animated, "Single-frame WebP should not be detected as animated");

            let output_path = temp_dir.join("output.webp");
            let result = tui_img::compress_image(&file, &output_path, None);
            assert!(result.is_ok(), "Single-frame WebP compression should succeed: {:?}", result.err());

            let _ = fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_single_frame_png_compress() {
            let temp_dir = std::env::temp_dir().join("tui_img_test_single_png_compress");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let input_path = temp_dir.join("single_frame.png");
            create_single_frame_png(&input_path).unwrap();

            let file = make_file(&input_path);
            assert!(!file.is_animated, "Single-frame PNG should not be detected as animated");

            let output_path = temp_dir.join("output.png");
            let result = tui_img::compress_image(&file, &output_path, None);
            assert!(result.is_ok(), "Single-frame PNG compression should succeed: {:?}", result.err());

            let _ = fs::remove_dir_all(&temp_dir);
        }
    }
}
