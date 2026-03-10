# 02 — Contract-Level Breakdown

---

## 1. StorageBase

**File:** `contracts/StorageBase.sol`

### Purpose
Base contract for all storage contracts. Provides a simple `owner`-gated access control pattern for storage mutation.

### Responsibilities
- Store the `owner` address (the implementation contract that created it).
- Restrict write access to the owner.
- Allow ownership transfer.

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `owner` | `address payable` | `internal` | The address authorized to call setter functions (typically the implementation contract) |

### Functions
| Function | Visibility | Modifiers | Description |
|---|---|---|---|
| `constructor()` | — | — | Sets `owner` to `msg.sender` |
| `setOwner(address _newOwner)` | `external` | `requireOwner` | Transfers storage ownership |

### Modifiers
| Modifier | Description |
|---|---|
| `requireOwner` | Reverts if `msg.sender != owner` |

### Access Control
- `owner`: Only the contract that deployed the storage (i.e., the implementation) can write to it.

### Critical Logic for Solana
- The ownership model maps to a program-derived authority pattern on Solana.

---

## 2. UpgradeManager

**File:** `contracts/access/UpgradeManager.sol`

### Purpose
Provides an `upgradeManager` role that is authorized to approve UUPS proxy upgrades. Separates upgrade authority from general ownership.

### Responsibilities
- Store the `upgradeManager` address.
- Gate upgrade authorization to `upgradeManager`.
- Allow the `owner` to change the `upgradeManager`.

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `upgradeManager` | `address` | `public` | Address authorized to approve upgrades (typically a multisig) |

### Inheritance
- `Initializable` (OpenZeppelin)
- `OwnableUpgradeable` (OpenZeppelin)

### Functions
| Function | Visibility | Modifiers | Description |
|---|---|---|---|
| `__UpgradeManager_init(address, address)` | `internal` | `onlyInitializing` | Sets `upgradeManager` and initializes `Ownable` |
| `setUpgradeManager(address)` | `external` | `onlyOwner` | Changes the upgrade manager address |

### Modifiers
| Modifier | Description |
|---|---|
| `onlyUpgradeManager` | Reverts if `msg.sender != upgradeManager` |

### Critical Logic for Solana
- On Solana, programs are upgraded by the program's upgrade authority. This maps to Anchor's `upgrade_authority` key. No explicit `UpgradeManager` contract is needed, but the concept should be replicated via a multisig upgrade authority.

---

## 3. ExchangeStorage

**File:** `contracts/exchange/Exchange.sol` (lines 59–198)

### Purpose
Dedicated storage contract for the Exchange. Deployed by the Exchange implementation during initialization. Stores all persistent state variables.

### Responsibilities
- Store configuration (helper proxy, order book, fee receiver, royalties registry, WETH address, chain ID, protocol fee).
- Store order fill mappings.
- Store ERC-20 whitelist.
- Provide getter/setter functions.

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `helperProxy` | `address` | `private` | Address of the ExchangeHelper proxy |
| `orderBook` | `address` | `private` | Public key of the off-chain Order Book service |
| `royaltiesRegistryProxy` | `address` | `private` | Address of the RoyaltiesRegistry proxy |
| `defaultFeeReceiver` | `address` | `private` | Default recipient of protocol fees |
| `weth` | `address` | `private` | Wrapped ETH contract address |
| `exchangeOwner` | `address` | `private` | Address with admin powers over fee settings |
| `feeReceivers` | `mapping(address => address)` | `private` | Token-specific fee receivers (token addr → receiver addr) |
| `fills` | `mapping(bytes32 => uint256)` | `private` | Order fills indexed by order key hash |
| `allowedERC20Assets` | `mapping(address => bool)` | `private` | Whitelist of allowed ERC-20 tokens for trading |
| `protocolFeeBps` | `uint16` | `private` | Protocol fee in basis points (10000 = 100%) |
| `chainId` | `uint256` | `private` | Chain ID for EIP-712 domain |

### Inheritance
- `StorageBase`
- `IExchangeStorage`

