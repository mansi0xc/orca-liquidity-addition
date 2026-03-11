# Full Stack Solana Integration Roadmap

## Constraints

- **Hard deadline:** First week of April 2026
- **Integration window:** 14 days maximum
- **Prerequisite:** All Solana programs deployed to devnet (mainnet deployment runs on a parallel track or follows)
- **Prerequisite:** `solana-price-oracle` already migrated and tested
- **Team:** 3–4 engineers working in parallel

## Assumptions

1. Solana programs are deployed on devnet with stable IDLs (no breaking instruction changes during integration)
2. Helius account provisioned with devnet + mainnet API keys
3. PostgreSQL database accessible from all services
4. Engineers have access to the existing EVM codebases and can modify them
5. Frontend deployment pipeline (Vercel/similar) is already set up
6. All engineers can work full days during the 14-day window

## Team Allocation

| Engineer | Primary Focus | Secondary |
|----------|--------------|-----------|
| **E1 (Backend Senior)** | LP Bonds API refactoring, Rewards Service | User API, General API |
| **E2 (Backend)** | Solana Order Book (new service) | General API support |
| **E3 (Indexer/Backend)** | NFT Indexer, Volume Indexer, Metadata Indexer | Monitoring |
| **E4 (Frontend)** | Wallet integration, transaction builders, UI | Launchpad frontend |

---

## DAY-BY-DAY EXECUTION ROADMAP

## DAY 1: Foundation & Shared Infrastructure

**Objective:** Establish all shared infrastructure, packages, and database migrations that every subsequent task depends on

**Tasks:**

E1 — Blockchain Abstraction Package:
- Create `packages/blockchain-utils/` with `IBlockchainService` interface (methods: `isValidAddress`, `resolveNameService`, `getBlockTimestamp`)
- Implement `EVMBlockchainService` (extract from existing code — ethers.utils.isAddress, ENS)
- Implement `SolanaBlockchainService` (`@solana/web3.js` PublicKey validation, `@bonfida/spl-name-service` for SNS)
- Implement `getBlockchainService(chain)` factory function
- Create chain-agnostic `formatUnits(value, decimals)` and `parseUnits(value, decimals)` utilities
- Add `solana` and `solana-devnet` to the global chain enum used by `blockchainMiddleware`

E2 — Database Migrations:
- Write Sequelize migration scripts for all new Solana tables: `SolanaNftContracts` (address VARCHAR(44)), `SolanaNftTokens` (tokenId VARCHAR(44)), `SolanaOrders` (key VARCHAR(88), maker VARCHAR(44)), `SolanaNftTransfers`, `SolanaNftOwners`, `SolanaTransactionVolume` (txSignature VARCHAR(88), slot BIGINT instead of blockNumber)
- Run migrations against development database and verify table creation
- Create Sequelize model definitions for each new table following existing chain-prefix pattern

E3 — Solana RPC & Indexer Infrastructure:
- Provision Helius devnet + mainnet RPC endpoints
- Configure WebSocket endpoints for account subscriptions
- Set up Helius webhook receiving endpoint (Express server scaffold for indexer webhooks)
- Register Helius webhooks for LP Bonds and Marketplace program IDs on devnet
- Test webhook delivery with a devnet transaction

E4 — Frontend Foundation:
- Add `@solana/wallet-adapter-react`, `@solana/wallet-adapter-wallets`, `@solana/wallet-adapter-base`, `@coral-xyz/anchor` to lp-bonds-webapp dependencies
- Copy Anchor IDL JSON files (lp_bonds.json, lp_bonds_evolution.json, marketplace.json, launchpad.json) into `src/idl/`
- Generate TypeScript types from IDLs
- Create `SolanaWalletProvider` component wrapping `ConnectionProvider` + `WalletProvider` with Phantom and Solflare adapters

**Deliverable:** Blockchain abstraction package published locally. All Solana DB tables created. Helius webhooks receiving devnet events. Frontend has Solana wallet dependencies and IDL types.

**Dependency:** None — this is Day 1

**Risk:** Helius webhook setup may take longer than expected if devnet rate limits are strict. Fallback: use transaction polling for Day 1, switch to webhooks when approved. IDL generation requires programs to be built — ensure devnet IDLs are committed to repo.

---

## DAY 2: User API + General API Solana Support

**Objective:** Extend the two lowest-dependency APIs to support Solana chains

**Tasks:**

E1 — User API (`evm-apis/user`):
- Add `@solana/web3.js` and `@bonfida/spl-name-service` as dependencies
- Update `blockchainMiddleware` to accept `solana` in the chain header and create `SolanaBlockchainService` context
- Add Solana table model mappings in `blockchainContext` (SolanaNftContracts, SolanaOrders, etc.)
- Replace `ethers.utils.isAddress` calls with chain-aware validation from blockchain-utils package
- Add Solana Name Service (SNS) resolution alongside ENS in `refreshWeb3Domains`
- Update `formatUnits` calls to handle SOL (9 decimals) vs ETH (18 decimals) via blockchain-utils
- Ensure User model address column supports VARCHAR(44) (widen if currently VARCHAR(42))
- Test all user endpoints (`/user/profile`, `/user/portfolio`, `/collection/bids`) with Solana addresses

