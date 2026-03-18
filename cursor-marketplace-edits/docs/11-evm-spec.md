# EVM Marketplace Spec — Energi GMI Exchange

## 1. Function List

| ID | Function | Contract | Visibility |
|----|----------|----------|------------|
| F1 | `matchOrders()` | Exchange.sol | public payable |
| F2 | `batchMatchOrders()` | Exchange.sol | external payable |
| F3 | `cancelOrder()` | Exchange.sol | public |
| F4 | `matchCollectionBidOrder()` | ExchangeHelper.sol | external |
| F5 | `setProtocolFeeBps()` | ExchangeStorage.sol | external |
| F6 | `setDefaultFeeReceiver()` | ExchangeStorage.sol | external |
| F7 | `setFeeReceiver()` | ExchangeStorage.sol | external |
| F8 | `setERC20AssetAllowed()` | ExchangeStorage.sol | external |
| F9 | `setOrderBook()` | ExchangeStorage.sol | external |
| F10 | `setExchangeOwner()` | ExchangeStorage.sol | external |
| F11 | `togglePause()` | Exchange.sol | external |

---

## 2. Function Details

### F1: matchOrders()

**Inputs:**
- `orderLeft` (Order) — taker order
- `signatureLeft` (bytes) — EIP-712 ECDSA signature for taker order
- `matchLeftBeforeTimestamp` (uint256) — order-book expiry for taker
- `orderBookSignatureLeft` (bytes) — order-book signature for taker match allowance
- `orderRight` (Order) — maker order
- `signatureRight` (bytes) — EIP-712 ECDSA signature for maker order
- `matchRightBeforeTimestamp` (uint256) — order-book expiry for maker
- `orderBookSignatureRight` (bytes) — order-book signature for maker match allowance

**State Changes:**
- `fills[leftOrderKeyHash]` += left fill amount (if salt > 0)
- `fills[rightOrderKeyHash]` += right fill amount (if salt > 0)
- Token transfers (ERC20/ERC721/ERC1155/ETH)
- Fee distributions (protocol, royalties, origin fees)

**Require Conditions:**
1. `whenNotPaused`
2. `nonReentrant`
3. Right order make asset ≠ ETH class
4. No collection bids (unless caller is ExchangeHelper)
5. All ERC20 tokens must be on whitelist
6. `matchBeforeTimestamp > block.timestamp` (both sides)
7. Valid ECDSA signatures (or ERC-1271 for contracts)
8. Order not previously cancelled (`fill < UINT256_MAX`)
9. Order time constraints: `start <= now <= end`
10. Assets must be compatible (fungible ↔ non-fungible)

### F2: batchMatchOrders()

**Inputs:** Arrays of orders, signatures, timestamps, orderBookSignatures
**Require:** `orders.length % 2 == 0`, all arrays equal length
**Effect:** Loops and calls `matchOrders()` for each pair

### F3: cancelOrder()

**Inputs:** `order` (Order struct)
**Require:** `tx.origin == order.maker`, `order.salt != 0`
**State Change:** `fills[orderKeyHash] = UINT256_MAX`
**Events:** `CancelOrder(hash)`

---

## 3. Order Lifecycle

### Order Struct
```
Order {
    address maker;          // Creator
    Asset makeAsset;        // Asset offered (type + amount)
    address taker;          // Specific taker (0x0 = any)
    Asset takeAsset;        // Asset requested (type + amount)
    uint256 salt;           // Uniqueness nonce (0 = no sig required)
    uint256 start;          // Validity start (0 = immediate)
    uint256 end;            // Validity end (0 = no expiry)
    bytes4 dataType;        // V1 or empty
    bytes data;             // Encoded payouts + origin fees
    bool collectionBid;     // Collection-wide bid flag
}
```

### Lifecycle Stages
1. **Creation:** Maker constructs order off-chain
2. **Signing:** Maker signs EIP-712 hash with private key
3. **Order Book Registration:** Off-chain service stores order and creates `matchAllowance` signature with expiry timestamp
4. **Matching:** Taker (or order book) submits both orders + signatures to `matchOrders()`
5. **Validation:** On-chain verification of signatures, time, assets, fills
6. **Execution:** Assets transferred, fees deducted, fills updated
7. **Cancellation (optional):** Maker calls `cancelOrder()` → fill = MAX

