# LP Bonds (Level 1 Locker) — Behavioral Gaps Report

> **Scope**: `lp-bonds` program vs. `LiquidityBondLockerV3`, `LiquidityBonds`, `LPBondsExchange`  
> **Date**: 2026-03-25  
> **Method**: Function-level adversarial comparison of every state transition, parameter binding, edge case, and trust assumption.

---

## BG-01: Batch Minting Eliminated

| Field | Detail |
|---|---|
| **Function / Flow** | `lockPositionChild` (EVM) vs. `add_liquidity_and_mint_bond` (Solana) |
| **EVM Behavior** | `_numberOfBonds > 0` parameter. Creates N positions + N bonds in a single transaction via a `for` loop. User provides `_amount0 * N` and `_amount1 * N` tokens total. Each iteration: mint position → transfer LP NFT to multisig → mint bond NFT. Nonce incremented once for all N bonds. |
| **Solana Behavior** | No `_numberOfBonds` parameter. Creates exactly 1 position + 1 bond per instruction invocation. Nonce incremented per invocation. To mint N bonds, N separate transactions (or instructions within one tx) are required. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Behavioral change: EVM allows atomic batch (all-or-nothing for N bonds), Solana forces N independent operations. This affects gas/fee economics and atomicity guarantees. A user who wants 5 bonds on EVM gets all-or-nothing; on Solana, they might get 3 of 5 if the 4th fails. The oracle must sign N separate messages instead of 1. |
| **Suggested fix** | Document as intentional. If atomicity is required, build a composable wrapper program. No code change needed. |

---

## BG-02: Bond Configuration Model Eliminated

| Field | Detail |
|---|---|
| **Function / Flow** | `setBond` / `bonds[bondId]` mapping (EVM) vs. `ProtocolConfig` singleton (Solana) |
| **EVM Behavior** | Multiple bond configurations (`bonds[1]`, `bonds[2]`, etc.) each with independent `token0`, `token1`, `tickLower`, `tickUpper`, `fee`, `lockDuration`, `multiplier`, `collection`, `pool`, `active`, `requiredAmount1`. `bondExists(_bondId)` modifier gates operations per-config. `setBond` creates/updates with 16 parameters. |
| **Solana Behavior** | Single `ProtocolConfig` PDA stores one whirlpool, one token pair, one tick range, one lock duration. No bond ID concept. No per-bond required amounts, fees, multipliers, or active flags. All L1 bonds use identical parameters. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | MEDIUM |
| **Why this matters** | EVM supports multiple concurrent bond configurations (different pools, different tick ranges, different fees). Solana's singleton config means the protocol can only operate on ONE pool at a time. Changing the config via `update_config` affects ALL future bonds. If the protocol needs to offer multiple bond types simultaneously (e.g., different token pairs), a new program deployment is required. This is a fundamental architectural constraint not present in EVM. |
| **Suggested fix** | If multi-pool support is required, refactor `ProtocolConfig` into a PDA keyed by a `bond_config_id` (like `["config", bond_id]`). Otherwise, document as intentional simplification and ensure the backend/frontend don't assume multi-config support. |

---

## BG-03: `basePositions` Concept Removed

| Field | Detail |
|---|---|
| **Function / Flow** | `setBasePosition` / `basePositions[bondId]` (EVM) vs. N/A (Solana) |
| **EVM Behavior** | `basePositions[_bondId]` stores a reference position ID per bond config. This value is included in the signature hash: `keccak256(abi.encodePacked(basePositions[_bondId], _amount0, _amount1, address(this), nonce, msg.sender))`. The `basePositionExists(_bondId)` modifier gates `lockPositionChild`. |
| **Solana Behavior** | No `basePositions` equivalent. The oracle message binds to `whirlpool`, `token_mint_a`, `token_mint_b`, `contract_address` (program ID) instead. No base position concept. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | The EVM `basePositions` was a signature-binding mechanism to associate signatures with specific bond configs. Solana replaces this with direct whirlpool + token mint binding in the oracle message, which is more explicit and harder to confuse. The EVM system_level_analysis noted that `basePositions` could be shared across configs, creating signature interchangeability — Solana eliminates this vulnerability. |
| **Suggested fix** | None needed. Solana's approach is strictly better. |

