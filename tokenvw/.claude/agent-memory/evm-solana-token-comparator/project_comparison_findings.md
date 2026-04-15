---
name: EVM-Solana LPToken Comparison Findings
description: Key findings from the exhaustive EVM-to-Solana LPToken migration comparison (2026-03-31)
type: project
---

LPToken migration comparison completed with ~88% parity. 8 functions fully equivalent, 3 partially equivalent (intentional improvements), 9 handled by runtime, 3 missing (low severity).

**Why:** The comparison was requested to verify migration completeness before deployment.

**How to apply:**
- Top gaps to address: (1) placeholder program ID, (2) bond program integration architecture (bond program CPIs SPL Token directly but lp_token owns mint_authority -- mutually exclusive), (3) missing Metaplex token metadata for name/symbol
- Bond programs at `/solana-lp-bonds-contracts/` do NOT reference lp_token at all -- integration path needs to be decided (bond program must CPI into lp_token for minting, or architecture must change)
- Security improvements over EVM: dual-signer burn, two-step ownership, no renounceOwnership, zero-owner validation
- Comparison docs at: `/solana-token/docs/comparison/`
