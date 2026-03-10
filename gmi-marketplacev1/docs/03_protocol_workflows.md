# 03 — Protocol Workflows

---

## 1. Order Creation & Signing (Off-Chain)

### Step-by-Step

1. **User constructs an Order struct:**
   ```
   Order {
     maker:        user's address
     makeAsset:    { assetType: { assetClass, data }, value }
     taker:        address(0) (anyone can fill) or specific counterparty
     takeAsset:    { assetType: { assetClass, data }, value }
     salt:         random nonce > 0 (for off-chain orders) or 0 (for direct submission)
     start:        0 or Unix timestamp when order becomes valid
     end:          0 or Unix timestamp when order expires
     dataType:     bytes4(keccak256('V1')) or 0xffffffff (empty)
     data:         ABI-encoded DataV1 { payouts[], originFees[] } or empty bytes
     collectionBid: false (normal order) or true (collection-wide bid)
   }
   ```

2. **User computes EIP-712 hash:**
   - Domain: `{ name: "Energi", version: "1", chainId, verifyingContract: ExchangeProxy }`
   - TypeHash: `Order(address maker,Asset makeAsset,address taker,Asset takeAsset,uint256 salt,uint256 start,uint256 end,bytes4 dataType,bytes data,bool collectionBid)Asset(AssetType assetType,uint256 value)AssetType(bytes4 assetClass,bytes data)`
   - Hash: `keccak256("\x19\x01" || domainSeparator || hashStruct)`

3. **User signs the EIP-712 hash** with their private key (producing a 65-byte signature: `r, s, v`).

4. **User submits the signed order to the Order Book service.**

5. **Order Book validates the order** and stores it. It does NOT submit it on-chain yet.

---

## 2. Match Allowance Signing (Off-Chain — Order Book)

### Step-by-Step

1. **Order Book identifies a matching pair** (left/taker order + right/maker order).

2. **For each order with `salt > 0`, Order Book signs a matchAllowance:**
   - TypeHash: `MatchAllowance(bytes32 orderKeyHash,uint256 matchBeforeTimestamp)`
   - `orderKeyHash`: `keccak256(maker, hash(makeAsset.assetType), hash(takeAsset.assetType), salt, collectionBid)`
   - `matchBeforeTimestamp`: a future Unix timestamp after which the match is invalid
   - Signed using EIP-712 with the same domain as orders

3. **Order Book submits the matched pair** (or delegates to a relayer) along with both order signatures and both matchAllowance signatures.

---

## 3. Order Matching (On-Chain — `matchOrders`)

### Step-by-Step

```
Caller → Exchange.matchOrders(
    orderLeft,           // Taker order
    signatureLeft,       // Taker's EIP-712 signature
    matchLeftBeforeTimestamp,
    orderBookSignatureLeft,
    orderRight,          // Maker order
    signatureRight,      // Maker's EIP-712 signature
    matchRightBeforeTimestamp,
    orderBookSignatureRight
)
```

1. **Validate maker cannot pay with ETH:**
   - `require(orderRight.makeAsset.assetType.assetClass != ETH_ASSET_CLASS)`

2. **Check collection bid flag:**
   - If either order has `collectionBid = true`, caller must be ExchangeHelper.

3. **Validate ERC-20 whitelist** (via `ExchangeHelper.checkERC20TokensAllowed`):
   - For each order, if makeAsset or takeAsset is ERC-20, check `allowedERC20Assets[address]`.
   - Validate order time constraints (start/end).
   - Validate asset class compatibility (fungible ↔ non-fungible only).
   - Check counterparty constraints (if `taker != address(0)`, verify it matches).

4. **Verify Order Book matchAllowance signatures** (via `ExchangeHelper.verifyMatch`):
   - For each order with `salt > 0`:
     - Compute `orderKeyHash`
     - Verify `matchBeforeTimestamp > block.timestamp`
     - Recover signer from the matchAllowance signature
     - Verify recovered signer == `orderBook` address

