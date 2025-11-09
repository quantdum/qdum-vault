use crate::crypto::sphincs::SphincsKeyManager;
use crate::dashboard::types::{Dashboard, ActionStep, AppMode};
use crate::dashboard::utils::suppress_output;

impl Dashboard {
    pub fn execute_register(&mut self) {
        self.mode = AppMode::RegisterPopup;
        self.action_steps.clear();
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Executing Register...".to_string());
        // Execute immediately
        self.perform_register_action();
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
}
