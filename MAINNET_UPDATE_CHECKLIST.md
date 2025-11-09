# pqcoin Mainnet Update Checklist

## What Needs to Change for Mainnet

After deploying the token to mainnet, update these values in pqcoin:

### 1. Default RPC URL
**Current**: `https://api.devnet.solana.com`
**Mainnet**: `https://api.mainnet-beta.solana.com`

**Files to update**:
- `src/main.rs` line 54: CLI default RPC
- `src/main.rs` line 44: Help text display

### 2. Default Mint Address
**Current**: `3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n` (devnet)
**Mainnet**: TBD (will be created during mainnet deployment)

**Files to update**:
- `src/main.rs` line 128: Balance command default
- `src/main.rs` line 147: Transfer command default
- `src/main.rs` line 508: Dashboard default

### 3. Program ID
**Current**: `HyC27AVHW4VwkEiWwWxevaUpvkiAqPUueaa94og9HmLQ`
**Mainnet**: SAME (if deploying with same keypair)

**Note**: Program ID is deterministic based on deploy keypair. If you deploy to mainnet with the same keypair used for devnet, the program ID will be identical. No update needed!

### 4. Solscan Explorer Links
**Current**: `https://solscan.io/tx/{}?cluster=devnet`
**Mainnet**: `https://solscan.io/tx/{}` (no cluster parameter for mainnet)

**Files to update**:
- `src/solana/client.rs` line 154
- `src/solana/client.rs` line 210
- `src/solana/client.rs` line 1352

### 5. Display Text
**Current**: "Connecting to Solana devnet..."
**Mainnet**: "Connecting to Solana..."

**Files to update**:
- `src/dashboard.rs` line 700

---

## Update Instructions

### Step 1: After Mainnet Deployment

Once you've deployed to mainnet and have your mint address:

```bash
export MAINNET_MINT="<YOUR_MAINNET_MINT_ADDRESS>"
```

### Step 2: Update Default Values

Run this script to update all defaults automatically:

```bash
# Update RPC URL
sed -i 's|https://api.devnet.solana.com|https://api.mainnet-beta.solana.com|g' src/main.rs

# Update mint address (replace with your actual mint)
sed -i "s|3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n|$MAINNET_MINT|g" src/main.rs

# Update Solscan links
sed -i 's|?cluster=devnet||g' src/solana/client.rs

# Update dashboard text
sed -i 's|Connecting to Solana devnet|Connecting to Solana|g' src/dashboard.rs
```

### Step 3: Update Help Text

Edit `src/main.rs` line 44 to show mainnet RPC in help:

```rust
// Before:
"RPC:".bright_blue(), "https://api.devnet.solana.com".dimmed(),

// After:
"RPC:".bright_blue(), "https://api.mainnet-beta.solana.com".dimmed(),
```

### Step 4: Build and Test

```bash
# Build
cargo build --release

# Test mainnet connection
./target/release/pqcoin --rpc-url https://api.mainnet-beta.solana.com status --wallet <ADDRESS>

# Verify correct defaults
./target/release/pqcoin --help
```

### Step 5: Update Version

Edit `Cargo.toml`:

```toml
[package]
name = "pqcoin"
version = "2.0.0"  # Mainnet launch version
```

### Step 6: Publish

```bash
# Commit changes
git add -A
git commit -m "Update defaults for mainnet launch

- Change default RPC to mainnet
- Update default mint to mainnet address
- Fix Solscan links for mainnet
- Bump version to 2.0.0"

# Tag release
git tag -a v2.0.0 -m "Mainnet launch"
git push origin main --tags

# Publish to crates.io
cargo publish
```

---

## Alternative: Keep Devnet Support

If you want to support both devnet and mainnet:

### Option A: Environment Variable

```rust
// src/main.rs
fn get_default_rpc() -> String {
    std::env::var("QDUM_NETWORK")
        .map(|n| match n.as_str() {
            "mainnet" => "https://api.mainnet-beta.solana.com".to_string(),
            _ => "https://api.devnet.solana.com".to_string(),
        })
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string())
}
```

### Option B: Add `--mainnet` Flag

```rust
#[derive(Parser)]
struct Cli {
    /// Use mainnet instead of devnet
    #[arg(long)]
    mainnet: bool,

    // ... other fields
}
```

### Option C: Network Subcommand

```rust
#[derive(Parser)]
struct Cli {
    /// Network selection
    #[arg(long, default_value = "mainnet", value_parser = ["mainnet", "devnet"])]
    network: String,

    // ... other fields
}
```

---

## Testing Checklist

After updating for mainnet:

- [ ] `pqcoin --help` shows mainnet RPC
- [ ] `pqcoin status` connects to mainnet by default
- [ ] Default mint address is mainnet QDUM token
- [ ] Solscan links open correctly (no devnet cluster param)
- [ ] Dashboard connects to mainnet
- [ ] All commands work with mainnet
- [ ] `--rpc-url` flag still allows devnet override
- [ ] Version bumped in Cargo.toml
- [ ] CHANGELOG.md updated

---

## Quick Reference

### Current Defaults (Devnet)
```
RPC:     https://api.devnet.solana.com
Program: HyC27AVHW4VwkEiWwWxevaUpvkiAqPUueaa94og9HmLQ
Mint:    3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n
```

### Mainnet Defaults (After Update)
```
RPC:     https://api.mainnet-beta.solana.com
Program: HyC27AVHW4VwkEiWwWxevaUpvkiAqPUueaa94og9HmLQ (SAME!)
Mint:    <YOUR_MAINNET_MINT_ADDRESS>
```

---

## Important Notes

1. **Program ID stays the same** if you deploy with the same keypair
2. **Mint address will be different** - this is the main thing that MUST change
3. Users can still override defaults with CLI flags:
   - `--rpc-url <URL>`
   - `--program-id <PROGRAM_ID>`
   - `--mint <MINT_ADDRESS>`
4. Consider keeping devnet support for testing
5. Update README.md with mainnet addresses

---

## Summary

**Must Update**:
- ✅ Default RPC URL → mainnet
- ✅ Default Mint Address → mainnet mint
- ✅ Solscan links → remove `?cluster=devnet`

**Optional Update**:
- Program ID (only if deploying with different keypair)

**Can Stay Same**:
- All CLI flags and functionality
- SPHINCS+ cryptography
- Transfer hook logic
- Verification flow
