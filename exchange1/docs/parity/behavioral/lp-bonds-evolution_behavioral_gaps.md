# LP Bonds Evolution — Behavioral Gaps Report

> **Scope**: `lp-bonds-evolution` program vs. `LiquidityBondsEvolution`  
> **Date**: 2026-03-25  
> **Method**: Function-level adversarial comparison of every state transition, parameter binding, edge case, and trust assumption.

---

## EBG-01: Layer Configuration Model — Per-Level PDA vs. Nested Mapping

| Field | Detail |
|---|---|
| **Function / Flow** | `setLayer` (EVM) vs. `configure_level` (Solana) |
| **EVM Behavior** | `layers[_origBondId][_layerId]` — two-dimensional mapping. First key is the *original* bond config ID, second is the layer ID. `Layer` struct stores: `bondId` (target bond config), `baseLayer` (NFT collection to burn), `outputLayer` (dead storage), `token` (dead storage), `fee` (basis points). `setLayer` has ZERO validation (EVM finding H-01). |
| **Solana Behavior** | `LevelConfig` PDA seeded by `["level_config", level_id]`. Single dimension keyed by level (2, 3, or 4). Stores: `whirlpool`, `token_mint_a`, `token_mint_b`, `layer_token_mint`, `tick_lower`, `tick_upper`, `required_amount_a`, `required_amount_b`, `fee_bps`, `lock_duration`, `multiplier`, `is_active`. Rich validation: `fee_bps <= MAX_FEE_BPS (5000)`, `lock_duration > 0`, `tick_lower < tick_upper`, bounds checks. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana's model is cleaner — single-key PDA vs. nested mapping. Eliminates EVM's dead storage (`outputLayer`, `token`). Adds all the validation the EVM version lacked. The EVM's `_origBondId` dimension mapped to the Level 1 bond config — Solana doesn't need this because level transitions are validated directly (`target_level == source_level + 1`). |
| **Suggested fix** | None needed. |

---

## EBG-02: Bond Burning — Actual SPL Burn vs. Transfer to `multiSigBurned`

| Field | Detail |
|---|---|
| **Function / Flow** | Base NFT destruction during evolution |
| **EVM Behavior** | `IERC721(layer.baseLayer).transferFrom(_msgSender(), multiSigBurned, _baseTokenId[i])` — NFTs are NOT burned, they are transferred to `multiSigBurned` address. The NFTs continue to exist and could be re-transferred by the multisig. EVM finding C-02: `multiSigBurned` can be `address(0)`. |
| **Solana Behavior** | `token::burn()` at evolution line 721-730: actually burns the bond NFT (supply goes to 0). The SPL `burn` instruction permanently destroys the token. Bond mint supply becomes 0, making the bond irrecoverable. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana's approach is strictly safer — burned bonds cannot be resurrected. EVM's transfer-to-multisig creates a trust risk (multiSig holder could re-issue "burned" bonds). The supply-zero check is then used by `EvolutionRecord` init and `close_orphaned_custody` to verify burns. |
| **Suggested fix** | None needed. |

---

## EBG-03: Evolution Fee — Charged on `amount_a` Only (Not `amount_a * numberOfBonds`)

| Field | Detail |
|---|---|
| **Function / Flow** | Fee calculation during `evolve_bond` |
| **EVM Behavior** | `fee = (_amount0 * numberOfBonds * layer.fee) / 10000`. Fee is proportional to total token0 across ALL bonds in the batch. |
| **Solana Behavior** | `evolve_bond` line 735: `let fee = ctx.accounts.level_config.calculate_fee(amount_a)?` where `calculate_fee`: `(amount as u128 * fee_bps as u128) / 10000`. Fee applies to `amount_a` for a single bond (no batch multiplier). |
| **Classification** | **EXACT MATCH** (single-bond equivalent) |
| **Severity** | LOW |
| **Why this matters** | Since Solana processes one evolution per instruction (no batch), the fee calculation is for a single bond. The per-bond fee formula `(amount_a * fee_bps) / 10000` matches the EVM per-bond formula `(_amount0 * 1 * layer.fee) / 10000`. Behavioral equivalence holds when `numberOfBonds = 1`. |
| **Suggested fix** | None. |

---

## EBG-04: Fee Can Round to Zero — Same as EVM

