use anyhow::Result;
use colored::Colorize;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

use crate::vault_manager::{VaultConfig, VaultProfile};

pub struct VaultSwitcher {
    vaults: Vec<VaultProfile>,
    state: ListState,
    active_vault_name: Option<String>,
    show_help: bool,
}

impl VaultSwitcher {
    pub fn new(config: &VaultConfig) -> Self {
        let vaults: Vec<VaultProfile> = config.list_vaults().into_iter().cloned().collect();
        let mut state = ListState::default();

        // Select active vault by default
        if let Some(active) = &config.active_vault {
            for (i, vault) in vaults.iter().enumerate() {
                if &vault.name == active {
                    state.select(Some(i));
                    break;
                }
            }
        }

        if state.selected().is_none() && !vaults.is_empty() {
            state.select(Some(0));
        }

        Self {
            vaults,
            state,
            active_vault_name: config.active_vault.clone(),
            show_help: false,
        }
    }

    /// Run the interactive vault switcher
    pub fn run(&mut self) -> Result<Option<String>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<Option<String>> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if let Event::Key(key) = event::read()? {
                match self.handle_key(key) {
                    KeyAction::Select => {
                        if let Some(i) = self.state.selected() {
                            if i < self.vaults.len() {
                                return Ok(Some(self.vaults[i].name.clone()));
                            } else {
                                // "Create New Vault" option selected
                                return Ok(Some("__CREATE_NEW__".to_string()));
                            }
                        }
                    }
                    KeyAction::Quit => {
                        return Ok(None);
                    }
                    KeyAction::Delete => {
                        if let Some(i) = self.state.selected() {
                            if i < self.vaults.len() {
                                return Ok(Some(format!("__DELETE__{}", self.vaults[i].name)));
                            }
                        }
                    }
                    KeyAction::Continue => {}
                }
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let size = f.area();

        // Create centered layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(5),     // Vault list (reduced to make room)
                Constraint::Length(5),  // Help text (increased height)
            ].as_ref())
            .split(size);

        // Title
        let title = Paragraph::new("Select Vault")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        f.render_widget(title, chunks[0]);

        // Vault list items
        let items: Vec<ListItem> = self.vaults.iter().enumerate().map(|(i, vault)| {
            let is_active = self.active_vault_name.as_ref() == Some(&vault.name);
            let is_selected = self.state.selected() == Some(i);

            let indicator = if is_active { "●" } else { "○" };
            let status = if is_active { " [ACTIVE]" } else { "" };

            let mut spans = vec![
                Span::styled(
                    format!(" {} ", indicator),
                    if is_active {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    }
                ),
                Span::styled(
                    vault.display_name(),
                    if is_selected {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    }
                ),
            ];

            if is_active {
                spans.push(Span::styled(
                    status,
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                ));
            }

            let wallet_info = if !vault.wallet_address.is_empty() {
                format!("    └─ {}", vault.short_wallet())
            } else {
                "    └─ (not initialized)".to_string()
            };

            let content = vec![
                Line::from(spans),
                Line::from(Span::styled(
                    wallet_info,
                    Style::default().fg(Color::DarkGray)
                )),
            ];

            ListItem::new(content)
        }).collect();

        // Add "Create New Vault" option
        let mut all_items = items;
        all_items.push(ListItem::new(vec![
            Line::from(Span::styled(
                "",
                Style::default()
            )),
            Line::from(vec![
                Span::styled(" + ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("Create New Vault", Style::default().fg(Color::Green)),
            ]),
        ]));

        let list = List::new(all_items)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::White)))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, chunks[1], &mut self.state);

        // Help text - multi-line for better visibility
        let help_text = vec![
            Line::from(vec![
                Span::styled("[↑↓] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Navigate  "),
                Span::styled("[Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("Select"),
            ]),
            Line::from(vec![
                Span::styled("[d] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("Delete  "),
                Span::styled("[q/Esc] ", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
                Span::raw("Quit"),
            ]),
        ];

        let help = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Controls "));
        f.render_widget(help, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => KeyAction::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Char('d') | KeyCode::Delete => KeyAction::Delete,
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
                KeyAction::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
                KeyAction::Continue
            }
            KeyCode::Home => {
                self.state.select(Some(0));
                KeyAction::Continue
            }
            KeyCode::End => {
                self.state.select(Some(self.vaults.len())); // +1 for "Create New" option
                KeyAction::Continue
            }
            _ => KeyAction::Continue,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.vaults.len() {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.vaults.len() // Wrap to "Create New" option
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

enum KeyAction {
    Select,
    Quit,
    Delete,
    Continue,
}

/// Simple prompt for vault creation
pub fn prompt_vault_name() -> Result<Option<String>> {
    println!("\n{}", "Create New Vault".bright_cyan().bold());
    println!("{}", "─".repeat(50).bright_cyan());

    print!("Vault name: ");
    io::Write::flush(&mut io::stdout())?;

    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();

    if name.is_empty() {
        return Ok(None);
    }

    Ok(Some(name))
}

/// Prompt for vault description
pub fn prompt_vault_description() -> Result<Option<String>> {
    print!("Description (optional): ");
    io::Write::flush(&mut io::stdout())?;

    let mut desc = String::new();
    io::stdin().read_line(&mut desc)?;
    let desc = desc.trim().to_string();

    if desc.is_empty() {
        Ok(None)
    } else {
        Ok(Some(desc))
    }
}

/// Prompt for yes/no confirmation
pub fn prompt_confirm(message: &str) -> Result<bool> {
    print!("{} [y/N]: ", message);
    io::Write::flush(&mut io::stdout())?;

    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    let response = response.trim().to_lowercase();

    Ok(response == "y" || response == "yes")
}

/// Prompt to type vault name for deletion confirmation (safer)
pub fn prompt_delete_confirmation(vault_name: &str) -> Result<bool> {
    println!("\n{}", "⚠️  WARNING: This action cannot be undone!".bright_red().bold());
    println!("{}", "─".repeat(50).red());
    println!();
    println!("This will delete the vault configuration for: {}", vault_name.bright_yellow().bold());
    println!("{}", "Note: This only removes the vault from the config.".dimmed());
    println!("{}", "      Your keys and wallet files will remain on disk.".dimmed());
    println!();
    print!("Type the vault name '{}' to confirm deletion: ", vault_name.bright_yellow());
    io::Write::flush(&mut io::stdout())?;

    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    let response = response.trim();

    Ok(response == vault_name)
}
