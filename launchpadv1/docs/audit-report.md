# Solana Launchpad Migration — Full Spec Compliance Audit Report

## 1. Executive Summary

This audit evaluates the Solana (Anchor) implementation of the EVM Launchpad contracts against the provided specifications (`evm-launchpad-spec.md`, `solana-migration-mapping.md`, `security-migration.md`, and `execution-plan.md`). 

The current Solana implementation is **incomplete and misses critical architectural constraints** established in the EVM design. While the basic account scaffolding and single-NFT Standard minting flow exist, the advanced functionality—namely Refundable collections (R80 remint behavior), OperatorFilter integration (C variants), and precise transaction limits—are heavily flawed or entirely unimplemented. The test suite is virtually non-existent, leaving fatal logic gaps undiscovered.

**Overall Verdict:** ❌ **FAIL (Not Ready for Production)**

---

## 2. Functionality Parity Table

| EVM Feature | Solana Implementation | Status | Notes |
|---|---|---|---|
| F1: Public Mint | `mint_public` | ⚠️ Partial | Only mints 1 NFT per instruction. Remint logic (R80) missing. |
| F2: Presale Mint | `mint_presale` | ⚠️ Partial | Same 1 NFT limitation as F1. |
| F3: Owner Mint | `mint_owner` | ✅ Fully Implemented | Free mints correctly mapped. |
| F4: Refund | `refund_nft` | ✅ Fully Implemented | Burns token and refunds SOL correctly. |
| F5-F9: Config / Toggle | `configure_*`, `toggle_*`| ✅ Fully Implemented | Works as specified. |
| F11-F12: Whitelist | `add_whitelist`, `remove_`| ✅ Fully Implemented | Properly maps to PDA constraints. |
| Operator Filter Transfers | N/A | ❌ Missing | No Transfer Hook or transfer validation implemented. |
| C-Variant Revenue Split| `mint_public`, `mint_presale` | ❌ Missing | No fee is sent to `fund_receiver` during mints. |
| R80 Remint Logic | N/A | ❌ Missing | No `refunded_token_ids` pool tracked in `state.rs`. |

---

## 3. Missing / Broken Features

### 3.1. Broken Per-Transaction Limits (`max_tx_mint_amount`)
The `quantity` parameter in `mint_public` and `mint_presale` is deceiving. The implementation arbitrarily hardcodes `actual_quantity = 1` and mints a single token, but checks the `quantity` argument against `max_tx_mint_amount`. 
- **Bypass**: A user can insert 5 `mint_public` instructions into a single Solana transaction. Since each instruction checks its own `actual_quantity = 1 <= max_tx_mint_amount`, the user successfully mints 5 NFTs in one transaction, completely bypassing the limit.

### 3.2. Missing OperatorRegistry Revenue Splits
The spec defines that RC and R80C variants explicitly send a percentage of the mint price to the `operatorRegistry.fundReceiver`.
- **Finding**: In `gmi-launchpad` mint instructions, the `CollectionType::Refundable100` and `CollectionType::Refundable80` branches **only** split funds between the `vault` and `owner`. The registry share logic is missing.

### 3.3. Missing R80 Remint Mechanism
The EVM spec requires `Refundable80` to recycle refunded token IDs, charging an 80% price when `totalMints == maxMintSupply`.
- **Finding**: `mint_public` ignores `utils::calculate_payment` and its `is_remint` flag. Moreover, in `state.rs`, the `Collection` struct completely omitted the `refunded_token_ids` pool. Remints are impossible in the current Solana state.

### 3.4. Missing Operator Filter Enforcements
The `has_operator_filter` boolean exists in state, but there is no mechanism to enforce it.
- **Finding**: The Solana program lacks any SPL Token-2022 Transfer Hook or custom `transfer` instruction. Users can bypass the registry and trade freely on any marketplace.

---

## 4. Security Findings

### [HIGH] Vault Drain Griefing / Lamport Edge Cases
`refund_nft` transfers the exact `refund_price` back to the user via `transfer_sol_from_vault`. It directly drains the Vault PDA lamports:
```rust
**vault.try_borrow_mut_lamports()? = vault.lamports().checked_sub(amount).unwrap();
```
- **Vulnerability**: If the vault is drained to an amount greater than zero but less than the Solana network's rent-exempt minimum, the transaction will fail, preventing the final users from getting their refunds. 