### Functions (Getters)
| Function | Returns | Description |
|---|---|---|
| `getHelperProxy()` | `address` | Returns helper proxy address |
| `getOrderBook()` | `address` | Returns order book public key |
| `getDefaultFeeReceiver()` | `address` | Returns default fee receiver |
| `getRoyaltiesRegistryProxy()` | `address` | Returns royalties registry proxy |
| `getFeeReceiver(address)` | `address` | Returns fee receiver for a specific token (falls back to default) |
| `getWETH()` | `address` | Returns WETH address |
| `getFill(bytes32)` | `uint256` | Returns fill amount for an order key hash |
| `isERC20AssetAllowed(address)` | `bool` | Returns whether an ERC-20 is whitelisted |
| `getProtocolFeeBps()` | `uint16` | Returns protocol fee bps |
| `getChainId()` | `uint256` | Returns configured chain ID |

### Functions (Setters)
| Function | Modifier | Description |
|---|---|---|
| `setHelperProxy(address)` | `requireOwner` | Update helper proxy |
| `setOrderBook(address)` | `requireOwner` | Update order book key |
| `setDefaultFeeReceiver(address)` | `requireExchangeOwner` | Update default fee receiver |
| `setRoyaltiesRegistryProxy(address)` | `requireOwner` | Update royalties registry proxy |
| `setFeeReceiver(address, address)` | `requireExchangeOwner` | Set token-specific fee receiver |
| `setWETH(address)` | `requireOwner` | Update WETH address |
| `setFill(bytes32, uint256)` | `requireOwner` | Update order fill value |
| `setERC20AssetAllowed(address, bool)` | `requireExchangeOwner` | Add/remove ERC-20 from whitelist |
| `setProtocolFeeBps(uint16)` | `requireExchangeOwner` | Update protocol fee |
| `setChainId(uint256)` | `requireOwner` | Update chain ID |
| `setExchangeOwner(address)` | `requireExchangeOwner` | Transfer exchange owner role |

### Modifiers
| Modifier | Description |
|---|---|
| `requireOwner` (from StorageBase) | Only the Exchange implementation can call |
| `requireExchangeOwner` | Only the `exchangeOwner` can call |

### Access Control
Two-tier:
- `owner` (StorageBase): The Exchange implementation contract. Controls infrastructure settings.
- `exchangeOwner`: A configured admin address. Controls economic settings (fees, whitelist).

### Critical Logic for Solana
- All mappings need to become PDA-based account lookups.
- The `fills` mapping is critical: each unique order key hash maps to a fill amount. On Solana this becomes a PDA account per order.
- The `allowedERC20Assets` mapping becomes a PDA per whitelisted mint.

---

## 4. Exchange

**File:** `contracts/exchange/Exchange.sol` (lines 200–993)

### Purpose
Main business logic contract for the NFT marketplace. Handles order matching, asset transfers, fee distribution, and ETH/WETH conversions.

### Responsibilities
- Accept and validate matched order pairs.
- Verify EIP-712 signatures (maker, taker, and order-book match allowance).
- Compute fill amounts for partial orders.
- Handle ETH ↔ WETH conversions.
- Distribute payments: protocol fees, royalties, origin fees, payouts.
- Transfer NFTs between maker and taker.
- Support batch matching and order cancellation.

### Inheritance
- `PausableUpgradeable` — pause/unpause functionality
- `ReentrancyGuardUpgradeable` — reentrancy protection
- `OwnableUpgradeable` — ownership
- `UpgradeManager` — upgrade authorization
- `IExchange` — interface
- `UUPSUpgradeable` — UUPS proxy pattern

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `_storage` | `ExchangeStorage` | `public` | Reference to the storage contract |
| `proxy` | `address` | `public` | Address of the Exchange proxy (holds funds) |

### Constants
| Constant | Type | Value | Description |
|---|---|---|---|
| `INTERFACE_ID_ERC2981` | `bytes4` | `keccak256('royaltyInfo(uint256,uint256)')` | ERC-2981 interface ID |
| `TO_MAKER` | `bytes4` | `keccak256('TO_MAKER')` | Transfer direction constant |
| `TO_TAKER` | `bytes4` | `keccak256('TO_TAKER')` | Transfer direction constant |
| `PROTOCOL` | `bytes4` | `keccak256('PROTOCOL')` | Transfer type: protocol fee |
| `ROYALTY` | `bytes4` | `keccak256('ROYALTY')` | Transfer type: royalty |
| `ORIGIN` | `bytes4` | `keccak256('ORIGIN')` | Transfer type: origin fee |
| `PAYOUT` | `bytes4` | `keccak256('PAYOUT')` | Transfer type: payout |
| `UINT256_MAX` | `uint256` | `2^256 - 1` | Used for order cancellation |

