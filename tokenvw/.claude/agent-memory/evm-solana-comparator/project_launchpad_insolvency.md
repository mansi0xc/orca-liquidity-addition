---
name: Launchpad C-variant insolvency (residual)
description: Critical finding from EVM-Solana comparison -- refund_price uses pre-fee applied_price but vault only holds post-fee amount, creating insolvency in C variants
type: project
---

The Solana launchpad C variants (has_operator_filter=true) have a residual insolvency bug discovered during comparison on 2026-03-31.

**Why:** In `mint_public.rs:247-249`, `base_refund_price` is set to `applied_price` (pre-fee) not `net_price` (post-fee). The vault only holds `vault_cut` (from net_price). So `refund_price > vault_balance` per token by `protocol_fee` (R100C) or `protocol_fee * 80%` (R80C). On remints, the deficit is much larger because `base_refund_price = original_price` (full mint price, not discounted remint price).

**How to apply:** When working on the launchpad program, the fix is: set `token_record.refund_price = vault_cut`. This is a one-line change in mint_public.rs and mint_presale.rs. The previous GMIERC721RC-comparison.md incorrectly stated "FIXED" -- it has been corrected to "PARTIALLY FIXED".
