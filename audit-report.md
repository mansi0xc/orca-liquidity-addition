# EVM-to-Solana Migration Audit Report

**Date:** 2026-03-24
**Auditor:** AI Security Audit Agent
**Status:** Migration ~60% complete (higher than initially estimated)

---

## Section 1: EVM Repo Architecture Map

### 1.1 File Tree

```
evm-contracts/liquidity-bonds-contracts/contracts/
|-- LiquidityBonds.sol                          # ERC721 bond NFT contract (Level 1)
|-- LiquidityBondLockerV3.sol                   # V3 locker: lock Uniswap positions, mint bonds (main chain)
|-- LiquidityBondsEvolution.sol                 # Evolution locker: layer-based bond upgrades (main chain)
|-- abstract/
|   |-- ABSLiquidityBondLockerV3.sol            # Abstract base for Algebra DEX variant lockers
|   |-- ABSLiquidityBondsEvolution.sol          # Abstract base for Algebra DEX variant evolution
|   |-- interface/
|       |-- IABSNonFungiblePositionManager.sol   # Algebra DEX position manager interface
|       |-- IABSV3Pool.sol                       # Algebra DEX pool interface
|-- apechain/
|   |-- ApeLiquidityBondLockerV3.sol            # ApeChain-specific locker (inherits ABS base)
|   |-- ApeLiquidityBonds.sol                   # ApeChain-specific bond NFT
|   |-- ApeLiquidityBondsLockerEvolution.sol    # ApeChain-specific evolution locker
|   |-- interface/
|       |-- IAlgebraNonfungiblePositionManager.sol
|       |-- IAlgebraV3Pool.sol
|       |-- IApeLiquidityBondLocker.sol
|-- interface/
|   |-- IERC20MintBurn.sol                      # Mintable/Burnable ERC20 interface
|   |-- IERC20Weird.sol                         # Non-standard ERC20 interface
|   |-- ILiquidityBondLocker.sol                # Locker interface
|   |-- ILiquidityBonds.sol                     # Bond NFT interface
|   |-- INonfungiblePositionManager.sol         # Uniswap V3 NFT position manager interface
|   |-- IOperatorRegistry.sol                   # Operator whitelist registry interface
|   |-- IUniswapV3Pool.sol                      # Uniswap V3 pool interface
|   |-- IWETH.sol                               # WETH interface
|-- proxy/
|   |-- LPBondsExchangeProxy.sol                # Transparent proxy for exchange
|   |-- LPBondsExchangeProxyAdmin.sol           # Proxy admin for exchange
|   |-- LiquidityBondLockerProxy.sol            # Transparent proxy for locker
|   |-- LiquidityBondLockerProxyAdmin.sol       # Proxy admin for locker
|   |-- LiquidityBondsProxy.sol                 # Transparent proxy for bonds
|   |-- LiquidityBondsProxyAdmin.sol            # Proxy admin for bonds
|-- tokenization/
|   |-- LPBondsExchange.sol                     # Exchange bonds for minted tokens
|-- test/
    |-- MockERC20.sol, MockERC20MintBurn.sol, MockERC721.sol,
    |-- MockLiquidityBonds.sol, MockOperatorRegistry.sol,
    |-- MockUniswapV3Positions.sol, MockWETH.sol,
    |-- NFTReceiver.sol, TestBalanceOf.sol
```

### 1.2 Contract Inheritance & Call Graph

```
CORE CONTRACTS:

LiquidityBonds (ERC721Upgradeable, OwnableUpgradeable, PausableUpgradeable, ReentrancyGuardUpgradeable)
  |-- mint() -> calls ILiquidityBondLocker.locks() for validation
  |-- burn() -> marks bond as redeemed
  |-- getBondInfo() -> reads from ILiquidityBondLocker, INonFungiblePositionManager
  |-- tokenURI() -> generates on-chain SVG + JSON metadata

LiquidityBondLockerV3 (OwnableUpgradeable, PausableUpgradeable, ReentrancyGuardUpgradeable, IERC721ReceiverUpgradeable)
  |-- lockPositionChild() -> verifies signature, transfers tokens, calls uniswapPositionManager.mint(),
  |                          transfers NFT to multiSig, calls lpbond.mint()
  |-- setBond() -> admin sets bond configuration
  |-- setBasePosition() -> admin sets base position for signature verification

LiquidityBondsEvolution (same inheritance as LockerV3)
  |-- lockPositionChild() -> burns base layer NFTs, verifies signature,
  |                          mints token1 (layer token), calls uniswapPositionManager.mint(),
  |                          transfers to multiSig, calls lpbond.mint()
  |-- setLayer() -> admin configures layer bonds
  |-- setBond() -> admin sets bond configuration

LPBondsExchange (OwnableUpgradeable, PausableUpgradeable, ReentrancyGuardUpgradeable)
  |-- exchange() -> transfers bond NFTs to multiSig, mints ERC20 tokens to user
```

