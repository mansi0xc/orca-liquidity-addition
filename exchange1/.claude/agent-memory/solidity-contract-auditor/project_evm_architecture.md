---
name: EVM LP Bonds Architecture
description: Architecture and cross-contract dependencies of the Energi EVM LP Bonds protocol - the reference implementation being ported to Solana
type: project
---

The EVM LP Bonds protocol at `/Users/mansitibrewal/chronicles/egmi-solana/evm-contracts/liquidity-bonds-contracts/contracts/` consists of 4 core contracts behind transparent proxies:

1. **LiquidityBonds** (ERC721) -- Bond NFTs representing locked Uniswap V3 positions
2. **LiquidityBondLockerV3** -- Primary bonding engine: takes user tokens, creates UniV3 positions, mints bond NFTs
3. **LiquidityBondsEvolution** -- Evolution/upgrade: burns lower-tier bonds, mints token1, creates new positions, mints higher-tier bonds
4. **LPBondsExchange** -- Redemption: exchanges bond NFTs for minted ERC20 tokens

Fully custodial model: LP position NFTs go to a multisig, users hold derivative bond NFTs. No on-chain unlock mechanism exists. Rewards are fully off-chain (getRewards0 returns 0).

All three action contracts use the same signature verification pattern with a global nonce (race condition risk) and missing parameter bindings.

**Why:** This is the reference implementation for the Solana port. Understanding the EVM architecture is essential for verifying feature parity.

**How to apply:** When reviewing Solana implementations, verify they preserve the bond-to-LP 1:1 mapping, custodial model, signature validation, layer/evolution system, and fee collection.
