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
    widgets::{Block, Borders, BorderType, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use solana_sdk::pubkey::Pubkey;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::crypto::sphincs::SphincsKeyManager;
use crate::solana::client::VaultClient;
use crate::icons::Icons;
use crate::theme::Theme;

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
    ResultPopup,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TransferInputField {
    Recipient,
    Amount,
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
    // Animation state
    animation_frame: u8,  // Counter for animation frames
    last_animation_update: std::time::Instant,
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
        mint: Pubkey,
    ) -> Result<Self> {
        let vault_client = VaultClient::new(&rpc_url, program_id)?;

        Ok(Self {
            wallet,
            keypair_path,
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
            animation_frame: 0,
            last_animation_update: std::time::Instant::now(),
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
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.copy_wallet_to_clipboard();
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
                            3 => self.execute_transfer(),
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

    fn execute_transfer(&mut self) {
        self.mode = AppMode::TransferPopup;
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.transfer_recipient.clear();
        self.transfer_amount.clear();
        self.transfer_focused_field = TransferInputField::Recipient;
        self.status_message = Some("Enter transfer details...".to_string());
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

            let sphincs_privkey = match key_manager.load_private_key(None) {
                Ok(pk) => pk,
                Err(e) => {
                    let mut state = progress_clone.lock().unwrap();
                    *state = (0, 46, format!("Failed to load key: {}", e));
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
        let bright = self.get_pulse_color_bright();

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
                    Line::from(Span::styled("NOT REGISTERED - Use [R]", Style::default().fg(Theme::ORANGE_NEON).add_modifier(Modifier::BOLD))),
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
            AppMode::ResultPopup => {
                let has_error = self.action_steps.iter().any(|step| matches!(step, ActionStep::Error(_)));
                let color = if has_error { Color::Red } else { Color::Green };
                self.render_action_popup(f, size, "TRANSFER RESULT", color);
            }
            _ => {}
        }
    }

    fn render_status_panel(&self, f: &mut Frame, area: Rect) {
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
            ("🔐 REGISTER", "[R]", "Initialize PQ account", Theme::GREEN),
            ("🔒 LOCK", "[L]", "Secure vault", Theme::RED),
            ("🔓 UNLOCK", "[U]", "Verify signature", Theme::YELLOW),
            ("💸 TRANSFER", "[T]", "Send tokens", Theme::CYAN),
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
            Line::from(Span::styled("  R           - Refresh status", Style::default().fg(Theme::TEXT))),
            Line::from(Span::styled("  C           - Copy wallet address", Style::default().fg(Theme::TEXT))),
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

        // Dark themed background based on action type
        let bg_color = match title {
            "REGISTER VAULT" => Color::Rgb(5, 10, 20),  // Blue tint
            "LOCK VAULT" => Color::Rgb(20, 5, 5),        // Red tint
            _ => Color::Rgb(10, 5, 20),                  // Purple tint
        };

        let background = Block::default().style(Style::default().bg(bg_color));
        f.render_widget(background, popup_area);

        // Build text from action steps with better formatting
        let mut text_lines = vec![
            Line::from(""),
        ];

        if self.action_steps.is_empty() {
            text_lines.push(Line::from(Span::styled(
                "Initializing...",
                Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
            )));
        } else {
            for step in &self.action_steps {
                let line = match step {
                    ActionStep::Starting => Line::from(vec![
                        Span::styled("⏳ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled("Preparing...", Style::default().fg(Color::White)),
                    ]),
                    ActionStep::InProgress(msg) => Line::from(vec![
                        Span::styled("⚡ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled(msg.clone(), Style::default().fg(Color::Cyan)),
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
                text_lines.push(Line::from("")); // Add spacing between steps
            }
        }

        // Add progress bar if we have progress data (for unlock)
        if self.progress_total > 0 && title == "UNLOCK VAULT" {
            text_lines.push(Line::from(""));
            text_lines.push(Line::from(Span::styled(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                Style::default().fg(Color::DarkGray),
            )));

            let progress_label = format!("Progress: {}/{} steps", self.progress_current, self.progress_total);
            text_lines.push(Line::from(Span::styled(
                progress_label,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));

            text_lines.push(Line::from(Span::styled(
                &self.progress_message,
                Style::default().fg(Color::Gray),
            )));
        }

        // Add instructions with better styling
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(Span::styled(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            Style::default().fg(Color::DarkGray),
        )));
        text_lines.push(Line::from(vec![
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(title_color).add_modifier(Modifier::BOLD)),
            Span::styled(" Close", Style::default().fg(Color::White)),
        ]));

        // Pulse effect for border - adjust intensity based on title color
        let pulse = self.get_pulse_intensity();
        let border_color = match title {
            "REGISTER VAULT" => Color::Rgb(0, (100 + pulse / 2) as u8, (200 + pulse / 4) as u8),  // Blue pulse
            "LOCK VAULT" => Color::Rgb((200 + pulse / 4) as u8, (50 + pulse / 5) as u8, 50),       // Red pulse
            _ => Color::Rgb((150 + pulse / 3) as u8, (100 + pulse / 4) as u8, (200 + pulse / 4) as u8),  // Purple pulse
        };

        let popup_paragraph = Paragraph::new(text_lines)
            .style(Style::default().bg(bg_color).fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(format!(" {} ", title))
                    .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
                    .style(Style::default().bg(bg_color)),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(popup_paragraph, popup_area);
    }

    fn render_unlock_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(80, 80, area);
        f.render_widget(Clear, popup_area);

        let pulse = self.get_pulse_intensity();
        let bg_color = Color::Rgb(10, 5, 20);

        // Calculate how many rows we can fit (popup height - borders - padding)
        let available_height = popup_area.height.saturating_sub(4); // borders + margins
        let total_rows = available_height as usize;
        let center_row = total_rows / 2;

        // Build animated quantum block rows with message in center
        let block_chars = ["█", "▓", "▒", "░"];
        let blocks_per_row = (popup_area.width.saturating_sub(4)) as usize; // full width minus borders

        let mut text_lines = vec![];

        for row in 0..total_rows {
            if row == center_row {
                // Center message row
                let msg = "🔓 UNLOCKING VAULT - PLEASE WAIT... 🔓";
                let padding = (blocks_per_row.saturating_sub(msg.len())) / 2;

                let mut spans = vec![];
                // Blocks before message
                for i in 0..padding {
                    let offset = (i + row * 3 + self.animation_frame as usize) % 8;
                    let block_idx = offset / 2;
                    let block_char = block_chars[block_idx.min(3)];
                    let intensity = ((offset as f64 / 8.0) * 155.0 + 100.0) as u8;
                    let color = Color::Rgb(0, intensity, intensity + 50);
                    spans.push(Span::styled(block_char, Style::default().fg(color).bg(bg_color)));
                }
                // Message
                spans.push(Span::styled(
                    msg,
                    Style::default()
                        .fg(Color::Rgb(255, 255, 0))
                        .bg(bg_color)
                        .add_modifier(Modifier::BOLD)
                ));
                // Blocks after message
                for i in (padding + msg.len())..blocks_per_row {
                    let offset = (i + row * 3 + self.animation_frame as usize) % 8;
                    let block_idx = offset / 2;
                    let block_char = block_chars[block_idx.min(3)];
                    let intensity = ((offset as f64 / 8.0) * 155.0 + 100.0) as u8;
                    let color = Color::Rgb(0, intensity, intensity + 50);
                    spans.push(Span::styled(block_char, Style::default().fg(color).bg(bg_color)));
                }
                text_lines.push(Line::from(spans));
            } else if row == center_row + 1 {
                // Subtext row
                let status = if self.progress_current > 0 && self.progress_current < self.progress_total {
                    format!("Verifying SPHINCS+ signature ({}/{})", self.progress_current, self.progress_total)
                } else {
                    "Starting verification...".to_string()
                };
                let padding = (blocks_per_row.saturating_sub(status.len())) / 2;

                let mut spans = vec![];
                // Blocks before status
                for i in 0..padding {
                    let offset = (i + row * 3 + self.animation_frame as usize) % 8;
                    let block_idx = offset / 2;
                    let block_char = block_chars[block_idx.min(3)];
                    let intensity = ((offset as f64 / 8.0) * 155.0 + 100.0) as u8;
                    let color = Color::Rgb(0, intensity, intensity + 50);
                    spans.push(Span::styled(block_char, Style::default().fg(color).bg(bg_color)));
                }
                // Status
                spans.push(Span::styled(
                    status.clone(),
                    Style::default()
                        .fg(Color::Rgb(150, 150, 200))
                        .bg(bg_color)
                ));
                // Blocks after status
                for i in (padding + status.len())..blocks_per_row {
                    let offset = (i + row * 3 + self.animation_frame as usize) % 8;
                    let block_idx = offset / 2;
                    let block_char = block_chars[block_idx.min(3)];
                    let intensity = ((offset as f64 / 8.0) * 155.0 + 100.0) as u8;
                    let color = Color::Rgb(0, intensity, intensity + 50);
                    spans.push(Span::styled(block_char, Style::default().fg(color).bg(bg_color)));
                }
                text_lines.push(Line::from(spans));
            } else {
                // Full row of animated quantum blocks
                let mut spans = vec![];
                for i in 0..blocks_per_row {
                    // Calculate animation offset based on row, column, and frame
                    let offset = (i + row * 3 + self.animation_frame as usize) % 8;
                    let block_idx = offset / 2;
                    let block_char = block_chars[block_idx.min(3)];

                    // Color gradient based on position and animation
                    let intensity = ((offset as f64 / 8.0) * 155.0 + 100.0) as u8;
                    let color = Color::Rgb(0, intensity, intensity + 50);

                    spans.push(Span::styled(
                        block_char,
                        Style::default().fg(color).bg(bg_color)
                    ));
                }
                text_lines.push(Line::from(spans));
            }
        }

        let popup_paragraph = Paragraph::new(text_lines)
            .style(Style::default().bg(bg_color).fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(pulse, pulse, 0)).add_modifier(Modifier::BOLD))
                    .title(" QUANTUM VAULT UNLOCK ")
                    .title_style(Style::default().fg(Color::Rgb(255, 255, 0)).add_modifier(Modifier::BOLD))
                    .style(Style::default().bg(bg_color)),
            )
            .alignment(Alignment::Left);

        f.render_widget(popup_paragraph, popup_area);
    }

    fn render_transfer_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(65, 55, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Dark themed background with green tint for transfer
        let bg_color = Color::Rgb(5, 15, 10);
        let background = Block::default().style(Style::default().bg(bg_color));
        f.render_widget(background, popup_area);

        let mut text_lines = vec![
            Line::from(""),
        ];

        // Show current balance with better styling
        if let Some(balance) = self.balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            text_lines.push(Line::from(vec![
                Span::styled("💰 Your Balance: ", Style::default().fg(Color::Gray)),
                Span::styled(format!("{:.6} QDUM", balance_qdum), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]));
            text_lines.push(Line::from(""));
            text_lines.push(Line::from(Span::styled(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                Style::default().fg(Color::DarkGray),
            )));
            text_lines.push(Line::from(""));
        }

        // Recipient field with better visual hierarchy
        let recipient_style = if self.transfer_focused_field == TransferInputField::Recipient {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        text_lines.push(Line::from(Span::styled(
            "Recipient Address:",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));

        let recipient_display = if self.transfer_recipient.is_empty() {
            "  [Enter wallet address...]".to_string()
        } else {
            format!("  {}", self.transfer_recipient)
        };

        text_lines.push(Line::from(Span::styled(
            recipient_display,
            recipient_style,
        )));

        if self.transfer_focused_field == TransferInputField::Recipient {
            text_lines.push(Line::from(Span::styled("  ▂▂▂▂▂▂▂▂▂▂▂", Style::default().fg(Color::Yellow))));
        } else {
            text_lines.push(Line::from(""));
        }

        text_lines.push(Line::from(""));

        // Amount field with better visual hierarchy
        let amount_style = if self.transfer_focused_field == TransferInputField::Amount {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        text_lines.push(Line::from(Span::styled(
            "Amount (QDUM):",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));

        let amount_display = if self.transfer_amount.is_empty() {
            "  [0.0]".to_string()
        } else {
            format!("  {}", self.transfer_amount)
        };

        text_lines.push(Line::from(Span::styled(
            amount_display,
            amount_style,
        )));

        if self.transfer_focused_field == TransferInputField::Amount {
            text_lines.push(Line::from(Span::styled("  ▂▂▂▂▂▂▂▂▂▂▂", Style::default().fg(Color::Yellow))));
        } else {
            text_lines.push(Line::from(""));
        }

        text_lines.push(Line::from(""));
        text_lines.push(Line::from(""));

        // Instructions with better styling
        text_lines.push(Line::from(Span::styled(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            Style::default().fg(Color::DarkGray),
        )));
        text_lines.push(Line::from(vec![
            Span::styled(" [Tab/↑↓] ", Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::styled(" Switch   ", Style::default().fg(Color::White)),
            Span::styled(" [Enter] ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" Send   ", Style::default().fg(Color::White)),
            Span::styled(" [Esc] ", Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(" Cancel", Style::default().fg(Color::White)),
        ]));

        // Pulse effect for border
        let pulse = self.get_pulse_intensity();
        let border_color = Color::Rgb(0, (150 + pulse / 3) as u8, (100 + pulse / 4) as u8);

        let popup_paragraph = Paragraph::new(text_lines)
            .style(Style::default().bg(bg_color).fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" TRANSFER QDUM TOKENS ")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    .style(Style::default().bg(Color::Black)),
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