### 1.3 Data Flow Diagram

**Lock Flow (EVM):**
1. User calls `lockPositionChild()` with amounts, signature, numberOfBonds
2. Contract verifies ECDSA signature (off-chain signer) over (basePosition, amount0, amount1, contract, nonce, sender)
3. Token0 transferred from user; Token1 either transferred from user (ETH wrapped) or minted (evolution)
4. Uniswap V3 position minted via `uniswapPositionManager.mint()`
5. Position NFT transferred to `multiSig` (admin controlled)
6. LP Bond NFT minted to user via `lpbond.mint()`
7. Lock record created mapping position ID to lock info

**Evolution Flow (EVM):**
1. User provides base layer NFTs (burned to `multiSigBurned`)
2. Token0 transferred from user, Token1 minted (layer token)
3. Protocol fee deducted from token0
4. New Uniswap V3 position minted
5. New LP Bond NFT minted to user

### 1.4 Key Functions & Access Controls

| Function | Contract | Access Control | State Modified |
|----------|----------|---------------|----------------|
| `initialize()` | LiquidityBonds | `initializer` | All state variables |
| `mint()` | LiquidityBonds | `onlyMinterOrOwner`, `whenNotPaused`, `nonReentrant` | `currentIndex`, `bonds[]` |
| `burn()` | LiquidityBonds | `onlyMinterOrOwner`, `whenNotPaused`, `nonReentrant` | `bonds[].isRedemeed` |
| `addMinter()` / `removeMinter()` | LiquidityBonds | `onlyOwner` | `minters[]` |
| `updateLiquidityBondLocker()` | LiquidityBonds | `onlyOwner` | `liquidityBondLocker` |
| `_transfer()` / `_approve()` | LiquidityBonds | `validateTransfer` / `validateApprove` | ERC721 state |
| `initialize()` | LiquidityBondLockerV3 | `initializer` | All config |
| `lockPositionChild()` | LiquidityBondLockerV3 | `nonReentrant`, `whenNotPaused`, `bondExists`, `basePositionExists` | `nonce`, `locks[]` |
| `setBond()` | LiquidityBondLockerV3 | `onlyOwner` | `bonds[]` |
| `setBasePosition()` | LiquidityBondLockerV3 | `onlyOwner` | `basePositions[]` |
| `setSigner()` | LiquidityBondLockerV3 | `onlyOwner` | `signer` |
| `setMultiSig()` | LiquidityBondLockerV3 | `onlyOwner` | `multiSig` |
| `recoverETH/ERC20/ERC721()` | LiquidityBondLockerV3 | `onlyOwner` | External transfers |
| `lockPositionChild()` | LiquidityBondsEvolution | `nonReentrant`, `whenNotPaused`, `bondExists`, `basePositionExists` | `nonce`, `locks[]` |
| `setLayer()` | LiquidityBondsEvolution | `onlyOwner` | `layers[][]` |
| `exchange()` | LPBondsExchange | `nonReentrant`, `whenNotPaused`, `basePositionExists` | `nonce` |

### 1.5 External Dependencies

- **OpenZeppelin Upgradeable:** OwnableUpgradeable, PausableUpgradeable, ReentrancyGuardUpgradeable, ERC721Upgradeable
- **OpenZeppelin Standard:** IERC20, IERC721, ECDSA, MessageHashUtils, Strings, Base64
- **Uniswap V3:** INonFungiblePositionManager (position management), IUniswapV3Pool (pool queries)
- **Algebra DEX (ApeChain):** IABSNonFungiblePositionManager, IABSV3Pool
- **Custom:** IOperatorRegistry (transfer whitelist), IERC20MintBurn (mintable tokens), IWETH (ETH wrapping)

---

## Section 2: Feature Parity Gaps

### 2.1 Core Contract Mapping

| EVM Contract | Solana Equivalent | Status |
|-------------|-------------------|--------|
| LiquidityBondLockerV3 | lp-bonds program | ⚠️ Partial |
| LiquidityBondsEvolution | lp-bonds-evolution program | ⚠️ Partial |
| LiquidityBonds (ERC721) | Bond NFT mint (SPL Token) | ⚠️ Partial |
| LPBondsExchange | None | ❌ Missing |
| ABSLiquidityBondLockerV3 | None | ❌ Missing (Algebra DEX not on Solana) |
| ABSLiquidityBondsEvolution | None | ❌ Missing (Algebra DEX not on Solana) |
| ApeChain contracts | None | ❌ Missing (chain-specific, N/A for Solana) |
| Proxy contracts | None (Solana programs are upgradeable natively) | N/A |

### 2.2 Function-Level Parity

