use solana_sdk::pubkey::Pubkey;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::solana::client::VaultClient;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectedAction {
    Register,
    Lock,
    Unlock,
    Transfer,
    Wrap,
    Unwrap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Normal,
    Help,
    RegisterPopup,
    LockPopup,
    UnlockPopup,
    TransferPopup,
    WrapPopup,
    UnwrapPopup,
    AirdropClaimPopup,
    AirdropStatsPopup,
    VaultSwitchPopup,
    DeleteConfirmPopup,
    CloseConfirmPopup,
    ChartPopup,
    ResultPopup,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferInputField {
    TokenType,
    Recipient,
    Amount,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferTokenType {
    StandardQcoin,
    Pqcoin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VaultManagementMode {
    List,      // Showing list of vaults
    Create,    // Creating new vault
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChartType {
    LockedAmount,
    HolderCount,
}

impl ChartType {
    pub fn to_string(&self) -> &str {
        match self {
            ChartType::LockedAmount => "LOCKED qcoin",
            ChartType::HolderCount => "LOCKED HOLDERS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChartTimeframe {
    FiveMinutes,
    OneDay,
    FiveDays,
    OneWeek,
    OneMonth,
    All,
}

impl ChartTimeframe {
    pub fn to_string(&self) -> &str {
        match self {
            ChartTimeframe::FiveMinutes => "5M",
            ChartTimeframe::OneDay => "1D",
            ChartTimeframe::FiveDays => "5D",
            ChartTimeframe::OneWeek => "1W",
            ChartTimeframe::OneMonth => "1M",
            ChartTimeframe::All => "ALL",
        }
    }

    pub fn to_duration(&self) -> Option<chrono::Duration> {
        use chrono::Duration as ChronoDuration;
        match self {
            ChartTimeframe::FiveMinutes => Some(ChronoDuration::minutes(5)),
            ChartTimeframe::OneDay => Some(ChronoDuration::days(1)),
            ChartTimeframe::FiveDays => Some(ChronoDuration::days(5)),
            ChartTimeframe::OneWeek => Some(ChronoDuration::days(7)),
            ChartTimeframe::OneMonth => Some(ChronoDuration::days(30)),
            ChartTimeframe::All => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionStep {
    Starting,
    InProgress(String),
    Success(String),
    Error(String),
}

#[derive(Clone)]
pub struct VaultStatus {
    pub is_locked: bool,
    pub pda: Option<Pubkey>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockHistoryEntry {
    pub timestamp: String,      // ISO 8601 format
    pub locked_amount: f64,     // Total amount of qcoin locked network-wide
    pub holder_count: usize,    // Number of addresses with locked tokens
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AirdropHistoryEntry {
    pub timestamp: String,       // ISO 8601 format
    pub distributed: f64,        // Total pqcoin claimed from airdrop pool
    pub remaining: f64,          // Remaining pqcoin in pool (out of 3% cap)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AirdropHistory {
    pub entries: Vec<AirdropHistoryEntry>,
}

impl AirdropHistory {
    pub fn load() -> anyhow::Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let history_path = home.join(".qdum").join("airdrop_history.json");

        if history_path.exists() {
            let contents = std::fs::read_to_string(&history_path)?;
            let history: AirdropHistory = serde_json::from_str(&contents)?;
            Ok(history)
        } else {
            Ok(AirdropHistory { entries: Vec::new() })
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let vault_dir = home.join(".qdum");
        std::fs::create_dir_all(&vault_dir)?;

        let history_path = vault_dir.join("airdrop_history.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&history_path, contents)?;
        Ok(())
    }

    pub fn add_entry(&mut self, distributed: f64, remaining: f64) {
        use chrono::Utc;
        let timestamp = Utc::now().to_rfc3339();
        self.entries.push(AirdropHistoryEntry {
            timestamp,
            distributed,
            remaining,
        });
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockHistory {
    pub entries: Vec<LockHistoryEntry>,
}

impl LockHistory {
    pub fn load() -> anyhow::Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let history_path = home.join(".qdum").join("network_lock_history.json");

        if history_path.exists() {
            let contents = std::fs::read_to_string(&history_path)?;
            let history: LockHistory = serde_json::from_str(&contents)?;
            Ok(history)
        } else {
            Ok(LockHistory { entries: Vec::new() })
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let vault_dir = home.join(".qdum");
        std::fs::create_dir_all(&vault_dir)?;

        let history_path = vault_dir.join("network_lock_history.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&history_path, contents)?;
        Ok(())
    }

    pub fn add_entry(&mut self, locked_amount: f64, holder_count: usize) {
        use chrono::Utc;
        let entry = LockHistoryEntry {
            timestamp: Utc::now().to_rfc3339(),
            locked_amount,
            holder_count,
        };
        self.entries.push(entry);

        // Keep only last 30 days of entries (hourly snapshots)
        if self.entries.len() > 720 {  // 30 days * 24 hours = 720 entries
            self.entries.remove(0);
        }
    }
}

/// Dashboard state structure
pub struct Dashboard {
    pub wallet: Pubkey,
    pub keypair_path: PathBuf,
    pub sphincs_public_key_path: String,
    pub sphincs_private_key_path: String,
    pub rpc_url: String,
    pub program_id: Pubkey,
    pub mint: Pubkey,
    pub should_quit: bool,
    pub selected_action: usize,
    pub mode: AppMode,
    pub status_message: Option<String>,
    pub vault_status: Option<VaultStatus>,
    pub balance: Option<u64>,
    pub pq_balance: Option<u64>,      // pqcoin balance
    pub standard_balance: Option<u64>, // Standard qcoin balance
    pub is_loading: bool,
    pub action_steps: Vec<ActionStep>,
    pub vault_client: VaultClient,
    pub needs_clear: bool,
    pub pending_action: bool,  // Flag to execute action on next loop iteration
    pub pending_transfer: bool,  // Flag specifically for transfer action
    pub unlock_complete: Option<Arc<AtomicBool>>,  // Flag to detect when unlock finishes
    pub unlock_success_message: Option<String>,  // Success message to display
    pub lock_complete: Option<Arc<AtomicBool>>,  // Flag to detect when lock finishes
    pub lock_success_message: Option<String>,  // Success message to display
    // Transfer state
    pub transfer_recipient: String,
    pub transfer_amount: String,
    pub transfer_focused_field: TransferInputField,
    pub transfer_token_type: TransferTokenType,
    pub in_transfer_form: bool,  // True when actively editing transfer form
    // Bridge state
    pub bridge_amount: String,
    pub standard_mint: Pubkey,  // Standard qcoin mint
    pub pq_mint: Pubkey,        // pqcoin mint
    // New vault state
    pub new_vault_name: String,
    // Vault management state
    pub vault_management_mode: VaultManagementMode,
    pub vault_list: Vec<crate::vault_manager::VaultProfile>,
    pub selected_vault_index: usize,
    pub in_vault_list: bool,  // True when actively in vault list
    // Delete confirmation state
    pub vault_to_delete: String,
    pub delete_confirmation_input: String,
    // Close confirmation state
    pub vault_to_close: String,
    pub close_confirmation_input: String,
    // Animation state
    pub animation_frame: u8,  // Counter for animation frames
    pub last_animation_update: std::time::Instant,
    // Chart state
    pub chart_type: ChartType,
    pub chart_timeframe: ChartTimeframe,
    pub airdrop_timeframe: ChartTimeframe,
    // Cached airdrop stats
    pub airdrop_distributed: u64,
    pub airdrop_remaining: u64,
}
