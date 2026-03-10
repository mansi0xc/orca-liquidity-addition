# 07 — Security Considerations

---

## 1. Security Mechanisms in the EVM Version

### 1.1 Reentrancy Protection
- **Mechanism:** `ReentrancyGuardUpgradeable` (OpenZeppelin) with `nonReentrant` modifier.
- **Applied to:** `Exchange.transfer()`, `Exchange.safeTransferERC20()`, and all proxy transfer functions.
- **Purpose:** Prevents a malicious token contract from re-entering the Exchange during a transfer callback (e.g., ERC-721 `safeTransferFrom` triggers `onERC721Received`).

### 1.2 Pausable
- **Mechanism:** `PausableUpgradeable` (OpenZeppelin) with `whenNotPaused` modifier.
- **Applied to:** `matchOrders`, `batchMatchOrders`, `cancelOrder`, `transfer`.
- **Purpose:** Emergency circuit breaker. Owner can halt all exchange operations.

### 1.3 EIP-712 Signature Verification
- **Mechanism:** Structured typed-data signing with domain separator bound to `chainId` and `verifyingContract` (proxy address).
- **Purpose:** Prevents cross-chain replay (different chainId) and cross-contract replay (different proxy address). Provides strong typing of signed data.

### 1.4 EIP-1271 Smart Contract Signature Support
- **Mechanism:** If `ecrecover` doesn't match the order maker and the maker is a contract, calls `IERC1271.isValidSignature()`.
- **Purpose:** Enables multisig wallets and smart contract wallets to create orders.

### 1.5 Signature Malleability Protection
- **Mechanism:** `LibSignature.recover` enforces `s` is in the lower half of the secp256k1 curve order.
- **Purpose:** Prevents signature malleability attacks where a valid signature can be transformed into a different but still valid signature.

### 1.6 Order Book matchAllowance (Time-Limited Authorization)
- **Mechanism:** The Order Book signs `(orderKeyHash, matchBeforeTimestamp)` using EIP-712. The on-chain contract verifies `matchBeforeTimestamp > block.timestamp` and the signer matches the stored `orderBook` address.
- **Purpose:** Rate-limits order matching. Even if someone obtains all order data and signatures, they cannot match orders without a fresh matchAllowance from the Order Book. This provides MEV protection and allows the Order Book to control matching timing.

### 1.7 Order Fill Tracking (Partial Fill Prevention)
- **Mechanism:** `fills[orderKeyHash]` monotonically increases. Fill is checked against remaining order value.
- **Purpose:** Prevents double-spending of orders. Once an order's fill reaches its `takeAsset.value`, it cannot be filled further.

### 1.8 Order Cancellation (Permanent)
- **Mechanism:** `fills[orderKeyHash] = UINT256_MAX` — an order with fill set to MAX can never be matched.
- **Purpose:** Provides irrevocable cancellation. The `calculateRemaining` function checks `fill < UINT256_MAX`.

### 1.9 ERC-20 Whitelist
- **Mechanism:** `allowedERC20Assets` mapping. Only whitelisted ERC-20 tokens can be traded.
- **Purpose:** Prevents trading of malicious or low-quality tokens. Reduces attack surface from arbitrary token contracts.

### 1.10 Asset Class Validation
- **Mechanism:** `LibOrder.validate` ensures only fungible-for-non-fungible trades.
- **Purpose:** Prevents economic attacks via unexpected asset class combinations (e.g., ERC-20 for ERC-20 or NFT-for-NFT).

### 1.11 Rounding Error Protection
- **Mechanism:** `LibMath.isRoundingErrorFloor` checks if rounding error >= 0.1% and reverts if so.
- **Purpose:** Prevents exploiting rounding errors in partial fill calculations to extract value.

### 1.12 Payout Sum Validation
- **Mechanism:** `require(sumBps == 10000)` — all payouts must sum to exactly 100%.
- **Purpose:** Ensures no funds are lost or created during payout distribution.

