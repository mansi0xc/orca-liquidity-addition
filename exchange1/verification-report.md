# LP Bonds Solana Programs -- Independent Security Verification Report

**Date:** 2026-03-25
**Auditor:** Claude Opus 4.6 (Solana/Anchor Security Specialist -- Fresh Independent Audit)
**Scope:** `lp-bonds` (Level 1 Locker) and `lp-bonds-evolution` (Level 2-4 Evolution)
**Branch:** `initial-implementation`
**Commit:** `37cfa26`

---

## Executive Summary

This is an independent, ground-up security audit of the LP Bonds Solana smart contracts. The programs implement a liquidity bond system on Solana using Orca Whirlpool concentrated liquidity positions, migrated from an EVM (Uniswap V3) architecture.

**Overall Assessment: MODERATE RISK -- Suitable for controlled launch with noted caveats.**

The codebase demonstrates mature security practices: comprehensive oracle verification with Ed25519 precompile, strict PDA derivation checks, whirlpool state cross-validation, and defense-in-depth patterns. Most critical security vulnerabilities from the initial audit have been addressed. However, several medium-severity issues remain that should be resolved before mainnet deployment.

### Finding Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0 | -- |
| HIGH | 2 | Require attention before mainnet |
| MEDIUM | 4 | Should fix before mainnet |
| LOW | 5 | Can fix post-launch |
| INFORMATIONAL | 3 | Acceptable risk / design decisions |

---

## PART 1: EVM Feature Parity Table

### LiquidityBondLockerV3.sol (ABSLiquidityBondLockerV3)

| EVM Function | Solana Equivalent | Status | Notes |
|---|---|---|---|
| `initialize(weth, posManager, signer)` | `initialize(whirlpool, ...)` | IMPLEMENTED | Solana uses Whirlpool instead of Uniswap V3 |
| `lockPositionChild(bondId, amount0, amount1, sig, isEth, numBonds)` | `add_liquidity_and_mint_bond(...)` | PARTIAL | No batch minting (`_numberOfBonds` loop) -- by design due to tx size limits [LOW] |
| `setBond(bondId, ...)` | `update_config(...)` | PARTIAL | EVM supports N bond configs; Solana has single whirlpool per L1 deployment [MEDIUM] |
| `_verifySignature(bondId, amount0, amount1, sig)` | `verify_oracle_attestation(...)` | IMPLEMENTED | Ed25519 precompile instead of ECDSA ecrecover; more fields in message |
| `setSigner(newSigner)` | `update_oracle_authority(...)` | IMPLEMENTED | |
| `setUniswapPositionManager(...)` | N/A (hardcoded WHIRLPOOL_PROGRAM_ID) | N/A | Correct for Solana -- program IDs are immutable |
| `setWeth(...)` | N/A (SOL wrapping handled inline) | IMPLEMENTED | `maybe_wrap_native_if_needed()` handles SOL/WSOL |
| `pause() / unpause()` | `pause() / unpause()` | IMPLEMENTED | |
| `recoverEth(to, amount)` | N/A | MISSING | No native SOL recovery [LOW] |
| `recoverErc20s(token, to, amount)` | `recover_tokens(amount)` | IMPLEMENTED | |
| `recoverErc721(token, to, tokenId)` | N/A | PARTIAL | No direct NFT recovery; `close_orphaned_custody` handles cleanup |
| `onERC721Received(...)` | N/A | N/A | Not needed on Solana (no callback pattern) |
| `_lockPositionBase(bondId, amount0, amount1, senderSig)` | Part of `add_liquidity_and_mint_bond` | IMPLEMENTED | Combined into single instruction |
| Owner transfer | `propose_admin / accept_admin` | IMPLEMENTED | Two-step pattern matches best practices |
| `nonce` (global) | Per-user nonce accounts | IMPLEMENTED | Improved: per-user prevents contention |
| `multiSig` storage | N/A | MISSING | No multisig integration [LOW] |
| `weirdERC20s` mapping | N/A | N/A | Not applicable on Solana |
| `startTime` mapping | `PositionCustody.created_at` | IMPLEMENTED | |
| `locks` mapping | `PositionCustody` PDA | IMPLEMENTED | |

### LiquidityBonds.sol (ERC721 Bond NFT)

| EVM Function | Solana Equivalent | Status | Notes |
|---|---|---|---|
| `mint(to, bondId, positionId)` | Bond mint via `bond_authority` PDA | IMPLEMENTED | SPL Token mint with supply=1 instead of ERC721 |
| `burn(bondId)` | `token::burn()` in redeem/evolve | IMPLEMENTED | |
| `addMinter / removeMinter` | `bond_authority` PDA is sole minter | IMPLEMENTED | Simpler -- PDA controls authority |
| `validateTransfer` modifier | N/A | MISSING | No operator registry / transfer restrictions [MEDIUM] |
| `setLiquidityBondLocker(...)` | N/A (hardcoded via PDA) | N/A | Not needed -- PDA derivation is deterministic |
| `setOperatorRegistry(...)` | N/A | MISSING | No operator registry [LOW] |
| `tokenURI(tokenId)` | No on-chain metadata | MISSING | No Metaplex metadata creation [LOW] |