E2 — General API (`evm-apis/general`):
- Add `solana` chain routing in order submission proxy (when `blockchain === 'solana'`, route to Solana order-book URL instead of EVM order-book)
- Integrate Jupiter Price API (`https://price.jup.ag/v6/price`) for SPL token pricing alongside CoinGecko
- Add SOL to the CoinGecko price cache (already fetches ETH — add SOL ID)
- Replace `ethers.utils.isAddress` with chain-aware validation
- Add Solana NFT data source: integrate Helius DAS API (`getAssetsByGroup`, `getAsset`) as alternative to NFTGo/Reservoir when `chain === 'solana'`
- Update stats queries to include Solana tables when chain header is `solana`
- Test NFT listing, collection stats, and order proxy endpoints with Solana data

E3 — Continue indexer infrastructure (parallel):
- Scaffold `solana-nft-indexer` project: Express + TypeScript, `@solana/web3.js`, Sequelize, Helius SDK
- Define webhook payload types for Helius Enhanced Transactions
- Implement instruction decoder for LP Bonds program (parse `add_liquidity_and_mint_bond`, `redeem_bond` instruction data using IDL)
- Implement instruction decoder for Metaplex Token Metadata program (parse NFT mint, transfer instructions)

E4 — Frontend wallet context (parallel):
- Build `SolanaWalletContext` component: wraps `useWallet`, `useConnection` hooks, exposes `address`, `connected`, `connect`, `disconnect`, `signMessage`, `sendTransaction`
- Build `ChainSelector` component: dropdown with EVM chains + Solana, stores selection in localStorage, dispatches chain change event
- Build `useChainAdapter` hook: returns `ChainAdapter` interface based on selected chain — if Solana returns SolanaWalletContext, if EVM returns existing WalletContext
- Wire `SolanaWalletProvider` into app root alongside existing `WagmiProvider`, gated by chain selection
- Test: connect Phantom on devnet, display address, disconnect

**Deliverable:** User API and General API accept `solana` chain and return correct responses. NFT indexer project scaffolded with instruction decoders. Frontend connects to Solana wallets.

**Dependency:** Day 1 — blockchain-utils package, DB tables, Helius webhooks

**Risk:** SNS resolution may have different latency profile than ENS — add timeout and fallback to raw address. Jupiter Price API may rate-limit on free tier — cache aggressively (60s TTL).

---

## DAY 3: LP Bonds API Refactoring (Part 1) + Order Book Start

**Objective:** Begin the deepest API refactoring (LP Bonds) and start the new Solana order-book service

**Tasks:**

E1 — LP Bonds API Refactoring (`evm-apis/lpbonds`):
- Define `ILockerService` interface: `getPositionCustody(bondId: string, chain: string): Promise<PositionCustodyData>`
- Extract existing EVM logic into `EVMLockerService` class implementing `ILockerService` (wraps existing `ethers.Contract` + locker ABI calls)
- Implement `SolanaLockerService` class implementing `ILockerService`:
  - Uses `@solana/web3.js` Connection to fetch PositionCustody PDA account
  - PDA derivation: `["position_custody", bond_mint_pubkey]` seeded from LP Bonds program ID
  - Deserialize account data using Borsh layout matching Anchor's `PositionCustody` struct (discriminator + fields)
  - Returns: bondMint, positionMint, whirlpool, tickLower, tickUpper, liquidity, depositor, createdAt, level, lockDuration, isEvolved
- Create `LockerServiceFactory` that returns EVM or Solana service based on chain parameter
- Update `LockerContractService` callers to use factory pattern
- Test SolanaLockerService against devnet LP Bonds program

E2 — Solana Order Book (new service, `solana-order-book`):
- Initialize project: Express + TypeScript, `@solana/web3.js`, `tweetnacl`, Sequelize, `pg`
- Create order model matching `SolanaOrders` table schema
- Implement Ed25519 signature verification module: `verifyOrderSignature(message: Uint8Array, signature: Uint8Array, pubkey: Uint8Array): boolean` using `nacl.sign.detached.verify`
- Define canonical order message format (byte layout): maker, taker, maker_asset (mint + amount + class), taker_asset (mint + amount + class), nonce, expiry
- Implement `POST /v1/create` endpoint: validate JWT, verify Ed25519 signature, check nonce, insert into SolanaOrders
- Implement `POST /v1/cancel` endpoint: verify maker signature, update order status

E3 — NFT Indexer development (parallel):
- Implement Helius webhook handler: receive enhanced transaction payload, route to appropriate processor based on program ID
- Build `NftTransferProcessor`: parse SPL Token transfer instructions, identify NFT transfers (amount=1, decimals=0), update SolanaNftOwners and SolanaNftTransfers
- Build `LPBondMintProcessor`: parse `add_liquidity_and_mint_bond` instruction, extract bond mint and position data, insert SolanaNftTokens with LP Bond type
- Build `CollectionCreateProcessor`: parse Metaplex `CreateCollection` instruction, insert SolanaNftContracts
- Connect processors to webhook handler with transaction routing

E4 — Frontend transaction builders (parallel):
- Create `SolanaLPBondsClient` class using `@coral-xyz/anchor` Program:
  - `addLiquidityAndMintBond(params)`: builds instruction with all required accounts (config PDA, whirlpool, token accounts, position mint, bond mint, etc.)
  - `redeemBond(bondMint)`: builds redeem instruction
  - `verifyCollateral(bondMint, oracleData)`: builds verify_collateral instruction with Ed25519 sysvar
- Create `SolanaEvolutionClient` class:
  - `evolveBond(sourceBondMint, targetLevel)`: builds evolve instruction
- Create `useLPBondsProgram()` hook: initializes Anchor program from IDL + wallet + connection
- Test: call `addLiquidityAndMintBond` on devnet from frontend

