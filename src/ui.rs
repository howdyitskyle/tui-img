use crate::models::{bytes_to_human, truncate_str, ColorSpace, ImageFile, OutputFormat};
use crate::path_to_tilde;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

pub fn ui(app: &mut crate::App, f: &mut Frame) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(f.size());

    render_main_content(f, vertical[0], app);
    render_status_bar(f, vertical[1], app);

    if let Some(msg) = &app.error_message {
        render_popup(f, f.size(), "Error", msg, Color::Red);
    } else if let Some(msg) = &app.success_message {
        render_popup(f, f.size(), "Success", msg, Color::Green);
    }
}

pub fn render_main_content(f: &mut Frame, area: Rect, app: &mut crate::App) {
    let has_selected = app.selected_file().is_some();
    let wide_enough = area.width >= 100 && has_selected;

    if wide_enough {
        let file_width = (area.width * 50) / 100;
        let settings_width = (area.width * 25) / 100;
        let output_width = area.width - file_width - settings_width - 2;

        let file_area = Rect::new(area.x, area.y, file_width, area.height);
        let settings_area = Rect::new(area.x + file_width + 1, area.y, settings_width, area.height);
        let output_area = Rect::new(
            area.x + file_width + settings_width + 2,
            area.y,
            output_width,
            area.height,
        );

        render_file_list(f, file_area, app);
        render_settings_panel(f, settings_area, app);
        render_output_panel(f, output_area, app);
    } else {
        render_file_list(f, area, app);
    }
}

