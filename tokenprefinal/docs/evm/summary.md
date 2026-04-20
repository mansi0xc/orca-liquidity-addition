# EVM Token Contracts -- Complete Function Reference

## All Functions Across All Contracts

| Contract | Function | Visibility | Modifiers | Parameters | State Reads | State Writes | Events | Returns | Purpose |
|---|---|---|---|---|---|---|---|---|---|
| **LPToken** | `initialize` | external | `initializer` | `name_: string, symbol_: string, owner_: address, chainId_: uint256` | `_initialized`, `_initializing` | `_name`, `_symbol`, `_owner`, `_paused`, `_status`, `chainId`, `_initialized` | `OwnershipTransferred` (x2), `Initialized` | void | One-time proxy initialization |
| LPToken | `mint` | external | `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant` | `_account: address, _amount: uint256` | `minters[msg.sender]`, `_owner`, `_paused`, `_status` | `_totalSupply`, `_balances[_account]`, `_status` | `Transfer(address(0), _account, _amount)` | `bool` | Mint tokens to account |
| LPToken | `burn` | external | `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant` | `_account: address, _amount: uint256` | `minters[msg.sender]`, `_owner`, `_paused`, `_status`, `_balances[_account]` | `_totalSupply`, `_balances[_account]`, `_status` | `Transfer(_account, address(0), _amount)` | `bool` | Burn tokens from account (no allowance check) |
| LPToken | `updateMinter` | external | `onlyOwner` | `_account: address, _isMinter: bool` | `_owner`, `minters[_account]` | `minters[_account]` | `MinterUpdated(_account, _isMinter)` | void | Add/remove minter role |
| LPToken | `pause` | external | `whenNotPaused`, `onlyOwner` | none | `_paused`, `_owner` | `_paused = true` | `Paused(msg.sender)` | void | Pause mint/burn operations |
| LPToken | `unpause` | external | `whenPaused`, `onlyOwner` | none | `_paused`, `_owner` | `_paused = false` | `Unpaused(msg.sender)` | void | Unpause mint/burn operations |
| LPToken | `impl` | external view | none | none | none | none | none | `address` | Returns address(this) |
| LPToken | `name` | public view | none (inherited ERC20Upgradeable) | none | `_name` | none | none | `string` | Returns token name |
| LPToken | `symbol` | public view | none (inherited ERC20Upgradeable) | none | `_symbol` | none | none | `string` | Returns token symbol |
| LPToken | `decimals` | public view | none (inherited ERC20Upgradeable) | none | none | none | none | `uint8` (18) | Returns decimal places |
| LPToken | `totalSupply` | public view | none (inherited ERC20Upgradeable) | none | `_totalSupply` | none | none | `uint256` | Returns total supply |
| LPToken | `balanceOf` | public view | none (inherited ERC20Upgradeable) | `account: address` | `_balances[account]` | none | none | `uint256` | Returns account balance |
| LPToken | `transfer` | public | none (inherited ERC20Upgradeable) | `to: address, amount: uint256` | `_balances[msg.sender]`, `_balances[to]` | `_balances[msg.sender]`, `_balances[to]` | `Transfer(msg.sender, to, amount)` | `bool` | Transfer tokens (works when paused) |
| LPToken | `allowance` | public view | none (inherited ERC20Upgradeable) | `owner: address, spender: address` | `_allowances[owner][spender]` | none | none | `uint256` | Returns remaining allowance |
| LPToken | `approve` | public | none (inherited ERC20Upgradeable) | `spender: address, amount: uint256` | none | `_allowances[msg.sender][spender]` | `Approval(msg.sender, spender, amount)` | `bool` | Set allowance (works when paused) |
| LPToken | `transferFrom` | public | none (inherited ERC20Upgradeable) | `from: address, to: address, amount: uint256` | `_allowances[from][msg.sender]`, `_balances[from]`, `_balances[to]` | `_allowances[from][msg.sender]`, `_balances[from]`, `_balances[to]` | `Transfer(from, to, amount)`, `Approval` (if finite) | `bool` | Transfer using allowance (works when paused) |
| LPToken | `increaseAllowance` | public | none (inherited ERC20Upgradeable) | `spender: address, addedValue: uint256` | `_allowances[msg.sender][spender]` | `_allowances[msg.sender][spender]` | `Approval(msg.sender, spender, newValue)` | `bool` | Increase allowance safely |
| LPToken | `decreaseAllowance` | public | none (inherited ERC20Upgradeable) | `spender: address, subtractedValue: uint256` | `_allowances[msg.sender][spender]` | `_allowances[msg.sender][spender]` | `Approval(msg.sender, spender, newValue)` | `bool` | Decrease allowance safely |
| LPToken | `owner` | public view | none (inherited OwnableUpgradeable) | none | `_owner` | none | none | `address` | Returns contract owner |
| LPToken | `transferOwnership` | public | `onlyOwner` (inherited OwnableUpgradeable) | `newOwner: address` | `_owner` | `_owner` | `OwnershipTransferred(old, new)` | void | Transfer ownership |
| LPToken | `renounceOwnership` | public | `onlyOwner` (inherited OwnableUpgradeable) | none | `_owner` | `_owner = address(0)` | `OwnershipTransferred(old, address(0))` | void | Irrevocably renounce ownership |
| LPToken | `paused` | public view | none (inherited PausableUpgradeable) | none | `_paused` | none | none | `bool` | Returns pause state |
| LPToken | `minters` | public view | none (auto-generated getter) | `_account: address` | `minters[_account]` | none | none | `bool` | Returns minter status |
| LPToken | `chainId` | public view | none (auto-generated getter) | none | `chainId` | none | none | `uint256` | Returns chain ID |
| **GMIToken** | `initialize` | external | `initializer` | `name_: string, symbol_: string, owner_: address, chainId_: uint256, maxSupply_: uint256` | `_initialized`, `_initializing` | `_name`, `_symbol`, `_owner`, `_paused`, `_status`, `chainId`, `maxSupply`, `_initialized` | `OwnershipTransferred` (x2), `Initialized` | void | One-time proxy initialization with max supply |
| GMIToken | `mint` | external | `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant` | `_account: address, _amount: uint256` | `minters[msg.sender]`, `_owner`, `_paused`, `_status`, `_totalSupply`, `maxSupply` | `_totalSupply`, `_balances[_account]`, `_status` | `Transfer(address(0), _account, _amount)` | `bool` | Mint tokens (capped at maxSupply) |
| GMIToken | `burn` | external | `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant` | `_account: address, _amount: uint256` | `minters[msg.sender]`, `_owner`, `_paused`, `_status`, `_balances[_account]` | `_totalSupply`, `_balances[_account]`, `_status` | `Transfer(_account, address(0), _amount)` | `bool` | Burn tokens from account (no allowance check) |
| GMIToken | `updateMinter` | external | `onlyOwner` | `_account: address, _isMinter: bool` | `_owner`, `minters[_account]` | `minters[_account]` | `MinterUpdated(_account, _isMinter)` | void | Add/remove minter role |
| GMIToken | `pause` | external | `whenNotPaused`, `onlyOwner` | none | `_paused`, `_owner` | `_paused = true` | `Paused(msg.sender)` | void | Pause ALL operations |
| GMIToken | `unpause` | external | `whenPaused`, `onlyOwner` | none | `_paused`, `_owner` | `_paused = false` | `Unpaused(msg.sender)` | void | Unpause ALL operations |
| GMIToken | `_transfer` | internal | `whenNotPaused`, `nonReentrant` (override) | `from: address, to: address, amount: uint256` | `_paused`, `_status`, `_balances[from]`, `_balances[to]` | `_balances[from]`, `_balances[to]`, `_status` | `Transfer(from, to, amount)` | void | Transfer with pause+reentrancy checks |
| GMIToken | `_approve` | internal | `whenNotPaused`, `nonReentrant` (override) | `owner: address, spender: address, amount: uint256` | `_paused`, `_status` | `_allowances[owner][spender]`, `_status` | `Approval(owner, spender, amount)` | void | Approve with pause+reentrancy checks |
| GMIToken | `impl` | external view | none | none | none | none | none | `address` | Returns address(this) |
| GMIToken | `maxSupply` | public view | none (auto-generated getter) | none | `maxSupply` | none | none | `uint256` | Returns max supply cap |
| GMIToken | (inherited ERC20/Ownable/Pausable functions) | -- | -- | -- | -- | -- | -- | -- | Same as LPToken but transfer/approve are paused-gated via overrides |
| **GMICVToken** | `initialize` | external | `initializer` | `name_: string, symbol_: string, owner_: address, chainId_: uint256, maxSupply_: uint256` | `_initialized`, `_initializing` | `_name`, `_symbol`, `_owner`, `_paused`, `_status`, `chainId`, `maxSupply`, `_initialized` | `OwnershipTransferred` (x2), `Initialized` | void | One-time initialization with max supply |
| GMICVToken | `mint` | external | `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant` | `_account: address, _amount: uint256` | `minters[msg.sender]`, `_owner`, `_paused`, `_status`, `_totalSupply`, `maxSupply` | `_totalSupply`, `_balances[_account]`, `_status` | `Transfer(address(0), _account, _amount)` | void | Mint tokens (capped, no return value) |
| GMICVToken | `burn` | external | `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant` | `_account: address, _amount: uint256` | `minters[msg.sender]`, `_owner`, `_paused`, `_status`, `_balances[_account]` | `_totalSupply`, `_balances[_account]`, `_status` | `Transfer(_account, address(0), _amount)` | void | Burn tokens (no return value, no allowance check) |
| GMICVToken | `updateMinter` | external | `onlyOwner` | `_account: address, _isMinter: bool` | `_owner`, `minters[_account]` | `minters[_account]` | `MinterUpdated(_account, _isMinter)` | void | Add/remove minter role |
| GMICVToken | `updateAllowedExchange` | external | `onlyOwner` | `_exchange: address, _isAllowed: bool` | `_owner`, `allowedExchanges[_exchange]` | `allowedExchanges[_exchange]` | `AllowedExchangesUpdated(_exchange, _isAllowed)` | void | Add/remove exchange from whitelist |
| GMICVToken | `updateMaxSupply` | external | `onlyOwner` | `_newMaxSupply: uint256` | `_owner`, `maxSupply` | `maxSupply` | `MaxSupplyUpdated(old, new)` | void | Update max supply cap |
| GMICVToken | `pause` | external | `whenNotPaused`, `onlyOwner` | none | `_paused`, `_owner` | `_paused = true` | `Paused(msg.sender)` | void | Pause ALL operations |
| GMICVToken | `unpause` | external | `whenPaused`, `onlyOwner` | none | `_paused`, `_owner` | `_paused = false` | `Unpaused(msg.sender)` | void | Unpause ALL operations |
| GMICVToken | `allowTrade` | external | `whenNotPaused`, `onlyOwner` | none | `_paused`, `_owner`, `tradeAllowed` | `tradeAllowed = !tradeAllowed` | `TradeAllowUpdated(newValue)` | void | Toggle global trade permission |
| GMICVToken | `_transfer` | internal | `whenNotPaused`, `nonReentrant`, `tradeAllowance(from, to)` (override) | `from: address, to: address, amount: uint256` | `_paused`, `_status`, `tradeAllowed`, `allowedExchanges[from]`, `allowedExchanges[to]`, `_balances[from]`, `_balances[to]` | `_balances[from]`, `_balances[to]`, `_status` | `Transfer(from, to, amount)` | void | Transfer with pause+reentrancy+trade checks |
| GMICVToken | `_approve` | internal | `whenNotPaused`, `nonReentrant`, `approveAllowance(spender)` (override) | `owner: address, spender: address, amount: uint256` | `_paused`, `_status`, `tradeAllowed`, `allowedExchanges[spender]` | `_allowances[owner][spender]`, `_status` | `Approval(owner, spender, amount)` | void | Approve with pause+reentrancy+trade checks |
| GMICVToken | `_isContract` | internal view | none | `_address: address` | none (uses assembly extcodesize) | none | none | `bool` | Check if address is a contract |
| GMICVToken | `allowedExchanges` | public view | none (auto-generated getter) | `_exchange: address` | `allowedExchanges[_exchange]` | none | none | `bool` | Returns exchange whitelist status |
| GMICVToken | `tradeAllowed` | public view | none (auto-generated getter) | none | `tradeAllowed` | none | none | `bool` | Returns global trade flag |
| GMICVToken | `maxSupply` | public view | none (auto-generated getter) | none | `maxSupply` | none | none | `uint256` | Returns max supply cap |
| GMICVToken | (inherited ERC20/Ownable/Pausable functions) | -- | -- | -- | -- | -- | -- | -- | Same as GMIToken |
| **LPTokenProxy** | `constructor` | constructor | none | `logic: address, admin: address, data: bytes` | none | ERC1967 impl slot, ERC1967 admin slot | `Upgraded(logic)`, `AdminChanged(address(0), admin)` | N/A | Deploy proxy pointing to implementation |
| **LPTokenProxyAdmin** | `constructor` | constructor | none | `owner_: address` | none | `_owner` | `OwnershipTransferred` (x2) | N/A | Deploy ProxyAdmin with specified owner |
| LPTokenProxyAdmin | `upgrade` | public | `onlyOwner` (inherited ProxyAdmin) | `proxy: ITransparentUpgradeableProxy, implementation: address` | `_owner` | ERC1967 impl slot (on proxy) | `Upgraded(implementation)` (on proxy) | void | Upgrade proxy implementation |
| LPTokenProxyAdmin | `upgradeAndCall` | public payable | `onlyOwner` (inherited ProxyAdmin) | `proxy: ITransparentUpgradeableProxy, implementation: address, data: bytes` | `_owner` | ERC1967 impl slot (on proxy) | `Upgraded(implementation)` (on proxy) | void | Upgrade and call initialization |
| LPTokenProxyAdmin | `changeProxyAdmin` | public | `onlyOwner` (inherited ProxyAdmin) | `proxy: ITransparentUpgradeableProxy, newAdmin: address` | `_owner` | ERC1967 admin slot (on proxy) | `AdminChanged(old, new)` (on proxy) | void | Change proxy admin |
| LPTokenProxyAdmin | `getProxyImplementation` | public view | none (inherited ProxyAdmin) | `proxy: ITransparentUpgradeableProxy` | ERC1967 impl slot (on proxy) | none | none | `address` | Get current implementation |
| LPTokenProxyAdmin | `getProxyAdmin` | public view | none (inherited ProxyAdmin) | `proxy: ITransparentUpgradeableProxy` | ERC1967 admin slot (on proxy) | none | none | `address` | Get current admin |
| LPTokenProxyAdmin | `owner` | public view | none (inherited Ownable) | none | `_owner` | none | none | `address` | Returns ProxyAdmin owner |
| LPTokenProxyAdmin | `transferOwnership` | public | `onlyOwner` (inherited Ownable) | `newOwner: address` | `_owner` | `_owner` | `OwnershipTransferred` | void | Transfer ProxyAdmin ownership |
| LPTokenProxyAdmin | `renounceOwnership` | public | `onlyOwner` (inherited Ownable) | none | `_owner` | `_owner = address(0)` | `OwnershipTransferred` | void | Irrevocably renounce (locks upgrades) |
| **GMITokenProxy** | `constructor` | constructor | none | `logic: address, admin: address, data: bytes` | none | ERC1967 slots | `Upgraded`, `AdminChanged` | N/A | Same as LPTokenProxy |
| **GMITokenProxyAdmin** | (all functions) | -- | -- | -- | -- | -- | -- | -- | Identical to LPTokenProxyAdmin |
| **GMICVTokenProxy** | `constructor` | constructor | none | `logic: address, admin: address, data: bytes` | none | ERC1967 slots | `Upgraded`, `AdminChanged` | N/A | Same as LPTokenProxy |
| **GMICVTokenProxyAdmin** | (all functions) | -- | -- | -- | -- | -- | -- | -- | Identical to LPTokenProxyAdmin |
| **DummyExchange** | `constructor` | constructor | none | `gmiToken_: address` | none | `gmiToken` | none | N/A | Set token address |
| DummyExchange | `send` | external | none | `to_: address, _amount: uint256` | `gmiToken` | none (external call) | `Transfer` (via token) | void | Transfer tokens from this contract |

