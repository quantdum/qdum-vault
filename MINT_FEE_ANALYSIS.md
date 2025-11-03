# QDUM Public Mint Fee Analysis & Scenarios

## Current Fee Structure

### Token Economics
- **Total Supply**: 4,294,967,296 QDUM (2^32)
- **Public Mint Supply**: 3,092,376,853 QDUM (72%)
- **Airdrop Reserve**: 1,202,590,443 QDUM (28%)
- **Decimals**: 6

### Current Fee Formula
```
Base Fee = (amount / 1,000 tokens) × 0.001 SOL
Final Fee = Base Fee × Scarcity Multiplier
```

### Current Scarcity Multiplier Tiers
| % Minted | Multiplier | SOL per 1000 QDUM |
|----------|------------|-------------------|
| 0-10%    | 3×         | 0.003 SOL         |
| 11-25%   | 5×         | 0.005 SOL         |
| 26-45%   | 8×         | 0.008 SOL         |
| 46-60%   | 12×        | 0.012 SOL         |
| 61-70%   | 15×        | 0.015 SOL         |
| 71-100%  | 20×        | 0.020 SOL         |

---

## Scenario Analysis: 25,000 Users Minting Through Total Supply

### Goal
Enable 25,000 users to realistically mint through the 3.09B QDUM public supply.

**Average per user**: 3,092,376,853 ÷ 25,000 = **123,695 QDUM per user**

---

## Current System Analysis

### Current Cost Per User (Current Multipliers)

Assuming each user mints their full 123,695 QDUM share:

| Tier | QDUM Range | QDUM Amount | Multiplier | Fee (SOL) |
|------|------------|-------------|------------|-----------|
| 0-10% | 0 - 309.2M | 12,369.5k | 3× | 37.11 SOL |
| 11-25% | 309.2M - 773M | 18,554.3k | 5× | 92.77 SOL |
| 26-45% | 773M - 1.39B | 24,739k | 8× | 197.91 SOL |
| 46-60% | 1.39B - 1.86B | 18,554.3k | 12× | 222.65 SOL |
| 61-70% | 1.86B - 2.16B | 12,369.5k | 15× | 185.54 SOL |
| 71-100% | 2.16B - 3.09B | 37,108.6k | 20× | 742.17 SOL |
| **TOTAL** | - | **123,695k** | - | **1,478.15 SOL** |

**Cost per user at current rates: ~1,478 SOL ($220,000+ at $150/SOL)**

This is completely unrealistic for retail users.

---

## Proposed Scenarios

### Scenario 1: AGGRESSIVE REDUCTION (Recommended for Mass Adoption)

**Target**: ~$50-100 per user to mint full allocation

#### New Multipliers
| % Minted | Old Multiplier | New Multiplier | Reduction |
|----------|----------------|----------------|-----------|
| 0-10%    | 3×             | **0.5×**       | -83%      |
| 11-25%   | 5×             | **0.75×**      | -85%      |
| 26-45%   | 8×             | **1×**         | -88%      |
| 46-60%   | 12×            | **1.5×**       | -88%      |
| 61-70%   | 15×            | **2×**         | -87%      |
| 71-100%  | 20×            | **3×**         | -85%      |

#### Cost Breakdown (123,695 QDUM per user)
| Tier | QDUM Amount | New Multiplier | Fee (SOL) |
|------|-------------|----------------|-----------|
| 0-10% | 12,369.5k | 0.5× | 6.18 SOL |
| 11-25% | 18,554.3k | 0.75× | 13.92 SOL |
| 26-45% | 24,739k | 1× | 24.74 SOL |
| 46-60% | 18,554.3k | 1.5× | 27.83 SOL |
| 61-70% | 12,369.5k | 2× | 24.74 SOL |
| 71-100% | 37,108.6k | 3× | 111.33 SOL |
| **TOTAL** | **123,695k** | - | **208.74 SOL** |

**Cost per user: ~209 SOL (~$31,350 at $150/SOL)**

Still too high. Let's go more aggressive.

---

### Scenario 2: ULTRA-LOW FEES (Mass Retail Target)

**Target**: $50-150 total cost per user for full allocation

#### New Multipliers
| % Minted | Multiplier | SOL per 1000 QDUM |
|----------|------------|-------------------|
| 0-25%    | **0.1×**   | 0.0001 SOL        |
| 26-50%   | **0.2×**   | 0.0002 SOL        |
| 51-75%   | **0.4×**   | 0.0004 SOL        |
| 76-100%  | **0.8×**   | 0.0008 SOL        |