| Field | Detail |
|---|---|
| **Function / Flow** | `calculate_fee` in `LevelConfig` |
| **EVM Behavior** | EVM finding H-03: `fee = (_amount0 * numberOfBonds * layer.fee) / 10000`. Small amounts round to zero. Fee-free evolution is possible with small deposits. |
| **Solana Behavior** | `calculate_fee`: `(amount as u128 * fee_bps as u128) / 10000`. If `amount_a = 1` and `fee_bps = 1`, fee = `1 * 1 / 10000 = 0` (integer division). No minimum fee enforcement. |
| **Classification** | **EXACT MATCH** (inherits EVM bug) |
| **Severity** | MEDIUM |
| **Why this matters** | An attacker could evolve bonds with dust amounts to pay zero fees. The oracle attestation binds `amount_a`, so the oracle could refuse to sign dust amounts, but this is an off-chain defense. On-chain, there's nothing preventing `fee = 0`. |
| **Suggested fix** | Add `require!(fee > 0, EvolutionError::FeeTooLow)` or set a `min_fee` field in `LevelConfig`. |

---

## EBG-05: Signature Binds `target_level` — Improvement Over EVM

| Field | Detail |
|---|---|
| **Function / Flow** | `_verifySignature` (EVM) vs. `verify_evolution_signature` (Solana) |
| **EVM Behavior** | Signature covers: `basePositions[_bondId], _amount0, _amount1, address(this), nonce, msg.sender`. Does NOT bind `_layerId`. EVM finding H-02: "A signature for one layer can be replayed on another layer of the same bond." |
| **Solana Behavior** | Evolution message (271 bytes) includes: `domain(18) + source_bond_mint(32) + target_level(1) + whirlpool(32) + token_mint_a(32) + token_mint_b(32) + amount_a(8) + amount_b(8) + liquidity(16) + tick_lower(4) + tick_upper(4) + tick_current(4) + nonce(8) + timestamp(8) + sender(32) + contract_address(32)`. `target_level` is explicitly bound. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana resolves the EVM cross-layer replay vulnerability by binding `target_level` and `source_bond_mint` in the signature. A signature for L1→L2 cannot be replayed for L2→L3. |
| **Suggested fix** | None needed. |

---

## EBG-06: `bondExists` Modifier — Wrong Bond Validated in EVM, Fixed in Solana

| Field | Detail |
|---|---|
| **Function / Flow** | Bond configuration lookup during evolution |
| **EVM Behavior** | EVM finding C-03: `bondExists(_bondId)` validates `bonds[_bondId]`, but the actual bond used for LP creation comes from `bonds[layer.bondId]`. If `layer.bondId` differs from `_bondId`, an inactive/non-existent bond could be used. |
| **Solana Behavior** | `LevelConfig` PDA is directly loaded by `target_level` (line 1660: `seeds = [LEVEL_CONFIG_SEED, &[target_level]]`). The level config itself contains all parameters (whirlpool, tick ranges, token mints). No indirection through a separate "bond config." `is_active` check on line 546: `require!(ctx.accounts.level_config.is_active, EvolutionError::LevelNotActive)`. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana eliminates the EVM indirection bug by making `LevelConfig` self-contained. The wrong-bond validation issue cannot occur because there's no intermediate lookup. |
| **Suggested fix** | None needed. |

---

## EBG-07: Evolved Bond Redemption — Pause-Gated

| Field | Detail |
|---|---|
| **Function / Flow** | `redeem_evolved_bond` (Solana) |
| **EVM Behavior** | No on-chain redemption for evolved bonds either. Multisig custody release is off-chain. |
| **Solana Behavior** | `redeem_evolved_bond` line 952: `require!(!ctx.accounts.evolution_config.is_paused, EvolutionError::EvolutionPaused)`. Evolved bond redemption IS pause-gated. Admin can freeze evolved bond withdrawals. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | HIGH |
| **Why this matters** | Same issue as BG-07 for L1 bonds. A malicious admin can permanently lock L2-L4 bond holders' funds by pausing evolution. Even more severe here because evolved bonds already have a `lock_duration` — adding a pause gate creates a double lock mechanism. |
| **Suggested fix** | Remove the pause check from `redeem_evolved_bond`. Users should always be able to redeem after their lock expires, regardless of pause state. |

---

## EBG-08: Collect Fees — Same Missing `update_fees_and_rewards` Issue

