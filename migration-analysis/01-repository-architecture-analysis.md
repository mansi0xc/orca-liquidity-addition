# Repository Architecture Analysis

## Executive Summary

This document provides a complete architectural analysis of all 16 repositories in the GMI (GonnaMakeIt) protocol stack, classifying each by blockchain dependency level, Solana compatibility, and recommended migration approach.

**Current State:**
- **Already migrated:** `liquidity-bonds-contracts` → `solana-lp-bonds-contracts`, `lp-bond-amount-fetcher` → `solana-price-oracle`
- **Remaining contract migrations:** 3 EVM contract repos + LPBondsExchange tokenization component
- **Full-stack migration pending:** 5 APIs, 4 indexers, 2 frontends

---

## 1. Smart Contract Repositories

### 1.1 token (`evm-contracts/token/`)

| Field | Detail |
|-------|--------|
| **Purpose** | ERC20 tokens: GMI (capped supply), GMI CV (trading-restricted), LP Token (uncapped) |
| **Current Architecture** | Upgradeable ERC20 via OpenZeppelin proxy (ERC1967). Minter roles, pausable, reentrancy-guarded. Storage: minters mapping, maxSupply, chainId |
| **Blockchain Dependencies** | ERC20Upgradeable, proxy pattern, ECDSA for admin ops, `onlyMintersOrOwner` modifier |
| **Key Patterns** | Minter allowlist, trade restrictions (GMI CV allowedExchanges), upgradeable proxies |
| **EVM Dependency Level** | **HIGH** — ERC20 standard, proxy upgradeability, Solidity storage layout |
| **Can Support Solana Directly** | **No** |
| **Why Not** | Solana tokens use SPL Token / Token-2022 program, not custom contract logic. Minter roles, trade restrictions, and upgradeability must be reimplemented via program authorities and token extensions |
| **Recommended Approach** | **Create Solana-specific implementation using Token-2022** |
| **Reasoning** | Token-2022 provides metadata extensions, transfer hooks (for trade restrictions like GMI CV), and mint authority (for minter roles). The supply cap can be enforced at the mint-authority program level. No code can be shared between Solidity ERC20 and SPL Token-2022 |

**Migration Specifics:**
- **GMI Token** → Token-2022 mint with metadata extension, mint authority PDA, max supply enforced by authority program
- **GMI CV Token** → Token-2022 with transfer hook extension for trade allowance logic (replaces `tradeAllowance` modifier)
- **LP Token** → Token-2022 mint with mint authority, no supply cap

---

### 1.2 evm-contracts (`evm-contracts/evm-contracts/`)

| Field | Detail |
|-------|--------|
| **Purpose** | NFT marketplace: order matching (Exchange), validation (ExchangeHelper), royalty registry |
| **Current Architecture** | UUPS upgradeable. Exchange matches maker/taker orders with partial fills. Supports ETH/WETH/ERC20 ↔ ERC721/ERC1155. RoyaltiesRegistry supports ERC2981, Rarible V1/V2, on-chain storage. Separate StorageBase contracts. UpgradeManager (multisig) |
| **Blockchain Dependencies** | EIP-712 order signing, ERC20/721/1155 safe transfers, WETH wrap/unwrap, ABI encoding for asset classes, event-driven fill tracking |
| **Key Patterns** | Off-chain orderbook + on-chain settlement, partial fills stored in `fills` mapping, protocol fees + royalty splits, collection bids |
| **EVM Dependency Level** | **HIGH** — deep EVM primitives (EIP-712, msg.value, WETH, delegatecall, proxy storage) |
| **Can Support Solana Directly** | **No** |
| **Why Not** | Solana has no msg.value equivalent, no ABI encoding, no EIP-712. Order matching must use Anchor instruction + account constraints. Asset transfers use SPL Token associated accounts, not safeTransferFrom. Royalties must be handled via Metaplex royalty enforcement or custom program logic |
| **Recommended Approach** | **Create Solana-specific Anchor program** |
| **Reasoning** | The order-matching and settlement logic must be completely rewritten for Solana's account model. PDAs replace storage mappings. Ed25519 replaces ECDSA/EIP-712. SPL Token/Token-2022 transfers replace ERC20/721/1155 transfers. Royalty enforcement uses Metaplex or custom program logic |

