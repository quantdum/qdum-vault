# QDUM Vault

Quantum-resistant vault management tool for Quantdum tokens with an interactive TUI dashboard.

## Features

- 🖥️ **Interactive TUI Dashboard** - Elegant terminal interface for vault management
- 🔐 **SPHINCS+-SHA2-128s** post-quantum signatures (NIST FIPS 205)
- 🔒 **Vault Locking** with cryptographic challenges
- ✅ **44-Transaction Verification** for on-chain signature validation
- 💸 **Token Transfers** with Token-2022 transfer hooks
- 📊 **Real-time Status** - Live vault and balance monitoring
- 🌐 **Solana Integration** via RPC (devnet/mainnet)

## Tokenomics

The QDUM token has a **fixed total supply** of **4,294,967,296 QDUM** (2³² tokens) with 6 decimals.

**Distribution:**
- **80% (3,435,973,836.8 QDUM)** - Initial liquidity pool (locked at program initialization)
- **17% (730,144,440.32 QDUM)** - Protocol reserves (minted to dev wallet at initialization)
- **3% (128,849,018.88 QDUM)** - Community airdrops (distributed at authority's discretion)

**Minting Model:**
- Initial distribution function mints 97% (80% + 17%) in a single transaction at program setup
- Mint authority is then transferred to a PDA, preventing unauthorized minting
- Airdrop function allows authority-controlled distribution of the remaining 3%
- Total supply is cryptographically enforced - no tokens can be minted beyond 4,294,967,296 QDUM

## Prerequisites

Before installing QDUM Vault, you need Rust and Cargo installed on your system.

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
cargo install qdum-vault
```

That's it! The `qdum-vault` command is now available globally.

### Alternative Installation Methods

#### Build from Source (GitHub)
```bash
git clone https://github.com/quantdum/qdum-vault.git
cd qdum-vault
cargo install --path .
```

#### Local Development
```bash
git clone https://github.com/quantdum/qdum-vault.git
cd qdum-vault
cargo build
./target/debug/qdum-vault --help
```

#### Quick Install Script
```bash
./install.sh
```

## Usage

### TUI Dashboard (Recommended)

Launch the interactive dashboard for the best experience:

```bash
qdum-vault dashboard \
  --wallet YOUR_WALLET_ADDRESS \
  --keypair ~/.config/solana/id.json \
  --mint 3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n
```

**Dashboard Features:**
- 📊 **Real-time vault status** - See if your vault is locked or unlocked
- 💰 **Live balance display** - Monitor your QDUM token balance
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

1. **Generate SPHINCS+ Keypair**
   ```bash
   qdum-vault init
   ```

   Keys are saved to `~/.qdum/`:
   - `sphincs_private.key` (64 bytes) - **Keep this safe!**
   - `sphincs_public.key` (32 bytes)

2. **Launch Dashboard and Register**
   ```bash
   qdum-vault dashboard \
     --wallet YOUR_WALLET_ADDRESS \
     --keypair ~/.config/solana/id.json \
     --mint 3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n
   ```

   Press `R` to register your vault on-chain.

3. **Lock Your Vault**

   Press `L` to lock your vault and secure your tokens.

4. **Unlock When Needed**

   Press `U` to unlock - watch the quantum verification process in real-time!

### CLI Commands (Alternative)

For scripting or automation, you can use individual commands:

```bash
# Register vault
qdum-vault register --wallet YOUR_WALLET --keypair ~/.config/solana/id.json

# Lock vault
qdum-vault lock --wallet YOUR_WALLET --keypair ~/.config/solana/id.json

# Unlock vault (44-transaction quantum verification)
qdum-vault unlock --wallet YOUR_WALLET --keypair ~/.config/solana/id.json

# Check status
qdum-vault status --wallet YOUR_WALLET

# Check balance
qdum-vault balance --keypair ~/.config/solana/id.json --mint MINT_ADDRESS

# Transfer tokens
qdum-vault transfer \
  --to RECIPIENT_ADDRESS \
  --amount 10000000000 \
  --keypair ~/.config/solana/id.json \
  --mint MINT_ADDRESS
```

**Note:** Amount is in base units with 6 decimals (10,000 QDUM = 10000000000 base units)

## Configuration

### Change RPC Endpoint
```bash
qdum-vault --rpc-url https://api.mainnet-beta.solana.com <command>
```

### Use Different Program
```bash
qdum-vault --program-id YOUR_PROGRAM_ID <command>
```

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
cargo uninstall qdum-vault
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
