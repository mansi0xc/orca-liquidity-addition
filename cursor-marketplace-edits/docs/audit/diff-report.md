# Phase 3 — Strict Equivalence Diff Report

This document compares the EVM and Solana implementations feature-by-feature and identifies ALL deviations.

---

## CRITICAL ISSUES

### DIFF-C1: Royalties Not Verified On-Chain

**Severity: CRITICAL**

**EVM Behavior:**
The Exchange calls `RoyaltiesRegistry.getRoyalties(token, tokenId)` on-chain, resolving royalties through a multi-tier lookup (owner → creator → provider → contract). Royalties are trustlessly enforced.

**Solana Behavior:**
Royalties are passed as `royalty_parts` in `MatchOrdersArgs` by the caller. The code contains a TODO: `"add on-chain verification against the royalties registry PDA"`. No CPI call to the royalties registry program occurs.

**Deviation:**
An attacker (or malicious orderbook operator) can pass empty royalty_parts or arbitrary values, bypassing all royalty payments. This is an economic exploit that steals from creators.

**Fix Required:**
- Perform CPI to `royalties_registry` program to fetch on-chain royalties, OR
- Pass royalties registry PDAs in remaining_accounts and deserialize+verify them on-chain

---

### DIFF-C2: Fee Receiver Not Validated

**Severity: CRITICAL**

**EVM Behavior:**
Protocol fee is sent to `storage.getFeeReceiver(tokenAddress)`. If no per-token receiver is set, it falls back to `defaultFeeReceiver`. Both are on-chain values that cannot be manipulated by callers.

**Solana Behavior:**
The fee receiver destination is `remaining_accounts[1]` — an arbitrary account passed by the caller. There is no validation that this account matches `config.default_fee_receiver` or a `FeeReceiver` PDA.

**Deviation:**
An attacker can redirect all protocol fees to their own account by providing a controlled account at index 1.

**Fix Required:**
- Validate that the fee receiver account matches the on-chain `default_fee_receiver` or the `FeeReceiver` PDA for the relevant token mint

---

### DIFF-C3: Payout Destinations Not Validated

**Severity: CRITICAL**

**EVM Behavior:**
Payouts are sent to addresses specified in the order's `DataV1.payouts`. These addresses are cryptographically bound to the order through the maker's signature.

**Solana Behavior:**
Payout destinations come from `remaining_accounts`. The code walks through accounts sequentially but never validates that the destination accounts match the `payouts[i].account` from the order data.

**Deviation:**
An attacker can provide their own accounts as payout destinations, redirecting payments meant for order makers/takers. While the order data (with correct addresses) is signed, the actual transfer targets are unverified `remaining_accounts`.

**Fix Required:**
- For each payout, validate that `remaining_accounts[i].key() == payout.account` (or the correct ATA for SPL tokens)

---

### DIFF-C4: No Token Whitelist Enforcement in match_orders

**Severity: CRITICAL**

**EVM Behavior:**
`ExchangeHelper.checkERC20TokensAllowed` verifies all ERC20 assets against `allowedERC20Assets` mapping before any match proceeds.

**Solana Behavior:**
The `AllowedToken` PDA is created/updated by `set_allowed_token`, but `match_orders` never reads or checks it. Any SPL token can be used in trades.

**Deviation:**
Unwhitelisted, potentially malicious or worthless tokens can be used in trades, enabling price manipulation and scam trading.

**Fix Required:**
- Pass `AllowedToken` PDA as an account in `MatchOrders` context
- Verify `allowed_token.is_allowed == true` for any `SplToken` asset class

---

### DIFF-C5: Origin Fee and Royalty Destinations Not Validated

**Severity: CRITICAL**

**EVM Behavior:**
Origin fee and royalty recipients are addresses from on-chain order data and registry data respectively. Transfers go directly to those addresses.

**Solana Behavior:**
All recipient accounts come from `remaining_accounts` via the `AccountWalker`. None are validated against the addresses in order data or royalty parts.

**Deviation:**
Same as DIFF-C3 — an attacker can substitute any account for any recipient.

**Fix Required:**
- Validate each royalty destination matches `royalty_parts[i].account` (or its ATA)
- Validate each origin fee destination matches the corresponding `origin_fees[i].account`

---

## HIGH SEVERITY ISSUES

### DIFF-H1: No Collection Bid Support

**Severity: HIGH**

