# Agents

## Build & Run
```bash
cargo build     # Build
cargo run       # Run
cargo check     # Type-check
cargo test      # Run 61 tests (56 unit + 5 integration)
cargo clippy    # Lint
```

## Architecture
```
src/
├── main.rs        # Entry point, App struct, event loop
├── lib.rs         # Library interface for integration tests
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
  - JPEG: Format → Quality → Color → EXIF → MaxWidth → MaxHeight → Overwrite → Backup → OutputDir → Format
  - WebP: Format → WebP (Lossy/Lossless) → Quality (only if Lossy) → Color → EXIF → MaxWidth → MaxHeight → Overwrite → Backup → OutputDir → Format
  - PNG: Format → Quality → Color → EXIF → Progressive → PNG Comp → MaxWidth → MaxHeight → Overwrite → Backup → OutputDir → Format
  - Other (GIF/TIFF/BMP/TGA/Same): Format → Color → EXIF → MaxWidth → MaxHeight → Overwrite → Backup → OutputDir → Format

## Testing
- 61 tests run via `cargo test` (56 unit + 5 integration)
- Integration tests verify JPEG/PNG/WebP compression and format conversion
- CI configured via `.github/workflows/ci.yml`
  - Runs tests and clippy on push/PR to main/master