# EVM Launchpad — Complete Protocol Specification

> Extracted from: `evm-contracts/launchpad-contracts/contracts/`
> Date: 2026-03-18
> Author: Protocol Migration Analysis

---

## 1. Contract Architecture Overview

The EVM Launchpad consists of **6 NFT collection contracts** (3 base variants × 2 registry modes) plus an **OperatorRegistry** infrastructure:

### Contract Hierarchy

| Contract | Base | Refund Type | OperatorFilter | Revenue Split |
|---|---|---|---|---|
| `GMIERC721` | ERC721A + ERC2981 | None (Standard) | No | 100% to owner on mint |
| `GMIERC721R` | ERC721Enumerable | 100% Refund | No | Held in contract |
| `GMIERC721R80` | ERC721Enumerable | 80% Refund | No | 20% owner + 80% held |
| `GMIERC721C` | ERC721A + ERC2981 | None (Standard) | Yes | 100% to owner on mint |
| `GMIERC721RC` | ERC721Enumerable | 100% Refund | Yes | Held, share to registry |
| `GMIERC721R80C` | ERC721Enumerable | 80% Refund | Yes | 20% owner + 80% held, share to registry |

### Dependency Contracts

| Contract | Purpose |
|---|---|
| `OperatorRegistry` | Upgradeable proxy, manages marketplace whitelist + revenue sharing |
| `OperatorRegistryProxy` | TransparentUpgradeableProxy |
| `OperatorRegistryProxyAdmin` | ProxyAdmin |
| `OperatorFilter` | Base contract for "C" variants to validate transfers/approvals |
| `FixedPoint` | Assembly-based percentage math (used by R/R80 for reserved calculation) |
| `ERC721A` | Gas-optimized ERC721 with batch minting |

---

## 2. Function Catalog

### 2.1 Core Minting Functions

#### F1: `mint(uint256 _quantity)` — Public Sale Mint
- **Visibility**: external, payable
- **Modifiers**: `whenNotPaused`, `noContracts`, `nonReentrant`
- **Inputs**: `_quantity` — number of NFTs to mint
- **Logic** (Standard / GMIERC721 / GMIERC721C):
  - Requires `publicsaleActive == true`
  - Delegates to `_mintTokens(msg.sender, _quantity, mintPrice, maxTxMintAmount, maxUserMintAmount, numberMinted[msg.sender])`
  - Increments `numberMinted[msg.sender] += _quantity`
  - Emits `Minted(msg.sender, _quantity)`
- **Logic** (Refundable 100% / GMIERC721R / GMIERC721RC):
  - Same as Standard
  - `_mintTokens` internally sets `refundPrice[tokenId] = _price` per token
  - Token IDs self-assigned from refunded pool if `totalMints == maxMintSupply && refundedTokenIds.length > 0`
- **Logic** (Refundable 80% / GMIERC721R80 / GMIERC721R80C):
  - Requires `publicsaleActive == true`
  - Requires `mintedAmount + _quantity <= maxMintSupply`
  - Requires `numberMinted[msg.sender] + _quantity <= maxUserMintAmount`
  - **Three-way branching**:
    1. `onlyRemint` (totalMints == maxMintSupply && refundedTokenIds > 0): Charges 80% price, calls `_reMintTokens`
    2. Mixed (totalMints + qty > maxMintSupply && refundedTokenIds > 0 && !onlyRemint): Splits into remint portion (80% price) + fresh mint portion (100% price)
    3. Fresh only (totalMints + qty <= maxMintSupply && !onlyRemint): Charges 100% price, calls `_mintTokens`
  - Increments `numberMinted[msg.sender] += _quantity`
  - Emits `Minted(msg.sender, _quantity)`

