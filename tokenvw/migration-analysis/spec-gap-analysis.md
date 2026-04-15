# Spec Gap Analysis

Comparison between `erc20-spec-from-source.md` (derived from EVM code) and `erc20-specification.md` (existing doc).

---

## GAP-1: Missing Functions — `renounceOwnership`

**Source truth:** OwnableUpgradeable.sol:59-61 exposes `renounceOwnership()` as a public function. It sets `_owner = address(0)`, permanently disabling all `onlyOwner` functions.

**erc20-specification.md:** Does not mention `renounceOwnership` at all.

**Severity:** MEDIUM — This is a callable function on the deployed EVM contract. Its absence from the spec means it was never considered during migration.

---

## GAP-2: Missing Functions — `increaseAllowance` and `decreaseAllowance`

**Source truth:** ERC20Upgradeable.sol:186-215 exposes both as public functions. They provide race-condition-safe alternatives to `approve`.

**erc20-specification.md:** Does not mention either function.

**Severity:** LOW — These are convenience functions from OpenZeppelin. SPL Token does not have equivalents. Since LPToken does not override them and they have no custom behavior, the gap is acceptable as long as the design doc acknowledges them.

---

## GAP-3: Missing Functions — `owner()`, `paused()`, `minters()`, `chainId()` view functions

**Source truth:** All four are public view functions on the EVM contract. `owner()` and `paused()` from OwnableUpgradeable/PausableUpgradeable. `minters()` and `chainId()` are auto-generated getters from public state variables.

**erc20-specification.md:** `owner` and `paused` listed only as state variables. `minters` listed as state variable. `chainId` listed as state variable. None documented as callable functions.

**Severity:** LOW — On Solana, account data is directly readable. The Anchor IDL auto-generates fetch methods for `TokenState` and `MinterRecord` accounts. Functional parity is achieved by different means.

---

## GAP-4: Incorrect Claim — `initialize` edge case: "owner_ cannot be zero address"

**Source truth:** `_transferOwnership(owner_)` in OwnableUpgradeable.sol:76-80 does NOT check for address(0). It unconditionally sets `_owner = newOwner`. Only `transferOwnership()` (the public function) has the zero-address check.

**erc20-specification.md, line 77:** States `owner_ cannot be zero address` as an edge case. This is **incorrect**. The EVM `initialize()` function WILL accept `address(0)` as `owner_`.

**Severity:** LOW — In practice, deployers will never initialize with address(0), but the spec asserts a constraint that does not exist in the code.

---

## GAP-5: Missing Behavior — `OwnershipTransferred` emitted twice during initialize

**Source truth:** `__Ownable_init()` emits `OwnershipTransferred(address(0), msg.sender)`, then `_transferOwnership(owner_)` emits `OwnershipTransferred(msg.sender, owner_)`.

**erc20-specification.md, line 73:** States "Events: None directly (Transfer(0,0,0) implicit from ERC20 init)". This is doubly wrong:
1. `__ERC20_init` does NOT emit any Transfer event.
2. Two `OwnershipTransferred` events ARE emitted.

**Severity:** LOW — Event accuracy is important for indexing but does not affect program logic.

---

## GAP-6: Missing Behavior — Infinite allowance pattern

**Source truth:** ERC20Upgradeable._spendAllowance (line 340-346): If `currentAllowance == type(uint256).max`, the allowance is NOT decremented on `transferFrom`. This is an explicit "infinite approval" optimization.

**erc20-specification.md:** Not mentioned anywhere.

**Severity:** LOW for Solana migration — SPL Token has no infinite-allowance concept. Delegate amounts are always finite and always decremented. This is an inherent platform difference.

---

## GAP-7: Missing Modifier Detail — Modifier execution order on `pause()` and `mint()`

**Source truth:**
- `pause()` modifiers: `whenNotPaused onlyOwner` → executed left-to-right
- `mint()` modifiers: `onlyMintersOrOwner whenNotPaused nonReentrant` → executed left-to-right

**erc20-specification.md:** Lists modifiers but does not specify execution order. The order matters for determining which error message a caller receives.

**Severity:** INFORMATIONAL — Does not affect functional parity.

---

## GAP-8: Missing Edge Case — `updateMinter(address(0), true)` succeeds

**Source truth:** `updateMinter` has no zero-address check on `_account`. Registering `address(0)` as a minter succeeds on EVM. However, no one can call from `address(0)`, so this has no practical effect.

**erc20-specification.md:** Not mentioned.

**Severity:** INFORMATIONAL — No practical impact. On Solana, `Pubkey::default()` can similarly be registered as a minter but no one holds that key.

---

## GAP-9: Missing Function — `transferOwnership` zero-address check

**Source truth:** OwnableUpgradeable.sol:68: `require(newOwner != address(0), "Ownable: new owner is the zero address")`

**erc20-specification.md:** Does not document `transferOwnership` at all (only mentions it as a concept in the Access Control section).

**Severity:** MEDIUM — This is a critical governance function missing from the spec.

---

## Summary

| Gap | Description | Severity |
|-----|-------------|----------|
| GAP-1 | `renounceOwnership` not documented | MEDIUM |
| GAP-2 | `increaseAllowance`/`decreaseAllowance` not documented | LOW |
| GAP-3 | View function getters not documented as functions | LOW |
| GAP-4 | Incorrect: claims `owner_` cannot be zero in initialize | LOW |
| GAP-5 | Missing: two OwnershipTransferred events during initialize | LOW |
| GAP-6 | Missing: infinite allowance pattern | LOW |
| GAP-7 | Missing: modifier execution order | INFORMATIONAL |
| GAP-8 | Missing: updateMinter(address(0)) succeeds | INFORMATIONAL |
| GAP-9 | `transferOwnership` not documented | MEDIUM |
