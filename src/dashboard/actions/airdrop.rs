use crate::dashboard::types::{Dashboard, AppMode, ActionStep};
use crate::dashboard::utils::suppress_output;

impl Dashboard {
    pub fn execute_claim_airdrop(&mut self) {
        // Keep mode as Normal - render in content area instead of popup
        self.action_steps.clear();
        self.action_steps.push(ActionStep::Starting);
        self.status_message = Some("Claiming Airdrop...".to_string());
        // Execute immediately
        self.perform_claim_airdrop_action();
    }

    pub fn perform_claim_airdrop_action(&mut self) {
        if !self.action_steps.is_empty() && !matches!(self.action_steps.last(), Some(ActionStep::Starting)) {
            return; // Already executed
        }

        self.action_steps.clear();
        self.action_steps.push(ActionStep::InProgress("Checking PQ account...".to_string()));

        // Execute the airdrop claim (with output suppressed)
        let keypair_path = self.keypair_path.to_str().unwrap();
        let wallet = self.wallet;
        let mint = self.pq_mint;  // Airdrop uses pqcoin (Token-2022), not standard qcoin!
        let vault_client = &self.vault_client;
        let keypair_path_str = keypair_path.to_string();

        // Debug: Log the wallet address and all details
        let _ = std::fs::write("/tmp/airdrop-debug.log",
            format!("Claiming airdrop for wallet: {}\nKeypair path: {}\nMint: {} (pqcoin/Token-2022)\n",
                wallet, keypair_path, mint));

        let result = suppress_output(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    vault_client.claim_airdrop(wallet, &keypair_path_str, mint).await
                })
            })
        });

        match result {
            Ok(_) => {
                self.action_steps.push(ActionStep::Success("✅ Claimed 100 qcoin successfully!".to_string()));
                self.action_steps.push(ActionStep::InProgress("⏰ Next claim available in 24 hours".to_string()));
                self.action_steps.push(ActionStep::InProgress("".to_string()));
                self.action_steps.push(ActionStep::InProgress("Press [P] to view airdrop pool stats...".to_string()));
                self.status_message = Some("✅ Airdrop claimed!".to_string());
                self.refresh_data();
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("CooldownNotElapsed") || error_msg.contains("Cooldown") {
                    self.action_steps.push(ActionStep::Error("❌ Cooldown period not elapsed - wait 24 hours between claims".to_string()));
                } else if error_msg.contains("AirdropCapExceeded") || error_msg.contains("Cap exceeded") {
                    self.action_steps.push(ActionStep::Error("❌ Airdrop pool exhausted - 3% supply cap reached".to_string()));
                } else if error_msg.contains("PQAccountNotInitialized") || error_msg.contains("not initialized") {
                    self.action_steps.push(ActionStep::Error("❌ PQ account not initialized - register this vault first (press G)".to_string()));
                } else if error_msg.contains("owner does not match") || error_msg.contains("OwnerMismatch") {
                    self.action_steps.push(ActionStep::Error("❌ This vault has not been registered yet!".to_string()));
                    self.action_steps.push(ActionStep::InProgress("".to_string()));
                    self.action_steps.push(ActionStep::InProgress("Each vault needs its own PQ account. Press [G] to register this vault.".to_string()));
                } else {
                    self.action_steps.push(ActionStep::Error(format!("❌ Airdrop claim failed: {}", e)));
                }
                self.status_message = Some("❌ Airdrop claim failed!".to_string());
            }
        }
    }
}