**Migration Specifics:**
- **Exchange** → Anchor program with `match_orders` instruction, PDA-based fill tracking, SPL token CPIs for settlement
- **ExchangeHelper** → Rolled into main program (Ed25519 sig verification via sysvar, order hashing via Anchor)
- **RoyaltiesRegistry** → PDA-based royalty config per collection/token, or integration with Metaplex royalty model
- **Asset Classes** → SOL (native), SPL Token, Token-2022, Metaplex NFTs (replace ETH/WETH/ERC20/ERC721/ERC1155)

---

### 1.3 liquidity-bonds-contracts (`evm-contracts/liquidity-bonds-contracts/`) — MIGRATED

| Field | Detail |
|-------|--------|
| **Purpose** | LP bond NFTs representing locked Uniswap V3 positions, with evolution and tokenization |
| **Current Architecture** | ERC721 bonds wrapping Uniswap V3 NFT positions. LiquidityBondLockerV3 locks positions and mints bonds. LPBondsExchange burns bonds for ERC20. Evolution variants for level upgrades |
| **Migration Status** | **Core locker and evolution migrated to `solana-lp-bonds-contracts`** |
| **EVM Dependency Level** | **HIGH** (was) |
| **Recommended Approach** | **Already migrated. Verify LPBondsExchange (tokenization) coverage** |
| **Gap Identified** | The `LPBondsExchange` contract (burn LP bonds → mint ERC20) does not have a visible equivalent instruction in the Solana programs. If tokenization is required on Solana, an additional instruction or separate program is needed |

---

### 1.4 launchpad-contracts (`evm-contracts/launchpad-contracts/`)

| Field | Detail |
|-------|--------|
| **Purpose** | NFT launchpad: mintable collections with presale/public sale, refund options (100%/80%), operator filtering |
| **Current Architecture** | GMIERC721 (ERC721A + ERC2981), GMIERC721R (100% refund, ERC721Enumerable), GMIERC721R80 (80% refund), C-variants (operator filtering via OperatorRegistry). Whitelist via merkle proof. Mint limits per wallet. Owner reserve. On-chain metadata |
| **Blockchain Dependencies** | ERC721A (batch minting), ERC2981 (royalties), ERC721Enumerable (refund iteration), merkle proofs, native coin payments (msg.value), OperatorRegistry for marketplace filtering |
| **Key Patterns** | Presale (whitelist + merkle) → public sale → optional refund. Refunded tokenIds reused for owner mints. NoContracts modifier prevents bot minting |
| **EVM Dependency Level** | **HIGH** — ERC721 variants, batch minting, native payments, storage patterns |
| **Can Support Solana Directly** | **No** |
| **Why Not** | Solana NFTs use Metaplex Token Metadata standard, not ERC721. Batch minting uses compressed NFTs or Metaplex candy machine patterns. Refund logic must be built as custom program state. Operator filtering is N/A (Solana uses different marketplace patterns) |
| **Recommended Approach** | **Create Solana-specific Anchor program** |
| **Reasoning** | NFT minting on Solana requires Metaplex CPI for metadata. Candy Machine pattern replaces presale/public sale mechanics. Refund logic needs PDA-based escrow. Whitelist can use merkle verification or SPL Token gating. Operator filtering is replaced by Metaplex royalty enforcement (pNFTs) or Authority checks |

**Migration Specifics:**
- **GMIERC721** → Anchor program with Metaplex NFT minting CPI, presale/public phase PDAs, mint-limit PDAs per wallet
- **Refund variants** → Escrow PDA holding SOL, refund instruction that burns NFT and returns SOL
- **OperatorRegistry** → Metaplex Programmable NFTs (pNFTs) with rule sets, or custom authority checks
- **Batch minting** → Compressed NFTs (Bubblegum) for gas efficiency, or standard Metaplex for smaller collections

