# Security Audit Fix Notes

**Date:** 2026-03-25
**Audit Report:** `token-audit-report.md`
**Branch:** `initial-implementation`

---

## Summary of All Fixes Applied

### Previously Applied (by prior session)

These fixes were already in place when this session started (visible in `git diff HEAD`):

#### [C-2] Oracle authority can be set to Pubkey::default()
- **lp-bonds/src/lib.rs**: Added `require!(oracle_authority != Pubkey::default(), ...)` in:
  - `initialize_oracle` instruction
  - `update_oracle_authority` instruction
- **lp-bonds-evolution/src/lib.rs**: Added `require!(..., EvolutionError::InvalidEvolutionOracle)` in:
  - `initialize_evolution` instruction (oracle_authority param)
  - `update_oracle` instruction

#### [M-01/H-2] PositionCustody not closed on redeem_bond (lp-bonds)
- **lp-bonds/src/lib.rs**: Added `close = user` to `position_custody` in `RedeemBond` struct.

#### [H-5/L-04] Residual tokens stuck in program_token_a/b_account after evolve_bond
- **lp-bonds-evolution/src/lib.rs**: After `increase_liquidity` CPI (STEP 8), added STEP 8.5:
  - Reload `program_token_a_account`, transfer any remaining balance back to `user_token_a_account`
  - Reload `program_token_b_account`, burn any remaining layer tokens

#### [H-3] No redemption path for evolved bonds (L2-L4)
- **lp-bonds-evolution/src/lib.rs**: Added `redeem_evolved_bond` instruction with:
  - Pause check
  - Lock expiry check (`created_at + lock_duration`)
  - Bond NFT ownership verification (`user_bond_account.owner == user, mint == bond_mint, amount == 1`)
  - Position NFT transfer from custody to user via `layer_token_authority` PDA signer
  - Bond NFT burn
  - `close = user` on `position_custody` to return rent
  - `BondRedeemed` event emission
- **lp-bonds-evolution/src/errors.rs**: Added `BondStillLocked` error variant
- **lp-bonds-evolution/src/events.rs**: Added `BondRedeemed` event struct

#### [M-1] Evolution oracle has no enabled flag
- **lp-bonds-evolution/src/state.rs**: Added `oracle_enabled: bool` field to `EvolutionConfig`
- **lp-bonds-evolution/src/lib.rs**:
  - Set `oracle_enabled = true` in `initialize_evolution`
  - Added `require!(oracle_enabled, EvolutionError::OracleNotEnabled)` check in `evolve_bond`
  - Added `set_oracle_enabled(enabled: bool)` admin instruction with `SetOracleEnabled` account struct
- **lp-bonds-evolution/src/errors.rs**: Added `OracleNotEnabled` error variant
- **lp-bonds-evolution/src/events.rs**: Added `OracleEnabledChanged` event struct

#### [M-2] calculate_fee silent u128-to-u64 truncation
- **lp-bonds-evolution/src/state.rs**: Added `require!(fee <= u64::MAX as u128, EvolutionError::ArithmeticOverflow)` before the cast in `calculate_fee`.

#### [M-3] AuthorityWhitelist permissions never enforced
- **lp-bonds-evolution/src/lib.rs**: Added `configure_level_delegated` instruction that:
  - Checks caller's `AuthorityWhitelist` account for `PERM_CONFIGURE_LEVELS` bit
  - Otherwise identical to `configure_level`
- **lp-bonds-evolution/src/lib.rs**: Added `ConfigureLevelDelegated` account struct with `authority_whitelist` PDA constraint

#### [Issue 1] validate_source_custody level coercion
- **lp-bonds-evolution/src/lib.rs**: Replaced the level 0/255 fallback with strict bounds check:
  ```rust
  require!(source_level >= MIN_BOND_LEVEL && source_level <= MAX_BOND_LEVEL, EvolutionError::InvalidBondLevel);
  let source_level = custody_ref.level;
  ```