| Field | Detail |
|---|---|
| **Function / Flow** | `collect_fees` (evolution program, lines 401-443) |
| **EVM Behavior** | Uniswap V3 handles fee accounting internally in `collect()`. |
| **Solana Behavior** | `collect_fees` calls `whirlpool_cpi::collect_fees()` without a preceding `update_fees_and_rewards` CPI. Same issue as BG-10 in the base program. |
| **Classification** | **MISSING BEHAVIOR** |
| **Severity** | HIGH |
| **Why this matters** | Identical to BG-10. Fees may be stale without the `update_fees_and_rewards` pre-call. Affects all evolved bond (L2-L4) fee collection. |
| **Suggested fix** | Add `update_fees_and_rewards` CPI before `collect_fees`. |

---

## EBG-09: Layer Token Minting — Oracle Binds Amount, But No On-Chain Cap

| Field | Detail |
|---|---|
| **Function / Flow** | `token::mint_to` during `evolve_bond` |
| **EVM Behavior** | EVM finding C-01: "Contract mints `_amount1 * numberOfBonds` of token1. Amount is only constrained by off-chain signature. Compromised signer enables unlimited minting." |
| **Solana Behavior** | `evolve_bond` line 767-780: mints `amount_b` layer tokens via `token::mint_to` with `layer_token_authority` PDA. `amount_b` is oracle-signed. No on-chain cap on total minted layer tokens. No per-level or global mint limit. |
| **Classification** | **EXACT MATCH** (inherits EVM risk) |
| **Severity** | HIGH |
| **Why this matters** | A compromised oracle key can sign unlimited `amount_b`, minting unlimited layer tokens. The on-chain program has no defense. While the oracle signature timestamp (60s staleness) and per-user nonce limit the blast radius, a compromised oracle can still mint unbounded tokens by signing many messages in rapid succession for different users (or the same user with sequential nonces). |
| **Suggested fix** | Add on-chain guardrails: (1) per-level cumulative mint cap stored in `LevelConfig`, (2) per-epoch rate limit, or (3) circuit-breaker that auto-pauses if cumulative mints exceed a threshold. |

---

## EBG-10: Residual Token Handling — Excess Layer Tokens Are Burned

| Field | Detail |
|---|---|
| **Function / Flow** | Post-`increase_liquidity` residual handling in `evolve_bond` |
| **EVM Behavior** | Residual tokens from LP creation stay in the contract. Only recoverable by admin via `recoverERC20`. |
| **Solana Behavior** | Lines 833-866: After `increase_liquidity` CPI, residual token A is returned to user (`token::transfer`), residual layer tokens (token B) are burned (`token::burn`). No protocol-owned residuals. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana explicitly handles residuals: user gets excess token A back, excess layer tokens are burned (deflationary). EVM leaves residuals in the contract (inflationary, since minted layer tokens that weren't consumed aren't burned). This is a strict improvement. |
| **Suggested fix** | None needed. |

---

## EBG-11: Double-Evolution Prevention — PDA Init vs. No EVM Check

| Field | Detail |
|---|---|
| **Function / Flow** | Preventing evolving the same bond twice |
| **EVM Behavior** | No explicit double-evolution check. A bond could theoretically be evolved multiple times if the user still holds it (but since NFTs are transferred to `multiSigBurned`, the user would need to re-acquire them). |
| **Solana Behavior** | `EvolutionRecord` PDA seeded by `["evolution_record", source_bond_mint]` with `init` constraint (line 1718). If a record already exists for this source bond, the `init` will fail with `AccountAlreadyInUse`. This is a one-time-per-source-bond guarantee. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana's PDA init constraint provides deterministic, on-chain double-evolution prevention. Even if the source bond mint were somehow re-created (impossible with SPL Token), the evolution record already exists. |
| **Suggested fix** | None needed. |

---

## EBG-12: Source Custody Cross-Program Read — Fragile Deserialization

| Field | Detail |
|---|---|
| **Function / Flow** | `validate_source_custody` (evolution, line 1020-1084) |
| **EVM Behavior** | EVM contracts are in the same address space and use direct `locks[positionId]` mapping reads across contracts. |
| **Solana Behavior** | `validate_source_custody` deserializes the source custody via `PositionCustodyRef::deserialize(&mut &custody_data[8..])`. This requires the `PositionCustodyRef` struct in `lp-bonds-evolution` to exactly match the field layout of `PositionCustody` in `lp-bonds`. If the base program adds, removes, or reorders fields, the deserialization will silently produce garbage values. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | MEDIUM |
| **Why this matters** | There's no compile-time coupling between `PositionCustodyRef` (evolution) and `PositionCustody` (lp-bonds). A program upgrade to `lp-bonds` that changes the custody struct layout will break all evolution operations without any error — it will silently read incorrect field values. The `PositionCustody` in the evolution program (line 1622-1639) also mirrors the same struct but is used for locally-created custodies. |
| **Suggested fix** | (1) Add an integration test that verifies `PositionCustodyRef` deserializes identically to `PositionCustody`. (2) Share the struct definition via a common library crate. (3) Add a discriminator/version byte to `PositionCustody` that `validate_source_custody` checks before deserializing. |

