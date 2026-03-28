# Batch 1 — Post-Fix Parity Report

> **Date**: 2026-03-25  
> **Scope**: Re-evaluation of behavioral gaps after Batch 1 critical fixes

---

## Confirmed Resolved Issues

| ID | Finding | Status | Fix Applied |
|---|---|---|---|
| **BG-07** | L1 redemption pause-gated | ✅ RESOLVED | Removed `require!(!config.is_paused)` from `redeem_bond` |
| **BG-09** | `recover_tokens` has no `supply == 0` guard | ✅ RESOLVED | Added `bond_mint.supply == 0` + `position_custody` PDA linkage to `RecoverTokens` struct |
| **BG-10** | No `update_fees_and_rewards` before `collect_fees` | ✅ RESOLVED | Added `whirlpool_cpi::update_fees_and_rewards()` CPI + tick array PDA validation |
| **EBG-07** | Evolved bond redemption pause-gated | ✅ RESOLVED | Removed `require!(!evolution_config.is_paused)` from `redeem_evolved_bond` |
| **EBG-08** | No `update_fees_and_rewards` before `collect_fees` (evolution) | ✅ RESOLVED | Same fix as BG-10, applied to evolution program |

---

## Remaining P0 Issues

| ID | Finding | Severity | Status |
|---|---|---|---|
| **BG-12** | Bond-to-token exchange flow entirely missing | CRITICAL | NOT ADDRESSED (requires new program/instruction) |

> [!CAUTION]
> The exchange flow (`LPBondsExchange.exchange()`) remains unimplemented. This is the only remaining P0 issue. Users have no way to convert bond NFTs to liquid SPL tokens. Implementation requires a dedicated exchange instruction with oracle-signed exchange rates.

---

## Remaining P1 Issues

| ID | Finding | Severity | Status |
|---|---|---|---|
| **BG-18** | `close_nonce_account` allows nonce reset / replay window | HIGH | NOT ADDRESSED |
| **EBG-13** | `close_evolution_nonce` allows nonce reset | HIGH | NOT ADDRESSED |
| **BG-08** | L1 lock duration enforced (EVM has no L1 lock) | MEDIUM | NOT ADDRESSED (needs protocol team input) |
| **BG-25** | `lock_duration = 0` prevented, no skip logic | MEDIUM | NOT ADDRESSED |
| **EBG-09** | No on-chain cap on layer token minting | HIGH | NOT ADDRESSED |

---

## Remaining P2 Issues

| ID | Finding | Severity | Status |
|---|---|---|---|
| **BG-11** | Hardcoded zero amounts in `FeesCollected` event | MEDIUM | NOT ADDRESSED |
| **BG-13** | `verify_collateral` shares nonce with mint | MEDIUM | NOT ADDRESSED |
| **BG-17** | No operator registry / transfer whitelist | MEDIUM | NOT ADDRESSED |
| **EBG-04** | Fee can round to zero (inherited from EVM) | MEDIUM | NOT ADDRESSED |
| **EBG-12** | Cross-program struct deserialization fragile | MEDIUM | NOT ADDRESSED |
| **EBG-14** | Unused permission bits (dead code) | MEDIUM | NOT ADDRESSED |
| **EBG-16** | `recover_tokens` (evo) incomplete account validation | MEDIUM | NOT ADDRESSED |

---

## Hardening Applied Beyond Original Scope

| Enhancement | Description | Programs |
|---|---|---|
| Tick array PDA validation | Added PDA derivation checks in `collect_fees` handler using `position_custody` tick indices and whirlpool `tick_spacing` | Both |
| `whirlpool_cpi::update_fees_and_rewards` | New CPI function and discriminator added | Both |

---

## Summary

| Metric | Value |
|---|---|
| Total findings (original) | 33 |
| Fixed in Batch 1 | 5 |
| Remaining P0 | 1 (exchange flow) |
| Remaining P1 | 5 |
| Remaining P2 | 7 |
| Feature parity | ~70% (exchange flow is the primary gap) |
