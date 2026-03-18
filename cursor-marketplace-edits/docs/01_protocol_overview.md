# 01 — Protocol Overview: Energi GMI NFT Marketplace Exchange

## 1. High-Level Protocol Description

The Energi GMI NFT Marketplace is an **on-chain order-matching exchange** for trading non-fungible tokens (ERC-721 and ERC-1155) against fungible assets (native ETH, Wrapped ETH, and whitelisted ERC-20 tokens). The protocol is built by Energi Core and operates as a decentralized marketplace where:

- **Orders are created off-chain** — users sign EIP-712 typed-data orders describing what they want to buy or sell.
- **An off-chain Order Book service** curates and validates orders, issuing time-limited `matchAllowance` signatures that authorize on-chain execution.
- **Matching and settlement happen on-chain** — the `Exchange` contract validates signatures, computes fill amounts, deducts fees/royalties, and atomically transfers assets between maker and taker.

The protocol supports:
- **Direct listings** — a seller lists an NFT for a price, a buyer fills it.
- **Direct offers** — a buyer offers WETH/ERC-20 for a specific NFT, the seller accepts.
- **Collection bids** — a buyer places a collection-wide offer (e.g., "buy any CryptoPunk for 10 WETH"), and multiple sellers can fill portions of that bid.
- **Partial fills** — orders can be partially filled over multiple transactions.
- **Batch matching** — multiple order pairs can be matched in a single transaction.

The system enforces:
- **Protocol fees** (configurable basis points) paid by the NFT seller.
- **Creator royalties** sourced from an on-chain Royalties Registry, ERC-2981, Rarible V1/V2 standards, or external royalty providers.
- **Origin fees** — additional fees that can be attached to individual orders.
- **Payout splitting** — order proceeds can be distributed to multiple addresses.

## 2. Economic Model

### Revenue Streams
| Fee Type | Payer | Recipient | Calculation |
|---|---|---|---|
| Protocol Fee | NFT seller (side that sells ERC-721/ERC-1155) | `defaultFeeReceiver` or token-specific `feeReceiver` | `protocolFeeBps` basis points of order amount |
| Creator Royalties | NFT seller | Royalty recipients from RoyaltiesRegistry or ERC-2981 | Variable bps per token/collection, capped at 50% |
| Origin Fees | Order originator (front-end, aggregator) | Specified in order data | Variable bps, added on top for taker-side, subtracted from proceeds for maker-side |

### Asset Flow
- **Fungible assets** (ETH/WETH/ERC-20) flow from buyer to seller (minus fees).
- **Non-fungible assets** (ERC-721/ERC-1155) flow from seller to buyer.
- Only fungible-for-non-fungible trades are allowed. NFT-for-NFT swaps are explicitly disallowed.
- Only whitelisted ERC-20 tokens can be traded (controlled by `allowedERC20Assets` mapping).

### ETH/WETH Conversion
The protocol transparently handles ETH ↔ WETH conversions:
- A buyer can pay with ETH even if the seller expects WETH (and vice versa).
- The Exchange contract wraps/unwraps via the WETH contract as needed.
- Protocol fees on WETH trades are always paid in ETH (unwrapped before forwarding to fee receiver).

## 3. Core Protocol Workflow