**Deliverable:** LP Bonds API has blockchain abstraction with working SolanaLockerService. Order book has create/cancel endpoints with Ed25519 verification. NFT indexer processes transfers and bond mints. Frontend can build LP bond transactions.

**Dependency:** Day 2 — blockchain-utils integrated into LP Bonds API, NFT indexer scaffold, frontend wallet context

**Risk:** Borsh deserialization of Anchor accounts requires exact struct layout matching. If fields don't align, data corruption occurs silently. Validate by deserializing a known devnet account and comparing every field. Anchor discriminator (first 8 bytes) must be skipped.

---

## DAY 4: LP Bonds API (Part 2) + Order Book + Rewards Service

**Objective:** Complete LP Bonds API refactoring, continue order book, update rewards service

**Tasks:**

E1 — LP Bonds API Completion:
- Update `PositionInfoService` to route oracle calls by chain: Solana bonds → `solana-price-oracle` URL, EVM bonds → `lp-bond-amount-fetcher` URL
- Update all BigNumber math to handle Solana amounts (BN from `@coral-xyz/anchor` vs ethers BigNumber)
- Update caching layer: Solana bond IDs are base58 strings (not numeric) — update cache key generation
- Update API response serialization: ensure bond IDs, addresses, transaction hashes use correct format per chain
- Update collection filtering logic to include Solana collections
- End-to-end test: API request for a devnet Solana LP bond → returns correct position custody data + oracle position info

E1 (afternoon) — Rewards Service Update (`evm-apis/rewards-service`):
- Add oracle URL routing: `ORACLE_URLS = { evm: process.env.EVM_ORACLE_URL, solana: process.env.SOLANA_ORACLE_URL }`
- Update oracle call logic: for Solana bonds, POST to solana-price-oracle with `{ tokenId: bondMint, chainId: 'solana', nonce, sender, contractAddress }`
- Replace `ethers.formatEther` / `ethers.parseEther` with chain-agnostic decimal formatting from blockchain-utils
- Verify `bondrewards` table `tokenid` column supports VARCHAR(44) for Solana base58 pubkeys (alter if numeric)
- Add `chainid = 'solana'` filter for Solana bond processing
- Test: insert a Solana bond into `bondrewards`, trigger reward calculation, verify oracle called correctly and reward computed

E2 — Solana Order Book continuation:
- Implement `GET /v1/maker/:address` — query orders by maker (Solana pubkey)
- Implement `GET /v1/taker-ct/:contract::tokenId` — query orders by target NFT
- Implement `GET /v1/hashkey/:key` — query order by hash
- Integrate Jupiter Price API for SPL token → USD pricing (used for order display values)
- Implement Solana clock-based order expiry: fetch latest slot via `getSlot()`, compare `getBlockTime(slot)` against order `matchBeforeTimestamp`
- Add periodic expiry sweep (cron every 60s): cancel expired orders
- Implement `GET /v1/health` endpoint: returns DB status, Solana RPC status, latest slot

E3 — NFT Indexer completion (parallel):
- Build `LPBondRedeemProcessor`: parse `redeem_bond` instruction, mark bond as redeemed in SolanaNftTokens
- Build `LPBondEvolveProcessor`: parse `evolve_bond` instruction, mark source bond evolved, create new bond entry
- Implement historical backfill: use `getSignaturesForAddress` + `getParsedTransaction` for LP Bonds program to index past transactions
- Add indexer health endpoint: returns last processed slot, slot lag, records processed
- Deploy to devnet and verify: create a bond → check indexer picked it up → verify DB entry

E4 — Frontend position info + oracle (parallel):
- Update `positionInfoFetcher.js`: route to solana-price-oracle when chain is Solana
- Create `useSolanaTokenBalances` hook: fetch SOL balance + SPL token balances via `getTokenAccountsByOwner`
- Update `useBasePosition` hook to work with Solana bond mints (base58 IDs instead of numeric)
- Create `SolanaTransactionConfirmation` component: shows Solana Explorer link, confirmation status (processed → confirmed → finalized)
- Test: fetch position info for a devnet Solana bond, display in UI

**Deliverable:** LP Bonds API fully supports Solana with oracle routing and correct serialization. Rewards service can process Solana bonds. Order book has full CRUD + expiry. NFT indexer handles all LP bond lifecycle events. Frontend fetches and displays Solana position data.

**Dependency:** Day 3 — ILockerService, SolanaLockerService, order book create/cancel, NFT indexer processors, frontend transaction builders

**Risk:** Rewards service `bondrewards` table schema change (tokenid column type) requires a migration that doesn't break existing EVM data. Use `ALTER TABLE` with `USING` clause to cast existing numeric values to VARCHAR, or add a new column `token_id_str` and migrate gradually.

---

## DAY 5: Order Book Completion + Volume Indexer + Frontend Lock Handlers

**Objective:** Finish the order book service, start the volume indexer, and wire up frontend lock/trade flows

**Tasks:**

E1 — Order Book integration testing:
- Write integration test suite: create order → query by maker → query by contract → cancel → verify state transitions
- Test Ed25519 signature verification with Phantom-signed messages
- Test order expiry sweep with artificially expired orders
- Load test: 100 concurrent order creates
- Deploy order-book to staging environment

