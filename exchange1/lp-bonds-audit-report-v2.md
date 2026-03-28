# LP Bonds Solana Smart Contract Security Audit Report v2

**Auditor:** Claude Opus 4.6 (Solana/Anchor Security Specialist)
**Date:** 2026-03-24
**Scope:** Full security audit of `lp-bonds` and `lp-bonds-evolution` Anchor programs, including EVM-to-Solana migration gap analysis
**Commit:** `37cfa26` (branch: `initial-implementation`)

---

## Section 1: Repo Architecture Map

### 1.1 File Tree

```
programs/
  lp-bonds/
    Cargo.toml
    src/
      lib.rs           # Main program: initialize, admin, add_liquidity_and_mint_bond, redeem_bond, oracle, verify_collateral
      state.rs         # ProtocolConfig, PositionCustody, OracleConfig, NonceAccount
      errors.rs        # LpBondsError enum (~40 error codes)
      constants.rs     # Seeds, oracle domains, tick bounds, NFT defaults
      whirlpool_cpi.rs # CPI wrappers: open_position, increase_liquidity, decrease_liquidity, collect_fees, close_position
      ed25519.rs       # Oracle attestation verification via Ed25519 precompile
      events.rs        # Event definitions

  lp-bonds-evolution/
    Cargo.toml
    src/
      lib.rs           # Evolution program: initialize_evolution, configure_level, evolve_bond, admin, nonce, authority
      state.rs         # EvolutionConfig, LevelConfig, EvolutionRecord, LayerTokenAuthority, EvolutionNonce, AuthorityWhitelist, PositionCustodyRef
      errors.rs        # EvolutionError enum (~40 error codes)
      constants.rs     # Seeds, oracle domain, MAX_BOND_LEVEL, MAX_FEE_BPS, permission bitmasks
      whirlpool_cpi.rs # CPI wrappers (identical to lp-bonds)
      ed25519.rs       # Evolution-specific oracle attestation verification
      events.rs        # Event definitions
```

### 1.2 Instruction Summary

#### lp-bonds (Level 1 Locker)

| Instruction | Parameters | Key Accounts |
|---|---|---|
| `initialize` | whirlpool, token_mint_a/b, tick_lower/upper, lock_duration | admin (Signer), config (init PDA) |
| `update_config` | whirlpool, token_mint_a/b, tick_lower/upper, lock_duration | admin (Signer), config (mut PDA) |
| `pause` | none | admin (Signer), config (mut PDA) |
| `unpause` | none | admin (Signer), config (mut PDA) |
| `propose_admin` | new_admin | admin (Signer), config (mut PDA) |
| `accept_admin` | none | new_admin (Signer), config (mut PDA) |
| `add_liquidity_and_mint_bond` | liquidity_amount, token_max_a/b, tick_current, oracle_nonce, oracle_timestamp | user, config, oracle_config, nonce_account, bond_mint, whirlpool, position_custody, etc. |
| `redeem_bond` | none | user, config, bond_mint, position_custody, custody_position_token_account |
| `initialize_oracle` | oracle_authority | admin (Signer), config, oracle_config (init PDA) |
| `update_oracle_authority` | new_authority | admin (Signer), oracle_config (mut PDA) |
| `initialize_nonce` | none | user (Signer), nonce_account (init PDA) |
| `verify_collateral` | amount0/1, liquidity, tick_current, nonce, timestamp | sender, oracle_config, config, nonce_account, bond_mint, position_custody, whirlpool |

#### lp-bonds-evolution

| Instruction | Parameters | Key Accounts |
|---|---|---|
| `initialize_evolution` | treasury, oracle_authority, lp_bonds_program_id | admin (Signer), evolution_config (init PDA) |
| `initialize_layer_authority` | none | admin, evolution_config, layer_token_authority (init PDA) |
| `create_layer_token_mint` | decimals | admin, evolution_config, layer_token_authority, layer_token_mint (init) |
| `configure_level` | level_id, tick_lower/upper, required_amount_a/b, fee_bps, lock_duration, multiplier, is_active | admin, evolution_config, level_config (init_if_needed PDA), whirlpool, mints |
| `pause_evolution` / `unpause_evolution` | none | admin, evolution_config |
| `update_treasury` | new_treasury | admin, evolution_config |
| `update_oracle` | new_oracle | admin, evolution_config |
| `propose_admin` / `accept_admin` | new_admin / none | admin/new_admin, evolution_config |
| `add_authority` | permissions | admin, evolution_config, authority, authority_whitelist (init PDA) |
| `remove_authority` | none | admin, evolution_config, authority_whitelist (close) |
| `initialize_evolution_nonce` | none | user, evolution_nonce (init PDA) |
| `evolve_bond` | target_level, amount_a/b, liquidity_amount, token_max_a/b, nonce, tick_current, oracle_timestamp | user, evolution_config, level_config, evolution_nonce, source_bond_mint, source_custody, target_bond_mint, whirlpool, etc. + remaining_accounts[0..3] for tick arrays and vaults |

### 1.3 Account Structures with Constraints Summary

#### ProtocolConfig (PDA: `["config"]`)
- admin, pending_admin, allowlisted_whirlpool, token_mint_a/b, tick_lower/upper_index, lock_duration, bond_counter, is_paused, bump

#### PositionCustody (PDA: `["position_custody", bond_mint]`)
- bond_mint, position_mint, whirlpool, tick_lower/upper_index, liquidity, depositor, created_at, level, lock_duration, is_evolved, evolved_from, bump, position_bump

#### OracleConfig (PDA: `["oracle_config"]`)
- oracle_authority, admin, enabled, bump

