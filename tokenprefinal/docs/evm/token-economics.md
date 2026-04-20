# Token Economics -- All Contracts

## Supply Mechanics Comparison

| Property | LPToken | GMIToken | GMICVToken |
|---|---|---|---|
| Initial supply | 0 (no premint) | 0 (no premint) | 0 (no premint) |
| Max supply cap | **None** (unbounded) | `maxSupply` (set at init) | `maxSupply` (set at init, updatable) |
| Can update max supply | N/A | **No** (no function exists) | Yes (`updateMaxSupply()`) |
| Mintable | Yes | Yes (capped) | Yes (capped) |
| Burnable | Yes | Yes | Yes |
| Decimals | 18 | 18 | 18 |
| Precision range | uint256 (0 to 2^256 - 1) | uint256 (0 to maxSupply) | uint256 (0 to maxSupply) |

## Minting

### Who Can Mint
All three tokens use the same `onlyMintersOrOwner` modifier:
- The contract **owner** (single address)
- Any address in the `minters` mapping (set to `true` by owner via `updateMinter()`)

### Conditions for Minting
- Contract must NOT be paused (`whenNotPaused`)
- Caller must be owner or minter
- (GMIToken/GMICVToken only) `totalSupply() + _amount <= maxSupply`
- `_account != address(0)` (OZ internal check)

### Mint Behavior
- Increases `_totalSupply` by `_amount`
- Increases `_balances[_account]` by `_amount`
- Emits `Transfer(address(0), _account, _amount)`
- LPToken/GMIToken: returns `bool true`
- GMICVToken: returns void

### Mint Limits
- LPToken: No upper bound. Can mint up to `uint256.max` total.
- GMIToken: Capped at `maxSupply`. Cannot be changed post-deployment.
- GMICVToken: Capped at `maxSupply`. Can be changed by owner.

## Burning

### Who Can Burn
Same as minting: owner or minters.

### Burn Mechanics -- CRITICAL DETAIL
**Burns do NOT require token holder approval.** A minter or owner can burn tokens from ANY address without the holder's consent. The `burn(address _account, uint256 _amount)` function calls `_burn(_account, _amount)` directly, which:
- Requires `_balances[_account] >= _amount`
- Decreases `_balances[_account]` by `_amount`
- Decreases `_totalSupply` by `_amount`
- Emits `Transfer(_account, address(0), _amount)`

There is no `_spendAllowance` check in any of the three tokens' burn functions.

### Burn From vs Self-Burn
These tokens do NOT have separate `burnFrom()` functions. The single `burn()` function serves both purposes. Any minter/owner can burn any account's tokens.

## Transfer Restrictions

### LPToken
- **No transfer restrictions when paused.** Transfers and approvals work at all times.
- No blacklist/whitelist on transfers.
- No fees on transfer.

### GMIToken
- **All transfers blocked when paused** (via `_transfer` override with `whenNotPaused`).
- **All approvals blocked when paused** (via `_approve` override with `whenNotPaused`).
- No blacklist/whitelist on transfers.
- No fees on transfer.
- **Bug**: `transferFrom()` broken for finite allowances due to `nonReentrant` on both `_transfer` and `_approve`.

### GMICVToken
- **All transfers blocked when paused.**
- **All approvals blocked when paused.**
- **Trading restrictions**: When `tradeAllowed == false`, contract addresses involved in transfers must be in `allowedExchanges`. EOA-to-EOA transfers unaffected.
- **Approval restrictions**: When `tradeAllowed == false`, approving a contract address requires it to be in `allowedExchanges`.
- No fees on transfer.
- **Same `transferFrom()` bug as GMIToken.**

## Fee Mechanisms

**None.** No transfer fees, mint fees, burn fees, or any other fee mechanisms exist in any of the three tokens.

## Pause Mechanics Summary

| Operation | LPToken (paused) | GMIToken (paused) | GMICVToken (paused) |
|---|---|---|---|
| `mint()` | BLOCKED | BLOCKED | BLOCKED |
| `burn()` | BLOCKED | BLOCKED | BLOCKED |
| `transfer()` | **ALLOWED** | BLOCKED | BLOCKED |
| `transferFrom()` | **ALLOWED** | BLOCKED | BLOCKED |
| `approve()` | **ALLOWED** | BLOCKED | BLOCKED |
| `increaseAllowance()` | **ALLOWED** | BLOCKED | BLOCKED |
| `decreaseAllowance()` | **ALLOWED** | BLOCKED | BLOCKED |
| `updateMinter()` | ALLOWED | ALLOWED | ALLOWED |
| `updateMaxSupply()` | N/A | N/A | ALLOWED |
| `updateAllowedExchange()` | N/A | N/A | ALLOWED |
| `allowTrade()` | N/A | N/A | BLOCKED |

## Solana Migration Notes (LPToken-specific)

| EVM Property | Value | Solana Equivalent | Notes |
|---|---|---|---|
| Decimals | 18 | 9 | Smaller precision on Solana; 1 token = 10^18 vs 10^9 smallest units |
| Max value | uint256 (2^256-1) | u64 (2^64-1) | Solana u64 max ~18.4 * 10^18; with 9 decimals, max ~18.4 billion tokens |
| Supply cap | None | None (per design) | Both are uncapped |
| Allowance model | Approve/transferFrom | Delegate/CPI | Different mechanical pattern |
| Burn authorization | Minter burns any account | Dual-signer required | Solana improves security by requiring token holder signature |
| Pause scope | Mint/burn only | Mint/burn only | Preserved in Solana migration |