E2 — Volume Indexer (`solana-volume-indexer`):
- Initialize project: Express + TypeScript, Sequelize, Helius SDK, Jupiter Price API client
- Register Helius webhook for Marketplace Program ID on devnet
- Implement `MatchOrdersProcessor`: decode `match_orders` instruction data from Marketplace program → extract maker, taker, asset mints, amounts, fees
- Implement volume calculation: fetch SOL/USD from CoinGecko, SPL token/USD from Jupiter → compute USD volume
- Insert into `SolanaTransactionVolume` table
- Implement hourly stats aggregation cron: aggregate into `SolanaStats1h` (volume, sales count, floor price per collection)
- Deploy to devnet and test with a mock marketplace transaction

E3 — Metadata Indexer Refactoring (`evm-indexers/metadata-indexer`):
- Define `IMetadataFetcher` interface: `fetchContractMetadata(address)`, `fetchTokenMetadata(address, tokenId)`, `fetchTokenURI(address, tokenId)`
- Extract existing logic into `EVMMetadataFetcher` (wraps viem tokenURI calls + Alchemy/Reservoir)
- Implement `SolanaMetadataFetcher`:
  - `fetchContractMetadata`: use Helius DAS API `getAsset(collectionMint)` → extract name, symbol, image, description
  - `fetchTokenMetadata`: use Helius DAS API `getAsset(tokenMint)` → extract metadata, attributes, image URI
  - `fetchTokenURI`: fetch Metaplex Metadata PDA → deserialize → return `uri` field
- Add chain routing in `IndexService`: check `metaStatus` on Solana tables, use SolanaMetadataFetcher
- Image processing pipeline (IPFS/HTTP fetch → S3 upload → thumbnail) remains unchanged (chain-agnostic)
- Test with devnet Metaplex NFTs

E4 — Frontend lock handler + trading UI:
- Add `orca-whirlpool` handler to lock handler registry (`lockHandlerRegistry.js`):
  - `approveTokens`: SPL Token approve for exact amount (no unlimited approval on Solana)
  - `lock`: calls `SolanaLPBondsClient.addLiquidityAndMintBond()`
- Update `CreateLPBonds` component: when Solana selected, show Orca Whirlpool position configuration (tick range, amounts), use Solana token balances, submit via Solana transaction builder
- Update `RequiredAmountsDisplay` to handle Solana oracle response format
- Add Solana Explorer links for transaction confirmations (replace Etherscan links when chain is Solana)
- Test: create an LP bond on devnet through the full UI flow

**Deliverable:** Order book service complete and deployed to staging. Volume indexer processing marketplace trades. Metadata indexer supports Solana NFTs. Frontend can create LP bonds on Solana through the UI.

**Dependency:** Day 4 — order book CRUD, NFT indexer live, frontend oracle integration

**Risk:** Volume indexer depends on the Marketplace program being active on devnet with test transactions. If no marketplace transactions exist yet, use synthetic test data and a script that submits mock marketplace instructions. Metadata indexer's Helius DAS API has different response shapes than Alchemy/Reservoir — map carefully.

---

## DAY 6: Frontend Portfolio + Trading + Marketplace UI

**Objective:** Complete the LP bonds webapp Solana user experience for portfolio management and trading

**Tasks:**

E1 — API endpoint verification:
- Verify all LP Bonds API endpoints return correct Solana data: `/v1/nfts` (list Solana LP bonds), `/v1/collections` (Solana collections), `/v1/portfolio` (owned Solana bonds), `/v1/nft/:mint` (single bond details)
- Verify General API endpoints work for Solana: `/nfts/collection/:address`, `/nfts/stats`, `/orders/*` (proxied to Solana order-book)
- Fix any serialization issues (address format, ID format, decimal handling)
- Document all API changes in an internal API changelog

E2 — Order book → General API wiring:
- Update General API order proxy: `POST /orders/listing/create` → route to Solana order-book `POST /v1/create` when `chain === 'solana'`
- Same for `/orders/listing/cancel`, `/orders/offer/create`, `/orders/offer/cancel`
- Verify order creation flow: frontend → General API → Solana order-book → SolanaOrders DB → query returns order
- Add validation: Solana orders must have base58 addresses and Ed25519 signatures

E3 — Indexer tuning (parallel):
- Monitor NFT indexer webhook processing latency — optimize if > 2s per event
- Add error handling and retry logic for failed webhook processing
- Implement dead letter queue for failed events (store in DB for manual replay)
- Verify volume indexer stats aggregation is correct by comparing with raw transaction data

E4 — Frontend portfolio + trading:
- Update `Portfolio` page: fetch owned Solana NFTs via LP Bonds API, display bond cards with metadata from Solana metadata indexer
- Update `AssetPage`: display bond details, position custody data, lock status, evolution eligibility, time until unlock
- Implement `ListSale` flow for Solana: build signed order message (Ed25519) → submit to order-book via General API → show confirmation
- Implement `AcceptBidsDrawer` for Solana: sign acceptance message → submit → show confirmation
- Update `ItemsTable` to display Solana NFT listings and offers
- Add Solana transaction history display (from SolanaTransactionVolume via API)

**Deliverable:** Full LP bonds portfolio and trading experience working on Solana through the frontend. APIs verified end-to-end.

**Dependency:** Day 5 — order book deployed, volume indexer live, frontend lock handlers, metadata indexer

**Risk:** Order message format between frontend (Ed25519 signing) and order-book (Ed25519 verification) must match exactly byte-for-byte. A mismatch means orders are rejected silently. Test with a hardcoded test vector first, then move to dynamic signing.

---

## DAY 7: Frontend Evolution + Redemption + Chain Selector Polish

