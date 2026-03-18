# Formal Protocol Verification & Adversarial Audit
**Target**: Solana Launchpad (EVM Migration)
**Type**: Principal Security & Spec Compliance Audit
**Status**: 🔴 CRITICAL - DO NOT DEPLOY

---

## 1. 🔴 Critical Failures & Exploit Scenarios

### 1.1 Multi-Instruction Transaction Bypass (`max_tx_mint_amount` invariant broken)
**Spec Element**: R5 (`_quantity <= _maxTxMintAmount`) 
**Invariant**: A user cannot construct a transaction that mints more than `max_tx_mint_amount` NFTs, mitigating bot sweeps.
**Vulnerability**: Solana instructions are completely stateless within a transaction context. The code implemented `mint_public` to mint exactly 1 NFT per instruction, validating the generic parameter `quantity <= max_tx_mint_amount`.
**Exploit Trace (Step-by-step)**:
1. `max_tx_mint_amount` is set to `2`. `max_user_mint_amount` is `50`.
2. Attacker crafts a single Solana transaction containing **50 consecutive `mint_public` instructions**.
3. For each instruction, `quantity` is passed as `1`.
4. The checks pass: `1 <= 2`. `minted_amount` updates correctly across instructions.
5. In a single atomic slot execution, the attacker sweeps 50 NFTs.
6. **Result**: Complete circumvention of the bot-prevention mechanism. 
**Proof**: `max_tx_mint_amount` is verified on a *per-instruction* level, not a *per-transaction* level.

### 1.2 The "Micro-Mint" Lamport Truncation Exploit (Free Mints)
**Spec Element**: R14 (`mintPrice` validation), IB9 / IB12 (80/20 splits)
**Invariant**: User pays exactly `mint_price`; owner receives exactly 20%, vault 80%.
**Vulnerability**: Integer truncation limits. When dealing with lamports, the absence of a minimum price enforcement allows catastrophic truncations.
**Exploit Trace (Step-by-step)**:
1. Authority configures/adjusts `CollectionType::Refundable80` with `mint_price = 4` lamports.
2. Attacker invokes `mint_public`.
3. `owner_cut = (4 * 20) / 100 = 0` lamports.
4. `vault_cut = (4 * 80) / 100 = 0` lamports.
5. `utils::transfer_sol` silently ignores `amount == 0` transfers.
6. **Result**: Attacker successfully mints the NFT for absolutely **0 lamports**, bypassing payment while increments state allocations.
**Proof**: Missing upper/lower bound checks on `mint_price` combined with `checked_div` truncation allows rounding to zero.

### 1.3 Missing R80 Remint Implementation (Permanent Economic Lock)
**Spec Element**: F1 (Branch 1 & 2), R18, IB12
**Invariant**: Refunded tokens in R80 variants re-enter the supply, and next users pay 80% price.
**Vulnerability**: `mint_public.rs` completely omits the `is_remint` checks. Furthermore, `refunded_token_ids` does not exist in `state.rs`.
**Exploit Trace**:
1. Collection reaches `max_mint_supply`.
2. 5 users refund their NFTs, dropping `minted_amount` below max limit.
3. 5 new users come to re-mint those exact slots.
4. Program logic charges them **100% full price** instead of the required 80% protocol discount.
5. **Result**: The protocol steals 20% value from the new minters and permanently breaks the EVM migration guarantee.
**Proof**: `utils::calculate_payment` is an orphaned function. `price` is hardcoded to `collection.mint_price` in `mint_public.rs` (Line 113).

