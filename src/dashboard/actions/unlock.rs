use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::dashboard::types::{Dashboard, AppMode};
use crate::solana::client::VaultClient;
use crate::crypto::sphincs::SphincsKeyManager;

impl Dashboard {
    pub fn execute_unlock(&mut self) {
        // Stay in Normal mode - will render splash animation in content area
        self.action_steps.clear();
        self.status_message = Some("Unlocking...".to_string());
        // Execute immediately
        self.perform_unlock_action();
    }

    pub fn perform_unlock_action(&mut self) {
        // Flag to indicate unlock is complete
        let unlock_complete = Arc::new(AtomicBool::new(false));
        let unlock_complete_clone = Arc::clone(&unlock_complete);
        self.unlock_complete = Some(Arc::clone(&unlock_complete));

        // Spawn unlock operation in background thread
        let keypair_path_str = self.keypair_path.to_str().unwrap().to_string();
        let sphincs_public_key_path = self.sphincs_public_key_path.clone();
        let sphincs_private_key_path = self.sphincs_private_key_path.clone();
        let wallet = self.wallet;
        let rpc_url = self.rpc_url.clone();
        let program_id = self.program_id;

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

                // Load SPHINCS+ keys
                let key_manager = match SphincsKeyManager::new(None) {
                    Ok(km) => km,
                    Err(_) => {
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
                    Err(_) => {
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
                    Err(_) => {
                        unsafe {
                            libc::dup2(original_stdout, 1);
                            libc::dup2(original_stderr, 2);
                            libc::close(original_stdout);
                            libc::close(original_stderr);
                        }
                        return;
                    }
                };

                // Create VaultClient
                let vault_client = match VaultClient::new(&rpc_url, program_id) {
                    Ok(client) => client,
                    Err(_) => {
                        unsafe {
                            libc::dup2(original_stdout, 1);
                            libc::dup2(original_stderr, 2);
                            libc::close(original_stdout);
                            libc::close(original_stderr);
                        }
                        return;
                    }
                };

                // Call unlock_vault
                let _result = vault_client.unlock_vault(
                    wallet,
                    &keypair_path_str,
                    &sphincs_privkey,
                    &sphincs_pubkey,
                    None,
                ).await;

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
    }
}
