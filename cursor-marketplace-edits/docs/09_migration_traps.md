# 09 — EVM → Solana Migration Traps

---

## Purpose

This document identifies specific risks, attack vectors, and implementation pitfalls that arise when porting Solidity contracts to Solana/Anchor programs. Each issue includes an assessment of whether the current migration plan is vulnerable, a prevention strategy, and enforcement rules for the implementation.

---

## 1. Replay Attacks Due to Nonce Model Differences

### Risk
On EVM, `salt` combined with EIP-712 domain separation (chainId + verifyingContract) prevents cross-chain and cross-contract replay. On Solana, there is no native EIP-712. If the domain prefix used in order hashing does not include the program ID and cluster, an order signed for devnet could be replayed on mainnet, or an order signed for one deployment could be replayed against another.

### Current Migration Plan Vulnerability
**Partially addressed.** The docs mention a domain prefix with `program_id + cluster`, but do not specify the exact byte layout or enforcement mechanism.

### Prevention in Anchor
- Include `program_id` (32 bytes) and a cluster discriminator byte in the message hash for all Ed25519 signature verification.
- The order hash message must be: `SHA256(program_id || cluster_byte || "energi" || version_byte || borsh_serialize(order))`.
- The matchAllowance hash must be: `SHA256(program_id || cluster_byte || "energi" || version_byte || order_key_hash || match_before_timestamp_le)`.

### Enforcement Rules
- **RULE R-1:** Every signature verification MUST include the program ID and cluster byte in the signed message.
- **RULE R-2:** The order key hash derivation MUST be deterministic and include `maker`, `make_asset_type_hash`, `take_asset_type_hash`, `salt`, and `collection_bid`.
- **RULE R-3:** Solana's native transaction deduplication (blockhash window) provides short-term replay protection; fill tracking provides long-term protection.

---

## 2. Signature Verification Differences (ECDSA vs Ed25519)

### Risk
EVM uses secp256k1 ECDSA with `ecrecover`. Solana natively uses Ed25519. If the implementation uses Ed25519 but does not correctly introspect the `Ed25519SigVerify` precompile instruction, an attacker could submit a transaction without valid signature verification instructions and the program would process it.

### Current Migration Plan Vulnerability
**Addressed conceptually.** The plan describes sysvar introspection but does not detail the exact byte-level parsing of the Ed25519 program instruction data.

### Prevention in Anchor
- Use `load_instruction_at_checked()` from `solana_program::sysvar::instructions` to load the Ed25519 verification instruction.
- Parse the Ed25519 instruction data format:
  - Bytes 0-1: number of signatures (u16 LE)
  - For each signature: offset to public key (u16), offset to signature (u16), offset to message (u16), message length (u16)
- Verify: (a) the instruction's program_id is `Ed25519SigVerify111111111111111111111111111`, (b) the public key matches the expected signer, (c) the message matches the expected hash.

### Enforcement Rules
- **RULE S-1:** For every order with `salt > 0`, the transaction MUST contain a preceding Ed25519SigVerify instruction for the order signature.
- **RULE S-2:** For every order with `salt > 0`, the transaction MUST contain a preceding Ed25519SigVerify instruction for the matchAllowance signature.
- **RULE S-3:** The program MUST verify the public key extracted from the Ed25519 instruction matches `order.maker` (for order sigs) or `config.order_book` (for matchAllowance sigs).
- **RULE S-4:** The program MUST verify the message extracted from the Ed25519 instruction matches the computed order hash or matchAllowance hash.
- **RULE S-5:** For orders with `salt == 0`, the maker MUST be a transaction signer (verified via `Signer` constraint or `is_signer` check).

---

## 3. PDA Spoofing Risks

### Risk
An attacker passes an account that looks like a valid PDA but was created by a different program or contains manipulated data. For example, a fake `OrderFill` account with `fill_amount = 0` for an order that was actually cancelled (`fill_amount = u64::MAX`).