**Objective:** Complete all remaining LP bonds webapp features for Solana and polish the chain switching experience

**Tasks:**

E1 — Backend bug fixes:
- Address any issues discovered during Day 6 frontend integration
- Verify rewards service cron processes Solana bonds correctly (manual trigger + verify DB update)
- Monitor and fix any indexer sync issues

E2 — Order book optimization:
- Implement order matching logic (when marketplace program emits a match, update both maker and taker orders in DB)
- Add WebSocket endpoint for real-time order updates (optional — if time permits, otherwise use polling)
- Performance test: 50 concurrent queries

E3 — Indexer enrichment (parallel):
- Wire metadata indexer to process newly indexed Solana NFTs: poll SolanaNftTokens where `metaStatus = 'new'`, fetch via DAS API, upload images to S3, update metadata fields
- Verify thumbnail generation works for Solana NFT images
- Add LP bond-specific metadata: decode on-chain bond metadata SVG or position info

E4 — Frontend evolution + redemption + polish:
- Implement bond evolution UI for Solana: select source bond → show evolution requirements → call `SolanaEvolutionClient.evolveBond()` → confirm → show new bond
- Implement bond redemption UI for Solana: show lock expiry countdown → when unlocked, call `SolanaLPBondsClient.redeemBond()` → confirm → show returned position
- Polish `ChainSelector` component: EVM networks in one group, Solana in another, visual divider, chain icon/logo per network
- Add chain-specific color theme hints (subtle background color change when on Solana vs EVM)
- Update wallet display: show Solana address in shortened format (first 4...last 4), show SOL balance
- Handle wrong wallet state: if user selects Solana but only EVM wallet connected, prompt Solana wallet connection
- Test complete user journey: connect Phantom → select Solana devnet → create bond → view in portfolio → list for sale → accept offer → evolve → redeem

**Deliverable:** LP bonds webapp fully functional on Solana: create, view, trade, evolve, redeem. Chain selector polished with clear UX.

**Dependency:** Day 6 — portfolio + trading UI, all APIs verified

**Risk:** Evolution flow involves multiple CPIs (burn source bond, add liquidity to new pool, mint new bond) — if any step fails, transaction reverts entirely. Frontend must show clear error messages. Test evolution with minimum amounts to reduce failure surface.

---

## DAY 8: Launchpad Frontend + Backend Hardening

**Objective:** Start the Solana launchpad frontend and harden all backend services for production

**Tasks:**

E1 — Backend service hardening:
- Add request validation middleware to all API endpoints (input sanitization, address format checks per chain)
- Add rate limiting to User API and General API Solana endpoints (prevent abuse during launch)
- Verify all database queries use indexes on Solana tables (add indexes: `SolanaNftTokens.owner`, `SolanaOrders.maker`, `SolanaTransactionVolume.tokenAddress`)
- Add structured logging to all Solana-specific code paths (log chain, program ID, pubkeys, latency)
- Verify error responses are consistent across EVM and Solana paths

E2 — Order book hardening:
- Add request validation (address format, signature length, nonce bounds)
- Add rate limiting per maker address
- Implement order count limits per maker (prevent spam)
- Add database connection pooling and timeout configuration
- Verify graceful shutdown behavior

E3 — Indexer hardening (parallel):
- Add webhook signature verification (Helius sends auth headers — validate them)
- Implement idempotency: if same transaction processed twice, don't duplicate records (check txSignature uniqueness)
- Add circuit breaker: if webhook processing fails 10x consecutively, alert and pause processing
- Verify backfill logic handles edge cases (reorgs on Solana are rare but possible — handle missing slots gracefully)
- Add metrics endpoints to all indexers: `/metrics` returning JSON with records_processed, latest_slot, slot_lag, errors

E4 — Launchpad Frontend:
- Create new React app (or new route within lp-bonds-webapp) for Solana launchpad
- Implement wallet connection using `@solana/wallet-adapter-react` (share with lp-bonds-webapp if same app)
- Build `CreateCollection` page: form for collection name, symbol, description, image upload, royalty %, max supply
- Integrate Metaplex SDK (`@metaplex-foundation/js`) for collection NFT creation
- Build phase configuration UI: presale start/end, public sale start/end, price per mint (SOL), max per wallet, whitelist upload (merkle tree generation)
- Wire up to Launchpad Anchor program: `create_collection` instruction, `configure_phases` instruction

**Deliverable:** Backend services hardened with validation, rate limiting, logging, and error handling. Launchpad frontend has collection creation and phase configuration.

**Dependency:** Day 7 — all core features complete, allowing focus on hardening

**Risk:** Launchpad frontend depends on the Launchpad Anchor program IDL being stable. If program changes are still happening, build against a mock/interface. Metaplex SDK version must match the on-chain Metaplex program version — verify compatibility.

---

## DAY 9: Launchpad Frontend Completion + Integration Testing Start

**Objective:** Complete launchpad frontend and begin systematic integration testing

**Tasks:**

E1 + E2 — Integration test suite (backend):
- Write test script that executes the full LP bond lifecycle via API calls:
  1. Create bond config (admin) → verify via API
  2. Lock position (simulated) → verify bond appears in NFT indexer data → verify via LP Bonds API
  3. Fetch position info from oracle → verify response shape and signature
  4. Trigger rewards calculation → verify bondrewards DB updated
  5. Create listing order → verify in order book → verify via General API
  6. Create offer → match → verify volume indexed → verify stats updated
  7. Evolve bond → verify old bond evolved, new bond created in indexer
  8. Redeem bond → verify bond marked redeemed in indexer