**EVM Behavior:**
`ExchangeHelper.matchCollectionBidOrder` processes collection-wide buy orders, matching one maker order against multiple taker orders. This is a core marketplace feature.

**Solana Behavior:**
The `match_orders` handler explicitly rejects `collection_bid == true` orders. Validation functions exist (`validate_collection_bid_maker_order`, `validate_collection_bid_taker_order`) but there is no instruction to use them.

**Deviation:**
Collection bids are completely non-functional. Users cannot place "buy any NFT from collection X" orders.

**Fix Required:**
- Add `match_collection_bid_orders` instruction
- Implement the multi-order matching logic from `ExchangeHelper._matchCollectionBidOrder`

---

### DIFF-H2: No Batch Match Orders

**Severity: HIGH**

**EVM Behavior:**
`Exchange.batchMatchOrders` accepts arrays of orders and matches them pairwise in a single transaction.

**Solana Behavior:**
No batch instruction exists. Only single-pair matching is supported.

**Deviation:**
Gas/compute efficiency is reduced. Atomic batch settlement is not possible.

**Fix Required:**
- Add `batch_match_orders` instruction (may need to be iterative due to Solana compute limits)

---

### DIFF-H3: No Wrapped SOL Auto-Conversion

**Severity: HIGH**

**EVM Behavior:**
`processEthAndWeth` handles all ETH↔WETH conversion scenarios automatically:
- Taker sends ETH, maker wants WETH → wrap
- Taker sends WETH, maker wants ETH → unwrap
- Maker sends WETH, taker wants ETH → unwrap
- Automatic proxy fund routing

**Solana Behavior:**
No SOL↔wSOL conversion exists. The `WrappedSol` asset class is defined but there's no wrapping/unwrapping logic.

**Deviation:**
Users must manually wrap/unwrap SOL, degrading UX and breaking some trade flows.

**Fix Required:**
- Implement SOL ↔ wSOL conversion logic
- Handle the case where one side specifies Sol and the other WrappedSol

---

### DIFF-H4: NFT Transfer Lacks Value==1 Check

**Severity: HIGH**

**EVM Behavior:**
`require(_asset.value == 1, 'Exchange: can only transfer one ERC721')` enforced for ERC721 transfers.

**Solana Behavior:**
No check that `value == 1` for `AssetClass::Nft` transfers. The SPL token program handles the actual amount, but the fill calculation could be manipulated if value != 1.

**Deviation:**
Could allow incorrect fill calculations for NFTs where value should always be 1.

**Fix Required:**
- Add `require!(asset.value == 1)` check for `Nft` asset class

---

### DIFF-H5: SOL Transfer Source Not Validated as Signer

**Severity: HIGH**

**EVM Behavior:**
ETH transfers use `payable(_to).call{value: amount}` which is safe because the contract holds the ETH.

**Solana Behavior:**
SOL transfers use `system_instruction::transfer` with `invoke` (not `invoke_signed`). This requires the source to be a signer. But the source comes from `remaining_accounts` which may not be a signer, and even if the payer is a signer, the transfer instruction uses arbitrary source accounts.

**Deviation:**
SOL transfer calls may fail at runtime because the source is not a signer, OR they may succeed but from the wrong account.

**Fix Required:**
- Ensure SOL transfer source is either the payer (who is a signer) or use a PDA with `invoke_signed`
- The exchange authority PDA should hold SOL and use `invoke_signed` for distributions

---

## MEDIUM SEVERITY ISSUES

### DIFF-M1: Order Key Hash Uses SHA-256, Not Keccak-256

**Severity: MEDIUM**

**EVM Behavior:**
`keccak256(abi.encode(maker, makeAssetHash, takeAssetHash, salt, collectionBid))`

**Solana Behavior:**
`SHA256(maker || makeAssetTypeHash || takeAssetTypeHash || salt || collectionBid)`

**Deviation:**
This is an acceptable cryptographic difference (SHA-256 is secure), but:
1. The encoding format differs: EVM uses ABI-encoded padded words, Solana uses raw concatenation
2. Asset type hashing differs: EVM hashes `(ASSET_TYPE_TYPEHASH, assetClass, keccak256(data))` via ABI encoding; Solana hashes `(class_byte, mint, token_id)` directly
3. This means order key hashes are different between chains — acceptable for cross-chain isolation but must be documented

**No Fix Required** (by design), but domain separation between chains is effectively achieved.

---

### DIFF-M2: Salt is u64, Not uint256

