# 05 — Instruction Mapping: Solidity Functions → Solana Instructions

---

## Exchange Program Instructions

---

### 1. `initialize`

**Solidity Function:** `Exchange.initialize(proxy, helperProxy, orderBook, defaultFeeReceiver, royaltiesRegistryProxy, weth, owner, upgradeManager, protocolFeeBps, chainId)`

**Purpose:** Initialize the exchange configuration and storage.

**Solana Instruction:** `initialize`

**Accounts Required:**
| Account | Type | Description |
|---|---|---|
| `exchange_config` | `init`, PDA | Singleton config account |
| `authority` | `Signer` | Deployer / initial owner |
| `system_program` | `Program` | For account creation |

**PDA Derivation:**
- `exchange_config`: `["exchange_config"]` + program_id

**Instruction Data:**
```rust
pub struct InitializeArgs {
    pub order_book: Pubkey,
    pub default_fee_receiver: Pubkey,
    pub royalties_registry_program: Pubkey,
    pub wsol_mint: Pubkey,
    pub exchange_owner: Pubkey,
    pub protocol_fee_bps: u16,
}
```

**Validation Logic:**
- `authority` must be a signer
- `exchange_config` must not already be initialized (init constraint handles this)
- `protocol_fee_bps <= 10000`

**CPI Calls:** None

**Error Handling:**
- `AlreadyInitialized` if config account exists
- `InvalidProtocolFee` if bps > 10000

---

### 2. `match_orders`

**Solidity Function:** `Exchange.matchOrders(orderLeft, signatureLeft, matchLeftBeforeTimestamp, orderBookSignatureLeft, orderRight, signatureRight, matchRightBeforeTimestamp, orderBookSignatureRight)`

**Purpose:** Match a taker (left) order with a maker (right) order, verify signatures, compute fills, and transfer assets.

**Solana Instruction:** `match_orders`

**Accounts Required:**
| Account | Type | Description |
|---|---|---|
| `exchange_config` | PDA, read | Exchange configuration |
| `payer` | `Signer`, `mut` | Transaction fee payer and potential SOL sender |
| `left_maker` | `mut` | Taker's wallet (SOL balance for native transfers) |
| `right_maker` | `mut` | Maker's wallet |
| `left_order_fill` | `init_if_needed`, PDA, `mut` | Fill account for left order |
| `right_order_fill` | `init_if_needed`, PDA, `mut` | Fill account for right order |
| `fee_receiver` | `mut` | Protocol fee recipient |
| `royalties_registry_program` | `Program` | For CPI royalty lookup |
| `token_program` | `Program` | SPL Token program |
| `associated_token_program` | `Program` | ATA program |
| `system_program` | `Program` | For SOL transfers / account creation |
| `instructions_sysvar` | `Sysvar` | For Ed25519/secp256k1 sig verification introspection |
| **Dynamic remaining accounts:** | | |
| — maker's NFT token account | `mut` | Source of NFT (seller) |
| — taker's NFT token account | `mut` | Destination for NFT (buyer) |
| — maker's payment token account | `mut` | Source of payment (if maker pays) |
| — taker's payment token account | `mut` | Source of payment (if taker pays) |
| — NFT mint | read | The NFT mint account |
| — payment mint | read | The payment token mint |
| — royalty recipient(s) token accounts | `mut` | For royalty payouts |
| — payout recipient(s) token accounts | `mut` | For order payout distribution |
| — royalty PDA accounts (from registry) | read | Royalty data |
| — AllowedToken PDA | read | Whitelist check for payment token |

**PDA Derivations:**
- `left_order_fill`: `["order_fill", left_order_key_hash]`
- `right_order_fill`: `["order_fill", right_order_key_hash]`
- `exchange_config`: `["exchange_config"]`

**Instruction Data:**
```rust
pub struct MatchOrdersArgs {
    pub order_left: Order,
    pub signature_left: Vec<u8>,
    pub match_left_before_timestamp: i64,
    pub order_book_signature_left: Vec<u8>,
    pub order_right: Order,
    pub signature_right: Vec<u8>,
    pub match_right_before_timestamp: i64,
    pub order_book_signature_right: Vec<u8>,
}
```

**Validation Logic:**
1. Check `exchange_config.is_paused == false`
2. Validate right order make asset is not SOL (native)
3. If either order has `collection_bid == true`, validate caller is appropriate
4. Validate payment token is in `AllowedToken` whitelist
5. Validate order timestamps (start/end vs `Clock::get()`)
6. Validate asset class compatibility (fungible ↔ non-fungible)
7. Verify counterparty constraints
8. For orders with `salt > 0`: verify Ed25519 signatures via instruction introspection
9. For orders with `salt > 0`: verify Order Book matchAllowance signatures
10. For orders with `salt == 0`: verify maker is a transaction signer
11. Match asset types
12. Calculate fills (check previous fills, compute new fills, verify not cancelled)
13. Process SOL/wSOL conversions if needed
14. Determine fee side
15. Transfer protocol fee
16. CPI to royalties_registry to get royalties, then transfer royalties
17. Transfer origin fees
18. Transfer payouts
19. Transfer NFT
20. Emit events
21. Update fill accounts

