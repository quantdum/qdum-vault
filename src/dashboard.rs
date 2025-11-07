use anyhow::Result;
use arboard::Clipboard;
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
    widgets::{Block, Borders, BorderType, Clear, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use solana_sdk::pubkey::Pubkey;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::fs::{self, OpenOptions};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::crypto::sphincs::SphincsKeyManager;
use crate::solana::client::VaultClient;
use crate::icons::Icons;
use crate::theme::Theme;
use crate::vault_manager::VaultConfig;

/// Helper function to suppress stdout/stderr during operation
/// This prevents CLI output from glitching behind the TUI
fn suppress_output<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    let original_stdout = unsafe { libc::dup(1) };
    let original_stderr = unsafe { libc::dup(2) };

    // Redirect to /dev/null
    let devnull = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .unwrap();
    let devnull_fd = devnull.as_raw_fd();

    unsafe {
        libc::dup2(devnull_fd, 1);
        libc::dup2(devnull_fd, 2);
    }

    // Run the function
    let result = f();

    // Restore stdout/stderr
    unsafe {
        libc::dup2(original_stdout, 1);
        libc::dup2(original_stderr, 2);
        libc::close(original_stdout);
        libc::close(original_stderr);
    }

    result
}

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
    TransferPopup,
    AirdropClaimPopup,
    AirdropStatsPopup,
    VaultSwitchPopup,
    DeleteConfirmPopup,
    CloseConfirmPopup,
    ChartPopup,
    ResultPopup,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TransferInputField {
    Recipient,
    Amount,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum VaultManagementMode {
    List,      // Showing list of vaults
    Create,    // Creating new vault
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChartType {
    LockedAmount,
    HolderCount,
}

impl ChartType {
    fn to_string(&self) -> &str {
        match self {
            ChartType::LockedAmount => "LOCKED QDUM",
            ChartType::HolderCount => "LOCKED HOLDERS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChartTimeframe {
    FiveMinutes,
    OneDay,
    FiveDays,
    OneWeek,
    OneMonth,
    All,
}

impl ChartTimeframe {
    fn to_string(&self) -> &str {
        match self {
            ChartTimeframe::FiveMinutes => "5M",
            ChartTimeframe::OneDay => "1D",
            ChartTimeframe::FiveDays => "5D",
            ChartTimeframe::OneWeek => "1W",
            ChartTimeframe::OneMonth => "1M",
            ChartTimeframe::All => "ALL",
        }
    }

    fn to_duration(&self) -> Option<chrono::Duration> {
        use chrono::Duration as ChronoDuration;
        match self {
            ChartTimeframe::FiveMinutes => Some(ChronoDuration::minutes(5)),
            ChartTimeframe::OneDay => Some(ChronoDuration::days(1)),
            ChartTimeframe::FiveDays => Some(ChronoDuration::days(5)),
            ChartTimeframe::OneWeek => Some(ChronoDuration::days(7)),
            ChartTimeframe::OneMonth => Some(ChronoDuration::days(30)),
            ChartTimeframe::All => None,
        }
    }
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
    sphincs_public_key_path: String,
    sphincs_private_key_path: String,
    rpc_url: String,
    program_id: Pubkey,
    mint: Pubkey,
    should_quit: bool,
    selected_action: usize,
    mode: AppMode,
    status_message: Option<String>,
    vault_status: Option<VaultStatus>,
    balance: Option<u64>,
    is_loading: bool,
    action_steps: Vec<ActionStep>,
    vault_client: VaultClient,
    needs_clear: bool,
    pending_action: bool,  // Flag to execute action on next loop iteration
    pending_transfer: bool,  // Flag specifically for transfer action
    progress_current: usize,
    progress_total: usize,
    progress_message: String,
    progress_state_shared: Option<Arc<Mutex<(usize, usize, String)>>>,  // Shared progress for background thread
    unlock_complete: Option<Arc<AtomicBool>>,  // Flag to detect when unlock finishes
    unlock_success_message: Option<String>,  // Success message to display
    // Transfer state
    transfer_recipient: String,
    transfer_amount: String,
    transfer_focused_field: TransferInputField,
    // New vault state
    new_vault_name: String,
    // Vault management state
    vault_management_mode: VaultManagementMode,
    vault_list: Vec<crate::vault_manager::VaultProfile>,
    selected_vault_index: usize,
    // Delete confirmation state
    vault_to_delete: String,
    delete_confirmation_input: String,
    // Close confirmation state
    vault_to_close: String,
    close_confirmation_input: String,
    // Animation state
    animation_frame: u8,  // Counter for animation frames
    last_animation_update: std::time::Instant,
    // Chart state
    chart_type: ChartType,
    chart_timeframe: ChartTimeframe,
    airdrop_timeframe: ChartTimeframe,
    // Cached airdrop stats
    airdrop_distributed: u64,
    airdrop_remaining: u64,
}

#[derive(Clone)]
struct VaultStatus {
    is_locked: bool,
    pda: Option<Pubkey>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LockHistoryEntry {
    timestamp: String,      // ISO 8601 format
    locked_amount: f64,     // Total amount of QDUM locked network-wide
    holder_count: usize,    // Number of addresses with locked tokens
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AirdropHistoryEntry {
    timestamp: String,       // ISO 8601 format
    distributed: f64,        // Total QDUM claimed from airdrop pool
    remaining: f64,          // Remaining QDUM in pool (out of 3% cap)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AirdropHistory {
    entries: Vec<AirdropHistoryEntry>,
}

impl AirdropHistory {
    fn load() -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let history_path = home.join(".qdum").join("airdrop_history.json");

        if history_path.exists() {
            let contents = std::fs::read_to_string(&history_path)?;
            let history: AirdropHistory = serde_json::from_str(&contents)?;
            Ok(history)
        } else {
            Ok(AirdropHistory { entries: Vec::new() })
        }
    }

    fn save(&self) -> Result<()> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let vault_dir = home.join(".qdum");
        std::fs::create_dir_all(&vault_dir)?;

        let history_path = vault_dir.join("airdrop_history.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&history_path, contents)?;
        Ok(())
    }

    fn add_entry(&mut self, distributed: f64, remaining: f64) {
        use chrono::Utc;
        let timestamp = Utc::now().to_rfc3339();
        self.entries.push(AirdropHistoryEntry {
            timestamp,
            distributed,
            remaining,
        });
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LockHistory {
    entries: Vec<LockHistoryEntry>,
}

impl LockHistory {
    fn load() -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let history_path = home.join(".qdum").join("network_lock_history.json");

        if history_path.exists() {
            let contents = std::fs::read_to_string(&history_path)?;
            let history: LockHistory = serde_json::from_str(&contents)?;
            Ok(history)
        } else {
            Ok(LockHistory { entries: Vec::new() })
        }
    }

    fn save(&self) -> Result<()> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let vault_dir = home.join(".qdum");
        std::fs::create_dir_all(&vault_dir)?;

        let history_path = vault_dir.join("network_lock_history.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&history_path, contents)?;
        Ok(())
    }

    fn add_entry(&mut self, locked_amount: f64, holder_count: usize) {
        use chrono::Utc;
        let entry = LockHistoryEntry {
            timestamp: Utc::now().to_rfc3339(),
            locked_amount,
            holder_count,
        };
        self.entries.push(entry);

        // Keep only last 30 days of entries (hourly snapshots)
        if self.entries.len() > 720 {  // 30 days * 24 hours = 720 entries
            self.entries.remove(0);
        }
    }
}

impl Dashboard {
    pub fn new(
        wallet: Pubkey,
        keypair_path: PathBuf,
        sphincs_public_key_path: String,
        sphincs_private_key_path: String,
        rpc_url: String,
        program_id: Pubkey,
        mint: Pubkey,
    ) -> Result<Self> {
        let vault_client = VaultClient::new(&rpc_url, program_id)?;

        Ok(Self {
            wallet,
            keypair_path,
            sphincs_public_key_path,
            sphincs_private_key_path,
            rpc_url,
            program_id,
            mint,
            should_quit: false,
            selected_action: 0,
            mode: AppMode::Normal,
            status_message: None,
            vault_status: None,
            balance: None,
            is_loading: false,
            action_steps: Vec::new(),
            vault_client,
            needs_clear: false,
            pending_action: false,
            pending_transfer: false,
            progress_current: 0,
            progress_total: 0,
            progress_message: String::new(),
            progress_state_shared: None,
            unlock_complete: None,
            unlock_success_message: None,
            transfer_recipient: String::new(),
            transfer_amount: String::new(),
            transfer_focused_field: TransferInputField::Recipient,
            new_vault_name: String::new(),
            vault_management_mode: VaultManagementMode::List,
            vault_list: Vec::new(),
            selected_vault_index: 0,
            vault_to_delete: String::new(),
            delete_confirmation_input: String::new(),
            vault_to_close: String::new(),
            close_confirmation_input: String::new(),
            animation_frame: 0,
            last_animation_update: std::time::Instant::now(),
            chart_type: ChartType::LockedAmount,
            chart_timeframe: ChartTimeframe::All,
            airdrop_timeframe: ChartTimeframe::All,
            airdrop_distributed: 0,
            airdrop_remaining: 0,
        })
    }

    // Get animated scanning dots
    fn get_animated_dots(&self) -> &'static str {
        match self.animation_frame % 4 {
            0 => "   ",
            1 => ".  ",
            2 => ".. ",
            3 => "...",
            _ => "   ",
        }
    }

    // Get pulsing intensity for status (0-255)
    fn get_pulse_intensity(&self) -> u8 {
        let phase = (self.animation_frame % 20) as f32 / 20.0;
        let pulse = ((phase * std::f32::consts::PI * 2.0).sin() + 1.0) / 2.0;
        (pulse * 155.0 + 100.0) as u8  // Range: 100-255 (much wider range)
    }

    // Get alternate colors for more dramatic effects
    fn get_pulse_color_bright(&self) -> bool {
        (self.animation_frame / 10) % 2 == 0
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
            // Update animation frame periodically (every 150ms)
            if self.last_animation_update.elapsed().as_millis() > 150 {
                self.animation_frame = self.animation_frame.wrapping_add(1);
                self.last_animation_update = std::time::Instant::now();
            }

            // Poll shared progress state from background unlock thread
            if let Some(ref progress_state) = self.progress_state_shared {
                if let Ok(state) = progress_state.lock() {
                    let new_current = state.0;
                    let new_total = state.1;
                    let new_message = state.2.clone();

                    // ALWAYS update - don't check for changes
                    self.progress_current = new_current;
                    self.progress_total = new_total;
                    self.progress_message = new_message.clone();

                    // Update action steps
                    if !new_message.is_empty() {
                        if self.action_steps.is_empty() {
                            self.action_steps.push(ActionStep::InProgress(new_message));
                        } else {
                            self.action_steps[0] = ActionStep::InProgress(new_message);
                        }
                    }
                }
            }

            // Check if unlock is complete
            if let Some(ref unlock_flag) = self.unlock_complete {
                if unlock_flag.load(Ordering::SeqCst) {
                    // Unlock finished - check if it was successful
                    let is_success = self.progress_current == self.progress_total && self.progress_total > 0;

                    if is_success {
                        // Success! Close popup, refresh vault status, and show success message
                        self.unlock_success_message = Some("✓ Vault unlocked successfully!".to_string());
                        self.mode = AppMode::Normal;
                        self.needs_clear = true;

                        // Refresh vault status (use block_in_place to avoid nested runtime)
                        let vault_client = &self.vault_client;
                        let wallet = self.wallet;
                        let mint = self.mint;

                        // Use block_in_place + Handle::current() to safely call async from sync context
                        let status_result = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                vault_client.get_vault_status(wallet).await
                            })
                        });

                        if let Ok((is_locked, pda)) = status_result {
                            self.vault_status = Some(VaultStatus {
                                is_locked,
                                pda: Some(pda),
                            });
                        }

                        // Refresh balance
                        let balance_result = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                vault_client.get_balance(wallet, mint).await
                            })
                        });
                        if let Ok(bal) = balance_result {
                            self.balance = Some(bal);
                        }
                    } else {
                        // Failed - show error message
                        self.unlock_success_message = Some(format!("✗ {}", self.progress_message));
                        self.mode = AppMode::Normal;
                        self.needs_clear = true;
                    }

                    // Clear unlock tracking
                    self.unlock_complete = None;
                    self.progress_state_shared = None;
                }
            }

            // Clear terminal if needed (after closing popups)
            if self.needs_clear {
                terminal.clear()?;
                self.needs_clear = false;
            }

            terminal.draw(|f| self.ui(f))?;
            // FORCE flush to ensure screen updates
            std::io::Write::flush(&mut std::io::stdout())?;

            // Execute pending action after popup is drawn
            if self.pending_action {
                self.pending_action = false;
                if self.mode == AppMode::UnlockPopup {
                    self.perform_unlock_action();
                }
            }

            // Execute pending transfer after UI is drawn
            if self.pending_transfer {
                self.pending_transfer = false;
                self.perform_transfer_action();
            }

            // Read events with timeout to enable animations (redraw every 150ms even without input)
            if !event::poll(std::time::Duration::from_millis(150))? {
                // No event, but timeout reached - continue loop to redraw with updated animation
                continue;
            }

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
            AppMode::ChartPopup => {
                // TAB or arrows switch chart type, Esc closes, R refreshes, m/1/5/7/3/a changes timeframe
                match code {
                    KeyCode::Tab | KeyCode::Right => {
                        // Switch to next chart type
                        self.chart_type = match self.chart_type {
                            ChartType::LockedAmount => ChartType::HolderCount,
                            ChartType::HolderCount => ChartType::LockedAmount,
                        };
                        self.status_message = Some(format!("📊 Showing {}", self.chart_type.to_string()));
                    }
                    KeyCode::Left => {
                        // Switch to previous chart type (same as TAB for 2 types)
                        self.chart_type = match self.chart_type {
                            ChartType::LockedAmount => ChartType::HolderCount,
                            ChartType::HolderCount => ChartType::LockedAmount,
                        };
                        self.status_message = Some(format!("📊 Showing {}", self.chart_type.to_string()));
                    }
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.status_message = None;
                        self.needs_clear = true;
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        self.chart_timeframe = ChartTimeframe::FiveMinutes;
                        self.status_message = Some("📊 Showing 5 minutes".to_string());
                    }
                    KeyCode::Char('1') => {
                        self.chart_timeframe = ChartTimeframe::OneDay;
                        self.status_message = Some("📊 Showing 1 day".to_string());
                    }
                    KeyCode::Char('5') => {
                        self.chart_timeframe = ChartTimeframe::FiveDays;
                        self.status_message = Some("📊 Showing 5 days".to_string());
                    }
                    KeyCode::Char('7') => {
                        self.chart_timeframe = ChartTimeframe::OneWeek;
                        self.status_message = Some("📊 Showing 1 week".to_string());
                    }
                    KeyCode::Char('3') => {
                        self.chart_timeframe = ChartTimeframe::OneMonth;
                        self.status_message = Some("📊 Showing 1 month".to_string());
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        self.chart_timeframe = ChartTimeframe::All;
                        self.status_message = Some("📊 Showing all data".to_string());
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Refresh network data (force bypass cache)
                        let _ = self.record_lock_history(true);
                        // Status message is set by record_lock_history
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') => {
                        // Show network query log
                        self.action_steps.clear();
                        self.action_steps.push(ActionStep::InProgress("📋 Network Query Log:".to_string()));
                        self.action_steps.push(ActionStep::InProgress("".to_string()));

                        if let Ok(log_content) = std::fs::read_to_string("/tmp/qdum-network-query.log") {
                            for line in log_content.lines().take(30) {
                                self.action_steps.push(ActionStep::InProgress(line.to_string()));
                            }
                        } else {
                            self.action_steps.push(ActionStep::Error("Failed to read log file".to_string()));
                        }

                        self.action_steps.push(ActionStep::InProgress("".to_string()));
                        self.action_steps.push(ActionStep::InProgress("Press [Esc] to close".to_string()));
                        self.mode = AppMode::ResultPopup;
                        self.needs_clear = true;
                    }
                    _ => {}
                }
            }
            AppMode::RegisterPopup | AppMode::LockPopup | AppMode::UnlockPopup | AppMode::ResultPopup => {
                // In action popups, only Esc closes (actions auto-execute)
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.action_steps.clear();
                        self.status_message = Some("Popup closed".to_string());
                        self.needs_clear = true;  // Force terminal clear on next loop
                    }
                    _ => {}
                }
            }
            AppMode::TransferPopup => {
                // Handle transfer popup input
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.transfer_recipient.clear();
                        self.transfer_amount.clear();
                        self.transfer_focused_field = TransferInputField::Recipient;
                        self.status_message = Some("Transfer cancelled".to_string());
                        self.needs_clear = true;
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        // Switch between fields
                        self.transfer_focused_field = match self.transfer_focused_field {
                            TransferInputField::Recipient => TransferInputField::Amount,
                            TransferInputField::Amount => TransferInputField::Recipient,
                        };
                    }
                    KeyCode::Up => {
                        // Switch between fields (reverse)
                        self.transfer_focused_field = match self.transfer_focused_field {
                            TransferInputField::Recipient => TransferInputField::Amount,
                            TransferInputField::Amount => TransferInputField::Recipient,
                        };
                    }
                    KeyCode::Char(c) => {
                        match self.transfer_focused_field {
                            TransferInputField::Recipient => {
                                self.transfer_recipient.push(c);
                            }
                            TransferInputField::Amount => {
                                // Only allow numbers and decimal point
                                if c.is_ascii_digit() || c == '.' {
                                    self.transfer_amount.push(c);
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        match self.transfer_focused_field {
                            TransferInputField::Recipient => {
                                self.transfer_recipient.pop();
                            }
                            TransferInputField::Amount => {
                                self.transfer_amount.pop();
                            }
                        }
                    }
                    KeyCode::Enter => {
                        // Validate and prepare transfer (don't execute yet)
                        if self.validate_transfer_inputs() {
                            self.pending_transfer = true;
                            self.mode = AppMode::Normal;
                        }
                    }
                    _ => {}
                }
            }
            AppMode::DeleteConfirmPopup => {
                // Handle delete confirmation input
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::VaultSwitchPopup;
                        self.vault_to_delete.clear();
                        self.delete_confirmation_input.clear();
                        self.status_message = Some("Delete cancelled".to_string());
                        self.needs_clear = true;
                    }
                    KeyCode::Char(c) => {
                        self.delete_confirmation_input.push(c);
                    }
                    KeyCode::Backspace => {
                        self.delete_confirmation_input.pop();
                    }
                    KeyCode::Enter => {
                        // Check if typed name matches
                        if self.delete_confirmation_input == self.vault_to_delete {
                            let vault_name = self.vault_to_delete.clone();
                            self.perform_vault_delete(&vault_name);
                        } else {
                            self.mode = AppMode::VaultSwitchPopup;
                            self.status_message = Some("❌ Vault name did not match - delete cancelled".to_string());
                            self.delete_confirmation_input.clear();
                        }
                    }
                    _ => {}
                }
            }
            AppMode::CloseConfirmPopup => {
                // Handle close confirmation input
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.vault_to_close.clear();
                        self.close_confirmation_input.clear();
                        self.status_message = Some("Close cancelled".to_string());
                        self.needs_clear = true;
                    }
                    KeyCode::Char(c) => {
                        self.close_confirmation_input.push(c);
                    }
                    KeyCode::Backspace => {
                        self.close_confirmation_input.pop();
                    }
                    KeyCode::Enter => {
                        // Check if typed name matches
                        if self.close_confirmation_input == self.vault_to_close {
                            self.perform_close();
                        } else {
                            self.mode = AppMode::Normal;
                            self.status_message = Some("❌ Vault name did not match - close cancelled".to_string());
                            self.close_confirmation_input.clear();
                            self.needs_clear = true;
                        }
                    }
                    _ => {}
                }
            }
            AppMode::VaultSwitchPopup => {
                let _ = std::fs::write("/tmp/vault-mode-check.log",
                    format!("In VaultSwitchPopup mode, management_mode={:?}, keycode={:?}\n",
                        self.vault_management_mode, code));

                match self.vault_management_mode {
                    VaultManagementMode::List => {
                        // Handle vault list navigation and selection
                        match code {
                            KeyCode::Esc => {
                                self.mode = AppMode::Normal;
                                self.vault_list.clear();
                                self.status_message = Some("Cancelled".to_string());
                                self.needs_clear = true;
                            }
                            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                                if self.selected_vault_index > 0 {
                                    self.selected_vault_index -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                                // +1 for "Create New" option
                                let max_index = self.vault_list.len();
                                if self.selected_vault_index < max_index {
                                    self.selected_vault_index += 1;
                                }
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') => {
                                // Switch to create mode
                                self.vault_management_mode = VaultManagementMode::Create;
                                self.new_vault_name.clear();
                                self.status_message = Some("Enter vault name...".to_string());
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                                // Show delete confirmation popup (if not "Create New" option)
                                if self.selected_vault_index < self.vault_list.len() {
                                    let selected_vault = &self.vault_list[self.selected_vault_index];
                                    self.vault_to_delete = selected_vault.name.clone();
                                    self.delete_confirmation_input.clear();
                                    self.mode = AppMode::DeleteConfirmPopup;
                                    self.status_message = Some(format!("Type '{}' to confirm deletion", selected_vault.name));
                                }
                            }
                            KeyCode::Enter => {
                                use std::io::Write;
                                let _ = std::fs::write("/tmp/vault-enter-pressed.log",
                                    format!("Enter pressed! selected_index={}, vault_list_len={}\n",
                                        self.selected_vault_index, self.vault_list.len()));

                                // If "Create New" is selected (last item)
                                if self.selected_vault_index == self.vault_list.len() {
                                    let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-enter-pressed.log")
                                        .and_then(|mut f| writeln!(f, "Create New selected"));
                                    self.vault_management_mode = VaultManagementMode::Create;
                                    self.new_vault_name.clear();
                                    self.status_message = Some("Enter vault name...".to_string());
                                } else if self.selected_vault_index < self.vault_list.len() {
                                    let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-enter-pressed.log")
                                        .and_then(|mut f| writeln!(f, "Vault switch selected, index={}", self.selected_vault_index));
                                    // Switch to selected vault
                                    let selected_vault = &self.vault_list[self.selected_vault_index];
                                    let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-enter-pressed.log")
                                        .and_then(|mut f| writeln!(f, "About to switch to vault: {}", selected_vault.name));
                                    self.perform_vault_switch(&selected_vault.name.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    VaultManagementMode::Create => {
                        // Handle vault creation input
                        match code {
                            KeyCode::Esc => {
                                // Go back to list mode
                                self.vault_management_mode = VaultManagementMode::List;
                                self.new_vault_name.clear();
                                self.status_message = Some("Select vault or create new...".to_string());
                            }
                            KeyCode::Char(c) => {
                                // Allow alphanumeric, dash, underscore
                                if c.is_alphanumeric() || c == '-' || c == '_' {
                                    self.new_vault_name.push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                self.new_vault_name.pop();
                            }
                            KeyCode::Enter => {
                                // Validate and create vault
                                if self.new_vault_name.is_empty() {
                                    self.status_message = Some("❌ Vault name cannot be empty".to_string());
                                } else {
                                    // Perform vault creation
                                    self.perform_new_vault_action();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            AppMode::Normal => {
                // Clear unlock success message on any keypress
                self.unlock_success_message = None;

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
                        self.execute_transfer();
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        self.execute_claim_airdrop();
                    }
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        // Fetch airdrop stats before showing popup
                        if let Ok((distributed, remaining)) = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                self.vault_client.get_airdrop_stats().await
                            })
                        }) {
                            self.airdrop_distributed = distributed;
                            self.airdrop_remaining = remaining;

                            // Save to history
                            let distributed_qdum = distributed as f64 / 1_000_000.0;
                            let remaining_qdum = remaining as f64 / 1_000_000.0;
                            if let Ok(mut history) = AirdropHistory::load() {
                                history.add_entry(distributed_qdum, remaining_qdum);
                                let _ = history.save();
                            }

                            self.mode = AppMode::AirdropStatsPopup;
                            self.needs_clear = true;
                            self.status_message = Some("Viewing airdrop pool stats...".to_string());
                        } else {
                            self.status_message = Some("Failed to fetch airdrop stats".to_string());
                        }
                    }
                    KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Char('3') => {
                        self.execute_close();
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        self.execute_chart();
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.copy_wallet_to_clipboard();
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') | KeyCode::Char('n') | KeyCode::Char('N') => {
                        self.execute_new_vault();
                    }
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                        if self.selected_action > 0 {
                            self.selected_action -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                        if self.selected_action < 4 {
                            self.selected_action += 1;
                        }
                    }
                    KeyCode::Enter => {
                        match self.selected_action {
                            0 => self.execute_register(),
                            1 => self.execute_lock(),
                            2 => self.execute_unlock(),
                            3 => self.execute_transfer(),
                            4 => self.execute_new_vault(),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            AppMode::AirdropClaimPopup => {
                // Esc closes popup, A shows stats
                match code {
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        self.mode = AppMode::AirdropStatsPopup;
                        self.needs_clear = true;
                    }
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.needs_clear = true;
                    }
                    _ => {}
                }
            }
            AppMode::AirdropStatsPopup => {
                // Esc closes, m/1/5/7/3/a changes timeframe
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.needs_clear = true;
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        self.airdrop_timeframe = ChartTimeframe::FiveMinutes;
                        self.status_message = Some("📊 Showing 5 minutes".to_string());
                    }
                    KeyCode::Char('1') => {
                        self.airdrop_timeframe = ChartTimeframe::OneDay;
                        self.status_message = Some("📊 Showing 1 day".to_string());
                    }
                    KeyCode::Char('5') => {
                        self.airdrop_timeframe = ChartTimeframe::FiveDays;
                        self.status_message = Some("📊 Showing 5 days".to_string());
                    }
                    KeyCode::Char('7') => {
                        self.airdrop_timeframe = ChartTimeframe::OneWeek;
                        self.status_message = Some("📊 Showing 1 week".to_string());
                    }
                    KeyCode::Char('3') => {
                        self.airdrop_timeframe = ChartTimeframe::OneMonth;
                        self.status_message = Some("📊 Showing 1 month".to_string());
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        self.airdrop_timeframe = ChartTimeframe::All;
                        self.status_message = Some("📊 Showing all data".to_string());
                    }
                    _ => {}
                }
            }
            AppMode::LockPopup | AppMode::RegisterPopup => {
                // Esc closes popup
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.needs_clear = true;
                    }
                    _ => {}
                }
            }
        }
    }

    fn refresh_data(&mut self) {
        self.is_loading = true;
        self.status_message = Some("🔄 Refreshing data...".to_string());

        // Fetch actual vault status and balance from blockchain
        let wallet = self.wallet;
        let mint = self.mint;
        let vault_client = &self.vault_client;

        let status_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.get_vault_status(wallet).await
            })
        });

        let balance_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.get_balance(wallet, mint).await
            })
        });

        match status_result {
            Ok((is_locked, pda)) => {
                self.vault_status = Some(VaultStatus {
                    is_locked,
                    pda: Some(pda),
                });
                // Fetch actual balance
                self.balance = balance_result.ok();
                self.is_loading = false;
                self.status_message = Some("✓ Data refreshed successfully".to_string());
            }
            Err(e) => {
                // Account might not exist yet (not registered)
                self.vault_status = Some(VaultStatus {
                    is_locked: false,
                    pda: None,
                });
                self.balance = Some(0);
                self.is_loading = false;
                self.status_message = Some(format!("⚠ {}", e));
            }
        }
    }

    fn execute_register(&mut self) {
        self.mode = AppMode::RegisterPopup;
        self.action_steps.clear();
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Executing Register...".to_string());
        // Execute immediately
        self.perform_register_action();
    }

    fn execute_lock(&mut self) {
        self.mode = AppMode::LockPopup;
        self.action_steps.clear();
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Executing Lock...".to_string());
        // Execute immediately
        self.perform_lock_action();
    }

    fn execute_unlock(&mut self) {
        self.mode = AppMode::UnlockPopup;
        self.action_steps.clear();
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts

        // Clear any old shared progress state from previous unlock
        self.progress_state_shared = None;

        // Initialize progress bar immediately so it shows in the popup
        self.progress_current = 0;
        self.progress_total = 46;  // Total unlock steps (matches callback)
        self.progress_message = "Initializing unlock process...".to_string();

        // Show one status line
        self.action_steps.push(ActionStep::InProgress("Initializing unlock process...".to_string()));

        self.status_message = Some("Executing Unlock...".to_string());
        self.pending_action = true;  // Set flag to execute on next loop
    }

    fn execute_claim_airdrop(&mut self) {
        self.mode = AppMode::AirdropClaimPopup;
        self.action_steps.clear();
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Claiming Airdrop...".to_string());
        // Execute immediately
        self.perform_claim_airdrop_action();
    }

    fn execute_transfer(&mut self) {
        self.mode = AppMode::TransferPopup;
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.transfer_recipient.clear();
        self.transfer_amount.clear();
        self.transfer_focused_field = TransferInputField::Recipient;
        self.status_message = Some("Enter transfer details...".to_string());
    }

    fn execute_close(&mut self) {
        // Check if vault is locked before allowing close
        if let Some(ref status) = self.vault_status {
            if status.is_locked {
                self.mode = AppMode::ResultPopup;
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error("❌ Cannot close PQ account while locked!".to_string()));
                self.action_steps.push(ActionStep::Error("You must unlock your vault first.".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [Esc] to close this message".to_string()));
                self.needs_clear = true;
                return;
            }
        }

        // Get active vault name
        let vault_name = match VaultConfig::load() {
            Ok(config) => {
                if let Some(active) = config.active_vault {
                    active
                } else {
                    self.status_message = Some("❌ No active vault".to_string());
                    return;
                }
            }
            Err(e) => {
                self.status_message = Some(format!("❌ Failed to load config: {}", e));
                return;
            }
        };

        // Show confirmation popup
        self.vault_to_close = vault_name;
        self.close_confirmation_input.clear();
        self.mode = AppMode::CloseConfirmPopup;
        self.needs_clear = true;
        self.status_message = Some("Type vault name to confirm close".to_string());
    }

    fn record_lock_history(&mut self, force_refresh: bool) -> Result<(f64, usize)> {
        // Query network-wide locked tokens
        let mint = self.mint;
        let vault_client = &self.vault_client;

        self.status_message = Some("🔍 Querying network for locked tokens...".to_string());

        // Get total locked QDUM across all holders
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.get_network_locked_total(mint, force_refresh).await
            })
        });

        match result {
            Ok((total_locked, holder_count)) => {

                // Load history, add entry, and save
                if let Ok(mut history) = LockHistory::load() {
                    history.add_entry(total_locked, holder_count);
                    if let Err(e) = history.save() {
                        self.status_message = Some(format!("⚠️  Failed to save history: {}", e));
                        return Err(e);
                    }
                }

                self.status_message = Some(format!("✅ Recorded: {:.2} QDUM locked ({} holders)", total_locked, holder_count));
                Ok((total_locked, holder_count))
            }
            Err(e) => {
                self.status_message = Some(format!("❌ Failed to query network: {}", e));
                Err(e)
            }
        }
    }

    fn execute_chart(&mut self) {
        // Record current lock status before showing chart (use cache if available)
        let _ = self.record_lock_history(false);

        // Show chart popup
        self.mode = AppMode::ChartPopup;
        self.needs_clear = true;
    }

    fn execute_new_vault(&mut self) {
        self.mode = AppMode::VaultSwitchPopup;
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.action_steps.clear();
        self.new_vault_name.clear();

        // Load vault list
        if let Ok(config) = VaultConfig::load() {
            self.vault_list = config.list_vaults().into_iter().cloned().collect();

            // Find active vault and select it
            if let Some(active_name) = &config.active_vault {
                for (i, vault) in self.vault_list.iter().enumerate() {
                    if &vault.name == active_name {
                        self.selected_vault_index = i;
                        break;
                    }
                }
            }
        } else {
            self.vault_list = Vec::new();
        }

        // Start in list mode
        self.vault_management_mode = VaultManagementMode::List;
        self.status_message = Some("Select vault or create new...".to_string());
    }

    fn validate_transfer_inputs(&mut self) -> bool {
        // Validate recipient
        if self.transfer_recipient.is_empty() {
            self.status_message = Some("❌ Recipient address required".to_string());
            return false;
        }

        // Validate amount
        if self.transfer_amount.is_empty() {
            self.status_message = Some("❌ Amount required".to_string());
            return false;
        }

        // Try parsing recipient to validate format
        if let Err(_) = Pubkey::from_str(&self.transfer_recipient) {
            self.status_message = Some("❌ Invalid recipient address format".to_string());
            return false;
        }

        // Try parsing amount
        if let Err(_) = self.transfer_amount.parse::<f64>() {
            self.status_message = Some("❌ Invalid amount format".to_string());
            return false;
        }

        true
    }

    fn copy_wallet_to_clipboard(&mut self) {
        match Clipboard::new() {
            Ok(mut clipboard) => {
                let wallet_str = self.wallet.to_string();
                match clipboard.set_text(wallet_str) {
                    Ok(_) => {
                        self.status_message = Some("✓ Wallet address copied to clipboard!".to_string());
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to copy to clipboard: {}", e));
                    }
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to access clipboard: {}", e));
            }
        }
    }

    fn perform_register_action(&mut self) {
        if !self.action_steps.is_empty() && !matches!(self.action_steps.last(), Some(ActionStep::Starting)) {
            return; // Already executed
        }

        self.action_steps.clear();

        // Check SOL balance first
        self.action_steps.push(ActionStep::InProgress("Checking wallet balance...".to_string()));

        let vault_client = &self.vault_client;
        let wallet = self.wallet;

        let sol_balance = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.get_sol_balance(wallet).await
                })
            })
        });

        match sol_balance {
            Ok(balance) => {
                if balance < 100_000_000 { // 0.1 SOL minimum
                    self.action_steps.push(ActionStep::Error(format!("Insufficient SOL balance: {} SOL", balance as f64 / 1_000_000_000.0)));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("To fund this wallet:".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  1. Visit: https://faucet.solana.com".to_string()));
                    self.action_steps.push(ActionStep::InProgress(format!("  2. Paste wallet: {}", wallet)));
                    self.action_steps.push(ActionStep::InProgress("  3. Request devnet SOL (airdrop)".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  4. Wait ~30 seconds".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  5. Press R to refresh and try again".to_string()));
                    self.status_message = Some("❌ Insufficient SOL! Fund wallet first.".to_string());
                    return;
                }
                self.action_steps.push(ActionStep::Success(format!("✓ Wallet funded: {} SOL", balance as f64 / 1_000_000_000.0)));
            }
            Err(_) => {
                // Continue anyway - might be RPC issue
                self.action_steps.push(ActionStep::InProgress("⚠ Could not verify balance, continuing...".to_string()));
            }
        }

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

        // Execute the register call (with output suppressed)
        let keypair_path = self.keypair_path.to_str().unwrap();
        let wallet = self.wallet;
        let vault_client = &self.vault_client;
        let keypair_path_str = keypair_path.to_string();

        let result = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.register_pq_account(
                        wallet,
                        &keypair_path_str,
                        &sphincs_pubkey,
                    ).await
                })
            })
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

        // Execute the lock call (with output suppressed)
        let keypair_path = self.keypair_path.to_str().unwrap();
        let wallet = self.wallet;
        let vault_client = &self.vault_client;
        let keypair_path_str = keypair_path.to_string();

        let result = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.lock_vault(wallet, &keypair_path_str).await
                })
            })
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

    fn perform_claim_airdrop_action(&mut self) {
        if !self.action_steps.is_empty() && !matches!(self.action_steps.last(), Some(ActionStep::Starting)) {
            return; // Already executed
        }

        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Checking PQ account...".to_string()));

        // Execute the airdrop claim (with output suppressed)
        let keypair_path = self.keypair_path.to_str().unwrap();
        let wallet = self.wallet;
        let mint = self.mint;
        let vault_client = &self.vault_client;
        let keypair_path_str = keypair_path.to_string();

        let result = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.claim_airdrop(wallet, &keypair_path_str, mint).await
                })
            })
        });

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("✓ Transaction confirmed!".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Claimed 100 QDUM successfully!".to_string()));
                self.action_steps.push(ActionStep::Success("⏰ Next claim available in 24 hours".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [A] to view airdrop pool stats...".to_string()));
                self.status_message = Some("Airdrop claimed!".to_string());
                self.refresh_data();
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("CooldownNotElapsed") || error_msg.contains("Cooldown") {
                    self.action_steps.push(ActionStep::Error("Cooldown period not elapsed - wait 24 hours between claims".to_string()));
                } else if error_msg.contains("AirdropCapExceeded") || error_msg.contains("Cap exceeded") {
                    self.action_steps.push(ActionStep::Error("Airdrop pool exhausted - 3% supply cap reached".to_string()));
                } else if error_msg.contains("PQAccountNotInitialized") || error_msg.contains("not initialized") {
                    self.action_steps.push(ActionStep::Error("PQ account not initialized - register first (press G)".to_string()));
                } else {
                    self.action_steps.push(ActionStep::Error(format!("Airdrop claim failed: {}", e)));
                }
                self.status_message = Some("Airdrop claim failed!".to_string());
            }
        }
    }

    fn perform_unlock_action(&mut self) {
        use std::sync::Arc;
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicBool, Ordering};

        // CRITICAL: Force reset everything to ensure clean state
        self.progress_state_shared = None;
        self.progress_current = 0;
        self.progress_total = 46;  // Match actual callback total
        self.progress_message = "Starting unlock...".to_string();

        // Set initial action steps
        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Loading SPHINCS+ key...".to_string()));

        // Store shared progress state that we'll poll from the UI thread
        let progress_state = Arc::new(Mutex::new((0usize, 46usize, String::from("Loading SPHINCS+ key..."))));
        self.progress_state_shared = Some(Arc::clone(&progress_state));

        // Flag to indicate unlock is complete
        let unlock_complete = Arc::new(AtomicBool::new(false));
        let unlock_complete_clone = Arc::clone(&unlock_complete);
        self.unlock_complete = Some(Arc::clone(&unlock_complete));

        // Spawn unlock operation as a tokio task - DO ALL WORK IN BACKGROUND
        let keypair_path_str = self.keypair_path.to_str().unwrap().to_string();
        let sphincs_public_key_path = self.sphincs_public_key_path.clone();
        let sphincs_private_key_path = self.sphincs_private_key_path.clone();
        let wallet = self.wallet;
        let rpc_url = self.rpc_url.clone();
        let program_id = self.program_id;
        let progress_clone = Arc::clone(&progress_state);

        std::thread::spawn(move || {

            // Create a NEW tokio runtime for this thread
            let rt = match tokio::runtime::Runtime::new() {
                Ok(r) => r,
                Err(_) => return,
            };

            rt.block_on(async move {

            // Redirect stdout/stderr to /dev/null to suppress console output
            use std::fs::OpenOptions;
            use std::os::unix::io::AsRawFd;

            let original_stdout = unsafe { libc::dup(1) };
            let original_stderr = unsafe { libc::dup(2) };

            // Redirect to /dev/null
            let dev_null = OpenOptions::new().write(true).open("/dev/null").ok();
            if let Some(null_file) = dev_null {
                let null_fd = null_file.as_raw_fd();
                unsafe {
                    libc::dup2(null_fd, 1);
                    libc::dup2(null_fd, 2);
                }
            }

            // Load SPHINCS+ key IN BACKGROUND - don't block UI!
            {
                let mut state = progress_clone.lock().unwrap();
                *state = (1, 46, "Loading SPHINCS+ private key...".to_string());
            }

            let key_manager = match SphincsKeyManager::new(None) {
                Ok(km) => km,
                Err(e) => {
                    let mut state = progress_clone.lock().unwrap();
                    *state = (0, 46, format!("Failed to init key manager: {}", e));
                    unsafe {
                        libc::dup2(original_stdout, 1);
                        libc::dup2(original_stderr, 2);
                        libc::close(original_stdout);
                        libc::close(original_stderr);
                    }
                    return;
                }
            };

            let sphincs_privkey = match key_manager.load_private_key(Some(sphincs_private_key_path.clone())) {
                Ok(pk) => pk,
                Err(e) => {
                    let mut state = progress_clone.lock().unwrap();
                    *state = (0, 46, format!("Failed to load private key: {}", e));
                    unsafe {
                        libc::dup2(original_stdout, 1);
                        libc::dup2(original_stderr, 2);
                        libc::close(original_stdout);
                        libc::close(original_stderr);
                    }
                    return;
                }
            };

            let sphincs_pubkey = match key_manager.load_public_key(Some(sphincs_public_key_path)) {
                Ok(pk) => pk,
                Err(e) => {
                    let mut state = progress_clone.lock().unwrap();
                    *state = (0, 46, format!("Failed to load public key: {}", e));
                    unsafe {
                        libc::dup2(original_stdout, 1);
                        libc::dup2(original_stderr, 2);
                        libc::close(original_stdout);
                        libc::close(original_stderr);
                    }
                    return;
                }
            };

            // Update progress - key loaded successfully
            {
                let mut state = progress_clone.lock().unwrap();
                *state = (2, 46, "Creating vault client...".to_string());
            }

            // Create VaultClient in the task
            let vault_client = match VaultClient::new(&rpc_url, program_id) {
                Ok(client) => client,
                Err(e) => {
                    let mut state = progress_clone.lock().unwrap();
                    *state = (0, 46, format!("Failed to create client: {}", e));
                    unsafe {
                        libc::dup2(original_stdout, 1);
                        libc::dup2(original_stderr, 2);
                        libc::close(original_stdout);
                        libc::close(original_stderr);
                    }
                    return;
                }
            };

            // Update progress - ready to unlock
            {
                let mut state = progress_clone.lock().unwrap();
                *state = (3, 46, "Starting unlock verification...".to_string());
            }

            // Create progress callback that updates shared state
            let progress_state_for_callback = Arc::clone(&progress_clone);
            let progress_callback = Some(Box::new(move |current: usize, total: usize, message: String| {
                if let Ok(mut state) = progress_state_for_callback.lock() {
                    *state = (current, total, message);
                }
            }) as Box<dyn FnMut(usize, usize, String) + Send>);

            // Call unlock_vault (stdout/stderr already redirected above)
            let result = vault_client.unlock_vault(
                wallet,
                &keypair_path_str,
                &sphincs_privkey,
                &sphincs_pubkey,
                progress_callback,
            ).await;

            // Update after unlock completes (or fails)
            {
                let mut state = progress_clone.lock().unwrap();
                match &result {
                    Ok(_) => *state = (46, 46, "Unlock completed!".to_string()),
                    Err(e) => *state = (0, 46, format!("Unlock failed: {}", e)),
                }
            }

            // Restore stdout/stderr before task ends
            unsafe {
                libc::dup2(original_stdout, 1);
                libc::dup2(original_stderr, 2);
                libc::close(original_stdout);
                libc::close(original_stderr);
            }

            // Mark as complete
            unlock_complete_clone.store(true, Ordering::SeqCst);

            }); // End rt.block_on
        }); // End std::thread::spawn

        // Note: The main render loop will poll progress_state_shared and update the UI
    }

    fn perform_transfer_action(&mut self) {
        // Check if vault is locked first
        if let Some(ref status) = self.vault_status {
            if status.is_locked {
                self.mode = AppMode::ResultPopup;
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error("❌ Vault is LOCKED!".to_string()));
                self.action_steps.push(ActionStep::Error("You must unlock your vault before transferring tokens.".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press 'U' to unlock your vault first.".to_string()));
                self.status_message = Some("❌ Transfer blocked: Vault is locked".to_string());
                self.transfer_recipient.clear();
                self.transfer_amount.clear();
                return;
            }
        }

        // Validate inputs
        if self.transfer_recipient.is_empty() {
            self.status_message = Some("❌ Recipient address required".to_string());
            return;
        }

        if self.transfer_amount.is_empty() {
            self.status_message = Some("❌ Amount required".to_string());
            return;
        }

        // Parse recipient address
        let recipient = match Pubkey::from_str(&self.transfer_recipient) {
            Ok(pk) => pk,
            Err(e) => {
                self.status_message = Some(format!("❌ Invalid recipient address: {}", e));
                return;
            }
        };

        // Parse amount (in QDUM, convert to base units)
        let amount_qdum: f64 = match self.transfer_amount.parse() {
            Ok(amt) => amt,
            Err(e) => {
                self.status_message = Some(format!("❌ Invalid amount: {}", e));
                return;
            }
        };

        let amount_base_units = (amount_qdum * 1_000_000.0) as u64;

        if amount_base_units == 0 {
            self.status_message = Some("❌ Amount must be greater than 0".to_string());
            return;
        }

        // Check if user has sufficient balance
        if let Some(balance) = self.balance {
            if balance < amount_base_units {
                let balance_qdum = balance as f64 / 1_000_000.0;
                self.mode = AppMode::Normal;
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error("❌ Insufficient balance!".to_string()));
                self.action_steps.push(ActionStep::Error(format!("Your balance: {:.6} QDUM", balance_qdum)));
                self.action_steps.push(ActionStep::Error(format!("Transfer amount: {:.6} QDUM", amount_qdum)));
                self.status_message = Some("❌ Transfer failed: Insufficient balance".to_string());
                self.transfer_recipient.clear();
                self.transfer_amount.clear();
                return;
            }
        }

        // Close the popup and show progress
        self.mode = AppMode::Normal;
        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Checking vault status...".to_string()));
        self.action_steps.push(ActionStep::Success("✓ Vault is unlocked".to_string()));

        // Load keypair
        let keypair_path = self.keypair_path.to_str().unwrap();
        let keypair_path_str = keypair_path.to_string();

        let keypair = match solana_sdk::signature::read_keypair_file(&keypair_path_str) {
            Ok(kp) => kp,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to load keypair: {}", e)));
                self.status_message = Some("Transfer failed!".to_string());
                return;
            }
        };

        self.action_steps.push(ActionStep::Success("✓ Keypair loaded".to_string()));

        // Store recipient for display (truncate if too long)
        let recipient_display = if self.transfer_recipient.len() > 20 {
            format!("{}...{}", &self.transfer_recipient[..8], &self.transfer_recipient[self.transfer_recipient.len()-8..])
        } else {
            self.transfer_recipient.clone()
        };

        self.action_steps.push(ActionStep::InProgress(format!("Sending {} QDUM to {}", amount_qdum, recipient_display)));
        self.action_steps.push(ActionStep::InProgress("Broadcasting transaction to Solana...".to_string()));

        // Execute the transfer
        let vault_client = &self.vault_client;
        let mint = self.mint;

        let result = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.transfer_tokens_with_confirm(
                        &keypair,
                        recipient,
                        mint,
                        amount_base_units,
                        true,  // skip_confirm = true (no interactive prompt)
                    ).await
                })
            })
        });

        // Clear and show final result in popup
        self.action_steps.clear();
        self.mode = AppMode::ResultPopup;

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Success("║      ✓ TRANSFER SUCCESSFUL!             ║".to_string()));
                self.action_steps.push(ActionStep::Success("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success(format!("Amount:     {:.6} QDUM", amount_qdum)));
                self.action_steps.push(ActionStep::Success(format!("Recipient:  {}", recipient_display)));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Transaction confirmed on Solana".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Tokens have been transferred".to_string()));

                // Get new balance
                if let Some(old_balance) = self.balance {
                    let new_balance = old_balance.saturating_sub(amount_base_units);
                    let new_balance_qdum = new_balance as f64 / 1_000_000.0;
                    self.action_steps.push(ActionStep::Success("".to_string()));
                    self.action_steps.push(ActionStep::InProgress(format!("New balance: {:.6} QDUM", new_balance_qdum)));
                }

                self.status_message = Some("✓ Transfer completed successfully!".to_string());
                self.transfer_recipient.clear();
                self.transfer_amount.clear();
                self.refresh_data();
            }
            Err(e) => {
                self.action_steps.push(ActionStep::Error("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Error("║      ✗ TRANSFER FAILED                   ║".to_string()));
                self.action_steps.push(ActionStep::Error("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::Error(format!("Amount:     {:.6} QDUM", amount_qdum)));
                self.action_steps.push(ActionStep::Error(format!("Recipient:  {}", recipient_display)));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::Error(format!("Error: {}", e)));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Common issues:".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Vault might still be locked".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Insufficient SOL for transaction fee".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Network connectivity issues".to_string()));

                self.status_message = Some("❌ Transfer failed!".to_string());
            }
        }
    }

    fn perform_vault_switch(&mut self, vault_name: &str) {
        use std::path::PathBuf;
        use solana_sdk::signature::{read_keypair_file, Signer};
        use std::io::Write;

        // Debug log
        let _ = std::fs::write("/tmp/vault-switch-debug.log", format!("Starting vault switch to: {}\n", vault_name));

        // Load config and switch vault
        match VaultConfig::load() {
            Ok(mut config) => {
                match config.switch_vault(vault_name) {
                    Ok(_) => {
                        let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-switch-debug.log")
                            .and_then(|mut f| writeln!(f, "Switch successful, getting active vault"));

                        // Get the newly active vault
                        if let Some(vault) = config.get_active_vault() {
                            let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-switch-debug.log")
                                .and_then(|mut f| writeln!(f, "Active vault: {}, keypair: {}", vault.name, vault.solana_keypair_path));

                            // Load the keypair to extract the wallet address
                            match read_keypair_file(&vault.solana_keypair_path) {
                                Ok(keypair) => {
                                    let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-switch-debug.log")
                                        .and_then(|mut f| writeln!(f, "Keypair loaded successfully, pubkey: {}", keypair.pubkey()));

                                    // Update all vault-specific state
                                    self.wallet = keypair.pubkey();
                                    self.keypair_path = PathBuf::from(&vault.solana_keypair_path);
                                    self.sphincs_public_key_path = vault.sphincs_public_key_path.clone();
                                    self.sphincs_private_key_path = vault.sphincs_private_key_path.clone();

                                    // Close vault management popup
                                    self.mode = AppMode::Normal;
                                    self.vault_list.clear();

                                    // Show success message
                                    self.status_message = Some(format!("✅ Switched to vault '{}' - Wallet: {}",
                                        vault_name,
                                        self.wallet.to_string().chars().take(8).collect::<String>() + "..."
                                    ));

                                    let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-switch-debug.log")
                                        .and_then(|mut f| writeln!(f, "About to refresh data"));

                                    // Refresh all data with new vault
                                    self.refresh_data();

                                    let _ = std::fs::OpenOptions::new().append(true).open("/tmp/vault-switch-debug.log")
                                        .and_then(|mut f| writeln!(f, "Refresh complete, should stay in dashboard"));
                                }
                                Err(e) => {
                                    self.action_steps.clear();
                                    self.action_steps.push(ActionStep::Error(format!("Failed to load keypair: {}", e)));
                                    self.status_message = Some("❌ Failed to load vault keypair".to_string());
                                    self.mode = AppMode::ResultPopup;
                                    self.vault_list.clear();
                                }
                            }
                        } else {
                            self.action_steps.clear();
                            self.action_steps.push(ActionStep::Error("No active vault after switch".to_string()));
                            self.status_message = Some("❌ Failed to load new vault".to_string());
                            self.mode = AppMode::ResultPopup;
                            self.vault_list.clear();
                        }
                    }
                    Err(e) => {
                        self.action_steps.clear();
                        self.action_steps.push(ActionStep::Error(format!("Failed to switch vault: {}", e)));
                        self.status_message = Some("❌ Failed to switch vault".to_string());
                        self.mode = AppMode::ResultPopup;
                        self.vault_list.clear();
                    }
                }
            }
            Err(e) => {
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error(format!("Failed to load config: {}", e)));
                self.status_message = Some("❌ Failed to load vault config".to_string());
                self.mode = AppMode::ResultPopup;
                self.vault_list.clear();
            }
        }
    }

    fn perform_vault_delete(&mut self, vault_name: &str) {
        use solana_sdk::signature::{read_keypair_file, Signer};

        // Load config
        let mut config = match VaultConfig::load() {
            Ok(c) => c,
            Err(e) => {
                self.status_message = Some(format!("❌ Failed to load vault config: {}", e));
                self.mode = AppMode::VaultSwitchPopup;
                self.vault_management_mode = VaultManagementMode::List;
                self.needs_clear = true;
                return;
            }
        };

        // Get the vault to delete
        let vault = match config.vaults.get(vault_name) {
            Some(v) => v.clone(),
            None => {
                self.status_message = Some(format!("❌ Vault '{}' not found", vault_name));
                self.mode = AppMode::VaultSwitchPopup;
                self.vault_management_mode = VaultManagementMode::List;
                self.needs_clear = true;
                return;
            }
        };

        // Try to close PQ account and reclaim rent first
        match read_keypair_file(&vault.solana_keypair_path) {
            Ok(keypair) => {
                let wallet = keypair.pubkey();

                // Try to close the PQ account (will fail gracefully if doesn't exist or is locked)
                let close_result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        self.vault_client.close_pq_account(wallet, &vault.solana_keypair_path, None).await
                    })
                });

                match close_result {
                    Ok(_) => {
                        self.status_message = Some(format!("💰 Closed PQ account and reclaimed rent for '{}'", vault_name));
                    }
                    Err(e) => {
                        let error_str = format!("{:?}", e);
                        if error_str.contains("AccountNotFound") || error_str.contains("not found") {
                            // No PQ account - that's fine, proceed with deletion
                            self.status_message = Some(format!("ℹ️  No PQ account found for '{}' (already closed or never created)", vault_name));
                        } else if error_str.contains("locked") || error_str.contains("CannotCloseWhileLocked") {
                            // BLOCKED - vault is locked, cannot delete
                            self.status_message = Some(format!("❌ Cannot delete '{}' - PQ account is LOCKED! Unlock first to reclaim rent.", vault_name));
                            self.mode = AppMode::VaultSwitchPopup;
                            self.vault_management_mode = VaultManagementMode::List;
                            self.vault_to_delete.clear();
                            self.delete_confirmation_input.clear();
                            self.needs_clear = true;  // Force terminal clear to prevent glitch
                            return; // Don't proceed with deletion
                        } else {
                            // Unknown error - warn but allow deletion
                            self.status_message = Some(format!("⚠️  Could not close PQ account: {}. Continue deletion anyway?", e));
                            // TODO: Could add another confirmation here
                        }
                    }
                }
            }
            Err(e) => {
                // Can't load keypair - just warn and continue with delete
                self.status_message = Some(format!("⚠️  Could not load keypair: {}. Deleting vault anyway.", e));
            }
        }

        // Now delete the vault from config
        match config.delete_vault(vault_name) {
            Ok(_) => {
                // Check if we deleted the active vault
                if let Some(new_active) = &config.active_vault {
                    self.status_message = Some(format!("✅ Deleted vault '{}'. Active: {}", vault_name, new_active));
                } else {
                    self.status_message = Some(format!("✅ Deleted vault '{}'", vault_name));
                }

                // Reload vault list and stay in VaultSwitchPopup
                self.vault_list = config.list_vaults().into_iter().cloned().collect();
                self.selected_vault_index = 0;
                self.mode = AppMode::VaultSwitchPopup;
                self.vault_management_mode = VaultManagementMode::List;
                self.vault_to_delete.clear();
                self.delete_confirmation_input.clear();
                self.needs_clear = true;  // Force clean display

                // Refresh dashboard data with potentially new active vault
                self.refresh_data();
            }
            Err(e) => {
                self.status_message = Some(format!("❌ Failed to delete vault: {}", e));
                self.mode = AppMode::VaultSwitchPopup;
                self.vault_management_mode = VaultManagementMode::List;
                self.needs_clear = true;
            }
        }
    }

    fn perform_close(&mut self) {
        // Clear any previous steps and show progress
        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Closing PQ account...".to_string()));
        self.mode = AppMode::ResultPopup;
        self.needs_clear = true;

        // Get wallet pubkey and keypair path
        let wallet = self.wallet;
        let keypair_path_str = self.keypair_path.to_str().unwrap().to_string();

        // Execute close
        let vault_client = &self.vault_client;
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.close_pq_account(wallet, &keypair_path_str, None).await
            })
        });

        // Show result
        self.action_steps.clear();
        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Success("║      ✓ PQ ACCOUNT CLOSED!               ║".to_string()));
                self.action_steps.push(ActionStep::Success("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success("✓ PQ account closed successfully".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Rent refunded to your wallet (~0.003 SOL)".to_string()));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Your vault is now closed. You can still:".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Register again to create a new PQ account".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Keep using this wallet for transfers".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [Esc] to close this message".to_string()));
                self.status_message = Some("✅ PQ account closed successfully!".to_string());

                // Refresh dashboard to update vault status
                self.refresh_data();
            }
            Err(e) => {
                self.action_steps.push(ActionStep::Error("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Error("║      ✗ CLOSE FAILED                     ║".to_string()));
                self.action_steps.push(ActionStep::Error("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::Error(format!("Error: {}", e)));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Common issues:".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • PQ account might not exist (already closed?)".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Vault might still be locked".to_string()));
                self.action_steps.push(ActionStep::InProgress("  • Network connectivity issues".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [Esc] to close this message".to_string()));
                self.status_message = Some("❌ Failed to close PQ account".to_string());
            }
        }

        // Clear the confirmation input
        self.vault_to_close.clear();
        self.close_confirmation_input.clear();
    }

    fn perform_new_vault_action(&mut self) {
        use solana_sdk::signature::Signer;
        use solana_sdk::signature::Keypair;

        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress(format!("Creating vault '{}'...", self.new_vault_name)));

        // Load config
        let mut config = match VaultConfig::load() {
            Ok(c) => c,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to load config: {}", e)));
                self.status_message = Some("❌ Failed to load vault config".to_string());
                self.mode = AppMode::ResultPopup;
                return;
            }
        };

        // Check if vault already exists
        if config.vaults.contains_key(&self.new_vault_name) {
            self.action_steps.push(ActionStep::Error(format!("Vault '{}' already exists", self.new_vault_name)));
            self.status_message = Some("❌ Vault already exists!".to_string());
            self.mode = AppMode::ResultPopup;
            return;
        }

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => {
                self.action_steps.push(ActionStep::Error("Could not determine home directory".to_string()));
                self.status_message = Some("❌ Failed to create vault".to_string());
                self.mode = AppMode::ResultPopup;
                return;
            }
        };
        let qdum_dir = home.join(".qdum");
        let vault_dir = qdum_dir.join(&self.new_vault_name);

        // Create vault directory
        if let Err(e) = fs::create_dir_all(&vault_dir) {
            self.action_steps.push(ActionStep::Error(format!("Failed to create directory: {}", e)));
            self.status_message = Some("❌ Failed to create vault directory".to_string());
            self.mode = AppMode::ResultPopup;
            return;
        }

        self.action_steps.push(ActionStep::Success("Vault directory created".to_string()));

        // Generate SPHINCS+ keys
        self.action_steps.push(ActionStep::InProgress("Generating SPHINCS+ keys...".to_string()));
        let key_manager = match SphincsKeyManager::new(Some(vault_dir.to_str().unwrap().to_string())) {
            Ok(km) => km,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to create key manager: {}", e)));
                self.status_message = Some("❌ Failed to generate keys".to_string());
                self.mode = AppMode::ResultPopup;
                return;
            }
        };

        if let Err(e) = key_manager.generate_and_save_keypair() {
            self.action_steps.push(ActionStep::Error(format!("Failed to generate SPHINCS+ keys: {}", e)));
            self.status_message = Some("❌ Failed to generate keys".to_string());
            self.mode = AppMode::ResultPopup;
            return;
        }

        self.action_steps.push(ActionStep::Success("SPHINCS+ keys generated".to_string()));

        // Generate Solana keypair
        self.action_steps.push(ActionStep::InProgress("Generating Solana keypair...".to_string()));
        let solana_keypair = Keypair::new();
        let wallet_address = solana_keypair.pubkey().to_string();

        let solana_keypair_path = vault_dir.join("solana-keypair.json");
        let keypair_json = match serde_json::to_string(&solana_keypair.to_bytes().to_vec()) {
            Ok(j) => j,
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("Failed to serialize keypair: {}", e)));
                self.status_message = Some("❌ Failed to save keypair".to_string());
                self.mode = AppMode::ResultPopup;
                return;
            }
        };

        if let Err(e) = fs::write(&solana_keypair_path, keypair_json) {
            self.action_steps.push(ActionStep::Error(format!("Failed to write keypair: {}", e)));
            self.status_message = Some("❌ Failed to save keypair".to_string());
            self.mode = AppMode::ResultPopup;
            return;
        }

        self.action_steps.push(ActionStep::Success("Solana keypair generated".to_string()));

        // Create vault profile
        let mut profile = crate::vault_manager::VaultProfile::new(
            self.new_vault_name.clone(),
            solana_keypair_path.to_str().unwrap().to_string(),
            vault_dir.join("sphincs_public.key").to_str().unwrap().to_string(),
            vault_dir.join("sphincs_private.key").to_str().unwrap().to_string(),
            wallet_address.clone(),
        );
        profile.description = Some("Created from dashboard".to_string());

        // Create and switch to vault
        if let Err(e) = config.create_vault(self.new_vault_name.clone(), profile) {
            self.action_steps.push(ActionStep::Error(format!("Failed to save vault: {}", e)));
            self.status_message = Some("❌ Failed to save vault config".to_string());
            self.mode = AppMode::ResultPopup;
            return;
        }

        if let Err(e) = config.switch_vault(&self.new_vault_name) {
            self.action_steps.push(ActionStep::Error(format!("Failed to switch vault: {}", e)));
            self.status_message = Some("❌ Failed to switch vault".to_string());
            self.mode = AppMode::ResultPopup;
            return;
        }

        // Update dashboard state with new vault info FIRST
        use solana_sdk::signature::read_keypair_file;
        use std::path::PathBuf;

        match read_keypair_file(&solana_keypair_path) {
            Ok(keypair) => {
                self.wallet = keypair.pubkey();
                self.keypair_path = PathBuf::from(&solana_keypair_path);
                self.sphincs_public_key_path = vault_dir.join("sphincs_public.key").to_str().unwrap().to_string();
                self.sphincs_private_key_path = vault_dir.join("sphincs_private.key").to_str().unwrap().to_string();

                // Clear the input
                self.new_vault_name.clear();

                // Refresh data with new vault
                self.refresh_data();

                // NOW clear and set up the success popup
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Success("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Success("║      ✓ VAULT CREATED!                   ║".to_string()));
                self.action_steps.push(ActionStep::Success("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success(format!("Vault Name: {}", config.get_active_vault().map(|v| v.name.as_str()).unwrap_or("Unknown"))));
                self.action_steps.push(ActionStep::Success(format!("Wallet:     {}", wallet_address)));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success("✓ SPHINCS+ keys generated".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Solana keypair generated".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Vault activated".to_string()));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [Esc] to close this message".to_string()));

                // Show success message
                self.status_message = Some("✅ Vault created successfully!".to_string());

                // Show result popup
                self.needs_clear = true;  // Force terminal clear for clean display
                self.mode = AppMode::ResultPopup;
            }
            Err(e) => {
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Error("║      ✗ VAULT LOAD FAILED                ║".to_string()));
                self.action_steps.push(ActionStep::Error("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::Error("Vault was created but failed to load:".to_string()));
                self.action_steps.push(ActionStep::Error(format!("{}", e)));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [Esc] to close this message".to_string()));
                self.status_message = Some("⚠️  Vault created but failed to load".to_string());
                self.needs_clear = true;  // Force terminal clear for clean display
                self.mode = AppMode::ResultPopup;
            }
        }
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Always render background with quantum gradient effect (dark purple/blue)
        // Render background with quantum gradient effect (dark purple/blue)
        let bg_block = Block::default()
            .style(Style::default().bg(Color::Rgb(15, 5, 35)));  // Dark purple background
        f.render_widget(bg_block, size);

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(5),  // Header
                    Constraint::Length(6),  // Wallet info (expanded for PQ account)
                    Constraint::Min(8),     // Main content
                    Constraint::Length(6),  // Footer + status (3 lines each = 6 total)
                ]
                .as_ref(),
            )
            .split(size);

        // Full-width animated header with block gradient animations
        let pulse = self.get_pulse_intensity();
        let _bright = self.get_pulse_color_bright();

        // Calculate dynamic width based on terminal size
        let width = chunks[0].width as usize;
        let border_line = "═".repeat(width.saturating_sub(2));
        let content_width = width.saturating_sub(2);  // Subtract border characters

        // Simpler block gradient - create flowing pattern as a string
        let block_chars = ['░', '▒', '▓', '█'];
        let mut gradient_bg = String::new();
        for i in 0..content_width {
            let idx = ((self.animation_frame as usize + i) / 4) % 4;
            gradient_bg.push(block_chars[idx]);
        }

        // Main title text - simple bold on gradient background
        let main_title = "Q D U M   -   QUANTUM RESISTANT VAULT";
        let subtitle = "POST-QUANTUM CRYPTOGRAPHY  •  SPHINCS+ SIGNATURES";

        let header = vec![
            Line::from(Span::styled(
                format!("╔{}╗", border_line),
                Style::default()
                    .fg(Color::Rgb(0, pulse, 200))
                    .add_modifier(Modifier::BOLD),
            )),
            // Animated block gradient background line
            Line::from(vec![
                Span::styled("║", Style::default().fg(Color::Rgb(0, pulse, 200))),
                Span::styled(
                    gradient_bg,
                    Style::default()
                        .fg(Color::Rgb(0, pulse / 2, 150))
                        .bg(Color::Rgb(0, pulse, 200))
                ),
                Span::styled("║", Style::default().fg(Color::Rgb(0, pulse, 200))),
            ]),
            // Main title line
            Line::from(vec![
                Span::styled("║", Style::default().fg(Color::Rgb(0, pulse, 200))),
                Span::styled(
                    format!("{:^width$}", main_title, width = content_width),
                    Style::default()
                        .fg(Color::Rgb(255, 255, 255))
                        .bg(Color::Rgb(0, pulse, 200))
                        .add_modifier(Modifier::BOLD)
                ),
                Span::styled("║", Style::default().fg(Color::Rgb(0, pulse, 200))),
            ]),
            Line::from(vec![
                Span::styled("║", Style::default().fg(Color::Rgb(0, pulse, 200))),
                Span::styled(
                    format!("{:^width$}", subtitle, width = content_width),
                    Style::default()
                        .fg(Color::Rgb(25, 10, 50))
                        .bg(Color::Rgb(0, pulse, 200))
                        .add_modifier(Modifier::BOLD)
                ),
                Span::styled("║", Style::default().fg(Color::Rgb(0, pulse, 200))),
            ]),
            Line::from(Span::styled(
                format!("╚{}╝", border_line),
                Style::default()
                    .fg(Color::Rgb(0, pulse, 200))
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        let header_paragraph = Paragraph::new(header)
            .alignment(Alignment::Left)
            .style(Style::default().bg(Color::Rgb(25, 10, 50)));
        f.render_widget(header_paragraph, chunks[0]);

        // Account info with clean table layout
        let mut account_rows = vec![
            // Wallet address row
            Row::new(vec![
                Line::from(Span::styled("WALLET", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(vec![
                    Span::styled(self.wallet.to_string(), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
                    Span::styled("  [C] COPY", Style::default().fg(Theme::SUBTEXT0)),
                ]),
            ]),
        ];

        // Add PQ Account and State rows if available
        if let Some(ref status) = self.vault_status {
            if let Some(pda) = status.pda {
                let state_text = if status.is_locked { "🔒 LOCKED" } else { "🔓 UNLOCKED" };
                let state_color = if status.is_locked { Theme::RED_NEON } else { Theme::GREEN_NEON };

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                    Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                ]));

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("PQ ACCOUNT", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(pda.to_string(), Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD))),
                ]));

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("STATE", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(state_text, Style::default().fg(state_color).add_modifier(Modifier::BOLD))),
                ]));
            } else {
                // PDA not available - vault not registered
                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                    Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                ]));

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("PQ ACCOUNT", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("NOT REGISTERED - Use [G]", Style::default().fg(Theme::ORANGE_NEON).add_modifier(Modifier::BOLD))),
                ]));
            }
        }

        let account_widths = [Constraint::Length(20), Constraint::Min(40)];

        let pulse_wallet = self.get_pulse_intensity();
        let account_table = Table::new(account_rows, account_widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(150, 0, pulse_wallet)))
                    .border_type(BorderType::Rounded)
                    .title(format!(" {} ACCOUNT INFO ", Icons::INFO))
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2);

        f.render_widget(account_table, chunks[1]);

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

        // Render popups on top of dashboard (NO early returns, NO full screen clears)
        match self.mode {
            AppMode::UnlockPopup => self.render_unlock_popup(f, size),
            AppMode::RegisterPopup => self.render_action_popup(f, size, "REGISTER", Color::Green),
            AppMode::LockPopup => self.render_action_popup(f, size, "LOCK VAULT", Color::Red),
            AppMode::TransferPopup => self.render_transfer_popup(f, size),
            AppMode::AirdropClaimPopup => self.render_action_popup(f, size, "CLAIM AIRDROP", Theme::CYAN_NEON),
            AppMode::AirdropStatsPopup => self.render_airdrop_stats_popup(f, size),
            AppMode::VaultSwitchPopup => self.render_vault_switch_popup(f, size),
            AppMode::DeleteConfirmPopup => self.render_delete_confirm_popup(f, size),
            AppMode::CloseConfirmPopup => self.render_close_confirm_popup(f, size),
            AppMode::ChartPopup => self.render_chart_popup(f, size),
            AppMode::ResultPopup => {
                self.render_transfer_result_popup(f, size);
            }
            _ => {}
        }
    }

    fn render_status_panel(&self, f: &mut Frame, area: Rect) {
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

        // Format balance
        let balance_text = if let Some(balance) = self.balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            format!("{:.6} QDUM", balance_qdum)
        } else {
            "Loading...".to_string()
        };

        // Build table rows with clean data organization
        let rows = vec![
            Row::new(vec![
                Line::from(Span::styled("VAULT", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(vault_name, Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("STATUS", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]),
            Row::new(vec![
                Line::from(Span::styled("BALANCE", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(balance_text, Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
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

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::quantum()))
                    .border_type(BorderType::Rounded)
                    .title(" VAULT STATUS ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2);

        f.render_widget(table, area);
    }

    fn render_actions_panel(&self, f: &mut Frame, area: Rect) {
        // Define actions with clean data structure
        let actions = vec![
            ("🔐 REGISTER", "[G]", "Initialize PQ account", Theme::GREEN),
            ("🔒 LOCK", "[L]", "Secure vault", Theme::RED),
            ("🔓 UNLOCK", "[U]", "Verify signature", Theme::YELLOW),
            ("💸 TRANSFER", "[T]", "Send tokens", Theme::CYAN),
            ("🎁 AIRDROP", "[A]", "Claim 100 QDUM (24h cooldown)", Theme::CYAN_NEON),
            ("📦 POOL", "[P]", "View airdrop pool stats", Theme::YELLOW_NEON),
            ("❌ CLOSE", "[X]", "Close & reclaim rent", Theme::RED_NEON),
            ("📊 Network", "[M]", "Locked QDUM and Holder chart", Theme::CYAN_NEON),
            ("🗄️ VAULTS", "[V/N]", "Manage vaults", Theme::PURPLE),
        ];

        // Build table rows
        let rows: Vec<Row> = actions
            .iter()
            .map(|(action, key, desc, color)| {
                Row::new(vec![
                    Line::from(Span::styled(*action, Style::default().fg(*color).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(*key, Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(*desc, Style::default().fg(Theme::SUBTEXT1))),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(15),  // Action name
            Constraint::Length(6),   // Key
            Constraint::Min(20),     // Description
        ];

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::quantum()))
                    .border_type(BorderType::Rounded)
                    .title(" QUICK ACTIONS ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2)
            .header(
                Row::new(vec![
                    Line::from(Span::styled("ACTION", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("KEY", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("DESCRIPTION", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                ])
                .style(Style::default().bg(Theme::SURFACE0))
                .bottom_margin(1)
            );

        f.render_widget(table, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
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
            .style(Style::default().bg(Theme::PANEL_BG))
            .alignment(Alignment::Center);
        f.render_widget(footer, footer_chunks[0]);

        // Status message - prioritize unlock success message
        let status_msg = if let Some(ref success_msg) = self.unlock_success_message {
            success_msg.clone()
        } else if let Some(ref msg) = self.status_message {
            msg.clone()
        } else {
            "Ready - Press H or ? for help, Q to quit".to_string()
        };

        let status_color = if self.unlock_success_message.as_ref()
            .map(|m| m.starts_with("✓"))
            .unwrap_or(false) {
            Theme::GREEN_NEON
        } else if self.unlock_success_message.as_ref()
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
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))
            )
            .style(Style::default().bg(Theme::PANEL_BG));

        f.render_widget(status_widget, footer_chunks[1]);
    }

    fn render_help_overlay(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(Span::styled(
                "QDUM VAULT - HELP",
                Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD),
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
            Line::from(Span::styled("  A           - Claim 100 QDUM airdrop (24h cooldown)", Style::default().fg(Theme::TEXT))),
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

        let help_paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" HELP ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(help_paragraph, help_area);
    }

    fn render_action_popup(&self, f: &mut Frame, area: Rect, title: &str, title_color: Color) {
        let popup_area = centered_rect(70, 70, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows from action steps
        let mut rows = vec![];

        if self.action_steps.is_empty() {
            rows.push(Row::new(vec![
                Line::from(Span::styled("STATUS", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("Initializing...", Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::ITALIC))),
            ]));
        } else {
            for (idx, step) in self.action_steps.iter().enumerate() {
                let (icon, message, color) = match step {
                    ActionStep::Starting => ("⏳", "Preparing...", Theme::YELLOW_NEON),
                    ActionStep::InProgress(msg) => ("⚡", msg.as_str(), Theme::CYAN_NEON),
                    ActionStep::Success(msg) => ("✓", msg.as_str(), Theme::GREEN_NEON),
                    ActionStep::Error(msg) => ("✗", msg.as_str(), Theme::RED_NEON),
                };

                let step_label = format!("STEP {}", idx + 1);
                rows.push(Row::new(vec![
                    Line::from(Span::styled(step_label, Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::styled(format!("{} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                        Span::styled(message, Style::default().fg(color)),
                    ]),
                ]));

                // Add separator between steps
                if idx < self.action_steps.len() - 1 {
                    rows.push(Row::new(vec![
                        Line::from(Span::styled("━━━━━━━━", Style::default().fg(Theme::DIM))),
                        Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                    ]));
                }
            }
        }

        // Add progress info if available
        if self.progress_total > 0 && title == "UNLOCK VAULT" {
            rows.push(Row::new(vec![
                Line::from(Span::styled("━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]));

            rows.push(Row::new(vec![
                Line::from(Span::styled("PROGRESS", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(
                    format!("{}/{} steps", self.progress_current, self.progress_total),
                    Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD),
                )),
            ]));

            if !self.progress_message.is_empty() {
                rows.push(Row::new(vec![
                    Line::from(Span::styled("MESSAGE", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(&self.progress_message, Style::default().fg(Theme::SUBTEXT1))),
                ]));
            }
        }

        // Add controls row
        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("CONTROLS", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(" [Esc] ", Style::default().fg(Theme::TEXT).bg(title_color).add_modifier(Modifier::BOLD)),
                Span::styled(" Close", Style::default().fg(Theme::TEXT)),
            ]),
        ]));

        let widths = [Constraint::Length(12), Constraint::Min(40)];

        // Pulse effect for border
        let pulse = self.get_pulse_intensity();
        let border_color = match title {
            "REGISTER VAULT" | "REGISTER" => Color::Rgb(0, (100 + pulse / 2) as u8, (200 + pulse / 4) as u8),
            "LOCK VAULT" => Color::Rgb((200 + pulse / 4) as u8, (50 + pulse / 5) as u8, 50),
            _ => Color::Rgb((150 + pulse / 3) as u8, (100 + pulse / 4) as u8, (200 + pulse / 4) as u8),
        };

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(format!(" {} ", title))
                    .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2);

        f.render_widget(table, popup_area);
    }

    fn render_unlock_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(75, 75, area);
        f.render_widget(Clear, popup_area);

        let pulse = self.get_pulse_intensity();

        // Split popup into animation area (top) and info table (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(popup_area);

        // Top section: Animated quantum blocks with title
        let anim_area = chunks[0];
        let block_chars = ["█", "▓", "▒", "░"];
        let blocks_per_row = (anim_area.width.saturating_sub(2)) as usize;
        let anim_rows = (anim_area.height.saturating_sub(2)) as usize;

        let mut anim_lines = vec![];
        for row in 0..anim_rows {
            let mut spans = vec![];
            for i in 0..blocks_per_row {
                let offset = (i + row * 3 + self.animation_frame as usize) % 8;
                let block_idx = offset / 2;
                let block_char = block_chars[block_idx.min(3)];
                let intensity = ((offset as f64 / 8.0) * 155.0 + 100.0) as u8;
                let color = Color::Rgb(intensity / 2, intensity, intensity + 50);
                spans.push(Span::styled(block_char, Style::default().fg(color)));
            }
            anim_lines.push(Line::from(spans));
        }

        let anim_widget = Paragraph::new(anim_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" 🔓 QUANTUM VAULT UNLOCK 🔓 ")
                    .title_style(Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG));

        f.render_widget(anim_widget, anim_area);

        // Bottom section: Static table showing unlock process
        let mut rows = vec![];

        // Header
        rows.push(Row::new(vec![
            Line::from(Span::styled("STEP", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("PROCESS", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Step 1
        rows.push(Row::new(vec![
            Line::from(Span::styled("1", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Load SPHINCS+ private key from vault", Style::default().fg(Theme::TEXT))),
        ]));

        // Step 2
        rows.push(Row::new(vec![
            Line::from(Span::styled("2", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Fetch challenge from on-chain PQ account", Style::default().fg(Theme::TEXT))),
        ]));

        // Step 3
        rows.push(Row::new(vec![
            Line::from(Span::styled("3", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Generate quantum-resistant signature (SPHINCS+)", Style::default().fg(Theme::TEXT))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("", Style::default())),
            Line::from(Span::styled("└─ This step takes ~1-2 minutes", Style::default().fg(Theme::SUBTEXT1).add_modifier(Modifier::ITALIC))),
        ]));

        // Step 4
        rows.push(Row::new(vec![
            Line::from(Span::styled("4", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Submit signature transaction to Solana", Style::default().fg(Theme::TEXT))),
        ]));

        // Step 5
        rows.push(Row::new(vec![
            Line::from(Span::styled("5", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Verify vault unlock status", Style::default().fg(Theme::TEXT))),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("━━━━━━", Style::default().fg(Theme::DIM))),
            Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
        ]));

        // Info message
        rows.push(Row::new(vec![
            Line::from(Span::styled("ℹ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(
                "Quantum-resistant cryptography is computationally intensive.",
                Style::default().fg(Theme::SUBTEXT1),
            )),
        ]));

        rows.push(Row::new(vec![
            Line::from(Span::styled("", Style::default())),
            Line::from(Span::styled(
                "Please wait patiently while SPHINCS+ signature is generated.",
                Style::default().fg(Theme::SUBTEXT1),
            )),
        ]));

        let widths = [Constraint::Length(8), Constraint::Min(50)];

        let info_table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(pulse, pulse, 0)).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" UNLOCK PROCESS ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2);

        f.render_widget(info_table, chunks[1]);
    }

    fn render_transfer_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 65, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Build table rows for transfer form
        let mut rows = vec![];

        // Balance row
        if let Some(balance) = self.balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            rows.push(Row::new(vec![
                Line::from(Span::styled("BALANCE", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(
                    format!("{:.6} QDUM", balance_qdum),
                    Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD),
                )),
            ]));

            rows.push(Row::new(vec![
                Line::from(Span::styled("━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
            ]));
        }

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
            Line::from(Span::styled("RECIPIENT", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
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

        let amount_display = if self.transfer_amount.is_empty() {
            "[Enter amount...]".to_string()
        } else {
            format!("{} QDUM", self.transfer_amount)
        };

        let amount_indicator = if self.transfer_focused_field == TransferInputField::Amount {
            " ◀ ACTIVE"
        } else {
            ""
        };

        rows.push(Row::new(vec![
            Line::from(Span::styled("AMOUNT", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
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
            Line::from(Span::styled("CONTROLS", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled(" [Tab/↑↓] ", Style::default().fg(Theme::TEXT).bg(Theme::BLUE).add_modifier(Modifier::BOLD)),
                Span::styled(" Switch  ", Style::default().fg(Theme::TEXT)),
                Span::styled(" [Enter] ", Style::default().fg(Theme::TEXT).bg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" Send  ", Style::default().fg(Theme::TEXT)),
                Span::styled(" [Esc] ", Style::default().fg(Theme::TEXT).bg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::styled(" Cancel", Style::default().fg(Theme::TEXT)),
            ]),
        ]));

        let widths = [Constraint::Length(14), Constraint::Min(38)];

        // Pulse effect for border
        let pulse = self.get_pulse_intensity();
        let border_color = Color::Rgb(0, (150 + pulse / 3) as u8, (100 + pulse / 4) as u8);

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" 💸 TRANSFER QDUM TOKENS 💸 ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2);

        f.render_widget(table, popup_area);
    }

    fn render_vault_switch_popup(&self, f: &mut Frame, area: Rect) {
        match self.vault_management_mode {
            VaultManagementMode::List => self.render_vault_list(f, area),
            VaultManagementMode::Create => self.render_vault_create(f, area),
        }
    }

    fn render_vault_list(&self, f: &mut Frame, area: Rect) {
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

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" 🗄️  VAULT MANAGEMENT 🗄️  ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(1);

        f.render_widget(table, popup_area);
    }

    fn render_vault_create(&self, f: &mut Frame, area: Rect) {
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
            Line::from(Span::styled("VAULT NAME", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))),
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

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(" 🗄️  CREATE NEW VAULT 🗄️  ")
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(2);

        f.render_widget(table, popup_area);
    }

    fn render_transfer_result_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 70, area);

        // Clear background - IMPORTANT for visibility
        f.render_widget(Clear, popup_area);

        // Determine if there are any errors
        let has_error = self.action_steps.iter().any(|step| matches!(step, ActionStep::Error(_)));
        let success = !has_error;

        // Build table rows from action_steps
        let mut rows = vec![];

        // Display all action steps directly
        if self.action_steps.is_empty() {
            rows.push(Row::new(vec![
                Line::from(Span::styled("No result to display", Style::default().fg(Theme::SUBTEXT1).add_modifier(Modifier::ITALIC))),
            ]));
        } else {
            for step in &self.action_steps {
                let (text, color) = match step {
                    ActionStep::Starting => ("⏳ Starting...".to_string(), Theme::YELLOW_NEON),
                    ActionStep::InProgress(msg) => (msg.clone(), Theme::TEXT),
                    ActionStep::Success(msg) => (msg.clone(), Theme::GREEN_NEON),
                    ActionStep::Error(msg) => (msg.clone(), Theme::RED_NEON),
                };

                rows.push(Row::new(vec![
                    Line::from(Span::styled(text, Style::default().fg(color))),
                ]));
            }
        }

        let widths = [Constraint::Percentage(100)];

        let border_color = if success { Theme::GREEN_NEON } else { Theme::RED_NEON };
        let title = if success { " ✓ ACTION COMPLETE " } else { " ✗ ACTION FAILED " };

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .title(title)
                    .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::PANEL_BG))
            .column_spacing(1);

        f.render_widget(table, popup_area);
    }

    fn render_close_confirm_popup(&self, f: &mut Frame, area: Rect) {
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
                Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD),
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

        // Create table
        let table = Table::new(
            rows,
            [Constraint::Percentage(100)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Theme::YELLOW_NEON))
                .title(" CLOSE PQ ACCOUNT ")
                .title_style(Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Theme::BASE)),
        );

        f.render_widget(table, popup_area);
    }

    fn render_chart_popup(&self, f: &mut Frame, area: Rect) {
        use ratatui::widgets::{Dataset, GraphType};
        use ratatui::symbols;
        use chrono::{DateTime, Utc, Duration as ChronoDuration};

        let popup_area = centered_rect(98, 95, area);  // Full screen chart

        // Clear background
        f.render_widget(Clear, popup_area);

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

            // Ensure minimum range of 10 QDUM for readability
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
                .name("Locked QDUM")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Theme::CYAN_NEON))
                .data(&data_points)
        ];

        // Create chart with dynamic title showing chart type, timeframe, and data count
        let chart_title = format!(" 📊 {} [{} - {} points] ",
            self.chart_type.to_string(),
            self.chart_timeframe.to_string(),
            filtered_entries.len());
        let chart = ratatui::widgets::Chart::new(datasets)
            .block(
                Block::default()
                    .title(chart_title)
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::CYAN_NEON))
                    .border_type(BorderType::Rounded)
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
                    .title("Locked QDUM")
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
            .constraints([
                Constraint::Min(10),        // Chart
                Constraint::Length(8),      // Info panel with timeframe controls
            ])
            .split(popup_area);

        // Render chart
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
                Span::styled(format!("{} (showing: {})", history.entries.len(), filtered_entries.len()), Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("  |  ", Style::default().fg(Theme::DIM)),
                Span::styled("Network Total: ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(
                    if let Some(last) = history.entries.last() {
                        format!("{:.2} QDUM", last.locked_amount)
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
                Span::styled("📊 Chart: ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("[TAB/←→] ", Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(
                    if self.chart_type == ChartType::LockedAmount { "⟪ LOCKED QDUM ⟫" } else { "  LOCKED QDUM  " },
                    Style::default().fg(if self.chart_type == ChartType::LockedAmount { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    if self.chart_type == ChartType::HolderCount { "⟪ LOCKED HOLDERS ⟫" } else { "  LOCKED HOLDERS  " },
                    Style::default().fg(if self.chart_type == ChartType::HolderCount { Theme::CYAN_NEON } else { Theme::SUBTEXT1 }).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(vec![
                Span::styled("⌚ Timeframe: ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
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
                Span::styled("[R] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Refresh  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[L] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("View Log", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ];

        let info_block = Paragraph::new(info_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::DIM))
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(Theme::PANEL_BG)),
            )
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(info_block, chunks[1]);
    }

    fn render_airdrop_stats_popup(&self, f: &mut Frame, area: Rect) {
        // Full screen popup (98% x 95%)
        let popup_area = centered_rect(98, 95, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Render background block to fill entire popup area
        let background = Block::default()
            .style(Style::default().bg(Theme::BASE));
        f.render_widget(background, popup_area);

        // Split layout: Title + Content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(10),    // Content
            ])
            .split(popup_area);

        // Title
        let title = Paragraph::new("🎁 AIRDROP POOL STATISTICS")
            .style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Theme::CYAN_NEON))
                .border_type(BorderType::Rounded)
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
                Span::styled(format!("{:.2} QDUM", total_qdum), Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("  (3% of supply)", Style::default().fg(Theme::DIM)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("✅ Distributed:         ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.2} QDUM", distributed_qdum), Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({:.3}%)", percent_used), Style::default().fg(Theme::GREEN)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💎 Remaining:           ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.2} QDUM", remaining_qdum), Style::default().fg(Theme::YELLOW_NEON).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({:.3}%)", 100.0 - percent_used), Style::default().fg(Theme::YELLOW)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📊 Claims Possible:     ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled(format!("{:.0} more", remaining_qdum / 100.0), Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("  (@ 100 QDUM each)", Style::default().fg(Theme::DIM)),
            ]),
        ];

        let stats = Paragraph::new(stats_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Theme::CYAN))
                .border_type(BorderType::Rounded)
                .title(" Pool Status ")
                .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Theme::PANEL_BG)))
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
            .block(
                Block::default()
                    .title(format!(" 📉 Airdrop Pool Depletion [{} - {} snapshots] ",
                        self.airdrop_timeframe.to_string(),
                        filtered_entries.len()))
                    .title_style(Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::CYAN))
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(Theme::PANEL_BG)),
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
                    .title("Remaining QDUM")
                    .style(Style::default().fg(Theme::SUBTEXT1))
                    .bounds([y_min, y_max])
                    .labels(vec![
                        Span::styled(format!("{:.0}", y_min), Style::default().fg(Theme::SUBTEXT1)),
                        Span::styled(format!("{:.0}", (y_min + y_max) / 2.0), Style::default().fg(Theme::SUBTEXT1)),
                        Span::styled(format!("{:.0}", y_max), Style::default().fg(Theme::SUBTEXT1)),
                    ])
            );

        f.render_widget(chart, content_chunks[1]);

        // Help text
        let help_text = vec![
            Line::from(vec![
                Span::styled("[Esc] ", Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("Close  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[M] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("5Min  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[1] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("1D  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[5] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("5D  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[7] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("1W  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[3] ", Style::default().fg(Theme::CYAN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("1M  ", Style::default().fg(Theme::SUBTEXT1)),
                Span::styled("[A] ", Style::default().fg(Theme::GREEN_NEON).add_modifier(Modifier::BOLD)),
                Span::styled("All", Style::default().fg(Theme::SUBTEXT1)),
            ]),
        ];
        let help = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Theme::DIM))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(Theme::PANEL_BG)));
        f.render_widget(help, content_chunks[2]);
    }

    fn render_delete_confirm_popup(&self, f: &mut Frame, area: Rect) {
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

        // Create table
        let table = Table::new(
            rows,
            [Constraint::Percentage(100)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Theme::RED_NEON))
                .title(" DELETE VAULT ")
                .title_style(Style::default().fg(Theme::RED_NEON).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Theme::BASE)),
        );

        f.render_widget(table, popup_area);
    }
}

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