## Function Counts

**Total: 74 unique function signatures across 9 contracts** (counting inherited functions once per contract that uses them, and excluding duplicated proxy/proxyAdmin contracts).

Breakdown:
- LPToken: 25 functions (7 declared + 18 inherited)
- GMIToken: 28 functions (9 declared + 19 inherited, but many inherited are overridden)
- GMICVToken: 33 functions (13 declared + 20 inherited)
- LPTokenProxy: 1 constructor + inherited proxy functions
- LPTokenProxyAdmin: 1 constructor + 7 inherited functions
- GMITokenProxy: identical to LPTokenProxy
- GMITokenProxyAdmin: identical to LPTokenProxyAdmin
- GMICVTokenProxy: identical to LPTokenProxy
- GMICVTokenProxyAdmin: identical to LPTokenProxyAdmin
- DummyExchange: 2 functions

## Key Behavioral Notes for Solana Migration Comparison

1. **LPToken pause scope**: Only blocks mint/burn -- transfers and approvals are UNAFFECTED by pause
2. **LPToken burn authorization**: No allowance check -- minter/owner can burn any account's tokens unilaterally
3. **LPToken supply**: Completely uncapped -- no maxSupply
4. **LPToken decimals**: 18 (Solana uses 9)
5. **LPToken value range**: uint256 (Solana uses u64)
6. **Error message prefix**: All custom errors use "GMIToken:" prefix even in LPToken (copy-paste artifact)
7. **chainId**: Stored but never used in any logic -- purely informational
8. **impl()**: Misleading behavior through proxy -- returns proxy address, not implementation address
9. **No storage gap**: LPToken does not declare `__gap`, limiting safe upgrade extensibility
10. **Interface mismatch**: IGMIToken interface in lp-token/ directory does not match LPToken's actual interface
