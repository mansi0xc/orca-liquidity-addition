# Risks and Mitigation Strategies

## 1. Critical Risk Matrix

| # | Risk | Probability | Impact | Severity | Category |
|---|------|-------------|--------|----------|----------|
| R1 | Unaudited programs deployed to mainnet | Medium | Critical | **CRITICAL** | Security |
| R2 | Token-2022 Transfer Hook complexity underestimated | High | High | **HIGH** | Technical |
| R3 | Marketplace program exploit (fund loss) | Medium | Critical | **CRITICAL** | Security |
| R4 | Orca Whirlpool CPI integration failures on mainnet | Medium | High | **HIGH** | Integration |
| R5 | Indexer data inconsistency between EVM and Solana | High | Medium | **HIGH** | Data |
| R6 | Oracle signing key compromise | Low | Critical | **HIGH** | Security |
| R7 | RPC rate limiting causes service degradation | High | Medium | **MEDIUM** | Infrastructure |
| R8 | Frontend dual-wallet UX confusion | Medium | Medium | **MEDIUM** | UX |
| R9 | Solana network congestion affects transactions | Medium | Medium | **MEDIUM** | External |
| R10 | Team lacks Solana-specific expertise | Medium | High | **HIGH** | Organizational |
| R11 | Timeline pressure leads to skipped testing | High | High | **HIGH** | Process |
| R12 | LPBondsExchange tokenization gap on Solana | Medium | Medium | **MEDIUM** | Feature |
| R13 | Ed25519 signature scheme implementation errors | Medium | High | **HIGH** | Technical |
| R14 | Database migration breaks existing EVM data | Low | Critical | **HIGH** | Data |
| R15 | Metaplex program version incompatibility | Medium | Medium | **MEDIUM** | Integration |

---

## 2. Detailed Risk Analysis and Mitigations

### R1: Unaudited Programs on Mainnet

**Description:** Pressure to meet the 4-day deployment timeline may lead to deploying programs without a formal security audit. The marketplace program handles asset transfers and fund settlement — any vulnerability allows direct fund theft.

**Impact:** Complete loss of user funds. Protocol reputation destruction. Potential legal liability.

**Mitigation Strategies:**

| Priority | Action | Owner |
|----------|--------|-------|
| P0 | **Do not deploy unaudited programs to mainnet.** This is non-negotiable for any program that handles user funds | Engineering Lead |
| P1 | Engage audit firm early (during development, not after) | Project Manager |
| P1 | Run Soteria/Sec3 automated analysis as pre-audit screen | Solana Engineers |
| P2 | Implement program-level pause functionality (already present in LP bonds) | Solana Engineers |
| P2 | Start with admin-restricted operations (whitelist users for mainnet beta) | Product |
| P3 | Bug bounty program on Immunefi before public launch | Security Team |

**Decision Gate:** Mainnet deployment of marketplace and launchpad programs MUST be blocked until audit is complete.

---

### R2: Token-2022 Transfer Hook Complexity

**Description:** The GMI CV token requires trade restriction logic (allowed exchanges whitelist) implemented as a Token-2022 Transfer Hook program. Transfer hooks are relatively new in the Solana ecosystem, have limited production examples, and interact with every transfer of the token.

**Impact:** Transfer hooks that fail cause ALL transfers to revert. A buggy hook can freeze the entire GMI CV token economy.

**Technical Details:**
- Transfer hooks execute as CPI on every `transfer` and `transferChecked` instruction
- The hook must be stateless or use PDAs for state (allowed exchange list)
- Hook failures are opaque to the caller — debugging is difficult
- Token-2022 hooks interact with the mint authority in ways that can conflict with other extensions

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | Build a comprehensive test suite for the transfer hook (happy path, edge cases, failure modes) |
| P0 | Test with EVERY token operation: transfer, transferChecked, mintTo, burn, approve, close |
| P1 | Implement an emergency bypass mechanism (admin can disable the hook via PDA flag) |
| P1 | Study existing production transfer hooks (e.g., Sanctum, Jito) for patterns |
| P2 | Consider deferring GMI CV to a later phase — launch GMI and LP Token first without transfer hooks |
| P2 | If complexity is too high, consider using a custom token program instead of transfer hook extension |