pub fn render_file_list(f: &mut Frame, area: Rect, app: &mut crate::App) {
    let is_focused = app.focused_column == crate::FocusedColumn::Files;
    let border_style = if is_focused {
        Style::new().cyan()
    } else {
        Style::new().dark_gray()
    };
    let title_style = if is_focused {
        Style::new().cyan().bold()
    } else {
        Style::new().dark_gray().bold()
    };

    let title = " Files ";

    let outer = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(title_style)
        .padding(Padding::new(1, 0, 0, 0));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let footer_height = 1;

    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(footer_height)])
        .split(inner);

    let avail_width = list_chunks[0].width as usize;
    let spacer_len = 2;
    let min_name_width = 20;
    let min_dims_width = 10;
    let min_mp_width = 6;
    let min_size_width = 7;
    let min_color_width = 5;
    let min_exif_width = 15;
    let min_ext_width = 4;

    let min_total = min_name_width
        + min_dims_width
        + min_mp_width
        + min_size_width
        + min_color_width
        + min_exif_width
        + min_ext_width
        + (7 * spacer_len);
    let extra = avail_width.saturating_sub(min_total);

    let name_width = min_name_width + (extra * 4 / 10);
    let exif_width = min_exif_width + (extra * 3 / 10);
    let dims_width = min_dims_width;
    let mp_width = min_mp_width;
    let size_width = min_size_width;
    let color_width = min_color_width;
    let ext_width = min_ext_width;

    let header = Line::from(vec![
        Span::from("  "),
        Span::from(format!("{:<width$}", "NAME", width = name_width)),
        Span::from("  "),
        Span::from(format!("{:>width$}", "DIM", width = dims_width)),
        Span::from("  "),
        Span::from(format!("{:>width$}", "MP", width = mp_width)),
        Span::from("  "),
        Span::from(format!("{:>width$}", "SIZE", width = size_width)),
        Span::from("  "),
        Span::from(format!("{:>width$}", "COLOR", width = color_width)),
        Span::from("  "),
        Span::from(format!("{:<width$}", "EXIF", width = exif_width)),
        Span::from("  "),
        Span::from(format!("{:>width$}", "TYPE", width = ext_width)),
    ])
    .style(Style::new().dark_gray().bold());

    let mut items: Vec<ListItem> = vec![ListItem::new(header)];

    let visible_files: Vec<(usize, &ImageFile)> = app
        .files
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(app.visible_rows)
        .collect();

    let file_items: Vec<ListItem> = visible_files
        .iter()
        .map(|(i, file)| {
            let selected = Some(*i) == app.selected_index;
            let base_style = if selected {
                Style::new().black().on_cyan()
            } else if file.queued {
                Style::new().green()
            } else if file.is_parent {
                Style::new().dark_gray()
            } else if file.is_dir {
                Style::new().cyan()
            } else {
                Style::new().white()
            };

            if file.is_parent || file.is_dir {
                let icon = if file.is_parent { "<" } else { ">" };
                let name = if file.is_parent {
                    "..".to_string()
                } else {
                    file.name.clone()
                };
                let queue = if file.queued { "[Q]" } else { "" };
                let display_name = if file.is_parent {
                    name
                } else {
                    format!("{}{}", name, queue)
                };
                let line = Line::from(vec![
                    Span::from(format!("{}  ", icon)),
                    Span::from(format!("{:<width$}", display_name, width = name_width)),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", "", width = dims_width)),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", "", width = mp_width)),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", "", width = size_width)),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", "", width = color_width)),
                    Span::from("  "),
                    Span::from(format!("{:<width$}", "", width = exif_width)),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", "", width = ext_width)),
                ])
                .style(base_style);
                ListItem::new(line)
            } else {
                let icon = " ";
                let name = truncate_str(&file.name, name_width);
                let queue = if file.queued { "[Q]" } else { "" };

                let ext = file
                    .extension()
                    .unwrap_or_else(|| "".to_string())
                    .to_uppercase();

                let (dims, mp) = file
                    .dimensions
                    .map(|(w, h)| {
                        let dims = format!("{}x{}", w, h);
                        let mp = format!("{:.1}", (w as f64 * h as f64) / 1_000_000.0);
                        (dims, mp)
                    })
                    .unwrap_or_else(|| (String::new(), String::new()));

                let size = file.size_str();
                let color = file.color_type.clone().unwrap_or_else(|| "—".to_string());
                let camera = file
                    .exif_data
                    .as_ref()
                    .and_then(|e| e.camera.clone())
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| "—".to_string());

                let info_style = if selected {
                    Style::new().black()
                } else {
                    Style::new().dark_gray()
                };

                let line = Line::from(vec![
                    Span::from(format!("{}  ", icon)),
                    Span::from(format!(
                        "{:<width$}",
                        format!("{}{}", name, queue),
                        width = name_width
                    )),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", dims, width = dims_width)).style(info_style),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", mp, width = mp_width)).style(info_style),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", size, width = size_width)).style(info_style),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", color, width = color_width)).style(info_style),
                    Span::from("  "),
                    Span::from(format!("{:<width$}", camera, width = exif_width)).style(info_style),
                    Span::from("  "),
                    Span::from(format!("{:>width$}", ext, width = ext_width)).style(info_style),
                ])
                .style(base_style);
                ListItem::new(line)
            }
        })
        .collect();

    items.extend(file_items);
    items.push(ListItem::new(""));

    let list = List::new(items).style(Style::new().white());

    f.render_stateful_widget(list, list_chunks[0], &mut app.list_state);

    let path_text = truncate_str(
        &app.current_dir.to_string_lossy(),
        (list_chunks[1].width as usize).saturating_sub(4),
    );
    let footer = Paragraph::new(Line::from(vec![
        Span::from("File Path: ").cyan(),
        Span::from(path_text).white(),
    ]))
    .style(Style::new().dark_gray());
    f.render_widget(footer, list_chunks[1]);
}

