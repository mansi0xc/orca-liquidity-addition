# Mainnet Deployment Roadmap

## Constraints

- **Hard deadline:** First week of April 2026
- **Deployment window:** 5 days maximum
- **Prerequisite:** All Solana programs (token authority, transfer hook, marketplace, launchpad) are developed, tested on devnet, and audited
- **Prerequisite:** `solana-lp-bonds-contracts` and `solana-price-oracle` are already migrated and tested
- **Team:** 2–3 engineers with Solana deployment experience

## Assumptions

1. All Anchor programs compile and pass devnet integration tests
2. Squads multisig vault is already created and signers configured
3. Helius RPC account is provisioned with mainnet API key
4. Orca Whirlpool pools for SOL-GMI (and level pools) are either pre-created or will be created on Day 3
5. Deployment keypairs are generated and stored securely (hardware wallet or cold storage)
6. Deployment scripts are pre-written and rehearsed on devnet
7. The oracle private key (Ed25519) is generated and stored in secrets management

---

## PRE-DEPLOYMENT CHECKLIST (Must Pass Before Day 1)

- [ ] All 6 programs build with `anchor build` (verifiable builds)
- [ ] All programs deployed and tested on devnet
- [ ] External security audit completed, all critical/high findings remediated
- [ ] Squads multisig vault address confirmed
- [ ] Mainnet RPC endpoint tested (Helius)
- [ ] Deployment scripts tested on devnet with identical flow
- [ ] SOL funded to deployer wallet (estimate: 50–100 SOL for deploys + rent)
- [ ] Token metadata JSON files uploaded to permanent storage (Arweave/IPFS)
- [ ] All program IDs pre-generated (keypairs created, not yet deployed)
- [ ] Team on-call schedule confirmed for deployment window
- [ ] Rollback procedure documented

---

## DAY-BY-DAY EXECUTION ROADMAP

## DAY 1: Program Deployment & Authority Transfer

**Objective:** Deploy all 6 Solana programs to mainnet and transfer upgrade authorities to multisig

**Tasks:**

- Deploy Token Authority Program using verified build (`anchor deploy --provider.cluster mainnet`)
- Deploy GMI CV Transfer Hook Program (Token-2022 transfer hook)
- Deploy Marketplace Program
- Deploy Launchpad Program
- Verify LP Bonds Program (`7oFXPveRzDZUHSTxaRquLDn8Z7U3CZYLonyET13Sqaxe`) is live and correct on mainnet
- Verify LP Bonds Evolution Program (`H7ymeEN673X7kKSYXUUoeByZVspALeHbLHzQQFmNVy43`) is live and correct on mainnet
- Run `solana program show <PROGRAM_ID>` for all 6 programs — confirm executable, correct size, deployer authority
- Transfer upgrade authority for each newly deployed program to Squads multisig vault using `solana program set-upgrade-authority`
- Verify all 6 program authorities show multisig vault address (not deployer EOA)
- Secure deployer keypair (move to cold storage — no further use after authority transfer)

**Deliverable:** All 6 programs deployed to mainnet with upgrade authority held by Squads multisig. Deployer keypair secured.

**Dependency:** Pre-deployment checklist passed

**Risk:** Program binary too large for single deploy transaction — mitigate by using `solana program deploy --buffer` with pre-uploaded buffer account. Rehearse on devnet the day before.

---

## DAY 2: Token-2022 Mints & Protocol Initialization

**Objective:** Create all Token-2022 mints and initialize core protocol state

**Tasks:**

Morning — Token Mints:
- Create GMI Token-2022 mint with MetadataPointer + TokenMetadata extensions (9 decimals, mint authority = Token Authority PDA)
- Create GMI CV Token-2022 mint with MetadataPointer + TokenMetadata + TransferHook extensions (hook program = deployed Transfer Hook Program, 9 decimals)
- Create LP Token-2022 mint with MetadataPointer + TokenMetadata extensions (9 decimals, no supply cap)
- Initialize on-chain metadata for all 3 mints (name, symbol, URI pointing to Arweave/IPFS JSON)
- Verify all mints: `spl-token display <MINT>` — confirm extensions, authority, decimals