### 1.13 Royalty Cap
- **Mechanism:** `require(totalRoyaltiesBps <= 5000)` — royalties cannot exceed 50%.
- **Purpose:** Prevents a malicious royalty configuration from draining the entire trade amount.

### 1.14 Counterparty Verification
- **Mechanism:** `checkCounterparties` verifies that if `order.taker != address(0)`, the counterparty matches.
- **Purpose:** Prevents order front-running — if an order specifies a particular counterparty, only that counterparty can fill it.

### 1.15 tx.origin for Order Cancellation
- **Mechanism:** `require(tx.origin == order.maker)` in `cancelOrder`.
- **Purpose:** Ensures only the original order creator (as an EOA) can cancel their order. Prevents cancellation via smart contracts that might be acting maliciously.

---

## 2. Replication on Solana

### 2.1 Reentrancy Protection
**EVM:** `nonReentrant` modifier
**Solana:** **Not needed.** Solana's runtime locks the program account during execution. A program cannot call itself recursively within a single instruction. CPI calls to external programs cannot re-enter the calling program because the calling program's account is locked.

**Verification:** The Solana runtime provides this guarantee at the VM level. No additional code is needed.

### 2.2 Pausable
**EVM:** `whenNotPaused` modifier
**Solana:**
```rust
require!(!ctx.accounts.exchange_config.is_paused, ExchangeError::Paused);
```
Added as the first check in every instruction handler that should be pausable.

### 2.3 Signature Verification
**EVM:** EIP-712 + secp256k1 `ecrecover`
**Solana (Ed25519 approach):**
1. The signer creates a message hash: `SHA256(domain_prefix || borsh_serialize(order))`
   - `domain_prefix = program_id || "energi" || version_byte || cluster_byte`
2. The signer signs with Ed25519.
3. Before calling `match_orders`, the caller includes an `Ed25519SigVerify` instruction in the transaction.
4. The `match_orders` instruction introspects the previous instruction via `sysvar::instructions`:
   ```rust
   let ix = load_instruction_at_checked(index, &ctx.accounts.instructions_sysvar)?;
   // Verify it's an Ed25519 verify instruction
   // Extract the public key, message, and signature from the instruction data
   // Verify the public key matches the expected signer (order maker or order book)
   // Verify the message matches the expected order hash or matchAllowance hash
   ```

**Solana (secp256k1 approach — if EVM compatibility needed):**
Same pattern but using `Secp256k1SigVerify` precompile instruction. The EIP-712 hash can be preserved exactly.

### 2.4 EIP-1271 (Smart Contract Signatures)
**EVM:** CPI to maker contract's `isValidSignature`
**Solana:** Rare use case. If needed, implement as:
- Maker is a PDA controlled by a multisig program.
- The multisig program exposes an instruction that "approves" an order hash.
- The exchange program checks if the approval exists (via PDA state).

**Recommendation:** Defer this feature unless there's a concrete requirement. Focus on standard Ed25519 signatures from wallets.

### 2.5 Signature Malleability
**EVM:** Enforce `s` in lower half
**Solana:** Ed25519 signatures do not have the malleability issue that secp256k1 has. The Ed25519SigVerify precompile handles this internally. If using secp256k1, the Secp256k1SigVerify precompile also normalizes signatures.

**No additional code needed.**

### 2.6 Order Book matchAllowance
**EVM:** EIP-712 signed `(orderKeyHash, matchBeforeTimestamp)`, verified on-chain
**Solana:**
1. Order Book signs `SHA256(domain_prefix || order_key_hash || match_before_timestamp_le_bytes)` with its Ed25519 key.
2. An `Ed25519SigVerify` instruction is included for the Order Book signature.
3. The program verifies:
   ```rust
   let clock = Clock::get()?;
   require!(match_before_timestamp > clock.unix_timestamp, ExchangeError::MatchAllowanceExpired);
   // Introspect Ed25519 verify instruction to confirm Order Book pubkey signed the correct data
   ```

### 2.7 Order Fill Tracking
**EVM:** `fills[orderKeyHash]` mapping
**Solana:** `OrderFill` PDA account per order:
```rust
#[account]
pub struct OrderFill {
    pub fill_amount: u64,
    pub bump: u8,
}
// Seeds: ["order_fill", order_key_hash]
```