#### NonceAccount (PDA: `["nonce", user]`)
- user, current_nonce, bump

#### EvolutionConfig (PDA: `["evolution_config"]`)
- admin, pending_admin, treasury, oracle_authority, lp_bonds_program_id, is_paused, evolution_counter, bump

#### LevelConfig (PDA: `["level_config", level_id]`)
- level_id, whirlpool, token_mint_a/b, layer_token_mint, tick_lower/upper, required_amount_a/b, fee_bps, lock_duration, multiplier, is_active, bump

#### EvolutionRecord (PDA: `["evolution_record", source_bond_mint]`)
- source_bond_mint, source_level, target_bond_mint, target_level, evolver, evolved_at, amount_a/b, liquidity, fee_paid, bump

---

## Section 2: Confirmed Issues (9 Known Issues)

### Issue 1: validate_source_custody() level==0 OR level==255 treated as level 1

**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 829-833
**Severity:** [MEDIUM]

**Description:** The `validate_source_custody` function silently maps `level == 0` or `level == 255` to `level 1`:

```rust
let source_level: u8 = if custody_ref.level == 0 || custody_ref.level == 255 {
    1
} else {
    custody_ref.level
};
```

**Attack Scenario:** An attacker who has a corrupted or uninitialized custody account (level byte = 0) that passes the other validation checks (PDA derivation, owner check, bond_mint match) could have it treated as a Level 1 bond and evolve it to Level 2. While `is_evolved == false` check and PDA derivation provide defense-in-depth, the level coercion masks data integrity issues. The `level == 255` case is also problematic -- a bond at level 255 should be rejected, not treated as level 1.

**Fix Recommendation:**
```rust
// Replace lines 829-833 with:
let source_level = custody_ref.level;
require!(
    source_level >= MIN_BOND_LEVEL && source_level <= MAX_BOND_LEVEL,
    EvolutionError::InvalidBondLevel
);
```

---

### Issue 2: evolve_bond has no _numberOfBonds loop (feature gap vs EVM batch minting)

**File:** `programs/lp-bonds-evolution/src/lib.rs`, line 304 (`evolve_bond`)
**File:** `programs/lp-bonds/src/lib.rs`, line 207 (`add_liquidity_and_mint_bond`)
**Severity:** [LOW] (Feature gap, not a vulnerability)

**Description:** The EVM `LiquidityBondLockerV3.lockPosition()` (line 179) accepts `_numberOfBonds` and loops to create multiple bonds in a single transaction. The Solana implementation only supports one bond per instruction invocation.

**Attack Scenario:** No security attack. This is a functional parity gap. On Solana, the 1232-byte transaction size limit and ~200k compute unit budget make batch minting in a single transaction infeasible regardless (each Ed25519 instruction alone is ~350 bytes).

**Fix Recommendation:** Document as intentional design difference. If batch minting is needed, implement via transaction versioning with Address Lookup Tables, or accept that users call multiple transactions. No code change needed for security.

---

### Issue 3: ProtocolConfig stores a SINGLE allowedPool

**File:** `programs/lp-bonds/src/state.rs`, line 17 (`allowlisted_whirlpool`)
**Severity:** [MEDIUM] (Architectural limitation)

**Description:** The EVM `LiquidityBondLockerV3` stores N bond configurations via `mapping(uint256 => Bond) public bonds`, supporting multiple token pairs and whirlpools simultaneously. The Solana `ProtocolConfig` only stores a single `allowlisted_whirlpool`.

The evolution program partially addresses this via `LevelConfig` accounts (one per level), but the L1 locker is limited to one whirlpool.

**Attack Scenario:** No direct attack. Operational limitation -- deploying multiple pairs requires multiple program deployments or config migration (which invalidates existing bonds if `update_config` changes the whirlpool).

**Fix Recommendation:** To support N whirlpools at L1, refactor `ProtocolConfig` to not store pool-specific data, and introduce a `BondConfig` PDA pattern similar to the evolution program's `LevelConfig`:
```rust
#[account]
pub struct BondConfig {
    pub bond_id: u64,
    pub whirlpool: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub lock_duration: i64,
    pub is_active: bool,
    pub bump: u8,
}
// PDA: ["bond_config", bond_id.to_le_bytes()]
```

---

### Issue 4: collect_fees CPI exists but no instruction calls it

**File:** `programs/lp-bonds/src/whirlpool_cpi.rs`, lines 400-450 (`collect_fees` function)
**File:** `programs/lp-bonds-evolution/src/whirlpool_cpi.rs`, lines 400-450 (identical)
**Severity:** [HIGH]

**Description:** Both programs define `collect_fees` CPI wrappers, but no instruction in either `lib.rs` invokes them. Searching for `collect_fees` calls in `lib.rs` yields zero results. Orca Whirlpool positions accumulate trading fees over time; without a `collect_fees` instruction, these fees are permanently locked.

**Attack Scenario:** No direct attack, but economically severe. Users' LP positions accumulate fees that cannot be collected. This represents permanent loss of yield for bond holders. The position custody PDA holds the position NFT, and only the custody PDA can authorize fee collection (as position authority). Since no instruction invokes `collect_fees` through the custody PDA, fees are irrecoverable.