---

## 2. API Repositories

### 2.1 user (`evm-apis/user/`)

| Field | Detail |
|-------|--------|
| **Purpose** | User management: authentication, profiles, portfolios, notifications, favorites, referrals, launchpad collection management |
| **Current Architecture** | Express + TypeScript, Sequelize + PostgreSQL, JWT auth, AWS S3, SendGrid. Multi-chain via `blockchainMiddleware` header. Chain-specific DB tables (EthereumOrders, EnergiNftContracts, etc.) |
| **Blockchain Dependencies** | ethers v5 for ENS lookup only. `ethers.utils.formatUnits` for bid display. No direct contract calls. Chain context from HTTP header |
| **EVM Dependency Level** | **LOW** |
| **Can Support Solana Directly** | **Yes** |
| **Why Not** | N/A — can support |
| **Recommended Approach** | **Extend existing repo with Solana chain support** |
| **Reasoning** | The API is already multi-chain via `blockchainContext`. Adding Solana requires: (1) new chain enum value, (2) Solana-specific DB tables (SolanaNftContracts, SolanaOrders, etc.), (3) replace ENS lookup with SNS (Solana Name Service) for Solana users, (4) Ed25519 signature verification for auth if wallet-signed. Core business logic (profiles, favorites, referrals) is chain-agnostic |

---

### 2.2 general (`evm-apis/general/`)

| Field | Detail |
|-------|--------|
| **Purpose** | Core marketplace API: NFT listing, stats, trading history, order creation/submission proxy |
| **Current Architecture** | Express + TypeScript, Sequelize + PostgreSQL, node-cache, external APIs (NFTGo, Reservoir, CoinGecko). Proxies order operations to order-book service |
| **Blockchain Dependencies** | ethers v5 for `isAddress` and `formatUnits` only. No direct RPC calls. Uses DB data populated by indexers |
| **EVM Dependency Level** | **LOW** |
| **Can Support Solana Directly** | **Yes** |
| **Recommended Approach** | **Extend existing repo with Solana support** |
| **Reasoning** | Almost entirely chain-agnostic. Address validation switches to `@solana/web3.js` `PublicKey.isOnCurve()`. External API calls (NFTGo, Reservoir) may need Solana equivalents (Helius, Tensor APIs). Order proxy calls route to Solana order-book service. DB tables already support chain prefixes |

---

### 2.3 lpbonds (`evm-apis/lpbonds/`)

| Field | Detail |
|-------|--------|
| **Purpose** | LP bonds marketplace API: stats, collections, orders, metadata, position info for LP bond NFTs |
| **Current Architecture** | Express + TypeScript, Sequelize + PostgreSQL. Direct on-chain reads via `ethers.Contract` to locker contracts (`basePositions(bondId)`). Calls lp-bond-amount-fetcher oracle. Multi-chain RPC_URLS |
| **Blockchain Dependencies** | ethers v5 `StaticJsonRpcProvider`, locker contract ABIs, on-chain position reads, BigNumber math |
| **EVM Dependency Level** | **MEDIUM-HIGH** |
| **Can Support Solana Directly** | **Partially** |
| **Recommended Approach** | **Refactor into multi-chain architecture** |
| **Reasoning** | Business logic (stats aggregation, collection management, order proxying) is chain-agnostic. But `LockerContractService` is deeply EVM-coupled (ethers.Contract + locker ABI). For Solana: replace with `@solana/web3.js` account fetches + Anchor IDL deserialization of PositionCustody PDAs. Create a blockchain abstraction layer: `ILockerService` with EVM and Solana implementations. Oracle calls already point to solana-price-oracle for Solana chains |

---

### 2.4 lp-bond-amount-fetcher (`evm-apis/lp-bond-amount-fetcher/`) — MIGRATED

