# Multi-Chain Architecture Recommendations

## 1. Architectural Principles

The GMI protocol must evolve from an EVM-only system to a multi-chain architecture supporting both EVM and Solana. The following principles guide all recommendations:

1. **Chain abstraction at service boundaries** — Business logic remains chain-agnostic; blockchain interactions are isolated behind interfaces
2. **Shared data models, separate indexing** — Database schemas are unified where possible; data ingestion is chain-specific
3. **Separate on-chain programs** — Smart contracts / programs are inherently chain-specific and cannot be shared
4. **Unified API surface** — Consumers (frontends, external integrations) interact with a single API that routes internally
5. **Progressive migration** — Solana support is added incrementally without breaking existing EVM functionality

---

## 2. Target Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        FRONTEND LAYER                           │
│                                                                 │
│  ┌──────────────────────┐    ┌──────────────────────┐          │
│  │   LP Bonds Webapp    │    │   Launchpad Webapp   │          │
│  │   (Multi-chain)      │    │   (Solana-specific)  │          │
│  │                      │    │                      │          │
│  │  ┌────────────────┐  │    │  Solana Wallet       │          │
│  │  │ Chain Adapter   │  │    │  Adapter + Anchor    │          │
│  │  │ ┌────┐ ┌─────┐ │  │    │  Client              │          │
│  │  │ │EVM │ │ SOL │ │  │    │                      │          │
│  │  │ └────┘ └─────┘ │  │    │                      │          │
│  │  └────────────────┘  │    └──────────────────────┘          │
│  └──────────────────────┘                                       │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                         API LAYER                                │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │ User API │ │General   │ │LP Bonds  │ │ Rewards Service  │  │
│  │(extended)│ │API       │ │API       │ │ (extended)       │  │
│  │          │ │(extended)│ │(refactor)│ │                  │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  │
│                                                                 │
│  ┌──────────────────────┐ ┌──────────────────────┐             │
│  │ EVM Order Book (Go)  │ │ Solana Order Book    │             │
│  │ (existing)           │ │ (new)                │             │
│  └──────────────────────┘ └──────────────────────┘             │
│                                                                 │
│  ┌──────────────────────┐ ┌──────────────────────┐             │
│  │ EVM Position Oracle  │ │ Solana Price Oracle  │             │
│  │ (existing)           │ │ (existing)           │             │
│  └──────────────────────┘ └──────────────────────┘             │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                       INDEXER LAYER                              │
│                                                                 │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │  EVM Indexers     │  │  Solana Indexers  │                   │
│  │  ┌─────────────┐ │  │  ┌─────────────┐ │                   │
│  │  │ Volume      │ │  │  │ Volume      │ │                   │
│  │  │ NFT         │ │  │  │ NFT/Account │ │                   │
│  │  │ Liquidity   │ │  │  │ Metadata    │ │                   │
│  │  │ Metadata    │ │  │  └─────────────┘ │                   │
│  │  └─────────────┘ │  │                  │                    │
│  └──────────────────┘  └──────────────────┘                    │
│              │                    │                              │
│              └────────┬───────────┘                              │
│                       ▼                                         │
│            ┌──────────────────┐                                 │
│            │  Shared Database  │                                 │
│            │  (PostgreSQL)     │                                 │
│            │  Chain-prefixed   │                                 │
│            │  tables           │                                 │
│            └──────────────────┘                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                     ON-CHAIN LAYER                               │
│                                                                 │
│  ┌──────────────────────┐  ┌──────────────────────────────┐    │
│  │       EVM             │  │         SOLANA                │    │
│  │  GMI/CV/LP Tokens     │  │  Token-2022 Mints            │    │
│  │  Exchange + Royalties │  │  Marketplace Program          │    │
│  │  LP Bond Locker       │  │  LP Bonds + Evolution         │    │
│  │  Launchpad            │  │  Launchpad Program            │    │
│  │  Uniswap V3 Pools     │  │  Orca Whirlpools             │    │
│  └──────────────────────┘  └──────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Contract Layer Architecture

### 3.1 Token Migration (Token-2022)

The user has specified that EVM token contracts will follow the **Token-2022** program standard on Solana.

**Architecture:**