**Severity: MEDIUM**

**EVM Behavior:**
`salt` is `uint256` (256-bit), providing 2^256 possible values.

**Solana Behavior:**
`salt` is `u64` (64-bit), providing only 2^64 possible values.

**Deviation:**
Reduced salt space means higher collision probability. With birthday paradox, collisions become likely around 2^32 (~4 billion) orders. For most practical purposes this is adequate, but it differs from the EVM spec.

**No Fix Required** for practical purposes, but should be documented.

---

### DIFF-M3: Timestamp Types Differ

**Severity: MEDIUM**

**EVM Behavior:**
`start` and `end` are `uint256`.

**Solana Behavior:**
`start` and `end` are `i64`.

**Deviation:**
i64 covers timestamps until ~year 292 billion. Functionally equivalent. But the signedness means negative timestamps are theoretically possible (though meaningless).

**No Fix Required**, but validate that negative timestamps are handled correctly (they should fail the `start < current_timestamp` check).

---

### DIFF-M4: Zero-Salt Maker Assignment Not Implemented

**Severity: MEDIUM**

**EVM Behavior:**
When `salt == 0` and `order.maker == address(0)`, the maker is set to `tx.origin`:
```solidity
if (_order.maker != address(0)) {
    require(_callerAddress == _order.maker, ...);
} else {
    _order.maker = _callerAddress;
}
```

**Solana Behavior:**
When `salt == 0` and `order.maker != Pubkey::default()`, the payer must equal maker. When `maker == Pubkey::default()`, the check is simply skipped — but **maker is NOT reassigned to payer**.

**Deviation:**
On EVM, a zero-salt order with zero maker gets the maker field populated with the caller. On Solana, the maker stays as `Pubkey::default()`, which means:
- Order key hash will use the default pubkey
- Payouts will default to the default pubkey
- Fill tracking will be wrong

**Fix Required:**
- Mutably update `order.maker` to `payer.key()` when `salt == 0 && maker == Pubkey::default()`

---

### DIFF-M5: cancel_order Uses maker as Signer, EVM Uses tx.origin

**Severity: MEDIUM**

**EVM Behavior:**
`require(tx.origin == order.maker)` — the EOA initiating the transaction must be the maker.

**Solana Behavior:**
`maker: Signer<'info>` with `require!(maker.key() == args.order.maker)` — direct signer check.

**Deviation:**
EVM uses `tx.origin` which allows cancellation through intermediary contracts. Solana uses direct signer which is more restrictive but arguably more secure. This is an acceptable security improvement.

**No Fix Required** — Solana approach is stricter and safer.

---

### DIFF-M6: Fill Overflow Semantics Differ for u64 vs uint256

**Severity: MEDIUM**

**EVM Behavior:**
`UINT256_MAX` (2^256 - 1) as the cancellation sentinel. Fill values can be up to 2^256 - 2.

**Solana Behavior:**
`u64::MAX` (2^64 - 1) as the cancellation sentinel. Fill values can be up to 2^64 - 2.

**Deviation:**
Asset values exceeding u64::MAX (~18.4 quintillion) cannot be represented. For practical token amounts this is sufficient, but high-decimal tokens could theoretically overflow.

**No Fix Required** for practical purposes.

---

### DIFF-M7: FeeSide Missing PROXY_WETH Equivalent

**Severity: MEDIUM**

**EVM Behavior:**
`getFeeSide` checks for `PROXY_WETH_ASSET_CLASS` alongside `WETH_ASSET_CLASS`.

**Solana Behavior:**
Only checks `WrappedSol`. No `ProxyWrappedSol` concept exists.

**Deviation:**
Since Solana doesn't have the proxy pattern or PROXY_WETH concept, this is expected. But if wrapped SOL is ever held by the exchange authority PDA and re-distributed, a separate class may be needed.

**No Fix Required** unless wrapped SOL proxy pattern is added.

---

### DIFF-M8: ERC-1271 Contract Signature Verification Not Supported

**Severity: MEDIUM**

**EVM Behavior:**
If the maker is a smart contract, ERC-1271 `isValidSignature` is used as fallback.

**Solana Behavior:**
Only Ed25519 signature verification. Program-owned accounts cannot sign Ed25519 signatures.

**Deviation:**
Multisig wallets and smart contract wallets on Solana use different patterns (e.g., Squads). There's no equivalent to ERC-1271 CPI call.