Afternoon — Token Authority Initialization:
- Call `initialize_token_mint` on Token Authority Program for GMI (set max supply)
- Call `initialize_token_mint` for GMI CV (link transfer hook config)
- Call `initialize_token_mint` for LP Token (no supply cap)
- Add initial minters via `add_minter` for each token (LP Bonds program PDA, treasury wallet, etc.)
- Initialize Trade Config PDA for GMI CV — set `trade_allowed = false` initially
- Verify minter PDAs exist: `["minter", mint_pubkey, minter_pubkey]` for each minter/token pair

Evening — Protocol Config Initialization:
- Call `initialize` on Marketplace Program — set protocol fee BPS (250 = 2.5%), fee receiver (treasury), admin
- Call `initialize` on Launchpad Program — set admin, treasury, default refund config
- Verify both configs: fetch PDA accounts and deserialize, confirm all fields correct

**Deliverable:** 3 Token-2022 mints created with correct extensions. Token authority initialized with minter roles. Marketplace and Launchpad protocols initialized.

**Dependency:** Day 1 — all programs deployed

**Risk:** Token-2022 extension ordering matters — MetadataPointer must be initialized before TokenMetadata. Follow exact sequence from devnet rehearsal. Transfer Hook extension requires the hook program to be deployed first (done Day 1).

---

## DAY 3: Oracle, Whirlpool & LP Bonds Configuration

**Objective:** Configure oracle, Orca Whirlpool integrations, and update LP Bonds protocol for mainnet

**Tasks:**

Morning — Oracle Configuration:
- Initialize Oracle Config PDA on LP Bonds Program via `initialize_oracle`
- Set oracle authority to the Ed25519 public key of the deployed solana-price-oracle service via `update_oracle_authority`
- Deploy solana-price-oracle to production (Vercel) with mainnet environment variables: `RPC_URL` (Helius mainnet), `ORACLE_PRIVATE_KEY`, `LP_BONDS_PROGRAM_ID`
- Update `CHAIN_CONFIG` in solana-price-oracle with mainnet Whirlpool pool addresses for SOL-GMI and all level pools
- Test oracle endpoint: `POST /position-info` — verify it returns signed position data for a known mainnet position or test position
- Verify oracle health endpoint returns 200

Afternoon — Whirlpool Setup:
- Create Orca Whirlpool pool for SOL-GMI if not already existing (via Orca SDK `createPool` with appropriate tick spacing and initial price)
- Create Whirlpool pools for each evolution level (L1, L2, L3) if not already existing
- Initialize required tick arrays for expected price ranges
- Verify each Whirlpool: fetch account, confirm tokenMintA/B match protocol tokens, confirm liquidity and tick state
- Update LP Bonds Protocol Config via `update_config` — set `allowlisted_whirlpool` to mainnet SOL-GMI Whirlpool, set `token_mint_a` and `token_mint_b` to new Token-2022 mints (or WSOL + GMI), set `lock_duration`

Evening — Evolution Level Configuration:
- Call `initialize_evolution` on LP Bonds Evolution Program
- Call `configure_level` for levels 1, 2, 3 — set each level's Whirlpool address, tick parameters, and required input
- Call `initialize_layer_authority` for each level
- Call `create_layer_token_mint` if evolution requires level-specific token mints
- Verify all level configs by fetching and deserializing PDAs

**Deliverable:** Oracle live on mainnet, Whirlpool pools created and verified, LP Bonds protocol configured with mainnet addresses, all evolution levels configured.

**Dependency:** Day 2 — token mints created (LP Bonds config references them)

**Risk:** Orca Whirlpool tick array initialization is often missed — positions fail to open if tick arrays covering the target range aren't initialized. Pre-initialize tick arrays for a wide price range. Oracle signing key mismatch between production deploy and on-chain config — test with a real `verify_collateral` call.

---

## DAY 4: Smoke Testing & Security Validation

**Objective:** Execute end-to-end smoke tests with real funds and validate all security controls

**Tasks:**