**Invariant enforcement:**
```rust
let new_fill = order_fill.fill_amount.checked_add(new_take_value)
    .ok_or(ExchangeError::FillOverflow)?;
order_fill.fill_amount = new_fill;
```

The `init_if_needed` constraint creates the account with `fill_amount = 0` on first use.

### 2.8 Order Cancellation
**EVM:** `fills[hash] = UINT256_MAX`
**Solana:**
```rust
order_fill.fill_amount = u64::MAX;
```
Check during matching:
```rust
require!(order_fill.fill_amount < u64::MAX, ExchangeError::OrderCancelled);
```

### 2.9 ERC-20 Whitelist
**EVM:** `allowedERC20Assets[addr]` mapping
**Solana:** `AllowedToken` PDA per mint:
- If the PDA exists and `is_allowed == true`, the token is whitelisted.
- The instruction must include this PDA as an account.
- Anchor constraint: `seeds = [b"allowed_token", mint.key().as_ref()], bump`

### 2.10 Asset Class Validation
**Solana:** Same validation logic as Solidity, implemented in Rust:
```rust
fn validate_order(order: &Order, clock: &Clock) -> Result<()> {
    // Time validation
    if order.start != 0 {
        require!(order.start < clock.unix_timestamp, ExchangeError::OrderNotStarted);
    }
    if order.end != 0 {
        require!(order.end > clock.unix_timestamp, ExchangeError::OrderExpired);
    }
    // Asset class compatibility
    match (&order.make_asset.asset_type.asset_class, &order.take_asset.asset_type.asset_class) {
        (AssetClass::Sol | AssetClass::WrappedSol | AssetClass::SplToken, 
         AssetClass::Nft | AssetClass::SemiFungible) => Ok(()),
        (AssetClass::Nft | AssetClass::SemiFungible,
         AssetClass::Sol | AssetClass::WrappedSol | AssetClass::SplToken) => Ok(()),
        _ => Err(ExchangeError::AssetClassMismatch.into()),
    }
}
```

### 2.11 Rounding Error Protection
**Solana:** Same math, using Rust checked operations:
```rust
fn safe_get_partial_amount_floor(numerator: u64, denominator: u64, target: u64) -> Result<u64> {
    require!(denominator > 0, ExchangeError::DivisionByZero);
    if target == 0 || numerator == 0 { return Ok(0); }
    let remainder = (target as u128 * numerator as u128) % denominator as u128;
    let is_error = remainder * 1000 >= (numerator as u128) * (target as u128);
    require!(!is_error, ExchangeError::RoundingError);
    Ok(((numerator as u128 * target as u128) / denominator as u128) as u64)
}
```

### 2.12 Payout Sum Validation
**Solana:** Identical check:
```rust
require!(sum_bps == 10000, ExchangeError::InvalidPayoutSum);
```

### 2.13 Royalty Cap
**Solana:** Identical check:
```rust
require!(total_royalties_bps <= 5000, ExchangeError::RoyaltiesTooHigh);
```

### 2.14 Counterparty Verification
**Solana:** Identical logic:
```rust
if order_left.taker != Pubkey::default() {
    require!(order_right.maker == order_left.taker, ExchangeError::CounterpartyMismatch);
}
if order_right.taker != Pubkey::default() {
    require!(order_right.taker == order_left.maker, ExchangeError::CounterpartyMismatch);
}
```

### 2.15 tx.origin for Cancellation
**EVM:** `tx.origin == order.maker`
**Solana:** The maker must be a transaction signer:
```rust
// In #[derive(Accounts)]
pub maker: Signer<'info>,
// In instruction handler
require!(ctx.accounts.maker.key() == order.maker, ExchangeError::NotOrderMaker);
```
This is actually stronger than `tx.origin` because it requires a direct signature, not just being the transaction originator.

---

## 3. New Risks Introduced by Solana Architecture

### 3.1 Account Substitution Attacks