| EVM Function | EVM Location | Solana Status | Notes |
|-------------|-------------|---------------|-------|
| **LiquidityBondLockerV3** | | | |
| `initialize(weth, uniswapPM, signer)` | LiquidityBondLockerV3.sol:147 | ⚠️ Partial | Solana uses `initialize()` with different params (whirlpool, mints, ticks, lock_duration). No WETH equivalent needed. |
| `lockPositionChild(bondId, amount0, amount1, sig, isEth, numBonds)` | LiquidityBondLockerV3.sol:173 | ⚠️ Partial | Solana `add_liquidity_and_mint_bond()` handles single bond per tx (no loop). No ETH wrapping (handled differently via wrapped SOL). No multiple bonds per call. |
| `_verifySignature()` | LiquidityBondLockerV3.sol:275 | ✅ Implemented | Solana uses Ed25519 precompile + oracle model instead of ECDSA. More secure (message binding includes more fields). |
| `getRewards0()` | LiquidityBondLockerV3.sol:299 | ❌ Missing | Returns 0 in EVM too (rewards service). Not needed. |
| `setBond()` | LiquidityBondLockerV3.sol:310 | ⚠️ Partial | Solana uses `ProtocolConfig` + `LevelConfig` instead of per-bond mapping. Different architecture. |
| `setUniswapPositionManager()` | LiquidityBondLockerV3.sol:371 | ❌ Missing | Solana hardcodes Whirlpool program ID. By design. |
| `setWeth()` | LiquidityBondLockerV3.sol:380 | N/A | No WETH on Solana; native SOL wrapping handled inline. |
| `setSigner()` | LiquidityBondLockerV3.sol:389 | ✅ Implemented | `update_oracle_authority()` serves same purpose. |
| `setBasePosition()` | LiquidityBondLockerV3.sol:398 | ❌ Missing | Different architecture: Solana uses per-user nonce, not per-bond base positions. |
| `pause()` / `unpause()` | LiquidityBondLockerV3.sol:404-410 | ✅ Implemented | `pause()` / `unpause()` instructions. |
| `setMultiSig()` | LiquidityBondLockerV3.sol:412 | ❌ Missing | Solana uses PDA custody, no external multisig for position storage. Different security model (arguably better). |
| `recoverETH()` | LiquidityBondLockerV3.sol:420 | ❌ Missing | No emergency recovery for SOL. |
| `recoverERC20()` | LiquidityBondLockerV3.sol:438 | ❌ Missing | No emergency token recovery. |
| `recoverERC721()` | LiquidityBondLockerV3.sol:430 | ❌ Missing | No emergency NFT recovery. |
| `setStartTime()` | LiquidityBondLockerV3.sol:452 | ❌ Missing | No admin start time override. |
| `setWeirdERC20()` | LiquidityBondLockerV3.sol:468 | N/A | Not needed on Solana (SPL Token is standardized). |
| **LiquidityBonds (ERC721)** | | | |
| `initialize(name, symbol, locker, registry, type)` | LiquidityBonds.sol:134 | ⚠️ Partial | Bond NFTs are SPL Token mints with 0 decimals. No ERC721 metadata/SVG. |
| `mint(to, positionId)` | LiquidityBonds.sol:163 | ✅ Implemented | `add_liquidity_and_mint_bond()` mints bond NFT via SPL Token. |
| `burn(bondId)` | LiquidityBonds.sol:184 | ✅ Implemented | `redeem_bond()` burns bond NFT. |
| `addMinter()` / `removeMinter()` | LiquidityBonds.sol:217-238 | ❌ Missing | Solana uses PDA-based minting authority, no external minter registry. |
| `getBondInfo()` | LiquidityBonds.sol:364 | ⚠️ Partial | Bond info stored in `PositionCustody` PDA. No aggregated view function. |
| `tokenURI()` (on-chain SVG) | LiquidityBonds.sol:479 | ❌ Missing | No on-chain metadata generation. URI-based metadata via `BOND_NFT_URI_BASE`. |
| `validateTransfer` modifier | LiquidityBonds.sol:90 | ❌ Missing | No operator registry / transfer whitelist on Solana. |
| `validateApprove` modifier | LiquidityBonds.sol:112 | ❌ Missing | No approval whitelist on Solana. |
| **LiquidityBondsEvolution** | | | |
| `initialize(uniswapPM, signer)` | LiquidityBondsEvolution.sol:164 | ✅ Implemented | `initialize_evolution()`. |
| `lockPositionChild(bondId, layerId, baseTokenIds, ...)` | LiquidityBondsEvolution.sol:187 | ✅ Implemented | `evolve_bond()`. Burns source, mints layer token, creates new position, mints new bond. |
| `setBond()` | LiquidityBondsEvolution.sol:317 | ✅ Implemented | `configure_level()`. |
| `setLayer()` | LiquidityBondsEvolution.sol:378 | ⚠️ Partial | Layer concept exists via `LevelConfig` but architecture differs. |
| `setMultiSig()` | LiquidityBondsEvolution.sol:432 | ⚠️ Partial | Treasury concept exists for fees, but position custody is PDA-based. |
| `setMultiSigBurned()` | LiquidityBondsEvolution.sol:436 | ❌ Missing | Source bonds are burned (supply reduced) rather than transferred to burn address. Better on Solana. |
| **LPBondsExchange** | | | |
| `initialize(multiSig, signer)` | LPBondsExchange.sol:62 | ❌ Missing | Entire exchange contract not migrated. |
| `exchange(collection, tokenIds, ...)` | LPBondsExchange.sol:71 | ❌ Missing | No bond-to-token exchange functionality. |
| `setBondConfig()` | LPBondsExchange.sol:96 | ❌ Missing | |
| **Admin Features** | | | |
| Owner transfer (Ownable) | Multiple contracts | ✅ Implemented | Two-step admin transfer (propose + accept). Better than EVM single-step. |
| Emergency recovery (ETH/ERC20/ERC721) | LiquidityBondLockerV3.sol:420-445 | ❌ Missing | No emergency recovery mechanisms. |
| `receive()` fallback | Multiple contracts | N/A | Solana programs do not have fallback handlers. |