### Fill Tracking
- `fills[orderKeyHash]` tracks cumulative take-asset filled amount
- Zero-salt orders are not tracked (one-time, no stored fill)
- Cancelled orders have `fill = UINT256_MAX`

---

## 4. Signature Verification Flow

### EIP-712 Domain
```
EIP712Domain(
    string name = "Energi",
    string version = "1",
    uint256 chainId,
    address verifyingContract = proxyAddress
)
```

### Order Signature (maker signs)
```
hash = keccak256(abi.encode(ORDER_TYPEHASH, maker, hashAsset(makeAsset), taker,
    hashAsset(takeAsset), salt, start, end, dataType, keccak256(data), collectionBid))
eip712Hash = keccak256("\x19\x01" || domainSeparator || hash)
signature = ECDSA.sign(eip712Hash, makerPrivateKey)
```

### Match Allowance Signature (order book signs)
```
hash = keccak256(abi.encode(orderKeyHash, matchBeforeTimestamp))
eip712Hash = keccak256("\x19\x01" || domainSeparator || hash)
signature = ECDSA.sign(eip712Hash, orderBookPrivateKey)
```

### Verification Logic
1. **salt > 0:** Recover signer via `ecrecover()`, compare to `order.maker`
2. **salt > 0 + contract maker:** Verify via `ERC-1271.isValidSignature()`
3. **salt == 0:** No signature needed; `msg.sender` must be maker
4. **EIP-191 variant:** If `v > 30`, hash with `\x19Ethereum Signed Message` prefix

### Malleability Protection
- Enforces `s <= secp256k1n/2`

---

## 5. Asset Custody Model

**Direct transfer, no escrow:**

- Maker's assets stay in maker's wallet until trade execution
- Taker's assets stay in taker's wallet (or sent as msg.value for ETH)
- Requires prior `approve()` to Exchange proxy for ERC20/ERC721/ERC1155
- Transfers executed atomically during `matchOrders()`
- If any transfer fails, entire transaction reverts

### Transfer Mechanics by Asset Class
| Asset Class | Transfer Method |
|-------------|----------------|
| ETH | `payable(to).call{value: amount}('')` |
| ERC20 | `IERC20.transferFrom(from, to, amount)` |
| WETH | `IERC20.transferFrom()` or wrap/unwrap + send |
| ERC721 | `IERC721.safeTransferFrom(from, to, tokenId)` |
| ERC1155 | `IERC1155.safeTransferFrom(from, to, id, amount, '')` |

---

## 6. Fee Structure

### Fee Side Determination (priority order)
1. ETH → other side pays fees
2. WETH → other side pays fees
3. ERC20 → other side pays fees
4. ERC1155 → other side pays fees
5. Neither fungible → no fees

### Fee Components
1. **Protocol Fee:** `amount * protocolFeeBps / 10000` → fee receiver
2. **Royalties:** From registry or ERC-2981. Capped at 50% (5000 bps)
3. **Origin Fees:** From order.data, split per Part array
4. **Payouts:** Remainder distributed per Part array (must sum to 100%)

### ETH/WETH Conversions
- Taker can send ETH (`msg.value`), auto-converted if maker expects WETH
- WETH can be auto-unwrapped to ETH for delivery
- Handled in `processEthAndWeth()`

---

## 7. Trust Assumptions

| Entity | Trust Level | Notes |
|--------|-------------|-------|
| Order Book | Semi-trusted | Signs match allowances; expiry timestamps limit window |
| Maker | Self-sovereign | Controls order via private key |
| Taker | Untrusted | Any address can be taker (if order allows) |
| Exchange Owner | Trusted admin | Sets fees, whitelists tokens |
| Owner | Highest privilege | Sets order book, pauses exchange |
| Upgrade Manager | Multisig controlled | Can upgrade implementation |
| Royalties Registry | Trusted | Returns royalty info; capped at 50% |
| Token Contracts | Assumed correct | Standard ERC20/721/1155 behavior expected |