### Current Migration Plan Vulnerability
**Addressed.** Anchor's `Account<'info, T>` type checks the 8-byte discriminator and program ownership automatically. PDA seed verification via `seeds` constraint ensures correct derivation.

### Prevention in Anchor
- Always use `#[account(seeds = [...], bump = ...)]` constraints.
- Store the bump in the account data and verify it on subsequent accesses.
- Never accept a PDA account without seed verification.

### Enforcement Rules
- **RULE P-1:** Every PDA account MUST use `seeds` and `bump` constraints in the `#[derive(Accounts)]` struct.
- **RULE P-2:** The bump MUST be stored in the account data and verified on every access (Anchor does this automatically with `bump = account.bump`).
- **RULE P-3:** Never use `UncheckedAccount` for PDA accounts that carry protocol state. Use `Account<'info, T>` with proper type checking.

---

## 4. Account Substitution Attacks

### Risk
An attacker substitutes legitimate accounts with malicious ones. Examples:
- Substituting the royalty recipient's token account with the attacker's account.
- Substituting the payment mint account with a different mint.
- Passing a wrong `AllowedToken` PDA to bypass the whitelist.
- Passing a wrong `FeeReceiver` PDA to redirect protocol fees.

### Current Migration Plan Vulnerability
**Partially addressed.** The plan mentions remaining_accounts validation but does not detail how to verify each dynamic account.

### Prevention in Anchor
- For token accounts: verify `token_account.mint == expected_mint` and `token_account.owner == expected_owner`.
- For PDAs in remaining_accounts: derive the expected PDA address from seeds and verify it matches the provided account's key.
- For mints: verify the account is owned by the Token Program.

### Enforcement Rules
- **RULE A-1:** Every token account MUST have its `mint` and `owner` verified against expected values.
- **RULE A-2:** Every PDA in `remaining_accounts` MUST be verified by re-deriving the PDA address from known seeds and comparing with the account's key.
- **RULE A-3:** The `AllowedToken` PDA MUST be derived from the payment mint's pubkey and verified as existing and `is_allowed == true`.
- **RULE A-4:** The `FeeReceiver` PDA (if present) MUST be derived from the payment mint's pubkey.
- **RULE A-5:** The `ExchangeConfig` account MUST be verified via PDA seed `["exchange_config"]`.

---

## 5. CPI Privilege Escalation

### Risk
A malicious program invokes the exchange program via CPI, passing crafted signer PDAs to bypass access control. For example, a malicious contract could call `set_protocol_fee_bps` with its own PDA as the "exchange_owner" signer.

### Current Migration Plan Vulnerability
**Addressed.** Admin instructions require the signer to match a stored pubkey in `ExchangeConfig`. CPI callers cannot forge these signatures.

### Prevention in Anchor
- Admin instructions check `signer.key() == config.exchange_owner` or `signer.key() == config.owner`.
- These are `Signer<'info>` accounts, meaning the runtime verifies the signature at the transaction level.
- CPI-injected signers (PDA signers from other programs) will have different pubkeys than the stored authority.

### Enforcement Rules
- **RULE C-1:** Admin instructions MUST use `Signer<'info>` constraint AND verify the signer's key matches the stored authority.
- **RULE C-2:** Never rely solely on account ownership for authorization; always check specific pubkey matches.
- **RULE C-3:** The exchange PDA authority (for CPI token transfers) MUST use seeds that include the program ID (inherent in PDA derivation).

---

## 6. Liquidity Accounting Inconsistencies

### Risk
Rounding differences between EVM (uint256) and Solana (u64) can cause accounting discrepancies. On EVM, `uint256` provides 78 decimal digits of precision. On Solana, `u64` provides ~19.3 digits. Intermediate calculations may overflow or lose precision.

### Current Migration Plan Vulnerability
**Partially addressed.** The docs mention using `u128` for intermediate calculations but don't enforce it systematically.

### Prevention in Anchor
- Use `u128` for all intermediate multiplication results: `(a as u128 * b as u128) / c as u128`.
- Use `checked_*` operations for all arithmetic.
- Port the `safeGetPartialAmountFloor` rounding error check exactly from Solidity.