### Events
| Event | Parameters | Description |
|---|---|---|
| `Match` | `leftHash, rightHash, leftMaker, rightMaker, newLeftFill, newRightFill` | Emitted when two orders are matched |
| `CancelOrder` | `hash` | Emitted when an order is cancelled |
| `Transfer` | `assetClass (indexed), from (indexed), to (indexed), assetData, assetValue, transferDirection, transferType` | Emitted for every asset transfer |

### Functions

#### Core Matching
| Function | Visibility | Modifiers | Description |
|---|---|---|---|
| `initialize(...)` | `public` | `initializer` | Initializes the contract, deploys ExchangeStorage |
| `matchOrders(...)` | `public payable` | `whenNotPaused` | Matches a single taker/maker order pair |
| `batchMatchOrders(...)` | `external payable` | `whenNotPaused` | Matches multiple order pairs in a single tx |
| `cancelOrder(Order)` | `public` | `whenNotPaused` | Cancels an order by setting fill to MAX |

#### Internal Matching & Transfers
| Function | Visibility | Description |
|---|---|---|
| `matchAndTransfer(...)` | `internal` | Orchestrates asset matching, fill calculation, and transfers |
| `processEthAndWeth(...)` | `internal` | Handles ETH/WETH wrapping/unwrapping logic |
| `doTransfers(...)` | `internal` | Determines fee side and orchestrates all transfers |
| `doTransfersWithFees(...)` | `internal` | Transfers with fee deduction (protocol, royalties, origin, payouts) |
| `transferProtocolFee(...)` | `internal` | Calculates and transfers protocol fee |
| `transferRoyalties(...)` | `internal` | Looks up and transfers royalties |
| `transferFees(...)` | `internal` | Iterates over fee parts and transfers each |
| `transferERC2981Royalties(...)` | `internal` | Transfers royalties from ERC-2981 |
| `transferPayouts(...)` | `internal` | Distributes remaining amount to payout recipients |
| `transfer(...)` | `internal` | Low-level asset transfer (ETH, ERC-20, ERC-721, ERC-1155) |

#### Admin & Utility
| Function | Visibility | Modifiers | Description |
|---|---|---|---|
| `safeTransferERC20(...)` | `external` | `nonReentrant, onlyOwner` | Emergency ERC-20 rescue |
| `setOrderFill(...)` | `external` | — | Sets order fill (only callable by ExchangeHelper) |
| `togglePause()` | `external` | `onlyOwner` | Pause/unpause the exchange |
| `receiveETH()` | `public payable` | — | Accepts ETH from proxy forwarding |
| `receive()` | `external payable` | `onlyWETH` | Accepts ETH only from WETH contract withdrawals |
| `_authorizeUpgrade(...)` | `internal` | `onlyUpgradeManager` | UUPS upgrade authorization |

#### View Functions
| Function | Visibility | Description |
|---|---|---|
| `getProtocolFeeBps()` | `external view` | Returns protocol fee bps |
| `getDefaultFeeReceiver()` | `external view` | Returns default fee receiver |
| `getFeeReceiver(address)` | `external view` | Returns token-specific fee receiver |
| `getOrderFill(bytes32)` | `external view` | Returns fill for an order |
| `getOrdersFills(bytes32[])` | `external view` | Returns fills for multiple orders |
| `isERC20AssetAllowed(address)` | `external view` | Checks ERC-20 whitelist |

### Modifiers
| Modifier | Description |
|---|---|
| `onlyWETH` | Only accepts ETH from the WETH contract |
| `whenNotPaused` (inherited) | Blocks function when paused |
| `nonReentrant` (inherited) | Prevents reentrancy |
| `onlyOwner` (inherited) | Only contract owner |
| `onlyUpgradeManager` (inherited) | Only upgrade manager |

