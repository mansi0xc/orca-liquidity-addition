# 10 — Implementation Progress

---

## Program Architecture

The Solana implementation consists of two Anchor programs:

| Program | Program ID | Source |
|---------|-----------|--------|
| `exchange` | `CuUtAt1GpoDoDGnrQbjeZTH7qSPdVwahybi4KoucQ6zN` | `programs/exchange/` |
| `royalties_registry` | `CdVAwwosJmSWZD4Yg4j6JW9Xm6YLYFLCytjkqBZwF8AQ` | `programs/royalties-registry/` |

---

## Exchange Program — Completed Instructions

### 1. `initialize` ✅

- Creates `ExchangeConfig` PDA with all protocol settings
- Seeds: `["exchange_config"]`
- Validates `protocol_fee_bps <= 10000`

### 2. `match_orders` ✅

- Full order matching with signature verification, fill calculation, and transfer pipeline
- Ed25519 signature verification via sysvar instruction introspection
- Match allowance verification (order book signature + timestamp)
- Asset type matching (SOL/wSOL interop, same-class verification)
- Fill calculation with overflow protection
- Transfer pipeline: protocol fee → royalties → origin fees → payouts
- `exchange_authority` PDA as delegate for SPL token transfers
- Supports both SOL (system_program) and SPL (token_program) transfers
- `remaining_accounts` layout documented in `logic/transfers.rs`

### 3. `cancel_order` ✅

- Sets `OrderFill.fill_amount = u64::MAX` for permanent cancellation
- Maker must be a signer and match `order.maker`
- Salt must be non-zero
- Order key hash verified against computed hash

### 4. `set_protocol_fee_bps` ✅

- Updates protocol fee, capped at 10000 bps
- Requires `exchange_owner` signer

### 5. `set_default_fee_receiver` ✅

- Updates fallback fee receiver address
- Requires `exchange_owner` signer

### 6. `set_fee_receiver` ✅

- Sets per-token fee receiver via PDA: `["fee_receiver", mint]`
- `init_if_needed` for new token registrations

### 7. `set_allowed_token` ✅

- Whitelist/delist SPL tokens via PDA: `["allowed_token", mint]`
- `init_if_needed` for new tokens

### 8. `set_order_book` ✅

- Updates the order book public key (for match allowance signatures)
- Requires `owner` signer (protocol deployer)

### 9. `set_exchange_owner` ✅

- Transfers `exchange_owner` role
- Requires current `exchange_owner` signer

### 10. `toggle_pause` ✅

- Toggles `is_paused` flag
- Requires `owner` signer

### 11. `set_royalties_registry_program` ✅

- Updates the royalties registry program ID reference
- Requires `owner` signer

### 12. `safe_transfer_spl` ✅

- Emergency SPL token rescue from exchange-owned accounts
- Uses `exchange_authority` PDA as signer for CPI
- Requires `owner` signer

---

## Royalties Registry Program — Completed Instructions

### 1. `initialize` ✅

- Creates `RegistryConfig` PDA: `["registry_config"]`

### 2. `set_royalties_by_collection` ✅

- PDA: `["collection_royalties", collection_mint]`
- Validates royalty sum ≤ 10000 bps, no zero-address recipients
- Owner or collection authority

### 3. `set_owner_royalties_by_token` ✅

- PDA: `["owner_token_royalties", mint, token_id_bytes]`
- Token-level owner royalties

### 4. `set_creator_royalties_by_token` ✅

- PDA: `["creator_token_royalties", mint, token_id_bytes]`
- Token-level creator royalties

### 5. `set_provider_by_collection` ✅

- PDA: `["royalty_provider", collection_mint]`
- Sets external royalty provider program for a collection

### 6. `transfer_registry_ownership` ✅

- Transfers the `owner` role on the registry

---

## Exchange Program — Logic Modules

| Module | File | Status | Description |
|--------|------|--------|-------------|
| `bps` | `logic/bps.rs` | ✅ | Basis point calculation with u128 intermediates |
| `math` | `logic/math.rs` | ✅ | `safe_get_partial_amount_floor` with rounding check |
| `order` | `logic/order.rs` | ✅ | Order key hash, full hash, match allowance hash |
| `fill` | `logic/fill.rs` | ✅ | Fill computation with cancellation detection |
| `fee_side` | `logic/fee_side.rs` | ✅ | Fee side determination from asset classes |
| `order_data` | `logic/order_data.rs` | ✅ | DataV1 deserialization from order bytes |
| `exchange` | `logic/exchange.rs` | ✅ | Asset matching, counterparty check, fee calculation |
| `signature` | `logic/signature.rs` | ✅ | Ed25519 sysvar introspection verification |
| `asset` | `logic/asset.rs` | ✅ | Asset type hashing |
| `transfers` | `logic/transfers.rs` | ✅ | Full transfer pipeline (fees, royalties, payouts) |

---

## State Accounts

### Exchange Program

| Account | Seeds | Size | Status |
|---------|-------|------|--------|
| `ExchangeConfig` | `["exchange_config"]` | Fixed | ✅ |
| `OrderFill` | `["order_fill", order_key_hash]` | Fixed | ✅ |
| `AllowedToken` | `["allowed_token", mint]` | Fixed | ✅ |
| `FeeReceiver` | `["fee_receiver", mint]` | Fixed | ✅ |

### Royalties Registry Program