5. **Verify maker and taker order signatures** (via `ExchangeHelper.verifyOrder`):
   - For each order:
     - If `salt == 0`: verify `tx.origin == order.maker` (or `order.maker == address(0)` → set to caller). No signature needed if `collectionBid == true`.
     - If `salt > 0` and `caller != order.maker`:
       - Compute EIP-712 hash of the order
       - Recover signer from signature via `ecrecover`
       - If signer != maker, try EIP-1271 (`isValidSignature`) on maker's contract
       - Revert if all verification fails

6. **Call `matchAndTransfer`** (see Workflow 4 below).

---

## 4. Asset Matching & Transfer (On-Chain — `matchAndTransfer`)

### Step-by-Step

1. **Match asset types** (via `LibExchange.matchAssets`):
   - Verify `orderRight.takeAsset.assetType` ↔ `orderLeft.makeAsset.assetType` compatibility
   - Verify `orderLeft.takeAsset.assetType` ↔ `orderRight.makeAsset.assetType` compatibility
   - ETH and WETH are mutually compatible
   - ERC-20, ERC-721, ERC-1155 must match exactly (same data/address/tokenId)
   - Returns `makerAssetType` (what maker expects to receive) and `takerAssetType` (what taker expects to receive)

2. **Calculate fill amounts** (via `ExchangeHelper.calculateFills`):
   - Retrieve current fills for both orders from storage
   - For orders with `salt == 0`, fill is always 0 (not tracked)
   - Compute `calculateRemaining(order, fill)` for both orders:
     - `takeValue = order.takeAsset.value - fill`
     - `makeValue = order.makeAsset.value * (takeValue / order.takeAsset.value)`
   - Determine which order gets fully filled:
     - If `rightTakeValue > leftMakeValue`: left order fully filled → `fillLeft`
     - Otherwise: right order fully filled (or both) → `fillRight`
   - Update fills in storage for orders with `salt > 0`
   - Returns `FillResult { rightOrderTakeValue, leftOrderTakeValue }`

3. **Process ETH/WETH conversions** (via `processEthAndWeth`):
   - Calculate `totalMakeValue` and `totalTakeValue` (including origin fees)
   - **If `msg.value > 0`** (taker sending ETH):
     - If maker expects ETH: forward `totalTakeValue` to proxy via `receiveETH()`
     - If maker expects WETH: wrap ETH via `WETH.deposit()`, transfer WETH to proxy, update assetClass to `PROXY_WETH`
   - **If `msg.value == 0`** and ETH is expected by either side:
     - If maker expects ETH but taker is sending WETH: transfer WETH from taker → unwrap → forward ETH to proxy
     - If taker expects ETH but maker is sending WETH: transfer WETH from maker → unwrap → forward ETH to proxy
   - **If maker expects WETH and taker sends WETH**: no conversion needed

4. **Execute transfers** (via `doTransfers`):
   - Determine **fee side** via `getFeeSide(makerAssetClass, takerAssetClass)`:
     - MAKE side = maker pays fees (maker is selling NFT for ETH/WETH/ERC-20)
     - TAKE side = taker pays fees (taker is selling NFT)
   - Parse order data for both sides (payouts, origin fees)
   - Call appropriate transfer functions (see Workflow 5 and 6)

5. **Emit `Match` event** with both order hashes, makers, and new fill values.

---

## 5. Fee Distribution (On-Chain — `doTransfersWithFees`)

### Step-by-Step

This function handles the transfer of the "fee asset" (the fungible asset in the trade).

1. **Calculate total amount:**
   - `totalAmount = amount + sum(originFees of fee-paying side)`
   - Origin fees of the fee-payer are added on top; origin fees of the other side are subtracted from payouts later.