pub fn render_settings_panel(f: &mut Frame, area: Rect, app: &crate::App) {
    let is_focused = app.focused_column == crate::FocusedColumn::ImageSettings;
    let border_style = if is_focused {
        Style::new().cyan()
    } else {
        Style::new().dark_gray()
    };
    let title_style = if is_focused {
        Style::new().cyan().bold()
    } else {
        Style::new().dark_gray().bold()
    };

    let outer = Block::default()
        .title(" Image Settings ")
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(title_style)
        .padding(Padding::new(1, 1, 0, 1));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let file = app.selected_file();

    let color = file
        .map(|f| match f.settings.color_space {
            ColorSpace::Rgb => "RGB",
            ColorSpace::Grayscale => "Grayscale",
            ColorSpace::Rgba => "RGBA",
        })
        .unwrap_or("RGB");

    let slider = create_quality_slider(
        file.map(|f| f.settings.quality).unwrap_or(85),
        (inner.width as usize).saturating_sub(24),
    );

    let exif = file
        .map(|f| {
            if f.settings.remove_exif {
                "Remove"
            } else {
                "Keep"
            }
        })
        .unwrap_or("Keep");
    let progressive = file
        .map(|f| if f.settings.progressive { "Yes" } else { "No" })
        .unwrap_or("No");
    let webp_lossless = file
        .map(|f| {
            if f.settings.webp_lossless {
                "Lossless"
            } else {
                "Lossy"
            }
        })
        .unwrap_or("Lossy");
    let overwrite = file
        .map(|f| {
            if f.settings.overwrite {
                "Overwrite"
            } else {
                "New file"
            }
        })
        .unwrap_or("Overwrite");
    let backup = file
        .map(|f| if f.settings.backup { "Yes" } else { "No" })
        .unwrap_or("No");

    fn opt_no_hint(
        label: String,
        value: String,
        selected: bool,
        panel_focused: bool,
    ) -> Line<'static> {
        let prefix = if selected { ">" } else { " " };
        let label_len = label.len();
        let padding = if label_len < 10 {
            " ".repeat(10 - label_len)
        } else {
            String::new()
        };
        let color_style = if selected {
            Style::new().cyan().bold()
        } else {
            Style::new().dark_gray()
        };
        let value_style = if selected || panel_focused {
            Style::new().white()
        } else {
            Style::new().dark_gray()
        };
        Line::from(vec![
            Span::from(format!("{} ", prefix)).style(color_style),
            Span::from(label).style(color_style),
            Span::from(padding).style(color_style),
            Span::from(": ").style(color_style),
            Span::from(value).style(value_style),
        ])
    }

    fn separator() -> Line<'static> {
        Line::from(vec![Span::raw(" ")])
    }

    let is_settings_focused = app.focused_column == crate::FocusedColumn::ImageSettings;
    let is_q = is_settings_focused && app.setting_option == crate::SettingOption::Quality;
    let is_c = is_settings_focused && app.setting_option == crate::SettingOption::Color;
    let is_e = is_settings_focused && app.setting_option == crate::SettingOption::Exif;
    let is_fmt = is_settings_focused && app.setting_option == crate::SettingOption::Format;
    let is_p = is_settings_focused && app.setting_option == crate::SettingOption::Progressive;
    let is_mw = is_settings_focused && app.setting_option == crate::SettingOption::MaxWidth;
    let is_mh = is_settings_focused && app.setting_option == crate::SettingOption::MaxHeight;
    let is_lar = is_settings_focused && app.setting_option == crate::SettingOption::LockAspectRatio;
    let is_pc = is_settings_focused && app.setting_option == crate::SettingOption::PngCompress;
    let is_wl = is_settings_focused && app.setting_option == crate::SettingOption::WebpLossless;
    let is_ow = is_settings_focused && app.setting_option == crate::SettingOption::Overwrite;
    let is_bk = is_settings_focused && app.setting_option == crate::SettingOption::Backup;
    let is_dir = is_settings_focused && app.setting_option == crate::SettingOption::OutputDir;

    let global_format = app
        .global_output_format
        .map(|f| f.as_str())
        .unwrap_or("Same");

    let dir_path = if app.input_mode && app.input_target == crate::SettingOption::OutputDir {
        if app.input_buffer.is_empty() {
            "New Path".to_string()
        } else {
            format!("{}▌", app.input_buffer)
        }
    } else {
        app.global_output_directory
            .as_ref()
            .map(|p| path_to_tilde(p))
            .unwrap_or_else(|| "Same as source".to_string())
    };

    let max_width = if !app.width_input.is_empty() {
        format!("{}px", app.width_input)
    } else {
        file.as_ref()
            .and_then(|f| f.settings.max_width)
            .map(|w| format!("{}px", w))
            .unwrap_or_else(|| "None".to_string())
    };

    let max_height = if !app.height_input.is_empty() {
        format!("{}px", app.height_input)
    } else {
        file.as_ref()
            .and_then(|f| f.settings.max_height)
            .map(|h| format!("{}px", h))
            .unwrap_or_else(|| "None".to_string())
    };

    let aspect_lock = file
        .map(|f| {
            if f.settings.lock_aspect_ratio {
                "Yes"
            } else {
                "No"
            }
        })
        .unwrap_or("No");

    let mut settings_lines = vec![];

    settings_lines.push(separator());
    settings_lines.push(opt_no_hint(
        "Format".into(),
        global_format.into(),
        is_fmt,
        is_settings_focused,
    ));

    let format = app.global_output_format.unwrap_or(OutputFormat::Same);

    if format == OutputFormat::Webp {
        settings_lines.push(opt_no_hint(
            "WebP".into(),
            webp_lossless.into(),
            is_wl,
            is_settings_focused,
        ));
    }

    let show_quality = match app.global_output_format {
        None | Some(OutputFormat::Jpeg) => true,
        Some(OutputFormat::Webp) => app
            .selected_file()
            .map(|f| !f.settings.webp_lossless)
            .unwrap_or(false),
        Some(OutputFormat::Same) => {
            if let Some(file) = app.selected_file() {
                matches!(
                    file.extension().as_deref(),
                    Some("jpg") | Some("jpeg") | Some("webp")
                )
            } else {
                true
            }
        }
        _ => false,
    };

    if show_quality {
        settings_lines.push(opt_no_hint(
            "Quality".into(),
            slider,
            is_q,
            is_settings_focused,
        ));
    }

    settings_lines.push(opt_no_hint(
        "Color".into(),
        color.into(),
        is_c,
        is_settings_focused,
    ));
    settings_lines.push(opt_no_hint(
        "EXIF".into(),
        exif.into(),
        is_e,
        is_settings_focused,
    ));

    if format == OutputFormat::Png {
        settings_lines.push(separator());
        settings_lines.push(opt_no_hint(
            "Progressive".into(),
            progressive.into(),
            is_p,
            is_settings_focused,
        ));
        settings_lines.push(opt_no_hint(
            "PNG Comp".into(),
            file.as_ref()
                .map(|f| f.settings.png_compression.to_string())
                .unwrap_or_else(|| "6".to_string()),
            is_pc,
            is_settings_focused,
        ));
    }

    settings_lines.push(separator());
    settings_lines.push(opt_no_hint(
        "Max Width".into(),
        max_width.clone(),
        is_mw,
        is_settings_focused,
    ));
    settings_lines.push(opt_no_hint(
        "Max Height".into(),
        max_height.clone(),
        is_mh,
        is_settings_focused,
    ));
    settings_lines.push(opt_no_hint(
        "Aspect Lock".into(),
        aspect_lock.into(),
        is_lar,
        is_settings_focused,
    ));
    settings_lines.push(separator());
    settings_lines.push(opt_no_hint(
        "Output".into(),
        overwrite.into(),
        is_ow,
        is_settings_focused,
    ));
    settings_lines.push(opt_no_hint(
        "Backup".into(),
        backup.into(),
        is_bk,
        is_settings_focused,
    ));
    settings_lines.push(separator());
    settings_lines.push(opt_no_hint(
        "Output Dir".into(),
        dir_path.clone(),
        is_dir,
        is_settings_focused,
    ));

    if app.input_mode {
        let input_line = Line::from(vec![
            Span::from("  > ").style(Style::new().cyan().bold()),
            Span::from(app.input_buffer.as_str()).style(Style::new().white()),
            Span::from("_").style(Style::new().cyan()),
        ]);
        settings_lines.push(input_line);
        settings_lines.push(Line::from(vec![
            Span::from("  [Enter] apply  ").style(Style::new().dark_gray()),
            Span::from("[ESC] cancel").style(Style::new().dark_gray()),
        ]));
    }

    let settings_para = Paragraph::new(settings_lines)
        .style(Style::new().white())
        .block(Block::default());
    f.render_widget(settings_para, inner);
}