| Field | Detail |
|-------|--------|
| **Purpose** | Oracle: compute Uniswap V3 LP position amounts and sign responses |
| **Migration Status** | **Migrated to `solana-price-oracle`** |
| **Recommended Approach** | **Keep both. EVM version serves EVM chains. Solana version serves Solana** |
| **Reasoning** | Fundamentally different math (sqrtPriceX96 vs sqrtPriceX64), different signing (ECDSA vs Ed25519), different position lookup (NFT tokenId vs PDA). Cannot be merged. Each oracle serves its respective chain |

---

### 2.5 rewards-service (`evm-apis/rewards-service/`)

| Field | Detail |
|-------|--------|
| **Purpose** | Daily LP bond reward calculation (cron job). Calls oracle for liquidity amounts, applies reward formula, updates DB |
| **Current Architecture** | Express + Node.js, PostgreSQL (raw pg), node-cron. Calls lp-bond-amount-fetcher for each bond. Formula: `(amount0 * 2) / 365 * multiplier` |
| **Blockchain Dependencies** | ethers v6 for `formatEther`/`parseEther` only. No direct RPC. Calls oracle URL |
| **EVM Dependency Level** | **LOW** |
| **Can Support Solana Directly** | **Yes** |
| **Recommended Approach** | **Extend existing repo with Solana support** |
| **Reasoning** | Only change needed: route oracle calls to solana-price-oracle for Solana bonds (by chainId). Replace `ethers.formatEther` with chain-agnostic decimal conversion. Add `chainId` column to `bondrewards` table (may already exist). Reward formula is chain-agnostic |

---

### 2.6 order-book (`evm-apis/order-book/`)

| Field | Detail |
|-------|--------|
| **Purpose** | Query + Submit services for NFT marketplace orders. Go + Fiber + PostgreSQL |
| **Current Architecture** | Two services: Query (read orders), Submit (create/cancel with EIP-712 verification). Uses go-ethereum for EIP-712 signing, block timestamp fetching, Uniswap price lookups |
| **Blockchain Dependencies** | EIP-712 order signing/verification, `eth_getBlockByNumber` for expiry, Uniswap V2 factory/pair for ERC20 pricing, ECDSA signature recovery |
| **EVM Dependency Level** | **HIGH** |
| **Can Support Solana Directly** | **No** |
| **Recommended Approach** | **Create Solana-specific order-book service** |
| **Reasoning** | EIP-712 signing is replaced by Ed25519 message signing. Block timestamp is replaced by Solana clock/slot. Uniswap pricing is replaced by Orca Whirlpool pricing or Jupiter price API. Signature verification uses `nacl.sign.detached.verify`. The Query service schema may be shared (orders table), but the Submit service requires complete reimplementation of signing, verification, and pricing logic |

---

## 3. Indexer Repositories

### 3.1 volume-indexer (`evm-indexers/volume-indexer/`)

| Field | Detail |
|-------|--------|
| **Purpose** | NFT marketplace volume and price history. Tracks sales from Seaport, GMI Exchange, Blur, Magic Eden, Wyvern |
| **Current Architecture** | Node.js, Web3, Sequelize. WebSocket subscription to new blocks. Decodes marketplace-specific events (OrderFulfilled, Match, etc.). CoinGecko for USD conversion |
| **Blockchain Dependencies** | `eth_subscribe('newBlockHeaders')`, `eth_getPastLogs`, ABI decoding of marketplace events, topic-based filtering |
| **EVM Dependency Level** | **HIGH** |
| **Can Support Solana Directly** | **No** |
| **Recommended Approach** | **Create Solana-specific volume indexer** |
| **Reasoning** | Solana has no event logs in the EVM sense. Volume tracking requires parsing transaction instructions from the Solana marketplace program (match_orders instruction data), or using Geyser plugin streams for account state changes. The data model (TransactionVolume) can be shared, but the indexing engine must be completely rewritten. Consider Helius webhooks or Yellowstone gRPC (Geyser) for real-time transaction streaming |

