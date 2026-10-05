use ratatui::{
    layout::{Constraint, Direction, Layout, Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap, Block, Borders, Clear, BorderType},
    Frame,
};
use crate::tui::app::{TuiApp, CurrentView};

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let px = if r.width < 100 { std::cmp::min(95, percent_x + 30) } else if r.width > 200 { std::cmp::max(20, percent_x.saturating_sub(20)) } else { percent_x };
    let py = if r.height < 30 { std::cmp::min(90, percent_y + 20) } else { percent_y };
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - py) / 2),
            Constraint::Percentage(py),
            Constraint::Percentage((100 - py) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - px) / 2),
            Constraint::Percentage(px),
            Constraint::Percentage((100 - px) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let size = f.area();

    // Main layout takes the full screen now
    let has_tasks = !app.active_downloads.is_empty() || (app.is_generating && matches!(app.current_view, CurrentView::Chat)) || !app.current_task_status.is_empty();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top bar
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // Main area (Chat/Settings)
            Constraint::Length(if has_tasks { 3 } else { 0 }), // Tasks panel
            Constraint::Length(3), // Input area
        ])
        .split(size);

    // ==========================================
    // 1. TOP BAR
    // ==========================================
    let mut top_bar = vec![
        Span::styled(" O M N I ", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD)),
        Span::styled(format!("v{}   ", env!("CARGO_PKG_VERSION")), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("CPU: {:.0}% | RAM: {:.1}GB   ", app.sys_monitor.cpu_usage, app.sys_monitor.ram_used_gb), Style::default().fg(Color::Rgb(100,100,150))),
        Span::styled("(Tab: Control Center | F2: Settings | Esc: Exit)", Style::default().fg(Color::DarkGray)),
    ];
    let header = Paragraph::new(Line::from(top_bar));
    f.render_widget(header, chunks[0]);
    f.render_widget(Paragraph::new(Line::from(Span::styled(str::repeat("─", size.width.into()), Style::default().fg(Color::DarkGray)))), chunks[1]);

    // ==========================================
    // 2. MAIN AREA (CHAT OR SETTINGS)
    // ==========================================
    match app.current_view {
        CurrentView::Chat => {
            let mut chat_lines = Vec::new();
            for msg in &app.messages {
                let color = if msg.starts_with("[System]") {
                    Color::DarkGray
                } else if msg.starts_with("[User]") {
                    app.config.user_color
                } else if msg.starts_with("[Thinker]") {
                    app.config.ai_color
                } else if msg.starts_with("[FILE INCLUSION") {
                    Color::Yellow
                } else {
                    Color::White
                };
                
                for line in msg.split('\n') {
                    chat_lines.push(Line::from(Span::styled(line.to_string(), Style::default().fg(color))));
                }
                chat_lines.push(Line::from(""));
            }

            let total_lines = chat_lines.len() as u16;
            let area_height = chunks[2].height;
            let mut scroll_pos = if total_lines > area_height { total_lines - area_height } else { 0 };
            scroll_pos = scroll_pos.saturating_sub(app.chat_scroll_offset as u16);
            
            let chat_area = Paragraph::new(chat_lines)
                .wrap(Wrap { trim: true })
                .scroll((scroll_pos, 0));
            f.render_widget(chat_area, chunks[2]);
        }
        CurrentView::Settings => {
            let mut settings_lines = vec![
                Line::from(""),
                Line::from(Span::styled(" === Engine Configuration ===", Style::default().fg(Color::DarkGray))),
                Line::from(""),
            ];
            
            let opts = [
                ("Active Model      ", format!("[ {} ]", app.config.default_model)),
                ("Context Size      ", format!("[ {} ]", app.config.context_size)),
                ("CPU Threads       ", format!("[ {} ]", app.config.cpu_threads)),
                ("Hardware          ", format!("[ {} ]", app.config.hardware_backend)),
            ];
            
            for (i, (label, val)) in opts.iter().enumerate() {
                if i == app.settings_selected {
                    settings_lines.push(Line::from(vec![
                        Span::styled(" ❯ ", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD)),
                        Span::styled(*label, Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD)),
                        Span::styled(val.clone(), Style::default().fg(Color::White)),
                    ]));
                } else {
                    settings_lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::raw(*label),
                        Span::styled(val.clone(), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
            
            settings_lines.push(Line::from(""));
            settings_lines.push(Line::from(Span::styled(" (Use Up/Down to navigate, Esc to return)", Style::default().fg(Color::Rgb(100,100,100)))));
            let settings_area = Paragraph::new(settings_lines).wrap(Wrap { trim: true });
            f.render_widget(settings_area, chunks[2]);
        }
    }

    // ==========================================
    // 3. TASKS PANEL
    // ==========================================
    if has_tasks {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let current_spin = spinner[app.tick_count % spinner.len()];
        let mut task_lines = vec![
            Line::from(Span::styled(" 1 task running", Style::default().fg(Color::Gray))),
        ];
        if !app.active_downloads.is_empty() {
            let task = &app.active_downloads[0];
            let mb_down = (task.downloaded_bytes as f64) / 1024.0 / 1024.0;
            let mb_total = (task.total_bytes as f64) / 1024.0 / 1024.0;
            let text = if task.total_bytes == 0 {
                format!("  {} Downloading {}: {:.1}MB", current_spin, task.model_name, mb_down)
            } else {
                format!("  {} Downloading {}: {:.1}MB / {:.1}MB ({:.1}%)", current_spin, task.model_name, mb_down, mb_total, task.percent)
            };
            task_lines.push(Line::from(Span::styled(text, Style::default().fg(Color::DarkGray))));
        } else if app.is_generating && matches!(app.current_view, CurrentView::Chat) {
            task_lines.push(Line::from(vec![
                Span::styled(format!("  {} ", current_spin), Style::default().fg(app.config.accent_color)),
                Span::styled(app.current_task_status.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }
        let tasks_block = Paragraph::new(task_lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(50, 50, 50)))
            );
        f.render_widget(tasks_block, chunks[3]);
    }

    // ==========================================
    // 4. INPUT AREA
    // ==========================================
    let input_text = app.input.value();
    let context_tag = if app.attached_files.is_empty() { String::new() } else { format!(" 📎 {} files attached | ", app.attached_files.len()) };
    let input_lines = vec![
        Line::from(vec![
            Span::styled(" ❯ ", Style::default().fg(Color::White)),
            Span::raw(input_text),
            Span::styled("█", Style::default().fg(Color::DarkGray).add_modifier(Modifier::RAPID_BLINK)),
        ]),
        Line::from(""), // Spacer
        Line::from(vec![
            Span::styled(format!(" {}+ {} ", context_tag, app.config.default_model), Style::default().fg(Color::DarkGray)),
            Span::styled("  ^  ", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let input_block = Paragraph::new(input_lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(80, 80, 80)))
        )
        .wrap(Wrap { trim: true });
    
    if matches!(app.current_view, CurrentView::Chat) {
        f.render_widget(input_block, chunks[4]);
    }

    // ==========================================
    // 5. OMNI CONTROL CENTER (THE FLOATING MODAL)
    // ==========================================
    if app.show_sidebar {
        // Create a centered area for the modal (50% width, 60% height)
        let modal_area = centered_rect(55, 65, size);
        
        // Clear the background to give a floating effect
        f.render_widget(Clear, modal_area);
        
        // Draw the outer rounded block
        let block = Block::default()
            .title(Span::styled(" ❖ Omni Control Center ", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.config.accent_color));
            
        let inner_area = block.inner(modal_area);
        f.render_widget(block, modal_area);
        
        // Split the modal into Tools and Sessions sections
        let cc_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Tools Title
                Constraint::Length(4), // Tools list
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // History Title
                Constraint::Min(0),    // History list
                Constraint::Length(1), // Footer
            ])
            .split(inner_area);
            
        // Render Tools (Top Section)
        let tools_title = Line::from(Span::styled(" ⚙ System Operations", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
        f.render_widget(Paragraph::new(tools_title), cc_chunks[0]);
        
        let tools = [
            "[+] New Conversation",
            "[⚙] Engine Settings",
            "[⚡] Performance Monitor",
            "[×] Clear Memory Cache",
        ];
        
        let tools_start = app.saved_sessions.len();
        let mut tools_lines = Vec::new();
        for (i, &t) in tools.iter().enumerate() {
            let idx = i + tools_start;
            if idx == app.sidebar_selected {
                tools_lines.push(Line::from(Span::styled(format!("  ❯ {}", t), Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))));
            } else if t.starts_with("[×]") {
                tools_lines.push(Line::from(Span::styled(format!("    {}", t), Style::default().fg(Color::Rgb(255, 100, 100)))));
            } else {
                tools_lines.push(Line::from(Span::styled(format!("    {}", t), Style::default().fg(Color::Gray))));
            }
        }
        f.render_widget(Paragraph::new(tools_lines), cc_chunks[1]);
        
        // Render Sessions (Bottom Section)
        let hist_title = Line::from(Span::styled("  Recent Sessions", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
        f.render_widget(Paragraph::new(hist_title), cc_chunks[3]);
        
        let mut hist_lines = Vec::new();
        if app.saved_sessions.is_empty() {
            hist_lines.push(Line::from(Span::styled("    (No saved sessions)", Style::default().fg(Color::DarkGray))));
        } else {
            for (i, (name, _)) in app.saved_sessions.iter().enumerate() {
                let display_name = if name.len() > 30 { format!("{}..", &name[..28]) } else { name.clone() };
                if i == app.sidebar_selected {
                    hist_lines.push(Line::from(Span::styled(format!("  ❯ {}", display_name), Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))));
                } else {
                    hist_lines.push(Line::from(Span::styled(format!("    {}", display_name), Style::default().fg(Color::Gray))));
                }
            }
        }
        f.render_widget(Paragraph::new(hist_lines), cc_chunks[4]);
        
        // Render Footer
        let footer = Line::from(Span::styled(" [↑/↓] Navigate   [Enter] Select   [Esc] Close ", Style::default().fg(Color::DarkGray)));
        f.render_widget(Paragraph::new(footer).alignment(Alignment::Center), cc_chunks[5]);
    } else if input_text.starts_with('/') && matches!(app.current_view, CurrentView::Chat) {
        // ==========================================
        // 6. AUTOCOMPLETE POPUP OVERLAY
        // ==========================================
        let commands = vec![
            ("goal <task>", "Trigger Swarm multi-agent execution"),
            ("read <file>", "Inject a local file into context"),
            ("copy", "Copy the last generated code block"),
            ("models", "List all available local .gguf models"),
            ("download <url>", "Download a model"),
            ("system <prompt>", "Change the active persona"),
            ("theme <color>", "Change accent color"),
            ("reset", "Clear chat history"),
            ("exit", "Quit Omni Engine"),
        ];

        let typed = input_text.to_lowercase().replace("/", "");
        let filtered: Vec<_> = commands.into_iter()
            .filter(|(cmd, _)| cmd.contains(&typed) || typed.is_empty())
            .collect();

        if !filtered.is_empty() {
            let popup_height = filtered.len() as u16 + 2;
            let popup_width = chunks[4].width.max(75);
            let popup_area = Rect {
                x: chunks[4].x,
                y: chunks[4].y.saturating_sub(popup_height),
                width: popup_width,
                height: popup_height,
            };

            let mut lines = Vec::new();
            for (cmd, desc) in filtered {
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(format!("{:<20}", cmd), Style::default().fg(Color::White)),
                    Span::styled(desc, Style::default().fg(Color::DarkGray)),
                ]));
            }

            let popup_block = Paragraph::new(lines)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(60, 60, 60)))
                );

            f.render_widget(Clear, popup_area);
            f.render_widget(popup_block, popup_area);
        }
    }

    // ==========================================
    // 7. GLOBAL LOADING OVERLAY (BLOCKING)
    // ==========================================
    if app.is_global_loading {
        let loading_area = centered_rect(60, 50, size);
        f.render_widget(Clear, loading_area);
        
        let frames = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
        let spinner = frames[(app.tick_count as usize / 2) % frames.len()];
        
        let logo = vec![
            Line::from(""),
            Line::from(Span::styled("   ██████╗ ███╗   ███╗███╗   ██╗██╗", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("  ██╔═══██╗████╗ ████║████╗  ██║██║", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("  ██║   ██║██╔████╔██║██╔██╗ ██║██║", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("  ██║   ██║██║╚██╔╝██║██║╚██╗██║██║", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("  ╚██████╔╝██║ ╚═╝ ██║██║ ╚████║██║", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("   ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝", Style::default().fg(app.config.accent_color).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("      {}  ", spinner), Style::default().fg(Color::Cyan)),
                Span::styled(app.global_loading_msg.clone(), Style::default().fg(Color::White).add_modifier(Modifier::RAPID_BLINK))
            ]),
            Line::from(""),
            Line::from(Span::styled("  Please wait while the quantum weights are aligned...", Style::default().fg(Color::DarkGray))),
        ];
        
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(app.config.accent_color))
            .style(Style::default().bg(Color::Rgb(10, 10, 15)));
            
        f.render_widget(Paragraph::new(logo).alignment(Alignment::Center).block(block), loading_area);
    }
}