```
┌─────────────────────────────────────────────────────────────────────┐
│                        OFF-CHAIN LAYER                              │
│                                                                     │
│  1. User creates order (maker/taker, assets, price, salt, etc.)     │
│  2. User signs order using EIP-712 typed data                       │
│  3. Order submitted to Order Book service                           │
│  4. Order Book validates and stores order                           │
│  5. Order Book signs matchAllowance (time-limited authorization)    │
│  6. Matching engine pairs compatible orders                         │
│  7. Relayer submits matched pair to on-chain Exchange               │
│                                                                     │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        ON-CHAIN LAYER                               │
│                                                                     │
│  Exchange.matchOrders(leftOrder, rightOrder, signatures, ...)       │
│    │                                                                │
│    ├─ 1. Validate ERC-20 tokens are whitelisted                     │
│    ├─ 2. Verify Order Book matchAllowance signatures (time check)   │
│    ├─ 3. Verify maker/taker order signatures (EIP-712 / EIP-1271)   │
│    ├─ 4. Match asset types (verify compatibility)                   │
│    ├─ 5. Calculate fill amounts (handle partial fills)              │
│    ├─ 6. Process ETH/WETH conversions                               │
│    ├─ 7. Determine fee side (who pays protocol fee)                 │
│    ├─ 8. Transfer protocol fee                                      │
│    ├─ 9. Transfer royalties (Registry → ERC-2981 → Rarible V1/V2)   │
│    ├─ 10. Transfer origin fees (both sides)                         │
│    ├─ 11. Transfer remaining payouts                                │
│    └─ 12. Emit Match and Transfer events                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## 4. User Lifecycle

### For a Seller (Listing an NFT)
1. **Approve** — Seller approves the Exchange proxy to transfer their NFT (ERC-721 `setApprovalForAll` or ERC-1155 `setApprovalForAll`).
2. **Create Order** — Seller constructs an order: `makeAsset` = NFT, `takeAsset` = price in ETH/WETH/ERC-20. Sets `salt > 0` for off-chain registration.
3. **Sign Order** — Seller signs the EIP-712 typed-data hash of the order.
4. **Submit to Order Book** — Order is sent to the off-chain Order Book service.
5. **Wait for Match** — Order Book finds a matching buyer and issues `matchAllowance` signatures.
6. **Settlement** — A relayer (or the buyer directly) calls `matchOrders` on-chain.
7. **Receive Payment** — Seller receives the sale price minus protocol fee, royalties, and origin fees.

### For a Buyer (Making an Offer or Buying a Listing)
1. **Approve** — Buyer approves the Exchange proxy to transfer WETH/ERC-20 (or sends ETH with the transaction).
2. **Create Order** — Buyer constructs an order: `makeAsset` = ETH/WETH/ERC-20 amount, `takeAsset` = specific NFT.
3. **Sign Order** — Buyer signs the EIP-712 typed-data hash.
4. **Submit to Order Book** — Order is sent off-chain.
5. **Settlement** — When matched, the buyer receives the NFT.

### For a Collection Bidder
1. **Approve** — Bidder approves WETH/ERC-20 spending.
2. **Create Collection Bid Order** — `collectionBid = true`, `takeAsset` specifies collection address (tokenId = 0 as placeholder), `makeAsset` = total WETH/ERC-20 amount.
3. **Sign and Submit** — Signed and sent to Order Book.
4. **Multiple Fills** — Multiple sellers can fill portions of the bid. `ExchangeHelper.matchCollectionBidOrder` handles batch matching against the single bid.

### Order Cancellation
- The order maker calls `cancelOrder(order)` which sets the order's fill to `UINT256_MAX`, permanently preventing any future matching.
- Batch cancellation is available via `ExchangeHelper.batchCancelOrders`.

## 5. Roles

| Role | Address(es) | Permissions |
|---|---|---|
| **Owner** | Set at initialization, transferable via `OwnableUpgradeable` | Toggle pause, transfer ownership, set upgrade manager, emergency ERC-20 rescue |
| **Upgrade Manager** | Set at initialization, changeable by owner | Authorize UUPS upgrades for Exchange, ExchangeHelper, and RoyaltiesRegistry |
| **Exchange Owner (Storage)** | Stored in `ExchangeStorage.exchangeOwner` | Set `defaultFeeReceiver`, `feeReceivers`, `protocolFeeBps`, `allowedERC20Assets` |
| **Order Book** | Off-chain service with known public key stored on-chain | Signs `matchAllowance` to authorize time-limited order matching |
| **Maker** | Any user | Creates and signs orders (sell-side or buy-side) |
| **Taker** | Any user | Fills existing orders; if `salt == 0`, can submit directly without Order Book signature |
| **Relayer** | Any EOA | Submits matched order pairs on-chain (calls `matchOrders`) |
| **Token Owner** | Owner of an NFT collection contract | Can set royalties for their collection in the RoyaltiesRegistry |
| **Token Creator** | Creator of a specific tokenId | Can set creator royalties for their token in the RoyaltiesRegistry |

## 6. Contract Interactions

```
                    ┌──────────────────────┐
                    │   ExchangeProxy      │ ← Users interact here (entry point)
                    │   (ERC1967Proxy)     │   Delegates all calls to Exchange impl
                    │   Holds ETH/tokens   │
                    └──────────┬───────────┘
                               │ delegatecall
                               ▼
                    ┌──────────────────────┐
                    │     Exchange         │ ← Business logic
                    │   (Implementation)   │
                    └──┬───────┬───────┬───┘
                       │       │       │
          ┌────────────┘       │       └────────────┐
          ▼                    ▼                    ▼
┌─────────────────┐  ┌──────────────────┐  ┌──────────────────────┐
│ ExchangeStorage │  │ ExchangeHelper   │  │ RoyaltiesRegistry    │
│ (Separate SC)   │  │ (via Proxy)      │  │ (via Proxy)          │
│ - fills         │  │ - verifyOrder    │  │ - getRoyalties       │
│ - settings      │  │ - verifyMatch    │  │ - setRoyalties       │
│ - allowedERC20  │  │ - matchAssets    │  │ - providerExtractor  │
│ - fees          │  │ - calculateFills │  │                      │
└─────────────────┘  │ - collectionBids │  └──────────┬───────────┘
                     └──────────────────┘             │
                                                      ▼
                                            ┌──────────────────────┐
                                            │ RoyaltiesRegistry    │
                                            │ Storage              │
                                            │ - ownerRoyalties     │
                                            │ - creatorRoyalties   │
                                            │ - royaltiesByToken   │
                                            │ - providers          │
                                            └──────────────────────┘