**Fix Recommendation:** Add a `collect_fees` instruction to the lp-bonds program:
```rust
pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
    let custody = &ctx.accounts.position_custody;
    let bond_mint_key = ctx.accounts.bond_mint.key();
    let custody_seeds: &[&[u8]] = &[
        POSITION_CUSTODY_SEED,
        bond_mint_key.as_ref(),
        &[custody.bump],
    ];
    let signer_seeds = &[custody_seeds];

    whirlpool_cpi::collect_fees(
        &ctx.accounts.whirlpool_program.to_account_info(),
        &ctx.accounts.whirlpool.to_account_info(),
        &ctx.accounts.position_custody.to_account_info(),
        &ctx.accounts.whirlpool_position.to_account_info(),
        &ctx.accounts.custody_position_token_account.to_account_info(),
        &ctx.accounts.fee_collector_token_a.to_account_info(),
        &ctx.accounts.fee_collector_token_b.to_account_info(),
        &ctx.accounts.token_vault_a.to_account_info(),
        &ctx.accounts.token_vault_b.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        signer_seeds,
    )?;
    Ok(())
}
```

Decide whether fees go to the bond holder, the protocol treasury, or are split. The EVM contracts do not explicitly surface fee collection either, so this may be by design -- but on Solana, fees in Whirlpool positions can only be collected by the position authority (custody PDA), making this critical.

---

### Issue 5: redeem_bond exists in lp-bonds but is incomplete (no decrease_liquidity/close_position)

**File:** `programs/lp-bonds/src/lib.rs`, lines 683-737 (`redeem_bond`)
**Severity:** [HIGH]

**Description:** The `redeem_bond` instruction exists and:
1. Checks pause status
2. Validates lock expiry
3. Burns the bond NFT
4. Transfers the position NFT from custody back to the user

However, it does NOT:
- Call `decrease_liquidity` to withdraw tokens from the position
- Call `close_position` to close the Whirlpool position
- Close the `PositionCustody` PDA to reclaim rent

The evolution program has NO `redeem_bond` instruction at all. Evolved bonds (Level 2-4) can never be redeemed.

**Attack Scenario:** For L1 bonds: After redemption, the user gets the position NFT but the `PositionCustody` account remains allocated (rent leak). The user must manually interact with the Whirlpool program to decrease liquidity and close the position -- which is acceptable but suboptimal UX.

For L2-4 bonds: These bonds have NO redemption path. The position NFT is held by the evolution program's `layer_token_authority` PDA, and no instruction transfers it back. Liquidity is permanently locked.

**Fix Recommendation:**
1. For lp-bonds `redeem_bond`: Add `close = user` to the `position_custody` account constraint in `RedeemBond` struct to reclaim rent. The current approach of returning just the position NFT is acceptable if users are expected to interact with Whirlpool directly.

2. For lp-bonds-evolution: Add a `redeem_evolved_bond` instruction that mirrors the L1 pattern (burn bond, transfer position NFT to user, close custody).

---

### Issue 6: LPBondsExchange has NO Solana equivalent

**Severity:** [HIGH] (Feature gap)

**Description:** The EVM `LPBondsExchange` contract (`contracts/tokenization/LPBondsExchange.sol`) allows users to exchange LP bond NFTs for ERC20 tokens. There is no corresponding program or instruction in the Solana codebase.

**Attack Scenario:** No direct attack. Users who need to convert bonds to fungible tokens have no on-chain mechanism to do so. This may block ecosystem integrations that depend on tokenized bond positions.

**Fix Recommendation:** Implement a separate `lp-bonds-exchange` program or add exchange instructions to the existing programs. This requires design decisions about:
- Which ERC20 equivalent (SPL token) bonds are exchanged for
- Exchange rate mechanism (oracle-based or admin-set)
- MultiSig custody pattern for exchange inventory

---

### Issue 7: No emergency recovery instructions

**Severity:** [MEDIUM]

**Description:** The EVM contracts have `recoverETH`, `recoverERC20`, and `recoverERC721` functions (found in both `LiquidityBondLockerV3.sol` line 420 and `LiquidityBondsEvolution.sol` line 444). The Solana programs have NO equivalent recovery mechanism.

If tokens are accidentally sent to program-controlled PDAs, or if a bug causes tokens to become stuck, there is no admin-gated recovery path.

**Attack Scenario:** No direct attack. Risk scenario: tokens accidentally deposited to program ATAs, orphaned position NFTs after failed transactions, or dust amounts remaining after evolution fee calculations. These assets become permanently inaccessible.

**Fix Recommendation:** Add admin-gated recovery instructions:
```rust
pub fn recover_tokens(ctx: Context<RecoverTokens>, amount: u64) -> Result<()> {
    // Admin-only, transfers tokens from program-owned ATA to admin
    // MUST NOT allow recovering active position NFTs or bond-backing tokens
}
```
Include safeguards: do not allow recovery of tokens from active custody accounts, and require time-locked admin approval for large amounts.

---

### Issue 8: No operator whitelist on bond NFT transfers

**Severity:** [LOW]

**Description:** The EVM `LiquidityBonds.sol` uses an `OperatorRegistry` pattern (lines 70, 92, 102, 112) that restricts which addresses can receive or operate on bond NFTs via `_beforeTokenTransfer` and `validateApprove` modifiers. The Solana bond NFTs have no such restriction -- they are freely transferable standard SPL tokens.

**Attack Scenario:** Bond NFTs can be sold or transferred to any address, which may violate compliance requirements or enable wash trading. The `bond_authority` PDA has `freeze_authority` on the bond mint (line 1020), which could theoretically be used to freeze specific token accounts, but there is no instruction that exercises this capability.

**Fix Recommendation:** If operator restrictions are required, implement a `freeze_bond` instruction that uses the `freeze_authority` to freeze non-whitelisted token accounts. Alternatively, use Token-2022 transfer hooks for automatic validation. For most DeFi use cases, free transferability is acceptable and even desirable.

---