### Enforcement Rules
- **RULE L-1:** All fee calculations (bps, partial amounts) MUST use `u128` intermediate values to prevent overflow.
- **RULE L-2:** All arithmetic MUST use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` or return explicit errors on overflow.
- **RULE L-3:** The `safe_get_partial_amount_floor` function MUST implement the same 0.1% rounding error check as the Solidity version.
- **RULE L-4:** The last payout recipient MUST receive the remainder (not a calculated amount) to prevent dust loss.
- **RULE L-5:** Total outgoing transfers MUST equal total incoming transfers for every trade.

---

## 7. Incorrect Account Ownership Validation

### Risk
On Solana, any program can own accounts. An attacker could create an account with the right data layout but owned by a different program. Anchor's `Account<'info, T>` checks the discriminator but only if the account is not deserialized manually.

### Current Migration Plan Vulnerability
**Addressed by Anchor's type system.** `Account<'info, T>` verifies ownership automatically.

### Prevention in Anchor
- Use `Account<'info, T>` for all typed accounts (never `AccountInfo` for state accounts).
- For token accounts, use `anchor_spl::token::TokenAccount` which verifies ownership by the Token Program.
- For mint accounts, use `anchor_spl::token::Mint`.

### Enforcement Rules
- **RULE O-1:** Protocol state accounts MUST use `Account<'info, T>` (not raw `AccountInfo`).
- **RULE O-2:** SPL token accounts MUST use `anchor_spl::token::TokenAccount` or `InterfaceAccount`.
- **RULE O-3:** Mint accounts MUST use `anchor_spl::token::Mint` or `InterfaceMint`.
- **RULE O-4:** Program accounts (`token_program`, `system_program`) MUST use `Program<'info, T>` constraint.

---

## 8. Incorrect Signer Validation

### Risk
Failing to verify that critical accounts are signers allows unauthorized operations. On EVM, `msg.sender` is implicit. On Solana, signer status must be explicitly checked. For example, if the `maker` account in `cancel_order` is not required to be a `Signer`, anyone could cancel anyone's orders.

### Current Migration Plan Vulnerability
**Addressed.** The plan specifies `Signer` for the maker in `cancel_order` and for authority in admin instructions.

### Prevention in Anchor
- Use `Signer<'info>` for all accounts that must be transaction signers.
- Combine with pubkey checks: `require!(signer.key() == expected_key)`.

### Enforcement Rules
- **RULE SG-1:** `cancel_order` MUST require `maker: Signer<'info>` AND verify `maker.key() == order.maker`.
- **RULE SG-2:** Admin instructions MUST require appropriate authority as `Signer<'info>`.
- **RULE SG-3:** For `salt == 0` orders, the maker MUST be a transaction `Signer`.
- **RULE SG-4:** The `payer` in `match_orders` MUST be a `Signer` (to authorize SOL spending).

---

## 9. Incorrect PDA Seed Validation

### Risk
Using incorrect or incomplete seeds for PDA derivation can lead to collisions or mismatches. For example, if `OrderFill` uses only the first 16 bytes of the order key hash as a seed, different orders could collide on the same PDA.

### Current Migration Plan Vulnerability
**Low risk.** Seeds are well-defined in the docs, but implementation must follow them exactly.

### Prevention in Anchor
- Use the full 32-byte order key hash as a seed for `OrderFill`.
- Use the full 32-byte mint pubkey as a seed for `AllowedToken` and `FeeReceiver`.
- Document and enforce seed layouts.

### Enforcement Rules
- **RULE PD-1:** `OrderFill` PDA seeds: `["order_fill", order_key_hash(32 bytes)]`.
- **RULE PD-2:** `AllowedToken` PDA seeds: `["allowed_token", mint.key().as_ref()]`.
- **RULE PD-3:** `FeeReceiver` PDA seeds: `["fee_receiver", mint.key().as_ref()]`.
- **RULE PD-4:** `ExchangeConfig` PDA seeds: `["exchange_config"]`.
- **RULE PD-5:** All PDAs in the royalties-registry MUST use the exact seeds defined in doc 04.

---

## 10. Oracle Manipulation Risks

