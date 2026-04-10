# Agents

## Build & Run
```bash
cargo build     # Build the project
cargo run       # Run the image compressor
cargo check     # Type-check without full build
cargo test      # Run tests
cargo clippy    # Run linter
```

## Project Structure
- Package name: `tui-img`
- Entry point: TUI file explorer for batch image compression

## Dependencies
- `ratatui` (0.26) - TUI framework
- `image` - Image decoding/encoding
- `oxipng` - PNG optimization
- `webp` - WebP encoding
- `kamadak-exif` - EXIF metadata
- `walkdir` - Directory traversal
- `crossterm` - Terminal input
- `anyhow` - Error handling
- `rayon` - Parallel processing
- `memmap2` - Memory-mapped file reading

## Keyboard Shortcuts
| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate file list / settings options |
| `Enter` | Open directory / enter folder |
| `Backspace` | Go up one directory |
| `Space` | Toggle file in/out of queue |
| `Tab` | Switch between Files, Image Settings, and Output columns |
| `←/→` | Change setting value |
| `c` | Compress queued files |
| `C` | Clear queue |
| `q` | Quit |
| `PgUp/PgDown` | Page up/down in file list |
| `Home/End` | Jump to first/last file |

## Global Settings (Image Settings column, Tab to access)
| Setting | Description | Values |
|---------|-------------|--------|
| Format | Output file format (global) | Same, JPEG, PNG, WebP, GIF, TIFF, BMP, TGA |
| WebP | Encoding mode (global) | Lossy, Lossless |
| Output Dir | Custom output directory (global) | Path (supports ~) |

## Per-File Settings
| Setting | Description | Values |
|---------|-------------|--------|
| Quality | Compression quality | 0-100 |
| Color | Color space conversion | RGB, Grayscale, RGBA |
| EXIF | Metadata handling | Remove, Keep |
| Progressive | PNG interlacing | Yes, No |
| PNG Comp | Compression level | 0-9 |
| Max Width | Resize max width | None or pixels |
| Max Height | Resize max height | None or pixels |
| Lock Aspect Ratio | Keep aspect ratio when resizing | Yes, No |
| Overwrite | File behavior | Overwrite, New file |
| Backup | Create backup first | Yes, No (planned) |

## UI Layout
- **Header**: Title bar with cyan accent
- **Main**: File list (left ~60%), Image Settings + Output panels (right)
- **Status bar**: Keyboard shortcuts

## Features
- File browser with directory navigation (↑/↓, Enter, Backspace)
- Tab-based column switching (Files/ImageSettings/Output)
- Global settings (Format, WebP mode, Output Dir) and per-file settings
- Batch compression: JPEG, PNG, WebP, GIF, TIFF, BMP, TGA
- Format conversion between all supported formats
- Resize images with max width/height
- PNG interlacing (progressive display)
- WebP lossless encoding option
- Output directory with auto-creation
- Lock aspect ratio for resizing
- Auto-save unique filenames for duplicates

## Notes
- 66 unit tests (`cargo test`)
- No CI/CD configured
- Images are re-encoded without EXIF when enabled (strips all metadata)
- Max width/height uses Lanczos3 resampling
- Output directory is created automatically if it doesn't exist
- Supports `~` in Output Dir path (expands to home directory)
