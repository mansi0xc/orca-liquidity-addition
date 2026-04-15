# Function-by-Function Parity

All EVM file:line refs point to `contracts/lp-token/LPToken.sol`. Solana refs point to `programs/lp_token/src/`.

Legend: **parity** | **partial** | **missing** | **intentional-diff** | **runtime**

## LPToken-declared functions

| # | EVM function | EVM loc | Solana equivalent | Solana loc | Tag | Notes |
|---|---|---|---|---|---|---|
| 1 | `initialize(name_, symbol_, owner_, chainId_)` | `LPToken.sol:57-71` | `initialize_mint(params)` | `instructions/initialize_mint.rs:24-83` | intentional-diff | Name/symbol not stored on-chain (base SPL mint has no such field); use Metaplex. `chainId_` -> `evm_chain_id`. Adds zero-address owner guard that the EVM version lacks. |
| 2 | `mint(_account, _amount) onlyMintersOrOwner whenNotPaused nonReentrant` | `LPToken.sol:79-86` | `mint_tokens(amount)` | `instructions/mint_tokens.rs:18-106` | parity | Owner-or-minter gate in `apply()` via `is_owner` short-circuit + `verify_minter`. Pause gate via `constraint = !token_state.is_paused`. Reentrancy unneeded on Solana (no reentrant CPI model). |
| 3 | `burn(_account, _amount) onlyMintersOrOwner whenNotPaused nonReentrant` | `LPToken.sol:94-100` | `burn_tokens(amount)` | `instructions/burn_tokens.rs:24-100` | intentional-diff | Adds mandatory token_account_authority co-signer. Documented as security improvement; acceptable for LP bond redemption. |
| 4 | `updateMinter(_account, _isMinter) onlyOwner` | `LPToken.sol:108-114` | `update_minter(params)` | `instructions/update_minter.rs:17-78` | parity | Duplicate-op guard preserved. Uses `init_if_needed` for first registration of a new minter. |
| 5 | `pause() onlyOwner whenNotPaused` | `LPToken.sol:120-122` | `set_pause(true)` | `instructions/set_pause.rs:21-60` | parity | Collapsed with unpause into one instruction with a bool, with explicit state-transition guard. |
| 6 | `unpause() onlyOwner whenPaused` | `LPToken.sol:128-130` | `set_pause(false)` | `instructions/set_pause.rs:21-60` | parity | Same instruction as above. |
| 7 | `impl() -> address` | `LPToken.sol:137-139` | (none) | -- | intentional-diff | Solana programs have a fixed `declare_id!()` known to clients; the EVM `impl()` was a proxy-pattern artifact. Omission is correct. |

## Inherited ERC20 surface

| # | EVM function | EVM origin | Solana equivalent | Solana loc | Tag | Notes |
|---|---|---|---|---|---|---|
| 8 | `name()` | OZ ERC20Upgradeable | (Metaplex metadata) | not implemented | partial | See 00-summary item 3. Not load-bearing for on-chain logic. |
| 9 | `symbol()` | OZ ERC20Upgradeable | (Metaplex metadata) | not implemented | partial | Same as above. |
| 10 | `decimals() -> 18` | OZ ERC20Upgradeable | SPL `Mint.decimals` | set at init in `initialize_mint.rs:34` | intentional-diff | 9 vs 18. |
| 11 | `totalSupply()` | OZ ERC20Upgradeable | SPL `Mint.supply` | on-chain field | runtime | Fetched from the mint account directly; no instruction needed. |
| 12 | `balanceOf(acct)` | OZ ERC20Upgradeable | SPL `TokenAccount.amount` | on-chain field | runtime | Same. |
| 13 | `transfer(to, amt)` | OZ ERC20Upgradeable | `transfer_tokens(amount)` + SPL Token transfer | `instructions/transfer_tokens.rs:17-53` | parity | Wrapper is optional; clients can call SPL directly. |
| 14 | `transferFrom(from, to, amt)` | OZ ERC20Upgradeable | `transfer_tokens` with delegate as `from_authority` | `instructions/transfer_tokens.rs` | parity | SPL delegation model covers `transferFrom`. |
| 15 | `approve(spender, amt)` | OZ ERC20Upgradeable | `approve_delegate(amount)` + SPL approve | `instructions/approve_delegate.rs:16-52` | parity | Wrapper is optional. |
| 16 | `allowance(owner, spender)` | OZ ERC20Upgradeable | SPL `TokenAccount.delegated_amount` | on-chain field | runtime | Different semantics: one delegate at a time, vs EVM multi-spender map. See `05-security-analysis.md` item B. |
| 17 | `increaseAllowance(s, v)` | OZ ERC20Upgradeable | `approve_delegate` with new total | wrapper | intentional-diff | Solana lacks atomic increment; clients compute new total. |
| 18 | `decreaseAllowance(s, v)` | OZ ERC20Upgradeable | `approve_delegate` with new total or SPL revoke | wrapper | intentional-diff | Same. |