2. **Transfer protocol fee** (via `transferProtocolFee`):
   - `protocolFee = amount * protocolFeeBps / 10000`
   - `rest = totalAmount - protocolFee`
   - If fee asset is WETH: unwrap to ETH first, then transfer ETH
   - If fee asset is ERC-20: transfer directly
   - If fee asset is ETH: transfer from proxy
   - Destination: `feeReceiver[tokenAddress]` or `defaultFeeReceiver`

3. **Transfer royalties** (via `transferRoyalties`):
   - Query `RoyaltiesRegistry.getRoyalties(tokenAddress, tokenId)` for the NFT being traded
   - If no registry royalties, try ERC-2981: `IERC2981(token).royaltyInfo(tokenId, amount)`
   - For each royalty recipient:
     - `royaltyAmount = amount * royaltyBps / 10000`
     - `rest = rest - royaltyAmount`
   - Enforce `totalRoyaltiesBps <= 5000` (50% cap)

4. **Transfer origin fees** (both sides):
   - For each origin fee in fee-payer's order:
     - `feeValue = amount * fee.value / 10000`
     - `rest = rest - feeValue`
     - Transfer to fee recipient
   - For each origin fee in other side's order:
     - Same calculation and transfer

5. **Transfer payouts** (via `transferPayouts`):
   - Distribute remaining `rest` to payout recipients
   - For all payouts except the last: `payoutAmount = amount * payout.value / 10000`
   - Last payout gets whatever remains (avoids rounding issues)
   - Validate `sum(payout.value) == 10000` (100%)

6. **Transfer the non-fee asset** (the NFT):
   - Directly transfer from maker/taker to the other side's payout addresses

---

## 6. Low-Level Transfer (On-Chain — `transfer`)

### Step-by-Step

For each individual asset transfer:

1. **Validate**: `to != address(0)` and `value != 0`

