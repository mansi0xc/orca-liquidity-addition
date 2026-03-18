# Phase 1 — EVM Specification (Source of Truth)

This document extracts the complete behavioral specification of the GMI NFT Marketplace Exchange from the EVM Solidity implementation. Every invariant, data structure, and flow described here MUST be preserved in the Solana implementation.

---

## 1. Order Model

### 1.1 Order Struct (`LibOrderTypes.Order`)

| Field | Type | Description |
|-------|------|-------------|
| `maker` | `address` | Order creator |
| `makeAsset` | `Asset` | Asset the maker is selling |
| `taker` | `address` | Intended counterparty (0x0 = any) |
| `takeAsset` | `Asset` | Asset the maker wants to receive |
| `salt` | `uint256` | Nonce for order uniqueness. 0 = on-chain order, >0 = off-chain signed |
| `start` | `uint256` | Earliest valid timestamp (0 = no restriction) |
| `end` | `uint256` | Latest valid timestamp (0 = no expiry) |
| `dataType` | `bytes4` | Data format identifier (`V1` = `keccak256("V1")[0:4]`, `0xffffffff` = empty) |
| `data` | `bytes` | ABI-encoded `DataV1` (payouts + origin fees) |
| `collectionBid` | `bool` | True for collection-wide bids |

### 1.2 Asset Structs

```
Asset {
    AssetType assetType;
    uint256 value;          // Amount (1 for ERC-721)
}

AssetType {
    bytes4 assetClass;      // ETH, WETH, ERC20, ERC721, ERC1155
    bytes data;             // abi.encode(tokenAddress) or abi.encode(tokenAddress, tokenId)
}
```

### 1.3 Asset Classes

| Constant | Value | Description |
|----------|-------|-------------|
| `ETH_ASSET_CLASS` | `keccak256("ETH")[0:4]` | Native ETH |
| `WETH_ASSET_CLASS` | `keccak256("WETH")[0:4]` | Wrapped ETH |
| `PROXY_WETH_ASSET_CLASS` | `keccak256("PROXY_WETH")[0:4]` | WETH held by proxy (internal) |
| `ERC20_ASSET_CLASS` | `keccak256("ERC20")[0:4]` | ERC-20 tokens |
| `ERC721_ASSET_CLASS` | `keccak256("ERC721")[0:4]` | ERC-721 NFTs |
| `ERC1155_ASSET_CLASS` | `keccak256("ERC1155")[0:4]` | ERC-1155 semi-fungibles |

### 1.4 Order Data V1

```
DataV1 {
    Part[] payouts;        // Where to send proceeds (bps, must sum to 10000)
    Part[] originFees;     // Additional fees (bps of order amount)
}

Part {
    address payable account;
    uint16 value;          // Basis points (0-10000)
}
```

### 1.5 Salt Logic

- **`salt == 0`**: On-chain order. No signature required. Either:
  - `order.maker == tx.origin` (caller is maker)
  - `order.maker == address(0)` → set maker to `tx.origin`
  - `order.collectionBid == true` → allowed (collection bid formatted orders)
- **`salt > 0`**: Off-chain order. Requires:
  - EIP-712 signature from `order.maker` (if caller != maker)
  - ERC-1271 contract signature (if maker is contract)
  - OrderBook `matchAllowance` signature

### 1.6 Collection Bids

- `collectionBid == true` on maker order
- Maker order `takeAsset.assetType.data` encodes `(collectionAddress, 0)` (any tokenId)
- Must go through `ExchangeHelper.matchCollectionBidOrder`, NOT direct `matchOrders`
- Taker orders must have `collectionBid == false`
- Maker order `makeAsset` must be WETH or ERC20
- Maker order `takeAsset` must be ERC721 or ERC1155
- Multiple taker orders matched in batch against single collection bid

---

## 2. Signature System

### 2.1 EIP-712 Domain

```
EIP712Domain {
    name: "Energi",
    version: "1",
    chainId: <configured chain ID>,
    verifyingContract: <Exchange proxy address>
}
```

**Critical**: The verifying contract is the PROXY address, not implementation, ensuring signatures survive upgrades.

### 2.2 Order Hash (for maker signature)

