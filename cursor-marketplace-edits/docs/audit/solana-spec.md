# Phase 2 — Solana Implementation Analysis

This document maps the Solana implementation of the GMI NFT Marketplace Exchange, identifying how each EVM component is represented.

---

## 1. Architecture Mapping

### 1.1 Program Replacements

| EVM Contract | Solana Program | Notes |
|-------------|----------------|-------|
| Exchange + ExchangeStorage | `exchange` program | Single Anchor program with PDA-based state |
| ExchangeHelper | Merged into `exchange` | Logic in `logic/*.rs`, no separate program |
| RoyaltiesRegistry + Storage | `royalties_registry` program | Separate program with PDA-based state |
| ExchangeProxy (ERC-1967) | N/A | No upgradeability pattern (no proxy) |
| StorageBase | N/A | Replaced by Anchor account ownership |

### 1.2 PDA Design

| PDA | Seeds | Account Type | Replaces |
|-----|-------|-------------|----------|
| `exchange_config` | `[b"exchange_config"]` | `ExchangeConfig` | ExchangeStorage singleton |
| `exchange_authority` | `[b"exchange_authority"]` | Unchecked (signer PDA) | Proxy holding funds |
| `order_fill` | `[b"order_fill", order_key_hash]` | `OrderFill` | `fills` mapping |
| `allowed_token` | `[b"allowed_token", mint]` | `AllowedToken` | `allowedERC20Assets` mapping |
| `fee_receiver` | `[b"fee_receiver", mint]` | `FeeReceiver` | `feeReceivers` mapping |
| `registry_config` | `[b"registry_config"]` | `RegistryConfig` | RoyaltiesRegistryStorage |
| `collection_royalties` | `[b"collection_royalties", mint]` | `CollectionRoyalties` | `royaltiesByToken` |
| `owner_royalties` | `[b"owner_royalties", mint, token_id]` | `OwnerTokenRoyalties` | `ownerRoyaltiesByTokenAndTokenId` |
| `creator_royalties` | `[b"creator_royalties", mint, token_id]` | `CreatorTokenRoyalties` | `creatorRoyaltiesByTokenAndTokenId` |
| `royalty_provider` | `[b"royalty_provider", mint]` | `RoyaltyProvider` | `royaltiesProviders` |

### 1.3 Instruction Structure

| Instruction | Handler | Replaces |
|------------|---------|----------|
| `initialize` | `handler_initialize` | Exchange.initialize |
| `match_orders` | `handler_match_orders` | Exchange.matchOrders |
| `cancel_order` | `handler_cancel_order` | Exchange.cancelOrder |
| `set_protocol_fee_bps` | `handler_set_protocol_fee_bps` | ExchangeStorage.setProtocolFeeBps |
| `set_default_fee_receiver` | `handler_set_default_fee_receiver` | ExchangeStorage.setDefaultFeeReceiver |
| `set_fee_receiver` | `handler_set_fee_receiver` | ExchangeStorage.setFeeReceiver |
| `set_allowed_token` | `handler_set_allowed_token` | ExchangeStorage.setERC20AssetAllowed |
| `set_order_book` | `handler_set_order_book` | ExchangeStorage.setOrderBook |
| `set_exchange_owner` | `handler_set_exchange_owner` | ExchangeStorage.setExchangeOwner |
| `toggle_pause` | `handler_toggle_pause` | Exchange.togglePause |
| `set_royalties_registry_program` | `handler_set_royalties_registry_program` | N/A (new) |
| `safe_transfer_spl` | `handler_safe_transfer_spl` | Exchange.safeTransferERC20 |

---

## 2. Order Representation

### 2.1 Order Struct (`state/types.rs`)

```rust
pub struct Order {
    pub maker: Pubkey,          // 32 bytes (vs 20 bytes address)
    pub make_asset: Asset,
    pub taker: Pubkey,          // Pubkey::default() = any
    pub take_asset: Asset,
    pub salt: u64,              // u64 vs uint256
    pub start: i64,             // i64 vs uint256
    pub end: i64,
    pub data_type: [u8; 4],
    pub data: Vec<u8>,
    pub collection_bid: bool,
}
```

### 2.2 Asset Types

```rust
pub enum AssetClass {
    Sol,            // → ETH_ASSET_CLASS
    WrappedSol,     // → WETH_ASSET_CLASS
    SplToken,       // → ERC20_ASSET_CLASS
    Nft,            // → ERC721_ASSET_CLASS
    SemiFungible,   // → ERC1155_ASSET_CLASS
}

pub struct AssetType {
    pub asset_class: AssetClass,
    pub mint: Pubkey,           // Token mint address
    pub token_id: u64,          // For SemiFungible; 0 for NFTs
}
```

### 2.3 Data Types

- `DATA_TYPE_V1`: `[0xa0, 0x83, 0x2e, 0xf7]` — matches EVM `keccak256("V1")[0:4]`
- `DATA_TYPE_EMPTY`: `[0xff, 0xff, 0xff, 0xff]` — matches EVM sentinel

---

## 3. Signature System

### 3.1 Ed25519 Instead of ECDSA

The Solana implementation uses Ed25519 signature verification via the Ed25519 native program, introspected through the instructions sysvar.

### 3.2 Domain Separation

```
Order Hash = SHA256(
    program_id || "energi" || 0x01 ||
    maker || hash(make_asset_type) || make_value ||
    taker || hash(take_asset_type) || take_value ||
    salt || start || end || data_type || SHA256(data) || collection_bid
)

Match Allowance Hash = SHA256(
    program_id || "energi" || 0x01 ||
    order_key_hash || match_before_timestamp
)
```