---

### 3.2 liquidity-indexer (`evm-indexers/liquidity-indexer/`)

| Field | Detail |
|-------|--------|
| **Purpose** | Airdrop-related: indexes bridged token mints/burns and EnergiSwap/Uniswap V2 liquidity events |
| **Current Architecture** | Node.js, Web3 v4, PostgreSQL (raw queries). Cron every 5 min. Monitors Transfer(0x0 → addr) for mints, Mint/Burn for LP events |
| **Blockchain Dependencies** | `contract.getPastEvents()` for Transfer/Mint/Burn, EVM event topics, Uniswap V2 pool ABIs |
| **EVM Dependency Level** | **HIGH** |
| **Can Support Solana Directly** | **No** |
| **Recommended Approach** | **Evaluate necessity, then create Solana-specific if needed** |
| **Reasoning** | This indexer tracks Energi/Ethereum bridge and LP activity for airdrops. If the Solana protocol has equivalent airdrop mechanics requiring liquidity tracking, a new indexer watching Orca Whirlpool account changes and Wormhole/bridge activity would be needed. If airdrop is EVM-only, this indexer stays EVM-only |

---

### 3.3 nft-indexer (`evm-indexers/nft-indexer/`)

| Field | Detail |
|-------|--------|
| **Purpose** | NFT contract discovery, token ownership, transfer history. Handles ERC721 transfers, launchpad events, Uniswap V3 position tracking |
| **Current Architecture** | Node.js, Web3, Sequelize. WebSocket block subscription. Topic-based log filtering for Transfer, OwnershipTransferred, CollectionLaunched, IncreaseLiquidity. Eligible contract whitelist |
| **Blockchain Dependencies** | `eth_subscribe`, `getPastLogs`, ERC165/ERC721 interface detection, topic filtering, log decoding |
| **EVM Dependency Level** | **HIGH** |
| **Can Support Solana Directly** | **No** |
| **Recommended Approach** | **Create Solana-specific NFT indexer** |
| **Reasoning** | Solana NFT ownership is tracked via token accounts (SPL Associated Token Accounts), not Transfer events. Collection discovery uses Metaplex collection verification. Transfer tracking uses transaction parsing or Geyser account change streams. The data model (NftContract, NftToken, NftOwner, NftTransfer) can be shared across chains, but the indexing mechanism is entirely different |

---

### 3.4 metadata-indexer (`evm-indexers/metadata-indexer/`)

| Field | Detail |
|-------|--------|
| **Purpose** | NFT metadata enrichment. Fetches from Alchemy/Reservoir APIs or on-chain tokenURI. Uploads images to S3, generates thumbnails |
| **Current Architecture** | Express + Sequelize. Queue-based processing. Two modes: API (Alchemy/Reservoir) and on-chain (viem contract reads). S3 for image storage |
| **Blockchain Dependencies** | viem for `tokenURI()`, `name()`, `symbol()`, `totalSupply()` reads. Alchemy/Reservoir APIs (EVM-specific) |
| **EVM Dependency Level** | **MEDIUM** |
| **Can Support Solana Directly** | **Partially** |
| **Recommended Approach** | **Refactor into multi-chain metadata service** |
| **Reasoning** | Metadata fetching has chain-specific and chain-agnostic parts. Chain-agnostic: IPFS/HTTP URI fetching, image processing, S3 upload, thumbnail generation, DB updates. Chain-specific: how to read metadata URI (EVM: tokenURI() call; Solana: Metaplex on-chain metadata account). Create a `MetadataFetcher` interface with EVM (viem) and Solana (@solana/web3.js + Metaplex) implementations. API sources change too (Helius/Tensor replace Alchemy/Reservoir for Solana) |

---

## 4. Frontend Repositories

### 4.1 lp-bonds-webapp (`evm-frontend/lp-bonds-webapp/`)

