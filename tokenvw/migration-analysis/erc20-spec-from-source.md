# ERC20 Spec From Source — LPToken.sol

Derived **exclusively** from EVM Solidity source code.
Source files examined:

- `LPToken.sol` (Energi Core, Solidity 0.8.22)
- `ERC20Upgradeable.sol` (OpenZeppelin 4.5.0)
- `OwnableUpgradeable.sol` (OpenZeppelin 4.4.1)
- `PausableUpgradeable.sol` (OpenZeppelin 4.4.1)
- `ReentrancyGuardUpgradeable.sol` (OpenZeppelin 4.4.1)
- `IERC20Upgradeable.sol` (OpenZeppelin 4.5.0)
- `IERC20MetadataUpgradeable.sol` (OpenZeppelin 4.4.1)
- `IERC20MintBurn.sol` (Energi Core — consumer interface)

---

## Inheritance Chain

```
LPToken
  ├── ERC20Upgradeable
  │     ├── Initializable
  │     ├── ContextUpgradeable
  │     ├── IERC20Upgradeable
  │     └── IERC20MetadataUpgradeable
  ├── OwnableUpgradeable
  │     ├── Initializable
  │     └── ContextUpgradeable
  ├── PausableUpgradeable
  │     ├── Initializable
  │     └── ContextUpgradeable
  └── ReentrancyGuardUpgradeable
        └── Initializable
```

---

## State Variables (Complete)

### From LPToken.sol
| Variable | Type | Visibility | Source |
|----------|------|-----------|--------|
| `minters` | `mapping(address => bool)` | `public` (auto-getter) | LPToken.sol:37 |
| `chainId` | `uint256` | `public` (auto-getter) | LPToken.sol:39 |

### From ERC20Upgradeable
| Variable | Type | Visibility | Source |
|----------|------|-----------|--------|
| `_balances` | `mapping(address => uint256)` | `private` | ERC20Upgradeable.sol:37 |
| `_allowances` | `mapping(address => mapping(address => uint256))` | `private` | ERC20Upgradeable.sol:39 |
| `_totalSupply` | `uint256` | `private` | ERC20Upgradeable.sol:41 |
| `_name` | `string` | `private` | ERC20Upgradeable.sol:43 |
| `_symbol` | `string` | `private` | ERC20Upgradeable.sol:44 |

### From OwnableUpgradeable
| Variable | Type | Visibility | Source |
|----------|------|-----------|--------|
| `_owner` | `address` | `private` | OwnableUpgradeable.sol:22 |

### From PausableUpgradeable
| Variable | Type | Visibility | Source |
|----------|------|-----------|--------|
| `_paused` | `bool` | `private` | PausableUpgradeable.sol:29 |

### From ReentrancyGuardUpgradeable
| Variable | Type | Visibility | Source |
|----------|------|-----------|--------|
| `_status` | `uint256` | `private` | ReentrancyGuardUpgradeable.sol:38 |

---

## Events (Complete)

| Event | Source | Parameters |
|-------|--------|-----------|
| `Transfer(from, to, value)` | IERC20Upgradeable.sol:75 | `address indexed from, address indexed to, uint256 value` |
| `Approval(owner, spender, value)` | IERC20Upgradeable.sol:81 | `address indexed owner, address indexed spender, uint256 value` |
| `MinterUpdated(account, isMinter)` | LPToken.sol:35 | `address indexed account, bool isMinter` |
| `Paused(account)` | PausableUpgradeable.sol:22 | `address account` |
| `Unpaused(account)` | PausableUpgradeable.sol:27 | `address account` |
| `OwnershipTransferred(previousOwner, newOwner)` | OwnableUpgradeable.sol:24 | `address indexed previousOwner, address indexed newOwner` |

---

## Modifiers (Complete)

