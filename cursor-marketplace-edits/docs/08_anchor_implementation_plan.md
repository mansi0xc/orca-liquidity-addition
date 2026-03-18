# 08 — Anchor Implementation Plan

---

## Overview

This roadmap describes the step-by-step implementation of the Energi GMI NFT Marketplace on Solana using the Anchor framework. The implementation is divided into 7 phases, each building on the previous one.

**Estimated Total Duration:** 12-16 weeks (with a team of 2-3 Solana developers)

---

## Phase 1: Program Architecture & Scaffolding (Week 1-2)

### Objectives
- Set up the Anchor workspace
- Define all data structures (accounts, instructions, errors)
- Establish the project structure

### Tasks

#### 1.1 Initialize Anchor Workspace
```
anchor init energi-exchange
cd energi-exchange
```
Create two programs in the workspace:
- `programs/exchange/` — main exchange program
- `programs/royalties-registry/` — royalties registry program

#### 1.2 Define Shared Types (Rust Module)
Create a shared types crate or module used by both programs:

```
programs/
├── exchange/
│   └── src/
│       ├── lib.rs              # Program entry point
│       ├── instructions/       # Instruction handlers
│       │   ├── mod.rs
│       │   ├── initialize.rs
│       │   ├── match_orders.rs
│       │   ├── cancel_order.rs
│       │   ├── collection_bid.rs
│       │   └── admin.rs
│       ├── state/              # Account structs
│       │   ├── mod.rs
│       │   ├── config.rs
│       │   ├── order_fill.rs
│       │   ├── allowed_token.rs
│       │   └── fee_receiver.rs
│       ├── logic/              # Business logic (ported from Solidity libraries)
│       │   ├── mod.rs
│       │   ├── order.rs        # LibOrder equivalent
│       │   ├── fill.rs         # LibFill equivalent
│       │   ├── exchange.rs     # LibExchange equivalent
│       │   ├── math.rs         # LibMath equivalent
│       │   ├── bps.rs          # LibBps equivalent
│       │   ├── fee_side.rs     # LibFeeSide equivalent
│       │   ├── order_data.rs   # LibOrderData equivalent
│       │   ├── signature.rs    # Signature verification
│       │   └── asset.rs        # LibAsset equivalent
│       ├── errors.rs           # Custom errors
│       └── events.rs           # Anchor events
├── royalties-registry/
│   └── src/
│       ├── lib.rs
│       ├── instructions/
│       │   ├── mod.rs
│       │   ├── initialize.rs
│       │   ├── set_royalties.rs
│       │   └── admin.rs
│       ├── state/
│       │   ├── mod.rs
│       │   ├── config.rs
│       │   ├── collection_royalties.rs
│       │   ├── token_royalties.rs
│       │   └── provider.rs
│       ├── errors.rs
│       └── events.rs
```

#### 1.3 Define Error Codes
```rust
// programs/exchange/src/errors.rs
#[error_code]
pub enum ExchangeError {
    #[msg("Exchange is paused")]
    Paused,
    #[msg("Maker cannot pay with native SOL")]
    MakerCannotPayWithSol,
    #[msg("Token not allowed for trading")]
    TokenNotAllowed,
    #[msg("Order has expired")]
    OrderExpired,
    #[msg("Order has not started yet")]
    OrderNotStarted,
    #[msg("Asset classes are incompatible")]
    AssetClassMismatch,
    #[msg("Invalid signature")]
    InvalidSignature,
    #[msg("Match allowance has expired")]
    MatchAllowanceExpired,
    #[msg("Invalid order book signature")]
    InvalidOrderBookSignature,
    #[msg("Order has been cancelled")]
    OrderCancelled,
    #[msg("Nothing to fill")]
    NothingToFill,
    #[msg("Royalties exceed 50% cap")]
    RoyaltiesTooHigh,
    #[msg("Payout sum does not equal 10000 bps")]
    InvalidPayoutSum,
    #[msg("Not the order maker")]
    NotOrderMaker,
    #[msg("Zero salt orders cannot be cancelled")]
    ZeroSaltCannotCancel,
    #[msg("Counterparty mismatch")]
    CounterpartyMismatch,
    #[msg("Assets do not match")]
    AssetsDoNotMatch,
    #[msg("Fill overflow")]
    FillOverflow,
    #[msg("Rounding error exceeds threshold")]
    RoundingError,
    #[msg("Division by zero")]
    DivisionByZero,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid protocol fee")]
    InvalidProtocolFee,
    #[msg("Collection bid must use wSOL or SPL token")]
    InvalidCollectionBidAsset,
    #[msg("Invalid collection bid taker order")]
    InvalidCollectionBidTaker,
    #[msg("Fill unable to complete")]
    FillUnableToComplete,
    #[msg("Cannot transfer to zero address")]
    ZeroAddressTransfer,
    #[msg("Transfer amount cannot be zero")]
    ZeroAmountTransfer,
    #[msg("Unknown asset class")]
    UnknownAssetClass,
}
```