Morning — Functional Smoke Tests (use small real amounts):
- **Token flow:** Mint 100 GMI via authorized minter → transfer between wallets → verify balance
- **GMI CV restriction:** Attempt transfer to non-allowed address → confirm Transfer Hook rejects → add address to allowed list → retry → confirm success
- **LP Bond creation:** Call `add_liquidity_and_mint_bond` with minimal SOL + GMI amounts → verify bond NFT minted, PositionCustody PDA created, Whirlpool position opened
- **Oracle verification:** Call `verify_collateral` for the newly created bond → verify oracle signature accepted on-chain
- **Bond evolution:** Evolve a level-1 bond to level-2 → verify source bond burned, new bond minted, new position opened
- **Bond redemption:** Wait for lock expiry (or use test config with 0 duration) → call `redeem_bond` → verify bond burned, position returned
- **Marketplace order:** Create a signed order → submit to Solana order-book → query order → cancel order → verify state
- **Launchpad flow:** Create collection → configure phases → mint 1 NFT → verify metadata correct

Afternoon — Security Validation:
- **Authority sweep:** Run script to verify ALL program upgrade authorities = Squads multisig
- **PDA authority check:** Verify every admin/authority PDA points to correct wallet (not deployer)
- **Pause testing:** Pause each program via multisig → attempt operations → confirm all revert with "paused" error → unpause → confirm operations succeed
- **Fee receiver check:** Verify marketplace fee receiver, launchpad treasury are correct addresses
- **Minter list audit:** Verify only authorized addresses are in minter PDAs for all token mints
- **Oracle authority check:** Verify on-chain oracle authority matches deployed oracle's public key
- **Nonce verification:** Create a nonce PDA → verify it increments correctly → attempt replay → confirm rejection

Evening — Infrastructure Validation:
- Verify Helius RPC latency from all deployed services (oracle, order-book, indexers)
- Verify WebSocket connections are stable (10-minute connection test)
- Verify Helius API rate limits are sufficient (simulate burst of 50 requests/second)
- Test fallback RPC endpoint (disconnect primary, confirm fallback works)
- Verify all program IDs are documented in a single canonical reference

**Deliverable:** All protocol functions verified on mainnet with real transactions. Security controls validated. Infrastructure confirmed stable.

**Dependency:** Day 3 — all configs and oracle deployed

**Risk:** Smoke test failures. The entire day is allocated to testing so there's time to debug. If a critical test fails, Day 5 becomes fix + retest, and go-live shifts by 1 day. Have the Solana program engineers on standby for emergency patches (deployed via multisig upgrade).

---

## DAY 5: Go-Live, Backend Deployment & Monitoring Activation

**Objective:** Activate the protocol for public use and deploy all supporting backend services

**Tasks:**

Morning — Final Configuration:
- Enable GMI CV trading: call `update_trade_allowance(true)` via multisig
- Add Marketplace Program as allowed exchange for GMI CV: call `add_allowed_exchange` via multisig
- Verify all programs are in unpaused state
- Publish official program IDs, token mint addresses, and PDA addresses to documentation

Midday — Backend Service Deployment:
- Deploy solana-price-oracle to production if not already done on Day 3 (verify again)
- Deploy Solana order-book service with mainnet config (marketplace program ID, Ed25519 verification keys, Solana RPC)
- Update rewards-service environment: add Solana oracle URL to routing config, verify `bondrewards` table supports Solana bond format
- Update lpbonds API: add Solana RPC URL, deploy SolanaLockerService, verify PDA-based position custody reads work
- Update user API: add `solana` chain support, deploy SNS resolution
- Update general API: add Solana routing in order submission proxy, add Jupiter Price API integration
- Start Solana NFT indexer (pointed at mainnet programs)
- Start Solana volume indexer (pointed at marketplace program)

Afternoon — Monitoring Activation:
- Enable Helius webhook for marketplace program transactions → volume indexer
- Enable Helius webhook for LP Bonds program transactions → NFT indexer
- Configure alerting: RPC latency > 5s (warning), RPC errors > 10% (critical), indexer slot lag > 100 (warning), oracle offline (critical), program pause detected (critical)
- Verify monitoring dashboard shows live data from indexers
- Deploy updated frontend (lp-bonds-webapp with Solana wallet support) — or confirm deployment from integration track