### Security Mechanisms
- **Reentrancy**: `nonReentrant` on `transfer()` and `safeTransferERC20()`
- **Pausable**: All matching and transfer functions gated by `whenNotPaused`
- **Signature verification**: Delegated to ExchangeHelper (which uses LibExchange)
- **Order Book authorization**: Time-limited match allowance signatures
- **ERC-20 whitelist**: Only whitelisted tokens can be traded
- **tx.origin for cancellation**: `cancelOrder` checks `tx.origin == order.maker`

### Critical Logic for Solana
- The entire `matchOrders` flow must be replicated as a Solana instruction.
- ETH/WETH conversion logic needs to be replaced with SOL/wSOL handling.
- The `transfer` function dispatches by asset class — on Solana this becomes SPL token transfers and SOL transfers.
- `fills` mapping is the core state that tracks partial orders.
- The proxy pattern (holding funds) is not needed on Solana since programs can handle funds directly.

---

## 5. ExchangeProxy

**File:** `contracts/exchange/ExchangeProxy.sol`

### Purpose
UUPS (ERC-1967) proxy for the Exchange. Acts as the stable entry point and holds funds (ETH, ERC-20 tokens). All user interactions go through this proxy.

### Responsibilities
- Delegate all calls to the Exchange implementation.
- Hold ETH and tokens on behalf of the protocol during settlement.
- Provide safe transfer functions callable only by the implementation.
- Emit events on behalf of the implementation.

### Inheritance
- `ERC1967Proxy` (OpenZeppelin)
- `ReentrancyGuard` (OpenZeppelin)

### Events
Same as Exchange: `Match`, `CancelOrder`, `Transfer`.

### Functions
| Function | Visibility | Modifiers | Description |
|---|---|---|---|
| `safeTransferERC20(...)` | `external` | `nonReentrant, onlyImplementation` | Transfer ERC-20 tokens held by proxy |
| `safeTransferERC20From(...)` | `external` | `nonReentrant, onlyImplementation` | TransferFrom ERC-20 tokens via proxy |
| `safeTransferERC721From(...)` | `external` | `nonReentrant, onlyImplementation` | Transfer ERC-721 via proxy |
| `safeTransferERC1155From(...)` | `external` | `nonReentrant, onlyImplementation` | Transfer ERC-1155 via proxy |
| `safeTransferETH(...)` | `external` | `nonReentrant, onlyImplementation` | Send ETH from proxy |
| `receiveETH()` | `external payable` | — | Accept ETH deposits |
| `emitMatch(...)` | `external` | `onlyImplementation` | Emit Match event |
| `emitCancelOrder(...)` | `external` | `onlyImplementation` | Emit CancelOrder event |
| `emitTransfer(...)` | `external` | `onlyImplementation` | Emit Transfer event |
| `receive()` | `external payable` | — | Accept plain ETH transfers |

### Modifiers
| Modifier | Description |
|---|---|
| `senderOrigin` | Ensures `tx.origin == msg.sender` (only direct calls, not from contracts) |
| `onlyImplementation` | Only the current implementation contract can call |

### Critical Logic for Solana
- The proxy pattern does not apply on Solana. The Anchor program directly is the entry point.
- The fund-holding role is replaced by either the program's PDA authority or user token accounts.

---

## 6. ExchangeHelper

**File:** `contracts/exchange/helper/ExchangeHelper.sol`

### Purpose
Extension contract that offloads validation, signature verification, fill calculation, and collection bid logic from the main Exchange contract (to avoid contract size limits).

### Responsibilities
- Verify order signatures (EIP-712, EIP-1271).
- Verify Order Book matchAllowance signatures.
- Match asset types between orders.
- Calculate fill amounts.
- Process collection bid orders (validation, formatting, batch matching).
- Validate ERC-20 token whitelist.
- Check counterparty constraints.

### Inheritance
- `OwnableUpgradeable`
- `UpgradeManager`
- `IExchangeHelper`
- `UUPSUpgradeable`

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `exchangeProxy` | `address` | — | Address of the Exchange proxy |
| `orderBook` | `address` | — | Order Book public key |
| `chainId` | `uint256` | — | Chain ID for signature verification |

### Functions

#### Initialization
| Function | Visibility | Modifiers | Description |
|---|---|---|---|
| `initialize(...)` | `public` | `initializer` | Sets exchange proxy, order book, chain ID, owner, upgrade manager |

