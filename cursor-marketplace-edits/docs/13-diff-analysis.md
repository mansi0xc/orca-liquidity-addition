# EVM vs Solana Diff & Security Analysis

## 1. Functional Parity Table

| EVM Function | Solana Instruction | Status | Notes |
|---|---|---|---|
| `matchOrders()` | `match_orders` | **Equivalent** | Core matching logic preserved |
| `batchMatchOrders()` | — | **Missing** | No batch matching instruction |
| `cancelOrder()` | `cancel_order` | **Equivalent** | Same fill=MAX mechanism |
| `matchCollectionBidOrder()` | — | **Missing** | Collection bid flag exists but instruction blocked |
| `setProtocolFeeBps()` | `set_protocol_fee_bps` | **Equivalent** | |
| `setDefaultFeeReceiver()` | `set_default_fee_receiver` | **Equivalent** | |
| `setFeeReceiver()` | `set_fee_receiver` | **Equivalent** | |
| `setERC20AssetAllowed()` | `set_allowed_token` | **Partial** | PDA exists but **never checked** in match_orders |
| `setOrderBook()` | `set_order_book` | **Equivalent** | |
| `setExchangeOwner()` | `set_exchange_owner` | **Equivalent** | |
| `togglePause()` | `toggle_pause` | **Equivalent** | |
| — | `initialize` | **Solana-only** | Required for PDA initialization |
| — | `set_royalties_registry_program` | **Solana-only** | Stored but not used |
| — | `safe_transfer_spl` | **Solana-only** | Emergency fund recovery |
| ETH/WETH conversions | — | **Missing** | No WSOL auto-wrap/unwrap logic |
| ERC-1271 contract sigs | — | **Missing** | Only Ed25519 EOA signatures |
| Royalty registry lookup | — | **Missing** | Royalties passed as input, not verified on-chain |
| ERC-2981 royalty query | — | **Missing** | No on-chain royalty standard on Solana |
| Transfer events | TransferEvent | **Partial** | Event struct defined but never emitted |

---

## 2. Behavioral Differences

### Order Execution Differences

| Aspect | EVM | Solana |
|--------|-----|--------|
| **Signature scheme** | ECDSA (EIP-712) | Ed25519 (instruction introspection) |
| **Domain separator** | EIP-712 domain hash | `program_id + "energi" + version` |
| **Salt type** | uint256 | u64 |
| **Fill type** | uint256 | u64 |
| **Cancellation marker** | 2^256 - 1 | u64::MAX (2^64 - 1) |
| **Fill storage** | mapping(bytes32 => uint256) | PDA account per order |
| **Reentrancy protection** | `nonReentrant` modifier | None (relies on Solana runtime) |
| **Token whitelist** | Checked in matchOrders | **NOT checked** despite set_allowed_token existing |
| **Asset types** | ETH/WETH/ERC20/ERC721/ERC1155 | Sol/WrappedSol/SplToken/Nft/SemiFungible |
| **Transfer authority** | `approve()` to exchange proxy | Delegate to exchange_authority PDA |
| **Collection bids** | Fully implemented | Blocked (`collection_bid` must be false) |
| **Batch matching** | batchMatchOrders() | Not implemented |

### Fee Handling Differences

| Aspect | EVM | Solana |
|--------|-----|--------|
| **Protocol fee base** | Applied to `total_amount` (base + origin fees) | Applied to `total_amount` (same) |
| **Royalty source** | On-chain registry + ERC-2981 | **Client-provided**, no on-chain verification |
| **Royalty cap** | 5000 bps (50%) | 5000 bps (50%) — same |
| **Fee receiver lookup** | `feeReceivers[token]` mapping | FeeReceiver PDA per mint |
| **ETH fee handling** | Special WETH wrap/unwrap logic | N/A (SOL is simpler) |
| **Origin fee overflow** | Checked in uint256 space | Checked in u128 space |

### Cancellation Differences

| Aspect | EVM | Solana |
|--------|-----|--------|
| **Auth check** | `tx.origin == order.maker` | `maker` is a Signer account |
| **EVM uses tx.origin** | Yes (vulnerable to phishing) | N/A (uses direct signer) |
| **PDA creation on cancel** | N/A | `init_if_needed` creates PDA if it doesn't exist |

---

## 3. Security Differences & Vulnerabilities

### CRITICAL — Severity: HIGH

#### 3.1 Royalties Not Verified On-Chain
**Location:** `match_orders.rs:33-34`
```
TODO: add on-chain verification against the royalties registry PDA
```
**Issue:** `royalty_parts` are passed as client input and never validated against any on-chain registry. A malicious caller can:
- Set royalties to 0% (bypassing creator royalties)
- Set royalty recipients to attacker addresses
- Set excessively high royalties to drain funds