### LiquidityBondsEvolution.sol (ABSLiquidityBondsEvolution)

| EVM Function | Solana Equivalent | Status | Notes |
|---|---|---|---|
| `initialize(posManager, signer)` | `initialize_evolution(treasury, oracle, program_id)` | IMPLEMENTED | |
| `lockPositionChild(bondId, layerId, baseTokenId, amount0, amount1, fee, sig, numBonds)` | `evolve_bond(...)` | PARTIAL | No batch evolution [LOW -- by design] |
| `setBond(bondId, ...)` | `configure_level(level_id, ...)` | IMPLEMENTED | |
| `setLayer(layerId, ...)` | Part of `configure_level` | IMPLEMENTED | Layer config integrated into LevelConfig |
| `setSigner(newSigner)` | `update_oracle(new_oracle)` | IMPLEMENTED | |
| `pause / unpause` | `pause_evolution / unpause_evolution` | IMPLEMENTED | |
| `recoverEth / recoverErc20s / recoverErc721` | `recover_tokens` / `close_orphaned_custody` | PARTIAL | No native SOL recovery |
| `layers` mapping | `LevelConfig` PDA per level | IMPLEMENTED | |
| `multiSigBurned` | N/A | MISSING | No burned NFT receiver concept [LOW] |
| Bond redemption (unlock) | `redeem_evolved_bond` | IMPLEMENTED | |

### LPBondsExchange.sol

| EVM Function | Solana Equivalent | Status | Notes |
|---|---|---|---|
| Entire contract | N/A | MISSING | No LP Bonds Exchange on Solana [MEDIUM -- discuss with team] |

---

## PART 2: Independent Security Findings

### [H-1] No Fee Collection Instruction for Custodied Positions

**Severity:** HIGH
**File:** `programs/lp-bonds/src/whirlpool_cpi.rs` lines 408-458; `programs/lp-bonds/src/lib.rs` (entire file)
**File:** `programs/lp-bonds-evolution/src/whirlpool_cpi.rs` lines 408-458

**Description:** Both programs define `collect_fees` CPI wrappers but no instruction in either program invokes them. Orca Whirlpool positions accumulate trading fees continuously. Since the position NFT is held by the custody PDA, only that PDA can authorize fee collection. Without a callable instruction, all accumulated fees are permanently locked.

**Impact:** Users' LP positions earn fees that can never be collected. For long lock durations (months), this represents significant economic loss. This is especially impactful for higher-level evolved bonds with longer lock periods.