| Field | Detail |
|-------|--------|
| **Purpose** | LP bonds marketplace UI: create bonds, portfolio, trading, evolution, tokenization |
| **Current Architecture** | React 18, wagmi 2.x, viem, Redux, MUI. WalletContext wraps wagmi hooks. Contract interaction via viem `readContract`/`writeContract` with ABIs. Lock handler registry (per-DEX). Calls lp-bond-amount-fetcher oracle. Client SDK for API calls |
| **Blockchain Dependencies** | wagmi (EVM wallet adapter), viem (contract calls), EVM ABIs, chain configs (mainnet, arbitrum, apechain, etc.), EIP-712 for orders |
| **EVM Dependency Level** | **HIGH** |
| **Can Support Solana Directly** | **No (as-is)** |
| **Recommended Approach** | **Refactor into multi-chain frontend OR create Solana-specific frontend** |
| **Reasoning** | Two viable paths: (A) Refactor WalletContext to abstract chain type, add Solana wallet adapter alongside wagmi, create transaction builder abstraction for both chains. This is complex but avoids duplication. (B) Create a separate Solana-specific frontend using `@solana/wallet-adapter-react` and `@coral-xyz/anchor` client. Simpler initially but doubles maintenance. Recommendation: **Option A** with a phased approach — first add Solana support side-by-side, then gradually unify |

---

### 4.2 launchpad-webapp (`evm-frontend/launchpad-webapp/`)

| Field | Detail |
|-------|--------|
| **Purpose** | NFT launchpad UI: create collections, configure presale/public sale, mint NFTs |
| **Current Architecture** | React 18, wagmi, @tanstack/react-query, Redux Toolkit, MUI |
| **Blockchain Dependencies** | wagmi for wallet, viem for contract interaction, EVM chain configs |
| **EVM Dependency Level** | **HIGH** |
| **Can Support Solana Directly** | **No (as-is)** |
| **Recommended Approach** | **Create Solana-specific frontend initially, plan multi-chain refactor later** |
| **Reasoning** | The launchpad UX is simpler than the LP bonds webapp, and the Solana launchpad program will have very different interaction patterns (Metaplex candy machine-style). A separate frontend is faster to build and avoids coupling. Multi-chain can be introduced later if both chains are active |

---

## 5. Already-Migrated Repositories

### 5.1 solana-lp-bonds-contracts

| Field | Detail |
|-------|--------|
| **Purpose** | Anchor programs for LP bond creation, locking, evolution on Solana |
| **Architecture** | Two programs: `lp_bonds` (lock + mint), `lp_bonds_evolution` (level upgrades). Orca Whirlpool CPI for liquidity. Ed25519 oracle verification. PDA-based state |
| **Status** | **Migrated and tested** |
| **Gap** | LPBondsExchange (tokenization: burn bond → mint ERC20) not present. May need additional instruction or program |

### 5.2 solana-price-oracle

| Field | Detail |
|-------|--------|
| **Purpose** | Express API computing Orca Whirlpool position amounts with Ed25519 signing |
| **Architecture** | Direct Solana account fetching, custom Whirlpool layout decoding, Q64.64 math, tweetnacl signing |
| **Status** | **Migrated and tested** |

---

## 6. Repository Classification Summary Table

