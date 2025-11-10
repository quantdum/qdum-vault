use std::path::PathBuf;
use std::fs;
use solana_sdk::signature::{read_keypair_file, Signer, Keypair};
use std::io::Write;
use crate::dashboard::types::{Dashboard, AppMode, ActionStep, VaultManagementMode};
use crate::vault_manager::VaultConfig;
use crate::crypto::sphincs::SphincsKeyManager;

impl Dashboard {
    pub fn execute_new_vault(&mut self) {
        // Stay in Normal mode - render vault list in content area
        self.action_steps.clear();
        self.new_vault_name.clear();
        self.selected_action = 11;  // Set to Vaults action index

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

    pub fn execute_close(&mut self) {
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

    pub fn perform_vault_switch(&mut self, vault_name: &str) {
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

                                    // IMPORTANT: Clear all cached vault data to force refresh
                                    self.vault_status = None;
                                    self.balance = None;
                                    self.pq_balance = None;
                                    self.standard_balance = None;

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

    pub fn perform_vault_delete(&mut self, vault_name: &str) {
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

    pub fn perform_close(&mut self) {
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

    pub fn perform_new_vault_action(&mut self) {
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
}
