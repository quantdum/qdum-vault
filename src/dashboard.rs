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
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use solana_sdk::pubkey::Pubkey;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::crypto::sphincs::SphincsKeyManager;
use crate::solana::client::VaultClient;
use crate::icons::Icons;

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

        // Wallet info with quantum gradient background
        let mut wallet_text = vec![
            Line::from(vec![
                Span::styled(format!("{} ", Icons::WALLET), Style::default().fg(Color::Rgb(0, 255, 200))),
                Span::styled("WALLET: ", Style::default().fg(Color::Rgb(255, 0, 200))),  // Magenta
                Span::styled(
                    self.wallet.to_string(),
                    Style::default()
                        .fg(Color::Rgb(0, 255, 255))  // Bright cyan
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  [C] COPY", Style::default().fg(Color::Rgb(150, 150, 255))),
            ]),
            Line::from(""),
        ];

        // Add PQ Account info if available
        if let Some(ref status) = self.vault_status {
            if let Some(pda) = status.pda {
                let state_text = if status.is_locked { "LOCKED" } else { "UNLOCKED" };
                let state_color = if status.is_locked {
                    Color::Rgb(255, 100, 100)  // Red for locked
                } else {
                    Color::Rgb(100, 255, 100)  // Green for unlocked
                };

                wallet_text.push(Line::from(vec![
                    Span::styled(format!("{} ", Icons::QUANTUM), Style::default().fg(Color::Rgb(150, 0, 255))),
                    Span::styled("PQ ACCOUNT: ", Style::default().fg(Color::Rgb(255, 0, 200))),
                    Span::styled(
                        pda.to_string(),
                        Style::default()
                            .fg(Color::Rgb(200, 150, 255))  // Light purple
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                wallet_text.push(Line::from(vec![
                    Span::styled(format!("{} ", Icons::SECURITY), Style::default().fg(Color::Rgb(255, 200, 0))),
                    Span::styled("STATE: ", Style::default().fg(Color::Rgb(255, 0, 200))),
                    Span::styled(
                        state_text,
                        Style::default()
                            .fg(state_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                // PDA not available - vault not registered
                wallet_text.push(Line::from(vec![
                    Span::styled(format!("{} ", Icons::QUANTUM), Style::default().fg(Color::Rgb(150, 0, 255))),
                    Span::styled("PQ ACCOUNT: ", Style::default().fg(Color::Rgb(255, 0, 200))),
                    Span::styled(
                        "NOT REGISTERED - Use [R] to Register",
                        Style::default()
                            .fg(Color::Rgb(255, 150, 0))  // Orange warning
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }
        let pulse_wallet = self.get_pulse_intensity();
        let wallet_paragraph = Paragraph::new(wallet_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(150, 0, pulse_wallet)))  // Pulsing purple border
                    .title(format!(" {} ACCOUNT INFO ", Icons::INFO))
                    .title_style(Style::default()
                        .fg(Color::Rgb(0, 255, 200))  // Static cyan title
                        .add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Color::Rgb(25, 10, 50)))  // Purple bg
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
        let status_text = if let Some(ref status) = self.vault_status {
            if status.is_locked {
                format!("{} LOCKED", Icons::LOCKED_STATUS)
            } else {
                format!("{} UNLOCKED", Icons::UNLOCKED_STATUS)
            }
        } else {
            format!("{} Loading...", Icons::LOADING)
        };

        let pulse = self.get_pulse_intensity();
        let bright = self.get_pulse_color_bright();

        // Much more dramatic status colors with wider range
        let (status_color, status_bg, status_icon) = if let Some(ref status) = self.vault_status {
            if status.is_locked {
                // LOCKED: Dramatic red/pink pulsing with icon
                let icon = if bright { "▓▓▓ [X] LOCKED ▓▓▓" } else { "░░░ [X] LOCKED ░░░" };
                (Color::Rgb(pulse, 0, 100), Color::Rgb(pulse / 2, 0, 40), icon)
            } else {
                // UNLOCKED: Dramatic green pulsing with icon
                let icon = if bright { "▓▓▓ [O] UNLOCKED ▓▓▓" } else { "░░░ [O] UNLOCKED ░░░" };
                (Color::Rgb(0, pulse, 100), Color::Rgb(0, pulse / 2, 30), icon)
            }
        } else {
            let icon = if bright { "▓▓▓ [~] LOADING ▓▓▓" } else { "░░░ [~] LOADING ░░░" };
            (Color::Rgb(pulse, pulse, 0), Color::Rgb(pulse / 2, pulse / 2, 0), icon)
        };

        let balance_text = if let Some(balance) = self.balance {
            // Convert from base units (6 decimals) to human-readable QDUM
            let balance_qdum = balance as f64 / 1_000_000.0;
            format!("{:.6} QDUM", balance_qdum)
        } else {
            "Loading...".to_string()
        };

        let items = vec![
            // LARGE ANIMATED STATUS DISPLAY
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    status_icon,
                    Style::default()
                        .fg(status_color)
                        .bg(status_bg)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            ListItem::new(Line::from("")),
            // LARGE ANIMATED BALANCE DISPLAY - Solid fill with inverted text
            ListItem::new(Line::from(vec![
                Span::styled("  ┌────────────────────────┐", Style::default().fg(Color::Rgb(pulse, pulse, 0))),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("  │", Style::default().fg(Color::Rgb(pulse, pulse, 0))),
                Span::styled(format!("{:<24}", balance_text), Style::default()
                    .fg(Color::Rgb(25, 10, 50))  // Background purple color for text
                    .bg(Color::Rgb(pulse, pulse, 0))  // Pulsing yellow solid fill matching border
                    .add_modifier(Modifier::BOLD)),
                Span::styled("│", Style::default().fg(Color::Rgb(pulse, pulse, 0))),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("  └────────────────────────┘", Style::default().fg(Color::Rgb(pulse, pulse, 0))),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", Icons::ALGORITHM), Style::default().fg(Color::Rgb(150, 0, 255))),
                Span::styled("ALGORITHM: ", Style::default().fg(Color::Rgb(255, 0, 200))),
                Span::styled("SPHINCS+-SHA2-128s", Style::default().fg(Color::Rgb(0, 255, 200))),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", Icons::SECURITY), Style::default().fg(Color::Rgb(150, 0, 255))),
                Span::styled("SECURITY: ", Style::default().fg(Color::Rgb(255, 0, 200))),
                Span::styled("NIST FIPS 205", Style::default().fg(Color::Rgb(0, 255, 200))),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", Icons::NETWORK), Style::default().fg(Color::Rgb(0, 200, 255))),
                Span::styled("NETWORK: ", Style::default().fg(Color::Rgb(255, 0, 200))),
                Span::styled("Solana Devnet", Style::default().fg(Color::Rgb(0, 255, 100))),
            ])),
        ];

        let pulse_for_border = self.get_pulse_intensity();
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(150, 0, pulse_for_border)))  // Pulsing purple border
                    .title(" [≡] VAULT STATUS ")
                    .title_style(Style::default()
                        .fg(Color::Rgb(0, 255, 200))  // Static cyan title
                        .add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Color::Rgb(25, 10, 50)));

        f.render_widget(list, area);
    }

    fn render_actions_panel(&self, f: &mut Frame, area: Rect) {
        let actions = vec![
            (format!("{} REGISTER", Icons::REGISTER), "G/1", "Initialize quantum account", Color::Rgb(0, 255, 100)),
            (format!("{} LOCK", Icons::LOCK), "L", "Secure vault with PQ crypto", Color::Rgb(255, 0, 100)),
            (format!("{} UNLOCK", Icons::UNLOCK), "U", "Verify SPHINCS+ signature", Color::Rgb(255, 255, 0)),
            (format!("{} TRANSFER", Icons::TRANSFER), "T/2", "Execute quantum-safe transfer", Color::Rgb(0, 200, 255)),
        ];

        let items: Vec<ListItem> = actions
            .iter()
            .enumerate()
            .map(|(idx, (name, key, desc, color))| {
                let arrow = if idx == self.selected_action {
                    format!("{} ", Icons::ARROW_RIGHT)
                } else {
                    "  ".to_string()
                };

                let bg_color = if idx == self.selected_action {
                    Some(Color::Rgb(40, 40, 80))
                } else {
                    None
                };

                // Create gradient text animation for selected action
                if idx == self.selected_action {
                    let mut spans = vec![
                        Span::styled(arrow, Style::default().fg(*color)),
                    ];

                    // Animated gradient for each character in the name
                    let chars: Vec<char> = name.chars().collect();
                    for (char_idx, ch) in chars.iter().enumerate() {
                        // Create a wave effect across the text
                        let phase = (self.animation_frame as f32 / 10.0) + (char_idx as f32 / 3.0);
                        let wave = ((phase * std::f32::consts::PI).sin() + 1.0) / 2.0;

                        // Neon quantum gradient: Electric blue → Neon magenta → Bright cyan
                        let r = ((wave * 0.7 + 0.3) * 255.0) as u8;  // More red for neon magenta
                        let g = ((0.3 + (1.0 - wave) * 0.5) * 255.0) as u8;  // Controlled green
                        let b = (((1.0 - wave) * 0.8 + 0.2) * 255.0) as u8;  // High blue for electric effect

                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default()
                                .fg(Color::Rgb(r, g, b))
                                .bg(Color::Rgb(40, 40, 80))
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    spans.push(Span::styled(format!(" [{}]", key), Style::default()
                        .fg(Color::Rgb(120, 120, 150))
                        .bg(Color::Rgb(40, 40, 80))));
                    spans.push(Span::styled(format!(" - {}", desc), Style::default()
                        .fg(Color::Rgb(150, 150, 170))
                        .bg(Color::Rgb(40, 40, 80))));

                    ListItem::new(Line::from(spans))
                } else {
                    let style = Style::default().fg(Color::Rgb(200, 200, 220));

                    ListItem::new(Line::from(vec![
                        Span::styled(arrow, Style::default().fg(*color)),
                        Span::styled(name.clone(), style),
                        Span::styled(format!(" [{}]", key), Style::default()
                            .fg(Color::Rgb(120, 120, 150))),
                        Span::styled(format!(" - {}", desc), Style::default()
                            .fg(Color::Rgb(150, 150, 170))),
                    ]))
                }
            })
            .collect();

        let pulse_actions = self.get_pulse_intensity();
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(150, 0, pulse_actions)))  // Pulsing purple border
                    .title(format!(" {} QUANTUM OPERATIONS (↑↓ select, Enter execute) ", Icons::ARROW_RIGHT))
                    .title_style(Style::default()
                        .fg(Color::Rgb(0, 255, 200))  // Static cyan
                        .add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Color::Rgb(25, 10, 50)));

        f.render_widget(list, area);
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
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(Color::Rgb(220, 50, 50))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Quit  ", Style::default().fg(Color::Rgb(200, 200, 220))),
            Span::styled(
                " H/? ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(Color::Rgb(180, 100, 220))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Help  ", Style::default().fg(Color::Rgb(200, 200, 220))),
            Span::styled(
                " R ",
                Style::default()
                    .fg(Color::Rgb(20, 20, 40))
                    .bg(Color::Rgb(100, 200, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Refresh  ", Style::default().fg(Color::Rgb(200, 200, 220))),
            Span::styled(
                " ↑↓/jk ",
                Style::default()
                    .fg(Color::Rgb(20, 20, 40))
                    .bg(Color::Rgb(100, 180, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Navigate  ", Style::default().fg(Color::Rgb(200, 200, 220))),
            Span::styled(
                " Enter ",
                Style::default()
                    .fg(Color::Rgb(20, 20, 40))
                    .bg(Color::Rgb(255, 220, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Execute", Style::default().fg(Color::Rgb(200, 200, 220))),
        ])];
        let pulse_footer = self.get_pulse_intensity();
        let footer = Paragraph::new(footer_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(150, 0, pulse_footer)))  // Pulsing purple border
                    .title(format!(" {} CONTROLS ", Icons::KEYBOARD))
                    .title_style(Style::default()
                        .fg(Color::Rgb(0, 255, 200))  // Static cyan
                        .add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Color::Rgb(25, 10, 50)))
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
            Line::from("  C           - Copy wallet address"),
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

        // Render a solid background block to ensure opacity
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        f.render_widget(background, popup_area);

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

        // Add progress bar if we have progress data (for unlock)
        if self.progress_total > 0 && title == "UNLOCK VAULT" {
            text_lines.push(Line::from(""));
            text_lines.push(Line::from(""));

            let progress_label = format!("{}/{} steps - {}", self.progress_current, self.progress_total, self.progress_message);

            text_lines.push(Line::from(Span::styled(
                progress_label,
                Style::default().fg(Color::Cyan),
            )));
        }

        // Add instructions - always show close button since we auto-execute
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Close"),
        ]));

        let popup_paragraph = Paragraph::new(text_lines)
            .style(Style::default().bg(Color::Black).fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
                    .title(format!(" {} ", title))
                    .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
                    .style(Style::default().bg(Color::Black)),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(popup_paragraph, popup_area);
    }

    fn render_unlock_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 50, area);
        f.render_widget(Clear, popup_area);

        // Calculate percentage - exactly like the reference code
        let percent = if self.progress_total > 0 {
            ((self.progress_current as f64 / self.progress_total as f64) * 100.0) as u16
        } else {
            0
        };

        // Split popup into sections
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(3)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(3),  // Gauge
                Constraint::Min(3),     // Status
                Constraint::Length(2),  // Controls
            ])
            .split(popup_area);

        // Title - add animation frame to verify rendering is happening
        let title = Paragraph::new(format!("UNLOCKING - {}/{} ({}%) Frame:{}",
            self.progress_current,
            self.progress_total,
            percent,
            self.animation_frame % 10))
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(title, chunks[0]);

        // Gauge - EXACTLY like reference code with percent() method
        let label = format!("{}%", percent);
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" Progress "))
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .percent(percent)
            .label(label);
        f.render_widget(gauge, chunks[1]);

        // Status message
        let status_msg = if !self.progress_message.is_empty() {
            self.progress_message.clone()
        } else {
            "Starting...".to_string()
        };
        let status = Paragraph::new(status_msg)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(status, chunks[2]);

        // Controls
        let controls = Paragraph::new("[Esc] Close")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(controls, chunks[3]);

        // Border LAST
        let border = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .title(" UNLOCK VAULT ")
            .style(Style::default().bg(Color::Black));
        f.render_widget(border, popup_area);
    }

    fn render_transfer_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 50, area);

        // Clear background
        f.render_widget(Clear, popup_area);

        // Background block
        let background = Block::default().style(Style::default().bg(Color::Black));
        f.render_widget(background, popup_area);

        let mut text_lines = vec![
            Line::from(Span::styled(
                "═══ TRANSFER TOKENS ═══",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Show current balance
        if let Some(balance) = self.balance {
            let balance_qdum = balance as f64 / 1_000_000.0;
            text_lines.push(Line::from(vec![
                Span::styled("Your Balance: ", Style::default().fg(Color::Gray)),
                Span::styled(format!("{:.6} QDUM", balance_qdum), Style::default().fg(Color::Green)),
            ]));
            text_lines.push(Line::from(""));
        }

        // Recipient field
        let recipient_style = if self.transfer_focused_field == TransferInputField::Recipient {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        text_lines.push(Line::from(vec![
            Span::styled("Recipient: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if self.transfer_recipient.is_empty() {
                    "[Enter wallet address]"
                } else {
                    &self.transfer_recipient
                },
                recipient_style,
            ),
        ]));

        if self.transfer_focused_field == TransferInputField::Recipient {
            text_lines.push(Line::from(Span::styled("▂", Style::default().fg(Color::Yellow))));
        } else {
            text_lines.push(Line::from(""));
        }

        text_lines.push(Line::from(""));

        // Amount field
        let amount_style = if self.transfer_focused_field == TransferInputField::Amount {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        text_lines.push(Line::from(vec![
            Span::styled("Amount (QDUM): ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if self.transfer_amount.is_empty() {
                    "[0.0]"
                } else {
                    &self.transfer_amount
                },
                amount_style,
            ),
        ]));

        if self.transfer_focused_field == TransferInputField::Amount {
            text_lines.push(Line::from(Span::styled("▂", Style::default().fg(Color::Yellow))));
        } else {
            text_lines.push(Line::from(""));
        }

        text_lines.push(Line::from(""));
        text_lines.push(Line::from(""));

        // Instructions
        text_lines.push(Line::from(vec![
            Span::styled("[Tab/↑↓]", Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Switch field  "),
            Span::styled("[Enter]", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Send  "),
            Span::styled("[Esc]", Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" Cancel"),
        ]));

        let popup_paragraph = Paragraph::new(text_lines)
            .style(Style::default().bg(Color::Black).fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    .title(" Transfer QDUM Tokens ")
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
