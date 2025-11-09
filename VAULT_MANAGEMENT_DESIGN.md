# Multi-Vault Management System Design

## Current Limitations

The existing config system only stores:
- Single keypair path
- No vault profiles
- No easy switching between wallets

## Proposed Design

### 1. Vault Profile Structure

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct VaultProfile {
    /// Unique name for this vault (e.g., "personal", "trading", "cold-storage")
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Path to Solana keypair JSON
    pub solana_keypair_path: String,

    /// Path to SPHINCS+ public key
    pub sphincs_public_key_path: String,

    /// Path to SPHINCS+ private key
    pub sphincs_private_key_path: String,

    /// Wallet address (cached for quick display)
    pub wallet_address: String,

    /// When this vault was created
    pub created_at: String,

    /// Last used timestamp
    pub last_used: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct VaultConfig {
    /// Active vault name
    pub active_vault: Option<String>,

    /// All vault profiles
    pub vaults: HashMap<String, VaultProfile>,

    /// Config version (for future migrations)
    pub version: u32,
}
```

### 2. New Commands

```
pqcoin vault list              # List all vaults
pqcoin vault create <NAME>     # Create new vault profile
pqcoin vault switch <NAME>     # Switch active vault
pqcoin vault switch            # Interactive menu to select vault
pqcoin vault delete <NAME>     # Delete a vault profile
pqcoin vault show <NAME>       # Show vault details
pqcoin vault rename <OLD> <NEW> # Rename a vault
```

### 3. Interactive Vault Switcher

When you run `pqcoin vault switch` (no arguments), show interactive menu:

```
╔═══════════════════════════════════════════════════════════╗
║              Select Vault (↑↓ arrows, Enter)              ║
╠═══════════════════════════════════════════════════════════╣
║                                                           ║
║  ● Personal Wallet                     [ACTIVE]          ║
║    └─ 7vZ8...Xq2M (0.5 SOL, 1,000 QDUM)                 ║
║                                                           ║
║  ○ Trading Wallet                                        ║
║    └─ 9Kp3...Yz7N (2.3 SOL, 50,000 QDUM)                ║
║                                                           ║
║  ○ Cold Storage                                          ║
║    └─ 4Hj9...Wq8P (10 SOL, 500,000 QDUM) [LOCKED]       ║
║                                                           ║
║  + Create New Vault                                      ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

[q] Quit  [↑↓] Navigate  [Enter] Select  [d] Delete
```

### 4. Automatic Context

Once a vault is active, all commands use it automatically:

```bash
# No need to specify --keypair every time!
pqcoin status           # Uses active vault
pqcoin lock             # Uses active vault
pqcoin unlock           # Uses active vault
pqcoin balance          # Uses active vault

# Override if needed
pqcoin status --vault trading    # Use "trading" vault temporarily
```

### 5. Dashboard Integration

The dashboard should show:
- Current active vault name
- Quick vault switcher (press 'V' to switch)
- Visual indicator of which vault is active

```
╔═══════════════════════════════════════════════════════════╗
║  QDUM Vault Dashboard                [Personal Wallet]   ║
╠═══════════════════════════════════════════════════════════╣
║  Wallet: 7vZ8...Xq2M                                     ║
║  Balance: 1,000 QDUM                                      ║
║  Status: 🔓 Unlocked                                      ║
╚═══════════════════════════════════════════════════════════╝

[V] Switch Vault  [U] Unlock  [L] Lock  [T] Transfer  [Q] Quit
```

---

## Implementation Plan

### Phase 1: Core Vault Management

**File**: `src/vault_manager.rs` (new)

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Serialize, Deserialize, Clone)]
pub struct VaultProfile {
    pub name: String,
    pub description: Option<String>,
    pub solana_keypair_path: String,
    pub sphincs_public_key_path: String,
    pub sphincs_private_key_path: String,
    pub wallet_address: String,
    pub created_at: String,
    pub last_used: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct VaultConfig {
    pub active_vault: Option<String>,
    pub vaults: HashMap<String, VaultProfile>,
    pub version: u32,
}

impl VaultConfig {
    pub fn load() -> Result<Self> {
        // Load from ~/.qdum/vaults.json
    }

    pub fn save(&self) -> Result<()> {
        // Save to ~/.qdum/vaults.json
    }

    pub fn create_vault(&mut self, name: String, profile: VaultProfile) -> Result<()> {
        // Add new vault profile
    }

    pub fn switch_vault(&mut self, name: &str) -> Result<()> {
        // Set active vault
    }

    pub fn get_active_vault(&self) -> Option<&VaultProfile> {
        // Get current active vault
    }

    pub fn delete_vault(&mut self, name: &str) -> Result<()> {
        // Remove vault profile
    }

    pub fn list_vaults(&self) -> Vec<&VaultProfile> {
        // Return all vaults sorted by last_used
    }
}
```

### Phase 2: CLI Commands

**File**: `src/main.rs` (update Commands enum)

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Vault management (create, switch, list, delete)
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },
}