#### 1.4 Define Event Structs
```rust
// programs/exchange/src/events.rs
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
    pub order_key_hash: [u8; 32],
    pub maker: Pubkey,
}

#[event]
pub struct TransferEvent {
    pub asset_class: u8,
    pub from: Pubkey,
    pub to: Pubkey,
    pub mint: Pubkey,
    pub value: u64,
    pub transfer_direction: u8,
    pub transfer_type: u8,
}
```

### Deliverables
- [ ] Anchor workspace initialized with two programs
- [ ] All Rust structs defined (Order, Asset, AssetType, AssetClass, etc.)
- [ ] All account structs defined (ExchangeConfig, OrderFill, AllowedToken, etc.)
- [ ] Error codes defined
- [ ] Event structs defined
- [ ] Project compiles without errors

---

## Phase 2: State Accounts & Admin Instructions (Week 2-3)

### Objectives
- Implement all PDA state accounts
- Implement initialization and admin instructions
- Test account creation and configuration

### Tasks

#### 2.1 Implement ExchangeConfig Account
```rust
#[account]
pub struct ExchangeConfig {
    pub owner: Pubkey,
    pub exchange_owner: Pubkey,
    pub order_book: Pubkey,
    pub default_fee_receiver: Pubkey,
    pub royalties_registry_program: Pubkey,
    pub wsol_mint: Pubkey,
    pub protocol_fee_bps: u16,
    pub is_paused: bool,
    pub bump: u8,
}
```

#### 2.2 Implement Exchange Initialization
```rust
pub fn initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
    let config = &mut ctx.accounts.exchange_config;
    config.owner = ctx.accounts.authority.key();
    config.exchange_owner = args.exchange_owner;
    config.order_book = args.order_book;
    config.default_fee_receiver = args.default_fee_receiver;
    config.royalties_registry_program = args.royalties_registry_program;
    config.wsol_mint = args.wsol_mint;
    config.protocol_fee_bps = args.protocol_fee_bps;
    config.is_paused = false;
    config.bump = ctx.bumps.exchange_config;
    Ok(())
}
```

#### 2.3 Implement Admin Instructions
- `set_protocol_fee_bps`
- `set_default_fee_receiver`
- `set_fee_receiver`
- `set_allowed_token`
- `set_order_book`
- `toggle_pause`
- `safe_transfer_spl`

#### 2.4 Implement RoyaltiesRegistry Initialization and Admin
- `initialize` (registry)
- `set_royalties_by_collection`
- `set_owner_royalties_by_token`
- `set_creator_royalties_by_token`
- `set_provider_by_collection`

### Deliverables
- [ ] All PDA accounts creatable and storable
- [ ] All admin instructions implemented and tested
- [ ] Unit tests for initialization
- [ ] Unit tests for each admin instruction
- [ ] Access control tests (unauthorized callers are rejected)

---

## Phase 3: Core Instruction Handlers (Week 3-6)

### Objectives
- Implement the core order matching logic
- Port all Solidity library functions to Rust
- Implement `cancel_order`

### Tasks

#### 3.1 Port Library Functions to Rust