| Modifier | Source | Behavior |
|----------|--------|----------|
| `onlyMintersOrOwner` | LPToken.sol:44-47 | `require(minters[msg.sender] \|\| msg.sender == owner())` |
| `onlyOwner` | OwnableUpgradeable.sol:47-50 | `require(owner() == _msgSender())` |
| `whenNotPaused` | PausableUpgradeable.sol:56-59 | `require(!paused())` |
| `whenPaused` | PausableUpgradeable.sol:68-71 | `require(paused())` |
| `nonReentrant` | ReentrancyGuardUpgradeable.sol:55-67 | `require(_status != _ENTERED); _status = _ENTERED; _; _status = _NOT_ENTERED` |
| `initializer` | Initializable (OpenZeppelin) | Can only be called once; sets initialized flag |
| `onlyInitializing` | Initializable (OpenZeppelin) | Only callable during `initializer` execution |

---

## ALL External/Public Functions

### F1: `initialize(string name_, string symbol_, address owner_, uint256 chainId_)`

- **Visibility:** `external`
- **Modifiers:** `initializer`
- **Source:** LPToken.sol:57-71
- **Behavior:**
  1. `__ERC20_init(name_, symbol_)` → stores `_name`, `_symbol`
  2. `__Ownable_init()` → calls `_transferOwnership(_msgSender())` → sets `_owner = msg.sender`, emits `OwnershipTransferred(address(0), msg.sender)`
  3. `__Pausable_init()` → sets `_paused = false`
  4. `__ReentrancyGuard_init()` → sets `_status = 1 (_NOT_ENTERED)`
  5. `_transferOwnership(owner_)` → sets `_owner = owner_`, emits `OwnershipTransferred(msg.sender, owner_)`
  6. `chainId = chainId_`
- **Require statements:** None explicit. `initializer` modifier prevents re-initialization.
- **Edge cases:**
  - `owner_` CAN be `address(0)` — no check exists in `_transferOwnership`. But this would immediately renounce ownership.
  - Calling again after initialization reverts via `initializer` guard.
  - Two `OwnershipTransferred` events emitted: first from `__Ownable_init` (0→deployer), then from `_transferOwnership` (deployer→owner_).

### F2: `mint(address _account, uint256 _amount) returns (bool)`

- **Visibility:** `external`
- **Modifiers:** `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant`
- **Source:** LPToken.sol:79-86
- **Modifier execution order:** `onlyMintersOrOwner` → `whenNotPaused` → `nonReentrant` → body
- **Behavior:**
  1. Access check: `require(minters[msg.sender] || msg.sender == owner())`
  2. Pause check: `require(!paused())`
  3. Reentrancy check: `require(_status != _ENTERED)`
  4. `_mint(_account, _amount)`:
     - `require(account != address(0), "ERC20: mint to the zero address")` ← **critical**
     - `_beforeTokenTransfer(address(0), account, amount)` ← no-op (not overridden)
     - `_totalSupply += amount` ← reverts on overflow (Solidity 0.8)
     - `_balances[account] += amount`
     - `emit Transfer(address(0), account, amount)`
     - `_afterTokenTransfer(address(0), account, amount)` ← no-op
  5. `return true`
- **Require statements:**
  - `minters[msg.sender] || msg.sender == owner()` (onlyMintersOrOwner)
  - `!paused()` (whenNotPaused)
  - `_status != _ENTERED` (nonReentrant)
  - `_account != address(0)` (ERC20._mint)
- **Edge cases:**
  - `_amount = 0`: succeeds, emits Transfer event with 0 value
  - `_account = address(0)`: reverts
  - Overflow of `_totalSupply + amount`: reverts (Solidity 0.8 checked arithmetic)

### F3: `burn(address _account, uint256 _amount) returns (bool)`