#### VerifyCollateral bond NFT ownership verification
- **lp-bonds/src/lib.rs**: Added `sender_bond_account` to `VerifyCollateral` struct with constraints:
  - `owner == sender.key()`
  - `mint == bond_mint.key()`
  - `amount == 1`

#### verify_collateral whirlpool cross-validation fix
- **lp-bonds/src/lib.rs**: Changed `verify_collateral` to read token mints from on-chain whirlpool state rather than global config. Uses `custody.whirlpool` as the authoritative source instead of `config.token_mint_a/b`, which may have changed since the bond was minted.

---

### Applied in This Session

#### [M-4] BLOCKER: is_evolved flag breaks L2->L3->L4 evolution (FIX 1)
- **lp-bonds-evolution/src/lib.rs**: Removed `require!(!custody_ref.is_evolved, EvolutionError::BondAlreadyEvolved)` from `validate_source_custody()`. The `is_evolved` flag was incorrectly blocking L2->L3 and L3->L4 progression because evolved bonds (L2+) have `is_evolved == true` by design. Double-evolution prevention is already handled by the EvolutionRecord PDA `init` constraint (seeded by source_bond_mint).

#### [M-1] BLOCKER: recover_tokens can drain active custody positions (FIX 2)
- **lp-bonds-evolution/src/lib.rs**: Added `bond_mint: Account<'info, Mint>` to `RecoverTokens` struct with constraint `bond_mint.supply == 0 @ EvolutionError::RecoveryCustodyProtected`. This proves the bond has been burned and the position is no longer active before allowing token recovery.

#### [H-1] No fee collection instruction (FIX 3)
- **lp-bonds/src/lib.rs**: Added `collect_fees` instruction:
  - User must hold bond NFT (amount == 1)
  - CPI to Whirlpool `collect_fees` with `position_custody` PDA as signer
  - Fees sent directly to user's token A/B accounts
  - Emits `FeesCollected` event (reuses existing event struct)
  - All accounts boxed to avoid BPF stack overflow
- **lp-bonds/src/lib.rs**: Added `CollectFees` account struct with full validation (bond ownership, custody PDA, position token account, whirlpool match)
- **lp-bonds-evolution/src/lib.rs**: Added `collect_fees` instruction:
  - Same pattern but uses `layer_token_authority` PDA as position authority signer
  - Checks `evolution_config.is_paused`
  - All accounts boxed
- **lp-bonds-evolution/src/lib.rs**: Added `CollectFees` account struct
- **lp-bonds-evolution/src/events.rs**: Added `FeesCollected` event struct

#### [M-3] No lock_duration > 0 validation (FIX 4)
- **lp-bonds/src/lib.rs**: Added `require!(lock_duration > 0, LpBondsError::InvalidLockDuration)` in:
  - `initialize` instruction
  - `update_config` instruction
- **lp-bonds-evolution/src/lib.rs**: Added `require!(lock_duration > 0, EvolutionError::InvalidLockDuration)` in:
  - `configure_level` instruction
  - `configure_level_delegated` instruction
- **lp-bonds-evolution/src/errors.rs**: Added `InvalidLockDuration` error variant

#### [L-4] Treasury can be set to Pubkey::default() (FIX 5)
- **lp-bonds-evolution/src/lib.rs**: Added `require!(new_treasury != Pubkey::default(), EvolutionError::TreasuryNotSet)` in `update_treasury` instruction.

#### Stack overflow fix for RedeemBond
- **lp-bonds/src/lib.rs**: Boxed all `Account` fields in `RedeemBond` struct to reduce stack frame below 4096 bytes. Changed `Account<'info, ...>` to `Box<Account<'info, ...>>` for: `config`, `user_bond_account`, `user_position_token_account`, `bond_mint`, `position_mint`, `position_custody`, `custody_position_token_account`.

#### [L-1] Oracle admin divergence from protocol admin
- **lp-bonds/src/lib.rs**: Changed `UpdateOracleAuthority` struct to check `admin.key() == config.admin` (using `ProtocolConfig.admin`) instead of `oracle_config.admin`. Added `config: Account<'info, ProtocolConfig>` with proper PDA derivation. This ensures the current protocol admin controls oracle authority updates even after admin transfer.

