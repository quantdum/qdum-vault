# QDUM Vault CLI

Command-line tool for managing your quantum-resistant Quantdum token vault.

## Features

- 🔐 **SPHINCS+-SHA2-128s** post-quantum signatures (NIST FIPS 205)
- 🔒 **Vault Locking** with cryptographic challenges
- ✅ **11-Step Verification** for on-chain signature validation
- 🌐 **Solana Integration** via RPC (devnet/mainnet)

## Installation

### Quick Install
```bash
./install.sh
```

### Manual Install
```bash
cargo install --path .
```

### Development Mode
```bash
cargo build
./target/debug/qdum-vault --help
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
  --wallet YOUR_WALLET_ADDRESS \
  --mint MINT_ADDRESS
```

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
