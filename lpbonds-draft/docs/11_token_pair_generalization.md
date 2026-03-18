# 11 - Token Pair Generalization for LP Bonds

## Summary

The Level 1 `add_liquidity_and_mint_bond` flow was partially coupled to WSOL assumptions even though Whirlpool itself is pair-agnostic.

This refactor removes WSOL-specific account assumptions and makes token behavior derive from Whirlpool on-chain state (`token_mint_a`, `token_mint_b`, `token_vault_a`, `token_vault_b`).

The instruction now supports:
- SOL pairs (via native-mint-aware wrapping path)
- Stablecoin pairs (for example USDC-GMI, USDT-GMI)
- Any arbitrary SPL-SPL Whirlpool pair

## What Was Wrong Before

The previous design had these issues:
- `AddLiquidityAndMintBond` still required a fixed `wsol_mint` account.
- The account struct included a WSOL-specific user token account path.
- Token account wiring was not fully generic at the account-constraint layer.
- Design implied Level 1 was effectively SOL-centric even though Whirlpool is not.

Result:
- Integration complexity for non-SOL pairs.
- Reduced composability for any Whirlpool where token A is not native mint.
- Unclear security model around account substitution for generic pairs.

## Refactor Overview

### 1. Removed WSOL-specific account assumptions

In `AddLiquidityAndMintBond`:
- Removed `wsol_mint` account requirement.
- Removed WSOL-only account path.
- Added explicit generic mint accounts:
  - `token_mint_a`
  - `token_mint_b`
- Added generic user token source accounts:
  - `user_token_a_account`
  - `user_token_b_account`

### 2. Whirlpool is source of truth

Inside instruction logic, Whirlpool state is deserialized and validated:
- `whirlpool.token_mint_a`
- `whirlpool.token_mint_b`
- `whirlpool.token_vault_a`
- `whirlpool.token_vault_b`

Runtime checks enforce that passed accounts match Whirlpool state exactly.

### 3. Token account and vault validation

The following checks are now enforced:
- `user_token_a_account.mint == whirlpool.token_mint_a`
- `user_token_b_account.mint == whirlpool.token_mint_b`
- `token_vault_a.mint == whirlpool.token_mint_a`
- `token_vault_b.mint == whirlpool.token_mint_b`
- `token_vault_a.key == whirlpool.token_vault_a`
- `token_vault_b.key == whirlpool.token_vault_b`

This prevents token-side account substitution and ensures the CPI targets the expected Whirlpool vaults.

### 4. Conditional SOL wrapping

Native handling is now conditional:
- If mint is not native mint, token is treated as normal SPL.
- If mint is native mint (`So111...`), instruction runs native sync/wrap preparation for that side only.

The wrapping path is selective and only activated for the Whirlpool side(s) that actually use native mint.

### 5. Event model generalized

`BondMinted` event payload no longer exposes SOL-only semantics.
It now tracks pair-generic token limits (`token_max_a`, `token_max_b`).

## Security Implications

The generalized path improves safety for non-SOL pools because validation is now fully pair-aware.

Key security properties:
- Whirlpool program is pinned and validated.
- Whirlpool account owner is validated when reading Whirlpool state.
- User token source accounts are constrained to user ownership and correct mint.
- Vault accounts are typed SPL token accounts and validated by both mint and Whirlpool key match.
- No hardcoded token assumptions remain in instruction account wiring.
- Generic logic prevents accidental execution against mismatched pool-side token accounts.

## Edge Cases

### SOL / native-mint side present
- Native wrapping logic executes only on the relevant side.
- Non-native side remains a normal SPL transfer path.

### SPL-SPL pool
- No native wrapping branch runs.
- Both sides are validated and processed as standard SPL accounts.

### Wrong pool accounts supplied
- Instruction fails due to Whirlpool key/mint/vault mismatch checks.

## Compatibility Notes

- Existing clients must now provide generic `token_mint_a` and `user_token_a_account` instead of WSOL-specific accounts.
- Account order and instruction builders should be regenerated from updated IDL after deployment.

## Files Updated

- `programs/lp-bonds/src/lib.rs`
- `programs/lp-bonds/src/events.rs`
- `programs/lp-bonds/src/state.rs`
- `programs/lp-bonds/src/constants.rs`
