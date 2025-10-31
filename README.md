# QDUM Vault CLI

Command-line tool for managing your quantum-resistant Quantdum token vault.

## Features

- 🔐 **SPHINCS+-SHA2-128s** post-quantum signatures (NIST FIPS 205)
- 🔒 **Vault Locking** with cryptographic challenges
- ✅ **11-Step Verification** for on-chain signature validation
- 🪙 **Token Minting** with progressive fee structure
- 🌐 **Solana Integration** via RPC (devnet/mainnet)

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

### 1. Generate SPHINCS+ Keypair
```bash
qdum-vault init
```

Keys are saved to `~/.qdum/`:
- `sphincs_private.key` (64 bytes)
- `sphincs_public.key` (32 bytes)

### 2. Register Your Vault On-Chain
```bash
qdum-vault register \
  --wallet YOUR_WALLET_ADDRESS \
  --keypair ~/.config/solana/id.json
```

### 3. Lock Your Vault
```bash
qdum-vault lock \
  --wallet YOUR_WALLET_ADDRESS \
  --keypair ~/.config/solana/id.json
```

### 4. Unlock Your Vault
```bash
qdum-vault unlock \
  --wallet YOUR_WALLET_ADDRESS \
  --keypair ~/.config/solana/id.json
```

This performs an 11-step SPHINCS+ signature verification on-chain.

### 5. Check Status
```bash
qdum-vault status --wallet YOUR_WALLET_ADDRESS
```

### 6. Check Balance
```bash
qdum-vault balance \
  --keypair ~/.config/solana/id.json \
  --mint 3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n
```

### 7. Mint QDUM Tokens
```bash
qdum-vault mint \
  --amount 10000000000 \
  --keypair ~/.config/solana/id.json
```

Mint QDUM tokens with progressive fees:
- Amount range: 10,000 to 50,000 QDUM (in base units with 6 decimals)
- Example: 10,000 QDUM = 10000000000 base units
- Fees increase based on scarcity (how much has been minted)

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

- **Algorithm**: SPHINCS+-SHA2-128s
- **Public Key**: 32 bytes
- **Private Key**: 64 bytes
- **Signature**: 7,856 bytes
- **Verification**: 11 transactions (chunked)

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