### 2.3 Solana-Only Features (Not in EVM)

| Feature | Location | Description |
|---------|----------|-------------|
| Oracle staleness check | lp-bonds/src/lib.rs:253-256 | 60-second max oracle age. EVM has no staleness check. |
| Ed25519 precompile verification | lp-bonds/src/ed25519.rs | More secure than ECDSA: domain separation, program ID binding, message reconstruction. |
| Per-user nonce | lp-bonds/src/state.rs:142 | Per-user sequential nonce vs EVM global nonce. Better replay protection. |
| Tick array PDA validation | lp-bonds/src/lib.rs:419-474 | Validates tick array PDAs. EVM trusts Uniswap internally. |
| Position post-CPI validation | lp-bonds/src/lib.rs:543-555 | Validates position belongs to correct whirlpool after CPI. |
| Authority whitelist with permissions | lp-bonds-evolution/src/state.rs:198-224 | Bitmask-based permission system. Not in EVM. |
| Two-step admin transfer | lp-bonds/src/lib.rs:147-174 | Propose + accept pattern prevents accidental admin loss. |
| Domain-separated oracle messages | lp-bonds/src/constants.rs:20-24 | MINT vs VERIFY domains prevent cross-instruction replay. |

---

## Section 3: Security Vulnerabilities

### [CRITICAL] -- Exploitable with direct fund loss

**No critical vulnerabilities found.** The Solana implementation demonstrates strong security practices overall.

---

### [HIGH] -- Exploitable with significant impact

#### H-01: Missing Emergency Recovery Mechanisms

**Location:** Both `lp-bonds/src/lib.rs` and `lp-bonds-evolution/src/lib.rs` (entire programs)

**Description:** The EVM contracts include `recoverETH()`, `recoverERC20()`, and `recoverERC721()` emergency functions (e.g., LiquidityBondLockerV3.sol:420-445). The Solana programs have no equivalent. If tokens are accidentally sent to PDA-controlled accounts or the program encounters an edge case where user funds are trapped in custody, there is no admin mechanism to recover them.

**Impact:** Permanent fund loss in edge cases (e.g., failed CPI leaves tokens in program accounts, or a bug prevents normal redemption).

**Recommended Fix:** Add admin-only instructions:
- `admin_recover_sol()` -- Transfer SOL from PDA accounts
- `admin_recover_token()` -- Transfer SPL tokens from PDA accounts
- `admin_force_close_custody()` -- Emergency close of position custody with proper safeguards

**Effort:** M

---

#### H-02: No Multiple Bonds Per Transaction Support

**Location:** `lp-bonds/src/lib.rs:207` (`add_liquidity_and_mint_bond`)

**Description:** The EVM `lockPositionChild()` supports minting multiple bonds in a single transaction via the `_numberOfBonds` parameter with a for-loop (LiquidityBondLockerV3.sol:233-263). The Solana implementation mints exactly one bond per instruction invocation. While this is a design choice that avoids compute budget issues, it changes the user experience and may cause issues with off-chain systems that expect batch operations.

**Impact:** Functional gap -- users who relied on batch minting must submit multiple transactions, increasing costs and complexity. However, this is mitigated by the fact that Solana transactions can bundle multiple instructions.

**Recommended Fix:** Document this as a known design difference. If batch support is needed, consider adding a `batch_mint` instruction that processes up to N bonds per call (limited by compute budget, likely 2-3 max).

**Effort:** M

---

#### H-03: Position Custody Not Closed on Redemption

**Location:** `lp-bonds/src/lib.rs:683-737` (`redeem_bond`)

