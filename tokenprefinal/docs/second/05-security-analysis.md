# Security Analysis — Gaps, Behavioral Differences, CPI Review

## A. Behavioral differences vs EVM (beyond decimals)

| # | Difference | Impact | Status |
|---|---|---|---|
| 1 | **Burn requires dual signer** (`authority` + `token_account_authority`). EVM allowed minter to burn any address unilaterally. | Stronger user protection; users must co-sign any burn of their balance. Does not break the LP bond redemption flow (user always signs when redeeming). | Intentional; `burn_tokens.rs:12-20` |
| 2 | **Two-step ownership transfer.** EVM transfers instantly in one step. | Typo-proof; prevents permanent loss from wrong address. | Intentional; `transfer_ownership.rs:7-14` |
| 3 | **`renounceOwnership` not exposed.** | LP token cannot have its owner set to zero -> always governable. | Intentional; `transfer_ownership.rs:18-25` |
| 4 | **Zero-address owner blocked in `initialize_mint`.** EVM `initialize` does not check this. | Fail-fast on init typo. | Defense-in-depth; `initialize_mint.rs:62-65` |
| 5 | **No-op ownership proposal blocked** (`new_owner == current owner`). | Avoids emitting a useless event and pending state. | Small UX hardening; `transfer_ownership.rs:52-56` |
| 6 | **Name/symbol not stored on-chain.** | Clients must read Metaplex metadata separately. | Intentional architectural difference. |
| 7 | **`totalSupply`, `balanceOf`, `allowance` served by SPL runtime, not wrapped.** | Instructions do not expose views; clients read accounts. | Runtime-handled. |
| 8 | **`allowance` semantics: one delegate per account, not a (owner,spender) map.** | EVM supports N concurrent approvals per holder; SPL supports one delegate at a time. | Intentional Solana platform difference; noted for integrators. |
| 9 | **Decimals 9 vs 18.** | Supply range `~1.8e19` at 9-dp vs `~5.7e28` at 18-dp (since u64 caps at `2^64-1`). Both vastly exceed LP bond scale. | Intentional. |
| 10 | **Reentrancy guard omitted.** | Solana runtime forbids reentrant CPIs. | Runtime-handled. |

## B. SPL Token CPI authority review

### `mint_to` CPI (`mint_tokens.rs:85-96`)

- Authority: `token_state` PDA (`to_account_info()`)
- Signer seeds: `[TOKEN_STATE_SEED, mint_key.as_ref(), &[token_state.bump]]` (`mint_tokens.rs:78-82`)
- Correctness: `token_state` PDA is set as `mint::authority` at init (`initialize_mint.rs:35`). SPL will verify the authority signed and matches the mint's stored authority. Bump used matches the stored bump. PASS.

### `burn` CPI (`burn_tokens.rs:80-90`)

- Authority: `token_account_authority` (a `Signer<'info>`) -- NOT the PDA
- Signer seeds: none (regular `CpiContext::new`)
- Correctness: SPL `burn` requires the token account's owner (or delegate) to be the authority. The `burn_tokens.rs:58` constraint enforces `token_account.owner == token_account_authority.key()`, and `token_account_authority` is a `Signer`. PASS.
- Side-effect: the `authority` (minter/owner) signer is NOT passed to SPL; they only control whether the burn may proceed logically. The actual SPL-level authorization comes from the token-account owner co-signer. This is the dual-signer model.

### `transfer` CPI (`transfer_tokens.rs:39-49`)

- Authority: `from_authority` (a `Signer<'info>`)
- Correctness: SPL accepts either the token account owner or its delegate as authority. When `from_authority` is the delegate, SPL enforces `delegated_amount`. When it is the owner, no delegation check. PASS.
- Note: The wrapper does NOT enforce that `from_authority == from_token_account.owner` -- this is intentional so that delegates can call it. Covered by SPL's internal check.

### `approve` CPI (`approve_delegate.rs:38-48`)

- Authority: `token_account_owner` (a `Signer`)
- Correctness: Constraint at `approve_delegate.rs:23` enforces `token_account.owner == token_account_owner.key()`, preventing anyone else from setting a delegate. SPL `approve` itself also enforces this, but the early constraint yields a clearer error. PASS.

## C. Solana-specific vulnerability audit