### Risk
The royalties registry is an oracle for royalty data. If an attacker can manipulate royalty entries to set royalties to 50% (the cap) and direct them to their own address, they can siphon funds from every trade involving that collection.

### Current Migration Plan Vulnerability
**Mitigated by access control.** Only the registry owner or collection authority can set royalties. However, if the authority check is incorrect, this becomes exploitable.

### Prevention in Anchor
- Verify collection authority via Metaplex metadata `update_authority`.
- Require the authority to be a transaction signer.
- Enforce royalty sum ≤ 10000 bps at write time AND enforce ≤ 5000 bps cap at trade time.

### Enforcement Rules
- **RULE OR-1:** `set_royalties_by_collection` MUST verify the signer is either `registry_config.owner` OR the collection's `update_authority` from Metaplex metadata.
- **RULE OR-2:** `set_owner_royalties_by_token` MUST verify the same.
- **RULE OR-3:** `set_creator_royalties_by_token` MUST verify the signer is either `registry_config.owner` OR a verified creator from Metaplex metadata.
- **RULE OR-4:** Royalty recipient addresses MUST NOT be `Pubkey::default()` (zero address).
- **RULE OR-5:** The exchange MUST enforce `total_royalties_bps <= 5000` regardless of what the registry returns.

---

## 11. Incorrect Token Authority Configuration

### Risk
SPL token transfers require the correct authority (signer or delegate). If the exchange program uses the wrong authority for CPI token transfers, transfers will fail. Worse, if authority validation is skipped, an attacker could drain tokens from accounts they don't own.

### Current Migration Plan Vulnerability
**Addressed conceptually.** The docs describe using user signers for `salt == 0` and delegation for `salt > 0` orders.

### Prevention in Anchor
- For `salt == 0` (taker present as signer): use the taker's signature directly as the authority.
- For `salt > 0` (maker not present): the maker must have pre-delegated their token account to the exchange PDA. The exchange PDA signs via `invoke_signed`.
- Verify delegation before attempting transfer.

### Enforcement Rules
- **RULE T-1:** For taker-side transfers where taker is a signer: use taker as authority.
- **RULE T-2:** For maker-side transfers where maker is NOT a signer (salt > 0): use exchange PDA as authority (maker must have delegated).
- **RULE T-3:** The exchange PDA authority MUST be derived from seeds `["exchange_authority"]` and signed via `invoke_signed` with the correct bump.
- **RULE T-4:** Never transfer tokens without verifying that the authority is correct for the source account.

---

## 12. Tick Array Substitution Attacks (Whirlpool)

### Risk
Not applicable to this protocol. The Energi GMI marketplace does not use Orca Whirlpool or any AMM. It is a peer-to-peer order-matching exchange.

### Current Migration Plan Vulnerability
**Not applicable.**

---

## 13. Unsafe remaining_accounts Usage

### Risk
The `match_orders` instruction uses `remaining_accounts` for dynamic accounts (royalty recipients, payout addresses, NFT token accounts). If these are not validated, an attacker could:
- Pass a fake royalty account that steals funds.
- Pass the wrong payout destination.
- Skip royalty accounts entirely, avoiding royalty payments.

### Current Migration Plan Vulnerability
**Partially addressed.** The docs mention validating remaining_accounts but lack a concrete validation schema.

### Prevention in Anchor
- Define a strict ordering for remaining_accounts.
- Validate each account against expected values derived from the order data.
- Use a structured parser that maps remaining_accounts to typed references.

### Enforcement Rules
- **RULE RA-1:** Define a fixed layout for remaining_accounts:
  ```
  [0]: maker's NFT token account (for NFT side)
  [1]: taker's NFT token account (for NFT side)  
  [2]: NFT mint account
  [3]: payment mint account
  [4]: AllowedToken PDA
  [5]: maker's payment token account  
  [6]: taker's payment token account
  [7]: fee_receiver payment token account
  [8..8+N]: royalty recipient payment token accounts
  [8+N..]: payout recipient payment token accounts
  Optionally: royalty PDA accounts from the registry
  ```