```
┌───────────────────────────────────────────────┐
│            Token Authority Program             │
│          (Anchor, custom program)              │
│                                                │
│  Instructions:                                 │
│  ├─ initialize_token_mint                      │
│  │   → Create Token-2022 mint with extensions  │
│  ├─ add_minter / remove_minter                 │
│  │   → PDA: MinterConfig["minter", mint, user] │
│  ├─ mint_tokens                                │
│  │   → Minter-only, enforce max_supply for GMI │
│  ├─ update_trade_allowance (GMI CV only)       │
│  │   → PDA: TradeConfig["trade", mint]         │
│  └─ pause / unpause                            │
│      → PDA: TokenConfig["token_config", mint]  │
│                                                │
│  Token-2022 Extensions Used:                   │
│  ├─ MetadataPointer + TokenMetadata            │
│  ├─ TransferHook (GMI CV trade restrictions)   │
│  ├─ MintCloseAuthority                         │
│  └─ PermanentDelegate (if burn needed)         │
└───────────────────────────────────────────────┘
```

**Key Decisions:**
- GMI CV's `tradeAllowance` modifier maps to a **Transfer Hook** program that checks allowed exchanges (stored as PDA whitelist)
- Max supply for GMI is enforced in the authority program's `mint_tokens` instruction
- LP Token has no supply cap; authority program simply controls who can mint
- Minter roles stored as PDAs: `["minter", mint_pubkey, minter_pubkey]`

### 3.2 Marketplace Program

```
┌───────────────────────────────────────────────┐
│           Marketplace Program                  │
│              (Anchor)                          │
│                                                │
│  Instructions:                                 │
│  ├─ initialize                                 │
│  │   → ProtocolConfig PDA (fee BPS, receiver)  │
│  ├─ match_orders                               │
│  │   → Ed25519 signature verification          │
│  │   → SPL Token / Token-2022 transfers        │
│  │   → SOL transfers (native)                  │
│  │   → Royalty distribution                    │
│  │   → PDA: Fill["fill", order_hash]           │
│  ├─ cancel_order                               │
│  │   → PDA: Cancel["cancel", order_hash]       │
│  ├─ update_protocol_fee                        │
│  ├─ update_fee_receiver                        │
│  └─ update_royalties                           │
│      → RoyaltyConfig PDA per collection        │
│                                                │
│  Asset Types:                                  │
│  ├─ SOL (native lamports)                      │
│  ├─ SPL Token (Token program)                  │
│  ├─ Token-2022                                 │
│  └─ Metaplex NFT (token + metadata + edition)  │
│                                                │
│  Royalty Sources:                              │
│  ├─ On-chain RoyaltyConfig PDA                 │
│  ├─ Metaplex metadata (seller_fee_basis_points)│
│  └─ Programmable NFT (pNFT) enforcement        │
└───────────────────────────────────────────────┘
```

**Key Differences from EVM:**
- No partial fills in the same way (Solana transactions are atomic); partial fill state tracked via PDA
- Order hash computed off-chain, verified on-chain via Ed25519
- No WETH equivalent; native SOL handled directly via system program
- Asset transfers use CPI to Token/Token-2022/Metaplex programs
- Collection bids use Metaplex collection verification (verified collection field)

### 3.3 Launchpad Program

```
┌───────────────────────────────────────────────┐
│           Launchpad Program                    │
│              (Anchor)                          │
│                                                │
│  Instructions:                                 │
│  ├─ create_collection                          │
│  │   → CollectionConfig PDA                    │
│  │   → Metaplex Collection NFT                 │
│  ├─ configure_phases                           │
│  │   → PhaseConfig PDA (presale, public)       │
│  │   → Merkle root for whitelist               │
│  ├─ mint_nft                                   │
│  │   → Phase validation                        │
│  │   → Mint limit per wallet (PDA counter)     │
│  │   → SOL payment to escrow PDA               │
│  │   → Metaplex NFT minting CPI               │
│  ├─ refund                                     │
│  │   → Burn NFT via Metaplex CPI               │
│  │   → Return SOL from escrow (100% or 80%)    │
│  ├─ owner_mint                                 │
│  │   → Reserve allocation                      │
│  └─ withdraw                                   │
│      → Creator withdrawal from escrow          │
│                                                │
│  Refund Model:                                 │
│  ├─ EscrowVault PDA holds mint proceeds        │
│  ├─ RefundConfig PDA (rate: 100% or 80%)       │
│  └─ RefundReceipt PDA tracks refunded tokens   │
│                                                │
│  NFT Standard: Metaplex Token Metadata         │
│  ├─ Regular NFTs (standard)                    │
│  ├─ pNFTs (for marketplace restriction)        │
│  └─ Compressed NFTs (for large collections)    │
└───────────────────────────────────────────────┘
```