#### Cost Breakdown (123,695 QDUM per user)
| Tier | QDUM Amount | Multiplier | Fee (SOL) |
|------|-------------|------------|-----------|
| 0-25% | 30,923.8k | 0.1× | 3.09 SOL |
| 26-50% | 30,923.8k | 0.2× | 6.18 SOL |
| 51-75% | 30,923.8k | 0.4× | 12.37 SOL |
| 76-100% | 30,923.8k | 0.8× | 24.74 SOL |
| **TOTAL** | **123,695k** | - | **46.38 SOL** |

**Cost per user: ~46 SOL (~$6,957 at $150/SOL)**

Still too expensive for mass adoption.

---

### Scenario 3: MICRO-FEES (Maximum Accessibility) ⭐ RECOMMENDED

**Target**: $10-50 total cost for full allocation

#### New Fee Formula
```
Base Fee = (amount / 10,000 tokens) × 0.001 SOL
Final Fee = Base Fee × Multiplier
```

Note: Changed divisor from 1,000 to **10,000** tokens

#### New Multipliers
| % Minted | Multiplier | SOL per 10k QDUM |
|----------|------------|------------------|
| 0-30%    | **1×**     | 0.001 SOL        |
| 31-60%   | **2×**     | 0.002 SOL        |
| 61-90%   | **3×**     | 0.003 SOL        |
| 91-100%  | **5×**     | 0.005 SOL        |

#### Cost Breakdown (123,695 QDUM per user)
| Tier | QDUM Amount | Multiplier | Fee (SOL) |
|------|-------------|------------|-----------|
| 0-30% | 37,108.6k | 1× | 3.71 SOL |
| 31-60% | 37,108.6k | 2× | 7.42 SOL |
| 61-90% | 37,108.6k | 3× | 11.13 SOL |
| 91-100% | 12,369.5k | 5× | 6.18 SOL |
| **TOTAL** | **123,695k** | - | **28.44 SOL** |

**Cost per user: ~28 SOL (~$4,266 at $150/SOL)**

Better, but still high for retail.

---

### Scenario 4: FLAT FEE (Simplest) ⭐⭐ BEST FOR UX

**Target**: Predictable, affordable fees

#### Fee Structure
```
Flat Fee = (amount / 100,000 tokens) × 0.001 SOL
```

No scarcity multiplier. Simple linear pricing.

**SOL per 100,000 QDUM = 0.001 SOL**

#### Cost for Full Allocation (123,695 QDUM)
```
Fee = 123,695 / 100,000 × 0.001 = 0.00124 SOL
```

**Cost per user: ~0.00124 SOL (~$0.19 at $150/SOL)**

This is **extremely affordable** but may not generate enough protocol revenue.

---

### Scenario 5: BALANCED APPROACH ⭐⭐⭐ OPTIMAL

**Target**: $5-20 per user, with progressive scarcity

#### New Fee Formula
```
Base Fee = (amount / 50,000 tokens) × 0.001 SOL
Final Fee = Base Fee × Multiplier
```

#### New Multipliers
| % Minted | Multiplier | Effective SOL per 50k QDUM |
|----------|------------|---------------------------|
| 0-40%    | **0.5×**   | 0.0005 SOL                |
| 41-70%   | **1×**     | 0.001 SOL                 |
| 71-90%   | **1.5×**   | 0.0015 SOL                |
| 91-100%  | **2×**     | 0.002 SOL                 |

#### Cost Breakdown (123,695 QDUM per user)
| Tier | QDUM Amount | Multiplier | Fee (SOL) |
|------|-------------|------------|-----------|
| 0-40% | 49,478.2k | 0.5× | 0.49 SOL |
| 41-70% | 37,108.6k | 1× | 0.74 SOL |
| 71-90% | 24,739k | 1.5× | 0.74 SOL |
| 91-100% | 12,369.5k | 2× | 0.49 SOL |
| **TOTAL** | **123,695k** | - | **2.46 SOL** |

**Cost per user: ~2.5 SOL (~$375 at $150/SOL)**

---

### Scenario 6: SUPER AGGRESSIVE (Mass Market) ⭐⭐⭐⭐ MOST RECOMMENDED

**Target**: $1-10 per user total

#### New Fee Formula
```
Base Fee = (amount / 500,000 tokens) × 0.001 SOL
Final Fee = Base Fee × Multiplier
```

#### New Multipliers
| % Minted | Multiplier | SOL per 500k QDUM |
|----------|------------|-------------------|
| 0-50%    | **0.5×**   | 0.0005 SOL        |
| 51-80%   | **1×**     | 0.001 SOL         |
| 81-95%   | **1.5×**   | 0.0015 SOL        |
| 96-100%  | **2×**     | 0.002 SOL         |