- **Visibility:** `external`
- **Modifiers:** `onlyMintersOrOwner`, `whenNotPaused`, `nonReentrant`
- **Source:** LPToken.sol:94-100
- **Behavior:**
  1. Same access/pause/reentrancy checks as mint
  2. `_burn(_account, _amount)`:
     - `require(account != address(0), "ERC20: burn from the zero address")` ← **critical**
     - `_beforeTokenTransfer(account, address(0), amount)` ← no-op
     - `uint256 accountBalance = _balances[account]`
     - `require(accountBalance >= amount, "ERC20: burn amount exceeds balance")`
     - `unchecked { _balances[account] = accountBalance - amount; }`
     - `_totalSupply -= amount`
     - `emit Transfer(account, address(0), amount)`
     - `_afterTokenTransfer(account, address(0), amount)` ← no-op
  3. `return true`
- **Require statements:**
  - Same as mint (access, pause, reentrancy)
  - `_account != address(0)` (ERC20._burn)
  - `_balances[_account] >= _amount` (balance check)
- **CRITICAL BEHAVIOR: Minter can burn from ANY address without that address's consent.** No allowance/approval needed. This is the intended design for bond lifecycle operations.

### F4: `updateMinter(address _account, bool _isMinter)`

- **Visibility:** `external`
- **Modifiers:** `onlyOwner`
- **Source:** LPToken.sol:108-114
- **Behavior:**
  1. `require(owner() == _msgSender())` (onlyOwner)
  2. `require(minters[_account] != _isMinter, "GMIToken: Duplicate operation")`
  3. `minters[_account] = _isMinter`
  4. `emit MinterUpdated(_account, _isMinter)`
- **Require statements:**
  - `msg.sender == owner()` (onlyOwner)
  - `minters[_account] != _isMinter` (duplicate prevention)
- **Edge cases:**
  - `_account = address(0)`: succeeds (no zero-address check)
  - Re-adding an active minter: reverts (duplicate)
  - Removing an inactive minter: reverts (duplicate)

### F5: `pause()`

- **Visibility:** `external`
- **Modifiers:** `whenNotPaused`, `onlyOwner`
- **Source:** LPToken.sol:120-122
- **Modifier execution order:** `whenNotPaused` → `onlyOwner` → body
- **Behavior:**
  1. `require(!paused())` (whenNotPaused)
  2. `require(owner() == _msgSender())` (onlyOwner)
  3. `_pause()`:
     - Has its own `whenNotPaused` check (redundant but harmless)
     - `_paused = true`
     - `emit Paused(_msgSender())`
- **Edge cases:**
  - Calling when already paused: reverts with "Pausable: paused"

### F6: `unpause()`

- **Visibility:** `external`
- **Modifiers:** `whenPaused`, `onlyOwner`
- **Source:** LPToken.sol:128-130
- **Modifier execution order:** `whenPaused` → `onlyOwner` → body
- **Behavior:**
  1. `require(paused())` (whenPaused)
  2. `require(owner() == _msgSender())` (onlyOwner)
  3. `_unpause()`:
     - Has its own `whenPaused` check (redundant but harmless)
     - `_paused = false`
     - `emit Unpaused(_msgSender())`
- **Edge cases:**
  - Calling when not paused: reverts with "Pausable: not paused"

### F7: `impl() returns (address)`

- **Visibility:** `external view`
- **Modifiers:** none
- **Source:** LPToken.sol:137-139
- **Behavior:** returns `address(this)`
- **Purpose:** Proxy implementation address discovery. Not relevant on Solana.

### F8: `name() returns (string)`

- **Visibility:** `public view`
- **Source:** ERC20Upgradeable.sol:67-69
- **Behavior:** returns `_name`

### F9: `symbol() returns (string)`

- **Visibility:** `public view`
- **Source:** ERC20Upgradeable.sol:75-77
- **Behavior:** returns `_symbol`

### F10: `decimals() returns (uint8)`

- **Visibility:** `public view`
- **Source:** ERC20Upgradeable.sol:92-94
- **Behavior:** returns `18` (hardcoded, not overridden by LPToken)

### F11: `totalSupply() returns (uint256)`

- **Visibility:** `public view`
- **Source:** ERC20Upgradeable.sol:99-101
- **Behavior:** returns `_totalSupply`