- **RULE RA-2:** Every token account in remaining_accounts MUST have its `mint` verified.
- **RULE RA-3:** Every token account in remaining_accounts MUST have its `owner` verified against expected recipients (from order data, royalty data, or config).
- **RULE RA-4:** The number of remaining_accounts MUST match the expected count derived from order data (number of royalties + number of payouts + fixed accounts).

---

## 14. Incorrect Integer Rounding Differences

### Risk
Solidity uses `uint256` with floor division. Rust's `u64` has the same floor division semantics, but the reduced precision (64 bits vs 256 bits) means intermediate values can overflow. Additionally, the order of operations matters: `(a * b) / c` must use `u128` intermediates.

### Current Migration Plan Vulnerability
**Partially addressed.** The plan mentions `u128` but doesn't enforce it for every calculation.

### Prevention in Anchor
- Use `u128` for every multiplication before division.
- Port `safeGetPartialAmountFloor` exactly with the modular arithmetic rounding check.
- The `bps` function: `(value as u128 * bps_value as u128 / 10000) as u64`.

### Enforcement Rules
- **RULE IR-1:** `bps(value, bps_value)` MUST compute `(value as u128 * bps_value as u128 / 10000u128) as u64`.
- **RULE IR-2:** `safe_get_partial_amount_floor(num, den, target)` MUST check: `(target as u128 * num as u128) % den as u128 * 1000 < num as u128 * target as u128`.
- **RULE IR-3:** `sub_fee(value, fee)` MUST cap the fee at the available value (never underflow).
- **RULE IR-4:** Fill calculations MUST use `u128` intermediates for `makeValue * takeValue / totalTakeValue`.

---

## 15. Missing Pause Check

### Risk
If an instruction handler forgets to check the `is_paused` flag, an attacker can execute operations during an emergency pause.

### Enforcement Rules
- **RULE PC-1:** `match_orders`, `batch_match_orders`, `cancel_order`, `batch_cancel_orders`, and `match_collection_bid_order` MUST check `require!(!config.is_paused, ExchangeError::Paused)` as the FIRST operation.
- **RULE PC-2:** Admin instructions (fee changes, whitelist changes) do NOT need pause checks (admin must be able to configure during pause).

---

## 16. Fill Monotonicity Violation

### Risk
If the program allows fill amounts to decrease, an order could be re-filled after partial execution, leading to overspend.

### Enforcement Rules
- **RULE FM-1:** `OrderFill.fill_amount` MUST only increase. The new fill MUST be computed as `old_fill + new_take_value`.
- **RULE FM-2:** Before updating fill, verify `old_fill < u64::MAX` (not cancelled).
- **RULE FM-3:** Verify `new_take_value > 0` (nothing to fill otherwise).
- **RULE FM-4:** Use `checked_add` to prevent overflow when computing new fill.

---

## 17. SOL/wSOL Conversion Pitfalls

### Risk
wSOL on Solana works differently from WETH on EVM. Creating/closing wSOL token accounts has nuances:
- Wrapping: fund an ATA with SOL, then call `sync_native`.
- Unwrapping: close the wSOL ATA; lamports are returned to the owner.
- If a temporary wSOL account is not properly closed, SOL can be locked.

### Enforcement Rules
- **RULE W-1:** Temporary wSOL accounts created during `match_orders` MUST be closed before the instruction ends.
- **RULE W-2:** When closing wSOL accounts, verify the destination is correct (the rightful owner).
- **RULE W-3:** Protocol fees for wSOL trades should be converted to SOL by closing a temporary wSOL account.
- **RULE W-4:** The `PROXY_WETH_ASSET_CLASS` concept from EVM is replaced by tracking whether wSOL was already transferred to a program-owned temporary account.

---

## 18. Collection Bid Integrity

### Risk
Collection bids create synthetic maker orders with `salt = 0` and `collectionBid = true`. These bypass signature verification. If the synthetic order creation is incorrect, an attacker could:
- Create synthetic orders with inflated values.
- Match against unfavorable prices.
- Drain the collection bidder's funds.

