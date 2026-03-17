# ERC20 Token Specification — LPToken

This document fully describes the behavioral specification of `LPToken.sol` without referencing EVM code.
The LPToken is the migration target for the Solana `lp_token` program.

---

## Token Identity

| Property | Value |
|----------|-------|
| Standard | ERC20 (upgradeable) |
| Decimals | 18 |
| Max Supply | None (unbounded) |
| Upgradeability | TransparentUpgradeableProxy |
| Example Name | "HELLO-GMI LP Bond L4" |
| Example Symbol | "HELLO-GMI-L4" |

---

## State Variables

| Variable | Type | Description |
|----------|------|-------------|
| `name` | string | Token name (set at initialize, immutable after) |
| `symbol` | string | Token symbol (set at initialize, immutable after) |
| `decimals` | uint8 | Always 18 |
| `totalSupply` | uint256 | Total tokens in circulation |
| `balanceOf[addr]` | mapping | Token balance per address |
| `allowance[owner][spender]` | mapping | Delegated spend approval |
| `minters[addr]` | mapping(address → bool) | Addresses authorized to call mint/burn |
| `chainId` | uint256 | EVM chain ID recorded at initialization |
| `paused` | bool | Global pause flag |
| `owner` | address | Admin with exclusive governance rights |

---

## Events

| Event | Parameters | Emitted When |
|-------|-----------|--------------|
| `Transfer(from, to, amount)` | address indexed, address indexed, uint256 | On any token movement including mint (from=0) and burn (to=0) |
| `Approval(owner, spender, amount)` | address indexed, address indexed, uint256 | On any allowance update |
| `MinterUpdated(account, isMinter)` | address indexed, bool | When a minter is added or removed |
| `Paused(account)` | address | When contract is paused |
| `Unpaused(account)` | address | When contract is unpaused |

---

## Functions — Full Specification

---

### `initialize`

**Purpose:** One-time setup of the token. Replaces a constructor for upgradeable contracts.

**Parameters:**
- `name_` (string) — token name
- `symbol_` (string) — token symbol
- `owner_` (address) — initial owner
- `chainId_` (uint256) — EVM chain ID for this deployment

**State Updates:**
- Sets token `name` and `symbol`
- Transfers ownership to `owner_`
- Stores `chainId`
- Initializes pause state to unpaused
- Initializes reentrancy guard

**Access:** External, `initializer` (can only be called once)

**Events:** None directly (Transfer(0,0,0) implicit from ERC20 init)

**Edge Cases:**
- Calling again after initialization reverts (initializer guard)
- `owner_` cannot be zero address

---

### `mint`

**Purpose:** Creates new tokens and assigns them to `_account`.

**Parameters:**
- `_account` (address) — recipient of minted tokens
- `_amount` (uint256) — number of tokens to create

**State Updates:**
- Increases `totalSupply` by `_amount`
- Increases `balanceOf[_account]` by `_amount`

**Events:** `Transfer(address(0), _account, _amount)`

**Access:** `onlyMintersOrOwner` — caller must be a registered minter or the owner

**Security Assumptions:**
- Only trusted minter contracts (Locker, Exchange) should be registered
- No maximum supply limit — minting is bounded only by `uint256` overflow

**Guards:**
- `whenNotPaused` — reverts if contract is paused
- `nonReentrant` — prevents reentrant calls

**Returns:** `bool` (always `true` on success)

**Edge Cases:**
- Minting to address(0) reverts (OpenZeppelin ERC20 guard)
- Adding `_amount` that overflows `totalSupply` reverts (Solidity 0.8 overflow check)
- Caller must have `minters[msg.sender] == true` OR be the owner

---

### `burn`

**Purpose:** Destroys tokens from `_account`. The minter/owner initiates this, not the account holder.

**Parameters:**
- `_account` (address) — address from which tokens are removed
- `_amount` (uint256) — number of tokens to destroy

**State Updates:**
- Decreases `totalSupply` by `_amount`
- Decreases `balanceOf[_account]` by `_amount`

**Events:** `Transfer(_account, address(0), _amount)`

**Access:** `onlyMintersOrOwner` — caller must be a registered minter or the owner

**Security Assumptions:**
- A minter can burn from ANY address without that address's consent
- This is by design for the bond lifecycle: the exchange contract burns LP tokens when a user redeems a bond position

**Guards:**
- `whenNotPaused` — reverts if contract is paused
- `nonReentrant` — prevents reentrant calls

**Returns:** `bool` (always `true` on success)

**Edge Cases:**
- `_amount` > `balanceOf[_account]` reverts with underflow
- Account holder does NOT need to approve the minter to burn their tokens

---

### `updateMinter`

**Purpose:** Grants or revokes minting/burning privileges for an address.

**Parameters:**
- `_account` (address) — address to update
- `_isMinter` (bool) — `true` to add minter, `false` to remove

**State Updates:**
- Sets `minters[_account] = _isMinter`

**Events:** `MinterUpdated(_account, _isMinter)`

**Access:** `onlyOwner`

