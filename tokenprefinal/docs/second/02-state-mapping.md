# Storage -> Account Mapping

EVM stores everything in contract storage slots of one proxy instance. Solana splits state across: (a) the SPL Mint account, (b) SPL TokenAccounts, (c) program-owned PDAs.

## Per-storage-variable mapping

| EVM storage | EVM declared at | Solana location | Tag |
|---|---|---|---|
| `_name` (string) | OZ ERC20Upgradeable | Metaplex metadata PDA (NOT wired; see 00-summary item 3) | partial |
| `_symbol` (string) | OZ ERC20Upgradeable | Metaplex metadata PDA (NOT wired) | partial |
| `_totalSupply` (uint256) | OZ ERC20Upgradeable | `Mint.supply` (u64) | runtime |
| `_balances[addr]` (mapping) | OZ ERC20Upgradeable | `TokenAccount.amount` per (mint, owner) | runtime |
| `_allowances[owner][spender]` (nested mapping) | OZ ERC20Upgradeable | `TokenAccount.delegate` + `TokenAccount.delegated_amount` (single delegate per account) | intentional-diff |
| `_owner` (address) | OZ OwnableUpgradeable | `TokenState.owner` (Pubkey) | parity |
| `_paused` (bool) | OZ PausableUpgradeable | `TokenState.is_paused` | parity |
| `_status` (ReentrancyGuard) | OZ ReentrancyGuardUpgradeable | n/a -- Solana runtime forbids reentrancy by design | runtime |
| `minters[addr]` (mapping) | `LPToken.sol:37` | `MinterRecord.is_active` at PDA `[b"minter", token_state, addr]` | parity |
| `chainId` (uint256) | `LPToken.sol:39` | `TokenState.evm_chain_id` (u64) | intentional-diff (type narrowed -- chain IDs fit in u64) |
| `_initialized`, `_initializing` (OZ Initializable) | OZ internals | Handled by Anchor `init` constraint on `token_state` and `token_mint` | runtime |
| ERC1967 impl slot (proxy) | OZ proxy | BPFLoaderUpgradeable `ProgramData` account | runtime |
| ERC1967 admin slot (proxy) | OZ proxy | ProgramData upgrade_authority field | runtime |

## PDA design

### `TokenState`

- Seeds: `[b"token_state", mint_pubkey]`
- Defined: `state/token_state.rs:10-40`
- Init: `initialize_mint.rs:42-49`
- Size: `8 + INIT_SPACE` -> 8 disc + 32 (owner) + 32 (pending_owner) + 1 (paused) + 8 (chain_id) + 1 (bump) = 82 bytes total.
- Authority role: acts as `mint_authority` and `freeze_authority` on the SPL mint. See `initialize_mint.rs:35-36`.
- Rationale: bound 1:1 to a single SPL mint. This is the correct shape because EVM LPToken is itself a 1:1 instance per proxy deployment. One program can therefore host many independent LP tokens.

### `MinterRecord`

- Seeds: `[b"minter", token_state_pubkey, minter_pubkey]`
- Defined: `state/minter_record.rs:9-21`
- Init: `update_minter.rs:42-49` via `init_if_needed`
- Size: 8 + 1 (is_active) + 32 (minter) + 1 (bump) = 42 bytes.
- Rationale: EVM `mapping(address => bool)` -> one PDA per (token, minter) pair. Using `token_state` (not the raw mint) in the seed keeps records logically grouped to the governance scope.

## Seed / Derivation validation checklist

| Check | Status | Evidence |
|---|---|---|
| `TokenState` seeds include the mint -> prevents cross-mint substitution | PASS | `mint_tokens.rs:26`, `burn_tokens.rs:34`, `set_pause.rs:28`, etc. All instructions redrive `token_state` from `token_mint.key()`. |
| `bump = token_state.bump` used on verified PDA -> no missing bump check | PASS | Every account constraint uses `bump = token_state.bump` rather than `bump` alone. |
| `MinterRecord` seed contains both `token_state` and `minter` -> cannot reuse a record across mints | PASS | `update_minter.rs:46`, `mint_tokens.rs:128-134`. |
| `verify_minter` cross-checks PDA address against expected derivation | PASS | `mint_tokens.rs:127-139`. |
| `verify_minter` checks program ownership of the `UncheckedAccount` | PASS | `mint_tokens.rs:146-149`. |
| Discriminator verification on `UncheckedAccount` data | PASS | via `MinterRecord::try_deserialize` at `mint_tokens.rs:160`. |
| `init_if_needed` seed cannot be manipulated by attacker | PASS | `target_minter` is `UncheckedAccount` used only as seed material; seeds include the trusted `token_state`. An attacker re-calling `update_minter` on an existing record just toggles `is_active` and is gated by duplicate-op check. |

## Cross-mint isolation

When the same deployed `lp_token` program is used to host multiple SPL mints, each gets a distinct `TokenState` PDA. The attack surface to check is: could someone use mint-A's `token_state` to authorize mint-B operations?

- `mint_tokens`: `token_state` seeds include `token_mint.key()`, so passing the wrong mint fails seed verification. Further, `token_mint.mint_authority.contains(&token_state.key())` is checked (`mint_tokens.rs:48`). PASS.
- `burn_tokens`: same two checks (`burn_tokens.rs:34`, `burn_tokens.rs:50`). PASS.
- `update_minter`, `set_pause`, `transfer_ownership`, `accept_ownership`: all rederive `token_state` from the supplied `token_mint`. PASS.

See ATK-6 through ATK-9 in `tests/lp_token.ts:1594-1762` for direct coverage.
