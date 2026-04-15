---
name: Marketplace Analysis Findings
description: GMI marketplace architecture, order-book pattern, fee system, bugs (fillCollectionBidOrder dead code), and migration-critical patterns for Solana
type: project
---

GMI marketplace is an order-book mediated NFT exchange with UUPS proxies (Exchange, ExchangeHelper, RoyaltiesRegistry).

**Architecture:**
- External storage pattern (StorageBase-derived contracts, not proxy slot storage)
- ExchangeProxy holds all funds (ETH/ERC20) during trades; implementation runs via delegatecall
- ExchangeHelper split from Exchange due to contract size limits; delegates library calls
- Off-chain order book signs EIP-712 match allowances with expiry timestamps
- Orders with salt=0 are direct submissions (no sig/fill tracking); salt>0 are off-chain registered

**Fee System (applied in order):**
1. Protocol fee (configurable bps, paid by NFT seller)
2. Royalties (from RoyaltiesRegistry or ERC-2981, capped at 50%)
3. Origin fees (from order data, both sides)
4. Payouts (must sum to 100%)

**Bugs Found:**
- fillCollectionBidOrder: dead code -- partial fill case overwritten by full fill (MEDIUM)
- providerExtractor: Rarible provider overwrites data from first provider (LOW)
- No zero-address checks in ExchangeStorage setters (LOW)
- No events on ExchangeStorage config changes (LOW)
- receiveETH() on Exchange impl has no access control (LOW)

**Why:** Documented to inform Solana marketplace migration -- need equivalent signing, fill tracking, royalty aggregation, and fee distribution.

**How to apply:** When migrating to Solana: (1) replace EIP-712 with ed25519 signing, (2) replace WETH wrapping with SOL/wSOL, (3) simplify royalty sources to Metaplex standard, (4) fix the fillCollectionBidOrder bug, (5) adapt order-book service for Solana transactions.
