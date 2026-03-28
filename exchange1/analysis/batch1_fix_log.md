# Batch 1 — Fix Log

> **Date**: 2026-03-25  
> **Programs**: `lp-bonds`, `lp-bonds-evolution`

---

## Fix 1: Remove Pause Gate from Redemption

### Why
A malicious or compromised admin could permanently lock user funds by:
1. Pausing the protocol (`pause()`)
2. Calling `recover_tokens` to drain assets
3. Users cannot redeem because `redeem_bond` checked `is_paused`

This was identified as **BG-07** (lp-bonds) and **EBG-07** (evolution).

### Before
```rust
// lp-bonds: redeem_bond
pub fn redeem_bond(ctx: Context<RedeemBond>) -> Result<()> {
    require!(!ctx.accounts.config.is_paused, LpBondsError::ProtocolPaused); // ← BLOCKS USERS
    ...
}

// evolution: redeem_evolved_bond
pub fn redeem_evolved_bond(ctx: Context<RedeemEvolvedBond>) -> Result<()> {
    require!(!ctx.accounts.evolution_config.is_paused, ...); // ← BLOCKS USERS
    ...
}
```

### After
```rust
// lp-bonds: redeem_bond
/// SECURITY: Redemption is NOT pause-gated. Users must always be able to
/// withdraw their assets after the lock expires, regardless of protocol
/// pause state. This prevents admin rug-pull via pause + drain.
pub fn redeem_bond(ctx: Context<RedeemBond>) -> Result<()> {
    // No pause check — lock duration still enforced
    let custody = &ctx.accounts.position_custody;
    let current_time = Clock::get()?.unix_timestamp;
    require!(custody.is_lock_expired(current_time), LpBondsError::BondStillLocked);
    ...
}
```

### Behavioral Change
- **Before**: Paused protocol → redemption blocked → user funds locked
- **After**: Paused protocol → redemption still works → user funds always accessible after lock expires

### Files Modified
- `programs/lp-bonds/src/lib.rs` (L684-689)
- `programs/lp-bonds-evolution/src/lib.rs` (L957-965)

---

## Fix 2: Add `update_fees_and_rewards` CPI Before `collect_fees`

### Why
Orca Whirlpool requires calling `update_fees_and_rewards` before `collect_fees` to refresh the position's internal fee counters. Without this, collected fees may be stale or zero.

Identified as **BG-10** (lp-bonds) and **EBG-08** (evolution).

### Before
```rust
pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
    // CPI: collect_fees from the Whirlpool position
    whirlpool_cpi::collect_fees(...)?; // ← fees may be stale
    ...
}
```

### After
```rust
pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
    // Tick array PDA validation (defense-in-depth)
    {
        let whirlpool_state = whirlpool_cpi::Whirlpool::from_account_info(&ctx.accounts.whirlpool)?;
        // ... derive and validate tick array PDAs ...
    }

    // CPI: update_fees_and_rewards MUST be called before collect_fees
    whirlpool_cpi::update_fees_and_rewards(
        &ctx.accounts.whirlpool_program,
        &ctx.accounts.whirlpool,
        &ctx.accounts.whirlpool_position,
        &ctx.accounts.tick_array_lower,
        &ctx.accounts.tick_array_upper,
    )?;

    // CPI: collect_fees (now with fresh fee accounting)
    whirlpool_cpi::collect_fees(...)?;
    ...
}
```

### Behavioral Change
- **Before**: Fees could be zero/stale depending on when last update occurred
- **After**: Fees always refreshed immediately before collection

### Files Modified
- `programs/lp-bonds/src/whirlpool_cpi.rs` — added `UPDATE_FEES_AND_REWARDS` discriminator + CPI fn
- `programs/lp-bonds-evolution/src/whirlpool_cpi.rs` — same
- `programs/lp-bonds/src/lib.rs` (L877-920) — CPI call + tick array PDA validation
- `programs/lp-bonds-evolution/src/lib.rs` (L418-462) — same
- `programs/lp-bonds/src/lib.rs` CollectFees struct — added `tick_array_lower`, `tick_array_upper`
- `programs/lp-bonds-evolution/src/lib.rs` CollectFees struct — same

### Breaking Change
`collect_fees` now requires 2 additional accounts. SDK updated in `sdk/collectFees.ts`.

---

## Fix 3: Safety Guard on `recover_tokens`

### Why
The original `recover_tokens` had no guard against recovering active custody assets. An admin could drain any `bond_authority`-owned token account, including those backing active bonds.

Identified as **BG-09**.

### Before
```rust
pub struct RecoverTokens<'info> {
    pub admin: Signer<'info>,
    pub config: Account<'info, ProtocolConfig>,
    pub bond_authority: UncheckedAccount<'info>,
    /// Must NOT be a custody position token account. (comment only, no enforcement)
    pub source_token_account: Account<'info, TokenAccount>,
    pub admin_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
```

### After
```rust
pub struct RecoverTokens<'info> {
    pub admin: Signer<'info>,
    pub config: Account<'info, ProtocolConfig>,
    pub bond_authority: UncheckedAccount<'info>,
    /// Must have supply == 0 (bond burned)
    #[account(constraint = bond_mint.supply == 0 @ LpBondsError::InvalidBondBalance)]
    pub bond_mint: Account<'info, Mint>,
    /// Validates linkage between bond_mint and source
    #[account(
        seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
        bump = position_custody.bump,
        constraint = position_custody.bond_mint == bond_mint.key(),
    )]
    pub position_custody: Account<'info, PositionCustody>,
    pub source_token_account: Account<'info, TokenAccount>,
    pub admin_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
```

### Behavioral Change
- **Before**: Admin can recover ANY `bond_authority`-owned token account
- **After**: Admin can ONLY recover tokens for burned bonds (supply == 0) with valid custody PDA linkage

### Files Modified
- `programs/lp-bonds/src/lib.rs` RecoverTokens struct (L1506-1549)

### Breaking Change
`recover_tokens` now requires 2 additional accounts: `bond_mint` and `position_custody`.

---

## Phase 2 Hardening: Tick Array PDA Validation

### Why
The initial Batch 1 fix only validated tick array ownership (owner == WHIRLPOOL_PROGRAM_ID). An attacker could pass any Whirlpool-program-owned account (e.g., a tick array from a different pool or different tick range). PDA derivation validation ensures the exact correct tick arrays are used.

### Defense Layers
1. **Account constraint**: `tick_array.owner == WHIRLPOOL_PROGRAM_ID`
2. **Handler validation**: PDA derived from `(whirlpool, get_start_tick_index(tick, spacing))` must match provided account key

### Files Modified
- `programs/lp-bonds/src/lib.rs` collect_fees handler (L877-915)
- `programs/lp-bonds-evolution/src/lib.rs` collect_fees handler (L418-456)