---

## 4. API Layer Architecture

### 4.1 Chain Abstraction Pattern

For APIs being extended (user, general, lpbonds, rewards-service), implement a **blockchain service abstraction**:

```typescript
// Shared interface
interface IBlockchainService {
  isValidAddress(address: string): boolean;
  resolveNameService(address: string): Promise<string | null>;
  getBlockTimestamp(): Promise<number>;
}

// EVM implementation
class EVMBlockchainService implements IBlockchainService {
  isValidAddress(address: string) { return ethers.utils.isAddress(address); }
  async resolveNameService(address: string) { /* ENS lookup */ }
  async getBlockTimestamp() { /* eth_getBlockByNumber */ }
}

// Solana implementation
class SolanaBlockchainService implements IBlockchainService {
  isValidAddress(address: string) {
    try { new PublicKey(address); return true; } catch { return false; }
  }
  async resolveNameService(address: string) { /* SNS lookup */ }
  async getBlockTimestamp() { /* getSlot + getBlockTime */ }
}

// Factory
function getBlockchainService(chain: string): IBlockchainService {
  if (SOLANA_CHAINS.includes(chain)) return new SolanaBlockchainService();
  return new EVMBlockchainService();
}
```

### 4.2 LP Bonds API Refactoring

The `lpbonds` API requires the deepest refactoring due to direct on-chain reads:

```typescript
// Current: tightly coupled to EVM
class LockerContractService {
  async getBasePosition(bondId: number, chainId: number) {
    const provider = new ethers.providers.StaticJsonRpcProvider(RPC_URLS[chainId]);
    const contract = new ethers.Contract(lockerAddress, lockerABI, provider);
    return contract.basePositions(bondId);
  }
}

// Target: abstracted
interface ILockerService {
  getPositionCustody(bondId: string, chain: string): Promise<PositionInfo>;
}

class EVMLockerService implements ILockerService {
  async getPositionCustody(bondId: string, chain: string) {
    // existing ethers.Contract logic
  }
}

class SolanaLockerService implements ILockerService {
  async getPositionCustody(bondMint: string, chain: string) {
    const connection = new Connection(SOLANA_RPC_URL);
    const [custodyPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("position_custody"), new PublicKey(bondMint).toBuffer()],
      LP_BONDS_PROGRAM_ID
    );
    const account = await connection.getAccountInfo(custodyPDA);
    // Deserialize using Anchor IDL
    return deserializePositionCustody(account.data);
  }
}
```

### 4.3 Order Book Architecture

The Solana order-book requires a new service but can share the database schema:

```
┌─────────────────────────────────────────────┐
│              Order Book Routing              │
│                                              │
│  Request arrives → check chain header        │
│  ├─ EVM chain → route to EVM order book     │
│  └─ Solana   → route to Solana order book   │
└─────────────────────────────────────────────┘

EVM Order Book (existing Go service):
  - EIP-712 signing
  - ECDSA verification
  - eth_getBlockByNumber for expiry
  - Uniswap V2 pricing

Solana Order Book (new service):
  - Ed25519 message signing
  - nacl.sign.detached.verify
  - Solana Clock for expiry
  - Jupiter/Orca pricing
  - Same DB schema (Orders table with chain prefix)
```

**Recommendation:** Build the Solana order-book in TypeScript (not Go) to leverage `@solana/web3.js` and `@coral-xyz/anchor` directly. The EVM Go service remains as-is.

### 4.4 Oracle Architecture (Already Complete)

