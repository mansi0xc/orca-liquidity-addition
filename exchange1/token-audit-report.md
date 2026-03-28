# LP Bonds Solana Programs -- Security Audit & Feature Parity Report

**Date:** 2026-03-23
**Auditor:** Automated Security Audit (claude-opus-4-6)
**Scope:** `lp-bonds` (Level 1 Locker) and `lp-bonds-evolution` (Level 2-4 Evolution)
**Branch:** `initial-implementation`
**Anchor Version:** 0.32.1

---

## Table of Contents

1. [Repository Architecture Map](#section-1-repository-architecture-map)
2. [L1-L4 Bond Level System](#section-2-l1-l4-bond-level-system)
3. [Feature Parity Gaps (EVM vs Solana)](#section-3-feature-parity-gaps)
4. [Security Vulnerabilities](#section-4-security-vulnerabilities)
5. [Architectural Mismatches (EVM to Solana)](#section-5-architectural-mismatches)
6. [Prioritized Fix Recommendations](#section-6-prioritized-fix-recommendations)
7. [Items Requiring Human Review](#section-7-items-requiring-human-review)

---

## Section 1: Repository Architecture Map

### File Tree

```
solana-lp-bonds-contracts/
|-- Anchor.toml                          # Anchor config: devnet programs, test validator
|-- Cargo.toml                           # Workspace root
|-- build.sh                             # Build script
|-- docs/
|   |-- ADD_LIQUIDITY_AND_MINT_BOND.md   # PDA derivation + integration guide
|-- programs/
|   |-- lp-bonds/                        # Level 1 Locker program
|   |   |-- Cargo.toml                   # anchor-lang 0.32.1, anchor-spl 0.32.1
|   |   |-- src/
|   |       |-- lib.rs                   # Program entry: 10 instructions, account structs
|   |       |-- state.rs                 # ProtocolConfig, PositionCustody, OracleConfig, NonceAccount
|   |       |-- errors.rs                # LpBondsError enum (6000-6119)
|   |       |-- events.rs               # 13 event structs
|   |       |-- constants.rs             # Seeds, oracle constants, tick bounds, NFT defaults
|   |       |-- ed25519.rs               # Ed25519 oracle attestation verification (844 lines w/ tests)
|   |       |-- whirlpool_cpi.rs         # Orca Whirlpool CPI: open/increase/decrease/collect/close
|   |-- lp-bonds-evolution/              # Level 2-4 Evolution program
|       |-- Cargo.toml
|       |-- src/
|           |-- lib.rs                   # Program entry: 12 instructions, account structs
|           |-- state.rs                 # EvolutionConfig, LevelConfig, EvolutionRecord, etc.
|           |-- errors.rs                # EvolutionError enum (6000-6109)
|           |-- events.rs               # 13 event structs
|           |-- constants.rs             # Seeds, oracle, tick bounds, permissions bitmask
|           |-- ed25519.rs               # Parallel ed25519 verification for evolution messages
|           |-- whirlpool_cpi.rs         # Identical Orca CPI module
|-- scripts/
|   |-- admin-configure.ts               # Admin configuration script
|   |-- burn-excess-supply.ts            # Burn excess token supply
|   |-- check-prerequisites.ts           # Pre-flight checks
|   |-- configure-bonds.ts               # Bond configuration
|   |-- reconfigure-levels.ts            # Level reconfiguration
|   |-- transfer-mint-authority.ts       # Mint authority transfer
|   |-- user-test.ts                     # User-facing test
|-- tests/
|   |-- lp-bonds.ts                      # LP bonds integration tests
|   |-- lp-bonds-evolution.ts            # Evolution integration tests
|   |-- oracle-verification.ts           # Oracle verification tests
|-- version-optimization/                # Deployment/config transaction logs
```

### Program IDs

| Program | ID | Network |
|---|---|---|
| lp-bonds | `Hjba1MCsx8WUtuVSyYY8QFvTzEjxsTPAUrkwJPTgQJf8` | devnet |
| lp-bonds-evolution | `9VAsVsZpSqkwT3jBXe9yqKd1GSy9pH4ZpDduttsGoXPr` | devnet |

### Call Graph (lp-bonds)

```
initialize
  |-> validate tick range
  |-> init ProtocolConfig PDA

update_config           -> AdminOnly constraint (admin signer check)
pause / unpause         -> AdminOnly constraint
propose_admin           -> AdminOnly constraint
accept_admin            -> AcceptAdmin constraint (pending_admin signer check)

initialize_oracle       -> admin == config.admin check, init OracleConfig PDA
update_oracle_authority -> admin == oracle_config.admin check
initialize_nonce        -> user signer, init NonceAccount PDA

add_liquidity_and_mint_bond
  |-> pause check
  |-> input validation (liquidity > 0, at least one token amount > 0)
  |-> oracle verification block:
  |     |-> oracle_config.enabled check
  |     |-> nonce == current_nonce + 1
  |     |-> timestamp staleness check (60s window)
  |     |-> reconstruct_oracle_message()
  |     |-> verify_oracle_attestation() -> Ed25519 instruction at (current_index - 1)
  |     |-> commit nonce
  |-> whirlpool state validation block:
  |     |-> Whirlpool::from_account_info() (owner check)
  |     |-> allowlisted_whirlpool check
  |     |-> tick_current == whirlpool_state.tick_current_index
  |     |-> token mints cross-validation (config, whirlpool, user accounts, vaults)
  |     |-> tick spacing alignment
  |     |-> tick array PDA derivation + owner checks
  |-> maybe_wrap_native_if_needed (token A, token B)
  |-> CPI: whirlpool_cpi::open_position
  |-> post-CPI: validate position_token_account mint, position whirlpool
  |-> CPI: create custody ATA
  |-> CPI: whirlpool_cpi::increase_liquidity
  |-> SPL transfer: position NFT to custody
  |-> SPL mint_to: bond NFT (via bond_authority PDA)
  |-> update config.bond_counter
  |-> init PositionCustody
  |-> emit BondMinted, OracleVerifiedForMint

redeem_bond
  |-> pause check
  |-> lock expired check
  |-> SPL burn: bond NFT
  |-> SPL transfer: position NFT from custody to user (via custody PDA signer)
  |-> emit BondRedeemed

verify_collateral
  |-> bond_mint == custody.bond_mint
  |-> whirlpool deserialization + owner check
  |-> whirlpool == custody.whirlpool
  |-> tick_current match
  |-> oracle verification (same pattern as mint)
  |-> commit nonce
  |-> emit CollateralVerified
```

### Call Graph (lp-bonds-evolution)

```
initialize_evolution          -> init EvolutionConfig PDA
initialize_layer_authority    -> admin check, init LayerTokenAuthority PDA
create_layer_token_mint       -> admin check, init Mint with layer_token_authority as authority
configure_level               -> admin check, init_if_needed LevelConfig PDA
pause_evolution / unpause     -> admin check
update_treasury / update_oracle -> admin check
propose_admin / accept_admin  -> two-step admin transfer
add_authority / remove_authority -> admin check, AuthorityWhitelist PDA init/close
initialize_evolution_nonce    -> user signer, init EvolutionNonce PDA

evolve_bond
  |-> remaining_accounts: tick_array_lower, tick_array_upper, token_vault_a, token_vault_b
  |-> pause check
  |-> input validation (liquidity > 0, at least one token amount > 0)
  |-> enforce minimum amounts from LevelConfig
  |-> validate_source_custody():
  |     |-> owner == lp_bonds_program_id OR evolution program
  |     |-> PDA derivation check
  |     |-> PositionCustodyRef deserialization
  |     |-> bond_mint match
  |     |-> is_evolved == false (double-evolution prevention)
  |     |-> custody.whirlpool != Pubkey::default()
  |-> level transition: target_level == source_level + 1, <= MAX_BOND_LEVEL (4)
  |-> oracle verification (validate_oracle_and_nonce):
  |     |-> nonce == current_nonce + 1
  |     |-> timestamp staleness (60s)
  |     |-> reconstruct_evolution_message()
  |     |-> verify_evolution_signature() -> Ed25519 at (current_index - 1)
  |-> commit nonce
  |-> validate_whirlpool_and_ticks():
  |     |-> whirlpool deserialization + owner check
  |     |-> whirlpool == level_config.whirlpool
  |     |-> tick_current match
  |     |-> token mints cross-validation
  |     |-> token vaults cross-validation
  |     |-> tick spacing alignment
  |     |-> tick array PDA derivation + owner + coverage checks
  |-> bond ownership: user_source_bond_account.amount == 1
  |-> SPL burn: source bond NFT
  |-> SPL transfer: token A from user to program
  |-> SPL mint_to: layer tokens (amount_b) via layer_token_authority
  |-> SPL transfer: protocol fee to treasury
  |-> SPL approve: whirlpool_program delegate on program token accounts
  |-> CPI: open_position (position_mint data_is_empty check, PDA validation)
  |-> post-CPI: validate position_token_account, position whirlpool
  |-> CPI: create custody ATA
  |-> SPL transfer: position NFT to custody
  |-> CPI: increase_liquidity
  |-> SPL mint_to: target bond NFT (via bond_authority PDA)
  |-> init PositionCustody
  |-> init EvolutionRecord
  |-> update evolution_counter
  |-> emit BondEvolved
```

### PDA Structure

| PDA | Seeds | Program |
|---|---|---|
| ProtocolConfig | `["config"]` | lp-bonds |
| BondAuthority | `["bond_authority"]` | lp-bonds |
| PositionCustody | `["position_custody", bond_mint]` | lp-bonds or evolution |
| OracleConfig | `["oracle_config"]` | lp-bonds |
| NonceAccount | `["nonce", user]` | lp-bonds |
| EvolutionConfig | `["evolution_config"]` | evolution |
| LevelConfig | `["level_config", level_id]` | evolution |
| EvolutionRecord | `["evolution_record", source_bond_mint]` | evolution |
| LayerTokenAuthority | `["layer_token_authority"]` | evolution |
| EvolutionNonce | `["evolution_nonce", user]` | evolution |
| AuthorityWhitelist | `["authority_whitelist", authority]` | evolution |
| BondAuthority (evo) | `["bond_authority"]` | evolution |
| PositionCustody (evo) | `["position_custody", target_bond_mint]` | evolution |

---

## Section 2: L1-L4 Bond Level System

### EVM Architecture

The EVM system uses two contracts with an integer `bondType` / `bondId` differentiation:

- **LiquidityBondLockerV3**: Level 1 bonds. Creates Uniswap V3 positions via `INonFungiblePositionManager.mint()`. Position NFTs are transferred to a multisig. Bond NFTs are minted to users. Each bond has a `Bond` struct with configurable tick range, fees, lock duration, multiplier, and `isGMIPool` flag. Multiple bonds can be minted in a single transaction (`_numberOfBonds` loop).

- **LiquidityBondsEvolution**: Level 2-4 bonds. Uses a `Layer` struct mapping `(bondId, layerId)` to evolution paths. Burns base layer NFT, mints layer tokens (`IERC20MintBurn`), deducts fee, creates new Uniswap V3 position, transfers to multisig, mints new bond NFT. Requires ownership of source bond NFT (`baseNFT.ownerOf()`).

### Solana Architecture

Two separate Anchor programs:

- **lp-bonds**: Level 1 only. Single allowlisted whirlpool, admin-controlled tick range. Bond NFTs are SPL Token mints with supply=1, decimals=0. Position NFTs are held in PDA-owned token accounts (PositionCustody). Timelocked redemption.

- **lp-bonds-evolution**: Level 2-4. Per-level configuration via `LevelConfig` accounts (whirlpool, tick range, required amounts, fee_bps, lock_duration, multiplier). Burns source bond, mints layer tokens, creates new position in target whirlpool, mints new bond NFT. Evolution record tracked per source bond.

### Gap Analysis Per Level

| Aspect | EVM | Solana | Gap |
|---|---|---|---|
| **Level 1 - Bond creation** | `lockPositionChild` with signature, loop for N bonds | `add_liquidity_and_mint_bond` with oracle attestation, 1 bond per tx | Solana cannot batch-mint due to 1232-byte tx limit. By design. |
| **Level 1 - Bond redemption** | Position NFTs go to multisig; no on-chain unlock | `redeem_bond` with timelock; position NFT returned to user | DIFFERENT: EVM has no on-chain redemption. Solana has full redemption path. |
| **Level 1 - Fee collection** | N/A (positions in multisig) | CPI wrappers exist (`collect_fees`) but no instruction exposed | Whirlpool fee collection not exposed as user instruction |
| **Level 1 - Lock duration** | `startTime` mapping, no on-chain enforcement | `PositionCustody.lock_duration` + `is_lock_expired()` | Solana has stronger lock enforcement |
| **Level 2-4 - Evolution** | `lockPositionChild` with layerId, burns base NFT via transfer to burned multisig | `evolve_bond` with oracle, burns bond via SPL burn | Architectural difference: EVM burns by transfer; Solana burns properly |
| **Level 2-4 - Layer tokens** | `IERC20MintBurn.mint()` on configured token | `layer_token_authority` PDA mints via `MintTo` CPI | Functionally equivalent |
| **Level 2-4 - Fees** | `(amount0 * numberOfBonds * layer.fee) / 10000`, transferred to multisig | `calculate_fee(amount_a)` with `fee_bps / 10000`, transferred to treasury | Equivalent logic |
| **Level 2-4 - Double-evolution** | Burns base NFT (transfer to burned address), preventing reuse | `is_evolved` flag check + evolution_record PDA (keyed by source_bond_mint) | Solana has redundant double-prevention |
| **Multiple bonds per tx** | EVM supports N bonds in a single tx via loop | Solana: 1 bond per tx (by design due to tx size limits) | By design |
| **Base position / first position** | `basePositions` mapping + `lockPositionBase` for initial setup | No equivalent; single allowlisted whirlpool config | Different model |

---

## Section 3: Feature Parity Gaps

| EVM Feature | EVM Location | Solana Status | Notes |
|---|---|---|---|
| **Batch minting (N bonds/tx)** | `LiquidityBondLockerV3.lockPositionChild` loop | NOT IMPLEMENTED | Solana tx size limit prevents this. Acceptable. |
| **Base position creation** | `lockPositionBase` (admin creates first position) | NOT IMPLEMENTED | Solana uses different model (admin sets whirlpool config). |
| **Position transfer to multisig** | `uniswapPositionManager.transferFrom(this, multiSig, tokenId)` | NOT IMPLEMENTED | Solana custodies in PDA instead. Superior design. |
| **Multisig storage** | `multiSig` / `multiSigBurned` addresses | NOT NEEDED | PDA custody replaces multisig. |
| **ETH/WETH wrapping** | `weth.deposit{value: msg.value}()` in lockPositionChild | IMPLEMENTED | `maybe_wrap_native_if_needed()` handles SOL wrapping. |
| **Weird ERC20 handling** | `weirdERC20s` mapping, special approve/transfer | NOT NEEDED | SPL Token is uniform. No "weird token" edge cases. |
| **Bond rewards query** | `getRewards0()` view function | NOT IMPLEMENTED | No on-chain rewards calculation. |
| **ERC721 recovery** | `recoverERC721()` owner-only | NOT IMPLEMENTED | No emergency NFT recovery mechanism. |
| **ERC20 recovery** | `recoverERC20()` owner-only | NOT IMPLEMENTED | No emergency token recovery mechanism. |
| **ETH recovery** | `recoverETH()` owner-only | NOT IMPLEMENTED | No emergency SOL recovery mechanism. |
| **ReentrancyGuard** | `nonReentrant` modifier on all state-changing functions | NOT NEEDED | Solana's execution model prevents reentrancy. |
| **Upgradeable proxy** | OZ Upgradeable pattern (proxy + implementation) | PARTIAL | Anchor programs can be upgraded via `solana program deploy --program-id`. No proxy pattern needed, but `Anchor.toml` has `upgradeable = false` in test config. |
| **ECDSA signature (EVM)** | `ECDSA.recover` + `toEthSignedMessageHash` | ADAPTED | Ed25519 precompile + oracle attestation. Different crypto primitive, equivalent security model. |
| **Global nonce** | Single `uint256 nonce` for all users | ADAPTED | Per-user nonce accounts. Better isolation. |
| **Lock/unlock with position ID** | `locks[uniswapV3PositionId]` mapping | ADAPTED | `PositionCustody` PDA keyed by bond_mint. Different but functionally equivalent. |
| **Bond `isGMIPool` flag** | Per-bond configuration flag | NOT IMPLEMENTED | Unclear if needed. See Section 7. |
| **Bond `amount0Min` / `amount1Min`** | Per-bond minimum output amounts (slippage) | NOT IMPLEMENTED | Oracle attestation replaces slippage protection. Oracle signs exact amounts. |
| **Layer `outputLayer` / `baseLayer` NFT collections** | Layer struct with separate NFT collection addresses | ADAPTED | Solana uses single bond_authority PDA per program for minting. Different model. |
| **Fee collection from positions** | Positions in multisig; fees collected externally | GAP | Position fees accrue in custody but no instruction to collect them. |
| **Oracle enable/disable** | N/A (always uses signer) | IMPLEMENTED | `OracleConfig.enabled` flag. Additional feature. |
| **Two-step admin transfer** | N/A (single `transferOwnership`) | IMPLEMENTED | `propose_admin` + `accept_admin`. Security improvement. |
| **Authority whitelist** | N/A | IMPLEMENTED (evolution) | `AuthorityWhitelist` with permission bitmask. Additional feature. |
| **Collateral verification** | N/A | IMPLEMENTED | `verify_collateral` instruction. Additional feature. |

---

## Section 4: Security Vulnerabilities

### [MEDIUM] M-01: PositionCustody account not closed on redeem_bond

**File:** `/programs/lp-bonds/src/lib.rs` (lines 683-737)
**Function:** `redeem_bond`

**Description:** When a user redeems a bond, the PositionCustody account is not closed. The bond NFT is burned and the position NFT is transferred to the user, but the PositionCustody PDA remains on-chain with stale data. This wastes rent (approximately 0.002 SOL per custody account) and leaves state artifacts. While not directly exploitable (the bond is burned, preventing re-redemption), the stale account could cause confusion if the bond_mint is somehow reused.

**Exploit scenario:** Over time, thousands of PositionCustody accounts accumulate on-chain, locking rent-exempt SOL permanently. No mechanism exists to reclaim these funds.

**Recommended fix:** Add `close = user` to the `position_custody` account in the `RedeemBond` struct, or create a separate `close_custody` admin instruction. Similarly consider closing `custody_position_token_account` after transfer.

---

### [MEDIUM] M-02: PositionCustody account not closed on evolve_bond (source custody)

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (line 1328)
**Function:** `evolve_bond`

**Description:** The `source_custody` account is read for validation but is never closed or marked. While `is_evolved` flag is checked to prevent double-evolution, the source PositionCustody remains on-chain holding a position NFT for a bond that was burned. Since the source bond mint still has a PositionCustody PDA (from the base program), and the evolution program only checks `is_evolved` via deserialization, the old custody data persists.

**Exploit scenario:** Same rent-locking issue as M-01. Additionally, the source custody still holds the original position NFT. The old position is never moved or closed -- it becomes permanently stuck in the PDA.

**Recommended fix:** The evolution flow should either (a) transfer the source position NFT to the user or treasury, or (b) close the source position via Whirlpool CPI (`decrease_liquidity` to 0, then `close_position`). This requires CPI from the evolution program to the base program or direct Whirlpool CPI. Flag for human review regarding business intent.

---

### [MEDIUM] M-03: Source bond is burned but source whirlpool position remains locked

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (lines 464-475)
**Function:** `evolve_bond` (STEP 1: Burn source bond NFT)

**Description:** When evolving a bond, the source bond NFT is burned, but the Whirlpool position associated with the source bond (held by the source PositionCustody PDA) remains locked. The position NFT is still in the custody PDA's token account. Since the bond is burned, no one can call `redeem_bond` on the base program to retrieve it. The source liquidity is effectively lost.

**Exploit scenario:** A user evolves a Level 1 bond worth 10 SOL in liquidity to Level 2. The Level 1 position (10 SOL liquidity) is permanently locked in the base program's PositionCustody PDA. The user has a new Level 2 position with new liquidity, but their original liquidity is gone.

**Recommended fix:** This is likely a critical business logic question. If by design, clearly document that evolution consumes the source position. If not by design, the evolution flow needs to extract the source position's liquidity before or after burning the bond.

---

### [MEDIUM] M-04: Evolution source level defaulting logic could allow bypass

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (lines 829-833)
**Function:** `validate_source_custody`

**Description:** The source level detection has a fallback that treats level 0 or 255 as level 1:
```rust
let source_level: u8 = if custody_ref.level == 0 || custody_ref.level == 255 {
    1
} else {
    custody_ref.level
};
```
This means if an attacker can craft a PositionCustody account with `level = 0`, it would be treated as Level 1 and eligible for evolution to Level 2. While the owner check (must be lp-bonds program or evolution program) prevents arbitrary accounts, corrupted or uninitialized data in a legitimate custody account could trigger this fallback.

**Exploit scenario:** If a Level 4 bond's custody had its level field corrupted to 0 (e.g., through a reinitialization bug), it would be treated as Level 1 and could be evolved again through levels 2-4, effectively getting multiple evolutions from one bond.

**Recommended fix:** Remove the fallback. Require `custody_ref.level >= MIN_BOND_LEVEL && custody_ref.level <= MAX_BOND_LEVEL` explicitly. If the level field is 0 or 255, reject with `InvalidBondLevel`.

---

### [MEDIUM] M-05: approve() to whirlpool_program is incorrect pattern

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (lines 540-564)
**Function:** `evolve_bond` (STEP 5: Approve tokens for Whirlpool)

**Description:** The code approves the `whirlpool_program` as delegate on program token accounts. The comment acknowledges this is "redundant but harmless." However, setting a program as a delegate is semantically incorrect -- SPL Token delegates are pubkeys that can call `transfer` or `burn` on the delegated account. A program ID cannot sign transactions. More importantly, the delegate remains set after the CPI completes, which means if the whirlpool program has any instruction that uses delegate authority, it could drain these accounts in future transactions.

**Exploit scenario:** The Orca Whirlpool program has no known mechanism to exploit delegate authority, and the delegate amount is limited to the specific amounts approved. However, the delegate persists across transactions. If the program token accounts accumulate tokens from multiple evolutions (since they use `init_if_needed`), leftover balances from prior transactions could be at risk if Orca ever added a sweep-delegate instruction.

**Recommended fix:** Either (a) remove the approvals entirely since they are documented as redundant, or (b) revoke the delegate after the CPI completes using `token::revoke()`.

---

### [LOW] L-01: Oracle admin can differ from protocol admin

**File:** `/programs/lp-bonds/src/lib.rs` (lines 1267-1274)
**Function:** `UpdateOracleAuthority` account struct

**Description:** The `UpdateOracleAuthority` struct checks `admin.key() == oracle_config.admin`, not `config.admin`. The OracleConfig stores its own admin field, which is set during `initialize_oracle` to the signer at that time. If the protocol admin is transferred via `propose_admin`/`accept_admin`, the oracle admin remains the old admin. This creates a split authority where the old admin retains oracle control.

**Exploit scenario:** Admin A initializes the protocol and oracle. Admin A proposes and transfers protocol admin to Admin B. Admin A can still update the oracle authority because `oracle_config.admin` was never updated.

**Recommended fix:** Either (a) have `update_oracle_authority` check against `config.admin` instead of `oracle_config.admin`, or (b) update `oracle_config.admin` as part of `accept_admin`, or (c) add an explicit `transfer_oracle_admin` instruction.

---

### [LOW] L-02: Redundant signer check

**File:** `/programs/lp-bonds/src/lib.rs` (lines 282-285)
**Function:** `add_liquidity_and_mint_bond`

**Description:**
```rust
require!(
    ctx.accounts.user.is_signer,
    LpBondsError::UnauthorizedSigner
);
```
The `user` account is declared as `Signer<'info>` in the `AddLiquidityAndMintBond` struct (line 999), which means Anchor automatically validates that the account is a signer before the instruction handler executes. This `require!` is redundant. The same pattern appears in `evolve_bond` (line 382-385).

**Exploit scenario:** None. This is a code quality issue, not a security issue.

**Recommended fix:** Remove the redundant check or convert to a comment documenting the intent.

---

### [LOW] L-03: Whirlpool deserialization does not validate discriminator

**File:** `/programs/lp-bonds/src/whirlpool_cpi.rs` (lines 43-58) and `/programs/lp-bonds-evolution/src/whirlpool_cpi.rs` (same)
**Function:** `Whirlpool::from_account_info`

**Description:** The deserialization skips the first 8 bytes (Anchor discriminator) but does not validate that the discriminator matches the expected Whirlpool account type. It only checks that the account is owned by the Whirlpool program. If the Whirlpool program has other account types with the same owner, a different account type could be supplied and deserialized as a Whirlpool struct, potentially yielding garbage values.

**Exploit scenario:** An attacker supplies a Whirlpool program-owned account that is not a Whirlpool (e.g., a TickArray or FeeTier account). The first 32 bytes after the discriminator would be interpreted as `whirlpools_config`, potentially passing validation if the byte layout happens to match expected values. This is mitigated by the subsequent field-level validation (token mints, vaults), but the attack surface exists.

**Recommended fix:** Validate the 8-byte discriminator against the known Whirlpool account discriminator before deserialization.

---

### [LOW] L-04: init-if-needed on shared program token accounts in evolution

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (lines 1389-1403)
**Function:** `EvolveBond` struct -- `program_token_a_account` and `program_token_b_account`

**Description:** The program's token accounts (`program_token_a_account` and `program_token_b_account`) use `init_if_needed` with `associated_token::authority = layer_token_authority`. These accounts are shared across all evolution transactions. If token A or layer token amounts from previous transactions are not fully consumed by the Whirlpool CPI (due to rounding or slippage), residual balances accumulate in these accounts.

**Exploit scenario:** Transaction 1 deposits 1000 tokens but Whirlpool only consumes 999. Transaction 2 deposits 1000 tokens, and the account now has 1001. The extra 1 token from Transaction 1 is available for Transaction 2's position. Over many transactions, this leakage could benefit later users at the expense of earlier ones.

**Recommended fix:** After the `increase_liquidity` CPI, check balances and either (a) return excess tokens to the user, or (b) transfer them to the treasury.

---

### [LOW] L-05: No fee collection instruction for custodied positions

**File:** Both programs

**Description:** While `whirlpool_cpi.rs` defines a `collect_fees` CPI helper function, neither program exposes a user-facing instruction to collect accumulated trading fees from custodied Whirlpool positions. Fees will accrue to the positions but cannot be harvested by the bond holder during the lock period.

**Exploit scenario:** A bond holder locks a position for 90 days. During that time, the position earns significant trading fees. The bond holder cannot access these fees until redemption. Upon redemption, they get the position NFT and can collect fees themselves, but this delays their earnings.

**Recommended fix:** Add a `collect_fees` instruction (admin-only or bond-holder-accessible) that collects fees from the custodied position and sends them to the bond holder or a designated recipient.

---

### [LOW] L-06: No event emission for oracle enable/disable toggle

**File:** `/programs/lp-bonds/src/lib.rs`

**Description:** There is no instruction to toggle `oracle_config.enabled`. The field is set to `true` during `initialize_oracle` but there is no way to disable or re-enable the oracle. If the oracle key is compromised, the only option is to update the authority, but the oracle cannot be disabled entirely.

**Exploit scenario:** If the oracle backend goes down, all minting is blocked because oracle verification is mandatory. There is no emergency disable.

**Recommended fix:** Add `enable_oracle` and `disable_oracle` admin instructions with corresponding events. Alternatively, if oracle should never be disabled, document this decision.

---

### [LOW] L-07: token_vault_a/b type mismatch between constraint and handler validation

**File:** `/programs/lp-bonds/src/lib.rs` (lines 1086-1098)
**Function:** `AddLiquidityAndMintBond` struct

**Description:** `token_vault_a` and `token_vault_b` are declared as `Box<Account<'info, TokenAccount>>` with constraints checking their `mint` field. They are also validated in the handler against `whirlpool_state.token_vault_a/b` addresses. However, since they are declared as `Account<'info, TokenAccount>`, Anchor validates that they are valid SPL Token accounts owned by the Token program. The Whirlpool's vaults are indeed SPL Token accounts, so this is not a vulnerability per se, but the handler additionally validates they match the whirlpool's expected vaults. This is good defense-in-depth.

**Exploit scenario:** None directly. The redundant validation is appropriate.

---

## Section 5: Architectural Mismatches

### ERC20/EVM Pattern to SPL/Solana Adaptation

| EVM Pattern | Expected Solana Adaptation | Current Status |
|---|---|---|
| **msg.sender** | `Signer<'info>` account type | CORRECT: All user-facing instructions use `Signer<'info>` |
| **Ownable (single owner)** | Admin pubkey in config PDA | CORRECT: Two-step transfer implemented |
| **Pausable** | `is_paused` bool in config | CORRECT: Both programs implement pause |
| **ReentrancyGuard** | Not needed (Solana runtime prevents reentrancy) | CORRECT: Not implemented |
| **Upgradeable proxy** | Program authority upgrade | NOTED: `upgradeable = false` in test config. Production deployment approach unclear. |
| **ERC721 (bond NFT)** | SPL Token with supply=1, decimals=0 | CORRECT: Bond mints are 0-decimal, supply-1 tokens |
| **mapping(uint => struct)** | PDA per key | CORRECT: PositionCustody PDA per bond_mint |
| **transferFrom (ERC20)** | SPL Token transfer CPI | CORRECT: Standard token::transfer used |
| **approve (ERC20)** | SPL Token approve/delegate | NOTED: Used in evolution for whirlpool_program (unnecessary) |
| **ECDSA.recover** | Ed25519 precompile instruction | CORRECT: Well-implemented with strict ordering |
| **Global nonce** | Per-user nonce accounts | IMPROVED: Better isolation than EVM global nonce |
| **Uniswap V3 Position Manager** | Orca Whirlpool CPI | ADAPTED: Direct CPI with position NFT custody |
| **address(this)** | Program ID (`crate::ID`) | CORRECT: Used in oracle message reconstruction |
| **require(..., "message")** | `require!(..., ErrorCode)` | CORRECT: Rich error codes |
| **Events (Solidity)** | Anchor `emit!` macro | CORRECT: Comprehensive event coverage |

### Significant Architectural Differences

1. **Position Custody Model**: EVM transfers positions to a multisig wallet. Solana uses PDA-owned token accounts. The Solana approach is superior for on-chain redemption but means the program holds position authority, requiring careful CPI seed management.

2. **Bond NFT Model**: EVM uses a separate `ILiquidityBonds` ERC721 collection contract. Solana creates a fresh SPL Token mint per bond (supply=1, decimals=0). Each bond is its own mint address rather than a token ID within a collection. This is a standard Solana NFT pattern but means there's no single "collection" address for enumeration.

3. **Layer Token Minting**: EVM calls `curToken1.mint(address(this), amount)` via `IERC20MintBurn` interface. Solana uses `LayerTokenAuthority` PDA as the mint authority. This is architecturally sound -- the evolution program controls minting via PDA seeds.

4. **Fee Distribution**: EVM sends fees to `multiSig`. Solana sends to a `treasury` address configured in `EvolutionConfig`. The Solana approach is more flexible but requires treasury token accounts to exist.

5. **Signature Verification**: EVM uses `ECDSA.recover` with Ethereum signed message prefix. Solana uses Ed25519 precompile with strict instruction ordering. The Solana approach is more secure (runtime-verified signatures, strict ordering prevents replay across instructions) but requires transaction construction to include the Ed25519 instruction.

6. **Source Bond Burning on Evolution**: EVM transfers source NFT to `multiSigBurned` (an address that holds "burned" NFTs). Solana uses SPL `token::burn` (permanent destruction). Solana's approach is cleaner but means the source bond is irrecoverable.

---

## Section 6: Prioritized Fix Recommendations

### Priority 1 -- [MEDIUM] Source position liquidity stranded on evolution (M-03)

**File:** `/programs/lp-bonds-evolution/src/lib.rs`
**Function:** `evolve_bond`

**Issue:** When a bond is evolved, the source Whirlpool position (potentially containing significant liquidity) is permanently locked in the base program's PositionCustody PDA. The bond NFT that controls it is burned.

**Fix:** Determine business intent (see Section 7). If the source liquidity should be extracted:
1. Add a CPI from the evolution program to the Whirlpool program to `decrease_liquidity` (to 0), `collect_fees`, and `close_position` on the source position.
2. Transfer the extracted tokens to the user or treasury.
3. Close the source PositionCustody account.

---

### Priority 2 -- [MEDIUM] PositionCustody accounts never closed (M-01, M-02)

**File:** `/programs/lp-bonds/src/lib.rs` (redeem_bond) and evolution program
**Function:** `redeem_bond`, `evolve_bond`

**Issue:** PositionCustody accounts persist after bond redemption or evolution, permanently locking ~0.002 SOL per bond.

**Fix:** In `RedeemBond`:
```rust
#[account(
    mut,
    close = user,
    seeds = [POSITION_CUSTODY_SEED, bond_mint.key().as_ref()],
    bump = position_custody.bump,
    ...
)]
pub position_custody: Account<'info, PositionCustody>,
```
Similarly close `custody_position_token_account` after the position NFT is transferred out.

---

### Priority 3 -- [MEDIUM] Evolution source level fallback to 1 for level=0 (M-04)

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (line 829-833)
**Function:** `validate_source_custody`

**Issue:** Level 0 or 255 defaults to Level 1, potentially allowing re-evolution of high-level bonds.

**Fix:**
```rust
require!(
    custody_ref.level >= MIN_BOND_LEVEL && custody_ref.level <= MAX_BOND_LEVEL,
    EvolutionError::InvalidBondLevel
);
let source_level = custody_ref.level;
```

---

### Priority 4 -- [MEDIUM] Redundant and potentially risky token approvals (M-05)

**File:** `/programs/lp-bonds-evolution/src/lib.rs` (lines 540-564)
**Function:** `evolve_bond`

**Issue:** `token::approve` to whirlpool_program as delegate is redundant and leaves delegate set after CPI.

**Fix:** Remove both `token::approve` calls entirely since the `increase_liquidity` CPI uses `position_authority` (the `layer_token_authority` PDA, which is the owner of these accounts and signs via `invoke_signed`).

---

### Priority 5 -- [LOW] Oracle admin divergence from protocol admin (L-01)

**File:** `/programs/lp-bonds/src/lib.rs` (lines 1267-1274)
**Function:** `UpdateOracleAuthority`

**Issue:** Oracle admin is set once and never updated when protocol admin changes.

**Fix:** Change the constraint in `UpdateOracleAuthority` to use `config.admin`:
```rust
#[derive(Accounts)]
pub struct UpdateOracleAuthority<'info> {
    #[account(constraint = admin.key() == config.admin @ LpBondsError::InvalidAdminAuthority)]
    pub admin: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,
    #[account(mut, seeds = [ORACLE_CONFIG_SEED], bump = oracle_config.bump)]
    pub oracle_config: Account<'info, OracleConfig>,
}
```

---

### Priority 6 -- [LOW] Add Whirlpool account discriminator validation (L-03)

**File:** `/programs/lp-bonds/src/whirlpool_cpi.rs` and `/programs/lp-bonds-evolution/src/whirlpool_cpi.rs`
**Function:** `Whirlpool::from_account_info`

**Fix:** Add discriminator check:
```rust
let data = account.try_borrow_data()?;
require!(data.len() >= 8, ErrorCode::AccountDidNotDeserialize);
// Anchor discriminator for Whirlpool: sha256("account:Whirlpool")[..8]
const WHIRLPOOL_DISCRIMINATOR: [u8; 8] = [63, 149, 209, 12, 225, 128, 99, 9];
require!(
    data[..8] == WHIRLPOOL_DISCRIMINATOR,
    ErrorCode::AccountDiscriminatorMismatch
);
```
Note: The actual discriminator value should be verified against the Orca Whirlpool program source.

---

### Priority 7 -- [LOW] Add fee collection instruction (L-05)

**File:** Both programs

**Fix:** Add a `collect_position_fees` instruction that:
1. Takes `bond_mint`, `position_custody`, `whirlpool`, token accounts, etc.
2. Verifies the caller holds the bond NFT (or is admin).
3. Calls `whirlpool_cpi::collect_fees` via the custody PDA as position authority.
4. Transfers collected fees to the bond holder's token accounts.

---

### Priority 8 -- [LOW] Add oracle disable/enable toggle (L-06)

**File:** `/programs/lp-bonds/src/lib.rs`

**Fix:** Add `enable_oracle` and `disable_oracle` instructions:
```rust
pub fn disable_oracle(ctx: Context<AdminOnly>) -> Result<()> {
    // Also need oracle_config in the context
    ctx.accounts.oracle_config.enabled = false;
    emit!(OracleDisabled { ... });
    Ok(())
}
```
Requires adding `oracle_config` to the `AdminOnly` context or creating a new context.

---

### Priority 9 -- [LOW] Clean up residual token balances in evolution (L-04)

**File:** `/programs/lp-bonds-evolution/src/lib.rs`

**Fix:** After `increase_liquidity` CPI, reload program token accounts and return any excess to the user:
```rust
// After increase_liquidity CPI
ctx.accounts.program_token_a_account.reload()?;
if ctx.accounts.program_token_a_account.amount > 0 {
    token::transfer(
        CpiContext::new_with_signer(...),
        ctx.accounts.program_token_a_account.amount,
    )?;
}
```

---

## Section 7: Items Requiring Human Review

### 7.1: Source Position Liquidity on Evolution (CRITICAL BUSINESS DECISION)

**Question:** When a Level 1 bond is evolved to Level 2, what should happen to the original Whirlpool position held by the Level 1 PositionCustody PDA?

**Options:**
- A) Liquidity is intentionally "locked forever" as an economic mechanism (stake-and-forget).
- B) Liquidity should be withdrawn and returned to the user (minus fees).
- C) Liquidity should be withdrawn and added to the new Level 2 position.
- D) Liquidity should be withdrawn and sent to the treasury as an evolution cost.

**Impact:** If option A, document clearly. If B/C/D, significant code changes needed.

### 7.2: `isGMIPool` Flag Equivalent

**Question:** The EVM `Bond.isGMIPool` flag differentiates between GMI pools and standard pools. Is this distinction needed on Solana, or do all Solana whirlpools use the same token ordering?

### 7.3: Bond NFT Metadata

**Question:** `BOND_NFT_NAME_PREFIX`, `BOND_NFT_SYMBOL`, and `BOND_NFT_URI_BASE` constants are defined but no Metaplex metadata is attached to bond NFTs. Is on-chain metadata required for marketplace/wallet display?

### 7.4: Rewards Calculation

**Question:** The EVM contract has `getRewards0()` for calculating earned rewards. Solana has no equivalent. Is rewards calculation handled entirely off-chain, or should there be an on-chain view function?

### 7.5: Multiple Bonds Per Transaction

**Question:** The EVM supports minting N bonds in a single transaction via a loop. The Solana implementation limits to 1 bond per transaction due to the 1232-byte limit. Is this an acceptable tradeoff, or should a versioned/lookup table transaction approach be explored?

### 7.6: Whirlpool Discriminator Value

**Question:** L-03 recommends validating the Whirlpool account discriminator. The exact discriminator bytes need to be verified against the Orca Whirlpool program source. The audit cannot determine the correct value without access to the Orca codebase.

### 7.7: Program Upgradeability

**Question:** The `Anchor.toml` sets `upgradeable = false` for tests. What is the production deployment strategy? If programs will be deployed as non-upgradeable, there is no recovery path for bugs. If upgradeable, the upgrade authority keypair management needs to be documented.

### 7.8: Fee BPS Cap in Evolution

**Question:** `MAX_FEE_BPS = 5000` (50%). Is this intentional? A 50% fee cap seems high. The EVM contract has no explicit cap on the fee field in the Bond struct.

### 7.9: Authority Whitelist Unused

**Question:** The `AuthorityWhitelist` system (with `PERM_CONFIGURE_LEVELS`, `PERM_PAUSE`, etc.) is defined and has add/remove instructions, but no instruction in the evolution program checks the whitelist. The admin checks all use `evolution_config.admin`. Is the whitelist intended for future use, or should it be integrated into existing admin checks?

### 7.10: Cross-Program Position Custody Reading

**Question:** The evolution program reads PositionCustody from the base program via `PositionCustodyRef` deserialization. If the base program's PositionCustody struct is modified (fields added/removed/reordered), the evolution program's deserialization will break. Is there a versioning strategy for cross-program account compatibility?