2. **By asset class:**
   - **ETH**: `payable(to).call{value: amount}('')` (transfer from proxy)
   - **ERC-721**: `IERC721(token).safeTransferFrom(from, to, tokenId)` — must be exactly 1
   - **ERC-1155**: `IERC1155(token).safeTransferFrom(from, to, tokenId, value, '')`
   - **PROXY_WETH**: `IERC20(weth).transfer(to, value)` (from proxy's balance)
   - **WETH**: `IERC20(weth).transferFrom(from, to, value)` (from user's balance)
   - **ERC-20**: `IERC20(token).transferFrom(from, to, value)` (from user's balance)
   - **Unknown**: revert

3. **Emit `Transfer` event** with asset class, from, to, data, value, direction, and type.

### Reentrancy Protection
- The `transfer` function has `nonReentrant` modifier.
- The proxy's transfer functions also have `nonReentrant`.

---

## 7. Batch Matching (On-Chain — `batchMatchOrders`)

### Step-by-Step

1. Validate arrays have even length and all arrays match in length.
2. Loop through orders in pairs (i, i+1):
   - Call `matchOrders(orders[i], signatures[i], ..., orders[i+1], signatures[i+1], ...)`
3. Each pair is independently validated and executed.

---

## 8. Order Cancellation (On-Chain — `cancelOrder`)

### Step-by-Step

1. **Validate caller**: `tx.origin == order.maker`
2. **Validate salt**: `order.salt != 0` (only off-chain orders can be cancelled)
3. **Compute order key hash**: `LibOrder.hashKey(order)`
4. **Set fill to MAX**: `storage.setFill(orderKeyHash, UINT256_MAX)`
5. **Emit `CancelOrder` event**

### Batch Cancellation (via ExchangeHelper)
1. Call `ExchangeHelper.batchCancelOrders(orders[])`
2. For each order, calls `Exchange.cancelOrder(order)` via the Exchange proxy

---

## 9. Collection Bid Matching (On-Chain — `ExchangeHelper.matchCollectionBidOrder`)

### Step-by-Step

```
Caller → ExchangeHelper.matchCollectionBidOrder(
    orders[],             // orders[0] = collection bid maker order, orders[1..n] = taker orders
    signatures[],         // signatures[0] = maker sig, signatures[1..n] = taker sigs
    matchBeforeTimestamps[],
    orderBookSignatures[]
)
```

1. **Verify collection bid order** (via `verifyCollectionBid`):
   - Verify maker signature of `orders[0]` using EIP-712
   - Verify Order Book matchAllowance for `orders[0]`

2. **Validate collection bid maker order** (via `LibOrder.validateCollectionBidMakerOrder`):
   - `collectionBid == true`
   - `salt > 0`
   - makeAsset is WETH or ERC-20
   - takeAsset is ERC-721 or ERC-1155

3. **Validate taker orders batch** (via `LibOrder.validateCollectionBidTakerOrdersBatch`):
   - Each taker order has `collectionBid == false`
   - Each taker order's makeAsset token address matches the collection bid's takeAsset token address

4. **Calculate collection bid fills** (via `setCollectionBidOrderFill`):
   - Get current fill for collection bid order
   - Get current fills for all taker orders
   - Calculate remaining amounts using `LibFill.fillCollectionBidOrder`
   - Update collection bid fill in storage

5. **Format matched order pairs** (via `formatCollectionBidOrdersBatch`):
   - For each taker order (i = 1..n):
     - Create a synthetic maker order from the collection bid with specific NFT details and fill amounts
     - Pair: `formattedOrders[(i-1)*2] = takerOrder, formattedOrders[(i-1)*2+1] = syntheticMakerOrder`
   - Verify price consistency: `collectionBid.takeValue / collectionBid.makeValue == sumTakerMake / sumTakerTake`

6. **Format signatures batch**:
   - For each taker: pair taker signature/matchAllowance with empty bytes for synthetic maker order (salt=0, so no sig needed)

7. **Call `Exchange.batchMatchOrders`** with formatted pairs
   - Each pair goes through normal `matchOrders` flow
   - Synthetic maker orders have `salt = 0` and `collectionBid = true`, so they bypass signature and matchAllowance verification

---

## 10. Royalty Configuration (On-Chain — RoyaltiesRegistry)

### Setting Owner Royalties by Token+TokenId

1. **Caller**: Registry owner or `IOwnable(token).owner()` (via `tx.origin`)
2. Delete existing royalties for this token+tokenId
3. Iterate new royalties array:
   - Validate each recipient is not zero address
   - Push to storage
   - Sum bps
4. Validate sum ≤ 10,000
5. Mark as initialized in storage
6. Emit `RoyaltiesSetForToken(token, tokenId, recipients, bps, OWNER)`

### Setting Creator Royalties by Token+TokenId

1. **Caller**: Registry owner or `ICreator(token).creator(tokenId)` (via `tx.origin`)
2. Same flow as owner royalties but stored in creator mappings
3. Emit with `CREATOR` setter flag

### Setting Collection-Level Royalties

1. **Caller**: Registry owner or `IOwnable(token).owner()` (via `tx.origin`)
2. Same flow but keyed by token address only
3. Emit `RoyaltiesSetForContract(token, recipients, bps)`

### Royalty Lookup During Trade

1. Check `ownerRoyaltiesByTokenAndTokenId[token, tokenId]`
2. If not initialized → check `royaltiesByToken[token]` (collection-level)
3. Check `creatorRoyaltiesByTokenAndTokenId[token, tokenId]`
4. If both owner and creator exist → merge arrays
5. If only one → return it
6. If neither → try `providerExtractor(token, tokenId)`:
   - Try `IRoyaltiesRegistry(provider).getRoyalties(token, tokenId)` (uint16 bps)
   - Try `IRoyaltiesProviders(provider).getRoyalties(token, tokenId)` (uint96 bps, Rarible style)
   - Try `IRoyaltiesProviders(provider).royaltyFeeInfoCollection(token)` (LooksRare style)
7. If no provider → try `royaltiesFromContract(token, tokenId)`:
   - Try Rarible V2: `supportsInterface(_INTERFACE_ID_ROYALTIES)` → `getRaribleV2Royalties(tokenId)`
   - Try Rarible V1: `supportsInterface(_INTERFACE_ID_FEE_RECIPIENTS)` → `getFeeRecipients(tokenId)` + `getFeeBps(tokenId)`
8. If nothing → return empty array (no royalties)

---

## 11. Admin Configuration Workflows

### Toggle Pause
- **Caller**: Owner
- `Exchange.togglePause()` → calls `_pause()` or `_unpause()`
- When paused: `matchOrders`, `batchMatchOrders`, `cancelOrder`, and all `transfer` calls are blocked

### Set Protocol Fee
- **Caller**: `exchangeOwner` (via ExchangeStorage)
- `ExchangeStorage.setProtocolFeeBps(uint16)`

### Whitelist ERC-20 Token
- **Caller**: `exchangeOwner` (via ExchangeStorage)
- `ExchangeStorage.setERC20AssetAllowed(address, bool)`

### Set Fee Receiver
- **Caller**: `exchangeOwner` (via ExchangeStorage)
- `ExchangeStorage.setFeeReceiver(address token, address receiver)` — per-token fee receiver
- `ExchangeStorage.setDefaultFeeReceiver(address)` — default fee receiver

### Set Order Book
- **Caller**: `owner` (via ExchangeStorage)
- `ExchangeStorage.setOrderBook(address)`

### Upgrade Contracts
- **Caller**: `upgradeManager` (multisig)
- For Exchange: `ExchangeProxy.upgradeToAndCall(newImpl, data)`
- For ExchangeHelper: `ExchangeHelperProxy.upgradeToAndCall(newImpl, data)`
- For RoyaltiesRegistry: `RoyaltiesRegistryProxy.upgradeToAndCall(newImpl, data)`

### Emergency ERC-20 Rescue
- **Caller**: Owner
- `Exchange.safeTransferERC20(token, to, value)` — rescue stuck ERC-20 tokens from implementation

---

## 12. Signature Verification Flow

### EIP-712 Order Signature Verification

```
Input: order, signature, callerAddress, verifyingContractProxy, chainId

1. If order.salt == 0:
   a. If order.collectionBid == true → skip (handled by collection bid flow)
   b. If order.maker != address(0) → require callerAddress == order.maker
   c. If order.maker == address(0) → set order.maker = callerAddress

2. If order.salt > 0 AND callerAddress != order.maker:
   a. Compute hashStruct = LibOrder.hash(order)
      = keccak256(ORDER_TYPEHASH, maker, hash(makeAsset), taker, hash(takeAsset),
                  salt, start, end, dataType, keccak256(data), collectionBid)
   b. Compute EIP712 message hash:
      = keccak256("\x19\x01" || domainSeparator || hashStruct)
      where domainSeparator = keccak256(EIP712Domain, "Energi", "1", chainId, proxy)
   c. Recover signer = ecrecover(messageHash, v, r, s)
      - Handle v > 30 case: adjust with "\x19Ethereum Signed Message" prefix
      - Validate s is in lower half (malleability protection)
   d. If signer == order.maker → valid
   e. If signer != order.maker AND order.maker is a contract:
      - Call IERC1271(order.maker).isValidSignature(messageHash, signature)
      - Must return 0x1626ba7e (magic value)
   f. Otherwise → revert
```

### Match Allowance Signature Verification

```
Input: orderKeyHash, matchBeforeTimestamp, orderBookSignature, proxy, orderBook, chainId

1. require(matchBeforeTimestamp > block.timestamp)
2. Compute hashStruct = keccak256(MATCH_ALLOWANCE_TYPEHASH, orderKeyHash, matchBeforeTimestamp)
3. Compute EIP712 message hash (same domain as orders)
4. Recover signer from orderBookSignature
5. require(signer == orderBook)
```