```
EVM Oracle (lp-bond-amount-fetcher):     Solana Oracle (solana-price-oracle):
  - Uniswap V3 sqrtPriceX96               - Orca Whirlpool sqrtPriceX64
  - ECDSA signing                          - Ed25519 signing (tweetnacl)
  - ethers.Contract for positions          - Direct account fetch + decode
  - POST /position-info                    - POST /position-info (same path)
```

Both oracles maintain the same API contract (`POST /position-info`), enabling the rewards-service to route by chain without structural changes.

---

## 5. Indexer Layer Architecture

### 5.1 Solana Indexing Strategy

Solana lacks EVM-style event logs. The indexing approach must use one of:

| Method | Pros | Cons | Best For |
|--------|------|------|----------|
| **Geyser Plugin (Yellowstone gRPC)** | Real-time, low latency, account-level granularity | Infrastructure setup, requires validator access or hosted provider | NFT indexer, volume indexer |
| **Helius Webhooks** | Managed, easy setup, transaction-level | Vendor dependency, may miss account changes | Volume tracking, metadata updates |
| **Transaction Polling** | Simple, no infra dependency | High latency, RPC rate limits | Low-frequency data |
| **Account Subscription (WebSocket)** | Real-time account changes | Connection management, no historical replay | Live position tracking |

**Recommended Approach: Helius Webhooks + Account Polling**

For a team migrating to Solana, Helius webhooks provide the best balance of reliability and development speed. Account polling supplements for specific PDA monitoring.

### 5.2 Solana NFT Indexer

```
┌─────────────────────────────────────────────────────┐
│              Solana NFT Indexer                       │
│                                                      │
│  Data Sources:                                       │
│  ├─ Helius Enhanced Transactions webhook             │
│  │   → NFT transfers (TOKEN_PROGRAM + METADATA)      │
│  │   → Collection creation events                    │
│  │   → Mint events                                   │
│  ├─ Account subscription (WebSocket)                 │
│  │   → Token account ownership changes               │
│  │   → Metadata account updates                      │
│  └─ Metaplex DAS API                                 │
│      → getAssetsByOwner                              │
│      → getAssetsByGroup (collection)                 │
│                                                      │
│  Processing:                                         │
│  ├─ Parse transaction instructions                   │
│  │   → Identify program (Metaplex, Token, System)    │
│  │   → Extract mint, from, to, amount                │
│  ├─ Deserialize account data                         │
│  │   → Token accounts → ownership                    │
│  │   → Metadata accounts → collection, attributes    │
│  └─ Update shared DB tables                          │
│      → SolanaNftContracts (collections)              │
│      → SolanaNftTokens (individual NFTs)             │
│      → SolanaNftTransfers (transfer history)         │
│      → SolanaNftOwners (ownership mapping)           │
│                                                      │
│  Tech: Node.js + @solana/web3.js + Helius SDK        │
└─────────────────────────────────────────────────────┘
```

### 5.3 Solana Volume Indexer

```
┌─────────────────────────────────────────────────────┐
│            Solana Volume Indexer                      │
│                                                      │
│  Data Sources:                                       │
│  ├─ Helius webhook (filtered by program ID)          │
│  │   → Marketplace program transactions              │
│  │   → match_orders instruction parsing              │
│  └─ Jupiter Price API                                │
│      → SPL token → USD conversion                    │
│                                                      │
│  Processing:                                         │
│  ├─ Decode match_orders instruction data             │
│  │   → Maker/taker, asset classes, amounts           │
│  ├─ Compute USD volume                               │
│  │   → SOL price from Jupiter/CoinGecko              │
│  │   → SPL token prices from Jupiter                 │
│  └─ Update SolanaTransactionVolume table             │
│                                                      │
│  DB: Shared PostgreSQL with chain-prefixed tables    │
└─────────────────────────────────────────────────────┘
```

### 5.4 Metadata Indexer (Multi-Chain Refactor)

