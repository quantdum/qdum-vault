// Module declarations
pub mod types;
pub mod utils;
pub mod actions;
pub mod ui;

// Re-export commonly used types
pub use types::*;
pub use utils::*;

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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::crypto::sphincs::SphincsKeyManager;
use crate::solana::client::VaultClient;
use crate::icons::Icons;
use crate::theme::Theme;
use crate::vault_manager::VaultConfig;

// Types are now defined in the types module and re-exported above

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
            pq_balance: None,
            standard_balance: None,
            is_loading: false,
            action_steps: Vec::new(),
            vault_client,
            needs_clear: false,
            pending_action: false,
            pending_transfer: false,
            unlock_complete: None,
            unlock_success_message: None,
            lock_complete: None,
            lock_success_message: None,
            transfer_recipient: String::new(),
            transfer_amount: String::new(),
            transfer_focused_field: TransferInputField::TokenType,
            transfer_token_type: TransferTokenType::StandardQcoin,
            in_transfer_form: false,
            bridge_amount: String::new(),
            standard_mint: Pubkey::from_str("GS2tyNMdpiKnQ9AxFhB74SbzYF7NmoTREoKZC6pzxds7").unwrap(),
            pq_mint: mint, // Use the mint passed in (pqcoin)
            new_vault_name: String::new(),
            vault_management_mode: VaultManagementMode::List,
            vault_list: Vec::new(),
            selected_vault_index: 0,
            in_vault_list: false,
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

    // Get pulsing intensity for status (0-255)

    // Get alternate colors for more dramatic effects

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
            // Update animation frame periodically
            // During lock/unlock, update faster for smooth spinner animation (20 FPS)
            let is_unlocking = self.unlock_complete.as_ref().map(|f| !f.load(Ordering::SeqCst)).unwrap_or(false);
            let is_locking = self.lock_complete.as_ref().map(|f| !f.load(Ordering::SeqCst)).unwrap_or(false);

            let animation_interval_ms = if is_unlocking || is_locking {
                16  // ~60 FPS during lock/unlock for smooth animation
            } else {
                150 // Normal speed
            };

            if self.last_animation_update.elapsed().as_millis() > animation_interval_ms {
                self.animation_frame = self.animation_frame.wrapping_add(1);
                self.last_animation_update = std::time::Instant::now();
            }

            // Clear terminal if needed (before rendering)
            if self.needs_clear {
                terminal.clear()?;
                self.needs_clear = false;
            }

            // Debug: log main loop iteration
            if let Some(ref unlock_flag) = self.unlock_complete {
                let is_complete = unlock_flag.load(Ordering::SeqCst);
                let _ = std::fs::OpenOptions::new().append(true).create(true).open("/tmp/main_loop.log")
                    .and_then(|mut f| std::io::Write::write_all(&mut f, format!("Main loop: unlock_complete={}\n", is_complete).as_bytes()));
            }

            // CRITICAL: Render BEFORE checking unlock complete, so final progress is shown
            terminal.draw(|f| self.ui(f))?;

            // Check if unlock is complete (AFTER rendering)
            if let Some(ref unlock_flag) = self.unlock_complete {
                if unlock_flag.load(Ordering::SeqCst) {
                    // Unlock finished - refresh data silently
                    self.mode = AppMode::Normal;
                    self.needs_clear = true;
                    self.action_steps.clear();

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
                        self.status_message = Some("✅ Vault unlocked successfully!".to_string());
                    } else {
                        self.status_message = Some("❌ Failed to verify vault status".to_string());
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

                    // Refresh pq_balance
                    let pq_balance_result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            vault_client.get_balance(wallet, self.pq_mint).await
                        })
                    });
                    if let Ok(bal) = pq_balance_result {
                        self.pq_balance = Some(bal);
                    }

                    // Refresh standard_balance
                    let standard_balance_result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            vault_client.get_balance(wallet, self.standard_mint).await
                        })
                    });
                    if let Ok(bal) = standard_balance_result {
                        self.standard_balance = Some(bal);
                    }

                    // Clear unlock tracking
                    self.unlock_complete = None;
                }
            }

            // Check if lock is complete (AFTER rendering)
            if let Some(ref lock_flag) = self.lock_complete {
                if lock_flag.load(Ordering::SeqCst) {
                    // Lock finished - refresh data silently
                    self.mode = AppMode::Normal;
                    self.needs_clear = true;
                    self.action_steps.clear();

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
                        self.status_message = Some("✅ Vault locked successfully!".to_string());
                    } else {
                        self.status_message = Some("❌ Failed to verify vault status".to_string());
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

                    // Refresh pq_balance
                    let pq_balance_result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            vault_client.get_balance(wallet, self.pq_mint).await
                        })
                    });
                    if let Ok(bal) = pq_balance_result {
                        self.pq_balance = Some(bal);
                    }

                    // Refresh standard_balance
                    let standard_balance_result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            vault_client.get_balance(wallet, self.standard_mint).await
                        })
                    });
                    if let Ok(bal) = standard_balance_result {
                        self.standard_balance = Some(bal);
                    }

                    // Clear lock tracking
                    self.lock_complete = None;
                }
            }
            // FORCE flush to ensure screen updates
            std::io::Write::flush(&mut std::io::stdout())?;


            // Execute pending transfer after UI is drawn
            if self.pending_transfer {
                self.pending_transfer = false;
                self.perform_transfer_action();
            }

            // Read events with timeout to enable animations and progress updates
            // Use shorter timeout during lock/unlock for smooth 20 FPS animation
            let is_unlocking = self.unlock_complete.as_ref().map(|f| !f.load(Ordering::SeqCst)).unwrap_or(false);
            let is_locking = self.lock_complete.as_ref().map(|f| !f.load(Ordering::SeqCst)).unwrap_or(false);

            let poll_duration = if is_unlocking || is_locking {
                std::time::Duration::from_millis(16)  // ~60 FPS during lock/unlock for smooth animation
            } else {
                std::time::Duration::from_millis(150)  // Normal animation speed
            };

            if !event::poll(poll_duration)? {
                // No event, but timeout reached - continue loop to redraw with updated animation/progress
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
                            self.action_steps.push(ActionStep::Error("❌ Failed to read log file".to_string()));
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
                // In action popups, Esc closes, R refreshes
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.action_steps.clear();
                        self.status_message = Some("Popup closed".to_string());
                        self.needs_clear = true;  // Force terminal clear on next loop
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Manual refresh - actually refresh the data
                        self.refresh_data();
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
                        self.transfer_focused_field = TransferInputField::TokenType;
                        self.transfer_token_type = TransferTokenType::StandardQcoin;
                        self.status_message = Some("Transfer cancelled".to_string());
                        self.needs_clear = true;
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        // Switch between fields (forward)
                        self.transfer_focused_field = match self.transfer_focused_field {
                            TransferInputField::TokenType => TransferInputField::Recipient,
                            TransferInputField::Recipient => TransferInputField::Amount,
                            TransferInputField::Amount => TransferInputField::TokenType,
                        };
                    }
                    KeyCode::Up => {
                        // Switch between fields (reverse)
                        self.transfer_focused_field = match self.transfer_focused_field {
                            TransferInputField::TokenType => TransferInputField::Amount,
                            TransferInputField::Recipient => TransferInputField::TokenType,
                            TransferInputField::Amount => TransferInputField::Recipient,
                        };
                    }
                    KeyCode::Left | KeyCode::Right => {
                        // Toggle token type when on that field
                        if self.transfer_focused_field == TransferInputField::TokenType {
                            self.transfer_token_type = match self.transfer_token_type {
                                TransferTokenType::StandardQcoin => TransferTokenType::Pqcoin,
                                TransferTokenType::Pqcoin => TransferTokenType::StandardQcoin,
                            };
                        }
                    }
                    KeyCode::Char(c) => {
                        match self.transfer_focused_field {
                            TransferInputField::TokenType => {
                                // No character input for token type field (use arrow keys)
                            }
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
                            TransferInputField::TokenType => {
                                // No backspace for token type field
                            }
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
            AppMode::WrapPopup => {
                // Handle wrap popup input
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.bridge_amount.clear();
                        self.status_message = Some("Wrap cancelled".to_string());
                        self.needs_clear = true;
                    }
                    KeyCode::Char(c) => {
                        // Only allow numbers and decimal point
                        if c.is_ascii_digit() || c == '.' {
                            self.bridge_amount.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        self.bridge_amount.pop();
                    }
                    KeyCode::Enter => {
                        // Validate amount
                        if !self.bridge_amount.is_empty() {
                            if let Ok(amount_f64) = self.bridge_amount.parse::<f64>() {
                                let amount = (amount_f64 * 1_000_000.0) as u64;
                                let keypair_path = self.keypair_path.clone();
                                let vault_client = self.vault_client.clone();
                                let standard_mint = self.standard_mint;
                                let pq_mint = self.pq_mint;

                                self.bridge_amount.clear();

                                // Clear previous results and show in-progress
                                self.action_steps.clear();
                                self.action_steps.push(ActionStep::InProgress(format!("Wrapping {} qcoin → pqcoin...", amount_f64)));
                                self.mode = AppMode::ResultPopup;
                                self.needs_clear = true;

                                // Perform wrap synchronously (blocking)
                                let result = tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(async {
                                        vault_client.bridge_wrap(
                                            &keypair_path.to_string_lossy(),
                                            amount,
                                            standard_mint,
                                            pq_mint,
                                        ).await
                                    })
                                });

                                self.action_steps.clear();
                                match result {
                                    Ok(sig) => {
                                        self.action_steps.push(ActionStep::Success(format!("✅ Wrapped {:.6} qcoin → {:.6} pqcoin", amount_f64, amount_f64)));
                                        self.action_steps.push(ActionStep::Success(format!("Transaction: {}", sig)));

                                        // Auto-refresh balances after successful wrap
                                        self.refresh_data();
                                    }
                                    Err(e) => {
                                        self.action_steps.push(ActionStep::Error(format!("❌ Wrap failed: {}", e)));
                                    }
                                }
                                self.mode = AppMode::ResultPopup;
                            } else {
                                self.status_message = Some("Invalid amount".to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            AppMode::UnwrapPopup => {
                // Handle unwrap popup input
                match code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.bridge_amount.clear();
                        self.status_message = Some("Unwrap cancelled".to_string());
                        self.needs_clear = true;
                    }
                    KeyCode::Char(c) => {
                        // Only allow numbers and decimal point
                        if c.is_ascii_digit() || c == '.' {
                            self.bridge_amount.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        self.bridge_amount.pop();
                    }
                    KeyCode::Enter => {
                        // Validate amount
                        if !self.bridge_amount.is_empty() {
                            if let Ok(amount_f64) = self.bridge_amount.parse::<f64>() {
                                let amount = (amount_f64 * 1_000_000.0) as u64;
                                let keypair_path = self.keypair_path.clone();
                                let vault_client = self.vault_client.clone();
                                let standard_mint = self.standard_mint;
                                let pq_mint = self.pq_mint;

                                self.bridge_amount.clear();

                                // Clear previous results and show in-progress
                                self.action_steps.clear();
                                self.action_steps.push(ActionStep::InProgress(format!("Unwrapping {} pqcoin → Standard qcoin...", amount_f64)));
                                self.mode = AppMode::ResultPopup;
                                self.needs_clear = true;

                                // Perform unwrap synchronously (blocking)
                                let result = tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(async {
                                        vault_client.bridge_unwrap(
                                            &keypair_path.to_string_lossy(),
                                            amount,
                                            standard_mint,
                                            pq_mint,
                                        ).await
                                    })
                                });

                                self.action_steps.clear();
                                match result {
                                    Ok(sig) => {
                                        self.action_steps.push(ActionStep::Success(format!("✅ Unwrapped {:.6} pqcoin → {:.6} qcoin", amount_f64, amount_f64)));
                                        self.action_steps.push(ActionStep::Success(format!("Transaction: {}", sig)));

                                        // Auto-refresh balances after successful unwrap
                                        self.refresh_data();
                                    }
                                    Err(e) => {
                                        self.action_steps.push(ActionStep::Error(format!("❌ Unwrap failed: {}", e)));
                                    }
                                }
                                self.mode = AppMode::ResultPopup;
                            } else {
                                self.status_message = Some("Invalid amount".to_string());
                            }
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
                // Clear unlock/lock success messages on any keypress
                self.unlock_success_message = None;
                self.lock_success_message = None;

                // Special handling when actively in Transfer form (selected_action == 4 AND in_transfer_form)
                if self.selected_action == 4 && self.in_transfer_form {
                    match code {
                        KeyCode::Tab | KeyCode::Down => {
                            // Switch between fields (forward)
                            self.transfer_focused_field = match self.transfer_focused_field {
                                TransferInputField::TokenType => TransferInputField::Recipient,
                                TransferInputField::Recipient => TransferInputField::Amount,
                                TransferInputField::Amount => TransferInputField::TokenType,
                            };
                            return;
                        }
                        KeyCode::Up => {
                            // Switch between fields (reverse)
                            self.transfer_focused_field = match self.transfer_focused_field {
                                TransferInputField::TokenType => TransferInputField::Amount,
                                TransferInputField::Recipient => TransferInputField::TokenType,
                                TransferInputField::Amount => TransferInputField::Recipient,
                            };
                            return;
                        }
                        KeyCode::Left | KeyCode::Right => {
                            // Toggle token type when on that field
                            if self.transfer_focused_field == TransferInputField::TokenType {
                                self.transfer_token_type = match self.transfer_token_type {
                                    TransferTokenType::StandardQcoin => TransferTokenType::Pqcoin,
                                    TransferTokenType::Pqcoin => TransferTokenType::StandardQcoin,
                                };
                            }
                            return;
                        }
                        KeyCode::Char(c) => {
                            match self.transfer_focused_field {
                                TransferInputField::TokenType => {
                                    // No character input for token type field (use arrow keys)
                                }
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
                            return;
                        }
                        KeyCode::Backspace => {
                            match self.transfer_focused_field {
                                TransferInputField::TokenType => {
                                    // No backspace for token type field
                                }
                                TransferInputField::Recipient => {
                                    self.transfer_recipient.pop();
                                }
                                TransferInputField::Amount => {
                                    self.transfer_amount.pop();
                                }
                            }
                            return;
                        }
                        KeyCode::Enter => {
                            // Validate and execute transfer
                            if self.validate_transfer_inputs() {
                                self.perform_transfer_action();
                            }
                            return;
                        }
                        KeyCode::Esc => {
                            // Exit transfer form, return to viewing mode
                            self.in_transfer_form = false;
                            self.transfer_recipient.clear();
                            self.transfer_amount.clear();
                            self.transfer_focused_field = TransferInputField::TokenType;
                            self.transfer_token_type = TransferTokenType::StandardQcoin;
                            self.status_message = Some("Transfer cancelled".to_string());
                            return;
                        }
                        _ => {}
                    }
                }

                // Special handling when actively in Vaults list (selected_action == 11 AND in_vault_list)
                if self.selected_action == 11 && self.in_vault_list && !self.vault_list.is_empty() {
                    match self.vault_management_mode {
                        VaultManagementMode::List => {
                            match code {
                                KeyCode::Esc => {
                                    // Exit vault list, return to viewing mode
                                    self.in_vault_list = false;
                                    self.status_message = Some("Vault selection cancelled".to_string());
                                    return;
                                }
                                KeyCode::Up => {
                                    if self.selected_vault_index > 0 {
                                        self.selected_vault_index -= 1;
                                    }
                                    return;
                                }
                                KeyCode::Down => {
                                    let max_index = self.vault_list.len().saturating_sub(1);
                                    if self.selected_vault_index < max_index {
                                        self.selected_vault_index += 1;
                                    }
                                    return;
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') => {
                                    self.vault_management_mode = VaultManagementMode::Create;
                                    self.new_vault_name.clear();
                                    self.status_message = Some("Enter vault name...".to_string());
                                    return;
                                }
                                KeyCode::Char('d') | KeyCode::Char('D') => {
                                    if self.selected_vault_index < self.vault_list.len() {
                                        let selected_vault = &self.vault_list[self.selected_vault_index];
                                        self.vault_to_delete = selected_vault.name.clone();
                                        self.delete_confirmation_input.clear();
                                        self.mode = AppMode::DeleteConfirmPopup;
                                        self.status_message = Some(format!("Type '{}' to confirm deletion", selected_vault.name));
                                    }
                                    return;
                                }
                                KeyCode::Enter => {
                                    if self.selected_vault_index < self.vault_list.len() {
                                        let selected_vault = &self.vault_list[self.selected_vault_index];
                                        self.perform_vault_switch(&selected_vault.name.clone());
                                    }
                                    return;
                                }
                                _ => {}
                            }
                        }
                        VaultManagementMode::Create => {
                            match code {
                                KeyCode::Esc => {
                                    self.vault_management_mode = VaultManagementMode::List;
                                    self.new_vault_name.clear();
                                    self.status_message = Some("Select vault or create new...".to_string());
                                    return;
                                }
                                KeyCode::Char(c) => {
                                    if c.is_alphanumeric() || c == '-' || c == '_' {
                                        self.new_vault_name.push(c);
                                    }
                                    return;
                                }
                                KeyCode::Backspace => {
                                    self.new_vault_name.pop();
                                    return;
                                }
                                KeyCode::Enter => {
                                    if !self.new_vault_name.is_empty() {
                                        self.perform_new_vault_action();
                                    }
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                }

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
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        // Navigate to Portfolio (index 0)
                        self.selected_action = 0;
                    }
                    KeyCode::Char('g') | KeyCode::Char('G') => {
                        // Navigate to Register (index 1)
                        self.selected_action = 1;
                    }
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        // Navigate to Transfer (index 4)
                        self.selected_action = 4;
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
                    KeyCode::Char('x') | KeyCode::Char('X') => {
                        // Navigate to Close (index 9)
                        self.selected_action = 9;
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        // Navigate to Chart (index 10)
                        self.selected_action = 10;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.copy_wallet_to_clipboard();
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        // Navigate to Vaults (index 11) and load vault list
                        self.selected_action = 11;

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
                    }
                    KeyCode::Char('w') | KeyCode::Char('W') => {
                        // Navigate to Wrap (index 5)
                        self.selected_action = 5;
                    }
                    KeyCode::Char('e') | KeyCode::Char('E') => {
                        // Navigate to Unwrap (index 6)
                        self.selected_action = 6;
                    }
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                        if self.selected_action > 0 {
                            self.selected_action -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                        if self.selected_action < 11 {  // 12 total actions (0-11)
                            self.selected_action += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Enter key is handled by the special action input handlers
                        // (Transfer at index 4, Vaults at index 11)
                        // For other actions, Enter executes them
                        match self.selected_action {
                            0 => {
                                // Portfolio view - just stay on this view (content renders automatically)
                            }
                            1 => self.execute_register(),
                            2 => self.execute_lock(),
                            3 => self.execute_unlock(),
                            4 => {
                                // Transfer - Enter activates the form for editing
                                self.in_transfer_form = true;
                                self.status_message = Some("Enter transfer details (Tab to navigate, Esc to exit)".to_string());
                            }
                            5 => self.execute_wrap(),
                            6 => self.execute_unwrap(),
                            7 => self.execute_claim_airdrop(),
                            8 => {
                                // View airdrop pool stats
                                if let Ok((distributed, remaining)) = tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(async {
                                        self.vault_client.get_airdrop_stats().await
                                    })
                                }) {
                                    self.airdrop_distributed = distributed;
                                    self.airdrop_remaining = remaining;

                                    let distributed_qdum = distributed as f64 / 1_000_000.0;
                                    let remaining_qdum = remaining as f64 / 1_000_000.0;
                                    if let Ok(mut history) = AirdropHistory::load() {
                                        history.add_entry(distributed_qdum, remaining_qdum);
                                        let _ = history.save();
                                    }

                                    self.mode = AppMode::AirdropStatsPopup;
                                    self.needs_clear = true;
                                }
                            }
                            9 => self.execute_close(),
                            10 => self.execute_chart(),
                            11 => {
                                // Vaults - Load vault list and activate for interaction
                                // Load vault list if not already loaded
                                if self.vault_list.is_empty() {
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
                                    }

                                    // Start in list mode
                                    self.vault_management_mode = VaultManagementMode::List;
                                }

                                self.in_vault_list = true;
                                self.status_message = Some("Navigate vaults (↑↓), Enter to switch, Esc to exit".to_string());
                            }
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

        let pq_balance_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.get_balance(wallet, self.pq_mint).await
            })
        });

        let standard_balance_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.get_balance(wallet, self.standard_mint).await
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
                self.pq_balance = pq_balance_result.ok();
                self.standard_balance = standard_balance_result.ok();
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























    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Render white background for modern clean look
        let bg_block = Block::default()
            .style(Style::default().bg(Theme::BASE));  // White background
        f.render_widget(bg_block, size);

        // Early return for result popup to avoid flash - only render popup on dark background
        if self.mode == AppMode::ResultPopup {
            self.render_transfer_result_popup(f, size);
            return;
        }

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

        // Professional header with clean gray borders
        let width = chunks[0].width as usize;
        let border_line = "━".repeat(width.saturating_sub(2));
        let content_width = width.saturating_sub(2);

        // Static gray border color
        let border_color = Color::Rgb(140, 140, 140);

        // Modern title text - Bloomberg style
        let main_title = "PQCOIN TERMINAL █";
        let subtitle = "POST-QUANTUM SECURE  │  SPHINCS+ SHA2-128s  │  NIST FIPS 205  │  SOLANA DEVNET";

        let header = vec![
            Line::from(Span::styled(
                format!("┏{}┓", border_line),
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )),
            // Main title with Bloomberg orange accent
            Line::from(vec![
                Span::styled("┃", Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:^width$}", main_title, width = content_width),
                    Style::default()
                        .fg(Theme::BLOOMBERG_ORANGE)
                        .bg(Theme::BASE)
                        .add_modifier(Modifier::BOLD)
                ),
                Span::styled("┃", Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
            ]),
            // Subtitle with white text
            Line::from(vec![
                Span::styled("┃", Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:^width$}", subtitle, width = content_width),
                    Style::default()
                        .fg(Theme::TEXT)
                        .bg(Theme::BASE)
                        .add_modifier(Modifier::BOLD)
                ),
                Span::styled("┃", Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::styled(
                format!("┗{}┛", border_line),
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        let header_paragraph = Paragraph::new(header)
            .alignment(Alignment::Left)
            .style(Style::default().bg(Theme::BASE));
        f.render_widget(header_paragraph, chunks[0]);

        // Get active vault name for account info
        let vault_name = if let Ok(config) = VaultConfig::load() {
            config.active_vault.unwrap_or_else(|| "No Vault".to_string())
        } else {
            "Unknown".to_string()
        };

        // Account info with clean table layout
        let mut account_rows = vec![
            // Wallet address row
            Row::new(vec![
                Line::from(Span::styled("WALLET", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
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
                    Line::from(Span::styled("PQ ACCOUNT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(pda.to_string(), Style::default().fg(Theme::PURPLE).add_modifier(Modifier::BOLD))),
                ]));

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("STATE", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(state_text, Style::default().fg(state_color).add_modifier(Modifier::BOLD))),
                ]));

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("VAULT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(vault_name.to_uppercase(), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD))),
                ]));
            } else {
                // PDA not available - vault not registered
                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                    Line::from(Span::styled("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Theme::DIM))),
                ]));

                account_rows.push(Row::new(vec![
                    Line::from(Span::styled("PQ ACCOUNT", Style::default().fg(Theme::CYAN_BRIGHT).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled("NOT REGISTERED - Use [G]", Style::default().fg(Theme::ORANGE_NEON).add_modifier(Modifier::BOLD))),
                ]));
            }
        }

        let account_widths = [Constraint::Length(20), Constraint::Min(40)];

        // Static gray border color matching splash screen
        let border_color = Color::Rgb(140, 140, 140);
        let account_table = Table::new(account_rows, account_widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Double)
                    .title(format!(" {} ACCOUNT INFO ", Icons::INFO))
                    .title_style(Style::default().fg(Theme::BLOOMBERG_ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().bg(Theme::BASE))
            .column_spacing(2);

        f.render_widget(account_table, chunks[1]);

        // Main content area - sidebar + content layout
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
            .split(chunks[2]);

        // Left sidebar - only actions panel (portfolio moved to content view)
        // Render actions panel taking full sidebar height
        self.render_actions_panel(f, main_chunks[0]);

        // Right content area - shows different views based on selected action
        self.render_content_area(f, main_chunks[1]);

        // Footer with controls and status message
        self.render_footer(f, chunks[3]);

        // Render help overlay if in help mode
        if self.mode == AppMode::Help {
            self.render_help_overlay(f, size);
        }

        // Render popups on top of dashboard (NO early returns, NO full screen clears)
        match self.mode {
            AppMode::RegisterPopup => self.render_action_popup(f, size, "REGISTER", Color::Green),
            AppMode::TransferPopup => self.render_transfer_popup(f, size),
            AppMode::WrapPopup => self.render_wrap_popup(f, size),
            AppMode::UnwrapPopup => self.render_unwrap_popup(f, size),
            AppMode::AirdropClaimPopup => self.render_action_popup(f, size, "CLAIM AIRDROP", Theme::CYAN_NEON),
            AppMode::AirdropStatsPopup => self.render_airdrop_stats_popup(f, size),
            AppMode::VaultSwitchPopup => self.render_vault_switch_popup(f, size),
            AppMode::DeleteConfirmPopup => self.render_delete_confirm_popup(f, size),
            AppMode::CloseConfirmPopup => self.render_close_confirm_popup(f, size),
            AppMode::ChartPopup => self.render_chart_popup(f, size),
            _ => {}
        }
    }


















}