Uses `program_id` as domain binding (equivalent to `verifyingContract` in EIP-712).

### 3.3 Verification Flow

```
verify_order_signatures:
  sig_ix_index = 0
  
  if left_order.salt > 0:
    verify matchAllowance (orderbook signs) at sig_ix_index++
    if payer != left_order.maker:
      verify order hash (maker signs) at sig_ix_index++
  else:
    if left_order.maker != default:
      REQUIRE payer == left_order.maker
  
  if right_order.salt > 0:
    verify matchAllowance (orderbook signs) at sig_ix_index++
    if payer != right_order.maker:
      verify order hash (maker signs) at sig_ix_index++
  else:
    if right_order.maker != default:
      REQUIRE payer == right_order.maker
```

### 3.4 Ed25519 Instruction Introspection

The program reads Ed25519 program instructions from the instructions sysvar. It verifies:
1. Instruction is from Ed25519 program (`ed25519_program::ID`)
2. Public key matches expected signer
3. Message matches expected hash

---

## 4. Fill Tracking

### 4.1 OrderFill PDA

```rust
pub struct OrderFill {
    pub fill_amount: u64,    // u64 vs uint256
    pub bump: u8,
}
```

Derived with seeds `[b"order_fill", order_key_hash]`.

### 4.2 Monotonicity

- Fill can only increase: `fill_amount = fill_amount + new_take_value` (checked add)
- Cancel sets `fill_amount = u64::MAX`
- Zero salt orders are not tracked (fill = 0 always)

### 4.3 init_if_needed

Order fill accounts are created with `init_if_needed`, so they start at 0 if not previously created. This matches the EVM behavior where uninitialized mapping values default to 0.

---

## 5. Asset Handling

### 5.1 NFT Transfers

Uses `anchor_spl::token::transfer` with the exchange authority PDA as delegate. The PDA must be pre-approved as delegate on the source token account.

### 5.2 SOL Transfers

Uses `system_instruction::transfer` via `invoke`. Requires the source account to be a signer.

### 5.3 SPL Token Transfers

Uses `anchor_spl::token::transfer` with PDA signer seeds.

### 5.4 Wrapped SOL Handling

The implementation has a `WrappedSol` asset class and stores `wsol_mint` in config, but **does not implement automatic wrapping/unwrapping** like the EVM implementation does with ETH/WETH.

---

## 6. Fee & Royalty Logic

### 6.1 Protocol Fees

- `protocol_fee_bps` stored in `ExchangeConfig`
- Fee calculated and deducted in `do_transfers_with_fees`
- Fee destination taken from `remaining_accounts[1]`

### 6.2 Royalties

- **Royalties are client-supplied** via `royalty_parts` in `MatchOrdersArgs`
- **NOT verified against the royalties registry PDA** (TODO comment in code)
- Cap enforced: `total_royalties_bps <= 5000`

### 6.3 Origin Fees

- Parsed from order data
- Both orders' origin fees deducted from `rest`
- Added to `total_amount` for the fee payer side

### 6.4 Payouts

- Parsed from order data
- Must sum to exactly 10000 bps
- Last payout receives remainder (dust prevention)

### 6.5 Transfer Pipeline

Same structure as EVM:
1. Protocol fee
2. Royalties (from client-supplied parts)
3. Origin fees (fee payer order)
4. Origin fees (other order)
5. Payouts (other order's recipients)

---

## 7. Existing Security Checks

### 7.1 Account Validation

- `exchange_config` validated by PDA seeds + bump
- `order_fill` validated by PDA seeds (includes order_key_hash)
- `exchange_authority` validated by PDA seeds
- `instructions_sysvar` validated by address constraint

### 7.2 Signature Checks

- Ed25519 introspection for maker signatures and matchAllowance
- Zero salt: payer must equal maker
- Non-zero salt: full signature verification chain

### 7.3 Pause Mechanism

- `is_paused` flag in `ExchangeConfig`
- Checked at start of `match_orders` and `cancel_order`

### 7.4 Authorization

- Admin functions use `exchange_owner` constraint
- Some admin functions use `owner` constraint (upgrade-level)
- `cancel_order` requires maker to be signer

### 7.5 Arithmetic Safety

- `checked_add` for fill updates
- u128 intermediates for bps and partial amount calculations
- 0.1% rounding error threshold (matching EVM)

---

## 8. Missing Features (vs EVM)

### 8.1 No Batch Match Orders

The EVM `batchMatchOrders` is not implemented.

### 8.2 No Collection Bid Instruction

Despite `collection_bid` flag and validation functions existing, there is no `match_collection_bid_orders` instruction. The `match_orders` handler rejects any order with `collection_bid == true`.

### 8.3 No Token Whitelist Enforcement

`AllowedToken` PDA exists and can be set, but `match_orders` does not check it.

### 8.4 No Royalties Registry Verification

Royalties are client-supplied and unverified. The TODO comment acknowledges this gap.

### 8.5 No ETH/WETH Auto-Conversion

No wrapped SOL wrapping/unwrapping logic equivalent to `processEthAndWeth`.

### 8.6 No Fee Receiver PDA Lookup

The fee receiver destination comes from `remaining_accounts` without verification against the `FeeReceiver` PDA or `default_fee_receiver` config.

### 8.7 No Reentrancy Guard

No explicit reentrancy protection (Solana's runtime provides some protection via account locking, but CPI can still cause issues).

### 8.8 No ERC-2981 Equivalent

No on-chain royalty standard fallback like ERC-2981.