#### Cost Breakdown (123,695 QDUM per user)
| Tier | QDUM Amount | Multiplier | Fee (SOL) |
|------|-------------|------------|-----------|
| 0-50% | 61,847.7k | 0.5× | 0.062 SOL |
| 51-80% | 37,108.6k | 1× | 0.074 SOL |
| 81-95% | 18,554.3k | 1.5× | 0.056 SOL |
| 96-100% | 6,184.8k | 2× | 0.025 SOL |
| **TOTAL** | **123,695k** | - | **0.217 SOL** |

**Cost per user: ~0.22 SOL (~$33 at $150/SOL)**

✅ **HIGHLY ACCESSIBLE for retail users**

---

## Comparison Table

| Scenario | Total SOL/User | USD/User @ $150 | Accessibility | Revenue |
|----------|----------------|-----------------|---------------|---------|
| **Current** | 1,478 SOL | $221,700 | ❌ Impossible | $$$$$ |
| **#1 Aggressive** | 209 SOL | $31,350 | ❌ Too High | $$$$ |
| **#2 Ultra-Low** | 46 SOL | $6,957 | ⚠️ Expensive | $$$ |
| **#3 Micro-Fees** | 28 SOL | $4,266 | ⚠️ Moderate | $$ |
| **#4 Flat** | 0.00124 SOL | $0.19 | ✅ Perfect | $ (too low) |
| **#5 Balanced** | 2.5 SOL | $375 | ⚠️ Borderline | $$ |
| **#6 Super Aggressive** | 0.22 SOL | $33 | ✅✅ Excellent | $$ |

---

## Revenue Analysis

### Scenario 6 (Recommended): 0.22 SOL per user

**Total protocol revenue if all 25,000 users mint:**
- 25,000 users × 0.22 SOL = **5,500 SOL**
- At $150/SOL = **$825,000 total revenue**

**Distribution over supply tiers:**
- 0-50% (1.546B QDUM): 2,500 users × 0.062 SOL = 155 SOL
- 51-80% (927M QDUM): 7,500 users × 0.074 SOL = 555 SOL
- 81-95% (464M QDUM): 3,750 users × 0.056 SOL = 210 SOL
- 96-100% (155M QDUM): 11,250 users × 0.025 SOL = 281 SOL

---

## Implementation Recommendations

### ✅ Recommended: Scenario 6 (Super Aggressive)

**Why:**
1. **Accessible**: $33 per user is achievable for retail
2. **Fair**: Still generates $825k+ in protocol revenue
3. **Scalable**: Allows genuine mass adoption
4. **Progressive**: Maintains scarcity incentive without being punitive

### Changes Required:

#### 1. Update Base Fee Divisor
```rust
// OLD
let base_fee = amount
    .saturating_mul(BASE_FEE_LAMPORTS)
    .saturating_div(1_000_000_000); // per 1,000 tokens

// NEW
let base_fee = amount
    .saturating_mul(BASE_FEE_LAMPORTS)
    .saturating_div(500_000_000_000); // per 500,000 tokens
```

#### 2. Update Scarcity Multipliers
```rust
fn calculate_scarcity_multiplier(total_minted: u64) -> u64 {
    const PUBLIC_MINT_SUPPLY: u64 = 3_092_376_853_000_000;
    let percent_minted = (total_minted as u128 * 100) / PUBLIC_MINT_SUPPLY as u128;

    if percent_minted <= 50 { return 0.5; } // Need to use f64 or scale by 10
    if percent_minted <= 80 { return 1; }
    if percent_minted <= 95 { return 1.5; } // Need to use f64 or scale by 10
    2 // 96-100%
}
```

**Note**: Solana programs don't support f64 in arithmetic. Use integer scaling:
- Multiply by 10, then divide by 10 at the end
- 0.5× becomes 5, 1× becomes 10, 1.5× becomes 15, 2× becomes 20
- Final fee = (base_fee × multiplier) / 10

---

## Alternative: Tiered User Limits

Another approach: Limit how much each user can mint:

**Example**:
- Tier 1 (0-10k QDUM): 0.0001 SOL per 1k
- Tier 2 (10k-50k QDUM): 0.0005 SOL per 1k
- Tier 3 (50k-100k QDUM): 0.001 SOL per 1k
- Tier 4 (100k+ QDUM): 0.002 SOL per 1k

This encourages distribution across more users while keeping individual costs low.

---

## Conclusion

**For 25,000 users to realistically mint through the supply:**

🎯 **Implement Scenario 6**: Change base divisor to 500,000 tokens and use gentle multipliers (0.5×, 1×, 1.5×, 2×)

This achieves:
- ✅ $33 average cost per user (affordable)
- ✅ $825k+ protocol revenue (sustainable)
- ✅ Progressive scarcity (maintains tokenomics)
- ✅ Mass retail adoption (achievable goal)