#### F2: `presaleMint(uint256 _quantity)` — Presale / Whitelist Mint
- **Visibility**: external, payable
- **Modifiers**: `whenNotPaused`, `noContracts`, `nonReentrant`
- **Inputs**: `_quantity` — number of NFTs to mint
- **Logic** (Standard / GMIERC721 / GMIERC721C):
  - Requires `presaleActive == true`
  - Requires `whitelists[msg.sender] > 0`
  - Delegates to `_mintTokens(msg.sender, _quantity, presaleMintPrice, presaleMaxTxMintAmount, whitelists[msg.sender], presaleNumberMinted[msg.sender])`
  - NOTE: `whitelists[msg.sender]` acts as max user mint limit for presale (not just a boolean!)
  - Increments `presaleNumberMinted[msg.sender] += _quantity`
  - Emits `PresaleMinted(msg.sender, _quantity)`
- **Logic** (Refundable 100% / GMIERC721R / GMIERC721RC):
  - Same whitelist check
  - **ADDITIONAL**: If `presaleMintPrice == 0`, requires `reservedMints + _quantity <= reservedNFTs`, increments `reservedMints`
  - For GMIERC721R (non-C): same 6-param `_mintTokens` as standard (with refund-aware token ID assignment)
  - For GMIERC721RC (C variant): same but `_mintTokens` also sends share to `operatorRegistry.fundReceiver()`
- **Logic** (Refundable 80% / GMIERC721R80 / GMIERC721R80C):
  - Same whitelist check + `presaleMintPrice == 0 → reservedMints` logic (R80 only, not R80C)
  - Same three-way branching as `mint()` but using `presaleMintPrice` and `presaleMaxTxMintAmount`

#### F3: `ownerMint(address _to, uint256 _quantity)` — Owner/Admin Mint
- **Visibility**: external
- **Modifiers**: `onlyOwner`, `nonReentrant`
- **Inputs**: `_to` — recipient, `_quantity` — count
- **Logic** (Standard / GMIERC721 / GMIERC721C):
  - Requires `mintedAmount + _quantity <= maxMintSupply`
  - Increments `mintedAmount += _quantity`
  - Calls `_safeMint(_to, _quantity)` (batch via ERC721A)
  - Marks `isOwnerMint[tokenId] = true` for each token
  - Emits `OwnerMinted(_to, _quantity)`
- **Logic** (Refundable 100% / GMIERC721R / GMIERC721RC):
  - Same supply check
  - **ADDITIONAL**: Requires `reservedMints + _quantity <= reservedNFTs` (R variant only)
  - Assigns from refundedTokenIds pool if `totalMints == maxMintSupply && refundedTokenIds.length > 0`
  - Otherwise uses `totalMints + 1` as next token ID
  - Marks `isOwnerMint` = true
  - Increments `reservedMints += _quantity` (R variant only)
  - Emits `OwnerMinted(_to, _quantity)`
- **Logic** (Refundable 80% / GMIERC721R80 / GMIERC721R80C):
  - Same as Refundable 100% but no reserved mints tracking (R80C)
  - R80 base: same reservedMints check

#### F4: `refund(uint256[] calldata _tokenIds)` — NFT Refund (R/R80/RC/R80C only)
- **Visibility**: external
- **Modifiers**: `nonReentrant`
- **Inputs**: `_tokenIds` — array of token IDs to refund
- **Logic**:
  - For each tokenId:
    - Requires `msg.sender == ownerOf(tokenId)` — caller must own it
    - Requires `isOwnerMint[tokenId] == false` — cannot refund owner-minted tokens
    - Requires `refundPrice[tokenId] > 0` — cannot refund free NFTs
    - Burns the token (`_burn(tokenId)`)
    - Accumulates `refundAmount += refundPrice[tokenId]`
    - Pushes `tokenId` to `refundedTokenIds` array
    - Emits `Refund(msg.sender, tokenId, tokenAmount)`
  - Decrements `mintedAmount -= _tokenIds.length`
  - Increments `refundCounter += _tokenIds.length`
  - Transfers accumulated refund amount to caller

### 2.2 Configuration Functions

#### F5: `publicsaleConfig(...)` — Configure Public Sale
- **Visibility**: external
- **Modifier**: `onlyOwner`
- **Params** (Standard/C variants): `_mintPrice, _maxUserMintAmount, _maxTxMintAmount, _publicsaleStatus`
- **Params** (R/R80 variants): `_maxUserMintAmount, _maxTxMintAmount, _publicsaleStatus` (no price change!)
- **Params** (RC/R80C variants): `_mintPrice, _maxUserMintAmount, _maxTxMintAmount, _publicsaleStatus`
- Sets internal state + emits change events