---

## EBG-13: `close_evolution_nonce` — Same Nonce Reset Vulnerability

| Field | Detail |
|---|---|
| **Function / Flow** | `close_evolution_nonce` (evolution, line 1825-1837) |
| **EVM Behavior** | Global nonce, no close mechanism. |
| **Solana Behavior** | Identical risk to BG-18. User can close evolution nonce → re-initialize at 0 → old oracle signatures become replayable within the timestamp window. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | HIGH |
| **Why this matters** | Same nonce reset vulnerability as the base program. Evolution signatures within the 60s staleness window could be replayed after nonce reset. |
| **Suggested fix** | Same as BG-18: remove `close_evolution_nonce` or add a minimum-nonce tracking mechanism. |

---

## EBG-14: Unused Permission Bits — Dead Code Risk

| Field | Detail |
|---|---|
| **Function / Flow** | `PERM_PAUSE (0x02)`, `PERM_UPDATE_TREASURY (0x04)`, `PERM_UPDATE_ORACLE (0x08)` |
| **EVM Behavior** | N/A — these are Solana-specific constants. |
| **Solana Behavior** | `constants.rs` defines 4 permission bits. Only `PERM_CONFIGURE_LEVELS (0x01)` is used in `configure_level_delegated` (line 1396-1430). No `pause_delegated`, `update_treasury_delegated`, or `update_oracle_delegated` instructions exist. |
| **Classification** | **MISSING BEHAVIOR** |
| **Severity** | MEDIUM |
| **Why this matters** | The `AuthorityWhitelist` stores a `permissions` bitmask, and `add_authority` accepts a `permissions` parameter. An integrator might set `PERM_PAUSE | PERM_CONFIGURE_LEVELS` expecting the whitelisted authority to be able to pause — but only level configuration actually works. This creates false security expectations. |
| **Suggested fix** | Either implement the missing delegated instructions (pause, update_treasury, update_oracle) or remove the unused permission bits from constants.rs to prevent confusion. |

---

## EBG-15: `multiSigBurned` Address — No Solana Equivalent

| Field | Detail |
|---|---|
| **Function / Flow** | `setMultiSigBurned` (EVM) |
| **EVM Behavior** | A dedicated address receives "burned" base-layer NFTs. NFTs are not actually burned — they are transferred. `multiSigBurned` can be `address(0)` (EVM finding C-02). |
| **Solana Behavior** | `token::burn()` is used — no `multiSigBurned` equivalent. Bonds are actually destroyed (supply=0). The bond mint continues to exist (as a zero-supply mint) but cannot be re-minted. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Eliminates the EVM trust assumption around `multiSigBurned`. Burned bonds are permanently destroyed on Solana. |
| **Suggested fix** | None. |

---

## EBG-16: `recover_tokens` WITH Supply Check — But Incomplete Account Validation

| Field | Detail |
|---|---|
| **Function / Flow** | `recover_tokens` (evolution, lines 368-399) |
| **EVM Behavior** | `recoverERC20` / `recoverERC721` — generic recovery with no custody protection. |
| **Solana Behavior** | `RecoverTokens` struct (line 1840-1873): `bond_mint.supply == 0` constraint ensures recovery only from burned bonds. BUT: the `source_token_account` is only constrained by `owner == layer_token_authority.key()`. The `bond_mint` parameter is any mint with supply 0 — it doesn't have to be the bond mint ASSOCIATED with the source token account. An admin could pass any zero-supply mint and drain any `layer_token_authority`-owned token account. |
| **Classification** | **DANGEROUS DEVIATION** |
| **Severity** | MEDIUM |
| **Why this matters** | The `bond_mint.supply == 0` check is a necessary but insufficient guard. It doesn't verify that the `source_token_account` is the custody position token account for THAT specific `bond_mint`. An admin with any zero-supply mint (even from an unrelated program) could drain any token account owned by `layer_token_authority`. In practice, `layer_token_authority`-owned accounts are typically program-created token transit accounts, not long-lived custody, so the blast radius is limited. |
| **Suggested fix** | Add a constraint linking `source_token_account` to `bond_mint`: e.g., verify that a `PositionCustody` PDA seeded by `bond_mint` exists and its `position_mint` matches `source_token_account.mint`. Or derive the expected ATA from `(layer_token_authority, position_mint)` and verify it matches. |