**Fallback:** If transfer hook proves unstable, implement trade restriction at the marketplace program level instead (marketplace checks allowed addresses before executing trades involving GMI CV).

---

### R3: Marketplace Program Exploit

**Description:** The marketplace program settles trades by transferring assets between parties. An exploit in order matching, signature verification, or asset settlement logic could allow:
- Unauthorized asset transfers
- Replay attacks (reusing old orders)
- Partial fill manipulation
- Fee calculation overflow/underflow

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | External security audit (mandatory before mainnet) |
| P0 | Implement comprehensive input validation (zero amounts, duplicate accounts, self-trades) |
| P1 | Use Anchor's account constraint system rigorously (`has_one`, `constraint`, `seeds`) |
| P1 | Nonce-based replay protection for all signed operations |
| P1 | Separate settlement into atomic steps (verify → lock → transfer → record) with rollback |
| P2 | Implement transaction size limits (max assets per match) |
| P2 | Rate limiting at the program level (daily volume caps during soft launch) |
| P3 | Formal verification of critical paths (order matching, fee calculation) |

---

### R4: Orca Whirlpool CPI Integration Failures

**Description:** LP bonds programs interact with Orca Whirlpool via CPI for `open_position`, `increase_liquidity`, `decrease_liquidity`, `collect_fees`, and `close_position`. Mainnet Whirlpool program behavior may differ from devnet in:
- Account validation strictness
- Tick array initialization requirements
- Minimum liquidity thresholds
- Fee tier availability

**Impact:** LP bond creation fails on mainnet despite working on devnet.

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | Test on mainnet-fork or with mainnet-cloned accounts (using `solana-test-validator --clone`) |
| P1 | Ensure tick arrays are initialized for all required tick ranges before launch |
| P1 | Verify Whirlpool fee tiers available for SOL-GMI pair |
| P1 | Test with actual mainnet Whirlpool program (clone in test validator) |
| P2 | Pre-create positions manually to verify CPI flow end-to-end |
| P2 | Implement graceful error handling — if CPI fails, return clear error (not generic "program error") |

**Note:** The existing `solana-lp-bonds-contracts` tests clone the Whirlpool program for local testing. This same approach must be verified against the mainnet Whirlpool version.

---

### R5: Indexer Data Inconsistency

**Description:** Running parallel indexers for EVM and Solana creates data consistency risks:
- Same NFT collection tracked on both chains with different state
- Volume double-counting if cross-chain activity isn't deduplicated
- Stale data on one chain while the other is live
- Race conditions in shared DB writes

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P1 | Chain prefix on ALL table names (enforced by convention and code review) |
| P1 | API layer aggregates per-chain — never mixes chain data in single query without explicit intent |
| P1 | Indexer health endpoints report slot/block lag per chain |
| P2 | Data reconciliation cron job comparing indexer state vs on-chain state |
| P2 | Separate database schemas (not just table prefixes) if volume warrants it |
| P3 | Cross-chain deduplication for collections/tokens that exist on both chains |

---

### R6: Oracle Signing Key Compromise

**Description:** The solana-price-oracle signs position data with an Ed25519 key. This signature is verified on-chain in `verify_collateral`. If the oracle key is compromised, an attacker can forge position data (fake liquidity amounts) to:
- Mint bonds with inflated collateral values
- Bypass position validation
- Manipulate reward calculations

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | Oracle private key stored in secure secret management (AWS Secrets Manager, not env file) |
| P0 | Oracle deployed with restricted network access (no public SSH, minimal attack surface) |
| P1 | Key rotation mechanism: `update_oracle_authority` instruction allows admin to rotate key |
| P1 | Monitor oracle usage — alert on unexpected signature patterns or volumes |
| P2 | Implement oracle key in HSM or cloud KMS (sign requests without exposing key material) |
| P2 | Multiple oracle signers with threshold (require 2-of-3 signatures) — future enhancement |
| P3 | On-chain verification of position data against actual Whirlpool state (oracle becomes advisory) |

---

### R7: RPC Rate Limiting