#### F6: `presaleConfig(...)` — Configure Presale
- **Visibility**: external
- **Modifier**: `onlyOwner`
- **Same parameter patterns as F5 but for presale variables**

#### F7: `togglePresale()` — Toggle Presale Status
- **Visibility**: external
- **Modifier**: `onlyOwner`
- Toggles `presaleActive = !presaleActive`
- **BUG NOTE**: Emits `PresaleToggled(!presaleActive)` — emits the OPPOSITE of new state (emits old state due to toggle before emit)

#### F8: `togglePublicsale()` — Toggle Public Sale Status
- **Visibility**: external
- **Modifier**: `onlyOwner`
- Same pattern as F7 (same bug with event emission)

#### F9: `togglePause()` — Pause/Unpause Contract
- **Visibility**: external
- **Modifier**: `onlyOwner`
- Calls `_pause()` or `_unpause()` based on current state

#### F10: `setBaseURI(string memory _uri)` — Set Base URI
- **Visibility**: external
- **Modifier**: `onlyOwner`
- Updates `baseURI` storage
- Emits `SetBaseUri(_uri)`

### 2.3 Whitelist Management

#### F11: `addWhitelist(address[] memory _users, uint256[] memory _limit)` — Add Whitelist
- **Visibility**: external
- **Modifier**: `onlyOwner`
- Requires `_users.length == _limit.length`
- For each: requires `_users[i] != address(0)`, sets `whitelists[_users[i]] = _limit[i]`
- Emits `WhitelistAdded(_users[i])` per user
- **NOTE**: `_limit[i]` is the per-user mint cap, NOT a boolean

#### F12: `removeWhitelist(address[] memory _users)` — Remove Whitelist
- **Visibility**: external
- **Modifier**: `onlyOwner`
- For each: requires non-zero address, sets `whitelists[_users[i]] = 0` (only if currently > 0)
- Emits `WhitelistRemoved(_users[i])` per user

### 2.4 View Functions

#### F13: `canBeRefunded(uint256 _tokenId)` — Check Refund Eligibility (R/R80 variants only)
- Requires token exists
- Returns `!isOwnerMint[_tokenId]`

#### F14: `supportsInterface(bytes4 interfaceId)` — ERC165 (Standard/C variants only)
- Returns support for IERC721, IERC721Metadata, IERC2981

### 2.5 Internal Functions

#### F15: `_mintTokens(...)` — Core Mint Logic
- **Standard variant** (GMIERC721/C):
  - Validates `msg.value == _quantity * _price`
  - Validates `_quantity <= _maxTxMintAmount`
  - Validates `_userMints + _quantity <= _maxUserMintAmount`
  - Validates `mintedAmount + _quantity <= maxMintSupply`
  - Increments `mintedAmount`
  - Calls `_safeMint`
  - Transfers `msg.value` to `owner()` (if > 0)
- **Refundable 100% variant** (GMIERC721R):
  - Same validations
  - Individual mints with token ID from `totalMints + 1` or refundedTokenIds pool
  - Stores `refundPrice[tokenId] = _price`
  - Does NOT transfer funds (held in contract for refund)
- **Refundable 100% C variant** (GMIERC721RC):
  - Same as R but transfers share to `operatorRegistry.fundReceiver()`: `(_price * operatorRegistry.sharePercentageBps()) / 10000`
- **Refundable 80% variant** (GMIERC721R80):
  - `mintCompliance` modifier checks `_quantity <= _maxTxMintAmount`
  - Individual mints, stores `refundPrice[tokenId] = (_price * 80) / 100`
  - Transfers 20% to `owner()`
- **Refundable 80% C variant** (GMIERC721R80C):
  - Same 80/20 split
  - Additionally transfers `(cut80 * operatorRegistry.sharePercentageBps()) / 10000` to `operatorRegistry.fundReceiver()`