- Run test suite against devnet with real transactions
- Document all failures and categorize: API bug, indexer bug, order-book bug, data format mismatch

E3 — Indexer integration testing (parallel):
- Verify NFT indexer → metadata indexer pipeline: create bond on devnet → NFT indexer picks up → sets metaStatus=new → metadata indexer fetches metadata → S3 upload → DB updated with image URLs
- Verify volume indexer accuracy: compare indexed volumes against raw devnet transaction data
- Test indexer recovery: stop indexer → create 10 transactions → restart → verify backfill catches up
- Test webhook retry behavior: return 500 from webhook handler → verify Helius retries → accept on retry → verify no duplicates

E4 — Launchpad Frontend completion:
- Build `MintPage`: connect wallet → check whitelist eligibility → show price and supply remaining → mint button → submit `mint_nft` instruction → confirm → show minted NFT
- Build `RefundPage`: show owned NFTs from collection → select NFT → refund button → submit `refund` instruction → confirm → show SOL returned
- Build `CollectionDashboard`: display minted count, remaining supply, total revenue, holder count (from indexer data)
- Style all pages with MUI to match existing lp-bonds-webapp design language
- Test on devnet: create collection → configure phases → mint NFT → refund NFT

**Deliverable:** Launchpad frontend fully functional on devnet. Integration test suite covering full LP bond lifecycle. Indexer pipeline verified end-to-end.

**Dependency:** Day 8 — backend hardened, launchpad collection creation UI

**Risk:** Integration tests may reveal data format mismatches between services (e.g., API returns amount as string but frontend expects number). Allocate debugging time. If integration tests take longer than expected, deprioritize launchpad dashboard polish.

---

## DAY 10: End-to-End Testing + Cross-Service Bug Fixes

**Objective:** Execute comprehensive E2E tests across the full stack and fix all blocking bugs

**Tasks:**

E1 + E4 — End-to-end testing (frontend-driven):
- Test 1: Connect Phantom → select Solana devnet → create LP bond with SOL + test token → verify bond in portfolio → verify in API → verify in indexer
- Test 2: List bond for sale → verify order in order book → verify listing shows on marketplace page
- Test 3: Second wallet creates offer → accept offer → verify trade completes → verify volume indexed
- Test 4: Evolve bond from L1 to L2 → verify evolution UI works → verify indexer updates
- Test 5: Redeem bond after lock expiry → verify redemption UI → verify bond marked redeemed
- Test 6: Launchpad: create collection → mint NFT → view in portfolio → refund → verify SOL returned
- Test 7: Chain switching: complete actions on EVM (Sepolia) → switch to Solana → complete actions → switch back → verify no state corruption
- Test 8: Error handling: attempt operations with insufficient funds, expired orders, wrong wallet — verify graceful errors

E2 + E3 — Bug fixing (parallel, as tests reveal issues):
- Fix API response format issues
- Fix indexer data gaps or delays
- Fix order book edge cases
- Fix frontend transaction building errors
- Fix decimal/formatting inconsistencies between chains
- Fix any race conditions in concurrent order processing

All — Bug triage:
- P0 (blocks launch): fix immediately
- P1 (degrades experience): fix today if possible, otherwise Day 11
- P2 (cosmetic): defer to Day 12+

**Deliverable:** All 8 E2E test scenarios passing. All P0 bugs fixed. P1 bug list documented.

**Dependency:** Days 1–9 — all services deployed and functional

**Risk:** E2E testing typically reveals 2–3x more bugs than expected. Having all 4 engineers available for both testing and fixing is critical. If too many P0 bugs, extend testing into Day 11 and compress monitoring setup.

---

## DAY 11: P1 Bug Fixes + Monitoring & Alerting Setup

**Objective:** Fix remaining P1 bugs and establish production monitoring

**Tasks:**

E1 + E2 — P1 bug fixes:
- Fix all P1 bugs identified on Day 10
- Re-run failed E2E test scenarios to verify fixes
- Fix any edge cases discovered during testing (partial fills, concurrent orders, rapid bond creation)

E3 — Monitoring infrastructure:
- Set up monitoring dashboard (Grafana, Datadog, or simple custom dashboard):
  - RPC latency (p50, p95, p99) per service
  - Indexer slot lag (latest slot — last indexed slot)
  - API request latency per endpoint per chain
  - Order book: orders created/cancelled per hour
  - Oracle: response latency, error rate
  - Error rate per service
- Configure alerting rules:
  - RPC latency > 5s for 5 min → Warning
  - RPC error rate > 10% for 5 min → Critical
  - Indexer slot lag > 100 → Warning
  - Indexer slot lag > 1000 → Critical
  - Oracle /health returns non-200 for 2 min → Critical
  - Any program pause detected → Critical
  - Order book health check fails → Critical
- Set up alert routing (Slack, PagerDuty, or email)
- Verify alerts fire correctly by triggering test conditions

E4 — Frontend polish + performance:
- Fix any remaining P1 UI bugs
- Add loading states for all Solana operations (transaction submission → confirmation)
- Add error toasts with human-readable messages (translate Solana program errors to user-friendly text)
- Optimize bundle size: tree-shake unused Solana dependencies
- Test on mobile browsers (responsive design verification)
- Verify Solana wallet popup behavior on Chrome, Firefox, Safari

**Deliverable:** All P1 bugs fixed. Monitoring dashboard live with alerting. Frontend polished and performant.

**Dependency:** Day 10 — E2E tests complete, bug list finalized