**Priority order (each builds on the previous):**

1. **`bps.rs`** (LibBps) — trivial:
   ```rust
   pub fn bps(value: u64, bps_value: u16) -> Result<u64> {
       Ok((value as u128 * bps_value as u128 / 10000) as u64)
   }
   ```

2. **`math.rs`** (LibMath) — safe partial amount floor/ceil with rounding error checks

3. **`asset.rs`** (LibAsset) — hashing functions for assets (SHA-256 instead of keccak256)

4. **`order.rs`** (LibOrder) — order hashing, key computation, validation, `calculateRemaining`

5. **`fill.rs`** (LibFill) — `fillOrder`, `fillLeft`, `fillRight`, `fillCollectionBidOrder`

6. **`fee_side.rs`** (LibFeeSide) — `getFeeSide` fee payer determination

7. **`order_data.rs`** (LibOrderData) — parse order data into payouts and origin fees

8. **`exchange.rs`** (LibExchange) — asset matching, signature verification helpers, fee calculation, counterparty checks

9. **`signature.rs`** — Ed25519 signature verification via sysvar introspection

#### 3.2 Implement Order Hashing

```rust
// Equivalent to LibOrder.hashKey
pub fn compute_order_key_hash(order: &Order) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(order.maker.to_bytes());
    hasher.update(hash_asset_type(&order.make_asset.asset_type));
    hasher.update(hash_asset_type(&order.take_asset.asset_type));
    hasher.update(order.salt.to_le_bytes());
    hasher.update([order.collection_bid as u8]);
    hasher.finalize().into()
}
```

#### 3.3 Implement Signature Verification

```rust
pub fn verify_ed25519_signature(
    instructions_sysvar: &AccountInfo,
    expected_signer: &Pubkey,
    expected_message: &[u8],
    sig_instruction_index: u8,
) -> Result<()> {
    let ix = load_instruction_at_checked(
        sig_instruction_index as usize,
        instructions_sysvar,
    )?;
    require!(ix.program_id == ed25519_program::ID, ExchangeError::InvalidSignature);
    // Parse Ed25519 instruction data
    // Verify public key matches expected_signer
    // Verify message matches expected_message
    Ok(())
}
```

#### 3.4 Implement `cancel_order` Instruction

```rust
pub fn cancel_order(ctx: Context<CancelOrder>, order: Order) -> Result<()> {
    require!(!ctx.accounts.exchange_config.is_paused, ExchangeError::Paused);
    require!(ctx.accounts.maker.key() == order.maker, ExchangeError::NotOrderMaker);
    require!(order.salt != 0, ExchangeError::ZeroSaltCannotCancel);
    
    let order_key_hash = compute_order_key_hash(&order);
    let order_fill = &mut ctx.accounts.order_fill;
    order_fill.fill_amount = u64::MAX;
    
    emit!(CancelOrderEvent {
        order_key_hash,
        maker: order.maker,
    });
    Ok(())
}
```

#### 3.5 Implement `match_orders` Instruction (Core)

This is the most complex instruction. Break it down into sub-functions:

1. `validate_and_verify_orders` — ERC-20 whitelist, time, asset class, signatures
2. `match_assets` — verify asset compatibility
3. `calculate_fills` — compute fill amounts, update PDA state
4. `process_sol_and_wsol` — handle SOL/wSOL conversion
5. `do_transfers` — determine fee side, execute all transfers
6. `do_transfers_with_fees` — protocol fee + royalties + origin fees + payouts
7. `transfer_asset` — low-level SPL/SOL transfer via CPI

### Deliverables
- [ ] All library functions ported and unit-tested
- [ ] `cancel_order` instruction working and tested
- [ ] `match_orders` instruction working for basic cases
- [ ] Unit tests for each library function
- [ ] Integration tests for basic order matching (SPL token for NFT)

---

## Phase 4: Oracle / Royalties Integration (Week 6-8)

### Objectives
- Complete the royalties registry program
- Integrate royalty lookup into the exchange's match flow
- Test royalty distribution

### Tasks

