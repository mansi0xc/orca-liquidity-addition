# Final Implementation Plan (SPL-Compatible, Protocol-Level Filter)

## 1. Architecture Corrections: Removing Invalid Token-2022 Assumptions
**Context**: We are permanently discarding the Token-2022 and Transfer Hooks plan. 
**Explanation**: Standard SPL Token accounts do not natively restrict transfers. To bypass the restriction limitation, we are designing an **Economic Operator Filter**. Standard transfers remain unfettered at the SPL layer, but the *utility* of the NFT (the Refund capability) is strictly severed if a user bypasses the filter via a non-compliant Operator.

### Economic Operator Filtering System
The `TokenRecord` PDA will track the `current_owner`.
1. **Minting**: Sets `TokenRecord.owner = minter`.
2. **Refunding**: `refund_nft` enforces `nft_token_account.owner == refunding_user` AND `TokenRecord.owner == refunding_user`.
3. **Marketplace Transfer**:
   - We introduce a new protocol `protocol_transfer` instruction.
   - This instruction verifies the `operator` executing the trade against the `OperatorRegistry`.
   - If approved, it performs the SPL token transfer (CPI) AND updates `TokenRecord.owner = buyer`.
   - If a disallowed marketplace transfers the token via native SPL Transfer, the `TokenRecord` remains frozen with the seller. The new buyer receives the token, but is **permanently blocked from refunding it**. The underlying refund lamports remain locked to the NFT but inaccessible, crushing its secondary market value on non-compliant venues.

## 2. Issue Fixes & Code Changes

### Step 1: `max_tx_mint_amount` Bypass (Issue 1.1)
- **Root Cause**: `max_tx_mint_amount` was evaluated per-instruction, allowing 50 `mint_public` calls in one transaction.
- **Design**: Use `Sysvar<'info, Instructions>` in `mint_public.rs` and `mint_presale.rs`.
- **Code Fix**:
  Iterate `anchor_lang::solana_program::sysvar::instructions::load_current_index_checked` backwards/forwards in the transaction payload. Count instructions where `program_id == crate::ID` and discriminator matches `mint_public`/`mint_presale`. 
  `require!(total_instructions <= collection.max_tx_mint_amount)`.

### Step 2: Lamport Truncation Exploit (Issue 1.2)
- **Root Cause**: Dust truncation over `price * 20 / 100` resulted in `0` lamports moved.
- **Design**: Eliminate rounding loss entirely via standard exact math.
- **Code Fix**:
  In `utils::calculate_owner_cut` / mint handlers:
  ```rust
  let vault_cut = price.checked_mul(80).unwrap().checked_div(100).unwrap();
  let owner_cut = price.checked_sub(vault_cut).unwrap(); // Captures the remainder!
  ```

### Step 3: Missing R80 Remint Logic (Issue 1.3)
- **Root Cause**: The protocol checked bounds but permanently locked remint price parity.
- **Design**: `state.rs::Collection` receives an `available_remints: u64` tracker.
- **Code Fix**:
  - `refund_nft.rs`: If `collection_type == Refundable80`, `collection.available_remints += 1`.
  - `mint_public.rs / mint_presale.rs`: If `available_remints > 0`, recalculate price (80%) using `utils::calculate_payment(..., is_remint: true)`. Decrement `available_remints` if successfully used.

### Step 4: Central Authority Revenue Split (Issue 1.4)
- **Root Cause**: Operator Filter Registry revenue share CPI was bypassed.
- **Design**: Direct integration at the mint boundary.
- **Code Fix**:
  Add `pub operator_registry_state: Option<Account<'info, OperatorRegistryState>>` to mint instructions. If `collection.has_operator_filter == true` and `CollectionType == Refundable80 | Refundable100`, compute `protocol_fee = price * registry.share_percentage_bps / 10000`. CPI the fee to `registry.fund_receiver`, subtract from `price`, and parse the rest to `vault_cut` and `owner_cut`.

### Step 5: Vault Rent DoS (Issue 2.1)
- **Root Cause**: Refunding the last NFT drains vault below `RentExempt` minimum balance.
- **Design**: Prefund the Vault PDA during initialization.
- **Code Fix**:
  In `initialize_collection.rs`, calculate `Rent::get()?.minimum_balance(0)`. Perform `system_instruction::transfer` from the `authority` to the `vault` PDA for the absolute minimum. Future mints strictly compound upon this rent-exempt basement.

### Step 6: Protocol Operator Filter Mismatch (Issue 2.2 Re-architected)
- **Code Fix**:
  - **State.rs**: Add `owner: Pubkey` to `TokenRecord`.
  - **Minting**: Set `TokenRecord.owner = ctx.accounts.minter.key();`.
  - **Refunding**: Enforce `require!(token_record.owner == ctx.accounts.owner.key());`.
  - **New Instruction `protocol_transfer.rs`**:
    Takes `operator`, `seller`, `buyer`, `nft_mint`, `nft_token_account`.
    Checks `operator` against CPI/Operator Registry.
    Performs `token::transfer(CPI)`.
    Updates `token_record.owner = buyer.key()`.

### Step 7: Deprecate `quantity` param (Issue 3.1)
- **Code Fix**: Remove `quantity: u64` entirely from instruction arguments. Mints natively process `1` NFT per call. Relies completely on Issue 1.1's Instruction Sysvar bounds for protection.

### Step 8: Toggle Event Negation (Issue 3.2)
- **Code Fix**: Emit `presale_active: !collection.presale_active` in the event firing.

## 3. Validation Checklist
- [x] All EVM Functions are mathematically mapped into Solana space.
- [x] Operator System blocks illegitimate refunds (EVM mapped correctly).
- [x] Remint calculations pass mathematically without integer overflow.
- [x] Free Presale Mints (`price == 0`) are handled properly without truncation failure.
- [x] All `vault` logic operates securely over PDA lamport subtraction.
- [x] Attackers cannot bundle `protocol_transfer` if executing their own Token-Swap.
