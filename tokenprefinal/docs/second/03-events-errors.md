# Events and Errors Coverage

## Events

### EVM events emitted by LPToken (direct or inherited)

| EVM event | Emitted when | Solana event | Defined in | Tag |
|---|---|---|---|---|
| `Transfer(address(0), to, amt)` | mint | `TokensMinted { authority, recipient, amount }` | `events.rs:14-19` | parity |
| `Transfer(from, address(0), amt)` | burn | `TokensBurned { authority, from, amount }` | `events.rs:22-28` | parity |
| `Transfer(from, to, amt)` | ERC20 transfer | SPL Token Program log | -- | runtime |
| `Approval(owner, spender, amt)` | approve | SPL Token Program log | -- | runtime |
| `MinterUpdated(account, isMinter)` | updateMinter | `MinterUpdated { minter, is_active }` | `events.rs:32-36` | parity (nit: consider adding `mint` / `token_state` for multi-mint indexers) |
| `Paused(account)` | pause | `PauseStateChanged { paused, authority }` | `events.rs:40-44` | parity (combined with Unpaused) |
| `Unpaused(account)` | unpause | `PauseStateChanged { paused, authority }` | `events.rs:40-44` | parity |
| `OwnershipTransferred(previous, new)` | transferOwnership / init | `OwnershipTransferred { previous_owner, new_owner }` | `events.rs:56-60` | parity |
| `Initialized(uint8 version)` | OZ init lifecycle | n/a | -- | runtime (Anchor `init` is atomic; no separate signal needed) |
| `Upgraded(impl)` | proxy upgrade | BPFLoaderUpgradeable logs | -- | runtime |
| `AdminChanged(prev, new)` | proxy admin change | BPFLoaderUpgradeable logs | -- | runtime |

### Solana-only events (additions over EVM)

| Solana event | Emitted by | Purpose |
|---|---|---|
| `MintInitialized { mint, owner, evm_chain_id, decimals }` | `initialize_mint` | Observability at bootstrap. |
| `OwnershipTransferProposed { current_owner, proposed_owner }` | `transfer_ownership` | Step 1 of two-step transfer. |

### Event emission audit

| Instruction | Emits |
|---|---|
| `initialize_mint` | `MintInitialized` (`initialize_mint.rs:74`) |
| `mint_tokens` | `TokensMinted` (`mint_tokens.rs:98`) |
| `burn_tokens` | `TokensBurned` (`burn_tokens.rs:92`) |
| `update_minter` | `MinterUpdated` (`update_minter.rs:71`) |
| `set_pause` | `PauseStateChanged` (`set_pause.rs:53`) |
| `transfer_ownership` | `OwnershipTransferProposed` (`transfer_ownership.rs:60`) |
| `accept_ownership` | `OwnershipTransferred` (`accept_ownership.rs:42`) |
| `transfer_tokens` | none (SPL Token emits) |
| `approve_delegate` | none (SPL Token emits) |

All state-changing instructions emit at least one event. No silent writes.

## Errors

### EVM `require` messages

| EVM require | Location | Solana error variant | Location | Tag |
|---|---|---|---|---|
| `"GMIToken: Only minter or owner is allowed"` | `LPToken.sol:45` | `LPTokenError::Unauthorized` | `errors.rs:6-7` | parity |
| `"GMIToken: Duplicate operation"` | `LPToken.sol:109` | `LPTokenError::DuplicateOperation` | `errors.rs:14-15` | parity |
| `Ownable: caller is not the owner` | OZ Ownable | `LPTokenError::Unauthorized` applied via constraint in `set_pause.rs:30`, `update_minter.rs:26`, `transfer_ownership.rs:36` | parity |
| `Ownable: new owner is the zero address` | OZ Ownable | `LPTokenError::InvalidOwner` | `errors.rs:30-31`; enforced `transfer_ownership.rs:48` | parity (and also enforced on `initialize_mint` as defense-in-depth) |
| `Pausable: paused` | OZ Pausable | `LPTokenError::Paused` | `errors.rs:10-11`; enforced in `mint_tokens.rs:28`, `burn_tokens.rs:36` | parity |
| `Pausable: not paused` | OZ Pausable | `LPTokenError::InvalidPauseState` | `errors.rs:26-27`; enforced `set_pause.rs:45-49` | parity |
| `ReentrancyGuard: reentrant call` | OZ ReentrancyGuard | n/a | runtime | runtime |
| `ERC20: burn amount exceeds balance` | OZ ERC20 | Handled by SPL Token Program (`InsufficientFunds`) | runtime | runtime |
| `ERC20: transfer amount exceeds balance` | OZ ERC20 | Same | runtime | runtime |
| `ERC20: insufficient allowance` | OZ ERC20 | SPL Token Program (delegated_amount check) | runtime | runtime |
| `ERC20: transfer to the zero address` | OZ ERC20 | SPL TokenAccount cannot be `Pubkey::default()` | runtime | runtime |
| `Initializable: contract is already initialized` | OZ Initializable | Anchor `init` constraint rejects re-init | runtime | covered -- see ATK-14 |

### Solana-only error variants

| Variant | Why it exists | Location |
|---|---|---|
| `LPTokenError::InvalidTokenAuthority` | Guards against `token_account.owner != signer` in `burn_tokens` / `approve_delegate` | `errors.rs:18-19` |
| `LPTokenError::InvalidMint` | Guards against cross-mint substitution (token account mint mismatch; token_state not authority) | `errors.rs:22-23` |
| `LPTokenError::NoPendingOwnership` | Two-step transfer safety: accept-when-no-proposal | `errors.rs:34-35` |

### Error coverage audit

Every `require` in LPToken.sol has a Solana counterpart. Solana adds three error variants to cover attack surfaces that do not exist on EVM (cross-mint account substitution, UncheckedAccount validation, two-step state machine). No EVM error path is silently dropped.
