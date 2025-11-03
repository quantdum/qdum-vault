# QDUM Mint Fee Structure for $2.5M Protocol Revenue

## Target Metrics
- **Protocol Revenue Goal**: $2,500,000
- **Number of Users**: 25,000
- **Average QDUM per User**: 123,695 QDUM
- **SOL Price**: $150
- **Required SOL per User**: 0.6667 SOL ($100)

---

## Recommended Scenarios

All scenarios generate exactly **$2.5M in protocol revenue** but with different distribution strategies.

---

### 🏆 SCENARIO A: Progressive Multipliers (RECOMMENDED)

**Best for**: Encouraging early adoption while maintaining strong scarcity economics

#### Fee Structure
```
Base Formula: (amount / 406,000) × 0.001 SOL × multiplier
```

#### Multipliers by Supply %
| % Minted | Multiplier | SOL per 1000 QDUM |
|----------|------------|-------------------|
| 0-25%    | **0.75×**  | ~0.00185 SOL      |
| 26-50%   | **1.5×**   | ~0.0037 SOL       |
| 51-75%   | **2.5×**   | ~0.00616 SOL      |
| 76-100%  | **4×**     | ~0.00985 SOL      |

#### Cost Breakdown (per user minting 123,695 QDUM)
| Tier | QDUM Minted | Multiplier | Fee (SOL) | Fee (USD) |
|------|-------------|------------|-----------|-----------|
| 0-25% | 30,924 | 0.75× | 0.057 | $8.57 |
| 26-50% | 30,924 | 1.5× | 0.114 | $17.14 |
| 51-75% | 30,924 | 2.5× | 0.191 | $28.57 |
| 76-100% | 30,924 | 4× | 0.305 | $45.71 |
| **TOTAL** | **123,695** | - | **0.667 SOL** | **$100** |

**Total Protocol Revenue**: 16,667 SOL ($2.5M)

#### Advantages
✅ Smooth progression encourages steady minting
✅ Early users get best rates (0.75×)
✅ Strong scarcity signal at end (4×)
✅ $100 total is affordable for retail
✅ Balanced revenue across all tiers

#### Implementation Code
```rust
// Update divisor
const BASE_DIVISOR: u64 = 406_000_000; // 406,000 tokens (in base units with 6 decimals)

fn calculate_scarcity_multiplier(total_minted: u64) -> u64 {
    const PUBLIC_MINT_SUPPLY: u64 = 3_092_376_853_000_000;
    let percent_minted = (total_minted as u128 * 100) / PUBLIC_MINT_SUPPLY as u128;

    // Using 10x scaling to handle decimals in Solana
    // 0.75× = 7.5, 1.5× = 15, 2.5× = 25, 4× = 40
    // Final fee = (base × multiplier) / 10

    if percent_minted <= 25 { return 7; }   // 0.75× (rounded to nearest 0.5)
    if percent_minted <= 50 { return 15; }  // 1.5×
    if percent_minted <= 75 { return 25; }  // 2.5×
    40 // 4× for 76-100%
}

fn calculate_mint_fee(amount: u64, total_minted: u64) -> u64 {
    const BASE_FEE_LAMPORTS: u64 = 1_000_000; // 0.001 SOL
    const BASE_DIVISOR: u64 = 406_000_000; // 406,000 tokens

    let base_fee = amount
        .saturating_mul(BASE_FEE_LAMPORTS)
        .saturating_div(BASE_DIVISOR);

    let multiplier = calculate_scarcity_multiplier(total_minted);

    // Multiply then divide by 10 to handle decimal multipliers
    base_fee.saturating_mul(multiplier).saturating_div(10)
}
```

---

### 💎 SCENARIO B: Higher Early Multipliers

**Best for**: Frontloading revenue while keeping later stages accessible

#### Fee Structure
```
Base Formula: (amount / 510,000) × 0.001 SOL × multiplier
```

#### Multipliers
| % Minted | Multiplier | SOL per 1000 QDUM |
|----------|------------|-------------------|
| 0-25%    | **1×**     | ~0.00196 SOL      |
| 26-50%   | **2×**     | ~0.00392 SOL      |
| 51-75%   | **3×**     | ~0.00588 SOL      |
| 76-100%  | **5×**     | ~0.0098 SOL       |

#### Cost Breakdown
| Tier | QDUM Minted | Multiplier | Fee (SOL) | Fee (USD) |
|------|-------------|------------|-----------|-----------|
| 0-25% | 30,924 | 1× | 0.061 | $9.09 |
| 26-50% | 30,924 | 2× | 0.121 | $18.18 |
| 51-75% | 30,924 | 3× | 0.182 | $27.27 |
| 76-100% | 30,924 | 5× | 0.303 | $45.45 |
| **TOTAL** | **123,695** | - | **0.667 SOL** | **$100** |

#### Advantages
✅ Simple integer multipliers (easier to implement)
✅ No fractional multipliers needed
✅ Still incentivizes early minting
✅ Predictable fee structure

