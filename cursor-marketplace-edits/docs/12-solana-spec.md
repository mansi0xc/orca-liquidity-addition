# Solana Marketplace Spec — Energi GMI Exchange

**Program ID:** `CuUtAt1GpoDoDGnrQbjeZTH7qSPdVwahybi4KoucQ6zN`
**Framework:** Anchor 0.31.1

---

## 1. Instruction List

| ID | Instruction | Handler File |
|----|-------------|-------------|
| I1 | `initialize` | instructions/initialize.rs |
| I2 | `match_orders` | instructions/match_orders.rs |
| I3 | `cancel_order` | instructions/cancel_order.rs |
| I4 | `set_protocol_fee_bps` | instructions/admin.rs |
| I5 | `set_default_fee_receiver` | instructions/admin.rs |
| I6 | `set_fee_receiver` | instructions/admin.rs |
| I7 | `set_allowed_token` | instructions/admin.rs |
| I8 | `set_order_book` | instructions/admin.rs |
| I9 | `set_exchange_owner` | instructions/admin.rs |
| I10 | `toggle_pause` | instructions/admin.rs |
| I11 | `set_royalties_registry_program` | instructions/admin.rs |
| I12 | `safe_transfer_spl` | instructions/admin.rs |

---

## 2. Instruction Details

### I1: initialize

**Accounts:**
| Account | Type | Seeds |
|---------|------|-------|
| exchange_config | PDA (init) | `["exchange_config"]` |
| authority | Signer (payer) | — |
| system_program | Program | — |

**Parameters (InitializeArgs):**
- `order_book: Pubkey`
- `default_fee_receiver: Pubkey`
- `royalties_registry_program: Pubkey`
- `wsol_mint: Pubkey`
- `exchange_owner: Pubkey`
- `protocol_fee_bps: u16`

**Constraints:** `protocol_fee_bps <= 10000`

---

### I2: match_orders

**Accounts:**
| Account | Type | Seeds | Constraint |
|---------|------|-------|------------|
| exchange_config | PDA | `["exchange_config"]` | bump verified |
| payer | Signer (mut) | — | tx fee payer |
| left_order_fill | PDA (init_if_needed) | `["order_fill", left_order_key_hash]` | — |
| right_order_fill | PDA (init_if_needed) | `["order_fill", right_order_key_hash]` | — |
| instructions_sysvar | AccountInfo | — | `address = sysvar::instructions::ID` |
| exchange_authority | PDA | `["exchange_authority"]` | token delegate |
| token_program | Program(Token) | — | — |
| system_program | Program(System) | — | — |
| remaining_accounts[] | Dynamic | — | See transfer layout |

**Parameters (MatchOrdersArgs):**
- `left_order_key_hash: [u8; 32]`
- `right_order_key_hash: [u8; 32]`
- `order_left: Order`
- `signature_left: Vec<u8>`
- `match_left_before_timestamp: i64`
- `order_book_signature_left: Vec<u8>`
- `order_right: Order`
- `signature_right: Vec<u8>`
- `match_right_before_timestamp: i64`
- `order_book_signature_right: Vec<u8>`
- `royalty_parts: Vec<Part>`