**CPI Calls:**
- `royalties_registry::get_royalties` — to fetch royalty data
- `spl_token::transfer` / `spl_token::transfer_checked` — for SPL token transfers
- `system_program::transfer` — for SOL transfers

**Error Handling:**
- `Paused` — exchange is paused
- `MakerCannotPayWithSol` — maker order uses SOL
- `TokenNotAllowed` — payment token not whitelisted
- `OrderExpired` — order past end timestamp
- `OrderNotStarted` — order before start timestamp
- `AssetClassMismatch` — incompatible asset classes
- `InvalidSignature` — signature verification failed
- `MatchAllowanceExpired` — matchBeforeTimestamp exceeded
- `InvalidOrderBookSignature` — Order Book sig mismatch
- `OrderCancelled` — fill is u64::MAX
- `NothingToFill` — fill would be zero
- `RoyaltiesTooHigh` — royalties > 50%
- `InvalidPayoutSum` — payouts don't sum to 10000

---

### 3. `batch_match_orders`

**Solidity Function:** `Exchange.batchMatchOrders(orders[], signatures[], matchBeforeTimestamps[], orderBookSignatures[])`

**Purpose:** Match multiple order pairs in one transaction.

**Solana Instruction:** `batch_match_orders`

**Note:** Due to Solana's transaction size limit (1232 bytes) and compute budget, batch matching will likely be limited to 1-2 pairs per transaction. For larger batches, multiple transactions or versioned transactions with address lookup tables should be used.

**Alternative Design:** Rather than a single batch instruction, each pair can be a separate `match_orders` call within a single Solana transaction. This is more idiomatic for Solana.

---

### 4. `cancel_order`

**Solidity Function:** `Exchange.cancelOrder(Order memory order)`

**Purpose:** Cancel an order by setting its fill to `u64::MAX`.

**Solana Instruction:** `cancel_order`

**Accounts Required:**
| Account | Type | Description |
|---|---|---|
| `exchange_config` | PDA, read | For pause check |
| `maker` | `Signer` | Order maker (must sign to cancel) |
| `order_fill` | `init_if_needed`, PDA, `mut` | Fill account for the order |
| `system_program` | `Program` | For account creation if needed |

**PDA Derivation:**
- `order_fill`: `["order_fill", order_key_hash]`

**Instruction Data:**
```rust
pub struct CancelOrderArgs {
    pub order: Order,
}
```

**Validation Logic:**
1. Check not paused
2. Verify `maker` is a signer AND equals `order.maker`
3. Verify `order.salt != 0`
4. Compute order key hash
5. Set `order_fill.fill_amount = u64::MAX`

**CPI Calls:** None

**Error Handling:**
- `Paused`
- `NotOrderMaker` — signer is not the order maker
- `ZeroSaltCannotCancel` — order has salt == 0

**Note:** The EVM version uses `tx.origin` which in Solana translates to requiring the maker to be a direct signer of the transaction.

---

### 5. `batch_cancel_orders`

**Solidity Function:** `ExchangeHelper.batchCancelOrders(Order[] calldata orders)`

**Purpose:** Cancel multiple orders in one transaction.

**Solana Instruction:** `batch_cancel_orders`

**Accounts Required:**
Same as `cancel_order` but with multiple `order_fill` accounts passed as remaining accounts.

**Instruction Data:**
```rust
pub struct BatchCancelOrdersArgs {
    pub orders: Vec<Order>,
}
```

---

### 6. `match_collection_bid_order`

**Solidity Function:** `ExchangeHelper.matchCollectionBidOrder(orders[], signatures[], matchBeforeTimestamps[], orderBookSignatures[])`

**Purpose:** Match a collection-wide bid against multiple seller orders.

**Solana Instruction:** `match_collection_bid_order`

**Accounts Required:**
Similar to `match_orders` but with additional accounts for multiple taker orders:
| Account | Type | Description |
|---|---|---|
| `exchange_config` | PDA, read | Configuration |
| `payer` | `Signer`, `mut` | Tx fee payer |
| `bid_maker` | `mut` | Collection bidder's wallet |
| `collection_bid_fill` | `init_if_needed`, PDA, `mut` | Fill for the collection bid |
| `instructions_sysvar` | `Sysvar` | For signature verification |
| `token_program` | `Program` | SPL Token |
| `system_program` | `Program` | System program |
| **Remaining accounts (per taker):** | | |
| — taker wallet | `mut` | |
| — taker order fill PDA | `init_if_needed`, `mut` | |
| — taker NFT token account | `mut` | |
| — buyer NFT token account | `mut` | |
| — payment token accounts | `mut` | |