**Partial Fix:**
- Document that program-owned accounts must use salt==0 (direct signer) approach
- Consider supporting Squads multisig verification in future

---

## FUNCTIONAL GAPS

### DIFF-F1: No getOrderFill / getOrdersFills View Functions

**Severity: LOW**

**EVM**: External view functions for querying fills.
**Solana**: Fills can be read directly from PDA accounts. No instruction needed.

**No Fix Required** — Solana accounts are natively queryable.

---

### DIFF-F2: No Batch Cancel

**Severity: LOW**

**EVM**: `ExchangeHelper.batchCancelOrders` allows batch cancellation.
**Solana**: Only single cancel instruction.

**Fix Recommended** but not critical — can be done via multiple instructions in one transaction.

---

### DIFF-F3: No Royalties Provider Extraction

**Severity: MEDIUM**

**EVM**: `RoyaltiesRegistry.providerExtractor` supports:
- External royalty providers
- Rarible V1/V2 interfaces
- LooksRare fee info

**Solana**: `RoyaltyProvider` PDA exists but is never read during match execution. No CPI to providers.

**Fix Required**: At minimum, read collection/token royalties from the registry.

---

### DIFF-F4: No ERC-2981 Equivalent Fallback

**Severity: MEDIUM**

**EVM**: Falls back to ERC-2981 `royaltyInfo()` if registry returns empty royalties.

**Solana**: No equivalent. Metaplex token metadata has royalty fields but they're not queried.

**Fix Recommended**: Consider reading Metaplex metadata royalty fields as fallback.

---

## INVARIANT VERIFICATION

| Invariant | EVM | Solana | Status |
|-----------|-----|--------|--------|
| INV-1: Fungible↔Non-Fungible only | ✅ Enforced | ✅ Enforced | PASS |
| INV-2: Fills monotonic | ✅ checked_add | ✅ checked_add | PASS |
| INV-3: Royalties ≤ 50% | ✅ Enforced | ✅ Enforced (but on unverified data) | PARTIAL |
| INV-4: Payouts sum = 100% | ✅ Enforced | ✅ Enforced | PASS |
| INV-5: Maker cannot pay ETH/SOL | ✅ Enforced | ✅ Enforced | PASS |
| INV-6: OrderBook sig for salt>0 | ✅ Enforced | ✅ Enforced | PASS |
| INV-7: matchAllowance not expired | ✅ Enforced | ✅ Enforced | PASS |
| INV-8: ERC20/SPL whitelist | ✅ Enforced | ❌ NOT Enforced | FAIL |
| INV-9: Signature domain binding | ✅ EIP-712 | ✅ program_id domain | PASS |
| INV-10: Collection bids via helper | ✅ Enforced | ✅ Rejected (no handler) | PARTIAL |
| INV-11: Cancel is permanent | ✅ Enforced | ✅ Enforced | PASS |
| INV-12: Reentrancy protection | ✅ nonReentrant | ⚠️ Solana runtime | PARTIAL |
| INV-13: Pause mechanism | ✅ Enforced | ✅ Enforced | PASS |
| INV-14: Value conservation | ✅ Enforced | ❌ Destination unvalidated | FAIL |
| INV-15: NFT value == 1 | ✅ Enforced | ❌ Not checked | FAIL |
| INV-16: No zero-address transfers | ✅ Enforced | ❌ Not checked | FAIL |
| INV-17: No zero-amount transfers | ✅ Enforced | ⚠️ Skipped (amount==0 returns Ok) | PARTIAL |

---

## SUMMARY

| Severity | Count | Issues |
|----------|-------|--------|
| CRITICAL | 5 | C1 (royalties unverified), C2 (fee receiver unvalidated), C3 (payout dest unvalidated), C4 (whitelist unenforced), C5 (origin fee dest unvalidated) |
| HIGH | 5 | H1 (no collection bids), H2 (no batch match), H3 (no SOL/wSOL conversion), H4 (no NFT value==1), H5 (SOL transfer source) |
| MEDIUM | 8 | M1-M8 (hash algo, salt size, timestamps, zero-salt maker, cancel semantics, fill overflow, proxy_weth, ERC-1271) |
| LOW/INFO | 4 | F1-F4 (view functions, batch cancel, provider extraction, ERC-2981) |

**Conclusion: The Solana implementation has 5 CRITICAL security vulnerabilities that allow economic exploitation. It is NOT safe for production deployment.**