pub fn render_output_panel(f: &mut Frame, area: Rect, app: &crate::App) {
    let is_focused = app.focused_column == crate::FocusedColumn::Output;
    let border_style = if is_focused {
        Style::new().cyan()
    } else {
        Style::new().dark_gray()
    };
    let title_style = if is_focused {
        Style::new().cyan().bold()
    } else {
        Style::new().dark_gray().bold()
    };

    let outer = Block::default()
        .title(" Output ")
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(title_style)
        .padding(Padding::new(1, 1, 0, 1));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let queued_files: Vec<&ImageFile> = app.files.iter().filter(|f| f.queued).collect();
    let total_size: u64 = queued_files.iter().map(|f| f.size).sum();

    let mut output_lines = vec![];

    if let Some(file) = app.selected_file() {
        let global_format = app
            .global_output_format
            .map(|f| f.as_str())
            .unwrap_or("Same");
        let color_space = file.settings.color_space.as_str();
        let quality = file.settings.quality;

        output_lines.push(Line::from(vec![Span::raw("")]));
        output_lines.push(Line::from(vec![Span::from("Output Settings").dark_gray()]));

        output_lines.push(Line::from(vec![Span::raw(format!(
            "  Format: {}",
            global_format
        ))
        .dark_gray()]));
        output_lines.push(Line::from(vec![Span::raw(format!(
            "  Color: {}",
            color_space
        ))
        .dark_gray()]));
        output_lines.push(Line::from(vec![Span::raw(format!(
            "  Quality: {}%",
            quality
        ))
        .dark_gray()]));

        if let Some(w) = file.settings.max_width {
            output_lines.push(Line::from(vec![
                Span::raw(format!("  Max Width: {}px", w)).dark_gray()
            ]));
        }
        if let Some(h) = file.settings.max_height {
            output_lines.push(Line::from(vec![Span::raw(format!(
                "  Max Height: {}px",
                h
            ))
            .dark_gray()]));
        }
    }

    output_lines.push(Line::from(vec![Span::raw("")]));
    output_lines.push(Line::from(vec![Span::from("Queue").dark_gray()]));

    if queued_files.is_empty() {
        output_lines.push(Line::from(vec![
            Span::raw("  No files selected").dark_gray()
        ]));
    } else {
        output_lines.push(Line::from(vec![Span::raw(format!(
            "  {} files • {}",
            queued_files.len(),
            bytes_to_human(total_size)
        ))
        .dark_gray()]));

        if queued_files.len() <= 10 {
            for file in &queued_files {
                let ext = file.extension().unwrap_or_default().to_uppercase();
                output_lines.push(Line::from(vec![
                    Span::from("  ").dark_gray(),
                    Span::from(&file.name).white(),
                    Span::raw(" ").dark_gray(),
                    Span::raw(format!("({})", ext)).dark_gray(),
                ]));
            }
        } else {
            for file in &queued_files[..5] {
                let ext = file.extension().unwrap_or_default().to_uppercase();
                output_lines.push(Line::from(vec![
                    Span::from("  ").dark_gray(),
                    Span::from(&file.name).white(),
                    Span::raw(" ").dark_gray(),
                    Span::raw(format!("({})", ext)).dark_gray(),
                ]));
            }
            output_lines.push(Line::from(vec![Span::raw("  ...").dark_gray()]));
            output_lines.push(Line::from(vec![Span::raw(format!(
                "  +{} more files",
                queued_files.len() - 5
            ))
            .dark_gray()]));
        }
    }

    if !app.compression_results.is_empty() && !app.compressing {
        output_lines.push(Line::from(vec![Span::raw("")]));
        output_lines.push(Line::from(vec![Span::from("Results:").dark_gray()]));

        for result in app.compression_results.iter().rev().take(5) {
            let file = &app.files[result.file_index];
            if let Some(ref error) = result.error {
                output_lines.push(Line::from(vec![
                    Span::from(format!("  {} ", "✗")).red(),
                    Span::from(&file.name).white(),
                    Span::raw(" ").dark_gray(),
                    Span::raw(truncate_str(error, 30)).dark_gray(),
                ]));
            } else {
                let savings = if result.original_size > result.new_size {
                    let diff = result.original_size - result.new_size;
                    format!("-{}", bytes_to_human(diff))
                } else {
                    "+".to_string()
                };
                let savings_color = if result.original_size > result.new_size {
                    Style::new().green()
                } else {
                    Style::new().yellow()
                };
                output_lines.push(Line::from(vec![
                    Span::from(format!("  {} ", "✓")).cyan(),
                    Span::from(&file.name).white(),
                    Span::raw(" ").dark_gray(),
                    Span::from(format!(
                        "{} → {} ({})",
                        bytes_to_human(result.original_size),
                        bytes_to_human(result.new_size),
                        savings
                    ))
                    .style(savings_color),
                ]));
            }
        }
    }

    if app.compressing {
        if let Some((current, total, sub_progress, status)) = &app.progress {
            let base = (*current as i32 - 1).max(0) as f32;
            let file_progress = *sub_progress as f32 / 100.0;
            let overall = ((base + file_progress) * 100.0 / *total as f32) as u8;
            let bar_width = 20;
            let filled = (overall as usize * bar_width) / 100;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

            output_lines.push(Line::from(vec![Span::raw("")]));
            output_lines.push(Line::from(vec![Span::from("Compressing...").cyan().bold()]));
            output_lines.push(Line::from(vec![Span::raw(status.clone()).white()]));
            output_lines.push(Line::from(vec![Span::raw("")]));
            output_lines.push(Line::from(vec![Span::raw(format!(
                "[{}] {}% ({}/{})",
                bar, overall, current, total
            ))
            .cyan()]));
        }
    }

    let para = Paragraph::new(output_lines)
        .style(Style::new().white())
        .block(Block::default());
    f.render_widget(para, inner);
}

