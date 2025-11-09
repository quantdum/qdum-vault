use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;
use crate::dashboard::types::{Dashboard, AppMode, ActionStep, TransferInputField, TransferTokenType};
use crate::dashboard::utils::suppress_output;

impl Dashboard {
    pub fn execute_transfer(&mut self) {
        self.mode = AppMode::TransferPopup;
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.transfer_recipient.clear();
        self.transfer_amount.clear();
        self.transfer_focused_field = TransferInputField::TokenType;
        self.transfer_token_type = TransferTokenType::StandardQDUM;
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
            TransferTokenType::StandardQDUM => {
                (self.standard_mint, self.standard_balance, "qcoin", false)
            }
            TransferTokenType::PqQDUM => {
                (self.pq_mint, self.pq_balance, "pqcoin", true)
            }
        };

        // If pqQDUM transfer, check vault is unlocked
        if requires_unlock {
            if let Some(ref status) = self.vault_status {
                if status.is_locked {
                    self.mode = AppMode::Normal;
                    self.action_steps.clear();
                    self.action_steps.push(ActionStep::Error("❌ Vault is locked!".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("You must unlock your vault to transfer pqQDUM.".to_string()));
                    self.action_steps.push(ActionStep::InProgress("Press U to unlock your vault first.".to_string()));
                    self.status_message = Some("❌ Unlock vault to transfer pqQDUM".to_string());
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
                    TransferTokenType::StandardQDUM => {
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
                    TransferTokenType::PqQDUM => {
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

        // Close the popup and show progress
        self.mode = AppMode::Normal;
        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress(format!("Preparing {} transfer...", token_name)));

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

        self.action_steps.push(ActionStep::InProgress(format!("Sending {} {} to {}", amount_qdum, token_name, recipient_display)));
        self.action_steps.push(ActionStep::InProgress("Broadcasting transaction to Solana...".to_string()));

        // Execute the transfer
        let vault_client = &self.vault_client;

        // Add debug logging
        self.action_steps.push(ActionStep::InProgress(format!("Token type: {}", token_name)));
        self.action_steps.push(ActionStep::InProgress(format!("Mint: {}", mint)));

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

        // Debug: Check what we got
        let result_type = if result.is_ok() { "Success" } else { "Error" };
        eprintln!("Transfer result: {}", result_type);

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Success("║      ✓ TRANSFER SUCCESSFUL!             ║".to_string()));
                self.action_steps.push(ActionStep::Success("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success(format!("Amount:     {:.6} {}", amount_qdum, token_name)));
                self.action_steps.push(ActionStep::Success(format!("Recipient:  {}", recipient_display)));
                self.action_steps.push(ActionStep::Success("".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Transaction confirmed on Solana".to_string()));
                self.action_steps.push(ActionStep::Success("✓ Tokens have been transferred".to_string()));

                // Show updated balance
                if let Some(old_balance) = balance {
                    let new_balance = old_balance.saturating_sub(amount_base_units);
                    let new_balance_qdum = new_balance as f64 / 1_000_000.0;
                    self.action_steps.push(ActionStep::Success("".to_string()));
                    self.action_steps.push(ActionStep::InProgress(format!("New {} balance: {:.6} {}", token_name, new_balance_qdum, token_name)));
                }

                self.status_message = Some("✓ Transfer completed successfully!".to_string());
                self.transfer_recipient.clear();
                self.transfer_amount.clear();

                // Wait for RPC to update its cache, then refresh balance
                std::thread::sleep(std::time::Duration::from_secs(1));
                self.refresh_data();
            }
            Err(e) => {
                let error_str = e.to_string();

                self.action_steps.push(ActionStep::Error("╔══════════════════════════════════════════╗".to_string()));
                self.action_steps.push(ActionStep::Error("║      ✗ TRANSFER FAILED                   ║".to_string()));
                self.action_steps.push(ActionStep::Error("╚══════════════════════════════════════════╝".to_string()));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::Error(format!("Token Type: {}", token_name)));
                self.action_steps.push(ActionStep::Error(format!("Amount:     {:.6} {}", amount_qdum, token_name)));
                self.action_steps.push(ActionStep::Error(format!("Recipient:  {}", recipient_display)));
                self.action_steps.push(ActionStep::Error("".to_string()));
                self.action_steps.push(ActionStep::Error(format!("Error: {}", error_str)));
                self.action_steps.push(ActionStep::Error("".to_string()));

                // Provide specific help based on error type
                if error_str.contains("InvalidAccountData") {
                    self.action_steps.push(ActionStep::InProgress("💡 Issue: Invalid account data for transfer hook".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  This is a known issue with pqQDUM transfer hooks.".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  The transfer hook validation failed.".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  Possible solutions:".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  1. Ensure vault is unlocked (Press U)".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  2. Try unwrapping pqcoin to Standard qcoin first".to_string()));
                    self.action_steps.push(ActionStep::InProgress("     (Press Shift+W to unwrap)".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  3. Transfer Standard qcoin instead".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  Note: Standard qcoin transfers work normally".to_string()));
                } else if error_str.contains("Vault is locked") {
                    self.action_steps.push(ActionStep::InProgress("💡 Issue: Vault is currently locked".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  pqcoin transfers require the vault to be unlocked.".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  To unlock your vault:".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  • Press [U] to start the unlock process".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  • Sign the challenge with your SPHINCS+ key".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  • Once unlocked, you can transfer pqcoin".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  Note: Standard qcoin can be transferred without unlocking".to_string()));
                } else if error_str.contains("Sender token account not found") || error_str.contains("could not find account") {
                    self.action_steps.push(ActionStep::InProgress("💡 Solution:".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));

                    match self.transfer_token_type {
                        TransferTokenType::StandardQDUM => {
                            self.action_steps.push(ActionStep::InProgress("  You don't have a Standard qcoin token account yet.".to_string()));
                            self.action_steps.push(ActionStep::InProgress("".to_string()));
                            self.action_steps.push(ActionStep::InProgress("  To get Standard qcoin:".to_string()));
                            self.action_steps.push(ActionStep::InProgress("  1. Use AIRDROP (Press A) to claim 100 qcoin".to_string()));
                            self.action_steps.push(ActionStep::InProgress("     OR".to_string()));
                            self.action_steps.push(ActionStep::InProgress("  2. Use UNWRAP (Press Shift+W) to convert pqcoin".to_string()));
                            self.action_steps.push(ActionStep::InProgress("     to Standard qcoin".to_string()));
                        }
                        TransferTokenType::PqQDUM => {
                            self.action_steps.push(ActionStep::InProgress("  You don't have any pqcoin tokens yet.".to_string()));
                            self.action_steps.push(ActionStep::InProgress("".to_string()));
                            self.action_steps.push(ActionStep::InProgress("  To get pqcoin:".to_string()));
                            self.action_steps.push(ActionStep::InProgress("  1. Get Standard qcoin (Press A for airdrop)".to_string()));
                            self.action_steps.push(ActionStep::InProgress("  2. Use WRAP (Press W) to convert Standard qcoin".to_string()));
                            self.action_steps.push(ActionStep::InProgress("     to pqcoin".to_string()));
                        }
                    }
                } else if error_str.contains("insufficient funds") || error_str.contains("Insufficient") {
                    self.action_steps.push(ActionStep::InProgress("💡 Issue: Insufficient SOL for transaction fees".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  You need SOL to pay for transaction fees on Solana.".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  Get devnet SOL: solana airdrop 1".to_string()));
                } else {
                    self.action_steps.push(ActionStep::InProgress("Common issues:".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  • Insufficient SOL for transaction fee".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  • Network connectivity issues".to_string()));
                    self.action_steps.push(ActionStep::InProgress("  • Invalid recipient address".to_string()));
                }

                self.status_message = Some("❌ Transfer failed!".to_string());
            }
        }

        // Debug: Ensure we have action steps
        if self.action_steps.is_empty() {
            eprintln!("WARNING: No action steps after transfer!");
            self.action_steps.push(ActionStep::Error("❌ Transfer failed with unknown error".to_string()));
            self.action_steps.push(ActionStep::InProgress("Please check the terminal for details".to_string()));
        }
    }
}
