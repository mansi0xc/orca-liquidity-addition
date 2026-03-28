---
name: LP Bonds Solana Audit Findings
description: Key findings from security audit of lp-bonds and lp-bonds-evolution Solana programs, including EVM comparison and cross-program architecture
type: project
---

Completed security audit of lp-bonds (Level 1 Locker) and lp-bonds-evolution (Level 2-4) Solana programs on 2026-03-23.

**Why:** These are the core DeFi programs for the LP bonds protocol, migrated from EVM (LiquidityBondLockerV3 + LiquidityBondsEvolution). Security is critical as they custody Orca Whirlpool position NFTs and mint bond NFTs.

**How to apply:**
- Top priority: Source position liquidity is stranded when bonds are evolved (M-03). Business decision needed.
- PositionCustody accounts are never closed, leaking rent (M-01, M-02).
- Evolution level fallback treats level=0 as level=1, potential bypass (M-04).
- Oracle admin diverges from protocol admin after admin transfer (L-01).
- Authority whitelist in evolution is defined but never checked.
- Cross-program PositionCustodyRef deserialization is fragile if base program struct changes.
- Ed25519 oracle verification is well-implemented with strict ordering, overlap checks, trailing garbage checks.
- Programs use Anchor 0.32.1 with anchor-spl 0.32.1.
- Program IDs: lp-bonds=Hjba1MCsx8WUtuVSyYY8QFvTzEjxsTPAUrkwJPTgQJf8, evolution=9VAsVsZpSqkwT3jBXe9yqKd1GSy9pH4ZpDduttsGoXPr.
