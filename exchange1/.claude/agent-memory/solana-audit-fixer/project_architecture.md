---
name: LP Bonds Solana Architecture
description: Key architectural patterns, PDA seeds, error conventions, and stack limitations in the lp-bonds and lp-bonds-evolution programs
type: project
---

Two Anchor programs on devnet (Anchor 0.32.1):
- lp-bonds (Hjba1MC...): Level 1 locker - creates Orca Whirlpool positions, mints bond NFTs, timelocked redemption
- lp-bonds-evolution (9VAsVsZ...): Level 2-4 evolution - burns source bond, adds new liquidity, mints upgraded bond

**Why:** Understanding the dual-program architecture is critical for cross-program account reading (PositionCustodyRef) and knowing which PDA authority signs which operations.

**How to apply:**
- lp-bonds uses `bond_authority` PDA for minting and `position_custody` PDA for holding position NFTs
- lp-bonds-evolution uses `layer_token_authority` PDA as the primary signer for token ops and `bond_authority` PDA for bond minting
- EvolveBond struct has a pre-existing BPF stack frame overflow warning that cannot be fixed by boxing alone - all accounts are already boxed
- The EvolveBond struct has many `init`/`init_if_needed` accounts which generate large deserialization code
- When adding `close = user` to account structs, always box the accounts to avoid stack overflow (RedeemBond and RedeemEvolvedBond both needed this)