**Description:** When a bond is redeemed, the `position_custody` account is NOT closed. The position NFT is transferred to the user and the bond NFT is burned, but the `PositionCustody` PDA remains allocated with its data intact. This means:
1. Rent for the custody account is never reclaimed
2. The account remains on-chain permanently, increasing state bloat
3. In the evolution program, `validate_source_custody` checks `is_evolved == false` to prevent double evolution, but does not check if the bond was already redeemed (burned). A burned bond mint has supply 0, but the custody data is still readable.

**Impact:** Moderate -- rent is not recovered (approximately 0.002 SOL per bond), and stale custody records persist indefinitely. The double-redemption is prevented by the bond NFT burn (supply becomes 0, so the user cannot present the bond again), but the lack of cleanup is wasteful.

**Recommended Fix:** Add `close = user` to the `position_custody` account in `RedeemBond`, or use a separate cleanup instruction. Also add a `redeemed` flag to `PositionCustody` for defense in depth.

**Effort:** S

---

### [MEDIUM] -- Potential issues under specific conditions

#### M-01: Tick Current Mismatch Can Cause Frequent Transaction Failures

**Location:** `lp-bonds/src/lib.rs:333-336`, `lp-bonds-evolution/src/lib.rs:870-873`

**Description:** The oracle-provided `tick_current` is validated against the on-chain whirlpool `tick_current_index` at execution time. In volatile markets, the pool's tick can change between oracle signing and transaction execution. With only 60 seconds of oracle staleness allowed, transactions may frequently fail in volatile conditions because `tick_current` no longer matches by the time the transaction lands.

**Impact:** Poor UX -- users may need multiple attempts to successfully mint bonds during volatile periods. This is a fundamental tension between security (tick binding) and usability.

**Recommended Fix:** Consider allowing a small tick tolerance range (e.g., +/- 5 ticks from oracle-signed value), or make the staleness window configurable by admin. The trade-off is that a wider tolerance could allow slightly stale price data.

**Effort:** S

---

#### M-02: Evolution Source Custody Accepts `is_evolved == false` from Level 0/255

**Location:** `lp-bonds-evolution/src/lib.rs:829-833`

**Description:** In `validate_source_custody`, if `custody_ref.level == 0 || custody_ref.level == 255`, the source level defaults to 1. This is a fallback for older custody records that may not have the level field properly set. However, level 0 or 255 could indicate corrupted data, and treating it as level 1 could allow evolution from invalid states.

**Impact:** Low-medium -- if a corrupted custody record exists (e.g., from a failed initialization), it could be used as a valid Level 1 source for evolution.

**Recommended Fix:** Add explicit validation that `custody_ref.level >= 1 && custody_ref.level <= 4` and reject level 0 or 255 entirely.

**Effort:** S

---

#### M-03: Evolution Program's `source_custody` is UncheckedAccount

**Location:** `lp-bonds-evolution/src/lib.rs:1328-1329`

**Description:** The `source_custody` in `EvolveBond` is declared as `UncheckedAccount`. While `validate_source_custody()` performs comprehensive validation (owner check against lp_bonds_program_id, PDA derivation, deserialization, bond_mint check), the account is `#[account(mut)]` despite the handler only reading from it. Making it mutable when only reading is unnecessarily permissive.

**Impact:** Low -- the validation in `validate_source_custody` is thorough, but the `mut` annotation on a read-only account is a code smell that could lead to future bugs if someone adds write logic.

**Recommended Fix:** Remove the `mut` annotation from `source_custody` since the handler only reads from it.

**Effort:** S

---

#### M-04: No Lock Duration Validation on Config Update

**Location:** `lp-bonds/src/lib.rs:93-124` (`update_config`)

**Description:** The `update_config` instruction allows the admin to set `lock_duration` to any `i64` value, including 0 or negative values. The EVM contract requires `_lockDuration > 0` (LiquidityBondLockerV3.sol:329). While the Solana `initialize` also lacks this check, a zero or negative lock duration means bonds could be immediately redeemable, undermining the locking mechanism.

**Impact:** Medium -- admin misconfiguration could create immediately redeemable bonds. The admin is trusted, but defense-in-depth requires input validation.

**Recommended Fix:** Add `require!(lock_duration > 0, LpBondsError::InvalidLockDuration)` to both `initialize` and `update_config`.

**Effort:** S

---

#### M-05: Approve Delegation to Whirlpool Program in Evolution

**Location:** `lp-bonds-evolution/src/lib.rs:540-564`

**Description:** The evolution program approves the `whirlpool_program` as a delegate on `program_token_a_account` and `program_token_b_account`. The code comments acknowledge this is "redundant but harmless." However, setting approval to an external program's account key (rather than the program itself) could theoretically be exploited if the Orca Whirlpool program or its key is compromised. The approvals should be revoked after the CPI to follow the principle of least privilege.

**Impact:** Low -- the approved amount equals the exact deposit amount, so exposure is limited. But the pattern is not best practice.