**Description:** Solana RPC providers (Helius, Triton, etc.) impose rate limits. The protocol has multiple consumers (indexers, APIs, oracle, frontend) all hitting the same RPC endpoint. During high activity:
- Indexers fall behind (slot lag increases)
- API requests timeout
- Frontend transactions fail to confirm

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P1 | Separate RPC endpoints per service tier (indexers get dedicated endpoint, APIs get another) |
| P1 | Request caching for hot reads (account info, slot, block time) with 1–5s TTL |
| P1 | Use `getMultipleAccountsInfo` instead of individual `getAccountInfo` calls |
| P2 | WebSocket for real-time data instead of polling |
| P2 | Fallback RPC provider configuration (primary → fallback rotation) |
| P2 | Monitor RPC credit usage via provider dashboard |
| P3 | Dedicated RPC node if volume exceeds hosted provider limits |

**Cost estimate:** Helius Business plan (~$499/month) provides 500M credits/month. Estimate protocol usage at ~50-100M credits/month initially.

---

### R8: Frontend Dual-Wallet UX

**Description:** Users must manage both EVM wallets (MetaMask, Coinbase) and Solana wallets (Phantom, Solflare) within the same application. This creates confusion around:
- Which wallet to connect for which chain
- Simultaneous connection states
- Transaction signing with the wrong wallet
- Cross-chain asset display

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P1 | Chain selector is the PRIMARY navigation element — wallet context follows chain selection |
| P1 | Only show the relevant wallet connector for the selected chain |
| P1 | Clear visual indicators of active chain (color coding, chain logo, network badge) |
| P2 | Unified portfolio view that labels assets by chain |
| P2 | Auto-detect installed wallets and suggest chain based on availability |
| P3 | Wallet abstraction layer (future: unified wallet experience via account abstraction) |

**UX Pattern:**
```
1. User selects "Solana" from chain selector
2. App shows ONLY Solana wallet connectors (Phantom, Solflare)
3. All contract interactions use Anchor client
4. EVM wallet context is hidden (not disconnected — just not shown)
5. User switches to "Ethereum"
6. App shows ONLY EVM wallet connectors
7. Both connections persist in background
```

---

### R9: Solana Network Congestion

**Description:** Solana has experienced periods of network congestion, high priority fee requirements, and occasional instability. During congestion:
- Transactions may be dropped
- Confirmation times increase from ~400ms to seconds or minutes
- Priority fees spike, increasing cost

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P1 | Implement priority fee estimation (Helius `getPriorityFeeEstimate` API) |
| P1 | Transaction retry logic with exponential backoff and fee bumping |
| P1 | Set appropriate compute unit limits (not default max) to reduce rejection |
| P2 | Frontend shows estimated fees and confirmation time |
| P2 | Implement "fast" and "economy" transaction modes |
| P3 | Jito bundle submission for critical transactions (guaranteed inclusion) |

---

### R10: Team Solana Expertise Gap

**Description:** The existing team is experienced with EVM/Solidity. Solana/Anchor development has fundamentally different paradigms (account model, PDA derivation, CPI, compute budget, rent). Ramp-up time is often underestimated.

**Impact:** Slower development, subtle bugs from EVM-pattern thinking (e.g., assuming storage works like Solidity mappings), architectural mistakes.

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | Allocate 1–2 weeks for Solana developer onboarding before counting productive development time |
| P1 | Hire or contract at least 1 experienced Solana/Anchor developer for the program engineering |
| P1 | Code review by Solana-experienced engineer for all program code |
| P2 | Internal knowledge sharing sessions on Solana architecture |
| P2 | Use Anchor (not raw Solana programs) to reduce footgun surface area |
| P3 | Engage Solana ecosystem developer relations for architecture review |

**Common EVM → Solana Pitfalls:**
- Expecting `msg.sender` (Solana has explicit signer accounts)
- Expecting storage maps (Solana uses PDAs, one account per entry)
- Ignoring account size limits (10KB per account)
- Ignoring compute budget (200K CU default, 1.4M max)
- Not handling rent (accounts below minimum balance are garbage collected)
- Using u128 math without checking overflow (Solana BPF has no native u128)

---

### R11: Timeline Pressure → Skipped Testing

**Description:** The aggressive timelines (4 days deployment, 10 days integration) create enormous pressure to cut corners. The most common cuts: unit tests, integration tests, load tests, security reviews.

