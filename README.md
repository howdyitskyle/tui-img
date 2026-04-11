# TUI Image Tool

A powerful terminal-based image batch compression and conversion tool built with Rust.

![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

## Features

- **Batch Compression** - Compress multiple images at once with configurable settings
- **Format Conversion** - Convert between JPEG, PNG, WebP, GIF, TIFF, BMP, and TGA formats
- **EXIF Management** - Keep or remove image metadata
- **Resize Images** - Set maximum width/height with Lanczos3 resampling
- **Virtual Scrolling** - Handle directories with thousands of files
- **Metadata Caching** - Fast directory navigation with cached metadata
- **Parallel Processing** - Uses rayon for parallel EXIF loading and directory scanning
- **Auto Unique Filenames** - Creates `file_2.ext` when filenames would conflict
- **Auto Directory Creation** - Output directories are created automatically if they don't exist
- **Smart Settings Navigation** - Arrow keys in Image Settings panel automatically skip irrelevant options based on output format

## Supported Formats

- **Input**: JPEG, PNG, WebP, GIF, TIFF, BMP, TGA
- **Output**: JPEG, PNG, WebP, GIF, TIFF, BMP, TGA (or keep original format)
- **AVIF**: Available with `--features avif` (requires NASM for compilation)

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/howdyitskyle/tui-img.git
cd tui-img

# Build release version
cargo build --release

# Run
./target/release/tui-img
```

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

## Usage

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Navigate file list |
| `Enter` | Open directory / Enter folder |
| `Backspace` | Go up one directory |
| `Space` | Toggle file in/out of queue |
| `Tab` | Switch between Files, Settings, and Output columns |
| `PgUp` / `PgDown` | Page up/down in file list |
| `Home` / `End` | Jump to first/last file |

### Settings

Navigate to the Settings column and use:

| Key | Action |
|-----|--------|
| `←` / `→` | Change setting value |

#### Available Settings

| Setting | Description | Values |
|---------|-------------|--------|
| Format | Output file format | Same, JPEG, PNG, WebP, GIF, TIFF, BMP, TGA |
| Quality | Compression quality | 0-100 (JPEG, WebP, TIFF) |
| Color | Color space conversion | RGB, Grayscale, RGBA |
| EXIF | Metadata handling | Remove, Keep |
| Progressive | PNG interlacing | Yes, No |
| PNG Comp | Compression level | 0-9 |
| WebP | Encoding mode | Lossy, Lossless |
| Max Width | Resize max width | None or pixels |
| Max Height | Resize max height | None or pixels |
| Output | File behavior | Overwrite, New file |
| Backup | Create backup first | Yes, No |
| Output Dir | Custom output directory | Path |

### Compression

| Key | Action |
|-----|--------|
| `c` | Compress queued files |
| `C` | Clear queue |
| `q` | Quit |

## Configuration

Default settings are applied automatically. Navigate to the Settings panel to customize compression options per file or globally.

## Performance

The application includes several performance optimizations:

- **Parallel Directory Loading** - Uses rayon to scan directories concurrently
- **Memory-Mapped File Reading** - Uses mmap for fast metadata extraction
- **Metadata Caching** - Cached dimensions and color type for instant re-navigation
- **Virtual Scrolling** - Only renders visible rows, handles 10,000+ files smoothly
- **Parallel EXIF Loading** - Preloads EXIF data in background while browsing
- **Auto File Refresh** - File list refreshes automatically after compression when output is "Same as source"

## Architecture

```
src/
├── main.rs      # Entry point, App struct, event handling
├── models.rs    # Data structures and helper functions
├── cache.rs     # Metadata and EXIF caching
├── compression.rs # Image compression logic
└── ui.rs        # TUI rendering
```

## Testing

```bash
cargo test
```

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Submit a pull request

## Acknowledgments

Built with:
- [ratatui](https://github.com/ratatui/ratatui) - TUI framework
- [image](https://github.com/image-rs/image) - Image processing
- [oxipng](https://github.com/shssoichiro/oxipng) - PNG optimization
- [webp](https://github.com/nickg/webp) - WebP encoding
- [rayon](https://github.com/rayon-rs/rayon) - Parallel processing
