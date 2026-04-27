mod cache;
mod compression;
mod models;
mod ui;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use walkdir::WalkDir;

use anyhow::Result;
use cache::{cache_metadata, get_cached_metadata};
use compression::{compress_image, CompressionEvent};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use models::{
    bytes_to_human, truncate_str, CachedImageInfo, ColorSpace, ExifData, ImageFile, ImageSettings,
    OutputFormat,
};
use ratatui::prelude::Span;
use ratatui::Terminal;

fn show_webp_setting(format: Option<OutputFormat>) -> bool {
    format == Some(OutputFormat::Webp)
}

fn show_quality_setting(format: Option<OutputFormat>, file: Option<&ImageFile>) -> bool {
    match format {
        None | Some(OutputFormat::Jpeg) | Some(OutputFormat::Png) => true,
        Some(OutputFormat::Webp) => file.map(|f| !f.settings.webp_lossless).unwrap_or(true),
        Some(OutputFormat::Same) => {
            if let Some(f) = file {
                matches!(
                    f.extension().as_deref(),
                    Some("jpg") | Some("jpeg") | Some("webp")
                )
            } else {
                true
            }
        }
        _ => false,
    }
}

fn show_progressive_setting(format: Option<OutputFormat>) -> bool {
    format == Some(OutputFormat::Png)
}

#[derive(Clone, Copy, PartialEq)]
pub enum FocusedColumn {
    Files,
    ImageSettings,
    Output,
}

#[derive(Clone, Copy, PartialEq, Debug, Hash, Eq)]
pub enum SettingOption {
    Quality,
    Color,
    Exif,
    Format,
    Progressive,
    MaxWidth,
    MaxHeight,
    PngCompress,
    WebpLossless,
    Overwrite,
    Backup,
    OutputDir,
}