### F12: `balanceOf(address account) returns (uint256)`

- **Visibility:** `public view`
- **Source:** ERC20Upgradeable.sol:106-108
- **Behavior:** returns `_balances[account]`

### F13: `transfer(address to, uint256 amount) returns (bool)`

- **Visibility:** `public`
- **Source:** ERC20Upgradeable.sol:118-122
- **Not overridden by LPToken** — NO custom guards, NOT pause-gated
- **Behavior:**
  1. `_transfer(msg.sender, to, amount)`:
     - `require(from != address(0))` ← always true for external calls
     - `require(to != address(0), "ERC20: transfer to the zero address")`
     - `_beforeTokenTransfer(from, to, amount)` ← no-op
     - `require(_balances[from] >= amount, "ERC20: transfer amount exceeds balance")`
     - `_balances[from] -= amount`
     - `_balances[to] += amount`
     - `emit Transfer(from, to, amount)`
  2. `return true`
- **Require statements:**
  - `to != address(0)` (ERC20._transfer)
  - `_balances[msg.sender] >= amount` (balance check)

### F14: `transferFrom(address from, address to, uint256 amount) returns (bool)`

- **Visibility:** `public`
- **Source:** ERC20Upgradeable.sol:163-172
- **Not overridden by LPToken** — NOT pause-gated
- **Behavior:**
  1. `_spendAllowance(from, msg.sender, amount)`:
     - If `currentAllowance != type(uint256).max`: decrements allowance
     - `require(currentAllowance >= amount, "ERC20: insufficient allowance")`
     - Emits `Approval(from, msg.sender, newAllowance)` if allowance is finite
  2. `_transfer(from, to, amount)`: same as F13's internal _transfer
  3. `return true`
- **CRITICAL BEHAVIOR: Infinite allowance** — if allowance is `type(uint256).max`, it is NOT decremented on transferFrom. This is a semantic difference from SPL Token which always decrements.

### F15: `approve(address spender, uint256 amount) returns (bool)`

- **Visibility:** `public`
- **Source:** ERC20Upgradeable.sol:141-145
- **Not overridden by LPToken** — NOT pause-gated
- **Behavior:**
  1. `_approve(msg.sender, spender, amount)`:
     - `require(owner != address(0))` ← always true for external calls
     - `require(spender != address(0), "ERC20: approve to the zero address")`
     - `_allowances[owner][spender] = amount`
     - `emit Approval(owner, spender, amount)`
  2. `return true`
- **Require statements:**
  - `spender != address(0)` (ERC20._approve)

### F16: `allowance(address owner, address spender) returns (uint256)`

- **Visibility:** `public view`
- **Source:** ERC20Upgradeable.sol:127-129
- **Behavior:** returns `_allowances[owner][spender]`

### F17: `increaseAllowance(address spender, uint256 addedValue) returns (bool)`

- **Visibility:** `public`
- **Source:** ERC20Upgradeable.sol:186-190
- **Not overridden by LPToken**
- **Behavior:**
  1. `_approve(msg.sender, spender, _allowances[msg.sender][spender] + addedValue)`
  2. `return true`
- **Require statements:**
  - `spender != address(0)` (ERC20._approve)
  - Overflow of `currentAllowance + addedValue`: reverts (Solidity 0.8)

### F18: `decreaseAllowance(address spender, uint256 subtractedValue) returns (bool)`

- **Visibility:** `public`
- **Source:** ERC20Upgradeable.sol:206-215
- **Not overridden by LPToken**
- **Behavior:**
  1. `require(currentAllowance >= subtractedValue, "ERC20: decreased allowance below zero")`
  2. `_approve(msg.sender, spender, currentAllowance - subtractedValue)`
  3. `return true`

### F19: `owner() returns (address)`

- **Visibility:** `public view`
- **Source:** OwnableUpgradeable.sol:40-42
- **Behavior:** returns `_owner`

