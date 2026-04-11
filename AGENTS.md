# Agents

## Build & Run
```bash
cargo build     # Build
cargo run       # Run
cargo check     # Type-check
cargo test      # Run 66 tests
cargo clippy    # Lint
```

## Architecture
```
src/
├── main.rs        # Entry point, App struct, event loop
├── models.rs      # Data structures
├── cache.rs       # Metadata/EXIF caching
├── compression.rs # Image encoding
└── ui.rs          # TUI rendering
```

## Key Dependencies
- `ratatui 0.26` - TUI framework
- `image 0.25` - Image encoding with jpeg/png/webp/gif/tiff/bmp/tga features
- `oxipng 4` - PNG optimization
- `webp 0.3` - WebP encoding
- `kamadak-exif 0.5` - EXIF metadata
- `rayon 1.10` - Parallel processing

## Quirks
- Release build uses `panic = "abort"` (no unwinding)
- Images re-encode without EXIF when stripped (all metadata removed)
- Max resize uses Lanczos3 resampling
- Output dir auto-created; `~` expands to home directory
- Auto-unique filenames: `file.ext`, `file_2.ext`, `file_3.ext`, ...
- File list auto-refreshes after compression when output dir = Same as source
- Image Settings panel visible when focused (not just when file selected)
- Image Settings navigation skips irrelevant options based on format:
  - JPEG: Format → Quality → Color → EXIF → MaxWidth...
  - WebP: Format → WebP (Lossy/Lossless) → Quality (only if Lossy) → Color...
  - PNG: Format → Quality → Color → EXIF → Progressive → PNG Comp → MaxWidth...
  - Other (GIF/TIFF/BMP/TGA/Same): Format → Quality (if source jpg/webp) → Color → EXIF → MaxWidth...

## Testing
- No integration tests; all 66 unit tests run via `cargo test`
- No CI configured