**EVM comparison:** EVM queries on-chain royalty registry and ERC-2981.

**Risk:** Creator royalty theft / circumvention.

---

#### 3.2 Token Whitelist Not Enforced
**Location:** `match_orders.rs` — no call to check AllowedToken PDA
**Issue:** The `set_allowed_token` admin instruction creates AllowedToken PDAs, but `match_orders` never checks them. Any token can be traded regardless of whitelist status.

**EVM comparison:** EVM checks `allowedERC20Assets[token]` for all ERC20 tokens.

**Risk:** Unauthorized tokens can be traded on the exchange.

---

#### 3.3 Remaining Accounts Not Validated
**Location:** `transfers.rs:10-30` (AccountWalker)
**Issue:** The `AccountWalker` only checks array bounds. It does NOT validate:
- Token account mint matches expected mint
- Token account owner matches expected party
- Destination accounts are real token accounts (not arbitrary accounts)
- Fee receiver matches the FeeReceiver PDA

A malicious caller could substitute:
- Wrong fee receiver (diverting protocol fees)
- Wrong payout destination (diverting payment)
- Wrong NFT source account (sending from wrong holder)

**EVM comparison:** EVM uses `transferFrom()` which validates ownership implicitly via ERC20/721 approval mechanisms.

**Risk:** Fund diversion, theft via account substitution.

---

#### 3.4 No Token Account Ownership Validation on Transfers
**Location:** `transfers.rs:33-53` (spl_transfer)
**Issue:** SPL transfer CPIs use the exchange_authority PDA as authority (delegate). The program does not verify:
- The source token account is owned by the expected party (seller for NFT, buyer for payment)
- The destination token account belongs to the intended recipient
- The token account mint matches the order's asset mint

The CPI will only succeed if the exchange_authority has been set as delegate, but this could be any account that has approved the PDA.

**Risk:** If a third party has approved the exchange_authority PDA on their token account, an attacker could craft orders that drain those accounts.

---

### HIGH — Severity: MEDIUM-HIGH

#### 3.5 SOL Transfer Requires Payer as Signer
**Location:** `transfers.rs:56-67`
**Issue:** SOL transfers use `system_instruction::transfer` which requires the `from` account to be a signer. In the match_orders context, only the `payer` is a signer. This means:
- Only the payer can send SOL
- The other party's SOL cannot be transferred
- This could break certain order configurations where the non-payer needs to send SOL

**Risk:** Certain valid order configurations may fail silently or revert.

---

#### 3.6 No Reentrancy Guard
**Location:** `match_orders.rs` — no reentrancy check
**Issue:** EVM Exchange uses `nonReentrant` modifier. Solana's runtime prevents reentrancy into the same program within a single instruction, but CPI callbacks from token programs could theoretically be exploited if the token is a malicious program.

**Mitigation:** Solana's runtime provides some protection, but this is not equivalent to the explicit guard in the EVM version.

---

### MEDIUM — Severity: MEDIUM

#### 3.7 Zero-Salt Order Replay
**Location:** `match_orders.rs:128-137`
**Issue:** Zero-salt orders do not track fills. The fill amount is always treated as 0:
```rust
let left_fill_amount = if args.order_left.salt == 0 { 0u64 } else { ... };
```
If a zero-salt order has enough value, it could theoretically be matched multiple times in separate transactions, each time filling the full amount.

**Mitigation needed:** Zero-salt orders should only be usable once. The EVM version has the same behavior (salt=0 means no tracking), relying on the fact that the maker IS the tx sender. On Solana, if `maker == payer`, this is safe because the payer controls their own funds. However, this should be documented.

---

#### 3.8 init_if_needed on OrderFill PDAs
**Location:** `match_orders.rs:49-65`, `cancel_order.rs:28-35`
**Issue:** `init_if_needed` has known security implications in Anchor. If the PDA already exists from a cancelled order (fill=MAX), and is then reinitialized, the init_if_needed won't re-initialize it — it will use the existing account. This is actually the correct behavior here (cancelled orders stay cancelled), but it means the PDA account persists permanently.

**Risk:** Low — functions correctly, but bloats on-chain state.

---

#### 3.9 Missing TransferEvent Emissions
**Location:** `events.rs:20-28` — `TransferEvent` defined but never emitted
**Issue:** Individual transfer events are not emitted during trade execution. Only the aggregate `MatchEvent` is emitted. This makes it harder for indexers to track:
- Protocol fee amounts
- Royalty distributions
- Individual payout amounts

**EVM comparison:** EVM emits detailed `Transfer` events for every individual transfer.

---