Evening — Go-Live Verification:
- Execute one full end-to-end flow through the live frontend: connect Phantom → select Solana → create LP bond → verify in portfolio → verify in indexer data → verify in API response
- Confirm all monitoring alerts are firing correctly (trigger test alert)
- Document incident response contacts and escalation path
- Send go-live confirmation to stakeholders

**Deliverable:** Protocol is LIVE on Solana mainnet. All backend services deployed. Monitoring active. Frontend serving Solana users.

**Dependency:** Day 4 — all smoke tests passed

**Risk:** Backend service startup failures. Mitigate by deploying to staging environment the day before and verifying. If frontend is not ready from the integration track, launch in "API-only" mode (backend functional, frontend follows within days). Indexer initial sync may take hours — this is expected and does not block go-live.

---

## POST-DEPLOYMENT (Days 6–7)

**Day 6 — Active Monitoring:**
- Monitor all transactions for error rates
- Watch indexer slot lag convergence
- Verify oracle response accuracy for live positions
- Check volume indexer data against on-chain transactions
- Respond to any user-reported issues

**Day 7 — Stabilization:**
- Address any issues found on Day 6
- Verify reward calculations running correctly for Solana bonds
- Performance tune RPC usage based on observed patterns
- Document lessons learned
- Confirm all systems stable — hand off to standard on-call

---

## DEPLOYMENT DEPENDENCY GRAPH

```
DAY 1: Deploy Programs
  │
  ├──► DAY 2: Token Mints + Protocol Init
  │       │
  │       ├──► DAY 3: Oracle + Whirlpool + LP Bonds Config
  │       │       │
  │       │       └──► DAY 4: Smoke Tests + Security Validation
  │       │               │
  │       │               └──► DAY 5: Go-Live + Backend Deploy + Monitoring
  │       │
  │       └──► DAY 3 (parallel): Marketplace + Launchpad init (done Day 2 evening)
  │
  └──► DAY 3 (parallel): Oracle deployment (independent of token mints)
```

---

## CRITICAL GO/NO-GO GATES

| Gate | Checked At | Criteria | Abort Action |
|------|-----------|----------|--------------|
| **G1: Programs Deploy** | End of Day 1 | All 6 programs deployed + authorities transferred | Cannot proceed. Debug deployment. Extend window. |
| **G2: Mints Created** | End of Day 2 | All 3 Token-2022 mints created with correct extensions | Cannot proceed. Debug extension init. |
| **G3: Oracle Live** | Midday Day 3 | Oracle returns valid signed response for mainnet position | Cannot go live without oracle. Debug and redeploy. |
| **G4: Smoke Tests Pass** | End of Day 4 | All 8 smoke test scenarios pass | Do NOT go live. Day 5 becomes debug day. Go-live shifts. |
| **G5: Security Validated** | End of Day 4 | All authority/PDA/pause checks pass | Do NOT go live until all checks pass. |

If G4 or G5 fails, the go-live on Day 5 is postponed. Use Day 5 as a fix day and go-live shifts to Day 6 (within the buffer of post-deployment days).

---

## MAINNET DEPLOYMENT QUICK REFERENCE

| Item | Value |
|------|-------|
| **LP Bonds Program** | `7oFXPveRzDZUHSTxaRquLDn8Z7U3CZYLonyET13Sqaxe` |
| **LP Bonds Evolution** | `H7ymeEN673X7kKSYXUUoeByZVspALeHbLHzQQFmNVy43` |
| **Token Authority** | `<TBD — generated pre-deployment>` |
| **Transfer Hook** | `<TBD — generated pre-deployment>` |
| **Marketplace** | `<TBD — generated pre-deployment>` |
| **Launchpad** | `<TBD — generated pre-deployment>` |
| **GMI Mint** | `<TBD — created Day 2>` |
| **GMI CV Mint** | `<TBD — created Day 2>` |
| **LP Token Mint** | `<TBD — created Day 2>` |
| **Multisig Vault** | `<Squads vault address>` |
| **Oracle Public Key** | `<Ed25519 pubkey from solana-price-oracle>` |
| **SOL-GMI Whirlpool** | `<TBD — created/identified Day 3>` |
| **Helius RPC** | `https://mainnet.helius-rpc.com/?api-key=<KEY>` |