#### 4.1 Complete RoyaltiesRegistry Program
- Implement all royalty setter instructions
- Implement cascading royalty lookup logic
- Implement authority verification (collection owner/creator via Metaplex metadata)

#### 4.2 Royalty Lookup in Exchange

The exchange program reads royalty PDA accounts directly (passed as remaining accounts):

```rust
fn get_royalties(
    remaining_accounts: &[AccountInfo],
    collection_mint: &Pubkey,
    token_id: u64,
    royalties_registry_program: &Pubkey,
) -> Result<Vec<RoyaltyPart>> {
    // 1. Try to find and read OwnerTokenRoyalties PDA
    let owner_token_seeds = [b"owner_royalties", collection_mint.as_ref(), &token_id.to_le_bytes()];
    let (expected_pda, _) = Pubkey::find_program_address(&owner_token_seeds, royalties_registry_program);
    // Search remaining_accounts for this PDA, read if found...
    
    // 2. If not initialized, try CollectionRoyalties PDA
    // 3. Try CreatorTokenRoyalties PDA
    // 4. Merge if both found
    // 5. Return result
}
```

#### 4.3 Metaplex Integration
For verifying collection owners and token creators:
- Read Metaplex Token Metadata account
- Verify `update_authority` matches the signer (for owner royalties)
- Verify creator array for creator royalties

#### 4.4 External Royalty Providers
Implement CPI to external royalty provider programs (optional, can be deferred):
- `RoyaltyProvider` PDA stores the external program ID
- CPI call to the provider program to fetch royalties
- Validate response (sum ≤ 10000 bps)

### Deliverables
- [ ] RoyaltiesRegistry program fully implemented
- [ ] Royalty lookup integrated into `match_orders`
- [ ] Royalty distribution in fee transfers verified
- [ ] Metaplex metadata integration for owner/creator verification
- [ ] Tests for all royalty scenarios (owner, creator, merged, collection-level, no royalties)

---

## Phase 5: Token Handling & SOL/wSOL (Week 8-10)

### Objectives
- Implement all token transfer logic (SOL, wSOL, SPL tokens, NFTs)
- Handle SOL ↔ wSOL conversions
- Implement the full fee distribution pipeline

### Tasks

#### 5.1 Implement Transfer Functions

```rust
fn transfer_asset(
    asset: &Asset,
    from: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    token_program: &AccountInfo,
    system_program: &AccountInfo,
    // ... additional accounts
) -> Result<()> {
    match asset.asset_type.asset_class {
        AssetClass::Sol => {
            // system_program::transfer CPI
        },
        AssetClass::WrappedSol | AssetClass::SplToken => {
            // spl_token::transfer CPI
        },
        AssetClass::Nft | AssetClass::SemiFungible => {
            // spl_token::transfer CPI (amount=1 for NFT)
        },
    }
}
```

#### 5.2 Implement SOL/wSOL Conversion
```rust
fn process_sol_and_wsol(
    // Equivalent to Exchange.processEthAndWeth
    // Handle: SOL → wSOL wrapping, wSOL → SOL unwrapping
    // Create temporary wSOL accounts if needed
    // Close temporary accounts after use
) -> Result<()> { ... }
```

Key operations:
- **Wrap SOL:** Create temp wSOL ATA, transfer SOL, call `sync_native`
- **Unwrap SOL:** Transfer wSOL to temp account, close account (SOL goes to owner)

#### 5.3 Implement Full Fee Distribution Pipeline
Port the complete `doTransfersWithFees` flow:
1. Calculate total amount with origin fees
2. Transfer protocol fee (to fee receiver)
3. Fetch and transfer royalties (from royalties registry)
4. Transfer origin fees (both sides)
5. Transfer remaining payouts (with 100% sum validation)

#### 5.4 Implement Collection Bid Matching
Port `ExchangeHelper.matchCollectionBidOrder`:
1. Verify collection bid order signature and matchAllowance
2. Validate maker and taker orders
3. Calculate collection bid fills
4. Format synthetic maker orders
5. Execute each matched pair