### [HIGH] Arithmetic Mismatch in Payment Extraction
For `Refundable80`, the code calculates the split:
```rust
let owner_cut = utils::calculate_owner_cut(...);
let vault_cut = utils::calculate_refund_price(...);
// transfers both...
```
Because integer division truncates, `(price * 20 / 100) + (price * 80 / 100)` does not always equal `price` exactly. If `price` is odd/granulated, lamports will be permanently locked or lost during the mint.

### [MEDIUM] Misleading Argument Logic
In `mint_public.rs`, `quantity` is passed as an argument, checked, but ignored for the actual mint (`actual_quantity = 1`). This allows a user to purposely invoke overflow tests or inflate their expected checks, though the actual blast radius is limited by `actual_quantity = 1`.

---

## 5. Architecture Issues

1. **Misaligned Mint Quantity Architecture**: Solana programs usually handle bulk mints by either looping inside the instruction (with a CPI per NFT) or deferring to multiple instructions. By keeping a `quantity` parameter but hardcoding `actual_quantity = 1`, the code creates a dangerous contradiction.
2. **Missing Accounts**: To implement the C-variant revenue splits, `mint_public` and `mint_presale` must accept the `operator_registry` state PDA and the `fund_receiver` account. Currently, these accounts are nowhere in the `#[derive(Accounts)]` structs.
3. **Array vs. PDA**: The execution plan pointed out `Vec<u64>` for `refunded_token_ids` might exceed limits. Instead of solving this, the developer entirely deleted it. A proper Solana architecture would use a linked-list PDA or a bitmap PDA for remint tracking.

---

## 6. Test Coverage Gaps

The test suite (`tests/gmi-launchpad.ts` and `tests/operator-registry.ts`) is practically a stub.

| Component | Coverage | Notes |
|---|---|---|
| Collection Types | **Poor** | Missing tests for `Refundable100` and `Refundable80`. Only tests `Standard`. |
| Mint Limits | **Missing** | No tests asserting `max_user_mint_amount` or `max_tx_mint_amount` rejections. |
| Presale / Whitelist | **Missing** | No tests for `mint_presale` or `add_whitelist`. |
| Edge Cases | **Missing** | No supply cap tests, overflow tests, or free mint tests. |
| Operator Registry | **Poor** | Tests independent registry setup but fails to test CPI integration with the launchpad. |
| Refunds | **Missing** | No tests verifying that a user can actually refund an NFT and receive SOL. |

---

## 7. Recommended Fixes

1. **Refactor Mint Instructions**: 
   - Remove the `quantity` parameter entirely and strictly document that the instruction mints 1 NFT. 
   - Address the `max_tx_mint_amount` by either enforcing it dynamically (e.g. using a transaction sysvar or tracking recent slots) OR loop CPIs internally up to `quantity` (while keeping an eye on compute budgets).
2. **Implement Revenue Sharing**:
   - Add `registry_state` and `fund_receiver` (CHECK) accounts to `MintPublic` and `MintPresale`.
   - Add logic: if `has_operator_filter == true`, calculate and transfer `share_percentage_bps` to the `fund_receiver`.
3. **Re-Architect R80 Remints**:
   - Create a new `RefundPool` PDA holding a bitmap or vector to track refunded token IDs.
   - Update `mint_public` and `mint_presale` to check this pool when `minted_amount >= max_mint_supply` and apply the exact 80% `is_remint` discount.
4. **Fix Vault Lamports**:
   - Make sure vault PDA initialization pads the account with slightly more than the rent-exempt minimum.
   - Or, ensure users pay for the token record rent, leaving vault exclusively holding NFT value.
5. **Implement SPL Token-2022 Transfer Hook**:
   - The only way to securely replicate the `OperatorFilter` is by migrating the `nft_mint` to Token-2022 and attaching a Transfer Hook extension that CPIs into the `operator_registry` to check if `is_allowed`.
6. **Eliminate Truncation Loss**:
   - Ensure `vault_cut = price - owner_cut - registry_cut` instead of independently calculating percentages.
7. **Expand Tests**:
   - Implement complete e2e test suites for all 3 `CollectionType` behaviors.