**Impact:** Bugs reach production. Data corruption. Fund loss in worst case.

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | Define minimum test coverage gates that block deployment |
| P0 | Mandatory: all program instructions tested on devnet before mainnet |
| P0 | Mandatory: smoke test every API endpoint with Solana data before launch |
| P1 | Automate integration tests in CI (GitHub Actions + Solana test validator) |
| P1 | Testnet dry-run is NOT optional — it prevents day-of surprises |
| P2 | Push back on timeline if testing reveals issues — delay is cheaper than an exploit |

**Minimum Test Gates:**

| Gate | Requirement |
|------|------------|
| Program unit tests | 100% instruction coverage |
| Program integration tests | All cross-program flows (bond create → evolve → redeem) |
| API integration tests | All modified endpoints return correct data for Solana inputs |
| Frontend smoke tests | Wallet connect, create bond, view portfolio on Solana |
| Order book | Create → query → cancel → match flow |
| Indexer | Process 1000+ transactions without data loss |

---

### R12: LPBondsExchange Tokenization Gap

**Description:** The EVM `LPBondsExchange` contract allows burning LP bond NFTs in exchange for minting an ERC20 token. This functionality does not appear in the `solana-lp-bonds-contracts` programs. If this feature is needed on Solana, it requires:
- A new instruction or program
- Token-2022 minting authority for the exchange token
- Burn authority for LP bond NFTs
- Multisig approval flow

**Impact:** Users on Solana cannot tokenize their LP bonds. Feature gap between EVM and Solana.

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P1 | Confirm with product whether tokenization is required at Solana launch |
| P2 | If required: add `exchange_bond` instruction to LP Bonds program |
| P2 | If deferred: document as known limitation, plan for future sprint |
| P3 | Consider if tokenization can use a simple authority-signed backend flow instead of on-chain program |

---

### R13: Ed25519 Signature Scheme Errors

**Description:** The protocol uses Ed25519 signatures extensively (oracle signing, order-book verification). The canonical message format must be identical between signer (oracle/off-chain) and verifier (on-chain program). Any mismatch (byte order, padding, field ordering) causes signature verification to fail silently.

**Impact:** Oracle verification fails → bond creation blocked. Order verification fails → marketplace non-functional.

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | Integration test that signs off-chain and verifies on-chain for every message type |
| P1 | Canonical message format documented with byte-level specification |
| P1 | Use the same `buildCanonicalMessage` function (ported) in oracle and program tests |
| P2 | Add debug logging for signature verification failures (log expected vs actual message hash) |
| P2 | Test with multiple Ed25519 libraries (tweetnacl, @solana/web3.js, dalek) to ensure interop |

**Existing Reference:** The `solana-price-oracle` already has a working `buildCanonicalMessage()` (198 bytes) that matches the on-chain `verify_collateral` implementation. New programs should follow this pattern exactly.

---

### R14: Database Migration Breaks EVM Data

**Description:** Adding Solana tables and modifying shared schemas could break existing EVM data if migrations are not carefully managed. Specific risks:
- ALTER TABLE on shared tables (e.g., widening VARCHAR for Solana addresses) causes lock contention
- Sequelize sync could overwrite existing columns
- Index changes could degrade query performance for EVM endpoints

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P0 | ALL Solana data goes in NEW tables (chain-prefixed) — never modify existing EVM tables |
| P0 | Test all migrations on a production DB clone first |
| P1 | Use explicit Sequelize migrations (not sync) for schema changes |
| P1 | Run migrations during low-traffic window |
| P2 | Database backup before any migration |
| P2 | Rollback scripts prepared for every migration |

---

### R15: Metaplex Program Version Incompatibility

**Description:** Metaplex has multiple program versions (Token Metadata v1.1, v1.3, v1.13+, Bubblegum for compressed NFTs). The launchpad and NFT indexer must target the correct version. Version mismatches cause CPI failures or incorrect data deserialization.

**Mitigation Strategies:**

| Priority | Action |
|----------|--------|
| P1 | Pin Metaplex program IDs to specific versions in Anchor.toml |
| P1 | Test against mainnet Metaplex programs (clone in local validator) |
| P2 | Use `@metaplex-foundation/mpl-token-metadata` TypeScript SDK (maintained, version-aware) |
| P2 | Monitor Metaplex upgrade announcements — they occasionally update on mainnet |