### Deliverables
- [ ] All transfer types working (SOL, wSOL, SPL, NFT, SFT)
- [ ] SOL/wSOL conversion tested
- [ ] Full fee pipeline working (protocol fee + royalties + origin fees + payouts)
- [ ] Collection bid matching working
- [ ] Integration tests for all transfer scenarios
- [ ] Integration tests for fee distribution correctness

---

## Phase 6: Testing (Week 10-13)

### Objectives
- Comprehensive test coverage
- Security testing
- Performance testing
- Fuzz testing

### Tasks

#### 6.1 Unit Tests
Each Rust module should have unit tests:
- `logic/bps.rs` — basis point calculations
- `logic/math.rs` — partial amount calculations, rounding error detection
- `logic/order.rs` — order hashing, validation, remaining calculations
- `logic/fill.rs` — fill computation for all cases (fillLeft, fillRight, collectionBid)
- `logic/fee_side.rs` — fee side determination for all asset class combinations
- `logic/order_data.rs` — data parsing
- `logic/exchange.rs` — asset matching, total amount calculation
- `logic/signature.rs` — Ed25519 verification

#### 6.2 Integration Tests (TypeScript / Anchor)
```
tests/
├── exchange/
│   ├── initialize.test.ts
│   ├── match_orders_eth_for_nft.test.ts
│   ├── match_orders_wsol_for_nft.test.ts
│   ├── match_orders_spl_for_nft.test.ts
│   ├── match_orders_partial_fill.test.ts
│   ├── match_orders_collection_bid.test.ts
│   ├── match_orders_with_royalties.test.ts
│   ├── match_orders_with_origin_fees.test.ts
│   ├── match_orders_with_payouts.test.ts
│   ├── match_orders_with_fees_and_royalties.test.ts
│   ├── cancel_order.test.ts
│   ├── batch_operations.test.ts
│   ├── admin_operations.test.ts
│   ├── pause_unpause.test.ts
│   ├── erc20_whitelist.test.ts
│   └── access_control.test.ts
├── royalties-registry/
│   ├── initialize.test.ts
│   ├── set_royalties.test.ts
│   ├── get_royalties.test.ts
│   └── access_control.test.ts
└── utils/
    ├── order_helper.ts       # Order creation and signing utilities
    ├── signature_helper.ts   # Ed25519 signing utilities
    └── setup.ts              # Test environment setup
```

#### 6.3 Security Tests
Test specific attack vectors:
- [ ] Account substitution — pass wrong PDA, wrong token account
- [ ] Unauthorized access — wrong signer for admin operations
- [ ] Order replay — try to fill a cancelled order, try to over-fill
- [ ] Signature forgery — invalid signatures, wrong signer
- [ ] matchAllowance expiry — expired timestamps
- [ ] Rounding attacks — orders designed to maximize rounding error
- [ ] Counterparty bypass — fill an order with wrong taker constraint
- [ ] Collection bid manipulation — malicious taker orders
- [ ] Whitelist bypass — trade with non-whitelisted token
- [ ] Paused state bypass — try operations when paused
- [ ] Integer overflow — extreme values in amounts, bps

#### 6.4 Economic Invariant Tests
For each test, verify:
- [ ] Sum of all outgoing transfers == sum of all incoming transfers
- [ ] Protocol fee == expected bps of amount
- [ ] Royalties == expected bps of amount (within rounding)
- [ ] Origin fees == expected bps of amount
- [ ] Payouts sum to 100% of remaining
- [ ] Fill amounts correctly reflect partial fills
- [ ] No dust left in temporary accounts

#### 6.5 Fuzz Testing
Use `cargo-fuzz` or similar to fuzz:
- Order validation logic
- Fill computation
- Fee calculation
- Signature verification

#### 6.6 Performance Testing
- Measure compute units for `match_orders` with varying complexity
- Measure transaction size for different account counts
- Determine practical limits for:
  - Number of royalty recipients per trade
  - Number of payout recipients per order
  - Number of origin fee parts per order
  - Batch sizes for collection bids