## Inherited Ownable / Pausable / custom getters

| # | EVM function | EVM origin | Solana equivalent | Solana loc | Tag | Notes |
|---|---|---|---|---|---|---|
| 19 | `owner()` | OZ OwnableUpgradeable | `TokenState.owner` | `state/token_state.rs:15` | runtime | Read via Anchor account fetch. |
| 20 | `transferOwnership(newOwner)` | OZ OwnableUpgradeable | `transfer_ownership(new_owner)` | `instructions/transfer_ownership.rs:27-67` | intentional-diff | Two-step. Strictly stronger; see 04. |
| 21 | `renounceOwnership()` | OZ OwnableUpgradeable | (not implemented) | -- | intentional-diff | Documented decision at `transfer_ownership.rs:18-25`. |
| 22 | `paused()` | OZ PausableUpgradeable | `TokenState.is_paused` | `state/token_state.rs:32` | runtime | |
| 23 | `minters(addr)` | auto-getter | `MinterRecord.is_active` at PDA `[b"minter", token_state, addr]` | `state/minter_record.rs:11-21` | runtime | |
| 24 | `chainId` | auto-getter | `TokenState.evm_chain_id` | `state/token_state.rs:36` | runtime | Purely informational on both chains. |

## Events (see also `03-events-errors.md`)

| EVM event | Solana `emit!` | Tag |
|---|---|---|
| `Transfer(address(0), to, amt)` on mint | `TokensMinted` | parity |
| `Transfer(from, address(0), amt)` on burn | `TokensBurned` | parity |
| `Transfer(from, to, amt)` on transfer | SPL Token Program log | runtime |
| `Approval(owner, spender, amt)` | SPL Token Program log | runtime |
| `MinterUpdated(account, isMinter)` | `MinterUpdated` | parity (see 00-summary item 8 for a robustness nit) |
| `Paused(sender)` / `Unpaused(sender)` | `PauseStateChanged { paused, authority }` | parity |
| `OwnershipTransferred(prev, new)` | `OwnershipTransferred` | parity |
| `Initialized(uint8)` (OZ internal) | n/a | runtime |
| (no EVM event) | `OwnershipTransferProposed` | new, for two-step pattern |
| (no EVM event) | `MintInitialized` | new, for init observability |

## Proxy-layer functions (LPTokenProxy / LPTokenProxyAdmin)

| EVM function | Solana equivalent | Tag |
|---|---|---|
| `upgrade(proxy, impl)` | `solana program deploy --upgrade-authority <multisig>` | runtime |
| `upgradeAndCall(proxy, impl, data)` | `solana program deploy` + separate init ix | runtime |
| `changeProxyAdmin(proxy, newAdmin)` | `solana program set-upgrade-authority` | runtime |
| `getProxyImplementation` / `getProxyAdmin` | `solana program show` | runtime |
| Proxy `constructor` | BPFLoaderUpgradeable handles deploy | runtime |

Handled by BPFLoaderUpgradeable natively; see `05-security-analysis.md` item G for upgrade-authority hardening.