| Account | Seeds | Size | Status |
|---------|-------|------|--------|
| `RegistryConfig` | `["registry_config"]` | Fixed | ✅ |
| `CollectionRoyalties` | `["collection_royalties", collection_mint]` | Dynamic | ✅ |
| `OwnerTokenRoyalties` | `["owner_token_royalties", mint, token_id_bytes]` | Dynamic | ✅ |
| `CreatorTokenRoyalties` | `["creator_token_royalties", mint, token_id_bytes]` | Dynamic | ✅ |
| `RoyaltyProvider` | `["royalty_provider", collection_mint]` | Fixed | ✅ |

---

## Security Checks Applied

| Rule | Description | Applied In |
|------|-------------|------------|
| PC-1 | Pause check on all state-changing instructions | `match_orders`, `cancel_order` |
| SG-1 | Maker must be signer (cancel) or Ed25519 verified (match) | `cancel_order`, `match_orders` |
| P-1 | All PDAs use `seeds` + `bump` constraints | All account structs |
| FM-1 | Fill set to `u64::MAX` on cancellation | `cancel_order` |
| FM-3 | Verify `new_fill > 0` | `match_orders` |
| FM-4 | Fill update uses `checked_add` | `match_orders` |
| S-1 | Order signature verified via Ed25519 sysvar | `match_orders` |
| S-2 | Match allowance timestamp verified | `match_orders` |
| S-5 | Salt == 0 requires maker to be tx signer | `match_orders` |
| IR-1 | u128 intermediates for bps calculations | `logic/bps.rs` |
| L-1 | Checked arithmetic for all fee calculations | `logic/exchange.rs`, `logic/transfers.rs` |
| RA-1 | Deterministic remaining_accounts layout | `logic/transfers.rs` |

---

## Remaining Work

### Not Yet Implemented

| Instruction | Priority | Notes |
|-------------|----------|-------|
| `batch_cancel_orders` | Medium | Loop over multiple orders; limited by tx size |
| `match_collection_bid_order` | High | Collection-wide bid matching; complex account layout |
| AllowedToken whitelist check in `match_orders` | High | Not yet enforced during matching |
| wSOL wrap/unwrap handling | Medium | `sync_native` / close ATA for SOL ↔ wSOL |

### Design Decisions (Deviations from Doc)

| Decision | Reason |
|----------|--------|
| `batch_match_orders` not implemented as separate instruction | Solana allows multiple `match_orders` calls in a single tx; separate batch instruction adds unnecessary complexity |
| Order key hashes passed as instruction args | Anchor's IDL builder cannot evaluate function calls in PDA seed constraints; client computes and passes hashes, program verifies |
| Royalty parts passed in instruction data | Avoids CPI to royalties registry during match; client reads registry off-chain and provides parts. **On-chain verification against registry PDA still needed** |
| `get_royalties` not a CPI endpoint | Exchange program reads royalty PDAs directly from remaining_accounts; more compute-efficient |
| `exchange_authority` PDA as token delegate | Mirrors EVM's approve/transferFrom pattern; both parties set exchange_authority as delegate on their token accounts before trading |

### Security TODOs

| Item | Priority | Description |
|------|----------|-------------|
| Royalty verification | **Critical** | Verify client-provided `royalty_parts` match on-chain registry PDA |
| Token account mint validation | High | Verify remaining_accounts token account mints match expected mints (RULE RA-2) |
| Token account owner validation | High | Verify token account owners match intended parties (RULE RA-3) |
| remaining_accounts length check | Medium | Verify exact count matches expected from order data (RULE RA-4) |
| AllowedToken enforcement | High | Check payment token is whitelisted before matching |
| Metaplex metadata integration | Medium | Verify collection authority / creator in royalties-registry instructions |

---

## Build Status

- **Anchor version:** 0.31.1
- **Solana toolchain:** Platform SBF
- **Build status:** ✅ Compiles successfully (`anchor build`)
- **Warnings:** Framework-level only (cfg conditions, deprecated `realloc`); no application-level warnings
- **Tests:** Not yet written

---

## File Structure

```
solana-contracts/
├── Anchor.toml
├── Cargo.toml
├── docs/
│   ├── 01_protocol_overview.md
│   ├── ...
│   ├── 09_migration_traps.md
│   └── 10_implementation_progress.md
└── programs/
    ├── exchange/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── errors.rs
    │       ├── events.rs
    │       ├── instructions/
    │       │   ├── mod.rs
    │       │   ├── initialize.rs
    │       │   ├── admin.rs
    │       │   ├── cancel_order.rs
    │       │   └── match_orders.rs
    │       ├── logic/
    │       │   ├── mod.rs
    │       │   ├── bps.rs
    │       │   ├── math.rs
    │       │   ├── order.rs
    │       │   ├── fill.rs
    │       │   ├── fee_side.rs
    │       │   ├── order_data.rs
    │       │   ├── exchange.rs
    │       │   ├── signature.rs
    │       │   ├── asset.rs
    │       │   └── transfers.rs
    │       └── state/
    │           ├── mod.rs
    │           ├── config.rs
    │           ├── order_fill.rs
    │           ├── allowed_token.rs
    │           ├── fee_receiver.rs
    │           └── types.rs
    └── royalties-registry/
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── errors.rs
            ├── events.rs
            ├── instructions/
            │   ├── mod.rs
            │   ├── initialize.rs
            │   ├── set_royalties.rs
            │   └── admin.rs
            └── state/
                ├── mod.rs
                ├── config.rs
                ├── collection_royalties.rs
                ├── token_royalties.rs
                ├── provider.rs
                └── types.rs
```