**Recommended Fix:** Revoke approvals (set to 0) after the `increase_liquidity` CPI completes. Or remove the approvals entirely since `layer_token_authority` owns the accounts and signs the CPI.

**Effort:** S

---

#### M-06: Whirlpool Deserialization Assumes Fixed Layout

**Location:** `lp-bonds/src/whirlpool_cpi.rs:20-58`, `lp-bonds-evolution/src/whirlpool_cpi.rs:20-50`

**Description:** The `Whirlpool` struct is manually defined to match the Orca Whirlpool program's account layout. If Orca updates their Whirlpool program and changes the account layout, the deserialization could produce incorrect values or fail silently by reading wrong offsets. There is no discriminator validation (only owner check).

**Impact:** Medium -- if Orca migrates to a new version with different layout, the program could read incorrect token mints, vaults, or tick data, potentially allowing operations on wrong pools.

**Recommended Fix:** Add the Orca Whirlpool Anchor discriminator check (first 8 bytes) to `from_account_info`. Pin the Orca program version in documentation and add an admin mechanism to pause if Orca upgrades.

**Effort:** S

---

### [LOW] -- Best practice violations, minor issues

#### L-01: Bond NFT Has No On-Chain Metadata

**Location:** `lp-bonds/src/lib.rs:603-621`

**Description:** The EVM `LiquidityBonds.sol:479-537` generates rich on-chain SVG metadata via `tokenURI()`. The Solana bond NFTs are plain SPL Token mints with no Metaplex metadata. The `BOND_NFT_URI_BASE` constant suggests an off-chain metadata service, but no Metaplex `create_metadata_accounts_v3` CPI is performed.

**Impact:** Low -- bonds will appear as generic tokens in wallets, reducing user experience. No security impact.

**Recommended Fix:** Add Metaplex Token Metadata CPI to create metadata (name, symbol, URI) during `add_liquidity_and_mint_bond` and `evolve_bond`.

**Effort:** M

---

#### L-02: No Event for Lock Duration Changes

**Location:** `lp-bonds/src/lib.rs:93-124` (`update_config`)

**Description:** The `ConfigUpdated` event includes `lock_duration`, which is good. However, there is no separate event for individual field changes, making it harder to track which specific parameter changed. This is a minor observability gap.

**Impact:** Informational -- makes off-chain monitoring slightly harder.

**Effort:** S

---

#### L-03: Redundant Signer Check

**Location:** `lp-bonds/src/lib.rs:282-285`, `lp-bonds-evolution/src/lib.rs:382-385`

**Description:** Both programs explicitly check `ctx.accounts.user.is_signer` in the handler body, but `user` is already declared as `Signer<'info>` in the account struct. Anchor automatically enforces the signer check for `Signer<'info>` accounts. The manual check is redundant.

**Impact:** None -- just unnecessary code. Does not affect security.

**Effort:** S

---

#### L-04: No Batch Redeem Support

**Location:** `lp-bonds/src/lib.rs:683-737` (`redeem_bond`)

**Description:** Users can only redeem one bond per instruction. EVM `burn()` is also single, but combined with lower gas costs, this is less of an issue there. On Solana, users with many expired bonds need many transactions.

**Impact:** Low -- UX inconvenience. Can be mitigated by bundling multiple redeem instructions in one transaction.

**Effort:** S

---

#### L-05: `init_if_needed` Usage Without Idempotency Guards

**Location:** `lp-bonds/src/lib.rs:1038-1044` (`user_bond_account`), `lp-bonds-evolution/src/lib.rs:1344-1350` (`user_target_bond_account`)

**Description:** `init_if_needed` is used for the user's bond token account. While this is necessary (the ATA may or may not exist), it opens a minor front-running vector where an attacker could create the ATA before the user, causing the user to pay less rent but the attacker to pay the rent. This is a known Solana pattern and generally acceptable.

**Impact:** Informational.

**Effort:** N/A

---

#### L-06: Evolution Record is Permanent

**Location:** `lp-bonds-evolution/src/lib.rs:724-736` (`evolution_record`)

**Description:** The `EvolutionRecord` PDA is created for each evolution but never closed. Seeds are `["evolution_record", source_bond_mint]`, making the record permanent. This creates on-chain state that can never be reclaimed.

**Impact:** Low -- state bloat. Each record costs ~0.003 SOL in rent.

**Recommended Fix:** Consider whether evolution records are needed on-chain or if events are sufficient. If on-chain records are required, add a cleanup instruction for old records.

**Effort:** S

---

## Section 4: Architectural Mismatches

### 4.1 Obvious Mismatches