#### [L-3] Whirlpool discriminator not verified
- **lp-bonds/src/whirlpool_cpi.rs**: Added `WHIRLPOOL_DISCRIMINATOR` constant (`sha256("account:Whirlpool")[..8]` = `[63, 149, 209, 12, 225, 128, 99, 9]`). In `from_account_info`, added discriminator validation before deserialization.
- **lp-bonds-evolution/src/whirlpool_cpi.rs**: Same change applied.

#### [L-5] evolve_bond accepts extra remaining_accounts silently
- **lp-bonds-evolution/src/lib.rs**: Changed `ctx.remaining_accounts.len() >= 4` to `ctx.remaining_accounts.len() == 4` to reject transactions with unexpected extra accounts.

#### [L-6] Nonce accounts cannot be closed
- **lp-bonds/src/lib.rs**: Added `close_nonce_account` instruction with `CloseNonceAccount` account struct. Uses `close = user` constraint. Only the nonce owner can close their account.
- **lp-bonds-evolution/src/lib.rs**: Added `close_evolution_nonce` instruction with `CloseEvolutionNonce` account struct. Same pattern.

#### [Issue 7] No emergency token recovery
- **lp-bonds/src/lib.rs**: Added `recover_tokens` instruction with `RecoverTokens` account struct:
  - Admin signer required (checked against `config.admin`)
  - Transfers `amount` tokens from a `bond_authority`-owned account to `admin_token_account`
  - Source account constrained to be owned by `bond_authority` PDA
  - Emits `RecoveryEvent`
- **lp-bonds/src/events.rs**: Added `RecoveryEvent` event struct
- **lp-bonds/src/errors.rs**: Added `RecoveryCustodyProtected` error variant
- **lp-bonds-evolution/src/lib.rs**: Added `recover_tokens` instruction with `RecoverTokens` account struct:
  - Admin signer required (checked against `evolution_config.admin`)
  - Transfers tokens from `layer_token_authority`-owned account to admin
  - Emits `RecoveryEvent`
- **lp-bonds-evolution/src/events.rs**: Added `RecoveryEvent` event struct
- **lp-bonds-evolution/src/errors.rs**: Added `RecoveryCustodyProtected` error variant

#### Stack overflow fix for RedeemEvolvedBond
- **lp-bonds-evolution/src/lib.rs**: Boxed `Account` fields in `RedeemEvolvedBond` struct to reduce stack frame below 4096 bytes.

#### [M-05] Remove redundant token approve/revoke for Whirlpool CPI
- **lp-bonds-evolution/src/lib.rs**: Removed both `token::approve()` calls (STEP 5) that set `whirlpool_program` as delegate on `program_token_a_account` and `program_token_b_account`. These were redundant because `layer_token_authority` PDA owns the token accounts and signs the `increase_liquidity` CPI via `invoke_signed`. Orca's Whirlpool uses the `position_authority` (layer_token_authority) as the transfer authority directly, so delegation is never exercised. Also removed the now-unused `deposit_amount_a` variable. Renumbered STEP 5 (was STEP 6) through subsequent steps.
- **lp-bonds/src** — No changes needed. The base lp-bonds program does not use approve/delegate for Whirlpool CPI.

#### [M-02/M-03] Admin-only close_orphaned_custody instruction
- **lp-bonds/src/lib.rs**: Added `close_orphaned_custody` instruction and `CloseOrphanedCustody` account struct:
  - Admin signer required (checked against `config.admin`)
  - `bond_mint.supply == 0` constraint ensures the bond NFT has been burned
  - `close = admin` on `position_custody` PDA to reclaim rent to admin
  - PDA validated via `[POSITION_CUSTODY_SEED, bond_mint.key().as_ref()]` seeds
  - Does NOT extract source liquidity (stays locked by design)