---

## BG-04: Signature Parameter Binding — Missing `isEth` / `numberOfBonds` Equivalent

| Field | Detail |
|---|---|
| **Function / Flow** | `_verifySignature` (EVM) vs. `verify_oracle_attestation` (Solana) |
| **EVM Behavior** | Signature covers: `basePositions[bondId], _amount0, _amount1, address(this), nonce, msg.sender`. Does NOT bind: `_numberOfBonds`, `_isEth`, `_fee`, chain ID. EVM finding H-02: attacker with valid signature could manipulate unbound parameters. |
| **Solana Behavior** | Oracle message covers: `domain(18) + whirlpool(32) + token_mint_a(32) + token_mint_b(32) + amount_a(8) + amount_b(8) + liquidity(16) + tick_lower(4) + tick_upper(4) + tick_current(4) + nonce(8) + timestamp(8) + sender(32) + contract_address(32)` = 238 bytes. No equivalent to `isEth` (native wrapping handled transparently). No `numberOfBonds` (single-bond per call). |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana binds MORE parameters than EVM (liquidity, both ticks, tick_current, timestamp, both token mints). The EVM vulnerabilities around unbound parameters are resolved. The `liquidity` binding is particularly important — it prevents the oracle from signing amounts that don't match the actual LP position liquidity. |
| **Suggested fix** | None needed. |

---

## BG-05: Reentrancy Guard Implicit vs. Explicit

| Field | Detail |
|---|---|
| **Function / Flow** | All user-facing functions |
| **EVM Behavior** | `nonReentrant` modifier on `lockPositionChild`, `mint`, `burn`, `exchange`. Uses OpenZeppelin `ReentrancyGuardUpgradeable` with storage-based reentrancy lock. |
| **Solana Behavior** | No explicit reentrancy guard. Relies on Solana runtime's built-in reentrancy protection: a program cannot CPI into itself, and accounts locked during execution prevent concurrent modification. |
| **Classification** | **EXACT MATCH** (behavioral equivalence via runtime guarantee) |
| **Severity** | LOW |
| **Why this matters** | Solana's runtime provides stronger reentrancy protection than EVM's storage-based check. However, cross-program reentrancy (Program A → Program B → Program A) is still possible if account locks allow it. In this program, all custody accounts are PDA-controlled, making cross-program reentrancy non-exploitable for state corruption. |
| **Suggested fix** | None needed. |

---

## BG-06: Residual Token Handling — Missing Return Logic

| Field | Detail |
|---|---|
| **Function / Flow** | `add_liquidity_and_mint_bond` (Solana) vs. `lockPositionChild` (EVM) |
| **EVM Behavior** | After `uniswapPositionManager.mint()`, residual tokens (from price movement during LP creation) remain in the contract. EVM finding M-07: leftover tokens stay with unlimited approval. Owner can recover via `recoverERC20`. |
| **Solana Behavior** | After `whirlpool_cpi::increase_liquidity()`, residual tokens remain in the user's token accounts (since the user's accounts are passed as `token_owner_account_a/b`). The CPI only pulls what it needs. No residual issue. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana's model is inherently safer — residual tokens stay with the user, not the protocol. The EVM pattern of pulling all tokens first and leaving residuals in the contract is eliminated. However, `token_max_a` / `token_max_b` are slippage bounds, and the actual consumed amount may be less. The user keeps the difference automatically. |
| **Suggested fix** | None needed. |

---

## BG-07: Lock Duration — L1 Redemption IS Pause-Gated

