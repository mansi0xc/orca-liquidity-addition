---
name: LP Bonds Audit Status
description: Status of the independent security audit of lp-bonds and lp-bonds-evolution Solana programs, key findings, and blockers for mainnet
type: project
---

Independent security audit completed 2026-03-25 on branch `initial-implementation` (commit 37cfa26).

Report saved to: `/Users/mansitibrewal/chronicles/egmi-solana/solana-lp-bonds-contracts/verification-report.md`

**Why:** This is a pre-mainnet audit. Real user funds will be at stake.

**Key blockers identified:**
1. [M-4] `is_evolved = true` flag on target custody blocks L2->L3->L4 evolution (line 1010 in evolution lib.rs). Only L1->L2 works. EvolutionRecord PDA `init` already prevents double-evolution, making `is_evolved` redundant and harmful.
2. [M-1] Evolution program's `recover_tokens` can drain active custody position NFTs because custody token accounts are owned by `layer_token_authority` PDA (same authority used in recover_tokens). Comment says "must NOT be custody" but no on-chain enforcement.

**Should fix:** H-1 (no collect_fees instruction -- trading fees locked forever during bond lock period), M-3 (lock_duration can be 0), L-4 (treasury can be zero address).

**How to apply:** These must be resolved before mainnet. When reviewing fixes, verify the actual code matches the intent -- the previous fix session had a comment-only "guard" on M-1 that provides zero protection.
