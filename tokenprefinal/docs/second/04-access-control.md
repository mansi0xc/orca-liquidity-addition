# Access Control Verification

## EVM modifier inventory (LPToken)

- `onlyOwner` (OZ Ownable) -- checks `msg.sender == _owner`
- `onlyMintersOrOwner` (custom, `LPToken.sol:44-47`) -- `minters[msg.sender] || msg.sender == owner()`
- `whenNotPaused` / `whenPaused` (OZ Pausable)
- `nonReentrant` (OZ ReentrancyGuard) -- n/a on Solana
- `initializer` (OZ Initializable) -- handled by Anchor `init`

## Per-instruction gate verification

| Instruction | EVM gate | Solana gate | Where enforced | Tag |
|---|---|---|---|---|
| `initialize_mint` | `initializer` | Anchor `init` on `token_state` and `token_mint` re-init-safe | `initialize_mint.rs:32-49` | parity |
| `initialize_mint` | -- | `require!(params.owner != Pubkey::default())` defense-in-depth | `initialize_mint.rs:62-65` | stronger than EVM |
| `mint_tokens` | `onlyMintersOrOwner` | `is_owner` check + `verify_minter(...)` | `mint_tokens.rs:66-74` | parity |
| `mint_tokens` | `whenNotPaused` | `constraint = !token_state.is_paused @ Paused` | `mint_tokens.rs:28` | parity |
| `mint_tokens` | `nonReentrant` | n/a | -- | runtime |
| `burn_tokens` | `onlyMintersOrOwner` | same as above | `burn_tokens.rs:66-76` | parity |
| `burn_tokens` | `whenNotPaused` | constraint on `token_state` | `burn_tokens.rs:36` | parity |
| `burn_tokens` | (EVM allows minter to burn anyone) | Extra: `token_account_authority: Signer` AND `token_account.owner == token_account_authority.key()` | `burn_tokens.rs:30`, `58` | stronger than EVM |
| `update_minter` | `onlyOwner` | `constraint = owner.key() == token_state.owner @ Unauthorized` | `update_minter.rs:26` | parity |
| `update_minter` | duplicate-op require | `require!(minter_record.is_active != params.is_active)` | `update_minter.rs:61-64` | parity |
| `set_pause` (pause) | `onlyOwner`, `whenNotPaused` | owner constraint + `require!(!current)` | `set_pause.rs:30, 45` | parity |
| `set_pause` (unpause) | `onlyOwner`, `whenPaused` | owner constraint + `require!(current)` | `set_pause.rs:30, 48` | parity |
| `transfer_ownership` | `onlyOwner` | owner constraint | `transfer_ownership.rs:36` | parity |
| `transfer_ownership` | zero-addr guard (OZ) | `require!(new_owner != Pubkey::default())` | `transfer_ownership.rs:47` | parity |
| `transfer_ownership` | -- | extra: `require!(new_owner != token_state.owner)` (no-op guard) | `transfer_ownership.rs:53` | stronger |
| `accept_ownership` | no EVM equivalent | `constraint = new_owner.key() == token_state.pending_owner @ NoPendingOwnership` | `accept_ownership.rs:26` | new, safer |
| `transfer_tokens` | none (OZ ERC20 has none) | no custom gate -- pause does NOT block | `transfer_tokens.rs` | parity |
| `approve_delegate` | none (OZ ERC20 has none) | no custom gate -- pause does NOT block | `approve_delegate.rs` | parity |

## Pause-gate preservation

LPToken's pause scope is subtle: OZ Pausable's `whenNotPaused` only protects functions that explicitly declare the modifier. LPToken declares it on `mint` and `burn` only -- not on `_transfer` or `_approve`. This is the critical behavioral distinction from GMIToken (which DOES override `_transfer` / `_approve` with `whenNotPaused`).

Verification:
- `mint_tokens` pause gate: `mint_tokens.rs:28` -- PRESENT
- `burn_tokens` pause gate: `burn_tokens.rs:36` -- PRESENT
- `transfer_tokens` pause gate: absent by design (`transfer_tokens.rs:37-38` comment confirms)
- `approve_delegate` pause gate: absent by design (`approve_delegate.rs:36-37` comment confirms)

This matches LPToken.sol exactly. See test `ATK-24` (`tests/lp_token.ts:2151`) which verifies transfers during pause.

## Owner-only gate preservation

Every EVM `onlyOwner` function has a Solana analogue with a direct constraint comparing the signer to `token_state.owner`. No owner check is missing.

## Minter-or-owner gate preservation

`mint_tokens` and `burn_tokens` implement:
1. `is_owner = authority.key() == token_state.owner` -- short-circuit owner path
2. If not owner, run `verify_minter(...)` which requires all five checks (PDA derivation, program ownership, non-empty, discriminator, `is_active`)

This matches EVM `minters[msg.sender] || msg.sender == owner()` and is defense-in-depth stronger for the minter path (on EVM, a storage slot is trusted implicitly; on Solana, the passed account must be proven to be the correct PDA of the trusted program).

Negative-case tests covering this path: `ATK-1` through `ATK-3`, `ATK-13` in `tests/lp_token.ts:1429-1852`.
