use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Clear, Paragraph, Row, Table, Wrap},
};
use crate::dashboard::types::*;
use crate::icons::Icons;
use crate::theme::Theme;
use crate::vault_manager::VaultConfig;

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl Dashboard {
    pub fn render_action_popup(&self, f: &mut Frame, area: Rect, title: &str, title_color: Color) {
        let popup_area = centered_rect(85, 75, area);  // Wider popup for long messages

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build content lines instead of table rows for better text wrapping
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
                let max_width = (popup_area.width as usize).saturating_sub(20);
                if message.len() > max_width {
                    // First line with step label and icon
                    content_lines.push(Line::from(vec![
                        Span::styled(step_label, Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
                    ]));

                    // Message on next line with icon, allowing wrapping
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

        // Add spacing and controls
        content_lines.push(Line::from(""));
        content_lines.push(Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))));
        content_lines.push(Line::from(""));
        content_lines.push(Line::from(vec![
            Span::styled(" [ESC] ", Style::default().fg(Theme::BASE).bg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
            Span::styled(" Close", Style::default().fg(Theme::TEXT)),
        ]));

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

        let paragraph = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(format!(" ┃ {} ┃ ", title))
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(paragraph, popup_area);
    }
    pub fn render_unlock_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 50, area);
        f.render_widget(Clear, popup_area);

        // Glitch characters for animation (same as splash screen)
        let glitch_chars = vec!["█", "▓", "▒", "░", "▀", "▄", "▌", "▐", "■", "□"];

        // Generate animated glitch pattern using animation frame
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
            // Glitch effect top
            Line::from(vec![
                Span::styled(glitch_top.clone(), Style::default().fg(Color::Rgb(0, 150, 200))),
                Span::styled(glitch_mid.clone(), Style::default().fg(Color::Rgb(140, 140, 140))),
                Span::styled(glitch_bot.clone(), Style::default().fg(Color::Rgb(180, 0, 200))),
            ]),
            Line::from(""),
            // Main message
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
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [ESC] ", Style::default().fg(Theme::BASE).bg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::TEXT)),
            ]),
        ];

        let content = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(140, 140, 140)).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, popup_area);
    }
    pub fn render_lock_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 50, area);
        f.render_widget(Clear, popup_area);

        // Glitch characters for animation (same as splash screen)
        let glitch_chars = vec!["█", "▓", "▒", "░", "▀", "▄", "▌", "▐", "■", "□"];

        // Generate animated glitch pattern using animation frame
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
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [ESC] ", Style::default().fg(Theme::BASE).bg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::TEXT)),
            ]),
        ];

        let content = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(140, 140, 140)).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Center);

        f.render_widget(content, popup_area);
    }
    pub fn render_transfer_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 70, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for transfer form
        let mut rows = vec![];

        // Show both balances
        let standard_balance_qdum = self.standard_balance.map(|b| b as f64 / 1_000_000.0).unwrap_or(0.0);
        let pq_balance_qdum = self.pq_balance.map(|b| b as f64 / 1_000_000.0).unwrap_or(0.0);

        rows.push(Row::new(vec![
            Line::from(Span::styled("STANDARD QCOIN", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(
                format!("{:.6} qcoin", standard_balance_qdum),
                Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("PQ QCOIN", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(
                format!("{:.6} pqcoin", pq_balance_qdum),
                Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Token type selector
        let token_type_color = if self.transfer_focused_field == TransferInputField::TokenType {
            Theme::YELLOW_NEON
        } else {
            Theme::TEXT
        };

        let (token_type_display, token_note) = match self.transfer_token_type {
            TransferTokenType::StandardQcoin => ("Standard qcoin", "Can transfer without unlocking"),
            TransferTokenType::Pqcoin => ("pqcoin", "Requires vault unlock"),
        };

        let token_type_indicator = if self.transfer_focused_field == TransferInputField::TokenType {
            " ◀ ACTIVE"
        } else {
            ""
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("TOKEN TYPE", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(token_type_display, Style::default().fg(token_type_color).add_modifier(Modifier::BOLD)),
                Span::styled(token_type_indicator, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
            ]),
        ]));

        if self.transfer_focused_field == TransferInputField::TokenType {
            rows.push(Row::new(vec![
                Line::from(""),
                Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
            ]));
        }

        rows.push(Row::new(vec![
            Line::from(""),
            Line::from(Span::styled(token_note, Style::default().fg(Theme::DIM))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Recipient field
        let recipient_color = if self.transfer_focused_field == TransferInputField::Recipient {
            Theme::YELLOW_NEON
        } else {
            Theme::TEXT
        };

        let recipient_display = if self.transfer_recipient.is_empty() {
            "[Enter wallet address...]".to_string()
        } else {
            self.transfer_recipient.clone()
        };

        let recipient_indicator = if self.transfer_focused_field == TransferInputField::Recipient {
            " ◀ ACTIVE"
        } else {
            ""
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("RECIPIENT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(recipient_display, Style::default().fg(recipient_color).add_modifier(Modifier::BOLD)),
                Span::styled(recipient_indicator, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
            ]),
        ]));

        if self.transfer_focused_field == TransferInputField::Recipient {
            rows.push(Row::new(vec![
                Line::from(""),
                Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
            ]));
        }

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Amount field
        let amount_color = if self.transfer_focused_field == TransferInputField::Amount {
            Theme::YELLOW_NEON
        } else {
            Theme::TEXT
        };

        let token_symbol = match self.transfer_token_type {
            TransferTokenType::StandardQcoin => "qcoin",
            TransferTokenType::Pqcoin => "pqcoin",
        };

        let amount_display = if self.transfer_amount.is_empty() {
            "[Enter amount...]".to_string()
        } else {
            format!("{} {}", self.transfer_amount, token_symbol)
        };

        let amount_indicator = if self.transfer_focused_field == TransferInputField::Amount {
            " ◀ ACTIVE"
        } else {
            ""
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("AMOUNT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(amount_display, Style::default().fg(amount_color).add_modifier(Modifier::BOLD)),
                Span::styled(amount_indicator, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
            ]),
        ]));

        if self.transfer_focused_field == TransferInputField::Amount {
            rows.push(Row::new(vec![
                Line::from(""),
                Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
            ]));
        }

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Controls row
        rows.push(Row::new(vec![
            Line::from(Span::styled("CONTROLS", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(" [Tab/↑↓] ", Style::default().fg(Theme::TEXT).bg(Theme::BLUE).add_modifier(Modifier::BOLD)),
                Span::styled(" Switch  ", Style::default().fg(Theme::TEXT)),
                Span::styled(" [←→] ", Style::default().fg(Theme::TEXT).bg(Theme::PURPLE).add_modifier(Modifier::BOLD)),
                Span::styled(" Type  ", Style::default().fg(Theme::TEXT)),
            ]),
        ]));

        rows.push(Row::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(Theme::TEXT).bg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" Send  ", Style::default().fg(Theme::TEXT)),
                Span::styled(" [Esc] ", Style::default().fg(Theme::TEXT).bg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::TEXT)),
            ]),
        ]));

        let widths = [Constraint::Length(14), Constraint::Min(38)];

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

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
            .column_spacing(2);

        f.render_widget(table, popup_area);
    }
    pub fn render_wrap_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 50, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for wrap form
        let mut rows = vec![];

        // Standard CASH balance row
        if let Some(balance) = self.standard_balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            rows.push(Row::new(vec![
                Line::from(Span::styled("STANDARD QCOIN", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(
                    format!("{:.6} qcoin", balance_qdum),
                    Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD),
                )),
            ]));

            rows.push(Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]));
        }

        // Direction indicator
        rows.push(Row::new(vec![
            Line::from(Span::styled("DIRECTION", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Standard qcoin → pqcoin", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Amount field
        let amount_display = if self.bridge_amount.is_empty() {
            "[Enter amount...]".to_string()
        } else {
            format!("{} qcoin", self.bridge_amount)
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("AMOUNT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(amount_display, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" ◀ ACTIVE", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
            ]),
        ]));

        rows.push(Row::new(vec![
            Line::from(""),
            Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Controls row
        rows.push(Row::new(vec![
            Line::from(Span::styled("CONTROLS", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(Theme::TEXT).bg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" Wrap  ", Style::default().fg(Theme::TEXT)),
                Span::styled(" [Esc] ", Style::default().fg(Theme::TEXT).bg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::TEXT)),
            ]),
        ]));

        let widths = [Constraint::Length(14), Constraint::Min(38)];

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ WRAP TO PQCOIN ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(2);

        f.render_widget(table, popup_area);
    }
    pub fn render_unwrap_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 50, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for unwrap form
        let mut rows = vec![];

        // PQ CASH balance row
        if let Some(balance) = self.pq_balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            rows.push(Row::new(vec![
                Line::from(Span::styled("PQ QCOIN", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(
                    format!("{:.6} pqcoin", balance_qdum),
                    Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD),
                )),
            ]));

            rows.push(Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]));
        }

        // Direction indicator
        rows.push(Row::new(vec![
            Line::from(Span::styled("DIRECTION", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("pqcoin → Standard qcoin", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Amount field
        let amount_display = if self.bridge_amount.is_empty() {
            "[Enter amount...]".to_string()
        } else {
            format!("{} pqcoin", self.bridge_amount)
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("AMOUNT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(amount_display, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" ◀ ACTIVE", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
            ]),
        ]));

        rows.push(Row::new(vec![
            Line::from(""),
            Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Controls row
        rows.push(Row::new(vec![
            Line::from(Span::styled("CONTROLS", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(Theme::TEXT).bg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" Unwrap  ", Style::default().fg(Theme::TEXT)),
                Span::styled(" [Esc] ", Style::default().fg(Theme::TEXT).bg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::TEXT)),
            ]),
        ]));

        let widths = [Constraint::Length(14), Constraint::Min(38)];

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ UNWRAP TO STANDARD QCOIN ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(2);

        f.render_widget(table, popup_area);
    }
    pub fn render_vault_switch_popup(&self, f: &mut Frame, area: Rect) {
        match self.vault_management_mode {
            VaultManagementMode::List => self.render_vault_list(f, area),
            VaultManagementMode::Create => self.render_vault_create(f, area),
        }
    }
    pub fn render_vault_list(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 70, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Get active vault name
        let active_vault_name = if let Ok(config) = VaultConfig::load() {
            config.active_vault
        } else {
            None
        };

        // Build table rows for vault list
        let mut rows = vec![];

        // Header
        rows.push(Row::new(vec![
            Line::from(Span::styled("Select a vault to switch, or create a new one", Style::default().fg(Theme::TEXT))),
        ]).height(2));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Vault list
        for (i, vault) in self.vault_list.iter().enumerate() {
            let is_active = active_vault_name.as_ref() == Some(&vault.name);
            let is_selected = self.selected_vault_index == i;

            let indicator = if is_active { "●" } else { "○" };
            let status = if is_active { " [ACTIVE]" } else { "" };

            let name_color = if is_selected {
                Theme::YELLOW_NEON
            } else if is_active {
                Theme::GREEN_NEON
            } else {
                Theme::TEXT
            };

            let indicator_color = if is_active {
                Theme::GREEN_NEON
            } else {
                Theme::DIM
            };

            let mut spans = vec![
                Span::styled(" ", Style::default()),
                Span::styled(indicator, Style::default().fg(indicator_color).add_modifier(Modifier::BOLD)),
                Span::styled("  ", Style::default()),
                Span::styled(&vault.name, Style::default().fg(name_color).add_modifier(Modifier::BOLD)),
            ];

            if is_active {
                spans.push(Span::styled(status, Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)));
            }

            if is_selected {
                spans.insert(0, Span::styled("▶ ", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)));
            } else {
                spans.insert(0, Span::styled("  ", Style::default()));
            }

            rows.push(Row::new(vec![Line::from(spans)]));

            // Wallet address
            let wallet_info = if !vault.wallet_address.is_empty() {
                format!("     └─ {}", vault.short_wallet())
            } else {
                "     └─ (not initialized)".to_string()
            };

            rows.push(Row::new(vec![
                Line::from(Span::styled(wallet_info, Style::default().fg(Theme::DIM))),
            ]));
        }

        // Separator
        rows.push(Row::new(vec![Line::from("")]));

        // "Create New Vault" option
        let is_create_selected = self.selected_vault_index == self.vault_list.len();
        let create_color = if is_create_selected {
            Theme::YELLOW_NEON
        } else {
            Theme::GREEN_NEON
        };

        let mut create_spans = vec![
            Span::styled(" + ", Style::default().fg(create_color).add_modifier(Modifier::BOLD)),
            Span::styled("Create New Vault", Style::default().fg(create_color).add_modifier(Modifier::BOLD)),
        ];

        if is_create_selected {
            create_spans.insert(0, Span::styled("▶ ", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)));
        } else {
            create_spans.insert(0, Span::styled("  ", Style::default()));
        }

        rows.push(Row::new(vec![Line::from(create_spans)]));

        rows.push(Row::new(vec![Line::from("")]));

        // Controls - Line 1
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("↑↓/jk", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" Navigate  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("Enter", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" Select  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("N", Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD)),
                Span::styled(" New", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        // Controls - Line 2
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("D", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" Delete  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("Esc", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        let widths = [Constraint::Percentage(100)];

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

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

        f.render_widget(table, popup_area);
    }
    pub fn render_vault_create(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 40, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for vault creation form
        let mut rows = vec![];

        // Instruction row
        rows.push(Row::new(vec![
            Line::from(Span::styled("Create a new quantum-resistant vault", Style::default().fg(Theme::TEXT))),
        ]).height(2));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Vault name field
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

        // Info row
        rows.push(Row::new(vec![
            Line::from(Span::styled("• New keys will be auto-generated", Style::default().fg(Theme::SUBTEXT1))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("• Vault will be automatically activated", Style::default().fg(Theme::SUBTEXT1))),
        ]));

        rows.push(Row::new(vec![Line::from("")]));

        // Controls row
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

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

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

        f.render_widget(table, popup_area);
    }
    pub fn render_transfer_result_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(75, 60, area);

        // Clear background - IMPORTANT for visibility
        f.render_widget(Clear, popup_area);

        // Determine if there are any errors
        let has_error = self.action_steps.iter().any(|step| matches!(step, ActionStep::Error(_)));
        let success = !has_error;

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);
        let title = if success { " ┃ SUCCESS ┃ " } else { " ┃ ERROR ┃ " };

        // Build content lines with improved formatting
        let mut content_lines = vec![Line::from("")];

        // Display all action steps with better formatting
        if self.action_steps.is_empty() {
            content_lines.push(Line::from(Span::styled(
                "No result to display",
                Style::default().fg(Theme::SUBTEXT1).add_modifier(Modifier::ITALIC)
            )));
        } else {
            for step in &self.action_steps {
                match step {
                    ActionStep::Starting => {
                        content_lines.push(Line::from(Span::styled(
                            "  ⏳ Starting...",
                            Style::default().fg(Theme::YELLOW_NEON)
                        )));
                    }
                    ActionStep::InProgress(msg) => {
                        // Regular info messages (left-aligned)
                        if msg.is_empty() {
                            content_lines.push(Line::from(""));
                        } else {
                            content_lines.push(Line::from(Span::styled(
                                format!("  {}", msg),
                                Style::default().fg(Theme::SUBTEXT1)
                            )));
                        }
                    }
                    ActionStep::Success(msg) => {
                        // Success messages (green, bold)
                        if msg.starts_with("╔") || msg.starts_with("║") || msg.starts_with("╚") {
                            // Box characters - keep centered
                            content_lines.push(Line::from(Span::styled(
                                msg.clone(),
                                Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)
                            )));
                        } else if msg.is_empty() {
                            content_lines.push(Line::from(""));
                        } else {
                            content_lines.push(Line::from(Span::styled(
                                format!("  ✓ {}", msg),
                                Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)
                            )));
                        }
                    }
                    ActionStep::Error(msg) => {
                        // Error messages (red, bold)
                        if msg.starts_with("╔") || msg.starts_with("║") || msg.starts_with("╚") {
                            // Box characters - keep as is
                            content_lines.push(Line::from(Span::styled(
                                msg.clone(),
                                Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)
                            )));
                        } else if msg.starts_with("❌") {
                            // Already has emoji, keep as is
                            content_lines.push(Line::from(Span::styled(
                                format!("  {}", msg),
                                Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)
                            )));
                        } else if msg.is_empty() {
                            content_lines.push(Line::from(""));
                        } else if msg.starts_with("Error:") {
                            // Main error message - highlight differently
                            content_lines.push(Line::from(""));
                            content_lines.push(Line::from(Span::styled(
                                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                                Style::default().fg(Theme::RED_NEON)
                            )));
                            content_lines.push(Line::from(Span::styled(
                                format!("  ⚠️  {}", msg),
                                Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)
                            )));
                            content_lines.push(Line::from(Span::styled(
                                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                                Style::default().fg(Theme::RED_NEON)
                            )));
                            content_lines.push(Line::from(""));
                        } else {
                            content_lines.push(Line::from(Span::styled(
                                format!("  ✗ {}", msg),
                                Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)
                            )));
                        }
                    }
                };
            }
        }

        // Add spacing before controls
        content_lines.push(Line::from(""));
        content_lines.push(Line::from(""));

        // Add instruction
        content_lines.push(Line::from(vec![
            Span::styled(" [ESC] ", Style::default().fg(Theme::BASE).bg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
            Span::styled(" Close this window", Style::default().fg(Theme::TEXT)),
        ]));

        let title_color = if success { Theme::GREEN_NEON } else { Theme::RED_NEON };

        let content = Paragraph::new(content_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(title)
                    .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Left);

        f.render_widget(content, popup_area);
    }
    pub fn render_close_confirm_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 45, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for close confirmation
        let mut rows = vec![];

        // Warning header
        rows.push(Row::new(vec![
            Line::from(Span::styled(
                "⚠️  CLOSE PQ ACCOUNT & RECLAIM RENT ⚠️",
                Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Info text
        rows.push(Row::new(vec![
            Line::from(Span::styled(
                format!("Closing PQ account for vault: {}", self.vault_to_close),
                Style::default().fg(Theme::TEXT),
            )),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                "This will:",
                Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                "  • Close your on-chain PQ account",
                Style::default().fg(Theme::SUBTEXT1),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                "  • Refund ~0.003 SOL rent to your wallet",
                Style::default().fg(Theme::GREEN_NEON),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                "  • Keep your vault config and keys intact",
                Style::default().fg(Theme::SUBTEXT1),
            )),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        // Instruction
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("Type ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(&self.vault_to_close, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" to confirm:", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        // Input field
        let input_display = if self.close_confirmation_input.is_empty() {
            "[type vault name here...]"
        } else {
            &self.close_confirmation_input
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                input_display,
                Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD),
            )),
        ]));

        // Underline for input field
        rows.push(Row::new(vec![
            Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        // Controls
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("[Enter] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Confirm  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[Esc] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Cancel", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

        // Create table
        let table = Table::new(
            rows,
            [Constraint::Percentage(100)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                .title(" ┃ CLOSE PQ ACCOUNT ┃ ")
                .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Theme::BASE)),
        );

        f.render_widget(table, popup_area);
    }
    pub fn render_chart_popup(&self, f: &mut Frame, area: Rect) {
        use ratatui::widgets::{Dataset, GraphType};
        use ratatui::symbols;
        use chrono::{DateTime, Utc, Duration as ChronoDuration};

        let popup_area = centered_rect(98, 95, area);  // Full screen chart

        // Clear background and render a solid background block
        f.render_widget(Clear, popup_area);

        // Fill entire popup area with background color
        let bg_block = Block::default()
            .style(Style::default().bg(Theme::BASE));
        f.render_widget(bg_block, popup_area);

        // Load network-wide lock history
        let history = LockHistory::load().unwrap_or_else(|_| LockHistory { entries: Vec::new() });

        // Filter entries based on selected timeframe
        let filtered_entries: Vec<&LockHistoryEntry> = if let Some(duration) = self.chart_timeframe.to_duration() {
            let cutoff = Utc::now() - duration;
            let filtered: Vec<&LockHistoryEntry> = history.entries.iter()
                .filter(|entry| {
                    DateTime::parse_from_rfc3339(&entry.timestamp)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc) > cutoff)
                        .unwrap_or(false)
                })
                .collect();

            // Debug: write filter info
            let _ = std::fs::write("/tmp/qdum-chart-filter.log",
                format!("Timeframe: {}\nTotal entries: {}\nFiltered entries: {}\nCutoff: {:?}\n",
                    self.chart_timeframe.to_string(), history.entries.len(), filtered.len(), cutoff));

            filtered
        } else {
            // Show all data
            let _ = std::fs::write("/tmp/qdum-chart-filter.log",
                format!("Timeframe: ALL\nTotal entries: {}\nFiltered entries: {}\n",
                    history.entries.len(), history.entries.len()));
            history.entries.iter().collect()
        };

        // Prepare data for chart with intelligent sampling
        const MAX_POINTS: usize = 150; // Limit chart points for performance and readability

        // Extract the appropriate value based on chart type
        let get_value = |entry: &LockHistoryEntry| -> f64 {
            match self.chart_type {
                ChartType::LockedAmount => entry.locked_amount,
                ChartType::HolderCount => entry.holder_count as f64,
            }
        };

        let data_points: Vec<(f64, f64)> = if filtered_entries.len() <= MAX_POINTS {
            // If we have fewer entries than the max, use all of them
            filtered_entries.iter()
                .enumerate()
                .map(|(i, entry)| (i as f64, get_value(entry)))
                .collect()
        } else {
            // Sample data points evenly across the dataset
            let step = filtered_entries.len() as f64 / MAX_POINTS as f64;
            (0..MAX_POINTS)
                .map(|i| {
                    let index = (i as f64 * step) as usize;
                    let entry = filtered_entries[index.min(filtered_entries.len() - 1)];
                    (i as f64, get_value(entry))
                })
                .collect()
        };

        // Parse timestamps for better labeling
        let (first_time, last_time) = if !filtered_entries.is_empty() {
            let first = filtered_entries.first().and_then(|e| DateTime::parse_from_rfc3339(&e.timestamp).ok());
            let last = filtered_entries.last().and_then(|e| DateTime::parse_from_rfc3339(&e.timestamp).ok());
            (first, last)
        } else {
            (None, None)
        };

        // Calculate dynamic Y-axis bounds with padding for better visualization
        let (y_min, y_max) = if data_points.is_empty() {
            (0.0, 100.0)  // Default range if no data
        } else {
            let values: Vec<f64> = data_points.iter().map(|(_, y)| *y).collect();
            let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            // Add 20% padding above and below the data range
            let range = max_val - min_val;
            let padding = if range > 0.0 { range * 0.2 } else { max_val * 0.2 };

            let padded_min = (min_val - padding).max(0.0);  // Don't go below 0
            let padded_max = max_val + padding;

            // Ensure minimum range of 10 qcoin for readability
            if (padded_max - padded_min) < 10.0 {
                let mid = (padded_max + padded_min) / 2.0;
                (mid - 5.0, mid + 5.0)
            } else {
                (padded_min, padded_max)
            }
        };

        // Create dataset
        let datasets = vec![
            Dataset::default()
                .name("Locked qcoin")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Theme::CYAN_NEON))
                .data(&data_points)
        ];

        // Create chart with dynamic title showing chart type, timeframe, and data count
        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);
        let chart_title = format!(" ┃ {} [{} - {} points] ┃ ",
            self.chart_type.to_string(),
            self.chart_timeframe.to_string(),
            filtered_entries.len());
        let chart = ratatui::widgets::Chart::new(datasets)
            .style(Style::default().bg(Theme::BASE))  // Set background on chart itself
            .block(
                Block::default()
                    .title(chart_title)
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .style(Style::default().bg(Theme::BASE)),
            )
            .x_axis(
                ratatui::widgets::Axis::default()
                    .title("Time →")
                    .style(Style::default().fg(Theme::SUBTEXT1))
                    .bounds([0.0, data_points.len().max(10) as f64])
                    .labels({
                        // Create time-based labels
                        let start_label = if let Some(first) = first_time {
                            format!("{}", first.format("%m/%d %H:%M"))
                        } else {
                            "Start".to_string()
                        };

                        let end_label = if let Some(last) = last_time {
                            format!("{}", last.format("%m/%d %H:%M"))
                        } else {
                            "Now".to_string()
                        };

                        // Calculate middle timestamp
                        let mid_label = if let (Some(first), Some(last)) = (first_time, last_time) {
                            let duration = last.signed_duration_since(first);
                            let mid_time = first + duration / 2;
                            format!("{}", mid_time.format("%m/%d"))
                        } else {
                            "".to_string()
                        };

                        vec![
                            Span::styled(start_label, Style::default().fg(Theme::SUBTEXT1)),
                            Span::styled(mid_label, Style::default().fg(Theme::SUBTEXT1)),
                            Span::styled(end_label, Style::default().fg(Theme::SUBTEXT1)),
                        ]
                    })
            )
            .y_axis(
                ratatui::widgets::Axis::default()
                    .title("Locked qcoin")
                    .style(Style::default().fg(Theme::SUBTEXT1))
                    .bounds([y_min, y_max])
                    .labels(vec![
                        Span::styled(format!("{:.0}", y_min), Style::default().fg(Theme::SUBTEXT1)),
                        Span::styled(format!("{:.0}", (y_min + y_max) / 2.0), Style::default().fg(Theme::SUBTEXT1)),
                        Span::styled(format!("{:.0}", y_max), Style::default().fg(Theme::SUBTEXT1)),
                    ])
            );

        // Create info panel below chart
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Min(10),        // Chart
                Constraint::Length(8),      // Info panel with timeframe controls
            ])
            .split(popup_area);

        // Render chart directly
        f.render_widget(chart, chunks[0]);

        // Get cache age for display
        let cache_age_text = if let Some(age) = self.vault_client.get_network_lock_cache_age() {
            let seconds = age.as_secs();
            if seconds < 60 {
                format!("{}s ago", seconds)
            } else if seconds < 3600 {
                format!("{}m ago", seconds / 60)
            } else {
                format!("{}h ago", seconds / 3600)
            }
        } else {
            "never".to_string()
        };

        // Render info panel
        let info_text = vec![
            Line::from(vec![
                Span::styled("📊 ", Style::default().fg(Theme::CYAN_NEON)),
                Span::styled("Snapshots: ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{} (showing: {})", history.entries.len(), filtered_entries.len()), Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("  |  ", Style::default().fg(Theme::DIM)),
                Span::styled("Network Total: ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(
                    if let Some(last) = history.entries.last() {
                        format!("{:.2} qcoin", last.locked_amount)
                    } else {
                        "No data".to_string()
                    },
                    Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)
                ),
                Span::styled("  |  ", Style::default().fg(Theme::DIM)),
                Span::styled("Updated: ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(cache_age_text, Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),  // Empty line for spacing
            Line::from(vec![
                Span::styled("📊 Chart: ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("[TAB/←→] ", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(
                    if self.chart_type == ChartType::LockedAmount { "⟪ LOCKED qcoin ⟫" } else { "  LOCKED qcoin  " },
                    Style::default().fg(if self.chart_type == ChartType::LockedAmount { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    if self.chart_type == ChartType::HolderCount { "⟪ LOCKED HOLDERS ⟫" } else { "  LOCKED HOLDERS  " },
                    Style::default().fg(if self.chart_type == ChartType::HolderCount { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(vec![
                Span::styled("⌚ Timeframe: ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("[M] ", Style::default().fg(if self.chart_timeframe == ChartTimeframe::FiveMinutes { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)),
                Span::styled("5M  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[1] ", Style::default().fg(if self.chart_timeframe == ChartTimeframe::OneDay { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)),
                Span::styled("1D  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[5] ", Style::default().fg(if self.chart_timeframe == ChartTimeframe::FiveDays { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)),
                Span::styled("5D  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[7] ", Style::default().fg(if self.chart_timeframe == ChartTimeframe::OneWeek { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)),
                Span::styled("1W  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[3] ", Style::default().fg(if self.chart_timeframe == ChartTimeframe::OneMonth { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)),
                Span::styled("1M  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[A] ", Style::default().fg(if self.chart_timeframe == ChartTimeframe::All { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)),
                Span::styled("ALL", Style::default().fg(Theme::SUBTEXT1)),
            ]),
            Line::from(""),  // Empty line for spacing
            Line::from(vec![
                Span::styled("[Esc] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Close  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[R] ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("Refresh  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[L] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("View Log", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ];

        let info_block = Paragraph::new(info_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .style(Style::default().bg(Theme::BASE)),
            )
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(info_block, chunks[1]);
    }
    pub fn render_airdrop_stats_popup(&self, f: &mut Frame, area: Rect) {
        // Full screen popup (98% x 95%)
        let popup_area = centered_rect(98, 95, area);

        // Clear background and render a solid background block
        f.render_widget(Clear, popup_area);

        // Fill entire popup area with background color
        let bg_block = Block::default()
            .style(Style::default().bg(Theme::BASE));
        f.render_widget(bg_block, popup_area);

        // Split layout: Title + Content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(10),    // Content
            ])
            .split(popup_area);

        // Title with static gray border
        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);
        let title = Paragraph::new("┃ AIRDROP POOL STATISTICS ┃")
            .style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                .border_type(BorderType::Double)
                .style(Style::default().bg(Theme::BASE)));
        f.render_widget(title, chunks[0]);

        // Use cached airdrop stats (fetched when entering popup mode)
        let distributed = self.airdrop_distributed;
        let remaining = self.airdrop_remaining;

        const TOTAL_CAP: u64 = 128_849_018_880_000; // 3% cap with 6 decimals
        let distributed_qdum = distributed as f64 / 1_000_000.0;
        let remaining_qdum = remaining as f64 / 1_000_000.0;
        let total_qdum = TOTAL_CAP as f64 / 1_000_000.0;
        let percent_used = (distributed as f64 / TOTAL_CAP as f64 * 100.0);

        // Content area - split into stats and visual
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(12),  // Stats panel
                Constraint::Min(10),     // Visual bar
                Constraint::Length(3),   // Help text
            ])
            .split(chunks[1]);

        // Stats panel
        let stats_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("📦 Total Airdrop Pool:  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.2} qcoin", total_qdum), Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("  (3% of supply)", Style::default().fg(Theme::DIM)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("✅ Distributed:         ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.2} qcoin", distributed_qdum), Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({:.3}%)", percent_used), Style::default().fg(Theme::GREEN)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💎 Remaining:           ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.2} qcoin", remaining_qdum), Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({:.3}%)", 100.0 - percent_used), Style::default().fg(Theme::YELLOW)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📊 Claims Possible:     ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.0} more", remaining_qdum / 100.0), Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("  (@ 100 qcoin each)", Style::default().fg(Theme::DIM)),
            ]),
        ];

        let stats = Paragraph::new(stats_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                .border_type(BorderType::Double)
                .title(" Pool Status ")
                .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Theme::BASE)))
            .alignment(Alignment::Left);
        f.render_widget(stats, content_chunks[0]);

        // Load airdrop history and create chart showing remaining claims over time
        use ratatui::widgets::{Dataset, GraphType};
        use ratatui::symbols;
        use chrono::{DateTime, Utc};

        let history = AirdropHistory::load().unwrap_or_else(|_| AirdropHistory { entries: Vec::new() });

        // Filter entries based on selected timeframe
        let filtered_entries: Vec<&AirdropHistoryEntry> = if let Some(duration) = self.airdrop_timeframe.to_duration() {
            let cutoff = Utc::now() - duration;
            history.entries.iter()
                .filter(|entry| {
                    DateTime::parse_from_rfc3339(&entry.timestamp)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc) > cutoff)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            history.entries.iter().collect()
        };

        // Prepare data for chart with intelligent sampling
        const MAX_POINTS: usize = 150; // Limit chart points for performance and readability

        let data_points: Vec<(f64, f64)> = if filtered_entries.is_empty() {
            // No history, show current point
            vec![(0.0, remaining_qdum)]
        } else if filtered_entries.len() <= MAX_POINTS {
            // If we have fewer entries than the max, use all of them
            filtered_entries.iter()
                .enumerate()
                .map(|(i, entry)| (i as f64, entry.remaining))
                .collect()
        } else {
            // Sample data points evenly across the dataset
            let step = filtered_entries.len() as f64 / MAX_POINTS as f64;
            (0..MAX_POINTS)
                .map(|i| {
                    let index = (i as f64 * step) as usize;
                    let entry = filtered_entries[index.min(filtered_entries.len() - 1)];
                    (i as f64, entry.remaining)
                })
                .collect()
        };

        // Parse timestamps for better labeling
        let (first_time, last_time) = if !filtered_entries.is_empty() {
            let first = filtered_entries.first().and_then(|e| DateTime::parse_from_rfc3339(&e.timestamp).ok());
            let last = filtered_entries.last().and_then(|e| DateTime::parse_from_rfc3339(&e.timestamp).ok());
            (first, last)
        } else {
            (None, None)
        };

        // Calculate dynamic Y-axis bounds with padding for better visualization
        let (y_min, y_max) = if data_points.is_empty() {
            (0.0, 100.0)  // Default range if no data
        } else {
            let values: Vec<f64> = data_points.iter().map(|(_, y)| *y).collect();
            let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            // Add 20% padding above and below the data range
            let range = max_val - min_val;
            let padding = if range > 0.0 { range * 0.2 } else { max_val * 0.2 };

            let padded_min = (min_val - padding).max(0.0);  // Don't go below 0
            let padded_max = max_val + padding;

            // Ensure minimum range for readability
            if (padded_max - padded_min) < 100.0 {
                let mid = (padded_max + padded_min) / 2.0;
                (mid - 50.0, mid + 50.0)
            } else {
                (padded_min, padded_max)
            }
        };

        let datasets = vec![
            Dataset::default()
                .name("Remaining Claims")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Theme::YELLOW_NEON))
                .data(&data_points)
        ];

        let chart = ratatui::widgets::Chart::new(datasets)
            .style(Style::default().bg(Theme::BASE))  // Set background on chart itself
            .block(
                Block::default()
                    .title(format!(" ┃ Airdrop Pool Depletion [{} - {} snapshots] ┃ ",
                        self.airdrop_timeframe.to_string(),
                        filtered_entries.len()))
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .style(Style::default().bg(Theme::BASE)),
            )
            .x_axis(
                ratatui::widgets::Axis::default()
                    .title("Time →")
                    .style(Style::default().fg(Theme::SUBTEXT1))
                    .bounds([0.0, data_points.len().max(10) as f64])
                    .labels({
                        // Create time-based labels
                        let start_label = if let Some(first) = first_time {
                            format!("{}", first.format("%m/%d %H:%M"))
                        } else {
                            "Start".to_string()
                        };

                        let end_label = if let Some(last) = last_time {
                            format!("{}", last.format("%m/%d %H:%M"))
                        } else {
                            "Now".to_string()
                        };

                        // Calculate middle timestamp
                        let mid_label = if let (Some(first), Some(last)) = (first_time, last_time) {
                            let duration = last.signed_duration_since(first);
                            let mid_time = first + duration / 2;
                            format!("{}", mid_time.format("%m/%d"))
                        } else {
                            "".to_string()
                        };

                        vec![
                            Span::styled(start_label, Style::default().fg(Theme::SUBTEXT1)),
                            Span::styled(mid_label, Style::default().fg(Theme::SUBTEXT1)),
                            Span::styled(end_label, Style::default().fg(Theme::SUBTEXT1)),
                        ]
                    })
            )
            .y_axis(
                ratatui::widgets::Axis::default()
                    .title("Remaining qcoin")
                    .style(Style::default().fg(Theme::SUBTEXT1))
                    .bounds([y_min, y_max])
                    .labels(vec![
                        Span::styled(format!("{:.0}", y_min), Style::default().fg(Theme::SUBTEXT1)),
                        Span::styled(format!("{:.0}", (y_min + y_max) / 2.0), Style::default().fg(Theme::SUBTEXT1)),
                        Span::styled(format!("{:.0}", y_max), Style::default().fg(Theme::SUBTEXT1)),
                    ])
            );

        // Render chart directly
        f.render_widget(chart, content_chunks[1]);

        // Help text
        let help_text = vec![
            Line::from(vec![
                Span::styled("[Esc] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Close  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[M] ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("5Min  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[1] ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("1D  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[5] ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("5D  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[7] ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("1W  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[3] ", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
                Span::styled("1M  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[A] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("All", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ];
        let help = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                .border_type(BorderType::Double)
                .style(Style::default().bg(Theme::BASE)));
        f.render_widget(help, content_chunks[2]);
    }
    pub fn render_delete_confirm_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 40, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for delete confirmation
        let mut rows = vec![];

        // Warning header
        rows.push(Row::new(vec![
            Line::from(Span::styled(
                "⚠️  WARNING: PERMANENT DELETION ⚠️",
                Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Info text
        rows.push(Row::new(vec![
            Line::from(Span::styled(
                format!("Deleting vault: {}", self.vault_to_delete),
                Style::default().fg(Theme::TEXT),
            )),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        // Instruction
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("Type ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(&self.vault_to_delete, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(" to confirm:", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        // Input field
        let input_display = if self.delete_confirmation_input.is_empty() {
            "[type vault name here...]"
        } else {
            &self.delete_confirmation_input
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled(
                input_display,
                Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD),
            )),
        ]));

        // Underline for input field
        rows.push(Row::new(vec![
            Line::from(Span::styled("▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔", Style::default().fg(Theme::YELLOW_NEON))),
        ]));

        rows.push(Row::new(vec![Line::from("")])); // Empty line

        // Controls
        rows.push(Row::new(vec![
            Line::from(vec![
                Span::styled("[Enter] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Confirm  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[Esc] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Cancel", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ]));

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

        // Create table
        let table = Table::new(
            rows,
            [Constraint::Percentage(100)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                .title(" ┃ DELETE VAULT ┃ ")
                .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Theme::BASE)),
        );

        f.render_widget(table, popup_area);
    }
    pub fn render_help_overlay(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(Span::styled(
                "pqcash VAULT - HELP",
                Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Navigation:", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::styled("  ↑/↓ or j/k  - Navigate actions", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  Enter       - Execute selected action", Style::default().fg(Theme::TEXT))),
            Line::from(""),
            Line::from(vec![
                Span::styled("Actions:", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::styled("  G or 1      - Register PQ account", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  L           - Lock vault", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  U           - Unlock vault", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  T or 2      - Transfer tokens", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  A           - Claim 100 qcoin airdrop (24h cooldown)", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  P           - View airdrop pool statistics", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  X or 3      - Close PQ account & reclaim rent", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  R           - Refresh status", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  C           - Copy wallet address", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  V           - Switch vault", Style::default().fg(Theme::TEXT))),
            Line::from(""),
            Line::from(vec![
                Span::styled("Other:", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::styled("  H or ?      - Show this help", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  Q or Esc    - Quit dashboard", Style::default().fg(Theme::TEXT))),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to close help",
                Style::default().fg(Theme::YELLOW_NEON),
            )),
        ];

        // Center the help box
        let help_area = centered_rect(60, 60, area);

        // Clear the background
        f.render_widget(Clear, help_area);

        // Static gray border matching main dashboard
        let border_color = Color::Rgb(140, 140, 140);

        let help_paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(" ┃ HELP ┃ ")
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(help_paragraph, help_area);
    }
}