#### Collection Bids
| Function | Visibility | Description |
|---|---|---|
| `matchCollectionBidOrder(...)` | `external payable` | Matches a collection-wide bid against multiple taker orders |
| `matchCollectionBidOrders(...)` | `external payable` | Batch version — multiple collection bids |
| `batchCancelOrders(...)` | `external` | Cancels multiple orders in one tx |

#### Library Delegation
| Function | Library | Description |
|---|---|---|
| `bps(uint256, uint16)` | `LibBps` | Calculate basis points |
| `calculateFills(...)` | `LibFill` | Calculate and record order fills |
| `hashKey(Order)` | `LibOrder` | Hash order key |
| `verifyOrder(...)` | `LibExchange` | Verify order signature |
| `verifyMatch(...)` | `LibExchange` | Verify order-book match allowance signatures |
| `matchAssets(...)` | `LibExchange` | Match asset types |
| `calculateTotalAmount(...)` | `LibExchange` | Calculate total with origin fees |
| `subFeeInBps(...)` | `LibExchange` | Subtract fee from amount |
| `getRoyaltiesByAssetType(...)` | `LibExchange` | Get royalties for an asset |
| `parse(Order)` | `LibOrderData` | Parse order data into payouts/origin fees |
| `getFeeSide(...)` | `LibFeeSide` | Determine fee payer |
| `calculateTotalTakeAndMakeValues(...)` | — | Calculate total values including origin fees |
| `checkERC20TokensAllowed(...)` | — | Validate ERC-20 whitelist and counterparties |

#### Internal
| Function | Visibility | Description |
|---|---|---|
| `verifyCollectionBid(...)` | `internal view` | Verify collection bid order signature and match allowance |
| `formatCollectionBidOrdersBatch(...)` | `internal` | Format collection bid into matched order pairs |
| `formatCollectionBidSignaturesBatch(...)` | `internal pure` | Format signatures for batch matching |
| `setCollectionBidOrderFill(...)` | `internal` | Update collection bid fill state |
| `_matchCollectionBidOrder(...)` | `internal` | Core collection bid matching logic |

### Security Mechanisms
- `calculateFills` checks `msg.sender == exchangeProxy` (only Exchange can call).
- Collection bid validation ensures taker orders have `collectionBid == false`.
- ERC-20 whitelist validation.
- Counterparty verification.

### Critical Logic for Solana
- All library functions are inlined on Solana (no separate contract deployment needed).
- Collection bid logic is complex and must be carefully preserved.
- The `calculateFills` function has side effects (updates order fills on-chain).

---

## 7. ExchangeHelperProxy

**File:** `contracts/exchange/helper/ExchangeHelperProxy.sol`

### Purpose
Simple UUPS proxy for ExchangeHelper. No additional logic.

### Inheritance
- `ERC1967Proxy`

### Critical Logic for Solana
Not needed on Solana — the helper logic is part of the main program.

---

## 8. RoyaltiesRegistryStorage

**File:** `contracts/royalties-registry/RoyaltiesRegistry.sol` (lines 51–145)

### Purpose
Separate storage contract for the RoyaltiesRegistry. Stores all royalty configurations.

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `ownerRoyaltiesByTokenAndTokenId` | `mapping(bytes32 => RoyaltiesSet)` | `private` | Royalties set by token owner, keyed by `keccak256(token, tokenId)` |
| `creatorRoyaltiesByTokenAndTokenId` | `mapping(bytes32 => RoyaltiesSet)` | `private` | Royalties set by token creator, keyed by `keccak256(token, tokenId)` |
| `royaltiesByToken` | `mapping(address => RoyaltiesSet)` | `private` | Collection-level royalties set by token owner |
| `royaltiesProviders` | `mapping(address => address)` | `private` | External royalty provider addresses per collection |

### Functions
All functions follow the getter/setter pattern with `requireOwner` on setters.

**Owner Royalties (by token+tokenId):**
- `getOwnerRoyaltiesByTokenAndTokenId` / `initializeOwnerRoyaltiesByTokenAndTokenId` / `pushOwnerRoyaltyByTokenAndTokenId` / `deleteOwnerRoyaltiesByTokenAndTokenId`