pub fn create_quality_slider(quality: u8, width: usize) -> String {
    let slider_width = width.max(10);
    let pos = (quality as usize * slider_width) / 100;

    let mut bar = String::with_capacity(slider_width);
    for i in 0..slider_width {
        if i == pos {
            bar.push('█');
        } else if i % (slider_width / 4).max(1) == 0 {
            bar.push('│');
        } else {
            bar.push('▒');
        }
    }
    format!("[{}] {}%", bar, quality)
}

pub fn render_status_bar(f: &mut Frame, area: Rect, app: &crate::App) {
    let spans = if app.compressing {
        vec![
            Span::from("[Esc]").cyan(),
            Span::raw(" Cancel   "),
            Span::from("[q]").cyan(),
            Span::raw(" Quit"),
        ]
    } else {
        vec![
            Span::from("[↑↓]").cyan(),
            Span::raw(" Nav   "),
            Span::from("[Space]").cyan(),
            Span::raw(" Queue   "),
            Span::from("[Enter]").cyan(),
            Span::raw(" Open Dir   "),
            Span::from("[←→]").cyan(),
            Span::raw(" Change   "),
            Span::from("[Tab]").cyan(),
            Span::raw(" Switch Panel   "),
            Span::raw("│   "),
            Span::from("[c]").cyan(),
            Span::raw(" Compress   "),
            Span::from("[C]").cyan(),
            Span::raw(" Clear   "),
            Span::from("[q]").cyan(),
            Span::raw(" Quit"),
        ]
    };

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::new().dark_gray())
        .centered()
        .block(Block::default());

    f.render_widget(paragraph, area);
}

pub fn render_popup(f: &mut Frame, area: Rect, title: &str, message: &str, color: Color) {
    let popup_width = (area.width as usize / 2).min(message.len() + 10).max(40);
    let popup_height = 5;

    let left = (area.width as usize - popup_width) as u16 / 2;
    let top = (area.height as usize - popup_height) as u16 / 2;

    let rect = Rect::new(left, top, popup_width as u16, popup_height as u16);

    let block = Block::default()
        .title(title)
        .title_style(Style::new().bold().black())
        .borders(Borders::ALL)
        .border_style(Style::new().black())
        .style(Style::new().bg(color));

    f.render_widget(block, rect);

    let inner = Rect::new(
        left + 1,
        top + 1,
        popup_width as u16 - 2,
        popup_height as u16 - 2,
    );
    let para = Paragraph::new(vec![Line::from(Span::raw(message))])
        .style(Style::new().black())
        .centered();
    f.render_widget(para, inner);
}
