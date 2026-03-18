# Solana Migration Mapping — EVM → Solana

> Phase 2: Complete mapping of EVM Launchpad components to Solana (Anchor) architecture

---

## 1. What Remains the SAME

### Core Logic Preserved
- **Three Collection Types**: Standard, Refundable 100%, Refundable 80/20 → Will be modeled as a single Anchor program with a `collection_type` enum
- **Sale Lifecycle**: Presale (whitelist-gated) → Public Sale → ongoing
- **Minting Controls**: Per-user limits, per-tx limits, max supply cap
- **Whitelist System**: Per-user mint limit (not boolean), owner-managed
- **Owner Mint**: Free mint by authority, no price check, non-refundable
- **Refund Logic**: Token burn + escrow return, owner-minted tokens non-refundable
- **80/20 Split**: 20% to owner immediately, 80% held for potential refund
- **Remint Logic**: Refunded token IDs recycled when max supply reached
- **Pause/Unpause**: Global pause mechanism

### Invariants Preserved
1. `mintedAmount` ≤ `maxMintSupply` at all times
2. A token marked `isOwnerMint` can never be refunded
3. `refundPrice[token]` is set at mint time and never changes
4. Free presale mints consume from `reservedNFTs` pool
5. `totalMints` is monotonically increasing (never decremented)
6. On refund, `mintedAmount` decreases, `refundCounter` increases

---

## 2. What CHANGES for Solana

### 2.1 Storage → PDA Accounts

| EVM Pattern | Solana Pattern |
|---|---|
| Contract storage variables | PDA-derived accounts with structured data |
| `mapping(address => uint256)` | Per-user PDA accounts (seeds: `[prefix, collection, user]`) |
| `mapping(uint256 => bool)` | Per-token PDA accounts or bitmap in collection state |
| `mapping(uint256 => uint256)` | Per-token PDA accounts |
| `uint256[]` array | Vector in account or separate list account |
| Single contract → single storage | Multiple PDA accounts per collection |

### 2.2 msg.sender → Signer Accounts

| EVM | Solana |
|---|---|
| `msg.sender` | `Signer` account constraint |
| `onlyOwner` modifier | `has_one = authority` constraint on collection PDA |
| `msg.value` | SPL Token transfer or SOL system_program transfer |

### 2.3 ERC721 → Metaplex / Custom NFT

| EVM | Solana |
|---|---|
| ERC721 `_safeMint` | Metaplex Token Metadata `create_metadata_accounts_v3` + mint |
| ERC721 `_burn` | Token close + metadata burn |
| `ownerOf(tokenId)` | Token account owner check |
| `totalSupply()` | Tracked in collection account state |
| Token ID auto-increment | Mint keypair + sequential index in collection |
| `baseURI + tokenId` | Metaplex metadata URI |

### 2.4 Native Token Transfer → SOL Transfer

| EVM | Solana |
|---|---|
| `payable` function | Implicit SOL transfer via system_program |
| `msg.value` check | Manual lamport balance check or CPI to system_program::transfer |
| `_transferNRG()` | `system_program::transfer` CPI |
| Contract holds ETH | PDA vault holds SOL |

### 2.5 Events → Anchor Events / Logs

| EVM | Solana |
|---|---|
| Solidity `event` + `emit` | `#[event]` + `emit!()` in Anchor |
| Indexed parameters | Off-chain indexing (Geyser/Yellowstone) |

### 2.6 Modifiers → Manual Checks

| EVM Modifier | Solana Equivalent |
|---|---|
| `noContracts` | Not needed (Solana txns always from wallet/signer) — but CPI guard for safety |
| `whenNotPaused` | Manual check `require!(!collection.paused, ...)` |
| `nonReentrant` | Not needed (Solana runtime prevents reentrancy) |
| `onlyOwner` | `has_one = authority` or `constraint = signer.key() == collection.authority` |
| `mintCompliance` | Inline `require!()` check |

### 2.7 OperatorFilter → Solana Transfer Hooks or Program Checks

| EVM | Solana |
|---|---|
| OperatorFilter modifier on transfer | Transfer Hook extension (Token-2022) OR custom transfer instruction |
| `isOperatorAllowed()` | Check against OperatorRegistry PDA |
| Codehash whitelist | Not applicable on Solana |

---

## 3. PDA Architecture