```
ORDER_TYPEHASH = keccak256(
    'Order(address maker,Asset makeAsset,address taker,Asset takeAsset,uint256 salt,uint256 start,uint256 end,bytes4 dataType,bytes data,bool collectionBid)Asset(AssetType assetType,uint256 value)AssetType(bytes4 assetClass,bytes data)'
)

hash(order) = keccak256(abi.encode(
    ORDER_TYPEHASH,
    order.maker,
    hash(order.makeAsset),
    order.taker,
    hash(order.takeAsset),
    order.salt,
    order.start,
    order.end,
    order.dataType,
    keccak256(order.data),
    order.collectionBid
))
```

### 2.3 Order Key Hash (order identity)

```
hashKey(order) = keccak256(abi.encode(
    order.maker,
    hash(order.makeAsset.assetType),
    hash(order.takeAsset.assetType),
    order.salt,
    order.collectionBid
))
```

This is used as the key in the `fills` mapping.

### 2.4 Match Allowance Hash (for orderbook signature)

```
MATCH_ALLOWANCE_TYPEHASH = keccak256('MatchAllowance(bytes32 orderKeyHash,uint256 matchBeforeTimestamp)')

hash(orderKeyHash, matchBeforeTimestamp) = keccak256(abi.encode(
    MATCH_ALLOWANCE_TYPEHASH,
    orderKeyHash,
    matchBeforeTimestamp
))
```

### 2.5 Signature Validation Flow

For each order in `matchOrders`:

1. **`salt == 0`**: No signature. Caller must be maker (or maker is zero for auto-assignment).
2. **`salt > 0`**:
   a. If `callerAddress != order.maker`:
      - Compute EIP-712 `hashStruct` of order
      - Recover signer via ECDSA
      - If signer != maker, check ERC-1271 `isValidSignature` on maker contract
   b. If `callerAddress == order.maker`: No order signature needed.
   c. **matchAllowance**: OrderBook must sign `(orderKeyHash, matchBeforeTimestamp)` with `matchBeforeTimestamp > block.timestamp`

### 2.6 Signature Malleability Protection

- `s` value must be in lower half of secp256k1 curve
- `v` must be 27 or 28 (or 31/32 for `eth_sign` prefix)

---

## 3. Matching Engine

### 3.1 matchOrders Flow

```
matchOrders(leftOrder, sigLeft, matchLeftBefore, obSigLeft, rightOrder, sigRight, matchRightBefore, obSigRight):
1. REQUIRE: rightOrder.makeAsset.assetClass != ETH (maker cannot pay with ETH)
2. REQUIRE: collection bids must go through ExchangeHelper
3. checkERC20TokensAllowed(leftOrder, rightOrder) via ExchangeHelper:
   a. validate(leftOrder) — time + asset class checks
   b. validate(rightOrder) — time + asset class checks
   c. Check ERC20 whitelist for any ERC20 assets in either order
   d. checkCounterparties(leftOrder, rightOrder)
4. verifyMatch via ExchangeHelper:
   a. Compute leftOrderKeyHash, rightOrderKeyHash
   b. For salt > 0 orders: verify matchAllowance signatures
5. verifyOrder for rightOrder (maker) — signature check
6. verifyOrder for leftOrder (taker) — signature check
7. matchAndTransfer:
   a. matchAssets — verify asset type compatibility
   b. calculateFills — partial fill computation
   c. processEthAndWeth — handle ETH/WETH conversions
   d. doTransfers — execute fee+royalty+payout pipeline
```

### 3.2 Asset Compatibility (matchAssets)

The `matchAssets` function verifies cross-order compatibility:

- **ETH ↔ ETH**: Compatible (ETH and WETH are interchangeable)
- **ETH ↔ WETH**: Compatible
- **WETH ↔ WETH**: Compatible
- **ERC20 ↔ ERC20**: Same token address required
- **ERC721 ↔ ERC721**: Same token address AND tokenId required
- **ERC1155 ↔ ERC1155**: Same token address AND tokenId required

Returns `(makerAssetType, takerAssetType)` — the resolved asset types.

### 3.3 Order Validation (`LibOrder.validate`)

1. `start == 0 || start < block.timestamp`
2. `end == 0 || end > block.timestamp`
3. Fungible↔Non-Fungible enforcement:
   - If make is fungible (ETH/WETH/ERC20), take MUST be non-fungible (ERC721/ERC1155)
   - If take is fungible, make MUST be non-fungible
   - If make is non-fungible, take MUST be fungible
   - **NFT↔NFT trades are strictly prohibited**