#### 3.10 Ed25519 Instruction Index Assumption
**Location:** `match_orders.rs:211` — `sig_ix_index` starts at 0
**Issue:** The program assumes Ed25519 verify instructions are at indices 0, 1, 2, 3 in the transaction. If additional instructions are prepended to the transaction, the verification will fail or verify the wrong instruction.

**Mitigation:** This is actually a security feature — it prevents injection of unrelated Ed25519 instructions. However, it makes the transaction layout rigid.

---

#### 3.11 No Expiry on Zero-Salt Orders
**Location:** `match_orders.rs:241-248`
**Issue:** Zero-salt orders skip the `matchAllowanceExpired` check entirely. If `start == 0` and `end == 0`, a zero-salt order has no time bound whatsoever. The only protection is that the maker must be the payer.

---

### LOW — Severity: LOW

#### 3.12 Fill Value Precision
**Issue:** EVM uses uint256 (77 digits), Solana uses u64 (19 digits). For large orders, the Solana implementation has less precision for partial fills. Unlikely to be a practical issue for NFT marketplace orders.

#### 3.13 No Batch Matching
**Issue:** No `batchMatchOrders` equivalent. Each match requires a separate transaction. Higher latency for bulk operations.

#### 3.14 Emergency Transfer Function
**Location:** `admin.rs:269-289` — `safe_transfer_spl`
**Issue:** Owner can transfer any SPL tokens using the exchange_authority PDA. This is powerful — if anyone has approved the exchange_authority PDA as delegate, the owner can drain those tokens.

**EVM comparison:** No equivalent admin function exists in EVM.

---

## 4. Critical Invariants Verification

### INV-1: NFT Must Always Transfer Correctly
**Status: PARTIALLY VERIFIED**
- Transfer CPI is correctly constructed in `spl_transfer()`
- BUT: No validation that the NFT source account belongs to the seller
- BUT: No validation that the NFT destination account belongs to the buyer
- No `value == 1` check for NFTs (EVM checks `ERC721 value must be 1`)

### INV-2: Seller Must Own NFT at Execution
**Status: NOT VERIFIED**
- The program does not check NFT ownership
- Relies on the CPI failing if the exchange_authority is not delegate
- A malicious seller could theoretically sign an order for an NFT they don't own; the tx would fail at transfer time, but the error message would be opaque

### INV-3: Buyer Must Pay Correct Amount
**Status: PARTIALLY VERIFIED**
- Fill calculation correctly computes amounts
- Fee deductions follow correct BPS math
- BUT: No validation that the source account has sufficient balance
- Relies on CPI transfer failure for insufficient funds

### INV-4: Order Cannot Execute Twice
**Status: VERIFIED for salt > 0**
- OrderFill PDA tracks cumulative fill
- `calculate_remaining()` checks `fill < MAX`
- Fill overflow check with `checked_add`

**Status: NOT VERIFIED for salt == 0**
- Zero-salt orders always read fill as 0
- No fill tracking means same order can match repeatedly
- Relies on maker being payer (they control their own funds)

---

## 5. Summary of Missing Protections

| Protection | EVM | Solana | Severity |
|-----------|-----|--------|----------|
| Royalty on-chain verification | Yes | **NO** | CRITICAL |
| Token whitelist enforcement | Yes | **NO** | HIGH |
| Remaining account validation | Implicit (ERC transfers) | **NO** | CRITICAL |
| Token account ownership check | Via approve() | **NO** | HIGH |
| Reentrancy guard | Yes (modifier) | Partial (runtime) | MEDIUM |
| Transfer events | Yes (detailed) | **NO** (not emitted) | LOW |
| Batch matching | Yes | **NO** | LOW |
| Collection bids | Yes | **NO** (blocked) | LOW |
| Contract wallet signatures | Yes (ERC-1271) | **NO** | LOW |
| NFT value==1 check | Yes | **NO** | MEDIUM |

---

## 6. Recommendations

1. **CRITICAL:** Add on-chain royalty verification via royalties registry CPI
2. **CRITICAL:** Validate remaining accounts — check mint, owner, and authority on every token account used in transfers
3. **HIGH:** Enforce token whitelist in `match_orders` by checking AllowedToken PDA
4. **HIGH:** Validate that NFT source accounts are owned by the seller and payment source accounts by the buyer
5. **MEDIUM:** Emit TransferEvent for each individual transfer for indexer parity
6. **MEDIUM:** Add NFT value==1 check for AssetClass::Nft
7. **MEDIUM:** Consider adding explicit reentrancy protection or documenting why it's not needed
8. **LOW:** Implement batch matching instruction
9. **LOW:** Document zero-salt order replay behavior explicitly
