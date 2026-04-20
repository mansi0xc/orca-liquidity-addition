# ERC20 Discovery — Energi Core Token Contracts

## Repository Location

`/Users/mansitibrewal/chronicles/egmi-solana/evm-contracts/token/`

---

## Contract Files

| File | Purpose |
|------|---------|
| `contracts/gmi-token/GMIToken.sol` | Primary GMI governance token with max-supply enforcement |
| `contracts/gmi-cv-token/GMICVToken.sol` | Collector-version token with trading restrictions and exchange whitelist |
| `contracts/lp-token/LPToken.sol` | Liquidity-provider bond token — **migration target** |
| `contracts/gmi-token/interfaces/IGMIToken.sol` | Interface for GMIToken |
| `contracts/gmi-cv-token/interfaces/IGMICVToken.sol` | Interface for GMICVToken |
| `contracts/lp-token/interfaces/IGMIToken.sol` | Interface for LPToken (shares IGMIToken shape) |
| `contracts/gmi-token/proxy/GMITokenProxy.sol` | TransparentUpgradeableProxy for GMIToken |
| `contracts/gmi-token/proxy/GMITokenProxyAdmin.sol` | ProxyAdmin for GMIToken |
| `contracts/gmi-cv-token/proxy/GMICVTokenProxy.sol` | TransparentUpgradeableProxy for GMICVToken |
| `contracts/gmi-cv-token/proxy/GMICVTokenProxyAdmin.sol` | ProxyAdmin for GMICVToken |
| `contracts/lp-token/proxy/LPTokenProxy.sol` | TransparentUpgradeableProxy for LPToken |
| `contracts/lp-token/proxy/LPTokenProxyAdmin.sol` | ProxyAdmin for LPToken |
| `contracts/test/DummyExchange.sol` | Test fixture for trading restriction tests |

---

## Inherited Dependencies

All three tokens share the same inheritance chain from OpenZeppelin Contracts-Upgradeable v4.5.0:

```
LPToken / GMIToken / GMICVToken
    └── ERC20Upgradeable          (balances, allowances, transfer, approve)
    └── OwnableUpgradeable        (owner role, transferOwnership)
    └── PausableUpgradeable       (paused flag, whenNotPaused modifier)
    └── ReentrancyGuardUpgradeable (nonReentrant modifier)
```

Proxy pattern:
```
TransparentUpgradeableProxy (OZ)
    └── GMITokenProxy / GMICVTokenProxy / LPTokenProxy

ProxyAdmin (OZ)
    └── GMITokenProxyAdmin / GMICVTokenProxyAdmin / LPTokenProxyAdmin
```

---

## Dependency Map

```
LPToken (Migration Target)
│
├── State
│   ├── minters: mapping(address => bool)   — authorized mint/burn callers
│   ├── chainId: uint256                    — EVM network identifier
│   └── [paused: bool]                      — inherited from PausableUpgradeable
│
├── Access Control
│   ├── onlyOwner          (OwnableUpgradeable)
│   └── onlyMintersOrOwner (custom modifier)
│
├── Lifecycle Guards
│   ├── whenNotPaused      (PausableUpgradeable)
│   └── nonReentrant       (ReentrancyGuardUpgradeable)
│
├── Core ERC20 (ERC20Upgradeable)
│   ├── balanceOf, totalSupply, decimals (18)
│   ├── transfer, transferFrom, approve, allowance
│   └── _mint, _burn (internal)
│
└── Custom Functions
    ├── mint(address, uint256)        — onlyMintersOrOwner, whenNotPaused, nonReentrant
    ├── burn(address, uint256)        — onlyMintersOrOwner, whenNotPaused, nonReentrant
    ├── updateMinter(address, bool)   — onlyOwner
    ├── pause()                       — onlyOwner, whenNotPaused
    ├── unpause()                     — onlyOwner, whenPaused
    └── impl()                        — view, returns address(this)

GMIToken (adds max-supply enforcement)
└── Everything in LPToken PLUS:
    ├── maxSupply: uint256
    ├── MaxSupplyUpdated event
    ├── mint() checks: totalSupply + amount <= maxSupply
    ├── _transfer() override: whenNotPaused, nonReentrant
    └── _approve() override: whenNotPaused, nonReentrant

GMICVToken (adds trading restrictions)
└── Everything in GMIToken PLUS:
    ├── allowedExchanges: mapping(address => bool)
    ├── tradeAllowed: bool
    ├── AllowedExchangesUpdated event
    ├── TradeAllowUpdated event
    ├── tradeAllowance modifier (blocks transfers to/from non-whitelisted contracts)
    ├── approveAllowance modifier (blocks approvals to non-whitelisted contracts)
    ├── updateAllowedExchange(address, bool) — onlyOwner
    ├── allowTrade()                         — onlyOwner, whenNotPaused (toggle)
    └── _isContract(address) internal view
```

---

## Key Observations

### LPToken Specific Behaviors

1. **No max-supply cap** — LPToken has no `maxSupply` field. Mint is unbounded (only limited by uint256 overflow).

2. **Transfer NOT guarded by pause** — Unlike GMIToken which overrides `_transfer` with `whenNotPaused`, LPToken does NOT override `_transfer` or `_approve`. Regular `transfer`, `transferFrom`, and `approve` calls proceed regardless of pause state.

3. **Mint and burn ARE guarded by pause** — Only the custom `mint()` and `burn()` functions check `whenNotPaused`.

4. **Minter can burn from any address** — `burn(address _account, uint256 _amount)` allows a minter to reduce the balance of ANY address without that address's consent. This is a superpowered burn beyond standard ERC20 allowances.

5. **Upgradeable via TransparentProxy** — Production deployment uses a proxy, so the implementation can be upgraded by the ProxyAdmin.

6. **chainId tracking** — Stored for multi-chain deployment awareness.

### Deployment Context (from deploy-lptoken.js)

- Token Name: `"HELLO-GMI LP Bond L4"`
- Symbol: `"HELLO-GMI-L4"`
- Minters: LOCKER contract + EXCHANGE contract
- Initial mint: 1 LP token minted to owner
- Exchange and Locker contracts call `mint`/`burn` when users open/close bond positions

---

## External Integrations

| Integration | Type | Notes |
|-------------|------|-------|
| Locker Contract | Minter | Mints LP tokens when user locks NFT bonds |
| Exchange Contract | Minter | Mints/burns LP tokens during bond lifecycle |
| ProxyAdmin | Upgrader | Controls implementation upgrades |
| OpenZeppelin v4.5.0 | Library | ERC20Upgradeable, Ownable, Pausable, ReentrancyGuard |
