pub mod cache;
pub mod compression;
pub mod models;

#[cfg(feature = "animation")]
pub use compression::assemble_frames;
pub use compression::{
    apply_processing, compress_bmp, compress_gif, compress_image, compress_image_to_path,
    compress_jpeg, compress_png, compress_tga, compress_tiff, compress_webp,
};
#[cfg(feature = "animation")]
pub use models::detect_animation;
pub use models::{
    path_to_tilde, CachedImageInfo, ColorSpace, ExifData, ImageFile, ImageSettings, OutputFormat,
};
