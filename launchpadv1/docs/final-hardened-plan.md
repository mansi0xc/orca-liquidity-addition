# Final Hardened Implementation Plan

## 1. Updated Architecture
Token-2022 and Transfer Hooks are permanently removed. The protocol utilizes standard SPL Tokens and enforces **Economic Operator Filtering** purely at the application layer. Raw SPL transfers are not blocked globally, but the protocol strictly segregates token states into **Settled** vs **Unsettled** ownership. An Unsettled token mathematically loses its Launchpad protocol utility (e.g., Refund rights), enforcing compliant interactions without requiring token-level restrictions.

## 2. Instruction-Level Design
### `protocol_transfer` (Safe Settlement Primitive)
- **Purpose**: Facilitate the atomic, fully compliant transfer of an NFT.
- **Signers**: `seller` (must approve), `buyer` (if applicable for payment/rent), `operator` (optional, the marketplace PDA).
- **Execution**: 
  1. Validates `token_record.owner == seller.key()`.
  2. If `operator` is provided, asserts `OperatorRegistry.is_whitelisted(operator)`.
  3. Executes `spl_token::transfer` directly into the buyer's account.
  4. Updates `token_record.owner = buyer.key()`.
- **Atomicity**: The state update and SPL transfer occur in the same execution context.

### `sync_record` (Ownership Resolution)
- **Purpose**: Heal Unsettled tokens.
- **Execution**: Requires the signature of the `token_record.owner` (the Seller). It updates the record to the new token holder. If traded on a disallowed platform, the seller's signature is unobtainable by the automated platform, irrevocably rendering the token Unsettled.

### `mint_public` & `mint_presale`
- **Purpose**: Minting with strict slot-based limits.
- **Execution**: Evaluates `Clock::get()?.slot`. Appends rate limit constraints deterministically. Pulls exact R80 identity from `RefundQueue` PDA if applicable.

## 3. Account + PDA Model
- **TokenRecord (`mint_record` PDA)**
  - Stores `owner: Pubkey` to track formal Settled ownership.
- **MintCounter PDA**
  - Stores `last_mint_slot: u64` and `slot_mint_count: u64` for temporal limits.
- **RefundQueue PDA** (New)
  - Stores `refunded_indices: Vec<u64>` preserving absolute EVM identity topologies for R80 remints.

## 4. Security Validations
- **Revenue Split (Zero-Loss Accounting)**: Fractional lamports are handled securely via subtractive remainders.
  ```rust
  let protocol_fee = (price as u128 * registry.share_bps as u128) / 10000;
  let net_price = price.checked_sub(protocol_fee as u64)?;
  let vault_cut = (net_price as u128 * 80) / 100;
  let owner_cut = net_price.checked_sub(vault_cut as u64)?;
  ```
- **Vault Rent DoS**: Exclusively funded with `Rent::get()?.minimum_balance(0)` during `initialize_collection`. Refunds physically cannot drain the base rent epoch payload.
- **Slot Limits**: `require!(mint_counter.slot_mint_count <= max_tx_mint_amount)`. CPI instruction loops are nullified.

## 5. Invariant Definitions
- **Ownership Correctness**: `token_account.owner == token_record.owner`. This invariant is required for `refund_nft`. When broken by native SPL transfers, the state is strictly labeled Unsettled.
- **Mint Limits**: Sum(Mints) per user per Slot <= `max_tx_mint_amount`.
- **Vault Solvency**: `VaultBalance >= Total Refund Obligations + RentMinimum`.
- **Payment Distribution**: `price == protocol_fee + vault_cut + owner_cut` precisely without truncation.

## 6. Function Mapping Table (EVM → Solana)
| EVM Spec ID | Function Descriptor | Solana Instruction | Implementation Status |
|-------------|---------------------|--------------------|-----------------------|
| **F1** | `mintPublic` | `mint_public` | ✅ Fully Implemented |
| **F2** | `mintPresale` | `mint_presale` | ✅ Fully Implemented |
| **F3** | `refund` | `refund_nft` | ✅ Fully Implemented |
| **F4** | `toggleSales` | `toggle_presale` / `public`| ✅ Fully Implemented |
| **F5** | `setBaseURI` | `set_base_uri` | ✅ Fully Implemented |
| **F6** | `calculatePayment` | N/A (Embedded logic) | ✅ Constraints mapped |
| **F7** | `safeTransferFrom` | `protocol_transfer` | ✅ Safe escrow pattern |
| **F8** | `withdraw` | `withdraw_vault` | ✅ Present |
| **IB9/12** | 80/20 Math Splits | `utils::*` logic | ✅ Subtractive exact match|

## 7. Attack Resistance Explanation
- **CPI Limit Bypass Vectors**: Defeated. Because limits are bound sequentially to the absolute physical `Clock::slot`, MEV builders and proxy scripts cannot construct recursive parallel execution traces without artificially delaying execution into subsequent blocks (violating the physical time threshold).
- **Secondary Liquidity Arbitrage**: Defeated. Bad actors transferring tokens to non-whitelisted AMMs permanently detach the asset's utility (Refund execution). By locking the `TokenRecord` to the initial seller, illicit platforms hold mathematically dead artifacts, forcing organic liquidity directly to whitelisted operators implementing the `protocol_transfer` CPI.
- **Rent Exhaustion Lock**: Defeated. By proactively supplying structural base reserves during the initial transaction footprint, no end-user interacts with the rent curve.