**Risk:** Monitoring setup can be time-consuming if the team hasn't used the chosen tool before. Simplify: start with application-level health endpoints + a single dashboard page, add granular metrics post-launch.

---

## DAY 12: Staging Deployment + Pre-Production Validation

**Objective:** Deploy the entire stack to staging environment and validate against mainnet-like conditions

**Tasks:**

E1 — Staging deployment (backend):
- Deploy all APIs (user, general, lpbonds, rewards-service) to staging with devnet/mainnet config
- Deploy Solana order-book to staging
- Deploy solana-price-oracle to staging with mainnet RPC (if mainnet programs deployed) or devnet
- Configure all services to point to staging database
- Verify all health endpoints return 200

E2 — Staging deployment (indexers):
- Deploy NFT indexer to staging, pointed at devnet (or mainnet if deployed)
- Deploy volume indexer to staging
- Deploy metadata indexer (refactored) to staging
- Verify webhook delivery to staging endpoints
- Verify indexer data appearing in staging database

E3 — Staging deployment (infrastructure):
- Configure staging RPC endpoints (separate from production)
- Set up staging monitoring (lightweight — health checks only)
- Run database migration scripts against staging database
- Verify all environment variables are correct for staging

E4 — Staging deployment (frontend):
- Deploy lp-bonds-webapp to staging URL with Solana support
- Deploy launchpad-webapp (or launchpad routes) to staging
- Configure frontend to use staging API endpoints
- Verify wallet connection works on staging URL

All — Pre-production validation:
- Execute abbreviated E2E test suite (Tests 1, 2, 3, 7 from Day 10) on staging
- Verify monitoring captures staging activity
- Verify alerts would fire for simulated failures
- Run load test: 50 concurrent API requests across all endpoints
- Document staging results and any remaining issues

**Deliverable:** Full stack deployed to staging. Pre-production validation complete. Go/no-go decision for production.

**Dependency:** Day 11 — bugs fixed, monitoring ready

**Risk:** Staging environment may behave differently from production (different RPC latency, different database performance). Use this as a signal — if staging has issues, production will too. Fix before proceeding.

---

## DAY 13: Production Deployment + Smoke Tests

**Objective:** Deploy all backend services, indexers, and frontend to production

**Tasks:**

Morning — Backend production deployment:
- Deploy User API with Solana support to production
- Deploy General API with Solana support to production
- Deploy LP Bonds API (refactored) to production
- Deploy Rewards Service (updated) to production
- Deploy Solana Order Book to production
- Deploy solana-price-oracle to production with mainnet environment variables
- Verify all health endpoints return 200 in production
- Verify production database has Solana tables (run migrations if not already done)

Midday — Indexer production deployment:
- Deploy NFT indexer to production, pointed at mainnet Solana programs
- Deploy Volume indexer to production, pointed at Marketplace program
- Deploy Metadata indexer to production (with Solana support)
- Register production Helius webhooks for mainnet program IDs
- Verify indexers receiving and processing events (create a test transaction if mainnet programs allow)

Afternoon — Frontend production deployment:
- Deploy lp-bonds-webapp to production with Solana wallet support enabled
- Deploy launchpad-webapp to production
- Configure production API endpoints in frontend config
- Verify wallet connection works on production URL
- Verify chain selector shows Solana as an option

Evening — Production smoke tests:
- Connect wallet to production frontend → select Solana → verify connection
- Verify API returns Solana data (may be empty if mainnet just launched)
- Verify oracle returns signed responses for mainnet positions
- Verify monitoring dashboard shows production metrics
- Verify alert routing works (test alert)

**Deliverable:** Entire full stack deployed to production. All services healthy. Smoke tests passing.

**Dependency:** Day 12 — staging validation passed. Day 5 of Mainnet Deployment Roadmap — Solana programs live on mainnet.

**Risk:** Production deployment may reveal configuration issues (wrong RPC URLs, wrong program IDs, database connection issues). This is why staging validation on Day 12 is critical. Have rollback plan: revert to pre-Solana deployment if critical issues found.

---

## DAY 14: Final Validation + Go-Live Confirmation + Documentation

**Objective:** Final end-to-end validation on production, document everything, confirm go-live

**Tasks:**

Morning — Production E2E validation:
- Execute full user journey on production: connect Phantom → select Solana → create LP bond (if liquidity available) → view portfolio → verify indexer data → verify API data
- If mainnet deployment completed (Day 5 of deployment roadmap): execute a real trade (list + buy)
- Verify rewards service cron fires and processes Solana bonds
- Verify metadata indexer fetches and processes Solana NFT metadata
- Verify volume data appears in stats

Midday — Documentation:
- Write operations runbook: how to monitor, how to respond to alerts, common failure scenarios and fixes
- Document all production program IDs, mint addresses, API endpoints, webhook URLs
- Document environment variables for each service (without secrets)
- Write user-facing documentation: how to connect Solana wallet, how to create bonds on Solana, supported wallets
- Create internal architecture diagram showing all services and their interactions

Afternoon — Handoff:
- Share runbook with on-call team
- Verify on-call engineer can access monitoring dashboard and alert channels
- Confirm incident response escalation path
- Final check: all services healthy, all monitoring active, all alerts configured

Evening — Go-Live:
- Announce Solana support is live (product/marketing)
- Monitor initial user activity closely (all engineers on standby)
- Respond to any immediate user-reported issues

**Deliverable:** Production validated. Documentation complete. Go-live confirmed. Team on standby for monitoring.