#### Implementation Code
```rust
const BASE_DIVISOR: u64 = 510_000_000; // 510,000 tokens

fn calculate_scarcity_multiplier(total_minted: u64) -> u64 {
    const PUBLIC_MINT_SUPPLY: u64 = 3_092_376_853_000_000;
    let percent_minted = (total_minted as u128 * 100) / PUBLIC_MINT_SUPPLY as u128;

    if percent_minted <= 25 { return 1; }
    if percent_minted <= 50 { return 2; }
    if percent_minted <= 75 { return 3; }
    5 // 76-100%
}

fn calculate_mint_fee(amount: u64, total_minted: u64) -> u64 {
    const BASE_FEE_LAMPORTS: u64 = 1_000_000; // 0.001 SOL
    const BASE_DIVISOR: u64 = 510_000_000;

    let base_fee = amount
        .saturating_mul(BASE_FEE_LAMPORTS)
        .saturating_div(BASE_DIVISOR);

    let multiplier = calculate_scarcity_multiplier(total_minted);
    base_fee.saturating_mul(multiplier)
}
```

---

### ⚖️ SCENARIO C: Balanced Distribution

**Best for**: Maximum incentive for early minting

#### Fee Structure
```
Base Formula: (amount / 348,000) × 0.001 SOL × multiplier
```

#### Multipliers
| % Minted | Multiplier | SOL per 1000 QDUM |
|----------|------------|-------------------|
| 0-25%    | **0.5×**   | ~0.00144 SOL      |
| 26-50%   | **1×**     | ~0.00287 SOL      |
| 51-75%   | **2×**     | ~0.00575 SOL      |
| 76-100%  | **4×**     | ~0.0115 SOL       |

#### Cost Breakdown
| Tier | QDUM Minted | Multiplier | Fee (SOL) | Fee (USD) |
|------|-------------|------------|-----------|-----------|
| 0-25% | 30,924 | 0.5× | 0.044 | $6.67 |
| 26-50% | 30,924 | 1× | 0.089 | $13.33 |
| 51-75% | 30,924 | 2× | 0.178 | $26.67 |
| 76-100% | 30,924 | 4× | 0.356 | $53.33 |
| **TOTAL** | **123,695** | - | **0.667 SOL** | **$100** |

#### Advantages
✅ Cheapest early fees ($6.67 for first 25%)
✅ Strongest scarcity incentive
✅ Rewards very early adopters most
✅ Clear doubling pattern (0.5, 1, 2, 4)

#### Implementation Code
```rust
const BASE_DIVISOR: u64 = 348_000_000; // 348,000 tokens

fn calculate_scarcity_multiplier(total_minted: u64) -> u64 {
    const PUBLIC_MINT_SUPPLY: u64 = 3_092_376_853_000_000;
    let percent_minted = (total_minted as u128 * 100) / PUBLIC_MINT_SUPPLY as u128;

    // Using 10x scaling: 0.5× = 5, 1× = 10, 2× = 20, 4× = 40
    if percent_minted <= 25 { return 5; }
    if percent_minted <= 50 { return 10; }
    if percent_minted <= 75 { return 20; }
    40
}

fn calculate_mint_fee(amount: u64, total_minted: u64) -> u64 {
    const BASE_FEE_LAMPORTS: u64 = 1_000_000;
    const BASE_DIVISOR: u64 = 348_000_000;

    let base_fee = amount
        .saturating_mul(BASE_FEE_LAMPORTS)
        .saturating_div(BASE_DIVISOR);

    let multiplier = calculate_scarcity_multiplier(total_minted);
    base_fee.saturating_mul(multiplier).saturating_div(10)
}
```

---

### 🚀 SCENARIO D: Cheap Start, Expensive End

**Best for**: Mass early adoption, aggressive scarcity at end

#### Fee Structure
```
Base Formula: (amount / 417,000) × 0.001 SOL × multiplier
```

#### Multipliers
| % Minted | Multiplier | SOL per 1000 QDUM |
|----------|------------|-------------------|
| 0-25%    | **0.25×**  | ~0.0006 SOL       |
| 26-50%   | **0.75×**  | ~0.0018 SOL       |
| 51-75%   | **2×**     | ~0.0048 SOL       |
| 76-100%  | **6×**     | ~0.0144 SOL       |

#### Cost Breakdown
| Tier | QDUM Minted | Multiplier | Fee (SOL) | Fee (USD) |
|------|-------------|------------|-----------|-----------|
| 0-25% | 30,924 | 0.25× | 0.019 | $2.78 |
| 26-50% | 30,924 | 0.75× | 0.056 | $8.33 |
| 51-75% | 30,924 | 2× | 0.148 | $22.22 |
| 76-100% | 30,924 | 6× | 0.444 | $66.67 |
| **TOTAL** | **123,695** | - | **0.667 SOL** | **$100** |