#### F16: `_reMintTokens(...)` — Remint from Refunded Pool (R80/R80C only)
- `mintCompliance` modifier
- Requires `_quantity <= refundedTokenIds.length`
- Mints from `refundedTokenIds[0]`, removes from array
- Sets `refundPrice[tokenId] = (_price * 80) / 100`

#### F17: `_transferNRG(address _to, uint256 _value)` — Native Token Transfer
- Uses low-level `call` (not `transfer`)
- Requires success

#### F18: `_removeTokenId(uint256 _index)` — Array Element Removal
- **R variant**: Shifts elements left (O(n))
- **R80/R80C variant**: Swaps with last element (O(1))

---

## 3. Require Conditions Catalog

| ID | Condition | Contracts | Location |
|---|---|---|---|
| R1 | `publicsaleActive == true` | All | `mint()` |
| R2 | `presaleActive == true` | All | `presaleMint()` |
| R3 | `whitelists[msg.sender] > 0` | All | `presaleMint()` |
| R4 | `msg.value == _quantity * _price` | All standard | `_mintTokens()` |
| R5 | `_quantity <= _maxTxMintAmount` | All | `_mintTokens()` / `mintCompliance` |
| R6 | `_userMints + _quantity <= _maxUserMintAmount` | All | `_mintTokens()` / `mint()` |
| R7 | `mintedAmount + _quantity <= maxMintSupply` | All | `_mintTokens()` / `ownerMint()` / `mint()` |
| R8 | `!Address.isContract(msg.sender)` | All (mint/presaleMint) | `noContracts` modifier |
| R9 | `msg.sender == ownerOf(tokenId)` | R/R80/RC/R80C | `refund()` |
| R10 | `isOwnerMint[tokenId] == false` | R/R80/RC/R80C | `refund()` |
| R11 | `refundPrice[tokenId] > 0` | R/R80/RC/R80C | `refund()` |
| R12 | `_users.length == _limit.length` | All | `addWhitelist()` |
| R13 | `_users[i] != address(0)` | All | `addWhitelist()`/`removeWhitelist()` |
| R14 | `mintPrice_ > 1 ether` | R/R80 (non-C) | `constructor` |
| R15 | `reservedMints + _quantity <= reservedNFTs` | R (non-C), R80 (free presale) | `presaleMint()`/`ownerMint()` |
| R16 | `Transfer success (bool os == true)` | All | `_transferNRG()` |
| R17 | `_quantity <= refundedTokenIds.length` | R80/R80C | `_reMintTokens()` |
| R18 | `msg.value == ((price * 80) / 100) * qty` | R80/R80C | `mint()` remint branch |

---

## 4. Events Catalog

| ID | Event | Parameters | Contracts |
|---|---|---|---|
| E1 | `Minted` | `address indexed user, uint256 quantity` | All |
| E2 | `PresaleMinted` | `address indexed user, uint256 quantity` | All |
| E3 | `OwnerMinted` | `address indexed user, uint256 quantity` | All |
| E4 | `Refund` | `address indexed user, uint256 tokenId, uint256 tokenAmount` | R/R80/RC/R80C |
| E5 | `MaxUserMintAmountChanged` | `uint256 newMaxUserMintAmount` | All |
| E6 | `MaxTxMintAmountChanged` | `uint256 newMaxTxMintAmount` | All |
| E7 | `MintPriceChanged` | `uint256 newMintPrice` | Standard/C/RC/R80C |
| E8 | `PresaleMaxUserMintAmountChanged` | `uint256 newPresaleMaxUserMintAmount` | All |
| E9 | `PresaleMaxTxMintAmountChanged` | `uint256 newPresaleMaxTxMintAmount` | All |
| E10 | `PresaleMintPriceChanged` | `uint256 newPresaleMintPrice` | Standard/C/RC/R80C |
| E11 | `WhitelistAdded` | `address indexed users` | All |
| E12 | `WhitelistRemoved` | `address indexed users` | All |
| E13 | `PresaleToggled` | `bool presaleStatus` | All |
| E14 | `PublicsaleToggled` | `bool publicsaleStatus` | All |
| E15 | `SetBaseUri` | `string uri` | All |
| E16 | `Withdrawal` | `address indexed to, uint256 amount` | Standard/C only |
| E17 | `CollectionLaunched` | `address collectionAddress, string name, string symbol` | C variants only |