### F20: `transferOwnership(address newOwner)`

- **Visibility:** `public`
- **Modifiers:** `onlyOwner`
- **Source:** OwnableUpgradeable.sol:67-70
- **Behavior:**
  1. `require(owner() == _msgSender())` (onlyOwner)
  2. `require(newOwner != address(0), "Ownable: new owner is the zero address")`
  3. `_transferOwnership(newOwner)`:
     - `_owner = newOwner`
     - `emit OwnershipTransferred(oldOwner, newOwner)`
- **CRITICAL: Rejects address(0).** Use `renounceOwnership` to set owner to zero.

### F21: `renounceOwnership()`

- **Visibility:** `public`
- **Modifiers:** `onlyOwner`
- **Source:** OwnableUpgradeable.sol:59-61
- **Behavior:**
  1. `require(owner() == _msgSender())` (onlyOwner)
  2. `_transferOwnership(address(0))`:
     - `_owner = address(0)`
     - `emit OwnershipTransferred(oldOwner, address(0))`
- **CRITICAL: Permanently removes all owner-gated functionality.** No way to recover.

### F22: `paused() returns (bool)`

- **Visibility:** `public view`
- **Source:** PausableUpgradeable.sol:45-47
- **Behavior:** returns `_paused`

### F23: `minters(address) returns (bool)` (auto-generated getter)

- **Visibility:** `public view`
- **Source:** LPToken.sol:37 (`public` mapping)
- **Behavior:** returns `minters[addr]`

### F24: `chainId() returns (uint256)` (auto-generated getter)

- **Visibility:** `public view`
- **Source:** LPToken.sol:39 (`public` variable)
- **Behavior:** returns `chainId`

---

## Implicit Behaviors (Not Immediately Obvious)

### IB1: _beforeTokenTransfer / _afterTokenTransfer hooks
- Neither hook is overridden in LPToken. Both are no-ops.
- This means: **transfers, mints, and burns have NO additional custom logic beyond what is explicitly coded.**

### IB2: Pause does NOT block transfers
- LPToken does NOT override `_transfer`, `_approve`, or `_beforeTokenTransfer`.
- Therefore: `transfer()`, `transferFrom()`, `approve()`, `increaseAllowance()`, `decreaseAllowance()` all work regardless of pause state.
- Only `mint()` and `burn()` check `whenNotPaused`.

### IB3: Infinite allowance pattern
- ERC20._spendAllowance: `if (currentAllowance != type(uint256).max)` → skip decrement.
- If a user approves `type(uint256).max`, the allowance never decreases on `transferFrom`.
- SPL Token does NOT have this concept — all delegated amounts are always decremented.

### IB4: Burn without consent
- `_burn(account, amount)` requires NO approval from `account`.
- A minter/owner can burn from ANY address unilaterally.
- The IERC20MintBurn interface confirms this: `function burn(address from, uint256 amount) external`.

### IB5: Modifier execution order on pause()
- `pause()` has modifiers `whenNotPaused onlyOwner`.
- Executed left-to-right: `whenNotPaused` checked BEFORE `onlyOwner`.
- A non-owner calling `pause()` when unpaused gets the `onlyOwner` error.
- A non-owner calling `pause()` when paused gets the `whenNotPaused` error.
- This means the error message leaks information about pause state to non-owners.

### IB6: updateMinter has no pause guard
- `updateMinter()` only has `onlyOwner`. It works regardless of pause state.
- This matches: the owner can add/remove minters even while paused.

### IB7: OwnershipTransferred emitted twice during initialize
- `__Ownable_init()` sets owner to `msg.sender` → `OwnershipTransferred(0, msg.sender)`
- `_transferOwnership(owner_)` → `OwnershipTransferred(msg.sender, owner_)`
- Two events in a single initialize call.

