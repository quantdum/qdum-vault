use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use solana_sdk::pubkey::Pubkey;
use std::io;

pub struct Dashboard {
    wallet: Pubkey,
    should_quit: bool,
}

impl Dashboard {
    pub fn new(wallet: Pubkey) -> Self {
        Self {
            wallet,
            should_quit: false,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the app
        let res = self.run_app(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{:?}", err)
        }

        Ok(())
    }

    fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if event::poll(std::time::Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                return Ok(());
            }
        }
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(7),  // Header
                    Constraint::Length(3),  // Wallet info
                    Constraint::Min(10),    // Main content
                    Constraint::Length(3),  // Footer
                ]
                .as_ref(),
            )
            .split(size);

        // Header with ASCII art
        let header = vec![
            Line::from(Span::styled(
                "╔════════════════════════════════════════════════════════════════════╗",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "║        Q D U M   V A U L T   -   I N T E R A C T I V E   T U I    ║",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "╠════════════════════════════════════════════════════════════════════╣",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("║  ", Style::default().fg(Color::Green)),
                Span::raw("🔐 "),
                Span::styled(
                    "Post-Quantum Security for Solana",
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("                              ║", Style::default().fg(Color::Green)),
            ]),
            Line::from(Span::styled(
                "╚════════════════════════════════════════════════════════════════════╝",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )),
        ];
        let header_paragraph = Paragraph::new(header).alignment(Alignment::Left);
        f.render_widget(header_paragraph, chunks[0]);

        // Wallet info
        let wallet_text = vec![Line::from(vec![
            Span::styled("Wallet: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                self.wallet.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];
        let wallet_paragraph = Paragraph::new(wallet_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title(" Account Info ")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(wallet_paragraph, chunks[1]);

        // Main content area - split into two columns
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(chunks[2]);

        // Left panel - Status
        self.render_status_panel(f, main_chunks[0]);

        // Right panel - Actions
        self.render_actions_panel(f, main_chunks[1]);

        // Footer with controls
        let footer_text = vec![Line::from(vec![
            Span::styled(
                " [Q/Esc] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit  "),
            Span::styled(
                " [R] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Refresh  "),
            Span::styled(
                " [L] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Lock  "),
            Span::styled(
                " [U] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Unlock  "),
        ])];
        let footer = Paragraph::new(footer_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title(" Controls ")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            )
            .alignment(Alignment::Center);
        f.render_widget(footer, chunks[3]);
    }

    fn render_status_panel(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items = vec![
            ListItem::new(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "🔒 LOCKED",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Algorithm: ", Style::default().fg(Color::Gray)),
                Span::styled("SPHINCS+-SHA2-128s", Style::default().fg(Color::Green)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Security: ", Style::default().fg(Color::Gray)),
                Span::styled("NIST FIPS 205", Style::default().fg(Color::Green)),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled("Network: ", Style::default().fg(Color::Gray)),
                Span::styled("Solana Devnet", Style::default().fg(Color::Cyan)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Color::Gray)),
                Span::styled("Loading...", Style::default().fg(Color::Yellow)),
            ])),
        ];

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Vault Status ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        );

        f.render_widget(list, area);
    }

    fn render_actions_panel(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items = vec![
            ListItem::new(Line::from(vec![
                Span::styled("→ ", Style::default().fg(Color::Green)),
                Span::styled("Register", Style::default().fg(Color::White)),
                Span::styled(" - Register PQ account", Style::default().fg(Color::Gray)),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled("→ ", Style::default().fg(Color::Red)),
                Span::styled("Lock", Style::default().fg(Color::White)),
                Span::styled(" - Lock your vault", Style::default().fg(Color::Gray)),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled("→ ", Style::default().fg(Color::Yellow)),
                Span::styled("Unlock", Style::default().fg(Color::White)),
                Span::styled(" - Unlock with quantum sig", Style::default().fg(Color::Gray)),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled("→ ", Style::default().fg(Color::Cyan)),
                Span::styled("Transfer", Style::default().fg(Color::White)),
                Span::styled(" - Send QDUM tokens", Style::default().fg(Color::Gray)),
            ])),
        ];

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Available Actions ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        );

        f.render_widget(list, area);
    }
}