```typescript
// Shared interface
interface IMetadataFetcher {
  fetchContractMetadata(address: string): Promise<ContractMetadata>;
  fetchTokenMetadata(address: string, tokenId: string): Promise<TokenMetadata>;
  fetchTokenURI(address: string, tokenId: string): Promise<string>;
}

// EVM implementation (existing)
class EVMMetadataFetcher implements IMetadataFetcher {
  // viem contract reads for tokenURI, name, symbol
  // Alchemy/Reservoir API fallback
}

// Solana implementation (new)
class SolanaMetadataFetcher implements IMetadataFetcher {
  async fetchContractMetadata(collectionMint: string) {
    // Metaplex DAS API: getAsset(collectionMint)
    // Or: fetch Metadata PDA and deserialize
  }
  async fetchTokenMetadata(collectionMint: string, tokenMint: string) {
    // Metaplex DAS API: getAsset(tokenMint)
    // Includes on-chain attributes, URI, verified collection
  }
  async fetchTokenURI(collectionMint: string, tokenMint: string) {
    // Read Metadata PDA → uri field
  }
}

// Shared processing pipeline (unchanged)
// URI → fetch JSON → extract image → upload S3 → generate thumbnails → update DB
```

---

## 6. Frontend Architecture

### 6.1 LP Bonds Webapp — Multi-Chain Wallet Abstraction

```typescript
// Chain-agnostic wallet interface
interface IWalletAdapter {
  address: string | null;
  chainType: 'evm' | 'solana';
  connected: boolean;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  signMessage(message: Uint8Array): Promise<Uint8Array>;
}

// Chain-agnostic transaction builder
interface ITransactionBuilder {
  buildLockPosition(params: LockParams): Promise<TransactionRequest>;
  buildRedeemBond(params: RedeemParams): Promise<TransactionRequest>;
  buildEvolveBond(params: EvolveParams): Promise<TransactionRequest>;
  submitTransaction(tx: TransactionRequest): Promise<string>;
}

// EVM implementation
class EVMTransactionBuilder implements ITransactionBuilder {
  // Uses viem writeContract with ABIs
}

// Solana implementation
class SolanaTransactionBuilder implements ITransactionBuilder {
  // Uses @coral-xyz/anchor Program client
  // Builds Anchor instructions with account constraints
}
```

**UI Chain Selector Pattern:**

```
┌──────────────────────────────────────┐
│  Chain Selector (header component)    │
│  ┌─────────┐  ┌──────────────────┐  │
│  │   EVM   │  │     Solana       │  │
│  │ Networks │  │   (devnet/      │  │
│  │ dropdown │  │    mainnet)     │  │
│  └─────────┘  └──────────────────┘  │
│                                      │
│  When EVM selected:                  │
│    → wagmi WagmiProvider active      │
│    → viem contract calls             │
│    → EVM ABIs loaded                 │
│                                      │
│  When Solana selected:               │
│    → SolanaWalletProvider active     │
│    → Anchor client calls             │
│    → IDL loaded                      │
│                                      │
│  Shared: API client SDK, UI shell,   │
│  portfolio views, stats, metadata    │
└──────────────────────────────────────┘
```

### 6.2 Launchpad Webapp — Solana-Specific

Given the launchpad involves a completely different NFT creation paradigm on Solana (Metaplex vs ERC721), building a separate frontend is more practical:

- **Solana wallet adapter** (`@solana/wallet-adapter-react`)
- **Metaplex SDK** for collection creation and NFT minting
- **Anchor client** for launchpad program interaction
- Separate deploy target
- Shared design system (MUI components) via package extraction

---

## 7. Database Architecture

### 7.1 Multi-Chain Table Strategy

The existing pattern of chain-prefixed tables (e.g., `EthereumNftContracts`, `EnergiOrders`) extends naturally to Solana:

```sql
-- New Solana tables (following existing convention)
CREATE TABLE "SolanaNftContracts" (
  "address" VARCHAR(44) PRIMARY KEY,  -- base58 pubkey (longer than EVM 0x...)
  "type" VARCHAR(10),                  -- 'metaplex_nft', 'token_2022', etc.
  "name" VARCHAR(255),
  "symbol" VARCHAR(50),
  "metaStatus" VARCHAR(20),
  -- ... shared columns
);

CREATE TABLE "SolanaNftTokens" (
  "contractAddress" VARCHAR(44),       -- collection mint
  "tokenId" VARCHAR(44),               -- token mint (pubkey, not uint256)
  "owner" VARCHAR(44),
  "metadata" JSONB,
  -- ... shared columns
);

CREATE TABLE "SolanaOrders" (
  "key" VARCHAR(88),                   -- order hash (base58)
  "maker" VARCHAR(44),
  "makerToken" VARCHAR(44),
  "takerToken" VARCHAR(44),
  -- ... same structure as EVM orders
);

CREATE TABLE "SolanaTransactionVolume" (
  "txSignature" VARCHAR(88),           -- Solana tx signature (not txHash)
  "tokenAddress" VARCHAR(44),
  "tokenId" VARCHAR(44),
  "fromAddress" VARCHAR(44),
  "toAddress" VARCHAR(44),
  "value" DECIMAL,
  "valueUsd" DECIMAL,
  "slot" BIGINT,                       -- Solana slot (not blockNumber)
  "market" VARCHAR(50),
  -- ... shared columns
);
```