### 3.1 Collection Config PDA
```
Seeds: ["collection", collection_id.as_bytes()]
Bump: auto-derived
```
**Stores**: All collection-level configuration (name, symbol, prices, limits, supply, sale status, authority, collection_type, pause state)

### 3.2 Mint Counter PDA (Per User per Collection)
```
Seeds: ["mint_counter", collection.key(), user.key()]
Bump: auto-derived
```
**Stores**: `number_minted` (public), `presale_number_minted` (presale)

### 3.3 Whitelist Entry PDA
```
Seeds: ["whitelist", collection.key(), user.key()]
Bump: auto-derived
```
**Stores**: `mint_limit` (u64), `is_active` (bool)

### 3.4 Token Record PDA (Per NFT)
```
Seeds: ["token_record", collection.key(), mint.key()]
Bump: auto-derived
```
**Stores**: `is_owner_mint` (bool), `refund_price` (u64), `token_index` (u64)

### 3.5 Vault PDA (Escrow for Refundable Collections)
```
Seeds: ["vault", collection.key()]
Bump: auto-derived
```
**Purpose**: Holds SOL for potential refunds. System-owned PDA that the program controls.

### 3.6 Operator Registry PDA
```
Seeds: ["operator_registry"]
Bump: auto-derived
```
**Stores**: `authority`, `fund_receiver`, `share_percentage_bps`, `paused`

### 3.7 Operator Whitelist PDA
```
Seeds: ["operator_whitelist", collection.key(), operator.key()]
Bump: auto-derived
```
**Stores**: `is_allowed` (bool)

### 3.8 Universal Operator PDA
```
Seeds: ["universal_operator", operator.key()]
Bump: auto-derived
```
**Stores**: `is_allowed` (bool)

---

## 4. Instruction Set Mapping

### 4.1 Collection Management

| EVM Function | Solana Instruction | Accounts Required |
|---|---|---|
| `constructor(...)` | `initialize_collection` | `collection` (init), `authority` (signer+payer), `vault` (init, refundable only), `system_program` |
| `publicsaleConfig(...)` | `configure_publicsale` | `collection` (mut), `authority` (signer) |
| `presaleConfig(...)` | `configure_presale` | `collection` (mut), `authority` (signer) |
| `togglePresale()` | `toggle_presale` | `collection` (mut), `authority` (signer) |
| `togglePublicsale()` | `toggle_publicsale` | `collection` (mut), `authority` (signer) |
| `togglePause()` | `toggle_pause` | `collection` (mut), `authority` (signer) |
| `setBaseURI(...)` | `set_base_uri` | `collection` (mut), `authority` (signer) |

### 4.2 Whitelist Management

| EVM Function | Solana Instruction | Accounts Required |
|---|---|---|
| `addWhitelist(users, limits)` | `add_whitelist` (per-user, or batch with remaining accounts) | `collection`, `authority` (signer+payer), `whitelist_entry` (init), `user` (pubkey), `system_program` |
| `removeWhitelist(users)` | `remove_whitelist` | `collection`, `authority` (signer), `whitelist_entry` (mut/close) |

### 4.3 Minting

| EVM Function | Solana Instruction | Accounts Required |
|---|---|---|
| `mint(quantity)` | `mint_public` | `collection` (mut), `minter` (signer+payer), `mint_counter` (init_if_needed/mut), `token_record` (init), `mint` (init), `token_account` (init), `metadata` (init), `vault` (mut, refundable only), `authority` (for SOL transfer dest), `token_program`, `metadata_program`, `system_program`, `rent` |
| `presaleMint(quantity)` | `mint_presale` | Same as above + `whitelist_entry` (verified) |
| `ownerMint(to, quantity)` | `mint_owner` | `collection` (mut), `authority` (signer+payer), `recipient`, `token_record` (init), `mint` (init), `token_account` (init), `metadata` (init), `token_program`, `metadata_program`, `system_program` |

### 4.4 Refund

| EVM Function | Solana Instruction | Accounts Required |
|---|---|---|
| `refund(tokenIds)` | `refund_nft` (per-token, or batch) | `collection` (mut), `owner` (signer), `token_record` (mut/close), `mint` (mut), `token_account` (mut), `vault` (mut), `token_program`, `system_program` |

### 4.5 Operator Registry