#[derive(Subcommand)]
enum VaultAction {
    /// List all vault profiles
    List,

    /// Create a new vault profile
    Create {
        /// Name for the vault
        name: String,

        /// Description (optional)
        #[arg(long)]
        description: Option<String>,

        /// Generate new keys automatically
        #[arg(long)]
        auto_generate: bool,
    },

    /// Switch active vault (interactive if no name provided)
    Switch {
        /// Vault name (omit for interactive menu)
        name: Option<String>,
    },

    /// Show vault details
    Show {
        /// Vault name (defaults to active)
        name: Option<String>,
    },

    /// Delete a vault profile
    Delete {
        /// Vault name
        name: String,

        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },

    /// Rename a vault
    Rename {
        /// Current name
        old_name: String,

        /// New name
        new_name: String,
    },
}
```

### Phase 3: Interactive Vault Switcher

**File**: `src/vault_switcher.rs` (new)

```rust
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

pub struct VaultSwitcher {
    vaults: Vec<VaultProfile>,
    state: ListState,
    active_vault_name: Option<String>,
}

impl VaultSwitcher {
    pub fn new(config: &VaultConfig) -> Self {
        // Initialize with vault list
    }

    pub fn run(&mut self) -> Result<Option<String>> {
        // Run interactive TUI, return selected vault name
    }

    fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        // Draw the vault selection UI
    }

    fn handle_input(&mut self) -> Result<bool> {
        // Handle keyboard input (up/down/enter/q/d)
    }
}
```

### Phase 4: Dashboard Integration

**File**: `src/dashboard.rs` (update)

Add vault switcher to dashboard:
- Press 'V' to open vault switcher
- Show active vault name in header
- Refresh data when vault is switched

---

## User Experience Examples

### Example 1: First-time Setup

```bash
# Initialize first vault (automatically becomes active)
$ pqcoin init
✓ Generated SPHINCS+ keys
✓ Generated Solana keypair
✓ Created vault profile: "default"
✓ Vault "default" is now active

Keys saved to ~/.qdum/
Wallet: 7vZ8mpR3HqLqkFX2nM5Xq2M
```

### Example 2: Create Additional Vaults

```bash
# Create new vault with auto-generated keys
$ pqcoin vault create trading --auto-generate --description "My trading wallet"
✓ Generated new keys
✓ Created vault profile: "trading"

Switch to this vault? [Y/n]: y
✓ Active vault: trading

# Or create vault with existing keys
$ pqcoin vault create cold-storage
? Solana keypair path: ~/.config/solana/cold-wallet.json
? SPHINCS+ public key: ~/.qdum/cold/sphincs_public.key
? SPHINCS+ private key: ~/.qdum/cold/sphincs_private.key
✓ Created vault profile: "cold-storage"
```

### Example 3: Interactive Vault Switching

```bash
# Open interactive menu
$ pqcoin vault switch

# Shows TUI menu, user selects with arrows, press Enter
# Menu disappears, new vault is active

✓ Switched to vault: trading
  Wallet: 9Kp3...Yz7N
