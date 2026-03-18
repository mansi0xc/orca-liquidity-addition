# 04 — Solana Migration Plan

---

## 1. Contract-to-Program Mapping

| EVM Contract | Solana Equivalent | Rationale |
|---|---|---|
| `Exchange` (implementation) | **`exchange` Anchor program** | Core business logic: order matching, transfers, fee distribution |
| `ExchangeProxy` (ERC-1967) | **Not needed** | Solana programs are directly callable; upgradeability via Anchor's `upgrade` authority |
| `ExchangeStorage` | **PDA state accounts** within `exchange` program | No separate storage contract; state lives in program-owned PDAs |
| `ExchangeHelper` | **Merged into `exchange` program** | Solana programs can be large; no contract size limit like EVM's 24KB. All helper logic inlined. |
| `ExchangeHelperProxy` | **Not needed** | No proxy pattern |
| `RoyaltiesRegistry` | **`royalties_registry` Anchor program** | Separate program for royalty management (clear separation of concerns, independent upgradeability) |
| `RoyaltiesRegistryProxy` | **Not needed** | No proxy pattern |
| `RoyaltiesRegistryStorage` | **PDA state accounts** within `royalties_registry` program | Royalty data stored in PDAs |
| `StorageBase` | **Not needed** | Ownership pattern replaced by Anchor account constraints |
| `UpgradeManager` | **Program upgrade authority** | Solana's native upgrade authority (can be a multisig) |

### Design Decision: Two Programs

The protocol is split into **two Anchor programs**:
1. **`exchange`** — handles order matching, fills, transfers, fee distribution, all configuration
2. **`royalties_registry`** — handles royalty configuration and lookup

This mirrors the EVM architecture where Exchange and RoyaltiesRegistry are separate upgradeable contracts. The exchange program will call the royalties registry via CPI (Cross-Program Invocation) during trades.

---

## 2. Anchor Programs and Their Components

### Program 1: `exchange`

**Instructions (entry points):**
- `initialize` — set up exchange config
- `match_orders` — match a taker/maker order pair
- `batch_match_orders` — match multiple pairs
- `cancel_order` — cancel an order
- `batch_cancel_orders` — cancel multiple orders
- `match_collection_bid_order` — match collection bid
- `match_collection_bid_orders` — batch collection bids
- `set_protocol_fee_bps` — admin: update protocol fee
- `set_default_fee_receiver` — admin: update default fee receiver
- `set_fee_receiver` — admin: set token-specific fee receiver
- `set_erc20_asset_allowed` — admin: whitelist/delist SPL token
- `set_order_book` — admin: update order book pubkey
- `toggle_pause` — admin: pause/unpause
- `safe_transfer_spl` — admin: rescue stuck SPL tokens

**PDA State Accounts:**
- `ExchangeConfig` — singleton, holds all configuration
- `OrderFill` — per-order, holds fill amount
- `AllowedToken` — per-SPL-token, marks it as whitelisted
- `FeeReceiver` — per-SPL-token, optional custom fee receiver

### Program 2: `royalties_registry`

**Instructions:**
- `initialize` — set up registry config
- `set_royalties_by_collection` — set collection-level royalties
- `set_owner_royalties_by_token` — set owner royalties per collection+tokenId
- `set_creator_royalties_by_token` — set creator royalties per collection+tokenId
- `set_provider_by_collection` — set external provider for a collection

**PDA State Accounts:**
- `RegistryConfig` — singleton, holds owner/admin info
- `CollectionRoyalties` — per-collection royalty configuration
- `OwnerTokenRoyalties` — per-collection+tokenId owner royalties
- `CreatorTokenRoyalties` — per-collection+tokenId creator royalties
- `RoyaltyProvider` — per-collection external provider reference

---

## 3. Storage Mapping: EVM → Solana Accounts

### ExchangeStorage → Exchange PDA Accounts