External Token Contracts:
  ├── IERC20 (WETH, whitelisted ERC-20s)
  ├── IERC721 (NFT collections)
  ├── IERC1155 (Semi-fungible collections)
  ├── IERC2981 (On-chain royalty standard)
  ├── IRoyaltiesV1 (Rarible V1)
  ├── IRoyaltiesV2 (Rarible V2)
  └── IWrappedCoin (WETH deposit/withdraw)
```

## 7. System Architecture

### Proxy Pattern
All three main contracts use the **UUPS (ERC-1967) proxy pattern**:
- `ExchangeProxy` → `Exchange` (implementation)
- `ExchangeHelperProxy` → `ExchangeHelper` (implementation)
- `RoyaltiesRegistryProxy` → `RoyaltiesRegistry` (implementation)

The proxy pattern is used for upgradeability. The `UpgradeManager` role (typically a multisig) must authorize upgrades.

### Storage Separation
Both `Exchange` and `RoyaltiesRegistry` deploy **separate storage contracts** (`ExchangeStorage` and `RoyaltiesRegistryStorage`) during initialization. This pattern means:
- Storage is immutable across upgrades (the storage contract address never changes).
- The implementation contract reads/writes to the storage contract via external calls.
- The storage contract restricts writes to its `owner` (the implementation) and, for some settings, the `exchangeOwner`.

### Library Architecture
Heavy use of libraries for logic separation:
- **LibExchange** — signature verification, asset matching, royalty lookup, fee calculation
- **LibOrder** — order hashing (EIP-712), validation, fill calculation, collection bid formatting
- **LibFill** — fill computation for partial orders
- **LibSignature** — ECDSA recovery with EIP-191 support
- **LibEIP712** — EIP-712 domain separator and message hashing
- **LibMath** — safe partial amount calculations with rounding error checks
- **LibBps** — basis point arithmetic
- **LibFeeSide** — determines which side pays fees based on asset classes
- **LibOrderData** — parses order data into payouts and origin fees
- **LibAsset** — EIP-712 hashing for asset types

## 8. Critical Invariants

1. **Fills monotonically increase** — An order's fill value can only increase. It is never decremented. Cancellation sets fill to `UINT256_MAX`.
2. **Payouts must sum to 100%** — All payout parts in an order must sum to exactly 10,000 bps (100%).
3. **Royalties capped at 50%** — Total royalties for any trade cannot exceed 5,000 bps.
4. **Only fungible-for-non-fungible trades** — The protocol only allows ETH/WETH/ERC-20 to trade against ERC-721/ERC-1155. NFT-for-NFT and ERC-20-for-ERC-20 are blocked.
5. **Order Book authorization required for salt > 0 orders** — Any order with `salt > 0` requires a valid, time-limited `matchAllowance` signature from the Order Book service.
6. **Maker cannot pay with ETH** — Only the taker (left order) can send ETH with the transaction. The maker (right order) must use WETH if they want to pay with the native asset.
7. **Protocol fee is paid by the NFT seller** — The fee side is determined by asset classes. The side selling the ERC-721/ERC-1155 effectively pays the protocol fee by receiving less.
8. **Signature verification is order-specific** — Each order's EIP-712 hash is bound to a specific `chainId` and `verifyingContract` (the proxy address), preventing cross-chain or cross-contract replay.
9. **Collection bids must use WETH or ERC-20** — Collection bid maker orders cannot use ETH (must use WETH or whitelisted ERC-20).
10. **ExchangeHelper is trusted** — The ExchangeHelper contract can set order fills and call `batchMatchOrders` on the Exchange. Only the registered helper proxy address is authorized.

## 9. Security Assumptions

1. **Order Book service is trusted but verified** — The Order Book's public key is stored on-chain. Its `matchAllowance` signatures are verified, and they are time-limited (`matchBeforeTimestamp`). Even if the Order Book is compromised, it cannot forge user order signatures.
2. **Users custody their own assets** — The protocol never takes custody. Transfers happen directly between users via approvals to the Exchange proxy.
3. **EIP-712 signatures are unforgeable** — The protocol relies on ECDSA signature security (secp256k1) and EIP-712 structured data hashing.
4. **Smart contract wallets are supported** — Orders from smart contracts are validated via EIP-1271 (`isValidSignature`).
5. **WETH contract is trusted** — The WETH address is stored in configuration and its `deposit`/`withdraw` functions are called directly.
6. **Proxy is the canonical address** — All EIP-712 signatures use the proxy address as the `verifyingContract`, ensuring signatures remain valid across implementation upgrades.
7. **Reentrancy protection** — The `transfer` function in `Exchange` uses `nonReentrant` from OpenZeppelin. The proxy also has its own `ReentrancyGuard` for its direct transfer functions.
8. **Only EOAs can cancel orders** — `cancelOrder` uses `tx.origin == order.maker`, restricting cancellation to externally owned accounts.
9. **Pausing halts all operations** — The owner can pause the Exchange, which blocks all matching and transfers via `whenNotPaused`.
10. **Upgrade authorization** — Only the designated `upgradeManager` (typically a multisig wallet) can authorize UUPS upgrades, and this is separate from the `owner` role.