**Proof of Concept:**
1. User creates Level 1 bond with 90-day lock
2. Over 90 days, position earns trading fees in the whirlpool
3. User redeems bond after lock expiry -- gets position NFT back
4. Fees accumulated BEFORE redemption are lost (collected to custody PDA's token accounts which are not accessible)

**Note:** After redemption, the user gets the position NFT and can collect fees going forward. But fees accumulated during the lock period that were pending at the time of any `update_position` or fee growth events are effectively collected to the custody PDA's associated token accounts and become inaccessible (no instruction to drain them).

**Recommended Fix:** Add `collect_fees` instruction to both programs that:
- Verifies bond ownership (user holds the bond NFT)
- CPIs to `whirlpool_cpi::collect_fees` using custody PDA as signer
- Transfers collected fees to the bond holder's token accounts
- Emits `FeesCollected` event

---

### [H-2] recover_tokens Can Drain Active Custody Position NFTs (lp-bonds)

**Severity:** HIGH
**File:** `programs/lp-bonds/src/lib.rs` lines 826-854 (`recover_tokens`), lines 1447-1475 (`RecoverTokens` struct)

**Description:** The `recover_tokens` instruction transfers tokens from any `bond_authority`-owned token account to the admin. The constraint is only `source_token_account.owner == bond_authority.key()`. However, in the `add_liquidity_and_mint_bond` flow, the custody position token account is created as an ATA owned by `position_custody` PDA, NOT `bond_authority`. So this specific attack path is mitigated for custody accounts.

**However**, the `bond_authority` PDA could own other token accounts created for operational purposes. More importantly, there is NO check that prevents recovering tokens from a token account that happens to hold a position NFT if one were ever transferred to a `bond_authority`-owned account by mistake.

**Mitigating Factor:** In practice, position NFTs are held by `position_custody` PDA-owned accounts, not `bond_authority`-owned accounts. The current code structure makes this low-likelihood but the protection is implicit rather than explicit.

**Impact:** If tokens accidentally end up in a `bond_authority`-owned account, admin can drain them. This is intentional (recovery). The concern is lack of guardrail against recovering position NFTs specifically.

**Recommended Fix:** Add an explicit check that `source_token_account.mint` is not a known position mint, or add a comment acknowledging this is intentionally admin-controlled for emergency use. Consider adding a timelock.

---

### [M-1] Evolution Program's recover_tokens Has No Custody Protection

**Severity:** MEDIUM
**File:** `programs/lp-bonds-evolution/src/lib.rs` lines 364-395 (`recover_tokens`), lines 1788-1815 (`RecoverTokens` struct)

**Description:** The evolution program's `recover_tokens` transfers from any `layer_token_authority`-owned token account. The custody position token accounts in the evolution program ARE owned by `layer_token_authority` PDA (as seen in `RedeemEvolvedBond.custody_position_token_account` constraint: `owner == layer_token_authority.key()`).

This means an admin could potentially use `recover_tokens` to drain a custody position token account holding an active position NFT, effectively stealing a user's locked position.

**Proof of Concept:**
1. User evolves bond to Level 2, position NFT stored in `layer_token_authority`-owned ATA
2. Admin calls `recover_tokens` with `source_token_account` = the custody position token account
3. Admin receives the position NFT, user's bond becomes unredeemable

**Impact:** Admin can steal any active evolved bond's position NFT. While admin is trusted, this violates the trust model where locked positions should be inaccessible even to admin during the lock period.

**Recommended Fix:** Add a constraint that verifies the source token account's mint has supply > 0 AND is not associated with any active PositionCustody. Or more practically, check that `source_token_account.amount` for SPL token mint with `decimals == 0` and `supply == 1` (NFT-like) requires additional authorization.

Simplest fix: Add `RecoveryCustodyProtected` error and check that the source account is NOT the ATA derived from `layer_token_authority` + any known position mint. Or require the admin to pass the bond_mint and verify `bond_mint.supply == 0`.

---

### [M-2] Single Whirlpool Limitation at Level 1 -- Config Update Breaks Existing Bonds

**Severity:** MEDIUM
**File:** `programs/lp-bonds/src/lib.rs` lines 93-124 (`update_config`), `programs/lp-bonds/src/state.rs` line 17

**Description:** `ProtocolConfig` stores a single `allowlisted_whirlpool`. When admin calls `update_config` to change the whirlpool address, existing bonds that were minted against the old whirlpool are still valid (their custody records the original whirlpool). However, the `redeem_bond` instruction does NOT check the whirlpool -- it only checks custody PDA and bond ownership. So existing bonds remain redeemable.

The real issue is that `verify_collateral` uses `custody.whirlpool` (not config), which is correct. But `add_liquidity_and_mint_bond` requires `whirlpool.key() == config.allowlisted_whirlpool`, meaning after a config update, new bonds can only be minted against the new whirlpool.

**Impact:** Config changes are safe for existing bonds but create operational complexity. This is an architectural limitation, not a vulnerability.

---

### [M-3] No Validation That lock_duration > 0 in Both Programs

**Severity:** MEDIUM
**File:** `programs/lp-bonds/src/lib.rs` line 58 (`initialize`), line 100 (`update_config`)
**File:** `programs/lp-bonds-evolution/src/lib.rs` line 107 (`configure_level`)

**Description:** Neither `initialize`, `update_config`, nor `configure_level` validate that `lock_duration > 0`. An admin could set `lock_duration = 0`, allowing immediate redemption after minting/evolution, which defeats the purpose of the bond locking mechanism.

**Impact:** Misconfiguration risk. If `lock_duration = 0`, bonds can be immediately redeemed, which could be exploited in conjunction with flash-loan-like patterns to:
1. Mint bond (getting oracle-attested position)
2. Immediately redeem (get position NFT)
3. Remove liquidity from the position

**Recommended Fix:** Add `require!(lock_duration > 0, ...)` in `initialize`, `update_config`, and `configure_level`.

---

### [M-4] Double-Evolution Prevention Relies on is_evolved Field Which Is Only Set on TARGET Custody

**Severity:** MEDIUM
**File:** `programs/lp-bonds-evolution/src/lib.rs` lines 1009-1010 (`validate_source_custody`)

**Description:** The double-evolution check `require!(!custody_ref.is_evolved, EvolutionError::BondAlreadyEvolved)` reads `is_evolved` from the SOURCE custody. But the source custody's `is_evolved` is set to `false` when created at Level 1 (line 643 in lp-bonds lib.rs). The TARGET custody created by evolution has `is_evolved = true` (line 852 in evolution lib.rs).

This means: A Level 1 bond (is_evolved=false) can be evolved to Level 2. The NEW Level 2 custody has is_evolved=true. If someone tries to evolve the Level 2 bond to Level 3, the source custody (Level 2) has is_evolved=true, so the check blocks it.

**Wait -- this is actually correct behavior.** Let me re-trace:
- L1 bond: custody.is_evolved = false. Can evolve to L2. CHECK: passes.
- L2 bond: custody.is_evolved = true. Try to evolve to L3. CHECK: fails (BondAlreadyEvolved).

**This means bonds can only ever be evolved ONCE.** A Level 1 bond can go to Level 2, but a Level 2 bond CANNOT go to Level 3. This seems like a bug -- the system is designed for L1->L2->L3->L4 progression, but the `is_evolved` check blocks anything beyond a single evolution step.

**Impact:** The L1->L2->L3->L4 multi-step evolution path is broken. Only L1->L2 evolution works. L2->L3 and L3->L4 are blocked by the `is_evolved` check.

**Proof of Concept:**
1. User has L1 bond (custody.is_evolved = false)
2. User evolves L1 -> L2 (new L2 custody.is_evolved = true)
3. User tries to evolve L2 -> L3: `validate_source_custody` reads L2 custody, sees `is_evolved = true`, returns `BondAlreadyEvolved` error

**Recommended Fix:** Remove or rethink the `is_evolved` check. The EvolutionRecord PDA (seeded by source_bond_mint) already prevents double-evolution of the SAME bond because `init` on the evolution_record will fail if it already exists. The `is_evolved` flag is redundant and actively harmful for multi-step evolution.

---

### [L-1] Redundant is_signer Check in Both Programs

**Severity:** LOW (Informational)
**File:** `programs/lp-bonds/src/lib.rs` line 282-285
**File:** `programs/lp-bonds-evolution/src/lib.rs` lines 509-512

**Description:** Both `add_liquidity_and_mint_bond` and `evolve_bond` contain explicit `require!(ctx.accounts.user.is_signer, ...)` checks. Since `user` is declared as `Signer<'info>` in the account struct, Anchor already enforces this. The redundant check is harmless defense-in-depth.

**Impact:** None. Code quality issue only.

---

### [L-2] No On-Chain Metadata for Bond NFTs

**Severity:** LOW
**File:** `programs/lp-bonds/src/lib.rs` (bond minting section, lines 603-621)

**Description:** Bond NFTs are minted as bare SPL tokens with supply=1 and decimals=0. No Metaplex Token Metadata is created. This means bond NFTs have no on-chain name, symbol, image, or attributes. They will not be properly displayed in wallets or marketplaces.

The EVM version stores bond metadata via ERC721's `tokenURI()` with on-chain SVG generation.

**Impact:** Poor UX. Bond NFTs appear as unknown tokens in wallets. No security impact.

**Recommended Fix:** Add Metaplex `create_metadata_accounts_v3` CPI after minting bond NFTs. Use constants from `constants.rs` (`BOND_NFT_NAME_PREFIX`, `BOND_NFT_SYMBOL`, `BOND_NFT_URI_BASE`).

---

### [L-3] Evolution Program Accepts Potentially Stale Source Custody Data

**Severity:** LOW
**File:** `programs/lp-bonds-evolution/src/lib.rs` lines 971-1032 (`validate_source_custody`)

**Description:** The source custody is an `UncheckedAccount` read from either the base lp-bonds program or the evolution program. The function validates owner, PDA derivation, and deserializes via `PositionCustodyRef`. However, if the base program's PositionCustody struct layout ever changes (fields added/removed/reordered), the `PositionCustodyRef` deserialization could produce incorrect data without failing.

**Mitigating Factor:** Both programs are deployed together and the struct layouts are currently identical. This is a maintenance risk, not an active vulnerability.

---

### [L-4] Treasury Can Be Set to Pubkey::default() in Evolution Program

**Severity:** LOW
**File:** `programs/lp-bonds-evolution/src/lib.rs` lines 228-244 (`update_treasury`)

**Description:** `update_treasury` does not validate that `new_treasury != Pubkey::default()`. A zero-address treasury would cause fee transfers to fail (no matching token account), effectively blocking all evolutions that have `fee_bps > 0`.

**Impact:** Admin misconfiguration could block evolution. Not exploitable by non-admin.

**Recommended Fix:** Add `require!(new_treasury != Pubkey::default(), EvolutionError::TreasuryNotSet)`.

---

### [L-5] LPBondsExchange (Tokenization/Exchange) Missing Entirely

**Severity:** LOW (Feature gap)
**File:** EVM: `evm-contracts/liquidity-bonds-contracts/contracts/tokenization/LPBondsExchange.sol`

**Description:** The EVM system includes an `LPBondsExchange` contract for bond tokenization and trading. No equivalent exists on Solana.

**Impact:** Reduced functionality. Not a security issue. Should be discussed with the team whether this feature is needed for Solana launch.

---

### [I-1] EvolveBond Stack Frame Warnings

**Severity:** INFORMATIONAL
**File:** `programs/lp-bonds-evolution/src/lib.rs` (`EvolveBond` struct)

**Description:** The `EvolveBond` account struct is very large (30+ accounts) and generates BPF linker stack frame warnings. While all accounts are boxed, the Anchor-generated `try_accounts` function may still exceed stack limits at runtime on some execution paths.

**Impact:** Potential runtime failure under certain conditions. Difficult to test without mainnet-like conditions.

---

### [I-2] bond_counter and evolution_counter Can Overflow at u64::MAX

**Severity:** INFORMATIONAL
**File:** `programs/lp-bonds/src/lib.rs` line 625; `programs/lp-bonds-evolution/src/lib.rs` line 873

**Description:** Both counters use `checked_add(1)` which will return an error at `u64::MAX`. This is correct behavior (safe arithmetic), but `u64::MAX = 18,446,744,073,709,551,615` bonds would need to be minted, which is practically impossible.

**Impact:** None in practice.

---

### [I-3] position_mint Is Signer But Not Validated as Uninitialized in lp-bonds

**Severity:** INFORMATIONAL
**File:** `programs/lp-bonds/src/lib.rs` line 1135 (`position_mint: Signer<'info>`)

**Description:** In `AddLiquidityAndMintBond`, `position_mint` is a `Signer<'info>` but there's no explicit `data_is_empty()` check like in the evolution program (line 669-672). However, since `position_mint` is passed to the Whirlpool `open_position` CPI which initializes the mint, the Whirlpool program itself will reject an already-initialized mint.

**Mitigating Factor:** The Whirlpool program provides the guard. The evolution program's explicit check is defense-in-depth.

---

## PART 3: Attack Scenario Results

### 1. FAKE BOND ATTACK -- NOT VULNERABLE

**Can an attacker mint a bond NFT without depositing liquidity?**

No. The bond mint authority is the `bond_authority` PDA (seeds: `["bond_authority"]`), and only the program can sign for it. The `mint_to` CPI uses `signer_seeds` derived from the PDA. An attacker cannot forge PDA signatures.

The bond mint is created fresh (`init` constraint in `AddLiquidityAndMintBond.bond_mint`) with `mint::authority = bond_authority`. An attacker cannot pass a pre-existing mint because `init` requires the account to be uninitialized.

**Can they pass a fake bond_mint?** No -- the bond mint is initialized by the instruction itself with Anchor's `init` constraint.

### 2. DOUBLE EVOLUTION ATTACK -- NOT VULNERABLE (but see M-4 for over-restriction)

**Can an attacker evolve the same bond twice?**

No, for two reasons:
1. `EvolutionRecord` PDA is seeded by `source_bond_mint` and uses `init`. Attempting to evolve the same source bond again would fail because the PDA already exists.
2. The source bond NFT is burned during evolution (Step 1 of `evolve_bond`). After burn, the bond no longer exists to evolve.

**Can they evolve a bond they don't own?**

No. The `user_source_bond_account` constraint checks `owner == user.key()` and `amount == 1`.

### 3. ORACLE REPLAY ATTACK -- NOT VULNERABLE

**Can an attacker replay a previous oracle signature?**

No. Strict sequential nonce (`current_nonce + 1`) prevents replay. Each nonce can only be used once.

**Cross-program replay?** Prevented by `contract_address` field in the oracle message (set to `crate::ID`). A signature for the lp-bonds program cannot be used in the evolution program and vice versa.

**Cross-instruction replay?** Prevented by domain separators: `ORACLE_DOMAIN_MINT` vs `ORACLE_DOMAIN_VERIFY` vs `EVOLUTION_SIGNATURE_DOMAIN`. All three are different 18-byte prefixes.

**Cross-user replay?** Prevented by `sender` field in the oracle message, which must match the transaction signer.

**Stale signature?** Prevented by 60-second timestamp staleness check.

### 4. LIQUIDITY THEFT ATTACK -- NOT VULNERABLE

**Can an attacker redeem a bond they don't own?**

No. `RedeemBond.user_bond_account` requires `owner == user.key()`, `mint == bond_mint.key()`, and `amount == 1`.

**Redeem before lock expires?**

No. `custody.is_lock_expired(current_time)` check using `Clock::get()?.unix_timestamp`. The clock sysvar is validated by Anchor.

**Drain custody position token account?**

No. The custody position token account is owned by `position_custody` PDA. Only the program (via PDA signer seeds) can transfer from it, and only in `redeem_bond` after all checks pass.

### 5. FEE MANIPULATION ATTACK -- NOT VULNERABLE

**Can an attacker pass a treasury token account for a different mint?**

No. `EvolveBond.treasury_token_account` has constraint `mint == token_mint_a.key()`.

**Set fee_bps to 0?**

Only admin can call `configure_level`. If admin sets `fee_bps = 0`, fees are zero -- this is intentional behavior.

**Manipulate fee calculation?**

No. `calculate_fee` uses `checked_mul` and `checked_div` with overflow checks, and validates `fee <= u64::MAX` before casting.

### 6. ADMIN IMPERSONATION ATTACK -- NOT VULNERABLE

**Can an attacker call admin functions without being admin?**

No. All admin instructions use `constraint = admin.key() == config.admin` with `Signer<'info>`.

**Intercept pending admin transfer?**

No. `accept_admin` requires the `new_admin` to be `Signer<'info>` and match `config.pending_admin`.

### 7. WHIRLPOOL MANIPULATION ATTACK -- NOT VULNERABLE

**Can an attacker pass a fake whirlpool account?**

No. `Whirlpool::from_account_info` checks owner == `WHIRLPOOL_PROGRAM_ID` (hardcoded constant) AND validates the Anchor discriminator. The whirlpool key is also checked against `config.allowlisted_whirlpool`.

**Manipulate tick arrays?**

No. Tick arrays are validated via: (1) owner == `WHIRLPOOL_PROGRAM_ID`, (2) PDA derivation from whirlpool + start_tick_index, (3) tick coverage bounds.

**Pass vault accounts for a different pool?**

No. Token vaults are cross-validated against the deserialized whirlpool state: `token_vault_a == whirlpool_state.token_vault_a` and `token_vault_b == whirlpool_state.token_vault_b`.

### 8. FRONT-RUNNING ATTACK -- NOT VULNERABLE

**Can an attacker front-run add_liquidity_and_mint_bond?**

Not meaningfully. The oracle signature is bound to the specific user (sender field), specific nonce (sequential), and specific program (contract_address). An attacker cannot use someone else's oracle signature.

**Front-run evolve_bond?**

Same protections apply. The oracle signature is user-specific and nonce-specific.

**Pre-create ATA accounts with malicious data?**

ATAs created via `init_if_needed` are safe because Anchor validates the ATA derivation (mint + authority). A pre-created ATA with wrong mint or authority would fail the constraint.

### 9. STUCK FUNDS ATTACK -- PARTIAL VULNERABILITY (see H-1)

**Are there code paths where tokens become permanently stuck?**

Yes -- accumulated Whirlpool trading fees on custodied positions cannot be collected (see H-1). This is the only stuck funds scenario.

Position NFTs themselves are recoverable via `redeem_bond` / `redeem_evolved_bond` after lock expiry.

Residual tokens after `increase_liquidity` in the evolution program are properly handled (returned to user or burned -- Step 8.5).

**Permanent rent loss?**

No. `close = user` on `position_custody` in `RedeemBond` and `RedeemEvolvedBond` returns rent. `close_orphaned_custody` handles orphaned custodies. Nonce accounts can be closed via `close_nonce_account` / `close_evolution_nonce`.

### 10. RECOVERY ABUSE ATTACK -- PARTIAL VULNERABILITY (see M-1)

**Can admin abuse recover_tokens to steal user funds?**

In lp-bonds: Low risk. `recover_tokens` requires `source_token_account.owner == bond_authority.key()`. Custody position token accounts are owned by `position_custody` PDA, not `bond_authority`. Admin cannot access custody accounts via this instruction.

In lp-bonds-evolution: **YES** (see M-1). `recover_tokens` requires `source_token_account.owner == layer_token_authority.key()`. Custody position token accounts in the evolution program ARE owned by `layer_token_authority`. Admin could drain active position NFTs.

---

## PART 4: Solana-Specific Vulnerability Classes

### 1. ACCOUNT CONFUSION -- NOT VULNERABLE

All accounts have appropriate type constraints. `Account<'info, T>` enforces discriminator checks. `UncheckedAccount`s are manually validated (whirlpool via discriminator + owner check, tick arrays via PDA derivation + owner check, source custody in evolution via owner + PDA + deserialization).

### 2. PDA COLLISION -- NOT VULNERABLE

- `ProtocolConfig`: `["config"]` -- singleton, no collision possible
- `PositionCustody`: `["position_custody", bond_mint]` -- unique per bond mint (bond mints are unique keypairs)
- `OracleConfig`: `["oracle_config"]` -- singleton
- `NonceAccount`: `["nonce", user]` -- unique per user
- `EvolutionRecord`: `["evolution_record", source_bond_mint]` -- unique per source bond
- `LevelConfig`: `["level_config", level_id]` -- unique per level (level_id is u8)
- `AuthorityWhitelist`: `["authority_whitelist", authority]` -- unique per authority

No collision risk identified. All seed inputs are either singletons or include unique identifiers.

### 3. SYSVAR ABUSE -- NOT VULNERABLE

Instructions sysvar is validated by `address = anchor_lang::solana_program::sysvar::instructions::ID`. Clock sysvar is accessed via `Clock::get()` which reads from the sysvar cache. Rent sysvar is `Sysvar<'info, Rent>` which Anchor validates.

### 4. CPI REENTRANCY EQUIVALENT -- NOT VULNERABLE

State is written AFTER CPIs in most cases. Specifically:
- Bond minting: `position_custody` is initialized after all CPIs (open_position, increase_liquidity, transfer)
- Evolution: `position_custody` and `evolution_record` are initialized after all CPIs
- Nonce is committed after oracle verification

The Whirlpool program cannot call back into these programs because it has no mechanism to do so.

### 5. TRANSACTION SIMULATION ATTACKS -- NOT VULNERABLE

All state-changing instructions modify on-chain state. `verify_collateral` commits a nonce, preventing simulation-then-execute attacks. No view-like instructions that leak exploitable information.

### 6. ACCOUNT DATA REUSE -- LOW RISK

After `close = user` on PositionCustody, the account data is zeroed and lamports returned. Anchor's `close` constraint handles this correctly. Subsequent attempts to use the closed PDA would fail because the account has zero lamports and no data.

However, if someone recreates the PDA (by creating a new bond with the same bond_mint key), this is impossible because bond_mint is created via `init` with a fresh keypair each time.

### 7. COMPUTE BUDGET MANIPULATION -- NEEDS REVIEW

The `evolve_bond` instruction is very compute-heavy (30+ accounts, multiple CPIs, validation logic). The BPF stack frame warnings suggest it may approach compute limits. An attacker could not exploit this to drain funds, but could potentially DoS specific evolution attempts by crafting transactions that exhaust compute budget.

### 8. ANCHOR DISCRIMINATOR ATTACKS -- NOT VULNERABLE

All `Account<'info, T>` types automatically verify discriminators. For `UncheckedAccount`s:
- Whirlpool: discriminator manually checked in `from_account_info`
- Source custody in evolution: deserialized via `PositionCustodyRef` which includes discriminator check (via Anchor's `AnchorDeserialize`)

Wait -- `PositionCustodyRef` is NOT an Anchor `#[account]` type. It's a plain `AnchorSerialize/Deserialize` struct. The deserialization in `validate_source_custody` skips 8 bytes for discriminator but does NOT validate the discriminator value. An account with any 8-byte prefix followed by valid field data would pass.

**However**, the PDA derivation check + owner check (must be lp-bonds or evolution program) provides equivalent protection. An account at the correct PDA address owned by the correct program will have the correct discriminator (because only the program can create accounts at that PDA).

---

## PART 5: Previous Fix Verification

### Fix: [C-2] Oracle authority cannot be set to Pubkey::default()
**Status: VERIFIED**
- `initialize_oracle` at lib.rs line 748: `require!(oracle_authority != Pubkey::default(), ...)`
- `update_oracle_authority` at lib.rs line 770: `require!(new_authority != Pubkey::default(), ...)`
- `initialize_evolution` at evolution lib.rs line 53: `require!(oracle_authority != Pubkey::default(), ...)`
- `update_oracle` at evolution lib.rs line 251: `require!(new_oracle != Pubkey::default(), ...)`

### Fix: [M-01/H-2] PositionCustody closed on redeem_bond
**Status: VERIFIED**
- `RedeemBond.position_custody` at lib.rs line 1295: `close = user` present in constraints

### Fix: [H-5] Residual tokens returned after evolve_bond
**Status: VERIFIED**
- Evolution lib.rs lines 788-818: After increase_liquidity, program_token_a_account reloaded and remaining balance transferred to user. program_token_b_account reloaded and remaining layer tokens burned.

### Fix: [H-3] redeem_evolved_bond added
**Status: VERIFIED**
- Evolution lib.rs lines 898-960: Full implementation with pause check, lock expiry, bond burn, position transfer via layer_token_authority PDA, BondRedeemed event, close = user on custody.

### Fix: [M-1] Oracle enabled flag for evolution
**Status: VERIFIED**
- `EvolutionConfig.oracle_enabled` field exists in state.rs line 29
- Checked in evolve_bond at line 514-517
- `set_oracle_enabled` instruction at lines 268-278

### Fix: [M-2] calculate_fee overflow protection
**Status: VERIFIED**
- state.rs line 107: `require!(fee <= u64::MAX as u128, ...)` before cast

### Fix: [M-3] AuthorityWhitelist permissions enforced via configure_level_delegated
**Status: VERIFIED**
- Evolution lib.rs lines 151-205: `configure_level_delegated` with permission check at line 164-167

### Fix: [Issue 1] validate_source_custody level validation
**Status: VERIFIED**
- Evolution lib.rs lines 1026-1030: Strict bounds check `source_level >= MIN_BOND_LEVEL && source_level <= MAX_BOND_LEVEL`

### Fix: VerifyCollateral bond NFT ownership
**Status: VERIFIED**
- lib.rs lines 1407-1413: `sender_bond_account` with owner, mint, and amount constraints

### Fix: verify_collateral uses custody.whirlpool instead of config
**Status: VERIFIED**
- lib.rs lines 909-930: Reads from `custody.whirlpool` and validates against on-chain whirlpool state

### Fix: [L-1] Oracle admin uses protocol config admin
**Status: VERIFIED**
- `UpdateOracleAuthority` struct at lines 1338-1348: checks `admin.key() == config.admin` with config PDA included

### Fix: [L-3] Whirlpool discriminator verification
**Status: VERIFIED**
- whirlpool_cpi.rs lines 44, 57-62: `WHIRLPOOL_DISCRIMINATOR` constant and validation in `from_account_info`
- Same in evolution's whirlpool_cpi.rs

### Fix: [L-5] evolve_bond remaining_accounts == 4 (not >= 4)
**Status: VERIFIED**
- Evolution lib.rs line 444: `ctx.remaining_accounts.len() == 4`

### Fix: [L-6] Nonce accounts can be closed
**Status: VERIFIED**
- `close_nonce_account` at lib.rs lines 818-821 with `CloseNonceAccount` struct
- `close_evolution_nonce` at evolution lib.rs lines 357-362 with `CloseEvolutionNonce` struct

### Fix: [Issue 7] Emergency token recovery
**Status: VERIFIED**
- `recover_tokens` in both programs with admin check, event emission, and owner constraint on source account

### Fix: [M-05] Removed redundant approve/revoke
**Status: VERIFIED**
- Evolution lib.rs: No `token::approve` calls present. `increase_liquidity` uses `layer_token_authority` as position_authority directly via invoke_signed.

### Fix: [M-02/M-03] close_orphaned_custody
**Status: VERIFIED**
- Both programs: `close_orphaned_custody` with `bond_mint.supply == 0` constraint and `close = admin`

### Fix: [L-06] Oracle toggle for base lp-bonds
**Status: VERIFIED**
- lib.rs lines 788-798: `set_oracle_enabled` instruction with `SetOracleEnabled` struct

---

## PART 6: Final Risk Assessment

### Must Fix Before Mainnet (Blockers)

1. **[M-4] is_evolved Flag Blocks Multi-Step Evolution (L2->L3->L4)**
   - File: `programs/lp-bonds-evolution/src/lib.rs` line 1010
   - The `is_evolved = true` flag on target custody prevents further evolution
   - Either remove the `is_evolved` check (EvolutionRecord's `init` already prevents double-evolution of same source bond) or change the flag semantics
   - **Without this fix, only L1->L2 evolution works. L2->L3 and L3->L4 are broken.**

2. **[M-1] Evolution recover_tokens Can Drain Active Custody Positions**
   - File: `programs/lp-bonds-evolution/src/lib.rs` lines 364-395
   - Add protection against recovering from custody position token accounts
   - Suggested: require passing the associated bond_mint and verify `bond_mint.supply == 0`

### Should Fix Before Mainnet (Strongly Recommended)

3. **[H-1] No Fee Collection Instruction**
   - Files: Both programs' `lib.rs`
   - Users lose all trading fees accumulated during lock period
   - Add `collect_fees` instruction using custody PDA as signer

4. **[M-3] No lock_duration > 0 Validation**
   - Files: Both programs' initialization/configuration instructions
   - Add minimum duration check to prevent misconfiguration

5. **[L-4] Treasury Can Be Set to Zero Address**
   - File: `programs/lp-bonds-evolution/src/lib.rs` line 232
   - Add `!= Pubkey::default()` check

### Can Fix Post-Launch (Acceptable Risk)

6. **[L-2] No On-Chain Metadata for Bond NFTs**
   - UX issue only, no security impact
   - Can add Metaplex metadata in a future update

7. **[L-5] LPBondsExchange Not Implemented**
   - Feature gap, discuss with team
   - Not required for core bond functionality

8. **[H-2] recover_tokens Lacks Explicit NFT Guard in lp-bonds**
   - Low probability (custody owned by different PDA)
   - Add comment or explicit check as time permits

### Missing EVM Features That Must Be Discussed With Team

1. **Batch Minting** (`_numberOfBonds` parameter) -- Not feasible on Solana due to tx size limits. Acceptable trade-off.
2. **Multiple Whirlpool Support at L1** -- Currently single pool per deployment. Consider multi-pool architecture if needed.
3. **Operator Registry / Transfer Restrictions** -- EVM has `validateTransfer` modifier. Solana bonds are freely transferable SPL tokens. Decide if transfer restrictions are needed.
4. **LPBondsExchange** -- Entire tokenization/exchange contract missing. Decide if needed for Solana launch.
5. **ETH/SOL Recovery** -- EVM has `recoverEth`. No native SOL recovery on Solana. Low priority.
6. **Collect Fees During Lock** -- EVM positions can have fees collected by the locker. Solana has the CPI wrapper but no instruction to call it.

---

*End of Report*
