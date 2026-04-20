---
name: EVM Contract Analysis Key Findings
description: Critical behavioral findings from exhaustive EVM token contract analysis -- bugs, design patterns, and migration-relevant details for LPToken/GMIToken/GMICVToken
type: project
---

Exhaustive analysis of all 13 EVM contract files completed 2026-03-31. Documentation at `docs/evm/`.

**Why:** Authoritative EVM reference needed for cross-chain migration verification against Solana `programs/lp_token/`.

**How to apply:** Use these findings when comparing EVM and Solana behavior, reviewing migration completeness, or auditing the Solana program.

Key findings:

1. **LPToken pause scope**: Pause ONLY blocks mint/burn. Transfers and approvals work when paused. This is because LPToken does NOT override `_transfer()` or `_approve()`, unlike GMIToken and GMICVToken which do.

2. **LPToken burn -- no allowance check**: `burn(account, amount)` calls `_burn()` directly without checking allowances. Any minter/owner can burn any user's tokens unilaterally. The Solana migration improves this by requiring dual-signer (authority + token holder).

3. **GMIToken/GMICVToken transferFrom bug**: Both override `_transfer` and `_approve` with `nonReentrant`. Since `transferFrom()` calls `_transfer()` then `_spendAllowance()` -> `_approve()`, the reentrancy guard entered by `_transfer` causes `_approve` to revert. `transferFrom()` only works with infinite (max uint256) allowances. This bug does NOT affect LPToken.

4. **Interface mismatches**: `lp-token/interfaces/IGMIToken.sol` declares `maxSupply()` and `updateMaxSupply()` which LPToken does not implement. GMIToken declares `MaxSupplyUpdated` event but has no `updateMaxSupply()` function.

5. **Error message copy-paste**: LPToken uses "GMIToken:" prefix in error messages (e.g., "GMIToken: Only minter or owner is allowed").

6. **chainId is informational only**: Stored in state but never referenced in any logic.

7. **impl() is misleading through proxy**: `address(this)` returns proxy address via delegatecall, not the implementation address.

8. **No storage gap in any token**: None of the three implementation contracts declare `__gap` arrays for upgrade safety.

9. **tradeAllowed defaults to false on GMICVToken**: Not explicitly set in initialize, so trading restrictions are active from deployment.