| EVM Storage Variable | Solana Account | PDA Seeds | Account Data |
|---|---|---|---|
| `helperProxy` | N/A (merged) | — | — |
| `orderBook` | `ExchangeConfig` | `["exchange_config"]` | `order_book: Pubkey` |
| `royaltiesRegistryProxy` | `ExchangeConfig` | `["exchange_config"]` | `royalties_registry: Pubkey` |
| `defaultFeeReceiver` | `ExchangeConfig` | `["exchange_config"]` | `default_fee_receiver: Pubkey` |
| `weth` | `ExchangeConfig` | `["exchange_config"]` | `wsol_mint: Pubkey` |
| `exchangeOwner` | `ExchangeConfig` | `["exchange_config"]` | `exchange_owner: Pubkey` |
| `protocolFeeBps` | `ExchangeConfig` | `["exchange_config"]` | `protocol_fee_bps: u16` |
| `chainId` | N/A | — | Not needed (Solana has single chain; cluster ID can be stored if needed) |
| `fills[orderKeyHash]` | `OrderFill` | `["order_fill", order_key_hash]` | `fill_amount: u64` |
| `allowedERC20Assets[addr]` | `AllowedToken` | `["allowed_token", mint_pubkey]` | `is_allowed: bool` |
| `feeReceivers[token]` | `FeeReceiver` | `["fee_receiver", mint_pubkey]` | `receiver: Pubkey` |

### RoyaltiesRegistryStorage → RoyaltiesRegistry PDA Accounts

| EVM Storage Variable | Solana Account | PDA Seeds | Account Data |
|---|---|---|---|
| `ownerRoyaltiesByTokenAndTokenId[hash]` | `OwnerTokenRoyalties` | `["owner_royalties", collection_mint, token_id_bytes]` | `initialized: bool, royalties: Vec<RoyaltyPart>` |
| `creatorRoyaltiesByTokenAndTokenId[hash]` | `CreatorTokenRoyalties` | `["creator_royalties", collection_mint, token_id_bytes]` | `initialized: bool, royalties: Vec<RoyaltyPart>` |
| `royaltiesByToken[addr]` | `CollectionRoyalties` | `["collection_royalties", collection_mint]` | `initialized: bool, royalties: Vec<RoyaltyPart>` |
| `royaltiesProviders[addr]` | `RoyaltyProvider` | `["royalty_provider", collection_mint]` | `provider: Pubkey` |

---

## 4. Access Control Mapping

| EVM Role | EVM Mechanism | Solana Equivalent |
|---|---|---|
| Owner | `OwnableUpgradeable.owner()` | `ExchangeConfig.owner: Pubkey` — checked via `Signer` constraint |
| Upgrade Manager | `UpgradeManager.upgradeManager` | Program's native `upgrade_authority` — set to multisig |
| Exchange Owner | `ExchangeStorage.exchangeOwner` | `ExchangeConfig.exchange_owner: Pubkey` — checked via `Signer` constraint |
| Order Book | `ExchangeStorage.orderBook` | `ExchangeConfig.order_book: Pubkey` — used for Ed25519 signature verification |
| `requireOwner` (StorageBase) | `msg.sender == owner` | PDA authority check — program owns its PDAs by default |
| `onlyImplementation` (Proxy) | `msg.sender == implementation` | N/A — no proxy pattern |
| `onlyWETH` | `msg.sender == weth` | N/A — SOL transfers don't come from wSOL contract |
| `whenNotPaused` | `PausableUpgradeable` | `ExchangeConfig.is_paused: bool` — checked at instruction start |
| `nonReentrant` | `ReentrancyGuardUpgradeable` | Not needed — Solana's runtime prevents reentrancy by default (single-threaded execution per tx) |
| `requireOwnerOrTokenOwner` | `tx.origin == IOwnable(token).owner()` | Signer must be collection authority (metadata authority from Metaplex) |
| `requireOwnerOrTokenIdCreator` | `tx.origin == ICreator(token).creator(tokenId)` | Signer must be `update_authority` from Metaplex metadata |

---

## 5. Signature Verification Mapping

### EVM: EIP-712 + ECDSA (secp256k1)
- Orders are hashed using EIP-712 structured data.
- Signatures are 65 bytes (r, s, v) over secp256k1.
- `ecrecover` is used to recover the signer.

### Solana: Ed25519 Signature Verification

On Solana, the native signature scheme is **Ed25519** (not secp256k1). Two options:

#### Option A: Ed25519 Signatures (Recommended)
- Use Solana's `Ed25519SigVerify` precompile (sysvar instruction introspection).
- Orders are serialized using a canonical format (Borsh or custom), hashed with SHA-256, and signed with Ed25519 keys.
- The instruction introspects the Ed25519 program's instruction data to verify signatures.
- This is the idiomatic Solana approach and avoids secp256k1 overhead.