### Issue 9: On-chain SVG metadata missing

**File:** `programs/lp-bonds/src/constants.rs`, lines 59-61
**Severity:** [LOW] (Feature gap)

**Description:** The EVM `LiquidityBonds.sol` generates on-chain SVG metadata via `tokenURI()` that renders bond details (position, amounts, etc.) as an SVG image using Base64 encoding. The Solana implementation uses a static URI base:
```rust
pub const BOND_NFT_URI_BASE: &str = "https://api.lpbonds.io/metadata/";
```

Bond metadata is served from a centralized API endpoint rather than being computed on-chain.

**Attack Scenario:** No security attack. The centralized metadata server is a single point of failure -- if it goes down, NFT marketplaces cannot display bond metadata. Also, metadata can be changed server-side without on-chain proof.

**Fix Recommendation:** Either:
1. Accept the off-chain metadata approach (simpler, cheaper CU-wise)
2. Use Metaplex Token Metadata with on-chain fields for critical attributes (level, whirlpool, liquidity amount)
3. Store essential metadata in the `PositionCustody` PDA (already done) and use it as the source of truth, with off-chain rendering

---

## Section 3: New Security Findings

### [CRITICAL]

#### C-1: Treasury token account mint not verified in evolve_bond

**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 1405-1409
```rust
#[account(
    mut,
    constraint = treasury_token_account.owner == evolution_config.treasury @ EvolutionError::InvalidTokenOwner,
)]
pub treasury_token_account: Box<Account<'info, TokenAccount>>,
```

**Description:** The `treasury_token_account` is only validated by `owner == evolution_config.treasury`. There is NO constraint that its `mint` matches `token_mint_a` (the token from which fees are deducted). An attacker could pass a treasury token account for a different mint, and the fee transfer (STEP 4, line 514-526) would fail at the SPL Token level (mint mismatch) -- but this is relying on an implicit external program check rather than an explicit constraint.

**Attack Scenario:** While SPL Token would reject the transfer if mints mismatch, this is defense through an external program rather than an explicit constraint. If the treasury holds a token account for the same mint as `token_mint_a` but with a different decimals interpretation (e.g., Token-2022), the transfer could succeed with unexpected amounts. More critically, if `fee == 0` (fee_bps set to 0), the transfer is skipped entirely, meaning a wrong-mint treasury account silently passes validation for all future calls.

**Fix Recommendation:**
```rust
#[account(
    mut,
    constraint = treasury_token_account.owner == evolution_config.treasury @ EvolutionError::InvalidTokenOwner,
    constraint = treasury_token_account.mint == token_mint_a.key() @ EvolutionError::InvalidTokenMint,
)]
pub treasury_token_account: Box<Account<'info, TokenAccount>>,
```

---

#### C-2: Oracle authority can be set to Pubkey::default() (zero address)

**File:** `programs/lp-bonds/src/lib.rs`, lines 744-761 (`initialize_oracle`)
**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 187-203 (`update_oracle`)

**Description:** Neither `initialize_oracle` nor `update_oracle_authority` (in either program) validates that the new oracle authority is not `Pubkey::default()`. If set to `Pubkey::default()`, an attacker could construct an Ed25519 instruction with an all-zero public key. The Ed25519 precompile would verify the signature against the zero key, and if the attacker can produce a valid signature for the zero key (which is a known weak key in Ed25519), all oracle verification is bypassed.

**Attack Scenario:** Admin accidentally (or maliciously) sets `oracle_authority` to `Pubkey::default()`. In Ed25519, the zero public key has a known algebraic structure -- while standard implementations reject it, the Solana precompile behavior for the zero key should not be relied upon for security.

**Fix Recommendation:**
```rust
// In initialize_oracle:
require!(oracle_authority != Pubkey::default(), LpBondsError::InvalidOracleAuthority);

// In update_oracle_authority:
require!(new_authority != Pubkey::default(), LpBondsError::InvalidOracleAuthority);

// In evolution update_oracle:
require!(new_oracle != Pubkey::default(), EvolutionError::InvalidEvolutionOracle);
```

---

### [HIGH]

#### H-1: Evolution program fees may remain stuck in program_token_a_account

**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 477-531 (STEPS 2-5)

**Description:** Token flow in `evolve_bond`:
- STEP 2: User transfers `amount_a` to `program_token_a_account` (line 478-488)
- STEP 3: Mint `amount_b` layer tokens to `program_token_b_account` (line 498-509)
- STEP 4: Transfer `fee` from `program_token_a_account` to treasury (lines 512-526)
- STEP 5: `deposit_amount_a = amount_a - fee` (line 529-531)
- STEP 8: `increase_liquidity` with `token_max_a` as max, drawing from `program_token_a_account`

The issue is that `token_max_a` (the oracle-signed slippage parameter) may be LESS than `deposit_amount_a`. The Whirlpool CPI will only draw up to the actual required amount based on current price. Any remainder (`deposit_amount_a - actual_amount_used`) stays in `program_token_a_account` with no mechanism to return it to the user or treasury.

Similarly, `program_token_b_account` may have leftover layer tokens if the Whirlpool CPI uses less than `amount_b`.

**Attack Scenario:** Over many evolutions, dust amounts accumulate in `program_token_a_account` and `program_token_b_account`. Since these accounts are ATAs owned by `layer_token_authority` PDA, and no instruction exists to sweep them, these tokens are permanently locked. With high-value tokens, this could represent significant trapped value.

