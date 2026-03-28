# LP Bonds Evolution — Security State

## 📌 Overview

This document tracks all implemented security fixes, invariants, and assumptions for the LP Bonds Evolution protocol (Solana migration).

It is the **single source of truth** for:

* What has been fixed
* What must NEVER be broken
* What assumptions future changes must respect

---

# ✅ COMPLETED FIXES

## 🔒 EBG-01 — Nonce Reset Attack (CRITICAL)

**Problem:**
Nonce accounts could be closed and reinitialized → replay attacks possible.

**Fix:**

* Removed:

  * `close_nonce_account`
  * `close_evolution_nonce`
* Enforced:

  * Strict monotonic nonce (`current_nonce + 1`)
  * `init` instead of `init_if_needed`

**Invariant:**

* Nonce is permanently bound to user
* Nonce can NEVER reset

---

## 🔒 EBG-04 — Fee Sync Issue (HIGH)

**Problem:**
Fees could be collected without updating Whirlpool accumulators.

**Fix:**

* Added CPI call:

  * `update_fees_and_rewards` before `collect_fees`

**Invariant:**

* Fees must always reflect latest Whirlpool state

---

## 🔒 EBG-05 — Tick Array Validation (HIGH)

**Problem:**
Tick arrays could be spoofed via remaining_accounts.

**Fix:**

* Enforced:

  * Owner check = Whirlpool program
  * PDA derivation check
  * Tick coverage validation

**Invariant:**

* Tick arrays must ALWAYS match Whirlpool state

---

## 🔒 EBG-06 — Pause Bypass on Redemption (MEDIUM)

**Problem:**
Pause could block user withdrawals.

**Fix:**

* `redeem_evolved_bond` is NOT pause-gated

**Invariant:**

* Users must ALWAYS be able to withdraw after lock expiry

---

## 🔒 EBG-07 — Fee Rounding Exploit (HIGH)

**Problem:**
Small amounts could result in zero fee.

**Fix:**

* Minimum fee = 1 enforced when:

  * `fee_bps > 0`
  * `amount > 0`

**Invariant:**

* Fee can NEVER be zero when fee_bps > 0

---

## 🔒 EBG-09 — Unlimited Mint via Oracle (CRITICAL)

**Problem:**
Oracle-controlled `amount_b` could mint unlimited tokens.

**Fix:**

* Added to `LevelConfig`:

  * `max_total_mint`
  * `total_minted`
* Enforced in `evolve_bond`:

  * `total_minted + amount_b <= max_total_mint`
* Updated AFTER mint success

**Invariant:**

* Total minted tokens per level MUST NOT exceed cap

**Design Note:**

* `total_minted` tracks minted amount (not circulating supply)
* Burned tokens do NOT reduce total_minted

---

# ⚠️ GLOBAL INVARIANTS (DO NOT BREAK)

## 🔐 Oracle + Nonce

* Oracle validation MUST happen before any state change
* Nonce must be strictly sequential
* Signature must bind:

  * sender
  * amounts
  * ticks
  * liquidity

---

## 🧮 Arithmetic Safety

* All arithmetic must use:

  * `checked_add`
  * `checked_mul`
  * `checked_div`
* NO unchecked math allowed

---

## 🧾 State Transitions

* State updates must occur:

  * AFTER successful CPI
  * NEVER before external calls

---

## 💰 Token Flow Integrity

* Tokens must:

  * originate from user
  * be accounted before/after CPI
* Residual tokens must be:

  * returned (token A)
  * burned (token B)

---

## 🌊 Whirlpool Integrity

* Whirlpool must be:

  * validated via deserialization
  * cross-checked with LevelConfig
* Vaults and ticks must match on-chain state

---

## 🧷 PDA Integrity

* All PDAs must be:

  * derived deterministically
  * validated explicitly
* No implicit trust in remaining_accounts

---

# 🧪 TEST GUARANTEES

The following MUST always pass:

* Nonce cannot be reset
* Reinitialization of nonce fails
* Fee never rounds to zero
* Mint cap cannot be exceeded
* Tick arrays must be valid
* Oracle replay must fail

---

# 🚫 FORBIDDEN CHANGES

DO NOT:

* Reintroduce `init_if_needed` for nonce
* Add any close instruction for nonce
* Modify oracle verification flow
* Move state updates before CPI
* Remove tick validation
* Remove mint cap enforcement

---

# 📦 ASSUMPTIONS

* Oracle is trusted but must be bounded
* Whirlpool program is trusted external dependency
* Layer token mint authority is secure PDA
* Anchor constraints are NOT sufficient alone (explicit checks required)

---

# 🔜 PENDING / FUTURE HARDENING

(To be addressed in Batch 3)

* Per-transaction mint limits
* Oracle rate limiting
* Config change timelocks
* Emergency circuit breakers
* Cross-level supply accounting

---

# 🧠 USAGE

Before ANY code change:

1. Read this file
2. Identify affected invariants
3. Ensure no invariant is violated
4. Add/update tests accordingly

---
