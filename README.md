# pqcoin

Post-quantum digital currency CLI - Quantum-resistant cryptocurrency vault with SPHINCS+ signatures and interactive TUI dashboard.

## Features

- 🖥️ **Interactive TUI Dashboard** - Elegant terminal interface for vault management
- 🔐 **SPHINCS+-SHA2-128s** post-quantum signatures (NIST FIPS 205)
- 🔒 **Vault Locking** with cryptographic challenges
- ✅ **44-Transaction Verification** for on-chain signature validation
- 💸 **Token Transfers** with Token-2022 transfer hooks
- 📊 **Real-time Status** - Live vault and balance monitoring
- 🌐 **Solana Integration** via RPC (devnet/mainnet)

## Tokenomics

**Fixed total supply** of **4,294,967,296 tokens** (2³² with 6 decimals).

**Distribution:**
- **80% (3,435,973,837)** - Initial liquidity pool (locked at program initialization)
- **17% (730,144,440)** - Protocol reserves (minted to dev wallet at initialization)
- **3% (128,849,019)** - Community airdrops (distributed at authority's discretion)

**Minting Model:**
- Initial distribution function mints 97% (80% + 17%) in a single transaction at program setup
- Mint authority is then transferred to a PDA, preventing unauthorized minting
- Airdrop function allows authority-controlled distribution of the remaining 3%
- Total supply is cryptographically enforced at 4,294,967,296 tokens

## Prerequisites

Before installing pqcoin, you need Rust and Cargo installed on your system.

### Install Rust

**Linux/macOS/WSL:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, restart your terminal or run:
```bash
source $HOME/.cargo/env
```

**Windows:**

We recommend using WSL (Windows Subsystem for Linux) for the best experience:

1. **Install WSL** (if not already installed):
   - Open PowerShell as Administrator and run:
     ```powershell
     wsl --install
     ```
   - Restart your computer

2. **Launch WSL**:
   - Open "Ubuntu" from Start Menu, or
   - Type `wsl` in PowerShell, or
   - Type `bash` in Command Prompt

3. **Install Rust in WSL**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

Alternatively, you can use native Windows with [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022), but WSL provides better terminal experience for the quantum-themed UI.

## Installation

### Install from crates.io (Recommended)

Once Rust is installed, simply run:

```bash
cargo install pqcoin
```

That's it! The `pqcoin` command is now available globally.

### Alternative Installation Methods

#### Build from Source (GitHub)
```bash
git clone https://github.com/quantdum/pqcoin.git
cd pqcoin
cargo install --path .
```

#### Local Development
```bash
git clone https://github.com/quantdum/pqcoin.git
cd pqcoin
cargo build
./target/debug/pqcoin --help
```

#### Quick Install Script
```bash
./install.sh
```

## Usage

### TUI Dashboard (Recommended)

Launch the interactive dashboard for the best experience:

```bash
pqcoin dashboard
# Or simply:
pqcoin
```

The dashboard will use your active vault profile from `~/.qdum/vaults.json`.

**Dashboard Features:**
- 📊 **Real-time vault status** - See if your vault is locked or unlocked
- 💰 **Live balance display** - Monitor your token balance
- 🔓 **Interactive unlock** - Visual progress through 44-transaction verification
- 💸 **Easy transfers** - Transfer tokens with on-screen guidance
- ⚡ **Quick actions** - Register, lock, and manage vault with keyboard shortcuts

**Keyboard Controls:**
- `U` - Unlock vault (44-step quantum verification)
- `R` - Register vault on-chain
- `L` - Lock vault
- `T` - Transfer tokens
- `Q` - Quit

### First Time Setup

1. **Initialize New Vault**
   ```bash
   pqcoin init
   ```

   This creates:
   - SPHINCS+ keypair (32-byte public key, 64-byte private key)
   - Solana wallet keypair
   - Vault profile in `~/.qdum/vaults.json`

   **Keep your keys safe!** They're stored in `~/.qdum/<vault-name>-{pq-key,wallet}.json`

2. **Fund Your Wallet**
   ```bash
   # Get devnet SOL for testing
   solana airdrop 1 <YOUR_WALLET_ADDRESS> --url devnet
   ```

3. **Launch Dashboard**
   ```bash
   pqcoin dashboard
   # Or simply:
   pqcoin
   ```

4. **Register On-Chain**

   Press `R` in the dashboard to register your SPHINCS+ public key on-chain.

5. **Claim Airdrop**

   Press `A` to claim 100 tokens from the community airdrop pool (requires registered PQ account).

6. **Lock Your Vault**

   Press `L` to lock your vault and secure your tokens.

7. **Unlock When Needed**

   Press `U` to unlock - watch the 44-step quantum verification process in real-time!

### CLI Commands (Alternative)

For scripting or automation, you can use individual commands:

```bash
# Register vault on-chain
pqcoin register

# Lock vault
pqcoin lock

# Unlock vault (44-transaction quantum verification)
pqcoin unlock

# Check vault status
pqcoin status

# Check balance
pqcoin balance

# Transfer tokens
pqcoin transfer <RECIPIENT_ADDRESS> <AMOUNT>

# Bridge operations
pqcoin bridge wrap <AMOUNT>    # Convert to quantum-protected variant
pqcoin bridge unwrap <AMOUNT>  # Convert back to standard tokens

# Claim airdrop (100 tokens, 24h cooldown)
pqcoin claim-airdrop

# Vault management
pqcoin vault list              # List all vaults
pqcoin vault switch            # Interactive vault switcher
pqcoin vault create <NAME>     # Create new vault
pqcoin vault show              # Show current vault details
```

**Note:** Commands use the active vault from `~/.qdum/vaults.json`. Use `pqcoin vault switch` to change vaults.

## Configuration

### Vault Profiles

pqcoin uses a multi-vault system stored in `~/.qdum/vaults.json`:

```bash
# Create additional vaults
pqcoin vault create personal
pqcoin vault create business

# Switch between vaults
pqcoin vault switch personal

# List all vaults
pqcoin vault list
```

### Network Configuration

Default: Devnet (`https://api.devnet.solana.com`)

To use mainnet (when deployed):
- Update `src/solana/client.rs` with mainnet RPC and program IDs
- Rebuild: `cargo build --release`

## Architecture

- **Algorithm**: SPHINCS+-SHA2-128s (NIST FIPS 205)
- **Public Key**: 32 bytes
- **Private Key**: 64 bytes
- **Signature**: 7,856 bytes
- **Verification**: 44 transactions total:
  - 1 signature generation
  - 1 storage initialization
  - 10 signature upload chunks (800 bytes each)
  - 1 verification state init
  - 3 FORS tree verification
  - 28 WOTS+ layer verification (7 layers × 4 steps)
  - 1 finalization
- **PDA Reuse**: Saves ~0.07 SOL on subsequent unlocks

## Security

⚠️ **IMPORTANT**: Keep your SPHINCS+ private key extremely safe!
- Store offline or in a hardware security module
- Never share or commit to version control
- Anyone with your private key can unlock your vault

## Development

### Build
```bash
cargo build
```

### Test
```bash
cargo test
```

### Install Locally
```bash
cargo install --path .
```

### Uninstall
```bash
cargo uninstall pqcoin
```

## Troubleshooting

### Command not found
Make sure `~/.cargo/bin` is in your PATH:
```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Need SOL for transactions
```bash
solana airdrop 1 --url devnet
```

## License

MIT