---

## EBG-17: Level Transition — Strict `+1` vs. EVM Layer-to-Bond Lookup

| Field | Detail |
|---|---|
| **Function / Flow** | Level transition validation |
| **EVM Behavior** | Level transitions are implicit: `layers[_origBondId][_layerId]` specifies a `baseLayer` (collection to burn) and `bondId` (target bond config). Any layerId can map to any bond. No strict sequential enforcement. |
| **Solana Behavior** | `evolve_bond` line 575-581: `require!(target_level == source_level + 1, EvolutionError::InvalidLevelTransition)`. Strictly sequential: 1→2→3→4. Cannot skip levels. Source level is read from source custody. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | Solana enforces stricter progression. EVM could theoretically allow L1→L3 (if configured). Solana prevents skipping. This is a design decision that matches the described tier system. |
| **Suggested fix** | None needed. |

---

## EBG-18: `evolve_bond` — No Check That User Still Owns Source Bond at Burn Time

| Field | Detail |
|---|---|
| **Function / Flow** | Source bond ownership during evolution |
| **EVM Behavior** | `ownerOf(_baseTokenId[i]) == _msgSender()` — explicit ownership check before transfer. |
| **Solana Behavior** | `EvolveBond` struct line 1674-1679: `user_source_bond_account.owner == user.key()` (ownership check via constraint). BUT: `user_source_bond_account.amount` is NOT checked to be `== 1`. The `token::burn` at line 721 would fail if amount is 0, but the error would be an opaque SPL Token error rather than a descriptive `EvolutionError`. |
| **Classification** | **SAFE DEVIATION** |
| **Severity** | LOW |
| **Why this matters** | The burn will revert if the user doesn't hold the token, so there's no security issue. But the error message will be an SPL Token error rather than a protocol-specific error, making debugging harder for users/frontends. |
| **Suggested fix** | Add `constraint = user_source_bond_account.amount == 1 @ EvolutionError::InvalidBondBalance` to the `EvolveBond` struct. |

---

## FINAL SUMMARY

### Finding Distribution

| Classification | Count |
|---|---|
| **EXACT MATCH** | 3 |
| **SAFE DEVIATION** | 8 |
| **DANGEROUS DEVIATION** | 4 |
| **MISSING BEHAVIOR** | 3 |
| **Total** | **18** |

### Critical / High Findings

| # | ID | Classification | Severity | Summary |
|---|---|---|---|---|
| 1 | EBG-07 | DANGEROUS DEVIATION | HIGH | Evolved bond redemption is pause-gated |
| 2 | EBG-08 | MISSING BEHAVIOR | HIGH | No `update_fees_and_rewards` before `collect_fees` |
| 3 | EBG-09 | EXACT MATCH (inherited) | HIGH | No on-chain cap on layer token minting |
| 4 | EBG-13 | DANGEROUS DEVIATION | HIGH | `close_evolution_nonce` allows nonce reset |

### Combined Findings Count (Both Programs)

| Source | Findings |
|---|---|
| `lp-bonds` | 21 |
| `lp-bonds-evolution` | 18 |
| **Deduplicated Total** | **33** (some findings are shared: fee collection, pause-gating, nonce reset) |

### Recommendations (Priority Order)

1. **P0**: Remove pause gate from `redeem_evolved_bond` (EBG-07)
2. **P0**: Add `update_fees_and_rewards` CPI before `collect_fees` (EBG-08)
3. **P1**: Add on-chain layer token mint caps / circuit-breaker (EBG-09)
4. **P1**: Remove or safeguard `close_evolution_nonce` (EBG-13)
5. **P2**: Add minimum fee enforcement (EBG-04)
6. **P2**: Strengthen `recover_tokens` account linkage (EBG-16)
7. **P2**: Implement or remove unused permission bits (EBG-14)
8. **P3**: Add integration test for cross-program struct layout (EBG-12)
9. **P3**: Add `amount == 1` constraint to evolved bond burn (EBG-18)

---

*End of lp-bonds-evolution Behavioral Gaps Report*