| EVM Function | Solana Instruction |
|---|---|
| `initialize(...)` | `initialize_registry` |
| `addWhitelist(collection, operator)` | `add_operator_whitelist` |
| `removeWhitelist(collection, operator)` | `remove_operator_whitelist` |
| `addUniversalOperator(operator)` | `add_universal_operator` |
| `removeUniversalOperator(operator)` | `remove_universal_operator` |
| `changeFundReceiver(addr)` | `change_fund_receiver` |
| `changeSharePercentageBps(bps)` | `change_share_percentage` |
| `pause()/unpause()` | `toggle_registry_pause` |

---

## 5. Account Schemas

### 5.1 Collection Account (~512 bytes)
```rust
#[account]
pub struct Collection {
    pub authority: Pubkey,                // 32 bytes
    pub collection_type: CollectionType,  // 1 byte (enum: Standard, Refundable100, Refundable80)
    pub has_operator_filter: bool,        // 1 byte
    pub operator_registry: Option<Pubkey>,// 33 bytes (1 + 32)
    
    // Supply
    pub max_mint_supply: u64,             // 8
    pub minted_amount: u64,               // 8
    pub total_mints: u64,                 // 8 (for refundable: gross ever minted)
    pub refund_counter: u64,              // 8
    pub refunded_token_ids: Vec<u64>,     // 4 + n*8 (dynamic) — capped
    
    // Pricing
    pub mint_price: u64,                  // 8
    pub presale_mint_price: u64,          // 8
    
    // Limits
    pub max_user_mint_amount: u64,        // 8
    pub max_tx_mint_amount: u64,          // 8
    pub presale_max_user_mint_amount: u64,// 8
    pub presale_max_tx_mint_amount: u64,  // 8
    
    // Sale Status
    pub presale_active: bool,             // 1
    pub publicsale_active: bool,          // 1
    pub paused: bool,                     // 1
    
    // Reserved (for Refundable variants)
    pub reserved_nfts: u64,              // 8
    pub reserved_mints: u64,             // 8
    
    // Metadata
    pub name: String,                    // 4 + len
    pub symbol: String,                  // 4 + len
    pub base_uri: String,               // 4 + len
    
    pub bump: u8,                        // 1
    pub vault_bump: u8,                  // 1
}
```

### 5.2 MintCounter Account (48 bytes)
```rust
#[account]
pub struct MintCounter {
    pub collection: Pubkey,       // 32
    pub user: Pubkey,             // 32 (redundant but useful for validation)
    pub number_minted: u64,       // 8
    pub presale_number_minted: u64, // 8
    pub bump: u8,                 // 1
}
```

### 5.3 WhitelistEntry Account (42 bytes)
```rust
#[account]
pub struct WhitelistEntry {
    pub collection: Pubkey,  // 32
    pub user: Pubkey,        // 32
    pub mint_limit: u64,     // 8
    pub bump: u8,            // 1
}
```

### 5.4 TokenRecord Account (50 bytes)
```rust
#[account]
pub struct TokenRecord {
    pub collection: Pubkey,    // 32
    pub mint: Pubkey,          // 32
    pub token_index: u64,      // 8
    pub refund_price: u64,     // 8
    pub is_owner_mint: bool,   // 1
    pub bump: u8,              // 1
}
```

### 5.5 OperatorRegistryState Account
```rust
#[account]
pub struct OperatorRegistryState {
    pub authority: Pubkey,           // 32
    pub fund_receiver: Pubkey,       // 32
    pub share_percentage_bps: u64,   // 8
    pub paused: bool,                // 1
    pub bump: u8,                    // 1
}
```

### 5.6 OperatorWhitelist Account
```rust
#[account]
pub struct OperatorWhitelist {
    pub collection: Pubkey,  // 32
    pub operator: Pubkey,    // 32
    pub is_allowed: bool,    // 1
    pub bump: u8,            // 1
}
```

---

## 6. Cross-Program Interactions

### Required CPIs
1. **System Program**: SOL transfers (mint payment, refund disbursement)
2. **Token Program**: SPL Token mint/burn/transfer
3. **Associated Token Program**: Create associated token accounts
4. **Metaplex Token Metadata Program**: Create/update metadata, create master edition
5. **Rent Sysvar**: Rent-exempt checks

### CPI Security Rules
- All CPI target program IDs MUST be validated against expected program IDs
- PDA signing must use verified seeds + bump
- No arbitrary CPI targets allowed
