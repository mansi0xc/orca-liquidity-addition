---
name: Audit Fix Implementation Patterns
description: Key patterns and gotchas discovered while implementing security fixes for the LP Bonds contracts
type: feedback
---

When adding new account structs with many Account<> fields, ALWAYS use Box<Account<>> to avoid BPF stack frame overflow. The 4096-byte stack limit is easily exceeded with 10+ accounts.

**Why:** Both CollectFees structs (lp-bonds and evolution) hit stack overflow errors until all Account<> fields were boxed. The EvolveBond struct has a pre-existing unfixable stack overflow.

**How to apply:** Any new account struct with more than ~6 Account<> fields should use Box<Account<>> by default. UncheckedAccount and Signer are small enough to leave unboxed.

For collect_fees CPI in lp-bonds: position_custody PDA is the signer (it owns the custody token account holding the position NFT).
For collect_fees CPI in evolution: layer_token_authority PDA is the signer (it owns the custody token account).

The whirlpool_cpi::collect_fees helper already exists in both programs' whirlpool_cpi.rs modules, including the COLLECT_FEES discriminator constant.
