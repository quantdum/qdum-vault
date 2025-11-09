use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::dashboard::types::{Dashboard, AppMode, ActionStep};
use crate::solana::client::VaultClient;

impl Dashboard {
    pub fn execute_lock(&mut self) {
        self.mode = AppMode::LockPopup;
        self.action_steps.clear();
        self.needs_clear = true;  // Force terminal clear to prevent background artifacts
        self.status_message = Some("Executing Lock...".to_string());
        self.pending_action = true;  // Set flag to execute on next loop
    }

    pub fn perform_lock_action(&mut self) {
        // Flag to indicate lock is complete
        let lock_complete = Arc::new(AtomicBool::new(false));
        let lock_complete_clone = Arc::clone(&lock_complete);
        self.lock_complete = Some(Arc::clone(&lock_complete));

        // Spawn lock operation in background thread
        let keypair_path_str = self.keypair_path.to_str().unwrap().to_string();
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

                // Call lock_vault
                let _result = vault_client.lock_vault(wallet, &keypair_path_str).await;

                // Restore stdout/stderr before task ends
                unsafe {
                    libc::dup2(original_stdout, 1);
                    libc::dup2(original_stderr, 2);
                    libc::close(original_stdout);
                    libc::close(original_stderr);
                }

                // Mark as complete
                lock_complete_clone.store(true, Ordering::SeqCst);
            }); // End rt.block_on
        }); // End std::thread::spawn
    }
}