**Risk:** An attacker passes a fake PDA account that contains manipulated data (e.g., a fake `ExchangeConfig` with zero protocol fee, or a fake `OrderFill` showing zero fill for a cancelled order).

**Mitigation:**
- **PDA seed verification:** Every PDA account must be verified against its expected seeds using Anchor's `seeds` and `bump` constraints.
- **Program ownership check:** Every PDA must be owned by the expected program. Anchor's `Account<'info, T>` type automatically checks the account discriminator and program ownership.
- **Example:**
  ```rust
  #[account(
      seeds = [b"order_fill", order_key_hash.as_ref()],
      bump = order_fill.bump,
  )]
  pub order_fill: Account<'info, OrderFill>,
  ```

### 3.2 PDA Spoofing

**Risk:** An attacker creates an account at the expected PDA address but with different data, or a PDA from a different program that happens to have the same address.

**Mitigation:**
- **Anchor's account discriminator:** Every Anchor account type has an 8-byte discriminator. If an account has the wrong discriminator, deserialization fails.
- **Program ownership:** Anchor verifies that the account is owned by the current program.
- **init_if_needed with proper seeds:** When creating PDA accounts, Anchor ensures the correct seeds are used.

### 3.3 CPI Privilege Escalation

**Risk:** A malicious program makes a CPI call to the exchange program, passing its own PDA as the signer, attempting to execute privileged operations.

**Mitigation:**
- **Signer verification:** For admin operations, the signer must match `config.owner` or `config.exchange_owner`. These are stored pubkeys, not arbitrary PDAs.
- **For order operations:** Signature verification (Ed25519) is done via sysvar introspection, which verifies the actual transaction signatures, not CPI-provided signers.
- **PDA signer for transfers:** When the exchange PDA signs CPI calls (e.g., to transfer tokens from a delegate position), the PDA seeds include the program ID, preventing other programs from generating the same signer.

### 3.4 Missing Account Attacks

**Risk:** A required account (e.g., royalty recipient's token account) is not passed to the instruction, causing transfers to fail or royalties to be skipped.

**Mitigation:**
- **Mandatory accounts:** Core accounts (`exchange_config`, `order_fill`, etc.) are required by Anchor's account struct.
- **Remaining accounts validation:** Dynamic accounts (royalty recipients, payout addresses) passed as `remaining_accounts` must be validated:
  ```rust
  // Validate the number of remaining accounts matches expected
  // Validate each account's ownership and derivation
  ```
- **Off-chain validation:** The Order Book service validates that all required accounts are included before signing the matchAllowance.

### 3.5 Compute Budget Exhaustion

**Risk:** A match with many royalty recipients, payout addresses, or complex fee structures exceeds the compute budget, causing the transaction to fail.

**Mitigation:**
- **Compute budget extension:** The transaction includes `set_compute_unit_limit(1_400_000)` instruction.
- **Limit royalty recipients:** Enforce a maximum number of royalty recipients (e.g., 10) and payout recipients (e.g., 5) per order.
- **Off-chain estimation:** The Order Book/relayer estimates compute cost before submitting.

### 3.6 Clock Manipulation

**Risk:** Solana's `Clock::get()?.unix_timestamp` could be slightly off, allowing expired matchAllowances to be used.

**Mitigation:**
- Solana's clock is within ~1-2 seconds of real time.
- matchAllowance timestamps should have sufficient buffer (e.g., 60+ seconds).
- This is similar to EVM's `block.timestamp` which validators can manipulate slightly.

### 3.7 Token Account Creation DoS

**Risk:** If the exchange needs to create Associated Token Accounts (ATAs) for recipients during a trade, an attacker could drain the payer's SOL by forcing many ATA creations.

**Mitigation:**
- Require all recipient ATAs to exist before the trade. The off-chain system can pre-create them.
- Alternatively, use `init_if_needed` but limit the number of ATA creations per instruction.
- The payer (relayer) pays for account creation — they can refuse to submit trades with too many new accounts.

### 3.8 Front-Running / MEV

**Risk:** Validators or MEV bots observe pending transactions and front-run order matches.

**Mitigation:**
- The **Order Book matchAllowance** mechanism is the primary defense — same as EVM. Only transactions with valid matchAllowance signatures can execute.
- matchAllowance is time-limited and specific to an order, so a front-runner cannot use it for a different order.
- Additionally, Solana's transaction ordering is less susceptible to traditional MEV compared to EVM (though Jito's MEV infrastructure exists).

### 3.9 Replay Attacks

**Risk:** A previously executed match transaction is replayed to double-fill an order.

**Mitigation:**
- **Fill tracking:** The `OrderFill` PDA records cumulative fills. Replaying a transaction would attempt to fill beyond the remaining amount, which would be rejected by `calculateRemaining`.
- **matchAllowance expiry:** The `match_before_timestamp` check prevents old matchAllowances from being reused after they expire.
- **Solana's transaction deduplication:** Solana natively rejects duplicate transactions within a recent blockhash window (~60-90 seconds).
- **Cross-cluster replay:** The domain prefix includes the cluster identifier, preventing replays from devnet to mainnet.

### 3.10 Integer Overflow/Underflow

**Risk:** Arithmetic operations could overflow or underflow, especially in fee calculations.

**Mitigation:**
- Rust's default behavior panics on overflow in debug mode.
- In release mode, use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` for all arithmetic.
- Anchor errors propagate cleanly.
- `u64` provides sufficient range for token amounts on Solana (max ~18.4 quintillion lamports).
- Use `u128` for intermediate multiplication results to prevent overflow: `(a as u128 * b as u128) / c as u128`.