### 3.4 Counterparty Checks

- If `leftOrder.taker != 0`, then `rightOrder.maker == leftOrder.taker`
- If `rightOrder.taker != 0`, then `rightOrder.taker == leftOrder.maker`

### 3.5 ERC20 Whitelist

Both orders' make and take assets are checked against `allowedERC20Assets` mapping if their class is ERC20.

### 3.6 batchMatchOrders

- Orders array must have even length
- Pairs matched at `(i, i+1)` where `i` is even
- Each pair goes through full `matchOrders` flow

---

## 4. Fill Calculation

### 4.1 calculateRemaining

```
calculateRemaining(order, takeAssetFill):
  REQUIRE: takeAssetFill < UINT256_MAX  // not cancelled
  takeValue = order.takeAsset.value - takeAssetFill
  makeValue = safeGetPartialAmountFloor(order.makeAsset.value, order.takeAsset.value, takeValue)
  return (makeValue, takeValue)
```

### 4.2 fillOrder

```
fillOrder(leftOrder, rightOrder, leftFill, rightFill):
  (leftMakeValue, leftTakeValue) = calculateRemaining(leftOrder, leftFill)
  (rightMakeValue, rightTakeValue) = calculateRemaining(rightOrder, rightFill)
  
  if rightTakeValue > leftMakeValue:
    return fillLeft(leftMakeValue, leftTakeValue, rightOrder.makeAsset.value, rightOrder.takeAsset.value)
  else:
    return fillRight(leftOrder.makeAsset.value, leftOrder.takeAsset.value, rightMakeValue, rightTakeValue)
```

### 4.3 fillLeft (left order fully filled)

```
fillLeft(leftMakeValue, leftTakeValue, rightMakeValue, rightTakeValue):
  rightTake = safeGetPartialAmountFloor(leftTakeValue, rightMakeValue, rightTakeValue)
  REQUIRE: rightTake <= leftMakeValue
  return FillResult(rightTake, leftTakeValue)
```

### 4.4 fillRight (right order fully filled)

```
fillRight(leftMakeValue, leftTakeValue, rightMakeValue, rightTakeValue):
  leftTake = safeGetPartialAmountFloor(rightTakeValue, leftMakeValue, leftTakeValue)
  REQUIRE: leftTake <= rightMakeValue
  return FillResult(rightTakeValue, leftTake)
```

### 4.5 FillResult

```
FillResult {
    rightOrderTakeValue: uint256    // Amount maker receives
    leftOrderTakeValue: uint256     // Amount taker receives
}
```

### 4.6 Fill Storage

- Only orders with `salt > 0` have fills stored
- `salt == 0` orders always start with fill = 0
- Fills are monotonically increasing (add only)
- Cancel sets fill to `UINT256_MAX`
- Fill stored per `orderKeyHash`

### 4.7 SafeGetPartialAmountFloor (LibMath)

```
safeGetPartialAmountFloor(numerator, denominator, target):
  REQUIRE: denominator != 0
  if numerator == 0 || target == 0: return 0
  REQUIRE: !isRoundingErrorFloor(numerator, denominator, target)  // 0.1% threshold
  return (numerator * target) / denominator

isRoundingErrorFloor(numerator, denominator, target):
  remainder = mulmod(target, numerator, denominator)
  return (1000 * remainder) >= (numerator * target)
```

---

## 5. Fee & Royalty System

### 5.1 Fee Side Determination (`LibFeeSide`)

Determines which side pays protocol fees + royalties. Priority order:

| Rank | Asset Class | Fee Side |
|------|-------------|----------|
| 1 | ETH (maker receives) | TAKE (taker pays fees) |
| 2 | ETH (taker receives) | MAKE (maker pays fees) |
| 3 | WETH/PROXY_WETH (maker receives) | TAKE |
| 4 | WETH/PROXY_WETH (taker receives) | MAKE |
| 5 | ERC20 (maker receives) | TAKE |
| 6 | ERC20 (taker receives) | MAKE |
| 7 | ERC1155 (maker receives) | TAKE |
| 8 | ERC1155 (taker receives) | MAKE |
| 9 | None of above | NONE |

**Key Insight**: The NFT seller (whoever sells ERC721/ERC1155) pays fees. The fungible asset is the fee asset.

### 5.2 Transfer Pipeline (`doTransfers`)

