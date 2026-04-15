# EVM Token Contracts -- System Overview

## Contracts Discovered

| # | File | Contract Name | Type |
|---|---|---|---|
| 1 | `lp-token/LPToken.sol` | LPToken | Implementation (upgradeable) |
| 2 | `lp-token/interfaces/IGMIToken.sol` | IGMIToken | Interface |
| 3 | `lp-token/proxy/LPTokenProxy.sol` | LPTokenProxy | Transparent proxy |
| 4 | `lp-token/proxy/LPTokenProxyAdmin.sol` | LPTokenProxyAdmin | Proxy admin |
| 5 | `gmi-token/GMIToken.sol` | GMIToken | Implementation (upgradeable) |
| 6 | `gmi-token/interfaces/IGMIToken.sol` | IGMIToken | Interface |
| 7 | `gmi-token/proxy/GMITokenProxy.sol` | GMITokenProxy | Transparent proxy |
| 8 | `gmi-token/proxy/GMITokenProxyAdmin.sol` | GMITokenProxyAdmin | Proxy admin |
| 9 | `gmi-cv-token/GMICVToken.sol` | GMICVToken | Implementation (upgradeable) |
| 10 | `gmi-cv-token/interfaces/IGMICVToken.sol` | IGMICVToken | Interface |
| 11 | `gmi-cv-token/proxy/GMICVTokenProxy.sol` | GMICVTokenProxy | Transparent proxy |
| 12 | `gmi-cv-token/proxy/GMICVTokenProxyAdmin.sol` | GMICVTokenProxyAdmin | Proxy admin |
| 13 | `test/DummyExchange.sol` | DummyExchange | Test helper |

All contracts use **Solidity 0.8.22** (`pragma solidity 0.8.22`).

## Inheritance Hierarchy

### LPToken (Migration Target)

```
LPToken
  |-- ERC20Upgradeable
  |     |-- Initializable
  |     |-- ContextUpgradeable
  |     |-- IERC20Upgradeable
  |     |-- IERC20MetadataUpgradeable
  |-- OwnableUpgradeable
  |     |-- Initializable
  |     |-- ContextUpgradeable
  |-- PausableUpgradeable
  |     |-- Initializable
  |     |-- ContextUpgradeable
  |-- ReentrancyGuardUpgradeable
        |-- Initializable
```

### GMIToken

```
GMIToken
  |-- ERC20Upgradeable
  |-- OwnableUpgradeable
  |-- PausableUpgradeable
  |-- ReentrancyGuardUpgradeable
```

(Same base chain as LPToken, plus maxSupply and _transfer/_approve overrides)

### GMICVToken

```
GMICVToken
  |-- ERC20Upgradeable
  |-- OwnableUpgradeable
  |-- PausableUpgradeable
  |-- ReentrancyGuardUpgradeable
```

(Same base chain as GMIToken, plus trading restrictions via allowedExchanges)

### Proxy Contracts

```
LPTokenProxy --> TransparentUpgradeableProxy --> ERC1967Proxy --> Proxy
GMITokenProxy --> TransparentUpgradeableProxy --> ERC1967Proxy --> Proxy
GMICVTokenProxy --> TransparentUpgradeableProxy --> ERC1967Proxy --> Proxy

LPTokenProxyAdmin --> ProxyAdmin --> Ownable
GMITokenProxyAdmin --> ProxyAdmin --> Ownable
GMICVTokenProxyAdmin --> ProxyAdmin --> Ownable
```

## Deployment Architecture

Each token is deployed as a **three-contract system**:

```
                     +-------------------+
                     |   ProxyAdmin      |  (owner controls upgrades)
                     +-------------------+
                              |
                              | admin
                              v
  User  ------>  [TokenProxy (TransparentUpgradeableProxy)]
                              |
                              | delegatecall
                              v
                     +-------------------+
                     |  Token (impl)     |  (logic contract)
                     +-------------------+
```

- **Proxy**: Holds all storage; delegates all calls to implementation
- **ProxyAdmin**: Only account that can call admin functions on the proxy (upgrade, changeAdmin)
- **Implementation**: Stateless logic; initialized via `initialize()` called through the proxy

## Key Architectural Differences Between Tokens

| Feature | LPToken | GMIToken | GMICVToken |
|---|---|---|---|
| Max supply cap | **No** | Yes (`maxSupply`) | Yes (`maxSupply`) |
| `updateMaxSupply()` | **No** | **No** (missing from impl, present in interface) | Yes |
| Pause blocks transfers | **No** | Yes (`_transfer` override) | Yes (`_transfer` override) |
| Pause blocks approvals | **No** | Yes (`_approve` override) | Yes (`_approve` override) |
| Pause blocks mint/burn | Yes | Yes | Yes |
| Trading restrictions | No | No | Yes (`tradeAllowed`, `allowedExchanges`) |
| `mint()` returns bool | Yes | Yes | **No** |
| `burn()` returns bool | Yes | Yes | **No** |

## Critical Observation: LPToken Pause Scope

LPToken does NOT override `_transfer()` or `_approve()`. This means:
- **Pausing only blocks `mint()` and `burn()`** (which have `whenNotPaused`)
- **Standard ERC20 transfers and approvals continue to work when paused**
- This is a deliberate design choice distinguishing LPToken from GMIToken/GMICVToken

## Interface Anomaly

The `lp-token/interfaces/IGMIToken.sol` interface includes `maxSupply()` and `updateMaxSupply()` which do NOT exist on the LPToken implementation. This interface appears to be copied from the GMIToken interface directory and is not a faithful representation of LPToken's actual interface.
