---
name: Launchpad EVM Analysis Findings
description: Key findings from comprehensive analysis of the EVM launchpad NFT contracts -- 6 variants (Standard/Refundable/80%Refundable x base/CreatorControls), OperatorRegistry proxy, insolvency bugs in C variants.
type: project
---

## Launchpad Contracts Analysis (2026-03-31)

**Source**: `/Users/mansitibrewal/chronicles/egmi-solana/evm-contracts/launchpad-contracts/contracts/`
**Output**: `/Users/mansitibrewal/chronicles/egmi-solana/solana-launchpad-contracts/docs/evm/`

### Architecture
- 6 NFT collection contract variants: GMIERC721, GMIERC721C, GMIERC721R, GMIERC721RC, GMIERC721R80, GMIERC721R80C
- 3 families: Standard (no refund, ERC721A), 100% Refundable (OZ ERC721Enumerable), 80% Refundable (OZ ERC721Enumerable)
- Each family has base + "C" (Creator Controls with OperatorFilter/OperatorRegistry)
- OperatorRegistry behind TransparentUpgradeableProxy
- NOT a token launchpad / bonding curve system -- pure NFT mint platform

### Critical Bugs Found
1. **Insolvency in RC/R80C**: refundPrice records full escrow amount but share is sent to fundReceiver, making contract unable to refund all holders
2. **ownerMint inner loop bug**: In refundable variants, recycling refunded tokens can mint more than requested _quantity
3. **Toggle event value inversion**: All 6 contracts emit wrong boolean in toggle events

**Why:** These findings are critical context for the Solana migration -- the insolvency bugs should be fixed, not replicated.
**How to apply:** When migrating to Solana, ensure refund pool accounting correctly deducts any platform shares. Fix the ownerMint loop bug.