### 7.2 Key Schema Differences

| Field | EVM | Solana |
|-------|-----|--------|
| Address format | `0x` + 40 hex chars (42 total) | Base58, up to 44 chars |
| Token ID | uint256 (numeric) | Pubkey (base58 string) |
| Transaction ID | `0x` + 64 hex (txHash) | Base58 signature (88 chars) |
| Block reference | blockNumber (uint) | slot (uint) |
| Timestamp source | block.timestamp | getBlockTime(slot) |

---

## 8. New Repositories to Create

Based on the analysis, the following new repositories are required:

| New Repository | Type | Language | Purpose |
|----------------|------|----------|---------|
| `solana-token-program` | Anchor program | Rust | Token-2022 authority + transfer hook |
| `solana-marketplace-program` | Anchor program | Rust | Order matching + settlement |
| `solana-launchpad-program` | Anchor program | Rust | NFT launchpad with refund |
| `solana-order-book` | API service | TypeScript | Ed25519 order management |
| `solana-nft-indexer` | Indexer | TypeScript | NFT ownership + transfer tracking |
| `solana-volume-indexer` | Indexer | TypeScript | Marketplace volume tracking |

**Repositories NOT needed (extend existing instead):**
- No new user API repo (extend existing)
- No new general API repo (extend existing)
- No new rewards-service repo (extend existing)
- No new metadata-indexer repo (refactor existing)
- No new lp-bonds webapp repo (refactor existing)

---

## 9. Shared Package Extraction

To reduce duplication, extract shared logic into internal packages:

```
packages/
├── shared-types/         # TypeScript types shared across APIs
│   ├── order.ts          # Order model (chain-agnostic fields)
│   ├── nft.ts            # NFT/collection models
│   └── chain.ts          # Chain enum, config types
│
├── shared-db-models/     # Sequelize models for all chains
│   ├── nft-contract.ts
│   ├── nft-token.ts
│   ├── orders.ts
│   └── chain-factory.ts  # Generates chain-prefixed models
│
├── blockchain-utils/     # Chain abstraction utilities
│   ├── address.ts        # isValidAddress(chain, addr)
│   ├── decimals.ts       # formatUnits/parseUnits
│   └── name-service.ts   # ENS + SNS resolution
│
└── api-client/           # Frontend API client SDK
    ├── base.ts
    ├── user.ts
    ├── lpbonds.ts
    └── general.ts
```

---

## 10. Summary of Recommendations

### Immediate Actions (Pre-Migration)
1. Define and publish Anchor IDLs for all new Solana programs
2. Establish shared TypeScript types package
3. Set up Solana devnet infrastructure (RPC, Helius account)
4. Create chain abstraction interfaces in existing APIs

### Migration Order
1. Token program (foundation for all other programs)
2. Marketplace program (needed for trading)
3. Launchpad program (independent feature)
4. Order book service (needed for marketplace)
5. NFT indexer + volume indexer (needed for UI)
6. API extensions (user, general, lpbonds, rewards)
7. Frontend refactoring (final consumer of all above)

### Anti-Patterns to Avoid
- **Don't fork EVM repos for Solana** — leads to divergent maintenance
- **Don't add Solana logic inline in EVM-specific code** — use abstraction interfaces
- **Don't duplicate business logic** — extract shared packages
- **Don't build Solana indexers using EVM patterns** — embrace account-based indexing
- **Don't mix wallet providers in a single context** — separate EVM and Solana wallet contexts cleanly