---

## 3. Risk-Adjusted Timeline

Adding buffer for risk materialization:

| Phase | Base Estimate | Risk Buffer | Risk-Adjusted |
|-------|-------------|-------------|---------------|
| Program development | 5 weeks | +2 weeks (R2, R10, R4) | 7 weeks |
| Security audit | 4 weeks | +1 week (audit findings) | 5 weeks |
| Mainnet deployment | 4 days | +2 days (R4, R7) | 6 days |
| Full-stack integration | 3 weeks | +1 week (R5, R7, R8, R11) | 4 weeks |
| **Total** | **~13 weeks** | **+4 weeks** | **~17 weeks** |

---

## 4. Contingency Plans

### Contingency A: Marketplace Audit Delayed

**Trigger:** Audit firm cannot start within the planned window.

**Action:** Deploy LP Bonds (already audited) and Token programs first. Marketplace launches in a later phase. Users can create and manage bonds but cannot trade them on the native marketplace — use external Solana NFT marketplaces (Tensor, Magic Eden) as interim.

### Contingency B: Transfer Hook Unstable

**Trigger:** GMI CV transfer hook causes transaction failures in testing.

**Action:** Launch GMI CV as a standard Token-2022 mint without transfer hook. Implement trade restriction logic at the application layer (marketplace program rejects unauthorized trades). Plan transfer hook for a future upgrade.

### Contingency C: Solana Network Issues During Deployment

**Trigger:** Solana mainnet experiences congestion or instability during the deployment window.

**Action:** Deployment window is extended. All deployment scripts are idempotent (can be re-run safely). If partially deployed, pause all programs until deployment completes. Have a rollback plan: close partially-initialized accounts and redeploy.

### Contingency D: Indexer Falls Behind

**Trigger:** Solana indexer cannot keep up with transaction volume.

**Action:** Switch from webhook-based to poll-based indexing with longer intervals. Implement batch processing. If persistent, add dedicated Geyser plugin stream (higher throughput, lower latency). Interim: use Helius DAS API for on-demand data instead of indexer.

---

## 5. Security Checklist (Pre-Mainnet)

### Program Security

- [ ] All programs audited by Solana-experienced firm
- [ ] All audit findings addressed (critical/high fixed, medium acknowledged)
- [ ] Anchor account constraints used for ALL accounts
- [ ] Signer checks on all authority operations
- [ ] PDA bump validation (canonical bumps only)
- [ ] Integer overflow checks on all arithmetic
- [ ] Reentrancy protection (where applicable)
- [ ] Compute budget estimated and tested for all instructions
- [ ] Account size limits verified (no account exceeds 10KB without realloc)
- [ ] Rent exemption calculated for all created accounts
- [ ] Close account instructions clean up properly

### Infrastructure Security

- [ ] Program upgrade authority = multisig (not EOA)
- [ ] Oracle signing key in secrets manager (not env file)
- [ ] RPC API keys rotated and scoped
- [ ] No private keys in source code or git history
- [ ] Backend services run with minimal permissions
- [ ] Rate limiting on all public API endpoints
- [ ] CORS configured correctly on all services
- [ ] Helius webhook endpoints authenticated (secret header)

### Operational Security

- [ ] Emergency pause procedures documented and tested
- [ ] On-call rotation established for post-launch
- [ ] Incident response playbook written
- [ ] Communication plan for security incidents
- [ ] Monitoring dashboards accessible to on-call
- [ ] Alert escalation paths configured

---

## 6. Key Takeaways

1. **The single highest risk is deploying unaudited programs.** No timeline pressure justifies this.

2. **Token-2022 Transfer Hook is the highest technical risk.** Plan a fallback (no hook) and don't let it block the rest of the migration.

3. **The team's EVM expertise doesn't transfer directly to Solana.** Budget ramp-up time and bring in Solana-experienced engineers.

4. **Indexing on Solana is fundamentally different.** Don't try to port EVM event-listener patterns. Embrace account-based indexing with Helius.

5. **The 4-day and 10-day timelines are feasible ONLY as execution windows** within a larger project. They are not feasible as total project timelines.

6. **LP Bonds is the safest starting point** — programs are already built and tested. Launch LP Bonds first, marketplace second, launchpad third.