**Creator Royalties (by token+tokenId):**
- `getCreatorRoyaltiesByTokenAndTokenId` / `initializeCreatorRoyaltiesByTokenAndTokenId` / `pushCreatorRoyaltyByTokenAndTokenId` / `deleteCreatorRoyaltiesByTokenAndTokenId`

**Collection Royalties (by token):**
- `getRoyaltiesByToken` / `initializeRoyaltiesByToken` / `pushRoyaltyByToken` / `deleteRoyaltiesByToken`

**Providers (by token):**
- `getProviderByToken` / `setProviderByToken`

### Critical Logic for Solana
- Each royalty entry becomes a PDA account.
- The three-tier lookup (owner by token+id → owner by token → creator by token+id) must be preserved.

---

## 9. RoyaltiesRegistry

**File:** `contracts/royalties-registry/RoyaltiesRegistry.sol` (lines 147–474)

### Purpose
Manages royalty configurations for NFTs. Supports multiple royalty standards and a cascading lookup hierarchy.

### Responsibilities
- Store and manage owner-set royalties (per token+id, or per collection).
- Store and manage creator-set royalties (per token+id).
- Look up royalties from external providers (Rarible, LooksRare patterns).
- Look up royalties from token contracts (Rarible V1, V2 standards).
- Merge owner and creator royalties when both exist.

### Inheritance
- `OwnableUpgradeable`
- `UpgradeManager`
- `UUPSUpgradeable`
- `IRoyaltiesRegistry`

### State Variables
| Variable | Type | Visibility | Description |
|---|---|---|---|
| `_storage` | `RoyaltiesRegistryStorage` | `public` | Reference to the storage contract |

### Constants
| Constant | Value | Description |
|---|---|---|
| `OWNER` | `bytes4(keccak256('OWNER'))` | Setter type for owner-set royalties |
| `CREATOR` | `bytes4(keccak256('CREATOR'))` | Setter type for creator-set royalties |

### Events
| Event | Parameters | Description |
|---|---|---|
| `RoyaltiesSetForToken` | `token, tokenId, recipients[], bps[], setter` | Emitted when per-token royalties are set |
| `RoyaltiesSetForContract` | `token, recipients[], bps[]` | Emitted when per-collection royalties are set |

### Functions

#### Setters
| Function | Modifier | Description |
|---|---|---|
| `setProviderByToken(address, address)` | `requireOwnerOrTokenOwner` | Set external royalties provider for a collection |
| `setRoyaltiesByToken(address, Part[])` | `requireOwnerOrTokenOwner` | Set collection-level royalties |
| `setOwnerRoyaltiesByTokenAndTokenId(address, uint256, Part[])` | `requireOwnerOrTokenOwner` | Set owner royalties for specific token ID |
| `setCreatorRoyaltiesByTokenAndTokenId(address, uint256, Part[])` | `requireOwnerOrTokenIdCreator` | Set creator royalties for specific token ID |

#### Getters
| Function | Description |
|---|---|
| `getProviderByToken(address)` | Returns the external provider for a collection |
| `getRoyalties(address, uint256)` | Main royalty lookup — cascading hierarchy |

#### Internal
| Function | Description |
|---|---|
| `royaltiesFromContract(address, uint256)` | Queries token contract for Rarible V2, then V1 royalties |
| `providerExtractor(address, uint256)` | Queries external royalty provider with multiple interface patterns |

### Modifiers
| Modifier | Description |
|---|---|
| `requireOwnerOrTokenOwner(address token)` | Registry owner or collection owner |
| `requireOwnerOrTokenIdCreator(address token, uint256 tokenId)` | Registry owner or token creator |

### Royalty Lookup Hierarchy
1. **Owner royalties by token+tokenId** → if not initialized, try **Owner royalties by token (collection-level)**
2. **Creator royalties by token+tokenId**
3. If both owner and creator found → **merge them**
4. If only owner found → return owner royalties
5. If only creator found → return creator royalties
6. If neither → try **external provider** (`providerExtractor`)
7. If no provider → try **token contract** (`royaltiesFromContract`: Rarible V2 → V1)
8. If nothing found → return empty array