### OperatorRegistry Events

| ID | Event | Parameters |
|---|---|---|
| E18 | `WhitelistAdded` | `address indexed collection, address indexed operator` |
| E19 | `WhitelistRemoved` | `address indexed collection, address indexed operator` |
| E20 | `UniversalWhitelistAdded` | `address indexed operator` |
| E21 | `UniversalWhitelistRemoved` | `address indexed operator` |
| E22 | `FundReceiverChanged` | `address indexed oldReceiver, address indexed newReceiver` |
| E23 | `SharePercentageBpsChanged` | `uint256 oldSharePercentage, uint256 newSharePercentage` |

---

## 5. Storage Layout

### 5.1 Common State (All Contracts)

```
uint256 maxMintSupply          // Immutable after construction
uint256 mintPrice              // Configurable by owner (Standard/C/RC/R80C)
uint256 maxUserMintAmount      // Configurable by owner
uint256 maxTxMintAmount        // Configurable by owner
uint256 mintedAmount           // Tracks live minted count (decremented on refund)

bool presaleActive             // Toggle by owner
bool publicsaleActive          // Toggle by owner
uint256 presaleMintPrice       // Configurable
uint256 presaleMaxUserMintAmount
uint256 presaleMaxTxMintAmount

mapping(address => uint256) numberMinted          // Per-user public mint count
mapping(address => uint256) presaleNumberMinted    // Per-user presale mint count
mapping(uint256 => bool) isOwnerMint              // Flags owner-minted tokens
mapping(address => uint256) whitelists            // Per-user whitelist mint limit

string baseURI                 // Metadata base URI
string VERSION = "1.0.0"       // Contract version
```

### 5.2 Refundable State (R/R80/RC/R80C)

```
uint256 refundCounter          // Total refunded count
uint256 totalMints             // Total ever minted (monotonically increasing)
uint256[] refundedTokenIds     // Pool of refunded token IDs for reminting

mapping(uint256 => uint256) refundPrice   // Per-token refund value
```

### 5.3 Reserved Mint State (R/R80 non-C only)

```
uint256 reservedNFTs           // 20% of maxMintSupply (set in constructor)
uint256 reservedMints          // Count of reserved mints used
```

### 5.4 OperatorRegistry State

```
mapping(address => mapping(address => bool)) isWhitelist      // collection => operator => allowed
mapping(address => bool) universalAllowedOperators            // operator => allowed for all
address fundReceiver                                          // Revenue share recipient
uint256 sharePercentageBps                                    // Revenue share in basis points
mapping(bytes32 => bool) whitelistedCodehashes                // Codehash-based whitelist
```

---

## 6. Modifiers Catalog

| Modifier | Logic | Used By |
|---|---|---|
| `noContracts` | `require(!Address.isContract(msg.sender))` | `mint()`, `presaleMint()` |
| `onlyOwner` | OpenZeppelin Ownable | All admin functions |
| `whenNotPaused` | OpenZeppelin Pausable | `mint()`, `presaleMint()` |
| `nonReentrant` | OpenZeppelin ReentrancyGuard / ERC721A | All state-mutating functions |
| `mintCompliance` | `require(_quantity <= _maxTxMintAmount)` | R80/R80C `_mintTokens`, `_reMintTokens` |
| `validateTransfer` | Checks caller is EOA or whitelisted, receiver is EOA or whitelisted | C variants on transfer |
| `validateApprove` | Checks operator is EOA or whitelisted | C variants on approve |

---

## 7. Implicit Behaviors