**Instruction Data:**
```rust
pub struct MatchCollectionBidArgs {
    pub collection_bid_order: Order,
    pub collection_bid_signature: Vec<u8>,
    pub collection_bid_match_before_timestamp: i64,
    pub collection_bid_order_book_signature: Vec<u8>,
    pub taker_orders: Vec<Order>,
    pub taker_signatures: Vec<Vec<u8>>,
    pub taker_match_before_timestamps: Vec<i64>,
    pub taker_order_book_signatures: Vec<Vec<u8>>,
}
```

**Validation Logic:**
1. Verify collection bid order: signature + matchAllowance
2. Validate collection bid maker order: `collectionBid == true`, `salt > 0`, make asset is wSOL/SPL, take asset is NFT
3. Validate each taker order: `collectionBid == false`, same collection
4. Calculate fills for collection bid
5. Format matched pairs (synthetic maker orders from collection bid)
6. Execute each matched pair (inline, not via CPI)

**CPI Calls:**
- `spl_token::transfer` for each payment and NFT transfer
- `royalties_registry` CPI for royalty lookups

---

### 7. `set_protocol_fee_bps`

**Solidity Function:** `ExchangeStorage.setProtocolFeeBps(uint16)`

**Purpose:** Update the protocol fee basis points.

**Solana Instruction:** `set_protocol_fee_bps`

**Accounts Required:**
| Account | Type | Description |
|---|---|---|
| `exchange_config` | PDA, `mut` | Config to update |
| `exchange_owner` | `Signer` | Must be `exchange_config.exchange_owner` |

**Instruction Data:**
```rust
pub struct SetProtocolFeeBpsArgs {
    pub new_protocol_fee_bps: u16,
}
```

**Validation:** `exchange_owner` signer matches `exchange_config.exchange_owner`. `new_protocol_fee_bps <= 10000`.

---

### 8. `set_default_fee_receiver`

**Solidity Function:** `ExchangeStorage.setDefaultFeeReceiver(address)`

**Accounts:** `exchange_config` (mut), `exchange_owner` (Signer)

**Instruction Data:** `{ new_default_fee_receiver: Pubkey }`

---

### 9. `set_fee_receiver`

**Solidity Function:** `ExchangeStorage.setFeeReceiver(address token, address receiver)`

**Accounts:** `exchange_config` (read), `exchange_owner` (Signer), `fee_receiver` PDA (init_if_needed, mut), `mint` (read), `system_program`

**PDA:** `["fee_receiver", mint_pubkey]`

**Instruction Data:** `{ receiver: Pubkey }`

---

### 10. `set_allowed_token`

**Solidity Function:** `ExchangeStorage.setERC20AssetAllowed(address, bool)`

**Accounts:** `exchange_config` (read), `exchange_owner` (Signer), `allowed_token` PDA (init_if_needed, mut), `mint` (read), `system_program`

**PDA:** `["allowed_token", mint_pubkey]`

**Instruction Data:** `{ is_allowed: bool }`

---

### 11. `set_order_book`

**Solidity Function:** `ExchangeStorage.setOrderBook(address)`

**Accounts:** `exchange_config` (mut), `owner` (Signer — must be `config.owner`)

**Instruction Data:** `{ new_order_book: Pubkey }`

---

### 12. `toggle_pause`

**Solidity Function:** `Exchange.togglePause()`

**Accounts:** `exchange_config` (mut), `owner` (Signer — must be `config.owner`)

**Instruction Data:** None

**Logic:** `config.is_paused = !config.is_paused`

---

### 13. `safe_transfer_spl`

**Solidity Function:** `Exchange.safeTransferERC20(address token, address to, uint256 value)`

**Purpose:** Emergency rescue of stuck SPL tokens.

**Accounts:** `exchange_config` (read), `owner` (Signer), `source_token_account` (mut, owned by exchange PDA), `destination_token_account` (mut), `exchange_authority` PDA (for signing), `token_program`

**Instruction Data:** `{ amount: u64 }`

---

## Royalties Registry Program Instructions

---

### 14. `initialize` (Registry)

**Solidity Function:** `RoyaltiesRegistry.initialize(owner, upgradeManager)`

**Accounts:** `registry_config` (init, PDA), `authority` (Signer), `system_program`

**PDA:** `["registry_config"]`

---

### 15. `set_royalties_by_collection`