---

## 4. Security Checklist for Implementation

### Replay Attacks
- [ ] Order fills are tracked in PDA accounts and monotonically increase
- [ ] matchAllowance timestamps are checked against `Clock::get()`
- [ ] Domain prefix includes program ID and cluster to prevent cross-program/cross-cluster replay
- [ ] Order cancellation sets fill to `u64::MAX` permanently

### Signature Validation
- [ ] Ed25519 signature verification via sysvar introspection for all orders with `salt > 0`
- [ ] Order Book matchAllowance signature verified for all orders with `salt > 0`
- [ ] For `salt == 0` orders, maker must be a direct transaction signer
- [ ] Message hash includes full order data (not just key hash) for order signatures
- [ ] matchAllowance message includes order key hash and timestamp

### Account Substitution Attacks
- [ ] All PDA accounts verified against expected seeds
- [ ] All accounts verified for correct program ownership (Anchor handles this)
- [ ] Account discriminators verified (Anchor handles this)
- [ ] Token accounts verified for correct mint
- [ ] Token accounts verified for correct owner

### PDA Spoofing
- [ ] PDA bumps are stored and verified
- [ ] `init_if_needed` uses correct seeds and program ID
- [ ] No PDAs from external programs are accepted without explicit verification

### CPI Privilege Escalation
- [ ] Admin instructions verify signer matches stored authority pubkey
- [ ] PDA signers use seeds that include unique identifiers
- [ ] Token transfers use the correct authority (user signer or exchange PDA delegate)
- [ ] External program IDs are verified against stored configuration

### Liquidity Accounting Invariants
- [ ] Protocol fee + royalties + origin fees + payouts = total amount (verified by payout sum = 10000 bps)
- [ ] No funds are created or destroyed during a trade
- [ ] Rounding errors are checked (< 0.1%) and handled deterministically
- [ ] Last payout recipient gets remainder to prevent dust loss
- [ ] Royalties capped at 50%
- [ ] Protocol fee is calculated on the original amount, not the post-fee amount

### Additional Solana-Specific Checks
- [ ] Compute budget is set appropriately for complex transactions
- [ ] All mutable accounts are marked as `mut`
- [ ] All signers are marked as `Signer`
- [ ] `system_program`, `token_program`, `associated_token_program` are verified as correct programs
- [ ] Remaining accounts (dynamic) are validated for correctness
- [ ] wSOL handling correctly wraps/unwraps and closes temporary accounts
- [ ] Account closures properly zero out data and return lamports
