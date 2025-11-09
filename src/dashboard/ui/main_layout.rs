use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph, Row, Table},
};
use crate::dashboard::types::*;
use crate::icons::Icons;
use crate::theme::Theme;
use crate::vault_manager::VaultConfig;

impl Dashboard {
    pub fn render_status_panel(&self, f: &mut Frame, area: Rect) {
        // Get active vault name
        let vault_name = if let Ok(config) = VaultConfig::load() {
            config.active_vault.unwrap_or_else(|| "No Vault".to_string())
        } else {
            "Unknown".to_string()
        };

        // Determine vault status
        let (status_text, status_color) = if let Some(ref status) = self.vault_status {
            if status.is_locked {
                ("🔒 LOCKED", Theme::RED_NEON)
            } else {
                ("🔓 UNLOCKED", Theme::GREEN_NEON)
            }
        } else {
            ("⏳ LOADING", Theme::YELLOW_NEON)
        };

        // Format balances with truncated mint addresses
        let standard_mint_str = self.standard_mint.to_string();
        let standard_mint_short = format!("{}...{}", &standard_mint_str[..4], &standard_mint_str[standard_mint_str.len()-4..]);

        let pq_mint_str = self.pq_mint.to_string();
        let pq_mint_short = format!("{}...{}", &pq_mint_str[..4], &pq_mint_str[pq_mint_str.len()-4..]);

        let pq_balance_text = if let Some(balance) = self.pq_balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            format!("{:.6} pqcoin ({})", balance_qdum, pq_mint_short)
        } else {
            format!("Loading... ({})", pq_mint_short)
        };

        let standard_balance_text = if let Some(balance) = self.standard_balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            format!("{:.6} qcoin ({})", balance_qdum, standard_mint_short)
        } else {
            format!("Loading... ({})", standard_mint_short)
        };

        // Build table rows with clean data organization
        let rows = vec![
            Row::new(vec![
                Line::from(Span::styled("VAULT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(vault_name, Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("STATUS", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("STANDARD QCOIN", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(standard_balance_text, Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("PQ QCOIN", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(pq_balance_text, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("ALGORITHM", Style::default().fg(Theme::CYAN_NEON))),
                Line::from(Span::styled("SPHINCS+-SHA2-128s", Style::default().fg(Theme::TEXT))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("SECURITY", Style::default().fg(Theme::CYAN_NEON))),
                Line::from(Span::styled("NIST FIPS 205", Style::default().fg(Theme::TEXT))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("NETWORK", Style::default().fg(Theme::CYAN_NEON))),
                Line::from(Span::styled("Solana Devnet", Style::default().fg(Theme::TEXT))),
            ]),
        ];

        let widths = [Constraint::Length(20), Constraint::Min(30)];

        let pulse = self.get_pulse_intensity();
        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(60, 180 + pulse / 3, 255)).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" VAULT STATUS ")
                    .title_style(Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::GLASS_1))
            .column_spacing(2);

        f.render_widget(table, area);
    }
    pub fn render_actions_panel(&self, f: &mut Frame, area: Rect) {
        // Define actions with clean data structure
        let actions = vec![
            ("🔐 REGISTER", "[G]", "Initialize PQ account", Theme::GREEN),
            ("🔒 LOCK", "[L]", "Secure vault", Theme::RED),
            ("🔓 UNLOCK", "[U]", "Verify signature", Theme::YELLOW),
            ("💸 TRANSFER", "[T]", "Send qcoin or pqcoin", Theme::CYAN),
            ("🔄 WRAP", "[W]", "qcoin -> pqcoin", Theme::GREEN_NEON),
            ("🔃 UNWRAP", "[Shift+W]", "pqcoin -> qcoin", Theme::CYAN_BRIGHT),
            ("🎁 AIRDROP", "[A]", "Claim 100 qcoin (24h cooldown)", Theme::CYAN_NEON),
            ("📦 POOL", "[P]", "View airdrop pool stats", Theme::YELLOW_NEON),
            ("❌ CLOSE", "[X]", "Close & reclaim rent", Theme::RED_NEON),
            ("📊 Network", "[M]", "Locked qcoin and Holder chart", Theme::CYAN_NEON),
            ("🗄️ VAULTS", "[V/N]", "Manage vaults", Theme::PURPLE),
        ];

        // Build table rows with selection highlighting
        let pulse = self.get_pulse_intensity();
        let rows: Vec<Row> = actions
            .iter()
            .enumerate()
            .map(|(idx, (action, key, desc, color))| {
                let is_selected = idx == self.selected_action;

                // Animated background for selected row
                let row_style = if is_selected {
                    Style::default().bg(Color::Rgb(28 + pulse / 4, 32 + pulse / 4, 48 + pulse / 4))
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

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(60, 180 + pulse / 3, 255)).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" QUICK ACTIONS ")
                    .title_style(Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::GLASS_1))
            .column_spacing(2)
            .header(
                Row::new(vec![
                    Line::from(Span::styled("ACTION", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("KEY", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("DESCRIPTION", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                ])
                .style(Style::default().bg(Theme::GLASS_2))
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

        // Controls with colorful badges
        let footer_text = vec![Line::from(vec![
            Span::styled(
                " Q/Esc ",
                Style::default()
                    .fg(Theme::TEXT)
                    .bg(Theme::RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Quit  ", Style::default().fg(Theme::SUBTEXT1)),
            Span::styled(
                " H/? ",
                Style::default()
                    .fg(Theme::TEXT)
                    .bg(Theme::PURPLE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Help  ", Style::default().fg(Theme::SUBTEXT1)),
            Span::styled(
                " R ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Refresh  ", Style::default().fg(Theme::SUBTEXT1)),
            Span::styled(
                " ↑↓/jk ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Navigate  ", Style::default().fg(Theme::SUBTEXT1)),
            Span::styled(
                " Enter ",
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Execute", Style::default().fg(Theme::SUBTEXT1)),
        ])];
        let pulse_footer = self.get_pulse_intensity();
        let footer = Paragraph::new(footer_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(150, 0, pulse_footer)))
                    .border_type(BorderType::Rounded)
                    .title(format!(" {} CONTROLS ", Icons::KEYBOARD))
                    .title_style(Style::default()
                        .fg(Theme::CYAN_NEON)
                        .add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::GLASS_1))
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

        let status_widget = Paragraph::new(status_msg)
            .style(Style::default().fg(status_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(status_color))
                    .border_type(BorderType::Rounded)
                    .title(" STATUS ")
                    .title_style(Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))
            )
            .style(Style::default().bg(Theme::GLASS_1));

        f.render_widget(status_widget, footer_chunks[1]);
    }
}