When `feeSide == MAKE`:
```
1. doTransfersWithFees(leftOrderTakeValue, rightOrder.maker, rightOrderData, leftOrderData, makerAssetType, takerAssetType, TO_TAKER)
2. transferPayouts(makerAssetType, rightOrderTakeValue, leftOrder.maker, rightOrderData.payouts, TO_MAKER)
```

When `feeSide == TAKE`:
```
1. doTransfersWithFees(rightOrderTakeValue, leftOrder.maker, leftOrderData, rightOrderData, takerAssetType, makerAssetType, TO_MAKER)
2. transferPayouts(takerAssetType, leftOrderTakeValue, rightOrder.maker, leftOrderData.payouts, TO_TAKER)
```

When `feeSide == NONE`:
```
1. transferPayouts(makerAssetType, leftOrderTakeValue, leftOrder.maker, rightOrderData.payouts, TO_MAKER)
2. transferPayouts(takerAssetType, rightOrderTakeValue, rightOrder.maker, leftOrderData.payouts, TO_TAKER)
```

### 5.3 doTransfersWithFees

```
doTransfersWithFees(amount, from, feePayerData, otherData, feePayerAssetType, otherAssetType, direction):
  totalAmount = calculateTotalAmount(amount, feePayerData.originFees)
  rest = transferProtocolFee(totalAmount, amount, from, otherAssetType, direction)
  rest = transferRoyalties(otherAssetType, feePayerAssetType, rest, amount, from, direction)
  rest = transferFees(otherAssetType, rest, amount, feePayerData.originFees, from, direction, ORIGIN)
  rest = transferFees(otherAssetType, rest, amount, otherData.originFees, from, direction, ORIGIN)
  transferPayouts(otherAssetType, rest, from, otherData.payouts, direction)
```

### 5.4 Protocol Fee

```
transferProtocolFee(totalAmount, amount, from, assetType, direction):
  (rest, protocolFee) = subFeeInBps(totalAmount, amount, protocolFeeBps)
  if protocolFee > 0:
    transfer(Asset(assetType, protocolFee), from, getFeeReceiver(tokenAddress), direction, PROTOCOL)
  return rest
```

- Protocol fee is `protocolFeeBps` basis points of the `amount`
- Subtracted from `totalAmount` (which includes origin fees)
- Fee receiver: `feeReceivers[tokenAddress]` if set, otherwise `defaultFeeReceiver`
- WETH protocol fees are converted to ETH before transfer

### 5.5 Royalties

Resolution order:
1. `RoyaltiesRegistry.getRoyalties(token, tokenId)` — registry-based
2. If empty: try `ERC-2981.royaltyInfo(tokenId, amount)` on the token contract

Registry resolution (`RoyaltiesRegistry.getRoyalties`):
1. Owner royalties by (token, tokenId)
2. If not initialized: Owner royalties by token (collection-level)
3. Creator royalties by (token, tokenId)
4. If both owner and creator exist: merge arrays
5. If only one exists: return that
6. If neither exists:
   a. Try external provider
   b. Try Rarible V2 interface on contract
   c. Try Rarible V1 interface on contract
   d. Return empty

**CRITICAL INVARIANT**: `totalRoyaltiesBps <= 5000` (50% cap enforced)

### 5.6 Origin Fees

- From `feePayerData.originFees` and `otherData.originFees`
- Each is calculated as `bps(amount, fee.value)` and subtracted from `rest`
- Added to `totalAmount` only for the fee payer side

### 5.7 Payouts

- From order data `payouts` array
- Each payout's bps must sum to exactly 10000
- All payouts except the last use `bps(amount, payout.value)`
- Last payout receives the **remainder** (avoids rounding dust)

### 5.8 calculateTotalAmount

```
calculateTotalAmount(amount, originFees):
  total = amount
  for each fee in originFees:
    total += bps(amount, fee.value)
  return total
```

### 5.9 subFeeInBps

```
subFeeInBps(rest, total, feeInBps):
  fee = bps(total, feeInBps)
  if rest > fee:
    return (rest - fee, fee)
  else:
    return (0, rest)
```

---

## 6. State & Storage

### 6.1 ExchangeStorage