### Security Mechanisms
- Royalty sum validation (≤ 10,000 bps / 100%).
- No zero-address royalty recipients.
- `requireOwnerOrTokenOwner` uses `tx.origin` to check against `IOwnable(token).owner()`.
- `requireOwnerOrTokenIdCreator` uses `tx.origin` to check against `ICreator(token).creator(tokenId)`.

### Critical Logic for Solana
- The cascading royalty lookup must be preserved exactly.
- External provider calls become CPI calls on Solana.
- The Rarible V1/V2 interface checks become optional CPI patterns.
- The `tx.origin` checks need an alternative (Solana has no `tx.origin`; use signer verification).

---

## 10. RoyaltiesRegistryProxy

**File:** `contracts/royalties-registry/RoyaltiesRegistryProxy.sol`

### Purpose
Simple UUPS proxy for RoyaltiesRegistry. No additional logic.

### Critical Logic for Solana
Not needed on Solana.

---

## 11. Libraries

### LibAssetClasses
**File:** `contracts/libraries/LibAssetClasses.sol`

Defines `bytes4` constants for asset class identifiers:
- `ETH_ASSET_CLASS` = `keccak256('ETH')`
- `WETH_ASSET_CLASS` = `keccak256('WETH')`
- `PROXY_WETH_ASSET_CLASS` = `keccak256('PROXY_WETH')`
- `ERC20_ASSET_CLASS` = `keccak256('ERC20')`
- `ERC721_ASSET_CLASS` = `keccak256('ERC721')`
- `ERC1155_ASSET_CLASS` = `keccak256('ERC1155')`

On Solana: replaced with Rust enum variants.

### LibAssetTypes
**File:** `contracts/libraries/LibAssetTypes.sol`

Defines structs:
- `AssetType { bytes4 assetClass, bytes data }` — asset classification
- `Asset { AssetType assetType, uint256 value }` — asset with amount

On Solana: Rust structs with Borsh serialization.

### LibAsset
**File:** `contracts/libraries/LibAsset.sol`

Provides EIP-712 hashing for `AssetType` and `Asset` structs. Uses typehashes for structured data hashing.

On Solana: replaced with Borsh-based hashing or SHA-256.

### LibOrderTypes
**File:** `contracts/libraries/LibOrderTypes.sol`

Defines:
- `Order { maker, makeAsset, taker, takeAsset, salt, start, end, dataType, data, collectionBid }` — the core order struct
- `BatchBidOrders { orders[], signatures[], matchBeforeTimestamps[], orderBookSignatures[] }` — batch bid container

On Solana: Rust structs, possibly with `#[derive(AnchorSerialize, AnchorDeserialize)]`.

### LibOrder
**File:** `contracts/libraries/LibOrder.sol`

Key functions:
- `hashKey(Order)` — unique order identifier (maker + asset types + salt + collectionBid)
- `hash(Order)` — full EIP-712 order hash for signature verification
- `hash(bytes32, uint256)` — match allowance hash
- `calculateRemaining(Order, fill)` — remaining make/take values after partial fills
- `validate(Order)` — time validation and asset class compatibility checks
- `validateCollectionBidMakerOrder(Order)` — collection bid validation
- `validateCollectionBidTakerOrdersBatch(Order[])` — batch taker validation
- `formatCollectionBidOrdersBatch(...)` — format collection bid into matched pairs
- `formatCollectionBidMakerOrder(...)` — create synthetic maker order from collection bid

### LibFill
**File:** `contracts/libraries/LibFill.sol`

Key functions:
- `fillOrder(left, right, leftFill, rightFill)` — computes new fill amounts for a matched pair
- `fillLeft(...)` / `fillRight(...)` — handles the two fill cases (which order gets fully filled)
- `fillCollectionBidOrder(orders[], fills[])` — fill computation for collection bids

### LibExchange
**File:** `contracts/libraries/LibExchange.sol`