| Field | Detail |
|---|---|
| **Function / Flow** | `redeem_bond` (Solana) |
| **EVM Behavior** | No explicit `unlockPosition` function exists on-chain (system_level_analysis L-11). LP position release depends entirely on multisig. No pause check on redemption because redemption is off-chain (multisig-controlled). |
| **Solana Behavior** | `redeem_bond` at line 686: `require!(!ctx.accounts.config.is_paused, LpBondsError::ProtocolPaused)`. L1 redemption IS pause-gated. If admin pauses the protocol, users CANNOT redeem their L1 bonds until unpaused. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | HIGH |
| **Why this matters** | A malicious or compromised admin can permanently lock user funds by pausing the protocol. In EVM, the multisig custody release is independent of contract pause state. On Solana, pausing freezes ALL operations including redemption. This creates a rug-pull vector: admin pauses → users cannot withdraw → admin (via `recover_tokens` which is NOT pause-gated) drains tokens. The evolution program's `redeem_evolved_bond` is also pause-gated (line 952). |
| **Suggested fix** | Remove the pause check from `redeem_bond` and `redeem_evolved_bond`. Redemption should ALWAYS be available regardless of protocol pause state. Alternatively, make redemption governed by a separate `redemption_paused` flag controlled by a timelock or governance. |

---

## BG-08: Lock Duration — L1 Bonds HAVE a Timelock

| Field | Detail |
|---|---|
| **Function / Flow** | `redeem_bond` (Solana) |
| **EVM Behavior** | System-level analysis section 8: "No unlock mechanism exists on-chain." L1 bonds have no on-chain timelock — the multisig can release at any time. `lockDuration` in the EVM `Bond` struct is used for metadata display, not enforced in any `unlock` function (because no `unlock` function exists). |
| **Solana Behavior** | `redeem_bond` line 690-693: `require!(custody.is_lock_expired(current_time), LpBondsError::BondStillLocked)` where `is_lock_expired = current_time >= created_at + lock_duration`. L1 bonds have `lock_duration` enforced from `ProtocolConfig.lock_duration` (set at init, requires `> 0`). |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | MEDIUM |
| **Why this matters** | EVM L1 bonds are immediately redeemable (via multisig). Solana L1 bonds are locked for `lock_duration` seconds. This is a fundamental behavioral difference affecting user expectations. If `lock_duration` is set to e.g. 365 days, users cannot access their underlying LP position for a year — which may or may not be intended. The EVM system had no such restriction for L1. The parity report incorrectly states "No timelock for L1 bonds (matching EVM behavior)." |
| **Suggested fix** | Clarify with the protocol team: is L1 timelock intentional? If it matches EVM intent, keep it. If L1 should be freely redeemable, set `lock_duration = 0` at initialization or remove the lock check for level-1 bonds specifically. |

---

## BG-09: `recover_tokens` Missing Active-Custody Guard