| Field | Type | Description |
|-------|------|-------------|
| `fills` | `mapping(bytes32 => uint256)` | Order key hash → fill amount |
| `allowedERC20Assets` | `mapping(address => bool)` | ERC20 whitelist |
| `feeReceivers` | `mapping(address => address)` | Per-token fee receiver overrides |
| `helperProxy` | `address` | ExchangeHelper proxy |
| `orderBook` | `address` | OrderBook service pubkey |
| `defaultFeeReceiver` | `address` | Default protocol fee destination |
| `royaltiesRegistryProxy` | `address` | RoyaltiesRegistry proxy |
| `weth` | `address` | WETH contract address |
| `exchangeOwner` | `address` | Owner for fee/whitelist admin |
| `protocolFeeBps` | `uint16` | Protocol fee in basis points |
| `chainId` | `uint256` | Chain identifier |

### 6.2 RoyaltiesRegistryStorage

| Field | Type | Description |
|-------|------|-------------|
| `ownerRoyaltiesByTokenAndTokenId` | `mapping(bytes32 => RoyaltiesSet)` | Per-token owner royalties |
| `creatorRoyaltiesByTokenAndTokenId` | `mapping(bytes32 => RoyaltiesSet)` | Per-token creator royalties |
| `royaltiesByToken` | `mapping(address => RoyaltiesSet)` | Per-collection royalties |
| `royaltiesProviders` | `mapping(address => address)` | External royalty provider per token |

### 6.3 Cancel Mechanism

```
cancelOrder(order):
  REQUIRE: tx.origin == order.maker
  REQUIRE: order.salt != 0
  hash = hashKey(order)
  fills[hash] = UINT256_MAX    // Permanent cancellation
```

---

## 7. ETH/WETH Handling

### 7.1 msg.value > 0

- Taker sends ETH
- If maker wants ETH: forward to proxy
- If maker wants WETH: wrap ETH to WETH, transfer to proxy, use PROXY_WETH class

### 7.2 msg.value == 0, ETH involved

- If maker wants ETH: taker must be sending WETH → unwrap → forward to proxy
- If taker wants ETH: maker must be sending WETH → unwrap → forward to proxy
- If maker wants WETH but taker sending WETH: direct WETH transfer

### 7.3 Proxy holds all funds

The Exchange proxy contract holds ETH and WETH during transfer execution. All payments flow through the proxy.

---

## 8. Critical Invariants

### INV-1: Fungible ↔ Non-Fungible Only
No NFT↔NFT or fungible↔fungible trades allowed.

### INV-2: Fills Monotonically Increase
`fills[hash]` can only increase (via addition) or be set to MAX (cancel). Never decreases.

### INV-3: Royalties Capped at 50%
`totalRoyaltiesBps <= 5000` enforced in `transferRoyalties`.

### INV-4: Payouts Sum to 100%
`sumBps == 10000` enforced in `transferPayouts`.

### INV-5: Maker Cannot Pay with ETH
`orderRight.makeAsset.assetType.assetClass != ETH_ASSET_CLASS` enforced at top of `matchOrders`.

### INV-6: OrderBook Signature Required for salt > 0
All orders with non-zero salt require a valid matchAllowance from the order book service.

### INV-7: matchAllowance Not Expired
`matchBeforeTimestamp > block.timestamp` enforced.

### INV-8: ERC20 Whitelist
Only whitelisted ERC20 tokens can be used in orders.

### INV-9: Signature Domain Binding
Orders are bound to (chainId, verifyingContract proxy address) via EIP-712 domain.

### INV-10: Collection Bids Through Helper Only
`collectionBid == true` orders must go through `ExchangeHelper`, not direct `matchOrders`.

### INV-11: Cancellation is Permanent
Setting fill to MAX cannot be undone.

### INV-12: Reentrancy Protection
`transfer()` function has `nonReentrant` modifier.

### INV-13: Pause Mechanism
`whenNotPaused` on `matchOrders`, `batchMatchOrders`, `cancelOrder`.

### INV-14: Value Conservation
`totalAmount = amount + sum(originFees)`. All of `totalAmount` is distributed to: protocol fee + royalties + origin fees + payouts. No funds are lost or created.

### INV-15: ERC721 Value Must Be 1
`require(_asset.value == 1)` for ERC721 transfers.

### INV-16: No Zero-Address Transfers
`require(_to != address(0))` for all transfers.

### INV-17: No Zero-Amount Transfers
`require(_asset.value != 0)` for all transfers.