**Guards:**
- `require(minters[_account] != _isMinter)` — prevents duplicate/no-op operations

**Edge Cases:**
- Registering an already-registered minter reverts with "Duplicate operation"
- Removing a non-minter reverts with "Duplicate operation"

---

### `pause`

**Purpose:** Halts all `mint` and `burn` operations. Does NOT halt regular transfers.

**Parameters:** None

**State Updates:**
- Sets `paused = true`

**Events:** `Paused(msg.sender)`

**Access:** `onlyOwner`, `whenNotPaused`

**Edge Cases:**
- Calling while already paused reverts
- After pausing, `mint()` and `burn()` revert with "Pausable: paused"
- After pausing, `transfer()`, `approve()`, `transferFrom()` still succeed (no override in LPToken)

---

### `unpause`

**Purpose:** Restores `mint` and `burn` capability.

**Parameters:** None

**State Updates:**
- Sets `paused = false`

**Events:** `Unpaused(msg.sender)`

**Access:** `onlyOwner`, `whenPaused`

**Edge Cases:**
- Calling while not paused reverts

---

### `transfer` (ERC20 standard — not overridden)

**Purpose:** Move tokens from caller to recipient.

**Parameters:**
- `to` (address) — recipient
- `amount` (uint256) — token amount

**State Updates:**
- Decreases `balanceOf[msg.sender]`
- Increases `balanceOf[to]`

**Events:** `Transfer(msg.sender, to, amount)`

**Access:** Any address

**Guards:** None beyond standard ERC20 (balance check)

**Important:** LPToken does NOT override `_transfer`. Pause state has NO effect on regular transfers.

---

### `transferFrom` (ERC20 standard — not overridden)

**Purpose:** Move tokens from `from` to `to` using caller's allowance.

**Parameters:**
- `from` (address) — source
- `to` (address) — recipient
- `amount` (uint256) — token amount

**State Updates:**
- Decreases `allowance[from][msg.sender]`
- Decreases `balanceOf[from]`
- Increases `balanceOf[to]`

**Events:** `Transfer(from, to, amount)`, `Approval(from, msg.sender, newAllowance)`

**Access:** Any address, subject to allowance

**Guards:** None beyond standard ERC20

**Important:** Not blocked by pause.

---

### `approve` (ERC20 standard — not overridden)

**Purpose:** Authorize a spender to transfer up to `amount` tokens on caller's behalf.

**Parameters:**
- `spender` (address) — authorized spender
- `amount` (uint256) — maximum allowed spend

**State Updates:**
- Sets `allowance[msg.sender][spender] = amount`

**Events:** `Approval(msg.sender, spender, amount)`

**Access:** Any address

**Guards:** None beyond standard ERC20

**Important:** Not blocked by pause.

---

### `allowance` (ERC20 standard — view)

**Purpose:** Query the remaining delegation amount.

**Parameters:**
- `owner` (address)
- `spender` (address)

**Returns:** Current allowance (uint256)

---

### `balanceOf` (ERC20 standard — view)

**Purpose:** Query token balance of an address.

**Parameters:**
- `account` (address)

**Returns:** Token balance (uint256)

---

### `totalSupply` (ERC20 standard — view)

**Purpose:** Total tokens in existence.

**Returns:** uint256

---

### `impl`

**Purpose:** Returns the current implementation address. Used to verify the proxy is pointing at the correct implementation.

**Returns:** `address(this)` — the implementation contract address

---

## Access Control Summary

| Operation | Owner | Registered Minter | Any Address |
|-----------|-------|-------------------|-------------|
| mint | ✓ | ✓ | ✗ |
| burn | ✓ | ✓ | ✗ |
| updateMinter | ✓ | ✗ | ✗ |
| pause | ✓ | ✗ | ✗ |
| unpause | ✓ | ✗ | ✗ |
| transfer | ✓ | ✓ | ✓ |
| transferFrom | ✓ | ✓ | ✓ |
| approve | ✓ | ✓ | ✓ |
| balanceOf | ✓ | ✓ | ✓ |
| totalSupply | ✓ | ✓ | ✓ |

---

## Pause State Effect Matrix

| Operation | Paused | Unpaused |
|-----------|--------|----------|
| mint | REVERTS | Allowed |
| burn | REVERTS | Allowed |
| transfer | **Allowed** | Allowed |
| transferFrom | **Allowed** | Allowed |
| approve | **Allowed** | Allowed |
| updateMinter | Allowed | Allowed |

> Note: The pause only blocks `mint` and `burn` in LPToken. This differs from GMIToken, which also blocks `transfer` and `approve`.

---

## Behavioral Invariants

1. `totalSupply` = sum of all `balanceOf` values at all times
2. `totalSupply` can only increase via `mint` (by minters/owner)
3. `totalSupply` can only decrease via `burn` (by minters/owner)
4. `minters[x]` is always the opposite of the last `updateMinter(x, !)` call
5. Pause only affects `mint` and `burn` — not user-facing transfers
6. Only the `owner` can change the pause state and minter registry
7. A single `owner` address holds all governance power (no multi-sig)