**Dependency:** Day 13 — production deployment complete

**Risk:** Users may find edge cases not covered in testing. Having all engineers on standby for the first 24–48 hours after go-live is essential. Priority: fix data-loss bugs immediately, defer cosmetic issues.

---

## DEPENDENCY GRAPH (14-Day Overview)

```
DAY 1:  Foundation ──────────────────────────────────────────────────┐
  │                                                                   │
DAY 2:  User API + General API + NFT Indexer scaffold + Frontend wallet │
  │                                                                   │
DAY 3:  LP Bonds API refactor + Order Book start + NFT Indexer build  │
  │          + Frontend tx builders                                   │
  │                                                                   │
DAY 4:  LP Bonds API finish + Order Book CRUD + Rewards Service       │
  │          + NFT Indexer finish + Frontend oracle                   │
  │                                                                   │
DAY 5:  Order Book done + Volume Indexer + Metadata Indexer           │
  │          + Frontend lock handlers                                │
  │                                                                   │
DAY 6:  Frontend portfolio + trading + API verification              │
  │                                                                   │
DAY 7:  Frontend evolution + redemption + chain selector             │
  │                                                                   │
DAY 8:  Hardening (all services) + Launchpad frontend start          │
  │                                                                   │
DAY 9:  Integration testing + Launchpad frontend finish              │
  │                                                                   │
DAY 10: E2E testing + Bug fixes ─────────────────────────────────────┤
  │                                                                   │
DAY 11: P1 bugs + Monitoring ─────────────────────────────────────────┤
  │                                                                   │
DAY 12: Staging deployment + Pre-production validation               │
  │                                                                   │
DAY 13: Production deployment + Smoke tests                          │
  │                                                                   │
DAY 14: Final validation + Go-live ──────────────────────────────────┘
```

---

## ENGINEER ALLOCATION HEATMAP

| Day | E1 (Backend Senior) | E2 (Backend) | E3 (Indexer) | E4 (Frontend) |
|-----|---------------------|-------------|-------------|---------------|
| 1 | blockchain-utils pkg | DB migrations | Helius + indexer infra | Frontend deps + wallet provider |
| 2 | User API Solana | General API Solana | NFT indexer scaffold + decoders | SolanaWalletContext + ChainSelector |
| 3 | LP Bonds ILockerService | Order Book create/cancel | NFT indexer processors | Solana tx builders (LP Bonds) |
| 4 | LP Bonds completion + Rewards | Order Book queries + expiry | NFT indexer finish + backfill | Position info fetcher + balances |
| 5 | Order Book integration tests | Volume Indexer | Metadata Indexer refactor | Lock handlers + CreateLPBonds UI |
| 6 | API endpoint verification | Order Book → General API | Indexer tuning + DLQ | Portfolio + trading UI |
| 7 | Backend bug fixes | Order Book matching | Metadata enrichment | Evolution + redemption + polish |
| 8 | Backend hardening | Order Book hardening | Indexer hardening | Launchpad frontend (create + config) |
| 9 | Integration test suite | Integration test suite | Indexer integration tests | Launchpad frontend (mint + refund) |
| 10 | E2E testing + bug fixes | E2E testing + bug fixes | E2E testing + bug fixes | E2E testing + bug fixes |
| 11 | P1 bug fixes | P1 bug fixes | Monitoring + alerting | Frontend polish + performance |
| 12 | Staging deploy (backend) | Staging deploy (order book) | Staging deploy (indexers) | Staging deploy (frontend) |
| 13 | Prod deploy (APIs) | Prod deploy (order book) | Prod deploy (indexers) | Prod deploy (frontend) |
| 14 | Final validation | Documentation | Documentation | Go-live + monitoring |

---

## CRITICAL GO/NO-GO GATES

| Gate | Day | Criteria | Abort Action |
|------|-----|----------|--------------|
| **G1** | End Day 4 | LP Bonds API returns correct Solana data for devnet bonds | Cannot proceed with frontend integration — fix API first |
| **G2** | End Day 5 | Order Book create/query/cancel works with Ed25519 signatures | Marketplace trading blocked — prioritize fix |
| **G3** | End Day 7 | Frontend can create, view, trade, evolve, redeem bonds on devnet | Cannot move to hardening — fix frontend first |
| **G4** | End Day 10 | All 8 E2E test scenarios pass | Cannot deploy to staging — extend bug fix period |
| **G5** | End Day 12 | Staging validation passes | Cannot deploy to production — fix staging issues |
| **G6** | End Day 13 | Production health checks all green | Do NOT announce go-live — debug production issues |

---

## SCOPE REDUCTION (IF BEHIND SCHEDULE)

If the team falls behind, cut in this order (lowest impact first):

| Priority | Cut | Impact | Save |
|----------|-----|--------|------|
| 1 | Launchpad frontend polish (dashboard, analytics) | Low — core mint/refund still works | 1 day |
| 2 | Metadata indexer Solana support | Medium — NFTs show without images/metadata temporarily | 1 day |
| 3 | Volume indexer | Medium — marketplace works but no volume stats | 1 day |
| 4 | Evolution UI | Medium — bonds can still be evolved via CLI/SDK | 0.5 day |
| 5 | Launchpad frontend entirely | High — launch without Solana launchpad, add later | 2 days |

**Minimum viable launch (saves 5.5 days):** LP bonds creation + trading + redemption. No launchpad, no volume stats, no metadata enrichment, no evolution UI. These are added in a fast-follow sprint.