### IB8: Zero-amount operations
- `mint(addr, 0)`: succeeds, emits `Transfer(0, addr, 0)`
- `burn(addr, 0)`: succeeds (0 >= 0), emits `Transfer(addr, 0, 0)`
- `transfer(addr, 0)`: succeeds
- `approve(spender, 0)`: succeeds (effectively revokes allowance)

### IB9: Self-transfer
- `transfer(msg.sender, amount)`: succeeds (deducts and adds to same account)
- `transferFrom(msg.sender, msg.sender, amount)`: succeeds with appropriate allowance

---

## Access Control Summary

| Function | Access | Pause-Gated | Reentrancy-Gated |
|----------|--------|-------------|-----------------|
| initialize | external, initializer (once) | No | No |
| mint | onlyMintersOrOwner | Yes (whenNotPaused) | Yes |
| burn | onlyMintersOrOwner | Yes (whenNotPaused) | Yes |
| updateMinter | onlyOwner | **No** | No |
| pause | onlyOwner | Yes (whenNotPaused) | No |
| unpause | onlyOwner | Yes (whenPaused) | No |
| impl | anyone (view) | No | No |
| transfer | anyone | **No** | No |
| transferFrom | anyone (with allowance) | **No** | No |
| approve | anyone | **No** | No |
| increaseAllowance | anyone | **No** | No |
| decreaseAllowance | anyone | **No** | No |
| transferOwnership | onlyOwner | No | No |
| renounceOwnership | onlyOwner | No | No |
| name/symbol/decimals/totalSupply/balanceOf/allowance/owner/paused/minters/chainId | anyone (view) | No | No |

---

## Complete Require Statement Inventory

| Function | Require | Error Message |
|----------|---------|---------------|
| mint | `minters[msg.sender] \|\| msg.sender == owner()` | "GMIToken: Only minter or owner is allowed" |
| mint | `!paused()` | "Pausable: paused" |
| mint | `_status != _ENTERED` | "ReentrancyGuard: reentrant call" |
| mint | `_account != address(0)` | "ERC20: mint to the zero address" |
| burn | `minters[msg.sender] \|\| msg.sender == owner()` | "GMIToken: Only minter or owner is allowed" |
| burn | `!paused()` | "Pausable: paused" |
| burn | `_status != _ENTERED` | "ReentrancyGuard: reentrant call" |
| burn | `_account != address(0)` | "ERC20: burn from the zero address" |
| burn | `_balances[_account] >= _amount` | "ERC20: burn amount exceeds balance" |
| updateMinter | `owner() == _msgSender()` | "Ownable: caller is not the owner" |
| updateMinter | `minters[_account] != _isMinter` | "GMIToken: Duplicate operation" |
| pause | `!paused()` | "Pausable: paused" |
| pause | `owner() == _msgSender()` | "Ownable: caller is not the owner" |
| unpause | `paused()` | "Pausable: not paused" |
| unpause | `owner() == _msgSender()` | "Ownable: caller is not the owner" |
| transfer | `to != address(0)` | "ERC20: transfer to the zero address" |
| transfer | `_balances[from] >= amount` | "ERC20: transfer amount exceeds balance" |
| transferFrom | `currentAllowance >= amount` | "ERC20: insufficient allowance" |
| transferFrom | `to != address(0)` | "ERC20: transfer to the zero address" |
| transferFrom | `_balances[from] >= amount` | "ERC20: transfer amount exceeds balance" |
| approve | `spender != address(0)` | "ERC20: approve to the zero address" |
| increaseAllowance | `spender != address(0)` | "ERC20: approve to the zero address" |
| decreaseAllowance | `currentAllowance >= subtractedValue` | "ERC20: decreased allowance below zero" |
| decreaseAllowance | `spender != address(0)` | "ERC20: approve to the zero address" |
| transferOwnership | `owner() == _msgSender()` | "Ownable: caller is not the owner" |
| transferOwnership | `newOwner != address(0)` | "Ownable: new owner is the zero address" |
| renounceOwnership | `owner() == _msgSender()` | "Ownable: caller is not the owner" |