### Enforcement Rules
- **RULE CB-1:** Synthetic maker orders MUST have `salt = 0` and `collectionBid = true`.
- **RULE CB-2:** Synthetic maker order values MUST be derived from the remaining taker order values.
- **RULE CB-3:** The price ratio check MUST be enforced: `collection_bid.take_value / collection_bid.make_value == sum_taker_make / sum_taker_take`.
- **RULE CB-4:** Taker orders in a collection bid MUST have `collectionBid = false`.
- **RULE CB-5:** Taker orders' make asset token address MUST match the collection bid's take asset token address.

---

## 19. Compute Budget Exhaustion

### Risk
Complex trades with many royalty recipients, payout addresses, and origin fees may exceed Solana's compute budget, causing the transaction to fail after consuming compute units (CU) but before completing all transfers. This leaves state partially updated (fills updated but transfers incomplete).

### Enforcement Rules
- **RULE CU-1:** Limit royalty recipients to a maximum of 10 per trade.
- **RULE CU-2:** Limit payout recipients to a maximum of 10 per order.
- **RULE CU-3:** Limit origin fee recipients to a maximum of 5 per order.
- **RULE CU-4:** All state updates (fills) and transfers MUST be atomic within a single instruction. If any transfer fails, the entire instruction reverts (Solana provides this by default).

---

## 20. Cross-Program Account Confusion

### Risk
The exchange program reads royalty PDA accounts that are owned by the royalties-registry program. If the exchange does not verify these accounts are owned by the correct registry program, a fake registry could provide manipulated royalty data.

### Enforcement Rules
- **RULE XP-1:** When reading royalty PDA accounts from remaining_accounts, verify `account.owner == config.royalties_registry_program`.
- **RULE XP-2:** Re-derive the expected PDA address using the registry program's ID and verify it matches the provided account's key.
- **RULE XP-3:** If a royalty PDA account is not found (account doesn't exist or is wrong), treat it as "no royalties" for that lookup tier, not as an error.

---

## Summary: Implementation Enforcement Checklist

Every instruction implementation MUST be checked against the following rules before being considered complete:

### For ALL instructions:
- [ ] PDA seeds verified (RULE P-1, P-2, PD-*)
- [ ] Account ownership verified (RULE O-*)
- [ ] Signer validation correct (RULE SG-*)

### For `match_orders`:
- [ ] Pause check as first operation (RULE PC-1)
- [ ] Signature verification via sysvar introspection (RULE S-1 through S-5)
- [ ] Domain prefix includes program_id + cluster (RULE R-1)
- [ ] Order key hash is deterministic (RULE R-2)
- [ ] Asset class compatibility validated (fungible ↔ non-fungible)
- [ ] Token whitelist checked via AllowedToken PDA (RULE A-3)
- [ ] Counterparty constraints enforced
- [ ] Fill tracking is monotonic (RULE FM-*)
- [ ] All arithmetic uses u128 intermediates (RULE L-1, IR-*)
- [ ] Rounding error checked (RULE L-3)
- [ ] Royalties capped at 50% (RULE OR-5)
- [ ] Payouts sum to 10000 bps
- [ ] Last payout gets remainder (RULE L-4)
- [ ] Token accounts verified for mint and owner (RULE A-1, RA-2, RA-3)
- [ ] Token authority is correct for transfers (RULE T-*)
- [ ] SOL/wSOL conversion handles temp accounts (RULE W-*)
- [ ] Remaining accounts validated (RULE RA-*)
- [ ] Cross-program royalty accounts verified (RULE XP-*)
- [ ] Events emitted

### For `cancel_order`:
- [ ] Pause check (RULE PC-1)
- [ ] Maker is signer (RULE SG-1)
- [ ] Maker matches order.maker
- [ ] Salt != 0
- [ ] Fill set to u64::MAX

### For admin instructions:
- [ ] Correct authority is signer (RULE C-1, SG-2)
- [ ] Authority matches stored config value
- [ ] Input validation (e.g., protocol_fee_bps <= 10000)

### For royalties-registry:
- [ ] Authority verification via Metaplex metadata (RULE OR-1, OR-2, OR-3)
- [ ] No zero-address recipients (RULE OR-4)
- [ ] Royalty sum ≤ 10000 bps at write time