| EVM Pattern | Solana Equivalent | Current Implementation | Issue |
|------------|-------------------|----------------------|-------|
| `msg.sender` | `Signer<'info>` | Correctly uses `Signer` accounts | None -- properly migrated |
| EVM global storage (`mapping`) | Solana PDA accounts | Uses PDAs with proper seeds | None -- well designed |
| `onlyOwner` modifier | `constraint = admin.key() == config.admin` | Correctly implemented | None |
| `Ownable.transferOwnership()` | Two-step admin transfer | `propose_admin` + `accept_admin` | Improvement over EVM |
| Solidity events | Anchor `emit!` macro | All major events are emitted | None |
| ERC721 NFT | SPL Token mint (supply=1, decimals=0) | Bond NFTs are SPL Token mints | Missing Metaplex metadata (L-01) |
| `ReentrancyGuard` | Solana CPI reentrancy model | No explicit reentrancy guard needed; Solana runtime prevents concurrent writes | Correctly handled |
| Proxy upgrade pattern | Solana native program upgrade | No proxy needed | None |
| `block.timestamp` | `Clock::get()?.unix_timestamp` | Correctly uses Clock sysvar | None |
| `WETH.deposit()` | SOL wrapping via `sync_native` | `maybe_wrap_native_if_needed()` | Correctly handled |
| `receive() payable` | N/A | Not needed on Solana | N/A |
| `ERC20.approve()` + `transferFrom()` | SPL Token transfer with signer | Properly uses signer-based transfers | None |

### 4.2 Subtle Mismatches

| EVM Pattern | Solana Equivalent | Current Implementation | Issue |
|------------|-------------------|----------------------|-------|
| **Global nonce** (single `nonce` variable) | **Per-user nonce** (`NonceAccount` PDA per user) | Solana uses per-user nonces | **Improvement** -- EVM's global nonce is a DoS vector (frontrunner can increment nonce). Solana's per-user nonce isolates users. |
| **Position stored in multiSig** (EVM transfers NFT to admin multisig) | **Position in PDA custody** (Solana uses position_custody PDA) | Custody PDA owns position NFT | **Improvement** -- PDA custody is trustless and programmatic. EVM multiSig requires trusted operators. |
| **ECDSA signature over packed data** | **Ed25519 precompile + domain-separated messages** | Full oracle attestation with domain, program ID, timestamps | **Improvement** -- Solana approach is more secure: domain separation prevents cross-contract replay, program ID binding prevents cross-deployment replay, timestamp staleness prevents stale data. |
| **Multiple bonds per tx** (EVM loops over numberOfBonds) | **One bond per instruction** | Single mint per instruction | **Trade-off** -- Solana compute budget limits make loops impractical. Users can bundle instructions in a single tx if needed. |
| **Token0/Token1 ordering** (EVM uses `token0 < token1` comparison) | **Token A/Token B from config** | Solana reads from ProtocolConfig/LevelConfig | **Different** -- EVM dynamically sorts tokens, Solana requires admin to configure in correct order. Less flexible but less error-prone at runtime. |
| **Fee deduction from token0** (EVM: `(_amount0 * numberOfBonds * layer.fee) / 10000`) | **Fee from token A via `calculate_fee(amount_a)`** | Evolution program deducts fee before LP deposit | **Correct parity** -- both calculate fee as BPS of token A amount. |
| **ERC721 transfer whitelist** (IOperatorRegistry) | **No equivalent** | Not implemented | **Gap** -- EVM restricts transfers to whitelisted operators/EOAs. Solana bond NFTs have no transfer restrictions. If transfer restrictions are a business requirement, this needs implementation (potentially via Metaplex programmable NFTs). |
| **18-decimal precision** (EVM uses uint256 with 18 decimals) | **Variable decimal precision** (SPL Token uses per-mint decimals, typically 6-9) | Solana handles amounts in native token decimals | **Important** -- No explicit decimal conversion is performed. The oracle service must provide amounts in the correct decimal scale for each token. |
| **Rent exemption** | N/A (EVM has no rent) | Accounts are rent-exempt by default (Anchor `init` ensures this) | **None** -- properly handled via Anchor space calculations. |
| **Account reallocation** | N/A | Fixed account sizes via `InitSpace` | **None** -- all accounts use fixed-size structs. No dynamic data. |
| **Tx size limits** | Solana 1232-byte tx limit | `evolve_bond` uses `remaining_accounts` for tick arrays and vaults | **Well handled** -- complex operations use `remaining_accounts` pattern to reduce account list size. May still be tight with many accounts. |
| **Compute budget** | Solana 200K default, 1.4M max | Complex operations (open_position + increase_liquidity + mint) may require increased compute | **Risk** -- `add_liquidity_and_mint_bond` performs 5+ CPIs in sequence. May need `ComputeBudgetProgram.setComputeUnitLimit()` in client. |

---

## Section 5: Prioritized Fix Recommendations

