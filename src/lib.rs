pub mod cache;
pub mod compression;
pub mod models;

pub use compression::{
    apply_processing, compress_bmp, compress_gif, compress_image_to_path, compress_jpeg,
    compress_png, compress_tga, compress_tiff, compress_webp,
};
#[cfg(feature = "animation")]
pub use compression::extract_frames_to_path;
pub use models::{
    path_to_tilde, CachedImageInfo, ColorSpace, ExifData, ImageFile, ImageSettings, OutputFormat,
};
#[cfg(feature = "animation")]
pub use models::detect_animation;
