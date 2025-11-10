use anyhow::Result;
use crate::dashboard::types::{Dashboard, AppMode, LockHistory};

impl Dashboard {
    pub fn record_lock_history(&mut self, force_refresh: bool) -> Result<(f64, usize)> {
        // Query network-wide locked tokens
        let mint = self.mint;
        let vault_client = &self.vault_client;

        self.status_message = Some("🔍 Querying network for locked tokens...".to_string());

        // Get total locked qcoin across all holders
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                vault_client.get_network_locked_total(mint, force_refresh).await
            })
        });

        match result {
            Ok((total_locked, holder_count)) => {

                // Load history, add entry, and save
                if let Ok(mut history) = LockHistory::load() {
                    history.add_entry(total_locked, holder_count);
                    if let Err(e) = history.save() {
                        self.status_message = Some(format!("⚠️  Failed to save history: {}", e));
                        return Err(e);
                    }
                }

                self.status_message = Some(format!("✅ Recorded: {:.2} qcoin locked ({} holders)", total_locked, holder_count));
                Ok((total_locked, holder_count))
            }
            Err(e) => {
                self.status_message = Some(format!("❌ Failed to query network: {}", e));
                Err(e)
            }
        }
    }

    pub fn execute_chart(&mut self) {
        // Record current lock status before showing chart (use cache if available)
        let _ = self.record_lock_history(false);

        // Show chart popup
        self.mode = AppMode::ChartPopup;
        self.needs_clear = true;
    }
}
