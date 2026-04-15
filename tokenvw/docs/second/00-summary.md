# LPToken EVM -> Solana Parity — Executive Summary

**Scope:** `contracts/lp-token/LPToken.sol` (EVM) vs `programs/lp_token/` (Solana / Anchor 0.31.1).
**Report date:** 2026-04-15.
**Verdict:** Behaviorally equivalent. The Solana port covers every LPToken-declared function, plus inherited ERC20 surface (via SPL Token Program), plus Pausable/Ownable semantics. A handful of surface-level gaps are all either intentional design decisions, runtime-handled, or low-severity hardening items.

## Headline Numbers

| Category | Count |
|---|---|
| LPToken-declared external functions | 7 (`initialize`, `mint`, `burn`, `updateMinter`, `pause`, `unpause`, `impl`) |
| Declared functions with Solana counterpart | 7 / 7 (pause + unpause collapsed into one `set_pause`) |
| Inherited ERC20 functions | 12 (name, symbol, decimals, totalSupply, balanceOf, transfer, transferFrom, approve, allowance, increaseAllowance, decreaseAllowance, plus getters) |
| Inherited ERC20 coverage | Handled by SPL Token Program natively; `transfer_tokens` and `approve_delegate` wrappers added for ergonomic parity |
| Inherited Ownable / Pausable | `owner`, `paused`, `minters(addr)`, `chainId` all queryable from `TokenState` / `MinterRecord`; `transferOwnership` implemented as two-step (intentional upgrade); `renounceOwnership` intentionally omitted |
| Custom Solana instructions beyond EVM | `accept_ownership` (two-step pattern) |
| EVM events | 8 total -- all have Anchor `emit!` counterparts except OpenZeppelin-internal `Initialized` (runtime artifact) |
| Custom errors | 8 Anchor variants -- covers every `require` string in LPToken plus Solana-specific ones |

## True Gaps / Follow-up Items

These are real items worth addressing or consciously accepting. None block behavioral equivalence.

1. **Program ID placeholder not yet replaced.** `declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS")` at `programs/lp_token/src/lib.rs:23` -- must be replaced before mainnet deploy. Already flagged in project memory.
2. **Upgrade authority not yet set to multisig.** The Solana upgrade model (BPFLoaderUpgradeable) replaces the EVM `LPTokenProxyAdmin`. Deployment runbook should set `program upgrade authority` to a squads/multisig equivalent of the EVM ProxyAdmin owner. No on-chain artifact enforces this today.
3. **No on-chain metadata (name, symbol).** EVM `ERC20Upgradeable` stores name/symbol on-chain; the base SPL mint does not. The `InitializeMint` doc comment points to Metaplex Token Metadata but there is no CPI into `mpl-token-metadata` from `initialize_mint`. If clients expect on-chain name/symbol, add a post-init Metaplex CPI or a separate `set_metadata` instruction. Intentional and documented, but surface-visible gap.
4. **Decimals behavioral note.** Fixed to 9 (u64 constraint); EVM uses 18. Balance range compresses from `uint256` to `u64`. Acceptable for LP bond scale but document in integration guide.
5. **`impl()` not ported.** The EVM `impl()` view returns `address(this)` (effectively proxy-observable sentinel). On Solana this concept does not exist -- `declare_id!()` is the program ID and is always known to the client. Correctly omitted. Listed here only so reviewers do not flag it.
6. **`renounceOwnership` intentionally not implemented.** Documented in `transfer_ownership.rs:18-25`. Blocking renunciation is a conscious safety choice for a governance-required LP token.
7. **Minor: `transfer_tokens` / `approve_delegate` wrappers duplicate SPL Token Program functionality.** They add no security value (the SPL program already enforces delegation/balance). They exist for IDL parity with EVM signatures. Consider whether you want to publish them or ask clients to use SPL Token directly. Low-severity.
8. **`events::MinterUpdated` loses `token_state` / `mint` dimension.** Current event is `{ minter, is_active }`. If multiple mints ever share this program, indexers cannot tell which mint the event pertains to. Add `mint: Pubkey` or `token_state: Pubkey` for robustness. Low severity.

## Security Posture

- Authority model is tight. Every privileged instruction derives `token_state` PDA from `[TOKEN_STATE_SEED, token_mint]` and constrains the signer against `token_state.owner`, matching EVM `onlyOwner`.
- `verify_minter` (`mint_tokens.rs:120-166`) implements the five checks required for safe `UncheckedAccount` minter lookups: PDA derivation, program ownership, non-empty, discriminator + deserialization, `is_active`. This is noticeably more defense-in-depth than the EVM mapping lookup.
- `init_if_needed` on `minter_record` is safe (PDA-derived seeds, attacker cannot front-run).
- Dual-signer burn (`authority + token_account_authority`) is strictly stronger than EVM burn. Consciously accepted -- does not break the LP bond redemption flow.
- Two-step ownership transfer strictly stronger than EVM `_transferOwnership`.

## Deployment Blockers

| Blocker | Location | Severity |
|---|---|---|
| Replace placeholder program ID | `programs/lp_token/src/lib.rs:23` | MUST FIX |
| Set program upgrade authority to multisig on deploy | deploy runbook | MUST FIX |
| (Optional) Wire Metaplex metadata in `initialize_mint` or ship a `set_metadata` instruction | `initialize_mint.rs` | SHOULD FIX |

## File Index

- `01-function-parity.md` -- function-by-function table, EVM file:line vs Solana file:line
- `02-state-mapping.md` -- EVM storage -> Solana PDAs/accounts, seed rationale
- `03-events-errors.md` -- event + error coverage matrix
- `04-access-control.md` -- owner / minter / pause gate verification per instruction
- `05-security-analysis.md` -- SPL CPI authority review, behavioral differences, defense-in-depth checks
- `06-test-coverage.md` -- `tests/lp_token.ts` coverage by instruction with negative-case audit
