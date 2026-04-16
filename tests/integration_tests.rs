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
}