### Deliverables
- [ ] >90% code coverage from unit tests
- [ ] All integration tests passing
- [ ] All security attack vectors tested and passing
- [ ] Economic invariants verified for all test cases
- [ ] Performance benchmarks documented
- [ ] Fuzz testing results clean

---

## Phase 7: Deployment (Week 13-16)

### Objectives
- Deploy to devnet for testing
- Deploy to mainnet
- Set up monitoring and operations

### Tasks

#### 7.1 Devnet Deployment
1. Deploy both programs to Solana devnet
2. Initialize config accounts with test parameters
3. Create test SPL tokens and NFTs
4. Run full integration test suite against devnet
5. Test with Phantom/Solflare wallets
6. Stress test with concurrent transactions

#### 7.2 Security Audit
- Engage a Solana-specialized security auditor
- Provide:
  - Full source code
  - This documentation set
  - Test results
  - Deployment guide
- Address all findings before mainnet

#### 7.3 Mainnet Deployment Preparation
1. Set up multisig wallet (Squads) for:
   - Program upgrade authority
   - Exchange owner operations
2. Prepare deployment configuration:
   ```
   Order Book: <order-book-ed25519-pubkey>
   Default Fee Receiver: <treasury-pubkey>
   Royalties Registry: <registry-program-id>
   wSOL Mint: So11111111111111111111111111111111111111112
   Protocol Fee BPS: 0 (or configured value)
   ```
3. Deploy programs with upgrade authority set to multisig
4. Initialize config accounts
5. Whitelist initial SPL tokens (wSOL, USDC, etc.)

#### 7.4 Mainnet Deployment
1. Deploy `royalties_registry` program
2. Deploy `exchange` program
3. Initialize `RegistryConfig`
4. Initialize `ExchangeConfig`
5. Whitelist tokens
6. Verify all accounts and configuration
7. Run sanity test trades

#### 7.5 Post-Deployment Operations
1. **Monitoring:**
   - Set up log monitoring for program events
   - Set up account balance monitoring
   - Set up transaction success/failure monitoring
2. **Indexing:**
   - Deploy off-chain indexer for `MatchEvent`, `CancelOrderEvent`, `TransferEvent`
   - Integrate with existing Order Book service
3. **Emergency procedures:**
   - Document pause/unpause procedure
   - Document upgrade procedure
   - Document fee receiver change procedure
   - Test emergency procedures on devnet

### Deliverables
- [ ] Programs deployed to devnet and fully tested
- [ ] Security audit completed and all findings addressed
- [ ] Programs deployed to mainnet
- [ ] Configuration verified
- [ ] Monitoring and alerting in place
- [ ] Off-chain indexer running
- [ ] Emergency procedures documented and tested
- [ ] Order Book service integrated with Solana programs

---

## Dependencies & Prerequisites

| Dependency | Version | Purpose |
|---|---|---|
| Anchor Framework | 0.30+ | Program framework |
| Solana CLI | 1.18+ | Deployment and testing |
| Rust | 1.75+ | Program language |
| Node.js | 18+ | TypeScript tests |
| `@solana/web3.js` | 1.90+ | Client library |
| `@coral-xyz/anchor` | 0.30+ | Anchor TypeScript client |
| `@solana/spl-token` | 0.4+ | SPL Token operations |
| `@metaplex-foundation/mpl-token-metadata` | latest | Metaplex metadata integration |
| Squads SDK | latest | Multisig deployment |

---

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Transaction size limit for complex trades | High | Medium | Use versioned transactions + address lookup tables; limit royalty/payout recipients |
| Compute budget exceeded | Medium | High | Profile CU usage early; optimize hot paths; set appropriate compute limits |
| Ed25519 vs secp256k1 decision | Low | High | Start with Ed25519; implement secp256k1 adapter if cross-chain compatibility needed |
| Metaplex metadata changes | Low | Medium | Use stable Metaplex APIs; abstract metadata access behind a trait |
| Security audit findings | Medium | High | Budget 2-3 weeks for remediation after audit |
| Off-chain Order Book integration complexity | Medium | Medium | Define interface early; build adapter layer |