**Solidity Function:** `RoyaltiesRegistry.setRoyaltiesByToken(address token, Part[] royalties)`

**Accounts:**
| Account | Type | Description |
|---|---|---|
| `registry_config` | PDA, read | For owner check |
| `authority` | `Signer` | Must be registry owner or collection authority |
| `collection_royalties` | `init_if_needed`, PDA, `mut` | Royalty data |
| `collection_mint` | read | The NFT collection mint/identifier |
| `system_program` | `Program` | Account creation |

**PDA:** `["collection_royalties", collection_mint]`

**Instruction Data:**
```rust
pub struct SetRoyaltiesByCollectionArgs {
    pub royalties: Vec<RoyaltyPart>,
}
```

**Validation:**
- Authority is registry owner OR collection authority (verify against Metaplex metadata)
- Sum of royalty bps ≤ 10000
- No zero-address recipients

---

### 16. `set_owner_royalties_by_token`

**Solidity Function:** `RoyaltiesRegistry.setOwnerRoyaltiesByTokenAndTokenId(address, uint256, Part[])`

**Accounts:** `registry_config`, `authority` (Signer), `owner_token_royalties` (init_if_needed, PDA, mut), `collection_mint`, `system_program`

**PDA:** `["owner_royalties", collection_mint, token_id_bytes]`

**Validation:** Authority is registry owner or collection owner.

---

### 17. `set_creator_royalties_by_token`

**Solidity Function:** `RoyaltiesRegistry.setCreatorRoyaltiesByTokenAndTokenId(address, uint256, Part[])`

**Accounts:** `registry_config`, `authority` (Signer), `creator_token_royalties` (init_if_needed, PDA, mut), `collection_mint`, `nft_metadata` (read — for creator verification), `system_program`

**PDA:** `["creator_royalties", collection_mint, token_id_bytes]`

**Validation:** Authority is registry owner or NFT creator (from Metaplex metadata).

---

### 18. `set_provider_by_collection`

**Solidity Function:** `RoyaltiesRegistry.setProviderByToken(address, address)`

**Accounts:** `registry_config`, `authority` (Signer), `royalty_provider` (init_if_needed, PDA, mut), `collection_mint`, `system_program`

**PDA:** `["royalty_provider", collection_mint]`

**Instruction Data:** `{ provider_program: Pubkey }`

---

### 19. `get_royalties` (CPI-callable view)

**Solidity Function:** `RoyaltiesRegistry.getRoyalties(address token, uint256 tokenId)`

**Purpose:** Look up royalties for a specific NFT. Called by the exchange program via CPI.

**Accounts:**
| Account | Type | Description |
|---|---|---|
| `owner_token_royalties` | PDA, read (optional) | Owner royalties by token+id |
| `collection_royalties` | PDA, read (optional) | Collection-level royalties |
| `creator_token_royalties` | PDA, read (optional) | Creator royalties by token+id |
| `royalty_provider` | PDA, read (optional) | External provider config |

**Design Note:** On Solana, "view" functions don't exist as CPI. Instead, the exchange program can read the royalty PDA accounts directly (if both programs share the same data schema or the accounts are passed to the exchange instruction). The exchange program derives the PDA addresses, reads the accounts, and applies the cascading lookup logic locally.

**Alternative:** The royalty accounts can be passed as remaining accounts to `match_orders`, and the exchange program reads them directly without CPI. This is more gas-efficient and avoids CPI overhead.

---

## Order Struct (Shared)

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Order {
    pub maker: Pubkey,
    pub make_asset: Asset,
    pub taker: Pubkey,         // Pubkey::default() for "anyone"
    pub take_asset: Asset,
    pub salt: u64,
    pub start: i64,            // 0 for no start constraint
    pub end: i64,              // 0 for no end constraint
    pub data_type: [u8; 4],
    pub data: Vec<u8>,         // Borsh-encoded DataV1
    pub collection_bid: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Asset {
    pub asset_type: AssetType,
    pub value: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AssetType {
    pub asset_class: AssetClass,
    pub mint: Pubkey,          // Token mint (Pubkey::default() for SOL)
    pub token_id: u64,         // 0 for fungible tokens
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum AssetClass {
    Sol,          // Native SOL
    WrappedSol,   // wSOL (SPL)
    SplToken,     // Generic SPL token (ERC-20 equivalent)
    Nft,          // Non-fungible (ERC-721 equivalent, supply=1)
    SemiFungible, // Semi-fungible (ERC-1155 equivalent)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DataV1 {
    pub payouts: Vec<PayoutPart>,
    pub origin_fees: Vec<FeePart>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayoutPart {
    pub account: Pubkey,
    pub value: u16,  // basis points
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FeePart {
    pub account: Pubkey,
    pub value: u16,  // basis points
}
```