| ID | Behavior | Details |
|---|---|---|
| IB1 | **Token ID starts from 1** | Standard/C variants override `_startTokenId() → 1`. R/R80 variants use `totalMints + 1` |
| IB2 | **whitelist value is a mint limit, not boolean** | `whitelists[user]` stores the max number of presale mints, not just true/false |
| IB3 | **Refunded token IDs are recycled** | When all fresh IDs used, refunded IDs are re-minted to new users |
| IB4 | **Owner mints bypass price** | `ownerMint()` charges zero, no `msg.value` check |
| IB5 | **Owner mints are non-refundable** | `isOwnerMint[id] = true` blocks refund |
| IB6 | **Free presale mints consume reserved pool** | In R variants, if `presaleMintPrice == 0`, mints come from reserved pool |
| IB7 | **mintedAmount is net (decremented on refund)** | `mintedAmount` goes down when refund happens, enabling re-minting up to `maxMintSupply` |
| IB8 | **totalMints is gross (never decremented)** | `totalMints` only goes up, representing total ever minted |
| IB9 | **R80 refund pays 80% OF ORIGINAL mint price, not 80% of current price** | Uses stored `refundPrice[tokenId]` which was set at mint time |
| IB10 | **R80C sends 20% to owner + share to fundReceiver per mint** | Two separate transfers per fresh mint |
| IB11 | **RC sends full price share to fundReceiver per mint** | `(_price * sharePercentageBps) / 10000` per token |
| IB12 | **R80 remint price is 80% of full price** | User pays 80%, gets 80% refund value; owner gets 0% on remint |
| IB13 | **Constructor min price check** | R/R80 (non-C): `mintPrice_ > 1 ether`. C variants: removed (commented out) |
| IB14 | **Reserved NFTs = 20% of maxMintSupply** | Calculated via FixedPoint assembly |
| IB15 | **Event bug in togglePresale/togglePublicsale** | Emits negated value AFTER toggle, effectively emitting old state |
| IB16 | **R variant removeTokenId is O(n), R80 variant is O(1)** | Different array manipulation strategies |
| IB17 | **Standard variants transfer funds immediately to owner** | No fund custody in contract |
| IB18 | **Refundable variants hold funds in contract** | Contract balance = sum of refundable amounts |
| IB19 | **C variants can receive ETH** | RC/R80C have `receive() external payable {}` |

---

## 8. Access Control Model

### Roles
1. **Owner** (Ownable): Full admin access — configure sales, manage whitelist, pause, owner mint, set URI
2. **Whitelisted User**: Can presale mint (up to their whitelist limit)
3. **Public User (EOA only)**: Can public mint when sale is active
4. **OperatorRegistry Owner**: Manage marketplace whitelist, fund receiver, share percentage

### Key Constraints
- No role-based access beyond owner
- No multi-sig requirement (single owner)
- No timelock on admin actions
- Owner can be transferred (OpenZeppelin)
- `noContracts` modifier prevents smart contract callers (except owner mint which has no such check)

---

## 9. Upgradeability

- **NFT Contracts**: NOT upgradeable (deployed as-is)
- **OperatorRegistry**: Upgradeable via TransparentUpgradeableProxy
  - `initialize()` pattern
  - ProxyAdmin controls upgrades
  - State persists across upgrades

---

## 10. Security Model Summary

### Existing Protections
1. **Reentrancy Guard**: On all mint/refund functions
2. **Contract caller prevention**: `noContracts` modifier checks `Address.isContract`
3. **Pausability**: Owner can pause/unpause
4. **Ownership validation**: Refund requires token ownership
5. **Supply cap enforcement**: All mints check against `maxMintSupply`
6. **Per-user limits**: Both public and presale have per-user caps
7. **Per-tx limits**: Maximum mint quantity per transaction
8. **Value validation**: Exact payment required (no excess allowed)
9. **Owner-mint non-refundable**: Prevents owner from minting and immediately refunding