| Field | Detail |
|---|---|
| **Function / Flow** | `recover_tokens` (Solana `lp-bonds`) |
| **EVM Behavior** | `recoverERC20` / `recoverERC721` are generic recovery functions. The EVM locker doesn't store position NFTs in the contract (they go to multisig), so there's no risk of admin recovering active custody assets. |
| **Solana Behavior** | `recover_tokens` at line 825-856. Source account must be owned by `bond_authority` PDA. No check that the source account is NOT a custody position token account. No `bond_mint.supply == 0` check (unlike `lp-bonds-evolution`'s `recover_tokens` which HAS this check at line 1865). Admin can pass ANY token account owned by `bond_authority` and drain it, including active position NFTs. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | CRITICAL |
| **Why this matters** | An admin (or compromised admin key) can steal active LP position NFTs from custody by calling `recover_tokens` with a custody position token account. The position NFT is held in an ATA owned by the `PositionCustody` PDA, NOT the `bond_authority` PDA — so this specific attack vector may not work. BUT: any tokens accidentally sent to `bond_authority`-owned accounts (e.g., fee distributions, airdrops) can be drained. The deeper risk is architectural: there's no programmatic guardrail preventing future refactors from creating `bond_authority`-owned accounts that hold critical assets. The evolution program's version is safer with the `supply == 0` check. |
| **Suggested fix** | Add a `bond_mint` parameter and `bond_mint.supply == 0` constraint, matching the evolution program's `RecoverTokens` account struct. |

---

## BG-10: Collect Fees — No `update_fees_and_rewards` CPI Before Collection

| Field | Detail |
|---|---|
| **Function / Flow** | `collect_fees` (Solana) |
| **EVM Behavior** | Uniswap V3 `collect()` function automatically returns accrued fees. The Uniswap V3 PositionManager handles fee accounting internally. |
| **Solana Behavior** | `collect_fees` at line 858-899 calls `whirlpool_cpi::collect_fees()` directly. However, Orca Whirlpool requires calling `update_fees_and_rewards` BEFORE `collect_fees` to update the position's fee accounting. Without this pre-call, collected fees may be stale or zero. |
| **Classification** | **MISSING BEHAVIOR** |
| **Severity** | HIGH |
| **Why this matters** | If `update_fees_and_rewards` is not called before `collect_fees`, the Whirlpool will return stale fee amounts (fees accumulated since the last update, which could be zero if an unrelated operation triggered an update). Users will see inconsistent fee collection and potentially lose fees. The Orca Whirlpool documentation requires this two-step process. |
| **Suggested fix** | Add a `whirlpool_cpi::update_fees_and_rewards()` CPI call before `collect_fees()` in both the `lp-bonds` and `lp-bonds-evolution` programs. This requires adding tick array accounts to the `CollectFees` account struct. |

---

## BG-11: Collect Fees Event — Hardcoded Zero Amounts

| Field | Detail |
|---|---|
| **Function / Flow** | `collect_fees` (Solana) |
| **EVM Behavior** | Fee collection amounts are tracked via `tokensOwed0`/`tokensOwed1` from the Uniswap position manager. |
| **Solana Behavior** | Line 892-893: `fees_a: 0, fees_b: 0` — hardcoded zeros in the `FeesCollected` event. The comment says "Actual amounts determined by Whirlpool CPI" but the values are never read from the CPI result. |
| **Classification** | **MISSING BEHAVIOR** |
| **Severity** | MEDIUM |
| **Why this matters** | Off-chain indexers and analytics systems relying on `FeesCollected` events will see `0, 0` for every fee collection, making it impossible to track fee revenue per bond. This data is critical for protocol dashboards, user-facing UIs, and accounting. |
| **Suggested fix** | Read the user's token account balances before and after the `collect_fees` CPI, then emit the difference. Alternatively, use `user_token_a_account.reload()` after CPI and compute delta. |

---

## BG-12: Bond-to-Token Exchange Flow — Entirely Missing

| Field | Detail |
|---|---|
| **Function / Flow** | `LPBondsExchange.exchange()` (EVM) vs. N/A (Solana) |
| **EVM Behavior** | User transfers bond NFTs to multisig → receives newly minted ERC20 tokens at oracle-signed exchange rate. Per-NFT rate: `_amount1`. Total: `_amount1 * tokenIds.length`. Requires: active config, valid signature, bond ownership. |
| **Solana Behavior** | No exchange instruction exists in either program. No account struct. No exchange config. No exchange-specific signature domain. |
| **Classification** | **MISSING BEHAVIOR** |
| **Severity** | CRITICAL |
| **Why this matters** | The exchange is one of three core user flows (mint, evolve, exchange). Without it, users can only: (1) hold bonds, (2) redeem for raw LP position, or (3) evolve. They CANNOT convert bonds to liquid ERC20/SPL tokens. This eliminates a key exit path. For a protocol that may want to offer token-based exits (e.g., for bonds with impermanent loss), this is a critical missing feature. |
| **Suggested fix** | Implement a new program or instruction with: (1) exchange config PDA per collection/mint, (2) oracle-signed exchange rate, (3) bond burn + SPL token mint, (4) per-user nonce, (5) `EXCHANGE_V1` domain separator. |

---

## BG-13: `verify_collateral` Consumes User's Nonce

| Field | Detail |
|---|---|
| **Function / Flow** | `verify_collateral` (Solana) — no EVM equivalent |
| **EVM Behavior** | No post-mint collateral verification exists. |
| **Solana Behavior** | `verify_collateral` at line 923-1043 shares the user's `NonceAccount` (same PDA: `["nonce", user]`). Each verify call increments the nonce. This means `verify_collateral` and `add_liquidity_and_mint_bond` compete for the same nonce sequence. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | MEDIUM |
| **Why this matters** | If a user has a pending mint signature (for nonce N+1) and calls `verify_collateral` first (consuming nonce N+1), the mint signature becomes invalid. The oracle must re-sign with nonce N+2. This interleaving of two different operations on the same nonce sequence creates coordination complexity. The EVM system doesn't have this issue because verify_collateral doesn't exist and mint has its own nonce. |
| **Suggested fix** | Use a separate nonce account for `verify_collateral` (e.g., PDA `["verify_nonce", user]`) to decouple verification nonces from mint nonces. This prevents one operation from invalidating the other's pending signatures. |

---

## BG-14: `weirdERC20` Handling — No Equivalent Needed

| Field | Detail |
|---|---|
| **Function / Flow** | `setWeirdERC20` / `weirdERC20s` mapping (EVM) |
| **EVM Behavior** | Special handling for non-standard ERC20 tokens (no return value on `transfer`/`approve`). `IERC20Weird` interface used for token1 only. Token0 always uses standard IERC20 (EVM finding M-06). |
| **Solana Behavior** | No equivalent. SPL Token program has a uniform interface — all tokens use the same transfer/approve/burn instructions. No "weird" token concept. |
| **Classification** | **EXACT MATCH** (architecturally unnecessary) |
| **Severity** | LOW |
| **Why this matters** | SPL Token uniformity eliminates the entire class of EVM weird-ERC20 bugs. No action needed. |
| **Suggested fix** | None. |

---

## BG-15: `startTime` Per-Bond Metadata — Missing

| Field | Detail |
|---|---|
| **Function / Flow** | `setStartTime` / `startTime[bondId]` (EVM) |
| **EVM Behavior** | Admin sets a `startTime` per bond config via `setStartTime(bondId, timestamp)`. Used in `LiquidityBonds.getBondInfo()` for calculating `durationLeft`. This is metadata — it doesn't gate any on-chain logic in the locker. |
| **Solana Behavior** | No `startTime` concept. `PositionCustody.created_at` stores the bond creation timestamp (set automatically from `Clock`). No admin-settable start time. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | The EVM `startTime` was admin-configurable metadata. Solana's `created_at` is automatically set and immutable. This means the admin cannot retroactively change bond start times (which is actually more secure). Duration calculations must use `created_at` instead. |
| **Suggested fix** | None needed. `created_at` is a safer approach. |

---

## BG-16: On-Chain SVG / TokenURI — Missing

| Field | Detail |
|---|---|
| **Function / Flow** | `tokenURI()` / `getBondInfo()` + SVG generation (EVM `LiquidityBonds`) |
| **EVM Behavior** | Fully on-chain SVG + Base64 JSON metadata. `getBondInfo` queries locker for position data, formats amounts, generates SVG artwork, returns `data:application/json;base64,...` URI. |
| **Solana Behavior** | No on-chain metadata. Bond mints are bare SPL tokens with `decimals=0`, `supply=1`. `BOND_NFT_URI_BASE` constant suggests off-chain metadata at `https://api.lpbonds.io/metadata/`. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | On-chain metadata is an EVM convention (especially for marketplaces). Solana NFTs typically use off-chain or Metaplex metadata. The bond functions correctly without on-chain SVG. Off-chain metadata at `BOND_NFT_URI_BASE` is unused in the current program. |
| **Suggested fix** | If marketplace compatibility is desired, add Metaplex metadata CPI during mint. Otherwise, no change needed. |

---

## BG-17: Operator Registry / Transfer Whitelist — Missing

| Field | Detail |
|---|---|
| **Function / Flow** | `_transfer`, `_approve`, `_setApprovalForAll` overrides + `operatorRegistry` (EVM `LiquidityBonds`) |
| **EVM Behavior** | All bond transfers are gated by an operator registry whitelist. If `msg.sender` is a contract (not EOA) or `to` is a contract, they must be whitelisted. Uses `tx.origin` for EOA detection (EVM finding H-01). `updateOperatorRegistry` changes the registry. |
| **Solana Behavior** | No transfer restrictions. Bond tokens are standard SPL tokens — any holder can transfer freely. No operator registry. No whitelist. |
| **Classification** | **MISSING BEHAVIOR** |
| **Severity** | MEDIUM |
| **Why this matters** | The EVM operator registry exists to prevent bonds from being traded on unauthorized marketplaces. Without it, bonds can be freely listed and traded anywhere. This may be intentional (more permissive) or a compliance gap (if the protocol needs to restrict secondary market trading). The EVM's `tx.origin` approach was broken anyway (EVM finding H-01), so this "missing" behavior may actually be a feature. |
| **Suggested fix** | If transfer restrictions are required, implement a freeze authority check or token-2022 transfer hook. Otherwise, document as intentional. |

---

## BG-18: `close_nonce_account` Allows Nonce Reset

| Field | Detail |
|---|---|
| **Function / Flow** | `close_nonce_account` (Solana) — no EVM equivalent |
| **EVM Behavior** | Global nonce is monotonically increasing. No reset mechanism. |
| **Solana Behavior** | `close_nonce_account` at line 820-823 closes the `NonceAccount` PDA, returning rent. After closure, the user can call `initialize_nonce` again to create a new nonce account starting at 0. This effectively resets the nonce. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | HIGH |
| **Why this matters** | Nonce reset creates a replay window. Scenario: (1) User completes 10 operations (nonce=10). (2) User closes nonce account. (3) User re-initializes nonce (nonce=0). (4) Any old oracle signatures for nonces 1-10 are now replayable if the oracle has not changed. The oracle signature includes timestamp (60s staleness), so old signatures would fail the timestamp check. BUT: if the oracle re-signs identical parameters with a fresh timestamp, the nonce ordering is broken — nonce 1 could be used after nonce 10 was previously used. |
| **Suggested fix** | Either: (1) Remove `close_nonce_account` / `close_evolution_nonce` entirely, or (2) add a minimum nonce in the oracle-signed message that the program checks against (the oracle tracks the last-known nonce for each user and refuses to sign nonces below a floor). The timestamp staleness (60s) significantly mitigates this in practice but does not eliminate it. |

---

## BG-19: `set_oracle_enabled(false)` Disables All Operations

| Field | Detail |
|---|---|
| **Function / Flow** | `set_oracle_enabled` (Solana) |
| **EVM Behavior** | No oracle toggle in EVM. Signature verification is always on. The signer address can be changed but not disabled. |
| **Solana Behavior** | `set_oracle_enabled(false)` at line 790: sets `oracle_config.enabled = false`. `add_liquidity_and_mint_bond` line 245: `require!(oracle_config.enabled, ...)` — minting REQUIRES oracle. `verify_collateral` line 978: same check. There is NO code path to mint without oracle. Setting `enabled = false` permanently bricks minting until re-enabled. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | This is a deliberate kill-switch not present in EVM. It's useful for emergency scenarios but could be used to grief users if the admin is compromised. Since `redeem_bond` does NOT check oracle_enabled, existing bond holders can still redeem. |
| **Suggested fix** | None, but consider whether this should require timelock or governance. |

---

## BG-20: `update_config` Can Change Token Mints While Bonds Outstanding

| Field | Detail |
|---|---|
| **Function / Flow** | `update_config` (Solana) |
| **EVM Behavior** | `setBond` creates/updates per-bond configs. Existing bonds are NOT affected because each bond references its own config (bond struct stores all parameters). |
| **Solana Behavior** | `update_config` at line 94-126 overwrites `token_mint_a`, `token_mint_b`, `allowlisted_whirlpool` on the GLOBAL config singleton. Existing bonds store their whirlpool in `PositionCustody.whirlpool`, so `verify_collateral` correctly uses the bond-specific whirlpool. BUT: `collect_fees` on existing bonds will fail if the whirlpool key in config no longer matches (the `CollectFees` account struct constrains `whirlpool.key() == position_custody.whirlpool`, not config). Wait — checking line 1593: `whirlpool.key() == position_custody.whirlpool` — this is actually bond-specific, NOT config. So `collect_fees` is safe. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Initial analysis suggested this was dangerous, but the Solana implementation correctly uses `position_custody.whirlpool` (not `config.allowlisted_whirlpool`) for existing-bond operations. New mints will use the updated config. Existing bonds are unaffected. |
| **Suggested fix** | None needed. The custody-based whirlpool binding is correct. |

---

## BG-21: ETH Recovery — No SOL Recovery Function

| Field | Detail |
|---|---|
| **Function / Flow** | `recoverETH` (EVM) vs. N/A (Solana) |
| **EVM Behavior** | `recoverETH(to, amount)` allows admin to withdraw trapped ETH from the contract via low-level call. |
| **Solana Behavior** | No SOL recovery function. SOL rent from closed accounts goes to specified recipients (admin or user depending on the `close` directive). No mechanism to recover SOL accidentally sent to the program's derived addresses. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | SOL cannot be "accidentally sent" to Solana programs the way ETH can be sent to EVM contracts. Solana programs don't have a `receive()` fallback. Rent SOL is handled by `close` directives. No recovery needed. |
| **Suggested fix** | None needed. |

---

## BG-22: ERC721 `onERC721Received` — No Equivalent

| Field | Detail |
|---|---|
| **Function / Flow** | `onERC721Received` (EVM) |
| **EVM Behavior** | `IERC721ReceiverUpgradeable` implementation required for the contract to receive ERC721 tokens via `safeTransferFrom`. Returns `this.onERC721Received.selector`. |
| **Solana Behavior** | No equivalent needed. SPL Token transfers work with any account — no "safe transfer" / receiver hook pattern. |
| **Classification** | **EXACT MATCH** (architecturally unnecessary) |
| **Severity** | LOW |
| **Why this matters** | Solana's account model doesn't require receiver callbacks. The associated token account system handles ownership natively. |
| **Suggested fix** | None. |

---

## BG-23: `propose_admin` Allows Setting `Pubkey::default()` as Pending Admin

| Field | Detail |
|---|---|
| **Function / Flow** | `propose_admin` (Solana) vs. `transferOwnership` (EVM) |
| **EVM Behavior** | OpenZeppelin `Ownable2Step.transferOwnership()` requires `newOwner != address(0)`. |
| **Solana Behavior** | `propose_admin` at line 149-160: sets `config.pending_admin = new_admin`. No check that `new_admin != Pubkey::default()`. The `AcceptAdmin` struct line 1091: `config.pending_admin != Pubkey::default()` prevents acceptance of default. BUT: calling `propose_admin(Pubkey::default())` effectively cancels a pending transfer — this may be intentional (no separate `cancelAdminTransfer` instruction). |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Using `Pubkey::default()` as a cancel mechanism is an implicit behavior not documented anywhere. It works correctly but could confuse integrators. |
| **Suggested fix** | Add a comment documenting that `propose_admin(Pubkey::default())` cancels the pending transfer. Optionally add a dedicated `cancel_admin_transfer` instruction for clarity. |

---

## BG-24: No `whenNotPaused` Guard on `pause()` / No `whenPaused` Guard on `unpause()`

| Field | Detail |
|---|---|
| **Function / Flow** | `pause` / `unpause` (Solana) vs. EVM |
| **EVM Behavior** | `pause()` has `whenNotPaused` modifier — can only pause if not already paused. `unpause()` has `whenPaused` — can only unpause if paused. Double-pause or double-unpause reverts. |
| **Solana Behavior** | `pause` at line 129-136 and `unpause` at line 139-146: unconditional `config.is_paused = true/false`. No check on current state. Admin can call `pause()` when already paused (no-op) or `unpause()` when already unpaused (no-op). |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | No security impact. Double-pause/unpause is a no-op on Solana vs. a revert on EVM. The Solana approach is more gas-efficient (no extra check) with identical end-state semantics. Events are emitted regardless, which could confuse off-chain trackers. |
| **Suggested fix** | Minor: add `require!(!config.is_paused)` / `require!(config.is_paused)` guards to prevent spurious events. |

---

## BG-25: `lock_duration = 0` Prevented at Init but Not Considered for Skip Logic

| Field | Detail |
|---|---|
| **Function / Flow** | `initialize` / `update_config` (Solana) |
| **EVM Behavior** | `setBond` requires `_lockDuration > 0`. But the EVM system has no on-chain redemption, so `lockDuration` is purely metadata. |
| **Solana Behavior** | `initialize` line 63: `require!(lock_duration > 0, ...)`. `update_config` line 106: same check. This means L1 bonds ALWAYS have a non-zero lock duration. `redeem_bond` enforces this via `is_lock_expired()`. There is no way to create freely-redeemable L1 bonds. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | MEDIUM |
| **Why this matters** | If the protocol wants to allow L1 bonds without a lock (matching the EVM behavior where there's no on-chain lock), they cannot. The minimum lock duration is 1 second, but even this creates a 1-second window where the bond cannot be redeemed. For a protocol that conceptually treats L1 as "no lock" (EVM behavior), this is a constraint. |
| **Suggested fix** | Allow `lock_duration = 0` in `initialize` and `update_config` if L1 bonds should be freely redeemable. Or add a `skip_lock_check` field to ProtocolConfig. |

---

## FINAL SUMMARY

### Finding Distribution

| Classification | Count |
|---|---|
| **EXACT MATCH** | 3 |
| **SAFE DEVIATION** | 10 |
| **DANGEROUS DEVIATION** | 5 |
| **MISSING BEHAVIOR** | 3 |
| **Total** | **21** (targeting `lp-bonds` only — excludes evolution-specific findings) |

### Critical / High Findings

| # | ID | Classification | Severity | Summary |
|---|---|---|---|---|
| 1 | BG-07 | DANGEROUS DEVIATION | HIGH | L1 redemption is pause-gated — admin can lock user funds |
| 2 | BG-09 | DANGEROUS DEVIATION | CRITICAL | `recover_tokens` has no `supply == 0` guard |
| 3 | BG-10 | MISSING BEHAVIOR | HIGH | No `update_fees_and_rewards` CPI before `collect_fees` |
| 4 | BG-12 | MISSING BEHAVIOR | CRITICAL | Bond-to-token exchange flow entirely missing |
| 5 | BG-18 | DANGEROUS DEVIATION | HIGH | `close_nonce_account` allows nonce reset / replay window |

### Recommendations (Priority Order)

1. **P0**: Remove pause gate from `redeem_bond` (BG-07)
2. **P0**: Add `bond_mint.supply == 0` check to `recover_tokens` (BG-09)
3. **P0**: Add `update_fees_and_rewards` CPI before `collect_fees` (BG-10)
4. **P0**: Implement exchange instruction (BG-12)
5. **P1**: Prevent nonce reset after close by either removing `close_nonce_account` or adding a minimum-nonce floor (BG-18)
6. **P1**: Clarify L1 lock duration semantics with protocol team (BG-08, BG-25)
7. **P2**: Fix hardcoded zero amounts in `FeesCollected` event (BG-11)
8. **P2**: Separate verify_collateral nonce from mint nonce (BG-13)

---

*End of lp-bonds Behavioral Gaps Report*