| Vulnerability | Status | Evidence |
|---|---|---|
| Account substitution (wrong PDA) | Mitigated | All PDAs derived from trusted seeds; cross-mint tests ATK-6/7/8/9 pass. |
| PDA collision via uncontrolled seeds | Mitigated | Seeds use typed `Pubkey` and fixed byte labels; no user-supplied strings. |
| Missing signer validation | Mitigated | Every authority is typed `Signer<'info>`. |
| Incorrect account ownership | Mitigated | `verify_minter` explicitly checks `minter_record_info.owner == program_id` (`mint_tokens.rs:146-149`). |
| CPI with wrong signer seeds | Mitigated | `mint_to` uses seeds derived from the mint being operated on. |
| Re-initialization attack | Mitigated | `init` constraint on `token_state` rejects re-init. ATK-14 covers this. |
| Rent exemption not enforced | Mitigated | Anchor `init` allocates rent-exempt by default. |
| Discriminator skipped on UncheckedAccount | Mitigated | `try_deserialize` re-verifies discriminator (`mint_tokens.rs:160`). |
| Compute-budget exhaustion | Low risk | No unbounded loops; all instructions are O(1). |
| Clock / timestamp misuse | N/A | No timestamp logic in LPToken. |
| `init_if_needed` front-running | Mitigated | PDA seeds are program-deterministic; attackers cannot create the same address under a different program. Subsequent re-entry of `update_minter` hits the duplicate-op guard. |

## D. Owner privilege escalation surface

An attacker who captured the `owner` private key could:
- Register themselves as minter and mint unlimited tokens -> same as EVM.
- Burn the owner's balance (still needs owner to sign as `token_account_authority` since dual-signer). For non-owner balances, the attacker-owner can register a minter, mint to themselves, but cannot unilaterally drain other users' balances -- this is a Solana-specific strict improvement over EVM LPToken.
- Pause the contract indefinitely (same as EVM; no time-lock).
- Transfer ownership to any address in two steps (same as EVM; two-step adds safety).

No privilege escalation beyond what the owner already has.

## E. Minter privilege surface

A compromised minter key can:
- Mint unlimited tokens (no max supply) -> same as EVM.
- Burn tokens only with token-account-owner co-signature (strictly less than EVM, which allowed unilateral burn).

## F. `verify_minter` deep-dive (`mint_tokens.rs:120-166`)

Five explicit checks:
1. Expected PDA derivation via `find_program_address` (not `create_program_address`; no attacker-supplied bump).
2. `minter_record_info.key() == expected_pda` -- exact match against derived address.
3. `minter_record_info.owner == program_id` -- ensures the account is one this program created.
4. `!minter_record_info.data_is_empty()` -- weeds out freshly-created-but-empty accounts.
5. `MinterRecord::try_deserialize(...)` -- Anchor discriminator + bincode parse.
6. `record.is_active == true`.

This is stricter than the typical Anchor `Account<'info, MinterRecord>` constraint because it handles the owner-OR-minter branching cleanly: the `UncheckedAccount` lets the owner path skip record validation entirely while still enforcing full validation on the minter path.

## G. Upgrade authority model

Solana uses BPFLoaderUpgradeable. After deployment:
- `solana program deploy` sets the deployer key as upgrade authority by default.
- MUST run `solana program set-upgrade-authority <program-id> --new-upgrade-authority <squads-multisig>` before mainnet.
- To permanently lock (equivalent of EVM renouncing ProxyAdmin), set `--final`.

This replaces the EVM `LPTokenProxyAdmin` contract. There is no on-chain Anchor artifact that encodes "must be multisig" -- it is operational.

Recommendation: add a `deploy/README.md` or runbook step that enforces this and `anchor verify` against the deployed program.

## H. Storage-layout / upgrade extensibility

EVM LPToken does not declare a `__gap` array, so future upgrades that add storage variables risk layout collisions with OZ base classes (see EVM analysis summary item 9).

On Solana, `TokenState` uses `#[derive(InitSpace)]` and allocates a fixed size at init. Adding fields in a future program version requires either:
- A migration instruction that `realloc`s accounts, or
- A new PDA namespace (e.g. `TokenStateV2`) with a migration step.

No gap array analog exists -- this is a conscious Solana pattern. Document the migration approach before the first mainnet upgrade.

## I. Summary

The Solana implementation is materially safer than the EVM original on two dimensions (dual-signer burn, two-step ownership) with no regression on attack-surface equivalence. No critical security gaps identified. Operational items (program ID placeholder, upgrade authority assignment, optional Metaplex wiring) are captured in `00-summary.md` deployment blockers.