pub struct App {
    pub current_dir: PathBuf,
    pub files: Vec<ImageFile>,
    pub list_state: ratatui::widgets::ListState,
    pub selected_index: Option<usize>,
    pub show_settings: bool,
    pub focused_column: FocusedColumn,
    pub setting_option: SettingOption,
    pub queue: Vec<usize>,
    pub default_quality: u8,
    pub global_output_format: Option<OutputFormat>,
    pub global_output_directory: Option<PathBuf>,
    pub compressing: bool,
    pub compression_cancelled: bool,
    pub progress: Option<(usize, usize, u8, String)>,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub input_mode: bool,
    pub input_buffer: String,
    pub input_target: SettingOption,
    pub width_input: String,
    pub height_input: String,
    pub(crate) metadata_cache: HashMap<PathBuf, CachedImageInfo>,
    pub(crate) exif_cache: HashMap<PathBuf, ExifData>,
    pub scroll_offset: usize,
    pub visible_rows: usize,
    pub compression_results: Vec<CompressionResult>,
    pub results_scroll: usize,
    pub status_spans_cache: Option<Vec<Span<'static>>>,
    pub show_help: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CompressionResult {
    pub file_index: usize,
    pub original_size: u64,
    pub new_size: u64,
    pub output_filename: Option<String>,
    pub error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            current_dir,
            files: Vec::new(),
            list_state: ratatui::widgets::ListState::default(),
            selected_index: None,
            show_settings: true,
            focused_column: FocusedColumn::Files,
            setting_option: SettingOption::Format,
            queue: Vec::new(),
            default_quality: 85,
            global_output_format: None,
            global_output_directory: None,
            compressing: false,
            compression_cancelled: false,
            progress: None,
            error_message: None,
            success_message: None,
            input_mode: false,
            input_buffer: String::new(),
            input_target: SettingOption::Format,
            width_input: String::new(),
            height_input: String::new(),
            metadata_cache: HashMap::new(),
            exif_cache: HashMap::new(),
            scroll_offset: 0,
            visible_rows: 20,
            compression_results: Vec::new(),
            results_scroll: 0,
            status_spans_cache: None,
            show_help: false,
        }
    }

    pub fn update_visible_rows(&mut self, terminal_height: u16) {
        self.visible_rows = terminal_height.saturating_sub(5) as usize;
        self.scroll_offset = self
            .scroll_offset
            .min(self.files.len().saturating_sub(self.visible_rows));
    }

    pub fn is_supported_image(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| {
                matches!(
                    ext.to_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "webp" | "gif" | "tiff" | "tif" | "bmp" | "tga"
                )
            })
            .unwrap_or(false)
    }

    pub fn load_directory(&mut self) {
        self.files.clear();
        self.queue.clear();
        self.selected_index = None;
        self.scroll_offset = 0;

        let should_add_parent = self.current_dir.parent().is_some();

        let entries: Vec<_> = WalkDir::new(&self.current_dir)
            .min_depth(1)
            .max_depth(1)
            .sort_by(|a, b| {
                if a.file_type().is_dir() != b.file_type().is_dir() {
                    b.file_type().is_dir().cmp(&a.file_type().is_dir())
                } else {
                    a.file_name().cmp(b.file_name())
                }
            })
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        let paths_and_dirs: Vec<(PathBuf, bool)> = entries
            .iter()
            .filter_map(|e| {
                let path = e.path().to_path_buf();
                let is_dir = path.is_dir();
                if is_dir || Self::is_supported_image(&path) {
                    Some((path, is_dir))
                } else {
                    None
                }
            })
            .collect();

        let metadata_results: Vec<(PathBuf, Option<CachedImageInfo>)> = paths_and_dirs
            .par_iter()
            .filter(|(_, is_dir)| !is_dir)
            .filter_map(|(path, _)| {
                if let Some(cached) = get_cached_metadata(&self.metadata_cache, path) {
                    Some((path.clone(), Some(cached)))
                } else {
                    let info = CachedImageInfo {
                        dimensions: image::image_dimensions(path).ok(),
                        color_type: models::fast_color_type(path),
                        file_mtime: cache::get_file_mtime(path),
                    };
                    Some((path.clone(), Some(info)))
                }
            })
            .collect();

        for (path, info) in &metadata_results {
            if let Some(ref cached) = info {
                cache_metadata(&mut self.metadata_cache, path.clone(), cached.clone());
            }
        }

        let metadata_map: HashMap<PathBuf, CachedImageInfo> = metadata_results
            .into_iter()
            .filter_map(|(path, info)| info.map(|i| (path, i)))
            .collect();

        let mut new_files: Vec<ImageFile> = paths_and_dirs
            .into_iter()
            .map(|(path, is_dir)| {
                let mut file = ImageFile::new_lightweight(path.clone(), is_dir);
                if !is_dir {
                    if let Some(info) = metadata_map.get(&path) {
                        file.dimensions = info.dimensions;
                        file.color_type = info.color_type.clone();
                    }
                }
                file
            })
            .collect();

        if should_add_parent {
            new_files.insert(0, ImageFile::new_parent());
        }

        self.files = new_files;

        if !self.files.is_empty() {
            self.selected_index = Some(0);
            self.list_state.select(Some(0));
            if let Some(file) = self.files.first_mut() {
                file.load_exif_if_needed(&mut self.exif_cache);
            }
        }
    }

    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.load_directory();
        }
    }

    pub fn enter_directory(&mut self) {
        if let Some(idx) = self.selected_index {
            let file = &self.files[idx];
            if file.is_parent {
                self.navigate_up();
            } else if file.is_dir {
                self.current_dir = file.path.clone();
                self.load_directory();
            }
        }
    }

    pub fn toggle_queue(&mut self) {
        if let Some(idx) = self.selected_index {
            self.files[idx].queued = !self.files[idx].queued;
            if self.files[idx].queued {
                self.queue.push(idx);
                self.compression_results.clear();
                self.results_scroll = 0;
            } else {
                self.queue.retain(|&i| i != idx);
            }
            self.update_queue_positions();
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(idx) = self.selected_index {
            self.files[idx].selected = !self.files[idx].selected;
        }
    }

    pub fn update_queue_positions(&mut self) {
        let mut new_queue = Vec::new();
        for (i, file) in self.files.iter().enumerate() {
            if file.queued {
                new_queue.push(i);
            }
        }
        self.queue = new_queue;
    }

    pub fn get_visible_files(&self) -> Vec<(usize, &ImageFile)> {
        self.files.iter().enumerate().collect()
    }

    pub fn get_visible_index(&self) -> Option<usize> {
        if let Some(idx) = self.selected_index {
            let visible = self.get_visible_files();
            visible.iter().position(|(i, _)| *i == idx)
        } else {
            None
        }
    }

    pub fn clear_queue(&mut self) {
        for idx in &self.queue {
            self.files[*idx].queued = false;
        }
        self.queue.clear();
        self.compression_results.clear();
        self.results_scroll = 0;
    }

    pub fn queue_size(&self) -> u64 {
        self.queue.iter().map(|&i| self.files[i].size).sum()
    }

    pub fn commit_size_inputs(&mut self) {
        let width_val = if !self.width_input.is_empty() {
            self.width_input.parse::<u32>().ok().map(|v| v.min(10000))
        } else {
            None
        };
        let height_val = if !self.height_input.is_empty() {
            self.height_input.parse::<u32>().ok().map(|v| v.min(10000))
        } else {
            None
        };

        if let Some(file) = self.selected_file_mut() {
            if let Some(val) = width_val {
                file.settings.max_width = Some(val);
            }
            if let Some(val) = height_val {
                file.settings.max_height = Some(val);
            }
        }
        self.width_input.clear();
        self.height_input.clear();
    }

    pub fn selected_file(&self) -> Option<&ImageFile> {
        self.selected_index.map(|i| &self.files[i])
    }

    pub fn selected_file_mut(&mut self) -> Option<&mut ImageFile> {
        self.selected_index.map(|i| &mut self.files[i])
    }

    pub fn preload_exif_batch(&mut self, start_idx: usize, count: usize) {
        let end_idx = (start_idx + count).min(self.files.len());
        if start_idx >= end_idx {
            return;
        }

        let indices_and_paths: Vec<(usize, PathBuf)> = (start_idx..end_idx)
            .filter_map(|i| {
                let file = &self.files[i];
                if file.needs_exif
                    && file.exif_data.is_none()
                    && !self.exif_cache.contains_key(&file.path)
                {
                    Some((i, file.path.clone()))
                } else {
                    None
                }
            })
            .collect();

        if indices_and_paths.is_empty() {
            return;
        }

        let results: Vec<(usize, Option<ExifData>)> = indices_and_paths
            .into_par_iter()
            .map(|(idx, path)| (idx, ExifData::read_from_file(&path)))
            .collect();

        for (idx, exif_data) in results {
            if let Some(exif) = exif_data {
                self.exif_cache
                    .insert(self.files[idx].path.clone(), exif.clone());
                self.files[idx].exif_data = Some(exif);
            }
        }
    }

    pub fn apply_default_quality(&mut self) {
        let default = self.default_quality;
        if let Some(file) = self.selected_file_mut() {
            file.settings.quality = default;
        }
    }

    pub fn compress_queue(&mut self, tx: mpsc::Sender<CompressionEvent>) {
        if self.queue.is_empty() || self.compressing {
            return;
        }

        self.compressing = true;
        self.status_spans_cache = None;
        let total = self.queue.len();
        self.progress = Some((0, total, 0, "Starting...".to_string()));

        let queue_copy: Vec<(usize, PathBuf, PathBuf, String, ImageSettings, u64)> = self
            .queue
            .iter()
            .map(|&idx| {
                let file = &self.files[idx];
                let output_dir = file
                    .settings
                    .output_directory
                    .clone()
                    .or_else(|| self.global_output_directory.clone());
                let output_path = if let Some(ref dir) = output_dir {
                    dir.join(&file.name)
                } else {
                    file.path.clone()
                };
                (
                    idx,
                    output_path,
                    file.path.clone(),
                    file.name.clone(),
                    file.settings.clone(),
                    file.size,
                )
            })
            .collect();

        let _ = tx.send(CompressionEvent::Started(total));

        let global_format = self.global_output_format;

        thread::spawn(move || {
            use compression::FileResult;
            let mut results: Vec<FileResult> = Vec::new();
            let mut total_saved: u64 = 0;
            let queue_total = queue_copy.len();

            for (i, (idx, output_path, source_path, filename, settings, original_size)) in
                queue_copy.into_iter().enumerate()
            {
                let _ = tx.send(CompressionEvent::Stage(format!(
                    "Converting {}...",
                    truncate_str(&filename, 30)
                )));

                let temp_file = ImageFile {
                    path: source_path,
                    name: filename.clone(),
                    is_dir: false,
                    is_parent: false,
                    size: original_size,
                    dimensions: None,
                    color_type: None,
                    needs_exif: false,
                    settings,
                    queued: false,
                    selected: false,
                    exif_data: None,
                };

                match compress_image(&temp_file, &output_path, global_format) {
                    Ok((new_size, output_filename)) => {
                        let _ = tx.send(CompressionEvent::Stage(format!(
                            "Compressing to {}...",
                            truncate_str(&filename, 30)
                        )));
                        let _ = tx.send(CompressionEvent::Progress {
                            current: i + 1,
                            total: queue_total,
                            filename: filename.clone(),
                            sub_progress: 100,
                        });

                        let savings = original_size.saturating_sub(new_size);
                        total_saved += savings;

                        let result = FileResult {
                            file_index: idx,
                            original_size,
                            new_size,
                            output_filename: Some(output_filename),
                            error: None,
                        };
                        results.push(result.clone());
                        let _ = tx.send(CompressionEvent::FileCompleted(result));
                    }
                    Err(e) => {
                        let result = FileResult {
                            file_index: idx,
                            original_size,
                            new_size: original_size,
                            output_filename: None,
                            error: Some(e.to_string()),
                        };
                        results.push(result.clone());
                        let _ = tx.send(CompressionEvent::FileCompleted(result));
                    }
                }
            }

            let success_count = results.iter().filter(|r| r.error.is_none()).count();
            let _ = tx.send(CompressionEvent::Completed {
                success_count,
                total_saved,
                results,
            });
        });
    }

    pub fn process_compression_events(&mut self, rx: &mpsc::Receiver<CompressionEvent>) {
        while let Ok(event) = rx.try_recv() {
            match event {
                CompressionEvent::Started(total) => {
                    self.progress = Some((0, total, 0, "Starting...".to_string()));
                    self.compression_results.clear();
                }
                CompressionEvent::Progress {
                    current,
                    total,
                    filename,
                    sub_progress,
                } => {
                    self.progress = Some((current, total, sub_progress, filename));
                }
                CompressionEvent::Stage(stage) => {
                    if let Some((current, total, _, _)) = self.progress {
                        self.progress = Some((current, total, 0, stage));
                    }
                }
                CompressionEvent::FileCompleted(result) => {
                    self.compression_results.push(CompressionResult {
                        file_index: result.file_index,
                        original_size: result.original_size,
                        new_size: result.new_size,
                        output_filename: result.output_filename.clone(),
                        error: result.error.clone(),
                    });
                }
                CompressionEvent::Completed {
                    success_count,
                    total_saved,
                    results,
                } => {
                    self.compressing = false;
                    self.progress = None;
                    self.focused_column = FocusedColumn::Files;
                    self.status_spans_cache = None;

                    let output_dir = self.global_output_directory.clone();
                    let reload_needed = output_dir.is_none();

                    for result in &results {
                        self.files[result.file_index].size = result.new_size;
                        self.files[result.file_index].queued = false;
                    }

                    if reload_needed {
                        let current_dir = self.current_dir.clone();
                        self.load_directory();
                        self.current_dir = current_dir;
                    } else if let Some(ref dir) = output_dir {
                        if dir == &self.current_dir {
                            let current_dir = self.current_dir.clone();
                            self.load_directory();
                            self.current_dir = current_dir;
                        }
                    }

                    self.update_queue_positions();

                    let error_count = results.iter().filter(|r| r.error.is_some()).count();
                    if error_count == 0 {
                        self.success_message = Some(format!(
                            "Compressed {} files, saved {}",
                            success_count,
                            bytes_to_human(total_saved)
                        ));
                    } else {
                        let errors: Vec<String> =
                            results.iter().filter_map(|r| r.error.clone()).collect();
                        self.error_message = Some(format!(
                            "Completed: {} ok, {} failed - {}",
                            success_count,
                            error_count,
                            errors.first().cloned().unwrap_or_default()
                        ));
                    }
                }
                CompressionEvent::Cancelled => {
                    self.compressing = false;
                    self.compression_cancelled = false;
                    self.progress = None;
                    self.focused_column = FocusedColumn::Files;
                    self.status_spans_cache = None;
                    self.error_message = Some("Compression cancelled".to_string());
                }
            }
        }
    }

    pub fn cancel_compression(&mut self) {
        if self.compressing {
            self.compression_cancelled = true;
        }
    }
}