**Fix Recommendation:** After the `increase_liquidity` CPI, add logic to return remaining tokens:
```rust
// After STEP 8:
let remaining_a = program_token_a_account.reload()?.amount;
if remaining_a > 0 {
    token::transfer(
        CpiContext::new_with_signer(...),
        remaining_a,
    )?; // back to user
}
// Same for program_token_b_account
```

---

#### H-2: PositionCustody PDA not closed on redeem_bond -- rent leak and stale state

**File:** `programs/lp-bonds/src/lib.rs`, lines 1191-1242 (`RedeemBond` struct)

**Description:** The `redeem_bond` instruction burns the bond NFT and transfers the position NFT to the user, but the `PositionCustody` account is NOT closed. The `position_custody` field in `RedeemBond` is `mut` but lacks `close = user`.

Each `PositionCustody` account costs rent (~0.003 SOL for ~300 bytes). Over time, these accumulate as irrecoverable rent.

More critically, the stale `PositionCustody` PDA still exists after redemption. If a redeemed bond's mint is somehow reused (theoretically impossible with burned mint, but defense-in-depth matters), the stale custody record could be referenced.

**Attack Scenario:** Rent accumulation over time. Each bond creates a ~300-byte custody account. At 100,000 bonds, this is ~300 SOL in permanently locked rent.

**Fix Recommendation:**
```rust
#[account(
    mut,
    close = user,  // <-- Add this
    seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
    bump = position_custody.bump,
    constraint = position_custody.bond_mint == bond_mint.key() @ LpBondsError::InvalidCustodyBondMint,
)]
pub position_custody: Account<'info, PositionCustody>,
```

---

#### H-3: No redeem instruction for evolved bonds (L2-L4)

**File:** `programs/lp-bonds-evolution/src/lib.rs` (entire file -- no redeem instruction exists)

**Description:** The evolution program creates `PositionCustody` accounts and stores position NFTs in ATAs owned by `layer_token_authority`, but provides no instruction to return positions to users. Users who evolve bonds to L2-L4 can never redeem them.

The custody's position token account is owned by `layer_token_authority` PDA, not the individual `position_custody` PDA. This means even if a redeem instruction were added, it would need to use `layer_token_authority` as the signer, not the custody PDA.

**Attack Scenario:** All liquidity deposited via evolution is permanently locked. Users pay fees and deposit tokens but can never recover the underlying Whirlpool position.

**Fix Recommendation:** Add a `redeem_evolved_bond` instruction using the `layer_token_authority` PDA as signer to transfer the position NFT from custody to the user, burn the bond NFT, and close the custody account.

---

#### H-4: Verify_collateral does not check caller owns the bond

**File:** `programs/lp-bonds/src/lib.rs`, lines 808-928 (`verify_collateral`)

**Description:** The `verify_collateral` instruction does not verify that the `sender` actually owns the bond NFT. Any signer can call `verify_collateral` for any bond_mint, as long as they have a valid oracle signature and nonce. This emits `CollateralVerified` events for bonds the caller does not own.

**Attack Scenario:** An attacker could:
1. Call `verify_collateral` for any bond_mint they do not own
2. Consume nonces (griefing their own nonce account)
3. Emit misleading `CollateralVerified` events that could confuse off-chain indexers

