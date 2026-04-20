# Access Control -- Complete Matrix

## Role Definitions

### Implementation Contracts (LPToken, GMIToken, GMICVToken)

| Role | Identifier | Storage | Initial Holder | Can Be Transferred |
|---|---|---|---|---|
| Owner | `_owner` (address) | OwnableUpgradeable storage | `owner_` parameter in `initialize()` | Yes (`transferOwnership`) or renounced (`renounceOwnership`) |
| Minter | `minters[address]` (mapping) | Contract storage | None (must be explicitly granted) | No (can only be granted/revoked by owner) |

### Proxy Contracts (ProxyAdmin)

| Role | Identifier | Storage | Initial Holder | Can Be Transferred |
|---|---|---|---|---|
| ProxyAdmin Owner | `_owner` (address) | Ownable storage | `owner_` parameter in constructor | Yes (`transferOwnership`) or renounced |

## Role Hierarchy

```
ProxyAdmin Owner
  |
  +--> Can upgrade implementation (changes ALL logic)
  +--> Can change proxy admin
  |
Token Owner (set in implementation, stored in proxy storage)
  |
  +--> Can grant/revoke Minter role
  +--> Can pause/unpause
  +--> Can mint/burn (same as Minter)
  +--> Can transfer ownership
  +--> Can renounce ownership
  |    (GMICVToken only:)
  +--> Can update max supply
  +--> Can update allowed exchanges
  +--> Can toggle trade allowed
  |
Minter
  |
  +--> Can mint tokens
  +--> Can burn ANY account's tokens
```

## Complete Function-Level Permission Matrix

### LPToken

| Function | Owner | Minter | ProxyAdmin Owner | Anyone |
|---|---|---|---|---|
| `initialize()` | Once (initializer) | - | - | - |
| `mint()` | Yes* | Yes* | No | No |
| `burn()` | Yes* | Yes* | No | No |
| `updateMinter()` | Yes | No | No | No |
| `pause()` | Yes** | No | No | No |
| `unpause()` | Yes*** | No | No | No |
| `transfer()` | Yes | Yes | No | Yes |
| `transferFrom()` | Yes | Yes | No | Yes (with allowance) |
| `approve()` | Yes | Yes | No | Yes |
| `increaseAllowance()` | Yes | Yes | No | Yes |
| `decreaseAllowance()` | Yes | Yes | No | Yes |
| `transferOwnership()` | Yes | No | No | No |
| `renounceOwnership()` | Yes | No | No | No |
| `impl()` | Yes | Yes | No | Yes |
| `name()` | Yes | Yes | No | Yes |
| `symbol()` | Yes | Yes | No | Yes |
| `decimals()` | Yes | Yes | No | Yes |
| `totalSupply()` | Yes | Yes | No | Yes |
| `balanceOf()` | Yes | Yes | No | Yes |
| `allowance()` | Yes | Yes | No | Yes |
| `owner()` | Yes | Yes | No | Yes |
| `paused()` | Yes | Yes | No | Yes |
| `minters()` | Yes | Yes | No | Yes |
| `chainId()` | Yes | Yes | No | Yes |

`*` = requires `whenNotPaused`
`**` = requires `whenNotPaused`
`***` = requires `whenPaused`

### GMIToken (differences from LPToken)

| Function | Owner | Minter | Anyone | Notes |
|---|---|---|---|---|
| `transfer()` | Yes* | Yes* | Yes* | Blocked when paused (unlike LPToken) |
| `transferFrom()` | Yes* | Yes* | Yes* (broken for finite allowances) | `nonReentrant` bug |
| `approve()` | Yes* | Yes* | Yes* | Blocked when paused |
| `increaseAllowance()` | Yes* | Yes* | Yes* | Blocked when paused |
| `decreaseAllowance()` | Yes* | Yes* | Yes* | Blocked when paused |
| `maxSupply()` | Yes | Yes | Yes | View function |

### GMICVToken (additional functions)

| Function | Owner | Minter | Anyone | Notes |
|---|---|---|---|---|
| `updateAllowedExchange()` | Yes | No | No | |
| `updateMaxSupply()` | Yes | No | No | |
| `allowTrade()` | Yes* | No | No | Requires whenNotPaused |
| `allowedExchanges()` | Yes | Yes | Yes | View function |
| `tradeAllowed()` | Yes | Yes | Yes | View function |

### ProxyAdmin Contracts

| Function | ProxyAdmin Owner | Anyone |
|---|---|---|
| `upgrade()` | Yes | No |
| `upgradeAndCall()` | Yes | No |
| `changeProxyAdmin()` | Yes | No |
| `getProxyImplementation()` | Yes | Yes |
| `getProxyAdmin()` | Yes | Yes |
| `transferOwnership()` | Yes | No |
| `renounceOwnership()` | Yes | No |
| `owner()` | Yes | Yes |

## Role Granting and Revocation

### Owner Role
- **Granted**: Via `transferOwnership(newOwner)` (only current owner can call)
- **Revoked**: Via `renounceOwnership()` (sets to address(0), irreversible)
- **Initial**: Set in `initialize()` via `_transferOwnership(owner_)`
- **Validation**: `transferOwnership()` checks `newOwner != address(0)`

### Minter Role
- **Granted**: Via `updateMinter(account, true)` (only owner)
- **Revoked**: Via `updateMinter(account, false)` (only owner)
- **Initial**: No minters at initialization
- **Validation**: Duplicate check prevents setting same value twice
- **Note**: Minters cannot self-revoke; only owner can manage minter status

## Centralization Risk Assessment

| Risk | Severity | Description |
|---|---|---|
| Single owner controls all admin functions | HIGH | No multisig, no timelock, no governance |
| Owner can add arbitrary minters | HIGH | Minters can then mint unlimited tokens (LPToken) or burn any account |
| Minters can burn without holder consent | HIGH | Any minter can destroy any user's balance |
| ProxyAdmin owner can upgrade logic | CRITICAL | Can change all contract behavior with no warning |
| Owner can pause/unpause instantly | MEDIUM | No timelock on pause state changes |
| Ownership transfer is immediate | MEDIUM | No two-step transfer pattern (transferOwnership + acceptOwnership) |
| renounceOwnership is irreversible | LOW | Could accidentally lock administrative functions |