#### Option B: Secp256k1 Signatures (EVM Compatibility)
- Use Solana's `Secp256k1SigVerify` precompile.
- Preserves exact EIP-712 + secp256k1 verification from EVM.
- Useful if the same keys/signatures must work on both chains.
- Higher compute cost than Ed25519.

**Recommendation:** Use Ed25519 (Option A) for native Solana experience. If cross-chain signature compatibility is required (e.g., same off-chain order book signing for both EVM and Solana), use secp256k1 (Option B).

### Mapping EIP-712 Concepts

| EVM Concept | Solana Equivalent |
|---|---|
| EIP-712 Domain Separator | Custom domain prefix bytes (program ID + cluster) |
| EIP-712 hashStruct | Borsh-serialized order → SHA-256 hash |
| `ecrecover` | Ed25519 signature verification via sysvar introspection |
| EIP-1271 (smart contract signatures) | CPI to a verification program (if needed, rare on Solana) |
| Order Book matchAllowance signature | Same pattern: serialize `(orderKeyHash, matchBeforeTimestamp)`, sign with Ed25519 |
| `tx.origin` | Transaction signer (first signer in tx) |

---

## 6. Token Mint/Burn Mapping

The Energi GMI protocol does **not mint or burn tokens**. It is a marketplace that transfers existing tokens between users. The mapping is:

| EVM Token Standard | Solana Equivalent | Transfer Mechanism |
|---|---|---|
| ETH (native) | SOL (native) | `system_program::transfer` |
| WETH (ERC-20 wrapper) | wSOL (wrapped SOL) | SPL Token `transfer` (or sync native) |
| ERC-20 (whitelisted tokens) | SPL Token | SPL Token `transfer` / `transfer_checked` |
| ERC-721 (NFTs) | Metaplex NFT (SPL Token with supply=1) | SPL Token `transfer` (amount=1) |
| ERC-1155 (semi-fungible) | SPL Token (multiple editions or Metaplex SFTs) | SPL Token `transfer` |

### Approval Model Change
- **EVM**: Users approve the Exchange proxy to spend their tokens (`approve` / `setApprovalForAll`).
- **Solana**: Users **delegate** to the Exchange program's PDA, OR the Exchange PDA is granted authority, OR the user signs the transaction directly (most common).

**Recommended approach**: Require the user to be a signer of the `match_orders` instruction. The instruction handler transfers tokens on behalf of both maker and taker using signed transfer instructions. For the off-chain order case (maker is not present), use **delegate/approve** pattern via SPL Token's `approve` instruction, where the exchange PDA is the delegate.

---

## 7. Nonce Handling

### EVM: `salt` Field
- `salt > 0`: order registered in off-chain Order Book, requires signatures and matchAllowance.
- `salt == 0`: order submitted directly by maker (no signature needed if caller is maker).
- `salt` is part of the order key hash, making each order unique.

### Solana Equivalent
- The `salt` concept is preserved as-is in the Solana order struct.
- `salt > 0` orders require Ed25519 signature verification.
- `salt == 0` orders require the maker to be a transaction signer.
- The `OrderFill` PDA is derived from the order key hash (which includes the salt), ensuring uniqueness.
- Order cancellation sets fill to `u64::MAX`.

---

## 8. Event → Account State / Log Translation

### EVM Events → Solana Approach

Solana does not have Ethereum-style events. Two mechanisms replace them:

#### A. Anchor Events (Program Logs)
```rust
#[event]
pub struct MatchEvent {
    pub left_hash: [u8; 32],
    pub right_hash: [u8; 32],
    pub left_maker: Pubkey,
    pub right_maker: Pubkey,
    pub new_left_fill: u64,
    pub new_right_fill: u64,
}

#[event]
pub struct CancelOrderEvent {
    pub order_hash: [u8; 32],
}

#[event]
pub struct TransferEvent {
    pub asset_class: u8,
    pub from: Pubkey,
    pub to: Pubkey,
    pub mint: Pubkey,
    pub token_id: u64,
    pub value: u64,
    pub transfer_direction: u8,
    pub transfer_type: u8,
}
```

These are emitted via `emit!()` and indexed by off-chain services using `getProgramAccounts` or log subscription.

#### B. Account State
For durable state queries (e.g., "what is the fill of order X?"), the `OrderFill` PDA account serves as the permanent record.

---

## 9. Emulating Contract Modifiers in Anchor