### Known Limitations / Risks
1. **Single owner**: No multi-sig, key compromise = total loss
2. **`isContract` check bypassable**: Can be called from constructor (code.length == 0 during construction)
3. **Centralized whitelist**: Owner has full control over who can presale mint
4. **No withdrawal function for R variants**: Funds held in contract indefinitely (until all refund or contract drains)
5. **togglePresale/togglePublicsale event bug**: Events emit wrong state
6. **R variant O(n) array removal**: Gas-intensive for large refund pools
7. **No refund deadline**: Users can refund anytime
8. **R80C double transfer**: Gas overhead from two separate native token transfers per mint

---

## 11. OperatorRegistry Deep Dive

### Functions

| Function | Access | Purpose |
|---|---|---|
| `initialize(fundReceiver_, sharePercentageBps_)` | Once (initializer) | Setup |
| `addWhitelist(collection, operator)` | onlyOwner, whenNotPaused | Whitelist marketplace for collection |
| `removeWhitelist(collection, operator)` | onlyOwner, whenNotPaused | Remove whitelist |
| `addUniversalOperator(operator)` | onlyOwner, whenNotPaused | Allow operator for ALL collections |
| `removeUniversalOperator(operator)` | onlyOwner, whenNotPaused | Remove universal operator |
| `changeFundReceiver(newAddr)` | onlyOwner, whenNotPaused | Update revenue share recipient |
| `changeSharePercentageBps(bps)` | onlyOwner, whenNotPaused | Update share percentage |
| `addWhitelistedCodehash(hash, bool)` | onlyOwner, whenNotPaused | Whitelist by codehash |
| `isOperatorAllowed(collection, operator)` | view | Check: whitelistedCodehash OR per-collection whitelist OR universal operator |
| `pause()` | onlyOwner | Pause registry |
| `unpause()` | onlyOwner | Unpause registry |

### Revenue Sharing (C variants only)

- **RC**: On each mint, sends `(_price * sharePercentageBps) / 10000` to `fundReceiver`
- **R80C**: On each fresh mint, sends owner 20% of price, and sends `(cut80 * sharePercentageBps) / 10000` to `fundReceiver`
- The `sharePercentageBps` is a percentage of the HELD amount (not total price)

---

## 12. Cross-Contract Relationships

```
GMIERC721C ─────────── uses ──────> OperatorFilter ──── uses ──> IOperatorRegistry
GMIERC721RC ────────── uses ──────> OperatorFilter ──── uses ──> IOperatorRegistry
GMIERC721R80C ─────── uses ──────> OperatorFilter ──── uses ──> IOperatorRegistry

GMIERC721R ──── inherits ──> FixedPoint (for reservedNFTs calculation)
GMIERC721R80 ── inherits ──> FixedPoint (for reservedNFTs calculation)

OperatorRegistryProxy ──── delegates to ──> OperatorRegistry (implementation)
OperatorRegistryProxyAdmin ── manages ──> OperatorRegistryProxy
```

---

## 13. Complete Function → Variant Matrix

| Function | GMIERC721 | GMIERC721R | GMIERC721R80 | GMIERC721C | GMIERC721RC | GMIERC721R80C |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| `mint()` | ✅ | ✅ | ✅ (3-way) | ✅ | ✅ | ✅ (3-way) |
| `presaleMint()` | ✅ | ✅ (+reserved) | ✅ (3-way+reserved) | ✅ | ✅ | ✅ (3-way) |
| `ownerMint()` | ✅ | ✅ (+reserved) | ✅ (+reserved) | ✅ | ✅ | ✅ |
| `refund()` | ❌ | ✅ (100%) | ✅ (80%) | ❌ | ✅ (100%) | ✅ (80%) |
| `canBeRefunded()` | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| `publicsaleConfig()` | ✅ (+price) | ✅ (no price) | ✅ (no price) | ✅ (+price) | ✅ (+price) | ✅ (+price) |
| `presaleConfig()` | ✅ (+price) | ✅ (no price) | ✅ (no price) | ✅ (+price) | ✅ (+price) | ✅ (+price) |
| `togglePresale()` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `togglePublicsale()` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `togglePause()` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `setBaseURI()` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `addWhitelist()` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `removeWhitelist()` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `supportsInterface()` | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| Transfer filter | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ |
| Approve filter | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ |
| Revenue share | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| `receive()` | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
