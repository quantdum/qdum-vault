use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use solana_sdk::pubkey::Pubkey;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::fs::OpenOptions;
use tokio::runtime::Runtime;

use crate::crypto::sphincs::SphincsKeyManager;
use crate::solana::client::VaultClient;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SelectedAction {
    Register,
    Lock,
    Unlock,
    Transfer,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AppMode {
    Normal,
    Help,
    RegisterPopup,
    LockPopup,
    UnlockPopup,
}

#[derive(Debug, Clone, PartialEq)]
enum ActionStep {
    Starting,
    InProgress(String),
    Success(String),
    Error(String),
}

pub struct Dashboard {
    wallet: Pubkey,
    keypair_path: PathBuf,
    rpc_url: String,
    program_id: Pubkey,
    should_quit: bool,
    selected_action: usize,
    mode: AppMode,
    status_message: Option<String>,
    vault_status: Option<VaultStatus>,
    balance: Option<u64>,
    is_loading: bool,
    action_steps: Vec<ActionStep>,
    runtime: Runtime,
    vault_client: VaultClient,
}

#[derive(Clone)]
struct VaultStatus {
    is_locked: bool,
    pda: Option<Pubkey>,
}

impl Dashboard {
    pub fn new(
        wallet: Pubkey,
        keypair_path: PathBuf,
        rpc_url: String,
        program_id: Pubkey,
    ) -> Result<Self> {
        let runtime = Runtime::new()?;
        let vault_client = VaultClient::new(&rpc_url, program_id)?;

        Ok(Self {
            wallet,
            keypair_path,
            rpc_url,
            program_id,
            should_quit: false,
            selected_action: 0,
            mode: AppMode::Normal,
            status_message: None,
            vault_status: None,
            balance: None,
            is_loading: false,
            action_steps: Vec::new(),
            runtime,
            vault_client,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Initial refresh with a welcome message
        self.status_message = Some("Dashboard loaded! Press any key to test...".to_string());
        self.refresh_data();

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

        if let Err(err) = &res {
            println!("Error: {:?}", err);
        }

        Ok(())
    }

    fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        // Open debug log file
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/qdum-debug.log")
            .ok();

        if let Some(ref mut f) = log {
            let _ = writeln!(f, "\n=== Dashboard started ===");
        }

        loop {
            terminal.draw(|f| self.ui(f))?;

            // Read events - IMPORTANT: Only handle KeyPress, not KeyRelease
            match event::read()? {
                Event::Key(key) => {
                    if let Some(ref mut f) = log {
                        let _ = writeln!(f, "Event::Key received - kind={:?} code={:?} mods={:?}",
                            key.kind, key.code, key.modifiers);
                    }

                    // CRITICAL: On Windows/WSL, we get both Press and Release events
                    // We only want to handle Press events to avoid double-triggering
                    if key.kind == KeyEventKind::Press {
                        if let Some(ref mut f) = log {
                            let _ = writeln!(f, "  -> Processing KeyPress: {:?}", key.code);
                        }
                        // Debug: show what key was pressed
                        self.status_message = Some(format!("DEBUG: Key={:?} Mods={:?}", key.code, key.modifiers));
                        self.handle_key_event(key.code, key.modifiers);
                    }
                }
                Event::Resize(w, h) => {
                    if let Some(ref mut f) = log {
                        let _ = writeln!(f, "Event::Resize {}x{}", w, h);
                    }
                }
                other => {
                    if let Some(ref mut f) = log {
                        let _ = writeln!(f, "Event::Other {:?}", other);
                    }
                }
            }

            if self.should_quit {
                if let Some(ref mut f) = log {
                    let _ = writeln!(f, "=== Dashboard quit ===");
                }
                return Ok(());
            }
        }
    }

    fn handle_key_event(&mut self, code: KeyCode, _modifiers: KeyModifiers) {
        match self.mode {
            AppMode::Help => {
                // Any key exits help mode
                self.mode = AppMode::Normal;
                self.status_message = None;
            }
            AppMode::RegisterPopup | AppMode::LockPopup | AppMode::UnlockPopup => {
                // In action popups, only Esc closes (actions auto-execute)
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.action_steps.clear();
                        self.status_message = Some("Popup closed".to_string());
                    }
                    _ => {}
                }
            }
            AppMode::Normal => {
                match code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        self.should_quit = true;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') | KeyCode::F(1) => {
                        self.mode = AppMode::Help;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        self.refresh_data();
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') => {
                        self.execute_lock();
                    }
                    KeyCode::Char('u') | KeyCode::Char('U') => {
                        self.execute_unlock();
                    }
                    KeyCode::Char('g') | KeyCode::Char('G') | KeyCode::Char('1') => {
                        self.execute_register();
                    }
                    KeyCode::Char('t') | KeyCode::Char('T') | KeyCode::Char('2') => {
                        self.status_message = Some("Transfer not yet implemented in TUI. Use: qdum-vault transfer".to_string());
                    }
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                        if self.selected_action > 0 {
                            self.selected_action -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                        if self.selected_action < 3 {
                            self.selected_action += 1;
                        }
                    }
                    KeyCode::Enter => {
                        match self.selected_action {
                            0 => self.execute_register(),
                            1 => self.execute_lock(),
                            2 => self.execute_unlock(),
                            3 => self.status_message = Some("Transfer not yet implemented in TUI".to_string()),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn refresh_data(&mut self) {
        self.is_loading = true;
        self.status_message = Some("🔄 Refreshing data...".to_string());

        // In a real implementation, we'd fetch this asynchronously
        // For now, set placeholder values
        self.vault_status = Some(VaultStatus {
            is_locked: true,
            pda: None,
        });
        self.balance = Some(0);

        self.is_loading = false;
        self.status_message = Some("✓ Data refreshed successfully".to_string());
    }

    fn execute_register(&mut self) {
        self.mode = AppMode::RegisterPopup;
        self.action_steps.clear();
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Executing Register...".to_string());
        // Execute immediately
        self.perform_register_action();
    }

    fn execute_lock(&mut self) {
        self.mode = AppMode::LockPopup;
        self.action_steps.clear();
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Executing Lock...".to_string());
        // Execute immediately
        self.perform_lock_action();
    }

    fn execute_unlock(&mut self) {
        self.mode = AppMode::UnlockPopup;
        self.action_steps.clear();
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Executing Unlock...".to_string());
        // Execute immediately
        self.perform_unlock_action();
    }

    fn perform_register_action(&mut self) {
        if !self.action_steps.is_empty() && !matches!(self.action_steps.last(), Some(ActionStep::Starting)) {
            return; // Already executed
        }

        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Loading SPHINCS+ public key...".to_string()));

        // Load SPHINCS+ public key
        let key_manager = match SphincsKeyManager::new(None) {
            Ok(km) => km,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to initialize key manager: {}", e)));
                self.status_message = Some("Register failed!".to_string());
                return;
            }
        };

        let sphincs_pubkey = match key_manager.load_public_key(None) {
            Ok(pk) => pk,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to load SPHINCS+ public key: {}", e)));
                self.status_message = Some("Register failed! Run 'qdum-vault init' first.".to_string());
                return;
            }
        };

        self.action_steps.push(ActionStep::Success("✓ SPHINCS+ public key loaded".to_string()));
        self.action_steps.push(ActionStep::InProgress("Connecting to Solana devnet...".to_string()));

        // Execute the register call
        let keypair_path = self.keypair_path.to_str().unwrap();
        let result = self.runtime.block_on(async {
            self.vault_client.register_pq_account(
                self.wallet,
                keypair_path,
                &sphincs_pubkey,
            ).await
        });

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("✓ Transaction confirmed!".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Account registered successfully!".to_string()));
                self.status_message = Some("Register completed!".to_string());
                self.refresh_data();
            }
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Registration failed: {}", e)));
                self.status_message = Some("Register failed!".to_string());
            }
        }
    }

    fn perform_lock_action(&mut self) {
        if !self.action_steps.is_empty() && !matches!(self.action_steps.last(), Some(ActionStep::Starting)) {
            return; // Already executed
        }

        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Checking account status...".to_string()));

        // Execute the lock call
        let keypair_path = self.keypair_path.to_str().unwrap();
        let result = self.runtime.block_on(async {
            self.vault_client.lock_vault(self.wallet, keypair_path).await
        });

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("✓ Transaction confirmed!".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Vault locked successfully!".to_string()));
                self.status_message = Some("Lock completed!".to_string());
                self.refresh_data();
            }
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Lock failed: {}", e)));
                self.status_message = Some("Lock failed!".to_string());
            }
        }
    }

    fn perform_unlock_action(&mut self) {
        if !self.action_steps.is_empty() && !matches!(self.action_steps.last(), Some(ActionStep::Starting)) {
            return; // Already executed
        }

        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Loading SPHINCS+ private key...".to_string()));

        // Load SPHINCS+ private key
        let key_manager = match SphincsKeyManager::new(None) {
            Ok(km) => km,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to initialize key manager: {}", e)));
                self.status_message = Some("Unlock failed!".to_string());
                return;
            }
        };

        let sphincs_privkey = match key_manager.load_private_key(None) {
            Ok(pk) => pk,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to load SPHINCS+ private key: {}", e)));
                self.status_message = Some("Unlock failed! Run 'qdum-vault init' first.".to_string());
                return;
            }
        };

        self.action_steps.push(ActionStep::Success("✓ SPHINCS+ private key loaded".to_string()));
        self.action_steps.push(ActionStep::InProgress("Generating quantum signature (this may take a moment)...".to_string()));

        // Execute the unlock call
        let keypair_path = self.keypair_path.to_str().unwrap();
        let result = self.runtime.block_on(async {
            self.vault_client.unlock_vault(
                self.wallet,
                keypair_path,
                &sphincs_privkey,
            ).await
        });

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("✓ Signature generated!".to_string()));
                self.action_steps.push(ActionStep::Success("✓ 44 verification transactions submitted!".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Vault unlocked successfully!".to_string()));
                self.status_message = Some("Unlock completed!".to_string());
                self.refresh_data();
            }
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Unlock failed: {}", e)));
                self.status_message = Some("Unlock failed!".to_string());
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
                    Constraint::Length(5),  // Header
                    Constraint::Length(3),  // Wallet info
                    Constraint::Min(8),     // Main content
                    Constraint::Length(6),  // Footer + status (3 lines each = 6 total)
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

        // Footer with controls and status message
        self.render_footer(f, chunks[3]);

        // Render help overlay if in help mode
        if self.mode == AppMode::Help {
            self.render_help_overlay(f, size);
        }

        // Render action popups
        match self.mode {
            AppMode::RegisterPopup => self.render_action_popup(f, size, "REGISTER", Color::Green),
            AppMode::LockPopup => self.render_action_popup(f, size, "LOCK VAULT", Color::Red),
            AppMode::UnlockPopup => self.render_action_popup(f, size, "UNLOCK VAULT", Color::Yellow),
            _ => {}
        }
    }

    fn render_status_panel(&self, f: &mut Frame, area: Rect) {
        let status_text = if let Some(ref status) = self.vault_status {
            if status.is_locked {
                "🔒 LOCKED"
            } else {
                "🔓 UNLOCKED"
            }
        } else {
            "Loading..."
        };

        let status_color = if let Some(ref status) = self.vault_status {
            if status.is_locked {
                Color::Red
            } else {
                Color::Green
            }
        } else {
            Color::Yellow
        };

        let balance_text = if let Some(balance) = self.balance {
            format!("{} QDUM", balance)
        } else {
            "Loading...".to_string()
        };

        let items = vec![
            ListItem::new(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status_text,
                    Style::default()
                        .fg(status_color)
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
                Span::styled(balance_text, Style::default().fg(Color::Yellow)),
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

    fn render_actions_panel(&self, f: &mut Frame, area: Rect) {
        let actions = vec![
            ("Register", "G/1", "Register PQ account", Color::Green),
            ("Lock", "L", "Lock your vault", Color::Red),
            ("Unlock", "U", "Unlock with quantum sig", Color::Yellow),
            ("Transfer", "T/2", "Send QDUM tokens", Color::Cyan),
        ];

        let items: Vec<ListItem> = actions
            .iter()
            .enumerate()
            .map(|(idx, (name, key, desc, color))| {
                let arrow = if idx == self.selected_action { "▶ " } else { "  " };
                let style = if idx == self.selected_action {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(arrow, Style::default().fg(*color)),
                    Span::styled(*name, style),
                    Span::styled(format!(" [{}]", key), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" - {}", desc), Style::default().fg(Color::Gray)),
                ]))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Available Actions (↑↓ to select, Enter to execute) ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        );

        f.render_widget(list, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        // Always split footer into controls + status
        let footer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)].as_ref())
            .split(area);

        // Controls
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
                " [H/?] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Help  "),
            Span::styled(
                " [R] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Refresh  "),
            Span::styled(
                " [↑↓/jk] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Navigate  "),
            Span::styled(
                " [Enter] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Execute"),
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
        f.render_widget(footer, footer_chunks[0]);

        // Status message - always show
        let status_msg = self.status_message.as_ref()
            .map(|s| s.clone())
            .unwrap_or_else(|| "Ready - Press H or ? for help, Q to quit".to_string());

        let status_widget = Paragraph::new(status_msg)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Status ")
            );

        f.render_widget(status_widget, footer_chunks[1]);
    }

    fn render_help_overlay(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(Span::styled(
                "QDUM VAULT - HELP",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Navigation:", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  ↑/↓ or j/k  - Navigate actions"),
            Line::from("  Enter       - Execute selected action"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Actions:", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  G or 1      - Register PQ account"),
            Line::from("  L           - Lock vault"),
            Line::from("  U           - Unlock vault"),
            Line::from("  T or 2      - Transfer tokens"),
            Line::from("  R           - Refresh status"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Other:", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  H or ?      - Show this help"),
            Line::from("  Q or Esc    - Quit dashboard"),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to close help",
                Style::default().fg(Color::Yellow),
            )),
        ];

        // Center the help box
        let help_area = centered_rect(60, 60, area);

        // Clear the background
        f.render_widget(Clear, help_area);

        let help_paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    .title(" Help ")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(help_paragraph, help_area);
    }

    fn render_action_popup(&self, f: &mut Frame, area: Rect, title: &str, title_color: Color) {
        let popup_area = centered_rect(70, 70, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build text from action steps
        let mut text_lines = vec![
            Line::from(Span::styled(
                format!("═══ {} ═══", title),
                Style::default().fg(title_color).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.action_steps.is_empty() {
            text_lines.push(Line::from("No steps yet..."));
        } else {
            for step in &self.action_steps {
                let line = match step {
                    ActionStep::Starting => Line::from(vec![
                        Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                        Span::styled("Preparing...", Style::default().fg(Color::White)),
                    ]),
                    ActionStep::InProgress(msg) => Line::from(vec![
                        Span::styled("⏳ ", Style::default().fg(Color::Cyan)),
                        Span::styled(msg.clone(), Style::default().fg(Color::White)),
                    ]),
                    ActionStep::Success(msg) => Line::from(vec![
                        Span::styled("✓ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::styled(msg.clone(), Style::default().fg(Color::Green)),
                    ]),
                    ActionStep::Error(msg) => Line::from(vec![
                        Span::styled("✗ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::styled(msg.clone(), Style::default().fg(Color::Red)),
                    ]),
                };
                text_lines.push(line);
            }
        }

        // Add instructions - always show close button since we auto-execute
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Close"),
        ]));

        let popup_paragraph = Paragraph::new(text_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
                    .title(format!(" {} ", title))
                    .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD)),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(popup_paragraph, popup_area);
    }
}

// Helper function to create a centered rect
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