#### Advantages
✅ Ultra-cheap early entry ($2.78 for first 25%)
✅ Creates massive FOMO for late minters
✅ Maximizes distribution in early phases
✅ Strong revenue from final phase

#### Implementation Code
```rust
const BASE_DIVISOR: u64 = 417_000_000; // 417,000 tokens

fn calculate_scarcity_multiplier(total_minted: u64) -> u64 {
    const PUBLIC_MINT_SUPPLY: u64 = 3_092_376_853_000_000;
    let percent_minted = (total_minted as u128 * 100) / PUBLIC_MINT_SUPPLY as u128;

    // Using 100x scaling: 0.25× = 25, 0.75× = 75, 2× = 200, 6× = 600
    // Final fee = (base × multiplier) / 100
    if percent_minted <= 25 { return 25; }
    if percent_minted <= 50 { return 75; }
    if percent_minted <= 75 { return 200; }
    600
}

fn calculate_mint_fee(amount: u64, total_minted: u64) -> u64 {
    const BASE_FEE_LAMPORTS: u64 = 1_000_000;
    const BASE_DIVISOR: u64 = 417_000_000;

    let base_fee = amount
        .saturating_mul(BASE_FEE_LAMPORTS)
        .saturating_div(BASE_DIVISOR);

    let multiplier = calculate_scarcity_multiplier(total_minted);
    base_fee.saturating_mul(multiplier).saturating_div(100)
}
```

---

## Comparison Table

| Scenario | Early Fees | Late Fees | Best For | Implementation |
|----------|------------|-----------|----------|----------------|
| **A: Progressive** | $8.57 | $45.71 | Balanced growth | Medium (0.75× needs 10x scale) |
| **B: Higher Early** | $9.09 | $45.45 | Simple, predictable | Easy (integer multipliers) |
| **C: Balanced** | $6.67 | $53.33 | Rewarding early adopters | Medium (0.5× needs 10x scale) |
| **D: Cheap Start** | $2.78 | $66.67 | Max early distribution | Medium (0.25× needs 100x scale) |

---

## Final Recommendation

### 🏆 Implement Scenario A: Progressive Multipliers

**Rationale:**
1. ✅ **Affordable** - $8.57 for early minters is very accessible
2. ✅ **Balanced** - Revenue distributed evenly across tiers
3. ✅ **Smooth growth** - No dramatic jumps that shock users
4. ✅ **Strong endgame** - 4× multiplier creates real scarcity
5. ✅ **Achievable** - $100 total per user is retail-friendly

### Implementation Steps

1. Update the on-chain Rust program (`quantdum-token/programs/quantdum-token/src/instructions/free_mint.rs`)
2. Update the CLI client code (`qdum-vault/src/solana/client.rs`)
3. Test with small amounts on devnet
4. Deploy to mainnet after validation

### Key Changes Needed

```rust
// Change from current divisor (1,000 tokens) to:
const BASE_DIVISOR: u64 = 406_000_000; // 406,000 tokens

// Change multiplier tiers from (3, 5, 8, 12, 15, 20) to:
// (7, 15, 25, 40) with 10x scaling (represents 0.75×, 1.5×, 2.5×, 4×)
```

---

## Revenue Validation

### Total Protocol Revenue Calculation
- **25,000 users** × **0.667 SOL** = **16,667 SOL**
- At $150/SOL = **$2,500,000** ✅

### Revenue by Tier (Scenario A)
| Tier | Users | SOL per User | Total SOL | USD Value |
|------|-------|--------------|-----------|-----------|
| 0-25% | 6,250 | 0.057 | 356 SOL | $53,400 |
| 26-50% | 6,250 | 0.114 | 713 SOL | $106,950 |
| 51-75% | 6,250 | 0.191 | 1,194 SOL | $179,100 |
| 76-100% | 6,250 | 0.305 | 1,906 SOL | $285,900 |

Wait, this doesn't add up to $2.5M. Let me recalculate assuming users are distributed evenly across the supply, not equally per tier...

Actually, the calculation assumes ALL users mint their full 123,695 QDUM allocation, which spans all 4 tiers. So:
- Each user pays fees in ALL 4 tiers as they mint their allocation
- Total revenue = 25,000 users × 0.667 SOL = 16,667 SOL = $2.5M ✅

---

## User Experience Impact

**At $100 per user:**
- ✅ Retail accessible (compared to current $221k)
- ✅ Comparable to NFT mint prices
- ✅ Serious enough to prevent spam
- ✅ Generates strong protocol revenue

**Gas costs**: Add ~$5-20 in SOL transaction fees, so total cost ~$105-120 per user.

---

## Alternative: If SOL price changes

| SOL Price | Required SOL/User | USD/User | Protocol Revenue |
|-----------|-------------------|----------|------------------|
| $100 | 1.00 SOL | $100 | $2.5M |
| $150 | 0.667 SOL | $100 | $2.5M |
| $200 | 0.50 SOL | $100 | $2.5M |

The **USD target of $100/user** should remain constant, with SOL amounts adjusted based on market price at launch.