| # | Repository | Purpose | EVM Dep | Solana Direct? | Recommended Approach |
|---|-----------|---------|---------|----------------|---------------------|
| 1 | `evm-contracts/token` | ERC20 tokens (GMI, GMI CV, LP) | HIGH | No | **Create Solana Token-2022 program** |
| 2 | `evm-contracts/evm-contracts` | NFT marketplace (Exchange, Royalties) | HIGH | No | **Create Solana marketplace Anchor program** |
| 3 | `evm-contracts/liquidity-bonds-contracts` | LP bond NFTs + locker | HIGH | N/A | **Already migrated** (verify tokenization gap) |
| 4 | `evm-contracts/launchpad-contracts` | NFT launchpad + refunds | HIGH | No | **Create Solana launchpad Anchor program** |
| 5 | `evm-apis/user` | User management API | LOW | Yes | **Extend with Solana chain support** |
| 6 | `evm-apis/general` | Core marketplace API | LOW | Yes | **Extend with Solana chain support** |
| 7 | `evm-apis/lpbonds` | LP bonds marketplace API | MED-HIGH | Partial | **Refactor with blockchain abstraction layer** |
| 8 | `evm-apis/lp-bond-amount-fetcher` | Position amount oracle | HIGH | N/A | **Already migrated** (keep both versions) |
| 9 | `evm-apis/rewards-service` | Daily reward calculation | LOW | Yes | **Extend with Solana oracle routing** |
| 10 | `evm-apis/order-book` | Order matching (Go) | HIGH | No | **Create Solana-specific order-book** |
| 11 | `evm-indexers/volume-indexer` | Marketplace volume tracking | HIGH | No | **Create Solana volume indexer** |
| 12 | `evm-indexers/liquidity-indexer` | Bridge/LP airdrop tracking | HIGH | No | **Evaluate need, create if required** |
| 13 | `evm-indexers/nft-indexer` | NFT ownership/transfers | HIGH | No | **Create Solana NFT indexer** |
| 14 | `evm-indexers/metadata-indexer` | NFT metadata enrichment | MEDIUM | Partial | **Refactor into multi-chain service** |
| 15 | `evm-frontend/lp-bonds-webapp` | LP bonds marketplace UI | HIGH | No | **Refactor into multi-chain frontend** |
| 16 | `evm-frontend/launchpad-webapp` | Launchpad UI | HIGH | No | **Create Solana-specific frontend** |

---

## 7. Detailed Decision Matrix

### Repositories to REUSE (extend with Solana support)

| Repository | Changes Required | Effort |
|-----------|-----------------|--------|
| `user` API | Add Solana chain enum, SNS lookup, Ed25519 auth, Solana DB tables | 2–3 days |
| `general` API | Solana address validation, route to Solana order-book, Solana external APIs | 2–3 days |
| `rewards-service` | Route Solana bonds to solana-price-oracle, decimal conversion | 1–2 days |

### Repositories requiring NEW Solana implementations

| Repository | New Solana Repo | Effort |
|-----------|----------------|--------|
| `token` contracts | `solana-token-program` | 5–7 days |
| `evm-contracts` marketplace | `solana-marketplace-program` | 10–14 days |
| `launchpad-contracts` | `solana-launchpad-program` | 7–10 days |
| `order-book` API | `solana-order-book` | 5–7 days |
| `volume-indexer` | `solana-volume-indexer` | 5–7 days |
| `nft-indexer` | `solana-nft-indexer` | 5–7 days |

### Repositories to REFACTOR (multi-chain abstraction)

| Repository | Abstraction Needed | Effort |
|-----------|-------------------|--------|
| `lpbonds` API | `ILockerService` + Solana implementation | 3–5 days |
| `metadata-indexer` | `IMetadataFetcher` + Solana implementation | 3–4 days |
| `lp-bonds-webapp` | Wallet abstraction + transaction builder pattern | 7–10 days |

### Repositories to SHARE (business logic reuse)

| Component | Shared Logic | Chain-Specific |
|-----------|-------------|----------------|
| Reward formula | `(amount0 * 2) / 365 * multiplier` | Oracle endpoint URL |
| API client SDK | REST API types, response models | Transaction building |
| DB schema | NftContract, NftToken, Orders models | Chain-prefixed tables |
| Metadata processing | IPFS fetch, S3 upload, thumbnails | On-chain metadata read |

### Repositories to KEEP SEPARATE (dual chain)

| EVM Version | Solana Version | Reason |
|------------|----------------|--------|
| `lp-bond-amount-fetcher` | `solana-price-oracle` | Fundamentally different math + signing |
| `liquidity-bonds-contracts` | `solana-lp-bonds-contracts` | Already separate |
| EVM `order-book` | Solana `order-book` | Different signing/verification |
