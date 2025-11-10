use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;
use crate::dashboard::types::{Dashboard, AppMode, ActionStep, TransferInputField, TransferTokenType};
use crate::dashboard::utils::suppress_output;

impl Dashboard {
    pub fn execute_transfer(&mut self) {
        // Stay in Normal mode - render transfer form in content area
        self.selected_action = 4;  // Set to Transfer action index
        self.transfer_recipient.clear();
        self.transfer_amount.clear();
        self.transfer_focused_field = TransferInputField::TokenType;
        self.transfer_token_type = TransferTokenType::StandardQcoin;
        self.status_message = Some("Select token type and enter transfer details...".to_string());
    }

    pub fn validate_transfer_inputs(&mut self) -> bool {
        // Check recipient
        if self.transfer_recipient.is_empty() {
            self.status_message = Some("Recipient address required".to_string());
            return false;
        }

        if Pubkey::from_str(&self.transfer_recipient).is_err() {
            self.status_message = Some("Invalid recipient address".to_string());
            return false;
        }

        // Check amount
        if self.transfer_amount.is_empty() {
            self.status_message = Some("Amount required".to_string());
            return false;
        }

        if self.transfer_amount.parse::<f64>().is_err() {
            self.status_message = Some("Invalid amount (must be a number)".to_string());
            return false;
        }

        let amount: f64 = self.transfer_amount.parse().unwrap();
        if amount <= 0.0 {
            self.status_message = Some("Amount must be greater than 0".to_string());
            return false;
        }

        true
    }