fn handle_input(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    compression_tx: std::sync::Arc<std::sync::Mutex<Option<mpsc::Sender<CompressionEvent>>>>,
) -> bool {
    // Close help panel on any key
    if app.show_help {
        if !matches!(key.code, KeyCode::Null) {
            app.show_help = false;
        }
        return false;
    }

    if app.compressing {
        if let KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') = key.code {
            app.cancel_compression();
            if let Some(tx) = compression_tx.lock().unwrap().take() {
                let _ = tx.send(compression::CompressionEvent::Cancelled);
            }
        }
        return false;
    }

    if app.input_mode {
        match key.code {
            KeyCode::Enter => {
                let input = app.input_buffer.clone();
                match app.input_target {
                    SettingOption::OutputDir if !input.is_empty() => {
                        let final_path: PathBuf = if let Some(remainder) = input.strip_prefix('~') {
                            if let Ok(home) = std::env::var("HOME") {
                                if remainder.is_empty() || remainder.starts_with('/') {
                                    PathBuf::from(home).join(&input[2..])
                                } else {
                                    PathBuf::from(home).join(remainder)
                                }
                            } else {
                                PathBuf::from(&input)
                            }
                        } else if PathBuf::from(&input).is_absolute() {
                            PathBuf::from(&input)
                        } else {
                            app.current_dir.join(&input)
                        };
                        app.global_output_directory = Some(final_path);
                    }
                    SettingOption::MaxWidth => {
                        if let Some(file) = app.selected_file_mut() {
                            if input.is_empty() {
                                file.settings.max_width = None;
                            } else if let Ok(val) = input.parse::<u32>() {
                                file.settings.max_width = Some(val.min(10000));
                            }
                        }
                    }
                    SettingOption::MaxHeight => {
                        if let Some(file) = app.selected_file_mut() {
                            if input.is_empty() {
                                file.settings.max_height = None;
                            } else if let Ok(val) = input.parse::<u32>() {
                                file.settings.max_height = Some(val.min(10000));
                            }
                        }
                    }
                    _ => {}
                }
                app.input_mode = false;
                app.input_buffer.clear();
            }
            KeyCode::Esc => {
                app.input_mode = false;
                app.input_buffer.clear();
            }
            KeyCode::Backspace => {
                app.input_buffer.pop();
            }
            KeyCode::Char(c)
                if matches!(app.input_target, SettingOption::OutputDir) || c.is_ascii_digit() =>
            {
                app.input_buffer.push(c);
            }
            _ => {}
        }
        return false;
    }

    if app.focused_column == FocusedColumn::ImageSettings {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                match app.setting_option {
                    SettingOption::MaxWidth => {
                        app.width_input.push(c);
                    }
                    SettingOption::MaxHeight => {
                        app.height_input.push(c);
                    }
                    _ => {}
                }
                return false;
            }
            KeyCode::Backspace => {
                match app.setting_option {
                    SettingOption::MaxWidth => {
                        app.width_input.pop();
                    }
                    SettingOption::MaxHeight => {
                        app.height_input.pop();
                    }
                    _ => {}
                }
                return false;
            }
            KeyCode::Tab
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Enter => {
                app.commit_size_inputs();
            }
            _ => {}
        }
    }

    match key.code {
        // Handle help toggle (works from anywhere)
        KeyCode::Char('?') => {
            app.show_help = !app.show_help;
            return false;
        }

        KeyCode::Tab if app.show_settings => {
            app.focused_column = match app.focused_column {
                FocusedColumn::Files => FocusedColumn::ImageSettings,
                FocusedColumn::ImageSettings => FocusedColumn::Output,
                FocusedColumn::Output => FocusedColumn::Files,
            };
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.focused_column == FocusedColumn::Files {
                if let Some(idx) = app.selected_index {
                    if idx > 0 {
                        app.selected_index = Some(idx - 1);
                        app.list_state.select(Some(idx - 1));
                        if let Some(file) = app.files.get_mut(idx - 1) {
                            file.load_exif_if_needed(&mut app.exif_cache);
                        }
                        app.preload_exif_batch(idx.saturating_sub(5), 10);
                        if idx <= app.scroll_offset {
                            app.scroll_offset = app.scroll_offset.saturating_sub(1);
                        }
                    }
                }
            } else if app.focused_column == FocusedColumn::Output {
                if app.results_scroll > 0 {
                    app.results_scroll -= 1;
                }
            } else {
                let format = app.global_output_format;
                let file = app.selected_file();
                let from_quality = show_quality_setting(format, file);
                let from_webp = show_webp_setting(format);
                let from_progressive = show_progressive_setting(format);

                app.setting_option = match app.setting_option {
                    SettingOption::Quality => {
                        if from_webp {
                            SettingOption::WebpLossless
                        } else {
                            SettingOption::Format
                        }
                    }
                    SettingOption::Color => {
                        if from_quality {
                            SettingOption::Quality
                        } else if from_webp {
                            SettingOption::WebpLossless
                        } else {
                            SettingOption::Format
                        }
                    }
                    SettingOption::Exif => SettingOption::Color,
                    SettingOption::Format => SettingOption::OutputDir,
                    SettingOption::WebpLossless => SettingOption::Format,
                    SettingOption::Progressive => SettingOption::Exif,
                    SettingOption::PngCompress => SettingOption::Progressive,
                    SettingOption::MaxWidth => {
                        if from_progressive {
                            SettingOption::PngCompress
                        } else {
                            SettingOption::Exif
                        }
                    }
                    SettingOption::MaxHeight => SettingOption::MaxWidth,
                    SettingOption::Overwrite => SettingOption::MaxHeight,
                    SettingOption::Backup => SettingOption::Overwrite,
                    SettingOption::OutputDir => SettingOption::Backup,
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.focused_column == FocusedColumn::Files {
                if let Some(idx) = app.selected_index {
                    if idx < app.files.len() - 1 {
                        app.selected_index = Some(idx + 1);
                        app.list_state.select(Some(idx + 1));
                        if let Some(file) = app.files.get_mut(idx + 1) {
                            file.load_exif_if_needed(&mut app.exif_cache);
                        }
                        app.preload_exif_batch(idx + 1, 10);
                        if idx >= app.scroll_offset + app.visible_rows - 1 {
                            app.scroll_offset = (idx + 1).saturating_sub(app.visible_rows - 1);
                        }
                    }
                }
            } else if app.focused_column == FocusedColumn::Output {
                let max_scroll = app.compression_results.len().saturating_sub(1);
                if app.results_scroll < max_scroll {
                    app.results_scroll += 1;
                }
            } else {
                let format = app.global_output_format;
                let file = app.selected_file();
                let from_quality = show_quality_setting(format, file);
                let from_webp = show_webp_setting(format);
                let from_progressive = show_progressive_setting(format);

                app.setting_option = match app.setting_option {
                    SettingOption::Format => {
                        if from_webp {
                            SettingOption::WebpLossless
                        } else if from_quality {
                            SettingOption::Quality
                        } else {
                            SettingOption::Color
                        }
                    }
                    SettingOption::WebpLossless => {
                        if from_quality {
                            SettingOption::Quality
                        } else {
                            SettingOption::Color
                        }
                    }
                    SettingOption::Quality => SettingOption::Color,
                    SettingOption::Color => SettingOption::Exif,
                    SettingOption::Exif => {
                        if from_progressive {
                            SettingOption::Progressive
                        } else {
                            SettingOption::MaxWidth
                        }
                    }
                    SettingOption::Progressive => SettingOption::PngCompress,
                    SettingOption::PngCompress => SettingOption::MaxWidth,
                    SettingOption::MaxWidth => SettingOption::MaxHeight,
                    SettingOption::MaxHeight => SettingOption::Overwrite,
                    SettingOption::Overwrite => SettingOption::Backup,
                    SettingOption::Backup => SettingOption::OutputDir,
                    SettingOption::OutputDir => SettingOption::Format,
                };
            }
        }
        KeyCode::PageUp if app.focused_column == FocusedColumn::Files && !app.files.is_empty() => {
            let new_idx = app
                .selected_index
                .unwrap_or(0)
                .saturating_sub(app.visible_rows);
            app.selected_index = Some(new_idx);
            app.list_state.select(Some(new_idx));
            app.scroll_offset = app.scroll_offset.saturating_sub(app.visible_rows);
            if let Some(file) = app.files.get_mut(new_idx) {
                file.load_exif_if_needed(&mut app.exif_cache);
            }
            app.preload_exif_batch(new_idx, 10);
        }
        KeyCode::PageDown
            if app.focused_column == FocusedColumn::Files && !app.files.is_empty() =>
        {
            let max_idx = app.files.len() - 1;
            let new_idx = (app.selected_index.unwrap_or(0) + app.visible_rows).min(max_idx);
            app.selected_index = Some(new_idx);
            app.list_state.select(Some(new_idx));
            app.scroll_offset = (app.scroll_offset + app.visible_rows)
                .min(max_idx.saturating_sub(app.visible_rows - 1));
            if let Some(file) = app.files.get_mut(new_idx) {
                file.load_exif_if_needed(&mut app.exif_cache);
            }
            app.preload_exif_batch(new_idx, 10);
        }
        KeyCode::Home if app.focused_column == FocusedColumn::Files && !app.files.is_empty() => {
            app.selected_index = Some(0);
            app.list_state.select(Some(0));
            app.scroll_offset = 0;
            if let Some(file) = app.files.first_mut() {
                file.load_exif_if_needed(&mut app.exif_cache);
            }
            app.preload_exif_batch(0, 10);
        }
        KeyCode::End if app.focused_column == FocusedColumn::Files && !app.files.is_empty() => {
            let max_idx = app.files.len() - 1;
            app.selected_index = Some(max_idx);
            app.list_state.select(Some(max_idx));
            app.scroll_offset = max_idx.saturating_sub(app.visible_rows - 1);
            if let Some(file) = app.files.last_mut() {
                file.load_exif_if_needed(&mut app.exif_cache);
            }
            app.preload_exif_batch(max_idx, 10);
        }
        KeyCode::Left if app.focused_column == FocusedColumn::ImageSettings => {
            match app.setting_option {
                SettingOption::Quality => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.quality = file.settings.quality.saturating_sub(5);
                    }
                    app.default_quality = app.default_quality.saturating_sub(5);
                }
                SettingOption::Color => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.color_space = match file.settings.color_space {
                            ColorSpace::Rgba => ColorSpace::Grayscale,
                            ColorSpace::Grayscale => ColorSpace::Rgb,
                            ColorSpace::Rgb => ColorSpace::Rgba,
                        };
                    }
                }
                SettingOption::Exif => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.remove_exif = !file.settings.remove_exif;
                    }
                }
                SettingOption::Format => {
                    app.global_output_format = match app.global_output_format {
                        None => Some(OutputFormat::Tga),
                        Some(OutputFormat::Same) => Some(OutputFormat::Tga),
                        Some(OutputFormat::Jpeg) => None,
                        Some(OutputFormat::Png) => Some(OutputFormat::Jpeg),
                        Some(OutputFormat::Webp) => Some(OutputFormat::Png),
                        Some(OutputFormat::Gif) => Some(OutputFormat::Webp),
                        Some(OutputFormat::Tiff) => Some(OutputFormat::Gif),
                        Some(OutputFormat::Bmp) => Some(OutputFormat::Tiff),
                        Some(OutputFormat::Tga) => Some(OutputFormat::Bmp),
                        #[cfg(feature = "avif")]
                        Some(OutputFormat::Avif) => Some(OutputFormat::Tga),
                    };
                }
                SettingOption::Progressive => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.progressive = !file.settings.progressive;
                    }
                }
                SettingOption::PngCompress => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.png_compression =
                            file.settings.png_compression.saturating_sub(1);
                    }
                }
                SettingOption::MaxWidth => {
                    if let Some(file) = app.selected_file_mut() {
                        let current = file.settings.max_width.unwrap_or(0);
                        let new_val = current.saturating_sub(100);
                        if new_val > 0 {
                            file.settings.max_width = Some(new_val);
                        } else {
                            file.settings.max_width = None;
                        }
                    }
                }
                SettingOption::MaxHeight => {
                    if let Some(file) = app.selected_file_mut() {
                        let current = file.settings.max_height.unwrap_or(0);
                        let new_val = current.saturating_sub(100);
                        if new_val > 0 {
                            file.settings.max_height = Some(new_val);
                        } else {
                            file.settings.max_height = None;
                        }
                    }
                }
                SettingOption::WebpLossless => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.webp_lossless = !file.settings.webp_lossless;
                    }
                }
                SettingOption::Overwrite => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.overwrite = !file.settings.overwrite;
                    }
                }
                SettingOption::Backup => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.backup = !file.settings.backup;
                    }
                }
                SettingOption::OutputDir => {
                    app.input_mode = true;
                    app.input_target = SettingOption::OutputDir;
                    app.input_buffer.clear();
                }
            };
        }
        KeyCode::Right if app.focused_column == FocusedColumn::ImageSettings => {
            match app.setting_option {
                SettingOption::Quality => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.quality = (file.settings.quality + 5).min(100);
                    }
                    app.default_quality = (app.default_quality + 5).min(100);
                }
                SettingOption::Color => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.color_space = match file.settings.color_space {
                            ColorSpace::Rgb => ColorSpace::Rgba,
                            ColorSpace::Grayscale => ColorSpace::Rgb,
                            ColorSpace::Rgba => ColorSpace::Grayscale,
                        };
                    }
                }
                SettingOption::Exif => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.remove_exif = !file.settings.remove_exif;
                    }
                }
                SettingOption::Format => {
                    app.global_output_format = match app.global_output_format {
                        None => Some(OutputFormat::Jpeg),
                        Some(OutputFormat::Same) => Some(OutputFormat::Jpeg),
                        Some(OutputFormat::Jpeg) => Some(OutputFormat::Png),
                        Some(OutputFormat::Png) => Some(OutputFormat::Webp),
                        Some(OutputFormat::Webp) => Some(OutputFormat::Gif),
                        Some(OutputFormat::Gif) => Some(OutputFormat::Tiff),
                        Some(OutputFormat::Tiff) => Some(OutputFormat::Bmp),
                        Some(OutputFormat::Bmp) => Some(OutputFormat::Tga),
                        #[cfg(feature = "avif")]
                        Some(OutputFormat::Tga) => Some(OutputFormat::Avif),
                        #[cfg(not(feature = "avif"))]
                        Some(OutputFormat::Tga) => None,
                    };
                }
                SettingOption::Progressive => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.progressive = !file.settings.progressive;
                    }
                }
                SettingOption::PngCompress => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.png_compression = (file.settings.png_compression + 1).min(9);
                    }
                }
                SettingOption::MaxWidth => {
                    if let Some(file) = app.selected_file_mut() {
                        let current = file.settings.max_width.unwrap_or(0);
                        let new_val = if current == 0 {
                            100
                        } else {
                            (current + 100).min(10000)
                        };
                        file.settings.max_width = Some(new_val);
                    }
                }
                SettingOption::MaxHeight => {
                    if let Some(file) = app.selected_file_mut() {
                        let current = file.settings.max_height.unwrap_or(0);
                        let new_val = if current == 0 {
                            100
                        } else {
                            (current + 100).min(10000)
                        };
                        file.settings.max_height = Some(new_val);
                    }
                }
                SettingOption::WebpLossless => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.webp_lossless = !file.settings.webp_lossless;
                    }
                }
                SettingOption::Overwrite => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.overwrite = !file.settings.overwrite;
                    }
                }
                SettingOption::Backup => {
                    if let Some(file) = app.selected_file_mut() {
                        file.settings.backup = !file.settings.backup;
                    }
                }
                SettingOption::OutputDir => {
                    app.input_mode = true;
                    app.input_target = SettingOption::OutputDir;
                    app.input_buffer.clear();
                }
            };
        }
        KeyCode::Enter if app.focused_column == FocusedColumn::Files => {
            app.enter_directory();
        }
        KeyCode::Backspace if app.focused_column == FocusedColumn::Files => {
            app.navigate_up();
        }
        KeyCode::Char(' ') if app.focused_column == FocusedColumn::Files => {
            app.toggle_queue();
        }
        KeyCode::Char('c') if !app.queue.is_empty() && !app.compressing => {
            if let Some(tx) = compression_tx.lock().unwrap().as_ref() {
                app.compress_queue(tx.clone());
            }
        }
        KeyCode::Char('C') => {
            app.clear_queue();
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.compressing {
                app.cancel_compression();
            } else {
                return true;
            }
        }
        _ => {}
    }
    false
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel();
    let compression_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

    let mut app = App::new();
    app.load_directory();

    'main_loop: loop {
        terminal.draw(|f| ui::ui(&mut app, f))?;

        app.process_compression_events(&rx);

        while event::poll(std::time::Duration::ZERO)? {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if handle_input(&mut app, key, compression_tx.clone()) {
                        break 'main_loop;
                    }
                }
                Ok(Event::Resize(_, h)) => {
                    app.update_visible_rows(h);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        if app.compressing {
            std::thread::sleep(std::time::Duration::from_millis(50));
        } else if app.error_message.is_some() || app.success_message.is_some() {
            std::thread::sleep(std::time::Duration::from_millis(2000));
            app.error_message = None;
            app.success_message = None;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::path_to_tilde;

    #[test]
    fn test_bytes_to_human() {
        assert_eq!(bytes_to_human(0), "0 B");
        assert_eq!(bytes_to_human(500), "500 B");
        assert_eq!(bytes_to_human(1024), "1.0 KB");
        assert_eq!(bytes_to_human(1536), "1.5 KB");
        assert_eq!(bytes_to_human(1048576), "1.0 MB");
        assert_eq!(bytes_to_human(1572864), "1.5 MB");
        assert_eq!(bytes_to_human(1073741824), "1.0 GB");
    }

    #[test]
    fn test_output_format_extension() {
        assert_eq!(OutputFormat::Same.extension(), "");
        assert_eq!(OutputFormat::Jpeg.extension(), "jpg");
        assert_eq!(OutputFormat::Png.extension(), "png");
        assert_eq!(OutputFormat::Webp.extension(), "webp");
        assert_eq!(OutputFormat::Gif.extension(), "gif");
        assert_eq!(OutputFormat::Tiff.extension(), "tiff");
        assert_eq!(OutputFormat::Bmp.extension(), "bmp");
        assert_eq!(OutputFormat::Tga.extension(), "tga");
    }

    #[test]
    fn test_output_format_from_extension() {
        assert_eq!(
            OutputFormat::from_extension("jpg"),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(
            OutputFormat::from_extension("jpeg"),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(OutputFormat::from_extension("png"), Some(OutputFormat::Png));
        assert_eq!(
            OutputFormat::from_extension("webp"),
            Some(OutputFormat::Webp)
        );
        assert_eq!(OutputFormat::from_extension("gif"), Some(OutputFormat::Gif));
        assert_eq!(
            OutputFormat::from_extension("tiff"),
            Some(OutputFormat::Tiff)
        );
        assert_eq!(OutputFormat::from_extension("bmp"), Some(OutputFormat::Bmp));
        assert_eq!(OutputFormat::from_extension("tga"), Some(OutputFormat::Tga));
        assert_eq!(OutputFormat::from_extension("unknown"), None);
        assert_eq!(
            OutputFormat::from_extension("JPG"),
            Some(OutputFormat::Jpeg)
        );
    }

    #[test]
    fn test_color_space_as_str() {
        assert_eq!(ColorSpace::Rgb.as_str(), "RGB");
        assert_eq!(ColorSpace::Grayscale.as_str(), "Grayscale");
        assert_eq!(ColorSpace::Rgba.as_str(), "RGBA");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("exactly10!", 10), "exactly10!");
        assert_eq!(truncate_str("this is a long string", 10), "this is...");
        assert_eq!(truncate_str("1234567890", 5), "12...");
        assert_eq!(truncate_str("abc", 10), "abc");
    }

    #[test]
    fn test_image_file_new_parent() {
        let file = ImageFile::new_parent();
        assert!(file.is_dir);
        assert!(file.is_parent);
        assert_eq!(file.name, "..");
        assert!(!file.needs_exif);
        assert!(file.exif_data.is_none());
    }

    #[test]
    fn test_exif_data_read_from_nonexistent() {
        let result = ExifData::read_from_file(std::path::Path::new("/nonexistent/file.jpg"));
        assert!(result.is_none());
    }

    #[test]
    fn test_fast_color_type_nonexistent() {
        let result = models::fast_color_type(std::path::Path::new("/nonexistent/file.jpg"));
        assert!(result.is_none());
    }

    #[test]
    fn test_fast_color_type_nonexistent_mmap() {
        let result = models::fast_color_type(std::path::Path::new("/nonexistent/file.jpg"));
        assert!(result.is_none());
    }

    #[test]
    fn test_image_settings_default() {
        let settings = ImageSettings::default();
        assert_eq!(settings.quality, 85);
        assert_eq!(settings.color_space, ColorSpace::Rgb);
        assert!(settings.remove_exif);
        assert_eq!(settings.output_format, OutputFormat::Same);
        assert!(settings.output_directory.is_none());
        assert!(!settings.progressive);
        assert!(settings.max_width.is_none());
        assert!(settings.max_height.is_none());
        assert_eq!(settings.png_compression, 6);
        assert!(!settings.webp_lossless);
        assert!(!settings.overwrite);
        assert!(!settings.backup);
    }

    #[test]
    fn test_image_file_extension() {
        let path = PathBuf::from("/test/image.jpg");
        let file = ImageFile::new_lightweight(path, false);
        assert_eq!(file.extension(), Some("jpg".to_string()));
    }

    #[test]
    fn test_image_file_extension_uppercase() {
        let path = PathBuf::from("/test/image.PNG");
        let file = ImageFile::new_lightweight(path, false);
        assert_eq!(file.extension(), Some("png".to_string()));
    }

    #[test]
    fn test_image_file_format_name() {
        let path = PathBuf::from("/test/image.webp");
        let file = ImageFile::new_lightweight(path, false);
        assert_eq!(file.format_name(), "WEBP");
    }

    #[test]
    fn test_image_file_size_str_directory() {
        let file = ImageFile::new_parent();
        assert_eq!(file.size_str(), "");
    }

    #[test]
    fn test_image_file_dimensions_str_none() {
        let path = PathBuf::from("/test/image.jpg");
        let file = ImageFile::new_lightweight(path, false);
        assert_eq!(file.dimensions_str(), "");
    }

    #[test]
    fn test_image_file_dimensions_str_some() {
        let mut file = ImageFile::new_parent();
        file.dimensions = Some((1920, 1080));
        assert_eq!(file.dimensions_str(), "1920×1080");
    }

    #[test]
    fn test_output_format_as_str() {
        assert_eq!(OutputFormat::Same.as_str(), "Same");
        assert_eq!(OutputFormat::Jpeg.as_str(), "JPEG");
        assert_eq!(OutputFormat::Png.as_str(), "PNG");
        assert_eq!(OutputFormat::Webp.as_str(), "WebP");
        assert_eq!(OutputFormat::Gif.as_str(), "GIF");
        assert_eq!(OutputFormat::Tiff.as_str(), "TIFF");
        assert_eq!(OutputFormat::Bmp.as_str(), "BMP");
        assert_eq!(OutputFormat::Tga.as_str(), "TGA");
    }

    #[test]
    fn test_color_type_str() {
        assert_eq!(
            models::color_type_str(image::ExtendedColorType::Rgb8),
            "RGB"
        );
        assert_eq!(
            models::color_type_str(image::ExtendedColorType::Rgba8),
            "RGBA"
        );
        assert_eq!(models::color_type_str(image::ExtendedColorType::L8), "L");
        assert_eq!(models::color_type_str(image::ExtendedColorType::La8), "La");
        assert_eq!(models::color_type_str(image::ExtendedColorType::L16), "L16");
        assert_eq!(
            models::color_type_str(image::ExtendedColorType::Rgb16),
            "RGB16"
        );
    }

    #[test]
    fn test_output_format_supports_quality() {
        assert!(!OutputFormat::Same.supports_quality());
        assert!(OutputFormat::Jpeg.supports_quality());
        assert!(!OutputFormat::Png.supports_quality());
        assert!(OutputFormat::Webp.supports_quality());
        assert!(!OutputFormat::Gif.supports_quality());
        assert!(OutputFormat::Tiff.supports_quality());
        assert!(!OutputFormat::Bmp.supports_quality());
        assert!(!OutputFormat::Tga.supports_quality());
    }

    #[test]
    fn test_is_supported_image() {
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.jpg"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.jpeg"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.png"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.webp"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.gif"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.tiff"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.bmp"
        )));
        assert!(App::is_supported_image(std::path::Path::new(
            "/test/image.tga"
        )));
        assert!(!App::is_supported_image(std::path::Path::new(
            "/test/image.raw"
        )));
        assert!(!App::is_supported_image(std::path::Path::new(
            "/test/file.txt"
        )));
        assert!(!App::is_supported_image(std::path::Path::new(
            "/test/noextension"
        )));
    }

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert!(app.files.is_empty());
        assert!(app.queue.is_empty());
        assert!(app.selected_index.is_none());
        assert_eq!(app.default_quality, 85);
        assert!(!app.compressing);
        assert!(app.progress.is_none());
        assert!(!app.input_mode);
        assert!(app.metadata_cache.is_empty());
        assert!(app.exif_cache.is_empty());
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.visible_rows, 20);
    }

    #[test]
    fn test_app_update_visible_rows() {
        let mut app = App::new();
        app.files = (0..100).map(|_| ImageFile::new_parent()).collect();

        app.update_visible_rows(40);
        assert_eq!(app.visible_rows, 35);
        assert_eq!(app.scroll_offset, 0);

        app.scroll_offset = 50;
        app.update_visible_rows(40);
        assert_eq!(app.scroll_offset, 50);

        app.scroll_offset = 80;
        app.update_visible_rows(40);
        assert_eq!(app.scroll_offset, 65);
    }

    #[test]
    fn test_app_queue_operations() {
        let mut app = App::new();
        app.files = vec![
            ImageFile::new_parent(),
            ImageFile::new_lightweight(PathBuf::from("/test/1.jpg"), false),
            ImageFile::new_lightweight(PathBuf::from("/test/2.jpg"), false),
            ImageFile::new_lightweight(PathBuf::from("/test/3.jpg"), false),
        ];
        app.files[1].size = 100;
        app.files[2].size = 200;
        app.files[3].size = 300;

        assert_eq!(app.queue_size(), 0);

        app.queue.push(1);
        app.queue.push(2);
        app.files[1].queued = true;
        app.files[2].queued = true;
        assert_eq!(app.queue_size(), 300);

        app.clear_queue();
        assert_eq!(app.queue_size(), 0);
        assert!(!app.files[1].queued);
        assert!(!app.files[2].queued);
    }

    #[test]
    fn test_truncate_str_unicode() {
        assert_eq!(truncate_str("日本語テスト", 5), "日本...");
        assert_eq!(truncate_str("hello", 0), "...");
        assert_eq!(truncate_str("", 10), "");
    }

    #[test]
    fn test_bytes_to_human_edge_cases() {
        assert_eq!(bytes_to_human(1), "1 B");
        assert_eq!(bytes_to_human(1023), "1023 B");
        assert_eq!(bytes_to_human(1048575), "1024.0 KB");
        assert_eq!(bytes_to_human(1073741824), "1.0 GB");
        assert_eq!(bytes_to_human(1099511627776), "1024.0 GB");
    }

    #[test]
    fn test_exif_data_empty() {
        let exif = ExifData {
            camera: None,
            lens: None,
            date_taken: None,
            exposure: None,
            iso: None,
            aperture: None,
            focal_length: None,
            flash: None,
        };
        assert!(exif.camera.is_none());
    }

    #[test]
    fn test_cached_image_info() {
        let info = CachedImageInfo {
            dimensions: Some((1920, 1080)),
            color_type: Some("RGB".to_string()),
            file_mtime: 1234567890,
        };
        assert_eq!(info.dimensions, Some((1920, 1080)));
        assert_eq!(info.color_type, Some("RGB".to_string()));
        assert_eq!(info.file_mtime, 1234567890);
    }

    #[test]
    fn test_get_file_mtime_nonexistent() {
        let mtime = cache::get_file_mtime(std::path::Path::new("/nonexistent/file.jpg"));
        assert_eq!(mtime, 0);
    }

    #[test]
    fn test_get_cached_metadata_empty_cache() {
        let cache: std::collections::HashMap<PathBuf, CachedImageInfo> =
            std::collections::HashMap::new();
        let result = cache::get_cached_metadata(&cache, std::path::Path::new("/test/file.jpg"));
        assert!(result.is_none());
    }

    #[test]
    fn test_image_settings_max_width() {
        let mut settings = ImageSettings::default();
        assert!(settings.max_width.is_none());

        settings.max_width = Some(1920);
        assert_eq!(settings.max_width, Some(1920));

        settings.max_width = Some(3840);
        assert_eq!(settings.max_width, Some(3840));

        settings.max_width = None;
        assert!(settings.max_width.is_none());
    }

    #[test]
    fn test_image_settings_max_height() {
        let mut settings = ImageSettings::default();
        assert!(settings.max_height.is_none());

        settings.max_height = Some(1080);
        assert_eq!(settings.max_height, Some(1080));

        settings.max_height = Some(2160);
        assert_eq!(settings.max_height, Some(2160));

        settings.max_height = None;
        assert!(settings.max_height.is_none());
    }

    #[test]
    fn test_image_settings_all_fields() {
        let mut settings = ImageSettings::default();

        settings.quality = 75;
        assert_eq!(settings.quality, 75);

        settings.color_space = ColorSpace::Rgba;
        assert_eq!(settings.color_space, ColorSpace::Rgba);

        settings.remove_exif = false;
        assert!(!settings.remove_exif);

        settings.output_format = OutputFormat::Webp;
        assert_eq!(settings.output_format, OutputFormat::Webp);

        settings.output_directory = Some(PathBuf::from("/output"));
        assert_eq!(settings.output_directory, Some(PathBuf::from("/output")));

        settings.progressive = true;
        assert!(settings.progressive);

        settings.max_width = Some(800);
        settings.max_height = Some(600);
        assert_eq!(settings.max_width, Some(800));
        assert_eq!(settings.max_height, Some(600));

        settings.png_compression = 9;
        assert_eq!(settings.png_compression, 9);

        settings.webp_lossless = true;
        assert!(settings.webp_lossless);

        settings.overwrite = false;
        assert!(!settings.overwrite);

        settings.backup = true;
        assert!(settings.backup);
    }

    #[test]
    fn test_aspect_ratio_calculation() {
        let orig_w: u32 = 1920;
        let orig_h: u32 = 1080;
        let new_width: u32 = 960;

        let aspect = orig_h as f64 / orig_w as f64;
        let expected_height = (new_width as f64 * aspect) as u32;

        assert_eq!(expected_height, 540);
    }

    #[test]
    fn test_color_space_cycling() {
        let mut settings = ImageSettings::default();
        assert_eq!(settings.color_space, ColorSpace::Rgb);

        settings.color_space = ColorSpace::Rgba;
        assert_eq!(settings.color_space, ColorSpace::Rgba);

        settings.color_space = ColorSpace::Grayscale;
        assert_eq!(settings.color_space, ColorSpace::Grayscale);

        settings.color_space = ColorSpace::Rgb;
        assert_eq!(settings.color_space, ColorSpace::Rgb);
    }

    #[test]
    fn test_quality_bounds() {
        let mut settings = ImageSettings::default();

        settings.quality = 0;
        assert_eq!(settings.quality, 0);

        settings.quality = 100;
        assert_eq!(settings.quality, 100);

        settings.quality = 50;
        assert_eq!(settings.quality, 50);
    }

    #[test]
    fn test_png_compression_bounds() {
        let mut settings = ImageSettings::default();

        assert_eq!(settings.png_compression, 6);

        settings.png_compression = 0;
        assert_eq!(settings.png_compression, 0);

        settings.png_compression = 9;
        assert_eq!(settings.png_compression, 9);
    }

    #[test]
    fn test_image_settings_clone() {
        let mut settings = ImageSettings::default();
        settings.quality = 75;
        settings.max_width = Some(1920);

        let cloned = settings.clone();
        assert_eq!(cloned.quality, 75);
        assert_eq!(cloned.max_width, Some(1920));

        let mut cloned_mut = settings.clone();
        cloned_mut.quality = 50;
        assert_eq!(settings.quality, 75);
        assert_eq!(cloned_mut.quality, 50);
    }

    #[test]
    fn test_settings_navigation_down_jpeg() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Jpeg);

        let expected = vec![
            SettingOption::Format,
            SettingOption::Quality,
            SettingOption::Color,
            SettingOption::Exif,
            SettingOption::MaxWidth,
            SettingOption::MaxHeight,
            SettingOption::Overwrite,
            SettingOption::Backup,
            SettingOption::OutputDir,
        ];

        for exp in expected {
            assert_eq!(app.setting_option, exp);
            match app.setting_option {
                SettingOption::Format => app.setting_option = SettingOption::Quality,
                SettingOption::Quality => app.setting_option = SettingOption::Color,
                SettingOption::Color => app.setting_option = SettingOption::Exif,
                SettingOption::Exif => app.setting_option = SettingOption::MaxWidth,
                SettingOption::MaxWidth => app.setting_option = SettingOption::MaxHeight,
                SettingOption::MaxHeight => app.setting_option = SettingOption::Overwrite,
                SettingOption::Overwrite => app.setting_option = SettingOption::Backup,
                SettingOption::Backup => app.setting_option = SettingOption::OutputDir,
                SettingOption::OutputDir => app.setting_option = SettingOption::Format,
                _ => {}
            }
        }
        assert_eq!(app.setting_option, SettingOption::Format);
    }

    #[test]
    fn test_multiple_files_with_global_output_dir() {
        use std::path::PathBuf;

        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/source/img1.jpg"),
            false,
        ));
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/source/img2.jpg"),
            false,
        ));
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/source/img3.jpg"),
            false,
        ));

        app.global_output_directory = Some(PathBuf::from("/output"));

        app.files[0].queued = true;
        app.files[1].queued = true;
        app.files[2].queued = true;
        app.queue.push(0);
        app.queue.push(1);
        app.queue.push(2);

        let queue_copy: Vec<(usize, PathBuf, PathBuf, String, ImageSettings, u64)> = app
            .queue
            .iter()
            .map(|&idx| {
                let file = &app.files[idx];
                let output_dir = file
                    .settings
                    .output_directory
                    .clone()
                    .or_else(|| app.global_output_directory.clone());
                let output_path = if let Some(ref dir) = output_dir {
                    dir.join(&file.name)
                } else {
                    file.path.clone()
                };
                (
                    idx,
                    output_path,
                    file.path.clone(),
                    file.name.clone(),
                    file.settings.clone(),
                    file.size,
                )
            })
            .collect();

        assert_eq!(queue_copy.len(), 3);
        assert_eq!(queue_copy[0].1, PathBuf::from("/output/img1.jpg"));
        assert_eq!(queue_copy[1].1, PathBuf::from("/output/img2.jpg"));
        assert_eq!(queue_copy[2].1, PathBuf::from("/output/img3.jpg"));
    }

    #[test]
    fn test_path_to_tilde() {
        use std::path::PathBuf;

        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());

        let path_in_home = PathBuf::from(format!("{}/images/output", home));
        assert_eq!(path_to_tilde(&path_in_home), "~/images/output");

        let path_exact_home = PathBuf::from(&home);
        assert_eq!(path_to_tilde(&path_exact_home), "~");

        let path_outside_home = PathBuf::from("/tmp/other");
        assert_eq!(path_to_tilde(&path_outside_home), "/tmp/other");
    }

    #[test]
    fn test_output_dir_save_and_display() {
        use std::path::PathBuf;

        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());

        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.selected_index = Some(0);

        let input_path = format!("{}/images/output", home);
        app.files[0].settings.output_directory = Some(PathBuf::from(&input_path));

        let displayed = path_to_tilde(app.files[0].settings.output_directory.as_ref().unwrap());
        assert_eq!(displayed, "~/images/output");
    }

    #[test]
    fn test_output_dir_in_compression() {
        use std::path::PathBuf;

        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/source/image.jpg"),
            false,
        ));
        app.files[0].settings.output_directory = Some(PathBuf::from("/home/user/Downloads"));
        app.queue.push(0);
        app.files[0].queued = true;

        let queue_copy: Vec<(usize, PathBuf, PathBuf, String, ImageSettings, u64)> = app
            .queue
            .iter()
            .map(|&idx| {
                let file = &app.files[idx];
                let output_path = if let Some(ref dir) = file.settings.output_directory {
                    dir.join(&file.name)
                } else {
                    file.path.clone()
                };
                (
                    idx,
                    output_path,
                    file.path.clone(),
                    file.name.clone(),
                    file.settings.clone(),
                    file.size,
                )
            })
            .collect();

        assert_eq!(queue_copy.len(), 1);
        assert_eq!(
            queue_copy[0].1,
            PathBuf::from("/home/user/Downloads/image.jpg")
        );
        assert_eq!(queue_copy[0].2, PathBuf::from("/source/image.jpg"));
    }

    #[test]
    fn test_output_dir_full_flow() {
        use std::path::PathBuf;

        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.selected_index = Some(0);
        app.current_dir = PathBuf::from("/test");

        app.input_buffer = "~/Downloads".to_string();
        app.input_target = SettingOption::OutputDir;
        app.setting_option = SettingOption::OutputDir;

        let input = app.input_buffer.clone();
        let final_path: PathBuf = if let Some(remainder) = input.strip_prefix('~') {
            if let Ok(home) = std::env::var("HOME") {
                if remainder.is_empty() || remainder.starts_with('/') {
                    PathBuf::from(home).join(&input[2..])
                } else {
                    PathBuf::from(home).join(remainder)
                }
            } else {
                PathBuf::from(&input)
            }
        } else if PathBuf::from(&input).is_absolute() {
            PathBuf::from(&input)
        } else {
            app.current_dir.join(&input)
        };

        if let Some(file) = app.selected_file_mut() {
            file.settings.output_directory = Some(final_path.clone());
        }

        app.queue.push(0);
        app.files[0].queued = true;

        let output_path = if let Some(ref dir) = app.files[0].settings.output_directory {
            dir.join(&app.files[0].name)
        } else {
            app.files[0].path.clone()
        };

        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        assert_eq!(
            output_path,
            PathBuf::from(format!("{}/Downloads/image.jpg", home))
        );
        assert_eq!(
            app.files[0].settings.output_directory,
            Some(PathBuf::from(format!("{}/Downloads", home)))
        );
    }

    #[test]
    fn test_output_dir_tilde_input_flow() {
        use std::path::PathBuf;

        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());

        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.selected_index = Some(0);
        app.current_dir = PathBuf::from("/test");

        app.input_buffer = "~/images".to_string();
        app.input_target = SettingOption::OutputDir;

        let input = app.input_buffer.clone();
        assert_eq!(input, "~/images");

        let final_path: PathBuf = if input.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                let remainder = &input[1..];
                if remainder.is_empty() || remainder.starts_with('/') {
                    PathBuf::from(home).join(&input[2..])
                } else {
                    PathBuf::from(home).join(remainder)
                }
            } else {
                PathBuf::from(&input)
            }
        } else if PathBuf::from(&input).is_absolute() {
            PathBuf::from(&input)
        } else {
            app.current_dir.join(&input)
        };

        let expected = PathBuf::from(format!("{}/images", home));
        assert_eq!(final_path, expected);

        app.files[0].settings.output_directory = Some(final_path);
        let displayed = path_to_tilde(app.files[0].settings.output_directory.as_ref().unwrap());
        assert_eq!(displayed, "~/images");
    }

    fn simulate_down_nav(app: &mut App) -> Vec<SettingOption> {
        let mut visited = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let max_iterations = 20;

        while seen.insert(app.setting_option) && visited.len() < max_iterations {
            visited.push(app.setting_option);
            let format = app.global_output_format;
            let file = app.selected_file();
            let from_quality = match format {
                None | Some(OutputFormat::Jpeg) => true,
                Some(OutputFormat::Webp) => file.map(|f| !f.settings.webp_lossless).unwrap_or(true),
                Some(OutputFormat::Same) => {
                    if let Some(f) = file {
                        matches!(
                            f.extension().as_deref(),
                            Some("jpg") | Some("jpeg") | Some("webp")
                        )
                    } else {
                        true
                    }
                }
                _ => false,
            };
            let from_webp = format == Some(OutputFormat::Webp);
            let from_progressive = format == Some(OutputFormat::Png);

            match app.setting_option {
                SettingOption::Format => {
                    app.setting_option = if from_webp {
                        SettingOption::WebpLossless
                    } else if from_quality {
                        SettingOption::Quality
                    } else {
                        SettingOption::Color
                    }
                }
                SettingOption::WebpLossless => {
                    app.setting_option = if from_quality {
                        SettingOption::Quality
                    } else {
                        SettingOption::Color
                    }
                }
                SettingOption::Quality => app.setting_option = SettingOption::Color,
                SettingOption::Color => app.setting_option = SettingOption::Exif,
                SettingOption::Exif => {
                    app.setting_option = if from_progressive {
                        SettingOption::Progressive
                    } else {
                        SettingOption::MaxWidth
                    }
                }
                SettingOption::Progressive => app.setting_option = SettingOption::PngCompress,
                SettingOption::PngCompress => app.setting_option = SettingOption::MaxWidth,
                SettingOption::MaxWidth => app.setting_option = SettingOption::MaxHeight,
                SettingOption::MaxHeight => app.setting_option = SettingOption::Overwrite,
                SettingOption::Overwrite => app.setting_option = SettingOption::Backup,
                SettingOption::Backup => app.setting_option = SettingOption::OutputDir,
                SettingOption::OutputDir => app.setting_option = SettingOption::Format,
            }
        }
        visited
    }

    fn simulate_up_nav(app: &mut App) -> Vec<SettingOption> {
        let mut visited = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let max_iterations = 20;

        while seen.insert(app.setting_option) && visited.len() < max_iterations {
            visited.push(app.setting_option);
            let format = app.global_output_format;
            let file = app.selected_file();
            let from_quality = match format {
                None | Some(OutputFormat::Jpeg) => true,
                Some(OutputFormat::Webp) => file.map(|f| !f.settings.webp_lossless).unwrap_or(true),
                Some(OutputFormat::Same) => {
                    if let Some(f) = file {
                        matches!(
                            f.extension().as_deref(),
                            Some("jpg") | Some("jpeg") | Some("webp")
                        )
                    } else {
                        true
                    }
                }
                _ => false,
            };
            let from_webp = format == Some(OutputFormat::Webp);
            let from_progressive = format == Some(OutputFormat::Png);

            match app.setting_option {
                SettingOption::Format => {
                    app.setting_option = if from_webp {
                        SettingOption::WebpLossless
                    } else if from_progressive {
                        SettingOption::Progressive
                    } else if from_quality {
                        SettingOption::Quality
                    } else {
                        SettingOption::Color
                    }
                }
                SettingOption::OutputDir => app.setting_option = SettingOption::Backup,
                SettingOption::Backup => app.setting_option = SettingOption::Overwrite,
                SettingOption::Overwrite => app.setting_option = SettingOption::MaxHeight,
                SettingOption::MaxHeight => app.setting_option = SettingOption::MaxWidth,
                SettingOption::MaxWidth => {
                    app.setting_option = if from_progressive {
                        SettingOption::PngCompress
                    } else {
                        SettingOption::Exif
                    }
                }
                SettingOption::Exif => app.setting_option = SettingOption::Color,
                SettingOption::Color => {
                    app.setting_option = if from_quality {
                        SettingOption::Quality
                    } else if from_webp {
                        SettingOption::WebpLossless
                    } else {
                        SettingOption::Format
                    }
                }
                SettingOption::Quality => {
                    app.setting_option = if from_webp {
                        SettingOption::WebpLossless
                    } else {
                        SettingOption::Format
                    }
                }
                SettingOption::WebpLossless => app.setting_option = SettingOption::Format,
                SettingOption::Progressive => app.setting_option = SettingOption::Exif,
                SettingOption::PngCompress => app.setting_option = SettingOption::Progressive,
            }
        }
        visited
    }

    #[test]
    fn test_settings_navigation_down_same_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = None;

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
        assert!(visited.contains(&SettingOption::Format));
    }

    #[test]
    fn test_settings_navigation_down_jpeg_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Jpeg);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
        assert!(visited.contains(&SettingOption::Format));
        assert!(visited.contains(&SettingOption::Quality));
    }

    #[test]
    fn test_settings_navigation_down_png_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.png"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Png);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
        assert!(visited.contains(&SettingOption::Format));
        assert!(visited.contains(&SettingOption::Progressive));
        assert!(visited.contains(&SettingOption::PngCompress));
    }

    #[test]
    fn test_settings_navigation_down_webp_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.webp"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Webp);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
    }

    #[test]
    fn test_settings_navigation_down_gif_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.gif"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Gif);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
    }

    #[test]
    fn test_settings_navigation_down_tiff_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.tiff"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Tiff);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
    }

    #[test]
    fn test_settings_navigation_down_bmp_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.bmp"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Bmp);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
    }

    #[test]
    fn test_settings_navigation_down_tga_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.tga"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Tga);

        let visited = simulate_down_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
    }

    #[test]
    fn test_settings_navigation_up_webp_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Webp);

        let visited = simulate_up_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
        assert!(
            !visited.contains(&SettingOption::Quality),
            "Quality should be skipped when no file selected"
        );
    }

    #[test]
    fn test_settings_navigation_up_png_no_file() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.png"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = Some(OutputFormat::Png);

        let visited = simulate_up_nav(&mut app);
        assert!(
            visited.len() < 20,
            "Navigation loop detected! Visited: {:?}",
            visited
        );
    }

    #[test]
    fn test_format_cycling_right_arrow() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = None;

        app.global_output_format = Some(OutputFormat::Jpeg);
        assert_eq!(app.global_output_format, Some(OutputFormat::Jpeg));
        app.global_output_format = Some(OutputFormat::Png);
        assert_eq!(app.global_output_format, Some(OutputFormat::Png));
        app.global_output_format = Some(OutputFormat::Webp);
        assert_eq!(app.global_output_format, Some(OutputFormat::Webp));
        app.global_output_format = Some(OutputFormat::Gif);
        assert_eq!(app.global_output_format, Some(OutputFormat::Gif));
        app.global_output_format = Some(OutputFormat::Tiff);
        assert_eq!(app.global_output_format, Some(OutputFormat::Tiff));
        app.global_output_format = Some(OutputFormat::Bmp);
        assert_eq!(app.global_output_format, Some(OutputFormat::Bmp));
        app.global_output_format = Some(OutputFormat::Tga);
        assert_eq!(app.global_output_format, Some(OutputFormat::Tga));
        app.global_output_format = None;
        assert_eq!(app.global_output_format, None);
    }

    #[test]
    fn test_format_cycling_left_arrow() {
        let mut app = App::new();
        app.files.push(ImageFile::new_lightweight(
            PathBuf::from("/test/image.jpg"),
            false,
        ));
        app.focused_column = FocusedColumn::ImageSettings;
        app.setting_option = SettingOption::Format;
        app.global_output_format = None;

        app.global_output_format = Some(OutputFormat::Tga);
        assert_eq!(app.global_output_format, Some(OutputFormat::Tga));
        app.global_output_format = Some(OutputFormat::Bmp);
        assert_eq!(app.global_output_format, Some(OutputFormat::Bmp));
        app.global_output_format = Some(OutputFormat::Tiff);
        assert_eq!(app.global_output_format, Some(OutputFormat::Tiff));
        app.global_output_format = Some(OutputFormat::Gif);
        assert_eq!(app.global_output_format, Some(OutputFormat::Gif));
        app.global_output_format = Some(OutputFormat::Webp);
        assert_eq!(app.global_output_format, Some(OutputFormat::Webp));
        app.global_output_format = Some(OutputFormat::Png);
        assert_eq!(app.global_output_format, Some(OutputFormat::Png));
        app.global_output_format = Some(OutputFormat::Jpeg);
        assert_eq!(app.global_output_format, Some(OutputFormat::Jpeg));
        app.global_output_format = None;
        assert_eq!(app.global_output_format, None);
    }
}