**Validation Steps:**
1. Exchange not paused
2. Right order make_asset ≠ Sol (maker can't pay native SOL)
3. Both orders `collection_bid == false`
4. Order time validation (start/end vs clock)
5. Counterparty check (taker field if non-default)
6. Order key hash verification (computed == provided)
7. Ed25519 signature verification (see Section 3)
8. Asset compatibility check (fungible ↔ non-fungible)
9. Fill calculation and overflow check
10. Transfer execution with fee deductions

**CPIs:**
- `token::transfer` via exchange_authority PDA (for SPL transfers)
- `system_instruction::transfer` (for SOL transfers)

---

### I3: cancel_order

**Accounts:**
| Account | Type | Seeds |
|---------|------|-------|
| exchange_config | PDA | `["exchange_config"]` |
| maker | Signer | — |
| order_fill | PDA (init_if_needed) | `["order_fill", order_key_hash]` |
| payer | Signer (mut) | — |
| system_program | Program | — |

**Constraints:**
1. Exchange not paused
2. `maker == signer` (must match order.maker)
3. `order.salt != 0` (zero-salt orders cannot be cancelled)
4. Computed key hash matches provided hash

**State Change:** `order_fill.fill_amount = u64::MAX`

---

## 3. Signature Verification

### Ed25519 Program Usage

Signatures are verified via **instruction introspection** of pre-appended Ed25519 program instructions in the transaction:

```
Transaction layout:
  IX[0]: Ed25519 verify (order_book match allowance for left order)
  IX[1]: Ed25519 verify (maker signature for left order, if payer != maker)
  IX[2]: Ed25519 verify (order_book match allowance for right order)
  IX[3]: Ed25519 verify (maker signature for right order, if payer != maker)
  IX[N]: match_orders instruction (the actual program call)
```

The program loads each Ed25519 instruction from the sysvar and verifies:
1. Instruction is from the Ed25519 program
2. Public key in the instruction matches expected signer
3. Message in the instruction matches expected hash

### Message Formats

**Order Hash (signed by maker):**
```
SHA256(
    program_id ||
    "energi" ||
    0x01 (version) ||
    maker || hash_asset_type(make_asset) || make_value ||
    taker || hash_asset_type(take_asset) || take_value ||
    salt || start || end || data_type ||
    SHA256(data) || collection_bid
)
```

**Match Allowance Hash (signed by order_book):**
```
SHA256(
    program_id ||
    "energi" ||
    0x01 (version) ||
    order_key_hash ||
    match_before_timestamp
)
```

**Order Key Hash (for PDA derivation and fill tracking):**
```
SHA256(
    maker || hash_asset_type(make_asset_type) ||
    hash_asset_type(take_asset_type) || salt || collection_bid
)
```

### Signature Rules by Salt Value

| Salt | Condition | Verification |
|------|-----------|-------------|
| > 0 | Always | Order book signs match_allowance_hash |
| > 0 | payer ≠ maker | Maker signs order_hash |
| > 0 | payer == maker | Maker signature skipped (payer is maker) |
| == 0 | maker ≠ default | payer must equal maker (tx signer) |
| == 0 | maker == default | No verification |

---

## 4. NFT Transfer Logic

### Asset Classes
```rust
enum AssetClass {
    Sol,          // Native SOL (lamports)
    WrappedSol,   // wSOL SPL token
    SplToken,     // Fungible SPL token
    Nft,          // Non-fungible token (SPL with 1 supply)
    SemiFungible, // Semi-fungible token
}
```

### Compatibility Rules
- Fungible (Sol/WrappedSol/SplToken) ↔ Non-fungible (Nft/SemiFungible)
- NFT-to-NFT trades are not allowed
- Sol and WrappedSol are mutually compatible for matching

### Transfer Dispatch
| Asset Class | Transfer Method |
|-------------|----------------|
| Sol | `system_instruction::transfer` (payer must be signer) |
| All others | `token::transfer` via CPI with exchange_authority PDA as delegate |

### Token Account Requirements
- Source token accounts must have `exchange_authority` PDA approved as delegate
- Delegation must cover the full transfer amount
- Token accounts must exist before match_orders is called

---

## 5. Escrow / Custody Model

**Delegation model (NOT escrow):**

- No tokens are held in escrow at any point
- Token source accounts approve `exchange_authority` PDA as delegate
- On match, the PDA signs CPI transfers atomically
- If any transfer fails, entire transaction reverts
- No intermediate hold period

**PDA Authority:** seeds = `["exchange_authority"]`

---

## 6. Remaining Accounts Usage

### Layout: FeeSide::Make or FeeSide::Take
```
[0]              fee_payer_source (ATA/wallet)
[1]              protocol_fee_receiver_dest
[2..2+R]         royalty_recipient_dests
[2+R..2+R+O1]    fee_payer origin_fee dests
[2+R+O1..+O2]    other origin_fee dests
[...+P1]         other_order payout dests
[next]           non_fee_source (NFT ATA)
[next+1..+P2]    fee_payer_order payout dests
```

### Layout: FeeSide::None
```
[0]              source_make (make asset ATA)
[1..1+PL]        left_order payout dests
[1+PL]           source_take (take asset ATA)
[2+PL..+PR]      right_order payout dests
```

**Walker pattern:** Sequential `AccountWalker` struct with bounds checking. Throws `InsufficientRemainingAccounts` if underrun.

**CRITICAL NOTE:** No validation is performed on remaining account addresses beyond bounds checking. The program trusts that the correct accounts are provided.

---

## 7. State Accounts

### ExchangeConfig (PDA: `["exchange_config"]`)
```
owner: Pubkey                      // Deployer, highest authority
exchange_owner: Pubkey             // Fee management authority
order_book: Pubkey                 // Match allowance signer
default_fee_receiver: Pubkey       // Default protocol fee recipient
royalties_registry_program: Pubkey // Royalty lookup program
wsol_mint: Pubkey                  // Wrapped SOL mint
protocol_fee_bps: u16             // Protocol fee basis points
is_paused: bool                    // Pause flag
bump: u8                           // PDA bump
```

### OrderFill (PDA: `["order_fill", order_key_hash]`)
```
fill_amount: u64    // Cumulative take-asset filled; u64::MAX = cancelled
bump: u8
```

### AllowedToken (PDA: `["allowed_token", mint]`)
```
is_allowed: bool
bump: u8
```

### FeeReceiver (PDA: `["fee_receiver", mint]`)
```
receiver: Pubkey
bump: u8
```

---

## 8. Events

### MatchEvent
```
left_order_key_hash: [u8; 32]
right_order_key_hash: [u8; 32]
left_maker: Pubkey
right_maker: Pubkey
new_left_fill: u64
new_right_fill: u64
```

### CancelOrderEvent
```
order_key_hash: [u8; 32]
maker: Pubkey
```

### TransferEvent (defined but NOT emitted)
```
asset_class: u8, from: Pubkey, to: Pubkey, mint: Pubkey,
value: u64, transfer_direction: u8, transfer_type: u8
```

---

## 9. Error Codes (37 total)

Key errors:
- `Paused` — exchange is paused
- `MakerCannotPayWithSol` — maker can't use native SOL
- `OrderExpired` / `OrderNotStarted` — time constraints
- `InvalidSignature` — Ed25519 verification failed
- `MatchAllowanceExpired` — order book timestamp expired
- `OrderCancelled` — fill == u64::MAX
- `NothingToFill` — no remaining fill amount
- `RoyaltiesTooHigh` — royalties > 50%
- `InvalidPayoutSum` — payouts ≠ 10000 bps
- `MakerMustBeSignerForZeroSalt` — zero-salt requires maker as payer
- `CollectionBidMustUseCollectionBidInstruction` — collection bids blocked
- `InsufficientRemainingAccounts` — not enough remaining accounts
