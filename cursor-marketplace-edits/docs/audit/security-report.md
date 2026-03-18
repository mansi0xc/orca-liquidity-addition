# Phase 4 — Solana Security Audit Report

This document evaluates the Solana exchange program against known Solana-specific attack vectors.

---

## 1. Signature Attacks

### SA-1: Ed25519 Instruction Index Manipulation

**Status: VULNERABLE**

**Description:**
The `verify_ed25519_signature` function reads Ed25519 instructions from the instructions sysvar by index (`sig_instruction_index`). The program assumes instructions appear in a specific order (matchAllowance first, then maker signature, etc.).

**Attack Vector:**
An attacker can prepend arbitrary Ed25519 instructions before the match_orders instruction. If the program reads from index 0, it would read the attacker's Ed25519 instruction instead of the legitimate one.

**Current Code (signature.rs:28):**
```rust
let ix = ix_sysvar::load_instruction_at_checked(sig_instruction_index, instructions_sysvar)
```

**Mitigation Status:** The `sig_ix_index` starts at 0 and increments. If an attacker prepends extra Ed25519 instructions, those could be at the expected indices. However, since the program verifies both the public key and message content, a forged instruction would need to contain the correct public key (orderbook/maker) AND correct message hash. The Ed25519 program would reject invalid signatures.

**Residual Risk:** LOW — The Ed25519 program validates the actual signature. Even if a fake instruction is at the expected index, the signature would fail if not signed by the expected key.

**Recommendation:** Consider verifying that the Ed25519 instruction's instruction_index fields reference the same instruction (self-referencing), which is the standard Solana pattern for Ed25519 verification.

---

### SA-2: Ed25519 Instruction Offset Parsing

**Status: CORRECT**

**Description:**
The Ed25519 instruction data has a 2-byte header (u8 count + u8 padding), then the `Ed25519SignatureOffsets` struct starts at byte 2. The struct fields at bytes 6-7, 10-11, and 12-13 correspond to `public_key_offset`, `message_data_offset`, and `message_data_size` respectively.

The code's offset parsing is **correct** despite misleading comments in the source that describe a 4-byte header. The actual Solana Ed25519 layout uses a 2-byte header.

**No Fix Required.**

---

### SA-3: Replay Protection

**Status: ADEQUATE**

**Description:**
Order replay is prevented by:
1. Fill tracking: fills monotonically increase, so re-executing the same fill would require more remaining balance
2. matchAllowance expiry: expired signatures are rejected
3. Order key hash uniqueness: includes maker, assets, salt

Once an order is fully filled, `calculateRemaining` returns 0, and `NothingToFill` is triggered.

---

## 2. Account Substitution Attacks

### AS-1: Fake Token Accounts

**Status: VULNERABLE**

**Description:**
The `remaining_accounts` used for transfers are not validated for:
- Token account ownership
- Token account mint
- ATA derivation

An attacker can provide token accounts owned by different programs or with different mints.

**Impact:** Transfers may fail (best case) or send to wrong accounts (worst case).

**Fix Required:**
- Validate token account mint matches the expected asset mint
- Validate token account owner is the Token Program
- For SPL transfers, consider requiring ATAs and validating their derivation

---

### AS-2: Fee Receiver Account Substitution

**Status: VULNERABLE (See DIFF-C2)**

The fee receiver at `remaining_accounts[1]` is not validated against on-chain configuration.

---

### AS-3: Payout Destination Substitution

**Status: VULNERABLE (See DIFF-C3)**

Payout destinations are not validated against order data.

---

## 3. PDA Attacks

### PDA-1: Order Fill PDA Spoofing

**Status: SECURE**

The `order_fill` PDA uses seeds `[b"order_fill", order_key_hash]` and is validated by Anchor's `init_if_needed` with bump. The `order_key_hash` is verified against the computed hash from order data. An attacker cannot spoof a different order fill PDA.

---

### PDA-2: Exchange Authority PDA

**Status: ADEQUATE**

The `exchange_authority` PDA uses seeds `[b"exchange_authority"]` and is used for CPI signing. It's validated by Anchor seeds constraint.

---

### PDA-3: Config PDA

**Status: SECURE**

`exchange_config` uses seeds `[b"exchange_config"]` with bump constraint. Singleton, cannot be duplicated.

---

### PDA-4: Missing Bump Validation on init_if_needed

**Status: CONCERN**

When `init_if_needed` creates a new `OrderFill` account, the bump is set. But on subsequent calls where the account already exists, the bump stored in the account is not verified against the canonical bump. This is handled by Anchor's seeds constraint, so the risk is mitigated.

---

## 4. Matching Exploits

### ME-1: Fill Manipulation via Zero-Salt Orders

**Status: CONCERN**

**Description:**
Zero-salt orders don't track fills. If a zero-salt order has `make_asset.value = 100` and `take_asset.value = 100`, it can be matched multiple times because the fill is always read as 0.

**EVM Behavior:**
Same behavior — zero-salt orders can be matched multiple times. The EVM also doesn't store fills for salt==0 orders.

**Solana Behavior:**
Matches EVM. This is by design — zero-salt orders are "immediate" orders that are expected to fully fill in one match.

**Risk:** LOW — same as EVM, and the maker must sign each transaction for zero-salt orders.

---

### ME-2: Overfilling Orders

**Status: SECURE**

Fill updates use `checked_add`, preventing arithmetic overflow. The `calculateRemaining` function subtracts fill from total, which would underflow (and error) if overfilled.

---

### ME-3: Cross-Order Fill State Leakage

**Status: CONCERN**