```

### Example 4: List All Vaults

```bash
$ pqcoin vault list

╔═══════════════════════════════════════════════════════════╗
║                     Your Vaults                           ║
╠═══════════════════════════════════════════════════════════╣
║                                                           ║
║  ● Personal Wallet                      [ACTIVE]         ║
║    Wallet: 7vZ8mpR3HqLqkFX2nM5Xq2M                       ║
║    Balance: 1,000 QDUM                                    ║
║    Status: Unlocked                                       ║
║    Last used: 2 minutes ago                               ║
║                                                           ║
║  ○ Trading Wallet                                        ║
║    Wallet: 9Kp3RzWnTyMqPxYz7N                           ║
║    Balance: 50,000 QDUM                                   ║
║    Status: Unlocked                                       ║
║    Last used: 1 hour ago                                  ║
║                                                           ║
║  ○ Cold Storage                                          ║
║    Wallet: 4Hj9LmNpQwRzWq8P                             ║
║    Balance: 500,000 QDUM                                  ║
║    Status: Locked 🔒                                     ║
║    Last used: 3 days ago                                  ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

Switch vault:    pqcoin vault switch
Create vault:    pqcoin vault create <name>
Delete vault:    pqcoin vault delete <name>
```

### Example 5: Temporary Vault Override

```bash
# Use different vault for one command
$ pqcoin balance --vault cold-storage
Balance: 500,000 QDUM

# Active vault unchanged
$ pqcoin status
Wallet: 7vZ8...Xq2M (Personal Wallet)
Status: Unlocked
```

### Example 6: Dashboard Quick Switch

```bash
$ pqcoin dashboard

# In dashboard, press 'V'
# Interactive vault switcher appears as modal
# Select vault, press Enter
# Dashboard refreshes with new vault data
```

---

## Benefits

### 1. **Simplified Workflow**
- No need to remember or type keypair paths
- One command to switch between wallets
- Visual, intuitive vault selection

### 2. **Better Organization**
- Name your vaults descriptively
- Group by purpose (trading, savings, cold storage)
- Track last usage

### 3. **Safety**
- Each vault has isolated keys
- Clear indication of which vault is active
- Prevent accidental operations on wrong wallet

### 4. **Power User Features**
- Quick switching in dashboard (press 'V')
- Command-line vault selection
- Temporary vault override for single commands

---

## Migration from Current Config

Old config (single keypair):
```json
{
  "keypair_path": "/home/user/.config/solana/id.json"
}
```

Will auto-migrate to:
```json
{
  "version": 1,
  "active_vault": "default",
  "vaults": {
    "default": {
      "name": "default",
      "description": "Auto-migrated from old config",
      "solana_keypair_path": "/home/user/.config/solana/id.json",
      "sphincs_public_key_path": "/home/user/.qdum/sphincs_public.key",
      "sphincs_private_key_path": "/home/user/.qdum/sphincs_private.key",
      "wallet_address": "7vZ8mpR3HqLqkFX2nM5Xq2M",
      "created_at": "2025-11-06T10:30:00Z",
      "last_used": "2025-11-06T10:30:00Z"
    }
  }
}
```

---

## Implementation Checklist

- [ ] Create `src/vault_manager.rs` with VaultConfig/VaultProfile
- [ ] Create `src/vault_switcher.rs` with interactive TUI
- [ ] Update `src/main.rs` with Vault commands
- [ ] Add vault switching to dashboard ('V' key)
- [ ] Add migration logic for old config
- [ ] Update all existing commands to use active vault
- [ ] Add `--vault <NAME>` flag to all commands for override
- [ ] Write tests for vault management
- [ ] Update README with vault management docs
- [ ] Add examples to help text

---

## Example Implementation Files

I can generate the complete implementation code for:
1. `src/vault_manager.rs` - Core vault management
2. `src/vault_switcher.rs` - Interactive TUI
3. Updated `src/main.rs` - New commands
4. Updated `src/dashboard.rs` - Vault switcher integration

Would you like me to create the full implementation?
