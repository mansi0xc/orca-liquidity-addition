# Proxy Contracts -- Analysis

All three tokens use an identical proxy architecture. This document covers all six proxy-related contracts.

---

## LPTokenProxy / GMITokenProxy / GMICVTokenProxy

**Inheritance**: `TransparentUpgradeableProxy -> ERC1967Proxy -> Proxy`

All three are identical in structure, differing only in contract name.

### Constructor

| Field | Detail |
|---|---|
| **Signature** | `constructor(address logic, address admin, bytes memory data)` |
| **Parameters** | `logic`: implementation contract address; `admin`: ProxyAdmin contract address; `data`: ABI-encoded calldata for initialization (typically `abi.encodeCall(Token.initialize, (...))`) |
| **Behavior** | Stores `logic` in ERC1967 implementation slot, stores `admin` in ERC1967 admin slot, if `data.length > 0` performs a delegatecall to `logic` with `data` |
| **Events** | `Upgraded(logic)`, `AdminChanged(address(0), admin)` |

### Inherited Behavior (TransparentUpgradeableProxy)

The TransparentUpgradeableProxy pattern works as follows:

1. **Admin calls**: If `msg.sender == admin`, the proxy handles the call directly (admin functions: `upgradeTo`, `upgradeToAndCall`, `changeAdmin`, `admin`, `implementation`). The call is NOT delegated.
2. **Non-admin calls**: ALL calls are delegated to the implementation via `delegatecall`. The admin CANNOT call implementation functions through the proxy.

### Admin Functions (available only to ProxyAdmin contract)

| Function | Purpose |
|---|---|
| `upgradeTo(address newImplementation)` | Changes the implementation address |
| `upgradeToAndCall(address newImplementation, bytes data)` | Changes implementation and calls initialization |
| `changeAdmin(address newAdmin)` | Changes the admin address |
| `admin()` | Returns current admin |
| `implementation()` | Returns current implementation |

---

## LPTokenProxyAdmin / GMITokenProxyAdmin / GMICVTokenProxyAdmin

**Inheritance**: `ProxyAdmin -> Ownable`

All three are identical in structure.

### Constructor

| Field | Detail |
|---|---|
| **Signature** | `constructor(address owner_)` |
| **Parameters** | `owner_`: address that will own the ProxyAdmin |
| **Behavior** | Calls `_transferOwnership(owner_)` to set the owner |
| **Note** | OpenZeppelin's `ProxyAdmin` constructor also calls `_transferOwnership(msg.sender)`, so the constructor first sets owner to deployer, then immediately transfers to `owner_`. This results in two `OwnershipTransferred` events. |

### Inherited Functions (ProxyAdmin)

| Function | Signature | Access | Purpose |
|---|---|---|---|
| `getProxyImplementation` | `function getProxyImplementation(ITransparentUpgradeableProxy proxy) public view returns (address)` | Anyone | Returns the implementation of a proxy |
| `getProxyAdmin` | `function getProxyAdmin(ITransparentUpgradeableProxy proxy) public view returns (address)` | Anyone | Returns the admin of a proxy |
| `changeProxyAdmin` | `function changeProxyAdmin(ITransparentUpgradeableProxy proxy, address newAdmin) public onlyOwner` | Owner | Changes the admin of the proxy |
| `upgrade` | `function upgrade(ITransparentUpgradeableProxy proxy, address implementation) public onlyOwner` | Owner | Upgrades the proxy implementation |
| `upgradeAndCall` | `function upgradeAndCall(ITransparentUpgradeableProxy proxy, address implementation, bytes memory data) public payable onlyOwner` | Owner | Upgrades and calls |

### Inherited Functions (Ownable)

| Function | Purpose |
|---|---|
| `owner()` | Returns owner |
| `transferOwnership(address)` | Transfers ownership |
| `renounceOwnership()` | Renounces ownership (IRREVERSIBLE -- would lock upgrade capability) |

---

## Security Notes

1. **ProxyAdmin owner** is the ultimate authority -- can upgrade implementation to arbitrary code
2. If `renounceOwnership()` is called on ProxyAdmin, upgrades become impossible (proxy is frozen)
3. No timelock on upgrades -- changes are immediate
4. No multisig requirement -- single address controls upgrades
5. The ProxyAdmin uses non-upgradeable `Ownable` (not `OwnableUpgradeable`) since it is not behind a proxy itself

---

## DummyExchange (Test Contract)

**File**: `test/DummyExchange.sol`

A simple test helper for GMICVToken's trading restriction tests.

### State Variables
| Variable | Type | Purpose |
|---|---|---|
| `gmiToken` | `address` | The token contract address |

### Functions

#### `constructor(address gmiToken_)`
Sets the token address.

#### `send(address to_, uint256 _amount)`
| Field | Detail |
|---|---|
| **Signature** | `function send(address to_, uint256 _amount) external` |
| **Purpose** | Transfers tokens held by this contract to a recipient |
| **Access control** | None (anyone can call) |
| **Behavior** | Calls `IERC20(gmiToken).transfer(to_, _amount)` |
| **Note** | Used in tests to verify that contract addresses are subject to `allowedExchanges` restrictions in GMICVToken |