**Description:**
If two different orders happen to produce the same `order_key_hash`, they would share a fill PDA, leading to incorrect fill tracking.

**Mitigation:** The `order_key_hash` includes `maker`, `makeAssetType`, `takeAssetType`, `salt`, and `collectionBid`. For two different orders to collide, they would need identical values for all these fields, which means they're effectively the same order. Risk is negligible.

---

## 5. Fee Manipulation

### FM-1: Protocol Fee Bypass

**Status: VULNERABLE**

Since the fee receiver account is not validated (DIFF-C2), an attacker can provide their own account, effectively sending the fee to themselves.

---

### FM-2: Royalty Bypass

**Status: VULNERABLE**

Since royalties are client-supplied (DIFF-C1), an attacker can pass `royalty_parts: []` to skip all royalties.

---

### FM-3: Origin Fee Inflation

**Status: CONCERN**

**Description:**
Origin fees are parsed from order data. Since the order data is signed, the fees cannot be modified. However, the destination accounts for origin fees come from `remaining_accounts` and are not validated.

**Impact:** Origin fees could be redirected to attacker accounts.

---

### FM-4: Payout Percentage Validation

**Status: ADEQUATE**

Payouts must sum to exactly 10000 bps. This is enforced via `validate_payout_sum`.

---

## 6. Royalty Exploits

### RE-1: Empty Royalties Bypass

**Status: VULNERABLE**

An attacker can submit `royalty_parts: []` in `MatchOrdersArgs`, paying zero royalties regardless of what the royalties registry specifies.

---

### RE-2: Royalty Cap Enforcement

**Status: ADEQUATE (on unverified data)**

The 50% cap is enforced: `total_royalties_bps + royalty.value as u64 <= 5000`. But this only applies to the client-supplied royalty parts, which may not reflect the actual registry royalties.

---

### RE-3: Fake Registry Data

**Status: N/A**

The registry is never queried, so fake registry data is not a concern — the entire registry is bypassed.

---

## 7. Reentrancy / CPI Risks

### CPI-1: SPL Token CPI Safety

**Status: ADEQUATE**

Token transfers use Anchor's `token::transfer` CPI, which is a well-audited path. The Token Program is a known, trusted program.

---

### CPI-2: System Program CPI Safety

**Status: ADEQUATE**

SOL transfers use `system_instruction::transfer` with `invoke`, which is safe.

---

### CPI-3: Malicious Token Programs

**Status: SECURE**

The `token_program` is constrained as `Program<'info, Token>`, which validates it's the SPL Token Program. No arbitrary programs can be passed.

---

### CPI-4: Reentrancy via Transfer Hooks

**Status: LOW RISK**

Token-2022 transfer hooks could potentially execute arbitrary code during transfers. However, the program uses Token (not Token-2022), so this is not currently a risk. If Token-2022 support is added, transfer hooks must be considered.

---

## 8. Orderbook Trust Model

### OT-1: matchAllowance Timestamp Validation

**Status: ADEQUATE**

```rust
require!(args.match_left_before_timestamp > clock.unix_timestamp, ExchangeError::MatchAllowanceExpired);
```

Uses Solana's `Clock` sysvar, which is consensus-derived and cannot be manipulated by callers.

---

### OT-2: Orderbook Key Rotation

**Status: ADEQUATE**

The `order_book` pubkey can be updated by the config owner via `set_order_book`. Previous matchAllowance signatures become invalid because they reference the old orderbook key.

---

### OT-3: Orderbook as Single Point of Trust

**Status: ARCHITECTURAL CONCERN**

The orderbook can:
- Deny matching by not issuing matchAllowance signatures
- Selectively allow certain matches (censorship)
- Front-run users by issuing matchAllowance with tight timestamps

This is the same trust model as EVM and is by design.

---

## 9. Additional Findings

### AF-1: No Event Validation

**Status: INFO**

Events are emitted with correct data but are not validated against actual transfer outcomes. An attacker cannot forge events directly, but if transfers fail silently (amount==0 returns Ok), the event would report a successful match with zero transfers.

---

### AF-2: Compute Budget Risk

**Status: CONCERN**

Complex orders with many payouts, origin fees, and royalties could exceed Solana's 200,000 CU default limit (or even the 1.4M max with priority fees). There's no explicit compute check.

---

### AF-3: Account Serialization Size

**Status: CONCERN**

`MatchOrdersArgs` contains variable-length fields (`data: Vec<u8>`, `royalty_parts: Vec<Part>`, `signature_*: Vec<u8>`). Very large inputs could exceed transaction size limits (1232 bytes).

---

## Summary of Vulnerabilities

| ID | Description | Severity | Status |
|----|-------------|----------|--------|
| SA-2 | Ed25519 offset parsing | CORRECT | No Fix |
| AS-1 | Token accounts not validated | CRITICAL | Needs Fix |
| AS-2 | Fee receiver not validated | CRITICAL | Needs Fix |
| AS-3 | Payout destinations not validated | CRITICAL | Needs Fix |
| FM-1 | Protocol fee bypass | CRITICAL | Needs Fix |
| FM-2 | Royalty bypass | CRITICAL | Needs Fix |
| RE-1 | Empty royalties bypass | CRITICAL | Needs Fix |
| SA-1 | Ed25519 instruction ordering | LOW | Monitor |
| ME-1 | Zero-salt fill non-tracking | LOW | By design |
| FM-3 | Origin fee redirection | HIGH | Needs Fix |
| AF-2 | Compute budget risk | MEDIUM | Document |
| AF-3 | Transaction size limits | MEDIUM | Document |