### 1.4 Central Authority Revenue Robbery (C-Variants Missing Split)
**Spec Element**: F13 / Registry Revenue Share logic (OperatorRegistry CPI)
**Invariant**: If `has_operator_filter == true` and `CollectionType == Refundable80 (R80C) / Refundable100 (RC)`, `sharePercentageBps` of the price goes to the registry's `fundReceiver`.
**Vulnerability**: The entire revenue split is unimplemented in the mint handlers.
**Exploit Trace**:
1. Operator Registry configures global share to 2% (200 BPS).
2. Users mint `Refundable100`.
3. The vault absorbs 100% of funds. 
4. The Launchpad protocol revenue share is bypassed entirely. The platform earns nothing.
**Proof**: `mint_public.rs` transfers strictly to `owner_account` and `vault`. `operator_registry` account is not declared in `#[derive(Accounts)]`.

---

## 2. 🟠 High Risk Issues

### 2.1 Vault Rent Exhaustion Denial of Service (DoS)
**Spec Element**: S6 (Rent Lamport Edge Cases)
**Invariant**: `vault` PDA must be rent-exempt to persist.
**Vulnerability**: The `refund_nft.rs` executes:
`**vault.try_borrow_mut_lamports()? = vault.lamports().checked_sub(amount).unwrap();`
If the user refunds the final NFT of the collection, the vault subtracts the last `refund_price`. However, if the remaining lamports fall below `~2_000_000` (rent epoch minimum), the Solana runtime throws a `RentExempt` violation, reverting the transaction.
**Result**: The last users to refund will be completely unable to withdraw their funds (Permanent Lock).

### 2.2 Operator Filter Whitelist Complete Bypass
**Spec Element**: EVM OperatorFilter Registry integration
**Invariant**: A marketplace not on the `OperatorWhitelist` cannot facilitate transfers.
**Vulnerability**: Program lacks a `transfer` instruction wrapper or SPL Token-2022 Transfer Hook.
**Result**: Any user can transfer the minted NFTs via standard `spl_token` instructions natively. Protocol economy completely circumvented.

---

## 3. 🟡 Logic / Spec Deviations

### 3.1 Unused `quantity` Parameter
In `mint_public.rs` and `mint_presale.rs`, the instruction demands `quantity: u64`. However, Line 97 physically overrides it:
`let actual_quantity: u64 = 1;`
This breaks interface expectations and leads to severe frontend bugs (if a dApp passes 5, only 1 is minted, breaking indexing and UI state updates).

### 3.2 Toggle Event Negation Bug (Accidentally Fixed / Behavior Changed)
The EVM Spec notes IB15 ("Event bug in toggle... emits negated value AFTER toggle"). The Solana code implements it correctly. While correct execution is better, this is a spec behavioral discrepancy that indexers mapping from EVM will fail to track if they explicitly parsed the bugged EVM events.

---

## 4. ⚠️ Missing Features & Untested Behaviors

### ❌ MISSING FEATURES
- `OperatorRegistry` CPI Integration.
- SPL Token 2022 Transfer Hook Extension.
- `RefundPool` Vector (for R80/RC remint cyclic routing).
- `calculate_payment` remint routing integration.

### ❌ UNTESTED COVERAGE
- `CollectionType::Refundable100` (No test exists).
- `CollectionType::Refundable80` (No test exists).
- `quantity` accumulation bypass tests (max_tx limit bounds).
- Zero-cost presale limits (`reserved_nfts`).

---

## 5. ✅ Verified Correct Components

- **MintCounter PDA Isolation**: The `minter.key()` derivation securely isolates user counts.
- **Whitelist Enforcement**: Correctly implements dynamic `mint_limit` rather than boolean checks.
- **Refund Logic**: Successfully prevents owner-mint refunds and securely burns the token mint (`mint.supply == 1` check handles uniqueness).

---

## Summary Verdict 

The implementation models the superficial state architecture successfully but fails entirely at the complex transactional and economic boundaries. The disconnect between per-instruction logic on Solana versus per-transaction logic on EVM systematically broke the `max_tx_mint_amount` security protections.

To resolve, the team must discard the `quantity: u64` arguments, mandate instruction introspection for aggregate transaction limit checks, refactor the `CollectionType` match arms to CPI into `operator_registry`, and switch the underlying `spl_token` standard to `token_2022` to accommodate the Transfer Hooks constraint.
