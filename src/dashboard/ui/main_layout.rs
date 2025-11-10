use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph, Row, Table, Wrap},
};
use crate::dashboard::types::*;
use crate::icons::Icons;
use crate::theme::Theme;
use crate::vault_manager::VaultConfig;

impl Dashboard {
    pub fn render_status_panel(&self, f: &mut Frame, area: Rect) {
        // Format balances with truncated addresses
        let pq_balance_text = if let Some(balance) = self.pq_balance {
            let balance_tokens = balance as f64 / 1_000_000.0;
            format!("{:>15.2}", balance_tokens)
        } else {
            format!("{:>15}", "---")
        };

        let pq_mint_str = self.pq_mint.to_string();
        let pq_mint_truncated = format!("{}...{}", &pq_mint_str[..4], &pq_mint_str[pq_mint_str.len()-4..]);

        let standard_balance_text = if let Some(balance) = self.standard_balance {
            let balance_tokens = balance as f64 / 1_000_000.0;
            format!("{:>15.2}", balance_tokens)
        } else {
            format!("{:>15}", "---")
        };

        let standard_mint_str = self.standard_mint.to_string();
        let standard_mint_truncated = format!("{}...{}", &standard_mint_str[..4], &standard_mint_str[standard_mint_str.len()-4..]);

        // Calculate total portfolio value
        let total_balance = if let (Some(pq), Some(std)) = (self.pq_balance, self.standard_balance) {
            let total = (pq + std) as f64 / 1_000_000.0;
            format!("{:>15.2}", total)
        } else {
            format!("{:>15}", "---")
        };

        // Build Bloomberg-style table with dense info
        let rows = vec![
            Row::new(vec![
                Line::from(Span::styled("qcoin", Style::default().fg(Theme::BLOOMBERG_ORANGE))),
                Line::from(Span::styled(standard_balance_text, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled(&standard_mint_truncated, Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("", Style::default())),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("pqcoin", Style::default().fg(Theme::BLOOMBERG_ORANGE))),
                Line::from(Span::styled(pq_balance_text, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled(&pq_mint_truncated, Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("", Style::default())),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("─────────────────────", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("─────────────────────", Style::default().fg(Theme::DIM))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("TOTAL", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(total_balance, Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD))),
            ]).height(1),
        ];

        let widths = [Constraint::Length(18), Constraint::Min(20)];

        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ PORTFOLIO SUMMARY ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(1);

        f.render_widget(table, area);
    }
    pub fn render_actions_panel(&self, f: &mut Frame, area: Rect) {
        // Bloomberg-style professional action menu
        let actions = vec![
            ("PORTFOLIO", "S", "View detailed portfolio summary", Theme::BLOOMBERG_ORANGE),
            ("REGISTER", "G", "Initialize PQ account on-chain", Theme::BLOOMBERG_ORANGE),
            ("LOCK", "L", "Secure vault with challenge", Theme::BLOOMBERG_ORANGE),
            ("UNLOCK", "U", "44-step SPHINCS+ verification", Theme::BLOOMBERG_ORANGE),
            ("TRANSFER", "T", "Send tokens to recipient", Theme::BLOOMBERG_ORANGE),
            ("WRAP", "W", "Standard -> PQ-Secured", Theme::BLOOMBERG_ORANGE),
            ("UNWRAP", "E", "PQ-Secured -> Standard", Theme::BLOOMBERG_ORANGE),
            ("AIRDROP", "A", "Claim 100 tokens (24h limit)", Theme::BLOOMBERG_ORANGE),
            ("STATS", "P", "View network statistics", Theme::BLOOMBERG_ORANGE),
            ("CLOSE", "X", "Close vault & reclaim rent", Theme::BLOOMBERG_ORANGE),
            ("CHART", "M", "Network metrics & charts", Theme::BLOOMBERG_ORANGE),
            ("VAULTS", "V", "Switch/manage vaults", Theme::BLOOMBERG_ORANGE),
        ];

        // Build table rows with selection highlighting
        let pulse = self.get_pulse_intensity();
        let rows: Vec<Row> = actions
            .iter()
            .enumerate()
            .map(|(idx, (action, key, desc, color))| {
                let is_selected = idx == self.selected_action;

                // Animated background for selected row - light purple tint on white
                let row_style = if is_selected {
                    Style::default().bg(Color::Rgb(
                        (230 + pulse / 8).min(255),
                        (220 + pulse / 8).min(255),
                        (245).min(255)
                    ))
                } else {
                    Style::default()
                };

                // Add selection indicator and brighter colors for selected item
                let action_text = if is_selected {
                    format!("▶ {}", action)
                } else {
                    format!("  {}", action)
                };

                let action_color = if is_selected {
                    // Brighter, animated color for selected
                    match color {
                        Color::Rgb(r, g, b) => Color::Rgb(
                            r.saturating_add(pulse / 3),
                            g.saturating_add(pulse / 3),
                            b.saturating_add(pulse / 3),
                        ),
                        _ => *color,
                    }
                } else {
                    *color
                };

                Row::new(vec![
                    Line::from(Span::styled(action_text, Style::default().fg(action_color).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(*key, Style::default().fg(if is_selected { Theme::YELLOW_NEON } else { Theme::CYAN_BRIGHT }).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(*desc, Style::default().fg(if is_selected { Theme::TEXT } else { Theme::SUBTEXT1 }))),
                ])
                .style(row_style)
            })
            .collect();

        let widths = [
            Constraint::Length(17),  // Action name (increased for ▶ indicator)
            Constraint::Length(6),   // Key
            Constraint::Min(20),     // Description
        ];

        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ QUICK ACTIONS ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(2)
            .header(
                Row::new(vec![
                    Line::from(Span::styled("ACTION", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("KEY", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("DESCRIPTION", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                ])
                .style(Style::default().bg(Theme::GLASS_1))
                .bottom_margin(1)
            );

        f.render_widget(table, area);
    }
    pub fn render_footer(&self, f: &mut Frame, area: Rect) {
        // Always split footer into controls + status
        let footer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)].as_ref())
            .split(area);

        // Controls with Bloomberg-style badges
        let footer_text = vec![Line::from(vec![
            Span::styled(
                " Q/ESC ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::RED_NEON)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Quit  ", Style::default().fg(Theme::TEXT)),
            Span::styled(
                " H/? ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::BLOOMBERG_ORANGE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Help  ", Style::default().fg(Theme::TEXT)),
            Span::styled(
                " R ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::CYAN_NEON)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Refresh  ", Style::default().fg(Theme::TEXT)),
            Span::styled(
                " ↑↓/JK ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::YELLOW_NEON)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Navigate  ", Style::default().fg(Theme::TEXT)),
            Span::styled(
                " ENTER ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::GREEN_NEON)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Execute", Style::default().fg(Theme::TEXT)),
        ])];
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);
        let footer = Paragraph::new(footer_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(format!(" {} CONTROLS ", Icons::KEYBOARD))
                    .title_style(Style::default()
                        .fg(Theme::BLOOMBERG_ORANGE)
                        .add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);
        f.render_widget(footer, footer_chunks[0]);

        // Status message - prioritize unlock/lock success messages
        let status_msg = if let Some(ref success_msg) = self.unlock_success_message {
            success_msg.clone()
        } else if let Some(ref success_msg) = self.lock_success_message {
            success_msg.clone()
        } else if let Some(ref msg) = self.status_message {
            msg.clone()
        } else {
            "Ready - Press H or ? for help, Q to quit".to_string()
        };

        let status_color = if self.unlock_success_message.as_ref()
            .map(|m| m.starts_with("✓"))
            .unwrap_or(false)
            || self.lock_success_message.as_ref()
            .map(|m| m.starts_with("✓"))
            .unwrap_or(false) {
            Theme::GREEN_NEON
        } else if self.unlock_success_message.as_ref()
            .map(|m| m.starts_with("✗"))
            .unwrap_or(false)
            || self.lock_success_message.as_ref()
            .map(|m| m.starts_with("✗"))
            .unwrap_or(false) {
            Theme::RED_NEON
        } else {
            Theme::CYAN_NEON
        };

        let status_border = if status_color == Theme::GREEN_NEON || status_color == Theme::RED_NEON {
            status_color
        } else {
            border_color
        };

        let status_widget = Paragraph::new(status_msg)
            .style(Style::default().fg(status_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(status_border).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" STATUS ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
            )
            .style(Style::default().bg(Theme::BASE));

        f.render_widget(status_widget, footer_chunks[1]);
    }

    pub fn render_content_area(&self, f: &mut Frame, area: Rect) {
        // Check if unlock/lock is in progress - show splash animation
        if let Some(ref unlock_flag) = self.unlock_complete {
            let is_complete = unlock_flag.load(std::sync::atomic::Ordering::SeqCst);
            let _ = std::fs::OpenOptions::new().append(true).create(true).open("/tmp/unlock_check.log")
                .and_then(|mut f| std::io::Write::write_all(&mut f, format!("Unlock check: complete={}\n", is_complete).as_bytes()));

            if !is_complete {
                self.render_unlock_splash_animation(f, area);
                return;
            }
        }

        if let Some(ref lock_flag) = self.lock_complete {
            if !lock_flag.load(std::sync::atomic::Ordering::SeqCst) {
                self.render_lock_splash_animation(f, area);
                return;
            }
        }

        // If action is in progress, show action steps instead of static content
        if !self.action_steps.is_empty() {
            self.render_action_progress(f, area);
            return;
        }

        // Otherwise render different content based on selected action
        match self.selected_action {
            0 => self.render_portfolio_content(f, area),    // PORTFOLIO
            1 => self.render_register_content(f, area),     // REGISTER
            2 => self.render_lock_content(f, area),         // LOCK
            3 => self.render_unlock_content(f, area),       // UNLOCK
            4 => self.render_transfer_content(f, area),     // TRANSFER
            5 => self.render_wrap_content(f, area),         // WRAP
            6 => self.render_unwrap_content(f, area),       // UNWRAP
            7 => self.render_airdrop_content(f, area),      // AIRDROP
            8 => self.render_stats_content(f, area),        // STATS
            9 => self.render_close_content(f, area),        // CLOSE
            10 => self.render_chart_content(f, area),        // CHART
            11 => self.render_vaults_content(f, area),      // VAULTS
            _ => self.render_default_content(f, area),      // Default
        }
    }

    fn render_portfolio_content(&self, f: &mut Frame, area: Rect) {
        // Enhanced portfolio view with more details
        let pq_balance_text = if let Some(balance) = self.pq_balance {
            let balance_tokens = balance as f64 / 1_000_000.0;
            format!("{:.6}", balance_tokens)
        } else {
            "---".to_string()
        };

        let standard_balance_text = if let Some(balance) = self.standard_balance {
            let balance_tokens = balance as f64 / 1_000_000.0;
            format!("{:.6}", balance_tokens)
        } else {
            "---".to_string()
        };

        let pq_mint_str = self.pq_mint.to_string();
        let standard_mint_str = self.standard_mint.to_string();

        let total_balance = if let (Some(pq), Some(std)) = (self.pq_balance, self.standard_balance) {
            let total = (pq + std) as f64 / 1_000_000.0;
            format!("{:.6}", total)
        } else {
            "---".to_string()
        };

        // Build detailed portfolio table
        let rows = vec![
            Row::new(vec![
                Line::from(Span::styled("TOKEN", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("BALANCE", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("MINT ADDRESS", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("qcoin", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(standard_balance_text, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(standard_mint_str, Style::default().fg(Theme::SUBTEXT1))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("pqcoin", Style::default().fg(Theme::PURPLE_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(pq_balance_text, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(pq_mint_str, Style::default().fg(Theme::SUBTEXT1))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]).height(1),
            Row::new(vec![
                Line::from(Span::styled("TOTAL", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(total_balance, Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("", Style::default())),
            ]).height(1),
        ];

        let widths = [Constraint::Length(12), Constraint::Length(20), Constraint::Min(45)];

        let border_color = Color::Rgb(140, 140, 140);

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ PORTFOLIO DETAILS ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(2);

        f.render_widget(table, area);
    }

    fn render_action_progress(&self, f: &mut Frame, area: Rect) {
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        // Build content lines from action steps
        let mut content_lines = vec![Line::from("")];

        if self.action_steps.is_empty() {
            content_lines.push(Line::from(vec![
                Span::styled("STATUS: ", Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
                Span::styled("Initializing...", Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::ITALIC)),
            ]));
        } else {
            for (idx, step) in self.action_steps.iter().enumerate() {
                let (icon, message, color) = match step {
                    ActionStep::Starting => ("⏳", "Preparing...", Theme::YELLOW_NEON),
                    ActionStep::InProgress(msg) => ("⚡", msg.as_str(), Theme::CYAN_NEON),
                    ActionStep::Success(msg) => ("✓", msg.as_str(), Theme::GREEN_NEON),
                    ActionStep::Error(msg) => ("✗", msg.as_str(), Theme::RED_NEON),
                };

                let step_label = format!("STEP {}:", idx + 1);

                // For long messages, wrap them
                let max_width = (area.width as usize).saturating_sub(20);
                if message.len() > max_width {
                    // First line with step label
                    content_lines.push(Line::from(vec![
                        Span::styled(step_label, Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
                    ]));

                    // Message on next line with icon
                    content_lines.push(Line::from(vec![
                        Span::styled(format!("  {} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                        Span::styled(message, Style::default().fg(color)),
                    ]));
                } else {
                    // Single line for short messages
                    content_lines.push(Line::from(vec![
                        Span::styled(format!("{} ", step_label), Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                        Span::styled(message, Style::default().fg(color)),
                    ]));
                }

                // Add separator between steps
                if idx < self.action_steps.len() - 1 {
                    content_lines.push(Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))));
                }
                content_lines.push(Line::from(""));
            }
        }

        let content = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ ACTION IN PROGRESS ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        f.render_widget(content, area);
    }

    fn render_default_content(&self, f: &mut Frame, area: Rect) {
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let text = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "WELCOME TO PQCOIN TERMINAL",
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Post-Quantum Secure Digital Currency",
                Style::default().fg(Theme::TEXT)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Select an action from the sidebar to get started",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Security: SPHINCS+ SHA2-128s (NIST FIPS 205)",
                Style::default().fg(Theme::SUBTEXT0)
            )),
            Line::from(Span::styled(
                "Network: Solana Devnet",
                Style::default().fg(Theme::SUBTEXT0)
            )),
        ];

        let welcome = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ OVERVIEW ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(welcome, area);
    }

    fn render_register_content(&self, f: &mut Frame, area: Rect) {
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "REGISTER PQ ACCOUNT",
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Initialize your post-quantum account on-chain",
                Style::default().fg(Theme::TEXT)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "This will:",
                Style::default().fg(Theme::SUBTEXT1).add_modifier(Modifier::BOLD)
            )),
            Line::from(Span::styled(
                "  • Create a PQ account associated with your wallet",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • Store your SPHINCS+ public key on-chain",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • Enable post-quantum secure operations",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press ENTER to register or ESC to cancel",
                Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)
            )),
        ];

        let content = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ REGISTER ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, area);
    }

    fn render_lock_content(&self, f: &mut Frame, area: Rect) {
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "LOCK VAULT",
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Secure your vault with post-quantum cryptography",
                Style::default().fg(Theme::TEXT)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Locking your vault will:",
                Style::default().fg(Theme::SUBTEXT1).add_modifier(Modifier::BOLD)
            )),
            Line::from(Span::styled(
                "  • Generate a random 32-byte challenge",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • Create SPHINCS+ signature (44 steps)",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • Store encrypted signature on-chain",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • Mark vault as LOCKED",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press ENTER to lock or ESC to cancel",
                Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)
            )),
        ];

        let content = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ LOCK VAULT ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, area);
    }

    fn render_unlock_content(&self, f: &mut Frame, area: Rect) {
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "UNLOCK VAULT",
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Verify post-quantum signature to unlock",
                Style::default().fg(Theme::TEXT)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Unlocking requires:",
                Style::default().fg(Theme::SUBTEXT1).add_modifier(Modifier::BOLD)
            )),
            Line::from(Span::styled(
                "  • 44-step SPHINCS+ signature verification",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • Challenge-response authentication",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(Span::styled(
                "  • On-chain signature validation",
                Style::default().fg(Theme::SUBTEXT1)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "⚠️  This process takes ~30 seconds",
                Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press ENTER to unlock or ESC to cancel",
                Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)
            )),
        ];

        let content = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ UNLOCK VAULT ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, area);
    }

    // Placeholder stubs for other content views
    fn render_transfer_content(&self, f: &mut Frame, area: Rect) {
        let border_color = Color::Rgb(140, 140, 140);

        // Build transfer form
        let mut rows = vec![];

        // Token Type Selection
        let token_type_focused = self.transfer_focused_field == TransferInputField::TokenType;
        let token_type_text = match self.transfer_token_type {
            TransferTokenType::StandardQcoin => "Standard qcoin",
            TransferTokenType::Pqcoin => "pqcoin (PQ-Secured)",
        };
        let token_type_color = if token_type_focused { Theme::CYAN_NEON } else { Theme::TEXT };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                if token_type_focused { "▶ TOKEN TYPE" } else { "  TOKEN TYPE" },
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD),
            )),
        ]).height(1));

        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(token_type_text,
                    Style::default()
                        .fg(token_type_color)
                        .add_modifier(if token_type_focused { Modifier::BOLD } else { Modifier::empty() }),
                ),
            ]),
        ]).height(1));

        rows.push(Row::new(vec![
            Line::from(Span::styled("    [← →] Toggle token type", Style::default().fg(Theme::SUBTEXT1))),
        ]).height(1));

        rows.push(Row::new(vec![Line::from("")]));

        // Recipient Field
        let recipient_focused = self.transfer_focused_field == TransferInputField::Recipient;
        let recipient_color = if recipient_focused { Theme::CYAN_NEON } else { Theme::TEXT };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                if recipient_focused { "▶ RECIPIENT" } else { "  RECIPIENT" },
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD),
            )),
        ]).height(1));

        let recipient_display = if self.transfer_recipient.is_empty() {
            "(Enter recipient address...)".to_string()
        } else {
            self.transfer_recipient.clone()
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                format!("    {}", recipient_display),
                Style::default()
                    .fg(if self.transfer_recipient.is_empty() { Theme::SUBTEXT1 } else { recipient_color })
                    .add_modifier(if recipient_focused { Modifier::BOLD } else { Modifier::empty() }),
            )),
        ]).height(1));

        rows.push(Row::new(vec![Line::from("")]));

        // Amount Field
        let amount_focused = self.transfer_focused_field == TransferInputField::Amount;
        let amount_color = if amount_focused { Theme::CYAN_NEON } else { Theme::TEXT };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                if amount_focused { "▶ AMOUNT" } else { "  AMOUNT" },
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD),
            )),
        ]).height(1));

        let amount_display = if self.transfer_amount.is_empty() {
            "(Enter amount...)".to_string()
        } else {
            format!("{} {}", self.transfer_amount, token_type_text)
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                format!("    {}", amount_display),
                Style::default()
                    .fg(if self.transfer_amount.is_empty() { Theme::SUBTEXT1 } else { amount_color })
                    .add_modifier(if amount_focused { Modifier::BOLD } else { Modifier::empty() }),
            )),
        ]).height(1));

        rows.push(Row::new(vec![Line::from("")]));
        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));
        rows.push(Row::new(vec![Line::from("")]));

        // Controls
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("[Tab/↑↓] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Navigate  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[Enter] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Execute  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[Esc] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Cancel", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        let widths = [Constraint::Percentage(100)];

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ TRANSFER TOKENS ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(0);

        f.render_widget(table, area);
    }

    fn render_wrap_content(&self, f: &mut Frame, area: Rect) {
        self.render_placeholder_content(f, area, "WRAP", "Convert standard qcoin to PQ-secured pqcoin");
    }

    fn render_unwrap_content(&self, f: &mut Frame, area: Rect) {
        self.render_placeholder_content(f, area, "UNWRAP", "Convert PQ-secured pqcoin back to standard qcoin");
    }

    fn render_airdrop_content(&self, f: &mut Frame, area: Rect) {
        self.render_placeholder_content(f, area, "AIRDROP", "Claim 100 tokens (24-hour cooldown)");
    }

    fn render_stats_content(&self, f: &mut Frame, area: Rect) {
        self.render_placeholder_content(f, area, "STATS", "View network statistics and metrics");
    }

    fn render_close_content(&self, f: &mut Frame, area: Rect) {
        self.render_placeholder_content(f, area, "CLOSE", "Close vault and reclaim rent");
    }

    fn render_chart_content(&self, f: &mut Frame, area: Rect) {
        self.render_placeholder_content(f, area, "CHART", "Network metrics and charts");
    }

    fn render_vaults_content(&self, f: &mut Frame, area: Rect) {
        // Render vault management UI based on mode
        match self.vault_management_mode {
            VaultManagementMode::List => self.render_vault_list_content(f, area),
            VaultManagementMode::Create => self.render_vault_create_content(f, area),
        }
    }

    fn render_vault_list_content(&self, f: &mut Frame, area: Rect) {
        // Reuse the popup rendering but without Clear widget
        // This is a temporary solution - ideally we'd refactor to share code
        let border_color = Color::Rgb(140, 140, 140);

        // Build vault list rows
        let mut rows = vec![];

        if self.vault_list.is_empty() {
            rows.push(Row::new(vec![
                Line::from(Span::styled("No vaults found", Style::default().fg(Theme::YELLOW_NEON))),
            ]));
            rows.push(Row::new(vec![Line::from("")]));
            rows.push(Row::new(vec![
                Line::from(Span::styled("Press [N] to create your first vault", Style::default().fg(Theme::SUBTEXT1))),
            ]));
        } else {
            for (i, vault) in self.vault_list.iter().enumerate() {
                let is_selected = i == self.selected_vault_index;
                let indicator = if is_selected { "▶" } else { " " };

                rows.push(Row::new(vec![
                    Line::from(Span::styled(
                        format!("{} {}", indicator, vault.name),
                        Style::default()
                            .fg(if is_selected { Theme::CYAN_NEON } else { Theme::TEXT })
                            .add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() }),
                    )),
                ]));

                rows.push(Row::new(vec![
                    Line::from(Span::styled(
                        format!("   Wallet: {}...{}",
                            &vault.wallet_address[..8],
                            &vault.wallet_address[vault.wallet_address.len()-8..]),
                        Style::default().fg(Theme::SUBTEXT1),
                    )),
                ]));
            }
        }

        rows.push(Row::new(vec![Line::from("")]));
        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));
        rows.push(Row::new(vec![Line::from("")]));

        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("[↑↓] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Navigate  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[Enter] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Switch  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[N] ", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("New  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[D] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Delete", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        let widths = [Constraint::Percentage(100)];

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ VAULT MANAGEMENT ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(1);

        f.render_widget(table, area);
    }

    fn render_vault_create_content(&self, f: &mut Frame, area: Rect) {
        let border_color = Color::Rgb(140, 140, 140);

        let mut rows = vec![];

        rows.push(Row::new(vec![
            Line::from(Span::styled("Create a new quantum-resistant vault", Style::default().fg(Theme::TEXT))),
        ]).height(2));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        let vault_display = if self.new_vault_name.is_empty() {
            "[Enter vault name...]".to_string()
        } else {
            self.new_vault_name.clone()
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("VAULT NAME", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled(vault_display, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
        ]));

        rows.push(Row::new(vec![Line::from("")]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("• New keys will be auto-generated", Style::default().fg(Theme::SUBTEXT1))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("• Vault will be automatically activated", Style::default().fg(Theme::SUBTEXT1))),
        ]));

        rows.push(Row::new(vec![Line::from("")]));

        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("Enter", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" to create • ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("Esc", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" to go back", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        let widths = [Constraint::Percentage(100)];

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ CREATE NEW VAULT ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(2);

        f.render_widget(table, area);
    }

    fn render_placeholder_content(&self, f: &mut Frame, area: Rect, title: &str, description: &str) {
        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);

        let text = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                title,
                Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from(Span::styled(
                description,
                Style::default().fg(Theme::TEXT)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press ENTER to execute or ESC to cancel",
                Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)
            )),
        ];

        let content = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(format!(" ┃ {} ┃ ", title))
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, area);
    }

    pub fn render_unlock_splash_animation(&self, f: &mut Frame, area: Rect) {
        // Glitch characters for animation (same as splash screen)
        let glitch_chars = vec!["█", "▓", "▒", "░", "▀", "▄", "▌", "▐", "■", "□"];

        // Generate animated glitch pattern using animation frame (60 FPS)
        let seed = self.animation_frame as usize;

        let glitch_top = format!("{}{}{}{}",
            glitch_chars[seed % glitch_chars.len()],
            glitch_chars[(seed + 1) % glitch_chars.len()],
            glitch_chars[(seed + 2) % glitch_chars.len()],
            glitch_chars[(seed + 3) % glitch_chars.len()],
        );

        let glitch_mid = format!(" {}{}{}{}{} ",
            glitch_chars[(seed + 4) % glitch_chars.len()],
            glitch_chars[(seed + 5) % glitch_chars.len()],
            glitch_chars[(seed + 6) % glitch_chars.len()],
            glitch_chars[(seed + 7) % glitch_chars.len()],
            glitch_chars[(seed + 8) % glitch_chars.len()],
        );

        let glitch_bot = format!("{}{}{}",
            glitch_chars[(seed + 9) % glitch_chars.len()],
            glitch_chars[(seed + 10) % glitch_chars.len()],
            glitch_chars[(seed + 11) % glitch_chars.len()],
        );

        // Content with glitch animation
        let content_lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(""),
            // Glitch effect top
            Line::from(vec![
                Span::styled(glitch_top.clone(), Style::default().fg(Color::Rgb(0, 150, 200))),
                Span::styled(glitch_mid.clone(), Style::default().fg(Color::Rgb(140, 140, 140))),
                Span::styled(glitch_bot.clone(), Style::default().fg(Color::Rgb(180, 0, 200))),
            ]),
            Line::from(""),
            // Main message with animation frame indicator
            Line::from(vec![
                Span::styled("U", Style::default().fg(Color::Rgb(120, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("N", Style::default().fg(Color::Rgb(140, 80, 220)).add_modifier(Modifier::BOLD)),
                Span::styled("L", Style::default().fg(Color::Rgb(120, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("O", Style::default().fg(Color::Rgb(100, 50, 180)).add_modifier(Modifier::BOLD)),
                Span::styled("C", Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("K", Style::default().fg(Color::Rgb(160, 80, 220)).add_modifier(Modifier::BOLD)),
                Span::styled("I", Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("N", Style::default().fg(Color::Rgb(120, 50, 180)).add_modifier(Modifier::BOLD)),
                Span::styled("G", Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}", glitch_chars[seed % glitch_chars.len()]), Style::default().fg(Color::Rgb(120, 50, 180)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}", glitch_chars[(seed + 1) % glitch_chars.len()]), Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}", glitch_chars[(seed + 2) % glitch_chars.len()]), Style::default().fg(Color::Rgb(160, 80, 220)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            // Glitch effect bottom
            Line::from(vec![
                Span::styled(glitch_bot, Style::default().fg(Color::Rgb(180, 0, 200))),
                Span::styled(glitch_mid, Style::default().fg(Color::Rgb(140, 140, 140))),
                Span::styled(glitch_top, Style::default().fg(Color::Rgb(0, 150, 200))),
            ]),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("SPHINCS+ SHA2-128s  •  NIST FIPS 205  •  Quantum-Resistant", Style::default().fg(Color::Rgb(100, 100, 100))),
            ]),
        ];

        let border_color = Color::Rgb(140, 140, 140);
        let content = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" UNLOCK ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, area);
    }

    pub fn render_lock_splash_animation(&self, f: &mut Frame, area: Rect) {
        // Glitch characters for animation (same as splash screen)
        let glitch_chars = vec!["█", "▓", "▒", "░", "▀", "▄", "▌", "▐", "■", "□"];

        // Generate animated glitch pattern using animation frame (60 FPS)
        let seed = self.animation_frame as usize;

        let glitch_top = format!("{}{}{}{}",
            glitch_chars[seed % glitch_chars.len()],
            glitch_chars[(seed + 1) % glitch_chars.len()],
            glitch_chars[(seed + 2) % glitch_chars.len()],
            glitch_chars[(seed + 3) % glitch_chars.len()],
        );

        let glitch_mid = format!(" {}{}{}{}{} ",
            glitch_chars[(seed + 4) % glitch_chars.len()],
            glitch_chars[(seed + 5) % glitch_chars.len()],
            glitch_chars[(seed + 6) % glitch_chars.len()],
            glitch_chars[(seed + 7) % glitch_chars.len()],
            glitch_chars[(seed + 8) % glitch_chars.len()],
        );

        let glitch_bot = format!("{}{}{}",
            glitch_chars[(seed + 9) % glitch_chars.len()],
            glitch_chars[(seed + 10) % glitch_chars.len()],
            glitch_chars[(seed + 11) % glitch_chars.len()],
        );

        // Content with glitch animation
        let content_lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(""),
            // Glitch effect top
            Line::from(vec![
                Span::styled(glitch_top.clone(), Style::default().fg(Color::Rgb(0, 150, 200))),
                Span::styled(glitch_mid.clone(), Style::default().fg(Color::Rgb(140, 140, 140))),
                Span::styled(glitch_bot.clone(), Style::default().fg(Color::Rgb(180, 0, 200))),
            ]),
            Line::from(""),
            // Main message
            Line::from(vec![
                Span::styled("L", Style::default().fg(Color::Rgb(120, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("O", Style::default().fg(Color::Rgb(140, 80, 220)).add_modifier(Modifier::BOLD)),
                Span::styled("C", Style::default().fg(Color::Rgb(120, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("K", Style::default().fg(Color::Rgb(100, 50, 180)).add_modifier(Modifier::BOLD)),
                Span::styled("I", Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled("N", Style::default().fg(Color::Rgb(160, 80, 220)).add_modifier(Modifier::BOLD)),
                Span::styled("G", Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled(".", Style::default().fg(Color::Rgb(120, 50, 180)).add_modifier(Modifier::BOLD)),
                Span::styled(".", Style::default().fg(Color::Rgb(140, 60, 200)).add_modifier(Modifier::BOLD)),
                Span::styled(".", Style::default().fg(Color::Rgb(160, 80, 220)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            // Glitch effect bottom
            Line::from(vec![
                Span::styled(glitch_bot, Style::default().fg(Color::Rgb(180, 0, 200))),
                Span::styled(glitch_mid, Style::default().fg(Color::Rgb(140, 140, 140))),
                Span::styled(glitch_top, Style::default().fg(Color::Rgb(0, 150, 200))),
            ]),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("SPHINCS+ SHA2-128s  •  NIST FIPS 205  •  Quantum-Resistant", Style::default().fg(Color::Rgb(100, 100, 100))),
            ]),
        ];

        let border_color = Color::Rgb(140, 140, 140);
        let content = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" LOCK ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, area);
    }
}