| EVM Modifier | Anchor Equivalent |
|---|---|
| `whenNotPaused` | `require!(!config.is_paused, ExchangeError::Paused)` at instruction start |
| `nonReentrant` | Not needed (Solana runtime prevents reentrancy) |
| `onlyOwner` | `has_one = owner @ ExchangeError::Unauthorized` in account constraint, or `require!(ctx.accounts.authority.key() == config.owner)` |
| `onlyUpgradeManager` | Program upgrade authority (native Solana mechanism) |
| `requireOwner` (StorageBase) | PDA ownership (program owns its PDAs implicitly) |
| `requireExchangeOwner` | `require!(signer.key() == config.exchange_owner)` |
| `onlyWETH` | N/A |
| `senderOrigin` | N/A (Solana signatures are always from key holders) |
| `onlyImplementation` | N/A (no proxy pattern) |

---

## 10. Preserving Protocol Invariants

| Invariant | EVM Implementation | Solana Implementation |
|---|---|---|
| Fills monotonically increase | `fills[hash] += newFill` (only increases) | `order_fill.fill_amount = order_fill.fill_amount.checked_add(new_fill)?` — validate previous fill < new fill |
| Cancel sets fill to MAX | `fills[hash] = UINT256_MAX` | `order_fill.fill_amount = u64::MAX` |
| Payouts sum to 100% | `require(sumBps == 10000)` | `require!(sum_bps == 10000, ExchangeError::InvalidPayoutSum)` |
| Royalties capped at 50% | `require(totalRoyaltiesBps <= 5000)` | `require!(total_royalties_bps <= 5000, ExchangeError::RoyaltiesTooHigh)` |
| Only fungible ↔ non-fungible | `LibOrder.validate` checks | Equivalent validation in `validate_order()` function |
| Maker cannot pay with SOL | `require(rightOrder.makeAsset != ETH)` | `require!(right_order.make_asset.asset_class != AssetClass::Sol)` |
| ERC-20 whitelist | `allowedERC20Assets[addr]` check | `AllowedToken` PDA must exist |
| Order Book auth for salt > 0 | `matchAllowance` signature check | Ed25519 signature verification |
| Signature bound to chain+contract | EIP-712 domain with chainId + proxy | Domain prefix with program_id + cluster |
| Collection bids via helper only | `msg.sender == helperProxy` check | Part of the same program — validated within instruction logic |

---

## 11. Account Model Design

### ExchangeConfig (Singleton PDA)
```
Seeds: ["exchange_config"]
Fields:
  - owner: Pubkey
  - exchange_owner: Pubkey
  - order_book: Pubkey
  - default_fee_receiver: Pubkey
  - royalties_registry_program: Pubkey
  - wsol_mint: Pubkey
  - protocol_fee_bps: u16
  - is_paused: bool
  - bump: u8
```

### OrderFill (Per-Order PDA)
```
Seeds: ["order_fill", order_key_hash (32 bytes)]
Fields:
  - fill_amount: u64
  - bump: u8
```

### AllowedToken (Per-Token PDA)
```
Seeds: ["allowed_token", mint_pubkey]
Fields:
  - is_allowed: bool
  - bump: u8
```

### FeeReceiver (Per-Token PDA)
```
Seeds: ["fee_receiver", mint_pubkey]
Fields:
  - receiver: Pubkey
  - bump: u8
```

### RegistryConfig (Singleton PDA — royalties_registry program)
```
Seeds: ["registry_config"]
Fields:
  - owner: Pubkey
  - bump: u8
```

### CollectionRoyalties (Per-Collection PDA)
```
Seeds: ["collection_royalties", collection_mint]
Fields:
  - initialized: bool
  - royalties: Vec<RoyaltyPart>  // max ~10 entries
  - bump: u8
```

### OwnerTokenRoyalties (Per-Token PDA)
```
Seeds: ["owner_royalties", collection_mint, token_id_bytes]
Fields:
  - initialized: bool
  - royalties: Vec<RoyaltyPart>
  - bump: u8
```

### CreatorTokenRoyalties (Per-Token PDA)
```
Seeds: ["creator_royalties", collection_mint, token_id_bytes]
Fields:
  - initialized: bool
  - royalties: Vec<RoyaltyPart>
  - bump: u8
```

### RoyaltyProvider (Per-Collection PDA)
```
Seeds: ["royalty_provider", collection_mint]
Fields:
  - provider_program: Pubkey
  - bump: u8
```

### Shared Structs
```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RoyaltyPart {
    pub account: Pubkey,
    pub value: u16,  // basis points
}
```
