# QDUM Tokenomics

## Token Distribution

**Total Supply**: 4,294,967,296 QDUM (2³² tokens) - Fixed and immutable

### Allocation

| Allocation | Percentage | Amount (QDUM) | Purpose |
|------------|------------|---------------|---------|
| **Liquidity Pool** | 80% | 3,435,973,836.8 | Initial DEX liquidity on Raydium/Orca (locked) |
| **Protocol Reserves** | 17% | 730,144,440.32 | Development, operations, ecosystem growth |
| **Community Airdrop** | 3% | 128,849,018.88 | Controlled airdrops at authority's discretion |

## Distribution Mechanism

### Initial Distribution (One-Time)
The program includes an `initial_distribution` instruction that can only be called **once**:

1. **80% → Liquidity Pool**: 3,435,973,836.8 QDUM
   - Minted to a liquidity token account
   - Deposited into Raydium/Orca DEX pool
   - LP tokens locked/burned for permanent liquidity

2. **17% → Protocol Reserves**: 730,144,440.32 QDUM
   - Minted to development wallet: `JBuRohHDXhDUsmnauxP1sd7QDRXUANA53UQu49mBJppjm`
   - Used for development, operations, ecosystem growth
   - All transactions visible on-chain

3. **3% → Community Airdrop**: 128,849,018.88 QDUM
   - Distributed via `airdrop` instruction
   - Controlled by program authority
   - Capped at 3% maximum
   - For community incentives, partnerships, growth

### Security Model
- **Initial distribution** can only be called once (enforced on-chain)
- After distribution, **mint authority transferred to PDA**
- Only the `airdrop` function can mint additional tokens (3% cap)
- Total supply cryptographically enforced - impossible to exceed 2³² QDUM

## Liquidity Pool Strategy

### Benefits
1. **Locked Liquidity**: 80% in permanent DEX pool (LP tokens burned)
2. **Immediate Trading**: Large pool depth from day 1
3. **Fair Price Discovery**: Market determines token value
4. **No Inflation**: Fixed supply, mint authority controlled by PDA

## Protocol Reserves (17%)

**Development Wallet**: `JBuRohHDXhDUsmnauxP1sd7QDRXUANA53UQu49mBJppjm`

### Usage
- Team compensation
- Continued development
- Marketing and partnerships
- Ecosystem grants
- Operational expenses
- Security audits

### Transparency
- All transactions visible on-chain via Solana explorers
- Community can track all reserve movements
- Wallet address updatable only by program authority

## Community Airdrop (3%)

### Airdrop Mechanism
- **Maximum**: 128,849,018.88 QDUM (3% of total supply)
- **Control**: Only program authority can execute airdrops
- **On-chain Tracking**: `airdrop_distributed` counter prevents exceeding cap
- **Validation**: Combined total (97% initial + 3% airdrop) cannot exceed total supply

### Potential Uses
- Early adopters and community rewards
- Partnership incentives
- Ecosystem development grants
- Liquidity mining programs
- Strategic distributions

## Token Supply Breakdown

### At Launch (Initial Distribution)
- **Total Minted**: 4,166,118,277.12 QDUM (97%)
  - 80% Liquidity: 3,435,973,836.8 QDUM
  - 17% Reserves: 730,144,440.32 QDUM

### Reserved (Airdrop Cap)
- **Total Reserved**: 128,849,018.88 QDUM (3%)
  - Mintable only via `airdrop` instruction
  - Tracked on-chain with `airdrop_distributed` counter

### Total Supply Cap
- **Maximum Ever**: 4,294,967,296 QDUM (2³² tokens)
- **Enforcement**: On-chain validation prevents exceeding cap
- **Immutable**: Mint authority controlled by PDA after initial distribution

## On-Chain Implementation

### Token Distribution Instructions

1. **`initialize`**: Initialize program state
   - Sets up mint state with authority
   - Can only be called once

2. **`initial_distribution`**: One-time distribution (80% + 17%)
   - Mints 80% to liquidity token account
   - Mints 17% to protocol reserves token account
   - Updates `authority_minted` counter
   - **Can only be called once** (requires `authority_minted == 0`)

3. **`airdrop`**: Controlled airdrop (3% cap)
   - Only callable by program authority
   - Capped at 128,849,018.88 QDUM total
   - Tracks `airdrop_distributed` counter
   - Validates combined total doesn't exceed supply

### Security Instructions
- **Transfer Hook**: Token-2022 transfer validation
- **SPHINCS+ Verification**: 44-step quantum-resistant signature verification
- **Lock/Unlock**: Token locking mechanism
- **Metadata Management**: On-chain token metadata