    pub fn perform_transfer_action(&mut self) {
        // Check which token type is selected
        let (mint, balance, token_name, requires_unlock) = match self.transfer_token_type {
            TransferTokenType::StandardQcoin => {
                (self.standard_mint, self.standard_balance, "qcoin", false)
            }
            TransferTokenType::Pqcoin => {
                (self.pq_mint, self.pq_balance, "pqcoin", true)
            }
        };

        // If pqcoin transfer, check vault is unlocked
        if requires_unlock {
            if let Some(ref status) = self.vault_status {
                if status.is_locked {
                    self.mode = AppMode::Normal;
                    self.action_steps.clear();
                    self.action_steps.push(ActionStep::Error("❌ Vault is locked!".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("You must unlock your vault to transfer pqcoin.".to_string()));
                    self.action_steps.push(ActionStep::InProgress("Press U to unlock your vault first.".to_string()));
                    self.status_message = Some("❌ Unlock vault to transfer pqcoin".to_string());
                    return;
                }
            } else {
                self.status_message = Some("❌ Vault status unknown".to_string());
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

        // Parse amount (in qcoin/pqcoin, convert to base units)
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

        // Check if token account exists
        let wallet = self.wallet;
        let vault_client = &self.vault_client;
        let account_exists = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.token_account_exists(wallet, mint).await
                })
            })
        });

        match account_exists {
            Ok(false) => {
                // Token account doesn't exist yet
                self.mode = AppMode::Normal;
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error(format!("❌ No {} token account found!", token_name)));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::InProgress("💡 Your token account needs to be created first.".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));

                match self.transfer_token_type {
                    TransferTokenType::StandardQcoin => {
                        self.action_steps.push(ActionStep::InProgress("  To create your Standard qcoin account:".to_string()));
                        self.action_steps.push(ActionStep::InProgress("".to_string()));
                        self.action_steps.push(ActionStep::InProgress("  • Press [A] to claim AIRDROP (100 qcoin)".to_string()));
                        self.action_steps.push(ActionStep::InProgress("    This will create your account automatically".to_string()));
                        self.action_steps.push(ActionStep::InProgress("".to_string()));
                        self.action_steps.push(ActionStep::InProgress("  OR".to_string()));
                        self.action_steps.push(ActionStep::InProgress("".to_string()));
                        self.action_steps.push(ActionStep::InProgress("  • Press [Shift+W] to UNWRAP pqcoin".to_string()));
                        self.action_steps.push(ActionStep::InProgress("    This converts pqcoin → Standard qcoin".to_string()));
                    }
                    TransferTokenType::Pqcoin => {
                        self.action_steps.push(ActionStep::InProgress("  To create your pqcoin account:".to_string()));
                        self.action_steps.push(ActionStep::InProgress("".to_string()));
                        self.action_steps.push(ActionStep::InProgress("  1. Get Standard qcoin first (Press [A] for airdrop)".to_string()));
                        self.action_steps.push(ActionStep::InProgress("  2. Press [W] to WRAP Standard qcoin → pqcoin".to_string()));
                        self.action_steps.push(ActionStep::InProgress("     This creates your pqcoin account".to_string()));
                    }
                }

                self.status_message = Some(format!("❌ {} account doesn't exist yet", token_name));
                self.mode = AppMode::ResultPopup;
                return;
            }
            Err(e) => {
                self.mode = AppMode::Normal;
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error(format!("❌ Failed to check account: {}", e)));
                self.status_message = Some("❌ Error checking account".to_string());
                self.mode = AppMode::ResultPopup;
                return;
            }
            Ok(true) => {
                // Account exists, continue with balance check
            }
        }

        // Check if user has sufficient balance
        if let Some(bal) = balance {
            if bal < amount_base_units {
                let balance_qdum = bal as f64 / 1_000_000.0;
                self.mode = AppMode::Normal;
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error(format!("❌ Insufficient {} balance!", token_name)));
                self.action_steps.push(ActionStep::Error(format!("Your balance: {:.6} {}", balance_qdum, token_name)));
                self.action_steps.push(ActionStep::Error(format!("Transfer amount: {:.6} {}", amount_qdum, token_name)));
                self.status_message = Some("❌ Transfer failed: Insufficient balance".to_string());
                self.transfer_recipient.clear();
                self.transfer_amount.clear();
                return;
            }
        } else {
            self.mode = AppMode::Normal;
            self.action_steps.clear();
            self.action_steps.push(ActionStep::Error(format!("❌ No {} balance available!", token_name)));
            self.status_message = Some(format!("❌ No {} to transfer", token_name));
            self.transfer_recipient.clear();
            self.transfer_amount.clear();
            return;
        }

        // Show progress
        self.mode = AppMode::Normal;
        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress(format!("Transferring {:.6} {}...", amount_qdum, token_name)));

        // Load keypair
        let keypair_path = self.keypair_path.to_str().unwrap();
        let keypair_path_str = keypair_path.to_string();

        let keypair = match solana_sdk::signature::read_keypair_file(&keypair_path_str) {
            Ok(kp) => kp,
            Err(e) => {
                self.action_steps.clear();
                self.action_steps.push(ActionStep::Error(format!("❌ Failed to load keypair: {}", e)));
                self.status_message = Some("❌ Transfer failed!".to_string());
                return;
            }
        };

        // Execute the transfer
        let vault_client = &self.vault_client;
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

        // Show result
        self.action_steps.clear();
        self.mode = AppMode::ResultPopup;

        match result {
            Ok(_) => {
                // Store recipient for display (truncate if too long)
                let recipient_display = if self.transfer_recipient.len() > 20 {
                    format!("{}...{}", &self.transfer_recipient[..8], &self.transfer_recipient[self.transfer_recipient.len()-8..])
                } else {
                    self.transfer_recipient.clone()
                };

                self.action_steps.push(ActionStep::Success(format!("✅ Transferred {:.6} {} to {}", amount_qdum, token_name, recipient_display)));
                self.status_message = Some("✅ Transfer completed successfully!".to_string());
                self.transfer_recipient.clear();
                self.transfer_amount.clear();

                // Wait for RPC to update its cache, then refresh balance
                std::thread::sleep(std::time::Duration::from_secs(1));
                self.refresh_data();
            }
            Err(e) => {
                self.action_steps.push(ActionStep::Error(format!("❌ Transfer failed: {}", e)));
                self.status_message = Some("❌ Transfer failed!".to_string());
            }
        }
    }
}