While the attacker burns their own nonces (not the bond owner's), the event emission could have off-chain trust implications.

**Fix Recommendation:** Add a bond ownership check:
```rust
// Add to VerifyCollateral struct:
#[account(
    constraint = sender_bond_account.owner == sender.key() @ LpBondsError::InvalidTokenOwner,
    constraint = sender_bond_account.mint == bond_mint.key() @ LpBondsError::InvalidBondMint,
    constraint = sender_bond_account.amount == 1 @ LpBondsError::InvalidBondBalance,
)]
pub sender_bond_account: Account<'info, TokenAccount>,
```

---

### [MEDIUM]

#### M-1: OracleConfig.enabled not checked in verify_collateral nonce path

**File:** `programs/lp-bonds/src/lib.rs`, line 863

**Description:** While `verify_collateral` does check `oracle_config.enabled` (line 863), the check happens AFTER the nonce validation setup but before the actual verification call. This is correct -- however, there is a subtlety: if the oracle is disabled between when the user obtains the signature and when they submit the transaction, the transaction fails but the nonce is NOT consumed (because the error happens before nonce commitment at line 906). This is correct behavior.

However, in the evolution program, the oracle enabled check is NOT present at all. The `EvolutionConfig` stores `oracle_authority` directly (not a separate `OracleConfig`), and there is no `enabled` flag checked before oracle verification in `validate_oracle_and_nonce`.

**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 981-1036 (`validate_oracle_and_nonce`)

**Attack Scenario:** The evolution oracle cannot be disabled without changing the authority to an invalid key. This means there is no clean way to temporarily disable evolution oracle verification for maintenance.

**Fix Recommendation:** Add an `oracle_enabled` field to `EvolutionConfig` and check it in `validate_oracle_and_nonce`:
```rust
require!(ctx.accounts.evolution_config.oracle_enabled, EvolutionError::OracleNotEnabled);
```

---

#### M-2: LevelConfig.calculate_fee silent u128-to-u64 truncation

**File:** `programs/lp-bonds-evolution/src/state.rs`, lines 97-106

```rust
pub fn calculate_fee(&self, amount: u64) -> Result<u64> {
    let product = (amount as u128)
        .checked_mul(self.fee_bps as u128)
        .ok_or(error!(EvolutionError::ArithmeticOverflow))?;
    let fee = product
        .checked_div(10000)
        .ok_or(error!(EvolutionError::ArithmeticOverflow))?;
    Ok(fee as u64)  // <-- potential truncation
}
```

**Description:** The `fee as u64` cast could silently truncate if `fee > u64::MAX`. In practice, since `amount` is u64 and `fee_bps <= 5000` (`MAX_FEE_BPS`), the maximum fee is `u64::MAX * 5000 / 10000 = u64::MAX / 2`, which fits in u64. So this is safe given the current `MAX_FEE_BPS` cap.

**Attack Scenario:** No practical attack with current `MAX_FEE_BPS = 5000`. However, if `MAX_FEE_BPS` were raised above 10000 (no constraint prevents this at the constant level), the result could truncate.

**Fix Recommendation:** Add an explicit bounds check:
```rust
let fee = product.checked_div(10000).ok_or(...)?;
require!(fee <= u64::MAX as u128, EvolutionError::ArithmeticOverflow);
Ok(fee as u64)
```

---

#### M-3: AuthorityWhitelist permissions not checked in configure_level

**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 1114-1144 (`ConfigureLevel` struct)

**Description:** The `configure_level` instruction only checks that `admin.key() == evolution_config.admin`. Whitelisted authorities with `PERM_CONFIGURE_LEVELS` permission have no way to call this instruction -- the `AuthorityWhitelist` infrastructure exists but is never checked by any instruction that would use those permissions.

**Attack Scenario:** The `AuthorityWhitelist` system is dead code from a functional perspective. Authorities can be added and removed, but no instruction checks their permissions. This means the permission system provides a false sense of security -- operators who believe they have delegated `PERM_CONFIGURE_LEVELS` to a sub-admin have actually done nothing.

**Fix Recommendation:** Either:
1. Add an alternative `configure_level_delegated` instruction that accepts an authority whitelist account and checks `PERM_CONFIGURE_LEVELS` permission
2. Remove the `AuthorityWhitelist` system entirely to avoid confusion

---

#### M-4: source_custody marked as `mut` but never modified

**File:** `programs/lp-bonds-evolution/src/lib.rs`, line 1328

```rust
#[account(mut)]
pub source_custody: UncheckedAccount<'info>,
```

**Description:** The `source_custody` account in `EvolveBond` is marked `mut` but is never written to. After `evolve_bond`, the source bond is burned but the source custody PDA remains. Since the source bond's mint has supply=0, the custody is effectively orphaned but still occupies rent.

**Attack Scenario:** Rent leak. Each evolution orphans a source custody account. With no instruction to close these accounts (they are owned by either the base lp-bonds program or the evolution program), rent is permanently locked.

**Fix Recommendation:**
1. Remove `mut` from `source_custody` since it's read-only
2. Add a separate admin instruction to close orphaned custody accounts and reclaim rent, or add `close` logic to `evolve_bond` (requires the source custody to be an `Account` rather than `UncheckedAccount`, which conflicts with the cross-program ownership model)

---

#### M-5: Stale oracle timestamp edge case with Solana clock skew

**File:** `programs/lp-bonds/src/lib.rs`, lines 253-256
**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 1007-1011

```rust
let age = now.checked_sub(oracle_timestamp).ok_or(ArithmeticOverflow)?;
require!(age >= 0, OracleTimestampFuture);
require!(age <= MAX_ORACLE_STALENESS_SECONDS, OracleTimestampStale);
```

**Description:** The `checked_sub` returns `None` (causing `ArithmeticOverflow`) if `oracle_timestamp > now`, which effectively rejects future timestamps. The `age >= 0` check is redundant since `checked_sub` of two `i64` values only underflows for extreme values.

However, there is a subtle issue: `Clock::get()?.unix_timestamp` on Solana can lag behind real-world time by several seconds. If the oracle server's clock is slightly ahead of the Solana cluster's clock, a valid oracle signature could be rejected as "future" when it's actually current.

**Attack Scenario:** Legitimate oracle signatures rejected during periods of Solana clock lag. This is a liveness issue, not a security issue. The 60-second staleness window provides reasonable buffer.

**Fix Recommendation:** Allow a small future tolerance (e.g., 5 seconds):
```rust
let FUTURE_TOLERANCE: i64 = 5;
let adjusted_now = now + FUTURE_TOLERANCE;
let age = adjusted_now.checked_sub(oracle_timestamp).ok_or(ArithmeticOverflow)?;
require!(age >= 0, OracleTimestampFuture);
require!(age <= MAX_ORACLE_STALENESS_SECONDS + FUTURE_TOLERANCE, OracleTimestampStale);
```

---

#### M-6: position_custody in RedeemBond does not verify custody is for the correct program

**File:** `programs/lp-bonds/src/lib.rs`, lines 1223-1229

**Description:** The `RedeemBond` struct validates `position_custody` via PDA seeds `[POSITION_CUSTODY_SEED, bond_mint]`, which binds it to the lp-bonds program. However, the evolution program also creates `PositionCustody` accounts with the SAME seed pattern but under its own program ID. Since Anchor's `seeds` constraint derives PDAs under the current program ID, this is safe -- but it means L2-L4 bonds cannot be redeemed via the L1 program's `redeem_bond` (different program ID = different PDA). This is correct security behavior.

**Fix Recommendation:** No code change needed. Document that `redeem_bond` is L1-only. A separate redemption instruction is needed for evolved bonds (see H-3).

---

### [LOW]

#### L-1: No Token-2022 support

**Description:** Both programs use `anchor_spl::token::Token` (SPL Token program) exclusively. Token-2022 (Token Extensions) mints/accounts are not supported. If `token_mint_a` or `token_mint_b` is a Token-2022 token, all CPI calls will fail.

**Fix Recommendation:** Add Token-2022 support via `token::TokenInterface` if any configured whirlpool uses Token-2022 tokens.

---

#### L-2: WHIRLPOOL_PROGRAM_ID hardcoded with no upgrade path

**File:** `programs/lp-bonds/src/whirlpool_cpi.rs`, line 10-11
**File:** `programs/lp-bonds-evolution/src/whirlpool_cpi.rs`, line 10-11

```rust
pub const WHIRLPOOL_PROGRAM_ID: Pubkey =
    pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
```

**Description:** The Orca Whirlpool program ID is hardcoded as a constant. If Orca upgrades their program to a new address, or deploys a v2, these programs require redeployment.

**Fix Recommendation:** Store the Whirlpool program ID in the config accounts. This allows admin-gated updates without redeployment.

---

#### L-3: Bond counter overflow at u64::MAX

**File:** `programs/lp-bonds/src/lib.rs`, lines 625-627

**Description:** `bond_counter` uses `checked_add` so it won't silently overflow, but at `u64::MAX` bonds, all minting stops permanently. This is practically impossible (18 quintillion bonds) but worth noting.

**Fix Recommendation:** No change needed. The checked arithmetic correctly prevents overflow.

---

#### L-4: Whirlpool deserialization does not verify discriminator

**File:** `programs/lp-bonds/src/whirlpool_cpi.rs`, lines 43-58

```rust
pub fn from_account_info(account: &AccountInfo) -> Result<Self> {
    require_keys_eq!(*account.owner, WHIRLPOOL_PROGRAM_ID, ...);
    let data = account.try_borrow_data()?;
    let whirlpool_data = &data[8..]; // Skip discriminator
    Self::deserialize(&mut &whirlpool_data[..])
}
```

**Description:** The deserialization skips the 8-byte discriminator but does not verify it matches the expected Whirlpool discriminator. Any account owned by the Whirlpool program with sufficient data length could be deserialized. Since the owner check ensures it's a Whirlpool program account, and the PDA/address constraints further restrict which account is passed, this is low risk but not zero risk.

**Fix Recommendation:** Add discriminator verification:
```rust
const WHIRLPOOL_DISCRIMINATOR: [u8; 8] = [63, 149, 209, 12, 225, 128, 99, 9]; // sha256("account:Whirlpool")[..8]
require!(&data[..8] == &WHIRLPOOL_DISCRIMINATOR, ErrorCode::AccountDiscriminatorMismatch);
```

---

#### L-5: remaining_accounts[4+] silently ignored in evolve_bond

**File:** `programs/lp-bonds-evolution/src/lib.rs`, lines 317-323

```rust
require!(ctx.remaining_accounts.len() >= 4, ...);
let tick_array_lower = &ctx.remaining_accounts[0];
// ... uses [0], [1], [2], [3] only
```

**Description:** Extra remaining accounts beyond index 3 are silently accepted. While this has no security impact, it wastes transaction space and could confuse integrators.

**Fix Recommendation:** Add a strict length check:
```rust
require!(ctx.remaining_accounts.len() == 4, EvolutionError::InsufficientRemainingAccounts);
```

---

#### L-6: Nonce and EvolutionRecord accounts cannot be closed

**Description:** `NonceAccount`, `EvolutionNonce`, and `EvolutionRecord` accounts are created but never closed. Over time, these accumulate rent:
- `NonceAccount`: one per user per program (~2 accounts per user)
- `EvolutionRecord`: one per evolution event

No instructions exist to close these accounts and reclaim rent.

**Fix Recommendation:** Add admin-gated close instructions, or allow users to close their own nonce accounts if they no longer need the protocol.

---

## Section 4: Missing Features (EVM vs Solana)

| EVM Feature | EVM Contract | Solana Status | Notes |
|---|---|---|---|
| N bond configs (`mapping(uint256=>Bond)`) | LiquidityBondLockerV3 | PARTIAL | Single `allowlisted_whirlpool` in L1; `LevelConfig` per level in evolution |
| Batch minting (`_numberOfBonds` loop) | LiquidityBondLockerV3 | MISSING | One bond per tx; Solana tx size limit makes batching infeasible |
| ECDSA signature verification | LiquidityBondLockerV3 | IMPLEMENTED | Uses Ed25519 precompile (Solana equivalent) |
| Bond redemption | LiquidityBondLockerV3 | PARTIAL | L1 `redeem_bond` exists but no decrease_liquidity/close_position; L2-4 NO redemption |
| Fee collection | Uniswap V3 (implicit) | MISSING | `collect_fees` CPI wrapper exists but no instruction calls it |
| LPBondsExchange (NFT-to-ERC20) | LPBondsExchange | MISSING | No equivalent program |
| On-chain SVG tokenURI | LiquidityBonds | MISSING | Static URI base instead |
| Operator whitelist on transfers | LiquidityBonds (OperatorRegistry) | MISSING | Bonds freely transferable |
| recoverETH/ERC20/ERC721 | Both lockers + evolution | MISSING | No admin recovery |
| MultiSig custody | LiquidityBondLockerV3 | MISSING | No multi-sig; single admin |
| ReentrancyGuard | All EVM contracts | N/A | Solana's execution model prevents reentrancy |
| Pausable | All EVM contracts | IMPLEMENTED | Both programs have pause/unpause |
| Two-step admin transfer | All EVM contracts | IMPLEMENTED | Both programs have propose/accept pattern |
| Layer struct (evolution layers) | LiquidityBondsEvolution | IMPLEMENTED | Via `LevelConfig` accounts |
| Authority whitelist | Not in EVM | IMPLEMENTED (dead code) | Permissions defined but never checked |

---

## Section 5: Prioritized Fix List

| Priority | ID | File | Instruction/Function | Fix | Effort |
|---|---|---|---|---|---|
| 1 | C-1 | `lp-bonds-evolution/src/lib.rs:1405-1409` | `EvolveBond` struct | Add `constraint = treasury_token_account.mint == token_mint_a.key()` | 5 min |
| 2 | C-2 | `lp-bonds/src/lib.rs:744-761`, `lp-bonds-evolution/src/lib.rs:187-203` | `initialize_oracle`, `update_oracle_authority`, `update_oracle` | Add `require!(authority != Pubkey::default())` | 10 min |
| 3 | H-3 | `lp-bonds-evolution/src/lib.rs` | NEW: `redeem_evolved_bond` | New instruction: burn bond, transfer position NFT, close custody | 2-4 hours |
| 4 | H-1 | `lp-bonds-evolution/src/lib.rs:477-685` | `evolve_bond` | After increase_liquidity CPI, sweep remaining tokens back to user | 1-2 hours |
| 5 | H-2 | `lp-bonds/src/lib.rs:1223-1229` | `RedeemBond` struct | Add `close = user` to `position_custody` | 5 min |
| 6 | H-4 | `lp-bonds/src/lib.rs:808-928` | `verify_collateral` | Add bond ownership check (sender must hold 1 bond NFT) | 15 min |
| 7 | Issue 4 | `lp-bonds/src/lib.rs` | NEW: `collect_fees` | New instruction calling `whirlpool_cpi::collect_fees` through custody PDA | 2-3 hours |
| 8 | Issue 1 | `lp-bonds-evolution/src/lib.rs:829-833` | `validate_source_custody` | Replace level coercion with strict range validation | 5 min |
| 9 | M-3 | `lp-bonds-evolution/src/lib.rs:1114-1144` | `ConfigureLevel` | Either wire up AuthorityWhitelist checks or remove dead code | 1-2 hours |
| 10 | M-1 | `lp-bonds-evolution/src/lib.rs` | `EvolutionConfig` | Add `oracle_enabled` field and check in `validate_oracle_and_nonce` | 30 min |
| 11 | M-4 | `lp-bonds-evolution/src/lib.rs:1328` | `EvolveBond` struct | Remove `mut` from `source_custody`; add close mechanism for orphans | 30 min |
| 12 | Issue 7 | Both programs | NEW: `recover_tokens` | Admin-gated token recovery instruction | 2-3 hours |
| 13 | M-2 | `lp-bonds-evolution/src/state.rs:97-106` | `calculate_fee` | Add explicit u128-to-u64 bounds check | 5 min |
| 14 | L-4 | Both `whirlpool_cpi.rs` | `Whirlpool::from_account_info` | Verify Anchor discriminator before deserialization | 15 min |
| 15 | L-6 | Both programs | NEW: close instructions | Admin/user close for NonceAccount, EvolutionNonce, EvolutionRecord | 2-3 hours |
| 16 | Issue 6 | NEW program | NEW: `lp-bonds-exchange` | Equivalent to EVM LPBondsExchange | 1-2 weeks |

---

## Section 6: Items for Human Review / Business Logic Questions

1. **Fee collection design decision:** When `collect_fees` is implemented, should LP position fees go to the bond holder, the protocol treasury, or be split? The answer affects the incentive model and determines the instruction's access control pattern.

2. **Evolved bond redemption timing:** Should L2-4 bonds have a lock duration from their `PositionCustody.lock_duration`, or should they inherit the remaining lock from their source bond? The current implementation sets `created_at = now` on evolution, which restarts the clock -- is this intentional?

3. **Authority whitelist usage:** The `AuthorityWhitelist` system in the evolution program is fully implemented but never checked by any instruction. Should this be wired up to `configure_level`, `pause_evolution`, `update_treasury`, and `update_oracle`? Or was it built speculatively and should be removed?

4. **update_config and existing bonds:** When an admin calls `update_config` to change the whirlpool or token mints, existing bonds still reference the old whirlpool in their `PositionCustody.whirlpool`. The `verify_collateral` instruction correctly uses custody-specific whirlpool data. However, can this cause confusion or operational issues? Should there be a migration mechanism?

5. **LPBondsExchange priority:** Is the exchange functionality needed for launch, or can it be added post-launch? This is the largest missing feature gap.

6. **Single vs multi-sig admin:** The EVM contracts use `multiSig` patterns. Should the Solana programs require a Squads multi-sig as admin? Currently, a single signer controls all admin functions.

7. **Operator whitelist necessity:** The EVM `OperatorRegistry` restricts bond NFT transfers. Is this a regulatory requirement that must be mirrored on Solana, or was it specific to the EVM ecosystem (e.g., for OpenSea compliance)?

8. **Token-2022 support:** If any whirlpool uses Token-2022 mints (e.g., for transfer hooks or non-transferable tokens), the current programs will fail. Is Token-2022 in scope?

9. **Evolution position custody ownership model:** The evolution program stores position NFTs in ATAs owned by `layer_token_authority` (a global PDA), NOT per-bond custody PDAs. This means a single PDA controls ALL evolved positions. If this PDA is compromised or the program has a bug, ALL evolved positions are at risk. Consider per-bond custody PDAs for evolved positions as well.

10. **Residual tokens after evolution:** After `evolve_bond`, the `program_token_a_account` and `program_token_b_account` (owned by `layer_token_authority`) may retain dust. These accounts are reused across all evolutions (ATAs are `init_if_needed`). Over time, dust accumulates. Is there a sweep mechanism planned?

---

*End of Report*