### Administrative Instructions
- **`update_dev_wallet`**: Update development wallet address
- **`transfer_authority`**: Transfer program authority

## Launch Checklist

### Pre-Launch
- [ ] Fund deployer wallet: 5 SOL on mainnet
- [ ] Create Token-2022 mint with metadata extension
- [ ] Initialize token metadata (name, symbol, logo URI)
- [ ] Create token accounts (liquidity + protocol reserves)
- [ ] Deploy program to mainnet (~4.57 SOL)
- [ ] Initialize program state
- [ ] Determine initial SOL:QDUM price ratio
- [ ] Calculate SOL needed for liquidity pairing

### Launch Day
1. **Initial Distribution** (ONE TIME ONLY!)
   ```bash
   npx ts-node scripts/initial-distribution.ts \
     --mint <MINT> \
     --liquidity-account <LIQUIDITY_ACCOUNT> \
     --protocol-account <PROTOCOL_ACCOUNT>
   ```
   - Mints 80% (3,435,973,836.8 QDUM) → Liquidity account
   - Mints 17% (730,144,440.32 QDUM) → Protocol reserves

2. **Transfer Mint Authority to PDA**
   ```bash
   spl-token authorize <MINT> mint <MINT_AUTHORITY_PDA>
   ```
   - Critical security step!
   - Prevents unauthorized minting

3. **Create Liquidity Pool**
   - Use Raydium or Orca DEX
   - Deposit 3,435,973,836.8 QDUM (all 80%)
   - Add matching SOL for desired price
   - Receive LP tokens

4. **Lock/Burn LP Tokens**
   - Burn LP tokens (permanent), OR
   - Lock with Streamflow (time-locked), OR
   - Send to multisig (governance)
   - **Share proof transaction publicly**

5. **Announce Launch**
   - Contract address
   - DEX pool link
   - LP lock proof
   - Tokenomics breakdown

### Post-Launch
- [ ] Monitor pool health and trading
- [ ] Verify token shows in wallets (logo, name, symbol)
- [ ] List on aggregators (Jupiter, DexScreener, BirdEye)
- [ ] Submit to token registries
- [ ] Apply to CoinGecko/CoinMarketCap (after 2-4 weeks)
- [ ] Execute community airdrops (3% cap)
- [ ] Marketing and community growth

## Risk Considerations

### Liquidity
- ✅ **Mitigated**: 80% locked permanently via burned/locked LP tokens
- ✅ **No rug pull risk**: LP tokens removed from circulation
- ⚠️ **Market volatility**: Price determined by market forces

### Protocol Reserves (17%)
- ⚠️ **Large allocation**: 730M+ QDUM in development wallet
- ✅ **Transparency**: All transactions visible on-chain
- ✅ **Accountability**: Community can track usage via Solana explorers
- 💡 **Recommendation**: Consider public vesting schedule or quarterly reports

### Airdrop Allocation (3%)
- ✅ **Capped**: Maximum 128M QDUM (cannot exceed)
- ✅ **Controlled**: Only program authority can execute
- ✅ **Tracked**: On-chain counter prevents abuse
- ⚠️ **Authority risk**: Centralized control (consider community governance)

### Supply Security
- ✅ **Immutable cap**: 2³² tokens maximum (enforced on-chain)
- ✅ **PDA-controlled minting**: Mint authority transferred to PDA
- ✅ **No inflation**: Fixed supply, no additional minting beyond cap
- ✅ **Auditable**: All supply changes tracked on-chain

## Future Considerations

- **Governance**: DAO for protocol reserves spending
- **Staking**: Incentive mechanisms for long-term holders
- **Utility**: Ecosystem integrations and use cases
- **Cross-chain**: Bridges to other blockchain networks
- **Partnerships**: Strategic integrations leveraging post-quantum security

---

## Key Features

- 🔒 **Quantum-Resistant**: SPHINCS+-SHA2-128s (NIST FIPS 205)
- 🔐 **Secure Vaults**: Lock tokens with 44-step cryptographic verification
- 💧 **Locked Liquidity**: 80% permanently locked via burned LP tokens
- 🎯 **Fixed Supply**: 2³² tokens maximum, cryptographically enforced
- ✅ **Transparent**: All distribution tracked on-chain

**Program ID**: `HyC27AVHW4VwkEiWwWxevaUpvkiAqPUueaa94og9HmLQ`

---

For more information:
- 📖 [GitHub Repository](https://github.com/quantdum/pqcoin)
- 📊 [Token Tracker](https://solscan.io)
- 🌐 [Website](https://github.com/quantdum/pqcoin)
