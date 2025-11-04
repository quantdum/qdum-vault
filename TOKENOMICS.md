# QDUM Tokenomics - Liquidity Pool Launch Model

## Token Distribution

**Total Supply**: 4,294,967,296 QDUM (2^32 tokens)

### Allocation

| Allocation | Percentage | Amount (QDUM) | Purpose |
|------------|------------|---------------|---------|
| **Liquidity Pool** | 83% | 3,564,822,855 | Initial DEX liquidity on Raydium/Orca |
| **Development Wallet** | 17% | 730,144,441 | Team, development, marketing, operations |

## Liquidity Pool Strategy

### Initial Launch
- **83% of supply** (3,564,822,855 QDUM) will be deposited into a Solana DEX liquidity pool
- Paired with SOL (amount TBD based on desired initial price)
- Provides instant liquidity for traders
- No public mint mechanism - all tokens distributed through DEX or team

### Benefits
1. **Fair Launch**: No tiered pricing, all market participants get same price
2. **Immediate Liquidity**: Large pool depth from day 1
3. **Price Discovery**: Market determines token value
4. **No Mint Fees**: Users trade directly on DEX with only standard AMM fees

## Development Wallet

**Wallet Address**: TBD (to be set on-chain via `update_dev_wallet` instruction)

### 17% Allocation Usage
- Team compensation
- Continued development
- Marketing and partnerships
- Ecosystem grants
- Operational expenses
- Future airdrops/incentives

### Transparency
- All development wallet transactions visible on-chain
- Wallet address updatable only by program authority
- Consider vesting schedule or lockup for transparency

## Comparison: Old vs New Model

| Aspect | Old Model (Removed) | New Model |
|--------|---------------------|-----------|
| **Distribution** | Public mint with progressive fees | Liquidity pool launch |
| **Cost per User** | $100+ with increasing tiers | Market-determined DEX price |
| **Accessibility** | Limited to 25,000 users | Unlimited via DEX |
| **Liquidity** | Fragmented across users | Concentrated in pool |
| **Price Discovery** | Fixed fee tiers | Market-driven |
| **Revenue Model** | Protocol fees | N/A (market trading) |

## On-Chain Implementation

### Removed Instructions
- `free_mint` - Progressive fee minting
- `set_mint_enabled` - Mint control toggle
- Mint fee calculation logic
- Scarcity multiplier system

### Retained Instructions
- `airdrop` - For development wallet distributions
- `update_dev_wallet` - Update dev wallet address
- `transfer_hook` - Token-2022 transfer hook
- SPHINCS+ verification system
- Metadata management

## Launch Checklist

### Pre-Launch
- [ ] Finalize total supply distribution (confirm 83%/17% split)
- [ ] Set development wallet address
- [ ] Determine initial SOL:QDUM price ratio
- [ ] Select DEX (Raydium, Orca, or other)
- [ ] Calculate SOL needed for liquidity pairing

### Launch Day
1. Deploy program to mainnet
2. Initialize mint with total supply
3. Set development wallet address
4. Mint 83% to liquidity deployer wallet
5. Mint 17% to development wallet
6. Create liquidity pool on DEX
7. Add liquidity (3,564,822,855 QDUM + SOL)
8. Announce launch
9. Update documentation with pool address

### Post-Launch
- [ ] Monitor pool health
- [ ] Track volume and price
- [ ] Community engagement
- [ ] List on aggregators (Jupiter, etc.)
- [ ] Submit to token registries
- [ ] Marketing campaign

## Risk Considerations

### Liquidity Risks
- Large liquidity removes = price impact
- Consider locking liquidity or using escrow
- Monitor for price manipulation

### Development Wallet
- 17% is significant allocation
- Consider vesting/lockup for community trust
- Transparent reporting of usage

### Market Dynamics
- Initial price volatility expected
- No price floor mechanism
- Purely market-driven valuation

## Future Considerations

- Governance for development wallet spending
- Staking mechanisms
- Ecosystem incentives from dev allocation
- Additional liquidity incentives
- Cross-chain bridges

---

**Note**: This model pivots from a controlled public mint to a free market launch. The SPHINCS+ post-quantum security and transfer hook functionality remain core features of the token.