Key functions:
- `matchAssets(AssetType, AssetType)` — verify two asset types are compatible (ETH/WETH matching, same-class matching)
- `matchAssets(Order, Order)` — match both sides of two orders
- `verifyOrder(Order, signature, caller, proxy, chainId)` — full order signature verification (salt=0 maker check, EIP-712 recovery, EIP-1271)
- `verifyMatch(...)` — verify both orders' match allowance signatures
- `verifyMatchAllowance(...)` — verify a single match allowance signature with time check
- `recoverMatchAllowanceSigner(...)` — ECDSA recovery for match allowance
- `subFee(value, fee)` — subtract fee from value (capped at value)
- `subFeeInBps(rest, total, feeInBps)` — subtract fee as bps of total
- `calculateTotalAmount(amount, originFees)` — sum amount + origin fees
- `getRoyaltiesByAssetType(assetType, registry)` — lookup royalties for an NFT
- `checkCounterparties(left, right)` — validate taker/maker address constraints

### LibSignature
**File:** `contracts/libraries/LibSignature.sol`

ECDSA signature recovery:
- `recover(hash, signature)` — split signature into r, s, v and recover signer
- `recover(hash, v, r, s)` — recover signer with malleability protection and EIP-191 support (v > 30)
- `toEthSignedMessageHash(hash)` — wrap hash with `\x19Ethereum Signed Message:\n32` prefix

### LibEIP712
**File:** `contracts/libraries/LibEIP712.sol`

- `hashEIP712Message(hashStruct, proxy, chainId)` — constructs EIP-712 domain separator with name "Energi", version "1", and computes final message hash

### LibMath
**File:** `contracts/libraries/LibMath.sol`

- `safeGetPartialAmountFloor(num, den, target)` — `(num * target) / den` with 0.1% rounding error check
- `safeGetPartialAmountCeil(num, den, target)` — ceiling version
- `isRoundingErrorFloor` / `isRoundingErrorCeil` — rounding error detection

### LibBps
**File:** `contracts/libraries/LibBps.sol`

- `bps(value, bpsValue)` — `value * bpsValue / 10000`

### LibFeeSide
**File:** `contracts/libraries/LibFeeSide.sol`

- `getFeeSide(makerClass, takerClass)` — determines which side pays fees. Priority: ETH → WETH → ERC-20 → ERC-1155 → NONE.

### LibOrderData / LibOrderDataV1
**File:** `contracts/libraries/LibOrderData.sol`, `LibOrderDataV1.sol`

- `parse(Order)` — decodes order's `data` field into `DataV1 { payouts[], originFees[] }`. Handles `V1` data type and `0xffffffff` (empty).
- `decodeOrderDataV1(bytes)` — ABI-decodes V1 data

### LibRoyaltiesV1 / LibRoyaltiesV2
**File:** `contracts/libraries/LibRoyaltiesV1.sol`, `LibRoyaltiesV2.sol`

Interface ID constants and struct definitions for Rarible V1/V2 royalty standards.

### SafeMath
**File:** `contracts/libraries/SafeMath.sol`

Standard SafeMath library: `add`, `sub`, `mul`, `div`, `mod` with overflow protection.

On Solana: Rust has built-in overflow checking in debug mode; use `checked_*` operations.

---

## 12. Interfaces

### IExchange
External interface for the Exchange contract: `matchOrders`, `batchMatchOrders`, `cancelOrder`, getters, setters.

### IExchangeStorage
External interface for ExchangeStorage: all getters and setters.

### IExchangeHelper
External interface for ExchangeHelper: all library-delegated functions plus collection bid matching.

### IExchangeOrders
Subset interface for order-related functions on Exchange: `batchMatchOrders`, `cancelOrder`, `setOrderFill`, `getOrderFill`.

### IRoyaltiesRegistry
External interface for RoyaltiesRegistry: set/get royalties by token, by token+id, providers.

### IRoyaltiesRegistryStorage
External interface for RoyaltiesRegistryStorage: all CRUD operations.

### IRoyaltiesProviders
Interface for external royalty providers: Rarible-style `getRoyalties` and LooksRare-style `royaltyFeeInfoCollection`.

### IWrappedCoin
Interface for WETH: `deposit()` and `withdraw(uint256)`.

### IRoyaltiesV1
Rarible V1 standard: `getFeeRecipients(uint256)` and `getFeeBps(uint256)`.

### IRoyaltiesV2
Rarible V2 standard: `getRaribleV2Royalties(uint256)`.

### IOwnable
Simple ownership interface: `owner()`.

### ICreator
Token creator interface: `creator(uint256 tokenId)`.
