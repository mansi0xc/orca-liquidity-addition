# Interface Contracts -- Analysis

## IGMIToken (used by both lp-token and gmi-token directories)

**File**: `lp-token/interfaces/IGMIToken.sol` and `gmi-token/interfaces/IGMIToken.sol`
**Pragma**: `solidity 0.8.22`

Both files are **identical**. The interface declares:

| Function | Parameters | Returns |
|---|---|---|
| `minters(address)` | `_account` | `bool` |
| `chainId()` | none | `uint256` |
| `maxSupply()` | none | `uint256` |
| `initialize(...)` | `name_`, `symbol_`, `owner_`, `chainId_`, `maxSupply_` | void |
| `mint(address, uint256)` | `_account`, `_amount` | void |
| `burn(address, uint256)` | `_account`, `_amount` | void |
| `updateMinter(address, bool)` | `_account`, `_isMinter` | void |
| `updateMaxSupply(uint256)` | `_newMaxSupply` | void |
| `pause()` | none | void |
| `unpause()` | none | void |

### Discrepancies with Implementations

**LPToken vs IGMIToken**:
- `maxSupply()` -- declared in interface but NOT in LPToken
- `updateMaxSupply()` -- declared in interface but NOT in LPToken
- `initialize()` -- interface has 5 params (includes `maxSupply_`); LPToken has 4 params (no `maxSupply_`)
- `mint()`/`burn()` -- interface returns void; LPToken returns `bool`

**GMIToken vs IGMIToken**:
- `updateMaxSupply()` -- declared in interface but NOT in GMIToken implementation
- `mint()`/`burn()` -- interface returns void; GMIToken returns `bool`

**Conclusion**: The IGMIToken interface is not a faithful representation of either LPToken or GMIToken. It appears to be a desired specification that was not fully implemented.

---

## IGMICVToken

**File**: `gmi-cv-token/interfaces/IGMICVToken.sol`
**Pragma**: `solidity 0.8.22`

| Function | Parameters | Returns |
|---|---|---|
| `minters(address)` | `_account` | `bool` |
| `allowedExchanges(address)` | `_exchange` | `bool` |
| `chainId()` | none | `uint256` |
| `maxSupply()` | none | `uint256` |
| `tradeAllowed()` | none | `bool` |
| `initialize(...)` | `name_`, `symbol_`, `owner_`, `chainId_`, `maxSupply_` | void |
| `mint(address, uint256)` | `_account`, `_amount` | void |
| `burn(address, uint256)` | `_account`, `_amount` | void |
| `updateMinter(address, bool)` | `_account`, `_isMinter` | void |
| `updateMaxSupply(uint256)` | `_newMaxSupply` | void |
| `pause()` | none | void |
| `unpause()` | none | void |

### Discrepancies with GMICVToken
- Missing `updateAllowedExchange()` from interface
- Missing `allowTrade()` from interface
- Missing `impl()` from interface
- Otherwise matches implementation signatures