- **lp-bonds-evolution/src/lib.rs**: Added identical `close_orphaned_custody` instruction and `CloseOrphanedCustody` account struct:
  - Admin signer required (checked against `evolution_config.admin`)
  - Same bond_mint supply == 0 guard
  - Same `close = admin` rent reclaim pattern

#### [L-06] Oracle toggle for base lp-bonds program
- **lp-bonds/src/lib.rs**: Added `set_oracle_enabled` admin instruction with `SetOracleEnabled` account struct:
  - Admin signer required (checked against `config.admin`)
  - Toggles `oracle_config.enabled` field (which already existed but had no toggle instruction)
  - Emits `OracleEnabledChanged` event
  - Matches the pattern from the evolution program's `set_oracle_enabled`
- **lp-bonds/src/events.rs**: Added `OracleEnabledChanged` event struct with `enabled: bool`, `admin: Pubkey`, `timestamp: i64`
- **lp-bonds/src/state.rs**: Updated doc comment on `OracleConfig.enabled` to reference `set_oracle_enabled`
- **No account struct size change:** The `enabled` field already existed in `OracleConfig`. This fix only adds the admin instruction to toggle it.

---

## Account Struct Size Changes

### EvolutionConfig (lp-bonds-evolution)
- **Added field:** `oracle_enabled: bool` (1 byte)
- **INIT_SPACE impact:** Increased by 1 byte. New deployments will allocate the correct size. Existing accounts would need realloc if this field is to be used on already-initialized configs.
- **Migration:** For existing deployed configs, a realloc instruction or redeployment would be needed. Since this is a devnet program, redeployment is recommended.

## Fixes NOT Applied / Deferred

### [M-4] source_custody incorrectly marked mut
- **Status:** Already NOT an issue in the current code. The `source_custody` field in `EvolveBond` is declared as `UncheckedAccount<'info>` without `#[account(mut)]`. No change needed.

### [H-4] Bond NFT ownership not verified in evolve_bond
- **Status:** Already handled. The `user_source_bond_account` in `EvolveBond` has constraints `owner == user.key()` and `mint == source_bond_mint.key()`, and the handler checks `amount == 1`. This provides complete ownership verification.

### [C-1] Treasury token account mint not verified
- **Status:** Already handled. The `treasury_token_account` in `EvolveBond` has both `owner == evolution_config.treasury` and `mint == token_mint_a.key()` constraints.

### [M-05] approve() to whirlpool_program is incorrect pattern
- **Status:** FIXED. See "Applied in This Session" section above.

### [M-02/M-03] PositionCustody account not closed / source liquidity stays locked
- **Status:** FIXED via `close_orphaned_custody` instruction. See "Applied in This Session" section above. Source liquidity stays intentionally locked; only rent reclaim is supported when bond_mint.supply == 0.

### [L-02] Redundant signer check
- **Status:** Not fixed. The redundant `require!(ctx.accounts.user.is_signer, ...)` checks in both `add_liquidity_and_mint_bond` and `evolve_bond` are harmless and provide defense-in-depth. Not worth modifying for a code quality issue.

### [L-06] No oracle disable/enable toggle (lp-bonds base program)
- **Status:** FIXED. See "Applied in This Session" section above.

## Pre-existing Issues

### EvolveBond stack frame overflow warnings
- **Status:** Pre-existing. The BPF linker reports stack frame warnings for `EvolveBond::try_accounts` generated code. All accounts are already boxed. The warnings do not prevent compilation but may cause runtime issues if the stack limit is actually exceeded during execution. This is a known limitation of complex Anchor account structs with many `init`/`init_if_needed` accounts. A potential fix would be to split the instruction into multiple steps (e.g., prepare + execute pattern).

## Build Status

- **lp-bonds:** Compiles cleanly (warnings only from Anchor derive macros, which are cosmetic)
- **lp-bonds-evolution:** Compiles with pre-existing BPF linker stack warnings on EvolveBond (not introduced by any fix)
- Both `.so` deploy artifacts generated successfully
- No Rust compilation errors in either program
- **Tests:** Could not run (`ts-mocha` not installed in environment). No test failures to report.