| Priority | File Path | Function/Instruction | Issue | Recommended Fix | Effort |
|----------|-----------|---------------------|-------|----------------|--------|
| P0 | `lp-bonds/src/lib.rs` | `update_config` | Lock duration can be set to 0 or negative (M-04) | Add `require!(lock_duration > 0)` validation | S |
| P0 | `lp-bonds/src/lib.rs` | `redeem_bond` | Position custody not closed, rent not reclaimed (H-03) | Add `close = user` on position_custody or add cleanup instruction | S |
| P1 | Both programs | All | No emergency recovery for trapped funds (H-01) | Add admin-gated recovery instructions for SOL and SPL tokens | M |
| P1 | `lp-bonds-evolution/src/lib.rs:829-833` | `validate_source_custody` | Level 0/255 treated as Level 1 (M-02) | Reject level 0 and 255 explicitly | S |
| P1 | `lp-bonds-evolution/src/lib.rs:1328` | `EvolveBond.source_custody` | Unnecessarily mutable (M-03) | Remove `mut` annotation | S |
| P2 | `lp-bonds/src/whirlpool_cpi.rs:44-58` | `Whirlpool::from_account_info` | No discriminator check (M-06) | Add Anchor discriminator validation | S |
| P2 | `lp-bonds-evolution/src/lib.rs:540-564` | `evolve_bond` (approve step) | Delegate approval not revoked after CPI (M-05) | Revoke approvals after increase_liquidity CPI | S |
| P2 | `lp-bonds/src/lib.rs:207` | `add_liquidity_and_mint_bond` | Single bond per instruction (H-02) | Document as design choice; optionally add batch_mint | M |
| P2 | `lp-bonds/src/lib.rs:333-336` | `add_liquidity_and_mint_bond` | Tick mismatch causes frequent failures in volatile markets (M-01) | Consider tick tolerance or configurable staleness | S |
| P3 | `lp-bonds/src/lib.rs:603-621` | `add_liquidity_and_mint_bond` | No Metaplex metadata for bond NFTs (L-01) | Add `create_metadata_accounts_v3` CPI | M |
| P3 | `lp-bonds-evolution/src/lib.rs:724-736` | `evolve_bond` | Evolution records never closed (L-06) | Add cleanup instruction or use events instead | S |
| P3 | N/A | N/A | LPBondsExchange not migrated | Implement exchange program if needed for Solana deployment | XL |
| P3 | N/A | N/A | ERC721 transfer restrictions not migrated | Implement via Metaplex programmable NFTs if needed | L |

---

## Section 6: Items Requiring Human Review

### 6.1 Business Logic Questions

1. **LPBondsExchange migration needed?** The EVM `LPBondsExchange.sol` allows users to exchange bond NFTs for minted tokens. Is this functionality required for the Solana deployment, or is it deferred to a later phase?

2. **Transfer restrictions on bond NFTs?** The EVM `LiquidityBonds.sol` enforces operator whitelisting for transfers and approvals via `IOperatorRegistry`. Are transfer restrictions a business requirement on Solana? If so, consider Metaplex Programmable NFTs or a wrapper pattern.

3. **Multiple bonds per transaction?** The EVM locker supports minting multiple bonds per call. Should the Solana program support this (via batch instructions or a loop with compute budget increase), or is one-per-instruction acceptable?

4. **Position custody model:** The EVM sends position NFTs to a multisig wallet. The Solana program stores them in PDA-controlled custody. This is architecturally different -- the Solana model is more trustless. Confirm this is the desired behavior.

5. **Fee collection destination:** The EVM evolution contract sends fees to `multiSig`. The Solana evolution program sends fees to a `treasury` address. Are these equivalent in the operational model?

6. **Oracle service compatibility:** The Solana program uses Ed25519 signatures with domain-separated, field-rich messages. The EVM uses ECDSA over `abi.encodePacked`. The oracle service must be updated to support the new message format. Has this been done?

### 6.2 Design Decisions Needing Confirmation

7. **Lock duration = 0 allowed?** The EVM requires `_lockDuration > 0`. The Solana `initialize` and `update_config` do not validate this. If zero-duration bonds are intentionally allowed, document it. Otherwise, add validation.

8. **Admin key management:** The Solana two-step admin transfer is an improvement, but what happens if the pending admin key is lost? There is no timeout or cancellation mechanism for a pending admin proposal.

9. **Emergency pause scope:** The EVM `pause()` is per-contract. The Solana `is_paused` flag on `ProtocolConfig` pauses the entire protocol. Is this the intended granularity?

10. **Whirlpool version pinning:** The programs hardcode the Orca Whirlpool program ID. If Orca deploys a new version, the programs need redeployment or an admin update mechanism. Is this acceptable?

11. **Compute budget for complex operations:** The `add_liquidity_and_mint_bond` and `evolve_bond` instructions perform 4-6 CPIs. Have these been tested against compute limits? Client-side `setComputeUnitLimit()` may be needed.

12. **Token decimal handling:** The EVM uses 18-decimal precision throughout. Solana tokens have varying decimals (SOL=9, USDC=6, custom tokens=variable). Has the oracle service been updated to provide amounts in the correct decimal scale for each token?
